//! Dictation's IO on the hub: `bots.voice`, `/voice/in`, the OpenAI key, the helper. The helper (`backplane-voice`, native/voice) is a child process: it holds the microphone,
//! the engines and the downloads, so nothing of them loads in the hub and a crash there ends one session. One live session at a time; its messages go to the client that
//! started it: `{"t":"voice","thread","ev":"view","view"}` (what to draw), `"level"`, `"edit"` (place::Edit), `"draft"` (a chip changed the words already there).
//!
//! The key is a secret (`<home>/secrets/openai.key`, 0600): the helper reads the file, so it is never in an argv, an environment, the log or a message.
use super::{dictation::{Dictation, Out, Reader}, page::{self, Prefs}, resident, vdict::{self, VDict}, words};
use crate::{api::{Body, Caller, Fut, HttpHandler, HttpReply, RpcHandler}, auth::Req, hub::{now_ms, Hub}, net::{botnet, pairing}, proc::{self, Kind}};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, HashMap}, path::{Path, PathBuf}, process::Stdio, sync::{Arc, Mutex}, time::{Duration, Instant}};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, sync::mpsc};

const COPY_NO: &str = "a hub on a copied log (BACKPLANE_COPY=1) starts no dictation";
const NO_HELPER: &str = "backplane-voice did not start (is it installed next to Backplane?)";
const SET_UP: &str = "set up dictation in Settings, Voice: download the local model or add an OpenAI API key";
/// A client's last upload gets this long after its stop; then stdin closes anyway.
const UPLOAD_GRACE: Duration = Duration::from_secs(3);
/// A helper told to stop has this long to finish its last words.
const STOP_LIMIT: Duration = Duration::from_secs(20);
const PIECE_MAX: u64 = 4 << 20;

pub fn key_path(home: &Path) -> PathBuf { home.join("secrets/openai.key") }
fn dir(home: &Path) -> PathBuf { home.join("voice") }
fn local_dir(home: &Path) -> PathBuf { home.join("voice/local") }
/// Terms older hubs kept; read as more entries, never written.
fn dict_txt(home: &Path) -> PathBuf { home.join("voice/dictionary.txt") }
fn keywords_path(home: &Path) -> PathBuf { home.join("voice/keywords.txt") }
fn text_of(p: &Path) -> String { std::fs::read_to_string(p).unwrap_or_default() }
fn s<'a>(p: &'a Value, k: &str) -> &'a str { p.get(k).and_then(Value::as_str).unwrap_or("") }

/// Where the helper is: `BACKPLANE_VOICE_BIN`, else beside this program.
pub fn exe() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("BACKPLANE_VOICE_BIN").filter(|p| !p.is_empty()) { return Some(p.into()) }
    let b = std::env::current_exe().ok()?.parent()?.join(format!("backplane-voice{}", std::env::consts::EXE_SUFFIX)); b.exists().then_some(b)
}
/// The model is downloaded whole (the helper writes ready-<model> last).
pub fn ready(home: &Path, model: &str) -> bool { text_of(&local_dir(home).join(format!("ready-{}", vdict::model_id(model)))).trim() == "ok" }
/// The engine a setting (voice.engine: auto, local, cloud) picks: local when asked for, or for auto when the model is downloaded; else the cloud.
pub fn engine_of(want: &str, ready: bool) -> &'static str { if ready { if want == "cloud" { "openai" } else { "local" } } else if want == "local" { "local" } else { "openai" } }

/// The `voice.*` settings.
pub async fn prefs(hub: &Hub) -> Prefs {
    let seqs: Vec<(String, u32)> = hub.model().await.settings_with_prefix("voice.").map(|(k, q)| (k.to_string(), q)).collect();
    let mut out = BTreeMap::new();
    for (k, q) in seqs { if let Some(v) = hub.store.get(q as usize - 1).ok().and_then(|l| serde_json::from_str::<Value>(&l).ok()).and_then(|v| v["value"].as_str().map(str::to_string)) { if !v.is_empty() { out.insert(k, v); } } }
    out
}
/// The dictionary as it stands: the entries, the learned fixes, and dictionary.txt's terms.
pub fn dict_of(p: &Prefs, home: &Path) -> VDict {
    let lines: Vec<&str> = p.iter().filter(|(k, _)| k.starts_with(vdict::E_PRE)).map(|(_, v)| v.as_str()).collect();
    let learned: Vec<(&str, &str)> = p.iter().filter_map(|(k, v)| k.strip_prefix(vdict::L_PRE).map(|h| (h, v.as_str()))).collect();
    let mut d = VDict::of(&lines, &learned); d.add_lines(&words::dict_parse(&text_of(&dict_txt(home)))); d
}
fn write_keywords(home: &Path, d: &VDict) { botnet::mkdir(&dir(home)); let _ = std::fs::write(keywords_path(home), words::keywords(d.terms()).join("\n")); }
fn live_args(home: &Path, engine: &str, model: &str, upload: bool, device: &str, server: &str) -> Vec<String> {
    let p = |x: PathBuf| x.to_string_lossy().into_owned();
    let mut a: Vec<String> = if engine == "local" { vec!["live".into(), "--engine".into(), "local".into(), "--local-dir".into(), p(local_dir(home)), "--model".into(), vdict::model_id(model).into()] }
        else { vec!["live".into(), "--key-file".into(), p(key_path(home)), "--keywords-file".into(), p(keywords_path(home))] };
    if upload { a.extend(["--source", "stdin", "--rate", "16000"].map(String::from)) } else if !device.is_empty() { a.extend(["--device".to_string(), device.to_string()]) }
    if engine == "local" && !server.is_empty() { a.extend(["--server".to_string(), server.to_string()]) }
    a
}
fn frame(thread: &str, ev: &str, mut body: Value) -> Value { body["t"] = json!("voice"); body["thread"] = json!(thread); body["ev"] = json!(ev); body }

/// `cut`: a send while dictating, to the session's reader (Reader::cut).
struct Live { always: bool, id: u64, client: u64, thread: String, key: String, upload: bool, audio: Option<mpsc::Sender<Vec<u8>>>, stopped: Option<Instant>, ctl: mpsc::UnboundedSender<Ctl> }
/// To a live session's reader: a client's send while dictating (cut, seq), Keep talking in an end-of-turn countdown, Jev's answer about a pause.
enum Ctl { Cut(u64, u64), Hold, Jev(u64, Option<f64>), /** always on: the open thread changed (between messages only) */ Target(String) }
/// Always-on (always.rs): who turned it on and how to start it again; why it is paused ("" listening); muted by the person; the window's focus.
struct Always { client: u64, params: Value, paused: String, off: bool, focused: bool, watching: bool }
#[derive(Default)]
struct St { always: Option<Always>, live: Option<Live>, dicts: HashMap<String, Dictation>, job: bool, next: u64, /** when each thread's last end-of-turn send went */ auto: HashMap<String, Instant> }
#[derive(Default)]
pub struct Voice { st: Mutex<St> }

impl Voice {
    fn with<T>(&self, thread: &str, f: impl FnOnce(&mut Dictation) -> T) -> T { let mut g = self.st.lock().unwrap(); if g.dicts.len() > 64 { let keep = g.live.as_ref().map(|l| l.thread.clone()); g.dicts.retain(|k, _| Some(k) == keep.as_ref()); } f(g.dicts.entry(thread.to_string()).or_default()) }
    fn view(&self, thread: &str) -> Value { self.with(thread, |d| d.view()) }
    fn tell(&self, hub: &Hub, client: u64, thread: &str) { hub.push(client, frame(thread, "view", json!({"view": self.view(thread)}))); }
    fn fail(&self, hub: &Hub, client: u64, thread: &str, why: &str) -> Value { self.with(thread, |d| { d.idle(); d.err = why.to_string(); }); self.tell(hub, client, thread); json!({"path": ""}) }
    /// Closes the helper's stdin: for a microphone session that is the stop, for an upload the end of the audio.
    fn close_audio(&self, id: u64) { let mut g = self.st.lock().unwrap(); if let Some(l) = g.live.as_mut().filter(|l| l.id == id) { l.audio = None; l.stopped.get_or_insert_with(Instant::now); } }

    async fn start(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, p: &Value) -> Value {
        let (home, mut thread, upload) = (hub.cfg.home.clone(), s(p, "thread").to_string(), s(p, "src") == "client");
        let always = p["always"] == true;
        if always {
            // no thread open: Bub's chat
            if thread.is_empty() { thread = bub_thread(hub).await.unwrap_or_default() }
            if thread.is_empty() { return json!({"path": "", "error": "no thread to dictate into: open one, or set up Bub"}) }
            let watch = { let mut g = self.st.lock().unwrap(); let a = g.always.get_or_insert_with(|| Always { client, params: Value::Null, paused: String::new(), off: false, focused: true, watching: false });
                a.client = client; a.params = json!({"thread": thread, "always": true, "src": s(p, "src")}); a.off = false; !std::mem::replace(&mut a.watching, true) };
            if watch { let (v, h) = (self.clone(), hub.clone()); let w: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> = Box::pin(async move { v.watch(h).await }); tokio::spawn(w); }
            if !self.st.lock().unwrap().always.as_ref().map_or(true, |a| a.paused.is_empty()) { self.tell(hub, client, &thread); return json!({"path": ""}) }
        }
        let draft = s(p, "draft"); let caret = p["caret"].as_u64().map_or(words::len(draft), |c| c as usize);
        self.with(&thread, |d| d.err.clear());
        if self.st.lock().unwrap().live.is_some() { return self.fail(hub, client, &thread, "already listening: stop the other dictation first") }
        if crate::copy::on() { return self.fail(hub, client, &thread, COPY_NO) }
        let pr = prefs(hub).await; let model = pr.get("voice.model").cloned().unwrap_or_default();
        let engine = engine_of(pr.get("voice.engine").map_or("auto", String::as_str), ready(&home, &model));
        if engine == "openai" && text_of(&key_path(&home)).trim().is_empty() { return self.fail(hub, client, &thread, SET_UP) }
        let Some(bin) = exe() else { return self.fail(hub, client, &thread, NO_HELPER) };
        let d = dict_of(&pr, &home); write_keywords(&home, &d);
        let device = if upload { String::new() } else { pr.get("voice.device").cloned().unwrap_or_default() };
        // the resident recognizer, when it is up: the session starts with the model already loaded
        let (server, hold) = if engine == "local" { resident::of(&home).and_then(|r| r.server(&home, &model)).map_or((String::new(), None), |(p, h)| (p, Some(h))) } else { (String::new(), None) };
        let mut cmd = proc::tokio_cmd(if engine == "local" { Kind::Local } else { Kind::Net }, &bin);
        let mut args = live_args(&home, engine, &model, upload, &device, &server); if always { args.push("--always".into()) }
        cmd.args(args).current_dir("/").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
        let Ok(mut child) = cmd.spawn() else { return self.fail(hub, client, &thread, NO_HELPER) };
        let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else { return self.fail(hub, client, &thread, NO_HELPER) };
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(64);
        tokio::spawn(async move { while let Some(b) = rx.recv().await { if stdin.write_all(&b).await.is_err() { break } } });
        let key = if upload { hex::encode(pairing::urandom(16)) } else { String::new() };
        let (ctl_tx, ctl_rx) = mpsc::unbounded_channel();
        let id = { let mut g = self.st.lock().unwrap(); g.next += 1; let id = g.next; g.live = Some(Live { always, id, client, thread: thread.clone(), key: key.clone(), upload, audio: Some(tx), stopped: None, ctl: ctl_tx.clone() }); id };
        self.with(&thread, |x| { x.begin(); x.engine = engine.to_string(); x.always = always; });
        self.tell(hub, client, &thread);
        if upload { hub.push(client, frame(&thread, "audio", json!({"path": format!("/voice/in?s={key}")}))); }
        let reader = Reader::new(d, engine == "local", draft, caret).with_cmds(super::cmds::Cmds::of(&pr)).with_eot(super::eot::Cfg::of(&pr));
        let reader = if always { reader.with_gate(super::always::Cfg::of(&pr).gate) } else { reader };
        { let (v, hub, thread, engine) = (self.clone(), hub.clone(), thread.clone(), engine.to_string()); tokio::spawn(async move { let _hold = hold; v.read(hub, client, id, thread, engine, reader, child, stdout, ctl_rx, ctl_tx).await }); }
        json!({"path": if upload { format!("/voice/in?s={key}") } else { String::new() }})
    }
    async fn read(self: Arc<Self>, hub: Arc<Hub>, client: u64, id: u64, mut thread: String, engine: String, mut r: Reader, mut child: tokio::process::Child, stdout: tokio::process::ChildStdout, mut ctl: mpsc::UnboundedReceiver<Ctl>, me: mpsc::UnboundedSender<Ctl>) {
        let mut lines = BufReader::new(stdout).lines(); let mut tick = tokio::time::interval(Duration::from_secs(1));
        // the end-of-turn countdown: when it ends, and the pause it is about
        let mut due: Option<(tokio::time::Instant, u64)> = None;
        loop {
            let at = due.map(|d| d.0);
            tokio::select! {
                l = lines.next_line() => { let Ok(Some(l)) = l else { break };
                    let outs = self.with(&thread, |d| r.step(&l, d, &engine));
                    // always on: the message was said to a name first ("Bub, …"): it goes there
                    if let Some(name) = outs.iter().find_map(|o| if let Out::Route(n) = o { Some(n.clone()) } else { None }) {
                        if let Some(t) = route_to(&hub, &name).await.filter(|t| *t != thread) { self.retarget(&hub, client, id, &mut thread, &t); }
                    }
                    self.out(&hub, client, &thread, &r, outs, &me, &mut due).await;
                }
                Some(c) = ctl.recv() => {
                    let outs = match c { Ctl::Cut(n, seq) => self.with(&thread, |d| r.cut(n, seq, d)), Ctl::Hold => r.eot_hold(), Ctl::Jev(gen, p) => r.eot_answer(gen, p),
                        Ctl::Target(t) => { if !r.started() && !t.is_empty() && t != thread { self.retarget(&hub, client, id, &mut thread, &t); } vec![] } };
                    self.out(&hub, client, &thread, &r, outs, &me, &mut due).await;
                }
                _ = async { match at { Some(t) => tokio::time::sleep_until(t).await, None => std::future::pending().await } } => {
                    let gen = due.take().map_or(0, |d| d.1); let outs = r.eot_fire(gen); self.out(&hub, client, &thread, &r, outs, &me, &mut due).await;
                }
                _ = tick.tick() => {
                    let outs = self.with(&thread, |_| r.idle(Instant::now())); self.out(&hub, client, &thread, &r, outs, &me, &mut due).await;
                    if !hub.is_attached(client) { self.close_audio(id) } // the client went away: nobody is listening to this microphone
                    let late = self.st.lock().unwrap().live.as_ref().filter(|l| l.id == id).and_then(|l| l.stopped).map_or(false, |t| t.elapsed() > STOP_LIMIT);
                    if late { let _ = child.start_kill(); }
                }
            }
        }
        let code = tokio::time::timeout(Duration::from_secs(5), child.wait()).await.ok().and_then(|r| r.ok()).and_then(|s| s.code());
        { let mut g = self.st.lock().unwrap(); if g.live.as_ref().map_or(false, |l| l.id == id) { g.live = None } }
        self.with(&thread, |d| { d.idle(); if code != Some(0) && d.err.is_empty() { d.err = "the voice helper stopped".into() } });
        self.tell(&hub, client, &thread);
        // always on: it listens again unless the person muted it, it is paused (the watcher starts it when that ends) or nobody is there
        let again = { let mut g = self.st.lock().unwrap(); match g.always.as_mut() { Some(a) if !a.off && a.paused.is_empty() && hub.is_attached(client) => { a.params["thread"] = json!(thread); Some(a.params.clone()) } _ => None } };
        if let Some(p) = again { self.with(&thread, |d| d.err.clear()); tokio::time::sleep(Duration::from_secs(1)).await; let _ = self.again(hub.clone(), client, p).await; }
    }
    /// Always on: start again (boxed: start, read and watch reach one another).
    fn again(self: &Arc<Self>, hub: Arc<Hub>, client: u64, p: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Value> + Send>> { let v = self.clone(); Box::pin(async move { v.start(&hub, client, &p).await }) }
    /// Always on: the message goes to another thread from here on (its strip shows there).
    fn retarget(&self, hub: &Hub, client: u64, id: u64, thread: &mut String, to: &str) {
        let from = std::mem::replace(thread, to.to_string());
        { let mut g = self.st.lock().unwrap(); if let Some(l) = g.live.as_mut().filter(|l| l.id == id) { l.thread = to.to_string() } if let Some(a) = g.always.as_mut() { a.params["thread"] = json!(to) } }
        let st = self.with(&from, |d| { let s = d.clone(); d.idle(); s }); self.with(to, |d| *d = st);
        self.tell(hub, client, &from); self.tell(hub, client, to);
    }
    /// Always on: pauses for a lock, a call or the background (always.rs `pause_reason`) and listens again after; every 2 s while it is on.
    async fn watch(self: Arc<Self>, hub: Arc<Hub>) {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let Some((client, focused, paused, off, params)) = ({ let g = self.st.lock().unwrap(); g.always.as_ref().map(|a| (a.client, a.focused, a.paused.clone(), a.off, a.params.clone())) }) else { break };
            if off { self.st.lock().unwrap().always.as_mut().map(|a| a.watching = false); break }
            let cfg = super::always::Cfg::of(&prefs(&hub).await);
            let (apps, locked) = tokio::task::spawn_blocking({ let ig = cfg.ignore.clone(); move || system_state(&ig) }).await.unwrap_or_default();
            let why = super::always::pause_reason(&cfg, &apps, locked, focused);
            let thread = s(&params, "thread").to_string();
            if why != paused {
                self.st.lock().unwrap().always.as_mut().map(|a| a.paused = why.clone());
                self.with(&thread, |d| d.paused = why.clone());
                if !why.is_empty() { let id = self.st.lock().unwrap().live.as_ref().filter(|l| l.always).map(|l| l.id); if let Some(id) = id { self.close_audio(id) } self.tell(&hub, client, &thread); }
                else if self.st.lock().unwrap().live.is_none() { let _ = self.again(hub.clone(), client, params).await; }
            }
        }
    }
    /// What a reader's step gives, to the client: each edit with the session's cut count and its number (`Reader::cut`); an end-of-turn question
    /// goes to Jev (its answer comes back on `me`), a countdown is timed in `due`, an end-of-turn send is counted for the undo log.
    async fn out(&self, hub: &Arc<Hub>, client: u64, thread: &str, r: &Reader, outs: Vec<Out>, me: &mpsc::UnboundedSender<Ctl>, due: &mut Option<(tokio::time::Instant, u64)>) {
        for o in outs { match o {
            Out::Ask(a) => {
                let cfg = super::eot::Cfg::of(&prefs(hub).await);
                if !cfg.jev { let _ = me.send(Ctl::Jev(a.gen, None)); continue }
                let (hub, me, th) = (hub.clone(), me.clone(), thread.rsplit('|').next().unwrap_or("").to_string());
                tokio::spawn(async move {
                    let last = { let m = hub.model().await; m.page(&hub.store, &th, 0, 32).ok() }.and_then(|p| p.rows.into_iter().filter(|r| r.kind == "msg" && r.role == "assistant" && r.thread == th).max_by_key(|r| r.seq)).map(|r| r.text).unwrap_or_default();
                    let st = super::eot::state(&a.draft, a.pause_ms, a.audio, &last);
                    // over the limit, or failing: the audio model decides alone (eot::decide)
                    let p = match tokio::time::timeout(Duration::from_millis(super::eot::JEV_LIMIT_MS), super::jev::ask(&hub.cfg.home, &cfg.model, &st, 2)).await {
                        Ok(Ok((p, ms))) => { hub.set_info("voice.eot.last", &json!({"by": "jev", "p": p, "ms": ms}).to_string()).await; Some(p) }
                        Ok(Err(e)) => { eprintln!("voice: Jev: {e}"); hub.set_info("voice.eot.last", &json!({"by": "audio", "error": e}).to_string()).await; None }
                        Err(_) => { hub.set_info("voice.eot.last", &json!({"by": "audio", "error": format!("over {} ms", super::eot::JEV_LIMIT_MS)}).to_string()).await; None }
                    };
                    let _ = me.send(Ctl::Jev(a.gen, p));
                });
            }
            Out::Countdown(ms, by) => {
                *due = (ms > 0).then(|| (tokio::time::Instant::now() + Duration::from_millis(ms), r.eot_counting().unwrap_or(0)));
                hub.push(client, frame(thread, "countdown", json!({"ms": ms, "by": by})));
            }
            Out::AutoSent => {
                self.st.lock().unwrap().auto.insert(thread.to_string(), Instant::now());
                let l = super::eot::Learned::parse(&prefs(hub).await.get(super::eot::LEARNED).cloned().unwrap_or_default()).after(false);
                let _ = commit(hub, &[(super::eot::LEARNED.to_string(), l.text())]).await;
            }
            Out::Edit(e) => { hub.push(client, frame(thread, "edit", json!({"del": e.del, "ins": e.ins, "adv": e.adv, "to": e.to, "back": e.back, "cut": r.cuts, "seq": r.seq}))); }
            // a voice command: the client sends (or steers) the draft it has now, or clears it, with its undo window; `cut` is the count it takes
            Out::Command(c) => { hub.push(client, frame(thread, "command", json!({"cmd": c, "cut": r.cuts}))); }
            Out::View => self.tell(hub, client, thread),
            Out::Route(_) => {} // the read loop moved the session to the named thread already
            Out::Level(v) => { hub.push(client, frame(thread, "level", json!({"v": v}))); }
        } }
    }
    fn stop(self: &Arc<Self>) {
        // the person stopped it: always-on is muted (it starts again only when turned on)
        let was = { let mut g = self.st.lock().unwrap(); g.always.as_mut().map(|a| { a.off = true; a.paused.clear(); s(&a.params, "thread").to_string() }) };
        if let Some(t) = was { self.with(&t, |d| { d.always = false; d.paused.clear(); }) }
        let (id, upload) = { let g = self.st.lock().unwrap(); let Some(l) = g.live.as_ref() else { return }; (l.id, l.upload) };
        if !upload { return self.close_audio(id) }
        let v = self.clone(); tokio::spawn(async move { tokio::time::sleep(UPLOAD_GRACE).await; v.close_audio(id) });
    }
    /// A piece of a client's audio into the helper's stdin; None when no session has this key.
    fn sink(&self, key: &str) -> Option<(u64, Option<mpsc::Sender<Vec<u8>>>)> { let g = self.st.lock().unwrap(); g.live.as_ref().filter(|l| !l.key.is_empty() && l.key == key).map(|l| (l.id, l.audio.clone())) }

    /// What the hub finds out for Settings, published as info for every client.
    async fn refresh(&self, hub: &Arc<Hub>) -> Value {
        let home = hub.cfg.home.clone(); let masked = words::mask(text_of(&key_path(&home)).trim()); hub.set_info("voice.key", &masked).await;
        hub.set_info("voice.eot.key", if super::jev::key().is_some() { "set" } else { "" }).await;
        let (mut devs, mut local) = (json!({"default": "", "devices": []}), json!({"error": "The voice helper is not installed beside Backplane."}));
        if let Some(bin) = exe() {
            let run = |args: Vec<String>| { let mut c = proc::tokio_cmd(Kind::Local, &bin); c.args(args).stdin(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true); async move { tokio::time::timeout(Duration::from_secs(10), c.output()).await.ok().and_then(|r| r.ok()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default() } };
            let event = |t: &str, ev: &str| t.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|o| o["event"] == ev);
            if let Some(mut o) = event(&run(vec!["devices".into()]).await, "devices") { o.as_object_mut().map(|m| m.remove("event")); devs = o }
            if let Some(mut o) = event(&run(vec!["local-status".into(), "--local-dir".into(), local_dir(&home).to_string_lossy().into_owned()]).await, "local") { o.as_object_mut().map(|m| m.remove("event")); local = o }
        }
        hub.set_info("voice.devices", &devs.to_string()).await; hub.set_info("voice.local", &local.to_string()).await;
        json!({"voiceKey": masked, "voiceDevices": devs.to_string(), "local": local})
    }
    /// Downloads or removes a model; its progress goes out as info, so Settings follows it wherever it is open.
    async fn job(self: Arc<Self>, hub: Arc<Hub>, client: u64, cmd: &'static str, model: String) {
        let busy = format!("{}:{model}", if cmd == "local-remove" { "remove" } else { "install" });
        let mut last = json!({"busy": busy}); hub.set_info("voice.job", &last.to_string()).await; self.page(&hub, client).await;
        let home = hub.cfg.home.clone(); botnet::mkdir(&local_dir(&home)); let mut err = String::new();
        let child = exe().ok_or("The voice helper could not start. Check that it is installed beside Backplane.").and_then(|bin| { let mut c = proc::tokio_cmd(Kind::Net, bin); c.args([cmd, "--local-dir", &local_dir(&home).to_string_lossy(), "--model", &model]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true); c.spawn().map_err(|_| "The voice helper could not start. Check that it is installed beside Backplane.") });
        match child {
            Err(e) => err = e.to_string(),
            Ok(mut c) => {
                let mut lines = BufReader::new(c.stdout.take().unwrap()).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    let Ok(mut o) = serde_json::from_str::<Value>(&l) else { continue };
                    match s(&o, "event") {
                        "progress" => { o["busy"] = json!(busy); last = o; hub.set_info("voice.job", &last.to_string()).await; self.page(&hub, client).await; }
                        "error" if err.is_empty() => err = s(&o, "error").to_string(),
                        _ => {}
                    }
                }
                let code = c.wait().await.ok().and_then(|s| s.code());
                if err.is_empty() && code != Some(0) { err = "The voice helper stopped before finishing. Try the download again.".into() }
            }
        }
        self.refresh(&hub).await; if let Some(r) = resident::of(&home) { r.kick() }
        hub.set_info("voice.job", &if err.is_empty() { String::new() } else { json!({"error": err}).to_string() }).await;
        self.st.lock().unwrap().job = false; self.page(&hub, client).await;
    }
    async fn page(&self, hub: &Arc<Hub>, client: u64) { let rows = page::rows(&prefs(hub).await, &hub.info_now(), "", ""); hub.push(client, frame("", "page", json!({"rows": rows}))); }
    async fn op_job(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, cmd: &'static str, model: &str) -> Result<(), String> {
        if crate::copy::on() { return Err("A hub on a copied log cannot install or remove voice models.".into()) }
        { let mut g = self.st.lock().unwrap(); if g.job { return Err("A model operation is already running.".into()) } g.job = true; }
        let (v, h, m) = (self.clone(), hub.clone(), vdict::model_id(model).to_string()); tokio::spawn(async move { v.job(h, client, cmd, m).await }); Ok(())
    }
    async fn act(&self, hub: &Arc<Hub>, client: u64, p: &Value) -> Result<(), String> {
        let thread = s(p, "thread"); let pr = prefs(hub).await;
        // a send while dictating: the words up to the client's last edit (`seq`) went with it; `cut` counts the client's sends this session
        if let (Some(n), true) = (p["cut"].as_u64(), s(p, "action") == "voice-clear") { let g = self.st.lock().unwrap(); if let Some(l) = g.live.as_ref().filter(|l| l.thread == thread) { let _ = l.ctl.send(Ctl::Cut(n, p["seq"].as_u64().unwrap_or(0))); } }
        match s(p, "action") {
            // always on: the window's focus (for "pause while in the background"), and the thread now open (the next message goes there)
            "voice-focus" => { if let Some(a) = self.st.lock().unwrap().always.as_mut() { a.focused = s(p, "value") != "0" } return Ok(()) }
            "voice-target" => { let g = self.st.lock().unwrap(); if let Some(l) = g.live.as_ref().filter(|l| l.always) { let _ = l.ctl.send(Ctl::Target(s(p, "value").to_string())); } return Ok(()) }
            // Keep talking, in an end-of-turn countdown
            "voice-hold" => { let g = self.st.lock().unwrap(); if let Some(l) = g.live.as_ref().filter(|l| l.thread == thread) { let _ = l.ctl.send(Ctl::Hold); } return Ok(()) }
            // Undo of a voice send: when it was an end-of-turn send a moment ago, the threshold learns to wait a little more (only here, never sent)
            "voice-undid" => {
                let recent = self.st.lock().unwrap().auto.remove(thread).map_or(false, |t| t.elapsed() < Duration::from_secs(15));
                if recent { let l = super::eot::Learned::parse(&pr.get(super::eot::LEARNED).cloned().unwrap_or_default()).after(true); commit(hub, &[(super::eot::LEARNED.to_string(), l.text())]).await?; }
                return Ok(())
            }
            _ => {}
        }
        let a = self.with(thread, |d| d.act(s(p, "action"), s(p, "value"), s(p, "draft"), &|k| pr.get(k).cloned().unwrap_or_default()));
        commit(hub, &a.settings).await?;
        if let Some(t) = a.draft { hub.push(client, frame(thread, "draft", json!({"text": t}))); }
        self.tell(hub, client, thread); Ok(())
    }
    /// A button of the Voice page (`voice-op`), or a line typed to the dictionary (`voice-term`), or a `setting`.
    pub async fn page_act(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, action: &str, value: &str) -> Result<(), String> {
        match action {
            "setting" => {
                let (k, v) = value.split_once('=').unwrap_or((value, "")); if !k.starts_with("voice.") { return Err("not a voice setting".into()) } commit(hub, &[(k.to_string(), v.to_string())]).await?; if let Some(r) = resident::of(&hub.cfg.home) { r.kick() }
                // always on turned on: this client starts it in its open thread; off: it stops
                if k == super::always::ON { if v == "on" { hub.push(client, frame("", "always", json!({"want": true}))); } else { self.stop(); self.st.lock().unwrap().always = None; hub.push(client, frame("", "always", json!({"want": false}))); } }
            }
            "voice-term" => commit(hub, &page::term_settings(value)).await?,
            "voice-phrases" => commit(hub, &[page::phrases_setting(value)]).await?,
            "voice-always-ignore" => commit(hub, &[(super::always::IGNORE.to_string(), value.split(',').map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", "))]).await?,
            "voice-jev-model" => commit(hub, &[(super::eot::MODEL.to_string(), value.trim().to_string())]).await?,
            "voice-op" => { self.op(hub, client, value).await?; }
            _ => return Err("unknown voice action".into()),
        }
        self.page(hub, client).await; Ok(())
    }
    async fn op(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, value: &str) -> Result<(), String> {
        let (o, m) = value.split_once(':').unwrap_or((value, ""));
        match o {
            "status" => { self.refresh(hub).await; Ok(()) }
            "install" => self.op_job(hub, client, "local-install", m).await,
            "remove" => self.op_job(hub, client, "local-remove", m).await,
            "jev-test" => {
                let model = super::eot::Cfg::of(&prefs(hub).await).model; hub.set_info("voice.eot.test", "").await;
                let v = match super::jev::test(&hub.cfg.home, &model).await { Ok(r) => json!({"results": r.iter().map(|(d, p, ms)| json!({"draft": d, "p": p, "ms": ms})).collect::<Vec<_>>()}), Err(e) => json!({"error": e}) };
                hub.set_info("voice.eot.test", &v.to_string()).await; Ok(())
            }
            "unkey" => { let _ = std::fs::remove_file(key_path(&hub.cfg.home)); self.refresh(hub).await; Ok(()) }
            _ => Err("unknown voice request".into()),
        }
    }
}
/// Settings from a chip, in one batch (a blank value removes an entry).
pub async fn commit(hub: &Hub, kv: &[(String, String)]) -> Result<(), String> {
    if kv.is_empty() { return Ok(()) }
    let at = now_ms() / 1000; let batch: Vec<Value> = kv.iter().map(|(k, v)| json!({"$": "SettingSet", "key": k, "value": v, "at": at})).collect();
    hub.commit_many(&batch).await.map(|_| ()).map_err(|e| format!("{e:#}"))
}
/// A term for the dictionary, from a bot's `voice_dictionary_add`.
pub async fn add_term(hub: &Hub, term: &str) -> Result<(), String> { if !vdict::line_ok(term) { return Err("a term to add".into()) } commit(hub, &[super::dictation::entry_kv(term)]).await }
fn set_key(home: &Path, key: &str) -> Result<(), String> {
    let k = key.trim(); if !words::openai_ok(k) { return Err("that is not an OpenAI API key (they start with sk-)".into()) }
    botnet::mkdir(&home.join("secrets")); if botnet::write_secret(&key_path(home), &format!("{k}\n")) { Ok(()) } else { Err("the key could not be written".into()) }
}

pub struct VoiceRpc(pub Arc<Voice>);
impl RpcHandler for VoiceRpc {
    fn methods(&self) -> &'static [&'static str] { &["bots.voice"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let ok = |mut b: Value| { b["t"] = json!("reply"); b["id"] = id.clone(); b["ok"] = json!(true); b };
            let no = |e: String| json!({"t":"reply","id":id,"ok":false,"error":e});
            let done = |r: Result<(), String>| r.map_or_else(&no, |()| ok(json!({})));
            let home = &hub.cfg.home; let v = &self.0;
            vec![match s(p, "op") {
                "api-add" if s(p, "name") == page::KEY_NAME => match set_key(home, s(p, "value")) { Ok(()) => { let b = v.refresh(hub).await; ok(b) } Err(e) => no(e) },
                "api-add" | "api-del" => crate::settings::keys_rpc::answer(home, id, p),
                "status" => { let mut b = v.refresh(hub).await; b["apiKeys"] = json!(crate::settings::keys::names_kept(home)); ok(b) }
                "key" => match set_key(home, s(p, "key")) { Ok(()) => { let b = v.refresh(hub).await; ok(b) } Err(e) => no(e) },
                "unkey" => done(v.op(hub, who.client, "unkey").await),
                "add" => done(add_term(hub, s(p, "term")).await),
                "start" => { let b = v.start(hub, who.client, p).await; ok(b) }
                "stop" => { v.stop(); ok(json!({})) }
                "view" => ok(json!({"view": v.view(s(p, "thread"))})),
                // a client starting up: whether always-on is wanted (then it starts it), and whether it is already running
                "always" => { let want = super::always::Cfg::of(&prefs(hub).await).on; let running = v.st.lock().unwrap().always.as_ref().map_or(false, |a| !a.off);
                    hub.push(who.client, frame("", "always", json!({"want": want, "running": running}))); ok(json!({"want": want, "running": running})) }
                "act" => done(v.act(hub, who.client, p).await),
                "page" => { let rows = page::rows(&prefs(hub).await, &hub.info_now(), "", ""); ok(json!({"rows": rows})) }
                "page-act" => done(v.page_act(hub, who.client, s(p, "action"), s(p, "value")).await),
                "install" => done(v.op_job(hub, who.client, "local-install", s(p, "model")).await),
                "remove" => done(v.op_job(hub, who.client, "local-remove", s(p, "model")).await),
                _ => no("unknown voice request".into()),
            }]
        })
    }
}

/// `POST /voice/in?s=<key>[&end=1]`: a client's audio, 16 kHz mono s16le, in order, into the helper of the session it started. The body goes straight to the helper's stdin.
pub struct VoiceIn(pub Arc<Voice>);
impl HttpHandler for VoiceIn {
    fn routes(&self) -> &'static [&'static str] { &["/voice/in"] }
    fn takes_body(&self) -> bool { true }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _req: &'a Req) -> Fut<'a, HttpReply> { Box::pin(async { HttpReply::text("405 Method Not Allowed", "POST only") }) }
    fn call_body<'a>(&'a self, _hub: &'a Arc<Hub>, req: &'a Req, mut body: Body<'a>) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            if req.method != "POST" { return HttpReply::text("405 Method Not Allowed", "POST only") }
            if body.len > PIECE_MAX { return HttpReply::text("413 Payload Too Large", "too big") }
            let Some((id, tx)) = self.0.sink(&req.param("s")) else { return HttpReply::text("404 Not Found", "no such session") };
            if body.proceed().await.is_err() { return HttpReply::text("400 Bad Request", "gone") }
            let mut buf = vec![0u8; 16 << 10];
            loop {
                match body.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => if let Some(t) = &tx { if t.send(buf[..n].to_vec()).await.is_err() { break } },
                    Err(_) => return HttpReply::text("400 Bad Request", "upload stopped"),
                }
            }
            drop(tx); if req.param("end") == "1" { self.0.close_audio(id) }
            HttpReply::text("200 OK", "ok")
        })
    }
}

/// The engine and model a hearing uses: `want` "" follows Settings, Voice; "local" the model chosen there, "local:<model>" that model, "openai" GPT-Live-Transcribe.
pub fn choose(p: &Prefs, home: &Path, want: &str) -> (&'static str, String) {
    let set = p.get("voice.model").cloned().unwrap_or_default();
    match want.split_once(':').map_or((want, ""), |(a, b)| (a, b)) {
        ("local", m) => ("local", vdict::model_id(if m.is_empty() { &set } else { m }).to_string()),
        ("openai", _) | ("cloud", _) => ("openai", String::new()),
        _ => (engine_of(p.get("voice.engine").map_or("auto", String::as_str), ready(home, &set)), vdict::model_id(&set).to_string()),
    }
}
/// A recording's words, corrected with the dictionary, what to ask about, the engine that heard them, and (from the local engine) each segment with
/// when it was said and how sure the engine was. `path` holds the audio, or a whole multipart form whose content type `ct` names. Err says why nothing was heard.
pub async fn hear_file(hub: &Hub, path: &Path, ct: &str, want: &str) -> Result<super::ringbox::Heard, String> {
    let home = hub.cfg.home.clone();
    let pr = prefs(hub).await; let (engine, model) = choose(&pr, &home, want);
    if engine == "openai" && text_of(&key_path(&home)).trim().is_empty() { return Err("no OpenAI key in Settings, Voice".into()) }
    if engine == "local" && !ready(&home, &model) { return Err(format!("the local model {model} is not downloaded (Settings, Voice)")) }
    let Some(bin) = exe() else { return Err(NO_HELPER.into()) };
    let d = dict_of(&pr, &home); write_keywords(&home, &d);
    let p = |x: PathBuf| x.to_string_lossy().into_owned();
    let mut args = vec!["file".to_string(), p(path.to_path_buf()), "--content-type".to_string(), ct.to_string()];
    let hold = if engine == "local" { resident::of(&home).and_then(|r| r.server(&home, &model)) } else { None };
    if engine == "local" { args.extend(["--engine".to_string(), "local".to_string(), "--local-dir".to_string(), p(local_dir(&home)), "--model".to_string(), model.clone()]) }
    else { args.extend(["--key-file".to_string(), p(key_path(&home)), "--keywords-file".to_string(), p(keywords_path(&home))]) }
    if let Some((sock, _)) = &hold { args.extend(["--server".to_string(), sock.clone()]) }
    let mut c = proc::tokio_cmd(if engine == "local" { Kind::Local } else { Kind::Net }, &bin); c.args(args).current_dir("/").stdin(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
    let o = match tokio::time::timeout(Duration::from_secs(300), c.output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(format!("{} did not run: {e}", bin.display())),
        Err(_) => return Err("the voice helper took over 5 minutes".into()),
    };
    drop(hold); let out = String::from_utf8_lossy(&o.stdout);
    let evs: Vec<Value> = out.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect();
    let Some(done) = evs.iter().find(|o| o["event"] == "done") else { return Err(evs.iter().find(|o| o["event"] == "error").map_or("the voice helper stopped without words".to_string(), |e| s(e, "error").to_string())) };
    let heard = s(done, "text").trim().to_string();
    // a ring's note is its own message: a send phrase said at its end out of habit is not part of it
    let heard = super::cmds::Cmds::of(&pr).strip_send(&heard);
    let fx = d.fix(&vdict::words_text(&heard)); let text = vdict::text(&fx.words); let none: [&str; 0] = [];
    let note = words::note(&d.asks(&none, &vdict::VFix { words: vdict::words_text(&text), fixes: vec![], ask: fx.ask }));
    let segs: Vec<(i64, i64, i64, String)> = done["segments"].as_array().map(|a| a.iter().filter_map(|x| Some((x[0].as_i64()?, x[1].as_i64()?, x[2].as_i64()?, x[3].as_str()?.to_string()))).collect()).unwrap_or_default();
    let conf = segs.iter().map(|x| x.2).min();
    Ok(super::ringbox::Heard { model: if engine == "local" { model } else { "gpt-live-transcribe".to_string() }, text, unsure: note, conf, segs, at: now_ms() / 1000 })
}

/// Bub's chat (the uberbot's), where always-on dictation goes when no thread is open.
async fn bub_thread(hub: &Arc<Hub>) -> Option<String> { let b = hub.setting("uber.bot").await.filter(|b| !b.is_empty())?; crate::bots::api::chat_thread(hub, &b).await }
/// The thread a spoken name means: Bub, or a thread's title (always.rs `resolve`).
async fn route_to(hub: &Arc<Hub>, name: &str) -> Option<String> {
    let mut ts: Vec<(String, String)> = { let m = hub.model().await; m.threads.iter().filter(|(_, t)| !t.deleted && !t.archived && !t.title.trim().is_empty()).map(|(id, t)| (id.clone(), t.title.clone())).collect() };
    if let Some(b) = bub_thread(hub).await { ts.retain(|(id, _)| *id != b); ts.push((b, "Bub".into())) }
    super::always::resolve(name, &ts).map(str::to_string)
}
/// Who else has the microphone (pw-dump, else pactl), and whether the screen is locked (loginctl): what always-on pauses for.
fn system_state(ignore: &[String]) -> (Vec<String>, bool) {
    let run = |c: &str, a: &[&str]| crate::proc::std_cmd(Kind::Local, c).args(a).stderr(Stdio::null()).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    let apps = run("pw-dump", &[]).or_else(|| run("pactl", &["-f", "json", "list", "source-outputs"])).map_or(vec![], |j| super::always::capturing(&j, ignore));
    let ids: Vec<String> = run("loginctl", &["list-sessions", "--no-legend"]).unwrap_or_default().lines().filter_map(|l| l.split_whitespace().next().map(String::from)).collect();
    let shows: Vec<String> = ids.iter().filter_map(|id| run("loginctl", &["show-session", id, "-p", "LockedHint", "-p", "Type"])).collect();
    (apps, super::always::locked(&shows))
}

/// Registers `bots.voice` and `/voice/in`, and starts the resident recognizer's keeper (before `settings::start`, which then leaves `bots.voice` alone).
pub fn start(hub: &Arc<Hub>) -> Arc<Voice> { resident::start(hub); let v = Arc::new(Voice::default()); hub.register_rpc(Arc::new(VoiceRpc(v.clone()))); hub.register_http(Arc::new(VoiceIn(v.clone()))); v }

#[cfg(test)]
mod tests;

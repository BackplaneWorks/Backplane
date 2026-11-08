//! Dictation's IO on the hub: `bots.voice`, `/voice/in`, the OpenAI key, the helper. The helper (`backplane-voice`, native/voice) is a child process: it holds the microphone,
//! the engines and the downloads, so nothing of them loads in the hub and a crash there ends one session. One live session at a time; its messages go to the client that
//! started it: `{"t":"voice","thread","ev":"view","view"}` (what to draw), `"level"`, `"edit"` (place::Edit), `"draft"` (a chip changed the words already there).
//!
//! The key is a secret (`<home>/secrets/openai.key`, 0600): the helper reads the file, so it is never in an argv, an environment, the log or a message.
use super::{dictation::{Dictation, Out, Reader}, page::{self, Prefs}, vdict::{self, VDict}, words};
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
fn ready(home: &Path, model: &str) -> bool { text_of(&local_dir(home).join(format!("ready-{}", vdict::model_id(model)))).trim() == "ok" }
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
fn live_args(home: &Path, engine: &str, model: &str, upload: bool, device: &str) -> Vec<String> {
    let p = |x: PathBuf| x.to_string_lossy().into_owned();
    let mut a: Vec<String> = if engine == "local" { vec!["live".into(), "--engine".into(), "local".into(), "--local-dir".into(), p(local_dir(home)), "--model".into(), vdict::model_id(model).into()] }
        else { vec!["live".into(), "--key-file".into(), p(key_path(home)), "--keywords-file".into(), p(keywords_path(home))] };
    if upload { a.extend(["--source", "stdin", "--rate", "16000"].map(String::from)) } else if !device.is_empty() { a.extend(["--device".to_string(), device.to_string()]) }
    a
}
fn frame(thread: &str, ev: &str, mut body: Value) -> Value { body["t"] = json!("voice"); body["thread"] = json!(thread); body["ev"] = json!(ev); body }

struct Live { id: u64, client: u64, thread: String, key: String, upload: bool, audio: Option<mpsc::Sender<Vec<u8>>>, stopped: Option<Instant> }
#[derive(Default)]
struct St { live: Option<Live>, dicts: HashMap<String, Dictation>, job: bool, next: u64 }
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
        let (home, thread, upload) = (hub.cfg.home.clone(), s(p, "thread").to_string(), s(p, "src") == "client");
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
        let mut cmd = proc::tokio_cmd(if engine == "local" { Kind::Local } else { Kind::Net }, &bin);
        cmd.args(live_args(&home, engine, &model, upload, &device)).current_dir("/").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
        let Ok(mut child) = cmd.spawn() else { return self.fail(hub, client, &thread, NO_HELPER) };
        let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else { return self.fail(hub, client, &thread, NO_HELPER) };
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(64);
        tokio::spawn(async move { while let Some(b) = rx.recv().await { if stdin.write_all(&b).await.is_err() { break } } });
        let key = if upload { hex::encode(pairing::urandom(16)) } else { String::new() };
        let id = { let mut g = self.st.lock().unwrap(); g.next += 1; let id = g.next; g.live = Some(Live { id, client, thread: thread.clone(), key: key.clone(), upload, audio: Some(tx), stopped: None }); id };
        self.with(&thread, |x| { x.begin(); x.engine = engine.to_string(); });
        self.tell(hub, client, &thread);
        if upload { hub.push(client, frame(&thread, "audio", json!({"path": format!("/voice/in?s={key}")}))); }
        let reader = Reader::new(d, engine == "local", draft, caret);
        { let (v, hub, thread, engine) = (self.clone(), hub.clone(), thread.clone(), engine.to_string()); tokio::spawn(async move { v.read(hub, client, id, thread, engine, reader, child, stdout).await }); }
        json!({"path": if upload { format!("/voice/in?s={key}") } else { String::new() }})
    }
    async fn read(self: Arc<Self>, hub: Arc<Hub>, client: u64, id: u64, thread: String, engine: String, mut r: Reader, mut child: tokio::process::Child, stdout: tokio::process::ChildStdout) {
        let mut lines = BufReader::new(stdout).lines(); let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                l = lines.next_line() => { let Ok(Some(l)) = l else { break };
                    let outs = self.with(&thread, |d| r.step(&l, d, &engine));
                    for o in outs { match o {
                        Out::Edit(e) => { hub.push(client, frame(&thread, "edit", json!({"del": e.del, "ins": e.ins, "adv": e.adv, "to": e.to}))); }
                        Out::View => self.tell(&hub, client, &thread),
                        Out::Level(v) => { hub.push(client, frame(&thread, "level", json!({"v": v}))); }
                    } }
                }
                _ = tick.tick() => {
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
    }
    fn stop(self: &Arc<Self>) {
        let (id, upload) = { let g = self.st.lock().unwrap(); let Some(l) = g.live.as_ref() else { return }; (l.id, l.upload) };
        if !upload { return self.close_audio(id) }
        let v = self.clone(); tokio::spawn(async move { tokio::time::sleep(UPLOAD_GRACE).await; v.close_audio(id) });
    }
    /// A piece of a client's audio into the helper's stdin; None when no session has this key.
    fn sink(&self, key: &str) -> Option<(u64, Option<mpsc::Sender<Vec<u8>>>)> { let g = self.st.lock().unwrap(); g.live.as_ref().filter(|l| !l.key.is_empty() && l.key == key).map(|l| (l.id, l.audio.clone())) }

    /// What the hub finds out for Settings, published as info for every client.
    async fn refresh(&self, hub: &Arc<Hub>) -> Value {
        let home = hub.cfg.home.clone(); let masked = words::mask(text_of(&key_path(&home)).trim()); hub.set_info("voice.key", &masked).await;
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
        self.refresh(&hub).await;
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
        let a = self.with(thread, |d| d.act(s(p, "action"), s(p, "value"), s(p, "draft"), &|k| pr.get(k).cloned().unwrap_or_default()));
        commit(hub, &a.settings).await?;
        if let Some(t) = a.draft { hub.push(client, frame(thread, "draft", json!({"text": t}))); }
        self.tell(hub, client, thread); Ok(())
    }
    /// A button of the Voice page (`voice-op`), or a line typed to the dictionary (`voice-term`), or a `setting`.
    pub async fn page_act(self: &Arc<Self>, hub: &Arc<Hub>, client: u64, action: &str, value: &str) -> Result<(), String> {
        match action {
            "setting" => { let (k, v) = value.split_once('=').unwrap_or((value, "")); if !k.starts_with("voice.") { return Err("not a voice setting".into()) } commit(hub, &[(k.to_string(), v.to_string())]).await?; }
            "voice-term" => commit(hub, &page::term_settings(value)).await?,
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

/// A recording's words, corrected with the dictionary, what to ask about, and the engine that heard them ("" words with no engine, no helper or no speech). `path` holds the audio,
/// or a whole multipart form whose content type `ct` names.
pub async fn hear_file(hub: &Hub, path: &Path, ct: &str) -> (String, String, String) {
    let (home, none) = (hub.cfg.home.clone(), (String::new(), String::new(), String::new()));
    let pr = prefs(hub).await; let model = pr.get("voice.model").cloned().unwrap_or_default();
    let engine = engine_of(pr.get("voice.engine").map_or("auto", String::as_str), ready(&home, &model));
    if engine == "openai" && text_of(&key_path(&home)).trim().is_empty() { return none }
    let Some(bin) = exe() else { return none };
    let d = dict_of(&pr, &home); write_keywords(&home, &d);
    let p = |x: PathBuf| x.to_string_lossy().into_owned();
    let mut args = vec!["file".to_string(), p(path.to_path_buf()), "--content-type".to_string(), ct.to_string()];
    if engine == "local" { args.extend(["--engine".to_string(), "local".to_string(), "--local-dir".to_string(), p(local_dir(&home)), "--model".to_string(), vdict::model_id(&model).to_string()]) }
    else { args.extend(["--key-file".to_string(), p(key_path(&home)), "--keywords-file".to_string(), p(keywords_path(&home))]) }
    let mut c = proc::tokio_cmd(if engine == "local" { Kind::Local } else { Kind::Net }, &bin); c.args(args).current_dir("/").stdin(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
    let o = match tokio::time::timeout(Duration::from_secs(100), c.output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => { eprintln!("voice: a recording was not heard: {} did not run: {e}", bin.display()); return none }
        Err(_) => { eprintln!("voice: a recording was not heard: the helper took over 100 s"); return none }
    };
    let out = String::from_utf8_lossy(&o.stdout);
    let heard = out.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|o| o["event"] == "done").map_or(String::new(), |o| s(&o, "text").trim().to_string());
    let fx = d.fix(&vdict::words_text(&heard)); let text = vdict::text(&fx.words); let none: [&str; 0] = [];
    let note = words::note(&d.asks(&none, &vdict::VFix { words: vdict::words_text(&text), fixes: vec![], ask: fx.ask }));
    (text, note, if engine == "local" { vdict::model_id(&model).to_string() } else { "gpt-live-transcribe".to_string() })
}
/// A ring's form with its recording (`raw`, the body's bytes): heard here with the dictionary, then deleted; the form comes back with what was heard first. With no key or no words, as it came.
pub async fn ring(hub: &Hub, nonce: &str, ct: &str, body: &str, raw: &[u8]) -> String {
    let path = dir(&hub.cfg.home).join(format!("ring-{nonce}.form")); botnet::mkdir(&dir(&hub.cfg.home));
    if !botnet::write_secret_bytes(&path, raw) { return body.to_string() }
    let (heard, note, model) = hear_file(hub, &path, ct).await; let _ = std::fs::remove_file(&path);
    super::ring::heard_form(body, &heard, &note, &model)
}

/// Registers `bots.voice` and `/voice/in` (before `settings::start`, which then leaves `bots.voice` alone).
pub fn start(hub: &Arc<Hub>) -> Arc<Voice> { let v = Arc::new(Voice::default()); hub.register_rpc(Arc::new(VoiceRpc(v.clone()))); hub.register_http(Arc::new(VoiceIn(v.clone()))); v }

#[cfg(test)]
mod tests;

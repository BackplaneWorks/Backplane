//! The term lane: ptys and the terminals clients show (`term.open/input/resize/snap/restart`), terminals in the chat (`terminal_*` tools,
//! emb.rs), the VT emulator (vt.rs). One actor owns every shell and screen (src/server/term.bend's `Term.*`/`Et.*`): what a program prints
//! goes to every client as `{"t":"term","thread":<key>,"pid":N,"b":[bytes]}`, input comes in as `term.input`.
//! Bounded throughout: at most MAX_SHELLS ptys (MAX_PER_THREAD chat terminals a thread), output reads in 64 KB chunks through a 64-chunk
//! queue (the program waits when we are slow), input past 256 queued writes is dropped, one input request at most MAX_INPUT bytes,
//! screens are the emulator's bounded grid and 2048-row scrollback.
pub mod browser;
pub mod emb;
pub mod pty;
pub mod tools;
pub mod vt;

use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::{Arc, OnceLock}};
use tokio::sync::Mutex;
use vt::Vt;

pub const MAX_SHELLS: usize = 64;
pub const MAX_PER_THREAD: usize = 16;
pub const MAX_INPUT: usize = 262144;
/// Et.all: a frame for every client.
pub const ALL: u64 = u64::MAX;

struct Shell { pid: i32, pty: Arc<pty::Pty> }
struct Screen { title: String, vt: Vt, seen: bool, state: String, pid: i32 }
#[derive(Default)]
struct Inner { shells: HashMap<String, Shell>, screens: HashMap<String, Screen> }

pub struct Terms { hub: Arc<Hub>, inner: Mutex<Inner>, home: PathBuf, shell: String }

static TERMS: OnceLock<std::sync::Mutex<HashMap<PathBuf, Arc<Terms>>>> = OnceLock::new();
/// The terminals of the hub with this home (one hub per home).
pub fn get(hub: &Hub) -> Option<Arc<Terms>> { TERMS.get()?.lock().unwrap().get(&hub.cfg.home).cloned() }

/// The one line main.rs calls, after `Hub::start`.
pub fn start(hub: Arc<Hub>) {
    let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/sh".into());
    let home = hub.cfg.home.clone();
    let _ = std::fs::create_dir_all(home.join("terms"));
    let t = Arc::new(Terms { hub: hub.clone(), inner: Mutex::new(Inner::default()), home, shell });
    TERMS.get_or_init(Default::default).lock().unwrap().insert(hub.cfg.home.clone(), t.clone());
    hub.register_rpc(Arc::new(TermRpc(t)));
    browser::start(&hub);
}

pub fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn u(v: &Value, k: &str, d: u64) -> u64 { v.get(k).and_then(Value::as_u64).unwrap_or(d) }
fn frame(key: &str, pid: Option<i32>, b: &[u8], reset: Option<(usize, usize)>) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("t".into(), "term".into()); o.insert("thread".into(), key.into());
    if let Some(p) = pid { o.insert("pid".into(), p.into()); }
    o.insert("b".into(), Value::Array(b.iter().map(|&x| Value::from(x)).collect()));
    if let Some((c, r)) = reset { o.insert("reset".into(), json!([c, r])); }
    Value::Object(o)
}

impl Terms {
    fn send_to(&self, client: u64, f: Value) { if client == ALL { self.hub.push_all(&f) } else { self.hub.push(client, f); } }

    /// Where a thread's shell works: its worktree, else its project's folder (Hub.cwd).
    pub async fn cwd(&self, thread: &str) -> String {
        let m = self.hub.model().await;
        match m.threads.get(thread) { Some(t) if !t.worktree.is_empty() => t.worktree.clone(), Some(t) => m.projects.get(&t.project).map(|p| p.root.clone()).unwrap_or_default(), None => String::new() }
    }

    /// Start `cmd args` on a pty and the tasks that move its bytes. Err is the text for the screen.
    /// The shell and its (new) screen go in together, before the first byte is read.
    async fn launch(self: &Arc<Self>, key: &str, cmd: &str, args: &[String], cwd: &str, cols: u16, rows: u16, title: &str, seen: bool) -> Result<i32, String> {
        let sp = pty::Pty::spawn(cmd, args, cwd, cols, rows).map_err(|e| e.to_string())?;
        let pid = sp.pty.pid; let (mut out, exit) = (sp.output, sp.exit);
        { let mut g = self.inner.lock().await;
          g.shells.insert(key.into(), Shell { pid, pty: Arc::new(sp.pty) });
          g.screens.insert(key.into(), Screen { title: title.into(), vt: Vt::new(cols as usize, rows as usize), seen, state: "running".into(), pid }); }
        let (me, k) = (self.clone(), key.to_string());
        tokio::spawn(async move {
            while let Some(chunk) = out.recv().await { me.fed(&k, pid, &chunk).await }
            let code = exit.await.unwrap_or(255);
            me.fed(&k, pid, format!("\r\n[process exited {code}]\r\n").as_bytes()).await;
            me.gone(&k, pid, code).await;
        });
        Ok(pid)
    }

    /// TFed: a program printed. Output from a process a restart replaced is dropped before it can reach the replacement.
    async fn fed(&self, key: &str, pid: i32, bs: &[u8]) {
        let mut g = self.inner.lock().await;
        let live = g.shells.get(key).map_or(pid == 0, |s| s.pid == pid);
        if !live { return }
        let emb = emb::is_key(key);
        let replies = match g.screens.get_mut(key) {
            Some(sc) if sc.pid == pid => {
                sc.vt.feed(bs);
                let r = sc.vt.replies().to_vec(); sc.vt.drain();
                if emb { sc.seen = emb::stays(sc.seen, &sc.state); sc.state = emb::after(&sc.state, bs); r } else { vec![] }
            }
            _ => vec![],
        };
        if !replies.is_empty() { if let Some(sh) = g.shells.get(key) { sh.pty.write(replies); } }
        drop(g);
        self.hub.push_all(&frame(key, Some(pid), bs, None));
    }

    /// TGone: the program ended. A chat terminal keeps its last screen on disk.
    async fn gone(&self, key: &str, pid: i32, code: i32) {
        let mut g = self.inner.lock().await;
        if g.shells.get(key).map_or(false, |s| s.pid == pid) { g.shells.remove(key); }
        if emb::is_key(key) {
            if let Some(sc) = g.screens.get_mut(key) { if sc.pid == pid { sc.seen = emb::stays(sc.seen, &sc.state); sc.state = format!("exited {code}"); self.save(key, sc) } }
        }
    }

    fn save(&self, key: &str, sc: &Screen) {
        if !emb::key_ok(key) { return }
        let mut b = format!("{}x{}\t{}\n", sc.vt.cols(), sc.vt.rows(), sc.state).into_bytes(); b.extend(emb::snap(&sc.vt));
        let (path, tmp) = (self.home.join("terms").join(key), self.home.join("terms").join(format!("{key}.tmp")));
        if std::fs::write(&tmp, b).is_ok() { let _ = std::fs::rename(&tmp, &path); }
    }
    /// Et.load: the screen from its file when the actor does not hold it (the hub started again since it ran).
    fn load(&self, g: &mut Inner, key: &str) {
        if g.screens.contains_key(key) || !emb::key_ok(key) { return }
        let Ok(b) = std::fs::read(self.home.join("terms").join(key)) else { return };
        if b.len() > 4 << 20 { return }
        let nl = b.iter().position(|&x| x == 10).unwrap_or(b.len()); let head = String::from_utf8_lossy(&b[..nl]).into_owned(); let body = if nl < b.len() { &b[nl + 1..] } else { &[][..] };
        let mut it = head.split('\t'); let mut xy = it.next().unwrap_or("").split('x');
        let (c, r) = (emb::cols(emb::num(xy.next().unwrap_or(""))) as usize, emb::rows(emb::num(xy.next().unwrap_or(""))) as usize);
        let mut vt = Vt::new(c, r); vt.feed(body);
        g.screens.insert(key.into(), Screen { title: String::new(), vt, seen: true, state: it.next().unwrap_or("").to_string(), pid: 0 });
    }

    // ---- the requests (TOpen, TIn, TSize) ----
    pub async fn open_shell(self: &Arc<Self>, thread: &str, cols: u16, rows: u16) {
        let (cols, rows) = (cols.clamp(1, vt::MAX_COLS as u16), rows.clamp(1, vt::MAX_ROWS as u16));
        { let mut g = self.inner.lock().await;
          if let Some(sh) = g.shells.get(thread) { sh.pty.resize(cols, rows); if let Some(sc) = g.screens.get_mut(thread) { sc.vt.resize(cols as usize, rows as usize) } return }
          if g.shells.len() >= MAX_SHELLS { drop(g); self.hub.push_all(&frame(thread, Some(0), b"[too many terminals are open]\r\n", None)); return } }
        let cwd = self.cwd(thread).await;
        match self.launch(thread, &self.shell, &["-l".to_string()], &cwd, cols, rows, "", true).await {
            Ok(_) => {}
            Err(e) => { let m = format!("{} in {}: {}. Check SHELL and that the thread folder exists.", self.shell, cwd, e); self.hub.push_all(&frame(thread, Some(0), format!("[could not start a shell: {m}]\r\n").as_bytes(), None)); }
        }
    }
    pub async fn input(&self, key: &str, bs: Vec<u8>) -> bool { let g = self.inner.lock().await; g.shells.get(key).map_or(false, |sh| sh.pty.write(bs)) }
    pub async fn resize(&self, key: &str, cols: u16, rows: u16) {
        let (cols, rows) = (cols.clamp(1, vt::MAX_COLS as u16), rows.clamp(1, vt::MAX_ROWS as u16));
        let mut g = self.inner.lock().await;
        if let Some(sh) = g.shells.get(key) { sh.pty.resize(cols, rows) }
        if let Some(sc) = g.screens.get_mut(key) { sc.vt.resize(cols as usize, rows as usize) }
    }
    /// A chat terminal's command on a pty of its own (TRun); `again` hangs up the one running first.
    pub async fn run(self: &Arc<Self>, key: &str, title: &str, cwd: &str, cmd: &str, cols: u32, rows: u32, again: bool) {
        let (c, r) = (emb::cols(cols), emb::rows(rows));
        { let mut g = self.inner.lock().await; let has = g.shells.contains_key(key) || g.screens.contains_key(key);
            if has && !again { return }
            if let Some(sh) = g.shells.remove(key) { sh.pty.hangup() }
            let mine = format!("{}:", key.rsplitn(2, ':').nth(1).unwrap_or(""));
            if !has && g.shells.keys().filter(|k| k.starts_with(&mine)).count() >= MAX_PER_THREAD { drop(g); self.hub.push_all(&frame(key, Some(0), b"[too many terminals are open in this chat]\r\n", None)); return }
        }
        self.hub.push_all(&frame(key, None, &[], Some((c as usize, r as usize))));
        let args = vec!["-lc".to_string(), emb::script(cmd, &self.shell)];
        match self.launch(key, &self.shell, &args, cwd, c as u16, r as u16, title, false).await {
            Ok(_) => {}
            Err(e) => { self.inner.lock().await.screens.insert(key.into(), Screen { title: title.into(), vt: Vt::new(c as usize, r as usize), seen: false, state: "failed to start".into(), pid: 0 });
                self.hub.push_all(&frame(key, Some(0), format!("[could not start a shell: {e}]\r\n").as_bytes(), None)); }
        }
    }
    /// TSnap: bytes that draw the screen on a blank terminal, to one client (ALL: every client).
    pub async fn snap(&self, client: u64, key: &str) {
        let mut g = self.inner.lock().await; self.load(&mut g, key);
        let f = match g.screens.get(key) { Some(sc) => frame(key, None, &emb::snap(&sc.vt), Some((sc.vt.cols(), sc.vt.rows()))), None => frame(key, None, &[], Some((emb::cols(0) as usize, emb::rows(0) as usize))) };
        drop(g); self.send_to(client, f);
    }
    /// TRead: ("state", report) and the agent has now seen it; None: no such terminal.
    pub async fn read(&self, key: &str) -> Option<(String, String)> {
        let mut g = self.inner.lock().await; self.load(&mut g, key);
        let sc = g.screens.get_mut(key)?; sc.seen = true;
        Some((sc.state.clone(), emb::report(emb::id_of(key), &sc.title, &sc.state, &emb::text(&sc.vt))))
    }
    /// True when the actor holds this terminal's screen (or its file).
    pub async fn read_known(&self, key: &str) -> bool { let mut g = self.inner.lock().await; self.load(&mut g, key); g.screens.contains_key(key) }
    /// TNote: each of the thread's terminals the agent has not seen as they are now, marked seen.
    pub async fn note(&self, thread: &str) -> String {
        let pre = format!("x:{thread}:"); let mut g = self.inner.lock().await; let mut keys: Vec<&String> = g.screens.keys().filter(|k| k.starts_with(&pre)).collect(); keys.sort();
        let keys: Vec<String> = keys.into_iter().cloned().collect(); let mut rs = vec![];
        for k in keys { let sc = g.screens.get_mut(&k).unwrap(); if !sc.seen { rs.push(emb::report(emb::id_of(&k), &sc.title, &sc.state, &emb::text(&sc.vt))); sc.seen = true } }
        emb::note(&rs)
    }
    /// TKill: hang up the terminal's process group; running becomes stopped.
    pub async fn kill(&self, key: &str) {
        let mut g = self.inner.lock().await;
        if let Some(sh) = g.shells.get(key) { sh.pty.hangup() }
        if let Some(sc) = g.screens.get_mut(key) { if sc.state == "running" { sc.state = "stopped".into() } }
    }
    /// The chat terminals of a thread that are running now.
    pub async fn running_of(&self, thread: &str) -> Vec<String> { let pre = format!("x:{thread}:"); self.inner.lock().await.shells.keys().filter(|k| k.starts_with(&pre)).cloned().collect() }
    pub async fn shell_count(&self) -> usize { self.inner.lock().await.shells.len() }
}

// ---- reading the log for what the model does not index yet ----

/// The summary of chat terminal `id` of `thread` (its ActivityLogged), scanning the newest lines first, at most SCAN of them.
pub fn find_act(hub: &Arc<Hub>, thread: &str, id: &str) -> Option<(String, u64)> {
    const SCAN: usize = 200_000;
    let n = hub.store.len(); let low = n.saturating_sub(SCAN); let mut end = n;
    while end > low {
        let a = end.saturating_sub(2000).max(low);
        for line in hub.store.range(a, end).ok()?.iter().rev() {
            if !line.contains("\"ActivityLogged\"") || !line.contains(id) { continue }
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            if s(&v, "$") == "ActivityLogged" && s(&v, "thread") == thread && s(&v, "act") == id && s(&v, "tone") == emb::TONE { return Some((s(&v, "summary").to_string(), u(&v, "at", 0))) }
        }
        end = a;
    }
    None
}

struct TermRpc(Arc<Terms>);
impl RpcHandler for TermRpc {
    fn methods(&self) -> &'static [&'static str] { &["term.open", "term.input", "term.resize", "term.snap", "term.restart"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let ok = || vec![json!({"t":"reply","id":id,"ok":true})];
            let t = &self.0; let thread = s(p, "thread");
            match method {
                "term.open" => { t.open_shell(thread, u(p, "cols", 80).min(1000) as u16, u(p, "rows", 24).min(1000) as u16).await; ok() }
                "term.input" => {
                    let bs: Vec<u8> = p.get("b").and_then(Value::as_array).map(|a| a.iter().take(MAX_INPUT).map(|x| x.as_u64().unwrap_or(0) as u8).collect()).unwrap_or_default();
                    if !bs.is_empty() { t.input(thread, bs).await; } ok()
                }
                "term.resize" => { t.resize(thread, u(p, "cols", 80).min(1000) as u16, u(p, "rows", 24).min(1000) as u16).await; ok() }
                "term.snap" => { let r = ok(); t.snap(who.client, s(p, "key")).await; r }
                "term.restart" => {
                    let term = s(p, "id");
                    let Some((sum, _)) = find_act(hub, thread, term) else { return vec![json!({"t":"reply","id":id,"ok":false,"error":"no such terminal"})] };
                    let key = emb::key(thread, term); let now = now_s(); let sm = emb::parse_summary(&sum);
                    let _ = hub.commit(&json!({"$":"SettingSet","key":emb::at_key(&key),"value":now.to_string(),"at":now})).await;
                    let cwd = t.cwd(thread).await;
                    t.run(&key, &sm.title, &cwd, &sm.cmd, sm.cols, sm.rows, true).await; ok()
                }
                _ => vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown method"})],
            }
        })
    }
}

#[cfg(test)]
mod tests;

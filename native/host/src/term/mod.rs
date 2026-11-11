//! The term lane: ptys and the terminals clients show (`term.open/input/resize/snap/restart`), terminals in the chat (`terminal_*` tools,
//! emb.rs), the VT emulator (vt.rs). One actor owns every shell and screen: what a program prints
//! goes to every client as `{"t":"term","thread":<key>,"pid":N,"b":[bytes]}`, input comes in as `term.input`.
//! Bounded: at most MAX_SHELLS ptys in all (MAX_PER_THREAD chat terminals a thread), output reads in 64 KB chunks through a 64-chunk
//! queue (the program waits when we are slow), at most KEEP_FINISHED finished screens in memory (the rest are read back from <home>/terms),
//! output frames go out with `Hub::push_all_budgeted` (a client whose outbox is over half full is skipped, never dropped; it gets one snapshot when it has
//! room again, `catch_up`), input past 256 queued writes is dropped, one input request at most MAX_INPUT bytes,
//! screens are the emulator's bounded grid and 2048-row scrollback.
pub mod browser;
pub mod driver;
pub mod emb;
#[cfg(unix)] pub mod pty;
#[cfg(windows)] #[path = "pty_windows.rs"] pub mod pty;
pub mod task;
pub mod teach;
pub mod stream;
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
/// Finished chat terminals whose screens stay in memory (each up to 2048 rows of scrollback); older ones are on disk.
pub const KEEP_FINISHED: usize = 32;
const MAX_SKIPPED: usize = 4096;
/// Et.all: a frame for every client.
pub const ALL: u64 = u64::MAX;

struct Shell { pid: i32, pty: Arc<pty::Pty> }
/// One terminal's screen and its stream: `gen` names this run of it (a new process or a screen read back from disk is a new one), `off` is how many
/// bytes it printed in this run, `ring` the newest of them (at most RING), so a client that held up to an offset takes only what came after (stream.rs).
struct Screen { title: String, vt: Vt, seen: bool, state: String, pid: i32, gen: u64, off: u64, ring: std::collections::VecDeque<u8>, dirty: bool, cwd: String, at: u64 }
impl Screen {
    fn new(title: &str, vt: Vt, seen: bool, state: &str, pid: i32, cwd: &str) -> Screen {
        Screen { title: title.into(), vt, seen, state: state.into(), pid, gen: stream::gen(), off: 0, ring: Default::default(), dirty: false, cwd: cwd.into(), at: now_s() }
    }
    fn took(&mut self, bs: &[u8]) { self.off += bs.len() as u64; stream::keep(&mut self.ring, bs); self.dirty = true; }
}
#[derive(Default)]
struct Inner { shells: HashMap<String, Shell>, screens: HashMap<String, Screen>, finished: std::collections::VecDeque<String>,
    /// (client, key) whose output frames were skipped: the client's copy of that screen is stale until a snapshot goes out. Bounded by clients x terminals.
    skipped: std::collections::HashSet<(u64, String)> }

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
    // a skipped client gets its snapshot even when the terminal prints nothing more
    { let t = t.clone(); tokio::spawn(async move { let mut iv = tokio::time::interval(std::time::Duration::from_millis(500)); let mut n = 0u64;
    loop { iv.tick().await; t.catch_up().await; n += 1; if n % 4 == 0 { t.save_dirty().await } } }); }
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
/// A frame of the screen's stream: `g` its run, `o` the offset its bytes start at (a snapshot's `o` is the offset it shows the screen at).
fn sframe(key: &str, sc: &Screen, pid: Option<i32>, b: &[u8], reset: Option<(usize, usize)>, o: u64) -> Value {
    let mut f = frame(key, pid, b, reset); f["g"] = json!(sc.gen); f["o"] = json!(o); f
}
/// A terminal's state changed (running, done N, exited N, stopped): every client hears it, as a frame with no bytes.
fn state_frame(key: &str, sc: &Screen) -> Value { let mut f = frame(key, None, &[], None); f["state"] = json!(sc.state); f["g"] = json!(sc.gen); f["o"] = json!(sc.off); f }

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
          g.screens.insert(key.into(), Screen::new(title, Vt::new(cols as usize, rows as usize), seen, "running", pid, cwd)); }
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
        let (replies, f, moved) = match g.screens.get_mut(key) {
            Some(sc) if sc.pid == pid => {
                let o = sc.off; sc.vt.feed(bs); sc.took(bs);
                let r = sc.vt.replies().to_vec(); sc.vt.drain();
                let was = sc.state.clone();
                let r = if emb { sc.seen = emb::stays(sc.seen, &sc.state); sc.state = emb::after(&sc.state, bs); r } else { vec![] };
                (r, sframe(key, sc, Some(pid), bs, None, o), (sc.state != was).then(|| state_frame(key, sc)))
            }
            _ => (vec![], frame(key, Some(pid), bs, None), None),
        };
        if !replies.is_empty() { if let Some(sh) = g.shells.get(key) { sh.pty.write(replies); } }
        // droppable: a client over half its outbox is skipped and marked, not dropped
        let bytes = bs.len() * 4 + 96;
        for c in self.hub.clients() { if !self.hub.push_budgeted(c, f.clone(), bytes) && g.skipped.len() < MAX_SKIPPED { g.skipped.insert((c, key.to_string())); } }
        drop(g);
        if let Some(sf) = moved { self.hub.push_all(&sf) }
        self.catch_up().await;
    }
    /// Each skipped client gets one snapshot of the screen it missed as soon as its outbox takes one (a budgeted push: over half full it is
    /// refused and the mark stays); gone clients are forgotten.
    pub async fn catch_up(&self) {
        let mut g = self.inner.lock().await; if g.skipped.is_empty() { return }
        let live = self.hub.clients(); g.skipped.retain(|(c, _)| live.contains(c));
        let marks: Vec<(u64, String)> = g.skipped.iter().cloned().collect();
        for (c, key) in marks {
            let Some(sc) = g.screens.get(&key) else { g.skipped.remove(&(c, key)); continue };
            let snap = emb::snap(&sc.vt); let f = sframe(&key, sc, None, &snap, Some((sc.vt.cols(), sc.vt.rows())), sc.off);
            if self.hub.push_budgeted(c, f, snap.len() * 4 + 96) { g.skipped.remove(&(c, key)); }
        }
    }

    /// TGone: the program ended. A chat terminal keeps its last screen on disk.
    async fn gone(&self, key: &str, pid: i32, code: i32) {
        let mut g = self.inner.lock().await;
        if g.shells.get(key).map_or(false, |s| s.pid == pid) { g.shells.remove(key); }
        let mut sf = None;
        if let Some(sc) = g.screens.get_mut(key) { if sc.pid == pid {
            sc.seen = emb::stays(sc.seen, &sc.state);
            // a stopped one stays stopped: its end is the hang-up's
            if sc.state != "stopped" { sc.state = format!("exited {code}") }
            if emb::is_key(key) { self.save(key, sc); sc.dirty = false }
            sf = Some(state_frame(key, sc));
        } }
        Self::finish(&mut g, key);
        drop(g); if let Some(f) = sf { self.hub.push_all(&f) }
    }
    /// A finished terminal's screen stays in memory only among the newest KEEP_FINISHED (chat terminals are on disk; a shell's is not kept).
    fn finish(g: &mut Inner, key: &str) {
        g.finished.retain(|k| k != key); g.finished.push_back(key.into());
        while g.finished.len() > KEEP_FINISHED { if let Some(old) = g.finished.pop_front() { if !g.shells.contains_key(&old) { g.screens.remove(&old); } } }
    }

    fn save(&self, key: &str, sc: &Screen) {
        if !emb::key_ok(key) { return }
        let mut b = format!("{}x{}\t{}\n", sc.vt.cols(), sc.vt.rows(), sc.state).into_bytes(); b.extend(emb::snap(&sc.vt));
        let name = crate::plat::fs::file_name(key); let (path, tmp) = (self.home.join("terms").join(&*name), self.home.join("terms").join(format!("{name}.tmp")));
        if std::fs::write(&tmp, b).is_ok() { let _ = crate::plat::fs::rename(&tmp, &path); }
    }
    /// Running chat terminals' screens go to disk now and then (every 2 s at most, only when they printed), so a hub that stops while one runs
    /// still has its last screen; read back, it is a stopped one.
    pub async fn save_dirty(&self) {
        let mut g = self.inner.lock().await;
        let keys: Vec<String> = g.screens.iter().filter(|(k, sc)| sc.dirty && emb::is_key(k)).map(|(k, _)| k.clone()).collect();
        for k in keys { if let Some(sc) = g.screens.get_mut(&k) { self.save(&k, sc); sc.dirty = false } }
    }
    /// Et.load: the screen from its file when the actor does not hold it (the hub started again since it ran).
    fn load(&self, g: &mut Inner, key: &str) {
        if g.screens.contains_key(key) || !emb::key_ok(key) { return }
        let Ok(b) = std::fs::read(self.home.join("terms").join(&*crate::plat::fs::file_name(key))) else { return };
        if b.len() > 4 << 20 { return }
        let nl = b.iter().position(|&x| x == 10).unwrap_or(b.len()); let head = String::from_utf8_lossy(&b[..nl]).into_owned(); let body = if nl < b.len() { &b[nl + 1..] } else { &[][..] };
        let mut it = head.split('\t'); let mut xy = it.next().unwrap_or("").split('x');
        let (c, r) = (emb::cols(emb::num(xy.next().unwrap_or(""))) as usize, emb::rows(emb::num(xy.next().unwrap_or(""))) as usize);
        let mut vt = Vt::new(c, r); vt.feed(body);
        // a terminal that ran when the hub stopped did not end by itself: it was stopped
        let state = stream::read_back(it.next().unwrap_or(""));
        let mut sc = Screen::new("", vt, true, &state, 0, ""); sc.took(body); sc.dirty = false;
        g.screens.insert(key.into(), sc);
        Self::finish(g, key); // a screen read back is a finished one: it counts against the cap too
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
        // a new size is news for every client that shows it: the screen as it now is, at that size (the program redraws after)
        let mut f = None;
        if let Some(sc) = g.screens.get_mut(key) { if (sc.vt.cols(), sc.vt.rows()) != (cols as usize, rows as usize) {
            sc.vt.resize(cols as usize, rows as usize); f = Some(sframe(key, sc, None, &emb::snap(&sc.vt), Some((sc.vt.cols(), sc.vt.rows())), sc.off)) } }
        drop(g); if let Some(f) = f { self.hub.push_all(&f) }
    }
    /// A chat terminal's command on a pty of its own (TRun); `again` hangs up the one running first.
    pub async fn run(self: &Arc<Self>, key: &str, title: &str, cwd: &str, cmd: &str, cols: u32, rows: u32, again: bool) { self.run_with(crate::copy::on(), key, title, cwd, cmd, cols, rows, again).await }
    /// On a copied log (BACKPLANE_COPY=1) no chat terminal runs: the command in the log is the real one, in the real folder.
    /// The entry is still logged and the person's own panel shell (`open_shell`) still opens, as in .
    pub async fn run_with(self: &Arc<Self>, copy: bool, key: &str, title: &str, cwd: &str, cmd: &str, cols: u32, rows: u32, again: bool) {
        if copy { return }
        let (c, r) = (emb::cols(cols), emb::rows(rows));
        { let mut g = self.inner.lock().await; let running = g.shells.contains_key(key); let has = running || g.screens.contains_key(key);
            if has && !again { return }
            if let Some(sh) = g.shells.remove(key) { sh.pty.hangup() }
            let mine = format!("{}:", key.rsplitn(2, ':').nth(1).unwrap_or(""));
            if !running && g.shells.len() >= MAX_SHELLS { drop(g); self.hub.push_all(&frame(key, Some(0), b"[too many terminals are open]\r\n", None)); return }
            if !running && g.shells.keys().filter(|k| k.starts_with(&mine)).count() >= MAX_PER_THREAD { drop(g); self.hub.push_all(&frame(key, Some(0), b"[too many terminals are open in this chat]\r\n", None)); return }
        }
        self.hub.push_all(&frame(key, None, &[], Some((c as usize, r as usize))));
        let args = vec!["-lc".to_string(), emb::script(cmd, &self.shell)];
        match self.launch(key, &self.shell, &args, cwd, c as u16, r as u16, title, false).await {
            Ok(_) => {}
            Err(e) => { self.inner.lock().await.screens.insert(key.into(), Screen::new(title, Vt::new(c as usize, r as usize), false, "failed to start", 0, cwd));
                self.hub.push_all(&frame(key, Some(0), format!("[could not start a shell: {e}]\r\n").as_bytes(), None)); }
        }
    }
    /// TSnap: bytes that draw the screen on a blank terminal, to one client (ALL: every client).
    pub async fn snap(&self, client: u64, key: &str) {
        let mut g = self.inner.lock().await; self.load(&mut g, key);
        let f = match g.screens.get(key) { Some(sc) => { let mut f = sframe(key, sc, None, &emb::snap(&sc.vt), Some((sc.vt.cols(), sc.vt.rows())), sc.off); f["state"] = json!(sc.state); f }
            None => frame(key, None, &[], Some((emb::cols(0) as usize, emb::rows(0) as usize))) };
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
        if let Some(sh) = g.shells.get(key) { sh.pty.hangup(); let p = sh.pty.clone(); tokio::spawn(async move { tokio::time::sleep(std::time::Duration::from_secs(3)).await; p.force() }); }
        let mut sf = None;
        let live = g.shells.contains_key(key);
        if let Some(sc) = g.screens.get_mut(key) { if live || sc.state == "running" { if sc.state != "stopped" { sc.state = "stopped".into(); sf = Some(state_frame(key, sc)) } } }
        drop(g); if let Some(f) = sf { self.hub.push_all(&f) }
    }
    /// Stream.read: what a client that holds run `g` up to `from` needs to hold the screen as it is now: the bytes since, or a snapshot.
    /// None: no such terminal.
    pub async fn stream_read(&self, key: &str, g: u64, from: Option<u64>) -> Option<Value> {
        let mut gd = self.inner.lock().await; self.load(&mut gd, key);
        let sc = gd.screens.get(key)?; let running = gd.shells.contains_key(key);
        let mut o = json!({"key":key,"g":sc.gen,"o":sc.off,"state":sc.state,"cols":sc.vt.cols(),"rows":sc.vt.rows(),"title":view_title(sc),"running":running});
        match stream::since(sc.gen, sc.off, &sc.ring, g, from) {
            Some(b) => { o["from"] = json!(from.unwrap_or(0)); o["b"] = json!(b64(&b)); }
            None => { o["reset"] = json!([sc.vt.cols(), sc.vt.rows()]); o["b"] = json!(b64(&emb::snap(&sc.vt))); }
        }
        Some(o)
    }
    /// Every terminal the hub holds now (running ones and the finished screens it keeps), newest first; `pre` keeps the keys that start with it.
    pub async fn list(&self, pre: &str) -> Vec<Value> {
        let g = self.inner.lock().await;
        let mut v: Vec<(u64, Value)> = g.screens.iter().filter(|(k, _)| k.starts_with(pre)).map(|(k, sc)| (sc.at, json!({"key":k,"kind":kind_of(k),"thread":thread_of(k),"title":view_title(sc),
            "state":sc.state,"running":g.shells.contains_key(k),"cols":sc.vt.cols(),"rows":sc.vt.rows(),"g":sc.gen,"o":sc.off,"cwd":sc.cwd,"at":sc.at * 1000}))).collect();
        v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1["key"].as_str().cmp(&b.1["key"].as_str()))); v.into_iter().map(|x| x.1).collect()
    }
    /// A new shell of its own for a client (a phone's Terminals list): key `s:<project>:<n>` (`s::<n>` for the hub's home folder), in `cwd`.
    pub async fn new_shell(self: &Arc<Self>, project: &str, cwd: &str, cols: u16, rows: u16) -> Result<String, String> {
        let (cols, rows) = (cols.clamp(1, vt::MAX_COLS as u16), rows.clamp(1, vt::MAX_ROWS as u16));
        if self.inner.lock().await.shells.len() >= MAX_SHELLS { return Err("too many terminals are open".into()) }
        let key = format!("s:{project}:{}", stream::gen());
        let title = std::path::Path::new(cwd).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "shell".into());
        self.launch(&key, &self.shell, &["-l".to_string()], cwd, cols, rows, &title, true).await.map_err(|e| format!("could not start {} in {}: {}", self.shell, cwd, e))?;
        Ok(key)
    }
    pub async fn known(&self, key: &str) -> bool { self.read_known(key).await }
    /// A program of its own on a pty under `key` (a sign-in another machine asked for, settings/accounts_there.rs): `prog args` with no shell after it,
    /// or the login shell when `prog` is empty. One already running under `key` is hung up first.
    pub async fn run_alone(self: &Arc<Self>, key: &str, title: &str, cwd: &str, prog: &str, args: &[String], cols: u16, rows: u16) -> Result<i32, String> {
        let (cols, rows) = (cols.clamp(20, vt::MAX_COLS as u16), rows.clamp(3, vt::MAX_ROWS as u16));
        { let mut g = self.inner.lock().await;
          if let Some(sh) = g.shells.remove(key) { sh.pty.hangup() }
          if g.shells.len() >= MAX_SHELLS { return Err("too many terminals are open".into()) } }
        let (prog, args) = if prog.is_empty() { (self.shell.clone(), vec!["-l".to_string()]) } else { (prog.to_string(), args.to_vec()) };
        self.launch(key, &prog, &args, cwd, cols, rows, title, true).await
    }
    /// The screen of `key` as text, its state ("running", "exited 0", "stopped"), whether its program still runs, and its width. None: no such terminal.
    pub async fn screen(&self, key: &str) -> Option<(String, String, bool, usize)> {
        let g = self.inner.lock().await;
        let sc = g.screens.get(key)?;
        Some((emb::text(&sc.vt), sc.state.clone(), g.shells.contains_key(key), sc.vt.cols()))
    }
    /// The chat terminals of a thread that are running now.
    pub async fn running_of(&self, thread: &str) -> Vec<String> { let pre = format!("x:{thread}:"); self.inner.lock().await.shells.keys().filter(|k| k.starts_with(&pre)).cloned().collect() }
    pub async fn shell_count(&self) -> usize { self.inner.lock().await.shells.len() }
}

fn b64(b: &[u8]) -> String { use base64::Engine; base64::engine::general_purpose::STANDARD.encode(b) }
/// chat (`x:<thread>:<id>`), shell (a client's own, `s:<project>:<n>`) or panel (a thread's floating terminal, keyed by the thread).
pub fn kind_of(k: &str) -> &'static str { if emb::is_key(k) { "chat" } else if k.starts_with("s:") { "shell" } else { "panel" } }
/// The thread a terminal belongs to ("" for a client's own shell).
pub fn thread_of(k: &str) -> &str { if emb::is_key(k) { k[2..].rsplitn(2, ':').nth(1).unwrap_or("") } else if k.starts_with("s:") { "" } else { k } }
fn view_title(sc: &Screen) -> String { if !sc.vt.title().is_empty() { sc.vt.title().to_string() } else { sc.title.clone() } }

// ---- reading the log for what the model does not index yet ----

/// The summary of chat terminal `id` of `thread` (its ActivityLogged) and its time, from the model's per-thread entries (newest first,
/// a page at a time: no scan of the whole log, and only this thread's rows are read).
pub async fn find_act(hub: &Arc<Hub>, thread: &str, id: &str) -> Option<(String, u64)> {
    let mut before = 0u64;
    loop {
        let page = { let m = hub.model().await; m.page(&hub.store, thread, before, 32).ok()? };
        for r in page.rows.iter().filter(|r| r.kind == "act" && r.id == id && r.role == emb::TONE) {
            let v: Value = serde_json::from_str(&hub.store.get(r.seq as usize - 1).ok()?).ok()?; // the row's text is cut; the log line is whole
            return Some((s(&v, "summary").to_string(), u(&v, "at", 0)));
        }
        if page.below == 0 || page.rows.is_empty() { return None }
        before = page.rows.iter().map(|r| r.ord).min()?;
    }
}

struct TermRpc(Arc<Terms>);
impl RpcHandler for TermRpc {
    fn methods(&self) -> &'static [&'static str] { &["term.open", "term.input", "term.resize", "term.snap", "term.restart", "term.stop"] }
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
                // a card's Stop: hang the terminal up (a chat terminal's or a client's own shell)
                "term.stop" => { let k = s(p, "key"); if !t.read_known(k).await { return vec![json!({"t":"reply","id":id,"ok":false,"error":"no such terminal"})] } t.kill(k).await; ok() }
                "term.restart" => {
                    let term = s(p, "id");
                    let Some((sum, _)) = find_act(hub, thread, term).await else { return vec![json!({"t":"reply","id":id,"ok":false,"error":"no such terminal"})] };
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

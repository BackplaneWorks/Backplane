//! KiCad IPC on the hub, in Rust.
//!
//! The transport ([`Transport`]) is in the hub: pure-Rust nng and protobuf (kipc.rs over nng.rs and proto.rs), so no Bun or JS
//! process runs under it.
//! Rust owns the actor: one record per project, at most two servers, requests wait for a server
//! that is starting, the 1/5/30 s restart schedule, the `-bp.N` version check after Ping + GetVersion, the server a third
//! project retires (asked to `kiapi.backplane.Shutdown`, never killed), the 18 commands a headless KiCad refuses, the bp.3
//! guards and the words shown. The pure rules are in kserve.rs (a Rust twin of core/kserve until the rules process is
//! wired). NOTHING HERE SENDS A SIGNAL TO ANY PROCESS: KiCad's servers end themselves (`--parent-pid`, `--idle-exit`,
//! `Shutdown`); a child is only ever `wait`ed for.
//!
//! Entry points for the `agents` stream's MCP layer: [`kicad_ipc`] and [`kicad_ipc_status`] (the MCP tools of the same
//! names), plus [`kicad_ipc_in`] / [`kicad_ipc_status_in`] which also take the calling thread so a fallback logs
//! "KiCad: fell back to files: <why>" as an activity line, exactly like `KIpc.fallback`. Client RPC: `kicad.retry`.
//! Deliberate difference from the JS hub: the project named by `path` must lie under a folder the hub knows (a project root
//! or a thread's worktree), as for `kicad.watch`; the JS hub starts a KiCad on any absolute path an agent names.
//! Not ported: KL.Library.prepare (managed library/config setup: BACKPLANE_KICAD_LIBRARY and KICAD_CONFIG_HOME are only
//! passed through from the hub's environment), `kicad-cli --version` probe (health is read from the runtime's manifest.json),
//! the info keys `kicad.*` / `kicad.ipc.servers` (the hub core owns info; [`servers`] returns the lines), Settings' install.
use super::{kipc, kserve::{self as ks, Srv}, project};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Map, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, OnceLock}, time::Duration};
use tokio::sync::{mpsc, oneshot};

/// What agents are told about RunAction's target editor (bp.13 batch 6, `runaction-frame`): with it, name the editor for an action both
/// editors have (an undo or redo on the schematic); without it, such an action goes to the board editor while one is open.
pub fn runaction_guidance(caps: &[String]) -> &'static str {
    if caps.iter().any(|c| c == "runaction-frame") {
        "kicad_ipc RunAction takes `frame`: name the editor for an action both editors have (`common.*`, an undo or redo): {\"action\": \"common.Interactive.undo\", \"frame\": \"FT_SCHEMATIC_EDITOR\"} for the schematic, FT_PCB_EDITOR for the board. KiCad refuses a frame that is not open or has no such action, saying why."
    } else {
        "kicad_ipc RunAction on this KiCad takes no `frame`: an action both editors have (`common.*`, an undo or redo) goes to the board editor while one is open; eeschema.* actions go to the schematic editor."
    }
}

/// How long a server that listens may say it is not ready yet before its start counts as failed.
pub const READY_WAIT: Duration = Duration::from_secs(30);
/// A reply that says KiCad is up but still loading (AS_NOT_READY, "KiCad is not ready to reply").
pub fn not_ready(reply: &Value) -> bool { reply["ok"] != true && (reply["status"] == "AS_NOT_READY" || reply["error"].as_str().is_some_and(|e| e.contains("not ready to reply"))) }
#[test] fn a_server_still_loading_is_not_a_failed_one() {
    assert!(not_ready(&json!({"ok": false, "status": "AS_NOT_READY", "error": "KiCad is not ready to reply"})) && not_ready(&json!({"ok": false, "error": "KiCad is not ready to reply"})));
    assert!(!not_ready(&json!({"ok": true})) && !not_ready(&json!({"ok": false, "error": "no reply"})) && !not_ready(&json!({"ok": false, "status": "AS_BAD_REQUEST", "error": "x"})));
}
/// What the runtime is: the kicad-cli, its root, the capabilities its manifest names, its version and API level (KEnv).
#[derive(Clone, Debug, Default)]
pub struct Env { pub cli: String, pub root: String, pub caps: Vec<String>, pub version: String, pub api: u64, pub status: String, pub why: String }

fn failed(text: &str) -> Value { json!({"ok":false,"error":text,"why":text,"fallback":true}) } // KServe.failed

enum Msg {
    Ask { project: String, env: Env, ty: String, msg: Value, reply: oneshot::Sender<Value> },
    Raw { sock: String, env: Env, ty: String, msg: Value, reply: oneshot::Sender<Value> },
    Retire { project: String, reply: oneshot::Sender<Value> },
    Retired { project: String, ok: bool, why: String, reply: oneshot::Sender<Value> },
    Info(oneshot::Sender<String>),
    Argv { project: String, env: Env, reply: oneshot::Sender<String> },
    Clear,
    Line(u64, Value),
    Up { project: String, version: String },
    Bad { project: String, why: String },
    Late { project: String, born: u64 },
    Go { project: String },
    Tick,
    Dump(String),
    Trim,
    Trimmed { project: String, ok: bool },
    Evicted { victim: String, project: String, ok: bool, why: String },
}

/// A server process's key ("<project>\t<number>") as its project and number.
fn proc_of(key: &str) -> (String, u64) { match key.rsplit_once('\t') { Some((p, n)) => (p.to_string(), n.parse().unwrap_or(0)), None => (key.to_string(), 0) } }
struct Wt { project: String, ty: String, msg: Value, env: Env, reply: oneshot::Sender<Value>, at: std::time::Instant }
/// A server answering a request, or asked within these seconds (an agent between two commands of one edit or one simulation), is
/// never the one stopped to make room: stopping it loses the request, the run and the open document.
const IN_USE_S: u64 = if cfg!(test) { 1 } else { 15 };
/// How long a request waits for room while both servers are at work, and the longest any request waits for its server at all.
const ROOM_WAIT: Duration = Duration::from_secs(if cfg!(test) { 4 } else { 120 });
const WAIT_MAX: Duration = Duration::from_secs(if cfg!(test) { 6 } else { 180 });
/// The longest a request is waited for from outside the actor, whatever happened inside it (KiCad's own limit is 30 s a request).
const ASK_MAX: Duration = Duration::from_secs(WAIT_MAX.as_secs() + 120);
/// The pool's room and times. The test knob BACKPLANE_KICAD_POOL_TEST ("room=2,in_use=5,room_wait=20,spare_idle=15", seconds; any
/// left out stays) lets an e2e on a real hub see each rule in minutes, and the two-server rules on a machine with room for more.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pool { room: usize, in_use: u64, room_wait: Duration, spare_idle: u64 }
fn pool_of(total_ram: Option<u64>, knob: &str) -> Pool {
    let mut p = Pool { room: ks::room(total_ram), in_use: IN_USE_S, room_wait: ROOM_WAIT, spare_idle: SPARE_IDLE_S };
    for (k, v) in knob.split(',').filter_map(|x| x.split_once('=')) { let Ok(n) = v.trim().parse::<u64>() else { continue };
        match k.trim() { "room" => p.room = (n as usize).clamp(1, 8), "in_use" => p.in_use = n, "room_wait" => p.room_wait = Duration::from_secs(n), "spare_idle" => p.spare_idle = n, _ => {} } }
    p
}
#[test] fn the_pools_test_knob() {
    let std = pool_of(Some(16 << 30), ""); assert_eq!(std, Pool { room: 2, in_use: IN_USE_S, room_wait: ROOM_WAIT, spare_idle: SPARE_IDLE_S });
    assert_eq!(pool_of(Some(64 << 30), "room=2, in_use=5,room_wait=20,spare_idle=15,x=1,room_wait=z"), Pool { room: 2, in_use: 5, room_wait: Duration::from_secs(20), spare_idle: 15 });
    assert_eq!(pool_of(Some(64 << 30), "in_use=5").room, 4);
}
pub const ROOM_BUSY: &str = "IPC busy: every KiCad server this machine has room for is at work for another project; ask again when one ends";
/// A server beyond the first two that nobody has asked for this long is shut down (the room past two is for work, not for keeping).
const SPARE_IDLE_S: u64 = if cfg!(test) { 2 } else { 600 };
/// What carries the actor's requests. It takes request objects and answers, through the actor's mailbox, with
/// the objects it writes (`Msg::Line`): replies by id, and `{"event":"line"|"exit"}` for a server.
trait Transport: Send { fn send(&mut self, fields: Value) -> bool; } // false: it is gone, and nothing will answer
/// The hub's own: the default.
impl Transport for kipc::Native { fn send(&mut self, fields: Value) -> bool { self.handle(fields); true } }
struct Helper { link: Box<dyn Transport>, gen: u64 }

struct Actor {
    me: mpsc::UnboundedSender<Msg>, home: String, rt: String, hub_pid: u32,
    helper: Option<Helper>, gen: u64, pend: HashMap<String, oneshot::Sender<Value>>, next: u64,
    srvs: Vec<Srv>, wait: Vec<Wt>, shown: String, env: Env, lines: Arc<std::sync::Mutex<String>>,
    /// Requests out per project and the project of each (by id); the processes alive per project (each start has a number: an old
    /// one's exit is not the new one's), the current one's number, and the projects whose start waits for an old process to be gone.
    uses: HashMap<String, u32>, pend_of: HashMap<String, String>, procs: HashMap<String, Vec<u64>>, cur: HashMap<String, u64>, proc_n: u64, relaunch: Vec<String>, ticking: bool,
    /// How many servers run at once (kserve::room, by the machine's memory), and whether the check that trims the ones beyond two is set.
    pool: Pool, trimming: bool,
    /// Servers asked to stop for another project and not answered yet: not asked a second time for a second waiter.
    stopping: Vec<String>,
}

pub fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }
pub fn now_ms() -> u128 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0) }

impl Actor {
    fn srv(&self, project: &str) -> Srv { self.srvs.iter().find(|s| s.project == project).cloned().unwrap_or_else(|| Srv::none(project)) } // KServe.get
    fn put(&mut self, s: Srv) { match self.srvs.iter_mut().find(|x| x.project == s.project) { Some(x) => *x = s, None => self.srvs.push(s) } } // KServe.put
    fn emit(&mut self) { let t = ks::lines(&self.srvs); if t != self.shown { *self.lines.lock().unwrap() = t.clone(); self.shown = t } } // KServe.emit
    fn later(&self, secs: f64, m: Msg) { let me = self.me.clone(); tokio::spawn(async move { tokio::time::sleep(Duration::from_secs_f64(secs)).await; let _ = me.send(m); }); }

    /// A request line registered for its reply (KServe.call). The helper is started on first use.
    fn call(&mut self, mut fields: Value, reply: oneshot::Sender<Value>) -> Option<String> {
        if self.helper.is_none() { self.start_helper() }
        let Some(h) = &mut self.helper else { let _ = reply.send(failed("the KiCad IPC transport could not start")); return None };
        self.next += 1; let id = self.next.to_string();
        fields["id"] = json!(self.next);
        // (registered before it is sent: a transport may answer at once)
        self.pend.insert(id.clone(), reply);
        if !h.link.send(fields) { if let Some(r) = self.pend.remove(&id) { let _ = r.send(failed("the KiCad IPC helper exited")); } return None }
        Some(id)
    }
    /// The servers as room is made among them: one that is answering a request, or was asked a moment ago, counts as held.
    fn in_use(&self) -> Vec<Srv> {
        let now = now_s();
        self.srvs.iter().cloned().map(|s| if s.state == "ready" && (self.uses.get(&s.project).copied().unwrap_or(0) > 0 || now.saturating_sub(s.last) < self.pool.in_use || self.stopping.contains(&s.project)) { s.hold() } else { s }).collect()
    }
    /// A project's server process is gone: every request out to it is answered with `why` now (no reply line will come for it),
    /// and it counts as used by nobody.
    fn gone(&mut self, project: &str, why: &str) {
        let ids: Vec<String> = self.pend_of.iter().filter(|(_, p)| p.as_str() == project).map(|(id, _)| id.clone()).collect();
        for id in ids { self.pend_of.remove(&id); if let Some(r) = self.pend.remove(&id) { let _ = r.send(json!({"ok": false, "error": why})); } }
        self.uses.remove(project);
    }
    /// Servers beyond the first two that have been idle SPARE_IDLE_S are asked to stop (the least recently used first); checked once a minute while there are more than two.
    fn trim(&mut self) {
        self.trimming = false; let now = now_s();
        while ks::count(&self.srvs) > 2 {
            let Some(v) = self.in_use().into_iter().filter(|s| s.state == "ready" && s.why != ks::HELD && now.saturating_sub(s.last) >= self.pool.spare_idle).min_by_key(|s| s.last) else { break };
            // (held until it answers, so it is asked once; one that refuses, for unsaved changes, stays held and is kept, as when room is made)
            let s = self.srv(&v.project).hold(); self.put(s); self.stopping.push(v.project.clone());
            let (sock, env, me, project) = (v.sock.clone(), self.env.clone(), self.me.clone(), v.project.clone());
            tokio::spawn(async move {
                let (t, r) = oneshot::channel(); let _ = me.send(Msg::Raw { sock, env, ty: "kiapi.backplane.Shutdown".into(), msg: json!({}), reply: t });
                let o = r.await.unwrap_or_else(|_| failed("no reply"));
                let _ = me.send(Msg::Trimmed { project, ok: o["ok"].as_bool().unwrap_or(false) });
            });
            break
        }
        self.spare();
    }
    fn spare(&mut self) { if ks::count(&self.srvs) > 2 && !self.trimming { self.trimming = true; self.later((self.pool.spare_idle as f64 / 4.0).clamp(1.0, 60.0), Msg::Trim) } }
    /// What the actor knows, in hub.log, when a request has waited past a limit: the servers, the requests out and the ones waiting.
    fn dump(&self, why: &str) {
        use std::io::Write; let now = now_s();
        let srvs: Vec<String> = self.srvs.iter().map(|s| format!("{} {} sock={} asked {} s ago{} out={} procs={:?} cur={:?}", ks::name(&s.project), s.state, s.sock, now.saturating_sub(s.last), if s.why.is_empty() { String::new() } else { format!(" ({})", s.why) },
            self.uses.get(&s.project).copied().unwrap_or(0), self.procs.get(&s.project).cloned().unwrap_or_default(), self.cur.get(&s.project))).collect();
        let wait: Vec<String> = self.wait.iter().map(|w| format!("{} {} {} s", ks::name(&w.project), w.ty.rsplit('.').next().unwrap_or(""), w.at.elapsed().as_secs())).collect();
        let out: Vec<String> = self.pend_of.iter().map(|(id, p)| format!("#{id} {}", ks::name(p))).collect();
        let _ = writeln!(std::io::stderr(), "kicad ipc: {why}; servers: [{}]; waiting: [{}]; out: [{}] ({} replies awaited); to start after an exit: {:?}", srvs.join(" | "), wait.join(" | "), out.join(", "), self.pend.len(), self.relaunch);
    }
    /// (First come, first served: the list is in the order asked, and a tick asks again in that order.)
    fn queue(&mut self, w: Wt) { self.wait.push(w); if !self.ticking { self.ticking = true; self.later(1.0, Msg::Tick) } }
    /// Once a second while anything waits: a request waiting for room is planned again (a server may have fallen idle), and none waits
    /// for ever: for room ROOM_WAIT, for its own server WAIT_MAX, whatever became of the events it was waiting on.
    fn tick(&mut self) {
        self.ticking = false;
        for w in std::mem::take(&mut self.wait) {
            let age = w.at.elapsed(); let starting = self.srv(&w.project).state == "starting";
            if age > WAIT_MAX { self.dump(&format!("a {} for {} waited {} s for its server", w.ty.rsplit('.').next().unwrap_or(""), ks::name(&w.project), age.as_secs())); let _ = w.reply.send(failed(&ks::unavailable(&format!("the headless KiCad for {} did not come up in {} s", ks::name(&w.project), WAIT_MAX.as_secs()), &w.env.cli))); }
            else if starting { self.wait.push(w) }
            else if age > self.pool.room_wait && ks::plan(&self.in_use(), &w.project, &self.env.caps, self.pool.room) == "busy" { let _ = w.reply.send(failed(&ks::unavailable(ROOM_BUSY, &w.env.cli))); }
            else { self.ask(w) }
        }
        if !self.wait.is_empty() && !self.ticking { self.ticking = true; self.later(1.0, Msg::Tick) }
    }
    fn start_helper(&mut self) {
        // (a new transport: what the old one was asked will never be answered by it, so it is answered here and nothing counts as in use)
        for (_, r) in self.pend.drain() { let _ = r.send(failed("the KiCad IPC helper restarted before answering; request outcome unknown")); }
        self.pend_of.clear(); self.uses.clear();
        self.gen += 1; let gen = self.gen;
        let me = self.me.clone();
        self.helper = Some(Helper { link: Box::new(kipc::Native::new("", Arc::new(move |o| { let _ = me.send(Msg::Line(gen, o)); }))), gen });
    }
    fn sink(&mut self, fields: Value) { let (t, _r) = oneshot::channel(); self.call(fields, t); }

    fn forward(&mut self, project: &str, env: &Env, ty: &str, msg: Value, reply: oneshot::Sender<Value>) { // KServe.forward
        let srv = self.srv(project).touch(now_s());
        let f = self.call_fields(&srv.sock, env, ty, msg); self.put(srv);
        if let Some(id) = self.call(f, reply) { self.pend_of.insert(id, project.to_string()); *self.uses.entry(project.to_string()).or_insert(0) += 1 }
    }
    fn call_fields(&self, sock: &str, env: &Env, ty: &str, msg: Value) -> Value { // KServe.call.fields
        json!({"sock":sock,"type":ty,"message":msg,"desc":ks::desc(&env.root, &env.caps),"lib":format!("{}/lib", env.root),"timeout":30000})
    }
    fn fail_project(&mut self, project: &str, text: &str) { // KServe.fail.project
        let (mine, rest): (Vec<Wt>, Vec<Wt>) = std::mem::take(&mut self.wait).into_iter().partition(|w| w.project == project);
        self.wait = rest;
        for w in mine { let _ = w.reply.send(failed(&ks::unavailable(text, &w.env.cli))); }
    }
    fn launch(&mut self, project: &str) { // KServe.launch: helper present, then the server is started and gets 30 s to say it listens
        let now = now_s();
        // its last process is still going (it was asked to stop and has not yet): a second one beside it would share its socket and its
        // project's lock. This start waits for that exit, 30 s at most (Late)
        let old = self.srv(project).sock; self.close(&old); // (and its connection and token are not the new server's)
        if self.procs.get(project).is_some_and(|p| !p.is_empty()) {
            if !self.relaunch.iter().any(|p| p == project) { self.relaunch.push(project.to_string()) }
            let s = self.srv(project).starting(now); self.put(s); self.later(30.0, Msg::Late { project: project.into(), born: now }); self.emit(); return
        }
        self.relaunch.retain(|p| p != project);
        self.proc_n += 1; let n = self.proc_n; self.procs.entry(project.to_string()).or_default().push(n); self.cur.insert(project.to_string(), n);
        let dir = ks::dir(&self.rt, &self.home, project);
        { use crate::plat::fs::DirBuilderExt; let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir); }
        let argv = ks::argv(&self.env.cli, project, &dir, self.hub_pid, &self.env.caps);
        let envv: Map<String, Value> = ["BACKPLANE_KICAD_LIBRARY", "KICAD_CONFIG_HOME"].iter().map(|k| (k.to_string(), json!(std::env::var(k).unwrap_or_default()))).collect();
        self.sink(json!({"op":"spawn","key":format!("{project}\t{n}"),"argv":argv,"memory":"1536M","env":envv}));
        let s = self.srv(project).starting(now); self.put(s);
        if self.helper.is_some() { self.later(30.0, Msg::Late { project: project.into(), born: now }) }
        self.emit();
    }
    fn close(&mut self, sock: &str) { if !sock.is_empty() { self.sink(json!({"op":"close","sock":sock})) } } // KServe.close

    fn ask(&mut self, w: Wt) {
        // its server has just been asked to stop for another project: a request sent to it now would be lost with it ("KiCad closed the
        // connection"). It waits; when the stop is answered the project is started again (after the old process's exit), or goes on if refused
        if self.stopping.contains(&w.project) { return self.queue(w) }
        // room is made only among servers nobody is using; while both are at work the request waits for one to fall idle (tick)
        let mut plan = ks::plan(&self.in_use(), &w.project, &self.env.caps, self.pool.room);
        if plan == "busy" && ks::plan(&self.srvs, &w.project, &self.env.caps, self.pool.room) != "busy" { plan = "room".into() }
        match plan.as_str() {
            "ready" => self.forward(&w.project.clone(), &w.env.clone(), &w.ty.clone(), w.msg, w.reply),
            "wait" | "room" => self.queue(w),
            "failed" => { let why = format!("the headless KiCad for {} stopped: {}", ks::name(&w.project), self.srv(&w.project).why); let _ = w.reply.send(failed(&ks::unavailable(&why, &w.env.cli))); }
            "busy" => { let _ = w.reply.send(failed(&ks::unavailable(ks::BUSY, &w.env.cli))); }
            "start" => { let p = w.project.clone(); self.queue(w); self.launch(&p) }
            p => { // "evict:<victim>": the least recently used server is asked to stop (it does so itself); the new one starts when it has
                let victim = p.trim_start_matches("evict:").to_string(); let project = w.project.clone();
                self.evicting(&victim, &project);
                let s = Srv::new(&project, now_s()); self.put(s); self.queue(w); self.emit();
            }
        }
    }
    fn evicting(&mut self, victim: &str, project: &str) { // KServe.evicting: kiapi.backplane.Shutdown, never a signal
        let sock = self.srv(victim).sock; let env = self.env.clone(); let me = self.me.clone();
        let (victim, project) = (victim.to_string(), project.to_string()); self.stopping.push(victim.clone());
        tokio::spawn(async move {
            let (t, r) = oneshot::channel(); let _ = me.send(Msg::Raw { sock, env, ty: "kiapi.backplane.Shutdown".into(), msg: json!({}), reply: t });
            let o = r.await.unwrap_or_else(|_| failed("no reply"));
            let _ = me.send(Msg::Evicted { victim, project, ok: o["ok"].as_bool().unwrap_or(false), why: o["error"].as_str().unwrap_or("").into() });
        });
    }
    /// A server that is listening answers Ping and GetVersion, and is the build the manifest names, before any request goes to it (KServe.hand).
    fn hand(&self, project: String, env: Env, sock: String) {
        let me = self.me.clone();
        tokio::spawn(async move {
            let ask = |ty: &str| { let (t, r) = oneshot::channel(); let _ = me.send(Msg::Raw { sock: sock.clone(), env: env.clone(), ty: ty.into(), msg: json!({}), reply: t }); r };
            // KiCad listens before it has finished loading and says so ("KiCad is not ready to reply", AS_NOT_READY): that is asked again for
            // up to READY_WAIT, never taken as a failed server (which stays failed until the person retries it)
            let mut p = ask("kiapi.common.commands.Ping").await.unwrap_or_else(|_| failed("no reply"));
            let until = std::time::Instant::now() + READY_WAIT;
            while not_ready(&p) && std::time::Instant::now() < until { tokio::time::sleep(Duration::from_millis(250)).await; p = ask("kiapi.common.commands.Ping").await.unwrap_or_else(|_| failed("no reply")); }
            if !p["ok"].as_bool().unwrap_or(false) { let _ = me.send(Msg::Bad { project, why: format!("it did not answer Ping: {}", p["error"].as_str().unwrap_or("")) }); return }
            let o = ask("kiapi.common.commands.GetVersion").await.unwrap_or_else(|_| failed("no reply"));
            let full = o["message"]["version"]["full_version"].as_str().unwrap_or("").to_string();
            // status says the manifest's version (a test build's GetVersion is a describe name, "...-dirty-bp.13"); KiCad's own string is logged
            if o["ok"].as_bool().unwrap_or(false) && ks::version_ok(&full, &env.version) {
                // (written so that a closed stderr cannot stop the handshake: eprintln! panics there, and the server would never be handed its requests)
                { use std::io::Write; let _ = writeln!(std::io::stderr(), "kicad ipc: {project} up, KiCad says {full} (manifest {})", env.version); }
                let _ = me.send(Msg::Up { project, version: shown_version(&env.version, &full) }); }
            else { let err = o["error"].as_str().unwrap_or(""); let _ = me.send(Msg::Bad { project, why: if err.is_empty() { format!("this KiCad is {full}, not the {} Backplane installed", env.version) } else { format!("it did not answer GetVersion: {err}") } }); }
        });
    }

    fn line(&mut self, o: Value) { // what the transport said: a server's stdout line or exit, else a reply (by id)
        match o["event"].as_str() {
            Some("line") => {
                let (project, n) = proc_of(o["key"].as_str().unwrap_or(""));
                if self.cur.get(&project) != Some(&n) { return } // a process this record is no longer about
                if let Some(path) = ks::ready(o["text"].as_str().unwrap_or("")) {
                    let s = self.srv(&project);
                    if s.state == "starting" && s.sock.is_empty() { self.hand(project.clone(), self.env.clone(), path.clone()); let s = s.listening(&path); self.put(s) }
                }
            }
            Some("exit") => {
                let (project, n) = proc_of(o["key"].as_str().unwrap_or(""));
                if let Some(p) = self.procs.get_mut(&project) { p.retain(|x| *x != n) }
                // the current process's exit is the server's; an older one's (stopped to make room, started again since) only frees its place
                let current = self.cur.get(&project) == Some(&n);
                // (the current process's requests out get no reply line: answered here, and the project counts as unused at once)
                if current { self.cur.remove(&project); self.gone(&project, &format!("the headless KiCad for {} exited before answering; request outcome unknown", ks::name(&project))) }
                if self.relaunch.iter().any(|p| *p == project) && self.srv(&project).state == "starting" { // a start was waiting for this exit
                    let sock = self.srv(&project).sock; self.close(&sock);
                    if self.procs.get(&project).map_or(true, |p| p.is_empty()) { self.launch(&project) }
                } else if current && self.stopping.contains(&project) && o["code"].as_i64() == Some(0) {
                    // it left because it was asked to, before the answer to that was handled: stopped, and what waits for the project
                    // is not failed for it (it is started again when the stop's answer is in)
                    let s = self.srv(&project); self.close(&s.sock); self.put(s.set("stopped", "")); self.emit()
                } else if current { self.exited(project, o["code"].as_i64().unwrap_or(1) as i32, o["tail"].as_str().unwrap_or("").to_string()) }
            }
            _ => { let id = match &o["id"] { Value::String(s) => s.clone(), v => v.to_string() };
                if let Some(p) = self.pend_of.remove(&id) { if let Some(u) = self.uses.get_mut(&p) { *u = u.saturating_sub(1) } let s = self.srv(&p); if s.state == "ready" { self.put(s.touch(now_s())) } }
                if let Some(r) = self.pend.remove(&id) { let _ = r.send(o); } }
        }
    }
    fn exited(&mut self, project: String, code: i32, tail: String) { // the server's process ended: the record says what that means
        let now = now_s(); let before = self.srv(&project);
        if before.state == "stopped" { self.close(&before.sock); return }
        let srv = before.exited(code, now, &tail); self.put(srv.clone()); self.close(&srv.sock);
        let name = ks::name(&project);
        match srv.state.as_str() {
            "starting" => if let Some(s) = ks::retry(&srv.fails, now) { self.later(s as f64, Msg::Go { project }) },
            "failed" => self.fail_project(&project, &format!("the headless KiCad for {name} stopped: {}", srv.why)),
            _ => self.fail_project(&project, &format!("the headless KiCad for {name} exited")),
        }
        self.emit();
    }
    fn up(&mut self, project: String, version: String) { // it answered Ping and GetVersion as the right build: ready, and what waited goes out
        let s = self.srv(&project); if s.state != "starting" { return }
        let sock = s.sock.clone(); self.put(s.up(&sock, &version, now_s()));
        let (mine, rest): (Vec<Wt>, Vec<Wt>) = std::mem::take(&mut self.wait).into_iter().partition(|w| w.project == project); self.wait = rest;
        for w in mine { self.forward(&w.project.clone(), &w.env.clone(), &w.ty.clone(), w.msg, w.reply) }
        self.emit(); self.spare();
    }
    fn bad(&mut self, project: String, why: String) { // it failed the handshake or never listened
        if self.srv(&project).state != "starting" { return }
        let s = self.srv(&project).set("failed", &why); self.put(s);
        self.fail_project(&project, &format!("the headless KiCad for {} {why}", ks::name(&project))); self.emit();
    }
    fn evicted(&mut self, victim: String, project: String, ok: bool, why: String) {
        self.stopping.retain(|p| *p != victim);
        if ok { let s = self.srv(&victim).set("stopped", ""); self.put(s); self.launch(&project); return }
        let s = self.srv(&victim).hold(); self.put(s); // Shutdown refused (unsaved changes): held, never forced; the next least recently used is asked
        let other = ks::evict(&self.in_use(), self.pool.room);
        if other.is_empty() {
            let s = self.srv(&project).set("stopped", ""); self.put(s);
            self.fail_project(&project, &format!("{} ({} would not stop: {why})", ks::BUSY, ks::name(&victim))); self.emit();
        } else { self.evicting(&other, &project) }
    }
    fn retire(&mut self, project: String, reply: oneshot::Sender<Value>) {
        let s = self.srv(&project);
        if s.sock.is_empty() || matches!(s.state.as_str(), "none" | "stopped" | "failed") {
            self.put(s.set("stopped", "")); self.fail_project(&project, "the GUI KiCad took over this project"); self.emit();
            let _ = reply.send(json!({"ok":true,"retired":false})); return
        }
        let sock = s.sock.clone(); let env = self.env.clone(); let me = self.me.clone();
        tokio::spawn(async move {
            let (t, r) = oneshot::channel(); let _ = me.send(Msg::Raw { sock, env, ty: "kiapi.backplane.Shutdown".into(), msg: json!({}), reply: t });
            let o = r.await.unwrap_or_else(|_| failed("no reply"));
            let _ = me.send(Msg::Retired { project, ok: o["ok"].as_bool().unwrap_or(false), why: o["error"].as_str().unwrap_or("").into(), reply });
        });
    }
    fn retired(&mut self, project: String, ok: bool, why: String, reply: oneshot::Sender<Value>) {
        if ok {
            let s = self.srv(&project).set("stopped", ""); self.put(s);
            self.fail_project(&project, "the GUI KiCad took over this project"); self.emit();
            let _ = reply.send(json!({"ok":true,"retired":true}));
        } else {
            let s = self.srv(&project).hold(); self.put(s); self.emit();
            let _ = reply.send(json!({"ok":false,"error":why}));
        }
    }
    fn handle(&mut self, m: Msg) {
        match m {
            Msg::Ask { project, env, ty, msg, reply } => { self.env = env.clone(); self.ask(Wt { project, ty, msg, env, reply, at: std::time::Instant::now() }) }
            Msg::Raw { sock, env, ty, msg, reply } => { let f = self.call_fields(&sock, &env, &ty, msg); self.call(f, reply); }
            Msg::Tick => self.tick(),
            Msg::Dump(why) => self.dump(&why),
            Msg::Trim => self.trim(),
            Msg::Trimmed { project, ok } => { self.stopping.retain(|p| *p != project); let s = self.srv(&project); if ok && s.state == "ready" { self.put(s.set("stopped", "")); self.emit() } }
            Msg::Retire { project, reply } => self.retire(project, reply),
            Msg::Retired { project, ok, why, reply } => self.retired(project, ok, why, reply),
            Msg::Info(r) => { let _ = r.send(ks::lines(&self.srvs)); }
            Msg::Argv { project, env, reply } => { let d = ks::dir(&self.rt, &self.home, &project); let _ = reply.send(ks::argv(&env.cli, &project, &d, self.hub_pid, &env.caps).join(" ")); }
            Msg::Clear => { for s in self.srvs.iter_mut().filter(|s| s.state == "failed") { *s = s.clone().cleared() } self.emit() }
            Msg::Line(gen, t) => if self.helper.as_ref().map(|h| h.gen) == Some(gen) { self.line(t) },
            Msg::Up { project, version } => self.up(project, version),
            Msg::Bad { project, why } => self.bad(project, why),
            Msg::Late { project, born } => { let s = self.srv(&project); if s.state == "starting" && s.started == born && s.sock.is_empty() {
                let waits = self.relaunch.iter().any(|p| *p == project); self.relaunch.retain(|p| *p != project);
                self.bad(project, if waits { "could not start: its last process, asked to stop, had not exited after 30 s".into() } else { "did not say it was listening within 30 s".into() }) } }
            Msg::Go { project } => if self.srv(&project).state == "starting" { self.launch(&project) },
            Msg::Evicted { victim, project, ok, why } => self.evicted(victim, project, ok, why),
        }
    }
}

pub struct Ipc { tx: mpsc::UnboundedSender<Msg>, lines: Arc<std::sync::Mutex<String>> }
static IPC: OnceLock<Ipc> = OnceLock::new();

/// Start the actor (one line in kicad::start). The transport itself starts on the first request.
pub fn start(hub: &Arc<Hub>) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let lines = Arc::new(std::sync::Mutex::new(String::new()));
    let home = hub.cfg.home.to_string_lossy().to_string();
    let mut a = Actor { me: tx.clone(), home, rt: std::env::var("XDG_RUNTIME_DIR").unwrap_or_default(), hub_pid: std::process::id(), helper: None, gen: 0, pend: HashMap::new(), next: 0,
        srvs: vec![], wait: vec![], shown: String::new(), env: Env::default(), lines: lines.clone(),
        uses: HashMap::new(), pend_of: HashMap::new(), procs: HashMap::new(), cur: HashMap::new(), proc_n: 0, relaunch: vec![], ticking: false, pool: pool_of(crate::memscope::total_ram(), &std::env::var("BACKPLANE_KICAD_POOL_TEST").unwrap_or_default()), trimming: false, stopping: vec![] };
    tokio::spawn(async move { while let Some(m) = rx.recv().await { a.handle(m) } });
    if IPC.set(Ipc { tx, lines }).is_ok() { hub.register_rpc(Arc::new(IpcRpc)) }
    // Settings > CAD reads kicad.* and freecad from the hub's info: say what is found now, and again whenever the path field changes
    let h = hub.clone();
    tokio::spawn(async move {
        let mut rx = h.subscribe(); let mut seen = String::from("\u{0}");
        loop {
            let fcp = h.setting("freecad.path").await.unwrap_or_default(); super::stepglb::set_freecad_setting(&fcp);
            let now = format!("{}\u{1}{fcp}", h.setting("kicad.path").await.unwrap_or_default());
            if now != seen { seen = now; publish_cad(&h).await }
            match rx.recv().await { Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {} Err(_) => return }
        }
    });
}
/// The servers' lines now ("project|state|why", one per line): the info key `kicad.ipc.servers`.
pub fn servers() -> String { IPC.get().map(|i| i.lines.lock().unwrap().clone()).unwrap_or_default() }

// The runtime ---------------------------------------------------------------------------------------------------------------

/// The installs under `dirs` (each a folder of `<version>/bin/kicad-cli`), newest first across all of them: a newer runtime in any of the hub's
/// install folders wins over an older one in another.
fn newest_in(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut v: Vec<(String, PathBuf)> = dirs.iter().flat_map(|d| std::fs::read_dir(d).map(|r| r.flatten().filter_map(|e| e.file_name().into_string().ok().map(|n| (n, d.clone()))).collect::<Vec<_>>()).unwrap_or_default()).collect();
    v.sort_by(|a, b| match (ks::version_ge(&a.0, &b.0), ks::version_ge(&b.0, &a.0)) { (true, true) => std::cmp::Ordering::Equal, (true, false) => std::cmp::Ordering::Less, _ => std::cmp::Ordering::Greater });
    v.into_iter().flat_map(|(n, d)| [d.join(&n).join("bin").join("kicad-cli"), d.join(n).join("bin").join("kicad-cli.exe")]).collect()
}
/// `<cli> --version`, its settings in a throwaway folder (proc::kicad_probe): the probe never writes the person's KiCad settings.
/// Bounded (proc::probe_output): a run that hangs (#1160, macOS) is killed and the probe asked once more, since only some runs hang.
pub fn cli_version_output(cli: &str) -> Option<std::process::Output> { cli_version_within(cli, crate::proc::PROBE_LIMIT) }
pub fn cli_version_within(cli: &str, limit: std::time::Duration) -> Option<std::process::Output> {
    (0..2).find_map(|_| { let (mut c, _home) = crate::proc::kicad_probe(cli); c.arg("--version"); crate::proc::probe_output(c, limit) })
}
/// Publish what this machine has for Settings > CAD: info `kicad.cli`, `kicad.version`, `kicad.status`, `kicad.source` and `freecad` (empty when absent).
pub async fn publish_cad(hub: &Arc<Hub>) {
    let env = runtime(hub).await;
    let cli = env.cli.clone();
    let probe = if cli.is_empty() { None } else {
        tokio::task::spawn_blocking(move || cli_version_output(&cli)).await.ok().flatten()
    };
    let ran = probe.as_ref().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let (status, version) = cad_status(&env, ran.as_deref());
    let source = if hub.setting("kicad.path").await.map_or(false, |s| !s.is_empty()) { "setting" } else { "path" };
    let fc = super::stepglb::freecad_cmd(&hub.cfg.home);
    // the found runtime's own capabilities (its manifest), comma-separated: what an update offers over it is said from these
    let caps = env.caps.join(",");
    // the Backplane library the runtime pins (its manifest's backplaneLibrary): "2026.11.13 (314532f)", "" for none
    let library = if env.root.is_empty() || status == "stock" { String::new() } else { library_label(&manifest_of(&env.root)) };
    // the update a person is offered where they work (the pane's strip at Edit, as well as Settings): this hub's package over an older
    // Backplane runtime, with what it brings and its size; "" when there is none (install.rs `update_offer`)
    let update = super::install::update_offer(super::install::package(), &status, &version, &env.caps).map_or(String::new(), |v| v.to_string());
    eprintln!("kicad: runtime {} {} at {}; update offer: {}", status, if version.is_empty() { "-" } else { &version }, if env.cli.is_empty() { "-" } else { &env.cli }, if update.is_empty() { "none" } else { &update });
    for (k, v) in [("kicad.cli", env.cli.as_str()), ("kicad.version", version.as_str()), ("kicad.status", status.as_str()), ("kicad.source", source), ("kicad.caps", caps.as_str()), ("kicad.library", library.as_str()), ("kicad.update", update.as_str()), ("freecad", fc.as_str())] { hub.set_info(k, v).await; }
    // the libraries the selected runtime registers by default reach the person's existing tables (libtables.rs); a copy of the log changes nothing
    if !env.root.is_empty() && !crate::copy::on() {
        let (root, ver, home) = (PathBuf::from(&env.root), version.clone(), hub.cfg.home.clone());
        let said = tokio::task::spawn_blocking(move || super::libtables::config_root().map(|c| super::libtables::sync(&root, &ver, &c, &home)).unwrap_or_default()).await.unwrap_or_default();
        for l in said { eprintln!("kicad: library tables: {l}") }
    }
}
/// What Settings > CAD says of a runtime: its status and version from the probe's `--version` (None: it did not run). A Backplane
/// runtime's `--version` prints only KiCad's number (bp.4 prints `10.99.0`), so its manifest's version and status are taken when the
/// manifest names a Backplane build; a CLI with no such manifest is stock KiCad.
pub fn cad_status(env: &Env, ran: Option<&str>) -> (String, String) {
    match ran {
        _ if env.cli.is_empty() => ("missing".into(), String::new()),
        None => ("broken".into(), String::new()),
        Some(_) if env.version.contains("-bp.") => (env.status.clone(), env.version.clone()),
        Some(v) if v.contains("-bp.") => (env.status.clone(), v.to_string()),
        Some(v) => ("stock".into(), v.to_string()),
    }
}
/// Where a runtime is looked for, as `places` reads them from this process (tests give their own).
#[derive(Clone, Debug, Default)]
pub struct Places { pub setting: Option<String>, pub env_cli: Option<PathBuf>, pub beside: Option<PathBuf>, pub data: String, pub hub_home: PathBuf, pub install: PathBuf, pub system: bool, pub path: Vec<PathBuf>, pub mac: Vec<PathBuf> }
pub fn places(hub_home: &Path, setting: Option<String>) -> Places {
    let user_home = crate::plat::dirs::home_or_empty();
    let data = std::env::var("XDG_DATA_HOME").ok().filter(|s| !s.is_empty()).unwrap_or(format!("{user_home}/.local/share"));
    Places { setting, env_cli: std::env::var_os("BACKPLANE_KICAD_CLI").filter(|s| !s.is_empty()).map(PathBuf::from), beside: std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("kicad/bin/kicad-cli"))),
        data, hub_home: hub_home.to_path_buf(), install: super::install::parent(hub_home),
        // BACKPLANE_KICAD_NO_SYSTEM=1 (rigs that try Install KiCad on a machine that has one): the system's own copies (PATH, /usr/bin, the macOS app) are not looked at
        system: std::env::var("BACKPLANE_KICAD_NO_SYSTEM").map_or(true, |v| v != "1"),
        path: std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).filter(|d| !d.as_os_str().is_empty()).collect(), mac: super::stepglb::kicad_mac_clis().into_iter().map(PathBuf::from).collect() }
}
/// The runtime candidates in order (Rt.candidates, trimmed): the `kicad.path` setting, BACKPLANE_KICAD_CLI, beside the hub, the hub's own installs (newest first
/// across its install folders), then the system's KiCad (PATH, /usr/bin, the macOS app). 0.14's home (`~/.backplane-bend/kicad`) is never looked in: what is left
/// there is a stale fork that must not win over the hub's own install.
pub fn candidates(p: &Places) -> Vec<PathBuf> {
    let mut c: Vec<PathBuf> = vec![];
    if let Some(s) = p.setting.as_ref().filter(|s| !s.is_empty()) {
        // the field takes the command line itself, or a folder that holds it (an install's bin/, or a macOS KiCad.app)
        let f = PathBuf::from(s);
        if f.is_dir() { for r in ["kicad-cli", "kicad-cli.exe", "bin/kicad-cli", "bin/kicad-cli.exe", "Contents/MacOS/kicad-cli", "KiCad.app/Contents/MacOS/kicad-cli"] { c.push(r.split('/').fold(f.to_path_buf(), |a, s| a.join(s))) } } else { c.push(f) }
    }
    c.extend(p.env_cli.clone()); c.extend(p.beside.clone());
    let mut managed = vec![PathBuf::from(&p.data).join("backplane/kicad"), p.install.clone(), p.hub_home.join("kicad")]; managed.dedup();
    c.extend(newest_in(&managed));
    if p.system {
        for d in &p.path { c.push(d.join("kicad-cli")); c.push(d.join("kicad-cli.exe")) }
        c.push("/usr/bin/kicad-cli".into());
        c.extend(p.mac.iter().cloned()); // macOS: the app bundle, system-wide or in ~/Applications
    }
    c
}
/// The first candidate that is there, read (`Env.status` "missing" when none is).
pub fn pick(c: Vec<PathBuf>) -> Env { match c.into_iter().find(|p| p.is_file()) { Some(p) => read_runtime(&p), None => Env { status: "missing".into(), ..Default::default() } } }
/// The runtime to use: the one Backplane has selected, for the hub's own work and every agent's (agents/kicadsel.rs).
pub async fn runtime(hub: &Arc<Hub>) -> Env { pick(candidates(&places(&hub.cfg.home, hub.setting("kicad.path").await))) }
/// The Backplane library a runtime pins, as Settings says it: "2026.11.13 (314532f)", from its manifest's `backplaneLibrary` (`version`, the
/// fork's bundle label, and `commit`, the pin; the short commit keeps the label from reading as a tag of the library's repository). ""
/// for a runtime without one (stock KiCad, bp.12.x and older).
pub fn library_label(manifest: &Value) -> String {
    let l = &manifest["backplaneLibrary"];
    let (v, c) = (l["version"].as_str().unwrap_or("").trim(), l["commit"].as_str().unwrap_or("").trim());
    let short: String = c.chars().take(7).collect();
    match (v.is_empty(), short.is_empty()) { (false, false) => format!("{v} ({short})"), (false, true) => v.to_string(), (true, false) => short, _ => String::new() }
}
/// A runtime root's manifest.json (the package's, or a bundle's Contents/Resources), Null when it has none.
pub fn manifest_of(root: &str) -> Value {
    let text = std::fs::read_to_string(format!("{root}/manifest.json")).or_else(|_| std::fs::read_to_string(format!("{root}/Contents/Resources/manifest.json"))).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or(Value::Null)
}
/// Rt.manifest + KH.status from the runtime's manifest.json (no version probe: `runs` is taken as true when a manifest is read).
pub fn read_runtime(cli: &Path) -> Env {
    let root = cli.parent().and_then(|b| b.parent()).map(|r| r.to_string_lossy().to_string()).unwrap_or_default();
    let m = manifest_of(&root);
    let version = m["version"].as_str().unwrap_or("").to_string(); let api = m["apiVersion"].as_u64().unwrap_or(0);
    let fork = version.contains("-bp.");
    let caps: Vec<String> = m["capabilities"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    let status = ks::status(true, true, fork, &version, api).to_string();
    Env { cli: cli.to_string_lossy().into(), root, caps: ks::caps_safe(&caps, api), version, api, status, why: String::new() }
}

// What the hub asks ---------------------------------------------------------------------------------------------------------

async fn ask(i: &Ipc, project: &str, env: &Env, ty: &str, msg: Value) -> Value {
    let (t, r) = oneshot::channel();
    if i.tx.send(Msg::Ask { project: project.into(), env: env.clone(), ty: ty.into(), msg, reply: t }).is_err() { return failed("no reply") }
    // (bounded from outside too: whatever became of the request inside, its caller is answered)
    match tokio::time::timeout(ASK_MAX, r).await { Ok(o) => o.unwrap_or_else(|_| failed("no reply")), Err(_) => { let _ = i.tx.send(Msg::Dump(format!("a {} for {} got no answer in {} s", ty.rsplit('.').next().unwrap_or(""), ks::name(project), ASK_MAX.as_secs()))); failed(&format!("KiCad did not answer in {} s", ASK_MAX.as_secs())) } }
}
async fn info_on(i: &Ipc) -> String { let (t, r) = oneshot::channel(); if i.tx.send(Msg::Info(t)).is_err() { return String::new() } r.await.unwrap_or_default() }
async fn argv_on(i: &Ipc, project: &str, env: &Env) -> String { let (t, r) = oneshot::channel(); if i.tx.send(Msg::Argv { project: project.into(), env: env.clone(), reply: t }).is_err() { return String::new() } r.await.unwrap_or_default() }

/// Ask a running headless KiCad for `project` to shut down cleanly (`kiapi.backplane.Shutdown`, never a signal) before a GUI KiCad takes over.
pub async fn retire(project: &str) -> Value {
    let Some(i) = IPC.get() else { return json!({"ok":true,"retired":false}) };
    let (t, r) = oneshot::channel();
    if i.tx.send(Msg::Retire { project: project.into(), reply: t }).is_err() { return json!({"ok":true,"retired":false}) }
    r.await.unwrap_or_else(|_| json!({"ok":true,"retired":false}))
}

/// KIpc.pro: the project of a path: the .kicad_pro itself, a project folder, or the one beside a board or schematic ("" for none).
/// A canonical document named by its worktree twin resolves to main's copy first (canonical.rs).
pub fn pro_of(path: &str) -> String {
    if !crate::plat::fs::is_abs(path) { return String::new() }
    let path = super::canonical::route(path);
    if path.ends_with(".kicad_pro") { return path }
    let here = if Path::new(&path).is_dir() { project::read(&super::canonical::design_root(&path)).pro } else { Path::new(&path).parent().map(|d| project::read(&d.to_string_lossy()).pro).unwrap_or_default() };
    if !here.is_empty() { return here }
    // fab output (mfg/<board>/gerber/...: kicad/gerber) is its project's: the folders above it, three at most
    if super::gerber::is_set_path(Path::new(&path)) {
        for d in Path::new(&path).ancestors().skip(1).take(4) { let p = project::read(&d.to_string_lossy()).pro; if !p.is_empty() { return p } }
    }
    String::new()
}
/// A listed KiCad read (`Get*`, `Ping`): the commands a copy may run and a writer lock never gates.
/// The requests that wait out KiCad's task gate in the open editor rather than fail: reads, and the routing plan's commands.
pub fn patient_of(type_: &str) -> bool { is_read(type_) || (type_.starts_with("kiapi.backplane.plan.") && !type_.ends_with(".ApprovePlan")) }
pub fn is_read(type_: &str) -> bool {
    let name = type_.rsplit('.').next().unwrap_or(type_);
    name.starts_with("Get") || name == "Ping"
}
/// KIpc.err: KiCad's own error, with its status.
fn err_of(o: &Value) -> String {
    let (st, e) = (o["status"].as_str().unwrap_or(""), o["error"].as_str().unwrap_or(""));
    if st.is_empty() { if e.is_empty() { "KiCad did not answer".into() } else { e.into() } } else { format!("KiCad answered {st}: {e}") }
}
/// KIpc.check: what is wrong with the request itself ("" when nothing).
pub fn check(ty: &str, path: &str, pro: &str, msg: &Value, caps: &[String]) -> String {
    if ty.is_empty() { return "type is required: the protobuf command, e.g. kiapi.board.commands.GetItems".into() }
    if !crate::plat::fs::is_abs(path) { return "path must be the absolute path of the project's .kicad_pro, .kicad_pcb or .kicad_sch".into() }
    if pro.is_empty() { return format!("no KiCad project (.kicad_pro) found for {path}") }
    if ks::headless_refused(ty) { return ks::REFUSAL.into() }
    ks::guard(ty, msg, caps)
}
/// Same validation as [`check`], without the headless-only command refusal when a live GUI KiCad owns `pro`.
pub fn check_gui(ty: &str, path: &str, pro: &str, msg: &Value, caps: &[String]) -> String {
    if ty.is_empty() { return "type is required: the protobuf command, e.g. kiapi.board.commands.GetItems".into() }
    if !crate::plat::fs::is_abs(path) { return "path must be the absolute path of the project's .kicad_pro, .kicad_pcb or .kicad_sch".into() }
    if pro.is_empty() { return format!("no KiCad project (.kicad_pro) found for {path}") }
    ks::guard(ty, msg, caps)
}

async fn log_fallback(hub: &Arc<Hub>, thread: Option<&str>, why: &str) {
    let Some(th) = thread.filter(|t| !t.is_empty()) else { return };
    let now = now_s() * 1000;
    let _ = hub.commit(&json!({"$":"ActivityLogged","thread":th,"act":format!("kicad-ipc:{now}"),"tone":"info","kind":"kicad","summary":ks::activity(why),"at":now})).await;
}
/// One command for the project's headless KiCad (the MCP tool `kicad_ipc`): Ok({"type","message"}) or Err(text for the agent).
pub async fn kicad_ipc(hub: &Arc<Hub>, path: &str, type_: &str, message: Value) -> Result<Value, String> { kicad_ipc_in(hub, None, path, type_, message).await }
pub async fn kicad_ipc_in(hub: &Arc<Hub>, thread: Option<&str>, path: &str, type_: &str, message: Value) -> Result<Value, String> {
    if let Some(why) = copy_ipc_refused(crate::copy::on(), type_) { return Err(why) }
    run(hub, thread, path, type_, message, false).await
}
/// Default-deny on a copy (BACKPLANE_COPY=1): KiCad edits the real project's files (CreateItems, SaveDocument ...), so only the commands listed as reads run: `Get*` and `Ping`.
pub fn copy_ipc_refused(copy: bool, type_: &str) -> Option<String> {
    let name = type_.rsplit('.').next().unwrap_or(type_);
    if copy && !is_read(type_) { Some(format!("a hub on a copied log (BACKPLANE_COPY=1) writes nothing outside its home: {name} is not a listed read")) } else { None }
}
/// Approval of the routing plan is the person's alone (the fork refuses it for agent clients from bp.13 batch 4, f74ea40f83; this
/// refuses it on every build): an agent's `kicad_ipc` may not send ApprovePlan, nor RunAction of the plan view's accept, reject,
/// approve-layer or reject-layer. `plan_approve` asks the person; the Plan panel and KiCad's own plan view are where they decide.
pub fn approval_refused(type_: &str, msg: &Value) -> Option<String> {
    const ACTIONS: &[&str] = &["pcbnew.Plan.accept", "pcbnew.Plan.reject", "pcbnew.Plan.approveLayer", "pcbnew.Plan.rejectLayer"];
    let short = type_.rsplit('.').next().unwrap_or(type_);
    let action = msg["action"].as_str().unwrap_or("");
    (short == "ApprovePlan" || (short == "RunAction" && ACTIONS.contains(&action))).then(|| format!("Approval is the person's alone: an agent cannot {}. Ask them with plan_approve and end your turn; Backplane tells you what they decided.", if short == "ApprovePlan" { "send ApprovePlan".to_string() } else { format!("run {action}") }))
}
/// The MCP tool `kicad_ipc_status`: Ok({"server","servers","argv","version","capabilities","detail"}); starts the server if it is not running.
pub async fn kicad_ipc_status(hub: &Arc<Hub>, path: &str) -> Result<Value, String> { kicad_ipc_status_in(hub, None, path).await }
pub async fn kicad_ipc_status_in(hub: &Arc<Hub>, thread: Option<&str>, path: &str) -> Result<Value, String> { run(hub, thread, path, "kiapi.backplane.GetCapabilities", json!({}), true).await }

/// One command for the project's headless KiCad even while a GUI KiCad has the project: for a document that GUI does not have open
/// (the hub's BOM edit with only the board, or nothing, open there: a GUI KiCad refuses OpenDocument).
pub async fn kicad_ipc_headless(hub: &Arc<Hub>, path: &str, type_: &str, message: Value) -> Result<Value, String> {
    if let Some(why) = copy_ipc_refused(crate::copy::on(), type_) { return Err(why) }
    run_on(hub, None, path, type_, message, false, false).await
}
/// Before an agent's SaveDocument through the open KiCad: Err(the answer) when nothing is to be written (KiCad says the document is
/// unmodified; or the board's only change since it opened is KiCad's own load-time migration, which has no way back: lane P, bp.12.2,
/// UndoAgentChange of it drops the board's teardrops); else Ok(the format-upgrade line the save will carry, if any).
pub async fn save_gate(gui: &super::embed::GuiRoute, msg: &Value, path: &str, saves_board: bool) -> Result<Option<String>, Value> {
    let skip = |why: String| json!({"type": "kiapi.common.commands.SaveDocumentResponse", "message": {}, "not_saved": why});
    if super::savegate::modified(gui, &msg["document"]).await == Some(false) { return Err(skip(super::savegate::NOTHING_CHANGED.into())) }
    if saves_board {
        if let Some((fp, zones, 0)) = super::coedit::load_migration_full(gui).await {
            return Err(skip(format!("{}; that is the board's only change since it opened, so it was not saved. Save your own change first (it is then saved with KiCad's, and the result says so).", super::coedit::load_migration_words(fp, zones))))
        }
    }
    Ok(super::savegate::upgrade_of(gui, path).await)
}
/// Whether a SaveDocument is the board's: its document's type when given, else the path asked.
pub fn board_save(msg: &Value, path: &str) -> bool {
    match msg["document"]["type"].as_str() { Some(t) => t == "DOCTYPE_PCB", None => path.ends_with(".kicad_pcb") }
}
async fn run(hub: &Arc<Hub>, thread: Option<&str>, path: &str, ty: &str, msg: Value, status: bool) -> Result<Value, String> { run_on(hub, thread, path, ty, msg, status, true).await }
async fn run_on(hub: &Arc<Hub>, thread: Option<&str>, path: &str, ty: &str, msg: Value, status: bool, gui_first: bool) -> Result<Value, String> {
    let msg = if msg.is_object() { msg } else { json!({}) }; // KIpc.msg: a missing message is {}
    let routed = super::canonical::route(path); let path = &routed; // main's copy, whichever thread named it
    let pro = pro_of(path);
    // the GUI KiCad is starting (or closing) for this project: no headless KiCad is started for it meanwhile, it would take the project's
    // lock first (KiCad's "File Open Warning" outside the pane, the release check's stall). One already running serves on until it is
    // retired (an agent's commit there finishes and is saved); else this waits for the editor and goes to it, or says to retry
    if !pro.is_empty() && super::embed::gui_starting(&pro) && !headless_active(&pro).await {
        let until = std::time::Instant::now() + GUI_WAIT;
        while super::embed::gui_starting(&pro) && super::embed::gui_route(&pro).is_none() && std::time::Instant::now() < until { tokio::time::sleep(Duration::from_millis(250)).await; }
        if super::embed::gui_route(&pro).is_none() && super::embed::gui_starting(&pro) { return Err(STARTING_IN_EDITOR.into()) }
    }
    // a headless KiCad is never started beside the editor's KiCad for the same project (it would hold the project's and the document's locks:
    // bp.13 opens over them, bp.12.4 asks outside the pane): the hub's own edits go through the editor's KiCad (embed::open_hidden)
    if !gui_first && !pro.is_empty() && super::embed::gui_route(&pro).is_some() && !headless_active(&pro).await {
        return Err(HEADLESS_BESIDE_EDITOR.into())
    }
    if let Some(gui) = (gui_first && !pro.is_empty()).then(|| super::embed::gui_route(&pro)).flatten() {
        let plain = check_gui(ty, path, &pro, &msg, &gui.caps);
        if !plain.is_empty() { return Err(plain) }
        if !super::known_root(hub, Path::new(&pro).parent().map(|p| p.to_str().unwrap_or("")).unwrap_or(""), "").await { return Err(format!("{path} is not in a project this hub knows")) }
        let setting = hub.setting("kicad.ipc").await.unwrap_or_else(|| "on".into());
        if setting == "off" { let why = "KiCad IPC is turned off in Settings"; log_fallback(hub, thread, why).await; return Err(ks::unavailable(why, &gui.cli)) }
        // the person's unsaved edits are never written by an agent's save (SaveDocument, SaveDocumentAs); its commits are tracked for that.
        // A co-editing runtime (bp.11, `coedit`) is different: the agent edits the person's live document, its save includes both
        // sides' edits on purpose, and nobody is asked to save, revert, close KiCad or wait (docs/kicad-coedit.md).
        let short = ty.rsplit('.').next().unwrap_or(ty);
        let coedit = gui.caps.iter().any(|c| c == "coedit");
        if !coedit && matches!(short, "SaveDocument" | "SaveDocumentAs") {
            let why = super::embed::unsaved_update(&pro, |u| u).refuse_save(Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or(path));
            if !why.is_empty() { return Err(why) }
        }
        if short == "BeginCommit" { super::embed::unsaved_update(&pro, |u| u.agent_begins()); }
        let commits = short == "EndCommit" && msg_commits(&msg);
        let actor = if coedit { super::coedit::client_name(&super::coedit::Who::Agent(thread.unwrap_or(""))) } else { String::new() };
        let mut held = None;
        if let Some(th) = thread.filter(|t| coedit && !t.is_empty()) {
            // what this thread works on, in the editor's Agents panel and as an area on the canvas
            if super::coedit::due(th) {
                let task = match crate::agents::handle() { Some(a) => a.thread_title(th).await, None => String::new() };
                let _ = super::embed::gui_call_as(&gui, "kiapi.backplane.coedit.SetAgentActivity", super::coedit::activity(th, &task, "working"), &actor).await;
            }
            // the person paused this agent in KiCad: its writes wait for "Resume" (reads go on)
            if !is_read(ty) { held = super::coedit::wait_resumed(th).await; }
        }
        let saves_board = short == "SaveDocument" && board_save(&msg, path);
        // never a save of a document KiCad says is unmodified: it would write only KiCad's own rewrite (its format, its load changes)
        let mut upgrade = None;
        if short == "SaveDocument" && msg["document"].is_object() {
            match save_gate(&gui, &msg, path, saves_board).await { Err(skip) => return Ok(skip), Ok(u) => upgrade = u }
        }
        let msg_doc = json!({"document": msg["document"].clone()});
        // KiCad's task gate (a check or a refill running) refuses all but re-entrant-safe reads: a read, and the plan tools' requests
        // (plan_get's GetPlan with derived data, plan_verify, plan_route; the Plan panel's reads), wait for the task and go again; a refusal
        // did nothing, so going again is safe. Past READ_TASK_WAIT the agent is told in words what KiCad is busy with and to ask again.
        let o = if patient_of(ty) { super::embed::gui_call_patient(&gui, ty, msg, &actor, super::embed::READ_TASK_WAIT).await } else { super::embed::gui_call_as(&gui, ty, msg, &actor).await };
        if let Some(note) = held { log_fallback(hub, thread, &note).await; }
        if commits && o["ok"].as_bool().unwrap_or(false) { super::embed::unsaved_update(&pro, |u| u.agent_committed()); }
        // the hub's own record of an agent's change not on disk (KiCad does not mark a document modified for it)
        if o["ok"].as_bool().unwrap_or(false) && thread.is_some_and(|t| !t.is_empty()) && writes_items(short, &msg_doc, commits) { super::embed::agent_changed(&pro, &doc_file(path, &pro)) }
        if o["fallback"].as_bool().unwrap_or(false) { log_fallback(hub, thread, o["why"].as_str().unwrap_or("")).await; return Err(o["error"].as_str().unwrap_or("").into()) }
        let ok = o["ok"].as_bool().unwrap_or(false);
        let servers = format!("{pro}|ready|gui ({})", gui.mode);
        if status {
            // KiCad's open dialogs and its notices (embed-notice: "Could not use OpenGL, falling back to software rendering"): the agent says them
            let st = super::embed::session_state(&pro);
            return Ok(json!({"server":format!("gui ({})", gui.mode),"servers":servers,"argv":gui.argv,"version":gui.version,"capabilities":o["message"].clone(),"detail":if ok { String::new() } else { err_of(&o) },
                "dialogs":st["dialogs"].clone(),"dialogs_reported":st["dialogs_reported"].clone(),"notices":st["notices"].clone()}))
        }
        if !ok { return Err(if short == "SaveDocument" { super::savegate::explain_save_error(&err_of(&o), super::savegate::saves_design(&msg_doc, path)) } else { err_of(&o) }) }
        let mut out = json!({"type":o["type"].as_str().unwrap_or(""),"message":o["message"].clone()});
        // an agent's save of a board KiCad changed on load (its 3D model migration): said, so the agent reports it rather than owns it
        if short == "SaveDocument" && super::savegate::wrote_nothing(&o) { out["not_saved"] = json!(super::savegate::NOTHING_CHANGED); }
        else if let Some(u) = upgrade { out["format_upgrade"] = json!(u); }
        if saves_board {
            if let Some((fp, zones)) = super::coedit::load_migration(&gui).await { out["kicad_load_changes"] = json!(format!("{}. They were saved with yours: tell the person, they are not your changes.", super::coedit::load_migration_words(fp, zones))); }
        }
        // what the person is doing in this KiCad right now, so the agent asks before working over them (docs/kicad-coedit.md)
        if thread.is_some_and(|t| !t.is_empty()) { if let Some(p) = super::coedit::person_now(&pro) { out["person"] = p; } }
        return Ok(out);
    }
    let env = runtime(hub).await;
    let plain = check(ty, path, &pro, &msg, &env.caps);
    if !plain.is_empty() { return Err(plain) }
    // the project must lie under a folder the hub knows (a deliberate difference from the JS hub, see the header)
    if !super::known_root(hub, Path::new(&pro).parent().map(|p| p.to_str().unwrap_or("")).unwrap_or(""), "").await { return Err(format!("{path} is not in a project this hub knows")) }
    let setting = hub.setting("kicad.ipc").await.unwrap_or_else(|| "on".into());
    let gate = ks::gate(&env.status, &env.version, &env.why, &setting);
    if !gate.is_empty() { log_fallback(hub, thread, &gate).await; return Err(ks::unavailable(&gate, &env.cli)) }
    let Some(i) = IPC.get() else { let why = "this Backplane has no KiCad IPC helper"; log_fallback(hub, thread, why).await; return Err(ks::unavailable(why, &env.cli)) };
    let msg_for_warning = msg.clone();
    let o = ask(i, &pro, &env, ty, msg).await;
    let lines = info_on(i).await; let argv = argv_on(i, &pro, &env).await;
    if o["fallback"].as_bool().unwrap_or(false) { log_fallback(hub, thread, o["why"].as_str().unwrap_or("")).await; return Err(o["error"].as_str().unwrap_or("").into()) }
    let ok = o["ok"].as_bool().unwrap_or(false);
    if status { return Ok(json!({"server":ks::note(&lines),"servers":lines,"argv":argv,"version":env.version,"capabilities":o["message"].clone(),"detail":if ok { String::new() } else { err_of(&o) }})) }
    if !ok { return Err(err_of(&o)) }
    let mut out = json!({"type":o["type"].as_str().unwrap_or(""),"message":o["message"].clone()});
    // a KiCad Open in KiCad started detached has this project open: it has no API server, so it does not show the agent's change, and
    // its next save would replace it. Said to the agent here and to the person in the thread (no silent overwrite trap)
    let short = ty.rsplit('.').next().unwrap_or(ty);
    if matches!(short, "SaveDocument" | "SaveDocumentAs") || (short == "EndCommit" && msg_commits(&msg_for_warning)) {
        if let Some((pid, file)) = super::open::detached_open(&pro) {
            let warning = super::open::detached_warning(&file);
            out["detached_kicad"] = json!(format!("{warning}. (KiCad pid {pid} has it open without Backplane's connection: tell the person; do not ask them to close it.)"));
            if let Some(th) = thread.filter(|t| !t.is_empty()) {
                let now = now_s() * 1000;
                let _ = hub.commit(&json!({"$":"ActivityLogged","thread":th,"act":format!("kicad-detached:{pro}"),"tone":"warn","kind":"kicad","summary":warning,"at":now})).await;
            }
        }
    }
    Ok(out)
}

/// How long a request waits for the editor's KiCad that is starting for its project, and what it is told after.
const GUI_WAIT: Duration = Duration::from_secs(if cfg!(test) { 1 } else { 90 });
pub const HEADLESS_BESIDE_EDITOR: &str = "the editor's KiCad has this project open: no second KiCad is started for it (it would hold the project's locks); the change goes through the editor's KiCad";
pub const STARTING_IN_EDITOR: &str = "KiCad is starting in the editor for this project: retry in a moment (no second KiCad is started meanwhile: it would hold the project's lock)";
/// Whether the project's headless KiCad is running (starting or ready).
async fn headless_active(pro: &str) -> bool {
    let Some(i) = IPC.get() else { return false };
    info_on(i).await.lines().any(|l| { let mut f = l.split('|'); f.next() == Some(pro) && f.next().is_some_and(ks::active) })
}

/// Whether an EndCommit request commits (CMA_COMMIT, the default) rather than drops.
fn msg_commits(msg: &Value) -> bool { !msg.to_string().contains("CMA_DROP") }
/// Commands that change items without a commit of their own (KiCad commits each by itself when no commit is open).
pub const ITEM_WRITES: &[&str] = &["CreateItems", "UpdateItems", "DeleteItems", "MoveItems", "PlaceSymbolFromLibrary", "PlaceFootprintFromLibrary", "ImportNetlist", "SetBoardDesignRules", "FlipItems", "InteractiveMoveItems"];
/// Whether this request leaves a change in the document: an EndCommit that commits, or an item write (inside a commit it is the
/// EndCommit's; outside one KiCad commits it itself, and a later EndCommit is not needed).
pub fn writes_items(short: &str, _doc: &Value, commits: bool) -> bool { (short == "EndCommit" && commits) || ITEM_WRITES.contains(&short) }
/// The design file a request on `path` changes: the file itself, or a project's board.
pub fn doc_file(path: &str, pro: &str) -> String {
    if path.ends_with(".kicad_pcb") || path.ends_with(".kicad_sch") { return path.to_string() }
    let f = super::project::read(&Path::new(pro).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default());
    if !f.pcb.is_empty() { f.pcb } else { f.sch }
}

/// Client RPC `kicad.retry` (Hub.kretry): failed servers are forgotten; the next request starts them.
pub struct IpcRpc;
impl RpcHandler for IpcRpc {
    fn methods(&self) -> &'static [&'static str] { &["kicad.retry"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _method: &'a str, _p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move { if let Some(i) = IPC.get() { let _ = i.tx.send(Msg::Clear); } vec![json!({"t":"reply","id":id,"ok":true})] })
    }
}

/// The version a ready server reports: its manifest's when it has one, else what KiCad said.
fn shown_version(manifest: &str, full: &str) -> String { if manifest.is_empty() { full } else { manifest }.to_string() }

#[cfg(test)]
mod tests {
    /// A stand-in kicad-cli that, like KiCad, writes its settings: to $KICAD_CONFIG_HOME when set, else to `person` (the person's
    /// kicad_common.json); it logs the folder it was given and answers --version and api-server --describe.
    #[cfg(unix)] fn writing_cli(dir: &std::path::Path, person: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let (cli, log) = (dir.join("kicad-cli"), dir.join("probe.log"));
        std::fs::write(&cli, format!("#!/bin/sh\necho \"$KICAD_CONFIG_HOME\" >> '{log}'\nif [ -n \"$KICAD_CONFIG_HOME\" ]; then mkdir -p \"$KICAD_CONFIG_HOME/10.0\"; echo rewritten > \"$KICAD_CONFIG_HOME/10.0/kicad_common.json\"; else echo rewritten > '{person}'; fi\nif [ \"$1\" = api-server ]; then echo '{{\"version\":\"10.99.0-bp.13\"}}'; else echo 10.0.8; fi\n", log = log.display(), person = person.display())).unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        (cli, log)
    }

    /// Every probe of a KiCad (start-up and Settings > CAD, an agent's KiCad selection, a check run's match) runs with its settings in a
    /// throwaway folder: the person's kicad_common.json is byte-identical after it, and the folder is gone.
    #[cfg(unix)] #[test] fn a_probe_never_writes_the_persons_kicad_settings() {
        let t = crate::plat::tmp::dir("kprobe");
        let person = t.path().join("kicad_common.json"); std::fs::write(&person, "{\"mine\": 1}").unwrap();
        let (cli, log) = writing_cli(t.path(), &person);
        let out = cli_version_output(&cli.to_string_lossy()).expect("ran");
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "10.0.8", "the probe still says the version");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let v = rt.block_on(async { let (mut c, _h) = crate::proc::kicad_probe_tokio(&cli); c.arg("--version").output().await.unwrap() });
        assert_eq!(String::from_utf8_lossy(&v.stdout).trim(), "10.0.8");
        assert_eq!(std::fs::read_to_string(&person).unwrap(), "{\"mine\": 1}", "the person's settings are untouched");
        let given: Vec<String> = std::fs::read_to_string(&log).unwrap().lines().map(String::from).collect();
        assert_eq!(given.len(), 2, "{given:?}");
        for d in &given { assert!(!d.is_empty() && d.contains("bp-kicad-probe-"), "a throwaway folder: {d:?}"); assert!(!std::path::Path::new(d).exists(), "removed after: {d}"); }
        assert_ne!(given[0], given[1], "one folder per probe");
    }

    /// #1160 (macOS): some `kicad-cli --version` runs never exit (KiCad waits on its Python lookup's child) and ignore SIGTERM; they piled
    /// up under the hub. A stand-in that hangs the same way on its first run: that run is killed and reaped at the limit, the probe asks once
    /// more and gets the version; one that always hangs gives None in bounded time and leaves no process behind.
    #[cfg(unix)] #[test] fn a_probe_that_hangs_is_killed_reaped_and_asked_again() {
        use std::os::unix::fs::PermissionsExt;
        let t = crate::plat::tmp::dir("kprobe-hang");
        let (cli, pids, once) = (t.path().join("kicad-cli"), t.path().join("pids"), t.path().join("hung-once"));
        // the first run (or every run with ALWAYS) records its pid and hangs as the stuck probes did: SIGTERM ignored, the same pid (exec)
        std::fs::write(&cli, format!("#!/bin/sh\necho $$ >> '{p}'\nif [ ! -e '{o}' ] || [ -e '{o}.always' ]; then touch '{o}'; trap '' TERM; exec sleep 60; fi\necho 10.99.0\n", p = pids.display(), o = once.display())).unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        let gone = |pid: &str| { let r = std::process::Command::new("kill").args(["-0", pid]).stderr(std::process::Stdio::null()).status().unwrap(); !r.success() };
        let limit = std::time::Duration::from_millis(1500);
        let at = std::time::Instant::now();
        let out = cli_version_within(&cli.to_string_lossy(), limit).expect("the second run answers");
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "10.99.0");
        assert!(at.elapsed() < limit * 2 + std::time::Duration::from_secs(5), "bounded: {:?}", at.elapsed());
        let ran: Vec<String> = std::fs::read_to_string(&pids).unwrap().lines().map(String::from).collect();
        assert_eq!(ran.len(), 2, "asked twice: {ran:?}");
        assert!(gone(&ran[0]), "the hung run is killed and reaped, not left alive or a zombie: pid {}", ran[0]);

        std::fs::write(once.with_extension("always"), "").unwrap(); std::fs::remove_file(&pids).unwrap();
        let at = std::time::Instant::now();
        assert!(cli_version_within(&cli.to_string_lossy(), limit).is_none(), "a KiCad that always hangs has no version");
        assert!(at.elapsed() < limit * 2 + std::time::Duration::from_secs(5), "bounded: {:?}", at.elapsed());
        let ran: Vec<String> = std::fs::read_to_string(&pids).unwrap().lines().map(String::from).collect();
        assert_eq!(ran.len(), 2, "{ran:?}"); for p in &ran { assert!(gone(p), "pid {p} left behind"); }
    }

    #[test] fn the_backplane_library_is_said_by_its_bundle_label_and_short_commit() {
        let m = serde_json::json!({"version": "10.99.0-bp.13", "backplaneLibrary": {"repository": "https://github.com/BackplaneWorks/Backplane_KiCad_Library.git", "version": "2026.11.13", "commit": "314532f925fef0becc5a6eaf6be3b000f41cb0d0", "sha256": "x"}});
        assert_eq!(library_label(&m), "2026.11.13 (314532f)");
        assert_eq!(library_label(&serde_json::json!({"version": "10.99.0-bp.12.4"})), "", "no pin before bp.13");
        assert_eq!(library_label(&serde_json::json!({"backplaneLibrary": {"commit": "314532f925"}})), "314532f");
    }

    /// An agent's save: not sent for an unmodified document, nor for a board whose only change is KiCad's load-time migration; else sent.
    #[tokio::test] async fn an_agents_save_writes_only_real_changes() {
        use super::super::embed::stand_in_route;
        let serve = |state: &'static str, log: Value| { let (route, mut rx) = stand_in_route(&format!("/p/sg-{state}.kicad_pro"));
            tokio::spawn(async move { while let Some((ty, _, reply)) = rx.recv().await {
                let _ = reply.send(if ty.ends_with("GetDocumentModifiedState") { json!({"ok": true, "message": {"state": state}}) }
                    else if ty.ends_with("GetChanges") { json!({"ok": true, "message": log.clone()}) }
                    else { json!({"ok": true, "message": {}}) }); } });
            route };
        let doc = json!({"document": {"type": "DOCTYPE_PCB"}});
        let mig = json!({"changes": [{"description": "Auto-migrate 3D Model References", "modified": [{"value": "a"}], "created": [], "deleted": []}]});
        let r = serve("DMS_UNMODIFIED", json!({})); assert!(save_gate(&r, &doc, "/p/x.kicad_pcb", true).await.unwrap_err()["not_saved"].as_str().unwrap().starts_with("nothing to save"));
        let r = serve("DMS_MODIFIED", mig.clone()); assert!(save_gate(&r, &doc, "/p/x.kicad_pcb", true).await.unwrap_err()["not_saved"].as_str().unwrap().contains("only change since it opened"));
        let mut both = mig.clone(); both["changes"].as_array_mut().unwrap().push(json!({"description": "Move", "actor": "agent:t1"}));
        let r = serve("DMS_MODIFIED", both); assert!(save_gate(&r, &doc, "/p/x.kicad_pcb", true).await.is_ok(), "a real change goes, KiCad's with it (said)");
        let r = serve("DMS_MODIFIED", mig); assert!(save_gate(&r, &json!({"document": {"type": "DOCTYPE_SCHEMATIC"}}), "/p/x.kicad_sch", false).await.is_ok(), "a schematic: the board's migration is not its");
    }
    #[test] fn runaction_guidance_follows_the_capability() {
        assert!(super::runaction_guidance(&["runaction-frame".to_string()]).contains("FT_SCHEMATIC_EDITOR"));
        assert!(super::runaction_guidance(&[]).contains("goes to the board editor while one is open"));
    }
    #[test] fn status_shows_the_manifests_version_and_kicads_own_only_without_one() {
        assert_eq!(shown_version("10.99.0-bp.13", "backplane-kicad-v10.99.0-bp.7-15-g8d903542-dirty-bp.13"), "10.99.0-bp.13");
        assert_eq!(shown_version("", "10.99.0-bp.13"), "10.99.0-bp.13");
    }
    /// A managed install on Windows has `bin\kicad-cli.exe`: the newest version's is found, with a space in the folder name.
    /// An update installs beside the old runtime: a hotfix (bp.12.1) is taken over its release (bp.12) and older ones, whatever the folder order.
    #[test] fn the_hotfix_runtime_is_taken_over_the_release_it_fixes() {
        let d = crate::plat::tmp::dir("bp kicad hotfix").join("kicad");
        for v in ["10.99.0-bp.12", "10.99.0-bp.10", "10.99.0-bp.12.1", "10.99.0-bp.12-851cbefda6", "10.99.0-bp.11"] { std::fs::create_dir_all(d.join(v).join("bin")).unwrap(); std::fs::write(d.join(v).join("bin/kicad-cli"), "").unwrap(); std::fs::write(d.join(v).join("bin/kicad-cli.exe"), "MZ").unwrap(); }
        let found = newest_in(std::slice::from_ref(&d)).into_iter().find(|p| p.is_file()).unwrap();
        assert!(found.to_string_lossy().contains("10.99.0-bp.12.1"), "{found:?}");
    }
    #[test] fn a_managed_runtime_is_found_by_its_exe_name_in_a_folder_with_spaces() {
        let base = crate::plat::tmp::dir("bp kicad"); let d = base.join("kicad");
        for v in ["10.99.0-bp.4", "10.99.0-bp.6"] { std::fs::create_dir_all(d.join(v).join("bin")).unwrap(); std::fs::write(d.join(v).join("bin/kicad-cli.exe"), "MZ").unwrap(); }
        let found = newest_in(std::slice::from_ref(&d)).into_iter().find(|p| p.is_file()).unwrap();
        assert!(found.ends_with("10.99.0-bp.6/bin/kicad-cli.exe"), "{found:?}");
    }
    use super::*;
    /// bp.4's kicad-cli --version prints `10.99.0`: its manifest says what it is, so Settings > CAD shows ok and 10.99.0-bp.4, not stock
    #[test] fn reads_and_the_plans_commands_wait_out_kicads_task() {
        assert!(patient_of("kiapi.backplane.plan.GetPlan") && patient_of("kiapi.backplane.plan.VerifyPlan") && patient_of("kiapi.backplane.plan.RoutePlan"));
        assert!(patient_of("kiapi.board.commands.GetItems") && patient_of("kiapi.common.commands.Ping"));
        assert!(!patient_of("kiapi.common.commands.CreateItems") && !patient_of("kiapi.backplane.plan.ApprovePlan"));
    }
    #[test] fn cad_status_trusts_a_backplane_manifest() {
        let s = |a: &str, b: &str| (a.to_string(), b.to_string());
        let bp4 = Env { cli: "/k/bin/kicad-cli".into(), version: "10.99.0-bp.4".into(), status: "ok".into(), api: 2, ..Default::default() };
        assert_eq!(cad_status(&bp4, Some("10.99.0")), s("ok", "10.99.0-bp.4"));
        assert_eq!(cad_status(&bp4, None), s("broken", ""));
        let stock = Env { cli: "/usr/bin/kicad-cli".into(), status: "stock".into(), ..Default::default() };
        assert_eq!(cad_status(&stock, Some("10.0.7")), s("stock", "10.0.7"));
        assert_eq!(cad_status(&Env::default(), Some("x")), s("missing", ""));
        let old = Env { cli: "/k".into(), version: "10.99.0-bp.2".into(), status: "old".into(), api: 2, ..Default::default() };
        assert_eq!(cad_status(&old, Some("10.99.0")), s("old", "10.99.0-bp.2"));
    }
    #[test] fn check_orders_like_kipc_check() {
        assert!(check("", "/p/a.kicad_pro", "/p/a.kicad_pro", &json!({}), &[]).starts_with("type is required"));
        assert!(check("kiapi.board.commands.GetItems", "rel.kicad_pro", "x", &json!({}), &[]).starts_with("path must be the absolute path"));
        assert_eq!(check("kiapi.board.commands.GetItems", "/p/x.kicad_pcb", "", &json!({}), &[]), "no KiCad project (.kicad_pro) found for /p/x.kicad_pcb");
        assert_eq!(check("kiapi.common.commands.GetSelection", "/p/a.kicad_pro", "/p/a.kicad_pro", &json!({}), &[]), ks::REFUSAL);
        assert_eq!(check("kiapi.board.commands.GetItems", "/p/a.kicad_pro", "/p/a.kicad_pro", &json!({}), &[]), "");
    }
    #[test] fn err_words() {
        assert_eq!(err_of(&json!({"status":"AS_BAD_REQUEST","error":"nope"})), "KiCad answered AS_BAD_REQUEST: nope"); assert_eq!(err_of(&json!({})), "KiCad did not answer"); assert_eq!(err_of(&json!({"error":"x"})), "x");
        assert_eq!(failed("w")["fallback"], json!(true));
    }
    #[test] fn fab_output_is_its_projects() {
        let d = crate::plat::tmp::dir("pro-fab"); std::fs::create_dir_all(d.join("mfg/b/gerber")).unwrap();
        std::fs::write(d.join("b.kicad_pro"), "{}").unwrap(); std::fs::write(d.join("mfg/b/gerber/b-F_Cu.gtl"), "x").unwrap();
        let want = d.join("b.kicad_pro").to_string_lossy().into_owned();
        assert_eq!(pro_of(&d.join("mfg/b/gerber").to_string_lossy()), want);
        assert_eq!(pro_of(&d.join("mfg/b/gerber/b-F_Cu.gtl").to_string_lossy()), want);
        assert_eq!(pro_of(&d.join("mfg/b").to_string_lossy()), "", "a folder with no fab files is no set");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn pro_paths() { assert_eq!(pro_of("/p/a.kicad_pro"), "/p/a.kicad_pro"); assert_eq!(pro_of("a.kicad_pro"), ""); }

    /// The whole path through the actor on the default transport, no Bun: a request starts the server, waits while it starts,
    /// Ping and GetVersion hand it over, and only then the request goes out; its clean exit leaves the record stopped.
    #[cfg(unix)]
    #[tokio::test] async fn actor_over_the_rust_transport() {
        use super::super::nng::tests as stand_in;
        use crate::plat::fs::PermissionsExt;
        // the real server's answer to GetVersion (bp.4), and one with no message for what is not asked about
        let version: Vec<u8> = { let h = "0a040a02746b120208011a5c0a3c747970652e676f6f676c65617069732e636f6d2f6b696170692e636f6d6d6f6e2e636f6d6d616e64732e47657456657273696f6e526573706f6e7365121c0a1a080a1063221431302e39392e302d756e6b6e6f776e2d62702e34"; (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap()).collect() };
        let root = crate::plat::tmp::short_dir("ia"); // short: a unix socket path must fit SUN_LEN (104 on macOS)
        let (rt, project) = (root.to_string_lossy().to_string(), "/p/a.kicad_pro".to_string());
        let dir = ks::dir(&rt, "", &project); std::fs::create_dir_all(&dir).unwrap();
        let sock = format!("{dir}/api.sock"); let l = crate::plat::ipc::bind(&sock).unwrap();
        let cli = format!("{rt}/kicad-cli"); std::fs::write(&cli, "#!/bin/sh\necho \"KiCad API server listening at ipc://$4\"\nsleep 1\n").unwrap(); std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        let answered = version.clone();
        let srv = tokio::spawn(async move { // Ping, GetVersion, then the request itself
            let (mut s, _) = stand_in::accept(&l).await; let mut asked = vec![];
            for _ in 0..3 { let (id, b) = stand_in::read_req(&mut s).await.unwrap(); asked.push(String::from_utf8_lossy(&b).to_string()); stand_in::write_rep(&mut s, id, &answered).await }
            asked
        });
        let (tx, mut rx) = mpsc::unbounded_channel(); let lines = Arc::new(std::sync::Mutex::new(String::new()));
        let mut a = Actor { me: tx.clone(), home: String::new(), rt, hub_pid: std::process::id(), helper: None, gen: 0, pend: HashMap::new(), next: 0, srvs: vec![], wait: vec![], shown: String::new(), env: Env::default(), lines: lines.clone(),
        uses: HashMap::new(), pend_of: HashMap::new(), procs: HashMap::new(), cur: HashMap::new(), proc_n: 0, relaunch: vec![], ticking: false, pool: pool_of(None, ""), trimming: false, stopping: vec![] };
        tokio::spawn(async move { while let Some(m) = rx.recv().await { a.handle(m) } });
        let env = Env { cli, version: "10.99.0-bp.4".into(), ..Default::default() };
        let (t, r) = oneshot::channel();
        tx.send(Msg::Ask { project: project.clone(), env, ty: "kiapi.common.commands.GetVersion".into(), msg: json!({}), reply: t }).unwrap();
        let o = tokio::time::timeout(Duration::from_secs(10), r).await.expect("an answer in time").unwrap();
        assert_eq!(o["ok"], json!(true), "{o}"); assert_eq!(o["message"]["version"]["full_version"], json!("10.99.0-unknown-bp.4"));
        let asked = srv.await.unwrap();
        assert!(asked[0].ends_with("commands.Ping") && asked[1].ends_with("commands.GetVersion") && asked[2].ends_with("commands.GetVersion"), "{asked:?}");
        assert_eq!(lines.lock().unwrap().clone(), format!("{project}|ready|"));
        for _ in 0..40 { tokio::time::sleep(Duration::from_millis(100)).await; if lines.lock().unwrap().contains("|stopped|") { break } }
        assert_eq!(lines.lock().unwrap().clone(), format!("{project}|stopped|"));
    }
    /// A stand-in for the transport and the KiCads behind it: a spawn listens at once, Ping/GetVersion/Shutdown answer, RunSimulation
    /// is kept unanswered until the test lets it go (a run in flight), and a server told to shut down exits at once or, lingering, when told.
    #[derive(Default)] struct Crowd { socks: HashMap<String, String>, held: Vec<Value>, spawned: Vec<String>, shut: Vec<String>, linger: bool, lingering: Vec<String> }
    struct CrowdLink { st: Arc<std::sync::Mutex<Crowd>>, me: mpsc::UnboundedSender<Msg>, gen: u64 }
    impl Transport for CrowdLink {
        fn send(&mut self, f: Value) -> bool {
            let mut st = self.st.lock().unwrap(); let id = f["id"].clone(); let say = |o: Value| { let _ = self.me.send(Msg::Line(self.gen, o)); };
            match f["op"].as_str() {
                Some("spawn") => { let key = f["key"].as_str().unwrap().to_string(); let sock = format!("/s/{}", key.replace(['/', '\t'], "_")); st.socks.insert(sock.clone(), key.clone()); st.spawned.push(key.clone());
                    say(json!({"id": id, "ok": true})); say(json!({"event": "line", "key": key, "text": format!("KiCad API server listening at ipc://{sock}")})) }
                Some("close") => say(json!({"id": id, "ok": true})),
                _ => { let (ty, sock) = (f["type"].as_str().unwrap_or("").rsplit('.').next().unwrap().to_string(), f["sock"].as_str().unwrap_or("").to_string());
                    let Some(key) = st.socks.get(&sock).cloned() else { say(json!({"id": id, "ok": false, "error": "KiCad closed the connection before answering; request outcome unknown"})); return true };
                    match ty.as_str() {
                        "Held" => st.held.push(id),
                        "Shutdown" => { st.shut.push(key.clone()); say(json!({"id": id, "ok": true, "message": {}}));
                            if st.linger { st.lingering.push(key) } else { st.socks.remove(&sock); say(json!({"event": "exit", "key": key, "code": 0, "tail": ""})) } }
                        _ => say(json!({"id": id, "ok": true, "type": ty, "message": {"version": {"full_version": "x-bp.4"}, "from": key}})),
                    } }
            }
            true
        }
    }
    fn crowd_actor(room: usize) -> (Actor, mpsc::UnboundedReceiver<Msg>, Arc<std::sync::Mutex<Crowd>>, Env) {
        let (tx, rx) = mpsc::unbounded_channel(); let st = Arc::new(std::sync::Mutex::new(Crowd::default()));
        let a = Actor { me: tx.clone(), home: String::new(), rt: crate::plat::tmp::dir("ipc-crowd").path().to_string_lossy().into_owned(), hub_pid: 1, helper: Some(Helper { link: Box::new(CrowdLink { st: st.clone(), me: tx.clone(), gen: 1 }), gen: 1 }), gen: 1, pend: HashMap::new(), next: 0,
            srvs: vec![], wait: vec![], shown: String::new(), env: Env::default(), lines: Arc::new(std::sync::Mutex::new(String::new())),
            uses: HashMap::new(), pend_of: HashMap::new(), procs: HashMap::new(), cur: HashMap::new(), proc_n: 0, relaunch: vec![], ticking: false, pool: Pool { room, ..pool_of(None, "") }, trimming: false, stopping: vec![] };
        (a, rx, st, Env { cli: "/rt/bin/kicad-cli".into(), version: "10.99.0-bp.4".into(), caps: vec!["shutdown".into()], ..Default::default() })
    }
    fn crowd_of(room: usize) -> (mpsc::UnboundedSender<Msg>, Arc<std::sync::Mutex<Crowd>>, Env) {
        let (mut a, mut rx, st, env) = crowd_actor(room); let tx = a.me.clone();
        tokio::spawn(async move { while let Some(m) = rx.recv().await { a.handle(m) } });
        (tx, st, env)
    }
    fn crowd() -> (mpsc::UnboundedSender<Msg>, Arc<std::sync::Mutex<Crowd>>, Env) { crowd_of(2) }
    fn asks(tx: &mpsc::UnboundedSender<Msg>, env: &Env, project: &str, ty: &str) -> oneshot::Receiver<Value> {
        let (t, r) = oneshot::channel(); tx.send(Msg::Ask { project: project.into(), env: env.clone(), ty: format!("kiapi.x.{ty}"), msg: json!({}), reply: t }).unwrap(); r
    }
    async fn within(secs: f64, r: oneshot::Receiver<Value>) -> Value { tokio::time::timeout(Duration::from_secs_f64(secs), r).await.expect("an answer in time").unwrap() }
    async fn until(what: &str, f: impl Fn() -> bool) { for _ in 0..400 { if f() { return } tokio::time::sleep(Duration::from_millis(10)).await } panic!("{what}") }
    /// Room for two servers and three projects: the one a run is in flight on is never the one stopped for the third (it lost the
    /// run and its caller the answer: "KiCad closed the connection"); the third is served by stopping the idle one; with both at
    /// work it waits for one to fall idle, and past the limit is told so instead of waiting for ever.
    #[tokio::test] async fn a_server_at_work_is_never_stopped_to_make_room() {
        let (tx, st, env) = crowd();
        let run = asks(&tx, &env, "/p/a.kicad_pro", "Held");
        until("the run is out", || st.lock().unwrap().held.len() == 1).await;
        assert_eq!(within(2.0, asks(&tx, &env, "/p/b.kicad_pro", "GetVersion")).await["message"]["from"], json!("/p/b.kicad_pro\t2"));
        // b falls idle; a's run is still out: c takes b's place, and a is never asked to stop
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(within(3.0, asks(&tx, &env, "/p/c.kicad_pro", "GetVersion")).await["message"]["from"], json!("/p/c.kicad_pro\t3"));
        assert_eq!(st.lock().unwrap().shut, vec!["/p/b.kicad_pro\t2".to_string()]);
        // both at work (a's run, c just asked): b waits for c to fall idle, then is started again and answered
        assert_eq!(within(5.0, asks(&tx, &env, "/p/b.kicad_pro", "GetVersion")).await["message"]["from"], json!("/p/b.kicad_pro\t4"));
        assert_eq!(st.lock().unwrap().shut, vec!["/p/b.kicad_pro\t2".to_string(), "/p/c.kicad_pro\t3".to_string()]);
        // a second run, on b: now neither falls idle, and c is told so once the limit has passed (never left waiting)
        let run_b = asks(&tx, &env, "/p/b.kicad_pro", "Held");
        until("b's run is out", || st.lock().unwrap().held.len() == 2).await;
        let o = within(8.0, asks(&tx, &env, "/p/c.kicad_pro", "GetVersion")).await;
        assert!(o["ok"] == false && o["error"].as_str().unwrap().contains(ROOM_BUSY), "{o}");
        // the runs' own answers come when KiCad gives them: nothing dropped them
        let held = st.lock().unwrap().held.clone();
        for id in held { tx.send(Msg::Line(1, json!({"id": id, "ok": true, "message": {"status": "SIM_RUNNING"}}))).unwrap() }
        assert_eq!(within(2.0, run).await["message"]["status"], json!("SIM_RUNNING")); assert_eq!(within(2.0, run_b).await["message"]["status"], json!("SIM_RUNNING"));
        assert_eq!(st.lock().unwrap().spawned.len(), 4);
    }
    /// A server told to stop that has not exited yet: its project asked for again starts no second process beside it (two KiCads on one
    /// socket and one project's lock; the old one's exit then took the new one for stopped). The start waits for the exit.
    #[tokio::test] async fn a_project_is_not_started_again_beside_its_last_process() {
        let (tx, st, env) = crowd(); st.lock().unwrap().linger = true;
        for p in ["a", "b"] { assert_eq!(within(2.0, asks(&tx, &env, &format!("/p/{p}.kicad_pro"), "GetVersion")).await["ok"], json!(true)) }
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(within(3.0, asks(&tx, &env, "/p/c.kicad_pro", "GetVersion")).await["ok"], json!(true)); // a (the least recently used) was told to stop, and lingers
        assert_eq!(st.lock().unwrap().lingering, vec!["/p/a.kicad_pro\t1".to_string()]);
        tokio::time::sleep(Duration::from_millis(1100)).await;
        let again = asks(&tx, &env, "/p/a.kicad_pro", "GetVersion");
        until("room is made for a", || st.lock().unwrap().shut.len() == 2).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(st.lock().unwrap().spawned.iter().filter(|k| k.starts_with("/p/a.")).count(), 1, "no second process while the first is alive");
        // the old process's exit: only now the new one, and the request is its (never "the headless KiCad exited")
        tx.send(Msg::Line(1, json!({"event": "exit", "key": "/p/a.kicad_pro\t1", "code": 0, "tail": ""}))).unwrap();
        let o = within(3.0, again).await; assert_eq!(o["ok"], json!(true), "{o}");
        assert!(o["message"]["from"].as_str().unwrap().starts_with("/p/a.kicad_pro\t") && o["message"]["from"] != json!("/p/a.kicad_pro\t1"), "{o}");
    }
    /// A project asked for in the moment its idle server is being stopped for another: the request is not sent to the server that is
    /// about to exit (it was lost with it), it is answered by the project's next process.
    #[tokio::test] async fn a_request_for_a_server_being_stopped_waits_for_the_next_one() {
        let (tx, st, env) = crowd();
        for p in ["a", "b"] { assert_eq!(within(2.0, asks(&tx, &env, &format!("/p/{p}.kicad_pro"), "GetVersion")).await["ok"], json!(true)) }
        tokio::time::sleep(Duration::from_millis(2100)).await; // both idle
        let c = asks(&tx, &env, "/p/c.kicad_pro", "GetVersion"); let a = asks(&tx, &env, "/p/a.kicad_pro", "GetVersion"); // a is the one stopped for c, and asked for at once
        assert_eq!(within(3.0, c).await["message"]["from"], json!("/p/c.kicad_pro\t3"));
        let o = within(6.0, a).await; assert_eq!((o["ok"].clone(), o["message"]["from"].clone()), (json!(true), json!("/p/a.kicad_pro\t4")), "{o}");
        assert_eq!(st.lock().unwrap().spawned.iter().filter(|k| k.starts_with("/p/a.")).count(), 2);
    }
    /// A server that dies with a request out: the request is answered at once with what happened, and the project does not count as
    /// in use for ever after (it was never the one stopped again, so a third project got no server).
    #[tokio::test] async fn a_server_that_dies_mid_request_is_not_in_use_for_ever() {
        let (tx, st, env) = crowd();
        let out = asks(&tx, &env, "/p/a.kicad_pro", "Held");
        until("a's request is out", || st.lock().unwrap().held.len() == 1).await;
        tx.send(Msg::Line(1, json!({"event": "exit", "key": "/p/a.kicad_pro\t1", "code": 137, "tail": "Killed"}))).unwrap();
        let o = within(1.0, out).await; assert!(o["ok"] == false && o["error"].as_str().unwrap().contains("exited before answering"), "{o}");
        // a starts again by itself (a crash is retried) and b is at work: c can only have a's place, a moment after a fell idle
        until("a is started again", || st.lock().unwrap().spawned.len() == 2).await;
        let _b = asks(&tx, &env, "/p/b.kicad_pro", "Held");
        until("b's request is out", || st.lock().unwrap().held.len() == 2).await;
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(within(2.5, asks(&tx, &env, "/p/c.kicad_pro", "GetVersion")).await["ok"], json!(true));
        assert_eq!(st.lock().unwrap().shut, vec!["/p/a.kicad_pro\t2".to_string()]);
    }
    /// The transport restarted with a request out: its caller is answered (the old transport's reply would never be taken), and
    /// nothing counts as in use because of it.
    #[tokio::test] async fn a_transport_restarted_mid_request_leaves_nothing_in_use() {
        let (mut a, mut rx, st, env) = crowd_actor(2);
        let pump = |a: &mut Actor, rx: &mut mpsc::UnboundedReceiver<Msg>| while let Ok(m) = rx.try_recv() { a.handle(m) };
        let ask = |a: &Actor, p: &str, ty: &str| { let (t, r) = oneshot::channel(); a.me.send(Msg::Ask { project: p.into(), env: env.clone(), ty: format!("kiapi.x.{ty}"), msg: json!({}), reply: t }).unwrap(); r };
        let out = ask(&a, "/p/a.kicad_pro", "Held"); let mut b = ask(&a, "/p/b.kicad_pro", "GetVersion");
        for _ in 0..50 { pump(&mut a, &mut rx); tokio::time::sleep(Duration::from_millis(10)).await }
        assert_eq!((a.uses.get("/p/a.kicad_pro").copied(), b.try_recv().unwrap()["ok"].clone(), st.lock().unwrap().held.len()), (Some(1), json!(true), 1));
        a.start_helper(); let gen = a.gen; a.helper = Some(Helper { link: Box::new(CrowdLink { st: st.clone(), me: a.me.clone(), gen }), gen });
        let o = within(1.0, out).await; assert!(o["ok"] == false && o["error"].as_str().unwrap().contains("helper restarted"), "{o}");
        assert!(a.uses.is_empty() && a.pend_of.is_empty() && a.pend.is_empty());
        // the old transport's late reply is nobody's; a third project gets a place once the two are idle
        a.handle(Msg::Line(1, json!({"id": 1, "ok": true, "message": {}})));
        tokio::time::sleep(Duration::from_millis(1100)).await;
        let mut c = ask(&a, "/p/c.kicad_pro", "GetVersion");
        for _ in 0..100 { pump(&mut a, &mut rx); tokio::time::sleep(Duration::from_millis(10)).await }
        assert_eq!(c.try_recv().unwrap()["ok"], json!(true)); assert_eq!(st.lock().unwrap().shut, vec!["/p/a.kicad_pro\t1".to_string()]);
    }
    /// Two projects wait for room: the one that asked first gets the first place, whatever the number of ticks between.
    #[tokio::test] async fn those_waiting_for_room_are_served_in_the_order_they_asked() {
        let (tx, st, env) = crowd();
        let _a = asks(&tx, &env, "/p/a.kicad_pro", "Held"); let _b = asks(&tx, &env, "/p/b.kicad_pro", "Held");
        until("both are at work", || st.lock().unwrap().held.len() == 2).await;
        let c = asks(&tx, &env, "/p/c.kicad_pro", "GetVersion"); tokio::time::sleep(Duration::from_millis(50)).await; let mut d = asks(&tx, &env, "/p/d.kicad_pro", "GetVersion");
        tokio::time::sleep(Duration::from_millis(400)).await; // (a tick passes with both waiting)
        tokio::time::sleep(Duration::from_millis(700)).await; // (and another)
        let first = st.lock().unwrap().held[0].clone(); tx.send(Msg::Line(1, json!({"id": first, "ok": true, "message": {}}))).unwrap(); // a's request ends: a falls idle a second later
        assert_eq!(within(2.5, c).await["message"]["from"], json!("/p/c.kicad_pro\t3"));
        assert!(d.try_recv().is_err(), "d asked after c and is still waiting"); assert!(!st.lock().unwrap().spawned.iter().any(|k| k.starts_with("/p/d.")));
    }
    /// A machine with room for three: the third project starts beside the two, nothing is stopped for it; the one beyond two that
    /// stays idle is shut down, and the two stay.
    #[tokio::test] async fn room_beyond_two_is_used_and_given_back_when_idle() {
        let (tx, st, env) = crowd_of(3);
        for p in ["a", "b", "c"] { assert_eq!(within(2.0, asks(&tx, &env, &format!("/p/{p}.kicad_pro"), "GetVersion")).await["ok"], json!(true)) }
        assert!(st.lock().unwrap().shut.is_empty() && st.lock().unwrap().spawned.len() == 3);
        until("the idle spare is shut down", || st.lock().unwrap().shut.len() == 1).await;
        tokio::time::sleep(Duration::from_millis(2500)).await;
        assert_eq!(st.lock().unwrap().shut, vec!["/p/a.kicad_pro\t1".to_string()], "the least recently used, and only it");
    }
    #[test] fn a_copy_runs_only_the_listed_kicad_reads() {
        for t in ["kiapi.board.commands.GetItems", "kiapi.common.commands.Ping", "kiapi.backplane.GetCapabilities"] { assert_eq!(copy_ipc_refused(true, t), None, "{t}") }
        assert!(approval_refused("kiapi.backplane.plan.ApprovePlan", &json!({"layer": "F.Cu", "approve": true})).is_some());
        for a in ["pcbnew.Plan.accept", "pcbnew.Plan.reject", "pcbnew.Plan.approveLayer", "pcbnew.Plan.rejectLayer"] { assert!(approval_refused("kiapi.common.commands.RunAction", &json!({"action": a})).is_some(), "{a}") }
        for a in ["pcbnew.Plan.togglePlanView", "pcbnew.Plan.toggleLock", "common.Control.zoomFitScreen"] { assert_eq!(approval_refused("kiapi.common.commands.RunAction", &json!({"action": a})), None, "{a}") }
        assert_eq!(approval_refused("kiapi.backplane.plan.GetPlan", &json!({})), None);
        assert!(board_save(&json!({"document": {"type": "DOCTYPE_PCB"}}), "/p/x.kicad_sch") && !board_save(&json!({"document": {"type": "DOCTYPE_SCHEMATIC"}}), "/p/x.kicad_pcb"));
        assert!(board_save(&json!({}), "/p/x.kicad_pcb") && !board_save(&json!({}), "/p/x.kicad_pro"));
        for t in ["kiapi.common.commands.SaveDocument", "kiapi.board.commands.CreateItems", "kiapi.board.commands.BeginCommit", "kiapi.board.commands.EndCommit", "kiapi.backplane.Shutdown", "", "GetItemsX.Delete"] { assert!(copy_ipc_refused(true, t).is_some(), "{t}") }
        assert_eq!(copy_ipc_refused(false, "kiapi.common.commands.SaveDocument"), None);
    }
}

/// Whether `path`'s project is open in a co-editing KiCad (bp.11 `coedit`): agents write its live documents through IPC at once,
/// with no document lock (the fork defers what the person holds and merges the rest), and never ask the person to wait.
pub fn coedit_live(path: &str) -> bool {
    let pro = pro_of(&super::canonical::route(path));
    !pro.is_empty() && super::embed::gui_route(&pro).is_some_and(|g| g.caps.iter().any(|c| c == "coedit"))
}

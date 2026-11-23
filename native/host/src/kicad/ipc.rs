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

pub const REQUEST_TIMEOUT_MS: u64 = 30_000;
pub const BEGIN_COMMIT_TIMEOUT_MS: u64 = 15_000;
pub const ASK_TIMEOUT: Duration = Duration::from_secs(90);
const PROBE_PING_TIMEOUT_MS: u64 = 3_000;

/// Timeout in milliseconds for an IPC command (`BeginCommit` is an in-memory sub-millisecond op and gets a tighter bound).
pub fn timeout_ms_for(ty: &str) -> u64 {
    if ty.rsplit('.').next() == Some("BeginCommit") { BEGIN_COMMIT_TIMEOUT_MS } else { REQUEST_TIMEOUT_MS }
}

/// True when `reply` is a transport/dial connection error (not a KiCad API status refusal).
pub fn is_conn_error(reply: &Value) -> bool {
    if reply["ok"] == true || !reply["status"].is_null() { return false }
    reply["error"].as_str().is_some_and(|e| {
        e.starts_with("dial ipc://")
            || e.contains(super::nng::CLOSED)
            || e.contains("KiCad IPC connection failed")
            || e == kipc::CONN_CLOSED
    })
}

/// True when `reply` failed on `dial ipc://...` before any bytes reached KiCad (safe to invalidate and retry).
pub fn is_dial_refused(reply: &Value) -> bool {
    reply["ok"] != true
        && reply["status"].is_null()
        && reply["error"].as_str().is_some_and(|e| e.starts_with("dial ipc://"))
}

/// True when `reply` is a client-side IPC timeout (`KiCad did not answer in ... s`).
pub fn is_timeout_error(reply: &Value) -> bool {
    reply["ok"] != true
        && reply["status"].is_null()
        && reply["error"].as_str().is_some_and(|e| e.starts_with("KiCad did not answer in "))
}

struct AskOutcome { reply: Value, sock: String, launch_gen: u64, dead_invalidated: bool }
enum PendingReply { Ask(oneshot::Sender<AskOutcome>), Raw(oneshot::Sender<Value>) }

fn send_pending(reply: PendingReply, sock: &str, launch_gen: u64, val: Value, dead_invalidated: bool) {
    match reply {
        PendingReply::Ask(tx) => {
            let _ = tx.send(AskOutcome { reply: val, sock: sock.into(), launch_gen, dead_invalidated });
        }
        PendingReply::Raw(tx) => {
            let _ = tx.send(val);
        }
    }
}

enum Msg {
    Ask { project: String, env: Env, ty: String, msg: Value, deadline: std::time::Instant, reply: oneshot::Sender<AskOutcome> },
    Raw { sock: String, env: Env, ty: String, msg: Value, timeout_ms: Option<u64>, fence: Option<(String, u64)>, reply: oneshot::Sender<Value> },
    Retire { project: String, reply: oneshot::Sender<Value> },
    Retired { project: String, launch_gen: u64, ok: bool, why: String, reply: oneshot::Sender<Value> },
    Info(oneshot::Sender<String>),
    Argv { project: String, env: Env, reply: oneshot::Sender<String> },
    ReadySock { project: String, reply: oneshot::Sender<Option<(String, u64)>> },
    Invalidate { project: String, launch_gen: u64, why: String, reply: oneshot::Sender<bool> },
    Clear,
    Line(u64, Value),
    Up { project: String, version: String, launch_gen: u64 },
    Bad { project: String, why: String, launch_gen: u64 },
    Late { project: String, born: u64, launch_gen: u64 },
    Go { project: String, launch_gen: u64 },
    Evicted { victim: String, victim_gen: u64, project: String, project_gen: u64, ok: bool, why: String },
}

struct Wt { project: String, ty: String, msg: Value, env: Env, deadline: std::time::Instant, reply: oneshot::Sender<AskOutcome> }
/// What carries the actor's requests. It takes request objects and answers, through the actor's mailbox, with
/// the objects it writes (`Msg::Line`): replies by id, and `{"event":"line"|"exit"}` for a server.
trait Transport: Send {
    fn send(&mut self, fields: Value) -> bool; // false: it is gone, and nothing will answer
    fn process_alive(&self, _key: &str) -> Option<bool> { None }
    fn pid_of(&self, _key: &str) -> u32 { 0 }
}
/// The hub's own: the default.
impl Transport for kipc::Native {
    fn send(&mut self, fields: Value) -> bool { self.handle(fields); true }
    fn process_alive(&self, key: &str) -> Option<bool> { kipc::Native::process_alive(self, key) }
    fn pid_of(&self, key: &str) -> u32 { kipc::Native::pid_of(self, key) }
}
struct Helper { link: Box<dyn Transport>, gen: u64 }
struct Pending { project: Option<String>, sock: String, launch_gen: u64, reply: PendingReply }

struct Actor {
    me: mpsc::UnboundedSender<Msg>, home: String, rt: String, hub_pid: u32,
    helper: Option<Helper>, gen: u64, pend: HashMap<String, Pending>, next: u64,
    srvs: Vec<Srv>, srv_gen: HashMap<String, u64>, wait: Vec<Wt>, shown: String, env: Env,
    lines: Arc<std::sync::Mutex<String>>, timeout_override_ms: Option<u64>,
}

pub fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }
pub fn now_ms() -> u128 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0) }

/// Socket directory for headless `kicad-cli api-server`: `ks::dir` when `<dir>/api.sock` fits macOS's 104-byte `SUN_LEN`,
/// else a short per-UID temp folder so long macOS home paths never fail Unix socket bind/connect.
pub(crate) fn sock_dir_for(rt: &str, home: &str, project: &str) -> String {
    let d = ks::dir(rt, home, project);
    #[cfg(unix)]
    {
        const MAX_DIR: usize = 94; // + "/api.sock" (9) = 103 < 104 (macOS SUN_LEN)
        const MAX_FALLBACK_DIR: usize = 82; // + "/api-9999999.sock" (17) = 99 < 104
        if d.len() > MAX_DIR {
            let dig = ks::digest(project);
            let ns = format!("bp-ks-{}", crate::plat::fs::uid());
            let tmp = std::env::temp_dir().join(&ns).join(&dig).to_string_lossy().into_owned();
            if tmp.len() <= MAX_FALLBACK_DIR { return tmp }
            return format!("/tmp/{ns}/{dig}");
        }
    }
    d
}

#[cfg(unix)]
fn check_dir_owner_no_symlink(p: &Path, uid: u32) -> Result<(), String> {
    use crate::plat::fs::{MetadataExt, PermissionsExt};
    let meta = std::fs::symlink_metadata(p).map_err(|e| format!("could not inspect {}: {e}", p.display()))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(format!("refusing unsafe KiCad socket folder (not a real directory): {}", p.display()));
    }
    if meta.uid() != uid {
        return Err(format!("refusing KiCad socket folder owned by uid {} instead of {}: {}", meta.uid(), uid, p.display()));
    }
    if meta.permissions().mode() & 0o077 != 0 {
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_private_sock_dir(dir: &Path) -> Result<(), String> {
    use crate::plat::fs::DirBuilderExt;
    let uid = crate::plat::fs::uid();
    if let Some(parent) = dir.parent().filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("bp-ks-"))) {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        check_dir_owner_no_symlink(parent, uid)?;
    }
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
        .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    check_dir_owner_no_symlink(dir, uid)
}

impl Actor {
    fn srv(&self, project: &str) -> Srv { self.srvs.iter().find(|s| s.project == project).cloned().unwrap_or_else(|| Srv::none(project)) } // KServe.get
    fn put(&mut self, s: Srv) { match self.srvs.iter_mut().find(|x| x.project == s.project) { Some(x) => *x = s, None => self.srvs.push(s) } } // KServe.put
    fn emit(&mut self) { let t = ks::lines(&self.srvs); if t != self.shown { *self.lines.lock().unwrap() = t.clone(); self.shown = t } } // KServe.emit
    fn later(&self, secs: f64, m: Msg) { let me = self.me.clone(); tokio::spawn(async move { tokio::time::sleep(Duration::from_secs_f64(secs)).await; let _ = me.send(m); }); }
    fn launch_gen(&self, project: &str) -> u64 { self.srv_gen.get(project).copied().unwrap_or(0) }
    fn bump_gen(&mut self, project: &str) -> u64 { let g = self.launch_gen(project) + 1; self.srv_gen.insert(project.to_string(), g); g }
    fn process_alive(&self, project: &str) -> Option<bool> { self.helper.as_ref().and_then(|h| h.link.process_alive(project)) }
    fn pid_of(&self, project: &str) -> u32 { self.helper.as_ref().map_or(0, |h| h.link.pid_of(project)) }
    fn sock_dir(&self, project: &str) -> String { sock_dir_for(&self.rt, &self.home, project) }

    fn remove_stale_sock(&self, project: &str, sock: &str) {
        #[cfg(unix)]
        if !sock.is_empty() {
            let p = Path::new(sock);
            let parent = p.parent();
            let expected_dir = self.sock_dir(project);
            let kserve_dir = ks::dir(&self.rt, &self.home, project);
            if let Some(dir) = parent.filter(|d| *d == Path::new(&expected_dir) || *d == Path::new(&kserve_dir)) {
                if check_dir_owner_no_symlink(dir, crate::plat::fs::uid()).is_ok() {
                    let _ = std::fs::remove_file(p);
                }
            }
        }
        #[cfg(not(unix))]
        let _ = (project, sock);
    }

    fn invalidate_server(&mut self, project: &str, expected_gen: Option<u64>, why: &str, remove_sock_file: bool) {
        if expected_gen.is_some_and(|g| self.launch_gen(project) != g) { return }
        let before = self.srv(project);
        let old_sock = before.sock.clone();
        if !old_sock.is_empty() {
            self.close(&old_sock);
            if remove_sock_file { self.remove_stale_sock(project, &old_sock); }
        }
        if ks::active(&before.state) {
            let s = before.disconnected(why);
            self.put(s);
            self.fail_project(project, why);
            self.emit();
        }
    }

    fn mark_unresponsive(&mut self, project: &str, expected_gen: u64, why: &str) {
        if self.launch_gen(project) != expected_gen { return }
        let before = self.srv(project);
        if !before.sock.is_empty() {
            self.close(&before.sock);
        }
        if matches!(before.state.as_str(), "ready" | "unresponsive") {
            let s = before.unresponsive(why);
            self.put(s);
            self.fail_project(project, why);
            self.emit();
        }
    }

    fn reap_dead(&mut self) {
        let candidates: Vec<(String, u32)> = self.srvs.iter()
            .filter(|s| matches!(s.state.as_str(), "ready" | "unresponsive") || (s.state == "starting" && !s.sock.is_empty()))
            .map(|s| (s.project.clone(), s.pid))
            .collect();
        for (project, pid) in candidates {
            if self.process_alive(&project) == Some(false) {
                self.invalidate_server(&project, None, "the headless KiCad process exited", true);
            } else if pid == 0 {
                let live_pid = self.pid_of(&project);
                if live_pid > 0 {
                    if let Some(s) = self.srvs.iter_mut().find(|x| x.project == project) { s.pid = live_pid; }
                }
            }
        }
    }

    /// A request line registered for its reply (KServe.call). The helper is started on first use.
    fn call(&mut self, fields: Value, reply: oneshot::Sender<Value>) { self.call_for(None, "", 0, fields, PendingReply::Raw(reply)) }
    fn call_for(&mut self, project: Option<&str>, sock: &str, launch_gen: u64, mut fields: Value, reply: PendingReply) {
        if self.helper.is_none() { self.start_helper() }
        let Some(h) = &mut self.helper else { send_pending(reply, sock, launch_gen, failed("the KiCad IPC transport could not start"), false); return };
        self.next += 1; let id = self.next.to_string();
        fields["id"] = json!(self.next);
        if !h.link.send(fields) { send_pending(reply, sock, launch_gen, failed("the KiCad IPC helper exited"), false); return }
        self.pend.insert(id, Pending { project: project.map(String::from), sock: sock.into(), launch_gen, reply });
    }
    fn start_helper(&mut self) {
        self.gen += 1; let gen = self.gen;
        let me = self.me.clone();
        self.helper = Some(Helper { link: Box::new(kipc::Native::new("", Arc::new(move |o| { let _ = me.send(Msg::Line(gen, o)); }))), gen });
    }
    fn sink(&mut self, fields: Value) { let (t, _r) = oneshot::channel(); self.call(fields, t) }

    fn forward(&mut self, w: Wt) { // KServe.forward
        if w.reply.is_closed() { return }
        let srv = self.srv(&w.project).touch(now_s());
        let launch_gen = self.launch_gen(&w.project);
        let sock = srv.sock.clone();
        let rem_ms = w.deadline.saturating_duration_since(std::time::Instant::now()).as_millis() as u64;
        if rem_ms == 0 {
            let short = w.ty.rsplit('.').next().unwrap_or(&w.ty);
            let _ = w.reply.send(AskOutcome {
                reply: failed(&format!("KiCad IPC request {short} timed out after {} s waiting on the headless server", ASK_TIMEOUT.as_secs())),
                sock,
                launch_gen,
                dead_invalidated: false,
            });
            return;
        }
        let req_timeout = self.timeout_override_ms.unwrap_or_else(|| timeout_ms_for(&w.ty)).min(rem_ms).max(1);
        let f = self.call_fields(&sock, &w.env, &w.ty, w.msg, Some(req_timeout));
        self.put(srv);
        self.call_for(Some(&w.project), &sock, launch_gen, f, PendingReply::Ask(w.reply));
    }
    fn call_fields(&self, sock: &str, env: &Env, ty: &str, msg: Value, timeout_ms: Option<u64>) -> Value { // KServe.call.fields
        let timeout = timeout_ms.unwrap_or_else(|| self.timeout_override_ms.unwrap_or_else(|| timeout_ms_for(ty)));
        json!({"sock":sock,"type":ty,"message":msg,"desc":ks::desc(&env.root, &env.caps),"lib":format!("{}/lib", env.root),"timeout":timeout})
    }
    fn fail_project(&mut self, project: &str, text: &str) { // KServe.fail.project
        let (mine, rest): (Vec<Wt>, Vec<Wt>) = std::mem::take(&mut self.wait).into_iter().partition(|w| w.project == project);
        self.wait = rest;
        let launch_gen = self.launch_gen(project);
        for w in mine {
            let _ = w.reply.send(AskOutcome { reply: failed(&ks::unavailable(text, &w.env.cli)), sock: String::new(), launch_gen, dead_invalidated: false });
        }
    }
    fn launch(&mut self, project: &str) { // KServe.launch: helper present, then the server is started and gets 30 s to say it listens
        let now = now_s();
        let launch_gen = self.bump_gen(project);
        let dir = self.sock_dir(project);
        #[cfg(unix)]
        if let Err(why) = ensure_private_sock_dir(Path::new(&dir)) {
            let s = self.srv(project).set("stopped", "");
            self.put(s);
            self.fail_project(project, &why);
            self.emit();
            return;
        }
        #[cfg(not(unix))]
        { let _ = std::fs::create_dir_all(&dir); }
        let argv = ks::argv(&self.env.cli, project, &dir, self.hub_pid, &self.env.caps);
        let envv: Map<String, Value> = ["BACKPLANE_KICAD_LIBRARY", "KICAD_CONFIG_HOME"].iter().map(|k| (k.to_string(), json!(std::env::var(k).unwrap_or_default()))).collect();
        self.sink(json!({"op":"spawn","key":project,"gen":launch_gen,"argv":argv,"memory":"1536M","env":envv}));
        let s = self.srv(project).starting(now); self.put(s);
        if self.helper.is_some() { self.later(30.0, Msg::Late { project: project.into(), born: now, launch_gen }) }
        self.emit();
    }
    fn close(&mut self, sock: &str) { if !sock.is_empty() { self.sink(json!({"op":"close","sock":sock})) } } // KServe.close

    fn ask(&mut self, w: Wt) {
        if w.reply.is_closed() { return }
        if std::time::Instant::now() >= w.deadline {
            let short = w.ty.rsplit('.').next().unwrap_or(&w.ty);
            let launch_gen = self.launch_gen(&w.project);
            let _ = w.reply.send(AskOutcome {
                reply: failed(&format!("KiCad IPC request {short} timed out after {} s waiting on the headless server", ASK_TIMEOUT.as_secs())),
                sock: String::new(),
                launch_gen,
                dead_invalidated: false,
            });
            return;
        }
        let plan = ks::plan(&self.srvs, &w.project, &self.env.caps);
        let cur_gen = self.launch_gen(&w.project);
        match plan.as_str() {
            "ready" => self.forward(w),
            "unresponsive" => {
                if self.srv(&w.project).sock.is_empty() {
                    let _ = w.reply.send(AskOutcome {
                        reply: failed(&ks::unavailable("the headless KiCad process is unresponsive", &w.env.cli)),
                        sock: String::new(),
                        launch_gen: cur_gen,
                        dead_invalidated: false,
                    });
                } else {
                    self.forward(w);
                }
            }
            "wait" => self.wait.insert(0, w),
            "failed" => { let why = format!("the headless KiCad for {} stopped: {}", ks::name(&w.project), self.srv(&w.project).why); let _ = w.reply.send(AskOutcome { reply: failed(&ks::unavailable(&why, &w.env.cli)), sock: String::new(), launch_gen: cur_gen, dead_invalidated: false }); }
            "busy" => { let _ = w.reply.send(AskOutcome { reply: failed(&ks::unavailable(ks::BUSY, &w.env.cli)), sock: String::new(), launch_gen: cur_gen, dead_invalidated: false }); }
            "start" => { let p = w.project.clone(); self.wait.insert(0, w); self.launch(&p) }
            p => { // "evict:<victim>": the least recently used server is asked to stop (it does so itself); the new one starts when it has
                let victim = p.trim_start_matches("evict:").to_string(); let project = w.project.clone();
                let victim_gen = self.launch_gen(&victim);
                let project_gen = self.bump_gen(&project);
                self.evicting(&victim, victim_gen, &project, project_gen);
                let s = Srv::new(&project, now_s()); self.put(s); self.wait.insert(0, w); self.emit();
            }
        }
    }
    fn evicting(&mut self, victim: &str, victim_gen: u64, project: &str, project_gen: u64) { // KServe.evicting: kiapi.backplane.Shutdown, never a signal
        let sock = self.srv(victim).sock; let env = self.env.clone(); let me = self.me.clone();
        let (victim, project) = (victim.to_string(), project.to_string());
        tokio::spawn(async move {
            let (t, r) = oneshot::channel(); let _ = me.send(Msg::Raw { sock, env, ty: "kiapi.backplane.Shutdown".into(), msg: json!({}), timeout_ms: None, fence: Some((victim.clone(), victim_gen)), reply: t });
            let o = r.await.unwrap_or_else(|_| failed("no reply"));
            let _ = me.send(Msg::Evicted { victim, victim_gen, project, project_gen, ok: o["ok"].as_bool().unwrap_or(false), why: o["error"].as_str().unwrap_or("").into() });
        });
    }
    /// A server that is listening answers Ping and GetVersion, and is the build the manifest names, before any request goes to it (KServe.hand).
    fn hand(&self, project: String, env: Env, sock: String, launch_gen: u64) {
        let me = self.me.clone();
        tokio::spawn(async move {
            let ask = |ty: &str| { let (t, r) = oneshot::channel(); let _ = me.send(Msg::Raw { sock: sock.clone(), env: env.clone(), ty: ty.into(), msg: json!({}), timeout_ms: None, fence: Some((project.clone(), launch_gen)), reply: t }); r };
            // KiCad listens before it has finished loading and says so ("KiCad is not ready to reply", AS_NOT_READY): that is asked again for
            // up to READY_WAIT, never taken as a failed server (which stays failed until the person retries it)
            let mut p = ask("kiapi.common.commands.Ping").await.unwrap_or_else(|_| failed("no reply"));
            let until = std::time::Instant::now() + READY_WAIT;
            while not_ready(&p) && std::time::Instant::now() < until { tokio::time::sleep(Duration::from_millis(250)).await; p = ask("kiapi.common.commands.Ping").await.unwrap_or_else(|_| failed("no reply")); }
            if !p["ok"].as_bool().unwrap_or(false) { let _ = me.send(Msg::Bad { project, why: format!("it did not answer Ping: {}", p["error"].as_str().unwrap_or("")), launch_gen }); return }
            let o = ask("kiapi.common.commands.GetVersion").await.unwrap_or_else(|_| failed("no reply"));
            let full = o["message"]["version"]["full_version"].as_str().unwrap_or("").to_string();
            // status says the manifest's version (a test build's GetVersion is a describe name, "...-dirty-bp.13"); KiCad's own string is logged
            if o["ok"].as_bool().unwrap_or(false) && ks::version_ok(&full, &env.version) {
                // (written so that a closed stderr cannot stop the handshake: eprintln! panics there, and the server would never be handed its requests)
                { use std::io::Write; let _ = writeln!(std::io::stderr(), "kicad ipc: {project} up, KiCad says {full} (manifest {})", env.version); }
                let _ = me.send(Msg::Up { project, version: shown_version(&env.version, &full), launch_gen }); }
            else { let err = o["error"].as_str().unwrap_or(""); let _ = me.send(Msg::Bad { project, why: if err.is_empty() { format!("this KiCad is {full}, not the {} Backplane installed", env.version) } else { format!("it did not answer GetVersion: {err}") }, launch_gen }); }
        });
    }

    fn line(&mut self, o: Value) { // what the transport said: a server's stdout line or exit, else a reply (by id)
        match o["event"].as_str() {
            Some("line") => {
                let project = o["key"].as_str().unwrap_or("").to_string();
                let ev_gen = o.get("gen").and_then(Value::as_u64);
                let cur_gen = self.launch_gen(&project);
                if ev_gen.is_some_and(|g| g != cur_gen) { return }
                if let Some(path) = ks::ready(o["text"].as_str().unwrap_or("")) {
                    let s = self.srv(&project);
                    if s.state == "starting" && s.sock.is_empty() {
                        self.hand(project.clone(), self.env.clone(), path.clone(), cur_gen);
                        let mut s = s.listening(&path);
                        s.pid = self.pid_of(&project);
                        self.put(s);
                    }
                }
            }
            Some("exit") => {
                let project = o["key"].as_str().unwrap_or("").to_string();
                let ev_gen = o.get("gen").and_then(Value::as_u64);
                let cur_gen = self.launch_gen(&project);
                if ev_gen.is_some_and(|g| g != cur_gen) { return }
                self.exited(project, o["code"].as_i64().unwrap_or(1) as i32, o["tail"].as_str().unwrap_or("").to_string(), cur_gen);
            }
            _ => {
                let id = match &o["id"] { Value::String(s) => s.clone(), v => v.to_string() };
                if let Some(p) = self.pend.remove(&id) {
                    let mut dead_invalidated = false;
                    if let Some(project) = &p.project {
                        if self.launch_gen(project) == p.launch_gen {
                            if o["ok"].as_bool().unwrap_or(false) {
                                if let Some(s) = self.srvs.iter_mut().find(|x| x.project == *project && x.state == "unresponsive" && x.sock == p.sock) {
                                    s.state = "ready".into();
                                    s.why.clear();
                                    s.last = now_s();
                                    self.emit();
                                }
                            } else if is_conn_error(&o) {
                                if self.process_alive(project) == Some(false) {
                                    self.invalidate_server(project, Some(p.launch_gen), "the headless KiCad process exited", true);
                                    dead_invalidated = true;
                                } else {
                                    self.mark_unresponsive(project, p.launch_gen, "KiCad IPC connection dropped");
                                }
                            } else if is_timeout_error(&o) && self.process_alive(project) == Some(false) {
                                self.invalidate_server(project, Some(p.launch_gen), "the headless KiCad process exited", true);
                                dead_invalidated = true;
                            }
                        }
                    }
                    send_pending(p.reply, &p.sock, p.launch_gen, o, dead_invalidated);
                }
            }
        }
    }
    fn exited(&mut self, project: String, code: i32, tail: String, launch_gen: u64) { // the server's process ended: the record says what that means
        if self.launch_gen(&project) != launch_gen { return }
        let now = now_s(); let before = self.srv(&project);
        let old_sock = before.sock.clone();
        self.close(&old_sock);
        self.remove_stale_sock(&project, &old_sock);
        if before.state == "stopped" {
            if code == 0 {
                if !before.why.is_empty() { self.put(before.set("stopped", "")); self.emit(); }
            } else {
                let fails = ks::fails_add(code, &before.fails, now);
                let mut srv = before;
                srv.fails = fails;
                if ks::retry(&srv.fails, now).is_none() {
                    srv = srv.set("failed", &format!("stopped four times in five minutes: {tail}"));
                }
                self.put(srv);
                self.emit();
            }
            return
        }
        let srv = before.exited(code, now, &tail); self.put(srv.clone());
        let name = ks::name(&project);
        match srv.state.as_str() {
            "starting" => if let Some(s) = ks::retry(&srv.fails, now) { self.later(s as f64, Msg::Go { project, launch_gen }) },
            "failed" => self.fail_project(&project, &format!("the headless KiCad for {name} stopped: {}", srv.why)),
            _ => self.fail_project(&project, &format!("the headless KiCad for {name} exited")),
        }
        self.emit();
    }
    fn up(&mut self, project: String, version: String, launch_gen: u64) { // it answered Ping and GetVersion as the right build: ready, and what waited goes out
        if self.launch_gen(&project) != launch_gen { return }
        let s = self.srv(&project); if s.state != "starting" || s.sock.is_empty() { return }
        if self.process_alive(&project) == Some(false) {
            self.invalidate_server(&project, Some(launch_gen), "the headless KiCad process exited", true);
            return
        }
        let sock = s.sock.clone();
        let mut up_srv = s.up(&sock, &version, now_s());
        up_srv.pid = self.pid_of(&project);
        self.put(up_srv);
        let (mine, rest): (Vec<Wt>, Vec<Wt>) = std::mem::take(&mut self.wait).into_iter().partition(|w| w.project == project); self.wait = rest;
        for w in mine { self.forward(w) }
        self.emit();
    }
    fn bad(&mut self, project: String, why: String, launch_gen: u64) { // it failed the handshake or never listened
        if self.launch_gen(&project) != launch_gen { return }
        let before = self.srv(&project);
        if before.state != "starting" { return }
        let old_sock = before.sock.clone();
        self.close(&old_sock);
        if self.process_alive(&project) == Some(false) {
            self.remove_stale_sock(&project, &old_sock);
        }
        let s = before.set("failed", &why); self.put(s);
        self.fail_project(&project, &format!("the headless KiCad for {} {why}", ks::name(&project))); self.emit();
    }
    fn evicted(&mut self, victim: String, victim_gen: u64, project: String, project_gen: u64, ok: bool, why: String) {
        if ok {
            if self.launch_gen(&victim) == victim_gen {
                let before = self.srv(&victim);
                let old_sock = before.sock.clone();
                self.close(&old_sock);
                self.remove_stale_sock(&victim, &old_sock);
                self.bump_gen(&victim);
                let s = before.set("stopped", ""); self.put(s);
                self.emit();
            }
            if self.launch_gen(&project) == project_gen && self.srv(&project).state == "starting" {
                let live_others = self.srvs.iter().filter(|x| x.project != project && ks::active(&x.state)).count();
                if live_others < 2 {
                    self.launch(&project);
                } else {
                    let other = ks::evict(&self.srvs);
                    if other.is_empty() {
                        self.bump_gen(&project);
                        let s = self.srv(&project).set("stopped", ""); self.put(s);
                        self.fail_project(&project, &format!("{} ({} was busy)", ks::BUSY, ks::name(&victim))); self.emit();
                    } else {
                        let other_gen = self.launch_gen(&other);
                        self.evicting(&other, other_gen, &project, project_gen);
                    }
                }
            }
            return
        }
        if self.launch_gen(&victim) == victim_gen && ks::active(&self.srv(&victim).state) {
            let s = self.srv(&victim).hold(); self.put(s); // Shutdown refused (unsaved changes): held, never forced; the next least recently used is asked
        }
        if self.launch_gen(&project) != project_gen || self.srv(&project).state != "starting" { return }
        let other = ks::evict(&self.srvs);
        if other.is_empty() {
            let before = self.srv(&project);
            let old_sock = before.sock.clone();
            self.close(&old_sock);
            self.remove_stale_sock(&project, &old_sock);
            self.bump_gen(&project);
            let s = before.set("stopped", ""); self.put(s);
            self.fail_project(&project, &format!("{} ({} would not stop: {why})", ks::BUSY, ks::name(&victim))); self.emit();
        } else {
            let other_gen = self.launch_gen(&other);
            self.evicting(&other, other_gen, &project, project_gen);
        }
    }
    fn retire(&mut self, project: String, reply: oneshot::Sender<Value>) {
        let s = self.srv(&project);
        let launch_gen = self.launch_gen(&project);
        if s.state == "starting" && s.sock.is_empty() && self.process_alive(&project) != Some(false) {
            let _ = reply.send(json!({"ok":false,"error":"the headless KiCad server is still starting"}));
            return;
        }
        if s.state == "failed" && self.process_alive(&project) == Some(true) {
            let _ = reply.send(json!({"ok":false,"error":"the failed headless KiCad process is still running"}));
            return;
        }
        if s.sock.is_empty() || matches!(s.state.as_str(), "none" | "stopped" | "failed") {
            let old_sock = s.sock.clone();
            self.close(&old_sock);
            self.remove_stale_sock(&project, &old_sock);
            self.bump_gen(&project);
            self.put(s.set("stopped", "")); self.fail_project(&project, "the GUI KiCad took over this project"); self.emit();
            let _ = reply.send(json!({"ok":true,"retired":false})); return
        }
        let sock = s.sock.clone(); let env = self.env.clone(); let me = self.me.clone();
        tokio::spawn(async move {
            let (t, r) = oneshot::channel(); let _ = me.send(Msg::Raw { sock, env, ty: "kiapi.backplane.Shutdown".into(), msg: json!({}), timeout_ms: None, fence: Some((project.clone(), launch_gen)), reply: t });
            let o = r.await.unwrap_or_else(|_| failed("no reply"));
            let _ = me.send(Msg::Retired { project, launch_gen, ok: o["ok"].as_bool().unwrap_or(false), why: o["error"].as_str().unwrap_or("").into(), reply });
        });
    }
    fn retired(&mut self, project: String, launch_gen: u64, ok: bool, why: String, reply: oneshot::Sender<Value>) {
        if self.launch_gen(&project) != launch_gen {
            let cur = self.srv(&project);
            if !ks::active(&cur.state) {
                let _ = reply.send(json!({"ok":true,"retired":false}));
            } else {
                let _ = reply.send(json!({"ok":false,"error":"a newer headless KiCad server replaced the retired one"}));
            }
            return;
        }
        if ok {
            let before = self.srv(&project);
            let old_sock = before.sock.clone();
            self.close(&old_sock);
            self.remove_stale_sock(&project, &old_sock);
            self.bump_gen(&project);
            let s = before.set("stopped", ""); self.put(s);
            self.fail_project(&project, "the GUI KiCad took over this project"); self.emit();
            let _ = reply.send(json!({"ok":true,"retired":true}));
        } else {
            let s = self.srv(&project).hold(); self.put(s); self.emit();
            let _ = reply.send(json!({"ok":false,"error":why}));
        }
    }
    fn handle(&mut self, m: Msg) {
        match m {
            Msg::Ask { project, env, ty, msg, deadline, reply } => { self.env = env.clone(); self.reap_dead(); self.ask(Wt { project, ty, msg, env, deadline, reply }) }
            Msg::Raw { sock, env, ty, msg, timeout_ms, fence, reply } => {
                if reply.is_closed() { return }
                if let Some((proj, expected_gen)) = fence {
                    if self.launch_gen(&proj) != expected_gen || self.srv(&proj).sock != sock {
                        let _ = reply.send(failed("stale KiCad server generation"));
                        return;
                    }
                }
                let f = self.call_fields(&sock, &env, &ty, msg, timeout_ms);
                self.call(f, reply)
            }
            Msg::Retire { project, reply } => self.retire(project, reply),
            Msg::Retired { project, launch_gen, ok, why, reply } => self.retired(project, launch_gen, ok, why, reply),
            Msg::Info(r) => { self.reap_dead(); let _ = r.send(ks::lines(&self.srvs)); }
            Msg::Argv { project, env, reply } => { let d = self.sock_dir(&project); let _ = reply.send(ks::argv(&env.cli, &project, &d, self.hub_pid, &env.caps).join(" ")); }
            Msg::ReadySock { project, reply } => {
                self.reap_dead();
                let s = self.srv(&project);
                let out = (matches!(s.state.as_str(), "ready" | "unresponsive") && !s.sock.is_empty()).then(|| (s.sock, self.launch_gen(&project)));
                let _ = reply.send(out);
            }
            Msg::Invalidate { project, launch_gen, why, reply } => {
                if self.launch_gen(&project) != launch_gen {
                    let _ = reply.send(false);
                } else if self.process_alive(&project) == Some(false) {
                    self.invalidate_server(&project, Some(launch_gen), &why, true);
                    self.bump_gen(&project);
                    let _ = reply.send(true);
                } else {
                    self.mark_unresponsive(&project, launch_gen, &why);
                    let _ = reply.send(false);
                }
            }
            Msg::Clear => {
                let projects: Vec<String> = self.srvs.iter()
                    .filter(|s| s.state == "failed" && self.process_alive(&s.project) != Some(true))
                    .map(|s| s.project.clone())
                    .collect();
                for p in &projects {
                    if let Some(s) = self.srvs.iter_mut().find(|x| x.project == *p) {
                        *s = s.clone().cleared();
                    }
                    self.bump_gen(p);
                }
                self.emit()
            }
            Msg::Line(gen, t) => if self.helper.as_ref().map(|h| h.gen) == Some(gen) { self.line(t) },
            Msg::Up { project, version, launch_gen } => self.up(project, version, launch_gen),
            Msg::Bad { project, why, launch_gen } => self.bad(project, why, launch_gen),
            Msg::Late { project, born, launch_gen } => {
                let s = self.srv(&project);
                if s.state == "starting" && s.started == born && s.sock.is_empty() && self.launch_gen(&project) == launch_gen {
                    self.bad(project, "did not say it was listening within 30 s".into(), launch_gen)
                }
            }
            Msg::Go { project, launch_gen } => if self.srv(&project).state == "starting" && self.launch_gen(&project) == launch_gen { self.launch(&project) },
            Msg::Evicted { victim, victim_gen, project, project_gen, ok, why } => self.evicted(victim, victim_gen, project, project_gen, ok, why),
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
        srvs: vec![], srv_gen: HashMap::new(), wait: vec![], shown: String::new(), env: Env::default(), lines: lines.clone(), timeout_override_ms: None };
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
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ProbeKey {
    path: PathBuf,
    mtime: Option<std::time::SystemTime>,
    size: u64,
}

/// Resolve `cli` without probing the working directory for bare command names, then canonicalize so fingerprinting and launching
/// target the exact same file.
pub(crate) fn resolve_probe_path(cli: &Path) -> Option<PathBuf> {
    if cli.as_os_str().is_empty() { return None }
    let cand = if cli.components().count() > 1 {
        cli.to_path_buf()
    } else {
        let path_env = std::env::var_os("PATH")?;
        let exts: &[&str] = if cfg!(windows) && cli.extension().is_none() { &["", ".exe", ".cmd", ".bat"] } else { &[""] };
        let mut hit = None;
        'outer: for dir in std::env::split_paths(&path_env) {
            if dir.as_os_str().is_empty() { continue }
            for ext in exts {
                let mut file = cli.as_os_str().to_os_string();
                if !ext.is_empty() { file.push(ext); }
                let p = dir.join(file);
                if std::fs::metadata(&p).is_ok_and(|m| crate::plat::fs::runnable(&m)) {
                    hit = Some(p);
                    break 'outer;
                }
            }
        }
        hit?
    };
    Some(crate::plat::fs::canonicalize(&cand).unwrap_or(cand))
}

pub(crate) fn probe_key_for(resolved: &Path) -> Option<ProbeKey> {
    let meta = std::fs::metadata(resolved).ok()?;
    Some(ProbeKey { path: resolved.to_path_buf(), mtime: meta.modified().ok(), size: meta.len() })
}

#[derive(Clone, Debug)]
pub enum ProbeOutcome {
    Output(std::process::Output),
    Timeout { limit: Duration, attempts: usize },
    SpawnErr(String),
}

impl ProbeOutcome {
    pub fn is_ok_success(&self) -> bool {
        matches!(self, Self::Output(o) if o.status.success())
    }

    pub fn into_output(self) -> Option<std::process::Output> {
        match self {
            Self::Output(o) => Some(o),
            _ => None,
        }
    }
}

struct Flight {
    waiters: std::sync::atomic::AtomicUsize,
    done: std::sync::Mutex<Option<ProbeOutcome>>,
    cv: std::sync::Condvar,
}

#[derive(Default)]
struct CacheState {
    entries: HashMap<ProbeKey, (std::time::Instant, ProbeOutcome)>,
    in_flight: HashMap<ProbeKey, Arc<Flight>>,
}

pub(crate) struct VersionProbeCache {
    state: std::sync::Mutex<CacheState>,
    failure_ttl: Duration,
}

struct FlightGuard<'a> {
    cache: &'a VersionProbeCache,
    key: ProbeKey,
    flight: Arc<Flight>,
    finished: bool,
}

impl Drop for FlightGuard<'_> {
    fn drop(&mut self) {
        if self.finished { return }
        {
            let mut st = self.cache.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.in_flight.get(&self.key).is_some_and(|f| Arc::ptr_eq(f, &self.flight)) {
                st.in_flight.remove(&self.key);
            }
        }
        let mut done = self.flight.done.lock().unwrap_or_else(|e| e.into_inner());
        if done.is_none() {
            *done = Some(ProbeOutcome::SpawnErr("probe panicked".into()));
            self.flight.cv.notify_all();
        }
    }
}

const VERSION_CACHE_CAP: usize = 128;
const VERSION_FAILURE_TTL: Duration = Duration::from_secs(30);

impl VersionProbeCache {
    pub fn new(failure_ttl: Duration) -> Self {
        Self { state: std::sync::Mutex::new(CacheState::default()), failure_ttl }
    }

    pub fn get_or_probe(
        &self,
        cli: &Path,
        limit: Duration,
        attempts: usize,
        run: impl FnMut(&Path, Duration, usize, Option<std::time::Instant>) -> ProbeOutcome,
    ) -> ProbeOutcome {
        self.get_or_probe_at(cli, limit, attempts, std::time::Instant::now, run)
    }

    pub(crate) fn get_or_probe_at(
        &self,
        cli: &Path,
        limit: Duration,
        attempts: usize,
        now: impl Fn() -> std::time::Instant,
        mut run: impl FnMut(&Path, Duration, usize, Option<std::time::Instant>) -> ProbeOutcome,
    ) -> ProbeOutcome {
        let attempts = attempts.max(1);
        let Some(resolved) = resolve_probe_path(cli) else { return run(cli, limit, attempts, None) };
        let Some(key) = probe_key_for(&resolved) else { return run(&resolved, limit, attempts, None) };
        let wait_deadline = std::time::Instant::now() + limit.saturating_mul(attempts as u32);
        let mut rem_attempts = attempts;
        let mut waited_attempts_at_limit = 0usize;
        let mut waited = false;

        loop {
            let (flight, waiter) = {
                let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if waited && std::time::Instant::now() >= wait_deadline {
                    return ProbeOutcome::Timeout { limit, attempts };
                }
                let check_now = now();
                if let Some((at, outcome)) = st.entries.get(&key) {
                    let expired = !outcome.is_ok_success() && check_now.saturating_duration_since(*at) >= self.failure_ttl;
                    if expired {
                        st.entries.remove(&key);
                    } else {
                        match outcome {
                            ProbeOutcome::Output(o) if o.status.success() => return ProbeOutcome::Output(o.clone()),
                            ProbeOutcome::Timeout { limit: fl, attempts: fa } if *fl < limit || *fa < attempts => {}
                            ProbeOutcome::Timeout { .. } => return ProbeOutcome::Timeout { limit, attempts },
                            other => return other.clone(),
                        }
                    }
                }
                if std::time::Instant::now() >= wait_deadline {
                    return ProbeOutcome::Timeout { limit, attempts };
                }
                if let Some(flight) = st.in_flight.get(&key).cloned() {
                    flight.waiters.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    (flight, true)
                } else {
                    let flight = Arc::new(Flight {
                        waiters: std::sync::atomic::AtomicUsize::new(0),
                        done: std::sync::Mutex::new(None),
                        cv: std::sync::Condvar::new(),
                    });
                    st.in_flight.insert(key.clone(), flight.clone());
                    (flight, false)
                }
            };

            if waiter {
                waited = true;
                let mut done = flight.done.lock().unwrap_or_else(|e| e.into_inner());
                while done.is_none() {
                    let Some(rem) = wait_deadline.checked_duration_since(std::time::Instant::now()).filter(|d| !d.is_zero()) else {
                        return ProbeOutcome::Timeout { limit, attempts };
                    };
                    let (next, res) = flight.cv.wait_timeout(done, rem).unwrap_or_else(|e| e.into_inner());
                    done = next;
                    if done.is_none() && res.timed_out() {
                        return ProbeOutcome::Timeout { limit, attempts };
                    }
                }
                if std::time::Instant::now() >= wait_deadline {
                    return ProbeOutcome::Timeout { limit, attempts };
                }
                match done.clone().expect("notified flight always holds an outcome") {
                    ProbeOutcome::Timeout { limit: fl, attempts: fa } => {
                        if fl >= limit && fa > 0 {
                            waited_attempts_at_limit += fa;
                            rem_attempts = rem_attempts.saturating_sub(fa);
                            if rem_attempts == 0 {
                                return ProbeOutcome::Timeout { limit, attempts };
                            }
                        }
                        if std::time::Instant::now() >= wait_deadline {
                            return ProbeOutcome::Timeout { limit, attempts };
                        }
                        continue;
                    }
                    other => return other,
                }
            }

            let mut guard = FlightGuard { cache: self, key: key.clone(), flight: flight.clone(), finished: false };
            let raw = run(&resolved, limit, rem_attempts, waited.then_some(wait_deadline));
            let finished_at = now();
            let (cached_outcome, notify_outcome, caller_outcome) = match raw {
                ProbeOutcome::Timeout { limit: _, attempts: 0 } => {
                    let zero = ProbeOutcome::Timeout { limit: Duration::ZERO, attempts: 0 };
                    (None, zero, ProbeOutcome::Timeout { limit, attempts })
                }
                ProbeOutcome::Timeout { limit: l, attempts: a } => {
                    let total_at_limit = if l >= limit { a + waited_attempts_at_limit } else { a };
                    let cached = ProbeOutcome::Timeout { limit: l, attempts: total_at_limit };
                    let raw_done = ProbeOutcome::Timeout { limit: l, attempts: a };
                    let ret = if l >= limit && total_at_limit >= attempts {
                        ProbeOutcome::Timeout { limit: l, attempts: total_at_limit.max(attempts) }
                    } else {
                        ProbeOutcome::Timeout { limit, attempts }
                    };
                    (Some(cached), raw_done, ret)
                }
                other => (Some(other.clone()), other.clone(), other),
            };
            {
                let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if st.in_flight.get(&key).is_some_and(|f| Arc::ptr_eq(f, &flight)) {
                    st.in_flight.remove(&key);
                }
                if let Some(cached) = cached_outcome {
                    if st.entries.len() >= VERSION_CACHE_CAP {
                        let ttl = self.failure_ttl;
                        st.entries.retain(|_, (at, o)| o.is_ok_success() || finished_at.saturating_duration_since(*at) < ttl);
                        while st.entries.len() >= VERSION_CACHE_CAP {
                            if let Some(oldest) = st.entries.iter().min_by_key(|(_, (at, _))| *at).map(|(k, _)| k.clone()) {
                                st.entries.remove(&oldest);
                            } else {
                                break;
                            }
                        }
                    }
                    let keep_existing = st.entries.get(&key).is_some_and(|(at, cur)| {
                        let cur_fresh = cur.is_ok_success() || finished_at.saturating_duration_since(*at) < self.failure_ttl;
                        if !cur_fresh { return false }
                        if cur.is_ok_success() && !cached.is_ok_success() { return true }
                        match (cur, &cached) {
                            (ProbeOutcome::Timeout { limit: cl, attempts: ca }, ProbeOutcome::Timeout { limit: nl, attempts: na }) => *cl >= *nl && *ca >= *na,
                            _ => false,
                        }
                    });
                    if !keep_existing {
                        st.entries.insert(key, (finished_at, cached));
                    }
                }
            }
            {
                let mut done = flight.done.lock().unwrap_or_else(|e| e.into_inner());
                *done = Some(notify_outcome);
                flight.cv.notify_all();
            }
            guard.finished = true;
            return caller_outcome;
        }
    }

    #[cfg(test)]
    pub(crate) fn in_flight_waiters(&self, cli: &Path) -> usize {
        let Some(resolved) = resolve_probe_path(cli) else { return 0 };
        let Some(key) = probe_key_for(&resolved) else { return 0 };
        let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.in_flight.get(&key).map_or(0, |f| f.waiters.load(std::sync::atomic::Ordering::SeqCst))
    }
}

static VERSION_CACHE: OnceLock<VersionProbeCache> = OnceLock::new();
fn version_cache() -> &'static VersionProbeCache {
    VERSION_CACHE.get_or_init(|| VersionProbeCache::new(VERSION_FAILURE_TTL))
}

fn probe_cli_version_uncached_until(cli: &Path, limit: Duration, attempts: usize, deadline: Option<std::time::Instant>) -> ProbeOutcome {
    let attempts = attempts.max(1);
    let mut full_attempts = 0usize;
    let mut tried = 0usize;
    let mut max_limit_tried = Duration::ZERO;
    for _ in 0..attempts {
        if deadline.is_some_and(|dl| std::time::Instant::now() >= dl) {
            break;
        }
        let (mut c, _home) = crate::proc::kicad_probe(cli);
        c.arg("--version");
        let now_run = std::time::Instant::now();
        let (step_limit, step_end) = if let Some(dl) = deadline {
            let Some(rem) = dl.checked_duration_since(now_run).filter(|d| !d.is_zero()) else {
                break;
            };
            let step = limit.min(rem);
            (step, now_run + step)
        } else {
            (limit, now_run + limit)
        };
        if step_limit >= limit {
            full_attempts += 1;
        }
        tried += 1;
        max_limit_tried = max_limit_tried.max(step_limit);
        match crate::proc::probe_output_until(c, step_end, deadline, step_limit) {
            Ok(out) => return ProbeOutcome::Output(out),
            Err(crate::proc::ProbeError::Spawn(e)) => return ProbeOutcome::SpawnErr(e.to_string()),
            Err(crate::proc::ProbeError::Timeout) => {}
        }
    }
    if full_attempts > 0 {
        ProbeOutcome::Timeout { limit, attempts: full_attempts }
    } else {
        ProbeOutcome::Timeout { limit: max_limit_tried, attempts: tried }
    }
}

/// `<cli> --version`, single-flight deduplicated and cached by `(canonical_path, mtime, size)` with budget-aware sharing.
pub fn cli_version_cached_path(cli: &Path, limit: Duration, attempts: usize) -> ProbeOutcome {
    version_cache().get_or_probe(cli, limit, attempts, |p, l, a, deadline| {
        probe_cli_version_uncached_until(p, l, a, deadline)
    })
}

pub fn cli_version_cached(cli: &str, limit: Duration, attempts: usize) -> ProbeOutcome {
    cli_version_cached_path(Path::new(cli), limit, attempts)
}

/// `<cli> --version`, its settings in a throwaway pre-seeded folder (proc::kicad_probe): the probe never writes the person's KiCad
/// settings or triggers `FindPythonInterpreter`. Bounded (proc::probe_output_result): a run that hangs (#1160, macOS) has its process
/// group killed and reaped, and is asked once more; concurrent callers share one probe and results are cached by `(path, mtime, size)`.
pub fn cli_version_output_path(cli: &Path) -> Option<std::process::Output> {
    cli_version_cached_path(cli, crate::proc::PROBE_LIMIT, 2).into_output()
}

pub fn cli_version_output(cli: &str) -> Option<std::process::Output> {
    cli_version_output_path(Path::new(cli))
}

/// Uncached bounded `<cli> --version` probe (up to 2 attempts, killing and reaping the process group on timeout).
pub fn cli_version_within(cli: &str, limit: Duration) -> Option<std::process::Output> {
    probe_cli_version_uncached_until(Path::new(cli), limit, 2, None).into_output()
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
    // and with no KiCad at all, the install (missing_offer): shown after the person's Edit press, installed only on their Install press
    let folder = super::install::folder_words(&super::install::parent(&hub.cfg.home), std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(std::path::PathBuf::from).as_deref(), cfg!(windows));
    let update = super::install::update_offer(super::install::package(), &status, &version, &env.caps)
        .or_else(|| super::install::missing_offer(super::install::package(), &status, &folder, super::install::this_os())).map_or(String::new(), |v| v.to_string());
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

async fn ask_detailed(i: &Ipc, project: &str, env: &Env, ty: &str, msg: Value) -> AskOutcome {
    let deadline = std::time::Instant::now() + ASK_TIMEOUT;
    let (t, r) = oneshot::channel();
    if i.tx.send(Msg::Ask { project: project.into(), env: env.clone(), ty: ty.into(), msg, deadline, reply: t }).is_err() {
        return AskOutcome { reply: failed("no reply"), sock: String::new(), launch_gen: 0, dead_invalidated: false };
    }
    match tokio::time::timeout(ASK_TIMEOUT, r).await {
        Ok(Ok(o)) => o,
        Ok(Err(_)) => AskOutcome { reply: failed("no reply"), sock: String::new(), launch_gen: 0, dead_invalidated: false },
        Err(_) => {
            let short = ty.rsplit('.').next().unwrap_or(ty);
            let suffix = if short == "BeginCommit" { "; request outcome unknown if it had already reached KiCad" } else { "" };
            AskOutcome {
                reply: failed(&format!("KiCad IPC request {short} timed out after {} s waiting on the headless server{suffix}", ASK_TIMEOUT.as_secs())),
                sock: String::new(),
                launch_gen: 0,
                dead_invalidated: false,
            }
        }
    }
}
#[cfg_attr(not(test), allow(dead_code))]
async fn ask(i: &Ipc, project: &str, env: &Env, ty: &str, msg: Value) -> Value { ask_detailed(i, project, env, ty, msg).await.reply }
async fn explain_begin_commit_timeout_headless(i: &Ipc, pro: &str, env: &Env, orig: &Value, orig_sock: &str, orig_gen: u64) -> String {
    let name = ks::name(pro);
    let unknown = "request outcome unknown if bytes reached KiCad";
    if orig_gen == 0 || orig_sock.is_empty() {
        return format!(
            "BeginCommit timed out ({}); {unknown} — the headless KiCad server for {name} is no longer active",
            err_of(orig)
        );
    }
    let diag_wait = Duration::from_millis(PROBE_PING_TIMEOUT_MS + 1_000);
    let (rt, rx) = oneshot::channel();
    if i.tx.send(Msg::ReadySock { project: pro.into(), reply: rt }).is_err() {
        return format!("BeginCommit timed out ({}; {unknown}), and the KiCad IPC actor is not running", err_of(orig));
    }
    let ready = match tokio::time::timeout(diag_wait, rx).await {
        Ok(Ok(v)) => v,
        Ok(Err(_)) | Err(_) => {
            return format!(
                "BeginCommit timed out ({}; {unknown}), and the KiCad IPC actor did not answer readiness check in time",
                err_of(orig)
            );
        }
    };
    let Some((sock, launch_gen)) = ready else {
        return format!(
            "BeginCommit timed out ({}; {unknown}) — the headless KiCad server for {name} is no longer running (stale state was cleared; retry to start a fresh server)",
            err_of(orig)
        );
    };
    if launch_gen != orig_gen || sock != orig_sock {
        return format!(
            "BeginCommit timed out ({}; {unknown}) — the original KiCad server generation has since been replaced (not probing the newer server)",
            err_of(orig)
        );
    }
    let (pt, pr) = oneshot::channel();
    if i.tx.send(Msg::Raw {
        sock,
        env: env.clone(),
        ty: "kiapi.common.commands.Ping".into(),
        msg: json!({}),
        timeout_ms: Some(PROBE_PING_TIMEOUT_MS),
        fence: Some((pro.into(), launch_gen)),
        reply: pt,
    }).is_err() {
        return format!("BeginCommit timed out ({}; {unknown}), and the KiCad IPC actor stopped before probing health", err_of(orig));
    }
    let ping = match tokio::time::timeout(diag_wait, pr).await {
        Ok(Ok(v)) => v,
        Ok(Err(_)) => failed("no reply"),
        Err(_) => failed("follow-up Ping probe timed out waiting on transport queue"),
    };
    if ping["ok"].as_bool().unwrap_or(false) {
        format!(
            "BeginCommit timed out ({}; {unknown}) while KiCad still answers reads/Ping — a modal dialog, busy task, or stale open commit may be blocking writes; check kicad_ipc_status, drop any open commit with EndCommit (CMA_DROP), or restart KiCad",
            err_of(orig)
        )
    } else {
        let ping_err = err_of(&ping);
        let (itx, irx) = oneshot::channel();
        let dead = if i.tx.send(Msg::Invalidate {
            project: pro.into(),
            launch_gen,
            why: format!("KiCad stopped responding after BeginCommit timed out ({ping_err})"),
            reply: itx,
        }).is_ok() {
            tokio::time::timeout(diag_wait, irx).await.ok().and_then(|r| r.ok()).unwrap_or(false)
        } else {
            false
        };
        if dead {
            format!(
                "BeginCommit timed out ({}; {unknown}) and a follow-up Ping failed ({ping_err}); the headless KiCad process for {name} exited and its stale session was invalidated — retry to start a fresh server",
                err_of(orig)
            )
        } else {
            format!(
                "BeginCommit timed out ({}; {unknown}) and a follow-up Ping failed ({ping_err}); the headless KiCad process for {name} is still running and was marked unresponsive (not starting a duplicate server) — check kicad_ipc_status or restart KiCad",
                err_of(orig)
            )
        }
    }
}
async fn explain_begin_commit_timeout_gui(gui: &super::embed::GuiRoute, pro: &str, orig: &Value) -> String {
    let st = super::embed::session_state(pro);
    let dialogs: Vec<&str> = st["dialogs"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    if !dialogs.is_empty() {
        return format!(
            "BeginCommit timed out ({}; request outcome unknown if bytes reached KiCad) while KiCad has dialog(s) open ({}); close or answer the dialog in KiCad, then retry",
            err_of(orig),
            dialogs.join(", ")
        );
    }
    let ping = super::embed::gui_call_timeout(gui, "kiapi.common.commands.Ping", json!({}), "", PROBE_PING_TIMEOUT_MS).await;
    if ping["ok"].as_bool().unwrap_or(false) {
        format!(
            "BeginCommit timed out ({}; request outcome unknown if bytes reached KiCad) while KiCad still answers reads/Ping — an unreported modal dialog, busy task, or stale open commit may be blocking writes; check kicad_ipc_status, drop any open commit with EndCommit (CMA_DROP), or restart KiCad",
            err_of(orig)
        )
    } else {
        super::embed::check_exited(pro);
        format!(
            "BeginCommit timed out ({}; request outcome unknown if bytes reached KiCad) and a follow-up Ping failed ({}); KiCad may have exited or hung — check kicad_ipc_status or reopen KiCad",
            err_of(orig),
            err_of(&ping)
        )
    }
}
async fn info_on(i: &Ipc) -> String {
    let (t, r) = oneshot::channel();
    if i.tx.send(Msg::Info(t)).is_err() { return String::new() }
    tokio::time::timeout(Duration::from_millis(PROBE_PING_TIMEOUT_MS + 1_000), r).await.ok().and_then(|res| res.ok()).unwrap_or_default()
}
async fn argv_on(i: &Ipc, project: &str, env: &Env) -> String {
    let (t, r) = oneshot::channel();
    if i.tx.send(Msg::Argv { project: project.into(), env: env.clone(), reply: t }).is_err() { return String::new() }
    tokio::time::timeout(Duration::from_millis(PROBE_PING_TIMEOUT_MS + 1_000), r).await.ok().and_then(|res| res.ok()).unwrap_or_default()
}

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
/// A simulation's own commands (kicad/ksim.rs): starting a run and stopping one. A class of their own, not reads: a run changes no
/// document, so it takes no document lock and needs no snapshot (agents/design.rs `kicad_gate`, snapguard), but it does occupy the
/// project's simulator and may show in the person's, so everywhere else it is treated as a command: refused on a copied hub, held while
/// the person has paused the agent (a stop is not), not asked again by itself while KiCad is busy (ksim waits that out itself).
/// Collecting a run (GetSimulationResult) and the simulation's other Get* commands are reads.
pub fn is_sim(type_: &str) -> bool { matches!(type_.rsplit('.').next().unwrap_or(type_), "RunSimulation" | "StopSimulation") }
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
        if !status { dispatch(EMBEDDED, ty.rsplit('.').next().unwrap_or(ty))?; }
        // the person's unsaved edits are never written by an agent's save (SaveDocument, SaveDocumentAs); its commits are tracked for that.
        // A co-editing runtime (bp.11, `coedit`) is different: the agent edits the person's live document, its save includes both
        // sides' edits on purpose, and nobody is asked to save, revert, close KiCad or wait (docs/kicad-coedit.md).
        let short = ty.rsplit('.').next().unwrap_or(ty);
        let coedit = gui.caps.iter().any(|c| c == "coedit");
        if !coedit && matches!(short, "SaveDocument" | "SaveDocumentAs") {
            let why = super::embed::unsaved_update(&pro, |u| u).refuse_save(Path::new(path).file_name().and_then(|f| f.to_str()).unwrap_or(path));
            if !why.is_empty() { return Err(why) }
        }
        // (a co-editing KiCad says who made each change: there the person counts only by their own, ET_DOCUMENT_EDITED, #1169)
        if short == "BeginCommit" && !coedit { let pending = !super::embed::agent_unsaved(&pro).is_empty(); super::embed::unsaved_update(&pro, |u| u.agent_begins(pending)); }
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
            // (a simulation run waits like a write: it would show in the paused person's Simulator; stopping one never waits)
            if !is_read(ty) && short != "StopSimulation" { held = super::coedit::wait_resumed(th).await; }
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
        let ok = o["ok"].as_bool().unwrap_or(false);
        if !ok && short == "BeginCommit" { unpin() }
        if !ok && is_conn_error(&o) { super::embed::check_exited(&pro); }
        if !status && !ok && short == "BeginCommit" && is_timeout_error(&o) {
            return Err(explain_begin_commit_timeout_gui(&gui, &pro, &o).await);
        }
        if o["fallback"].as_bool().unwrap_or(false) { log_fallback(hub, thread, o["why"].as_str().unwrap_or("")).await; return Err(o["error"].as_str().unwrap_or("").into()) }
        let live = super::embed::gui_route(&pro).is_some() && !is_conn_error(&o);
        let servers = format!("{pro}|{}|gui ({})", if live { "ready" } else { "stopped" }, gui.mode);
        if status {
            // KiCad's open dialogs and its notices (embed-notice: "Could not use OpenGL, falling back to software rendering"): the agent says them
            let st = super::embed::session_state(&pro);
            return Ok(json!({"server":format!("gui ({})", gui.mode),"servers":servers,"argv":gui.argv,"version":gui.version,"capabilities":o["message"].clone(),"detail":if ok { String::new() } else { err_of(&o) },
                "dialogs":st["dialogs"].clone(),"dialogs_reported":st["dialogs_reported"].clone(),"notices":st["notices"].clone()}))
        }
        if !ok { return Err(if short == "SaveDocument" { super::savegate::explain_save_error(&err_of(&o), super::savegate::saves_design(&msg_doc, path)) } else { err_of(&o) }) }
        let wrote_nothing = short == "SaveDocument" && super::savegate::wrote_nothing(&o);
        let mut o = o; let mut out = json!({"type":o["type"].as_str().unwrap_or(""),"message":o["message"].take()}); // (taken, not copied: a simulation's vectors)
        // an agent's save of a board KiCad changed on load (its 3D model migration): said, so the agent reports it rather than owns it
        if wrote_nothing { out["not_saved"] = json!(super::savegate::NOTHING_CHANGED); }
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
    if !status { dispatch(HEADLESS, ty.rsplit('.').next().unwrap_or(ty))?; }
    let mut outcome = ask_detailed(i, &pro, &env, ty, msg.clone()).await;
    if !status && outcome.dead_invalidated && is_dial_refused(&outcome.reply) {
        outcome = ask_detailed(i, &pro, &env, ty, msg).await;
    }
    let (o, orig_sock, orig_gen) = (outcome.reply, outcome.sock, outcome.launch_gen);
    let ok = o["ok"].as_bool().unwrap_or(false);
    let short = ty.rsplit('.').next().unwrap_or(ty);
    if !ok && short == "BeginCommit" { unpin() }
    if !status && !ok && short == "BeginCommit" && is_timeout_error(&o) {
        return Err(explain_begin_commit_timeout_headless(i, &pro, &env, &o, &orig_sock, orig_gen).await);
    }
    if o["fallback"].as_bool().unwrap_or(false) { log_fallback(hub, thread, o["why"].as_str().unwrap_or("")).await; return Err(o["error"].as_str().unwrap_or("").into()) }
    if status {
        let lines = info_on(i).await;
        let argv = argv_on(i, &pro, &env).await;
        return Ok(json!({"server":ks::note(&lines),"servers":lines,"argv":argv,"version":env.version,"capabilities":o["message"].clone(),"detail":if ok { String::new() } else { err_of(&o) }}));
    }
    if !ok { return Err(err_of(&o)) }
    let mut o = o; let mut out = json!({"type":o["type"].as_str().unwrap_or(""),"message":o["message"].take()}); // (taken, not copied: a simulation's vectors)
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

/// Which KiCad one tool call's requests really went to (#1192): each request that was dispatched (sent to the editor's KiCad or to the
/// headless one, whatever its answer) is recorded, a request refused before that is not. A commit begun on one of them is kept there: while
/// it is open, a request that would now go to the other is refused, never continued on a KiCad that does not have the commit.
#[derive(Default)]
pub struct Served { pub sent: Vec<&'static str>, pinned: Option<&'static str> }
impl Served {
    /// "headless", "embedded", "both" (it changed between requests), or None when nothing was dispatched.
    pub fn word(&self) -> Option<&'static str> {
        let first = *self.sent.first()?; Some(if self.sent.iter().all(|s| *s == first) { first } else { "both" })
    }
    /// The KiCad this call's last request went to (EMBEDDED or HEADLESS), None before any: where its next one goes too (#1224).
    pub fn last(&self) -> Option<&'static str> { self.sent.last().copied() }
}
tokio::task_local! { pub static SERVED: std::cell::RefCell<Served>; }
pub const EMBEDDED: &str = "embedded";
pub const HEADLESS: &str = "headless";
/// Before a request is sent to `route`: refused when this call's open commit is on the other KiCad; else recorded as sent there.
fn dispatch(route: &'static str, short: &str) -> Result<(), String> {
    SERVED.try_with(|s| { let mut s = s.borrow_mut();
        if let Some(p) = s.pinned.filter(|p| *p != route) {
            return Err(format!("this edit's commit is open in the {p} KiCad, and the project's requests now go to the {route} one (the person's KiCad {} the project meanwhile): {short} was not sent, so nothing of the edit lands on a KiCad that lacks the commit. Run the edit again.", if route == EMBEDDED { "opened" } else { "closed" }))
        }
        s.sent.push(route);
        if short == "BeginCommit" { s.pinned = Some(route) } else if short == "EndCommit" { s.pinned = None }
        Ok(())
    }).unwrap_or(Ok(()))
}

/// A BeginCommit that KiCad refused opened no commit: nothing is kept on its KiCad.
fn unpin() { let _ = SERVED.try_with(|s| s.borrow_mut().pinned = None); }

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
    #[test] fn served_by_counts_what_was_sent_and_a_commit_stays_on_its_kicad() {
        use super::{dispatch, unpin, Served, SERVED, EMBEDDED, HEADLESS};
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let word = |f: &dyn Fn()| rt.block_on(SERVED.scope(std::cell::RefCell::new(Served::default()), async { f(); SERVED.with(|s| s.borrow().word()) }));
        assert_eq!(word(&|| {}), None, "nothing sent: no KiCad served it");
        assert_eq!(word(&|| { dispatch(HEADLESS, "GetItems").unwrap(); dispatch(HEADLESS, "CreateItems").unwrap(); }), Some("headless"));
        assert_eq!(word(&|| { dispatch(EMBEDDED, "GetItems").unwrap(); }), Some("embedded"));
        assert_eq!(word(&|| { dispatch(HEADLESS, "GetItems").unwrap(); dispatch(EMBEDDED, "GetItems").unwrap(); }), Some("both"), "the route changed between two reads");
        // a commit begun headless is not continued in the editor's KiCad: refused before it is sent, and not counted as sent there
        assert_eq!(word(&|| { dispatch(HEADLESS, "BeginCommit").unwrap(); let e = dispatch(EMBEDDED, "CreateItems").unwrap_err(); assert!(e.contains("commit is open in the headless KiCad") && e.contains("opened"), "{e}");
            dispatch(HEADLESS, "EndCommit").unwrap(); dispatch(EMBEDDED, "GetItems").unwrap(); }), Some("both"), "once the commit has ended the next request goes where the project now is");
        assert_eq!(word(&|| { dispatch(EMBEDDED, "BeginCommit").unwrap(); assert!(dispatch(HEADLESS, "SaveDocument").unwrap_err().contains("closed")); }), Some("embedded"));
        // a refused BeginCommit opened nothing
        assert_eq!(word(&|| { dispatch(HEADLESS, "BeginCommit").unwrap(); unpin(); dispatch(EMBEDDED, "GetItems").unwrap(); }), Some("both"));
        // outside a kicad_edit (kicad_ipc, the hub's own calls) nothing is counted or refused
        dispatch(HEADLESS, "BeginCommit").unwrap(); dispatch(EMBEDDED, "CreateItems").unwrap();
    }
    /// A stand-in kicad-cli that, like KiCad, writes its settings: to $KICAD_CONFIG_HOME when set, else to `person` (the person's
    /// kicad_common.json); it logs the folder it was given and answers --version and api-server --describe.
    #[cfg(unix)] fn writing_cli(dir: &std::path::Path, person: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let (cli, log) = (dir.join("kicad-cli"), dir.join("probe.log"));
        std::fs::write(&cli, format!("#!/bin/sh\n[ \"$1\" = {NOOP} ] && exit 0\necho \"$KICAD_CONFIG_HOME\" >> '{log}'\nif [ -n \"$KICAD_CONFIG_HOME\" ]; then mkdir -p \"$KICAD_CONFIG_HOME/10.0\"; echo rewritten > \"$KICAD_CONFIG_HOME/10.0/kicad_common.json\"; else echo rewritten > '{person}'; fi\nif [ \"$1\" = api-server ]; then echo '{{\"version\":\"10.99.0-bp.13\"}}'; else echo 10.0.8; fi\n", log = log.display(), person = person.display())).unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        started(&cli);
        (cli, log)
    }
    /// The stand-ins' argument that makes them exit at once, doing nothing (logged nowhere): `started` runs it.
    const NOOP: &str = "--bp-test-noop";
    /// "Text file busy": a script written a moment ago can still be open for writing in a child another test thread just forked, until that
    /// child's exec (longer on a loaded box), and the probe's spawn then fails as if KiCad did not run. Run the stand-in with NOOP until it
    /// starts: the same approach as `wait_execs` in agents/proc.rs's tests (4cfb8c3356) and the reaper tests' group() (799609fe53), with
    /// the NOOP argument because these stand-ins act when run (the hung one records its pid and hangs).
    #[cfg(unix)] fn started(cli: &std::path::Path) {
        let t = std::time::Instant::now();
        loop { match std::process::Command::new(cli).arg(NOOP).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status() {
            Err(e) if e.raw_os_error() == Some(crate::plat::errno::ETXTBSY) && t.elapsed() < std::time::Duration::from_secs(120) => std::thread::sleep(std::time::Duration::from_millis(25)),
            _ => break } }
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
        std::fs::write(&cli, format!("#!/bin/sh\n[ \"$1\" = {NOOP} ] && exit 0\necho $$ >> '{p}'\nif [ ! -e '{o}' ] || [ -e '{o}.always' ]; then touch '{o}'; trap '' TERM; exec sleep 60; fi\necho 10.99.0\n", p = pids.display(), o = once.display())).unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        started(&cli);
        let gone = |pid: &str| { let r = std::process::Command::new("kill").args(["-0", pid]).stderr(std::process::Stdio::null()).status().unwrap(); !r.success() };
        let ran = || -> Vec<String> { std::fs::read_to_string(&pids).unwrap_or_default().lines().map(String::from).collect() };
        // the limit the test gives the probe (the hub's own, PROBE_LIMIT, is not the test's): the hung run is killed at it, and the run that
        // answers must start a shell and say the version within it. 1.5 s was too tight on a loaded box (a land gate at load 25-29); 5 s is
        // not, and the test is 15 s at most. What is proven is unchanged: a run hung, was killed and reaped, the probe asked once more and
        // that run answered; elapsed time is only bounded, generously
        let limit = std::time::Duration::from_secs(5);
        let at = std::time::Instant::now();
        let out = cli_version_within(&cli.to_string_lossy(), limit).expect("the second run answers");
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "10.99.0");
        assert!(at.elapsed() < limit * 2 + std::time::Duration::from_secs(60), "bounded: {:?}", at.elapsed());
        let first = ran();
        assert_eq!(first.len(), 2, "asked twice: {first:?}");
        assert!(gone(&first[0]), "the hung run is killed and reaped, not left alive or a zombie: pid {}", first[0]);

        std::fs::write(once.with_extension("always"), "").unwrap(); std::fs::remove_file(&pids).unwrap();
        let at = std::time::Instant::now();
        assert!(cli_version_within(&cli.to_string_lossy(), limit).is_none(), "a KiCad that always hangs has no version");
        assert!(at.elapsed() < limit * 2 + std::time::Duration::from_secs(60), "bounded: {:?}", at.elapsed());
        let always = ran();
        assert_eq!(always.len(), 2, "{always:?}"); for p in &always { assert!(gone(p), "pid {p} left behind"); }
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
        let mut a = Actor { me: tx.clone(), home: String::new(), rt, hub_pid: std::process::id(), helper: None, gen: 0, pend: HashMap::new(), next: 0, srvs: vec![], srv_gen: HashMap::new(), wait: vec![], shown: String::new(), env: Env::default(), lines: lines.clone(), timeout_override_ms: None };
        tokio::spawn(async move { while let Some(m) = rx.recv().await { a.handle(m) } });
        let env = Env { cli, version: "10.99.0-bp.4".into(), ..Default::default() };
        let (t, r) = oneshot::channel();
        tx.send(Msg::Ask { project: project.clone(), env, ty: "kiapi.common.commands.GetVersion".into(), msg: json!({}), deadline: std::time::Instant::now() + ASK_TIMEOUT, reply: t }).unwrap();
        let o = tokio::time::timeout(Duration::from_secs(10), r).await.expect("an answer in time").unwrap().reply;
        assert_eq!(o["ok"], json!(true), "{o}"); assert_eq!(o["message"]["version"]["full_version"], json!("10.99.0-unknown-bp.4"));
        let asked = srv.await.unwrap();
        assert!(asked[0].ends_with("commands.Ping") && asked[1].ends_with("commands.GetVersion") && asked[2].ends_with("commands.GetVersion"), "{asked:?}");
        assert_eq!(lines.lock().unwrap().clone(), format!("{project}|ready|"));
        for _ in 0..40 { tokio::time::sleep(Duration::from_millis(100)).await; if lines.lock().unwrap().contains("|stopped|") { break } }
        assert_eq!(lines.lock().unwrap().clone(), format!("{project}|stopped|"));
    }
    #[test] fn a_simulation_run_is_a_class_of_its_own() {
        for t in ["kiapi.backplane.sim.RunSimulation", "kiapi.backplane.sim.StopSimulation"] { assert!(is_sim(t) && !is_read(t) && !patient_of(t) && copy_ipc_refused(true, t).is_some(), "{t}") }
        for t in ["kiapi.backplane.sim.GetSimulationResult", "kiapi.backplane.sim.GetSimulationModels"] { assert!(is_read(t) && !is_sim(t), "{t}") }
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

    struct MockTransport {
        tx: mpsc::UnboundedSender<Value>,
        alive: Arc<std::sync::Mutex<HashMap<String, bool>>>,
        pids: Arc<std::sync::Mutex<HashMap<String, u32>>>,
    }
    impl super::Transport for MockTransport {
        fn send(&mut self, fields: Value) -> bool { let _ = self.tx.send(fields); true }
        fn process_alive(&self, key: &str) -> Option<bool> { self.alive.lock().unwrap().get(key).copied() }
        fn pid_of(&self, key: &str) -> u32 { self.pids.lock().unwrap().get(key).copied().unwrap_or(0) }
    }

    fn mock_actor(rt: String) -> (
        Ipc,
        mpsc::UnboundedReceiver<Value>,
        mpsc::UnboundedSender<Msg>,
        Arc<std::sync::Mutex<HashMap<String, bool>>>,
        Arc<std::sync::Mutex<HashMap<String, u32>>>,
    ) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (ftx, frx) = mpsc::unbounded_channel();
        let alive = Arc::new(std::sync::Mutex::new(HashMap::new()));
        let pids = Arc::new(std::sync::Mutex::new(HashMap::new()));
        let lines = Arc::new(std::sync::Mutex::new(String::new()));
        let mut a = Actor {
            me: tx.clone(),
            home: String::new(),
            rt,
            hub_pid: std::process::id(),
            helper: Some(Helper {
                link: Box::new(MockTransport { tx: ftx, alive: alive.clone(), pids: pids.clone() }),
                gen: 1,
            }),
            gen: 1,
            pend: HashMap::new(),
            next: 0,
            srvs: vec![],
            srv_gen: HashMap::new(),
            wait: vec![],
            shown: String::new(),
            env: Env::default(),
            lines: lines.clone(),
            timeout_override_ms: None,
        };
        tokio::spawn(async move { while let Some(m) = rx.recv().await { a.handle(m) } });
        (Ipc { tx: tx.clone(), lines }, frx, tx, alive, pids)
    }

    #[tokio::test]
    async fn process_exit_invalidates_ready_state_and_clears_stale_socket() {
        let root = crate::plat::tmp::short_dir("ipc-exit");
        let rt = root.to_string_lossy().to_string();
        let project = "/p/exit.kicad_pro".to_string();
        let dir = ks::dir(&rt, "", &project);
        std::fs::create_dir_all(&dir).unwrap();
        let sock = format!("{dir}/api.sock");
        std::fs::write(&sock, "stale").unwrap();

        let (ipc, mut frx, tx, alive, pids) = mock_actor(rt);
        alive.lock().unwrap().insert(project.clone(), true);
        pids.lock().unwrap().insert(project.clone(), 12345);
        let env = Env { cli: "/k/kicad-cli".into(), version: "10.99.0-bp.13".into(), ..Default::default() };

        let (atx, arx) = oneshot::channel();
        tx.send(Msg::Ask { project: project.clone(), env: env.clone(), ty: "kiapi.common.commands.Ping".into(), msg: json!({}), deadline: std::time::Instant::now() + ASK_TIMEOUT, reply: atx }).unwrap();
        let spawn = frx.recv().await.unwrap();
        assert_eq!(spawn["op"], "spawn");
        let launch_gen = spawn["gen"].as_u64().unwrap();
        tx.send(Msg::Line(1, json!({"event":"line","key":project,"gen":launch_gen,"text":format!("KiCad API server listening at ipc://{sock}")}))).unwrap();
        for _ in 0..2 {
            let req = frx.recv().await.unwrap();
            let id = req["id"].clone();
            let msg = if req["type"].as_str().unwrap().ends_with("GetVersion") { json!({"version":{"full_version":"10.99.0-bp.13"}}) } else { json!({}) };
            tx.send(Msg::Line(1, json!({"id":id,"ok":true,"type":"r","message":msg}))).unwrap();
        }
        let user_req = frx.recv().await.unwrap();
        tx.send(Msg::Line(1, json!({"id":user_req["id"],"ok":true,"type":"PingResponse","message":{}}))).unwrap();
        assert_eq!(arx.await.unwrap().reply["ok"], json!(true));
        assert_eq!(info_on(&ipc).await, format!("{project}|ready|"));

        // Process dies without delivering an exit event yet -> Info / Ask reaps it immediately.
        alive.lock().unwrap().insert(project.clone(), false);
        let info = info_on(&ipc).await;
        assert!(info.contains("|stopped|") && !info.contains("|ready|"), "got: {info}");
        #[cfg(unix)]
        assert!(!std::path::Path::new(&sock).exists(), "stale socket should be removed on process exit");

        // Next request sees stopped state and launches a fresh server instead of talking to dead socket.
        alive.lock().unwrap().insert(project.clone(), true);
        let (atx2, _arx2) = oneshot::channel();
        tx.send(Msg::Ask { project: project.clone(), env, ty: "kiapi.common.commands.Ping".into(), msg: json!({}), deadline: std::time::Instant::now() + ASK_TIMEOUT, reply: atx2 }).unwrap();
        let mut saw_spawn = false;
        while let Ok(Some(msg)) = tokio::time::timeout(Duration::from_millis(500), frx.recv()).await {
            if msg["op"] == "spawn" { saw_spawn = true; break }
        }
        assert!(saw_spawn, "new request after process exit must spawn fresh server");
    }

    #[tokio::test]
    async fn connection_drop_on_ready_server_invalidates_readiness_immediately() {
        let root = crate::plat::tmp::short_dir("ipc-drop");
        let rt = root.to_string_lossy().to_string();
        let project = "/p/drop.kicad_pro".to_string();
        let dir = ks::dir(&rt, "", &project);
        std::fs::create_dir_all(&dir).unwrap();
        let sock = format!("{dir}/api.sock");
        std::fs::write(&sock, "live").unwrap();

        let (ipc, mut frx, tx, alive, pids) = mock_actor(rt);
        alive.lock().unwrap().insert(project.clone(), true);
        pids.lock().unwrap().insert(project.clone(), 4321);
        let env = Env { cli: "/k/kicad-cli".into(), version: "10.99.0-bp.13".into(), ..Default::default() };

        let (atx, arx) = oneshot::channel();
        tx.send(Msg::Ask { project: project.clone(), env: env.clone(), ty: "kiapi.common.commands.Ping".into(), msg: json!({}), deadline: std::time::Instant::now() + ASK_TIMEOUT, reply: atx }).unwrap();
        let spawn = frx.recv().await.unwrap();
        let launch_gen = spawn["gen"].as_u64().unwrap();
        tx.send(Msg::Line(1, json!({"event":"line","key":project,"gen":launch_gen,"text":format!("KiCad API server listening at ipc://{sock}")}))).unwrap();
        for _ in 0..2 {
            let req = frx.recv().await.unwrap();
            let msg = if req["type"].as_str().unwrap().ends_with("GetVersion") { json!({"version":{"full_version":"10.99.0-bp.13"}}) } else { json!({}) };
            tx.send(Msg::Line(1, json!({"id":req["id"],"ok":true,"type":"r","message":msg}))).unwrap();
        }
        let user_req = frx.recv().await.unwrap();
        tx.send(Msg::Line(1, json!({"id":user_req["id"],"ok":false,"error":format!("dial ipc://{sock}: Connection refused")}))).unwrap();
        let ans = arx.await.unwrap();
        assert_eq!(ans.reply["ok"], json!(false));
        assert!(!ans.dead_invalidated, "live process must not report dead_invalidated");
        let info = info_on(&ipc).await;
        assert!(info.contains("|unresponsive|") && !info.contains("|ready|"), "live connection drop must mark unresponsive immediately: {info}");
        #[cfg(unix)]
        assert!(std::path::Path::new(&sock).exists(), "live process socket must not be unlinked");

        // Next call while alive must reconnect on existing generation (NO second spawn!) and promote back to ready on success.
        let (atx2, arx2) = oneshot::channel();
        tx.send(Msg::Ask { project: project.clone(), env, ty: "kiapi.common.commands.Ping".into(), msg: json!({}), deadline: std::time::Instant::now() + ASK_TIMEOUT, reply: atx2 }).unwrap();
        let mut msg2 = frx.recv().await.unwrap();
        if msg2["op"] == "close" { msg2 = frx.recv().await.unwrap(); }
        assert_ne!(msg2["op"], "spawn", "must never spawn a duplicate KiCad while process is alive");
        assert_eq!(msg2["type"], "kiapi.common.commands.Ping");
        assert_eq!(msg2["sock"], sock);
        tx.send(Msg::Line(1, json!({"id":msg2["id"],"ok":true,"type":"PingResponse","message":{}}))).unwrap();
        assert_eq!(arx2.await.unwrap().reply["ok"], json!(true));
        assert!(info_on(&ipc).await.contains("|ready|"), "successful call must promote unresponsive back to ready");
    }

    #[tokio::test]
    async fn stale_handshake_up_cannot_resurrect_exited_server() {
        let root = crate::plat::tmp::short_dir("ipc-gen");
        let rt = root.to_string_lossy().to_string();
        let project = "/p/gen.kicad_pro".to_string();
        let dir = ks::dir(&rt, "", &project);
        std::fs::create_dir_all(&dir).unwrap();
        let sock1 = format!("{dir}/s1.sock");
        let sock2 = format!("{dir}/s2.sock");
        #[cfg(unix)]
        let (_l1, _l2) = (crate::plat::ipc::bind(&sock1).unwrap(), crate::plat::ipc::bind(&sock2).unwrap());
        let (ipc, mut frx, tx, alive, _pids) = mock_actor(rt);
        alive.lock().unwrap().insert(project.clone(), true);
        let env = Env { cli: "/k/kicad-cli".into(), version: "10.99.0-bp.13".into(), ..Default::default() };

        let (atx, _arx) = oneshot::channel();
        tx.send(Msg::Ask { project: project.clone(), env, ty: "kiapi.common.commands.Ping".into(), msg: json!({}), deadline: std::time::Instant::now() + ASK_TIMEOUT, reply: atx }).unwrap();
        let spawn1 = frx.recv().await.unwrap();
        let g1 = spawn1["gen"].as_u64().unwrap();
        tx.send(Msg::Line(1, json!({"event":"line","key":project,"gen":g1,"text":format!("KiCad API server listening at ipc://{sock1}")}))).unwrap();
        // Process crashes once -> enters "starting" retry state (awaiting Go).
        tx.send(Msg::Line(1, json!({"event":"exit","key":project,"gen":g1,"code":1,"tail":"boom"}))).unwrap();
        // Trigger retry -> launches gen 2 and gen 2 is now in "starting" WITH a non-empty socket.
        tx.send(Msg::Go { project: project.clone(), launch_gen: g1 }).unwrap();
        let spawn2 = loop {
            let msg = frx.recv().await.unwrap();
            if msg["op"] == "spawn" { break msg }
        };
        let g2 = spawn2["gen"].as_u64().unwrap();
        assert_ne!(g2, g1);
        tx.send(Msg::Line(1, json!({"event":"line","key":project,"gen":g2,"text":format!("KiCad API server listening at ipc://{sock2}")}))).unwrap();
        // Late Up for generation g1 arrives while gen 2 is "starting" with sock2 populated: must be ignored.
        tx.send(Msg::Up { project: project.clone(), version: "10.99.0-bp.13".into(), launch_gen: g1 }).unwrap();
        let info = info_on(&ipc).await;
        assert!(!info.contains("|ready|"), "stale Up must not mark replacement generation ready: {info}");
        // Genuine Up for gen 2 marks it ready.
        tx.send(Msg::Up { project: project.clone(), version: "10.99.0-bp.13".into(), launch_gen: g2 }).unwrap();
        let info2 = info_on(&ipc).await;
        assert!(info2.contains("|ready|"), "current generation Up must mark server ready: {info2}");
    }

    #[tokio::test]
    async fn dial_refused_recovers_by_starting_fresh_server() {
        let root = crate::plat::tmp::short_dir("ipc-retry");
        let rt = root.to_string_lossy().to_string();
        let project = "/p/retry.kicad_pro".to_string();
        let dir = ks::dir(&rt, "", &project);
        std::fs::create_dir_all(&dir).unwrap();
        let sock1 = format!("{dir}/s1.sock");
        let sock2 = format!("{dir}/s2.sock");
        #[cfg(unix)]
        let (_l1, _l2) = (crate::plat::ipc::bind(&sock1).unwrap(), crate::plat::ipc::bind(&sock2).unwrap());
        let (ipc, mut frx, tx, alive, _pids) = mock_actor(rt);
        alive.lock().unwrap().insert(project.clone(), true);
        let env = Env { cli: "/k/kicad-cli".into(), version: "10.99.0-bp.13".into(), ..Default::default() };

        // Start initial ready server (gen 1).
        let (atx0, arx0) = oneshot::channel();
        tx.send(Msg::Ask { project: project.clone(), env: env.clone(), ty: "kiapi.common.commands.Ping".into(), msg: json!({}), deadline: std::time::Instant::now() + ASK_TIMEOUT, reply: atx0 }).unwrap();
        let sp1 = frx.recv().await.unwrap();
        let g1 = sp1["gen"].as_u64().unwrap();
        tx.send(Msg::Line(1, json!({"event":"line","key":project,"gen":g1,"text":format!("KiCad API server listening at ipc://{sock1}")}))).unwrap();
        for _ in 0..2 {
            let req = frx.recv().await.unwrap();
            let msg = if req["type"].as_str().unwrap().ends_with("GetVersion") { json!({"version":{"full_version":"10.99.0-bp.13"}}) } else { json!({}) };
            tx.send(Msg::Line(1, json!({"id":req["id"],"ok":true,"type":"r","message":msg}))).unwrap();
        }
        let u0 = frx.recv().await.unwrap();
        tx.send(Msg::Line(1, json!({"id":u0["id"],"ok":true,"type":"PingResponse","message":{}}))).unwrap();
        assert_eq!(arx0.await.unwrap().reply["ok"], json!(true));

        // Drive mock responses:
        // (1) dial refused while process is alive -> unresponsive (dead_invalidated = false, no duplicate spawn)
        // (2) dial refused after process exits -> stopped & dead_invalidated = true -> retry launches gen 2 and succeeds.
        let proj_drive = project.clone();
        let alive_drive = alive.clone();
        let s1_clone = sock1.clone();
        let s2_clone = sock2.clone();
        let drive = tokio::spawn(async move {
            // (1) Alive process: dial refused marks unresponsive, next ask reuses gen 1 (not spawn).
            let u1 = frx.recv().await.unwrap();
            assert_eq!(u1["type"], "kiapi.common.commands.Ping");
            tx.send(Msg::Line(1, json!({"id":u1["id"],"ok":false,"error":format!("dial ipc://{s1_clone}: Connection refused")}))).unwrap();

            // (2) Next ask hits existing socket without spawning; now simulate dead process before sending dial refused.
            let u2 = loop {
                let msg = frx.recv().await.unwrap();
                if msg["op"] == "close" { continue }
                assert_ne!(msg["op"], "spawn", "alive unresponsive server must never spawn a duplicate KiCad");
                break msg
            };
            assert_eq!(u2["type"], "kiapi.common.commands.Ping");
            alive_drive.lock().unwrap().insert(proj_drive.clone(), false);
            tx.send(Msg::Line(1, json!({"id":u2["id"],"ok":false,"error":format!("dial ipc://{s1_clone}: Connection refused")}))).unwrap();

            let sp2 = loop {
                let msg = frx.recv().await.unwrap();
                if msg["op"] == "spawn" { break msg }
            };
            let g2 = sp2["gen"].as_u64().unwrap();
            assert_ne!(g2, g1);
            tx.send(Msg::Line(1, json!({"event":"line","key":proj_drive,"gen":g2,"text":format!("KiCad API server listening at ipc://{s2_clone}")}))).unwrap();
            for _ in 0..2 {
                let req = frx.recv().await.unwrap();
                let msg = if req["type"].as_str().unwrap().ends_with("GetVersion") { json!({"version":{"full_version":"10.99.0-bp.13"}}) } else { json!({}) };
                tx.send(Msg::Line(1, json!({"id":req["id"],"ok":true,"type":"r","message":msg}))).unwrap();
            }
            let u3 = frx.recv().await.unwrap();
            tx.send(Msg::Line(1, json!({"id":u3["id"],"ok":true,"type":"PingResponse","message":{}}))).unwrap();
        });

        let o_alive = ask_detailed(&ipc, &project, &env, "kiapi.common.commands.Ping", json!({})).await;
        assert!(!o_alive.dead_invalidated && is_dial_refused(&o_alive.reply));
        assert!(info_on(&ipc).await.contains("|unresponsive|"));

        let mut outcome = ask_detailed(&ipc, &project, &env, "kiapi.common.commands.Ping", json!({})).await;
        assert!(outcome.dead_invalidated && is_dial_refused(&outcome.reply));
        alive.lock().unwrap().insert(project.clone(), true);
        if outcome.dead_invalidated && is_dial_refused(&outcome.reply) {
            outcome = ask_detailed(&ipc, &project, &env, "kiapi.common.commands.Ping", json!({})).await;
        }
        assert_eq!(outcome.reply["ok"], json!(true), "retry after dead server invalidated must succeed: {}", outcome.reply);
        drive.await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn begin_commit_timeout_while_reads_work_and_when_server_hangs() {
        use super::super::nng::tests as stand_in;
        use crate::plat::fs::PermissionsExt;

        let version: Vec<u8> = {
            let h = "0a040a02746b120208011a5c0a3c747970652e676f6f676c65617069732e636f6d2f6b696170692e636f6d6d6f6e2e636f6d6d616e64732e47657456657273696f6e526573706f6e7365121c0a1a080a1063221431302e39392e302d756e6b6e6f776e2d62702e34";
            (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap()).collect()
        };
        let ping_ok: Vec<u8> = {
            // ApiResponse { header: { kicad_token: "tk" }, status: { status: AS_OK (1) } }
            let h = "0a040a02746b12020801";
            (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap()).collect()
        };

        let root = crate::plat::tmp::short_dir("ipc-bctmo");
        let rt = root.to_string_lossy().to_string();
        let desc_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .find(|d| d.join("test/fixtures/kipc/api.desc").is_file())
            .unwrap()
            .join("test/fixtures/kipc/api.desc");
        let env_root = root.join("rt");
        std::fs::create_dir_all(env_root.join("share/backplane")).unwrap();
        std::fs::copy(&desc_src, env_root.join("share/backplane/api.desc")).unwrap();

        let project = "/p/bctmo.kicad_pro".to_string();
        let dir = ks::dir(&rt, "", &project);
        std::fs::create_dir_all(&dir).unwrap();
        let sock = format!("{dir}/api.sock");
        let l = crate::plat::ipc::bind(&sock).unwrap();
        let cli = format!("{rt}/kicad-cli");
        std::fs::write(&cli, "#!/bin/sh\necho \"KiCad API server listening at ipc://$4\"\nsleep 15\n").unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();

        let ver_b = version.clone();
        let ping_b = ping_ok.clone();
        let srv = tokio::spawn(async move {
            // Conn 1: Ping + GetVersion (handshake), then BeginCommit (silently dropped -> timeout!)
            let (mut s1, _) = stand_in::accept(&l).await;
            for _ in 0..2 {
                let (id, _) = stand_in::read_req(&mut s1).await.unwrap();
                stand_in::write_rep(&mut s1, id, &ver_b).await;
            }
            let (_id_bc, req_bc) = stand_in::read_req(&mut s1).await.unwrap();
            assert!(String::from_utf8_lossy(&req_bc).contains("BeginCommit"));
            // Do NOT reply on s1 -> triggers timeout_override_ms (250 ms) and drops s1.

            // Case (a): Conn 2 (after timeout dropped s1): follow-up Ping SUCCEEDS (reads work while BeginCommit timed out).
            let (mut s2, _) = stand_in::accept(&l).await;
            let (id_p2, req_p2) = stand_in::read_req(&mut s2).await.unwrap();
            assert!(String::from_utf8_lossy(&req_p2).contains("Ping"));
            stand_in::write_rep(&mut s2, id_p2, &ping_b).await;

            // Case (b): Another BeginCommit times out on s2...
            let (_id_bc2, req_bc2) = stand_in::read_req(&mut s2).await.unwrap();
            assert!(String::from_utf8_lossy(&req_bc2).contains("BeginCommit"));
            // ...and on Conn 3, follow-up Ping ALSO hangs (server wedged)!
            let (mut s3, _) = stand_in::accept(&l).await;
            let (_id_p3, _) = stand_in::read_req(&mut s3).await.unwrap();
            tokio::time::sleep(Duration::from_secs(6)).await;
            drop((s1, s2, s3));
        });

        let (tx, mut rx) = mpsc::unbounded_channel();
        let lines = Arc::new(std::sync::Mutex::new(String::new()));
        let mut a = Actor {
            me: tx.clone(), home: String::new(), rt, hub_pid: std::process::id(),
            helper: None, gen: 0, pend: HashMap::new(), next: 0,
            srvs: vec![], srv_gen: HashMap::new(), wait: vec![], shown: String::new(),
            env: Env::default(), lines: lines.clone(), timeout_override_ms: Some(250),
        };
        tokio::spawn(async move { while let Some(m) = rx.recv().await { a.handle(m) } });
        let ipc = Ipc { tx, lines };
        let env = Env {
            cli,
            root: env_root.to_string_lossy().into(),
            caps: vec!["api-descriptor".into()],
            version: "10.99.0-bp.4".into(),
            ..Default::default()
        };

        // Case (a): BeginCommit times out in ~250ms, Ping succeeds -> actionable error saying reads/Ping still work, server stays ready.
        let o1 = ask_detailed(&ipc, &project, &env, "kiapi.common.commands.BeginCommit", json!({})).await;
        assert!(is_timeout_error(&o1.reply), "expected timeout error, got: {}", o1.reply);
        let msg1 = explain_begin_commit_timeout_headless(&ipc, &project, &env, &o1.reply, &o1.sock, o1.launch_gen).await;
        assert!(msg1.contains("still answers reads/Ping") && msg1.contains("CMA_DROP"), "unexpected msg1: {msg1}");
        assert!(info_on(&ipc).await.contains("|ready|"), "server should still be ready when Ping succeeded");

        // Case (b): BeginCommit times out, follow-up Ping fails while process is still alive -> marked unresponsive (no duplicate server spawn).
        let o2 = ask_detailed(&ipc, &project, &env, "kiapi.common.commands.BeginCommit", json!({})).await;
        assert!(is_timeout_error(&o2.reply), "expected timeout error, got: {}", o2.reply);
        let msg2 = explain_begin_commit_timeout_headless(&ipc, &project, &env, &o2.reply, &o2.sock, o2.launch_gen).await;
        assert!(msg2.contains("follow-up Ping failed") && msg2.contains("unresponsive"), "unexpected msg2: {msg2}");
        let info_after = info_on(&ipc).await;
        assert!(info_after.contains("|unresponsive|") && !info_after.contains("|ready|"), "server must be marked unresponsive while process lives: {info_after}");
        srv.abort();
    }

    #[tokio::test]
    async fn expired_or_dropped_waiting_begin_commit_is_never_dispatched() {
        let root = crate::plat::tmp::short_dir("ipc-wait-exp");
        let rt = root.to_string_lossy().to_string();
        let project = "/p/waitexp.kicad_pro".to_string();
        let dir = ks::dir(&rt, "", &project);
        std::fs::create_dir_all(&dir).unwrap();
        let sock = format!("{dir}/api.sock");
        let (ipc, mut frx, tx, alive, _pids) = mock_actor(rt);
        alive.lock().unwrap().insert(project.clone(), true);
        let env = Env { cli: "/k/kicad-cli".into(), version: "10.99.0-bp.13".into(), ..Default::default() };

        // Request 1: short deadline that expires while waiting in "starting".
        let (t1, r1) = oneshot::channel();
        tx.send(Msg::Ask {
            project: project.clone(),
            env: env.clone(),
            ty: "kiapi.common.commands.BeginCommit".into(),
            msg: json!({}),
            deadline: std::time::Instant::now() + Duration::from_millis(40),
            reply: t1,
        }).unwrap();
        // Request 2: receiver dropped before server becomes ready.
        let (t2, r2) = oneshot::channel();
        tx.send(Msg::Ask {
            project: project.clone(),
            env,
            ty: "kiapi.common.commands.BeginCommit".into(),
            msg: json!({}),
            deadline: std::time::Instant::now() + ASK_TIMEOUT,
            reply: t2,
        }).unwrap();
        drop(r2);

        let sp = frx.recv().await.unwrap();
        let launch_gen = sp["gen"].as_u64().unwrap();
        tokio::time::sleep(Duration::from_millis(70)).await;
        tx.send(Msg::Line(1, json!({"event":"line","key":project,"gen":launch_gen,"text":format!("KiCad API server listening at ipc://{sock}")}))).unwrap();
        tx.send(Msg::Up { project: project.clone(), version: "10.99.0-bp.13".into(), launch_gen }).unwrap();

        let o1 = r1.await.unwrap();
        assert!(o1.reply["ok"] == json!(false) && o1.reply["error"].as_str().unwrap().contains("timed out"), "{}", o1.reply);
        // Actor must still be healthy and never dispatch BeginCommit for either expired/dropped request.
        assert!(info_on(&ipc).await.contains("|ready|"));
        while let Ok(Some(msg)) = tokio::time::timeout(Duration::from_millis(100), frx.recv()).await {
            assert_ne!(msg.get("type").and_then(Value::as_str), Some("kiapi.common.commands.BeginCommit"), "expired/dropped BeginCommit must never be sent: {msg}");
        }
    }

    #[tokio::test]
    async fn delayed_retire_evict_and_timeout_probe_cannot_affect_replacement_generation() {
        let root = crate::plat::tmp::short_dir("ipc-stale-gen");
        let rt = root.to_string_lossy().to_string();
        let proj_a = "/p/a.kicad_pro".to_string();
        let proj_b = "/p/b.kicad_pro".to_string();
        let dir_a = ks::dir(&rt, "", &proj_a);
        std::fs::create_dir_all(&dir_a).unwrap();
        let sock_a1 = format!("{dir_a}/a1.sock");
        let sock_a2 = format!("{dir_a}/a2.sock");
        #[cfg(unix)]
        let mut _l_a2 = Some(crate::plat::ipc::bind(&sock_a2).unwrap());

        let (ipc, mut frx, tx, alive, _pids) = mock_actor(rt);
        alive.lock().unwrap().insert(proj_a.clone(), true);
        let env = Env { cli: "/k/kicad-cli".into(), version: "10.99.0-bp.13".into(), ..Default::default() };

        // Start proj_a on gen 1 -> crash -> launch gen 2 on sock_a2 and bring it ready.
        let (t1, _r1) = oneshot::channel();
        tx.send(Msg::Ask { project: proj_a.clone(), env: env.clone(), ty: "kiapi.common.commands.Ping".into(), msg: json!({}), deadline: std::time::Instant::now() + ASK_TIMEOUT, reply: t1 }).unwrap();
        let sp1 = frx.recv().await.unwrap();
        let g1 = sp1["gen"].as_u64().unwrap();
        tx.send(Msg::Line(1, json!({"event":"line","key":proj_a,"gen":g1,"text":format!("KiCad API server listening at ipc://{sock_a1}")}))).unwrap();
        tx.send(Msg::Line(1, json!({"event":"exit","key":proj_a,"gen":g1,"code":1,"tail":"crash"}))).unwrap();
        tx.send(Msg::Go { project: proj_a.clone(), launch_gen: g1 }).unwrap();
        let sp2 = loop {
            let msg = frx.recv().await.unwrap();
            if msg["op"] == "spawn" { break msg }
        };
        let g2 = sp2["gen"].as_u64().unwrap();
        assert_ne!(g2, g1);
        tx.send(Msg::Line(1, json!({"event":"line","key":proj_a,"gen":g2,"text":format!("KiCad API server listening at ipc://{sock_a2}")}))).unwrap();
        tx.send(Msg::Up { project: proj_a.clone(), version: "10.99.0-bp.13".into(), launch_gen: g2 }).unwrap();
        assert!(info_on(&ipc).await.contains("|ready|"));

        // Stale Retired from gen g1 must not stop/unlink replacement gen g2.
        let (rtx, rrx) = oneshot::channel();
        tx.send(Msg::Retired { project: proj_a.clone(), launch_gen: g1, ok: true, why: String::new(), reply: rtx }).unwrap();
        let r_ans = rrx.await.unwrap();
        assert_eq!(r_ans["ok"], json!(false));

        // Stale Evicted naming victim proj_a @ g1 must not stop/unlink replacement gen g2.
        tx.send(Msg::Evicted { victim: proj_a.clone(), victim_gen: g1, project: proj_b, project_gen: 999, ok: true, why: String::new() }).unwrap();

        // Drain any handshake Ping/GetVersion left from mock spawn, then confirm timeout probe for g1 sends NO Ping.
        while tokio::time::timeout(Duration::from_millis(50), frx.recv()).await.ok().flatten().is_some() {}
        let explain = explain_begin_commit_timeout_headless(&ipc, &proj_a, &env, &failed("KiCad did not answer in 15 s"), &sock_a1, g1).await;
        assert!(explain.contains("replaced"), "unexpected explain: {explain}");
        assert!(tokio::time::timeout(Duration::from_millis(50), frx.recv()).await.is_err(), "stale probe must not touch replacement socket");

        // Even when gen 3 reuses the exact same socket path as gen 2, a fenced Raw request for g2 must be rejected without reaching transport.
        #[cfg(unix)]
        { _l_a2 = None; }
        tx.send(Msg::Line(1, json!({"event":"exit","key":proj_a,"gen":g2,"code":1,"tail":"crash2"}))).unwrap();
        tx.send(Msg::Go { project: proj_a.clone(), launch_gen: g2 }).unwrap();
        let sp3 = loop {
            let msg = frx.recv().await.unwrap();
            if msg["op"] == "spawn" { break msg }
        };
        let g3 = sp3["gen"].as_u64().unwrap();
        #[cfg(unix)]
        { _l_a2 = Some(crate::plat::ipc::bind(&sock_a2).unwrap()); }
        tx.send(Msg::Line(1, json!({"event":"line","key":proj_a,"gen":g3,"text":format!("KiCad API server listening at ipc://{sock_a2}")}))).unwrap();
        tx.send(Msg::Up { project: proj_a.clone(), version: "10.99.0-bp.13".into(), launch_gen: g3 }).unwrap();
        while tokio::time::timeout(Duration::from_millis(50), frx.recv()).await.ok().flatten().is_some() {}

        let (ftx, frx_raw) = oneshot::channel();
        tx.send(Msg::Raw {
            sock: sock_a2.clone(),
            env,
            ty: "kiapi.backplane.Shutdown".into(),
            msg: json!({}),
            timeout_ms: None,
            fence: Some((proj_a.clone(), g2)),
            reply: ftx,
        }).unwrap();
        let raw_ans = frx_raw.await.unwrap();
        assert_eq!(raw_ans["ok"], json!(false));
        assert!(raw_ans["error"].as_str().unwrap().contains("stale KiCad server generation"), "{raw_ans}");
        assert!(tokio::time::timeout(Duration::from_millis(50), frx.recv()).await.is_err(), "fenced Raw for stale gen must not reach transport even on reused socket path");

        assert!(info_on(&ipc).await.contains("|ready|"), "replacement generation must remain ready");
        #[cfg(unix)]
        assert!(std::path::Path::new(&sock_a2).exists(), "replacement socket must still exist");
    }

    #[tokio::test]
    async fn startup_failure_or_retire_does_not_release_live_child_ownership() {
        let root = crate::plat::tmp::short_dir("ipc-start-own");
        let rt = root.to_string_lossy().to_string();
        let project = "/p/startown.kicad_pro".to_string();
        let (ipc, mut frx, tx, alive, _pids) = mock_actor(rt);
        alive.lock().unwrap().insert(project.clone(), true);
        let env = Env { cli: "/k/kicad-cli".into(), version: "10.99.0-bp.13".into(), ..Default::default() };

        let (atx, arx) = oneshot::channel();
        tx.send(Msg::Ask {
            project: project.clone(),
            env: env.clone(),
            ty: "kiapi.common.commands.Ping".into(),
            msg: json!({}),
            deadline: std::time::Instant::now() + ASK_TIMEOUT,
            reply: atx,
        }).unwrap();
        let sp = frx.recv().await.unwrap();
        let g1 = sp["gen"].as_u64().unwrap();

        // Starting process with empty socket and alive == true must refuse retire().
        let (rtx1, rrx1) = oneshot::channel();
        tx.send(Msg::Retire { project: project.clone(), reply: rtx1 }).unwrap();
        let r1 = rrx1.await.unwrap();
        assert_eq!(r1["ok"], json!(false), "retire while starting child is alive must fail: {r1}");

        // Handshake fails via Msg::Bad -> state = failed; while process stays alive, both retire and Clear refuse to release ownership.
        tx.send(Msg::Bad { project: project.clone(), why: "handshake failed".into(), launch_gen: g1 }).unwrap();
        let _ = arx.await.unwrap();
        assert!(info_on(&ipc).await.contains("|failed|"));

        let (rtx2, rrx2) = oneshot::channel();
        tx.send(Msg::Retire { project: project.clone(), reply: rtx2 }).unwrap();
        let r2 = rrx2.await.unwrap();
        assert_eq!(r2["ok"], json!(false), "retire while failed child is alive must fail: {r2}");

        tx.send(Msg::Clear).unwrap();
        assert!(info_on(&ipc).await.contains("|failed|"), "Clear must not reset failed state while child is still alive");

        // Once the child actually exits, Clear succeeds and next request can spawn a fresh server.
        alive.lock().unwrap().insert(project.clone(), false);
        tx.send(Msg::Clear).unwrap();
        assert!(info_on(&ipc).await.contains("|stopped|"));
    }

    #[tokio::test]
    async fn embed_begin_commit_timeout_reply_has_no_fallback_and_explains_gui_state() {
        use std::sync::{atomic::{AtomicBool, Ordering}, Mutex};
        let pro = "/p/mock-gui.kicad_pro";
        let ping_ok = Arc::new(AtomicBool::new(true));
        let held = Arc::new(Mutex::new(Vec::<oneshot::Sender<Value>>::new()));
        let route = super::super::embed::mock_gui_route_for_test(pro, {
            let ping_ok = ping_ok.clone();
            let held = held.clone();
            move |ty, _msg, reply| {
                if ty == "kiapi.common.commands.BeginCommit" {
                    held.lock().unwrap().push(reply);
                } else if ty == "kiapi.common.commands.Ping" {
                    if ping_ok.load(Ordering::SeqCst) {
                        let _ = reply.send(json!({"ok": true, "type": "PingResponse", "message": {}}));
                    } else {
                        let _ = reply.send(json!({"ok": false, "error": "KiCad IPC connection dropped"}));
                    }
                }
            }
        });

        let timed = super::super::embed::gui_call_timeout(&route, "kiapi.common.commands.BeginCommit", json!({}), "", 50).await;
        assert!(is_timeout_error(&timed), "expected timeout error, got: {timed}");
        assert_ne!(timed["fallback"], json!(true), "timeout must not set fallback:true: {timed}");

        let msg_ok = explain_begin_commit_timeout_gui(&route, pro, &timed).await;
        assert!(msg_ok.contains("still answers reads/Ping"), "unexpected msg_ok: {msg_ok}");

        ping_ok.store(false, Ordering::SeqCst);
        let msg_fail = explain_begin_commit_timeout_gui(&route, pro, &timed).await;
        assert!(msg_fail.contains("follow-up Ping failed"), "unexpected msg_fail: {msg_fail}");
    }

    #[test]
    fn sock_dir_for_respects_unix_sun_len_for_long_home() {
        assert_eq!(
            sock_dir_for("/run/user/1000", "/h", "/p/a.kicad_pro"),
            ks::dir("/run/user/1000", "/h", "/p/a.kicad_pro")
        );
        #[cfg(unix)]
        {
            let long_home = "/Users/long-example-username-here/Library/Application Support/Backplane";
            let kserve_a = ks::dir("", long_home, "/p/a.kicad_pro");
            let d_a = sock_dir_for("", long_home, "/p/a.kicad_pro");
            let d_b = sock_dir_for("", long_home, "/p/b.kicad_pro");
            assert_ne!(d_a, kserve_a, "long macOS home must use fallback dir");
            assert_eq!(d_a, sock_dir_for("", long_home, "/p/a.kicad_pro"), "fallback dir must be deterministic");
            assert_ne!(d_a, d_b, "distinct projects must keep distinct fallback dirs");
            assert!(d_a.contains(&format!("bp-ks-{}", crate::plat::fs::uid())), "expected per-UID fallback dir, got: {d_a}");
            let sock = format!("{d_a}/api-9999999.sock");
            assert!(sock.len() < 104, "headless socket path exceeds macOS SUN_LEN: {sock} ({})", sock.len());

            // Symlinked socket directory must be rejected.
            let tmp = crate::plat::tmp::dir("sock-sym");
            let real = tmp.join("real");
            let link = tmp.join("link");
            std::fs::create_dir_all(&real).unwrap();
            std::os::unix::fs::symlink(&real, &link).unwrap();
            let err = ensure_private_sock_dir(&link).unwrap_err();
            assert!(err.contains("not a real directory"), "unexpected symlink error: {err}");
        }
    }

    fn ok_test_out(s: &str) -> ProbeOutcome {
        #[cfg(unix)]
        let status = { use std::os::unix::process::ExitStatusExt; std::process::ExitStatus::from_raw(0) };
        #[cfg(windows)]
        let status = { use std::os::windows::process::ExitStatusExt; std::process::ExitStatus::from_raw(0) };
        ProbeOutcome::Output(std::process::Output { status, stdout: s.as_bytes().to_vec(), stderr: vec![] })
    }

    fn stdout_trimmed(o: ProbeOutcome) -> String {
        String::from_utf8_lossy(&o.into_output().expect("output").stdout).trim().to_string()
    }

    fn wait_for_waiters(cache: &VersionProbeCache, cli: &Path, want: usize) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while cache.in_flight_waiters(cli) < want {
            assert!(std::time::Instant::now() < deadline, "timed out waiting for {want} single-flight waiters");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn version_probe_cache_single_flight_and_mtime_size_invalidation() {
        use std::sync::{atomic::{AtomicUsize, Ordering}, Condvar, Mutex};

        let d = crate::plat::tmp::dir("vprobe-cache");
        let cli = d.join("kicad-cli");
        std::fs::write(&cli, "v1-payload").unwrap();
        let orig_mtime = std::fs::metadata(&cli).unwrap().modified().unwrap();
        let t0 = std::time::Instant::now();

        let cache = Arc::new(VersionProbeCache::new(Duration::from_secs(30)));
        let calls = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));

        let leader = {
            let (cache, cli, calls, active, max_active, gate) = (cache.clone(), cli.clone(), calls.clone(), active.clone(), max_active.clone(), gate.clone());
            std::thread::spawn(move || {
                cache.get_or_probe_at(&cli, Duration::from_secs(5), 1, || t0, |_, _, _, _| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    let cur = active.fetch_add(1, Ordering::SeqCst) + 1;
                    max_active.fetch_max(cur, Ordering::SeqCst);
                    let (lk, cv) = &*gate;
                    let mut st = lk.lock().unwrap();
                    st.0 = true;
                    cv.notify_all();
                    while !st.1 {
                        st = cv.wait(st).unwrap();
                    }
                    drop(st);
                    active.fetch_sub(1, Ordering::SeqCst);
                    ok_test_out("10.0.9\n")
                })
            })
        };

        {
            let (lk, cv) = &*gate;
            let mut st = lk.lock().unwrap();
            while !st.0 {
                st = cv.wait(st).unwrap();
            }
        }

        let mut waiters = vec![];
        for _ in 0..7 {
            let (cache, cli, calls) = (cache.clone(), cli.clone(), calls.clone());
            waiters.push(std::thread::spawn(move || {
                cache.get_or_probe_at(&cli, Duration::from_secs(5), 1, || t0, |_, _, _, _| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    ok_test_out("unexpected\n")
                })
            }));
        }
        wait_for_waiters(&cache, &cli, 7);
        {
            let (lk, cv) = &*gate;
            let mut st = lk.lock().unwrap();
            st.1 = true;
            cv.notify_all();
        }
        assert_eq!(stdout_trimmed(leader.join().unwrap()), "10.0.9");
        for w in waiters {
            assert_eq!(stdout_trimmed(w.join().unwrap()), "10.0.9");
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "single-flight deduplicated 8 concurrent probes");
        assert_eq!(max_active.load(Ordering::SeqCst), 1);

        let hit = cache.get_or_probe_at(&cli, Duration::from_secs(5), 1, || t0, |_, _, _, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            ok_test_out("unexpected\n")
        });
        assert_eq!(stdout_trimmed(hit), "10.0.9");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "subsequent call hits cache");

        // Size change with identical mtime -> cache miss
        std::fs::write(&cli, "v1-payload-longer").unwrap();
        std::fs::File::options().write(true).open(&cli).unwrap().set_modified(orig_mtime).unwrap();
        let resolved_cli = resolve_probe_path(&cli).unwrap();
        assert_eq!(probe_key_for(&resolved_cli).unwrap().mtime, Some(orig_mtime));
        let hit2 = cache.get_or_probe_at(&cli, Duration::from_secs(5), 1, || t0, |_, _, _, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            ok_test_out("10.0.10\n")
        });
        assert_eq!(stdout_trimmed(hit2), "10.0.10");
        assert_eq!(calls.load(Ordering::SeqCst), 2, "size change invalidates cache even with identical mtime");

        // mtime change with identical size -> cache miss
        std::fs::write(&cli, "v2-payload-longer").unwrap();
        std::fs::File::options().write(true).open(&cli).unwrap().set_modified(orig_mtime + Duration::from_secs(10)).unwrap();
        let hit3 = cache.get_or_probe_at(&cli, Duration::from_secs(5), 1, || t0, |_, _, _, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            ok_test_out("10.0.11\n")
        });
        assert_eq!(stdout_trimmed(hit3), "10.0.11");
        assert_eq!(calls.load(Ordering::SeqCst), 3, "mtime change invalidates cache even with identical size");

        // Negative cache TTL (deterministic virtual clock) and larger-limit bypass
        let fail_cli = d.join("fail-cli");
        std::fs::write(&fail_cli, "fail").unwrap();
        let fail_calls = AtomicUsize::new(0);
        let r1 = cache.get_or_probe_at(&fail_cli, Duration::from_secs(2), 1, || t0, |_, l, a, _| {
            fail_calls.fetch_add(1, Ordering::SeqCst);
            ProbeOutcome::Timeout { limit: l, attempts: a }
        });
        assert!(matches!(r1, ProbeOutcome::Timeout { limit, attempts: 1 } if limit == Duration::from_secs(2)));
        let r2 = cache.get_or_probe_at(&fail_cli, Duration::from_secs(2), 1, || t0 + Duration::from_secs(10), |_, l, a, _| {
            fail_calls.fetch_add(1, Ordering::SeqCst);
            ProbeOutcome::Timeout { limit: l, attempts: a }
        });
        assert!(matches!(r2, ProbeOutcome::Timeout { .. }));
        assert_eq!(fail_calls.load(Ordering::SeqCst), 1, "failure cached within TTL for equal/smaller limit");

        let r3 = cache.get_or_probe_at(&fail_cli, Duration::from_secs(5), 1, || t0 + Duration::from_secs(10), |_, l, a, _| {
            fail_calls.fetch_add(1, Ordering::SeqCst);
            ProbeOutcome::Timeout { limit: l, attempts: a }
        });
        assert!(matches!(r3, ProbeOutcome::Timeout { limit, attempts: 1 } if limit == Duration::from_secs(5)));
        assert_eq!(fail_calls.load(Ordering::SeqCst), 2, "larger timeout limit bypasses smaller timeout cache");

        let r4 = cache.get_or_probe_at(&fail_cli, Duration::from_secs(5), 1, || t0 + Duration::from_secs(41), |_, _, _, _| {
            fail_calls.fetch_add(1, Ordering::SeqCst);
            ok_test_out("10.0.12\n")
        });
        assert_eq!(stdout_trimmed(r4), "10.0.12");
        assert_eq!(fail_calls.load(Ordering::SeqCst), 3, "expired failure is retried");

        // Failure TTL starts at completion, not probe start: a full 30s timeout stays cached for 30s after completion.
        let slow_fail_cli = d.join("slow-fail-cli");
        std::fs::write(&slow_fail_cli, "slow-fail").unwrap();
        let clock = Arc::new(Mutex::new(t0));
        let slow_calls = AtomicUsize::new(0);
        let f1 = cache.get_or_probe_at(
            &slow_fail_cli,
            Duration::from_secs(15),
            2,
            || *clock.lock().unwrap(),
            |_, l, a, _| {
                slow_calls.fetch_add(1, Ordering::SeqCst);
                *clock.lock().unwrap() += Duration::from_secs(30);
                ProbeOutcome::Timeout { limit: l, attempts: a }
            },
        );
        assert!(matches!(f1, ProbeOutcome::Timeout { limit, attempts: 2 } if limit == Duration::from_secs(15)));

        *clock.lock().unwrap() = t0 + Duration::from_secs(40);
        let f2 = cache.get_or_probe_at(
            &slow_fail_cli,
            Duration::from_secs(15),
            2,
            || *clock.lock().unwrap(),
            |_, l, a, _| {
                slow_calls.fetch_add(1, Ordering::SeqCst);
                ProbeOutcome::Timeout { limit: l, attempts: a }
            },
        );
        assert!(matches!(f2, ProbeOutcome::Timeout { limit, attempts: 2 } if limit == Duration::from_secs(15)));
        assert_eq!(slow_calls.load(Ordering::SeqCst), 1, "full-budget failure stays cached relative to completion");

        *clock.lock().unwrap() = t0 + Duration::from_secs(61);
        let f3 = cache.get_or_probe_at(
            &slow_fail_cli,
            Duration::from_secs(15),
            2,
            || *clock.lock().unwrap(),
            |_, _, _, _| {
                slow_calls.fetch_add(1, Ordering::SeqCst);
                ok_test_out("10.0.14\n")
            },
        );
        assert_eq!(stdout_trimmed(f3), "10.0.14");
        assert_eq!(slow_calls.load(Ordering::SeqCst), 2, "failure expires 30s after completion");

        // Panic safety: panicking leader wakes registered waiters and clears in-flight entry deterministically
        let p_cli = d.join("panic-cli");
        std::fs::write(&p_cli, "p").unwrap();
        let p_gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
        let p_leader = {
            let (cache, p_cli, p_gate) = (cache.clone(), p_cli.clone(), p_gate.clone());
            std::thread::spawn(move || {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    cache.get_or_probe_at(&p_cli, Duration::from_secs(5), 1, || t0, |_, _, _, _| {
                        let (lk, cv) = &*p_gate;
                        let mut st = lk.lock().unwrap();
                        st.0 = true;
                        cv.notify_all();
                        while !st.1 {
                            st = cv.wait(st).unwrap();
                        }
                        panic!("simulated panic in probe leader");
                    })
                }));
            })
        };
        {
            let (lk, cv) = &*p_gate;
            let mut st = lk.lock().unwrap();
            while !st.0 {
                st = cv.wait(st).unwrap();
            }
        }
        let p_waiter = {
            let (cache, p_cli) = (cache.clone(), p_cli.clone());
            std::thread::spawn(move || cache.get_or_probe_at(&p_cli, Duration::from_secs(5), 1, || t0, |_, _, _, _| ok_test_out("unused\n")))
        };
        wait_for_waiters(&cache, &p_cli, 1);
        {
            let (lk, cv) = &*p_gate;
            let mut st = lk.lock().unwrap();
            st.1 = true;
            cv.notify_all();
        }
        p_leader.join().unwrap();
        assert!(matches!(p_waiter.join().unwrap(), ProbeOutcome::SpawnErr(ref m) if m.contains("panicked")));
        let recovered = cache.get_or_probe_at(&p_cli, Duration::from_secs(5), 1, || t0, |_, _, _, _| ok_test_out("10.0.13\n"));
        assert_eq!(stdout_trimmed(recovered), "10.0.13");
    }

    #[test]
    fn version_probe_cache_mixed_budget_arrival_orders() {
        use std::sync::{atomic::{AtomicUsize, Ordering}, Condvar, Mutex};

        let d = crate::plat::tmp::dir("vprobe-budget");
        let t0 = std::time::Instant::now();

        // Case A: weak (2s, 1 attempt) arrives first and times out; strong (2s, 2 attempts) joins in-flight.
        // Weak must return Timeout { 2s, 1 }; strong must continue with ONLY the remaining 1 attempt and succeed.
        {
            let cli = d.join("case-a");
            std::fs::write(&cli, "a").unwrap();
            let cache = Arc::new(VersionProbeCache::new(Duration::from_secs(30)));
            let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
            let calls = Arc::new(AtomicUsize::new(0));
            let strong_observed_attempts = Arc::new(AtomicUsize::new(0));

            let weak = {
                let (cache, cli, gate, calls) = (cache.clone(), cli.clone(), gate.clone(), calls.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_secs(2), 1, || t0, |_, l, a, _| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        assert_eq!(a, 1);
                        let (lk, cv) = &*gate;
                        let mut st = lk.lock().unwrap();
                        st.0 = true;
                        cv.notify_all();
                        while !st.1 {
                            st = cv.wait(st).unwrap();
                        }
                        ProbeOutcome::Timeout { limit: l, attempts: a }
                    })
                })
            };
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                while !st.0 {
                    st = cv.wait(st).unwrap();
                }
            }
            let strong = {
                let (cache, cli, calls, obs) = (cache.clone(), cli.clone(), calls.clone(), strong_observed_attempts.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_secs(2), 2, || t0, |_, _, a, _| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        obs.store(a, Ordering::SeqCst);
                        ok_test_out("10.99.0\n")
                    })
                })
            };
            wait_for_waiters(&cache, &cli, 1);
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                st.1 = true;
                cv.notify_all();
            }
            assert!(matches!(weak.join().unwrap(), ProbeOutcome::Timeout { limit, attempts: 1 } if limit == Duration::from_secs(2)));
            assert_eq!(stdout_trimmed(strong.join().unwrap()), "10.99.0");
            assert_eq!(strong_observed_attempts.load(Ordering::SeqCst), 1, "strong runs only remaining 1 attempt after joining weak");
            assert_eq!(calls.load(Ordering::SeqCst), 2);
            let cached = cache.get_or_probe_at(&cli, Duration::from_secs(2), 1, || t0, |_, _, _, _| {
                calls.fetch_add(1, Ordering::SeqCst);
                ok_test_out("unexpected\n")
            });
            assert_eq!(stdout_trimmed(cached), "10.99.0");
            assert_eq!(calls.load(Ordering::SeqCst), 2, "weak success from strong is cached");
        }

        // Case B: strong (2s, 2 attempts) arrives first and runs past weak's 150ms budget; weak joins in-flight.
        // Weak must return Timeout { 150ms, 1 } promptly BEFORE strong finishes, while strong caches success for follow-up callers.
        {
            let cli = d.join("case-b");
            std::fs::write(&cli, "b").unwrap();
            let cache = Arc::new(VersionProbeCache::new(Duration::from_secs(30)));
            let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
            let calls = Arc::new(AtomicUsize::new(0));

            let strong = {
                let (cache, cli, gate, calls) = (cache.clone(), cli.clone(), gate.clone(), calls.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_secs(2), 2, || t0, |_, _, _, _| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        let (lk, cv) = &*gate;
                        let mut st = lk.lock().unwrap();
                        st.0 = true;
                        cv.notify_all();
                        while !st.1 {
                            st = cv.wait(st).unwrap();
                        }
                        ok_test_out("10.99.0\n")
                    })
                })
            };
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                while !st.0 {
                    st = cv.wait(st).unwrap();
                }
            }
            let weak = {
                let (cache, cli, calls) = (cache.clone(), cli.clone(), calls.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_millis(150), 1, || t0, |_, _, _, _| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        ok_test_out("unexpected\n")
                    })
                })
            };
            wait_for_waiters(&cache, &cli, 1);
            let weak_res = weak.join().unwrap();
            assert!(matches!(weak_res, ProbeOutcome::Timeout { limit, attempts: 1 } if limit == Duration::from_millis(150)));
            // Only now unblock strong: weak returned strictly before strong finished!
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                st.1 = true;
                cv.notify_all();
            }
            assert_eq!(stdout_trimmed(strong.join().unwrap()), "10.99.0");
            let cached = cache.get_or_probe_at(&cli, Duration::from_millis(150), 1, || t0, |_, _, _, _| {
                calls.fetch_add(1, Ordering::SeqCst);
                ok_test_out("unexpected\n")
            });
            assert_eq!(stdout_trimmed(cached), "10.99.0");
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }

        // Case C: both arrival orders when leader succeeds quickly -> 1 probe serves both.
        for (weak_first, name) in [(true, "case-c-weak"), (false, "case-c-strong")] {
            let cli = d.join(name);
            std::fs::write(&cli, "c").unwrap();
            let cache = Arc::new(VersionProbeCache::new(Duration::from_secs(30)));
            let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
            let calls = Arc::new(AtomicUsize::new(0));
            let (la, wa) = if weak_first { (1, 2) } else { (2, 1) };

            let leader = {
                let (cache, cli, gate, calls) = (cache.clone(), cli.clone(), gate.clone(), calls.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_secs(2), la, || t0, |_, _, _, _| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        let (lk, cv) = &*gate;
                        let mut st = lk.lock().unwrap();
                        st.0 = true;
                        cv.notify_all();
                        while !st.1 {
                            st = cv.wait(st).unwrap();
                        }
                        ok_test_out("10.99.0\n")
                    })
                })
            };
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                while !st.0 {
                    st = cv.wait(st).unwrap();
                }
            }
            let waiter = {
                let (cache, cli, calls) = (cache.clone(), cli.clone(), calls.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_secs(2), wa, || t0, |_, _, _, _| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        ok_test_out("unexpected\n")
                    })
                })
            };
            wait_for_waiters(&cache, &cli, 1);
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                st.1 = true;
                cv.notify_all();
            }
            assert_eq!(stdout_trimmed(leader.join().unwrap()), "10.99.0");
            assert_eq!(stdout_trimmed(waiter.join().unwrap()), "10.99.0");
            assert_eq!(calls.load(Ordering::SeqCst), 1, "{name}");
        }

        // Case D: waiter taking over from a weaker probe inherits only its remaining wall-clock budget.
        {
            let cli = d.join("case-d-mock");
            std::fs::write(&cli, "d").unwrap();
            let cache = Arc::new(VersionProbeCache::new(Duration::from_secs(30)));
            let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
            let rem_seen_ms = Arc::new(AtomicUsize::new(usize::MAX));

            let weak = {
                let (cache, cli, gate) = (cache.clone(), cli.clone(), gate.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_millis(140), 1, std::time::Instant::now, |_, l, a, _| {
                        let (lk, cv) = &*gate;
                        let mut st = lk.lock().unwrap();
                        st.0 = true;
                        cv.notify_all();
                        while !st.1 {
                            st = cv.wait(st).unwrap();
                        }
                        drop(st);
                        std::thread::sleep(Duration::from_millis(110));
                        ProbeOutcome::Timeout { limit: l, attempts: a }
                    })
                })
            };
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                while !st.0 {
                    st = cv.wait(st).unwrap();
                }
            }
            let strong = {
                let (cache, cli, rem_seen_ms) = (cache.clone(), cli.clone(), rem_seen_ms.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_millis(200), 1, std::time::Instant::now, |_, l, a, dl| {
                        let dl = dl.expect("expected continuation deadline after weaker probe");
                        let rem = dl.saturating_duration_since(std::time::Instant::now()).as_millis() as usize;
                        rem_seen_ms.store(rem, Ordering::SeqCst);
                        ProbeOutcome::Timeout { limit: l.min(Duration::from_millis(rem as u64)), attempts: a }
                    })
                })
            };
            wait_for_waiters(&cache, &cli, 1);
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                st.1 = true;
                cv.notify_all();
            }
            assert!(matches!(weak.join().unwrap(), ProbeOutcome::Timeout { .. }));
            assert!(matches!(strong.join().unwrap(), ProbeOutcome::Timeout { .. }));
            let rem = rem_seen_ms.load(Ordering::SeqCst);
            assert!(rem < 115, "expected remaining continuation deadline < 115ms, got {rem}ms");
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let cli = d.join("case-d-real");
            std::fs::write(&cli, "#!/bin/sh\nsleep 10\n").unwrap();
            std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
            let cache = Arc::new(VersionProbeCache::new(Duration::from_secs(30)));
            let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));

            let weak = {
                let (cache, cli, gate) = (cache.clone(), cli.clone(), gate.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_millis(200), 1, std::time::Instant::now, |_, l, a, _| {
                        let (lk, cv) = &*gate;
                        let mut st = lk.lock().unwrap();
                        st.0 = true;
                        cv.notify_all();
                        while !st.1 {
                            st = cv.wait(st).unwrap();
                        }
                        drop(st);
                        std::thread::sleep(Duration::from_millis(180));
                        ProbeOutcome::Timeout { limit: l, attempts: a }
                    })
                })
            };
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                while !st.0 {
                    st = cv.wait(st).unwrap();
                }
            }
            let strong = {
                let (cache, cli) = (cache.clone(), cli.clone());
                std::thread::spawn(move || {
                    let started = std::time::Instant::now();
                    let res = cache.get_or_probe_at(&cli, Duration::from_millis(300), 1, std::time::Instant::now, probe_cli_version_uncached_until);
                    (started.elapsed(), res)
                })
            };
            wait_for_waiters(&cache, &cli, 1);
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                st.1 = true;
                cv.notify_all();
            }
            assert!(matches!(weak.join().unwrap(), ProbeOutcome::Timeout { .. }));
            let (elapsed, strong_res) = strong.join().unwrap();
            assert!(matches!(strong_res, ProbeOutcome::Timeout { .. }));
            assert!(elapsed < Duration::from_millis(420), "continuation exceeded caller budget: {elapsed:?}");

            let followup_calls = AtomicUsize::new(0);
            let retry = cache.get_or_probe_at(&cli, Duration::from_millis(300), 1, std::time::Instant::now, |_, _, _, _| {
                followup_calls.fetch_add(1, Ordering::SeqCst);
                ok_test_out("10.99.2\n")
            });
            assert_eq!(stdout_trimmed(retry), "10.99.2");
            assert_eq!(followup_calls.load(Ordering::SeqCst), 1, "capped continuation timeout must not cache as a full 300ms timeout");
        }

        // Case E: waiter joining a LARGER-limit flight (400ms, 1) with (220ms, 2) must carry its remaining wall-clock
        // deadline into its second attempt and must not cache a truncated continuation as a full (220ms, 2) timeout.
        {
            let cli = d.join("case-e-mock");
            std::fs::write(&cli, "e").unwrap();
            let cache = Arc::new(VersionProbeCache::new(Duration::from_secs(30)));
            let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
            let rem_seen_ms = Arc::new(AtomicUsize::new(usize::MAX));
            let attempts_seen = Arc::new(AtomicUsize::new(0));

            let leader = {
                let (cache, cli, gate) = (cache.clone(), cli.clone(), gate.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_millis(400), 1, std::time::Instant::now, |_, l, a, _| {
                        let (lk, cv) = &*gate;
                        let mut st = lk.lock().unwrap();
                        st.0 = true;
                        cv.notify_all();
                        while !st.1 {
                            st = cv.wait(st).unwrap();
                        }
                        drop(st);
                        std::thread::sleep(Duration::from_millis(300));
                        ProbeOutcome::Timeout { limit: l, attempts: a }
                    })
                })
            };
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                while !st.0 {
                    st = cv.wait(st).unwrap();
                }
            }
            let waiter = {
                let (cache, cli, rem_seen_ms, attempts_seen) = (cache.clone(), cli.clone(), rem_seen_ms.clone(), attempts_seen.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_millis(220), 2, std::time::Instant::now, |_, l, a, dl| {
                        attempts_seen.store(a, Ordering::SeqCst);
                        let dl = dl.expect("expected continuation deadline after larger-limit flight");
                        let rem = dl.saturating_duration_since(std::time::Instant::now()).as_millis() as usize;
                        rem_seen_ms.store(rem, Ordering::SeqCst);
                        ProbeOutcome::Timeout { limit: l.min(Duration::from_millis(rem as u64)), attempts: a }
                    })
                })
            };
            wait_for_waiters(&cache, &cli, 1);
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                st.1 = true;
                cv.notify_all();
            }
            assert!(matches!(leader.join().unwrap(), ProbeOutcome::Timeout { .. }));
            assert!(matches!(waiter.join().unwrap(), ProbeOutcome::Timeout { .. }));
            assert_eq!(attempts_seen.load(Ordering::SeqCst), 1);
            let rem = rem_seen_ms.load(Ordering::SeqCst);
            assert!(rem < 170, "expected remaining continuation deadline < 170ms (total 440ms - 300ms wait), got {rem}ms");
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let cli = d.join("case-e-real");
            std::fs::write(&cli, "#!/bin/sh\ntrap '' TERM\nexec sleep 10\n").unwrap();
            std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
            let cache = Arc::new(VersionProbeCache::new(Duration::from_secs(30)));
            let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));

            let leader = {
                let (cache, cli, gate) = (cache.clone(), cli.clone(), gate.clone());
                std::thread::spawn(move || {
                    cache.get_or_probe_at(&cli, Duration::from_millis(450), 1, std::time::Instant::now, |_, l, a, _| {
                        let (lk, cv) = &*gate;
                        let mut st = lk.lock().unwrap();
                        st.0 = true;
                        cv.notify_all();
                        while !st.1 {
                            st = cv.wait(st).unwrap();
                        }
                        drop(st);
                        std::thread::sleep(Duration::from_millis(360));
                        ProbeOutcome::Timeout { limit: l, attempts: a }
                    })
                })
            };
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                while !st.0 {
                    st = cv.wait(st).unwrap();
                }
            }
            let waiter = {
                let (cache, cli) = (cache.clone(), cli.clone());
                std::thread::spawn(move || {
                    let started = std::time::Instant::now();
                    let res = cache.get_or_probe_at(&cli, Duration::from_millis(250), 2, std::time::Instant::now, probe_cli_version_uncached_until);
                    (started.elapsed(), res)
                })
            };
            wait_for_waiters(&cache, &cli, 1);
            {
                let (lk, cv) = &*gate;
                let mut st = lk.lock().unwrap();
                st.1 = true;
                cv.notify_all();
            }
            assert!(matches!(leader.join().unwrap(), ProbeOutcome::Timeout { limit, attempts: 1 } if limit == Duration::from_millis(450)));
            let (elapsed, waiter_res) = waiter.join().unwrap();
            assert!(matches!(waiter_res, ProbeOutcome::Timeout { .. }));
            assert!(elapsed < Duration::from_millis(560), "waiter continuation exceeded 500ms budget (incl. SIGTERM cleanup): {elapsed:?}");

            let leader_followup = AtomicUsize::new(0);
            let cached_leader = cache.get_or_probe_at(&cli, Duration::from_millis(450), 1, std::time::Instant::now, |_, _, _, _| {
                leader_followup.fetch_add(1, Ordering::SeqCst);
                ok_test_out("unexpected\n")
            });
            assert!(matches!(cached_leader, ProbeOutcome::Timeout { .. }));
            assert_eq!(leader_followup.load(Ordering::SeqCst), 0, "leader's 450ms x1 timeout must remain cached");

            let followup_calls = AtomicUsize::new(0);
            let retry = cache.get_or_probe_at(&cli, Duration::from_millis(250), 2, std::time::Instant::now, |_, _, _, _| {
                followup_calls.fetch_add(1, Ordering::SeqCst);
                ok_test_out("10.99.3\n")
            });
            assert_eq!(stdout_trimmed(retry), "10.99.3");
            assert_eq!(followup_calls.load(Ordering::SeqCst), 1, "truncated continuation must not cache as a full (250ms, 2) timeout");
        }

        // Case F: equal-limit waiter (250ms, 2) joining a SIGTERM-ignoring (250ms, 1) flight obeys its 500ms
        // wall-clock deadline end-to-end and does not cache a truncated continuation as (250ms, 2).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let cli = d.join("case-f-real");
            let log = d.join("case-f-real.log");
            std::fs::write(&cli, format!("#!/bin/sh\necho $$ >> '{}'\ntrap '' TERM\nexec sleep 10\n", log.display())).unwrap();
            std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
            let cache = Arc::new(VersionProbeCache::new(Duration::from_secs(30)));

            let weak = {
                let (cache, cli) = (cache.clone(), cli.clone());
                std::thread::spawn(move || cache.get_or_probe_at(&cli, Duration::from_millis(250), 1, std::time::Instant::now, probe_cli_version_uncached_until))
            };
            let wait_start = std::time::Instant::now() + Duration::from_secs(5);
            while !log.is_file() {
                assert!(std::time::Instant::now() < wait_start, "weak equal-limit probe did not start");
                std::thread::sleep(Duration::from_millis(2));
            }
            let strong = {
                let (cache, cli) = (cache.clone(), cli.clone());
                std::thread::spawn(move || {
                    let started = std::time::Instant::now();
                    let res = cache.get_or_probe_at(&cli, Duration::from_millis(250), 2, std::time::Instant::now, probe_cli_version_uncached_until);
                    (started.elapsed(), res)
                })
            };
            wait_for_waiters(&cache, &cli, 1);
            assert!(matches!(weak.join().unwrap(), ProbeOutcome::Timeout { limit, attempts: 1 } if limit == Duration::from_millis(250)));
            let (elapsed, strong_res) = strong.join().unwrap();
            assert!(matches!(strong_res, ProbeOutcome::Timeout { .. }));
            assert!(elapsed < Duration::from_millis(570), "equal-limit continuation exceeded 500ms budget: {elapsed:?}");

            let followup_1 = AtomicUsize::new(0);
            let cached_1 = cache.get_or_probe_at(&cli, Duration::from_millis(250), 1, std::time::Instant::now, |_, _, _, _| {
                followup_1.fetch_add(1, Ordering::SeqCst);
                ok_test_out("unexpected\n")
            });
            assert!(matches!(cached_1, ProbeOutcome::Timeout { .. }));
            assert_eq!(followup_1.load(Ordering::SeqCst), 0, "weak (250ms, 1) timeout stays cached");

            let followup_2 = AtomicUsize::new(0);
            let retry_2 = cache.get_or_probe_at(&cli, Duration::from_millis(250), 2, std::time::Instant::now, |_, _, _, _| {
                followup_2.fetch_add(1, Ordering::SeqCst);
                ok_test_out("10.99.4\n")
            });
            assert_eq!(stdout_trimmed(retry_2), "10.99.4");
            assert_eq!(followup_2.load(Ordering::SeqCst), 1, "truncated equal-limit continuation must not cache as (250ms, 2)");
        }
    }

    #[cfg(unix)]
    #[test]
    fn version_probe_cache_real_timeouts_populate_cache_for_same_budget() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let d = crate::plat::tmp::dir("vprobe-real-to");
        let make_hang = |name: &str| {
            let cli = d.join(name);
            let log = d.join(format!("{name}.log"));
            std::fs::write(&cli, format!("#!/bin/sh\necho $$ >> '{}'\nexec sleep 10\n", log.display())).unwrap();
            std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
            (cli, log)
        };
        let cache = Arc::new(VersionProbeCache::new(Duration::from_secs(30)));
        let limit = Duration::from_millis(200);

        // 1. Solo 1-attempt real timeout populates cache for (limit, 1) and serves repeat call from cache.
        let (c1, l1) = make_hang("solo-1");
        let r1 = cache.get_or_probe(&c1, limit, 1, probe_cli_version_uncached_until);
        assert!(matches!(r1, ProbeOutcome::Timeout { limit: l, attempts: 1 } if l == limit), "got {r1:?}");
        assert_eq!(std::fs::read_to_string(&l1).unwrap().lines().count(), 1);
        let r1_again = cache.get_or_probe(&c1, limit, 1, probe_cli_version_uncached_until);
        assert!(matches!(r1_again, ProbeOutcome::Timeout { limit: l, attempts: 1 } if l == limit));
        assert_eq!(std::fs::read_to_string(&l1).unwrap().lines().count(), 1, "solo 1-attempt timeout cached");

        // 2. Solo 2-attempt real timeout populates cache for (limit, 2) and serves repeat call from cache.
        let (c2, l2) = make_hang("solo-2");
        let r2 = cache.get_or_probe(&c2, limit, 2, probe_cli_version_uncached_until);
        assert!(matches!(r2, ProbeOutcome::Timeout { limit: l, attempts: 2 } if l == limit), "got {r2:?}");
        assert_eq!(std::fs::read_to_string(&l2).unwrap().lines().count(), 2);
        let r2_again = cache.get_or_probe(&c2, limit, 2, probe_cli_version_uncached_until);
        assert!(matches!(r2_again, ProbeOutcome::Timeout { limit: l, attempts: 2 } if l == limit));
        assert_eq!(std::fs::read_to_string(&l2).unwrap().lines().count(), 2, "solo 2-attempt timeout cached");

        // 3. Same-limit 1-attempt followed mid-flight by 2-attempt continuation populates cache for (limit, 2).
        let (c3, l3) = make_hang("weak-then-2");
        let weak_limit = Duration::from_millis(400);
        let weak = {
            let (cache, cli) = (cache.clone(), c3.clone());
            std::thread::spawn(move || cache.get_or_probe(&cli, weak_limit, 1, probe_cli_version_uncached_until))
        };
        let wait_start = std::time::Instant::now() + Duration::from_secs(5);
        while !l3.is_file() {
            assert!(std::time::Instant::now() < wait_start, "weak probe did not start");
            std::thread::sleep(Duration::from_millis(2));
        }
        std::thread::sleep(Duration::from_millis(80));
        let strong = {
            let (cache, cli) = (cache.clone(), c3.clone());
            std::thread::spawn(move || cache.get_or_probe(&cli, weak_limit, 2, probe_cli_version_uncached_until))
        };
        wait_for_waiters(&cache, &c3, 1);
        assert!(matches!(weak.join().unwrap(), ProbeOutcome::Timeout { limit: l, attempts: 1 } if l == weak_limit));
        assert!(matches!(strong.join().unwrap(), ProbeOutcome::Timeout { limit: l, attempts: 2 } if l == weak_limit));
        assert_eq!(std::fs::read_to_string(&l3).unwrap().lines().count(), 2, "1 weak + 1 continuation = 2 total spawns");

        let followup_calls = AtomicUsize::new(0);
        let hit = cache.get_or_probe(&c3, weak_limit, 2, |p, l, a, dl| {
            followup_calls.fetch_add(1, Ordering::SeqCst);
            probe_cli_version_uncached_until(p, l, a, dl)
        });
        assert!(matches!(hit, ProbeOutcome::Timeout { limit: l, attempts: 2 } if l == weak_limit));
        assert_eq!(followup_calls.load(Ordering::SeqCst), 0, "repeat 2-attempt call hits cache");
    }

    #[test]
    fn resolve_probe_path_ignores_cwd_file_for_bare_command_name() {
        // A bare command name must never resolve to a file in the current working directory.
        if Path::new("Cargo.toml").is_file() {
            assert_eq!(resolve_probe_path(Path::new("Cargo.toml")), None);
        }
        assert_eq!(resolve_probe_path(Path::new("bp-nonexistent-kicad-cli-test-only")), None);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let d = crate::plat::tmp::dir("resolve-symlink");
            let (t1, t2, link) = (d.join("t1"), d.join("t2"), d.join("kicad-cli"));
            std::fs::write(&t1, "a").unwrap();
            std::fs::write(&t2, "longer-target").unwrap();
            std::fs::set_permissions(&t1, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::set_permissions(&t2, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::os::unix::fs::symlink(&t1, &link).unwrap();

            let r1 = resolve_probe_path(&link).unwrap();
            assert_eq!(r1, crate::plat::fs::canonicalize(&t1).unwrap());
            assert_eq!(probe_key_for(&r1).unwrap().size, 1);

            std::fs::remove_file(&link).unwrap();
            std::os::unix::fs::symlink(&t2, &link).unwrap();
            let r2 = resolve_probe_path(&link).unwrap();
            assert_eq!(r2, crate::plat::fs::canonicalize(&t2).unwrap());
            assert_eq!(probe_key_for(&r2).unwrap().size, 13);
        }
    }

    #[cfg(unix)]
    #[test]
    fn cli_version_output_deduplicates_concurrent_probes_and_invalidates_on_change() {
        use std::os::unix::fs::PermissionsExt;
        let d = crate::plat::tmp::dir("cli-cache");
        let (cli, log) = (d.join("kicad-cli"), d.join("invocations.log"));
        std::fs::write(
            &cli,
            format!(
                "#!/bin/sh\n[ \"$1\" = {NOOP} ] && exit 0\necho $$ >> '{}'\nsleep 0.15\necho 10.0.9\n",
                log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        started(&cli);

        let cli_s = cli.to_string_lossy().into_owned();
        let handles: Vec<_> = (0..6)
            .map(|_| {
                let s = cli_s.clone();
                std::thread::spawn(move || {
                    let o = cli_version_output(&s).expect("probe succeeded");
                    String::from_utf8_lossy(&o.stdout).trim().to_string()
                })
            })
            .collect();
        for h in handles {
            assert_eq!(h.join().unwrap(), "10.0.9");
        }
        assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 1, "6 concurrent probes spawned kicad-cli only once");

        let seq = cli_version_output(&cli_s).expect("cached hit");
        assert_eq!(String::from_utf8_lossy(&seq.stdout).trim(), "10.0.9");
        assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 1);

        let prev_mtime = std::fs::metadata(&cli).unwrap().modified().unwrap();
        std::fs::write(
            &cli,
            format!(
                "#!/bin/sh\n[ \"$1\" = {NOOP} ] && exit 0\necho $$ >> '{}'\necho 10.0.10-updated\n",
                log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::File::options().write(true).open(&cli).and_then(|f| f.set_modified(prev_mtime + Duration::from_secs(2)));
        started(&cli);

        let updated = cli_version_output(&cli_s).expect("re-probed after file update");
        assert_eq!(String::from_utf8_lossy(&updated.stdout).trim(), "10.0.10-updated");
        assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 2);
    }
}

/// Whether `path`'s project is open in a co-editing KiCad (bp.11 `coedit`): agents write its live documents through IPC at once,
/// with no document lock (the fork defers what the person holds and merges the rest), and never ask the person to wait.
pub fn coedit_live(path: &str) -> bool {
    let pro = pro_of(&super::canonical::route(path));
    !pro.is_empty() && super::embed::gui_route(&pro).is_some_and(|g| g.caps.iter().any(|c| c == "coedit"))
}

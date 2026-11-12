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
    Evicted { victim: String, project: String, ok: bool, why: String },
}

struct Wt { project: String, ty: String, msg: Value, env: Env, reply: oneshot::Sender<Value> }
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
}

pub fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

impl Actor {
    fn srv(&self, project: &str) -> Srv { self.srvs.iter().find(|s| s.project == project).cloned().unwrap_or_else(|| Srv::none(project)) } // KServe.get
    fn put(&mut self, s: Srv) { match self.srvs.iter_mut().find(|x| x.project == s.project) { Some(x) => *x = s, None => self.srvs.push(s) } } // KServe.put
    fn emit(&mut self) { let t = ks::lines(&self.srvs); if t != self.shown { *self.lines.lock().unwrap() = t.clone(); self.shown = t } } // KServe.emit
    fn later(&self, secs: f64, m: Msg) { let me = self.me.clone(); tokio::spawn(async move { tokio::time::sleep(Duration::from_secs_f64(secs)).await; let _ = me.send(m); }); }

    /// A request line registered for its reply (KServe.call). The helper is started on first use.
    fn call(&mut self, mut fields: Value, reply: oneshot::Sender<Value>) {
        if self.helper.is_none() { self.start_helper() }
        let Some(h) = &mut self.helper else { let _ = reply.send(failed("the KiCad IPC transport could not start")); return };
        self.next += 1; let id = self.next.to_string();
        fields["id"] = json!(self.next);
        if !h.link.send(fields) { let _ = reply.send(failed("the KiCad IPC helper exited")); return }
        self.pend.insert(id, reply);
    }
    fn start_helper(&mut self) {
        self.gen += 1; let gen = self.gen;
        let me = self.me.clone();
        self.helper = Some(Helper { link: Box::new(kipc::Native::new("", Arc::new(move |o| { let _ = me.send(Msg::Line(gen, o)); }))), gen });
    }
    fn sink(&mut self, fields: Value) { let (t, _r) = oneshot::channel(); self.call(fields, t) }

    fn forward(&mut self, project: &str, env: &Env, ty: &str, msg: Value, reply: oneshot::Sender<Value>) { // KServe.forward
        let srv = self.srv(project).touch(now_s());
        let f = self.call_fields(&srv.sock, env, ty, msg); self.put(srv); self.call(f, reply)
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
        let dir = ks::dir(&self.rt, &self.home, project);
        { use crate::plat::fs::DirBuilderExt; let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir); }
        let argv = ks::argv(&self.env.cli, project, &dir, self.hub_pid, &self.env.caps);
        let envv: Map<String, Value> = ["BACKPLANE_KICAD_LIBRARY", "KICAD_CONFIG_HOME"].iter().map(|k| (k.to_string(), json!(std::env::var(k).unwrap_or_default()))).collect();
        self.sink(json!({"op":"spawn","key":project,"argv":argv,"memory":"1536M","env":envv}));
        let s = self.srv(project).starting(now); self.put(s);
        if self.helper.is_some() { self.later(30.0, Msg::Late { project: project.into(), born: now }) }
        self.emit();
    }
    fn close(&mut self, sock: &str) { if !sock.is_empty() { self.sink(json!({"op":"close","sock":sock})) } } // KServe.close

    fn ask(&mut self, w: Wt) {
        let plan = ks::plan(&self.srvs, &w.project, &self.env.caps);
        match plan.as_str() {
            "ready" => self.forward(&w.project.clone(), &w.env.clone(), &w.ty.clone(), w.msg, w.reply),
            "wait" => self.wait.insert(0, w),
            "failed" => { let why = format!("the headless KiCad for {} stopped: {}", ks::name(&w.project), self.srv(&w.project).why); let _ = w.reply.send(failed(&ks::unavailable(&why, &w.env.cli))); }
            "busy" => { let _ = w.reply.send(failed(&ks::unavailable(ks::BUSY, &w.env.cli))); }
            "start" => { let p = w.project.clone(); self.wait.insert(0, w); self.launch(&p) }
            p => { // "evict:<victim>": the least recently used server is asked to stop (it does so itself); the new one starts when it has
                let victim = p.trim_start_matches("evict:").to_string(); let project = w.project.clone();
                self.evicting(&victim, &project);
                let s = Srv::new(&project, now_s()); self.put(s); self.wait.insert(0, w); self.emit();
            }
        }
    }
    fn evicting(&mut self, victim: &str, project: &str) { // KServe.evicting: kiapi.backplane.Shutdown, never a signal
        let sock = self.srv(victim).sock; let env = self.env.clone(); let me = self.me.clone();
        let (victim, project) = (victim.to_string(), project.to_string());
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
            let p = ask("kiapi.common.commands.Ping").await.unwrap_or_else(|_| failed("no reply"));
            if !p["ok"].as_bool().unwrap_or(false) { let _ = me.send(Msg::Bad { project, why: format!("it did not answer Ping: {}", p["error"].as_str().unwrap_or("")) }); return }
            let o = ask("kiapi.common.commands.GetVersion").await.unwrap_or_else(|_| failed("no reply"));
            let full = o["message"]["version"]["full_version"].as_str().unwrap_or("").to_string();
            if o["ok"].as_bool().unwrap_or(false) && ks::version_ok(&full, &env.version) { let _ = me.send(Msg::Up { project, version: full }); }
            else { let err = o["error"].as_str().unwrap_or(""); let _ = me.send(Msg::Bad { project, why: if err.is_empty() { format!("this KiCad is {full}, not the {} Backplane installed", env.version) } else { format!("it did not answer GetVersion: {err}") } }); }
        });
    }

    fn line(&mut self, o: Value) { // what the transport said: a server's stdout line or exit, else a reply (by id)
        match o["event"].as_str() {
            Some("line") => {
                let project = o["key"].as_str().unwrap_or("").to_string();
                if let Some(path) = ks::ready(o["text"].as_str().unwrap_or("")) {
                    let s = self.srv(&project);
                    if s.state == "starting" && s.sock.is_empty() { self.hand(project.clone(), self.env.clone(), path.clone()); let s = s.listening(&path); self.put(s) }
                }
            }
            Some("exit") => self.exited(o["key"].as_str().unwrap_or("").to_string(), o["code"].as_i64().unwrap_or(1) as i32, o["tail"].as_str().unwrap_or("").to_string()),
            _ => { let id = match &o["id"] { Value::String(s) => s.clone(), v => v.to_string() }; if let Some(r) = self.pend.remove(&id) { let _ = r.send(o); } }
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
        self.emit();
    }
    fn bad(&mut self, project: String, why: String) { // it failed the handshake or never listened
        if self.srv(&project).state != "starting" { return }
        let s = self.srv(&project).set("failed", &why); self.put(s);
        self.fail_project(&project, &format!("the headless KiCad for {} {why}", ks::name(&project))); self.emit();
    }
    fn evicted(&mut self, victim: String, project: String, ok: bool, why: String) {
        if ok { let s = self.srv(&victim).set("stopped", ""); self.put(s); self.launch(&project); return }
        let s = self.srv(&victim).hold(); self.put(s); // Shutdown refused (unsaved changes): held, never forced; the next least recently used is asked
        let other = ks::evict(&self.srvs);
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
            Msg::Ask { project, env, ty, msg, reply } => { self.env = env.clone(); self.ask(Wt { project, ty, msg, env, reply }) }
            Msg::Raw { sock, env, ty, msg, reply } => { let f = self.call_fields(&sock, &env, &ty, msg); self.call(f, reply) }
            Msg::Retire { project, reply } => self.retire(project, reply),
            Msg::Retired { project, ok, why, reply } => self.retired(project, ok, why, reply),
            Msg::Info(r) => { let _ = r.send(ks::lines(&self.srvs)); }
            Msg::Argv { project, env, reply } => { let d = ks::dir(&self.rt, &self.home, &project); let _ = reply.send(ks::argv(&env.cli, &project, &d, self.hub_pid, &env.caps).join(" ")); }
            Msg::Clear => { for s in self.srvs.iter_mut().filter(|s| s.state == "failed") { *s = s.clone().cleared() } self.emit() }
            Msg::Line(gen, t) => if self.helper.as_ref().map(|h| h.gen) == Some(gen) { self.line(t) },
            Msg::Up { project, version } => self.up(project, version),
            Msg::Bad { project, why } => self.bad(project, why),
            Msg::Late { project, born } => { let s = self.srv(&project); if s.state == "starting" && s.started == born && s.sock.is_empty() { self.bad(project, "did not say it was listening within 30 s".into()) } }
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
        srvs: vec![], wait: vec![], shown: String::new(), env: Env::default(), lines: lines.clone() };
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
    v.sort_by(|a, b| if a.0 == b.0 { std::cmp::Ordering::Equal } else if ks::version_ge(&a.0, &b.0) { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater });
    v.into_iter().flat_map(|(n, d)| [d.join(&n).join("bin").join("kicad-cli"), d.join(n).join("bin").join("kicad-cli.exe")]).collect()
}
/// Publish what this machine has for Settings > CAD: info `kicad.cli`, `kicad.version`, `kicad.status`, `kicad.source` and `freecad` (empty when absent).
pub async fn publish_cad(hub: &Arc<Hub>) {
    let env = runtime(hub).await;
    let cli = env.cli.clone();
    let probe = if cli.is_empty() { None } else {
        tokio::task::spawn_blocking(move || crate::proc::std_cmd(crate::proc::Kind::Local, &cli).arg("--version").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()).await.ok().flatten()
    };
    let ran = probe.as_ref().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let (status, version) = cad_status(&env, ran.as_deref());
    let source = if hub.setting("kicad.path").await.map_or(false, |s| !s.is_empty()) { "setting" } else { "path" };
    let fc = super::stepglb::freecad_cmd(&hub.cfg.home);
    for (k, v) in [("kicad.cli", env.cli.as_str()), ("kicad.version", version.as_str()), ("kicad.status", status.as_str()), ("kicad.source", source), ("freecad", fc.as_str())] { hub.set_info(k, v).await; }
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
/// Rt.manifest + KH.status from the runtime's manifest.json (no version probe: `runs` is taken as true when a manifest is read).
pub fn read_runtime(cli: &Path) -> Env {
    let root = cli.parent().and_then(|b| b.parent()).map(|r| r.to_string_lossy().to_string()).unwrap_or_default();
    let text = std::fs::read_to_string(format!("{root}/manifest.json")).or_else(|_| std::fs::read_to_string(format!("{root}/Contents/Resources/manifest.json"))).unwrap_or_default();
    let m: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
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
    r.await.unwrap_or_else(|_| failed("no reply"))
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
    if Path::new(&path).is_dir() { return project::read(&super::canonical::design_root(&path)).pro }
    Path::new(&path).parent().map(|d| project::read(&d.to_string_lossy()).pro).unwrap_or_default()
}
/// A listed KiCad read (`Get*`, `Ping`): the commands a copy may run and a writer lock never gates.
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
/// The MCP tool `kicad_ipc_status`: Ok({"server","servers","argv","version","capabilities","detail"}); starts the server if it is not running.
pub async fn kicad_ipc_status(hub: &Arc<Hub>, path: &str) -> Result<Value, String> { kicad_ipc_status_in(hub, None, path).await }
pub async fn kicad_ipc_status_in(hub: &Arc<Hub>, thread: Option<&str>, path: &str) -> Result<Value, String> { run(hub, thread, path, "kiapi.backplane.GetCapabilities", json!({}), true).await }

async fn run(hub: &Arc<Hub>, thread: Option<&str>, path: &str, ty: &str, msg: Value, status: bool) -> Result<Value, String> {
    let msg = if msg.is_object() { msg } else { json!({}) }; // KIpc.msg: a missing message is {}
    let routed = super::canonical::route(path); let path = &routed; // main's copy, whichever thread named it
    let pro = pro_of(path);
    if let Some(gui) = (!pro.is_empty()).then(|| super::embed::gui_route(&pro)).flatten() {
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
        let actor = if coedit { thread.filter(|t| !t.is_empty()).map(|t| format!("agent:{t}")).unwrap_or_else(|| "agent".into()) } else { String::new() };
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
        let o = super::embed::gui_call_as(&gui, ty, msg, &actor).await;
        if let Some(note) = held { log_fallback(hub, thread, &note).await; }
        if commits && o["ok"].as_bool().unwrap_or(false) { super::embed::unsaved_update(&pro, |u| u.agent_committed()); }
        if o["fallback"].as_bool().unwrap_or(false) { log_fallback(hub, thread, o["why"].as_str().unwrap_or("")).await; return Err(o["error"].as_str().unwrap_or("").into()) }
        let ok = o["ok"].as_bool().unwrap_or(false);
        let servers = format!("{pro}|ready|gui ({})", gui.mode);
        if status { return Ok(json!({"server":format!("gui ({})", gui.mode),"servers":servers,"argv":gui.argv,"version":gui.version,"capabilities":o["message"].clone(),"detail":if ok { String::new() } else { err_of(&o) }})) }
        if !ok { return Err(err_of(&o)) }
        let mut out = json!({"type":o["type"].as_str().unwrap_or(""),"message":o["message"].clone()});
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
    let o = ask(i, &pro, &env, ty, msg).await;
    let lines = info_on(i).await; let argv = argv_on(i, &pro, &env).await;
    if o["fallback"].as_bool().unwrap_or(false) { log_fallback(hub, thread, o["why"].as_str().unwrap_or("")).await; return Err(o["error"].as_str().unwrap_or("").into()) }
    let ok = o["ok"].as_bool().unwrap_or(false);
    if status { return Ok(json!({"server":ks::note(&lines),"servers":lines,"argv":argv,"version":env.version,"capabilities":o["message"].clone(),"detail":if ok { String::new() } else { err_of(&o) }})) }
    if ok { Ok(json!({"type":o["type"].as_str().unwrap_or(""),"message":o["message"].clone()})) } else { Err(err_of(&o)) }
}

/// Whether an EndCommit request commits (CMA_COMMIT, the default) rather than drops.
fn msg_commits(msg: &Value) -> bool { !msg.to_string().contains("CMA_DROP") }

/// Client RPC `kicad.retry` (Hub.kretry): failed servers are forgotten; the next request starts them.
pub struct IpcRpc;
impl RpcHandler for IpcRpc {
    fn methods(&self) -> &'static [&'static str] { &["kicad.retry"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _method: &'a str, _p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move { if let Some(i) = IPC.get() { let _ = i.tx.send(Msg::Clear); } vec![json!({"t":"reply","id":id,"ok":true})] })
    }
}

#[cfg(test)]
mod tests {
    /// A managed install on Windows has `bin\kicad-cli.exe`: the newest version's is found, with a space in the folder name.
    #[test] fn a_managed_runtime_is_found_by_its_exe_name_in_a_folder_with_spaces() {
        let base = crate::plat::tmp::dir("bp kicad"); let d = base.join("kicad");
        for v in ["10.99.0-bp.4", "10.99.0-bp.6"] { std::fs::create_dir_all(d.join(v).join("bin")).unwrap(); std::fs::write(d.join(v).join("bin/kicad-cli.exe"), "MZ").unwrap(); }
        let found = newest_in(std::slice::from_ref(&d)).into_iter().find(|p| p.is_file()).unwrap();
        assert!(found.ends_with("10.99.0-bp.6/bin/kicad-cli.exe"), "{found:?}");
    }
    use super::*;
    /// bp.4's kicad-cli --version prints `10.99.0`: its manifest says what it is, so Settings > CAD shows ok and 10.99.0-bp.4, not stock
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
        let mut a = Actor { me: tx.clone(), home: String::new(), rt, hub_pid: std::process::id(), helper: None, gen: 0, pend: HashMap::new(), next: 0, srvs: vec![], wait: vec![], shown: String::new(), env: Env::default(), lines: lines.clone() };
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
    #[test] fn a_copy_runs_only_the_listed_kicad_reads() {
        for t in ["kiapi.board.commands.GetItems", "kiapi.common.commands.Ping", "kiapi.backplane.GetCapabilities"] { assert_eq!(copy_ipc_refused(true, t), None, "{t}") }
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

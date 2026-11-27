//! SolidCraft in the desktop's CAD pane: one window program per project root, started and watched by the hub.
//!
//! Boundary with the client (`native/gpui/src/viewers/`): the hub does no window work (it may have no display). It starts
//! `solidcraft --control PORT --new-instance <file>` on the display the client names, publishes the pid at once (`starting`), and the
//! client takes that process's window into its pane by pid, as it does stock KiCad's. Detach and bring-back are the client's window
//! moves; the hub only records where the window is (`container`, `toplevel`: 0 when it floats on its own).
//!
//! The control channel is the hub's: it chooses the port and the token (`SOLIDCRAFT_CONTROL_TOKEN`, 256 random bits a launch), keeps
//! the program's token and port files under `<home>/solidcraft/control` and its settings under `<home>/solidcraft/settings`
//! (machine-local, never the person's own SolidCraft profile). Over it the hub opens files in the running window (`doc.open`: a
//! `viewer_show` of another part never restarts the program), asks whether it answers, and saves a design before closing it. The
//! agent's MCP bridge reaches the same window with [`control`]: what the agent models, the person sees.
//!
//! Info `editor.solidcraft.<digest(root)>` (JSON): `{state: stopped|installing|starting|ready|unresponsive|failed, pid, gen, file, root,
//! control_port, container, toplevel, version, reason, install_progress: {done, total}, runtime: {installed, path, source, ...}}`.
//! RPCs: `editor.solidcraft.{attach, open, detach, bring_back, close}`, each answering with that picture; `install {consent}` and
//! `runtime {}` (look for the programs again) are the machine's, not a project's.
use super::runtime;
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, process::Stdio, sync::{Arc, Mutex, OnceLock}, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// How long a started program has to open its control channel before the start is called failed.
pub const START_LIMIT: Duration = Duration::from_secs(90);
/// How many health checks in a row may go unanswered before the session says `unresponsive` (a long recompute holds the program's
/// window thread, so one silence is not a hang).
pub const SILENT_CHECKS: u32 = 3;

// Pure state & rules ----------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Session {
    pub root: String,
    /// The file the window shows (absolute; "" for a blank design).
    pub file: String,
    pub state: String,
    pub pid: u32,
    /// Counts starts for this root: a late answer about an earlier program is told from the one running.
    pub gen: u64,
    pub port: u16,
    pub token: String,
    pub container: u64,
    pub toplevel: u64,
    pub display: String,
    pub version: String,
    pub reason: String,
    /// The close was asked for: the exit that follows is `stopped`, not `failed`.
    pub closing: bool,
    pub silent: u32,
    /// The pane left it (another page, another project) and keeps its window for a quick return: when, in seconds since the epoch;
    /// 0 while it is in a pane or popped out. A parked program is closed after [`parked_limit`].
    pub parked_at: u64,
}
impl Session {
    pub fn is_live(&self) -> bool { self.pid != 0 && matches!(self.state.as_str(), "starting" | "ready" | "unresponsive") }
    /// The picture clients read. The token never leaves the hub.
    pub fn to_json(&self, rt: &Value, install: &Value) -> Value {
        let installing = install["state"] == "running";
        let state = if !self.is_live() && installing { "installing" } else if self.state.is_empty() { "stopped" } else { &self.state };
        json!({
            "state": state, "pid": self.pid, "gen": self.gen, "file": self.file, "root": self.root,
            "control_port": if self.is_live() { self.port } else { 0 }, "container": self.container, "toplevel": self.toplevel, "parked": self.parked_at != 0,
            "version": self.version, "reason": self.reason,
            "install_progress": {"done": install["done"].as_u64().unwrap_or(0), "total": install["total"].as_u64().unwrap_or(0), "note": install["note"], "state": install["state"]},
            "runtime": rt,
        })
    }
}
/// Info key for a project root.
pub fn info_key(root: &str) -> String { format!("editor.solidcraft.{}", crate::files::kicad::kserve::digest(&norm_root(root))) }
/// One spelling per folder: no trailing separator.
pub fn norm_root(root: &str) -> String { let t = root.trim_end_matches(['/', '\\']); if t.is_empty() { root.to_string() } else { t.to_string() } }
/// The file a request names, absolute: as given when absolute, else under the root. Err for a name SolidCraft does not open or one
/// that climbs out of a relative path.
pub fn resolve_file(root: &str, file: &str) -> Result<String, String> {
    if file.is_empty() { return Ok(String::new()) }
    if !super::is_cad(file) { return Err(format!("{file} is not a file SolidCraft opens ({})", super::CAD_EXT.join(", "))) }
    let p = Path::new(file);
    if p.is_absolute() { return Ok(file.to_string()) }
    if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) { return Err(format!("{file} leaves the project folder")) }
    Ok(Path::new(root).join(p).to_string_lossy().into_owned())
}
/// The window program's arguments: the control port, a process of its own (never a tab in the person's own SolidCraft), the file.
pub fn argv(port: u16, file: &str) -> Vec<String> {
    let mut v = vec!["--control".to_string(), port.to_string(), "--new-instance".to_string()];
    if !file.is_empty() { v.push(file.to_string()); }
    v
}
/// One control request as its line: the token rides on every first (and here only) request of a connection.
pub fn request_line(id: u64, method: &str, params: &Value, token: &str) -> String { format!("{}\n", json!({"id": id, "method": method, "params": params, "token": token})) }
/// An engine command as the control channel takes it (`engine.execute`: a bare `doc.open` is an unknown method).
pub fn execute(command: &str, params: Value) -> (&'static str, Value) { ("engine.execute", json!({"command": command, "params": params})) }
/// A reply line read: Ok(result) or Err(the program's words).
pub fn parse_reply(line: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(line.trim()).map_err(|_| "SolidCraft's reply was not JSON".to_string())?;
    if v["ok"] == true { Ok(v["result"].clone()) } else { Err(v["error"].as_str().unwrap_or("SolidCraft refused the request").to_string()) }
}
/// What a health check does to a session: (changed, the state after).
pub fn health_step(s: &mut Session, answered: bool) -> bool {
    let before = s.state.clone();
    if answered { s.silent = 0; if s.state == "unresponsive" { s.state = "ready".into(); s.reason.clear(); } }
    else if s.state == "ready" { s.silent += 1; if s.silent >= SILENT_CHECKS { s.state = "unresponsive".into(); s.reason = "SolidCraft is not answering (it may be computing)".into(); } }
    s.state != before
}
/// What an exit does to a session: `stopped` when it was asked for or ended cleanly once ready (the person closed its window), else `failed` with why.
pub fn exit_step(s: &mut Session, code: Option<i32>, tail: &str) {
    let was_ready = matches!(s.state.as_str(), "ready" | "unresponsive");
    let clean = s.closing || (was_ready && code == Some(0));
    s.state = if clean { "stopped" } else { "failed" }.into();
    s.reason = if clean { if s.closing { "closed".into() } else { "SolidCraft was closed".into() } } else {
        let l = tail.lines().map(str::trim).rev().find(|l| !l.is_empty()).unwrap_or("");
        let how = code.map_or("was stopped by a signal".to_string(), |c| format!("exited with code {c}"));
        if l.is_empty() { format!("SolidCraft {how}") } else { format!("SolidCraft {how}: {}", l.chars().take(240).collect::<String>()) }
    };
    s.pid = 0; s.closing = false; s.silent = 0; s.container = 0; s.toplevel = 0; s.parked_at = 0;
}
/// How long a parked program is kept (`BACKPLANE_SOLIDCRAFT_IDLE_S`, default ten minutes).
pub fn parked_limit(env: Option<&str>) -> u64 { env.and_then(|v| v.trim().parse().ok()).unwrap_or(600) }
/// The roots whose programs have been parked longer than `limit` seconds: closed, so programs do not pile up across projects.
pub fn parked_victims(sessions: &HashMap<String, Session>, now: u64, limit: u64) -> Vec<String> {
    let mut v: Vec<String> = sessions.values().filter(|s| s.is_live() && !s.closing && s.parked_at != 0 && now.saturating_sub(s.parked_at) >= limit).map(|s| s.root.clone()).collect();
    v.sort(); v
}
fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }

// Sessions ------------------------------------------------------------------------------------------------------------------

fn sessions() -> &'static Mutex<HashMap<String, Session>> { static S: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new(); S.get_or_init(Default::default) }
fn with<R>(root: &str, f: impl FnOnce(&mut Session) -> R) -> R {
    let mut m = sessions().lock().unwrap();
    let s = m.entry(norm_root(root)).or_insert_with(|| Session { root: norm_root(root), state: "stopped".into(), ..Default::default() });
    f(s)
}
fn snapshot(root: &str) -> Session { with(root, |s| s.clone()) }

/// The live window's control channel for a project root: what the agent's MCP bridge connects to
/// (`solidcraft-cli mcp --connect 127.0.0.1:<port> --token <token>`). None while no window is ready.
#[derive(Clone, Debug, PartialEq)]
pub struct Control { pub port: u16, pub token: String, pub file: String }
pub fn control(root: &str) -> Option<Control> {
    let m = sessions().lock().unwrap();
    m.get(&norm_root(root)).filter(|s| s.pid != 0 && matches!(s.state.as_str(), "ready" | "unresponsive")).map(|s| Control { port: s.port, token: s.token.clone(), file: s.file.clone() })
}
/// The control channel of the window that has `path` open or whose root holds it (an MCP tool given only a file).
pub fn control_for_path(path: &str) -> Option<Control> {
    let m = sessions().lock().unwrap();
    m.values().filter(|s| s.pid != 0 && matches!(s.state.as_str(), "ready" | "unresponsive"))
        .filter(|s| s.file == path || Path::new(path).starts_with(&s.root)).max_by_key(|s| (s.file == path, s.root.len()))
        .map(|s| Control { port: s.port, token: s.token.clone(), file: s.file.clone() })
}
/// The folders the program keeps its control files and settings in, under the hub's home.
pub fn control_dir(home: &Path) -> PathBuf { home.join("solidcraft").join("control") }
pub fn settings_dir(home: &Path) -> PathBuf { home.join("solidcraft").join("settings") }
/// Its recovery files, crash reports and logs (`XDG_DATA_HOME`): the hub's, never the person's own SolidCraft's.
pub fn data_dir(home: &Path) -> PathBuf { home.join("solidcraft").join("data") }
/// The environment every SolidCraft the hub starts gets (the window program and the agent's `solidcraft-cli` alike, so they share
/// one settings and recovery folder): its folders under the hub's home, no usage statistics, no update check.
pub fn data_env(home: &Path) -> Vec<(&'static str, std::ffi::OsString)> {
    vec![("SOLIDCRAFT_SETTINGS_DIR", settings_dir(home).into()), ("SOLIDCRAFT_CONTROL_DIR", control_dir(home).into()), ("XDG_DATA_HOME", data_dir(home).into()),
        ("SOLIDCRAFT_NO_TELEMETRY", "1".into()), ("SOLIDCRAFT_UPDATE_CHECK", "0".into())]
}
/// The settings a first start gets, as SolidCraft's own file holds them (a map of names to texts; `solidcraft.prefs` is JSON, and
/// what it leaves out keeps its default). Without them a first start asks how to import a STEP in a dialog nobody can answer in a
/// headless run (seen with 0.50.0: the model never loads), and shows a statistics notice and donation and chat buttons in the pane.
pub fn seed_text() -> String {
    let prefs = json!({"preferences": {"ask_import_options": false, "show_discord_button": false, "show_donate_button": false, "check_updates": false, "usage_statistics": false, "show_home_on_startup": false}});
    let stats = json!({"install_id": "", "notice_ok": true, "days": {}, "forget": []});
    let q = |v: &Value| Value::String(v.to_string()).to_string();
    format!("{{\n    \"solidcraft.prefs\": {},\n    \"usage_statistics\": {},\n}}\n", q(&prefs), q(&stats))
}
/// The hub's SolidCraft folders made (private to the user), and the settings seeded where there are none yet: what the person
/// changes in the pane afterwards is theirs and is never written over.
pub fn seed_settings(home: &Path) -> Result<(), String> {
    for d in [control_dir(home), settings_dir(home), data_dir(home)] {
        std::fs::create_dir_all(&d).map_err(|e| format!("Could not make {}: {e}", d.display()))?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700)); }
    }
    let f = settings_dir(home).join("app.ron");
    if !f.exists() { std::fs::write(&f, seed_text()).map_err(|e| format!("Could not write {}: {e}", f.display()))?; }
    Ok(())
}

/// One request on a connection of its own; `limit` bounds connect, send and the reply.
pub async fn call(port: u16, token: &str, method: &str, params: Value, limit: Duration) -> Result<Value, String> {
    if crate::copy::on() { return Err("a hub on a copied log does not control SolidCraft".into()) }
    let line = request_line(1, method, &params, token);
    let io = async {
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.map_err(|e| format!("no SolidCraft is listening on port {port}: {e}"))?;
        s.write_all(line.as_bytes()).await.map_err(|e| e.to_string())?;
        let mut r = BufReader::new(s); let mut reply = String::new();
        // event lines (no id) are not ours; the first line with an id, or a refusal, is
        loop {
            reply.clear();
            if r.read_line(&mut reply).await.map_err(|e| e.to_string())? == 0 { return Err("SolidCraft closed the connection".to_string()) }
            let v: Value = serde_json::from_str(reply.trim()).unwrap_or(Value::Null);
            if v.get("event").is_some() && v.get("id").is_none() { continue }
            return parse_reply(&reply)
        }
    };
    tokio::time::timeout(limit, io).await.map_err(|_| format!("SolidCraft did not answer {method} within {} s", limit.as_secs()))?
}
/// An engine command in the live window of `root`.
pub async fn command(root: &str, cmd: &str, params: Value, limit: Duration) -> Result<Value, String> {
    let c = control(root).ok_or("SolidCraft is not open for this project")?;
    let (m, p) = execute(cmd, params);
    call(c.port, &c.token, m, p, limit).await
}

async fn publish(hub: &Arc<Hub>, root: &str) -> Value {
    let info = hub.info_now();
    let read = |k: &str| info[k].as_str().and_then(|s| serde_json::from_str::<Value>(s).ok()).unwrap_or(Value::Null);
    let (mut rt, install) = (read("solidcraft.runtime"), read("solidcraft.install"));
    if rt.is_null() { rt = runtime::publish(hub).await.to_json(); }
    let v = snapshot(root).to_json(&rt, &install);
    hub.set_info(&info_key(root), &v.to_string()).await;
    v
}
fn free_port() -> Result<u16, String> { std::net::TcpListener::bind(("127.0.0.1", 0)).and_then(|l| l.local_addr()).map(|a| a.port()).map_err(|e| format!("no free port for SolidCraft's control channel: {e}")) }
fn new_token() -> Result<String, String> { let mut b = [0u8; 32]; crate::plat::fs::random(&mut b).map_err(|_| "no secure random source".to_string())?; Ok(hex::encode(b)) }

/// Start the window program for `root` showing `file`, and watch it: its control channel answering makes it `ready`, its exit
/// `stopped` or `failed`. Returns once the pid is published (`starting`).
async fn spawn(hub: &Arc<Hub>, root: &str, file: &str, display: &str, container: u64, toplevel: u64) -> Result<(), String> {
    let rt = { let h = hub.cfg.home.clone(); tokio::task::spawn_blocking(move || runtime::find(&h)).await.unwrap_or_default() };
    let gui = rt.gui.clone().ok_or_else(|| if rt.cli.is_some() { "This computer has SolidCraft's command-line program but not its window program".to_string() } else { "SolidCraft is not installed".to_string() })?;
    let (port, token) = (free_port()?, new_token()?);
    let home = hub.cfg.home.clone();
    { let h = home.clone(); tokio::task::spawn_blocking(move || seed_settings(&h)).await.map_err(|e| e.to_string())??; }
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, &gui);
    c.args(argv(port, file)).current_dir(root).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped())
        .env("SOLIDCRAFT_CONTROL_TOKEN", &token).envs(data_env(&home))
        // the pane takes an X11 window in (Linux): the program is kept off Wayland, as KiCad is
        .env_remove("WAYLAND_DISPLAY").env("WINIT_UNIX_BACKEND", "x11").env_remove("SOLIDCRAFT_CONTROL_PORT");
    if !display.is_empty() { c.env("DISPLAY", display); }
    crate::plat::proc::own_group_tokio(&mut c);
    let mut child = c.spawn().map_err(|e| format!("Could not start {}: {e}", gui.display()))?;
    let pid = child.id().unwrap_or(0);
    let gen = with(root, |s| {
        let gen = s.gen + 1;
        *s = Session { root: s.root.clone(), file: file.to_string(), state: "starting".into(), pid, gen, port, token: token.clone(), container, toplevel, display: display.to_string(), version: rt.version.clone(), ..Default::default() };
        gen
    });
    publish(hub, root).await;
    // stderr: its last lines say why a start failed
    let tail = Arc::new(Mutex::new(std::collections::VecDeque::<String>::new()));
    if let Some(e) = child.stderr.take() {
        let tail = tail.clone();
        tokio::spawn(async move { let mut l = BufReader::new(e).lines(); while let Ok(Some(line)) = l.next_line().await { let mut t = tail.lock().unwrap(); if t.len() >= 20 { t.pop_front(); } t.push_back(line); } });
    }
    // ready: the control channel answers
    { let (hub, root, token) = (hub.clone(), root.to_string(), token.clone()); tokio::spawn(async move {
        let t0 = std::time::Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if with(&root, |s| s.gen != gen || s.state != "starting") { return }
            match call(port, &token, "control.capabilities", json!({}), Duration::from_secs(5)).await {
                Ok(_) => {
                    if with(&root, |s| { if s.gen == gen && s.state == "starting" { s.state = "ready".into(); true } else { false } }) { publish(&hub, &root).await; }
                    return
                }
                Err(_) if t0.elapsed() < START_LIMIT => {}
                Err(why) => {
                    // never answered: stopped, and the exit says failed with this reason
                    let hit = with(&root, |s| if s.gen == gen && s.state == "starting" { s.reason = format!("SolidCraft did not open its control channel within {} s ({why})", START_LIMIT.as_secs()); Some(s.pid) } else { None });
                    if let Some(pid) = hit { crate::plat::proc::kill_group(pid, 15); }
                    return
                }
            }
        }
    }); }
    // its exit
    { let (hub, root) = (hub.clone(), root.to_string()); tokio::spawn(async move {
        let status = crate::proc::wait(&mut child).await;
        let code = status.ok().and_then(|s| s.code());
        tokio::time::sleep(Duration::from_millis(50)).await; // the last stderr lines
        let tail = tail.lock().unwrap().iter().cloned().collect::<Vec<_>>().join("\n");
        let ours = with(&root, |s| { if s.gen != gen { return false } let kept = std::mem::take(&mut s.reason); exit_step(s, code, &tail); if s.state == "failed" && kept.contains("control channel") { s.reason = kept; } true });
        if ours { publish(&hub, &root).await; }
    }); }
    Ok(())
}
/// Ask the live window to show `file` (a tab of its own, made active). The session's `file` follows on success.
async fn open_in(hub: &Arc<Hub>, root: &str, file: &str) -> Result<(), String> {
    command(root, "doc.open", json!({"path": file}), Duration::from_secs(120)).await?;
    with(root, |s| s.file = file.to_string());
    publish(hub, root).await;
    Ok(())
}
/// A new design in the live window, in a tab of its own, saved at `file` at once so it has a path from its first second (the
/// Files tab shows it, the agent's checkpoints and autosave have somewhere to write). The session's `file` follows.
async fn new_in(hub: &Arc<Hub>, root: &str, file: &str) -> Result<(), String> {
    if Path::new(file).exists() { return Err(format!("{file} already exists")) }
    if let Some(d) = Path::new(file).parent() { std::fs::create_dir_all(d).map_err(|e| format!("Could not make {}: {e}", d.display()))?; }
    let name = Path::new(file).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    command(root, "file.new", json!({"name": name, "new_tab": true}), Duration::from_secs(30)).await?;
    command(root, "file.save_as", json!({"path": file}), Duration::from_secs(30)).await?;
    with(root, |s| s.file = file.to_string());
    publish(hub, root).await;
    Ok(())
}
/// The first free `mech/part-N.sc` under a project root (N from 1): where "New 3D part" puts a design nobody named.
pub fn next_part(root: &str) -> String {
    let dir = Path::new(root).join("mech");
    (1..10_000).map(|n| dir.join(format!("part-{n}.sc"))).find(|p| !p.exists()).unwrap_or_else(|| dir.join("part.sc")).to_string_lossy().into_owned()
}
/// Close the program of `root`: a design is saved first (its edits are the person's and the agent's work), then it is asked to end,
/// and stopped for good if it has not within five seconds.
pub async fn close(hub: &Arc<Hub>, root: &str, save: bool) {
    let s = snapshot(root);
    if !s.is_live() { return }
    if save && s.state == "ready" && super::is_design(&s.file) { let _ = command(root, "file.save", json!({}), Duration::from_secs(20)).await; }
    with(root, |x| if x.gen == s.gen { x.closing = true });
    crate::plat::proc::kill_group(s.pid, 15);
    let (hub, root) = (hub.clone(), root.to_string());
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        if with(&root, |x| x.gen == s.gen && x.pid == s.pid && x.is_live()) { crate::plat::proc::kill_group(s.pid, 9); }
        let _ = &hub;
    });
}
/// Every program ended, at hub shutdown (no save: the hub is going; SolidCraft keeps its own recovery files).
pub fn shutdown_all() {
    let pids: Vec<u32> = sessions().lock().unwrap().values_mut().filter(|s| s.is_live()).map(|s| { s.closing = true; s.pid }).collect();
    for p in pids { crate::plat::proc::kill_group(p, 15); }
}

// Hub wiring & RPC handler --------------------------------------------------------------------------------------------------

const METHODS: &[&str] = &["editor.solidcraft.attach", "editor.solidcraft.open", "editor.solidcraft.detach", "editor.solidcraft.bring_back", "editor.solidcraft.close", "editor.solidcraft.install", "editor.solidcraft.runtime"];
pub struct EmbedRpc;

fn ok_reply(id: &Value, payload: Value) -> Vec<Value> {
    let mut r = json!({"t":"reply","id":id,"ok":true});
    if let (Some(dst), Some(src)) = (r.as_object_mut(), payload.as_object()) { for (k, v) in src { if k != "t" && k != "id" && k != "ok" { dst.insert(k.clone(), v.clone()); } } }
    vec![r]
}
fn err_reply(id: &Value, error: &str) -> Vec<Value> { vec![json!({"t":"reply","id":id,"ok":false,"error":error})] }
/// The project folder a request names: `root`, else `project` (a folder, or a file in it: its folder).
pub fn root_of(p: &Value) -> Result<String, String> {
    let raw = p["root"].as_str().filter(|s| !s.is_empty()).or(p["project"].as_str().filter(|s| !s.is_empty())).ok_or("No project was named")?;
    let path = Path::new(raw);
    if !path.is_absolute() { return Err(format!("{raw} is not an absolute path")) }
    let dir = if path.is_dir() { path } else if super::is_cad(raw) { path.parent().unwrap_or(path) } else { path };
    Ok(norm_root(&dir.to_string_lossy()))
}

impl RpcHandler for EmbedRpc {
    fn methods(&self) -> &'static [&'static str] { METHODS }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if crate::copy::on() { return err_reply(id, "a hub on a copied log (BACKPLANE_COPY=1) does not start or control SolidCraft") }
            if method == "editor.solidcraft.install" {
                return match runtime::install(hub, p["consent"].as_bool().unwrap_or(false)).await { Ok(note) => ok_reply(id, json!({"note": note})), Err(why) => err_reply(id, &why) }
            }
            // the programs looked for again (Settings > CAD set a folder, setup's Check again): every pane's picture follows
            if method == "editor.solidcraft.runtime" {
                let rt = runtime::publish(hub).await;
                let roots: Vec<String> = sessions().lock().unwrap().values().filter(|s| !s.is_live()).map(|s| s.root.clone()).collect();
                for r in roots { publish(hub, &r).await; }
                return ok_reply(id, rt.to_json())
            }
            let root = match root_of(p) { Ok(r) => r, Err(why) => return err_reply(id, &why) };
            if !crate::files::kicad::known_root(hub, &root, "").await { return err_reply(id, &format!("{root} is not in a project this hub knows")) }
            // `create` with no file: a new design at the first free mech/part-N.sc
            let create = p["create"].as_bool().unwrap_or(false);
            let named = p["file"].as_str().unwrap_or("");
            let file = if create && named.is_empty() { next_part(&root) } else { match resolve_file(&root, named) { Ok(f) => f, Err(why) => return err_reply(id, &why) } };
            if create && !super::is_design(&file) { return err_reply(id, "A new design is a .sc file") }
            // a file outside the project is opened only where the hub may show it (a link the person followed); a new design need not exist yet
            if !file.is_empty() && !Path::new(&file).starts_with(&root) && !crate::files::kicad::known_root(hub, &root, &file).await { return err_reply(id, &format!("{file} is not in a project this hub knows")) }
            let (container, toplevel) = (p["container"].as_u64().unwrap_or(0), p["toplevel"].as_u64().unwrap_or(0));
            match method {
                "editor.solidcraft.attach" | "editor.solidcraft.bring_back" => {
                    let s = snapshot(&root);
                    if s.is_live() {
                        with(&root, |x| { x.container = container; x.toplevel = toplevel; x.parked_at = 0; });
                        let fresh = super::is_design(&file) && !Path::new(&file).exists();
                        if !file.is_empty() && file != s.file && s.state != "starting" { if let Err(why) = if fresh { new_in(hub, &root, &file).await } else { open_in(hub, &root, &file).await } { let mut v = publish(hub, &root).await; v["open_error"] = json!(why); return ok_reply(id, v) } }
                        return ok_reply(id, publish(hub, &root).await)
                    }
                    if method == "editor.solidcraft.bring_back" { return err_reply(id, "SolidCraft is not open for this project") }
                    if !file.is_empty() && !Path::new(&file).is_file() && !super::is_design(&file) { return err_reply(id, &format!("{file} does not exist")) }
                    // a design that does not exist yet: a blank one, saved there once the window is up
                    let (arg, new) = if !file.is_empty() && !Path::new(&file).is_file() { (String::new(), Some(file.clone())) } else { (file.clone(), None) };
                    if runtime::publish(hub).await.gui.is_none() {
                        with(&root, |x| { x.state = "stopped".into(); x.reason = "SolidCraft is not installed".into(); x.file = file.clone(); });
                        return ok_reply(id, publish(hub, &root).await)
                    }
                    if let Err(why) = spawn(hub, &root, &arg, p["display"].as_str().unwrap_or(""), container, toplevel).await {
                        with(&root, |x| { x.state = "failed".into(); x.reason = why; });
                        return ok_reply(id, publish(hub, &root).await)
                    }
                    if let Some(path) = new {
                        let (hub, root) = (hub.clone(), root.clone());
                        tokio::spawn(async move {
                            for _ in 0..600 { tokio::time::sleep(Duration::from_millis(150)).await; let st = snapshot(&root).state; if st == "ready" { break } if st != "starting" { return } }
                            if let Some(d) = Path::new(&path).parent() { let _ = std::fs::create_dir_all(d); }
                            match command(&root, "file.save_as", json!({"path": path}), Duration::from_secs(30)).await {
                                Ok(_) => { with(&root, |x| x.file = path.clone()); }
                                Err(why) => { eprintln!("solidcraft embed: new design not saved: {why}"); with(&root, |x| x.reason = format!("The new design was not saved: {why}")); }
                            }
                            publish(&hub, &root).await;
                        });
                    }
                    ok_reply(id, publish(hub, &root).await)
                }
                "editor.solidcraft.open" => {
                    if file.is_empty() { return err_reply(id, "No file was named") }
                    let fresh = create && super::is_design(&file) && !Path::new(&file).exists();
                    if !fresh && !Path::new(&file).is_file() { return err_reply(id, &format!("{file} does not exist")) }
                    match if fresh { new_in(hub, &root, &file).await } else { open_in(hub, &root, &file).await } { Ok(()) => ok_reply(id, publish(hub, &root).await), Err(why) => err_reply(id, &why) }
                }
                "editor.solidcraft.detach" => {
                    let keep = p["keep"].as_bool().unwrap_or(false);
                    with(&root, |x| { x.container = 0; x.toplevel = 0; x.parked_at = if keep && x.is_live() { now_s().max(1) } else { 0 }; });
                    ok_reply(id, publish(hub, &root).await)
                }
                "editor.solidcraft.close" => {
                    close(hub, &root, p["save"].as_bool().unwrap_or(true)).await;
                    ok_reply(id, publish(hub, &root).await)
                }
                _ => err_reply(id, "unknown method"),
            }
        })
    }
}

pub fn start(hub: &Arc<Hub>) {
    hub.register_rpc(Arc::new(EmbedRpc));
    let hub = hub.clone();
    // health: a ready window that stops answering is said so, and said ready again when it answers; an install's progress reaches
    // every stopped pane's picture
    tokio::spawn(async move {
        let mut was_installing = false;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let live: Vec<Session> = sessions().lock().unwrap().values().filter(|s| s.pid != 0 && matches!(s.state.as_str(), "ready" | "unresponsive")).cloned().collect();
            for s in live {
                let answered = call(s.port, &s.token, "control.capabilities", json!({}), Duration::from_secs(4)).await.is_ok();
                if with(&s.root, |x| x.gen == s.gen && x.is_live() && health_step(x, answered)) { publish(&hub, &s.root).await; }
            }
            let limit = parked_limit(std::env::var("BACKPLANE_SOLIDCRAFT_IDLE_S").ok().as_deref());
            let old = parked_victims(&sessions().lock().unwrap(), now_s(), limit);
            for r in old { eprintln!("solidcraft embed: closing the program parked for {r} ({limit} s out of any pane)"); close(&hub, &r, true).await; }
            let installing = runtime::running();
            if installing || was_installing {
                let idle: Vec<String> = sessions().lock().unwrap().values().filter(|s| !s.is_live()).map(|s| s.root.clone()).collect();
                for r in idle { publish(&hub, &r).await; }
            }
            was_installing = installing;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_files_and_arguments() {
        assert_eq!(info_key("/home/user/lamp/"), info_key("/home/user/lamp"));
        assert!(info_key("/home/user/lamp").starts_with("editor.solidcraft.") && info_key("/home/user/lamp") != info_key("/home/user/lamp2"));
        assert_eq!(resolve_file("/p", "mech/case.step").unwrap(), "/p/mech/case.step");
        assert_eq!(resolve_file("/p", "/elsewhere/a.sc").unwrap(), "/elsewhere/a.sc");
        assert_eq!(resolve_file("/p", "").unwrap(), "");
        assert!(resolve_file("/p", "../x.step").unwrap_err().contains("leaves"));
        assert!(resolve_file("/p", "board.kicad_pcb").unwrap_err().contains("not a file SolidCraft opens"));
        assert_eq!(argv(7878, "/p/a.step"), ["--control", "7878", "--new-instance", "/p/a.step"]);
        assert_eq!(argv(7878, ""), ["--control", "7878", "--new-instance"]);
        assert!(root_of(&json!({})).is_err() && root_of(&json!({"project": "rel/x"})).is_err());
        assert_eq!(root_of(&json!({"project": "/nonexistent-root/mech/a.step"})).unwrap(), "/nonexistent-root/mech");
        assert_eq!(root_of(&json!({"project": "/a", "root": "/nonexistent-root/"})).unwrap(), "/nonexistent-root");
    }
    #[test]
    fn seeded_settings() {
        let t = seed_text();
        assert!(t.starts_with("{\n    \"solidcraft.prefs\": \"{") && t.ends_with(",\n}\n"));
        // each value is a text holding JSON, as SolidCraft's settings file keeps them
        let line = t.lines().nth(1).unwrap().trim().trim_end_matches(','); let (_, v) = line.split_once(": ").unwrap();
        let inner: String = serde_json::from_str(v).unwrap(); let p: Value = serde_json::from_str(&inner).unwrap();
        assert_eq!(p["preferences"]["ask_import_options"], false); assert_eq!(p["preferences"]["show_donate_button"], false); assert_eq!(p["preferences"]["usage_statistics"], false);
        let line = t.lines().nth(2).unwrap().trim().trim_end_matches(','); let (k, v) = line.split_once(": ").unwrap();
        let inner: String = serde_json::from_str(v).unwrap(); assert_eq!(k, "\"usage_statistics\""); assert_eq!(serde_json::from_str::<Value>(&inner).unwrap()["notice_ok"], true);
        let env = data_env(Path::new("/home/user/.bp")); assert_eq!(env.len(), 5);
        assert!(env.iter().any(|(k, v)| *k == "XDG_DATA_HOME" && v == "/home/user/.bp/solidcraft/data"));
    }
    #[test]
    fn wire() {
        let l = request_line(1, "control.capabilities", &json!({}), "ab");
        assert!(l.ends_with('\n') && !l[..l.len() - 1].contains('\n'));
        assert_eq!(serde_json::from_str::<Value>(&l).unwrap(), json!({"id": 1, "method": "control.capabilities", "params": {}, "token": "ab"}));
        assert_eq!(execute("doc.open", json!({"path": "/p/a.step"})), ("engine.execute", json!({"command": "doc.open", "params": {"path": "/p/a.step"}})));
        assert_eq!(parse_reply("{\"id\":1,\"ok\":true,\"result\":{\"version\":\"0.50.0\"}}\n").unwrap()["version"], "0.50.0");
        assert_eq!(parse_reply("{\"ok\":false,\"error\":\"unauthorized: bad token\"}").unwrap_err(), "unauthorized: bad token");
        assert!(parse_reply("nonsense").is_err());
    }
    #[test]
    fn picture_and_steps() {
        let mut s = Session { root: "/p".into(), file: "/p/a.step".into(), state: "ready".into(), pid: 42, gen: 3, port: 7878, token: "secret".into(), container: 9, toplevel: 8, ..Default::default() };
        let rt = json!({"installed": true});
        let v = s.to_json(&rt, &Value::Null);
        assert_eq!((v["state"].as_str(), v["pid"].as_u64(), v["gen"].as_u64(), v["control_port"].as_u64(), v["container"].as_u64()), (Some("ready"), Some(42), Some(3), Some(7878), Some(9)));
        assert!(!v.to_string().contains("secret"), "the token never leaves the hub");
        // silence: unresponsive after three checks, ready again at the first answer
        assert!(!health_step(&mut s, false) && !health_step(&mut s, false) && health_step(&mut s, false)); assert_eq!(s.state, "unresponsive");
        assert!(!health_step(&mut s, false)); assert!(health_step(&mut s, true)); assert_eq!((s.state.as_str(), s.silent, s.reason.as_str()), ("ready", 0, ""));
        // the person closed its window: stopped, not a failure
        let mut a = s.clone(); exit_step(&mut a, Some(0), ""); assert_eq!((a.state.as_str(), a.pid, a.container), ("stopped", 0, 0));
        assert_eq!(a.to_json(&rt, &Value::Null)["control_port"], 0);
        // a crash says why
        let mut b = s.clone(); exit_step(&mut b, Some(101), "thread 'main' panicked\n\n"); assert_eq!(b.state, "failed"); assert!(b.reason.contains("code 101") && b.reason.contains("panicked"));
        let mut c = s.clone(); exit_step(&mut c, None, ""); assert_eq!(c.reason, "SolidCraft was stopped by a signal");
        // asked to close: stopped whatever the code
        let mut d = s.clone(); d.closing = true; exit_step(&mut d, None, "x"); assert_eq!((d.state.as_str(), d.reason.as_str(), d.closing), ("stopped", "closed", false));
        // a start that ends before it was ready failed, even with code 0
        let mut e = Session { state: "starting".into(), pid: 5, ..Default::default() }; exit_step(&mut e, Some(0), ""); assert_eq!(e.state, "failed");
        // an install shows in a stopped pane's picture, never over a live one
        let inst = json!({"state": "running", "done": 5, "total": 10, "note": "Downloading"});
        assert_eq!(a.to_json(&rt, &inst)["state"], "installing"); assert_eq!(a.to_json(&rt, &inst)["install_progress"]["done"], 5);
        assert_eq!(s.to_json(&rt, &inst)["state"], "ready");
        assert_eq!(Session::default().to_json(&rt, &Value::Null)["state"], "stopped");
        // parked programs are closed after the limit; one in a pane, popped out, closing or gone never is
        assert_eq!((parked_limit(None), parked_limit(Some("30")), parked_limit(Some("x"))), (600, 30, 600));
        let mut m = HashMap::new();
        m.insert("/a".to_string(), Session { root: "/a".into(), parked_at: 100, ..s.clone() });
        m.insert("/b".to_string(), Session { root: "/b".into(), parked_at: 0, ..s.clone() });
        m.insert("/c".to_string(), Session { root: "/c".into(), parked_at: 650, ..s.clone() });
        m.insert("/d".to_string(), Session { root: "/d".into(), parked_at: 100, closing: true, ..s.clone() });
        m.insert("/e".to_string(), Session { root: "/e".into(), parked_at: 100, ..a.clone() });
        assert_eq!(parked_victims(&m, 700, 600), vec!["/a"]); assert_eq!(parked_victims(&m, 1300, 600), vec!["/a", "/c"]);
        assert!(s.to_json(&rt, &Value::Null)["parked"] == false && m["/a"].to_json(&rt, &Value::Null)["parked"] == true);
    }
    /// The whole of a session against a stand-in program: it notes its arguments and environment and waits; the test answers on the
    /// port the hub chose, as the program's control channel would.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_session_from_attach_to_close() {
        use std::os::unix::fs::PermissionsExt;
        let base = crate::plat::tmp::short_dir("sc-e");
        let (home, proj, bin) = (base.join("home"), base.join("proj"), base.join("bin"));
        for d in [&home, &proj.join("mech"), &bin] { std::fs::create_dir_all(d).unwrap(); }
        for f in ["mech/a.step", "mech/b.step"] { std::fs::write(proj.join(f), "ISO-10303-21;").unwrap(); }
        let cfg = crate::hub::Cfg { home: home.clone(), version: "0.14.0".into(), commit: "test".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
        start(&hub);
        let root = proj.to_string_lossy().into_owned();
        let _ = hub.commit(&json!({"$":"ProjectCreated","id":"p1","title":"case","root":root,"at":1})).await;
        let (mut conn, _) = hub.connect(true);
        let key = info_key(&root);
        let info = |hub: &Arc<Hub>| serde_json::from_str::<Value>(hub.info_now()[&key].as_str().unwrap_or("null")).unwrap_or(Value::Null);
        // no SolidCraft: the pane is told, nothing starts
        let r = hub.request(&mut conn, &json!({"id": 1, "m": "editor.solidcraft.attach", "p": {"project": root, "file": "mech/a.step", "container": 9, "toplevel": 8, "display": ":99"}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert_eq!((r["ok"].as_bool(), r["state"].as_str(), r["runtime"]["installed"].as_bool(), r["pid"].as_u64()), (Some(true), Some("stopped"), Some(false), Some(0)), "{r}");
        // a folder the hub does not know, and a file SolidCraft does not open, are refused
        let r = hub.request(&mut conn, &json!({"id": 2, "m": "editor.solidcraft.attach", "p": {"project": "/nonexistent-elsewhere", "file": "a.step"}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert!(r["ok"] == false && r["error"].as_str().unwrap().contains("not in a project"), "{r}");
        let r = hub.request(&mut conn, &json!({"id": 3, "m": "editor.solidcraft.attach", "p": {"project": root, "file": "x.kicad_pcb"}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert!(r["ok"] == false, "{r}");
        let r = hub.request(&mut conn, &json!({"id": 4, "m": "editor.solidcraft.attach", "p": {"project": root, "file": "mech/gone.step", "container": 1}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert!(r["ok"] == false && r["error"].as_str().unwrap().contains("does not exist"), "{r}");
        // the stand-in program
        let noted = base.join("noted");
        std::fs::write(bin.join("solidcraft"), format!("#!/bin/sh\necho \"$@\" > '{n}.tmp'\necho \"$SOLIDCRAFT_CONTROL_TOKEN $DISPLAY [$WAYLAND_DISPLAY] $SOLIDCRAFT_CONTROL_DIR $SOLIDCRAFT_SETTINGS_DIR $XDG_DATA_HOME $SOLIDCRAFT_NO_TELEMETRY $SOLIDCRAFT_UPDATE_CHECK\" >> '{n}.tmp'\nmv '{n}.tmp' '{n}'\nexec sleep 300\n", n = noted.display())).unwrap();
        std::fs::set_permissions(bin.join("solidcraft"), std::fs::Permissions::from_mode(0o755)).unwrap();
        runtime::test_over().lock().unwrap().insert(home.clone(), bin.clone());
        let r = hub.request(&mut conn, &json!({"id": 5, "m": "editor.solidcraft.attach", "p": {"project": root, "file": "mech/a.step", "container": 9, "toplevel": 8, "display": ":99"}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert_eq!((r["state"].as_str(), r["gen"].as_u64(), r["container"].as_u64(), r["runtime"]["installed"].as_bool()), (Some("starting"), Some(1), Some(9), Some(true)), "{r}");
        let pid = r["pid"].as_u64().unwrap() as u32; assert!(pid > 0 && crate::plat::proc::alive(pid));
        assert_eq!(control(&root), None, "no channel for the agent before the window is ready");
        for _ in 0..200 { if noted.is_file() { break } tokio::time::sleep(Duration::from_millis(25)).await; }
        let text = std::fs::read_to_string(&noted).unwrap(); let mut lines = text.lines();
        let args: Vec<&str> = lines.next().unwrap().split(' ').collect(); let env: Vec<&str> = lines.next().unwrap().split(' ').collect();
        assert_eq!((args[0], args[2], args[3]), ("--control", "--new-instance", proj.join("mech/a.step").to_str().unwrap()));
        let port: u16 = args[1].parse().unwrap(); assert_eq!(r["control_port"], port);
        assert_eq!((env[0].len(), env[1], env[2]), (64, ":99", "[]"), "a token of 256 bits, the client's display, never Wayland");
        assert_eq!((Path::new(env[3]), Path::new(env[4])), (control_dir(&home).as_path(), settings_dir(&home).as_path()));
        assert_eq!(env[5..], [data_dir(&home).to_str().unwrap(), "1", "0"], "its recovery folder is the hub's; no statistics, no update check");
        // the first start's settings were seeded, and a later start leaves the person's changes alone
        let seeded = std::fs::read_to_string(settings_dir(&home).join("app.ron")).unwrap(); assert_eq!(seeded, seed_text());
        std::fs::write(settings_dir(&home).join("app.ron"), "{\n}\n").unwrap(); seed_settings(&home).unwrap();
        assert_eq!(std::fs::read_to_string(settings_dir(&home).join("app.ron")).unwrap(), "{\n}\n");
        let token = env[0].to_string();
        // its control channel answers: ready, with its version
        let opened = Arc::new(Mutex::new(Vec::<String>::new()));
        let l = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
        { let (opened, token) = (opened.clone(), token.clone()); tokio::spawn(async move { loop {
            let Ok((s, _)) = l.accept().await else { return }; let (r, mut w) = s.into_split(); let mut line = String::new();
            if BufReader::new(r).read_line(&mut line).await.unwrap_or(0) == 0 { continue }
            let v: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
            let reply = if v["token"] != token.as_str() { json!({"ok": false, "error": "unauthorized"}) } else if v["method"] == "control.capabilities" { json!({"id": 1, "ok": true, "result": {"version": "0.50.0"}}) }
                else { opened.lock().unwrap().push(format!("{} {}", v["params"]["command"].as_str().unwrap_or(""), v["params"]["params"]["path"].as_str().unwrap_or(""))); json!({"id": 1, "ok": true, "result": {}}) };
            let _ = w.write_all(format!("{reply}\n").as_bytes()).await;
        } }); }
        for _ in 0..200 { if info(&hub)["state"] == "ready" { break } tokio::time::sleep(Duration::from_millis(25)).await; }
        let i = info(&hub); assert_eq!((i["state"].as_str(), i["file"].as_str()), (Some("ready"), proj.join("mech/a.step").to_str()), "{i}");
        assert_eq!(control(&root).map(|c| (c.port, c.token)), Some((port, token.clone())));
        // another part: opened in the same program (the same pid and generation)
        let r = hub.request(&mut conn, &json!({"id": 6, "m": "editor.solidcraft.attach", "p": {"project": root, "file": "mech/b.step", "container": 9, "toplevel": 8}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert_eq!((r["pid"].as_u64(), r["gen"].as_u64(), r["file"].as_str()), (Some(pid as u64), Some(1), proj.join("mech/b.step").to_str()), "{r}");
        let r = hub.request(&mut conn, &json!({"id": 7, "m": "editor.solidcraft.open", "p": {"project": root, "file": "mech/a.step"}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert_eq!(r["file"].as_str(), proj.join("mech/a.step").to_str(), "{r}");
        assert_eq!(*opened.lock().unwrap(), vec![format!("doc.open {}", proj.join("mech/b.step").display()), format!("doc.open {}", proj.join("mech/a.step").display())]);
        // a new part: a tab of its own, saved at the first free mech/part-N.sc at once
        let r = hub.request(&mut conn, &json!({"id": 30, "m": "editor.solidcraft.open", "p": {"project": root, "create": true}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert_eq!((r["ok"].as_bool(), r["file"].as_str(), r["pid"].as_u64()), (Some(true), proj.join("mech/part-1.sc").to_str(), Some(pid as u64)), "{r}");
        assert_eq!(opened.lock().unwrap()[2..], ["file.new ".to_string(), format!("file.save_as {}", proj.join("mech/part-1.sc").display())]);
        let r = hub.request(&mut conn, &json!({"id": 31, "m": "editor.solidcraft.open", "p": {"project": root, "create": true, "file": "mech/a.step"}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert_eq!(r["ok"], false, "a new design is a .sc: {r}");
        std::fs::write(proj.join("mech/part-1.sc"), "x").unwrap(); assert_eq!(next_part(&root), proj.join("mech/part-2.sc").to_string_lossy());
        let r = hub.request(&mut conn, &json!({"id": 32, "m": "editor.solidcraft.open", "p": {"project": root, "file": "mech/a.step"}})).await.into_iter().find(|f| f["t"] == "reply").unwrap(); assert_eq!(r["ok"], true);
        // detached: the window floats; brought back: in the pane again; the program is the same one throughout
        let r = hub.request(&mut conn, &json!({"id": 8, "m": "editor.solidcraft.detach", "p": {"project": root}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert_eq!((r["container"].as_u64(), r["state"].as_str(), r["pid"].as_u64()), (Some(0), Some("ready"), Some(pid as u64)));
        let r = hub.request(&mut conn, &json!({"id": 9, "m": "editor.solidcraft.bring_back", "p": {"project": root, "container": 19, "toplevel": 18}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert_eq!((r["container"].as_u64(), r["toplevel"].as_u64(), r["pid"].as_u64()), (Some(19), Some(18), Some(pid as u64)));
        // the pane leaves and keeps the window: parked; back in a pane: not parked
        let r = hub.request(&mut conn, &json!({"id": 20, "m": "editor.solidcraft.detach", "p": {"project": root, "keep": true}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert_eq!((r["parked"].as_bool(), r["state"].as_str()), (Some(true), Some("ready")), "{r}");
        assert_eq!(parked_victims(&sessions().lock().unwrap(), now_s() + 700, 600), vec![norm_root(&root)]);
        let r = hub.request(&mut conn, &json!({"id": 21, "m": "editor.solidcraft.attach", "p": {"project": root, "container": 19, "toplevel": 18}})).await.into_iter().find(|f| f["t"] == "reply").unwrap();
        assert_eq!((r["parked"].as_bool(), r["pid"].as_u64(), r["file"].as_str()), (Some(false), Some(pid as u64), proj.join("mech/a.step").to_str()), "{r}");
        // closed: stopped, the process gone, no channel left
        let r = hub.request(&mut conn, &json!({"id": 10, "m": "editor.solidcraft.close", "p": {"project": root}})).await.into_iter().find(|f| f["t"] == "reply").unwrap(); assert_eq!(r["ok"], true);
        for _ in 0..200 { if info(&hub)["state"] == "stopped" { break } tokio::time::sleep(Duration::from_millis(25)).await; }
        let i = info(&hub); assert_eq!((i["state"].as_str(), i["pid"].as_u64(), i["reason"].as_str(), i["control_port"].as_u64()), (Some("stopped"), Some(0), Some("closed"), Some(0)), "{i}");
        assert!(!crate::plat::proc::alive(pid) && control(&root).is_none());
        let r = hub.request(&mut conn, &json!({"id": 11, "m": "editor.solidcraft.bring_back", "p": {"project": root, "container": 1, "toplevel": 1}})).await.into_iter().find(|f| f["t"] == "reply").unwrap(); assert_eq!(r["ok"], false);
        // a program that dies at once: failed, with what it said
        std::fs::write(bin.join("solidcraft"), "#!/bin/sh\necho 'error: no GPU adapter' >&2\nexit 7\n").unwrap();
        let r = hub.request(&mut conn, &json!({"id": 12, "m": "editor.solidcraft.attach", "p": {"project": root, "file": "mech/a.step"}})).await.into_iter().find(|f| f["t"] == "reply").unwrap(); assert_eq!(r["gen"], 2, "{r}");
        for _ in 0..200 { if info(&hub)["state"] == "failed" { break } tokio::time::sleep(Duration::from_millis(25)).await; }
        let i = info(&hub); assert!(i["state"] == "failed" && i["reason"].as_str().unwrap().contains("code 7") && i["reason"].as_str().unwrap().contains("no GPU adapter"), "{i}");
        runtime::test_over().lock().unwrap().remove(&home);
    }
    /// A stand-in control channel: one connection, checks the token, answers the method.
    async fn stand_in(token: &'static str) -> u16 {
        let l = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap(); let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((s, _)) = l.accept().await else { return };
                tokio::spawn(async move {
                    let (r, mut w) = s.into_split(); let mut line = String::new();
                    if BufReader::new(r).read_line(&mut line).await.unwrap_or(0) == 0 { return }
                    let v: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
                    let reply = if v["token"] != token { json!({"ok": false, "error": "unauthorized: wrong token"}) }
                        else if v["method"] == "control.capabilities" { json!({"id": v["id"], "ok": true, "result": {"version": "0.50.0"}}) }
                        else if v["method"] == "engine.execute" && v["params"]["command"] == "doc.open" { json!({"id": v["id"], "ok": true, "result": {"path": v["params"]["params"]["path"]}}) }
                        else if v["method"] == "slow" { tokio::time::sleep(Duration::from_secs(30)).await; json!({}) }
                        else { json!({"id": v["id"], "ok": false, "error": "unknown method"}) };
                    let _ = w.write_all(format!("{{\"event\":\"noise\",\"params\":{{}}}}\n{reply}\n").as_bytes()).await;
                });
            }
        });
        port
    }
    #[tokio::test]
    async fn control_calls() {
        let port = stand_in("tok").await;
        assert_eq!(call(port, "tok", "control.capabilities", json!({}), Duration::from_secs(5)).await.unwrap()["version"], "0.50.0", "event lines are skipped");
        assert!(call(port, "nope", "control.capabilities", json!({}), Duration::from_secs(5)).await.unwrap_err().contains("unauthorized"));
        assert_eq!(call(port, "tok", "doc.open", json!({}), Duration::from_secs(5)).await.unwrap_err(), "unknown method");
        assert!(call(port, "tok", "slow", json!({}), Duration::from_millis(200)).await.unwrap_err().contains("did not answer slow"));
        let dead = free_port().unwrap();
        assert!(call(dead, "tok", "control.capabilities", json!({}), Duration::from_secs(2)).await.unwrap_err().contains("no SolidCraft is listening"));
        // a live session's channel is what the MCP bridge gets, and commands go through engine.execute
        let root = "/nonexistent-sc-test/control-calls";
        with(root, |s| { s.state = "ready".into(); s.pid = 1; s.port = port; s.token = "tok".into(); s.file = format!("{root}/a.step"); });
        assert_eq!(control(root), Some(Control { port, token: "tok".into(), file: format!("{root}/a.step") }));
        assert_eq!(control_for_path(&format!("{root}/mech/b.step")).map(|c| c.port), Some(port));
        assert_eq!(control_for_path("/nonexistent-other/b.step"), None);
        assert_eq!(command(root, "doc.open", json!({"path": "/x.step"}), Duration::from_secs(5)).await.unwrap()["path"], "/x.step");
        with(root, |s| { s.state = "stopped".into(); s.pid = 0; });
        assert_eq!(control(root), None);
        assert!(command(root, "doc.open", json!({}), Duration::from_secs(1)).await.unwrap_err().contains("not open"));
    }
}

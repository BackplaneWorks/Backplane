//! SolidCraft for agents: the tools of SolidCraft's own MCP server (`solidcraft-cli mcp`), served through the hub as `solidcraft_*`,
//! with a harness round every call.
//!
//! - One session per project (session.rs). When SolidCraft is open in the project's pane the session is bridged to it, so an agent's
//!   edit appears in the window as it is made; otherwise it is headless and works on the design's file, which the viewers follow.
//! - Before a call that changes the design, the design's file is copied to a checkpoint (`<home>/solidcad/<project>/`), and the turn's
//!   snapshot is taken as for any project file write. `solidcraft_checkpoint` lists, saves and restores them.
//! - A call that does not answer inside its budget (a stalled kernel) or whose server dies is not left hanging: the child is replaced,
//!   the design is reopened from its last save, and the answer says what was kept and what to check. An edit is saved as soon as it
//!   goes through, so that save is at most one call old. A call whose caller went away is reported to the next caller.
//! - After a change the answer carries a digest (bodies, volume, bounding box, failing features) and, when the model has changed since
//!   the agent last saw it, says to look: `solidcraft_look` draws it from several sides. `solidcraft_summary` gives the parameters and
//!   mass properties. The same failing call sent three times in a row is called out.
//!
//! The harness's shape (snapshot before a change, resolve a dangling call to an explicit error the model can act on, keep the worker
//! independent of any one caller, summarise parameters) follows patterns common to chat-driven CAD tools; it is written here from
//! scratch against SolidCraft's documented MCP interface (docs/SOLIDCRAFT.md).
pub mod client;
pub mod harness;
pub mod session;
#[cfg(test)]
mod tests;

use super::{mcp::ToolSet, Agents};
use crate::{api::Fut, files::kicad::canonical as kc, hub::Hub};
use harness as h;
use serde_json::{json, Value};
pub use session::Seam;
use session::Session;
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex as StdMutex, OnceLock, Weak}, time::{Duration, Instant}};

pub struct Cad {
    hub: Arc<Hub>,
    ag: Weak<Agents>,
    seam: Seam,
    sessions: StdMutex<HashMap<String, Arc<Session>>>,
    /// the upstream tools as agents are shown them, once a server has listed them
    defs: StdMutex<Option<Vec<Value>>>,
    /// which project folders get the tools (ten seconds each)
    wanted: StdMutex<HashMap<String, (Instant, bool)>>,
}

static CAD: OnceLock<Arc<Cad>> = OnceLock::new();
pub fn get() -> Option<Arc<Cad>> { CAD.get().cloned() }

/// The program on PATH, or the one `BACKPLANE_SOLIDCRAFT_CLI` names: what a hub without the managed runtime uses.
pub fn cli_on_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("BACKPLANE_SOLIDCRAFT_CLI").map(PathBuf::from).filter(|p| p.is_file()) { return Some(p) }
    let exe = if cfg!(windows) { "solidcraft-cli.exe" } else { "solidcraft-cli" };
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(exe)).find(|p| p.is_file())
}

/// Registers the tools (main.rs, once the agents run). `seam`: where the program and the pane's control channel are found.
pub fn register(hub: &Arc<Hub>, ag: &Arc<Agents>, seam: Seam) -> Arc<Cad> {
    let cad = Arc::new(Cad { hub: hub.clone(), ag: Arc::downgrade(ag), seam, sessions: Default::default(), defs: Default::default(), wanted: Default::default() });
    let _ = CAD.set(cad.clone());
    ag.add_tools(cad.clone());
    // the tool list, read once from the program when it is there (nothing is installed for it)
    let c = cad.clone(); tokio::spawn(async move { c.learn().await });
    cad
}

const CAD_EXT: &[&str] = &[".sc", ".solidcraft", ".step", ".stp", ".iges", ".igs", ".stl", ".3mf", ".obj", ".glb"];
const BOARD_EXT: &[&str] = &[".kicad_pro", ".kicad_pcb", ".kicad_sch", ".prjpcb", ".pcbdoc", ".schdoc"];
/// (CAD files, a `mech` folder, board files) in `root`, four folders deep, hidden and build folders left out.
pub fn scan(root: &Path) -> (bool, bool, bool) {
    let (mut cad, mut mech, mut board) = (false, false, false); let mut seen = 0usize;
    let mut stack = vec![(root.to_path_buf(), 0u8)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            seen += 1; if seen > 20_000 || (cad && board) { return (cad, mech, board) }
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.starts_with('.') || matches!(name.as_str(), "node_modules" | "target" | "build" | "dist") { continue }
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() { if name == "mech" { mech = true } if depth < 4 { stack.push((e.path(), depth + 1)) } }
            else if CAD_EXT.iter().any(|x| name.ends_with(x)) { cad = true }
            else if BOARD_EXT.iter().any(|x| name.ends_with(x)) { board = true }
        }
    }
    (cad, mech, board)
}

fn def(name: &str, desc: &str, props: Value, req: &[&str]) -> Value { json!({"name":name,"description":desc,"inputSchema":{"type":"object","properties":props,"required":req}}) }
/// The harness's own tools.
pub fn own_defs() -> Vec<Value> { vec![
    def("solidcraft_look", "Draw the open SolidCraft design from several sides at once (default iso, front, top, right) and return the pictures with a digest (bodies, volume, bounding box, failing features). Call it after every stage of modelling and compare with the intent before going on.",
        json!({"views":{"type":"array","items":{"type":"string"},"description":"iso, top, front, back, bottom, left, right, home, fit, or a saved view's name; at most 6"},"width":{"type":"number"},"height":{"type":"number"}}), &[]),
    def("solidcraft_summary", "The open SolidCraft design in brief: its file, the user parameters (name, expression, value, unit), the timeline's features (failing ones carry their error) and each body's mass properties (volume, area, mass, density, centre of mass, bounding box).", json!({}), &[]),
    def("solidcraft_checkpoint", "Checkpoints of the design's file, kept outside the project. One is taken by itself before every change. list shows them (newest first); save keeps the design as it is now (saving it first); restore puts one back as the design's file and reopens it (the newest without id), keeping the current state as a checkpoint of its own.",
        json!({"action":{"type":"string","enum":["list","save","restore"]},"id":{"type":"number","description":"restore: the checkpoint's id"},"note":{"type":"string","description":"save: what this state is"}}), &["action"]),
    def("solidcraft_status", "The CAD session of this project: whether SolidCraft is installed, whether the session is headless or acts on the SolidCraft window open in Backplane, the design's file, checkpoints, restarts.", json!({}), &[]),
    def("solidcraft_call", "Call a SolidCraft MCP tool by its own name with its own arguments: for tools not listed here (an installed plugin's `<plugin>.<entry>`), or when the solidcraft_* list has not loaded.",
        json!({"tool":{"type":"string","description":"the tool's upstream name, e.g. inspect_design"},"arguments":{"type":"object"}}), &["tool"]),
] }

impl Cad {
    async fn learn(&self) {
        if self.defs.lock().unwrap().is_some() { return }
        let Some(program) = (self.seam.cli)() else { return };
        let spawn = client::Spawn { program, mode: client::Mode::Headless, allow: vec![] };
        let Ok(c) = client::Client::start(&spawn, Duration::from_secs(30)).await else { return };
        if let Ok(tools) = c.tools(Duration::from_secs(30)).await { self.learned(&tools) }
        c.kill();
    }
    fn learned(&self, tools: &[Value]) { let v: Vec<Value> = tools.iter().filter_map(h::listed).collect(); if !v.is_empty() { *self.defs.lock().unwrap() = Some(v) } }

    /// The folder a thread's design files live in (the main checkout for a hardware project).
    async fn root_of(&self, thread: &str) -> Option<PathBuf> {
        let r = crate::files::root_of(&self.hub, thread).await?;
        Some(PathBuf::from(kc::design_root(&r.to_string_lossy())))
    }
    fn session(&self, root: &Path) -> Arc<Session> {
        let key = root.to_string_lossy().into_owned();
        self.sessions.lock().unwrap().entry(key.clone()).or_insert_with(|| Arc::new(Session::new(root.to_path_buf(), self.hub.cfg.home.join("solidcad").join(crate::files::kicad::kserve::digest(&key))))).clone()
    }

    /// Whether this thread's project gets the tools up front (harness::wanted).
    pub async fn wanted(&self, thread: &str) -> bool {
        let Some(root) = self.root_of(thread).await else { return false };
        let key = root.to_string_lossy().into_owned();
        if (self.seam.control)(&key).is_some() { return true }
        if let Some((at, v)) = self.wanted.lock().unwrap().get(&key) { if at.elapsed() < Duration::from_secs(10) { return *v } }
        let r2 = root.clone();
        let (cad, mech, board) = tokio::task::spawn_blocking(move || scan(&r2)).await.unwrap_or((false, false, true));
        let v = h::wanted(cad, mech, board, false);
        self.wanted.lock().unwrap().insert(key, (Instant::now(), v)); v
    }
    /// tools/list for one thread: a project with nothing mechanical keeps only the two small entry tools.
    pub async fn filter(&self, thread: &str, tools: &mut Vec<Value>) {
        if self.wanted(thread).await { super::mcp::always_load(tools, "solidcraft_look"); return }
        tools.retain(|t| { let n = t.get("name").and_then(Value::as_str).unwrap_or(""); !n.starts_with(h::PREFIX) || matches!(n, "solidcraft_call" | "solidcraft_status") });
    }

    /// Ends every session's child (the hub stops).
    pub async fn stop(&self) { let all: Vec<Arc<Session>> = self.sessions.lock().unwrap().values().cloned().collect(); for s in all { s.stop().await } }

    async fn run(&self, thread: &str, name: &str, a: &Value) -> (bool, Value) {
        let Some(root) = self.root_of(thread).await else { return (false, json!({"error":"this thread has no project folder: SolidCraft works on a project's files"})) };
        let s = self.session(&root);
        let (tool, args) = match name {
            "solidcraft_status" => { let mut v = s.status(&self.seam).await; let cli = (self.seam.cli)(); v["installed"] = json!(cli.is_some()); if let Some(p) = cli { v["cli"] = json!(p.to_string_lossy()) } return (true, v) }
            "solidcraft_checkpoint" => return s.checkpoint(&self.seam, a).await,
            "solidcraft_summary" => return s.summary(&self.seam).await,
            "solidcraft_look" => return s.look(&self.seam, a).await,
            "solidcraft_call" => { let t = a.get("tool").and_then(Value::as_str).unwrap_or("").trim_start_matches(h::PREFIX).to_string(); if t.is_empty() { return (false, json!({"error":"tool is required"})) } (t, a.get("arguments").cloned().unwrap_or(json!({}))) }
            n => match h::upstream(n) { Some(t) => (t.to_string(), a.clone()), None => return (false, json!({"error":"unknown tool"})) },
        };
        if h::HIDDEN.contains(&tool.as_str()) { return (false, json!({"error": format!("{tool} acts on the person's SolidCraft app, not the design: it is not offered here")})) }
        if self.defs.lock().unwrap().is_none() { if let Ok(t) = s.tools(&self.seam).await { self.learned(&t) } }
        let (ag, th) = (self.ag.clone(), thread.to_string());
        let guard = move |paths: Vec<String>| -> Fut<'static, Result<(), String>> { let (ag, th) = (ag.clone(), th.clone()); Box::pin(async move {
            let Some(ag) = ag.upgrade() else { return Ok(()) };
            let ps: Vec<&str> = paths.iter().map(String::as_str).collect(); ag.files_write_guard(&th, &ps).await }) };
        s.call(&self.seam, &tool, &args, &guard).await
    }
}

impl ToolSet for Cad {
    fn list(&self, _thread: &str) -> Vec<Value> { let mut v = self.defs.lock().unwrap().clone().unwrap_or_default(); v.extend(own_defs()); v }
    fn owns(&self, name: &str) -> bool { name.starts_with(h::PREFIX) }
    fn call<'a>(&'a self, thread: &'a str, name: &'a str, a: &'a Value) -> Fut<'a, (bool, Value)> { Box::pin(self.run(thread, name, a)) }
}

//! Embedded GUI KiCad per hardware project: process lifecycle, `editor.kicad.*` RPCs, `editor.kicad.<digest>` info,
//! and live `kicad_ipc` routing to the open GUI's API socket.
//!
//! Boundary with the client (`native/gpui/src/viewers/{kicad_pane,native_child}.rs`):
//!   - The hub performs NO X11 / Win32 window operations (it may run on a headless machine with no DISPLAY).
//!   - In `stock` mode (upstream KiCad 10.0.x, or an older runtime without `--embedded`), the hub seeds an isolated
//!     `XDG_CONFIG_HOME` (`api.enable_server: true`, wizards off, side docks hidden), launches `kicad <pro>`
//!     with a short private `TMPDIR` so the GUI binds `<tmp>/kicad/api.sock`, publishes `pid` at once in `"starting"`,
//!     polls `sockets_at` until `Ping` + `GetVersion` answer, and drives frame switches via `RunAction`.
//!   - In `fork` mode (Backplane KiCad bp.8+, `BACKPLANE_EMBED.md`), the hub launches
//!     `kicad --embedded --parent-window <container> --toplevel-window <toplevel> --api-socket <sock> --parent-pid <hub> <pro>`,
//!     waits for `"KiCad API server listening at ipc://<sock>"`, reads `GetFrames` (`holder` + `events_socket`), shows the
//!     requested frame (`ShowFrame`), and keeps one `NextEvents` long-poll in flight on `<sock>.events`.
//!   - Before spawning GUI KiCad for a project, any headless `kserve` for that project is retired cleanly (`ipc::retire`).
//!     While the GUI session is `"ready"`, `ipc::run` routes that project's `kicad_ipc` (including GUI-only commands such as
//!     `GetSelection` and `RunAction`) to the GUI socket; modal-dialog `AS_BUSY` errors pass straight back to the caller.
use super::{ipc::{self, Env}, kipc, kserve as ks, open, project::{self, Files}, proto};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::{HashMap, VecDeque}, path::{Path, PathBuf}, process::Stdio, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex, OnceLock}, time::Duration};
use tokio::{io::{AsyncBufReadExt, AsyncRead, BufReader}, sync::{mpsc, oneshot}};

pub const MAX_WARM: usize = 3;
const EVENTS_LINE: &str = "KiCad embed events at ";

// Pure state & rules --------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub kind: String,
    pub doc: String,
    pub title: String,
    pub modified: bool,
    pub shown: bool,
    /// bp.9: the client may map and unmap this frame itself (shown once, or pre-warmed)
    pub realized: bool,
    pub xid: u64,
    pub min_w: i32,
    pub min_h: i32,
}

impl Frame {
    pub fn to_json(&self) -> Value {
        json!({
            "kind": self.kind, "doc": self.doc, "title": self.title,
            "modified": self.modified, "shown": self.shown, "realized": self.realized,
            "xid": self.xid, "min_w": self.min_w, "min_h": self.min_h,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub pro: String,
    pub root: String,
    pub mode: String,      // "stock" | "fork"
    pub state: String,     // "starting" | "ready" | "unresponsive" | "failed" | "stopped"
    pub reason: String,
    pub pid: u32,
    pub gen: u64,
    pub active: String,    // "PCB" | "SCH" | "PCB_3D" | "SYMBOL_EDITOR" | "FOOTPRINT_EDITOR" | "OTHER"
    pub container: u64,
    pub toplevel: u64,
    pub holder: u64,
    pub display: String,
    pub sock: String,
    pub events_sock: String,
    pub sock_dir: String,
    pub argv: String,
    pub cli: String,
    pub version: String,
    pub desc: String,
    pub caps: Vec<String>,
    pub frames: Vec<Frame>,
    pub missed_pings: u32,
    pub touched_at: u64,
    pub closing: bool,
}

impl Session {
    pub fn new(pro: &str, root: &str, mode: &str, active: &str, gen: u64, now: u64) -> Self {
        Session {
            pro: pro.into(), root: root.into(), mode: mode.into(), state: "starting".into(),
            reason: String::new(), pid: 0, gen, active: canon_frame(active).into(),
            container: 0, toplevel: 0, holder: 0, display: String::new(),
            sock: String::new(), events_sock: String::new(), sock_dir: String::new(),
            argv: String::new(), cli: String::new(), version: String::new(), desc: String::new(),
            caps: vec![], frames: vec![], missed_pings: 0, touched_at: now, closing: false,
        }
    }
    pub fn spawned(mut self, pid: u32) -> Self { self.pid = pid; self }
    pub fn ready(mut self, sock: &str, version: &str, now: u64) -> Self {
        self.state = "ready".into(); self.reason.clear(); self.sock = sock.into();
        if !version.is_empty() { self.version = version.into(); }
        self.missed_pings = 0; self.touched_at = now; self
    }
    pub fn ping_result(mut self, ok: bool) -> Self {
        if !matches!(self.state.as_str(), "ready" | "unresponsive") { return self }
        if ok {
            self.missed_pings = 0;
            if self.state == "unresponsive" { self.state = "ready".into(); self.reason.clear(); }
        } else {
            self.missed_pings = self.missed_pings.saturating_add(1);
            if self.missed_pings >= 2 {
                self.state = "unresponsive".into();
                self.reason = "KiCad is not responding to API pings".into();
            }
        }
        self
    }
    pub fn detached(mut self, now: u64) -> Self {
        self.container = 0; self.touched_at = now; self
    }
    pub fn failed(mut self, why: &str) -> Self {
        self.state = "failed".into(); self.reason = why.into(); self.pid = 0; self.holder = 0;
        self.frames.clear(); self
    }
    pub fn stopped(mut self, reason: &str) -> Self {
        self.state = "stopped".into(); self.reason = reason.into(); self.pid = 0; self.holder = 0;
        self.frames.clear(); self
    }
    pub fn exited(self, code: i32, tail: &str) -> Self {
        if self.closing { return self.stopped("") }
        // KiCad quit by itself (its window closed, its own File > Exit): said, so the pane and a later request never read "stopped ()"
        if code == 0 { return self.stopped("KiCad was closed") }
        let detail = tail.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        let why = if detail.is_empty() { format!("KiCad exited (code {code})") } else { format!("KiCad exited (code {code}): {detail}") };
        self.failed(&why)
    }
    pub fn unsaved_docs(&self) -> Vec<String> {
        self.frames.iter().filter(|f| f.modified).map(|f| if !f.doc.is_empty() { f.doc.clone() } else { f.title.clone() }).collect()
    }
    pub fn is_live(&self) -> bool {
        matches!(self.state.as_str(), "starting" | "ready" | "unresponsive")
    }
    pub fn info_json(&self) -> Value {
        json!({
            "project": self.pro,
            "root": self.root,
            "state": self.state,
            "mode": self.mode,
            "pid": self.pid,
            "reason": self.reason,
            "active": self.active,
            "container": self.container,
            "toplevel": self.toplevel,
            "holder": self.holder,
            "sock": self.sock,
            "display": self.display,
            "frames": self.frames.iter().map(Frame::to_json).collect::<Vec<_>>(),
            // bp.9: the client may map and unmap a frame KiCad has shown once (its tab switches need no round trip)
            "local_map": self.mode == "fork" && !self.desc.is_empty() && proto::load(&self.desc).has_field("kiapi.backplane.embed.ShowFrame", "prewarm"),
        })
    }
}

/// Stable info key `editor.kicad.<12-hex-sha256>` for a canonical `.kicad_pro` path.
pub fn info_key(pro: &str) -> String { format!("editor.kicad.{}", ks::digest(pro)) }

/// Canonical uppercase frame kind shared with `native/gpui/src/viewers/kicad_pane.rs`.
pub fn canon_frame(raw: &str) -> &'static str {
    match raw.trim().to_ascii_uppercase().as_str() {
        "" | "0" | "PCB" | "FK_PCB" | "BOARD" => "PCB",
        "1" | "SCH" | "FK_SCH" | "SCHEMATIC" => "SCH",
        "2" | "PCB_3D" | "FK_PCB_3D" | "3D" | "3D_VIEWER" => "PCB_3D",
        "SYMBOL_EDITOR" | "FK_SYMBOL_EDITOR" | "SYM" => "SYMBOL_EDITOR",
        "FOOTPRINT_EDITOR" | "FK_FOOTPRINT_EDITOR" | "FP" => "FOOTPRINT_EDITOR",
        _ => "OTHER",
    }
}

/// Default frame kind from the path the client attached with.
pub fn default_frame_for_path(path: &str, requested: &str) -> &'static str {
    if !requested.trim().is_empty() { return canon_frame(requested) }
    if path.ends_with(".kicad_sch") { "SCH" } else { "PCB" }
}

/// `kiapi.backplane.embed.FrameKind` enum name for a canonical frame string.
/// The editors pre-warmed after the first one is shown: the board and the schematic, less the one shown and those KiCad has open. Never
/// the 3D viewer: its first paint builds the board's models on KiCad's UI thread (~0.9 s on a small board), a hitch wherever it falls, so
/// it is paid only when the person asks for 3D.
pub fn prewarm_kinds(active: &str, have: &[String]) -> Vec<&'static str> {
    ["SCH", "PCB"].into_iter().filter(|k| *k != canon_frame(active) && !have.iter().any(|h| h == k)).collect()
}

pub fn fk_of(kind: &str) -> &'static str {
    match canon_frame(kind) {
        "PCB" => "FK_PCB",
        "SCH" => "FK_SCH",
        "SYMBOL_EDITOR" => "FK_SYMBOL_EDITOR",
        "FOOTPRINT_EDITOR" => "FK_FOOTPRINT_EDITOR",
        "PCB_3D" => "FK_PCB_3D",
        _ => "FK_OTHER",
    }
}

/// Canonical frame string from `FrameInfo.kind` (either enum name or integer value).
pub fn kind_of_fk_val(v: &Value) -> &'static str {
    if let Some(s) = v.as_str() { return canon_frame(s) }
    match v.as_i64().unwrap_or(0) {
        1 => "PCB",
        2 => "SCH",
        3 => "SYMBOL_EDITOR",
        4 => "FOOTPRINT_EDITOR",
        5 => "PCB_3D",
        _ => "OTHER",
    }
}

/// `CloseMode` enum name for `editor.kicad.close`'s `save` parameter (`"ask"` | `"save"` | `"discard"`).
pub fn close_mode_of(save: &str) -> &'static str {
    match save.trim().to_ascii_lowercase().as_str() {
        "save" => "CM_SAVE",
        "discard" => "CM_DISCARD",
        _ => "CM_FAIL_IF_MODIFIED",
    }
}

/// Stock KiCad 10.0 registers `RunAction` on `API_HANDLER_PCB` only; these actions open sibling frames in the same `Kiway`.
pub fn stock_action_for(kind: &str) -> Option<&'static str> {
    match canon_frame(kind) {
        "PCB" => None,
        "SCH" => Some("pcbnew.EditorControl.showEeschema"),
        "PCB_3D" => Some("common.Control.show3DViewer"),
        "SYMBOL_EDITOR" => Some("common.Control.showSymbolEditor"),
        "FOOTPRINT_EDITOR" => Some("common.Control.showFootprintEditor"),
        _ => None,
    }
}

/// Pure mode decision (`"fork"` vs `"stock"`).
/// `override_mode`: `BACKPLANE_KICAD_EMBED_MODE` if set (`"fork"` | `"stock"`).
/// `has_embed_schema`: whether the runtime's `api.desc` defines `kiapi.backplane.embed.GetFrames`.
/// `described`: parsed JSON of `kicad-cli api-server --describe` if it ran and succeeded.
pub fn mode_with(override_mode: Option<&str>, env: &Env, has_embed_schema: bool, described: Option<&Value>) -> &'static str {
    if let Some(m) = override_mode {
        match m.trim().to_ascii_lowercase().as_str() {
            "fork" => return "fork",
            "stock" => return "stock",
            _ => {}
        }
    }
    if has_embed_schema { return "fork" }
    if env.caps.iter().any(|c| c == "embed" || c == "embedded") { return "fork" }
    if env.version.contains("-bp.") && ks::version_ge(&env.version, "10.99.0-bp.8") { return "fork" }
    if let Some(d) = described {
        if d["capabilities"].as_array().is_some_and(|a| a.iter().any(|x| matches!(x.as_str(), Some("embed" | "embedded")))) {
            return "fork";
        }
        if let Some(v) = d["version"].as_str() {
            if v.contains("-bp.") && ks::version_ge(v, "10.99.0-bp.8") { return "fork" }
        }
    }
    "stock"
}

/// Command line for stock mode: `kicad --frame pcb <pcb>` when a board exists so `API_HANDLER_PCB` serves `RunAction`
/// while `Kiway` stays project-suite (`IsSingle() == false`), else `--frame sch <sch>`, else `<pro>`.
/// Stock KiCad 10.0: the project manager on the project (`kicad --frame pcb <board>` segfaults in 10.0.7 on a fresh config, exit 139);
/// the client opens the editors with KiCad's own shortcuts (viewers/native_child.rs `stock_opener`).
pub fn stock_argv(kicad_bin: &str, files: &Files) -> Vec<String> { vec![kicad_bin.into(), files.pro.clone()] }

/// Command line for fork mode (`BACKPLANE_EMBED.md`).
pub fn fork_argv(kicad_bin: &str, pro: &str, sock: &str, container: u64, toplevel: u64, hub_pid: u32) -> Vec<String> {
    let mut v = vec![kicad_bin.to_string(), "--embedded".into()];
    if container > 0 { v.push("--parent-window".into()); v.push(container.to_string()); }
    if toplevel > 0 { v.push("--toplevel-window".into()); v.push(toplevel.to_string()); }
    v.extend(["--api-socket".into(), sock.to_string(), "--parent-pid".into(), hub_pid.to_string(), pro.to_string()]);
    v
}

/// Least-recently-used detached clean session to evict before spawning a new one when live sessions >= `max_warm`.
pub fn evict_warm(sessions: &HashMap<String, Session>, keep_pro: &str, max_warm: usize) -> Option<String> {
    let live = sessions.values().filter(|s| s.pro != keep_pro && s.is_live()).count();
    if live < max_warm { return None }
    sessions.values()
        .filter(|s| s.pro != keep_pro && s.state == "ready" && s.container == 0 && s.unsaved_docs().is_empty())
        .min_by_key(|s| s.touched_at)
        .map(|s| s.pro.clone())
}

fn u64_of(v: &Value) -> u64 {
    v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok())).unwrap_or(0)
}

/// On Win32, `bp.9`'s `KIPLATFORM::EMBED::IsMapped` calls `IsWindowVisible(hwnd)`, which returns `false` whenever the host's
/// `container` HWND is not yet `WS_VISIBLE` (e.g. cold startup before `KicadPane` reveals the container, or while detached).
/// Read the child frame's own `WS_VISIBLE` style bit directly (excluding off-screen `Prewarm` frames at `(-30000, -30000)`).
#[cfg(windows)]
fn win32_frame_mapped(xid: u64) -> bool {
    if xid == 0 { return false }
    use std::ffi::c_void;
    type HWND = *mut c_void;
    #[repr(C)] #[derive(Default)] struct RECT { left: i32, top: i32, right: i32, bottom: i32 }
    #[repr(C)] #[derive(Default)] struct POINT { x: i32, y: i32 }
    const GWL_STYLE: i32 = -16;
    const GA_PARENT: u32 = 1;
    const WS_VISIBLE: isize = 0x1000_0000;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn IsWindow(h: HWND) -> i32;
        fn GetWindowLongPtrW(h: HWND, idx: i32) -> isize;
        fn GetWindowRect(h: HWND, r: *mut RECT) -> i32;
        fn GetAncestor(h: HWND, flags: u32) -> HWND;
        fn ScreenToClient(h: HWND, pt: *mut POINT) -> i32;
    }
    unsafe {
        let h = xid as usize as HWND;
        if h.is_null() || IsWindow(h) == 0 { return false }
        if (GetWindowLongPtrW(h, GWL_STYLE) & WS_VISIBLE) == 0 { return false }
        let mut rc = RECT::default();
        if GetWindowRect(h, &mut rc) != 0 {
            let mut pt = POINT { x: rc.left, y: rc.top };
            let parent = GetAncestor(h, GA_PARENT);
            if !parent.is_null() && ScreenToClient(parent, &mut pt) != 0 && (pt.x <= -10000 || pt.y <= -10000) {
                return false;
            }
        }
        true
    }
}
#[cfg(not(windows))]
fn win32_frame_mapped(_xid: u64) -> bool { false }

pub fn parse_frame_info(v: &Value) -> Frame {
    let xid = u64_of(&v["window"]);
    Frame {
        kind: kind_of_fk_val(&v["kind"]).into(),
        doc: v["document"].as_str().unwrap_or("").into(),
        title: v["title"].as_str().unwrap_or("").into(),
        modified: v["modified"].as_bool().unwrap_or(false),
        shown: v["shown"].as_bool().unwrap_or(false) || win32_frame_mapped(xid),
        realized: v["realized"].as_bool().unwrap_or(false),
        xid,
        min_w: v["min_width"].as_i64().unwrap_or(0) as i32,
        min_h: v["min_height"].as_i64().unwrap_or(0) as i32,
    }
}

pub fn parse_get_frames(msg: &Value) -> (u64, String, Vec<Frame>) {
    let holder = u64_of(&msg["holder"]);
    let ev = msg["events_socket"].as_str().unwrap_or("");
    let events_sock = ev.strip_prefix("ipc://").unwrap_or(ev).to_string();
    let frames = msg["frames"].as_array()
        .map(|a| a.iter().map(parse_frame_info).collect())
        .unwrap_or_default();
    (holder, events_sock, frames)
}

/// Seed an isolated `XDG_CONFIG_HOME` for stock KiCad so the IPC API server is enabled, first-run / update wizards
/// stay silent, GAL (`canvas_type: 0`) is selected, and side AUI docks start collapsed for small embedded viewports.
pub fn seed_stock_config(cfg_home: &Path) -> Result<(), String> {
    let common = json!({
        "meta": { "filename": "kicad_common.json", "version": 6 },
        "api": { "enable_server": true, "interpreter_path": "" },
        "auto_backup": { "enabled": false },
        "do_not_show_again": {
            "data_collection_prompt": true,
            "env_var_overwrite_warning": true,
            "update_check_prompt": true
        },
        "environment": { "vars": null },
        "system": {
            "check_for_updates": false,
            "first_run_shown": true,
            "max_undo_items": 0,
            "editor_name": ""
        }
    });
    let editor = |filename: &str, version: u32| json!({
        "meta": { "filename": filename, "version": version },
        "appearance": { "canvas_type": 0 },
        "aui": {
            "design_blocks_panel_docked_width": 120,
            "show_design_blocks_panel": false,
            "show_layer_manager": false,
            "show_net_inspector": false,
            "show_properties_panel": false,
            "show_search_panel": false,
            "hierarchy_panel_docked_width": 120,
            "show_hierarchy_panel": false
        },
        "window": {
            "maximized": false,
            "pos_x": 0, "pos_y": 0,
            "size_x": 900, "size_y": 650
        }
    });
    for ver in ["10.0", "10.99"] {
        let dir = cfg_home.join("kicad").join(ver);
        std::fs::create_dir_all(&dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
        let write = |name: &str, body: String| -> Result<(), String> {
            let p = dir.join(name);
            if !p.exists() { std::fs::write(&p, body).map_err(|e| format!("could not write {}: {e}", p.display()))?; }
            Ok(())
        };
        write("kicad_common.json", serde_json::to_string_pretty(&common).unwrap_or_default())?;
        for (name, v) in [("pcbnew.json", 5), ("fpedit.json", 5), ("eeschema.json", 3), ("symbol_editor.json", 1), ("3d_viewer.json", 4)] {
            write(name, serde_json::to_string_pretty(&editor(name, v)).unwrap_or_default())?;
        }
        for (tbl, empty) in [
            ("sym-lib-table", "(sym_lib_table\n  (version 7)\n)\n"),
            ("fp-lib-table", "(fp_lib_table\n  (version 7)\n)\n"),
            ("design-block-lib-table", "(design_block_lib_table\n  (version 7)\n)\n"),
        ] {
            let dst = dir.join(tbl);
            if !dst.exists() {
                let tpl = Path::new("/usr/share/kicad/template").join(tbl);
                let text = std::fs::read_to_string(&tpl).unwrap_or_else(|_| empty.to_string());
                std::fs::write(&dst, text).map_err(|e| format!("could not write {}: {e}", dst.display()))?;
            }
        }
    }
    Ok(())
}

// Live GUI route table for kicad/ipc.rs -------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct GuiRoute {
    pub pro: String,
    pub mode: String,
    pub sock: String,
    pub desc: String,
    pub cli: String,
    pub argv: String,
    pub version: String,
    pub caps: Vec<String>,
    tx: mpsc::UnboundedSender<Msg>,
}

fn norm_pro_key(pro: &str) -> String {
    let mut n = pro.replace('\\', "/");
    while n.len() > 1 && n.ends_with('/') && !(n.len() == 3 && n.as_bytes()[1] == b':') {
        n.pop();
    }
    if n.len() >= 2 && n.as_bytes()[0].is_ascii_alphabetic() && n.as_bytes()[1] == b':' {
        n.make_ascii_lowercase();
    }
    n
}

static ROUTES: OnceLock<Mutex<HashMap<String, GuiRoute>>> = OnceLock::new();
fn routes() -> &'static Mutex<HashMap<String, GuiRoute>> { ROUTES.get_or_init(Default::default) }

/// Active `"ready"` GUI session route for `pro`, if any (`ipc::run` checks this before headless `kserve`).
pub fn gui_route(pro: &str) -> Option<GuiRoute> {
    routes().lock().unwrap().get(&norm_pro_key(pro)).cloned()
}

// The person's unsaved edits (the person's rule, Nov 11): an agent's save never writes what the person has open and unsaved. Per project:
// whether a document is modified (from each publish: GetFrames in fork mode, the client's titles in stock mode), whether the agent's own
// commits changed it since the last save, and whether the person had the editor (the keyboard on a click, from the client) or had it
// modified before the agent's commit began. All three clear when nothing is modified any more (a save, a revert).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Unsaved { pub modified: bool, pub agent: bool, pub person: bool }
impl Unsaved {
    /// What a publish says: nothing modified clears everything.
    pub fn seen(self, modified: bool) -> Self { if modified { Unsaved { modified, ..self } } else { Unsaved::default() } }
    /// The person took the editor (a click gave KiCad the keyboard).
    pub fn touched(self) -> Self { Unsaved { person: true, ..self } }
    /// An agent's BeginCommit: edits already pending that are not the agent's are the person's.
    pub fn agent_begins(self) -> Self { Unsaved { person: self.person || (self.modified && !self.agent), ..self } }
    /// An agent's commit landed.
    pub fn agent_committed(self) -> Self { Unsaved { agent: true, modified: true, ..self } }
    /// Why an agent's save is refused ("" when it may save): the document is modified and the person may have edits in it.
    pub fn refuse_save(self, file: &str) -> String {
        if self.modified && self.person { format!("You have unsaved edits in KiCad ({file}). The agent's save was refused so that it does not write them: save or revert your edits in KiCad first, then ask again.") } else { String::new() }
    }
}
static UNSAVED: OnceLock<Mutex<HashMap<String, Unsaved>>> = OnceLock::new();
fn unsaved() -> &'static Mutex<HashMap<String, Unsaved>> { UNSAVED.get_or_init(Default::default) }
/// The project's state, changed by `f`; answers the new state.
pub fn unsaved_update(pro: &str, f: impl FnOnce(Unsaved) -> Unsaved) -> Unsaved {
    let k = norm_pro_key(pro);
    let mut m = unsaved().lock().unwrap(); let n = f(m.get(&k).copied().unwrap_or_default()); m.insert(k, n); n
}

// Backplane's theme per project ("light" | "dark"), as the client last said it (attach's `theme`, `editor.kicad.theme`): the fork's
// `--theme` at launch and SetTheme while it runs. Stock KiCad keeps its own.
static THEMES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
fn themes() -> &'static Mutex<HashMap<String, String>> { THEMES.get_or_init(Default::default) }
pub fn theme_of(pro: &str) -> String { themes().lock().unwrap().get(&norm_pro_key(pro)).cloned().unwrap_or_default() }
/// The fork's ThemeMode for Backplane's theme ("" for none).
pub fn theme_mode(theme: &str) -> &'static str { match theme { "light" => "TM_LIGHT", "dark" => "TM_DARK", _ => "" } }

/// `<runtime>/backplane/kicad/<digest>/live.sock`: the open co-editing KiCad's API socket, beside the headless server's `api.sock`, so
/// an agent's own scripts (KiStack's layout tools) edit the live document instead of a file under the person (docs/kicad-coedit.md).
/// Removed when the editor is not ready. Only under $XDG_RUNTIME_DIR (the headless sockets' own place on a desktop).
fn publish_live_sock(pro: &str, sock: &str) {
    let rt = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    if rt.is_empty() || pro.is_empty() { return }
    let dir = std::path::PathBuf::from(ks::dir(&rt, "", pro));
    let link = dir.join("live.sock");
    let _ = std::fs::remove_file(&link);
    if sock.is_empty() { return }
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(unix)]
    { let _ = std::os::unix::fs::symlink(sock, &link); }
}

fn put_route(s: &Session, tx: &mpsc::UnboundedSender<Msg>) {
    unsaved_update(&s.pro, |u| u.seen(s.frames.iter().any(|f| f.modified)));
    let k = norm_pro_key(&s.pro);
    let mut r = routes().lock().unwrap();
    publish_live_sock(&s.pro, if s.state == "ready" && s.caps.iter().any(|c| c == "coedit") { &s.sock } else { "" });
    if s.state == "ready" && !s.sock.is_empty() {
        r.insert(k, GuiRoute {
            pro: s.pro.clone(), mode: s.mode.clone(), sock: s.sock.clone(), desc: s.desc.clone(),
            cli: s.cli.clone(), argv: s.argv.clone(), version: s.version.clone(), caps: s.caps.clone(),
            tx: tx.clone(),
        });
    } else {
        r.remove(&k);
    }
}

/// Forward one IPC command through the embed actor's shared `kipc::Native` transport so GUI requests and agent `kicad_ipc`
/// requests serialize cleanly on one socket connection and share its `kicad_token`.
pub async fn gui_call(route: &GuiRoute, ty: &str, msg: Value) -> Value { gui_call_as(route, ty, msg, "").await }
/// The same, as `client` (an agent's actor, `agent:<thread>`): the fork's co-editing attributes, groups and undoes its changes by it.
pub async fn gui_call_as(route: &GuiRoute, ty: &str, msg: Value, client: &str) -> Value {
    let (t, r) = oneshot::channel();
    if route.tx.send(Msg::Raw { sock: route.sock.clone(), desc: route.desc.clone(), ty: ty.into(), msg, timeout: 30000, client: client.into(), reply: t }).is_err() {
        return json!({"ok":false,"error":"no reply","why":"no reply","fallback":true})
    }
    r.await.unwrap_or_else(|_| json!({"ok":false,"error":"no reply","why":"no reply","fallback":true}))
}

/// The pane of `pro` switched to `frame` from the hub (xprobe.rs: `editor.kicad.select {show}`), as its own rail does; in bp.8 fork mode a
/// non-empty `item` is a reference KiCad cross-probes there (ShowFrame's item).
pub async fn show_for(pro: &str, frame: &str, item: &str) -> Result<Value, String> {
    let Some(r) = gui_route(pro) else { return Err("KiCad is not open for this project".into()) };
    let (tx, rx) = oneshot::channel();
    r.tx.send(Msg::Show { client: 0, pro: r.pro, frame: frame.into(), doc: String::new(), item: item.into(), focus: false, remount: false, reply: tx })
        .map_err(|_| "the embedded KiCad actor stopped".to_string())?;
    rx.await.unwrap_or_else(|_| Err("no reply from embedded KiCad actor".into()))
}

// Runtime / schema discovery ------------------------------------------------------------------------------------------------

fn live_pids() -> &'static Mutex<std::collections::HashSet<u32>> {
    static P: OnceLock<Mutex<std::collections::HashSet<u32>>> = OnceLock::new();
    P.get_or_init(Default::default)
}

/// End every KiCad session this hub spawned when the hub itself exits on a signal.
pub fn shutdown_all() {
    let pids: Vec<u32> = live_pids().lock().map_or_else(|e| e.into_inner().drain().collect(), |mut g| g.drain().collect());
    for pid in pids {
        crate::agents::proc::unwatch_group(pid);
        crate::plat::proc::kill_group(pid, crate::plat::proc::TERM);
        crate::plat::proc::kill_group(pid, crate::plat::proc::KILL);
        crate::plat::proc::kill(pid, crate::plat::proc::KILL);
    }
}

#[derive(Clone, Debug)]
struct ResolvedProject {
    pro: String,
    root: String,
    files: Files,
    frame: String,
}

/// The canonical .kicad_pro for a project path (a .kicad_pro, .kicad_pcb, .kicad_sch or the project's folder).
pub fn resolve_pro(raw_path: &str) -> Result<String, String> { resolve_project(raw_path, "").map(|r| r.pro) }

fn resolve_project(raw_path: &str, req_frame: &str) -> Result<ResolvedProject, String> {
    if raw_path.trim().is_empty() { return Err("project is required".into()) }
    if !crate::plat::fs::is_abs(raw_path) {
        return Err("project must be an absolute path (.kicad_pro, .kicad_pcb, .kicad_sch, or project folder)".into())
    }
    let routed = super::canonical::route(raw_path);
    let frame = default_frame_for_path(&routed, req_frame).to_string();
    let p = Path::new(&routed);
    let files = if p.is_dir() {
        project::read(&super::canonical::design_root(&routed))
    } else if routed.ends_with(".kicad_pro") {
        let dir = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
        let mut f = project::read(&dir);
        if f.pro.is_empty() && p.is_file() {
            let pcb = p.with_extension("kicad_pcb"); let sch = p.with_extension("kicad_sch");
            f = Files {
                pro: routed.clone(),
                pcb: if pcb.is_file() { pcb.to_string_lossy().into() } else { String::new() },
                sch: if sch.is_file() { sch.to_string_lossy().into() } else { String::new() },
                all: vec![],
            };
        }
        f
    } else {
        let dir = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
        let mut f = project::read(&dir);
        if routed.ends_with(".kicad_pcb") && f.pcb.is_empty() && p.is_file() { f.pcb = routed.clone(); }
        if routed.ends_with(".kicad_sch") && f.sch.is_empty() && p.is_file() { f.sch = routed.clone(); }
        f
    };
    if files.pro.is_empty() {
        return Err(format!("no KiCad project (.kicad_pro) found for {raw_path}"))
    }
    let root = Path::new(&files.pro).parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
    Ok(ResolvedProject { pro: files.pro.clone(), root, files, frame })
}

/// Locate the best protobuf descriptor set (`api.desc`): the active runtime's own `share/backplane/api.desc` (or CMake
/// build's `../api/api.desc`), else `BACKPLANE_API_DESC`, else an installed Backplane KiCad's `api.desc` so stock GUI
/// sessions still decode full `kiapi.board.*` / `kiapi.schematic.*` messages for agents!
fn discover_desc(home: &Path, env: &Env) -> (String, bool) {
    let mut own = vec![];
    if !env.root.is_empty() {
        own.push(PathBuf::from(&env.root).join("share/backplane/api.desc"));
        own.push(PathBuf::from(&env.root).join("Contents/Resources/backplane/api.desc"));
        if let Some(parent) = Path::new(&env.root).parent() {
            own.push(parent.join("api/api.desc"));
        }
    }
    for p in &own {
        if p.is_file() {
            let s = p.to_string_lossy().to_string();
            let has_embed = proto::load(&s).has_own("kiapi.backplane.embed.GetFrames");
            return (s, has_embed);
        }
    }
    if let Ok(p) = std::env::var("BACKPLANE_API_DESC") {
        if !p.is_empty() && Path::new(&p).is_file() { return (p, false) }
    }
    if std::env::var("BACKPLANE_KICAD_NO_SYSTEM").map_or(true, |v| v != "1") {
        let user_home = crate::plat::dirs::home_or_empty();
        let data = std::env::var("XDG_DATA_HOME").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("{user_home}/.local/share"));
        for base in [PathBuf::from(&data).join("backplane/kicad"), super::install::parent(home), home.join("kicad")] { // never 0.14's ~/.backplane-bend/kicad: a stale fork
            if let Ok(rd) = std::fs::read_dir(&base) {
                let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
                dirs.sort(); dirs.reverse();
                for d in dirs {
                    let cand = d.join("share/backplane/api.desc");
                    if cand.is_file() { return (cand.to_string_lossy().into(), false) }
                }
            }
        }
    }
    (String::new(), false)
}

fn describe_cli(cli: &str) -> Option<Value> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Value>>>> = OnceLock::new();
    if cli.is_empty() { return None }
    if let Some(cached) = CACHE.get_or_init(Default::default).lock().unwrap().get(cli).cloned() {
        return cached;
    }
    let out = crate::proc::std_cmd(crate::proc::Kind::Local, cli)
        .args(["api-server", "--describe"])
        .stdin(Stdio::null()).stderr(Stdio::null()).output().ok();
    let parsed = out.filter(|o| o.status.success())
        .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok());
    CACHE.get_or_init(Default::default).lock().unwrap().insert(cli.to_string(), parsed.clone());
    parsed
}

/// Short directory for `api.sock` / `<tmp>/kicad/api.sock` (< 65 chars base so socket path fits `SUN_LEN`).
fn short_sock_dir(home: &Path, digest: &str, gen: u64) -> PathBuf {
    let tag = format!("{digest}-{gen}");
    let candidates = [
        std::env::var("XDG_RUNTIME_DIR").ok().filter(|s| !s.is_empty()).map(|s| PathBuf::from(s).join("bp-ke")),
        std::env::var("BP_TMP").ok().filter(|s| !s.is_empty()).map(|s| PathBuf::from(s).join("ke")),
        Some(home.join("ke")),
    ];
    for c in candidates.into_iter().flatten() {
        let p = c.join(&tag);
        if p.to_string_lossy().len() <= 72 { return p }
    }
    std::env::temp_dir().join("bp-ke").join(tag)
}

fn kicad_gui_bin(cli: &str) -> Result<PathBuf, String> {
    if cli.is_empty() { return Err("KiCad was not found: set its path in Settings > CAD".into()) }
    if let Some(b) = open::program(cli, "kicad") { return Ok(b) }
    // macOS bundle layout: Contents/MacOS/kicad beside kicad-cli or one bundle up
    Err(format!("KiCad GUI binary (`kicad`) was not found beside {cli}"))
}

// Actor & process supervision -----------------------------------------------------------------------------------------------

enum Msg {
    Attach {
        client: u64, pro: String, root: String, files: Files, frame: String,
        container: u64, toplevel: u64, display: String, wait: bool, env: Env,
        reply: oneshot::Sender<Result<Value, String>>,
    },
    Show {
        client: u64, pro: String, frame: String, doc: String, item: String, remount: bool, focus: bool,
        reply: oneshot::Sender<Result<Value, String>>,
    },
    Detach {
        client: u64, pro: String, reply: oneshot::Sender<Result<Value, String>>,
    },
    Close {
        client: u64, pro: String, save: String, reply: oneshot::Sender<Result<Value, String>>,
    },
    Restart {
        client: u64, pro: String, env: Env, reply: oneshot::Sender<Result<Value, String>>,
    },
    ClientFrames {
        client: u64, pro: String, frames: Vec<Frame>, reply: oneshot::Sender<Result<Value, String>>,
    },
    Raw {
        sock: String, desc: String, ty: String, msg: Value, timeout: u64, client: String, reply: oneshot::Sender<Value>,
    },
    TransportLine(u64, Value),
    Spawned { pro: String, gen: u64, pid: u32 },
    EventsSock { pro: String, gen: u64, sock: String },
    StartupReason { pro: String, gen: u64, reason: String },
    Ready { pro: String, gen: u64, sock: String, version: String, holder: u64, events_sock: String, ev_since: u64, frames: Vec<Frame>, spawn_container: u64, spawn_active: String },
    StartFailed { pro: String, gen: u64, why: String },
    FramesUpdated { pro: String, gen: u64, holder: Option<u64>, frames: Vec<Frame>, reply: Option<(u64, oneshot::Sender<Result<Value, String>>)> },
    ForkShowRequested { pro: String, gen: u64, kind: String },
    ForkSync { pro: String, gen: u64 },
    /// ForkSync for the project's current session, whichever generation it is (from outside the actor: the prewarm RPC)
    ForkSyncNow { pro: String },
    HealthResult { pro: String, gen: u64, ok: bool, modified_docs: Option<Vec<String>> },
    ChildExited { pro: String, gen: u64, code: i32, tail: String, reply: Option<oneshot::Sender<Result<Value, String>>> },
    Tick,
}

struct Actor {
    hub: Arc<Hub>,
    me: mpsc::UnboundedSender<Msg>,
    info_tx: mpsc::UnboundedSender<(String, String, Option<oneshot::Sender<()>>)>,
    helper: Option<(kipc::Native, u64)>,
    hgen: u64,
    next_id: u64,
    next_gen: u64,
    pend: HashMap<String, oneshot::Sender<Value>>,
    sessions: HashMap<String, Session>,
    attach_waiters: HashMap<String, Vec<(u64, oneshot::Sender<Result<Value, String>>)>>,
}

impl Actor {
    fn ensure_helper(&mut self) {
        if self.helper.is_some() { return }
        self.hgen += 1; let g = self.hgen; let me = self.me.clone();
        let n = kipc::Native::new("", Arc::new(move |o| { let _ = me.send(Msg::TransportLine(g, o)); }));
        self.helper = Some((n, g));
    }

    fn raw_call(&mut self, sock: &str, desc: &str, ty: &str, msg: Value, timeout: u64, client: &str, reply: oneshot::Sender<Value>) {
        self.ensure_helper();
        let Some((n, _)) = &mut self.helper else {
            let _ = reply.send(json!({"ok":false,"error":"transport unavailable"})); return;
        };
        self.next_id += 1;
        let id = self.next_id.to_string();
        self.pend.insert(id, reply);
        n.handle(json!({"id": self.next_id, "sock": sock, "desc": desc, "type": ty, "message": msg, "timeout": timeout, "client": client}));
    }

    fn close_sock(&mut self, sock: &str) {
        if sock.is_empty() { return }
        if let Some((n, _)) = &mut self.helper {
            self.next_id += 1;
            n.handle(json!({"id": self.next_id, "op": "close", "sock": sock}));
        }
    }

    fn publish_ack(&self, s: &Session) -> Option<oneshot::Receiver<()>> {
        put_route(s, &self.me);
        let key = info_key(&s.pro);
        let val = serde_json::to_string(&s.info_json()).unwrap_or_default();
        let (ack_tx, ack_rx) = oneshot::channel();
        self.info_tx.send((key, val, Some(ack_tx))).ok().map(|()| ack_rx)
    }

    fn publish(&self, s: &Session, _direct: Option<u64>) {
        put_route(s, &self.me);
        let key = info_key(&s.pro);
        let val = serde_json::to_string(&s.info_json()).unwrap_or_default();
        let _ = self.info_tx.send((key, val, None));
    }

    fn publish_and_reply(&self, s: &Session, _direct: Option<u64>, reply: oneshot::Sender<Result<Value, String>>, res: Result<Value, String>) {
        if let Some(ack_rx) = self.publish_ack(s) {
            tokio::spawn(async move {
                let _ = ack_rx.await;
                let _ = reply.send(res);
            });
        } else {
            let _ = reply.send(res);
        }
    }

    fn finish_attach_waiters(&mut self, pro: &str, res: Result<Value, String>) {
        let Some(ws) = self.attach_waiters.remove(pro) else { return };
        if let Some(s) = self.sessions.get(pro) {
            if let Some(ack_rx) = self.publish_ack(s) {
                tokio::spawn(async move {
                    let _ = ack_rx.await;
                    for (_, tx) in ws { let _ = tx.send(res.clone()); }
                });
                return;
            }
        }
        for (_, tx) in ws { let _ = tx.send(res.clone()); }
    }

    fn cleanup_session_sockets(&mut self, s: &Session) {
        self.close_sock(&s.sock);
        self.close_sock(&s.events_sock);
        if !s.sock_dir.is_empty() {
            // the sockets' folder goes only once that KiCad has exited (a TERM takes a moment; removing api.sock under a running KiCad
            // left it unreachable): up to 15 s, then it goes anyway
            let (dir, pid) = (s.sock_dir.clone(), s.pid);
            tokio::spawn(async move {
                for _ in 0..60 { if pid == 0 || !crate::plat::proc::alive(pid) { break } tokio::time::sleep(Duration::from_millis(250)).await; }
                let _ = tokio::task::spawn_blocking(move || { let _ = std::fs::remove_dir_all(dir); }).await;
            });
        }
    }

    fn kill_pid(pid: u32) {
        if pid == 0 { return }
        if !crate::plat::proc::kill_group(pid, crate::plat::proc::TERM) {
            let _ = crate::plat::proc::kill(pid, crate::plat::proc::TERM);
        }
        tokio::spawn(async move {
            for _ in 0..15 {
                if !crate::plat::proc::alive(pid) { return }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if !crate::plat::proc::kill_group(pid, crate::plat::proc::KILL) {
                let _ = crate::plat::proc::kill(pid, crate::plat::proc::KILL);
            }
        });
    }

    fn attach(&mut self, client: u64, pro: String, root: String, files: Files, frame: String, container: u64, toplevel: u64, display: String, wait: bool, env: Env, reply: oneshot::Sender<Result<Value, String>>) {
        let now = ipc::now_s();
        if let Some(existing) = self.sessions.get_mut(&pro) {
            if existing.state == "starting" {
                if container > 0 { existing.container = container; }
                if toplevel > 0 { existing.toplevel = toplevel; }
                if !display.is_empty() { existing.display = display; }
                existing.active = canon_frame(&frame).into();
                existing.touched_at = now;
                let snap = existing.clone();
                if wait {
                    self.publish(&snap, Some(client));
                    self.attach_waiters.entry(pro).or_default().push((client, reply));
                } else {
                    let out = snap.info_json();
                    self.publish_and_reply(&snap, Some(client), reply, Ok(out));
                }
                return;
            }
            if matches!(existing.state.as_str(), "ready" | "unresponsive") {
                let prev_cont = existing.container;
                existing.container = container;
                if toplevel > 0 { existing.toplevel = toplevel; }
                if !display.is_empty() { existing.display = display; }
                existing.active = canon_frame(&frame).into();
                existing.touched_at = now;
                let snap = existing.clone();
                if snap.mode == "fork" {
                    let me = self.me.clone(); let cid = client;
                    tokio::spawn(async move {
                        if container > 0 {
                            let _ = raw_on(&me, &snap.sock, &snap.desc, "kiapi.backplane.embed.AttachToParent",
                                json!({"window": container, "toplevel": snap.toplevel, "scale": 1.0}), 15000).await;
                        }
                        match switch_fork_frame(&me, &snap.sock, &snap.desc, &snap.active, "", "", false, false).await {
                            Ok(Some((holder, _, frames)))
                                if !cfg!(windows) || container == 0 || container == prev_cont || frames.iter().any(|f| f.shown && f.xid > 0) =>
                            {
                                let _ = me.send(Msg::FramesUpdated { pro: snap.pro.clone(), gen: snap.gen, holder: Some(holder), frames, reply: Some((cid, reply)) });
                            }
                            _ if cfg!(windows) && container > 0 && prev_cont > 0 && container != prev_cont => {
                                // On Windows, if the previous client crashed and the OS destroyed `prev_cont` (tearing down `bp.8` `WS_CHILD` frames),
                                // automatically restart the session into the new client's container.
                                let _ = me.send(Msg::Restart { client: cid, pro: snap.pro.clone(), env, reply });
                            }
                            _ => {
                                let _ = me.send(Msg::FramesUpdated { pro: snap.pro.clone(), gen: snap.gen, holder: None, frames: snap.frames.clone(), reply: Some((cid, reply)) });
                            }
                        }
                    });
                } else {
                    let out = snap.info_json();
                    let ack = self.publish_ack(&snap);
                    let me = self.me.clone(); let active = snap.active.clone(); let sock = snap.sock.clone(); let desc = snap.desc.clone();
                    tokio::spawn(async move {
                        if let Some(act) = stock_action_for(&active) {
                            let _ = run_stock_action(&me, &sock, &desc, act).await;
                        }
                        if let Some(rx) = ack { let _ = rx.await; }
                        let _ = reply.send(Ok(out));
                    });
                }
                return;
            }
        }

        let sweep_locks = self.sessions.get(&pro).is_some_and(|s| s.state == "failed" || s.reason == "restarting");

        // Warm-session cap: retire the oldest detached clean session before starting a 4th GUI KiCad.
        if let Some(victim) = evict_warm(&self.sessions, &pro, MAX_WARM) {
            if let Some(vs) = self.sessions.get_mut(&victim) {
                vs.closing = true;
                let vs_snap = vs.clone();
                if vs_snap.mode == "fork" && !vs_snap.sock.is_empty() {
                    let me = self.me.clone();
                    tokio::spawn(async move {
                        let _ = raw_on(&me, &vs_snap.sock, &vs_snap.desc, "kiapi.backplane.embed.Close", json!({"mode":"CM_FAIL_IF_MODIFIED"}), 10000).await;
                    });
                } else {
                    Self::kill_pid(vs_snap.pid);
                }
            }
        }

        let kicad_bin = match kicad_gui_bin(&env.cli) {
            Ok(b) => b.to_string_lossy().to_string(),
            Err(why) => { let _ = reply.send(Err(why)); return; }
        };
        let (desc, has_embed_schema) = discover_desc(&self.hub.cfg.home, &env);
        let override_mode = std::env::var("BACKPLANE_KICAD_EMBED_MODE").ok();
        let described = if override_mode.is_none() && !has_embed_schema && env.version.is_empty() {
            describe_cli(&env.cli)
        } else {
            None
        };
        let mode = mode_with(override_mode.as_deref(), &env, has_embed_schema, described.as_ref());
        let eff_display = if !display.is_empty() { display } else { std::env::var("DISPLAY").unwrap_or_default() };

        self.next_gen += 1;
        let gen = self.next_gen;
        let digest = ks::digest(&pro);
        let sock_dir = short_sock_dir(&self.hub.cfg.home, &digest, gen);
        let sock_dir_str = sock_dir.to_string_lossy().to_string();
        let req_sock = if mode == "fork" {
            sock_dir.join("api.sock").to_string_lossy().to_string()
        } else {
            sock_dir.join("kicad").join("api.sock").to_string_lossy().to_string()
        };
        let argv_vec = if mode == "fork" {
            let mut v = fork_argv(&kicad_bin, &pro, &req_sock, container, toplevel, std::process::id());
            // bp.9 and later: the first paint already in Backplane's theme (an older fork would refuse the flag)
            let theme = theme_of(&pro);
            if !theme.is_empty() && !desc.is_empty() && proto::load(&desc).has_own("kiapi.backplane.embed.SetTheme") { let at = v.len() - 1; v.insert(at, theme); v.insert(at, "--theme".into()); }
            v
        } else {
            stock_argv(&kicad_bin, &files)
        };

        let mut sess = Session::new(&pro, &root, mode, &frame, gen, now);
        sess.container = container;
        sess.toplevel = toplevel;
        sess.display = eff_display.clone();
        sess.sock_dir = sock_dir_str.clone();
        sess.argv = argv_vec.join(" ");
        sess.cli = env.cli.clone();
        sess.version = if !env.version.is_empty() {
            env.version.clone()
        } else {
            described.as_ref().and_then(|d| d["version"].as_str()).unwrap_or("").to_string()
        };
        sess.desc = desc.clone();
        sess.caps = env.caps.clone();

        // On Windows, cross-process top-level stock windows are never adopted; publish stock mode ready without spawning a floating window.
        if cfg!(windows) && mode == "stock" {
            sess.state = "ready".into();
            let out = sess.info_json();
            self.sessions.insert(pro.clone(), sess.clone());
            self.publish_and_reply(&sess, Some(client), reply, Ok(out));
            return;
        }

        if wait {
            self.publish(&sess, Some(client));
            self.sessions.insert(pro.clone(), sess.clone());
            self.attach_waiters.entry(pro.clone()).or_default().push((client, reply));
        } else {
            let out = sess.info_json();
            self.sessions.insert(pro.clone(), sess.clone());
            self.publish_and_reply(&sess, Some(client), reply, Ok(out));
        }

        let home = self.hub.cfg.home.clone();
        let me = self.me.clone();
        let active = sess.active.clone();
        let spawn_container = sess.container;
        let mode_s = mode.to_string();
        let root_s = root.clone();
        tokio::spawn(async move {
            // Retire any headless kserve for this project before the GUI opens its files.
            let _ = ipc::retire(&pro).await;
            spawn_gui_session(me, home, pro, root_s, gen, mode_s, argv_vec, sock_dir, req_sock, eff_display, desc, active, spawn_container, sweep_locks).await;
        });
    }

    fn show(&mut self, client: u64, pro: String, frame: String, doc: String, item: String, remount: bool, focus: bool, reply: oneshot::Sender<Result<Value, String>>) {
        let Some(s) = self.sessions.get_mut(&pro) else {
            let _ = reply.send(Err("no embedded KiCad session for this project; attach first".into())); return;
        };
        if s.state == "starting" {
            s.active = canon_frame(&frame).into();
            s.touched_at = ipc::now_s();
            let snap = s.clone();
            let out = snap.info_json();
            self.publish_and_reply(&snap, Some(client), reply, Ok(out));
            return;
        }
        if s.state != "ready" {
            let _ = reply.send(Err(format!("KiCad is {} ({})", s.state, s.reason))); return;
        }
        s.active = canon_frame(&frame).into();
        s.touched_at = ipc::now_s();
        let snap = s.clone();
        let me = self.me.clone();
        let cid = client;
        if snap.mode == "fork" {
            tokio::spawn(async move {
                let synced = match switch_fork_frame(&me, &snap.sock, &snap.desc, &snap.active, &doc, &item, remount, focus).await {
                    Ok(v) => v,
                    Err(e) => { let _ = reply.send(Err(e)); return; }
                };
                if let Some((holder, _, frames)) = synced {
                    let _ = me.send(Msg::FramesUpdated { pro: snap.pro.clone(), gen: snap.gen, holder: Some(holder), frames, reply: Some((cid, reply)) });
                } else {
                    let _ = me.send(Msg::FramesUpdated { pro: snap.pro.clone(), gen: snap.gen, holder: None, frames: snap.frames.clone(), reply: Some((cid, reply)) });
                }
            });
        } else {
            let ack = self.publish_ack(&snap);
            tokio::spawn(async move {
                if !snap.sock.is_empty() {
                    if let Some(act) = stock_action_for(&snap.active) {
                        if let Err(e) = run_stock_action(&me, &snap.sock, &snap.desc, act).await {
                            let _ = reply.send(Err(e)); return;
                        }
                    }
                }
                if let Some(rx) = ack { let _ = rx.await; }
                let _ = reply.send(Ok(snap.info_json()));
            });
        }
    }

    fn detach(&mut self, client: u64, pro: String, reply: oneshot::Sender<Result<Value, String>>) {
        let Some(s) = self.sessions.get_mut(&pro) else {
            let _ = reply.send(Ok(json!({"detached":true}))); return;
        };
        *s = s.clone().detached(ipc::now_s());
        let snap = s.clone();
        if snap.mode == "fork" && snap.state == "ready" && !snap.sock.is_empty() {
            let me = self.me.clone(); let cid = client;
            tokio::spawn(async move {
                let _ = raw_on(&me, &snap.sock, &snap.desc, "kiapi.backplane.embed.AttachToParent",
                    json!({"window": 0, "toplevel": 0, "scale": 1.0}), 10000).await;
                let gf = raw_on(&me, &snap.sock, &snap.desc, "kiapi.backplane.embed.GetFrames", json!({}), 10000).await;
                if gf["ok"].as_bool().unwrap_or(false) {
                    let (holder, _, frames) = parse_get_frames(&gf["message"]);
                    let _ = me.send(Msg::FramesUpdated { pro: snap.pro.clone(), gen: snap.gen, holder: Some(holder), frames, reply: Some((cid, reply)) });
                } else {
                    let _ = me.send(Msg::FramesUpdated { pro: snap.pro.clone(), gen: snap.gen, holder: None, frames: snap.frames.clone(), reply: Some((cid, reply)) });
                }
            });
        } else {
            let out = snap.info_json();
            self.publish_and_reply(&snap, Some(client), reply, Ok(out));
        }
    }

    fn close(&mut self, _client: u64, pro: String, save: String, reply: oneshot::Sender<Result<Value, String>>) {
        let Some(s) = self.sessions.get(&pro).cloned() else {
            let _ = reply.send(Ok(json!({"closed":true}))); return;
        };
        if !s.is_live() {
            let _ = reply.send(Ok(json!({"closed":true}))); return;
        }
        let me = self.me.clone();
        let save_norm = save.trim().to_ascii_lowercase();
        if s.mode == "fork" && !s.sock.is_empty() {
            let cm = close_mode_of(&save_norm);
            let pid = s.pid;
            tokio::spawn(async move {
                let o = raw_on(&me, &s.sock, &s.desc, "kiapi.backplane.embed.Close", json!({"mode": cm}), 15000).await;
                if o["ok"].as_bool().unwrap_or(false) {
                    if pid > 0 {
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_millis(800)).await;
                            if crate::plat::proc::alive(pid) {
                                Actor::kill_pid(pid);
                            }
                        });
                    }
                    let _ = me.send(Msg::ChildExited { pro: s.pro.clone(), gen: s.gen, code: 0, tail: String::new(), reply: Some(reply) });
                } else {
                    let err = o["error"].as_str().unwrap_or("KiCad refused to close").to_string();
                    let mut unsaved = s.unsaved_docs();
                    if unsaved.is_empty() {
                        let gf = raw_on(&me, &s.sock, &s.desc, "kiapi.backplane.embed.GetFrames", json!({}), 5000).await;
                        if gf["ok"].as_bool().unwrap_or(false) {
                            let (_, _, frames) = parse_get_frames(&gf["message"]);
                            unsaved = frames.iter().filter(|f| f.modified).map(|f| if !f.doc.is_empty() { f.doc.clone() } else { f.title.clone() }).collect();
                        }
                    }
                    let _ = reply.send(Ok(json!({"closed":false,"error":err,"unsaved":unsaved})));
                }
            });
            return;
        }

        // Stock mode close: check modified documents unless "discard", save them on "save", then terminate captured group.
        let known_frames = s.frames.clone();
        tokio::spawn(async move {
            if save_norm != "discard" && !s.sock.is_empty() {
                let (docs, modified, _) = stock_modified_docs(&me, &s.sock, &s.desc, &known_frames).await;
                if !modified.is_empty() {
                    if save_norm == "save" {
                        for spec in docs {
                            let r = raw_on(&me, &s.sock, &s.desc, "kiapi.common.commands.SaveDocument", json!({"document": spec}), 10000).await;
                            if !r["ok"].as_bool().unwrap_or(false) {
                                let err = r["error"].as_str().unwrap_or("SaveDocument failed");
                                let _ = reply.send(Err(format!("could not save before closing: {err}")));
                                return;
                            }
                        }
                    } else {
                        let _ = reply.send(Ok(json!({"closed":false,"error":"unsaved changes in KiCad","unsaved":modified})));
                        return;
                    }
                }
            }
            Actor::kill_pid(s.pid);
            let _ = me.send(Msg::ChildExited { pro: s.pro.clone(), gen: s.gen, code: 0, tail: String::new(), reply: Some(reply) });
        });
    }

    fn restart(&mut self, client: u64, pro: String, env: Env, reply: oneshot::Sender<Result<Value, String>>) {
        let prev = self.sessions.get(&pro).cloned();
        if let Some(mut s) = prev.clone() {
            s.closing = true;
            Self::kill_pid(s.pid);
            self.cleanup_session_sockets(&s);
            let stopped = s.stopped("restarting");
            put_route(&stopped, &self.me);
            self.sessions.insert(pro.clone(), stopped);
        }
        let (root, frame, container, toplevel, display) = match prev {
            Some(s) => (s.root, s.active, s.container, s.toplevel, s.display),
            None => {
                let root = Path::new(&pro).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                (root, "PCB".into(), 0, 0, String::new())
            }
        };
        let files = project::read(&root);
        self.attach(client, pro, root, files, frame, container, toplevel, display, true, env, reply);
    }

    fn handle(&mut self, m: Msg) {
        match m {
            Msg::Attach { client, pro, root, files, frame, container, toplevel, display, wait, env, reply } => {
                self.attach(client, pro, root, files, frame, container, toplevel, display, wait, env, reply);
            }
            Msg::Show { client, pro, frame, doc, item, remount, focus, reply } => self.show(client, pro, frame, doc, item, remount, focus, reply),
            Msg::Detach { client, pro, reply } => self.detach(client, pro, reply),
            Msg::Close { client, pro, save, reply } => self.close(client, pro, save, reply),
            Msg::Restart { client, pro, env, reply } => self.restart(client, pro, env, reply),
            Msg::ClientFrames { client, pro, frames, reply } => {
                if let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.mode == "stock" && s.is_live()) {
                    s.frames = frames;
                    let snap = s.clone();
                    let out = snap.info_json();
                    self.publish_and_reply(&snap, Some(client), reply, Ok(out));
                } else {
                    let _ = reply.send(Ok(json!({"ok":true})));
                }
            }
            Msg::Raw { sock, desc, ty, msg, timeout, client, reply } => self.raw_call(&sock, &desc, &ty, msg, timeout, &client, reply),
            Msg::TransportLine(g, o) => {
                if self.helper.as_ref().map(|(_, hg)| *hg) != Some(g) { return }
                let id = match &o["id"] { Value::String(s) => s.clone(), v => v.to_string() };
                if let Some(tx) = self.pend.remove(&id) { let _ = tx.send(o); }
            }
            Msg::Spawned { pro, gen, pid } => {
                if let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.state == "starting") {
                    *s = s.clone().spawned(pid);
                    let snap = s.clone();
                    self.publish(&snap, None);
                }
            }
            Msg::EventsSock { pro, gen, sock } => {
                if let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen) {
                    s.events_sock = sock;
                }
            }
            Msg::StartupReason { pro, gen, reason } => {
                if let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.state == "starting" && s.reason != reason) {
                    s.reason = reason;
                    let snap = s.clone();
                    self.publish(&snap, None);
                }
            }
            Msg::Ready { pro, gen, sock, version, holder, events_sock, ev_since, frames, spawn_container, spawn_active } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.state == "starting") else { return };
                *s = s.clone().ready(&sock, &version, ipc::now_s());
                s.holder = holder;
                if !events_sock.is_empty() { s.events_sock = events_sock; }
                if !frames.is_empty() { s.frames = frames; }
                let snap = s.clone();
                self.publish(&snap, None);
                self.finish_attach_waiters(&pro, Ok(snap.info_json()));
                if snap.mode == "fork" && !snap.events_sock.is_empty() {
                    spawn_fork_events_loop(self.me.clone(), snap.pro.clone(), snap.gen, snap.events_sock.clone(), snap.desc.clone(), ev_since);
                }
                // Reconcile any reattach, detach, or rail switch that arrived while KiCad was still starting.
                if snap.mode == "fork" && (snap.container != spawn_container || snap.active != spawn_active) {
                    let me = self.me.clone();
                    let s2 = snap.clone();
                    tokio::spawn(async move {
                        if s2.container != spawn_container {
                            let _ = raw_on(&me, &s2.sock, &s2.desc, "kiapi.backplane.embed.AttachToParent",
                                json!({"window": s2.container, "toplevel": if s2.container > 0 { s2.toplevel } else { 0 }, "scale": 1.0}), 15000).await;
                        }
                        if s2.container > 0 {
                            if let Ok(Some((h, _, f))) = switch_fork_frame(&me, &s2.sock, &s2.desc, &s2.active, "", "", false, false).await {
                                let _ = me.send(Msg::FramesUpdated { pro: s2.pro, gen: s2.gen, holder: Some(h), frames: f, reply: None });
                            }
                        } else {
                            let gf = raw_on(&me, &s2.sock, &s2.desc, "kiapi.backplane.embed.GetFrames", json!({}), 10000).await;
                            if gf["ok"].as_bool().unwrap_or(false) {
                                let (h, _, f) = parse_get_frames(&gf["message"]);
                                let _ = me.send(Msg::FramesUpdated { pro: s2.pro, gen: s2.gen, holder: Some(h), frames: f, reply: None });
                            }
                        }
                    });
                } else if snap.mode == "stock" && snap.active != spawn_active {
                    if let Some(act) = stock_action_for(&snap.active) {
                        let me = self.me.clone();
                        let sock = snap.sock.clone();
                        let desc = snap.desc.clone();
                        tokio::spawn(async move { let _ = run_stock_action(&me, &sock, &desc, act).await; });
                    }
                }
            }
            Msg::StartFailed { pro, gen, why } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.state == "starting") else { return };
                let pid = s.pid;
                *s = s.clone().failed(&why);
                let snap = s.clone();
                Self::kill_pid(pid);
                self.cleanup_session_sockets(&snap);
                self.publish(&snap, None);
                self.finish_attach_waiters(&pro, Err(why));
            }
            Msg::FramesUpdated { pro, gen, holder, frames, reply } => {
                if let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.is_live()) {
                    if let Some(h) = holder { s.holder = h; }
                    s.frames = frames;
                    let snap = s.clone();
                    if let Some((cid, tx)) = reply {
                        let out = snap.info_json();
                        self.publish_and_reply(&snap, Some(cid), tx, Ok(out));
                    } else {
                        self.publish(&snap, None);
                    }
                } else if let Some((_, tx)) = reply {
                    let _ = tx.send(Err("KiCad session ended".into()));
                }
            }
            Msg::ForkShowRequested { pro, gen, kind } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.state == "ready") else { return };
                s.active = canon_frame(&kind).into();
                let snap = s.clone();
                let me = self.me.clone();
                tokio::spawn(async move {
                    if let Ok(Some((holder, _, frames))) = switch_fork_frame(&me, &snap.sock, &snap.desc, &snap.active, "", "", false, false).await {
                        let _ = me.send(Msg::FramesUpdated { pro: snap.pro, gen: snap.gen, holder: Some(holder), frames, reply: None });
                    }
                });
            }
            Msg::ForkSyncNow { pro } => {
                if let Some(gen) = self.sessions.get(&pro).map(|s| s.gen) { let _ = self.me.send(Msg::ForkSync { pro, gen }); }
            }
            Msg::ForkSync { pro, gen } => {
                let Some(s) = self.sessions.get(&pro).filter(|s| s.gen == gen && s.is_live()).cloned() else { return };
                let me = self.me.clone();
                tokio::spawn(async move {
                    let gf = raw_on(&me, &s.sock, &s.desc, "kiapi.backplane.embed.GetFrames", json!({}), 10000).await;
                    if gf["ok"].as_bool().unwrap_or(false) {
                        let (holder, _, frames) = parse_get_frames(&gf["message"]);
                        let _ = me.send(Msg::FramesUpdated { pro: s.pro, gen: s.gen, holder: Some(holder), frames, reply: None });
                    }
                });
            }
            Msg::HealthResult { pro, gen, ok, modified_docs } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && matches!(s.state.as_str(), "ready" | "unresponsive")) else { return };
                let was_unresponsive = s.state == "unresponsive";
                let prev = s.clone();
                *s = s.clone().ping_result(ok);
                if s.mode == "stock" {
                    if let Some(mods) = modified_docs {
                        let mut frames: Vec<Frame> = mods.iter().map(|d| Frame {
                            kind: if d.ends_with(".kicad_sch") { "SCH".into() } else { "PCB".into() },
                            doc: d.clone(), title: d.rsplit('/').next().unwrap_or(d).into(),
                            modified: true, shown: true, realized: false, xid: 0, min_w: 0, min_h: 0,
                        }).collect();
                        frames.sort_by(|a, b| a.doc.cmp(&b.doc));
                        s.frames = frames;
                    }
                }
                let recovered_fork = was_unresponsive && s.state == "ready" && s.mode == "fork";
                if *s != prev {
                    let snap = s.clone();
                    self.publish(&snap, None);
                }
                if recovered_fork {
                    let _ = self.me.send(Msg::ForkSync { pro, gen });
                }
            }
            Msg::ChildExited { pro, gen, code, tail, reply } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen) else {
                    if let Some(tx) = reply { let _ = tx.send(Ok(json!({"closed":true}))); }
                    return;
                };
                if matches!(s.state.as_str(), "stopped" | "failed") {
                    if let Some(tx) = reply { let _ = tx.send(Ok(json!({"closed":true}))); }
                    return;
                }
                let was_starting = s.state == "starting";
                let old = s.clone();
                *s = s.clone().exited(code, &tail);
                let snap = s.clone();
                self.cleanup_session_sockets(&old);
                if let Some(tx) = reply {
                    self.publish_and_reply(&snap, None, tx, Ok(json!({"closed":true})));
                } else {
                    self.publish(&snap, None);
                }
                if was_starting {
                    let why = if snap.reason.is_empty() { "KiCad exited before becoming ready".to_string() } else { snap.reason.clone() };
                    self.finish_attach_waiters(&pro, Err(why));
                }
            }
            Msg::Tick => {
                let targets: Vec<Session> = self.sessions.values()
                    .filter(|s| matches!(s.state.as_str(), "ready" | "unresponsive") && !s.sock.is_empty())
                    .cloned().collect();
                for s in targets {
                    let me = self.me.clone();
                    tokio::spawn(async move {
                        let p = raw_on(&me, &s.sock, &s.desc, "kiapi.common.commands.Ping", json!({}), 4000).await;
                        let ok = p["ok"].as_bool().unwrap_or(false);
                        let mods = if ok && s.mode == "stock" {
                            let (_, names, supported) = stock_modified_docs(&me, &s.sock, &s.desc, &s.frames).await;
                            supported.then_some(names)
                        } else {
                            None
                        };
                        let _ = me.send(Msg::HealthResult { pro: s.pro, gen: s.gen, ok, modified_docs: mods });
                    });
                }
            }
        }
    }
}

async fn raw_on(me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, ty: &str, msg: Value, timeout: u64) -> Value {
    let (t, r) = oneshot::channel();
    if me.send(Msg::Raw { sock: sock.into(), desc: desc.into(), ty: ty.into(), msg, timeout, client: String::new(), reply: t }).is_err() {
        return json!({"ok":false,"error":"embed actor stopped"})
    }
    r.await.unwrap_or_else(|_| json!({"ok":false,"error":"no reply"}))
}

async fn drain_lines<R: AsyncRead + Unpin>(r: R, mut each: impl FnMut(String)) {
    let (mut b, mut v) = (BufReader::new(r), Vec::new());
    loop {
        v.clear();
        match b.read_until(b'\n', &mut v).await { Ok(0) | Err(_) => return, Ok(_) => {} }
        if v.last() == Some(&b'\n') { v.pop(); }
        if v.last() == Some(&b'\r') { v.pop(); }
        each(String::from_utf8_lossy(&v).into_owned());
    }
}

async fn show_fork_frame(me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, kind: &str, doc: &str, item: &str, focus: bool) -> Result<Value, String> {
    let mut body = json!({ "kind": fk_of(kind), "focus": focus });
    if !doc.is_empty() { body["document"] = json!(doc); }
    if !item.is_empty() { body["item"] = json!(item); }
    for attempt in 0..8 {
        let o = raw_on(me, sock, desc, "kiapi.backplane.embed.ShowFrame", body.clone(), 15000).await;
        if o["ok"].as_bool().unwrap_or(false) { return Ok(o) }
        let st = o["status"].as_str().unwrap_or("");
        if st == "AS_NOT_READY" && attempt < 7 {
            tokio::time::sleep(Duration::from_millis(150)).await;
            continue;
        }
        let e = o["error"].as_str().unwrap_or("ShowFrame failed");
        return Err(if st.is_empty() { e.to_string() } else { format!("KiCad answered {st}: {e}") });
    }
    Err("ShowFrame did not succeed".into())
}

/// Show `kind` in fork mode and hide every other currently-shown frame via `HideFrame` so GTK3/wxMSW owns frame map state cleanly.
async fn switch_fork_frame(
    me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, kind: &str, doc: &str, item: &str, remount: bool, focus: bool,
) -> Result<Option<(u64, String, Vec<Frame>)>, String> {
    let target_kind = canon_frame(kind);
    if remount {
        let gf0 = raw_on(me, sock, desc, "kiapi.backplane.embed.GetFrames", json!({}), 5000).await;
        if gf0["ok"].as_bool().unwrap_or(false) {
            let (_, _, f0) = parse_get_frames(&gf0["message"]);
            for f in f0.into_iter().filter(|f| f.kind == target_kind && f.xid > 0) {
                let _ = raw_on(me, sock, desc, "kiapi.backplane.embed.HideFrame", json!({ "window": f.xid }), 5000).await;
            }
        }
    }
    let shown_res = show_fork_frame(me, sock, desc, kind, doc, item, focus).await?;
    let resp_win = u64_of(&shown_res["message"]["frame"]["window"]);
    let resp_kind = kind_of_fk_val(&shown_res["message"]["frame"]["kind"]);
    let target_win = (resp_win > 0 && resp_kind == target_kind).then_some(resp_win);
    let gf = raw_on(me, sock, desc, "kiapi.backplane.embed.GetFrames", json!({}), 10000).await;
    if !gf["ok"].as_bool().unwrap_or(false) { return Ok(None) }
    let (mut holder, mut events_sock, mut frames) = parse_get_frames(&gf["message"]);
    let others: Vec<u64> = frames.iter()
        .filter(|f| f.shown && f.xid > 0 && match target_win { Some(w) => f.xid != w, None => f.kind != target_kind })
        .map(|f| f.xid)
        .collect();
    if !others.is_empty() {
        for win in others {
            let _ = raw_on(me, sock, desc, "kiapi.backplane.embed.HideFrame", json!({ "window": win }), 5000).await;
        }
        let gf2 = raw_on(me, sock, desc, "kiapi.backplane.embed.GetFrames", json!({}), 10000).await;
        if gf2["ok"].as_bool().unwrap_or(false) {
            let (h2, ev2, f2) = parse_get_frames(&gf2["message"]);
            holder = h2;
            if !ev2.is_empty() { events_sock = ev2; }
            frames = f2;
        }
    }
    Ok(Some((holder, events_sock, frames)))
}

async fn run_stock_action(me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, action: &str) -> Result<(), String> {
    for attempt in 0..8 {
        let o = raw_on(me, sock, desc, "kiapi.common.commands.RunAction", json!({"action": action}), 10000).await;
        if o["ok"].as_bool().unwrap_or(false) { return Ok(()) }
        let st = o["status"].as_str().unwrap_or("");
        // Before PCB_EDIT_FRAME is opened, stock KiCad's project manager answers AS_UNHANDLED; the client opens the frame via shortcut.
        if st == "AS_UNHANDLED" { return Ok(()) }
        if matches!(st, "AS_BUSY" | "AS_NOT_READY") && attempt < 7 {
            tokio::time::sleep(Duration::from_millis(150)).await;
            continue;
        }
        let e = o["error"].as_str().unwrap_or("RunAction failed");
        return Err(if st.is_empty() { e.to_string() } else { format!("KiCad answered {st}: {e}") });
    }
    Ok(())
}

async fn stock_modified_docs(me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, frames: &[Frame]) -> (Vec<Value>, Vec<String>, bool) {
    let mut specs = vec![];
    let mut names = vec![];
    let mut supported = false;
    for (dt, kind) in [("DOCTYPE_PCB", "PCB"), ("DOCTYPE_SCHEMATIC", "SCH")] {
        let o = raw_on(me, sock, desc, "kiapi.common.commands.GetOpenDocuments", json!({"type": dt}), 4000).await;
        let Some(docs) = o["message"]["documents"].as_array() else { continue };
        let frame_dirty = frames.iter().any(|f| f.kind == kind && (f.modified || f.title.trim_start().starts_with('*')));
        for d in docs {
            let m = raw_on(me, sock, desc, "kiapi.common.commands.GetDocumentModifiedState", json!({"document": d}), 4000).await;
            if m["ok"].as_bool().unwrap_or(false) { supported = true; }
            let state = &m["message"]["state"];
            // Stock KiCad 10.0.x answers AS_UNHANDLED for GetDocumentModifiedState; fall back to the client-observed '*' frame title.
            let is_dirty = state == &json!("DMS_MODIFIED") || state == &json!(2)
                || (!m["ok"].as_bool().unwrap_or(false) && frame_dirty);
            if is_dirty {
                let label = d["board_filename"].as_str()
                    .or_else(|| d["project"]["name"].as_str())
                    .unwrap_or("unsaved document").to_string();
                specs.push(d.clone());
                names.push(label);
            }
        }
    }
    if names.is_empty() {
        for f in frames.iter().filter(|f| f.modified || f.title.trim_start().starts_with('*')) {
            names.push(if !f.doc.is_empty() { f.doc.clone() } else { f.title.clone() });
        }
    }
    (specs, names, supported)
}

fn sweep_crashed_locks(root: &str, pro: &str) {
    let mut dirs = vec![];
    if !root.is_empty() { dirs.push(PathBuf::from(root)); }
    if let Some(p) = Path::new(pro).parent() {
        if !dirs.iter().any(|d| d == p) { dirs.push(p.to_path_buf()); }
    }
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for ent in rd.flatten() {
            let name = ent.file_name();
            let s = name.to_string_lossy();
            if s.starts_with('~') && s.ends_with(".lck") {
                let _ = std::fs::remove_file(ent.path());
            }
        }
    }
}

fn embed_sock_exists(sock: &str) -> bool {
    #[cfg(windows)]
    {
        let p = Path::new(sock);
        let (Some(dir), Some(base)) = (p.parent(), p.file_name()) else { return false };
        std::fs::read_dir(r"\\.\pipe\").ok().into_iter().flatten().flatten().any(|e| {
            let full = e.file_name().to_string_lossy().into_owned();
            let fp = Path::new(&full);
            fp.parent() == Some(dir) && fp.file_name() == Some(base)
        })
    }
    #[cfg(not(windows))]
    {
        Path::new(sock).exists()
    }
}

async fn spawn_gui_session(
    me: mpsc::UnboundedSender<Msg>, home: PathBuf, pro: String, root: String, gen: u64, mode: String,
    argv: Vec<String>, sock_dir: PathBuf, req_sock: String, display: String, desc: String, active: String, spawn_container: u64, sweep_locks: bool,
) {
    if sweep_locks {
        sweep_crashed_locks(&root, &pro);
    }
    let cfg_dir = home.join("kicad-embed-cfg").join(ks::digest(&pro));
    let cache_dir = home.join("kicad-embed-cache").join(ks::digest(&pro));
    {
        use crate::plat::fs::DirBuilderExt;
        for d in [&sock_dir, &sock_dir.join("kicad"), &cfg_dir, &cache_dir] {
            if let Err(e) = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(d) {
                let _ = me.send(Msg::StartFailed { pro, gen, why: format!("could not prepare {}: {e}", d.display()) });
                return;
            }
        }
    }
    if mode == "stock" {
        if let Err(why) = seed_stock_config(&cfg_dir) {
            let _ = me.send(Msg::StartFailed { pro, gen, why });
            return;
        }
    }

    let before = kipc::sockets_at(&req_sock);
    let cwd = Path::new(&pro).parent().filter(|p| !p.as_os_str().is_empty()).map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, &argv[0]);
    c.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(cwd)
        .kill_on_drop(false);
    #[cfg(not(windows))]
    {
        c.env("GDK_BACKEND", "x11")
            .env("GDK_SCALE", "1")
            .env_remove("WAYLAND_DISPLAY");
        if !display.is_empty() { c.env("DISPLAY", &display); }
        if mode == "stock" {
            c.env("XDG_CONFIG_HOME", &cfg_dir)
                .env("XDG_CACHE_HOME", &cache_dir)
                .env("TMPDIR", &sock_dir);
        } else if std::env::var_os("TMPDIR").is_none() {
            c.env("TMPDIR", &sock_dir);
        }
    }
    #[cfg(windows)]
    let _ = &display;
    for k in ["BACKPLANE_KICAD_LIBRARY", "KICAD_CONFIG_HOME"] {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() { c.env(k, v); }
        }
    }
    // Own session + PDEATHSIG (Linux) / Job Object (Windows) / reaper (macOS) so stock KiCad never outlives a stopped or killed hub.
    crate::plat::proc::agent_group(&mut c);

    let mut tries = 0;
    let mut child = loop {
        match c.spawn() {
            Ok(ch) => break ch,
            Err(e) if e.raw_os_error() == Some(crate::plat::errno::ETXTBSY) && tries < 40 => {
                tries += 1; tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(e) => {
                let _ = me.send(Msg::StartFailed { pro, gen, why: format!("could not start KiCad: {e}") });
                return;
            }
        }
    };
    let pid = child.id().unwrap_or(0);
    crate::plat::proc::adopt(pid, true);
    if pid > 0 {
        live_pids().lock().unwrap_or_else(|e| e.into_inner()).insert(pid);
        crate::agents::proc::watch_group(pid);
    }
    let _ = me.send(Msg::Spawned { pro: pro.clone(), gen, pid });

    let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
    let (sock_tx, sock_rx) = oneshot::channel::<String>();
    let sock_tx = Arc::new(Mutex::new(Some(sock_tx)));
    let done = Arc::new(AtomicBool::new(false));
    let tail = Arc::new(Mutex::new(VecDeque::<String>::new()));

    // Poll `sockets_at` so buffered stdout on a pipe (and stock KiCad, which prints no ready line) never delays discovery.
    {
        let sock_tx = sock_tx.clone();
        let done = done.clone();
        let req_sock = req_sock.clone();
        tokio::spawn(async move {
            while !done.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(80)).await;
                if let Some((found, _)) = kipc::sockets_at(&req_sock).into_iter().find(|(p, i)| before.get(p) != Some(i)) {
                    if let Some(tx) = sock_tx.lock().unwrap().take() { let _ = tx.send(found); }
                    return;
                }
            }
        });
    }

    let so = {
        let sock_tx = sock_tx.clone();
        let tail = tail.clone();
        let me = me.clone();
        let pro = pro.clone();
        tokio::spawn(async move {
            if let Some(out) = stdout {
                drain_lines(out, |line| {
                    { let mut t = tail.lock().unwrap(); t.push_back(line.clone()); if t.len() > 16 { t.pop_front(); } }
                    if let Some(ev) = line.strip_prefix(EVENTS_LINE) {
                        let ev_sock = ev.trim().strip_prefix("ipc://").unwrap_or(ev.trim()).to_string();
                        let _ = me.send(Msg::EventsSock { pro: pro.clone(), gen, sock: ev_sock });
                    }
                    if let Some(path) = ks::ready(&line) {
                        if let Some(tx) = sock_tx.lock().unwrap().take() { let _ = tx.send(path); }
                    }
                }).await;
            }
        })
    };
    let se = {
        let tail = tail.clone();
        tokio::spawn(async move {
            if let Some(err) = stderr {
                drain_lines(err, |line| {
                    let mut t = tail.lock().unwrap(); t.push_back(line); if t.len() > 16 { t.pop_front(); }
                }).await;
            }
        })
    };

    // Wait for child exit in parallel and always notify the actor.
    {
        let done = done.clone();
        let tail = tail.clone();
        let me = me.clone();
        let pro = pro.clone();
        tokio::spawn(async move {
            let code = child.wait().await.map(|s| s.code().unwrap_or(128)).unwrap_or(1);
            if pid > 0 {
                live_pids().lock().unwrap_or_else(|e| e.into_inner()).remove(&pid);
                crate::agents::proc::unwatch_group(pid);
                crate::plat::proc::kill_group(pid, crate::plat::proc::KILL);
            }
            done.store(true, Ordering::SeqCst);
            let _ = so.await; let _ = se.await;
            let joined: Vec<String> = tail.lock().unwrap().iter().cloned().collect();
            let _ = me.send(Msg::ChildExited { pro, gen, code, tail: joined.join("\n"), reply: None });
        });
    }

    // Handshake: wait up to 30 s for the API socket, then Ping + GetVersion (retrying AS_NOT_READY while frames initialize).
    // If fork KiCad opens a startup modal dialog before `SetReadyToReply` (e.g. LocalHistory's "Recover Unsaved Changes" after a crash),
    // `<sock>.events` already reports `ET_DIALOG_OPENED`: extend `deadline` while the dialog is open so the hub never kills KiCad under the user.
    let mut deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let sock = match tokio::time::timeout_at(deadline, sock_rx).await {
        Ok(Ok(s)) => s,
        _ => {
            if !done.load(Ordering::SeqCst) {
                let _ = me.send(Msg::StartFailed { pro, gen, why: "KiCad did not open its IPC socket within 30 s".into() });
            }
            return;
        }
    };

    let mut version = String::new();
    let mut hand_ok = false;
    let mut ev_since: u64 = 0;
    let mut open_dialogs: Vec<String> = Vec::new();
    while tokio::time::Instant::now() < deadline && !done.load(Ordering::SeqCst) {
        let p = raw_on(&me, &sock, &desc, "kiapi.common.commands.Ping", json!({}), 3000).await;
        if p["ok"].as_bool().unwrap_or(false) {
            let v = raw_on(&me, &sock, &desc, "kiapi.common.commands.GetVersion", json!({}), 3000).await;
            if v["ok"].as_bool().unwrap_or(false) {
                version = v["message"]["version"]["full_version"].as_str().unwrap_or("").to_string();
                hand_ok = true;
                break;
            }
        } else if mode == "fork" {
            let ev_path = format!("{sock}.events");
            if embed_sock_exists(&ev_path) {
                let o = raw_on(&me, &ev_path, &desc, "kiapi.backplane.embed.NextEvents",
                    json!({"since": ev_since, "wait_ms": 0, "max": 64}), 1500).await;
                if o["ok"].as_bool().unwrap_or(false) {
                    let next = u64_of(&o["message"]["next"]);
                    if next > ev_since { ev_since = next; }
                    if let Some(evs) = o["message"]["events"].as_array() {
                        for ev in evs {
                            let ty = ev["type"].as_str().unwrap_or("");
                            let ty_n = ev["type"].as_i64().unwrap_or(0);
                            let detail = ev["detail"].as_str().unwrap_or("dialog").to_string();
                            if ty == "ET_DIALOG_OPENED" || ty_n == 10 {
                                open_dialogs.push(detail);
                            } else if ty == "ET_DIALOG_CLOSED" || ty_n == 11 {
                                if let Some(pos) = open_dialogs.iter().rposition(|d| d == &detail) {
                                    open_dialogs.remove(pos);
                                } else {
                                    open_dialogs.pop();
                                }
                            }
                        }
                    }
                }
            }
            if let Some(dlg) = open_dialogs.last() {
                deadline = tokio::time::Instant::now() + Duration::from_secs(30);
                let _ = me.send(Msg::StartupReason { pro: pro.clone(), gen, reason: format!("waiting on dialog: {dlg}") });
            } else {
                let _ = me.send(Msg::StartupReason { pro: pro.clone(), gen, reason: String::new() });
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !hand_ok {
        if !done.load(Ordering::SeqCst) {
            let _ = me.send(Msg::StartFailed { pro, gen, why: "KiCad did not answer Ping and GetVersion within 30 s".into() });
        }
        return;
    }

    if mode == "fork" {
        let gf0 = raw_on(&me, &sock, &desc, "kiapi.backplane.embed.GetFrames", json!({}), 10000).await;
        let (mut holder, mut events_sock, mut frames) = if gf0["ok"].as_bool().unwrap_or(false) {
            parse_get_frames(&gf0["message"])
        } else {
            (0, format!("{sock}.events"), vec![])
        };
        if events_sock.is_empty() { events_sock = format!("{sock}.events"); }
        #[cfg(target_os = "linux")]
        {
            // On the very first frame opened at process startup, wxGTK's process-wide `gs_decorExtentSupport` starts at 0 and only
            // flips to 2 when wxGTK's 1000 ms `wxRequestFrameExtentsTimeout` fires after the first `wxTopLevelWindowGTK::Show(true)`.
            // Because `PrepareFrame` strips decorations, `GTKUpdateDecorSize` returns early without calling `gtk_widget_show` on that
            // first frame. Waiting past the 1000 ms timer and toggling `HideFrame` + `ShowFrame` calls `gtk_widget_show` cleanly.
            if let Ok(first) = show_fork_frame(&me, &sock, &desc, &active, "", "", false).await {
                let win = u64_of(&first["message"]["frame"]["window"]);
                if win > 0 {
                    tokio::time::sleep(Duration::from_millis(1150)).await;
                    let _ = raw_on(&me, &sock, &desc, "kiapi.backplane.embed.HideFrame", json!({ "window": win }), 5000).await;
                }
            }
        }
        if let Ok(Some((h2, ev2, f2))) = switch_fork_frame(&me, &sock, &desc, &active, "", "", false, false).await {
            holder = h2;
            if !ev2.is_empty() { events_sock = ev2; }
            frames = f2;
        }
        // the chords KiCad hands to Backplane, with Ctrl+; for a comment (comment.rs)
        let rk = raw_on(&me, &sock, &desc, "kiapi.backplane.embed.SetReservedKeys", json!({"chords": super::comment::RESERVED}), 5000).await;
        eprintln!("kicad embed: reserved keys {}", if rk["ok"] == true { rk["message"]["chords"].to_string() } else { format!("refused: {}", rk["error"]) });
        let _ = me.send(Msg::Ready { pro, gen, sock, version, holder, events_sock, ev_since, frames, spawn_container, spawn_active: active });
    } else {
        if let Some(act) = stock_action_for(&active) {
            let _ = run_stock_action(&me, &sock, &desc, act).await;
        }
        let _ = me.send(Msg::Ready { pro, gen, sock, version, holder: 0, events_sock: String::new(), ev_since: 0, frames: vec![], spawn_container, spawn_active: active });
    }
}

fn spawn_fork_events_loop(me: mpsc::UnboundedSender<Msg>, pro: String, gen: u64, events_sock: String, desc: String, init_since: u64) {
    tokio::spawn(async move {
        let mut since: u64 = init_since;
        let mut errs = 0u32;
        loop {
            let o = raw_on(&me, &events_sock, &desc, "kiapi.backplane.embed.NextEvents",
                json!({"since": since, "wait_ms": 25000, "max": 0}), 35000).await;
            if !o["ok"].as_bool().unwrap_or(false) {
                errs += 1;
                eprintln!("kicad embed: NextEvents failed ({errs}/6): {}", o["error"].as_str().unwrap_or(""));
                if errs > 5 { return }
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }
            errs = 0;
            let msg = &o["message"];
            let next = u64_of(&msg["next"]);
            if next > since { since = next; }
            let mut need_sync = msg["lost"].as_bool().unwrap_or(false);
            let mut show_req: Option<String> = None;
            if let Some(evs) = msg["events"].as_array() {
                for ev in evs {
                    let ty = ev["type"].as_str().unwrap_or("");
                    let ty_n = ev["type"].as_i64().unwrap_or(0);
                    if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("kicad embed event: {ev}"); }
                    super::checkrun::on_event(&pro, ev);
                    if ty == "ET_SELECTION_CHANGED" || ty_n == 14 { super::xprobe::kicad_selected(&pro, ev); continue }
                    if super::reload::on_event(&pro, ev) { need_sync = true; continue }
                    if super::comment::on_event(&pro, ev) { continue }
                    if super::progress::on_event(&pro, ev) { continue }
                    if ty == "ET_SHOW_REQUESTED" || ty_n == 9 {
                        show_req = Some(kind_of_fk_val(&ev["frame"]["kind"]).to_string());
                    } else if matches!(ty, "ET_FRAME_OPENED" | "ET_FRAME_CLOSED" | "ET_FRAME_SHOWN" | "ET_FRAME_HIDDEN"
                        | "ET_TITLE_CHANGED" | "ET_MODIFIED_CHANGED" | "ET_DETACHED" | "ET_DOCUMENT_CHANGED" | "ET_ATTACHED")
                        || matches!(ty_n, 1 | 2 | 3 | 4 | 5 | 6 | 12 | 13 | 18)
                    {
                        need_sync = true;
                    }
                }
            }
            if let Some(kind) = show_req {
                if me.send(Msg::ForkShowRequested { pro: pro.clone(), gen, kind }).is_err() { return }
            } else if need_sync {
                if me.send(Msg::ForkSync { pro: pro.clone(), gen }).is_err() { return }
            }
        }
    });
}

// Hub wiring & RPC handler --------------------------------------------------------------------------------------------------

pub fn start(hub: &Arc<Hub>) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (info_tx, mut info_rx) = mpsc::unbounded_channel::<(String, String, Option<oneshot::Sender<()>>)>();
    let info_hub = hub.clone();
    tokio::spawn(async move {
        while let Some((k, v, ack)) = info_rx.recv().await {
            info_hub.set_info(&k, &v).await;
            if let Some(a) = ack { let _ = a.send(()); }
        }
    });
    let mut a = Actor {
        hub: hub.clone(), me: tx.clone(), info_tx, helper: None, hgen: 0, next_id: 0, next_gen: 0,
        pend: HashMap::new(), sessions: HashMap::new(), attach_waiters: HashMap::new(),
    };
    tokio::spawn(async move { while let Some(m) = rx.recv().await { a.handle(m) } });
    let tick = tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            if tick.send(Msg::Tick).is_err() { return }
        }
    });
    hub.register_rpc(Arc::new(EmbedRpc(tx)));
}

pub struct EmbedRpc(mpsc::UnboundedSender<Msg>);

const METHODS: &[&str] = &[
    "editor.kicad.attach",
    "editor.kicad.show",
    "editor.kicad.detach",
    "editor.kicad.close",
    "editor.kicad.restart",
    "editor.kicad.frames",
    "editor.kicad.touched",
    "editor.kicad.theme",
    "editor.kicad.prewarm",
];

fn ok_reply(id: &Value, payload: Value) -> Vec<Value> {
    let mut r = json!({"t":"reply","id":id,"ok":true});
    if let (Some(dst), Some(src)) = (r.as_object_mut(), payload.as_object()) {
        for (k, v) in src {
            if k != "t" && k != "id" && k != "ok" { dst.insert(k.clone(), v.clone()); }
        }
    }
    vec![r]
}

fn err_reply(id: &Value, error: &str) -> Vec<Value> {
    vec![json!({"t":"reply","id":id,"ok":false,"error":error})]
}

impl RpcHandler for EmbedRpc {
    fn methods(&self) -> &'static [&'static str] { METHODS }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        let actor_tx = self.0.clone();
        let client = who.client;
        Box::pin(async move {
            if crate::copy::on() {
                return err_reply(id, "a hub on a copied log (BACKPLANE_COPY=1) does not start or control GUI KiCad");
            }
            let raw_project = p["project"].as_str().unwrap_or("");
            let req_frame = p["frame"].as_str().unwrap_or("");
            let resolved = match resolve_project(raw_project, req_frame) {
                Ok(r) => r,
                Err(why) => return err_reply(id, &why),
            };
            if !super::known_root(hub, &resolved.root, "").await {
                return err_reply(id, &format!("{} is not in a project this hub knows", resolved.pro));
            }
            if let Some(t) = p["theme"].as_str().filter(|t| matches!(*t, "light" | "dark")) { themes().lock().unwrap().insert(norm_pro_key(&resolved.pro), t.to_string()); }
            if method == "editor.kicad.prewarm" {
                // the client, once the first editor is on show: the others opened hidden at the pane's size (KiCad paints a pre-warmed frame at
                // the size given; at its 1280x800 default the first real show relaid the whole frame out)
                let Some(gui) = gui_route(&resolved.pro) else { return ok_reply(id, json!({"prewarmed": []})) };
                if gui.mode != "fork" || gui.desc.is_empty() || !proto::load(&gui.desc).has_field("kiapi.backplane.embed.ShowFrame", "prewarm") { return ok_reply(id, json!({"prewarmed": []})) }
                let (w, h) = (p["width"].as_u64().unwrap_or(0), p["height"].as_u64().unwrap_or(0));
                let have: Vec<String> = p["have"].as_array().into_iter().flatten().filter_map(|k| k.as_str().map(|k| canon_frame(k).to_string())).collect();
                let kinds = prewarm_kinds(p["active"].as_str().unwrap_or("PCB"), &have);
                let (me, gui2, pro) = (actor_tx.clone(), gui.clone(), resolved.pro.clone());
                tokio::spawn(async move {
                    for k in kinds {
                        let mut body = json!({"kind": fk_of(k), "prewarm": true});
                        if w > 0 && h > 0 { body["width"] = json!(w); body["height"] = json!(h); }
                        let _ = gui_call(&gui2, "kiapi.backplane.embed.ShowFrame", body).await;
                        // KiCad marks it realized ~0.4 s after painting it, with no event: the frames are read again then
                        tokio::time::sleep(Duration::from_millis(900)).await;
                        if me.send(Msg::ForkSyncNow { pro: pro.clone() }).is_err() { return }
                        tokio::time::sleep(Duration::from_millis(600)).await;
                    }
                });
                return ok_reply(id, json!({"prewarming": true}))
            }
            if method == "editor.kicad.theme" {
                let mode = p["mode"].as_str().unwrap_or("");
                if theme_mode(mode).is_empty() { return err_reply(id, "mode is light or dark") }
                themes().lock().unwrap().insert(norm_pro_key(&resolved.pro), mode.to_string());
                // a running fork that can: now, live; otherwise it is used at the next launch
                let Some(gui) = gui_route(&resolved.pro) else { return ok_reply(id, json!({"applied": false, "why": "KiCad is not running"})) };
                if gui.mode != "fork" || gui.desc.is_empty() || !proto::load(&gui.desc).has_own("kiapi.backplane.embed.SetTheme") { return ok_reply(id, json!({"applied": false, "why": "this KiCad cannot switch its theme (needs Backplane KiCad bp.9)"})) }
                let o = gui_call(&gui, "kiapi.backplane.embed.SetTheme", json!({"mode": theme_mode(mode)})).await;
                return if o["ok"].as_bool().unwrap_or(false) { ok_reply(id, json!({"applied": true, "theme": o["message"].clone()})) } else { err_reply(id, &format!("SetTheme: {}", o["error"].as_str().unwrap_or(""))) }
            }
            if method == "editor.kicad.touched" { unsaved_update(&resolved.pro, |u| u.touched()); return ok_reply(id, json!({})) }
            let (tx, rx) = oneshot::channel();
            let sent = match method {
                "editor.kicad.attach" => {
                    let container = p.get("container").map(u64_of).filter(|&x| x > 0)
                        .or_else(|| p.get("window").map(u64_of)).unwrap_or(0);
                    let toplevel = p.get("toplevel").map(u64_of).unwrap_or(0);
                    let display = p["display"].as_str().unwrap_or("").to_string();
                    let wait = p["wait"].as_bool().unwrap_or(true);
                    let env = ipc::runtime(hub).await;
                    actor_tx.send(Msg::Attach {
                        client, pro: resolved.pro, root: resolved.root, files: resolved.files,
                        frame: resolved.frame, container, toplevel, display, wait, env, reply: tx,
                    })
                }
                "editor.kicad.show" => {
                    let doc = p["document"].as_str().unwrap_or("").to_string();
                    let item = p["item"].as_str().unwrap_or("").to_string();
                    let remount = p["remount"].as_bool().unwrap_or(false);
                    let focus = p["focus"].as_bool().unwrap_or(false);
                    actor_tx.send(Msg::Show {
                        client, pro: resolved.pro, frame: resolved.frame, doc, item, remount, focus, reply: tx,
                    })
                }
                "editor.kicad.detach" => {
                    actor_tx.send(Msg::Detach { client, pro: resolved.pro, reply: tx })
                }
                "editor.kicad.close" => {
                    let save = p["save"].as_str().unwrap_or("ask").to_string();
                    actor_tx.send(Msg::Close { client, pro: resolved.pro, save, reply: tx })
                }
                "editor.kicad.restart" => {
                    let env = ipc::runtime(hub).await;
                    actor_tx.send(Msg::Restart { client, pro: resolved.pro, env, reply: tx })
                }
                "editor.kicad.frames" => {
                    let frames = p["frames"].as_array().map(|arr| arr.iter().map(|f| {
                        let title = f["title"].as_str().unwrap_or("").to_string();
                        let modified = f["modified"].as_bool().unwrap_or_else(|| title.trim_start().starts_with('*'));
                        Frame {
                            kind: canon_frame(f["kind"].as_str().unwrap_or("")).into(),
                            doc: f["doc"].as_str().unwrap_or("").to_string(),
                            title,
                            modified,
                            shown: f["shown"].as_bool().unwrap_or(false),
                            realized: false,
                            xid: f.get("xid").map(u64_of).unwrap_or(0),
                            min_w: 0,
                            min_h: 0,
                        }
                    }).collect()).unwrap_or_default();
                    actor_tx.send(Msg::ClientFrames { client, pro: resolved.pro, frames, reply: tx })
                }
                _ => return err_reply(id, "unknown method"),
            };
            if sent.is_err() { return err_reply(id, "the embedded KiCad actor stopped") }
            match rx.await {
                Ok(Ok(v)) => ok_reply(id, v),
                Ok(Err(why)) => err_reply(id, &why),
                Err(_) => err_reply(id, "no reply from embedded KiCad actor"),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn pure_frame_and_mode_rules() {
        assert_eq!(canon_frame(""), "PCB");
        assert_eq!(canon_frame("0"), "PCB");
        assert_eq!(canon_frame("1"), "SCH");
        assert_eq!(canon_frame("2"), "PCB_3D");
        assert_eq!(canon_frame("fk_symbol_editor"), "SYMBOL_EDITOR");
        assert_eq!(canon_frame("FOOTPRINT_EDITOR"), "FOOTPRINT_EDITOR");
        assert_eq!(default_frame_for_path("/p/a.kicad_sch", ""), "SCH");
        assert_eq!(default_frame_for_path("/p/a.kicad_pcb", ""), "PCB");
        assert_eq!(default_frame_for_path("/p/a.kicad_pcb", "PCB_3D"), "PCB_3D");
        assert_eq!(fk_of("SCH"), "FK_SCH");
        assert_eq!(prewarm_kinds("PCB", &["PCB".to_string()]), ["SCH"]);
        assert_eq!(prewarm_kinds("SCH", &["SCH".to_string(), "PCB".to_string()]), Vec::<&str>::new());
        assert_eq!(prewarm_kinds("PCB_3D", &[]), ["SCH", "PCB"]);
        assert_eq!(fk_of("PCB_3D"), "FK_PCB_3D");
        assert_eq!(kind_of_fk_val(&json!("FK_FOOTPRINT_EDITOR")), "FOOTPRINT_EDITOR");
        assert_eq!(kind_of_fk_val(&json!(5)), "PCB_3D");
        assert_eq!(close_mode_of("ask"), "CM_FAIL_IF_MODIFIED");
        assert_eq!(close_mode_of("save"), "CM_SAVE");
        assert_eq!(close_mode_of("discard"), "CM_DISCARD");
        assert_eq!(stock_action_for("PCB"), None);
        assert_eq!(stock_action_for("SCH"), Some("pcbnew.EditorControl.showEeschema"));
        assert_eq!(stock_action_for("PCB_3D"), Some("common.Control.show3DViewer"));

        let stock_env = Env { cli: "/usr/bin/kicad-cli".into(), status: "stock".into(), ..Default::default() };
        assert_eq!(mode_with(None, &stock_env, false, None), "stock");
        assert_eq!(mode_with(Some("fork"), &stock_env, false, None), "fork");

        let bp6 = Env { cli: "/k/bin/kicad-cli".into(), version: "10.99.0-bp.6".into(), status: "ok".into(), ..Default::default() };
        assert_eq!(mode_with(None, &bp6, false, None), "stock", "bp.6 predates --embedded");
        assert_eq!(mode_with(None, &bp6, true, None), "fork", "api.desc with kiapi.backplane.embed means fork");

        let bp8 = Env { cli: "/k/bin/kicad-cli".into(), version: "10.99.0-bp.8".into(), status: "ok".into(), ..Default::default() };
        assert_eq!(mode_with(None, &bp8, false, None), "fork");
        assert_eq!(mode_with(Some("stock"), &bp8, true, None), "stock", "explicit env override wins");

        let desc_json = json!({"version": "10.99.0-bp.8", "capabilities": ["api-server"]});
        assert_eq!(mode_with(None, &stock_env, false, Some(&desc_json)), "fork");
    }

    #[test] fn pure_argv_and_session_transitions() {
        let f = Files { pro: "/p/b.kicad_pro".into(), pcb: "/p/b.kicad_pcb".into(), sch: "/p/b.kicad_sch".into(), all: vec![] };
        assert_eq!(stock_argv("/usr/bin/kicad", &f), vec!["/usr/bin/kicad", "/p/b.kicad_pro"], "the project manager: --frame pcb crashes stock 10.0.7");
        assert_eq!(fork_argv("/k/bin/kicad", "/p/b.kicad_pro", "/t/api.sock", 101, 202, 999),
            vec!["/k/bin/kicad", "--embedded", "--parent-window", "101", "--toplevel-window", "202", "--api-socket", "/t/api.sock", "--parent-pid", "999", "/p/b.kicad_pro"]);

        let s = Session::new("/p/b.kicad_pro", "/p", "fork", "SCH", 1, 10).spawned(4321);
        assert_eq!(s.state, "starting");
        assert_eq!(s.pid, 4321);
        assert_eq!(s.active, "SCH");
        let s = s.ready("/t/api.sock", "10.99.0-bp.8", 20);
        assert_eq!(s.state, "ready");
        let s = s.ping_result(false);
        assert_eq!(s.state, "ready", "one missed ping stays ready");
        let s = s.ping_result(false);
        assert_eq!(s.state, "unresponsive", "two missed pings mark unresponsive");
        let s = s.ping_result(true);
        assert_eq!(s.state, "ready", "next good ping recovers");

        let crashed = s.clone().exited(1, "line1\nfatal boom\n");
        assert_eq!(crashed.state, "failed");
        assert!(crashed.reason.contains("fatal boom"), "{}", crashed.reason);

        let clean = s.exited(0, "");
        assert_eq!(clean.state, "stopped");
    }

    #[test] fn warm_eviction_picks_oldest_detached_clean_session() {
        let mut m = HashMap::new();
        let mk = |pro: &str, container: u64, dirty: bool, t: u64| {
            let mut s = Session::new(pro, "/p", "fork", "PCB", 1, t).ready("/s", "10.99.0-bp.8", t);
            s.container = container;
            if dirty {
                s.frames.push(Frame { kind: "PCB".into(), doc: "/p/x.kicad_pcb".into(), title: "*x".into(), modified: true, shown: true, realized: false, xid: 1, min_w: 0, min_h: 0 });
            }
            s
        };
        m.insert("/p/1.kicad_pro".into(), mk("/p/1.kicad_pro", 55, false, 1)); // attached: never evict
        m.insert("/p/2.kicad_pro".into(), mk("/p/2.kicad_pro", 0, true, 2));   // dirty: never evict
        m.insert("/p/3.kicad_pro".into(), mk("/p/3.kicad_pro", 0, false, 5));  // clean detached @ t=5
        m.insert("/p/4.kicad_pro".into(), mk("/p/4.kicad_pro", 0, false, 3));  // clean detached @ t=3 (oldest)
        assert_eq!(evict_warm(&m, "/p/new.kicad_pro", 3).as_deref(), Some("/p/4.kicad_pro"));
        assert_eq!(evict_warm(&m, "/p/new.kicad_pro", 10), None);
    }

    #[test] fn stock_config_seeding_writes_expected_files() {
        let dir = crate::plat::tmp::uncreated("seed");
        seed_stock_config(&dir).unwrap();
        for ver in ["10.0", "10.99"] {
            let c: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("kicad").join(ver).join("kicad_common.json")).unwrap()).unwrap();
            assert_eq!(c["api"]["enable_server"], json!(true));
            assert_eq!(c["system"]["first_run_shown"], json!(true));
            let p: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("kicad").join(ver).join("pcbnew.json")).unwrap()).unwrap();
            assert_eq!(p["appearance"]["canvas_type"], json!(0));
            assert!(dir.join("kicad").join(ver).join("sym-lib-table").is_file());
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stock_and_fork_standins_and_live_ipc_routing() {
        use super::super::nng::tests as stand_in;
        use crate::plat::fs::PermissionsExt;

        let base = crate::plat::tmp::short_dir("ke-t");

        let home_dir = base.join("home");
        let proj_dir = base.join("proj");
        let bin_dir = base.join("bin");
        std::fs::create_dir_all(&home_dir).unwrap();
        std::fs::create_dir_all(&proj_dir).unwrap();
        std::fs::create_dir_all(&bin_dir).unwrap();

        let pro = proj_dir.join("amp.kicad_pro");
        let pcb = proj_dir.join("amp.kicad_pcb");
        let sch = proj_dir.join("amp.kicad_sch");
        std::fs::write(&pro, "{}").unwrap();
        std::fs::write(&pcb, "(kicad_pcb)").unwrap();
        std::fs::write(&sch, "(kicad_sch)").unwrap();

        // Build a minimal Hub registered with `proj_dir` so `known_root`, `set_info`, and `ipc::kicad_ipc` run end-to-end.
        let cfg = crate::hub::Cfg {
            home: home_dir.clone(), version: "0.14.0".into(), commit: "test".into(),
            listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into(),
        };
        let hub = Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
        ipc::start(&hub);
        start(&hub);
        let _ = hub.commit(&json!({"$":"ProjectCreated","id":"p1","title":"amp","root":proj_dir.to_string_lossy(),"at":1})).await;

        let s = proto::builtin();
        let ok_bytes = |ty: &str, body: Value| -> Vec<u8> {
            use prost::Message;
            #[derive(Clone, PartialEq, Message)] struct Any { #[prost(string, tag = "1")] type_url: String, #[prost(bytes = "vec", tag = "2")] value: Vec<u8> }
            #[derive(Clone, PartialEq, Message)] struct ResHeader { #[prost(string, tag = "1")] kicad_token: String }
            #[derive(Clone, PartialEq, Message)] struct ResStatus { #[prost(int32, tag = "1")] status: i32, #[prost(string, tag = "2")] error_message: String }
            #[derive(Clone, PartialEq, Message)] struct ApiResponse { #[prost(message, optional, tag = "1")] header: Option<ResHeader>, #[prost(message, optional, tag = "2")] status: Option<ResStatus>, #[prost(message, optional, tag = "3")] message: Option<Any> }
            let val = if ty.is_empty() { vec![] } else { proto::encode_message(&s.message(ty).unwrap(), &body).unwrap() };
            ApiResponse {
                header: Some(ResHeader { kicad_token: "tok-embed".into() }),
                status: Some(ResStatus { status: 1, error_message: String::new() }),
                message: (!ty.is_empty()).then(|| Any { type_url: format!("type.googleapis.com/{ty}"), value: val }),
            }.encode_to_vec()
        };
        let busy_bytes = |msg: &str| -> Vec<u8> {
            use prost::Message;
            #[derive(Clone, PartialEq, Message)] struct ResHeader { #[prost(string, tag = "1")] kicad_token: String }
            #[derive(Clone, PartialEq, Message)] struct ResStatus { #[prost(int32, tag = "1")] status: i32, #[prost(string, tag = "2")] error_message: String }
            #[derive(Clone, PartialEq, Message)] struct ApiResponse { #[prost(message, optional, tag = "1")] header: Option<ResHeader>, #[prost(message, optional, tag = "2")] status: Option<ResStatus> }
            ApiResponse {
                header: Some(ResHeader { kicad_token: "tok-embed".into() }),
                status: Some(ResStatus { status: 7 /* AS_BUSY */, error_message: msg.into() }),
            }.encode_to_vec()
        };

        // --- 1. Fork mode stand-in ---
        let cli_path = bin_dir.join("kicad-cli");
        let gui_path = bin_dir.join("kicad");
        std::fs::write(&cli_path, "#!/bin/sh\nif [ \"$1\" = \"api-server\" ] && [ \"$2\" = \"--describe\" ]; then\n  echo '{\"version\":\"10.99.0-bp.8\",\"capabilities\":[\"api-server\"]}'\n  exit 0\nfi\necho '10.99.0'\n").unwrap();
        std::fs::set_permissions(&cli_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        // Stand-in `kicad` records the `--api-socket` path in `sock_file` and sleeps until closed.
        let sock_notice = base.join("fork_sock_path");
        std::fs::write(&gui_path, format!(
            "#!/bin/sh\nsock=\"\"\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"--api-socket\" ]; then sock=\"$2\"; shift 2; continue; fi\n  shift\ndone\nprintf '%s' \"$sock\" > '{}'\nwhile [ ! -S \"$sock\" ]; do sleep 0.02; done\necho \"KiCad embed events at ipc://$sock.events\"\necho \"KiCad API server listening at ipc://$sock\"\nsleep 30\n",
            sock_notice.display()
        )).unwrap();
        std::fs::set_permissions(&gui_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = hub.commit(&json!({"$":"SettingSet","key":"kicad.path","value":cli_path.to_string_lossy(),"at":2})).await;

        let dialog_open = Arc::new(AtomicBool::new(false));
        let dialog_srv = dialog_open.clone();
        let pro_str = pro.to_string_lossy().to_string();
        let pcb_str = pcb.to_string_lossy().to_string();
        let sch_str = sch.to_string_lossy().to_string();

        // Background fake nng server binding `<sock>` and `<sock>.events` as soon as the stand-in writes `sock_notice`.
        let srv_task = {
            let sock_notice = sock_notice.clone();
            let pcb_str = pcb_str.clone();
            let sch_str = sch_str.clone();
            let ok_ping = ok_bytes("", json!({}));
            let ok_ver = ok_bytes("kiapi.common.commands.GetVersionResponse", json!({"version":{"major":10,"minor":99,"full_version":"10.99.0-bp.8"}}));
            let ok_frames_pcb = ok_bytes("kiapi.backplane.embed.GetFramesResponse", json!({
                "frames": [{"kind":"FK_PCB","document":pcb_str,"title":"amp — PCB","modified":false,"window":7001,"min_width":320,"min_height":240,"shown":true}],
                "holder": 5001,
            }));
            let ok_frames_3d = ok_bytes("kiapi.backplane.embed.GetFramesResponse", json!({
                "frames": [
                    {"kind":"FK_PCB","document":pcb_str,"title":"amp — PCB","modified":false,"window":7001,"shown":false},
                    {"kind":"FK_PCB_3D","document":pcb_str,"title":"amp — 3D","modified":false,"window":7005,"shown":true}
                ],
                "holder": 5001,
            }));
            let ok_show = ok_bytes("kiapi.backplane.embed.ShowFrameResponse", json!({"frame":{"kind":"FK_PCB","window":7001,"shown":true}}));
            let ok_attach = ok_bytes("kiapi.backplane.embed.AttachToParentResponse", json!({"holder": 0, "frames": []}));
            let ok_run = ok_bytes("kiapi.common.commands.RunActionResponse", json!({"status":"RAS_OK"}));
            let ok_close = ok_bytes("kiapi.backplane.embed.CloseResponse", json!({"exiting":true}));
            let busy_rep = busy_bytes("KiCad is showing the dialog 'Track Width'; only reads are served until it is closed");
            let ev_show_3d = ok_bytes("kiapi.backplane.embed.NextEventsResponse", json!({
                "events": [{"seq": 1, "type": "ET_SHOW_REQUESTED", "frame": {"kind": "FK_PCB_3D", "window": 7005}}],
                "next": 1, "lost": false
            }));
            let ev_empty = ok_bytes("kiapi.backplane.embed.NextEventsResponse", json!({"events": [], "next": 1, "lost": false}));

            tokio::spawn(async move {
                let sock_path = loop {
                    if let Ok(t) = std::fs::read_to_string(&sock_notice) {
                        if !t.is_empty() { break t }
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                };
                let ev_path = format!("{sock_path}.events");
                let l_main = crate::plat::ipc::bind(&sock_path).unwrap();
                let l_ev = crate::plat::ipc::bind(&ev_path).unwrap();

                let ev_srv = tokio::spawn(async move {
                    let (mut es, _) = stand_in::accept(&l_ev).await;
                    // First NextEvents triggers ET_SHOW_REQUESTED for FK_PCB_3D after a brief pause!
                    tokio::time::sleep(Duration::from_millis(120)).await;
                    if let Some((id, _)) = stand_in::read_req(&mut es).await {
                        stand_in::write_rep(&mut es, id, &ev_show_3d).await;
                    }
                    while let Some((id, _)) = stand_in::read_req(&mut es).await {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        stand_in::write_rep(&mut es, id, &ev_empty).await;
                    }
                });

                let mut saw_3d = false;
                let (mut ms, _) = stand_in::accept(&l_main).await;
                while let Some((id, req)) = stand_in::read_req(&mut ms).await {
                    let txt = String::from_utf8_lossy(&req);
                    if txt.contains("commands.Ping") {
                        stand_in::write_rep(&mut ms, id, &ok_ping).await;
                    } else if txt.contains("commands.GetVersion") {
                        stand_in::write_rep(&mut ms, id, &ok_ver).await;
                    } else if txt.contains("embed.GetFrames") {
                        stand_in::write_rep(&mut ms, id, if saw_3d { &ok_frames_3d } else { &ok_frames_pcb }).await;
                    } else if txt.contains("embed.ShowFrame") {
                        if req.ends_with(&[0x08, 0x05]) || txt.contains("FK_PCB_3D") || saw_3d { saw_3d = true; }
                        // Detect FK_PCB_3D enum value 5 (`08 05` in ShowFrame body)
                        if req.windows(2).any(|w| w == [0x08, 0x05]) { saw_3d = true; }
                        stand_in::write_rep(&mut ms, id, &ok_show).await;
                    } else if txt.contains("embed.AttachToParent") {
                        stand_in::write_rep(&mut ms, id, &ok_attach).await;
                    } else if txt.contains("commands.RunAction") {
                        if dialog_srv.load(Ordering::SeqCst) {
                            stand_in::write_rep(&mut ms, id, &busy_rep).await;
                        } else {
                            stand_in::write_rep(&mut ms, id, &ok_run).await;
                        }
                    } else if txt.contains("embed.Close") {
                        stand_in::write_rep(&mut ms, id, &ok_close).await;
                        break;
                    } else {
                        stand_in::write_rep(&mut ms, id, &ok_ping).await;
                    }
                }
                ev_srv.abort();
                let _ = sch_str;
            })
        };

        let (mut conn, _) = hub.connect(true);
        let rep = hub.request(&mut conn, &json!({
            "id": 1, "m": "editor.kicad.attach",
            "p": { "project": pcb_str, "container": 9001, "toplevel": 8001, "display": ":99", "frame": "PCB" }
        })).await;
        assert_eq!(rep[0]["ok"], json!(true), "{rep:?}");
        assert_eq!(rep[0]["mode"], json!("fork"));
        assert_eq!(rep[0]["state"], json!("ready"));
        assert_eq!(rep[0]["holder"], json!(5001));
        assert!(rep[0]["pid"].as_u64().unwrap_or(0) > 0);
        assert_eq!(rep[0]["frames"][0]["xid"], json!(7001));

        // Verify `editor.kicad.<digest>` is published on the hub and switches `active` to `PCB_3D` on ET_SHOW_REQUESTED!
        let key = info_key(&pro_str);
        for _ in 0..30 {
            let cur = hub.info_now()[&key].as_str().unwrap_or("").to_string();
            if cur.contains("\"active\":\"PCB_3D\"") { break }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let info_val: Value = serde_json::from_str(hub.info_now()[&key].as_str().unwrap()).unwrap();
        assert_eq!(info_val["active"], json!("PCB_3D"), "{info_val}");

        // Live `kicad_ipc` routes GUI-only `RunAction` to the embedded GUI socket (normally refused on headless kserve!).
        let ipc_ok = ipc::kicad_ipc(&hub, &pcb_str, "kiapi.common.commands.RunAction", json!({"action":"pcbnew.InteractiveSelection.ClearSelection"})).await.unwrap();
        assert_eq!(ipc_ok["message"]["status"], json!("RAS_OK"));

        // Modal dialog `AS_BUSY` passes straight back to the agent!
        dialog_open.store(true, Ordering::SeqCst);
        let ipc_busy = ipc::kicad_ipc(&hub, &pcb_str, "kiapi.common.commands.RunAction", json!({"action":"pcbnew.InteractiveSelection.ClearSelection"})).await.unwrap_err();
        assert!(ipc_busy.contains("AS_BUSY") && ipc_busy.contains("Track Width"), "{ipc_busy}");
        dialog_open.store(false, Ordering::SeqCst);

        // Detach parks via AttachToParent{window:0}, then Close shuts down cleanly.
        let det = hub.request(&mut conn, &json!({"id": 2, "m": "editor.kicad.detach", "p": {"project": pro_str}})).await;
        assert_eq!(det[0]["ok"], json!(true));
        assert_eq!(det[0]["container"], json!(0));

        let cls = hub.request(&mut conn, &json!({"id": 3, "m": "editor.kicad.close", "p": {"project": pro_str, "save": "discard"}})).await;
        assert_eq!(cls[0]["ok"], json!(true));
        assert_eq!(cls[0]["closed"], json!(true));
        let _ = tokio::time::timeout(Duration::from_secs(5), srv_task).await;

        // --- 2. Stock mode stand-in (`--describe` fails -> mode == "stock", `pid` published in "starting", `sockets_at` discovery) ---
        let stock_dir = base.join("bin_stock");
        std::fs::create_dir_all(&stock_dir).unwrap();
        let stock_cli = stock_dir.join("kicad-cli");
        let stock_gui = stock_dir.join("kicad");
        std::fs::write(&stock_cli, "#!/bin/sh\nif [ \"$1\" = \"api-server\" ]; then exit 2; fi\necho '10.0.7'\n").unwrap();
        std::fs::set_permissions(&stock_cli, std::fs::Permissions::from_mode(0o755)).unwrap();

        let stock_tmp_notice = base.join("stock_tmp_path");
        std::fs::write(&stock_gui, format!(
            "#!/bin/sh\nprintf '%s' \"$TMPDIR\" > '{}'\nsleep 30\n",
            stock_tmp_notice.display()
        )).unwrap();
        std::fs::set_permissions(&stock_gui, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = hub.commit(&json!({"$":"SettingSet","key":"kicad.path","value":stock_cli.to_string_lossy(),"at":3})).await;

        let stock_actions = Arc::new(Mutex::new(Vec::<String>::new()));
        let stock_actions_srv = stock_actions.clone();
        let stock_srv = {
            let ok_ping = ok_bytes("", json!({}));
            let ok_ver = ok_bytes("kiapi.common.commands.GetVersionResponse", json!({"version":{"major":10,"minor":0,"patch":7,"full_version":"10.0.7"}}));
            let ok_run = ok_bytes("kiapi.common.commands.RunActionResponse", json!({"status":"RAS_OK"}));
            let ok_docs_pcb = ok_bytes("kiapi.common.commands.GetOpenDocumentsResponse", json!({
                "documents": [{"type":"DOCTYPE_PCB","board_filename":"amp.kicad_pcb","project":{"name":"amp","path":proj_dir.to_string_lossy()}}]
            }));
            let ok_docs_empty = ok_bytes("kiapi.common.commands.GetOpenDocumentsResponse", json!({"documents": []}));
            let ok_dirty = ok_bytes("kiapi.common.commands.GetDocumentModifiedStateResponse", json!({"state":"DMS_MODIFIED"}));
            let hub_chk = hub.clone();
            let key_chk = key.clone();
            tokio::spawn(async move {
                let tmpd = loop {
                    if let Ok(t) = std::fs::read_to_string(&stock_tmp_notice) {
                        if !t.is_empty() { break t }
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                };
                // Before we even bind `api.sock`, verify `editor.kicad.<digest>` is ALREADY in `"starting"` with `pid > 0`!
                for _ in 0..20 {
                    if let Some(raw) = hub_chk.info_now()[&key_chk].as_str() {
                        if let Ok(v) = serde_json::from_str::<Value>(raw) {
                            if v["state"] == json!("starting") && v["pid"].as_u64().unwrap_or(0) > 0 { break }
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                let starting_info: Value = serde_json::from_str(hub_chk.info_now()[&key_chk].as_str().unwrap()).unwrap();
                assert_eq!(starting_info["state"], json!("starting"), "{starting_info}");
                assert_eq!(starting_info["mode"], json!("stock"), "{starting_info}");
                assert!(starting_info["pid"].as_u64().unwrap_or(0) > 0, "stock client needs pid while starting: {starting_info}");

                let sock_path = format!("{tmpd}/kicad/api.sock");
                let l = crate::plat::ipc::bind(&sock_path).unwrap();
                let (mut ms, _) = stand_in::accept(&l).await;
                let mut saved = false;
                while let Some((id, req)) = stand_in::read_req(&mut ms).await {
                    let txt = String::from_utf8_lossy(&req);
                    if txt.contains("commands.Ping") {
                        stand_in::write_rep(&mut ms, id, &ok_ping).await;
                    } else if txt.contains("commands.GetVersion") {
                        stand_in::write_rep(&mut ms, id, &ok_ver).await;
                    } else if txt.contains("commands.RunAction") {
                        stock_actions_srv.lock().unwrap().push(txt.into_owned());
                        stand_in::write_rep(&mut ms, id, &ok_run).await;
                    } else if txt.contains("commands.GetOpenDocuments") {
                        // DOCTYPE_PCB = 3 (`08 03` at end of body)
                        if req.ends_with(&[0x08, 0x03]) && !saved {
                            stand_in::write_rep(&mut ms, id, &ok_docs_pcb).await;
                        } else {
                            stand_in::write_rep(&mut ms, id, &ok_docs_empty).await;
                        }
                    } else if txt.contains("commands.GetDocumentModifiedState") {
                        stand_in::write_rep(&mut ms, id, &ok_dirty).await;
                    } else if txt.contains("commands.SaveDocument") {
                        saved = true;
                        stand_in::write_rep(&mut ms, id, &ok_ping).await;
                    } else {
                        stand_in::write_rep(&mut ms, id, &ok_ping).await;
                    }
                }
            })
        };

        // Attach via project root directory with `wait: false` (client path: returns `"starting"` immediately so the WS loop never starves `outbox`!).
        let (mut conn2, _) = hub.connect(true);
        let mut out2 = hub.attach(conn2.client);
        let rep_s = hub.request(&mut conn2, &json!({
            "id": 10, "m": "editor.kicad.attach",
            "p": { "project": proj_dir.to_string_lossy(), "container": 9100, "toplevel": 8100, "display": ":99", "frame": "PCB", "wait": false }
        })).await;
        assert_eq!(rep_s[0]["ok"], json!(true), "{rep_s:?}");
        assert_eq!(rep_s[0]["mode"], json!("stock"));
        assert!(matches!(rep_s[0]["state"].as_str(), Some("starting" | "ready")), "{rep_s:?}");

        // Stream info frames from `out2` until `"state": "ready"` arrives on the connection's outbox!
        let mut saw_ready = rep_s[0]["state"] == json!("ready");
        while !saw_ready {
            let out_item = tokio::time::timeout(Duration::from_secs(5), out2.recv()).await.expect("outbox ready timeout").expect("outbox closed");
            if let crate::hub::Out::Value(ev) = out_item {
                if let Some(raw) = ev["info"][&key].as_str() {
                    if let Ok(iv) = serde_json::from_str::<Value>(raw) {
                        if iv["state"] == json!("ready") { saw_ready = true; }
                    }
                }
            }
        }
        assert!(saw_ready);

        // Switch frame to SCH in stock mode -> sends RunAction("pcbnew.EditorControl.showEeschema")
        let shw_s = hub.request(&mut conn, &json!({
            "id": 11, "m": "editor.kicad.show", "p": { "project": pcb_str, "frame": "SCH" }
        })).await;
        assert_eq!(shw_s[0]["ok"], json!(true), "{shw_s:?}");
        assert!(stock_actions.lock().unwrap().iter().any(|a| a.contains("showEeschema")), "{:?}", stock_actions.lock().unwrap());

        // Close with save="ask" refuses while document is modified, then save="save" saves and closes!
        let ask_s = hub.request(&mut conn, &json!({
            "id": 12, "m": "editor.kicad.close", "p": { "project": pro_str, "save": "ask" }
        })).await;
        assert_eq!(ask_s[0]["ok"], json!(true));
        assert_eq!(ask_s[0]["closed"], json!(false), "{ask_s:?}");
        assert_eq!(ask_s[0]["unsaved"], json!(["amp.kicad_pcb"]));

        let sav_s = hub.request(&mut conn, &json!({
            "id": 13, "m": "editor.kicad.close", "p": { "project": pro_str, "save": "save" }
        })).await;
        assert_eq!(sav_s[0]["ok"], json!(true), "{sav_s:?}");
        assert_eq!(sav_s[0]["closed"], json!(true));
        let _ = tokio::time::timeout(Duration::from_secs(5), stock_srv).await;

        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn real_stock_kicad_argv_starts_without_crashing_and_binds_ipc() {
        let stock_cli = Path::new("/usr/bin/kicad-cli");
        let stock_gui = Path::new("/usr/bin/kicad");
        if !stock_cli.is_file() || !stock_gui.is_file() {
            eprintln!("skipping real_stock_kicad_argv test: /usr/bin/kicad(-cli) not installed");
            return;
        }
        let user_home = crate::plat::dirs::home_or_empty();
        let mut xvfb_bin = PathBuf::new();
        let mut xvfb_ld = String::new();
        for root in [format!("{user_home}/.local/xvfb-root"), format!("{user_home}/.cache/xvfb-root")] {
            let b = PathBuf::from(&root).join("usr/bin/Xvfb");
            if b.is_file() {
                xvfb_bin = b;
                xvfb_ld = format!("{root}/usr/lib/x86_64-linux-gnu");
                break;
            }
        }
        if !xvfb_bin.is_file() {
            for d in std::env::var("PATH").unwrap_or_default().split(':') {
                let b = PathBuf::from(d).join("Xvfb");
                if b.is_file() { xvfb_bin = b; break; }
            }
        }
        if !xvfb_bin.is_file() {
            eprintln!("skipping real_stock_kicad_argv test: Xvfb not available");
            return;
        }

        let Some(disp_num) = (190..290).find(|n| {
            !Path::new(&format!("/tmp/.X11-unix/X{n}")).exists() && !Path::new(&format!("/tmp/.X{n}-lock")).exists()
        }) else {
            eprintln!("skipping real_stock_kicad_argv test: no free X display");
            return;
        };
        let disp = format!(":{disp_num}");

        let base = crate::plat::tmp::short_dir("kr");
        let home_dir = base.join("h");
        let proj_dir = base.join("p");
        std::fs::create_dir_all(&home_dir).unwrap();
        std::fs::create_dir_all(&proj_dir).unwrap();

        let demo_dir = Path::new("/usr/share/kicad/demos/ecc83");
        if demo_dir.join("ecc83-pp.kicad_pro").is_file() {
            for ext in ["kicad_pro", "kicad_pcb", "kicad_sch"] {
                std::fs::copy(demo_dir.join(format!("ecc83-pp.{ext}")), proj_dir.join(format!("ecc83-pp.{ext}"))).unwrap();
            }
        } else {
            std::fs::write(proj_dir.join("ecc83-pp.kicad_pro"), "{\"meta\":{\"filename\":\"ecc83-pp.kicad_pro\",\"version\":3}}").unwrap();
            std::fs::write(proj_dir.join("ecc83-pp.kicad_pcb"), "(kicad_pcb (version 20240108) (generator \"pcbnew\"))").unwrap();
            std::fs::write(proj_dir.join("ecc83-pp.kicad_sch"), "(kicad_sch (version 20231120) (generator \"eeschema\"))").unwrap();
        }
        let files = project::read(&proj_dir.to_string_lossy());
        let argv = stock_argv("/usr/bin/kicad", &files);
        assert_eq!(argv, vec!["/usr/bin/kicad".to_string(), files.pro.clone()], "stock_argv must never pass --frame (crashes 10.0.7 with exit 139)");

        let mut xvfb_cmd = crate::proc::std_cmd(crate::proc::Kind::Local, &xvfb_bin);
        xvfb_cmd.args([&disp, "-screen", "0", "1280x800x24", "-nolisten", "tcp", "-xkbdir", "/usr/share/X11/xkb"])
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if !xvfb_ld.is_empty() { xvfb_cmd.env("LD_LIBRARY_PATH", &xvfb_ld); }
        let mut xvfb = xvfb_cmd.spawn().expect("spawn Xvfb");
        let xvfb_pid = xvfb.id();

        struct Guard { xvfb_pid: u32, kicad_pid: Arc<Mutex<u32>> }
        impl Drop for Guard {
            fn drop(&mut self) {
                let kp = *self.kicad_pid.lock().unwrap();
                if kp > 0 { Actor::kill_pid(kp); }
                if self.xvfb_pid > 0 { let _ = crate::plat::proc::kill(self.xvfb_pid, crate::plat::proc::TERM); }
            }
        }
        let kicad_pid = Arc::new(Mutex::new(0u32));
        let _guard = Guard { xvfb_pid, kicad_pid: kicad_pid.clone() };

        for _ in 0..40 {
            if Path::new(&format!("/tmp/.X11-unix/X{disp_num}")).exists() { break }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let cfg = crate::hub::Cfg {
            home: home_dir.clone(), version: "0.14.0".into(), commit: "test".into(),
            listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into(),
        };
        let hub = Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
        ipc::start(&hub);
        start(&hub);
        let _ = hub.commit(&json!({"$":"ProjectCreated","id":"p1","title":"ecc83","root":proj_dir.to_string_lossy(),"at":1})).await;
        let _ = hub.commit(&json!({"$":"SettingSet","key":"kicad.path","value":"/usr/bin/kicad-cli","at":2})).await;

        let (mut conn, _) = hub.connect(true);
        let rep = hub.request(&mut conn, &json!({
            "id": 1, "m": "editor.kicad.attach",
            "p": { "project": files.pcb, "container": 0, "toplevel": 0, "display": disp, "frame": "PCB" }
        })).await;
        assert_eq!(rep[0]["ok"], json!(true), "real stock KiCad launch failed (did stock_argv crash?): {rep:?}");
        assert_eq!(rep[0]["mode"], json!("stock"));
        assert_eq!(rep[0]["state"], json!("ready"));
        let pid = rep[0]["pid"].as_u64().unwrap_or(0) as u32;
        assert!(pid > 0 && crate::plat::proc::alive(pid), "real stock KiCad pid={pid} must be alive");
        *kicad_pid.lock().unwrap() = pid;
        let sock = rep[0]["sock"].as_str().unwrap_or("");
        assert!(!sock.is_empty() && Path::new(sock).exists(), "real stock KiCad must bind IPC socket: {sock}");

        let cls = hub.request(&mut conn, &json!({"id": 2, "m": "editor.kicad.close", "p": {"project": files.pro, "save": "discard"}})).await;
        assert_eq!(cls[0]["ok"], json!(true), "{cls:?}");
        for _ in 0..30 {
            if !crate::plat::proc::alive(pid) { break }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(!crate::plat::proc::alive(pid), "stock KiCad pid={pid} should exit after close");
        *kicad_pid.lock().unwrap() = 0;
        let _ = xvfb.kill();
        let _ = xvfb.wait();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stock_kicad_hub_exit_probe_child() {
        use std::os::unix::fs::PermissionsExt;
        let Ok(raw) = std::env::var("BP_KICAD_EXIT_PROBE") else { return };
        let base = PathBuf::from(raw);
        crate::plat::hub::exit_on_signals(super::shutdown_all);
        let home_dir = base.join("h");
        let proj_dir = base.join("p");
        let bin_dir = base.join("bin");
        std::fs::create_dir_all(&home_dir).unwrap();
        std::fs::create_dir_all(&proj_dir).unwrap();
        std::fs::create_dir_all(&bin_dir).unwrap();
        let pro = proj_dir.join("amp.kicad_pro");
        let pcb = proj_dir.join("amp.kicad_pcb");
        std::fs::write(&pro, "{\"meta\":{\"filename\":\"amp.kicad_pro\",\"version\":3}}").unwrap();
        std::fs::write(&pcb, "(kicad_pcb (version 20240108) (generator \"pcbnew\"))").unwrap();

        let cli = bin_dir.join("kicad-cli");
        let gui = bin_dir.join("kicad");
        let sub_pid_file = base.join("sub_pid");
        std::fs::write(&cli, "#!/bin/sh\nif [ \"$1\" = \"api-server\" ]; then exit 2; fi\necho '10.0.7'\n").unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(&gui, format!(
            "#!/bin/sh\nsleep 120 &\nprintf '%s' \"$!\" > '{}'\nexec sleep 120\n",
            sub_pid_file.display()
        )).unwrap();
        std::fs::set_permissions(&gui, std::fs::Permissions::from_mode(0o755)).unwrap();

        let cfg = crate::hub::Cfg {
            home: home_dir, version: "0.15.0".into(), commit: "test".into(),
            listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into(),
        };
        let hub = Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
        ipc::start(&hub);
        start(&hub);
        let _ = hub.commit(&json!({"$":"ProjectCreated","id":"p1","title":"amp","root":proj_dir.to_string_lossy(),"at":1})).await;
        let _ = hub.commit(&json!({"$":"SettingSet","key":"kicad.path","value":cli.to_string_lossy(),"at":2})).await;

        let (mut conn, _) = hub.connect(true);
        let rep = hub.request(&mut conn, &json!({
            "id": 1, "m": "editor.kicad.attach",
            "p": { "project": pcb.to_string_lossy(), "container": 0, "toplevel": 0, "display": ":99", "frame": "PCB", "wait": false }
        })).await;
        assert_eq!(rep[0]["ok"], json!(true), "{rep:?}");
        let key = info_key(&pro.to_string_lossy());
        let mut kpid = 0u32;
        for _ in 0..80 {
            if let Some(txt) = hub.info_now()[&key].as_str() {
                if let Ok(v) = serde_json::from_str::<Value>(txt) {
                    if let Some(p) = v["pid"].as_u64().filter(|p| *p > 0) {
                        kpid = p as u32;
                        if sub_pid_file.is_file() { break }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(kpid > 0, "stock KiCad stand-in did not publish pid");
        let sub_pid: u32 = std::fs::read_to_string(&sub_pid_file).unwrap_or_default().trim().parse().unwrap_or(0);
        assert!(sub_pid > 0, "stock KiCad stand-in sub-child pid missing");
        std::fs::write(base.join("ready"), format!("{kpid} {sub_pid}\n")).unwrap();
        std::future::pending::<()>().await;
    }

    #[cfg(unix)]
    #[test]
    fn stock_kicad_ends_when_hub_exits_on_sigterm() {
        use std::time::Instant;
        let base = crate::plat::tmp::short_dir("kexit");
        let ready = base.join("ready");
        let mut hub_child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["files::kicad::embed::tests::stock_kicad_hub_exit_probe_child", "--exact", "--nocapture", "--test-threads=1"])
            .env("BP_KICAD_EXIT_PROBE", base.path())
            .env_remove("BACKPLANE_COPY")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn probe hub");
        let hub_pid = hub_child.id();
        let t0 = Instant::now();
        while !ready.is_file() && t0.elapsed() < Duration::from_secs(15) {
            std::thread::sleep(Duration::from_millis(25));
        }
        let text = match std::fs::read_to_string(&ready) {
            Ok(t) => t,
            Err(_) => {
                let _ = hub_child.kill();
                let out = hub_child.wait_with_output().unwrap();
                panic!("probe hub did not reach ready: {}", String::from_utf8_lossy(&out.stderr));
            }
        };
        let nums: Vec<u32> = text.split_whitespace().filter_map(|s| s.parse().ok()).collect();
        let (kpid, sub_pid) = (nums[0], nums[1]);
        assert!(crate::plat::proc::alive(kpid) && crate::plat::proc::alive(sub_pid), "stock KiCad and sub-child should be alive before SIGTERM");

        assert!(crate::plat::proc::kill(hub_pid, crate::plat::proc::TERM), "send SIGTERM to probe hub");
        let status = hub_child.wait().expect("wait probe hub");
        assert_eq!(status.code(), Some(143), "probe hub should exit with 128 + SIGTERM (143): {status:?}");

        let t1 = Instant::now();
        while (crate::plat::proc::alive(kpid) || crate::plat::proc::alive(sub_pid)) && t1.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(25));
        }
        let k_alive = crate::plat::proc::alive(kpid);
        let s_alive = crate::plat::proc::alive(sub_pid);
        if k_alive { let _ = crate::plat::proc::kill_group(kpid, crate::plat::proc::KILL); }
        if s_alive { let _ = crate::plat::proc::kill(sub_pid, crate::plat::proc::KILL); }
        let _ = std::fs::remove_dir_all(&base);
        assert!(!k_alive, "stock KiCad pid={kpid} survived hub SIGTERM");
        assert!(!s_alive, "stock KiCad process-group child pid={sub_pid} survived hub SIGTERM");
    }

#[cfg(test)]
mod theme_tests {
    #[test] fn backplanes_theme_names_the_forks_mode() { assert_eq!(super::theme_mode("light"), "TM_LIGHT"); assert_eq!(super::theme_mode("dark"), "TM_DARK"); assert_eq!(super::theme_mode("auto"), ""); }
    #[test] fn windows_pro_key_normalizes_separators_and_drive_case() {
        assert_eq!(super::norm_pro_key(r"C:\bp\proj\a.kicad_pro"), super::norm_pro_key("c:/bp/proj/a.kicad_pro"));
        assert_eq!(super::norm_pro_key(r"C:\bp\proj/a.kicad_pro"), "c:/bp/proj/a.kicad_pro");
    }
}

#[cfg(test)]
mod unsaved_tests {
    use super::Unsaved;
    #[test] fn an_agent_save_never_writes_the_persons_unsaved_edits() {
        let clean = Unsaved::default();
        // only the agent's commits: it may save
        let a = clean.agent_begins().agent_committed(); assert_eq!(a.refuse_save("a.kicad_pcb"), "");
        // the person clicked into KiCad and the board is modified: refused, whoever made the change
        let p = a.touched(); assert!(p.refuse_save("a.kicad_pcb").starts_with("You have unsaved edits in KiCad (a.kicad_pcb)"));
        // edits pending before the agent's first commit are the person's
        let pending = clean.seen(true).agent_begins().agent_committed(); assert!(!pending.refuse_save("b").is_empty());
        // a touch without changes refuses nothing; a save (nothing modified) clears it all
        assert_eq!(clean.touched().refuse_save("c"), ""); assert_eq!(p.seen(false), Unsaved::default());
    }
}

}

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
    /// KiCad's open dialogs (titles, innermost last), from ET_DIALOG_OPENED/CLOSED: a modal KiCad raised by itself (a load-time notice on a
    /// pre-warmed frame) is said in the pane instead of a silent "Starting KiCad…"
    pub dialogs: Vec<String>,
    /// KiCad's resident memory, MB (Linux, sampled on the health tick; 0 unknown): the warm policy's measure of what a KiCad costs
    pub rss_mb: u64,
    /// in no pane, its frames left in the client's kept (unmapped) container: as good as detached for the warm cap and idleness
    pub parked: bool,
    pub touched_at: u64,
    pub closing: bool,
    /// KiCad said it lost its parent (ET_DETACHED) since the last attach: the next attach re-parents even into a container of the same id
    pub lost_parent: bool,
    /// the library editor and item last shown on KiCad's ET_EDITOR_REQUESTED: the same request again is not shown again (a ShowFrame on
    /// the item makes KiCad say ET_EDITOR_REQUESTED once more: shown each time, it looped, hundreds a second)
    pub editor_req: (String, String, Option<std::time::Instant>),
}

impl Session {
    pub fn new(pro: &str, root: &str, mode: &str, active: &str, gen: u64, now: u64) -> Self {
        Session {
            pro: pro.into(), root: root.into(), mode: mode.into(), state: "starting".into(),
            reason: String::new(), pid: 0, gen, active: canon_frame(active).into(),
            container: 0, toplevel: 0, holder: 0, display: String::new(),
            sock: String::new(), events_sock: String::new(), sock_dir: String::new(),
            argv: String::new(), cli: String::new(), version: String::new(), desc: String::new(),
            caps: vec![], frames: vec![], missed_pings: 0, dialogs: vec![], rss_mb: 0, parked: false, touched_at: now, closing: false, lost_parent: false, editor_req: (String::new(), String::new(), None),
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
        // a dialog of KiCad's own is open (a load-time notice, a question): its modal loop may leave a ping unanswered; KiCad is asking, not hung
        if !ok && !self.dialogs.is_empty() { return self }
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
    /// A dialog opened (`open`) or closed in KiCad, by its title.
    pub fn dialog(mut self, open: bool, title: &str) -> Self {
        if open { self.dialogs.push(title.to_string()); }
        else if let Some(i) = self.dialogs.iter().rposition(|d| d == title) { self.dialogs.remove(i); }
        else { self.dialogs.pop(); }
        self
    }
    pub fn detached(mut self, now: u64) -> Self {
        self.container = 0; self.touched_at = now; self
    }
    pub fn failed(mut self, why: &str) -> Self {
        self.state = "failed".into(); self.reason = why.into(); self.pid = 0; self.holder = 0;
        self.frames.clear(); self.dialogs.clear(); self
    }
    pub fn stopped(mut self, reason: &str) -> Self {
        self.state = "stopped".into(); self.reason = reason.into(); self.pid = 0; self.holder = 0;
        self.frames.clear(); self.dialogs.clear(); self
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
            "dialogs": self.dialogs,
            "rss_mb": self.rss_mb,
            // what the embedded editor can do beyond the basics (embed-gerbview, embed-editor-requested, ...): the client gates on these
            "embed_caps": self.caps.iter().filter(|c| c.starts_with("embed-")).collect::<Vec<_>>(),
            // bp.9: the client may map and unmap a frame KiCad has shown once (its tab switches need no round trip)
            "local_map": self.mode == "fork" && !self.desc.is_empty() && proto::load(&self.desc).has_field("kiapi.backplane.embed.ShowFrame", "prewarm"),
        })
    }
}

/// Whether a show of the session's `active` frame is the client's own map (bp.9+ local map, the frame realized), with nothing else asked
/// (`plain`: no document, item or remount).
pub fn local_switch(s: &Session, local_map: bool, plain: bool) -> bool {
    plain && local_map && s.mode == "fork" && s.state == "ready"
        && s.frames.iter().any(|f| f.kind == s.active && f.xid > 0 && (f.realized || f.shown))
}

/// What the hub waits for after KiCad's first ShowFrame at start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstShow {
    /// bp.13 (`embed-frame-drawn`): ET_FRAME_DRAWN for that frame, up to this many ms (the 3D viewer never says it: the cap)
    Drawn(u64),
    /// older forks: wxGTK's 1000 ms frame-extents timer, then HideFrame and ShowFrame again (its first Show was deferred)
    Sleep(u64),
    /// nothing (`BACKPLANE_KICAD_DECOR_SLEEP=0`: the client's remount re-shows a frame left unmapped)
    None,
}
pub fn first_show_wait(caps: &[String], decor_env: Option<&str>) -> FirstShow {
    if caps.iter().any(|c| c == "embed-frame-drawn") { return FirstShow::Drawn(1500) }
    match decor_env { Some("0") => FirstShow::None, _ => FirstShow::Sleep(1150) }
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
        // bp.13: GerbView runs in KiCad's process and embeds like the editors
        "GERBVIEW" | "FK_GERBVIEW" => "GERBVIEW",
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

/// The embedded editor's capabilities from its GetCapabilities answer: the names it says, and for a build that says only its commands
/// (bp.13 77a6ff17d0, before the names came) the ones its commands show.
pub fn embed_caps(msg: &Value) -> Vec<String> {
    let strs = |k: &str| msg[k].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect::<Vec<_>>();
    let mut caps = strs("capabilities");
    let cmds = strs("commands");
    let has = |c: &str| cmds.iter().any(|x| x == c || x.rsplit('.').next() == Some(c));
    if has("OpenGerbers") && !caps.iter().any(|c| c == "embed-gerbview") { caps.push("embed-gerbview".into()); }
    caps
}

/// Shows KiCad asks for (ET_SHOW_REQUESTED) while the hub itself is driving it are not the person's: a check run in the editor (ERC opens the
/// schematic editor, and KiCad asks to show it) moved the pane to the schematic after a warm Edit on Hubble (lanes A, E). Until the instant
/// stored, a show request for a frame other than the active one is not followed.
fn host_holds() -> &'static std::sync::Mutex<HashMap<String, std::time::Instant>> {
    static H: std::sync::OnceLock<std::sync::Mutex<HashMap<String, std::time::Instant>>> = std::sync::OnceLock::new();
    H.get_or_init(Default::default)
}
/// The hub is about to drive KiCad for `pro` (a check run): KiCad's show requests are not followed until `for_` has passed, or `release`.
pub fn hold_shows(pro: &str, for_: Duration) { host_holds().lock().unwrap().insert(pro.to_string(), std::time::Instant::now() + for_); }
/// The hub's drive is over: follow show requests again after a short grace (KiCad's last ones are still on their way).
pub fn release_shows(pro: &str) { hold_shows(pro, Duration::from_millis(1500)); }
fn shows_held(pro: &str) -> bool { host_holds().lock().unwrap().get(pro).is_some_and(|t| std::time::Instant::now() < *t) }
/// Why a KiCad show request (ET_SHOW_REQUESTED) is not followed, or None to follow it. `actor`: the API client that caused it (with
/// embed-request-actor; "" for the person or KiCad itself); `other`: it asks for a frame other than the active one; `held`: the hub is
/// driving KiCad (a check run, a pre-warm). A request with no actor during the hub's own drive is KiCad's, not the person's: on bp.13
/// batch 5 a DRC with schematic parity Raise()s the hidden board (FetchNetlistFromSchematic) with no actor, and the pane followed it to
/// the board while the person was on the schematic (lane A, Nov 13: a Ctrl+Y typed then went to the board). Fork batch 6 drops that Raise.
fn show_not_followed(by_actor: bool, actor: &str, editor: bool, other: bool, held: bool) -> Option<&'static str> {
    if by_actor && !actor.is_empty() && !actor.starts_with("person:") { return Some("an API client's") }
    if !editor && other && held && (!by_actor || actor.is_empty()) { return Some("while the hub drives it") }
    None
}

/// ET_EDITOR_REQUESTED (25): the editor's kind and the item (FrameInfo.lib_id, "Lib:Name") to open in it.
pub fn editor_requested(ev: &Value) -> Option<(String, String)> {
    if !(ev["type"] == "ET_EDITOR_REQUESTED" || ev["type"].as_i64() == Some(25)) { return None }
    let kind = kind_of_fk_val(&ev["frame"]["kind"]);
    if !matches!(kind, "SYMBOL_EDITOR" | "FOOTPRINT_EDITOR") { return None }
    Some((kind.to_string(), ev["frame"]["lib_id"].as_str().or(ev["frame"]["libId"].as_str()).unwrap_or("").to_string()))
}

pub fn fk_of(kind: &str) -> &'static str {
    match canon_frame(kind) {
        "PCB" => "FK_PCB",
        "SCH" => "FK_SCH",
        "SYMBOL_EDITOR" => "FK_SYMBOL_EDITOR",
        "FOOTPRINT_EDITOR" => "FK_FOOTPRINT_EDITOR",
        "PCB_3D" => "FK_PCB_3D",
        "GERBVIEW" => "FK_GERBVIEW",
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
        7 => "GERBVIEW",
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
        .filter(|s| s.pro != keep_pro && s.state == "ready" && (s.container == 0 || s.parked) && s.unsaved_docs().is_empty())
        .min_by_key(|s| s.touched_at)
        .map(|s| s.pro.clone())
}

/// Keeping KiCad warm before Edit (the person's "scarily fast": Edit on a project already open in Backplane draws at once). The client asks
/// (`editor.kicad.warm`) when it shows a thread of a project on this machine; the hub decides here.
#[derive(Clone, Copy, Debug)]
pub struct WarmCfg {
    pub max: usize, pub idle_s: u64,
    /// what a KiCad is taken to cost before one has been measured (bp.12 measured on real boards: 4.5-5.1 GB at its peak, the global footprint
    /// table parsed at start)
    pub kicad_mb: u64,
    /// memory left free for everything else once the new KiCad has started
    pub headroom_mb: u64,
}
/// Free memory (MemAvailable) below which the other editors are not pre-warmed after the first is on show: a pre-warmed schematic took
/// ~1.5 GB on Hubble_DK, and the headroom for everything else is kept as for a warm KiCad (WARM.headroom_mb).
pub const PREWARM_FREE_MB: u64 = 2048 + 4096;
pub const WARM: WarmCfg = WarmCfg { max: MAX_WARM, idle_s: 30 * 60, kicad_mb: 5200, headroom_mb: 4096 };

/// What the next KiCad will cost: the largest one measured running, else the configured guess.
pub fn kicad_cost_mb(sessions: &HashMap<String, Session>, cfg: WarmCfg) -> u64 {
    sessions.values().filter(|s| s.is_live()).map(|s| s.rss_mb).max().filter(|m| *m > 0).unwrap_or(cfg.kicad_mb)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warm {
    /// start KiCad detached (no pane yet), first closing `evict` (a clean, idle one) when the cap is reached
    Start { evict: Option<String> },
    /// it is up (or starting) already: only remembered as used now
    Have,
    /// not now, and why (said in the reply; Edit still starts it)
    No(&'static str),
}

/// Whether to warm `pro`. `hardware`: the project has KiCad files; `locked`: a KiCad lock file of the project's is there and no session of
/// ours holds it (the person's own KiCad, or a crash's leftover: KiCad would ask about it in a dialog, which a pre-warm must never raise);
/// `free_mb`: memory available (None: unknown, taken as enough); `quiet`: the runtime loads with no window of its own on screen (bp.12's
/// ET_PROGRESS instead of KiCad's progress windows: an older one shows "Load PCB" over Backplane when a thread is merely shown).
pub fn warm_decision(sessions: &HashMap<String, Session>, pro: &str, hardware: bool, locked: bool, free_mb: Option<u64>, quiet: bool, cfg: WarmCfg) -> Warm {
    if !hardware { return Warm::No("not a KiCad project") }
    if !quiet && !sessions.get(pro).is_some_and(Session::is_live) { return Warm::No("this KiCad shows its own windows while loading (needs Backplane KiCad bp.12)") }
    if sessions.get(pro).is_some_and(Session::is_live) { return Warm::Have }
    // a session that failed or was closed by the person is not started again behind their back; Edit does that
    if sessions.get(pro).is_some_and(|s| s.state == "failed" || s.reason == "KiCad was closed") { return Warm::No("KiCad stopped for this project; Edit starts it") }
    if locked { return Warm::No("the project is open in another KiCad") }
    // memory: the new KiCad's cost and the headroom must fit (builds and other programs take theirs first); room made by an eviction is
    // not counted on: a refused warm costs only Edit's cold start
    if free_mb.is_some_and(|m| m < kicad_cost_mb(sessions, cfg) + cfg.headroom_mb) { return Warm::No("too little free memory") }
    let live = sessions.values().filter(|s| s.pro != pro && s.is_live()).count();
    if live < cfg.max { return Warm::Start { evict: None } }
    match evict_warm(sessions, pro, cfg.max) { Some(v) => Warm::Start { evict: Some(v) }, None => Warm::No("as many KiCads as are kept warm are in use") }
}

/// The sessions to close for idleness: up, in no pane, nothing unsaved, not used for `idle_s`.
pub fn idle_victims(sessions: &HashMap<String, Session>, now: u64, idle_s: u64) -> Vec<String> {
    let mut v: Vec<String> = sessions.values()
        .filter(|s| s.state == "ready" && (s.container == 0 || s.parked) && !s.closing && s.unsaved_docs().is_empty() && now.saturating_sub(s.touched_at) >= idle_s)
        .map(|s| s.pro.clone()).collect();
    v.sort(); v
}

/// A KiCad lock file (`~<name>.lck`) for the project's own files in its folder.
pub fn project_locked(files: &Files) -> bool {
    [&files.pro, &files.pcb, &files.sch].into_iter().filter(|f| !f.is_empty()).any(|f| {
        let p = Path::new(f);
        match (p.parent(), p.file_name()) { (Some(d), Some(n)) => d.join(format!("~{}.lck", n.to_string_lossy())).exists(), _ => false }
    })
}

/// A process's resident memory in MB (Linux; None elsewhere).
fn rss_mb(pid: u32) -> Option<u64> {
    let st = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    st.lines().find(|l| l.starts_with("VmRSS:"))?.split_whitespace().nth(1)?.parse::<u64>().ok().map(|kb| kb / 1024)
}
/// MemAvailable in MB (Linux; None elsewhere).
fn free_mb() -> Option<u64> {
    let m = std::fs::read_to_string("/proc/meminfo").ok()?;
    m.lines().find(|l| l.starts_with("MemAvailable:"))?.split_whitespace().nth(1)?.parse::<u64>().ok().map(|kb| kb / 1024)
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

/// What a GetOpenDocuments answer says is open. `Busy`: KiCad is running a task and answers nothing else (AS_BUSY): what it has open is
/// unknown, never "nothing" (a caller waits or refuses; it never writes a file KiCad may hold, nor counts it clean or closed).
#[derive(Debug, PartialEq)]
pub enum OpenDocs { Docs(Vec<Value>), Busy(String), Unknown }
pub fn open_docs(reply: &Value) -> OpenDocs {
    if reply["status"] == "AS_BUSY" { return OpenDocs::Busy(reply["error"].as_str().unwrap_or("a task is running").to_string()) }
    match reply["message"]["documents"].as_array() { Some(d) if reply["ok"] == true => OpenDocs::Docs(d.clone()), _ => OpenDocs::Unknown }
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

/// Every ready co-editing GUI (bp.11 `coedit`): told when an agent's turn ends.
pub fn coedit_routes() -> Vec<GuiRoute> {
    routes().lock().unwrap().values().filter(|r| r.caps.iter().any(|c| c == "coedit")).cloned().collect()
}

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

/// The kinds of the frames each project's KiCad has open now ("PCB", "SCH", ...), kept with its route (put_route, at every publish).
fn open_frames() -> &'static Mutex<HashMap<String, Vec<String>>> { static F: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new(); F.get_or_init(Default::default) }
/// Whether the project's open KiCad has a frame of `kind` open (shown or not): no round trip.
pub fn has_frame(pro: &str, kind: &str) -> bool { open_frames().lock().unwrap().get(&norm_pro_key(pro)).is_some_and(|v| v.iter().any(|k| k == kind)) }
fn put_route(s: &Session, tx: &mpsc::UnboundedSender<Msg>) {
    open_frames().lock().unwrap().insert(norm_pro_key(&s.pro), s.frames.iter().map(|f| f.kind.clone()).collect());
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

/// The client name of the hub's checks in the open KiCad (kicad/edchecks.rs): their requests are not counted as another client's.
pub const CHECKS_CLIENT: &str = "hub:checks";
/// Per GUI socket: the requests in flight from other clients than the checks, and when the last named client's began or ended (ms).
fn gui_use() -> &'static Mutex<HashMap<String, (u32, u64)>> { static U: OnceLock<Mutex<HashMap<String, (u32, u64)>>> = OnceLock::new(); U.get_or_init(Default::default) }
/// Whether another client's request to this KiCad is in flight, or was within `quiet_ms`: a check started now would make it AS_BUSY.
pub fn others_busy(sock: &str, quiet_ms: u64) -> bool {
    gui_use().lock().unwrap().get(sock).is_some_and(|(n, at)| *n > 0 || crate::hub::now_ms().saturating_sub(*at) < quiet_ms)
}
/// Forward one IPC command through the embed actor's shared `kipc::Native` transport so GUI requests and agent `kicad_ipc`
/// requests serialize cleanly on one socket connection and share its `kicad_token`.
pub async fn gui_call(route: &GuiRoute, ty: &str, msg: Value) -> Value { gui_call_as(route, ty, msg, "").await }
/// The same, as `client` (an agent's actor, `agent:<thread>`): the fork's co-editing attributes, groups and undoes its changes by it.
pub async fn gui_call_as(route: &GuiRoute, ty: &str, msg: Value, client: &str) -> Value {
    // every request but the checks' own is counted, so an in-editor check never starts while another client waits (kicad/checkrun.rs)
    // a named client's (an agent, the person's edit from a panel or a phone) also holds checks a moment after it ends; the hub's own
    // unnamed reads (the live BOM's poll, every second) only while in flight
    struct Use(String, bool);
    impl Drop for Use { fn drop(&mut self) { if let Some(e) = gui_use().lock().unwrap().get_mut(&self.0) { e.0 = e.0.saturating_sub(1); if self.1 { e.1 = crate::hub::now_ms() } } } }
    let named = !client.is_empty();
    let _use = (client != CHECKS_CLIENT).then(|| { let mut m = gui_use().lock().unwrap(); let e = m.entry(route.sock.clone()).or_default(); e.0 += 1; if named { e.1 = crate::hub::now_ms() } Use(route.sock.clone(), named) });
    let (t, r) = oneshot::channel();
    if route.tx.send(Msg::Raw { sock: route.sock.clone(), desc: route.desc.clone(), ty: ty.into(), msg, timeout: 30000, client: client.into(), reply: t }).is_err() {
        return json!({"ok":false,"error":"no reply","why":"no reply","fallback":true})
    }
    r.await.unwrap_or_else(|_| json!({"ok":false,"error":"no reply","why":"no reply","fallback":true}))
}

/// One commit of item updates in the open editor as `client` (`coedit::client_name`): BeginCommit, UpdateItems, EndCommit, every one
/// with the document's header. KiCad routes a commit request without a header to the board editor (compatibility from before 11.0), so a
/// schematic commit whose EndCommit lacks it is never ended ("does not has a commit in progress") while its edit shows staged. A failed
/// update drops the commit (CMA_DROP, with the header too). Returns UpdateItems' reply.
pub async fn commit_as(route: &GuiRoute, client: &str, doc: &Value, items: Vec<Value>, message: &str) -> Result<Value, String> {
    let header = json!({"document": doc});
    let err = |o: &Value| o["error"].as_str().unwrap_or("no reply").to_string();
    let begun = gui_call_as(route, "kiapi.common.commands.BeginCommit", json!({"header": header}), client).await;
    if !begun["ok"].as_bool().unwrap_or(false) { return Err(format!("BeginCommit: {}", err(&begun))) }
    let id = begun["message"]["id"].clone();
    let up = gui_call_as(route, "kiapi.common.commands.UpdateItems", json!({"header": header, "items": items}), client).await;
    if !up["ok"].as_bool().unwrap_or(false) {
        let _ = gui_call_as(route, "kiapi.common.commands.EndCommit", json!({"header": header, "id": id, "action": "CMA_DROP"}), client).await;
        return Err(format!("UpdateItems: {}", err(&up)))
    }
    let end = gui_call_as(route, "kiapi.common.commands.EndCommit", json!({"header": header, "id": id, "action": "CMA_COMMIT", "message": message}), client).await;
    if !end["ok"].as_bool().unwrap_or(false) { return Err(format!("EndCommit: {}", err(&end))) }
    Ok(up["message"].clone())
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
    // fab output (an mfg/ Gerber set or file: kicad/gerber) is its project's
    let routed = if super::gerber::is_set_path(Path::new(&routed)) { let pro = super::ipc::pro_of(&routed); if pro.is_empty() { routed } else { pro } } else { routed };
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
        client: u64, pro: String, keep: bool, reply: oneshot::Sender<Result<Value, String>>,
    },
    /// the client shows this project: keep its KiCad warm (detached) if the policy says so
    Warm {
        client: u64, pro: String, root: String, files: Files, frame: String, display: String, hardware: bool, locked: bool, env: Env,
        reply: oneshot::Sender<Result<Value, String>>,
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
    Ready { pro: String, gen: u64, sock: String, version: String, holder: u64, events_sock: String, ev_since: u64, frames: Vec<Frame>, spawn_container: u64, spawn_active: String, caps: Vec<String> },
    StartFailed { pro: String, gen: u64, why: String },
    FramesUpdated { pro: String, gen: u64, holder: Option<u64>, frames: Vec<Frame>, reply: Option<(u64, oneshot::Sender<Result<Value, String>>)> },
    /// `editor`: KiCad's ET_EDITOR_REQUESTED (a library editor with the item it is for), not a plain ET_SHOW_REQUESTED
    ForkShowRequested { pro: String, gen: u64, kind: String, item: String, editor: bool, actor: String },
    Dialog { pro: String, gen: u64, open: bool, title: String },
    /// the dialogs of KiCad's the client sees on screen (a modal one stops KiCad's events and pings: this is how the hub learns of it)
    ClientDialogs { pro: String, open: Vec<String> },
    ForkSync { pro: String, gen: u64 },
    /// KiCad says it lost its parent (ET_DETACHED): the client that held the pane is gone (killed or quit; X destroyed its container)
    ForkDetached { pro: String, gen: u64 },
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

    /// Close a clean session for room or idleness (refused by KiCad if anything is modified meanwhile).
    fn retire(&mut self, victim: &str) {
        let Some(vs) = self.sessions.get_mut(victim) else { return };
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

    fn attach(&mut self, client: u64, pro: String, root: String, files: Files, frame: String, container: u64, toplevel: u64, display: String, wait: bool, env: Env, reply: oneshot::Sender<Result<Value, String>>) {
        let now = ipc::now_s();
        if let Some(existing) = self.sessions.get_mut(&pro) {
            if existing.state == "starting" {
                if container > 0 { existing.container = container; }
                if toplevel > 0 { existing.toplevel = toplevel; }
                if !display.is_empty() { existing.display = display; }
                existing.active = canon_frame(&frame).into();
                existing.touched_at = now; existing.parked = false;
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
                existing.touched_at = now; existing.parked = false;
                let snap = existing.clone();
                // the client's kept container again (its last View kept KiCad's frames in it): nothing to re-parent, and a realized frame is the
                // client's to map; the attach only says the session is in use again
                let lost = std::mem::take(&mut existing.lost_parent);
                if container > 0 && container == prev_cont && !lost && local_switch(&snap, snap.info_json()["local_map"] == true, true) {
                    let out = snap.info_json();
                    self.publish_and_reply(&snap, Some(client), reply, Ok(out));
                    return;
                }
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
        if let Some(victim) = evict_warm(&self.sessions, &pro, MAX_WARM) { self.retire(&victim); }

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
        let caps = env.caps.clone();
        tokio::spawn(async move {
            // Retire any headless kserve for this project before the GUI opens its files.
            let _ = ipc::retire(&pro).await;
            spawn_gui_session(me, home, pro, root_s, gen, mode_s, argv_vec, sock_dir, req_sock, eff_display, desc, active, spawn_container, sweep_locks, caps).await;
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
        // a frame KiCad has realized (shown once, or pre-warmed) on a runtime whose client maps frames itself: the switch is the client's
        // (the new `active` in the info), with no round trip to KiCad; a cross-probe's selection follows over SelectItems (xprobe.rs)
        if local_switch(&snap, snap.info_json()["local_map"] == true, doc.is_empty() && item.is_empty() && !remount) {
            let out = snap.info_json();
            self.publish_and_reply(&snap, Some(cid), reply, Ok(out));
            return;
        }
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

    fn detach(&mut self, client: u64, pro: String, keep: bool, reply: oneshot::Sender<Result<Value, String>>) {
        let Some(s) = self.sessions.get_mut(&pro) else {
            let _ = reply.send(Ok(json!({"detached":true}))); return;
        };
        // View with the client keeping its container: KiCad stays parented in it, only no longer in a pane
        if keep && s.container > 0 {
            s.parked = true; s.touched_at = ipc::now_s();
            let snap = s.clone();
            let out = snap.info_json();
            self.publish_and_reply(&snap, Some(client), reply, Ok(out));
            return;
        }
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
            Msg::Detach { client, pro, keep, reply } => self.detach(client, pro, keep, reply),
            Msg::Warm { client, pro, root, files, frame, display, hardware, locked, env, reply } => {
                let (desc, _) = discover_desc(&self.hub.cfg.home, &env);
                let quiet = !desc.is_empty() && proto::load(&desc).has_own("kiapi.backplane.embed.CancelProgress");
                match warm_decision(&self.sessions, &pro, hardware, locked, free_mb(), quiet, WARM) {
                    Warm::Have => {
                        if let Some(s) = self.sessions.get_mut(&pro) { s.touched_at = ipc::now_s(); }
                        let _ = reply.send(Ok(json!({"warm": true, "started": false})));
                    }
                    Warm::No(why) => { eprintln!("kicad embed: not warming {pro}: {why} (runtime {} {}, api.desc {desc})", env.root, env.version); let _ = reply.send(Ok(json!({"warm": false, "why": why}))); }
                    Warm::Start { evict } => {
                        if let Some(v) = evict { self.retire(&v); }
                        eprintln!("kicad embed: warming {pro}");
                        let (tx, rx) = oneshot::channel();
                        // detached: no container yet (the fork parks its frames); Edit attaches it to the pane
                        self.attach(client, pro, root, files, frame, 0, 0, display, false, env, tx);
                        tokio::spawn(async move { let r = rx.await.unwrap_or_else(|_| Err("no reply".into())); let _ = reply.send(r.map(|_| json!({"warm": true, "started": true}))); });
                    }
                }
            }
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
            Msg::Raw { sock, desc, ty, msg, timeout, client, reply } => {
                // BACKPLANE_KICAD_DEBUG: every request the hub sends the embedded KiCad, in order, with the time, its kind and KiCad's memory
                // then (what is loaded at open, and which request separates a 4.8 GB open from a 5.5 GB one)
                if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() && !ty.ends_with("NextEvents") && !ty.ends_with("Ping") {
                    let pid = self.sessions.values().find(|s| s.sock == sock).map_or(0, |s| s.pid);
                    eprintln!("kicad embed: request {ty} kind={} client={client} rss={}MB (at {})", msg["kind"].as_str().unwrap_or(""), rss_mb(pid).unwrap_or(0), ipc::now_ms());
                }
                self.raw_call(&sock, &desc, &ty, msg, timeout, &client, reply)
            }
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
            Msg::Ready { pro, gen, sock, version, holder, events_sock, ev_since, frames, spawn_container, spawn_active, caps } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.state == "starting") else { return };
                *s = s.clone().ready(&sock, &version, ipc::now_s());
                for c in caps { if !s.caps.contains(&c) { s.caps.push(c); } }
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
            Msg::ForkShowRequested { pro, gen, kind, item, editor, actor } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.state == "ready") else { return };
                // a library editor is shown on KiCad's ET_EDITOR_REQUESTED (embedded, on its item), never on the ET_SHOW_REQUESTED KiCad sends
                // before it (three of them on Ctrl+E: showing then floated a 900x720 top-level over the pane for ~0.8 s)
                // embed-request-actor (bp.13 after batch 4): the request says which API client caused it, "" for the person or KiCad itself:
                // only those are followed (the hub's own pre-warm, check runs and an agent's actions never move the person's pane); one with
                // no actor while the hub drives KiCad (hold_shows) is KiCad's own and not followed either (show_not_followed)
                let by_actor = s.caps.iter().any(|c| c == "embed-request-actor");
                if let Some(why) = show_not_followed(by_actor, &actor, editor, canon_frame(&kind) != s.active, shows_held(&pro)) {
                    eprintln!("kicad embed: {pro}: KiCad asked to show {kind} ({why}, actor {actor:?}): not followed");
                    let _ = self.me.send(Msg::ForkSync { pro, gen });
                    return;
                }
                if !editor && matches!(canon_frame(&kind), "SYMBOL_EDITOR" | "FOOTPRINT_EDITOR") && s.caps.iter().any(|c| c == "embed-editor-requested") {
                    let _ = self.me.send(Msg::ForkSync { pro, gen });
                    return;
                }
                if editor {
                    let (k, now) = (canon_frame(&kind).to_string(), std::time::Instant::now());
                    // the same editor and item again: on show already, or asked within 2 s (KiCad's echo, queued behind another show)
                    let same = s.editor_req.0 == k && s.editor_req.1 == item;
                    if same && (s.active == k || s.editor_req.2.is_some_and(|t| now.duration_since(t) < Duration::from_secs(2))) { return }
                    s.editor_req = (k, item.clone(), Some(now));
                } else {
                    s.editor_req = Default::default();
                }
                s.active = canon_frame(&kind).into();
                let snap = s.clone();
                let me = self.me.clone();
                tokio::spawn(async move {
                    if let Ok(Some((holder, _, frames))) = switch_fork_frame(&me, &snap.sock, &snap.desc, &snap.active, "", &item, false, false).await {
                        let _ = me.send(Msg::FramesUpdated { pro: snap.pro, gen: snap.gen, holder: Some(holder), frames, reply: None });
                    }
                });
            }
            Msg::Dialog { pro, gen, open, title } => {
                if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("kicad embed: dialog {} {title:?} (gen {gen}, session {:?})", if open { "opened" } else { "closed" }, self.sessions.get(&pro).map(|s| (s.gen, s.state.clone()))); }
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.is_live()) else { return };
                *s = s.clone().dialog(open, &title);
                let snap = s.clone();
                self.publish(&snap, None);
            }
            Msg::ClientDialogs { pro, open } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.is_live() && s.dialogs != open) else { return };
                s.dialogs = open;
                if !s.dialogs.is_empty() && s.state == "unresponsive" { s.state = "ready".into(); s.reason.clear(); s.missed_pings = 0; }
                let snap = s.clone();
                self.publish(&snap, None);
            }
            Msg::ForkSyncNow { pro } => {
                if let Some(gen) = self.sessions.get(&pro).map(|s| s.gen) { let _ = self.me.send(Msg::ForkSync { pro, gen }); }
            }
            Msg::ForkDetached { pro, gen } => {
                // the next attach re-parents whatever its container's id: a client started again gets the same X ids as the one that went
                // (Xvfb and Xorg hand a new connection the freed resource base), so an equal id is no proof the container is the kept one
                // (a late one from the hub's own detach only costs the next attach a re-parent)
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen) else { return };
                eprintln!("kicad embed: KiCad lost its parent {}: the next Edit re-parents", s.container);
                s.lost_parent = true;
                let _ = self.me.send(Msg::ForkSync { pro, gen });
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
                for v in idle_victims(&self.sessions, ipc::now_s(), WARM.idle_s) { eprintln!("kicad embed: idle, closing {v}"); self.retire(&v); }
                for s in self.sessions.values_mut().filter(|s| s.is_live() && s.pid > 0) { if let Some(m) = rss_mb(s.pid) { s.rss_mb = m; } }
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
    // GerbView (bp.13, in KiCad's process): its files are opened with OpenGerbers (Gerbers and drill files, or one .gbrjob for all its
    // layers; `doc` lists them a line each), which shows its frame; ShowFrame shows it again after
    if canon_frame(kind) == "GERBVIEW" && !doc.is_empty() {
        let files: Vec<&str> = doc.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let o = raw_on(me, sock, desc, "kiapi.backplane.embed.OpenGerbers", json!({ "files": files, "show": true }), 60000).await;
        if o["ok"].as_bool().unwrap_or(false) { return Ok(o) }
        return Err(format!("OpenGerbers: {}", o["error"].as_str().unwrap_or("failed")));
    }
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
        let docs = match open_docs(&o) {
            OpenDocs::Docs(d) => d,
            // busy: whether it holds unsaved work is unknown, so it counts as unsaved (the close asks), never as clean
            OpenDocs::Busy(why) => { names.push(format!("{kind} (KiCad is busy: {why})")); continue }
            OpenDocs::Unknown => continue,
        };
        let docs = &docs;
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
    caps: Vec<String>,
) {
    // where a cold start's time goes: one line per phase (hub log), for the perf e2e and the person's own log
    let t0 = std::time::Instant::now();
    let phase = |what: &str| eprintln!("kicad embed: start +{}ms {what}", t0.elapsed().as_millis());
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
    // a memory scope of its own (memscope.rs `wrap_helper`): KiCad killed for memory is killed alone, never with or instead of the hub
    let mut c = crate::proc::tokio_cmd_in(crate::proc::Kind::Local, &argv[0], crate::memscope::wrap_helper("kicad")); crate::memscope::raise_oom(&mut c);
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
        // Bound glibc's arenas during the threaded library preload. Five Hubble GPU opens averaged 235 MB (5.7%) less RSS with 8 arenas;
        // the remaining 532 MB spread is unexplained. The fork's Xvfb preload experiment saved ~650 MB for +0.3 s, specific to that venue.
        // The person's own value wins, including an explicitly empty value (as in the fork's launcher default).
        #[cfg(target_os = "linux")]
        if std::env::var_os("MALLOC_ARENA_MAX").is_none() { c.env("MALLOC_ARENA_MAX", "8"); }
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
    crate::memscope::watch_helper(pid, "KiCad");
    crate::plat::proc::adopt(pid, true); crate::memscope::cap_job(pid, "kicad");
    if pid > 0 {
        live_pids().lock().unwrap_or_else(|e| e.into_inner()).insert(pid);
        crate::agents::proc::watch_group(pid);
    }
    let _ = me.send(Msg::Spawned { pro: pro.clone(), gen, pid });
    phase("spawned");

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

    phase("socket");
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

    phase("answering");
    // the embedded editor's own capabilities (bp.13's embed-frame-drawn, embed-gerbview, embed-editor-requested are said there, not in the
    // headless manifest the runtime's caps come from)
    let mut caps = caps;
    if mode == "fork" {
        let gc = raw_on(&me, &sock, &desc, "kiapi.backplane.GetCapabilities", json!({}), 3000).await;
        for c in embed_caps(&gc["message"]) { if !caps.iter().any(|x| *x == c) { caps.push(c); } }
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
                phase("first show");
                let win = u64_of(&first["message"]["frame"]["window"]);
                match first_show_wait(&caps, std::env::var("BACKPLANE_KICAD_DECOR_SLEEP").ok().as_deref()) {
                    // bp.13: the fork shows embedded frames at once and says when the first one has drawn (ET_FRAME_DRAWN)
                    FirstShow::Drawn(cap) if win > 0 => {
                        let drawn = wait_frame_drawn(&me, &sock, &desc, win, &mut ev_since, cap).await;
                        phase(if drawn { "drawn" } else { "drawn: not said within the cap" });
                    }
                    FirstShow::Sleep(ms) if win > 0 => {
                        tokio::time::sleep(Duration::from_millis(ms)).await;
                        let _ = raw_on(&me, &sock, &desc, "kiapi.backplane.embed.HideFrame", json!({ "window": win }), 5000).await;
                    }
                    _ => {}
                }
            }
        }
        if let Ok(Some((h2, ev2, f2))) = switch_fork_frame(&me, &sock, &desc, &active, "", "", false, false).await {
            holder = h2;
            if !ev2.is_empty() { events_sock = ev2; }
            frames = f2;
        }
        // the chords KiCad hands to Backplane, with Ctrl+; for a comment (comment.rs): beside the ready, not before it
        { let (me, sock, desc, caps) = (me.clone(), sock.clone(), desc.clone(), caps.clone()); tokio::spawn(async move {
            let rk = raw_on(&me, &sock, &desc, "kiapi.backplane.embed.SetReservedKeys", json!({"chords": super::comment::reserved(proto::load(&desc).has_field("kiapi.backplane.embed.CommentSelection", "capture_only"), &caps)}), 5000).await;
            eprintln!("kicad embed: reserved keys {}", if rk["ok"] == true { rk["message"]["chords"].to_string() } else { format!("refused: {}", rk["error"]) });
        }); }
        phase("ready");
        eprintln!("kicad embed: running {version} ({})", argv.first().map(String::as_str).unwrap_or(""));
        let _ = me.send(Msg::Ready { pro, gen, sock, version, holder, events_sock, ev_since, frames, spawn_container, spawn_active: active, caps });
    } else {
        if let Some(act) = stock_action_for(&active) {
            let _ = run_stock_action(&me, &sock, &desc, act).await;
        }
        let _ = me.send(Msg::Ready { pro, gen, sock, version, holder: 0, events_sock: String::new(), ev_since: 0, frames: vec![], spawn_container, spawn_active: active, caps });
    }
}

/// ET_FRAME_DRAWN (28) for `win` on the events socket within `cap_ms`; `since` moves past what was read (the events loop starts there).
async fn wait_frame_drawn(me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, win: u64, since: &mut u64, cap_ms: u64) -> bool {
    let until = tokio::time::Instant::now() + Duration::from_millis(cap_ms);
    let ev_path = format!("{sock}.events");
    let mut seen: Vec<Value> = Vec::new();
    while tokio::time::Instant::now() < until {
        let left = until.saturating_duration_since(tokio::time::Instant::now()).as_millis() as u64;
        let o = raw_on(me, &ev_path, desc, "kiapi.backplane.embed.NextEvents", json!({"since": *since, "wait_ms": left.min(1000), "max": 64}), left + 1000).await;
        if !o["ok"].as_bool().unwrap_or(false) { tokio::time::sleep(Duration::from_millis(30)).await; continue }
        let next = u64_of(&o["message"]["next"]);
        for ev in o["message"]["events"].as_array().into_iter().flatten() { seen.push(ev.clone()); }
        if next > *since { *since = next; }
        if seen.iter().any(|ev| (ev["type"] == "ET_FRAME_DRAWN" || ev["type"].as_i64() == Some(28)) && u64_of(&ev["frame"]["window"]) == win) { return true }
    }
    false
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
            let mut show_req: Option<(String, String, bool, String)> = None;
            if let Some(evs) = msg["events"].as_array() {
                for ev in evs {
                    let ty = ev["type"].as_str().unwrap_or("");
                    let ty_n = ev["type"].as_i64().unwrap_or(0);
                    if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("kicad embed event: {ev}"); }
                    super::checkrun::on_event(&pro, ev);
                    if super::edchecks::on_event(ev) { continue }
                    if ty == "ET_SELECTION_CHANGED" || ty_n == 14 {
                        if let Some((kind, names)) = super::xprobe::selection_event(ev) { super::coedit::person_selected(&pro, &kind.to_lowercase(), names.to_json()); }
                        super::xprobe::kicad_selected(&pro, ev); continue
                    }
                    if super::coedit::on_edited(&pro, ev) { continue }
                    if super::coedit::on_notice(&pro, ev) { continue }
                    if super::reload::on_event(&pro, ev) { need_sync = true; continue }
                    if super::comment::on_event(&pro, ev) { continue }
                    if super::progress::on_event(&pro, ev) { continue }
                    if crate::plan::events::on_event(&pro, ev) { continue }
                    if crate::review::markers::on_event(&pro, ev) { continue }
                    if super::coedit::on_event(ev) { continue }
                    if matches!(ty, "ET_DIALOG_OPENED" | "ET_DIALOG_CLOSED") || matches!(ty_n, 10 | 11) {
                        let open = ty == "ET_DIALOG_OPENED" || ty_n == 10;
                        let title = ev["detail"].as_str().unwrap_or("").to_string();
                        if me.send(Msg::Dialog { pro: pro.clone(), gen, open, title }).is_err() { return }
                        continue
                    }
                    if ty == "ET_DETACHED" || ty_n == 12 {
                        if me.send(Msg::ForkDetached { pro: pro.clone(), gen }).is_err() { return }
                        continue
                    }
                    if ty == "ET_SHOW_REQUESTED" || ty_n == 9 {
                        // (an editor request in the same batch wins)
                        if !show_req.as_ref().is_some_and(|r| r.2) { show_req = Some((kind_of_fk_val(&ev["frame"]["kind"]).to_string(), String::new(), false, ev["detail"].as_str().unwrap_or("").to_string())); }
                    } else if let Some((k, item)) = editor_requested(ev) {
                        // bp.13: KiCad asked for its symbol or footprint editor (Edit Symbol, Edit Footprint...): shown in the pane on that item
                        show_req = Some((k, item, true, ev["detail"].as_str().unwrap_or("").to_string()));
                    } else if matches!(ty, "ET_FRAME_OPENED" | "ET_FRAME_CLOSED" | "ET_FRAME_SHOWN" | "ET_FRAME_HIDDEN"
                        | "ET_TITLE_CHANGED" | "ET_MODIFIED_CHANGED" | "ET_DETACHED" | "ET_DOCUMENT_CHANGED" | "ET_ATTACHED")
                        || matches!(ty_n, 1 | 2 | 3 | 4 | 5 | 6 | 12 | 13 | 18)
                    {
                        need_sync = true;
                    }
                }
            }
            if let Some((kind, item, editor, actor)) = show_req {
                if me.send(Msg::ForkShowRequested { pro: pro.clone(), gen, kind, item, editor, actor }).is_err() { return }
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
            let t = std::time::Instant::now();
            if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("kicad embed: publish {k} {}", v.chars().take(400).collect::<String>()); }
            info_hub.set_info(&k, &v).await;
            if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("kicad embed: published {k} in {} ms", t.elapsed().as_millis()); }
            if t.elapsed() > Duration::from_millis(100) { eprintln!("kicad embed: publishing {k} took {} ms", t.elapsed().as_millis()); }
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
    "editor.kicad.warm",
    "editor.kicad.dialogs",
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
                let mut kinds = prewarm_kinds(p["active"].as_str().unwrap_or("PCB"), &have);
                // memory: a pre-warmed editor costs its frame and its libraries (Hubble_DK's schematic: ~1.5 GB, 3.9 -> 5.4 GB), worth it only
                // with room to spare; below that the first switch loads it (~1 s) instead. BACKPLANE_KICAD_PREWARM_FREE_MB overrides the floor
                let floor = std::env::var("BACKPLANE_KICAD_PREWARM_FREE_MB").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(PREWARM_FREE_MB);
                if let Some(free) = free_mb().filter(|f| *f < floor) {
                    if !kinds.is_empty() { eprintln!("kicad embed: {}: not pre-warming {kinds:?}: {free} MB free, under {floor} MB", resolved.pro); }
                    kinds.clear();
                }
                let (me, gui2, pro) = (actor_tx.clone(), gui.clone(), resolved.pro.clone());
                // a pre-warm is the hub's own drive: KiCad's show requests for the frames it opens are not the person's (Hubble_DK on
                // bp.13 batch 4 posts ET_SHOW_REQUESTED for the pre-warmed schematic, and the pane flipped to it after Edit)
                if !kinds.is_empty() { hold_shows(&pro, Duration::from_secs(60)); }
                tokio::spawn(async move {
                    struct Release(String);
                    impl Drop for Release { fn drop(&mut self) { release_shows(&self.0); } }
                    let _release = Release(pro.clone());
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
            if method == "editor.kicad.dialogs" {
                let open: Vec<String> = p["open"].as_array().into_iter().flatten().filter_map(|t| t.as_str().map(String::from)).take(8).collect();
                let _ = actor_tx.send(Msg::ClientDialogs { pro: resolved.pro, open });
                return ok_reply(id, json!({}))
            }
            if method == "editor.kicad.touched" { unsaved_update(&resolved.pro, |u| u.touched()); super::checkrun::person_touched(&resolved.pro); return ok_reply(id, json!({})) }
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
                "editor.kicad.warm" => {
                    // BACKPLANE_KICAD_WARM=0: never (the perf e2e measures a true cold Edit with it)
                    if std::env::var("BACKPLANE_KICAD_WARM").is_ok_and(|v| v == "0") { return ok_reply(id, json!({"warm": false, "why": "off"})) }
                    let hardware = super::canonical::hardware(&resolved.root);
                    let locked = project_locked(&resolved.files);
                    let display = p["display"].as_str().unwrap_or("").to_string();
                    let env = ipc::runtime(hub).await;
                    actor_tx.send(Msg::Warm {
                        client, pro: resolved.pro, root: resolved.root, files: resolved.files, frame: resolved.frame, display, hardware, locked, env, reply: tx,
                    })
                }
                "editor.kicad.detach" => {
                    actor_tx.send(Msg::Detach { client, pro: resolved.pro, keep: p["keep"].as_bool().unwrap_or(false), reply: tx })
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

/// Tests: a route to a stand-in editor, whose requests come out of the receiver as (type, message, reply).
#[cfg(test)]
pub fn stand_in_route(pro: &str) -> (GuiRoute, mpsc::UnboundedReceiver<(String, Value, oneshot::Sender<Value>)>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
    let (otx, orx) = mpsc::unbounded_channel();
    tokio::spawn(async move { while let Some(m) = rx.recv().await { if let Msg::Raw { ty, msg, reply, .. } = m { if otx.send((ty, msg, reply)).is_err() { return } } } });
    (GuiRoute { pro: pro.into(), mode: "fork".into(), sock: "/stand-in".into(), desc: String::new(), cli: String::new(), argv: String::new(), version: "10.99.0-bp.13".into(), caps: vec![], tx }, orx)
}

#[cfg(test)]
mod tests {
    /// A named client's request holds the checks while in flight and a moment after; the hub's unnamed reads only while in flight.
    #[tokio::test] async fn others_busy_counts_named_clients_after_and_polls_only_during() {
        let (route, mut rx) = stand_in_route("/p/busy.kicad_pro");
        let sock = route.sock.clone();
        gui_use().lock().unwrap().remove(&sock);
        tokio::spawn(async move { while let Some((_, _, reply)) = rx.recv().await { let _ = reply.send(json!({"ok": true, "message": {}})); } });
        let _ = gui_call(&route, "kiapi.common.commands.GetOpenDocuments", json!({})).await;
        assert!(!others_busy(&sock, 1_500), "a poll that ended holds nothing");
        let _ = gui_call_as(&route, "kiapi.common.commands.GetItems", json!({}), "agent:t1").await;
        assert!(others_busy(&sock, 1_500), "an agent's request just ended: wait a moment");
        assert!(!others_busy(&sock, 0));
        let _ = gui_call_as(&route, "kiapi.backplane.embed.GetMarkers", json!({}), CHECKS_CLIENT).await;
        gui_use().lock().unwrap().remove(&sock);
        let _ = gui_call_as(&route, "kiapi.backplane.embed.GetMarkers", json!({}), CHECKS_CLIENT).await;
        assert!(!others_busy(&sock, 1_500), "the checks' own never count");
    }

    #[test] fn a_show_request_with_no_actor_during_the_hubs_own_check_is_not_followed() {
        // bp.13 batch 5: DRC parity's Raise() of the hidden board, no actor, while the hub's check holds shows
        assert_eq!(show_not_followed(true, "", false, true, true), Some("while the hub drives it"));
        // the same with no hub drive: the person's (or KiCad's on their action), followed
        assert_eq!(show_not_followed(true, "", false, true, false), None);
        // a person's request through a client, even during a check: followed
        assert_eq!(show_not_followed(true, "person:phone", false, true, true), None);
        // an agent's or the hub's own client: never followed
        assert_eq!(show_not_followed(true, "hub:checks", false, false, false), Some("an API client's"));
        // the frame already on show, or a library editor's request: followed (nothing moves / the person asked for it)
        assert_eq!(show_not_followed(true, "", false, false, true), None);
        assert_eq!(show_not_followed(true, "", true, true, true), None);
        // a runtime without actors: told by time only, as before
        assert_eq!(show_not_followed(false, "", false, true, true), Some("while the hub drives it"));
        assert_eq!(show_not_followed(false, "", false, true, false), None);
    }
    #[test] fn gerbview_and_the_editors_kicad_asks_for() {
        use super::*;
        assert_eq!(canon_frame("FK_GERBVIEW"), "GERBVIEW"); assert_eq!(fk_of("GERBVIEW"), "FK_GERBVIEW"); assert_eq!(kind_of_fk_val(&json!(7)), "GERBVIEW");
        // 77a6ff17d0 says its commands only; the names come with batch 3
        assert_eq!(embed_caps(&json!({"capabilities": ["embed", "embed-frame-drawn"], "commands": ["kiapi.backplane.embed.OpenGerbers"]})), vec!["embed", "embed-frame-drawn", "embed-gerbview"]);
        assert_eq!(embed_caps(&json!({"capabilities": ["embed-gerbview"], "commands": ["kiapi.backplane.embed.OpenGerbers"]})), vec!["embed-gerbview"]);
        assert!(embed_caps(&json!({"capabilities": ["embed"], "commands": ["kiapi.backplane.embed.ShowFrame"]})).iter().all(|c| c != "embed-gerbview"));
        assert_eq!(editor_requested(&json!({"type": "ET_EDITOR_REQUESTED", "frame": {"kind": "FK_SYMBOL_EDITOR", "lib_id": "Device:R"}})), Some(("SYMBOL_EDITOR".into(), "Device:R".into())));
        assert_eq!(editor_requested(&json!({"type": 25, "frame": {"kind": 4, "libId": "R_SMD:R_0603"}})), Some(("FOOTPRINT_EDITOR".into(), "R_SMD:R_0603".into())));
        assert_eq!(editor_requested(&json!({"type": "ET_SHOW_REQUESTED", "frame": {"kind": "FK_SYMBOL_EDITOR"}})), None);
    }
    #[tokio::test] async fn a_commit_as_the_person_carries_the_header_and_the_name_on_every_call() {
        let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
        let route = GuiRoute { pro: "/p/x.kicad_pro".into(), mode: "fork".into(), sock: "/s".into(), desc: String::new(), cli: String::new(),
            argv: String::new(), version: "10.99.0-bp.12".into(), caps: vec![], tx };
        let seen = tokio::spawn(async move {
            let mut seen = vec![];
            while let Some(m) = rx.recv().await {
                if let Msg::Raw { ty, msg, client, reply, .. } = m {
                    let message = if ty.ends_with("BeginCommit") { json!({"id": {"value": "c1"}}) } else { json!({}) };
                    let _ = reply.send(json!({"ok": true, "message": message}));
                    seen.push((ty, msg, client));
                    if seen.len() == 3 { break }
                }
            }
            seen
        });
        let doc = json!({"type": "DOCTYPE_SCHEMATIC", "sheet_path": {}});
        let who = super::super::coedit::client_name(&super::super::coedit::Who::Person);
        commit_as(&route, &who, &doc, vec![json!({"@type": "x"})], "BOM: MPN of R4").await.unwrap();
        let seen = seen.await.unwrap();
        let tys: Vec<&str> = seen.iter().map(|(t, _, _)| t.rsplit('.').next().unwrap()).collect();
        assert_eq!(tys, ["BeginCommit", "UpdateItems", "EndCommit"]);
        for (ty, msg, client) in &seen {
            assert_eq!(msg["header"]["document"], doc, "{ty} carries the document's header");
            assert_eq!(client, &who, "{ty} goes as the person");
        }
        assert_eq!(seen[2].1["id"], json!({"value": "c1"}));
        assert_eq!(seen[2].1["action"], "CMA_COMMIT");
    }

    use super::*;

    #[test] fn a_realized_frame_is_shown_by_the_client_with_no_round_trip() {
        let mut s = Session::new("/p/a.kicad_pro", "/p", "fork", "SCH", 1, 0).ready("/s", "", 0);
        let f = |kind: &str, realized: bool| Frame { kind: kind.into(), doc: String::new(), title: String::new(), modified: false, shown: false, realized, xid: 7, min_w: 0, min_h: 0 };
        s.frames = vec![f("PCB", true), f("SCH", true)];
        assert!(!local_switch(&s, false, true), "a runtime that does not map locally");
        assert!(local_switch(&s, true, true));
        assert!(!local_switch(&s, true, false), "a document, item or remount goes to KiCad");
        s.frames = vec![f("PCB", true), f("SCH", false)];
        assert!(!local_switch(&s, true, true), "never shown: KiCad's first show (GTK)");
    }

    #[test] fn the_first_show_waits_for_the_drawn_frame_where_the_fork_says_it() {
        assert_eq!(first_show_wait(&["embed".into(), "embed-frame-drawn".into()], None), FirstShow::Drawn(1500));
        assert_eq!(first_show_wait(&["embed".into()], None), FirstShow::Sleep(1150), "older forks keep wxGTK's timer");
        assert_eq!(first_show_wait(&[], Some("0")), FirstShow::None);
        assert_eq!(first_show_wait(&["embed-frame-drawn".into()], Some("0")), FirstShow::Drawn(1500));
    }

    #[test] fn kicads_own_dialogs_are_published_until_they_close() {
        let s = Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0).dialog(true, "Load Schematic").dialog(true, "Information");
        assert_eq!(s.info_json()["dialogs"], json!(["Load Schematic", "Information"]), "innermost last");
        let s = s.dialog(false, "Load Schematic");
        assert_eq!(s.dialogs, ["Information"], "closed by title, whatever the order");
        let s = s.dialog(false, "");
        assert!(s.dialogs.is_empty(), "an untitled close closes the innermost");
        assert!(Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0).dialog(true, "x").failed("gone").dialogs.is_empty(), "a dead KiCad has none");
        let asking = Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0).ready("/s", "", 0).dialog(true, "Information");
        assert_eq!(asking.ping_result(false).ping_result(false).ping_result(false).state, "ready", "a dialog open: missed pings are not a hang");
    }

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
        assert_eq!(open_docs(&json!({"ok": false, "status": "AS_BUSY", "error": "a task is running"})), OpenDocs::Busy("a task is running".into()), "busy is not closed");
        assert_eq!(open_docs(&json!({"ok": true, "message": {"documents": []}})), OpenDocs::Docs(vec![]));
        assert_eq!(open_docs(&json!({"ok": false, "status": "AS_UNHANDLED"})), OpenDocs::Unknown);
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

    #[test] fn a_shown_project_is_kept_warm_within_the_cap_memory_and_locks() {
        let ready = |p: &str, t: u64, cont: u64| { let mut s = Session::new(p, "/p", "fork", "PCB", 1, t).ready("/s", "", t); s.container = cont; (p.to_string(), s) };
        let cfg = WarmCfg { max: 2, idle_s: 1800, kicad_mb: 2000, headroom_mb: 1000 };
        let empty = HashMap::new();
        assert_eq!(warm_decision(&empty, "/p/a.kicad_pro", true, false, Some(8000), true, cfg), Warm::Start { evict: None });
        assert_eq!(warm_decision(&empty, "/p/a.kicad_pro", true, false, None, true, cfg), Warm::Start { evict: None }, "memory unknown: enough");
        assert!(matches!(warm_decision(&empty, "/p/a.kicad_pro", false, false, Some(8000), true, cfg), Warm::No(_)), "not hardware");
        assert!(matches!(warm_decision(&empty, "/p/a.kicad_pro", true, true, Some(8000), true, cfg), Warm::No(_)), "locked by another KiCad: its dialog would come up");
        assert!(matches!(warm_decision(&empty, "/p/a.kicad_pro", true, false, Some(2900), true, cfg), Warm::No(_)), "less than a KiCad and the headroom");
        assert!(matches!(warm_decision(&empty, "/p/a.kicad_pro", true, false, Some(8000), false, cfg), Warm::No(_)), "a runtime that shows its load windows is never warmed");
        // a KiCad measured running sets the cost of the next: 5 GB measured, 6 GB free is not enough for another
        let mut big = Session::new("/p/b.kicad_pro", "/p", "fork", "PCB", 1, 0).ready("/s", "", 0); big.container = 3; big.rss_mb = 5000;
        let m: HashMap<String, Session> = [("/p/b.kicad_pro".to_string(), big)].into_iter().collect();
        assert_eq!(kicad_cost_mb(&m, cfg), 5000);
        assert!(matches!(warm_decision(&m, "/p/a.kicad_pro", true, false, Some(5500), true, cfg), Warm::No(_)));
        assert_eq!(warm_decision(&m, "/p/a.kicad_pro", true, false, Some(6500), true, cfg), Warm::Start { evict: None });
        let m: HashMap<String, Session> = [ready("/p/a.kicad_pro", 10, 0)].into_iter().collect();
        assert_eq!(warm_decision(&m, "/p/a.kicad_pro", true, true, Some(100), true, cfg), Warm::Have, "ours already: its own lock and memory do not count");
        // at the cap: the oldest clean one in no pane makes room; none such: refused (never a 3rd)
        let m: HashMap<String, Session> = [ready("/p/a.kicad_pro", 10, 0), ready("/p/b.kicad_pro", 5, 0)].into_iter().collect();
        assert_eq!(warm_decision(&m, "/p/c.kicad_pro", true, false, Some(8000), true, cfg), Warm::Start { evict: Some("/p/b.kicad_pro".into()) });
        let m: HashMap<String, Session> = [ready("/p/a.kicad_pro", 10, 7), ready("/p/b.kicad_pro", 5, 9)].into_iter().collect();
        assert!(matches!(warm_decision(&m, "/p/c.kicad_pro", true, false, Some(8000), true, cfg), Warm::No(_)), "both in panes");
        // the person closed KiCad, or it failed: Edit starts it, not a pre-warm
        let closed = Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0).exited(0, "");
        let failed = Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0).failed("x");
        for s in [closed, failed] { let m: HashMap<String, Session> = [("/p/a.kicad_pro".to_string(), s)].into_iter().collect(); assert!(matches!(warm_decision(&m, "/p/a.kicad_pro", true, false, Some(8000), true, cfg), Warm::No(_))); }
        // evicted for room (closing): warm again later
        let mut ev = Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0); ev.closing = true; let ev = ev.exited(0, "");
        let m: HashMap<String, Session> = [("/p/a.kicad_pro".to_string(), ev)].into_iter().collect();
        assert_eq!(warm_decision(&m, "/p/a.kicad_pro", true, false, Some(8000), true, cfg), Warm::Start { evict: None });
    }

    #[test] fn idle_kicads_in_no_pane_close_after_the_idle_time() {
        let mk = |p: &str, t: u64, cont: u64, modified: bool| { let mut s = Session::new(p, "/p", "fork", "PCB", 1, t).ready("/s", "", t); s.container = cont;
            if modified { s.frames.push(Frame { kind: "PCB".into(), doc: p.into(), title: String::new(), modified: true, shown: true, realized: true, xid: 1, min_w: 0, min_h: 0 }); } (p.to_string(), s) };
        let m: HashMap<String, Session> = [mk("/p/old.kicad_pro", 0, 0, false), mk("/p/new.kicad_pro", 1500, 0, false), mk("/p/pane.kicad_pro", 0, 5, false), mk("/p/dirty.kicad_pro", 0, 0, true)].into_iter().collect();
        assert_eq!(idle_victims(&m, 1800, 1800), ["/p/old.kicad_pro"]);
    }

    #[test] fn a_kicad_lock_file_beside_the_project_counts() {
        let dd = crate::plat::tmp::dir("warm-lock"); let d = dd.to_path_buf();
        let f = Files { pro: d.join("a.kicad_pro").to_string_lossy().into(), pcb: d.join("a.kicad_pcb").to_string_lossy().into(), sch: String::new(), all: vec![] };
        assert!(!project_locked(&f));
        std::fs::write(d.join("~a.kicad_pcb.lck"), "{}").unwrap();
        assert!(project_locked(&f));
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

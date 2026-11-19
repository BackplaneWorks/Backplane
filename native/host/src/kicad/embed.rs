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
use std::{collections::{HashMap, HashSet, VecDeque}, path::{Path, PathBuf}, process::Stdio, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex, OnceLock}, time::Duration};
use tokio::{io::{AsyncBufReadExt, AsyncRead, BufReader}, sync::{mpsc, oneshot}};

/// The most KiCads the hub keeps running at once, in all: those in panes, parked and warmed, the one being asked for included (not 3 besides
/// it). A warm-up or an Edit that would start one more first closes the oldest clean one in no pane ([`evict_warm`]); a warm-up with none
/// such is refused.
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
    /// What KiCad would have put in a message box it could not show (ET_NOTICE, `embed-notice`: "Could not use OpenGL, falling back to
    /// software rendering"), oldest first: each {id, kind, title, detail, error, frame, dismissed}. Kept for the session (an agent asking the
    /// session's state reads them all); the person's dismissal only hides one in the pane
    pub notices: Vec<Value>,
    /// The frames KiCad said it has drawn since they were last shown (ET_FRAME_DRAWN, `embed-frame-drawn`), by window: the client holds the
    /// viewer's picture under a first Edit until its frame is here (KicadPane::handoff). The last 16
    pub drawn: Vec<u64>,
    /// macOS (`embed-reserved-key-event`): the reserved chords pressed in KiCad (ET_RESERVED_KEY), the last 8 as [seq, index in
    /// SetReservedKeys or 63 for the comment key]; elsewhere they reach the client's window directly (_BACKPLANE_KEY / WM_APP)
    pub keys: Vec<(u64, u64)>,
    /// KiCad's resident memory now, MB (sampled on the health tick, mem.rs `proc_mb`; 0 unknown)
    pub rss_mb: u64,
    /// its peak so far, MB (Linux VmHWM, macOS lifetime maximum footprint, Windows peak working set; 0 unknown): the warm policy's measure
    /// of what a KiCad costs
    pub peak_mb: u64,
    /// (rss_mb, peak_mb) as last published from the health tick ([`mem_moved`]), and whether the peak has been logged past the default
    pub mem_pub: (u64, u64), pub peak_logged: bool,
    /// a request of the hub's drive (a show, a pre-warm) waits on KiCad's check or refill task (task-gate-reads): its description, "" for none
    pub task_wait: String,
    /// a start's last step ("spawned", "socket", "answering", "first show") and when KiCad was started (epoch ms): a pane waiting on it
    /// says what KiCad is doing past 10 s, never a bare "Starting KiCad…" (the release check's fresh home, Nov 16)
    pub start_phase: String, pub start_at: u64,
    /// a document KiCad is about to open is held by another live KiCad (not ours): {file, holder} put to the person in the pane, Open
    /// anyway or Cancel, before KiCad opens it (bp.13 would open over it without asking); Null when nothing is asked
    pub lock_ask: Value,
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
            container: 0, toplevel: 0, holder: 0, display: String::new(), start_phase: String::new(), start_at: 0, lock_ask: Value::Null,
            sock: String::new(), events_sock: String::new(), sock_dir: String::new(),
            argv: String::new(), cli: String::new(), version: String::new(), desc: String::new(),
            caps: vec![], frames: vec![], missed_pings: 0, dialogs: vec![], notices: vec![], drawn: vec![], keys: vec![], rss_mb: 0, peak_mb: 0, mem_pub: (0, 0), peak_logged: false, task_wait: String::new(), parked: false, touched_at: now, closing: false, lost_parent: false, editor_req: (String::new(), String::new(), None),
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
    /// A notice from KiCad (`notice_of`): the same one again (kind, title and detail) is not added twice; at most 8 kept, the oldest go.
    pub fn notice(mut self, n: Value) -> Self {
        if self.notices.iter().any(|o| o["kind"] == n["kind"] && o["title"] == n["title"] && o["detail"] == n["detail"]) { return self }
        let id = self.notices.iter().filter_map(|o| o["id"].as_u64()).max().unwrap_or(0) + 1;
        let mut n = n; n["id"] = json!(id); n["dismissed"] = json!(false);
        self.notices.push(n);
        if self.notices.len() > 8 { self.notices.remove(0); }
        self
    }
    /// The person dismissed notice `id` in the pane (an agent still reads it).
    pub fn dismiss_notice(mut self, id: u64) -> Self {
        for n in self.notices.iter_mut().filter(|n| n["id"].as_u64() == Some(id)) { n["dismissed"] = json!(true); }
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
    /// KiCad's process ended: `how` its exit ("signal 9 (SIGKILL)", "code 1"; empty: code alone), `tail` its last output lines (kept in
    /// the hub's log only). After it was ready it closed unexpectedly (a crash, a kill): said as that, with how, never its output. Before
    /// it was ready its start failed: the reason is an error line of its output, never start-up noise (Fontconfig, GTK, GLib, Mesa).
    pub fn exited(self, code: i32, tail: &str) -> Self { self.exited_how(code, "", tail) }
    pub fn exited_how(self, code: i32, how: &str, tail: &str) -> Self {
        if self.closing { return self.stopped("") }
        // KiCad quit by itself (its window closed, its own File > Exit): said, so the pane and a later request never read "stopped ()"
        if code == 0 && how.is_empty() { return self.stopped("KiCad was closed") }
        let how = if how.is_empty() { format!("code {code}") } else { how.to_string() };
        if !tail.trim().is_empty() { eprintln!("kicad embed: {} ended ({how}); its last output:\n{}", self.pro, tail.trim_end()) }
        if matches!(self.state.as_str(), "ready" | "unresponsive") {
            let mut s = self.failed(""); s.state = CLOSED.into(); s.reason = format!("KiCad closed unexpectedly ({how})"); return s
        }
        let why = match start_error(tail) { Some(l) => format!("KiCad exited before it was ready ({how}): {l}"), None => format!("KiCad exited before it was ready ({how})") };
        self.failed(&why)
    }
    pub fn closed_unexpectedly(&self) -> bool { self.state == CLOSED }
    /// After a death: KiCad's autosaves of this project's unsaved changes (`_autosave-*` beside its files, bp.13's local history in the
    /// project folder), which KiCad offers back when it opens the file again. The hub never removes them.
    pub fn autosaves(&self) -> Vec<String> {
        let dir = Path::new(&self.pro).parent().map(Path::to_path_buf).unwrap_or_default();
        let mut v: Vec<String> = std::fs::read_dir(&dir).into_iter().flatten().flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|n| n.starts_with("_autosave-")).collect();
        v.sort();
        if history_newer(&dir) { v.push(".history".into()) }
        v
    }
    /// What is not on disk: what KiCad says is modified, and what the hub knows an agent changed and nobody has saved since.
    pub fn unsaved_docs(&self) -> Vec<String> {
        let mut v: Vec<String> = self.frames.iter().filter(|f| f.modified).map(unsaved_name).collect();
        for d in agent_unsaved(&self.pro) { let name = Path::new(&d).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(d); if !v.iter().any(|x| x.ends_with(&name)) { v.push(name) } }
        v
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
            "autosaves": if self.state == CLOSED { json!(self.autosaves()) } else { json!([]) },
            "agent_unsaved": json!(agent_unsaved(&self.pro).iter().map(|d| Path::new(d).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect::<Vec<_>>()),
            "dialogs": self.dialogs,
            "notices": self.notices,
            "drawn": self.drawn,
            "keys": self.keys,
            "rss_mb": self.rss_mb, "peak_mb": self.peak_mb,
            "task_wait": self.task_wait,
            "start_phase": self.start_phase, "start_at": self.start_at, "lock_ask": self.lock_ask,
            // what the embedded editor can do beyond the basics (embed-gerbview, embed-editor-requested, ...): the client gates on these
            "embed_caps": self.caps.iter().filter(|c| c.starts_with("embed-") || c.starts_with("prewarm-") || *c == SIM_FRAME).collect::<Vec<_>>(),
            // bp.9: the client may map and unmap a frame KiCad has shown once (its tab switches need no round trip)
            // not on macOS: KiCad's frames are its own windows there, shown and hidden through the hub (native_child_mac.rs)
            "local_map": !cfg!(target_os = "macos") && self.mode == "fork" && !self.desc.is_empty() && proto::load(&self.desc).has_field("kiapi.backplane.embed.ShowFrame", "prewarm"),
        })
    }
}

/// macOS: why this KiCad cannot be in the pane (None: it can). Its frames are other windows over ours, which needs the fork's
/// macOS embedding (`SetHostGeometry`, capability embed-macos); stock KiCad and older forks (bp.6) open as the KiCad app instead.
pub fn mac_refusal(macos: bool, mode: &str, desc: &str) -> Option<String> {
    if !macos { return None }
    if mode == "fork" && !desc.is_empty() && proto::load(desc).has_own("kiapi.backplane.embed.SetHostGeometry") { return None }
    Some(if mode == "fork" { "this Backplane KiCad cannot be shown in the pane on macOS: Update KiCad in Settings, CAD (it needs embed-macos)" } else { "only Backplane KiCad can be shown in the pane on macOS" }.into())
}
/// `mac_refusal`'s words when this hub's pin would cure it: the update row above the pane offers it (info kicad.update), so the pane
/// points there, not to Settings. `to`: the pin's version when it can be shown in the pane on macOS.
pub fn mac_refusal_with(why: String, to: Option<&str>) -> String {
    match to {
        Some(v) => format!("This KiCad cannot be shown in the pane on macOS. Backplane KiCad {v} can: Update, above the pane, installs it and opens it here."),
        None => why,
    }
}

/// Whether a show of the session's `active` frame is the client's own map (bp.9+ local map, the frame realized), with nothing else asked
/// (`plain`: no document, item or remount).
pub fn local_switch(s: &Session, local_map: bool, plain: bool) -> bool {
    plain && local_map && s.mode == "fork" && s.state == "ready"
        && s.frames.iter().any(|f| f.kind == s.active && f.xid > 0 && (f.realized || f.shown))
}

/// KiCad 10's local history (`<project>/.history`, a git repository of snapshots) holds changes that are not in the saved files, which
/// KiCad offers back ("Recover Unsaved Changes") when it opens the project again. Read from the snapshot itself (`history_unsaved`): a
/// board or schematic in it that differs from the file on disk and was snapshot after that file was written. KiCad snapshots every save
/// too, so a snapshot newer than the files is not enough (after the hub saved an agent's change the pane said a copy of unsaved changes
/// was kept). A history it cannot read (packed, another layout) falls back to that: a snapshot newer than the saved files.
pub fn history_newer(dir: &Path) -> bool {
    if let Some(unsaved) = history_unsaved(dir) { return unsaved }
    let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let head = dir.join(".history").join(".git");
    let snap = ["refs/heads/master", "refs/heads/main", "logs/HEAD"].iter().filter_map(|r| mtime(&head.join(r))).max();
    let saved = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kicad_pcb" || x == "kicad_sch")).filter_map(|p| mtime(&p)).max();
    matches!((snap, saved), (Some(s), Some(f)) if s > f)
}
/// The local history's latest snapshot against the files: Some(true) when a board or schematic in it differs from its file and was taken
/// at or after that file was written (or the file is gone); Some(false) when every one matches or the file is newer; None when the
/// snapshot cannot be read (no history, packed objects): the caller judges by times.
pub fn history_unsaved(dir: &Path) -> Option<bool> {
    let git = dir.join(".history").join(".git");
    let head = std::fs::read_to_string(git.join("HEAD")).ok()?;
    let head = head.trim();
    let sha = match head.strip_prefix("ref: ") {
        Some(r) => std::fs::read_to_string(git.join(r)).ok().map(|s| s.trim().to_string())
            .or_else(|| std::fs::read_to_string(git.join("packed-refs")).ok()?.lines().find(|l| l.ends_with(&format!(" {r}"))).map(|l| l[..40.min(l.len())].to_string()))?,
        None => head.to_string(),
    };
    let (kind, commit) = git_object(&git, &sha)?;
    if kind != "commit" { return None }
    let text = String::from_utf8_lossy(&commit);
    let tree = text.lines().find_map(|l| l.strip_prefix("tree "))?.trim().to_string();
    let when: u64 = text.lines().find_map(|l| l.strip_prefix("committer ")).and_then(|l| l.rsplitn(3, ' ').nth(1)).and_then(|s| s.parse().ok())?;
    let mut files = vec![];
    git_tree_files(&git, &tree, "", &mut files, 0)?;
    let mut any = false;
    for (path, blob) in files.iter().filter(|(p, _)| p.ends_with(".kicad_pcb") || p.ends_with(".kicad_sch")) {
        any = true;
        let f = dir.join(path);
        let Ok(disk) = std::fs::read(&f) else { return Some(true) };
        let (_, snap) = git_object(&git, blob)?;
        let written = std::fs::metadata(&f).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
        if snap != disk && when >= written { return Some(true) }
    }
    if any { Some(false) } else { None }
}
/// A loose object of a git repository (`objects/xx/yyyy`, zlib): its type and body. None for a packed one.
fn git_object(git: &Path, sha: &str) -> Option<(String, Vec<u8>)> {
    use std::io::Read;
    if sha.len() < 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) { return None }
    let raw = std::fs::read(git.join("objects").join(&sha[..2]).join(&sha[2..40])).ok()?;
    let mut out = vec![];
    flate2::read::ZlibDecoder::new(&raw[..]).read_to_end(&mut out).ok()?;
    let nul = out.iter().position(|&b| b == 0)?;
    let kind = String::from_utf8_lossy(&out[..nul]).split(' ').next().unwrap_or("").to_string();
    Some((kind, out[nul + 1..].to_vec()))
}
/// A tree's files, recursively: (path, blob sha).
fn git_tree_files(git: &Path, sha: &str, prefix: &str, out: &mut Vec<(String, String)>, depth: u32) -> Option<()> {
    if depth > 8 { return Some(()) }
    let (kind, body) = git_object(git, sha)?;
    if kind != "tree" { return None }
    let mut i = 0;
    while i < body.len() {
        let sp = i + body[i..].iter().position(|&b| b == b' ')?;
        let nul = sp + body[sp..].iter().position(|&b| b == 0)?;
        if nul + 21 > body.len() { return None }
        let mode = String::from_utf8_lossy(&body[i..sp]).to_string();
        let name = String::from_utf8_lossy(&body[sp + 1..nul]).to_string();
        let id: String = body[nul + 1..nul + 21].iter().map(|b| format!("{b:02x}")).collect();
        let path = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        if mode == "40000" { git_tree_files(git, &id, &path, out, depth + 1)?; } else { out.push((path, id)); }
        i = nul + 21;
    }
    Some(())
}
/// The state of a session whose KiCad died after it was ready (a crash, a kill): the pane offers to open the editor again.
pub const CLOSED: &str = "closed_unexpectedly";
/// Lines of KiCad's start-up output that are noise, never a reason a start failed (the toolkits' warnings on a box without their config).
fn noise(l: &str) -> bool {
    let l = l.trim();
    l.is_empty() || ["Fontconfig", "Gtk-", "GLib", "Gdk-", "dbus", "libEGL", "MESA", "Mesa", "pci id for fd", "at-spi", "AT-SPI", "libGL error", "Xlib", "(kicad", "wxWidgets Debug"]
        .iter().any(|n| l.contains(n))
}
/// The line of a failed start's output that says why: the last that is not noise and reads as an error.
pub fn start_error(tail: &str) -> Option<String> {
    tail.lines().rev().map(str::trim).filter(|l| !noise(l))
        .find(|l| { let s = l.to_ascii_lowercase(); ["error", "fatal", "failed", "cannot", "can't", "could not", "not found", "denied", "abort", "segmentation"].iter().any(|w| s.contains(w)) })
        .map(|l| l.chars().take(200).collect())
}
/// bp.13.1's exit when KiCad's windows went with their container: it saved its autosave and local history first, and says so.
pub fn windows_lost(code: i32, tail: &str) -> bool { code == 3 && tail.contains("windows lost") }
/// How a process ended, for the person: a signal by its name, else its exit code.
pub fn exit_how(status: &std::process::ExitStatus) -> (i32, String) {
    #[cfg(unix)] { use std::os::unix::process::ExitStatusExt; if let Some(sig) = status.signal() {
        let name = match sig { 9 => "SIGKILL", 11 => "SIGSEGV", 6 => "SIGABRT", 15 => "SIGTERM", 7 => "SIGBUS", 4 => "SIGILL", 8 => "SIGFPE", _ => "" };
        return (128 + sig, if name.is_empty() { format!("signal {sig}") } else { format!("signal {sig}, {name}") })
    } }
    let c = status.code().unwrap_or(1);
    (c, if c == 0 { String::new() } else { format!("code {c}") })
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
        // bp.13.2: KiCad's Simulator (capability sim-frame), a frame like the editors with no document of its own
        "SIMULATOR" | "FK_SIMULATOR" | "SIMULATION" | "SIM" => "SIMULATOR",
        _ => "OTHER",
    }
}

/// Default frame kind from the path the client attached with.
pub fn default_frame_for_path(path: &str, requested: &str) -> &'static str {
    if !requested.trim().is_empty() { return canon_frame(requested) }
    if path.ends_with(".kicad_sch") { "SCH" } else { "PCB" }
}

/// The editor to open when the person did not ask for one (a warm-up, a restart with no session before it): `frame` when its document
/// exists, else the project's other editor that has one; None when the project has neither a board nor a schematic. KiCad opening an
/// editor on a missing document asks "PCB '…' does not exist. Do you wish to create it?" over the app (#1326: a schematic-only project's
/// warm-up). The 3D viewer counts as the board's; the editors with no document of their own (symbol and footprint editors, GerbView,
/// the Simulator) are as asked.
pub fn frame_for_documents(frame: &str, has_pcb: bool, has_sch: bool) -> Option<&'static str> {
    let f = canon_frame(frame);
    let has = |k: &str| match k { "PCB" | "PCB_3D" => has_pcb, "SCH" => has_sch, _ => true };
    if has(f) { return Some(f) }
    ["PCB", "SCH"].into_iter().find(|k| has(k))
}
/// The editor an `editor.kicad.attach` opens: the frame it names, exactly (the pane always names one); a path that names a document
/// (.kicad_pcb, .kicad_sch) its own; a frameless attach of a project (.kicad_pro, its folder: an agent's or the hub's own) the board,
/// or the schematic of a project without one, never a missing document (#1326).
pub fn attach_frame(raw_path: &str, requested: &str, has_pcb: bool, has_sch: bool) -> &'static str {
    if !requested.trim().is_empty() || raw_path.ends_with(".kicad_pcb") || raw_path.ends_with(".kicad_sch") { return default_frame_for_path(raw_path, requested) }
    frame_for_documents("PCB", has_pcb, has_sch).unwrap_or("PCB")
}
/// [`frame_for_documents`] for a project's files as they are on disk.
pub fn frame_with_document(frame: &str, files: &Files) -> Option<&'static str> {
    let on_disk = |f: &str| !f.is_empty() && Path::new(f).is_file();
    frame_for_documents(frame, on_disk(&files.pcb), on_disk(&files.sch))
}

/// `kiapi.backplane.embed.FrameKind` enum name for a canonical frame string.
/// The editors pre-warmed after the first one is shown: the board and the schematic, less the one shown and those KiCad has open. Never
/// the 3D viewer: its first paint builds the board's models on KiCad's UI thread (~0.9 s on a small board), a hitch wherever it falls, so
/// it is paid only when the person asks for 3D.
pub fn prewarm_kinds(active: &str, have: &[String]) -> Vec<&'static str> {
    ["SCH", "PCB"].into_iter().filter(|k| *k != canon_frame(active) && !have.iter().any(|h| h == k)).collect()
}

/// The runtime capability of the embedded Simulator frame (bp.13.2), in the headless manifest and the editor's GetCapabilities alike.
pub const SIM_FRAME: &str = "sim-frame";
/// The editor a rail item opens in the pane's KiCad, or None when this runtime (its capabilities) has none for it: Board, Schematic
/// and 3D always; Simulation only where KiCad embeds its Simulator (`sim-frame`), so an older runtime has no entry and no error.
pub fn rail_frame(item: &str, caps: &[String]) -> Option<&'static str> {
    match item.trim().to_ascii_lowercase().as_str() {
        "board" => Some("PCB"),
        "schematic" => Some("SCH"),
        "3d" => Some("PCB_3D"),
        "simulation" => caps.iter().any(|c| c == SIM_FRAME).then_some("SIMULATOR"),
        _ => None,
    }
}
/// Why `kind` cannot be shown by a session with these capabilities, or None: the Simulator needs `sim-frame`.
pub fn frame_refused(kind: &str, caps: &[String]) -> Option<&'static str> {
    (canon_frame(kind) == "SIMULATOR" && !caps.iter().any(|c| c == SIM_FRAME)).then_some("this KiCad has no simulation; needs Backplane KiCad bp.13.2")
}
/// What an unsaved frame is called in the close question: its document, else its title; the Simulator has no document (its `modified`
/// is its workbook's), so it is named when KiCad gave no title.
pub fn unsaved_name(f: &Frame) -> String {
    if !f.doc.is_empty() { f.doc.clone() } else if !f.title.trim().is_empty() { f.title.clone() } else if f.kind == "SIMULATOR" { "Simulation workbook".into() } else { f.title.clone() }
}
/// The selected runtime's capabilities as last read (each attach, warm and restart reads its manifest): what the presenter gates the
/// rail's Simulation entry on. True when they changed (the views are drawn again).
pub fn note_runtime(env: &Env) -> bool { note_runtime_in(runtime_caps_cell(), env) }
/// `note_runtime` on `cell`: the process's one (`runtime_caps_cell`, a hub's) or a test's own. Tests share the process, and every one that
/// attaches or warms an embedded KiCad notes its runtime in the process's cell, so a test of the rule keeps its own (Nov 19: the rail test
/// read another test's note in the full suite).
fn note_runtime_in(cell: &std::sync::Mutex<Vec<String>>, env: &Env) -> bool {
    let mut c = cell.lock().unwrap();
    if *c == env.caps { return false }
    *c = env.caps.clone(); true
}
fn runtime_caps_cell() -> &'static std::sync::Mutex<Vec<String>> { static C: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> = std::sync::OnceLock::new(); C.get_or_init(Default::default) }
/// The rail's Simulation entry for a project folder: the frame it opens ("SIMULATOR"), when the runtime embeds the Simulator and the
/// project has a schematic; None otherwise (no entry).
pub fn rail_simulation(root: &str) -> Option<&'static str> { rail_simulation_in(runtime_caps_cell(), root) }
/// `rail_simulation` with the runtime's capabilities in `cell` (see `note_runtime_in`).
fn rail_simulation_in(cell: &std::sync::Mutex<Vec<String>>, root: &str) -> Option<&'static str> {
    let frame = rail_frame("simulation", &cell.lock().unwrap().clone())?;
    // (asked at every picture: the folder is read at most every 5 s)
    static SEEN: std::sync::OnceLock<std::sync::Mutex<HashMap<String, (std::time::Instant, bool)>>> = std::sync::OnceLock::new();
    let mut seen = SEEN.get_or_init(Default::default).lock().unwrap();
    let has = match seen.get(root) {
        Some((at, has)) if at.elapsed() < Duration::from_secs(5) => *has,
        _ => { let has = resolve_project(root, "").is_ok_and(|r| !r.files.sch.is_empty() && Path::new(&r.files.sch).is_file()); seen.insert(root.to_string(), (std::time::Instant::now(), has)); has }
    };
    has.then_some(frame)
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
/// How long after the person's own switch KiCad's unattributed show requests for another frame are not followed.
pub const SWITCH_HOLD: Duration = Duration::from_millis(2000);
/// The person switched the pane's frame: hold KiCad's show requests for SWITCH_HOLD, a longer hold of the hub's own (a check run) kept.
pub fn hold_shows_after_switch(pro: &str) {
    let until = std::time::Instant::now() + SWITCH_HOLD;
    let mut h = host_holds().lock().unwrap();
    if h.get(pro).map_or(true, |t| *t < until) { h.insert(pro.to_string(), until); }
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
        "SIMULATOR" => "FK_SIMULATOR",
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
        8 => "SIMULATOR",
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

/// Whether a KiCad that is up is started again at an attach: the runtime selected now (`now`, its kicad-cli) is another than the one it
/// runs (`runs`), and nothing in it is unsaved. A session that never said its runtime (a stand-in's) is left alone.
pub fn runtime_moved(runs: &str, now: &str, unsaved: bool) -> bool { !runs.is_empty() && !now.is_empty() && runs != now && !unsaved }

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

/// Least-recently-used detached clean session to evict before starting `keep_pro`'s KiCad when the others running are `max_warm` already
/// (so that with it there are never more than `max_warm` in all).
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
    /// KiCads running at once, in all ([`MAX_WARM`])
    pub max: usize, pub idle_s: u64,
    /// what a KiCad is taken to cost before one has been measured running (KICAD_MB_*)
    pub kicad_mb: u64,
    /// memory left free for everything else once the new KiCad has started ([`warm_headroom_mb`])
    pub headroom_mb: u64,
    /// memory pressure (Linux PSI `full avg10`, percent: every non-idle task waiting for memory) at or above which nothing is warmed
    /// whatever is free: the machine is thrashing already, and a KiCad would push it further into swap
    pub pressure_max: f64,
}
/// What a KiCad costs before one has been measured running on this machine: a medium board's, with the schematic the hub pre-warms by
/// itself after the board shows (every warm KiCad gets there) and room for 3D. The peak of a KiCad running is what protects on a large
/// board: it replaces this as soon as it is higher. bp.12 parsed the global footprint table at start and took 4.5-5.1 GB (the 5,200 MB
/// this replaces). Measured on the published runtime bp.13.2 (edfc943200), Nov 19 2026, medians (Integrator: 3 runs on Linux; Lane M:
/// 6 on macOS), demo boards A (kit-dev-coldfire-xilinx_5213, 160 footprints, 3 sheets) and B (stickhub, 94 footprints):
/// Linux, h@backplane, VmRSS: A board 752 MB, + schematic 1043, + 3D 1705; B board ~700, + schematic ~1000.
pub const KICAD_MB_LINUX: u64 = 2048;
/// macOS, M4 16 GB, `ps` RSS: A board 596 MB, + schematic 719, + 3D 854; B 560 / 638. The hub reads the peak physical footprint, which
/// reads higher than RSS: the Linux figure until the footprint has been measured there.
pub const KICAD_MB_MACOS: u64 = 2048;
/// Windows: not measured yet: the Linux figure.
pub const KICAD_MB_WINDOWS: u64 = 2048;
pub const KICAD_MB: u64 = if cfg!(target_os = "macos") { KICAD_MB_MACOS } else if cfg!(windows) { KICAD_MB_WINDOWS } else { KICAD_MB_LINUX };
/// Free memory (MemAvailable) below which the other editors are not pre-warmed after the first is on show: a pre-warmed schematic took
/// ~1.5 GB on Hubble_DK (bp.12), and the headroom for everything else is kept as the warm-up's was then (4 GB). Unchanged by the warm-up's
/// rule (pinned in a test); read on Linux only, as before ([`prewarm_free_mb`]).
pub const PREWARM_FREE_MB: u64 = 2048 + 4096;
/// The warm-up's defaults; the headroom here is the most it is ([`warm_cfg`] sets this machine's).
pub const WARM: WarmCfg = WarmCfg { max: MAX_WARM, idle_s: 5 * 60, kicad_mb: KICAD_MB, headroom_mb: WARM_HEADROOM_MAX_MB, pressure_max: 10.0 };
pub const WARM_HEADROOM_MIN_MB: u64 = 2048;
pub const WARM_HEADROOM_MAX_MB: u64 = 4096;
/// The room kept for everything else: an eighth of the machine's memory, at least 2 GB and at most 4 GB (4 GB at 32 GB and above, 2 GB at
/// 16 GB and below). Unknown total: the most.
pub fn warm_headroom_mb(total_mb: Option<u64>) -> u64 {
    total_mb.map_or(WARM_HEADROOM_MAX_MB, |t| (t / 8).clamp(WARM_HEADROOM_MIN_MB, WARM_HEADROOM_MAX_MB))
}
/// This machine's warm-up rule: [`WARM`] with the headroom for its memory. BACKPLANE_KICAD_WARM_HEADROOM_MB overrides the headroom and
/// BACKPLANE_KICAD_WARM_PRESSURE_MAX the pressure limit (tests on a shared, loaded box; a forced refusal with a huge headroom).
pub fn warm_cfg() -> WarmCfg {
    let over = std::env::var("BACKPLANE_KICAD_WARM_HEADROOM_MB").ok().and_then(|v| v.parse::<u64>().ok());
    let pressure = std::env::var("BACKPLANE_KICAD_WARM_PRESSURE_MAX").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(WARM.pressure_max);
    WarmCfg { headroom_mb: over.unwrap_or_else(|| warm_headroom_mb(super::mem::total_mb())), pressure_max: pressure, ..WARM }
}
/// The machine's memory as the warm-up reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MemNow {
    /// available (None: this platform cannot say: taken as enough)
    pub free_mb: Option<u64>,
    /// Linux PSI `full avg10` (None: unreadable, taken as no pressure)
    pub pressure: Option<f64>,
}
impl MemNow {
    pub fn read() -> MemNow { MemNow { free_mb: super::mem::free_mb(), pressure: super::mem::pressure_avg10() } }
}
/// The memory line the hub logs at start: what the warm-up will read on this machine (Lane M and Carbon read it on macOS and Windows,
/// where no client asks for a warm-up yet).
pub fn warm_memory_line() -> String {
    let (free, total, cfg) = (super::mem::free_mb(), super::mem::total_mb(), warm_cfg());
    let mb = |v: Option<u64>| v.map_or("unknown".to_string(), |m| format!("{m} MB"));
    format!("kicad embed: memory: free {}, total {}, pressure {}; warm-up needs a KiCad's {} MB (before one is measured) + headroom {} MB",
        mb(free), mb(total), super::mem::pressure_avg10().map_or("unknown".into(), |p| format!("{p:.2}")), cfg.kicad_mb, cfg.headroom_mb)
}

/// How far a KiCad's resident or peak memory moves before the session info is published again from the health tick.
pub const MEM_PUBLISH_STEP_MB: u64 = 64;
/// Whether (rss, peak) has moved `step` MB or more from what was last published (either figure, either way).
pub fn mem_moved(published: (u64, u64), now: (u64, u64), step: u64) -> bool {
    published.0.abs_diff(now.0) >= step || published.1.abs_diff(now.1) >= step
}

/// What the next KiCad will cost, and where the figure comes from: the largest peak of a KiCad running now, never below the default
/// (a KiCad still loading, or a small project, must not make the next look cheap).
pub fn kicad_cost_mb(sessions: &HashMap<String, Session>, cfg: WarmCfg) -> (u64, &'static str) {
    let measured = sessions.values().filter(|s| s.is_live()).map(|s| s.peak_mb).max().unwrap_or(0);
    if measured > cfg.kicad_mb { (measured, "measured") } else { (cfg.kicad_mb, "default") }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Warm {
    /// start KiCad detached (no pane yet), first closing `evict` (a clean, idle one) when the cap is reached
    Start { evict: Option<String> },
    /// it is up (or starting) already: only remembered as used now
    Have,
    /// not now, and why (said in the reply and the hub log; Edit still starts it, cold)
    No(String),
}

/// Whether to warm `pro`. `hardware`: the project has KiCad files; `locked`: a KiCad lock file of the project's is there and no session of
/// ours holds it (the person's own KiCad, or a crash's leftover: KiCad would ask about it in a dialog, which a pre-warm must never raise);
/// `mem`: the machine's memory now; `quiet`: the runtime loads with no window of its own on screen (bp.12's ET_PROGRESS instead of
/// KiCad's progress windows: an older one shows "Load PCB" over Backplane when a thread is merely shown).
pub fn warm_decision(sessions: &HashMap<String, Session>, pro: &str, hardware: bool, locked: bool, mem: MemNow, quiet: bool, cfg: WarmCfg) -> Warm {
    let no = |w: &str| Warm::No(w.to_string());
    if !hardware { return no("not a KiCad project") }
    if !quiet && !sessions.get(pro).is_some_and(Session::is_live) { return no("this KiCad shows its own windows while loading (needs Backplane KiCad bp.12)") }
    if sessions.get(pro).is_some_and(Session::is_live) { return Warm::Have }
    // a session that failed or was closed by the person is not started again behind their back; Edit does that
    if sessions.get(pro).is_some_and(|s| s.state == "failed" || s.state == CLOSED || s.reason == "KiCad was closed") { return no("KiCad stopped for this project; Edit starts it") }
    if locked { return no("the project is open in another KiCad") }
    // memory: the machine already waiting for memory (a build growing) refuses whatever the numbers; else the new KiCad's cost and the
    // headroom must fit in what is available (builds and other programs take theirs first). Room made by an eviction is not counted on:
    // a refused warm costs only Edit's cold start
    if let Some(p) = mem.pressure.filter(|p| *p >= cfg.pressure_max) {
        return Warm::No(format!("memory pressure (full avg10 {p:.2} >= {:.0})", cfg.pressure_max))
    }
    let (cost, src) = kicad_cost_mb(sessions, cfg);
    if let Some(free) = mem.free_mb.filter(|m| *m < cost + cfg.headroom_mb) {
        return Warm::No(format!("too little free memory (free {free} MB < KiCad {cost} MB ({src}) + headroom {} MB)", cfg.headroom_mb))
    }
    let live = sessions.values().filter(|s| s.pro != pro && s.is_live()).count();
    if live < cfg.max { return Warm::Start { evict: None } }
    match evict_warm(sessions, pro, cfg.max) { Some(v) => Warm::Start { evict: Some(v) }, None => no("as many KiCads as are kept warm are in use") }
}

/// The free memory the post-Edit pre-warm reads: Linux only, as before the warm-up's rule (elsewhere unknown, so never held back): a real
/// reading on macOS (free + inactive + purgeable) would hold the pre-warm back on a 16 GB Mac, a change not made here.
pub fn prewarm_free_mb() -> Option<u64> { if cfg!(target_os = "linux") { super::mem::free_mb() } else { None } }
/// The pre-warm's memory gate: Some(free) when `free` is under `floor` (refused), None when allowed or unknown.
pub fn prewarm_refused(free: Option<u64>, floor: u64) -> Option<u64> { free.filter(|f| *f < floor) }

/// How long after the last desktop client that used KiCad went before its KiCads are closed: a reconnect (a new connection id) is not a close.
pub const APP_GONE_S: u64 = 15;
/// The sessions to close when the app that showed them is gone: every live one with nothing unsaved (unsaved work is kept: the app
/// opened again takes it back into its pane).
pub fn app_gone_victims(sessions: &HashMap<String, Session>) -> Vec<String> {
    let mut v: Vec<String> = sessions.values().filter(|s| s.is_live() && !s.closing && s.unsaved_docs().is_empty()).map(|s| s.pro.clone()).collect();
    v.sort(); v
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

/// What a starting KiCad waited for, from its cgroup's pressure stall totals (its memory scope holds only it, so the totals are its
/// start's) and the machine's load: a slow open on a busy machine says so in the hub log (Nov 13: Day_11 took 10.3 s on the person's
/// desktop during main-build's tests, 3.4 s on a quiet test instance). Linux; None elsewhere or when unreadable.
fn start_waits(pid: u32) -> Option<String> {
    // Windows has no stall totals: what the process used against how long it has run (a start held by Defender shows a long age with
    // little CPU and reading; bp-win, Carbon, Nov 13: the retry line said "waited ?")
    #[cfg(windows)]
    { let (cpu, read, wrote, age) = crate::plat::proc::usage(pid)?;
      return Some(format!("{age:.1} s running, cpu {cpu:.2} s, read {:.0} MB, wrote {:.0} MB (its own)", read as f64 / 1e6, wrote as f64 / 1e6)) }
    #[allow(unreachable_code)]
    if !cfg!(target_os = "linux") || pid == 0 { return None }
    let cg = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    let dir = format!("/sys/fs/cgroup{}", cg.lines().find_map(|l| l.strip_prefix("0::"))?.trim());
    let some = |r: &str| std::fs::read_to_string(format!("{dir}/{r}.pressure")).ok().and_then(|t| stall_total_us(&t)).map(|us| format!("{:.2}", us as f64 / 1e6)).unwrap_or_else(|| "?".into());
    let load = std::fs::read_to_string("/proc/loadavg").ok().and_then(|t| t.split_whitespace().next().map(str::to_string)).unwrap_or_else(|| "?".into());
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0);
    Some(format!("cpu {} s, io {} s, memory {} s (stall totals of its scope); load {load} on {cpus} cpus", some("cpu"), some("io"), some("memory")))
}
/// The `some` line's `total=` (microseconds) of a cgroup `*.pressure` file.
pub fn stall_total_us(text: &str) -> Option<u64> {
    text.lines().find(|l| l.starts_with("some "))?.split_whitespace().find_map(|f| f.strip_prefix("total="))?.parse().ok()
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

/// The wheel zooms at the pointer in the KiCad whose settings are in `dir` (`<config>/kicad/<version>`): `input.center_on_zoom` off in its
/// `kicad_common.json`, set once (the mark `.bp-wheel-zoom` beside it), so what the person then chooses in KiCad's Preferences stays.
/// KiCad's default centres the view on the pointer and warps the pointer to the middle; under XWayland, and in a pane, the warp does not
/// happen, so each wheel step moved the view to somewhere the pointer was not. No file yet (KiCad never ran here): a small one is made,
/// which KiCad fills in.
pub fn wheel_zoom_once(dir: &Path) {
    let mark = dir.join(".bp-wheel-zoom");
    if mark.exists() || std::fs::create_dir_all(dir).is_err() { return }
    let f = dir.join("kicad_common.json");
    let wrote = match std::fs::read_to_string(&f) {
        Ok(t) => match serde_json::from_str::<Value>(&t).ok().filter(Value::is_object) {
            Some(mut c) => {
                if !c["input"].is_object() { c["input"] = json!({}); }
                if c["input"]["center_on_zoom"] == json!(false) { true } else { c["input"]["center_on_zoom"] = json!(false); std::fs::write(&f, serde_json::to_string_pretty(&c).unwrap_or_default()).is_ok() }
            }
            None => false, // not JSON we know: left as it is, and looked at again next time
        },
        Err(_) => std::fs::write(&f, serde_json::to_string_pretty(&json!({"meta": {"filename": "kicad_common.json", "version": 6}, "input": {"center_on_zoom": false}})).unwrap_or_default()).is_ok(),
    };
    if wrote { let _ = std::fs::write(&mark, ""); }
}

/// The side panels a seeded profile starts with hidden, by KiCad 10's own keys.
static SEEDED_AUI: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| json!({
    "design_blocks_panel_docked_width": 120,
    "design_blocks_show": false,
    "show_layer_manager": false,
    "show_net_inspector": false,
    "show_properties": false,
    "show_search": false,
    "show_schematic_hierarchy": false,
    "hierarchy_panel_docked_width": 120
}));

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
        // the wheel zooms at the pointer. KiCad's default centres the view on the pointer and warps the pointer to the middle; under
        // XWayland (and in a pane) the warp does not happen, so every wheel step moved the view somewhere the pointer was not
        "input": { "center_on_zoom": false },
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
        // KiCad 10's own keys (as it writes them in pcbnew.json / eeschema.json): the side panels hidden, the canvas the pane (stock
        // KiCad has no embed-autopanels). The names before Nov 16 (show_properties_panel, show_search_panel, show_design_blocks_panel,
        // show_hierarchy_panel) were not KiCad's: only the layer manager and the net inspector were hidden
        "aui": SEEDED_AUI.clone(),
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
        wheel_zoom_once(&dir);
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

pub(crate) fn norm_pro_key(pro: &str) -> String {
    // one project however its path is spelled (a symlinked folder, macOS's /var for /private/var): the open KiCad's route, its unsaved
    // state and its frames are found from any spelling, so an agent's write never misses the open editor and goes around it
    let pro = &super::canonical::same_file(pro);
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

static NOTES: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
fn session_notes() -> &'static Mutex<HashMap<String, Value>> { NOTES.get_or_init(Default::default) }
/// What an agent asking the session's state reads beside its server: KiCad's open dialogs and its notices (`notice_of`), and whether
/// this KiCad reports its dialogs at all (on Windows only with `embed-dialog-events`: without it an empty list says nothing). Null: no session.
pub fn session_state(pro: &str) -> Value { session_notes().lock().unwrap().get(&norm_pro_key(pro)).cloned().unwrap_or(Value::Null) }

/// Active `"ready"` GUI session route for `pro`, if any (`ipc::run` checks this before headless `kserve`).
pub fn gui_route(pro: &str) -> Option<GuiRoute> {
    routes().lock().unwrap().get(&norm_pro_key(pro)).cloned()
}

// The person's unsaved edits (the person's rule, Nov 11): an agent's save never writes what the person has open and unsaved. Per project:
// whether a document is modified (from each publish: GetFrames in fork mode, the client's titles in stock mode), whether the agent's own
// commits changed it since the last save, and whether the person changed it. A co-editing runtime (`coedit`, bp.11+) says who made each
// change (ET_DOCUMENT_EDITED's actor): there only the person's own change counts (#1169: a pan, a click, a selection or the keyboard
// handed to KiCad changes nothing, and once counted it blocked every agent save). A runtime that says no actor (bp.10, stock) cannot
// tell: there the person having had the editor (a press or the keyboard, from the client), or a change pending that no agent made when
// an agent's commit begins, still counts. All three clear when nothing is modified any more (a save, a revert).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Unsaved { pub modified: bool, pub agent: bool, pub person: bool }
impl Unsaved {
    /// What a publish says: nothing modified clears everything.
    pub fn seen(self, modified: bool) -> Self { if modified { Unsaved { modified, ..self } } else { Unsaved::default() } }
    /// The person took the editor (a press or the keyboard handed to KiCad): counted only where KiCad says no actor (`touched_counts`).
    pub fn touched(self) -> Self { Unsaved { person: true, ..self } }
    /// The person's own change, as a co-editing KiCad attributes it (ET_DOCUMENT_EDITED, actor "" or `person:`).
    pub fn person_edited(self) -> Self { Unsaved { person: true, modified: true, ..self } }
    /// An agent's BeginCommit on a runtime that says no actor: edits already pending that no agent made are the person's. `agents_pending`:
    /// the hub's record has an agent's change not on disk in this project (`agent_unsaved`, any thread's): those are not the person's.
    pub fn agent_begins(self, agents_pending: bool) -> Self { Unsaved { person: self.person || (self.modified && !self.agent && !agents_pending), ..self } }
    /// An agent's commit landed.
    pub fn agent_committed(self) -> Self { Unsaved { agent: true, modified: true, ..self } }
    /// Why an agent's save is refused ("" when it may save): the document is modified and the person may have edits in it.
    pub fn refuse_save(self, file: &str) -> String {
        if self.modified && self.person { format!("Not saved: the person may have unsaved edits of their own in KiCad ({file}), and an agent's save would write them too. Your change is already made in the open editor: do not send it again (that would make it twice). It is written to the file when the person saves in KiCad (Ctrl+S); tell them so.") } else { String::new() }
    }
}
static UNSAVED: OnceLock<Mutex<HashMap<String, Unsaved>>> = OnceLock::new();

// An agent's changes in the open editor that are not on disk, the hub's own record (KiCad does not mark a document modified for an API
// commit, bp.13 batch 14): per project, each document an agent changed and when. A change is on disk once its file was written after it
// (the hub's save after the commit, the agent's own SaveDocument, the person's Ctrl+S), so nothing clears it but the file.
static AGENT_CHANGES: OnceLock<Mutex<HashMap<String, Vec<(String, std::time::SystemTime)>>>> = OnceLock::new();
fn agent_changes() -> &'static Mutex<HashMap<String, Vec<(String, std::time::SystemTime)>>> { AGENT_CHANGES.get_or_init(Default::default) }
/// An agent changed `doc` (a board or schematic file) in the project's open KiCad now.
pub fn agent_changed(pro: &str, doc: &str) {
    let mut m = agent_changes().lock().unwrap(); let v = m.entry(norm_pro_key(pro)).or_default();
    v.retain(|(d, _)| d != doc); v.push((doc.to_string(), std::time::SystemTime::now()));
}
/// The documents with an agent's change that is not on disk (its file not written since).
pub fn agent_unsaved(pro: &str) -> Vec<String> {
    let mut m = agent_changes().lock().unwrap();
    let Some(v) = m.get_mut(&norm_pro_key(pro)) else { return vec![] };
    v.retain(|(d, at)| std::fs::metadata(d).and_then(|x| x.modified()).map_or(true, |w| w < *at));
    v.iter().map(|(d, _)| d.clone()).collect()
}
/// Whether the person having the editor counts as their edit (#1169): only where KiCad does not say who changed what (no `coedit`); a
/// co-editing KiCad's ET_DOCUMENT_EDITED does (`is_persons_edit`). No editor route yet: counted, as before.
pub fn touched_counts(pro: &str) -> bool { !gui_route(pro).is_some_and(|g| g.caps.iter().any(|c| c == "coedit")) }
/// ET_DOCUMENT_EDITED made by the person (actor "" in KiCad, or Backplane's UI acting as them: `person:`), their undo and redo included;
/// an agent's (`agent:`) or the hub's own (`hub:`) is not.
pub fn is_persons_edit(ev: &Value) -> bool {
    if !(ev["type"] == "ET_DOCUMENT_EDITED" || ev["type"].as_i64() == Some(20)) { return false }
    let Ok(d) = serde_json::from_str::<Value>(ev["detail"].as_str().unwrap_or("")) else { return false };
    let a = d["actor"].as_str().unwrap_or(""); a.is_empty() || a.starts_with("person:")
}
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
/// Whether each project's ready KiCad is in a client's pane (attached to a container, not parked), kept at every publish (put_route).
fn attached_panes() -> &'static Mutex<HashMap<String, bool>> { static P: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new(); P.get_or_init(Default::default) }
fn attach_clients() -> &'static Mutex<HashMap<String, u64>> { static C: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new(); C.get_or_init(Default::default) }
/// The client whose pane the project's ready KiCad is in now (attached to its container, not parked), for Follow agent (kicad/follow.rs):
/// None when it is in no pane. No round trip.
pub fn pane_client(pro: &str) -> Option<u64> {
    let k = norm_pro_key(pro);
    if !attached_panes().lock().unwrap().get(&k).copied().unwrap_or(false) { return None }
    attach_clients().lock().unwrap().get(&k).copied()
}
/// Projects whose GUI KiCad is starting or closing: the hub starts no headless KiCad for them meanwhile (it would take the project's lock
/// first, and the GUI KiCad would ask "File Open Warning: already open" outside the pane: the release check's fresh-home stall, Nov 16).
fn gui_holding() -> &'static Mutex<std::collections::HashSet<String>> { static H: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new(); H.get_or_init(Default::default) }
/// Whether the project's GUI KiCad is starting or closing (not ready, not gone): a headless KiCad must not be started for it now.
pub fn gui_starting(pro: &str) -> bool { gui_holding().lock().unwrap().contains(&norm_pro_key(pro)) }
fn put_route(s: &Session, tx: &mpsc::UnboundedSender<Msg>) {
    { let mut h = gui_holding().lock().unwrap(); let k = norm_pro_key(&s.pro);
      if s.state == "starting" || (s.closing && s.is_live()) { h.insert(k); } else { h.remove(&k); } }
    session_notes().lock().unwrap().insert(norm_pro_key(&s.pro), json!({"dialogs": s.dialogs, "notices": s.notices,
        "dialogs_reported": !cfg!(windows) || s.caps.iter().any(|c| c == "embed-dialog-events")}));
    open_frames().lock().unwrap().insert(norm_pro_key(&s.pro), s.frames.iter().map(|f| f.kind.clone()).collect());
    attached_panes().lock().unwrap().insert(norm_pro_key(&s.pro), s.state == "ready" && s.container != 0 && !s.parked);
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

/// Open the project's `kind` editor ("SCH", "PCB") hidden in its GUI KiCad (a pre-warm), for the hub's own edit of a document that KiCad
/// has not opened: never a headless KiCad beside the editor's (it would hold the document's lock, and the editor's KiCad would open over
/// it or ask outside the pane). `doc`'s lock is read first: another live KiCad's refuses (the hub's edit is not the person's to override).
pub async fn open_hidden(gui: &GuiRoute, kind: &str, doc: &str) -> Result<(), String> {
    // a document that is not there: KiCad would ask whether to create it, out of sight (#1326)
    if !Path::new(doc).is_file() { return Err(format!("{doc} does not exist: KiCad was not asked to open it")) }
    if gui.mode != "fork" || gui.desc.is_empty() || !proto::load(&gui.desc).has_field("kiapi.backplane.embed.ShowFrame", "prewarm") {
        return Err("this KiCad cannot open the document out of sight (Backplane KiCad bp.12 or later does): open it in the editor, then try again".into())
    }
    match lock_verdict(doc) {
        v @ super::locks::Verdict::Foreign { .. } => return Err(format!("{} is open in {}: not changed", Path::new(doc).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(), super::locks::holder_words(&v))),
        super::locks::Verdict::Stale => { super::locks::clear_stale(doc, "before the editor's KiCad opens it out of sight"); }
        _ => {}
    }
    hold_shows(&gui.pro, Duration::from_secs(60));
    let r = gui_call(gui, "kiapi.backplane.embed.ShowFrame", json!({"kind": fk_of(kind), "prewarm": true})).await;
    release_shows(&gui.pro);
    if r["ok"] != true { return Err(format!("the editor's KiCad did not open it: {}", r["error"].as_str().unwrap_or(""))) }
    Ok(())
}
/// The same, as `client` (an agent's actor, `agent:<thread>`): the fork's co-editing attributes, groups and undoes its changes by it.
/// The hub's commands that never change KiCad's selection: they do not make a later selection the hub's.
const NEVER_SELECTS: &[&str] = &["SetAgentActivity", "SetHostMarkers", "RunChecks", "HideFrame", "AttachToParent", "Shutdown", "SaveDocument", "SetVisibleLayers", "SetActiveLayer"];
pub async fn gui_call_as(route: &GuiRoute, ty: &str, msg: Value, client: &str) -> Value { gui_call_in(route, ty, msg, client, None).await }
/// How long a read waits on KiCad's task gate (a check or a refill running: bp.13 answers only re-entrant-safe reads meanwhile, AS_BUSY
/// "a task is running: <what> (token <t>, ...)" for the rest, GetPlan with its derived data among them) before it is answered in words.
pub const READ_TASK_WAIT: Duration = Duration::from_secs(30);
/// gui_call_as for a request that waits out KiCad's task (GetTaskStatus with its token) for at most `max`, then is sent again: an agent's
/// read, the plan tools', the Plan panel's. A task gate's refusal is no failure of the request (nothing was done); past `max` it is
/// answered with what KiCad is busy with and to ask again (`busy_words`), its status still AS_BUSY.
pub async fn gui_call_patient(route: &GuiRoute, ty: &str, msg: Value, client: &str, max: Duration) -> Value {
    let o = gui_call_in(route, ty, msg, client, Some(max)).await;
    match task_refusal(&o) { Some((what, _)) => json!({"ok": false, "status": "AS_BUSY", "error": busy_words(&what, max), "task": what}), None => o }
}
/// What an agent is told when KiCad's task outlasts its wait.
pub fn busy_words(what: &str, waited: Duration) -> String {
    format!("KiCad is busy with {what} and answers only some reads until it ends; it was still running after {} s. Nothing was done: ask again in a moment.", waited.as_secs())
}
async fn gui_call_in(route: &GuiRoute, ty: &str, msg: Value, client: &str, patient: Option<Duration>) -> Value {
    // every request but the checks' own is counted, so an in-editor check never starts while another client waits (kicad/checkrun.rs)
    // a named client's (an agent, the person's edit from a panel or a phone) also holds checks a moment after it ends; the hub's own
    // unnamed reads (the live BOM's poll, every second) only while in flight
    struct Use(String, bool);
    impl Drop for Use { fn drop(&mut self) { if let Some(e) = gui_use().lock().unwrap().get_mut(&self.0) { e.0 = e.0.saturating_sub(1); if self.1 { e.1 = crate::hub::now_ms() } } } }
    // a command of ours that can change KiCad's selection (any request but a read): what KiCad then selects is not the person's
    if !super::ipc::is_read(ty) && !NEVER_SELECTS.iter().any(|n| ty.rsplit('.').next() == Some(*n)) { super::coedit::hub_command(&route.pro) }
    let named = !client.is_empty();
    let _use = (client != CHECKS_CLIENT).then(|| { let mut m = gui_use().lock().unwrap(); let e = m.entry(route.sock.clone()).or_default(); e.0 += 1; if named { e.1 = crate::hub::now_ms() } Use(route.sock.clone(), named) });
    if TASK_GATED.iter().any(|g| ty.rsplit('.').next() == Some(*g)) && has_task_gate(&route.sock) { return raw_gated(&route.tx, &route.sock, &route.desc, ty, msg, 30000, client, None).await }
    if let Some(max) = patient { return raw_gated_for(&route.tx, &route.sock, &route.desc, ty, msg, 30000, client, None, true, max, false).await }
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
    // bounded (#1160): a describe that hangs is killed, asked once more, and not remembered, so a later attach asks again
    let out = (0..2).find_map(|_| { let (mut c, _home) = crate::proc::kicad_probe(cli); c.args(["api-server", "--describe"]); crate::proc::probe_output(c, crate::proc::PROBE_LIMIT) });
    let ran = out.is_some();
    let parsed = out.filter(|o| o.status.success())
        .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok());
    if ran { CACHE.get_or_init(Default::default).lock().unwrap().insert(cli.to_string(), parsed.clone()); }
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
    /// The first start's KiCad is gone (start_retry): start the second, on the placeholder generation the session waited under.
    RetryNow { pro: String, gen: u64, client: u64, env: Env, waited: String },
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
    /// a gated request on `sock` waits on KiCad's task (`what`), or no longer ("")
    TaskWait { sock: String, what: String },
    StartPhase { pro: String, gen: u64, what: String },
    /// a launch or a document open waits on the person: another live KiCad holds `ask.file` (locks.rs); `answer` gets true for Open anyway
    LockAsk { pro: String, gen: u64, ask: Value, answer: oneshot::Sender<bool> },
    /// the person's answer from the pane (editor.kicad.lock)
    LockAnswer { pro: String, open: bool },
    FramesUpdated { pro: String, gen: u64, holder: Option<u64>, frames: Vec<Frame>, reply: Option<(u64, oneshot::Sender<Result<Value, String>>)> },
    /// `editor`: KiCad's ET_EDITOR_REQUESTED (a library editor with the item it is for), not a plain ET_SHOW_REQUESTED
    ForkShowRequested { pro: String, gen: u64, kind: String, item: String, editor: bool, actor: String },
    Dialog { pro: String, gen: u64, open: bool, title: String },
    /// the dialogs of KiCad's the client sees on screen (a modal one stops KiCad's events and pings: this is how the hub learns of it)
    ClientDialogs { pro: String, open: Vec<String> },
    /// KiCad's ET_NOTICE (`notice_of`)
    Notice { pro: String, gen: u64, notice: Value },
    /// KiCad's ET_FRAME_DRAWN for this window (`embed-frame-drawn`)
    FrameDrawn { pro: String, gen: u64, window: u64, drawn: bool },
    /// macOS: a reserved chord pressed in KiCad (ET_RESERVED_KEY), for the client (Session.keys)
    ReservedKey { pro: String, gen: u64, seq: u64, index: u64 },
    /// the person dismissed a notice in the pane
    NoticeDismiss { pro: String, id: u64 },
    ForkSync { pro: String, gen: u64 },
    /// KiCad says it lost its parent (ET_DETACHED): the client that held the pane is gone (killed or quit; X destroyed its container)
    ForkDetached { pro: String, gen: u64 },
    /// ForkSync for the project's current session, whichever generation it is (from outside the actor: the prewarm RPC)
    ForkSyncNow { pro: String },
    HealthResult { pro: String, gen: u64, ok: bool, modified_docs: Option<Vec<String>> },
    ChildExited { pro: String, gen: u64, code: i32, how: String, tail: String, reply: Option<oneshot::Sender<Result<Value, String>>> },
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
    /// the runtime each project's latest start used, and the projects whose start was already retried once (start_retry)
    start_env: HashMap<String, Env>,
    start_retried: std::collections::HashSet<String>,
    /// the launch or document open waiting on the person's answer to a foreign lock, by project
    lock_waits: HashMap<String, oneshot::Sender<bool>>,
    /// the desktop clients that asked for KiCad (an attach, a show, a warm) and are still connected, and since when there has been none
    /// after there was one: the app closed, so its KiCads close too (APP_GONE_S)
    users: HashSet<u64>,
    users_gone: Option<u64>,
}

/// Whether a failed start is tried once more by itself: only a missed start window (KiCad ran but its socket or its first answer came after
/// 30 s), once per project until a start succeeds. On Windows real-time scanning holds a new runtime's or a fresh home's files on the first
/// run (bp-win: Defender's cloud lookups at each "did not open its IPC socket within 30 s"); the second start finds them scanned. A crash,
/// a missing program or an unprepared folder is not retried: it would fail the same way.
fn start_retry(why: &str, retried: bool) -> bool { !retried && why.starts_with("KiCad did not") && why.contains(" within ") }
/// macOS: KiCad's API answers "not ready" while its UI thread waits on a system prompt. The hub is KiCad's responsible process, so the prompt
/// names Backplane ("would like to access files in your Documents folder", a project on the Desktop or a removable volume). After
/// MAC_PROMPT_AFTER of that with no KiCad dialog open, the start waits for the person (the pane says so, MAC_PROMPT_REASON) up to
/// MAC_PROMPT_BOUND, and never restarts KiCad into the same prompt: its failure says "KiCad waited", which `start_retry` does not retry.
/// (The Mac mini gate, Nov 16: the first Edit stalled 30 s on the Documents prompt, then a second KiCad met the same prompt.)
pub const MAC_PROMPT_REASON: &str = "mac-permission-prompt";
const MAC_PROMPT_AFTER: Duration = Duration::from_secs(5);
const MAC_PROMPT_BOUND: Duration = Duration::from_secs(600);
pub fn mac_prompt_wait(macos: bool, not_ready_for: Option<Duration>, kicad_dialog_open: bool) -> bool {
    macos && !kicad_dialog_open && not_ready_for.is_some_and(|d| d >= MAC_PROMPT_AFTER)
}
/// macOS: KiCad's own offer to recover unsaved work ("KiCad found unsaved changes from your last session…", Restore / Discard) is a native
/// alert shown before its API is up, so no dialog event reaches the hub and the stall looks like a system prompt. When the project's folder
/// holds what KiCad offers back (its `_autosave-*` files, or a local-history snapshot newer than the saved files: `history_newer`), the
/// stall is KiCad's question, said as such (MAC_RECOVER_REASON); the permission words only when there is nothing to recover.
/// (The Mac mini gate, Nov 16: after a kill the pane said "macOS is asking for permission" over KiCad's recovery alert.)
pub const MAC_RECOVER_REASON: &str = "kicad-recover-prompt";
pub fn mac_wait_reason(project_dir: &Path) -> &'static str {
    let kept = std::fs::read_dir(project_dir).into_iter().flatten().flatten()
        .any(|e| e.file_name().to_str().is_some_and(|n| n.starts_with("_autosave-")));
    if kept || history_newer(project_dir) { MAC_RECOVER_REASON } else { MAC_PROMPT_REASON }
}
pub fn mac_prompt_failure(bound: Duration) -> String {
    format!("KiCad waited {} min on a macOS permission prompt that was not answered (answer it, then Try again)", bound.as_secs() / 60)
}
/// How long the start retry waits for the first KiCad after asking it to close (kill_pid's grace: a KiCad that missed its start window
/// gets no more), then after killing it.
const RETRY_TERM_WAIT: Duration = Duration::from_millis(1500);
const RETRY_KILL_WAIT: Duration = Duration::from_secs(5);

/// Ends the first start's KiCad before the retry starts another on the same project: asked to close, killed if it is still there after
/// `term_wait`, then waited for up to `kill_wait`. Says what it took, for the log; a KiCad lock file left in the project is named (the
/// second start sweeps them, as after a crash).
async fn gone_before_retry(pid: u32, root: &str, pro: &str, term_wait: Duration, kill_wait: Duration) -> String {
    use crate::plat::proc;
    let t0 = std::time::Instant::now();
    let gone_within = |limit: Duration| async move {
        let end = tokio::time::Instant::now() + limit;
        while tokio::time::Instant::now() < end { if !proc::group_alive(pid) { return true } tokio::time::sleep(Duration::from_millis(50)).await; }
        !proc::group_alive(pid)
    };
    let how = if pid == 0 || !proc::group_alive(pid) { "was already gone".to_string() } else {
        if !proc::kill_group(pid, proc::TERM) { let _ = proc::kill(pid, proc::TERM); }
        if gone_within(term_wait).await { format!("closed in {} ms", t0.elapsed().as_millis()) } else {
            if !proc::kill_group(pid, proc::KILL) { let _ = proc::kill(pid, proc::KILL); }
            if gone_within(kill_wait).await { format!("did not close in {} ms and was killed (gone at {} ms)", term_wait.as_millis(), t0.elapsed().as_millis()) }
            else { format!("was still there {} ms after it was killed", t0.elapsed().as_millis()) }
        }
    };
    let mut locks: Vec<String> = Vec::new();
    for dir in [Some(Path::new(root)), Path::new(pro).parent()].into_iter().flatten() {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with('~') && n.ends_with(".lck") && !locks.contains(&n) { locks.push(n); }
        }
    }
    if locks.is_empty() { how } else { format!("{how}, leaving {} (swept)", locks.join(", ")) }
}

/// How long a GUI start waits for the project's headless KiCad to finish an agent's unsaved work (it stops itself once saved).
const HANDOVER_WAIT: std::time::Duration = std::time::Duration::from_secs(if cfg!(test) { 2 } else { 60 });
/// How long a start has for KiCad's socket and its first answers (a dialog of KiCad's own open meanwhile extends it); short under test.
const START_WINDOW_S: u64 = if cfg!(test) { 3 } else { 30 };
/// The start window in force (BACKPLANE_KICAD_START_WINDOW_S for an e2e), and whether this start is one an e2e makes stall: its socket
/// never seen (BACKPLANE_KICAD_TEST_STALLS=N, the first N starts of this hub; warm-before-Edit starts count, so such a run sets
/// BACKPLANE_KICAD_WARM=0).
fn start_window(pro: &str) -> (u64, bool) {
    static STARTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let window = std::env::var("BACKPLANE_KICAD_START_WINDOW_S").ok().and_then(|v| v.parse().ok()).filter(|v| *v > 0).unwrap_or_else(|| test_window(pro).unwrap_or(START_WINDOW_S));
    let stalls = std::env::var("BACKPLANE_KICAD_TEST_STALLS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    (window, STARTS.fetch_add(1, Ordering::SeqCst) < stalls)
}

/// A test that starts a real KiCad gives the projects under its own folder the window a real start needs (`test_window_for`); the stand-in tests keep the short one.
/// Per folder, not an environment variable: the tests share one process, and the stand-ins' short window must stay theirs.
#[cfg(test)] fn test_windows() -> &'static Mutex<Vec<(String, u64)>> { static W: OnceLock<Mutex<Vec<(String, u64)>>> = OnceLock::new(); W.get_or_init(|| Mutex::new(vec![])) }
#[cfg(test)] pub(crate) fn test_window_for(dir: &Path, secs: u64) {
    let mut w = test_windows().lock().unwrap();
    for d in [Some(dir.to_path_buf()), dir.canonicalize().ok()].into_iter().flatten() { w.push((format!("{}/", d.to_string_lossy().trim_end_matches('/')), secs)); }
}
#[cfg(test)] fn test_window(pro: &str) -> Option<u64> { test_windows().lock().unwrap().iter().find(|(d, _)| pro.starts_with(d.as_str())).map(|(_, s)| *s) }
#[cfg(not(test))] fn test_window(_pro: &str) -> Option<u64> { None }

impl Actor {
    /// KiCads of the hub's running now (any state but stopped or failed).
    fn live_count(&self) -> usize { self.sessions.values().filter(|s| s.is_live()).count() }
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
    /// A connected client asked for KiCad (the hub's own requests carry no connection: they keep no KiCad up).
    fn note_user(&mut self, client: u64) { if self.hub.is_attached(client) { self.users.insert(client); self.users_gone = None; } }
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
        // which client's pane holds this project's KiCad (Follow agent: kicad/follow.rs follows into that window's pane only)
        if container > 0 { attach_clients().lock().unwrap().insert(norm_pro_key(&pro), client); }
        // the runtime selected now is another than the one this project's KiCad runs (Backplane KiCad installed or updated since it started,
        // the path set in Settings): Open in KiCad gets the selected one, not the stock KiCad left running from before the install. One
        // with unsaved work is kept as it is
        if self.sessions.get(&pro).is_some_and(|s| s.state == "ready" && runtime_moved(&s.cli, &env.cli, s.frames.iter().any(|f| f.modified))) {
            let mut s = self.sessions.get(&pro).cloned().expect("checked");
            eprintln!("kicad embed: {pro}: started again on the selected runtime {} (it ran {})", env.cli, s.cli);
            s.closing = true;
            Self::kill_pid(s.pid);
            self.cleanup_session_sockets(&s);
            let stopped = s.stopped("restarting");
            put_route(&stopped, &self.me);
            self.sessions.insert(pro.clone(), stopped);
        }
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

        let sweep_locks = self.sessions.get(&pro).is_some_and(|s| s.state == "failed" || s.state == CLOSED || s.reason == "restarting");
        self.start_env.insert(pro.clone(), env.clone());

        // Warm-session cap: retire the oldest detached clean session before starting a 4th GUI KiCad.
        if let Some(victim) = evict_warm(&self.sessions, &pro, MAX_WARM) {
            eprintln!("kicad embed: evicting {victim}: {} KiCads running, the cap is {MAX_WARM} in all; room for {pro} (Edit)", self.live_count());
            self.retire(&victim);
        }

        let kicad_bin = match kicad_gui_bin(&env.cli) {
            // Backplane KiCad on a computer whose glibc is at least its own: started on the computer's loader and graphics driver
            // (hostlibs.rs: the runtime's own glibc loads no driver here, so KiCad drew in software and its 3D viewer drew nothing)
            Ok(b) => b.parent().and_then(Path::parent).filter(|_| b.with_file_name("kicad.real").is_file())
                .and_then(|root| super::hostlibs::gui_program(root, &self.hub.cfg.home.join("kicad-embed-cache").join("hostlibs"))).unwrap_or(b).to_string_lossy().to_string(),
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
        // Backplane KiCad reads the person's own KiCad settings (`<config>/kicad/10.99`): its wheel zooms at the pointer (set once; a copy
        // of the log writes nothing of the person's)
        // (not while another KiCad of ours is up: it would write its own settings back over this when it closes)
        if mode == "fork" && !crate::copy::on() && !self.sessions.values().any(|s| s.pro != pro && s.is_live()) {
            if let (Some(root), Some(ver)) = (super::libtables::config_root(), super::libtables::settings_version(&env.version)) { wheel_zoom_once(&root.join(ver)); }
        }
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

        // macOS: only a fork with the macOS embedding (embed-macos: SetHostGeometry) can be in the pane; any other KiCad (stock, bp.6)
        // fails at once, so the pane offers the KiCad app as it did before (native_child_mac.rs)
        // (not in the hub's tests: their stand-in runtimes exercise the attach logic every platform shares; mac_refusal has its own test)
        if let Some(why) = mac_refusal(cfg!(target_os = "macos") && !cfg!(test), mode, &desc) {
            let to = super::install::package().filter(|p| super::install::pin_in_pane("macos", p) && (mode != "fork" || super::install::older(&env.version, p))).map(|p| p.version);
            sess.state = "failed".into(); sess.reason = mac_refusal_with(why, to);
            let out = sess.info_json();
            self.sessions.insert(pro.clone(), sess.clone());
            self.publish_and_reply(&sess, Some(client), reply, Ok(out));
            return;
        }

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
        let version = env.version.clone();
        let lock_docs = launch_docs(&files, &sess.active);
        tokio::spawn(async move {
            // Retire any headless kserve for this project before the GUI opens its files: it holds the project's lock while it runs (the
            // GUI KiCad would ask "File Open Warning" outside the pane). It refuses while it has unsaved work (an agent's commit not yet
            // saved): asked again each second while that agent finishes on it (its requests still go there), up to HANDOVER_WAIT
            let t = std::time::Instant::now(); let mut said = false;
            loop {
                let r = ipc::retire(&pro).await;
                if r["ok"] != false { if said { eprintln!("kicad embed: {pro}: the headless KiCad stopped after {} ms; the editor starts", t.elapsed().as_millis()); } break }
                if !said { said = true; eprintln!("kicad embed: {pro}: the headless KiCad has unsaved work ({}); waiting for it before the editor starts", r["error"].as_str().unwrap_or(""));
                           let _ = me.send(Msg::StartPhase { pro: pro.clone(), gen, what: "handover".into() }); }
                if t.elapsed() >= HANDOVER_WAIT { eprintln!("kicad embed: {pro}: the headless KiCad still has unsaved work after {} s; the editor starts anyway", HANDOVER_WAIT.as_secs()); break }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            // the lock of every document this KiCad opens at launch (the project, the editor shown): a stale one is cleared and logged,
            // one held by another live KiCad is put to the person before KiCad opens it (locks.rs)
            for doc in lock_docs {
                if !lock_gate(&me, &pro, gen, &doc, "before KiCad starts").await {
                    let _ = me.send(Msg::StartFailed { pro: pro.clone(), gen, why: format!("Not opened: {} is open in another KiCad, and you chose Cancel", Path::new(&doc).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()) });
                    return;
                }
            }
            // #1185: window geometry an older runtime's embedded KiCad saved into the person's settings, repaired while no KiCad runs
            if mode_s == "fork" { let (v, h) = (version.clone(), home.clone()); let _ = tokio::task::spawn_blocking(move || super::geomguard::guard(&v, &h)).await; }
            spawn_gui_session(me, home, pro, root_s, gen, mode_s, argv_vec, sock_dir, req_sock, eff_display, desc, active, spawn_container, sweep_locks, caps).await;
        });
    }

    fn show(&mut self, client: u64, pro: String, frame: String, doc: String, item: String, remount: bool, focus: bool, reply: oneshot::Sender<Result<Value, String>>) {
        let Some(s) = self.sessions.get_mut(&pro) else {
            let _ = reply.send(Err("no embedded KiCad session for this project; attach first".into())); return;
        };
        if let Some(why) = frame_refused(&frame, &s.caps) { let _ = reply.send(Err(why.into())); return; }
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
            // numbered: a show still waiting on KiCad's task when a newer one comes gives way to it (only the latest editor is sent)
            let seq = next_show(&snap.sock);
            // the reply waits in the open: a show refused by KiCad's task gate is answered at once (raw_gated), so the client's view of
            // the session, its task_wait, is not held behind it; the switch then lands with the frames' update (Nov 14: the pane said
            // "Starting KiCad…" for the whole DRC, the info pushes queued after the show's reply)
            early_replies().lock().unwrap().insert((snap.sock.clone(), seq), reply);
            tokio::spawn(async move {
                let take = || early_replies().lock().unwrap().remove(&(snap.sock.clone(), seq));
                // an editor whose document KiCad has not opened yet: its lock read first (locks.rs), another live KiCad's put to the person
                if let Some(doc) = unopened_doc(&snap) {
                    // the person is to be asked: the client's show is answered now (it waits on its reply, and its connection with it: held
                    // for the answer, the client's heartbeat timed out and the pane was rebuilt under the question, P3); the switch follows the
                    // answer through the frames' update
                    if matches!(lock_verdict(&doc), super::locks::Verdict::Foreign { .. }) { if let Some(r) = take() { let _ = r.send(Ok(snap.info_json())); } }
                    if !lock_gate(&me, &snap.pro, snap.gen, &doc, "before KiCad opens it").await {
                        if let Some(r) = take() { let _ = r.send(Err(format!("Not opened: {} is open in another KiCad, and you chose Cancel", Path::new(&doc).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()))); }
                        return;
                    }
                }
                let synced = match switch_fork_frame_n(&me, &snap.sock, &snap.desc, &snap.active, &doc, &item, remount, focus, Some(seq)).await {
                    Ok(v) => v,
                    Err(e) if e == SUPERSEDED => { if let Some(r) = take() { let _ = r.send(Ok(snap.info_json())); } return; }
                    Err(e) => { if let Some(r) = take() { let _ = r.send(Err(e)); } return; }
                };
                let reply = take().map(|r| (cid, r));
                if let Some((holder, _, frames)) = synced {
                    let _ = me.send(Msg::FramesUpdated { pro: snap.pro.clone(), gen: snap.gen, holder: Some(holder), frames, reply });
                } else {
                    let _ = me.send(Msg::FramesUpdated { pro: snap.pro.clone(), gen: snap.gen, holder: None, frames: snap.frames.clone(), reply });
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
                    let _ = me.send(Msg::ChildExited { pro: s.pro.clone(), gen: s.gen, code: 0, how: String::new(), tail: String::new(), reply: Some(reply) });
                } else {
                    let err = o["error"].as_str().unwrap_or("KiCad refused to close").to_string();
                    let mut unsaved = s.unsaved_docs();
                    if unsaved.is_empty() {
                        let gf = raw_on(&me, &s.sock, &s.desc, "kiapi.backplane.embed.GetFrames", json!({}), 5000).await;
                        if gf["ok"].as_bool().unwrap_or(false) {
                            let (_, _, frames) = parse_get_frames(&gf["message"]);
                            unsaved = frames.iter().filter(|f| f.modified).map(unsaved_name).collect();
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
            let _ = me.send(Msg::ChildExited { pro: s.pro.clone(), gen: s.gen, code: 0, how: String::new(), tail: String::new(), reply: Some(reply) });
        });
    }

    fn restart(&mut self, client: u64, pro: String, env: Env, reply: oneshot::Sender<Result<Value, String>>) {
        // The KiCad running now goes before the next starts (the update's reopen on a new runtime, a lost window's restart): started at
        // once, the next met the old one's lock on the project and waited on a question nobody answered (clean-box bp.13 update run). As the
        // start retry does: the session is held "starting" under a new generation (an attach joins its waiters, the old one's exit is not
        // taken for it), the old KiCad is asked to close, killed if it stays, and the next starts only once it is gone (Msg::RetryNow).
        if let Some(old) = self.sessions.get(&pro).map(|s| s.pid).filter(|p| *p > 0 && crate::plat::proc::group_alive(*p)) {
            self.next_gen += 1;
            let g = self.next_gen;
            let Some(s) = self.sessions.get_mut(&pro) else { return };
            s.closing = true; s.gen = g; s.pid = 0; s.state = "starting".into(); s.reason = "restarting".into();
            let (root, snap) = (s.root.clone(), s.clone());
            self.publish(&snap, None);
            self.attach_waiters.entry(pro.clone()).or_default().push((client, reply));
            let (me, pro_c) = (self.me.clone(), pro.clone());
            tokio::spawn(async move {
                let waited = gone_before_retry(old, &root, &pro_c, RETRY_TERM_WAIT, RETRY_KILL_WAIT).await;
                let _ = me.send(Msg::RetryNow { pro: pro_c, gen: g, client, env, waited });
            });
            return;
        }
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
                (root, String::new(), 0, 0, String::new())
            }
        };
        let files = project::read(&root);
        // no session before it: the board, or the schematic of a project without one (#1326), never a missing document
        let frame = if frame.is_empty() { frame_with_document("PCB", &files).unwrap_or("PCB").to_string() } else { frame };
        self.attach(client, pro, root, files, frame, container, toplevel, display, true, env, reply);
    }

    fn handle(&mut self, m: Msg) {
        match m {
            Msg::Attach { client, pro, root, files, frame, container, toplevel, display, wait, env, reply } => {
                self.note_user(client);
                self.attach(client, pro, root, files, frame, container, toplevel, display, wait, env, reply);
            }
            Msg::Show { client, pro, frame, doc, item, remount, focus, reply } => {
                self.note_user(client);
                // the person's switch (a client's show): for SWITCH_HOLD a show request KiCad makes with no actor for another frame is KiCad's own,
                // not followed. A schematic opening cold made KiCad ask for the board ~250 ms after Ctrl+Tab to it (ET_SHOW_REQUESTED FK_PCB,
                // no actor: lane M's Mac mini, bp.13 batch 13), the hub showed the board, and the pane followed KiCad back to it
                if client != 0 { hold_shows_after_switch(&pro); }
                self.show(client, pro, frame, doc, item, remount, focus, reply)
            }
            Msg::Detach { client, pro, keep, reply } => self.detach(client, pro, keep, reply),
            Msg::Warm { client, pro, root, files, frame, display, hardware, locked, env, reply } => {
                self.note_user(client);
                // a warm-up is never asked for a document: it starts KiCad on one that exists (a schematic-only project's schematic), and
                // on none when the project has no board and no schematic (#1326)
                let Some(frame) = frame_with_document(&frame, &files).map(str::to_string) else {
                    let why = "the project has no board and no schematic to open";
                    eprintln!("kicad embed: not warming {pro}: {why}");
                    let _ = reply.send(Ok(json!({"warm": false, "why": why})));
                    return
                };
                let (desc, _) = discover_desc(&self.hub.cfg.home, &env);
                let quiet = !desc.is_empty() && proto::load(&desc).has_own("kiapi.backplane.embed.CancelProgress");
                let (mem, cfg) = (MemNow::read(), warm_cfg());
                match warm_decision(&self.sessions, &pro, hardware, locked, mem, quiet, cfg) {
                    Warm::Have => {
                        if let Some(s) = self.sessions.get_mut(&pro) { s.touched_at = ipc::now_s(); }
                        let _ = reply.send(Ok(json!({"warm": true, "started": false})));
                    }
                    Warm::No(why) => { eprintln!("kicad embed: not warming {pro}: {why} (runtime {} {}, api.desc {desc})", env.root, env.version); let _ = reply.send(Ok(json!({"warm": false, "why": why}))); }
                    Warm::Start { evict } => {
                        if let Some(v) = evict {
                            eprintln!("kicad embed: evicting {v}: {} KiCads running, the cap is {} in all; room for {pro} (warm-up)", self.live_count(), WARM.max);
                            self.retire(&v);
                        }
                        let (cost, src) = kicad_cost_mb(&self.sessions, cfg);
                        let mb = |v: Option<u64>| v.map_or("unknown".to_string(), |m| format!("{m} MB"));
                        eprintln!("kicad embed: warming {pro} (free {}, KiCad {cost} MB ({src}), headroom {} MB, pressure {})", mb(mem.free_mb), cfg.headroom_mb, mem.pressure.map_or("unknown".into(), |p| format!("{p:.2}")));
                        let (tx, rx) = oneshot::channel();
                        // detached: no container yet (the fork parks its frames); Edit attaches it to the pane
                        self.attach(client, pro, root, files, frame, 0, 0, display, false, env, tx);
                        tokio::spawn(async move { let r = rx.await.unwrap_or_else(|_| Err("no reply".into())); let _ = reply.send(r.map(|_| json!({"warm": true, "started": true}))); });
                    }
                }
            }
            Msg::Close { client, pro, save, reply } => self.close(client, pro, save, reply),
            Msg::Restart { client, pro, env, reply } => self.restart(client, pro, env, reply),
            Msg::RetryNow { pro, gen, client, env, waited } => {
                // the session was left (stopped or retired) while the first KiCad went: nothing to start
                if !self.sessions.get(&pro).is_some_and(|s| s.gen == gen && s.state == "starting") { return }
                // a KiCad that outlived even its kill: no second one beside it; the pane says why and offers to try again (never a silent pane)
                if waited.starts_with("was still there") {
                    let why = format!("the KiCad before it would not close ({waited})");
                    eprintln!("kicad embed: {pro}: {why}; not starting another");
                    if let Some(s) = self.sessions.get_mut(&pro) { *s = s.clone().failed(&why); let snap = s.clone(); self.publish(&snap, None); }
                    self.finish_attach_waiters(&pro, Err(why));
                    return;
                }
                eprintln!("kicad embed: {pro}: the KiCad before {waited}; starting the next");
                let (tx, _rx) = oneshot::channel();
                self.restart(client, pro, env, tx);
            }
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
            Msg::LockAsk { pro, gen, ask, answer } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen) else { let _ = answer.send(false); return };
                eprintln!("kicad embed: {pro}: {} is held by {}: the person is asked (Open anyway or Cancel)", ask["file"].as_str().unwrap_or(""), ask["holder"].as_str().unwrap_or(""));
                s.lock_ask = ask; let snap = s.clone();
                self.lock_waits.insert(pro.clone(), answer);
                self.publish(&snap, None);
            }
            Msg::LockAnswer { pro, open } => {
                let Some(answer) = self.lock_waits.remove(&pro) else { return };
                if let Some(s) = self.sessions.get_mut(&pro) { s.lock_ask = Value::Null; let snap = s.clone(); self.publish(&snap, None); }
                let _ = answer.send(open);
            }
            Msg::StartPhase { pro, gen, what } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.state == "starting") else { return };
                if what == "spawned" { s.start_at = ipc::now_ms() as u64; }
                s.start_phase = what;
                let snap = s.clone();
                self.publish(&snap, None);
            }
            Msg::TaskWait { sock, what } => {
                if let Some(s) = self.sessions.values_mut().find(|s| s.sock == sock && !sock.is_empty()) {
                    if s.task_wait != what { s.task_wait = what; let snap = s.clone(); self.publish(&snap, None); }
                }
            }
            Msg::Raw { sock, desc, ty, msg, timeout, client, reply } => {
                // BACKPLANE_KICAD_DEBUG: every request the hub sends the embedded KiCad, in order, with the time, its kind and KiCad's memory
                // then (what is loaded at open, and which request separates a 4.8 GB open from a 5.5 GB one)
                if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() && !ty.ends_with("NextEvents") && !ty.ends_with("Ping") {
                    let pid = self.sessions.values().find(|s| s.sock == sock).map_or(0, |s| s.pid);
                    eprintln!("kicad embed: request {ty} kind={} client={client} rss={}MB (at {})", msg["kind"].as_str().unwrap_or(""), super::mem::proc_mb(pid).map_or(0, |m| m.0), ipc::now_ms());
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
                note_task_gate(&s.sock, &s.caps);
                if self.start_retried.remove(&pro) { eprintln!("kicad embed: {pro}: started after a missed start window (the hub's retry or the person's Try again)"); }
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
                let Some(pid) = self.sessions.get(&pro).filter(|s| s.gen == gen && s.state == "starting").map(|s| s.pid) else { return };
                let env = self.start_env.get(&pro).cloned();
                if let Some(env) = env.filter(|_| start_retry(&why, self.start_retried.contains(&pro))) {
                    // once more, by itself: the pane stays on "Starting KiCad..." and the first start's waiters get the second start's answer
                    eprintln!("kicad embed: {pro}: {why}; starting it once more (the first start waited {})", start_waits(pid).unwrap_or_else(|| "?".into()));
                    self.start_retried.insert(pro.clone());
                    let client = self.attach_waiters.get(&pro).and_then(|w| w.first()).map_or(0, |w| w.0);
                    // the second KiCad starts only once the first is gone (bp-win: spawned 8 ms after the first was asked to close, it found the
                    // project held and never answered Ping). Meanwhile the session stays "starting" under a new generation: an attach joins its
                    // waiters instead of starting a third, and the first KiCad's own exit or late answers (its generation) are not taken for it
                    self.next_gen += 1;
                    let (old_pid, wait_gen) = (pid, self.next_gen);
                    let Some(s) = self.sessions.get_mut(&pro) else { return };
                    s.gen = wait_gen; s.pid = 0; s.reason = "restarting".into();
                    let (root, pro_c, me) = (s.root.clone(), pro.clone(), self.me.clone());
                    tokio::spawn(async move {
                        let waited = gone_before_retry(old_pid, &root, &pro_c, RETRY_TERM_WAIT, RETRY_KILL_WAIT).await;
                        let _ = me.send(Msg::RetryNow { pro: pro_c, gen: wait_gen, client, env, waited });
                    });
                    return;
                }
                let Some(s) = self.sessions.get_mut(&pro) else { return };
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
            Msg::Notice { pro, gen, notice } => {
                eprintln!("kicad embed: KiCad notice ({}{}): {}{}", notice["kind"].as_str().unwrap_or(""),
                    notice["frame"].as_str().filter(|f| !f.is_empty()).map(|f| format!(", {f}")).unwrap_or_default(),
                    notice["title"].as_str().unwrap_or(""), notice["detail"].as_str().filter(|d| !d.is_empty()).map(|d| format!(": {d}")).unwrap_or_default());
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.is_live()) else { return };
                *s = s.clone().notice(notice);
                let snap = s.clone();
                self.publish(&snap, None);
            }
            Msg::FrameDrawn { pro, gen, window, drawn } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.is_live()) else { return };
                // hidden: its next showing is drawn again before it counts
                if s.drawn.contains(&window) == drawn { return }
                if drawn { s.drawn.push(window); if s.drawn.len() > 16 { s.drawn.remove(0); } } else { s.drawn.retain(|w| *w != window); }
                let snap = s.clone();
                self.publish(&snap, None);
            }
            Msg::ReservedKey { pro, gen, seq, index } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen && s.is_live()) else { return };
                s.keys.push((seq, index)); if s.keys.len() > 8 { s.keys.remove(0); }
                let snap = s.clone();
                self.publish(&snap, None);
            }
            Msg::NoticeDismiss { pro, id } => {
                let Some(s) = self.sessions.get_mut(&pro) else { return };
                *s = s.clone().dismiss_notice(id);
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
            Msg::ChildExited { pro, gen, code, how, tail, reply } => {
                let Some(s) = self.sessions.get_mut(&pro).filter(|s| s.gen == gen) else {
                    if let Some(tx) = reply { let _ = tx.send(Ok(json!({"closed":true}))); }
                    return;
                };
                if matches!(s.state.as_str(), "stopped" | "failed" | CLOSED) {
                    if let Some(tx) = reply { let _ = tx.send(Ok(json!({"closed":true}))); }
                    return;
                }
                let was_starting = s.state == "starting";
                let old = s.clone();
                *s = s.clone().exited_how(code, &how, &tail);
                // bp.13.1: KiCad whose windows were lost with their container saved its autosave and local history and exited with code 3
                // ("kicad embed: windows lost"): started again on the project at once, with no window to hold it (the next Edit adopts it),
                // so KiCad offers its Recover Unsaved Changes; the person is never left with a dead pane
                let windows_lost = windows_lost(code, &tail) && s.state == CLOSED;
                if windows_lost { s.container = 0; s.reason = "KiCad lost its window and is starting again to offer back the unsaved changes".into(); }
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
                if windows_lost {
                    eprintln!("kicad embed: {pro}: KiCad lost its windows (code 3): starting it again");
                    let (me, hub) = (self.me.clone(), self.hub.clone());
                    tokio::spawn(async move { let env = ipc::runtime(&hub).await; let (tx, _rx) = oneshot::channel(); let _ = me.send(Msg::Restart { client: 0, pro, env, reply: tx }); });
                }
            }
            Msg::Tick => {
                // the app closed (no desktop client that used KiCad is connected any more): its KiCads close with it
                let hub = self.hub.clone();
                let had = !self.users.is_empty() || self.users_gone.is_some();
                self.users.retain(|c| hub.is_attached(*c));
                if had && self.users.is_empty() {
                    let since = *self.users_gone.get_or_insert(ipc::now_s());
                    if ipc::now_s().saturating_sub(since) >= APP_GONE_S {
                        self.users_gone = None;
                        for v in app_gone_victims(&self.sessions) { eprintln!("kicad embed: the app closed, closing {v}"); self.retire(&v); }
                    }
                } else { self.users_gone = None; }
                for v in idle_victims(&self.sessions, ipc::now_s(), WARM.idle_s) { eprintln!("kicad embed: idle, closing {v}"); self.retire(&v); }
                // memory: read each tick (the warm-up's cost reads it), published when it has moved a step (the session info otherwise kept
                // the first sample, taken while starting), and the peak logged once it passes the default (it is then the warm-up's cost)
                let mut moved = vec![];
                for s in self.sessions.values_mut().filter(|s| s.is_live() && s.pid > 0) {
                    let Some((now, peak)) = super::mem::proc_mb(s.pid) else { continue };
                    s.rss_mb = now; s.peak_mb = peak;
                    if !s.peak_logged && peak > WARM.kicad_mb { s.peak_logged = true; eprintln!("kicad embed: {} peak {peak} MB, past the {} MB default: the warm-up's cost now", s.pro, WARM.kicad_mb); }
                    if mem_moved(s.mem_pub, (now, peak), MEM_PUBLISH_STEP_MB) { s.mem_pub = (now, peak); moved.push(s.pro.clone()); }
                }
                for p in moved { if let Some(s) = self.sessions.get(&p) { self.publish(s, None); } }
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
    raw_gated(me, sock, desc, ty, msg, timeout, "", None).await
}
async fn raw_once(me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, ty: &str, msg: Value, timeout: u64, client: &str) -> Value {
    let (t, r) = oneshot::channel();
    if me.send(Msg::Raw { sock: sock.into(), desc: desc.into(), ty: ty.into(), msg, timeout, client: client.into(), reply: t }).is_err() {
        return json!({"ok":false,"error":"embed actor stopped"})
    }
    r.await.unwrap_or_else(|_| json!({"ok":false,"error":"no reply"}))
}

/// The hub's own drive of the editor's frames, which KiCad with `task-gate-reads` (lane P, 56ef479828) refuses AS_BUSY while a check or a
/// refill runs, where KiCad before it made them wait: sent again as soon as that task ends, never shown to the person as a failure.
const TASK_GATED: &[&str] = &["ShowFrame", "HideFrame", "AttachToParent", "SetFrameGeometry", "SetTheme", "OpenGerbers", "SetReservedKeys", "SetChrome", "SetHostMarkers"];
/// The task a refusal names, and its token when said: Some only for the task gate's own refusal ("a task is running: <what> (token <t>, ...").
pub fn task_refusal(o: &Value) -> Option<(String, String)> {
    if o["ok"] == true || o["status"] != "AS_BUSY" { return None }
    let rest = o["error"].as_str()?.strip_prefix("a task is running: ")?;
    let what = rest.split(" (token ").next().unwrap_or(rest).trim().to_string();
    let token = rest.split(" (token ").nth(1).and_then(|t| t.split(',').next()).unwrap_or("").trim().to_string();
    Some((what, token))
}
#[derive(Debug, PartialEq)]
pub enum TaskState { Running, Unknown, Ended }
/// What a GetTaskStatus answer says: running, unknown (no state: proto3's TS_UNKNOWN), or ended (any other state, or the request refused).
pub fn task_state(st: &Value) -> TaskState {
    if st["ok"] != true { return TaskState::Ended }
    match &st["message"]["state"] {
        Value::Null => TaskState::Unknown,
        v if v == "TS_RUNNING" || v.as_i64() == Some(1) => TaskState::Running,
        v if v == "TS_UNKNOWN" || v.as_i64() == Some(0) => TaskState::Unknown,
        _ => TaskState::Ended,
    }
}
/// The KiCads (their sockets) whose runtime says `task-gate-reads`: only their gated requests are retried (older runtimes make them wait).
fn gated_socks() -> &'static Mutex<HashSet<String>> { static S: OnceLock<Mutex<HashSet<String>>> = OnceLock::new(); S.get_or_init(Default::default) }
pub fn note_task_gate(sock: &str, caps: &[String]) { if caps.iter().any(|c| c == "task-gate-reads") { gated_socks().lock().unwrap().insert(sock.to_string()); } else { gated_socks().lock().unwrap().remove(sock); } }
fn has_task_gate(sock: &str) -> bool { gated_socks().lock().unwrap().contains(sock) }
/// The latest show asked per KiCad (its socket): an older one still waiting on a task gives way (only the latest editor is sent).
fn show_seq() -> &'static Mutex<HashMap<String, u64>> { static S: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new(); S.get_or_init(Default::default) }
fn next_show(sock: &str) -> u64 { let mut m = show_seq().lock().unwrap(); let e = m.entry(sock.to_string()).or_default(); *e += 1; *e }
fn show_current(sock: &str, seq: u64) -> bool { show_seq().lock().unwrap().get(sock).map_or(true, |n| *n == seq) }
/// A numbered show's reply while its request may wait on KiCad's task: taken by the first of its refusal (answered at once) or its end.
fn early_replies() -> &'static Mutex<HashMap<(String, u64), oneshot::Sender<Result<Value, String>>>> { static R: OnceLock<Mutex<HashMap<(String, u64), oneshot::Sender<Result<Value, String>>>>> = OnceLock::new(); R.get_or_init(Default::default) }
/// What a gated request superseded by a newer show answers.
pub const SUPERSEDED: &str = "superseded by a newer show";
/// One request; a TASK_GATED one refused by KiCad's task gate is sent again when the task ends (GetTaskStatus with its token, else after a
/// short wait), up to 10 minutes, the session saying meanwhile what it waits on (`task_wait`); `seq`: a show's number, given up for a newer.
async fn raw_gated(me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, ty: &str, msg: Value, timeout: u64, client: &str, seq: Option<u64>) -> Value {
    let gated = TASK_GATED.iter().any(|g| ty.rsplit('.').next() == Some(*g)) && has_task_gate(sock);
    raw_gated_for(me, sock, desc, ty, msg, timeout, client, seq, gated, Duration::from_secs(600), true).await
}
/// raw_gated for any request (`gated`), waiting at most `max` for KiCad's task; `say`: the session says what it waits on (the hub's own
/// drive of the frames; an agent's or the Plan panel's read waits without it). Past `max` the task gate's refusal is the answer.
async fn raw_gated_for(me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, ty: &str, msg: Value, timeout: u64, client: &str, seq: Option<u64>, gated: bool, max: Duration, say: bool) -> Value {
    let t0 = std::time::Instant::now();
    let mut waited = false;
    loop {
        let o = raw_once(me, sock, desc, ty, msg.clone(), timeout, client).await;
        let refusal = if gated { task_refusal(&o) } else { None };
        let Some((what, token)) = refusal.filter(|_| t0.elapsed() < max) else {
            if waited { if say { let _ = me.send(Msg::TaskWait { sock: sock.into(), what: String::new() }); } eprintln!("kicad embed: {ty} sent after KiCad's task, {} ms", t0.elapsed().as_millis()); }
            return o
        };
        if !waited { eprintln!("kicad embed: {ty} waits on KiCad's task: {what}"); waited = true; }
        if say { let _ = me.send(Msg::TaskWait { sock: sock.into(), what: what.clone() }); }
        // a show's caller is answered now (the switch follows when the task ends): its client sees the wait instead of a held request
        if let Some(n) = seq { if let Some(r) = early_replies().lock().unwrap().remove(&(sock.to_string(), n)) { let _ = r.send(Ok(json!({"waiting": true, "task_wait": what}))); } }
        let mut unknown_once = false;
        loop {
            if seq.is_some_and(|n| !show_current(sock, n)) { let _ = me.send(Msg::TaskWait { sock: sock.into(), what: String::new() }); return json!({"ok": false, "error": SUPERSEDED, "superseded": true}) }
            if token.is_empty() { tokio::time::sleep(Duration::from_millis(250)).await; break }
            // TaskStatusResponse.state: TS_RUNNING goes on; anything else ended it. An absent state (proto3's TS_UNKNOWN) is asked once more;
            // a refused request (the token gone: "unknown task token", AS_BAD_REQUEST) is an ended task too
            let st = raw_once(me, sock, desc, "kiapi.common.tasks.GetTaskStatus", json!({"token": {"value": token}}), 3000, "").await;
            match task_state(&st) { TaskState::Running => {}, TaskState::Unknown if !unknown_once => unknown_once = true, _ => break }
            if t0.elapsed() > max { break }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
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
    show_fork_frame_n(me, sock, desc, kind, doc, item, focus, None).await
}
/// `seq`: the show's number (next_show): while it waits on KiCad's task a newer show takes over and this one gives way (Err(SUPERSEDED)).
async fn show_fork_frame_n(me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, kind: &str, doc: &str, item: &str, focus: bool, seq: Option<u64>) -> Result<Value, String> {
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
        let o = raw_gated(me, sock, desc, "kiapi.backplane.embed.ShowFrame", body.clone(), 15000, "", seq).await;
        if o["ok"].as_bool().unwrap_or(false) { return Ok(o) }
        if o["superseded"] == true { return Err(SUPERSEDED.into()) }
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

/// The editor `kind` of the session on `sock` would open a document another live KiCad holds (KiCad has no frame of that kind yet):
/// why it is not shown, or None.
fn held_elsewhere(sock: &str, kind: &str) -> Option<String> {
    let ext = match kind { "PCB" | "PCB_3D" => "kicad_pcb", "SCH" => "kicad_sch", _ => return None };
    let pro = routes().lock().unwrap().values().find(|r| r.sock == sock).map(|r| r.pro.clone())?;
    if has_frame(&pro, if ext == "kicad_pcb" { "PCB" } else { "SCH" }) { return None }
    let doc = Path::new(&pro).with_extension(ext);
    if !doc.is_file() { return None }
    let doc = doc.to_string_lossy().to_string();
    matches!(lock_verdict(&doc), super::locks::Verdict::Foreign { .. })
        .then(|| format!("{} is open in another KiCad: not shown until the person answers", Path::new(&doc).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()))
}

/// Show `kind` in fork mode and hide every other currently-shown frame via `HideFrame` so GTK3/wxMSW owns frame map state cleanly.
async fn switch_fork_frame(
    me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, kind: &str, doc: &str, item: &str, remount: bool, focus: bool,
) -> Result<Option<(u64, String, Vec<Frame>)>, String> {
    switch_fork_frame_n(me, sock, desc, kind, doc, item, remount, focus, None).await
}
async fn switch_fork_frame_n(
    me: &mpsc::UnboundedSender<Msg>, sock: &str, desc: &str, kind: &str, doc: &str, item: &str, remount: bool, focus: bool, seq: Option<u64>,
) -> Result<Option<(u64, String, Vec<Frame>)>, String> {
    let target_kind = canon_frame(kind);
    // every path that shows an editor KiCad has not opened yet (a show, a re-attach, a follow of KiCad's own request) stops here while
    // another live KiCad holds that document: only the show's own lock_gate (the person's answer) lets KiCad open it (P3: a re-attach
    // after the pane's tab press sent ShowFrame FK_SCH around the question, and bp.12.4 asked in its own box outside the pane)
    if let Some(why) = held_elsewhere(sock, target_kind) { return Err(why) }
    if remount {
        let gf0 = raw_on(me, sock, desc, "kiapi.backplane.embed.GetFrames", json!({}), 5000).await;
        if gf0["ok"].as_bool().unwrap_or(false) {
            let (_, _, f0) = parse_get_frames(&gf0["message"]);
            for f in f0.into_iter().filter(|f| f.kind == target_kind && f.xid > 0) {
                let _ = raw_on(me, sock, desc, "kiapi.backplane.embed.HideFrame", json!({ "window": f.xid }), 5000).await;
            }
        }
    }
    let shown_res = show_fork_frame_n(me, sock, desc, kind, doc, item, focus, seq).await?;
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
            let _ = raw_gated(me, sock, desc, "kiapi.backplane.embed.HideFrame", json!({ "window": win }), 5000, "", seq).await;
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

/// The board or schematic the shown editor (`active`) opens when KiCad has no frame of that kind yet ("" none, or one already open).
fn unopened_doc(s: &Session) -> Option<String> {
    let (kind, ext) = match s.active.as_str() { "PCB" | "PCB_3D" => ("PCB", "kicad_pcb"), "SCH" => ("SCH", "kicad_sch"), _ => return None };
    if s.frames.iter().any(|f| f.kind == kind) { return None }
    let doc = Path::new(&s.pro).with_extension(ext);
    doc.is_file().then(|| doc.to_string_lossy().to_string())
}

/// The documents a KiCad opens at launch: the project, and the shown editor's board or schematic.
fn launch_docs(files: &Files, active: &str) -> Vec<String> {
    let mut v = vec![files.pro.clone()];
    match active { "PCB" | "PCB_3D" => v.push(files.pcb.clone()), "SCH" => v.push(files.sch.clone()), _ => {} }
    v.retain(|d| !d.is_empty()); v
}

/// The lock of `doc` as this hub sees it: KiCad's lock read (locks.rs), and a holder this hub started that has not gone yet (live_pids:
/// a pid stays there until its process is reaped, while it is being stopped or restarted too) is this hub's own, whatever its command
/// line reads (lane K's update gate, Nov 16: the bp.12.4 KiCad being stopped for the update, its command line no longer readable, was
/// taken for another's and the person was asked; the lead's rule: a KiCad the hub started is its own until it is gone).
pub fn lock_verdict(doc: &str) -> super::locks::Verdict {
    let (user, host) = super::locks::me();
    let v = super::locks::classify(super::locks::inspect(doc).as_ref(), std::process::id(), &user, &host);
    let v = own_pid_verdict(v, |p| live_pids().lock().unwrap_or_else(|e| e.into_inner()).contains(&p));
    super::locks::orphaned(v, crate::plat::proc::alive)
}
/// A foreign verdict whose holder is one of this hub's own processes (`own`) is this hub's.
pub fn own_pid_verdict(v: super::locks::Verdict, own: impl Fn(u32) -> bool) -> super::locks::Verdict {
    match v { super::locks::Verdict::Foreign { pid: Some(p), .. } if own(p) => super::locks::Verdict::Ours(p), v => v }
}

/// Before KiCad opens `doc`: its lock read (lock_verdict). Stale: removed and logged. Ours: one of this hub's KiCads (being stopped, or
/// the headless one retired): waited for until it lets go (up to OWN_LOCK_WAIT), never asked about. Foreign: the person is asked in the
/// pane; true to go on (nothing held, Open anyway: the lock removed, or the other KiCad let go meanwhile), false for Cancel.
async fn lock_gate(me: &mpsc::UnboundedSender<Msg>, pro: &str, gen: u64, doc: &str, when: &str) -> bool {
    let mut v = lock_verdict(doc);
    let t = std::time::Instant::now();
    while matches!(v, super::locks::Verdict::Ours(_)) && t.elapsed() < OWN_LOCK_WAIT { tokio::time::sleep(Duration::from_millis(250)).await; v = lock_verdict(doc); }
    // a holder whose command line cannot be read (a process exiting, or one starting through a launcher) is looked at again before
    // anyone is asked (lane M, Mac mini: the pane asked for 180 s about a process gone a moment later)
    let unreadable = |v: &super::locks::Verdict| matches!(v, super::locks::Verdict::Foreign { pid: Some(_), what, .. } if what.is_empty());
    let t = std::time::Instant::now();
    while unreadable(&v) && t.elapsed() < Duration::from_secs(5) { tokio::time::sleep(Duration::from_millis(250)).await; v = lock_verdict(doc); }
    // a KiCad an earlier Backplane started, left running when that Backplane was killed (its hub gone): closed, then the lock is free.
    // TERM only, and KILL never before KiCad's autosave debounce has passed (ORPHAN_TERM_WAIT): its own recovery snapshot of the person's
    // unsaved edits stands, and KiCad offers it back when it opens the files; an agent's change was saved by the hub when it landed
    // (f6c2eac8e), and bp.13 (3a19ebb5bb) arms the autosave for API commits too. The pane says the editor was reopened and why.
    if let super::locks::Verdict::Orphan(pid) = v {
        eprintln!("kicad embed: {pro}: {doc} is held by a KiCad an earlier Backplane started (pid {pid}, its hub gone): closing it {when}");
        if !crate::plat::proc::kill_group(pid, crate::plat::proc::TERM) { let _ = crate::plat::proc::kill(pid, crate::plat::proc::TERM); }
        let t = std::time::Instant::now();
        while crate::plat::proc::alive(pid) && t.elapsed() < ORPHAN_TERM_WAIT { tokio::time::sleep(Duration::from_millis(100)).await; }
        let killed = crate::plat::proc::alive(pid);
        if killed { if !crate::plat::proc::kill_group(pid, crate::plat::proc::KILL) { let _ = crate::plat::proc::kill(pid, crate::plat::proc::KILL); } tokio::time::sleep(Duration::from_millis(300)).await; }
        eprintln!("kicad embed: {pro}: the earlier Backplane's KiCad (pid {pid}) {} after {} ms", if killed { "did not close and was killed" } else { "closed" }, t.elapsed().as_millis());
        let dir = Path::new(doc).parent().map(Path::to_path_buf).unwrap_or_default();
        let recovery = std::fs::read_dir(&dir).into_iter().flatten().flatten().any(|e| e.file_name().to_string_lossy().starts_with("_autosave-"));
        let _ = me.send(Msg::Notice { pro: pro.into(), gen, notice: json!({"kind": "orphan_closed", "frame": "", "error": false,
            "title": "The editor was reopened: the KiCad left running by Backplane's last session was closed",
            "detail": if recovery { "KiCad will offer to recover the unsaved changes it kept" } else { "" }}) });
        v = lock_verdict(doc);
    }
    match v {
        super::locks::Verdict::Stale => { super::locks::clear_stale(doc, &format!("{when}: nobody holds it, its KiCad ended")); true }
        super::locks::Verdict::Ours(pid) => { eprintln!("kicad embed: {pro}: {doc} is still held by this hub's KiCad (pid {pid}) {when}, after {} s", OWN_LOCK_WAIT.as_secs()); true }
        super::locks::Verdict::Foreign { .. } => {
            let holder = super::locks::holder_words(&v);
            let file = Path::new(doc).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            let (tx, rx) = oneshot::channel();
            if me.send(Msg::LockAsk { pro: pro.into(), gen, ask: json!({"file": file, "path": doc, "holder": holder}), answer: tx }).is_err() { return false }
            // while the person has not answered, the lock is read again each second: the other KiCad letting go ends the question by itself
            let mut rx = rx; let mut freed = false;
            let open = loop {
                tokio::select! {
                    r = &mut rx => break r.unwrap_or(false),
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        if !freed && !matches!(lock_verdict(doc), super::locks::Verdict::Foreign { .. }) {
                            freed = true; eprintln!("kicad embed: {pro}: {holder} let go of {file}: the question ends by itself");
                            let _ = me.send(Msg::LockAnswer { pro: pro.into(), open: true });
                        }
                    }
                }
            };
            if freed { if matches!(lock_verdict(doc), super::locks::Verdict::Stale) { super::locks::clear_stale(doc, "its holder let go while the person was asked"); } }
            else if open { super::locks::override_lock(doc, &holder); } else { eprintln!("kicad embed: {pro}: the person chose Cancel over {holder}'s lock on {file}"); }
            open
        }
        _ => true,
    }
}
/// How long an earlier Backplane's KiCad has to close on TERM before it is killed: past KiCad's autosave debounce (5 s), so its snapshot
/// of the last edit is on disk.
const ORPHAN_TERM_WAIT: Duration = Duration::from_secs(if cfg!(test) { 1 } else { 15 });
/// How long a launch waits for a lock still held by a KiCad of this hub's (stopped for a restart, an update; a headless one retired).
const OWN_LOCK_WAIT: Duration = Duration::from_secs(if cfg!(test) { 1 } else { 15 });

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
            // only a stale lock, by the same verdict as before an editor opens: written on this computer and held by nobody (a live KiCad's,
            // the person's own, or one another computer wrote on a shared folder is never swept: locks.rs)
            if s.starts_with('~') && s.ends_with(".lck") {
                let (user, host) = super::locks::me();
                if super::locks::classify(super::locks::inspect_file(&ent.path()).as_ref(), std::process::id(), &user, &host) == super::locks::Verdict::Stale {
                    super::locks::clear_stale_file(&ent.path(), "a restart's sweep");
                }
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
    let phase = { let (me, pro) = (me.clone(), pro.clone()); move |what: &str| {
        eprintln!("kicad embed: start +{}ms {what}", t0.elapsed().as_millis());
        let _ = me.send(Msg::StartPhase { pro: pro.clone(), gen, what: what.to_string() });
    } };
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
    // a launch that fails for its scope (an older systemd, no user manager) starts again unscoped (memscope::spawn)
    let build = |wrap: Option<Vec<String>>| {
    let mut c = crate::proc::tokio_cmd_in(crate::proc::Kind::Local, &argv[0], wrap); crate::memscope::raise_oom(&mut c);
    c.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&cwd)
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
    c };

    let mut tries = 0;
    let mut child = loop {
        match crate::memscope::spawn(crate::memscope::wrap_helper("kicad"), crate::memscope::Evidence::Stderr, &build) {
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
            let (code, how) = child.wait().await.map(|s| exit_how(&s)).unwrap_or((1, "code 1".into()));
            if pid > 0 {
                live_pids().lock().unwrap_or_else(|e| e.into_inner()).remove(&pid);
                crate::agents::proc::unwatch_group(pid);
                crate::plat::proc::kill_group(pid, crate::plat::proc::KILL);
            }
            done.store(true, Ordering::SeqCst);
            let _ = so.await; let _ = se.await;
            let joined: Vec<String> = tail.lock().unwrap().iter().cloned().collect();
            let _ = me.send(Msg::ChildExited { pro, gen, code, how, tail: joined.join("\n"), reply: None });
        });
    }

    // Handshake: wait up to 30 s for the API socket, then Ping + GetVersion (retrying AS_NOT_READY while frames initialize).
    // If fork KiCad opens a startup modal dialog before `SetReadyToReply` (e.g. LocalHistory's "Recover Unsaved Changes" after a crash),
    // `<sock>.events` already reports `ET_DIALOG_OPENED`: extend `deadline` while the dialog is open so the hub never kills KiCad under the user.
    let (window, stall) = start_window(&pro);
    let mut deadline = tokio::time::Instant::now() + Duration::from_secs(window);
    let sock_rx = async move { if stall { std::future::pending::<()>().await; } sock_rx.await };
    let sock = match tokio::time::timeout_at(deadline, sock_rx).await {
        Ok(Ok(s)) => s,
        _ => {
            if !done.load(Ordering::SeqCst) {
                let _ = me.send(Msg::StartFailed { pro, gen, why: format!("KiCad did not open its IPC socket within {window} s") });
            }
            return;
        }
    };

    phase("socket");
    let mut version = String::new();
    let mut hand_ok = false;
    let mut ev_since: u64 = 0;
    let mut open_dialogs: Vec<String> = Vec::new();
    // the dialogs startup_dialog sent the session as open: only their closes are sent
    let mut forwarded: Vec<String> = Vec::new();
    // macOS: since when KiCad has answered "not ready", and whether the start now waits on a system prompt (mac_prompt_wait)
    let (mut not_ready_since, mut prompted): (Option<std::time::Instant>, bool) = (None, false);
    while tokio::time::Instant::now() < deadline && !done.load(Ordering::SeqCst) {
        let p = raw_on(&me, &sock, &desc, "kiapi.common.commands.Ping", json!({}), 3000).await;
        if p["status"] == "AS_NOT_READY" { not_ready_since.get_or_insert_with(std::time::Instant::now); }
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
                            if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("kicad embed event: {ev}"); }
                            if let Some(m) = startup_dialog(&pro, gen, ev, &mut forwarded) { let _ = me.send(m); }
                            if let Some(n) = notice_of(ev) {
                                let _ = me.send(Msg::Notice { pro: pro.clone(), gen, notice: n });
                            } else if ty == "ET_DIALOG_OPENED" || ty_n == 10 {
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
            } else if mac_prompt_wait(cfg!(target_os = "macos"), not_ready_since.map(|t| t.elapsed()), false) {
                let why = mac_wait_reason(Path::new(&pro).parent().unwrap_or(Path::new(".")));
                if !prompted {
                    prompted = true;
                    deadline = tokio::time::Instant::now() + MAC_PROMPT_BOUND;
                    eprintln!("kicad embed: {pro}: KiCad not ready for {} s with no dialog of its own: waiting on {}", MAC_PROMPT_AFTER.as_secs(),
                        if why == MAC_RECOVER_REASON { "KiCad's offer to recover unsaved changes (its alert comes before its API)" } else { "a macOS prompt" });
                }
                let _ = me.send(Msg::StartupReason { pro: pro.clone(), gen, reason: why.into() });
            } else {
                let _ = me.send(Msg::StartupReason { pro: pro.clone(), gen, reason: String::new() });
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !hand_ok {
        if !done.load(Ordering::SeqCst) {
            let why = if prompted { mac_prompt_failure(MAC_PROMPT_BOUND) } else { format!("KiCad did not answer Ping and GetVersion within {window} s") };
            let _ = me.send(Msg::StartFailed { pro, gen, why });
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
                        let drawn = wait_frame_drawn(&me, &pro, gen, &mut forwarded, &sock, &desc, win, &mut ev_since, cap).await;
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
        if let Some(w) = start_waits(pid) { eprintln!("kicad embed: start waited {w}"); }
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
async fn wait_frame_drawn(me: &mpsc::UnboundedSender<Msg>, pro: &str, gen: u64, forwarded: &mut Vec<String>, sock: &str, desc: &str, win: u64, since: &mut u64, cap_ms: u64) -> bool {
    let until = tokio::time::Instant::now() + Duration::from_millis(cap_ms);
    let ev_path = format!("{sock}.events");
    let mut seen: Vec<Value> = Vec::new();
    while tokio::time::Instant::now() < until {
        let left = until.saturating_duration_since(tokio::time::Instant::now()).as_millis() as u64;
        let o = raw_on(me, &ev_path, desc, "kiapi.backplane.embed.NextEvents", json!({"since": *since, "wait_ms": left.min(1000), "max": 64}), left + 1000).await;
        if !o["ok"].as_bool().unwrap_or(false) { tokio::time::sleep(Duration::from_millis(30)).await; continue }
        let next = u64_of(&o["message"]["next"]);
        for ev in o["message"]["events"].as_array().into_iter().flatten() {
            if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("kicad embed event: {ev}"); }
            // the recovery alert's close comes here when its answer let KiCad's API up: the session must not keep it open
            if let Some(m) = startup_dialog(pro, gen, ev, forwarded) { let _ = me.send(m); }
            seen.push(ev.clone());
        }
        if next > *since { *since = next; }
        if seen.iter().any(|ev| (ev["type"] == "ET_FRAME_DRAWN" || ev["type"].as_i64() == Some(28)) && u64_of(&ev["frame"]["window"]) == win) { return true }
    }
    false
}

/// A titled ET_DIALOG_OPENED/CLOSED read before the events loop runs (the start's handshake, the first frame's draw), for the session as
/// the loop sends it: macOS's recovery alert (an NSAlert before KiCad's API is up, named by fork embed's MODAL_TITLE_SCOPE) reaches
/// `dialogs`, so the pane says "KiCad is asking: Recover Unsaved Changes". An untitled one (an older fork) is left to StartupReason.
/// `forwarded`: the titles sent as open. A close is sent only for one of them: a close whose open was not sent (untitled then, titled
/// at its close on an older fork) would otherwise take another dialog off the session (`dialog` pops the last when no title matches).
fn startup_dialog(pro: &str, gen: u64, ev: &Value, forwarded: &mut Vec<String>) -> Option<Msg> {
    let (ty, ty_n) = (ev["type"].as_str().unwrap_or(""), ev["type"].as_i64().unwrap_or(0));
    if !(matches!(ty, "ET_DIALOG_OPENED" | "ET_DIALOG_CLOSED") || matches!(ty_n, 10 | 11)) { return None }
    let title = ev["detail"].as_str().unwrap_or("").to_string();
    if title.is_empty() { return None }
    let open = ty == "ET_DIALOG_OPENED" || ty_n == 10;
    if open { forwarded.push(title.clone()) } else { forwarded.remove(forwarded.iter().rposition(|t| *t == title)?); }
    Some(Msg::Dialog { pro: pro.to_string(), gen, open, title })
}

/// KiCad's ET_NOTICE (29, `embed-notice`): a message box embedded KiCad did not show (it would have sat over a frame the host had not
/// shown, holding up its start), sent here instead. Its detail is JSON {kind, title, detail, error}; kind "graphics" is a drawing fallback
/// ("Could not use OpenGL, falling back to software rendering", a driver reset). `frame`: the kind of editor it concerns, "" for none.
/// bp.13's ET_AGENT_NOTICE "lock_overridden": KiCad opened a file another process held, without asking (embedded, nobody could answer).
/// The hub's lock check comes first (locks.rs), so this means one it did not see (a lock taken between the check and the open): said in
/// the strip as KiCad's notice, never silently.
pub fn lock_notice_of(ev: &Value) -> Option<Value> {
    if !super::coedit::is_notice(ev) { return None }
    let d: Value = serde_json::from_str(ev["detail"].as_str().unwrap_or("")).ok()?;
    if d["kind"] != "lock_overridden" { return None }
    let det: Value = match &d["details"] { Value::String(t) => serde_json::from_str(t).unwrap_or(Value::Null), v => v.clone() };
    let msg = det["message"].as_str().unwrap_or("").trim().to_string();
    Some(json!({"kind": "lock_overridden", "title": "KiCad opened this although another KiCad had it open; either may save over the other's changes", "detail": msg, "error": true, "frame": ""}))
}

pub fn notice_of(ev: &Value) -> Option<Value> {
    if !(ev["type"] == "ET_NOTICE" || ev["type"].as_i64() == Some(29)) { return None }
    let raw = ev["detail"].as_str().unwrap_or("");
    let d: Value = serde_json::from_str(raw).unwrap_or_else(|_| json!({"title": raw}));
    let txt = |k: &str| d[k].as_str().unwrap_or("").trim().to_string();
    let (title, detail) = (txt("title"), txt("detail"));
    if title.is_empty() && detail.is_empty() { return None }
    let frame = if ev["frame"].is_object() { kind_of_fk_val(&ev["frame"]["kind"]).to_string() } else { String::new() };
    Some(json!({"kind": txt("kind"), "title": title, "detail": detail, "error": d["error"].as_bool().unwrap_or(false), "frame": frame}))
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
                    if let Some(n) = notice_of(ev) {
                        if me.send(Msg::Notice { pro: pro.clone(), gen, notice: n }).is_err() { return }
                        continue
                    }
                    if ty == "ET_FRAME_DRAWN" || ty_n == 28 {
                        let window = u64_of(&ev["frame"]["window"]);
                        if window != 0 && me.send(Msg::FrameDrawn { pro: pro.clone(), gen, window, drawn: true }).is_err() { return }
                        continue
                    }
                    if ty == "ET_RESERVED_KEY" || ty_n == 30 {
                        let (seq, index) = (u64_of(&ev["seq"]), u64_of(&ev["reserved_key"]["index"]));
                        if me.send(Msg::ReservedKey { pro: pro.clone(), gen, seq, index }).is_err() { return }
                        continue
                    }
                    if ty == "ET_FRAME_HIDDEN" || ty_n == 4 {
                        let window = u64_of(&ev["frame"]["window"]);
                        if window != 0 && me.send(Msg::FrameDrawn { pro: pro.clone(), gen, window, drawn: false }).is_err() { return }
                    }
                    if ty == "ET_SELECTION_CHANGED" || ty_n == 14 {
                        if let Some((kind, names)) = super::xprobe::selection_event(ev) {
                            // BACKPLANE_TEST_SELECTION_DELAY_MS (tests): the selection reaches the person's record that late, as on a slow box
                            let delay = std::env::var("BACKPLANE_TEST_SELECTION_DELAY_MS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
                            let (p2, k2, n2) = (pro.clone(), kind.to_lowercase(), names.to_json());
                            if delay > 0 { tokio::spawn(async move { tokio::time::sleep(std::time::Duration::from_millis(delay)).await; super::coedit::person_selected(&p2, &k2, n2) }); }
                            else { super::coedit::person_selected(&p2, &k2, n2); }
                        }
                        super::xprobe::kicad_selected(&pro, ev); continue
                    }
                    if is_persons_edit(ev) { unsaved_update(&pro, |u| u.person_edited()); }
                    if super::coedit::on_edited(&pro, ev) { continue }
                    if let Some(n) = lock_notice_of(ev) {
                        eprintln!("kicad embed: {pro}: KiCad opened a locked file anyway: {}", n["detail"].as_str().unwrap_or(""));
                        if me.send(Msg::Notice { pro: pro.clone(), gen, notice: n }).is_err() { return }
                        continue
                    }
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
    eprintln!("{}", warm_memory_line());
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
        pend: HashMap::new(), sessions: HashMap::new(), attach_waiters: HashMap::new(), start_env: HashMap::new(), start_retried: Default::default(), lock_waits: HashMap::new(),
        users: HashSet::new(), users_gone: None,
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
    "editor.kicad.notice.dismiss",
    "editor.kicad.host",
    "editor.kicad.lock",
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
            // the person's switch, Edit or pre-warm: an automatic check in KiCad starts after it, not in its middle (its task would refuse them)
            if matches!(method, "editor.kicad.show" | "editor.kicad.attach" | "editor.kicad.prewarm") { super::checkrun::person_touched(&resolved.pro); }
            // the update row above the pane says what the pin offers: read the signed KiCad pins again first (at most every 15 minutes)
            if method == "editor.kicad.attach" { super::pins::nudge(hub); }
            if method == "editor.kicad.prewarm" {
                // the client, once the first editor is on show: the others opened hidden at the pane's size (KiCad paints a pre-warmed frame at
                // the size given; at its 1280x800 default the first real show relaid the whole frame out)
                let Some(gui) = gui_route(&resolved.pro) else { return ok_reply(id, json!({"prewarming": [], "prewarmed": []})) };
                if gui.mode != "fork" || gui.desc.is_empty() || !proto::load(&gui.desc).has_field("kiapi.backplane.embed.ShowFrame", "prewarm") { return ok_reply(id, json!({"prewarming": [], "prewarmed": []})) }
                let (w, h) = (p["width"].as_u64().unwrap_or(0), p["height"].as_u64().unwrap_or(0));
                let have: Vec<String> = p["have"].as_array().into_iter().flatten().filter_map(|k| k.as_str().map(|k| canon_frame(k).to_string())).collect();
                let mut kinds = prewarm_kinds(p["active"].as_str().unwrap_or("PCB"), &have);
                // memory: a pre-warmed editor costs its frame and its libraries (Hubble_DK's schematic: ~1.5 GB, 3.9 -> 5.4 GB), worth it only
                // with room to spare; below that the first switch loads it (~1 s) instead. BACKPLANE_KICAD_PREWARM_FREE_MB overrides the floor
                let floor = std::env::var("BACKPLANE_KICAD_PREWARM_FREE_MB").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(PREWARM_FREE_MB);
                if let Some(free) = prewarm_refused(prewarm_free_mb(), floor) {
                    if !kinds.is_empty() { eprintln!("kicad embed: {}: not pre-warming {kinds:?}: {free} MB free, under {floor} MB", resolved.pro); }
                    kinds.clear();
                }
                // a document another live KiCad holds is never pre-warmed (bp.13 would open it over that lock without asking): the person's own
                // switch to it asks first (lock_gate); a stale lock is cleared
                kinds.retain(|k| {
                    let ext = match *k { "PCB" | "PCB_3D" => "kicad_pcb", "SCH" => "kicad_sch", _ => return true };
                    let doc = Path::new(&resolved.pro).with_extension(ext).to_string_lossy().to_string();
                    // never a document the project does not have (#1326: the board of a schematic-only project; KiCad would ask to create it)
                    let named = match *k { "SCH" => &resolved.files.sch, _ => &resolved.files.pcb };
                    if !Path::new(&doc).is_file() && !(!named.is_empty() && Path::new(named).is_file()) {
                        eprintln!("kicad embed: {}: not pre-warming {k}: the project has no {ext}", resolved.pro);
                        return false
                    }
                    match lock_verdict(&doc) {
                        v @ super::locks::Verdict::Foreign { .. } => { eprintln!("kicad embed: {}: not pre-warming {k}: {doc} is held by {}", resolved.pro, super::locks::holder_words(&v)); false }
                        super::locks::Verdict::Stale => { super::locks::clear_stale(&doc, "before a pre-warm"); true }
                        _ => true,
                    }
                });
                // the reply says what is built ([] when nothing is): the client's F5 guard stands down at once instead of its 60 s cap
                let building: Vec<&'static str> = kinds.clone();
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
                return ok_reply(id, json!({"prewarming": building}))
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
            if method == "editor.kicad.host" {
                // macOS: where the pane is on screen (points) and whether the client's window can be seen; KiCad places its frames there
                let Some(gui) = gui_route(&resolved.pro) else { return ok_reply(id, json!({"applied": false, "why": "KiCad is not running"})) };
                if gui.mode != "fork" || gui.desc.is_empty() || !proto::load(&gui.desc).has_own("kiapi.backplane.embed.SetHostGeometry") { return ok_reply(id, json!({"applied": false, "why": "this KiCad cannot follow a host window (needs embed-host-geometry)"})) }
                let n = |k: &str| p[k].as_i64().unwrap_or(0);
                let o = gui_call(&gui, "kiapi.backplane.embed.SetHostGeometry", json!({"x": n("x"), "y": n("y"), "width": n("width"), "height": n("height"),
                    "visible": p["visible"].as_bool().unwrap_or(true), "raise": p["raise"].as_bool().unwrap_or(false)})).await;
                return if o["ok"].as_bool().unwrap_or(false) { ok_reply(id, json!({"applied": true})) } else { err_reply(id, &format!("SetHostGeometry: {}", o["error"].as_str().unwrap_or(""))) }
            }
            if method == "editor.kicad.notice.dismiss" {
                let Some(nid) = p["id"].as_u64() else { return err_reply(id, "id is the notice's id") };
                let _ = actor_tx.send(Msg::NoticeDismiss { pro: resolved.pro, id: nid });
                return ok_reply(id, json!({}))
            }
            if method == "editor.kicad.lock" {
                let open = match p["answer"].as_str() { Some("open") => true, Some("cancel") => false, _ => return err_reply(id, "answer is \"open\" or \"cancel\"") };
                let _ = actor_tx.send(Msg::LockAnswer { pro: resolved.pro, open });
                return ok_reply(id, json!({}))
            }
            if method == "editor.kicad.dialogs" {
                let open: Vec<String> = p["open"].as_array().into_iter().flatten().filter_map(|t| t.as_str().map(String::from)).take(8).collect();
                let _ = actor_tx.send(Msg::ClientDialogs { pro: resolved.pro, open });
                return ok_reply(id, json!({}))
            }
            if method == "editor.kicad.touched" { if touched_counts(&resolved.pro) { unsaved_update(&resolved.pro, |u| u.touched()); } super::checkrun::person_touched(&resolved.pro); super::coedit::person_input(&resolved.pro); super::follow::touched(&resolved.pro); return ok_reply(id, json!({})) }
            let (tx, rx) = oneshot::channel();
            let sent = match method {
                "editor.kicad.attach" => {
                    let container = p.get("container").map(u64_of).filter(|&x| x > 0)
                        .or_else(|| p.get("window").map(u64_of)).unwrap_or(0);
                    let toplevel = p.get("toplevel").map(u64_of).unwrap_or(0);
                    let display = p["display"].as_str().unwrap_or("").to_string();
                    let wait = p["wait"].as_bool().unwrap_or(true);
                    let env = ipc::runtime(hub).await;
                    if note_runtime(&env) { hub.bump_views(); }
                    // a frameless attach of a project: never its missing board (#1326); a named frame exactly as asked
                    let on_disk = |f: &str| !f.is_empty() && Path::new(f).is_file();
                    let frame = attach_frame(raw_project, req_frame, on_disk(&resolved.files.pcb), on_disk(&resolved.files.sch)).to_string();
                    actor_tx.send(Msg::Attach {
                        client, pro: resolved.pro, root: resolved.root, files: resolved.files,
                        frame, container, toplevel, display, wait, env, reply: tx,
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
                    // (asked for every shown thread of a project here, warm or not: the runtime's capabilities are known from then,
                    // and the rail has its Simulation entry before KiCad is started)
                    let env = ipc::runtime(hub).await;
                    if note_runtime(&env) { hub.bump_views(); }
                    // BACKPLANE_KICAD_WARM=0: never (the perf e2e measures a true cold Edit with it)
                    if std::env::var("BACKPLANE_KICAD_WARM").is_ok_and(|v| v == "0") { return ok_reply(id, json!({"warm": false, "why": "off"})) }
                    let hardware = super::canonical::hardware(&resolved.root);
                    let locked = project_locked(&resolved.files);
                    let display = p["display"].as_str().unwrap_or("").to_string();
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
    #[cfg(unix)]
    #[test] fn a_restart_sweep_removes_only_this_hosts_unheld_locks() {
        use std::os::unix::io::AsRawFd;
        let root = crate::plat::tmp::dir("lock-sweep-host");
        let docs = root.path().join("boards"); std::fs::create_dir(&docs).unwrap();
        let host = super::super::locks::me().1;
        let mut kept = vec![];
        let mut removed = vec![];
        let mut holds = vec![];
        for dir in [root.path(), docs.as_path()] {
            for (name, owner, survives) in [("far", "some-other-computer", true), ("unknown", "", true), ("stale", host.as_str(), false), ("held", host.as_str(), true)] {
                let path = dir.join(format!("~{name}.kicad_pro.lck"));
                std::fs::write(&path, serde_json::json!({"hostname": owner, "username": "h", "token": "test"}).to_string()).unwrap();
                if name == "held" {
                    let f = std::fs::File::open(&path).unwrap();
                    assert_eq!(unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) }, 0);
                    holds.push(f);
                }
                if survives { kept.push(path); } else { removed.push(path); }
            }
            let malformed = dir.join("~malformed.kicad_pro.lck"); std::fs::write(&malformed, "not json").unwrap(); kept.push(malformed);
            // read_to_string fails for a directory even when this test runs as root: exercise the unreadable-lock path.
            let unreadable = dir.join("~unreadable.kicad_pro.lck"); std::fs::create_dir(&unreadable).unwrap();
            let seen = super::super::locks::inspect_file(&unreadable).expect("unreadable is not absent");
            assert!(matches!(super::super::locks::classify(Some(&seen), std::process::id(), "h", &host), super::super::locks::Verdict::Foreign { .. }));
            kept.push(unreadable);
        }
        super::sweep_crashed_locks(&root.path().to_string_lossy(), &docs.join("board.kicad_pro").to_string_lossy());
        for path in kept { assert!(path.exists(), "{} must survive the sweep", path.display()); }
        for path in removed { assert!(!path.exists(), "{} is this host's stale lock", path.display()); }
    }

    #[test] fn the_macos_refusal_names_the_update_when_the_pin_cures_it() {
        let why = mac_refusal(true, "fork", "").unwrap();
        assert!(why.contains("Settings"), "no cure: as before");
        let w = mac_refusal_with(why.clone(), Some("10.99.0-bp.13"));
        assert!(w.contains("10.99.0-bp.13") && w.contains("Update, above the pane") && !w.contains("Settings"), "{w}");
        assert_eq!(mac_refusal_with(why.clone(), None), why);
    }

    /// A stand-in kicad-cli that, like KiCad, writes its settings: to $KICAD_CONFIG_HOME when set, else to `person` (the person's
    /// kicad_common.json); it logs the folder it was given and answers --version and api-server --describe.
    #[cfg(unix)] fn writing_cli(dir: &std::path::Path, person: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let (cli, log) = (dir.join("kicad-cli"), dir.join("probe.log"));
        std::fs::write(&cli, format!("#!/bin/sh\necho \"$KICAD_CONFIG_HOME\" >> '{log}'\nif [ -n \"$KICAD_CONFIG_HOME\" ]; then mkdir -p \"$KICAD_CONFIG_HOME/10.0\"; echo rewritten > \"$KICAD_CONFIG_HOME/10.0/kicad_common.json\"; else echo rewritten > '{person}'; fi\nif [ \"$1\" = api-server ]; then echo '{{\"version\":\"10.99.0-bp.13\"}}'; else echo 10.0.8; fi\n", log = log.display(), person = person.display())).unwrap();
        std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
        (cli, log)
    }

    #[cfg(unix)] #[test] fn describing_a_kicad_never_writes_the_persons_settings() {
        let t = crate::plat::tmp::dir("kdescribe");
        let person = t.path().join("kicad_common.json"); std::fs::write(&person, "{\"mine\": 1}").unwrap();
        let (cli, log) = writing_cli(t.path(), &person);
        assert_eq!(describe_cli(&cli.to_string_lossy()).and_then(|v| v["version"].as_str().map(String::from)).as_deref(), Some("10.99.0-bp.13"));
        assert_eq!(std::fs::read_to_string(&person).unwrap(), "{\"mine\": 1}");
        let d = std::fs::read_to_string(&log).unwrap(); let d = d.trim();
        assert!(d.contains("bp-kicad-probe-") && !std::path::Path::new(d).exists(), "{d:?}");
    }

    /// The open KiCad of a project is found however the project's path is spelled: an agent naming it through a symlinked folder
    /// reaches the open editor (co-edit, the unsaved-changes rule) instead of going around it to the headless server.
    #[cfg(unix)] #[tokio::test] async fn the_open_editor_is_found_through_a_symlinked_project_folder() {
        let real = crate::plat::tmp::uncreated("route-real"); std::fs::create_dir_all(&real).unwrap();
        let link = crate::plat::tmp::uncreated("route-link"); std::os::unix::fs::symlink(&real, &link).unwrap();
        let pro = format!("{}/b.kicad_pro", real.display()); std::fs::write(&pro, "{}").unwrap();
        let (route, _rx) = stand_in_route(&pro);
        routes().lock().unwrap().insert(norm_pro_key(&pro), route);
        assert!(gui_route(&format!("{}/b.kicad_pro", link.display())).is_some(), "the link's spelling finds the open editor");
        routes().lock().unwrap().remove(&norm_pro_key(&pro));
        let _ = std::fs::remove_file(&link); let _ = std::fs::remove_dir_all(&real);
    }
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
    // the release check's fresh-home stall (Nov 16): a headless KiCad started while the editor's starts takes the project's lock first.
    // While the GUI session starts (or closes, still alive) no headless KiCad may start for the project; ready or gone, it may
    #[test] fn no_headless_kicad_while_the_editors_starts_or_closes() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let pro = "/tmp/bp-gui-starting-test/a.kicad_pro";
        let mut s = Session::new(pro, "/tmp/bp-gui-starting-test", "fork", "PCB", 1, 0);
        put_route(&s, &tx); assert!(gui_starting(pro), "starting");
        s = s.ready("/s", "", 1); put_route(&s, &tx); assert!(!gui_starting(pro), "ready: requests go to it");
        s.closing = true; put_route(&s, &tx); assert!(gui_starting(pro), "closing, still alive: its locks are not released yet");
        let stopped = s.stopped("closed"); put_route(&stopped, &tx); assert!(!gui_starting(pro), "gone");
    }
    // lane K's update gate: the hub's own KiCad being stopped holds the lock with a command line that no longer reads: its own, never asked
    #[test] fn a_lock_held_by_a_kicad_this_hub_started_is_its_own_until_that_process_is_gone() {
        use super::super::locks::Verdict;
        let foreign = |pid| Verdict::Foreign { pid: Some(pid), user: "person".into(), host: "box".into(), what: String::new() };
        assert_eq!(own_pid_verdict(foreign(6163), |p| p == 6163), Verdict::Ours(6163), "being stopped: still ours");
        assert!(matches!(own_pid_verdict(foreign(77), |p| p == 6163), Verdict::Foreign { .. }), "another process: asked");
        assert_eq!(own_pid_verdict(Verdict::Stale, |_| true), Verdict::Stale);
        // a Backplane KiCad whose hub is gone (Backplane killed, started again): an orphan, closed rather than asked about
        let me = std::process::id();
        assert_eq!(super::super::locks::orphaned(foreign(me), |_| false), foreign(me), "not a Backplane KiCad: stays foreign");
        live_pids().lock().unwrap().insert(424242);
        assert!(live_pids().lock().unwrap().contains(&424242), "a spawned KiCad is in live_pids until reaped");
        live_pids().lock().unwrap().remove(&424242);
    }
    #[test] fn kicads_lock_override_is_said_in_the_strip() {
        let ev = json!({"type": "ET_AGENT_NOTICE", "detail": json!({"kind": "lock_overridden", "actor": "", "doc": "", "details": {"message": "Project 'a.kicad_pro' is already open by 'h' at 'laptop'."}}).to_string()});
        let n = lock_notice_of(&ev).unwrap();
        assert!(n["title"].as_str().unwrap().contains("another KiCad had it open") && n["detail"].as_str().unwrap().contains("laptop") && n["error"] == true);
        assert!(lock_notice_of(&json!({"type": "ET_AGENT_NOTICE", "detail": json!({"kind": "deferred"}).to_string()})).is_none());
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
    }
    /// KiCad's ET_NOTICE (embed-notice): read from its JSON detail, kept once, dismissed only in the pane, in the session's state.
    /// Backplane KiCad installed beside the stock KiCad that is up: the next attach starts the selected one, unless work is unsaved.
    #[test] fn a_kicad_of_another_runtime_than_the_selected_one_is_started_again_unless_unsaved() {
        let (stock, fork) = ("/usr/bin/kicad-cli", "/h/kicad/10.99.0-bp.12.4/bin/kicad-cli");
        assert!(runtime_moved(stock, fork, false));
        assert!(!runtime_moved(stock, fork, true), "unsaved work keeps the KiCad that holds it");
        assert!(!runtime_moved(fork, fork, false) && !runtime_moved("", fork, false) && !runtime_moved(stock, "", false));
    }
    #[test] fn a_notice_from_kicad_is_kept_said_and_dismissed() {
        let ev = json!({"type": 29, "frame": {"kind": "FK_PCB"}, "detail": r#"{"kind":"graphics","title":"Could not use OpenGL, falling back to software rendering","detail":"GLX 1.4 missing","error":false}"#});
        let n = notice_of(&ev).unwrap();
        assert_eq!(n, json!({"kind": "graphics", "title": "Could not use OpenGL, falling back to software rendering", "detail": "GLX 1.4 missing", "error": false, "frame": "PCB"}));
        assert_eq!(notice_of(&json!({"type": "ET_NOTICE", "detail": "plain words"})).unwrap()["title"], "plain words", "a detail that is not JSON is its title");
        assert!(notice_of(&json!({"type": 29, "detail": "{}"})).is_none() && notice_of(&json!({"type": 10, "detail": "x"})).is_none(), "empty, or a dialog");
        let s = Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0).notice(n.clone()).notice(n.clone());
        assert_eq!(s.notices.len(), 1, "the same notice again is kept once");
        assert_eq!((s.notices[0]["id"].as_u64(), s.notices[0]["dismissed"].as_bool()), (Some(1), Some(false)));
        let s = s.dismiss_notice(1);
        assert_eq!(s.info_json()["notices"][0]["dismissed"], true, "dismissed: still there for an agent");
        let many = (0..10).fold(Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0), |s, i| s.notice(json!({"kind": "graphics", "title": format!("n{i}"), "detail": ""})));
        assert_eq!((many.notices.len(), many.notices[0]["title"].as_str()), (8, Some("n2")), "at most 8, the oldest go");
        let asking = Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0).ready("/s", "", 0).dialog(true, "Information");
        assert_eq!(asking.ping_result(false).ping_result(false).ping_result(false).state, "ready", "a dialog open: missed pings are not a hang");
    }

    #[test] fn the_rails_simulation_entry_needs_sim_frame() {
        let (old, new): (Vec<String>, Vec<String>) = (vec!["api-server".into(), "embed".into()], vec!["api-server".into(), "sim-run".into(), "sim-frame".into()]);
        assert_eq!(rail_frame("board", &old), Some("PCB")); assert_eq!(rail_frame("schematic", &old), Some("SCH")); assert_eq!(rail_frame("3d", &old), Some("PCB_3D"));
        assert_eq!(rail_frame("simulation", &old), None, "bp.13.1 and older: no entry");
        assert_eq!(rail_frame("simulation", &new), Some("SIMULATOR")); assert_eq!(rail_frame("Simulation", &new), Some("SIMULATOR"));
        assert_eq!(rail_frame("simulation", &["sim-run".to_string()]), None, "the headless commands alone embed nothing");
        assert_eq!(rail_frame("mech", &new), None);
        // a show of it on a session without the capability is refused in words, and never reaches KiCad
        assert!(frame_refused("SIMULATOR", &old).is_some_and(|w| w.contains("bp.13.2"))); assert_eq!(frame_refused("FK_SIMULATOR", &new), None); assert_eq!(frame_refused("SCH", &old), None);
        // the session says it to the client beside its embed-* capabilities
        let mut s = Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0); s.caps = new.clone();
        assert_eq!(s.info_json()["embed_caps"], json!(["sim-frame"]));
        // the selected runtime's capabilities, as the presenter reads them: a change is said once
        let env = |caps: &[String]| Env { cli: "/k/bin/kicad-cli".into(), caps: caps.to_vec(), ..Default::default() };
        // (on a cell of its own: every test that attaches or warms an embedded KiCad notes its runtime in the process's)
        let cell = std::sync::Mutex::new(vec![]); let note = |e: &Env| note_runtime_in(&cell, e); let rail = |r: &str| rail_simulation_in(&cell, r);
        note(&env(&old)); assert!(!note(&env(&old))); assert_eq!(rail("/nowhere/at/all"), None);
        assert!(note(&env(&new))); assert!(!note(&env(&new)));
        let t = crate::plat::tmp::dir("rail-sim"); let d = t.to_string_lossy().to_string();
        std::fs::write(t.join("a.kicad_pro"), "{}").unwrap(); std::fs::write(t.join("a.kicad_pcb"), "(kicad_pcb)").unwrap();
        assert_eq!(rail(&d), None, "a project with no schematic has nothing to simulate");
        let t2 = crate::plat::tmp::dir("rail-sim-sch"); let d2 = t2.to_string_lossy().to_string();
        std::fs::write(t2.join("a.kicad_pro"), "{}").unwrap(); std::fs::write(t2.join("a.kicad_sch"), "(kicad_sch)").unwrap();
        assert_eq!(rail(&d2), Some("SIMULATOR"));
        assert!(note(&env(&old))); assert_eq!(rail(&d2), None, "an older runtime selected: the entry goes");
    }
    #[test] fn an_agents_show_of_the_simulator_does_not_move_the_pane() {
        // RunSimulation{show} on a hidden Simulator posts ET_SHOW_REQUESTED with the API client that caused it: not followed (the pane
        // stays on what the person is looking at); the frame is in GetFrames, so the strip lists its tab
        assert_eq!(show_not_followed(true, "agent:t1", false, true, false), Some("an API client's"));
        assert_eq!(show_not_followed(true, "hub:sim", false, true, false), Some("an API client's"));
        // the person's own (Inspect > Simulator in the schematic editor: no actor) is followed
        assert_eq!(show_not_followed(true, "", false, true, false), None);
        // its unsaved workbook in the close question: the frame has no document
        let f = |title: &str| Frame { kind: "SIMULATOR".into(), doc: String::new(), title: title.into(), modified: true, shown: true, realized: true, xid: 9, min_w: 0, min_h: 0 };
        assert_eq!(unsaved_name(&f("*rc — Simulator")), "*rc — Simulator"); assert_eq!(unsaved_name(&f("")), "Simulation workbook");
        let mut s = Session::new("/p/a.kicad_pro", "/p", "fork", "SCH", 1, 0); s.frames.push(f(""));
        assert_eq!(s.unsaved_docs(), ["Simulation workbook"]);
    }
    #[test] fn a_frameless_attach_of_a_project_opens_only_a_document_it_has() {
        // #1326 (c): an agent's or the hub's editor.kicad.attach with no frame, on a .kicad_pro or a project folder
        for path in ["/p/a.kicad_pro", "/p"] {
            assert_eq!(attach_frame(path, "", true, false), "PCB", "{path}: board only");
            assert_eq!(attach_frame(path, "", false, true), "SCH", "{path}: schematic only");
            assert_eq!(attach_frame(path, "", true, true), "PCB", "{path}: both, the board as before");
            assert_eq!(attach_frame(path, "", false, false), "PCB", "{path}: neither, as before (nothing better to open)");
        }
        // a named frame is exactly as asked, documents or not (the pane always names one: Edit on the Board page stays the board)
        assert_eq!(attach_frame("/p/a.kicad_pro", "PCB", false, true), "PCB");
        assert_eq!(attach_frame("/p/a.kicad_pro", "board", false, true), "PCB");
        assert_eq!(attach_frame("/p/a.kicad_pro", "SCH", true, false), "SCH");
        // a path that names its document keeps it
        assert_eq!(attach_frame("/p/a.kicad_pcb", "", false, true), "PCB");
        assert_eq!(attach_frame("/p/a.kicad_sch", "", true, false), "SCH");
    }
    #[test] fn a_warm_up_opens_only_a_document_the_project_has() {
        // #1326: a schematic-only project warmed on the board made KiCad ask "PCB '…' does not exist. Do you wish to create it?"
        assert_eq!(frame_for_documents("PCB", true, false), Some("PCB"), "board only: the board");
        assert_eq!(frame_for_documents("PCB", false, true), Some("SCH"), "schematic only: the schematic");
        assert_eq!(frame_for_documents("PCB", true, true), Some("PCB"), "both: the board, as before");
        assert_eq!(frame_for_documents("PCB", false, false), None, "neither: nothing opened");
        assert_eq!(frame_for_documents("SCH", true, false), Some("PCB"), "a schematic asked for and none there: the board");
        assert_eq!(frame_for_documents("SCH", true, true), Some("SCH"));
        assert_eq!(frame_for_documents("PCB_3D", false, true), Some("SCH"), "the 3D viewer is the board's");
        assert_eq!(frame_for_documents("FOOTPRINT_EDITOR", false, false), Some("FOOTPRINT_EDITOR"), "an editor with no document of its own: as asked");
        // on disk: an empty or missing path is no document
        let t = crate::plat::tmp::dir("warm-docs");
        let sch = t.path().join("a.kicad_sch"); std::fs::write(&sch, "(kicad_sch)").unwrap();
        let files = |pcb: &str, sch: &str| Files { pro: String::new(), pcb: pcb.into(), sch: sch.into(), all: vec![] };
        let s = sch.to_string_lossy().to_string(); let missing = t.path().join("a.kicad_pcb").to_string_lossy().to_string();
        assert_eq!(frame_with_document("PCB", &files(&missing, &s)), Some("SCH"), "a board named but not on disk");
        assert_eq!(frame_with_document("PCB", &files("", &s)), Some("SCH"));
        assert_eq!(frame_with_document("PCB", &files(&missing, "")), None);
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
        // bp.13.2: the Simulator, never pre-warmed
        assert_eq!(canon_frame("fk_simulator"), "SIMULATOR"); assert_eq!(fk_of("SIMULATOR"), "FK_SIMULATOR");
        assert_eq!(kind_of_fk_val(&json!(8)), "SIMULATOR"); assert_eq!(kind_of_fk_val(&json!("FK_SIMULATOR")), "SIMULATOR");
        assert_eq!(prewarm_kinds("SIMULATOR", &[]), ["SCH", "PCB"]); assert!(!prewarm_kinds("PCB", &[]).contains(&"SIMULATOR"));
        assert_eq!(stock_action_for("SIMULATOR"), None);
        assert_eq!(close_mode_of("ask"), "CM_FAIL_IF_MODIFIED");
        assert_eq!(close_mode_of("save"), "CM_SAVE");
        assert_eq!(close_mode_of("discard"), "CM_DISCARD");
        assert_eq!(stock_action_for("PCB"), None);
        assert_eq!(stock_action_for("SCH"), Some("pcbnew.EditorControl.showEeschema"));
        assert_eq!(stock_action_for("PCB_3D"), Some("common.Control.show3DViewer"));

        assert_eq!(mac_refusal(false, "stock", ""), None, "not macOS: nothing changes");
        assert!(mac_refusal(true, "stock", "").is_some_and(|w| w.contains("only Backplane KiCad")));
        assert!(mac_refusal(true, "fork", "").is_some_and(|w| w.contains("Update KiCad")), "a fork with no descriptor (bp.6 on macOS)");
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

        // a death after it was ready: closed unexpectedly, said by how it ended, never by its output
        let crashed = s.clone().exited_how(137, "signal 9, SIGKILL", "Fontconfig error: Cannot load default config file: File not found\nfatal boom\n");
        assert_eq!(crashed.state, CLOSED);
        assert_eq!(crashed.reason, "KiCad closed unexpectedly (signal 9, SIGKILL)");
        let crashed = s.clone().exited(139, "x");
        assert_eq!((crashed.state.as_str(), crashed.reason.as_str()), (CLOSED, "KiCad closed unexpectedly (code 139)"));

        let clean = s.exited(0, "");
        assert_eq!(clean.state, "stopped");
    }

    #[test] fn an_agents_change_is_unsaved_until_its_file_is_written_after_it() {
        let d = crate::plat::tmp::dir("agent-unsaved"); let pcb = d.join("a.kicad_pcb"); std::fs::write(&pcb, "x").unwrap();
        let pro = d.join("a.kicad_pro").to_string_lossy().into_owned(); let doc = pcb.to_string_lossy().into_owned();
        assert!(agent_unsaved(&pro).is_empty());
        std::thread::sleep(std::time::Duration::from_millis(20)); agent_changed(&pro, &doc);
        assert_eq!(agent_unsaved(&pro), vec![doc.clone()], "KiCad says nothing; the hub knows");
        let s = Session { frames: vec![], ..Session::new(&pro, &d.to_string_lossy(), "fork", "PCB", 1, 0) };
        assert_eq!(s.unsaved_docs(), vec!["a.kicad_pcb".to_string()], "close, quit and the save gate count it");
        std::thread::sleep(std::time::Duration::from_millis(20)); std::fs::write(&pcb, "y").unwrap();
        assert!(agent_unsaved(&pro).is_empty(), "written after the change: on disk");
    }
    #[test] fn a_local_history_snapshot_newer_than_the_save_counts_as_kept() {
        let d = crate::plat::tmp::dir("hist-newer"); std::fs::write(d.join("a.kicad_pcb"), "x").unwrap();
        assert!(!history_newer(&d), "no history");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let h = d.join(".history/.git/refs/heads"); std::fs::create_dir_all(&h).unwrap(); std::fs::write(h.join("master"), "c399").unwrap();
        assert!(history_newer(&d), "a snapshot after the save");
        std::thread::sleep(std::time::Duration::from_millis(20)); std::fs::write(d.join("a.kicad_pcb"), "y").unwrap();
        assert!(!history_newer(&d), "saved after the snapshot");
    }
    /// KiCad's real local history (a git repository): what the pane says follows the snapshot's content, not its time. Made with git.
    #[test] fn a_local_history_snapshot_counts_as_kept_only_when_it_holds_what_the_files_do_not() {
        if std::process::Command::new("git").arg("--version").output().is_err() { eprintln!("no git: skipped"); return }
        let d = crate::plat::tmp::dir("hist-content"); let h = d.join(".history"); std::fs::create_dir_all(&h).unwrap();
        let git = |args: &[&str], at: u64| { let o = std::process::Command::new("git").current_dir(&h).args(["-c", "user.name=k", "-c", "user.email=k@k", "-c", "init.defaultBranch=master"]).args(args)
            .env("GIT_COMMITTER_DATE", format!("@{at} +0000")).env("GIT_AUTHOR_DATE", format!("@{at} +0000")).output().unwrap(); assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr)); };
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let snap = |text: &str, at: u64, msg: &str| { std::fs::write(h.join("a.kicad_pcb"), text).unwrap(); git(&["add", "-A"], at); git(&["commit", "-qm", msg], at); };
        git(&["init", "-q"], now);
        std::fs::write(d.join("a.kicad_pcb"), "(kicad_pcb saved)").unwrap();
        // KiCad's snapshot of a save (the hub saved an agent's change): newer than the file, the same content: nothing unsaved
        snap("(kicad_pcb saved)", now + 5, "PCB Save");
        assert_eq!(history_unsaved(&d), Some(false)); assert!(!history_newer(&d), "a save's own snapshot is not a kept copy");
        // an autosave of the person's unsaved edit: different and newer: kept, KiCad offers it back
        snap("(kicad_pcb edited)", now + 10, "Autosave");
        assert_eq!(history_unsaved(&d), Some(true)); assert!(history_newer(&d), "the person's edit is kept: never 'were not kept'");
        // the file written after that snapshot (saved later): nothing to offer back
        std::fs::write(d.join("a.kicad_pcb"), "(kicad_pcb later)").unwrap();
        let later = std::fs::File::options().write(true).open(d.join("a.kicad_pcb")).unwrap();
        later.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(now + 20)).unwrap();
        assert_eq!(history_unsaved(&d), Some(false));
        // a sub-folder's sheet in the snapshot, its file gone: kept
        std::fs::create_dir_all(h.join("sub")).unwrap(); std::fs::write(h.join("sub/b.kicad_sch"), "(kicad_sch)").unwrap(); git(&["add", "-A"], now + 30); git(&["commit", "-qm", "Autosave"], now + 30);
        assert_eq!(history_unsaved(&d), Some(true));
        // unreadable (no objects): judged by times, as before
        let e = crate::plat::tmp::dir("hist-unread"); std::fs::create_dir_all(e.join(".history/.git")).unwrap(); std::fs::write(e.join(".history/.git/HEAD"), "ref: refs/heads/master\n").unwrap();
        assert_eq!(history_unsaved(&e), None);
    }
    #[test] fn a_titled_dialog_before_the_events_loop_reaches_the_session_and_an_untitled_one_does_not() {
        let mut fwd = Vec::new();
        let open = json!({"type": "ET_DIALOG_OPENED", "detail": "Recover Unsaved Changes"});
        let Some(Msg::Dialog { open: o, title, gen, .. }) = startup_dialog("/p/a.kicad_pro", 3, &open, &mut fwd) else { panic!("no dialog") };
        assert!(o); assert_eq!((title.as_str(), gen), ("Recover Unsaved Changes", 3));
        let Some(Msg::Dialog { open: o, .. }) = startup_dialog("/p/a.kicad_pro", 3, &json!({"type": 11, "detail": "Recover Unsaved Changes"}), &mut fwd) else { panic!("no close") };
        assert!(!o && fwd.is_empty());
        assert!(startup_dialog("/p/a.kicad_pro", 3, &json!({"type": "ET_DIALOG_OPENED"}), &mut fwd).is_none());
        assert!(startup_dialog("/p/a.kicad_pro", 3, &json!({"type": "ET_FRAME_DRAWN", "detail": "x"}), &mut fwd).is_none());
    }
    #[test] fn a_close_whose_open_was_not_forwarded_takes_no_other_dialog_off_the_session() {
        // an older fork: opened untitled (not sent), closed under a title; a forwarded dialog stays open
        let mut fwd = Vec::new();
        assert!(startup_dialog("/p/a.kicad_pro", 3, &json!({"type": "ET_DIALOG_OPENED", "detail": "Recover Unsaved Changes"}), &mut fwd).is_some());
        assert!(startup_dialog("/p/a.kicad_pro", 3, &json!({"type": "ET_DIALOG_OPENED"}), &mut fwd).is_none());
        assert!(startup_dialog("/p/a.kicad_pro", 3, &json!({"type": "ET_DIALOG_CLOSED", "detail": "Loading"}), &mut fwd).is_none());
        assert_eq!(fwd, vec!["Recover Unsaved Changes".to_string()]);
    }
    #[test] fn a_macos_stall_is_kicads_recover_question_when_there_is_something_to_recover() {
        // nothing kept: the stall is a system prompt that names Backplane (the Documents prompt)
        let d = crate::plat::tmp::dir("mac-wait-none"); std::fs::write(d.join("a.kicad_pcb"), "x").unwrap();
        assert_eq!(mac_wait_reason(&d), MAC_PROMPT_REASON);
        // KiCad's autosave beside the files: its alert offers it back before its API is up
        let a = crate::plat::tmp::dir("mac-wait-autosave"); std::fs::write(a.join("a.kicad_pcb"), "x").unwrap();
        std::fs::write(a.join("_autosave-a.kicad_pcb"), "y").unwrap();
        assert_eq!(mac_wait_reason(&a), MAC_RECOVER_REASON);
        // a local-history snapshot newer than the saved files (the gate's case: an agent's unsaved change, then KiCad killed)
        let h = crate::plat::tmp::dir("mac-wait-history"); std::fs::write(h.join("a.kicad_pcb"), "x").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let r = h.join(".history/.git/refs/heads"); std::fs::create_dir_all(&r).unwrap(); std::fs::write(r.join("master"), "c399").unwrap();
        assert_eq!(mac_wait_reason(&h), MAC_RECOVER_REASON);
        // and once saved after the snapshot, nothing to recover again
        std::thread::sleep(std::time::Duration::from_millis(20)); std::fs::write(h.join("a.kicad_pcb"), "z").unwrap();
        assert_eq!(mac_wait_reason(&h), MAC_PROMPT_REASON);
    }
    #[test] fn kicad_that_lost_its_windows_is_started_again_only_when_it_says_so() {
        assert!(windows_lost(3, "x\nkicad embed: windows lost\n"));
        assert!(!windows_lost(3, "some other exit 3"), "code 3 alone is not it");
        assert!(!windows_lost(137, "kicad embed: windows lost"));
    }
    #[test] fn a_failed_start_is_said_by_an_error_line_never_by_start_up_noise() {
        let st = Session { state: "starting".into(), ..Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0) };
        let f = st.clone().exited_how(1, "code 1", "Fontconfig error: Cannot load default config file: File not found\n(kicad:12): Gtk-WARNING **: cannot open display\n");
        assert_eq!((f.state.as_str(), f.reason.as_str()), ("failed", "KiCad exited before it was ready (code 1)"), "only noise: no reason quoted");
        let f = st.clone().exited_how(1, "code 1", "Fontconfig error: Cannot load default config file\n10:00:01: Error: could not load library _pcbnew.kiface\nGLib-GIO-CRITICAL: x\n");
        assert_eq!(f.reason, "KiCad exited before it was ready (code 1): 10:00:01: Error: could not load library _pcbnew.kiface");
        assert_eq!(start_error("Fontconfig error: x\n"), None);
    }
    #[test] fn a_start_stalled_on_a_macos_prompt_waits_for_the_person_and_is_never_restarted() {
        use std::time::Duration as D;
        assert!(!mac_prompt_wait(true, None, false), "KiCad has not said not-ready");
        assert!(!mac_prompt_wait(true, Some(D::from_secs(2)), false), "a normal start says not-ready for a moment");
        assert!(mac_prompt_wait(true, Some(D::from_secs(6)), false), "not ready for 6 s with nothing of KiCad's open: a system prompt");
        assert!(!mac_prompt_wait(true, Some(D::from_secs(60)), true), "KiCad's own dialog is said as such");
        assert!(!mac_prompt_wait(false, Some(D::from_secs(60)), false), "macOS only");
        let why = mac_prompt_failure(MAC_PROMPT_BOUND);
        assert!(why.contains("10 min") && why.contains("macOS permission prompt"), "{why}");
        assert!(!start_retry(&why, false), "never a second KiCad into the same prompt");
        assert!(start_retry("KiCad did not answer Ping and GetVersion within 30 s", false), "a plain missed window still retries once");
    }
    #[test] fn a_missed_start_window_is_tried_once_more_and_nothing_else_is() {
        for why in ["KiCad did not open its IPC socket within 30 s", "KiCad did not answer Ping and GetVersion within 30 s"] {
            assert!(start_retry(why, false), "{why}: the first miss is tried again");
            assert!(!start_retry(why, true), "{why}: a second miss is said to the person");
        }
        for why in ["could not start KiCad: No such file or directory", "could not prepare /x: denied", "it did not answer Ping: refused"] {
            assert!(!start_retry(why, false), "{why}: would fail the same way");
        }
    }

    /// A KiCad that never opens its socket is started twice (the second start by itself, the person's attach waiting through both), then
    /// the pane is told it failed; a start that succeeds clears the retry (covered by the stand-in test above: one start, ready).
    #[cfg(unix)]
    #[tokio::test] async fn a_kicad_that_misses_the_start_window_is_started_once_more_then_failed() {
        use crate::plat::fs::PermissionsExt;
        let base = crate::plat::tmp::short_dir("ke-r");
        let (home_dir, proj_dir, bin_dir) = (base.join("home"), base.join("proj"), base.join("bin"));
        for d in [&home_dir, &proj_dir, &bin_dir] { std::fs::create_dir_all(d).unwrap(); }
        let pcb = proj_dir.join("slow.kicad_pcb");
        std::fs::write(proj_dir.join("slow.kicad_pro"), "{}").unwrap(); std::fs::write(&pcb, "(kicad_pcb)").unwrap();
        let cfg = crate::hub::Cfg { home: home_dir.clone(), version: "0.14.0".into(), commit: "test".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
        ipc::start(&hub); start(&hub);
        let _ = hub.commit(&json!({"$":"ProjectCreated","id":"p1","title":"slow","root":proj_dir.to_string_lossy(),"at":1})).await;
        let (cli, gui, starts) = (bin_dir.join("kicad-cli"), bin_dir.join("kicad"), base.join("starts"));
        std::fs::write(&cli, "#!/bin/sh\nif [ \"$2\" = \"--describe\" ]; then echo '{\"version\":\"10.99.0-bp.8\",\"capabilities\":[\"api-server\"]}'; exit 0; fi\necho 10.99.0\n").unwrap();
        // the stand-in KiCad counts its starts and never opens its socket (held by real-time scanning, as on bp-win); it ignores the ask to
        // close (so the retry has to kill it) and notes an overlap when the one before it is still there as it starts
        let pidf = base.join("pid");
        std::fs::write(&gui, format!("#!/bin/sh\ntrap '' TERM\np=$(cat '{pf}' 2>/dev/null)\nif [ -n \"$p\" ] && kill -0 \"$p\" 2>/dev/null; then echo overlap >> '{st}'; fi\necho $$ > '{pf}'\necho x >> '{st}'\nwhile :; do sleep 0.2; done\n", pf = pidf.display(), st = starts.display())).unwrap();
        for f in [&cli, &gui] { std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        let _ = hub.commit(&json!({"$":"SettingSet","key":"kicad.path","value":cli.to_string_lossy(),"at":2})).await;
        let (mut conn, _) = hub.connect(true);
        let t0 = std::time::Instant::now();
        let rep = hub.request(&mut conn, &json!({"id": 1, "m": "editor.kicad.attach", "p": {"project": pcb.to_string_lossy(), "container": 9001, "toplevel": 8001, "display": ":99", "frame": "PCB"}})).await;
        let log = std::fs::read_to_string(&starts).unwrap_or_default();
        assert_eq!(log.lines().filter(|l| *l == "x").count(), 2, "started once more by itself, and only once ({rep:?})");
        assert!(!log.contains("overlap"), "the second KiCad started while the first was still there: {log}");
        assert!(t0.elapsed() >= Duration::from_secs(2 * START_WINDOW_S) + RETRY_TERM_WAIT, "the attach waited through both starts and the kill: {:?}", t0.elapsed());
        assert!(rep[0].to_string().contains("did not open its IPC socket"), "then failed, with the reason: {rep:?}");
        let info: Value = serde_json::from_str(hub.info_now()[&info_key(&pcb.to_string_lossy().replace(".kicad_pcb", ".kicad_pro"))].as_str().unwrap_or("{}")).unwrap_or_default();
        assert!(info.is_null() || info["state"] == json!("failed"), "{info}");
    }

    /// A restart (the update's reopen on a new runtime) starts the next KiCad only once the running one is gone: the stand-in ignores the
    /// ask to close (so it is killed) and notes an overlap when the one before it is still there as it starts.
    #[cfg(unix)]
    #[tokio::test] async fn a_restart_starts_the_next_kicad_only_once_the_running_one_is_gone() {
        use crate::plat::fs::PermissionsExt;
        let base = crate::plat::tmp::short_dir("ke-rs");
        let (home_dir, proj_dir, bin_dir) = (base.join("home"), base.join("proj"), base.join("bin"));
        for d in [&home_dir, &proj_dir, &bin_dir] { std::fs::create_dir_all(d).unwrap(); }
        let pcb = proj_dir.join("re.kicad_pcb");
        std::fs::write(proj_dir.join("re.kicad_pro"), "{}").unwrap(); std::fs::write(&pcb, "(kicad_pcb)").unwrap();
        let cfg = crate::hub::Cfg { home: home_dir.clone(), version: "0.14.0".into(), commit: "test".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
        ipc::start(&hub); start(&hub);
        let _ = hub.commit(&json!({"$":"ProjectCreated","id":"p1","title":"re","root":proj_dir.to_string_lossy(),"at":1})).await;
        let (cli, gui, starts, pidf) = (bin_dir.join("kicad-cli"), bin_dir.join("kicad"), base.join("starts"), base.join("pid"));
        std::fs::write(&cli, "#!/bin/sh\nif [ \"$2\" = \"--describe\" ]; then echo '{\"version\":\"10.99.0-bp.8\",\"capabilities\":[\"api-server\"]}'; exit 0; fi\necho 10.99.0\n").unwrap();
        std::fs::write(&gui, format!("#!/bin/sh\ntrap '' TERM\np=$(cat '{pf}' 2>/dev/null)\nif [ -n \"$p\" ] && kill -0 \"$p\" 2>/dev/null; then echo overlap >> '{st}'; fi\necho $$ > '{pf}'\necho x >> '{st}'\nwhile :; do sleep 0.2; done\n", pf = pidf.display(), st = starts.display())).unwrap();
        for f in [&cli, &gui] { std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        let _ = hub.commit(&json!({"$":"SettingSet","key":"kicad.path","value":cli.to_string_lossy(),"at":2})).await;
        let p = json!({"project": pcb.to_string_lossy(), "container": 9001, "toplevel": 8001, "display": ":99", "frame": "PCB"});
        // the first KiCad starting (it waits for its window), then a restart while it runs
        let (h1, p1) = (hub.clone(), p.clone());
        let first = tokio::spawn(async move { let (mut c, _) = h1.connect(true); h1.request(&mut c, &json!({"id": 1, "m": "editor.kicad.attach", "p": p1})).await });
        for _ in 0..50 { if std::fs::read_to_string(&starts).unwrap_or_default().lines().count() >= 1 { break } tokio::time::sleep(Duration::from_millis(100)).await; }
        let old: u32 = std::fs::read_to_string(&pidf).unwrap_or_default().trim().parse().unwrap_or(0);
        assert!(old > 0, "the first KiCad started");
        let t0 = std::time::Instant::now();
        let (h2, p2) = (hub.clone(), p.clone());
        let again = tokio::spawn(async move { let (mut c, _) = h2.connect(true); h2.request(&mut c, &json!({"id": 2, "m": "editor.kicad.restart", "p": p2})).await });
        for _ in 0..100 { if std::fs::read_to_string(&starts).unwrap_or_default().lines().filter(|l| *l == "x").count() >= 2 { break } tokio::time::sleep(Duration::from_millis(100)).await; }
        let log = std::fs::read_to_string(&starts).unwrap_or_default();
        assert!(log.lines().filter(|l| *l == "x").count() >= 2, "the restart started the next KiCad: {log}");
        assert!(!log.contains("overlap"), "the next KiCad started while the one before was still there: {log}");
        assert!(t0.elapsed() >= RETRY_TERM_WAIT, "the restart waited for the old KiCad (it ignored the ask to close): {:?}", t0.elapsed());
        assert!(!crate::plat::proc::alive(old), "the old KiCad is gone");
        first.abort(); again.abort();
        if let Ok(p) = std::fs::read_to_string(&pidf) { if let Ok(p) = p.trim().parse::<u32>() { crate::plat::proc::kill_group(p, crate::plat::proc::KILL); } }
    }

    #[test] fn a_shown_project_is_kept_warm_within_the_cap_memory_and_locks() {
        let ready = |p: &str, t: u64, cont: u64| { let mut s = Session::new(p, "/p", "fork", "PCB", 1, t).ready("/s", "", t); s.container = cont; (p.to_string(), s) };
        let cfg = WarmCfg { max: 2, idle_s: 1800, kicad_mb: 2000, headroom_mb: 1000, pressure_max: 10.0 };
        let free = |m: u64| MemNow { free_mb: Some(m), pressure: None };
        let empty = HashMap::new();
        assert_eq!(warm_decision(&empty, "/p/a.kicad_pro", true, false, free(8000), true, cfg), Warm::Start { evict: None });
        assert_eq!(warm_decision(&empty, "/p/a.kicad_pro", true, false, MemNow::default(), true, cfg), Warm::Start { evict: None }, "memory unknown: enough");
        assert!(matches!(warm_decision(&empty, "/p/a.kicad_pro", false, false, free(8000), true, cfg), Warm::No(_)), "not hardware");
        assert!(matches!(warm_decision(&empty, "/p/a.kicad_pro", true, true, free(8000), true, cfg), Warm::No(_)), "locked by another KiCad: its dialog would come up");
        assert!(matches!(warm_decision(&empty, "/p/a.kicad_pro", true, false, free(2900), true, cfg), Warm::No(_)), "less than a KiCad and the headroom");
        assert!(matches!(warm_decision(&empty, "/p/a.kicad_pro", true, false, free(8000), false, cfg), Warm::No(_)), "a runtime that shows its load windows is never warmed");
        // a KiCad measured running sets the cost of the next (its peak): 5 GB measured, 6 GB free is not enough for another
        let mut big = Session::new("/p/b.kicad_pro", "/p", "fork", "PCB", 1, 0).ready("/s", "", 0); big.container = 3; big.peak_mb = 5000; big.rss_mb = 900;
        let m: HashMap<String, Session> = [("/p/b.kicad_pro".to_string(), big)].into_iter().collect();
        assert_eq!(kicad_cost_mb(&m, cfg), (5000, "measured"));
        assert!(matches!(warm_decision(&m, "/p/a.kicad_pro", true, false, free(5500), true, cfg), Warm::No(_)));
        assert_eq!(warm_decision(&m, "/p/a.kicad_pro", true, false, free(6500), true, cfg), Warm::Start { evict: None });
        let m: HashMap<String, Session> = [ready("/p/a.kicad_pro", 10, 0)].into_iter().collect();
        assert_eq!(warm_decision(&m, "/p/a.kicad_pro", true, true, free(100), true, cfg), Warm::Have, "ours already: its own lock and memory do not count");
        // at the cap: the oldest clean one in no pane makes room; none such: refused (never a 3rd)
        let m: HashMap<String, Session> = [ready("/p/a.kicad_pro", 10, 0), ready("/p/b.kicad_pro", 5, 0)].into_iter().collect();
        assert_eq!(warm_decision(&m, "/p/c.kicad_pro", true, false, free(8000), true, cfg), Warm::Start { evict: Some("/p/b.kicad_pro".into()) });
        let m: HashMap<String, Session> = [ready("/p/a.kicad_pro", 10, 7), ready("/p/b.kicad_pro", 5, 9)].into_iter().collect();
        assert!(matches!(warm_decision(&m, "/p/c.kicad_pro", true, false, free(8000), true, cfg), Warm::No(_)), "both in panes");
        // the person closed KiCad, or it failed: Edit starts it, not a pre-warm
        let closed = Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0).exited(0, "");
        let failed = Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0).failed("x");
        for s in [closed, failed] { let m: HashMap<String, Session> = [("/p/a.kicad_pro".to_string(), s)].into_iter().collect(); assert!(matches!(warm_decision(&m, "/p/a.kicad_pro", true, false, free(8000), true, cfg), Warm::No(_))); }
        // evicted for room (closing): warm again later
        let mut ev = Session::new("/p/a.kicad_pro", "/p", "fork", "PCB", 1, 0); ev.closing = true; let ev = ev.exited(0, "");
        let m: HashMap<String, Session> = [("/p/a.kicad_pro".to_string(), ev)].into_iter().collect();
        assert_eq!(warm_decision(&m, "/p/a.kicad_pro", true, false, free(8000), true, cfg), Warm::Start { evict: None });
    }

    #[test] fn the_warm_up_memory_rule_at_its_boundaries() {
        let cfg = WarmCfg { headroom_mb: 2048, ..WARM };
        let empty = HashMap::new();
        let at = |free: Option<u64>, pressure: Option<f64>| warm_decision(&empty, "/p/a.kicad_pro", true, false, MemNow { free_mb: free, pressure }, true, cfg);
        let need = KICAD_MB + 2048;
        // nothing measured: the default cost plus the headroom, exactly
        assert_eq!(kicad_cost_mb(&empty, cfg), (KICAD_MB, "default"));
        assert_eq!(at(Some(need), None), Warm::Start { evict: None }, "exactly enough");
        let Warm::No(why) = at(Some(need - 1), None) else { panic!("one MB short is refused") };
        assert_eq!(why, format!("too little free memory (free {} MB < KiCad {KICAD_MB} MB (default) + headroom 2048 MB)", need - 1), "the refusal says its numbers");
        assert_eq!(at(None, None), Warm::Start { evict: None }, "free unknown: enough");
        // memory pressure refuses whatever is free; just under it, or unreadable, does not
        let Warm::No(why) = at(Some(100_000), Some(10.0)) else { panic!("pressure at the limit refuses") };
        assert!(why.starts_with("memory pressure (full avg10 10.00 >= 10)"), "{why}");
        assert_eq!(at(Some(100_000), Some(9.99)), Warm::Start { evict: None });
        assert_eq!(at(Some(100_000), None), Warm::Start { evict: None }, "PSI unreadable: no pressure");
        // real PSI text: a build's capped slots (some high, full low) warm; a machine thrashing (full high) does not
        let psi = |t: &str| super::super::mem::psi_full_avg10(t);
        assert_eq!(at(Some(12_852), psi("some avg10=18.19 avg60=9.40 avg300=2.10 total=8732072\nfull avg10=0.31 avg60=0.20 avg300=0.46 total=8122749\n")), Warm::Start { evict: None });
        assert!(matches!(at(Some(12_852), psi("some avg10=64.02 avg60=40.00 avg300=12.00 total=9\nfull avg10=37.50 avg60=20.00 avg300=6.00 total=8\n")), Warm::No(w) if w.contains("full avg10 37.50")));
        // measured: a live KiCad's peak above the default is the cost; one below it (still loading, a small project) is not
        let live = |peak: u64| { let mut s = Session::new("/p/b.kicad_pro", "/p", "fork", "PCB", 1, 0).ready("/s", "", 0); s.container = 3; s.peak_mb = peak;
            [("/p/b.kicad_pro".to_string(), s)].into_iter().collect::<HashMap<String, Session>>() };
        assert_eq!(kicad_cost_mb(&live(KICAD_MB + 1), cfg), (KICAD_MB + 1, "measured"));
        assert_eq!(kicad_cost_mb(&live(400), cfg), (KICAD_MB, "default"));
        assert_eq!(kicad_cost_mb(&live(0), cfg), (KICAD_MB, "default"), "not read yet");
        let m = live(3000);
        assert_eq!(warm_decision(&m, "/p/a.kicad_pro", true, false, MemNow { free_mb: Some(5048), pressure: None }, true, cfg), Warm::Start { evict: None });
        assert!(matches!(warm_decision(&m, "/p/a.kicad_pro", true, false, MemNow { free_mb: Some(5047), pressure: None }, true, cfg), Warm::No(w) if w.contains("KiCad 3000 MB (measured)")));
        // a closed KiCad's peak no longer counts
        let mut gone = live(9000); gone.values_mut().for_each(|s| *s = s.clone().exited(0, ""));
        assert_eq!(kicad_cost_mb(&gone, cfg), (KICAD_MB, "default"));
    }
    #[test] fn a_kicads_memory_is_published_again_when_it_moves_a_step() {
        // Lane M's macOS run: the one sample published was (175, 63) at +0.6 s; the tick then read 480 -> 593 -> 715
        assert!(mem_moved((0, 0), (175, 63), 64), "the first reading");
        assert!(mem_moved((175, 63), (480, 480), 64));
        assert!(!mem_moved((480, 480), (530, 530), 64), "under a step: not again");
        assert!(mem_moved((480, 480), (544, 544), 64), "a step exactly");
        assert!(mem_moved((715, 745), (600, 745), 64), "falling counts too (the peak never falls)");
        assert!(!mem_moved((715, 745), (715, 800), 64));
        assert!(mem_moved((715, 745), (715, 809), 64), "the peak alone");
    }
    #[test] fn the_warm_up_headroom_follows_the_machine() {
        assert_eq!(warm_headroom_mb(Some(8 * 1024)), 2048, "8 GB: the least");
        assert_eq!(warm_headroom_mb(Some(16 * 1024)), 2048, "16 GB");
        assert_eq!(warm_headroom_mb(Some(24 * 1024)), 3072, "24 GB: an eighth");
        assert_eq!(warm_headroom_mb(Some(31_852)), 3981, "h@backplane's 31 GB");
        assert_eq!(warm_headroom_mb(Some(32 * 1024)), 4096, "32 GB: the most");
        assert_eq!(warm_headroom_mb(Some(64 * 1024)), 4096, "64 GB");
        assert_eq!(warm_headroom_mb(None), 4096, "unknown: the most");
        // the defaults: a bp.13.2 figure per OS, nothing like bp.12's 5.2 GB; the most headroom is today's 4 GB
        assert!(KICAD_MB_LINUX >= 1705 && KICAD_MB_LINUX < 5200 && KICAD_MB_MACOS >= KICAD_MB_LINUX && KICAD_MB_WINDOWS >= KICAD_MB_LINUX, "a medium board with its schematic and 3D (bp.13.2), not bp.12's");
        assert_eq!(WARM.headroom_mb, 4096);
        assert_eq!(WARM.pressure_max, 10.0);
    }
    #[test] fn the_post_edit_prewarm_gate_is_unchanged() {
        // editor.kicad.prewarm (the other editors opened hidden once the first is on show) keeps its own floor: 6 GB free, Linux's reading
        assert_eq!(PREWARM_FREE_MB, 6144);
        assert_eq!(prewarm_refused(Some(6143), PREWARM_FREE_MB), Some(6143));
        assert_eq!(prewarm_refused(Some(6144), PREWARM_FREE_MB), None);
        assert_eq!(prewarm_refused(None, PREWARM_FREE_MB), None, "unknown: pre-warmed");
        if !cfg!(target_os = "linux") { assert_eq!(prewarm_free_mb(), None, "off Linux the pre-warm never read free memory, and still does not"); }
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

    #[test] fn stall_total_reads_the_some_line() {
        let t = "some avg10=0.00 avg60=0.00 avg300=0.08 total=3057047\nfull avg10=0.00 avg60=0.00 avg300=0.06 total=2641386\n";
        assert_eq!(super::stall_total_us(t), Some(3057047));
        assert_eq!(super::stall_total_us("full avg10=0.00 total=5\n"), None);
    }
    #[test] fn the_task_gates_refusal_is_told_from_any_other() {
        let r = json!({"ok": false, "status": "AS_BUSY", "error": "a task is running: DRC of video (token 7f3a, for 1.2 s); kiapi.backplane.embed.ShowFrame waits until it completes (GetTaskStatus with the token); only reads are answered meanwhile"});
        assert_eq!(task_refusal(&r), Some(("DRC of video".to_string(), "7f3a".to_string())));
        assert_eq!(task_refusal(&json!({"ok": false, "status": "AS_BUSY", "error": "KiCad is showing the dialog 'x'"})), None, "a dialog is no task");
        assert_eq!(task_refusal(&json!({"ok": false, "status": "AS_BAD_REQUEST", "error": "a task is running: x"})), None);
        assert_eq!(task_refusal(&json!({"ok": true})), None);
        assert_eq!(task_refusal(&json!({"ok": false, "status": "AS_BUSY", "error": "a task is running: refill"})), Some(("refill".to_string(), String::new())), "no token: waited out by time");
    }
    /// KiCad's task gate refuses a show; the hub asks the task's status until it ends, then sends the show again, which lands.
    #[tokio::test] async fn a_show_refused_for_a_task_is_sent_again_when_it_ends() {
        let (mut route, mut rx) = stand_in_route("/p/gate.kicad_pro"); route.sock = "/stand-in-gate".into(); // its own KiCad (others count /stand-in's use)
        note_task_gate(&route.sock, &["task-gate-reads".to_string()]);
        let seen = Arc::new(Mutex::new(Vec::<String>::new())); let s2 = seen.clone();
        tokio::spawn(async move {
            let (mut shows, mut polls) = (0, 0);
            while let Some((ty, msg, reply)) = rx.recv().await {
                s2.lock().unwrap().push(ty.rsplit('.').next().unwrap_or("").to_string());
                let o = if ty.ends_with("ShowFrame") { shows += 1; if shows == 1 { json!({"ok": false, "status": "AS_BUSY", "error": "a task is running: DRC (token tk9, for 0.4 s); kiapi.backplane.embed.ShowFrame waits until it completes"}) } else { json!({"ok": true, "message": {"frame": {"window": 7}}}) } }
                    else if ty.ends_with("GetTaskStatus") { assert_eq!(msg["token"]["value"], "tk9"); polls += 1; json!({"ok": true, "message": {"state": if polls < 3 { "TS_RUNNING" } else { "TS_COMPLETED" }}}) }
                    else { json!({"ok": true, "message": {}}) };
                let _ = reply.send(o);
            }
        });
        let o = gui_call(&route, "kiapi.backplane.embed.ShowFrame", json!({"kind": "FK_PCB_EDITOR"})).await;
        assert_eq!(o["ok"], true, "{o}");
        assert_eq!(*seen.lock().unwrap(), ["ShowFrame", "GetTaskStatus", "GetTaskStatus", "GetTaskStatus", "ShowFrame"], "polled while running, sent once it ended");
        // a request outside the gated set is answered as it is by gui_call (its caller decides)
        let (mut route2, mut rx2) = stand_in_route("/p/gate2.kicad_pro"); route2.sock = "/stand-in-gate2".into();
        tokio::spawn(async move { while let Some((_, _, reply)) = rx2.recv().await { let _ = reply.send(json!({"ok": false, "status": "AS_BUSY", "error": "a task is running: DRC (token t, for 1 s)"})); } });
        assert_eq!(gui_call(&route2, "kiapi.board.commands.GetItems", json!({})).await["status"], "AS_BUSY");
        // ... and by gui_call_patient, past its wait, in words: what KiCad is busy with, nothing done, ask again (lane K: plan_get during DRC)
        let t0 = std::time::Instant::now();
        let o = gui_call_patient(&route2, "kiapi.backplane.plan.GetPlan", json!({}), "", Duration::from_millis(700)).await;
        assert!(t0.elapsed() >= Duration::from_millis(700), "it waited");
        assert_eq!((o["ok"].clone(), o["status"].clone(), o["task"].clone()), (json!(false), json!("AS_BUSY"), json!("DRC")), "{o}");
        let e = o["error"].as_str().unwrap();
        assert!(e.contains("KiCad is busy with DRC") && e.contains("Nothing was done") && e.contains("ask again") && !e.contains("token"), "{e}");
    }
    /// An agent's read (plan_get's GetPlan with derived data) refused AS_BUSY by KiCad's task gate during a 0.5 s DRC: the hub asks the
    /// task's status until it ends and reads again; the agent gets the plan, never the refusal (lane K's clean-box run on bp.13).
    #[tokio::test] async fn a_read_refused_for_a_task_is_read_again_when_it_ends() {
        let (mut route, mut rx) = stand_in_route("/p/gateread.kicad_pro"); route.sock = "/stand-in-gate-read".into();
        let seen = Arc::new(Mutex::new(Vec::<String>::new())); let s2 = seen.clone();
        tokio::spawn(async move {
            let (mut reads, mut polls) = (0, 0);
            while let Some((ty, msg, reply)) = rx.recv().await {
                s2.lock().unwrap().push(ty.rsplit('.').next().unwrap_or("").to_string());
                let o = if ty.ends_with("GetPlan") { reads += 1; if reads == 1 { json!({"ok": false, "status": "AS_BUSY", "error": "a task is running: DRC (token tk5, for 0.1 s); kiapi.backplane.plan.GetPlan waits until it completes (GetTaskStatus with the token); only reads are answered meanwhile"}) } else { json!({"ok": true, "message": {"json": "{\"revision\":4}"}}) } }
                    else if ty.ends_with("GetTaskStatus") { assert_eq!(msg["token"]["value"], "tk5"); polls += 1; json!({"ok": true, "message": {"state": if polls < 2 { "TS_RUNNING" } else { "TS_COMPLETED" }}}) }
                    else { json!({"ok": true, "message": {}}) };
                let _ = reply.send(o);
            }
        });
        let o = gui_call_patient(&route, "kiapi.backplane.plan.GetPlan", json!({"derived": true}), "agent:t1", READ_TASK_WAIT).await;
        assert_eq!(o["ok"], true, "{o}");
        assert_eq!(o["message"]["json"], "{\"revision\":4}");
        assert_eq!(*seen.lock().unwrap(), ["GetPlan", "GetTaskStatus", "GetTaskStatus", "GetPlan"], "polled while running, read again once it ended");
        // a refusal that is not the task gate's (a dialog open) is the answer at once
        let (mut route3, mut rx3) = stand_in_route("/p/gateread3.kicad_pro"); route3.sock = "/stand-in-gate-read3".into();
        tokio::spawn(async move { while let Some((_, _, reply)) = rx3.recv().await { let _ = reply.send(json!({"ok": false, "status": "AS_BUSY", "error": "the dialog 'Board Setup' is open"})); } });
        let t0 = std::time::Instant::now();
        let o = gui_call_patient(&route3, "kiapi.backplane.plan.GetPlan", json!({}), "", READ_TASK_WAIT).await;
        assert!(t0.elapsed() < Duration::from_secs(2) && o["error"] == "the dialog 'Board Setup' is open", "{o}");
    }
    /// A show waiting on the task gives way to a newer one: only the latest editor is sent when the task ends.
    #[tokio::test] async fn a_waiting_show_gives_way_to_a_newer_one() {
        let (mut route, mut rx) = stand_in_route("/p/coalesce.kicad_pro"); route.sock = "/stand-in-coalesce".into();
        note_task_gate(&route.sock, &["task-gate-reads".to_string()]);
        let sends = Arc::new(Mutex::new(0)); let s2 = sends.clone();
        tokio::spawn(async move { while let Some((ty, _, reply)) = rx.recv().await {
            if ty.ends_with("ShowFrame") { *s2.lock().unwrap() += 1; }
            let _ = reply.send(if ty.ends_with("ShowFrame") { json!({"ok": false, "status": "AS_BUSY", "error": "a task is running: DRC (token tk, for 1 s)"}) } else { json!({"ok": true, "message": {"state": "TS_RUNNING"}}) });
        } });
        let seq = next_show(&route.sock);
        let (tx, sock, desc) = (route.tx.clone(), route.sock.clone(), route.desc.clone());
        let waiting = tokio::spawn(async move { raw_gated(&tx, &sock, &desc, "kiapi.backplane.embed.ShowFrame", json!({}), 1000, "", Some(seq)).await });
        tokio::time::sleep(Duration::from_millis(300)).await;
        let _newer = next_show(&route.sock);
        let o = tokio::time::timeout(Duration::from_secs(3), waiting).await.expect("it gave way").unwrap();
        assert_eq!(o["superseded"], true, "{o}");
        assert_eq!(*sends.lock().unwrap(), 1, "the older show was not sent again");
    }
    #[test] fn only_a_runtime_with_the_gate_is_retried() {
        note_task_gate("/t/old.sock", &["api-server".to_string()]); assert!(!has_task_gate("/t/old.sock"), "older runtimes: their requests wait, nothing retried");
        note_task_gate("/t/new.sock", &["task-gate-reads".to_string()]); assert!(has_task_gate("/t/new.sock"));
    }
    #[test] fn a_task_status_is_read_as_kicad_says_it() {
        assert_eq!(task_state(&json!({"ok": true, "message": {"state": "TS_RUNNING"}})), TaskState::Running);
        assert_eq!(task_state(&json!({"ok": true, "message": {"state": "TS_COMPLETED"}})), TaskState::Ended);
        assert_eq!(task_state(&json!({"ok": true, "message": {}})), TaskState::Unknown, "proto3 leaves TS_UNKNOWN out: asked once more");
        assert_eq!(task_state(&json!({"ok": false, "status": "AS_BAD_REQUEST", "error": "unknown task token tk9"})), TaskState::Ended);
    }
    #[test] fn only_the_latest_show_is_sent() {
        let sock = "/t/coalesce.sock";
        let a = next_show(sock); assert!(show_current(sock, a));
        let b = next_show(sock); assert!(!show_current(sock, a), "an older show waiting on the task gives way"); assert!(show_current(sock, b));
        assert!(show_current("/t/other.sock", 99), "another KiCad's shows are its own");
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

    #[test] fn the_app_gone_closes_every_kicad_with_nothing_unsaved() {
        let mk = |pro: &str, state: &str, dirty: bool| { let mut s = Session::new(pro, "/p", "fork", "PCB", 1, 0); s.state = state.into(); if dirty { s.frames.push(Frame { kind: "PCB".into(), doc: "a.kicad_pcb".into(), title: String::new(), modified: true, shown: true, realized: true, xid: 1, min_w: 0, min_h: 0 }); } s };
        let mut m = HashMap::new();
        for (p, st, d) in [("/p/a", "ready", false), ("/p/b", "ready", true), ("/p/c", "starting", false), ("/p/d", "stopped", false), ("/p/e", "unresponsive", false)] { m.insert(p.to_string(), mk(p, st, d)); }
        assert_eq!(app_gone_victims(&m), vec!["/p/a", "/p/c", "/p/e"], "unsaved work and stopped ones are left");
        assert_eq!(WARM.idle_s, 300, "a KiCad in no pane closes after five minutes");
    }
    #[test] fn kicads_own_show_request_just_after_the_persons_switch_is_not_followed() {
        let pro = "/p/switch-hold.kicad_pro";
        assert!(!shows_held(pro));
        hold_shows_after_switch(pro);
        assert!(shows_held(pro), "the board KiCad asks for while the schematic opens cold: held");
        assert_eq!(show_not_followed(true, "", false, true, shows_held(pro)), Some("while the hub drives it"));
        assert_eq!(show_not_followed(true, "person:x", false, true, shows_held(pro)), None, "the person's own (attributed) request is followed");
        hold_shows(pro, Duration::from_secs(60)); hold_shows_after_switch(pro);
        assert!(host_holds().lock().unwrap()[pro] > std::time::Instant::now() + Duration::from_secs(30), "a longer hold of the hub's own is kept");
        host_holds().lock().unwrap().remove(pro);
    }
    #[test] fn a_seeded_config_zooms_at_the_pointer_and_an_older_one_is_set_once() {
        let dir = crate::plat::tmp::uncreated("seedzoom");
        let d = dir.join("kicad").join("10.99"); std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("kicad_common.json"), r#"{"api":{"enable_server":true},"input":{"center_on_zoom":true,"zoom_speed":3}}"#).unwrap();
        seed_stock_config(&dir).unwrap();
        let read = |v: &str| serde_json::from_str::<Value>(&std::fs::read_to_string(dir.join("kicad").join(v).join("kicad_common.json")).unwrap()).unwrap();
        assert_eq!(read("10.0")["input"]["center_on_zoom"], json!(false), "a new config");
        assert_eq!(read("10.99")["input"], json!({"center_on_zoom": false, "zoom_speed": 3}), "an older config: set, the rest kept");
        // KiCad never ran here (no file): a small one is made, once
        let fresh = crate::plat::tmp::uncreated("freshzoom").join("10.99"); wheel_zoom_once(&fresh);
        assert_eq!(serde_json::from_str::<Value>(&std::fs::read_to_string(fresh.join("kicad_common.json")).unwrap()).unwrap()["input"]["center_on_zoom"], json!(false));
        // the person turns it back on in KiCad: kept
        std::fs::write(d.join("kicad_common.json"), r#"{"input":{"center_on_zoom":true}}"#).unwrap();
        seed_stock_config(&dir).unwrap();
        assert_eq!(read("10.99")["input"]["center_on_zoom"], json!(true));
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
            // the side panels hidden by the keys KiCad 10 reads (it ignored show_properties_panel and the like: Properties stayed shown)
            for k in ["show_layer_manager", "show_properties", "show_search", "show_net_inspector", "design_blocks_show"] { assert_eq!(p["aui"][k], json!(false), "{ver} pcbnew {k}"); }
            let e: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("kicad").join(ver).join("eeschema.json")).unwrap()).unwrap();
            for k in ["show_properties", "show_search", "show_schematic_hierarchy", "design_blocks_show"] { assert_eq!(e["aui"][k], json!(false), "{ver} eeschema {k}"); }
            assert!(p["aui"].get("show_properties_panel").is_none(), "no name KiCad does not read");
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

        // no wall clock (every wait is a bounded poll on its condition, 180 s): Xvfb picks a free display itself and says its number on stdout once it accepts
        // clients (-displayfd), so two runs never race for one display and KiCad never starts before its display is up. (A display chosen here by its lock
        // file raced other runs, and the range filled with the locks of Xvfbs ended by SIGKILL: the test skipped from then on.)
        const BOUND: Duration = Duration::from_secs(180);
        let mut xvfb_cmd = crate::proc::std_cmd(crate::proc::Kind::Local, &xvfb_bin);
        xvfb_cmd.args(["-displayfd", "1", "-screen", "0", "1280x800x24", "-nolisten", "tcp", "-xkbdir", "/usr/share/X11/xkb"])
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
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

        let out = xvfb.stdout.take().unwrap();
        let said = tokio::task::spawn_blocking(move || { let mut l = String::new(); let _ = std::io::BufRead::read_line(&mut std::io::BufReader::new(out), &mut l); l });
        let said = tokio::time::timeout(BOUND, said).await.expect("Xvfb said no display within 180 s").unwrap();
        let disp = format!(":{}", said.trim().parse::<u32>().unwrap_or_else(|_| panic!("Xvfb could not open a display (it said {said:?})")));
        // the start window a real KiCad needs on a loaded machine: its socket and first answers are waited for (polled), bounded at 180 s; the stand-in tests keep their short one
        test_window_for(&proj_dir, BOUND.as_secs()); assert_eq!(test_window(&files.pro), Some(BOUND.as_secs()), "this project's start has the long window");

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
        let t = std::time::Instant::now();
        while crate::plat::proc::alive(pid) && t.elapsed() < BOUND { tokio::time::sleep(Duration::from_millis(50)).await; }
        assert!(!crate::plat::proc::alive(pid), "stock KiCad pid={pid} should exit after close");
        *kicad_pid.lock().unwrap() = 0;
        // TERM, never KILL: Xvfb removes its lock file and socket only when it ends itself
        let _ = crate::plat::proc::kill(xvfb_pid, crate::plat::proc::TERM);
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
        let a = clean.agent_begins(false).agent_committed(); assert_eq!(a.refuse_save("a.kicad_pcb"), "");
        // a runtime that says no actor: the person had the editor and the board is modified: refused, whoever made the change
        let p = a.touched(); let why = p.refuse_save("a.kicad_pcb");
        assert!(why.starts_with("Not saved: the person may have unsaved edits of their own in KiCad (a.kicad_pcb)"), "{why}");
        // #1169: the words never send the agent to make the change a second time
        assert!(why.contains("do not send it again") && !why.contains("ask again"), "{why}");
        // edits pending before the agent's first commit are the person's
        let pending = clean.seen(true).agent_begins(false).agent_committed(); assert!(!pending.refuse_save("b").is_empty());
        // #1169: unless the hub's record says an agent made them (another thread's, a save: false left pending)
        assert_eq!(clean.seen(true).agent_begins(true).agent_committed().refuse_save("b"), "");
        // a touch without changes refuses nothing; a save (nothing modified) clears it all
        assert_eq!(clean.touched().refuse_save("c"), ""); assert_eq!(p.seen(false), Unsaved::default());
        // a co-editing KiCad's attributed edit of the person's counts on its own
        assert!(!a.person_edited().refuse_save("d").is_empty() && clean.person_edited().modified);
    }
    /// #1169: on a co-editing KiCad only the person's own change counts, never an agent's or the hub's, and a pan (no event) nothing.
    #[test] fn only_the_persons_attributed_edit_counts() {
        use serde_json::json;
        let ev = |actor: &str| json!({"type": "ET_DOCUMENT_EDITED", "detail": json!({"seq": 4, "doc": "pcb", "actor": actor, "description": "Rotate"}).to_string()});
        assert!(super::is_persons_edit(&ev("")) && super::is_persons_edit(&ev("person:backplane-7")) && super::is_persons_edit(&json!({"type": 20, "detail": json!({"actor": "", "undo": true}).to_string()})));
        assert!(!super::is_persons_edit(&ev("agent:t9")) && !super::is_persons_edit(&ev("hub:bom")) && !super::is_persons_edit(&json!({"type": "ET_SELECTION_CHANGED", "detail": "{}"})));
        // no editor route: the old rule (counted)
        assert!(super::touched_counts("/nowhere/x.kicad_pro"));
    }
}

}

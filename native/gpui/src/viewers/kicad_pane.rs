//! The real KiCad editors in the viewer pane (docs/embedded-editors.md): the hub runs the project's GUI KiCad (kicad/embed.rs) and publishes
//! info `editor.kicad.<digest>`; this page makes a container window in the client's window (native_child.rs), tells the hub to attach KiCad to
//! it (`editor.kicad.attach`), and after every frame gpui draws puts the container over the pane with the rail's editor (Board: PCB,
//! Schematic: SCH, 3D: PCB_3D) mapped in it and the others unmapped, cutting holes where gpui drew something over the pane (the palette, a menu).
//! Stock KiCad on Linux makes its frames as top-levels: a tick adopts them into the container (the fork's `--embedded` makes them there).
//! On Windows, only the fork is embedded (`WS_CHILD` under the container HWND); stock mode explains that Backplane KiCad is needed.
use super::native_child as nc;
use crate::{hub::Command, screens::settings::Secret};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// The client window's native handle (X11 XID or Win32 HWND; None elsewhere).
fn top_of(window: &Window) -> Option<u64> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle as R};
    HasWindowHandle::window_handle(window).ok().map(|h| h.as_raw()).and_then(|h| match h {
        R::Xcb(x) => Some(x.window.get() as u64),
        R::Xlib(x) => Some(x.window as u64),
        R::Win32(w) => Some(w.hwnd.get() as usize as u64),
        _ => None,
    })
}

/// The editor tabs: (kind, label). Board, Schematic and 3D always; the symbol and footprint editors once KiCad has them (or the person asks).
pub const TABS: &[(&str, &str)] = &[("PCB", "Board"), ("SCH", "Schematic"), ("PCB_3D", "3D"), ("SYMBOL_EDITOR", "Symbols"), ("FOOTPRINT_EDITOR", "Footprints")]; // i18n-ok: wire kinds; labels go through trs
/// The tab Ctrl+Tab (step 1) or Ctrl+Shift+Tab (step -1) goes to from `at`, round the tabs.
pub fn next_tab(at: &str, step: i32) -> &'static str {
    let i = TABS.iter().position(|(k, _)| *k == at).unwrap_or(0) as i32;
    TABS[(i + step).rem_euclid(TABS.len() as i32) as usize].0
}

/// The hub's frame kind for a rail page.
pub fn frame_for(open: &str) -> &'static str { match open { "schematic" => "SCH", "3d" => "PCB_3D", _ => "PCB" } } // i18n-ok: wire frame kind enum names

fn is_abs_path(s: &str) -> bool {
    s.starts_with('/') || s.starts_with('\\')
        || (s.len() >= 3 && s.as_bytes()[0].is_ascii_alphabetic() && s.as_bytes()[1] == b':' && matches!(s.as_bytes()[2], b'/' | b'\\'))
}

fn norm_path(s: &str) -> String {
    let mut n = s.replace('\\', "/");
    while n.len() > 1 && n.ends_with('/') && !(n.len() == 3 && n.as_bytes()[1] == b':') {
        n.pop();
    }
    if n.len() >= 2 && n.as_bytes()[0].is_ascii_alphabetic() && n.as_bytes()[1] == b':' {
        n.make_ascii_lowercase();
    }
    n
}

/// The project file the hub attaches to, from a viewer key `<kind>|<root>|<file>` (the file relative to the root, or absolute).
pub fn project_of(key: &str) -> Option<String> {
    let mut p = key.splitn(3, '|'); let (_, root, file) = (p.next()?, p.next()?, p.next()?);
    let r = root.trim_end_matches(['/', '\\']);
    if file.is_empty() { return (!r.is_empty()).then(|| r.to_string()) }
    if is_abs_path(file) || r.is_empty() { return Some(file.to_string()) }
    let sep = if r.contains('\\') && !r.contains('/') { '\\' } else { '/' };
    Some(format!("{r}{sep}{file}"))
}

/// The project root directory from a viewer key `<kind>|<root>|<file>`, trimmed of any trailing `/` or `\`.
pub fn root_of(key: &str) -> Option<String> {
    let mut p = key.splitn(3, '|'); let (_, root, _) = (p.next()?, p.next()?, p.next()?);
    let r = root.trim_end_matches(['/', '\\']);
    (!r.is_empty()).then(|| r.to_string())
}

/// The `(project, root)` to attach KiCad to: the viewer key's own path when the hub's picture has arrived, else the sidebar project's `root`
/// (an optimistic rail press draws `Edit` before the hub's `viewer.key` round-trip completes).
pub fn edit_target(data: &Value, key: &str, pid: &str) -> Option<(String, String)> {
    if let Some(project) = project_of(key) {
        return Some((project, root_of(key).unwrap_or_default()));
    }
    let root = crate::screens::projects::find(data, pid)
        .and_then(|p| p["root"].as_str())
        .map(|r| r.trim_end_matches(['/', '\\']))
        .filter(|r| !r.is_empty())?;
    Some((root.to_string(), root.to_string()))
}

/// Path without any file extension on its final segment (dots in parent folder names are preserved).
pub fn path_stem(s: &str) -> &str {
    let s = s.trim_end_matches(['/', '\\']);
    let base_at = s.rfind(['/', '\\']).map_or(0, |i| i + 1);
    match s[base_at..].rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => &s[..base_at + stem.len()],
        _ => s,
    }
}

/// Whether a pane opened for `(pane_proj, pane_root, pane_pid)` belongs to the viewer showing `(viewer_key, cur_pid)`.
pub fn belongs_to(pane_proj: &str, pane_root: &str, pane_pid: &str, viewer_key: &str, cur_pid: &str) -> bool {
    if let Some(vr) = root_of(viewer_key) {
        if !pane_root.is_empty() { return norm_path(pane_root) == norm_path(&vr) }
    }
    if let Some(vp) = project_of(viewer_key) {
        let nr = norm_path(pane_root);
        let nvp = norm_path(&vp);
        if norm_path(path_stem(pane_proj)) == norm_path(path_stem(&vp))
            || (!nr.is_empty() && nvp.starts_with(&format!("{nr}/")))
        {
            return true;
        }
    }
    !pane_pid.is_empty() && pane_pid == cur_pid
}

/// The window to show for `want` among the container's frames, preferring the hub's own list (fork mode: kinds it knows) over titles.
/// In fork mode, wait until `info["frames"]` lists the window as shown so a half-opened frame parked in `holder` during `ShowFrame` is never mapped early.
pub fn pick(want: &str, info: &Value, frames: &[nc::Frame]) -> Option<u64> { pick_local(want, info, frames, &[]) }
/// `pick`, where `ever` are the fork's frames shown at least once (bp.9's local map: the client maps such a frame itself, no round trip).
pub fn pick_local(want: &str, info: &Value, frames: &[nc::Frame], ever: &[u64]) -> Option<u64> {
    if info["mode"] == "fork" && info["local_map"] == true {
        let x = info["frames"].as_array().into_iter().flatten().find(|f| f["kind"] == want).and_then(|f| f["xid"].as_u64());
        if let Some(x) = x.filter(|x| ever.contains(x) && frames.iter().any(|f| f.window == *x)) { return Some(x) }
    }
    if info["mode"] == "fork" {
        let hub = info["frames"].as_array().into_iter().flatten().find(|f| f["kind"] == want && f["shown"] != false).and_then(|f| f["xid"].as_u64());
        return hub.filter(|x| frames.iter().any(|f| f.window == *x));
    }
    let hub = info["frames"].as_array().into_iter().flatten().find(|f| f["kind"] == want).and_then(|f| f["xid"].as_u64());
    hub.filter(|x| frames.iter().any(|f| f.window == *x)).or_else(|| frames.iter().find(|f| f.kind == want).map(|f| f.window))
}

/// Pure OS-aware status text when the editor is not on show, from the hub's state (`None`: the editor is there).
pub fn waiting_on(info: &Value, have_frame: bool, windows: bool) -> Option<String> {
    if windows && info["mode"] == "stock" {
        return Some(crate::i18n::trs("The embedded editor needs Backplane KiCad."));
    }
    match info["state"].as_str().unwrap_or("") {
        "failed" => Some(format!("{} {}", crate::i18n::trs("KiCad could not start:"), info["reason"].as_str().unwrap_or(""))),
        "unresponsive" => Some(crate::i18n::trs("KiCad is not responding.")),
        "stopped" => Some(crate::i18n::trs("KiCad was closed.")),
        _ if have_frame => None,
        // KiCad has a dialog of its own open (a notice from loading a file, its own progress): said, not a silent "Starting KiCad…"
        _ if info["dialogs"].as_array().is_some_and(|d| !d.is_empty()) => {
            let title = info["dialogs"].as_array().and_then(|d| d.last()).and_then(Value::as_str).unwrap_or("");
            Some(crate::i18n::tf("gpui.c.kicad.dialog", &[("title", title.to_string().into())]))
        }
        _ => Some(crate::i18n::trs("Starting KiCad…")),
    }
}

/// What the pane says when the editor is not on show, from the hub's state (None: the editor is there).
pub fn waiting(info: &Value, have_frame: bool) -> Option<String> {
    waiting_on(info, have_frame, cfg!(windows))
}

/// Decide whether a render pass should switch the editor's page from `hub_open` / `pressed_open`,
/// without letting a stale `hub_open` revert a local `set_open` while the hub's view round-trip is still plotting.
pub fn hub_open_step<'a>(cur_open: &str, last_hub_open: &mut String, hub_open: &'a str, pressed_open: Option<&'a str>) -> Option<&'a str> {
    if let Some(p) = pressed_open {
        return (p != cur_open).then_some(p);
    }
    if hub_open != last_hub_open {
        *last_hub_open = hub_open.to_string();
        if matches!(hub_open, "board" | "schematic" | "3d") && hub_open != cur_open { // i18n-ok: rail page kinds
            return Some(hub_open);
        }
    }
    None
}

pub struct KicadPane {
    /// the .kicad_pro the hub resolved this pane's project to (its attach reply), "" until then
    pub bound: String,
    pub project: String, pub root: String, pub project_id: String, pub open: String, last_hub_open: String, hub: tokio::sync::mpsc::Sender<Command>,
    container: Option<u64>, toplevel: u64, attached: bool, pub info: Value, frames: Vec<nc::Frame>,
    marker: Arc<Mutex<Option<HitboxId>>>, last_root: (i32, i32), placed: Option<(Option<nc::R>, Option<u64>, Vec<nc::R>)>,
    focus: FocusHandle, keys: bool, pub light: bool, _subs: Vec<Subscription>,
    /// the fork's holder already in our save-set; reserved chords pressed in KiCad, waiting to be pressed in gpui
    kept: u64, press: Vec<String>,
    /// stock KiCad: the editor last asked for with its shortcut, and when; auxiliary frame (Symbol/Footprint/3D) opened from inside KiCad
    poked: Option<(String, std::time::Instant)>, unmapped_since: Option<(u64, std::time::Instant)>, override_kind: Option<String>, pending_show: Option<String>,
    /// the theme told to the hub at least once; the main window (its first home), where Pop back in returns it
    theme_sent: bool, home: u64,
    /// files changed on disk that KiCad holds back over the person's unsaved edits (info `kicad.reload.<digest>`)
    pub conflicts: Vec<String>,
    /// KiCad's long jobs now (bp.12 ET_PROGRESS via info `kicad.progress.<digest>`): id, title, detail, fraction (-1 indeterminate), cancellable
    pub jobs: Vec<Value>,
    /// the fork's frames KiCad has shown at least once (bp.9: the client may map and unmap them itself, so a tab switch is local)
    ever_shown: Vec<u64>,
    /// the other editors were asked pre-warmed (once per KiCad process: its pid)
    prewarmed: Option<u64>,
    /// when the first editor was placed over the pane (the pre-warm waits 2 s after it)
    first_placed: Option<std::time::Instant>,
    /// where the pane was last drawn (window coordinates): the comment window opens over it (screens/kcomment.rs)
    pub at_px: Option<Bounds<Pixels>>,
    /// KiCad's dialogs on screen now (their titles), as the client's X connection sees them
    dialogs_open: Vec<String>,
    /// the frame KiCad gave the keyboard to on a click, and when (a click-to-focus window manager may take it back at once)
    took: Option<(u64, std::time::Instant)>,
}

impl KicadPane {
    pub fn new(cx: &mut Context<Self>, project: String, root: String, project_id: String, open: String, hub_open: String, hub: tokio::sync::mpsc::Sender<Command>) -> Self {
        // the tick: adopt stock KiCad's new frames, read the container's frames, and give the editor the keyboard on a click over it
        cx.spawn(async move |weak, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(40)).await;
            if weak.update(cx, |this, cx| this.tick(cx)).is_err() { break }
        }).detach();
        KicadPane { bound: String::new(), project, root, project_id, open, last_hub_open: hub_open, hub, container: None, toplevel: 0, attached: false, info: Value::Null, frames: Vec::new(), marker: Arc::default(), last_root: (i32::MIN, 0), placed: None,
            focus: cx.focus_handle(), keys: false, light: false, _subs: Vec::new(), kept: 0, press: Vec::new(), poked: None, unmapped_since: None, override_kind: None, pending_show: None, theme_sent: false, home: 0, conflicts: Vec::new(), jobs: Vec::new(), ever_shown: Vec::new(), prewarmed: None, first_placed: None, at_px: None, dialogs_open: Vec::new(), took: None }
    }
    fn rpc(&self, name: &str, params: Value) { let _ = self.hub.try_send(Command::Rpc(name.to_string(), Secret(params), "kicad")); }
    /// The hub's `editor.kicad.<digest>` for this project changed.
    pub fn set_info(&mut self, info: Value, cx: &mut Context<Self>) {
        if info == self.info { return }
        if info["pid"] != self.info["pid"] { self.ever_shown.clear(); }
        // shown once, or realized (bp.9: KiCad says the client may map it itself, e.g. a pre-warmed editor): the client's to map
        for f in info["frames"].as_array().into_iter().flatten().filter(|f| f["shown"] == true || f["realized"] == true) { if let Some(x) = f["xid"].as_u64() { if !self.ever_shown.contains(&x) { self.ever_shown.push(x); } } }
        // KiCad became ready (a launch, a restart): it is told Backplane's theme again
        if info["state"] == "ready" && (self.info["state"] != "ready" || info["pid"] != self.info["pid"]) && self.theme_sent {
            self.rpc("editor.kicad.theme", json!({"project": self.project, "mode": if self.light { "light" } else { "dark" }}));
        }
        if info["pid"] != self.info["pid"] {
            let had_pid = !self.info["pid"].is_null();
            self.poked = None; self.unmapped_since = None; self.kept = 0; self.placed = None; self.override_kind = None;
            if had_pid { self.pending_show = None; }
        }
        if matches!(info["state"].as_str(), Some("failed" | "stopped")) { self.pending_show = None; }
        if let Some(act) = info["active"].as_str().filter(|a| !a.is_empty()) {
            if self.pending_show.as_deref() == Some(act) {
                self.pending_show = None;
            } else if self.pending_show.is_none() && Some(act) != self.info["active"].as_str() {
                self.override_kind = (act != frame_for(&self.open)).then(|| act.to_string());
            }
        }
        if info["mode"] == "fork" && self.info["mode"] != "fork" { if let Some(c) = self.container { nc::grab_click(c, false); } }
        if info["frames"] != self.info["frames"] {
            if let Some(c) = self.container { self.frames = nc::frames(c); }
            self.placed = None; self.unmapped_since = None;
        }
        self.info = info;
        cx.notify();
    }
    pub fn matches(&self, info: &Value) -> bool {
        let p = info["project"].as_str().unwrap_or("");
        // the hub's own name for this pane's project, from its attach reply: the .kicad_pro may sit below the folder the viewer named
        // (a repository whose board is in boards/<name>/), so the viewer's path alone did not match (Nov 11, SOT23_Card)
        if !self.bound.is_empty() && norm_path(p) == norm_path(&self.bound) { return true }
        let root = info["root"].as_str().unwrap_or("");
        (!p.is_empty() && norm_path(path_stem(p)) == norm_path(path_stem(&self.project)))
            || (!root.is_empty() && (norm_path(root) == norm_path(&self.project) || (!self.root.is_empty() && norm_path(root) == norm_path(&self.root))))
    }
    pub fn needs_sync_open(&self, hub_open: &str, pressed_open: Option<&str>) -> bool {
        pressed_open.map_or(hub_open != self.last_hub_open, |p| p != self.open)
    }
    pub fn sync_hub_open(&mut self, hub_open: &str, pressed_open: Option<&str>, cx: &mut Context<Self>) {
        if let Some(next) = hub_open_step(&self.open, &mut self.last_hub_open, hub_open, pressed_open).map(str::to_string) {
            self.set_open(&next, cx);
        }
    }
    /// The rail moved to another page: show its editor (the hub opens it when KiCad has not yet).
    pub fn set_open(&mut self, open: &str, cx: &mut Context<Self>) {
        if self.open == open && self.override_kind.is_none() { return }
        self.open = open.to_string();
        self.override_kind = None;
        self.unmapped_since = None;
        let kind = frame_for(open);
        // KiCad was closed (or failed): the rail's page starts it again, instead of a "stopped" refusal from show
        if self.attached && matches!(self.info["state"].as_str(), Some("stopped" | "failed")) { self.reattach(); }
        else if self.attached && self.locally_shown(kind) { self.placed = None; }
        else if self.attached {
            self.pending_show = (self.info["active"].as_str() != Some(kind)).then(|| kind.to_string());
            self.rpc("editor.kicad.show", json!({"project": self.project, "frame": kind, "focus": self.keys}));
        }
        cx.notify();
    }
    pub fn has_override(&self) -> bool { self.override_kind.is_some() }
    /// The passive grab that turns a click on the unfocused editor into the keyboard for it: stock KiCad only. The fork takes the keyboard on
    /// its own click (and says so with `_BACKPLANE_FOCUS`); a grab there froze the click until the pane's next tick replayed it, after the
    /// button was up, and KiCad dropped the keys until a second click (one click must do).
    fn grab_click(&self, c: u64, on: bool) { nc::grab_click(c, on && self.info["mode"] != "fork") }
    /// KiCad's frames moved into a new container in the window `t` (and the hub told), before the window they were in goes away: a window's
    /// destruction takes its children with it, and KiCad's would be among them. Also how a pane goes back in from its own window.
    pub fn rehome(&mut self, t: u64) {
        let Some(old) = self.container else { return };
        if t == self.toplevel { return }
        let Some(new) = nc::container_for(t, &self.project) else { return };
        if new != old { nc::move_frames(old, new); nc::destroy(old); }
        self.toplevel = t; self.container = Some(new); self.placed = None; self.unmapped_since = None; self.last_root = (i32::MIN, 0);
        self.grab_click(new, !self.keys);
        self.reattach();
    }
    /// The hub's `kicad.reload.<digest>`: this project's waiting files, if it is this pane's.
    pub fn set_reload(&mut self, v: Value, cx: &mut Context<Self>) {
        if !self.matches(&v) { return }
        let c: Vec<String> = v["conflicts"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(String::from)).collect();
        if c != self.conflicts { self.conflicts = c; cx.notify(); }
    }
    /// The hub's `kicad.progress.<digest>`: this project's running jobs, if it is this pane's.
    pub fn set_progress(&mut self, v: Value, cx: &mut Context<Self>) {
        if !self.matches(&v) { return }
        let j = v["jobs"].as_array().cloned().unwrap_or_default();
        if j != self.jobs { self.jobs = j; cx.notify(); }
    }
    /// The × on a job's bar.
    pub fn cancel_job(&mut self, id: u64) { self.rpc("editor.kicad.cancel", json!({"project": self.project, "id": id})); }
    /// The person's answer to a file changed on disk (Keep yours / Reload), for every waiting file.
    pub fn resolve(&mut self, choice: &str, cx: &mut Context<Self>) {
        for f in std::mem::take(&mut self.conflicts) { self.rpc("editor.kicad.resolve", json!({"project": self.project, "file": f, "choice": choice})); }
        cx.notify();
    }
    /// The main window's native handle (the pane's first home).
    pub fn home(&self) -> u64 { self.home }
    /// A tab pressed: that editor in the pane (KiCad opens it when it has not yet); the rail's page stays as it is.
    pub fn show_kind(&mut self, kind: &str, cx: &mut Context<Self>) {
        if kind == frame_for(&self.open) { self.override_kind = None; } else { self.override_kind = Some(kind.to_string()); }
        self.placed = None;
        if self.attached && matches!(self.info["state"].as_str(), Some("stopped" | "failed")) { self.reattach(); }
        else if self.attached && self.locally_shown(kind) {}
        else if self.attached { self.pending_show = Some(kind.to_string()); self.rpc("editor.kicad.show", json!({"project": self.project, "frame": kind})); }
        cx.notify();
    }
    /// bp.10+ local map: `kind`'s frame is one KiCad has shown or pre-warmed, so the client maps it itself and asks KiCad nothing. A hub
    /// ShowFrame here hid the other frames in KiCad while the client was mapping them (a quick Board, 3D, Board left the board's Show a
    /// no-op until the hub's re-show ~0.9 s later).
    /// Not on Windows: there a frame shown only by the client (`ShowWindowAsync`) kept its canvas unpainted, showing the board's pixels
    /// under the Schematic Editor's toolbars (bp.10 and bp.12 alike), so the hub's ShowFrame still switches it.
    fn locally_shown(&self, kind: &str) -> bool {
        !cfg!(windows) && self.info["mode"] == "fork" && self.info["local_map"] == true
            && pick_local(kind, &self.info, &self.frames, &self.ever_shown).is_some_and(|x| self.ever_shown.contains(&x))
    }
    pub fn tab(&self) -> String { self.wanted_kind().to_string() }
    /// Backplane's theme: KiCad follows it (the fork switches live; stock KiCad keeps its own).
    pub fn set_light(&mut self, light: bool, cx: &mut Context<Self>) {
        if self.light == light && self.theme_sent { return }
        self.light = light; self.theme_sent = true;
        if self.attached { self.rpc("editor.kicad.theme", json!({"project": self.project, "mode": if light { "light" } else { "dark" }})); }
        cx.notify();
    }
    /// The hub's state with KiCad's dialogs as this window sees them on screen (a modal one stops KiCad's events, so the hub may not know yet).
    fn seen_info(&self) -> Value {
        let mut i = self.info.clone();
        if !self.dialogs_open.is_empty() && i["dialogs"].as_array().map_or(true, |d| d.is_empty()) { i["dialogs"] = json!(self.dialogs_open); }
        i
    }
    fn wanted_kind(&self) -> &str { self.override_kind.as_deref().unwrap_or_else(|| frame_for(&self.open)) }
    /// The person took the editor: the hub then refuses an agent's save while anything is unsaved (kicad/embed.rs `Unsaved`).
    fn touched(&self) { self.rpc("editor.kicad.touched", json!({"project": self.project})); }
    /// What a comment is made on: the project and the editor on show (PCB or SCH; the 3D view comments on its board), when one is.
    pub fn comment_target(&self) -> Option<(String, &'static str)> {
        self.want()?; let p = if self.bound.is_empty() { self.project.clone() } else { self.bound.clone() };
        Some((p, if self.wanted_kind() == "SCH" { "SCH" } else { "PCB" })) // i18n-ok: wire frame kinds
    }
    /// Backplane's window over the editor has the keyboard (`on`): the editor takes no pointer input meanwhile, so the keys typed with the pointer
    /// resting over it come to Backplane (native_child `input_off`).
    pub fn hold_keys(&mut self, on: bool) {
        let Some(c) = self.container else { return };
        nc::input_off(c, on);
        if on && self.keys { nc::focus(self.toplevel); self.keys = false; nc::grab_escape(self.toplevel, false); self.grab_click(c, true); }
    }
    /// The keyboard back to the editor on show (a comment posted or cancelled), as a click on it gives it.
    pub fn give_keys(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.container else { return };
        nc::input_off(c, false);
        let Some(w) = self.want() else { return };
        self.grab_click(c, false); nc::focus(w); self.keys = true; nc::grab_escape(self.toplevel, true); cx.notify();
    }
    fn want(&self) -> Option<u64> { if self.info.is_null() || (cfg!(windows) && self.info["mode"] == "stock") { return None } pick_local(self.wanted_kind(), &self.info, &self.frames, &self.ever_shown) }
    fn tick(&mut self, cx: &mut Context<Self>) {
        // the pre-warm's 2 s are up: a frame drawn now sends it (after_frame), even when nothing else redraws the window
        if let (Some(t), Some(pid)) = (self.first_placed, self.info["pid"].as_u64()) { if self.prewarmed != Some(pid) && t.elapsed() >= std::time::Duration::from_secs(2) { cx.notify(); } }
        let Some(c) = self.container else { return };
        if let Some(pid) = self.info["pid"].as_u64().filter(|p| *p > 0) {
            if self.info["mode"] == "fork" {
                nc::dialogs(pid as u32, c, self.toplevel);
                // what KiCad asks (a load-time notice, a question) comes up over its pane, not in the middle of the screen; and the hub is told
                // which are open (a modal one stops KiCad's API: no events, no pings), so the pane names it and KiCad is not called hung
                let over = match (&self.placed, self.last_root.0 != i32::MIN) { (Some((Some(at), _, _)), true) => nc::R { x: self.last_root.0, y: self.last_root.1, w: at.w, h: at.h }, _ => nc::R { x: 0, y: 0, w: 0, h: 0 } };
                let open = nc::centre_dialogs(pid as u32, over);
                if open != self.dialogs_open { self.dialogs_open = open; self.rpc("editor.kicad.dialogs", json!({"project": self.project, "open": self.dialogs_open})); cx.notify(); }
            } else if !cfg!(windows) && !nc::adopt(pid as u32, c, self.toplevel).is_empty() {
                self.placed = None; cx.notify();
            }
        }
        let f = nc::frames(c);
        if f != self.frames {
            // If stock KiCad on Linux just opened Symbol Editor, Footprint Editor, or 3D Viewer from inside an editor, show it in the pane.
            if !cfg!(windows) && self.info["mode"] == "stock" {
                for nf in &f {
                    if !self.frames.iter().any(|of| of.window == nf.window)
                        && (matches!(nf.kind, "SYMBOL_EDITOR" | "FOOTPRINT_EDITOR") || (nf.kind == "PCB_3D" && self.open != "3d")) // i18n-ok: wire frame kind enum names
                    {
                        self.override_kind = Some(nf.kind.into());
                    }
                }
                let sync: Vec<Value> = f.iter().map(|nf| json!({
                    "kind": nf.kind,
                    "title": nf.title,
                    "modified": nf.title.trim_start().starts_with('*'),
                    "xid": nf.window,
                })).collect();
                self.rpc("editor.kicad.frames", json!({"project": self.project, "frames": sync}));
            }
            if let Some(ov) = &self.override_kind {
                let was_open = self.frames.iter().any(|x| x.kind == ov.as_str());
                if was_open && !f.iter().any(|x| x.kind == ov.as_str()) && !self.info["frames"].as_array().is_some_and(|a| a.iter().any(|x| x["kind"].as_str() == Some(ov.as_str()))) {
                    self.override_kind = None;
                }
            }
            self.frames = f; self.placed = None; cx.notify();
        }
        if let Some((Some(at), Some(show), _)) = &self.placed {
            nc::sync_root(c, *at, *show, &mut self.last_root);
        }
        let want = self.wanted_kind().to_string();
        // fork KiCad: if the placed frame stayed unmapped (e.g. cold-start wxGTK decor-extents deferral), ask the hub to remount it once cooldown passes.
        if self.info["mode"] == "fork" && self.info["state"] == "ready" && self.pending_show.is_none() {
            if let Some((Some(_), Some(w), _)) = &self.placed {
                if nc::report(*w).is_some_and(|(_, on, _)| !on) {
                    let since = match self.unmapped_since {
                        Some((uw, t)) if uw == *w => t,
                        _ => { let now = std::time::Instant::now(); self.unmapped_since = Some((*w, now)); now }
                    };
                    if since.elapsed() >= std::time::Duration::from_millis(800)
                        && self.poked.as_ref().map_or(true, |(k, t)| k != &want || t.elapsed() >= std::time::Duration::from_millis(1500))
                    {
                        self.poked = Some((want.clone(), std::time::Instant::now()));
                        self.unmapped_since = None;
                        self.placed = None;
                        self.rpc("editor.kicad.show", json!({"project": self.project, "frame": &want, "remount": true, "focus": self.keys}));
                    }
                } else {
                    self.unmapped_since = None;
                }
            } else {
                self.unmapped_since = None;
            }
        } else {
            self.unmapped_since = None;
        }
        // stock KiCad (Linux only): the wanted editor not open yet, asked for with its own shortcut (at most every 4 s; the fork opens it through ShowFrame)
        if !cfg!(windows) && self.info["mode"] == "stock" && pick(&want, &self.info, &self.frames).is_none() && self.poked.as_ref().map_or(true, |(k, t)| k != &want || t.elapsed().as_secs() >= 4) {
            if let Some((host, ks, m)) = nc::stock_opener(&want) {
                // the 3D viewer is the PCB editor's: open that first
                let (host, ks, m) = if host == "PCB" && !self.frames.iter().any(|f| f.kind == "PCB") { nc::stock_opener("PCB").unwrap() } else { (host, ks, m) }; // i18n-ok: wire frame kind enum names
                if let Some(w) = self.frames.iter().find(|f| f.kind == host).map(|f| f.window) { nc::poke(w, ks, m); self.poked = Some((want, std::time::Instant::now())); }
            }
        }
        // a click over the editor gives it the keyboard; a click anywhere else in the client's window gives it back
        if let Some(w) = self.want() { match (nc::button_over(w), nc::has_focus(c)) { (Some(true), false) => nc::focus(w), (Some(false), true) => nc::focus(self.toplevel), _ => {} } }
        let mut has = nc::has_focus(c);
        // KiCad took the keyboard on a click (`_BACKPLANE_FOCUS`) and a window manager that focuses on click (i3) gave it straight back to
        // Backplane's window: the click was KiCad's, so is the keyboard (once, within moments of it)
        if !has && self.keys {
            if let Some((w, at)) = self.took.take() { if at.elapsed() < std::time::Duration::from_millis(600) { nc::focus(w); has = true; } }
        }
        if has != self.keys { self.keys = has; nc::grab_escape(self.toplevel, has); self.grab_click(c, !has); if has { self.touched(); } cx.notify(); }
        if let Some(h) = self.info["holder"].as_u64().filter(|h| *h > 0 && *h != self.kept) { nc::keep(h); self.kept = h; }
        for e in nc::events() {
            // what reached Backplane from the editor (the e2e reads it: one click in KiCad, then a reserved chord, must arrive)
            eprintln!("GPUI_LOG {}", json!({"kicad_ev": match &e { nc::Ev::Escape => "escape".to_string(), nc::Ev::Click => "click".to_string(), nc::Ev::Key(k) => format!("key {k}"), nc::Ev::Focus(w) => format!("focus {w}") }}));
            match e {
                // the keyboard back to the client's window; a reserved chord is then pressed in gpui (after_frame, which has the window)
                nc::Ev::Escape => { nc::focus(self.toplevel); self.keys = false; nc::grab_escape(self.toplevel, false); self.grab_click(c, true); cx.notify(); }
                nc::Ev::Click => { if let Some(w) = self.want() { self.grab_click(c, false); nc::focus(w); self.keys = true; self.touched(); nc::grab_escape(self.toplevel, true); cx.notify(); } }
                // Ctrl+Tab / Ctrl+Shift+Tab pressed in KiCad: the next editor tab, and KiCad keeps the keyboard
                nc::Ev::Key(k) if k == "ctrl-tab" || k == "ctrl-shift-tab" => { let to = next_tab(self.wanted_kind(), if k == "ctrl-tab" { 1 } else { -1 }); self.show_kind(to, cx); }
                nc::Ev::Key(k) => { eprintln!("GPUI_LOG {}", json!({"kicad_key": k})); nc::focus(self.toplevel); self.keys = false; nc::grab_escape(self.toplevel, false); self.grab_click(c, true); self.press.push(k); cx.notify(); }
                nc::Ev::Focus(_) => { if let Some(w) = self.want() { self.took = Some((w, std::time::Instant::now())); } if !self.keys { self.keys = true; self.touched(); nc::grab_escape(self.toplevel, true); self.grab_click(c, false); cx.notify(); } }
            }
        }
    }
    /// First render: the container in the client's window, and the hub asked to attach KiCad to it.
    fn decide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let top = top_of(window);
        // drawn in another window now (popped out into its own, or back): a new container there, KiCad's frames moved into it, the hub told
        if let (Some(_), Some(t)) = (self.container, top) {
            if t == self.toplevel { return }
            self.rehome(t);
            let weak = cx.entity().downgrade();
            window.on_frame_drawn(move |window, cx| weak.update(cx, |this, cx| this.after_frame(window, cx)).is_ok());
            return
        }
        if self.container.is_some() || self.attached { return }
        let Some(top) = top else { self.info = json!({"state": "failed", "reason": crate::i18n::trs("this window cannot hold another program's (only X11 and XWayland for now)")}); self.attached = true; return };
        self.toplevel = top; self.home = top;
        // the project's container kept from its last View (KiCad's frames still in it, attached): the editor is mapped in this very frame where
        // it was, with no round trip and no frame of the pane's background
        if let Some(k) = take_kept(top, &self.project) {
            self.container = Some(k.container); self.ever_shown = k.ever_shown; self.frames = nc::frames(k.container);
            if let (Some(r), Some(w)) = (k.rect, self.want()) { nc::place_local(k.container, Some(r), Some(w), &self.ever_shown, &mut self.last_root); self.placed = Some((Some(r), Some(w), Vec::new())); }
        } else {
            self.container = nc::container_for(top, &self.project);
        }
        let Some(c) = self.container else {
            let why = if cfg!(windows) { crate::i18n::trs("could not create container window") } else { crate::i18n::trs("no X11 connection") };
            self.info = json!({"state": "failed", "reason": why});
            self.attached = true;
            return;
        };
        self.grab_click(c, true);
        self.attached = true;
        let display = std::env::var("DISPLAY").unwrap_or_default();
        let kind = frame_for(&self.open);
        self.pending_show = Some(kind.to_string());
        self.rpc("editor.kicad.attach", json!({"project": self.project, "container": c, "toplevel": top, "display": display, "frame": kind, "wait": false, "theme": if self.light { "light" } else { "dark" }}));
        let weak = cx.entity().downgrade();
        window.on_frame_drawn(move |window, cx| weak.update(cx, |this, cx| this.after_frame(window, cx)).is_ok());
    }
    /// Re-send `editor.kicad.attach` after a WebSocket reconnect so any info transition missed while disconnected is recovered.
    pub fn reattach(&mut self) {
        let Some(c) = self.container else { return };
        let display = std::env::var("DISPLAY").unwrap_or_default();
        let kind = self.wanted_kind().to_string();
        self.pending_show = Some(kind.clone());
        self.rpc("editor.kicad.attach", json!({"project": self.project, "container": c, "toplevel": self.toplevel, "display": display, "frame": kind, "wait": false, "theme": if self.light { "light" } else { "dark" }}));
    }
    /// After every frame: the container over the pane (the marker's bounds) with the wanted editor mapped, holes where gpui drew over it.
    fn after_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // a window this pane left (popped out, or back in) places nothing
        if top_of(window) != Some(self.toplevel) { return }
        for k in std::mem::take(&mut self.press) { if let Ok(k) = Keystroke::parse(&k) { window.defer(cx, move |window, cx| { window.dispatch_keystroke(k, cx); }); } }
        let Some(c) = self.container else { return };
        let seen = self.marker.lock().unwrap().and_then(|id| window.hitbox_drawn_after(id));
        self.at_px = seen.as_ref().map(|(b, _)| *b);
        let scale = window.scale_factor();
        let r = |b: &Bounds<Pixels>| nc::R { x: (f32::from(b.origin.x) * scale).round() as i32, y: (f32::from(b.origin.y) * scale).round() as i32, w: (f32::from(b.size.width) * scale).round() as i32, h: (f32::from(b.size.height) * scale).round() as i32 };
        let at = seen.as_ref().map(|(b, _)| r(b)).filter(|r| r.w > 0 && r.h > 0);
        let mut over: Vec<nc::R> = seen.as_ref().map(|(_, o)| o.iter().filter(|b| f32::from(b.size.width) >= 12. && f32::from(b.size.height) >= 12.).map(|b| r(b)).collect()).unwrap_or_default();
        // Centered modals (keysheet, improvements) draw a full-window dim backdrop plus a smaller card: cut out the card so the uncovered pane stays visible.
        if let Some(p) = at {
            if over.iter().any(|o| *o != p) { over.retain(|o| *o != p); }
        }
        let show = self.want();
        // bp.9 local map: the frames shown once are the client's to map and unmap, the switch with no round trip (KiCad follows the map state)
        let local = self.info["mode"] == "fork" && self.info["local_map"] == true;
        let managed: Option<Vec<u64>> = if local { None } else { (self.info["mode"] == "fork").then(|| {
            self.info["frames"].as_array().into_iter().flatten()
                .filter(|f| f["shown"] != false)
                .filter_map(|f| f["xid"].as_u64())
                .collect()
        }) };
        let prev_show = self.placed.as_ref().and_then(|(_, s, _)| *s);
        let key = (at, show, over.clone());
        if self.placed.as_ref() == Some(&key) {
            if let (Some(at), Some(show)) = (at, show) { nc::sync_root(c, at, show, &mut self.last_root); }
            return;
        }
        // the first editor on show (bp.9+): the others pre-warmed at this pane's size, so their first switch is one paint with no relayout
        // (2 s after the first editor was placed: asked at once, KiCad's pre-warm sometimes hid the editor just shown, a rare race)
        if let (true, Some(p), Some(_), Some(pid)) = (local, at, show, self.info["pid"].as_u64()) {
            let since = *self.first_placed.get_or_insert_with(std::time::Instant::now);
            if self.prewarmed != Some(pid) && since.elapsed() >= std::time::Duration::from_secs(2) {
                self.prewarmed = Some(pid);
                let have: Vec<&str> = self.frames.iter().map(|f| f.kind).collect();
                self.rpc("editor.kicad.prewarm", json!({"project": self.project, "width": p.w, "height": p.h, "active": self.wanted_kind(), "have": have}));
            }
        }
        if local { nc::place_local(c, at, show, &self.ever_shown, &mut self.last_root); } else { nc::place(c, at, show, managed.as_deref(), &mut self.last_root); }
        if self.keys && show != prev_show {
            if let Some(w) = show { nc::focus(w); }
        }
        match at { Some(p) if !over.is_empty() => nc::holes(c, Some(&nc::cut(p, &over))), _ => nc::holes(c, None) }
        self.placed = Some(key);
    }
    /// Back to the viewer: the editor's windows leave the pane (KiCad stays warm in the hub).
    pub fn detach(&mut self) {
        let Some(c) = self.container.take() else { return };
        nc::place(c, None, None, None, &mut self.last_root); nc::grab_escape(self.toplevel, false);
        // kept, unmapped, with KiCad's frames in it (still attached in the hub): the next Edit of this project in this window maps it at once
        // (a fork that maps locally; the oldest kept beyond KEEP is let go as before)
        if self.info["mode"] == "fork" && self.info["local_map"] == true && self.info["state"] == "ready" && self.toplevel == self.home {
            let rect = self.placed.as_ref().and_then(|(at, _, _)| *at);
            for old in keep(Kept { toplevel: self.toplevel, project: self.project.clone(), container: c, ever_shown: self.ever_shown.clone(), rect }) {
                self.rpc("editor.kicad.detach", json!({"project": old.project})); nc::destroy(old.container);
            }
            self.rpc("editor.kicad.detach", json!({"project": self.project, "keep": true}));
            return;
        }
        self.rpc("editor.kicad.detach", json!({"project": self.project})); nc::destroy(c);
    }
    /// The `{"kicad":{...}}` script step's answer: the container and every frame (geometry, mapped, painted), the hub's state, who has the keys.
    pub fn report(&self) -> Value {
        let live = self.container.map_or_else(Vec::new, nc::frames);
        let frames_src = if !live.is_empty() { &live } else { &self.frames };
        let shown = (!self.info.is_null() && !(cfg!(windows) && self.info["mode"] == "stock"))
            .then(|| pick_local(self.wanted_kind(), &self.info, frames_src, &self.ever_shown)).flatten();
        let top_depth = if self.toplevel != 0 { nc::window_meta(self.toplevel, None, self.toplevel).map(|(d, _, _, _)| d) } else { None };
        let g = |w: u64| nc::report(w).map_or(Value::Null, |(r, on, lit)| {
            let mut v = json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h, "shown": on, "painted": lit});
            if let Some((depth, (rx, ry), rects, in_tree)) = nc::window_meta(w, self.container, self.toplevel) {
                v["depth"] = depth.into(); v["root_x"] = rx.into(); v["root_y"] = ry.into(); v["shape_rects"] = rects.into(); v["in_tree"] = in_tree.into();
            }
            v
        });
        let pane = self.placed.as_ref().and_then(|(at, _, _)| *at).map_or(Value::Null, |p| json!({"x": p.x, "y": p.y, "w": p.w, "h": p.h}));
        let shown_meta = shown.and_then(|w| nc::window_meta(w, self.container, self.toplevel));
        json!({"ok": true, "project": self.project, "root": self.root, "open": self.open, "want": self.wanted_kind(), "say": waiting(&self.seen_info(), shown.is_some()), "dialogs": self.seen_info()["dialogs"], "jobs": self.jobs, "local_map": self.info["local_map"], "ever_shown": self.ever_shown.len(), "conflicts": self.conflicts,
            "state": self.info["state"], "reason": self.info["reason"], "mode": self.info["mode"], "pid": self.info["pid"],
            "holder": self.info["holder"], "sock": self.info["sock"], "pane": pane,
            "container": self.container.map_or(Value::Null, |c| { let mut v = g(c); v["xid"] = c.into(); v }),
            "toplevel": self.toplevel, "toplevel_depth": top_depth, "keys": self.keys,
            "frames": frames_src.iter().map(|f| { let mut v = g(f.window); v["xid"] = f.window.into(); v["kind"] = f.kind.into(); v["title"] = f.title.clone().into(); v }).collect::<Vec<_>>(),
            "shown": shown,
            "shown_root": shown_meta.map_or(Value::Null, |(_, (rx, ry), _, _)| json!({"x": rx, "y": ry})),
            "in_gpui_tree": shown_meta.map_or(false, |(_, _, _, ok)| ok)})
    }
}
impl Drop for KicadPane { fn drop(&mut self) { self.detach(); } }

impl Render for KicadPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.decide(window, cx);
        let light = self.light; let c = |n: &str| crate::theme::color(light, n);
        let say = waiting(&self.seen_info(), self.want().is_some());
        let marker = self.marker.clone(); let focus = self.focus.clone();
        let keys = self.keys;
        div().id("kicad-pane").size_full().relative().bg(rgb(0x001023)).track_focus(&focus)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.took = None;
                if this.keys {
                    nc::focus(this.toplevel);
                    this.keys = false;
                    nc::grab_escape(this.toplevel, false);
                    if let Some(c) = this.container { this.grab_click(c, true); }
                    cx.notify();
                }
            }))
            .when(keys, |d| d.border_2().border_color(c("accent")))
            .child(canvas(move |b, window, _| { *marker.lock().unwrap() = Some(window.insert_hitbox(b, HitboxBehavior::Normal).id); }, |_, _, _, _| {}).size_full())
            .when_some(say, |d, s| d.child(div().absolute().inset_0().flex().items_center().justify_center().text_color(c("faint")).child(s)))
    }
}

#[cfg(test)]
mod tests {
    use super::{belongs_to, edit_target, frame_for, hub_open_step, nc, path_stem, pick, project_of, root_of, waiting, waiting_on};
    use serde_json::{json, Value};
    #[test] fn ctrl_tab_goes_round_the_editor_tabs() {
        assert_eq!(super::next_tab("PCB", 1), "SCH"); assert_eq!(super::next_tab("PCB", -1), "FOOTPRINT_EDITOR"); assert_eq!(super::next_tab("FOOTPRINT_EDITOR", 1), "PCB"); assert_eq!(super::next_tab("?", 1), "SCH");
    }
    #[test] fn rail_pages_name_editors() { assert_eq!(frame_for("board"), "PCB"); assert_eq!(frame_for("schematic"), "SCH"); assert_eq!(frame_for("3d"), "PCB_3D"); }
    #[test] fn the_project_comes_from_the_viewer_key() {
        assert_eq!(project_of("board|/p/proj|boards/a/a.kicad_pcb").as_deref(), Some("/p/proj/boards/a/a.kicad_pcb"));
        assert_eq!(project_of("schematic|/p/proj|/abs/x.kicad_sch").as_deref(), Some("/abs/x.kicad_sch"));
        assert_eq!(project_of("0|/p/|").as_deref(), Some("/p"), "the rail's default board: the project's root");
        assert_eq!(root_of("1|/p/proj/|sub/power.kicad_sch").as_deref(), Some("/p/proj"));
        assert_eq!(project_of(r"board|C:\bp\proj|amp.kicad_pcb").as_deref(), Some(r"C:\bp\proj\amp.kicad_pcb"));
        assert_eq!(project_of(r"schematic|C:\bp\proj|C:\bp\proj\amp.kicad_sch").as_deref(), Some(r"C:\bp\proj\amp.kicad_sch"));
        assert_eq!(root_of(r"0|C:\bp\proj\|").as_deref(), Some(r"C:\bp\proj"));
        assert_eq!(project_of("0||"), None); assert_eq!(project_of("nothing"), None);
        let data = json!({"projects": [{"id": "http://127.0.0.1:4100|p1", "root": r"C:\bp\proj\"}]});
        assert_eq!(edit_target(&data, "", "p1"), Some((r"C:\bp\proj".into(), r"C:\bp\proj".into())), "early rail press before viewer.key arrives falls back to project root");
        assert_eq!(edit_target(&data, r"0|C:\bp\proj|amp.kicad_pcb", "p1"), Some((r"C:\bp\proj\amp.kicad_pcb".into(), r"C:\bp\proj".into())));
        assert_eq!(edit_target(&data, "", "missing"), None);
    }
    #[test] fn dots_in_parent_folders_and_windows_separators_match_cleanly() {
        assert_eq!(path_stem("/tmp/bp-kembed-xr.lVLR/editors1/proj"), "/tmp/bp-kembed-xr.lVLR/editors1/proj");
        assert_eq!(path_stem("/tmp/bp-kembed-xr.lVLR/editors1/proj/amp.kicad_pro"), "/tmp/bp-kembed-xr.lVLR/editors1/proj/amp");
        assert_eq!(path_stem(r"C:\bp\run.1\proj\amp.kicad_pro"), r"C:\bp\run.1\proj\amp");
        assert!(!belongs_to("/tmp/bp-kembed-xr.lVLR/editors1/proj", "", "p1", "0|/tmp/bp-kembed-xr.lVLR/other|", "p2"));
        assert!(belongs_to(r"C:\bp\proj\amp.kicad_pcb", r"C:\bp\proj", "p1", "1|c:/bp/proj|c:/bp/proj/amp.kicad_sch", "p1"));
    }
    #[test] fn switching_rail_pages_in_same_project_keeps_the_editor_pane() {
        assert!(belongs_to("/p/amp/amp.kicad_pcb", "/p/amp", "p1", "1|/p/amp|/p/amp/amp.kicad_sch", "p1"), "board -> schematic");
        assert!(belongs_to("/p/amp/amp.kicad_pcb", "/p/amp", "p1", "1|/p/amp|/p/amp/power.kicad_sch", "p1"), "board -> subsheet");
        assert!(belongs_to("/p/amp/amp.kicad_pcb", "/p/amp", "p1", "", "p1"), "3d page before STEP/GLB exists");
        assert!(!belongs_to("/p/amp/amp.kicad_pcb", "/p/amp", "p1", "0|/p/other|/p/other/b.kicad_pcb", "p2"), "another project");
    }
    #[test] fn the_hubs_frames_win_over_titles_when_they_are_in_the_container() {
        let frames = vec![nc::Frame { window: 10, title: "a — PCB Editor".into(), kind: "PCB" }, nc::Frame { window: 11, title: "x".into(), kind: "MANAGER" }];
        assert_eq!(pick("PCB", &Value::Null, &frames), Some(10));
        assert_eq!(pick("PCB", &json!({"frames": [{"kind": "PCB", "xid": 11}]}), &frames), Some(11));
        assert_eq!(pick("PCB", &json!({"frames": [{"kind": "PCB", "xid": 99}]}), &frames), Some(10), "not (yet) in the container: by title");
        assert_eq!(pick("PCB", &json!({"mode": "fork", "frames": []}), &frames), None, "fork mode waits until ShowFrame registers xid");
        assert_eq!(pick("PCB", &json!({"mode": "fork", "frames": [{"kind": "PCB", "xid": 10}]}), &frames), Some(10));
        assert_eq!(pick("SCH", &Value::Null, &frames), None);
    }
    #[test] fn the_pane_says_what_it_waits_for() {
        assert!(waiting(&json!({"state": "starting"}), false).is_some());
        assert!(waiting(&json!({"state": "ready"}), true).is_none());
        assert!(waiting(&json!({"state": "failed", "reason": "no kicad"}), true).unwrap().contains("no kicad"));
        assert_eq!(
            waiting_on(&json!({"mode": "stock", "state": "ready"}), false, true).as_deref(),
            Some("The embedded editor needs Backplane KiCad."),
            "stock mode on Windows says Backplane KiCad is needed"
        );
        assert!(waiting_on(&json!({"mode": "stock", "state": "ready"}), true, false).is_none(), "stock mode on Linux embeds");
        let asking = waiting(&json!({"state": "ready", "dialogs": ["Load Schematic", "Information"]}), false).unwrap();
        assert!(asking.contains("Information") && !asking.contains("Starting"), "{asking}");
        assert!(waiting(&json!({"state": "ready", "dialogs": ["Information"]}), true).is_none(), "the editor on show says nothing over it");
    }
    #[test] fn stale_hub_open_never_reverts_a_rail_switch_while_svg_plots() {
        let mut last = "board".to_string();
        // Person clicked rail:Schematic (`cur_open` became "schematic"); hub's `viewer.open` is still "board" even after PRESSED expires.
        assert_eq!(hub_open_step("schematic", &mut last, "board", Some("schematic")), None);
        assert_eq!(hub_open_step("schematic", &mut last, "board", None), None, "stale board must not revert schematic");
        assert_eq!(last, "board");
        // Hub finishes plotting schematic and updates `viewer.open` -> "schematic": `last_hub_open` catches up with no duplicate switch.
        assert_eq!(hub_open_step("schematic", &mut last, "schematic", None), None);
        assert_eq!(last, "schematic");
        // Agent or remote client later changes `viewer.open` -> "3d": editor switches.
        assert_eq!(hub_open_step("schematic", &mut last, "3d", None), Some("3d"));
        assert_eq!(last, "3d");
    }
}

/// Whether this window can hold the thread's KiCad: the hub is on this machine (the editor runs beside it) and the thread is not another machine's.
pub fn local(hub_addr: Option<&str>, thread: &Value) -> bool {
    if thread["mark"]["name"].as_str().map_or(false, |m| !m.is_empty()) { return false }
    match hub_addr.map(|a| a.rsplit_once(':').map_or(a, |(h, _)| h)) { Some(h) => h.is_empty() || matches!(h, "127.0.0.1" | "localhost" | "[::1]"), None => true }
}

/// A pane's container after View, kept for its next Edit (detach, decide).
struct Kept { toplevel: u64, project: String, container: u64, ever_shown: Vec<u64>, rect: Option<nc::R> }
static KEPT: std::sync::Mutex<Vec<Kept>> = std::sync::Mutex::new(Vec::new());
const KEEP: usize = 3;
/// Keep `k` (replacing one of the same window and project); answers those let go to stay within KEEP.
fn keep(k: Kept) -> Vec<Kept> {
    let mut m = KEPT.lock().unwrap();
    let mut out: Vec<Kept> = Vec::new();
    if let Some(i) = m.iter().position(|x| x.toplevel == k.toplevel && x.project == k.project) { out.push(m.remove(i)); }
    m.push(k);
    while m.len() > KEEP { out.push(m.remove(0)); }
    out.retain(|o| !m.iter().any(|x| x.container == o.container));
    out
}
fn take_kept(toplevel: u64, project: &str) -> Option<Kept> {
    let mut m = KEPT.lock().unwrap();
    m.iter().position(|x| x.toplevel == toplevel && x.project == project).map(|i| m.remove(i))
}

/// The project last asked warm (kicad_warm), so each project shown in a row is asked once.
static WARM_LAST: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

impl crate::Backplane {
    /// The hub's `editor.kicad.<digest>` (JSON text; "" when the editor went away): kept, and given to the pane when it is this project's.
    pub(crate) fn kicad_info(&mut self, digest: String, text: &str, cx: &mut Context<Self>) {
        let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        if v.is_null() { self.screens.kicad_infos.remove(&digest); } else { self.screens.kicad_infos.insert(digest.clone(), v.clone()); }
        let (has_pane, matched, pane_proj) = match self.screens.kicad.clone() {
            Some(p) => {
                let (m, prj) = { let pr = p.read(cx); (pr.matches(&v), pr.project.clone()) };
                if m { p.update(cx, |p, cx| p.set_info(v.clone(), cx)); }
                (true, m, prj)
            }
            None => (false, false, String::new()),
        };
        eprintln!("GPUI_LOG {}", json!({"kicad_info": digest, "gone": text.is_empty(), "state": v["state"], "mode": v["mode"], "pid": v["pid"], "active": v["active"], "frames": v["frames"].as_array().map_or(0, Vec::len), "dialogs": v["dialogs"], "project": v["project"], "root": v["root"], "has_pane": has_pane, "matched": matched, "pane_project": pane_proj}));
    }
    /// Direct reply from an `editor.kicad.*` RPC: surface synchronous attach refusals on the pane and seed initial info if `KicadEditor` has not arrived yet.
    pub(crate) fn kicad_reply(&mut self, method: String, reply: Value, cx: &mut Context<Self>) {
        if reply["ok"] != true {
            let why = crate::hub::rpc_error(&reply);
            if method == "editor.kicad.attach" {
                if let Some(p) = self.screens.kicad.clone() {
                    p.update(cx, |p, cx| p.set_info(json!({"state": "failed", "reason": why}), cx));
                }
            }
            return;
        }
        if method == "editor.kicad.attach" && reply["state"].is_string() {
            let mut v = reply;
            if let Some(obj) = v.as_object_mut() { obj.remove("t"); obj.remove("id"); obj.remove("ok"); }
            if let Some(p) = self.screens.kicad.clone() {
                // the reply to this pane's own attach: it binds the pane to the project the hub resolved
                if let Some(pro) = v["project"].as_str().filter(|s| !s.is_empty()) { let pro = pro.to_string(); p.update(cx, |p, _| p.bound = pro); }
                let apply = {
                    let pr = p.read(cx);
                    pr.matches(&v) && (pr.info.is_null() || v["state"] == "ready")
                };
                if apply { p.update(cx, |p, cx| p.set_info(v, cx)); }
                // info that came before the reply (kept by digest) applies now
                let late: Vec<Value> = self.screens.kicad_infos.values().cloned().collect();
                for i in late { if p.read(cx).matches(&i) { p.update(cx, |p, cx| p.set_info(i, cx)); } }
            }
        }
    }
    /// A thread is shown: its project's KiCad kept warm in the hub (`editor.kicad.warm`; the hub decides whether, embed.rs `warm_decision`), so
    /// Edit draws the editor at once instead of starting KiCad. Asked once per project shown in a row; only for a hub on this machine. Linux
    /// for now (a fork started with no window to live in parks its frames; Windows not yet checked).
    pub(crate) fn kicad_warm(&mut self) {
        let thread = &self.data["thread"];
        if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("GPUI_LOG {}", json!({"kicad_pic": thread["id"], "viewer_open": thread["viewer"]["open"], "panel": self.data["panel"], "editing": self.screens.kicad.is_some()})); }
        if !cfg!(target_os = "linux") { return }
        if !local(self.screens.attacher.target.as_ref().map(|t| t.addr.as_str()), thread) { return }
        let tid = crate::screens::attach::raw_id(thread["id"].as_str().unwrap_or(""));
        if tid.is_empty() { return }
        let pid = crate::keys::project_of(&self.data, tid);
        let Some(root) = crate::screens::projects::find(&self.data, &pid).and_then(|p| p["root"].as_str()).map(|r| r.trim_end_matches(['/', '\\']).to_string()).filter(|r| !r.is_empty()) else { return };
        { let mut last = WARM_LAST.lock().unwrap(); if *last == root { return } *last = root.clone(); }
        let display = std::env::var("DISPLAY").unwrap_or_default();
        let _ = self.tx.try_send(Command::Rpc("editor.kicad.warm".into(), Secret(json!({"project": root, "display": display, "theme": if self.light() { "light" } else { "dark" }})), "kicad"));
    }
    pub(crate) fn kicad_rejoin(&mut self, cx: &mut Context<Self>) {
        // a new hub (a restart ends its KiCads): the shown project is asked warm again
        WARM_LAST.lock().unwrap().clear(); self.kicad_warm();
        if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, _| p.reattach()); }
    }
    /// Edit: the thread's design in the real KiCad, in the pane (the page stays on the rail's Board, Schematic or 3D).
    pub(crate) fn kicad_edit(&mut self, cx: &mut Context<Self>) -> bool {
        let thread = &self.data["thread"];
        let viewer = &thread["viewer"];
        let key = crate::screens::viewer::shown_key(viewer);
        let tid = crate::screens::attach::raw_id(thread["id"].as_str().unwrap_or(""));
        let pid = crate::keys::project_of(&self.data, tid);
        let Some((project, root)) = edit_target(&self.data, &key, &pid) else { return false };
        let open_now = viewer["open"].as_str().unwrap_or("");
        let open = crate::screens::viewer::pressed(open_now)
            .filter(|k| matches!(k.as_str(), "board" | "schematic" | "3d")) // i18n-ok: rail page kinds
            .or_else(|| (!open_now.is_empty()).then(|| open_now.to_string()))
            .unwrap_or_else(|| "board".to_string()); // i18n-ok: rail page kinds
        if let Some(p) = &self.screens.kicad {
            let pr = p.read(cx);
            if belongs_to(&pr.project, &pr.root, &pr.project_id, &key, &pid) { return true }
        }
        let (hub, infos, hub_open) = (self.tx.clone(), self.screens.kicad_infos.clone(), open_now.to_string());
        let page = cx.new(move |cx| { let mut p = KicadPane::new(cx, project, root, pid, open, hub_open, hub); if let Some(v) = infos.values().find(|v| p.matches(v)).cloned() { p.info = v; } p });
        self.screens.kicad = Some(page); cx.notify(); true
    }
    /// View: back to Backplane's own viewer (KiCad stays open in the hub, warm for the next Edit).
    pub(crate) fn kicad_view(&mut self, cx: &mut Context<Self>) { if let Some(p) = self.screens.kicad.take() { p.update(cx, |p, _| p.detach()); } cx.notify(); }
    /// The editor for the thread's design when the person is editing it (None: the viewer draws the page).
    pub(crate) fn kicad_for(&self, thread: &Value, cx: &App) -> Option<Entity<KicadPane>> {
        let p = self.screens.kicad.as_ref()?;
        let key = crate::screens::viewer::shown_key(&thread["viewer"]);
        let tid = crate::screens::attach::raw_id(thread["id"].as_str().unwrap_or(""));
        let pid = crate::keys::project_of(&self.data, tid);
        let pr = p.read(cx);
        belongs_to(&pr.project, &pr.root, &pr.project_id, &key, &pid).then(|| p.clone())
    }
}

/// The editor's own row: a tab per editor (one click; Ctrl+Tab and Ctrl+Shift+Tab go round them), Full size, and Pop out / Pop back in.
/// `on` (the window it is drawn in): the main window's pane, or the editor's own window.
pub fn strip<V: 'static>(cx: &mut Context<V>, light: bool, pane: &Entity<KicadPane>, popped: bool, full: bool,
    act: impl Fn(&mut V, &str, &mut Window, &mut Context<V>) + Clone + 'static) -> AnyElement {
    let c = |n: &str| crate::theme::color(light, n);
    let at = pane.read(cx).tab();
    let have: Vec<String> = pane.read(cx).frames.iter().map(|f| f.kind.to_string()).collect();
    let mut row = div().id("kicad-strip").flex().items_center().gap_1().px_2().h(px(30.)).flex_shrink_0().bg(c("panel")).border_b_1().border_color(c("rule"));
    for (kind, label) in TABS {
        // the symbol and footprint editors: a tab once KiCad has them, or while shown
        if matches!(*kind, "SYMBOL_EDITOR" | "FOOTPRINT_EDITOR") && !have.iter().any(|h| h == kind) && at != *kind { continue }
        let on = at == *kind; let k = kind.to_string(); let p = pane.clone();
        row = row.child(crate::ui::with_rect(div().id(SharedString::from(format!("kicad-tab-{kind}"))), { let k = kind.to_string(); move || format!("kicad-tab:{k}") }).px_2().py(px(3.)).text_size(px(12.)).cursor_pointer()
            .text_color(c(if on { "text" } else { "dim" })).when(on, |d| d.border_b_2().border_color(c("accent"))).hover(|s| s.bg(c("raise")))
            .tooltip(crate::keys::tip(&crate::i18n::trs(label), "Ctrl+Tab", light))
            .child(crate::i18n::trs(label))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| { let k = k.clone(); p.update(cx, |p, cx| p.show_kind(&k, cx)); }));
    }
    // a file changed on disk over the person's unsaved edits: said here too (KiCad asks in its own infobar), with the same two answers
    if !pane.read(cx).conflicts.is_empty() {
        let names: Vec<String> = pane.read(cx).conflicts.iter().map(|f| std::path::Path::new(f).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect();
        let (pk, pr) = (pane.clone(), pane.clone());
        row = row.child(div().id("kicad-reload").ml_2().flex().items_center().gap_1().text_size(px(12.)).text_color(c("bad"))
            .child(crate::i18n::tf("gpui.c.kicad.changed", &[("files", names.join(", ").into())]))
            .child(div().id("kicad-keep").px_2().border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().text_color(c("text")).child(crate::i18n::trs("Keep yours"))
                .on_click(move |_, _, cx| pk.update(cx, |p, cx| p.resolve("keep", cx))))
            .child(div().id("kicad-reload-btn").px_2().border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().text_color(c("text")).child(crate::i18n::trs("Reload (lose your edits)"))
                .on_click(move |_, _, cx| pr.update(cx, |p, cx| p.resolve("reload", cx)))));
    }
    // KiCad's long job (loading, zone fill, DRC, plot): a slim bar here instead of KiCad's own progress window (bp.12)
    let jobs = if pane.read(cx).info["state"] == "ready" { pane.read(cx).jobs.clone() } else { Vec::new() };
    if let Some(j) = jobs.first() {
        let (id, f) = (j["id"].as_u64().unwrap_or(0), j["fraction"].as_f64().unwrap_or(-1.0));
        let title = j["title"].as_str().unwrap_or("").to_string(); let detail = j["detail"].as_str().unwrap_or("").to_string();
        let text = if detail.is_empty() { crate::i18n::trs(&title) } else { format!("{} · {detail}", crate::i18n::trs(&title)) };
        let w = 120.0f32; let p = pane.clone();
        row = row.child(div().id("kicad-progress").ml_2().flex().items_center().gap_1().text_size(px(11.)).text_color(c("dim"))
            .child(div().max_w(px(220.)).overflow_hidden().whitespace_nowrap().text_ellipsis().child(text))
            .child(div().w(px(w)).h(px(4.)).bg(c("line")).child(div().h_full().bg(c("accent")).w(px(if f >= 0.0 { w * f.min(1.0) as f32 } else { w * 0.3 }))))
            .when(jobs.len() > 1, |d| d.child(format!("+{}", jobs.len() - 1)))
            .when(j["cancellable"] == true, |d| d.child(div().id("kicad-progress-cancel").px_1().cursor_pointer().child("×").tooltip(crate::keys::tip(&crate::i18n::trs("Cancel"), "", light))
                .on_click(move |_, _, cx| p.update(cx, |p, _| p.cancel_job(id))))));
    }
    // the design's checks ("DRC 12 · ERC 3", coloured only when not clean): a press opens the Checks panel
    if let Some((text, tone)) = crate::screens::mgmt_checks::chip_for(&pane.read(cx).project) {
        let a = act.clone();
        row = row.child(crate::screens::mgmt_checks::chip_el(&text, tone, light).on_click(cx.listener(move |v, _, w, cx| a(v, "checks", w, cx))));
    }
    row = row.child(div().flex_1());
    let mk = |id: &'static str, label: String, tip: String, a: &'static str, cx: &mut Context<V>| {
        let act = act.clone();
        div().id(id).px_2().py(px(3.)).text_size(px(12.)).border_1().border_color(c("line")).bg(c("raise")).cursor_pointer().hover(|s| s.bg(c("panel")))
            .tooltip(crate::keys::tip(&tip, "", light)).child(label)
            .on_click(cx.listener(move |this, _, window, cx| act(this, a, window, cx)))
    };
    if !popped { row = row.child(mk("kicad-full", crate::i18n::trs(if full { "Exit full size" } else { "Full size" }), crate::i18n::trs("The editor takes the sidebar's and the thread's room"), "full", cx)); }
    row = row.child(mk("kicad-pop", crate::i18n::trs(if popped { "Pop back in" } else { "Pop out" }), crate::i18n::trs(if popped { "Back into Backplane's pane, as it is" } else { "The editor in a window of its own, as it is" }), if popped { "popin" } else { "popout" }, cx));
    row.into_any_element()
}

/// The editor in a window of its own (Pop out): the strip and the same pane (the same KiCad, nothing restarted). Closing it pops it back in.
pub struct KicadWindow { pub pane: Entity<KicadPane>, pub app: WeakEntity<crate::Backplane>, pub light: bool }
impl Render for KicadWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // the pane's own idea of the theme (Backplane's render sets it; Backplane is never read from here, it may be the one updating)
        let light = self.pane.read(cx).light; self.light = light;
        let _ = window;
        let strip = strip(cx, light, &self.pane, true, false, |this: &mut KicadWindow, a, window, cx| {
            if a == "popin" { if let Some(app) = this.app.upgrade() { app.update(cx, |app, cx| app.kicad_popin(cx)); } window.remove_window(); }
        });
        div().size_full().flex().flex_col().bg(crate::theme::color(light, "bg")).child(strip).child(div().flex_1().min_h_0().child(self.pane.clone()))
    }
}

impl crate::Backplane {
    /// Pop out: the editor into a window of its own; the pane says where it went.
    pub(crate) fn kicad_popout(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = self.screens.kicad.clone() else { return };
        if self.screens.kicad_window.is_some() { return }
        let (app, light) = (cx.entity().downgrade(), self.light());
        let title = std::path::Path::new(&pane.read(cx).project).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let opts = WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1400.), px(900.)), cx))),
            titlebar: Some(TitlebarOptions { title: Some(format!("{title} — KiCad · Backplane").into()), ..Default::default() }), ..Default::default() };
        match cx.open_window(opts, move |window, cx| {
            let app2 = app.clone();
            window.on_window_should_close(cx, move |_, cx| { if let Some(a) = app2.upgrade() { a.update(cx, |a, cx| a.kicad_popin(cx)); } true });
            cx.new(|_| KicadWindow { pane, app, light })
        }) {
            Ok(h) => { self.screens.kicad_window = Some(h.into()); self.screens.kicad_full = false; }
            Err(e) => { self.note = format!("{} {e}", crate::i18n::trs("The editor's window could not open:")); }
        }
        cx.notify();
    }
    /// Pop back in: the pane draws the editor again (its window, if still open, is closed by its own button or its title bar).
    pub(crate) fn kicad_popin(&mut self, cx: &mut Context<Self>) {
        // KiCad back into the main window first: the editor's window is about to go, and its children with it
        if let Some(p) = self.screens.kicad.clone() { p.update(cx, |p, _| { let h = p.home(); if h != 0 { p.rehome(h); } }); }
        self.screens.kicad_window = None; cx.notify();
    }
    /// The strip's buttons in the main window.
    pub(crate) fn kicad_strip_act(&mut self, a: &str, cx: &mut Context<Self>) {
        match a { "full" => { self.screens.kicad_full = !self.screens.kicad_full; cx.notify(); } "popout" => self.kicad_popout(cx), "checks" => crate::screens::mgmt_checks::open_panel(self, cx), _ => {} }
    }
}

impl crate::Backplane {
    /// A comment from KiCad's own dialog: posted once (by its seq) into the open thread, when the window shows this project's editor.
    pub(crate) fn kicad_comment(&mut self, text: &str, cx: &mut Context<Self>) {
        let Ok(v) = serde_json::from_str::<Value>(text) else { return };
        let Some(p) = self.screens.kicad.clone() else { return };
        if !p.read(cx).matches(&v) { return }
        let key = format!("{}#{}", v["project"].as_str().unwrap_or(""), v["seq"]);
        if !self.screens.kicad_comments_seen.insert(key) { return }
        let th = self.data["thread"]["id"].as_str().unwrap_or("").rsplit('|').next().unwrap_or("").to_string();
        if th.is_empty() { self.note = crate::i18n::trs("A comment from KiCad needs an open thread."); cx.notify(); return }
        // the hub words it (kicad/comment.rs `text`), as it does for the comment window and a phone's viewer
        let mut p = v.clone(); p["thread"] = json!(th);
        let _ = self.tx.try_send(Command::Rpc("kicad.comment.post".into(), Secret(p), "kicad"));
        cx.notify();
    }
}

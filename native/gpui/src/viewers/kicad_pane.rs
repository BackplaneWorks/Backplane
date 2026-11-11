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
    /// the fork's frames KiCad has shown at least once (bp.9: the client may map and unmap them itself, so a tab switch is local)
    ever_shown: Vec<u64>,
    /// the other editors were asked pre-warmed (once per KiCad process: its pid)
    prewarmed: Option<u64>,
}

impl KicadPane {
    pub fn new(cx: &mut Context<Self>, project: String, root: String, project_id: String, open: String, hub_open: String, hub: tokio::sync::mpsc::Sender<Command>) -> Self {
        // the tick: adopt stock KiCad's new frames, read the container's frames, and give the editor the keyboard on a click over it
        cx.spawn(async move |weak, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(40)).await;
            if weak.update(cx, |this, cx| this.tick(cx)).is_err() { break }
        }).detach();
        KicadPane { bound: String::new(), project, root, project_id, open, last_hub_open: hub_open, hub, container: None, toplevel: 0, attached: false, info: Value::Null, frames: Vec::new(), marker: Arc::default(), last_root: (i32::MIN, 0), placed: None,
            focus: cx.focus_handle(), keys: false, light: false, _subs: Vec::new(), kept: 0, press: Vec::new(), poked: None, unmapped_since: None, override_kind: None, pending_show: None, theme_sent: false, home: 0, conflicts: Vec::new(), ever_shown: Vec::new(), prewarmed: None }
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
        else if self.attached {
            self.pending_show = (self.info["active"].as_str() != Some(kind)).then(|| kind.to_string());
            self.rpc("editor.kicad.show", json!({"project": self.project, "frame": kind, "focus": self.keys}));
        }
        cx.notify();
    }
    pub fn has_override(&self) -> bool { self.override_kind.is_some() }
    /// KiCad's frames moved into a new container in the window `t` (and the hub told), before the window they were in goes away: a window's
    /// destruction takes its children with it, and KiCad's would be among them. Also how a pane goes back in from its own window.
    pub fn rehome(&mut self, t: u64) {
        let Some(old) = self.container else { return };
        if t == self.toplevel { return }
        let Some(new) = nc::container_for(t, &self.project) else { return };
        nc::move_frames(old, new); nc::destroy(old);
        self.toplevel = t; self.container = Some(new); self.placed = None; self.unmapped_since = None; self.last_root = (i32::MIN, 0);
        nc::grab_click(new, !self.keys);
        self.reattach();
    }
    /// The hub's `kicad.reload.<digest>`: this project's waiting files, if it is this pane's.
    pub fn set_reload(&mut self, v: Value, cx: &mut Context<Self>) {
        if !self.matches(&v) { return }
        let c: Vec<String> = v["conflicts"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(String::from)).collect();
        if c != self.conflicts { self.conflicts = c; cx.notify(); }
    }
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
        else if self.attached { self.pending_show = Some(kind.to_string()); self.rpc("editor.kicad.show", json!({"project": self.project, "frame": kind})); }
        cx.notify();
    }
    pub fn tab(&self) -> String { self.wanted_kind().to_string() }
    /// Backplane's theme: KiCad follows it (the fork switches live; stock KiCad keeps its own).
    pub fn set_light(&mut self, light: bool, cx: &mut Context<Self>) {
        if self.light == light && self.theme_sent { return }
        self.light = light; self.theme_sent = true;
        if self.attached { self.rpc("editor.kicad.theme", json!({"project": self.project, "mode": if light { "light" } else { "dark" }})); }
        cx.notify();
    }
    fn wanted_kind(&self) -> &str { self.override_kind.as_deref().unwrap_or_else(|| frame_for(&self.open)) }
    /// The person took the editor: the hub then refuses an agent's save while anything is unsaved (kicad/embed.rs `Unsaved`).
    fn touched(&self) { self.rpc("editor.kicad.touched", json!({"project": self.project})); }
    fn want(&self) -> Option<u64> { if self.info.is_null() || (cfg!(windows) && self.info["mode"] == "stock") { return None } pick_local(self.wanted_kind(), &self.info, &self.frames, &self.ever_shown) }
    fn tick(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.container else { return };
        if let Some(pid) = self.info["pid"].as_u64().filter(|p| *p > 0) {
            if self.info["mode"] == "fork" {
                nc::dialogs(pid as u32, c, self.toplevel);
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
        let has = nc::has_focus(c);
        if has != self.keys { self.keys = has; nc::grab_escape(self.toplevel, has); nc::grab_click(c, !has); if has { self.touched(); } cx.notify(); }
        if let Some(h) = self.info["holder"].as_u64().filter(|h| *h > 0 && *h != self.kept) { nc::keep(h); self.kept = h; }
        for e in nc::events() {
            match e {
                // the keyboard back to the client's window; a reserved chord is then pressed in gpui (after_frame, which has the window)
                nc::Ev::Escape => { nc::focus(self.toplevel); self.keys = false; nc::grab_escape(self.toplevel, false); nc::grab_click(c, true); cx.notify(); }
                nc::Ev::Click => { if let Some(w) = self.want() { nc::grab_click(c, false); nc::focus(w); self.keys = true; self.touched(); nc::grab_escape(self.toplevel, true); cx.notify(); } }
                // Ctrl+Tab / Ctrl+Shift+Tab pressed in KiCad: the next editor tab, and KiCad keeps the keyboard
                nc::Ev::Key(k) if k == "ctrl-tab" || k == "ctrl-shift-tab" => { let to = next_tab(self.wanted_kind(), if k == "ctrl-tab" { 1 } else { -1 }); self.show_kind(to, cx); }
                nc::Ev::Key(k) => { nc::focus(self.toplevel); self.keys = false; nc::grab_escape(self.toplevel, false); nc::grab_click(c, true); self.press.push(k); cx.notify(); }
                nc::Ev::Focus(_) => { if !self.keys { self.keys = true; self.touched(); nc::grab_escape(self.toplevel, true); nc::grab_click(c, false); cx.notify(); } }
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
        self.toplevel = top; self.home = top; self.container = nc::container_for(top, &self.project);
        let Some(c) = self.container else {
            let why = if cfg!(windows) { crate::i18n::trs("could not create container window") } else { crate::i18n::trs("no X11 connection") };
            self.info = json!({"state": "failed", "reason": why});
            self.attached = true;
            return;
        };
        nc::grab_click(c, true);
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
        if let (true, Some(p), Some(_), Some(pid)) = (local, at, show, self.info["pid"].as_u64()) {
            if self.prewarmed != Some(pid) {
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
        if let Some(c) = self.container.take() { nc::place(c, None, None, None, &mut self.last_root); self.rpc("editor.kicad.detach", json!({"project": self.project})); nc::grab_escape(self.toplevel, false); nc::destroy(c); }
    }
    /// The `{"kicad":{...}}` script step's answer: the container and every frame (geometry, mapped, painted), the hub's state, who has the keys.
    pub fn report(&self) -> Value {
        let live = self.container.map_or_else(Vec::new, nc::frames);
        let frames_src = if !live.is_empty() { &live } else { &self.frames };
        let shown = (!self.info.is_null() && !(cfg!(windows) && self.info["mode"] == "stock"))
            .then(|| pick(self.wanted_kind(), &self.info, frames_src)).flatten();
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
        json!({"ok": true, "project": self.project, "root": self.root, "open": self.open, "want": self.wanted_kind(), "local_map": self.info["local_map"], "ever_shown": self.ever_shown.len(), "conflicts": self.conflicts,
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
        let say = waiting(&self.info, self.want().is_some());
        let marker = self.marker.clone(); let focus = self.focus.clone();
        let keys = self.keys;
        div().id("kicad-pane").size_full().relative().bg(rgb(0x001023)).track_focus(&focus)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if this.keys {
                    nc::focus(this.toplevel);
                    this.keys = false;
                    nc::grab_escape(this.toplevel, false);
                    if let Some(c) = this.container { nc::grab_click(c, true); }
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
    #[test] fn a_kicad_comment_reads_as_the_persons_message_with_its_references_and_picture() {
        let c = json!({"frame": "FK_PCB", "note": "Move U3 closer to J1", "names": {"references": ["U3"], "nets": ["GND"], "pins": []},
            "items": [{"kind": "FOOTPRINT", "reference": "U3", "layer": "F.Cu", "x_mm": 10.0, "y_mm": 20.5}], "image": "/h/kicad-comments/a-4.png"});
        let t = super::comment_text(&c);
        assert!(t.starts_with("Move U3 closer to J1\n\nSelected in the KiCad board: U3; nets GND"), "{t}");
        assert!(t.contains("- footprint U3 on F.Cu at (10.00, 20.50) mm") && t.ends_with("\n\n/h/kicad-comments/a-4.png"), "{t}");
        assert!(super::comment_text(&json!({"frame": "FK_SCH", "note": "here", "empty": true})).contains("nothing selected"));
    }
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
        eprintln!("GPUI_LOG {}", json!({"kicad_info": digest, "gone": text.is_empty(), "state": v["state"], "mode": v["mode"], "pid": v["pid"], "active": v["active"], "frames": v["frames"].as_array().map_or(0, Vec::len), "project": v["project"], "root": v["root"], "has_pane": has_pane, "matched": matched, "pane_project": pane_proj}));
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
    pub(crate) fn kicad_rejoin(&mut self, cx: &mut Context<Self>) {
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
        row = row.child(div().id(SharedString::from(format!("kicad-tab-{kind}"))).px_2().py(px(3.)).text_size(px(12.)).cursor_pointer()
            .text_color(c(if on { "text" } else { "dim" })).when(on, |d| d.border_b_2().border_color(c("accent"))).hover(|s| s.bg(c("raise")))
            .tooltip(crate::keys::tip(&crate::i18n::trs(label), "Ctrl+Tab", light))
            .child(crate::i18n::trs(label))
            .on_click(move |_, _, cx| { let k = k.clone(); p.update(cx, |p, cx| p.show_kind(&k, cx)); }));
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
        match a { "full" => { self.screens.kicad_full = !self.screens.kicad_full; cx.notify(); } "popout" => self.kicad_popout(cx), _ => {} }
    }
}

/// A "Comment to Backplane" as the person's message: their note, then what they had selected in KiCad (references, pins and nets, which the
/// thread draws as links that cross-probe; each item's layer or sheet and position in mm), then the screenshot (its path alone on a line: the
/// thread draws it). English: it is the person's message to an agent, as they wrote it.
pub fn comment_text(c: &Value) -> String {
    let list = |k: &str| c["names"][k].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect::<Vec<_>>();
    let (refs, nets, pins) = (list("references"), list("nets"), list("pins"));
    let ed = if c["frame"] == "FK_SCH" || c["frame"] == 2 { "schematic" } else { "board" }; // i18n-ok: the person's message to an agent
    let mut t = c["note"].as_str().unwrap_or("").trim().to_string();
    if c["empty"] == true { t.push_str(&format!("\n\n(KiCad {ed}, nothing selected: the area in view)")); } // i18n-ok: the person's message to an agent
    else {
        let mut sel = Vec::new();
        if !refs.is_empty() { sel.push(refs.join(", ")); }
        if !pins.is_empty() { sel.push(format!("pins {}", pins.join(", "))); } // i18n-ok: the person's message to an agent
        if !nets.is_empty() { sel.push(format!("nets {}", nets.join(", "))); } // i18n-ok: the person's message to an agent
        t.push_str(&format!("\n\nSelected in the KiCad {ed}: {}", sel.join("; "))); // i18n-ok: the person's message to an agent
        if let Some(sp) = c["sheet_path"].as_str().filter(|s| !s.is_empty() && *s != "/") { t.push_str(&format!(" (sheet {sp})")); } // i18n-ok
    }
    let items = c["items"].as_array().cloned().unwrap_or_default();
    for it in items.iter().take(12) {
        let name = [it["reference"].as_str(), it["pin"].as_str(), it["net"].as_str()].into_iter().flatten().find(|s| !s.is_empty()).unwrap_or("");
        let at = match (it["x_mm"].as_f64(), it["y_mm"].as_f64()) { (Some(x), Some(y)) => format!(" at ({x:.2}, {y:.2}) mm"), _ => String::new() }; // i18n-ok
        let place = it["layer"].as_str().or(it["sheet_path"].as_str()).filter(|s| !s.is_empty()).map(|l| format!(" on {l}")).unwrap_or_default(); // i18n-ok
        t.push_str(&format!("\n- {} {name}{place}{at}", it["kind"].as_str().unwrap_or("item").to_lowercase()));
    }
    if items.len() > 12 || c["truncated"] == true { t.push_str(&format!("\n- … and more ({} items listed of the selection)", items.len().min(12))); } // i18n-ok
    if let Some(img) = c["image"].as_str().filter(|s| !s.is_empty()) { t.push_str(&format!("\n\n{img}")); }
    t
}

impl crate::Backplane {
    /// A comment from KiCad: posted once (by its seq) into the open thread, when the window shows this project's editor.
    pub(crate) fn kicad_comment(&mut self, text: &str, cx: &mut Context<Self>) {
        let Ok(v) = serde_json::from_str::<Value>(text) else { return };
        let Some(p) = self.screens.kicad.clone() else { return };
        if !p.read(cx).matches(&v) { return }
        let key = format!("{}#{}", v["project"].as_str().unwrap_or(""), v["seq"]);
        if !self.screens.kicad_comments_seen.insert(key) { return }
        let th = self.data["thread"]["id"].as_str().unwrap_or("").rsplit('|').next().unwrap_or("").to_string();
        if th.is_empty() { self.note = crate::i18n::trs("A comment from KiCad needs an open thread."); cx.notify(); return }
        let _ = self.tx.try_send(Command::Ask("turn.start".into(), json!({"thread": th, "text": comment_text(&v), "mode": "steer"}), 0));
        cx.notify();
    }
}

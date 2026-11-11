//! The real KiCad editors in the viewer pane (docs/embedded-editors.md): the hub runs the project's GUI KiCad (kicad/embed.rs) and publishes
//! info `editor.kicad.<digest>`; this page makes a container window in the client's window (native_child.rs), tells the hub to attach KiCad to
//! it (`editor.kicad.attach`), and after every frame gpui draws puts the container over the pane with the rail's editor (Board: PCB,
//! Schematic: SCH, 3D: PCB_3D) mapped in it and the others unmapped, cutting holes where gpui drew something over the pane (the palette, a menu).
//! Stock KiCad makes its frames as top-levels: a tick adopts them into the container (the fork's `--embedded` makes them there).
//! The keyboard: a click in the editor gives it the X focus (the tick sees the button go down over it); a click anywhere in gpui, or Ctrl+`
//! pressed in the editor, gives it back.
use super::native_child as nc;
use crate::{hub::Command, screens::settings::Secret};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

/// The hub's frame kind for a rail page.
pub fn frame_for(open: &str) -> &'static str { match open { "schematic" => "SCH", "3d" => "PCB_3D", _ => "PCB" } } // i18n-ok: wire frame kind enum names

/// The project file the hub attaches to, from a viewer key `<kind>|<root>|<file>` (the file relative to the root, or absolute).
pub fn project_of(key: &str) -> Option<String> {
    let mut p = key.splitn(3, '|'); let (_, root, file) = (p.next()?, p.next()?, p.next()?);
    if file.is_empty() { return (!root.is_empty()).then(|| root.trim_end_matches('/').to_string()) }
    Some(if file.starts_with('/') || root.is_empty() { file.to_string() } else { format!("{}/{}", root.trim_end_matches('/'), file) })
}

/// The project root directory from a viewer key `<kind>|<root>|<file>`, trimmed of any trailing `/`.
pub fn root_of(key: &str) -> Option<String> {
    let mut p = key.splitn(3, '|'); let (_, root, _) = (p.next()?, p.next()?, p.next()?);
    let r = root.trim_end_matches('/');
    (!r.is_empty()).then(|| r.to_string())
}

/// Path without any file extension on its final segment (dots in parent folder names are preserved).
pub fn path_stem(s: &str) -> &str {
    let s = s.trim_end_matches('/');
    let base_at = s.rfind('/').map_or(0, |i| i + 1);
    match s[base_at..].rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => &s[..base_at + stem.len()],
        _ => s,
    }
}

/// Whether a pane opened for `(pane_proj, pane_root, pane_pid)` belongs to the viewer showing `(viewer_key, cur_pid)`.
pub fn belongs_to(pane_proj: &str, pane_root: &str, pane_pid: &str, viewer_key: &str, cur_pid: &str) -> bool {
    if let Some(vr) = root_of(viewer_key) {
        if !pane_root.is_empty() { return pane_root == vr }
    }
    if let Some(vp) = project_of(viewer_key) {
        if path_stem(pane_proj) == path_stem(&vp) || (!pane_root.is_empty() && vp.starts_with(&format!("{pane_root}/"))) { return true }
    }
    !pane_pid.is_empty() && pane_pid == cur_pid
}

/// The window to show for `want` among the container's frames, preferring the hub's own list (fork mode: kinds it knows) over titles.
/// In fork mode, wait until `info["frames"]` lists the window as shown so a half-opened frame parked in `holder` during `ShowFrame` is never mapped early.
pub fn pick(want: &str, info: &Value, frames: &[nc::Frame]) -> Option<u32> {
    if info["mode"] == "fork" {
        let hub = info["frames"].as_array().into_iter().flatten().find(|f| f["kind"] == want && f["shown"] != false).and_then(|f| f["xid"].as_u64()).map(|x| x as u32);
        return hub.filter(|x| frames.iter().any(|f| f.window == *x));
    }
    let hub = info["frames"].as_array().into_iter().flatten().find(|f| f["kind"] == want).and_then(|f| f["xid"].as_u64()).map(|x| x as u32);
    hub.filter(|x| frames.iter().any(|f| f.window == *x)).or_else(|| frames.iter().find(|f| f.kind == want).map(|f| f.window))
}

/// What the pane says when the editor is not on show, from the hub's state (None: the editor is there).
pub fn waiting(info: &Value, have_frame: bool) -> Option<String> {
    match info["state"].as_str().unwrap_or("") {
        "failed" => Some(format!("{} {}", crate::i18n::trs("KiCad could not start:"), info["reason"].as_str().unwrap_or(""))),
        "unresponsive" => Some(crate::i18n::trs("KiCad is not responding.")),
        "stopped" => Some(crate::i18n::trs("KiCad was closed.")),
        _ if have_frame => None,
        _ => Some(crate::i18n::trs("Starting KiCad…")),
    }
}

pub struct KicadPane {
    /// the .kicad_pro the hub resolved this pane's project to (its attach reply), "" until then
    pub bound: String,
    pub project: String, pub root: String, pub project_id: String, pub open: String, hub: tokio::sync::mpsc::Sender<Command>,
    container: Option<u32>, toplevel: u32, attached: bool, pub info: Value, frames: Vec<nc::Frame>,
    marker: Arc<Mutex<Option<HitboxId>>>, last_root: (i32, i32), placed: Option<(Option<nc::R>, Option<u32>, Vec<nc::R>)>,
    focus: FocusHandle, keys: bool, light: bool, _subs: Vec<Subscription>,
    /// the fork's holder already in our save-set; reserved chords pressed in KiCad, waiting to be pressed in gpui
    kept: u32, press: Vec<String>,
    /// stock KiCad: the editor last asked for with its shortcut, and when; auxiliary frame (Symbol/Footprint/3D) opened from inside KiCad
    poked: Option<(String, std::time::Instant)>, override_kind: Option<String>, pending_show: Option<String>,
}

impl KicadPane {
    pub fn new(cx: &mut Context<Self>, project: String, root: String, project_id: String, open: String, hub: tokio::sync::mpsc::Sender<Command>) -> Self {
        // the tick: adopt stock KiCad's new frames, read the container's frames, and give the editor the keyboard on a click over it
        cx.spawn(async move |weak, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(40)).await;
            if weak.update(cx, |this, cx| this.tick(cx)).is_err() { break }
        }).detach();
        KicadPane { bound: String::new(), project, root, project_id, open, hub, container: None, toplevel: 0, attached: false, info: Value::Null, frames: Vec::new(), marker: Arc::default(), last_root: (i32::MIN, 0), placed: None,
            focus: cx.focus_handle(), keys: false, light: false, _subs: Vec::new(), kept: 0, press: Vec::new(), poked: None, override_kind: None, pending_show: None }
    }
    fn rpc(&self, name: &str, params: Value) { let _ = self.hub.try_send(Command::Rpc(name.to_string(), Secret(params), "kicad")); }
    /// The hub's `editor.kicad.<digest>` for this project changed.
    pub fn set_info(&mut self, info: Value, cx: &mut Context<Self>) {
        if info == self.info { return }
        if info["pid"] != self.info["pid"] {
            let had_pid = !self.info["pid"].is_null();
            self.poked = None; self.kept = 0; self.placed = None; self.override_kind = None;
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
            self.placed = None;
        }
        self.info = info;
        cx.notify();
    }
    pub fn matches(&self, info: &Value) -> bool {
        let p = info["project"].as_str().unwrap_or("");
        // the hub's own name for this pane's project, from its attach reply: the .kicad_pro may sit below the folder the viewer named
        // (a repository whose board is in boards/<name>/), so the viewer's path alone did not match (Nov 11, SOT23_Card)
        if !self.bound.is_empty() && p == self.bound { return true }
        let root = info["root"].as_str().unwrap_or("").trim_end_matches('/');
        (!p.is_empty() && path_stem(p) == path_stem(&self.project))
            || (!root.is_empty() && (root == self.project.trim_end_matches('/') || (!self.root.is_empty() && root == self.root)))
    }
    /// The rail moved to another page: show its editor (the hub opens it when KiCad has not yet).
    pub fn set_open(&mut self, open: &str, cx: &mut Context<Self>) {
        if self.open == open && self.override_kind.is_none() { return }
        self.open = open.to_string();
        self.override_kind = None;
        let kind = frame_for(open);
        // KiCad was closed (or failed): the rail's page starts it again, instead of a "stopped" refusal from show
        if self.attached && matches!(self.info["state"].as_str(), Some("stopped" | "failed")) { self.reattach(); }
        else if self.attached {
            self.pending_show = Some(kind.to_string());
            self.rpc("editor.kicad.show", json!({"project": self.project, "frame": kind}));
        }
        cx.notify();
    }
    pub fn has_override(&self) -> bool { self.override_kind.is_some() }
    fn wanted_kind(&self) -> &str { self.override_kind.as_deref().unwrap_or_else(|| frame_for(&self.open)) }
    /// The person took the editor: the hub then refuses an agent's save while anything is unsaved (kicad/embed.rs `Unsaved`).
    fn touched(&self) { self.rpc("editor.kicad.touched", json!({"project": self.project})); }
    fn want(&self) -> Option<u32> { if self.info.is_null() { return None } pick(self.wanted_kind(), &self.info, &self.frames) }
    fn tick(&mut self, cx: &mut Context<Self>) {
        let Some(c) = self.container else { return };
        if let Some(pid) = self.info["pid"].as_u64().filter(|p| *p > 0) {
            if self.info["mode"] == "fork" {
                nc::dialogs(pid as u32, c, self.toplevel);
            } else if !nc::adopt(pid as u32, c, self.toplevel).is_empty() {
                self.placed = None; cx.notify();
            }
        }
        let f = nc::frames(c);
        if f != self.frames {
            // If stock KiCad just opened Symbol Editor, Footprint Editor, or 3D Viewer from inside an editor, show it in the pane.
            if self.info["mode"] == "stock" {
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
        if self.info["mode"] == "fork" && self.info["state"] == "ready" {
            if let Some((Some(_), Some(w), _)) = &self.placed {
                if nc::report(*w).is_some_and(|(_, on, _)| !on)
                    && self.poked.as_ref().map_or(true, |(k, t)| k != &want || t.elapsed() >= std::time::Duration::from_millis(1500))
                {
                    self.poked = Some((want.clone(), std::time::Instant::now()));
                    self.placed = None;
                    self.rpc("editor.kicad.show", json!({"project": self.project, "frame": &want, "remount": true}));
                }
            }
        }
        // stock KiCad: the wanted editor not open yet, asked for with its own shortcut (at most every 4 s; the fork opens it through ShowFrame)
        if self.info["mode"] == "stock" && pick(&want, &self.info, &self.frames).is_none() && self.poked.as_ref().map_or(true, |(k, t)| k != &want || t.elapsed().as_secs() >= 4) {
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
        if let Some(h) = self.info["holder"].as_u64().filter(|h| *h > 0 && *h as u32 != self.kept) { nc::keep(h as u32); self.kept = h as u32; }
        for e in nc::events() {
            match e {
                // the keyboard back to the client's window; a reserved chord is then pressed in gpui (after_frame, which has the window)
                nc::Ev::Escape => { nc::focus(self.toplevel); self.keys = false; nc::grab_escape(self.toplevel, false); nc::grab_click(c, true); cx.notify(); }
                nc::Ev::Click => { if let Some(w) = self.want() { nc::grab_click(c, false); nc::focus(w); self.keys = true; self.touched(); nc::grab_escape(self.toplevel, true); cx.notify(); } }
                nc::Ev::Key(k) => { nc::focus(self.toplevel); self.keys = false; nc::grab_escape(self.toplevel, false); nc::grab_click(c, true); self.press.push(k); cx.notify(); }
                nc::Ev::Focus(_) => { if !self.keys { self.keys = true; self.touched(); nc::grab_escape(self.toplevel, true); nc::grab_click(c, false); cx.notify(); } }
            }
        }
    }
    /// First render: the container in the client's window, and the hub asked to attach KiCad to it.
    fn decide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle as R};
        if self.container.is_some() || self.attached { return }
        let top = HasWindowHandle::window_handle(window).ok().map(|h| h.as_raw()).and_then(|h| match h { R::Xcb(x) => Some(x.window.get()), R::Xlib(x) => Some(x.window as u32), _ => None });
        let Some(top) = top else { self.info = json!({"state": "failed", "reason": crate::i18n::trs("this window cannot hold another program's (only X11 and XWayland for now)")}); self.attached = true; return };
        self.toplevel = top; self.container = nc::container(top);
        let Some(c) = self.container else { self.info = json!({"state": "failed", "reason": crate::i18n::trs("no X11 connection")}); self.attached = true; return };
        nc::grab_click(c, true);
        self.attached = true;
        let display = std::env::var("DISPLAY").unwrap_or_default();
        let kind = frame_for(&self.open);
        self.pending_show = Some(kind.to_string());
        self.rpc("editor.kicad.attach", json!({"project": self.project, "container": c, "toplevel": top, "display": display, "frame": kind, "wait": false}));
        let weak = cx.entity().downgrade();
        window.on_frame_drawn(move |window, cx| weak.update(cx, |this, cx| this.after_frame(window, cx)).is_ok());
    }
    /// Re-send `editor.kicad.attach` after a WebSocket reconnect so any info transition missed while disconnected is recovered.
    pub fn reattach(&mut self) {
        let Some(c) = self.container else { return };
        let display = std::env::var("DISPLAY").unwrap_or_default();
        let kind = self.wanted_kind().to_string();
        self.pending_show = Some(kind.clone());
        self.rpc("editor.kicad.attach", json!({"project": self.project, "container": c, "toplevel": self.toplevel, "display": display, "frame": kind, "wait": false}));
    }
    /// After every frame: the container over the pane (the marker's bounds) with the wanted editor mapped, holes where gpui drew over it.
    fn after_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let managed: Option<Vec<u32>> = (self.info["mode"] == "fork").then(|| {
            self.info["frames"].as_array().into_iter().flatten()
                .filter(|f| f["shown"] != false)
                .filter_map(|f| f["xid"].as_u64().map(|x| x as u32))
                .collect()
        });
        let prev_show = self.placed.as_ref().and_then(|(_, s, _)| *s);
        let key = (at, show, over.clone());
        if self.placed.as_ref() == Some(&key) {
            if let (Some(at), Some(show)) = (at, show) { nc::sync_root(c, at, show, &mut self.last_root); }
            return;
        }
        nc::place(c, at, show, managed.as_deref(), &mut self.last_root);
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
        let shown = (!self.info.is_null()).then(|| pick(self.wanted_kind(), &self.info, frames_src)).flatten();
        let top_depth = if self.toplevel != 0 { nc::window_meta(self.toplevel, None, self.toplevel).map(|(d, _, _, _)| d) } else { None };
        let g = |w: u32| nc::report(w).map_or(Value::Null, |(r, on, lit)| {
            let mut v = json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h, "shown": on, "painted": lit});
            if let Some((depth, (rx, ry), rects, in_tree)) = nc::window_meta(w, self.container, self.toplevel) {
                v["depth"] = depth.into(); v["root_x"] = rx.into(); v["root_y"] = ry.into(); v["shape_rects"] = rects.into(); v["in_tree"] = in_tree.into();
            }
            v
        });
        let pane = self.placed.as_ref().and_then(|(at, _, _)| *at).map_or(Value::Null, |p| json!({"x": p.x, "y": p.y, "w": p.w, "h": p.h}));
        let shown_meta = shown.and_then(|w| nc::window_meta(w, self.container, self.toplevel));
        json!({"ok": true, "project": self.project, "root": self.root, "open": self.open, "want": self.wanted_kind(),
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
    use super::{belongs_to, frame_for, nc, path_stem, pick, project_of, root_of, waiting};
    use serde_json::{json, Value};
    #[test] fn rail_pages_name_editors() { assert_eq!(frame_for("board"), "PCB"); assert_eq!(frame_for("schematic"), "SCH"); assert_eq!(frame_for("3d"), "PCB_3D"); }
    #[test] fn the_project_comes_from_the_viewer_key() {
        assert_eq!(project_of("board|/p/proj|boards/a/a.kicad_pcb").as_deref(), Some("/p/proj/boards/a/a.kicad_pcb"));
        assert_eq!(project_of("schematic|/p/proj|/abs/x.kicad_sch").as_deref(), Some("/abs/x.kicad_sch"));
        assert_eq!(project_of("0|/p/|").as_deref(), Some("/p"), "the rail's default board: the project's root");
        assert_eq!(root_of("1|/p/proj/|sub/power.kicad_sch").as_deref(), Some("/p/proj"));
        assert_eq!(project_of("0||"), None); assert_eq!(project_of("nothing"), None);
    }
    #[test] fn dots_in_parent_folders_do_not_confuse_project_stem_matching() {
        assert_eq!(path_stem("/tmp/bp-kembed-xr.lVLR/editors1/proj"), "/tmp/bp-kembed-xr.lVLR/editors1/proj");
        assert_eq!(path_stem("/tmp/bp-kembed-xr.lVLR/editors1/proj/amp.kicad_pro"), "/tmp/bp-kembed-xr.lVLR/editors1/proj/amp");
        assert!(!belongs_to("/tmp/bp-kembed-xr.lVLR/editors1/proj", "", "p1", "0|/tmp/bp-kembed-xr.lVLR/other|", "p2"));
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
        let Some(project) = project_of(&key) else { return false };
        let root = root_of(&key).unwrap_or_default();
        let open = viewer["open"].as_str().unwrap_or("board").to_string();
        if let Some(p) = &self.screens.kicad {
            let pr = p.read(cx);
            if belongs_to(&pr.project, &pr.root, &pr.project_id, &key, &pid) { return true }
        }
        let (hub, infos) = (self.tx.clone(), self.screens.kicad_infos.clone());
        let page = cx.new(move |cx| { let mut p = KicadPane::new(cx, project, root, pid, open, hub); if let Some(v) = infos.values().find(|v| p.matches(v)).cloned() { p.info = v; } p });
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

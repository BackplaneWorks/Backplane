//! Another program's windows inside the client's (the embedded KiCad editors, docs/embedded-editors.md): a container window of ours in the client's
//! window, the program's frames made its children, one of them mapped over the pane and the rest unmapped. Unlike the Browser's Chromium
//! (viewers/cef_native.rs, the same process), these windows belong to another process, so everything here goes through the X server.
//! - The container has the screen's default (24-bit) visual: gpui's window has a 32-bit one, and GL canvases made under it are refused.
//! - Stock KiCad makes its frames as top-levels: `adopt` withdraws them from the window manager and reparents them into the container. The
//!   fork's `--embedded` makes them in the container from the start, so `adopt` finds nothing to do.
//! - The frames are never told they moved when only the client's window moves (they are not top-levels any more), so `place` sends them a
//!   synthetic ConfigureNotify with their root position (ICCCM 4.2.3); otherwise their menus and popups open where the window used to be.
//! - Overlays (the palette, menus) are drawn by gpui under the container; `holes` cuts them out of the container's shape instead of hiding it.
//! Linux (X11, and Wayland through XWayland) only for now; elsewhere every call is a no-op and `container` is None (the pane then says so).
#![allow(dead_code)]

/// A rectangle in the client window's physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct R { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }

/// The kind of an editor frame from its title, for stock KiCad (the fork reports kinds itself), named as the hub and the fork's embed IPC name
/// them: `PCB`, `SCH`, `PCB_3D`, `SYMBOL_EDITOR`, `FOOTPRINT_EDITOR`, or `MANAGER` (the project manager, hosting the editors; never shown).
pub fn kind_of_title(title: &str) -> &'static str {
    let t = title.trim_start_matches('*');
    let last = t.rsplit(['—', '-']).next().unwrap_or(t).trim();
    match last {
        "PCB Editor" => "PCB", // i18n-ok: stock KiCad frame titles and wire frame kinds
        "Schematic Editor" => "SCH", // i18n-ok: stock KiCad frame titles and wire frame kinds
        "3D Viewer" => "PCB_3D", // i18n-ok: stock KiCad frame titles and wire frame kinds
        "Symbol Editor" => "SYMBOL_EDITOR", // i18n-ok: stock KiCad frame titles and wire frame kinds
        "Footprint Editor" => "FOOTPRINT_EDITOR", // i18n-ok: stock KiCad frame titles and wire frame kinds
        _ if t.starts_with("3D Viewer") => "PCB_3D", // i18n-ok: stock KiCad frame titles and wire frame kinds
        _ => "MANAGER", // i18n-ok: wire frame kind
    }
}

/// How stock KiCad is asked for an editor it has not opened: (the kind of frame to press it in, keysym, X modifier mask). KiCad 10.0's own
/// shortcuts: Ctrl+P (PCB), Ctrl+E (schematic), Ctrl+L (symbols), Ctrl+F (footprints) in the project manager; Alt+3 (3D) in the PCB editor.
pub fn stock_opener(want: &str) -> Option<(&'static str, u32, u16)> {
    match want { "PCB" => Some(("MANAGER", 0x70, 4)), "SCH" => Some(("MANAGER", 0x65, 4)), "SYMBOL_EDITOR" => Some(("MANAGER", 0x6c, 4)), "FOOTPRINT_EDITOR" => Some(("MANAGER", 0x66, 4)), "PCB_3D" => Some(("PCB", 0x33, 8)), _ => None } // i18n-ok: wire frame kinds
}

/// Whether a stock KiCad title says its document has unsaved changes (KiCad 10.0 has no GetDocumentModifiedState).
pub fn dirty_title(title: &str) -> bool { title.starts_with('*') }

/// The rectangles of `pane` left when `over` (in the same coordinates) are cut out of it, relative to the pane's top-left: at most four per
/// overlay, slicing top and bottom bands first. Overlays outside the pane cut nothing.
pub fn cut(pane: R, over: &[R]) -> Vec<R> {
    let mut keep = vec![R { x: 0, y: 0, w: pane.w, h: pane.h }];
    for o in over {
        let o = R { x: o.x - pane.x, y: o.y - pane.y, w: o.w, h: o.h };
        let mut next = Vec::new();
        for k in keep {
            let (x0, y0, x1, y1) = (o.x.max(k.x), o.y.max(k.y), (o.x + o.w).min(k.x + k.w), (o.y + o.h).min(k.y + k.h));
            if x0 >= x1 || y0 >= y1 { next.push(k); continue }
            if y0 > k.y { next.push(R { x: k.x, y: k.y, w: k.w, h: y0 - k.y }); }
            if y1 < k.y + k.h { next.push(R { x: k.x, y: y1, w: k.w, h: k.y + k.h - y1 }); }
            if x0 > k.x { next.push(R { x: k.x, y: y0, w: x0 - k.x, h: y1 - y0 }); }
            if x1 < k.x + k.w { next.push(R { x: x1, y: y0, w: k.x + k.w - x1, h: y1 - y0 }); }
        }
        keep = next;
    }
    keep
}

/// What the editor's side sent the client (`events`).
#[derive(Clone, Debug, PartialEq)]
pub enum Ev { /** Ctrl+`: the keyboard back to the client */ Escape, /** a button press over the unfocused container (replayed to KiCad) */ Click, /** a reserved chord, as gpui names a keystroke */ Key(String), /** KiCad took the keyboard on a click into this frame */ Focus(u32) }

/// A reserved chord from `_BACKPLANE_KEY` (an X keysym and modifier mask) as gpui's keystroke text: `ctrl-k`, `ctrl-shift-tab`, `ctrl-``.
pub fn chord(keysym: u32, mods: u32) -> Option<String> {
    let key = match keysym { 0x20..=0x7e => char::from_u32(keysym)?.to_ascii_lowercase().to_string(), 0xff09 => "tab".into(), 0xff0d => "enter".into(), 0xff1b => "escape".into(), _ => return None };
    let mut s = String::new();
    if mods & 4 != 0 { s.push_str("ctrl-"); } if mods & 8 != 0 { s.push_str("alt-"); } if mods & 0x40 != 0 { s.push_str("super-"); } if mods & 1 != 0 { s.push_str("shift-"); }
    Some(s + &key)
}

/// A frame the container holds: its window, its title as the program last set it, and its kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame { pub window: u32, pub title: String, pub kind: &'static str }

#[cfg(target_os = "linux")]
mod imp {
    use super::{Frame, R};
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use x11rb::{connection::Connection, protocol::{shape::{self, ConnectionExt as _}, xproto::*}, rust_connection::RustConnection, wrapper::ConnectionExt as _};

    static SCREEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    /// One connection of our own (gpui's is not ours to use).
    fn conn() -> Option<MutexGuard<'static, Option<RustConnection>>> {
        static C: OnceLock<Mutex<Option<RustConnection>>> = OnceLock::new();
        let g = C.get_or_init(|| Mutex::new(match RustConnection::connect(None) {
            Ok((c, n)) => { SCREEN.store(n, std::sync::atomic::Ordering::Relaxed); Some(c) }
            Err(e) => { eprintln!("GPUI_LOG {{\"kicad_x11\":{}}}", serde_json::Value::String(e.to_string())); None }
        })).lock().ok()?;
        g.is_some().then_some(g)
    }
    fn root(c: &RustConnection) -> Option<Window> { c.setup().roots.get(SCREEN.load(std::sync::atomic::Ordering::Relaxed)).map(|s| s.root) }
    fn atom(c: &RustConnection, n: &str) -> u32 { c.intern_atom(false, n.as_bytes()).ok().and_then(|r| r.reply().ok()).map_or(0, |r| r.atom) }
    fn prop32(c: &RustConnection, w: Window, p: u32, t: impl Into<u32>) -> Vec<u32> {
        c.get_property(false, w, p, t.into(), 0, 64).ok().and_then(|r| r.reply().ok()).and_then(|r| r.value32().map(|v| v.collect())).unwrap_or_default()
    }
    fn title(c: &RustConnection, w: Window) -> String {
        let (name, utf8) = (atom(c, "_NET_WM_NAME"), atom(c, "UTF8_STRING")); // i18n-ok: X11 atom names
        let get = |p: u32, t: u32| c.get_property(false, w, p, t, 0, 1024).ok().and_then(|r| r.reply().ok()).map(|r| String::from_utf8_lossy(&r.value).into_owned()).filter(|s| !s.is_empty());
        get(name, utf8).or_else(|| get(u32::from(AtomEnum::WM_NAME), u32::from(AtomEnum::STRING))).unwrap_or_default()
    }

    /// A window of ours in the client's window `parent`, with the screen's default visual, unmapped until `place` maps it.
    pub fn container(parent: u32) -> Option<u32> {
        let g = conn()?; let c = g.as_ref().unwrap();
        let scr = c.setup().roots.get(SCREEN.load(std::sync::atomic::Ordering::Relaxed))?;
        let id = c.generate_id().ok()?;
        c.create_window(scr.root_depth, id, parent, 0, 0, 1, 1, 0, WindowClass::INPUT_OUTPUT, scr.root_visual,
            &CreateWindowAux::new().background_pixel(scr.black_pixel).border_pixel(scr.black_pixel).colormap(scr.default_colormap)
                .event_mask(EventMask::SUBSTRUCTURE_NOTIFY)).ok()?.check().ok()?;
        let _ = c.flush(); Some(id)
    }
    pub fn destroy(container: u32) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let _ = c.allow_events(Allow::ASYNC_POINTER, x11rb::CURRENT_TIME);
        let _ = c.ungrab_button(ButtonIndex::ANY, container, ModMask::ANY);
        if let Some(r) = root(c) {
            let kids = c.query_tree(container).ok().and_then(|q| q.reply().ok()).map(|t| t.children).unwrap_or_default();
            for k in kids {
                let _ = c.unmap_window(k);
                let _ = c.reparent_window(k, r, 0, 0);
            }
        }
        let _ = c.destroy_window(container); let _ = c.flush();
    }

    /// The program `pid`'s top-level frames, wherever the window manager put them (directly under the root, or in its frame window one level
    /// down): viewable, not override-redirect, no WM_TRANSIENT_FOR (dialogs stay floating), at least 100x100.
    fn top_levels(c: &RustConnection, pid: u32) -> Vec<Window> {
        let Some(root) = root(c) else { return vec![] };
        let pid_atom = atom(c, "_NET_WM_PID");
        let kids = |w: Window| c.query_tree(w).ok().and_then(|r| r.reply().ok()).map(|t| t.children).unwrap_or_default();
        let mut out = Vec::new();
        for a in kids(root) {
            for w in std::iter::once(a).chain(kids(a)) {
                if prop32(c, w, pid_atom, AtomEnum::CARDINAL).first() != Some(&pid) { continue }
                let Some(at) = c.get_window_attributes(w).ok().and_then(|r| r.reply().ok()) else { continue };
                if at.override_redirect || at.map_state != MapState::VIEWABLE { continue }
                if !prop32(c, w, u32::from(AtomEnum::WM_TRANSIENT_FOR), AtomEnum::WINDOW).is_empty() { continue }
                let Some(geo) = c.get_geometry(w).ok().and_then(|r| r.reply().ok()) else { continue };
                if geo.width < 100 || geo.height < 100 { continue }
                out.push(w);
            }
        }
        out
    }

    /// The program `pid`'s parked unmapped frames directly on `root` (left there by `destroy` on View or by the XFixes save-set on a client crash).
    fn parked_on_root(c: &RustConnection, pid: u32) -> Vec<Window> {
        let Some(root) = root(c) else { return vec![] };
        let pid_atom = atom(c, "_NET_WM_PID"); // i18n-ok: X11 atom name
        let kids = c.query_tree(root).ok().and_then(|r| r.reply().ok()).map(|t| t.children).unwrap_or_default();
        let mut out = Vec::new();
        for w in kids {
            if prop32(c, w, pid_atom, AtomEnum::CARDINAL).first() != Some(&pid) { continue }
            let Some(at) = c.get_window_attributes(w).ok().and_then(|r| r.reply().ok()) else { continue };
            if at.override_redirect || at.map_state != MapState::UNMAPPED { continue }
            if !prop32(c, w, u32::from(AtomEnum::WM_TRANSIENT_FOR), AtomEnum::WINDOW).is_empty() { continue }
            let Some(geo) = c.get_geometry(w).ok().and_then(|r| r.reply().ok()) else { continue };
            if geo.width < 100 || geo.height < 100 { continue }
            out.push(w);
        }
        out
    }

    /// Stock KiCad, in two steps a tick apart: every new top-level frame of `pid` is first withdrawn from the window manager (unmapped, and a
    /// synthetic UnmapNotify to the root as ICCCM 4.1.4 asks, so a tiling one gives its space back); once the window manager has let go of it
    /// (at least 150 ms later: it puts an unmanaged window back on the root, so reparenting at once lost the race to i3), it is reparented,
    /// unmapped, into `container` with its minimum size cleared. Answers the windows reparented now.
    pub fn adopt(pid: u32, container: u32, toplevel: u32) -> Vec<u32> {
        use x11rb::protocol::xfixes::{self, ConnectionExt as _};
        static WITHDRAWN: Mutex<Vec<(u32, std::time::Instant)>> = Mutex::new(Vec::new());
        let Some(g) = conn() else { return vec![] }; let c = g.as_ref().unwrap();
        let Some(root) = root(c) else { return vec![] };
        dialogs_over(c, pid, container, toplevel);
        let found = top_levels(c, pid);
        if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() && !found.is_empty() { eprintln!("GPUI_LOG {{\"kicad_adopt\":{{\"pid\":{pid},\"found\":{found:?}}}}}"); }
        let mut wd = WITHDRAWN.lock().unwrap();
        for w in found {
            if wd.iter().any(|(x, _)| *x == w) { continue }
            let _ = c.unmap_window(w);
            let ev = UnmapNotifyEvent { response_type: UNMAP_NOTIFY_EVENT, sequence: 0, event: root, window: w, from_configure: false };
            let _ = c.send_event(false, root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, ev);
            wd.push((w, std::time::Instant::now()));
        }
        let mut took = Vec::new();
        let _ = c.xfixes_query_version(5, 0).ok().and_then(|r| r.reply().ok());
        for w in parked_on_root(c, pid) {
            if wd.iter().any(|(x, _)| *x == w) { continue }
            let _ = c.change_property32(PropMode::REPLACE, w, AtomEnum::WM_NORMAL_HINTS, AtomEnum::WM_SIZE_HINTS, &[0u32; 18]);
            let _ = c.reparent_window(w, container, 0, 0);
            let _ = c.xfixes_change_save_set(xfixes::SaveSetMode::INSERT, xfixes::SaveSetTarget::ROOT, xfixes::SaveSetMapping::UNMAP, w);
            let _ = c.change_window_attributes(w, &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE | EventMask::STRUCTURE_NOTIFY));
            took.push(w);
        }
        wd.retain_mut(|(w, at)| {
            if at.elapsed() < std::time::Duration::from_millis(150) { return true }
            let Some(t) = c.query_tree(*w).ok().and_then(|r| r.reply().ok()) else { return false }; // gone
            if t.parent == container { return false }
            let viewable = c.get_window_attributes(*w).ok().and_then(|r| r.reply().ok()).map_or(false, |a| a.map_state == MapState::VIEWABLE);
            if viewable { // KiCad showed it again (it raises frames while opening a project): withdrawn again, and another wait
                let _ = c.unmap_window(*w);
                let ev = UnmapNotifyEvent { response_type: UNMAP_NOTIFY_EVENT, sequence: 0, event: root, window: *w, from_configure: false };
                let _ = c.send_event(false, root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, ev);
                *at = std::time::Instant::now(); return true
            }
            let _ = c.change_property32(PropMode::REPLACE, *w, AtomEnum::WM_NORMAL_HINTS, AtomEnum::WM_SIZE_HINTS, &[0u32; 18]);
            let _ = c.reparent_window(*w, container, 0, 0);
            let _ = c.xfixes_change_save_set(xfixes::SaveSetMode::INSERT, xfixes::SaveSetTarget::ROOT, xfixes::SaveSetMapping::UNMAP, *w);
            let _ = c.change_window_attributes(*w, &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE | EventMask::STRUCTURE_NOTIFY));
            took.push(*w); false
        });
        let _ = c.flush();
        took
    }

    /// The program's dialogs (top-levels transient for one of its frames, which the window manager no longer knows) made transient for the
    /// client's top-level instead, so they stay above Backplane and are centred on it.
    fn dialogs_over(c: &RustConnection, pid: u32, container: u32, toplevel: u32) {
        let Some(root) = root(c) else { return };
        let pid_atom = atom(c, "_NET_WM_PID");
        let kids = |w: Window| c.query_tree(w).ok().and_then(|r| r.reply().ok()).map(|t| t.children).unwrap_or_default();
        let ours = frame_windows(c, container);
        for a in kids(root) {
            if a == toplevel { continue }
            let l1 = kids(a);
            let l2: Vec<Window> = l1.iter().flat_map(|b| kids(*b)).collect();
            for w in std::iter::once(a).chain(l1).chain(l2) {
                if prop32(c, w, pid_atom, AtomEnum::CARDINAL).first() != Some(&pid) { continue }
                let t = prop32(c, w, u32::from(AtomEnum::WM_TRANSIENT_FOR), AtomEnum::WINDOW);
                if t.first().map_or(false, |f| ours.contains(f)) {
                    let _ = c.change_property32(PropMode::REPLACE, w, AtomEnum::WM_TRANSIENT_FOR, AtomEnum::WINDOW, &[toplevel]);
                }
            }
        }
    }
    pub fn dialogs(pid: u32, container: u32, toplevel: u32) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        dialogs_over(c, pid, container, toplevel);
        let _ = c.flush();
    }

    /// The frames in the container: its children, or (the fork) the children of KiCad's holder window, an override-redirect child of the container.
    fn frame_windows(c: &RustConnection, container: u32) -> Vec<Window> {
        let kids = |w: Window| c.query_tree(w).ok().and_then(|r| r.reply().ok()).map(|t| t.children).unwrap_or_default();
        let mut out = Vec::new();
        for k in kids(container) {
            let holder = c.get_window_attributes(k).ok().and_then(|r| r.reply().ok()).map_or(false, |a| a.override_redirect);
            if holder { out.extend(kids(k)); } else { out.push(k); }
        }
        out
    }
    /// The fork's holder in our save-set: if the client dies, the X server moves it to the root instead of destroying KiCad's windows with ours.
    /// XFixes' form (to the root, unmapped): with a reparenting window manager the core save-set would put the holder in the manager's frame,
    /// which it then destroys, and KiCad with it (bp.8, BACKPLANE_EMBED.md).
    pub fn keep(holder: u32) {
        use x11rb::protocol::xfixes::{self, ConnectionExt as _};
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let _ = c.xfixes_query_version(5, 0).ok().and_then(|r| r.reply().ok());
        let _ = c.xfixes_change_save_set(xfixes::SaveSetMode::INSERT, xfixes::SaveSetTarget::ROOT, xfixes::SaveSetMapping::UNMAP, holder);
        let _ = c.flush();
    }

    /// The container's children now (adopted, or made there by the fork), with their titles and kinds.
    pub fn frames(container: u32) -> Vec<Frame> {
        let Some(g) = conn() else { return vec![] }; let c = g.as_ref().unwrap();
        frame_windows(c, container).into_iter().map(|w| { let t = title(c, w); Frame { window: w, kind: super::kind_of_title(&t), title: t } }).collect()
    }

    /// The container over the pane at `r` with `show` (one of its children) filling it, or the container unmapped (`r` None).
    /// In stock mode (`managed` None), `place` maps `show` and unmaps siblings; in fork mode (`managed` Some), GTK3 owns frame map state via
    /// `ShowFrame`/`HideFrame` (calling external `XMapWindow` before `GTKShowFromOnIdle` trips `gtk_window_map_event` -> `gdk_window_hide`),
    /// so `place` only configures `show` and sends a synthetic `ConfigureNotify` to update root coords and wake `bp.8`'s GDK event loop.
    pub fn place(container: u32, r: Option<R>, show: Option<u32>, managed: Option<&[u32]>, last_root: &mut (i32, i32)) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let Some(r) = r else { let _ = c.unmap_window(container); let _ = c.flush(); return };
        let (w, h) = (r.w.max(1) as u32, r.h.max(1) as u32);
        let _ = c.configure_window(container, &ConfigureWindowAux::new().x(r.x).y(r.y).width(w).height(h));
        if let Some(t) = c.query_tree(container).ok().and_then(|q| q.reply().ok()) {
            for k in t.children {
                if c.get_window_attributes(k).ok().and_then(|q| q.reply().ok()).map_or(false, |a| a.override_redirect) {
                    let _ = c.configure_window(k, &ConfigureWindowAux::new().x(0).y(0).width(w).height(h).border_width(0));
                    if show.is_some() { let _ = c.map_window(k); }
                }
            }
        }
        for k in frame_windows(c, container) {
            if managed.map_or(false, |m| !m.contains(&k)) { continue }
            if Some(k) == show {
                let _ = c.configure_window(k, &ConfigureWindowAux::new().x(0).y(0).width(w).height(h).border_width(0));
                if managed.is_none() { let _ = c.map_window(k); }
            } else if managed.is_none() {
                let _ = c.unmap_window(k);
            }
        }
        if show.is_some() { let _ = c.map_window(container); } else { let _ = c.unmap_window(container); }
        if let (Some(k), Some(root)) = (show, root(c)) {
            if let Some(t) = c.translate_coordinates(container, root, 0, 0).ok().and_then(|r| r.reply().ok()) {
                let at = (t.dst_x as i32, t.dst_y as i32);
                if managed.is_some() || at != *last_root {
                    *last_root = at;
                    let ev = ConfigureNotifyEvent { response_type: CONFIGURE_NOTIFY_EVENT, sequence: 0, event: k, window: k, above_sibling: x11rb::NONE,
                        x: t.dst_x, y: t.dst_y, width: w as u16, height: h as u16, border_width: 0, override_redirect: false };
                    let _ = c.send_event(false, k, EventMask::STRUCTURE_NOTIFY, ev);
                }
            }
        }
        let _ = c.flush();
    }

    /// Re-check the container's root coordinates and `show`'s geometry (e.g. on a pure WM top-level move, or right after GTK's deferred
    /// `gtk_widget_show` maps `show` at its own default size) and sync `show` to `(0, 0, w, h)` + current root position.
    pub fn sync_root(container: u32, r: R, show: u32, last_root: &mut (i32, i32)) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let (uw, uh) = (r.w.max(1) as u32, r.h.max(1) as u32);
        let mut resized = false;
        if c.get_window_attributes(show).ok().and_then(|q| q.reply().ok()).map_or(false, |a| a.map_state == MapState::VIEWABLE) {
            if let Some(geo) = c.get_geometry(show).ok().and_then(|q| q.reply().ok()) {
                if (geo.x as i32, geo.y as i32, geo.width as u32, geo.height as u32) != (0, 0, uw, uh) {
                    let _ = c.configure_window(show, &ConfigureWindowAux::new().x(0).y(0).width(uw).height(uh).border_width(0));
                    resized = true;
                }
            }
        }
        let Some(root) = root(c) else { if resized { let _ = c.flush(); } return };
        let Some(t) = c.translate_coordinates(container, root, 0, 0).ok().and_then(|q| q.reply().ok()) else { if resized { let _ = c.flush(); } return };
        let at = (t.dst_x as i32, t.dst_y as i32);
        if resized || at != *last_root {
            *last_root = at;
            let (w, h) = (uw as u16, uh as u16);
            let ev = ConfigureNotifyEvent { response_type: CONFIGURE_NOTIFY_EVENT, sequence: 0, event: show, window: show, above_sibling: x11rb::NONE,
                x: t.dst_x, y: t.dst_y, width: w, height: h, border_width: 0, override_redirect: false };
            let _ = c.send_event(false, show, EventMask::STRUCTURE_NOTIFY, ev);
            let _ = c.flush();
        }
    }

    /// The container's shape: the pane minus `keep`'s complement (the rectangles `cut` left), or whole again (`keep` None).
    pub fn holes(container: u32, keep: Option<&[R]>) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        match keep {
            Some(k) => { let rs: Vec<Rectangle> = k.iter().map(|r| Rectangle { x: r.x as i16, y: r.y as i16, width: r.w.max(0) as u16, height: r.h.max(0) as u16 }).collect();
                         let _ = c.shape_rectangles(shape::SO::SET, shape::SK::BOUNDING, ClipOrdering::UNSORTED, container, 0, 0, &rs); }
            None => { let _ = c.shape_mask(shape::SO::SET, shape::SK::BOUNDING, container, 0, 0, x11rb::NONE); }
        }
        let _ = c.flush();
    }

    /// The keyboard to `w` (a frame), when it is viewable.
    pub fn focus(w: u32) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let viewable = c.get_window_attributes(w).ok().and_then(|r| r.reply().ok()).map_or(false, |a| a.map_state == MapState::VIEWABLE);
        if viewable { let _ = c.set_input_focus(InputFocus::PARENT, w, x11rb::CURRENT_TIME); let _ = c.flush(); }
    }
    /// Whether the X keyboard focus is in `container` (on one of its frames or their windows): the editor has the keys.
    pub fn has_focus(container: u32) -> bool {
        let Some(g) = conn() else { return false }; let c = g.as_ref().unwrap();
        let Some(mut w) = c.get_input_focus().ok().and_then(|r| r.reply().ok()).map(|f| f.focus) else { return false };
        let root = root(c).unwrap_or(0);
        for _ in 0..32 { if w == container { return true } if w == root || w == 0 || w == 1 { return false }
            match c.query_tree(w).ok().and_then(|r| r.reply().ok()) { Some(t) => w = t.parent, None => return false } }
        false
    }
    /// With a mouse button down now: whether the pointer is over `w` (a click there gives the editor the keyboard, a click elsewhere gives it back).
    /// None while no button is down.
    pub fn button_over(w: u32) -> Option<bool> {
        let g = conn()?; let c = g.as_ref().unwrap();
        let p = c.query_pointer(w).ok()?.reply().ok()?;
        let down = u16::from(p.mask) & (u16::from(KeyButMask::BUTTON1) | u16::from(KeyButMask::BUTTON2) | u16::from(KeyButMask::BUTTON3)) != 0;
        if !down { return None }
        let geo = c.get_geometry(w).ok()?.reply().ok()?;
        Some(p.same_screen && p.win_x >= 0 && p.win_y >= 0 && (p.win_x as i32) < geo.width as i32 && (p.win_y as i32) < geo.height as i32)
    }
    /// Passive button grab on `container` while unfocused so even a short click transfers X input focus and replays cleanly to KiCad.
    pub fn grab_click(container: u32, on: bool) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        if on {
            let _ = c.grab_button(false, container, EventMask::BUTTON_PRESS, GrabMode::SYNC, GrabMode::ASYNC, x11rb::NONE, x11rb::NONE, ButtonIndex::ANY, ModMask::ANY);
        } else {
            let _ = c.allow_events(Allow::ASYNC_POINTER, x11rb::CURRENT_TIME);
            let _ = c.ungrab_button(ButtonIndex::ANY, container, ModMask::ANY);
        }
        let _ = c.flush();
    }
    /// Ctrl+` (the escape chord) grabbed on the client's top-level while the editor has the keyboard, so pressing it there comes to us
    /// (`escape_pressed`) and not to KiCad; ungrabbed otherwise, so the client's own keys are untouched. With and without Caps and Num Lock.
    pub fn grab_escape(top: u32, on: bool) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let Some(kc) = keycode(c, 0x60) else { return };
        for extra in [0u16, u16::from(ModMask::LOCK), u16::from(ModMask::M2), u16::from(ModMask::LOCK) | u16::from(ModMask::M2)] {
            let mods = ModMask::from(u16::from(ModMask::CONTROL) | extra);
            let _ = if on { c.grab_key(true, top, mods, kc, GrabMode::ASYNC, GrabMode::ASYNC) } else { c.ungrab_key(kc, top, mods) };
        }
        let _ = c.flush();
    }
    fn keycode(c: &RustConnection, keysym: u32) -> Option<u8> {
        let s = c.setup(); let (lo, hi) = (s.min_keycode, s.max_keycode);
        let m = c.get_keyboard_mapping(lo, hi - lo + 1).ok()?.reply().ok()?;
        let per = m.keysyms_per_keycode as usize;
        m.keysyms.chunks(per.max(1)).position(|k| k.contains(&keysym)).map(|i| lo + i as u8)
    }
    /// What came to our own connection since the last call (drained): the grabbed escape chord, and the fork's ClientMessages to the container
    /// (`_BACKPLANE_KEY`: a reserved chord pressed in KiCad, as a gpui keystroke; `_BACKPLANE_FOCUS`: a click gave KiCad the keyboard).
    pub fn events() -> Vec<super::Ev> {
        let Some(g) = conn() else { return vec![] }; let c = g.as_ref().unwrap();
        let (key, focus) = (atom(c, "_BACKPLANE_KEY"), atom(c, "_BACKPLANE_FOCUS"));
        let mut out = Vec::new();
        while let Ok(Some(e)) = c.poll_for_event() {
            match e {
                x11rb::protocol::Event::KeyPress(_) => out.push(super::Ev::Escape),
                x11rb::protocol::Event::ButtonPress(b) => {
                    let _ = c.allow_events(Allow::REPLAY_POINTER, b.time);
                    out.push(super::Ev::Click);
                }
                x11rb::protocol::Event::ClientMessage(m) if m.format == 32 => {
                    let d = m.data.as_data32();
                    if m.type_ == key { if let Some(k) = super::chord(d[1], d[2]) { out.push(if k == "ctrl-`" { super::Ev::Escape } else { super::Ev::Key(k) }); } }
                    else if m.type_ == focus { out.push(super::Ev::Focus(d[0])); }
                }
                _ => {}
            }
        }
        let _ = c.flush();
        out
    }
    /// A key pressed and released in `w` by a synthetic event (no real input): how stock KiCad, which has no IPC to open an editor, is asked
    /// to (Ctrl+P in its project manager opens the PCB editor, even unmapped). `keysym` an X keysym, `mods` an X modifier mask.
    pub fn poke(w: u32, keysym: u32, mods: u16) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let (Some(kc), Some(root)) = (keycode(c, keysym), root(c)) else { return };
        for (t, mask) in [(KEY_PRESS_EVENT, EventMask::KEY_PRESS), (KEY_RELEASE_EVENT, EventMask::KEY_RELEASE)] {
            let ev = KeyPressEvent { response_type: t, detail: kc, sequence: 0, time: x11rb::CURRENT_TIME, root, event: w, child: x11rb::NONE, root_x: 0, root_y: 0, event_x: 0, event_y: 0, state: KeyButMask::from(mods), same_screen: true };
            let _ = c.send_event(true, w, mask, ev);
        }
        let _ = c.flush();
    }
    /// Where `w` is in its parent and whether it is mapped, and how many sampled pixels are painted (excluding black and the placeholder background): what the e2e report says.
    pub fn report(w: u32) -> Option<(R, bool, usize)> {
        let g = conn()?; let c = g.as_ref().unwrap();
        let geo = c.get_geometry(w).ok()?.reply().ok()?;
        let mapped = c.get_window_attributes(w).ok()?.reply().ok()?.map_state == MapState::VIEWABLE;
        let painted = if mapped {
            c.get_image(ImageFormat::Z_PIXMAP, w, 0, 0, geo.width, geo.height, !0).ok().and_then(|r| r.reply().ok()).map_or(0, |img| {
                let mut lit = 0usize;
                let mut buckets = std::collections::HashSet::new();
                for p in img.data.chunks(4).step_by(97) {
                    if p.len() < 3 { continue }
                    let (b, g, r) = (p[0], p[1], p[2]);
                    let is_placeholder = r <= 12 && (8..=28).contains(&g) && (20..=52).contains(&b);
                    if (b > 24 || g > 24 || r > 24) && !is_placeholder {
                        lit += 1;
                        buckets.insert((b >> 4, g >> 4, r >> 4));
                    }
                }
                if buckets.len() >= 4 { lit } else { 0 }
            })
        } else { 0 };
        Some((R { x: geo.x as i32, y: geo.y as i32, w: geo.width as i32, h: geo.height as i32 }, mapped, painted))
    }

    /// Extra X11 geometry/shape/ancestry details for `w`: `(depth, (root_x, root_y), shape_rects, in_tree)`.
    pub fn window_meta(w: u32, container: Option<u32>, toplevel: u32) -> Option<(u8, (i32, i32), usize, bool)> {
        let g = conn()?; let c = g.as_ref().unwrap();
        let geo = c.get_geometry(w).ok()?.reply().ok()?;
        let root_id = root(c).unwrap_or(0);
        let rpos = if root_id != 0 {
            c.translate_coordinates(w, root_id, 0, 0).ok().and_then(|q| q.reply().ok()).map_or((0, 0), |t| (t.dst_x as i32, t.dst_y as i32))
        } else { (0, 0) };
        let shape_rects = c.shape_get_rectangles(w, shape::SK::BOUNDING).ok().and_then(|q| q.reply().ok()).map_or(1, |r| r.rectangles.len());
        let mut saw_cont = container.map_or(true, |ct| ct == w);
        let mut saw_top = toplevel == 0 || w == toplevel;
        let mut cur = w;
        for _ in 0..16 {
            if saw_cont && saw_top { break }
            if cur == 0 || cur == root_id { break }
            let Some(t) = c.query_tree(cur).ok().and_then(|q| q.reply().ok()) else { break };
            cur = t.parent;
            if Some(cur) == container { saw_cont = true; }
            if cur == toplevel { saw_top = true; }
        }
        Some((geo.depth, rpos, shape_rects, saw_cont && saw_top))
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::{Frame, R};
    pub fn container(_parent: u32) -> Option<u32> { None }
    pub fn destroy(_container: u32) {}
    pub fn adopt(_pid: u32, _container: u32, _toplevel: u32) -> Vec<u32> { vec![] }
    pub fn dialogs(_pid: u32, _container: u32, _toplevel: u32) {}
    pub fn frames(_container: u32) -> Vec<Frame> { vec![] }
    pub fn place(_container: u32, _r: Option<R>, _show: Option<u32>, _managed: Option<&[u32]>, _last_root: &mut (i32, i32)) {}
    pub fn sync_root(_container: u32, _r: R, _show: u32, _last_root: &mut (i32, i32)) {}
    pub fn holes(_container: u32, _keep: Option<&[R]>) {}
    pub fn focus(_w: u32) {}
    pub fn has_focus(_container: u32) -> bool { false }
    pub fn button_over(_w: u32) -> Option<bool> { None }
    pub fn grab_click(_container: u32, _on: bool) {}
    pub fn grab_escape(_top: u32, _on: bool) {}
    pub fn events() -> Vec<super::Ev> { vec![] }
    pub fn poke(_w: u32, _keysym: u32, _mods: u16) {}
    pub fn keep(_holder: u32) {}
    pub fn report(_w: u32) -> Option<(R, bool, usize)> { None }
    pub fn window_meta(_w: u32, _container: Option<u32>, _toplevel: u32) -> Option<(u8, (i32, i32), usize, bool)> { None }
}

pub use imp::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn titles_name_their_editor() {
        assert_eq!(kind_of_title("pic_programmer — PCB Editor"), "PCB");
        assert_eq!(kind_of_title("*ecc83-pp — Schematic Editor"), "SCH");
        assert_eq!(kind_of_title("3D Viewer"), "PCB_3D");
        assert_eq!(kind_of_title("Device:R — Symbol Editor"), "SYMBOL_EDITOR");
        assert_eq!(kind_of_title("lib:R_0603 — Footprint Editor"), "FOOTPRINT_EDITOR");
        assert_eq!(kind_of_title("KiCad 10.0 — ecc83-pp"), "MANAGER");
        assert!(dirty_title("*ecc83-pp — Schematic Editor") && !dirty_title("ecc83-pp — PCB Editor"));
    }
    #[test] fn reserved_chords_become_gpui_keystrokes() {
        assert_eq!(chord(0x6b, 4).as_deref(), Some("ctrl-k"));
        assert_eq!(chord(0xff09, 5).as_deref(), Some("ctrl-shift-tab"));
        assert_eq!(chord(0x60, 4).as_deref(), Some("ctrl-`"));
        assert_eq!(chord(0xffbe, 4), None, "F1: not a reserved key");
    }
    #[test] fn overlays_cut_holes_and_nothing_else() {
        let pane = R { x: 100, y: 50, w: 400, h: 300 };
        assert_eq!(cut(pane, &[]), vec![R { x: 0, y: 0, w: 400, h: 300 }]);
        assert_eq!(cut(pane, &[R { x: 0, y: 0, w: 50, h: 40 }]), vec![R { x: 0, y: 0, w: 400, h: 300 }], "outside the pane");
        let k = cut(pane, &[R { x: 200, y: 100, w: 100, h: 100 }]);
        let area: i32 = k.iter().map(|r| r.w * r.h).sum();
        assert_eq!(area, 400 * 300 - 100 * 100);
        for r in &k { assert!(!(r.x < 200 && r.x + r.w > 100 && r.y < 150 && r.y + r.h > 50), "{r:?} overlaps the hole"); }
        let all = cut(pane, &[R { x: 0, y: 0, w: 1000, h: 1000 }]);
        assert!(all.is_empty(), "covered whole");
    }
}

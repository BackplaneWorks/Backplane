//! Another program's windows inside the client's (the embedded KiCad editors, docs/embedded-editors.md): a container window of ours in the client's
//! window, the program's frames made its children, one of them mapped over the pane and the rest unmapped. Unlike the Browser's Chromium
//! (viewers/cef_native.rs, the same process), these windows belong to another process, so everything here goes through X11 or async Win32 calls.
//! - On X11, the container has the screen's default (24-bit) visual: gpui's window has a 32-bit one, and GL canvases made under it are refused.
//!   On Windows, the container is a `WS_CHILD` HWND under gpui's top-level HWND (with `WS_CLIPCHILDREN` on the parent), and all geometry/visibility
//!   changes use `SWP_ASYNCWINDOWPOS` / `ShowWindowAsync` so a hung KiCad never blocks gpui's thread.
//! - Stock KiCad on Linux makes its frames as top-levels: `adopt` withdraws them from the window manager and reparents them into the container.
//!   The fork's `--embedded` makes them in the container from the start, so `adopt` finds nothing to do. On Windows, only the fork is embedded.
//! - Overlays (the palette, menus) are drawn by gpui under the container; `holes` cuts them out of the container's shape (`shape_rectangles` / `SetWindowRgn`).
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
        "Gerber Viewer" | "GerbView" => "GERBVIEW", // i18n-ok: stock KiCad frame titles and wire frame kinds
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
pub enum Ev { /** Ctrl+`: the keyboard back to the client */ Escape, /** a button press over the unfocused container (replayed to KiCad) */ Click, /** a reserved chord, as gpui names a keystroke */ Key(String), /** KiCad took the keyboard on a click into this frame */ Focus(u64) }

/// A reserved chord from `_BACKPLANE_KEY` (an X keysym and modifier mask) as gpui's keystroke text: `ctrl-k`, `ctrl-shift-tab`, `ctrl-``.
pub fn chord(keysym: u32, mods: u32) -> Option<String> {
    let key = match keysym { 0x20..=0x7e => char::from_u32(keysym)?.to_ascii_lowercase().to_string(), 0xff09 | 0xfe20 /* ISO_Left_Tab: GTK's Shift+Tab */ => "tab".into(), 0xff0d => "enter".into(), 0xff1b => "escape".into(), _ => return None };
    let mut s = String::new();
    if mods & 4 != 0 { s.push_str("ctrl-"); } if mods & 8 != 0 { s.push_str("alt-"); } if mods & 0x40 != 0 { s.push_str("super-"); } if mods & 1 != 0 { s.push_str("shift-"); }
    Some(s + &key)
}

/// Default reserved chord index from Windows `WM_APP + 0x102` (`wParam = aData[3]`, index in `BACKPLANE_EMBED::m_reserved`).
pub fn reserved_chord_index(idx: usize) -> Option<&'static str> {
    match idx {
        0 => Some("ctrl-k"), // i18n-ok: gpui keystroke
        1 => Some("ctrl-j"), // i18n-ok: gpui keystroke
        2 => Some("ctrl-tab"), // i18n-ok: gpui keystroke
        3 => Some("ctrl-shift-tab"), // i18n-ok: gpui keystroke
        4 => Some("ctrl-`"), // i18n-ok: gpui keystroke
        5 => Some("ctrl-;"), // i18n-ok: gpui keystroke (a comment: host kicad/comment.rs RESERVED)
        6 => Some("alt-shift-b"), // i18n-ok: gpui keystroke (the management panels: BOM, Checks, Lifecycle, Fab; host kicad/comment.rs RESERVED)
        7 => Some("alt-shift-c"), // i18n-ok: gpui keystroke
        8 => Some("alt-shift-l"), // i18n-ok: gpui keystroke
        9 => Some("alt-shift-f"), // i18n-ok: gpui keystroke
        10 => Some(";"), // i18n-ok: gpui keystroke (bp.13: KiCad's own comment key, host kicad/comment.rs `reserved`)
        63 => Some(";"), // i18n-ok: gpui keystroke (KiCad's comment key handed over, bp.13 batch 5 "comment-key-63": a fixed index no reserved chord takes)
        _ => None,
    }
}

/// Windows: whether a visible top-level window of KiCad's is a floating dialog that `dialogs` owns and centres over Backplane's window:
/// not a child, with a real caption (both WS_CAPTION bits: WS_BORDER|WS_DLGFRAME), and never a popup menu (#32768) or a tooltip.
/// A popup menu is WS_POPUP|WS_BORDER (one caption bit): taking it for a dialog re-owned and re-centred KiCad's right-click menu,
/// which moved it off the cursor or closed it at once.
pub fn floating_dialog(style: u32, class: &str) -> bool {
    const WS_CHILD: u32 = 0x4000_0000;
    const WS_CAPTION: u32 = 0x00C0_0000;
    if style & WS_CHILD != 0 || style & WS_CAPTION != WS_CAPTION { return false }
    !matches!(class, "#32768" | "tooltips_class32") // i18n-ok: Win32 window class names
}

/// A frame the container holds: its window, its title as the program last set it, and its kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame { pub window: u64, pub title: String, pub kind: &'static str }

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
    pub fn container(parent: u64) -> Option<u64> {
        let parent = parent as u32;
        let g = conn()?; let c = g.as_ref().unwrap();
        let scr = c.setup().roots.get(SCREEN.load(std::sync::atomic::Ordering::Relaxed))?;
        let id = c.generate_id().ok()?;
        c.create_window(scr.root_depth, id, parent, 0, 0, 1, 1, 0, WindowClass::INPUT_OUTPUT, scr.root_visual,
            &CreateWindowAux::new().background_pixmap(x11rb::NONE).border_pixel(scr.black_pixel).colormap(scr.default_colormap)
                .event_mask(EventMask::SUBSTRUCTURE_NOTIFY)).ok()?.check().ok()?;
        let _ = c.flush(); Some(id as u64)
    }
    pub fn container_for(parent: u64, _key: &str) -> Option<u64> { container(parent) }
    /// Every child of `old` (stock KiCad's frames, or the fork's holder) moved into `new`, unmapped where it was not shown: a pane moved to
    /// another window (popped out, or back in) takes KiCad with it, its state untouched.
    pub fn move_frames(old: u64, new: u64) {
        let (old, new) = (old as u32, new as u32);
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let kids = c.query_tree(old).ok().and_then(|r| r.reply().ok()).map(|t| t.children).unwrap_or_default();
        for k in kids { let _ = c.reparent_window(k, new, 0, 0); }
        let _ = c.flush();
    }
    pub fn destroy(container: u64) {
        let container = container as u32;
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let _ = c.allow_events(Allow::REPLAY_POINTER, x11rb::CURRENT_TIME);
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
    pub fn adopt(pid: u32, container: u64, toplevel: u64) -> Vec<u64> {
        use x11rb::protocol::xfixes::{self, ConnectionExt as _};
        let (container, toplevel) = (container as u32, toplevel as u32);
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
            took.push(w as u64);
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
            took.push(*w as u64); false
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
    pub fn dialogs(pid: u32, container: u64, toplevel: u64) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        dialogs_over(c, pid, container as u32, toplevel as u32);
        let _ = c.flush();
    }
    /// The program's dialogs that came up since the last call (KiCad's load-time notices, its questions), each moved once to the middle of
    /// `over` (the pane, in root coordinates), so what KiCad asks shows where its editor is, not in the middle of the screen. Answers the
    /// titles of the program's dialogs on screen now: a modal one stops KiCad's API (no events, no pings), so this is how Backplane knows.
    pub fn centre_dialogs(pid: u32, over: R) -> Vec<String> {
        static DONE: Mutex<Vec<u32>> = Mutex::new(Vec::new());
        let mut open = Vec::new();
        let Some(g) = conn() else { return open }; let c = g.as_ref().unwrap();
        let Some(root) = root(c) else { return open };
        let pid_atom = atom(c, "_NET_WM_PID"); // i18n-ok: X11 atom name
        let kids = |w: Window| c.query_tree(w).ok().and_then(|r| r.reply().ok()).map(|t| t.children).unwrap_or_default();
        let mut done = DONE.lock().unwrap();
        let mut moved = false;
        for a in kids(root) {
            for w in std::iter::once(a).chain(kids(a)) {
                if prop32(c, w, pid_atom, AtomEnum::CARDINAL).first() != Some(&pid) { continue }
                if prop32(c, w, u32::from(AtomEnum::WM_TRANSIENT_FOR), AtomEnum::WINDOW).is_empty() { continue }
                let Some(at) = c.get_window_attributes(w).ok().and_then(|r| r.reply().ok()) else { continue };
                if at.override_redirect || at.map_state != MapState::VIEWABLE { continue }
                open.push(title(c, w));
                if done.contains(&w) || over.w <= 0 { continue }
                let Some(geo) = c.get_geometry(w).ok().and_then(|r| r.reply().ok()) else { continue };
                let (x, y) = (over.x + (over.w - geo.width as i32) / 2, over.y + (over.h - geo.height as i32) / 2);
                let _ = c.configure_window(w, &ConfigureWindowAux::new().x(x.max(0)).y(y.max(0)));
                done.push(w); moved = true;
            }
        }
        if done.len() > 256 { done.drain(..128); }
        if moved { let _ = c.flush(); }
        open
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
    pub fn keep(holder: u64) {
        use x11rb::protocol::xfixes::{self, ConnectionExt as _};
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let _ = c.xfixes_query_version(5, 0).ok().and_then(|r| r.reply().ok());
        let _ = c.xfixes_change_save_set(xfixes::SaveSetMode::INSERT, xfixes::SaveSetTarget::ROOT, xfixes::SaveSetMapping::UNMAP, holder as u32);
        let _ = c.flush();
    }

    /// The container's children now (adopted, or made there by the fork), with their titles and kinds.
    pub fn frames(container: u64) -> Vec<Frame> {
        let Some(g) = conn() else { return vec![] }; let c = g.as_ref().unwrap();
        frame_windows(c, container as u32).into_iter().map(|w| { let t = title(c, w); Frame { window: w as u64, kind: super::kind_of_title(&t), title: t } }).collect()
    }

    /// The container over the pane at `r` with `show` (one of its children) filling it, or the container unmapped (`r` None).
    /// In stock mode (`managed` None), `place` maps `show` and unmaps siblings; in fork mode (`managed` Some), GTK3 owns frame map state via
    /// `ShowFrame`/`HideFrame` (calling external `XMapWindow` before `GTKShowFromOnIdle` trips `gtk_window_map_event` -> `gdk_window_hide`),
    /// so `place` only configures `show` and sends a synthetic `ConfigureNotify` to update root coords and wake `bp.8`'s GDK event loop.
    /// bp.9's local map: `place` where the client maps `show` and unmaps the other frames in `mine` (shown by KiCad once); a frame KiCad has
    /// never shown is left to it (its first show goes through KiCad, for GTK).
    pub fn place_local(container: u64, r: Option<R>, show: Option<u64>, mine: &[u64], last_root: &mut (i32, i32)) {
        place(container, r, show, Some(mine), last_root);
        if r.is_none() { return }
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        // no frame of background between two editors: the one coming has no X background (it and its canvases), goes on top and is mapped
        // before the others are unmapped, so the old editor's pixels stay on screen until the new one has drawn over them
        if let Some(k) = show.filter(|k| mine.contains(k)) {
            keep_pixels(c, k as u32, 3);
            let _ = c.configure_window(k as u32, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE));
            let _ = c.map_window(k as u32);
        }
        for k in mine { if Some(*k) != show { let _ = c.unmap_window(*k as u32); } }
        let _ = c.flush();
    }
    /// `w` and its windows `depth` levels down keep what is on screen under them until they draw (no X background is painted on map or
    /// expose): what the person sees between a map and KiCad's first paint is the last good picture, never a flash of background.
    fn keep_pixels(c: &RustConnection, w: Window, depth: u32) {
        let _ = c.change_window_attributes(w, &ChangeWindowAttributesAux::new().background_pixmap(x11rb::NONE));
        if depth == 0 { return }
        for k in c.query_tree(w).ok().and_then(|r| r.reply().ok()).map(|t| t.children).unwrap_or_default() { keep_pixels(c, k, depth - 1); }
    }
    pub fn place(container: u64, r: Option<R>, show: Option<u64>, managed: Option<&[u64]>, last_root: &mut (i32, i32)) {
        let container = container as u32;
        let show = show.map(|x| x as u32);
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
            if managed.map_or(false, |m| !m.contains(&(k as u64))) { continue }
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
    pub fn sync_root(container: u64, r: R, show: u64, last_root: &mut (i32, i32)) {
        let (container, show) = (container as u32, show as u32);
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
    pub fn holes(container: u64, keep: Option<&[R]>) {
        let container = container as u32;
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        match keep {
            Some(k) => { let rs: Vec<Rectangle> = k.iter().map(|r| Rectangle { x: r.x as i16, y: r.y as i16, width: r.w.max(0) as u16, height: r.h.max(0) as u16 }).collect();
                         let _ = c.shape_rectangles(shape::SO::SET, shape::SK::BOUNDING, ClipOrdering::UNSORTED, container, 0, 0, &rs); }
            None => { let _ = c.shape_mask(shape::SO::SET, shape::SK::BOUNDING, container, 0, 0, x11rb::NONE); }
        }
        let _ = c.flush();
    }

    /// An explicit hand-back to the client's window (a host chord pressed in the editor): as `focus` here.
    #[track_caller]
    pub fn hand_back(w: u64) { focus(w) }
    /// Windows only (the F3 input shield): Linux has its button grab (grab_click).
    pub fn shield(_container: u64, _on: bool) {}
    pub fn shield_tick(_container: u64, _shown: Option<u64>) {}
    pub fn keys_really_backplanes(_container: u64) -> bool { true }
    pub fn focus_debug(_container: u64) -> serde_json::Value { serde_json::Value::Null }
    /// Windows only (an invisible KiCad window taking the foreground): never so here.
    pub fn invisible_kicad_foreground(_frame: u64) -> Option<(u64, String)> { None }
    /// Whether `frame` and its container are both visible now (Windows re-posts an opening focus request on that): always so here.
    pub fn frame_visible(_container: u64, _frame: u64) -> bool { true }
    /// The keyboard to `w` (a frame), when it is viewable.
    #[track_caller]
    pub fn focus(w: u64) {
        if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() { eprintln!("GPUI_LOG {{\"kicad_focus\":{w},\"at\":\"{}\"}}", std::panic::Location::caller()); }
        let w = w as u32;
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let viewable = c.get_window_attributes(w).ok().and_then(|r| r.reply().ok()).map_or(false, |a| a.map_state == MapState::VIEWABLE);
        if viewable { let _ = c.set_input_focus(InputFocus::PARENT, w, x11rb::CURRENT_TIME); let _ = c.flush(); }
    }
    /// Pointer input over the container off (`off`) or back: while Backplane's own window over the editor has the keyboard, X would hand every key
    /// to KiCad when the pointer rests over it (keys go to the window under the pointer when it is inside the focus window), so its input region
    /// is emptied for that while; the editor stays drawn.
    pub fn input_off(container: u64, off: bool) {
        let container = container as u32;
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let _ = if off { c.shape_rectangles(shape::SO::SET, shape::SK::INPUT, ClipOrdering::UNSORTED, container, 0, 0, &[]) } else { c.shape_mask(shape::SO::SET, shape::SK::INPUT, container, 0, 0, x11rb::NONE) };
        let _ = c.flush();
    }
    /// Our connection's socket: readable when an event (a ClientMessage KiCad sends the container) is waiting.
    pub fn conn_fd() -> Option<i32> { use std::os::fd::AsRawFd; let g = conn()?; Some(g.as_ref()?.stream().as_raw_fd()) }
    /// Whether the ';' key is down now (the comment chord held: hold to talk). X11's own state, so a fake release of key repeat reads as held.
    pub fn semicolon_held() -> Option<bool> {
        static KC: OnceLock<u8> = OnceLock::new();
        let g = conn()?; let c = g.as_ref()?;
        let kc = *KC.get_or_init(|| {
            let (min, max) = (c.setup().min_keycode, c.setup().max_keycode);
            c.get_keyboard_mapping(min, max - min + 1).ok().and_then(|r| r.reply().ok()).and_then(|m| {
                let per = m.keysyms_per_keycode.max(1) as usize;
                m.keysyms.chunks(per).position(|k| k.contains(&0x3b)).map(|i| min + i as u8)
            }).unwrap_or(0)
        });
        if kc == 0 { return None }
        let keys = c.query_keymap().ok()?.reply().ok()?.keys;
        Some(keys[kc as usize / 8] & (1 << (kc % 8)) != 0)
    }
    /// Whether the X keyboard focus is in a top-level window of the program `pid` (a KiCad dialog: its own top-level, outside the pane):
    /// the keyboard is KiCad's there, never taken back to the frame or to Backplane while it is.
    pub fn focus_in_pid(pid: u32) -> bool {
        if pid == 0 { return false }
        let Some(g) = conn() else { return false }; let c = g.as_ref().unwrap();
        let Some(mut w) = c.get_input_focus().ok().and_then(|r| r.reply().ok()).map(|f| f.focus) else { return false };
        let pid_atom = atom(c, "_NET_WM_PID");
        let root = root(c).unwrap_or(0);
        for _ in 0..32 {
            if w == root || w <= 1 { return false }
            if prop32(c, w, pid_atom, AtomEnum::CARDINAL).first() == Some(&pid) { return true }
            match c.query_tree(w).ok().and_then(|r| r.reply().ok()) { Some(t) => w = t.parent, None => return false }
        }
        false
    }
    /// Whether the X keyboard focus is in `container` (on one of its frames or their windows): the editor has the keys.
    pub fn has_focus(container: u64) -> bool {
        let container = container as u32;
        let Some(g) = conn() else { return false }; let c = g.as_ref().unwrap();
        let Some(mut w) = c.get_input_focus().ok().and_then(|r| r.reply().ok()).map(|f| f.focus) else { return false };
        let root = root(c).unwrap_or(0);
        for _ in 0..32 { if w == container { return true } if w == root || w == 0 || w == 1 { return false }
            match c.query_tree(w).ok().and_then(|r| r.reply().ok()) { Some(t) => w = t.parent, None => return false } }
        false
    }
    /// With a mouse button down now: whether the pointer is over `w` (a click there gives the editor the keyboard, a click elsewhere gives it back).
    /// None while no button is down.
    pub fn button_over(w: u64) -> Option<bool> {
        let w = w as u32;
        let g = conn()?; let c = g.as_ref().unwrap();
        let p = c.query_pointer(w).ok()?.reply().ok()?;
        let down = u16::from(p.mask) & (u16::from(KeyButMask::BUTTON1) | u16::from(KeyButMask::BUTTON2) | u16::from(KeyButMask::BUTTON3)) != 0;
        if !down { return None }
        let geo = c.get_geometry(w).ok()?.reply().ok()?;
        Some(p.same_screen && p.win_x >= 0 && p.win_y >= 0 && (p.win_x as i32) < geo.width as i32 && (p.win_y as i32) < geo.height as i32)
    }
    /// Passive button grab on `container` while unfocused so even a short click transfers X input focus and replays cleanly to KiCad.
    pub fn grab_click(container: u64, on: bool) {
        let container = container as u32;
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        if on {
            let _ = c.grab_button(false, container, EventMask::BUTTON_PRESS, GrabMode::SYNC, GrabMode::ASYNC, x11rb::NONE, x11rb::NONE, ButtonIndex::ANY, ModMask::ANY);
        } else {
            // a press the grab froze goes on to KiCad (replayed), never dropped: the click that gave KiCad the keyboard is also KiCad's click
            let _ = c.allow_events(Allow::REPLAY_POINTER, x11rb::CURRENT_TIME);
            let _ = c.ungrab_button(ButtonIndex::ANY, container, ModMask::ANY);
        }
        let _ = c.flush();
    }
    /// Ctrl+` (the escape chord) grabbed on the client's top-level while the editor has the keyboard, so pressing it there comes to us
    /// (`escape_pressed`) and not to KiCad; ungrabbed otherwise, so the client's own keys are untouched. With and without Caps and Num Lock.
    pub fn grab_escape(top: u64, on: bool) {
        let top = top as u32;
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        // Ctrl+` (the keyboard back), and the management panels' Alt+Shift+B/C/L/F (host kicad/comment.rs RESERVED): caught here even when
        // KiCad's own hand-off (_BACKPLANE_KEY) does not see the key
        let alt_shift = u16::from(ModMask::M1) | u16::from(ModMask::SHIFT);
        // and Backplane's own chords (Ctrl+K, Ctrl+J, Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+;): a reserved key reaches Backplane even when KiCad's
        // hand-off misses it (a runtime without the fork's popup keys, a frame KiCad has not focused)
        let (ctrl, ctrl_shift) = (u16::from(ModMask::CONTROL), u16::from(ModMask::CONTROL) | u16::from(ModMask::SHIFT));
        for (keysym, base) in [(0x60u32, ctrl), (0x62, alt_shift), (0x63, alt_shift), (0x6c, alt_shift), (0x66, alt_shift),
                               (0x6b, ctrl), (0x6a, ctrl), (0xff09, ctrl), (0xff09, ctrl_shift), (0x3b, ctrl)] {
            let Some(kc) = keycode(c, keysym) else { continue };
            for extra in [0u16, u16::from(ModMask::LOCK), u16::from(ModMask::M2), u16::from(ModMask::LOCK) | u16::from(ModMask::M2)] {
                let mods = ModMask::from(base | extra);
                let _ = if on { c.grab_key(true, top, mods, kc, GrabMode::ASYNC, GrabMode::ASYNC) } else { c.ungrab_key(kc, top, mods) };
            }
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
    /// What our connection read for the pane and it has not taken yet (`pump` fills it, `events` empties it).
    static PENDING: Mutex<Vec<super::Ev>> = Mutex::new(Vec::new());
    /// Reads every event waiting on our connection, keeping the pane's (a key KiCad hands back, a click, KiCad taking the keyboard) for `events`
    /// and dropping the rest; true when it kept one. The socket is empty after it, so the pane's wake thread (kicad_pane) sleeps in poll until
    /// something new comes: woken only for the pane's own events, never by noise it never reads (that spun the main thread for seconds).
    pub fn pump() -> bool {
        let Some(g) = conn() else { return false }; let c = g.as_ref().unwrap();
        let (key, focus) = (atom(c, "_BACKPLANE_KEY"), atom(c, "_BACKPLANE_FOCUS"));
        let mut out = Vec::new();
        while let Ok(Some(e)) = c.poll_for_event() {
            match e {
                // a grabbed chord (grab_escape): Ctrl+` gives the keyboard back, any other is a reserved chord for the client
                x11rb::protocol::Event::KeyPress(k) => {
                    let sym = c.get_keyboard_mapping(k.detail, 1).ok().and_then(|r| r.reply().ok()).and_then(|m| m.keysyms.first().copied()).unwrap_or(0x60);
                    match super::chord(sym, u16::from(k.state) as u32 & (1 | 4 | 8 | 0x40)) { Some(ch) if ch != "ctrl-`" => out.push(super::Ev::Key(ch)), _ => out.push(super::Ev::Escape) }
                }
                x11rb::protocol::Event::ButtonPress(b) => {
                    let _ = c.allow_events(Allow::REPLAY_POINTER, b.time);
                    out.push(super::Ev::Click);
                }
                x11rb::protocol::Event::ClientMessage(m) if m.format == 32 => {
                    let d = m.data.as_data32();
                    if m.type_ == key { if let Some(k) = super::chord(d[1], d[2]) { out.push(if k == "ctrl-`" { super::Ev::Escape } else { super::Ev::Key(k) }); } }
                    else if m.type_ == focus { out.push(super::Ev::Focus(d[0] as u64)); }
                }
                _ => {}
            }
        }
        let _ = c.flush();
        drop(g);
        let kept = !out.is_empty();
        if kept { PENDING.lock().unwrap_or_else(|e| e.into_inner()).extend(out); }
        kept
    }
    pub fn events() -> Vec<super::Ev> {
        pump();
        std::mem::take(&mut *PENDING.lock().unwrap_or_else(|e| e.into_inner()))
    }
    /// A key pressed and released in `w` by a synthetic event (no real input): how stock KiCad, which has no IPC to open an editor, is asked
    /// to (Ctrl+P in its project manager opens the PCB editor, even unmapped). `keysym` an X keysym, `mods` an X modifier mask.
    pub fn poke(w: u64, keysym: u32, mods: u16) {
        let w = w as u32;
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let (Some(kc), Some(root)) = (keycode(c, keysym), root(c)) else { return };
        for (t, mask) in [(KEY_PRESS_EVENT, EventMask::KEY_PRESS), (KEY_RELEASE_EVENT, EventMask::KEY_RELEASE)] {
            let ev = KeyPressEvent { response_type: t, detail: kc, sequence: 0, time: x11rb::CURRENT_TIME, root, event: w, child: x11rb::NONE, root_x: 0, root_y: 0, event_x: 0, event_y: 0, state: KeyButMask::from(mods), same_screen: true };
            let _ = c.send_event(true, w, mask, ev);
        }
        let _ = c.flush();
    }
    /// Where `w` is in its parent and whether it is mapped, and how many sampled pixels are painted (excluding black and the placeholder background): what the e2e report says.
    pub fn report(w: u64) -> Option<(R, bool, usize)> {
        let w = w as u32;
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
    pub fn window_meta(w: u64, container: Option<u64>, toplevel: u64) -> Option<(u8, (i32, i32), usize, bool)> {
        let (w, container, toplevel) = (w as u32, container.map(|x| x as u32), toplevel as u32);
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

#[cfg(windows)]
mod imp {
    use super::{Ev, Frame, R};
    use std::{collections::{HashMap, HashSet}, ptr::null_mut, sync::{atomic::{AtomicBool, AtomicU64, Ordering}, Mutex, OnceLock}};
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            BitBlt, CombineRgn, CreateCompatibleBitmap, CreateCompatibleDC, CreateRectRgn, CreateSolidBrush,
            DeleteDC, DeleteObject, GetDC, GetDIBits, GetRegionData, GetWindowRgn, ReleaseDC, ScreenToClient,
            SelectObject, SetWindowRgn, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, RGNDATA, RGN_OR,
            SRCCOPY,
        },
        System::Threading::{AttachThreadInput, GetCurrentThreadId},
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, RegisterHotKey, SetFocus, UnregisterHotKey, VkKeyScanW, MOD_CONTROL,
                MOD_NOREPEAT, VK_OEM_3,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, EnumChildWindows, EnumWindows, GetAncestor, GetClassNameW,
                GetCursorPos, GetGUIThreadInfo, GetSystemMetrics, GetWindow, GetWindowLongPtrW, GetWindowRect,
                GetWindowThreadProcessId, InternalGetWindowText, IsChild, IsWindow,
                IsWindowVisible, LoadCursorW, PostMessageW, RegisterClassExW, SetParent, SetWindowLongPtrW,
                SetForegroundWindow, SetWindowPos, ShowWindowAsync, WindowFromPoint, GA_PARENT, GA_ROOT, GUITHREADINFO, GWLP_HWNDPARENT,
                GWL_STYLE, GW_CHILD, IDC_ARROW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
                SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOSIZE,
                SWP_NOZORDER, SW_HIDE, SW_SHOWNA, WM_ACTIVATE, WM_APP, WM_HOTKEY, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
                WM_MOUSEACTIVATE, WM_PARENTNOTIFY, WM_RBUTTONDOWN, WNDCLASSEXW,
                WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
            },
        },
    };

    const WM_BP_FOCUS: u32 = WM_APP + 0x101;
    const WM_BP_KEY: u32 = WM_APP + 0x102;
    const ESC_HOTKEY_ID: i32 = 1;

    fn hwnd(w: u64) -> HWND { w as usize as HWND }
    fn id_of(h: HWND) -> u64 { h as usize as u64 }
    fn mono_ms() -> u64 {
        static T0: OnceLock<std::time::Instant> = OnceLock::new();
        T0.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
    }

    fn events_q() -> &'static Mutex<Vec<Ev>> { static Q: OnceLock<Mutex<Vec<Ev>>> = OnceLock::new(); Q.get_or_init(Default::default) }
    fn containers_map() -> &'static Mutex<HashMap<String, u64>> { static M: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new(); M.get_or_init(Default::default) }
    fn centered_dialogs() -> &'static Mutex<HashSet<u64>> { static D: OnceLock<Mutex<HashSet<u64>>> = OnceLock::new(); D.get_or_init(Default::default) }

    static ACTIVE_CONTAINER: AtomicU64 = AtomicU64::new(0);
    static HOTKEY_HWND: AtomicU64 = AtomicU64::new(0);
    static ATTACHED_TID: AtomicU64 = AtomicU64::new(0);
    static CLICK_GRACE_UNTIL: AtomicU64 = AtomicU64::new(0);
    static FOCUS_ESCAPED: AtomicBool = AtomicBool::new(false);
    static CHILD_FOCUSED: AtomicBool = AtomicBool::new(false);
    /// When an explicit hand-back last made gpui's window the keyboard's (`hand_back`): a KiCad focus notice still in flight then
    /// (sent for a focus request KiCad accepted just before) is stale and must not attach the queues and take the keyboard back
    /// (opening-focus run Bp-7f4: Ctrl+K opened the palette, a late notice gave KiCad the keys, the typing reached the board).
    static HANDED_BACK_AT: AtomicU64 = AtomicU64::new(0);
    /// When a button last went down in the pane (the container's WM_PARENTNOTIFY / WM_MOUSEACTIVATE): a focus notice after a press
    /// that followed the hand-back is the click's own, never stale (opening-focus run B5: a canvas click 0.5 s after Ctrl+` lost
    /// its notice to a time-only rule).
    static PANE_PRESS_AT: AtomicU64 = AtomicU64::new(0);
    /// The frames Backplane has handed the keys to: past their cold construction, so `has_focus`'s reclaim (for wxMSW
    /// focusing a canvas by itself while a frame is built) never undoes a hand-in Backplane asked for (the editor tab's was)
    fn handed_in() -> &'static Mutex<HashSet<u64>> { static S: OnceLock<Mutex<HashSet<u64>>> = OnceLock::new(); S.get_or_init(Default::default) }

    unsafe fn in_child_focus(c: HWND) -> bool {
        if c.is_null() || unsafe { IsWindow(c) } == 0 { return false }
        let mut gti = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..unsafe { std::mem::zeroed() } };
        if unsafe { GetGUIThreadInfo(0, &mut gti) } == 0 || gti.hwndFocus.is_null() { return false }
        gti.hwndFocus == c || unsafe { IsChild(c, gti.hwndFocus) } != 0
    }

    unsafe fn attach_visible_child_queue(container: HWND) {
        let cur = unsafe { GetCurrentThreadId() };
        for f in frames(id_of(container)) {
            let kh = hwnd(f.window);
            if unsafe { IsWindowVisible(kh) } != 0 {
                let ctid = unsafe { GetWindowThreadProcessId(kh, null_mut()) };
                if ctid != 0 && ctid != cur {
                    ATTACHED_TID.store(ctid as u64, Ordering::SeqCst);
                    unsafe { AttachThreadInput(cur, ctid, 1); }
                    return;
                }
            }
        }
    }

    unsafe fn handle_child_mouse_down(container: HWND, button_msg: u32) {
        PANE_PRESS_AT.store(mono_ms(), Ordering::SeqCst);
        let already = unsafe { in_child_focus(container) };
        FOCUS_ESCAPED.store(false, Ordering::SeqCst);
        CHILD_FOCUSED.store(true, Ordering::SeqCst);
        unsafe { attach_visible_child_queue(container); }
        if button_msg == WM_RBUTTONDOWN || already {
            if let Ok(mut q) = events_q().lock() { q.push(Ev::Focus(0)); }
            return;
        }
        let now = mono_ms();
        if now >= CLICK_GRACE_UNTIL.load(Ordering::SeqCst) {
            CLICK_GRACE_UNTIL.store(now + 500, Ordering::SeqCst);
            if let Ok(mut q) = events_q().lock() { q.push(Ev::Click); }
        }
    }

    unsafe extern "system" fn wndproc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_BP_FOCUS => {
                // `bp.8` casts the 32-bit kernel HWND handle through `long` (`i32`) on Win64; zero-extend via `u32`.
                let fw = (lparam as u32) as u64;
                let back = HANDED_BACK_AT.load(Ordering::SeqCst);
                let since = mono_ms().saturating_sub(back);
                if since < 1000 && PANE_PRESS_AT.load(Ordering::SeqCst) < back {
                    if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() {
                        eprintln!("GPUI_LOG {}", serde_json::json!({"kicad_focus_notice_ignored": {"frame": fw, "ms_after_hand_back": since}}));
                    }
                    return 0;
                }
                FOCUS_ESCAPED.store(false, Ordering::SeqCst);
                CHILD_FOCUSED.store(true, Ordering::SeqCst);
                if fw != 0 {
                    let ctid = unsafe { GetWindowThreadProcessId(hwnd(fw), null_mut()) };
                    let cur = unsafe { GetCurrentThreadId() };
                    if ctid != 0 && ctid != cur {
                        ATTACHED_TID.store(ctid as u64, Ordering::SeqCst);
                        unsafe { AttachThreadInput(cur, ctid, 1); }
                    }
                }
                if let Ok(mut q) = events_q().lock() { q.push(Ev::Focus(fw)); }
                0
            }
            WM_MOUSEACTIVATE => {
                let mouse_msg = ((lparam as usize) >> 16) as u32 & 0xffff;
                if matches!(mouse_msg, WM_LBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDOWN) {
                    unsafe { handle_child_mouse_down(h, mouse_msg); }
                }
                unsafe { DefWindowProcW(h, msg, wparam, lparam) }
            }
            WM_PARENTNOTIFY => {
                let ev = (wparam & 0xffff) as u32;
                if matches!(ev, WM_LBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDOWN) {
                    unsafe { handle_child_mouse_down(h, ev); }
                }
                unsafe { DefWindowProcW(h, msg, wparam, lparam) }
            }
            WM_BP_KEY => {
                if let Some(k) = super::reserved_chord_index(wparam) {
                    CHILD_FOCUSED.store(false, Ordering::SeqCst);
                    CLICK_GRACE_UNTIL.store(0, Ordering::SeqCst);
                    if let Ok(mut q) = events_q().lock() {
                        q.push(if k == "ctrl-`" { Ev::Escape } else { Ev::Key(k.to_string()) }); // i18n-ok: gpui keystroke
                    }
                }
                0
            }
            WM_HOTKEY if wparam as i32 == ESC_HOTKEY_ID => {
                CHILD_FOCUSED.store(false, Ordering::SeqCst);
                CLICK_GRACE_UNTIL.store(0, Ordering::SeqCst);
                if let Ok(mut q) = events_q().lock() { q.push(Ev::Escape); }
                0
            }
            _ => unsafe { DefWindowProcW(h, msg, wparam, lparam) },
        }
    }

    fn class_name() -> &'static [u16] {
        static CLS: OnceLock<Vec<u16>> = OnceLock::new();
        CLS.get_or_init(|| {
            let mut n: Vec<u16> = "BackplaneKicadContainer".encode_utf16().chain(std::iter::once(0)).collect(); // i18n-ok: Win32 window class name
            unsafe {
                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: 0,
                    lpfnWndProc: Some(wndproc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: null_mut(),
                    hIcon: null_mut(),
                    hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                    // #001023 in Win32 COLORREF (`0x00BBGGRR`)
                    hbrBackground: CreateSolidBrush(0x0023_1000),
                    lpszMenuName: null_mut(),
                    lpszClassName: n.as_mut_ptr(),
                    hIconSm: null_mut(),
                };
                RegisterClassExW(&wc);
            }
            n
        })
    }

    /// Ensure `WS_CLIPCHILDREN` on gpui's `parent` HWND and create (or reuse) the hidden child container HWND for `(parent, key)`.
    unsafe fn parking_parent() -> HWND {
        static P: AtomicU64 = AtomicU64::new(0);
        let cur = P.load(Ordering::SeqCst);
        if cur != 0 && unsafe { IsWindow(hwnd(cur)) } != 0 { return hwnd(cur) }
        let cls = class_name();
        let w = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                cls.as_ptr(),
                null_mut(),
                WS_POPUP | WS_CLIPCHILDREN,
                -32000, -32000, 1, 1,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if !w.is_null() { P.store(id_of(w), Ordering::SeqCst); }
        w
    }

    /// Create or reuse the `WS_CHILD` container HWND for `key`, moving it into `parent` (`SetParent` on GPUI's own thread)
    /// when `parent` changes (Pop out / Pop back in). Keeping one container per project avoids `DestroyWindow` on a closing
    /// pop-out window taking KiCad's `WS_CHILD` frames with it and keeps `bp.8`/`bp.9`'s `m_holder` valid.
    pub fn container_for(parent: u64, key: &str) -> Option<u64> {
        unsafe {
            let p = hwnd(parent);
            if p.is_null() || IsWindow(p) == 0 { return None }
            let st = GetWindowLongPtrW(p, GWL_STYLE);
            if (st & WS_CLIPCHILDREN as isize) == 0 {
                SetWindowLongPtrW(p, GWL_STYLE, st | WS_CLIPCHILDREN as isize);
            }
            if !key.is_empty() {
                if let Ok(mut map) = containers_map().lock() {
                    if let Some(&existing) = map.get(key) {
                        let eh = hwnd(existing);
                        if IsWindow(eh) != 0 {
                            if GetAncestor(eh, GA_PARENT) != p {
                                SetWindowRgn(eh, null_mut(), 0);
                                ShowWindowAsync(eh, SW_HIDE);
                                SetParent(eh, p);
                            }
                            CHILD_FOCUSED.store(false, Ordering::SeqCst);
                            ACTIVE_CONTAINER.store(existing, Ordering::SeqCst);
                            return Some(existing);
                        }
                        map.remove(key);
                    }
                }
            }
            let cls = class_name();
            let c = CreateWindowExW(
                0,
                cls.as_ptr(),
                null_mut(),
                WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                0, 0, 1, 1,
                p,
                null_mut(),
                null_mut(),
                null_mut(),
            );
            if c.is_null() { return None }
            let cid = id_of(c);
            CHILD_FOCUSED.store(false, Ordering::SeqCst);
            ACTIVE_CONTAINER.store(cid, Ordering::SeqCst);
            if !key.is_empty() {
                if let Ok(mut map) = containers_map().lock() {
                    map.insert(key.to_string(), cid);
                }
            }
            Some(cid)
        }
    }

    pub fn container(parent: u64) -> Option<u64> { container_for(parent, "") }

    /// Park `old` (when distinct from `new`) under an immortal off-screen host HWND so closing a pop-out window never destroys it.
    pub fn move_frames(old: u64, new: u64) {
        ACTIVE_CONTAINER.store(new, Ordering::SeqCst);
        if old == 0 || old == new { return }
        if let Ok(mut map) = containers_map().lock() {
            map.retain(|_, &mut v| v != old);
        }
        unsafe {
            let oh = hwnd(old);
            if !oh.is_null() && IsWindow(oh) != 0 {
                SetWindowRgn(oh, null_mut(), 0);
                ShowWindowAsync(oh, SW_HIDE);
                let park = parking_parent();
                if !park.is_null() && GetAncestor(oh, GA_PARENT) != park {
                    SetParent(oh, park);
                }
            }
        }
    }

    /// Hide and park `container` without ever calling `DestroyWindow` or `SetParent(NULL)` on KiCad's cross-process child frames.
    pub fn destroy(container: u64) {
        unregister_escape();
        CHILD_FOCUSED.store(false, Ordering::SeqCst);
        if ACTIVE_CONTAINER.load(Ordering::SeqCst) == container {
            ACTIVE_CONTAINER.store(0, Ordering::SeqCst);
        }
        unsafe {
            let c = hwnd(container);
            if !c.is_null() && IsWindow(c) != 0 {
                SetWindowRgn(c, null_mut(), 0);
                ShowWindowAsync(c, SW_HIDE);
            }
        }
    }

    /// Stock cross-process top-level adoption is never used on Windows (fork only).
    pub fn adopt(_pid: u32, _container: u64, _toplevel: u64) -> Vec<u64> { vec![] }
    /// Windows: `dialogs` already centres them over Backplane's window.
    pub fn centre_dialogs(_pid: u32, _over: R) -> Vec<String> { vec![] }

    /// Own and centre floating KiCad dialogs (`floating_dialog`: captioned, non-child top-levels of `pid`, never menus or tooltips) over
    /// Backplane's `toplevel` HWND asynchronously.
    pub fn dialogs(pid: u32, container: u64, toplevel: u64) {
        if pid == 0 || toplevel == 0 { return }
        struct Ctx { pid: u32, container: HWND, toplevel: HWND }
        unsafe extern "system" fn cb(w: HWND, l: LPARAM) -> i32 {
            let ctx = unsafe { &*(l as *const Ctx) };
            if w == ctx.toplevel || w == ctx.container { return 1 }
            let mut wpid = 0u32;
            unsafe { GetWindowThreadProcessId(w, &mut wpid); }
            if wpid != ctx.pid { return 1 }
            if unsafe { IsWindowVisible(w) } == 0 { return 1 }
            let st = unsafe { GetWindowLongPtrW(w, GWL_STYLE) } as u32;
            let mut cls = [0u16; 64];
            let cn = unsafe { GetClassNameW(w, cls.as_mut_ptr(), cls.len() as i32) }.max(0) as usize;
            if !super::floating_dialog(st, &String::from_utf16_lossy(&cls[..cn])) { return 1 }
            let wid = id_of(w);
            let cur_owner = unsafe { GetWindowLongPtrW(w, GWLP_HWNDPARENT) } as usize;
            if cur_owner != ctx.toplevel as usize {
                unsafe { SetWindowLongPtrW(w, GWLP_HWNDPARENT, ctx.toplevel as isize); }
            }
            let first_seen = centered_dialogs().lock().map_or(false, |mut s| s.insert(wid));
            if first_seen {
                let (mut tr, mut dr) = (RECT::default(), RECT::default());
                if unsafe { GetWindowRect(ctx.toplevel, &mut tr) != 0 && GetWindowRect(w, &mut dr) != 0 } {
                    let (dw, dh) = (dr.right - dr.left, dr.bottom - dr.top);
                    let x = tr.left + ((tr.right - tr.left) - dw) / 2;
                    let y = tr.top + ((tr.bottom - tr.top) - dh) / 2;
                    unsafe { SetWindowPos(w, null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS); }
                }
            }
            1
        }
        let ctx = Ctx { pid, container: hwnd(container), toplevel: hwnd(toplevel) };
        unsafe { EnumWindows(Some(cb), &ctx as *const Ctx as LPARAM); }
        if let Ok(mut set) = centered_dialogs().lock() {
            set.retain(|&wid| unsafe { IsWindow(hwnd(wid)) != 0 && IsWindowVisible(hwnd(wid)) != 0 });
        }
    }

    /// `WS_CHILD` frames inside `container` (direct children on bp.8/bp.9, or children of KiCad's `BackplaneKiCadEmbedHolder` on bp.11+),
    /// reading titles via `InternalGetWindowText` (kernel read, never sends `WM_GETTEXT`).
    pub fn frames(container: u64) -> Vec<Frame> {
        let c = hwnd(container);
        if c.is_null() || unsafe { IsWindow(c) } == 0 { return vec![] }
        struct Ctx { parent: HWND, holders: Vec<HWND>, out: Vec<Frame> }
        unsafe extern "system" fn cb(w: HWND, l: LPARAM) -> i32 {
            let ctx = unsafe { &mut *(l as *mut Ctx) };
            let p = unsafe { GetAncestor(w, GA_PARENT) };
            if p == ctx.parent {
                let mut cls = [0u16; 64];
                let cn = unsafe { GetClassNameW(w, cls.as_mut_ptr(), cls.len() as i32) };
                if cn > 0 && String::from_utf16_lossy(&cls[..cn as usize]) == "BackplaneKiCadEmbedHolder" { // i18n-ok: Win32 window class name
                    ctx.holders.push(w);
                    return 1;
                }
            } else if !ctx.holders.contains(&p) {
                return 1;
            }
            let mut buf = [0u16; 512];
            let n = unsafe { InternalGetWindowText(w, buf.as_mut_ptr(), buf.len() as i32) };
            let t = if n > 0 { String::from_utf16_lossy(&buf[..n as usize]) } else { String::new() };
            ctx.out.push(Frame { window: id_of(w), kind: super::kind_of_title(&t), title: t });
            1
        }
        let mut ctx = Ctx { parent: c, holders: Vec::new(), out: Vec::new() };
        unsafe { EnumChildWindows(c, Some(cb), &mut ctx as *mut Ctx as LPARAM); }
        ctx.out
    }

    /// Position/size/show `container` and its child frames strictly via `SWP_ASYNCWINDOWPOS` and `ShowWindowAsync`.
    /// bp.9's local map: toggle `SW_SHOWNA`/`SW_HIDE` asynchronously on frames KiCad has already shown once (`mine`).
    pub fn place_local(container: u64, r: Option<R>, show: Option<u64>, mine: &[u64], last_root: &mut (i32, i32)) {
        place(container, r, show, Some(mine), last_root);
        if r.is_none() { return }
        for &k in mine {
            let kh = hwnd(k);
            if !kh.is_null() && unsafe { IsWindow(kh) } != 0 {
                unsafe { ShowWindowAsync(kh, if Some(k) == show { SW_SHOWNA } else { SW_HIDE }); }
            }
        }
    }
    pub fn place(container: u64, r: Option<R>, show: Option<u64>, managed: Option<&[u64]>, last_root: &mut (i32, i32)) {
        let c = hwnd(container);
        if c.is_null() || unsafe { IsWindow(c) } == 0 { return }
        ACTIVE_CONTAINER.store(container, Ordering::SeqCst);
        let Some(r) = r else {
            unsafe { ShowWindowAsync(c, SW_HIDE); }
            return;
        };
        let (w, h) = (r.w.max(1), r.h.max(1));
        unsafe {
            SetWindowPos(c, null_mut(), r.x, r.y, w, h, SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
            for f in frames(container) {
                if managed.map_or(false, |m| !m.contains(&f.window)) { continue }
                let kh = hwnd(f.window);
                if Some(f.window) == show {
                    SetWindowPos(kh, null_mut(), 0, 0, w, h, SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
                    if managed.is_none() { ShowWindowAsync(kh, SW_SHOWNA); }
                } else if managed.is_none() {
                    ShowWindowAsync(kh, SW_HIDE);
                }
            }
            ShowWindowAsync(c, if show.is_some() { SW_SHOWNA } else { SW_HIDE });
            if show.is_some() { shield_follow(c); } else if SHIELD_ON.load(Ordering::SeqCst) { ShowWindowAsync(hwnd(SHIELD.load(Ordering::SeqCst)), SW_HIDE); }
            let mut rc = RECT::default();
            if GetWindowRect(c, &mut rc) != 0 { *last_root = (rc.left, rc.top); }
        }
    }

    /// Keep `show` sized to `(0, 0, w, h)` asynchronously (e.g. right after KiCad's `ShowFrame` reveals a frame at its default size).
    pub fn sync_root(container: u64, r: R, show: u64, last_root: &mut (i32, i32)) {
        let (c, s) = (hwnd(container), hwnd(show));
        if c.is_null() || s.is_null() { return }
        let (w, h) = (r.w.max(1), r.h.max(1));
        unsafe {
            if IsWindowVisible(s) != 0 {
                if let Some((sr, _)) = geometry(show) {
                    if (sr.x, sr.y, sr.w, sr.h) != (0, 0, w, h) {
                        SetWindowPos(s, null_mut(), 0, 0, w, h, SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS);
                    }
                }
            }
            let mut rc = RECT::default();
            if GetWindowRect(c, &mut rc) != 0 { *last_root = (rc.left, rc.top); }
        }
    }

    /// Punch overlay holes in `container` using `SetWindowRgn` (system owns the region handle once `SetWindowRgn` succeeds).
    pub fn holes(container: u64, keep: Option<&[R]>) {
        let c = hwnd(container);
        if c.is_null() || unsafe { IsWindow(c) } == 0 { return }
        unsafe {
            match keep {
                None => { SetWindowRgn(c, null_mut(), 1); }
                Some(rs) => {
                    let acc = CreateRectRgn(0, 0, 0, 0);
                    if acc.is_null() { return }
                    for r in rs {
                        let tmp = CreateRectRgn(r.x, r.y, r.x + r.w.max(0), r.y + r.h.max(0));
                        if !tmp.is_null() {
                            CombineRgn(acc, acc, tmp, RGN_OR);
                            DeleteObject(tmp);
                        }
                    }
                    if SetWindowRgn(c, acc, 1) == 0 {
                        DeleteObject(acc);
                    }
                }
            }
            shield_follow(c);
        }
    }

    unsafe fn focus_target_in_frame(frame: HWND) -> HWND {
        struct Best { best: HWND, area: i64 }
        unsafe extern "system" fn cb(w: HWND, l: LPARAM) -> i32 {
            let b = unsafe { &mut *(l as *mut Best) };
            if unsafe { IsWindowVisible(w) } == 0 { return 1 }
            // Pick the largest visible leaf child HWND (the GAL / 3D canvas), never an AUI container wxPanel
            // whose wxControlContainer redirects WM_SETFOCUS to its first toolbar Edit box.
            if !unsafe { GetWindow(w, GW_CHILD) }.is_null() { return 1 }
            let mut rc = RECT::default();
            if unsafe { GetWindowRect(w, &mut rc) } != 0 {
                let area = ((rc.right - rc.left).max(0) as i64) * ((rc.bottom - rc.top).max(0) as i64);
                if area > b.area { b.best = w; b.area = area; }
            }
            1
        }
        let mut best = Best { best: frame, area: 0 };
        unsafe { EnumChildWindows(frame, Some(cb), &mut best as *mut Best as LPARAM); }
        let mut pt = POINT::default();
        if unsafe { GetCursorPos(&mut pt) } != 0 {
            let hit = unsafe { WindowFromPoint(pt) };
            if !hit.is_null() && hit != frame && unsafe { IsChild(frame, hit) } != 0 {
                let mut cls = [0u16; 64];
                let n = unsafe { GetClassNameW(hit, cls.as_mut_ptr(), cls.len() as i32) };
                if n > 0 {
                    let name = String::from_utf16_lossy(&cls[..n as usize]);
                    if name.eq_ignore_ascii_case("Edit") || name.eq_ignore_ascii_case("ComboBox") { // i18n-ok: Win32 window class name
                        return hit;
                    }
                }
            }
        }
        best.best
    }

    /// Focus `w` without ever making a synchronous Win32 call into KiCad's thread:
    /// - When `w` is KiCad's child frame (`target_tid != cur`), attach the foreground input queue and `PostMessageW(target, WM_ACTIVATE, 1, 0)`
    ///   so `DefWindowProcW` on KiCad's own UI thread calls `::SetFocus` directly on the clicked canvas/control asynchronously. Posted before
    ///   the frame is visible it is lost (the pane re-posts it once shown: kicad_pane `refocus`).
    /// - When `w` is gpui's top-level window (`target_tid == cur`), detach thread input queues first so `SetFocus(h)` never sends synchronous
    ///   `WM_KILLFOCUS` into KiCad (staying instant even when KiCad is `NtSuspendProcess`-frozen and preserving KiCad's `m_lostFocus` state).
    pub fn focus(w: u64) { focus_with(w, false) }
    /// An explicit hand-back to gpui's window (Ctrl+` or another host chord pressed in KiCad, also while KiCad is frozen):
    /// as `focus`, and the host window is activated too. KiCad's frame can be the active foreground window, where `SetFocus`
    /// alone moves only our background queue's focus and the keys keep going to KiCad. Never used for the reclaims of an
    /// opening or a pointer leaving the pane: activating there took the foreground from the editor just opened.
    /// F5: the foreground window when it is one of KiCad's (`frame`'s process) that nobody can see: hidden, or parked
    /// off-screen (a pre-warmed editor: wx-shown, natively unmapped at -30000). Its id and title. A visible KiCad popup
    /// or dialog never matches. Window-state reads only, no message to KiCad.
    pub fn invisible_kicad_foreground(frame: u64) -> Option<(u64, String)> {
        unsafe {
            let fg = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
            let f = hwnd(frame);
            if fg.is_null() || f.is_null() || fg == f || IsChild(fg, f) != 0 || IsChild(f, fg) != 0 { return None }
            let (mut a, mut b) = (0u32, 0u32);
            GetWindowThreadProcessId(fg, &mut a);
            GetWindowThreadProcessId(f, &mut b);
            if a == 0 || a != b { return None }
            let mut rc = RECT::default();
            let off = GetWindowRect(fg, &mut rc) != 0 && (rc.right <= -10000 || rc.bottom <= -10000);
            if IsWindowVisible(fg) != 0 && !off { return None }
            let mut t = [0u16; 160];
            let n = InternalGetWindowText(fg, t.as_mut_ptr(), t.len() as i32);
            Some((id_of(fg), String::from_utf16_lossy(&t[..n.max(0) as usize])))
        }
    }
    pub fn hand_back(w: u64) { HANDED_BACK_AT.store(mono_ms().max(1), Ordering::SeqCst); focus_with(w, true) }
    /// Whether `frame` and `container` are both visible (each with every ancestor): a read of window state, no message to KiCad.
    pub fn frame_visible(container: u64, frame: u64) -> bool {
        let (c, f) = (hwnd(container), hwnd(frame));
        !c.is_null() && !f.is_null() && unsafe { IsWindowVisible(c) != 0 && IsWindowVisible(f) != 0 }
    }
    fn focus_with(w: u64, activate: bool) {
        let h = hwnd(w);
        if h.is_null() { return }
        unsafe {
            let cur = GetCurrentThreadId();
            let target_tid = GetWindowThreadProcessId(h, null_mut());
            if target_tid != 0 && target_tid != cur {
                if let Ok(mut set) = handed_in().lock() { set.insert(w); }
                FOCUS_ESCAPED.store(false, Ordering::SeqCst);
                CHILD_FOCUSED.store(true, Ordering::SeqCst);
                CLICK_GRACE_UNTIL.store(mono_ms() + 500, Ordering::SeqCst);
                ATTACHED_TID.store(target_tid as u64, Ordering::SeqCst);
                AttachThreadInput(cur, target_tid, 1);
                let target = focus_target_in_frame(h);
                PostMessageW(target, WM_ACTIVATE, 1, 0);
                return;
            }
            CHILD_FOCUSED.store(false, Ordering::SeqCst);
            CLICK_GRACE_UNTIL.store(0, Ordering::SeqCst);
            let prev_tid = ATTACHED_TID.swap(0, Ordering::SeqCst) as u32;
            if prev_tid != 0 && prev_tid != cur {
                AttachThreadInput(prev_tid, cur, 0);
                AttachThreadInput(cur, prev_tid, 0);
            }
            let mut gti = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..std::mem::zeroed() };
            if GetGUIThreadInfo(0, &mut gti) != 0 && !gti.hwndFocus.is_null() {
                let focus_tid = GetWindowThreadProcessId(gti.hwndFocus, null_mut());
                if focus_tid != 0 && focus_tid != cur {
                    AttachThreadInput(focus_tid, cur, 0);
                    AttachThreadInput(cur, focus_tid, 0);
                }
            }
            // Activate only on an explicit hand-back, after detaching; never a foreign HWND here.
            if activate {
                let ok = SetForegroundWindow(h);
                if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() {
                    eprintln!("GPUI_LOG {}", serde_json::json!({"kicad_hand_back": {"window": w, "activated": ok != 0}}));
                }
            }
            SetFocus(h);
            let mut gti2 = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..std::mem::zeroed() };
            let escaped = GetGUIThreadInfo(0, &mut gti2) != 0
                && !gti2.hwndFocus.is_null()
                && GetWindowThreadProcessId(gti2.hwndFocus, null_mut()) != cur;
            FOCUS_ESCAPED.store(escaped, Ordering::SeqCst);
        }
    }

    pub fn conn_fd() -> Option<i32> { None }
    /// Windows sends keys to the focused window only: nothing to do.
    pub fn input_off(_container: u64, _off: bool) {}
    /// Whether the ';' key (VK_OEM_1 on a US layout) is down now: the comment chord held.
    pub fn semicolon_held() -> Option<bool> { Some(unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(0xBA) } as u16 & 0x8000 != 0) }
    pub fn focus_in_pid(_pid: u32) -> bool { false }
    /// Whether keyboard focus in the active foreground queue is inside `container`.
    pub fn has_focus(container: u64) -> bool {
        if FOCUS_ESCAPED.load(Ordering::SeqCst) { return false }
        let c = hwnd(container);
        if c.is_null() || unsafe { IsWindow(c) } == 0 { return false }
        unsafe {
            if in_child_focus(c) {
                // only during a frame's cold construction: a frame Backplane has handed the keys to is past it
                let mut gti = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..std::mem::zeroed() };
                let focused = if GetGUIThreadInfo(0, &mut gti) != 0 { gti.hwndFocus } else { null_mut() };
                let handed = handed_in().lock().map(|set| frames(container).iter().any(|f| set.contains(&f.window)
                    && (hwnd(f.window) == focused || IsChild(hwnd(f.window), focused) != 0))).unwrap_or(false);
                // nor for a frame nobody can see: a pre-warmed editor (hidden, or parked off-screen) that took the focus is the
                // F5 guard's to hand back to the shown editor; reclaiming it to gpui's window gave Backplane KiCad's keys with
                // nobody clicking (runs S9o, E14: native_child focus(top) right after the guard's re-post)
                let in_unseen_frame = frames(container).iter().map(|f| hwnd(f.window)).any(|fw| {
                    (fw == focused || IsChild(fw, focused) != 0) && {
                        let mut rc = RECT::default();
                        IsWindowVisible(fw) == 0 || (GetWindowRect(fw, &mut rc) != 0 && (rc.right <= -10000 || rc.bottom <= -10000))
                    }
                });
                if !CHILD_FOCUSED.load(Ordering::SeqCst) && !handed && !in_unseen_frame {
                    // wxMSW calls ::SetFocus(canvasHwnd) during cold frame construction even when ShowFrame(focus=false);
                    // reclaim focus back to GPUI's top-level HWND so opening an editor never steals the keyboard before a click.
                    let top = GetAncestor(c, GA_PARENT);
                    if !top.is_null() { focus(id_of(top)); }
                    return false;
                }
                return true;
            }
            if CHILD_FOCUSED.load(Ordering::SeqCst) && mono_ms() < CLICK_GRACE_UNTIL.load(Ordering::SeqCst) {
                return true;
            }
            let mut gti = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..std::mem::zeroed() };
            if GetGUIThreadInfo(0, &mut gti) != 0 {
                let top = GetAncestor(c, GA_PARENT);
                if !top.is_null() && gti.hwndFocus == top {
                    CHILD_FOCUSED.store(false, Ordering::SeqCst);
                }
            }
            false
        }
    }

    /// When a mouse button is down inside Backplane's top-level window outside `container` (including inside a `SetWindowRgn` hole),
    /// return `Some(false)` so gpui reclaims keyboard focus. Left/middle clicks inside `container` return `Some(true)`; dialogs stay untouched.
    pub fn button_over(w: u64) -> Option<bool> {
        unsafe {
            let l_or_m = [0x01, 0x04].iter().any(|k| (GetAsyncKeyState(*k) as u16 & 0x8000) != 0);
            let r_down = (GetAsyncKeyState(0x02) as u16 & 0x8000) != 0;
            if !l_or_m && !r_down { return None }
            let mut pt = POINT::default();
            if GetCursorPos(&mut pt) == 0 { return None }
            let hit = WindowFromPoint(pt);
            if hit.is_null() { return None }
            let top = GetAncestor(hwnd(w), GA_ROOT);
            if top.is_null() { return None }
            let mut c = GetAncestor(hwnd(w), GA_PARENT);
            while !c.is_null() && c != top {
                let next = GetAncestor(c, GA_PARENT);
                if next == top { break }
                c = next;
            }
            if c.is_null() || c == top { return None }
            let in_cont = hit == c || IsChild(c, hit) != 0;
            let in_top = hit == top || IsChild(top, hit) != 0;
            if in_cont {
                if l_or_m { Some(true) } else { None }
            } else if in_top {
                CHILD_FOCUSED.store(false, Ordering::SeqCst);
                CLICK_GRACE_UNTIL.store(0, Ordering::SeqCst);
                Some(false)
            } else {
                None
            }
        }
    }

    pub fn grab_click(_container: u64, _on: bool) {}
    /// F3: while Backplane has the keys, a never-painting input shield over the pane (a child of Backplane's own top-level,
    /// on its own thread, above the KiCad container, with the container's holes). A click on the pane then lands on
    /// Backplane's own window, so Backplane keeps the foreground by construction (a click on KiCad's window left no foreground
    /// window, runs F1/Hc: Windows dropped activation at the press, and SetForegroundWindow from Backplane was refused,
    /// run Hd); the shield hides itself and the pane hands KiCad the keys. It never touches KiCad's thread. Stage 1: that
    /// first click is consumed (it gives the keys; it does not reach KiCad).
    static SHIELD: AtomicU64 = AtomicU64::new(0);
    static SHIELD_ON: AtomicBool = AtomicBool::new(false);
    fn shield_class() -> &'static [u16] {
        static CLS: OnceLock<Vec<u16>> = OnceLock::new();
        CLS.get_or_init(|| {
            let mut n: Vec<u16> = "BackplaneKicadShield".encode_utf16().chain(std::iter::once(0)).collect(); // i18n-ok: Win32 window class name
            unsafe {
                let wc = WNDCLASSEXW { cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32, style: 0, lpfnWndProc: Some(shield_proc), cbClsExtra: 0,
                    cbWndExtra: 0, hInstance: null_mut(), hIcon: null_mut(), hCursor: LoadCursorW(null_mut(), IDC_ARROW), hbrBackground: null_mut(),
                    lpszMenuName: null_mut(), lpszClassName: n.as_mut_ptr(), hIconSm: null_mut() };
                RegisterClassExW(&wc);
            }
            n
        })
    }
    unsafe extern "system" fn shield_proc(h: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        use windows_sys::Win32::UI::WindowsAndMessaging::{WM_PAINT, WM_ERASEBKGND};
        match msg {
            WM_LBUTTONDOWN | WM_MBUTTONDOWN | WM_RBUTTONDOWN => {
                SHIELD_ON.store(false, Ordering::SeqCst);
                unsafe { ShowWindowAsync(h, SW_HIDE); }
                PANE_PRESS_AT.store(mono_ms(), Ordering::SeqCst);
                if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() {
                    let fg = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
                    eprintln!("GPUI_LOG {}", serde_json::json!({"kicad_shield_click": {"button": msg, "foreground": id_of(fg)}}));
                }
                if let Ok(mut q) = events_q().lock() { q.push(Ev::Click); }
                0
            }
            WM_PAINT => { unsafe { windows_sys::Win32::Graphics::Gdi::ValidateRect(h, std::ptr::null()); } 0 }
            WM_ERASEBKGND => 1,
            _ => unsafe { DefWindowProcW(h, msg, wparam, lparam) },
        }
    }
    /// The shield over `container` (same rect in the parent, same holes, just above it), when on.
    unsafe fn shield_follow(container: HWND) {
        if !SHIELD_ON.load(Ordering::SeqCst) { return }
        let s = hwnd(SHIELD.load(Ordering::SeqCst));
        if s.is_null() || container.is_null() || unsafe { IsWindow(s) } == 0 { return }
        let parent = unsafe { GetAncestor(container, GA_PARENT) };
        let mut rc = RECT::default();
        if unsafe { GetWindowRect(container, &mut rc) } == 0 { return }
        let mut tl = POINT { x: rc.left, y: rc.top };
        unsafe { windows_sys::Win32::Graphics::Gdi::ScreenToClient(parent, &mut tl); }
        let (w, hh) = (rc.right - rc.left, rc.bottom - rc.top);
        unsafe {
            // the container's holes (gpui's overlays drawn over the pane) stay clickable through the shield too
            let rgn = CreateRectRgn(0, 0, 0, 0);
            let kind = windows_sys::Win32::Graphics::Gdi::GetWindowRgn(container, rgn);
            if kind == 0 /* ERROR: no region */ { DeleteObject(rgn); SetWindowRgn(s, null_mut(), 0); } else if SetWindowRgn(s, rgn, 0) == 0 { DeleteObject(rgn); }
            // just above the container: inserted after the window above it (or at the top)
            let above = GetWindow(container, 3 /* GW_HWNDPREV */);
            let after = if above.is_null() || above == s { null_mut() /* HWND_TOP */ } else { above };
            SetWindowPos(s, after, tl.x, tl.y, w, hh, SWP_NOACTIVATE | 0x0040 /* SWP_SHOWWINDOW */);
        }
    }
    /// Debug: what has_focus sees (foreground, the foreground queue's focus and its class, the hand-in flags).
    pub fn focus_debug(container: u64) -> serde_json::Value {
        unsafe {
            let c = hwnd(container);
            let mut g = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..std::mem::zeroed() };
            let ok = GetGUIThreadInfo(0, &mut g) != 0;
            let mut cls = [0u16; 64];
            let k = if ok && !g.hwndFocus.is_null() { GetClassNameW(g.hwndFocus, cls.as_mut_ptr(), 64) } else { 0 };
            serde_json::json!({"foreground": id_of(windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow()),
                "focus": if ok { id_of(g.hwndFocus) } else { 0 }, "focus_class": String::from_utf16_lossy(&cls[..k.max(0) as usize]),
                "in_child": in_child_focus(c), "child_focused": CHILD_FOCUSED.load(Ordering::SeqCst), "escaped": FOCUS_ESCAPED.load(Ordering::SeqCst),
                "grace_left_ms": CLICK_GRACE_UNTIL.load(Ordering::SeqCst).saturating_sub(mono_ms())})
        }
    }
    static SHIELD_STABLE: AtomicU64 = AtomicU64::new(0);
    /// Windows' own answer, a read with no message to KiCad: gpui's thread owns the foreground window and the foreground
    /// queue's focus is not inside the KiCad container.
    pub fn keys_really_backplanes(container: u64) -> bool { keys_really_ours(hwnd(container)) }
    fn keys_really_ours(container: HWND) -> bool {
        unsafe {
            let fg = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
            if fg.is_null() || GetWindowThreadProcessId(fg, null_mut()) != GetCurrentThreadId() { return false }
            let mut g = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..std::mem::zeroed() };
            if GetGUIThreadInfo(0, &mut g) == 0 { return false }
            g.hwndFocus.is_null() || (g.hwndFocus != container && IsChild(container, g.hwndFocus) == 0)
        }
    }
    /// KiCad's shown editor really has the keys: the foreground queue's focus is inside `shown` (the editor on show), not
    /// merely inside the container, which also holds the hidden pre-warmed frames: focus in one of those (a pre-warm steal)
    /// is not "KiCad has the keys", the same reading as the F5 guard (a read, no message to KiCad).
    fn kicad_has_keys(shown: HWND) -> bool {
        if shown.is_null() { return false }
        unsafe {
            let mut g = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..std::mem::zeroed() };
            GetGUIThreadInfo(0, &mut g) != 0 && !g.hwndFocus.is_null() && (g.hwndFocus == shown || IsChild(shown, g.hwndFocus) != 0)
        }
    }
    /// Each pane tick (40 ms), with the pane on show in fork mode (`enabled`): the shield is down exactly while KiCad really
    /// has the keys (dropped at once when focus is seen inside KiCad) and up otherwise, after two ticks of agreement:
    /// Backplane having them, or another application in front (run AW1, case 3: back from Notepad by a click straight on
    /// the canvas left no foreground window, F3 by another route). A click on it is then always a click on Backplane's own
    /// window, which Windows activates; the pane's flag never decides it, so a wrong flag cannot eat a click.
    pub fn shield_tick(container: u64, shown: Option<u64>) {
        let c = hwnd(container);
        if c.is_null() || unsafe { IsWindow(c) } == 0 { return }
        let up = shown.is_some() && unsafe { IsWindowVisible(c) } != 0 && !kicad_has_keys(hwnd(shown.unwrap_or(0)));
        if up {
            let n = SHIELD_STABLE.fetch_add(1, Ordering::SeqCst) + 1;
            if n >= 2 && !SHIELD_ON.load(Ordering::SeqCst) { shield_set(container, true); }
        } else {
            SHIELD_STABLE.store(0, Ordering::SeqCst);
            if SHIELD_ON.load(Ordering::SeqCst) { shield_set(container, false); }
        }
    }
    /// The pane gave KiCad the keys (`on` false): the shield drops at once; raising it is `shield_tick`'s alone.
    pub fn shield(container: u64, on: bool) {
        if !on { SHIELD_STABLE.store(0, Ordering::SeqCst); shield_set(container, false); }
    }
    fn shield_set(container: u64, on: bool) {
        let c = hwnd(container);
        if c.is_null() || unsafe { IsWindow(c) } == 0 { return }
        let was = SHIELD_ON.swap(on, Ordering::SeqCst);
        unsafe {
            let parent = GetAncestor(c, GA_PARENT);
            let mut s = hwnd(SHIELD.load(Ordering::SeqCst));
            if on && (s.is_null() || IsWindow(s) == 0 || GetAncestor(s, GA_PARENT) != parent) {
                if !s.is_null() && IsWindow(s) != 0 { windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(s); }
                s = CreateWindowExW(windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_LAYERED | WS_EX_NOACTIVATE, shield_class().as_ptr(), null_mut(),
                    WS_CHILD | WS_CLIPSIBLINGS, 0, 0, 1, 1, parent, null_mut(), null_mut(), null_mut());
                if s.is_null() { SHIELD_ON.store(false, Ordering::SeqCst); return }
                // alpha 1 of 255: nothing seen, every click still hits it
                windows_sys::Win32::UI::WindowsAndMessaging::SetLayeredWindowAttributes(s, 0, 1, windows_sys::Win32::UI::WindowsAndMessaging::LWA_ALPHA);
                SHIELD.store(id_of(s), Ordering::SeqCst);
            }
            if on { shield_follow(c); } else if !s.is_null() && was { ShowWindowAsync(s, SW_HIDE); }
        }
        if std::env::var_os("BACKPLANE_KICAD_DEBUG").is_some() && was != on {
            eprintln!("GPUI_LOG {}", serde_json::json!({"kicad_shield": on}));
        }
    }


    fn unregister_escape() {
        let prev = HOTKEY_HWND.swap(0, Ordering::SeqCst);
        if prev != 0 {
            unsafe { UnregisterHotKey(hwnd(prev), ESC_HOTKEY_ID); }
        }
    }

    /// Register `Ctrl+`` on the active container while KiCad holds keyboard focus so `WM_HOTKEY` rescues focus even if KiCad's UI thread is hung.
    pub fn grab_escape(_top: u64, on: bool) {
        if !on {
            unregister_escape();
            return;
        }
        let c = ACTIVE_CONTAINER.load(Ordering::SeqCst);
        if c == 0 { return }
        let cur = HOTKEY_HWND.load(Ordering::SeqCst);
        if cur == c { return }
        unregister_escape();
        unsafe {
            let scan = VkKeyScanW('`' as u16);
            let vk = if scan != -1 && ((scan as u16) & 0xff00) == 0 { ((scan as u16) & 0xff) as u32 } else { VK_OEM_3 as u32 };
            if RegisterHotKey(hwnd(c), ESC_HOTKEY_ID, MOD_CONTROL | MOD_NOREPEAT, vk) != 0 {
                HOTKEY_HWND.store(c, Ordering::SeqCst);
            }
        }
    }

    pub fn events() -> Vec<Ev> {
        events_q().lock().map_or_else(|_| Vec::new(), |mut q| std::mem::take(&mut *q))
    }

    pub fn poke(_w: u64, _keysym: u32, _mods: u16) {}
    pub fn keep(_holder: u64) {}

    fn geometry(w: u64) -> Option<(R, bool)> {
        let h = hwnd(w);
        if h.is_null() || unsafe { IsWindow(h) } == 0 { return None }
        unsafe {
            let mut rc = RECT::default();
            if GetWindowRect(h, &mut rc) == 0 { return None }
            let parent = GetAncestor(h, GA_PARENT);
            let mut pt = POINT { x: rc.left, y: rc.top };
            if !parent.is_null() { ScreenToClient(parent, &mut pt); }
            Some((R { x: pt.x, y: pt.y, w: rc.right - rc.left, h: rc.bottom - rc.top }, IsWindowVisible(h) != 0))
        }
    }

    fn sample_painted(h: HWND) -> usize {
        unsafe {
            let mut rc = RECT::default();
            if GetWindowRect(h, &mut rc) == 0 { return 0 }
            let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            let x0 = rc.left.max(vx);
            let y0 = rc.top.max(vy);
            let x1 = rc.right.min(vx + vw);
            let y1 = rc.bottom.min(vy + vh);
            let (w, ht) = (x1 - x0, y1 - y0);
            if w <= 4 || ht <= 4 { return 0 }
            let sdc = GetDC(null_mut());
            if sdc.is_null() { return 0 }
            let mdc = CreateCompatibleDC(sdc);
            let bmp = CreateCompatibleBitmap(sdc, w, ht);
            if mdc.is_null() || bmp.is_null() {
                if !bmp.is_null() { DeleteObject(bmp); }
                if !mdc.is_null() { DeleteDC(mdc); }
                ReleaseDC(null_mut(), sdc);
                return 0;
            }
            let old = SelectObject(mdc, bmp);
            let ok = BitBlt(mdc, 0, 0, w, ht, sdc, x0, y0, SRCCOPY) != 0;
            let mut lit = 0usize;
            if ok {
                let mut bi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: w,
                        biHeight: -ht,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB,
                        ..std::mem::zeroed()
                    },
                    ..std::mem::zeroed()
                };
                let mut px = vec![0u8; (w as usize) * (ht as usize) * 4];
                let lines = GetDIBits(mdc, bmp, 0, ht as u32, px.as_mut_ptr() as *mut _, &mut bi, DIB_RGB_COLORS);
                if lines > 0 {
                    let mut buckets = HashSet::new();
                    for p in px.chunks(4).step_by(97) {
                        if p.len() < 3 { continue }
                        let (b, g, r) = (p[0], p[1], p[2]);
                        let is_placeholder = r <= 12 && (8..=28).contains(&g) && (20..=52).contains(&b);
                        if (b > 24 || g > 24 || r > 24) && !is_placeholder {
                            lit += 1;
                            buckets.insert((b >> 4, g >> 4, r >> 4));
                        }
                    }
                    if buckets.len() < 4 { lit = 0; }
                }
            }
            SelectObject(mdc, old);
            DeleteObject(bmp);
            DeleteDC(mdc);
            ReleaseDC(null_mut(), sdc);
            lit
        }
    }

    pub fn report(w: u64) -> Option<(R, bool, usize)> {
        let (r, vis) = geometry(w)?;
        let painted = if vis { sample_painted(hwnd(w)) } else { 0 };
        Some((r, vis, painted))
    }

    pub fn window_meta(w: u64, container: Option<u64>, toplevel: u64) -> Option<(u8, (i32, i32), usize, bool)> {
        let h = hwnd(w);
        if h.is_null() || unsafe { IsWindow(h) } == 0 { return None }
        unsafe {
            let mut rc = RECT::default();
            if GetWindowRect(h, &mut rc) == 0 { return None }
            let rgn = CreateRectRgn(0, 0, 0, 0);
            let shape_rects = if !rgn.is_null() {
                let kind = GetWindowRgn(h, rgn);
                let n = if kind == 0 {
                    1
                } else {
                    let sz = GetRegionData(rgn, 0, null_mut());
                    if sz >= std::mem::size_of::<RGNDATA>() as u32 {
                        let mut buf = vec![0u8; sz as usize];
                        if GetRegionData(rgn, sz, buf.as_mut_ptr() as *mut RGNDATA) != 0 {
                            (*(buf.as_ptr() as *const RGNDATA)).rdh.nCount as usize
                        } else { 1 }
                    } else { 0 }
                };
                DeleteObject(rgn);
                n
            } else { 1 };

            let mut saw_cont = container.map_or(true, |ct| ct == w);
            let mut saw_top = toplevel == 0 || w == toplevel;
            let mut cur = h;
            for _ in 0..16 {
                if saw_cont && saw_top { break }
                if cur.is_null() { break }
                if (GetWindowLongPtrW(cur, GWL_STYLE) as u32 & WS_CHILD) == 0 { break }
                cur = GetAncestor(cur, GA_PARENT);
                let cid = id_of(cur);
                if Some(cid) == container { saw_cont = true; }
                if cid == toplevel { saw_top = true; }
            }
            // Depth bit encodes `WS_CLIPCHILDREN` check readiness (`32` on DWM; `toplevel` carries `WS_CLIPCHILDREN`).
            Some((32, (rc.left, rc.top), shape_rects, saw_cont && saw_top))
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod imp {
    use super::{Frame, R};
    pub fn container(_parent: u64) -> Option<u64> { None }
    pub fn container_for(_parent: u64, _key: &str) -> Option<u64> { None }
    pub fn move_frames(_old: u64, _new: u64) {}
    pub fn destroy(_container: u64) {}
    pub fn adopt(_pid: u32, _container: u64, _toplevel: u64) -> Vec<u64> { vec![] }
    pub fn dialogs(_pid: u32, _container: u64, _toplevel: u64) {}
    pub fn centre_dialogs(_pid: u32, _over: super::R) -> Vec<String> { vec![] }
    pub fn frames(_container: u64) -> Vec<Frame> { vec![] }
    pub fn place(_container: u64, _r: Option<R>, _show: Option<u64>, _managed: Option<&[u64]>, _last_root: &mut (i32, i32)) {}
    pub fn place_local(_container: u64, _r: Option<R>, _show: Option<u64>, _mine: &[u64], _last_root: &mut (i32, i32)) {}
    pub fn sync_root(_container: u64, _r: R, _show: u64, _last_root: &mut (i32, i32)) {}
    pub fn holes(_container: u64, _keep: Option<&[R]>) {}
    pub fn focus(_w: u64) {}
    pub fn hand_back(_w: u64) {}
    pub fn shield(_container: u64, _on: bool) {}
    pub fn shield_tick(_container: u64, _shown: Option<u64>) {}
    pub fn keys_really_backplanes(_container: u64) -> bool { true }
    pub fn focus_debug(_container: u64) -> serde_json::Value { serde_json::Value::Null }
    pub fn invisible_kicad_foreground(_frame: u64) -> Option<(u64, String)> { None }
    pub fn frame_visible(_container: u64, _frame: u64) -> bool { true }
    pub fn conn_fd() -> Option<i32> { None }
    pub fn pump() -> bool { false }
    pub fn input_off(_container: u64, _off: bool) {}
    /// Unknown here: the window's own key-up decides.
    pub fn semicolon_held() -> Option<bool> { None }
    pub fn has_focus(_container: u64) -> bool { false }
    pub fn focus_in_pid(_pid: u32) -> bool { false }
    pub fn button_over(_w: u64) -> Option<bool> { None }
    pub fn grab_click(_container: u64, _on: bool) {}

    pub fn grab_escape(_top: u64, _on: bool) {}
    pub fn events() -> Vec<super::Ev> { vec![] }
    pub fn poke(_w: u64, _keysym: u32, _mods: u16) {}
    pub fn keep(_holder: u64) {}
    pub fn report(_w: u64) -> Option<(R, bool, usize)> { None }
    pub fn window_meta(_w: u64, _container: Option<u64>, _toplevel: u64) -> Option<(u8, (i32, i32), usize, bool)> { None }
}

pub use imp::*;

#[cfg(test)]
mod tests {
    use super::*;
    /// KiCad's right-click menu (WS_POPUP|WS_VISIBLE|WS_CLIPSIBLINGS|WS_BORDER, class #32768) was taken for a dialog and re-centred.
    #[test] fn only_captioned_top_levels_are_dialogs_and_menus_never_are() {
        let dialog = 0x94C8_0000; // WS_POPUP|WS_VISIBLE|WS_CLIPSIBLINGS|WS_CAPTION|WS_SYSMENU
        assert!(floating_dialog(dialog, "#32770") && floating_dialog(dialog, "wxWindowNR"));
        assert!(!floating_dialog(0x9480_0000, "#32768"), "a popup menu: one caption bit");
        assert!(!floating_dialog(0x9480_0000, "wxWindowNR"), "a bordered wx popup: one caption bit");
        assert!(!floating_dialog(dialog, "#32768") && !floating_dialog(dialog, "tooltips_class32"), "menus and tooltips by class");
        assert!(!floating_dialog(dialog | 0x4000_0000, "wxWindowNR"), "a child");
    }
    #[test] fn titles_name_their_editor() {
        assert_eq!(kind_of_title("pic_programmer — PCB Editor"), "PCB");
        assert_eq!(kind_of_title("*ecc83-pp — Schematic Editor"), "SCH");
        assert_eq!(kind_of_title("3D Viewer"), "PCB_3D");
        assert_eq!(kind_of_title("Device:R — Symbol Editor"), "SYMBOL_EDITOR");
        assert_eq!(kind_of_title("Hubble_DK-F_Cu.gtl (with X2 attributes) — Gerber Viewer"), "GERBVIEW");
        assert_eq!(kind_of_title("lib:R_0603 — Footprint Editor"), "FOOTPRINT_EDITOR");
        assert_eq!(kind_of_title("KiCad 10.0 — ecc83-pp"), "MANAGER");
        assert!(dirty_title("*ecc83-pp — Schematic Editor") && !dirty_title("ecc83-pp — PCB Editor"));
    }
    #[test] fn reserved_chords_become_gpui_keystrokes() {
        assert_eq!(chord(0x6b, 4).as_deref(), Some("ctrl-k"));
        assert_eq!(chord(0xff09, 5).as_deref(), Some("ctrl-shift-tab"));
        assert_eq!(chord(0xfe20, 5).as_deref(), Some("ctrl-shift-tab"), "ISO_Left_Tab, as GTK names Shift+Tab");
        assert_eq!(chord(0x60, 4).as_deref(), Some("ctrl-`"));
        assert_eq!(chord(0xffbe, 4), None, "F1: not a reserved key");
        assert_eq!(reserved_chord_index(0), Some("ctrl-k"));
        assert_eq!(reserved_chord_index(3), Some("ctrl-shift-tab"));
        assert_eq!(reserved_chord_index(4), Some("ctrl-`")); assert_eq!(reserved_chord_index(5), Some("ctrl-;")); assert_eq!(reserved_chord_index(6), Some("alt-shift-b")); assert_eq!(reserved_chord_index(63), Some(";")); assert_eq!(reserved_chord_index(9), Some("alt-shift-f")); assert_eq!(reserved_chord_index(10), Some(";")); assert_eq!(chord(0x3b, 0).as_deref(), Some(";")); assert_eq!(chord(0x3b, 4).as_deref(), Some("ctrl-;"));
        assert_eq!(reserved_chord_index(11), None);
        assert_eq!(chord(0x42, 8 | 1).as_deref(), Some("alt-shift-b"), "X11: Shift+B is keysym B");
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

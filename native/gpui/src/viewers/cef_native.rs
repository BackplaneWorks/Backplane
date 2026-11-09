//! The windowed Browser tab's native side (viewers/cef.rs, feature `cef`): Chromium draws into a native child window of the client's own window
//! (an X11 child window, a child HWND, an NSView subview), and these are the few calls the page makes on it: place it over the viewer pane, show
//! or hide it, give the keyboard back to the client's window, and read a few of its pixels for the e2e report.
//! Coordinates are the parent's: physical pixels from its top-left on X11 and Windows, points on macOS (converted here to the view's own origin).
#![allow(dead_code)]

/// The client window CEF's browsers are made children of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Host { X11(u32), Win(isize), Mac(usize) }
unsafe impl Send for Host {}
unsafe impl Sync for Host {}

impl Host {
    /// The client's window as raw-window-handle names it, when windowed CEF can parent to it (None: Wayland or anything else, the page then renders off screen).
    pub fn of(h: raw_window_handle::RawWindowHandle) -> Option<Host> {
        use raw_window_handle::RawWindowHandle as R;
        match h {
            R::Xcb(x) => Some(Host::X11(x.window.get())),
            R::Xlib(x) => Some(Host::X11(x.window as u32)),
            R::Win32(w) => Some(Host::Win(w.hwnd.get())),
            R::AppKit(a) => Some(Host::Mac(a.ns_view.as_ptr() as usize)),
            _ => None,
        }
    }
    /// The value CEF's `WindowInfo::parent_window` takes on this platform.
    #[cfg(target_os = "linux")] pub fn raw(&self) -> std::os::raw::c_ulong { match *self { Host::X11(w) => w as _, _ => 0 } }
    #[cfg(windows)] pub fn raw(&self) -> isize { match *self { Host::Win(h) => h, _ => 0 } }
    #[cfg(target_os = "macos")] pub fn raw(&self) -> *mut std::ffi::c_void { match *self { Host::Mac(v) => v as _, _ => std::ptr::null_mut() } }
}

/// A rectangle in the parent's coordinates (see the module comment).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct R { pub x: i32, pub y: i32, pub w: i32, pub h: i32 }

#[cfg(target_os = "linux")]
mod imp {
    use super::{Host, R};
    use std::sync::{Mutex, OnceLock};
    use x11rb::{connection::Connection, protocol::xproto::*, rust_connection::RustConnection, wrapper::ConnectionExt as _};
    /// One connection of our own to the X server (gpui's is not ours to use; X serializes both).
    fn conn() -> Option<std::sync::MutexGuard<'static, Option<RustConnection>>> {
        static C: OnceLock<Mutex<Option<RustConnection>>> = OnceLock::new();
        let g = C.get_or_init(|| Mutex::new(match RustConnection::connect(None) { Ok((c, n)) => { SCREEN.store(n, std::sync::atomic::Ordering::Relaxed); Some(c) } Err(e) => { eprintln!("GPUI_LOG {{\"cef_x11\":{}}}", serde_json::Value::String(e.to_string())); None } })).lock().ok()?;
        g.is_some().then_some(g)
    }
    static SCREEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    /// A window of ours inside the client's, with the screen's default visual, that Chromium's is made a child of: gpui's window has a 32-bit
    /// (transparent) visual, and a child Chromium creates with its parent's visual and no colormap of its own is refused (BadMatch).
    pub fn container(h: Host) -> Option<Host> {
        let Host::X11(parent) = h else { return None }; let g = conn()?; let c = g.as_ref().unwrap();
        let scr = c.setup().roots.get(SCREEN.load(std::sync::atomic::Ordering::Relaxed))?;
        let id = c.generate_id().ok()?;
        c.create_window(scr.root_depth, id, parent, 0, 0, 1, 1, 0, WindowClass::INPUT_OUTPUT, scr.root_visual,
            &CreateWindowAux::new().background_pixel(scr.white_pixel).border_pixel(scr.black_pixel).colormap(scr.default_colormap)).ok()?.check().ok()?;
        let _ = c.flush(); Some(Host::X11(id))
    }
    pub fn close(_w: u64) {}
    /// Whether a mouse button is down now (a focus change while one is, is the person's click; without, a window manager's focus-follows-mouse).
    pub fn button_down() -> bool {
        let Some(g) = conn() else { return false }; let c = g.as_ref().unwrap();
        let Some(root) = c.setup().roots.get(SCREEN.load(std::sync::atomic::Ordering::Relaxed)).map(|s| s.root) else { return false };
        c.query_pointer(root).ok().and_then(|r| r.reply().ok()).map_or(false, |p| u16::from(p.mask) & (u16::from(KeyButMask::BUTTON1) | u16::from(KeyButMask::BUTTON2) | u16::from(KeyButMask::BUTTON3)) != 0)
    }
    /// The container gone (its browser closed).
    pub fn destroy(frame: u64) { let Some(g) = conn() else { return }; let c = g.as_ref().unwrap(); let _ = c.destroy_window(frame as u32); let _ = c.flush(); }
    /// The container (and Chromium's window filling it) over the pane at `r`, mapped; or unmapped.
    pub fn place_in(frame: u64, w: u64, r: Option<R>) {
        { let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
          if let Some(r) = r { let _ = c.configure_window(frame as u32, &ConfigureWindowAux::new().x(r.x).y(r.y).width(r.w.max(1) as u32).height(r.h.max(1) as u32));
                               if w != 0 { let _ = c.configure_window(w as u32, &ConfigureWindowAux::new().x(0).y(0).width(r.w.max(1) as u32).height(r.h.max(1) as u32)); } }
          let _ = if r.is_some() { c.map_window(frame as u32) } else { c.unmap_window(frame as u32) }; let _ = c.flush(); }
        if w != 0 { show(w, r.is_some()); }
    }
    fn atom(c: &RustConnection, n: &str) -> u32 { c.intern_atom(false, n.as_bytes()).ok().and_then(|r| r.reply().ok()).map_or(0, |r| r.atom) }
    /// Chromium's own window inside CEF's wrapper (the one that takes the keyboard), or the wrapper when it has none yet.
    fn inner(c: &RustConnection, w: u32) -> u32 { c.query_tree(w).ok().and_then(|r| r.reply().ok()).and_then(|t| t.children.first().copied()).unwrap_or(w) }
    pub fn place(w: u64, r: R) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let _ = c.configure_window(w as u32, &ConfigureWindowAux::new().x(r.x).y(r.y).width(r.w.max(1) as u32).height(r.h.max(1) as u32));
        let _ = c.flush();
    }
    /// Shown or hidden: mapped or not, and `_NET_WM_STATE_HIDDEN` set as well, which CEF's X11 wrapper passes on to Chromium's widget (an unmap alone
    /// leaves its compositor running; tools/cef/host.cc).
    pub fn show(w: u64, on: bool) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap(); let w = w as u32;
        let (state, hidden) = (atom(c, "_NET_WM_STATE"), atom(c, "_NET_WM_STATE_HIDDEN"));
        let mut atoms: Vec<u32> = c.get_property(false, w, state, AtomEnum::ATOM, 0, 1024).ok().and_then(|r| r.reply().ok()).and_then(|p| p.value32().map(|v| v.collect())).unwrap_or_default();
        atoms.retain(|a| *a != hidden); if !on { atoms.push(hidden); }
        let _ = c.change_property32(PropMode::REPLACE, w, state, AtomEnum::ATOM, &atoms);
        let _ = if on { c.map_window(w) } else { c.unmap_window(w) };
        let _ = c.flush();
    }
    /// The keyboard to the page: X input focus on Chromium's window (CEF's own SetFocus leaves it on the wrapper when a page is shown again; host.cc).
    pub fn focus_child(w: u64) {
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let t = inner(c, w as u32);
        let viewable = c.get_window_attributes(t).ok().and_then(|r| r.reply().ok()).map_or(false, |a| a.map_state == MapState::VIEWABLE);
        if viewable { let _ = c.set_input_focus(InputFocus::PARENT, t, x11rb::CURRENT_TIME); let _ = c.flush(); }
    }
    /// The keyboard back to the client's window.
    pub fn focus_host(h: Host) {
        let Host::X11(w) = h else { return }; let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let _ = c.set_input_focus(InputFocus::PARENT, w, x11rb::CURRENT_TIME); let _ = c.flush();
    }
    /// Where the child is (its geometry in the parent) and whether it is mapped: what the e2e checks.
    pub fn geometry(w: u64) -> Option<(R, bool)> {
        let g = conn()?; let c = g.as_ref().unwrap();
        let geo = match c.get_geometry(w as u32).map(|r| r.reply()) { Ok(Ok(g)) => g, e => { eprintln!("GPUI_LOG {{\"cef_x11_geometry\":{}}}", serde_json::Value::String(format!("{e:?}"))); return None } };
        let mapped = c.get_window_attributes(w as u32).ok()?.reply().ok()?.map_state == MapState::VIEWABLE;
        Some((R { x: geo.x as i32, y: geo.y as i32, w: geo.width as i32, h: geo.height as i32 }, mapped))
    }
    /// The pixel at (x, y) of Chromium's window as the X server has it (the e2e's click-to-pixel probe).
    pub fn pixel(w: u64, x: i32, y: i32) -> Option<[u8; 3]> {
        let g = conn()?; let c = g.as_ref().unwrap();
        let img = c.get_image(ImageFormat::Z_PIXMAP, inner(c, w as u32), x as i16, y as i16, 1, 1, !0).ok()?.reply().ok()?;
        (img.data.len() >= 3).then(|| [img.data[0], img.data[1], img.data[2]])
    }
    /// The size of Chromium's own widget inside CEF's window (it follows the window's size once Chromium has handled the change).
    pub fn inner_size(w: u64) -> Option<(i32, i32)> {
        let g = conn()?; let c = g.as_ref().unwrap();
        let geo = c.get_geometry(inner(c, w as u32)).ok()?.reply().ok()?; Some((geo.width as i32, geo.height as i32))
    }
    /// How many of the child's sampled pixels (every 97th) are not white: the page painted.
    pub fn painted(w: u64) -> usize {
        let Some(g) = conn() else { return 0 }; let c = g.as_ref().unwrap(); let w = w as u32;
        let Some(geo) = c.get_geometry(w).ok().and_then(|r| r.reply().ok()) else { return 0 };
        let Some(img) = c.get_image(ImageFormat::Z_PIXMAP, w, 0, 0, geo.width, geo.height, !0).ok().and_then(|r| r.reply().ok()) else { return 0 };
        if img.depth < 24 { return 0 }
        img.data.chunks(4).step_by(97).filter(|p| p.len() >= 3 && (p[0] < 240 || p[1] < 240 || p[2] < 240)).count()
    }
}

#[cfg(windows)]
mod imp {
    use super::{Host, R};
    pub fn pixel(_w: u64, _x: i32, _y: i32) -> Option<[u8; 3]> { None }
    pub fn inner_size(_w: u64) -> Option<(i32, i32)> { None }
    pub fn container(_h: Host) -> Option<Host> { None }
    pub fn place_in(_frame: u64, _w: u64, _r: Option<R>) {}
    pub fn destroy(_frame: u64) {}
    #[cfg(windows)] pub fn close(w: u64) { unsafe { windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(w as isize as _); } }
    #[cfg(target_os = "macos")] pub fn close(w: u64) { unsafe { send0(v(w), c"removeFromSuperview"); } }
    /// Every window under `h` (class, rect in the screen, visible, style), for the report.
    #[cfg(windows)] pub fn tree(h: Host) -> Vec<String> {
        use windows_sys::Win32::{Foundation::*, UI::WindowsAndMessaging::*};
        let Host::Win(p) = h else { return vec![] };
        unsafe extern "system" fn cb(w: HWND, l: LPARAM) -> i32 {
            let out = &mut *(l as *mut Vec<String>); let mut cls = [0u16; 64]; let n = GetClassNameW(w, cls.as_mut_ptr(), 64);
            let mut r = std::mem::zeroed::<RECT>(); GetWindowRect(w, &mut r);
            out.push(format!("{:x} {} [{},{} {}x{}] vis={} style={:x} ex={:x} parent={:x}", w as usize, String::from_utf16_lossy(&cls[..n.max(0) as usize]), r.left, r.top, r.right - r.left, r.bottom - r.top, // i18n-ok: a diagnostic in the e2e report
                IsWindowVisible(w), GetWindowLongPtrW(w, GWL_STYLE), GetWindowLongPtrW(w, GWL_EXSTYLE), GetParent(w) as usize)); 1 }
        let mut out: Vec<String> = Vec::new();
        unsafe { let mut r = std::mem::zeroed::<RECT>(); GetWindowRect(p as HWND, &mut r); out.push(format!("client {:x} [{},{} {}x{}] ex={:x}", p, r.left, r.top, r.right - r.left, r.bottom - r.top, GetWindowLongPtrW(p as HWND, GWL_EXSTYLE))); // i18n-ok: a diagnostic in the e2e report
            EnumChildWindows(p as HWND, Some(cb), &mut out as *mut _ as LPARAM); }
        out
    }
    #[cfg(windows)] pub fn button_down() -> bool { use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState; unsafe { [0x01, 0x02, 0x04].iter().any(|k| GetAsyncKeyState(*k) as u16 & 0x8000 != 0) } }
    #[cfg(target_os = "macos")] pub fn button_down() -> bool {
        #[link(name = "objc")] unsafe extern "C" { fn objc_getClass(n: *const std::ffi::c_char) -> *mut c_void; }
        unsafe { let cls = objc_getClass(c"NSEvent".as_ptr()); !cls.is_null() && !send0(cls, c"pressedMouseButtons").is_null() } // i18n-ok: an Objective-C class name
    }
    use windows_sys::Win32::{Foundation::HWND, Graphics::Gdi::{GetDC, GetPixel, ReleaseDC}, UI::{Input::KeyboardAndMouse::SetFocus, WindowsAndMessaging::*}};
    fn hwnd(w: u64) -> HWND { w as isize as HWND }
    /// Asynchronous: the child belongs to CEF's UI thread, and a synchronous call from here would wait on it.
    pub fn place(w: u64, r: R) { unsafe { SetWindowPos(hwnd(w), std::ptr::null_mut(), r.x, r.y, r.w.max(1), r.h.max(1), SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS); } }
    pub fn show(w: u64, on: bool) { unsafe { ShowWindowAsync(hwnd(w), if on { SW_SHOWNA } else { SW_HIDE }); } }
    /// CEF's SetFocus does it on Windows (its UI thread owns the child); nothing more here.
    pub fn focus_child(_w: u64) {}
    /// The client's window is gpui's, on this (its) thread.
    pub fn focus_host(h: Host) { if let Host::Win(p) = h { unsafe { SetFocus(p as HWND); } } }
    pub fn geometry(w: u64) -> Option<(R, bool)> {
        unsafe {
            let mut rc = std::mem::zeroed::<windows_sys::Win32::Foundation::RECT>();
            if GetWindowRect(hwnd(w), &mut rc) == 0 { return None }
            let parent = GetParent(hwnd(w)); let mut pt = [windows_sys::Win32::Foundation::POINT { x: rc.left, y: rc.top }];
            windows_sys::Win32::Graphics::Gdi::ScreenToClient(parent, pt.as_mut_ptr());
            Some((R { x: pt[0].x, y: pt[0].y, w: rc.right - rc.left, h: rc.bottom - rc.top }, IsWindowVisible(hwnd(w)) != 0))
        }
    }
    pub fn painted(w: u64) -> usize {
        let Some((r, _)) = geometry(w) else { return 0 };
        unsafe {
            let dc = GetDC(hwnd(w)); if dc.is_null() { return 0 }
            let mut n = 0; let mut i = 0usize;
            for y in (0..r.h).step_by(16) { for x in (0..r.w).step_by(16) { i += 1; let c = GetPixel(dc, x, y); if c != 0xFFFF_FFFF && (c & 0xff < 240 || (c >> 8) & 0xff < 240 || (c >> 16) & 0xff < 240) { n += 1; } } }
            ReleaseDC(hwnd(w), dc); let _ = i; n
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{Host, R};
    pub fn pixel(_w: u64, _x: i32, _y: i32) -> Option<[u8; 3]> { None }
    pub fn inner_size(_w: u64) -> Option<(i32, i32)> { None }
    pub fn container(_h: Host) -> Option<Host> { None }
    pub fn place_in(_frame: u64, _w: u64, _r: Option<R>) {}
    pub fn destroy(_frame: u64) {}
    #[cfg(windows)] pub fn close(w: u64) { unsafe { windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(w as isize as _); } }
    #[cfg(target_os = "macos")] pub fn close(w: u64) { unsafe { send0(v(w), c"removeFromSuperview"); } }
    /// Every window under `h` (class, rect in the screen, visible, style), for the report.
    #[cfg(windows)] pub fn tree(h: Host) -> Vec<String> {
        use windows_sys::Win32::{Foundation::*, UI::WindowsAndMessaging::*};
        let Host::Win(p) = h else { return vec![] };
        unsafe extern "system" fn cb(w: HWND, l: LPARAM) -> i32 {
            let out = &mut *(l as *mut Vec<String>); let mut cls = [0u16; 64]; let n = GetClassNameW(w, cls.as_mut_ptr(), 64);
            let mut r = std::mem::zeroed::<RECT>(); GetWindowRect(w, &mut r);
            out.push(format!("{:x} {} [{},{} {}x{}] vis={} style={:x} ex={:x} parent={:x}", w as usize, String::from_utf16_lossy(&cls[..n.max(0) as usize]), r.left, r.top, r.right - r.left, r.bottom - r.top, // i18n-ok: a diagnostic in the e2e report
                IsWindowVisible(w), GetWindowLongPtrW(w, GWL_STYLE), GetWindowLongPtrW(w, GWL_EXSTYLE), GetParent(w) as usize)); 1 }
        let mut out: Vec<String> = Vec::new();
        unsafe { let mut r = std::mem::zeroed::<RECT>(); GetWindowRect(p as HWND, &mut r); out.push(format!("client {:x} [{},{} {}x{}] ex={:x}", p, r.left, r.top, r.right - r.left, r.bottom - r.top, GetWindowLongPtrW(p as HWND, GWL_EXSTYLE))); // i18n-ok: a diagnostic in the e2e report
            EnumChildWindows(p as HWND, Some(cb), &mut out as *mut _ as LPARAM); }
        out
    }
    #[cfg(windows)] pub fn button_down() -> bool { use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState; unsafe { [0x01, 0x02, 0x04].iter().any(|k| GetAsyncKeyState(*k) as u16 & 0x8000 != 0) } }
    #[cfg(target_os = "macos")] pub fn button_down() -> bool {
        #[link(name = "objc")] unsafe extern "C" { fn objc_getClass(n: *const std::ffi::c_char) -> *mut c_void; }
        unsafe { let cls = objc_getClass(c"NSEvent".as_ptr()); !cls.is_null() && !send0(cls, c"pressedMouseButtons").is_null() } // i18n-ok: an Objective-C class name
    }
    use std::ffi::{c_void, CStr};
    #[repr(C)] #[derive(Clone, Copy)] struct Rect { x: f64, y: f64, w: f64, h: f64 }
    #[link(name = "objc")] unsafe extern "C" { fn sel_registerName(n: *const std::ffi::c_char) -> *mut c_void; fn objc_msgSend(); }
    fn sel(n: &CStr) -> *mut c_void { unsafe { sel_registerName(n.as_ptr()) } }
    unsafe fn send0(o: *mut c_void, s: &CStr) -> *mut c_void { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) }
    unsafe fn send_bool(o: *mut c_void, s: &CStr) -> bool { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) }
    unsafe fn send_obj(o: *mut c_void, s: &CStr, a: *mut c_void) -> *mut c_void { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), a) }
    unsafe fn send_flag(o: *mut c_void, s: &CStr, a: bool) { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, bool) = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), a) }
    unsafe fn send_rect(o: *mut c_void, s: &CStr, r: Rect) { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, Rect) = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), r) }
    #[cfg(target_arch = "x86_64")] #[link(name = "objc")] unsafe extern "C" { fn objc_msgSend_stret(); }
    unsafe fn rect_of(o: *mut c_void, s: &CStr) -> Rect {
        #[cfg(target_arch = "aarch64")] { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> Rect = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) }
        #[cfg(target_arch = "x86_64")] { let mut r = Rect { x: 0., y: 0., w: 0., h: 0. }; let f: unsafe extern "C" fn(*mut Rect, *mut c_void, *mut c_void) = std::mem::transmute(objc_msgSend_stret as *const ()); f(&mut r, o, sel(s)); r }
    }
    fn v(w: u64) -> *mut c_void { w as usize as *mut c_void }
    /// `r` is in points from the parent's top-left; an unflipped parent (gpui's view) counts y from its bottom.
    pub fn place(w: u64, r: R) {
        unsafe {
            let parent = send0(v(w), c"superview"); if parent.is_null() { return }
            let y = if send_bool(parent, c"isFlipped") { r.y as f64 } else { rect_of(parent, c"bounds").h - (r.y + r.h) as f64 };
            send_rect(v(w), c"setFrame:", Rect { x: r.x as f64, y, w: r.w.max(1) as f64, h: r.h.max(1) as f64 });
        }
    }
    pub fn show(w: u64, on: bool) { unsafe { send_flag(v(w), c"setHidden:", !on); } }
    pub fn focus_child(w: u64) { unsafe { let win = send0(v(w), c"window"); if !win.is_null() { send_obj(win, c"makeFirstResponder:", v(w)); } } }
    pub fn focus_host(h: Host) { if let Host::Mac(p) = h { unsafe { let win = send0(p as *mut c_void, c"window"); if !win.is_null() { send_obj(win, c"makeFirstResponder:", p as *mut c_void); } } } }
    pub fn geometry(w: u64) -> Option<(R, bool)> {
        unsafe {
            let parent = send0(v(w), c"superview"); if parent.is_null() { return None }
            let f = rect_of(v(w), c"frame"); let y = if send_bool(parent, c"isFlipped") { f.y } else { rect_of(parent, c"bounds").h - f.y - f.h };
            Some((R { x: f.x as i32, y: y as i32, w: f.w as i32, h: f.h as i32 }, !send_bool(v(w), c"isHidden")))
        }
    }
    /// Not read on macOS (the e2e reads the page over CDP there): the child is counted painted once it is shown.
    pub fn painted(w: u64) -> usize { geometry(w).map_or(0, |(_, on)| on as usize) }
}

pub use imp::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn only_native_window_handles_get_a_windowed_browser() {
        use raw_window_handle::*;
        let x = RawWindowHandle::Xcb(XcbWindowHandle::new(std::num::NonZeroU32::new(42).unwrap()));
        assert_eq!(Host::of(x), Some(Host::X11(42)));
        let wl = RawWindowHandle::Wayland(WaylandWindowHandle::new(std::ptr::NonNull::<std::ffi::c_void>::dangling()));
        assert_eq!(Host::of(wl), None, "Wayland: off screen");
    }
}

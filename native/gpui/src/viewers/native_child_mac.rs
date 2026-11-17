//! macOS: the embedded KiCad's frames over the pane (fork capability `embed-macos`, BACKPLANE_EMBED.md "macOS"). A window of another
//! process cannot be parented into ours, so nothing is reparented: KiCad (an accessory app: no Dock icon, no menu bar) keeps every frame
//! a borderless window of its own and places it in screen coordinates over our pane from the rect we push (`editor.kicad.host`, the
//! fork's `SetHostGeometry`), ordered just above our window. Measured on the prototype and on real KiCad (test/kicad_embed/mac,
//! scripts/backplane-embed-macos-check.py in the fork):
//! - the "container" is our own window's number (CGWindowID): what KiCad orders its frames above;
//! - a click or an activation raises our window over KiCad's frame with no other notice: after every frame of ours, `keep_below`
//!   orders our window just below the frame on show again when something else is right above us (one frame of gap, ~11 ms);
//! - the keyboard goes to KiCad by activating its app from here (macOS 14+ lets the active app activate another; KiCad's own
//!   `activate` from the background is ignored there), and back by activating ours;
//! - the frame must leave the screen when our window cannot be seen (minimised, full screen: another app's window cannot join our
//!   full-screen Space, another Space, closed): `host_visible`, pushed with the rect.
//! Every call is plain Objective-C messaging through `objc_msgSend` (as viewers/cef_native.rs does) and CoreGraphics' window list,
//! which needs no permission for bounds, owners and stacking.
#![allow(dead_code, unsafe_op_in_unsafe_fn)]
use super::{Frame, R};
use std::ffi::{c_void, CStr};

#[repr(C)] #[derive(Clone, Copy, Debug, Default)] struct Rect { x: f64, y: f64, w: f64, h: f64 }
#[link(name = "objc")] unsafe extern "C" { fn sel_registerName(n: *const std::ffi::c_char) -> *mut c_void; fn objc_getClass(n: *const std::ffi::c_char) -> *mut c_void; fn objc_msgSend(); }
fn sel(n: &CStr) -> *mut c_void { unsafe { sel_registerName(n.as_ptr()) } }
fn class(n: &CStr) -> *mut c_void { unsafe { objc_getClass(n.as_ptr()) } }
unsafe fn send0(o: *mut c_void, s: &CStr) -> *mut c_void { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) }
unsafe fn send_bool(o: *mut c_void, s: &CStr) -> bool { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) }
unsafe fn send_isize(o: *mut c_void, s: &CStr) -> isize { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> isize = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) }
unsafe fn send_usize(o: *mut c_void, s: &CStr) -> usize { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) }
unsafe fn send_f64(o: *mut c_void, s: &CStr) -> f64 { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> f64 = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) }
unsafe fn send_i32_arg(o: *mut c_void, s: &CStr, a: i32) -> *mut c_void { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), a) }
unsafe fn send_isize_arg(o: *mut c_void, s: &CStr, a: isize) -> *mut c_void { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, isize) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), a) }
unsafe fn send_obj(o: *mut c_void, s: &CStr, a: *mut c_void) -> *mut c_void { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), a) }
unsafe fn send_order(o: *mut c_void, s: &CStr, place: isize, rel: isize) { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, isize, isize) = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), place, rel) }
unsafe fn send_rect_rect(o: *mut c_void, s: &CStr, r: Rect) -> Rect { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, Rect) -> Rect = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), r) }
unsafe fn rect_of(o: *mut c_void, s: &CStr) -> Rect { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> Rect = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) }

// CoreGraphics' window list (no permission needed for numbers, owners, layers and bounds)
#[link(name = "CoreGraphics", kind = "framework")] unsafe extern "C" { fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> *const c_void; } // i18n-ok: a framework name for the linker
#[link(name = "CoreFoundation", kind = "framework")] unsafe extern "C" { // i18n-ok: a framework name for the linker
    fn CFArrayGetCount(a: *const c_void) -> isize; fn CFArrayGetValueAtIndex(a: *const c_void, i: isize) -> *const c_void;
    fn CFDictionaryGetValue(d: *const c_void, k: *const c_void) -> *const c_void; fn CFNumberGetValue(n: *const c_void, ty: isize, out: *mut c_void) -> bool;
    fn CFRelease(o: *const c_void);
    static kCGWindowNumber: *const c_void; static kCGWindowOwnerPID: *const c_void; static kCGWindowLayer: *const c_void;
}
const ON_SCREEN_ABOVE: u32 = 1 << 1; const INCLUDING: u32 = 1 << 3; const EXCLUDE_DESKTOP: u32 = 1 << 4;
fn num(d: *const c_void, k: *const c_void) -> i64 { let mut v = 0i64; unsafe { let n = CFDictionaryGetValue(d, k); if !n.is_null() { CFNumberGetValue(n, 4 /* kCFNumberSInt64Type */, &mut v as *mut i64 as *mut c_void); } } v }
/// (number, owner pid, layer) of the on-screen windows above `w`, nearest first, or of `w` itself (`INCLUDING`).
fn list(option: u32, w: u64) -> Vec<(u64, i64, i64)> {
    let mut out = Vec::new();
    unsafe {
        let a = CGWindowListCopyWindowInfo(option, w as u32); if a.is_null() { return out }
        for i in 0..CFArrayGetCount(a) { let d = CFArrayGetValueAtIndex(a, i); out.push((num(d, kCGWindowNumber) as u64, num(d, kCGWindowOwnerPID), num(d, kCGWindowLayer))); }
        CFRelease(a);
    }
    out
}
/// The process owning window `w` (0 if it is gone).
pub fn owner(w: u64) -> i64 { list(INCLUDING, w).first().map(|e| e.1).unwrap_or(0) }

fn app() -> *mut c_void { unsafe { send0(class(c"NSApplication"), c"sharedApplication") } } // i18n-ok: an Objective-C class name
/// Our own NSWindow with that number, or null.
fn ns_window(n: u64) -> *mut c_void { unsafe { send_isize_arg(app(), c"windowWithWindowNumber:", n as isize) } }

/// gpui's window (its AppKit view) to its number, the "container" and "toplevel" the hub hands KiCad.
pub fn window_number(ns_view: *mut c_void) -> Option<u64> {
    if ns_view.is_null() { return None }
    unsafe { let w = send0(ns_view, c"window"); if w.is_null() { None } else { Some(send_isize(w, c"windowNumber") as u64) } }
}

/// The pane in screen points (top-left origin of the primary display, as KiCad and the window list count): `r` is in our window's
/// physical pixels from its content's top-left.
pub fn screen_rect(top: u64, r: R) -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let w = ns_window(top); if w.is_null() { return None }
        let view = send0(w, c"contentView"); if view.is_null() { return None }
        let scale = send_f64(w, c"backingScaleFactor").max(1.0);
        let content = send_rect_rect(w, c"convertRectToScreen:", rect_of(view, c"frame"));
        let screens = send0(class(c"NSScreen"), c"screens"); // i18n-ok: an Objective-C class name
        let primary = send_i32_arg(screens, c"objectAtIndex:", 0); if primary.is_null() { return None }
        let height = rect_of(primary, c"frame").h;
        let top_y = height - (content.y + content.h);
        Some(((content.x + r.x as f64 / scale).round() as i32, (top_y + r.y as f64 / scale).round() as i32,
              (r.w as f64 / scale).round() as i32, (r.h as f64 / scale).round() as i32))
    }
}

/// Whether our window can be seen: not minimised, not full screen (KiCad's window cannot join that Space), on the active Space, ordered in.
pub fn host_visible(top: u64) -> bool {
    unsafe {
        let w = ns_window(top); if w.is_null() { return false }
        const FULL_SCREEN: usize = 1 << 14;
        send_bool(w, c"isVisible") && !send_bool(w, c"isMiniaturized") && send_bool(w, c"isOnActiveSpace") && send_usize(w, c"styleMask") & FULL_SCREEN == 0
    }
}

/// Our window is in full screen: KiCad's frame cannot be shown over it (another app's window cannot join that Space).
pub fn host_full_screen(top: u64) -> bool {
    unsafe { let w = ns_window(top); if w.is_null() { return false } const FULL_SCREEN: usize = 1 << 14; send_usize(w, c"styleMask") & FULL_SCREEN != 0 }
}

/// After a click or an activation our window is above KiCad's frame `frame`: order ours just below it again (no IPC; KiCad keeps its
/// frame directly above us otherwise). True when it re-ordered.
pub fn keep_below(top: u64, frame: u64) -> bool {
    if frame == 0 { return false }
    // the normal windows just above ours, nearest first: KiCad's own (its dialogs, over the frame) up to the frame, or nothing to do.
    // A KiCad dialog between ours and the frame counted as another app's window: ours went below the frame and buried the dialog under
    // it (run 18: Find open but unseen, its Escape never reached it)
    let kicad = match owner(frame) { 0 => KICAD_PID.load(std::sync::atomic::Ordering::Relaxed), p => p };
    for e in list(ON_SCREEN_ABOVE | EXCLUDE_DESKTOP, top).into_iter().rev().filter(|e| e.2 == 0) {
        if e.0 == frame { return false }
        if e.1 != kicad { break }
    }
    // only when the frame is on screen at all (hidden by KiCad or by us: nothing to order)
    if !list(ON_SCREEN_ABOVE | EXCLUDE_DESKTOP, 0).iter().any(|e| e.0 == frame) { return false }
    unsafe { let w = ns_window(top); if w.is_null() { return false } send_order(w, c"orderWindow:relativeTo:", -1 /* NSWindowBelow */, frame as isize); }
    true
}

fn running_app(pid: i64) -> *mut c_void { unsafe { send_i32_arg(class(c"NSRunningApplication"), c"runningApplicationWithProcessIdentifier:", pid as i32) } } // i18n-ok: an Objective-C class name
fn own_pid() -> i64 { std::process::id() as i64 }

// ---- the native_child API (see its X11 and Win32 versions) ----

pub fn container(parent: u64) -> Option<u64> { (parent != 0).then_some(parent) }
pub fn container_for(parent: u64, _key: &str) -> Option<u64> { container(parent) }
pub fn move_frames(_old: u64, _new: u64) {}
pub fn destroy(_container: u64) {}
/// macOS: KiCad's frames are its own windows (no parenting): nothing to move out (the integrator, Nov 16).
pub fn release_holders(_container: Option<u64>) {}
pub fn adopt(_pid: u32, _container: u64, _toplevel: u64) -> Vec<u64> { vec![] }
pub fn dialogs(_pid: u32, _container: u64, _toplevel: u64) {}
/// KiCad centres its dialogs on our window itself (MakeTransientFor); the hub's ET_DIALOG events are the dialog list, as on Windows.
pub fn centre_dialogs(_pid: u32, _over: R) -> Vec<String> { vec![] }
/// KiCad's frames are its own windows: nothing of ours to list, map or unmap (the hub shows and hides them; `local_map` is off here).
pub fn frames(_container: u64) -> Vec<Frame> { vec![] }
pub fn place(_container: u64, _r: Option<R>, _show: Option<u64>, _managed: Option<&[u64]>, _last_root: &mut (i32, i32)) {}
pub fn place_local(_container: u64, _r: Option<R>, _show: Option<u64>, _mine: &[u64], _last_root: &mut (i32, i32)) {}
pub fn sync_root(_container: u64, _r: R, _show: u64, _last_root: &mut (i32, i32)) {}
/// gpui's overlays cannot be cut out of another process's window: an overlay over the pane is drawn under KiCad's frame (the client
/// hides the frame instead while one is up, as for the full-window modals).
pub fn holes(_container: u64, _keep: Option<&[R]>) {}
/// The keyboard to `w`: our own window (activate Backplane), or a KiCad frame (activate KiCad from here: on macOS 14+ only the active
/// app may hand activation to another; KiCad then makes its frame key and wx gives its canvas the focus back).
/// The KiCad the pane shows (the hub's session pid): a frame not on screen yet is not in the window list, so its owner is known from here.
pub fn note_kicad_pid(pid: u32) { if pid > 0 { KICAD_PID.store(pid as i64, std::sync::atomic::Ordering::Relaxed); } }

pub fn focus(w: u64) {
    // ours: one of our own windows (never "whoever the window list does not know": KiCad's frame before it is on screen is not there)
    let ours = !ns_window(w).is_null();
    let pid = if ours { own_pid() } else { match owner(w) { 0 => KICAD_PID.load(std::sync::atomic::Ordering::Relaxed), p => p } };
    if !ours && pid > 0 { KICAD_PID.store(pid, std::sync::atomic::Ordering::Relaxed); }
    eprintln!("GPUI_LOG {}", serde_json::json!({"kicad_mac_focus": {"window": w, "owner": pid, "ours": ours, "kicad_active_before": app_active(KICAD_PID.load(std::sync::atomic::Ordering::Relaxed))}}));
    if !ours && pid <= 0 { return } // a KiCad frame whose KiCad is not known yet: nothing to activate
    unsafe {
        if ours {
            send_activate_self(app());
            let nw = ns_window(w); if !nw.is_null() { send0(nw, c"makeKeyWindow"); }
        } else {
            let other = running_app(pid); if other.is_null() { return }
            // macOS 14+: hand activation over first (the active app's consent), then activate KiCad; its frame becomes key and wx gives
            // the canvas the focus (proved on real KiCad: scripts/backplane-embed-macos-check.py in the fork)
            let a = app();
            let responds: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool = std::mem::transmute(objc_msgSend as *const ());
            if responds(a, sel(c"respondsToSelector:"), sel(c"yieldActivationToApplication:")) { send_obj(a, c"yieldActivationToApplication:", other); }
            // NSApplicationActivateAllWindows is not wanted: only the frame on show is ours to raise, and KiCad orders it itself
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> bool = std::mem::transmute(objc_msgSend as *const ());
            f(other, sel(c"activateWithOptions:"), 0);
        }
    }
}
unsafe fn send_activate_self(a: *mut c_void) {
    // -[NSApplication activate] (macOS 14+), else activateIgnoringOtherApps:YES
    let responds: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool = std::mem::transmute(objc_msgSend as *const ());
    if responds(a, sel(c"respondsToSelector:"), sel(c"activate")) { send0(a, c"activate"); }
    else { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, bool) = std::mem::transmute(objc_msgSend as *const ()); f(a, sel(c"activateIgnoringOtherApps:"), true); }
}
pub fn hand_back(w: u64) { focus(w) }
/// Consent for KiCad to activate itself (macOS 14+), without activating it: the hub's ShowFrame{focus} then has KiCad take the keyboard
/// onto its canvas (TakeFocusForClick). Activating KiCad from here first put wx's focus on a toolbar field, and ShowFrame{focus} leaves a
/// focus already inside the frame where it is: the canvas never had the keys (client runs 9, 10).
pub fn yield_to(w: u64) {
    let pid = match owner(w) { 0 => KICAD_PID.load(std::sync::atomic::Ordering::Relaxed), p => p };
    if pid <= 0 || pid == own_pid() { return }
    unsafe {
        let other = running_app(pid); if other.is_null() { return }
        let a = app();
        let responds: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool = std::mem::transmute(objc_msgSend as *const ());
        if responds(a, sel(c"respondsToSelector:"), sel(c"yieldActivationToApplication:")) { send_obj(a, c"yieldActivationToApplication:", other); }
        else { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> bool = std::mem::transmute(objc_msgSend as *const ()); f(other, sel(c"activateWithOptions:"), 0); }
    }
    eprintln!("GPUI_LOG {}", serde_json::json!({"kicad_mac_yield": {"window": w, "to": pid}}));
    // KiCad activates itself on the focused show; when Backplane's own window became key again meanwhile (a palette closing), the consent
    // was gone and KiCad stayed in the background with Backplane holding the keys (client runs 16, 17). Then, 400 ms on, Backplane (still
    // the active app) activates KiCad itself: wx gives back the focus KiCad had, its canvas. NSRunningApplication is thread-safe.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        if app_active(pid) { return }
        unsafe {
            let other = running_app(pid); if other.is_null() { return }
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> bool = std::mem::transmute(objc_msgSend as *const ());
            f(other, sel(c"activateWithOptions:"), 0);
        }
        eprintln!("GPUI_LOG {}", serde_json::json!({"kicad_mac_yield_fallback": {"to": pid, "active_after": app_active(pid)}}));
    });
}
pub fn activated_after_close(_main: u64, _popped: u64) -> bool { true }
pub fn note_pane_keys(_kicad: bool) {}
pub fn shield(_container: u64, _on: bool) {}
pub fn shield_tick(_container: u64, _shown: Option<u64>) {}
pub fn backplane_press_ms_ago() -> u64 { u64::MAX }
pub fn keys_really_backplanes(_container: u64) -> bool { true }
pub fn focus_debug(_container: u64) -> serde_json::Value { serde_json::json!({"active": unsafe { send_bool(app(), c"isActive") }}) }
pub fn invisible_kicad_foreground(_frame: u64) -> Option<(u64, String)> { None }
pub fn frame_visible(_container: u64, _frame: u64) -> bool { true }
pub fn conn_fd() -> Option<i32> { None }
pub fn pump() -> bool { false }
pub fn input_off(_container: u64, _off: bool) {}
pub fn semicolon_held() -> Option<bool> { None }
/// The KiCad that last got the keyboard from here (focus): its app's own active state is the truth. NSWorkspace's frontmost app is the
/// menu bar's owner, which an accessory app never is, so it kept naming Backplane while KiCad had the keyboard (client run 7).
static KICAD_PID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
fn app_active(pid: i64) -> bool { if pid <= 0 { return false } let a = running_app(pid); !a.is_null() && unsafe { send_bool(a, c"isActive") } }
/// KiCad has the keyboard: its app is the active one.
pub fn has_focus(_container: u64) -> bool { app_active(KICAD_PID.load(std::sync::atomic::Ordering::Relaxed)) }
/// The app `pid` is the active one (an accessory app is active while it holds the keyboard, though it never owns the menu bar).
pub fn focus_in_pid(pid: u32) -> bool { app_active(pid as i64) }
pub fn button_over(_w: u64) -> Option<bool> { None }
pub fn grab_click(_container: u64, _on: bool) {}
pub fn grab_escape(_top: u64, _on: bool) {}
pub fn keep(_holder: u64) {}
/// KiCad's dialogs are its own app's windows: a new one takes the keyboard by KiCad's activation, nothing for the client to hand over.
pub fn take_new_dialogs() -> Vec<u64> { vec![] }
/// A press over a KiCad dialog is that dialog's app's own on macOS (its window, not ours): nothing for the pane to tell apart.
pub fn pointer_on_other_window(_w: u64) -> bool { false }
/// Focus is app activation on macOS (focus, yield_to): there is no window focus of ours for the pane to keep.
pub fn keep_focus(_top: u64) -> bool { false }
pub fn activate(_w: u64) {}
pub fn open_dialogs(_pid: u32) -> Vec<u64> { vec![] }
/// The reserved chords KiCad sent through the hub (kicad_pane `set_info`), for the pane's event loop.
static KEYS: std::sync::Mutex<Vec<super::Ev>> = std::sync::Mutex::new(Vec::new());
pub fn push_key(chord: &str) { if let Ok(mut q) = KEYS.lock() { q.push(if chord == "ctrl-`" { super::Ev::Escape } else { super::Ev::Key(chord.to_string()) }); } }
pub fn events() -> Vec<super::Ev> { KEYS.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default() }
/// A reserved chord as Backplane binds it on macOS: wx's Ctrl is Cmd there, so KiCad's "Ctrl+K" is the person's Cmd+K (Backplane's
/// `secondary-k`); the tab and Escape chords stay Ctrl (Cmd+Tab and Cmd+` belong to macOS).
pub fn mac_chord(chord: &str) -> String {
    if matches!(chord, "ctrl-tab" | "ctrl-shift-tab" | "ctrl-`") { chord.to_string() } else { chord.replacen("ctrl-", "cmd-", 1) }
}
pub fn poke(_w: u64, _keysym: u32, _mods: u16) {}
pub fn report(_w: u64) -> Option<(R, bool, usize)> { None }
pub fn window_meta(_w: u64, _container: Option<u64>, _toplevel: u64) -> Option<(u8, (i32, i32), usize, bool)> { None }

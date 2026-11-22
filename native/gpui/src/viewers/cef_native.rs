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

static PRESS_WAKERS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<u64, async_channel::Sender<()>>>> =
    std::sync::LazyLock::new(Default::default);

/// Registers `wake` so recorded native presses on `target` wake `CefPage` even when CEF emits no focus callback.
pub fn register_press_wake(target: u64, wake: async_channel::Sender<()>) {
    if target != 0 {
        PRESS_WAKERS.lock().unwrap().insert(target, wake);
    }
}

fn unregister_press_wake(target: u64) {
    if target != 0 {
        PRESS_WAKERS.lock().unwrap().remove(&target);
    }
}

fn notify_press_wake(target: u64) {
    let tx = PRESS_WAKERS.lock().unwrap().get(&target).cloned();
    if let Some(tx) = tx {
        let _ = tx.try_send(());
    }
}

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
        let _ = c.flush(); drop(g);
        containers().lock().unwrap().insert(id, parent); watch_presses(id);
        // a native child in the client's window like a KiCad pane's: while it is on screen (place_in) Backplane's keyboard is on the window's
        // focus proxy, not the window itself, or core X gives every key typed with the pointer over the page to Chromium (#1505's fault)
        nc::adopt_child(parent as u64, id as u64);
        Some(Host::X11(id))
    }
    pub fn close(_w: u64) {}
    /// Every container of ours (a tab's `container`), until it is destroyed: what a recorded press is matched against.
    /// (each with the client's top-level it is in)
    fn containers() -> &'static Mutex<std::collections::HashMap<u32, u32>> { static C: OnceLock<Mutex<std::collections::HashMap<u32, u32>>> = OnceLock::new(); C.get_or_init(Default::default) }
    fn top_of(frame: u64) -> Option<u64> { containers().lock().unwrap().get(&(frame as u32)).map(|t| *t as u64) }
    use crate::viewers::cef_focus::{next_focus_seq, FocusCallback, PressProbe, PressTracker, X11BarrierFences};
    use crate::viewers::native_child as nc;
    struct X11PressState {
        tracker: PressTracker,
        fences: X11BarrierFences,
    }
    impl X11PressState {
        const fn new() -> Self {
            Self {
                tracker: PressTracker::new(),
                fences: X11BarrierFences::new(),
            }
        }
    }
    /// The latest press delivered to one of our containers. Record it before replaying
    /// the original X event, so even a released click can explain one CEF focus callback.
    static PRESS: Mutex<X11PressState> = Mutex::new(X11PressState::new());
    static PRESS_WINDOWS: std::sync::LazyLock<Mutex<std::collections::HashSet<u32>>> =
        std::sync::LazyLock::new(Default::default);
    struct PressWatcher {
        conn: std::sync::Weak<RustConnection>,
        barrier_win: u32,
        barrier_atom: u32,
    }
    static WATCHER: OnceLock<Option<PressWatcher>> = OnceLock::new();

    struct BarrierRequest {
        id: u64,
        targets: Vec<u64>,
        cutoff: Option<u64>,
        timed_out: bool,
    }
    struct BarrierState {
        next_id: u64,
        dead: bool,
        in_flight: Vec<BarrierRequest>,
    }
    enum FenceResult {
        Fenced(u64),
        TimedOut(u64),
        Unfenced,
    }
    static BARRIERS: std::sync::LazyLock<(Mutex<BarrierState>, std::sync::Condvar)> =
        std::sync::LazyLock::new(|| {
            (
                Mutex::new(BarrierState {
                    next_id: 0,
                    dead: false,
                    in_flight: Vec::new(),
                }),
                std::sync::Condvar::new(),
            )
        });

    fn complete_barrier(id: u64) {
        enum Claimed {
            Clear(Vec<u64>),
            TimedOut(Vec<u64>),
        }
        let claimed = {
            let mut st = BARRIERS.0.lock().unwrap();
            let Some(pos) = st.in_flight.iter().position(|r| r.id == id) else {
                return;
            };
            if st.in_flight[pos].timed_out {
                Claimed::TimedOut(st.in_flight.swap_remove(pos).targets)
            } else {
                Claimed::Clear(std::mem::take(&mut st.in_flight[pos].targets))
            }
        };
        let targets = match claimed {
            Claimed::TimedOut(targets) => {
                let mut press = PRESS.lock().unwrap();
                let X11PressState { tracker, fences } = &mut *press;
                fences.finish_timeout_watcher(id, || {
                    for f in targets {
                        tracker.clear_target(f);
                    }
                    next_focus_seq()
                });
                return;
            }
            Claimed::Clear(targets) => targets,
        };
        // Lock ordering: BARRIERS.0 and PRESS are never held together. Because this runs on the
        // sole X11 press-watcher thread, clearing watched targets and allocating `cutoff` right
        // before looping back to `wait_for_event` guarantees pre-barrier clicks cannot overtake
        // `cutoff` while genuinely post-barrier clicks stay intact with `seq > cutoff`.
        let cutoff = {
            let mut press = PRESS.lock().unwrap();
            for f in targets {
                press.tracker.clear_target(f);
            }
            next_focus_seq()
        };
        let timed_out = {
            let mut st = BARRIERS.0.lock().unwrap();
            let Some(pos) = st.in_flight.iter().position(|r| r.id == id) else {
                return;
            };
            if st.in_flight[pos].timed_out {
                st.in_flight.swap_remove(pos);
                true
            } else {
                st.in_flight[pos].cutoff = Some(cutoff);
                BARRIERS.1.notify_all();
                false
            }
        };
        if timed_out {
            PRESS.lock().unwrap().fences.finish_timeout_watcher(id, || cutoff);
        }
    }

    fn mark_barriers_dead() {
        let mut st = BARRIERS.0.lock().unwrap();
        st.dead = true;
        st.in_flight.retain(|r| r.cutoff.is_some());
        BARRIERS.1.notify_all();
    }

    fn fence_x11_watcher(watched: &[u64]) -> FenceResult {
        let Some((w, conn)) = WATCHER
            .get()
            .and_then(Option::as_ref)
            .and_then(|w| w.conn.upgrade().map(|conn| (w, conn)))
        else {
            return FenceResult::Unfenced;
        };
        let id = {
            let mut st = BARRIERS.0.lock().unwrap();
            if st.dead {
                return FenceResult::Unfenced;
            }
            st.next_id = st.next_id.wrapping_add(1).max(1);
            let id = st.next_id;
            st.in_flight.push(BarrierRequest {
                id,
                targets: watched.to_vec(),
                cutoff: None,
                timed_out: false,
            });
            id
        };
        let ev = ClientMessageEvent::new(
            32,
            w.barrier_win,
            w.barrier_atom,
            [id as u32, (id >> 32) as u32, 0, 0, 0],
        );
        if conn
            .send_event(false, w.barrier_win, EventMask::NO_EVENT, ev)
            .ok()
            .and_then(|cookie| cookie.check().ok())
            .is_none()
        {
            let mut st = BARRIERS.0.lock().unwrap();
            st.in_flight.retain(|r| r.id != id);
            return FenceResult::Unfenced;
        }
        let guard = BARRIERS.0.lock().unwrap();
        let mut guard = BARRIERS
            .1
            .wait_timeout_while(guard, std::time::Duration::from_millis(100), |st| {
                !st.dead && st.in_flight.iter().any(|r| r.id == id && r.cutoff.is_none())
            })
            .unwrap()
            .0;
        let Some(pos) = guard.in_flight.iter().position(|r| r.id == id) else {
            return FenceResult::Unfenced;
        };
        if let Some(cutoff) = guard.in_flight[pos].cutoff {
            guard.in_flight.swap_remove(pos);
            FenceResult::Fenced(cutoff)
        } else if guard.dead {
            guard.in_flight.swap_remove(pos);
            FenceResult::Unfenced
        } else {
            guard.in_flight[pos].timed_out = true;
            FenceResult::TimedOut(id)
        }
    }

    fn watch_presses(frame: u32) {
        use std::sync::{Arc, Weak};
        let watcher = WATCHER.get_or_init(|| {
            let start = || -> Result<PressWatcher, Box<dyn std::error::Error>> {
                let (connection, screen_num) = RustConnection::connect(None)?;
                let connection = Arc::new(connection);
                let root = connection.setup().roots.get(screen_num).ok_or("no_screen")?.root;
                let barrier_win = connection.generate_id()?;
                connection.create_window(0, barrier_win, root, 0, 0, 1, 1, 0, WindowClass::INPUT_ONLY, x11rb::COPY_FROM_PARENT, &CreateWindowAux::new())?.check()?;
                let barrier_atom = connection.intern_atom(false, b"_BACKPLANE_CEF_PRESS_BARRIER")?.reply()?.atom;
                let thread = connection.clone();
                std::thread::Builder::new().name("cef-presses".into()).spawn(move || {
                    let error = loop {
                        match thread.wait_for_event() {
                            Ok(x11rb::protocol::Event::ButtonPress(press)) => {
                                let target = press.event as u64;
                                let seq = {
                                    let mut press = PRESS.lock().unwrap();
                                    if press.fences.is_fenced(target) {
                                        0
                                    } else {
                                        press.tracker.record(target, std::time::Instant::now())
                                    }
                                };
                                if seq != 0 {
                                    super::notify_press_wake(target);
                                }
                                // Replay releases the synchronous grab and delivers the same press to Chromium.
                                if let Err(error) = thread.allow_events(Allow::REPLAY_POINTER, press.time).and_then(|_| thread.flush()) {
                                    break error;
                                }
                            }
                            Ok(x11rb::protocol::Event::ClientMessage(msg))
                                if msg.window == barrier_win && msg.type_ == barrier_atom && msg.format == 32 =>
                            {
                                let d = msg.data.as_data32();
                                let id = (d[0] as u64) | ((d[1] as u64) << 32);
                                complete_barrier(id);
                            }
                            Ok(_) => {}
                            Err(error) => break error,
                        }
                    };
                    PRESS_WINDOWS.lock().unwrap().clear();
                    PRESS.lock().unwrap().fences.mark_watcher_dead();
                    mark_barriers_dead();
                    eprintln!("GPUI_LOG {}", serde_json::json!({"cef_press_watcher": error.to_string()}));
                })?;
                Ok(PressWatcher { conn: Arc::downgrade(&connection), barrier_win, barrier_atom })
            };
            match start() {
                Ok(watcher) => Some(watcher),
                Err(error) => {
                    eprintln!("GPUI_LOG {}", serde_json::json!({"cef_press_watcher": error.to_string()}));
                    None
                }
            }
        });
        let Some(connection) = watcher.as_ref().and_then(|w| Weak::upgrade(&w.conn)) else { return };
        // Only buttons that count as a click are grabbed. The server delivers wheel buttons
        // normally, and delivers touch taps that emulate a button through the same grab.
        // A container grab observes the actual delivered click, including entry from native Wayland.
        let mut installed = true;
        for button in [1u8, 2, 3, 8, 9] {
            let request = connection.grab_button(false, frame, EventMask::BUTTON_PRESS,
                GrabMode::SYNC, GrabMode::ASYNC, x11rb::NONE, x11rb::NONE,
                ButtonIndex::from(button), ModMask::ANY);
            installed &= request.ok().and_then(|request| request.check().ok()).is_some();
        }
        if installed {
            PRESS_WINDOWS.lock().unwrap().insert(frame);
        } else {
            let _ = connection.ungrab_button(ButtonIndex::ANY, frame, ModMask::ANY);
            let _ = connection.flush();
            eprintln!("GPUI_LOG {}", serde_json::json!({"cef_press_watcher": "could not grab page buttons", "frame": frame}));
        }
    }

    fn pointer_target(c: &RustConnection, mut w: u32, matches: impl Fn(u32) -> bool) -> Option<u32> {
        for _ in 0..16 {
            w = c.query_pointer(w).ok()?.reply().ok()?.child;
            if w == x11rb::NONE { break }
            if matches(w) { return Some(w) }
        }
        None
    }

    /// Linux watches the container window created in `container()`, so per-native-handle setup is a no-op here.
    pub fn watch_native(_w: u64) {}
    pub fn unwatch_native(_w: u64) {}

    /// A recent press is consumed once per callback kind for the page that received it.
    pub fn pressed_in(frame: u64, cb: FocusCallback) -> PressProbe {
        let mut press = PRESS.lock().unwrap();
        if let Some(seq) = press.tracker.consume(frame, cb, std::time::Instant::now()) {
            return PressProbe::Pressed(seq);
        }
        let blocked = press.tracker.is_unwatched_blocked(frame);
        drop(press);
        if blocked || PRESS_WINDOWS.lock().unwrap().contains(&(frame as u32)) {
            PressProbe::NotPressed
        } else {
            PressProbe::Unwatched
        }
    }

    fn any_button_held() -> bool {
        let Some(g) = conn() else { return false };
        let c = g.as_ref().unwrap();
        let Some(root) = c.setup().roots.get(SCREEN.load(std::sync::atomic::Ordering::Relaxed)).map(|s| s.root) else { return false };
        c.query_pointer(root).ok().and_then(|r| r.reply().ok()).is_some_and(|p|
            u16::from(p.mask) & u16::from(KeyButMask::BUTTON1 | KeyButMask::BUTTON2 | KeyButMask::BUTTON3) != 0)
    }

    fn apply_unwatched_clear(press: &mut PressTracker, unwatched: &[u64], held: bool) -> Option<u64> {
        let mut any_blocked = false;
        for &f in unwatched {
            press.clear_target(f);
            if held && press.block_unwatched_until_release(f).is_some() {
                any_blocked = true;
            }
        }
        any_blocked.then(|| press.unwatched_block_gen())
    }

    fn spawn_unwatched_release_poller(spawn_gen: Option<u64>) {
        let Some(mut block_gen) = spawn_gen else { return };
        let _ = std::thread::Builder::new().name("cef-unwatched-rel".into()).spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(5));
                if !any_button_held() {
                    let mut press = PRESS.lock().unwrap();
                    if press.tracker.clear_unwatched_release_block_if(block_gen) {
                        break;
                    }
                    block_gen = press.tracker.unwatched_block_gen();
                }
            }
        });
    }

    /// Clears recorded presses on all `frames` and allocates a cutoff sequence while still holding `PRESS`,
    /// synchronously fencing queued X11 `ButtonPress` events behind a barrier `ClientMessage` on the watcher
    /// connection (suppressing stalled pre-barrier clicks via `X11BarrierFences` across any timeout until
    /// the watcher processes the barrier, and gating unwatched targets behind button release) so pre-barrier
    /// queued presses cannot overtake the cutoff while genuinely post-cutoff clicks stay intact.
    pub fn clear_presses(frames: &[u64]) -> u64 {
        let mut unique: Vec<u64> = Vec::with_capacity(frames.len());
        for &f in frames {
            if f != 0 && !unique.contains(&f) {
                unique.push(f);
            }
        }
        if unique.is_empty() {
            return next_focus_seq();
        }
        let (watched, unwatched): (Vec<u64>, Vec<u64>) = {
            let pw = PRESS_WINDOWS.lock().unwrap();
            unique.into_iter().partition(|f| pw.contains(&(*f as u32)))
        };
        let held = !unwatched.is_empty() && any_button_held();
        let fence_result = if watched.is_empty() {
            FenceResult::Unfenced
        } else {
            fence_x11_watcher(&watched)
        };
        if let FenceResult::Fenced(cutoff) = fence_result {
            if !unwatched.is_empty() {
                let spawn_gen = {
                    let mut press = PRESS.lock().unwrap();
                    apply_unwatched_clear(&mut press.tracker, &unwatched, held)
                };
                spawn_unwatched_release_poller(spawn_gen);
            }
            return cutoff;
        }
        let timed_out_id = match fence_result {
            FenceResult::TimedOut(id) => Some(id),
            _ => None,
        };
        let (cutoff, spawn_gen) = {
            let mut press = PRESS.lock().unwrap();
            if let Some(watcher_cutoff) =
                timed_out_id.and_then(|id| press.fences.finish_timeout_caller(id, &watched))
            {
                let spawn_gen = apply_unwatched_clear(&mut press.tracker, &unwatched, held);
                (watcher_cutoff, spawn_gen)
            } else {
                for &f in &watched {
                    press.tracker.clear_target(f);
                }
                let spawn_gen = apply_unwatched_clear(&mut press.tracker, &unwatched, held);
                (next_focus_seq(), spawn_gen)
            }
        };
        spawn_unwatched_release_poller(spawn_gen);
        cutoff
    }

    /// Clears any recorded press on `frame` when keyboard focus returns to the host.
    pub fn clear_press(frame: u64) {
        if frame != 0 {
            let _ = clear_presses(&[frame]);
        }
    }

    pub fn fresh_press_seq_after(frame: u64, after_seq: u64) -> Option<u64> {
        PRESS.lock().unwrap().tracker.fresh_press_seq_after(frame, after_seq, std::time::Instant::now())
    }

    pub fn clear_press_up_to(frame: u64, max_seq: u64) {
        PRESS.lock().unwrap().tracker.clear_target_up_to(frame, max_seq);
    }

    /// Fallback for SYSTEM focus if the page button grab could not be installed. Require the held
    /// button and the pointer inside the page that reported focus.
    pub fn button_down(frame: u64) -> bool {
        if frame == 0 || PRESS.lock().unwrap().tracker.is_unwatched_blocked(frame) || !any_button_held() { return false }
        let Some(g) = conn() else { return false }; let c = g.as_ref().unwrap();
        let Some(root) = c.setup().roots.get(SCREEN.load(std::sync::atomic::Ordering::Relaxed)).map(|s| s.root) else { return false };
        let inside = pointer_target(c, root, |w| w as u64 == frame).is_some();
        drop(g);
        inside && !PRESS.lock().unwrap().tracker.is_unwatched_blocked(frame)
    }
    /// The container gone (its browser closed).
    pub fn destroy(frame: u64) {
        PRESS_WINDOWS.lock().unwrap().remove(&(frame as u32));
        {
            let mut press = PRESS.lock().unwrap();
            press.tracker.drop_target(frame);
            press.fences.drop_target(frame);
        }
        super::unregister_press_wake(frame);
        if let Some(t) = containers().lock().unwrap().remove(&(frame as u32)) { nc::release_child(t as u64, frame); }
        let Some(g) = conn() else { return }; let c = g.as_ref().unwrap();
        let _ = c.destroy_window(frame as u32); let _ = c.flush();
    }
    /// The container (and Chromium's window filling it) over the pane at `r`, mapped; or unmapped.
    pub fn place_in(frame: u64, w: u64, r: Option<R>) {
        if let Some(t) = top_of(frame) { nc::note_pane_on_screen(t, frame, r.is_some()); }
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
    /// (to its focus proxy while a native child is on screen there: native_child `focus_home`)
    pub fn focus_host(h: Host, frame: u64) { if let Host::X11(w) = h { nc::focus_home(w as u64, frame) } }
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
    use crate::viewers::cef_focus::{next_focus_seq, FocusCallback, MouseActivateOutcome, PressProbe, PressTracker};
    use std::collections::{HashMap, HashSet};
    use std::sync::{LazyLock, Mutex};
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{GetDC, GetPixel, ReleaseDC, ScreenToClient},
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, GetFocus, SetFocus},
            WindowsAndMessaging::*,
        },
    };

    static PRESS: Mutex<PressTracker> = Mutex::new(PressTracker::new());
    static PRESS_WINDOWS: LazyLock<Mutex<HashSet<u64>>> = LazyLock::new(Default::default);
    static HOOKED_THREADS: LazyLock<Mutex<HashMap<u32, (usize, usize, usize, HashSet<u64>)>>> =
        LazyLock::new(Default::default);

    pub fn pixel(_w: u64, _x: i32, _y: i32) -> Option<[u8; 3]> { None }
    pub fn inner_size(_w: u64) -> Option<(i32, i32)> { None }
    pub fn container(_h: Host) -> Option<Host> { None }
    pub fn place_in(_frame: u64, _w: u64, _r: Option<R>) {}
    pub fn destroy(_frame: u64) {}

    fn any_button_held() -> bool {
        unsafe { [0x01, 0x02, 0x04, 0x05, 0x06].iter().any(|k| GetAsyncKeyState(*k) as u16 & 0x8000 != 0) }
    }

    fn is_button_down_msg(msg: u32) -> bool {
        matches!(
            msg,
            WM_LBUTTONDOWN
                | WM_LBUTTONDBLCLK
                | WM_RBUTTONDOWN
                | WM_RBUTTONDBLCLK
                | WM_MBUTTONDOWN
                | WM_MBUTTONDBLCLK
                | WM_XBUTTONDOWN
                | WM_XBUTTONDBLCLK
                | WM_NCLBUTTONDOWN
                | WM_NCRBUTTONDOWN
                | WM_NCMBUTTONDOWN
                | WM_NCXBUTTONDOWN
        )
    }

    fn watched_root(target: HWND) -> Option<u64> {
        if target.is_null() {
            return None;
        }
        let watched: Vec<u64> = PRESS_WINDOWS.lock().unwrap().iter().copied().collect();
        watched.into_iter().find(|&w| {
            w != 0 && {
                let h = hwnd(w);
                h == target || unsafe { IsChild(h, target) } != 0
            }
        })
    }

    unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 && lparam != 0 {
            let msg = wparam as u32;
            if is_button_down_msg(msg) {
                let target = unsafe { (*(lparam as *const MOUSEHOOKSTRUCT)).hwnd };
                if let Some(w) = watched_root(target) {
                    let seq = PRESS.lock().unwrap().record_button_down(w, msg, std::time::Instant::now());
                    if seq != 0 {
                        super::notify_press_wake(w);
                    }
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }

    unsafe extern "system" fn callwnd_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 && lparam != 0 {
            let cwp = unsafe { &*(lparam as *const CWPSTRUCT) };
            if cwp.message == WM_MOUSEACTIVATE {
                let mouse_msg = ((cwp.lParam as usize >> 16) & 0xffff) as u32;
                if is_button_down_msg(mouse_msg)
                    && let Some(w) = watched_root(cwp.hwnd)
                {
                    PRESS.lock().unwrap().record_activate(w, mouse_msg, std::time::Instant::now());
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }

    unsafe extern "system" fn callwndret_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 && lparam != 0 {
            let cwpr = unsafe { &*(lparam as *const CWPRETSTRUCT) };
            if cwpr.message == WM_MOUSEACTIVATE {
                let mouse_msg = ((cwpr.lParam as usize >> 16) & 0xffff) as u32;
                if is_button_down_msg(mouse_msg)
                    && let Some(w) = watched_root(cwpr.hwnd)
                {
                    let outcome = match cwpr.lResult {
                        x if x == MA_NOACTIVATEANDEAT as LRESULT => MouseActivateOutcome::NoActivateAndEat,
                        x if x == MA_ACTIVATEANDEAT as LRESULT => MouseActivateOutcome::ActivateAndEat,
                        _ => MouseActivateOutcome::DeliverDown,
                    };
                    let confirmed = PRESS.lock().unwrap().complete_activate(w, mouse_msg, outcome, std::time::Instant::now());
                    if confirmed {
                        super::notify_press_wake(w);
                    }
                }
            }
        }
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
    }

    /// Hooks CEF's UI thread (`WH_MOUSE` + `WH_CALLWNDPROC` + `WH_CALLWNDPROCRET`) so clicks on any descendant HWND
    /// (including `WM_MOUSEACTIVATE` before button-down delivery) are recorded before CEF focus callbacks run.
    pub fn watch_native(w: u64) {
        if w == 0 { return }
        let tid = unsafe { GetWindowThreadProcessId(hwnd(w), std::ptr::null_mut()) };
        if tid == 0 { return }
        let mut hooked = HOOKED_THREADS.lock().unwrap();
        let ok = if let Some((_, _, _, set)) = hooked.get_mut(&tid) {
            set.insert(w);
            true
        } else {
            let mh = unsafe { SetWindowsHookExW(WH_MOUSE, Some(mouse_hook), std::ptr::null_mut(), tid) };
            if mh.is_null() {
                eprintln!("GPUI_LOG {}", serde_json::json!({"cef_press_watcher": "could not hook mouse events", "tid": tid}));
                false
            } else {
                let ch = unsafe { SetWindowsHookExW(WH_CALLWNDPROC, Some(callwnd_hook), std::ptr::null_mut(), tid) };
                if ch.is_null() {
                    unsafe { UnhookWindowsHookEx(mh); }
                    eprintln!("GPUI_LOG {}", serde_json::json!({"cef_press_watcher": "could not hook activate events", "tid": tid}));
                    false
                } else {
                    let rh = unsafe { SetWindowsHookExW(WH_CALLWNDPROCRET, Some(callwndret_hook), std::ptr::null_mut(), tid) };
                    if rh.is_null() {
                        unsafe {
                            UnhookWindowsHookEx(ch);
                            UnhookWindowsHookEx(mh);
                        }
                        eprintln!("GPUI_LOG {}", serde_json::json!({"cef_press_watcher": "could not hook activate return events", "tid": tid}));
                        false
                    } else {
                        let mut set = HashSet::new();
                        set.insert(w);
                        hooked.insert(tid, (mh as usize, ch as usize, rh as usize, set));
                        true
                    }
                }
            }
        };
        drop(hooked);
        if ok {
            PRESS_WINDOWS.lock().unwrap().insert(w);
        }
    }

    pub fn unwatch_native(w: u64) {
        if w == 0 { return }
        PRESS_WINDOWS.lock().unwrap().remove(&w);
        PRESS.lock().unwrap().drop_target(w);
        super::unregister_press_wake(w);
        let to_unhook: Vec<(usize, usize, usize)> = {
            let mut hooked = HOOKED_THREADS.lock().unwrap();
            let mut drop_tids = Vec::new();
            let mut hooks = Vec::new();
            for (&tid, (mh, ch, rh, set)) in hooked.iter_mut() {
                if set.remove(&w) && set.is_empty() {
                    drop_tids.push(tid);
                    hooks.push((*mh, *ch, *rh));
                }
            }
            for tid in drop_tids {
                hooked.remove(&tid);
            }
            hooks
        };
        for (mh, ch, rh) in to_unhook {
            unsafe {
                UnhookWindowsHookEx(mh as HHOOK);
                UnhookWindowsHookEx(ch as HHOOK);
                UnhookWindowsHookEx(rh as HHOOK);
            }
        }
    }

    /// A recent press is consumed once per callback kind for the native child that received it.
    pub fn pressed_in(w: u64, cb: FocusCallback) -> PressProbe {
        let mut press = PRESS.lock().unwrap();
        if let Some(seq) = press.consume(w, cb, std::time::Instant::now()) {
            return PressProbe::Pressed(seq);
        }
        let blocked = press.is_unwatched_blocked(w);
        drop(press);
        if blocked || PRESS_WINDOWS.lock().unwrap().contains(&w) {
            PressProbe::NotPressed
        } else {
            PressProbe::Unwatched
        }
    }

    pub fn clear_presses(frames: &[u64]) -> u64 {
        let mut unique: Vec<u64> = Vec::with_capacity(frames.len());
        for &w in frames {
            if w != 0 && !unique.contains(&w) {
                unique.push(w);
            }
        }
        if unique.is_empty() {
            return next_focus_seq();
        }
        let unwatched: Vec<u64> = {
            let pw = PRESS_WINDOWS.lock().unwrap();
            unique.iter().copied().filter(|w| !pw.contains(w)).collect()
        };
        let held = !unwatched.is_empty() && any_button_held();
        let (cutoff, spawn_gen) = {
            let mut press = PRESS.lock().unwrap();
            for &w in &unique {
                press.clear_target(w);
            }
            let mut any_blocked = false;
            if held {
                for &w in &unwatched {
                    if press.block_unwatched_until_release(w).is_some() {
                        any_blocked = true;
                    }
                }
            }
            (next_focus_seq(), any_blocked.then(|| press.unwatched_block_gen()))
        };
        if let Some(mut block_gen) = spawn_gen {
            let _ = std::thread::Builder::new().name("cef-unwatched-rel".into()).spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    if !any_button_held() {
                        let mut press = PRESS.lock().unwrap();
                        if press.clear_unwatched_release_block_if(block_gen) {
                            break;
                        }
                        block_gen = press.unwatched_block_gen();
                    }
                }
            });
        }
        cutoff
    }

    pub fn clear_press(w: u64) {
        if w != 0 {
            let _ = clear_presses(&[w]);
        }
    }

    pub fn fresh_press_seq_after(w: u64, after_seq: u64) -> Option<u64> {
        PRESS.lock().unwrap().fresh_press_seq_after(w, after_seq, std::time::Instant::now())
    }

    pub fn clear_press_up_to(w: u64, max_seq: u64) {
        PRESS.lock().unwrap().clear_target_up_to(w, max_seq);
    }

    pub fn close(w: u64) {
        unwatch_native(w);
        unsafe { DestroyWindow(hwnd(w)); }
    }

    /// Every window under `h` (class, rect in the screen, visible, style), for the report.
    pub fn tree(h: Host) -> Vec<String> {
        let Host::Win(p) = h else { return vec![] };
        unsafe extern "system" fn cb(w: HWND, l: LPARAM) -> i32 {
            let out = unsafe { &mut *(l as *mut Vec<String>) };
            let mut cls = [0u16; 64];
            let n = unsafe { GetClassNameW(w, cls.as_mut_ptr(), 64) };
            let mut r = unsafe { std::mem::zeroed::<RECT>() };
            unsafe { GetWindowRect(w, &mut r); }
            out.push(format!("{:x} {} [{},{} {}x{}] vis={} style={:x} ex={:x} parent={:x}", w as usize, String::from_utf16_lossy(&cls[..n.max(0) as usize]), r.left, r.top, r.right - r.left, r.bottom - r.top, // i18n-ok: a diagnostic in the e2e report
                unsafe { IsWindowVisible(w) }, unsafe { GetWindowLongPtrW(w, GWL_STYLE) }, unsafe { GetWindowLongPtrW(w, GWL_EXSTYLE) }, unsafe { GetParent(w) as usize }));
            1
        }
        let mut out: Vec<String> = Vec::new();
        unsafe {
            let mut r = std::mem::zeroed::<RECT>();
            GetWindowRect(p as HWND, &mut r);
            out.push(format!("client {:x} [{},{} {}x{}] ex={:x}", p, r.left, r.top, r.right - r.left, r.bottom - r.top, GetWindowLongPtrW(p as HWND, GWL_EXSTYLE))); // i18n-ok: a diagnostic in the e2e report
            EnumChildWindows(p as HWND, Some(cb), &mut out as *mut _ as LPARAM);
        }
        out
    }

    /// Fallback when `watch_native` is unavailable: requires both a held mouse button and the pointer inside `w`.
    pub fn button_down(w: u64) -> bool {
        if w == 0 || PRESS.lock().unwrap().is_unwatched_blocked(w) || !any_button_held() { return false }
        let mut pt = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut pt) } == 0 { return false }
        let at = unsafe { WindowFromPoint(pt) };
        let inside = !at.is_null() && (at == hwnd(w) || unsafe { IsChild(hwnd(w), at) } != 0);
        inside && !PRESS.lock().unwrap().is_unwatched_blocked(w)
    }

    fn hwnd(w: u64) -> HWND { w as isize as HWND }
    /// Asynchronous: the child belongs to CEF's UI thread, and a synchronous call from here would wait on it.
    pub fn place(w: u64, r: R) { unsafe { SetWindowPos(hwnd(w), std::ptr::null_mut(), r.x, r.y, r.w.max(1), r.h.max(1), SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS); } }
    pub fn show(w: u64, on: bool) { unsafe { ShowWindowAsync(hwnd(w), if on { SW_SHOWNA } else { SW_HIDE }); } }
    /// CEF's SetFocus does it on Windows (its UI thread owns the child); nothing more here.
    pub fn focus_child(_w: u64) {}
    /// The client's window is gpui's, on this (its) thread.
    /// When `target != 0`, only returns Win32 focus to the host if `target` (or a child of `target`)
    /// currently holds foreground keyboard focus, so a background tab yank never steals from an active tab.
    pub fn focus_host(h: Host, target: u64) {
        if let Host::Win(p) = h {
            if target != 0 {
                let t = hwnd(target);
                let mut gti = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
                let focused = if unsafe { GetGUIThreadInfo(0, &mut gti) } != 0 { gti.hwndFocus } else { unsafe { GetFocus() } };
                if focused.is_null() || (focused != t && unsafe { IsChild(t, focused) } == 0) {
                    return;
                }
            }
            unsafe { SetFocus(p as HWND); }
        }
    }
    pub fn geometry(w: u64) -> Option<(R, bool)> {
        unsafe {
            let mut rc = std::mem::zeroed::<RECT>();
            if GetWindowRect(hwnd(w), &mut rc) == 0 { return None }
            let parent = GetParent(hwnd(w));
            let mut pt = [POINT { x: rc.left, y: rc.top }];
            ScreenToClient(parent, pt.as_mut_ptr());
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
    use crate::viewers::cef_focus::{next_focus_seq, FocusCallback, PressProbe, PressTracker};
    use std::collections::HashSet;
    use std::ffi::{c_void, CStr};
    use std::sync::{LazyLock, Mutex, OnceLock};

    static PRESS: Mutex<PressTracker> = Mutex::new(PressTracker::new());
    static PRESS_WINDOWS: LazyLock<Mutex<HashSet<u64>>> = LazyLock::new(Default::default);
    static WATCHER: OnceLock<()> = OnceLock::new();

    pub fn pixel(_w: u64, _x: i32, _y: i32) -> Option<[u8; 3]> { None }
    pub fn inner_size(_w: u64) -> Option<(i32, i32)> { None }
    pub fn container(_h: Host) -> Option<Host> { None }
    pub fn place_in(_frame: u64, _w: u64, _r: Option<R>) {}
    pub fn destroy(_frame: u64) {}

    #[repr(C)] #[derive(Clone, Copy)] struct Rect { x: f64, y: f64, w: f64, h: f64 }
    #[repr(C)] #[derive(Clone, Copy)] struct Pt { x: f64, y: f64 }
    #[link(name = "objc")] unsafe extern "C" {
        fn sel_registerName(n: *const std::ffi::c_char) -> *mut c_void;
        fn objc_getClass(n: *const std::ffi::c_char) -> *mut c_void;
        fn objc_msgSend();
    }
    fn sel(n: &CStr) -> *mut c_void { unsafe { sel_registerName(n.as_ptr()) } }
    unsafe fn send0(o: *mut c_void, s: &CStr) -> *mut c_void { unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) } }
    unsafe fn send_void(o: *mut c_void, s: &CStr) { unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) } }
    unsafe fn send_usize(o: *mut c_void, s: &CStr) -> usize { unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) } }
    #[cfg(target_arch = "aarch64")]
    type ObjCBool = bool;
    #[cfg(not(target_arch = "aarch64"))]
    type ObjCBool = i8;
    #[cfg(target_arch = "aarch64")]
    #[inline]
    fn to_objc_bool(v: bool) -> ObjCBool { v }
    #[cfg(not(target_arch = "aarch64"))]
    #[inline]
    fn to_objc_bool(v: bool) -> ObjCBool { v as i8 }
    #[cfg(target_arch = "aarch64")]
    #[inline]
    fn from_objc_bool(v: ObjCBool) -> bool { v }
    #[cfg(not(target_arch = "aarch64"))]
    #[inline]
    fn from_objc_bool(v: ObjCBool) -> bool { v != 0 }
    unsafe fn send_bool(o: *mut c_void, s: &CStr) -> bool { unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> ObjCBool = std::mem::transmute(objc_msgSend as *const ()); from_objc_bool(f(o, sel(s))) } }
    unsafe fn send_obj_bool(o: *mut c_void, s: &CStr, a: *mut c_void) -> bool { unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> ObjCBool = std::mem::transmute(objc_msgSend as *const ()); from_objc_bool(f(o, sel(s), a)) } }
    unsafe fn send_flag(o: *mut c_void, s: &CStr, a: bool) { unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, ObjCBool) = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), to_objc_bool(a)) } }
    unsafe fn send_rect(o: *mut c_void, s: &CStr, r: Rect) { unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, Rect) = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), r) } }
    unsafe fn point_of(o: *mut c_void, s: &CStr) -> Pt { unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> Pt = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) } }
    unsafe fn convert_point(o: *mut c_void, s: &CStr, pt: Pt, from: *mut c_void) -> Pt { unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, Pt, *mut c_void) -> Pt = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), pt, from) } }
    unsafe fn convert_point_from_screen(win: *mut c_void, pt: Pt) -> Pt { unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, Pt) -> Pt = std::mem::transmute(objc_msgSend as *const ()); f(win, sel(c"convertPointFromScreen:"), pt) } } // i18n-ok: an Objective-C selector
    unsafe fn send_point_obj(o: *mut c_void, s: &CStr, pt: Pt) -> *mut c_void { unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void, Pt) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s), pt) } }
    #[cfg(target_arch = "x86_64")] #[link(name = "objc")] unsafe extern "C" { fn objc_msgSend_stret(); }
    unsafe fn rect_of(o: *mut c_void, s: &CStr) -> Rect {
        #[cfg(target_arch = "aarch64")] unsafe { let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> Rect = std::mem::transmute(objc_msgSend as *const ()); f(o, sel(s)) }
        #[cfg(target_arch = "x86_64")] unsafe { let mut r = Rect { x: 0., y: 0., w: 0., h: 0. }; let f: unsafe extern "C" fn(*mut Rect, *mut c_void, *mut c_void) = std::mem::transmute(objc_msgSend_stret as *const ()); f(&mut r, o, sel(s)); r }
    }
    fn v(w: u64) -> *mut c_void { w as usize as *mut c_void }

    /// Returns the actual NSView AppKit hit-tests at window-base point `pt`, honoring overlapping views and visibility.
    unsafe fn hit_view(win: *mut c_void, pt: Pt) -> *mut c_void {
        unsafe {
            let content = send0(win, c"contentView"); // i18n-ok: an Objective-C selector
            if content.is_null() { return std::ptr::null_mut() }
            let superview = send0(content, c"superview");
            let hit_pt = if superview.is_null() { pt } else { convert_point(superview, c"convertPoint:fromView:", pt, std::ptr::null_mut()) }; // i18n-ok: an Objective-C selector
            send_point_obj(content, c"hitTest:", hit_pt) // i18n-ok: an Objective-C selector
        }
    }

    fn on_mac_mouse_down(event: *mut c_void) {
        if event.is_null() { return }
        let watched: Vec<u64> = PRESS_WINDOWS.lock().unwrap().iter().copied().collect();
        if watched.is_empty() { return }
        unsafe {
            let ev_win = send0(event, c"window");
            if ev_win.is_null() {
                PRESS.lock().unwrap().clear_all();
                return;
            }
            let pt = point_of(event, c"locationInWindow"); // i18n-ok: an Objective-C selector
            let hit = hit_view(ev_win, pt);
            if !hit.is_null() {
                for w in watched {
                    if w == 0 { continue }
                    let view = v(w);
                    if send0(view, c"window") == ev_win
                        && !send_bool(view, c"isHiddenOrHasHiddenAncestor") // i18n-ok: an Objective-C selector
                        && send_obj_bool(hit, c"isDescendantOf:", view) // i18n-ok: an Objective-C selector
                    {
                        let seq = PRESS.lock().unwrap().record(w, std::time::Instant::now());
                        if seq != 0 {
                            super::notify_press_wake(w);
                        }
                        return;
                    }
                }
            }
            PRESS.lock().unwrap().clear_all();
        }
    }

    pub fn watch_native(w: u64) {
        if w == 0 { return }
        WATCHER.get_or_init(|| gpui::set_mac_mouse_down_observer(Some(on_mac_mouse_down)));
        PRESS_WINDOWS.lock().unwrap().insert(w);
    }

    pub fn unwatch_native(w: u64) {
        if w == 0 { return }
        PRESS_WINDOWS.lock().unwrap().remove(&w);
        PRESS.lock().unwrap().drop_target(w);
        super::unregister_press_wake(w);
    }

    pub fn pressed_in(w: u64, cb: FocusCallback) -> PressProbe {
        let mut press = PRESS.lock().unwrap();
        if let Some(seq) = press.consume(w, cb, std::time::Instant::now()) {
            return PressProbe::Pressed(seq);
        }
        let blocked = press.is_unwatched_blocked(w);
        drop(press);
        if blocked || PRESS_WINDOWS.lock().unwrap().contains(&w) {
            PressProbe::NotPressed
        } else {
            PressProbe::Unwatched
        }
    }

    pub fn clear_presses(frames: &[u64]) -> u64 {
        let mut press = PRESS.lock().unwrap();
        for &w in frames {
            if w != 0 {
                press.clear_target(w);
            }
        }
        next_focus_seq()
    }

    pub fn clear_press(w: u64) {
        if w != 0 {
            let _ = clear_presses(&[w]);
        }
    }

    pub fn fresh_press_seq_after(w: u64, after_seq: u64) -> Option<u64> {
        PRESS.lock().unwrap().fresh_press_seq_after(w, after_seq, std::time::Instant::now())
    }

    pub fn clear_press_up_to(w: u64, max_seq: u64) {
        PRESS.lock().unwrap().clear_target_up_to(w, max_seq);
    }

    pub fn close(w: u64) {
        unwatch_native(w);
        unsafe { send_void(v(w), c"removeFromSuperview"); } // i18n-ok: an Objective-C selector
    }

    /// Fallback when `watch_native` is unavailable: requires both a pressed mouse button and hit-testing inside `w`.
    pub fn button_down(w: u64) -> bool {
        if w == 0 { return false }
        unsafe {
            let cls = objc_getClass(c"NSEvent".as_ptr()); // i18n-ok: an Objective-C class name
            if cls.is_null() || send_usize(cls, c"pressedMouseButtons") == 0 { return false } // i18n-ok: an Objective-C selector
            let view = v(w);
            let win = send0(view, c"window");
            if win.is_null() || send_bool(view, c"isHiddenOrHasHiddenAncestor") { return false } // i18n-ok: an Objective-C selector
            let screen_pt = point_of(cls, c"mouseLocation"); // i18n-ok: an Objective-C selector
            let win_pt = convert_point_from_screen(win, screen_pt);
            let hit = hit_view(win, win_pt);
            !hit.is_null() && send_obj_bool(hit, c"isDescendantOf:", view) // i18n-ok: an Objective-C selector
        }
    }

    /// `r` is in points from the parent's top-left; an unflipped parent (gpui's view) counts y from its bottom.
    pub fn place(w: u64, r: R) {
        unsafe {
            let parent = send0(v(w), c"superview"); if parent.is_null() { return }
            let y = if send_bool(parent, c"isFlipped") { r.y as f64 } else { rect_of(parent, c"bounds").h - (r.y + r.h) as f64 }; // i18n-ok: an Objective-C selector
            send_rect(v(w), c"setFrame:", Rect { x: r.x as f64, y, w: r.w.max(1) as f64, h: r.h.max(1) as f64 }); // i18n-ok: an Objective-C selector
        }
    }
    pub fn show(w: u64, on: bool) { unsafe { send_flag(v(w), c"setHidden:", !on); } } // i18n-ok: an Objective-C selector
    /// Moves firstResponder onto CEF's view unless an inner Chromium subview (e.g. `RenderWidgetHostViewCocoa`) already holds it.
    pub fn focus_child(w: u64) {
        if w == 0 { return }
        unsafe {
            let view = v(w);
            let win = send0(view, c"window");
            if win.is_null() { return }
            let fr = send0(win, c"firstResponder"); // i18n-ok: an Objective-C selector
            let ns_view = objc_getClass(c"NSView".as_ptr()); // i18n-ok: an Objective-C class name
            if !fr.is_null() && !ns_view.is_null()
                && send_obj_bool(fr, c"isKindOfClass:", ns_view) // i18n-ok: an Objective-C selector
                && send_obj_bool(fr, c"isDescendantOf:", view) // i18n-ok: an Objective-C selector
            {
                return;
            }
            let _ = send_obj_bool(win, c"makeFirstResponder:", view); // i18n-ok: an Objective-C selector
        }
    }
    /// Moves firstResponder back to gpui's host view. When `target != 0`, only does so if `target`
    /// (or an NSView descendant of `target`) currently holds firstResponder, so a background tab yank never steals from an active tab.
    pub fn focus_host(h: Host, target: u64) {
        if let Host::Mac(p) = h {
            unsafe {
                let win = send0(p as *mut c_void, c"window"); // i18n-ok: an Objective-C selector
                if win.is_null() { return }
                if target != 0 {
                    let fr = send0(win, c"firstResponder"); // i18n-ok: an Objective-C selector
                    let ns_view = objc_getClass(c"NSView".as_ptr()); // i18n-ok: an Objective-C class name
                    let owns = !fr.is_null() && !ns_view.is_null()
                        && send_obj_bool(fr, c"isKindOfClass:", ns_view) // i18n-ok: an Objective-C selector
                        && send_obj_bool(fr, c"isDescendantOf:", v(target)); // i18n-ok: an Objective-C selector
                    if !owns { return }
                }
                let _ = send_obj_bool(win, c"makeFirstResponder:", p as *mut c_void); // i18n-ok: an Objective-C selector
            }
        }
    }
    pub fn geometry(w: u64) -> Option<(R, bool)> {
        unsafe {
            let parent = send0(v(w), c"superview"); if parent.is_null() { return None }
            let f = rect_of(v(w), c"frame"); let y = if send_bool(parent, c"isFlipped") { f.y } else { rect_of(parent, c"bounds").h - f.y - f.h }; // i18n-ok: an Objective-C selector
            Some((R { x: f.x as i32, y: y as i32, w: f.w as i32, h: f.h as i32 }, !send_bool(v(w), c"isHidden"))) // i18n-ok: an Objective-C selector
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

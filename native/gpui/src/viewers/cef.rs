//! Embedded Chromium (CEF, windowless/OSR) for the Browser tab, behind the `cef` cargo feature (docs/browser-cef.md).
//!
//! One process-wide engine (`Engine::get`) is started on the first Browser tab shown: `cef::initialize` with the multi-threaded message loop (CEF
//! runs its own UI thread; the callbacks below arrive there and only touch `Tab`'s mutexes) and `remote_debugging_port` on a loopback port the agent
//! drives over CDP. CEF's helper processes are this same executable re-run with `--type=...`: `subprocess()` must be the first thing `main` does.
//! A `Tab` is one browser: its newest frame (BGRA, copied out of `on_paint`), address, title, loading state, cursor and the popups it asked for,
//! plus the input methods the gpui page calls (mouse, wheel, key, text, IME, focus, resize, navigation).
use cef::{rc::Rc as _, *};
pub use cef::{CefStringList, ImplFileDialogCallback};
use super::downloads::{bring_to_downloads, safe_name, unique_path};
use std::{path::{Path, PathBuf}, sync::{atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering}, Arc, Mutex, OnceLock}};

/// The CEF version the `cef` crate in Cargo.toml was generated for; the runtime directory is `<cache>/backplane/cef-runtime/<VERSION>`.
pub const VERSION: &str = "154.0.34";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cur { Default, Hand, IBeam, Cross, Wait, Help, Move, ResizeH, ResizeV, ResizeNesw, ResizeNwse, NotAllowed, None }

pub struct Frame { pub w: u32, pub h: u32, pub bgra: Vec<u8> }
#[derive(Default)]
pub struct State { pub frame: Option<Frame>, pub fresh: bool, pub url: String, pub title: String, pub loading: bool, pub back: bool, pub forward: bool, pub popups: Vec<String>, pub popup_tabs: Vec<Arc<Tab>>, pub dialogs: Vec<Dialog>, pub download: String, pub closed: bool, pub load_error: String }
/// A file dialog CEF asked for (OSR has none of its own): the page shows gpui's picker and answers through `cb`.
pub struct Dialog { pub save: bool, pub multiple: bool, pub folder: bool, pub title: String, pub cb: FileDialogCallback }
unsafe impl Send for Dialog {}
struct B(Browser);
unsafe impl Send for B {}

/// One browser. All fields are shared with CEF's callbacks (its UI thread) and the gpui page (the main thread).
pub struct Tab {
    pub st: Mutex<State>,
    browser: Mutex<Option<B>>,
    cursor: Mutex<Cur>,
    /// logical (DIP) size of the view and the device scale factor, times 1000 for the scale.
    w: AtomicU32, h: AtomicU32, scale_milli: AtomicU32,
    wake: async_channel::Sender<()>,
    hidden: AtomicBool,
    pub id: AtomicI32,
    /// how many frames CEF has painted (the e2e's frame rate)
    pub paints: std::sync::atomic::AtomicU64,
    /// a resize in flight (when, the physical width asked for) and how long the last one took to reach a painted frame (ms, -1 none)
    resize_at: Mutex<Option<(std::time::Instant, u32)>>, resize_done: Mutex<f64>,
}

/// The platform's cursor handle as the `cef` crate's DisplayHandler takes it (unused: the page draws gpui's own cursor from the type).
#[cfg(target_os = "macos")] type CursorHandle = *mut u8;
#[cfg(not(target_os = "macos"))] type CursorHandle = sys::cef_cursor_handle_t;

fn s(x: Option<&CefString>) -> String { x.map(|c| c.to_string()).unwrap_or_default() }

wrap_render_handler! {
    struct RenderH { tab: Arc<Tab> }
    impl RenderHandler {
        fn view_rect(&self, _b: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(r) = rect { r.x = 0; r.y = 0; r.width = self.tab.w.load(Ordering::Relaxed).max(1) as _; r.height = self.tab.h.load(Ordering::Relaxed).max(1) as _; }
        }
        fn screen_info(&self, _b: Option<&mut Browser>, info: Option<&mut ScreenInfo>) -> ::std::os::raw::c_int {
            if let Some(i) = info { i.device_scale_factor = self.tab.scale_milli.load(Ordering::Relaxed) as f32 / 1000.0; return true as _; }
            false as _
        }
        fn on_paint(&self, _b: Option<&mut Browser>, ty: PaintElementType, _dirty: Option<&[Rect]>, buffer: *const u8, width: ::std::os::raw::c_int, height: ::std::os::raw::c_int) {
            if ty != PaintElementType::default() || buffer.is_null() || width <= 0 || height <= 0 { return }
            self.tab.paints.fetch_add(1, Ordering::Relaxed);
            { let mut r = self.tab.resize_at.lock().unwrap(); if let Some((t0, w)) = *r { if width as u32 == w { *self.tab.resize_done.lock().unwrap() = t0.elapsed().as_secs_f64() * 1e3; *r = None; } } }
            let bytes = unsafe { std::slice::from_raw_parts(buffer, width as usize * height as usize * 4) }.to_vec();
            { let mut st = self.tab.st.lock().unwrap(); st.frame = Some(Frame { w: width as u32, h: height as u32, bgra: bytes }); st.fresh = true; }
            let _ = self.tab.wake.try_send(());
        }
    }
}
wrap_display_handler! {
    struct DisplayH { tab: Arc<Tab> }
    impl DisplayHandler {
        fn on_address_change(&self, _b: Option<&mut Browser>, frame: Option<&mut Frame_>, url: Option<&CefString>) {
            if frame.map_or(false, |f| f.is_main() == 1) { eprintln!("GPUI_LOG {{\"cef_address\":{}}}", serde_json::Value::String(s(url))); self.tab.st.lock().unwrap().url = s(url); let _ = self.tab.wake.try_send(()); }
        }
        fn on_title_change(&self, _b: Option<&mut Browser>, title: Option<&CefString>) { self.tab.st.lock().unwrap().title = s(title); let _ = self.tab.wake.try_send(()); }
        fn on_cursor_change(&self, _b: Option<&mut Browser>, _c: CursorHandle, ty: CursorType, _i: Option<&CursorInfo>) -> ::std::os::raw::c_int {
            *self.tab.cursor.lock().unwrap() = cur_of(ty); let _ = self.tab.wake.try_send(()); true as _
        }
    }
}
wrap_load_handler! {
    struct LoadH { tab: Arc<Tab> }
    impl LoadHandler {
        fn on_loading_state_change(&self, _b: Option<&mut Browser>, loading: ::std::os::raw::c_int, back: ::std::os::raw::c_int, fwd: ::std::os::raw::c_int) {
            { let mut st = self.tab.st.lock().unwrap(); st.loading = loading == 1; st.back = back == 1; st.forward = fwd == 1; if loading == 1 { st.load_error.clear(); } }
            let _ = self.tab.wake.try_send(());
        }
        fn on_load_error(&self, _b: Option<&mut Browser>, frame: Option<&mut Frame_>, code: Errorcode, text: Option<&CefString>, url: Option<&CefString>) {
            if frame.map_or(false, |f| f.is_main() == 1) && code.get_raw() as i32 != -3 { // -3: aborted (a navigation that replaced this one)
                self.tab.st.lock().unwrap().load_error = format!("{} ({}): {}", s(text), code.get_raw() as i32, s(url)); let _ = self.tab.wake.try_send(());
            }
        }
    }
}
wrap_life_span_handler! {
    struct LifeH { tab: Arc<Tab> }
    impl LifeSpanHandler {
        fn on_before_popup(&self, _b: Option<&mut Browser>, _f: Option<&mut Frame_>, _id: ::std::os::raw::c_int, _url: Option<&CefString>, _name: Option<&CefString>,
            _disp: WindowOpenDisposition, _gesture: ::std::os::raw::c_int, _features: Option<&PopupFeatures>, wi: Option<&mut WindowInfo>, client: Option<&mut Option<Client>>,
            settings: Option<&mut BrowserSettings>, _extra: Option<&mut Option<DictionaryValue>>, _nojs: Option<&mut ::std::os::raw::c_int>) -> ::std::os::raw::c_int {
            eprintln!("GPUI_LOG {{\"cef_popup\":{}}}", serde_json::Value::String(format!("{:?} {}", _disp, s(_url))));
            if let Some(st) = settings { st.windowless_frame_rate = 30; }
            // a new window keeps its opener (window.opener, postMessage, window.close): CEF makes it, windowless, with our own handlers; the page shows it as a new tab
            // a link that saves to disk is a download of this page, not a window
            if _disp == WindowOpenDisposition::SAVE_TO_DISK { if let (Some(b), Some(u)) = (_b, _url) { if let Some(h) = b.host() { h.start_download(Some(u)); } } return true as _ }
            let (Some(wi), Some(client)) = (wi, client) else { return true as _ };
            let (w, h) = self.tab.size(); let scale = self.tab.scale_milli.load(Ordering::Relaxed) as f32 / 1000.0;
            let tab = Tab::detached("", w, h, scale, self.tab.wake.clone());
            *client = Some(client_for(&tab)); wi.windowless_rendering_enabled = true as _; wi.runtime_style = RuntimeStyle::ALLOY;
            self.tab.st.lock().unwrap().popup_tabs.push(tab); let _ = self.tab.wake.try_send(());
            false as _
        }
        fn on_after_created(&self, b: Option<&mut Browser>) { if let Some(b) = b { eprintln!("GPUI_LOG {{\"cef_created\":{}}}", b.identifier()); if let Some(h) = b.host() { h.was_hidden(false as _); h.was_resized(); h.invalidate(PaintElementType::default()); } self.tab.id.store(b.identifier(), Ordering::Relaxed); *self.tab.browser.lock().unwrap() = Some(B(b.clone())); let _ = self.tab.wake.try_send(()); } }
        fn do_close(&self, _b: Option<&mut Browser>) -> ::std::os::raw::c_int { false as _ }
        fn on_before_close(&self, _b: Option<&mut Browser>) { *self.tab.browser.lock().unwrap() = None; self.tab.st.lock().unwrap().closed = true; let _ = self.tab.wake.try_send(()); }
    }
}
wrap_dialog_handler! {
    struct DialogH { tab: Arc<Tab> }
    impl DialogHandler {
        fn on_file_dialog(&self, _b: Option<&mut Browser>, mode: FileDialogMode, title: Option<&CefString>, _def: Option<&CefString>, _f: Option<&mut CefStringList>, _e: Option<&mut CefStringList>, _d: Option<&mut CefStringList>, cb: Option<&mut FileDialogCallback>) -> ::std::os::raw::c_int {
            let Some(cb) = cb else { return false as _ };
            self.tab.st.lock().unwrap().dialogs.push(Dialog { save: mode == FileDialogMode::SAVE, multiple: mode == FileDialogMode::OPEN_MULTIPLE, folder: mode == FileDialogMode::OPEN_FOLDER, title: s(title), cb: cb.clone() });
            let _ = self.tab.wake.try_send(()); true as _
        }
    }
}
wrap_download_handler! {
    struct DownloadH { tab: Arc<Tab> }
    impl DownloadHandler {
        // the trait's default answers no, which silently blocks every download
        fn can_download(&self, _b: Option<&mut Browser>, _url: Option<&CefString>, _method: Option<&CefString>) -> ::std::os::raw::c_int { eprintln!("GPUI_LOG {{\"cef_can_download\":{}}}", serde_json::Value::String(s(_url))); true as _ }
        fn on_before_download(&self, _b: Option<&mut Browser>, _item: Option<&mut DownloadItem>, name: Option<&CefString>, cb: Option<&mut BeforeDownloadCallback>) -> ::std::os::raw::c_int {
            // no dialog: the file goes to the person's Downloads folder (never over a file), the page says so when it is done
            let dir = std::env::var_os("XDG_DOWNLOAD_DIR").map(PathBuf::from).or_else(|| crate::plat::user_home().map(|h| h.join("Downloads"))).unwrap_or_else(std::env::temp_dir);
            let _ = std::fs::create_dir_all(&dir);
            let path = unique_path(&dir, &safe_name(&s(name)));
            eprintln!("GPUI_LOG {{\"cef_download\":{}}}", serde_json::Value::String(path.to_string_lossy().into_owned()));
            if let Some(cb) = cb { cb.cont(Some(&CefString::from(path.to_string_lossy().as_ref())), false as _); }
            self.tab.st.lock().unwrap().download = format!("Downloading {}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()); let _ = self.tab.wake.try_send(()); true as _
        }
        fn on_download_updated(&self, _b: Option<&mut Browser>, item: Option<&mut DownloadItem>, _cb: Option<&mut DownloadItemCallback>) {
            let Some(item) = item else { return };
            let full = PathBuf::from(CefString::from(&item.full_path()).to_string());
            let name = full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            // with the hub's browser helper attached (DevTools), the engine names and places the file itself and `on_before_download` is never asked: a finished file that is not
            // in the person's Downloads folder is brought there, under its own name
            if item.is_complete() == 1 && !full.exists() { return } // already brought to Downloads by an earlier callback
            let full = if item.is_complete() == 1 { bring_to_downloads(&full, &CefString::from(&item.suggested_file_name()).to_string()) } else { full };
            let name = full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(name);
            let msg = if item.is_complete() == 1 { format!("Downloaded {name} to {}", full.parent().map(|p| p.display().to_string()).unwrap_or_default()) } else if item.is_canceled() == 1 || item.is_interrupted() == 1 { format!("Download of {name} stopped") } else { format!("Downloading {name} {}%", item.percent_complete().max(0)) };
            self.tab.st.lock().unwrap().download = msg; let _ = self.tab.wake.try_send(());
        }
    }
}
wrap_client! {
    struct ClientH { render: RenderHandler, display: DisplayHandler, load: LoadHandler, life: LifeSpanHandler, dialog: DialogHandler, download: DownloadHandler }
    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> { Some(self.render.clone()) }
        fn display_handler(&self) -> Option<DisplayHandler> { Some(self.display.clone()) }
        fn load_handler(&self) -> Option<LoadHandler> { Some(self.load.clone()) }
        fn life_span_handler(&self) -> Option<LifeSpanHandler> { Some(self.life.clone()) }
        fn dialog_handler(&self) -> Option<DialogHandler> { Some(self.dialog.clone()) }
        fn download_handler(&self) -> Option<DownloadHandler> { Some(self.download.clone()) }
    }
}
type Frame_ = cef::Frame;
fn client_for(tab: &Arc<Tab>) -> Client { ClientH::new(RenderH::new(tab.clone()), DisplayH::new(tab.clone()), LoadH::new(tab.clone()), LifeH::new(tab.clone()), DialogH::new(tab.clone()), DownloadH::new(tab.clone())) }

fn cur_of(t: CursorType) -> Cur {
    if t == CursorType::HAND { Cur::Hand } else if t == CursorType::IBEAM || t == CursorType::VERTICALTEXT { Cur::IBeam } else if t == CursorType::CROSS { Cur::Cross }
    else if t == CursorType::WAIT || t == CursorType::PROGRESS { Cur::Wait } else if t == CursorType::HELP { Cur::Help } else if t == CursorType::MOVE { Cur::Move }
    else if t == CursorType::EASTWESTRESIZE || t == CursorType::COLUMNRESIZE { Cur::ResizeH } else if t == CursorType::NORTHSOUTHRESIZE || t == CursorType::ROWRESIZE { Cur::ResizeV }
    else if t == CursorType::NORTHEASTSOUTHWESTRESIZE { Cur::ResizeNesw } else if t == CursorType::NORTHWESTSOUTHEASTRESIZE { Cur::ResizeNwse }
    else if t == CursorType::NOTALLOWED || t == CursorType::NODROP { Cur::NotAllowed } else if t == CursorType::NONE { Cur::None } else { Cur::Default }
}

/// macOS runs CEF on the main thread with an external message pump (it has no multi-threaded loop there): CEF says when it wants work through
/// `on_schedule_message_pump_work` (any thread), the delay goes on this channel, and `pump` (cef_page.rs, on gpui's main thread) calls
/// `do_message_loop_work` when it is due. Nothing runs while CEF asks for nothing. On Linux and Windows CEF has its own UI thread and never asks.
static PUMP: OnceLock<(async_channel::Sender<i64>, async_channel::Receiver<i64>)> = OnceLock::new();
fn pump_chan() -> &'static (async_channel::Sender<i64>, async_channel::Receiver<i64>) { PUMP.get_or_init(async_channel::unbounded) }
/// The delays CEF asked for (ms; 0 or less: now).
pub fn pump_requests() -> async_channel::Receiver<i64> { pump_chan().1.clone() }
/// One round of CEF's work on this (the main) thread.
pub fn pump_work() { do_message_loop_work(); }
/// Whether this platform pumps CEF from gpui's main thread.
pub const EXTERNAL_PUMP: bool = cfg!(target_os = "macos");
wrap_browser_process_handler! {
    struct PumpH;
    impl BrowserProcessHandler {
        fn on_schedule_message_pump_work(&self, delay_ms: i64) { let _ = pump_chan().0.try_send(delay_ms); }
    }
}
wrap_app! {
    struct AppH { args: Arc<Vec<(String, String)>> }
    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> { Some(PumpH::new()) }
        fn on_before_command_line_processing(&self, _ty: Option<&CefString>, cl: Option<&mut CommandLine>) {
            let Some(cl) = cl else { return };
            for (k, v) in self.args.iter() { if v.is_empty() { cl.append_switch(Some(&k.as_str().into())); } else { cl.append_switch_with_value(Some(&k.as_str().into()), Some(&v.as_str().into())); } }
        }
    }
}
fn app() -> App { AppH::new(Arc::new(switches())) }
fn switches() -> Vec<(String, String)> {
    let mut v = vec![("use-mock-keychain".into(), String::new()), ("password-store".into(), "basic".into()), ("noerrdialogs".into(), String::new()), ("disable-features".into(), "Translate".into())];
    // Linux: the window is X11 (gpui's x11 backend; on a Wayland desktop it runs through XWayland), and so is Chromium: pinned, so a Wayland session's
    // WAYLAND_DISPLAY never makes it pick its own Wayland platform, which off-screen rendering does not need and which would not share our X display
    #[cfg(target_os = "linux")]
    v.push(("ozone-platform".into(), "x11".into()));
    if std::env::var_os("BACKPLANE_CEF_NO_GPU").is_some() { v.push(("disable-gpu".into(), String::new())); v.push(("disable-gpu-compositing".into(), String::new())); }
    v
}

/// First call of `main`. In a CEF helper process (renderer, gpu, ...) this runs it to its end and returns its exit code; in the browser process it does nothing.
pub fn subprocess() -> Option<i32> {
    // macOS: the engine is a framework loaded at run time; a helper (`--type=...`) loads it before any CEF call, the browser process only when a Browser tab starts it
    #[cfg(target_os = "macos")]
    {
        if !std::env::args().any(|a| a.starts_with("--type=")) { return None }
        let fw = mac::helper_framework()?;
        if !mac::load(&fw) { eprintln!("backplane-gpui: could not load {}", fw.display()); return Some(1) }
    }
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = cef::args::Args::new();
    let cmd = args.as_cmd_line()?;
    if cmd.has_switch(Some(&CefString::from("type"))) != 1 { return None }
    let mut a = app();
    Some(execute_process(Some(args.as_main_args()), Some(&mut a), std::ptr::null_mut()).max(0))
}

/// The runtime directory: `$BACKPLANE_CEF` (the directory, or the `backplane-cef` file in it), else `<cache>/backplane/cef-runtime/<VERSION>`.
pub fn runtime_dir() -> Option<PathBuf> {
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("BACKPLANE_CEF").filter(|p| !p.is_empty()) { let p = PathBuf::from(p); cands.push(if p.is_dir() { p } else { p.parent().map(Path::to_path_buf).unwrap_or(p) }); }
    let cache = crate::plat::cache_base();
    if let Some(c) = cache { cands.push(c.join("backplane").join("cef-runtime").join(VERSION)); }
    if let Ok(me) = std::env::current_exe() { if let Some(d) = me.parent() { cands.push(d.to_path_buf()); if cfg!(target_os = "macos") { cands.push(d.join("../Frameworks")); } } }
    #[cfg(target_os = "macos")]
    return cands.into_iter().find(|d| d.join(mac::FRAMEWORK).join("Resources").join("icudtl.dat").exists() && d.join(mac::FRAMEWORK).join(mac::LIB).exists());
    #[cfg(not(target_os = "macos"))]
    cands.into_iter().find(|d| d.join("icudtl.dat").exists() && d.join("resources.pak").exists() && d.join(LIB).exists())
}
/// The engine's library in a runtime directory.
#[cfg(target_os = "windows")] pub const LIB: &str = "libcef.dll";
#[cfg(not(target_os = "windows"))] pub const LIB: &str = "libcef.so";
/// The engine's profile (cookies, storage): `~/.cache/backplane/cef-gpui-profile` (Windows: `%LOCALAPPDATA%\backplane\cef-gpui-profile`).
fn profile_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("BACKPLANE_CEF_PROFILE").filter(|p| !p.is_empty()) { return PathBuf::from(p) }
    #[cfg(target_os = "windows")] { if let Some(c) = crate::plat::cache_base() { return c.join("backplane").join("cef-gpui-profile") } }
    crate::plat::user_home().unwrap_or_default().join(".cache").join("backplane").join("cef-gpui-profile")
}

/// CEF reads icudtl.dat, the paks and the snapshot from the directory libcef.so was loaded from (RUNPATH `$ORIGIN`: the executable's directory, whatever a link there points at),
/// and a missing file is a failed CHECK inside `cef_initialize` that kills the whole process. So before starting: link what is missing there from the runtime directory, and
/// refuse (the page says why) when it still is not all there.
#[cfg(any(target_os = "linux", test))]
const RUNTIME_FILES: [&str; 6] = ["icudtl.dat", "resources.pak", "chrome_100_percent.pak", "chrome_200_percent.pak", "v8_context_snapshot.bin", "locales"];
#[cfg(any(target_os = "linux", test))]
fn ensure_beside(dir: &Path, exe_dir: &Path) -> Result<(), String> {
    if dir == exe_dir { return Ok(()) }
    for f in RUNTIME_FILES {
        let (want, have) = (exe_dir.join(f), dir.join(f));
        if want.exists() || !have.exists() { continue }
        #[cfg(unix)] { let _ = std::os::unix::fs::symlink(&have, &want); }
    }
    let missing: Vec<&str> = RUNTIME_FILES.iter().copied().filter(|f| !exe_dir.join(f).exists()).collect();
    if missing.is_empty() { Ok(()) } else { Err(format!("CEF's files are missing beside {} ({}) and could not be linked from {}", exe_dir.display(), missing.join(", "), dir.display())) }
}

pub struct Engine { pub cdp_port: u16, pub dir: PathBuf }
static ENGINE: OnceLock<Result<Engine, String>> = OnceLock::new();

fn free_port() -> u16 { std::net::TcpListener::bind("127.0.0.1:0").ok().and_then(|l| l.local_addr().ok()).map(|a| a.port()).unwrap_or(9333) }

impl Engine {
    /// Starts CEF once for the process; Err (said in the page, which then falls back to the hub's frames) when the runtime is missing or CEF refuses.
    pub fn get() -> &'static Result<Engine, String> { ENGINE.get_or_init(Engine::start) }
    fn start() -> Result<Engine, String> {
        if std::env::var_os("BACKPLANE_CEF_OFF").is_some() { return Err("BACKPLANE_CEF_OFF is set".into()) }
        let dir = runtime_dir().ok_or_else(|| "no CEF runtime (set BACKPLANE_CEF to the runtime directory, or put it in ~/.cache/backplane/cef-runtime/".to_string() + VERSION + ")")?;
        #[cfg(target_os = "linux")]
        if let Some(ed) = std::env::current_exe().ok().and_then(|e| e.canonicalize().ok()).and_then(|e| e.parent().map(Path::to_path_buf)) { ensure_beside(&dir, &ed)?; }
        let port = std::env::var("BACKPLANE_CEF_CDP_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or_else(free_port);
        let profile = profile_dir();
        let _ = std::fs::create_dir_all(&profile);
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        // the helper processes: this executable again (Linux, Windows); on macOS the helper app beside it (`mac::helper`), whose program is backplane-cef-helper
        #[cfg(target_os = "macos")]
        let (sub, res, loc) = { let fw = dir.join(mac::FRAMEWORK); if !mac::load(&fw) { return Err(format!("could not load {}", fw.display())) }
            mac::adopt_app_protocol();
            // the helpers find the same framework (set on the main thread before CEF starts any thread of its own)
            unsafe { std::env::set_var("BACKPLANE_CEF_FRAMEWORK", &fw); }
            (mac::helper(&exe).ok_or_else(|| format!("no {}.app beside {} (scripts/cef-mac-helpers.sh)", mac::HELPER, exe.display()))?, fw.join("Resources"), fw.join("Resources")) };
        #[cfg(not(target_os = "macos"))]
        let (sub, res, loc) = (exe.clone(), dir.clone(), dir.join("locales"));
        let settings = Settings {
            no_sandbox: true as _, multi_threaded_message_loop: (!EXTERNAL_PUMP) as _, external_message_pump: EXTERNAL_PUMP as _, windowless_rendering_enabled: true as _,
            browser_subprocess_path: CefString::from(sub.to_string_lossy().as_ref()),
            #[cfg(target_os = "macos")] framework_dir_path: CefString::from(dir.join(mac::FRAMEWORK).to_string_lossy().as_ref()),
            #[cfg(target_os = "macos")] main_bundle_path: CefString::from(mac::main_bundle(&exe).to_string_lossy().as_ref()),
            resources_dir_path: CefString::from(res.to_string_lossy().as_ref()),
            locales_dir_path: CefString::from(loc.to_string_lossy().as_ref()),
            root_cache_path: CefString::from(profile.to_string_lossy().as_ref()), cache_path: CefString::from(profile.to_string_lossy().as_ref()),
            remote_debugging_port: port as _, persist_session_cookies: true as _,
            log_severity: LogSeverity::WARNING, ..Default::default()
        };
        let args = cef::args::Args::new();
        let mut a = app();
        let ok = initialize(Some(args.as_main_args()), Some(&settings), Some(&mut a), std::ptr::null_mut());
        if ok != 1 { return Err(format!("CEF did not start (initialize answered {ok})")) }
        Ok(Engine { cdp_port: port, dir })
    }
    pub fn shutdown() { if ENGINE.get().map_or(false, |e| e.is_ok()) { cef::shutdown(); } }
}

impl Tab {
    /// Creates a browser at `url` with a view of `w`x`h` logical px at `scale`; `wake` gets a message whenever something changed.
    pub fn open(url: &str, w: u32, h: u32, scale: f32, wake: async_channel::Sender<()>) -> Arc<Tab> {
        let tab = Tab::detached(url, w, h, scale, wake);
        let mut client = client_for(&tab);
        let wi = WindowInfo { windowless_rendering_enabled: true as _, runtime_style: RuntimeStyle::ALLOY, ..Default::default() };
        let bs = BrowserSettings { windowless_frame_rate: 30, ..Default::default() };
        let r = browser_host_create_browser(Some(&wi), Some(&mut client), Some(&CefString::from(url)), Some(&bs), None, None);
        if r != 1 { tab.st.lock().unwrap().load_error = "CEF refused to create the browser".into(); }
        tab
    }
    /// A tab whose browser CEF makes itself (a popup): the state and handlers only.
    pub fn detached(url: &str, w: u32, h: u32, scale: f32, wake: async_channel::Sender<()>) -> Arc<Tab> {
        Arc::new(Tab { st: Mutex::new(State { url: url.into(), ..Default::default() }), browser: Mutex::new(None), cursor: Mutex::new(Cur::Default), w: w.max(1).into(), h: h.max(1).into(), scale_milli: ((scale * 1000.0) as u32).into(), wake, hidden: AtomicBool::new(false), id: AtomicI32::new(0), paints: Default::default(), resize_at: Mutex::new(None), resize_done: Mutex::new(-1.0) })
    }
    fn with<R>(&self, f: impl FnOnce(&Browser) -> R) -> Option<R> { self.browser.lock().unwrap().as_ref().map(|b| f(&b.0)) }
    fn host<R>(&self, f: impl FnOnce(&BrowserHost) -> R) -> Option<R> { self.with(|b| b.host().map(|h| f(&h))).flatten() }
    pub fn ready(&self) -> bool { self.browser.lock().unwrap().is_some() }
    pub fn cursor(&self) -> Cur { *self.cursor.lock().unwrap() }
    pub fn take_frame(&self) -> Option<Frame> { let mut st = self.st.lock().unwrap(); if st.fresh { st.fresh = false; st.frame.take() } else { None } }
    pub fn resize(&self, w: u32, h: u32, scale: f32) {
        let (w, h, sm) = (w.max(1), h.max(1), (scale * 1000.0) as u32);
        let changed = self.w.swap(w, Ordering::Relaxed) != w || self.h.swap(h, Ordering::Relaxed) != h || self.scale_milli.swap(sm, Ordering::Relaxed) != sm;
        if changed { *self.resize_at.lock().unwrap() = Some((std::time::Instant::now(), (w as f32 * scale).round() as u32)); }
        self.host(|h| { h.notify_screen_info_changed(); h.was_resized(); });
    }
    pub fn resize_ms(&self) -> f64 { *self.resize_done.lock().unwrap() }
    pub fn size(&self) -> (u32, u32) { (self.w.load(Ordering::Relaxed), self.h.load(Ordering::Relaxed)) }
    pub fn navigate(&self, url: &str) { let u = CefString::from(url); self.with(|b| if let Some(f) = b.main_frame() { f.load_url(Some(&u)); }); }
    pub fn back(&self) { self.with(|b| b.go_back()); }
    pub fn forward(&self) { self.with(|b| b.go_forward()); }
    pub fn reload(&self) { self.with(|b| b.reload()); }
    pub fn stop(&self) { self.with(|b| b.stop_load()); }
    /// F12: Chromium's DevTools for this page in a window of its own (a separate X window, not painted into ours).
    pub fn devtools(&self) {
        self.host(|h| { if h.has_dev_tools() == 1 { h.close_dev_tools(); return }
            let wi = WindowInfo { bounds: Rect { x: 100, y: 100, width: 1100, height: 760 }, window_name: CefString::from("DevTools"), runtime_style: RuntimeStyle::ALLOY, ..Default::default() };
            h.show_dev_tools(Some(&wi), None, Some(&BrowserSettings::default()), None); });
    }
    pub fn close(&self) { self.host(|h| h.close_browser(true as _)); }
    pub fn focus(&self, on: bool) { self.host(|h| h.set_focus(on as _)); }
    pub fn hidden(&self, hide: bool) { if self.hidden.swap(hide, Ordering::Relaxed) != hide { self.host(|h| h.was_hidden(hide as _)); } }
    pub fn mouse_move(&self, x: i32, y: i32, mods: u32, leave: bool) { self.host(|h| h.send_mouse_move_event(Some(&MouseEvent { x, y, modifiers: mods }), leave as _)); }
    /// button 0 left, 1 middle, 2 right
    pub fn mouse_click(&self, x: i32, y: i32, mods: u32, button: u8, up: bool, count: i32) {
        let b = match button { 1 => MouseButtonType::MIDDLE, 2 => MouseButtonType::RIGHT, _ => MouseButtonType::LEFT };
        self.host(|h| h.send_mouse_click_event(Some(&MouseEvent { x, y, modifiers: mods }), b, up as _, count));
    }
    pub fn wheel(&self, x: i32, y: i32, mods: u32, dx: i32, dy: i32) { self.host(|h| h.send_mouse_wheel_event(Some(&MouseEvent { x, y, modifiers: mods }), dx, dy)); }
    pub fn key(&self, kind: u8, mods: u32, vk: i32, native: i32, ch: u16) {
        let ty = match kind { 0 => KeyEventType::RAWKEYDOWN, 1 => KeyEventType::KEYUP, _ => KeyEventType::CHAR };
        // macOS's OSR builds an NSEvent from the native code: it must be the Mac's virtual key code, not the Windows one
        #[cfg(target_os = "macos")] let native = mac::key_code(vk).unwrap_or(native);
        let e = KeyEvent { size: std::mem::size_of::<KeyEvent>(), type_: ty, modifiers: mods, windows_key_code: vk, native_key_code: native, is_system_key: 0, character: ch, unmodified_character: ch, focus_on_editable_field: 0 };
        self.host(|h| h.send_key_event(Some(&e)));
    }
    pub fn ime_commit(&self, text: &str) { let t = CefString::from(text); self.host(|h| h.ime_commit_text(Some(&t), None, 0)); }
    pub fn ime_compose(&self, text: &str) {
        let t = CefString::from(text); let n = text.encode_utf16().count() as u32;
        let ul = CompositionUnderline { size: std::mem::size_of::<CompositionUnderline>(), range: Range { from: 0, to: n }, color: 0xff000000, background_color: 0, thick: 0, style: CompositionUnderlineStyle::SOLID };
        self.host(|h| h.ime_set_composition(Some(&t), Some(&[ul]), None, Some(&Range { from: n, to: n })));
    }
    pub fn ime_cancel(&self) { self.host(|h| h.ime_cancel_composition()); }
    pub fn edit(&self, cmd: &str) { self.with(|b| if let Some(f) = b.focused_frame() { match cmd { "copy" => f.copy(), "cut" => f.cut(), "paste" => f.paste(), "all" => f.select_all(), "undo" => f.undo(), "redo" => f.redo(), _ => {} } }); }
}
impl Drop for Tab { fn drop(&mut self) { let b = self.browser.get_mut().ok().and_then(|b| b.take()); if let Some(b) = b { if let Some(h) = b.0.host() { h.close_browser(true as _); } } } }

/// Windows virtual-key code for a gpui key name (what CEF's DOM key mapping starts from); None for a key with no code here.
pub fn vk(key: &str) -> Option<i32> {
    let one: Vec<char> = key.chars().collect();
    if one.len() == 1 { let c = one[0]; return Some(match c { 'a'..='z' => c.to_ascii_uppercase() as i32, 'A'..='Z' | '0'..='9' => c as i32, ' ' => 32, ';' | ':' => 186, '=' | '+' => 187, ',' | '<' => 188, '-' | '_' => 189, '.' | '>' => 190, '/' | '?' => 191, '`' | '~' => 192, '[' | '{' => 219, '\\' | '|' => 220, ']' | '}' => 221, '\'' | '"' => 222, _ => return None }) }
    Some(match key { "enter" => 13, "backspace" => 8, "tab" => 9, "escape" => 27, "space" => 32, "left" => 37, "up" => 38, "right" => 39, "down" => 40, "delete" => 46, "home" => 36, "end" => 35, "pageup" => 33, "pagedown" => 34, "insert" => 45,
        k if k.starts_with('f') && k[1..].parse::<i32>().map_or(false, |n| (1..=12).contains(&n)) => 111 + k[1..].parse::<i32>().unwrap(), _ => return None })
}
/// EVENTFLAG_COMMAND_DOWN: gpui's platform key (Command on macOS).
pub const COMMAND: u32 = 1 << 7;
/// CEF event flags from gpui modifiers (shift, control, alt) and mouse buttons held.
pub fn flags(shift: bool, ctrl: bool, alt: bool, left: bool, middle: bool, right: bool) -> u32 {
    (shift as u32) << 1 | (ctrl as u32) << 2 | (alt as u32) << 3 | (left as u32) << 4 | (middle as u32) << 5 | (right as u32) << 6
}

#[cfg(target_os = "macos")]
#[path = "cef_mac.rs"]
pub mod mac;

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn downloads_never_overwrite_and_names_are_plain() {
        let d = std::env::temp_dir().join(format!("bp-dl-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap();
        assert_eq!(unique_path(&d, "a.txt"), d.join("a.txt")); std::fs::write(d.join("a.txt"), b"x").unwrap();
        assert_eq!(unique_path(&d, "a.txt"), d.join("a (2).txt")); std::fs::write(d.join("a (2).txt"), b"x").unwrap(); assert_eq!(unique_path(&d, "a.txt"), d.join("a (3).txt"));
        assert_eq!(safe_name("../x/y.txt"), "_x_y.txt"); assert_eq!(safe_name(""), "download"); let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn runtime_files_are_linked_beside_the_executable_or_refused() {
        let b = std::env::temp_dir().join(format!("bp-cefrt-{}", std::process::id())); let (rt, ex) = (b.join("rt"), b.join("exe")); std::fs::create_dir_all(rt.join("locales")).unwrap(); std::fs::create_dir_all(&ex).unwrap();
        assert!(ensure_beside(&rt, &ex).is_err()); // nothing anywhere: refused, not a crash
        for f in RUNTIME_FILES { if f != "locales" { std::fs::write(rt.join(f), b"x").unwrap(); } }
        assert!(ensure_beside(&rt, &ex).is_ok()); for f in RUNTIME_FILES { assert!(ex.join(f).exists(), "{f}"); }
        assert!(ensure_beside(&rt, &rt).is_ok()); let _ = std::fs::remove_dir_all(&b);
    }
    #[test] fn key_codes() { assert_eq!(vk("a"), Some(65)); assert_eq!(vk("7"), Some(55)); assert_eq!(vk("enter"), Some(13)); assert_eq!(vk("f5"), Some(116)); assert_eq!(vk("é"), None); }
    #[test] fn event_flags() { assert_eq!(flags(true, false, false, true, false, false), 2 | 16); assert_eq!(flags(false, true, true, false, false, true), 4 | 8 | 64); }
}

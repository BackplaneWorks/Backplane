//! The little that differs between Windows, Linux and macOS.
use std::{path::Path, process::{Child, Command, ExitStatus}};

#[cfg(unix)] mod os {
    use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
    static STOP: AtomicBool = AtomicBool::new(false);
    extern "C" fn on_stop(_: libc::c_int) { STOP.store(true, SeqCst); }
    pub fn watch_stop() { unsafe { for s in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] { libc::signal(s, on_stop as extern "C" fn(libc::c_int) as libc::sighandler_t); } libc::signal(libc::SIGPIPE, libc::SIG_IGN); } }
    pub fn stop_asked() -> bool { STOP.load(SeqCst) }
    pub fn signal(pid: u32, sig: i32) { unsafe { libc::kill(pid as i32, sig); } }
    pub fn term(pid: u32) { signal(pid, libc::SIGTERM) }
    /// SIGABRT: a hung hub leaves a core if the machine keeps them, then dies
    pub fn abort(pid: u32) { signal(pid, libc::SIGABRT) }
    pub fn alive(pid: u32) -> bool { pid != 0 && (unsafe { libc::kill(pid as i32, 0) } == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)) }
    /// (free to this user, total) of the filesystem holding `p`
    pub fn space(p: &std::path::Path) -> Option<(u64, u64)> {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
        let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
        (unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0).then(|| (s.f_bavail as u64 * s.f_frsize as u64, s.f_blocks as u64 * s.f_frsize as u64))
    }
    pub fn detach(c: &mut std::process::Command) { use std::os::unix::process::CommandExt; unsafe { c.pre_exec(|| { libc::setsid(); Ok(()) }); } }
    pub fn signal_of(s: &std::process::ExitStatus) -> Option<i32> { use std::os::unix::process::ExitStatusExt; s.signal() }
}
#[cfg(windows)] mod os {
    pub fn watch_stop() {}
    pub fn stop_asked() -> bool { false }
    pub fn term(pid: u32) { let _ = quiet("taskkill").args(["/PID", &pid.to_string()]).output(); }
    pub fn abort(pid: u32) { let _ = quiet("taskkill").args(["/F", "/PID", &pid.to_string()]).output(); }
    pub fn alive(pid: u32) -> bool { quiet("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().map_or(false, |o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string())) }
    #[link(name = "kernel32")] extern "system" { fn GetDiskFreeSpaceExW(dir: *const u16, free: *mut u64, total: *mut u64, all: *mut u64) -> i32; }
    pub fn space(p: &std::path::Path) -> Option<(u64, u64)> {
        use std::os::windows::ffi::OsStrExt;
        let w: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect(); let (mut a, mut b, mut c) = (0u64, 0u64, 0u64);
        (unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut a, &mut b, &mut c) } != 0).then_some((a, b))
    }
    /// CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP: the hub gets a console that is never shown, which what it starts (git, sh) shares.
    /// DETACHED_PROCESS left it none, so every console program the hub ran opened a console window of its own.
    pub fn detach(c: &mut std::process::Command) { use std::os::windows::process::CommandExt; c.creation_flags(NO_WINDOW | 0x0000_0200); }
    const NO_WINDOW: u32 = 0x0800_0000;
    fn quiet(name: &str) -> std::process::Command { use std::os::windows::process::CommandExt; let mut c = std::process::Command::new(name); c.creation_flags(NO_WINDOW); c }
    pub fn signal_of(_: &std::process::ExitStatus) -> Option<i32> { None }
}
pub use os::{abort, alive, detach, signal_of, space, stop_asked, term, watch_stop};
pub fn free_bytes(p: &Path) -> Option<u64> { space(p).map(|s| s.0) }

/// The hub program in a build folder.
pub fn hub_exe(dir: &Path) -> std::path::PathBuf { dir.join(if cfg!(windows) { "backplane-hub.exe" } else { "backplane-hub" }) }

/// Ask politely, then insist. The child is reaped either way.
pub fn stop_child(c: &mut Child, wait_ms: u64) -> Option<ExitStatus> {
    term(c.id());
    let mut waited = 0;
    while waited < wait_ms { if let Ok(Some(s)) = c.try_wait() { return Some(s) } std::thread::sleep(std::time::Duration::from_millis(50)); waited += 50; }
    let _ = c.kill(); c.wait().ok()
}

/// The person's desktop notification, where the machine has one. Never waited for: a stuck notifier must not stop the guard.
pub fn notify(title: &str, body: &str) {
    let mut c = if cfg!(target_os = "macos") { let mut c = Command::new("osascript"); c.args(["-e", &format!("display notification {:?} with title {:?}", body, title)]); c }
        else if cfg!(windows) { let mut c = Command::new("msg"); c.args(["*", &format!("{title}: {body}")]); c }
        else { let mut c = Command::new("notify-send"); c.args(["-a", "Backplane", title, body]); c };
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    if let Ok(mut ch) = c.spawn() { std::thread::spawn(move || { let _ = ch.wait(); }); }
}

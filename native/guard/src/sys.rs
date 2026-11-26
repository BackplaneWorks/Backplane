//! The little that differs between Windows, Linux and macOS.
use std::{path::Path, process::{Child, Command, ExitStatus}};

#[cfg(unix)] mod os {
    use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
    static STOP: AtomicBool = AtomicBool::new(false);
    extern "C" fn on_stop(_: libc::c_int) { STOP.store(true, SeqCst); }
    pub fn watch_stop() { unsafe { for s in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] { libc::signal(s, on_stop as extern "C" fn(libc::c_int) as libc::sighandler_t); } libc::signal(libc::SIGPIPE, libc::SIG_IGN); } }
    pub fn stop_asked() -> bool { STOP.load(SeqCst) }
    /// First thing, before any thread: every signal unblocked and an ignored SIGCHLD at its default. The desktop app starts the guard from a GCD
    /// worker thread on macOS, whose mask blocks SIGTERM, SIGHUP, SIGCHLD...: the guard's own stop handler never ran, and std's posix_spawn passed
    /// the mask on to the hub, which then never saw a child end (#1456). The hub puts it right itself too; this keeps the guard stoppable.
    pub fn clean_signals() {
        unsafe {
            let mut none: libc::sigset_t = std::mem::zeroed(); libc::sigemptyset(&mut none); libc::pthread_sigmask(libc::SIG_SETMASK, &none, std::ptr::null_mut());
            let mut old: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut old) == 0 && old.sa_sigaction == libc::SIG_IGN {
                let mut dfl: libc::sigaction = std::mem::zeroed(); dfl.sa_sigaction = libc::SIG_DFL; libc::sigaction(libc::SIGCHLD, &dfl, std::ptr::null_mut());
            }
        }
    }
    pub fn signal(pid: u32, sig: i32) {
        if pid > 1 && pid <= i32::MAX as u32 {
            unsafe { libc::kill(pid as i32, sig); }
        }
    }
    pub fn term(pid: u32) { signal(pid, libc::SIGTERM) }
    /// SIGABRT: a hung hub leaves a core if the machine keeps them, then dies
    pub fn abort(pid: u32) { signal(pid, libc::SIGABRT) }
    pub fn kill(pid: u32) { signal(pid, libc::SIGKILL) }
    /// Whether `path` still names the same file inode as the open handle `held` (false if unlinked or replaced, e.g. after a reinstall).
    pub fn same_file(held: &std::fs::File, path: &std::path::Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        let Ok(a) = held.metadata() else { return true };
        match std::fs::metadata(path) {
            Ok(b) => a.dev() == b.dev() && a.ino() == b.ino(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => true,
        }
    }
    pub fn alive(pid: u32) -> bool {
        pid > 0 && pid <= i32::MAX as u32
            && (unsafe { libc::kill(pid as i32, 0) } == 0
                || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
    }
    /// (free to this user, total) of the filesystem holding `p`
    pub fn space(p: &std::path::Path) -> Option<(u64, u64)> {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
        let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
        (unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0).then(|| (s.f_bavail as u64 * s.f_frsize as u64, s.f_blocks as u64 * s.f_frsize as u64))
    }
    pub fn detach(c: &mut std::process::Command) { use std::os::unix::process::CommandExt; unsafe { c.pre_exec(|| { libc::setsid(); Ok(()) }); } }
    pub fn signal_of(s: &std::process::ExitStatus) -> Option<i32> { use std::os::unix::process::ExitStatusExt; s.signal() }
    /// Monotonic awake seconds since first call (pauses while macOS/Linux sleeps).
    pub fn awake_s() -> u64 {
        static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        T0.get_or_init(std::time::Instant::now).elapsed().as_secs()
    }
}
#[cfg(windows)] mod os {
    pub fn watch_stop() {}
    pub fn clean_signals() {}
    pub fn stop_asked() -> bool { false }
    pub fn term(pid: u32) { let _ = quiet("taskkill").args(["/PID", &pid.to_string()]).output(); }
    pub fn abort(pid: u32) { let _ = quiet("taskkill").args(["/F", "/PID", &pid.to_string()]).output(); }
    pub fn kill(pid: u32) { abort(pid) }
    #[repr(C)]
    struct FileTime { low: u32, high: u32 }
    #[repr(C)]
    struct ByHandleFileInformation {
        attrs: u32,
        creation: FileTime,
        last_access: FileTime,
        last_write: FileTime,
        volume_serial: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    fn file_id(f: &std::fs::File) -> Option<(u32, u32, u32)> {
        use std::os::windows::io::AsRawHandle;
        let mut info: ByHandleFileInformation = unsafe { std::mem::zeroed() };
        (unsafe { GetFileInformationByHandle(f.as_raw_handle() as *mut core::ffi::c_void, &mut info) } != 0)
            .then_some((info.volume_serial, info.index_high, info.index_low))
    }
    pub fn same_file(held: &std::fs::File, path: &std::path::Path) -> bool {
        let Some(a) = file_id(held) else { return true };
        match std::fs::File::open(path) {
            Ok(other) => file_id(&other).map_or(true, |b| a == b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => true,
        }
    }
    pub fn alive(pid: u32) -> bool { quiet("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().map_or(false, |o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string())) }
    #[link(name = "kernel32")] extern "system" {
        fn GetDiskFreeSpaceExW(dir: *const u16, free: *mut u64, total: *mut u64, all: *mut u64) -> i32;
        fn GetFileInformationByHandle(handle: *mut core::ffi::c_void, info: *mut ByHandleFileInformation) -> i32;
        fn QueryUnbiasedInterruptTime(unbiased_time: *mut u64) -> i32;
    }
    /// Monotonic awake seconds since first call (`QueryUnbiasedInterruptTime` excludes standby/hibernation).
    pub fn awake_s() -> u64 {
        static T0: std::sync::OnceLock<(Option<u64>, std::time::Instant)> = std::sync::OnceLock::new();
        let read = || { let mut t = 0u64; (unsafe { QueryUnbiasedInterruptTime(&mut t) } != 0).then_some(t / 10_000_000) };
        let (base, fallback) = *T0.get_or_init(|| (read(), std::time::Instant::now()));
        match (base, read()) { (Some(b), Some(now)) => now.saturating_sub(b), _ => fallback.elapsed().as_secs() }
    }
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
pub use os::{abort, alive, awake_s, clean_signals, detach, kill, same_file, signal_of, space, stop_asked, term, watch_stop};
pub fn free_bytes(p: &Path) -> Option<u64> { space(p).map(|s| s.0) }

fn trim_end_seps(p: &Path) -> String {
    let s = p.to_string_lossy();
    let t = s.trim_end_matches(['/', '\\']);
    if t.is_empty() && !s.is_empty() { s.chars().take(1).collect() } else { t.to_string() }
}

/// Parse a Darwin `KERN_PROCARGS2` buffer (`i32 argc`, NUL-terminated `exec_path`, NUL padding, then `argc` NUL-terminated `argv` strings).
#[allow(dead_code)]
pub(crate) fn parse_kern_procargs2(buf: &[u8]) -> Option<Vec<String>> {
    if buf.len() < 4 { return None }
    let argc = i32::from_ne_bytes(buf[..4].try_into().ok()?);
    if argc <= 0 || argc > 4096 { return None }
    let mut pos = 4usize;
    let nul = buf.get(pos..)?.iter().position(|&b| b == 0)?;
    pos += nul + 1;
    while pos < buf.len() && buf[pos] == 0 { pos += 1; }
    let mut out = Vec::with_capacity(argc as usize);
    for _ in 0..argc {
        let end = buf.get(pos..)?.iter().position(|&b| b == 0)?;
        out.push(String::from_utf8_lossy(&buf[pos..pos + end]).into_owned());
        pos += end + 1;
    }
    Some(out)
}

/// Whether `args` invokes a Backplane guard (`backplane hub run ... --home <home>`).
pub(crate) fn cmdline_is_guard_for_home(args: &[String], home: &Path) -> bool {
    let Some(i) = (1..args.len()).find(|&i| args[i] == "hub" && args.get(i + 1).map(String::as_str) == Some("run")) else {
        return false;
    };
    let bin = args[i - 1].rsplit(['/', '\\']).next().unwrap_or("").to_ascii_lowercase();
    if !bin.starts_with("backplane") { return false }
    let mut j = i + 2;
    let mut found_home: Option<&str> = None;
    while j < args.len() {
        let a = args[j].as_str();
        if a == "--" { break }
        if a == "--home" {
            let Some(v) = args.get(j + 1) else { break };
            found_home = Some(v.as_str());
            j += 2;
            continue;
        }
        if let Some(v) = a.strip_prefix("--home=") {
            found_home = Some(v);
        }
        j += 1;
    }
    let Some(val) = found_home else { return false };
    if trim_end_seps(Path::new(val)) == trim_end_seps(home) { return true }
    matches!((std::fs::canonicalize(val), std::fs::canonicalize(home)), (Ok(a), Ok(b)) if a == b)
}

#[cfg(target_os = "linux")]
fn read_cmdline(pid: u32) -> Option<Vec<String>> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    if raw.is_empty() { return None }
    let mut parts: Vec<String> = raw.split(|&b| b == 0).map(|s| String::from_utf8_lossy(s).into_owned()).collect();
    if raw.last() == Some(&0) { parts.pop(); }
    Some(parts)
}

#[cfg(target_os = "macos")]
fn read_cmdline(pid: u32) -> Option<Vec<String>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
    let mut len: libc::size_t = 0;
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) } != 0 || len == 0 {
        return None;
    }
    let cap = len.min(256 * 1024);
    let mut buf = vec![0u8; cap];
    let mut got = cap;
    if unsafe { libc::sysctl(mib.as_mut_ptr(), 3, buf.as_mut_ptr() as *mut libc::c_void, &mut got, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    buf.truncate(got);
    parse_kern_procargs2(&buf)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn read_cmdline(_pid: u32) -> Option<Vec<String>> { None }

/// Whether `pid` is another live `backplane hub run` guard process supervising `home`.
pub fn is_guard_for_home(pid: u32, home: &Path) -> bool {
    if pid <= 1 || pid == std::process::id() || pid > i32::MAX as u32 { return false }
    read_cmdline(pid).is_some_and(|args| cmdline_is_guard_for_home(&args, home))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn procargs2(exec_path: &str, argv: &[&str], envp: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(argv.len() as i32).to_ne_bytes());
        out.extend_from_slice(exec_path.as_bytes());
        out.extend_from_slice(&[0, 0, 0]);
        for a in argv {
            out.extend_from_slice(a.as_bytes());
            out.push(0);
        }
        for e in envp {
            out.extend_from_slice(e.as_bytes());
            out.push(0);
        }
        out
    }

    #[test]
    fn parse_kern_procargs2_extracts_exact_argv_and_rejects_malformed() {
        let buf = procargs2(
            "/Users/user/.local/share/backplane/backplane",
            &["backplane", "hub", "run", "--home", "/Users/user/Library/Application Support/Backplane/hub", "--port", "3802"],
            &["PATH=/usr/bin", "HOME=/Users/user"],
        );
        assert_eq!(
            parse_kern_procargs2(&buf).unwrap(),
            vec![
                "backplane",
                "hub",
                "run",
                "--home",
                "/Users/user/Library/Application Support/Backplane/hub",
                "--port",
                "3802",
            ]
        );
        assert_eq!(parse_kern_procargs2(&[]), None);
        assert_eq!(parse_kern_procargs2(&[0, 0, 0, 0]), None, "zero argc");
        assert_eq!(parse_kern_procargs2(&(-1i32).to_ne_bytes()), None, "negative argc");
        assert_eq!(parse_kern_procargs2(&(5000i32).to_ne_bytes()), None, "huge argc");
        let mut no_nul = (1i32).to_ne_bytes().to_vec();
        no_nul.extend_from_slice(b"/bin/backplane");
        assert_eq!(parse_kern_procargs2(&no_nul), None, "missing NUL after exec_path");
        let short = procargs2("/bin/backplane", &["backplane", "hub", "run"], &[]);
        let trunc = &short[..short.len() - 2];
        assert_eq!(parse_kern_procargs2(trunc), None, "truncated argv");
    }

    #[test]
    fn cmdline_is_guard_for_home_matches_exact_guard_invocations() {
        let s = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let mac_home = Path::new("/Users/user/Library/Application Support/Backplane/hub");
        assert!(cmdline_is_guard_for_home(
            &s(&["/Users/user/.local/share/backplane/backplane", "hub", "run", "--home", "/Users/user/Library/Application Support/Backplane/hub", "--port", "3802"]),
            mac_home,
        ));
        assert!(cmdline_is_guard_for_home(
            &s(&["systemd-run", "--user", "--scope", "-q", "--", "/opt/backplane/backplane", "hub", "run", "--home=/Users/user/Library/Application Support/Backplane/hub/", "--port", "3802"]),
            mac_home,
        ));
        assert!(!cmdline_is_guard_for_home(
            &s(&["/usr/bin/backplane", "hub", "status", "--home", "/Users/user/Library/Application Support/Backplane/hub"]),
            mac_home,
        ));
        assert!(!cmdline_is_guard_for_home(
            &s(&["/usr/bin/python3", "hub", "run", "--home", "/Users/user/Library/Application Support/Backplane/hub"]),
            mac_home,
        ));
        assert!(!cmdline_is_guard_for_home(
            &s(&["backplane", "hub", "run", "--", "--home", "/Users/user/Library/Application Support/Backplane/hub"]),
            mac_home,
        ));
        assert!(!cmdline_is_guard_for_home(
            &s(&["backplane", "hub", "run", "--home", "/tmp/other"]),
            mac_home,
        ));

        #[cfg(unix)]
        {
            let d = crate::tmp::dir("sys-canon");
            let real_home = d.join("real-home");
            std::fs::create_dir_all(&real_home).unwrap();
            let link_home = d.join("link-home");
            std::os::unix::fs::symlink(&real_home, &link_home).unwrap();
            assert!(cmdline_is_guard_for_home(
                &s(&["backplane", "hub", "run", "--home", link_home.to_str().unwrap()]),
                &real_home,
            ));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn is_guard_for_home_rejects_self_special_pids_and_non_guard_processes() {
        let d = crate::tmp::dir("sys-live");
        assert!(!is_guard_for_home(0, &d));
        assert!(!is_guard_for_home(1, &d));
        assert!(!is_guard_for_home(std::process::id(), &d));
        let mut child = Command::new("sh").args(["-c", "sleep 10"]).spawn().unwrap();
        assert!(!is_guard_for_home(child.id(), &d));
        let _ = child.kill();
        let _ = child.wait();
    }
}

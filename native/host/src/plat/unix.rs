//! Unix: exactly what the hub always called.

pub mod fs {
    pub use std::os::unix::fs::{symlink, DirBuilderExt, FileExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
    pub const O_NOFOLLOW: i32 = libc::O_NOFOLLOW;
    pub const O_NONBLOCK: i32 = libc::O_NONBLOCK;
    /// This process's user (an owner check on a private folder).
    pub fn uid() -> u32 { unsafe { libc::getuid() } }
    /// Whether a file is a program this user may run.
    pub fn runnable(m: &std::fs::Metadata) -> bool { m.is_file() && m.permissions().mode() & 0o111 != 0 }
    /// Raw bytes of a file name, and back.
    pub fn name_bytes(s: &std::ffi::OsStr) -> Vec<u8> { use std::os::unix::ffi::OsStrExt; s.as_bytes().to_vec() }
    pub fn name_from(b: &[u8]) -> std::ffi::OsString { use std::os::unix::ffi::OsStrExt; std::ffi::OsStr::from_bytes(b).to_os_string() }
    /// A file's mode bits as a test compares them: the real ones.
    pub fn mode_bits(actual: u32, _want: u32) -> u32 { actual }
    /// Flush a folder (a rename in it is on disk).
    pub fn sync_dir(d: &std::path::Path) -> std::io::Result<()> { std::fs::File::open(d)?.sync_all() }
    /// Random bytes from the OS (no fallback).
    pub fn random(b: &mut [u8]) -> std::io::Result<()> { use std::io::Read; std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(b)) }
}

/// The OS error numbers the hub names.
pub mod errno {
    pub use libc::{EACCES, EADDRINUSE, EADDRNOTAVAIL, EDQUOT, EINVAL, EIO, EISDIR, ELOOP, EMFILE, ENFILE, ENOSPC, ENOTDIR, ENOTSUP, EPERM, EROFS, ETXTBSY};
}

pub mod proc {
    pub use libc::{SIGHUP as HUP, SIGKILL as KILL, SIGTERM as TERM};
    /// A child in its own session and process group (setsid), so the group can be signalled as one and a signal to ours does not reach it.
    pub fn own_group(c: &mut std::process::Command) { use std::os::unix::process::CommandExt; unsafe { c.pre_exec(|| { libc::setsid(); Ok(()) }); } }
    pub fn own_group_tokio(c: &mut tokio::process::Command) { unsafe { c.pre_exec(|| { libc::setsid(); Ok(()) }); } }
    /// An agent: its own group, and on Linux the kernel kills it when the hub dies (PDEATHSIG).
    pub fn agent_group(c: &mut tokio::process::Command) { unsafe { c.pre_exec(|| { libc::setsid(); #[cfg(target_os = "linux")] libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL); Ok(()) }); } }
    /// The child keeps `f` open (its descriptor survives exec), so a lock on it lives as long as the child.
    pub fn inherit(c: &mut tokio::process::Command, f: &std::fs::File) { use std::os::fd::AsRawFd; let fd = f.as_raw_fd(); unsafe { c.pre_exec(move || { libc::fcntl(fd, libc::F_SETFD, 0); Ok(()) }); } }
    /// After the spawn of a child given `own_group` / `agent_group`: nothing to do on Unix (the group is the leader's pid).
    pub fn adopt(_pid: u32, _die_with_hub: bool) {}
    /// Signal the process group a leader made (`kill(-pid, sig)`). True if it was sent.
    pub fn kill_group(pid: u32, sig: i32) -> bool { unsafe { libc::kill(-(pid as i32), sig) == 0 } }
    /// Signal one process. True if it was sent.
    pub fn kill(pid: u32, sig: i32) -> bool { unsafe { libc::kill(pid as i32, sig) == 0 } }
    /// Whether a process is there (kill 0).
    pub fn alive(pid: u32) -> bool { pid > 0 && unsafe { libc::kill(pid as i32, 0) } == 0 }
    /// Whether a process is there, including another user's (kill 0 answering EPERM).
    pub fn exists(pid: u32) -> bool { pid > 0 && (unsafe { libc::kill(pid as i32, 0) } == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)) }
    /// The signal that ended a child, if one did.
    pub fn signal(s: &std::process::ExitStatus) -> Option<i32> { use std::os::unix::process::ExitStatusExt; s.signal() }
}

/// The hub process itself.
pub mod hub {
    /// One hub per home: an exclusive flock, not waited for. False if another process holds it.
    pub fn lock(f: &std::fs::File) -> bool { use std::os::fd::AsRawFd; unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 } }
    /// True when the signal is currently ignored (inherited SIG_IGN from nohup or a background job).
    pub fn signal_ignored(sig: i32) -> bool {
        unsafe { let mut old: libc::sigaction = std::mem::zeroed(); libc::sigaction(sig, std::ptr::null(), &mut old) == 0 && old.sa_sigaction == libc::SIG_IGN }
    }
    /// Every agent, terminal and socket is a few descriptors; macOS starts a process with a soft limit of 256. Raise it to the hard limit (macOS refuses more than 10240).
    pub fn raise_nofile() {
        unsafe {
            let mut l = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
            if libc::getrlimit(libc::RLIMIT_NOFILE, &mut l) != 0 { return }
            let want = if cfg!(target_os = "macos") { l.rlim_max.min(10240) } else { l.rlim_max };
            if l.rlim_cur < want { l.rlim_cur = want; libc::setrlimit(libc::RLIMIT_NOFILE, &l); }
        }
    }
    /// A signal that ends the hub says so in hub.log and exits nonzero (128 + the signal); SIGKILL cannot be caught.
    /// SIGHUP and SIGINT stay as the parent left them when it ignored them (nohup, a background `&`): registering would override that.
    pub fn exit_on_signals() {
        for (kind, name, n) in [(tokio::signal::unix::SignalKind::terminate(), "SIGTERM", 15), (tokio::signal::unix::SignalKind::interrupt(), "SIGINT", 2), (tokio::signal::unix::SignalKind::hangup(), "SIGHUP", 1)] {
            if n != 15 && signal_ignored(n) { continue }
            if let Ok(mut sig) = tokio::signal::unix::signal(kind) { tokio::spawn(async move { sig.recv().await; eprintln!("backplane-hub: {name} received, exiting"); std::process::exit(128 + n) }); }
        }
    }
    /// A shell or other program by its Unix name: itself.
    pub fn program(p: &std::ffi::OsStr) -> std::ffi::OsString { p.to_os_string() }
    /// What to start for `p` and the arguments before the caller's: the program itself (the kernel reads `#!`).
    pub fn launch(p: &std::ffi::OsStr) -> (std::ffi::OsString, Vec<std::ffi::OsString>) { (p.to_os_string(), vec![]) }
}

pub mod time {
    extern "C" { fn tzset(); }
    /// `at` in the person's zone, broken down (UTC when no zone is known).
    pub fn local_tm(zone: &str, at: u64) -> libc::tm {
        let t = at as libc::time_t; let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        static TZ: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = TZ.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            if zone.is_empty() { libc::gmtime_r(&t, &mut tm); }
            else { let old = std::env::var_os("TZ"); std::env::set_var("TZ", zone); tzset(); libc::localtime_r(&t, &mut tm); match old { Some(o) => std::env::set_var("TZ", o), None => std::env::remove_var("TZ") } tzset(); }
        }
        tm
    }
    /// The machine zone's offset from UTC at `now`, in seconds east.
    pub fn local_offset(now: u64) -> i64 {
        let t = now as libc::time_t; let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        unsafe { libc::localtime_r(&t, &mut tm); tm.tm_gmtoff as i64 }
    }
}

pub mod net {
    /// The machine's IPv4 addresses that are not loopback.
    pub fn non_loopback_v4() -> Vec<String> {
        let mut out = vec![];
        unsafe {
            let mut ifa: *mut libc::ifaddrs = std::ptr::null_mut();
            if libc::getifaddrs(&mut ifa) != 0 { return out }
            let mut p = ifa;
            while !p.is_null() {
                let a = &*p;
                if !a.ifa_addr.is_null() && (*a.ifa_addr).sa_family as i32 == libc::AF_INET && a.ifa_flags & (libc::IFF_LOOPBACK as u32) == 0 {
                    let sin = &*(a.ifa_addr as *const libc::sockaddr_in);
                    out.push(std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)).to_string());
                }
                p = a.ifa_next;
            }
            libc::freeifaddrs(ifa);
        }
        out
    }
}

/// The local socket KiCad's API listens on (nng `ipc://<path>`): a Unix socket at the path.
pub mod ipc {
    pub type Stream = tokio::net::UnixStream;
    pub async fn connect(path: &str) -> std::io::Result<Stream> { Stream::connect(path).await }
    /// A test's stand-in server.
    pub type Listener = tokio::net::UnixListener;
    pub type Served = tokio::net::UnixStream;
    pub fn bind(path: &str) -> std::io::Result<Listener> { Listener::bind(path) }
    pub async fn accept(l: &Listener) -> std::io::Result<Served> { l.accept().await.map(|(s, _)| s) }
}

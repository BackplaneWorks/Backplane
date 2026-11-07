//! Windows: the nearest meaning of each Unix call. No modes (a file is read-only or not), no inodes, no signals: a process group is a
//! named Job Object per leader (`Local\backplane-group-<pid>`), and every signal ends the group (TerminateJobObject).
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};

fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }

pub mod fs {
    use std::{fs, io, path::Path, time::UNIX_EPOCH};
    pub const O_NOFOLLOW: i32 = 0;
    pub const O_NONBLOCK: i32 = 0;
    /// No user ids: an owner check always passes.
    pub fn uid() -> u32 { 0 }
    /// A program is a file (its extension decides, not a bit).
    pub fn runnable(m: &fs::Metadata) -> bool { m.is_file() }
    pub fn name_bytes(s: &std::ffi::OsStr) -> Vec<u8> { s.to_string_lossy().as_bytes().to_vec() }
    pub fn name_from(b: &[u8]) -> std::ffi::OsString { String::from_utf8_lossy(b).into_owned().into() }
    pub fn random(b: &mut [u8]) -> io::Result<()> {
        use windows_sys::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
        for c in b.chunks_mut(1 << 20) { if unsafe { BCryptGenRandom(std::ptr::null_mut(), c.as_mut_ptr(), c.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) } != 0 { return Err(io::Error::other("BCryptGenRandom failed")) } }
        Ok(())
    }
    /// A link to a folder or to a file (Windows makes them differently).
    pub fn symlink<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> io::Result<()> {
        let dir = link.as_ref().parent().map_or_else(|| original.as_ref().to_path_buf(), |p| p.join(original.as_ref())).is_dir();
        if dir { std::os::windows::fs::symlink_dir(original, link) } else { std::os::windows::fs::symlink_file(original, link) }
    }
    fn secs(t: io::Result<std::time::SystemTime>) -> (i64, i64) { t.ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or((0, 0), |d| (d.as_secs() as i64, d.subsec_nanos() as i64)) }

    /// Modes: only the write bits mean anything (read-only or not).
    pub trait PermissionsExt { fn mode(&self) -> u32; fn set_mode(&mut self, mode: u32); fn from_mode(mode: u32) -> Self; }
    impl PermissionsExt for fs::Permissions {
        fn mode(&self) -> u32 { if self.readonly() { 0o555 } else { 0o755 } }
        fn set_mode(&mut self, mode: u32) { self.set_readonly(mode & 0o222 == 0) }
        fn from_mode(mode: u32) -> Self {
            // Permissions has no constructor on Windows: take a plain file's attributes (this program's) and set read-only from the mode
            let mut p = std::env::current_exe().and_then(fs::metadata).or_else(|_| fs::metadata(std::env::temp_dir())).map(|m| m.permissions()).expect("a file to read attributes from");
            p.set_readonly(mode & 0o222 == 0); p
        }
    }
    pub trait OpenOptionsExt { fn mode(&mut self, mode: u32) -> &mut Self; fn custom_flags(&mut self, flags: i32) -> &mut Self; }
    impl OpenOptionsExt for fs::OpenOptions { fn mode(&mut self, _: u32) -> &mut Self { self } fn custom_flags(&mut self, _: i32) -> &mut Self { self } }
    pub trait DirBuilderExt { fn mode(&mut self, mode: u32) -> &mut Self; }
    impl DirBuilderExt for fs::DirBuilder { fn mode(&mut self, _: u32) -> &mut Self { self } }
    pub trait FileTypeExt { fn is_socket(&self) -> bool; }
    impl FileTypeExt for fs::FileType { fn is_socket(&self) -> bool { false } }
    /// Change stamps from the times Windows keeps: ctime is the last write (Windows has no change time), dev and ino are 0.
    pub trait MetadataExt {
        fn mode(&self) -> u32; fn mtime(&self) -> i64; fn mtime_nsec(&self) -> i64; fn ctime(&self) -> i64; fn ctime_nsec(&self) -> i64;
        fn ino(&self) -> u64; fn dev(&self) -> u64; fn uid(&self) -> u32; fn nlink(&self) -> u64; fn size(&self) -> u64; fn blocks(&self) -> u64;
    }
    impl MetadataExt for fs::Metadata {
        fn mode(&self) -> u32 { (if self.is_dir() { 0o040000 } else { 0o100000 }) | self.permissions().mode() }
        fn mtime(&self) -> i64 { secs(self.modified()).0 }
        fn mtime_nsec(&self) -> i64 { secs(self.modified()).1 }
        fn ctime(&self) -> i64 { secs(self.modified()).0 }
        fn ctime_nsec(&self) -> i64 { secs(self.modified()).1 }
        fn ino(&self) -> u64 { 0 }
        fn dev(&self) -> u64 { 0 }
        fn uid(&self) -> u32 { 0 }
        fn nlink(&self) -> u64 { 1 }
        fn size(&self) -> u64 { self.len() }
        fn blocks(&self) -> u64 { self.len().div_ceil(512) }
    }
    /// Positioned reads and writes (they move the file's cursor on Windows; the hub keeps such a handle for these alone).
    pub trait FileExt { fn read_exact_at(&self, buf: &mut [u8], off: u64) -> io::Result<()>; fn write_all_at(&self, buf: &[u8], off: u64) -> io::Result<()>; }
    impl FileExt for fs::File {
        fn read_exact_at(&self, mut buf: &mut [u8], mut off: u64) -> io::Result<()> {
            use std::os::windows::fs::FileExt as W;
            while !buf.is_empty() { match self.seek_read(buf, off) { Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()), Ok(n) => { buf = &mut buf[n..]; off += n as u64 } Err(e) if e.kind() == io::ErrorKind::Interrupted => {} Err(e) => return Err(e) } }
            Ok(())
        }
        fn write_all_at(&self, mut buf: &[u8], mut off: u64) -> io::Result<()> {
            use std::os::windows::fs::FileExt as W;
            while !buf.is_empty() { match self.seek_write(buf, off) { Ok(0) => return Err(io::ErrorKind::WriteZero.into()), Ok(n) => { buf = &buf[n..]; off += n as u64 } Err(e) if e.kind() == io::ErrorKind::Interrupted => {} Err(e) => return Err(e) } }
            Ok(())
        }
    }
}

/// The Win32 error codes nearest each errno the hub names (raw_os_error on Windows is a Win32 code); negative ones have no Windows twin.
pub mod errno {
    pub const EACCES: i32 = 5; pub const EADDRINUSE: i32 = 10048; pub const EADDRNOTAVAIL: i32 = 10049; pub const EDQUOT: i32 = 1295;
    pub const EINVAL: i32 = 87; pub const EIO: i32 = 1117; pub const EISDIR: i32 = -21; pub const ELOOP: i32 = 1921; pub const EMFILE: i32 = 4; pub const ENFILE: i32 = -23;
    pub const ENOSPC: i32 = 112; pub const ENOTDIR: i32 = 267; pub const ENOTSUP: i32 = 50; pub const EPERM: i32 = -1; pub const EROFS: i32 = 19;
    /// "Text file busy": Windows' sharing violation (a program still open for writing).
    pub const ETXTBSY: i32 = 32;
}

pub mod proc {
    use super::{wide, CloseHandle, HANDLE};
    use windows_sys::Win32::System::{JobObjects::*, Threading::*};
    pub const HUP: i32 = 1; pub const KILL: i32 = 9; pub const TERM: i32 = 15;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200; const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    fn job_name(pid: u32) -> Vec<u16> { wide(&format!("Local\\backplane-group-{pid}")) }
    /// Handles of groups that die with the hub (kill-on-close): the hub holds them until the group is killed, or it exits.
    static KEPT: std::sync::Mutex<Vec<(u32, usize)>> = std::sync::Mutex::new(Vec::new());

    /// Its own process group and no console window. The Job Object that makes it a group is made by `adopt` after the spawn.
    pub fn own_group(c: &mut std::process::Command) { use std::os::windows::process::CommandExt; c.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW); }
    pub fn own_group_tokio(c: &mut tokio::process::Command) { c.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW); }
    pub fn agent_group(c: &mut tokio::process::Command) { own_group_tokio(c) }
    /// Put a just-started leader in its group's Job Object; with `die_with_hub` the job is killed when the hub exits (the PDEATHSIG of Linux).
    /// What the leader starts in the moment before this is not in the group.
    pub fn adopt(pid: u32, die_with_hub: bool) {
        if pid == 0 { return }
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), job_name(pid).as_ptr()); if job.is_null() { return }
            if die_with_hub {
                let mut l: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed(); l.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(job, JobObjectExtendedLimitInformation, &l as *const _ as *const _, std::mem::size_of_val(&l) as u32);
            }
            let p = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if !p.is_null() { AssignProcessToJobObject(job, p); CloseHandle(p); }
            if die_with_hub { KEPT.lock().unwrap_or_else(|e| e.into_inner()).push((pid, job as usize)) } else { CloseHandle(job); } // a closed handle leaves the job for as long as a process is in it
        }
    }
    /// End the group a leader made (any signal ends it). True if anything was ended.
    pub fn kill_group(pid: u32, sig: i32) -> bool {
        if pid == 0 { return false }
        let ok = unsafe { let j = OpenJobObjectW(8 /* JOB_OBJECT_TERMINATE */, 0, job_name(pid).as_ptr()); if j.is_null() { false } else { let r = TerminateJobObject(j, 1) != 0; CloseHandle(j); r } };
        { let mut k = KEPT.lock().unwrap_or_else(|e| e.into_inner()); k.retain(|(p, h)| if *p == pid { unsafe { CloseHandle(*h as HANDLE); } false } else { true }); }
        ok || kill(pid, sig)
    }
    pub fn kill(pid: u32, _sig: i32) -> bool {
        if pid == 0 { return false }
        unsafe { let h = OpenProcess(PROCESS_TERMINATE, 0, pid); if h.is_null() { return false } let r = TerminateProcess(h, 1) != 0; CloseHandle(h); r }
    }
    pub fn alive(pid: u32) -> bool {
        if pid == 0 { return false }
        unsafe { let h = OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid); if h.is_null() { return false } let r = WaitForSingleObject(h, 0) == 0x102; CloseHandle(h); r } // WAIT_TIMEOUT: still running
    }
    /// There, even if another user's (access denied).
    pub fn exists(pid: u32) -> bool {
        if pid == 0 { return false }
        if alive(pid) { return true }
        unsafe { let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid); if h.is_null() { return std::io::Error::last_os_error().raw_os_error() == Some(5) } CloseHandle(h); false }
    }
    pub fn signal(_: &std::process::ExitStatus) -> Option<i32> { None }
    /// When a process started (100 ns since 1601): with the pid it names one process.
    pub fn start_time(pid: u32) -> Option<u64> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid); if h.is_null() { return None }
            let z = windows_sys::Win32::Foundation::FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 }; let (mut c, mut e, mut k, mut u) = (z, z, z, z);
            let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0; let mut code = 0u32; let running = GetExitCodeProcess(h, &mut code) != 0 && code == 259; CloseHandle(h);
            (ok && running).then(|| (c.dwHighDateTime as u64) << 32 | c.dwLowDateTime as u64)
        }
    }
}

/// The hub process itself.
pub mod hub {
    /// One hub per home: an exclusive lock on the file, not waited for. False if another process holds it.
    pub fn lock(f: &std::fs::File) -> bool { f.try_lock().is_ok() }
    /// No descriptor limit to raise.
    pub fn raise_nofile() {}
    /// Ctrl+C or Ctrl+Break, the console closing, logoff or shutdown end the hub, said in hub.log, exit 130 (as SIGINT).
    pub fn exit_on_signals() {
        tokio::spawn(async { if tokio::signal::ctrl_c().await.is_ok() { eprintln!("backplane-hub: Ctrl+C received, exiting"); std::process::exit(130) } });
        use tokio::signal::windows as w;
        macro_rules! on { ($name:expr, $s:expr) => { if let Ok(mut s) = $s { tokio::spawn(async move { s.recv().await; eprintln!("backplane-hub: {} received, exiting", $name); std::process::exit(130) }); } } }
        on!("Ctrl+Break", w::ctrl_break()); on!("close", w::ctrl_close()); on!("shutdown", w::ctrl_shutdown()); on!("logoff", w::ctrl_logoff());
    }
    /// The hub's scripts are POSIX shell: `sh`/`bash` mean Git for Windows' (on PATH, or beside git.exe), which Windows agents need anyway.
    pub fn program(p: &std::ffi::OsStr) -> std::ffi::OsString {
        let name = match p.to_str() { Some("sh") | Some("/bin/sh") => "sh.exe", Some("bash") | Some("/bin/bash") => "bash.exe", _ => return p.to_os_string() };
        static FOUND: std::sync::OnceLock<Vec<(String, std::ffi::OsString)>> = std::sync::OnceLock::new();
        let found = FOUND.get_or_init(|| ["sh.exe", "bash.exe"].iter().filter_map(|n| shell_path(n).map(|f| (n.to_string(), f.into_os_string()))).collect());
        found.iter().find(|(n, _)| n == name).map_or_else(|| name.into(), |(_, f)| f.clone())
    }
    fn shell_path(name: &str) -> Option<std::path::PathBuf> {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let dirs: Vec<std::path::PathBuf> = std::env::split_paths(&path).collect();
        // git.exe in <Git>\cmd or <Git>\bin: its sh.exe is in <Git>\bin
        let from_git = dirs.iter().filter(|d| d.join("git.exe").is_file()).filter_map(|d| d.parent().map(|g| g.join("bin")));
        let installed = ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"].iter().filter_map(|v| std::env::var_os(v)).flat_map(|r| { let r = std::path::PathBuf::from(r); [r.join("Git").join("bin"), r.join("Programs").join("Git").join("bin")] });
        // PATH first, but not System32's bash.exe (WSL's, another machine's shell)
        dirs.iter().filter(|d| !d.to_string_lossy().to_ascii_lowercase().contains("system32")).cloned().chain(from_git).chain(installed).map(|d| d.join(name)).find(|f| f.is_file())
    }
}

pub mod time {
    /// The fields of a broken-down time the hub reads (as libc's `tm`).
    pub struct Tm { pub tm_year: i32, pub tm_yday: i32, pub tm_mon: i32, pub tm_mday: i32, pub tm_hour: i32, pub tm_min: i32 }
    /// `at` broken down: UTC when no zone is known, else in the machine's zone (Windows reads no IANA zone names; the person's zone is the machine's).
    pub fn local_tm(zone: &str, at: u64) -> Tm {
        let t = at as i64 + if zone.is_empty() { 0 } else { local_offset(at) };
        let (days, secs) = (t.div_euclid(86400), t.rem_euclid(86400));
        // civil from days (H. Hinnant)
        let z = days + 719468; let era = z.div_euclid(146097); let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153; let d = doy - (153 * mp + 2) / 5 + 1; let m = if mp < 10 { mp + 3 } else { mp - 9 }; let y = yoe + era * 400 + i64::from(m <= 2);
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let before = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][(m - 1) as usize] + i64::from(leap && m > 2);
        Tm { tm_year: (y - 1900) as i32, tm_yday: (before + d - 1) as i32, tm_mon: (m - 1) as i32, tm_mday: d as i32, tm_hour: (secs / 3600) as i32, tm_min: (secs % 3600 / 60) as i32 }
    }
    /// The machine zone's offset from UTC at `now`, in seconds east (daylight saving included).
    pub fn local_offset(now: u64) -> i64 {
        use windows_sys::Win32::{Foundation::{FILETIME, SYSTEMTIME}, System::Time::{FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime}};
        let ft = (now + 11_644_473_600) * 10_000_000; let utc_ft = FILETIME { dwLowDateTime: ft as u32, dwHighDateTime: (ft >> 32) as u32 };
        unsafe {
            let (mut utc, mut local): (SYSTEMTIME, SYSTEMTIME) = (std::mem::zeroed(), std::mem::zeroed());
            if FileTimeToSystemTime(&utc_ft, &mut utc) == 0 || SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) == 0 { return 0 }
            let mut lf: FILETIME = std::mem::zeroed(); if SystemTimeToFileTime(&local, &mut lf) == 0 { return 0 }
            let l = (lf.dwHighDateTime as u64) << 32 | lf.dwLowDateTime as u64;
            (l as i64 - ft as i64) / 10_000_000
        }
    }
}

pub mod net {
    /// The machine's IPv4 addresses that are not loopback, of adapters that are up.
    pub fn non_loopback_v4() -> Vec<String> {
        use windows_sys::Win32::{NetworkManagement::{IpHelper::*, Ndis::IfOperStatusUp}, Networking::WinSock::{AF_INET, SOCKADDR_IN}};
        let mut out = vec![]; let mut size: u32 = 16 << 10;
        for _ in 0..3 {
            let mut buf = vec![0u64; size as usize / 8 + 1]; let first = buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
            let r = unsafe { GetAdaptersAddresses(AF_INET as u32, GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER, std::ptr::null(), first, &mut size) };
            if r == 111 { continue } // ERROR_BUFFER_OVERFLOW: size now says how much
            if r != 0 { return out }
            let mut a = first;
            while !a.is_null() { unsafe {
                if (*a).IfType != IF_TYPE_SOFTWARE_LOOPBACK && (*a).OperStatus == IfOperStatusUp {
                    let mut u = (*a).FirstUnicastAddress;
                    while !u.is_null() { let sa = (*u).Address.lpSockaddr; if !sa.is_null() && (*sa).sa_family == AF_INET { let sin = &*(sa as *const SOCKADDR_IN); out.push(std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.S_un.S_addr)).to_string()); } u = (*u).Next; }
                }
                a = (*a).Next;
            } }
            return out
        }
        out
    }
}

/// The local socket KiCad's API listens on (nng `ipc://<path>`): nng on Windows makes it the named pipe `\\.\pipe\<path>`.
pub mod ipc {
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions};
    pub type Stream = NamedPipeClient;
    pub fn pipe_name(path: &str) -> String { format!(r"\\.\pipe\{path}") }
    pub async fn connect(path: &str) -> std::io::Result<Stream> {
        for _ in 0..50 {
            match ClientOptions::new().open(pipe_name(path)) { Err(e) if e.raw_os_error() == Some(231) => tokio::time::sleep(std::time::Duration::from_millis(20)).await, r => return r } // ERROR_PIPE_BUSY: every instance taken, wait for one
        }
        ClientOptions::new().open(pipe_name(path))
    }
    /// A test's stand-in server: one pipe instance waiting at a time.
    pub struct Listener { name: String, next: std::sync::Mutex<Option<NamedPipeServer>> }
    pub type Served = NamedPipeServer;
    pub fn bind(path: &str) -> std::io::Result<Listener> {
        let name = pipe_name(path); let first = ServerOptions::new().first_pipe_instance(true).create(&name)?;
        Ok(Listener { name, next: std::sync::Mutex::new(Some(first)) })
    }
    pub async fn accept(l: &Listener) -> std::io::Result<Served> {
        let s = match l.next.lock().unwrap().take() { Some(s) => s, None => ServerOptions::new().create(&l.name)? };
        s.connect().await?;
        *l.next.lock().unwrap() = Some(ServerOptions::new().create(&l.name)?);
        Ok(s)
    }
}

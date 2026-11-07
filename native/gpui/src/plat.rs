//! The little the client does differently on Unix (Linux, macOS) and Windows. On Unix each item is the call it always made.

/// This process's CPU time (user + system) and peak resident memory, in ms and MB.
#[cfg(unix)]
pub fn usage() -> (f64, f64) {
    let mut u: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut u) };
    let tv = |t: libc::timeval| t.tv_sec as f64 * 1000.0 + t.tv_usec as f64 / 1000.0;
    (tv(u.ru_utime) + tv(u.ru_stime), u.ru_maxrss as f64 / if cfg!(target_os = "macos") { 1048576.0 } else { 1024.0 })
}
#[cfg(windows)]
pub fn usage() -> (f64, f64) {
    use windows_sys::Win32::{Foundation::FILETIME, System::{ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS}, Threading::{GetCurrentProcess, GetProcessTimes}}};
    unsafe {
        let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 }; let (mut c, mut e, mut k, mut u) = (z, z, z, z);
        let ms = |t: FILETIME| ((t.dwHighDateTime as u64) << 32 | t.dwLowDateTime as u64) as f64 / 10_000.0;
        let cpu = if GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) != 0 { ms(k) + ms(u) } else { 0.0 };
        let mut m: PROCESS_MEMORY_COUNTERS = std::mem::zeroed(); m.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let peak = if GetProcessMemoryInfo(GetCurrentProcess(), &mut m, m.cb) != 0 { m.PeakWorkingSetSize as f64 / 1048576.0 } else { 0.0 };
        (cpu, peak)
    }
}

/// Local time now: (year, month 1-12, day, hour, minute, second).
#[cfg(unix)]
pub fn local_now() -> (i32, i32, i32, i32, i32, i32) {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as libc::time_t;
    // SAFETY: both pointers are to live locals; localtime_r writes only to `tm`.
    let tm = unsafe { let mut tm: libc::tm = std::mem::zeroed(); libc::localtime_r(&secs, &mut tm); tm };
    (tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
}
#[cfg(windows)]
pub fn local_now() -> (i32, i32, i32, i32, i32, i32) {
    let t = unsafe { let mut t = std::mem::zeroed(); windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut t); t };
    (t.wYear as i32, t.wMonth as i32, t.wDay as i32, t.wHour as i32, t.wMinute as i32, t.wSecond as i32)
}

/// A folder only this user may enter (0700). Windows: the user's profile folders are already private.
pub fn private_dir(p: &std::path::Path) -> bool {
    #[cfg(unix)] { use std::os::unix::fs::{DirBuilderExt, PermissionsExt}; std::fs::DirBuilder::new().recursive(true).mode(0o700).create(p).is_ok() && std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700)).is_ok() }
    #[cfg(windows)] { std::fs::create_dir_all(p).is_ok() }
}
/// Create or truncate a file only this user may read (0600).
pub fn private_file(p: &std::path::Path) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new(); o.write(true).create(true).truncate(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
    o.open(p)
}

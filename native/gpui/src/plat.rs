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
/// Create or truncate a file only this user may read (0600; Windows: make_private).
pub fn private_file(p: &std::path::Path) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new(); o.write(true).create(true).truncate(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
    let f = o.open(p)?; make_private(p)?; Ok(f)
}
/// Windows: the file's access is this user, SYSTEM and Administrators, full control, nothing inherited (a protected DACL). A file under
/// %APPDATA% gets that from its folder anyway; one written elsewhere (a settings folder the person chose) inherited Users and Authenticated Users.
/// Unix: the mode the file was opened with (0600) already says it.
pub fn make_private(p: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)] { win_acl::user_only(p) }
    #[cfg(not(windows))] { let _ = p; Ok(()) }
}
#[cfg(windows)]
mod win_acl {
    use windows_sys::Win32::{Foundation::{CloseHandle, LocalFree, HANDLE}, Security::{Authorization::*, *}, System::Threading::{GetCurrentProcess, OpenProcessToken}};
    fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }
    /// The current user's SID as text (S-1-5-21-...).
    fn user_sid() -> std::io::Result<String> {
        unsafe {
            let mut tok: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut tok) == 0 { return Err(std::io::Error::last_os_error()) }
            let mut buf = vec![0u8; 512]; let mut len = 0u32;
            let ok = GetTokenInformation(tok, TokenUser, buf.as_mut_ptr() as _, buf.len() as u32, &mut len); CloseHandle(tok);
            if ok == 0 { return Err(std::io::Error::last_os_error()) }
            let tu = &*(buf.as_ptr() as *const TOKEN_USER);
            let mut s: windows_sys::core::PWSTR = std::ptr::null_mut();
            if ConvertSidToStringSidW(tu.User.Sid, &mut s) == 0 { return Err(std::io::Error::last_os_error()) }
            let n = (0..).take_while(|&i| *s.add(i) != 0).count();
            let out = String::from_utf16_lossy(std::slice::from_raw_parts(s, n)); LocalFree(s as _); Ok(out)
        }
    }
    pub fn user_only(p: &std::path::Path) -> std::io::Result<()> {
        /* i18n-ok: Windows ACL and stderr before the window opens */ let sddl = wide(&format!("D:P(A;;FA;;;{})(A;;FA;;;SY)(A;;FA;;;BA)", user_sid()?));
        unsafe {
            let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, std::ptr::null_mut()) == 0 { return Err(std::io::Error::last_os_error()) }
            let (mut present, mut defaulted, mut dacl) = (0, 0, std::ptr::null_mut());
            let got = GetSecurityDescriptorDacl(sd, &mut present, &mut dacl, &mut defaulted);
            let path = wide(&p.to_string_lossy());
            let r = if got == 0 { Err(std::io::Error::last_os_error()) } else {
                match SetNamedSecurityInfoW(path.as_ptr(), SE_FILE_OBJECT, DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION, std::ptr::null_mut(), std::ptr::null_mut(), dacl, std::ptr::null()) { 0 => Ok(()), e => Err(std::io::Error::from_raw_os_error(e as i32)) }
            };
            LocalFree(sd as _); r
        }
    }
    #[cfg(test)]
    mod tests {
        /// A file written in a folder that lets every user read: after make_private only the user, SYSTEM and Administrators are on it, none inherited.
        #[test] fn a_private_file_lists_only_the_user_system_and_administrators() {
            let d = std::env::temp_dir().join(format!("bp-acl-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
            let f = d.join("x.token"); std::fs::write(&f, "secret").unwrap();
            super::user_only(&f).unwrap();
            let out = std::process::Command::new("icacls").arg(&f).output().unwrap(); let t = String::from_utf8_lossy(&out.stdout).to_string();
            let user = std::env::var("USERNAME").unwrap();
            assert!(t.contains(&user) && t.contains("NT AUTHORITY\\SYSTEM") && t.contains("BUILTIN\\Administrators"), "{t}");
            /* i18n-ok: Windows ACL and stderr before the window opens */ for wide in ["Users:", "Everyone", "Authenticated Users", "(I)"] { assert!(!t.contains(wide), "{wide} on a private file: {t}") }
            /* i18n-ok: Windows ACL and stderr before the window opens */ assert_eq!(std::fs::read_to_string(&f).unwrap(), "secret", "still the user's to read");
            let _ = std::fs::remove_dir_all(&d);
        }
    }
}

/// The user's own folder: $HOME (Windows: %USERPROFILE% when HOME is not set, as outside Git's shell).
pub fn user_home() -> Option<std::path::PathBuf> {
    let h = std::env::var_os("HOME").filter(|h| !h.is_empty());
    #[cfg(windows)] let h = h.or_else(|| std::env::var_os("USERPROFILE").filter(|h| !h.is_empty()));
    h.map(std::path::PathBuf::from)
}
/// Where this device's settings live: $XDG_CONFIG_HOME, else ~/.config (Windows: %APPDATA%).
pub fn config_base() -> Option<std::path::PathBuf> {
    let x = std::env::var_os("XDG_CONFIG_HOME").filter(|p| !p.is_empty()).map(std::path::PathBuf::from);
    #[cfg(windows)] let x = x.or_else(|| std::env::var_os("APPDATA").filter(|p| !p.is_empty()).map(std::path::PathBuf::from));
    x.or_else(|| user_home().map(|h| h.join(".config")))
}
/// Where caches live: $XDG_CACHE_HOME, else ~/.cache (Windows: %LOCALAPPDATA%).
pub fn cache_base() -> Option<std::path::PathBuf> {
    let x = std::env::var_os("XDG_CACHE_HOME").map(std::path::PathBuf::from);
    #[cfg(windows)] let x = x.or_else(|| std::env::var_os("LOCALAPPDATA").filter(|p| !p.is_empty()).map(std::path::PathBuf::from));
    x.or_else(|| user_home().map(|h| h.join(".cache")))
}

/// Why this window cannot open on Linux's displays, when it cannot: it draws through X11 only (gpui's `x11` feature), so a Wayland session without Xwayland
/// (`WAYLAND_DISPLAY` and no `DISPLAY`) is said plainly instead of gpui's panic ("neither DISPLAY nor WAYLAND_DISPLAY is set").
pub fn display_problem(display: Option<&str>, wayland: Option<&str>) -> Option<String> {
    if !cfg!(target_os = "linux") || display.map_or(false, |d| !d.is_empty()) { return None }
    Some(match wayland.filter(|w| !w.is_empty()) {
        /* i18n-ok: Windows ACL and stderr before the window opens */ Some(w) => format!("Backplane's window draws through X11 and this Wayland session ({w}) has no X server: DISPLAY is not set. Enable Xwayland (most compositors start it on demand; sway: `xwayland enable`) and start Backplane from the session, or set DISPLAY."),
        /* i18n-ok: Windows ACL and stderr before the window opens */ None => "No display: neither DISPLAY nor WAYLAND_DISPLAY is set. Start Backplane from a desktop session (an X server or Xwayland).".into(),
    })
}
#[cfg(test)]
mod display_tests {
    #[test] fn a_wayland_session_without_xwayland_is_said_not_panicked() {
        if !cfg!(target_os = "linux") { return }
        assert_eq!(super::display_problem(Some(":0"), Some("wayland-1")), None);
        assert!(super::display_problem(None, Some("wayland-1")).unwrap().contains("Xwayland"));
        assert!(super::display_problem(Some(""), None).unwrap().contains("No display"));
    }
}

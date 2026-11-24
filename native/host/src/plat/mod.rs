//! The little that differs between Unix (Linux, macOS) and Windows, in one place. Callers use `crate::plat::...` and never `std::os::unix`,
//! `libc::kill` or `pre_exec` themselves. On Unix every item is the call the hub always made (std's own extension traits re-exported, the
//! same libc calls), so Linux and macOS behave exactly as before; `windows.rs` gives the nearest Windows meaning and says where there is none.
#[cfg(unix)] mod unix;
#[cfg(unix)] pub use unix::*;
#[cfg(windows)] mod windows;
#[cfg(windows)] pub use windows::*;
#[cfg(target_os = "linux")]
pub mod browser_peer;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn effective_uid() -> u32 { unsafe { libc::geteuid() } }

pub(crate) fn is_tailscale_windows_exe_name(name: &str) -> bool {
    let base = name.rsplit(['\\', '/']).next().unwrap_or(name);
    base.eq_ignore_ascii_case("tailscaled.exe")
        || base.eq_ignore_ascii_case("tailscale-ipn.exe")
        || base.eq_ignore_ascii_case("tailscale.exe")
}

pub(crate) fn is_trusted_tailscale_windows_path(
    path: &str,
    program_files: Option<&str>,
    program_files_x86: Option<&str>,
) -> bool {
    if !is_tailscale_windows_exe_name(path) {
        return false;
    }
    let norm = |s: &str| {
        s.trim()
            .trim_end_matches(['\\', '/'])
            .replace('/', "\\")
            .to_ascii_lowercase()
    };
    let p = norm(path);
    let mut prefixes = vec![r"c:\program files\tailscale".to_string()];
    if let Some(pf) = program_files.filter(|s| !s.trim().is_empty()) {
        prefixes.push(format!(r"{}\tailscale", norm(pf)));
    }
    if let Some(pf86) = program_files_x86.filter(|s| !s.trim().is_empty()) {
        prefixes.push(format!(r"{}\tailscale", norm(pf86)));
    }
    prefixes.into_iter().any(|dir| {
        p.strip_prefix(&format!(r"{dir}\"))
            .is_some_and(|rest| !rest.is_empty() && !rest.contains('\\'))
    })
}

pub(crate) fn is_trusted_tailscale_macos_path(path: &str, home_dir: Option<&str>) -> bool {
    let trimmed = path.trim();
    if trimmed.is_empty() || !trimmed.starts_with('/') {
        return false;
    }
    if std::path::Path::new(trimmed)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }
    if trimmed.starts_with("/Applications/Tailscale.app/") {
        return true;
    }
    if let Some(home) = home_dir.map(str::trim).filter(|h| h.starts_with('/')) {
        let home = home.trim_end_matches('/');
        if trimmed.starts_with(&format!("{home}/Applications/Tailscale.app/")) {
            return true;
        }
    }
    if trimmed.starts_with("/Library/SystemExtensions/")
        && trimmed
            .split('/')
            .any(|seg| seg.starts_with("io.tailscale.ipn.macsys"))
    {
        return true;
    }
    if matches!(
        trimmed,
        "/opt/homebrew/bin/tailscaled" | "/usr/local/bin/tailscaled"
    ) {
        return true;
    }
    for prefix in ["/opt/homebrew/Cellar/tailscale/", "/usr/local/Cellar/tailscale/"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if let Some((_, tail)) = rest.split_once('/') {
                if tail == "bin/tailscaled" {
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) fn parse_macos_netstat_pid(
    output: &str,
    local_port: u16,
    remote_port: u16,
) -> Option<u32> {
    let want_local = format!("127.0.0.1.{remote_port}");
    let want_foreign = format!("127.0.0.1.{local_port}");
    let mut pid_col: usize = 8;
    for line in output.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if let Some(state_idx) = cols.iter().position(|c| *c == "(state)") {
            if let Some(pid_idx) = cols.iter().position(|c| *c == "pid") {
                if pid_idx > state_idx {
                    pid_col = 5 + (pid_idx - state_idx);
                }
            }
            continue;
        }
        if cols.first().copied() == Some("tcp4")
            && cols.get(3).copied() == Some(want_local.as_str())
            && cols.get(4).copied() == Some(want_foreign.as_str())
            && cols.get(5).copied() == Some("ESTABLISHED")
        {
            if let Some(pid) = cols.get(pid_col).and_then(|s| s.parse::<u32>().ok()) {
                if pid != 0 {
                    return Some(pid);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
mod macos_browser_peer {
    use super::{effective_uid, is_trusted_tailscale_macos_path, parse_macos_netstat_pid};
    use std::net::Ipv4Addr;

    pub fn accepts(stream: &tokio::net::TcpStream) -> bool {
        let (Ok(local), Ok(remote)) = (stream.local_addr(), stream.peer_addr()) else {
            return false;
        };
        if local.ip() != Ipv4Addr::LOCALHOST || remote.ip() != Ipv4Addr::LOCALHOST {
            return false;
        }
        let Ok(out) = crate::proc::std_cmd(crate::proc::Kind::Local, "/usr/sbin/netstat")
            .args(["-anv", "-p", "tcp"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
        else {
            return false;
        };
        if !out.status.success() {
            return false;
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let Some(pid) = parse_macos_netstat_pid(&stdout, local.port(), remote.port()) else {
            return false;
        };
        if pid == 0 || pid == std::process::id() {
            return false;
        }
        let mut info: libc::proc_bsdshortinfo = unsafe { std::mem::zeroed() };
        let rc = unsafe {
            libc::proc_pidinfo(
                pid as libc::c_int,
                libc::PROC_PIDT_SHORTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdshortinfo).cast(),
                std::mem::size_of::<libc::proc_bsdshortinfo>() as libc::c_int,
            )
        };
        if rc != std::mem::size_of::<libc::proc_bsdshortinfo>() as libc::c_int {
            return false;
        }
        let uid = effective_uid();
        if info.pbsi_uid != 0 && info.pbsi_uid != uid {
            return false;
        }
        let mut path_buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let plen = unsafe {
            libc::proc_pidpath(
                pid as libc::c_int,
                path_buf.as_mut_ptr().cast(),
                path_buf.len() as u32,
            )
        };
        if plen <= 0 || (plen as usize) >= path_buf.len() {
            return false;
        }
        let Ok(path) = std::str::from_utf8(&path_buf[..plen as usize]) else {
            return false;
        };
        let home = std::env::var("HOME").ok();
        if !is_trusted_tailscale_macos_path(path, home.as_deref()) {
            return false;
        }
        stream.peer_addr().is_ok()
    }
}

/// Verify that an accepted loopback TCP connection belongs to the Tailscale daemon.
pub fn browser_loopback_peer_ok(stream: &tokio::net::TcpStream) -> bool {
    let (Ok(local), Ok(remote)) = (stream.local_addr(), stream.peer_addr()) else {
        return false;
    };
    if !local.ip().is_loopback()
        || !remote.ip().is_loopback()
        || local.ip() != std::net::Ipv4Addr::LOCALHOST
        || remote.ip() != std::net::Ipv4Addr::LOCALHOST
    {
        return false;
    }
    #[cfg(windows)]
    {
        return browser_peer::accepts(stream);
    }
    #[cfg(target_os = "linux")]
    {
        return browser_peer::accepts(stream);
    }
    #[cfg(target_os = "macos")]
    {
        return macos_browser_peer::accepts(stream);
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        false
    }
}
pub mod dirs;
#[cfg(test)] pub mod tmp;

/// A program Backplane ships, by its full path: beside this one (an install's `builds/<tag>/` folder), else one folder up (a test binary in
/// `target/*/deps`). Never by bare name from PATH: an installed build's folder is not on PATH (0.16.0 on Windows), and a bare name would
/// start whatever program of that name PATH has first.
pub fn helper(name: &str) -> Option<std::path::PathBuf> { helper_in(&std::env::current_exe().ok()?, name, std::env::consts::EXE_SUFFIX) }
/// `helper` for the program `exe` and the system's program suffix (`.exe` on Windows).
/// A test binary in Cargo's newer layout, `target/<profile>/build/<package>/<hash>/out/`, finds it in `target/<profile>` (the stable toolchain on
/// the Windows runner moved test binaries there: bp-sh was not found, and every `sh` argument with a backslash or a quote changed on its way).
pub fn helper_in(exe: &std::path::Path, name: &str, suffix: &str) -> Option<std::path::PathBuf> {
    let file = format!("{name}{suffix}"); let d = exe.parent()?;
    let profile = (d.file_name()? == "out").then(|| d.ancestors().nth(3).filter(|b| b.file_name().is_some_and(|n| n == "build")).and_then(|b| b.parent())).flatten();
    std::iter::once(d.join(&file)).chain(d.parent().map(|u| u.join(&file))).chain(profile.map(|p| p.join(&file))).find(|f| f.is_file())
}
/// A lookup kept once it finds something; a miss is looked for again next time, so a program installed after the hub started (Git for
/// Windows from setup's button) is found without a restart. A `OnceLock` kept the miss for the hub's life: every `sh` failed after Git came.
pub struct Found<T>(std::sync::Mutex<Option<T>>);
impl<T: Clone> Found<T> {
    pub const fn new() -> Self { Found(std::sync::Mutex::new(None)) }
    pub fn get(&self, find: impl FnOnce() -> Option<T>) -> Option<T> {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() { *g = find() }
        g.clone()
    }
}

/// A child's exit as the shell says it: its code, else 128 + the signal that ended it (Unix only has those).
pub fn exit_code(s: &std::process::ExitStatus) -> i32 { s.code().unwrap_or_else(|| 128 + proc::signal(s).unwrap_or(0)) }


/// macOS: the words for a folder its privacy settings keep Backplane out of (Files and Folders), or None. `path` (`~` is home) or the nearest
/// folder above it that is there: reading it fails with EPERM, "Operation not permitted", which is what macOS's privacy control answers (a
/// folder's own permissions answer EACCES). Without them a refused folder only looked empty, and the client stayed where it was with no word.
/// Never on other systems. Reading a protected folder is also what makes macOS ask the person, once, for Backplane.
pub fn privacy_block(path: &str) -> Option<String> {
    if !cfg!(target_os = "macos") || path.is_empty() { return None }
    let home = dirs::home_or_empty();
    let p = match path.strip_prefix('~') { Some(r) if r.is_empty() || r.starts_with('/') => format!("{home}{r}"), _ => path.to_string() };
    for a in std::path::Path::new(&p).ancestors() {
        match std::fs::read_dir(a) {
            Ok(_) => return None,
            Err(e) if e.raw_os_error() == Some(1) => return Some(privacy_words(&a.to_string_lossy(), &home)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return None,
        }
    }
    None
}
/// macOS: `dir` is one of the folders its privacy settings guard (~/Desktop, ~/Documents, ~/Downloads), whose first read asks the person. A look
/// inside that only tags a row (the folder picker's) must not: opening the picker at ~ asked for all three at once.
pub fn privacy_guarded(dir: &str) -> bool { cfg!(target_os = "macos") && guarded_in(dir, &dirs::home_or_empty()) }
fn guarded_in(dir: &str, home: &str) -> bool {
    let h = home.trim_end_matches('/'); let d = dir.trim_end_matches('/');
    !h.is_empty() && ["Documents", "Desktop", "Downloads"].iter().any(|n| d == format!("{h}/{n}"))
}
/// The words for `dir`, refused by macOS's privacy settings: the protected folder it is in (Documents, Desktop, Downloads, a drive) and where
/// the person allows it; any other place is Full Disk Access's.
pub fn privacy_words(dir: &str, home: &str) -> String {
    let h = home.trim_end_matches('/');
    let named = ["Documents", "Desktop", "Downloads"].into_iter().find(|n| { let base = format!("{h}/{n}"); !h.is_empty() && (dir == base || dir.starts_with(&format!("{base}/"))) });
    match named {
        Some(n) => format!("macOS hasn't given Backplane access to {n}: allow it in System Settings → Privacy & Security → Files and Folders, then try again."),
        None if dir.starts_with("/Volumes/") => format!("macOS hasn't given Backplane access to {dir}: allow it in System Settings → Privacy & Security → Files and Folders (removable or network volumes), then try again."),
        None => format!("macOS hasn't given Backplane access to {dir}: allow it in System Settings → Privacy & Security → Full Disk Access, then try again."),
    }
}

#[cfg(test)] mod privacy_tests {
    #[test] fn a_refused_folder_is_named_with_where_to_allow_it() {
        let w = super::privacy_words("/Users/a/Documents/lamp", "/Users/a");
        assert_eq!(w, "macOS hasn't given Backplane access to Documents: allow it in System Settings → Privacy & Security → Files and Folders, then try again.");
        assert!(super::privacy_words("/Users/a/Desktop", "/Users/a/").contains("access to Desktop:"));
        assert!(super::privacy_words("/Users/a/Downloadsx/b", "/Users/a").contains("Full Disk Access"), "only the folder itself and what is in it");
        assert!(super::privacy_words("/Volumes/stick/p", "/Users/a").contains("removable or network volumes"));
        assert!(super::guarded_in("/Users/a/Documents/", "/Users/a") && super::guarded_in("/Users/a/Desktop", "/Users/a/") && !super::guarded_in("/Users/a/Documents/x", "/Users/a") && !super::guarded_in("/Users/a/Music", "/Users/a"));
        // not macOS: never a word, whatever the folder
        if !cfg!(target_os = "macos") { assert_eq!(super::privacy_block("/root/x"), None); }
    }
}

#[cfg(test)] mod helper_tests {
    /// A shipped helper is the file beside the hub (or one folder up for a test binary), by full path, with the system's suffix; never a bare name.
    #[test] fn a_helper_is_found_beside_the_hub_by_full_path() {
        let d = super::tmp::dir("helper"); let build = d.join("builds").join("v0.16.0"); let deps = build.join("deps"); std::fs::create_dir_all(&deps).unwrap();
        let hub = build.join("backplane-hub.exe");
        assert_eq!(super::helper_in(&hub, "bp-sh", ".exe"), None, "nothing there: no helper, never a bare name to try on PATH");
        std::fs::write(build.join("bp-sh.exe"), "x").unwrap();
        assert_eq!(super::helper_in(&hub, "bp-sh", ".exe"), Some(build.join("bp-sh.exe")));
        assert!(super::helper_in(&hub, "bp-sh", ".exe").unwrap().is_absolute());
        assert_eq!(super::helper_in(&deps.join("t.exe"), "bp-sh", ".exe"), Some(build.join("bp-sh.exe")), "a test binary in target/*/deps finds it one folder up");
        let out = build.join("build").join("backplane-hub").join("0123abcd").join("out"); std::fs::create_dir_all(&out).unwrap();
        assert_eq!(super::helper_in(&out.join("backplane_hub-0123abcd.exe"), "bp-sh", ".exe"), Some(build.join("bp-sh.exe")), "and one in Cargo's build/<package>/<hash>/out");
        assert_eq!(super::helper_in(&build.join("x").join("y").join("z").join("out").join("t.exe"), "bp-sh", ".exe"), None, "only under a build folder");
        assert_eq!(super::helper_in(&hub, "bp-sh", ""), None, "the suffix is the system's: bp-sh is not bp-sh.exe");
        // after an update the hub runs from the new build's folder, and its helpers are that folder's
        let next = d.join("builds").join("v0.16.1"); std::fs::create_dir_all(&next).unwrap(); std::fs::write(next.join("bp-sh.exe"), "y").unwrap();
        assert_eq!(super::helper_in(&next.join("backplane-hub.exe"), "bp-sh", ".exe"), Some(next.join("bp-sh.exe")));
        // the real one: this test binary's own folder or the one above
        if let Some(p) = super::helper("does-not-exist-anywhere") { panic!("found {p:?}") }
    }
    /// A miss is looked for again (Git installed while the hub runs); a hit is kept.
    #[test] fn a_lookup_keeps_hits_not_misses() {
        let f: super::Found<u32> = super::Found::new(); let n = std::cell::Cell::new(0);
        assert_eq!(f.get(|| { n.set(n.get() + 1); None }), None);
        assert_eq!(f.get(|| { n.set(n.get() + 1); Some(7) }), Some(7), "looked for again after a miss");
        assert_eq!(f.get(|| { n.set(n.get() + 1); Some(9) }), Some(7), "kept once found");
        assert_eq!(n.get(), 2);
    }
}

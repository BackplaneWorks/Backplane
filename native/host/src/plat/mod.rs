//! The little that differs between Unix (Linux, macOS) and Windows, in one place. Callers use `crate::plat::...` and never `std::os::unix`,
//! `libc::kill` or `pre_exec` themselves. On Unix every item is the call the hub always made (std's own extension traits re-exported, the
//! same libc calls), so Linux and macOS behave exactly as before; `windows.rs` gives the nearest Windows meaning and says where there is none.
#[cfg(unix)] mod unix;
#[cfg(unix)] pub use unix::*;
#[cfg(windows)] mod windows;
#[cfg(windows)] pub use windows::*;
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

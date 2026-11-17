//! The little that differs between Unix (Linux, macOS) and Windows, in one place. Callers use `crate::plat::...` and never `std::os::unix`,
//! `libc::kill` or `pre_exec` themselves. On Unix every item is the call the hub always made (std's own extension traits re-exported, the
//! same libc calls), so Linux and macOS behave exactly as before; `windows.rs` gives the nearest Windows meaning and says where there is none.
#[cfg(unix)] mod unix;
#[cfg(unix)] pub use unix::*;
#[cfg(windows)] mod windows;
#[cfg(windows)] pub use windows::*;
pub mod dirs;
#[cfg(test)] pub mod tmp;

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

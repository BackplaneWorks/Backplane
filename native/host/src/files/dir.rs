//! files.dir: server.bend's DirJob.sh in Rust. One folder of a thread's project: folders then files,
//! "d|f <tab> size <tab> name" lines, hidden names (glob `*`) left out, symlinked folders left out,
//! files by what they point at, 2000 of each, byte order (the shell's glob in the C locale).
use super::fm;
use std::path::Path;

/// DirJob.sent on refusal (since PR #960): one message for a folder that is missing, a link, or outside the project.
pub const UNAVAILABLE: &str = "that folder is unavailable or outside this project";

pub const MAX: usize = 2000;

/// The listing text, or the shell's one-line error ("no such folder").
pub fn list(root: &Path, rel: &str) -> Result<String, &'static str> {
    let dir = fm::contained(root, rel).ok_or(UNAVAILABLE)?;
    let rd = std::fs::read_dir(&dir).map_err(|_| UNAVAILABLE)?;
    let mut names: Vec<std::ffi::OsString> = rd.filter_map(|e| e.ok().map(|e| e.file_name())).collect();
    use std::os::unix::ffi::OsStrExt;
    names.sort_by(|a, b| a.to_string_lossy().encode_utf16().cmp(b.to_string_lossy().encode_utf16())); // JS string order (UTF-16 code units)
    let (mut dirs, mut files) = (String::new(), String::new());
    let (mut nd, mut nf) = (0, 0);
    for n in &names {
        let Some(name) = n.to_str() else { continue };
        // `*` skips dotfiles; a name holding a newline or tab would break the line format: left out
        // zip_listing: hidden names and names that could forge a listing record (tab, CR, LF, backslash) are left out
        if name.starts_with('.') || name.contains(['\n', '\t', '\r', '\\']) { continue }
        let p = dir.join(n);
        // symlinks and non-regular entries are never exposed (a link to a file is not listed either)
        let Ok(lm) = std::fs::symlink_metadata(&p) else { continue };
        if lm.is_dir() { if nd < MAX { dirs.push_str(&format!("d\t0\t{name}\n")); nd += 1 } }
        else if lm.is_file() && nf < MAX { files.push_str(&format!("f\t{}\t{name}\n", lm.len())); nf += 1 }
    }
    Ok(dirs + &files)
}

#[cfg(test)]
mod tests {
    use super::*; use std::os::unix::fs::symlink;
    fn tmp(n: &str) -> std::path::PathBuf { let p = std::env::temp_dir().join(format!("bp-dir-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    #[test] fn lists_folders_then_files_hides_dotfiles_and_symlinked_folders() {
        let b = tmp("l"); let r = b.join("r"); std::fs::create_dir_all(r.join("zdir")).unwrap(); std::fs::create_dir_all(r.join("adir")).unwrap(); std::fs::create_dir_all(r.join(".git")).unwrap();
        std::fs::write(r.join("b.txt"), "12345").unwrap(); std::fs::write(r.join("a.kicad_pcb"), "").unwrap(); std::fs::write(r.join(".env"), "k").unwrap(); std::fs::write(r.join("bad\tname"), "x").unwrap();
        std::fs::create_dir_all(b.join("out")).unwrap(); symlink(b.join("out"), r.join("linkdir")).unwrap(); symlink(r.join("b.txt"), r.join("linkfile")).unwrap(); symlink("/nonexistent", r.join("dangling")).unwrap();
        assert_eq!(list(&r, "").unwrap(), "d\t0\tadir\nd\t0\tzdir\nf\t0\ta.kicad_pcb\nf\t5\tb.txt\n");
        assert_eq!(list(&r, "adir").unwrap(), ""); let _ = std::fs::remove_dir_all(&b);
    }
    #[test] fn escapes_are_no_such_folder() {
        let b = tmp("e"); let r = b.join("r"); std::fs::create_dir_all(&r).unwrap(); symlink("/etc", r.join("etc")).unwrap();
        for p in ["..", "etc", "/etc", "x/../..", "missing", "a\0b"] { assert_eq!(list(&r, p), Err(UNAVAILABLE), "{p}") }
        assert_eq!(list(&b.join("none"), ""), Err(UNAVAILABLE)); let _ = std::fs::remove_dir_all(&b);
    }
    #[test] fn bounded_at_2000_each() {
        let b = tmp("m"); for i in 0..2105 { std::fs::write(b.join(format!("f{i:05}")), "").unwrap() } for i in 0..2005 { std::fs::create_dir(b.join(format!("d{i:05}"))).unwrap() }
        let t = list(&b, "").unwrap(); assert_eq!(t.lines().filter(|l| l.starts_with("d\t")).count(), 2000); assert_eq!(t.lines().filter(|l| l.starts_with("f\t")).count(), 2000); let _ = std::fs::remove_dir_all(&b);
    }
}

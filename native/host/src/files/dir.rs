//! files.dir: server.bend's DirJob.sh in Rust. One folder of a thread's project: folders then files,
//! "d|f <tab> size <tab> name" lines, hidden names (glob `*`) left out, symlinked folders left out,
//! files by what they point at, 2000 of each, byte order (the shell's glob in the C locale).
use super::fm;
use std::path::Path;

pub const MAX: usize = 2000;

/// The listing text, or the shell's one-line error ("no such folder").
pub fn list(root: &Path, rel: &str) -> Result<String, &'static str> {
    let dir = fm::contained(root, rel).ok_or("no such folder")?;
    let rd = std::fs::read_dir(&dir).map_err(|_| "no such folder")?;
    let mut names: Vec<std::ffi::OsString> = rd.filter_map(|e| e.ok().map(|e| e.file_name())).collect();
    use std::os::unix::ffi::OsStrExt;
    names.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    let (mut dirs, mut files) = (String::new(), String::new());
    let (mut nd, mut nf) = (0, 0);
    for n in &names {
        let Some(name) = n.to_str() else { continue };
        // `*` skips dotfiles; a name holding a newline or tab would break the line format: left out
        if name.starts_with('.') || name.contains(['\n', '\t']) { continue }
        let p = dir.join(n);
        let lm = std::fs::symlink_metadata(&p);
        let Ok(lm) = lm else { continue };
        if lm.is_dir() && nd < MAX { dirs.push_str(&format!("d\t0\t{name}\n")); nd += 1 }
        else if nf < MAX && !lm.is_dir() {
            // [ -f f ] follows links: a link to a file lists with the target's size
            let md = if lm.file_type().is_symlink() { std::fs::metadata(&p).ok() } else { Some(lm) };
            if let Some(md) = md.filter(|m| m.is_file()) { files.push_str(&format!("f\t{}\t{name}\n", md.len())); nf += 1 }
        }
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
        assert_eq!(list(&r, "").unwrap(), "d\t0\tadir\nd\t0\tzdir\nf\t0\ta.kicad_pcb\nf\t5\tb.txt\nf\t5\tlinkfile\n");
        assert_eq!(list(&r, "adir").unwrap(), ""); let _ = std::fs::remove_dir_all(&b);
    }
    #[test] fn escapes_are_no_such_folder() {
        let b = tmp("e"); let r = b.join("r"); std::fs::create_dir_all(&r).unwrap(); symlink("/etc", r.join("etc")).unwrap();
        for p in ["..", "etc", "/etc", "x/../..", "missing", "a\0b"] { assert_eq!(list(&r, p), Err("no such folder"), "{p}") }
        assert_eq!(list(&b.join("none"), ""), Err("no such folder")); let _ = std::fs::remove_dir_all(&b);
    }
    #[test] fn bounded_at_2000_each() {
        let b = tmp("m"); for i in 0..2105 { std::fs::write(b.join(format!("f{i:05}")), "").unwrap() } for i in 0..2005 { std::fs::create_dir(b.join(format!("d{i:05}"))).unwrap() }
        let t = list(&b, "").unwrap(); assert_eq!(t.lines().filter(|l| l.starts_with("d\t")).count(), 2000); assert_eq!(t.lines().filter(|l| l.starts_with("f\t")).count(), 2000); let _ = std::fs::remove_dir_all(&b);
    }
}

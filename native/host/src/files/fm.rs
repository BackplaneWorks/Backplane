//! core/fm in Rust: what the Files tab and /img may name. Security rules live here, with
//! property tests over hostile corpora (test/files/hostile_paths.tsv). Laws fm_kind_*, fm_safe_*,
//! fm_files_safe_*, fm_parent_*, fm_join_root, plus the symlink/containment rule of server's DirJob.sh.
use std::path::{Component, Path, PathBuf};

/// Fm.ext: the last 12 characters of a name, lower case (a name is short).
pub fn ext(name: &str) -> String {
    let n = name.chars().count();
    name.chars().skip(n.saturating_sub(12)).collect::<String>().to_ascii_lowercase()
}
fn ends(xs: &[&str], e: &str) -> bool { xs.iter().any(|x| e.ends_with(x)) }

/// Fm.kind: "board", "schematic", "3d", "image", "pdf" or "" (listed only).
pub fn kind(name: &str) -> &'static str {
    let e = ext(name);
    if e.ends_with(".kicad_pcb") { "board" } else if e.ends_with(".kicad_sch") { "schematic" }
    else if ends(&[".step", ".stp", ".glb"], &e) { "3d" }
    else if ends(&[".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"], &e) { "image" }
    else if e.ends_with(".pdf") { "pdf" } else { "" }
}
/// Fm.preview: project notes open as text, otherwise the design kind.
pub fn preview(name: &str) -> &'static str {
    if ends(&[".txt", ".md", ".markdown", ".csv", ".json", ".log", ".yaml", ".yml"], &ext(name)) { "text" } else { kind(name) }
}

/// Fm.safe: from the root, no ".." part, never absolute. (NUL is refused as well: no path holds one.)
pub fn safe(rel: &str) -> bool { !rel.starts_with('/') && !rel.contains('\0') && rel.split('/').all(|p| p != "..") }
/// Fm.drive: "C:" at the start.
fn drive(rel: &str) -> bool { rel.chars().nth(1) == Some(':') }
/// Fm.files.safe: Fm.safe, and nothing Windows would read as another place.
pub fn files_safe(rel: &str) -> bool { safe(rel) && !rel.contains('\\') && !drive(rel) }

/// Fm.join / Fm.last / Fm.parent
pub fn join(dir: &str, name: &str) -> String { if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") } }
pub fn last(p: &str) -> &str { p.rsplit('/').next().unwrap_or(p) }
pub fn parent(dir: &str) -> String {
    let cut = dir.len() - last(dir).len() - usize::from(dir.contains('/'));
    dir[..cut].to_string()
}

/// Img.is_image / Img.viewable's path half: absolute or `~/`, with an image extension.
fn abs_or_home(p: &str) -> bool { p.starts_with('/') || p.starts_with("~/") }
pub fn is_image(p: &str) -> bool { abs_or_home(p) && ends(&[".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"], &p.to_lowercase()) }
/// Img.viewable: what /img serves.
pub fn viewable(p: &str) -> bool {
    // since PR #960 a cwd-relative path is no viewer path at all: every kind needs an absolute or ~/ path
    abs_or_home(p) && (is_image(p) || preview(p) == "text" || p.chars().rev().take(4).collect::<String>().to_lowercase() == "fdp.")
}
/// Img.ctype
pub fn ctype(p: &str) -> &'static str {
    let l = p.to_lowercase();
    if preview(&l) == "text" { "text/plain; charset=utf-8" } else if l.ends_with(".pdf") { "application/pdf" } else if l.ends_with(".png") { "image/png" }
    else if l.ends_with(".jpg") || l.ends_with(".jpeg") { "image/jpeg" } else if l.ends_with(".gif") { "image/gif" } else if l.ends_with(".webp") { "image/webp" } else { "image/bmp" }
}

/// Att.from_uri("file://" ++ s): trim, then percent-decode as UTF-8 (a bad escape stays as written, `+` stays).
/// A decoded path is used once; "%2e%2e" is the literal name "..", which `safe` then refuses where it matters.
pub fn from_uri(s: &str) -> String {
    let t = s.trim().as_bytes(); let mut out = Vec::with_capacity(t.len()); let mut i = 0;
    let hex = |b: u8| (b as char).to_digit(16);
    while i < t.len() {
        match (t[i], t.get(i + 1).and_then(|b| hex(*b)), t.get(i + 2).and_then(|b| hex(*b))) {
            (b'%', Some(h), Some(l)) => { out.push((h * 16 + l) as u8); i += 3 }
            _ => { out.push(t[i]); i += 1 }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Resolve `rel` under `root` the way DirJob.sh does (`pwd -P` of both; the real target must be the
/// real root or below it). Symlinks that leave the root, `..`, absolute and NUL paths all yield None.
pub fn contained(root: &Path, rel: &str) -> Option<PathBuf> {
    if !safe(rel) { return None }
    let rr = crate::plat::fs::canonicalize(root).ok()?;
    let target = if rel.is_empty() { rr.clone() } else { crate::plat::fs::canonicalize(rr.join(rel)).ok()? };
    // case "$(pwd -P)/" in "$rr"/*) : the root itself, or a path below it, by whole components
    if target == rr || target.starts_with(&rr) { Some(target) } else { None }
}
/// Only normal components: a belt for callers that join a path they did not canonicalize.
pub fn plain(rel: &str) -> bool { Path::new(rel).components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir)) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plat::fs::symlink;
    // laws fm_kind_board fm_kind_schematic fm_kind_model fm_kind_pdf fm_kind_image fm_kind_other
    #[test] fn fm_kind() {
        for (n, k) in [("a.kicad_pcb","board"),("A.KICAD_PCB","board"),("x.kicad_sch","schematic"),("m.step","3d"),("m.STP","3d"),("m.glb","3d"),("p.PDF","pdf"),("i.png","image"),("i.JpEg","image"),("n.txt",""),("Makefile",""),("",""),("a.kicad_pcb.bak","")] { assert_eq!(kind(n), k, "{n}") }
        assert_eq!(preview("n.md"), "text"); assert_eq!(preview("a.kicad_pcb"), "board"); assert_eq!(preview("x.YAML"), "text");
    }
    // laws fm_safe_up fm_safe_abs fm_safe_down fm_files_safe_*
    #[test] fn fm_safe_rules() {
        for p in ["", "a", "a/b/c.txt", "..a", "a..", "a/..b/c", "a/.../b", ".hidden/x", "a//b"] { assert!(safe(p), "{p}") }
        for p in ["..", "../x", "a/..", "a/../b", "/etc/passwd", "/", "a/b/../../..", "a\0b"] { assert!(!safe(p), "{p}") }
        assert!(!files_safe("a\\b")); assert!(!files_safe("C:x")); assert!(!files_safe("C:\\x")); assert!(files_safe("a/b")); assert!(!files_safe("\\\\host\\s"));
    }
    #[test] fn fm_parent_join() {
        assert_eq!(parent(""), ""); assert_eq!(parent("a"), ""); assert_eq!(parent("a/b"), "a"); assert_eq!(parent("a/b/c"), "a/b");
        assert_eq!(join("", "x"), "x"); assert_eq!(join("a", "x"), "a/x"); assert_eq!(last("a/b"), "b");
    }
    // the corpus: <expected safe 0/1> TAB <decoded rel path as JSON string>; every line is a hostile or odd path
    #[test] fn hostile_corpus() {
        let corpus = include_str!("../../../../test/files/hostile_paths.tsv");
        let mut n = 0;
        for line in corpus.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
            let (want, js) = line.split_once('\t').unwrap();
            let rel: String = serde_json::from_str(js).unwrap_or_else(|_| panic!("bad line {line}"));
            assert_eq!(safe(&rel), want == "1", "{rel:?}"); n += 1;
        }
        assert!(n >= 40, "corpus too small: {n}");
    }
    #[test] fn percent_decoding_is_applied_once() {
        assert_eq!(from_uri("/a%20b"), "/a b"); assert_eq!(from_uri("%2e%2e/x"), "../x"); // decoded once: callers then run `safe`
        assert_eq!(from_uri("%252e%252e"), "%2e%2e"); assert!(safe("%2e%2e/x")); // a double-encoded name is a literal name
        assert_eq!(from_uri("100%"), "100%"); assert_eq!(from_uri("%zz"), "%zz"); assert_eq!(from_uri("a+b"), "a+b"); assert_eq!(from_uri("%E6%97%A5"), "日");
        assert!(!safe(&from_uri("%2e%2e%2fetc"))); assert!(!safe(&from_uri("a%2f..%2fb"))); assert!(!safe(&from_uri("%2fetc")));
    }
    fn tmp(name: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-fm-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    // the DirJob.sh rule: a symlink out of the root, a link chain, and a dotdot through a link are all refused
    #[test] fn containment_refuses_symlink_escapes() {
        let base = tmp("contain"); let root = base.join("root"); let out = base.join("outside");
        std::fs::create_dir_all(root.join("sub/deep")).unwrap(); std::fs::create_dir_all(&out).unwrap(); std::fs::write(out.join("secret"), "s").unwrap();
        symlink(&out, root.join("escape")).unwrap(); symlink("../outside", root.join("sub/rel_escape")).unwrap();
        symlink(root.join("sub"), root.join("inside_link")).unwrap(); symlink(root.join("escape"), root.join("chain")).unwrap();
        symlink("/etc", root.join("etc")).unwrap(); symlink("/", root.join("slash")).unwrap();
        assert!(contained(&root, "").is_some()); assert!(contained(&root, "sub/deep").is_some()); assert!(contained(&root, "inside_link/deep").is_some());
        for bad in ["escape", "escape/secret", "sub/rel_escape", "sub/rel_escape/secret", "chain", "etc", "etc/passwd", "slash", "slash/etc", "..", "sub/../../outside", "/etc", "missing", "sub/deep/../../../outside"] { assert!(contained(&root, bad).is_none(), "{bad}") }
        // a sibling that shares the root's name as a prefix is not inside it
        let sib = base.join("root-evil"); std::fs::create_dir_all(&sib).unwrap(); symlink(&sib, root.join("prefix")).unwrap(); assert!(contained(&root, "prefix").is_none());
        let _ = std::fs::remove_dir_all(&base);
    }
    #[test] fn viewable_and_ctype() {
        assert!(viewable("/x/a.PNG")); assert!(viewable("~/a.jpg")); assert!(!viewable("rel/note.md")); assert!(viewable("/x/note.md")); assert!(viewable("/a/b.pdf")); assert!(viewable("~/a.PDF"));
        assert!(!viewable("/etc/passwd")); assert!(!viewable("rel/a.pdf")); assert!(!viewable("/a/b.png.exe")); assert!(!viewable("/a/key.pem")); assert!(!viewable(""));
        assert_eq!(ctype("/a.PNG"), "image/png"); assert_eq!(ctype("/a.md"), "text/plain; charset=utf-8"); assert_eq!(ctype("/a.pdf"), "application/pdf"); assert_eq!(ctype("/a.bmp"), "image/bmp");
    }
}

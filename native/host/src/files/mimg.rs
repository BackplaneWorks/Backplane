//! Images in messages (hub-served local paths, #798): GET /message-image?entry=<message id>&path=<image path>. The hub keeps a private
//! snapshot of each image an assistant message names, taken when the message is committed (server/message-image,
//! effects/message_image.js, core/images): `<home>/message-images/<sha256>.<ext>` plus a ref `<sha256(entry NUL path)>.json`
//! naming it. The route serves only a snapshot a ref names (the message's own text granted the path); the credential must be the
//! hub's own (Conn.allowed AND Auth.credential), not a browser's cross-site request. The snapshot code is hub's, line for line:
//! raster signatures only (no SVG/HTML renamed to .png), 24 MiB encoded, 32 megapixels decoded, no symlink anywhere on the source path,
//! the file read once with O_NOFOLLOW and checked unchanged. Older messages that no ref names answer 404 (hub looks their text up in
//! store.db; this hub keeps no entry-id index) - stated limitation.
use sha2::{Digest, Sha256};
use std::{io::Read, path::{Path, PathBuf}}; use crate::plat::fs::{MetadataExt, OpenOptionsExt};

pub const MI_BYTES: u64 = 24 * 1024 * 1024;
pub const MI_PIXELS: u64 = 32 * 1024 * 1024;
const EXTS: [&str; 6] = [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"];

// ---- core/images: which paths a message names
fn sep(c: char) -> bool { matches!(c, ' ' | '\n' | '\t' | '\r' | '(' | ')' | '[' | ']' | '<' | '>' | '`' | '"' | '\'' | ',' | ';' | '|') }
fn abs_or_home(p: &str) -> bool { crate::plat::fs::is_abs(p) || p.starts_with("~/") }
/// Img.is_image
pub fn is_image(p: &str) -> bool { abs_or_home(p) && { let l = p.to_lowercase(); EXTS.iter().any(|e| l.ends_with(e)) } }
/// Img.bare: "file://" dropped, one trailing . : ! ? dropped.
fn bare(w: &str) -> String { let w = w.strip_prefix("file://").unwrap_or(w); match w.chars().last() { Some('.' | ':' | '!' | '?') => w[..w.len() - 1].to_string(), _ => w.to_string() } }
/// Img.may: an image extension (any case) follows some dot.
pub fn may(s: &str) -> bool {
    let b = s.as_bytes();
    (0..b.len()).filter(|i| b[*i] == b'.').any(|i| { let t: String = s[i + 1..].chars().take(4).collect::<String>().to_lowercase(); EXTS.iter().any(|e| format!(".{t}").starts_with(e)) })
}
/// Img.paths: the images a message shows, in order, each once. A word longer than 1024 characters is no path.
pub fn paths(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for w in text.split(sep).filter(|w| { let n = w.chars().count(); n > 0 && n <= 1024 }) { let p = bare(w); if is_image(&p) && !out.contains(&p) { out.push(p) } }
    out
}
/// Img.message_allowed: both the saved message and the exact spelling authorize the path.
/// A path that does not climb: no `.` or `..` part under this system's separators (Windows: `\\` and `/`), and on Unix no `\\` at all.
pub fn no_climb(p: &str) -> bool { !(cfg!(unix) && p.contains('\\')) && !p.split(crate::plat::fs::SEPS).any(|x| x == "." || x == "..") }
pub fn message_allowed(text: &str, path: &str) -> bool {
    is_image(path) && no_climb(path) && paths(text).iter().any(|p| p == path)
}

// ---- effects/message_image.js
fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }
fn sha_hex(v: &[u8]) -> String { hex(&Sha256::digest(v)) }
/// mi_type: a raster signature and its dimensions; Err for anything else or too big.
pub fn image_type(b: &[u8]) -> Result<&'static str, &'static str> {
    let (mut ext, mut w, mut h) = ("", 0u64, 0u64);
    let le = |i: usize, n: usize| (0..n).fold(0u64, |a, k| a | (b[i + k] as u64) << (8 * k));
    let be = |i: usize, n: usize| (0..n).fold(0u64, |a, k| a << 8 | b[i + k] as u64);
    if b.len() >= 24 && b[..8] == [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] && &b[12..16] == b"IHDR" { ext = "png"; w = be(16, 4); h = be(20, 4) }
    else if b.len() >= 10 && (&b[..6] == b"GIF87a" || &b[..6] == b"GIF89a") { ext = "gif"; w = le(6, 2); h = le(8, 2) }
    else if b.len() >= 26 && &b[..2] == b"BM" && le(14, 4) >= 40 { ext = "bmp"; let wi = le(18, 4) as u32 as i32; let hi = le(22, 4) as u32 as i32; w = if wi > 0 { wi as u64 } else { 0 }; h = hi.unsigned_abs() as u64 }
    else if b.len() >= 30 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        ext = "webp";
        match &b[12..16] {
            b"VP8X" => { w = le(24, 3) + 1; h = le(27, 3) + 1 }
            b"VP8L" if b[20] == 0x2f => { let n = le(21, 4); w = (n & 0x3fff) + 1; h = ((n >> 14) & 0x3fff) + 1 }
            b"VP8 " if b[23..26] == [0x9d, 1, 0x2a] => { w = le(26, 2) & 0x3fff; h = le(28, 2) & 0x3fff }
            _ => {}
        }
    } else if b.len() >= 4 && b[0] == 0xff && b[1] == 0xd8 {
        ext = "jpg"; let mut i = 2usize;
        while i + 4 <= b.len() {
            if b[i] != 0xff { break } i += 1;
            while i < b.len() && b[i] == 0xff { i += 1 }
            if i >= b.len() { break }
            let marker = b[i]; i += 1;
            if marker == 0xda || marker == 0xd9 { break }
            if marker == 0x01 || (0xd0..=0xd7).contains(&marker) { continue }
            if i + 2 > b.len() { break }
            let len = be(i, 2) as usize;
            if len < 2 || i + len > b.len() { break }
            if [0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf].contains(&marker) && len >= 8 { h = be(i + 3, 2); w = be(i + 5, 2); break }
            i += len;
        }
    }
    if ext.is_empty() || !(w > 0 && h > 0 && w.saturating_mul(h) <= MI_PIXELS) { return Err("Unsupported or oversized image") }
    Ok(ext)
}
/// mi_regular + mi_read: a regular file (not a link), 1..=24 MiB, opened without following, unchanged while read.
pub fn read_image(file: &Path) -> Result<Vec<u8>, &'static str> {
    let before = std::fs::symlink_metadata(file).map_err(|_| "Image unavailable")?;
    if !before.is_file() || before.file_type().is_symlink() || before.len() == 0 || before.len() > MI_BYTES { return Err("Image unavailable") }
    let mut f = std::fs::OpenOptions::new().read(true).custom_flags(crate::plat::fs::O_NONBLOCK | crate::plat::fs::O_NOFOLLOW).open(file).map_err(|_| "Image unavailable")?;
    let st = f.metadata().map_err(|_| "Image unavailable")?;
    if !st.is_file() || st.dev() != before.dev() || st.ino() != before.ino() || st.len() != before.len() { return Err("Image changed") }
    let mut b = Vec::with_capacity(st.len() as usize);
    f.by_ref().take(st.len() + 1).read_to_end(&mut b).map_err(|_| "Image truncated")?;
    if b.len() as u64 != st.len() { return Err("Image truncated") }
    let after = f.metadata().map_err(|_| "Image changed")?;
    if after.len() != st.len() || after.mtime() != st.mtime() || after.mtime_nsec() != st.mtime_nsec() || after.ctime() != st.ctime() || after.ctime_nsec() != st.ctime_nsec() { return Err("Image changed") }
    Ok(b)
}
/// mi_dir: <home>/message-images, private (0700, ours).
fn snapshot_dir(home: &Path) -> Result<PathBuf, &'static str> {
    let dir = home.join("message-images");
    std::fs::create_dir_all(&dir).map_err(|_| "Private image directory unavailable")?;
    { use crate::plat::fs::PermissionsExt; let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)); } // mkdirSync(mode) + umask: made 0700 here
    let st = std::fs::symlink_metadata(&dir).map_err(|_| "Private image directory unavailable")?;
    if !st.is_dir() || st.file_type().is_symlink() || (st.mode() & 0o077) != 0 || st.uid() != crate::plat::fs::uid() { return Err("Private image directory unavailable") }
    Ok(dir)
}
/// mi_source: the source path to read: absolute or ~/, an image extension, no "." or ".." part, no NUL CR LF or backslash, and no symlink anywhere on it.
pub fn source(src: &str) -> Result<PathBuf, &'static str> {
    // (Windows: a backslash is a separator there, not refused)
    if src.len() > 1024 || src.contains(['\0', '\r', '\n']) || (cfg!(unix) && src.contains('\\')) || src.split(crate::plat::fs::SEPS).any(|x| x == "." || x == "..") { return Err("Invalid image path") }
    let file = match src.strip_prefix("~/") { Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest), None => PathBuf::from(src) };
    let l = file.to_string_lossy().to_lowercase();
    if !file.is_absolute() || !(l.ends_with(".png") || l.ends_with(".jpg") || l.ends_with(".jpeg") || l.ends_with(".gif") || l.ends_with(".webp") || l.ends_with(".bmp")) { return Err("Invalid image path") }
    if !crate::plat::fs::no_link_on(&file).map_err(|_| "Invalid image path")? { return Err("Image symlink") }
    Ok(file)
}
fn valid_name(n: &str) -> bool { n.len() > 65 && n.as_bytes()[64] == b'.' && n[..64].bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) && matches!(&n[65..], "png" | "jpg" | "gif" | "webp" | "bmp") }
pub fn ref_name(entry: &str, source: &str) -> String { format!("{}.json", sha_hex(format!("{entry}\0{source}").as_bytes())) }
/// What a ref says besides the image: the thread of the message.
pub struct Snap { pub path: PathBuf, pub thread: Option<String> }
/// The ref half of mi_snapshot: the snapshot an existing ref names, checked against its own hash; never makes one.
pub fn lookup(home: &Path, entry: &str, src: &str) -> Result<Snap, &'static str> {
    if entry.is_empty() || entry.len() > 1024 { return Err("") }
    let dir = snapshot_dir(home)?; let rf = dir.join(ref_name(entry, src));
    let st = std::fs::symlink_metadata(&rf).map_err(|_| "")?;
    if !st.is_file() || st.file_type().is_symlink() || st.len() > 256 { return Err("") }
    let j: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&rf).map_err(|_| "")?).map_err(|_| "")?;
    let name = j.get("image").and_then(|v| v.as_str()).unwrap_or("");
    if !valid_name(name) { return Err("") }
    let file = dir.join(name); let b = read_image(&file)?; let ext = image_type(&b)?;
    if name == format!("{}.{ext}", sha_hex(&b)) { Ok(Snap { path: file, thread: j.get("thread").and_then(|v| v.as_str()).map(str::to_string) }) } else { Err("") }
}
/// Write the thread into a ref hub made (it has none), so the deleted-thread check can apply to it from now on.
pub fn set_thread(home: &Path, entry: &str, src: &str, thread: &str) -> bool {
    let Ok(dir) = snapshot_dir(home) else { return false };
    let rf = dir.join(ref_name(entry, src));
    let Ok(text) = std::fs::read_to_string(&rf) else { return false };
    let Ok(mut j) = serde_json::from_str::<serde_json::Value>(&text) else { return false };
    if j.get("thread").is_some() { return true }
    j["thread"] = serde_json::json!(thread);
    let tmp = rf.with_extension("json.thread");
    let ok = (|| { use std::io::Write; let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?; f.write_all(j.to_string().as_bytes())?; crate::plat::fs::rename(&tmp, &rf) })();
    let _ = std::fs::remove_file(&tmp); ok.is_ok()
}
/// mi_snapshot: the snapshot file for (entry, source), made if it is not there yet; Err when it cannot be had.
pub fn snapshot(home: &Path, entry: &str, src: &str, thread: &str) -> Result<Snap, &'static str> {
    if entry.is_empty() || entry.len() > 1024 { return Err("") }
    let dir = snapshot_dir(home)?; let rf = dir.join(ref_name(entry, src));
    if rf.exists() { return lookup(home, entry, src) }
    let file = source(src)?; let b = read_image(&file)?;
    if source(src)? != file { return Err("") }
    let ext = image_type(&b)?; let name = format!("{}.{ext}", sha_hex(&b)); let out = dir.join(&name);
    match std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&out) {
        Ok(mut f) => { use std::io::Write; f.write_all(&b).map_err(|_| "")? }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => { if read_image(&out)? != b { return Err("Image snapshot changed") } }
        Err(_) => return Err(""),
    }
    let tmp = rf.with_extension(format!("json.{}", &sha_hex(&b)[..16]));
    let body = serde_json::json!({"image": name, "thread": thread}).to_string();
    let r = (|| { use std::io::Write; let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?; f.write_all(body.as_bytes())?; crate::plat::fs::rename(&tmp, &rf) })();
    let _ = std::fs::remove_file(&tmp);
    r.map_err(|_| "")?;
    Ok(Snap { path: out, thread: Some(thread.to_string()) })
}

#[cfg(test)]
mod tests {
    use super::*;
    // canonical: `source()` refuses a path with a link anywhere on it, so a TMPDIR that is itself a link (h@backplane, macOS /var) must be resolved first
    fn tmp(n: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-mi-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); crate::plat::fs::canonicalize(&p).unwrap() }
    fn png(w: u32, h: u32) -> Vec<u8> { let mut v = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R']; v.extend(w.to_be_bytes()); v.extend(h.to_be_bytes()); v.extend([8, 6, 0, 0, 0]); v.extend([0u8; 16]); v }
    // images laws: Img.paths finds each image path once, in order; words over 1024 chars are no path
    #[test] fn img_paths() {
        assert_eq!(paths("see /a/b.png, and (~/c/d.JPG). Also file:///e/f.gif! and /a/b.png again <x> `/g.webp`: /h.bmp?"), ["/a/b.png", "~/c/d.JPG", "/e/f.gif", "/g.webp", "/h.bmp"]);
        assert!(paths("rel/x.png /x.txt http://x/y.png").is_empty());
        assert!(paths(&format!("/{}.png", "a".repeat(1100))).is_empty()); assert!(may("a .PNG b") && may("x.jpeg") && !may("x.txt png") && !may(""));
        assert!(message_allowed("see /a/b.png", "/a/b.png")); for (t, p) in [("see /a/b.png", "/a/c.png"), ("/a/../b.png", "/a/../b.png"), ("/a/./b.png", "/a/./b.png"), ("x", "/a/b.png"), ("/a/b.txt", "/a/b.txt")] { assert!(!message_allowed(t, p), "{p}") }
    }
    #[cfg(unix)] #[test] fn a_backslash_is_refused_on_unix() { assert!(!message_allowed("/a/b\\.png", "/a/b\\.png")) }
    /// Windows: an image an agent names as C:\\x\\y.png (the hub's own attachments are written so) is allowed, a climb under either separator is not.
    #[cfg(windows)] #[test] fn a_windows_path_is_allowed_and_a_climb_is_not() {
        let p = r"C:\x\home\attachments\t1\a-pasted-1.png";
        assert_eq!(paths(&format!("echo: look [attachment: {p}]")), [p]);
        assert!(message_allowed(&format!("[attachment: {p}]"), p));
        for q in [r"C:\x\..\y.png", r"C:\x\.\y.png", r"C:/x/../y.png"] { assert!(!message_allowed(&format!("see {q}"), q), "{q}") }
    }
    #[test] fn image_types_and_bounds() {
        assert_eq!(image_type(&png(10, 10)), Ok("png")); assert!(image_type(&png(10_000, 10_000)).is_err()); assert!(image_type(&png(0, 5)).is_err());
        assert!(image_type(b"<svg xmlns='http://www.w3.org/2000/svg'></svg>").is_err()); assert!(image_type(b"").is_err()); assert!(image_type(&[0xff, 0xd8, 0xff]).is_err());
        let mut gif = b"GIF89a".to_vec(); gif.extend([4, 0, 3, 0, 0, 0]); assert_eq!(image_type(&gif), Ok("gif"));
        let jpg = [0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, 0, 20, 0, 30, 3, 1, 0x22, 0, 2, 0x11, 0, 3, 0x11, 0]; assert_eq!(image_type(&jpg), Ok("jpg"));
    }
    #[test] fn a_ref_without_a_thread_gets_one() {
        let home = tmp("sth"); let src = tmp("sths"); let f = src.join("p.png"); std::fs::write(&f, png(8, 8)).unwrap(); let fs = f.to_string_lossy().into_owned();
        let s = snapshot(&home, "m1", &fs, "t1").unwrap(); let name = s.path.file_name().unwrap().to_string_lossy().into_owned();
        // a ref as hub writes it: the image only
        let rf = home.join("message-images").join(ref_name("m1", &fs)); std::fs::write(&rf, format!("{{\"image\":\"{name}\"}}")).unwrap();
        assert!(lookup(&home, "m1", &fs).unwrap().thread.is_none());
        assert!(set_thread(&home, "m1", &fs, "t9")); assert_eq!(lookup(&home, "m1", &fs).unwrap().thread.as_deref(), Some("t9"));
        assert!(!set_thread(&home, "nope", &fs, "t9"));
    }
    #[test] fn snapshot_refs_and_refusals() {
        let home = tmp("home"); let src = tmp("src"); let f = src.join("p.png"); std::fs::write(&f, png(8, 8)).unwrap();
        let fs = f.to_string_lossy().into_owned();
        let s = snapshot(&home, "m1", &fs, "t1").unwrap(); assert!(s.path.starts_with(home.join("message-images")) && s.thread.as_deref() == Some("t1"));
        assert_eq!(std::fs::read(&s.path).unwrap(), png(8, 8)); assert!(s.path.file_name().unwrap().to_string_lossy().ends_with(".png"));
        // the file may change or go away afterwards: the snapshot answers
        std::fs::remove_file(&f).unwrap(); let again = snapshot(&home, "m1", &fs, "t1").unwrap(); assert_eq!(again.path, s.path);
        // a different message naming it has no ref and the source is gone
        assert!(snapshot(&home, "m2", &fs, "t1").is_err());
        // refusals: a link anywhere on the path, a non-raster, a relative path, "..", a big file
        let real = src.join("real"); std::fs::create_dir_all(&real).unwrap(); std::fs::write(real.join("a.png"), png(4, 4)).unwrap(); crate::plat::fs::symlink(&real, src.join("ln")).unwrap();
        crate::plat::fs::symlink(real.join("a.png"), src.join("l.png")).unwrap(); std::fs::write(src.join("x.png"), b"<svg/>").unwrap(); std::fs::write(src.join("big.png"), vec![0u8; 25 << 20]).unwrap();
        for bad in [src.join("ln/a.png"), src.join("l.png"), src.join("x.png"), src.join("big.png"), src.join("none.png"), PathBuf::from("rel/a.png"), src.join("real/../real/a.png")] { assert!(snapshot(&home, "m3", &bad.to_string_lossy(), "t").is_err(), "{bad:?}") }
        assert!(snapshot(&home, "m3", &real.join("a.png").to_string_lossy(), "t").is_ok());
        // a ref pointing at something that is not its own hash is refused; ref names are hub's hash of entry NUL path
        assert_eq!(ref_name("e", "/p.png").len(), 64 + 5); assert_eq!(ref_name("e", "/p.png"), format!("{}.json", sha_hex(b"e\0/p.png")));
        // m1's own snapshot (not "the first .png in the folder": read_dir order differs between hosts) is replaced by another image: its hash no longer fits its name
        std::fs::write(&s.path, png(9, 9)).unwrap(); assert!(snapshot(&home, "m1", &fs, "t1").is_err());
    }
}

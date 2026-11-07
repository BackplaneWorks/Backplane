//! POST /attach?thread=<id>&key=<key>&name=<name>: the phones' way to attach a file.
//! The body goes straight from the socket to `<home>/attachments/<thread>/<key>-<name>`; the hub holds a buffer, never the file.
//! Answer: `{"attached": path, "thread"}`, or on failure `{"text", "thread", "upload": true, "uiError", "uiDetail"}` (Notice.upload.reply).
//! Bounds: the file may not pass the `files.max.mb` setting (Att.max: 512 MB default, 1..4000) nor the transport's own limit
//!.
//! Thread-folder rule (PR #960 spirit): an unknown, empty or deleted thread gets no attachment folder.
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

pub const KEY: &str = "files.max.mb";
pub const MB_DEFAULT: u64 = 512;
pub const MB_CEILING: u64 = 4000;

/// Att.ok: a file name char that can never leave its directory.
fn ok(c: char) -> bool { c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' }
/// Att.clean: every other char becomes '_'.
pub fn clean(s: &str) -> String { s.chars().map(|c| if ok(c) { c } else { '_' }).collect() }
/// Att.safe: cleaned, at most 100 chars, never empty, never hidden.
pub fn safe(s: &str) -> String {
    let t: String = clean(s).chars().take(100).collect();
    if t.is_empty() { "file".into() } else if t.starts_with('.') { format!("_{}", &t[1..]) } else { t }
}
/// Att.stored: the stored file name of an upload.
pub fn stored(key: &str, name: &str) -> String { format!("{}-{}", safe(key), safe(name)) }
/// Att.dir
pub fn dir(home: &std::path::Path, thread: &str) -> std::path::PathBuf { home.join("attachments").join(safe(thread)) }
/// Att.mb: the limit in MB for the setting's text ("" or anything not a count: the default; 0 -> 1; over the ceiling -> the ceiling).
pub fn mb(v: &str) -> u64 {
    let t = v.trim();
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) { return MB_DEFAULT }
    match t.parse::<u64>() { Ok(0) => 1, Ok(n) => n.min(MB_CEILING), Err(_) => MB_CEILING }
}
pub fn max_bytes(v: &str) -> u64 { mb(v) * 1048576 }

/// Notice.upload.reply: the failure body with its English words (clients read uiError/uiDetail).
pub fn failure(thread: &str, id: &str, detail: &str) -> Value {
    let text = match id { "upload.network" => "the upload did not reach the hub".to_string(), "upload.failed" => "the upload failed".into(), "upload.incomplete" => "the upload did not arrive whole".into(),
        "upload.cap" => format!("attachments are limited to {detail} MB"), _ => detail.to_string() };
    json!({"text":text,"thread":thread,"upload":true,"uiError":id,"uiDetail":detail})
}
/// Upload.json
pub fn attached(thread: &str, path: &str) -> Value { json!({"attached":path,"thread":thread}) }
pub fn failed_plain(thread: &str, why: &str) -> Value { json!({"text":why,"thread":thread,"upload":true}) }

/// Receive `len` bytes from `body` into the thread's attachment file. Returns (status, JSON).
/// `lim` is the setting's text (files.max.mb); `transport_max` the transport's own limit.
pub async fn receive<R: AsyncRead + Unpin>(home: &std::path::Path, thread: &str, key: &str, name: &str, len: u64, lim: &str, transport_max: u64, body: &mut R) -> (&'static str, Value) {
    let cap = max_bytes(lim).min(transport_max);
    if len > cap { return ("413 Payload Too Large", failure(thread, "upload.cap", &(cap / 1048576).to_string())) }
    let d = dir(home, thread);
    if let Err(e) = tokio::fs::create_dir_all(&d).await { return ("500 Internal Server Error", failed_plain(thread, &e.to_string())) }
    let path = d.join(stored(key, name));
    // O_NOFOLLOW: a link at the target is never followed, the open fails (500) and nothing is written; a plain file there is truncated and replaced
    let mut f = match tokio::fs::OpenOptions::new().write(true).create(true).truncate(true).custom_flags(crate::plat::fs::O_NOFOLLOW as _).open(&path).await {
        Ok(f) => f, Err(e) => return ("500 Internal Server Error", failed_plain(thread, &e.to_string())) };
    let p = path.to_string_lossy().into_owned();
    if len == 0 { return ("200 OK", attached(thread, &p)) }
    let mut got = 0u64; let mut buf = vec![0u8; 64 * 1024];
    while got < len {
        let want = ((len - got) as usize).min(buf.len());
        match body.read(&mut buf[..want]).await {
            Ok(0) | Err(_) => { drop(f); let _ = tokio::fs::remove_file(&path).await; return ("400 Bad Request", failure(thread, "upload.incomplete", "")) }
            Ok(n) => { if let Err(e) = f.write_all(&buf[..n]).await { drop(f); let _ = tokio::fs::remove_file(&path).await; return ("500 Internal Server Error", failed_plain(thread, &e.to_string())) } got += n as u64 }
        }
    }
    let _ = f.flush().await;
    ("200 OK", attached(thread, &p))
}
#[cfg(test)]
mod tests {
    use super::*;
    // laws attach_clean_ok, att_max_*: names never leave their folder; the limit is clamped
    #[test] fn att_names_and_limits() {
        for (i, o) in [("", "file"), (".hidden", "_hidden"), ("a/b", "a_b"), ("../x", "_._x"), ("..", "_."), (".", "_"), ("日本.png", "__.png"), ("a b\n.txt", "a_b_.txt")] { assert_eq!(safe(i), o, "{i:?}") }
        assert_eq!(safe(&"x".repeat(300)).len(), 100); assert_eq!(stored("k1", "p.png"), "k1-p.png"); assert_eq!(stored("", ""), "file-file");
        for t in ["../../etc", "/etc", "a/../b", ""] { let d = dir(std::path::Path::new("/h"), t); assert!(d.starts_with("/h/attachments") && d.components().count() == 4, "{d:?}") }
        assert_eq!((mb(""), mb("abc"), mb("0"), mb("1"), mb("128"), mb("4000"), mb("99999"), mb(" 64 "), mb("-5"), mb("1.5")), (512, 512, 1, 1, 128, 4000, 4000, 64, 512, 512));
    }
    #[test] fn failures_carry_the_words_and_ids() {
        let f = failure("t1", "upload.cap", "128"); assert_eq!(f["text"], "attachments are limited to 128 MB"); assert_eq!(f["uiError"], "upload.cap"); assert_eq!(f["uiDetail"], "128"); assert_eq!(f["upload"], true);
        assert_eq!(failure("t1", "upload.incomplete", "")["text"], "the upload did not arrive whole");
        assert_eq!(serde_json::to_string(&failure("t", "upload.incomplete", "")).unwrap(), r#"{"text":"the upload did not arrive whole","thread":"t","upload":true,"uiError":"upload.incomplete","uiDetail":""}"#);
    }
    fn tmp(n: &str) -> std::path::PathBuf { let p = std::env::temp_dir().join(format!("bp-att-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    #[tokio::test] async fn receives_whole_files_and_refuses_short_big_and_linked_ones() {
        let h = tmp("h");
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let (st, j) = receive(&h, "t1", "k", "a.bin", data.len() as u64, "", 1 << 28, &mut &data[..]).await;
        assert_eq!(st, "200 OK"); let p = j["attached"].as_str().unwrap(); assert!(p.replace('\\', "/").ends_with("/attachments/t1/k-a.bin")); assert_eq!(std::fs::read(p).unwrap(), data);
        // the body ends before its length: refused, nothing left behind
        let short = &data[..1000]; let (st, j) = receive(&h, "t1", "k", "short.bin", 5000, "", 1 << 28, &mut &short[..]).await;
        assert_eq!(st, "400 Bad Request"); assert_eq!(j["uiError"], "upload.incomplete"); assert!(!h.join("attachments/t1/k-short.bin").exists());
        // over the setting's limit, over the transport's: 413 before a byte is read
        let (st, j) = receive(&h, "t1", "k", "big", 3 * 1048576, "2", 1 << 28, &mut &[][..]).await; assert_eq!((st, j["uiDetail"].as_str().unwrap()), ("413 Payload Too Large", "2"));
        let (st, j) = receive(&h, "t1", "k", "big2", 300 << 20, "4000", 256 << 20, &mut &[][..]).await; assert_eq!((st, j["uiDetail"].as_str().unwrap()), ("413 Payload Too Large", "256"));
        assert!(!h.join("attachments/t1/k-big").exists());
        // an empty file is a file; a symlink at the target is not followed
        let (st, _) = receive(&h, "t1", "k", "empty", 0, "", 1 << 28, &mut &[][..]).await; assert_eq!(st, "200 OK"); assert!(h.join("attachments/t1/k-empty").exists());
        std::fs::write(h.join("victim"), "keep").unwrap(); crate::plat::fs::symlink(h.join("victim"), h.join("attachments/t1/k-link")).unwrap();
        let (st, _) = receive(&h, "t1", "k", "link", 3, "", 1 << 28, &mut &b"abc"[..]).await; assert_eq!(st, "500 Internal Server Error"); assert_eq!(std::fs::read_to_string(h.join("victim")).unwrap(), "keep");
        // a hostile thread id and name stay inside <home>/attachments
        let (st, j) = receive(&h, "../../etc", "../k", "../../passwd", 2, "", 1 << 28, &mut &b"hi"[..]).await; assert_eq!(st, "200 OK");
        let p = std::path::PathBuf::from(j["attached"].as_str().unwrap()); assert!(p.starts_with(h.join("attachments")), "{p:?}");
    }
}

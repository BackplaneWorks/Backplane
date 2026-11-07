//! /img: the preview send of effects/host.js (`sock_send_preview`) as an HttpReply: open without
//! following a final symlink and without blocking, regular files only, one `bytes=a-b` range
//! (malformed or multipart ranges are ignored), 416 with `Content-Range: bytes */size`.
//! LIMIT: HttpReply holds the body in memory, so one reply carries at most `PIECE` bytes; a longer answer
//! is a 206 of the first piece with its Content-Range (valid HTTP), until the API streams bodies.
use crate::api::HttpReply;
use std::io::{Read, Seek, SeekFrom};
use crate::plat::fs::OpenOptionsExt;

pub const PIECE: u64 = 32 << 20;

/// The parsed `Range` of the JS hub: Some((first, last)) for a satisfiable single range, Err for 416, None to ignore.
pub fn range(h: &str, size: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(r) = h.strip_prefix("bytes=") else { return Ok(None) };
    let Some((a, b)) = r.split_once('-') else { return Ok(None) };
    if !a.bytes().all(|c| c.is_ascii_digit()) || !b.bytes().all(|c| c.is_ascii_digit()) || (a.is_empty() && b.is_empty()) { return Ok(None) }
    let (first, last) = (a.parse::<u64>().ok(), b.parse::<u64>().ok());
    if (a.len() > 0 && first.is_none()) || (b.len() > 0 && last.is_none()) { return Ok(None) }
    if size == 0 || match first { None => last == Some(0), Some(f) => f >= size || last.map_or(false, |l| l < f) } { return Err(()) }
    let start = match first { Some(f) => f, None => { let l = last.unwrap(); if l >= size { 0 } else { size - l } } };
    let end = match (first, last) { (Some(_), Some(l)) if l < size => l, _ => size - 1 };
    Ok(Some((start, end)))
}

fn head(status: &'static str, ctype: &str, extra: String, body: Vec<u8>) -> HttpReply {
    HttpReply { status, ctype: ctype.into(), extra: format!("Cache-Control: private, no-cache\r\nAccept-Ranges: bytes\r\nX-Content-Type-Options: nosniff\r\n{extra}"), body }
}
fn fail(status: &'static str, msg: &str, extra: &str) -> HttpReply { head(status, "text/plain; charset=utf-8", extra.into(), msg.as_bytes().to_vec()) }

/// `path` is already decoded, viewable-checked and `~/`-expanded.
pub fn send(path: &str, ctype: &str, range_header: &str) -> HttpReply {
    if path.contains('\0') || ctype.contains(['\r', '\n']) { return fail("400 Bad Request", "invalid preview request", "") }
    let f = std::fs::OpenOptions::new().read(true).custom_flags(crate::plat::fs::O_NONBLOCK | crate::plat::fs::O_NOFOLLOW).open(path);
    let mut f = match f { Ok(f) => f, Err(e) => return match e.kind() {
        std::io::ErrorKind::NotFound => fail("404 Not Found", "not found", ""),
        std::io::ErrorKind::PermissionDenied if cfg!(windows) && std::path::Path::new(path).is_dir() => fail("404 Not Found", "not a regular file", ""), // Windows opens no folder as a file
        std::io::ErrorKind::PermissionDenied => fail("403 Forbidden", "cannot read preview", ""),
        _ if e.raw_os_error() == Some(crate::plat::errno::ENOTDIR) => fail("404 Not Found", "not found", ""),
        _ => fail("500 Internal Server Error", "cannot read preview", "") } };
    let Ok(md) = f.metadata() else { return fail("500 Internal Server Error", "cannot read preview", "") };
    if !md.is_file() { return fail("404 Not Found", "not a regular file", "") }
    let size = md.len();
    let (mut status, mut extra, mut start, mut end) = ("200 OK", String::new(), 0u64, size.saturating_sub(1));
    match range(range_header, size) {
        Err(()) => return fail("416 Range Not Satisfiable", "range not satisfiable", &format!("Content-Range: bytes */{size}\r\n")),
        Ok(Some((a, b))) => { start = a; end = b; status = "206 Partial Content" }
        Ok(None) => {}
    }
    if size > 0 && end - start + 1 > PIECE { end = start + PIECE - 1; status = "206 Partial Content" }
    if status.starts_with("206") { extra = format!("Content-Range: bytes {start}-{end}/{size}\r\n") }
    let len = if size == 0 { 0 } else { end - start + 1 };
    let mut body = vec![0u8; len as usize];
    if f.seek(SeekFrom::Start(start)).is_err() || f.read_exact(&mut body).is_err() { return fail("500 Internal Server Error", "cannot read preview", "") }
    head(status, ctype, extra, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    // the range cases of effects/host.js, one by one
    #[test] fn ranges_like_the_js_hub() {
        assert_eq!(range("", 10), Ok(None)); assert_eq!(range("bytes=0-3", 10), Ok(Some((0, 3)))); assert_eq!(range("bytes=2-", 10), Ok(Some((2, 9))));
        assert_eq!(range("bytes=-4", 10), Ok(Some((6, 9)))); assert_eq!(range("bytes=-40", 10), Ok(Some((0, 9)))); assert_eq!(range("bytes=5-99", 10), Ok(Some((5, 9))));
        assert_eq!(range("bytes=10-", 10), Err(())); assert_eq!(range("bytes=5-2", 10), Err(())); assert_eq!(range("bytes=-0", 10), Err(())); assert_eq!(range("bytes=0-0", 0), Err(()));
        for ignored in ["bytes=-", "bytes=a-b", "bytes=0-1,3-4", "items=0-1", "bytes=0", "bytes= 0-1"] { assert_eq!(range(ignored, 10), Ok(None), "{ignored}") }
    }
    fn tmp(n: &str) -> std::path::PathBuf { let p = std::env::temp_dir().join(format!("bp-srv-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    #[test] fn serves_ranges_and_refuses_non_files_and_links() {
        let d = tmp("s"); let f = d.join("a.png"); std::fs::write(&f, b"0123456789").unwrap();
        let r = send(f.to_str().unwrap(), "image/png", ""); assert_eq!((r.status, r.body.as_slice()), ("200 OK", &b"0123456789"[..]));
        let r = send(f.to_str().unwrap(), "image/png", "bytes=2-4"); assert_eq!((r.status, r.body.as_slice()), ("206 Partial Content", &b"234"[..])); assert!(r.extra.contains("Content-Range: bytes 2-4/10\r\n"));
        let r = send(f.to_str().unwrap(), "image/png", "bytes=20-"); assert_eq!(r.status, "416 Range Not Satisfiable"); assert!(r.extra.contains("bytes */10"));
        assert_eq!(send(d.to_str().unwrap(), "image/png", "").status, "404 Not Found");
        crate::plat::fs::symlink(&f, d.join("l.png")).unwrap(); assert_eq!(send(d.join("l.png").to_str().unwrap(), "image/png", "").status, "500 Internal Server Error"); // O_NOFOLLOW: ELOOP, as the JS hub
        assert_eq!(send(d.join("none.png").to_str().unwrap(), "image/png", "").status, "404 Not Found");
        assert_eq!(send("/etc/pass\0wd", "image/png", "").status, "400 Bad Request"); assert_eq!(send("/dev/null", "image/png", "").status, "404 Not Found");
        let _ = std::fs::remove_dir_all(&d);
    }
}

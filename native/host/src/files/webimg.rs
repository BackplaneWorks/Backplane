//! GET /webimg?url=: a picture a document shows from the web (a README's badges and screenshots), fetched by the hub and kept on its disk, so
//! every client draws it from the hub as it draws a file's (`/img`) and the web is asked once a day, not on every page. Only pictures come back
//! (PNG, JPEG, GIF, WebP, SVG by their first bytes); an address on this machine or its network is refused, as is anything but http(s).
use crate::{api::{Fut, HttpHandler, HttpReply}, auth::Req, hub::Hub};
use sha2::{Digest, Sha256};
use std::{path::{Path, PathBuf}, sync::Arc, time::{Duration, SystemTime}};

pub const MAX_BYTES: u64 = 16 << 20;
/// A kept picture is fresh this long; after it, it is fetched again and the old copy served if the web does not answer.
const FRESH: Duration = Duration::from_secs(24 * 3600);
/// A failed fetch is not tried again for this long.
const FAILED: Duration = Duration::from_secs(10 * 60);
/// The most kept, oldest dropped first.
const KEEP_BYTES: u64 = 256 << 20;

pub struct WebImg;
impl HttpHandler for WebImg {
    fn routes(&self) -> &'static [&'static str] { &["/webimg"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            let url = super::fm::from_uri(&req.param("url"));
            if !allowed(&url) { return HttpReply::text("400 Bad Request", "not a web picture") }
            let dir = hub.cfg.home.join("cache").join("webimg");
            match tokio::task::spawn_blocking(move || get(&dir, &url)).await {
                Ok(Some((ct, body))) => HttpReply { status: "200 OK", ctype: ct.into(), extra: "X-Content-Type-Options: nosniff\r\nCache-Control: private, max-age=86400\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox\r\n".into(), body },
                _ => HttpReply::text("404 Not Found", "the picture could not be fetched"),
            }
        })
    }
}

/// An http(s) address with a host that is not this machine or its private network, and nothing that could leave the request line.
pub fn allowed(url: &str) -> bool {
    let l = url.to_ascii_lowercase();
    let Some(rest) = l.strip_prefix("https://").or_else(|| l.strip_prefix("http://")) else { return false };
    if url.len() > 4096 || url.chars().any(|c| c.is_whitespace() || c.is_control()) { return false }
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    if auth.contains('@') { return false }
    let host = if auth.starts_with('[') { auth.split(']').next().unwrap_or("").trim_start_matches('[') } else { auth.split(':').next().unwrap_or("") };
    if host.is_empty() || host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") || host.ends_with(".internal") || host.ends_with(".ts.net") || !host.contains(['.', ':']) { return false }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v) => !(v.is_loopback() || v.is_private() || v.is_link_local() || v.is_unspecified() || v.is_broadcast() || v.octets()[0] == 100 && (64..128).contains(&v.octets()[1])),
            std::net::IpAddr::V6(v) => !(v.is_loopback() || v.is_unspecified() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80),
        };
    }
    true
}

/// The content type of a picture by its first bytes; None for anything else (an error page is no picture).
pub fn kind(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) { return Some("image/png") }
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) { return Some("image/jpeg") }
    if b.starts_with(b"GIF8") { return Some("image/gif") }
    if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" { return Some("image/webp") }
    let head = String::from_utf8_lossy(&b[..b.len().min(1024)]).to_ascii_lowercase();
    let head = head.trim_start_matches('\u{feff}').trim_start();
    if (head.starts_with("<svg") || head.starts_with("<?xml") || head.starts_with("<!--") || head.starts_with("<!doctype svg")) && head.contains("<svg") { return Some("image/svg+xml") }
    None
}

fn key(url: &str) -> String { let h = Sha256::digest(url.as_bytes()); h.iter().take(16).map(|b| format!("{b:02x}")).collect() }
fn age(p: &Path) -> Option<Duration> { std::fs::metadata(p).ok()?.modified().ok().and_then(|m| SystemTime::now().duration_since(m).ok()) }

/// The picture, kept or fetched.
fn get(dir: &Path, url: &str) -> Option<(&'static str, Vec<u8>)> {
    let _ = std::fs::create_dir_all(dir);
    let (file, fail) = (dir.join(key(url)), dir.join(format!("{}.failed", key(url))));
    let kept = std::fs::read(&file).ok().and_then(|b| kind(&b).map(|k| (k, b)));
    if kept.is_some() && age(&file).map_or(false, |a| a < FRESH) { return kept }
    if age(&fail).map_or(false, |a| a < FAILED) { return kept }
    match fetch(dir, url) {
        Some(b) => {
            let k = kind(&b)?;
            let tmp = dir.join(format!("{}.part", key(url)));
            if std::fs::write(&tmp, &b).is_ok() { let _ = std::fs::rename(&tmp, &file); }
            let _ = std::fs::remove_file(&fail);
            trim(dir);
            Some((k, b))
        }
        None => { let _ = std::fs::write(&fail, b""); kept }
    }
}

/// One fetch through curl (proc Kind::Net: a copy of a real log reaches no web): redirects followed only to http(s), at most MAX_BYTES, 20 s.
fn fetch(dir: &Path, url: &str) -> Option<Vec<u8>> {
    let out: PathBuf = dir.join(format!("{}.dl", key(url)));
    let st = crate::proc::std_cmd(crate::proc::Kind::Net, "curl")
        .args(["-sSfL", "--max-redirs", "5", "--proto", "=http,https", "--proto-redir", "=http,https", "--max-time", "20", "--max-filesize", &MAX_BYTES.to_string(), "-A", "Backplane", "-o"])
        .arg(&out).arg("--").arg(url).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().ok()?;
    let b = std::fs::read(&out).ok(); let _ = std::fs::remove_file(&out);
    if !st.success() { return None }
    b.filter(|b| !b.is_empty() && (b.len() as u64) <= MAX_BYTES)
}

/// Keep the folder under KEEP_BYTES, the least recently fetched first.
fn trim(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = rd.flatten().filter_map(|e| { let m = e.metadata().ok()?; Some((m.modified().ok()?, m.len(), e.path())) }).collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= KEEP_BYTES { return }
    files.sort();
    for (_, n, p) in files { if total <= KEEP_BYTES { break } if std::fs::remove_file(&p).is_ok() { total -= n } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn only_public_web_addresses() {
        for ok in ["https://img.shields.io/badge/x-y-green.svg", "http://example.com/a.png?x=1", "https://raw.githubusercontent.com/o/r/main/a.png", "https://1.1.1.1/a.png"] { assert!(allowed(ok), "{ok}") }
        for bad in ["file:///etc/passwd", "ftp://x.io/a", "https://localhost/a", "http://127.0.0.1/a", "http://192.168.1.1/x", "http://10.0.0.2/", "http://[::1]/", "http://169.254.169.254/latest",
            "http://100.101.1.2/", "https://box.tail1234.ts.net/x", "http://router/", "https://u:p@x.io/", "https://x.io/a b", "https://x.io/a\r\nHost: y", "javascript:alert(1)"] { assert!(!allowed(bad), "{bad}") }
    }
    #[test] fn pictures_by_their_bytes() {
        assert_eq!(kind(b"\x89PNG\r\n"), Some("image/png")); assert_eq!(kind(b"<?xml version=\"1.0\"?><svg xmlns=\"\">"), Some("image/svg+xml"));
        assert_eq!(kind(b"<svg width=\"1\"/>"), Some("image/svg+xml")); assert_eq!(kind(b"<!DOCTYPE html><html>"), None); assert_eq!(kind(b""), None);
    }
    #[test] fn a_kept_picture_is_served_without_the_web_and_a_kept_failure_is_not_retried() {
        let d = crate::plat::tmp::dir("webimg");
        let url = "https://example.invalid/a.png";
        std::fs::write(d.join(key(url)), b"\x89PNG\r\n\x1a\n....").unwrap();
        assert_eq!(get(&d, url).map(|x| x.0), Some("image/png"));
        let other = "https://example.invalid/b.png"; std::fs::write(d.join(format!("{}.failed", key(other))), b"").unwrap();
        assert_eq!(get(&d, other), None, "a recent failure is not fetched again");
    }
}

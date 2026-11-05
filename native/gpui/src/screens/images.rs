//! Images in messages: fetched once from the hub (`/message-image?entry=&path=` for a reply's images, the hub's
//! `url` for attachments), kept bounded, and drawn when they arrive. The fetch runs on a thread of its own; the
//! window repaints while any is loading (glue.rs). The token goes in the query as for /attach and is never logged.
use super::attach::Target;
use gpui::{Image, ImageFormat};
use std::{collections::HashMap, io::{Read, Write}, net::TcpStream, sync::{Arc, Mutex}, time::Duration};

pub const MAX_BYTES: usize = 24 << 20;
const KEEP: usize = 48;

#[derive(Clone)]
pub enum Slot { Loading, Ready(Arc<Image>), Failed }

#[derive(Clone, Default)]
pub struct Images { slots: Arc<Mutex<HashMap<String, Slot>>>, order: Arc<Mutex<Vec<String>>> }

/// The image format by its first bytes; None for anything else (a hub error page is not an image).
pub fn sniff(b: &[u8]) -> Option<ImageFormat> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) { Some(ImageFormat::Png) }
    else if b.starts_with(&[0xFF, 0xD8, 0xFF]) { Some(ImageFormat::Jpeg) }
    else if b.starts_with(b"GIF8") { Some(ImageFormat::Gif) }
    else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" { Some(ImageFormat::Webp) }
    else { None }
}

/// A GET request for a path on the hub, with the token appended to the query.
pub fn request(t: &Target, url: &str) -> String {
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("GET {url}{sep}token={} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", t.token, t.addr)
}

/// Split an HTTP response into (status, body); None when it is not a complete head.
pub fn split(resp: &[u8]) -> Option<(u16, &[u8])> {
    let at = resp.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&resp[..at]).ok()?;
    let status = head.split_whitespace().nth(1)?.parse().ok()?;
    Some((status, &resp[at + 4..]))
}

fn fetch(t: &Target, url: &str) -> Option<Arc<Image>> {
    let mut s = TcpStream::connect(&t.addr).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(30))).ok()?; s.set_write_timeout(Some(Duration::from_secs(30))).ok()?;
    s.write_all(request(t, url).as_bytes()).ok()?;
    let mut buf = Vec::new(); s.take((MAX_BYTES + 8192) as u64).read_to_end(&mut buf).ok()?;
    let (status, body) = split(&buf)?;
    if status != 200 || body.len() > MAX_BYTES { return None; }
    Some(Arc::new(Image::from_bytes(sniff(body)?, body.to_vec())))
}

impl Images {
    /// The image for `url` if it has arrived; the first ask starts the fetch.
    pub fn get(&self, t: Option<&Target>, url: &str) -> Slot {
        let Ok(mut m) = self.slots.lock() else { return Slot::Failed };
        if let Some(s) = m.get(url) { return s.clone(); }
        let Some(t) = t.cloned() else { return Slot::Failed };
        m.insert(url.to_string(), Slot::Loading);
        if let Ok(mut o) = self.order.lock() { o.push(url.to_string()); if o.len() > KEEP { let old = o.remove(0); m.remove(&old); } }
        drop(m);
        let (slots, url) = (self.slots.clone(), url.to_string());
        std::thread::spawn(move || {
            let r = fetch(&t, &url).map_or(Slot::Failed, Slot::Ready);
            if let Ok(mut m) = slots.lock() { if m.contains_key(&url) { m.insert(url, r); } }
        });
        Slot::Loading
    }
    pub fn loading(&self) -> bool { self.slots.lock().map_or(false, |m| m.values().any(|s| matches!(s, Slot::Loading))) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test] fn formats_by_magic_bytes() {
        assert!(matches!(sniff(&[0x89, b'P', b'N', b'G', 0, 0]), Some(ImageFormat::Png))); assert!(matches!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(ImageFormat::Jpeg)));
        assert!(sniff(b"<html>not found</html>").is_none()); assert!(sniff(b"").is_none());
        assert!(matches!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(ImageFormat::Webp)));
    }
    #[test] fn the_request_carries_the_token_in_the_query() {
        let t = Target { addr: "h:1".into(), token: "tok".into() };
        assert!(request(&t, "/message-image?entry=e1&path=%2Fa.png").starts_with("GET /message-image?entry=e1&path=%2Fa.png&token=tok HTTP/1.1\r\nHost: h:1\r\n"));
        assert!(request(&t, "/img").starts_with("GET /img?token=tok "));
    }
    #[test] fn responses_split_into_status_and_body() {
        assert_eq!(split(b"HTTP/1.1 200 OK\r\nA: b\r\n\r\nBODY"), Some((200, &b"BODY"[..]))); assert_eq!(split(b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap().0, 404); assert!(split(b"HTTP/1.1 200").is_none());
    }
    /// A real fetch from a local one-shot server, and a 404 that must not become an image.
    #[test] fn fetches_a_png_and_rejects_an_error_page() {
        let png: Vec<u8> = { let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]; v.extend_from_slice(&[0; 32]); v };
        for (status, body, ok) in [("200 OK", png.clone(), true), ("404 Not Found", b"<html>no</html>".to_vec(), false)] {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let addr = l.local_addr().unwrap().to_string();
            let b2 = body.clone();
            let srv = std::thread::spawn(move || { let (mut s, _) = l.accept().unwrap(); let mut b = [0u8; 2048]; let _ = s.read(&mut b); let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", b2.len()); let _ = s.write_all(&b2); });
            let t = Target { addr, token: "t".into() };
            assert_eq!(fetch(&t, "/message-image?entry=e&path=p").is_some(), ok);
            srv.join().unwrap();
        }
    }
}

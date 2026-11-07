//! Images in messages: fetched once from the hub (`/message-image?entry=&path=` for a reply's images, the hub's
//! `url` for attachments), kept bounded, and drawn when they arrive. The fetch runs on a thread of its own; the
//! window repaints while any is loading (glue.rs). The token goes in the query as for /attach and is never logged.
use super::attach::Target;
use gpui::{prelude::*, *};
use std::{collections::{HashMap, VecDeque}, io::{Read, Write}, net::TcpStream, sync::{Arc, Condvar, Mutex}, time::Duration};

pub const MAX_BYTES: usize = 24 << 20;
/// What the cache may hold, and how many fetches run at once.
pub const CACHE_BYTES: usize = 128 << 20;
pub const WORKERS: usize = 4;

#[derive(Clone)]
pub enum Slot { Loading, Ready(Arc<Image>), Failed }

struct Entry { slot: Slot, used: u64, bytes: usize }
#[derive(Default)]
struct State { items: HashMap<String, Entry>, queue: VecDeque<(String, Target)>, workers: usize, tick: u64 }

/// Shared by the render and the fetch workers.
#[derive(Clone, Default)]
pub struct Images { s: Arc<(Mutex<State>, Condvar)> }

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
    let mut s = t.stream().ok()?; s.set_timeouts(Duration::from_secs(30)).ok()?;
    s.write_all(request(t, url).as_bytes()).ok()?;
    let mut buf = Vec::new(); s.take((MAX_BYTES + 8192) as u64).read_to_end(&mut buf).ok()?;
    let (status, body) = split(&buf)?;
    if status != 200 || body.len() > MAX_BYTES { return None; }
    Some(Arc::new(Image::from_bytes(sniff(body)?, body.to_vec())))
}

/// Pixel size of an image from its header: PNG's IHDR, a JPEG's start-of-frame, a GIF's screen, WebP's VP8/VP8L/VP8X chunk.
pub fn dims(b: &[u8]) -> Option<(u32, u32)> {
    let be = |i: usize| b.get(i..i + 4).map(|x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]));
    let le16 = |i: usize| b.get(i..i + 2).map(|x| u16::from_le_bytes([x[0], x[1]]) as u32);
    let r = if b.starts_with(&[0x89, b'P', b'N', b'G']) { (be(16)?, be(20)?) }
    else if b.starts_with(b"GIF8") { (le16(6)?, le16(8)?) }
    else if b.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        loop {
            while *b.get(i)? != 0xFF { i += 1; }
            while *b.get(i)? == 0xFF { i += 1; }
            let m = *b.get(i)?; i += 1;
            if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) { let h = ((*b.get(i + 3)? as u32) << 8) | *b.get(i + 4)? as u32; let w = ((*b.get(i + 5)? as u32) << 8) | *b.get(i + 6)? as u32; break (w, h); }
            if m == 0xD8 || m == 0x01 || (0xD0..=0xD7).contains(&m) { continue; }
            i += ((*b.get(i)? as usize) << 8) | *b.get(i + 1)? as usize;
        }
    }
    else if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        match b.get(12..16)? {
            b"VP8X" => { let g = |i: usize| -> Option<u32> { Some(*b.get(i)? as u32 | (*b.get(i + 1)? as u32) << 8 | (*b.get(i + 2)? as u32) << 16) }; (g(24)? + 1, g(27)? + 1) }
            b"VP8L" => { let v = u32::from_le_bytes(b.get(21..25)?.try_into().ok()?); ((v & 0x3FFF) + 1, ((v >> 14) & 0x3FFF) + 1) }
            _ => (le16(26)? & 0x3FFF, le16(28)? & 0x3FFF),
        }
    } else { return None };
    if r.0 == 0 || r.1 == 0 { None } else { Some(r) }
}

/// The size a picture is drawn at: its own pixels, shrunk to fit `max_w` x `max_h` with its aspect kept, never enlarged. Layout reserves exactly this box.
pub fn fit(w: u32, h: u32, max_w: f32, max_h: f32) -> (f32, f32) {
    let (w, h) = (w as f32, h as f32); let k = (max_w / w).min(max_h / h).min(1.0); ((w * k).max(1.0), (h * k).max(1.0))
}

/// Only a path on the hub, no whitespace or control characters (nothing that could add to the request line).
pub fn safe_url(url: &str) -> bool { url.starts_with('/') && !url.starts_with("//") && !url.chars().any(|c| c.is_whitespace() || c.is_control()) }

impl Images {
    /// A new frame: what is asked for from now on counts as drawn in it.
    pub fn frame(&self) { if let Ok(mut st) = self.s.0.lock() { st.tick += 1; } }
    /// The image for `url` if it has arrived; the first ask queues the fetch. A failed one stays failed until `retry`.
    pub fn get(&self, t: Option<&Target>, url: &str) -> Slot {
        let Ok(mut st) = self.s.0.lock() else { return Slot::Failed };
        let tick = st.tick;
        if let Some(e) = st.items.get_mut(url) { e.used = tick; return e.slot.clone(); }
        let Some(t) = t.cloned().filter(|_| safe_url(url)) else { st.items.insert(url.to_string(), Entry { slot: Slot::Failed, used: tick, bytes: 0 }); return Slot::Failed };
        st.items.insert(url.to_string(), Entry { slot: Slot::Loading, used: tick, bytes: 0 });
        st.queue.push_back((url.to_string(), t));
        if st.workers < WORKERS { st.workers += 1; let me = self.clone(); std::thread::spawn(move || me.work()); }
        self.s.1.notify_one();
        Slot::Loading
    }
    /// A thread was opened again: failed images may be asked for once more.
    pub fn retry(&self) { if let Ok(mut st) = self.s.0.lock() { st.items.retain(|_, e| !matches!(e.slot, Slot::Failed)); } }
    pub fn loading(&self) -> bool { self.s.0.lock().map_or(false, |st| st.items.values().any(|e| matches!(e.slot, Slot::Loading))) }
    pub fn bytes(&self) -> usize { self.s.0.lock().map_or(0, |st| st.items.values().map(|e| e.bytes).sum()) }
    fn work(&self) {
        loop {
            let job = {
                let Ok(mut st) = self.s.0.lock() else { return };
                loop { if let Some(j) = st.queue.pop_front() { break j; } st = match self.s.1.wait_timeout(st, Duration::from_secs(30)) { Ok((g, to)) => { if to.timed_out() && g.queue.is_empty() { let mut g = g; g.workers -= 1; return; } g } Err(_) => return }; }
            };
            let img = fetch(&job.1, &job.0);
            let Ok(mut st) = self.s.0.lock() else { return };
            let bytes = img.as_ref().map_or(0, |i| i.bytes.len());
            if let Some(e) = st.items.get_mut(&job.0) { e.slot = img.map_or(Slot::Failed, Slot::Ready); e.bytes = bytes; }
            // over the byte cap: drop the least recently drawn images that were not drawn in the current frame
            let tick = st.tick;
            while st.items.values().map(|e| e.bytes).sum::<usize>() > CACHE_BYTES {
                let old = st.items.iter().filter(|(_, e)| e.used < tick && e.bytes > 0).min_by_key(|(_, e)| e.used).map(|(k, _)| k.clone());
                match old { Some(k) => { st.items.remove(&k); } None => break }
            }
        }
    }
}

/// The lightbox: the picture as large as the window allows over a dimmed page; a click anywhere, or Esc, puts it away.
pub fn lightbox(cx: &mut Context<crate::Backplane>, shown: Option<&(Arc<gpui::Image>, String)>) -> Option<AnyElement> {
    let (img, name) = shown?;
    Some(div().id("lightbox").absolute().inset_0().occlude().flex().flex_col().items_center().justify_center().gap_2().p_6().bg(gpui::hsla(0., 0., 0., 0.88)).cursor_pointer()
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.screens.lightbox = None; cx.notify(); }))
        .child(gpui::img(ImageSource::Image(img.clone())).flex_1().min_h_0().max_w_full().object_fit(ObjectFit::Contain))
        .child(div().text_size(px(12.)).text_color(gpui::rgb(0xc8ccd4)).child(format!("{name} · click or Esc to close"))).into_any_element())
}

#[cfg(test)]
mod tests {
    #[test] fn size_from_headers_and_fit_keeps_aspect() {
        let mut png = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R']; png.extend(480u32.to_be_bytes()); png.extend(640u32.to_be_bytes());
        assert_eq!(super::dims(&png), Some((480, 640))); assert_eq!(super::dims(b"nope"), None);
        let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xC0, 0, 17, 8, 0x01, 0xF4, 0x03, 0x20, 3];
        assert_eq!(super::dims(&jpg), Some((800, 500)));
        let (w, h) = super::fit(480, 640, 480.0, 360.0); assert!((w / h - 0.75).abs() < 1e-3 && h <= 360.0 && w <= 480.0);
        assert_eq!(super::fit(100, 50, 480.0, 360.0), (100.0, 50.0));
    }
    use super::*;
    use core::prelude::v1::test;
    #[test] fn formats_by_magic_bytes() {
        assert!(matches!(sniff(&[0x89, b'P', b'N', b'G', 0, 0]), Some(ImageFormat::Png))); assert!(matches!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(ImageFormat::Jpeg)));
        assert!(sniff(b"<html>not found</html>").is_none()); assert!(sniff(b"").is_none());
        assert!(matches!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(ImageFormat::Webp)));
    }
    #[test] fn only_hub_paths_are_requested() {
        for ok in ["/message-image?entry=e&path=%2Fa.png", "/img?path=x"] { assert!(safe_url(ok), "{ok}"); }
        for bad in ["http://evil/", "//evil/x", "x", "/a b", "/a\r\nHost: x", "/a\u{7f}", ""] { assert!(!safe_url(bad), "{bad:?}"); }
    }
    #[test] fn failed_stays_failed_until_retry_and_never_refetches() {
        let im = Images::default();
        assert!(matches!(im.get(None, "/x.png"), Slot::Failed));            // no target: failed, recorded
        assert!(matches!(im.get(Some(&Target { addr: "127.0.0.1:1".into(), token: "t".into(), tls: false }), "/x.png"), Slot::Failed), "a failed image is not fetched again");
        im.retry(); assert!(matches!(im.get(Some(&Target { addr: "127.0.0.1:1".into(), token: "t".into(), tls: false }), "/x.png"), Slot::Loading));
    }
    #[test] fn many_images_use_a_fixed_pool_and_never_evict_what_is_drawn() {
        // a server that hangs up on every request: each fetch fails fast (a refused connection takes Windows about 2 s, on every try)
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let addr = l.local_addr().unwrap().to_string();
        std::thread::spawn(move || { for c in l.incoming() { drop(c) } });
        let im = Images::default(); let t = Target { addr, token: "t".into(), tls: false };
        im.frame();
        for i in 0..200 { let _ = im.get(Some(&t), &format!("/img?path={i}")); }
        for _ in 0..200 { if !im.loading() { break; } std::thread::sleep(Duration::from_millis(25)); }
        assert!(!im.loading(), "loading must end, or the window repaints forever");
        assert!(im.s.0.lock().unwrap().workers <= WORKERS);
        assert_eq!(im.s.0.lock().unwrap().items.len(), 200, "everything drawn this frame is kept");
    }
    #[test] fn the_request_carries_the_token_in_the_query() {
        let t = Target { addr: "h:1".into(), token: "tok".into(), tls: false };
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
            let t = Target { addr, token: "t".into(), tls: false };
            assert_eq!(fetch(&t, "/message-image?entry=e&path=p").is_some(), ok);
            srv.join().unwrap();
        }
    }
}

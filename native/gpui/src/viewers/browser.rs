//! Browser tab: the hub's newest browser frame for a bot (GET /bots/<id>/screen.jpg, the JPEG the helper wrote). A poller thread exists only while the
//! page does (dropped with it); it fetches every 700 ms, decodes only when the bytes changed, and the page redraws only then, so an idle page
//! (an unchanged frame) costs no repaint. Input to the page (bots.browser clicks/keys, teach mode) is not wired yet.
use gpui::{prelude::*, *};
use std::{sync::{atomic::{AtomicBool, Ordering}, Arc}, time::Duration};

pub const POLL_MS: u64 = 700;
pub const MAX_JPEG: u64 = 8 << 20;

/// BGRA RenderImage from JPEG bytes; None when they are not a decodable JPEG.
pub fn decode_jpeg(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).ok()?;
    let mut rgba = img.to_rgba8();
    for p in rgba.pixels_mut() { p.0.swap(0, 2); } // gpui's image container holds BGRA
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}
fn fnv(bytes: &[u8]) -> u64 { bytes.iter().fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3)) }

pub struct BrowserPage { image: Option<Arc<RenderImage>>, retired: Vec<Arc<RenderImage>>, pub status: String, stop: Arc<AtomicBool>, visible: Arc<AtomicBool> }
impl BrowserPage {
    /// `base` like http://127.0.0.1:3821 (http only: this client has no TLS), `owner` the page's owner as the hub names it (a bot's id, or the thread's id
    /// for a thread's page: the host passes `BT.World.page`'s value), `token` the hub token (a Bearer header, never logged). The poller fetches only while
    /// `set_visible(true)`; mount the page with the tab and call `set_visible(false)` when another tab is shown.
    pub fn new(cx: &mut Context<Self>, base: String, owner: String, token: String) -> Self {
        let stop = Arc::new(AtomicBool::new(false)); let visible = Arc::new(AtomicBool::new(true)); let vis = visible.clone();
        let (tx, rx) = async_channel::bounded::<Result<Arc<RenderImage>, String>>(2);
        let flag = stop.clone();
        std::thread::Builder::new().name("browser-frames".into()).spawn(move || {
            let url = format!("{}/bots/{}/screen.jpg", base.trim_end_matches('/'), owner);
            let mut last = 0u64;
            while !flag.load(Ordering::Relaxed) {
                if !vis.load(Ordering::Relaxed) { std::thread::sleep(Duration::from_millis(POLL_MS)); continue } // another tab is shown: no requests
                let out = match ureq::get(&url).set("Authorization", &format!("Bearer {token}")).timeout(Duration::from_secs(3)).call() {
                    Ok(resp) => {
                        let mut body = Vec::new();
                        match std::io::Read::read_to_end(&mut std::io::Read::take(resp.into_reader(), MAX_JPEG), &mut body) {
                            Ok(_) if fnv(&body) == last => None, // the same frame: nothing to draw
                            Ok(_) => { last = fnv(&body); Some(decode_jpeg(&body).ok_or_else(|| "frame is not a JPEG".to_string())) }
                            Err(e) => Some(Err(format!("read: {e}"))),
                        }
                    }
                    Err(ureq::Error::Status(404, _)) => Some(Err("no browser frame yet".into())),
                    Err(ureq::Error::Status(c, _)) => Some(Err(format!("hub answered {c}"))),
                    Err(ureq::Error::Transport(t)) => Some(Err(format!("hub unreachable: {}", t.kind()))),
                };
                if let Some(msg) = out { if tx.try_send(msg).is_err() && tx.is_closed() { return } }
                std::thread::sleep(Duration::from_millis(POLL_MS));
            }
        }).ok();
        cx.spawn(async move |weak, cx| {
            while let Ok(msg) = rx.recv().await {
                if weak.update(cx, |this, cx| { match msg { Ok(i) => { if let Some(old) = this.image.replace(i) { this.retired.push(old); } this.status.clear(); } Err(e) => this.status = e } cx.notify(); }).is_err() { break }
            }
        }).detach();
        let status = if base.starts_with("https://") { "https hubs are not supported by this page yet (the client has no TLS)".to_string() } else { "Waiting for the first frame".to_string() };
        BrowserPage { image: None, retired: Vec::new(), status, stop, visible }
    }
    pub fn set_visible(&mut self, visible: bool) { self.visible.store(visible, Ordering::Relaxed); }
    /// Give the images back to the atlas (call before dropping the page, with the window).
    pub fn release(&mut self, window: &mut Window) { for i in self.retired.drain(..).chain(self.image.take()) { let _ = window.drop_image(i); } }
}
impl Drop for BrowserPage { fn drop(&mut self) { self.stop.store(true, Ordering::Relaxed); } }
impl Render for BrowserPage {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.retired.drain(..) { let _ = window.drop_image(old); } // a painted RenderImage stays in the atlas until it is dropped
        let status = self.status.clone();
        match &self.image {
            // a dead hub must not look like a live, frozen page: the status shows over the last frame
            Some(i) => div().id("browser-page").size_full().relative().child(img(i.clone()).size_full().object_fit(ObjectFit::Contain))
                .when(!status.is_empty(), |d| d.child(div().absolute().bottom_2().left_2().px_2().text_size(px(12.)).bg(rgb(0x1a1e25)).text_color(rgb(0xf06a6a)).child(status))).into_any_element(),
            None => div().id("browser-page").size_full().flex().items_center().justify_center().text_color(rgb(0x8a93a3)).child(status).into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    // not `use super::*`: that glob brings gpui's own `test` attribute macro into scope and #[test] then expands into itself
    use super::{decode_jpeg, fnv};
    fn jpeg(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb(rgb)); let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Jpeg).unwrap(); out.into_inner()
    }
    #[test] fn decodes_a_jpeg_to_bgra() {
        let i = decode_jpeg(&jpeg(16, 8, [200, 40, 40])).unwrap();
        let b = i.as_bytes(0).unwrap(); assert_eq!(b.len(), 16 * 8 * 4);
        assert!(b[2] > 150 && b[0] < 100, "BGRA: red lands in byte 2, got {:?}", &b[..4]);
    }
    #[test] fn refuses_bytes_that_are_not_a_jpeg() { assert!(decode_jpeg(b"not a jpeg").is_none()); assert!(decode_jpeg(&[]).is_none()); }
    #[test] fn the_same_bytes_hash_the_same_and_a_new_frame_differs() { let a = jpeg(8, 8, [1, 2, 3]); assert_eq!(fnv(&a), fnv(&a.clone())); assert_ne!(fnv(&a), fnv(&jpeg(8, 8, [200, 2, 3]))); }
}

//! Images in messages: fetched once from the hub (`/message-image?entry=&path=` for a reply's images, the hub's
//! `url` for attachments), kept bounded, and drawn when they arrive. The fetch runs on a thread of its own; the
//! window repaints while any is loading (glue.rs). The token goes in the query as for /attach and is never logged.
use super::attach::Target;
use gpui::{actions, prelude::*, *};
use std::{collections::{HashMap, VecDeque}, io::{Read, Write}, net::TcpStream, sync::{Arc, Condvar, Mutex}, time::Duration};

pub const MAX_BYTES: usize = 24 << 20;
/// What the cache may hold, counted as drawn (width x height x 4: GPUI keeps each image decoded, and again in its texture atlas), and how
/// many fetches run at once.
pub const CACHE_BYTES: usize = 256 << 20;
pub const WORKERS: usize = 4;

#[derive(Clone)]
pub enum Slot { Loading, Ready(Arc<Image>), Failed }

struct Entry { slot: Slot, used: u64, bytes: usize }
#[derive(Default)]
struct State { items: HashMap<String, Entry>, queue: VecDeque<(String, Target)>, workers: usize, tick: u64, done: u64, retired: Vec<Arc<Image>>, decoded: HashMap<String, Arc<RenderImage>>, retired_decoded: Vec<Arc<RenderImage>>,
    /// the pictures kept smaller than they came (`shrink`), with their own size: zoom and Save image as fetch those again whole
    shrunk: HashMap<String, (u32, u32)> }

/// Shared by the render and the fetch workers.
#[derive(Clone, Default)]
pub struct Images { s: Arc<(Mutex<State>, Condvar)> }

/// The image format by its first bytes; None for anything else (a hub error page is not an image).
pub fn sniff(b: &[u8]) -> Option<ImageFormat> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) { Some(ImageFormat::Png) }
    else if b.starts_with(&[0xFF, 0xD8, 0xFF]) { Some(ImageFormat::Jpeg) }
    /* i18n-ok: file signatures and an HTTP request */ else if b.starts_with(b"GIF8") { Some(ImageFormat::Gif) }
    /* i18n-ok: file signatures and an HTTP request */ else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" { Some(ImageFormat::Webp) }
    else if is_svg(b) { Some(ImageFormat::Svg) }
    else { None }
}

/// An SVG document by its start (a README's badges are SVG): an XML prolog, comment or doctype may come before the `<svg`.
fn is_svg(b: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&b[..b.len().min(1024)]).to_ascii_lowercase();
    let t = head.trim_start_matches('\u{feff}').trim_start();
    /* i18n-ok: file signatures */ (t.starts_with("<svg") || t.starts_with("<?xml") || t.starts_with("<!--") || t.starts_with("<!doctype svg")) && t.contains("<svg")
}
/// An SVG's size: its `width` and `height` in pixels, else its viewBox's.
fn svg_dims(b: &[u8]) -> Option<(u32, u32)> {
    let head = String::from_utf8_lossy(&b[..b.len().min(4096)]).into_owned();
    let tag = &head[head.find("<svg")?..]; let tag = &tag[..tag.find('>')?];
    let attr = |name: &str| -> Option<String> {
        let mut at = 0;
        while let Some(i) = tag[at..].find(name) { let i = at + i; let before = tag[..i].chars().last().map_or(false, |c| c.is_whitespace()); let rest = tag[i + name.len()..].trim_start();
            if before && rest.starts_with('=') { let rest = rest[1..].trim_start(); let q = rest.chars().next()?; if q == '"' || q == '\'' { return rest[1..].split(q).next().map(String::from) } } at = i + name.len(); }
        None
    };
    let num = |v: Option<String>| v.and_then(|v| { let v = v.trim().trim_end_matches("px"); v.parse::<f32>().ok() }).filter(|x| *x > 0.);
    let (w, h) = (num(attr("width")), num(attr("height")));
    let vb: Vec<f32> = attr("viewBox").map(|v| v.split([' ', ',']).filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    let (w, h) = match (w, h, vb.as_slice()) { (Some(w), Some(h), _) => (w, h), (Some(w), None, [_, _, vw, vh]) => (w, w * vh / vw), (None, Some(h), [_, _, vw, vh]) => (h * vw / vh, h), (None, None, [_, _, vw, vh]) => (*vw, *vh), _ => return None };
    (w >= 1. && h >= 1.).then(|| (w.round() as u32, h.round() as u32))
}

/// A GET request for a path on the hub, with the token appended to the query.
pub fn request(t: &Target, url: &str) -> String {
    let sep = if url.contains('?') { '&' } else { '?' };
    /* i18n-ok: file signatures and an HTTP request */ format!("GET {url}{sep}token={} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", t.token, t.addr)
}

/// Split an HTTP response into (status, body); None when it is not a complete head.
pub fn split(resp: &[u8]) -> Option<(u16, &[u8])> {
    let at = resp.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&resp[..at]).ok()?;
    let status = head.split_whitespace().nth(1)?.parse().ok()?;
    Some((status, &resp[at + 4..]))
}

/// The longest side an image is kept at. Messages show pictures a few hundred pixels wide (the lightbox at most the window), and GPUI keeps
/// each one decoded at full size and again in its texture atlas: the person's 3,800 x 2,500 screenshots cost 37 MB each, twice.
pub const MAX_SIDE: u32 = 2048;
/// A picture larger than MAX_SIDE is scaled down to it here, on the fetch thread (PNG out, quickly compressed); others are kept as they came.
pub fn shrink(img: Arc<Image>) -> Arc<Image> { shrink_to(img, MAX_SIDE) }
/// `shrink` to a longest side of `side`.
pub fn shrink_to(img: Arc<Image>, side: u32) -> Arc<Image> {
    let Some((w, h)) = dims(&img.bytes) else { return img };
    if w.max(h) <= side || !matches!(img.format, ImageFormat::Png | ImageFormat::Jpeg) { return img }
    let Ok(full) = image::load_from_memory(&img.bytes) else { return img };
    let small = full.resize(side, side, image::imageops::FilterType::Triangle).into_rgba8();
    let mut out = Vec::new();
    let enc = image::codecs::png::PngEncoder::new_with_quality(&mut out, image::codecs::png::CompressionType::Fast, image::codecs::png::FilterType::Adaptive);
    use image::ImageEncoder;
    if enc.write_image(small.as_raw(), small.width(), small.height(), image::ExtendedColorType::Rgba8).is_err() { return img }
    Arc::new(Image::from_bytes(ImageFormat::Png, out))
}
/// The picture decoded here, on the fetch thread, as GPUI paints it (BGRA). Handed to `img` as a RenderImage it is drawn in the first frame
/// it is on; as an Image, GPUI decodes it on its own executor and draws an empty box until that ends: a reply's pictures showed as blank boxes
/// for a few frames each time they came on screen (Nov 10). None for a format this build does not decode (GPUI decodes those as before).
pub fn decode(img: &Image) -> Option<Arc<RenderImage>> {
    if img.format == ImageFormat::Svg { return svg_raster(&img.bytes) }
    let format = match img.format { ImageFormat::Png => image::ImageFormat::Png, ImageFormat::Jpeg => image::ImageFormat::Jpeg, _ => return None };
    let mut buf = image::load_from_memory_with_format(&img.bytes, format).ok()?.into_rgba8();
    for px in buf.chunks_exact_mut(4) { px.swap(0, 2); }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buf)])))
}
/// How much finer than its own size an SVG is drawn: sharp on a high-density screen (the box it is drawn in stays its own size).
pub const SVG_SCALE: f32 = 2.;
/// An SVG drawn to pixels as GPUI paints them (BGRA, straight alpha), its text in the system's fonts. GPUI's own drawing of an SVG image
/// gives its red and blue swapped (a blue badge came out orange) and at one pixel a point.
pub fn svg_raster(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    use resvg::{tiny_skia, usvg};
    static FONTS: std::sync::LazyLock<Arc<usvg::fontdb::Database>> = std::sync::LazyLock::new(|| { let mut db = usvg::fontdb::Database::new(); db.load_system_fonts(); Arc::new(db) });
    let opt = usvg::Options { fontdb: FONTS.clone(), ..Default::default() };
    let tree = usvg::Tree::from_data(bytes, &opt).ok()?;
    let size = tree.size();
    let k = SVG_SCALE.min(4096. / size.width().max(size.height()).max(1.));
    let (w, h) = ((size.width() * k).ceil() as u32, (size.height() * k).ceil() as u32);
    let mut pix = tiny_skia::Pixmap::new(w.max(1), h.max(1))?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(k, k), &mut pix.as_mut());
    let mut buf = Vec::with_capacity((w * h * 4) as usize);
    for p in pix.pixels() { let c = p.demultiply(); buf.extend_from_slice(&[c.blue(), c.green(), c.red(), c.alpha()]); }
    let frame = image::RgbaImage::from_raw(w.max(1), h.max(1), buf)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(frame)])))
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
    /* i18n-ok: file signatures and an HTTP request */ else if b.starts_with(b"GIF8") { (le16(6)?, le16(8)?) }
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
    /* i18n-ok: file signatures and an HTTP request */ else if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        match b.get(12..16)? {
            b"VP8X" => { let g = |i: usize| -> Option<u32> { Some(*b.get(i)? as u32 | (*b.get(i + 1)? as u32) << 8 | (*b.get(i + 2)? as u32) << 16) }; (g(24)? + 1, g(27)? + 1) }
            b"VP8L" => { let v = u32::from_le_bytes(b.get(21..25)?.try_into().ok()?); ((v & 0x3FFF) + 1, ((v >> 14) & 0x3FFF) + 1) }
            _ => (le16(26)? & 0x3FFF, le16(28)? & 0x3FFF),
        }
    } else if is_svg(b) { svg_dims(b)? } else { return None };
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
    /// The image for `url` if it was asked for (its state), without asking: a document's pictures are asked for only near the screen.
    pub fn peek(&self, url: &str) -> Option<Slot> {
        let Ok(mut st) = self.s.0.lock() else { return None };
        let tick = st.tick;
        st.items.get_mut(url).map(|e| { e.used = tick; e.slot.clone() })
    }
    /// A thread was opened again: failed images may be asked for once more.
    /// The picture for `url` decoded for painting, once it has arrived and could be (see `decode`).
    pub fn decoded(&self, url: &str) -> Option<Arc<RenderImage>> { self.s.0.lock().ok()?.decoded.get(url).cloned() }
    /// The size the picture for `url` came at when the copy kept here is smaller (`shrink`); None when the copy is the picture as it came.
    pub fn shrunk(&self, url: &str) -> Option<(u32, u32)> { self.s.0.lock().ok()?.shrunk.get(url).copied() }
    pub fn retry(&self) { if let Ok(mut st) = self.s.0.lock() { st.items.retain(|_, e| !matches!(e.slot, Slot::Failed)); } }
    /// How many fetches have finished (arrived or failed) so far: the window redraws when this moves, not on every poll.
    pub fn done(&self) -> u64 { self.s.0.lock().map_or(0, |st| st.done) }
    /// The images the cache let go of leave GPUI too: its decoded copy (asset cache) and its texture, which otherwise stay for as long as the
    /// window runs, however many images a long thread shows. True when one went (the parts are drawn again: one may still have been on screen).
    pub fn release(&self, window: &mut Window, cx: &mut App) -> bool {
        let (gone, gone_decoded) = self.s.0.lock().map(|mut st| (std::mem::take(&mut st.retired), std::mem::take(&mut st.retired_decoded))).unwrap_or_default();
        for r in &gone_decoded { cx.drop_image(r.clone(), Some(window)); }
        for img in &gone {
            if let Some(r) = img.clone().get_render_image(window, cx) { cx.drop_image(r, Some(window)); }
            img.clone().remove_asset(cx);
        }
        !gone.is_empty() || !gone_decoded.is_empty()
    }
    pub fn loading(&self) -> bool { self.s.0.lock().map_or(false, |st| st.items.values().any(|e| matches!(e.slot, Slot::Loading))) }
    pub fn bytes(&self) -> usize { self.s.0.lock().map_or(0, |st| st.items.values().map(|e| e.bytes).sum()) }
    fn work(&self) {
        loop {
            let job = {
                let Ok(mut st) = self.s.0.lock() else { return };
                loop { if let Some(j) = st.queue.pop_front() { break j; } st = match self.s.1.wait_timeout(st, Duration::from_secs(30)) { Ok((g, to)) => { if to.timed_out() && g.queue.is_empty() { let mut g = g; g.workers -= 1; return; } g } Err(_) => return }; }
            };
            let came = fetch(&job.1, &job.0);
            let img = came.clone().map(shrink);
            let smaller = match (&came, &img) { (Some(c), Some(i)) if !Arc::ptr_eq(c, i) => dims(&c.bytes), _ => None };
            drop(came);
            let painted = img.as_deref().and_then(decode);
            let Ok(mut st) = self.s.0.lock() else { return };
            match smaller { Some(d) => { st.shrunk.insert(job.0.clone(), d); } None => { st.shrunk.remove(&job.0); } }
            if let Some(r) = painted { st.decoded.insert(job.0.clone(), r); }
            // what it costs once drawn: GPUI's decoded copy (BGRA), whatever the file's own size
            let bytes = img.as_ref().map_or(0, |i| dims(&i.bytes).map_or(i.bytes.len() * 4, |(w, h)| w as usize * h as usize * 4));
            if let Some(e) = st.items.get_mut(&job.0) { e.slot = img.map_or(Slot::Failed, Slot::Ready); e.bytes = bytes; }
            st.done += 1;
            // over the byte cap: drop the least recently drawn images that were not drawn in the current frame
            let tick = st.tick;
            while st.items.values().map(|e| e.bytes).sum::<usize>() > CACHE_BYTES {
                let old = st.items.iter().filter(|(_, e)| e.used < tick && e.bytes > 0).min_by_key(|(_, e)| e.used).map(|(k, _)| k.clone());
                match old { Some(k) => { if let Some(Entry { slot: Slot::Ready(i), .. }) = st.items.remove(&k) { st.retired.push(i); } if let Some(r) = st.decoded.remove(&k) { st.retired_decoded.push(r); } st.shrunk.remove(&k); } None => break }
            }
        }
    }
}

// The lightbox's zoom keys (bound in the `Lightbox` key context, so only while a picture is shown large).
actions!(lightbox, [ZoomIn, ZoomOut, ZoomFit]);
/* i18n-ok: key names and a key context */ pub fn bindings() -> Vec<KeyBinding> {
    /* i18n-ok: key names and a key context */ const CX: Option<&str> = Some("Lightbox");
    /* i18n-ok: key names and a key context */ vec![KeyBinding::new("secondary-=", ZoomIn, CX), KeyBinding::new("secondary-+", ZoomIn, CX), KeyBinding::new("secondary-shift-=", ZoomIn, CX),
        KeyBinding::new("secondary--", ZoomOut, CX), KeyBinding::new("secondary-0", ZoomFit, CX)]
}

/// Room left around a fitted picture, in px.
pub const FIT_MARGIN: f32 = 16.;
/// One zoom step (Cmd/Ctrl + = or -, the bar's buttons).
pub const STEP: f32 = 1.25;
/// How far in the lightbox zooms: this many screen pixels to one of the picture's, or this many times the fitted size, whichever is more.
pub const MAX_ZOOM: f32 = 16.;
/// The longest side the lightbox paints an original at (a GPU texture has a limit); Save image as still writes it whole.
pub const PAINT_SIDE: u32 = 8192;

/// What the lightbox's zoom works in: the picture's own pixels, the area it is shown in (px) and the scale of actual size (one of the
/// picture's pixels to one of the screen's: 1 / the window's scale factor, in px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geom { pub pic: (f32, f32), pub area: (f32, f32), pub actual: f32 }
impl Geom {
    /// The scale that shows it whole inside the area, never larger than its own size.
    pub fn fit(&self) -> f32 {
        let (aw, ah) = ((self.area.0 - 2. * FIT_MARGIN).max(1.), (self.area.1 - 2. * FIT_MARGIN).max(1.));
        (aw / self.pic.0.max(1.)).min(ah / self.pic.1.max(1.)).min(1.)
    }
    /// How small and how large it may be drawn: down to the fit or actual size, up to MAX_ZOOM times either, and always far enough to
    /// fill the area twice over (a small picture can still be zoomed past the window and moved about).
    pub fn limits(&self) -> (f32, f32) {
        let f = self.fit(); let cover = (self.area.0 / self.pic.0.max(1.)).max(self.area.1 / self.pic.1.max(1.));
        (f.min(self.actual), (self.actual * MAX_ZOOM).max(f * MAX_ZOOM).max(cover * 2.))
    }
}
/// The lightbox's view: `k` px per picture pixel (0: fitted, which follows the area's size), `off` the picture's centre from the area's.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Zoom { pub k: f32, pub off: (f32, f32) }
impl Zoom {
    pub fn fitted(&self) -> bool { self.k <= 0. }
    pub fn scale(&self, g: &Geom) -> f32 { if self.fitted() { g.fit() } else { self.k } }
    /// To scale `k1` (held to the limits), keeping the picture's point under `at` (from the area's centre) where it is; landing on the fit is fitted.
    pub fn to(self, k1: f32, at: (f32, f32), g: &Geom) -> Zoom {
        let (lo, hi) = g.limits(); let fit = g.fit(); let k0 = self.scale(g);
        if !k1.is_finite() || k0 <= 0. { return self }
        let k1 = k1.clamp(lo, hi);
        if (k1 - fit).abs() <= fit * 1e-3 { return Zoom::default() }
        let r = k1 / k0; let off = if self.fitted() { (0., 0.) } else { self.off };
        Zoom { k: k1, off: (at.0 - (at.0 - off.0) * r, at.1 - (at.1 - off.1) * r) }.clamped(g)
    }
    /// Zoom by `factor` at `at` (a wheel, a pinch).
    pub fn by(self, factor: f32, at: (f32, f32), g: &Geom) -> Zoom { if !factor.is_finite() || factor <= 0. || factor == 1. { return self } self.to(self.scale(g) * factor, at, g) }
    /// One step in or out; actual size (100%) and the fit are stops on the way.
    pub fn step(self, up: bool, at: (f32, f32), g: &Geom) -> Zoom {
        let k0 = self.scale(g); let k1 = if up { k0 * STEP } else { k0 / STEP };
        let between = |s: f32| if up { s > k0 * 1.001 && s < k1 } else { s < k0 / 1.001 && s > k1 };
        let stop = [g.actual, g.fit()].into_iter().filter(|s| between(*s)).min_by(|a, b| (a - k0).abs().total_cmp(&(b - k0).abs()));
        self.to(stop.unwrap_or(k1), at, g)
    }
    /// Actual size at `at`.
    pub fn actual(self, at: (f32, f32), g: &Geom) -> Zoom { self.to(g.actual, at, g) }
    /// A double click: from the fit to actual size at the pointer (twice the fit when that is the fit), from anything else back to the fit.
    pub fn toggle(self, at: (f32, f32), g: &Geom) -> Zoom {
        if !self.fitted() { return Zoom::default() }
        let fit = g.fit(); self.to(if (g.actual - fit).abs() <= fit * 1e-3 { fit * 2. } else { g.actual }, at, g)
    }
    /// Moved by `d` px (a drag, a scroll); a fitted picture stays put.
    pub fn pan(self, d: (f32, f32), g: &Geom) -> Zoom { if self.fitted() { return self } Zoom { k: self.k, off: (self.off.0 + d.0, self.off.1 + d.1) }.clamped(g) }
    /// Kept on screen: centred along a side where it is smaller than the area, else never leaving a gap at an edge.
    pub fn clamped(self, g: &Geom) -> Zoom {
        let k = self.scale(g);
        let c = |off: f32, size: f32, area: f32| { let room = (size * k - area) / 2.; if room <= 0. { 0. } else { off.clamp(-room, room) } };
        Zoom { k: self.k, off: (c(self.off.0, g.pic.0, g.area.0), c(self.off.1, g.pic.1, g.area.1)) }
    }
    /// Where it is drawn: x, y (from the area's top left), width and height, in px.
    pub fn rect(&self, g: &Geom) -> (f32, f32, f32, f32) {
        let z = if self.fitted() { Zoom::default() } else { self.clamped(g) }; let k = self.scale(g);
        let (w, h) = (g.pic.0 * k, g.pic.1 * k);
        (g.area.0 / 2. + z.off.0 - w / 2., g.area.1 / 2. + z.off.1 - h / 2., w, h)
    }
    /// Larger than the area along a side: a drag moves it.
    pub fn pannable(&self, g: &Geom) -> bool { let (_, _, w, h) = self.rect(g); !self.fitted() && (w > g.area.0 + 0.5 || h > g.area.1 + 0.5) }
    /// Its size as a share of actual size.
    pub fn percent(&self, g: &Geom) -> u32 { (self.scale(g) / g.actual.max(1e-6) * 100.).round() as u32 }
}

/// The picture behind a menu's Save image as or the lightbox's: the copy in hand, its name, where the hub serves it, and whether the copy
/// in hand was made smaller than the file (then the file is fetched again, whole).
#[derive(Clone)]
pub struct Save { pub img: Arc<Image>, pub name: String, pub src: Option<(Target, String)>, pub shrunk: bool }
/// The file's bytes as they came: the copy in hand, or fetched again when that one was made smaller. Never the smaller copy.
pub fn original_bytes(s: &Save) -> Result<Vec<u8>, String> {
    if !s.shrunk { return Ok(s.img.bytes.clone()) }
    let (t, url) = s.src.as_ref().ok_or_else(|| crate::i18n::t("gpui.c.image.err.nohub"))?;
    fetch(t, url).map(|i| i.bytes.clone()).ok_or_else(|| crate::i18n::t("gpui.c.image.err.unsent"))
}
/// The file name offered: the picture's own name (no folders, nothing a file system refuses), with the extension of its format when it has none.
pub fn save_name(name: &str, format: ImageFormat) -> String {
    let base: String = name.rsplit(['/', '\\']).next().unwrap_or("").chars().map(|c| if c.is_control() || ":*?\"<>|".contains(c) { '_' } else { c }).collect();
    let base = base.trim().trim_start_matches('.').to_string();
    let base = if base.is_empty() { "image".to_string() } else { base };
    let ext = match format { ImageFormat::Png => "png", ImageFormat::Jpeg => "jpg", ImageFormat::Gif => "gif", ImageFormat::Webp => "webp", ImageFormat::Svg => "svg", ImageFormat::Bmp => "bmp", ImageFormat::Tiff => "tiff", _ => "img" };
    if base.rsplit_once('.').map_or(false, |(stem, e)| !stem.is_empty() && !e.is_empty() && e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric())) { base } else { format!("{base}.{ext}") }
}
/// Write the original to `path`.
pub fn save_to(s: &Save, path: &std::path::Path) -> Result<(), String> { std::fs::write(path, original_bytes(s)?).map_err(|e| e.to_string()) }

/// A picture opened large: the copy in hand, its name, its file on the hub's machine ("" when it is none), where the hub serves it, and its
/// own size when the copy in hand was made smaller.
#[derive(Clone)]
pub struct Shown { pub img: Arc<Image>, pub name: String, pub file: String, pub src: Option<(Target, String)>, pub shrunk: Option<(u32, u32)> }
/// The original, fetched again for a picture kept smaller: the file, and it decoded for painting.
pub enum Full { None, Loading, Ready(Arc<Image>, Option<Arc<RenderImage>>), Failed }
/// The lightbox: the picture, its zoom, the area it was last drawn in, a drag in progress, its keyboard focus (and the one to give back),
/// and a word after a save.
pub struct Lightbox { pub id: u64, pub shown: Shown, pub paint: Option<Arc<RenderImage>>, pub full: Full, pub zoom: Zoom,
    pub area: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>, pub drag: Option<Point<Pixels>>, pub focus: FocusHandle, pub back: Option<FocusHandle>, pub note: String }
impl Lightbox {
    /// The picture's own size in pixels.
    pub fn dims(&self) -> Option<(u32, u32)> { self.shown.shrunk.or_else(|| dims(&self.shown.img.bytes)) }
    pub fn geom(&self, scale_factor: f32) -> Option<Geom> {
        let a = self.area.get(); let (aw, ah) = (f32::from(a.size.width), f32::from(a.size.height));
        let (w, h) = self.dims()?;
        (aw > 0. && ah > 0.).then(|| Geom { pic: (w as f32, h as f32), area: (aw, ah), actual: 1. / scale_factor.max(0.1) })
    }
    /// A window point from the area's centre.
    pub fn at(&self, p: Point<Pixels>) -> (f32, f32) { let a = self.area.get(); (f32::from(p.x - a.origin.x) - f32::from(a.size.width) / 2., f32::from(p.y - a.origin.y) - f32::from(a.size.height) / 2.) }
    /// A window point on the picture as drawn.
    pub fn on_picture(&self, p: Point<Pixels>, g: &Geom) -> bool {
        let a = self.area.get(); let (x, y, w, h) = self.zoom.rect(g);
        let (px_, py_) = (f32::from(p.x - a.origin.x), f32::from(p.y - a.origin.y));
        px_ >= x && px_ <= x + w && py_ >= y && py_ <= y + h
    }
    /// What Save image as writes: the original when it is here, else the copy in hand (fetched again whole if that was made smaller).
    pub fn save(&self) -> Save {
        match &self.full { Full::Ready(i, _) => Save { img: i.clone(), name: self.shown.name.clone(), src: self.shown.src.clone(), shrunk: false },
            _ => Save { img: self.shown.img.clone(), name: self.shown.name.clone(), src: self.shown.src.clone(), shrunk: self.shown.shrunk.is_some() } }
    }
    /// The best copy to copy to the clipboard.
    pub fn best(&self) -> Arc<Image> { match &self.full { Full::Ready(i, _) => i.clone(), _ => self.shown.img.clone() } }
}

static LIGHTBOX_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl crate::Backplane {
    /// Show a picture large; a picture kept smaller has its original fetched again for the zoom.
    pub(crate) fn lightbox_open(&mut self, shown: Shown, window: &mut Window, cx: &mut Context<Self>) {
        if self.screens.lightbox.is_some() { self.lightbox_close(window, cx); }
        let id = LIGHTBOX_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let paint = shown.src.as_ref().and_then(|(_, u)| self.screens.images.decoded(u));
        let again = shown.shrunk.and(shown.src.clone());
        let focus = cx.focus_handle(); let back = window.focused(cx);
        window.focus(&focus);
        self.screens.lightbox = Some(Lightbox { id, shown, paint, full: if again.is_some() { Full::Loading } else { Full::None }, zoom: Zoom::default(),
            area: Default::default(), drag: None, focus, back, note: String::new() });
        if let Some((t, url)) = again {
            let task = cx.background_executor().spawn(async move { let orig = fetch(&t, &url)?; let paint = decode(&shrink_to(orig.clone(), PAINT_SIDE)); Some((orig, paint)) });
            cx.spawn(async move |this, cx| {
                let got = task.await;
                let _ = this.update(cx, |this, cx| { if let Some(lb) = this.screens.lightbox.as_mut().filter(|lb| lb.id == id) { lb.full = got.map_or(Full::Failed, |(o, p)| Full::Ready(o, p)); cx.notify(); } });
            }).detach();
        }
        cx.notify();
    }
    /// Put the lightbox away: the original leaves GPUI's memory, and the keyboard goes back where it was. True when one was open.
    pub(crate) fn lightbox_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(lb) = self.screens.lightbox.take() else { return false };
        if let Full::Ready(img, paint) = lb.full {
            match paint { Some(r) => cx.drop_image(r, Some(window)), None => { if let Some(r) = img.clone().get_render_image(window, cx) { cx.drop_image(r, Some(window)); } img.remove_asset(cx); } }
        }
        if let Some(b) = lb.back { window.focus(&b); }
        crate::art::forget_probes("lightbox:");
        cx.notify(); true
    }
    fn lightbox_zoom(&mut self, window: &Window, cx: &mut Context<Self>, f: impl FnOnce(Zoom, &Lightbox, &Geom) -> Zoom) {
        let sf = window.scale_factor();
        let Some(lb) = self.screens.lightbox.as_mut() else { return };
        let Some(g) = lb.geom(sf) else { return };
        let z = f(lb.zoom, lb, &g); if z != lb.zoom { lb.zoom = z; cx.notify(); }
    }
    /// Save image as…: the person picks where (the Downloads folder first), then the original is written there.
    pub(crate) fn save_image(&mut self, s: Save, cx: &mut Context<Self>) {
        let name = save_name(&s.name, s.img.format);
        // test hook (no file dialog portal on a bare Xvfb): save to this path instead of asking the person
        let test = std::env::var_os("BACKPLANE_SAVE_TEST_FILE").filter(|f| !f.is_empty()).map(std::path::PathBuf::from);
        let r = test.is_none().then(|| cx.prompt_for_new_path(&crate::viewers::downloads::downloads_folder(), Some(&name)));
        cx.spawn(async move |this, cx| {
            let picked = match r { Some(r) => r.await.ok().and_then(|r| r.ok()).flatten(), None => test };
            let Some(path) = picked else { return };
            let p2 = path.clone();
            let said = cx.background_executor().spawn(async move { save_to(&s, &p2) }).await;
            let note = match &said { Ok(()) => crate::i18n::tf("gpui.c.image.saved", &[("path", path.display().to_string().into())]), Err(e) => { eprintln!("save image: {e}"); crate::i18n::tf("gpui.c.image.save.failed", &[("why", e.clone().into())]) } };
            let _ = this.update(cx, |this, cx| { if let Some(lb) = this.screens.lightbox.as_mut() { lb.note = note; cx.notify(); } });
        }).detach();
    }
    fn lightbox_menu(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(lb) = self.screens.lightbox.as_ref() else { return };
        let (img, save, file) = (lb.best(), lb.save(), lb.shown.file.clone());
        self.screens.ctx_img = Some(img); self.screens.ctx_save = Some(save);
        self.ctx_open(at, super::ctxmenu::image_items(&file), cx);
    }
}

/// The lightbox: the picture over a dimmed page, fitted to it at first. Pinch, Cmd/Ctrl + scroll and Cmd/Ctrl + = / - zoom, a drag moves a
/// zoomed picture (a plain scroll too), a double click or Cmd/Ctrl + 0 fits it again; the bar has Fit, 100%, - and +, its size in pixels and
/// Save image as…. Esc or a press on the dimmed page puts it away, never one on the picture.
pub fn lightbox(cx: &mut Context<crate::Backplane>, lb: Option<&Lightbox>, scale_factor: f32) -> Option<AnyElement> {
    let lb = lb?;
    let g = lb.geom(scale_factor);
    let fg = gpui::rgb(0xc8ccd4);
    let area_cell = lb.area.clone();
    let mut area = crate::ui::with_rect(div().id("lightbox-area"), || "lightbox:area".to_string()).relative().flex_1().min_h_0().w_full().overflow_hidden()
        // the area as laid out: the zoom's frame (drawn again once when it changes, as on the first frame)
        .child(canvas(move |b, window, _| { if area_cell.get() != b { area_cell.set(b); window.refresh(); } }, |_, _, _, _| {}).absolute().size_full());
    if let Some(g) = g {
        let (x, y, w, h) = lb.zoom.rect(&g);
        let src = match &lb.full { Full::Ready(_, Some(r)) => ImageSource::Render(r.clone()), Full::Ready(i, None) => ImageSource::Image(i.clone()), _ => lb.paint.clone().map_or(ImageSource::Image(lb.shown.img.clone()), ImageSource::Render) };
        area = area.child(crate::ui::with_rect(div().absolute().left(px(x)).top(px(y)).w(px(w)).h(px(h)), || "lightbox:pic".to_string()).child(gpui::img(src).size_full().object_fit(ObjectFit::Fill)));
        if lb.zoom.pannable(&g) { area = area.cursor(if lb.drag.is_some() { CursorStyle::ClosedHand } else { CursorStyle::OpenHand }); }
    }
    let area = area
        .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            let sf = window.scale_factor();
            let Some(lb) = this.screens.lightbox.as_mut() else { return };
            let Some(g) = lb.geom(sf) else { return };
            if !lb.on_picture(e.position, &g) { return } // the dimmed page: the lightbox's own press closes it
            cx.stop_propagation();
            if e.click_count == 2 { let at = lb.at(e.position); lb.zoom = lb.zoom.toggle(at, &g); lb.drag = None; } else { lb.drag = Some(e.position); }
            cx.notify();
        }))
        .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            let sf = window.scale_factor();
            let Some(lb) = this.screens.lightbox.as_ref() else { return };
            if !lb.geom(sf).map_or(false, |g| lb.on_picture(e.position, &g)) { return }
            cx.stop_propagation(); this.lightbox_menu(e.position, cx);
        }))
        .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, window, cx| {
            let sf = window.scale_factor();
            let Some(lb) = this.screens.lightbox.as_mut() else { return };
            let Some(from) = lb.drag else { return };
            if e.pressed_button != Some(MouseButton::Left) { lb.drag = None; cx.notify(); return }
            let Some(g) = lb.geom(sf) else { return };
            let d = e.position - from; lb.drag = Some(e.position);
            let z = lb.zoom.pan((f32::from(d.x), f32::from(d.y)), &g); if z != lb.zoom { lb.zoom = z; cx.notify(); }
        }))
        .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| { if let Some(lb) = this.screens.lightbox.as_mut() { if lb.drag.take().is_some() { cx.notify(); } } }))
        .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| { if let Some(lb) = this.screens.lightbox.as_mut() { if lb.drag.take().is_some() { cx.notify(); } } }))
        .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, window, cx| {
            // Cmd/Ctrl + scroll zooms at the pointer (Ctrl + wheel is also how Linux and Windows send a touchpad pinch); a plain scroll moves a zoomed picture
            let d = e.delta.pixel_delta(px(16.)); let (dx, dy) = (f32::from(d.x), f32::from(d.y));
            if e.modifiers.secondary() || e.modifiers.control { let f = (1.0 + dy * 0.0015).clamp(0.5, 2.0); this.lightbox_zoom(window, cx, |z, lb, g| z.by(f, lb.at(e.position), g)); }
            else { this.lightbox_zoom(window, cx, |z, _, g| z.pan((dx, dy), g)); }
        }))
        // a trackpad pinch (macOS's magnify gesture, gpui's PinchEvent: #1106)
        .on_pinch(cx.listener(|this, e: &PinchEvent, window, cx| {
            let f = crate::viewers::solid::pinch_zoom(e.delta);
            this.lightbox_zoom(window, cx, |z, lb, g| z.by(f, lb.at(e.position), g));
        }));
    let btn = |id: &'static str, label: String| crate::ui::with_rect(div().id(id), move || id.replace('-', ":")).px_2().border_1().border_color(gpui::rgb(0x5a5f69)).text_color(fg).cursor_pointer().hover(|s| s.bg(gpui::rgb(0x30343b))).child(label);
    let size = lb.dims().map(|(w, h)| crate::i18n::tf("gpui.c.image.size", &[("w", w.into()), ("h", h.into())]));
    let pct = g.map(|g| format!("{}%", lb.zoom.percent(&g)));
    let state = match lb.full { Full::Loading => Some(crate::i18n::t("gpui.c.image.loading.full")), Full::Failed => Some(crate::i18n::t("gpui.c.image.full.failed")), _ => None };
    /* i18n-ok: a number with its unit */ let actual_label = "100%".to_string();
    let bar = div().id("lightbox-bar").flex().flex_wrap().items_center().justify_center().gap_2().px_4().py_2().text_size(px(12.)).text_color(fg)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(div().child(lb.shown.name.clone()))
        .children(size.map(|s| div().text_color(gpui::rgb(0x9aa0aa)).child(s)))
        .children(pct.map(|p| div().min_w(px(40.)).child(p)))
        .child(btn("lightbox-fit", crate::i18n::t("gpui.c.image.fit")).on_click(cx.listener(|this, _, window, cx| this.lightbox_zoom(window, cx, |_, _, _| Zoom::default()))))
        .child(btn("lightbox-100", actual_label).on_click(cx.listener(|this, _, window, cx| this.lightbox_zoom(window, cx, |z, _, g| z.actual((0., 0.), g)))))
        .child(btn("lightbox-out", "\u{2212}".into()).on_click(cx.listener(|this, _, window, cx| this.lightbox_zoom(window, cx, |z, _, g| z.step(false, (0., 0.), g)))))
        .child(btn("lightbox-in", "+".into()).on_click(cx.listener(|this, _, window, cx| this.lightbox_zoom(window, cx, |z, _, g| z.step(true, (0., 0.), g)))))
        .child(btn("lightbox-save", crate::i18n::trs("Save image as…")).on_click(cx.listener(|this, _, _, cx| { if let Some(s) = this.screens.lightbox.as_ref().map(|lb| lb.save()) { this.save_image(s, cx); } })))
        .children(state.map(|s| div().text_color(gpui::rgb(0x9aa0aa)).child(s)))
        .children((!lb.note.is_empty()).then(|| div().text_color(gpui::rgb(0x9aa0aa)).child(lb.note.clone())))
        .child(div().text_color(gpui::rgb(0x9aa0aa)).child(crate::i18n::t("gpui.c.image.close")));
    Some(div().id("lightbox").absolute().inset_0().occlude().track_focus(&lb.focus).key_context("Lightbox").flex().flex_col().bg(gpui::hsla(0., 0., 0., 0.88))
        .on_action(cx.listener(|this, _: &ZoomIn, window, cx| this.lightbox_zoom(window, cx, |z, _, g| z.step(true, (0., 0.), g))))
        .on_action(cx.listener(|this, _: &ZoomOut, window, cx| this.lightbox_zoom(window, cx, |z, _, g| z.step(false, (0., 0.), g))))
        .on_action(cx.listener(|this, _: &ZoomFit, window, cx| this.lightbox_zoom(window, cx, |_, _, _| Zoom::default())))
        // a press on the dimmed page (not the picture, not the bar) puts it away
        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| { this.lightbox_close(window, cx); }))
        .child(area).child(bar).into_any_element())
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
    #[test] fn a_picture_over_the_longest_side_is_kept_smaller_and_others_as_they_came() {
        let png = |w: u32, h: u32| { let mut out = Vec::new(); image::codecs::png::PngEncoder::new(&mut out).write_image(&vec![200u8; (w * h * 4) as usize], w, h, image::ExtendedColorType::Rgba8).unwrap(); Arc::new(Image::from_bytes(ImageFormat::Png, out)) };
        use image::ImageEncoder;
        let big = shrink(png(3000, 1500)); assert_eq!(dims(&big.bytes), Some((MAX_SIDE, 1024)));
        let small = png(800, 600); assert!(Arc::ptr_eq(&shrink(small.clone()), &small), "kept as it came");
    }
    use core::prelude::v1::test;
    #[test] fn formats_by_magic_bytes() {
        assert!(matches!(sniff(&[0x89, b'P', b'N', b'G', 0, 0]), Some(ImageFormat::Png))); assert!(matches!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(ImageFormat::Jpeg)));
        assert!(sniff(b"<html>not found</html>").is_none()); assert!(sniff(b"").is_none());
        assert!(matches!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(ImageFormat::Webp)));
        assert!(matches!(sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"90\" height=\"20\">"), Some(ImageFormat::Svg)));
        assert!(matches!(sniff(b"<?xml version=\"1.0\"?>\n<svg viewBox=\"0 0 400 200\">"), Some(ImageFormat::Svg)));
        assert_eq!(dims(b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"90\" height=\"20\" role=\"img\">"), Some((90, 20)));
        assert_eq!(dims(b"<svg stroke-width=\"2\" viewBox=\"0 0 400 200\" width=\"200\">"), Some((200, 100)));
        assert_eq!(dims(b"<svg><g/></svg>"), None);
        let blue = svg_raster(b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"2\"><rect width=\"4\" height=\"2\" fill=\"#1f6feb\"/></svg>").unwrap();
        assert_eq!(blue.size(0), gpui::size(gpui::DevicePixels(8), gpui::DevicePixels(4)), "drawn at twice its size");
        assert_eq!(&blue.as_bytes(0).unwrap()[..4], &[0xeb, 0x6f, 0x1f, 0xff], "BGRA: blue stays blue");
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

    /// A 4000 x 2000 picture in an 1000 x 600 area on a 2x screen: fitted, zoomed at the pointer, panned and kept on screen.
    fn g() -> Geom { Geom { pic: (4000., 2000.), area: (1000., 600.), actual: 0.5 } }
    fn close(a: f32, b: f32) -> bool { (a - b).abs() < 1e-3 }
    #[test] fn the_lightbox_fits_then_zooms_about_the_pointer() {
        let g = g();
        assert!(close(g.fit(), (1000. - 2. * FIT_MARGIN) / 4000.), "the width decides: {}", g.fit());
        let z = Zoom::default(); assert!(z.fitted()); assert_eq!(z.rect(&g), (16., 300. - 2000. * g.fit() / 2., 968., 2000. * g.fit()));
        assert!(Geom { pic: (90., 20.), area: (1000., 600.), actual: 0.5 }.fit() == 1., "a small picture is never enlarged to fit");
        // zoom in at a point right of centre: the picture's point under the pointer stays under it
        let at = (200., 50.);
        let (x0, y0, w0, _) = z.rect(&g); let u = ((500. + at.0 - x0) / w0, (300. + at.1 - y0) / w0);
        let z2 = z.by(2., at, &g); assert!(close(z2.k, 2. * g.fit()));
        let (x1, y1, w1, _) = z2.rect(&g); assert!(close((500. + at.0 - x1) / w1, u.0) && close((300. + at.1 - y1) / w1, u.1), "{z2:?}");
        // zoomed out to (or past) the fit: fitted again, centred
        assert_eq!(z2.by(0.25, at, &g), Zoom::default());
        // never past the limits
        assert!(close(z.by(1e6, (0., 0.), &g).k, g.actual * MAX_ZOOM)); assert!(!z.by(f32::NAN, at, &g).k.is_nan());
        // a tiny picture still zooms past its area (twice what fills it), so it can be moved about
        let tiny = Geom { pic: (64., 48.), area: (1104., 700.), actual: 1. }; let k = Zoom::default().by(1e6, (0., 0.), &tiny).k;
        assert!(close(k, 2. * 1104. / 64.), "{k}"); assert!(Zoom::default().by(1e6, (0., 0.), &tiny).pannable(&tiny));
        assert_eq!(z.by(1., at, &g), z);
    }
    #[test] fn a_zoomed_picture_pans_and_stays_on_screen() {
        let g = g();
        assert_eq!(Zoom::default().pan((50., 50.), &g), Zoom::default(), "a fitted picture does not move");
        let z = Zoom::default().actual((0., 0.), &g); assert!(close(z.k, 0.5) && z.pannable(&g)); // 2000 x 1000 px in 1000 x 600
        let p = z.pan((100., -30.), &g); assert_eq!(p.off, (100., -30.));
        let far = z.pan((1e5, -1e5), &g); assert_eq!(far.off, (500., -200.), "no gap at an edge: (2000-1000)/2, (1000-600)/2");
        let (x, y, w, h) = far.rect(&g); assert!(close(x, 0.) && close(y + h, 600.) && close(w, 2000.));
        // a picture narrower than the area along a side stays centred there
        let tall = Geom { pic: (500., 4000.), area: (1000., 600.), actual: 1. };
        let t = Zoom::default().actual((0., 0.), &tall).pan((300., 300.), &tall); assert_eq!(t.off.0, 0.); assert_eq!(t.off.1, 300.);
        // the window shrinks while zoomed: the picture is held on screen when drawn
        let small = Geom { area: (1800., 900.), ..g }; let (x, _, _, _) = far.rect(&small); assert!(close(x, 1800. / 2. + 100. - 1000.), "{x}");
    }
    #[test] fn steps_stop_at_actual_size_and_at_the_fit_and_a_double_click_toggles() {
        let g = g(); let fit = g.fit();
        // stepping in from the fit (0.242) reaches 100% (0.5) exactly on the way
        let mut z = Zoom::default(); let mut ks = vec![];
        for _ in 0..5 { z = z.step(true, (0., 0.), &g); ks.push(z.k); }
        assert!(ks.iter().any(|k| close(*k, 0.5)), "{ks:?}"); assert!(close(ks[0], fit * STEP));
        assert_eq!(z.percent(&g), (z.k / 0.5 * 100.).round() as u32);
        // and stepping out lands on the fit, fitted
        let mut z = Zoom::default().actual((0., 0.), &g); for _ in 0..10 { z = z.step(false, (0., 0.), &g); } assert_eq!(z, Zoom::default());
        // a double click: fit -> 100% at the pointer, anything else -> fit
        let a = Zoom::default().toggle((100., 0.), &g); assert!(close(a.k, 0.5)); assert_eq!(a.percent(&g), 100); assert_eq!(a.toggle((0., 0.), &g), Zoom::default());
        // a small picture on a 1x screen: its fit is its actual size, a double click doubles it
        let s = Geom { pic: (300., 200.), area: (1000., 600.), actual: 1. }; assert!(close(Zoom::default().toggle((0., 0.), &s).k, 2.));
        // a small picture on a 2x screen: 100% is smaller than the fit, and zooming out reaches it
        let s2 = Geom { actual: 0.5, ..s }; let mut z = Zoom::default(); for _ in 0..6 { z = z.step(false, (0., 0.), &s2); } assert_eq!(z.percent(&s2), 100);
    }
    #[test] fn save_names_keep_the_files_name_and_add_its_kind() {
        assert_eq!(save_name("render.png", ImageFormat::Png), "render.png");
        assert_eq!(save_name("/w/boards/top view.jpeg", ImageFormat::Jpeg), "top view.jpeg");
        assert_eq!(save_name("Board render", ImageFormat::Png), "Board render.png");
        assert_eq!(save_name("", ImageFormat::Jpeg), "image.jpg"); assert_eq!(save_name("a:b?.png", ImageFormat::Png), "a_b_.png");
        assert_eq!(save_name(".hidden", ImageFormat::Webp), "hidden.webp"); assert_eq!(save_name("v1.2 plot", ImageFormat::Svg), "v1.2 plot.svg");
    }
    /// A 3000 x 1500 picture comes through the cache at 2048 px: the cache says so, and Save image as writes the 3000 px file the hub serves,
    /// byte for byte; a picture kept as it came is written from the copy in hand, with no second fetch.
    #[test] fn saving_writes_the_original_not_the_smaller_copy() {
        use image::ImageEncoder;
        let mut big = Vec::new(); image::codecs::png::PngEncoder::new(&mut big).write_image(&vec![90u8; 3000 * 1500 * 4], 3000, 1500, image::ExtendedColorType::Rgba8).unwrap();
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let addr = l.local_addr().unwrap().to_string();
        let served = big.clone(); let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0)); let h2 = hits.clone();
        std::thread::spawn(move || { for c in l.incoming() { let Ok(mut s) = c else { continue }; h2.fetch_add(1, std::sync::atomic::Ordering::SeqCst); let mut b = [0u8; 2048]; let _ = s.read(&mut b);
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", served.len()); let _ = s.write_all(&served); } });
        let t = Target { addr, token: "t".into(), tls: false }; let url = "/message-image?entry=e&path=%2Fbig.png";
        let im = Images::default(); im.frame(); let _ = im.get(Some(&t), url);
        for _ in 0..400 { if !im.loading() { break; } std::thread::sleep(Duration::from_millis(25)); }
        let Slot::Ready(kept) = im.get(Some(&t), url) else { panic!("not fetched") };
        assert_eq!(dims(&kept.bytes), Some((MAX_SIDE, 1024)), "kept smaller"); assert_eq!(im.shrunk(url), Some((3000, 1500)));
        let save = Save { img: kept.clone(), name: "big.png".into(), src: Some((t.clone(), url.into())), shrunk: im.shrunk(url).is_some() };
        let dir = crate::tmp::dir("img-save");
        let out = dir.join(save_name(&save.name, kept.format)); save_to(&save, &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), big, "the file as the hub has it, at 3000 x 1500");
        // as it came: no fetch, the bytes in hand
        let before = hits.load(std::sync::atomic::Ordering::SeqCst);
        let small = Save { img: Arc::new(Image::from_bytes(ImageFormat::Png, vec![1, 2, 3])), name: "s".into(), src: Some((t, url.into())), shrunk: false };
        assert_eq!(original_bytes(&small).unwrap(), vec![1, 2, 3]); assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), before);
        // kept smaller with no hub to ask: refused, never the smaller copy
        assert!(original_bytes(&Save { src: None, shrunk: true, ..small }).is_err());
    }
}

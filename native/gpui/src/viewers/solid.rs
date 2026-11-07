//! 3D / Mech: the hub's `solid` frame (CBOR {key, n, mesh, box3, none, ...}; mesh = varints: per triangle the colour XOR the previous one's, then
//! three vertices as zigzag deltas of x y z in 10 micrometres, board axes, z up). Decoding and drawing run on a worker thread, on the GPU (solid_gpu.rs: the model is uploaded once)
//! or else on the cores (raster.rs); the page only receives finished images, so input never waits for a model. The latest request wins, but a picture of a drag is never given up
//! part-way: a mouse moves faster than a picture is drawn, and giving up on each one for the next leaves the screen still until the pointer stops. The camera is the
//! product's free orbit (solid.js `Orbit`): the model follows the pointer in any direction, the wheel zooms at the pointer between 1/60 and 20 times
//! the fitted distance, a right/middle or shift drag moves it. The image is as big as the panel in device pixels. Nothing repaints while idle.
use gpui::{prelude::*, *};
use super::raster::{self, render};
#[cfg(test)] use super::raster::BG;
use std::sync::{atomic::{AtomicU64, Ordering}, Arc, Condvar, Mutex};

pub const MAX_TRIANGLES: usize = (64 << 20) / 84;
pub const MAX_PIXELS: usize = 2880 * 1800;
/// the most pixels a draft covers: beyond it the draft is drawn smaller and stretched
const DRAFT_PIXELS: usize = 1600 * 1000;
/// Samples a pixel a side: 1 for a draft, then as many as the panel's size leaves cheap (3 x 3 up to ~0.5 MP, 2 x 2 up to ~3 MP, none beyond).
pub fn samples(w: usize, h: usize, draft: bool) -> usize { if draft { 1 } else if w * h <= 520_000 { 3 } else if w * h <= 3_000_000 { 2 } else { 1 } }
pub(super) const FOV: f32 = 35.0 * std::f32::consts::PI / 180.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh { pub tris: Vec<[f32; 9]>, pub normals: Vec<[f32; 9]>, pub colors: Vec<u32>, pub center: [f32; 3], pub radius: f32 }

struct Varints<'a> { b: &'a [u8], at: usize, valid: bool }
impl<'a> Varints<'a> {
    fn new(b: &'a [u8]) -> Self { Varints { b, at: 0, valid: true } }
    fn next(&mut self) -> u32 {
        let (mut v, mut shift) = (0u32, 0u32);
        loop {
            let Some(&byte) = self.b.get(self.at) else { self.valid = false; return 0 };
            self.at += 1;
            v |= ((byte & 0x7f) as u32).checked_shl(shift).unwrap_or(0);
            if byte & 0x80 == 0 { return v }
            shift += 7; if shift > 35 { self.valid = false; return 0 }
        }
    }
    fn signed(&mut self) -> i32 { let u = self.next(); ((u >> 1) as i32) ^ -((u & 1) as i32) }
    fn more(&self) -> bool { self.at < self.b.len() }
}

/// Mesh from the frame's `n` and `mesh` bytes. Errors when the bytes are not exactly `n` triangles. Coordinates wrap like the encoder's U32 deltas.
pub fn decode_mesh(n: usize, bytes: &[u8]) -> Result<Mesh, String> {
    if n == 0 || n > MAX_TRIANGLES || bytes.len() < n * 10 { return Err("Invalid or oversized model".into()) }
    let mut v = Varints::new(bytes);
    let (mut color, mut p) = (0u32, [0i32; 3]);
    let (mut tris, mut colors, mut keys) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n * 3));
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for _ in 0..n {
        color ^= v.next();
        let mut t = [0f32; 9];
        for k in 0..3 {
            // 10 micrometres -> millimetres; the wire is board axes (x right, y down, z up), the orbit is right-handed: (x, -y, z), as the web's `mesh()` does (without it silk text reads mirrored)
            for a in 0..3 { p[a] = p[a].wrapping_add(v.signed()); t[3 * k + a] = if a == 1 { -(p[a] as f32 * 0.01) } else { p[a] as f32 * 0.01 }; }
            for a in 0..3 { lo[a] = lo[a].min(t[3 * k + a]); hi[a] = hi[a].max(t[3 * k + a]); }
            keys.push(p);
        }
        if !v.valid { return Err("Incomplete model".into()) }
        tris.push(t); colors.push(color);
    }
    if v.more() || !v.valid { return Err("Unexpected model bytes".into()) }
    let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let radius = (0..3).map(|a| (hi[a] - lo[a]) * (hi[a] - lo[a])).sum::<f32>().sqrt() / 2.0;
    let normals = raster::smooth_normals(&tris, &keys);
    Ok(Mesh { tris, normals, colors, center, radius: radius.max(1e-3) })
}

// ---- the frame: a minimal CBOR map reader (text keys, or the wire dictionary's integer keys; byte strings) ------
#[derive(Debug, Clone, PartialEq)]
pub enum Item { Int(i64), Text(String), Bytes(Vec<u8>), Array(Vec<Item>), Other }
fn head(b: &[u8], at: &mut usize) -> Option<(u8, u64)> {
    let h = *b.get(*at)?; *at += 1; let (major, ai) = (h >> 5, h & 31);
    let n = match ai { 0..=23 => ai as u64, 24 => { let v = *b.get(*at)? as u64; *at += 1; v } 25 => { let v = u16::from_be_bytes(b.get(*at..*at + 2)?.try_into().ok()?) as u64; *at += 2; v }
        26 => { let v = u32::from_be_bytes(b.get(*at..*at + 4)?.try_into().ok()?) as u64; *at += 4; v } 27 => { let v = u64::from_be_bytes(b.get(*at..*at + 8)?.try_into().ok()?); *at += 8; v } _ => return None };
    Some((major, n))
}
fn item(b: &[u8], at: &mut usize, depth: usize) -> Option<Item> {
    if depth > 8 { return None }
    let (major, n) = head(b, at)?;
    Some(match major {
        0 => Item::Int(n as i64), 1 => Item::Int(-1 - n as i64),
        2 => { let end = at.checked_add(n as usize)?; let v = b.get(*at..end)?.to_vec(); *at = end; Item::Bytes(v) }
        3 => { let end = at.checked_add(n as usize)?; let v = std::str::from_utf8(b.get(*at..end)?).ok()?.to_string(); *at = end; Item::Text(v) }
        4 => { if n > 1 << 20 { return None } let mut a = Vec::new(); for _ in 0..n { a.push(item(b, at, depth + 1)?); } Item::Array(a) }
        6 => return item(b, at, depth + 1), // a tag has one value: the tag number was the head's argument (as wire.rs reads it)
        _ => Item::Other,
    })
}
/// Step over one CBOR item without building it (byte strings are not copied).
fn skip(b: &[u8], at: &mut usize, depth: usize) -> Option<()> {
    if depth > 8 { return None }
    let (major, n) = head(b, at)?;
    match major {
        2 | 3 => { *at = at.checked_add(n as usize)?; if *at > b.len() { return None } }
        4 => { for _ in 0..n { skip(b, at, depth + 1)?; } }
        5 => { for _ in 0..n.checked_mul(2)? { skip(b, at, depth + 1)?; } }
        6 => { skip(b, at, depth + 1)?; }
        _ => {}
    }
    Some(())
}
/// The frame's `key` text only: the map is walked and every other value skipped, so a large `mesh` is not copied.
pub fn peek_key(b: &[u8]) -> Option<String> {
    let mut at = 0; let (major, n) = head(b, &mut at)?; if major != 5 || n > 256 { return None }
    for _ in 0..n {
        let name = match item(b, &mut at, 1)? { Item::Text(t) => t, Item::Int(i) => crate::wire::KEYS.get(i as usize)?.to_string(), _ => return None };
        if name == "key" { return match item(b, &mut at, 1)? { Item::Text(t) => Some(t), _ => None } }
        skip(b, &mut at, 1)?;
    }
    None
}
/// The frame's map as (key, value) pairs; integer keys go through the wire dictionary.
pub fn read_frame(b: &[u8]) -> Option<Vec<(String, Item)>> {
    let mut at = 0; let (major, n) = head(b, &mut at)?; if major != 5 || n > 256 { return None }
    let mut out = Vec::new();
    for _ in 0..n {
        let key = match item(b, &mut at, 1)? { Item::Text(t) => t, Item::Int(i) => crate::wire::KEYS.get(i as usize)?.to_string(), _ => return None };
        let val = item(b, &mut at, 1)?; out.push((key, val));
    }
    Some(out)
}
#[derive(Debug, PartialEq)]
pub enum SolidFrame { Model(String, Mesh), /// why there is no model (the hub's `none`), the previous model stays
    Note(String, String), Bad(String) }
fn is_solid_key(k: &str) -> bool { k.starts_with("2|") || k.starts_with("3|") || k.starts_with("4|") }
/// A solid frame as the page uses it; None when the bytes are not a solid frame at all.
pub fn parse_frame(b: &[u8]) -> Option<SolidFrame> {
    let f = read_frame(b)?;
    let get = |k: &str| f.iter().find(|(n, _)| n == k).map(|(_, v)| v);
    let key = match get("key") { Some(Item::Text(k)) => k.clone(), _ => return None };
    if !is_solid_key(&key) { return None }
    if let Some(Item::Text(why)) = get("none") { return Some(SolidFrame::Note(key, why.clone())) }
    let Some(Item::Bytes(mesh)) = get("mesh") else { return None }; // a note about the parts is not a model
    let n = match get("n") { Some(Item::Int(n)) if *n > 0 => *n as usize, _ => return Some(SolidFrame::Bad("missing triangle count".into())) };
    Some(match decode_mesh(n, mesh) { Ok(m) => SolidFrame::Model(key, m), Err(e) => SolidFrame::Bad(e) })
}

// ---- the camera: free orbit ------------------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera { pub right: [f32; 3], pub up: [f32; 3], pub fwd: [f32; 3], pub target: [f32; 3], pub dist: f32, pub fit: f32 }
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn mul(a: [f32; 3], s: f32) -> [f32; 3] { [a[0] * s, a[1] * s, a[2] * s] }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn norm(a: [f32; 3]) -> [f32; 3] { let l = dot(a, a).sqrt(); if l > 0.0 { mul(a, 1.0 / l) } else { [0.0, 0.0, 1.0] } }
/// `v` turned by `angle` about the unit `axis` (Rodrigues).
fn rotate(v: [f32; 3], axis: [f32; 3], angle: f32) -> [f32; 3] {
    let (s, c) = angle.sin_cos(); add(add(mul(v, c), mul(cross(axis, v), s)), mul(axis, dot(axis, v) * (1.0 - c)))
}
impl Camera {
    /// Looking at the model from the front-right and above, fitted to its bounding sphere.
    pub fn fitted(mesh: &Mesh) -> Camera { Camera::fitted_for(mesh, 1.0) }
    /// The same, for a panel `aspect` (width / height) wide: the field of view is the vertical one, so a panel narrower than it is tall needs the model further away to fit across.
    pub fn fitted_for(mesh: &Mesh, aspect: f32) -> Camera {
        let aspect = if aspect.is_finite() && aspect > 0.05 { aspect } else { 1.0 };
        let fit = mesh.radius / (FOV / 2.0).sin() * 1.05 / aspect.min(1.0);
        let (yaw, pitch) = (0.6f32, 0.7f32);
        let eye_dir = [pitch.cos() * yaw.sin(), -pitch.cos() * yaw.cos(), pitch.sin()];
        let fwd = mul(eye_dir, -1.0); let right = norm(cross(fwd, [0.0, 0.0, 1.0])); let up = cross(right, fwd);
        Camera { right, up, fwd, target: mesh.center, dist: fit, fit }
    }
    /// A standard view of the same model at the same distance (the web's toolbar: Top, Bottom, Front, Right, Back, Left; anything else, Iso, is the fitted view).
    /// Z is up and the part's front faces -Y, so Front looks along +Y from the -Y side.
    pub fn named(&self, name: &str) -> Camera {
        let (fwd, up) = match name {
            "front" => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), "back" => ([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
            "right" => ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), "left" => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            "top" => ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]), "bottom" => ([0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
            _ => return Camera { target: self.target, dist: self.fit, ..*self },
        };
        let mut c = Camera { right: [1.0, 0.0, 0.0], up, fwd, target: self.target, dist: self.fit, fit: self.fit };
        c.reortho(); c
    }
    pub fn eye(&self) -> [f32; 3] { sub(self.target, mul(self.fwd, self.dist)) }
    fn reortho(&mut self) { self.fwd = norm(self.fwd); self.right = norm(cross(self.fwd, self.up)); self.up = cross(self.right, self.fwd); }
    /// A drag of (dx, dy) pixels: the model follows the pointer, about the target, in any direction (no pole, no clamp).
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        let k = 0.008; // radians per pixel
        let (ar, au) = (self.right, self.up);
        // the scene turns about the view's vertical axis with a horizontal drag, about the horizontal axis with a vertical one
        for v in [&mut self.right, &mut self.fwd, &mut self.up] { *v = rotate(*v, au, dx * k); }
        let ar2 = rotate(ar, au, dx * k);
        for v in [&mut self.up, &mut self.fwd, &mut self.right] { *v = rotate(*v, ar2, dy * k); }
        self.reortho();
    }
    /// alt drag: roll about the view axis.
    pub fn roll(&mut self, dx: f32) { let (f, a) = (self.fwd, dx * 0.008); self.right = rotate(self.right, f, a); self.up = rotate(self.up, f, a); self.reortho(); }
    /// Move the view: a right/middle or shift drag of (dx, dy) pixels in a panel `h` pixels high.
    pub fn pan(&mut self, dx: f32, dy: f32, h: f32) {
        let per_px = 2.0 * (FOV / 2.0).tan() * self.dist / h.max(1.0);
        self.target = add(self.target, add(mul(self.right, -dx * per_px), mul(self.up, dy * per_px)));
    }
    /// Zoom by `steps` wheel notches at the pointer (nx, ny in -1..1, y up; aspect = width / height): the point under it stays under it.
    pub fn zoom_at(&mut self, steps: f32, nx: f32, ny: f32, aspect: f32) {
        let t = (FOV / 2.0).tan();
        let d2 = (self.dist / 1.1f32.powf(steps)).clamp(self.fit / 60.0, self.fit * 20.0);
        let under = |cam: &Camera, d: f32| add(cam.target, add(mul(cam.right, nx * aspect * t * d), mul(cam.up, ny * t * d)));
        let p = under(self, self.dist);
        self.dist = d2;
        self.target = sub(p, add(mul(self.right, nx * aspect * t * d2), mul(self.up, ny * t * d2)));
    }
}

pub fn to_image(bgra: Vec<u8>, w: usize, h: usize) -> Option<Arc<RenderImage>> {
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, bgra)?; // gpui takes BGRA in this container
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buf)])))
}

// ---- the worker: latest request wins -------------------------------------------------------------------------------
enum Msg { Mesh(SolidFrame), Image { ticket: u64, image: Arc<RenderImage>, ms: f64, tris: usize, w: usize, h: usize, by: &'static str } }
/// What to render: the model, the camera, the size, the samples a pixel a side, the ticket, whether it is a draft (the model is moving).
type Job = (Arc<Mesh>, Camera, usize, usize, usize, u64, bool);
/// Whether a render in progress goes on: a draft does, or a drag would show nothing at all; a picture at rest gives way to anything newer.
pub fn wanted(draft: bool, my: u64, newest: u64) -> bool { draft || my == newest }
/// Whether a finished picture goes on the screen: it is of this model (`floor`) and newer than the one there. Pictures of a drag finish behind the pointer: that is no reason to drop them.
pub fn shows(ticket: u64, shown: u64, floor: u64) -> bool { ticket >= floor && ticket > shown }
/// What draws: the process's GPU (solid_gpu::shared), or the cores when there is none or it fails.
#[derive(Default)] struct Drawer;
impl Drawer {
    fn draw(&mut self, mesh: &Arc<Mesh>, cam: &Camera, w: usize, h: usize, ss: usize, alive: &(dyn Fn() -> bool + Sync)) -> Option<(Vec<u8>, &'static str)> {
        #[cfg(target_os = "linux")] {
            let mut gpu = super::solid_gpu::shared().lock().unwrap_or_else(|e| e.into_inner());
            if let Some(g) = gpu.as_mut() { match g.render(mesh, cam, w, h) { Some(px) => return Some((px, "gpu")), None => { log::warn!("3D page: the GPU failed a picture, drawing on the CPU from now on"); std::mem::forget(gpu.take()); } } }
        }
        render(mesh, cam, w, h, ss, alive).map(|px| (px, "cpu"))
    }
}
#[derive(Default)] struct Pending { frame: Option<Vec<u8>>, render: Option<Job>, shutdown: bool }
struct Worker { slot: Arc<(Mutex<Pending>, Condvar)>, ticket: Arc<AtomicU64> }
impl Worker {
    fn spawn(tx: async_channel::Sender<Msg>) -> Worker {
        let slot: Arc<(Mutex<Pending>, Condvar)> = Arc::new((Mutex::new(Pending::default()), Condvar::new()));
        let ticket = Arc::new(AtomicU64::new(0));
        let (s2, g2) = (slot.clone(), ticket.clone());
        std::thread::Builder::new().name("solid-render".into()).spawn(move || { let mut drawer = Drawer::default(); loop {
            let job = { let (m, cv) = &*s2; let mut p = m.lock().unwrap();
                while p.frame.is_none() && p.render.is_none() && !p.shutdown { p = cv.wait(p).unwrap(); }
                if p.shutdown { return } (p.frame.take(), p.render.take()) };
            if let (Some(bytes), _) = &job { if let Some(f) = parse_frame(bytes) { if tx.send_blocking(Msg::Mesh(f)).is_err() { return } } }
            if let (_, Some((mesh, cam, w, h, ss, my, draft))) = job {
                let t0 = std::time::Instant::now();
                let g = g2.clone();
                if let Some((px, by)) = drawer.draw(&mesh, &cam, w, h, ss, &move || wanted(draft, my, g.load(Ordering::Relaxed))) {
                    if let Some(image) = to_image(px, w, h) { if tx.send_blocking(Msg::Image { ticket: my, image, ms: t0.elapsed().as_secs_f64() * 1000.0, tris: mesh.tris.len(), w, h, by }).is_err() { return } }
                }
            }
        } }).ok();
        Worker { slot, ticket }
    }
    fn submit_frame(&self, bytes: Vec<u8>) { let (m, cv) = &*self.slot; m.lock().unwrap().frame = Some(bytes); cv.notify_one(); }
    /// Ask for an image of `mesh`, replacing any request not yet begun.
    fn submit_render(&self, mesh: Arc<Mesh>, cam: Camera, w: usize, h: usize, ss: usize, draft: bool) -> u64 {
        let ticket = self.ticket.fetch_add(1, Ordering::Relaxed) + 1;
        let (m, cv) = &*self.slot; m.lock().unwrap().render = Some((mesh, cam, w, h, ss, ticket, draft)); cv.notify_one(); ticket
    }
}
impl Drop for Worker { fn drop(&mut self) { let (m, cv) = &*self.slot; m.lock().unwrap().shutdown = true; self.ticket.fetch_add(1, Ordering::Relaxed); cv.notify_one(); } }

// ---- the page ---------------------------------------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq)] enum Drag { Orbit, Pan, Roll }
/// What the demo/metrics read: how many images arrived, the last render's time.
#[derive(Clone, Debug, Default)] pub struct Stats { pub images: u64, pub painted: u64, pub last_ms: f64, pub last_tris: usize, pub last_size: (usize, usize), pub by: &'static str }

/// A note (no model for this source) keeps the page's model only when it is the same source's: a different key drops it.
/// Whether the panel's shape differs enough from the one the camera was fitted for to fit it again.
pub fn aspect_changed(was: f32, now: f32) -> bool { was <= 0.0 || (now / was - 1.0).abs() > 0.03 }
pub fn note_keeps_model(page_key: &str, note_key: &str) -> bool { page_key == note_key }

pub struct SolidPage {
    mesh: Option<Arc<Mesh>>, key: String, cam: Option<Camera>, touched: bool, fit_aspect: f32, image: Option<Arc<RenderImage>>, retired: Vec<Arc<RenderImage>>, drag: Option<(Drag, Point<Pixels>)>,
    pub note: String, worker: Worker, size: Arc<Mutex<[f32; 5]>> /* panel width, height, scale factor, origin x, y (logical px) */, asked: (usize, usize), /** the newest ticket asked for, the one on screen, the first of the model shown */ latest: u64, on_screen: u64, floor: u64, counted: u64, pub stats: Stats,
}
impl SolidPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let (tx, rx) = async_channel::bounded::<Msg>(4);
        cx.spawn(async move |weak, cx| {
            while let Ok(msg) = rx.recv().await {
                if weak.update(cx, |this, cx| this.apply(msg, cx)).is_err() { break }
            }
        }).detach();
        SolidPage { mesh: None, key: String::new(), cam: None, touched: false, fit_aspect: 0.0, image: None, retired: Vec::new(), drag: None, note: "No 3D model yet".into(), worker: Worker::spawn(tx),
            size: Arc::new(Mutex::new([0.0, 0.0, 1.0, 0.0, 0.0])), asked: (0, 0), latest: 0, on_screen: 0, floor: 0, counted: 0, stats: Stats::default() }
    }
    /// The key of the model the page holds now (set when a decoded frame is applied, not when one is queued): "" before any.
    pub fn key(&self) -> &str { &self.key }
    /// A plot-lane binary frame (keys 2|, 3|, 4| are solids).
    /// None: taken (a solid frame, decoded on the worker; the bytes are moved, not copied). Some(bytes): not a solid frame, handed back unread.
    pub fn accept(&mut self, bytes: Vec<u8>) -> Option<Vec<u8>> {
        if peek_key(&bytes).map_or(false, |k| is_solid_key(&k)) { self.worker.submit_frame(bytes); None } else { Some(bytes) }
    }
    fn apply(&mut self, msg: Msg, cx: &mut Context<Self>) {
        match msg {
            Msg::Mesh(SolidFrame::Model(key, mesh)) => {
                let fresh = key != self.key || self.cam.is_none();
                self.key = key; let mesh = Arc::new(mesh); self.floor = self.latest + 1;
                if fresh { let [pw, ph, _, _, _] = *self.size.lock().unwrap(); self.fit_aspect = pw / ph.max(1.0); self.touched = false; self.cam = Some(Camera::fitted_for(&mesh, self.fit_aspect)); }
                self.mesh = Some(mesh); self.note.clear(); self.ask_render(false); cx.notify();
            }
            Msg::Mesh(SolidFrame::Note(key, why)) => { // why there is no model; the same source's previous model stays shown
                if !note_keeps_model(&self.key, &key) {
                    // another source's model could not be made: the old source's model must not show under this key
                    self.mesh = None; self.cam = None; if let Some(i) = self.image.take() { self.retired.push(i); }
                    self.floor = self.latest + 1; // a render of the old model still in flight is stale
                }
                self.key = key; self.note = if self.mesh.is_some() { format!("{why} — previous saved revision shown") } else { why }; cx.notify();
            }
            Msg::Mesh(SolidFrame::Bad(e)) => { self.note = e; cx.notify(); }
            Msg::Image { ticket, image, ms, tris, w, h, by } => {
                if !shows(ticket, self.on_screen, self.floor) { return } // never painted, so never in the atlas: nothing to give back
                self.on_screen = ticket;
                if let Some(old) = self.image.replace(image) { self.retired.push(old); }
                self.stats = Stats { images: self.stats.images + 1, painted: self.stats.painted, last_ms: ms, last_tris: tris, last_size: (w, h), by }; cx.notify();
            }
        }
    }
    /// Request an image at the panel's device-pixel size (half size for a draft), within MAX_PIXELS.
    fn ask_render(&mut self, draft: bool) {
        let (Some(mesh), Some(cam)) = (&self.mesh, self.cam) else { return };
        let [pw, ph, sf, _, _] = *self.size.lock().unwrap();
        let (mut w, mut h) = (((pw * sf) as usize).max(64), ((ph * sf) as usize).max(48));
        if w * h > MAX_PIXELS { let k = (MAX_PIXELS as f32 / (w * h) as f32).sqrt(); w = (w as f32 * k) as usize; h = (h as f32 * k) as usize; }
        // a draft while moving: one sample a pixel, and half size only on a very large panel; the rest at rest: smooth outlines
        if draft && w * h > DRAFT_PIXELS { let k = (DRAFT_PIXELS as f32 / (w * h) as f32).sqrt(); w = ((w as f32 * k) as usize).max(32); h = ((h as f32 * k) as usize).max(24); }
        self.asked = (w, h);
        self.latest = self.worker.submit_render(mesh.clone(), cam, w, h, samples(w, h, draft), draft);
    }
    /// The toolbar's Fit (the first fitted view again, camera untouched) and its standard views.
    pub fn set_view(&mut self, name: &str) {
        let Some(m) = self.mesh.clone() else { return };
        let fitted = Camera::fitted_for(&m, self.fit_aspect.max(0.05));
        if name == "fit" { self.cam = Some(fitted); self.touched = false; }
        else { self.cam = Some(fitted.named(name)); self.touched = true; }
        self.ask_render(false);
    }
    /// Snap view: the picture on screen as a PNG in `dir` (never over a file); the path, or why not.
    pub fn snap(&mut self, dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
        let img = self.image.clone().ok_or("no picture yet")?;
        let (w, h) = { let s = img.size(0); (s.width.0 as u32, s.height.0 as u32) };
        let mut px = img.as_bytes(0).ok_or("no picture yet")?.to_vec();
        for p in px.chunks_exact_mut(4) { p.swap(0, 2); } // the container holds BGRA
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let mut path = dir.join(format!("backplane-3d-{stamp}.png")); let mut n = 1;
        while path.exists() { path = dir.join(format!("backplane-3d-{stamp}-{n}.png")); n += 1; }
        image::RgbaImage::from_raw(w, h, px).ok_or("bad picture")?.save(&path).map_err(|e| e.to_string())?;
        Ok(path)
    }
    /// For the demo and tests: an orbit drag step as the mouse handler does it.
    pub fn drag_by(&mut self, dx: f32, dy: f32) { if let Some(c) = &mut self.cam { c.orbit(dx, dy); } self.ask_render(true); }
    /// For the demo: the button went up, the full picture is asked for.
    pub fn settle(&mut self) { self.ask_render(false); }
    /// Give the images back to the atlas (call before dropping the page, with the window).
    pub fn release(&mut self, window: &mut Window) { for i in self.retired.drain(..).chain(self.image.take()) { let _ = window.drop_image(i); } }
}
impl Render for SolidPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // images no longer shown leave the sprite atlas (gpui keeps a painted RenderImage there until it is dropped)
        for old in self.retired.drain(..) { let _ = window.drop_image(old); }
        if self.stats.images != self.counted { self.counted = self.stats.images; self.stats.painted += 1; }
        let size = self.size.clone();
        let probe = canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, _| {
            let new = [f32::from(b.size.width), f32::from(b.size.height), window.scale_factor(), f32::from(b.origin.x), f32::from(b.origin.y)];
            let mut g = size.lock().unwrap();
            if *g != new { *g = new; drop(g); window.refresh(); } // the panel changed size or scale: ask for the next frame once, which re-renders at the new size
        }).absolute().size_full();
        // a panel whose size differs from the last request re-renders at the right size
        let [pw, ph, sf, _, _] = *self.size.lock().unwrap();
        // the model was fitted before the panel's size was known: an untouched camera is fitted again to the real shape
        if let Some(m) = self.mesh.clone() { if pw > 0.0 && ph > 0.0 && !self.touched && aspect_changed(self.fit_aspect, pw / ph) { self.fit_aspect = pw / ph; self.cam = Some(Camera::fitted_for(&m, self.fit_aspect)); self.ask_render(false); } }
        if self.mesh.is_some() && pw > 0.0 { let want = (((pw * sf) as usize).max(64), ((ph * sf) as usize).max(48)); if self.drag.is_none() && want.0 * want.1 <= MAX_PIXELS && want != self.asked && self.asked.0 * 2 != want.0 { self.ask_render(false); } }
        let body = match &self.image {
            Some(i) => img(i.clone()).size_full().object_fit(ObjectFit::Fill).into_any_element(),
            None => div().size_full().flex().items_center().justify_center().text_color(rgb(0x8a93a3)).child(self.note.clone()).into_any_element(),
        };
        let tools = (self.mesh.is_some()).then(|| {
            let mut bar = div().absolute().top_2().left_2().flex().flex_wrap().gap_1();
            for (label, view) in [("Fit", "fit"), ("Iso", "iso"), ("Top", "top"), ("Bottom", "bottom"), ("Front", "front"), ("Right", "right"), ("Back", "back"), ("Left", "left"), ("Snap view", "snap")] {
                bar = bar.child(div().id(SharedString::from(format!("solid-{view}"))).px_2().py_1().text_size(px(12.)).text_color(rgb(0xc8ced9)).bg(rgba(0x1d2430dd)).border_1().border_color(rgb(0x3a4456)).cursor_pointer()
                    .hover(|d| d.bg(rgba(0x2a3548ee))).child(label)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if view == "snap" {
                            let dir = std::env::var_os("BACKPLANE_GPUI_SNAP_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| crate::plat::user_home().map(|h| h.join("Pictures/Backplane")).unwrap_or_default());
                            this.note = match this.snap(&dir) { Ok(p) => format!("Saved {}", p.display()), Err(e) => format!("Snap view failed: {e}") };
                        } else { this.set_view(view); }
                        cx.notify();
                    })));
            }
            bar
        });
        div().id("solid-page").size_full().overflow_hidden().relative().child(body).child(probe).children(tools)
            .when(self.image.is_some() && !self.note.is_empty(), |d| d.child(div().absolute().bottom_2().left_2().px_2().text_size(px(12.)).text_color(rgb(0x8a93a3)).child(self.note.clone())))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, _| { this.drag = Some((if e.modifiers.alt { Drag::Roll } else if e.modifiers.shift || e.modifiers.control { Drag::Pan } else { Drag::Orbit }, e.position)); }))
            .on_mouse_down(MouseButton::Right, cx.listener(|this, e: &MouseDownEvent, _, _| { this.drag = Some((Drag::Pan, e.position)); }))
            .on_mouse_down(MouseButton::Middle, cx.listener(|this, e: &MouseDownEvent, _, _| { this.drag = Some((Drag::Pan, e.position)); }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                let Some((kind, last)) = this.drag else { return };
                if e.pressed_button.is_none() { this.drag = None; this.ask_render(false); cx.notify(); return } // the button went up where we could not see it
                let (dx, dy) = (f32::from(e.position.x - last.x), f32::from(e.position.y - last.y));
                let h = this.size.lock().unwrap()[1];
                this.touched = true; if let Some(c) = &mut this.cam { match kind { Drag::Orbit => c.orbit(dx, dy), Drag::Pan => c.pan(dx, dy, h), Drag::Roll => c.roll(dx) } }
                this.drag = Some((kind, e.position)); this.ask_render(true); // a draft while dragging, the full image on release. No repaint here: the picture's arrival is what repaints
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| { if this.drag.take().is_some() { this.ask_render(false); cx.notify(); } }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| { if this.drag.take().is_some() { this.ask_render(false); cx.notify(); } }))
            .on_mouse_up(MouseButton::Right, cx.listener(|this, _: &MouseUpEvent, _, cx| { if this.drag.take().is_some() { this.ask_render(false); cx.notify(); } }))
            .on_mouse_up(MouseButton::Middle, cx.listener(|this, _: &MouseUpEvent, _, cx| { if this.drag.take().is_some() { this.ask_render(false); cx.notify(); } }))
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, window, cx| {
                let steps = match e.delta { ScrollDelta::Pixels(p) => f32::from(p.y) / 20.0, ScrollDelta::Lines(l) => l.y };
                let [pw, ph, _, ox, oy] = *this.size.lock().unwrap();
                let _ = window; // the pointer in panel coordinates, -1..1 with y up
                let (nx, ny) = if pw > 0.0 && ph > 0.0 { ((f32::from(e.position.x) - ox) / pw * 2.0 - 1.0, 1.0 - (f32::from(e.position.y) - oy) / ph * 2.0) } else { (0.0, 0.0) };
                this.touched = true; if let Some(c) = &mut this.cam { c.zoom_at(steps, nx.clamp(-1.0, 1.0), ny.clamp(-1.0, 1.0), pw / ph.max(1.0)); }
                this.ask_render(false); cx.notify();
            }))
    }
}

#[cfg(test)]
mod tests {
    #[test] fn a_note_for_another_source_drops_the_old_model_and_the_same_sources_keeps_it() {
        assert!(super::note_keeps_model("2|/a|", "2|/a|"), "the same model: its previous revision stays with the note");
        assert!(!super::note_keeps_model("2|/a|", "2|/b|"), "thread B's model could not be made: A's must not show under B's key");
        assert!(!super::note_keeps_model("", "2|/b|"));
    }
    // not `use super::*`: that glob brings gpui's own `test` attribute macro into scope and #[test] then expands into itself
    use super::{decode_mesh, parse_frame, render, Camera, Mesh, SolidFrame, BG};
    use super::super::raster::smooth_normals;
    fn put(out: &mut Vec<u8>, mut v: u32) { loop { let b = (v & 0x7f) as u8; v >>= 7; if v == 0 { out.push(b); return } out.push(b | 0x80); } }
    fn zig(v: i32) -> u32 { ((v << 1) ^ (v >> 31)) as u32 }
    fn encode(tris: &[([i32; 9], u32)]) -> Vec<u8> {
        let (mut out, mut color, mut p) = (Vec::new(), 0u32, [0i32; 3]);
        for (t, c) in tris { put(&mut out, c ^ color); color = *c; for k in 0..3 { for a in 0..3 { put(&mut out, zig(t[3 * k + a].wrapping_sub(p[a]))); p[a] = t[3 * k + a]; } } }
        out
    }
    fn cube() -> Vec<([i32; 9], u32)> {
        let s = 1000; let v = |x: i32, y: i32, z: i32| [x * s, y * s, z * s];
        let quad = |a: [i32; 3], b: [i32; 3], c: [i32; 3], d: [i32; 3], col: u32| -> Vec<([i32; 9], u32)> {
            vec![([a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]], col), ([a[0], a[1], a[2], c[0], c[1], c[2], d[0], d[1], d[2]], col)] };
        let mut t = Vec::new();
        t.extend(quad(v(0, 0, 0), v(1, 0, 0), v(1, 1, 0), v(0, 1, 0), 0x3060c0)); t.extend(quad(v(0, 0, 1), v(1, 0, 1), v(1, 1, 1), v(0, 1, 1), 0xc06030));
        t.extend(quad(v(0, 0, 0), v(1, 0, 0), v(1, 0, 1), v(0, 0, 1), 0x30c060)); t.extend(quad(v(0, 1, 0), v(1, 1, 0), v(1, 1, 1), v(0, 1, 1), 0xc0c030));
        t.extend(quad(v(0, 0, 0), v(0, 1, 0), v(0, 1, 1), v(0, 0, 1), 0x9030c0)); t.extend(quad(v(1, 0, 0), v(1, 1, 0), v(1, 1, 1), v(1, 0, 1), 0x30c0c0));
        t
    }
    fn mesh() -> Mesh { let t = cube(); decode_mesh(t.len(), &encode(&t)).unwrap() }
    const ALIVE: &(dyn Fn() -> bool + Sync) = &|| true;
    #[test] fn decodes_what_the_format_encodes() {
        let m = mesh();
        assert_eq!(m.tris.len(), 12); assert_eq!(m.colors[0], 0x3060c0); assert_eq!(m.colors[11], 0x30c0c0);
        assert!((m.tris[0][3] - 10.0).abs() < 1e-3, "10 mm after 1000 x 10 um");
        assert!((m.center[0] - 5.0).abs() < 1e-3 && (m.radius - (3.0f32).sqrt() * 5.0).abs() < 1e-3);
    }
    #[test] fn the_wire_y_is_turned_over_as_the_webs_mesh_does_so_silk_text_is_not_mirrored() {
        let m = mesh();
        assert!((m.center[1] + 5.0).abs() < 1e-3, "y 0..10 mm on the wire is -10..0 here");
        let t = encode(&[([0, 0, 0, 1000, 0, 0, 0, 500, 0], 1)]); let m = decode_mesh(1, &t).unwrap();
        assert_eq!((m.tris[0][7], m.tris[0][1]), (-5.0, 0.0)); // 500 x 10 um down is 5 mm toward -y
    }
    #[test] fn the_standard_views_look_along_their_axes_with_z_up_and_front_toward_minus_y() {
        let m = mesh(); let c = Camera::fitted(&m);
        let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5);
        assert!(near(c.named("front").fwd, [0.0, 1.0, 0.0]) && near(c.named("front").up, [0.0, 0.0, 1.0]));
        assert!(near(c.named("right").fwd, [-1.0, 0.0, 0.0]) && near(c.named("top").fwd, [0.0, 0.0, -1.0]) && near(c.named("top").up, [0.0, 1.0, 0.0]));
        assert!(near(c.named("iso").fwd, c.fwd), "Iso is the fitted view");
        for v in ["front", "back", "left", "right", "top", "bottom"] { let n = c.named(v); assert!((super::dot(n.right, n.up)).abs() < 1e-5 && (super::dot(n.fwd, n.up)).abs() < 1e-5, "{v}: orthonormal"); assert_eq!(n.target, c.target); }
    }
    #[test] fn a_narrow_panel_fits_further_away_and_a_wide_one_the_same() {
        let m = mesh();
        let (sq, narrow, wide) = (Camera::fitted_for(&m, 1.0), Camera::fitted_for(&m, 0.5), Camera::fitted_for(&m, 2.0));
        assert!((narrow.dist / sq.dist - 2.0).abs() < 1e-3); assert!((wide.dist - sq.dist).abs() < 1e-3);
        assert_eq!(Camera::fitted_for(&m, f32::NAN).dist, sq.dist);
        assert!(super::aspect_changed(0.0, 0.5) && !super::aspect_changed(0.56, 0.57) && super::aspect_changed(0.56, 0.9));
    }
    #[test] fn rejects_wrong_counts_truncated_bytes_and_wraps_instead_of_panicking() {
        let bytes = encode(&cube());
        assert!(decode_mesh(11, &bytes).is_err()); assert!(decode_mesh(13, &bytes).is_err()); assert!(decode_mesh(0, &bytes).is_err()); assert!(decode_mesh(12, &bytes[..bytes.len() - 3]).is_err());
        let big = encode(&[([i32::MAX, 0, 0, 0, i32::MAX, 0, 0, 0, i32::MAX], 1), ([i32::MAX, i32::MAX, 0, i32::MAX, i32::MAX, i32::MAX, 5, 5, 5], 2)]);
        let _ = decode_mesh(2, &big); // overflowing deltas wrap (no debug-mode panic)
    }
    #[test] fn renders_the_model_in_the_middle_and_background_at_the_corners() {
        let m = mesh(); let (w, h) = (96, 64); let px = render(&m, &Camera::fitted(&m), w, h, 2, ALIVE).unwrap();
        let at = |x: usize, y: usize| { let i = (y * w + x) * 4; [px[i], px[i + 1], px[i + 2]] };
        assert_eq!(at(1, 1), [BG[0], BG[1], BG[2]]); assert_ne!(at(w / 2, h / 2), [BG[0], BG[1], BG[2]]);
    }
    #[test] fn free_orbit_has_no_pole_and_keeps_an_orthonormal_basis_and_zoom_is_bounded() {
        let m = mesh(); let a = render(&m, &Camera::fitted(&m), 64, 64, 2, ALIVE).unwrap();
        let mut c = Camera::fitted(&m); for _ in 0..400 { c.orbit(3.0, 7.0); } // many turns over the pole
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        assert!(dot(c.right, c.up).abs() < 1e-3 && dot(c.right, c.fwd).abs() < 1e-3 && (dot(c.fwd, c.fwd) - 1.0).abs() < 1e-3);
        assert_ne!(a, render(&m, &c, 64, 64, 2, ALIVE).unwrap());
        let mut z = Camera::fitted(&m); for _ in 0..500 { z.zoom_at(5.0, 0.0, 0.0, 1.5); } assert!((z.dist - z.fit / 60.0).abs() < 1e-3, "in-bound 1/60");
        for _ in 0..500 { z.zoom_at(-5.0, 0.0, 0.0, 1.5); } assert!((z.dist - z.fit * 20.0).abs() < 1e-1, "out-bound 20x");
    }
    #[test] fn roll_turns_about_the_view_axis_and_peek_reads_the_key_without_the_mesh() {
        let m = mesh(); let mut c = Camera::fitted(&m); let fwd = c.fwd; let up = c.up; c.roll(60.0);
        for a in 0..3 { assert!((c.fwd[a] - fwd[a]).abs() < 1e-4, "the view direction stays"); }
        assert!(c.up.iter().zip(up).any(|(x, y)| (x - y).abs() > 1e-2), "the up vector turned");
        let text = |f: &mut Vec<u8>, s: &str| { f.push(0x60 | s.len() as u8); f.extend(s.as_bytes()) };
        let mut f = vec![0xa2]; text(&mut f, "mesh"); f.push(0x5a); f.extend(1_000_000u32.to_be_bytes()); f.extend(vec![7u8; 1_000_000]); text(&mut f, "key"); text(&mut f, "4|x");
        assert_eq!(super::peek_key(&f).as_deref(), Some("4|x"));
        assert_eq!(super::peek_key(&[0xa1, 0x63, b'a', b'b', b'c', 0x01]), None);
    }
    #[test] fn zoom_at_the_pointer_keeps_the_point_under_it_fixed() {
        let m = mesh(); let mut c = Camera::fitted(&m); let (nx, ny, aspect) = (0.4f32, -0.3f32, 1.5f32);
        let t = (super::FOV / 2.0).tan();
        let point = |c: &Camera| { let p = [c.target[0] + c.right[0] * nx * aspect * t * c.dist + c.up[0] * ny * t * c.dist, c.target[1] + c.right[1] * nx * aspect * t * c.dist + c.up[1] * ny * t * c.dist, c.target[2] + c.right[2] * nx * aspect * t * c.dist + c.up[2] * ny * t * c.dist]; p };
        let before = point(&c); c.zoom_at(3.0, nx, ny, aspect); let after = point(&c);
        for a in 0..3 { assert!((before[a] - after[a]).abs() < 1e-3, "axis {a}: {before:?} vs {after:?}"); }
    }
    #[test] fn a_triangle_behind_the_near_plane_is_clipped_not_dropped() {
        let m = mesh(); let mut c = Camera::fitted(&m); c.dist = c.fit / 60.0 * 2.0; // inside the cube: faces cross the near plane
        let px = render(&m, &c, 48, 48, 2, ALIVE).unwrap();
        assert!(px.chunks_exact(4).any(|p| [p[0], p[1], p[2]] != [BG[0], BG[1], BG[2]]), "the near faces still cover the view");
    }
    #[test] fn a_superseded_render_is_abandoned() { let m = mesh(); assert!(render(&m, &Camera::fitted(&m), 32, 32, 1, &|| false).is_none()); }
    #[test] fn frame_parser_reads_model_note_and_ignores_other_frames() {
        let bytes = encode(&cube());
        let text = |f: &mut Vec<u8>, s: &str| { f.push(0x60 | s.len() as u8); f.extend(s.as_bytes()) };
        let mut f = vec![0xa3]; text(&mut f, "key"); text(&mut f, "2|board"); text(&mut f, "n"); f.push(12); text(&mut f, "mesh"); f.push(0x59); f.extend((bytes.len() as u16).to_be_bytes()); f.extend(&bytes);
        match parse_frame(&f) { Some(SolidFrame::Model(k, m)) => { assert_eq!(k, "2|board"); assert_eq!(m.tris.len(), 12) } other => panic!("{other:?}") }
        let mut n = vec![0xa2]; text(&mut n, "key"); text(&mut n, "3|board"); text(&mut n, "none"); text(&mut n, "no model for this board");
        assert_eq!(parse_frame(&n), Some(SolidFrame::Note("3|board".into(), "no model for this board".into())));
        let mut other = f.clone(); other[6] = b'1'; assert!(parse_frame(&other).is_none());
        assert!(parse_frame(&[0xff, 0x00]).is_none());
        // a tag wraps ONE value: tag 6 over the map's text key must not eat the next item
        let mut tagged = vec![0xa1, 0xc6, 0x03]; text(&mut tagged, "x"); // key = tag(6, 3) then value "x"... the value after the tag number is the 3
        let _ = parse_frame(&tagged);
    }
    #[test] fn a_curved_surface_shades_smoothly_and_a_hard_edge_stays_sharp() {
        // a flat strip folded by 20 degrees at the middle shares its vertices: the fold is smoothed; folded by 90 degrees it is not
        let fold = |deg: f32| {
            let (c, s) = (deg.to_radians().cos(), deg.to_radians().sin());
            let t = [[0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 10.0, 0.0], [10.0, 0.0, 0.0, 10.0 + 10.0 * c, 0.0, 10.0 * s, 10.0, 10.0, 0.0]];
            (t, [[0, 0, 0], [1000, 0, 0], [0, 1000, 0], [1000, 0, 0], [2000, 0, 0], [1000, 1000, 0]])
        };
        let (t, k) = fold(20.0); let n = smooth_normals(&t, &k);
        assert!((n[0][3] - n[1][0]).abs() < 1e-4 && (n[0][5] - n[1][2]).abs() < 1e-4, "the shared vertex has one normal on both faces");
        assert!(n[0][5].abs() > 1e-3, "and it is tilted between them");
        let (t, k) = fold(90.0); let n = smooth_normals(&t, &k);
        assert!((n[0][5] - 1.0).abs() < 1e-4 && n[1][2].abs() < 0.9, "a 90 degree fold keeps each face's own normal");
    }
    #[test] fn shared_edges_leave_no_gaps_and_the_picture_is_stable() {
        // a fan of thin triangles tiling a disc in front of the camera: every pixel inside is covered, whatever the sample count
        let n = 97; let mut t = Vec::new();
        for i in 0..n { let (a, b) = (i as f32 / n as f32 * 6.2831855, (i + 1) as f32 / n as f32 * 6.2831855);
            t.push(([0, 0, 0, (a.cos() * 4000.0) as i32, (a.sin() * 4000.0) as i32, 0, (b.cos() * 4000.0) as i32, (b.sin() * 4000.0) as i32, 0], 0xc0c0c0)); }
        let m = { let bytes = encode(&t); decode_mesh(t.len(), &bytes).unwrap() };
        let c = Camera::fitted(&m).named("top");
        for ss in [1, 2, 3] {
            let (w, h) = (120, 90); let px = render(&m, &c, w, h, ss, ALIVE).unwrap();
            let bg = [BG[0], BG[1], BG[2]];
            for y in 0..h { for x in 0..w {
                let (dx, dy) = (x as f32 + 0.5 - w as f32 / 2.0, y as f32 + 0.5 - h as f32 / 2.0);
                let i = (y * w + x) * 4; let on = [px[i], px[i + 1], px[i + 2]] != bg;
                if dx * dx + dy * dy < 12.0 * 12.0 { assert!(on, "ss {ss}: a hole at {x},{y}"); }
            } }
            assert_eq!(px, render(&m, &c, w, h, ss, ALIVE).unwrap(), "ss {ss}: same picture twice");
        }
    }
    #[test] fn a_red_model_is_red_and_light_is_not_decoded_twice() {
        // the wire is 0xRRGGBB in linear light: red must not come out blue (0x0000ff was what this drew before), and a middle grey must not come out near black
        let quad = |c: u32| { let t = vec![([0, 0, 0, 1000, 0, 0, 1000, 1000, 0], c), ([0, 0, 0, 1000, 1000, 0, 0, 1000, 0], c)]; decode_mesh(2, &encode(&t)).unwrap() };
        let at = |m: &Mesh| { let (w, h) = (32, 32); let px = render(m, &Camera::fitted(m).named("top"), w, h, 1, ALIVE).unwrap(); let i = (h / 2 * w + w / 2) * 4; [px[i + 2], px[i + 1], px[i]] };
        let [r, g, b] = at(&quad(0xc00000)); assert!(r > 100 && g < 40 && b < 40, "{r} {g} {b}");
        let [r, g, b] = at(&quad(0x0000c0)); assert!(b > 100 && g < 40 && r < 40, "{r} {g} {b}");
        // a plastic grey (0x373737 and up would be guessed as metal, which shows the dim studio straight on): 0.165 of light, decoded once
        let [r, g, b] = at(&quad(0x2a2a2a)); assert!((r, g) == (g, b) && r > 70, "0.165 of light lit is well above black (about 92; decoded twice, about 40): {r}");
    }
    #[test] fn the_nearest_translucent_layer_shows_over_what_is_opaque_and_nothing_further() {
        let quad = |z: i32, c: u32| vec![([0, 0, z, 1000, 0, z, 1000, 1000, z], c), ([0, 0, z, 1000, 1000, z, 0, 1000, z], c)];
        let at = |t: Vec<([i32; 9], u32)>| { let m = decode_mesh(t.len(), &encode(&t)).unwrap(); let (w, h) = (32, 32); let px = render(&m, &Camera::fitted(&m).named("top"), w, h, 1, ALIVE).unwrap(); let i = (h / 2 * w + w / 2) * 4; [px[i + 2] as i32, px[i + 1] as i32, px[i] as i32] };
        let (red, half_green, full_green) = (0xc00000, 0x80_00c000u32, 0x00c000);
        let base = at([quad(0, red), quad(10, red)].concat());
        let over = at([quad(0, red), quad(10, half_green)].concat());
        let solid = at([quad(0, red), quad(10, full_green)].concat());
        assert!(over[0] > solid[0] + 20 && over[0] < base[0] && over[1] > base[1] + 20, "red shows through: {base:?} {over:?} {solid:?}");
        let under = at([quad(10, red), quad(0, half_green)].concat());
        assert_eq!(under, base, "a layer behind the opaque one does not show");
    }
    #[test] fn a_drag_is_never_starved() {
        use super::{shows, wanted};
        assert!(wanted(true, 3, 9), "a draft finishes though the pointer moved on");
        assert!(!wanted(false, 3, 9) && wanted(false, 9, 9), "a picture at rest gives way");
        assert!(shows(5, 3, 1), "finished behind the pointer: still newer than the screen");
        assert!(!shows(3, 5, 1), "an older picture landing late does not go back in time");
        assert!(!shows(5, 3, 6), "a picture of the previous model never shows");
    }
    #[test] fn samples_a_pixel_follow_the_panel_and_a_draft_is_one() {
        assert_eq!(super::samples(800, 600, true), 1); assert_eq!(super::samples(640, 480, false), 3);
        assert_eq!(super::samples(1600, 1000, false), 2); assert_eq!(super::samples(2880, 1800, false), 1);
    }
    /// The one device-backed test: skipped where there is no Vulkan at all.
    #[cfg(target_os = "linux")] #[test] fn gpu_and_cpu_agree() {
        // SAFETY of the env write: the only test that reads this variable
        unsafe { std::env::set_var("BACKPLANE_SOLID_RENDERER", "gpu"); }
        let Some(mut g) = super::super::solid_gpu::Gpu::new() else { eprintln!("no GPU here: skipped"); return };
        let m = std::sync::Arc::new(mesh()); let (w, h) = (96, 64); let c = Camera::fitted(&m);
        let (a, b) = (g.render(&m, &c, w, h).expect("a picture"), render(&m, &c, w, h, 3, ALIVE).unwrap());
        // inside the model, away from edges where the two anti-alias differently: within a couple of levels of each other in every channel
        let mut worst = 0; let mut seen = 0;
        for y in 1..h - 1 { for x in 1..w - 1 { let i = (y * w + x) * 4;
            let flat = |p: &[u8]| [-(w as isize) * 4, -4, 4, w as isize * 4].iter().all(|d| (0..3).all(|k| (p[(i as isize + d) as usize + k] as i32 - p[i + k] as i32).abs() < 3));
            if !flat(&b) { continue } seen += 1;
            for k in 0..3 { worst = worst.max((a[i + k] as i32 - b[i + k] as i32).abs()); }
        } }
        assert!(seen > 1000 && worst <= 4, "{seen} flat pixels, worst difference {worst}");
    }
    #[test] #[ignore] fn bench_png() {
        let (frame, n) = super::super::demo::sphere_frame(std::env::var("TRIS").ok().and_then(|t| t.parse().ok()).unwrap_or(200_000));
        let t0 = std::time::Instant::now();
        let Some(SolidFrame::Model(_, m)) = parse_frame(&frame) else { panic!() };
        eprintln!("decode+normals {n} tris: {:?}", t0.elapsed());
        let (w, h) = (1200, 800);
        for (name, ss) in [("draft", 1), ("full2", 2), ("full3", 3)] {
            let c = Camera::fitted_for(&m, 1.5); let t0 = std::time::Instant::now();
            let px = render(&m, &c, w, h, ss, ALIVE).unwrap(); eprintln!("{name}: {:?}", t0.elapsed());
            let mut rgba = px.clone(); for p in rgba.chunks_exact_mut(4) { p.swap(0, 2); }
            image::RgbaImage::from_raw(w as u32, h as u32, rgba).unwrap().save(format!("/tmp/solid-{name}.png")).unwrap();
        }
    }
}

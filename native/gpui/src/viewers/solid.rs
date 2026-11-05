//! 3D / Mech: the hub's `solid` frame (CBOR {key, n, mesh, box3, ...}; mesh = varints: per triangle the colour XOR the previous one's, then
//! three vertices as zigzag deltas of x y z in 10 micrometres, board axes, z up) decoded, and rasterized on the CPU with a z-buffer into one
//! image. Nothing repaints while idle: the image is rebuilt only when the camera or the model changes (a draft at half size while dragging).
use gpui::{prelude::*, *};
use std::sync::Arc;

pub const MAX_TRIANGLES: usize = (64 << 20) / 84;

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh { pub tris: Vec<[f32; 9]>, pub colors: Vec<u32>, pub center: [f32; 3], pub radius: f32 }

struct Varints<'a> { b: &'a [u8], at: usize, pub valid: bool }
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

/// Mesh from the frame's `n` and `mesh` bytes. Errors when the bytes are not exactly `n` triangles.
pub fn decode_mesh(n: usize, bytes: &[u8]) -> Result<Mesh, String> {
    if n == 0 || n > MAX_TRIANGLES || bytes.len() < n * 10 { return Err("Invalid or oversized model".into()) }
    let mut v = Varints::new(bytes);
    let (mut color, mut p) = (0u32, [0i32; 3]);
    let (mut tris, mut colors) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for _ in 0..n {
        color ^= v.next();
        let mut t = [0f32; 9];
        for k in 0..3 {
            for a in 0..3 { p[a] += v.signed(); t[3 * k + a] = p[a] as f32 * 0.01; } // 10 micrometres -> millimetres
            for a in 0..3 { lo[a] = lo[a].min(t[3 * k + a]); hi[a] = hi[a].max(t[3 * k + a]); }
        }
        if !v.valid { return Err("Incomplete model".into()) }
        tris.push(t); colors.push(color);
    }
    if v.more() || !v.valid { return Err("Unexpected model bytes".into()) }
    let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let radius = (0..3).map(|a| (hi[a] - lo[a]) * (hi[a] - lo[a])).sum::<f32>().sqrt() / 2.0;
    Ok(Mesh { tris, colors, center, radius: radius.max(1e-3) })
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
        6 => { let _ = item(b, at, depth + 1)?; return item(b, at, depth + 1) } // a tag (the wire's word/number tags): the value follows
        _ => Item::Other,
    })
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
/// A solid frame's mesh: `Some(Ok)` model, `Some(Err)` a bad model, `None` not a solid frame (or only a note about one).
pub fn frame_mesh(b: &[u8]) -> Option<Result<(String, Mesh), String>> {
    let f = read_frame(b)?;
    let get = |k: &str| f.iter().find(|(n, _)| n == k).map(|(_, v)| v);
    let key = match get("key") { Some(Item::Text(k)) => k.clone(), _ => return None };
    if !(key.starts_with("2|") || key.starts_with("3|") || key.starts_with("4|")) { return None }
    let Some(Item::Bytes(mesh)) = get("mesh") else { return None };
    let n = match get("n") { Some(Item::Int(n)) if *n > 0 => *n as usize, _ => return Some(Err("missing triangle count".into())) };
    Some(decode_mesh(n, mesh).map(|m| (key, m)))
}

// ---- camera and rasterizer ----------------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera { pub yaw: f32, pub pitch: f32, pub zoom: f32 }
impl Default for Camera { fn default() -> Self { Camera { yaw: 0.6, pitch: 0.7, zoom: 1.0 } } }
impl Camera {
    pub fn orbit(&mut self, dx: f32, dy: f32) { self.yaw -= dx * 0.01; self.pitch = (self.pitch + dy * 0.01).clamp(-1.5, 1.5); }
    pub fn zoom_by(&mut self, steps: f32) { self.zoom = (self.zoom * 1.1f32.powf(steps)).clamp(0.1, 50.0); }
}
const BG: [u8; 4] = [18, 15, 13, 255]; // BGRA of the page background (#0d0f12)

/// Render into BGRA (w x h x 4), z-buffered, two-sided lambert with a headlight.
pub fn render(mesh: &Mesh, cam: &Camera, w: usize, h: usize) -> Vec<u8> {
    let mut px = vec![0u8; w * h * 4];
    for c in px.chunks_exact_mut(4) { c.copy_from_slice(&BG); }
    let mut z = vec![f32::INFINITY; w * h];
    let fov = 35f32.to_radians(); let f = 1.0 / (fov / 2.0).tan();
    let dist = mesh.radius / (fov / 2.0).sin() * 1.05 / cam.zoom;
    // eye on a sphere around the centre: z up, yaw about z
    let (sy, cy, sp, cp) = (cam.yaw.sin(), cam.yaw.cos(), cam.pitch.sin(), cam.pitch.cos());
    let eye = [mesh.center[0] + dist * cp * sy, mesh.center[1] - dist * cp * cy, mesh.center[2] + dist * sp];
    let fwd = norm([mesh.center[0] - eye[0], mesh.center[1] - eye[1], mesh.center[2] - eye[2]]);
    let right = norm(cross(fwd, [0.0, 0.0, 1.0])); let up = cross(right, fwd);
    let (cxp, cyp, scale) = (w as f32 / 2.0, h as f32 / 2.0, h as f32 / 2.0 * f);
    let near = 0.05 * mesh.radius;
    let project = |p: [f32; 3]| -> Option<[f32; 3]> {
        let d = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
        let (x, y, zc) = (dot(d, right), dot(d, up), dot(d, fwd));
        if zc < near { return None }
        Some([cxp + x / zc * scale, cyp - y / zc * scale, zc])
    };
    for (t, &color) in mesh.tris.iter().zip(&mesh.colors) {
        let (a, b, c) = ([t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]);
        let (Some(pa), Some(pb), Some(pc)) = (project(a), project(b), project(c)) else { continue };
        let n = norm(cross(sub(b, a), sub(c, a)));
        let shade = 0.3 + 0.7 * dot(n, fwd).abs();
        let rgb = [((color >> 16) & 255) as f32, ((color >> 8) & 255) as f32, (color & 255) as f32];
        let bgra = [(rgb[2] * shade) as u8, (rgb[1] * shade) as u8, (rgb[0] * shade) as u8, 255];
        let (minx, maxx) = (pa[0].min(pb[0]).min(pc[0]).floor().max(0.0) as usize, (pa[0].max(pb[0]).max(pc[0]).ceil() as i64).clamp(0, w as i64 - 1) as usize);
        let (miny, maxy) = (pa[1].min(pb[1]).min(pc[1]).floor().max(0.0) as usize, (pa[1].max(pb[1]).max(pc[1]).ceil() as i64).clamp(0, h as i64 - 1) as usize);
        let area = (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pb[1] - pa[1]) * (pc[0] - pa[0]);
        if area.abs() < 1e-6 { continue }
        for y in miny..=maxy { for x in minx..=maxx {
            let (sx, sy_) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((pb[0] - sx) * (pc[1] - sy_) - (pb[1] - sy_) * (pc[0] - sx)) / area;
            let w1 = ((pc[0] - sx) * (pa[1] - sy_) - (pc[1] - sy_) * (pa[0] - sx)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 { continue }
            let depth = w0 * pa[2] + w1 * pb[2] + w2 * pc[2];
            let i = y * w + x;
            if depth < z[i] { z[i] = depth; px[i * 4..i * 4 + 4].copy_from_slice(&bgra); }
        } }
    }
    px
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn norm(a: [f32; 3]) -> [f32; 3] { let l = dot(a, a).sqrt(); if l > 0.0 { [a[0] / l, a[1] / l, a[2] / l] } else { [0.0, 0.0, 1.0] } }

pub fn to_image(bgra: Vec<u8>, w: usize, h: usize) -> Option<Arc<RenderImage>> {
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, bgra)?; // gpui takes BGRA in this container
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buf)])))
}

// ---- the page ---------------------------------------------------------------------------------------------------
const FULL: (usize, usize) = (1280, 800);
const DRAFT: (usize, usize) = (640, 400);

pub struct SolidPage { mesh: Option<Arc<Mesh>>, key: String, cam: Camera, image: Option<Arc<RenderImage>>, dragging: Option<Point<Pixels>>, pub note: String }
impl SolidPage {
    pub fn new(_cx: &mut Context<Self>) -> Self { SolidPage { mesh: None, key: String::new(), cam: Camera::default(), image: None, dragging: None, note: "No 3D model yet".into() } }
    /// A plot-lane binary frame: true when it was a solid frame (taken, even when its model was bad); the screens lane calls this for keys 2|, 3|, 4|.
    pub fn accept(&mut self, bytes: &[u8], cx: &mut Context<Self>) -> bool {
        match frame_mesh(bytes) {
            None => false,
            Some(Err(e)) => { self.note = e; cx.notify(); true }
            Some(Ok((key, mesh))) => {
                if key != self.key { self.cam = Camera::default(); }
                self.key = key; self.mesh = Some(Arc::new(mesh)); self.note.clear(); self.redraw(false); cx.notify(); true
            }
        }
    }
    fn redraw(&mut self, draft: bool) {
        let Some(m) = &self.mesh else { return };
        let (w, h) = if draft { DRAFT } else { FULL };
        self.image = to_image(render(m, &self.cam, w, h), w, h);
    }
}
impl Render for SolidPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.image {
            Some(i) => img(i.clone()).size_full().object_fit(ObjectFit::Contain).into_any_element(),
            None => div().size_full().flex().items_center().justify_center().text_color(rgb(0x8a93a3)).child(self.note.clone()).into_any_element(),
        };
        div().id("solid-page").size_full().overflow_hidden().child(body)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, _| { this.dragging = Some(e.position); }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                let Some(last) = this.dragging else { return };
                this.cam.orbit(f32::from(e.position.x - last.x), f32::from(e.position.y - last.y)); this.dragging = Some(e.position);
                this.redraw(true); cx.notify(); // half size while dragging, the full image on release
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| { if this.dragging.take().is_some() { this.redraw(false); cx.notify(); } }))
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                let dy = match e.delta { ScrollDelta::Pixels(p) => f32::from(p.y) / 20.0, ScrollDelta::Lines(l) => l.y };
                this.cam.zoom_by(dy); this.redraw(false); cx.notify();
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_mesh, frame_mesh, render, Camera, BG};
    fn put(out: &mut Vec<u8>, mut v: u32) { loop { let b = (v & 0x7f) as u8; v >>= 7; if v == 0 { out.push(b); return } out.push(b | 0x80); } }
    fn zig(v: i32) -> u32 { ((v << 1) ^ (v >> 31)) as u32 }
    /// The encoder side of the documented format, for the tests: triangles of 10 um units with a colour each.
    fn encode(tris: &[([i32; 9], u32)]) -> Vec<u8> {
        let (mut out, mut color, mut p) = (Vec::new(), 0u32, [0i32; 3]);
        for (t, c) in tris { put(&mut out, c ^ color); color = *c; for k in 0..3 { for a in 0..3 { put(&mut out, zig(t[3 * k + a] - p[a])); p[a] = t[3 * k + a]; } } }
        out
    }
    fn cube() -> Vec<([i32; 9], u32)> {
        let s = 1000; // 10 mm
        let v = |x: i32, y: i32, z: i32| [x * s, y * s, z * s];
        let quad = |a: [i32; 3], b: [i32; 3], c: [i32; 3], d: [i32; 3], col: u32| -> Vec<([i32; 9], u32)> {
            vec![([a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]], col), ([a[0], a[1], a[2], c[0], c[1], c[2], d[0], d[1], d[2]], col)]
        };
        let mut t = Vec::new();
        t.extend(quad(v(0, 0, 0), v(1, 0, 0), v(1, 1, 0), v(0, 1, 0), 0x3060c0)); t.extend(quad(v(0, 0, 1), v(1, 0, 1), v(1, 1, 1), v(0, 1, 1), 0xc06030));
        t.extend(quad(v(0, 0, 0), v(1, 0, 0), v(1, 0, 1), v(0, 0, 1), 0x30c060)); t.extend(quad(v(0, 1, 0), v(1, 1, 0), v(1, 1, 1), v(0, 1, 1), 0xc0c030));
        t.extend(quad(v(0, 0, 0), v(0, 1, 0), v(0, 1, 1), v(0, 0, 1), 0x9030c0)); t.extend(quad(v(1, 0, 0), v(1, 1, 0), v(1, 1, 1), v(1, 0, 1), 0x30c0c0));
        t
    }
    #[test] fn decodes_what_the_format_encodes() {
        let tris = cube(); let m = decode_mesh(tris.len(), &encode(&tris)).unwrap();
        assert_eq!(m.tris.len(), 12); assert_eq!(m.colors[0], 0x3060c0); assert_eq!(m.colors[11], 0x30c0c0);
        assert!((m.tris[0][3] - 10.0).abs() < 1e-3, "10 mm after 1000 x 10 um");
        assert!((m.center[0] - 5.0).abs() < 1e-3 && (m.radius - (3.0f32).sqrt() * 5.0).abs() < 1e-3);
    }
    #[test] fn rejects_wrong_counts_and_truncated_bytes() {
        let tris = cube(); let bytes = encode(&tris);
        assert!(decode_mesh(11, &bytes).is_err(), "bytes left over");
        assert!(decode_mesh(13, &bytes).is_err(), "bytes run out");
        assert!(decode_mesh(0, &bytes).is_err()); assert!(decode_mesh(12, &bytes[..bytes.len() - 3]).is_err());
    }
    #[test] fn renders_the_model_in_the_middle_and_background_at_the_corners() {
        let tris = cube(); let m = decode_mesh(tris.len(), &encode(&tris)).unwrap();
        let (w, h) = (96, 64); let px = render(&m, &Camera::default(), w, h);
        let at = |x: usize, y: usize| { let i = (y * w + x) * 4; [px[i], px[i + 1], px[i + 2]] };
        assert_eq!(at(1, 1), [BG[0], BG[1], BG[2]], "corner is background"); assert_ne!(at(w / 2, h / 2), [BG[0], BG[1], BG[2]], "the cube is drawn");
    }
    #[test] fn orbit_and_zoom_change_the_picture_and_zbuffer_hides_the_far_side() {
        let tris = cube(); let m = decode_mesh(tris.len(), &encode(&tris)).unwrap();
        let a = render(&m, &Camera::default(), 64, 64);
        let mut c = Camera::default(); c.orbit(80.0, 0.0); assert_ne!(a, render(&m, &c, 64, 64));
        let mut z = Camera::default(); z.zoom_by(5.0); assert_ne!(a, render(&m, &z, 64, 64));
        // looking straight down the top face: the pixel at the centre is the top face's colour (0xc06030, nearer than the bottom 0x3060c0)
        let top = Camera { yaw: 0.0, pitch: 1.5, zoom: 1.0 }; let px = render(&m, &top, 64, 64); let i = (32 * 64 + 32) * 4;
        assert!(px[i + 2] > px[i], "red channel (top, 0xc0) beats blue (bottom, 0x30) at the centre: {:?}", &px[i..i + 3]);
    }
    #[test] fn frame_reader_finds_key_n_and_mesh_and_ignores_other_frames() {
        let bytes = encode(&cube());
        let mut f = vec![0xa3]; // map of 3
        for (k, v) in [("key", None), ("n", Some(12u8)), ("mesh", None)] {
            f.push(0x60 | k.len() as u8); f.extend(k.as_bytes());
            match (k, v) { ("key", _) => { let s = b"2|board"; f.push(0x60 | s.len() as u8); f.extend(s) } ("n", Some(n)) => f.push(n), _ => { f.push(0x59); f.extend((bytes.len() as u16).to_be_bytes()); f.extend(&bytes) } }
        }
        let (key, m) = frame_mesh(&f).unwrap().unwrap(); assert_eq!(key, "2|board"); assert_eq!(m.tris.len(), 12);
        let mut other = f.clone(); other[6] = b'1'; // key "1|board" is a plot, not a solid
        assert!(frame_mesh(&other).is_none());
        assert!(frame_mesh(&[0xff, 0x00]).is_none());
    }
}

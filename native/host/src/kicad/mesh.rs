//! gfx/mesh.bend's `Mesh.render` in Rust: a z-buffered rasterizer of a model through a camera, one sample a pixel, for `mech.render`.
//! The result is the Bend renderer's pixels exactly, so everything that decides a pixel is the Bend step in the Bend order:
//!   * single precision throughout, each operation rounded as Math.fround rounds it (no fused multiply-add: Rust never contracts);
//!   * the triangle reaches a pixel only if it passed `meets` at every level above it, the conservative test included: regions of
//!     64 px under a 2^d square (Bins.put), then 32, 16, 8 and 4 px squares inside a region (Seq.go, Tile.b16, Tile.b8);
//!   * equal depths go to the first triangle in a pixel's list, and that list is the model's triangles LAST FIRST: Chunk.cut turns
//!     each chunk and the chunk list round, Proj.all and Bins.all turn each chunk round once more (the clipped halves of a triangle
//!     end up second half first). `order_of_equal_depth` below states it.
//!
//! Regions are independent, so they are dealt over the cores in any order: the image does not depend on the deal. Projection runs in
//! blocks over the cores and each block's region lists are joined in block order, last block first. The face image of gfx/mesh.bend
//! (the board's own drawing in the window's 3D viewer) is not here: `Mesh.render` passes none, and only the native window drew one.
use super::{fl, glb::Tri3};
use std::sync::{atomic::{AtomicUsize, Ordering}, Mutex};

/// Render.ground: what shows where no triangle does
pub const GROUND: u32 = 2105384;

// Camera ------------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam { pub r: [f32; 3], pub u: [f32; 3], pub p: [f32; 3], pub dist: f32, pub sx: f32, pub sy: f32, pub fov: f32 }
impl Cam {
    /// Cam.at: the axes of a camera at yaw and pitch around the y-up world
    pub fn at(yaw: f32, pitch: f32, p: [f32; 3], dist: f32, fov: f32) -> Cam {
        let (cy, sy, cp, sp) = (fl::cos(yaw), fl::sin(yaw), fl::cos(pitch), fl::sin(pitch));
        Cam { r: [cy, 0.0, -sy], u: [-(sy * sp), cp, -(cy * sp)], p, dist, sx: 0.0, sy: 0.0, fov }
    }
    /// Cam.face: the same pivot, distance and field of view, looking from yaw and pitch, centred
    pub fn face(&self, yaw: f32, pitch: f32) -> Cam { Cam::at(yaw, pitch, self.p, self.dist, self.fov) }
    /// Cam.first
    fn first() -> Cam { Cam::at(0.3, 0.95, [0.0; 3], 1.0, 0.7) }
}
/// Box3.reach
fn reach(a0: f32, a1: f32, p: f32) -> f32 { fl::max((a0 - p).abs(), (a1 - p).abs()) }
/// Box3: x0 y0 z0, x1 y1 z1 of every corner
pub fn bounds(ts: &[Tri3]) -> ([f32; 3], [f32; 3]) {
    let big = 1000000000.0f32;
    let (mut lo, mut hi) = ([big; 3], [-big; 3]);
    for t in ts { for v in [t.a, t.b, t.c] { for k in 0..3 { lo[k] = fl::min(lo[k], v[k]); hi[k] = fl::max(hi[k], v[k]) } } }
    (lo, hi)
}
/// Cam.of_box.at: the camera c turned as it is, turning about p, far enough back to show the whole box round it
fn of_box_at(lo: [f32; 3], hi: [f32; 3], p: [f32; 3], c: Cam) -> Cam {
    let (dx, dy, dz) = (reach(lo[0], hi[0], p[0]), reach(lo[1], hi[1], p[1]), reach(lo[2], hi[2], p[2]));
    let r = fl::max((dx * dx + (dy * dy + dz * dz)).sqrt(), 0.000001);
    Cam { p, dist: 1.15 * (r / fl::sin(c.fov * 0.5)), sx: 0.0, sy: 0.0, ..c }
}
/// Model.fit: the camera that shows the box from the first side, centred on it
pub fn fit(lo: [f32; 3], hi: [f32; 3]) -> Cam { of_box_at(lo, hi, [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2])], Cam::first()) }

/// the camera resolved for a frame (View): eye, right, up, forward, focal length and centre in pixels, near plane, frame size, the unit
/// vector towards the light. lo, the first sample's offset in a pixel, is 0.5: Mesh.render takes one sample a pixel.
#[derive(Clone, Copy, Debug)]
struct View { e: [f32; 3], r: [f32; 3], u: [f32; 3], f: [f32; 3], focal: f32, cx: f32, cy: f32, near: f32, w: f32, h: f32, l: [f32; 3] }
const LO: f32 = 0.5;
impl View {
    /// View.of
    fn of(c: &Cam, w: u32, h: u32) -> View {
        let ([rx, ry, rz], [ux, uy, uz], [px, py, pz]) = (c.r, c.u, c.p);
        let (tx, ty, tz) = (px + (rx * c.sx + ux * c.sy), py + (ry * c.sx + uy * c.sy), pz + (rz * c.sx + uz * c.sy));
        let (fx, fy, fz) = (uy * rz - uz * ry, uz * rx - ux * rz, ux * ry - uy * rx);
        let (w, h) = (w as f32, h as f32);
        let (lr, lu, lb) = (-0.3f32, 0.55f32, 0.78f32);
        View { e: [tx - fx * c.dist, ty - fy * c.dist, tz - fz * c.dist], r: c.r, u: c.u, f: [fx, fy, fz],
            focal: (0.5 * fl::min(w, h)) / tan(0.5 * c.fov), cx: 0.5 * w, cy: 0.5 * h, near: c.dist * 0.001, w, h,
            l: [lr * rx + lu * ux - lb * fx, lr * ry + lu * uy - lb * fy, lr * rz + lu * uz - lb * fz] }
    }
    /// View.pt: a world point in view space (x right, y up, z forward)
    fn pt(&self, p: [f32; 3]) -> [f32; 3] {
        let (dx, dy, dz) = (p[0] - self.e[0], p[1] - self.e[1], p[2] - self.e[2]);
        [dot3(self.r, dx, dy, dz), dot3(self.u, dx, dy, dz), dot3(self.f, dx, dy, dz)]
    }
}
fn tan(a: f32) -> f32 { (a as f64).tan() as f32 }
fn dot3(a: [f32; 3], x: f32, y: f32, z: f32) -> f32 { a[0] * x + a[1] * y + a[2] * z }

// Projection --------------------------------------------------------------------------------------------------------------------

/// a screen triangle: pixel box (inclusive); three edge functions a*(x-ex)+b*(y-ey), all >= 0 inside, an edge measured from whichever
/// of its ends comes first so that two triangles sharing it get exactly opposite values; c, the edge's constant term for the rough
/// reach test; the 1/z plane za*x+zb*y+zc; the shaded colour and its opacity
#[derive(Clone, Copy, Debug)]
struct STri { x0: u32, y0: u32, x1: u32, y1: u32, a: [f32; 3], b: [f32; 3], ex: [f32; 3], ey: [f32; 3], c: [f32; 3], za: f32, zb: f32, zc: f32, color: u32, alpha: u32 }

/// Proj.first: whether (px, py) comes before (qx, qy): by x, then y
fn first(px: f32, py: f32, qx: f32, qy: f32) -> bool { px < qx || (px == qx && py <= qy) }
/// Proj.edges and Proj.box: edges AB, BC, CA and the 1/z plane of a screen triangle, kept if its box has sample points in it
fn edges(v: &View, xa: f32, ya: f32, ia: f32, xb: f32, yb: f32, ib: f32, xc: f32, yc: f32, ic: f32, color: u32, alpha: u32, out: &mut Vec<STri>) {
    let (e1x, e1y, e2x, e2y) = (xb - xa, yb - ya, xc - xa, yc - ya);
    let (d1, d2) = (ib - ia, ic - ia);
    let area = e1x * e2y - e2x * e1y;
    let gx = (d1 * e2y - d2 * e1y) / area;
    let gy = (d2 * e1x - d1 * e2x) / area;
    let x0 = fl::max((fl::min(xa, fl::min(xb, xc)) - (1.0 - LO)).ceil(), 0.0);
    let y0 = fl::max((fl::min(ya, fl::min(yb, yc)) - (1.0 - LO)).ceil(), 0.0);
    let x1 = fl::min((fl::max(xa, fl::max(xb, xc)) - LO).floor(), v.w - 1.0);
    let y1 = fl::min((fl::max(ya, fl::max(yb, yc)) - LO).floor(), v.h - 1.0);
    if !(x0 <= x1 && y0 <= y1 && area.abs() > 0.0001) { return }
    let (ab, bc, ca) = (first(xa, ya, xb, yb), first(xb, yb, xc, yc), first(xc, yc, xa, ya));
    let pick = |c: bool, p: f32, q: f32| if c { p } else { q };
    let s = area < 0.0; // Proj.flip: a clockwise triangle's edges turned so inside is >= 0
    let flip = |x: f32| if s { -x } else { x };
    let a = [flip(ya - yb), flip(yb - yc), flip(yc - ya)];
    let b = [flip(xb - xa), flip(xc - xb), flip(xa - xc)];
    let ex = [pick(ab, xa, xb), pick(bc, xb, xc), pick(ca, xc, xa)];
    let ey = [pick(ab, ya, yb), pick(bc, yb, yc), pick(ca, yc, ya)];
    let edge_c = |k: usize| -(a[k] * ex[k] + b[k] * ey[k]); // Edge.c
    let floor = |f: f32| fl::to_u32(fl::max(f, 0.0)); // Proj.floor
    out.push(STri { x0: floor(x0), y0: floor(y0), x1: floor(x1), y1: floor(y1), a, b, ex, ey, c: [edge_c(0), edge_c(1), edge_c(2)], za: gx, zb: gy, zc: ia - (gx * xa + gy * ya), color, alpha });
}
/// Proj.screen: three view-space points (all past the near plane) on screen
fn screen(v: &View, a: [f32; 3], b: [f32; 3], c: [f32; 3], col: u32, alpha: u32, out: &mut Vec<STri>) {
    let sx = |p: [f32; 3]| v.cx + (v.focal * p[0]) / p[2];
    let sy = |p: [f32; 3]| v.cy - (v.focal * p[1]) / p[2];
    edges(v, sx(a), sy(a), 1.0 / a[2], sx(b), sy(b), 1.0 / b[2], sx(c), sy(c), 1.0 / c[2], col, alpha, out)
}
/// VP.cut: where the segment from p (in front) to q (behind) crosses the near plane
fn cut(p: [f32; 3], q: [f32; 3], near: f32) -> [f32; 3] {
    let t = (p[2] - near) / (p[2] - q[2]);
    [p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1]), near]
}
/// Clip.one: one vertex (a) in front of the near plane
fn clip_one(v: &View, a: [f32; 3], b: [f32; 3], c: [f32; 3], col: u32, alpha: u32, out: &mut Vec<STri>) { screen(v, a, cut(a, b, v.near), cut(a, c, v.near), col, alpha, out) }
/// Clip.two: two vertices (a, b) in front: the quad, as two (`out` runs first to last, and a region's list is that order turned round)
fn clip_two(v: &View, a: [f32; 3], b: [f32; 3], c: [f32; 3], col: u32, alpha: u32, out: &mut Vec<STri>) {
    let bc = cut(b, c, v.near);
    screen(v, a, b, bc, col, alpha, out);
    screen(v, a, bc, cut(a, c, v.near), col, alpha, out);
}
/// Rgb.scale.ch
fn scale_ch(c: u32, s: u32, k: f32) -> u32 { fl::to_u32((((c >> s) & 255) as f32) * k) << s }
/// Proj.shade: flat Lambert over a little ambient (a triangle kept faces us)
fn shade(c: u32, n: [f32; 3], l: [f32; 3]) -> u32 {
    let len = fl::max(dot3(n, n[0], n[1], n[2]).sqrt(), 0.0000000001);
    let lam = fl::max(dot3(n, l[0], l[1], l[2]) / len, 0.0);
    let k = 0.3 + 0.7 * lam; let c = c & 16777215;
    scale_ch(c, 16, k) | (scale_ch(c, 8, k) | scale_ch(c, 0, k))
}
/// Proj.normal: a world triangle that shows, as screen triangles (back faces culled, cut at the near plane)
fn project(t: &Tri3, v: &View, out: &mut Vec<STri>) {
    let ([ax, ay, az], [bx, by, bz], [cx, cy, cz]) = (t.a, t.b, t.c);
    let (ux, uy, uz, wx, wy, wz) = (bx - ax, by - ay, bz - az, cx - ax, cy - ay, cz - az);
    let n = [uy * wz - uz * wy, uz * wx - ux * wz, ux * wy - uy * wx];
    if !(dot3(n, v.e[0] - ax, v.e[1] - ay, v.e[2] - az) > 0.0) { return } // Proj.facing: the eye is on the triangle's front side
    let col = shade(t.color, n, v.l);
    let alpha = ((255 - (t.color >> 24)) * 255) / 255; // Proj.alpha, a chunk there from the start: Fade.alpha is 255
    let (a, b, c) = (v.pt(t.a), v.pt(t.b), v.pt(t.c));
    match (a[2] > v.near, b[2] > v.near, c[2] > v.near) {
        (true, true, true) => screen(v, a, b, c, col, alpha, out),
        (true, false, false) => clip_one(v, a, b, c, col, alpha, out),
        (false, true, false) => clip_one(v, b, c, a, col, alpha, out),
        (false, false, true) => clip_one(v, c, a, b, col, alpha, out),
        (true, true, false) => clip_two(v, a, b, c, col, alpha, out),
        (false, true, true) => clip_two(v, b, c, a, col, alpha, out),
        (true, false, true) => clip_two(v, c, a, b, col, alpha, out),
        (false, false, false) => {}
    }
}

// Reaching a square -------------------------------------------------------------------------------------------------------------

/// Edge.reaches: the largest a*x + b*y + c over the square's pixel centres, with half a pixel to spare, is not below 0
fn reaches(a: f32, b: f32, c: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> bool {
    (c + fl::max(a * x0, a * x1)) + (fl::max(b * y0, b * y1) + 0.5 * (a.abs() + b.abs())) >= 0.0
}
/// STri.meets: does the triangle reach a pixel centre of the h x h square at (qx, qy)?
fn meets(t: &STri, qx: u32, qy: u32, h: u32) -> bool {
    if !(t.x0 < qx + h && t.x1 >= qx && t.y0 < qy + h && t.y1 >= qy) { return false }
    let (fx0, fy0, fx1, fy1) = (qx as f32 + 0.5, qy as f32 + 0.5, (qx + h) as f32 - 0.5, (qy + h) as f32 - 0.5);
    (0..3).all(|k| reaches(t.a[k], t.b[k], t.c[k], fx0, fy0, fx1, fy1))
}
/// Bins.put: t into every region of the n-px square at (qx, qy) it reaches
fn bin(t: &STri, id: u32, qx: u32, qy: u32, n: u32, cols: u32, bins: &mut [Vec<u32>]) {
    if n == REGION { bins[((qy / REGION) * cols + qx / REGION) as usize].push(id); return }
    let h = n / 2;
    for (x, y) in [(qx, qy), (qx + h, qy), (qx, qy + h), (qx + h, qy + h)] { if meets(t, x, y, h) { bin(t, id, x, y, h, cols, bins) } }
}
const REGION: u32 = 64;

// Rasterizing -------------------------------------------------------------------------------------------------------------------

/// a pixel's nearest opaque hit and nearest translucent hit (depth is 1/z; 0 is the background at infinity)
#[derive(Clone, Copy)]
struct Hit { z: f32, c: u32, tz: f32, tc: u32, ta: u32 }
/// Rgb.mix: fg over bg with opacity t
fn mix(fg: u32, bg: u32, t: u32) -> u32 {
    let ch = |s: u32| ((((fg >> s) & 255) * t + ((bg >> s) & 255) * (255 - t)) / 255) << s;
    ch(16) | (ch(8) | ch(0))
}
impl Hit { fn color(&self) -> u32 { if self.tz > self.z { mix(self.tc, self.c, self.ta) } else { self.c } } }

/// Blk.draw and Blk.go: a 4x4 block of pixels, one walk over its triangles
fn block(ts: &[u32], all: &[STri], rx: u32, ry: u32, bg: u32, px: &mut [u32], stride: usize) {
    let x0 = rx as f32 + 0.5; let y0 = ry as f32 + 0.5;
    let xs = [x0, x0 + 1.0, x0 + 2.0, x0 + 3.0]; let ys = [y0, y0 + 1.0, y0 + 2.0, y0 + 3.0];
    let mut hits = [Hit { z: 0.0, c: bg, tz: 0.0, tc: bg, ta: 0 }; 16];
    for id in ts {
        let t = &all[*id as usize];
        let opaque = t.alpha > 229;
        for (j, py) in ys.iter().enumerate() { for (i, qx) in xs.iter().enumerate() {
            let (x, y) = (*qx, *py);
            let inside = (0..3).all(|k| t.a[k] * (x - t.ex[k]) + t.b[k] * (y - t.ey[k]) >= 0.0);
            if !inside { continue }
            let iz = t.za * x + t.zb * y + t.zc;
            let h = &mut hits[j * 4 + i];
            if opaque { if iz > h.z { h.z = iz; h.c = t.color } } else if iz > h.tz { h.tz = iz; h.tc = t.color; h.ta = t.alpha }
        } }
    }
    for j in 0..4 { for i in 0..4 { px[j * stride + i] = hits[j * 4 + i].color() } }
}
/// Seq.go, Tile.b16, Tile.b8, Blk.four: the n-px square at (rx, ry) split to quadrants, each with the triangles that reach it, down to
/// 4x4 blocks. `px` is the region's pixels from this square's corner on; no triangle leaves them as they were (the ground).
fn square(ts: &[u32], all: &[STri], rx: u32, ry: u32, n: u32, bg: u32, px: &mut [u32], base: usize, stride: usize) {
    if ts.is_empty() { return }
    if n == 4 { block(ts, all, rx, ry, bg, &mut px[base..], stride); return }
    let h = n / 2; let mut q = Vec::with_capacity(ts.len());
    for (dx, dy) in [(0, 0), (h, 0), (0, h), (h, h)] {
        q.clear(); q.extend(ts.iter().copied().filter(|id| meets(&all[*id as usize], rx + dx, ry + dy, h)));
        square(&q, all, rx + dx, ry + dy, h, bg, px, base + dy as usize * stride + dx as usize, stride);
    }
}

fn workers() -> usize { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(16) }
/// a worker's share at low priority (a render takes seconds of CPU; the hub's threads answer first). Linux gives each thread its own nice.
fn low() { #[cfg(target_os = "linux")] unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 10); } }
/// jobs 0..n dealt over the cores, in any order: `f` must not depend on it
fn deal<T: Send>(n: usize, f: &(dyn Fn(usize) -> T + Sync)) -> Vec<(usize, T)> {
    let next = AtomicUsize::new(0); let done = Mutex::new(Vec::with_capacity(n));
    std::thread::scope(|s| { for _ in 0..workers().min(n.max(1)) { s.spawn(|| { low(); let mut mine = vec![];
        loop { let i = next.fetch_add(1, Ordering::Relaxed); if i >= n { break } mine.push((i, f(i))) }
        done.lock().unwrap().extend(mine); }); } });
    done.into_inner().unwrap()
}
const BLOCK: usize = 16384;

/// R.Frame.depth: the exponent of the smallest power of two covering w and h
fn frame_depth(w: u32, h: u32) -> u32 { let mut d = 1; while (1u64 << d) < w.max(h).max(2) as u64 { d += 1 } d }

/// Mesh.render: the model through the camera, z-buffered, over bg: w x h pixels of 8-bit RGB, row by row. A frame at least 33 px on its
/// longest side (the region grid needs one 64 px region; Render's frames are 800 x 600).
pub fn render(w: u32, h: u32, ts: &[Tri3], c: &Cam, bg: u32) -> Vec<u8> {
    let v = View::of(c, w, h);
    let d = frame_depth(w, h).max(6); let side = 1u32 << d; let cols = side / REGION;
    let regions = (cols * cols) as usize;
    // each block of the model: its screen triangles first to last, and each region's list of them, last first
    let nb = ts.len().div_ceil(BLOCK);
    let mut made = deal(nb, &|b| {
        let mut st = Vec::with_capacity(BLOCK / 2);
        for t in &ts[b * BLOCK..((b + 1) * BLOCK).min(ts.len())] { project(t, &v, &mut st) }
        let mut bins = vec![vec![]; regions];
        for (i, t) in st.iter().enumerate().rev() { bin(t, i as u32, 0, 0, side, cols, &mut bins) }
        (st, bins)
    });
    made.sort_by_key(|m| std::cmp::Reverse(m.0)); // the last block's triangles come first
    let mut all = Vec::with_capacity(made.iter().map(|m| m.1.0.len()).sum());
    let mut lists: Vec<Vec<u32>> = vec![vec![]; regions];
    for (_, (st, bins)) in made {
        let at = all.len() as u32;
        all.extend(st);
        for (l, b) in lists.iter_mut().zip(bins) { l.extend(b.into_iter().map(|i| at + i)) }
    }
    // the regions on the frame, drawn over the cores; each answers its own 64 x 64 pixels
    let on: Vec<u32> = (0..regions as u32).filter(|r| (r % cols) * REGION < w && (r / cols) * REGION < h).collect();
    let (all, lists) = (&all, &lists);
    let drawn = deal(on.len(), &|i| {
        let r = on[i]; let (x, y) = ((r % cols) * REGION, (r / cols) * REGION);
        let mut px = vec![bg; (REGION * REGION) as usize];
        square(&lists[r as usize], all, x, y, REGION, bg, &mut px, 0, REGION as usize);
        px
    });
    let mut out = vec![0u8; w as usize * h as usize * 3];
    for (i, px) in drawn {
        let r = on[i]; let (x0, y0) = ((r % cols) * REGION, (r / cols) * REGION);
        for y in y0..(y0 + REGION).min(h) { for x in x0..(x0 + REGION).min(w) {
            let p = px[((y - y0) * REGION + (x - x0)) as usize]; let at = (y as usize * w as usize + x as usize) * 3;
            out[at] = (p >> 16) as u8; out[at + 1] = (p >> 8) as u8; out[at + 2] = p as u8;
        } }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tri(a: [f32; 3], b: [f32; 3], c: [f32; 3], color: u32) -> Tri3 { Tri3 { a, b, c, color } }
    /// a square facing the camera at z (Cam looks along -z at yaw 0, pitch 0)
    fn quad(s: f32, z: f32, color: u32) -> [Tri3; 2] { [tri([-s, -s, z], [s, -s, z], [s, s, z], color), tri([-s, -s, z], [s, s, z], [-s, s, z], color)] }
    fn front() -> Cam { Cam { p: [0.0; 3], dist: 5.0, ..Cam::at(0.0, 0.0, [0.0; 3], 5.0, 0.7) } }
    fn px(img: &[u8], w: u32, x: u32, y: u32) -> u32 { let i = ((y * w + x) * 3) as usize; (img[i] as u32) << 16 | (img[i + 1] as u32) << 8 | img[i + 2] as u32 }

    #[test] fn no_triangles_is_the_ground() { let i = render(100, 80, &[], &front(), GROUND); assert_eq!(i.len(), 100 * 80 * 3); assert!(i.chunks(3).all(|p| p == [0x20, 0x20, 0x28])); }
    #[test] fn the_nearest_face_wins_whatever_the_order() {
        let (near, far) = (quad(1.0, 1.0, 0xff0000), quad(1.0, 0.0, 0x00ff00));
        let (a, b): (Vec<Tri3>, Vec<Tri3>) = (near.iter().chain(far.iter()).copied().collect(), far.iter().chain(near.iter()).copied().collect());
        let (i, j) = (render(200, 200, &a, &front(), GROUND), render(200, 200, &b, &front(), GROUND));
        assert_eq!(i, j); let c = px(&i, 200, 100, 100); assert!(c >> 16 > 0 && c & 0xff00 == 0, "{c:x}");
        assert_eq!(px(&i, 200, 2, 2), GROUND);
    }
    #[test] fn order_of_equal_depth() {
        // the same plane twice: the triangle LAST in the model wins, as Bend's lists are last first and a tie keeps the first
        let (r, g) = (quad(1.0, 0.0, 0xff0000), quad(1.0, 0.0, 0x00ff00));
        let m: Vec<Tri3> = r.iter().chain(g.iter()).copied().collect();
        let i = render(200, 200, &m, &front(), GROUND); let c = px(&i, 200, 100, 100); assert!(c & 0xff00 != 0 && c >> 16 == 0, "{c:x}");
        // across a block boundary too (the block lists join last block first)
        let mut m = vec![]; for k in 0..BLOCK + 2 { m.push(tri([-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], if k == BLOCK + 1 { 0x0000ff } else { 0xff0000 })) }
        let i = render(200, 200, &m, &front(), GROUND); let c = px(&i, 200, 130, 110); assert!(c & 0xff != 0 && c >> 8 == 0, "{c:x}");
    }
    #[test] fn a_back_face_is_culled_and_a_translucent_one_blends() {
        let back = [tri([-1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [1.0, -1.0, 0.0], 0xffffff)];
        assert_eq!(px(&render(200, 200, &back, &front(), GROUND), 200, 130, 110), GROUND);
        let glass = quad(1.0, 1.0, 0x80ff0000); let wall = quad(1.0, 0.0, 0x00ff00);
        let m: Vec<Tri3> = glass.iter().chain(wall.iter()).copied().collect();
        let c = px(&render(200, 200, &m, &front(), GROUND), 200, 100, 100); assert!(c >> 16 > 0 && (c >> 8) & 255 > 0, "{c:x}");
    }
    #[test] fn a_triangle_through_the_near_plane_is_cut() {
        // from behind the eye to in front of it: only the part in front draws, and nothing panics or wraps
        let t = [tri([-3.0, -3.0, 10.0], [3.0, -3.0, -3.0], [0.0, 3.0, -3.0], 0xffffff)];
        let i = render(100, 100, &t, &front(), GROUND); assert!(i.chunks(3).any(|p| p != [0x20, 0x20, 0x28]));
    }
    #[test] fn the_picture_does_not_depend_on_the_deal() {
        let mut m = vec![]; for k in 0..40000u32 { let f = k as f32 * 0.01; m.push(tri([f.sin(), f.cos(), f * 0.001], [f.sin() + 0.1, f.cos(), f * 0.001], [f.sin(), f.cos() + 0.1, f * 0.001], k.wrapping_mul(2654435761) & 0xffffff)) }
        let a = render(320, 240, &m, &front(), GROUND); for _ in 0..3 { assert_eq!(render(320, 240, &m, &front(), GROUND), a) }
    }
    #[test] fn a_fitted_camera_turns_about_the_box_centre() {
        let c = fit([-1.0, 0.0, 2.0], [3.0, 4.0, 2.0]); assert_eq!(c.p, [1.0, 2.0, 2.0]);
        let r = (2.0f32 * 2.0 + (2.0 * 2.0 + 0.0)).sqrt(); assert_eq!(c.dist, 1.15 * (r / fl::sin(0.7 * 0.5)));
        let f = c.face(1.0, 0.5); assert_eq!((f.p, f.dist, f.fov), (c.p, c.dist, c.fov));
    }
    // law cam_fit_turns_about: a camera fitted about a point turns about that point, whatever it shows
    #[test] fn cam_fit_turns_about() {
        let mut s = 12345u32; let mut r = || { s = s.wrapping_mul(1664525).wrapping_add(1013904223); (s >> 8) as f32 / 65536.0 - 128.0 };
        for _ in 0..1000 {
            let (lo, hi, p) = ([r(), r(), r()], [r(), r(), r()], [r(), r(), r()]);
            let c = Cam::at(r(), r(), [r(), r(), r()], r().abs() + 1.0, 0.7);
            let f = of_box_at(lo, hi, p, c); assert_eq!(f.p, p); assert_eq!((f.r, f.u, f.fov), (c.r, c.u, c.fov));
        }
    }
    #[test] fn frames_are_powers_of_two_over_the_longest_side() { assert_eq!((frame_depth(800, 600), frame_depth(1024, 1), frame_depth(1025, 1), frame_depth(64, 64)), (10, 10, 11, 6)); }
    #[test] fn edge_constants_and_mix() { assert_eq!(mix(0xff0000, 0x0000ff, 255), 0xff0000); assert_eq!(mix(0xff0000, 0x0000ff, 0), 0x0000ff); assert_eq!(mix(0xffffff, 0, 128), 0x808080); }
}

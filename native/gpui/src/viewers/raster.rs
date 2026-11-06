//! The 3D page's software renderer. Three things make it fast and clean:
//!   * smooth vertex normals, welded by position and creased by angle (`smooth_normals`), so a curved part shades as a curve, not as facets;
//!   * a tiled, deferred rasterizer: triangles are projected once on all cores, binned into 48 px tiles, each tile is drawn by one core into a
//!     visibility buffer (depth + triangle id), and only the visible pixels are lit, once each (no overdraw shading);
//!   * watertight edges: vertices are snapped to 1/16 pixel and tested with exact integer edge functions and the top-left rule, so no pixel is
//!     lost or drawn twice along a shared edge (no pinholes, no cracks), and supersampling (`ss` x `ss` samples a pixel) smooths the outlines.
use super::solid::{Camera, Mesh, FOV};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

pub const BG: [u8; 4] = [18, 15, 13, 255]; // BGRA of the page background (#0d0f12)
const TILE: i32 = 48; // divisible by 1, 2 and 3: a tile is a whole number of output pixels at any supersampling
/// two faces smooth into one surface when their normals are within about 55 degrees (a hexagon's 60 stays sharp, a circle's 45 steps blend)
const CREASE_COS: f32 = 0.574;

type V3 = [f32; 3];
fn dot(a: V3, b: V3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn norm(a: V3) -> V3 { let l = dot(a, a).sqrt(); if l > 1e-20 { [a[0] / l, a[1] / l, a[2] / l] } else { [0.0, 0.0, 1.0] } }

// ---- smooth normals ---------------------------------------------------------------------------------------------------
#[derive(Default)] struct Fx(u64);
impl Hasher for Fx {
    fn finish(&self) -> u64 { self.0 }
    fn write(&mut self, bytes: &[u8]) { for b in bytes { self.write_u64(*b as u64) } }
    fn write_i32(&mut self, v: i32) { self.write_u64(v as u32 as u64) }
    fn write_u64(&mut self, v: u64) { self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x517c_c1b7_2722_0a95); }
}

/// A normal for each corner of each triangle. `keys` is the integer position of each corner (3 a triangle, in order): corners with the same
/// key are one vertex. A corner's normal is the angle-weighted mean of the faces around its vertex that lie within the crease angle of its own face.
pub fn smooth_normals(tris: &[[f32; 9]], keys: &[[i32; 3]]) -> Vec<[f32; 9]> {
    let n = tris.len();
    let mut ids: HashMap<[i32; 3], u32, BuildHasherDefault<Fx>> = HashMap::with_capacity_and_hasher(n * 3 / 2 + 16, Default::default());
    let vid: Vec<u32> = keys.iter().map(|k| { let next = ids.len() as u32; *ids.entry(*k).or_insert(next) }).collect();
    let nv = ids.len(); drop(ids);
    // face normals and the angle each face makes at each of its corners
    let mut face = Vec::with_capacity(n); let mut ang = Vec::with_capacity(n * 3);
    for t in tris {
        let p = [[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]];
        let c = cross(sub(p[1], p[0]), sub(p[2], p[0]));
        face.push(if dot(c, c) > 1e-24 { norm(c) } else { [0.0; 3] });
        for k in 0..3 {
            let (e1, e2) = (sub(p[(k + 1) % 3], p[k]), sub(p[(k + 2) % 3], p[k]));
            let (cr, d) = (cross(e1, e2), dot(e1, e2));
            ang.push(dot(cr, cr).sqrt().atan2(d));
        }
    }
    // corners around each vertex (CSR)
    let mut start = vec![0u32; nv + 1];
    for v in &vid { start[*v as usize + 1] += 1; }
    for i in 0..nv { start[i + 1] += start[i]; }
    let mut fill = start.clone(); let mut around = vec![0u32; vid.len()];
    for (c, v) in vid.iter().enumerate() { let at = &mut fill[*v as usize]; around[*at as usize] = c as u32; *at += 1; }
    let mut out = vec![[0f32; 9]; n];
    for c in 0..vid.len() {
        let (t, k) = (c / 3, c % 3); let own = face[t];
        let v = vid[c] as usize; let list = &around[start[v] as usize..start[v + 1] as usize];
        let mut acc = [0f32; 3];
        if list.len() <= 64 && dot(own, own) > 0.0 {
            for &d in list { let d = d as usize; let f = face[d / 3]; if dot(f, own) >= CREASE_COS { for a in 0..3 { acc[a] += f[a] * ang[d]; } } }
        }
        let nrm = if dot(acc, acc) > 1e-20 { norm(acc) } else { own };
        out[t][3 * k..3 * k + 3].copy_from_slice(&nrm);
    }
    out
}

// ---- projection ---------------------------------------------------------------------------------------------------------
/// A screen triangle: vertices in 1/16 pixel, wound so every edge function is positive inside; 1/z at the vertices; camera-space unit normals
/// (i16 fixed point); its colour; the pixel box it can touch.
#[derive(Clone, Copy)]
struct Setup { v: [[i32; 2]; 3], iz: [f32; 3], n: [[i16; 3]; 3], color: u32, bx0: i32, by0: i32, bx1: i32, by1: i32 }
struct Geo { eye: V3, right: V3, up: V3, fwd: V3, near: f32, cx: f32, cy: f32, scale: f32, w: i32, h: i32 }
const SUB: f32 = 16.0;
fn floor_div(a: i64, b: i64) -> i64 { a.div_euclid(b) }
fn ceil_div(a: i64, b: i64) -> i64 { -((-a).div_euclid(b)) }

fn quant(n: V3) -> [i16; 3] { let n = norm(n); [(n[0] * 32767.0).round() as i16, (n[1] * 32767.0).round() as i16, (n[2] * 32767.0).round() as i16] }

fn emit(g: &Geo, p: [(V3, V3); 3], color: u32, out: &mut Vec<Setup>) {
    let mut s = [[0f32; 2]; 3]; let mut iz = [0f32; 3];
    for k in 0..3 { let z = p[k].0[2]; iz[k] = 1.0 / z; s[k] = [g.cx + p[k].0[0] * iz[k] * g.scale, g.cy - p[k].0[1] * iz[k] * g.scale]; }
    let (minx, maxx) = (s[0][0].min(s[1][0]).min(s[2][0]), s[0][0].max(s[1][0]).max(s[2][0]));
    let (miny, maxy) = (s[0][1].min(s[1][1]).min(s[2][1]), s[0][1].max(s[1][1]).max(s[2][1]));
    if !(maxx >= 0.0 && minx <= g.w as f32 && maxy >= 0.0 && miny <= g.h as f32) { return } // also false for NaN
    let lim = 4.0e6;
    let q = |v: [f32; 2]| [(v[0].clamp(-lim, lim) * SUB).round() as i32, (v[1].clamp(-lim, lim) * SUB).round() as i32];
    let mut v = [q(s[0]), q(s[1]), q(s[2])];
    let (n0, mut n1, mut n2) = (quant(p[0].1), quant(p[1].1), quant(p[2].1));
    let (mut z1, mut z2) = (iz[1], iz[2]);
    let area = (v[1][0] - v[0][0]) as i64 * (v[2][1] - v[0][1]) as i64 - (v[1][1] - v[0][1]) as i64 * (v[2][0] - v[0][0]) as i64;
    if area == 0 { return }
    if area < 0 { v.swap(1, 2); std::mem::swap(&mut n1, &mut n2); std::mem::swap(&mut z1, &mut z2); }
    let (mnx, mxx) = (v[0][0].min(v[1][0]).min(v[2][0]) as i64, v[0][0].max(v[1][0]).max(v[2][0]) as i64);
    let (mny, mxy) = (v[0][1].min(v[1][1]).min(v[2][1]) as i64, v[0][1].max(v[1][1]).max(v[2][1]) as i64);
    // pixel centres 16x + 8 inside [min, max]
    let (bx0, bx1) = (ceil_div(mnx - 8, 16).max(0), floor_div(mxx - 8, 16).min(g.w as i64 - 1));
    let (by0, by1) = (ceil_div(mny - 8, 16).max(0), floor_div(mxy - 8, 16).min(g.h as i64 - 1));
    if bx0 > bx1 || by0 > by1 { return }
    out.push(Setup { v, iz: [iz[0], z1, z2], n: [n0, n1, n2], color, bx0: bx0 as i32, by0: by0 as i32, bx1: bx1 as i32, by1: by1 as i32 });
}

fn project(mesh: &Mesh, lo: usize, hi: usize, g: &Geo, out: &mut Vec<Setup>) {
    for i in lo..hi {
        let (t, nm, color) = (&mesh.tris[i], &mesh.normals[i], mesh.colors[i]);
        let mut pv = [([0f32; 3], [0f32; 3]); 3];
        for k in 0..3 {
            let d = sub([t[3 * k], t[3 * k + 1], t[3 * k + 2]], g.eye);
            let n = [nm[3 * k], nm[3 * k + 1], nm[3 * k + 2]];
            pv[k] = ([dot(d, g.right), dot(d, g.up), dot(d, g.fwd)], [dot(n, g.right), dot(n, g.up), dot(n, g.fwd)]);
        }
        if pv.iter().all(|p| p.0[2] >= g.near) { emit(g, pv, color, out); continue }
        // clip against z >= near: 0, 1 or 2 triangles
        let mut poly = [([0f32; 3], [0f32; 3]); 4]; let mut m = 0;
        for k in 0..3 {
            let (p, q) = (pv[k], pv[(k + 1) % 3]);
            let (pin, qin) = (p.0[2] >= g.near, q.0[2] >= g.near);
            if pin { poly[m] = p; m += 1; }
            if pin != qin {
                let s = (g.near - p.0[2]) / (q.0[2] - p.0[2]);
                let l = |a: V3, b: V3| [a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s, a[2] + (b[2] - a[2]) * s];
                let mut pos = l(p.0, q.0); pos[2] = g.near;
                poly[m] = (pos, l(p.1, q.1)); m += 1;
            }
        }
        for k in 1..m.saturating_sub(1) { emit(g, [poly[0], poly[k], poly[k + 1]], color, out); }
    }
}

// ---- lighting -----------------------------------------------------------------------------------------------------------
struct Light { key: V3, fill: V3, up: V3, bg: [f32; 3] }
fn lin(c: u32) -> f32 { let v = c as f32 / 255.0; v * v } // gamma 2 stands for sRGB: cheap, and averages of samples are done in linear light

/// The colour (linear) of a surface point: `n` camera-space unit normal, `ray` the unit direction from the eye to the point.
#[inline]
fn light(l: &Light, n: V3, ray: V3, albedo: [f32; 3]) -> [f32; 3] {
    let v = [-ray[0], -ray[1], -ray[2]];
    let n = if dot(n, v) < 0.0 { [-n[0], -n[1], -n[2]] } else { n }; // two-sided: boards are thin, and a back face shows the same as a front
    let nv = dot(n, v).max(0.0);
    let wrap = |d: f32| ((d + 0.25) / 1.25).clamp(0.0, 1.0);
    let (d1, d2) = (wrap(dot(n, l.key)), wrap(dot(n, l.fill)));
    let sky = 0.5 + 0.5 * dot(n, l.up);
    let amb = 0.16 + 0.22 * sky;
    let hv = norm([l.key[0] + v[0], l.key[1] + v[1], l.key[2] + v[2]]);
    let mut s = dot(n, hv).max(0.0); s *= s; s *= s; s *= s; s *= s; s *= s; // ^32
    let spec = 0.20 * s * d1;
    let rim = { let f = 1.0 - nv; f * f * f * 0.10 * sky };
    let k = amb + 0.78 * d1 + 0.22 * d2;
    [(albedo[0] * k + spec + rim).min(1.0), (albedo[1] * k + spec + rim).min(1.0), (albedo[2] * k + spec + rim).min(1.0)]
}

// ---- tiles ------------------------------------------------------------------------------------------------------------------
fn draw_tile(tx: i32, ty: i32, ids: &[u32], set: &[Setup], g: &Geo, ss: i32, l: &Light) -> Vec<u8> {
    let (x0t, y0t) = (tx * TILE, ty * TILE);
    let (tw, th) = (TILE.min(g.w - x0t), TILE.min(g.h - y0t));
    let mut zb = [0f32; (TILE * TILE) as usize]; let mut idb = [u32::MAX; (TILE * TILE) as usize];
    for &id in ids {
        let s = &set[id as usize];
        let (bx0, bx1, by0, by1) = (s.bx0.max(x0t), s.bx1.min(x0t + tw - 1), s.by0.max(y0t), s.by1.min(y0t + th - 1));
        if bx0 > bx1 || by0 > by1 { continue }
        let mut e = [0i64; 3]; let mut sx = [0i64; 3]; let mut sy = [0i64; 3];
        let (px, py) = (bx0 as i64 * 16 + 8, by0 as i64 * 16 + 8);
        for k in 0..3 {
            let (a, b) = (s.v[k], s.v[(k + 1) % 3]);
            let (dx, dy) = ((b[0] - a[0]) as i64, (b[1] - a[1]) as i64);
            let own = dy > 0 || (dy == 0 && dx > 0); // a shared edge belongs to exactly one of its two triangles
            e[k] = dx * (py - a[1] as i64) - dy * (px - a[0] as i64) - if own { 0 } else { 1 };
            sx[k] = -dy * 16; sy[k] = dx * 16;
        }
        // 1/z is linear on the screen
        let f = |i: usize, c: usize| s.v[i][c] as f32 / SUB;
        let (ex1, ey1, ex2, ey2) = (f(1, 0) - f(0, 0), f(1, 1) - f(0, 1), f(2, 0) - f(0, 0), f(2, 1) - f(0, 1));
        let (d1, d2) = (s.iz[1] - s.iz[0], s.iz[2] - s.iz[0]);
        let inv = 1.0 / (ex1 * ey2 - ex2 * ey1);
        let (gx, gy) = ((d1 * ey2 - d2 * ey1) * inv, (d2 * ex1 - d1 * ex2) * inv);
        let z00 = s.iz[0] + gx * (bx0 as f32 + 0.5 - f(0, 0)) + gy * (by0 as f32 + 0.5 - f(0, 1));
        for y in by0..=by1 {
            let dy = (y - by0) as i64; let dyf = (y - by0) as f32;
            let (mut e0, mut e1, mut e2) = (e[0] + sy[0] * dy, e[1] + sy[1] * dy, e[2] + sy[2] * dy);
            let row = ((y - y0t) * TILE) as usize;
            for x in bx0..=bx1 {
                if (e0 | e1 | e2) >= 0 {
                    let z = z00 + gx * (x - bx0) as f32 + gy * dyf; let i = row + (x - x0t) as usize;
                    if z > zb[i] { zb[i] = z; idb[i] = id; }
                }
                e0 += sx[0]; e1 += sx[1]; e2 += sx[2];
            }
        }
    }
    // shade what is visible, average the samples of each output pixel in linear light
    let (ow, oh) = ((tw / ss) as usize, (th / ss) as usize);
    let mut acc = vec![[0f32; 3]; ow * oh];
    for y in 0..th { for x in 0..tw {
        let i = (y * TILE + x) as usize; let o = (y / ss) as usize * ow + (x / ss) as usize;
        let c = if idb[i] == u32::MAX { l.bg } else {
            let s = &set[idb[i] as usize];
            let (sxp, syp) = ((x0t + x) as f32 + 0.5, (y0t + y) as f32 + 0.5);
            let (pxs, pys) = (((x0t + x) as i64) * 16 + 8, ((y0t + y) as i64) * 16 + 8);
            let mut w = [0f32; 3];
            for k in 0..3 { // the edge opposite vertex (k + 2) % 3 is edge k
                let (a, b) = (s.v[k], s.v[(k + 1) % 3]);
                let ev = (b[0] - a[0]) as i64 * (pys - a[1] as i64) - (b[1] - a[1]) as i64 * (pxs - a[0] as i64);
                w[(k + 2) % 3] = (ev as f32).max(0.0) * s.iz[(k + 2) % 3];
            }
            let sum = (w[0] + w[1] + w[2]).max(1e-30);
            let mut n = [0f32; 3];
            for k in 0..3 { let f = w[k] / sum / 32767.0; for a in 0..3 { n[a] += s.n[k][a] as f32 * f; } }
            let ray = norm([(sxp - g.cx) / g.scale, -(syp - g.cy) / g.scale, 1.0]);
            let albedo = [lin(s.color & 255), lin((s.color >> 8) & 255), lin((s.color >> 16) & 255)];
            light(l, norm(n), ray, albedo)
        };
        for a in 0..3 { acc[o][a] += c[a]; }
    } }
    let inv = 1.0 / (ss * ss) as f32;
    let mut px = Vec::with_capacity(ow * oh * 4);
    for c in acc { px.extend_from_slice(&[(c[2] * inv).sqrt().mul_add(255.0, 0.5) as u8, (c[1] * inv).sqrt().mul_add(255.0, 0.5) as u8, (c[0] * inv).sqrt().mul_add(255.0, 0.5) as u8, 255]); }
    px
}

/// Render into BGRA (w x h x 4) with `ss` x `ss` samples a pixel (1 for a draft while the model is moving). `alive` is polled between pieces of
/// work: a superseded render stops and returns None.
pub fn render(mesh: &Mesh, cam: &Camera, w: usize, h: usize, ss: usize, alive: &(dyn Fn() -> bool + Sync)) -> Option<Vec<u8>> {
    let ss = ss.clamp(1, 3) as i32;
    let (iw, ih) = (w as i32 * ss, h as i32 * ss);
    let f = 1.0 / (FOV / 2.0).tan();
    let g = Geo { eye: cam.eye(), right: cam.right, up: cam.up, fwd: cam.fwd, near: (cam.dist * 0.02).max(1e-3), cx: iw as f32 / 2.0, cy: ih as f32 / 2.0, scale: ih as f32 / 2.0 * f, w: iw, h: ih };
    let z = [0.0, 0.0, 1.0];
    let l = Light { key: norm([-0.45, 0.70, -0.55]), fill: norm([0.75, -0.15, -0.45]), up: norm([dot(z, cam.right), dot(z, cam.up), dot(z, cam.fwd)]),
        bg: [lin(BG[2] as u32), lin(BG[1] as u32), lin(BG[0] as u32)] };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    // 1: project, in chunks over the cores
    const CHUNK: usize = 16384;
    let nchunks = mesh.tris.len().div_ceil(CHUNK);
    let next = AtomicUsize::new(0); let done: Mutex<Vec<(usize, Vec<Setup>)>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| { for _ in 0..threads.min(nchunks.max(1)) { sc.spawn(|| { loop {
        let c = next.fetch_add(1, Ordering::Relaxed); if c >= nchunks || !alive() { return }
        let mut out = Vec::with_capacity(CHUNK / 2);
        project(mesh, c * CHUNK, ((c + 1) * CHUNK).min(mesh.tris.len()), &g, &mut out);
        done.lock().unwrap().push((c, out));
    } }); } });
    if !alive() { return None }
    let mut parts = done.into_inner().unwrap(); parts.sort_by_key(|p| p.0);
    let set: Vec<Setup> = parts.into_iter().flat_map(|p| p.1).collect();
    // 2: bin into tiles
    let (tcols, trows) = ((iw + TILE - 1) / TILE, (ih + TILE - 1) / TILE);
    let mut bins: Vec<Vec<u32>> = vec![Vec::new(); (tcols * trows) as usize];
    for (i, s) in set.iter().enumerate() {
        for ty in s.by0 / TILE..=s.by1 / TILE { for tx in s.bx0 / TILE..=s.bx1 / TILE { bins[(ty * tcols + tx) as usize].push(i as u32); } }
    }
    // 3: draw the tiles over the cores
    let next = AtomicUsize::new(0); let tiles: Mutex<Vec<(usize, Vec<u8>)>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| { for _ in 0..threads { sc.spawn(|| { loop {
        let t = next.fetch_add(1, Ordering::Relaxed); if t >= bins.len() || !alive() { return }
        let px = draw_tile(t as i32 % tcols, t as i32 / tcols, &bins[t], &set, &g, ss, &l);
        tiles.lock().unwrap().push((t, px));
    } }); } });
    if !alive() { return None }
    let mut out = vec![0u8; w * h * 4];
    let tile_px = (TILE / ss) as usize;
    for (t, px) in tiles.into_inner().unwrap() {
        let (x0, y0) = ((t as i32 % tcols) as usize * tile_px, (t as i32 / tcols) as usize * tile_px);
        let tw = tile_px.min(w - x0);
        for (r, row) in px.chunks_exact(tw * 4).enumerate() { let at = ((y0 + r) * w + x0) * 4; out[at..at + tw * 4].copy_from_slice(row); }
    }
    Some(out)
}

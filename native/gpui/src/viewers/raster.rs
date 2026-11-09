//! The 3D page's software renderer, for a machine without a usable GPU (solid_gpu.rs draws the same picture on one). Three things make it fast and clean:
//!   * smooth vertex normals, welded by position and creased by angle (`smooth_normals`), so a curved part shades as a curve, not as facets;
//!   * a tiled, deferred rasterizer: triangles are projected once on all cores, binned into 48 px tiles, each tile is drawn by one core into a
//!     visibility buffer (depth + triangle id), and only the visible pixels are lit, once each (no overdraw shading);
//!   * watertight edges: vertices are snapped to 1/16 pixel and tested with exact integer edge functions and the top-left rule, so no pixel is
//!     lost or drawn twice along a shared edge (no pinholes, no cracks), and supersampling (`ss` x `ss` samples a pixel) smooths the outlines.
//! A colour is 0xTTRRGGBB in linear light (TT transparency): the wire's meaning, what the phones light too. sRGB is applied once, on the way out (`to_srgb`).
use super::solid::{Camera, Mesh, FOV};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// The page's background: a vertical gradient between two sRGB colours (docs/VIEWER-BACKGROUND.md), top to bottom.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Backdrop { pub top: [u8; 3], pub bottom: [u8; 3] }
static CURRENT: Mutex<Backdrop> = Mutex::new(Backdrop::DARK);
impl Backdrop {
    /// The backdrop the window drew its last frame with, for what is shown before a picture exists (the loading card).
    pub fn current() -> Backdrop { *CURRENT.lock().unwrap_or_else(|e| e.into_inner()) }
    pub fn make_current(self) { *CURRENT.lock().unwrap_or_else(|e| e.into_inner()) = self; }
    pub const LIGHT: Backdrop = Backdrop { top: [0xf5, 0xf6, 0xf8], bottom: [0xe1, 0xe4, 0xe9] };
    pub const DARK: Backdrop = Backdrop { top: [0x4d, 0x53, 0x5e], bottom: [0x35, 0x3a, 0x43] };
    /// What a setting resolves to: `mode` auto (the theme), light, dark or custom (`rgb` 0xRRGGBB, its bottom 12% darker).
    pub fn resolve(mode: &str, rgb: u32, light_theme: bool) -> Backdrop {
        match mode {
            "light" => Backdrop::LIGHT, "dark" => Backdrop::DARK,
            "custom" => { let c = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]; Backdrop { top: c, bottom: c.map(|v| (v as f32 * 0.88).round() as u8) } }
            _ => if light_theme { Backdrop::LIGHT } else { Backdrop::DARK },
        }
    }
    /// Both ends in linear light (red, green, blue), the space the picture is mixed in.
    pub fn linear(&self) -> ([f32; 3], [f32; 3]) { (self.top.map(to_linear), self.bottom.map(to_linear)) }
    /// The colour (RGB, sRGB bytes) at `t` from the top (0) to the bottom (1), mixed in linear light as the renderers do.
    pub fn at(&self, t: f32) -> [u8; 3] { let (a, b) = self.linear(); std::array::from_fn(|i| to_srgb(a[i] + (b[i] - a[i]) * t)) }
}
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
struct Setup { v: [[i32; 2]; 3], iz: [f32; 3], n: [[i16; 3]; 3], color: u32, fade: f32, bx0: i32, by0: i32, bx1: i32, by1: i32 }
struct Geo { eye: V3, right: V3, up: V3, fwd: V3, near: f32, cx: f32, cy: f32, scale: f32, w: i32, h: i32 }
const SUB: f32 = 16.0;
fn floor_div(a: i64, b: i64) -> i64 { a.div_euclid(b) }
fn ceil_div(a: i64, b: i64) -> i64 { -((-a).div_euclid(b)) }

fn quant(n: V3) -> [i16; 3] { let n = norm(n); [(n[0] * 32767.0).round() as i16, (n[1] * 32767.0).round() as i16, (n[2] * 32767.0).round() as i16] }

fn emit(g: &Geo, p: [(V3, V3); 3], color: u32, fade: f32, out: &mut Vec<Setup>) {
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
    out.push(Setup { v, iz: [iz[0], z1, z2], n: [n0, n1, n2], color, fade, bx0: bx0 as i32, by0: by0 as i32, bx1: bx1 as i32, by1: by1 as i32 });
}

/// A triangle's fade (0..1): its group's in `fades`, 1 for the board (group 0) and for a mesh without groups.
pub fn fade_of(mesh: &Mesh, fades: &[f32], i: usize) -> f32 { match mesh.groups.get(i) { Some(&g) if g > 0 => fades.get(g as usize).copied().unwrap_or(1.0).clamp(0.0, 1.0), _ => 1.0 } }
fn project(mesh: &Mesh, fades: &[f32], lo: usize, hi: usize, g: &Geo, out: &mut Vec<Setup>) {
    for i in lo..hi {
        let (t, nm, color) = (&mesh.tris[i], &mesh.normals[i], mesh.colors[i]);
        let fade = fade_of(mesh, fades, i);
        if fade <= 0.0 { continue } // a part that has only just arrived: not there yet
        let mut pv = [([0f32; 3], [0f32; 3]); 3];
        for k in 0..3 {
            let d = sub([t[3 * k], t[3 * k + 1], t[3 * k + 2]], g.eye);
            let n = [nm[3 * k], nm[3 * k + 1], nm[3 * k + 2]];
            pv[k] = ([dot(d, g.right), dot(d, g.up), dot(d, g.fwd)], [dot(n, g.right), dot(n, g.up), dot(n, g.fwd)]);
        }
        if pv.iter().all(|p| p.0[2] >= g.near) { emit(g, pv, color, fade, out); continue }
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
        for k in 1..m.saturating_sub(1) { emit(g, [poly[0], poly[k], poly[k + 1]], color, fade, out); }
    }
}

// ---- lighting -----------------------------------------------------------------------------------------------------------
/// The material model of solid.wgsl, in camera space (x right, y up, z forward): the test `gpu_and_cpu_agree` holds the two together.
struct Light { key: V3, fill: V3, sky_key: V3, sky_fill: V3, top: [f32; 3], bottom: [f32; 3], rows: f32 }
impl Light { fn bg(&self, y: f32) -> [f32; 3] { let t = y / self.rows; std::array::from_fn(|i| self.top[i] + (self.bottom[i] - self.top[i]) * t) } }
/// An sRGB byte as linear light.
pub fn to_linear(c: u8) -> f32 { let v = c as f32 / 255.0; if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) } }
/// Linear light as an sRGB byte, through a table: this runs for every pixel.
pub fn to_srgb(x: f32) -> u8 {
    static TABLE: OnceLock<Vec<u8>> = OnceLock::new();
    let t = TABLE.get_or_init(|| (0..4096).map(|i| { let l = i as f32 / 4095.0; let e = if l <= 0.0031308 { 12.92 * l } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }; (e * 255.0 + 0.5) as u8 }).collect());
    t[(x.clamp(0.0, 1.0) * 4095.0 + 0.5) as usize]
}
/// A wire colour's red, green, blue: the bytes are light already, nothing to decode.
fn albedo(c: u32) -> [f32; 3] { [((c >> 16) & 255) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0] }
/// Roughness, metalness, coat, guessed from a colour: the wire carries no more.
fn estimate(c: [f32; 3]) -> [f32; 3] {
    let (hi, lo) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    if hi < 0.12 && hi - lo < (hi * 0.45).max(0.015) { return [0.63, 0.0, 0.12] }
    if lo > 0.9 { return [0.82, 0.0, 0.0] }
    if c[0] > 0.18 && c[1] > c[0] * 0.55 && c[1] < c[0] * 1.02 && c[2] < c[1] * 0.45 { return [0.3, 0.86, 0.0] }
    if hi - lo < hi * 0.16 && lo > 0.18 && hi < 0.9 { return [0.28, 0.82, 0.0] }
    if c[0] > c[2] * 1.25 && c[1] > c[2] * 1.12 && c[0] > 0.06 { return [0.72, 0.0, 0.0] }
    if hi - lo > (hi * 0.28).max(0.015) { return [0.36, 0.0, 0.35] }
    [0.55, 0.0, 0.0]
}
fn mix(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }
fn smoothstep(a: f32, b: f32, x: f32) -> f32 { let t = ((x - a) / (b - a)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }
fn fresnel(f0: [f32; 3], h: f32) -> [f32; 3] { let k = (1.0 - h).powi(5); [f0[0] + (1.0 - f0[0]) * k, f0[1] + (1.0 - f0[1]) * k, f0[2] + (1.0 - f0[2]) * k] }
fn lit(n: V3, v: V3, l: V3, base: [f32; 3], f0: [f32; 3], rough: f32, metal: f32) -> [f32; 3] {
    let h = norm([v[0] + l[0], v[1] + l[1], v[2] + l[2]]);
    let (nl, nv, nh) = (dot(n, l).max(0.0), dot(n, v).max(0.001), dot(n, h).max(0.0));
    let (a2, k) = { let a = rough * rough; (a * a, (rough + 1.0) * (rough + 1.0) / 8.0) };
    let d = nh * nh * (a2 - 1.0) + 1.0;
    let dd = a2 / (std::f32::consts::PI * d * d).max(0.0001);
    let gg = (nl / (nl * (1.0 - k) + k)) * (nv / (nv * (1.0 - k) + k));
    let f = fresnel(f0, dot(v, h).max(0.0));
    let spec = dd * gg / (4.0 * nl * nv).max(0.001);
    std::array::from_fn(|i| (base[i] * (1.0 - metal) * (1.0 - f[i]) / std::f32::consts::PI + f[i] * spec) * nl)
}
fn studio(l: &Light, r: V3, rough: f32, metal: f32) -> [f32; 3] {
    let q = [r[0], r[1], -r[2]];
    let power = mix(110.0, 3.0, rough * rough);
    let (key, fill) = (dot(q, l.sky_key).max(0.0).powf(power), dot(q, l.sky_fill).max(0.0).powf(power * 0.65));
    let sky = (0.12 + 0.2 * smoothstep(-0.4, 0.25, q[1]) + 0.7 * smoothstep(0.45, 0.85, q[1])) * mix(0.15, 1.0, metal);
    [sky + 1.5 * key + 0.65 * fill, sky + 1.48 * key + 0.72 * fill, sky + 1.42 * key + 0.85 * fill]
}
/// The colour (linear) of a surface point: `n` camera-space unit normal, `ray` the unit direction from the eye to the point, `c` its wire colour.
#[inline]
fn light(l: &Light, n: V3, ray: V3, c: u32) -> [f32; 3] {
    let base = albedo(c);
    let mut m = estimate(base);
    if c >> 24 != 0 && m[0] < 0.8 { m = [0.24, 0.0, 1.0]; } // a translucent coat that is not silk is the soldermask: glossy, the person's choice (2026-10-31)
    let [rough, metal, coat] = m;
    let v = [-ray[0], -ray[1], -ray[2]];
    let n = if dot(n, v) < 0.0 { [-n[0], -n[1], -n[2]] } else { n }; // two-sided: boards are thin, and a back face shows the same as a front
    let nv = dot(n, v).max(0.0);
    let f0: [f32; 3] = std::array::from_fn(|i| mix(0.04, base[i], metal));
    let amb = (1.0 - metal) * (if rough >= 0.8 { 0.80 } else { 0.34 } + 0.10 * n[1].max(0.0));
    let (k, f) = (lit(n, v, l.key, base, f0, rough, metal), lit(n, v, l.fill, base, f0, rough, metal));
    let d = 2.0 * dot(n, v); let r = [n[0] * d - v[0], n[1] * d - v[1], n[2] * d - v[2]];
    let (env, fe) = (studio(l, r, rough, metal), fresnel(f0, nv));
    let (gloss, fc) = if coat > 0.0 { (studio(l, r, 0.3, 0.0), fresnel([0.04; 3], nv)) } else { ([0.0; 3], [0.0; 3]) };
    std::array::from_fn(|i| {
        let rgb = base[i] * amb + k[i] * 1.5 + f[i] * 0.85 + env[i] * fe[i] * (1.0 - rough * 0.35) + gloss[i] * fc[i] * coat;
        rgb / (1.0 + (rgb - 0.8).max(0.0) * 0.28) // a soft shoulder for highlights
    })
}

// ---- tiles ------------------------------------------------------------------------------------------------------------------
fn draw_tile(tx: i32, ty: i32, ids: &[u32], set: &[Setup], g: &Geo, ss: i32, l: &Light) -> Vec<u8> {
    let (x0t, y0t) = (tx * TILE, ty * TILE);
    let (tw, th) = (TILE.min(g.w - x0t), TILE.min(g.h - y0t));
    let mut zb = [0f32; (TILE * TILE) as usize]; let mut idb = [u32::MAX; (TILE * TILE) as usize];
    // the nearest translucent layer of each sample, as the hub's renderer keeps it: only that one shows, over what is opaque
    let mut tz = [0f32; (TILE * TILE) as usize]; let mut tid = [u32::MAX; (TILE * TILE) as usize];
    // the nearest fading part of each sample (a part model arriving): over everything else, its opacity its fade
    let mut fz = [0f32; (TILE * TILE) as usize]; let mut fid = [u32::MAX; (TILE * TILE) as usize];
    for &id in ids {
        let s = &set[id as usize]; let clear = s.color >> 24 != 0; let fading = s.fade < 1.0;
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
                    if fading { if z > fz[i] { fz[i] = z; fid[i] = id; } } else if clear { if z > tz[i] { tz[i] = z; tid[i] = id; } } else if z > zb[i] { zb[i] = z; idb[i] = id; }
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
        let surface = |id: u32| {
            let s = &set[id as usize];
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
            light(l, norm(n), ray, s.color)
        };
        let mut c = if idb[i] == u32::MAX { l.bg((y0t + y) as f32 + 0.5) } else { surface(idb[i]) };
        let coat = |c: &mut [f32; 3]| if tid[i] != u32::MAX && tz[i] > zb[i] {
            let (t, a) = (surface(tid[i]), 1.0 - (set[tid[i] as usize].color >> 24) as f32 / 255.0);
            for k in 0..3 { c[k] += (t[k] - c[k]) * a; }
        };
        let fade = |c: &mut [f32; 3]| if fid[i] != u32::MAX && fz[i] > zb[i] {
            let s = &set[fid[i] as usize]; let (t, a) = (surface(fid[i]), (1.0 - (s.color >> 24) as f32 / 255.0) * s.fade);
            for k in 0..3 { c[k] += (t[k] - c[k]) * a; }
        };
        // the nearer of the coat and the fading part goes over the other
        if fid[i] != u32::MAX && tid[i] != u32::MAX && tz[i] > fz[i] { fade(&mut c); coat(&mut c); } else { coat(&mut c); fade(&mut c); }
        for a in 0..3 { acc[o][a] += c[a]; }
    } }
    let inv = 1.0 / (ss * ss) as f32;
    let mut px = Vec::with_capacity(ow * oh * 4);
    for c in acc { px.extend_from_slice(&[to_srgb(c[2] * inv), to_srgb(c[1] * inv), to_srgb(c[0] * inv), 255]); }
    px
}

/// Render into BGRA (w x h x 4) with `ss` x `ss` samples a pixel (1 for a draft while the model is moving). `alive` is polled between pieces of
/// work: a superseded render stops and returns None.
pub fn render(mesh: &Mesh, cam: &Camera, w: usize, h: usize, ss: usize, alive: &(dyn Fn() -> bool + Sync)) -> Option<Vec<u8>> { render_faded(mesh, &[], cam, w, h, ss, &Backdrop::DARK, alive) }
/// `render` with each group of triangles (`Mesh::groups`) at its fade in `fades` (0 not drawn, under 1 drawn see-through over the rest, 1 as the board).
pub fn render_faded(mesh: &Mesh, fades: &[f32], cam: &Camera, w: usize, h: usize, ss: usize, back: &Backdrop, alive: &(dyn Fn() -> bool + Sync)) -> Option<Vec<u8>> {
    let ss = ss.clamp(1, 3) as i32;
    let (iw, ih) = (w as i32 * ss, h as i32 * ss);
    let f = 1.0 / (FOV / 2.0).tan();
    let g = Geo { eye: cam.eye(), right: cam.right, up: cam.up, fwd: cam.fwd, near: (cam.dist * 0.02).max(1e-3), cx: iw as f32 / 2.0, cy: ih as f32 / 2.0, scale: ih as f32 / 2.0 * f, w: iw, h: ih };
    let l = Light { key: norm([-0.55, 0.75, -0.65]), fill: norm([0.85, 0.25, -0.35]), sky_key: norm([-0.55, 0.75, 0.65]), sky_fill: norm([0.85, 0.25, 0.35]),
        top: back.linear().0, bottom: back.linear().1, rows: ih as f32 };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    // 1: project, in chunks over the cores
    const CHUNK: usize = 16384;
    let nchunks = mesh.tris.len().div_ceil(CHUNK);
    let next = AtomicUsize::new(0); let done: Mutex<Vec<(usize, Vec<Setup>)>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| { for _ in 0..threads.min(nchunks.max(1)) { sc.spawn(|| { loop {
        let c = next.fetch_add(1, Ordering::Relaxed); if c >= nchunks || !alive() { return }
        let mut out = Vec::with_capacity(CHUNK / 2);
        project(mesh, fades, c * CHUNK, ((c + 1) * CHUNK).min(mesh.tris.len()), &g, &mut out);
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

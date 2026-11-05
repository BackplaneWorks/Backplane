//! Board and schematic plots from the hub (core/plot.bend, as the phones draw them): a binary CBOR frame
//! `{t:"plot", key, fam, cs, box, edge, thick}` or `{key, none}`. `cs` lists each chunk as the index of one the client holds (the last
//! plot applied) or the chunk itself `{l layer, c colour, a alpha, m mode, w width, d varints}`; applying it is `Plot.apply`, so
//! `held` mirrors what the hub thinks this connection holds. Nothing is decided here: which chunks, their order, colour and
//! opacity come from the hub. Pure; decoded chunks are capsules (strokes) and fan triangles (fills), in micrometres.
use std::{collections::BTreeMap, sync::Arc};

/// A CBOR value as far as plots use it (no tags: a frame's dictionary words are plain keys 1 and 31 for "t" and "key").
#[derive(Clone, Debug, PartialEq)]
pub enum C { U(i64), Bytes(Vec<u8>), Text(String), Arr(Vec<C>), Map(BTreeMap<String, C>) }

impl C {
    pub fn get(&self, k: &str) -> Option<&C> { if let C::Map(m) = self { m.get(k) } else { None } }
    pub fn int(&self) -> Option<i64> { if let C::U(n) = self { Some(*n) } else { None } }
    pub fn text(&self) -> Option<&str> { if let C::Text(s) = self { Some(s) } else { None } }
}

const MAX_NODES: usize = 400_000;

/// Decode one CBOR item; None for anything outside the subset or truncated.
pub fn decode(b: &[u8]) -> Option<C> {
    struct R<'a> { b: &'a [u8], i: usize, nodes: usize }
    impl R<'_> {
        fn arg(&mut self, ai: u8) -> Option<usize> {
            Some(match ai {
                0..=23 => ai as usize,
                24 => { let v = *self.b.get(self.i)? as usize; self.i += 1; v }
                25 => { let v = (*self.b.get(self.i)? as usize) << 8 | *self.b.get(self.i + 1)? as usize; self.i += 2; v }
                26 => { let s = self.b.get(self.i..self.i + 4)?; self.i += 4; u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as usize }
                _ => return None,
            })
        }
        fn item(&mut self, depth: usize) -> Option<C> {
            self.nodes += 1;
            if depth > 32 || self.nodes > MAX_NODES { return None; }
            let h = *self.b.get(self.i)?; self.i += 1;
            let n = self.arg(h & 31)?;
            Some(match h >> 5 {
                0 => C::U(n as i64),
                1 => C::U(-1 - n as i64),
                2 => { let s = self.b.get(self.i..self.i.checked_add(n)?)?; self.i += n; C::Bytes(s.to_vec()) }
                3 => { let s = self.b.get(self.i..self.i.checked_add(n)?)?; self.i += n; C::Text(String::from_utf8_lossy(s).into_owned()) }
                4 => { if n > self.b.len() { return None; } let mut a = Vec::with_capacity(n.min(4096)); for _ in 0..n { a.push(self.item(depth + 1)?); } C::Arr(a) }
                5 => {
                    let mut m = BTreeMap::new();
                    for _ in 0..n {
                        let k = match self.item(depth + 1)? { C::Text(s) => s, C::U(1) => "t".into(), C::U(31) => "key".into(), C::U(k) => k.to_string(), _ => return None };
                        m.insert(k, self.item(depth + 1)?);
                    }
                    C::Map(m)
                }
                _ => return None,
            })
        }
    }
    let mut r = R { b, i: 0, nodes: 0 };
    let v = r.item(0)?;
    (r.i == b.len()).then_some(v)
}

/// Is this binary frame a plot? A map whose first key is 1 ("t") with the text "plot".
pub fn is_plot(d: &[u8]) -> bool { d.len() > 7 && (0xA0..=0xB7).contains(&d[0]) && d[1] == 1 && d[2..7] == [0x64, b'p', b'l', b'o', b't'] }

struct Varints<'a> { b: &'a [u8], i: usize, ok: bool }
impl Varints<'_> {
    fn more(&self) -> bool { self.ok && self.i < self.b.len() }
    fn next(&mut self) -> u32 {
        let mut acc = 0u32;
        for k in 0..5 {
            let Some(&v) = self.b.get(self.i) else { self.ok = false; return 0 };
            self.i += 1;
            let shift = 7 * k;
            if shift >= 28 && v & 0xF0 != 0 { self.ok = false; return 0 }
            acc |= ((v & 127) as u32) << shift;
            if v < 128 { return acc }
        }
        self.ok = false; 0
    }
    fn signed(&mut self) -> i32 { let z = self.next(); ((z >> 1) as i32) ^ -((z & 1) as i32) }
}

/// One chunk, decoded once: strokes as capsules (ax ay bx by r) and fills as fan triangles (6 floats each), micrometres.
#[derive(Debug, Default, PartialEq)]
pub struct Chunk { pub layer: u32, pub color: u32, pub alpha: f32, pub own: bool, pub caps: Vec<f32>, pub tris: Vec<f32>, pub polys: Vec<Vec<f32>> }

impl Chunk {
    pub fn decode(o: &C) -> Chunk {
        let g = |k: &str, d: i64| o.get(k).and_then(C::int).unwrap_or(d);
        let mode = g("m", 1);
        let mut c = Chunk { layer: g("l", 0) as u32, color: g("c", 0) as u32, alpha: g("a", 255) as f32 / 255., own: mode == 0 && g("w", 0) != 0, ..Default::default() };
        let r = g("w", 0) as f32 / 2.;
        let empty = Vec::new();
        let data = match o.get("d") { Some(C::Bytes(b)) => b, _ => &empty };
        let mut v = Varints { b: data, i: 0, ok: true };
        let (mut px, mut py) = (0i32, 0i32);
        while v.more() {
            let _info = v.next();
            let mut pts: Vec<f32> = Vec::new();
            let mut point = |v: &mut Varints| { px = px.wrapping_add(v.signed()); py = py.wrapping_add(v.signed()); (px as f32, py as f32) };
            if mode == 2 { let (x, y) = point(&mut v); pts = vec![x, y]; c.caps.extend([x, y, x, y, r]); }
            else {
                let n = v.next() as usize;
                for _ in 0..n.min(1 << 20) { if !v.more() { break; } let (x, y) = point(&mut v); pts.extend([x, y]); }
                if mode == 0 { fan(&pts, &mut c.tris); if pts.len() >= 6 { c.polys.push(pts.clone()); } } else { path(&pts, r, &mut c.caps); }
            }
        }
        c
    }
}

fn path(p: &[f32], r: f32, caps: &mut Vec<f32>) {
    if p.len() == 2 { caps.extend([p[0], p[1], p[0], p[1], r]); return; }
    let mut k = 2;
    while k + 1 < p.len() { caps.extend([p[k - 2], p[k - 1], p[k], p[k + 1], r]); k += 2; }
}
/// A polygon as a triangle fan, turned counter-clockwise.
fn fan(p: &[f32], tris: &mut Vec<f32>) {
    let n = p.len() / 2;
    if n < 3 { return; }
    let mut area = 0f32;
    for k in 0..n { let j = (k + 1) % n; area += p[2 * k] * p[2 * j + 1] - p[2 * j] * p[2 * k + 1]; }
    for k in 1..n - 1 {
        let (a, b) = if area >= 0. { (k, k + 1) } else { (k + 1, k) };
        tris.extend([p[0], p[1], p[2 * a], p[2 * a + 1], p[2 * b], p[2 * b + 1]]);
    }
}

/// What the viewer draws: chunks in paint order, the plot's box, or why there is nothing.
#[derive(Debug, Default)]
pub struct Frame { pub key: String, pub fam: String, pub bbox: [f32; 4], pub edge: [f32; 4], pub thick: f32, pub chunks: Vec<Arc<Chunk>>, pub none: String }

/// The chunks this client holds (the last plot applied), and the frame being drawn.
#[derive(Default)]
pub struct Store { held: Vec<Arc<Chunk>>, pub frame: Option<Frame> }

impl Store {
    pub fn reset(&mut self) { self.held.clear(); self.frame = None; }
    /// Apply one plot frame. Err names a frame that cannot be used; the store is then empty, as on the phones.
    pub fn receive(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        let o = decode(bytes).ok_or("undecodable plot frame")?;
        let key = o.get("key").and_then(C::text).unwrap_or("").to_string();
        let fam = o.get("fam").and_then(C::text).filter(|s| !s.is_empty()).unwrap_or(&key).to_string();
        // a 3D model rides the same socket; the mesh viewer is separate
        if key.starts_with("2|") || key.starts_with("3|") || key.starts_with("4|") { return Ok(()); }
        if let Some(why) = o.get("none").and_then(C::text) {
            self.held.clear();
            self.frame = Some(Frame { key, fam, none: why.to_string(), ..Default::default() });
            return Ok(());
        }
        let none: Vec<C> = Vec::new();
        let refs = match o.get("cs") { Some(C::Arr(a)) => a, _ => &none };
        let mut cs: Vec<Arc<Chunk>> = Vec::with_capacity(refs.len());
        for r in refs {
            match r {
                C::U(k) if *k >= 0 && (*k as usize) < self.held.len() => cs.push(self.held[*k as usize].clone()),
                C::Map(_) => cs.push(Arc::new(Chunk::decode(r))),
                _ => { self.held.clear(); self.frame = Some(Frame { key, fam, none: "Invalid geometry reference. Retry to reload this view.".into(), ..Default::default() }); return Err("invalid geometry reference"); }
            }
        }
        let four = |k: &str| -> [f32; 4] { match o.get(k) { Some(C::Arr(a)) if a.len() >= 4 => [0, 1, 2, 3].map(|i| a[i].int().unwrap_or(0) as f32), _ => [0.; 4] } };
        self.held = cs.clone();
        self.frame = Some(Frame { key, fam, bbox: four("box"), edge: four("edge"), thick: o.get("thick").and_then(C::int).unwrap_or(1600) as f32, chunks: cs, none: String::new() });
        Ok(())
    }
}

/// The view: where the plot's centre sits and how many pixels a micrometre is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View { pub cx: f32, pub cy: f32, pub scale: f32 }

impl View {
    /// Fit the box (micrometres) into a viewport of w x h pixels with a margin.
    pub fn fit(bbox: [f32; 4], w: f32, h: f32) -> View {
        let (bw, bh) = ((bbox[2] - bbox[0]).max(1.), (bbox[3] - bbox[1]).max(1.));
        let scale = ((w * 0.94) / bw).min((h * 0.94) / bh);
        View { cx: (bbox[0] + bbox[2]) / 2., cy: (bbox[1] + bbox[3]) / 2., scale: if scale.is_finite() && scale > 0. { scale } else { 1e-3 } }
    }
    /// Plot micrometres to viewport pixels (y down, as the board's y does).
    pub fn to_px(&self, x: f32, y: f32, w: f32, h: f32) -> (f32, f32) { (w / 2. + (x - self.cx) * self.scale, h / 2. + (y - self.cy) * self.scale) }
    pub fn to_plot(&self, px: f32, py: f32, w: f32, h: f32) -> (f32, f32) { (self.cx + (px - w / 2.) / self.scale, self.cy + (py - h / 2.) / self.scale) }
    /// Zoom by `factor` keeping the plot point under the pointer fixed; bounded to 1/60 .. 20 of the fitted scale.
    pub fn zoom(&self, factor: f32, px: f32, py: f32, w: f32, h: f32, fitted: f32) -> View {
        let scale = (self.scale * factor).clamp(fitted / 60., fitted * 20.);
        let (x, y) = self.to_plot(px, py, w, h);
        View { cx: x - (px - w / 2.) / scale, cy: y - (py - h / 2.) / scale, scale }
    }
    pub fn pan(&self, dx: f32, dy: f32) -> View { View { cx: self.cx - dx / self.scale, cy: self.cy - dy / self.scale, scale: self.scale } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn head(o: &mut Vec<u8>, major: u8, n: usize) { let m = major << 5; if n < 24 { o.push(m | n as u8) } else if n < 256 { o.extend([m | 24, n as u8]) } else if n < 65536 { o.push(m | 25); o.extend((n as u16).to_be_bytes()) } else { o.push(m | 26); o.extend((n as u32).to_be_bytes()) } }
    fn text(o: &mut Vec<u8>, s: &str) { head(o, 3, s.len()); o.extend(s.as_bytes()) }
    fn varint(o: &mut Vec<u8>, mut n: u32) { loop { let b = (n & 127) as u8; n >>= 7; if n == 0 { o.push(b); break } o.push(b | 128) } }
    fn signed(o: &mut Vec<u8>, v: i32) { varint(o, ((v << 1) ^ (v >> 31)) as u32) }
    /// A chunk map {l, c, a, m, w, d} with one piece of points (absolute, written as deltas).
    fn chunk(l: u32, c: u32, m: u32, w: u32, pts: &[(i32, i32)]) -> Vec<u8> {
        let mut d = Vec::new();
        varint(&mut d, 7);                       // info
        if m != 2 { varint(&mut d, pts.len() as u32); }
        let (mut px, mut py) = (0, 0);
        for (x, y) in pts { signed(&mut d, x - px); signed(&mut d, y - py); px = *x; py = *y; }
        let mut o = Vec::new(); head(&mut o, 5, 6);
        for (k, v) in [("l", l), ("c", c), ("a", 255), ("m", m), ("w", w)] { text(&mut o, k); head(&mut o, 0, v as usize); }
        text(&mut o, "d"); head(&mut o, 2, d.len()); o.extend(d); o
    }
    fn frame(key: &str, cs: &[Vec<u8>], refs: &[Option<usize>], bbox: [u32; 4]) -> Vec<u8> {
        let mut o = Vec::new(); head(&mut o, 5, 5);
        o.extend([1]); text(&mut o, "plot"); o.extend([0x18, 31]); text(&mut o, key);
        text(&mut o, "cs"); head(&mut o, 4, refs.len());
        let mut next = 0; for r in refs { match r { Some(k) => head(&mut o, 0, *k), None => { o.extend(&cs[next]); next += 1; } } }
        text(&mut o, "box"); head(&mut o, 4, 4); for v in bbox { head(&mut o, 0, v as usize); }
        text(&mut o, "thick"); head(&mut o, 0, 1600); o
    }
    #[test] fn a_plot_frame_is_recognised_and_its_chunks_decode() {
        let f = frame("0|/p|", &[chunk(3, 0xFF0000, 1, 200, &[(0, 0), (1000, 0), (1000, 500)]), chunk(5, 0x00FF00, 0, 0, &[(0, 0), (100, 0), (100, 100), (0, 100)])], &[None, None], [0, 0, 2000, 1000]);
        assert!(is_plot(&f));
        let mut s = Store::default(); s.receive(&f).unwrap();
        let fr = s.frame.as_ref().unwrap();
        assert_eq!(fr.key, "0|/p|"); assert_eq!(fr.bbox, [0., 0., 2000., 1000.]); assert_eq!(fr.chunks.len(), 2);
        assert_eq!(fr.chunks[0].caps, vec![0., 0., 1000., 0., 100., 1000., 0., 1000., 500., 100.]);   // two capsules, r = w/2
        assert_eq!((fr.chunks[0].layer, fr.chunks[0].color), (3, 0xFF0000));
        assert_eq!(fr.chunks[1].tris.len(), 12); assert_eq!(fr.chunks[1].polys.len(), 1);          // a square: two triangles
    }
    #[test] fn a_second_frame_may_keep_held_chunks_by_index() {
        let a = frame("0|/p|", &[chunk(1, 1, 1, 10, &[(0, 0), (5, 5)]), chunk(2, 2, 1, 10, &[(0, 0), (9, 9)])], &[None, None], [0, 0, 10, 10]);
        let b = frame("0|/p|", &[chunk(3, 3, 1, 10, &[(1, 1), (2, 2)])], &[Some(1), None, Some(0)], [0, 0, 10, 10]);
        let mut s = Store::default(); s.receive(&a).unwrap(); s.receive(&b).unwrap();
        let ls: Vec<u32> = s.frame.as_ref().unwrap().chunks.iter().map(|c| c.layer).collect();
        assert_eq!(ls, [2, 3, 1]);
    }
    #[test] fn a_bad_reference_empties_the_store_and_says_so() {
        let mut s = Store::default();
        let f = frame("0|/p|", &[], &[Some(4)], [0, 0, 1, 1]);
        assert!(s.receive(&f).is_err()); assert!(s.frame.as_ref().unwrap().none.contains("Invalid geometry"));
    }
    #[test] fn none_and_3d_frames() {
        let mut o = Vec::new(); head(&mut o, 5, 3); o.extend([1]); text(&mut o, "plot"); o.extend([0x18, 31]); text(&mut o, "1|/p|"); text(&mut o, "none"); text(&mut o, "No schematic in this project yet");
        let mut s = Store::default(); s.receive(&o).unwrap(); assert_eq!(s.frame.as_ref().unwrap().none, "No schematic in this project yet");
        let mut m = Vec::new(); head(&mut m, 5, 2); m.extend([1]); text(&mut m, "plot"); m.extend([0x18, 31]); text(&mut m, "2|/p|");
        let before = s.frame.as_ref().map(|f| f.key.clone()); s.receive(&m).unwrap(); assert_eq!(s.frame.as_ref().map(|f| f.key.clone()), before, "a model frame leaves the plot alone");
    }
    #[test] fn malformed_frames_are_refused_not_panicked() {
        for bad in [&[0xA1u8][..], &[0xA1, 1, 0x64, b'p'][..], &[0xFF; 40][..], &[][..]] { let mut s = Store::default(); assert!(s.receive(bad).is_err()); }
        let mut d = Vec::new(); head(&mut d, 5, 2); text(&mut d, "cs"); head(&mut d, 4, 1); head(&mut d, 5, 1); text(&mut d, "d"); head(&mut d, 2, 3); d.extend([0x80, 0x80, 0x80]);  // a varint that never ends
        text(&mut d, "key"); text(&mut d, "0|x|");
        let mut s = Store::default(); let _ = s.receive(&d);   // must not panic
    }
    #[test] fn fit_centres_the_box_and_zoom_keeps_the_point_under_the_pointer() {
        let v = View::fit([0., 0., 2000., 1000.], 940., 500.);
        assert!((v.cx - 1000.).abs() < 1e-3 && (v.cy - 500.).abs() < 1e-3); assert!((v.scale - 0.47).abs() < 1e-3, "{}", v.scale);
        let (x, y) = v.to_plot(100., 80., 940., 500.);
        let z = v.zoom(2., 100., 80., 940., 500., v.scale);
        let (x2, y2) = z.to_plot(100., 80., 940., 500.);
        assert!((x - x2).abs() < 1e-2 && (y - y2).abs() < 1e-2);
        assert!(v.zoom(1e9, 0., 0., 940., 500., v.scale).scale <= v.scale * 20.001); assert!(v.zoom(1e-9, 0., 0., 940., 500., v.scale).scale >= v.scale / 60.001);
        let p = v.pan(10., 0.); assert!(p.cx < v.cx);
    }
}

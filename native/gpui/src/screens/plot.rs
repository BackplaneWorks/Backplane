//! Board and schematic plots from the hub: a binary CBOR frame
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

/// One chunk, decoded once: strokes as capsules (ax ay bx by r) and fills as polygons (x y pairs), micrometres. `sig` is a hash of the
/// chunk as the hub sent it: the same chunk sent again is the one already held (its decoding and the canvas's triangles are kept).
#[derive(Debug, Default, PartialEq)]
pub struct Chunk { pub layer: u32, pub color: u32, pub alpha: f32, pub own: bool, pub caps: Vec<f32>, pub polys: Vec<Vec<f32>>, pub sig: u64,
    /// What a tap can pick: each piece's info index (its line in the hub's info file), its box and its points (`pts[at..at+n]`, x y pairs) as a dot, a stroke or a fill.
    pub pieces: Vec<Piece>, pub pts: Vec<f32>, pub r: f32 }
/// kind: 0 a fill (polygon), 1 a stroke (polyline of radius `Chunk::r`), 2 a dot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece { pub info: u32, pub bbox: [f32; 4], pub at: u32, pub n: u32, pub kind: u8 }

impl Chunk {
    /// The chunk's signature: its header and data bytes hashed.
    pub fn sig_of(o: &C) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for k in ["l", "c", "a", "m", "w"] { o.get(k).and_then(C::int).hash(&mut h); }
        if let Some(C::Bytes(b)) = o.get("d") { b.hash(&mut h); }
        h.finish()
    }
    pub fn decode(o: &C) -> Chunk {
        let g = |k: &str, d: i64| o.get(k).and_then(C::int).unwrap_or(d);
        let mode = g("m", 1);
        let mut c = Chunk { layer: g("l", 0) as u32, color: g("c", 0) as u32, alpha: g("a", 255) as f32 / 255., own: mode == 0 && g("w", 0) != 0, sig: Chunk::sig_of(o), ..Default::default() };
        let r = g("w", 0) as f32 / 2.; c.r = r;
        let empty = Vec::new();
        let data = match o.get("d") { Some(C::Bytes(b)) => b, _ => &empty };
        let mut v = Varints { b: data, i: 0, ok: true };
        let (mut px, mut py) = (0i32, 0i32);
        while v.more() {
            let info = v.next();
            let mut pts: Vec<f32> = Vec::new();
            let mut point = |v: &mut Varints| { px = px.wrapping_add(v.signed()); py = py.wrapping_add(v.signed()); (px as f32, py as f32) };
            if mode == 2 { let (x, y) = point(&mut v); pts = vec![x, y]; c.caps.extend([x, y, x, y, r]); }
            else {
                let n = v.next() as usize;
                for _ in 0..n.min(1 << 20) { if !v.more() { break; } let (x, y) = point(&mut v); pts.extend([x, y]); }
                if mode == 0 { if pts.len() >= 6 { c.polys.push(pts.clone()); } } else { path(&pts, r, &mut c.caps); }
            }
            if !pts.is_empty() {
                let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                for q in pts.chunks_exact(2) { x0 = x0.min(q[0]); x1 = x1.max(q[0]); y0 = y0.min(q[1]); y1 = y1.max(q[1]); }
                let kind = if mode == 2 || pts.len() == 2 { 2 } else if mode == 0 { 0 } else { 1 };
                c.pieces.push(Piece { info, bbox: [x0 - r, y0 - r, x1 + r, y1 + r], at: c.pts.len() as u32, n: (pts.len() / 2) as u32, kind });
                c.pts.extend_from_slice(&pts);
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
/// What the viewer draws: chunks in paint order, the plot's box, or why there is nothing.
#[derive(Clone, Debug, Default)]
pub struct Frame { pub key: String, pub fam: String, pub bbox: [f32; 4], pub edge: [f32; 4], pub thick: f32, pub chunks: Vec<Arc<Chunk>>, pub none: String }

/// A plot frame read off the UI thread: the frame and, for each chunk it sends in full, that chunk decoded (`Store::apply` does the rest).
#[derive(Debug)]
pub struct Prepared { o: C, new: Vec<Option<Arc<Chunk>>> }
/// Read a plot frame and decode its new chunks (pure: any thread). None when the bytes are not one.
pub fn prepare(bytes: &[u8]) -> Option<Prepared> {
    let o = decode(bytes)?;
    let new = match o.get("cs") { Some(C::Arr(a)) => a.iter().map(|r| matches!(r, C::Map(_)).then(|| Arc::new(Chunk::decode(r)))).collect(), _ => Vec::new() };
    Some(Prepared { o, new })
}

/// The chunks this client holds per source (a delta names its `base` source; a Split watches a schematic and a board at once), the newest frame
/// (`frame`, the one Board and Schematic draw) and each source's own (`frames`, what a Split's panes draw).
#[derive(Default)]
pub struct Store { pub sheets: Vec<Sheet>, held: std::collections::HashMap<String, Vec<Arc<Chunk>>>, held_key: String, pub frame: Option<Frame>, pub frames: std::collections::HashMap<String, Frame> }

const MAX_SOURCES: usize = 6;

impl Store {
    pub fn reset(&mut self) { self.held.clear(); self.held_key.clear(); self.frame = None; self.frames.clear(); }
    fn fail(&mut self, key: String, fam: String, why: &str) {
        self.held.remove(&key); if self.held_key == key { self.held_key.clear(); }
        let f = Frame { key: key.clone(), fam, none: why.to_string(), ..Default::default() };
        if !key.is_empty() { self.frames.insert(key, f.clone()); } else { self.held.clear(); self.frames.clear(); }
        self.frame = Some(f);
    }
    /// A frame the client will not take (over its size limit): said, not left on Loading.
    pub fn too_big(&mut self, bytes: usize) { self.fail(String::new(), String::new(), &format!("This plot is {} MB, too large to show here.", bytes >> 20)); }
    /// The newest frame of the source named `key`.
    pub fn frame_for(&self, key: &str) -> Option<&Frame> { self.frames.get(key) }
    /// Apply one plot frame. Err names a frame that cannot be used; the store is then empty, as on the phones.
    pub fn receive(&mut self, bytes: &[u8]) -> Result<(), &'static str> { self.apply(prepare(bytes)) }
    /// Apply a frame read by `prepare` (off the UI thread): held chunks are resolved here, in arrival order.
    pub fn apply(&mut self, p: Option<Prepared>) -> Result<(), &'static str> {
        let Some(Prepared { o, mut new }) = p else { self.fail(String::new(), String::new(), "The plot from the hub could not be read. Reopen the view to load it again."); return Err("undecodable plot frame") };
        let key = o.get("key").and_then(C::text).unwrap_or("").to_string();
        let fam = o.get("fam").and_then(C::text).filter(|s| !s.is_empty()).unwrap_or(&key).to_string();
        // a 3D model rides the same socket; the mesh viewer is separate
        if key.starts_with("2|") || key.starts_with("3|") || key.starts_with("4|") { return Ok(()); }
        if let Some(why) = o.get("none").and_then(C::text) {
            // {key:"", none:""}: the hub empties what an earlier connection left here; nothing is shown instead of what the page has
            if key.is_empty() && why.is_empty() { self.reset(); return Ok(()) }
            self.fail(key, fam, why);
            return Ok(());
        }
        let none: Vec<C> = Vec::new();
        let refs = match o.get("cs") { Some(C::Arr(a)) => a, _ => &none };
        // `base` names the source this delta is against: held chunks of another source are not what its indexes mean; none named is the last source applied
        let base = o.get("base").and_then(C::text).unwrap_or("");
        let from = if base.is_empty() { self.held_key.clone() } else { base.to_string() };
        let held: Vec<Arc<Chunk>> = self.held.get(&from).cloned().unwrap_or_default();
        let stale_base = !base.is_empty() && !self.held.contains_key(base);
        let mut cs: Vec<Arc<Chunk>> = Vec::with_capacity(refs.len());
        // a chunk sent again in full that this client holds (any source) is the held one
        let by_sig: std::collections::HashMap<u64, &Arc<Chunk>> = if new.iter().any(Option::is_some) { self.held.values().flatten().map(|c| (c.sig, c)).collect() } else { Default::default() };
        for (i, r) in refs.iter().enumerate() {
            match r {
                C::U(k) if !stale_base && *k >= 0 && (*k as usize) < held.len() => cs.push(held[*k as usize].clone()),
                C::Map(_) => { let c = new.get_mut(i).and_then(Option::take).unwrap_or_else(|| Arc::new(Chunk::decode(r))); cs.push(by_sig.get(&c.sig).map_or(c, |h| (*h).clone())); }
                _ => { self.fail(key, fam, "Invalid geometry reference. Retry to reload this view."); return Err("invalid geometry reference"); }
            }
        }
        let four = |k: &str| -> [f32; 4] { match o.get(k) { Some(C::Arr(a)) if a.len() >= 4 => [0, 1, 2, 3].map(|i| a[i].int().unwrap_or(0) as f32), _ => [0.; 4] } };
        if self.held.len() >= MAX_SOURCES && !self.held.contains_key(&key) { self.held.clear(); self.frames.clear(); }
        self.held.insert(key.clone(), cs.clone()); self.held_key = key.clone();
        let f = Frame { key: key.clone(), fam, bbox: four("box"), edge: four("edge"), thick: o.get("thick").and_then(C::int).unwrap_or(1600) as f32, chunks: cs, none: String::new() };
        self.frames.insert(key, f.clone()); self.frame = Some(f);
        Ok(())
    }
}


/// A piece a tap lies on: the chunk it is in (its index in the frame, which is the hub's `at`), its info index, its layer and its area (100 square micrometres).
#[derive(Clone, Debug, PartialEq)]
pub struct Pick { pub chunk: usize, pub info: u32, pub layer: u32, pub area: u64, pub kind: u8, pub pts: Vec<f32>, pub r: f32 }

fn seg_dist(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (dx, dy) = (bx - ax, by - ay); let l2 = dx * dx + dy * dy;
    let t = if l2 > 0. { (((px - ax) * dx + (py - ay) * dy) / l2).clamp(0., 1.) } else { 0. };
    ((px - ax - t * dx).powi(2) + (py - ay - t * dy).powi(2)).sqrt()
}
fn inside(pts: &[f32], x: f32, y: f32) -> bool {
    let n = pts.len() / 2; let mut c = false; let mut j = n.wrapping_sub(1);
    for i in 0..n { let (xi, yi, xj, yj) = (pts[2 * i], pts[2 * i + 1], pts[2 * j], pts[2 * j + 1]); if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi { c = !c } j = i; }
    c
}
/// Distance from (x, y) to a piece: 0 inside a fill, to the edge otherwise; to the stroke's centre line less its radius; to a dot less its radius.
fn distance(c: &Chunk, p: &Piece, x: f32, y: f32) -> f32 {
    let pts = &c.pts[p.at as usize..p.at as usize + p.n as usize * 2];
    match p.kind {
        2 => (((x - pts[0]).powi(2) + (y - pts[1]).powi(2)).sqrt() - c.r).max(0.),
        0 => { if inside(pts, x, y) { return 0. } let n = pts.len() / 2; (0..n).map(|i| { let j = (i + 1) % n; seg_dist(x, y, pts[2 * i], pts[2 * i + 1], pts[2 * j], pts[2 * j + 1]) }).fold(f32::MAX, f32::min) }
        _ => (pts.windows(4).step_by(2).map(|w| seg_dist(x, y, w[0], w[1], w[2], w[3])).fold(f32::MAX, f32::min) - c.r).max(0.),
    }
}
/// View.pick / Pick.best: of the pieces within `tol` micrometres of the tap on layers not hidden (`off` mask), the smallest wins (a pad over the zone it sits in), then the one drawn on top.
pub fn hit(f: &Frame, x: f32, y: f32, tol: f32, off: u32) -> Option<Pick> {
    let mut best: Option<Pick> = None;
    for (ci, c) in f.chunks.iter().enumerate() {
        if c.layer < 32 && (off >> c.layer) & 1 == 1 { continue }
        for p in &c.pieces {
            let b = p.bbox;
            if x < b[0] - tol || x > b[2] + tol || y < b[1] - tol || y > b[3] + tol { continue }
            if distance(c, p, x, y) > tol { continue }
            let area = (((b[2] - b[0]).max(1.) * (b[3] - b[1]).max(1.)) / 100.).min(4e9) as u64;
            let take = match &best { None => true, Some(o) => area < o.area || (area == o.area && c.layer >= o.layer) };
            if take { best = Some(Pick { chunk: ci, info: p.info, layer: c.layer, area, kind: p.kind, pts: c.pts[p.at as usize..p.at as usize + p.n as usize * 2].to_vec(), r: c.r }) }
        }
    }
    best
}

/// The versions history.refs lists (the Compare menu): where the repository is, its top folder, and a row (kind, label, commit) for HEAD, the branch point, branches, tags and commits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Refs { pub git: String, pub top: String, pub pcb: String, pub sch: String, pub rows: Vec<(String, String, String)> }
pub fn refs_of(r: &serde_json::Value) -> Option<Refs> {
    let g = |k: &str| r[k].as_str().unwrap_or("").to_string();
    if !r.is_object() { return None }
    let rows = g("refs").lines().filter_map(|l| { let f: Vec<&str> = l.split('\t').collect(); if f.len() < 3 || f[2].is_empty() || !matches!(f[0], "head" | "base" | "branch" | "tag" | "commit") { return None } Some((f[0].to_string(), f[1].to_string(), f[2].to_string())) }).take(60).collect();
    Some(Refs { git: g("git"), top: g("top"), pcb: g("pcb"), sch: g("sch"), rows })
}
/// core/hist's version path of `file` in `commit`: "git\t<gitdir>\t<commit>\t<len top>\t<top>/<rel>"; a comparison is "diff\n<old>\n<new live file>".
pub fn compare_path(r: &Refs, commit: &str, file: &str) -> Option<String> {
    let rel = file.strip_prefix(&format!("{}/", r.top))?;
    Some(format!("diff\ngit\t{}\t{}\t{}\t{}/{}\n{}", r.git, commit, r.top.chars().count(), r.top, rel, file))
}

/// A sheet of the schematic's hierarchy (Hier.show's line: depth, leaf, name, file; the root is the first row).
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet { pub depth: u32, pub leaf: bool, pub name: String, pub file: String }
pub fn sheet_rows(text: &str) -> Vec<Sheet> {
    text.lines().filter_map(|l| { let f: Vec<&str> = l.split('\t').collect(); if f.len() < 4 { return None } Some(Sheet { depth: f[0].parse().unwrap_or(0), leaf: f[1] == "1", name: f[2].to_string(), file: f[3].to_string() }) }).collect()
}

/// An inspected piece's card (Insp.card): its kind, then key = value rows. `g=` (a schematic item's connected group) is cross-probing's business, no row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Card { pub title: String, pub rows: Vec<(String, String)> }
pub fn card(info: &str) -> Option<Card> {
    if info.trim().is_empty() { return None }
    let mut parts = info.split(';');
    let title = parts.next().unwrap_or("").to_string();
    let rows = parts.filter(|p| !p.starts_with("g=")).map(|p| match p.split_once('=') { Some((k, v)) => (k.to_string(), v.to_string()), None => (p.to_string(), String::new()) }).collect();
    Some(Card { title, rows })
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
        assert_eq!(fr.chunks[1].polys.len(), 1); assert_eq!(fr.chunks[1].polys[0].len(), 8);          // a square
    }
    #[test] fn a_second_frame_may_keep_held_chunks_by_index() {
        let a = frame("0|/p|", &[chunk(1, 1, 1, 10, &[(0, 0), (5, 5)]), chunk(2, 2, 1, 10, &[(0, 0), (9, 9)])], &[None, None], [0, 0, 10, 10]);
        let b = frame("0|/p|", &[chunk(3, 3, 1, 10, &[(1, 1), (2, 2)])], &[Some(1), None, Some(0)], [0, 0, 10, 10]);
        let mut s = Store::default(); s.receive(&a).unwrap(); s.receive(&b).unwrap();
        let ls: Vec<u32> = s.frame.as_ref().unwrap().chunks.iter().map(|c| c.layer).collect();
        assert_eq!(ls, [2, 3, 1]);
    }
    #[test] fn a_chunk_sent_again_in_full_is_the_one_held_and_the_hubs_reset_empties_the_store() {
        let a = frame("0|/p|", &[chunk(1, 1, 1, 10, &[(0, 0), (5, 5)])], &[None], [0, 0, 10, 10]);
        let b = frame("1|/p|", &[chunk(9, 9, 1, 10, &[(0, 0), (7, 7)])], &[None], [0, 0, 10, 10]);
        let mut s = Store::default(); s.receive(&a).unwrap(); let first = s.frame.as_ref().unwrap().chunks[0].clone();
        s.receive(&b).unwrap(); s.receive(&a).unwrap();
        assert!(Arc::ptr_eq(&first, &s.frame.as_ref().unwrap().chunks[0]), "the board's chunk, sent whole after the schematic, is the same Arc");
        assert!(s.frame_for("1|/p|").is_some());
        let mut r = Vec::new(); head(&mut r, 5, 3); r.extend([1]); text(&mut r, "plot"); r.extend([0x18, 31]); text(&mut r, ""); text(&mut r, "none"); text(&mut r, "");
        s.receive(&r).unwrap(); assert!(s.frame.is_none() && s.frame_for("0|/p|").is_none(), "the reset leaves nothing to draw, not an empty plot");
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
    /// The child's `.plot` text for a real project (ecc83's board, microwave): geometry made by the real generator, not by this file's encoder.
    fn child_chunks(text: &str) -> (Vec<u32>, Vec<C>) {
        // the child's lines are not text lines: a chunk's bytes (chars 0..255) may hold newlines, so read by the declared lengths
        let cs: Vec<char> = text.chars().collect();
        let (mut at, mut cks, mut b) = (0usize, Vec::new(), Vec::new());
        while at < cs.len() {
            let tag = cs[at]; at += 1;
            let (mut nums, mut cur) = (Vec::new(), 0u32);
            loop {
                if at >= cs.len() { return (b, cks); }
                let c = cs[at]; at += 1;
                if c == ' ' { nums.push(cur); cur = 0; if tag == 'c' && nums.len() == 8 { break } }
                else if c == '\n' { if tag == 'b' { nums.push(cur) } break }
                else if c.is_ascii_digit() { cur = cur * 10 + (c as u32 - 48) }
            }
            if tag == 'b' { b = nums.clone(); }
            if tag == 'c' && nums.len() == 8 {
                let (dl, il) = (nums[6] as usize, nums[7] as usize);
                let d: Vec<u8> = cs[at..at + dl].iter().map(|c| *c as u32 as u8).collect();
                at += dl + il; if cs.get(at) == Some(&'\n') { at += 1 }
                let mut m = BTreeMap::new();
                for (k, i) in [("s", 0), ("l", 1), ("c", 2), ("a", 3), ("m", 4), ("w", 5)] { m.insert(k.to_string(), C::U(nums[i] as i64)); }
                m.insert("d".into(), C::Bytes(d)); cks.push(C::Map(m));
            }
        }
        (b, cks)
    }
    #[test] fn real_plots_from_the_generator_decode_inside_their_box() {
        for (name, text) in [("ecc83-pp", include_str!("../../../../test/files/fixtures/plot/ecc83-pp.kicad_pcb.plot")), ("microwave", include_str!("../../../../test/files/fixtures/plot/microwave.kicad_pcb.plot"))] {
            let (b, cs) = child_chunks(text);
            assert!(b.len() >= 4 && !cs.is_empty(), "{name}: parsed {} chunks", cs.len());
            let (x0, y0, x1, y1) = (b[0] as f32, b[1] as f32, b[2] as f32, b[3] as f32);
            let (mut caps, mut polys) = (0, 0);
            for c in &cs {
                let ch = Chunk::decode(c);
                caps += ch.caps.len() / 5; polys += ch.polys.len();
                let pad = 200_000.;   // a track's width and a little more
                for s in ch.caps.chunks_exact(5) { for (x, y) in [(s[0], s[1]), (s[2], s[3])] { assert!(x >= x0 - pad && x <= x1 + pad && y >= y0 - pad && y <= y1 + pad, "{name}: ({x},{y}) outside {b:?}"); } }
                for p in &ch.polys { for q in p.chunks_exact(2) { assert!(q[0] >= x0 - pad && q[0] <= x1 + pad && q[1] >= y0 - pad && q[1] <= y1 + pad, "{name}: polygon point outside"); } }
            }
            assert!(caps > 10 && polys > 0, "{name}: {caps} capsules, {polys} polygons");
        }
    }
    /// The first frame the Rust hub sends after kicad.watch for the ecc83 demo (written by the hub's own plot::frame, test/files/fixtures/plot).
    const BOARD: &[u8] = include_bytes!("../../../../test/files/fixtures/plot/ecc83-board.frame.cbor");
    const SCH: &[u8] = include_bytes!("../../../../test/files/fixtures/plot/ecc83-sch.frame.cbor");
    #[test] fn the_hubs_own_board_and_schematic_frames_decode() {
        for (name, bytes, key) in [("board", BOARD, "0|/fixture/ecc83|"), ("schematic", SCH, "1|/fixture/ecc83|")] {
            assert!(is_plot(bytes), "{name}: not recognised as a plot frame");
            let mut s = Store::default(); s.receive(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            let f = s.frame.as_ref().unwrap();
            assert_eq!(f.key, key, "{name}"); assert!(f.none.is_empty()); assert!(f.chunks.len() > 5, "{name}: {} chunks", f.chunks.len());
            assert!(f.bbox[2] > f.bbox[0] && f.bbox[3] > f.bbox[1], "{name}: box {:?}", f.bbox);
            let (mut caps, mut polys) = (0, 0);
            for c in &f.chunks { caps += c.caps.len() / 5; polys += c.polys.len(); assert!(c.alpha > 0. && c.alpha <= 1.); }
            assert!(caps > 10, "{name}: {caps} capsules"); if name == "board" { assert!(polys > 0, "no filled shapes on a board"); }
            // every capsule end sits inside the box (a margin for a track's width)
            let pad = 300_000.;
            for c in &f.chunks { for sgm in c.caps.chunks_exact(5) { for (x, y) in [(sgm[0], sgm[1]), (sgm[2], sgm[3])] { assert!(x >= f.bbox[0] - pad && x <= f.bbox[2] + pad && y >= f.bbox[1] - pad && y <= f.bbox[3] + pad, "{name}: ({x},{y}) outside {:?}", f.bbox); } } }
            let v = View::fit(f.bbox, 480., 600.); assert!(v.scale > 0. && v.scale.is_finite());
        }
    }
    #[test] fn a_second_hub_frame_with_every_chunk_kept_is_the_same_plot() {
        // a re-send of the same source: a delta keeping chunk i at i, which the real frame's chunks (all fresh) make possible to test
        let mut s = Store::default(); s.receive(BOARD).unwrap();
        let n = s.frame.as_ref().unwrap().chunks.len();
        let mut o = Vec::new(); head(&mut o, 5, 3); o.extend([1]); text(&mut o, "plot"); o.extend([0x18, 31]); text(&mut o, "0|/fixture/ecc83|"); text(&mut o, "cs"); head(&mut o, 4, n);
        for i in 0..n { head(&mut o, 0, i); }
        s.receive(&o).unwrap(); assert_eq!(s.frame.as_ref().unwrap().chunks.len(), n);
    }
    #[test] fn an_oversized_plot_is_said() { let mut s = Store::default(); s.too_big(40 << 20); assert!(s.frame.as_ref().unwrap().none.contains("40 MB")); }
    #[test] fn a_delta_against_another_source_is_refused() {
        let a = frame("0|/p|", &[chunk(1, 1, 1, 10, &[(0, 0), (5, 5)])], &[None], [0, 0, 10, 10]);
        let mut s = Store::default(); s.receive(&a).unwrap();
        // a frame naming a different base that keeps chunk 0 by index
        let mut o = Vec::new(); head(&mut o, 5, 4); o.extend([1]); text(&mut o, "plot"); o.extend([0x18, 31]); text(&mut o, "1|/q|"); text(&mut o, "base"); text(&mut o, "0|/other|"); text(&mut o, "cs"); head(&mut o, 4, 1); head(&mut o, 0, 0);
        assert!(s.receive(&o).is_err()); assert!(s.frame.as_ref().unwrap().none.contains("Invalid geometry"));
    }
    #[test] fn an_undecodable_frame_empties_the_store_and_says_why() {
        let a = frame("0|/p|", &[chunk(1, 1, 1, 10, &[(0, 0), (5, 5)])], &[None], [0, 0, 10, 10]);
        let mut s = Store::default(); s.receive(&a).unwrap();
        assert!(s.receive(&[0xA1, 0x01, 0x64]).is_err());
        let f = s.frame.as_ref().unwrap(); assert!(f.chunks.is_empty() && f.none.contains("could not be read"));
    }
    #[test] fn fit_centres_the_box_and_zoom_keeps_the_point_under_the_pointer() {
        let v = View::fit([0., 0., 2000., 1000.], 940., 500.);
        assert!((v.cx - 1000.).abs() < 1e-3 && (v.cy - 500.).abs() < 1e-3); assert!((v.scale - 0.4418).abs() < 1e-3, "{}", v.scale);
        let (x, y) = v.to_plot(100., 80., 940., 500.);
        let z = v.zoom(2., 100., 80., 940., 500., v.scale);
        let (x2, y2) = z.to_plot(100., 80., 940., 500.);
        assert!((x - x2).abs() < 1e-2 && (y - y2).abs() < 1e-2);
        assert!(v.zoom(1e9, 0., 0., 940., 500., v.scale).scale <= v.scale * 20.001); assert!(v.zoom(1e-9, 0., 0., 940., 500., v.scale).scale >= v.scale / 60.001);
        let p = v.pan(10., 0.); assert!(p.cx < v.cx);
    }

    #[test] fn a_tap_picks_the_smallest_piece_then_the_top_layer_and_skips_hidden_layers() {
        let mk = |layer: u32, kind: u8, pts: &[f32], info: u32| Chunk { layer, r: 50., pieces: vec![Piece { info, bbox: { let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN]; for q in pts.chunks(2) { b = [b[0].min(q[0]), b[1].min(q[1]), b[2].max(q[0]), b[3].max(q[1])]; } [b[0] - 50., b[1] - 50., b[2] + 50., b[3] + 50.] }, at: 0, n: (pts.len() / 2) as u32, kind }], pts: pts.to_vec(), ..Default::default() };
        let zone = mk(5, 0, &[0., 0., 10000., 0., 10000., 10000., 0., 10000.], 1);
        let pad = mk(12, 2, &[5000., 5000.], 2);
        let f = Frame { chunks: vec![Arc::new(zone), Arc::new(pad)], ..Default::default() };
        assert_eq!(hit(&f, 5000., 5000., 100., 0).map(|p| (p.chunk, p.info)), Some((1, 2)), "the pad, not the zone it sits in");
        assert_eq!(hit(&f, 5000., 5000., 100., 1 << 12).map(|p| p.info), Some(1), "a hidden layer is not there to tap");
        assert_eq!(hit(&f, 9000., 9000., 100., 0).map(|p| p.info), Some(1));
        assert!(hit(&f, 20000., 20000., 100., 0).is_none());
        let line = mk(3, 1, &[0., 0., 1000., 0.], 7); let g = Frame { chunks: vec![Arc::new(line)], ..Default::default() };
        assert_eq!(hit(&g, 500., 120., 100., 0).map(|p| p.info), Some(7), "within the stroke's width and the finger's reach"); assert!(hit(&g, 500., 500., 100., 0).is_none());
    }
    #[test] fn the_card_is_a_title_and_key_value_rows_without_the_group() {
        let c = card("Pad;ref=U1;pin=3;net=GND;g=7").unwrap();
        assert_eq!((c.title.as_str(), c.rows.len()), ("Pad", 3)); assert_eq!(c.rows[2], ("net".to_string(), "GND".to_string())); assert!(card("  ").is_none());
    }

    #[test] fn the_sheet_rows_are_hiers_lines() {
        let r = sheet_rows("0\t0\troot\t/p/top.kicad_sch\n1\t1\tamp\t/p/amp.kicad_sch\nbad");
        assert_eq!(r.len(), 2); assert_eq!((r[1].depth, r[1].leaf, r[1].name.as_str(), r[1].file.as_str()), (1, true, "amp", "/p/amp.kicad_sch"));
    }

    #[test] fn a_comparison_names_the_old_version_and_the_live_file() {
        let r = refs_of(&serde_json::json!({"git": "/p/.git", "top": "/p", "pcb": "/p/b.kicad_pcb", "sch": "/p/s.kicad_sch", "refs": "head\tHEAD\tabc\nbranch\tmain\tdef\nturn\tx\tz\nbad"})).unwrap();
        assert_eq!(r.rows.len(), 2, "head and branch; a turn checkpoint is not offered");
        assert_eq!(compare_path(&r, "abc", "/p/b.kicad_pcb").unwrap(), "diff\ngit\t/p/.git\tabc\t2\t/p/b.kicad_pcb\n/p/b.kicad_pcb");
        assert!(compare_path(&r, "abc", "/elsewhere/b.kicad_pcb").is_none());
    }
}

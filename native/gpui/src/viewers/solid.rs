//! 3D / Mech: the hub's `solid` frame (CBOR {key, n, mesh, box3, none, ...}; mesh = varints: per triangle the colour XOR the previous one's, then
//! three vertices as zigzag deltas of x y z in 10 micrometres, board axes, z up). Decoding and drawing run on a worker thread, on the GPU (solid_gpu.rs: the model is uploaded once)
//! or else on the cores (raster.rs); the page only receives finished images, so input never waits for a model. The latest request wins, but a picture of a drag is never given up
//! part-way: a mouse moves faster than a picture is drawn, and giving up on each one for the next leaves the screen still until the pointer stops. The camera is the
//! product's free orbit (0.14's solid.js `Orbit`): the model follows the pointer in any direction about its centre, the wheel zooms at the pointer (1.16 a notch)
//! between 1/60 and 20 times the fitted distance, a right/middle or shift drag moves it 1:1, a double click fits, and its keys are 0.14's. The image is as big as
//! the panel in device pixels (a draft while it moves). Nothing repaints while idle.
use gpui::{prelude::*, *};
use crate::i18n::{t, tf, trs};
use super::raster::{self, render};
use std::sync::{atomic::{AtomicU64, Ordering}, Arc, Condvar, Mutex};

pub const MAX_TRIANGLES: usize = (64 << 20) / 84;
pub const MAX_PIXELS: usize = 2880 * 1800;
/// A frame this big takes a moment to decode: the page says so meanwhile.
const BUILDING_BYTES: usize = 512 << 10;
/// the most pixels a draft covers: beyond it the draft is drawn smaller and stretched
const DRAFT_PIXELS: usize = 1600 * 1000;
/// A picture in motion should take about a display frame or two: a slower one (a machine drawing on its cores, a big model) makes the next
/// smaller (stretched to the panel, sharp again at rest), a quick one lets it grow back, between 1/8 and all of DRAFT_PIXELS.
pub fn draft_scale(scale: f32, ms: f64) -> f32 { (if ms > 30.0 { scale * 0.7 } else if ms < 12.0 { scale * 1.15 } else { scale }).clamp(0.125, 1.0) }
/// Samples a pixel a side: 1 for a draft, then as many as the panel's size leaves cheap (3 x 3 up to ~0.5 MP, 2 x 2 up to ~3 MP, none beyond).
pub fn samples(w: usize, h: usize, draft: bool) -> usize { if draft { 1 } else if w * h <= 520_000 { 3 } else if w * h <= 3_000_000 { 2 } else { 1 } }
pub(super) const FOV: f32 = 35.0 * std::f32::consts::PI / 180.0;

#[derive(Clone, Debug, PartialEq)]
/// `groups`: each triangle's fade group, 0 the board (always shown), k the k-th part frame (it fades in); empty for a model with no parts.
/// `refs`: a part frame's mesh only: the reference designator of each instance (the frame's `refs`; "" none), its triangles `tris.len() / refs.len()`
/// an instance in the instances' order. What a click on the model picks (`pick`).
pub struct Mesh { pub tris: Vec<[f32; 9]>, pub normals: Vec<[f32; 9]>, pub colors: Vec<u32>, pub center: [f32; 3], pub radius: f32, pub groups: Vec<u32>, pub refs: Vec<String> }

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
pub fn decode_mesh(n: usize, bytes: &[u8]) -> Result<Mesh, String> { decode_mesh_faces(n, bytes, &[]) }
/// A frame's `faces` (runs: how many triangles in a row are of one B-rep face, varints) as each triangle's face; None when they are not
/// exactly `n` triangles (then normals are creased by angle).
pub fn face_ids(runs: &[u8], n: usize) -> Option<Vec<u32>> {
    if runs.is_empty() { return None }
    let mut v = Varints::new(runs); let mut out = Vec::with_capacity(n); let mut f = 0u32;
    while v.more() { let k = v.next() as usize; if !v.valid || k == 0 || out.len() + k > n { return None } out.extend(std::iter::repeat(f).take(k)); f += 1; }
    (out.len() == n).then_some(out)
}
/// `decode_mesh` with the frame's `faces` (empty: none).
pub fn decode_mesh_faces(n: usize, bytes: &[u8], runs: &[u8]) -> Result<Mesh, String> {
    /* i18n-ok: the note is drawn through note_words */ if n == 0 || n > MAX_TRIANGLES || bytes.len() < n * 10 { return Err("Invalid or oversized model".into()) }
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
        /* i18n-ok: the note is drawn through note_words */ if !v.valid { return Err("Incomplete model".into()) }
        tris.push(t); colors.push(color);
    }
    /* i18n-ok: the note is drawn through note_words */ if v.more() || !v.valid { return Err("Unexpected model bytes".into()) }
    let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let radius = (0..3).map(|a| (hi[a] - lo[a]) * (hi[a] - lo[a])).sum::<f32>().sqrt() / 2.0;
    let normals = raster::smooth_normals_in(&tris, &keys, face_ids(runs, n).as_deref());
    Ok(Mesh { tris, normals, colors, center, radius: radius.max(1e-3), groups: Vec::new(), refs: Vec::new() })
}

/// A part frame's model placed at each of its instances, in the page's axes (mm, the wire's y turned up, as `decode_mesh`). `at` is 12
/// little-endian f32 an instance: the row-major 3 x 4 affine from the model's integer coordinates to the board model's. Errors when the
/// bytes are not exactly `n` triangles or `at` is not whole instances.
pub fn decode_part(n: usize, bytes: &[u8], at: &[u8]) -> Result<Mesh, String> { decode_part_faces(n, bytes, at, &[]) }
/// `decode_part` with the frame's `faces` (each instance's triangles in the same faces as the model's).
pub fn decode_part_faces(n: usize, bytes: &[u8], at: &[u8], runs: &[u8]) -> Result<Mesh, String> {
    /* i18n-ok: the note is drawn through note_words */ if n == 0 || n > MAX_TRIANGLES || bytes.len() < n * 10 { return Err("Invalid or oversized part".into()) }
    /* i18n-ok: the note is drawn through note_words */ if at.is_empty() || at.len() % 48 != 0 { return Err("Invalid part placement".into()) }
    let k = at.len() / 48;
    /* i18n-ok: the note is drawn through note_words */ if n.saturating_mul(k) > MAX_TRIANGLES { return Err("Too many part triangles".into()) }
    let mut v = Varints::new(bytes);
    let (mut color, mut p) = (0u32, [0i32; 3]);
    let (mut raw, mut cols) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for _ in 0..n {
        color ^= v.next();
        let mut t = [0i32; 9];
        for c in 0..3 { for a in 0..3 { p[a] = p[a].wrapping_add(v.signed()); t[3 * c + a] = p[a]; } }
        /* i18n-ok: the note is drawn through note_words */ if !v.valid { return Err("Incomplete part".into()) }
        raw.push(t); cols.push(color);
    }
    /* i18n-ok: the note is drawn through note_words */ if v.more() || !v.valid { return Err("Unexpected part bytes".into()) }
    let (mut tris, mut colors, mut keys) = (Vec::with_capacity(n * k), Vec::with_capacity(n * k), Vec::with_capacity(n * k * 3));
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for m in at.chunks_exact(48) {
        let a: [f32; 12] = std::array::from_fn(|i| f32::from_le_bytes([m[4 * i], m[4 * i + 1], m[4 * i + 2], m[4 * i + 3]]));
        /* i18n-ok: the note is drawn through note_words */ if a.iter().any(|x| !x.is_finite()) { return Err("Invalid part placement".into()) }
        for (t, c) in raw.iter().zip(&cols) {
            let mut o = [0f32; 9];
            for c3 in 0..3 {
                let (x, y, z) = (t[3 * c3] as f32, t[3 * c3 + 1] as f32, t[3 * c3 + 2] as f32);
                let w = [a[0] * x + a[1] * y + a[2] * z + a[3], a[4] * x + a[5] * y + a[6] * z + a[7], a[8] * x + a[9] * y + a[10] * z + a[11]];
                keys.push([w[0].round() as i32, w[1].round() as i32, w[2].round() as i32]);
                // 10 micrometres -> millimetres, y turned up, as decode_mesh
                o[3 * c3] = w[0] * 0.01; o[3 * c3 + 1] = -w[1] * 0.01; o[3 * c3 + 2] = w[2] * 0.01;
                for q in 0..3 { lo[q] = lo[q].min(o[3 * c3 + q]); hi[q] = hi[q].max(o[3 * c3 + q]); }
            }
            tris.push(o); colors.push(*c);
        }
    }
    let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let radius = (0..3).map(|a| (hi[a] - lo[a]) * (hi[a] - lo[a])).sum::<f32>().sqrt() / 2.0;
    // each instance's faces are the model's, numbered apart (a face of one instance never blends with another's)
    let faces = face_ids(runs, n).map(|f| { let top = f.last().map_or(0, |x| x + 1); (0..k as u32).flat_map(|i| f.iter().map(move |x| x + i * top)).collect::<Vec<u32>>() });
    let normals = raster::smooth_normals_in(&tris, &keys, faces.as_deref());
    Ok(Mesh { tris, normals, colors, center, radius: radius.max(1e-3), groups: Vec::new(), refs: Vec::new() })
}

/// The board and the parts that have come, as one model: the board's triangles (group 0) then each part frame's (group 1, 2...). Its
/// centre and size are the board's, so arriving parts never move the camera.
/// One layer's triangles onto a combined mesh, as fade group `g` (what `combine` does a layer).
pub fn append(m: &mut Mesh, l: &Mesh, g: u32) { m.tris.extend_from_slice(&l.tris); m.normals.extend_from_slice(&l.normals); m.colors.extend_from_slice(&l.colors); m.groups.resize(m.tris.len(), g); }
pub fn combine(board: Option<&Mesh>, parts: &[Arc<Mesh>]) -> Option<Mesh> {
    let first = board.or(parts.first().map(|p| &**p))?;
    let n = board.map_or(0, |b| b.tris.len()) + parts.iter().map(|p| p.tris.len()).sum::<usize>();
    let mut m = Mesh { tris: Vec::with_capacity(n), normals: Vec::with_capacity(n), colors: Vec::with_capacity(n), center: first.center, radius: first.radius, groups: Vec::with_capacity(n), refs: Vec::new() };
    if let Some(b) = board { m.tris.extend_from_slice(&b.tris); m.normals.extend_from_slice(&b.normals); m.colors.extend_from_slice(&b.colors); m.groups.resize(b.tris.len(), 0); }
    for (g, p) in parts.iter().enumerate() { m.tris.extend_from_slice(&p.tris); m.normals.extend_from_slice(&p.normals); m.colors.extend_from_slice(&p.colors); m.groups.resize(m.tris.len(), g as u32 + 1); }
    Some(m)
}
/// The model on show in layers: the board (fade group 0) and each part frame come so far (group k + 1), each its own allocation, so a
/// part that arrives adds itself and moves nothing already made (the GPU keeps a buffer a layer; `combine` makes one mesh of them only for
/// the CPU's drawing). Its centre and size are the board's (else the first part's), so arriving parts never move the camera.
pub struct Layers { pub board: Option<Arc<Mesh>>, pub parts: Vec<Arc<Mesh>>, pub center: [f32; 3], pub radius: f32 }
impl Layers {
    pub fn of(board: Option<Arc<Mesh>>, parts: Vec<Arc<Mesh>>) -> Option<Layers> {
        let first = board.as_ref().or(parts.first())?;
        let (center, radius) = if board.is_some() { (first.center, first.radius) } else { bound(&parts) };
        Some(Layers { board, parts, center, radius })
    }
    pub fn tris(&self) -> usize { self.board.as_ref().map_or(0, |b| b.tris.len()) + self.parts.iter().map(|p| p.tris.len()).sum::<usize>() }
    /// Each layer with its fade group.
    pub fn list(&self) -> Vec<(&Arc<Mesh>, Option<u32>)> { self.board.iter().map(|b| (b, Some(0))).chain(self.parts.iter().enumerate().map(|(g, p)| (p, Some(g as u32 + 1)))).collect() }
    /// One mesh of them all (`combine`).
    pub fn combined(&self) -> Option<Mesh> { combine(self.board.as_deref(), &self.parts) }
}
/// The sphere around every part (a model streamed in parts with no board under them: a STEP's): the box of the parts' spheres, then the
/// smallest sphere about its centre that holds each of them.
pub fn bound(parts: &[Arc<Mesh>]) -> ([f32; 3], f32) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in parts { for a in 0..3 { lo[a] = lo[a].min(p.center[a] - p.radius); hi[a] = hi[a].max(p.center[a] + p.radius); } }
    if parts.is_empty() { return ([0.0; 3], 1e-3) }
    let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let r = parts.iter().map(|p| dot(sub(p.center, c), sub(p.center, c)).sqrt() + p.radius).fold(1e-3f32, f32::max);
    (c, r)
}
/// Whether the camera, fitted to `was` and not moved since, fits again to `now`: the model it shows moved or changed size by more than a
/// fiftieth of itself (a streamed model growing as its parts come, then its whole model).
pub fn refit(was: ([f32; 3], f32), now: ([f32; 3], f32)) -> bool {
    let d = dot(sub(was.0, now.0), sub(was.0, now.0)).sqrt();
    d > now.1 * 0.02 || (now.1 - was.1).abs() > now.1 * 0.02
}
/// How long a part takes to fade in.
pub const FADE_MS: f32 = 300.0;
/// The 3D tools' height and top: a 22 px button centred in the 32 px header row beside the pane.
const TOOL_H: f32 = 22.0;
const TOOL_TOP: f32 = 5.0;
/// What the page says while the first picture of a model it holds is drawn.
/* i18n-ok: drawn through trs */ const DRAWING: &str = "Drawing the view…";
/// The page's own stage while it decodes a large model (the hub has sent it by then).
/* i18n-ok: compared with the stage; drawn through trs (stage_line) */ const BUILDING: &str = "Building the view…";
/// What the note under a model says when the hub could not make a newer revision and the last one stays (kept in English: note_words draws it).
/* i18n-ok: drawn through note_words */ const PREVIOUS_SHOWN: &str = " — previous saved revision shown";
/// The page's note in the reader's language: the hub's words (or the page's own) through the catalog, with the "previous revision shown" tail said apart.
pub fn note_words(note: &str) -> String {
    match note.strip_suffix(PREVIOUS_SHOWN) { Some(why) => tf("gpui.c.solid.previous", &[("why", trs(why).into())]), None => trs(note) }
}
/// A part group's opacity `ms` after it showed.
pub fn fade_at(ms: f32) -> f32 { (ms / FADE_MS).clamp(0.0, 1.0) }
/// The progress the page shows while parts stream ("" when there are none to wait for).
/* i18n-ok: drawn through trs */ pub fn progress(done: u32, of: u32) -> String { if of == 0 || done >= of { String::new() } else { format!("parts {done}/{of}") } }

// ---- the frame: a minimal CBOR map reader (text keys, or the wire dictionary's integer keys; byte strings) ------
#[derive(Debug, Clone, PartialEq)]
pub enum Item { Int(i64), Bool(bool), Text(String), Bytes(Vec<u8>), Array(Vec<Item>), Other }
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
        5 => { for _ in 0..n.checked_mul(2)? { skip(b, at, depth + 1)?; } Item::Other } // a map's entries are stepped over, so what follows it reads right
        6 => return item(b, at, depth + 1), // a tag has one value: the tag number was the head's argument (as wire.rs reads it)
        7 if n == 20 => Item::Bool(false),
        7 if n == 21 => Item::Bool(true),
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
pub enum SolidFrame { Model(String, Mesh), /// why there is no model (the hub's `none`) and its `retryable` flag, the previous model stays
    Note(String, String, Option<bool>), Bad(String),
    /// the parts of the model start again: `of` instances will come
    Parts(String, u32),
    /// one part model at its instances (placed), and how many instances have now come
    Part(String, Mesh, u32),
    /// what the hub is doing to make the model (`text`, "" when done), for how long already and about how long it took the last time (ms, 0 unknown)
    Stage(String, String, u64, u64) }
pub(crate) fn is_solid_key(k: &str) -> bool { k.starts_with("2|") || k.starts_with("3|") || k.starts_with("4|") }
/// `parts:{of, rev}`: the map is not an `Item` (read_frame skips maps), so it is read here: the frame's `parts` value's `of`.
fn read_parts(b: &[u8]) -> Option<u32> {
    let mut at = 0; let (major, n) = head(b, &mut at)?; if major != 5 || n > 256 { return None }
    for _ in 0..n {
        let name = match item(b, &mut at, 1)? { Item::Text(t) => t, Item::Int(i) => crate::wire::KEYS.get(i as usize)?.to_string(), _ => return None };
        if name != "parts" { skip(b, &mut at, 1)?; continue }
        let (major, m) = head(b, &mut at)?; if major != 5 || m > 16 { return None }
        let mut of = 0u32;
        for _ in 0..m {
            let k = match item(b, &mut at, 2)? { Item::Text(t) => t, _ => return None };
            match item(b, &mut at, 2)? { Item::Int(v) if k == "of" && v >= 0 => of = v as u32, _ => {} }
        }
        return Some(of)
    }
    None
}
/// A solid frame as the page uses it; None when the bytes are not a solid frame at all.
pub fn parse_frame(b: &[u8]) -> Option<SolidFrame> {
    let f = read_frame(b)?;
    let get = |k: &str| f.iter().find(|(n, _)| n == k).map(|(_, v)| v);
    let key = match get("key") { Some(Item::Text(k)) => k.clone(), _ => return None };
    if !is_solid_key(&key) { return None }
    if let Some(Item::Text(why)) = get("none") {
        let retryable = match get("retryable") { Some(Item::Bool(b)) => Some(*b), _ => None };
        return Some(SolidFrame::Note(key, why.clone(), retryable))
    }
    if let Some(Item::Text(text)) = get("stage") {
        let ms = |k: &str| match get(k) { Some(Item::Int(v)) if *v > 0 => *v as u64, _ => 0 };
        return Some(SolidFrame::Stage(key, text.clone(), ms("ms"), ms("about")))
    }
    if let Some(of) = read_parts(b) { return Some(SolidFrame::Parts(key, of)) }
    if let Some(Item::Text(_id)) = get("part") {
        /* i18n-ok: the note is drawn through note_words */ let (Some(Item::Bytes(mesh)), Some(Item::Bytes(at))) = (get("mesh"), get("at")) else { return Some(SolidFrame::Bad("part without its model".into())) };
        /* i18n-ok: the note is drawn through note_words */ let n = match get("n") { Some(Item::Int(n)) if *n > 0 => *n as usize, _ => return Some(SolidFrame::Bad("missing triangle count".into())) };
        let done = match get("done") { Some(Item::Int(d)) if *d >= 0 => *d as u32, _ => 0 };
        let runs = match get("faces") { Some(Item::Bytes(b)) => b.as_slice(), _ => &[] };
        // each instance's reference (one an instance, or none of them: a list of another length names nothing)
        let refs: Vec<String> = match get("refs") { Some(Item::Array(a)) if a.len() * 48 == at.len() => a.iter().map(|r| match r { Item::Text(t) => t.clone(), _ => String::new() }).collect(), _ => Vec::new() };
        return Some(match decode_part_faces(n, mesh, at, runs) { Ok(mut m) => { m.refs = refs; SolidFrame::Part(key, m, done) } Err(e) => SolidFrame::Bad(e) })
    }
    let Some(Item::Bytes(mesh)) = get("mesh") else { return None }; // a note about the parts is not a model
    /* i18n-ok: the note is drawn through note_words */ let n = match get("n") { Some(Item::Int(n)) if *n > 0 => *n as usize, _ => return Some(SolidFrame::Bad("missing triangle count".into())) };
    let runs = match get("faces") { Some(Item::Bytes(b)) => b.as_slice(), _ => &[] };
    Some(match decode_mesh_faces(n, mesh, runs) { Ok(m) => SolidFrame::Model(key, m), Err(e) => SolidFrame::Bad(e) })
}

// ---- the camera: free orbit ------------------------------------------------------------------------------------
/// 0.14's `Orbit` (src/web/solid.js): the view turns about the model's centre (`pivot`), and a pan or a zoom at the pointer moves it within
/// the view's plane (`shift`, right and up from the pivot at its depth), so a turn keeps the model where the person put it on the screen.
/// `target` is pivot + right·shift[0] + up·shift[1], kept up to date by every move (`place`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera { pub right: [f32; 3], pub up: [f32; 3], pub fwd: [f32; 3], pub target: [f32; 3], pub dist: f32, pub fit: f32, pub pivot: [f32; 3], pub shift: [f32; 2] }
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
/// A wheel event as a zoom factor (over 1: closer). A mouse wheel comes in lines (gpui: 3 a notch, fractions from a high-resolution wheel) and
/// turns 1.16 a notch, as 0.14's 100 px notch did (exp(100 · 0.0015)); a touchpad's pixels zoom smoothly, and with ctrl (a pinch, as browsers
/// send one) faster, as 0.14's pinch did.
pub fn wheel_zoom(lines: Option<f32>, pixels: f32, ctrl: bool) -> f32 {
    let k = match lines { Some(l) => l * 0.05, None => pixels * if ctrl { 0.01 } else { 0.004 } };
    k.clamp(-2.0, 2.0).exp()
}
/// A trackpad pinch (gpui's `PinchEvent`, AppKit's magnification) as a zoom factor (over 1: closer): AppKit's convention is a scale of
/// 1 + magnification per event; one event's step is held to 0.5 .. 2 so a stray huge value cannot jump the view, and anything not finite is no zoom.
pub fn pinch_zoom(magnification: f32) -> f32 {
    if !magnification.is_finite() { return 1.0 }
    (1.0 + magnification).clamp(0.5, 2.0)
}
impl Camera {
    /// Looking at the model from the front-right and above, fitted to its bounding sphere.
    pub fn fitted(mesh: &Mesh) -> Camera { Camera::fitted_for(mesh, 1.0) }
    /// The same, for a panel `aspect` (width / height) wide: the field of view is the vertical one, so a panel narrower than it is tall needs the model further away to fit across.
    pub fn fitted_for(mesh: &Mesh, aspect: f32) -> Camera { Camera::fit_to(mesh.center, mesh.radius, aspect) }
    /// The fitted view of a model of this centre and bounding radius.
    pub fn fit_to(center: [f32; 3], radius: f32, aspect: f32) -> Camera {
        let aspect = if aspect.is_finite() && aspect > 0.05 { aspect } else { 1.0 };
        let fit = radius / (FOV / 2.0).sin() * 1.05 / aspect.min(1.0);
        let (yaw, pitch) = (0.6f32, 0.7f32);
        let eye_dir = [pitch.cos() * yaw.sin(), -pitch.cos() * yaw.cos(), pitch.sin()];
        let fwd = mul(eye_dir, -1.0); let right = norm(cross(fwd, [0.0, 0.0, 1.0])); let up = cross(right, fwd);
        Camera { right, up, fwd, target: center, dist: fit, fit, pivot: center, shift: [0.0, 0.0] }
    }
    /// A standard view of the same model at the same distance (the web's toolbar: Top, Bottom, Front, Right, Back, Left; anything else, Iso, is the fitted view).
    /// Z is up and the part's front faces -Y, so Front looks along +Y from the -Y side. The model is centred again.
    pub fn named(&self, name: &str) -> Camera {
        let (fwd, up) = match name {
            "front" => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]), "back" => ([0.0, -1.0, 0.0], [0.0, 0.0, 1.0]),
            "right" => ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), "left" => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            "top" => ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0]), "bottom" => ([0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
            _ => return Camera { target: self.pivot, dist: self.fit, shift: [0.0, 0.0], ..*self },
        };
        let mut c = Camera { right: [1.0, 0.0, 0.0], up, fwd, target: self.pivot, dist: self.fit, fit: self.fit, pivot: self.pivot, shift: [0.0, 0.0] };
        c.reortho(); c.place(); c
    }
    pub fn eye(&self) -> [f32; 3] { sub(self.target, mul(self.fwd, self.dist)) }
    fn reortho(&mut self) { self.fwd = norm(self.fwd); self.right = norm(cross(self.fwd, self.up)); self.up = cross(self.right, self.fwd); }
    /// The target from the pivot and the shift in the view's plane.
    fn place(&mut self) { self.target = add(self.pivot, add(mul(self.right, self.shift[0]), mul(self.up, self.shift[1]))); }
    /// A drag of (dx, dy) pixels (y down): the model follows the pointer, about its centre, in any direction (no pole, no clamp). The web's
    /// `Orbit.spin` exactly: the view turns by -|d|·k about the screen axis u·dx + r·dy, so a drag to the right brings the model's front to the
    /// right (the camera goes round to the left) and a drag down brings its top toward the viewer. Where the model is on the screen stays.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        let k = 0.008; // radians per pixel
        let l = (dx * dx + dy * dy).sqrt();
        if l < 1e-6 { return }
        let axis = norm(add(mul(self.up, dx / l), mul(self.right, dy / l)));
        for v in [&mut self.right, &mut self.fwd, &mut self.up] { *v = rotate(*v, axis, -l * k); }
        self.reortho(); self.place();
    }
    /// alt drag: roll about the view axis; a drag to the right turns the model clockwise on screen (the web's `Orbit.roll`).
    pub fn roll(&mut self, dx: f32) { let (f, a) = (self.fwd, -dx * 0.008); self.right = rotate(self.right, f, a); self.up = rotate(self.up, f, a); self.reortho(); self.place(); }
    /// World units a pixel at the pivot's depth, in a panel `h` pixels high.
    pub fn unit(&self, h: f32) -> f32 { 2.0 * (FOV / 2.0).tan() * self.dist / h.max(1.0) }
    /// Move the view: a right/middle or shift drag of (dx, dy) pixels in a panel `h` pixels high; the point under the pointer (at the pivot's depth) follows it.
    pub fn pan(&mut self, dx: f32, dy: f32, h: f32) {
        let s = self.unit(h);
        self.shift[0] -= dx * s; self.shift[1] += dy * s; self.place();
    }
    /// Zoom by `steps` wheel notches of 1.1 at the pointer (nx, ny in -1..1, y up; aspect = width / height).
    pub fn zoom_at(&mut self, steps: f32, nx: f32, ny: f32, aspect: f32) { self.zoom_by(1.1f32.powf(steps), nx, ny, aspect) }
    /// `k` times closer (0.14's `Orbit.zoom`), between 1/60 and 20 times the fitted distance, at the pointer (nx, ny in -1..1, y up; aspect = width / height):
    /// the point under it, at the pivot's depth, stays under it.
    pub fn zoom_by(&mut self, k: f32, nx: f32, ny: f32, aspect: f32) {
        if !k.is_finite() || k <= 0.0 { return }
        let t = (FOV / 2.0).tan();
        let d = (self.dist / k).clamp(self.fit / 60.0, self.fit * 20.0);
        let q = d / self.dist;
        let (x, y) = (nx * aspect * t * self.dist, ny * t * self.dist); // the pointer's offset from the view's centre, world units at the pivot's depth
        self.shift[0] += x * (1.0 - q); self.shift[1] += y * (1.0 - q); self.dist = d; self.place();
    }
}

// ---- picking: the part under a click ---------------------------------------------------------------------------
impl Camera {
    /// The ray from the eye through the pointer (nx, ny in -1..1, y up; aspect = width / height), as the pictures are projected: (eye, direction).
    pub fn ray(&self, nx: f32, ny: f32, aspect: f32) -> ([f32; 3], [f32; 3]) {
        let t = (FOV / 2.0).tan();
        (self.eye(), norm(add(self.fwd, add(mul(self.right, nx * aspect * t), mul(self.up, ny * t)))))
    }
}
/// Where a ray meets a triangle (either face), as its distance along the ray; None when it misses or the triangle is behind the eye.
fn ray_tri(eye: [f32; 3], dir: [f32; 3], t: &[f32; 9]) -> Option<f32> {
    let (a, b, c) = ([t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]);
    let (e1, e2) = (sub(b, a), sub(c, a));
    let p = cross(dir, e2); let det = dot(e1, p);
    if det.abs() < 1e-12 { return None }
    let inv = 1.0 / det; let s = sub(eye, a);
    let u = dot(s, p) * inv; if !(0.0..=1.0).contains(&u) { return None }
    let q = cross(s, e1);
    let v = dot(dir, q) * inv; if v < 0.0 || u + v > 1.0 { return None }
    let d = dot(e2, q) * inv;
    (d > 1e-6).then_some(d)
}
/// Whether the model names its parts (a board's, whose part frames carry `refs`): only such a model is picked on.
pub fn has_refs(model: &Layers) -> bool { model.parts.iter().any(|p| p.refs.iter().any(|r| !r.is_empty())) }
/// What a click meets first (0.14's solid.js `pick`, by the instance itself): None nothing at all; Some("") the board or a part with no name;
/// Some(reference) the part whose model the ray enters first.
pub fn pick(model: &Layers, eye: [f32; 3], dir: [f32; 3]) -> Option<String> {
    let mut best: Option<(f32, String)> = None;
    for (m, g) in model.list() {
        // (a layer the ray passes clear of is not walked)
        let oc = sub(m.center, eye); let along = dot(oc, dir);
        if dot(oc, oc) - along * along > m.radius * m.radius { continue }
        let per = if g != Some(0) && !m.refs.is_empty() { m.tris.len() / m.refs.len() } else { 0 };
        for (i, t) in m.tris.iter().enumerate() {
            let Some(d) = ray_tri(eye, dir, t) else { continue };
            if best.as_ref().is_some_and(|b| b.0 <= d) { continue }
            best = Some((d, if per > 0 { m.refs.get(i / per).cloned().unwrap_or_default() } else { String::new() }));
        }
    }
    best.map(|b| b.1)
}
/// A reference without a schematic unit's letter ("U1A" is unit A of U1, as the board names it; the hub's plotgen `ref_base`).
fn ref_base(r: &str) -> &str { let t = r.trim_end_matches(|c: char| c.is_ascii_uppercase()); if t.len() < r.len() && t.ends_with(|c: char| c.is_ascii_digit()) { t } else { r } }
/// Whether a probe's reference names this instance's part (a schematic's unit names its whole part).
pub fn same_part(probe: &str, r: &str) -> bool { !probe.is_empty() && !r.is_empty() && (probe == r || ref_base(probe) == r) }
/// The colour a probed part's triangles take: most of the way to the pick colour, its translucency kept.
pub fn lit_color(c: u32) -> u32 {
    let mix = |sh: u32| { let (a, b) = ((c >> sh) & 255, (0xffe066u32 >> sh) & 255); (a * 35 + b * 65) / 100 };
    (c & 0xff00_0000) | mix(16) << 16 | mix(8) << 8 | mix(0)
}
/// The model with the probed part lit: the layers that hold an instance of it made again with that instance's triangles in `lit_color`, the others
/// shared as they are (the GPU keeps their buffers). None when the model has no such part.
pub fn lit_layers(model: &Layers, probe: &str) -> Option<Layers> {
    if !model.parts.iter().any(|p| p.refs.iter().any(|r| same_part(probe, r))) { return None }
    let parts = model.parts.iter().map(|p| {
        if !p.refs.iter().any(|r| same_part(probe, r)) { return p.clone() }
        let mut m = (**p).clone(); let per = m.tris.len() / m.refs.len();
        for (k, r) in p.refs.iter().enumerate() { if same_part(probe, r) { for c in &mut m.colors[k * per..(k + 1) * per] { *c = lit_color(*c); } } }
        Arc::new(m)
    }).collect();
    Some(Layers { board: model.board.clone(), parts, center: model.center, radius: model.radius })
}


pub fn to_image(bgra: Vec<u8>, w: usize, h: usize) -> Option<Arc<RenderImage>> {
    let buf = image::RgbaImage::from_raw(w as u32, h as u32, bgra)?; // gpui takes BGRA in this container
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buf)])))
}

// ---- the worker: latest request wins -------------------------------------------------------------------------------
/// `Scene`: the model with the parts that have come (`mesh`), after one or more frames: `board` when the board itself is new (its
/// pictures in flight are stale), `reset` when the parts started again, `groups` part frames held, `done` of `of` instances come.
/// `Building`: a large model frame of this key is being decoded now.
enum Msg { Mesh(SolidFrame), Building(String), Scene { key: String, mesh: Option<Arc<Layers>>, board: bool, reset: bool, groups: usize, done: u32, of: u32 }, Image { ticket: u64, key: String, view_rev: u64, image: Arc<RenderImage>, ms: f64, tris: usize, w: usize, h: usize, by: &'static str, draft: bool } }
/// What to render: the source key, the model, the camera, the view revision, the size, the samples a pixel a side, the ticket, whether it is a draft (the model is moving).
type Job = (String, Arc<Layers>, Arc<Vec<f32>>, Camera, u64, usize, usize, usize, u64, bool, raster::Backdrop);
/// What the worker holds of the model on show: the board, the part frames come so far, the progress.
#[derive(Default)] struct Scene { key: String, board: Option<Arc<Mesh>>, parts: Vec<Arc<Mesh>>, done: u32, of: u32 }
impl Scene {
    /// One frame onto the scene: whether the model changed (and whether the board did, and the parts started again), or a message for the page.
    fn take(&mut self, f: SolidFrame, board: &mut bool, reset: &mut bool) -> Result<bool, SolidFrame> {
        match f {
            SolidFrame::Model(key, m) => { self.key = key; self.board = Some(Arc::new(m)); self.parts.clear(); self.done = 0; self.of = 0; *board = true; *reset = true; Ok(true) }
            SolidFrame::Parts(key, of) => {
                if key != self.key && self.board.is_some() { return Ok(false) } // another model's parts: not under this board
                self.key = key; self.parts.clear(); self.done = 0; self.of = of; *reset = true; Ok(true)
            }
            SolidFrame::Part(key, m, done) => { if key != self.key { return Ok(false) } self.parts.push(Arc::new(m)); self.done = done.min(self.of.max(done)); Ok(true) }
            SolidFrame::Note(key, why, retryable) => { if key != self.key { *self = Scene { key: key.clone(), ..Default::default() } } Err(SolidFrame::Note(key, why, retryable)) }
            other => Err(other),
        }
    }
}
/// Whether a render in progress goes on: a draft does, or a drag would show nothing at all; a picture at rest gives way to anything newer.
pub fn wanted(draft: bool, my: u64, newest: u64) -> bool { draft || my == newest }
/// Whether a finished picture goes on the screen: it is of this model (`floor` and matching source key) and newer than the one there. Pictures of a drag finish behind the pointer: that is no reason to drop them.
pub fn shows(ticket: u64, shown: u64, floor: u64) -> bool { ticket >= floor && ticket > shown }
/// Whether a finished picture for `image_key` may be painted on a page showing `page_key`.
pub fn shows_for_key(page_key: &str, image_key: &str, ticket: u64, shown: u64, floor: u64) -> bool {
    !page_key.is_empty() && page_key == image_key && shows(ticket, shown, floor)
}
/// Whether the cached image `image_key` belongs to `page_key` and may be kept/drawn under it.
pub fn image_matches_key(page_key: &str, image_key: &str, has_image: bool) -> bool {
    has_image && !page_key.is_empty() && page_key == image_key
}
/// What draws: the process's GPU (solid_gpu::shared), or the cores when there is none or it fails.
/// `cpu`: the layers made one mesh for the CPU's drawing, and the layers it was made of (Weak: no later mesh can take their addresses).
#[derive(Default)] struct Drawer { cpu: Option<(Vec<std::sync::Weak<Mesh>>, Arc<Mesh>)> }
impl Drawer {
    fn draw(&mut self, model: &Layers, fades: &[f32], cam: &Camera, w: usize, h: usize, ss: usize, back: &raster::Backdrop, alive: &(dyn Fn() -> bool + Sync)) -> Option<(Vec<u8>, &'static str)> {
        #[cfg(target_os = "linux")] {
            let mut gpu = super::solid_gpu::shared().lock().unwrap_or_else(|e| e.into_inner());
            if let Some(g) = gpu.as_mut() { match g.render_layers(&model.list(), fades, cam, w, h, back) { Some(px) => return Some((px, "gpu")), None => { log::warn!("3D page: the GPU failed a picture, drawing on the CPU from now on"); std::mem::forget(gpu.take()); } } }
        }
        let mesh = self.cpu_mesh(model)?;
        raster::render_faded(&mesh, fades, cam, w, h, ss, back, alive).map(|px| (px, "cpu"))
    }
    /// The layers as one mesh for the CPU's drawing: the one made last when they are its layers, that one with the new layers appended when
    /// they are its layers and more (parts arriving: not every layer copied again a part), else made again.
    fn cpu_mesh(&mut self, model: &Layers) -> Option<Arc<Mesh>> {
        let list = model.list();
        let same = |of: &[std::sync::Weak<Mesh>], n: usize| of.len() <= list.len() && n == of.len() && of.iter().zip(&list).all(|(w, l)| std::sync::Weak::as_ptr(w) == Arc::as_ptr(l.0));
        let made = self.cpu.as_ref().is_some_and(|(of, _)| same(of, list.len()));
        // parts arriving add layers after the ones drawn: those are appended to the mesh made (not every layer copied again a part)
        let grows = !made && self.cpu.as_ref().is_some_and(|(of, _)| same(of, of.len()));
        if grows {
            let (of, m) = self.cpu.take().unwrap();
            let mut m = Arc::try_unwrap(m).unwrap_or_else(|a| (*a).clone());
            for (l, g) in &list[of.len()..] { append(&mut m, l, g.unwrap_or(0)); }
            self.cpu = Some((list.iter().map(|l| Arc::downgrade(l.0)).collect(), Arc::new(m)));
        } else if !made { self.cpu = model.combined().map(|m| (list.iter().map(|l| Arc::downgrade(l.0)).collect(), Arc::new(m))); }
        Some(self.cpu.as_ref()?.1.clone())
    }
    /// The model's buffer off the GPU (the page is put away).
    fn forget(&mut self) {
        #[cfg(target_os = "linux")] { if let Some(g) = super::solid_gpu::shared().lock().unwrap_or_else(|e| e.into_inner()).as_mut() { g.forget(); } }
    }
}
/// Frames queue (every part frame counts; a burst is applied at once, then one model goes to the page); a render request is the latest only.
/// `forget`: the page was put away: the model's GPU buffer goes (the next picture uploads it again).
#[derive(Default)] struct Pending { frames: Vec<Vec<u8>>, render: Option<Job>, shutdown: bool, forget: bool }
struct Worker { slot: Arc<(Mutex<Pending>, Condvar)>, ticket: Arc<AtomicU64> }
impl Worker {
    fn spawn(tx: async_channel::Sender<Msg>) -> Worker {
        let slot: Arc<(Mutex<Pending>, Condvar)> = Arc::new((Mutex::new(Pending::default()), Condvar::new()));
        let ticket = Arc::new(AtomicU64::new(0));
        let (s2, g2) = (slot.clone(), ticket.clone());
        std::thread::Builder::new().name("solid-render".into()).spawn(move || { let (mut drawer, mut scene) = (Drawer::default(), Scene::default()); loop {
            let (frames, render) = { let (m, cv) = &*s2; let mut p = m.lock().unwrap();
                while p.frames.is_empty() && p.render.is_none() && !p.shutdown && !p.forget { p = cv.wait(p).unwrap(); }
                if p.shutdown { return }
                if std::mem::take(&mut p.forget) && p.render.is_none() { drawer.forget(); }
                (std::mem::take(&mut p.frames), p.render.take()) };
            let (mut changed, mut board, mut reset) = (false, false, false);
            for bytes in &frames {
                if bytes.len() > BUILDING_BYTES { if let Some(k) = peek_key(bytes) { if tx.send_blocking(Msg::Building(k)).is_err() { return } } }
                let Some(f) = parse_frame(bytes) else { continue };
                match scene.take(f, &mut board, &mut reset) { Ok(c) => changed |= c, Err(f) => if tx.send_blocking(Msg::Mesh(f)).is_err() { return } }
            }
            if changed {
                let mesh = Layers::of(scene.board.clone(), scene.parts.clone()).map(Arc::new); // no copy: a new part is one more layer
                if tx.send_blocking(Msg::Scene { key: scene.key.clone(), mesh, board, reset, groups: scene.parts.len(), done: scene.done, of: scene.of }).is_err() { return }
            }
            if let Some((key, mesh, fades, cam, view_rev, w, h, ss, my, draft, back)) = render {
                let t0 = std::time::Instant::now();
                let g = g2.clone();
                if let Some((px, by)) = drawer.draw(&mesh, &fades, &cam, w, h, ss, &back, &move || wanted(draft, my, g.load(Ordering::Relaxed))) {
                    if let Some(image) = to_image(px, w, h) { if tx.send_blocking(Msg::Image { ticket: my, key, view_rev, image, ms: t0.elapsed().as_secs_f64() * 1000.0, tris: mesh.tris(), w, h, by, draft }).is_err() { return } }
                }
            }
        } }).ok();
        Worker { slot, ticket }
    }
    fn forget(&self) { let (m, cv) = &*self.slot; m.lock().unwrap().forget = true; cv.notify_one(); }
    fn submit_frame(&self, bytes: Vec<u8>) { let (m, cv) = &*self.slot; m.lock().unwrap().frames.push(bytes); cv.notify_one(); }
    /// Ask for an image of `mesh` for source `key` at `view_rev`, replacing any request not yet begun.
    fn submit_render(&self, key: String, mesh: Arc<Layers>, fades: Arc<Vec<f32>>, cam: Camera, view_rev: u64, w: usize, h: usize, ss: usize, draft: bool, back: raster::Backdrop) -> u64 {
        let ticket = self.ticket.fetch_add(1, Ordering::Relaxed) + 1;
        let (m, cv) = &*self.slot; m.lock().unwrap().render = Some((key, mesh, fades, cam, view_rev, w, h, ss, ticket, draft, back)); cv.notify_one(); ticket
    }
}
impl Drop for Worker { fn drop(&mut self) { let (m, cv) = &*self.slot; m.lock().unwrap().shutdown = true; self.ticket.fetch_add(1, Ordering::Relaxed); cv.notify_one(); } }

// ---- the page ---------------------------------------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq)] enum Drag { Orbit, Pan, Roll, Zoom }
/// What the demo/metrics read: how many images arrived, the last render's time.
#[derive(Clone, Debug, Default)] pub struct Stats { pub images: u64, pub painted: u64, pub last_ms: f64, pub last_tris: usize, pub last_size: (usize, usize), pub by: &'static str }

/// A note (no model for this source) keeps the page's model only when it is the same source's: a different key drops it.
/// Whether the panel's shape differs enough from the one the camera was fitted for to fit it again.
pub fn aspect_changed(was: f32, now: f32) -> bool { was <= 0.0 || (now / was - 1.0).abs() > 0.03 }
pub fn note_keeps_model(page_key: &str, note_key: &str) -> bool { page_key == note_key }
/// Whether a picture taken off the screen during window frame `at` is surely no longer read by the GPU when frame `now` is built: the
/// frame that last drew it (`at`) has ended by the end of the next one's draw.
pub fn gone_from_gpu(at: u64, now: u64) -> bool { at + 2 <= now }

/// A model being made: its key, what is happening, since when, about how long it took the last time (ms, 0 unknown).
#[derive(Clone, Debug, PartialEq)] pub struct Stage { pub key: String, pub text: String, pub since: std::time::Instant, pub about: u64, pub retrying: bool }
/// The stage as the page shows it: the hub's words, the seconds so far (from 1 s) and the hub's estimate (the last time's, or one from the
/// file's size), when long enough to say.
pub fn stage_line(text: &str, secs: u64, about_ms: u64) -> String {
    crate::screens::panestate::loading_line(text, secs, about_ms)
}
/// What the page asks of the window: the model of this source made again (its Retry, after the hub could not make it).
/// `Refine`: the person zoomed in on a STEP: its solids near `at` (the model's millimetres) within `r`, meshed finer (kicad.refine).
pub enum SolidEvent { Retry(String), Refine(String, [f32; 3], f32), /** a click on a board's model: the reference of the part under it, "" the board or nothing */ Pick(String) }
impl EventEmitter<SolidEvent> for SolidPage {}
/// Whether a source's model can be refined: a STEP's (the Mech tab's parts; a board's model is made at its finest already).
pub fn refinable(key: &str) -> bool {
    let path = key.splitn(3, '|').nth(2).unwrap_or("").to_ascii_lowercase();
    is_solid_key(key) && (path.ends_with(".step") || path.ends_with(".stp"))
}
/// How zoomed in a view must be for its part to be meshed finer: the half of the view it sees under this share of the model's size.
pub const REFINE_AT: f32 = 0.35;
/// What a view at rest asks to be refined: the point it looks at and the half-size of what it sees (with room around it), when it sees
/// less than REFINE_AT of the model (`radius`: the model's); None when the model as meshed is fine enough at that size.
pub fn refine_focus(cam: &Camera, radius: f32, aspect: f32) -> Option<([f32; 3], f32)> {
    let half = (FOV / 2.0).tan() * cam.dist * aspect.max(1.0);
    (half < radius * REFINE_AT).then(|| (cam.target, half * 1.5))
}
/// Whether the sphere `b` lies within the sphere `a` (a view inside one refined already asks nothing).
pub fn covers(a: ([f32; 3], f32), b: ([f32; 3], f32)) -> bool { dot(sub(a.0, b.0), sub(a.0, b.0)).sqrt() + b.1 <= a.1 }
/// The watch that asks the hub for a 3D source again: kind "3d" with the key's root and path ("2|<root>|<path>").
pub fn retry_watch(key: &str) -> Option<serde_json::Value> {
    let mut p = key.splitn(3, '|');
    let (k, root, path) = (p.next()?, p.next()?, p.next()?);
    is_solid_key(key).then(|| serde_json::json!({"kind": if k == "2" { "3d" } else if k == "3" { "3d-bare" } else { "3d-full" }, "root": root, "path": path, "tree": ""}))
}
/// Target render pixel size `(w, h)` for a panel of logical size `(pw, ph)` at scale `sf`.
/// Returns `None` when the panel has not been laid out yet (`pw <= 0.0 || ph <= 0.0`).
pub fn target_px(pw: f32, ph: f32, sf: f32, draft: bool, draft_scale: f32) -> Option<(usize, usize)> {
    if pw <= 0.0 || ph <= 0.0 { return None; }
    let (mut w, mut h) = (((pw * sf) as usize).max(64), ((ph * sf) as usize).max(48));
    if w * h > MAX_PIXELS { let k = (MAX_PIXELS as f32 / (w * h) as f32).sqrt(); w = (w as f32 * k) as usize; h = (h as f32 * k) as usize; }
    let most = (DRAFT_PIXELS as f32 * draft_scale) as usize;
    if draft && w * h > most { let k = (most as f32 / (w * h) as f32).sqrt(); w = ((w as f32 * k) as usize).max(32); h = ((h as f32 * k) as usize).max(24); }
    Some((w, h))
}
/// A cached image kept alongside a `Kept` model, tagged with the exact size, view revision, backdrop and probe that produced it.
struct KeptImage { image: Arc<RenderImage>, size: (usize, usize), view_rev: u64, back: raster::Backdrop, probe: String }
/// The model the page showed before the one on show (one: up to 60 MB of a big board's triangles), with its camera: going back to it
/// (3D after a Mech part, a thread back) shows it at once while the hub sends it again. `groups`: its part frames.
/// `image`: its last picture, as CPU bytes (out of the atlas; painted again only when size, view revision, backdrop and probe still match).
struct Kept { key: String, mesh: Arc<Layers>, cam: Camera, view_rev: u64, touched: bool, fit_aspect: f32, parts: (u32, u32), groups: usize, image: Option<KeptImage> }
struct SceneResult { clear_building_stage: bool, clear_stage: bool, clear_failed: bool, clear_note: bool, should_render: bool, notify: bool }
/// Pure scene/camera/image/cache state machine for `SolidPage` (exercised directly by unit tests).
struct SolidCore {
    probe: String,
    lit: Option<(Arc<Layers>, String, Option<Arc<Layers>>)>,
    mesh: Option<Arc<Layers>>,
    key: String,
    cam: Option<Camera>,
    touched: bool,
    fit_aspect: f32,
    fitted: ([f32; 3], f32),
    refined: Vec<([f32; 3], f32)>,
    view_rev: u64,
    image: Option<Arc<RenderImage>>,
    image_key: String,
    image_size: (usize, usize),
    image_view_rev: u64,
    image_gen: u64,
    retired: Vec<(Arc<RenderImage>, u64)>,
    frames: u64,
    born: Vec<std::time::Instant>,
    parts: (u32, u32),
    prev: Option<Kept>,
    restored: Option<usize>,
    back: raster::Backdrop,
    asked: (usize, usize),
    latest: u64,
    on_screen: u64,
    floor: u64,
}
impl SolidCore {
    fn new() -> Self {
        SolidCore {
            probe: String::new(), lit: None, mesh: None, key: String::new(), cam: None, touched: false, fit_aspect: 0.0,
            fitted: ([0.0; 3], 0.0), refined: Vec::new(), view_rev: 0, image: None, image_key: String::new(),
            image_size: (0, 0), image_view_rev: 0, image_gen: 0, retired: Vec::new(), frames: 0, born: Vec::new(),
            parts: (0, 0), prev: None, restored: None, back: raster::Backdrop::DARK, asked: (0, 0), latest: 0, on_screen: 0, floor: 0,
        }
    }
    /// Retire the current image and clear all image metadata so a new source never paints or caches the previous model's pixels.
    fn clear_image(&mut self) {
        if let Some(i) = self.image.take() { self.retired.push((i, self.frames)); }
        self.image_key.clear();
        self.image_size = (0, 0);
        self.image_view_rev = 0;
    }
    /// Advance the view revision on a camera/backdrop/probe/mesh change without clearing the current image (keeps same-key moves flash-free).
    fn bump_view(&mut self) { self.view_rev = self.view_rev.wrapping_add(1); }
    fn drawn(&mut self) -> Option<Arc<Layers>> {
        let mesh = self.mesh.clone()?;
        if self.probe.is_empty() { self.lit = None; return Some(mesh) }
        if !self.lit.as_ref().is_some_and(|l| Arc::ptr_eq(&l.0, &mesh) && l.1 == self.probe) { self.lit = Some((mesh.clone(), self.probe.clone(), lit_layers(&mesh, &self.probe).map(Arc::new))); }
        Some(self.lit.as_ref().and_then(|l| l.2.clone()).unwrap_or(mesh))
    }
    /// The model on show becomes the one kept (Kept). Only caches `image` when it was rendered for `self.key` and the current `view_rev`.
    fn keep_current(&mut self) {
        if let (Some(mesh), Some(cam)) = (self.mesh.clone(), self.cam) {
            let image = (image_matches_key(&self.key, &self.image_key, self.image.is_some()) && self.image_view_rev == self.view_rev)
                .then(|| self.image.clone().map(|image| KeptImage { image, size: self.image_size, view_rev: self.image_view_rev, back: self.back, probe: self.probe.clone() }))
                .flatten();
            self.prev = Some(Kept { key: self.key.clone(), mesh, cam, view_rev: self.view_rev, touched: self.touched, fit_aspect: self.fit_aspect, parts: self.parts, groups: self.born.len(), image });
        }
    }
    /// Save the outgoing source into `prev` and clear all active mesh/camera/image state so nothing of the old source can be rendered under the new key.
    fn switch_source_reset(&mut self) {
        self.keep_current();
        self.clear_image();
        self.mesh = None;
        self.cam = None;
        self.lit = None;
        self.restored = None;
        self.asked = (0, 0);
        self.floor = self.latest + 1;
        self.refined.clear();
    }
    fn apply_scene(&mut self, key: String, mesh: Option<Arc<Layers>>, board: bool, reset: bool, groups: usize, done: u32, of: u32, panel_wh: (f32, f32)) -> SceneResult {
        if key != self.key { self.switch_source_reset(); }
        // a model brought back stays until the hub's stream of it has caught up (no parts going and fading in again)
        if key == self.key && let Some(g) = self.restored {
            if groups < g && !(of > 0 && done >= of) && mesh.is_some() {
                return SceneResult { clear_building_stage: true, clear_stage: false, clear_failed: false, clear_note: false, should_render: false, notify: false };
            }
            self.restored = None;
            let shown = std::time::Instant::now() - std::time::Duration::from_secs(1);
            self.born = vec![shown; groups];
            self.parts = (done, of);
            let should_render = if let Some(m) = mesh {
                self.floor = self.latest + 1;
                self.mesh = Some(m);
                self.bump_view();
                true
            } else {
                false
            };
            return SceneResult { clear_building_stage: true, clear_stage: false, clear_failed: false, clear_note: false, should_render, notify: true };
        }
        let fresh = key != self.key || self.cam.is_none();
        self.key = key;
        if reset { self.born.clear(); }
        let now = std::time::Instant::now();
        self.born.truncate(groups);
        while self.born.len() < groups { self.born.push(now); } // a part fades in from when it shows
        self.parts = (done, of);
        let Some(mesh) = mesh else {
            return SceneResult { clear_building_stage: true, clear_stage: board, clear_failed: true, clear_note: false, should_render: false, notify: true };
        };
        if board || fresh { self.floor = self.latest + 1; } // pictures of the old board are stale; ones without the newest part are not
        if fresh {
            let (pw, ph) = panel_wh;
            self.fit_aspect = pw / ph.max(1.0);
            self.touched = false;
            self.cam = Some(Camera::fit_to(mesh.center, mesh.radius, self.fit_aspect));
            self.fitted = (mesh.center, mesh.radius);
        }
        // a model coming in parts with nothing under them (a STEP streaming) grows: the camera follows it until the person moves it
        else if !self.touched && refit(self.fitted, (mesh.center, mesh.radius)) {
            self.cam = Some(Camera::fit_to(mesh.center, mesh.radius, self.fit_aspect.max(0.05)));
            self.fitted = (mesh.center, mesh.radius);
        }
        self.mesh = Some(mesh);
        self.bump_view();
        SceneResult { clear_building_stage: true, clear_stage: board, clear_failed: true, clear_note: true, should_render: true, notify: true }
    }
    fn apply_note(&mut self, key: String) {
        if !note_keeps_model(&self.key, &key) {
            // another source's model could not be made: the old source's model must not show under this key
            self.keep_current();
            self.restored = None;
            self.mesh = None;
            self.cam = None;
            self.lit = None;
            self.clear_image();
            self.floor = self.latest + 1; // a render of the old model still in flight is stale
            self.born.clear();
            self.parts = (0, 0);
        }
        self.key = key;
    }
    fn apply_image(&mut self, ticket: u64, key: String, view_rev: u64, image: Arc<RenderImage>, w: usize, h: usize) -> bool {
        if !shows_for_key(&self.key, &key, ticket, self.on_screen, self.floor) { return false; } // stale key or ticket: never paint another model's picture
        self.on_screen = ticket;
        self.image_key = key;
        self.image_size = (w, h);
        self.image_view_rev = view_rev;
        self.image_gen = self.image_gen.wrapping_add(1);
        if let Some(old) = self.image.replace(image) { self.retired.push((old, self.frames)); }
        true
    }
    /// Restore `prev` when it matches `key`. Restores its cached pixels immediately only when viewport size, view revision, backdrop and probe still match.
    fn restore_kept(&mut self, key: &str, panel_wh_sf: (f32, f32, f32), draft_scale: f32) -> bool {
        if key.is_empty() || key == self.key || self.prev.as_ref().map_or(true, |p| p.key != key) { return false; }
        let Some(k) = self.prev.take() else { return false; };
        self.keep_current();
        self.clear_image();
        let want_px = target_px(panel_wh_sf.0, panel_wh_sf.1, panel_wh_sf.2, false, draft_scale);
        if let Some(ki) = k.image
            && ki.view_rev == k.view_rev
            && want_px == Some(ki.size)
            && ki.back == self.back
            && ki.probe == self.probe
        {
            self.retired.retain(|(r, _)| !Arc::ptr_eq(r, &ki.image));
            self.image = Some(ki.image);
            self.image_key = k.key.clone();
            self.image_size = ki.size;
            self.image_view_rev = ki.view_rev;
            self.image_gen = self.image_gen.wrapping_add(1);
            self.on_screen = self.latest;
        }
        self.fitted = (k.mesh.center, k.mesh.radius);
        self.key = k.key;
        self.mesh = Some(k.mesh);
        self.cam = Some(k.cam);
        self.view_rev = k.view_rev;
        self.lit = None;
        self.touched = k.touched;
        self.fit_aspect = k.fit_aspect;
        self.parts = k.parts;
        let shown = std::time::Instant::now() - std::time::Duration::from_secs(1);
        self.born = vec![shown; k.groups];
        self.restored = Some(k.groups);
        self.floor = self.latest + 1;
        self.asked = (0, 0);
        true
    }
    fn prepare_render(&mut self, panel_wh_sf: (f32, f32, f32), draft: bool, draft_scale: f32) -> Option<(String, Arc<Layers>, Camera, u64, usize, usize)> {
        let (mesh, cam) = (self.drawn()?, self.cam?);
        let (w, h) = target_px(panel_wh_sf.0, panel_wh_sf.1, panel_wh_sf.2, draft, draft_scale).unwrap_or((64, 48));
        self.asked = (w, h);
        Some((self.key.clone(), mesh, cam, self.view_rev, w, h))
    }
}
pub struct SolidPage {
    /// the button now held down, whose release ends the drag (the table decides what it does)
    press_btn: Option<MouseButton>,
    /** where the left button went down, until the pointer leaves it by 4 px (a click picks the part under it) */ press: Option<Point<Pixels>>,
    /** the page names the probed part beside its tools (a Split names it in its own bar: off there) */ pub names_probe: bool,
    core: SolidCore,
    drag: Option<(Drag, Point<Pixels>)>,
    /** whether the last picture asked for was mid-fade */
    fading: bool,
    /** the page's keys (0.14's: f/Home fit, 0-6 views, arrows turn, shift+arrows move, +/- zoom), taken when the model is pressed; the wheel's burst, to settle after it */ focus: FocusHandle, wheel: u64,
    /** what the hub said it is doing for a model (Stage), shown with its time running; whether a second's tick is running for it */ stage: Option<Stage>, ticking: bool,
    /** the share of DRAFT_PIXELS a picture in motion gets, from how long the last ones took (draft_scale) */ draft_scale: f32,
    /** the hub could not make this source's model (its Note): the page offers Retry when retryable */ failed: bool, retryable: Option<bool>, /** last Retry press time (1 s client cooldown) */ last_retry: Option<std::time::Instant>,
    pub note: String, /** room (px) the page's floating tools leave free at its top right, for the viewer pane's controls */ pub reserve: f32, worker: Worker, size: Arc<Mutex<[f32; 5]>> /* panel width, height, scale factor, origin x, y (logical px) */,
    /** runs in normal clients too (one relaxed atomic per qualifying paint); measures the GPUI paint callback with non-zero bounds, not GPU presentation */
    painted: Arc<AtomicU64>, painted_gen: Arc<AtomicU64>, pub stats: Stats,
}
impl SolidPage {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let (tx, rx) = async_channel::bounded::<Msg>(4);
        cx.spawn(async move |weak, cx| {
            while let Ok(msg) = rx.recv().await {
                if weak.update(cx, |this, cx| this.apply(msg, cx)).is_err() { break }
            }
        }).detach();
        SolidPage { press_btn: None, press: None, names_probe: true, core: SolidCore::new(), drag: None, fading: false, focus: cx.focus_handle(), wheel: 0, stage: None, ticking: false, draft_scale: 1.0, failed: false, retryable: None, last_retry: None, note: "No 3D model yet".into(), /* i18n-ok: the note is drawn through note_words */ reserve: 0.0, worker: Worker::spawn(tx),
            size: Arc::new(Mutex::new([0.0, 0.0, 1.0, 0.0, 0.0])), painted: Arc::new(AtomicU64::new(0)), painted_gen: Arc::new(AtomicU64::new(0)), stats: Stats::default() }
    }
    /// The background to draw on (the setting resolved by the window): a change draws the picture again at once, the camera untouched.
    pub fn set_backdrop(&mut self, back: raster::Backdrop) { if self.core.back != back { self.core.back = back; self.core.bump_view(); self.ask_render(false); } }
    /// The part the hub's probe names ("" none): drawn lit on a board's model, the picture made again when it changes.
    pub fn set_probe(&mut self, r: &str) { if self.core.probe != r { self.core.probe = r.to_string(); self.core.bump_view(); self.ask_render(false); } }
    /// A click on a board's model (a press let go where it went down): the part under it is lit and told to the window (`SolidEvent::Pick`), which
    /// makes it the board's probe; the board itself or the background clears it. A model that names no parts (a STEP, a GLB) is not picked on.
    /// A button went down: the table says what its drag does and whether its click selects.
    fn mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        let Some(b) = crate::mousecfg::button(e.button) else { return };
        let (scheme, m) = (crate::mousecfg::solid(), crate::mousecfg::mods(&e.modifiers));
        // a double click of the selecting button fits, as 0.14's did
        if e.click_count >= 2 && scheme.selects(b, crate::mouse::Mods::NONE) { self.drag = None; self.press = None; self.set_view("fit"); cx.notify(); return }
        self.press = scheme.selects(b, m).then_some(e.position);
        self.drag = match scheme.drag(b, m) { crate::mouse::Action::Rotate => Some(Drag::Orbit), crate::mouse::Action::Pan => Some(Drag::Pan), crate::mouse::Action::Roll => Some(Drag::Roll), crate::mouse::Action::Zoom => Some(Drag::Zoom), _ => None }.map(|d| (d, e.position));
        self.press_btn = Some(e.button);
    }
    /// A button came up: its drag ends, and a click of the selecting button that did not move picks.
    fn mouse_up(&mut self, e: &MouseUpEvent, cx: &mut Context<Self>) {
        if self.press_btn != Some(e.button) { return }
        self.press_btn = None;
        let click = self.press.take(); if self.drag.take().is_some() { self.settled(cx); cx.notify(); } if let Some(at) = click { self.click_at(at, cx); }
    }
    fn click_at(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let (Some(model), Some(cam)) = (self.core.mesh.clone(), self.core.cam) else { return };
        if !has_refs(&model) { return }
        let [pw, ph, _, ox, oy] = *self.size.lock().unwrap();
        if pw <= 0.0 || ph <= 0.0 { return }
        let (eye, dir) = cam.ray((f32::from(at.x) - ox) / pw * 2.0 - 1.0, 1.0 - (f32::from(at.y) - oy) / ph * 2.0, pw / ph);
        let r = pick(&model, eye, dir).unwrap_or_default();
        eprintln!("GPUI_LOG {}", serde_json::json!({"event": "solid_pick", "ref": r}));
        self.set_probe(&r); cx.emit(SolidEvent::Pick(r)); cx.notify();
    }
    /// The key of the model the page holds now (set when a decoded frame is applied, not when one is queued): "" before any.
    pub fn key(&self) -> &str { &self.core.key }
    /// The source key the currently painted image was rendered for ("" when no image is held for the current model).
    pub fn image_key(&self) -> &str { &self.core.image_key }
    /// Number of distinct rendered images that have reached the GPUI paint callback with non-zero bounds
    /// (runs in normal clients too as one relaxed atomic per qualifying paint; measures the paint callback, not GPU presentation).
    pub fn painted_at_canvas_size(&self) -> bool {
        let [w, h, scale, _, _] = *self.size.lock().unwrap();
        target_px(w, h, scale, false, self.draft_scale) == Some(self.stats.last_size)
            && self.painted_gen.load(Ordering::Relaxed) == self.core.image_gen
            && self.painted_count() > 0
    }
    pub fn painted_count(&self) -> u64 { self.painted.load(Ordering::Relaxed) }
    /// The source the page is showing: the one being made when the hub is making another than the model held (the page then shows its
    /// progress), else `key`. The viewer shows the page when this is the source it asks for.
    pub fn shown_key(&self) -> &str { match &self.stage { Some(s) if s.key != self.core.key => &s.key, _ => &self.core.key } }
    /// The stage line for the page, "" when nothing is being made.
    pub fn stage_text(&self) -> String { self.stage.as_ref().map_or(String::new(), |s| stage_line(&s.text, s.since.elapsed().as_secs(), s.about)) }
    fn set_stage(&mut self, st: Option<Stage>, cx: &mut Context<Self>) {
        self.stage = st;
        if self.stage.is_some() && !self.ticking { // the time runs on the screen: one repaint a second while something is being made; a Retry stage expires after 15 s
            self.ticking = true;
            cx.spawn(async move |weak, cx| loop {
                cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
                let go = weak.update(cx, |this, cx| {
                    let Some(s) = &this.stage else { this.ticking = false; return false };
                    if s.retrying && crate::screens::panestate::retry_expired(s.since, std::time::Instant::now()) {
                        this.stage = None; this.ticking = false; cx.notify(); return false
                    }
                    cx.notify(); true
                }).unwrap_or(false);
                if !go { break }
            }).detach();
        }
        cx.notify();
    }
    /// A plot-lane binary frame (keys 2|, 3|, 4| are solids).
    /// None: taken (a solid frame, decoded on the worker; the bytes are moved, not copied). Some(bytes): not a solid frame, handed back unread.
    pub fn accept(&mut self, bytes: Vec<u8>) -> Option<Vec<u8>> {
        if peek_key(&bytes).map_or(false, |k| is_solid_key(&k)) { self.worker.submit_frame(bytes); None } else { Some(bytes) }
    }
    fn apply(&mut self, msg: Msg, cx: &mut Context<Self>) {
        match msg {
            Msg::Scene { key, mesh, board, reset, groups, done, of } => {
                let [pw, ph, _, _, _] = *self.size.lock().unwrap();
                let res = self.core.apply_scene(key, mesh, board, reset, groups, done, of, (pw, ph));
                let mut stage_cleared = false;
                if res.clear_building_stage && self.stage.as_ref().is_some_and(|s| s.key == self.core.key && (res.clear_stage || s.text == BUILDING)) {
                    self.stage = None;
                    stage_cleared = true;
                }
                if res.clear_failed { self.failed = false; self.retryable = None; }
                if res.clear_note { self.note.clear(); }
                if res.should_render { self.ask_render(self.drag.is_some()); }
                if res.notify || stage_cleared { cx.notify(); }
            }
            Msg::Mesh(SolidFrame::Model(..) | SolidFrame::Parts(..) | SolidFrame::Part(..)) => {} // the worker makes these a Scene
            Msg::Mesh(SolidFrame::Note(key, why, retryable)) => { // why there is no model; the same source's previous model stays shown
                self.core.apply_note(key);
                self.stage = None;
                self.failed = !why.is_empty() && !crate::screens::panestate::is_empty_none(&why);
                self.retryable = retryable;
                self.note = if self.core.mesh.is_some() { format!("{why}{PREVIOUS_SHOWN}") } else { why }; cx.notify();
            }
            Msg::Mesh(SolidFrame::Bad(e)) => { self.failed = true; self.retryable = Some(true); self.note = e; self.set_stage(None, cx); }
            Msg::Mesh(SolidFrame::Stage(key, text, ms, about)) => {
                if key == self.core.key && !text.is_empty() { self.failed = false; }
                if text.is_empty() && self.stage.as_ref().is_some_and(|s| s.key != key) { return } // another source's end: not this one's stage
                let st = (!text.is_empty()).then(|| Stage { key, text, since: std::time::Instant::now().checked_sub(std::time::Duration::from_millis(ms)).unwrap_or_else(std::time::Instant::now), about, retrying: false });
                if st.is_none() && self.stage.as_ref().is_some_and(|s| s.text == BUILDING) { return } // the model is still being decoded here
                self.set_stage(st, cx);
            }
            Msg::Building(key) => self.set_stage(Some(Stage { key, text: BUILDING.into(), since: std::time::Instant::now(), about: 0, retrying: false }), cx),
            Msg::Image { ticket, key, view_rev, image, ms, tris, w, h, by, draft } => {
                if draft { self.draft_scale = draft_scale(self.draft_scale, ms); }
                if !self.core.apply_image(ticket, key, view_rev, image, w, h) { return }
                self.stats = Stats { images: self.stats.images + 1, painted: self.painted_count(), last_ms: ms, last_tris: tris, last_size: (w, h), by }; cx.notify();
                // a fade is drawn picture after picture while it runs (no timer: the next is asked when this one is in), then once more at rest
                if ticket == self.core.latest && self.drag.is_none() && self.fading { let still = self.fades().1; self.ask_render(still); }
            }
        }
    }
    /// The viewer asks for `key`: when it is the model kept, it comes back at once, with its camera (the hub sends it again meanwhile).
    pub fn want(&mut self, key: &str, cx: &mut Context<Self>) {
        let [pw, ph, sf, _, _] = *self.size.lock().unwrap();
        if !self.core.restore_kept(key, (pw, ph, sf), self.draft_scale) { return }
        if self.stage.as_ref().is_some_and(|s| s.key == self.core.key) { self.stage = None; }
        self.note.clear(); self.failed = false; self.ask_render(false); cx.notify();
        eprintln!("GPUI_METRIC {{\"event\":\"solid_restored\",\"triangles\":{}}}", self.core.mesh.as_ref().map_or(0, |m| m.tris()));
    }
    /// Each part group's fade now (index 0 the board), and whether any is still fading.
    fn fades(&self) -> (Vec<f32>, bool) {
        let mut v = Vec::with_capacity(self.core.born.len() + 1); v.push(1.0); let mut any = false;
        for b in &self.core.born { let f = fade_at(b.elapsed().as_secs_f32() * 1000.0); any |= f < 1.0; v.push(f); }
        (v, any)
    }
    /// The parts' progress for the page ("" when none is awaited).
    pub fn parts_progress(&self) -> String { progress(self.core.parts.0, self.core.parts.1) }
    /// Request an image at the panel's device-pixel size (half size for a draft), within MAX_PIXELS.
    fn ask_render(&mut self, draft: bool) {
        let (fades, fading) = self.fades(); self.fading = fading;
        let [pw, ph, sf, _, _] = *self.size.lock().unwrap();
        let Some((key, mesh, cam, view_rev, w, h)) = self.core.prepare_render((pw, ph, sf), draft, self.draft_scale) else { return };
        self.core.latest = self.worker.submit_render(key, mesh, Arc::new(fades), cam, view_rev, w, h, samples(w, h, draft), draft, self.core.back);
    }
    /// The toolbar's Fit (the first fitted view again, camera untouched) and its standard views.
    pub fn set_view(&mut self, name: &str) {
        let Some(m) = self.core.mesh.clone() else { return };
        let fitted = Camera::fit_to(m.center, m.radius, self.core.fit_aspect.max(0.05));
        if name == "fit" { self.core.cam = Some(fitted); self.core.touched = false; self.core.fitted = (m.center, m.radius); }
        else { self.core.cam = Some(fitted.named(name)); self.core.touched = true; }
        self.core.bump_view();
        self.ask_render(false);
    }
    /// A move of the camera by a key or the wheel: a draft now, the full picture once nothing has moved it for 150 ms.
    fn moved(&mut self, cx: &mut Context<Self>) {
        self.core.touched = true; self.core.bump_view(); self.ask_render(true); self.wheel += 1; let n = self.wheel;
        cx.spawn(async move |weak, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(150)).await;
            let _ = weak.update(cx, |this, cx| if this.wheel == n && this.drag.is_none() { this.settled(cx) });
        }).detach();
    }
    /// The view came to rest: the full picture, and, zoomed in on a STEP part, its solids there asked to be meshed finer (once a region:
    /// the finer model replaces this one when the hub has it, the camera where the person left it).
    fn settled(&mut self, cx: &mut Context<Self>) {
        self.ask_render(false);
        let (Some(m), Some(c)) = (&self.core.mesh, self.core.cam) else { return };
        if !refinable(&self.core.key) { return }
        let Some(f) = refine_focus(&c, m.radius, self.core.fit_aspect.max(0.05)) else { return };
        if self.core.refined.iter().any(|r| covers(*r, f)) { return }
        self.core.refined.push(f); cx.emit(SolidEvent::Refine(self.core.key.clone(), f.0, f.1));
    }
    /// The page's keys, as 0.14's 3D view had them; whether the key was one of them.
    fn on_key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let m = &e.keystroke.modifiers;
        if m.control || m.alt || m.platform || m.function { return false }
        let k = e.keystroke.key.as_str();
        let view = match k { "f" | "home" => Some("fit"), "0" => Some("iso"), "1" => Some("top"), "2" => Some("bottom"), "3" => Some("front"), "4" => Some("right"), "5" => Some("back"), "6" => Some("left"), _ => None };
        if let Some(v) = view { self.set_view(v); cx.notify(); return true }
        let Some(c) = &mut self.core.cam else { return false };
        let h = self.size.lock().unwrap()[1];
        match k {
            "left" | "right" | "up" | "down" => {
                let (dx, dy) = match k { "left" => (-16.0, 0.0), "right" => (16.0, 0.0), "up" => (0.0, -16.0), _ => (0.0, 16.0) };
                if m.shift { c.pan(dx, dy, h) } else { c.orbit(dx, dy) }
            }
            "+" | "=" | "plus" => c.zoom_by(1.15, 0.0, 0.0, 1.0),
            "-" | "_" | "minus" => c.zoom_by(1.0 / 1.15, 0.0, 0.0, 1.0),
            _ => return false,
        }
        self.moved(cx); true
    }
    /// Called after the window successfully enqueues the presenter's `"retry"` action: never clears the error on enqueue; puts the pane into "Trying again…" (bounded at 15 s) until the next frame decides.
    pub fn retry_sent(&mut self, cx: &mut Context<Self>) {
        let now = std::time::Instant::now();
        self.last_retry = Some(now);
        /* i18n-ok: stage_line draws through trs */ self.set_stage(Some(Stage { key: self.core.key.clone(), text: "Trying again…".into(), since: now, about: 0, retrying: true }), cx);
    }
    /// Retry: routed through the window's presenter `"retry"` action (preserving Split and far `on`); disabled during the 1 s cooldown.
    fn retry_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ready = crate::screens::panestate::retry_ready(self.last_retry, std::time::Instant::now());
        let base = div().id("solid-retry").px_2().py_1().text_size(crate::scale::tpx(12.)).border_1().border_color(rgb(0x3a4456)).child(trs("Retry"))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        if !ready {
            return base.text_color(rgb(0x6b7280)).bg(rgba(0x161c26dd));
        }
        base.text_color(rgb(0xc8ced9)).bg(rgba(0x1d2430dd)).cursor_pointer().hover(|d| d.bg(rgba(0x2a3548ee)))
            .on_click(cx.listener(|this, _, _, cx| {
                if !crate::screens::panestate::retry_ready(this.last_retry, std::time::Instant::now()) { return }
                let k = this.core.key.clone(); cx.emit(SolidEvent::Retry(k));
            }))
    }
    /// Snap view: the picture on screen as a PNG in `dir` (never over a file); the path, or why not.
    pub fn snap(&mut self, dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
        /* i18n-ok: a reason inside the translated "Snap view failed" */ let img = self.core.image.clone().ok_or("no picture yet")?;
        let (w, h) = { let s = img.size(0); (s.width.0 as u32, s.height.0 as u32) };
        /* i18n-ok: as above */ let mut px = img.as_bytes(0).ok_or("no picture yet")?.to_vec();
        for p in px.chunks_exact_mut(4) { p.swap(0, 2); } // the container holds BGRA
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let mut path = dir.join(format!("backplane-3d-{stamp}.png")); let mut n = 1;
        while path.exists() { path = dir.join(format!("backplane-3d-{stamp}-{n}.png")); n += 1; }
        /* i18n-ok: as above */ image::RgbaImage::from_raw(w, h, px).ok_or("bad picture")?.save(&path).map_err(|e| e.to_string())?;
        Ok(path)
    }
    /// For the demo and tests: an orbit drag step as the mouse handler does it.
    pub fn drag_by(&mut self, dx: f32, dy: f32) { if let Some(c) = &mut self.core.cam { c.orbit(dx, dy); self.core.bump_view(); } self.ask_render(true); }
    /// For the demo: the button went up, the full picture is asked for.
    pub fn settle(&mut self) { self.ask_render(false); }
    /// Give the images back to the atlas (call before dropping the page, with the window).
    pub fn release(&mut self, window: &mut Window) { self.core.image_key.clear(); self.core.image_size = (0, 0); self.core.image_view_rev = 0; for i in self.core.retired.drain(..).map(|r| r.0).chain(self.core.image.take()) { let _ = window.drop_image(i); } }
    /// The pictures that are (or may still be) in the window's sprite atlas, on the GPU.
    pub fn atlas_images(&self) -> usize { self.core.retired.len() + self.core.image.is_some() as usize }
    /// The page is put away (the viewer closed, another page or thread shown): its pictures leave the atlas and its model the GPU; the model
    /// stays here, on the CPU, so showing the page again draws it again at once. The next paint asks for a new picture.
    pub fn put_away(&mut self) {
        self.core.image_key.clear(); self.core.image_size = (0, 0); self.core.image_view_rev = 0;
        let Some(i) = self.core.image.take() else { return };
        self.core.retired.push((i, self.core.frames)); self.core.asked = (0, 0); self.core.on_screen = self.core.latest; self.worker.forget();
    }
    /// Once a window frame (from the window's render, the page drawn or not): the pictures that left the screen two frames ago leave the
    /// atlas. Not sooner: the frame that last drew one may still be on the GPU (gpui waits for a frame only at the end of the next one's
    /// draw), and gpui destroys an image's texture at once when it is dropped, a texture the GPU would still be reading.
    pub fn collect(&mut self, window: &mut Window) {
        self.core.frames += 1;
        let before = self.core.retired.len();
        let frames = self.core.frames;
        self.core.retired.retain(|(i, at)| if gone_from_gpu(*at, frames) { let _ = window.drop_image(i.clone()); false } else { true });
        if self.core.image.is_none() && self.core.retired.is_empty() && before > 0 { eprintln!("GPUI_METRIC {{\"event\":\"solid_put_away\",\"images_dropped\":{before},\"atlas_images\":{}}}", self.atlas_images()); }
    }

    pub fn run_tool(&mut self, tool: SolidTool, cx: &mut Context<Self>) {
        match tool {
            SolidTool::View(view) => self.set_view(view),
            SolidTool::Snap => {
                /* i18n-ok: a folder name */ let dir = std::env::var_os("BACKPLANE_GPUI_SNAP_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| crate::plat::user_home().map(|h| h.join("Pictures/Backplane")).unwrap_or_default());
                /* i18n-ok: the note is drawn through note_words */ self.note = match self.snap(&dir) { Ok(p) => format!("Saved {}", p.display()), Err(e) => format!("Snap view failed: {e}") };
            }
            SolidTool::ClearProbe => { self.set_probe(""); cx.emit(SolidEvent::Pick(String::new())); }
        }
        cx.notify();
    }

    pub fn probe_tool(&self) -> Option<crate::pane_tool::PaneTool<SolidTool>> {
        (self.names_probe && self.core.lit.as_ref().is_some_and(|l| l.1 == self.core.probe && l.2.is_some()))
            .then(|| crate::pane_tool::PaneTool::new("solid-probe-clear", self.core.probe.clone(), SolidTool::ClearProbe))
    }

    fn view_controls(&self, cx: &mut Context<Self>) -> Div {
        let mut bar = div().flex().flex_wrap().gap_1();
        for tool in solid_tools() {
            bar = bar.child(div().id(SharedString::from(tool.id)).px_2().h(crate::scale::tpx(TOOL_H)).flex().items_center().text_size(crate::scale::tpx(12.)).text_color(rgb(0xc8ced9)).bg(rgba(0x1d2430dd)).border_1().border_color(rgb(0x3a4456)).cursor_pointer()
                .hover(|d| d.bg(rgba(0x2a3548ee))).child(tool.label)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, _, cx| this.run_tool(tool.action, cx))));
        }
        // the probed part, when this model has it: its name in the colour it is lit in, and a press to put it away
        if let Some(tool) = self.probe_tool() {
            bar = bar.child(div().id("solid-probe").pl_2().pr(px(2.)).h(crate::scale::tpx(TOOL_H)).flex().items_center().gap_1().text_size(crate::scale::tpx(12.)).text_color(rgb(0xc8ced9)).bg(rgba(0x1d2430dd)).border_1().border_color(rgb(0xffe066))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(tool.label)
                .child(div().id(SharedString::from(tool.id)).p(px(2.)).cursor_pointer().hover(|d| d.bg(rgba(0x2a3548ee))).child(crate::art::icon("x", 12., rgb(0x8a93a3)))
                    .on_click(cx.listener(move |this, _, _, cx| this.run_tool(tool.action, cx)))));
        }
        bar
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SolidTool { View(&'static str), Snap, ClearProbe }

pub fn solid_tools() -> Vec<crate::pane_tool::PaneTool<SolidTool>> {
    [("viewer.fit", "fit"), ("viewer.c.view.iso", "iso"), ("viewer.c.view.top", "top"), ("viewer.c.view.bottom", "bottom"), ("viewer.c.view.front", "front"), ("viewer.c.view.right", "right"), ("viewer.c.view.back", "back"), ("viewer.c.view.left", "left"), ("viewer.c.view.snap", "snap")].into_iter().map(|(key, view)| {
        let action = if view == "snap" { SolidTool::Snap } else { SolidTool::View(view) };
        crate::pane_tool::PaneTool::new(format!("solid-{view}"), t(key), action)
    }).collect()
}

impl Render for SolidPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.stats.painted = self.painted_count();
        // a panel whose size differs from the last request re-renders at the right size
        let [pw, ph, sf, _, _] = *self.size.lock().unwrap();
        // the model was fitted before the panel's size was known: an untouched camera is fitted again to the real shape
        if let Some(m) = self.core.mesh.clone() { if pw > 0.0 && ph > 0.0 && !self.core.touched && aspect_changed(self.core.fit_aspect, pw / ph) { self.core.fit_aspect = pw / ph; self.core.cam = Some(Camera::fit_to(m.center, m.radius, self.core.fit_aspect)); self.core.fitted = (m.center, m.radius); self.core.bump_view(); self.ask_render(false); } }
        if self.core.mesh.is_some() && pw > 0.0 { let want = (((pw * sf) as usize).max(64), ((ph * sf) as usize).max(48)); if self.drag.is_none() && want.0 * want.1 <= MAX_PIXELS && want != self.core.asked && self.core.asked.0 * 2 != want.0 { self.ask_render(false); } }
        // the model is here and its first picture is being drawn (a big board on a machine without a usable GPU: a second or two): said, not blank
        let stage = { let t = self.stage_text(); if t.is_empty() && self.core.image.is_none() && self.core.mesh.is_some() { trs(DRAWING) } else { t } };
        let making_other = self.stage.as_ref().is_some_and(|s| s.key != self.core.key);
        let showing_image = !making_other && image_matches_key(&self.core.key, &self.core.image_key, self.core.image.is_some());
        let paint_gen = showing_image.then_some(self.core.image_gen);
        let size = self.size.clone();
        let painted = self.painted.clone();
        let painted_gen = self.painted_gen.clone();
        let page = cx.entity().downgrade();
        let probe = canvas(|_, _, _| (), move |b: Bounds<Pixels>, _, window: &mut Window, cx| {
            let new = [f32::from(b.size.width), f32::from(b.size.height), window.scale_factor(), f32::from(b.origin.x), f32::from(b.origin.y)];
            if let Some(g) = paint_gen && new[0] > 0.0 && new[1] > 0.0 && painted_gen.swap(g, Ordering::Relaxed) != g {
                painted.fetch_add(1, Ordering::Relaxed);
            }
            let mut g = size.lock().unwrap();
            let resized = g[..3] != new[..3];
            *g = new;
            drop(g);
            if resized {
                let page = page.clone();
                window.defer(cx, move |_, cx| { let _ = page.update(cx, |_, cx| cx.notify()); });
            }
        }).absolute().top_0().left_0().size_full(); // (at the page's corner: with no place of its own it sat under the picture, and the pointer was measured from the page's bottom edge)
        let body = match &self.core.image {
            _ if making_other || (self.core.image.is_none() && !stage.is_empty()) => div().id("solid-stage").size_full().child(crate::screens::panestate::view_on_backdrop(self.core.back, &crate::screens::panestate::PaneState::loading(stage.clone()), None)).into_any_element(),
            Some(i) => img(i.clone()).size_full().object_fit(ObjectFit::Fill).into_any_element(),
            None => {
                let st = crate::screens::panestate::classify_solid_note(&self.note, self.failed && !self.core.key.is_empty(), self.retryable);
                let retry = st.retry.then(|| self.retry_button(cx).into_any_element());
                crate::screens::panestate::view_on_backdrop(self.core.back, &st, retry)
            }
        };
        let tools = (self.core.mesh.is_some() && !making_other).then(|| {
            self.view_controls(cx).absolute().top(px(TOOL_TOP)).left_2().right(px(8. + self.reserve))
        });
        div().id("solid-page").size_full().overflow_hidden().relative().track_focus(&self.focus).key_context("Viewer").child(body).child(probe).children(tools)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| if this.on_key(e, cx) { cx.stop_propagation() }))
            .when(self.core.image.is_some() && !making_other && !self.note.is_empty(), |d| d.child(div().absolute().bottom_2().left_2().px_2().flex().items_center().gap_2().text_size(crate::scale::tpx(12.)).text_color(rgb(0x8a93a3)).child(note_words(&self.note))
                .when(self.failed && self.retryable.unwrap_or_else(|| crate::screens::panestate::is_retryable_none(&self.note)) && !self.core.key.is_empty(), |d| d.child(self.retry_button(cx)))))
            .when(self.core.image.is_some() && !making_other && (!stage.is_empty() || !self.parts_progress().is_empty()), |d| d.child(div().id("solid-parts").absolute().bottom_2().right_2().px_2().text_size(crate::scale::tpx(12.)).text_color(rgb(0x8a93a3)).child(if stage.is_empty() { trs(&self.parts_progress()) } else { stage.clone() })))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, window, cx| this.mouse_down(e, window, cx)))
            .on_mouse_down(crate::mousecfg::BUTTONS[1], cx.listener(|this, e: &MouseDownEvent, window, cx| this.mouse_down(e, window, cx)))
            .on_mouse_down(crate::mousecfg::BUTTONS[2], cx.listener(|this, e: &MouseDownEvent, window, cx| this.mouse_down(e, window, cx)))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                // (a press that moves is a turn, not a click)
                if let Some(p) = this.press { if f32::from(e.position.x - p.x).abs() > 4.0 || f32::from(e.position.y - p.y).abs() > 4.0 { this.press = None; } }
                let Some((kind, last)) = this.drag else { return };
                if e.pressed_button.is_none() { this.drag = None; this.ask_render(false); cx.notify(); return } // the button went up where we could not see it
                let (dx, dy) = (f32::from(e.position.x - last.x), f32::from(e.position.y - last.y));
                let h = this.size.lock().unwrap()[1];
                this.core.touched = true; if let Some(c) = &mut this.core.cam { match kind { Drag::Orbit => c.orbit(dx, dy), Drag::Pan => c.pan(dx, dy, h), Drag::Roll => c.roll(dx), Drag::Zoom => c.zoom_by(crate::mousecfg::drag_zoom_factor(dy, crate::mousecfg::invert()), 0.0, 0.0, 1.0) } this.core.bump_view(); }
                this.drag = Some((kind, e.position)); this.ask_render(true); // a draft while dragging, the full image on release. No repaint here: the picture's arrival is what repaints
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, e: &MouseUpEvent, _, cx| this.mouse_up(e, cx)))
            .on_mouse_up(crate::mousecfg::BUTTONS[1], cx.listener(|this, e: &MouseUpEvent, _, cx| this.mouse_up(e, cx)))
            .on_mouse_up(crate::mousecfg::BUTTONS[2], cx.listener(|this, e: &MouseUpEvent, _, cx| this.mouse_up(e, cx)))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, cx| { this.press = None; if this.drag.take().is_some() { this.settled(cx); cx.notify(); } }))
            .on_mouse_up_out(crate::mousecfg::BUTTONS[1], cx.listener(|this, _: &MouseUpEvent, _, cx| { this.press = None; if this.drag.take().is_some() { this.settled(cx); cx.notify(); } }))
            .on_mouse_up_out(crate::mousecfg::BUTTONS[2], cx.listener(|this, _: &MouseUpEvent, _, cx| { this.press = None; if this.drag.take().is_some() { this.settled(cx); cx.notify(); } }))
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                // what a wheel or two-finger scroll means is the table's (mouse.rs)
                let (lines, x, y) = crate::mousecfg::delta(&e.delta);
                let [pw, ph, _, ox, oy] = *this.size.lock().unwrap();
                match crate::mousecfg::solid().scroll(crate::mousecfg::mods(&e.modifiers), lines, x, y, crate::mousecfg::invert()) {
                    crate::mouse::Scroll::Zoom { lines, y, ctrl } => {
                        let k = if lines { wheel_zoom(Some(y), 0.0, ctrl) } else { wheel_zoom(None, y, ctrl) };
                        // the pointer in panel coordinates, -1..1 with y up
                        let (nx, ny) = if pw > 0.0 && ph > 0.0 { ((f32::from(e.position.x) - ox) / pw * 2.0 - 1.0, 1.0 - (f32::from(e.position.y) - oy) / ph * 2.0) } else { (0.0, 0.0) };
                        this.core.touched = true;
                        if let Some(c) = &mut this.core.cam { c.zoom_by(k, nx.clamp(-1.0, 1.0), ny.clamp(-1.0, 1.0), pw / ph.max(1.0)); }
                    }
                    crate::mouse::Scroll::Pan { dx, dy } => { this.core.touched = true; if let Some(c) = &mut this.core.cam { c.pan(dx, dy, ph); } }
                    crate::mouse::Scroll::None => return,
                }
                cx.stop_propagation(); this.moved(cx); // drafts while the wheel turns, the full picture when it rests
            }))
            // a trackpad pinch (macOS sends it as a magnify gesture, not a wheel) zooms at the pointer; two-finger scrolling stays the wheel's
            .on_pinch(cx.listener(|this, e: &PinchEvent, _, cx| {
                let [pw, ph, _, ox, oy] = *this.size.lock().unwrap();
                let (nx, ny) = if pw > 0.0 && ph > 0.0 { ((f32::from(e.position.x) - ox) / pw * 2.0 - 1.0, 1.0 - (f32::from(e.position.y) - oy) / ph * 2.0) } else { (0.0, 0.0) };
                let f = crate::mousecfg::solid().pinch_factor(pinch_zoom(e.delta), crate::mousecfg::invert()); if f == 1.0 { return }
                this.core.touched = true;
                if let Some(c) = &mut this.core.cam { c.zoom_by(f, nx.clamp(-1.0, 1.0), ny.clamp(-1.0, 1.0), pw / ph.max(1.0)); }
                cx.stop_propagation(); this.moved(cx);
            }))
    }
}

#[cfg(test)]
mod tests {
    #[test] fn solid_tools_match_the_camera_actions_and_snap() {
        let tools = super::solid_tools();
        let views = ["fit","iso","top","bottom","front","right","back","left","snap"];
        assert_eq!(tools.len(), views.len());
        for (tool, view) in tools.iter().zip(views) {
            assert_eq!(tool.id, format!("solid-{view}"));
            let key = if view == "fit" { "viewer.fit".into() } else { format!("viewer.c.view.{view}") };
            assert_eq!(tool.label, super::t(&key));
            assert!(tool.enabled && !tool.active);
            assert_eq!(tool.action, if view == "snap" { super::SolidTool::Snap } else { super::SolidTool::View(view) });
        }
    }

    /// The page's note, stage and progress read in Japanese, the hub's reason and the page's own tail each through the catalog; English stays as it was.
    #[test] fn the_page_says_its_note_stage_and_progress_in_the_readers_language() {
        let kept = "Invalid part placement — previous saved revision shown";
        crate::i18n::with_lang("ja", || {
            assert_eq!(super::note_words(kept), "部品の配置が不正です — 前回保存されたリビジョンを表示しています");
            assert_eq!(super::note_words("Saved /home/a/Pictures/Backplane/x.png"), "/home/a/Pictures/Backplane/x.pngを保存しました");
            assert_eq!(super::note_words("No 3D model yet"), "3Dモデルはまだありません");
            assert_eq!(super::stage_line("Building the view…", 4, 11_400), "ビューを構築中… 4秒 （約11秒）");
            assert_eq!(crate::i18n::trs(&super::progress(1, 3)), "部品 1/3");
        });
        assert_eq!(super::note_words(kept), kept); assert_eq!(super::stage_line("Building the view…", 4, 11_400), "Building the view… 4 s (about 11 s)");
    }
    #[test] fn a_note_for_another_source_drops_the_old_model_and_the_same_sources_keeps_it() {
        assert!(super::note_keeps_model("2|/a|", "2|/a|"), "the same model: its previous revision stays with the note");
        assert!(!super::note_keeps_model("2|/a|", "2|/b|"), "thread B's model could not be made: A's must not show under B's key");
        assert!(!super::note_keeps_model("", "2|/b|"));
    }
    // not `use super::*`: that glob brings gpui's own `test` attribute macro into scope and #[test] then expands into itself
    use super::{decode_mesh, parse_frame, pinch_zoom, render, wheel_zoom, Camera, Drawer, Layers, Mesh, SolidFrame, add, dot, mul, refit, sub, has_refs, lit_color, lit_layers, pick, same_part};
    use std::sync::Arc;
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
    /// The BGR of the default (dark) backdrop on output row `y` of `h`, as the renderers mix it: tests compare within a level (the CPU averages samples).
    fn bg_at(y: usize, h: usize) -> [u8; 3] { let c = super::super::raster::Backdrop::DARK.at((y as f32 + 0.5) / h as f32); [c[2], c[1], c[0]] }
    fn is_bg(p: &[u8], y: usize, h: usize) -> bool { let b = bg_at(y, h); (0..3).all(|k| (p[k] as i32 - b[k] as i32).abs() <= 1) }
    #[test] fn renders_the_model_in_the_middle_and_background_at_the_corners() {
        let m = mesh(); let (w, h) = (96, 64); let px = render(&m, &Camera::fitted(&m), w, h, 2, ALIVE).unwrap();
        let at = |x: usize, y: usize| { let i = (y * w + x) * 4; [px[i], px[i + 1], px[i + 2]] };
        assert!(is_bg(&at(1, 1), 1, h), "{:?}", at(1, 1)); assert!(is_bg(&at(w - 2, h - 2), h - 2, h)); assert!(!is_bg(&at(w / 2, h / 2), h / 2, h)); assert!(at(1, 1)[0] > at(1, h - 2)[0], "a gradient, lighter at the top");
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
    /// Where a model point shows on the screen, relative to the centre: (right, up), and how near the eye it is.
    fn screen(c: &Camera, p: [f32; 3]) -> (f32, f32, f32) { let d = super::sub(p, c.target); (super::dot(d, c.right), super::dot(d, c.up), -super::dot(d, c.fwd)) }
    #[test] fn a_drag_turns_the_model_the_way_the_pointer_goes_as_the_web_did() {
        let m = mesh(); let c0 = Camera::fitted(&m); let r = 10.0;
        // the point of the model facing the viewer, and its top
        let front = super::sub(c0.target, mul(c0.fwd, r)); let top = add(c0.target, mul(c0.up, r));
        let mut c = c0; c.orbit(40.0, 0.0);
        let (x, y, _) = screen(&c, front); assert!(x > 0.5 && y.abs() < 1e-3, "a drag to the right takes the front to the right: {x} {y}");
        assert!(super::dot(sub(c.eye(), c0.eye()), c0.right) < 0.0, "the camera goes round the other way, to the left");
        let mut c = c0; c.orbit(-40.0, 0.0); assert!(screen(&c, front).0 < -0.5, "left goes left");
        let mut c = c0; c.orbit(0.0, 40.0);
        let (x, y, _) = screen(&c, front); assert!(y < -0.5 && x.abs() < 1e-3, "a drag down takes the front down: {x} {y}");
        assert!(screen(&c, top).2 > screen(&c0, top).2 + 0.5, "and tips the top toward the viewer");
        let mut c = c0; c.orbit(0.0, -40.0); assert!(screen(&c, front).1 > 0.5, "up goes up");
        // the web's Orbit.spin, step for step: turn(normalize(u·mx + r·my), -|m|·k)
        let mut c = c0; c.orbit(30.0, -20.0);
        let (mx, my) = (30.0f32, -20.0f32); let l = (mx * mx + my * my).sqrt(); let k = super::norm(add(mul(c0.up, mx / l), mul(c0.right, my / l)));
        let want = super::rotate(c0.fwd, k, -l * 0.008);
        for a in 0..3 { assert!((c.fwd[a] - want[a]).abs() < 1e-4, "{:?} vs {:?}", c.fwd, want); }
        let mut c = c0; c.orbit(0.0, 0.0); assert_eq!(c, c0, "no movement, no turn");
    }
    #[test] fn an_alt_drag_to_the_right_turns_the_model_clockwise_on_screen() {
        let m = mesh(); let c0 = Camera::fitted(&m); let top = add(c0.target, mul(c0.up, 10.0));
        let mut c = c0; c.roll(40.0); assert!(screen(&c, top).0 > 0.5, "the top goes right: clockwise");
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
    #[test] fn a_wheel_notch_zooms_as_0_14_did_and_a_touchpad_smoothly() {
        let notch = wheel_zoom(Some(3.0), 0.0, false); assert!((notch - 1.1618).abs() < 1e-3, "{notch}");
        assert!((wheel_zoom(Some(-3.0), 0.0, false) * notch - 1.0).abs() < 1e-5, "out undoes in");
        let pad = wheel_zoom(None, 10.0, false); assert!(pad > 1.0 && pad < 1.05, "{pad}");
        assert!(wheel_zoom(None, 10.0, true) > pad, "a pinch zooms faster");
        assert!(wheel_zoom(Some(1e9), 0.0, false).is_finite());
    }
    /// A pinch's magnification: spreading the fingers zooms in, pinching them zooms out, nothing is no zoom, and a wild value is held to one step.
    #[test] fn a_pinch_zooms_by_its_magnification() {
        assert!((pinch_zoom(0.1) - 1.1).abs() < 1e-6);
        assert!(pinch_zoom(-0.1) < 1.0);
        assert_eq!(pinch_zoom(0.0), 1.0);
        assert_eq!(pinch_zoom(50.0), 2.0); assert_eq!(pinch_zoom(-5.0), 0.5);
        assert_eq!(pinch_zoom(f32::NAN), 1.0); assert_eq!(pinch_zoom(f32::INFINITY), 1.0);
    }
    #[test] fn a_turn_keeps_the_model_where_it_was_put_and_pans_follow_the_pointer() {
        let m = mesh(); let mut c = Camera::fitted(&m);
        c.pan(40.0, -25.0, 600.0); c.zoom_by(3.0, 0.4, -0.2, 1.5);
        let at = |c: &Camera| { let d = super::sub(c.pivot, c.target); (super::dot(d, c.right), super::dot(d, c.up)) };
        let before = at(&c); c.orbit(80.0, 30.0); c.roll(15.0); let after = at(&c);
        assert!((before.0 - after.0).abs() < 1e-3 && (before.1 - after.1).abs() < 1e-3, "{before:?} {after:?}: the centre stays on the same spot of the screen");
        // a pan of 100 px moves a point at the pivot's depth by exactly 100 px
        let h = 600.0; let px = 1.0 / c.unit(h); let p = c.pivot; let s0 = screen(&c, p); c.pan(100.0, 0.0, h); let s1 = screen(&c, p);
        assert!(((s1.0 - s0.0) * px - 100.0).abs() < 1e-2, "{}", (s1.0 - s0.0) * px);
        let n = c.named("top"); assert_eq!((n.shift, n.target, n.dist), ([0.0, 0.0], c.pivot, c.fit), "a standard view centres the model again");
    }
    #[test] fn a_picture_leaves_the_atlas_only_when_the_gpu_is_done_with_it() {
        assert!(!super::gone_from_gpu(7, 7) && !super::gone_from_gpu(7, 8), "the frame that drew it may still be on the GPU while the next is built");
        assert!(super::gone_from_gpu(7, 9));
    }
    #[test] fn a_slow_picture_in_motion_makes_the_next_smaller_and_a_quick_one_lets_it_grow_back() {
        let mut s = 1.0; for _ in 0..20 { s = super::draft_scale(s, 120.0); } assert_eq!(s, 0.125, "never below an eighth");
        let t = super::draft_scale(0.5, 20.0); assert_eq!(t, 0.5, "within budget: kept");
        let mut s = 0.125; for _ in 0..40 { s = super::draft_scale(s, 5.0); } assert_eq!(s, 1.0, "a GPU's quick pictures: all of it");
    }
    #[test] fn retry_asks_for_the_same_source_again() {
        assert_eq!(super::retry_watch("2|/p|/p/mech/a.step").unwrap(), serde_json::json!({"kind": "3d", "root": "/p", "path": "/p/mech/a.step", "tree": ""}));
        assert_eq!(super::retry_watch("2|/p|").unwrap()["path"], "", "a board's model: the project's board");
        assert!(super::retry_watch("0|/p|").is_none() && super::retry_watch("").is_none(), "not a 3D source");
    }
    #[test] fn a_stage_reads_from_its_frame_and_shows_its_time() {
        let mut f = vec![0xa4]; cbor_text(&mut f, "key"); cbor_text(&mut f, "2|/p|"); cbor_text(&mut f, "stage"); cbor_text(&mut f, "Meshing a.step with FreeCAD…");
        cbor_text(&mut f, "ms"); f.extend([0x19, 0x0b, 0xb8]); cbor_text(&mut f, "about"); f.push(0);
        assert_eq!(parse_frame(&f), Some(SolidFrame::Stage("2|/p|".into(), "Meshing a.step with FreeCAD…".into(), 3000, 0)));
        assert_eq!(super::stage_line("Reading the board…", 0, 0), "Reading the board…", "no time under a second");
        assert_eq!(super::stage_line("Meshing a.step with FreeCAD…", 4, 11_400), "Meshing a.step with FreeCAD… 4 s (about 11 s)");
        assert_eq!(super::stage_line("x", 2, 900), "x 2 s", "a quick one is not worth saying");
    }
    #[test] fn a_triangle_behind_the_near_plane_is_clipped_not_dropped() {
        let m = mesh(); let mut c = Camera::fitted(&m); c.dist = c.fit / 60.0 * 2.0; // inside the cube: faces cross the near plane
        let px = render(&m, &c, 48, 48, 2, ALIVE).unwrap();
        assert!(px.chunks_exact(4).enumerate().any(|(i, p)| !is_bg(p, i / 48, 48)), "the near faces still cover the view");
    }
    #[test] fn a_superseded_render_is_abandoned() { let m = mesh(); assert!(render(&m, &Camera::fitted(&m), 32, 32, 1, &|| false).is_none()); }
    #[test] fn frame_parser_reads_model_note_and_ignores_other_frames() {
        let bytes = encode(&cube());
        let text = |f: &mut Vec<u8>, s: &str| { f.push(0x60 | s.len() as u8); f.extend(s.as_bytes()) };
        let mut f = vec![0xa3]; text(&mut f, "key"); text(&mut f, "2|board"); text(&mut f, "n"); f.push(12); text(&mut f, "mesh"); f.push(0x59); f.extend((bytes.len() as u16).to_be_bytes()); f.extend(&bytes);
        match parse_frame(&f) { Some(SolidFrame::Model(k, m)) => { assert_eq!(k, "2|board"); assert_eq!(m.tris.len(), 12) } other => panic!("{other:?}") }
        let mut n = vec![0xa2]; text(&mut n, "key"); text(&mut n, "3|board"); text(&mut n, "none"); text(&mut n, "no model for this board");
        assert_eq!(parse_frame(&n), Some(SolidFrame::Note("3|board".into(), "no model for this board".into(), None)));
        let mut nr = vec![0xa3]; text(&mut nr, "key"); text(&mut nr, "3|board"); text(&mut nr, "none"); text(&mut nr, "boom"); text(&mut nr, "retryable"); nr.push(0xf5);
        assert_eq!(parse_frame(&nr), Some(SolidFrame::Note("3|board".into(), "boom".into(), Some(true))));
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
            for y in 0..h { for x in 0..w {
                let (dx, dy) = (x as f32 + 0.5 - w as f32 / 2.0, y as f32 + 0.5 - h as f32 / 2.0);
                let i = (y * w + x) * 4; let on = !is_bg(&px[i..i + 3], y, h);
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
    fn cbor_text(f: &mut Vec<u8>, s: &str) { if s.len() < 24 { f.push(0x60 | s.len() as u8) } else { f.extend([0x78, s.len() as u8]) } f.extend(s.as_bytes()) }
    fn cbor_bytes(f: &mut Vec<u8>, b: &[u8]) { f.push(0x5a); f.extend((b.len() as u32).to_be_bytes()); f.extend(b) }
    fn part_frame(key: &str, tris: &[([i32; 9], u32)], at: &[[f32; 12]], done: u8) -> Vec<u8> {
        let mut f = vec![0xa6]; cbor_text(&mut f, "key"); cbor_text(&mut f, key); cbor_text(&mut f, "part"); cbor_text(&mut f, "0123456789abcdef");
        cbor_text(&mut f, "n"); f.push(tris.len() as u8); cbor_text(&mut f, "mesh"); cbor_bytes(&mut f, &encode(tris));
        cbor_text(&mut f, "at"); cbor_bytes(&mut f, &at.iter().flatten().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>());
        cbor_text(&mut f, "done"); f.push(done); f
    }
    fn parts_frame(key: &str, of: u8) -> Vec<u8> { let mut f = vec![0xa2]; cbor_text(&mut f, "key"); cbor_text(&mut f, key); cbor_text(&mut f, "parts"); f.push(0xa2); cbor_text(&mut f, "of"); f.push(of); cbor_text(&mut f, "rev"); cbor_text(&mut f, "1.2.3"); f }
    const ID: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    fn at(m: &Mesh, d: [f32; 3]) -> Arc<Mesh> { let mut m = m.clone(); for t in &mut m.tris { for k in 0..9 { t[k] += d[k % 3] } } m.center = add(m.center, d); Arc::new(m) }
    #[test] fn parts_with_no_board_are_bounded_together_and_the_camera_follows_them_as_they_grow() {
        let m = mesh(); let (a, b) = (at(&m, [0.0; 3]), at(&m, [10.0, 0.0, 0.0]));
        let (c1, r1) = Layers::of(None, vec![a.clone()]).map(|l| (l.center, l.radius)).unwrap();
        assert_eq!((c1, r1), (m.center, m.radius), "one part: its own");
        let l = Layers::of(None, vec![a.clone(), b.clone()]).unwrap();
        for p in [&a, &b] { assert!(dot(sub(p.center, l.center), sub(p.center, l.center)).sqrt() + p.radius <= l.radius + 1e-4, "every part inside") }
        assert!((l.center[0] - (m.center[0] + 5.0)).abs() < 1e-4, "centred between them");
        assert!(refit((c1, r1), (l.center, l.radius)), "a part far off: the camera fits again");
        assert!(!refit((l.center, l.radius), (l.center, l.radius * 1.01)), "a hundredth: it stays");
        let board = Layers::of(Some(a.clone()), vec![b.clone()]).unwrap();
        assert_eq!((board.center, board.radius), (a.center, a.radius), "a board's parts never move its camera");
    }
    /// The client's share of opening a model, on real frames: decode (with the normals), the GPU's vertices, and one CPU picture. Opt in with
    /// a folder of model frames as test/files/step_timing.mjs saves them: BP_FRAMES=<dir> cargo test real_frames -- --ignored --nocapture
    #[test] #[ignore] fn real_frames_decode_and_draw_in_time() {
        let Ok(dir) = std::env::var("BP_FRAMES") else { eprintln!("skipped: no BP_FRAMES"); return };
        let mut fs: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect(); fs.sort();
        for f in fs {
            let b = std::fs::read(&f).unwrap();
            let t = std::time::Instant::now();
            let Some(SolidFrame::Model(_, m)) = parse_frame(&b) else { eprintln!("{}: not a model", f.display()); continue };
            let decode = t.elapsed().as_secs_f64() * 1000.0;
            #[cfg(target_os = "linux")]
            let (v, verts) = { let t = std::time::Instant::now(); let v = super::super::solid_gpu::bench_vertices(&m); (v, t.elapsed().as_secs_f64() * 1000.0) };
            #[cfg(not(target_os = "linux"))]
            let (v, verts) = (0usize, 0.0f64);
            let cam = Camera::fitted(&m); let m = Arc::new(m); let l = Layers::of(Some(m.clone()), vec![]).unwrap();
            let mut d = Drawer::default(); let t = std::time::Instant::now();
            let px = d.cpu_mesh(&l).and_then(|mm| render(&mm, &cam, 1280, 800, 1, &|| true)); let cpu = t.elapsed().as_secs_f64() * 1000.0;
            eprintln!("BENCH {} tris {} bytes {} decode {decode:.1} ms gpu-vertices {verts:.1} ms ({v}) cpu-picture-1280x800 {cpu:.1} ms {}", f.file_name().unwrap().to_string_lossy(), m.tris.len(), b.len(), if px.is_some() { "" } else { "(none)" });
        }
    }
    /// A close-up of a model frame as the page draws it, written as PNG: the CPU's picture, and the GPU's where there is one. Opt in:
    /// BP_CLOSE="FRAME|OUT_STEM|x,y,z|dist|view" cargo test close_up -- --ignored --nocapture (x y z: the point looked at, in the model's
    /// millimetres; view: iso, top, front...; several close-ups separated by `;`). bp-plot --frame MODEL.glb FRAME step makes a STEP's frame.
    #[test] #[ignore] fn close_up() {
        let Ok(spec) = std::env::var("BP_CLOSE") else { eprintln!("skipped: no BP_CLOSE"); return };
        for one in spec.split(';').filter(|s| !s.trim().is_empty()) {
            let f: Vec<&str> = one.split('|').collect();
            let b = std::fs::read(f[0]).unwrap();
            let Some(SolidFrame::Model(_, m)) = parse_frame(&b) else { panic!("{}: not a model", f[0]) };
            let t: Vec<f32> = f[2].split(',').map(|x| x.parse().unwrap()).collect();
            let mut c = Camera::fitted_for(&m, 1.5).named(f[4]);
            c.pivot = [t[0], t[1], t[2]]; c.shift = [0.0, 0.0]; c.target = c.pivot; c.dist = f[3].parse().unwrap();
            let (w, h) = (900usize, 600usize);
            let save = |px: Vec<u8>, tag: &str| { let mut px = px; for p in px.chunks_exact_mut(4) { p.swap(0, 2); } image::RgbaImage::from_raw(w as u32, h as u32, px).unwrap().save(format!("{}.{tag}.png", f[1])).unwrap(); };
            let m = Arc::new(m);
            save(render(&m, &c, w, h, 3, &|| true).unwrap(), "cpu");
            #[cfg(target_os = "linux")] if let Some(g) = super::super::solid_gpu::shared().lock().unwrap().as_mut() { if let Some(px) = g.render_layers(&[(&m, None)], &[], &c, w, h, &super::super::raster::Backdrop::DARK) { save(px, "gpu") } }
            eprintln!("CLOSE {} tris {}", f[1], m.tris.len());
        }
    }
    #[test] fn a_frames_faces_are_runs_of_triangles_and_wrong_ones_are_none() {
        assert_eq!(super::face_ids(&[2, 1, 3], 6), Some(vec![0, 0, 1, 2, 2, 2]));
        assert_eq!(super::face_ids(&[2, 1], 6), None, "fewer than the triangles: creased by angle instead");
        assert_eq!(super::face_ids(&[0, 6], 6), None, "an empty run");
        assert_eq!(super::face_ids(&[], 6), None);
    }
    #[test] fn within_a_face_normals_blend_and_across_faces_they_do_not_whatever_the_angle() {
        // two triangles folded 20 degrees along a shared edge: one face (a curve's facets) or two (a shallow chamfer)
        let a = 20f32.to_radians();
        let t = vec![[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 0.0, -a.cos(), a.sin(), 1.0, 0.0, 0.0]];
        let k: Vec<[i32; 3]> = t.iter().flat_map(|x| (0..3).map(|c| [(x[3 * c] * 1000.0) as i32, (x[3 * c + 1] * 1000.0) as i32, (x[3 * c + 2] * 1000.0) as i32]).collect::<Vec<_>>()).collect();
        let one = super::super::raster::smooth_normals_in(&t, &k, Some(&[0, 0]));
        let two = super::super::raster::smooth_normals_in(&t, &k, Some(&[0, 1]));
        // the corner at the origin is corner 0 of both
        let close = |p: &[f32], q: &[f32]| (0..3).all(|i| (p[i] - q[i]).abs() < 1e-5);
        assert!(close(&one[0][0..3], &one[1][0..3]) && one[0][2] < 0.999, "one face: the shared corner has one normal, between the two");
        assert!(close(&two[0][0..3], &[0.0, 0.0, 1.0]) && !close(&two[1][0..3], &[0.0, 0.0, 1.0]), "two faces: each keeps its own, the edge crisp");
    }
    #[test] fn a_step_zoomed_in_on_asks_once_for_its_region() {
        assert!(super::refinable("2|/p|/p/mech/lid.step") && super::refinable("2|/p|/p/mech/LID.STP") && !super::refinable("2|/p|") && !super::refinable("2|/p|/p/m.glb"));
        let m = mesh(); let mut c = Camera::fitted(&m);
        assert_eq!(super::refine_focus(&c, m.radius, 1.5), None, "the whole model in view: the first mesh is fine");
        c.dist = m.radius * 0.1;
        let f = super::refine_focus(&c, m.radius, 1.5).expect("zoomed in");
        assert_eq!(f.0, c.target);
        assert!(super::covers(f, (f.0, f.1 * 0.5)) && !super::covers(f, (add(f.0, [f.1 * 2.0, 0.0, 0.0]), f.1)), "a closer look inside it asks nothing; one beside it asks");
    }
    #[test] fn the_cpu_mesh_grows_by_the_parts_that_came_and_is_the_same_as_made_whole() {
        let m = mesh(); let (a, b, c) = (at(&m, [0.0; 3]), at(&m, [3.0, 0.0, 0.0]), at(&m, [0.0, 7.0, 0.0]));
        let mut d = Drawer::default();
        let made = d.cpu_mesh(&Layers::of(None, vec![a.clone(), b.clone()]).unwrap()).unwrap();
        let grown = Layers::of(None, vec![a.clone(), b.clone(), c.clone()]).unwrap();
        let now = d.cpu_mesh(&grown).unwrap();
        assert_eq!(*now, grown.combined().unwrap(), "appended: what combining every layer again makes");
        assert_eq!(now.tris.len(), made.tris.len() + c.tris.len());
        assert!(Arc::ptr_eq(&now, &d.cpu_mesh(&grown).unwrap()), "the same layers: the same mesh");
        let other = Layers::of(None, vec![b.clone(), a.clone()]).unwrap();
        assert_eq!(*d.cpu_mesh(&other).unwrap(), other.combined().unwrap(), "other layers: made again");
    }
    #[test] fn a_part_frame_places_its_model_at_every_instance_and_the_parts_frame_says_how_many() {
        assert_eq!(parse_frame(&parts_frame("2|b|", 7)), Some(SolidFrame::Parts("2|b|".into(), 7)));
        // the second instance: turned a quarter about z (x' = -y, y' = x) and moved 50 mm in x (5000 wire units)
        let turn = [0.0, -1.0, 0.0, 5000.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let tri = [([1000, 0, 0, 0, 1000, 0, 0, 0, 500], 0x00ff00u32)];
        match parse_frame(&part_frame("2|b|", &tri, &[ID, turn], 2)) {
            Some(SolidFrame::Part(k, m, done)) => {
                assert_eq!((k.as_str(), m.tris.len(), done), ("2|b|", 2, 2));
                assert_eq!(m.tris[0], [10.0, -0.0, 0.0, 0.0, -10.0, 0.0, 0.0, -0.0, 5.0], "mm, y turned up as the board's");
                assert_eq!(m.tris[1][0..3], [50.0, -10.0, 0.0]); assert_eq!(m.tris[1][3..6], [40.0, -0.0, 0.0]);
                assert_eq!(m.colors, vec![0x00ff00, 0x00ff00]);
            }
            other => panic!("{other:?}"),
        }
        assert!(super::decode_part(1, &encode(&tri), &[0u8; 47]).is_err(), "half an instance is refused");
        assert!(super::decode_part(1, &encode(&tri), &f32::NAN.to_le_bytes().repeat(12)).is_err());
    }
    #[test] fn a_click_picks_the_part_under_it_by_the_frames_references_and_the_probed_part_is_lit() {
        // one model (a 10 mm cube) at two instances 30 mm apart, named by the frame's `refs`
        let mut f = part_frame("2|b|", &cube(), &[ID, [1.0, 0.0, 0.0, 3000.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]], 2);
        f[0] = 0xa7; cbor_text(&mut f, "refs"); f.push(0x82); cbor_text(&mut f, "R1"); cbor_text(&mut f, "U2");
        let Some(SolidFrame::Part(_, part, _)) = parse_frame(&f) else { panic!() };
        assert_eq!(part.refs, vec!["R1".to_string(), "U2".to_string()]);
        // (a list that is not one an instance names nothing)
        let mut g = part_frame("2|b|", &cube(), &[ID], 1); g[0] = 0xa7; cbor_text(&mut g, "refs"); g.push(0x82); cbor_text(&mut g, "R1"); cbor_text(&mut g, "U2");
        let Some(SolidFrame::Part(_, lone, _)) = parse_frame(&g) else { panic!() }; assert!(lone.refs.is_empty());
        // the board: a slab under the first instance and one off to the side
        let m = mesh(); let board = { let mut b = (*at(&m, [0.0, 0.0, -20.0])).clone(); let side = at(&m, [60.0, 0.0, -20.0]); b.tris.extend_from_slice(&side.tris); b.normals.extend_from_slice(&side.normals); b.colors.extend_from_slice(&side.colors); b.center = [35.0, -5.0, -15.0]; b.radius = 60.0; Arc::new(b) };
        let l = Layers::of(Some(board.clone()), vec![Arc::new(part)]).unwrap();
        assert!(has_refs(&l) && !has_refs(&Layers::of(None, vec![Arc::new(lone)]).unwrap()), "only a model that names its parts is picked on");
        let down = [0.0, 0.0, -1.0];
        assert_eq!(pick(&l, [5.0, -5.0, 100.0], down), Some("R1".into()), "the part, though the board is under it");
        assert_eq!(pick(&l, [35.0, -5.0, 100.0], down), Some("U2".into()), "the second instance of the same model is its own part");
        assert_eq!(pick(&l, [65.0, -5.0, 100.0], down), Some(String::new()), "the board itself: no part");
        assert_eq!(pick(&l, [20.0, -5.0, 100.0], down), None, "nothing at all");
        assert_eq!(pick(&l, [5.0, -5.0, 100.0], [0.0, 0.0, 1.0]), None, "what is behind the eye is not picked");
        // the ray of a pointer: the middle of the panel looks along the camera, a corner off towards it
        let cam = Camera::fit_to(l.center, l.radius, 1.5);
        let (eye, mid) = cam.ray(0.0, 0.0, 1.5); assert_eq!(eye, cam.eye()); assert!(dot(mid, cam.fwd) > 0.9999);
        let (_, corner) = cam.ray(1.0, 1.0, 1.5); assert!(dot(corner, cam.right) > 0.0 && dot(corner, cam.up) > 0.0 && dot(corner, cam.right) > dot(corner, cam.up), "{corner:?}");
        // the probed part lit: its instance's triangles only, the other layers shared
        assert!(same_part("U2", "U2") && same_part("U2A", "U2") && !same_part("U2", "U21") && !same_part("", "") && !same_part("U2", ""));
        let lit = lit_layers(&l, "U2A").unwrap(); let (was, now) = (&l.parts[0], &lit.parts[0]); let per = was.tris.len() / 2;
        assert!(Arc::ptr_eq(lit.board.as_ref().unwrap(), &board) && !Arc::ptr_eq(was, now));
        assert_eq!(now.colors[..per], was.colors[..per], "R1 as it was");
        assert!((per..2 * per).all(|i| now.colors[i] == lit_color(was.colors[i]) && now.colors[i] != was.colors[i]), "U2 lit");
        assert_eq!((now.tris.len(), &now.refs), (was.tris.len(), &was.refs));
        assert!(lit_layers(&l, "R9").is_none() && lit_layers(&l, "").is_none(), "a part the model has not: the model as it is");
        assert_eq!(lit_color(0x80_102030) >> 24, 0x80, "a translucent triangle stays translucent");
    }
    #[test] fn the_worker_scene_starts_parts_again_and_combines_them_after_the_board() {
        let mut sc = super::Scene::default(); let (mut b, mut r) = (false, false);
        assert_eq!(sc.take(SolidFrame::Model("2|b|".into(), mesh()), &mut b, &mut r), Ok(true)); assert!(b && r);
        let part = |done| match parse_frame(&part_frame("2|b|", &cube()[..2], &[ID], done)) { Some(f) => f, None => panic!() };
        assert_eq!(sc.take(SolidFrame::Parts("2|b|".into(), 2), &mut b, &mut r), Ok(true));
        assert_eq!(sc.take(part(1), &mut b, &mut r), Ok(true)); assert_eq!(sc.take(part(2), &mut b, &mut r), Ok(true));
        assert_eq!(sc.take(SolidFrame::Parts("2|other|".into(), 9), &mut b, &mut r), Ok(false), "another model's parts do not go under this board");
        let all = super::combine(sc.board.as_deref(), &sc.parts).unwrap();
        assert_eq!(all.tris.len(), 12 + 2 + 2); assert_eq!(&all.groups[10..], &[0, 0, 1, 1, 2, 2]);
        assert_eq!((all.center, all.radius), (mesh().center, mesh().radius), "the camera stays on the board");
        assert_eq!((sc.done, sc.of), (2, 2));
        assert_eq!(sc.take(SolidFrame::Parts("2|b|".into(), 3), &mut b, &mut r), Ok(true)); assert!(sc.parts.is_empty() && sc.of == 3);
        assert_eq!(super::progress(1, 3), "parts 1/3"); assert_eq!(super::progress(3, 3), ""); assert_eq!(super::progress(0, 0), "");
        assert_eq!((super::fade_at(0.0), super::fade_at(150.0), super::fade_at(900.0)), (0.0, 0.5, 1.0));
    }
    #[test] fn a_part_fades_in_from_nothing_to_itself_over_the_board() {
        let board = mesh();
        // a part: a red square floating in front of the cube's top
        let red = [([0, 0, 1500, 1000, 0, 1500, 1000, 1000, 1500], 0xff0000u32), ([0, 0, 1500, 1000, 1000, 1500, 0, 1000, 1500], 0xff0000)];
        let Some(SolidFrame::Part(_, p, _)) = parse_frame(&part_frame("2|b|", &red, &[ID], 1)) else { panic!() };
        let all = super::combine(Some(&board), &[std::sync::Arc::new(p)]).unwrap();
        let c = Camera::fitted(&board).named("top"); let (w, h) = (48, 48);
        let plain = render(&board, &c, w, h, 1, ALIVE).unwrap();
        let at = |f: f32| super::super::raster::render_faded(&all, &[1.0, f], &c, w, h, 1, &super::super::raster::Backdrop::DARK, ALIVE).unwrap();
        assert_eq!(at(0.0), plain, "not there yet: the board alone");
        let (half, full) = (at(0.5), at(1.0));
        let mid = (h / 2 * w + w / 2) * 4; let red_of = |p: &[u8]| p[mid + 2] as i32 - p[mid + 1] as i32; // BGRA
        assert!(red_of(&plain) < red_of(&half) && red_of(&half) < red_of(&full), "{} {} {}", red_of(&plain), red_of(&half), red_of(&full));
        assert_eq!(full, super::super::raster::render_faded(&all, &[], &c, w, h, 1, &super::super::raster::Backdrop::DARK, ALIVE).unwrap(), "faded in is the same as never faded");
        #[cfg(target_os = "linux")] {
            unsafe { std::env::set_var("BACKPLANE_SOLID_RENDERER", "gpu"); }
            if let Some(mut g) = super::super::solid_gpu::Gpu::new() {
                let a = std::sync::Arc::new(all);
                let mut gp = |f: f32| g.render_faded(&a, &[1.0, f], &c, w, h).unwrap();
                let (z, hf, fl) = (gp(0.0), gp(0.5), gp(1.0));
                assert!(red_of(&z) < red_of(&hf) && red_of(&hf) < red_of(&fl), "gpu {} {} {}", red_of(&z), red_of(&hf), red_of(&fl));
                // the same model in layers (a buffer for the board, one for the part): the same pictures as the one combined mesh
                let b = std::sync::Arc::new(board.clone());
                let Some(SolidFrame::Part(_, part, _)) = parse_frame(&part_frame("2|b|", &red, &[ID], 1)) else { panic!() };
                let l = Layers::of(Some(b), vec![std::sync::Arc::new(part)]).unwrap();
                let mut gl = |f: f32| g.render_layers(&l.list(), &[1.0, f], &c, w, h, &super::super::raster::Backdrop::DARK).unwrap();
                assert_eq!((gl(0.0), gl(0.5), gl(1.0)), (z, hf, fl), "layers draw as the combined mesh");
            } else { eprintln!("no GPU here: the GPU half skipped") }
        }
    }
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
    #[cfg(target_os = "linux")] #[test] #[ignore] fn bench_parts_stream() {
        // what each arriving part frame costs the worker: the combined model rebuilt, and its GPU vertices rebuilt
        for n in [283_000, 590_000] { let (f, _) = super::super::demo::sphere_frame(n); let t = std::time::Instant::now(); let m = parse_frame(&f); eprintln!("decode {n} triangles ({} KB): {:.0} ms", f.len() >> 10, t.elapsed().as_secs_f64() * 1000.0); drop(m); }
        let (f, _) = super::super::demo::sphere_frame(283_000);
        let Some(SolidFrame::Model(_, board)) = parse_frame(&f) else { panic!() };
        let (pf, _) = super::super::demo::sphere_frame(12_000);
        let Some(SolidFrame::Model(_, part)) = parse_frame(&pf) else { panic!() };
        let mut parts = vec![]; let (mut comb, mut vert) = (0.0, 0.0);
        for _ in 0..40 {
            parts.push(std::sync::Arc::new(part.clone()));
            let t = std::time::Instant::now(); let all = super::combine(Some(&board), &parts).unwrap(); comb += t.elapsed().as_secs_f64() * 1000.0;
            let t = std::time::Instant::now(); let v = super::super::solid_gpu::bench_vertices(&all); vert += t.elapsed().as_secs_f64() * 1000.0; assert!(v > 0);
        }
        eprintln!("40 part frames over a 283k board (+12k each, 763k at the end): combine {comb:.0} ms, GPU vertices {vert:.0} ms, {:.0} ms a part", (comb + vert) / 40.0);
        // in layers (the shipping path on a GPU): a part frame is one more layer, only its own vertices are made
        let (b, mut ps, mut lay) = (std::sync::Arc::new(board.clone()), vec![], 0.0);
        let t = std::time::Instant::now(); let _ = super::super::solid_gpu::bench_vertices(&b); let first = t.elapsed().as_secs_f64() * 1000.0;
        for _ in 0..40 {
            let p = std::sync::Arc::new(part.clone()); ps.push(p.clone());
            let t = std::time::Instant::now(); let l = Layers::of(Some(b.clone()), ps.clone()).unwrap(); let v = super::super::solid_gpu::bench_vertices(&p); lay += t.elapsed().as_secs_f64() * 1000.0; assert!(v > 0 && l.tris() > 0);
        }
        eprintln!("in layers: the board's vertices once {first:.0} ms, then {:.2} ms a part", lay / 40.0);
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
            image::RgbaImage::from_raw(w as u32, h as u32, rgba).unwrap().save(crate::tmp::base().join(format!("solid-{name}.png"))).unwrap();
        }
    }
    #[test] fn solid_image_is_accepted_and_kept_only_for_its_own_source_key() {
        let board = "2|/proj|";
        let mech = "2|/proj|/proj/mech/bracket.step";
        // An image rendered for the board must never be accepted once the page switched to Mech, even with a fresh ticket.
        assert!(!super::shows_for_key(mech, board, 10, 5, 6));
        assert!(super::shows_for_key(mech, mech, 10, 5, 6));
        // An older ticket for the same key is still rejected by floor/shown.
        assert!(!super::shows_for_key(mech, mech, 5, 5, 6));
        assert!(!super::shows_for_key("", "", 10, 0, 1));
        // Cached image is kept/drawn only when image_key matches page_key and an image is present.
        assert!(!super::image_matches_key(mech, board, true));
        assert!(!super::image_matches_key(mech, "", true));
        assert!(!super::image_matches_key(mech, mech, false));
        assert!(super::image_matches_key(mech, mech, true));
    }
    fn dummy_layers() -> std::sync::Arc<super::Layers> {
        std::sync::Arc::new(super::Layers::of(Some(std::sync::Arc::new(mesh())), vec![]).unwrap())
    }
    fn dummy_img(w: usize, h: usize) -> std::sync::Arc<gpui::RenderImage> {
        super::to_image(vec![0x7fu8; w * h * 4], w, h).unwrap()
    }
    #[test] fn source_switch_to_meshless_parts_header_clears_old_mesh_and_never_submits_it() {
        let board = "2|/proj|";
        let mech = "2|/proj|/proj/mech/bracket.step";
        let mut core = super::SolidCore::new();
        // 1. Paint source A (board).
        let res = core.apply_scene(board.into(), Some(dummy_layers()), true, true, 0, 0, 0, (600.0, 800.0));
        assert!(res.should_render);
        let (rk, _, _, rev, w, h) = core.prepare_render((600.0, 800.0, 1.0), false, 1.0).unwrap();
        assert_eq!((rk.as_str(), w, h), (board, 600, 800));
        core.latest = 1;
        assert!(core.apply_image(1, board.into(), rev, dummy_img(w, h), w, h));
        assert!(core.image.is_some() && core.image_key == board);
        // 2. Switch to source B via a mesh-less Parts header before B's first Part arrives.
        let res_b = core.apply_scene(mech.into(), None, false, true, 0, 0, 4, (600.0, 800.0));
        assert!(!res_b.should_render);
        assert_eq!(core.key, mech);
        assert!(core.mesh.is_none(), "old mesh must be cleared on source switch");
        assert!(core.cam.is_none(), "old camera must be cleared on source switch");
        assert!(core.image.is_none() && core.image_key.is_empty(), "old image must be cleared");
        // 3. Attempting to submit a render before B's first part arrives must produce nothing (never submit A under B).
        assert!(core.prepare_render((600.0, 800.0, 1.0), false, 1.0).is_none());
        // 4. Late image for A is rejected even with a fresh ticket.
        assert!(!core.apply_image(2, board.into(), rev, dummy_img(w, h), w, h));
        assert!(core.image.is_none());
    }
    #[test] fn kept_cache_restores_image_only_when_viewport_size_matches() {
        let board = "2|/proj|";
        let mech = "2|/proj|/proj/mech/bracket.step";
        let mut core = super::SolidCore::new();
        // Paint A in a Split-sized viewport (300x400).
        core.apply_scene(board.into(), Some(dummy_layers()), true, true, 0, 0, 0, (300.0, 400.0));
        let (_, _, _, rev_a, w, h) = core.prepare_render((300.0, 400.0, 1.0), false, 1.0).unwrap();
        core.latest = 1;
        assert!(core.apply_image(1, board.into(), rev_a, dummy_img(w, h), w, h));
        // Switch to B (full-height 600x800): A is saved in prev with size (300, 400).
        core.apply_scene(mech.into(), Some(dummy_layers()), true, true, 0, 0, 0, (600.0, 800.0));
        // Restore A into a full-height 600x800 viewport: mesh + cam restore, but mismatched Split image must NOT restore.
        assert!(core.restore_kept(board, (600.0, 800.0, 1.0), 1.0));
        assert_eq!(core.key, board);
        assert!(core.mesh.is_some() && core.cam.is_some());
        assert!(core.image.is_none() && core.image_key.is_empty(), "mismatched viewport size must not restore stretched image");
        // Now paint A at 600x800, switch to B, and return to A at the SAME 600x800 viewport: cached image restores at once.
        let (_, _, _, rev_a2, w2, h2) = core.prepare_render((600.0, 800.0, 1.0), false, 1.0).unwrap();
        core.latest = 2;
        assert!(core.apply_image(2, board.into(), rev_a2, dummy_img(w2, h2), w2, h2));
        core.apply_scene(mech.into(), Some(dummy_layers()), true, true, 0, 0, 0, (600.0, 800.0));
        assert!(core.restore_kept(board, (600.0, 800.0, 1.0), 1.0));
        assert!(core.image.is_some() && core.image_key == board && core.image_size == (600, 800));
    }
    #[test] fn orbit_keeps_image_on_same_key_but_cache_rejects_stale_camera_image() {
        let board = "2|/proj|";
        let mech = "2|/proj|/proj/mech/bracket.step";
        let mut core = super::SolidCore::new();
        // Paint A at 600x800.
        core.apply_scene(board.into(), Some(dummy_layers()), true, true, 0, 0, 0, (600.0, 800.0));
        let (_, _, cam_before, rev1, w, h) = core.prepare_render((600.0, 800.0, 1.0), false, 1.0).unwrap();
        core.latest = 1;
        assert!(core.apply_image(1, board.into(), rev1, dummy_img(w, h), w, h));
        // Orbit on A: previous image stays on screen (no flash while replacement renders), view_rev advances.
        if let Some(c) = &mut core.cam { c.orbit(24.0, 12.0); }
        let cam_after = core.cam.unwrap();
        assert_ne!(cam_before.eye(), cam_after.eye());
        core.bump_view();
        assert_ne!(core.view_rev, rev1);
        assert!(core.image.is_some(), "same-key camera move must keep previous image to avoid flash");
        // Leave to B before the new camera's render finishes, then return to A at the same size.
        core.apply_scene(mech.into(), Some(dummy_layers()), true, true, 0, 0, 0, (600.0, 800.0));
        assert!(core.restore_kept(board, (600.0, 800.0, 1.0), 1.0));
        assert_eq!(core.cam.unwrap().eye(), cam_after.eye(), "orbited camera must be restored");
        assert!(core.image.is_none() && core.image_key.is_empty(), "pre-orbit image must not be restored under the orbited camera");
    }
    #[test] fn note_for_different_source_clears_image_and_metadata() {
        let board = "2|/proj|";
        let mech = "2|/proj|/proj/mech/bracket.step";
        let mut core = super::SolidCore::new();
        core.apply_scene(board.into(), Some(dummy_layers()), true, true, 0, 0, 0, (600.0, 800.0));
        let (_, _, _, rev, w, h) = core.prepare_render((600.0, 800.0, 1.0), false, 1.0).unwrap();
        core.latest = 1;
        assert!(core.apply_image(1, board.into(), rev, dummy_img(w, h), w, h));
        core.apply_note(mech.into());
        assert_eq!(core.key, mech);
        assert!(core.mesh.is_none() && core.cam.is_none());
        assert!(core.image.is_none() && core.image_key.is_empty() && core.image_size == (0, 0) && core.image_view_rev == 0);
    }
    #[test] fn restored_model_without_pixels_accepts_its_render_after_meshless_parts_header() {
        let board = "2|/proj|";
        let mech = "2|/proj|/proj/mech/bracket.step";
        let mut core = super::SolidCore::new();
        // Paint A at Split size (300x400), then switch to B so A is kept.
        core.apply_scene(board.into(), Some(dummy_layers()), true, true, 0, 0, 0, (300.0, 400.0));
        let (_, _, _, rev_a, w, h) = core.prepare_render((300.0, 400.0, 1.0), false, 1.0).unwrap();
        core.latest = 1;
        assert!(core.apply_image(1, board.into(), rev_a, dummy_img(w, h), w, h));
        core.apply_scene(mech.into(), Some(dummy_layers()), true, true, 0, 0, 0, (600.0, 800.0));
        // Restore A at full height (600x800): cached Split image is ineligible, mesh+cam are restored, ticket N=2 is queued.
        assert!(core.restore_kept(board, (600.0, 800.0, 1.0), 1.0));
        assert!(core.image.is_none());
        let (_, _, _, rev_n, wn, hn) = core.prepare_render((600.0, 800.0, 1.0), false, 1.0).unwrap();
        core.latest = 2;
        // A's mesh-less Parts header arrives before image N=2 finishes: must NOT advance floor past N=2 when no replacement render is queued.
        let hdr = core.apply_scene(board.into(), None, false, true, 0, 0, 4, (600.0, 800.0));
        assert!(!hdr.should_render);
        assert!(core.floor <= 2, "floor must not advance past in-flight ticket when mesh is None");
        // Image N=2 arrives and is accepted and shown.
        assert!(core.apply_image(2, board.into(), rev_n, dummy_img(wn, hn), wn, hn));
        assert!(core.image.is_some() && core.image_key == board && core.image_size == (600, 800));
    }
}

//! A board's model to the clients: the bare board at once (parse.rs, mesh.rs, in parallel), then each part model as it is ready, the
//! instances of a model in one frame (`plot::part_frame`), after a frame that starts them again (`plot::parts_frame`).
//!
//! Where a part stands is KiCad's own arithmetic (3d-viewer: BOARD_ADAPTER::GetFootprintMatrix, CalcModelMatrix), in its 3D space
//! (mm, x right, y UP, z up): T(x, -y, seat) Rz(angle) [Ry(pi) Rz(pi) on the back] T(offset) Rz(-rz) Ry(-ry) Rx(-rx) S(scale). The wire is
//! 10 um integers in board axes (y down): W = diag(100, -100, 100) from that space, so a model's mesh goes out as W v and its instance as
//! W M W^-1, which takes the mesh's integers to the board model's.
use super::{mesh, parse, Board, Footprint, Shape, Side, P2};
use super::super::{glb::Tri3, plot::{Plot, Solid}, solid};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// What the board's own model is, and what is needed to place its parts.
pub struct Made {
    pub plot: Plot,
    pub board: Board,
    /// the parts' seat on each side: the top of the front copper, the bottom of the back's (mm, z)
    pub seat: (f64, f64),
    pub layers: Vec<super::Layer>,
    /// parse and mesh, ms
    pub ms: (f64, f64),
}

/// The bare board straight from its file: parsed, every layer meshed, as the `solid` the clients draw.
pub fn board_model(bytes: &[u8]) -> Result<Made, String> {
    let t = Instant::now();
    let board = parse::board(bytes)?;
    let parse_ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    let layers = mesh::layers(&board);
    let mesh_ms = t.elapsed().as_secs_f64() * 1000.0;
    let tris = super::glbw::tris(&layers);
    let seat = seat_of(&layers, &board);
    Ok(Made { plot: solid::plot(solid::of(&tris)), board, seat, layers, ms: (parse_ms, mesh_ms) })
}

/// The front seat is the highest point of the front copper (else the laminate's top), the back's the lowest of the back copper.
fn seat_of(layers: &[super::Layer], b: &Board) -> (f64, f64) {
    let z = |name: &str, top: bool| layers.iter().filter(|l| l.name == name).flat_map(|l| l.pos.iter().map(|p| p[2] as f64)).fold(None, |a: Option<f64>, z| Some(match a { None => z, Some(a) => if top { a.max(z) } else { a.min(z) } }));
    let top = z("copper_top", true).or_else(|| z("laminate", true)).unwrap_or(b.stack.thickness);
    let bot = z("copper_bottom", false).or_else(|| z("laminate", false)).unwrap_or(0.0);
    (top, bot)
}

// ---- placement ------------------------------------------------------------------------------------------------------------
type M = [[f64; 4]; 3];
const I: M = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];
fn mul(a: &M, b: &M) -> M {
    let mut o = [[0.0; 4]; 3];
    for r in 0..3 { for c in 0..4 { o[r][c] = (0..3).map(|k| a[r][k] * b[k][c]).sum::<f64>() + if c == 3 { a[r][3] } else { 0.0 }; } }
    o
}
fn tr(x: f64, y: f64, z: f64) -> M { [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]] }
fn sc(x: f64, y: f64, z: f64) -> M { [[x, 0.0, 0.0, 0.0], [0.0, y, 0.0, 0.0], [0.0, 0.0, z, 0.0]] }
/// a turn of `deg` about axis 0 (x), 1 (y) or 2 (z), right-handed as glm::rotate
fn rot(axis: usize, deg: f64) -> M {
    let (s, c) = deg.to_radians().sin_cos();
    match axis {
        0 => [[1.0, 0.0, 0.0, 0.0], [0.0, c, -s, 0.0], [0.0, s, c, 0.0]],
        1 => [[c, 0.0, s, 0.0], [0.0, 1.0, 0.0, 0.0], [-s, 0.0, c, 0.0]],
        _ => [[c, -s, 0.0, 0.0], [s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]],
    }
}
/// GetFootprintMatrix: the footprint in KiCad's 3D space (mm, y up)
fn footprint_matrix(fp: &Footprint, seat: (f64, f64)) -> M {
    let back = fp.side == Side::Bottom;
    let mut m = mul(&tr(fp.at.x, -fp.at.y, if back { seat.1 } else { seat.0 }), &rot(2, fp.rot));
    if back { m = mul(&mul(&m, &rot(1, 180.0)), &rot(2, 180.0)); }
    m
}
/// CalcModelMatrix
fn model_matrix(m: &super::ModelRef) -> M {
    let [ox, oy, oz] = m.offset; let [rx, ry, rz] = m.rotate; let [sx, sy, sz] = m.scale;
    mul(&mul(&mul(&mul(&tr(ox, oy, oz), &rot(2, -rz)), &rot(1, -ry)), &rot(0, -rx)), &sc(sx, sy, sz))
}
/// W M W^-1 as the wire's 12 floats: W = diag(100, -100, 100), so rows and columns 1 change sign and the translation is in 10 um
fn wire(m: &M) -> [f32; 12] {
    let s = [1.0, -1.0, 1.0];
    let mut o = [0f32; 12];
    for r in 0..3 { for c in 0..3 { o[4 * r + c] = (s[r] * s[c] * m[r][c]) as f32; } o[4 * r + 3] = (s[r] * 100.0 * m[r][3]) as f32; }
    o
}
/// A model's place on the board, as the wire's affine from the model's integers (W v) to the board model's.
pub fn instance(fp: &Footprint, m: &super::ModelRef, seat: (f64, f64)) -> [f32; 12] { wire(&mul(&footprint_matrix(fp, seat), &model_matrix(m))) }
/// A model's triangles (mm, its own frame: KiCad's 3D space) as the wire's integers W v: (n, bytes).
pub fn encode_model(ts: &[Tri3]) -> (u32, Vec<u8>) {
    let q = |v: f32| (v as f64 * 100.0).round() as i32;
    encode(ts.iter().map(|t| ([q(t.a[0]), -q(t.a[1]), q(t.a[2]), q(t.b[0]), -q(t.b[1]), q(t.b[2]), q(t.c[0]), -q(t.c[1]), q(t.c[2])], t.color)))
}
fn zig(d: u32) -> u32 { (d << 1) ^ 0u32.wrapping_sub(d >> 31) }
fn num(o: &mut Vec<u8>, mut z: u32) { while z >= 128 { o.push((z & 127) as u8 | 128); z >>= 7 } o.push(z as u8) }
/// Triangles of integers in the `mesh` format (solid.rs `of`): the colour XOR the one before, then each corner's deltas, zigzag varints.
pub fn encode(ts: impl Iterator<Item = ([i32; 9], u32)>) -> (u32, Vec<u8>) {
    let (mut d, mut n, mut c, mut p) = (Vec::new(), 0u32, 0u32, [0u32; 3]);
    for (t, color) in ts {
        num(&mut d, color ^ c); c = color; n += 1;
        for k in 0..3 { for a in 0..3 { let v = t[3 * k + a] as u32; num(&mut d, zig(v.wrapping_sub(p[a]))); p[a] = v; } }
    }
    (n, d)
}

// ---- what is to come ------------------------------------------------------------------------------------------------------
/// One model file and every place it stands.
#[derive(Clone, Debug, PartialEq)]
/// `boxes`: each instance's box, drawn instead when the model will not convert
pub struct Use { pub path: PathBuf, pub refs: Vec<String>, pub at: Vec<[f32; 12]>, pub boxes: Vec<Option<[f32; 12]>> }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan { pub uses: Vec<Use>, pub boxes: Vec<(String, [f32; 12])>, pub of: u32 }

/// The box a part with no model is drawn as: its courtyard's (else its fab outline's) extent on the board, this tall.
pub const BOX_HIGH: f64 = 1.0;
fn extent(shapes: &[Shape]) -> Option<(P2, P2)> {
    let mut lo = P2::new(f64::MAX, f64::MAX); let mut hi = P2::new(f64::MIN, f64::MIN);
    let mut put = |p: P2, r: f64| { lo.x = lo.x.min(p.x - r); lo.y = lo.y.min(p.y - r); hi.x = hi.x.max(p.x + r); hi.y = hi.y.max(p.y + r); };
    for s in shapes { match s {
        Shape::Seg { a, b, .. } => { put(*a, 0.0); put(*b, 0.0) }
        Shape::Arc { a, m, b, .. } => { put(*a, 0.0); put(*m, 0.0); put(*b, 0.0) }
        Shape::Circle { c, r, .. } => put(*c, *r),
        Shape::Poly { pts, .. } => for p in pts { put(*p, 0.0) },
        Shape::Rect { c, w, h, rot, .. } => { let (s, k) = rot.to_radians().sin_cos(); let (hw, hh) = (w / 2.0, h / 2.0); for (x, y) in [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)] { put(P2::new(c.x + x * k + y * s, c.y + y * k - x * s), 0.0) } }
    } }
    (hi.x > lo.x && hi.y > lo.y).then_some((lo, hi))
}
/// The unit box's instance for a footprint: the box model is the cube of 0..1000 integers, so its instance scales it to the extent.
fn box_at(fp: &Footprint, seat: (f64, f64)) -> Option<[f32; 12]> {
    let (lo, hi) = extent(&fp.courtyard).or_else(|| extent(&fp.fab))?;
    let h = BOX_HIGH.min((hi.x - lo.x).min(hi.y - lo.y).max(0.3));
    let (z0, dz) = if fp.side == Side::Bottom { (seat.1 - h, h) } else { (seat.0, h) };
    // integers 0..1000 to 10 um wire units: x' = lo.x*100 + x (hi-lo)*100/1000; the wire's y is the board's y
    let k = |a: f64| (a * 0.1) as f32;
    Some([k(hi.x - lo.x), 0.0, 0.0, (lo.x * 100.0) as f32, 0.0, k(hi.y - lo.y), 0.0, (lo.y * 100.0) as f32, 0.0, 0.0, k(dz), (z0 * 100.0) as f32])
}
/// The box model: a cube 0..1000 on each axis, grey.
pub fn box_model() -> (u32, Vec<u8>) {
    let (o, l) = (0, 1000);
    let v = |x: i32, y: i32, z: i32| [x * l + o, y * l + o, z * l + o];
    let quad = |a: [i32; 3], b: [i32; 3], c: [i32; 3], d: [i32; 3]| [[a, b, c], [a, c, d]];
    let faces = [
        quad(v(0, 0, 0), v(0, 1, 0), v(1, 1, 0), v(1, 0, 0)), quad(v(0, 0, 1), v(1, 0, 1), v(1, 1, 1), v(0, 1, 1)),
        quad(v(0, 0, 0), v(1, 0, 0), v(1, 0, 1), v(0, 0, 1)), quad(v(0, 1, 0), v(0, 1, 1), v(1, 1, 1), v(1, 1, 0)),
        quad(v(0, 0, 0), v(0, 0, 1), v(0, 1, 1), v(0, 1, 0)), quad(v(1, 0, 0), v(1, 1, 0), v(1, 1, 1), v(1, 0, 1)),
    ];
    encode(faces.iter().flatten().map(|t| ([t[0][0], t[0][1], t[0][2], t[1][0], t[1][1], t[1][2], t[2][0], t[2][1], t[2][2]], 0x2a2c30u32)))
}
/// The box model's id on the wire (a model's id starting `b` is a box).
pub const BOX_ID: &str = "b000000000000001";

/// Every footprint's models, grouped by the file they resolve to (`resolve`: a reference to a file, None when it is missing). A part
/// whose every shown model is missing is a box; a part with no model at all is nothing.
pub fn plan(b: &Board, seat: (f64, f64), resolve: &dyn Fn(&str) -> Option<PathBuf>) -> Plan {
    let mut p = Plan::default();
    let mut at: std::collections::HashMap<PathBuf, usize> = Default::default();
    let mut memo: std::collections::HashMap<String, Option<PathBuf>> = Default::default();
    for fp in &b.footprints {
        let shown: Vec<&super::ModelRef> = fp.models.iter().filter(|m| !m.hide && !m.path.trim().is_empty()).collect();
        if shown.is_empty() { continue }
        let mut any = false;
        for m in shown {
            let Some(path) = memo.entry(m.path.clone()).or_insert_with(|| resolve(&m.path)).clone() else { continue };
            any = true;
            let i = *at.entry(path.clone()).or_insert_with(|| { p.uses.push(Use { path, refs: vec![], at: vec![], boxes: vec![] }); p.uses.len() - 1 });
            p.uses[i].refs.push(fp.reference.clone()); p.uses[i].at.push(instance(fp, m, seat)); p.uses[i].boxes.push(box_at(fp, seat)); p.of += 1;
        }
        if !any { if let Some(a) = box_at(fp, seat) { p.boxes.push((fp.reference.clone(), a)); p.of += 1; } }
    }
    p
}
/// A part model's id on the wire: 16 hex digits of its cache key (or of its path, when it has none).
pub fn part_id(key: &str) -> String { let h = super::super::plot::mesh_id(key.as_bytes()); format!("{h:016x}") }

/// The board's folder: KIPRJMOD.
pub fn board_dir(path: &str) -> PathBuf { Path::new(path).parent().map(Path::to_path_buf).unwrap_or_default() }

/// Where a board's own GLB is kept: the models cache, by the board's path and exact revision.
pub fn glb_path(path: &str, rev: &str) -> PathBuf {
    let rev: String = rev.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { '.' }).collect();
    PathBuf::from(format!("{}/board-native-v1-{:08x}.{rev}.glb", super::super::stepglb::cache_dir(), super::super::stepglb::fnv(path)))
}
/// The board's GLB into the cache (through a staged file and a rename), its older revisions removed: one a board.
pub fn keep_glb(layers: &[super::Layer], name: &str, path: &str, rev: &str) {
    let out = glb_path(path, rev);
    if super::super::stepglb::have(&out.to_string_lossy()) { return }
    let Some(dir) = out.parent() else { return }; let _ = std::fs::create_dir_all(dir);
    let bytes = super::glbw::glb(layers, name);
    if bytes.is_empty() { return }
    let stage = out.with_extension(format!("glb.{}.part", std::process::id()));
    if std::fs::write(&stage, &bytes).is_err() || std::fs::rename(&stage, &out).is_err() { let _ = std::fs::remove_file(&stage); return }
    let prefix = format!("board-native-v1-{:08x}.", super::super::stepglb::fnv(path));
    let own = out.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if let Ok(rd) = std::fs::read_dir(dir) { for e in rd.flatten() { let n = e.file_name().to_string_lossy().into_owned(); if n.starts_with(&prefix) && n.ends_with(".glb") && n != own { let _ = std::fs::remove_file(e.path()); } } }
}

/// The note a model with missing parts carries (as kicad-cli's log made it): "" when every part has its model.
pub fn missing_note(p: &Plan) -> String {
    let n = p.boxes.len();
    if n == 0 { return String::new() }
    format!("{n} {} no 3D model on the hub's machine (KiCad's 3D library is not installed there, or the paths are wrong): {} shown as a box", if n == 1 { "part has" } else { "parts have" }, if n == 1 { "it is" } else { "they are" })
}

/// The most triangles a model's parts may add (the phones refuse a model of over (64 << 20) / 84 triangles, board and parts together):
/// past it a part is drawn as its box.
pub const MOST_TRIS: u64 = (64 << 20) / 84 - 40_000;
/// A part model this big is thinned (solid::thin) before it goes out.
const THIN_OVER: usize = 8000;

/// How long the parts took, from the stream's start (ms), and what came.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Times { pub first: Option<f64>, pub all: f64, pub models: usize, pub boxes: usize, pub failed: usize }

/// Every part of the plan to `emit`, in order: the frame that starts them, the boxes, then each model the moment it is converted
/// (models::convert, cached by content). Blocks until all are out or `stop` is set. `board_tris`: what the board itself already uses.
#[allow(clippy::too_many_arguments)]
pub fn parts(key: &str, rev: &str, plan: &Plan, board_tris: u64, cache: &super::models::Cache, tools: &[super::super::stepglb::Mesher], stop: &std::sync::atomic::AtomicBool, emit: &(dyn Fn(Vec<u8>) + Sync)) -> Times {
    use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering::Relaxed};
    let t0 = Instant::now();
    let (done, budget, first) = (AtomicU32::new(0), AtomicU64::new(MOST_TRIS.saturating_sub(board_tris)), std::sync::Mutex::new(None::<f64>));
    let (models, failed, boxed) = (AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(plan.boxes.len()));
    emit(super::super::plot::parts_frame(key, plan.of, rev));
    let (bn, bm) = box_model();
    let boxes = |refs: Vec<String>, at: Vec<[f32; 12]>| {
        if at.is_empty() { return }
        let d = done.fetch_add(at.len() as u32, Relaxed) + at.len() as u32;
        emit(super::super::plot::part_frame(key, BOX_ID, bn, &bm, &at, &refs, d));
        first.lock().unwrap().get_or_insert(t0.elapsed().as_secs_f64() * 1000.0);
    };
    boxes(plan.boxes.iter().map(|b| b.0.clone()).collect(), plan.boxes.iter().map(|b| b.1).collect());
    // each file's content key; a file that cannot be read is a box
    let mut by_key: std::collections::HashMap<String, Vec<usize>> = Default::default();
    let mut jobs = vec![];
    for (i, u) in plan.uses.iter().enumerate() {
        match super::models::key(&u.path) {
            Ok(k) => { if !by_key.contains_key(&k) { jobs.push((k.clone(), u.path.clone())) } by_key.entry(k).or_default().push(i) }
            Err(_) => { failed.fetch_add(1, Relaxed); boxed.fetch_add(u.at.len(), Relaxed); boxes(u.refs.clone(), u.boxes.iter().flatten().copied().collect()) }
        }
    }
    let fall_back = |u: &Use| { boxed.fetch_add(u.at.len(), Relaxed); boxes(u.refs.clone(), u.boxes.iter().flatten().copied().collect()) };
    super::models::convert(cache, jobs, tools, stop, |k: &str, r: Result<PathBuf, String>| {
        if stop.load(Relaxed) { return }
        let Some(ix) = by_key.get(k) else { return };
        let tris = r.ok().and_then(|p| std::fs::read(p).ok()).and_then(|b| super::super::glb::tris(&b).ok()).filter(|t| !t.is_empty());
        let Some(mut ts) = tris else { failed.fetch_add(1, Relaxed); for &i in ix { fall_back(&plan.uses[i]) } return };
        if ts.len() > THIN_OVER { ts = solid::thin(ts) }
        let (n, mesh) = encode_model(&ts);
        let id = part_id(k);
        for &i in ix {
            let u = &plan.uses[i];
            let cost = n as u64 * u.at.len() as u64;
            if budget.fetch_update(Relaxed, Relaxed, |b| b.checked_sub(cost)).is_err() { fall_back(u); continue } // too many triangles for a phone: its boxes
            let d = done.fetch_add(u.at.len() as u32, Relaxed) + u.at.len() as u32;
            emit(super::super::plot::part_frame(key, &id, n, &mesh, &u.at, &u.refs, d));
            models.fetch_add(1, Relaxed);
            first.lock().unwrap().get_or_insert(t0.elapsed().as_secs_f64() * 1000.0);
        }
    });
    let first = *first.lock().unwrap();
    Times { first, all: t0.elapsed().as_secs_f64() * 1000.0, models: models.into_inner(), boxes: boxed.into_inner(), failed: failed.into_inner() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::ModelRef;
    fn fp(x: f64, y: f64, rot: f64, side: Side) -> Footprint {
        Footprint { reference: "R1".into(), value: String::new(), name: String::new(), at: P2::new(x, y), rot, side, models: vec![], courtyard: vec![Shape::Poly { pts: vec![P2::new(x - 1.0, y - 0.5), P2::new(x + 1.0, y - 0.5), P2::new(x + 1.0, y + 0.5), P2::new(x - 1.0, y + 0.5)], w: 0.05, fill: false }], fab: vec![], dnp: false }
    }
    fn mr(path: &str) -> ModelRef { ModelRef { path: path.into(), offset: [0.0; 3], scale: [1.0; 3], rotate: [0.0; 3], hide: false, opacity: 1.0 } }
    /// a model point (mm, KiCad 3D space) through an instance to the board model's integers
    fn place(a: &[f32; 12], v: [f64; 3]) -> [f64; 3] {
        let w = [v[0] * 100.0, -v[1] * 100.0, v[2] * 100.0];
        std::array::from_fn(|r| a[4 * r] as f64 * w[0] + a[4 * r + 1] as f64 * w[1] + a[4 * r + 2] as f64 * w[2] + a[4 * r + 3] as f64)
    }
    fn near(a: [f64; 3], b: [f64; 3]) -> bool { (0..3).all(|k| (a[k] - b[k]).abs() < 0.01) }
    #[test] fn a_front_part_stands_on_the_copper_at_its_place_turned_as_kicad_turns_it() {
        let seat = (1.635, -0.035);
        // KiCad's model +x at the footprint's place; at 90 degrees counter-clockwise on screen it points up the screen: board -y
        let a = instance(&fp(10.0, 20.0, 0.0, Side::Top), &mr("m"), seat);
        assert!(near(place(&a, [0.0, 0.0, 0.0]), [1000.0, 2000.0, 163.5]));
        assert!(near(place(&a, [1.0, 0.0, 0.0]), [1100.0, 2000.0, 163.5]));
        assert!(near(place(&a, [0.0, 1.0, 0.0]), [1000.0, 1900.0, 163.5]), "the model's +y is the board's -y (up the screen)");
        let a = instance(&fp(10.0, 20.0, 90.0, Side::Top), &mr("m"), seat);
        assert!(near(place(&a, [1.0, 0.0, 0.0]), [1000.0, 1900.0, 163.5]), "{:?}", place(&a, [1.0, 0.0, 0.0]));
        assert!(near(place(&a, [0.0, 0.0, 1.0]), [1000.0, 2000.0, 263.5]), "up is up");
    }
    #[test] fn a_back_part_hangs_under_the_board_mirrored() {
        let a = instance(&fp(10.0, 20.0, 0.0, Side::Bottom), &mr("m"), (1.635, -0.035));
        assert!(near(place(&a, [0.0, 0.0, 1.0]), [1000.0, 2000.0, -103.5]), "its top points down: {:?}", place(&a, [0.0, 0.0, 1.0]));
        // Ry(pi) Rz(pi) = diag(1, -1, -1): x stays, the model's y flips
        assert!(near(place(&a, [1.0, 0.0, 0.0]), [1100.0, 2000.0, -3.5]));
        assert!(near(place(&a, [0.0, 1.0, 0.0]), [1000.0, 2100.0, -3.5]));
    }
    #[test] fn a_models_offset_rotation_and_scale_come_before_the_footprints() {
        let mut m = mr("m"); m.offset = [1.0, 0.0, 0.5]; m.rotate = [0.0, 0.0, 90.0]; m.scale = [2.0, 2.0, 2.0];
        let a = instance(&fp(0.0, 0.0, 0.0, Side::Top), &m, (0.0, 0.0));
        // Rz(-90) takes +x to -y (KiCad space), then x2, then the offset: (1, -2, 0.5) mm, the board's y is the negative of that
        assert!(near(place(&a, [1.0, 0.0, 0.0]), [100.0, 200.0, 50.0]), "{:?}", place(&a, [1.0, 0.0, 0.0]));
    }
    #[test] fn parts_group_by_file_and_a_missing_model_is_a_box_over_the_courtyard() {
        let mut b = Board::default();
        let mut r1 = fp(10.0, 10.0, 0.0, Side::Top); r1.models = vec![mr("${KICAD10_3DMODEL_DIR}/R.step")];
        let mut r2 = fp(20.0, 10.0, 0.0, Side::Top); r2.reference = "R2".into(); r2.models = vec![mr("${KICAD10_3DMODEL_DIR}/R.step")];
        let mut u1 = fp(30.0, 10.0, 0.0, Side::Bottom); u1.reference = "U1".into(); u1.models = vec![mr("gone.step")];
        let mut tp = fp(40.0, 10.0, 0.0, Side::Top); tp.reference = "TP1".into(); // no model: nothing
        let mut h = fp(50.0, 10.0, 0.0, Side::Top); h.reference = "H1".into(); let mut hidden = mr("x.step"); hidden.hide = true; h.models = vec![hidden];
        b.footprints = vec![r1, r2, u1, tp, h];
        let p = plan(&b, (1.6, 0.0), &|s: &str| s.ends_with("R.step").then(|| PathBuf::from("/lib/R.step")));
        assert_eq!(p.of, 3); assert_eq!(p.uses.len(), 1); assert_eq!(p.uses[0].refs, vec!["R1", "R2"]);
        assert_eq!(p.boxes.len(), 1); assert_eq!(p.boxes[0].0, "U1");
        // the box spans the courtyard (29..31, 9.5..10.5) and hangs 1 mm under the back
        let a = p.boxes[0].1; let corner = |x: f64, y: f64, z: f64| std::array::from_fn::<f64, 3, _>(|r| a[4 * r] as f64 * x + a[4 * r + 1] as f64 * y + a[4 * r + 2] as f64 * z + a[4 * r + 3] as f64);
        assert!(near(corner(0.0, 0.0, 0.0), [2900.0, 950.0, -100.0]) && near(corner(1000.0, 1000.0, 1000.0), [3100.0, 1050.0, 0.0]), "{:?} {:?}", corner(0.0, 0.0, 0.0), corner(1000.0, 1000.0, 1000.0));
        let (n, d) = box_model(); assert_eq!(n, 12); assert!(d.len() >= 120);
    }
    #[test] fn a_model_encodes_as_the_boards_mesh_does() {
        let t = [Tri3 { a: [0.01, 0.02, 0.03], b: [1.0, 0.0, 0.0], c: [0.0, 1.0, 0.0], color: 0xff0000 }];
        let (n, d) = encode_model(&t); assert_eq!(n, 1);
        let mut ints = vec![]; let (mut z, mut s) = (0u32, 0); for b in &d { z |= ((b & 127) as u32) << s; s += 7; if b & 128 == 0 { ints.push(z); z = 0; s = 0 } }
        let un = |z: u32| ((z >> 1) as i32) ^ -((z & 1) as i32);
        assert_eq!(ints[0], 0xff0000); assert_eq!((un(ints[1]), un(ints[2]), un(ints[3])), (1, -2, 3), "mm to 10 um, y turned to the board's");
    }
    /// A board's model in words: what each layer is (triangles, the area its upward faces cover), the model's extent, the seats, the plan.
    fn summary(path: &Path) -> String {
        let made = board_model(&std::fs::read(path).unwrap()).unwrap();
        let mut o = String::new();
        for l in &made.layers {
            let up: f64 = l.idx.chunks_exact(3).map(|t| { let (a, b, c) = (l.pos[t[0] as usize], l.pos[t[1] as usize], l.pos[t[2] as usize]);
                ((b[0] - a[0]) as f64 * (c[1] - a[1]) as f64 - (c[0] - a[0]) as f64 * (b[1] - a[1]) as f64).abs() / 2.0 * if a[2] == b[2] && b[2] == c[2] { 1.0 } else { 0.0 } }).sum();
            o += &format!("{} {:08x} tris {} area {:.2}\n", l.name, l.color, l.tris(), up);
        }
        let so = made.plot.solid.as_ref().unwrap();
        o += &format!("solid {} box {:?}\nseat {:.3} {:.3}\n", so.n, so.b.map(|v| v as i32), made.seat.0, made.seat.1);
        let p = plan(&made.board, made.seat, &|raw: &str| Some(PathBuf::from(raw)));
        o += &format!("parts {} files {} boxes {} footprints {}\n", p.of, p.uses.len(), p.boxes.len(), made.board.footprints.len());
        o
    }
    /// Golden: the fixture boards' models as they were made when checked (test/pcb3d/golden/*.txt). A change to the mesher that moves
    /// any of it fails here: look at the difference, and write the new ones with BACKPLANE_GOLDEN_UPDATE=1 when it is meant.
    #[test] fn fixture_boards_make_the_models_they_made() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for name in ["ecc83-pp", "microwave"] {
            let got = summary(&root.join(format!("test/files/fixtures/plot/{name}.kicad_pcb")));
            let gold = root.join(format!("test/pcb3d/golden/{name}.txt"));
            if std::env::var("BACKPLANE_GOLDEN_UPDATE").is_ok() { std::fs::create_dir_all(gold.parent().unwrap()).unwrap(); std::fs::write(&gold, &got).unwrap(); continue }
            let want = std::fs::read_to_string(&gold).unwrap_or_default();
            assert_eq!(got, want, "{name}: the model differs from {} (BACKPLANE_GOLDEN_UPDATE=1 writes it)", gold.display());
        }
    }
    /// The whole pipeline on real boards, as the hub runs it, with kicad-cli's times beside it to compare. Opt in:
    /// BACKPLANE_TEST_BOARD=<a.kicad_pcb>:<b.kicad_pcb> [BACKPLANE_KICAD_CLI=<kicad-cli>] cargo test --release pcb3d::stream::tests::bench -- --ignored --nocapture
    /// Prints: time to the first board (read, parse, mesh, the solid), then the parts cold (an empty models cache) and warm.
    #[test] #[ignore] fn bench() {
        let Ok(list) = std::env::var("BACKPLANE_TEST_BOARD") else { eprintln!("skipped: no BACKPLANE_TEST_BOARD"); return };
        let cli = std::env::var("BACKPLANE_KICAD_CLI").ok();
        let home = std::env::temp_dir().join(format!("bp-pcb3d-bench-{}", std::process::id()));
        let tools = super::super::super::stepglb::machine(&home);
        for path in list.split(':').filter(|p| !p.is_empty()) {
            let t = Instant::now();
            let bytes = std::fs::read(path).unwrap();
            let made = board_model(&bytes).unwrap();
            let first = t.elapsed().as_secs_f64() * 1000.0;
            let so = made.plot.solid.as_ref().expect("a model");
            let frame = super::super::super::plot::solid_frame("2|/r|", so);
            let per: Vec<String> = made.layers.iter().map(|l| format!("{} {}", l.name, l.tris())).collect();
            eprintln!("{path}\n  first board {first:.1} ms (parse {:.1}, mesh {:.1}), {} triangles, frame {} KB\n  layers: {}", made.ms.0, made.ms.1, so.n, frame.len() >> 10, per.join(", "));
            let env = super::super::models::Env::system(cli.as_deref().map(Path::new));
            let dir = board_dir(path);
            let tp = Instant::now();
            let plan = plan(&made.board, made.seat, &|raw: &str| super::super::models::resolve(raw, &dir, &env));
            eprintln!("  plan {:.1} ms: {} instances, {} files, {} boxes", tp.elapsed().as_secs_f64() * 1000.0, plan.of, plan.uses.len(), plan.boxes.len());
            let cold = super::super::models::Cache::new(home.join(format!("cold-{}", super::super::super::stepglb::fnv(path))));
            for (name, cache) in [("cold", &cold), ("warm", &cold)] {
                let (frames, bytes_out) = (std::sync::atomic::AtomicUsize::new(0), std::sync::atomic::AtomicUsize::new(0));
                let stop = std::sync::atomic::AtomicBool::new(false);
                let t = parts("2|/r|", "rev", &plan, so.n as u64, cache, &tools, &stop, &|f: Vec<u8>| { frames.fetch_add(1, std::sync::atomic::Ordering::Relaxed); bytes_out.fetch_add(f.len(), std::sync::atomic::Ordering::Relaxed); });
                eprintln!("  parts {name}: first {} ms, all {:.0} ms; {} frames, {} KB; {} part frames, {} boxed, {} failed; to all parts from the start {:.0} ms",
                    t.first.map_or("-".into(), |f| format!("{f:.0}")), t.all, frames.into_inner(), bytes_out.into_inner() >> 10, t.models, t.boxes, t.failed, first + tp.elapsed().as_secs_f64() * 1000.0);
            }
            if let Some(cli) = &cli {
                let out = home.join("kicad.glb"); let _ = std::fs::create_dir_all(&home);
                for bare in [true, false] {
                    let t = Instant::now();
                    let o = crate::proc::std_cmd(crate::proc::Kind::Local, cli).args(super::super::super::solidread::flags(bare, false, &out.to_string_lossy(), path)).output().unwrap();
                    let ms = t.elapsed().as_secs_f64() * 1000.0;
                    let b = std::fs::read(&out).unwrap_or_default();
                    let t2 = Instant::now(); let n = super::super::super::solidread::plot_of(&b, false).ok().and_then(|p| p.solid).map_or(0, |s| s.n);
                    eprintln!("  kicad-cli {}: export {ms:.0} ms (exit {}), to the solid {:.0} ms more, {n} triangles", if bare { "bare" } else { "with parts" }, o.status.code().unwrap_or(-1), t2.elapsed().as_secs_f64() * 1000.0);
                }
            }
        }
        let _ = std::fs::remove_dir_all(&home);
    }

    /// For test/pcb3d/compare.py: the board's GLB as made here and where every part stands (each instance's extent in board axes, mm), from
    /// a warm models cache (run `bench` first). BACKPLANE_TEST_BOARD=<boards> BACKPLANE_PCB3D_DUMP=<dir> cargo test pcb3d::stream::tests::dump -- --ignored
    #[test] #[ignore] fn dump() {
        let (Ok(list), Ok(out)) = (std::env::var("BACKPLANE_TEST_BOARD"), std::env::var("BACKPLANE_PCB3D_DUMP")) else { eprintln!("skipped"); return };
        let out = PathBuf::from(out); std::fs::create_dir_all(&out).unwrap();
        let env = super::super::models::Env::system(std::env::var("BACKPLANE_KICAD_CLI").ok().as_deref().map(Path::new));
        let cache = super::super::models::Cache::standard();
        for path in list.split(':').filter(|p| !p.is_empty()) {
            let stem = Path::new(path).file_stem().unwrap().to_string_lossy().into_owned();
            let made = board_model(&std::fs::read(path).unwrap()).unwrap();
            std::fs::write(out.join(format!("{stem}.native.glb")), super::super::glbw::glb(&made.layers, &stem)).unwrap();
            let dir = board_dir(path);
            let plan = plan(&made.board, made.seat, &|raw: &str| super::super::models::resolve(raw, &dir, &env));
            let tools = super::super::super::stepglb::machine(&std::env::temp_dir());
            parts("2|/r|", "", &plan, 0, &cache, &tools, &std::sync::atomic::AtomicBool::new(false), &|_f: Vec<u8>| {}); // the cache warm, every model converted
            let side: std::collections::HashMap<&str, &str> = made.board.footprints.iter().map(|f| (f.reference.as_str(), if f.side == Side::Bottom { "B" } else { "F" })).collect();
            let ext = |ts: &[[i32; 9]], a: &[f32; 12]| {
                let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
                for t in ts { for k in 0..3 { let v = [t[3 * k] as f64, t[3 * k + 1] as f64, t[3 * k + 2] as f64];
                    for r in 0..3 { let w = (a[4 * r] as f64 * v[0] + a[4 * r + 1] as f64 * v[1] + a[4 * r + 2] as f64 * v[2] + a[4 * r + 3] as f64) * 0.01; lo[r] = lo[r].min(w); hi[r] = hi[r].max(w); } } }
                [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]]
            };
            let mut rows = vec![];
            for u in &plan.uses {
                let Ok(k) = super::super::models::key(&u.path) else { continue };
                let Some(Ok(g)) = cache.lookup(&k) else { continue };
                let ts = super::super::super::glb::tris(&std::fs::read(g).unwrap()).unwrap();
                let q = |v: f32| (v as f64 * 100.0).round() as i32;
                let ints: Vec<[i32; 9]> = ts.iter().map(|t| [q(t.a[0]), -q(t.a[1]), q(t.a[2]), q(t.b[0]), -q(t.b[1]), q(t.b[2]), q(t.c[0]), -q(t.c[1]), q(t.c[2])]).collect();
                for (r, a) in u.refs.iter().zip(&u.at) { rows.push(serde_json::json!({"ref": r, "side": side.get(r.as_str()).copied().unwrap_or("?"), "box": ext(&ints, a), "model": u.path.to_string_lossy()})); }
            }
            let cube: Vec<[i32; 9]> = vec![[0, 0, 0, 1000, 1000, 1000, 0, 0, 0]];
            for (r, a) in &plan.boxes { rows.push(serde_json::json!({"ref": r, "side": side.get(r.as_str()).copied().unwrap_or("?"), "box": ext(&cube, a), "model": "box"})); }
            std::fs::write(out.join(format!("{stem}.parts.json")), serde_json::to_string_pretty(&rows).unwrap()).unwrap();
            eprintln!("{stem}: {} parts placed", rows.len());
        }
    }

}

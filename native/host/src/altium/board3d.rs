//! An Altium board (.PcbDoc) as the native 3D view's board model (kicad/pcb3d::Board), read here with no KiCad: the outline and its
//! cutouts, the outer copper (tracks, arcs, fills, pours, copper text), pads and vias with their holes, the mask's openings, the silk,
//! the thickness of the layer stack, and the parts with their 3D bodies.
//!
//! The 2D reader (pcb.rs) makes every drawn shape; this module takes the outer layers' shapes from it and the pads and vias from the
//! same records (`pcb::pad_recs`, `pcb::via_recs`), so the 3D board is the board the Board view draws.
//!
//! Parts: a component (Components6) is a footprint placed as KiCad's own importer places it (its position, rotation, and the bottom
//! side as a flipped footprint). Its bodies (ComponentBodies6) are its 3D models:
//! - a STEP model the board carries (`Models/Data` lists them; `Models/N` is the N-th, zlib-compressed) is written out once, named by
//!   its content, into the models folder, and placed with the body's transform (MODEL.2D.X/Y, MODEL.3D.ROTX/Y/Z, MODEL.3D.DZ, its
//!   projection side) by the arithmetic of KiCad's importer (altium_pcb.cpp, ParseComponentsBodies6Data), so a part stands where
//!   KiCad's import of the same board stands it;
//! - an extruded body (MODELTYPE 0: an outline, a standoff and a height) is a prism, written as a small VRML model in the
//!   footprint's own frame (one file per distinct shape) that the models path reads natively;
//! - a STEP linked by name (not embedded) is a model reference by that name, resolved beside the board like any model file (a part
//!   whose file is not there is a box, and the 3D view says which file was not found).
//! The parts then go the way a KiCad board's go (pcb3d::stream::plan, models::convert, cached by content).
use super::pcb::{self, CompRec, Src};
use super::{Bin, Doc, Props};
use crate::files::kicad::board::{Item, Prim, Pt};
use crate::files::kicad::pcb3d::{geom, Board, Drill, Footprint, ModelRef, Pad, PadLayers, Shape, Side, Stackup, Via, P2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// This reader's version, in the names of the board models kept from it (stream.rs `board_model_kept`): raise it when it draws differently.
pub const VERSION: &str = "alt2-";

/// The board model of a .PcbDoc's bytes; the models it carries are written into `models` (named by content: written once).
pub fn board(bytes: &[u8], models: &Path) -> Result<Board, String> {
    let mut doc = Doc::open(bytes.to_vec()).ok_or("this file is not an Altium board (not a compound file)")?;
    let src = Src::of_doc(&mut doc).ok_or("this file is not an Altium board (it has no Board6 record)")?;
    let (items, ctx) = pcb::build(&src);
    let thick = ctx.layers.thick_um as f64 / 1000.0;
    let rule = pcb::mask_rule(&src);
    let mut b = Board { stack: Stackup { thickness: thick, copper: ctx.layers.copper_mm(), mask_margin: rule, ..Stackup::default() }, ..Board::default() };
    for it in &items { item(&mut b, it) }
    let refdes = |c: u16| if c == pcb::NONE { String::new() } else { ctx.comps.get(c as usize).map(|k| k.refdes.clone()).unwrap_or_default() };
    // the copper stack's outer layers (the first and last of Board6's stack)
    let side_of = |a: u32| match ctx.layers.paint(a) { Some(pcb::F_CU) => (true, false), Some(pcb::B_CU) => (false, true), Some(pcb::PADS) => (true, true), _ => (false, false) };
    for p in pcb::pad_recs(&src) { pad(&mut b, &p, side_of(p.layer), refdes(p.comp), rule) }
    for v in pcb::via_recs(&src) {
        let c = P2::new(v.x, -v.y);
        let (top, bottom) = (side_of(v.from as u32).0, side_of(v.to as u32).1);
        if top && bottom { b.vias.push(Via { c, d: v.d, drill: v.hole, tented_top: v.tent_top, tented_bottom: v.tent_bottom }); continue }
        // a blind or buried via: its land on the outer side it reaches, no hole through the board (as pcb3d draws KiCad's)
        for (on, bot) in [(top, false), (bottom, true)] {
            if on { b.pads.push(Pad { shapes: vec![Shape::Circle { c, r: v.d / 2.0, w: 0.0, fill: true }], layers: PadLayers { cu_top: !bot, cu_bottom: bot, ..PadLayers::default() }, drill: None, mask_margin: 0.0, reference: String::new() }) }
        }
    }
    let bodies = bodies(&src);
    let embedded = embedded_models(&mut doc, models);
    for (ci, k) in ctx.comps.iter().enumerate() {
        let mine: Vec<&Body> = bodies.iter().filter(|x| x.comp as usize == ci).collect();
        b.footprints.push(footprint(k, &mine, &embedded, thick, models));
    }
    Ok(b)
}

// ---- the 2D reader's shapes ---------------------------------------------------------------------------------------------------

fn p2(p: &Pt) -> P2 { P2::new(p.x as f64, p.y as f64) }
/// A 2D primitive as pcb3d shapes (a polyline is its segments)
fn shapes(prim: &Prim, out: &mut Vec<Shape>) {
    match prim {
        Prim::Seg { a, b, w } => out.push(Shape::Seg { a: p2(a), b: p2(b), w: *w as f64 }),
        Prim::Line { ps, w } => {
            if ps.len() == 1 { out.push(Shape::Seg { a: p2(&ps[0]), b: p2(&ps[0]), w: *w as f64 }) }
            for s in ps.windows(2) { out.push(Shape::Seg { a: p2(&s[0]), b: p2(&s[1]), w: *w as f64 }) }
        }
        Prim::Circ { c, r, w, fill } => out.push(Shape::Circle { c: p2(c), r: *r as f64, w: *w as f64, fill: *fill }),
        Prim::Poly { ps, w, fill } => out.push(Shape::Poly { pts: ps.iter().map(p2).collect(), w: *w as f64, fill: *fill }),
        Prim::Tint { .. } => {}
    }
}
fn shape(prim: &Prim) -> Option<Shape> { let mut v = vec![]; shapes(prim, &mut v); (v.len() == 1).then(|| v.remove(0)) }

/// One of the 2D reader's items onto the board model by its layer. Pads and vias (keys `p/`, `v/`) come from their records instead.
fn item(b: &mut Board, it: &Item) {
    if it.key.starts_with("p/") || it.key.starts_with("v/") { return }
    let sides: &[Side] = match it.layer { pcb::F_CU | pcb::F_SILK | pcb::F_MASK => &[Side::Top], pcb::B_CU | pcb::B_SILK | pcb::B_MASK => &[Side::Bottom], pcb::PADS => &[Side::Top, Side::Bottom], _ => &[] };
    match it.layer {
        pcb::EDGE => match &it.prim {
            // a closed outline or cutout is one loop
            Prim::Line { ps, w } if ps.len() >= 4 && ps.first() == ps.last() => b.outline.push(Shape::Poly { pts: ps[..ps.len() - 1].iter().map(p2).collect(), w: *w as f64, fill: false }),
            p => shapes(p, &mut b.outline),
        },
        pcb::F_CU | pcb::B_CU | pcb::PADS => for s in sides {
            match &it.prim {
                // a pour, region or fill: a filled polygon (holes already joined to it)
                Prim::Poly { ps, w, fill: true } if *w == 0.0 => b.zones.push((*s, ps.iter().map(p2).collect())),
                p => { let mut v = vec![]; shapes(p, &mut v); b.copper.extend(v.into_iter().map(|x| (*s, x))) }
            }
        },
        pcb::F_SILK | pcb::B_SILK => { let mut v = vec![]; shapes(&it.prim, &mut v); b.silk.extend(v.into_iter().map(|x| (sides[0], x))) }
        pcb::F_MASK | pcb::B_MASK => { let mut v = vec![]; shapes(&it.prim, &mut v); b.mask.extend(v.into_iter().map(|x| (sides[0], x))) }
        _ => {}
    }
}

/// `rule`: the board's mask expansion, for a pad that does not set its own
fn pad(b: &mut Board, p: &pcb::PadRec, (top, bottom): (bool, bool), reference: String, rule: f64) {
    let drill = p.thru().then(|| {
        let c = P2::new(p.x, -p.y);
        if p.hole_shape == 2 && p.slot > p.hole { Drill { c, w: p.slot, h: p.hole, rot: p.dir + p.slot_rot, plated: p.plated } } else { Drill { c, w: p.hole, h: p.hole, rot: 0.0, plated: p.plated } }
    });
    // an unplated hole has no copper (as the 2D reader and KiCad's import give it)
    let copper = !p.thru() || p.plated;
    let (t, bt) = (p.copper(false, 0.0).and_then(|x| shape(&x)), p.copper(true, 0.0).and_then(|x| shape(&x)));
    let mk = |s: Option<Shape>, top: bool, bottom: bool, drill: Option<Drill>| Pad {
        shapes: s.into_iter().collect(), layers: PadLayers { cu_top: top, cu_bottom: bottom, mask_top: top && !p.tent_top, mask_bottom: bottom && !p.tent_bottom },
        drill, mask_margin: p.mask_x.unwrap_or(rule), reference: reference.clone() };
    if !copper || (!top && !bottom) { if let Some(d) = drill { b.holes.push(d) } return }
    if top && bottom && t == bt { b.pads.push(mk(t, true, true, drill)); return }
    // a pad whose top and bottom differ: one a side, the hole with the first
    let mut drill = drill;
    if top { b.pads.push(mk(t, true, false, drill.take())) }
    if bottom { b.pads.push(mk(bt, false, true, drill.take())) }
}

// ---- parts --------------------------------------------------------------------------------------------------------------------

/// A component body (ComponentBodies6): its part, its properties, its outline on the board (mm, Altium's Y up).
pub(super) struct Body { pub comp: u16, pub p: Props, pub outline: Vec<(f64, f64)> }
/// ComponentBodies6: records of type 12, one sub-record: 7 bytes, the component (u16), 9 bytes, the properties (u32 length with a
/// type in its top byte, then `|KEY=VALUE|` text), then the outline: a u32 count and that many (x, y) doubles in 1/10000 mil.
pub(super) fn bodies(src: &Src) -> Vec<Body> {
    pcb::records(src.get("ComponentBodies6"), 12, 1).into_iter().map(|(_, _, subs)| {
        let mut r = Bin::new(subs[0]);
        r.skip(7);
        let comp = r.u16();
        r.skip(9);
        let n = (r.u32() & 0x00FF_FFFF) as usize;
        let p = Props::parse(r.bytes(n));
        let k = r.u32() as usize;
        let outline = (0..k.min(r.left() / 16)).map(|_| { let (x, y) = (r.f64(), r.f64()); (x * 2.54e-6, y * 2.54e-6) }).collect();
        Body { comp, p, outline }
    }).collect()
}

/// The STEP models the board carries, by their ID (upper case): each written once into `dir`, named by its compressed bytes' hash and its
/// own name. Err: why one could not be (not a STEP, a stream that does not inflate).
pub(super) fn embedded_models(doc: &mut Doc, dir: &Path) -> HashMap<String, Result<PathBuf, String>> {
    let mut out = HashMap::new();
    let Some(list) = doc.stream("Models/Data") else { return out };
    for (k, p) in super::pcb_text_records(&list).into_iter().enumerate() {
        let id = p.get("ID").trim().to_ascii_uppercase();
        if id.is_empty() { continue }
        let Some(z) = doc.stream(&format!("Models/{k}")) else { out.insert(id, Err(format!("the board has no data for model {}", p.get("NAME")))); continue };
        out.insert(id, write_model(&z, p.get("NAME"), dir));
    }
    out
}
/// One model's bytes (zlib, as Altium keeps it) written out as a STEP file; the file when it is already there.
pub(super) fn write_model(z: &[u8], name: &str, dir: &Path) -> Result<PathBuf, String> {
    let mut safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
    let low = safe.to_ascii_lowercase();
    if !(low.ends_with(".step") || low.ends_with(".stp")) { safe.push_str(".step") }
    let file = dir.join(format!("altium-v1-{:016x}-{safe}", crate::files::kicad::plot::mesh_id(z)));
    if file.is_file() { return Ok(file) }
    let mut data = Vec::new();
    if std::io::Read::read_to_end(&mut flate2::read::ZlibDecoder::new(z), &mut data).is_err() { data = z.to_vec() }
    if !data.starts_with(b"ISO-10303-21") { return Err(format!("{name}: not a STEP model (only STEP models are drawn)")) }
    put(&file, &data).map_err(|e| e.to_string())?;
    Ok(file)
}
/// A file written whole or not at all: through a staged file of this writer's own (a prewarm and a 3D view may write one at once)
fn put(file: &Path, data: &[u8]) -> std::io::Result<()> {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if let Some(d) = file.parent() { std::fs::create_dir_all(d)? }
    let tmp = file.with_file_name(format!(".{}.{}-{}.part", file.file_name().unwrap_or_default().to_string_lossy(), std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let r = std::fs::write(&tmp, data).and_then(|_| std::fs::rename(&tmp, file));
    if r.is_err() { let _ = std::fs::remove_file(&tmp); }
    r
}

fn mil(p: &Props, k: &str) -> f64 { super::parse_num(p.get(k)) * 0.0254 }
fn norm180(a: f64) -> f64 { let r = (a + 180.0).rem_euclid(360.0) - 180.0; if r == -180.0 { 180.0 } else { r } }

/// Where a body's model stands in its footprint, KiCad's way (ParseComponentsBodies6Data): the body's position less the part's, in
/// the part's unturned frame (y up; mirrored for a bottom part), and its turns negated, the part's own turn taken back out of the
/// model's z turn (the footprint turns it again). A body projected onto the other side than its part's is turned over and set below
/// the board. `thick`: the board's thickness, mm. Returns (offset mm, rotate degrees) as a KiCad model reference has them.
pub(super) fn place(k: &CompRec, p: &Props, thick: f64) -> ([f64; 3], [f64; 3]) {
    let (fx, fy) = (k.x, -k.y); // the part, KiCad's board axes (y down)
    let (mx, my) = (mil(p, "MODEL.2D.X"), -mil(p, "MODEL.2D.Y"));
    let mut off = [mx - fx, -(my - fy), mil(p, "MODEL.3D.DZ")];
    let mut orient = k.rot;
    if k.bottom { off[1] = -off[1]; orient = -orient }
    let (mut rx, ry, mut rz) = (p.num("MODEL.3D.ROTX"), p.num("MODEL.3D.ROTY"), p.num("MODEL.3D.ROTZ"));
    if (p.int("BODYPROJECTION") == 1) != k.bottom { rx += 180.0; rz = -rz; off[2] = -thick - off[2] }
    // KiCad's RotatePoint: (x cos + y sin, y cos - x sin)
    let (s, c) = orient.to_radians().sin_cos();
    let (x, y) = (off[0] * c + off[1] * s, off[1] * c - off[0] * s);
    off[0] = x; off[1] = y;
    (off, [norm180(-rx), norm180(-ry), norm180(-rz + p.num("MODEL.2D.ROTATION") + orient)])
}

fn footprint(k: &CompRec, bodies: &[&Body], embedded: &HashMap<String, Result<PathBuf, String>>, thick: f64, dir: &Path) -> Footprint {
    let mut fp = Footprint { reference: k.refdes.clone(), value: k.value.clone(), name: k.footprint.clone(), at: P2::new(k.x, -k.y), rot: k.rot,
        side: if k.bottom { Side::Bottom } else { Side::Top }, models: vec![], courtyard: vec![], fab: vec![], dnp: false };
    for b in bodies {
        // the body's outline on the board: a part whose model does not come is a box over these
        if b.outline.len() >= 3 { fp.courtyard.push(Shape::Poly { pts: b.outline.iter().map(|q| P2::new(q.0, -q.1)).collect(), w: 0.0, fill: false }) }
        let opacity = if b.p.has("BODYOPACITY3D") { b.p.num("BODYOPACITY3D").clamp(0.0, 1.0) } else { 1.0 };
        let model = |path: String, offset: [f64; 3], rotate: [f64; 3]| ModelRef { path, offset, scale: [1.0; 3], rotate, hide: false, opacity };
        match b.p.get("MODEL.MODELTYPE").trim() {
            "0" => if let Some(path) = prism(k, b, thick, dir) { fp.models.push(model(path.to_string_lossy().into_owned(), [0.0; 3], [0.0; 3])) },
            "1" => {
                let (offset, rotate) = place(k, &b.p, thick);
                let path = if b.p.bool("MODEL.EMBED") {
                    match embedded.get(&b.p.get("MODELID").trim().to_ascii_uppercase()) { Some(Ok(f)) => f.to_string_lossy().into_owned(), _ => b.p.get("MODEL.NAME").to_string() }
                } else { b.p.get("MODEL.NAME").to_string() };
                if !path.trim().is_empty() { fp.models.push(model(path, offset, rotate)) }
            }
            _ => {} // a cylinder or a sphere: not drawn yet
        }
    }
    fp
}

/// An extruded body as a prism in its footprint's own frame (KiCad's 3D model space: mm, y up, z up from the part's side of the
/// board, mirrored for a bottom part as the footprint is), written as VRML (units of 0.1 inch) into `dir`, named by its content so a
/// shape repeated across parts is one model. None: no outline or no height.
pub(super) fn prism(k: &CompRec, b: &Body, thick: f64, dir: &Path) -> Option<PathBuf> {
    let (z0, z1) = if b.p.has("MODEL.EXTRUDED.MAXZ") { (mil(&b.p, "MODEL.EXTRUDED.MINZ"), mil(&b.p, "MODEL.EXTRUDED.MAXZ")) } else { (mil(&b.p, "STANDOFFHEIGHT"), mil(&b.p, "OVERALLHEIGHT")) };
    if b.outline.len() < 3 || z1 - z0 < 1e-4 { return None }
    // a body on the other side than its part: through and below the board, from the part's side
    let (z0, z1) = if (b.p.int("BODYPROJECTION") == 1) != k.bottom { (-thick - z1, -thick - z0) } else { (z0, z1) };
    let (s, c) = (-k.rot).to_radians().sin_cos();
    let ring: geom::Ring = b.outline.iter().map(|&(x, y)| {
        let (dx, dy) = (x - k.x, y - k.y);
        let (u, v) = (dx * c - dy * s, dx * s + dy * c);
        [u, if k.bottom { -v } else { v }]
    }).collect();
    let text = vrml_prism(&ring, z0, z1, b.p.int("BODYCOLOR3D") as u32)?;
    let file = dir.join(format!("altium-body-v1-{:016x}.wrl", crate::files::kicad::plot::mesh_id(text.as_bytes())));
    if file.is_file() { return Some(file) }
    put(&file, text.as_bytes()).ok().map(|_| file)
}

/// A closed prism over `ring` (mm, y up) from z0 to z1 as a VRML2 triangle set, faces outward, in Altium's colour (0x00BBGGRR).
pub(super) fn vrml_prism(ring: &geom::Ring, z0: f64, z1: f64, bgr: u32) -> Option<String> {
    let shapes = geom::even_odd(&[ring.clone()]);
    let (pts, idx) = geom::triangulate(&shapes);
    if idx.is_empty() { return None }
    let mut v: Vec<[f64; 3]> = vec![];
    let mut tris: Vec<[usize; 3]> = vec![];
    // the caps: triangulate's are counter-clockwise (y up), so up as they are on top and turned over below
    for (z, up) in [(z1, true), (z0, false)] {
        let base = v.len();
        v.extend(pts.iter().map(|p| [p[0], p[1], z]));
        for t in idx.chunks_exact(3) { let (a, b, c) = (base + t[0] as usize, base + t[1] as usize, base + t[2] as usize); tris.push(if up { [a, b, c] } else { [a, c, b] }) }
    }
    // the walls: an outline counter-clockwise, its holes clockwise, so outward is to the right of each edge
    for sh in &shapes {
        for (n, r) in sh.iter().enumerate() {
            let mut r = r.clone();
            if (geom::area2(&r) > 0.0) != (n == 0) { r.reverse() }
            for i in 0..r.len() {
                let (p, q) = (r[i], r[(i + 1) % r.len()]);
                let base = v.len();
                v.extend([[p[0], p[1], z0], [q[0], q[1], z0], [q[0], q[1], z1], [p[0], p[1], z1]]);
                tris.push([base, base + 1, base + 2]); tris.push([base, base + 2, base + 3]);
            }
        }
    }
    let ch = |s: u32| (bgr >> s & 0xFF) as f64 / 255.0;
    let mut o = format!("#VRML V2.0 utf8\nShape {{ appearance Appearance {{ material Material {{ diffuseColor {:.3} {:.3} {:.3} }} }}\ngeometry IndexedFaceSet {{ coord Coordinate {{ point [\n", ch(0), ch(8), ch(16));
    let f = |x: f64| { let s = format!("{:.5}", x / 2.54); if s == "-0.00000" { "0.00000".to_string() } else { s } };
    for p in &v { o.push_str(&format!("{} {} {},\n", f(p[0]), f(p[1]), f(p[2]))) }
    o.push_str("] }\ncoordIndex [\n");
    for t in &tris { o.push_str(&format!("{} {} {} -1\n", t[0], t[1], t[2])) }
    o.push_str("] } }\n");
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::kicad::pcb3d::{models, stream};

    fn comp(x: f64, y: f64, rot: f64, bottom: bool) -> CompRec {
        CompRec { refdes: "U1".into(), footprint: "F".into(), value: String::new(), x, y, rot, bottom, nameon: true, commenton: true, ctx: String::new() }
    }
    fn near(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }

    #[test] fn a_top_body_sits_at_its_offset_in_the_parts_frame() {
        // a part at (10, 20) mm turned 90; its body 1 mm right of it on the board (y up), turned 90 about z with the part
        let k = comp(10.0, 20.0, 90.0, false);
        let p = Props::parse(format!("|MODEL.2D.X={}mil|MODEL.2D.Y={}mil|MODEL.3D.ROTZ=90|MODEL.3D.DZ=10mil|BODYPROJECTION=0", 11.0 / 0.0254, 20.0 / 0.0254).as_bytes());
        let (off, rot) = place(&k, &p, 1.6);
        // the part's frame is turned 90: board +x is its -y
        assert!(near(off[0], 0.0) && (off[1] + 1.0).abs() < 1e-9 && (off[2] - 0.254).abs() < 1e-9, "{off:?}");
        assert!(near(rot[0], 0.0) && near(rot[1], 0.0) && near(rot[2], 0.0), "{rot:?}");
        // through the footprint and model matrices (pcb3d's, KiCad's) the body's origin lands on the board where Altium has it
        let fp = Footprint { reference: "U1".into(), value: String::new(), name: String::new(), at: P2::new(10.0, -20.0), rot: 90.0, side: Side::Top, models: vec![], courtyard: vec![], fab: vec![], dnp: false };
        let m = ModelRef { path: String::new(), offset: off, scale: [1.0; 3], rotate: rot, hide: false, opacity: 1.0 };
        let w = stream::instance(&fp, &m, (1.6, 0.0));
        // the wire's translation is in 10 um on the board's axes (y down): Altium's (11, 20) mm is (1100, -2000)
        assert!((w[3] - 1100.0).abs() < 1e-2 && (w[7] + 2000.0).abs() < 1e-2 && (w[11] - 185.4).abs() < 1e-2, "{w:?}");
    }
    #[test] fn a_bottom_body_is_mirrored_and_one_on_the_far_side_turned_over() {
        let k = comp(0.0, 0.0, 30.0, true);
        let p = Props::parse(b"|MODEL.2D.X=0mil|MODEL.2D.Y=100mil|MODEL.3D.ROTX=0|MODEL.3D.ROTZ=45|BODYPROJECTION=1");
        let (off, rot) = place(&k, &p, 1.6);
        // y mirrored, the turn taken back out with the part's (negated on the bottom)
        let (s, c) = (-30f64).to_radians().sin_cos();
        assert!((off[0] - (-2.54 * s)).abs() < 1e-9 && (off[1] - (-2.54 * c)).abs() < 1e-9 && near(off[2], 0.0), "{off:?}");
        assert!(near(rot[2], norm180(-45.0 - 30.0)), "{rot:?}");
        // a top body on a bottom part: turned over and set through the board
        let q = Props::parse(b"|MODEL.2D.X=0mil|MODEL.2D.Y=0mil|MODEL.3D.ROTZ=45|MODEL.3D.DZ=0mil|BODYPROJECTION=0");
        let (off, rot) = place(&k, &q, 1.6);
        assert!(near(off[2], -1.6) && near(rot[0], 180.0) && near(rot[2], norm180(45.0 - 30.0)), "{off:?} {rot:?}");
    }
    #[test] fn an_extruded_body_is_a_closed_prism_in_its_parts_frame() {
        let d = crate::plat::tmp::dir("alt-prism");
        // a 2 x 1 mm body centred on a part at (5, 5) turned 90: in the part's frame it is 1 wide (x) and 2 tall (y)
        let b = Body { comp: 0, p: Props::parse(b"|MODEL.MODELTYPE=0|STANDOFFHEIGHT=0mil|OVERALLHEIGHT=39.37mil|BODYCOLOR3D=255"), outline: vec![(4.0, 4.5), (6.0, 4.5), (6.0, 5.5), (4.0, 5.5)] };
        let f = prism(&comp(5.0, 5.0, 90.0, false), &b, 1.6, &d).expect("a prism");
        let g = models::wrl_to_glb(&std::fs::read_to_string(&f).unwrap()).unwrap();
        let ts = crate::files::kicad::glb::tris(&g).unwrap();
        assert_eq!(ts.len(), 2 * 2 + 4 * 2, "two caps of two and four walls of two");
        let (lo, hi) = crate::files::kicad::mesh::bounds(&ts);
        assert!((hi[0] - lo[0] - 1.0).abs() < 1e-3 && (hi[1] - lo[1] - 2.0).abs() < 1e-3 && lo[2].abs() < 1e-3 && (hi[2] - 1.0).abs() < 1e-3, "{lo:?} {hi:?}");
        assert!(ts.iter().all(|t| (t.color >> 16 & 0xff) > 200 && (t.color & 0xff) < 10), "red: {:x}", ts[0].color);
        // every face outward: the signed volume is positive
        let vol: f64 = ts.iter().map(|t| { let (a, b, c) = (t.a.map(|x| x as f64), t.b.map(|x| x as f64), t.c.map(|x| x as f64)); a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0]) }).sum::<f64>() / 6.0;
        assert!((vol - 2.0).abs() < 1e-2, "volume {vol}");
        // the same shape on another part is the same model
        let b2 = Body { outline: b.outline.iter().map(|p| (p.0 + 10.0, p.1)).collect(), ..b };
        assert_eq!(prism(&comp(15.0, 5.0, 90.0, false), &b2, 1.6, &d), Some(f));
    }

    // a whole synthetic board as a compound file: a pad, a via, a part with an embedded STEP body
    fn text_rec(t: &str) -> Vec<u8> { let mut v = (t.len() as u32 + 1).to_le_bytes().to_vec(); v.extend_from_slice(t.as_bytes()); v.push(0); v }
    fn u(mil: f64) -> [u8; 4] { ((mil * 10000.0).round() as i32).to_le_bytes() }
    fn via(x: f64, y: f64, d: f64, hole: f64) -> Vec<u8> {
        let mut s = vec![74u8, 0x24, 0]; s.extend_from_slice(&0u16.to_le_bytes()); s.extend_from_slice(&[0xff; 4]); s.extend_from_slice(&[0; 4]);
        for v in [x, y, d, hole] { s.extend_from_slice(&u(v)) }
        s.extend_from_slice(&[1, 32]); s.resize(74, 0);
        let mut r = vec![3u8]; r.extend_from_slice(&(s.len() as u32).to_le_bytes()); r.extend(s); r
    }
    fn body(comp: u16, props: &str, outline: &[(f64, f64)]) -> Vec<u8> {
        let mut s = vec![57u8, 0, 0, 0xff, 0xff, 0xff, 0xff]; s.extend_from_slice(&comp.to_le_bytes()); s.extend_from_slice(&[0; 9]);
        s.extend_from_slice(&(props.len() as u32 + 1).to_le_bytes()); s.extend_from_slice(props.as_bytes()); s.push(0);
        s.extend_from_slice(&(outline.len() as u32).to_le_bytes());
        for (x, y) in outline { s.extend_from_slice(&(x * 10000.0).to_le_bytes()); s.extend_from_slice(&(y * 10000.0).to_le_bytes()) }
        let mut r = vec![12u8]; r.extend_from_slice(&(s.len() as u32).to_le_bytes()); r.extend(s); r
    }
    fn cfb(streams: &[(&str, Vec<u8>)]) -> Vec<u8> {
        use std::io::Write;
        let mut c = cfb::CompoundFile::create(std::io::Cursor::new(Vec::new())).unwrap();
        for (path, data) in streams {
            let dir = path.rsplit_once('/').map(|x| x.0).unwrap_or("");
            if !dir.is_empty() && !c.exists(format!("/{dir}")) { c.create_storage(format!("/{dir}")).unwrap() }
            c.create_stream(format!("/{path}")).unwrap().write_all(data).unwrap();
        }
        c.flush().unwrap();
        c.into_inner().into_inner()
    }
    fn rule(t: &str) -> Vec<u8> { let mut v = vec![2u8, 0]; v.extend(text_rec(t)); v }
    const STEP: &[u8] = b"ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";
    fn zlib(b: &[u8]) -> Vec<u8> { use std::io::Write; let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default()); e.write_all(b).unwrap(); e.finish().unwrap() }

    #[test] fn a_synthetic_pcbdoc_becomes_a_board_with_its_part_and_model() {
        let board6 = "|LAYER1NAME=Top Layer|LAYER1NEXT=32|LAYER1COPTHICK=1.4mil|LAYER1DIELHEIGHT=60mil|LAYER32NAME=Bottom Layer|LAYER32NEXT=0|LAYER32COPTHICK=1.4mil\
|KIND0=0|VX0=0mil|VY0=0mil|KIND1=0|VX1=1000mil|VY1=0mil|KIND2=0|VX2=1000mil|VY2=1000mil|KIND3=0|VX3=0mil|VY3=1000mil";
        let bodies = [
            body(0, "|MODEL.MODELTYPE=1|MODEL.EMBED=TRUE|MODELID={ab-1}|MODEL.NAME=Part.STEP|MODEL.2D.X=500mil|MODEL.2D.Y=500mil|MODEL.3D.ROTZ=0|MODEL.3D.DZ=0mil|BODYPROJECTION=1", &[(450.0, 450.0), (550.0, 450.0), (550.0, 550.0), (450.0, 550.0)]),
            body(0, "|MODEL.MODELTYPE=0|STANDOFFHEIGHT=0mil|OVERALLHEIGHT=40mil|BODYPROJECTION=1|BODYCOLOR3D=8421504", &[(450.0, 450.0), (550.0, 450.0), (550.0, 550.0)]),
            body(0, "|MODEL.MODELTYPE=1|MODEL.EMBED=FALSE|MODEL.NAME=linked.step|MODEL.2D.X=500mil|MODEL.2D.Y=500mil|BODYPROJECTION=1", &[]),
        ].concat();
        let bytes = cfb(&[
            ("Board6/Data", text_rec(board6)),
            ("Rules6/Data", [rule("|RULEKIND=SolderMaskExpansion|SCOPE1EXPRESSION=InNet('X')|SCOPE2EXPRESSION=All|PRIORITY=1|EXPANSION=9mil"), rule("|RULEKIND=SolderMaskExpansion|SCOPE1EXPRESSION=All|SCOPE2EXPRESSION=All|PRIORITY=2|EXPANSION=4mil")].concat()),
            ("Components6/Data", text_rec("|SOURCEDESIGNATOR=U1|PATTERN=SOIC8|X=500mil|Y=500mil|ROTATION=90|LAYER=BOTTOM")),
            ("Vias6/Data", via(100.0, 100.0, 24.0, 12.0)),
            ("ComponentBodies6/Data", bodies),
            ("Models/Data", text_rec("|EMBED=TRUE|ID={AB-1}|NAME=Part.STEP|ROTX=0|ROTY=0|ROTZ=0|DZ=0")),
            ("Models/0", zlib(STEP)),
        ]);
        let d = crate::plat::tmp::dir("alt-board");
        let b = board(&bytes, &d).expect("a board");
        assert!((b.stack.thickness - ((1.4 * 2.0 + 60.0) * 0.0254 + 0.02)).abs() < 1e-3, "{}", b.stack.thickness);
        assert!((b.stack.mask_margin - 0.1016).abs() < 1e-9, "the board's mask rule: {}", b.stack.mask_margin);
        assert!(b.outline.len() == 1 && matches!(&b.outline[0], Shape::Poly { pts, .. } if pts.len() == 4), "{:?}", b.outline);
        assert_eq!(b.vias.len(), 1);
        let v = b.vias[0]; assert!((v.c.x - 2.54).abs() < 1e-6 && (v.c.y + 2.54).abs() < 1e-6 && (v.drill - 0.3048).abs() < 1e-6 && v.tented_top && !v.tented_bottom, "{v:?}");
        let fp = &b.footprints[0];
        assert_eq!((fp.reference.as_str(), fp.side, fp.rot), ("U1", Side::Bottom, 90.0));
        assert!((fp.at.x - 12.7).abs() < 1e-9 && (fp.at.y + 12.7).abs() < 1e-9);
        assert_eq!(fp.models.len(), 3, "{:?}", fp.models);
        let step = PathBuf::from(&fp.models[0].path);
        assert_eq!(std::fs::read(&step).unwrap(), STEP, "the model inflated beside the others");
        assert!(step.starts_with(&*d) && fp.models[0].offset.iter().all(|x| x.abs() < 1e-9));
        assert!(fp.models[1].path.ends_with(".wrl"));
        assert_eq!(fp.models[2].path, "linked.step");
        assert_eq!(fp.courtyard.len(), 2, "each body's outline (an empty one is none)");
        // the plan: the embedded model and the prism resolve, the linked one is not found
        let env = models::Env::empty();
        let plan = stream::plan(&b, (1.6, 0.0), &|raw: &str| models::resolve(raw, Path::new("/nonexistent"), &env));
        assert_eq!((plan.uses.len(), plan.unresolved.as_slice()), (2, &["linked.step".to_string()][..]));
        // the board meshes
        assert!(!crate::files::kicad::pcb3d::mesh::layers(&b).is_empty());
        assert!(board(b"not a board", &d).is_err());
    }

    // ---- the private corpus beside KiCad's own import --------------------------------------------------------------------------

    type Mx = [[f64; 4]; 3];
    /// The wire's instance back to KiCad's 3D space (mm, y up): W^-1 o W with W = diag(100, -100, 100)
    fn unwire(o: &[f32; 12]) -> Mx {
        let s = [1.0, -1.0, 1.0];
        let mut m = [[0.0; 4]; 3];
        for r in 0..3 { for c in 0..3 { m[r][c] = o[4 * r + c] as f64 * s[r] * s[c] } m[r][3] = o[4 * r + 3] as f64 * s[r] / 100.0 }
        m
    }
    /// Each footprint's shown models as (reference, content key, placement)
    fn placed(b: &Board, resolve: &dyn Fn(&str) -> Option<PathBuf>) -> Vec<(String, String, Mx)> {
        let seat = (b.stack.thickness, 0.0);
        let mut out = vec![];
        for fp in &b.footprints { for m in fp.models.iter().filter(|m| !m.hide) {
            let Some(p) = resolve(&m.path) else { continue };
            if p.extension().map_or(false, |e| e.eq_ignore_ascii_case("wrl")) { continue }
            out.push((fp.reference.clone(), models::key(&p).unwrap_or_default(), unwire(&stream::instance(fp, m, seat))));
        } }
        out
    }
    /// BP_ALTIUM_CORPUS and BP_KICAD_CLI: every corpus board read here and through KiCad's importer; the parts' models must be the same
    /// files standing in the same places (positions to 10 um, turns to 1e-3), the board as thick, with as many pads, vias and holes.
    /// BP_ALTIUM_3D_OUT=<dir>: pictures of both (parts meshed with BACKPLANE_STEP2GLB or FreeCAD, cached in <dir>/parts).
    #[test] fn corpus_against_kicads_import() {
        let Ok(cli) = std::env::var("BP_KICAD_CLI") else { return };
        let out = std::env::var("BP_ALTIUM_3D_OUT").ok().map(PathBuf::from);
        let only = std::env::var("BP_ALTIUM_3D_ONLY").unwrap_or_default();
        for f in super::super::corpus_boards() {
            if !f.to_string_lossy().contains(only.as_str()) { continue }
            let name = f.file_stem().unwrap().to_string_lossy().to_string();
            let tmp = crate::plat::tmp::dir("alt3d-ref");
            let bytes = std::fs::read(&f).unwrap();
            let t = std::time::Instant::now();
            let ours = board(&bytes, &tmp.join("models")).expect("our board");
            let ms = t.elapsed().as_millis();
            let k = super::super::kicad_board_in(&cli, &f.to_string_lossy(), &tmp.join("altium")).expect("KiCad's import");
            let kbytes = std::fs::read(&k).unwrap();
            let theirs = crate::files::kicad::pcb3d::parse::board(&kbytes).unwrap();
            let emb = models::embedded(&kbytes, &tmp.join("emb"));
            let env = models::Env::empty();
            let dir = f.parent().unwrap().to_path_buf();
            let a = placed(&ours, &|r: &str| models::resolve(r, &dir, &env));
            let b = placed(&theirs, &|r: &str| models::resolve_in(r, &dir, &env, &emb));
            // KiCad's import moves the board onto its page: the outlines' corners say by how much (KiCad's 3D space: y up)
            let corner = |b: &Board| { let (sh, _) = crate::files::kicad::pcb3d::mesh::outline(b); geom::bounds(&geom::flat(&sh)).map(|(x0, _, _, y1)| (x0, y1)).unwrap_or_default() };
            let (oc, kc) = (corner(&ours), corner(&theirs));
            let shift = [kc.0 - oc.0, -(kc.1 - oc.1), 0.0];
            let a: Vec<(String, String, Mx)> = a.into_iter().map(|(r, k, mut m)| { for j in 0..3 { m[j][3] += shift[j] } (r, k, m) }).collect();
            // each of KiCad's models matched to one of ours: the same file, the nearest place
            let mut used = vec![false; a.len()];
            let (mut matched, mut errs, mut rot_err, mut far) = (0, vec![], 0f64, vec![]);
            for (r, key, m) in &b {
                let best = a.iter().enumerate().filter(|(i, x)| !used[*i] && &x.1 == key)
                    .map(|(i, x)| (i, (0..3).map(|j| (x.2[j][3] - m[j][3]).powi(2)).sum::<f64>().sqrt(), x)).min_by(|p, q| p.1.total_cmp(&q.1));
                let Some((i, d, x)) = best else { far.push(format!("{r} (no model of ours)")); continue };
                used[i] = true; matched += 1; errs.push(d);
                let re = (0..3).flat_map(|j| (0..3).map(move |c| (j, c))).map(|(j, c)| (x.2[j][c] - m[j][c]).abs()).fold(0.0, f64::max);
                rot_err = rot_err.max(re);
                if d > 0.01 || re > 1e-3 { far.push(format!("{r} off {d:.3} mm, turn {re:.3}")) }
            }
            errs.sort_by(f64::total_cmp);
            let p = |q: f64| errs.get(((errs.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(0.0);
            let prisms = ours.footprints.iter().flat_map(|f| &f.models).filter(|m| m.path.ends_with(".wrl")).count();
            eprintln!("ALT3D {name}: read {ms} ms; thick {:.3}/{:.3} mm; pads {}/{} vias {}/{} holes {}/{} zones {}/{} outline {}/{} parts {}/{}",
                ours.stack.thickness, theirs.stack.thickness, ours.pads.len(), theirs.pads.len(), ours.vias.len(), theirs.vias.len(), ours.holes.len(), theirs.holes.len(),
                ours.zones.len(), theirs.zones.len(), ours.outline.len(), theirs.outline.len(), ours.footprints.len(), theirs.footprints.len());
            eprintln!("  STEP models ours {} kicad {}: matched {matched}, place error median {:.4} p95 {:.4} max {:.4} mm, turn error max {rot_err:.2e}; prisms {prisms}", a.len(), b.len(), p(0.5), p(0.95), p(1.0));
            if std::env::var_os("BP_ALTIUM_3D_DEBUG").is_some() {
                for fp in &theirs.footprints { if fp.models.is_empty() { continue } eprintln!("  K {} at {:.3},{:.3} rot {} {:?} {:?}", fp.reference, fp.at.x, fp.at.y, fp.rot, fp.side, fp.models.iter().map(|m| (m.offset, m.rotate)).collect::<Vec<_>>()) }
                for fp in &ours.footprints { if fp.models.is_empty() { continue } eprintln!("  O {} at {:.3},{:.3} rot {} {:?} {:?}", fp.reference, fp.at.x, fp.at.y, fp.rot, fp.side, fp.models.iter().map(|m| (m.offset, m.rotate)).collect::<Vec<_>>()) }
            }
            if !far.is_empty() { eprintln!("  apart: {}", far.iter().take(12).cloned().collect::<Vec<_>>().join("; ")) }
            assert!((ours.stack.thickness - theirs.stack.thickness).abs() < 0.05, "{name}: thickness");
            assert!(matched as f64 >= 0.98 * b.len() as f64, "{name}: {matched} of {} models matched", b.len());
            assert!(p(0.95) < 0.01, "{name}: placements");
            if let Some(o) = &out { picture(&ours, &dir, &env, &HashMap::new(), o, &format!("{name}-ours")); picture(&theirs, &dir, &env, &emb, o, &format!("{name}-kicad")) }
        }
    }
    /// The board and its parts drawn from a few sides into `o/<tag>.<view>.png`
    fn picture(b: &Board, dir: &Path, env: &models::Env, emb: &HashMap<String, PathBuf>, o: &Path, tag: &str) {
        use crate::files::kicad::{glb, mesh as render, pcb3d, png, solid, stepglb};
        let _ = std::fs::create_dir_all(o);
        let layers = pcb3d::mesh::layers(b);
        let seat = (b.stack.thickness, 0.0);
        // the layers' board axes (y down) to KiCad's 3D space (y up): a mirror, so each triangle's corners turn the other way
        let up = |p: [f32; 3]| [p[0], -p[1], p[2]];
        let mut ts: Vec<glb::Tri3> = layers.iter().flat_map(|l| l.idx.chunks_exact(3).map(move |t| glb::Tri3 { a: up(l.pos[t[0] as usize]), b: up(l.pos[t[2] as usize]), c: up(l.pos[t[1] as usize]), color: l.color })).collect();
        let plan = stream::plan(b, seat, &|r: &str| models::resolve_in(r, dir, env, emb));
        let cache = models::Cache::new(o.join("parts"));
        let tools = stepglb::machine(o);
        let jobs: Vec<(String, PathBuf)> = plan.uses.iter().filter_map(|u| models::key(&u.path).ok().map(|k| (k, u.path.clone()))).collect();
        let got = std::sync::Mutex::new(HashMap::new());
        models::convert(&cache, jobs, &tools, &std::sync::atomic::AtomicBool::new(false), |k: &str, r: Result<PathBuf, String>| { got.lock().unwrap().insert(k.to_string(), r); });
        let got = got.into_inner().unwrap();
        let (mut ok, mut bad) = (0, 0);
        for u in &plan.uses {
            let Some(Ok(g)) = models::key(&u.path).ok().and_then(|k| got.get(&k)) else { bad += u.at.len(); continue };
            let Ok(mt) = glb::tris(&std::fs::read(g).unwrap_or_default()) else { continue };
            for at in &u.at {
                ok += 1;
                let m = unwire(at);
                let tf = |v: [f32; 3]| { let v = v.map(|x| x as f64); std::array::from_fn(|r| (m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2] + m[r][3]) as f32) };
                ts.extend(mt.iter().map(|t| glb::Tri3 { a: tf(t.a), b: tf(t.b), c: tf(t.c), color: t.color }));
            }
        }
        eprintln!("  picture {tag}: {ok} parts drawn, {bad} not converted, {} boxes, {} triangles", plan.boxes.len(), ts.len());
        solid::upright(&mut ts);
        let (lo, hi) = render::bounds(&ts); let cam = render::fit(lo, hi);
        for (view, yaw, pitch) in [("iso", 0.6f32, 0.75f32), ("top", 0.0, 1.5707), ("bottom", 0.0, -1.5707), ("low", 0.3, 0.25)] {
            let (w, h) = (1600, 1200);
            let _ = std::fs::write(o.join(format!("{tag}.{view}.png")), png::rgb(&render::render(w, h, &ts, &cam.face(yaw, pitch), render::GROUND), w, h));
        }
    }
}

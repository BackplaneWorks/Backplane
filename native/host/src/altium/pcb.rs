//! An Altium board (.PcbDoc) as the items a .kicad_pcb makes (kicad/board.rs): the same paint layers, keys, signatures and
//! info strings, so the board viewer, cross-probe, diff and phones draw it with no change.
//!
//! A board keeps one storage per object kind, each with a `Data` stream: text records (`Board6`, `Nets6`, `Components6`,
//! `Polygons6`) and binary ones (`Tracks6`, `Arcs6`, `Vias6`, `Pads6`, `Fills6`, `Texts6`, `Regions6`, `ShapeBasedRegions6`).
//! A binary record is a type byte, then sub-records (u32 length + bytes). Coordinates are i32 in 1/10000 mil with Y up;
//! items are millimetres with Y down (KiCad's). Field offsets follow KiCad's importer notes and altium_parser_pcb (read
//! as documentation of the format; this is an independent implementation).
//!
//! Layers: copper follows Board6's stack (TOP, then each layer's NEXT, to BOTTOM); the inner ones, mid layers and planes
//! alike, take In1..In6 in stack order (more than six share slots, spread evenly: the viewer has six inner layers).
//! Mechanical layers go by their kind or name (assembly → Fab, courtyard → CrtYd, notes → Cmts.User), else Dwgs.User.
//! An internal plane is negative copper: drawn as a faint tint of the board outline with its split lines and regions.
use super::{pcb_text_records, Bin, Doc, Props};
use crate::files::kicad::board::{self, kv, Info, Item, Prim, Pt};
use crate::files::kicad::strokefont;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

pub struct Comp { pub refdes: String, pub footprint: String, pub x_mm: f32, pub y_mm: f32, pub rot: f32, pub bottom: bool, pub value: String }

pub(super) const NONE: u16 = 0xFFFF;
pub(super) const MULTI: u32 = 74;
const KEEPOUT: u32 = 56;
const V7_MECH: u32 = 0x0102_0000;

// KiCad paint ids (board.rs LAYERS)
const B_FAB: u32 = 0;
const B_CRTYD: u32 = 1;
const B_PASTE: u32 = 2;
pub(super) const B_SILK: u32 = 3;
pub(super) const B_MASK: u32 = 4;
pub(super) const B_CU: u32 = 5;
pub(super) const F_CU: u32 = 12;
pub(super) const PADS: u32 = 13;
pub(super) const F_MASK: u32 = 14;
const F_PASTE: u32 = 15;
pub(super) const F_SILK: u32 = 16;
const F_CRTYD: u32 = 17;
const F_FAB: u32 = 18;
const DWGS: u32 = 19;
const CMTS: u32 = 20;
const MARGIN: u32 = 21;
pub(super) const EDGE: u32 = board::EDGE;
const VIA: u32 = 23;
const HOLE: u32 = 24;
const KNAMES: [&str; 25] = ["B.Fab", "B.CrtYd", "B.Paste", "B.SilkS", "B.Mask", "B.Cu", "In6.Cu", "In5.Cu", "In4.Cu", "In3.Cu", "In2.Cu", "In1.Cu", "F.Cu", "*.Cu", "F.Mask", "F.Paste", "F.SilkS", "F.CrtYd", "F.Fab", "Dwgs.User", "Cmts.User", "Margin", "Edge.Cuts", "via", "hole"];
fn kname(l: u32) -> &'static str { KNAMES.get(l as usize).copied().unwrap_or("Dwgs.User") }
fn is_cu(l: u32) -> bool { (B_CU..=PADS).contains(&l) }

// Units: Altium's internal 1/10000 mil, its text "mil" numbers, and its Y-up plane, to KiCad's millimetres and Y down
fn mm(v: i32) -> f64 { v as f64 * 2.54e-6 }
fn milmm(m: f64) -> f64 { m * 0.0254 }
fn kp(x: f64, y: f64) -> Pt { Pt { x: x as f32, y: -y as f32 } }
fn show(x: f64) -> String { let s = format!("{:.4}", x); s.trim_end_matches('0').trim_end_matches('.').to_string() }

/// The streams a board read needs, by storage name ("Tracks6" for "Tracks6/Data").
pub struct Src(pub HashMap<String, Vec<u8>>);
const STORES: [&str; 14] = ["Board6", "Nets6", "Components6", "Polygons6", "Tracks6", "Arcs6", "Vias6", "Pads6", "Fills6", "Texts6", "Regions6", "ShapeBasedRegions6", "WideStrings6", "ComponentBodies6"];
impl Src {
    pub fn of(bytes: &[u8]) -> Option<Src> { Src::of_doc(&mut Doc::open(bytes.to_vec())?) }
    pub fn of_doc(d: &mut Doc) -> Option<Src> {
        let mut m = HashMap::new();
        for s in STORES { if let Some(b) = d.stream(&format!("{s}/Data")) { m.insert(s.to_string(), b); } }
        if !m.contains_key("Board6") { return None }
        Some(Src(m))
    }
    pub(super) fn get(&self, s: &str) -> &[u8] { self.0.get(s).map(|v| v.as_slice()).unwrap_or(&[]) }
    pub(super) fn text(&self, s: &str) -> Vec<Props> { pcb_text_records(self.get(s)) }
}

/// A binary stream's records: (index, type, whole record bytes, its sub-records). Stops at the first record that does not fit.
pub(super) fn records(b: &[u8], ty: u8, nsub: usize) -> Vec<(usize, &[u8], Vec<&[u8]>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 5 <= b.len() {
        if b[i] != ty { break }
        let start = i;
        i += 1;
        let mut subs = Vec::with_capacity(nsub);
        let mut ok = true;
        for _ in 0..nsub {
            if i + 4 > b.len() { ok = false; break }
            let n = u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
            i += 4;
            if i + n > b.len() { ok = false; break }
            subs.push(&b[i..i + n]);
            i += n;
        }
        if !ok { break }
        out.push((out.len(), &b[start..i], subs));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Layers

struct StackLayer { id: u32, name: String, next: u32, copthick: f64, dielheight: f64, mech_on: bool, mech_kind: String }
pub(super) struct Layers { stack: Vec<StackLayer>, paint: HashMap<u32, Option<u32>>, pub thick_um: u32, planes: Vec<u32> }

fn default_name(a: u32) -> String {
    match a {
        1 => "Top Layer".into(), 2..=31 => format!("Mid-Layer {}", a - 1), 32 => "Bottom Layer".into(), 33 => "Top Overlay".into(), 34 => "Bottom Overlay".into(),
        35 => "Top Paste".into(), 36 => "Bottom Paste".into(), 37 => "Top Solder".into(), 38 => "Bottom Solder".into(), 39..=54 => format!("Internal Plane {}", a - 38),
        55 => "Drill Guide".into(), 56 => "Keep-Out Layer".into(), 57..=72 => format!("Mechanical {}", a - 56), 73 => "Drill Drawing".into(), 74 => "Multi-Layer".into(),
        _ if a > V7_MECH && a < V7_MECH + 0x10000 => format!("Mechanical {}", a - V7_MECH),
        _ => format!("Layer {a}"),
    }
}
fn is_mech(a: u32) -> bool { (57..=72).contains(&a) || (a > V7_MECH && a < V7_MECH + 0x10000) }
fn is_plane(a: u32) -> bool { (39..=54).contains(&a) }

/// A mechanical layer's paint by its kind (LayerKindMapping's names) or, failing that, its name.
fn mech_paint(kind: &str, name: &str, has_outline: bool) -> u32 {
    match kind {
        "AssemblyTop" | "DimensionsTop" | "ValueTop" | "DesignatorTop" | "ComponentOutlineTop" | "ComponentCenterTop" | "3DBodyTop" => return F_FAB,
        "AssemblyBottom" | "DimensionsBottom" | "ValueBottom" | "DesignatorBottom" | "ComponentOutlineBottom" | "ComponentCenterBottom" | "3DBodyBottom" => return B_FAB,
        "CourtyardTop" => return F_CRTYD,
        "CourtyardBottom" => return B_CRTYD,
        "AssemblyNotes" | "FabNotes" => return CMTS,
        "Dimensions" => return DWGS,
        "Board" | "BoardShape" | "VCut" => return if has_outline { DWGS } else { EDGE },
        _ => {}
    }
    let n = name.to_ascii_lowercase();
    let bottom = n.contains("bot");
    if n.contains("courtyard") { return if bottom { B_CRTYD } else { F_CRTYD } }
    if n.contains("assembly") || n.contains("3d body") || n.contains("component outline") { return if bottom { B_FAB } else { F_FAB } }
    DWGS
}

impl Layers {
    fn of(b: &Props, has_outline: bool) -> Layers {
        let mut stack = Vec::new();
        let read = |p: &str, id: u32| StackLayer {
            id, name: b.get(&format!("{p}NAME")).to_string(), next: b.int(&format!("{p}NEXT")) as u32,
            copthick: if b.has(&format!("{p}COPTHICK")) { b.num(&format!("{p}COPTHICK")) } else { 1.4 },
            dielheight: if b.has(&format!("{p}DIELHEIGHT")) { b.num(&format!("{p}DIELHEIGHT")) } else { 60.0 },
            mech_on: !b.get(&format!("{p}MECHENABLED")).contains("FALSE"), mech_kind: b.get(&format!("{p}MECHKIND")).to_string(),
        };
        for i in 1..=100 {
            let p = format!("LAYER{i}");
            if !b.has(&format!("{p}NAME")) { continue }
            let id = if b.has(&format!("{p}LAYERID")) { b.int(&format!("{p}LAYERID")) as u32 } else { i };
            stack.push(read(&p, id));
        }
        for i in 0.. {
            let p = format!("LAYERV7_{i}");
            if !b.has(&format!("{p}NAME")) { break }
            let id = b.int(&format!("{p}LAYERID")) as u32;
            stack.push(read(&p, id));
        }
        // the copper stack: TOP and each layer's NEXT (legacy LAYERn records are indexed by layer id)
        let by_id: HashMap<u32, usize> = stack.iter().enumerate().filter(|(_, l)| l.id < 0x100).map(|(i, l)| (l.id, i)).collect();
        let mut copper = Vec::new();
        let (mut id, mut thick) = (1u32, 0.0f64);
        while let Some(&i) = by_id.get(&id) {
            if copper.contains(&id) || copper.len() > 64 { break }
            copper.push(id);
            let l = &stack[i];
            thick += l.copthick;
            if l.next == 0 || id == 32 { break }
            thick += l.dielheight;
            id = l.next;
        }
        if copper.len() < 2 { copper = vec![1, 32]; thick = 0.0 }
        let mut paint: HashMap<u32, Option<u32>> = HashMap::new();
        let inner = &copper[1..copper.len() - 1];
        let n = inner.len();
        for (k, a) in inner.iter().enumerate() {
            let slot = if n <= 6 { k + 1 } else { 1 + k * 6 / n };
            paint.insert(*a, Some(F_CU - slot as u32));
        }
        paint.insert(copper[0], Some(F_CU));
        paint.insert(*copper.last().unwrap(), Some(B_CU));
        for l in &stack {
            if is_mech(l.id) { paint.insert(l.id, if l.mech_on { Some(mech_paint(&l.mech_kind, &l.name, has_outline)) } else { None }); }
        }
        let planes = copper.iter().copied().filter(|a| is_plane(*a)).collect();
        // copper and dielectric, and a solder mask each side (10 µm, as KiCad's import counts it)
        let thick_um = (thick * 25.4).round() as u32 + if thick > 0.0 { 20 } else { 0 };
        Layers { stack, paint, thick_um: if thick_um > 0 { thick_um } else { 1600 }, planes }
    }
    /// An Altium layer's paint id; None: not drawn.
    pub(super) fn paint(&self, a: u32) -> Option<u32> {
        if let Some(p) = self.paint.get(&a) { return *p }
        Some(match a {
            1 => F_CU, 32 => B_CU, 2..=31 => F_CU - (a - 1).min(6), 39..=54 => F_CU - (a - 38).min(6),
            33 => F_SILK, 34 => B_SILK, 35 => F_PASTE, 36 => B_PASTE, 37 => F_MASK, 38 => B_MASK,
            55 | 73 => DWGS, KEEPOUT => MARGIN, MULTI => PADS,
            _ if is_mech(a) => DWGS,
            _ => return None,
        })
    }
    /// The outer copper's thickness, mm (the top layer's, else 1.4 mil)
    pub fn copper_mm(&self) -> f64 { self.stack.iter().find(|l| l.id == 1).map_or(1.4, |l| l.copthick) * 0.0254 }
    fn name(&self, a: u32) -> String {
        self.stack.iter().find(|l| l.id == a).map(|l| l.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| default_name(a))
    }
}
fn versioned(v6: u8, v7: u32) -> u32 { if v7 > V7_MECH + 16 && v7 < V7_MECH + 0x10000 { v7 } else { v6 as u32 } }

// ---------------------------------------------------------------------------------------------------------------------
// Geometry (Altium millimetres, Y up, until kp)

#[derive(Clone, Copy, Debug)]
struct Vtx { round: bool, x: f64, y: f64, cx: f64, cy: f64, r: f64, sa: f64, ea: f64 }

/// Points along a circle from angle a0 (degrees, counter-clockwise, Y up) through `sweep` degrees.
fn arc_pts(cx: f64, cy: f64, r: f64, a0: f64, sweep: f64) -> Vec<(f64, f64)> {
    let n = ((sweep.abs() / 7.5).ceil() as usize).clamp(2, 96);
    (0..=n).map(|i| { let a = (a0 + sweep * i as f64 / n as f64).to_radians(); (cx + r * a.cos(), cy + r * a.sin()) }).collect()
}
fn sweep_of(sa: f64, ea: f64) -> f64 { let mut s = (ea - sa) % 360.0; if s <= 0.0 { s += 360.0 } s }
/// An outline's vertices to a point ring: an arc vertex runs its arc from the end nearer its point.
fn ring(vs: &[Vtx]) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(vs.len());
    for v in vs {
        if v.round && v.r > 0.0 {
            let sw = sweep_of(v.sa, v.ea);
            let (s, e) = ((v.cx + v.r * v.sa.to_radians().cos(), v.cy + v.r * v.sa.to_radians().sin()), (v.cx + v.r * v.ea.to_radians().cos(), v.cy + v.r * v.ea.to_radians().sin()));
            let ds = (s.0 - v.x).hypot(s.1 - v.y);
            let de = (e.0 - v.x).hypot(e.1 - v.y);
            if ds <= de { out.extend(arc_pts(v.cx, v.cy, v.r, v.sa, sw)) } else { out.extend(arc_pts(v.cx, v.cy, v.r, v.ea, -sw)) }
        } else { out.push((v.x, v.y)) }
    }
    out
}
fn area2(p: &[(f64, f64)]) -> f64 { let n = p.len(); (0..n).map(|i| { let (a, b) = (p[i], p[(i + 1) % n]); a.0 * b.1 - b.0 * a.1 }).sum() }

/// A polygon with holes as one ring, each hole joined to the outline by a zero-width cut (KiCad's filled polygons are
/// stored the same way): holes in order of their rightmost point, each bridged rightwards to the nearest edge its ray
/// meets (the outline's or an earlier hole's). The ring is a linked list, its edges binned in horizontal bands, so a pour
/// with thousands of clearance holes stays near linear.
fn fracture(outer: Vec<(f64, f64)>, holes: Vec<Vec<(f64, f64)>>) -> Vec<(f64, f64)> {
    use std::cmp::Ordering::Equal;
    if outer.len() < 3 { return outer }
    let ccw = area2(&outer) > 0.0;
    let mut hs: Vec<Vec<(f64, f64)>> = holes.into_iter().filter(|h| h.len() >= 3).collect();
    if hs.is_empty() { return outer }
    for h in hs.iter_mut() { if (area2(h) > 0.0) == ccw { h.reverse() } }
    let maxx = |h: &Vec<(f64, f64)>| h.iter().map(|p| p.0).fold(f64::MIN, f64::max);
    hs.sort_by(|a, b| maxx(b).partial_cmp(&maxx(a)).unwrap_or(Equal));
    let total = outer.len() + hs.iter().map(|h| h.len() + 2).sum::<usize>();
    let (y0, y1) = outer.iter().chain(hs.iter().flatten()).fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
    let nb = (total / 4).clamp(1, 4096);
    let bh = ((y1 - y0) / nb as f64).max(1e-12);
    let band = |y: f64| (((y - y0) / bh).floor().max(0.0) as usize).min(nb - 1);
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(total);
    let mut next: Vec<usize> = Vec::with_capacity(total);
    let mut bands: Vec<Vec<usize>> = vec![Vec::new(); nb];
    let index = |bands: &mut Vec<Vec<usize>>, pts: &Vec<(f64, f64)>, next: &Vec<usize>, i: usize| {
        let (a, b) = (pts[i], pts[next[i]]);
        for k in band(a.1.min(b.1))..=band(a.1.max(b.1)) { bands[k].push(i) }
    };
    let n0 = outer.len();
    for (i, p) in outer.into_iter().enumerate() { pts.push(p); next.push((i + 1) % n0) }
    for i in 0..n0 { index(&mut bands, &pts, &next, i) }
    for h in hs {
        let mi = (0..h.len()).max_by(|&a, &b| h[a].0.partial_cmp(&h[b].0).unwrap_or(Equal)).unwrap_or(0);
        let m = h[mi];
        // the nearest edge the ray to +x from m crosses (stale band entries are edges elsewhere: the test drops them)
        let mut best: Option<(f64, usize)> = None;
        for &i in &bands[band(m.1)] {
            let (a, b) = (pts[i], pts[next[i]]);
            if (a.1 > m.1) == (b.1 > m.1) { continue }
            let x = a.0 + (m.1 - a.1) * (b.0 - a.0) / (b.1 - a.1);
            if x < m.0 { continue }
            if best.map(|(bx, _)| x < bx).unwrap_or(true) { best = Some((x, if a.0 >= b.0 { i } else { next[i] })) }
        }
        let j = match best {
            Some((_, j)) => j,
            None => (0..pts.len()).min_by(|&a, &b| { let d = |i: usize| (pts[i].0 - m.0).hypot(pts[i].1 - m.1); d(a).partial_cmp(&d(b)).unwrap_or(Equal) }).unwrap_or(0),
        };
        // j → hole from m round to m again → j's copy → j's old next
        let after = next[j];
        let first = pts.len();
        let k = h.len();
        for t in 0..=k { pts.push(h[(mi + t) % k]); next.push(pts.len()) }
        let jc = pts.len();
        pts.push(pts[j]);
        next.push(after);
        next[j] = first;
        for i in first..=jc { index(&mut bands, &pts, &next, i) }
        index(&mut bands, &pts, &next, j);
    }
    let mut out = Vec::with_capacity(pts.len());
    let mut i = 0;
    for _ in 0..pts.len() { out.push(pts[i]); i = next[i]; if i == 0 { break } }
    out
}
fn kring(ps: &[(f64, f64)]) -> Vec<Pt> { ps.iter().map(|p| kp(p.0, p.1)).collect() }

/// A frame: origin, rotation (degrees, counter-clockwise), in Altium millimetres
#[derive(Clone, Copy)]
pub(super) struct Fr { x: f64, y: f64, c: f64, s: f64 }
impl Fr {
    fn new(x: f64, y: f64, deg: f64) -> Fr { let a = deg.to_radians(); Fr { x, y, c: a.cos(), s: a.sin() } }
    fn at(&self, u: f64, v: f64) -> (f64, f64) { (self.x + u * self.c - v * self.s, self.y + u * self.s + v * self.c) }
    fn k(&self, u: f64, v: f64) -> Pt { let (x, y) = self.at(u, v); kp(x, y) }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Shape { Round, Rect, Oct, RoundRect(f64) }
/// A pad shape of size w×h about the frame's origin.
fn pad_prim(fr: &Fr, w: f64, h: f64, shape: Shape) -> Prim {
    let (hw, hh) = (w / 2.0, h / 2.0);
    let poly = |ps: Vec<(f64, f64)>| Prim::Poly { ps: ps.into_iter().map(|(u, v)| fr.k(u, v)).collect(), w: 0.0, fill: true };
    match shape {
        Shape::Round if (w - h).abs() < 1e-9 => Prim::Circ { c: fr.k(0.0, 0.0), r: hw as f32, w: 0.0, fill: true },
        Shape::Round => if w > h { Prim::Seg { a: fr.k(-(hw - hh), 0.0), b: fr.k(hw - hh, 0.0), w: h as f32 } } else { Prim::Seg { a: fr.k(0.0, -(hh - hw)), b: fr.k(0.0, hh - hw), w: w as f32 } },
        Shape::Rect => poly(vec![(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)]),
        Shape::Oct => { let c = 0.25 * w.min(h); poly(vec![(-hw + c, -hh), (hw - c, -hh), (hw, -hh + c), (hw, hh - c), (hw - c, hh), (-hw + c, hh), (-hw, hh - c), (-hw, -hh + c)]) }
        Shape::RoundRect(ratio) => {
            let r = (ratio * w.min(h)).clamp(0.0, w.min(h) / 2.0);
            if r <= 1e-6 { return pad_prim(fr, w, h, Shape::Rect) }
            let mut ps = Vec::with_capacity(28);
            for (cx, cy, a0) in [(hw - r, -hh + r, 270.0), (hw - r, hh - r, 0.0), (-hw + r, hh - r, 90.0), (-hw + r, -hh + r, 180.0)] {
                for i in 0..=6 { let a = (a0 + 15.0 * i as f64).to_radians(); ps.push((cx + r * a.cos(), cy + r * a.sin())) }
            }
            poly(ps)
        }
    }
}
fn shape_word(s: Shape, w: f64, h: f64) -> &'static str {
    match s { Shape::Round if (w - h).abs() < 1e-9 => "circle", Shape::Round => "oval", Shape::Rect => "rect", Shape::Oct => "chamfered_rect", Shape::RoundRect(_) => "roundrect" }
}

// ---------------------------------------------------------------------------------------------------------------------
// The board

pub(super) struct CompRec { pub refdes: String, pub footprint: String, pub value: String, pub x: f64, pub y: f64, pub rot: f64, pub bottom: bool, pub nameon: bool, pub commenton: bool, pub ctx: String }

pub(super) struct Ctx<'a> { src: &'a Src, pub layers: Layers, nets: Vec<String>, pub comps: Vec<CompRec>, wide: HashMap<u32, String>, outline: Vec<(f64, f64)>, used: BTreeSet<u32>, poured: BTreeSet<u16> }
impl<'a> Ctx<'a> {
    fn new(src: &'a Src) -> Ctx<'a> {
        let board = src.text("Board6").into_iter().next().unwrap_or_default();
        let outline = ring(&vertices(&board));
        let layers = Layers::of(&board, outline.len() >= 3);
        let nets = src.text("Nets6").iter().map(|p| p.get("NAME").to_string()).collect();
        let wide = wide_strings(src.get("WideStrings6"));
        let comps = src.text("Components6").iter().map(|p| {
            let mut r = p.get("SOURCEDESIGNATOR").to_string();
            if r.is_empty() { r = p.get("NAME").to_string() }
            let pat = p.get("PATTERN");
            let footprint = pat.rsplit(['\\', '/']).next().unwrap_or(pat).to_string();
            CompRec { refdes: r, footprint, value: p.get("COMMENT").to_string(), x: milmm(p.num("X")), y: milmm(p.num("Y")), rot: p.num("ROTATION"),
                bottom: p.get("LAYER").eq_ignore_ascii_case("BOTTOM"), nameon: !p.has("NAMEON") || p.bool("NAMEON"), commenton: p.bool("COMMENTON"), ctx: String::new() }
        }).collect();
        // polygons whose copper Regions6 holds: their track and arc pieces (an older pour's) are drawn by the regions
        let poured = records(src.get("Regions6"), 11, 1).iter().map(|(_, _, s)| { let mut r = Bin::new(s[0]); head(&mut r).polygon }).filter(|p| *p != NONE).collect();
        let mut c = Ctx { src, layers, nets, comps, wide, outline, used: BTreeSet::new(), poured };
        // a component's value: its comment text when the record has none
        for t in c.texts() {
            if let Some(ci) = t.comp { if t.is_comment && !t.text.starts_with('.') { if let Some(k) = c.comps.get_mut(ci) { if k.value.is_empty() { k.value = t.text.clone() } } } }
        }
        for k in c.comps.iter_mut() { k.ctx = format!("{}{}{}", kv("ref", &k.refdes), kv("value", &k.value), kv("footprint", &k.footprint)) }
        c
    }
    fn net(&self, n: u16) -> &str { if n == NONE { "" } else { self.nets.get(n as usize).map(|s| s.as_str()).unwrap_or("") } }
    fn ctx(&self, c: u16) -> &str { if c == NONE { "" } else { self.comps.get(c as usize).map(|k| k.ctx.as_str()).unwrap_or("") } }
    fn paint(&mut self, a: u32) -> Option<u32> { self.used.insert(a); self.layers.paint(a) }
}
fn vertices(p: &Props) -> Vec<Vtx> {
    let mut out = Vec::new();
    for i in 0.. {
        let (kx, ky) = (format!("VX{i}"), format!("VY{i}"));
        if !p.has(&kx) || !p.has(&ky) { break }
        let n = |k: &str| milmm(p.num(&format!("{k}{i}")));
        out.push(Vtx { round: p.int(&format!("KIND{i}")) != 0, x: n("VX"), y: n("VY"), cx: n("CX"), cy: n("CY"), r: n("R"), sa: p.num(&format!("SA{i}")), ea: p.num(&format!("EA{i}")) });
    }
    out
}
/// WideStrings6: (index u32, byte length u32, UTF-16LE with a NUL) entries
fn wide_strings(b: &[u8]) -> HashMap<u32, String> {
    let mut m = HashMap::new();
    let mut r = Bin::new(b);
    while r.left() >= 8 {
        let (i, n) = (r.u32(), r.u32() as usize);
        if n > r.left() { break }
        let s = r.bytes(n);
        let u: Vec<u16> = s.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
        m.insert(i, String::from_utf16_lossy(&u));
    }
    m
}

fn push(out: &mut Vec<Item>, key: String, sig: u32, layer: u32, prim: Prim, info: &Info) { out.push(Item { key, sig, layer, prim, info: info.clone() }) }
fn sig(b: &[u8]) -> u32 { board::hash(b, board::FNV) }

/// The common head of track, arc, fill, region, via and pad records: layer, flags, net, polygon, component.
struct Head { layer: u8, flags1: u8, flags2: u8, net: u16, polygon: u16, comp: u16 }
fn head(r: &mut Bin) -> Head { Head { layer: r.u8(), flags1: r.u8(), flags2: r.u8(), net: r.u16(), polygon: r.u16(), comp: r.u16() } }

fn graphic_info(c: &Ctx, kind: &str, layer: u32, net: u16, comp: u16, width: f64) -> Info {
    let k = if is_cu(layer) && matches!(kind, "gr_line" | "gr_arc" | "gr_circle") { "track" } else { kind };
    let w = if width > 0.0 { kv("width", &show(width)) } else { String::new() };
    Arc::from(format!("{k}{}{}{w}{}", kv("net", c.net(net)), kv("layer", kname(layer)), c.ctx(comp)))
}

fn tracks(c: &mut Ctx, out: &mut Vec<Item>) {
    let src = c.src;
    for (i, rec, subs) in records(src.get("Tracks6"), 4, 1) {
        let mut r = Bin::new(subs[0]);
        let h = head(&mut r);
        r.skip(4);
        let (x1, y1, x2, y2, w) = (mm(r.i32()), mm(r.i32()), mm(r.i32()), mm(r.i32()), mm(r.i32()));
        r.skip(3);
        let v7 = if subs[0].len() >= r.i + 9 { r.skip(4 + 1); r.u32() } else { 0 };
        let a = versioned(h.layer, v7);
        if c.poured.contains(&h.polygon) { continue }
        let keepout = h.flags2 == 2 || a == KEEPOUT;
        let Some(l) = c.paint(a).map(|l| if keepout { MARGIN } else { l }) else { continue };
        let info = graphic_info(c, if keepout { "keepout" } else if h.polygon < NONE - 1 { "zone" } else { "gr_line" }, l, h.net, h.comp, w);
        push(out, format!("t/{i}"), sig(rec), l, Prim::Seg { a: kp(x1, y1), b: kp(x2, y2), w: w as f32 }, &info);
    }
}

fn arcs(c: &mut Ctx, out: &mut Vec<Item>) {
    let src = c.src;
    for (i, rec, subs) in records(src.get("Arcs6"), 1, 1) {
        let mut r = Bin::new(subs[0]);
        let h = head(&mut r);
        r.skip(4);
        let (cx, cy, rad) = (mm(r.i32()), mm(r.i32()), mm(r.i32()));
        let (sa, ea) = (r.f64(), r.f64());
        let w = mm(r.i32());
        r.skip(2);
        let v7 = if subs[0].len() >= r.i + 9 { r.skip(1 + 4); r.u32() } else { 0 };
        let a = versioned(h.layer, v7);
        if c.poured.contains(&h.polygon) { continue }
        let keepout = h.flags2 == 2 || a == KEEPOUT;
        let Some(l) = c.paint(a).map(|l| if keepout { MARGIN } else { l }) else { continue };
        let full = (ea - sa).abs() >= 359.999 || (sa == 0.0 && ea == 360.0);
        let info = graphic_info(c, if keepout { "keepout" } else if h.polygon < NONE - 1 { "zone" } else if full { "gr_circle" } else { "gr_arc" }, l, h.net, h.comp, w);
        let prim = if full { Prim::Circ { c: kp(cx, cy), r: rad as f32, w: w as f32, fill: false } } else { Prim::Line { ps: kring(&arc_pts(cx, cy, rad, sa, sweep_of(sa, ea))), w: w as f32 } };
        push(out, format!("a/{i}"), sig(rec), l, prim, &info);
    }
}

fn fills(c: &mut Ctx, out: &mut Vec<Item>) {
    let src = c.src;
    for (i, rec, subs) in records(src.get("Fills6"), 6, 1) {
        let mut r = Bin::new(subs[0]);
        let h = head(&mut r);
        r.skip(4);
        let (x1, y1, x2, y2) = (mm(r.i32()), mm(r.i32()), mm(r.i32()), mm(r.i32()));
        let rot = r.f64();
        let v7 = if subs[0].len() >= r.i + 9 { r.skip(5); r.u32() } else { 0 };
        let a = versioned(h.layer, v7);
        let keepout = h.flags2 == 2 || a == KEEPOUT;
        let Some(l) = c.paint(a).map(|l| if keepout { MARGIN } else { l }) else { continue };
        let fr = Fr::new((x1 + x2) / 2.0, (y1 + y2) / 2.0, rot);
        let (hw, hh) = ((x2 - x1).abs() / 2.0, (y2 - y1).abs() / 2.0);
        let ps = vec![fr.k(-hw, -hh), fr.k(hw, -hh), fr.k(hw, hh), fr.k(-hw, hh)];
        let info = graphic_info(c, if keepout { "keepout" } else { "gr_poly" }, l, h.net, h.comp, 0.0);
        push(out, format!("f/{i}"), sig(rec), l, if keepout { Prim::Poly { ps, w: 0.1, fill: false } } else { Prim::Poly { ps, w: 0.0, fill: true } }, &info);
    }
}

/// A via record (Vias6): position, land and hole (mm, Altium's Y up), the copper layers it spans, its mask tenting.
pub(super) struct ViaRec { pub i: usize, pub sig: u32, pub net: u16, pub x: f64, pub y: f64, pub d: f64, pub hole: f64, pub from: u8, pub to: u8, pub tent_top: bool, pub tent_bottom: bool }
pub(super) fn via_recs(src: &Src) -> Vec<ViaRec> {
    records(src.get("Vias6"), 3, 1).into_iter().map(|(i, rec, subs)| {
        let mut r = Bin::new(subs[0]);
        let h = head(&mut r);
        r.skip(4);
        let (x, y, d, hole) = (mm(r.i32()), mm(r.i32()), mm(r.i32()), mm(r.i32()));
        let (from, to) = (r.u8(), r.u8());
        ViaRec { i, sig: sig(rec), net: h.net, x, y, d, hole, from, to, tent_top: h.flags1 & 0x20 != 0, tent_bottom: h.flags1 & 0x40 != 0 }
    }).collect()
}

fn vias(c: &mut Ctx, out: &mut Vec<Item>) {
    for v in via_recs(c.src) {
        let i = v.i;
        c.used.insert(MULTI);
        let info: Info = Arc::from(format!("via{}{}{}", kv("net", c.net(v.net)), kv("size", &show(v.d)), kv("drill", &show(v.hole))));
        push(out, format!("v/{i}"), v.sig, VIA, Prim::Circ { c: kp(v.x, v.y), r: (v.d / 2.0) as f32, w: 0.0, fill: true }, &info);
        if v.hole > 0.0 { push(out, format!("v/{i}/h"), v.sig, HOLE, Prim::Circ { c: kp(v.x, v.y), r: (v.hole / 2.0) as f32, w: 0.0, fill: true }, &info) }
    }
}

fn shape_of(b: u8, alt: u8, radius: u8) -> Shape {
    match b { 2 => Shape::Rect, 3 => Shape::Oct, 1 if alt == 9 => Shape::RoundRect(radius as f64 / 200.0), 1 => Shape::Round, _ => Shape::Rect }
}

/// A pad record (Pads6): its name, layer, net, part, position and turn, the copper's size and shape on top and bottom, its hole
/// (round, square or a slot, offset from the pad's centre) and its mask and paste.
pub(super) struct PadRec {
    pub i: usize, pub sig: u32, pub name: String, pub layer: u32, pub net: u16, pub comp: u16, pub x: f64, pub y: f64, pub dir: f64,
    pub top: (f64, f64), pub bot: (f64, f64), pub tshape: Shape, pub bshape: Shape, pub hole: f64, pub plated: bool,
    pub hole_shape: u8, pub slot: f64, pub slot_rot: f64, pub off: (f64, f64),
    /// mask and paste expansion when set by hand (0 when by rule), mm
    pub mask_x: f64, pub paste_x: f64, pub tent_top: bool, pub tent_bottom: bool,
}
pub(super) fn pad_recs(src: &Src) -> Vec<PadRec> {
    let mut out = vec![];
    for (i, rec, subs) in records(src.get("Pads6"), 2, 6) {
        let name = { let mut r = Bin::new(subs[0]); let n = r.u8() as usize; super::cp1252(r.bytes(n)) };
        let s5 = subs[4];
        if s5.len() < 110 { continue }
        let mut r = Bin::new(s5);
        let (layer, flags1) = (r.u8(), r.u8());
        r.skip(1);
        let net = r.u16();
        r.skip(2);
        let comp = r.u16();
        r.skip(4);
        let (x, y) = (mm(r.i32()), mm(r.i32()));
        let top = (mm(r.i32()), mm(r.i32()));
        let _mid = (mm(r.i32()), mm(r.i32()));
        let bot = (mm(r.i32()), mm(r.i32()));
        let hole = mm(r.i32());
        let (tshape, _mshape, bshape) = (r.u8(), r.u8(), r.u8());
        let dir = r.f64();
        let plated = r.u8() != 0;
        r.skip(1);
        let _mode = r.u8();
        r.skip(23);
        let (paste_x, mask_x) = (mm(r.i32()), mm(r.i32()));
        r.skip(7);
        let (paste_mode, mask_mode) = (r.u8(), r.u8());
        // sub-record 6: per-layer sizes, hole shape, slot, hole offsets, alternative shapes and corner radii
        let s6 = subs[5];
        let (mut hole_shape, mut slot, mut slot_rot, mut off, mut alt, mut rad) = (0u8, 0.0, 0.0, (0.0, 0.0), [0u8; 2], [0u8; 2]);
        if s6.len() >= 596 {
            let mut q = Bin::new(s6);
            q.skip(29 * 4 * 2 + 29 + 1);
            hole_shape = q.u8();
            slot = mm(q.i32());
            slot_rot = q.f64();
            let ox = mm(q.i32()); q.skip(31 * 4);
            let oy = mm(q.i32()); q.skip(31 * 4);
            off = (ox, oy);
            q.skip(1);
            let alts = q.bytes(32);
            let rads = q.bytes(32);
            alt = [alts[0], alts[31]];
            rad = [rads[0], rads[31]];
        }
        out.push(PadRec { i, sig: sig(rec), name, layer: layer as u32, net, comp, x, y, dir, top, bot, tshape: shape_of(tshape, alt[0], rad[0]), bshape: shape_of(bshape, alt[1], rad[1]),
            hole, plated, hole_shape, slot, slot_rot, off, mask_x: if mask_mode == 2 { mask_x } else { 0.0 }, paste_x: if paste_mode == 2 { paste_x } else { 0.0 },
            tent_top: flags1 & 0x20 != 0, tent_bottom: flags1 & 0x40 != 0 });
    }
    out
}
impl PadRec {
    pub(super) fn thru(&self) -> bool { self.hole > 0.0 }
    /// The frame the copper is drawn in: about the hole's offset for a through pad
    pub(super) fn copper_frame(&self) -> Fr {
        let sfr = Fr::new(self.x, self.y, self.dir);
        if self.thru() { let (ox, oy) = sfr.at(self.off.0, self.off.1); Fr::new(ox, oy, self.dir) } else { sfr }
    }
    /// The copper on one side, grown by `grow` mm each way (mask and paste openings); None when it has no size
    pub(super) fn copper(&self, bottom: bool, grow: f64) -> Option<Prim> {
        let (sz, sh) = if bottom { (self.bot, self.bshape) } else { (self.top, self.tshape) };
        let (w, h) = (sz.0 + 2.0 * grow, sz.1 + 2.0 * grow);
        (w > 0.0 && h > 0.0).then(|| pad_prim(&self.copper_frame(), w, h, sh))
    }
    /// The hole: a round one, a square one or a slot
    pub(super) fn hole_prim(&self) -> Prim {
        let hfr = Fr::new(self.x, self.y, self.dir + if self.hole_shape == 2 { self.slot_rot } else { 0.0 });
        match self.hole_shape {
            2 if self.slot > self.hole => pad_prim(&hfr, self.slot, self.hole, Shape::Round),
            1 => pad_prim(&hfr, self.hole, self.hole, Shape::Rect),
            _ => Prim::Circ { c: kp(self.x, self.y), r: (self.hole / 2.0) as f32, w: 0.0, fill: true },
        }
    }
}

fn pads(c: &mut Ctx, out: &mut Vec<Item>) {
    for p in pad_recs(c.src) {
        let Some(l) = c.paint(p.layer) else { continue };
        let thru = p.thru();
        let kind = if !thru { "smd" } else if p.plated { "thru_hole" } else { "np_thru_hole" };
        let info: Info = Arc::from(format!("pad {}{}{}{}{}", p.name, kv("net", c.net(p.net)), kv("type", kind), kv("shape", shape_word(p.tshape, p.top.0, p.top.1)), c.ctx(p.comp)));
        let (s, k) = (p.sig, format!("p/{}", p.i));
        let (side, bottom) = if l == B_CU { (B_CU, true) } else { (l, false) };
        // an unplated hole has no copper (KiCad's import gives it none)
        if !thru || p.plated { if let Some(prim) = p.copper(bottom, 0.0) { push(out, k.clone(), s, side, prim, &info) } }
        // mask and paste (hidden in a board view, shown by a Gerber comparison)
        let mut masks = vec![];
        if (l == F_CU || l == PADS) && !p.tent_top { masks.push((F_MASK, false, p.mask_x, "m")) }
        if (l == B_CU || l == PADS) && !p.tent_bottom { masks.push((B_MASK, true, p.mask_x, "bm")) }
        if !thru && l == F_CU { masks.push((F_PASTE, false, p.paste_x, "s")) }
        if !thru && l == B_CU { masks.push((B_PASTE, true, p.paste_x, "bs")) }
        for (ml, bot, e, tag) in masks { if let Some(prim) = p.copper(bot, e) { push(out, format!("{k}/{tag}"), s, ml, prim, &info) } }
        if thru { push(out, format!("{k}/h"), s, HOLE, p.hole_prim(), &info) }
    }
}

/// Regions6 (polygon pours' copper, plain vertices) and ShapeBasedRegions6 (arcs kept): one region record.
struct Region { h: Head, kind: i64, cutout: bool, v7: String, outline: Vec<(f64, f64)>, holes: Vec<Vec<(f64, f64)>> }
fn region(b: &[u8], extended: bool) -> Region {
    let mut r = Bin::new(b);
    let h = head(&mut r);
    r.skip(5);
    let nholes = r.u16() as usize;
    r.skip(2);
    let pl = (r.u32() & 0x00FF_FFFF) as usize;
    let p = Props::parse(r.bytes(pl));
    let n = r.u32() as usize;
    let mut outline = Vec::new();
    if extended {
        let mut vs = Vec::with_capacity(n + 1);
        for _ in 0..(n + 1).min(r.left() / 37) {
            let round = r.u8() != 0;
            let (x, y, cx, cy, rad) = (mm(r.i32()), mm(r.i32()), mm(r.i32()), mm(r.i32()), mm(r.i32()));
            let (sa, ea) = (r.f64(), r.f64());
            vs.push(Vtx { round, x, y, cx, cy, r: rad, sa, ea });
        }
        outline = ring(&vs);
    } else {
        for _ in 0..n.min(r.left() / 16) { let (x, y) = (r.f64(), r.f64()); outline.push((x * 2.54e-6, y * 2.54e-6)) }
    }
    let mut holes = Vec::with_capacity(nholes);
    for _ in 0..nholes {
        if r.left() < 4 { break }
        let m = r.u32() as usize;
        let mut hv = Vec::with_capacity(m);
        for _ in 0..m.min(r.left() / 16) { let (x, y) = (r.f64(), r.f64()); hv.push((x * 2.54e-6, y * 2.54e-6)) }
        holes.push(hv);
    }
    if outline.len() > 1 && outline.first() == outline.last() { outline.pop(); }
    Region { kind: p.int("KIND"), cutout: p.bool("ISBOARDCUTOUT"), v7: p.get("V7_LAYER").to_string(), h, outline, holes }
}
/// A V7 layer name (MECHANICAL17...) as its id; 0 for others.
fn v7_named(n: &str) -> u32 { n.strip_prefix("MECHANICAL").and_then(|d| d.parse::<u32>().ok()).filter(|d| *d > 16).map(|d| V7_MECH + d).unwrap_or(0) }

fn regions(c: &mut Ctx, out: &mut Vec<Item>) {
    let src = c.src;
    // poured copper: the regions a polygon owns
    for (i, rec, subs) in records(src.get("Regions6"), 11, 1) {
        let g = region(subs[0], false);
        if g.h.polygon == NONE || g.kind != 0 || g.outline.len() < 3 { continue }
        let a = versioned(g.h.layer, v7_named(&g.v7));
        let Some(l) = c.paint(a) else { continue };
        let info: Info = Arc::from(format!("zone{}{}", kv("net", c.net(g.h.net)), kv("layer", kname(l))));
        push(out, format!("r/{i}"), sig(rec), l, Prim::Poly { ps: kring(&fracture(g.outline, g.holes)), w: 0.0, fill: true }, &info);
    }
    for (i, rec, subs) in records(src.get("ShapeBasedRegions6"), 11, 1) {
        let g = region(subs[0], true);
        if g.outline.len() < 3 { continue }
        let a = versioned(g.h.layer, v7_named(&g.v7));
        let keepout = g.h.flags2 == 2 || a == KEEPOUT;
        let (k, s) = (format!("s/{i}"), sig(rec));
        if g.kind == 0 && g.cutout {
            c.used.insert(a);
            let mut ps = kring(&g.outline); ps.push(ps[0]);
            push(out, k, s, EDGE, Prim::Line { ps, w: 0.1 }, &Arc::from("gr_poly;layer=Edge.Cuts"));
            continue
        }
        let Some(l) = c.paint(a) else { continue };
        if keepout || g.kind == 1 || g.kind == 2 {
            let lay = if keepout || g.kind == 1 { MARGIN } else { l };
            let info = graphic_info(c, if keepout { "keepout" } else { "gr_poly" }, lay, g.h.net, g.h.comp, 0.0);
            push(out, k, s, lay, Prim::Poly { ps: kring(&g.outline), w: 0.1, fill: false }, &info);
            continue
        }
        if g.kind != 0 || g.h.polygon != NONE { continue }
        let info = graphic_info(c, "gr_poly", l, g.h.net, g.h.comp, 0.0);
        push(out, k, s, l, Prim::Poly { ps: kring(&fracture(g.outline, g.holes)), w: 0.0, fill: true }, &info);
    }
}

struct Text { rec_i: usize, sig: u32, layer: u32, comp: Option<usize>, x: f64, y: f64, h: f64, rot: f64, mirror: bool, pen: f64, is_comment: bool, is_designator: bool,
    tt: bool, arial: bool, bold: bool, italic: bool, frame: Option<(f64, f64, f64, u8)>, text: String }
impl<'a> Ctx<'a> {
    fn texts(&self) -> Vec<Text> {
        let mut out = Vec::new();
        for (i, rec, subs) in records(self.src.get("Texts6"), 5, 2) {
            let b = subs[0];
            let mut r = Bin::new(b);
            let layer = r.u8();
            r.skip(6);
            let comp = r.u16();
            r.skip(4);
            let (x, y, h) = (mm(r.i32()), mm(r.i32()), mm(r.i32()));
            r.skip(2);
            let rot = r.f64();
            let mirror = r.u8() != 0;
            let pen = mm(r.i32());
            let mut arial = false;
            let (mut is_comment, mut is_designator, mut ft, mut bold, mut italic, mut wide, mut v7) = (false, false, 0u8, false, false, u32::MAX, 0u32);
            let (mut frame_wh, mut just, mut margin, mut offset, mut is_frame, mut offset_border, mut just_ok) = ((0.0, 0.0), 0u8, 0.0, 0.0, false, false, false);
            if b.len() >= 123 {
                is_comment = r.u8() != 0; is_designator = r.u8() != 0; r.skip(1); ft = r.u8(); bold = r.u8() != 0; italic = r.u8() != 0;
                let font: Vec<u16> = r.bytes(64).chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
                arial = String::from_utf16_lossy(&font).contains("Arial");
                let _inverted = r.u8();
                margin = mm(r.i32());
                wide = r.u32();
                r.skip(4);
                let _inv_rect = r.u8();
                frame_wh = (mm(r.i32()), mm(r.i32()));
                just = r.u8();
                offset = mm(r.i32());
                let rem = b.len().saturating_sub(r.i);
                if rem >= 93 { r.skip(8 + 8 + 4 + 1 + 1 + 1); ft = r.u8(); r.skip(64 + 1); v7 = r.u32() }
                if rem >= 103 { is_frame = r.u8() != 0; offset_border = r.u8() != 0; r.skip(8) } else { is_frame = frame_wh.0 != 0.0 && frame_wh.1 != 0.0 }
                if rem >= 115 { just_ok = r.u8() != 0 }
            }
            if ft == 2 { continue } // a barcode
            let text = match self.wide.get(&wide) { Some(s) => s.clone(), None => { let mut q = Bin::new(subs[1]); let n = q.u8() as usize; super::cp1252(q.bytes(n)) } }.replace("\r\n", "\n");
            let m = if offset_border { offset } else { margin };
            let frame = if is_frame { Some((frame_wh.0, frame_wh.1, m, if just_ok { just } else { 3 })) } else { None };
            out.push(Text { rec_i: i, sig: sig(rec), layer: versioned(layer, v7), comp: if comp == NONE { None } else { Some(comp as usize) }, x, y, h, rot, mirror, pen,
                is_comment, is_designator, tt: ft == 1, arial, bold, italic, frame, text });
        }
        out
    }
}

fn texts(c: &mut Ctx, out: &mut Vec<Item>) {
    for t in c.texts() {
        let Some(l) = c.paint(t.layer) else { continue };
        let k = t.comp.and_then(|i| c.comps.get(i));
        if let Some(k) = k { if (t.is_designator && !k.nameon) || (t.is_comment && !k.commenton) { continue } }
        let s = special(&t.text, k, &c.layers.name(t.layer));
        if s.trim().is_empty() { continue }
        // TrueType text drawn with the stroke font, at the size its capitals stand
        let size = if t.tt { t.h * if t.arial { 0.63 } else { 0.5 } } else { t.h };
        let pen = if t.tt { size * if t.bold { 0.16 } else { 0.11 } } else if t.pen > 0.0 { t.pen } else { size / 8.0 };
        let mir = if t.mirror { -1.0 } else { 1.0 };
        // anchor and justification in the text's own frame (Y up, from its position), as KiCad's import places them: a free
        // string starts at its position whatever its justification; a frame justifies inside its margins
        let (at, hj, vj) = match t.frame {
            Some((w, h, m, just)) => {
                let (rw, rh) = (w - 2.0 * m, h - 2.0 * m);
                let (hj, vj) = match just { 1 => (0, 0), 2 => (0, 1), 4 => (1, 0), 5 => (1, 1), 6 => (1, 2), 7 => (2, 0), 8 => (2, 1), 9 => (2, 2), _ => (0, 2) };
                (((m + rw * hj as f64 / 2.0) * mir, m + match vj { 0 => rh, 1 => rh / 2.0, _ => 0.0 }), hj, vj)
            }
            None => ((0.0, 0.0), 0, 2),
        };
        // a frame's text wraps at its width, as Altium (and KiCad's text box) set it
        let s = match t.frame { Some((w, _, m, _)) if w - 2.0 * m > 0.0 => strokefont::wrap(&s, size as f32, (w - 2.0 * m) as f32), _ => s };
        draw_text(out, &t, l, c.ctx_of(t.comp), &s, size, pen, at, hj, vj);
    }
}
impl<'a> Ctx<'a> { fn ctx_of(&self, c: Option<usize>) -> String { c.and_then(|i| self.comps.get(i)).map(|k| k.ctx.clone()).unwrap_or_default() } }
/// Stroke text at (u, v) in the text's frame; KiCad's baseline corrections for each vertical justification.
#[allow(clippy::too_many_arguments)]
fn draw_text(out: &mut Vec<Item>, t: &Text, l: u32, ctx: String, s: &str, size: f64, pen: f64, at: (f64, f64), hj: u32, vj: u32) {
    let fr = Fr::new(t.x, t.y, t.rot);
    let dv = match vj { 0 => 0.0407 * size, 1 => -0.0355 * size, _ => -0.1225 * size } + if t.tt && vj == 2 { 0.2 * t.h } else { 0.0 };
    let p = fr.k(at.0, at.1 + dv);
    let info: Info = Arc::from(format!("text{}{ctx}", kv("text", s)));
    let prims = strokefont::text_full(s, p.x, p.y, size as f32, size as f32, pen as f32, t.rot.to_radians() as f32, hj, vj, t.mirror, t.italic);
    for (n, prim) in prims.into_iter().enumerate() { out.push(Item { key: format!("x/{}/{n}", t.rec_i), sig: t.sig, layer: l, prim, info: info.clone() }) }
}
/// Altium's special strings: `.Designator`, `.Comment`, `.Layer_Name` and the like (others are left as written).
fn special(s: &str, k: Option<&CompRec>, layer: &str) -> String {
    let t = s.trim();
    if !t.starts_with('.') || t.contains(' ') { return s.to_string() }
    match t.to_ascii_lowercase().as_str() {
        ".designator" => k.map(|k| k.refdes.clone()).unwrap_or_else(|| s.to_string()),
        ".comment" => k.map(|k| k.value.clone()).unwrap_or_else(|| s.to_string()),
        ".layer_name" => layer.to_string(),
        _ => s.to_string(),
    }
}

fn outline(c: &mut Ctx, out: &mut Vec<Item>) {
    if c.outline.len() < 3 { return }
    let mut ps = kring(&c.outline);
    ps.push(ps[0]);
    let s = c.outline.iter().fold(board::FNV, |h, p| board::hash(&[p.0.to_le_bytes(), p.1.to_le_bytes()].concat(), h));
    out.push(Item { key: "o".into(), sig: s, layer: EDGE, prim: Prim::Line { ps, w: 0.1 }, info: Arc::from("gr_poly;layer=Edge.Cuts") });
}
/// A plane is negative copper: a faint tint of the board in its colour (its splits and regions draw over it)
fn planes(c: &mut Ctx, out: &mut Vec<Item>) {
    if c.outline.len() < 3 { return }
    let ps = kring(&c.outline);
    for a in c.layers.planes.clone() {
        let Some(l) = c.layers.paint(a) else { continue };
        let (col, _) = crate::files::kicad::plotgen::look(0, l);
        out.push(Item { key: format!("pl/{a}"), sig: a ^ c.outline.len() as u32, layer: l, prim: board::tint_of(ps.clone(), col, 40), info: Arc::from(format!("zone{}", kv("layer", &c.layers.name(a)))) });
    }
}

pub(super) fn build(src: &Src) -> (Vec<Item>, Ctx<'_>) {
    let mut c = Ctx::new(src);
    let mut out = Vec::new();
    type Step = fn(&mut Ctx, &mut Vec<Item>);
    let steps: [(&str, Step); 9] = [("outline", outline), ("planes", planes), ("regions", regions), ("fills", fills), ("tracks", tracks), ("arcs", arcs), ("pads", pads), ("vias", vias), ("texts", texts)];
    for (_name, f) in steps {
        let _t = std::time::Instant::now();
        f(&mut c, &mut out);
        #[cfg(test)] if std::env::var_os("BP_ALTIUM_TIME").is_some() { eprintln!("  {_name}: {} ms", _t.elapsed().as_millis()) }
    }
    (out, c)
}

/// The whole board as items with KiCad layer ids; None if the bytes are not a readable PcbDoc.
pub fn items(bytes: &[u8]) -> Option<Vec<Item>> { let src = Src::of(bytes)?; Some(build(&src).0) }
/// Board thickness from the layer stack, in micrometres (1600 when unknown).
pub fn thickness_um(bytes: &[u8]) -> u32 {
    let Some(src) = Src::of(bytes) else { return 1600 };
    let b = src.text("Board6").into_iter().next().unwrap_or_default();
    Layers::of(&b, false).thick_um
}
pub fn components(bytes: &[u8]) -> Vec<Comp> {
    let Some(src) = Src::of(bytes) else { return vec![] };
    Ctx::new(&src).comps.into_iter().map(|k| Comp { refdes: k.refdes, footprint: k.footprint, x_mm: k.x as f32, y_mm: -k.y as f32, rot: k.rot as f32, bottom: k.bottom, value: k.value }).collect()
}
pub fn nets(bytes: &[u8]) -> Vec<String> { Src::of(bytes).map(|s| s.text("Nets6").iter().map(|p| p.get("NAME").to_string()).collect()).unwrap_or_default() }
/// The Altium layers the board's objects use: (id, name from the stack).
pub fn layers(bytes: &[u8]) -> Vec<(u32, String)> {
    let Some(src) = Src::of(bytes) else { return vec![] };
    let (_, c) = build(&src);
    c.used.iter().map(|a| (*a, c.layers.name(*a))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every PcbDoc of the private corpus (BP_ALTIUM_CORPUS) parses with sane counts; debug pictures go to BP_ALTIUM_OUT.
    #[test] fn corpus_boards() {
        let Some(root) = super::super::corpus() else { return };
        let outdir = std::env::var("BP_ALTIUM_OUT").ok();
        let mut stack = vec![root];
        let mut files = vec![];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() { stack.push(p) } else if p.extension().map(|x| x.eq_ignore_ascii_case("pcbdoc")).unwrap_or(false) { files.push(p) }
            }
        }
        files.sort();
        for f in files {
            let b = std::fs::read(&f).unwrap();
            let t = std::time::Instant::now();
            let its = items(&b).expect("a readable board");
            let ms = t.elapsed().as_millis();
            let name = f.file_stem().unwrap().to_string_lossy().to_string();
            let mut per: std::collections::BTreeMap<u32, usize> = Default::default();
            for i in &its { *per.entry(i.layer).or_default() += 1 }
            let (cs, ns, ls) = (components(&b), nets(&b), layers(&b));
            eprintln!("{name}: {} items in {ms} ms, {} comps, {} nets, thick {} um; per layer {:?}", its.len(), cs.len(), ns.len(), thickness_um(&b), per);
            eprintln!("  layers {:?}", ls);
            assert!(its.len() > 50 && !cs.is_empty(), "{name}");
            assert!(its.iter().any(|i| i.layer == EDGE), "an outline");
            // beside KiCad's own import of the same board (BP_ALTIUM_KICAD_REF/<name>.kicad_pcb), through the hub's KiCad reader
            if let Some(k) = std::env::var_os("BP_ALTIUM_KICAD_REF").map(|d| std::path::PathBuf::from(d).join(format!("{name}.kicad_pcb"))).and_then(|p| std::fs::read(p).ok()) {
                let fs = crate::files::kicad::sexpr::forms(&k).unwrap_or_default();
                let kits = crate::files::kicad::plotgen::board_items(&fs);
                let count = |its: &[Item], l: u32| its.iter().filter(|i| i.layer == l).count();
                let row: Vec<String> = [F_CU, B_CU, PADS, VIA, HOLE, F_SILK, B_SILK, EDGE].iter().map(|l| format!("{} {}/{}", kname(*l), count(&its, *l), count(&kits, *l))).collect();
                eprintln!("  ours/kicad: {}", row.join(", "));
            }
            if let Some(o) = &outdir {
                let _ = std::fs::create_dir_all(o);
                let f: Vec<Item> = its.iter().filter(|i| matches!(i.layer, F_CU | PADS | VIA | HOLE | F_SILK | EDGE)).cloned().collect();
                super::super::debug_svg(&f, 0, &format!("{o}/{name}-F.svg"));
                let bk: Vec<Item> = its.iter().filter(|i| matches!(i.layer, B_CU | PADS | VIA | HOLE | B_SILK | EDGE)).cloned().collect();
                super::super::debug_svg(&bk, 0, &format!("{o}/{name}-B.svg"));
                super::super::debug_svg(&its, 0, &format!("{o}/{name}-all.svg"));
            }
        }
    }

    // Synthetic records, laid out as the format has them
    fn text_rec(t: &str) -> Vec<u8> { let mut v = (t.len() as u32 + 1).to_le_bytes().to_vec(); v.extend_from_slice(t.as_bytes()); v.push(0); v }
    fn bin_rec(ty: u8, subs: &[&[u8]]) -> Vec<u8> { let mut v = vec![ty]; for s in subs { v.extend_from_slice(&(s.len() as u32).to_le_bytes()); v.extend_from_slice(s) } v }
    /// 1/10000 mil from mils
    fn u(mil: f64) -> [u8; 4] { ((mil * 10000.0).round() as i32).to_le_bytes() }
    fn track(layer: u8, net: u16, comp: u16, a: (f64, f64), b: (f64, f64), w: f64) -> Vec<u8> {
        let mut s = vec![layer, 0x04, 0];
        s.extend_from_slice(&net.to_le_bytes()); s.extend_from_slice(&NONE.to_le_bytes()); s.extend_from_slice(&comp.to_le_bytes()); s.extend_from_slice(&[0; 4]);
        for v in [a.0, a.1, b.0, b.1, w] { s.extend_from_slice(&u(v)) }
        s.extend_from_slice(&[0; 3]);
        bin_rec(4, &[&s])
    }
    /// An SMD pad on `layer`, w×h mils, shape byte, turned `dir` degrees, at (x, y) mils
    fn pad(name: &str, layer: u8, net: u16, comp: u16, at: (f64, f64), wh: (f64, f64), shape: u8, dir: f64, hole: f64) -> Vec<u8> {
        let mut s1 = vec![name.len() as u8]; s1.extend_from_slice(name.as_bytes());
        let mut s5 = vec![layer, 0x04, 0];
        s5.extend_from_slice(&net.to_le_bytes()); s5.extend_from_slice(&[0; 2]); s5.extend_from_slice(&comp.to_le_bytes()); s5.extend_from_slice(&[0; 4]);
        for v in [at.0, at.1, wh.0, wh.1, wh.0, wh.1, wh.0, wh.1, hole] { s5.extend_from_slice(&u(v)) }
        s5.extend_from_slice(&[shape, shape, shape]);
        s5.extend_from_slice(&dir.to_le_bytes());
        s5.push(1); s5.push(0); s5.push(0);
        s5.resize(110, 0);
        bin_rec(2, &[&s1, &[], &[], &[], &s5, &[]])
    }
    fn src(board: &str, extra: Vec<(&str, Vec<u8>)>) -> Src {
        let mut m = HashMap::new();
        m.insert("Board6".to_string(), text_rec(board));
        for (k, v) in extra { m.entry(k.to_string()).or_insert_with(Vec::new).extend(v) }
        Src(m)
    }
    const TWO: &str = "|LAYER1NAME=Top Layer|LAYER1NEXT=32|LAYER1COPTHICK=1.4mil|LAYER1DIELHEIGHT=60mil|LAYER32NAME=Bottom Layer|LAYER32NEXT=0|LAYER32COPTHICK=1.4mil\
|KIND0=0|VX0=0mil|VY0=0mil|KIND1=0|VX1=1000mil|VY1=0mil|KIND2=0|VX2=1000mil|VY2=1000mil|KIND3=0|VX3=0mil|VY3=1000mil";
    fn near(a: f32, b: f32) -> bool { (a - b).abs() < 1e-3 }

    #[test] fn a_track_lands_in_millimetres_y_down_with_its_net() {
        let s = src(TWO, vec![("Nets6", text_rec("|NAME=GND")), ("Tracks6", track(32, 0, NONE, (0.0, 100.0), (1000.0, 100.0), 10.0))]);
        let (its, _) = build(&s);
        let t = its.iter().find(|i| i.key == "t/0").expect("the track");
        assert_eq!(t.layer, B_CU);
        assert!(matches!(t.prim, Prim::Seg { a, b, w } if near(a.x, 0.0) && near(a.y, -2.54) && near(b.x, 25.4) && near(w, 0.254)), "{:?}", t.prim);
        assert_eq!(&*t.info, "track;net=GND;layer=B.Cu;width=0.254");
        let e = its.iter().find(|i| i.layer == EDGE).expect("the outline");
        assert!(matches!(&e.prim, Prim::Line { ps, .. } if ps.len() == 5 && near(ps[2].x, 25.4) && near(ps[2].y, -25.4)));
        assert_eq!(Layers::of(&s.text("Board6")[0], true).thick_um, ((1.4 * 2.0 + 60.0) * 25.4f64).round() as u32 + 20);
    }
    #[test] fn inner_layers_follow_the_stack_and_share_slots_past_six() {
        let b = Props::parse(b"|LAYER1NAME=T|LAYER1NEXT=39|LAYER39NAME=GND|LAYER39NEXT=2|LAYER2NAME=S1|LAYER2NEXT=32|LAYER32NAME=B|LAYER32NEXT=0");
        let l = Layers::of(&b, true);
        assert_eq!((l.paint(1), l.paint(39), l.paint(2), l.paint(32)), (Some(F_CU), Some(F_CU - 1), Some(F_CU - 2), Some(B_CU)));
        assert_eq!(l.planes, [39]);
        // ten inner layers: In1..In6, in order, none past In6
        let mut t = String::from("|LAYER1NAME=T|LAYER1NEXT=2");
        for i in 2..=11 { t.push_str(&format!("|LAYER{i}NAME=M|LAYER{i}NEXT={}", if i == 11 { 32 } else { i + 1 })) }
        t.push_str("|LAYER32NAME=B|LAYER32NEXT=0");
        let l = Layers::of(&Props::parse(t.as_bytes()), true);
        let slots: Vec<u32> = (2..=11).map(|a| F_CU - l.paint(a).unwrap()).collect();
        assert_eq!(slots, [1, 1, 2, 2, 3, 4, 4, 5, 5, 6]);
        // mechanical layers by kind or name; a disabled one draws nothing
        let m = Props::parse(b"|LAYER57NAME=Mechanical 1|LAYER58NAME=Top Courtyard|LAYER59NAME=Notes|LAYER59MECHKIND=FabNotes|LAYER60NAME=Off|LAYER60MECHENABLED=FALSE");
        let l = Layers::of(&m, true);
        assert_eq!((l.paint(57), l.paint(58), l.paint(59), l.paint(60)), (Some(DWGS), Some(F_CRTYD), Some(CMTS), None));
    }
    #[test] fn a_pad_turns_with_its_direction_and_carries_its_part() {
        let s = src(TWO, vec![("Nets6", text_rec("|NAME=VCC")), ("Components6", text_rec("|SOURCEDESIGNATOR=U1|PATTERN=SOT23|COMMENT=LDO|X=500mil|Y=500mil|LAYER=TOP")),
            ("Pads6", pad("1", 1, 0, 0, (500.0, 500.0), (40.0, 20.0), 2, 90.0, 0.0))]);
        let (its, _) = build(&s);
        let p = its.iter().find(|i| i.key == "p/0").expect("the pad");
        assert_eq!(p.layer, F_CU);
        assert_eq!(&*p.info, "pad 1;net=VCC;type=smd;shape=rect;ref=U1;value=LDO;footprint=SOT23");
        // 40 by 20 mils turned a quarter: 20 wide (x), 40 tall (y)
        let Prim::Poly { ps, .. } = &p.prim else { panic!() };
        let (x0, x1) = ps.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.x), a.1.max(p.x)));
        let (y0, y1) = ps.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.y), a.1.max(p.y)));
        assert!(near(x1 - x0, 0.508) && near(y1 - y0, 1.016) && near((y0 + y1) / 2.0, -12.7), "{ps:?}");
        assert!(its.iter().any(|i| i.key == "p/0/m" && i.layer == F_MASK) && its.iter().any(|i| i.key == "p/0/s" && i.layer == F_PASTE));
        let c = &components(&[])[..]; assert!(c.is_empty());
    }
    #[test] fn a_through_pad_draws_on_every_copper_layer_with_its_hole() {
        let s = src(TWO, vec![("Pads6", pad("A", MULTI as u8, NONE, NONE, (0.0, 0.0), (60.0, 60.0), 1, 0.0, 30.0))]);
        let (its, _) = build(&s);
        assert!(matches!(its.iter().find(|i| i.key == "p/0").map(|i| (i.layer, &i.prim)), Some((PADS, Prim::Circ { r, .. })) if near(*r, 0.762)));
        assert!(matches!(its.iter().find(|i| i.key == "p/0/h").map(|i| (i.layer, &i.prim)), Some((HOLE, Prim::Circ { r, .. })) if near(*r, 0.381)));
        assert!(its.iter().find(|i| i.key == "p/0").unwrap().info.contains("type=thru_hole;shape=circle"));
    }
    #[test] fn a_hole_is_cut_into_its_outline() {
        let sq = |a: f64, b: f64| vec![(a, a), (b, a), (b, b), (a, b)];
        let r = fracture(sq(0.0, 10.0), vec![sq(4.0, 6.0)]);
        assert_eq!(r.len(), 4 + 5 + 1);
        assert!((area2(&r).abs() / 2.0 - 96.0).abs() < 1e-9, "the hole's area is taken out");
    }
    #[test] fn an_arc_vertex_runs_from_its_nearer_end() {
        // a quarter circle about the origin from (10, 0) to (0, 10), entered at (0, 10): it runs backwards
        let v = Vtx { round: true, x: 0.0, y: 10.0, cx: 0.0, cy: 0.0, r: 10.0, sa: 0.0, ea: 90.0 };
        let r = ring(&[v]);
        assert!((r[0].0).abs() < 1e-9 && (r[0].1 - 10.0).abs() < 1e-9 && (r.last().unwrap().0 - 10.0).abs() < 1e-9);
        assert!(r.iter().all(|p| (p.0.hypot(p.1) - 10.0).abs() < 1e-9));
    }
    #[test] fn designator_text_names_its_part() {
        let k = CompRec { refdes: "R7".into(), footprint: String::new(), value: "10k".into(), x: 0.0, y: 0.0, rot: 0.0, bottom: false, nameon: true, commenton: true, ctx: String::new() };
        assert_eq!(special(".Designator", Some(&k), "Top Overlay"), "R7");
        assert_eq!(special(".Comment", Some(&k), ""), "10k");
        assert_eq!(special(".Layer_Name", None, "Top Overlay"), "Top Overlay");
        assert_eq!(special("v1.2", None, ""), "v1.2");
    }
}

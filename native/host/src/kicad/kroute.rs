//! The board as shapes, for `kicad_edit`'s `route` and `place` (docs/kicad-edit.md): pads, tracks, vias, holes, the board's edge and
//! keep-out areas from KiCad's own items, each with its copper layers and net; a grid router over them (A*, 45 degree steps, vias the
//! board already uses) that keeps the net class's clearance; and the search for a legal place for a part next to a pin.
//!
//! Pure: KiCad's items in, tracks and vias (or a place) out, tested without a KiCad. What it routes is checked by KiCad's own DRC in
//! the same call (kedit's `check`), so a rule this model does not know (a custom rule, a courtyard) is a finding in the answer, never
//! a silent violation. Copper zones of other nets are not obstacles: they are refilled around what was routed.
use super::kedit::mm;
use serde_json::Value;
use std::collections::{BinaryHeap, HashMap};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct P(pub f64, pub f64);
impl P {
    fn sub(self, o: P) -> P { P(self.0 - o.0, self.1 - o.1) }
    fn len(self) -> f64 { self.0.hypot(self.1) }
    pub fn dist(self, o: P) -> f64 { self.sub(o).len() }
}
fn pt(v: &Value) -> P { P(mm(&v["x_nm"]), mm(&v["y_nm"])) }
pub fn seg_dist(p: P, a: P, b: P) -> f64 { p.dist(seg_near(p, a, b)) }
fn seg_near(p: P, a: P, b: P) -> P {
    let d = b.sub(a); let l2 = d.0 * d.0 + d.1 * d.1;
    let t = if l2 == 0.0 { 0.0 } else { (((p.0 - a.0) * d.0 + (p.1 - a.1) * d.1) / l2).clamp(0.0, 1.0) };
    P(a.0 + t * d.0, a.1 + t * d.1)
}
fn inside(poly: &[P], p: P) -> bool {
    let mut c = false; let n = poly.len();
    for i in 0..n { let (a, b) = (poly[i], poly[(i + n - 1) % n]); if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 { c = !c } }
    c
}

#[derive(Clone, Debug)]
pub enum Shape {
    Circle(P, f64),
    /// a track: two ends and half its width
    Capsule(P, P, f64),
    /// centre, half sizes, KiCad's angle in degrees (counter-clockwise on screen, y down), the corners' radius
    Rect(P, f64, f64, f64, f64),
    Poly(Vec<P>),
}
impl Shape {
    /// How far a point is from the copper (0 inside).
    pub fn dist(&self, p: P) -> f64 {
        match self {
            Shape::Circle(c, r) => (p.dist(*c) - r).max(0.0),
            Shape::Capsule(a, b, r) => (seg_dist(p, *a, *b) - r).max(0.0),
            Shape::Rect(c, hx, hy, deg, r) => { let (s, co) = deg.to_radians().sin_cos(); let d = p.sub(*c);
                let (lx, ly) = (d.0 * co - d.1 * s, d.0 * s + d.1 * co);
                ((lx.abs() - (hx - r)).max(0.0).hypot((ly.abs() - (hy - r)).max(0.0)) - r).max(0.0) }
            Shape::Poly(v) => if inside(v, p) { 0.0 } else { (0..v.len()).map(|i| seg_dist(p, v[i], v[(i + 1) % v.len()])).fold(f64::MAX, f64::min) },
        }
    }
    pub fn bbox(&self) -> (f64, f64, f64, f64) {
        match self {
            Shape::Circle(c, r) => (c.0 - r, c.1 - r, c.0 + r, c.1 + r),
            Shape::Capsule(a, b, r) => (a.0.min(b.0) - r, a.1.min(b.1) - r, a.0.max(b.0) + r, a.1.max(b.1) + r),
            Shape::Rect(c, hx, hy, ..) => { let r = hx.hypot(*hy); (c.0 - r, c.1 - r, c.0 + r, c.1 + r) }
            Shape::Poly(v) => v.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, p| (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))),
        }
    }
    /// The point a track joins it at, nearest to `p`: a pad's or via's centre, the nearest point of a track's centre line.
    pub fn anchor(&self, p: P) -> P {
        match self { Shape::Circle(c, _) | Shape::Rect(c, ..) => *c, Shape::Capsule(a, b, _) => seg_near(p, *a, *b), Shape::Poly(_) => p }
    }
    /// Points on its outline (and its centre), `step` apart at most: what a clearance check of shape against shape samples.
    pub fn samples(&self, step: f64) -> Vec<P> {
        let line = |a: P, b: P, out: &mut Vec<P>| { let n = (a.dist(b) / step).ceil().max(1.0) as usize; for i in 0..=n { let t = i as f64 / n as f64; out.push(P(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)) } };
        let mut out = vec![];
        match self {
            Shape::Circle(c, r) => { out.push(*c); for i in 0..16 { let a = i as f64 * std::f64::consts::PI / 8.0; out.push(P(c.0 + r * a.cos(), c.1 + r * a.sin())) } }
            Shape::Capsule(a, b, r) => { line(*a, *b, &mut out); for q in out.clone() { for i in 0..8 { let t = i as f64 * std::f64::consts::PI / 4.0; out.push(P(q.0 + r * t.cos(), q.1 + r * t.sin())) } } }
            Shape::Rect(c, ..) => { out.push(*c); let k = self.corners(); for i in 0..k.len() { line(k[i], k[(i + 1) % k.len()], &mut out) } }
            Shape::Poly(v) => for i in 0..v.len() { line(v[i], v[(i + 1) % v.len()], &mut out) },
        }
        out
    }
    fn corners(&self) -> Vec<P> {
        match self { Shape::Rect(c, hx, hy, deg, r) => { let (s, co) = deg.to_radians().sin_cos();
                // each corner as three points of its arc (one point when it is square)
                let mut out = vec![]; for (k, (a, b)) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().enumerate() { let (ix, iy) = (a * (hx - r), b * (hy - r));
                    let arc: &[f64] = if *r > 0.0 { &[0.0, 45.0, 90.0] } else { &[45.0] }; let rr = if *r > 0.0 { *r } else { 0.0 };
                    for t in arc { let ang = (180.0 + 90.0 * k as f64 + t).to_radians(); let (lx, ly) = (ix + rr * ang.cos(), iy + rr * ang.sin()); out.push(P(c.0 + lx * co + ly * s, c.1 - lx * s + ly * co)) } }
                out }
            Shape::Poly(v) => v.clone(), _ => vec![] }
    }
    /// The shape after its part turned by `deg` about `about` and moved by `by`.
    pub fn moved(&self, about: P, deg: f64, by: P) -> Shape {
        let (s, co) = deg.to_radians().sin_cos();
        let m = |p: P| { let d = p.sub(about); P(about.0 + d.0 * co + d.1 * s + by.0, about.1 - d.0 * s + d.1 * co + by.1) };
        match self { Shape::Circle(c, r) => Shape::Circle(m(*c), *r), Shape::Capsule(a, b, r) => Shape::Capsule(m(*a), m(*b), *r),
            Shape::Rect(c, hx, hy, a, r) => Shape::Rect(m(*c), *hx, *hy, a + deg, *r), Shape::Poly(v) => Shape::Poly(v.iter().map(|p| m(*p)).collect()) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind { Pad, Track, Via, Hole, Edge, KeepTracks, KeepVias }
#[derive(Clone, Debug)]
pub struct Item { pub shape: Shape, /** bit i: the board's i-th copper layer */ pub layers: u64, pub net: String, pub kind: Kind, /** the part and pad ("U1", "3"), a pad's */ pub of: (String, String), pub smd: bool }
/// One kind of via the board may use between two copper layers.
#[derive(Clone, Debug, PartialEq)]
pub struct Span { pub from: usize, pub to: usize, pub ty: String, pub size: f64, pub drill: f64, /** how many vias of this kind the board has (a kind the board hardly uses costs a route more) */ pub n: usize, /** one of the board's own vias of this kind (KiCad's item): a new one is made like it (filled, capped, tented as the board's are) */ pub like: Value }
#[derive(Clone, Debug, Default)]
pub struct Board {
    /// copper layers, top to bottom, as KiCad shows them ("F.Cu")
    pub layers: Vec<String>, pub items: Vec<Item>,
    /// a net's zone outlines per layer (its pour connects what stands in it)
    pub fills: Vec<(usize, String, Vec<P>)>,
    /// clearance by net (its class's), and the board's own minimums
    pub clearance: HashMap<String, f64>, pub min_clearance: f64, pub edge_clearance: f64, pub hole_clearance: f64,
    /// the via kinds the board already uses (the commonest size of each span)
    pub spans: Vec<Span>,
    /// copper layers that carry planes, not tracks (an inner layer with a zone and next to no tracks): never routed on unless asked
    pub planes: u64,
    /// each part's courtyard (reference, layer bit, outline)
    pub courtyards: Vec<(String, u64, Vec<P>)>,
}
fn copper_order(l: &str) -> i64 { match l { "F.Cu" => 0, "B.Cu" => 1000, _ => l.strip_prefix("In").and_then(|s| s.strip_suffix(".Cu")).and_then(|n| n.parse().ok()).unwrap_or(999) } }
fn arc_pts(a: P, m: P, b: P, n: usize) -> Vec<P> {
    // the circle through three points; a straight line when they are collinear
    let d = 2.0 * (a.0 * (m.1 - b.1) + m.0 * (b.1 - a.1) + b.0 * (a.1 - m.1));
    if d.abs() < 1e-9 { return vec![a, m, b] }
    let s = |p: P| p.0 * p.0 + p.1 * p.1;
    let c = P((s(a) * (m.1 - b.1) + s(m) * (b.1 - a.1) + s(b) * (a.1 - m.1)) / d, (s(a) * (b.0 - m.0) + s(m) * (a.0 - b.0) + s(b) * (m.0 - a.0)) / d);
    let r = c.dist(a); let ang = |p: P| (p.1 - c.1).atan2(p.0 - c.0);
    let (a0, am, mut a1) = (ang(a), ang(m), ang(b)); let tau = std::f64::consts::TAU;
    let norm = |x: f64| x.rem_euclid(tau);
    // go from a0 to a1 the way that passes am
    let ccw = norm(am - a0) < norm(a1 - a0);
    a1 = if ccw { a0 + norm(a1 - a0) } else { a0 - norm(a0 - a1) };
    (0..=n).map(|i| { let t = a0 + (a1 - a0) * i as f64 / n as f64; P(c.0 + r * t.cos(), c.1 + r * t.sin()) }).collect()
}
fn outline(shape: &Value) -> Vec<Vec<P>> {
    if shape["segment"].is_object() { vec![vec![pt(&shape["segment"]["start"]), pt(&shape["segment"]["end"])]] }
    else if shape["arc"].is_object() { vec![arc_pts(pt(&shape["arc"]["start"]), pt(&shape["arc"]["mid"]), pt(&shape["arc"]["end"]), 12)] }
    else if shape["rectangle"].is_object() { let (a, b) = (pt(&shape["rectangle"]["top_left"]), pt(&shape["rectangle"]["bottom_right"])); vec![vec![a, P(b.0, a.1), b, P(a.0, b.1), a]] }
    else if shape["circle"].is_object() { let (c, e) = (pt(&shape["circle"]["center"]), pt(&shape["circle"]["radius_point"])); let r = c.dist(e); vec![(0..=32).map(|i| { let t = i as f64 * std::f64::consts::TAU / 32.0; P(c.0 + r * t.cos(), c.1 + r * t.sin()) }).collect()] }
    else { shape["polygon"]["polygons"].as_array().into_iter().flatten().map(|p| { let mut v = nodes(&p["outline"]); if let Some(f) = v.first().copied() { v.push(f) } v }).collect() }
}
fn nodes(line: &Value) -> Vec<P> { line["nodes"].as_array().into_iter().flatten().filter(|n| n["point"].is_object()).map(|n| pt(&n["point"])).collect() }

impl Board {
    /// The board from KiCad's own items: enabled layers (BL_ names), footprints, tracks, arcs, vias, zones and board-level shapes;
    /// `rules` is GetBoardDesignRules' `constraints`, `classes` GetNetClassForNets' `classes`.
    pub fn from_raw(enabled: &[String], fps: &[Value], tracks: &[Value], arcs: &[Value], vias: &[Value], zones: &[Value], shapes: &[Value], rules: &Value, classes: &Value) -> Board {
        let mut layers: Vec<String> = enabled.iter().map(|l| super::kedit::layer_name(l)).filter(|l| l.ends_with(".Cu")).collect();
        layers.sort_by_key(|l| copper_order(l)); layers.dedup();
        let idx = |l: &str| layers.iter().position(|x| *x == super::kedit::layer_name(l));
        let all: u64 = (1u64 << layers.len()) - 1;
        let bits = |ls: &Value| ls.as_array().into_iter().flatten().filter_map(|l| idx(l.as_str().unwrap_or(""))).fold(0u64, |m, i| m | 1 << i);
        let mut b = Board { layers: layers.clone(), min_clearance: mm(&rules["min_clearance"]["value_nm"]), edge_clearance: mm(&rules["copper_edge_clearance"]["value_nm"]), hole_clearance: mm(&rules["hole_clearance"]["value_nm"]), ..Default::default() };
        for (net, c) in classes.as_object().into_iter().flatten() { b.clearance.insert(net.clone(), mm(&c["board"]["clearance"]["value_nm"])); }
        let net = |v: &Value| v["net"]["name"].as_str().unwrap_or("").to_string();
        // copper drawn as a shape (a filled polygon, a line): it stands in the way as a track would
        let copper = |s: &Value, items: &mut Vec<Item>| { let Some(i) = idx(s["layer"].as_str().unwrap_or("")) else { return }; let w = mm(&s["shape"]["attributes"]["stroke"]["width"]["value_nm"]) / 2.0; let nt = s["net"]["name"].as_str().unwrap_or("").to_string();
            let filled = s["shape"]["attributes"]["fill"]["fill_type"].as_str().is_some_and(|f| f != "GFT_UNFILLED");
            for line in outline(&s["shape"]) { if filled && line.len() > 3 { items.push(Item { shape: Shape::Poly(line[..line.len() - 1].to_vec()), layers: 1 << i, net: nt.clone(), kind: Kind::Track, of: Default::default(), smd: false }) }
                for k in line.windows(2) { items.push(Item { shape: Shape::Capsule(k[0], k[1], w), layers: 1 << i, net: nt.clone(), kind: Kind::Track, of: Default::default(), smd: false }) } } };
        let mut edge = |shape: &Value, items: &mut Vec<Item>| for line in outline(shape) { for w in line.windows(2) { items.push(Item { shape: Shape::Capsule(w[0], w[1], 0.0), layers: all, net: String::new(), kind: Kind::Edge, of: Default::default(), smd: false }) } };
        for fp in fps {
            let r = fp["reference_field"]["text"]["text"]["text"].as_str().unwrap_or("").to_string(); let mut yard: HashMap<u64, Vec<P>> = HashMap::new();
            for it in fp["definition"]["items"].as_array().into_iter().flatten() {
                let ty = it["@type"].as_str().unwrap_or("");
                if ty.ends_with(".Pad") {
                    let st = &it["pad_stack"]; let c = pt(&it["position"]); let cl = &st["copper_layers"][0]; let (sx, sy) = (mm(&cl["size"]["x_nm"]) / 2.0, mm(&cl["size"]["y_nm"]) / 2.0);
                    let deg = st["angle"]["value_degrees"].as_f64().unwrap_or(0.0); let pty = it["type"].as_str().unwrap_or(""); let hole = mm(&st["drill"]["diameter"]["x_nm"]) / 2.0;
                    if pty == "PT_NPTH" { b.items.push(Item { shape: Shape::Circle(c, hole.max(sx.min(sy))), layers: all, net: String::new(), kind: Kind::Hole, of: Default::default(), smd: false }); continue }
                    let shape = match cl["shape"].as_str().unwrap_or("") {
                        "PSS_CIRCLE" => Shape::Circle(c, sx),
                        "PSS_OVAL" => { let (s, co) = deg.to_radians().sin_cos(); let (l, r) = if sx >= sy { (sx - sy, sy) } else { (sy - sx, sx) }; let d = if sx >= sy { P(l * co, -l * s) } else { P(l * s, l * co) }; Shape::Capsule(P(c.0 - d.0, c.1 - d.1), P(c.0 + d.0, c.1 + d.1), r) }
                        "PSS_ROUNDRECT" => Shape::Rect(c, sx, sy, deg, cl["corner_rounding_ratio"].as_f64().unwrap_or(0.0).clamp(0.0, 0.5) * 2.0 * sx.min(sy)),
                        _ => Shape::Rect(c, sx, sy, deg, 0.0),
                    };
                    let ls = if pty == "PT_PTH" { all } else { bits(&st["layers"]) };
                    b.items.push(Item { shape, layers: ls, net: net(it), kind: Kind::Pad, of: (r.clone(), it["number"].as_str().unwrap_or("").into()), smd: pty != "PT_PTH" });
                } else if ty.ends_with(".BoardGraphicShape") {
                    match it["layer"].as_str().unwrap_or("") {
                        "BL_Edge_Cuts" => edge(&it["shape"], &mut b.items),
                        l if l.ends_with("_Cu") => copper(it, &mut b.items),
                        l @ ("BL_F_CrtYd" | "BL_B_CrtYd") => { let side = if l == "BL_F_CrtYd" { 1u64 } else { 1 << (layers.len().max(1) - 1) }; for line in outline(&it["shape"]) { yard.entry(side).or_default().extend(line) } }
                        _ => {}
                    }
                }
            }
            for (side, pts) in yard { b.courtyards.push((r.clone(), side, hull(&pts))) }
        }
        for t in tracks { if let Some(i) = idx(t["layer"].as_str().unwrap_or("")) { b.items.push(Item { shape: Shape::Capsule(pt(&t["start"]), pt(&t["end"]), mm(&t["width"]["value_nm"]) / 2.0), layers: 1 << i, net: net(t), kind: Kind::Track, of: Default::default(), smd: false }) } }
        for t in arcs { if let Some(i) = idx(t["layer"].as_str().unwrap_or("")) { for w in arc_pts(pt(&t["start"]), pt(&t["mid"]), pt(&t["end"]), 8).windows(2) {
            b.items.push(Item { shape: Shape::Capsule(w[0], w[1], mm(&t["width"]["value_nm"]) / 2.0), layers: 1 << i, net: net(t), kind: Kind::Track, of: Default::default(), smd: false }) } } }
        let mut seen: HashMap<(usize, usize, String, i64, i64), (usize, Value)> = HashMap::new();
        for v in vias {
            let st = &v["pad_stack"]; let (a, z) = (idx(st["drill"]["start_layer"].as_str().unwrap_or("")).unwrap_or(0), idx(st["drill"]["end_layer"].as_str().unwrap_or("")).unwrap_or(layers.len().max(1) - 1)); let (a, z) = (a.min(z), a.max(z));
            let (size, drill) = (mm(&st["copper_layers"][0]["size"]["x_nm"]), mm(&st["drill"]["diameter"]["x_nm"]));
            b.items.push(Item { shape: Shape::Circle(pt(&v["position"]), size / 2.0), layers: (a..=z).fold(0, |m, i| m | 1 << i), net: net(v), kind: Kind::Via, of: Default::default(), smd: false });
            let e = seen.entry((a, z, v["type"].as_str().unwrap_or("VT_THROUGH").into(), (size * 1e6).round() as i64, (drill * 1e6).round() as i64)).or_insert((0, Value::Null)); e.0 += 1; if e.1.is_null() { e.1 = v.clone() }
        }
        // of each kind the size the board uses most; of sizes used alike often (within a quarter of the commonest) the larger, which every rule of the board allows
        let mut spans: HashMap<(usize, usize, String), Vec<(usize, f64, f64, Value)>> = HashMap::new();
        for ((a, z, ty, s, d), (n, like)) in seen { spans.entry((a, z, ty)).or_default().push((n, s as f64 / 1e6, d as f64 / 1e6, like)) }
        b.spans = spans.into_iter().map(|((from, to, ty), mut v)| { let top = v.iter().map(|x| x.0).max().unwrap_or(0); let n: usize = v.iter().map(|x| x.0).sum(); v.retain(|x| x.0 * 4 >= top); v.sort_by(|a, z| z.1.partial_cmp(&a.1).unwrap()); let (_, size, drill, like) = v.remove(0); Span { from, to, ty, size, drill, n, like } }).collect(); b.spans.sort_by_key(|s| (s.from, s.to));
        let mut zoned = 0u64;
        for z in zones {
            let ls = bits(&z["layers"]);
            if z["type"] == "ZT_COPPER" { zoned |= ls;
                // the zone's own outline: what stands in it on its net is joined by the pour once it is refilled (an island the pour cannot
                // reach is KiCad's check to find, in the same call)
                for i in 0..layers.len() { if ls & 1 << i != 0 { for p in z["outline"]["polygons"].as_array().into_iter().flatten() { b.fills.push((i, z["copper_settings"]["net"]["name"].as_str().unwrap_or("").into(), nodes(&p["outline"]))) } } }
            } else { let k = &z["rule_area_settings"];
                for (flag, kind) in [("keepout_tracks", Kind::KeepTracks), ("keepout_vias", Kind::KeepVias)] { if k[flag].as_bool().unwrap_or(false) { for p in z["outline"]["polygons"].as_array().into_iter().flatten() { b.items.push(Item { shape: Shape::Poly(nodes(&p["outline"])), layers: ls, net: String::new(), kind, of: Default::default(), smd: false }) } } } }
        }
        for s in shapes { if s["layer"] == "BL_Edge_Cuts" { edge(&s["shape"], &mut b.items) } else { copper(s, &mut b.items) } }
        // a plane layer: an inner layer with a zone and under 2% of the board's tracks
        let total = b.items.iter().filter(|i| i.kind == Kind::Track).count().max(1);
        for i in 1..layers.len().saturating_sub(1) { if zoned & 1 << i != 0 && b.items.iter().filter(|x| x.kind == Kind::Track && x.layers == 1 << i).count() * 50 < total { b.planes |= 1 << i } }
        b
    }
    pub fn layer(&self, name: &str) -> Option<usize> { self.layers.iter().position(|l| l == name) }
    fn clr(&self, own: f64, it: &Item) -> f64 {
        match it.kind { Kind::Edge => self.edge_clearance.max(own), Kind::Hole => self.hole_clearance.max(own), Kind::KeepTracks | Kind::KeepVias => 0.0, _ => own.max(self.clearance.get(&it.net).copied().unwrap_or(0.0)).max(self.min_clearance) }
    }
    pub fn net_clearance(&self, net: &str) -> f64 { self.clearance.get(net).copied().unwrap_or(0.0).max(self.min_clearance) }
}
/// The convex hull of points (a courtyard drawn as lines becomes its outline).
pub fn hull(pts: &[P]) -> Vec<P> {
    let mut p: Vec<P> = pts.to_vec(); p.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap())); p.dedup();
    if p.len() < 3 { return p }
    let cross = |o: P, a: P, b: P| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let half = |it: &mut dyn Iterator<Item = &P>| { let mut h: Vec<P> = vec![]; for q in it { while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], *q) <= 0.0 { h.pop(); } h.push(*q) } h.pop(); h };
    let mut h = half(&mut p.iter()); h.extend(half(&mut p.iter().rev())); h
}
/// Whether two convex outlines overlap (separating axis).
pub fn overlap(a: &[P], b: &[P]) -> bool {
    if a.len() < 3 || b.len() < 3 { return false }
    for poly in [a, b] { for i in 0..poly.len() { let (p, q) = (poly[i], poly[(i + 1) % poly.len()]); let ax = P(-(q.1 - p.1), q.0 - p.0);
        let span = |v: &[P]| v.iter().fold((f64::MAX, f64::MIN), |m, x| { let d = x.0 * ax.0 + x.1 * ax.1; (m.0.min(d), m.1.max(d)) });
        let (s, t) = (span(a), span(b)); if s.1 <= t.0 + 1e-9 || t.1 <= s.0 + 1e-9 { return false } } }
    true
}

/// How to move convex `a` out of convex `b` by the shortest way (None: they do not overlap).
pub fn push_out(a: &[P], b: &[P]) -> Option<P> {
    if a.len() < 3 || b.len() < 3 { return None }
    let mid = |v: &[P]| { let n = v.len() as f64; P(v.iter().map(|p| p.0).sum::<f64>() / n, v.iter().map(|p| p.1).sum::<f64>() / n) };
    let mut best: Option<(f64, P)> = None;
    for poly in [a, b] { for i in 0..poly.len() { let (p, q) = (poly[i], poly[(i + 1) % poly.len()]); let l = p.dist(q); if l < 1e-9 { continue } let ax = P(-(q.1 - p.1) / l, (q.0 - p.0) / l);
        let span = |v: &[P]| v.iter().fold((f64::MAX, f64::MIN), |m, x| { let d = x.0 * ax.0 + x.1 * ax.1; (m.0.min(d), m.1.max(d)) });
        let (s, t) = (span(a), span(b)); let depth = (s.1.min(t.1)) - (s.0.max(t.0)); if depth <= 1e-9 { return None }
        if best.map_or(true, |x| depth < x.0) { best = Some((depth, ax)) } } }
    let (depth, ax) = best?; let d = mid(a).sub(mid(b)); let sign = if d.0 * ax.0 + d.1 * ax.1 >= 0.0 { 1.0 } else { -1.0 };
    Some(P(ax.0 * depth * sign, ax.1 * depth * sign))
}

// ---- the router ------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Opts { pub width: f64, /** copper layers to route on (indices); empty: the layers the net's pads are on, then every signal layer */ pub layers: Vec<usize>, /** false: no vias */ pub vias: bool, /** a via size to use instead of the board's */ pub via: Option<(f64, f64)>, /** plane layers too (a track there costs three times its length where it cuts another net's pour) */ pub planes: bool }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Routed { pub tracks: Vec<(P, P, usize)>, pub vias: Vec<(P, Span)>, pub length: f64 }

const TRACK: u8 = 1; const VIA: u8 = 2; const POUR: u8 = 4; const OWNPAD: u8 = 8; const UVIA: u8 = 16; const NEAR: u8 = 32; const COURT: u8 = 64;
/// How much room a track is given over its clearance where there is room (a cell nearer than this to other copper costs more).
const ROOM: f64 = 0.1;
const DIRS: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
struct Grid { x0: f64, y0: f64, g: f64, nx: usize, ny: usize, nl: usize, block: Vec<u8>, own: Vec<u32> }
impl Grid {
    fn at(&self, c: usize) -> P { P(self.x0 + (c % self.nx) as f64 * self.g, self.y0 + (c / self.nx) as f64 * self.g) }
    fn cells(&self, bb: (f64, f64, f64, f64), pad: f64) -> (usize, usize, usize, usize) {
        let f = |v: f64, o: f64, n: usize| (((v - o) / self.g).floor().max(0.0) as usize).min(n - 1);
        (f(bb.0 - pad, self.x0, self.nx), f(bb.1 - pad, self.y0, self.ny), f(bb.2 + pad + self.g, self.x0, self.nx), f(bb.3 + pad + self.g, self.y0, self.ny))
    }
}
/// The cells inside a polygon, by scan lines (the crossings of the outline; the cells between each pair are inside).
fn scan(gr: &Grid, poly: &[P], mut f: impl FnMut(usize)) {
    if poly.len() < 3 { return }
    let pb = Shape::Poly(poly.to_vec()).bbox(); let (cx0, cy0, cx1, cy1) = gr.cells(pb, 0.0);
    for cy in cy0..=cy1 { let y = gr.y0 + cy as f64 * gr.g;
        let mut xs: Vec<f64> = (0..poly.len()).filter_map(|i| { let (a, z) = (poly[i], poly[(i + 1) % poly.len()]); ((a.1 > y) != (z.1 > y)).then(|| a.0 + (y - a.1) / (z.1 - a.1) * (z.0 - a.0)) }).collect();
        xs.sort_by(|a, z| a.partial_cmp(z).unwrap());
        for w in xs.chunks(2) { if w.len() < 2 { break } let (a, z) = ((((w[0] - gr.x0) / gr.g).ceil().max(cx0 as f64)) as usize, (((w[1] - gr.x0) / gr.g).floor().min(cx1 as f64)) as usize); for cx in a..=z.min(gr.nx - 1) { f(cy * gr.nx + cx) } } }
}
#[derive(PartialEq)] struct Node(f32, u32);
impl Eq for Node {}
impl Ord for Node { fn cmp(&self, o: &Self) -> std::cmp::Ordering { o.0.partial_cmp(&self.0).unwrap_or(std::cmp::Ordering::Equal) } }
impl PartialOrd for Node { fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) } }

fn find(u: &mut Vec<usize>, mut i: usize) -> usize { while u[i] != i { u[i] = u[u[i]]; i = u[i] } i }

/// Connect everything of `net` that is not connected yet, with tracks `o.width` wide that keep the clearance of the net's class (and
/// of each neighbour's) to all other copper, holes and the board's edge. What was routed joins the board (the next net sees it).
/// Err names what is still apart when no way was found; the copper routed before that stays in `Routed` of the error's first field.
pub fn route_net(b: &mut Board, net: &str, o: &Opts) -> Result<Routed, (Routed, String)> {
    let mut out = Routed::default();
    for _ in 0..64 {
        match step(b, net, o) { Ok(None) => return Ok(out), Err(e) => return Err((out, e)),
            Ok(Some(r)) => { for (a, z, l) in &r.tracks { b.items.push(Item { shape: Shape::Capsule(*a, *z, o.width / 2.0), layers: 1 << l, net: net.into(), kind: Kind::Track, of: Default::default(), smd: false }) }
                for (p, s) in &r.vias { b.items.push(Item { shape: Shape::Circle(*p, s.size / 2.0), layers: (s.from..=s.to).fold(0, |m, i| m | 1 << i), net: net.into(), kind: Kind::Via, of: Default::default(), smd: false }) }
                out.length += r.length; out.tracks.extend(r.tracks); out.vias.extend(r.vias) } }
    }
    Err((out, "more than 64 connections on one net".into()))
}
fn name(it: &Item, layers: &[String]) -> String {
    let l = (0..layers.len()).find(|i| it.layers & 1 << i != 0).map(|i| layers[i].as_str()).unwrap_or("");
    match it.kind { Kind::Pad => format!("{}.{}", it.of.0, it.of.1), Kind::Via => { let c = it.shape.anchor(P(0.0, 0.0)); format!("the via at {:.3},{:.3}", c.0, c.1) }, _ => { let c = it.shape.anchor(P(0.0, 0.0)); format!("the track near {:.3},{:.3} on {l}", c.0, c.1) } }
}
/// One connection: from the net's first group of joined copper to the nearest other group. None: the net is all one group.
fn step(b: &Board, net: &str, o: &Opts) -> Result<Option<Routed>, String> {
    let own: Vec<usize> = (0..b.items.len()).filter(|i| b.items[*i].net == net && matches!(b.items[*i].kind, Kind::Pad | Kind::Track | Kind::Via)).collect();
    if own.len() < 2 { return if own.is_empty() { Err(format!("no pad, track or via is on net `{net}`")) } else { Ok(None) } }
    let clr = b.net_clearance(net); let nl = b.layers.len();
    // the layers to route on: asked for, else every copper layer that is not a plane (the net's own layers always)
    let padl = own.iter().fold(0u64, |m, i| m | b.items[*i].layers);
    let route: u64 = if o.layers.is_empty() { (1u64 << nl) - 1 } else { o.layers.iter().fold(0, |m, l| m | 1 << l) | padl };
    let spans: Vec<Span> = if !o.vias { vec![] } else { let mut s: Vec<Span> = b.spans.clone();
        if s.is_empty() && nl > 1 { s.push(Span { from: 0, to: nl - 1, ty: "VT_THROUGH".into(), size: 0.6, drill: 0.3, n: 0, like: Value::Null }) }
        if let Some((size, drill)) = o.via { for x in &mut s { x.size = size; x.drill = drill } } s };
    // drilled vias keep the board's hole clearance too; a laser micro via its copper's clearance (a board that asks more of them says so in its own rules, and KiCad's check in the same call)
    let micro = |s: &&Span| s.ty == "VT_MICRO";
    let viar = spans.iter().filter(|s| !micro(s)).map(|s| s.size / 2.0).fold(0.0, f64::max); let drillr = spans.iter().filter(|s| !micro(s)).map(|s| s.drill / 2.0).fold(0.0, f64::max);
    let uviar = spans.iter().filter(micro).map(|s| s.size / 2.0).fold(0.0, f64::max);
    // the board's own conventions: a layer it hardly routes on costs a quarter more, a drilled via on a board of laser micro vias costs by how
    // rare its kind is there
    let per: Vec<usize> = (0..nl).map(|l| b.items.iter().filter(|i| i.kind == Kind::Track && i.layers == 1 << l).count()).collect(); let most = per.iter().copied().max().unwrap_or(0);
    // (a plane layer is open to a stack of micro vias passing through and, at two and a half times the length, to a track with no other way)
    let lm: Vec<f32> = (0..nl).map(|l| if !o.layers.is_empty() || padl & 1 << l != 0 { 1.0 } else if b.planes & 1 << l != 0 && !o.planes { 2.5 } else if per[l] * 10 >= most { 1.0 } else { 1.25 }).collect();
    let (all_vias, has_micro) = (spans.iter().map(|s| s.n).sum::<usize>(), spans.iter().any(|s| s.ty == "VT_MICRO"));
    let rare: Vec<f32> = spans.iter().map(|s| if has_micro && s.ty != "VT_MICRO" && all_vias > 0 { 6.0 * (1.0 - s.n as f32 / all_vias as f32) } else { 0.0 }).collect();
    let bb = own.iter().map(|i| b.items[*i].shape.bbox()).fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |m, x| (m.0.min(x.0), m.1.min(x.1), m.2.max(x.2), m.3.max(x.3)));
    let mut last = String::new();
    for margin in [1.5, 5.0] {
        let (w, h) = (bb.2 - bb.0 + 2.0 * margin, bb.3 - bb.1 + 2.0 * margin);
        // a cell is a quarter of the track or finer; the whole search stays under 40 million states
        let mut g = (o.width / 8.0).clamp(0.0125, 0.025); let cap = 40e6 / (8.0 * route.count_ones() as f64); if w * h / (g * g) > cap { g = (w * h / cap).sqrt() }
        let mut gr = Grid { x0: bb.0 - margin, y0: bb.1 - margin, g, nx: (w / g).ceil() as usize + 1, ny: (h / g).ceil() as usize + 1, nl, block: vec![], own: vec![] };
        let n = gr.nx * gr.ny; gr.block = vec![0; n * nl]; gr.own = vec![0; n * nl];
        let eps = 0.75 * g; let mut uf: Vec<usize> = (0..own.len()).collect();
        let region = (gr.x0, gr.y0, gr.x0 + w, gr.y0 + h);
        for (k, it) in b.items.iter().enumerate() {
            let ib = it.shape.bbox(); if ib.2 < region.0 - 1.0 || ib.0 > region.2 + 1.0 || ib.3 < region.1 - 1.0 || ib.1 > region.3 + 1.0 { continue }
            if it.net == net && matches!(it.kind, Kind::Pad | Kind::Track | Kind::Via) {
                let me = own.iter().position(|x| *x == k).unwrap(); let (cx0, cy0, cx1, cy1) = gr.cells(ib, 0.0);
                for l in 0..nl { if it.layers & 1 << l == 0 { continue }
                    for cy in cy0..=cy1 { for cx in cx0..=cx1 { let c = cy * gr.nx + cx; if it.shape.dist(gr.at(c)) > 0.0 { continue }
                        let cell = &mut gr.own[l * n + c]; if *cell != 0 { let (a, z) = (find(&mut uf, *cell as usize - 1), find(&mut uf, me)); uf[a] = z } *cell = me as u32 + 1;
                        // no drilled via in an SMD pad of the net (a filled micro via may stand in one)
                        if it.kind == Kind::Pad && it.smd { gr.block[l * n + c] |= OWNPAD } } } }
                continue
            }
            let c0 = b.clr(clr, it); let (kt, kv) = (it.kind != Kind::KeepVias, it.kind != Kind::KeepTracks);
            let (it_t, it_v, it_u) = (o.width / 2.0 + c0 + eps, (viar + c0).max(if it.kind == Kind::Edge { 0.0 } else { drillr + b.hole_clearance }) + eps, uviar + c0 + eps);
            let (cx0, cy0, cx1, cy1) = gr.cells(ib, (it_t + ROOM).max(it_v).max(it_u));
            for l in 0..nl { if it.layers & 1 << l == 0 { continue }
                for cy in cy0..=cy1 { for cx in cx0..=cx1 { let c = cy * gr.nx + cx; let d = it.shape.dist(gr.at(c));
                    if kt && d < it_t { gr.block[l * n + c] |= TRACK } else if kt && d < it_t + ROOM && matches!(it.kind, Kind::Pad | Kind::Track | Kind::Via) { gr.block[l * n + c] |= NEAR } if kv && d < it_v { gr.block[l * n + c] |= VIA } if kv && d < it_u { gr.block[l * n + c] |= UVIA } } } }
        }
        // another net's pour on an inner layer: a track through it cuts the plane, so it costs three times its length there
        for (l, zn, poly) in &b.fills { if zn == net || *l == 0 || *l + 1 >= nl { continue }
            let pb = Shape::Poly(poly.clone()).bbox(); if poly.len() < 3 || pb.2 < region.0 || pb.0 > region.2 || pb.3 < region.1 || pb.1 > region.3 { continue }
            let mut cells = vec![]; scan(&gr, poly, |c| cells.push(c)); for c in cells { gr.block[l * n + c] |= POUR } }
        // under a part with eight pads or more (an IC): a drilled via there costs more (kept on the first layer's cells: a drilled via is in every layer)
        for (r, _, poly) in &b.courtyards { if b.items.iter().filter(|i| i.kind == Kind::Pad && i.of.0 == *r).count() < 8 { continue }
            let pb = Shape::Poly(poly.clone()).bbox(); if poly.len() < 3 || pb.2 < region.0 || pb.0 > region.2 || pb.3 < region.1 || pb.1 > region.3 { continue }
            let mut cells = vec![]; scan(&gr, poly, |c| cells.push(c)); for c in cells { gr.block[c] |= COURT } }
        // copper of the net's own zones joins what stands on it
        for (l, zn, poly) in &b.fills { if zn != net { continue } let mut first = None;
            for (me, k) in own.iter().enumerate() { let it = &b.items[*k]; if it.layers & 1 << l != 0 && inside(poly, it.shape.anchor(P(0.0, 0.0))) { match first { None => first = Some(me), Some(f) => { let (a, z) = (find(&mut uf, f), find(&mut uf, me)); uf[a] = z } } } } }
        // pads of one part and number are one (a part's two pads "1")
        let roots: Vec<usize> = (0..own.len()).map(|i| find(&mut uf, i)).collect();
        let src = roots[0]; if roots.iter().all(|r| *r == src) { return Ok(None) }
        let other = (0..own.len()).find(|i| roots[*i] != src).unwrap();
        let what = format!("{} and {}", name(&b.items[own[0]], &b.layers), name(&b.items[own[other]], &b.layers));
        // the target's box, for the search's estimate
        let tb = (0..own.len()).filter(|i| roots[*i] != src).map(|i| b.items[own[i]].shape.bbox()).fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |m, x| (m.0.min(x.0), m.1.min(x.1), m.2.max(x.2), m.3.max(x.3)));
        let est = |p: P| { let (dx, dy) = ((tb.0 - p.0).max(p.0 - tb.2).max(0.0), (tb.1 - p.1).max(p.1 - tb.3).max(0.0)); (dx.max(dy) + 0.4142 * dx.min(dy)) as f32 };
        let states = n * nl * 8; let mut cost = vec![f32::MAX; states]; let mut from = vec![u32::MAX; states]; let mut how = vec![0u8; states]; let mut heap = BinaryHeap::new();
        // start: every free cell of the source group, as a move in no direction yet (kept as all eight directions at once)
        for l in 0..nl { if route & 1 << l == 0 { continue } for c in 0..n { let id = gr.own[l * n + c]; if id != 0 && roots[id as usize - 1] == src && gr.block[l * n + c] & TRACK == 0 {
            for d in 0..8 { let s = (l * n + c) * 8 + d; cost[s] = 0.0; heap.push(Node(est(gr.at(c)), s as u32)) } } } }
        if heap.is_empty() { last = format!("{what}: the net's copper there has no room for a {} mm track at {clr} mm clearance", o.width); continue }
        let (mut goal, mut pops) = (None, 0usize);
        while let Some(Node(f, s)) = heap.pop() {
            let s = s as usize; let (lc, d) = (s / 8, s % 8); let (l, c) = (lc / n, lc % n); let gc = cost[s];
            if f > gc + est(gr.at(c)) + 1e-3 { continue }
            let id = gr.own[lc]; if id != 0 && roots[id as usize - 1] != src { goal = Some(s); break }
            pops += 1; if pops > 8_000_000 { break }
            let (cx, cy) = ((c % gr.nx) as i32, (c / gr.nx) as i32);
            for (nd, (dx, dy)) in DIRS.iter().enumerate() {
                let turn = { let t = (nd as i32 - d as i32).rem_euclid(8); t.min(8 - t) };
                if turn > 2 && gc > 0.0 { continue }
                let (x, y) = (cx + dx, cy + dy); if x < 0 || y < 0 || x >= gr.nx as i32 || y >= gr.ny as i32 { continue }
                let c2 = y as usize * gr.nx + x as usize; if gr.block[l * n + c2] & TRACK != 0 { continue }
                let fl = gr.block[l * n + c2]; let stepc = if dx * dy != 0 { 1.4142 * g } else { g } as f32 * lm[l] * if fl & POUR != 0 { 3.0 } else { 1.0 } * if fl & NEAR != 0 { 1.08 } else { 1.0 };
                let nc = gc + stepc + if gc > 0.0 { [0.0, 0.08, 0.35][turn as usize] } else { 0.0 }; let s2 = (l * n + c2) * 8 + nd;
                if nc < cost[s2] { cost[s2] = nc; from[s2] = s as u32; how[s2] = 0; heap.push(Node(nc + est(gr.at(c2)), s2 as u32)) }
            }
            for (si, sp) in spans.iter().enumerate().take(250) { if l < sp.from || l > sp.to { continue }
                if (sp.from..=sp.to).any(|k| gr.block[k * n + c] & (if sp.ty == "VT_MICRO" { UVIA } else { VIA | OWNPAD }) != 0) { continue }
                for l2 in sp.from..=sp.to { if l2 == l || route & 1 << l2 == 0 || gr.block[l2 * n + c] & TRACK != 0 { continue }
                    // a micro via joins its two ends only; a through via any two of its layers
                    if sp.ty == "VT_MICRO" && !(l2 == sp.from || l2 == sp.to) { continue }
                    if sp.ty == "VT_MICRO" && !(l == sp.from || l == sp.to) { continue }
                    // a via costs its detour's worth, more the more layers it drills through
                    let nc = gc + 1.0 + 0.4 * (sp.to - sp.from) as f32 + rare[si] + if sp.ty != "VT_MICRO" && gr.block[c] & COURT != 0 { 3.0 } else { 0.0 }; for nd in 0..8 { let s2 = (l2 * n + c) * 8 + nd; if nc < cost[s2] { cost[s2] = nc; from[s2] = s as u32; how[s2] = si as u8 + 1; heap.push(Node(nc + est(gr.at(c)), s2 as u32)) } } } }
        }
        let Some(goal) = goal else { last = format!("{what}: no way for a {} mm track at {clr} mm clearance{}", o.width, if spans.is_empty() { " without vias" } else { "" }); continue };
        // the way back, as (point, layer)
        let mut path: Vec<(P, usize)> = vec![]; let mut via_at: HashMap<usize, usize> = HashMap::new(); let mut s = goal;
        loop { let lc = s / 8; path.push((gr.at(lc % n), lc / n)); if how[s] != 0 { via_at.insert(path.len() - 1, how[s] as usize - 1); } if from[s] == u32::MAX { break } s = from[s] as usize }
        // (indices counted from the goal: turned round with the path)
        let last = path.len() - 1; let via_at: HashMap<usize, usize> = via_at.into_iter().map(|(k, v)| (last - k, v)).collect();
        path.reverse();
        // each end joins the copper it stands on at that copper's own point (a pad's centre, a track's centre line) when the short way there is clear
        let clear = |a: P, z: P, l: usize| { let k = (a.dist(z) / (g / 2.0)).ceil().max(1.0) as usize;
            (0..=k).all(|i| { let p = P(a.0 + (z.0 - a.0) * i as f64 / k as f64, a.1 + (z.1 - a.1) * i as f64 / k as f64);
                b.items.iter().all(|it| it.net == net || it.layers & 1 << l == 0 || it.kind == Kind::KeepVias || it.shape.dist(p) >= o.width / 2.0 + b.clr(clr, it) - 1e-6) }) };
        let join = |(p, l): (P, usize)| -> Option<P> { let id = gr.own[l * n + (((p.1 - gr.y0) / g).round() as usize) * gr.nx + ((p.0 - gr.x0) / g).round() as usize]; if id == 0 { return None }
            let a = b.items[own[id as usize - 1]].shape.anchor(p); (a != p && clear(p, a, l)).then_some(a) };
        let (head, tail) = (join(path[0]), join(*path.last().unwrap()));
        let lead = head.is_some() as usize;
        let mut pts: Vec<(P, usize)> = vec![]; if let Some(a) = head { pts.push((a, path[0].1)) } pts.extend(path); if let Some(a) = tail { let l = pts.last().unwrap().1; pts.push((a, l)) }
        // the way as runs of points, one a layer, with the via between each two
        let mut runs: Vec<(usize, Vec<P>)> = vec![(pts[0].1, vec![pts[0].0])]; let mut hops: Vec<usize> = vec![];
        for i in 1..pts.len() { if pts[i].1 != pts[i - 1].1 { hops.push(via_at.get(&(i - lead)).copied().unwrap_or(0)); runs.push((pts[i].1, vec![pts[i].0])) } else { runs.last_mut().unwrap().1.push(pts[i].0) } }
        // pulled tight: the grid's small steps become one or two straight tracks (0, 45 or 90 degrees) wherever those keep the clearance
        // and a little over it, and no corner is made sharper than a right angle
        let pb = pts.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |m, (p, _)| (m.0.min(p.0), m.1.min(p.1), m.2.max(p.0), m.3.max(p.1)));
        let by: Vec<&Item> = b.items.iter().filter(|it| it.net != net && it.kind != Kind::KeepVias && { let ib = it.shape.bbox(); ib.0 < pb.2 + 1.0 && ib.2 > pb.0 - 1.0 && ib.1 < pb.3 + 1.0 && ib.3 > pb.1 - 1.0 }).collect();
        let free = |a: P, z: P, l: usize, extra: f64| { let k = (a.dist(z) / (g / 2.0)).ceil().max(1.0) as usize;
            (0..=k).all(|i| { let p = P(a.0 + (z.0 - a.0) * i as f64 / k as f64, a.1 + (z.1 - a.1) * i as f64 / k as f64);
                by.iter().all(|it| it.layers & 1 << l == 0 || it.shape.dist(p) >= o.width / 2.0 + if matches!(it.kind, Kind::KeepTracks) { 0.0 } else { b.clr(clr, it) + extra } - 1e-6) }) };
        let dirv = |p: P, q: P| { let d = q.sub(p); let n = d.len().max(1e-12); P(d.0 / n, d.1 / n) };
        let turn_ok = |u: Option<P>, v: P| u.map_or(true, |u| u.0 * v.0 + u.1 * v.1 >= -1e-6);
        let ways = |a: P, z: P| -> Vec<Vec<P>> { let (dx, dy) = (z.0 - a.0, z.1 - a.1);
            if dx.abs() < 1e-9 || dy.abs() < 1e-9 || (dx.abs() - dy.abs()).abs() < 1e-9 { return vec![vec![z]] }
            let m = dx.abs().min(dy.abs()); let d = P(dx.signum() * m, dy.signum() * m); vec![vec![P(a.0 + d.0, a.1 + d.1), z], vec![P(z.0 - d.0, z.1 - d.1), z]] };
        for (l, v) in runs.iter_mut() { for _pass in 0..3 { if v.len() < 3 { break } let before = v.len();
            let mut out: Vec<P> = vec![v[0]]; let mut i = 0;
            while i + 1 < v.len() {
                let prev = (out.len() >= 2).then(|| dirv(out[out.len() - 2], out[out.len() - 1])); let mut took = None;
                // (first with a little room over the clearance; where the way itself has none, at the clearance)
                'far: for extra in [0.02, 0.003] { let stride = ((v.len() - i) / 80).max(1); for j in (i + 2..v.len()).rev().step_by(stride) {
                    let after = (j + 1 < v.len()).then(|| dirv(v[j], v[j + 1]));
                    for w in ways(v[i], v[j]) { let mut a = v[i]; let mut u = prev; let mut ok = true;
                        for q in &w { if a.dist(*q) < 1e-9 { continue } let d = dirv(a, *q); if !turn_ok(u, d) || !free(a, *q, *l, extra) { ok = false; break } u = Some(d); a = *q }
                        if ok && after.map_or(true, |d| turn_ok(u, d)) { took = Some((j, w)); break 'far } } } }
                match took { Some((j, w)) => { out.extend(w); i = j } None => { out.push(v[i + 1]); i += 1 } }
            }
            out.dedup_by(|a, z| a.dist(*z) < 1e-9); *v = out; if v.len() >= before { break } } }
        let mut r = Routed::default();
        for (k, (l, v)) in runs.iter().enumerate() {
            if k > 0 { r.vias.push((v[0], spans[hops[k - 1]].clone())) }
            let mut i = 0;
            while i + 1 < v.len() { let a = v[i]; let mut j = i + 1; if a.dist(v[j]) < 1e-9 { i = j; continue } let d0 = dirv(a, v[j]);
                // run on while the direction holds
                while j + 1 < v.len() { if v[j].dist(v[j + 1]) < 1e-9 { j += 1; continue } let d1 = dirv(v[j], v[j + 1]); if (d0.0 - d1.0).abs() > 1e-6 || (d0.1 - d1.1).abs() > 1e-6 { break } j += 1 }
                r.length += a.dist(v[j]); r.tracks.push((a, v[j], *l)); i = j }
        }
        return Ok(Some(r));
    }
    Err(last)
}

// ---- a place for a part ----------------------------------------------------------------------------------------------------

/// Where a part may stand so that its pad `pad` is as near to the point `to` as it can be (at most `within` mm): no courtyard over
/// another part's, no pad nearer to other nets' copper than the clearance. Answers the part's move (dx, dy), the turn in degrees
/// about its own place and the distance pad to point; `at` is the part's place now and `turns` the rotations to try.
pub fn place(b: &Board, reference: &str, pad: &str, at: P, to: P, within: f64, turns: &[f64]) -> Result<(P, f64, f64), String> {
    let mine: Vec<&Item> = b.items.iter().filter(|i| i.kind == Kind::Pad && i.of.0 == reference).collect();
    let anchor = mine.iter().find(|i| i.of.1 == pad).ok_or(format!("{reference} has no pad `{pad}`"))?.shape.anchor(P(0.0, 0.0));
    let yards: Vec<&(String, u64, Vec<P>)> = b.courtyards.iter().filter(|c| c.0 == reference).collect();
    let reach = within + mine.iter().map(|i| { let bb = i.shape.bbox(); (bb.2 - bb.0).max(bb.3 - bb.1) }).fold(0.0, f64::max) + 0.5;
    let near: Vec<&Item> = b.items.iter().filter(|i| !(i.kind == Kind::Pad && i.of.0 == reference) && matches!(i.kind, Kind::Pad | Kind::Track | Kind::Via | Kind::Hole | Kind::Edge) && { let bb = i.shape.bbox(); bb.0 < to.0 + reach + 3.0 && bb.2 > to.0 - reach - 3.0 && bb.1 < to.1 + reach + 3.0 && bb.3 > to.1 - reach - 3.0 }).collect();
    let step = 0.05; let k = (within / step).ceil() as i32; let mut cands: Vec<(f64, f64, P)> = vec![];
    for t in turns { for iy in -k..=k { for ix in -k..=k { let goal = P(to.0 + ix as f64 * step, to.1 + iy as f64 * step); let d = goal.dist(to); if d > within { continue }
        // the pad lands on `goal`: the part turns about `at`, then moves
        let (s, co) = t.to_radians().sin_cos(); let rel = anchor.sub(at); let turned = P(at.0 + rel.0 * co + rel.1 * s, at.1 - rel.0 * s + rel.1 * co);
        cands.push((d, *t, goal.sub(turned))) } } }
    cands.sort_by(|a, z| a.0.partial_cmp(&z.0).unwrap());
    let mut why: HashMap<String, usize> = HashMap::new();
    'c: for (_, t, by0) in cands {
        // a courtyard that overlaps another is pushed out of it the shortest way (a slot between two parts is found so), six times at most
        let mut by = by0; let mut last: Option<P> = None;
        for y in &yards { for round in 0..7 { let mv: Vec<P> = Shape::Poly(y.2.clone()).moved(at, t, by).corners();
            let Some((o, out)) = b.courtyards.iter().filter(|o| o.0 != reference && o.1 & y.1 != 0).find_map(|o| push_out(&mv, &o.2).map(|v| (o, v))) else { break };
            if round == 6 { *why.entry(format!("its courtyard would overlap {}'s", o.0)).or_default() += 1; continue 'c }
            let l = out.len().max(1e-9); last = Some(P(out.0 / l, out.1 / l)); by = P(by.0 + out.0 * (1.0 + 0.002 / l), by.1 + out.1 * (1.0 + 0.002 / l)) } }
        // pushed out of one part and close to the next (a slot): stand in the slot's middle, where the pads' gaps are even
        if let (Some(u), Some(y)) = (last, yards.first()) { let free = |s: f64| { let mv: Vec<P> = Shape::Poly(y.2.clone()).moved(at, t, P(by.0 + u.0 * s, by.1 + u.1 * s)).corners(); !b.courtyards.iter().any(|o| o.0 != reference && o.1 & y.1 != 0 && overlap(&mv, &o.2)) };
            let room = (1..=100).map(|k| k as f64 * 0.001).take_while(|s| free(*s)).last().unwrap_or(0.0); if room < 0.0995 { by = P(by.0 + u.0 * room / 2.0, by.1 + u.1 * room / 2.0) } }
        let d = { let (s, co) = t.to_radians().sin_cos(); let rel = anchor.sub(at); P(at.0 + rel.0 * co + rel.1 * s + by.0, at.1 - rel.0 * s + rel.1 * co + by.1).dist(to) };
        if d > within { *why.entry("the free places next to it are farther".to_string()).or_default() += 1; continue }
        // a pad too near other copper moves off it by what is missing (a track between a part's own pads leaves a few hundredths), five times at most
        let yard_free = |by: P| yards.iter().all(|y| { let mv: Vec<P> = Shape::Poly(y.2.clone()).moved(at, t, by).corners(); !b.courtyards.iter().any(|o| o.0 != reference && o.1 & y.1 != 0 && overlap(&mv, &o.2)) });
        let worst = |by: P| -> Option<(f64, P, String)> { let mut w: Option<(f64, P, String)> = None;
            for p in &mine { let sh = p.shape.moved(at, t, by); let bb = sh.bbox(); let own = b.net_clearance(&p.net); let mut pts: Option<Vec<P>> = None;
                for o in &near { if o.layers & p.layers == 0 || (o.net == p.net && !o.net.is_empty() && o.kind != Kind::Hole && o.kind != Kind::Edge) { continue }
                    let need = b.clr(own, o) + 0.0005; let ob = o.shape.bbox(); if ob.0 > bb.2 + need || ob.2 < bb.0 - need || ob.1 > bb.3 + need || ob.3 < bb.1 - need { continue }
                    let pts = pts.get_or_insert_with(|| sh.samples(0.04));
                    for q in pts.iter().copied().chain([o.shape.anchor(pts[0])].into_iter().filter(|a| sh.dist(*a) == 0.0)) { let d0 = o.shape.dist(q); if d0 < need && w.as_ref().map_or(true, |x| need - d0 > x.0) {
                        let e = 1e-4; let gr = P(o.shape.dist(P(q.0 + e, q.1)) - o.shape.dist(P(q.0 - e, q.1)), o.shape.dist(P(q.0, q.1 + e)) - o.shape.dist(P(q.0, q.1 - e))); let l = gr.len();
                        w = Some((need - d0, if l > 1e-12 { P(gr.0 / l, gr.1 / l) } else { P(0.0, 0.0) }, format!("its pad {} would be too near {}", p.of.1, name(o, &b.layers)))) } } } }
            w };
        for round in 0..6 { match worst(by) { None => break,
            Some((miss, dir, what)) => { if round == 5 || miss > 0.08 || dir == P(0.0, 0.0) { *why.entry(what).or_default() += 1; continue 'c }
                by = P(by.0 + dir.0 * (miss + 0.001), by.1 + dir.1 * (miss + 0.001)); if !yard_free(by) { *why.entry(what).or_default() += 1; continue 'c } } } }
        let d = { let (s, co) = t.to_radians().sin_cos(); let rel = anchor.sub(at); P(at.0 + rel.0 * co + rel.1 * s + by.0, at.1 - rel.0 * s + rel.1 * co + by.1).dist(to) };
        if d > within { *why.entry("the free places next to it are farther".to_string()).or_default() += 1; continue }
        return Ok((by, t, d));
    }
    let mut top: Vec<(String, usize)> = why.into_iter().collect(); top.sort_by(|a, z| z.1.cmp(&a.1));
    Err(format!("{reference}: no place within {within} mm of that point ({}): give a larger `within`, or move it yourself", top.iter().take(if std::env::var_os("KROUTE_DEBUG").is_some() { 12 } else { 3 }).map(|x| if std::env::var_os("KROUTE_DEBUG").is_some() { format!("{} x{}", x.0, x.1) } else { x.0.clone() }).collect::<Vec<_>>().join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pad(r: &str, n: &str, net: &str, c: P, hx: f64, hy: f64, deg: f64) -> Item { Item { shape: Shape::Rect(c, hx, hy, deg, 0.0), layers: 1, net: net.into(), kind: Kind::Pad, of: (r.into(), n.into()), smd: true } }
    fn board(items: Vec<Item>) -> Board { Board { layers: vec!["F.Cu".into(), "B.Cu".into()], items, min_clearance: 0.15, spans: vec![Span { from: 0, to: 1, ty: "VT_THROUGH".into(), size: 0.5, drill: 0.3, n: 1, like: Value::Null }], ..Default::default() } }
    fn opts() -> Opts { Opts { width: 0.2, layers: vec![], vias: true, via: None, planes: false } }
    /// every routed track keeps its clearance to every other net's copper (the model's own check, exact)
    fn clean(b: &Board, net: &str, r: &Routed, w: f64) -> bool {
        r.tracks.iter().all(|(a, z, l)| Shape::Capsule(*a, *z, 0.0).samples(0.01).iter().all(|p| b.items.iter().all(|it| it.net == net || it.layers & 1 << l == 0 || it.shape.dist(*p) >= w / 2.0 + 0.15 - 1e-6)))
    }
    #[test] fn shapes() {
        assert!((Shape::Rect(P(0.0, 0.0), 1.0, 0.5, 0.0, 0.0).dist(P(2.0, 0.0)) - 1.0).abs() < 1e-9);
        // KiCad's angle: 90 degrees turns the long side onto y
        assert!(Shape::Rect(P(0.0, 0.0), 1.0, 0.1, 90.0, 0.0).dist(P(0.0, 0.9)) == 0.0 && Shape::Rect(P(0.0, 0.0), 1.0, 0.1, 90.0, 0.0).dist(P(0.9, 0.0)) > 0.7);
        // 45 degrees, counter-clockwise on a screen whose y runs down: the long side points up and to the right
        assert!(Shape::Rect(P(0.0, 0.0), 1.0, 0.1, 45.0, 0.0).dist(P(0.6, -0.6)) == 0.0 && Shape::Rect(P(0.0, 0.0), 1.0, 0.1, 45.0, 0.0).dist(P(0.6, 0.6)) > 0.5);
        assert!((Shape::Capsule(P(0.0, 0.0), P(1.0, 0.0), 0.1).dist(P(0.5, 0.3)) - 0.2).abs() < 1e-9);
        // a rounded corner is farther from a point off the corner than a square one, and its outline's samples are on it
        let rr = Shape::Rect(P(0.0, 0.0), 1.0, 0.5, 0.0, 0.2); assert!((rr.dist(P(2.0, 0.0)) - 1.0).abs() < 1e-9 && rr.dist(P(1.1, 0.6)) > 0.2 && rr.dist(P(0.9, 0.4)) == 0.0);
        assert!(rr.samples(0.1).iter().all(|q| rr.dist(*q) < 1e-9) && rr.samples(0.1).iter().any(|q| (q.0 - 1.0).abs() < 1e-9));
        let a = arc_pts(P(1.0, 0.0), P(0.0, 1.0), P(-1.0, 0.0), 4); assert!(a.iter().all(|p| (p.len() - 1.0).abs() < 1e-9) && a[2].dist(P(0.0, 1.0)) < 1e-9);
        assert_eq!(hull(&[P(0.0, 0.0), P(1.0, 0.0), P(1.0, 1.0), P(0.0, 1.0), P(0.5, 0.5)]).len(), 4);
        let sq = |x: f64| vec![P(x, 0.0), P(x + 1.0, 0.0), P(x + 1.0, 1.0), P(x, 1.0)];
        assert!(overlap(&sq(0.0), &sq(0.5)) && !overlap(&sq(0.0), &sq(1.2)));
    }
    #[test] fn routes_two_pads_around_a_third() {
        let mut b = board(vec![pad("R1", "1", "A", P(0.0, 0.0), 0.2, 0.2, 0.0), pad("R2", "1", "A", P(4.0, 0.0), 0.2, 0.2, 0.0), pad("X", "1", "B", P(2.0, 0.0), 0.3, 1.0, 0.0)]);
        let r = route_net(&mut b, "A", &Opts { vias: false, ..opts() }).unwrap();
        assert!(r.vias.is_empty() && r.tracks.len() >= 3 && r.length > 4.5 && r.length < 9.0, "{r:?}");
        assert!(clean(&b, "A", &r, 0.2));
        // the ends are the pads' centres, and all of it is one chain
        assert_eq!(r.tracks[0].0, P(0.0, 0.0)); assert_eq!(r.tracks.last().unwrap().1, P(4.0, 0.0));
        assert!(r.tracks.windows(2).all(|w| w[0].1 == w[1].0));
        // routed copper is on the board: the net is now one group
        assert_eq!(route_net(&mut b, "A", &opts()).unwrap(), Routed::default());
    }
    #[test] fn a_wall_needs_a_via_and_without_one_it_says_so() {
        let wall = Item { shape: Shape::Capsule(P(2.0, -30.0), P(2.0, 30.0), 0.1), layers: 1, net: "B".into(), kind: Kind::Track, of: Default::default(), smd: false };
        let items = vec![pad("R1", "1", "A", P(0.0, 0.0), 0.2, 0.2, 0.0), pad("R2", "1", "A", P(4.0, 0.0), 0.2, 0.2, 0.0), wall];
        let mut b = board(items.clone());
        let r = route_net(&mut b, "A", &opts()).unwrap();
        assert_eq!(r.vias.len(), 2, "{r:?}"); assert!(r.tracks.iter().any(|t| t.2 == 1)); assert!(clean(&b, "A", &r, 0.2));
        let e = route_net(&mut board(items), "A", &Opts { vias: false, ..opts() }).unwrap_err().1;
        assert!(e.contains("R1.1 and R2.1") && e.contains("without vias"), "{e}");
    }
    #[test] fn the_boards_own_vias_and_straight_runs() {
        // nothing in the way: one diagonal and one straight track, not the grid's steps
        let mut b = board(vec![pad("R1", "1", "A", P(0.0, 0.0), 0.2, 0.2, 0.0), pad("R2", "1", "A", P(5.0, 2.0), 0.2, 0.2, 0.0)]);
        let r = route_net(&mut b, "A", &Opts { vias: false, ..opts() }).unwrap();
        assert!(r.tracks.len() <= 2 && r.length < 5.9, "{r:?}");
        // a board of laser micro vias (four layers, a through via used once): the wall is passed by micro vias, never the drilled one
        let wall = Item { shape: Shape::Capsule(P(2.0, -30.0), P(2.0, 30.0), 0.1), layers: 1, net: "B".into(), kind: Kind::Track, of: Default::default(), smd: false };
        let sp = |from, to, ty: &str, n| Span { from, to, ty: ty.into(), size: 0.25, drill: 0.1, n, like: Value::Null };
        let mut b = Board { layers: ["F.Cu", "In1.Cu", "In2.Cu", "B.Cu"].iter().map(|l| l.to_string()).collect(), items: vec![pad("R1", "1", "A", P(0.0, 0.0), 0.2, 0.2, 0.0), pad("R2", "1", "A", P(4.0, 0.0), 0.2, 0.2, 0.0), wall],
            min_clearance: 0.15, spans: vec![sp(0, 1, "VT_MICRO", 300), sp(1, 2, "VT_MICRO", 200), sp(0, 3, "VT_THROUGH", 1)], ..Default::default() };
        let r = route_net(&mut b, "A", &opts()).unwrap();
        assert!(r.vias.len() == 2 && r.vias.iter().all(|v| v.1.ty == "VT_MICRO") && r.tracks.iter().all(|t| t.2 <= 1), "{r:?}");
        // the same board with through vias only: the drilled via is what there is
        let mut b2 = board(vec![pad("R1", "1", "A", P(0.0, 0.0), 0.2, 0.2, 0.0), pad("R2", "1", "A", P(4.0, 0.0), 0.2, 0.2, 0.0), Item { shape: Shape::Capsule(P(2.0, -30.0), P(2.0, 30.0), 0.1), layers: 1, net: "B".into(), kind: Kind::Track, of: Default::default(), smd: false }]);
        assert!(route_net(&mut b2, "A", &opts()).unwrap().vias.iter().all(|v| v.1.ty == "VT_THROUGH"));
    }
    #[test] fn three_pads_and_a_track_already_there() {
        let t = Item { shape: Shape::Capsule(P(0.0, 0.0), P(0.0, 2.0), 0.1), layers: 1, net: "A".into(), kind: Kind::Track, of: Default::default(), smd: false };
        let mut b = board(vec![pad("R1", "1", "A", P(0.0, 0.0), 0.2, 0.2, 0.0), pad("R2", "1", "A", P(3.0, 2.0), 0.2, 0.2, 0.0), pad("R3", "1", "A", P(-3.0, 2.0), 0.2, 0.2, 0.0), t]);
        let r = route_net(&mut b, "A", &Opts { vias: false, ..opts() }).unwrap();
        // both pads join the track's end rather than run back to R1
        assert!(r.length < 6.6, "{r:?}");
        assert!(route_net(&mut b, "Z", &opts()).unwrap_err().1.contains("no pad"));
    }
    #[test] fn between_fine_pitch_pads_at_45_degrees() {
        // a QFN's row turned 45 degrees: 0.5 mm pitch, pads 0.3 wide; the middle pad's track leaves along the pad
        let dir = P(0.7071, -0.7071); let across = P(0.7071, 0.7071);
        let mut items: Vec<Item> = (0..5).map(|i| { let k = i as f64 - 2.0; pad("U1", &i.to_string(), if i == 2 { "A" } else { "B" }, P(across.0 * 0.5 * k, across.1 * 0.5 * k), 0.7, 0.15, 45.0) }).collect();
        items.push(pad("R1", "1", "A", P(dir.0 * 3.0, dir.1 * 3.0), 0.2, 0.25, 45.0));
        let mut b = board(items); let r = route_net(&mut b, "A", &Opts { vias: false, ..opts() }).unwrap();
        assert!(clean(&b, "A", &r, 0.2) && r.length < 3.6, "{r:?}");
    }
    #[test] fn a_place_by_a_pin() {
        // U1's pin at the origin with neighbours 0.5 mm either side; C1 (two pads 1 mm apart) parked far away
        let mut b = board(vec![pad("U1", "1", "V", P(0.0, 0.0), 0.15, 0.4, 0.0), pad("U1", "2", "X", P(0.5, 0.0), 0.15, 0.4, 0.0), pad("U1", "3", "Y", P(-0.5, 0.0), 0.15, 0.4, 0.0),
            pad("C1", "1", "V", P(20.0, 20.0), 0.25, 0.3, 0.0), pad("C1", "2", "GND", P(21.0, 20.0), 0.25, 0.3, 0.0)]);
        b.courtyards = vec![("C1".into(), 1, vec![P(19.5, 19.5), P(21.5, 19.5), P(21.5, 20.5), P(19.5, 20.5)]), ("U1".into(), 1, vec![P(-1.0, -0.6), P(1.0, -0.6), P(1.0, 0.6), P(-1.0, 0.6)])];
        let (by, turn, d) = place(&b, "C1", "1", P(20.5, 20.0), P(0.0, 0.0), 2.0, &[0.0, 90.0, 180.0, 270.0]).unwrap();
        assert!(d <= 2.0 && d > 0.5, "{d}");
        // where it lands: courtyard clear of U1's, pads clear of X and Y
        let moved: Vec<Shape> = b.items.iter().filter(|i| i.of.0 == "C1").map(|i| i.shape.moved(P(20.5, 20.0), turn, by)).collect();
        for s in &moved { for o in b.items.iter().filter(|i| i.of.0 == "U1" && i.net != "V") { assert!(s.samples(0.05).iter().all(|q| o.shape.dist(*q) >= 0.15)) } }
        assert!(moved[0].anchor(P(0.0, 0.0)).dist(P(0.0, 0.0)) <= 2.0 + 1e-9);
        assert!(place(&b, "C1", "1", P(20.5, 20.0), P(0.0, 0.0), 0.3, &[0.0]).unwrap_err().contains("no place within 0.3"));
        assert!(place(&b, "C1", "9", P(20.5, 20.0), P(0.0, 0.0), 2.0, &[0.0]).is_err());
    }
}

//! A .kicad_pcb's bytes to the shapes of its 3D model (pcb3d::Board). The file is cut into its top-level forms (sexpr::forms, which
//! also says whether the file is whole), the forms parse in parallel in chunks, and the parts join in file order.
//!
//! Conventions (see mod.rs): mm, x right, y down, angles in degrees, positive turns x towards -y (KiCad's own). A footprint's
//! children are stored in its frame (already flipped for a back footprint) and are placed by its position and angle; a pad's `(at x y a)`
//! angle is absolute (the footprint's angle is already in it: PAD::SetOrientation, pcb_io_kicad_sexpr_parser.cpp), so a pad turns by `a`
//! alone. (board.rs's 2D reader builds the pad frame with `rad(-a)`, which turns the other way; only a rect at 45 or 135 degrees shows it.)
use super::super::board as b2;
use super::super::sexpr::{self, Sx};
use super::super::strokefont;
use super::*;
use rayon::prelude::*;

// ---- numbers and placement

fn num(s: &str) -> f64 { match s.trim().parse::<f64>() { Ok(v) if v.is_finite() => v, _ => 0.0 } }
fn nf(x: &Sx, i: usize) -> f64 { num(x.arg(i)) }
fn pt(x: &Sx) -> P2 { P2::new(nf(x, 0), nf(x, 1)) }
fn at(x: &Sx, name: &str) -> P2 { pt(x.get(name)) }
fn xyz(x: &Sx, d: f64) -> [f64; 3] { let v = x.get("xyz"); if matches!(v, Sx::End) { [d; 3] } else { [nf(v, 0), nf(v, 1), nf(v, 2)] } }

/// A placement: origin and angle, the sine and cosine worked once (exact at the right angles).
#[derive(Clone, Copy, Debug)]
struct Tf { x: f64, y: f64, c: f64, s: f64 }
fn cs(deg: f64) -> (f64, f64) {
    let r = deg.rem_euclid(360.0);
    if r == 0.0 { (1.0, 0.0) } else if r == 90.0 { (0.0, 1.0) } else if r == 180.0 { (-1.0, 0.0) } else if r == 270.0 { (0.0, -1.0) }
    else { let a = deg.to_radians(); (a.cos(), a.sin()) }
}
impl Tf {
    fn new(x: f64, y: f64, deg: f64) -> Tf { let (c, s) = cs(deg); Tf { x, y, c, s } }
    fn at(&self, p: P2) -> P2 { P2::new(p.x * self.c + p.y * self.s + self.x, p.y * self.c - p.x * self.s + self.y) }
}
const HOME: Tf = Tf { x: 0.0, y: 0.0, c: 1.0, s: 0.0 };
/// p turned by deg (the footprint convention), about the origin
fn turn(p: P2, deg: f64) -> P2 { Tf::new(0.0, 0.0, deg).at(p) }

/// 10.99 writes a footprint's origin and angle as `(transform (translate x y) (rotate a))` and it wins over `(at x y a)`.
fn fp_place(x: &Sx) -> (Tf, P2, f64) {
    let tr = x.get("transform");
    let (p, a) = if matches!(tr, Sx::End) { let a = x.get("at"); (P2::new(nf(a, 0), nf(a, 1)), nf(a, 2)) } else { let t = tr.get("translate"); (P2::new(nf(t, 0), nf(t, 1)), num(tr.attr("rotate"))) };
    (Tf::new(p.x, p.y, a), p, a)
}

/// a bare word among the arguments (hide, dnp, ...), or `(name yes)`
fn flag(x: &Sx, name: &str) -> bool { x.args().iter().any(|c| c.text() == name || (c.name() == name && c.arg(0) != "no")) }
fn width(x: &Sx) -> f64 { let w = nf(x.get("width"), 0); if w > 0.0 { w } else { nf(x.get("stroke").get("width"), 0) } }
fn filled(x: &Sx) -> bool { matches!(x.get("fill").arg(0), "yes" | "solid") || flag(x.get("fill"), "yes") }
fn prop<'s>(x: &'s Sx, k: &str) -> &'s str { x.args().iter().find(|c| c.name() == "property" && c.arg(0) == k).map(|c| c.arg(1)).unwrap_or("") }

// ---- destinations

#[derive(Default)]
struct Acc {
    outline: Vec<Shape>,
    copper: Vec<(Side, Shape)>,
    silk: Vec<(Side, Shape)>,
    mask: Vec<(Side, Shape)>,
    crtyd: Vec<Shape>,
    fab: Vec<Shape>,
}
impl Acc {
    fn put(&mut self, layer: &str, s: Shape) {
        match layer {
            "Edge.Cuts" => self.outline.push(s),
            "F.Cu" => self.copper.push((Side::Top, s)), "B.Cu" => self.copper.push((Side::Bottom, s)),
            "F.SilkS" => self.silk.push((Side::Top, s)), "B.SilkS" => self.silk.push((Side::Bottom, s)),
            "F.Mask" => self.mask.push((Side::Top, s)), "B.Mask" => self.mask.push((Side::Bottom, s)),
            "F.CrtYd" | "B.CrtYd" => self.crtyd.push(s),
            "F.Fab" | "B.Fab" => self.fab.push(s),
            _ => {}
        }
    }
    fn wants(layer: &str) -> bool { matches!(layer, "Edge.Cuts" | "F.Cu" | "B.Cu" | "F.SilkS" | "B.SilkS" | "F.Mask" | "B.Mask" | "F.CrtYd" | "B.CrtYd" | "F.Fab" | "B.Fab") }
    fn append(&mut self, o: Acc) {
        self.outline.extend(o.outline); self.copper.extend(o.copper); self.silk.extend(o.silk); self.mask.extend(o.mask);
        self.crtyd.extend(o.crtyd); self.fab.extend(o.fab);
    }
}

// ---- graphics

/// The circle through a, m, b sampled from a to b (n segments); a straight pair when they are collinear.
fn arc_pts(a: P2, m: P2, b: P2, n: usize) -> Vec<P2> {
    let d = 2.0 * ((m.x - a.x) * (b.y - a.y) - (m.y - a.y) * (b.x - a.x));
    if d.abs() < 1e-12 { return vec![a, b] }
    let (px, py, qx, qy) = (m.x - a.x, m.y - a.y, b.x - a.x, b.y - a.y);
    let (p2, q2) = (px * px + py * py, qx * qx + qy * qy);
    let (ux, uy) = (a.x + (qy * p2 - py * q2) / d, a.y + (px * q2 - qx * p2) / d);
    let tau = std::f64::consts::TAU;
    let norm = |v: f64| v - tau * (v / tau).floor();
    let a0 = (a.y - uy).atan2(a.x - ux);
    let s1 = norm((b.y - uy).atan2(b.x - ux) - a0);
    let sm = norm((m.y - uy).atan2(m.x - ux) - a0);
    let r = (a.x - ux).hypot(a.y - uy);
    let sw = if sm < s1 { s1 } else { s1 - tau };
    (0..=n).map(|i| { let t = a0 + sw * (i as f64 / n as f64); P2::new(ux + r * t.cos(), uy + r * t.sin()) }).collect()
}
/// A `(pts ...)` list as a ring: `xy` points, `arc` items sampled.
fn ring(pts: &Sx, tf: &Tf) -> Vec<P2> {
    let mut o = Vec::with_capacity(pts.args().len());
    for c in pts.args() {
        match c.name() {
            "xy" => o.push(tf.at(pt(c))),
            "arc" => { let v = arc_pts(at(c, "start"), at(c, "mid"), at(c, "end"), 12); for p in v.into_iter().skip(1) { o.push(tf.at(p)) } }
            _ => {}
        }
    }
    o
}
fn bezier(p: [P2; 4], n: usize) -> Vec<P2> {
    (0..=n).map(|i| {
        let t = i as f64 / n as f64; let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        P2::new(a * p[0].x + b * p[1].x + c * p[2].x + d * p[3].x, a * p[0].y + b * p[1].y + c * p[2].y + d * p[3].y)
    }).collect()
}
/// The shapes one drawn form makes (tracks, gr_* and fp_* graphics), placed by tf.
fn gfx(x: &Sx, tf: &Tf, out: &mut Vec<Shape>) {
    let w = width(x);
    match x.name() {
        "segment" | "gr_line" | "fp_line" => out.push(Shape::Seg { a: tf.at(at(x, "start")), b: tf.at(at(x, "end")), w }),
        "arc" | "gr_arc" | "fp_arc" => { if !matches!(x.get("mid"), Sx::End) { out.push(Shape::Arc { a: tf.at(at(x, "start")), m: tf.at(at(x, "mid")), b: tf.at(at(x, "end")), w }) } }
        "gr_circle" | "fp_circle" => { let (c, e) = (at(x, "center"), at(x, "end")); out.push(Shape::Circle { c: tf.at(c), r: (e.x - c.x).hypot(e.y - c.y), w, fill: filled(x) }) }
        "gr_rect" | "fp_rect" => {
            let (a, b) = (at(x, "start"), at(x, "end"));
            out.push(Shape::Poly { pts: vec![tf.at(a), tf.at(P2::new(b.x, a.y)), tf.at(b), tf.at(P2::new(a.x, b.y))], w, fill: filled(x) });
        }
        "gr_poly" | "fp_poly" => { let pts = ring(x.get("pts"), tf); if pts.len() >= 2 { out.push(Shape::Poly { pts, w, fill: filled(x) }) } }
        "gr_curve" | "fp_curve" => {
            let q: Vec<P2> = x.get("pts").args().iter().filter(|c| c.name() == "xy").map(pt).collect();
            if q.len() >= 4 {
                let v: Vec<P2> = bezier([q[0], q[1], q[2], q[3]], 24).into_iter().map(|p| tf.at(p)).collect();
                for p in v.windows(2) { out.push(Shape::Seg { a: p[0], b: p[1], w }) }
            }
        }
        _ => {}
    }
}
fn is_gfx(name: &str) -> bool {
    matches!(name, "segment" | "arc") || ((name.starts_with("gr_") || name.starts_with("fp_")) && matches!(&name[3..], "line" | "arc" | "circle" | "rect" | "poly" | "curve"))
}

// ---- text

fn norm_deg(a: f64) -> f64 { a - 360.0 * ((a + 179.999) / 360.0).floor() }
/// Text as strokes (the 2D reader's rules: hide flags, the keep-upright turn of footprint text, justification, mirror).
fn draw(acc: &mut Acc, s: &str, p: P2, a: f64, x: &Sx, up: bool) {
    let layer = x.get("layer");
    if s.is_empty() || flag(x, "hide") || flag(x.get("effects"), "hide") || !Acc::wants(layer.arg(0)) || layer.args().iter().any(|c| c.text() == "knockout") { return }
    let (fx, ly) = (x.get("effects"), layer.arg(0));
    let (font, j) = (fx.get("font"), fx.get("justify"));
    let (h, w, th) = (nf(font.get("size"), 0), nf(font.get("size"), 1), nf(font.get("thickness"), 0));
    let pen = if th > 0.0 { th } else { h / 8.0 };
    let na = norm_deg(a);
    let unlock = flag(x, "unlocked") || flag(x.get("at"), "unlocked");
    let turn = up && !unlock && (na > 90.001 || na <= -89.999);
    let hj = if flag(j, "left") { 0 } else if flag(j, "right") { 2 } else { 1 };
    let vj = if flag(j, "top") { 0 } else if flag(j, "bottom") { 2 } else { 1 };
    let ang = (if turn { na + 180.0 } else { na }).to_radians();
    let ps = strokefont::text_full(s, p.x as f32, p.y as f32, h as f32, w as f32, pen as f32, ang as f32,
        if turn { 2u32.wrapping_sub(hj) } else { hj }, if turn { 2u32.wrapping_sub(vj) } else { vj }, flag(j, "mirror"), flag(font, "italic"));
    let q = |p: &b2::Pt| P2::new(p.x as f64, p.y as f64);
    for prim in ps {
        match prim {
            b2::Prim::Seg { a, b, w } => acc.put(ly, Shape::Seg { a: q(&a), b: q(&b), w: w as f64 }),
            b2::Prim::Line { ps, w } => {
                if ps.len() == 1 { acc.put(ly, Shape::Seg { a: q(&ps[0]), b: q(&ps[0]), w: w as f64 }) }
                for k in ps.windows(2) { acc.put(ly, Shape::Seg { a: q(&k[0]), b: q(&k[1]), w: w as f64 }) }
            }
            b2::Prim::Circ { c, r, w, fill } => acc.put(ly, Shape::Circle { c: q(&c), r: r as f64, w: w as f64, fill }),
            b2::Prim::Poly { ps, w, fill } => acc.put(ly, Shape::Poly { pts: ps.iter().map(q).collect(), w: w as f64, fill }),
            b2::Prim::Tint { .. } => {}
        }
    }
}
/// A text box's text, drawn at the middle of its box (borders are not drawn).
fn text_box(acc: &mut Acc, x: &Sx, tf: &Tf, up: bool) {
    let pts: Vec<P2> = if matches!(x.get("pts"), Sx::End) { vec![at(x, "start"), at(x, "end")] } else { x.get("pts").args().iter().filter(|c| c.name() == "xy").map(pt).collect() };
    if pts.is_empty() { return }
    let n = pts.len() as f64;
    let (cx, cy) = (pts.iter().map(|p| p.x).sum::<f64>() / n, pts.iter().map(|p| p.y).sum::<f64>() / n);
    let a = nf(x.get("angle"), 0);
    draw(acc, x.arg(0), tf.at(P2::new(cx, cy)), a, x, up);
}
fn expand(s: &str, r: &str, v: &str) -> String { if s.contains("${") { s.replace("${REFERENCE}", r).replace("${VALUE}", v) } else { s.to_string() } }

// ---- pads

#[derive(Clone, Copy, PartialEq)]
enum Part2 { Both, Front, Back }
fn pad_layers(x: &Sx, ty: &str) -> PadLayers {
    let mut l = PadLayers::default();
    for n in x.get("layers").args() {
        match n.text() {
            "*.Cu" | "F&B.Cu" => { l.cu_top = true; l.cu_bottom = true }
            "F.Cu" => l.cu_top = true, "B.Cu" => l.cu_bottom = true,
            "*.Mask" | "F&B.Mask" => { l.mask_top = true; l.mask_bottom = true }
            "F.Mask" => l.mask_top = true, "B.Mask" => l.mask_bottom = true,
            _ => {}
        }
    }
    let _ = ty;
    l
}
/// `key` from the layer's own entry of a 10.99 padstack, else from the pad.
fn pick<'s, 'a>(spec: &'s Sx<'a>, pad: &'s Sx<'a>, key: &str) -> &'s Sx<'a> { let v = spec.get(key); if matches!(v, Sx::End) { pad.get(key) } else { v } }

/// The copper outline one side of a pad has, board coordinates. `spec` is the padstack's layer entry (End for a plain pad).
fn pad_shapes(pad: &Sx, spec: &Sx, tf: &Tf) -> Vec<Shape> {
    let a = nf(pad.get("at"), 2);
    let shape = { let s = spec.get("shape").arg(0); if s.is_empty() { pad.arg(2) } else { s } };
    let size = pick(spec, pad, "size");
    let (w, h) = (nf(size, 0), nf(size, 1));
    let off = pick(spec, pad, "offset");
    let local = pt(pad.get("at"));
    let c = tf.at(local);
    let c = if matches!(off, Sx::End) { c } else { let o = turn(pt(off), a); P2::new(c.x + o.x, c.y + o.y) };
    let ptf = Tf::new(c.x, c.y, a);
    let rect = |r: f64, ch: f64, corners: u8| Shape::Rect { c, w, h, rot: a, r, chamfer: ch, chamfer_corners: corners };
    let chamfer_of = || -> (f64, u8) {
        let ratio = nf(pick(spec, pad, "chamfer_ratio"), 0);
        let mut bits = 0u8;
        for k in pick(spec, pad, "chamfer").args() {
            match k.text() { "top_left" => bits |= 1, "top_right" => bits |= 2, "bottom_left" => bits |= 4, "bottom_right" => bits |= 8, _ => {} }
        }
        if ratio > 0.0 && bits != 0 { (ratio * w.min(h), bits) } else { (0.0, 0) }
    };
    match shape {
        "circle" => { if w > 0.0 { vec![Shape::Circle { c, r: w / 2.0, w: 0.0, fill: true }] } else { vec![] } }
        "rect" if w > 0.0 && h > 0.0 => { let (ch, bits) = chamfer_of(); vec![rect(0.0, ch, bits)] }
        "oval" if w > 0.0 && h > 0.0 => vec![rect(w.min(h) / 2.0, 0.0, 0)],
        "roundrect" if w > 0.0 && h > 0.0 => { let (ch, bits) = chamfer_of(); vec![rect(nf(pick(spec, pad, "roundrect_rratio"), 0) * w.min(h), ch, bits)] }
        "trapezoid" if w > 0.0 && h > 0.0 => {
            let d = pick(spec, pad, "rect_delta");
            let (dx, dy) = (nf(d, 0) / 2.0, nf(d, 1) / 2.0);
            let (hx, hy) = (w / 2.0, h / 2.0);
            let p = [P2::new(-hx - dy, hy + dx), P2::new(hx + dy, hy - dx), P2::new(hx - dy, -hy + dx), P2::new(-hx + dy, -hy - dx)];
            vec![Shape::Poly { pts: p.iter().map(|p| ptf.at(*p)).collect(), w: 0.0, fill: true }]
        }
        "custom" => {
            let mut v = vec![];
            let anchor = { let o = pick(spec, pad, "options"); o.attr("anchor").to_string() };
            if w > 0.0 {
                if anchor == "rect" && h > 0.0 { v.push(rect(0.0, 0.0, 0)) } else { v.push(Shape::Circle { c, r: w / 2.0, w: 0.0, fill: true }) }
            }
            for p in pick(spec, pad, "primitives").args() { if is_gfx(p.name()) { gfx(p, &ptf, &mut v) } }
            v
        }
        _ => vec![],
    }
}
fn drill_of(pad: &Sx, tf: &Tf, plated: bool) -> Option<Drill> {
    let d = pad.get("drill");
    if matches!(d, Sx::End) { return None }
    let (w, h) = if d.arg(0) == "oval" { (nf(d, 1), nf(d, 2)) } else { (nf(d, 0), nf(d, 0)) };
    if w <= 0.0 || h <= 0.0 { return None }
    let a = nf(pad.get("at"), 2);
    let o = turn(pt(d.get("offset")), a);
    let c = tf.at(pt(pad.get("at")));
    Some(Drill { c: P2::new(c.x + o.x, c.y + o.y), w, h, rot: a, plated })
}

fn pads(out: &mut Vec<Pad>, pad: &Sx, tf: &Tf, fp_margin: Option<f64>, reference: &str) {
    let ty = pad.arg(1);
    let layers = pad_layers(pad, ty);
    let drill = drill_of(pad, tf, ty != "np_thru_hole");
    let margin = match nf_opt(pad.get("solder_mask_margin")).or(fp_margin) { Some(m) => m, None => f64::NAN };
    let ps = pad.get("padstack");
    let layer_entry = |n: &str| -> &Sx { ps.args().iter().find(|c| c.name() == "layer" && c.arg(0) == n).unwrap_or(&Sx::End) };
    let (fe, be) = (layer_entry("F.Cu"), layer_entry("B.Cu"));
    let front = pad_shapes(pad, fe, tf);
    let back = if matches!(be, Sx::End) && matches!(fe, Sx::End) { front.clone() } else { pad_shapes(pad, be, tf) };
    let which = if layers.cu_top && layers.cu_bottom && front != back { Part2::Both } else { Part2::Front };
    let mk = |shapes: Vec<Shape>, l: PadLayers, drill: Option<Drill>| Pad { shapes, layers: l, drill, mask_margin: margin, reference: reference.to_string() };
    match which {
        Part2::Both => {
            out.push(mk(front, PadLayers { cu_top: true, cu_bottom: false, mask_top: layers.mask_top, mask_bottom: false }, drill));
            out.push(mk(back, PadLayers { cu_top: false, cu_bottom: true, mask_top: false, mask_bottom: layers.mask_bottom }, None));
        }
        _ => {
            // only a back pad (no front copper) takes the back shape
            let s = if !layers.cu_top && layers.cu_bottom { back } else { front };
            out.push(mk(s, layers, drill));
        }
    }
}
fn nf_opt(x: &Sx) -> Option<f64> { if matches!(x, Sx::End) { None } else { Some(nf(x, 0)) } }

// ---- models

fn model(x: &Sx) -> ModelRef {
    let off = if !matches!(x.get("offset"), Sx::End) { xyz(x.get("offset"), 0.0) }
        else if !matches!(x.get("at"), Sx::End) { let v = xyz(x.get("at"), 0.0); [v[0] * 25.4, v[1] * 25.4, v[2] * 25.4] } else { [0.0; 3] };
    let op = x.get("opacity");
    ModelRef { path: x.arg(0).to_string(), offset: off, scale: xyz(x.get("scale"), 1.0), rotate: xyz(x.get("rotate"), 0.0), hide: flag(x, "hide"), opacity: if matches!(op, Sx::End) { 1.0 } else { nf(op, 0) } }
}

// ---- one form

/// Via tenting as the file says it: `(tenting front back)` (legacy; absent sides unset) or `(tenting (front yes) (back no))`.
fn tenting(x: &Sx) -> (Option<bool>, Option<bool>) {
    let t = x.get("tenting");
    if matches!(t, Sx::End) { return (None, None) }
    let (mut f, mut b) = (None, None);
    for c in t.args() {
        match c {
            Sx::List(_) => { let v = c.arg(0) != "no"; match c.name() { "front" => f = Some(v), "back" => b = Some(v), _ => {} } }
            _ => match c.text() { "front" => f = Some(true), "back" => b = Some(true), "none" => { f = None; b = None } _ => {} },
        }
    }
    (f, b)
}
struct RawVia { c: P2, d: f64, drill: f64, ft: Option<bool>, bt: Option<bool>, through: bool, cu_top: bool, cu_bottom: bool }
#[derive(Default)]
struct Setup { stack: Option<Stackup>, thickness_sum: f64, tent: Option<(bool, bool)> }

#[derive(Default)]
struct Part {
    acc: Acc,
    zones: Vec<(Side, Vec<P2>)>,
    pads: Vec<Pad>,
    vias: Vec<RawVia>,
    fps: Vec<Footprint>,
    general: Option<f64>,
    setup: Option<Setup>,
}

fn zone(p: &mut Part, x: &Sx, tf: &Tf) {
    if !matches!(x.get("keepout"), Sx::End) { return }
    for c in x.all("filled_polygon") {
        let side = match c.attr("layer") { "F.Cu" => Side::Top, "B.Cu" => Side::Bottom, _ => continue };
        let r = ring(c.get("pts"), tf);
        if r.len() >= 3 { p.zones.push((side, r)) }
    }
}
fn footprint(p: &mut Part, x: &Sx) {
    let (tf, at_, rot) = fp_place(x);
    let side = if x.attr("layer") == "B.Cu" { Side::Bottom } else { Side::Top };
    let fp_text = |kind: &str| x.args().iter().find(|c| c.name() == "fp_text" && c.arg(0) == kind).map(|c| c.arg(1)).unwrap_or("");
    let (mut r, mut v) = (prop(x, "Reference"), prop(x, "Value"));
    if r.is_empty() { r = fp_text("reference") }
    if v.is_empty() { v = fp_text("value") }
    let margin = nf_opt(x.get("solder_mask_margin"));
    let mut acc = Acc::default();
    let mut models = vec![];
    for c in x.args() {
        let n = c.name();
        if n == "pad" { pads(&mut p.pads, c, &tf, margin, r) }
        else if is_gfx(n) { let mut v = vec![]; gfx(c, &tf, &mut v); let l = c.attr("layer"); for s in v { acc.put(l, s) } }
        else {
            match n {
                "model" => models.push(model(c)),
                "zone" => zone(p, c, &tf),
                "property" | "fp_text" => {
                    let a = c.get("at");
                    let s = if n == "property" { c.arg(1) } else { c.arg(2) };
                    let s = if n == "fp_text" && matches!(c.arg(0), "reference" | "value") && false { s } else { s };
                    draw(&mut acc, &expand(s, r, v), tf.at(pt(a)), nf(a, 2), c, true);
                }
                "fp_text_box" => text_box(&mut acc, c, &tf, true),
                _ => {}
            }
        }
    }
    let dnp = flag(x, "dnp") || x.get("attr").args().iter().any(|a| a.text() == "dnp");
    p.fps.push(Footprint { reference: r.to_string(), value: v.to_string(), name: x.arg(0).to_string(), at: at_, rot, side, models, courtyard: std::mem::take(&mut acc.crtyd), fab: std::mem::take(&mut acc.fab), dnp });
    p.acc.append(acc);
}
fn via(p: &mut Part, x: &Sx) {
    let ps = x.get("padstack");
    let first_layer = ps.args().iter().find(|c| c.name() == "layer");
    let mut d = nf(x.get("size"), 0);
    if d <= 0.0 { if let Some(l) = first_layer { d = nf(l.get("size"), 0) } }
    let ls: Vec<&str> = x.get("layers").args().iter().map(|c| c.text()).collect();
    let (cu_top, cu_bottom) = (ls.contains(&"F.Cu"), ls.contains(&"B.Cu"));
    let blind = x.args().iter().any(|c| matches!(c.text(), "blind" | "micro"));
    let (ft, bt) = tenting(x);
    p.vias.push(RawVia { c: at(x, "at"), d, drill: nf(x.get("drill"), 0), ft, bt, through: cu_top && cu_bottom && !blind, cu_top, cu_bottom });
}

// ---- stackup

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba { Rgba { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: a as f32 / 255.0 } }
/// A stackup colour: a name from KiCad's tables (stackup_predefined_prms.cpp) or `#RRGGBB[AA]`.
fn colour(s: &str) -> Option<Rgba> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix('#') {
        let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
        return match h.len() { 6 => Some(rgba(v(0)?, v(2)?, v(4)?, 255)), 8 => Some(rgba(v(0)?, v(2)?, v(4)?, v(6)?)), _ => None };
    }
    Some(match s.to_ascii_lowercase().as_str() {
        "green" => rgba(60, 150, 80, 255), "red" => rgba(128, 0, 0, 255), "blue" => rgba(0, 0, 128, 255), "purple" => rgba(80, 0, 80, 255),
        "black" => rgba(20, 20, 20, 255), "white" => rgba(200, 200, 200, 255), "yellow" => rgba(128, 128, 0, 255),
        "fr4 natural" => rgba(109, 116, 75, 212), "ptfe natural" => rgba(252, 252, 250, 230), "polyimide" => rgba(205, 130, 0, 170),
        "phenolic natural" => rgba(92, 17, 6, 230), "aluminum" => rgba(213, 213, 213, 255),
        _ => return None,
    })
}
fn finish(s: &str) -> Finish {
    match s.to_ascii_lowercase().as_str() {
        "enig" => Finish::Enig, "enepig" => Finish::Enepig, "hal snpb" => Finish::Hasl, "hal lead-free" => Finish::HaslLeadFree,
        "hard gold" => Finish::HardGold, "immersion tin" => Finish::ImmersionTin, "immersion silver" => Finish::ImmersionSilver,
        "immersion gold" => Finish::ImmersionGold, "ht_osp" | "osp" => Finish::Osp, _ => Finish::None,
    }
}
fn setup(x: &Sx) -> Setup {
    let mut s = Setup::default();
    let t = x.get("tenting");
    if !matches!(t, Sx::End) { let (f, b) = tenting(x); s.tent = Some((f.unwrap_or(false), b.unwrap_or(false))) }
    let mut st = Stackup::default();
    st.mask_margin = nf(x.get("pad_to_mask_clearance"), 0);
    let su = x.get("stackup");
    if !matches!(su, Sx::End) {
        let mut sum = 0.0;
        for l in su.all("layer") {
            sum += l.all("thickness").map(|t| nf(t, 0)).sum::<f64>();
            let c = colour(l.attr("color"));
            match l.arg(0) {
                "F.Mask" => st.mask_top = c, "B.Mask" => st.mask_bottom = c,
                "F.SilkS" => st.silk_top = c, "B.SilkS" => st.silk_bottom = c,
                "F.Cu" => { let t = nf(l.get("thickness"), 0); if t > 0.0 { st.copper = t } }
                n if n.starts_with("dielectric") => { if st.substrate.is_none() { st.substrate = c } }
                _ => {}
            }
        }
        st.finish = finish(su.attr("copper_finish"));
        s.thickness_sum = sum;
    }
    s.stack = Some(st);
    s
}

fn form(p: &mut Part, bytes: &[u8]) {
    let x = sexpr::parse(bytes);
    let n = x.name();
    match n {
        "footprint" | "module" => footprint(p, &x),
        "via" => via(p, &x),
        "zone" => zone(p, &x, &HOME),
        "general" => { let t = nf(x.get("thickness"), 0); if t > 0.0 { p.general = Some(t) } }
        "setup" => p.setup = Some(setup(&x)),
        "gr_text" => { let a = x.get("at"); draw(&mut p.acc, x.arg(0), pt(a), nf(a, 2), &x, false) }
        "gr_text_box" => text_box(&mut p.acc, &x, &HOME, false),
        _ if is_gfx(n) => { let l = x.attr("layer"); if Acc::wants(l) { let mut v = vec![]; gfx(&x, &HOME, &mut v); for s in v { p.acc.put(l, s) } } }
        _ => {}
    }
}

/// The board a file holds; Err for a file that is not a whole board (unbalanced, still being written, not a kicad_pcb).
pub fn board(bytes: &[u8]) -> Result<Board, String> {
    let head = bytes.iter().position(|c| *c > 32).map(|i| &bytes[i..]).unwrap_or(&[]);
    if !head.starts_with(b"(kicad_pcb") { return Err("not a kicad_pcb file".into()) }
    let forms = sexpr::forms(bytes).ok_or_else(|| "the board file is not finished (unbalanced)".to_string())?;
    let parts: Vec<Part> = forms.par_chunks(128).map(|ch| { let mut p = Part::default(); for f in ch { form(&mut p, f) } p }).collect();
    let mut all = Part::default();
    let mut setups = None;
    for p in parts {
        all.acc.append(p.acc); all.zones.extend(p.zones); all.pads.extend(p.pads); all.vias.extend(p.vias); all.fps.extend(p.fps);
        if p.general.is_some() { all.general = p.general }
        if p.setup.is_some() { setups = p.setup }
    }
    let su = setups.unwrap_or_default();
    let mut stack = su.stack.unwrap_or_default();
    stack.thickness = if su.thickness_sum > 0.0 { su.thickness_sum } else { all.general.unwrap_or(1.6) };
    let (tf, tb) = su.tent.unwrap_or((true, true));
    let mut pads = all.pads;
    for p in &mut pads { if p.mask_margin.is_nan() { p.mask_margin = stack.mask_margin } }
    let mut vias = vec![];
    for v in all.vias {
        let (tt, tbm) = (v.ft.unwrap_or(tf), v.bt.unwrap_or(tb));
        if v.through { vias.push(Via { c: v.c, d: v.d, drill: v.drill, tented_top: tt, tented_bottom: tbm }) }
        else if (v.cu_top || v.cu_bottom) && v.d > 0.0 {
            // a blind or micro via: copper on the outer layer it touches, no hole through the board
            let l = PadLayers { cu_top: v.cu_top, cu_bottom: v.cu_bottom, mask_top: v.cu_top && !tt, mask_bottom: v.cu_bottom && !tbm };
            pads.push(Pad { shapes: vec![Shape::Circle { c: v.c, r: v.d / 2.0, w: 0.0, fill: true }], layers: l, drill: None, mask_margin: stack.mask_margin, reference: String::new() });
        }
    }
    Ok(Board { outline: all.acc.outline, stack, copper: all.acc.copper, zones: all.zones, pads, vias, holes: vec![], silk: all.acc.silk, mask: all.acc.mask, footprints: all.fps })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pcb(body: &str) -> Board { board(format!("(kicad_pcb (version 20240108)\n{body}\n)").as_bytes()).unwrap() }
    fn near(a: P2, x: f64, y: f64) -> bool { (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9 }
    fn rect_c(s: &Shape) -> P2 { match s { Shape::Rect { c, .. } => *c, Shape::Circle { c, .. } => *c, o => panic!("{o:?}") } }

    #[test] fn unfinished_or_foreign_files_are_errors() {
        assert!(board(b"(kicad_pcb (version 1)").is_err());
        assert!(board(b"(kicad_sch (version 1))").is_err());
        assert!(board(b"").is_err());
        assert!(board(b"(kicad_pcb (version 1))").is_ok());
    }
    #[test] fn footprint_placement_front_and_back() {
        for (a, ex, ey) in [(0.0, 11.0, 20.0), (90.0, 10.0, 19.0), (180.0, 9.0, 20.0)] {
            let b = pcb(&format!("(footprint \"L:F\" (layer \"F.Cu\") (at 10 20 {a}) (property \"Reference\" \"R1\") (property \"Value\" \"1k\") (pad \"1\" smd rect (at 1 0 {a}) (size 1 1) (layers \"F.Cu\" \"F.Mask\")))"));
            assert_eq!((b.footprints[0].reference.as_str(), b.footprints[0].value.as_str(), b.footprints[0].name.as_str()), ("R1", "1k", "L:F"));
            assert_eq!(b.footprints[0].side, Side::Top); assert_eq!(b.footprints[0].rot, a);
            assert!(near(rect_c(&b.pads[0].shapes[0]), ex, ey), "{a}: {:?}", b.pads[0].shapes);
            assert_eq!(b.pads[0].reference, "R1");
        }
        let b = pcb("(footprint \"L:F\" (layer \"B.Cu\") (at 10 20 90) (fp_line (start 0 0) (end 2 0) (layer \"B.SilkS\") (width 0.1)))");
        assert_eq!(b.footprints[0].side, Side::Bottom);
        assert_eq!(b.silk.len(), 1); assert_eq!(b.silk[0].0, Side::Bottom);
        match &b.silk[0].1 { Shape::Seg { a, b, w } => { assert!(near(*a, 10.0, 20.0) && near(*b, 10.0, 18.0) && *w == 0.1) } o => panic!("{o:?}") }
    }
    #[test] fn the_1099_transform_wins() {
        let b = pcb("(footprint \"F\" (layer \"F.Cu\") (at 1 2 3) (transform (translate 5 6) (rotate 90) (scale 1 1)) (pad \"1\" smd circle (at 1 0) (size 1 1) (layers \"F.Cu\")))");
        assert_eq!(b.footprints[0].at, P2::new(5.0, 6.0)); assert_eq!(b.footprints[0].rot, 90.0);
        assert!(near(rect_c(&b.pads[0].shapes[0]), 5.0, 5.0));
    }
    #[test] fn dnp_from_attr_or_flag() {
        let b = pcb("(footprint \"A\" (at 0 0) (attr smd dnp)) (footprint \"B\" (at 0 0) (dnp yes)) (footprint \"C\" (at 0 0) (attr smd))");
        assert_eq!(b.footprints.iter().map(|f| f.dnp).collect::<Vec<_>>(), vec![true, true, false]);
    }
    fn pad(s: &str) -> Pad { pcb(&format!("(footprint \"F\" (at 0 0) {s})")).pads.remove(0) }
    #[test] fn pad_shapes_all() {
        let p = pad("(pad \"1\" smd circle (at 1 2) (size 1.5 1.5) (layers \"F.Cu\" \"F.Mask\"))");
        assert_eq!(p.shapes, vec![Shape::Circle { c: P2::new(1.0, 2.0), r: 0.75, w: 0.0, fill: true }]);
        assert!(p.layers.cu_top && !p.layers.cu_bottom && p.layers.mask_top && !p.layers.mask_bottom);
        let p = pad("(pad \"1\" smd oval (at 0 0 90) (size 1 2) (layers \"F.Cu\"))");
        assert_eq!(p.shapes, vec![Shape::Rect { c: P2::new(0.0, 0.0), w: 1.0, h: 2.0, rot: 90.0, r: 0.5, chamfer: 0.0, chamfer_corners: 0 }]);
        let p = pad("(pad \"1\" smd roundrect (at 0 0) (size 2 1) (roundrect_rratio 0.25) (layers \"F.Cu\"))");
        assert_eq!(p.shapes, vec![Shape::Rect { c: P2::new(0.0, 0.0), w: 2.0, h: 1.0, rot: 0.0, r: 0.25, chamfer: 0.0, chamfer_corners: 0 }]);
        let p = pad("(pad \"1\" smd roundrect (at 0 0) (size 2 1) (roundrect_rratio 0) (chamfer_ratio 0.2) (chamfer top_left bottom_right) (layers \"F.Cu\"))");
        assert_eq!(p.shapes, vec![Shape::Rect { c: P2::new(0.0, 0.0), w: 2.0, h: 1.0, rot: 0.0, r: 0.0, chamfer: 0.2, chamfer_corners: 9 }]);
        let p = pad("(pad \"1\" smd trapezoid (at 0 0) (size 2 2) (rect_delta 0 1) (layers \"F.Cu\"))");
        match &p.shapes[0] { Shape::Poly { pts, fill, .. } => { assert!(*fill); assert_eq!(pts.len(), 4); assert!(near(pts[0], -1.5, 1.0) && near(pts[1], 1.5, 1.0) && near(pts[2], 0.5, -1.0)) } o => panic!("{o:?}") }
    }
    #[test] fn custom_pad_is_anchor_and_primitives_turned() {
        let p = pad("(pad \"1\" smd custom (at 10 0 90) (size 1 1) (layers \"F.Cu\") (options (clearance outline) (anchor rect)) (primitives (gr_line (start 0 0) (end 2 0) (width 0.2)) (gr_poly (pts (xy 0 0) (xy 1 0) (xy 1 1)) (width 0) (fill yes))))");
        assert_eq!(p.shapes.len(), 3);
        match &p.shapes[1] { Shape::Seg { a, b, w } => { assert!(near(*a, 10.0, 0.0) && near(*b, 10.0, -2.0) && *w == 0.2) } o => panic!("{o:?}") }
        match &p.shapes[2] { Shape::Poly { fill, pts, .. } => { assert!(*fill && pts.len() == 3) } o => panic!("{o:?}") }
    }
    #[test] fn drills_and_layers() {
        let p = pad("(pad \"1\" thru_hole circle (at 1 1) (size 2 2) (drill 1) (layers \"*.Cu\" \"*.Mask\"))");
        assert_eq!(p.drill, Some(Drill { c: P2::new(1.0, 1.0), w: 1.0, h: 1.0, rot: 0.0, plated: true }));
        assert!(p.layers.cu_top && p.layers.cu_bottom && p.layers.mask_top && p.layers.mask_bottom);
        let p = pad("(pad \"1\" thru_hole oval (at 0 0 90) (size 2 3) (drill oval 1 2) (layers \"F&B.Cu\"))");
        assert_eq!(p.drill, Some(Drill { c: P2::new(0.0, 0.0), w: 1.0, h: 2.0, rot: 90.0, plated: true }));
        let p = pad("(pad \"1\" thru_hole circle (at 0 0 90) (size 2 2) (drill 1 (offset 0.5 0)) (layers \"*.Cu\"))");
        assert!(near(p.drill.unwrap().c, 0.0, -0.5));
        let p = pad("(pad \"\" np_thru_hole circle (at 0 0) (size 1 1) (drill 1) (layers))");
        assert!(p.drill.is_some() && !p.drill.unwrap().plated && !p.layers.cu_top && !p.layers.cu_bottom);
    }
    #[test] fn mask_margin_pad_then_footprint_then_board() {
        let b = pcb("(setup (pad_to_mask_clearance 0.05)) (footprint \"F\" (at 0 0) (solder_mask_margin 0.1) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (solder_mask_margin 0.2)) (pad \"2\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\"))) (footprint \"G\" (at 0 0) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\")))");
        assert_eq!(b.pads.iter().map(|p| p.mask_margin).collect::<Vec<_>>(), vec![0.2, 0.1, 0.05]);
    }
    #[test] fn padstack_front_and_back_differ() {
        let b = pcb("(footprint \"F\" (at 0 0) (pad \"1\" thru_hole circle (at 0 0) (size 1 1) (drill 0.5) (layers \"*.Cu\") (padstack (mode front_inner_back) (layer \"F.Cu\" (shape rect) (size 2 2)) (layer \"B.Cu\" (shape circle) (size 1 1)))))");
        assert_eq!(b.pads.len(), 2);
        assert!(b.pads[0].layers.cu_top && !b.pads[0].layers.cu_bottom && b.pads[0].drill.is_some());
        assert!(matches!(b.pads[0].shapes[0], Shape::Rect { .. })); assert!(matches!(b.pads[1].shapes[0], Shape::Circle { .. }));
    }
    #[test] fn via_tenting_rules() {
        let v = |setup: &str, via: &str| pcb(&format!("{setup} (via (at 1 2) (size 0.6) (drill 0.3) (layers \"F.Cu\" \"B.Cu\") {via})")).vias.remove(0);
        let a = v("", ""); assert!(a.tented_top && a.tented_bottom && a.c == P2::new(1.0, 2.0) && a.d == 0.6 && a.drill == 0.3);
        let a = v("(setup (tenting (front yes) (back no)))", ""); assert!(a.tented_top && !a.tented_bottom);
        let a = v("(setup (tenting front))", ""); assert!(a.tented_top && !a.tented_bottom);
        let a = v("(setup (tenting (front no) (back no)))", "(tenting (front yes))"); assert!(a.tented_top && !a.tented_bottom);
        let a = v("", "(tenting front)"); assert!(a.tented_top && a.tented_bottom);
        let a = v("(setup (tenting (front no) (back no)))", "(tenting (back yes))"); assert!(!a.tented_top && a.tented_bottom);
    }
    #[test] fn blind_via_is_copper_only() {
        let b = pcb("(via blind (at 1 2) (size 0.6) (drill 0.3) (layers \"F.Cu\" \"In1.Cu\")) (via micro (at 3 2) (size 0.4) (drill 0.2) (layers \"In1.Cu\" \"B.Cu\")) (via blind (at 3 2) (size 0.4) (drill 0.2) (layers \"In1.Cu\" \"In2.Cu\"))");
        assert!(b.vias.is_empty() && b.pads.len() == 2);
        assert!(b.pads[0].layers.cu_top && !b.pads[0].layers.cu_bottom && b.pads[0].drill.is_none() && !b.pads[0].layers.mask_top);
        assert!(!b.pads[1].layers.cu_top && b.pads[1].layers.cu_bottom);
    }
    #[test] fn models_with_legacy_inches() {
        let b = pcb("(footprint \"F\" (at 0 0) (model \"${KICAD10_3DMODEL_DIR}/a.step\" (offset (xyz 1 2 3)) (scale (xyz 1 1 1)) (rotate (xyz 0 0 90)) (hide yes) (opacity 0.5)) (model \"b.wrl\" (at (xyz 0.1 0 0.2)) (scale (xyz 1 1 1)) (rotate (xyz 0 0 0))))");
        let m = &b.footprints[0].models;
        assert_eq!(m.len(), 2);
        assert_eq!(m[0], ModelRef { path: "${KICAD10_3DMODEL_DIR}/a.step".into(), offset: [1.0, 2.0, 3.0], scale: [1.0; 3], rotate: [0.0, 0.0, 90.0], hide: true, opacity: 0.5 });
        assert!((m[1].offset[0] - 2.54).abs() < 1e-12 && (m[1].offset[2] - 5.08).abs() < 1e-12 && !m[1].hide && m[1].opacity == 1.0);
    }
    #[test] fn stackup_thickness_colours_finish() {
        let b = pcb("(general (thickness 1.2)) (setup (stackup (layer \"F.SilkS\" (type \"Top Silk Screen\") (color \"White\")) (layer \"F.Mask\" (type \"Top Solder Mask\") (color \"Red\") (thickness 0.01)) (layer \"F.Cu\" (type \"copper\") (thickness 0.07)) (layer \"dielectric 1\" (type \"core\") (thickness 1.0) (color \"#102030FF\")) (layer \"B.Cu\" (type \"copper\") (thickness 0.07)) (layer \"B.Mask\" (type \"Bottom Solder Mask\") (color \"#00FF0080\") (thickness 0.01)) (copper_finish \"ENIG\")) (pad_to_mask_clearance 0.07))");
        assert!((b.stack.thickness - 1.16).abs() < 1e-12); assert_eq!(b.stack.copper, 0.07);
        assert_eq!(b.stack.mask_top, Some(rgba(128, 0, 0, 255))); assert_eq!(b.stack.silk_top, Some(rgba(200, 200, 200, 255)));
        assert_eq!(b.stack.mask_bottom, Some(rgba(0, 255, 0, 128))); assert_eq!(b.stack.substrate, Some(rgba(16, 32, 48, 255)));
        assert_eq!((b.stack.finish, b.stack.mask_margin, b.stack.silk_bottom), (Finish::Enig, 0.07, None));
        let b = pcb("(general (thickness 1.2))"); assert_eq!((b.stack.thickness, b.stack.copper, b.stack.mask_top), (1.2, 0.035, None));
        assert_eq!(pcb("").stack.thickness, 1.6);
    }
    #[test] fn text_makes_strokes_and_hides() {
        let b = pcb("(gr_text \"AB\" (at 5 5) (layer \"F.SilkS\") (effects (font (size 1 1) (thickness 0.15)))) (gr_text \"C\" (at 5 5) (layer \"B.Cu\") (effects (font (size 1 1)))) (gr_text \"H\" (at 5 5) (layer \"F.SilkS\") (hide yes) (effects (font (size 1 1)))) (gr_text \"K\" (at 5 5) (layer \"F.Cu\" knockout) (effects (font (size 1 1))))");
        assert!(!b.silk.is_empty() && b.silk.iter().all(|(s, sh)| *s == Side::Top && matches!(sh, Shape::Seg { w, .. } if (*w - 0.15).abs() < 1e-6)));
        assert!(!b.copper.is_empty() && b.copper.iter().all(|(s, _)| *s == Side::Bottom));
        let n = b.silk.len();
        let b = pcb("(gr_text \"AB\" (at 5 5) (layer \"F.SilkS\") (effects (font (size 1 1) (thickness 0.15))))"); assert_eq!(b.silk.len(), n);
        let b = pcb("(footprint \"F\" (at 0 0) (property \"Reference\" \"R9\" (at 0 -1 0) (layer \"F.SilkS\") (effects (font (size 1 1)))) (property \"Value\" \"v\" (at 0 1 0) (layer \"F.Fab\") (hide yes) (effects (font (size 1 1)))) (fp_text user \"${REFERENCE}\" (at 0 3 0) (layer \"F.SilkS\") (effects (font (size 1 1)))))");
        assert!(b.silk.len() > 2);
    }
    #[test] fn graphics_by_layer() {
        let b = pcb("(gr_line (start 0 0) (end 10 0) (stroke (width 0.1) (type solid)) (layer \"Edge.Cuts\")) (gr_arc (start 10 0) (mid 11 1) (end 10 2) (stroke (width 0.1) (type solid)) (layer \"Edge.Cuts\")) (gr_circle (center 5 5) (end 6 5) (stroke (width 0.1) (type solid)) (fill none) (layer \"Edge.Cuts\")) (gr_rect (start 0 0) (end 2 3) (stroke (width 0.1) (type solid)) (fill yes) (layer \"F.Mask\")) (gr_poly (pts (xy 0 0) (xy 1 0) (arc (start 1 0) (mid 1.5 0.5) (end 1 1)) (xy 0 1)) (width 0.2) (fill yes) (layer \"F.SilkS\")) (gr_curve (pts (xy 0 0) (xy 1 2) (xy 2 2) (xy 3 0)) (stroke (width 0.1) (type solid)) (layer \"F.Cu\")) (segment (start 0 0) (end 1 1) (width 0.25) (layer \"B.Cu\") (net 1)) (arc (start 0 0) (mid 1 1) (end 2 0) (width 0.25) (layer \"F.Cu\")) (gr_line (start 0 0) (end 1 1) (stroke (width 0.1) (type solid)) (layer \"Dwgs.User\"))");
        assert_eq!(b.outline.len(), 3);
        assert_eq!(b.mask.len(), 1); assert!(matches!(&b.mask[0].1, Shape::Poly { pts, fill: true, .. } if pts.len() == 4));
        assert!(matches!(&b.silk[0].1, Shape::Poly { pts, w, fill: true } if pts.len() == 15 && *w == 0.2));
        assert_eq!(b.copper.len(), 26); // 24 curve segments, a track, an arc
        assert!(b.copper.iter().any(|(s, sh)| *s == Side::Bottom && matches!(sh, Shape::Seg { w, .. } if *w == 0.25)));
        assert!(b.copper.iter().any(|(s, sh)| *s == Side::Top && matches!(sh, Shape::Arc { .. })));
    }
    #[test] fn footprint_outline_courtyard_fab_and_zones() {
        let b = pcb("(footprint \"F\" (at 10 0 0) (fp_line (start 0 0) (end 1 0) (stroke (width 0.1) (type solid)) (layer \"Edge.Cuts\")) (fp_rect (start -1 -1) (end 1 1) (stroke (width 0.05) (type solid)) (fill none) (layer \"F.CrtYd\")) (fp_line (start 0 0) (end 1 0) (stroke (width 0.1) (type solid)) (layer \"F.Fab\"))) (zone (net 1) (filled_polygon (layer \"F.Cu\") (pts (xy 0 0) (xy 4 0) (xy 4 4))) (filled_polygon (layer \"B.Cu\") (pts (xy 0 0) (xy 4 0) (xy 4 4) (xy 0 4))) (filled_polygon (layer \"In1.Cu\") (pts (xy 0 0) (xy 4 0) (xy 4 4))))");
        assert_eq!(b.outline.len(), 1); assert_eq!(b.footprints[0].courtyard.len(), 1); assert_eq!(b.footprints[0].fab.len(), 1);
        assert_eq!(b.zones.len(), 2); assert_eq!(b.zones[0].0, Side::Top); assert_eq!(b.zones[1].1.len(), 4);
    }

    fn fixture(n: &str) -> Board { board(&std::fs::read(format!("{}/../../test/files/fixtures/plot/{n}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap() }
    fn bbox(sh: &[Shape]) -> (f64, f64, f64, f64) {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let mut add = |p: P2| { x0 = x0.min(p.x); y0 = y0.min(p.y); x1 = x1.max(p.x); y1 = y1.max(p.y) };
        for s in sh { match s { Shape::Seg { a, b, .. } => { add(*a); add(*b) } Shape::Arc { a, b, .. } => { add(*a); add(*b) } Shape::Circle { c, r, .. } => { add(P2::new(c.x - r, c.y - r)); add(P2::new(c.x + r, c.y + r)) } Shape::Poly { pts, .. } => pts.iter().for_each(|p| add(*p)), Shape::Rect { c, .. } => add(*c) } }
        (x0, y0, x1, y1)
    }
    #[test] fn fixture_ecc83() {
        let b = fixture("ecc83-pp.kicad_pcb");
        eprintln!("ecc83: fps {} pads {} vias {} zones {} outline {} bbox {:?} stack {:?}", b.footprints.len(), b.pads.len(), b.vias.len(), b.zones.len(), b.outline.len(), bbox(&b.outline), b.stack);
        assert_eq!((b.footprints.len(), b.pads.len(), b.vias.len(), b.zones.len(), b.outline.len()), (15, 33, 0, 1, 4));
        assert_eq!(bbox(&b.outline), (121.285, 90.17, 173.355, 136.525));
        assert!(b.pads.iter().all(|p| !p.mask_margin.is_nan()));
    }
    #[test] fn fixture_microwave() {
        let b = fixture("microwave.kicad_pcb");
        eprintln!("microwave: fps {} pads {} vias {} zones {} outline {} bbox {:?}", b.footprints.len(), b.pads.len(), b.vias.len(), b.zones.len(), b.outline.len(), bbox(&b.outline));
        assert_eq!((b.footprints.len(), b.pads.len(), b.vias.len(), b.zones.len(), b.outline.len()), (4, 8, 0, 0, 4));
        assert_eq!(bbox(&b.outline), (59.69, 81.28, 351.79, 201.93));
    }

    /// BACKPLANE_TEST_BOARD=a.kicad_pcb:b.kicad_pcb cargo test --release pcb3d::parse -- --ignored --nocapture
    #[test] #[ignore] fn bench() {
        for path in std::env::var("BACKPLANE_TEST_BOARD").unwrap_or_default().split(':').filter(|s| !s.is_empty()) {
            let bytes = std::fs::read(path).unwrap();
            let mut best = u128::MAX;
            let mut b = Board::default();
            for _ in 0..5 { let t = std::time::Instant::now(); b = board(&bytes).unwrap(); best = best.min(t.elapsed().as_micros()) }
            let models: usize = b.footprints.iter().map(|f| f.models.len()).sum();
            let uniq: std::collections::HashSet<&str> = b.footprints.iter().flat_map(|f| f.models.iter().map(|m| m.path.as_str())).collect();
            println!("{path}: {} KB best of 5 {:.1} ms; footprints {} pads {} vias {} zones {} outline {} copper {} silk {} mask {} models {} ({} unique); thickness {} finish {:?}",
                bytes.len() / 1024, best as f64 / 1000.0, b.footprints.len(), b.pads.len(), b.vias.len(), b.zones.len(), b.outline.len(), b.copper.len(), b.silk.len(), b.mask.len(), models, uniq.len(), b.stack.thickness, b.stack.finish);
        }
    }
}

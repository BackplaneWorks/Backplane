//! core/board.bend, core/pcb2d.bend and pcb3d.bend's text in Rust: a .kicad_pcb's top-level forms as drawable items. Each Item keeps
//! Bend's key and signature. Items are appended in the order the Bend lists hold them: a piece's place among equals in a plot depends on it.
use super::fl;
use super::sexpr::Sx;
use super::strokefont;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt { pub x: f32, pub y: f32 }
#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    Seg { a: Pt, b: Pt, w: f32 },
    Line { ps: Vec<Pt>, w: f32 },
    Circ { c: Pt, r: f32, w: f32, fill: bool },
    Poly { ps: Vec<Pt>, w: f32, fill: bool },
    Tint { ps: Vec<Pt>, color: u32, alpha: u32 },
}
pub type Info = Arc<str>;
#[derive(Clone, Debug, PartialEq)]
pub struct Item { pub key: String, pub sig: u32, pub layer: u32, pub prim: Prim, pub info: Info }

// Tints: a coloured box's colour and opacity packed in 21 bits, never 0
fn q5(c: u32) -> u32 { (((c & 255).wrapping_mul(31)).wrapping_add(127)) / 255 }
fn e5(q: u32) -> u32 { (q.wrapping_mul(255).wrapping_add(15)) / 31 }
fn q6(a: u32) -> u32 { ((a.wrapping_mul(63).wrapping_add(127)) / 255).min(63).max(1) }
fn e6(q: u32) -> u32 { (q.wrapping_mul(255).wrapping_add(31)) / 63 }
pub fn tint_code(color: u32, alpha: u32) -> u32 { (q5(color >> 16) << 16) | (q5(color >> 8) << 11) | (q5(color) << 6) | q6(alpha.min(255)) }
pub fn tint_color(k: u32) -> u32 { (e5((k >> 16) & 31) << 16) | (e5((k >> 11) & 31) << 8) | e5((k >> 6) & 31) }
pub fn tint_alpha(k: u32) -> u32 { e6(k & 63) }
/// Tint.of: a filled polygon in a colour as viewers draw it
pub fn tint_of(ps: Vec<Pt>, color: u32, alpha: u32) -> Prim { let k = tint_code(color, alpha); Prim::Tint { ps, color: tint_color(k), alpha: tint_alpha(k) } }

/// a footprint's placement: origin and rotation, its sine and cosine worked once
#[derive(Clone, Copy, Debug)]
pub struct Place { pub x: f32, pub y: f32, pub c: f32, pub s: f32 }
impl Place {
    pub fn new(x: f32, y: f32, a: f32) -> Place { Place { x, y, c: fl::cos(a), s: fl::sin(a) } }
    pub fn at(&self, p: Pt) -> Pt { Pt { x: (p.x * self.c + p.y * self.s) + self.x, y: (p.y * self.c - p.x * self.s) + self.y } }
}
pub fn rad(deg: f32) -> f32 { deg * (fl::PI / 180.0) }
/// Geo.rot
pub fn rot(p: Pt, a: f32) -> Pt { let (c, s) = (fl::cos(a), fl::sin(a)); Pt { x: p.x * c + p.y * s, y: p.y * c - p.x * s } }
pub fn pt(x: &Sx) -> Pt { Pt { x: x.f(0), y: x.f(1) } }
pub fn at(x: &Sx, name: &str) -> Pt { pt(x.get(name)) }

// Arcs
fn norm(x: f32) -> f32 { let tau = 2.0 * fl::PI; x - tau * (x / tau).floor() }
#[allow(clippy::too_many_arguments)]
fn sweep(ux: f32, uy: f32, ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32) -> Vec<Pt> {
    let a0 = fl::atan2(ay - uy, ax - ux);
    let s1 = norm(fl::atan2(cy - uy, cx - ux) - a0);
    let sm = norm(fl::atan2(by - uy, bx - ux) - a0);
    let r = fl::hypot(ax - ux, ay - uy);
    let sw = if sm < s1 { s1 } else { s1 - 2.0 * fl::PI };
    (0..=24u32).map(|i| { let a = a0 + sw * (i as f32 / 24.0); Pt { x: ux + r * fl::cos(a), y: uy + r * fl::sin(a) } }).collect()
}
/// Arc.points: points along the circle through a, b, c from a to c (b decides the way)
pub fn arc_points(a: Pt, b: Pt, c: Pt) -> Vec<Pt> {
    let d = 2.0 * ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x));
    if d.abs() < 0.000001 { return vec![a, c] }
    let (px, py, qx, qy) = (b.x - a.x, b.y - a.y, c.x - a.x, c.y - a.y);
    let (p2, q2) = (px * px + py * py, qx * qx + qy * qy);
    sweep(a.x + (qy * p2 - py * q2) / d, a.y + (px * q2 - qx * p2) / d, a.x, a.y, b.x, b.y, c.x, c.y)
}

const LAYERS: [&str; 25] = ["B.Fab", "B.CrtYd", "B.Paste", "B.SilkS", "B.Mask", "B.Cu", "In6.Cu", "In5.Cu", "In4.Cu", "In3.Cu", "In2.Cu", "In1.Cu", "F.Cu", "*.Cu", "F.Mask", "F.Paste", "F.SilkS", "F.CrtYd", "F.Fab", "Dwgs.User", "Cmts.User", "Margin", "Edge.Cuts", "via", "hole"];
/// Layer.id: a layer's paint index (unknown layers draw with Dwgs.User)
pub fn layer_id(s: &str) -> u32 { LAYERS.iter().position(|n| *n == s).map(|i| i as u32).unwrap_or(19) }
pub const EDGE: u32 = 22;

fn width(x: &Sx) -> f32 { let w = x.get("width").f(0); if w > 0.0 { w } else { x.get("stroke").get("width").f(0) } }
fn filled(x: &Sx) -> bool { let f = x.get("fill").arg(0); f == "yes" || f == "solid" }
/// Board.key: the uuid, else the tstamp, else the fallback
pub fn key(x: &Sx, fallback: impl FnOnce() -> String) -> String {
    let u = x.attr("uuid"); if !u.is_empty() { return u.to_string() }
    let t = x.attr("tstamp"); if !t.is_empty() { return t.to_string() }
    fallback()
}
fn pts(xs: &[Sx], pl: &Place) -> Vec<Pt> { let mut o = Vec::with_capacity(xs.len()); for x in xs { if x.name() == "xy" { o.push(pl.at(pt(x))) } } o }
fn dist(a: Pt, b: Pt) -> f32 { fl::hypot(b.x - a.x, b.y - a.y) }
fn rect(a: Pt, b: Pt, pl: &Place) -> Vec<Pt> { vec![pl.at(a), pl.at(Pt { x: b.x, y: a.y }), pl.at(b), pl.at(Pt { x: a.x, y: b.y })] }

/// Info.kv
pub fn kv(k: &str, v: &str) -> String { if v.is_empty() { String::new() } else { format!(";{k}={v}") } }
fn net<'s>(x: &'s Sx) -> &'s str { let n = x.get("net"); let name = n.arg(1); if name.is_empty() { n.arg(0) } else { name } }
fn info_graphic(name: &str, x: &Sx, ctx: &str) -> Info {
    let n = if name == "segment" || name == "arc" { "track" } else { name };
    Arc::from(format!("{n}{}{}{}{ctx}", kv("net", net(x)), kv("layer", x.attr("layer")), kv("width", &fl::show(width(x)))))
}
/// Info.prop: a footprint property ("Reference", "Value")
pub fn prop<'s>(x: &'s Sx, k: &str) -> &'s str { x.args().iter().find(|c| c.name() == "property" && c.arg(0) == k).map(|c| c.arg(1)).unwrap_or("") }
fn info_footprint(x: &Sx) -> String { format!("{}{}{}", kv("ref", prop(x, "Reference")), kv("value", prop(x, "Value")), kv("footprint", x.arg(0))) }

/// Board.kind: "gr_line" -> 0 line, ... "poly" 4; "segment" 0, "arc" 1; anything else 9
fn kind(name: &str) -> u8 {
    if name.starts_with("gr_") || name.starts_with("fp_") { match &name[3..] { "line" => 0, "arc" => 1, "circle" => 2, "rect" => 3, "poly" => 4, _ => 9 } }
    else if name == "segment" { 0 } else if name == "arc" { 1 } else { 9 }
}
fn graphic(out: &mut Vec<Item>, x: &Sx, key: String, sig: u32, pl: &Place, ctx: &str) {
    let name = x.name();
    let k = kind(name);
    if k == 9 { return }
    let (layer, info) = (layer_id(x.attr("layer")), info_graphic(name, x, ctx));
    let prim = match k {
        0 => Prim::Seg { a: pl.at(at(x, "start")), b: pl.at(at(x, "end")), w: width(x) },
        1 => Prim::Line { ps: arc_points(pl.at(at(x, "start")), pl.at(at(x, "mid")), pl.at(at(x, "end"))), w: width(x) },
        2 => Prim::Circ { c: pl.at(at(x, "center")), r: dist(at(x, "center"), at(x, "end")), w: width(x), fill: filled(x) },
        3 => Prim::Poly { ps: rect(at(x, "start"), at(x, "end"), pl), w: width(x), fill: filled(x) },
        _ => Prim::Poly { ps: pts(x.get("pts").args(), pl), w: width(x), fill: filled(x) },
    };
    out.push(Item { key, sig, layer, prim, info });
}
fn via(out: &mut Vec<Item>, x: &Sx, key: String, sig: u32) {
    let c = at(x, "at");
    let info: Info = Arc::from(format!("via{}{}{}", kv("net", net(x)), kv("size", x.get("size").arg(0)), kv("drill", x.get("drill").arg(0))));
    let hole = format!("{key}/h");
    out.push(Item { key, sig, layer: 23, prim: Prim::Circ { c, r: x.get("size").f(0) / 2.0, w: 0.0, fill: true }, info: info.clone() });
    out.push(Item { key: hole, sig, layer: 24, prim: Prim::Circ { c, r: x.get("drill").f(0) / 2.0, w: 0.0, fill: true }, info });
}
fn pad(out: &mut Vec<Item>, x: &Sx, key: String, sig: u32, pl: &Place, ctx: &str) {
    let info: Info = Arc::from(format!("pad {}{}{}{}{ctx}", x.arg(0), kv("net", net(x)), kv("type", x.arg(1)), kv("shape", x.arg(2))));
    let a = x.get("at");
    let c = pl.at(pt(a));
    let (size, shape, drill) = (x.get("size"), x.arg(2), x.get("drill").f(0));
    let (w, h, ang) = (size.f(0), size.f(1), rad(-a.f(2)));
    let fr = Place::new(c.x, c.y, ang);
    let prim = if shape == "circle" { Prim::Circ { c, r: w / 2.0, w: 0.0, fill: true } }
    else if shape == "oval" {
        let horiz = w >= h;
        let (big, small) = if horiz { (w, h) } else { (h, w) };
        let half = (big - small) / 2.0;
        let (p, q) = if horiz { (Pt { x: -half, y: 0.0 }, Pt { x: half, y: 0.0 }) } else { (Pt { x: 0.0, y: -half }, Pt { x: 0.0, y: half }) };
        Prim::Seg { a: fr.at(p), b: fr.at(q), w: if horiz { h } else { w } }
    } else {
        let (hw, hh) = (w / 2.0, h / 2.0);
        Prim::Poly { ps: vec![fr.at(Pt { x: -hw, y: -hh }), fr.at(Pt { x: hw, y: -hh }), fr.at(Pt { x: hw, y: hh }), fr.at(Pt { x: -hw, y: hh })], w: 0.0, fill: true }
    };
    let hole = if drill > 0.0 { Some(format!("{key}/h")) } else { None };
    out.push(Item { key, sig, layer: layer_id(x.get("layers").arg(0)), prim, info: info.clone() });
    if let Some(k) = hole { out.push(Item { key: k, sig, layer: 24, prim: Prim::Circ { c, r: drill / 2.0, w: 0.0, fill: true }, info }) }
}
/// Fp.place: 10.99 writes the footprint origin and angle as a TRS transform; its presence is authoritative.
fn fp_place(x: &Sx) -> Place {
    let tr = x.get("transform");
    if tr.name().is_empty() { let a = x.get("at"); Place::new(a.f(0), a.f(1), rad(a.f(2))) }
    else { let t = tr.get("translate"); Place::new(t.f(0), t.f(1), rad(fl::num(tr.attr("rotate")))) }
}
fn footprint(out: &mut Vec<Item>, x: &Sx, fp: &str, sig: u32) {
    let (pl, ctx) = (fp_place(x), info_footprint(x));
    for (i, c) in x.args().iter().enumerate() {
        let k = format!("{fp}/{}", key(c, || i.to_string()));
        if c.name() == "pad" { pad(out, c, k, sig, &pl, &ctx) } else { graphic(out, c, k, sig, &pl, &ctx) }
    }
}
fn zone(out: &mut Vec<Item>, x: &Sx, key: &str, sig: u32) {
    let nn = x.attr("net_name");
    let base = format!("zone{}", kv("net", if nn.is_empty() { net(x) } else { nn }));
    let home = Place::new(0.0, 0.0, 0.0);
    for (i, c) in x.args().iter().enumerate() {
        if c.name() == "filled_polygon" {
            out.push(Item { key: format!("{key}/f{i}"), sig, layer: layer_id(c.attr("layer")), prim: Prim::Poly { ps: pts(c.get("pts").args(), &home), w: 0.0, fill: true }, info: Arc::from(format!("{base}{}", kv("layer", c.attr("layer")))) });
        }
    }
}

// Text (pcb3d.bend's Text.draw, pcb2d.bend's visibility)
/// Sx.flag: a bare word among the arguments (hide, dnp, ...), or (name yes)
fn flag(x: &Sx, name: &str) -> bool { x.args().iter().any(|c| c.text() == name || (c.name() == name && c.arg(0) != "no")) }
fn norm_deg(a: f32) -> f32 { a - 360.0 * ((a + 179.999) / 360.0).floor() }
#[allow(clippy::too_many_arguments)]
fn draw(out: &mut Vec<Item>, s: &str, p: Pt, a: f32, x: &Sx, up: bool, key: &str, sig: u32, ctx: &str) {
    if s.is_empty() || flag(x, "hide") || flag(x.get("effects"), "hide") { return }
    let (fx, layer) = (x.get("effects"), layer_id(x.attr("layer")));
    let (font, j) = (fx.get("font"), fx.get("justify"));
    let (h, w, th) = (font.get("size").f(0), font.get("size").f(1), font.get("thickness").f(0));
    let pen = if th > 0.0 { th } else { h / 8.0 };
    let na = norm_deg(a);
    let unlock = flag(x, "unlocked") || flag(x.get("at"), "unlocked");
    let turn = up && !unlock && (na > 90.001 || na <= -89.999);
    let hj = if flag(j, "left") { 0 } else if flag(j, "right") { 2 } else { 1 };
    let vj = if flag(j, "top") { 0 } else if flag(j, "bottom") { 2 } else { 1 };
    let ang = rad(if turn { na + 180.0 } else { na });
    let info: Info = Arc::from(format!("text{}{ctx}", kv("text", s)));
    let ps = strokefont::text_full(s, p.x, p.y, h, w, pen, ang, if turn { 2u32.wrapping_sub(hj) } else { hj }, if turn { 2u32.wrapping_sub(vj) } else { vj }, flag(j, "mirror"), flag(font, "italic"));
    for (i, prim) in ps.into_iter().enumerate() { out.push(Item { key: format!("{key}/{i}"), sig, layer, prim, info: info.clone() }) }
}
fn text_form(out: &mut Vec<Item>, x: &Sx, key: &str, sig: u32) {
    match x.name() {
        "gr_text" => { let a = x.get("at"); draw(out, x.arg(0), pt(a), a.f(2), x, false, &format!("{key}/text"), sig, "") }
        "footprint" => {
            let (pl, r, v, ctx) = (fp_place(x), prop(x, "Reference"), prop(x, "Value"), info_footprint(x));
            for (i, c) in x.args().iter().enumerate() {
                if c.name() == "fp_text" || c.name() == "property" {
                    let k = format!("{key}/text/{}", self::key(c, || i.to_string()));
                    let a = c.get("at");
                    let s = match c.arg(1) { "${REFERENCE}" => r, "${VALUE}" => v, s => s };
                    draw(out, s, pl.at(pt(a)), a.f(2), c, true, &k, sig, &ctx);
                }
            }
        }
        _ => {}
    }
}
/// Board.hash: FNV-1a over a form's bytes, its signature
pub fn hash(bs: &[u8], mut h: u32) -> u32 { for b in bs { h = (h ^ *b as u32).wrapping_mul(16777619) } h }
pub const FNV: u32 = 2166136261;

/// Pcb2.form: the items one top-level form draws. Board.index answers 9 for gr_poly and for what it does not know alike, and form 9 draws
/// nothing, so a top-level gr_poly draws nothing here either, as in Bend (kept for parity; reported).
pub fn form(out: &mut Vec<Item>, x: &Sx, i: usize, sig: u32) {
    let k = key(x, || format!("n{i}"));
    let name = x.name();
    match name {
        "via" => via(out, x, k.clone(), sig),
        "footprint" => footprint(out, x, &k, sig),
        "zone" => zone(out, x, &k, sig),
        "segment" | "arc" | "gr_line" | "gr_arc" | "gr_circle" | "gr_rect" => graphic(out, x, k.clone(), sig, &Place::new(0.0, 0.0, 0.0), ""),
        _ => {}
    }
    text_form(out, x, &k, sig);
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::sexpr::parse;
    // law pcb2_hidden_text_draws_nothing
    #[test] fn pcb2_hidden_text_draws_nothing() {
        for t in [&b"(gr_text \"A\" (at 1 2) (layer \"F.SilkS\") (hide yes) (effects (font (size 1 1))))"[..], b"(gr_text \"A\" (at 1 2) (layer \"F.SilkS\") (effects (font (size 1 1)) hide))", b"(gr_text \"\" (at 1 2) (layer \"F.SilkS\") (effects (font (size 1 1))))"] {
            let mut o = vec![]; form(&mut o, &parse(t), 0, 0); assert!(o.is_empty());
        }
        let mut o = vec![]; form(&mut o, &parse(b"(gr_text \"A\" (at 1 2) (layer \"F.SilkS\") (effects (font (size 1 1))))"), 0, 0); assert!(!o.is_empty());
    }
    // law fork_footprint_legacy_at: a stock file's footprint stands where its (at) says; 10.99's transform wins when present
    #[test] fn fork_footprint_legacy_at() {
        let x = parse(b"(footprint \"F\" (at 10 20 90))"); let (p, q) = (fp_place(&x), Place::new(10.0, 20.0, rad(90.0)));
        assert_eq!((p.x, p.y, p.c, p.s), (q.x, q.y, q.c, q.s));
        let x = parse(b"(footprint \"F\" (at 1 2 3) (transform (translate 0 0) (rotate 0)))"); let p = fp_place(&x); assert_eq!((p.x, p.y, p.c, p.s), (0.0, 0.0, 1.0, 0.0));
    }
    #[test] fn a_top_level_gr_poly_draws_nothing_as_in_bend() {
        let mut o = vec![]; form(&mut o, &parse(b"(gr_poly (pts (xy 0 0) (xy 1 1) (xy 2 0)) (layer \"F.Cu\") (uuid \"p\"))"), 0, 0); assert!(o.is_empty());
    }
    #[test] fn arcs_run_from_a_to_c() {
        let ps = arc_points(Pt { x: 1.0, y: 0.0 }, Pt { x: 0.0, y: 1.0 }, Pt { x: -1.0, y: 0.0 }); assert_eq!(ps.len(), 25);
        assert!((ps[12].y - 1.0).abs() < 1e-5 && ps[12].x.abs() < 1e-5);
        assert_eq!(arc_points(Pt { x: 0.0, y: 0.0 }, Pt { x: 1.0, y: 0.0 }, Pt { x: 2.0, y: 0.0 }).len(), 2);
    }
}

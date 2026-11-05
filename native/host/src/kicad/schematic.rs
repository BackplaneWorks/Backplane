//! core/schematic.bend in Rust: a .kicad_sch's forms as the same items a board draws; keys are KiCad uuids (a symbol's children add
//! "/pin/<n>", "/f/<field>", "/g<i>"), layers index Sch.layer_names. Library symbols come from the file's own lib_symbols; Conn tags
//! each connected group on the sheet. Sheet coordinates are mm with Y down; library coordinates have Y up.
use super::board::{self, arc_points, kv, prop, rad, rot, tint_of, Info, Item, Prim, Pt};
use super::fl;
use super::sexpr::{parse, Sx};
use super::strokefont;
use std::collections::HashMap;
use std::sync::Arc;

const PAGE: u32 = 0; const SHEET_FILL: u32 = 1; const BODY_FILL: u32 = 2; const SHEET: u32 = 3; const GRAPHIC: u32 = 4; const NOTE: u32 = 5; const BODY: u32 = 6;
const PIN: u32 = 7; const PIN_NAME: u32 = 8; const PIN_NUM: u32 = 9; const FIELD: u32 = 10; const FIELD_OTHER: u32 = 11; const BUS: u32 = 12; const BUS_ENTRY: u32 = 13;
const WIRE: u32 = 14; const NC: u32 = 15; const JUNCTION: u32 = 16; const LABEL: u32 = 17; const GLOBAL: u32 = 18; const HIER: u32 = 19; const SHEET_NAME: u32 = 20;
const SHEET_FILE: u32 = 21; const SHEET_PIN: u32 = 22; const DNP: u32 = 23;

/// Sch.look: colour (0xRRGGBB) and opacity of a layer
pub fn look(layer: u32) -> (u32, u32) {
    match layer {
        0 => (8018790, 200), 1 => (2758711, 90), 2 => (4209695, 110), 3 => (11757747, 255), 4 => (8360959, 230), 5 => (8360959, 255), 6 => (13788239, 255), 7 => (13788239, 255),
        8 => (4046261, 255), 9 => (13131866, 255), 10 => (4046261, 255), 11 => (11561648, 230), 12 => (5209599, 255), 13 => (5209599, 255), 14 => (4177743, 255), 15 => (7307263, 255),
        16 => (4177743, 255), 17 => (15132390, 255), 18 => (14707290, 255), 19 => (14263361, 255), 20 => (4046261, 255), 21 => (11045466, 255), 22 => (14263361, 255), 23 => (14696512, 220),
        _ => (10000536, 200),
    }
}

fn is_atom(x: &Sx, a: &str) -> bool { matches!(x, Sx::Atom(s) if s == a) }
fn atom(xs: &[Sx], a: &str) -> bool { xs.iter().any(|x| is_atom(x, a)) }
fn has(x: &Sx, n: &str) -> bool { x.get(n).name() == n }
/// Sch.flag: (name yes), (name), or a bare `name` atom (older files)
fn flag(x: &Sx, n: &str) -> bool { (has(x, n) && x.attr(n) != "no") || atom(x.args(), n) }
fn or_f(v: f32, d: f32) -> f32 { if v > 0.0 { v } else { d } }
fn width(x: &Sx, d: f32) -> f32 { or_f(x.get("stroke").get("width").f(0), d) }
/// Sch.fill: 0 no fill, 1 filled with the outline colour, 2 with the body colour; a fill of its own colour that is nearly clear is left out
fn fill(x: &Sx) -> u32 {
    let (t, a) = (x.get("fill").attr("type"), x.get("fill").get("color").f(3));
    let (colored, faint) = (a > 0.0, a < 0.25);
    if t == "outline" { 1 } else if colored && faint { 0 } else if t == "background" || t == "color" || colored { 2 } else { 0 }
}
fn deg(d: f32) -> f32 { d - 360.0 * (d / 360.0).floor() }
fn vertical(d: f32) -> bool { let n = deg(d); (n - 90.0).abs() < 1.0 || (n - 270.0).abs() < 1.0 }
fn readable(v: bool) -> f32 { if v { fl::PI / 2.0 } else { 0.0 } }

#[derive(Clone, Copy)]
struct Tx { h: f32, w: f32, pen: f32, hj: u32, vj: u32, hide: bool, it: bool }
impl Tx {
    fn of(x: &Sx) -> Tx {
        let e = x.get("effects"); let f = e.get("font"); let sz = f.get("size");
        let h = or_f(sz.f(0), 1.27);
        let js = e.get("justify").args();
        let thick = f.get("thickness").f(0);
        Tx { h, w: or_f(sz.f(1), 1.27), pen: or_f(thick, h / if flag(f, "bold") { 5.0 } else { 8.0 }),
            hj: if atom(js, "left") { 0 } else if atom(js, "right") { 2 } else { 1 }, vj: if atom(js, "top") { 0 } else if atom(js, "bottom") { 2 } else { 1 },
            hide: flag(e, "hide") || flag(x, "hide"), it: flag(f, "italic") }
    }
    fn just(self, hj: u32, vj: u32) -> Tx { Tx { hj, vj, ..self } }
}
fn text(s: &str, x: f32, y: f32, a: f32, t: &Tx) -> Vec<Prim> { strokefont::text_full(s, x, y, t.h, t.w, t.pen, a, t.hj, t.vj, false, t.it) }
/// Sch.put: items for a list of prims; each key gets "/<i>"
fn put(out: &mut Vec<Item>, ps: Vec<Prim>, key: &str, sig: u32, layer: u32, info: &Info) {
    for (i, prim) in ps.into_iter().enumerate() { out.push(Item { key: format!("{key}/{i}"), sig, layer, prim, info: info.clone() }) }
}
/// Sch.text.items: a text's items unless it is hidden or empty
#[allow(clippy::too_many_arguments)]
fn text_items(out: &mut Vec<Item>, s: &str, x: f32, y: f32, a: f32, t: &Tx, key: &str, sig: u32, layer: u32, info: &Info) {
    if t.hide || s.is_empty() { return }
    put(out, text(s, x, y, a, t), key, sig, layer, info)
}

/// Xf: where a symbol puts library points: flip Y (fy -1 for library points), mirror (mx, my), rotate by a radians, move to (x, y)
#[derive(Clone, Copy)]
struct Xf { x: f32, y: f32, a: f32, mx: f32, my: f32, fy: f32, c: f32, s: f32 }
impl Xf {
    fn new(x: f32, y: f32, a: f32, mx: f32, my: f32, fy: f32) -> Xf { Xf { x, y, a, mx, my, fy, c: fl::cos(a), s: fl::sin(a) } }
    fn home() -> Xf { Xf::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0) }
    fn turn(&self, dx: f32, dy: f32) -> Pt { let (px, py) = (dx * self.mx, dy * self.my); Pt { x: px * self.c + py * self.s, y: py * self.c - px * self.s } }
    fn unturn(&self, dx: f32, dy: f32) -> Pt { let r = rot(Pt { x: dx, y: dy }, -self.a); Pt { x: r.x * self.mx, y: r.y * self.my } }
    fn at(&self) -> Pt { Pt { x: self.x, y: self.y } }
    fn vec(&self, dx: f32, dy: f32) -> Pt { self.turn(dx, dy * self.fy) }
    fn pt(&self, px: f32, py: f32) -> Pt { add(self.at(), self.vec(px, py)) }
    fn sx(&self, x: &Sx) -> Pt { self.pt(x.f(0), x.f(1)) }
    fn get(&self, x: &Sx, n: &str) -> Pt { self.sx(x.get(n)) }
    fn quarter(&self) -> bool { self.s.abs() > 0.5 }
}
fn add(a: Pt, b: Pt) -> Pt { Pt { x: a.x + b.x, y: a.y + b.y } }
fn sub(a: Pt, b: Pt) -> Pt { Pt { x: a.x - b.x, y: a.y - b.y } }
fn scale(a: Pt, k: f32) -> Pt { Pt { x: a.x * k, y: a.y * k } }
fn pts(xs: &[Sx], t: &Xf) -> Vec<Pt> { xs.iter().filter(|x| x.name() == "xy").map(|x| t.sx(x)).collect() }

fn bez_at(f: f32, a: Pt, b: Pt, c: Pt, d: Pt) -> Pt {
    let g = 1.0 - f;
    add(add(scale(a, g * (g * g)), scale(b, 3.0 * (f * (g * g)))), add(scale(c, 3.0 * (f * (f * g))), scale(d, f * (f * f))))
}
fn bez(ps: Vec<Pt>) -> Vec<Pt> {
    if ps.len() < 4 { return ps }
    (0..=16u32).map(|i| bez_at(i as f32 / 16.0, ps[0], ps[1], ps[2], ps[3])).collect()
}
fn rect(a: Pt, b: Pt) -> Vec<Pt> { vec![a, Pt { x: b.x, y: a.y }, b, Pt { x: a.x, y: b.y }] }
fn rect_lib(t: &Xf, a: Pt, b: Pt) -> Vec<Pt> { vec![t.pt(a.x, a.y), t.pt(b.x, a.y), t.pt(b.x, b.y), t.pt(a.x, b.y)] }

/// Sch.fill.put: an outline and, when filled, its fill (on `lf`, or the outline's layer)
#[allow(clippy::too_many_arguments)]
fn fill_put(out: &mut Vec<Item>, k: u32, fill: Prim, line: Prim, key: &str, sig: u32, ll: u32, lf: u32, info: &Info) {
    if k == 1 || k == 2 { out.push(Item { key: format!("{key}/f"), sig, layer: if k == 1 { ll } else { lf }, prim: fill, info: info.clone() }) }
    out.push(Item { key: key.to_string(), sig, layer: ll, prim: line, info: info.clone() });
}
#[allow(clippy::too_many_arguments)]
fn poly(out: &mut Vec<Item>, ps: Vec<Pt>, closed: bool, w: f32, k: u32, key: &str, sig: u32, ll: u32, lf: u32, info: &Info) {
    let line = if closed { Prim::Poly { ps: ps.clone(), w, fill: false } } else { Prim::Line { ps: ps.clone(), w } };
    fill_put(out, k, Prim::Poly { ps, w: 0.0, fill: true }, line, key, sig, ll, lf, info)
}
/// Sch.shapes: polyline 0, rectangle 1, circle 2, arc 3, bezier 4, text 5
fn shape_index(n: &str) -> u32 { match n { "polyline" => 0, "rectangle" => 1, "circle" => 2, "arc" => 3, "bezier" => 4, "text" => 5, _ => 99 } }
#[allow(clippy::too_many_arguments)]
fn shape_k(out: &mut Vec<Item>, i: u32, x: &Sx, t: &Xf, key: &str, sig: u32, ll: u32, lf: u32, info: &Info, w: f32, fk: u32) {
    match i {
        0 => poly(out, pts(x.get("pts").args(), t), false, w, fk, key, sig, ll, lf, info),
        1 => poly(out, rect_lib(t, board::at(x, "start"), board::at(x, "end")), true, w, fk, key, sig, ll, lf, info),
        2 => { let (c, r) = (t.get(x, "center"), x.get("radius").f(0));
               fill_put(out, fk, Prim::Circ { c, r, w: 0.0, fill: true }, Prim::Circ { c, r, w, fill: false }, key, sig, ll, lf, info) }
        3 => out.push(Item { key: key.to_string(), sig, layer: ll, prim: Prim::Line { ps: arc_points(t.get(x, "start"), t.get(x, "mid"), t.get(x, "end")), w }, info: info.clone() }),
        4 => out.push(Item { key: key.to_string(), sig, layer: ll, prim: Prim::Line { ps: bez(pts(x.get("pts").args(), t)), w }, info: info.clone() }),
        5 => { let at = x.get("at"); let p = t.sx(at);
               text_items(out, x.arg(0), p.x, p.y, readable(vertical(at.f(2)) ^ t.quarter()), &Tx::of(x), key, sig, NOTE, info) }
        _ => {}
    }
}
fn shape(out: &mut Vec<Item>, x: &Sx, t: &Xf, key: &str, sig: u32, ll: u32, lf: u32, info: &Info) {
    shape_k(out, shape_index(x.name()), x, t, key, sig, ll, lf, info, width(x, 0.1524), fill(x))
}

// Coloured boxes
fn tint_rgb(c: &Sx) -> u32 { (fl::to_u32(fl::round(c.f(0))) << 16) | (fl::to_u32(fl::round(c.f(1))) << 8) | fl::to_u32(fl::round(c.f(2))) }
fn tint_a8(c: &Sx) -> u32 { fl::to_u32(fl::round(fl::min(c.f(3), 1.0) * 255.0)) }
fn tint_on(x: &Sx) -> bool { x.get("fill").attr("type") == "color" && x.get("fill").get("color").f(3) > 0.0 }
fn tint_circle(c: Pt, r: f32) -> Vec<Pt> {
    (0..32u32).map(|i| { let a = (i as f32 / 32.0) * (2.0 * fl::PI); Pt { x: c.x + r * fl::cos(a), y: c.y + r * fl::sin(a) } }).collect()
}
fn tint_pts(i: u32, x: &Sx) -> Vec<Pt> {
    match i { 0 => pts(x.get("pts").args(), &Xf::home()), 1 => rect(board::at(x, "start"), board::at(x, "end")), 2 => tint_circle(board::at(x, "center"), x.get("radius").f(0)), _ => vec![] }
}
fn tint_items(out: &mut Vec<Item>, ps: Vec<Pt>, x: &Sx, key: &str, sig: u32, info: &Info) {
    let c = x.get("fill").get("color");
    out.push(Item { key: format!("{key}/tint"), sig, layer: SHEET_FILL, prim: tint_of(ps, tint_rgb(c), tint_a8(c)), info: info.clone() })
}
/// Sch.top: a sheet's own shape: its tint, then its outline (filled only by "outline")
fn top(out: &mut Vec<Item>, x: &Sx, key: &str, sig: u32) {
    let i = shape_index(x.name());
    let info: Info = Arc::from("graphic");
    if tint_on(x) { tint_items(out, tint_pts(i, x), x, key, sig, &info) }
    shape_k(out, i, x, &Xf::home(), key, sig, GRAPHIC, GRAPHIC, &info, width(x, 0.1524), if x.get("fill").attr("type") == "outline" { 1 } else { 0 })
}

// Library symbols
struct Part<'s> { u: u32, s: u32, xs: &'s [Sx<'s>] }
pub struct Lib<'s> { units: u32, off: f32, names: bool, nums: bool, parts: Vec<Part<'s>> }
/// U32.read: digits, wrapping as Bend's does; None for what is not a number
fn u32_read(s: &str) -> Option<u32> {
    if s.is_empty() { return None }
    let mut acc = 0u32;
    for c in s.chars() { let n = acc.wrapping_mul(10).wrapping_add((c as u32).wrapping_sub(48)); if n / 10 != acc { return None } acc = n }
    Some(acc)
}
fn lib_of<'s>(x: &'s Sx<'s>) -> Lib<'s> {
    let pn = x.get("pin_names");
    let parts: Vec<Part> = x.all("symbol").map(|p| {
        let name = p.arg(0);
        let r: Vec<&str> = name.rsplit('_').take(2).collect();
        if r.len() == 2 { Part { u: u32_read(r[1]).unwrap_or(0), s: u32_read(r[0]).unwrap_or(0), xs: p.args() } } else { Part { u: 0, s: 0, xs: p.args() } }
    }).collect();
    let mut units = 1u32; for p in parts.iter().rev() { units = p.u.max(units) }
    Lib { units, off: if has(pn, "offset") { pn.get("offset").f(0) } else { 0.508 }, names: !flag(pn, "hide"), nums: !flag(x.get("pin_numbers"), "hide"), parts }
}
/// Lib.table with Lib.find's answer: the first symbol of a name
fn lib_table<'s>(x: &'s Sx<'s>) -> HashMap<&'s str, Lib<'s>> {
    let mut m = HashMap::new();
    for l in x.all("symbol") { m.entry(l.arg(0)).or_insert_with(|| lib_of(l)); }
    m
}
fn tag(nets: &HashMap<String, String>, id: &str) -> String { kv("g", nets.get(id).map(|s| s.as_str()).unwrap_or("")) }

// Pins
const DECO: f32 = 0.635; const GAP: f32 = 0.254;
fn item(key: String, sig: u32, layer: u32, prim: Prim, info: &Info) -> Item { Item { key, sig, layer, prim, info: info.clone() } }
struct Ctx<'c, 's> { lib: &'c HashMap<&'s str, Lib<'s>>, nets: &'c HashMap<String, String> }
#[allow(clippy::too_many_arguments)]
fn pin(out: &mut Vec<Item>, x: &Sx, t: &Xf, lib: &Lib, key: &str, sig: u32, rf: &str, nets: &HashMap<String, String>) {
    if flag(x, "hide") { return }
    let at = x.get("at");
    let a = rad(at.f(2));
    let len = x.get("length").f(0);
    let p = t.sx(at);
    let d = t.vec(fl::cos(a), fl::sin(a));
    let e = add(p, scale(d, len));
    let (name, num) = (x.get("name").arg(0), x.get("number").arg(0));
    let pk = format!("{key}/pin/{num}");
    let info: Info = Arc::from(format!("pin {num}{}{}{}{}", kv("name", name), kv("type", x.arg(0)), kv("symbol", rf), tag(nets, &pk)));
    let w = 0.1524f32;
    let line = || item(pk.clone(), sig, PIN, Prim::Seg { a: p, b: e, w }, &info);
    let short = || item(pk.clone(), sig, PIN, Prim::Seg { a: p, b: sub(e, scale(d, 2.0 * DECO)), w }, &info);
    let bub = || item(format!("{pk}/o"), sig, PIN, Prim::Circ { c: sub(e, scale(d, DECO)), r: DECO, w, fill: false }, &info);
    let clk = || item(format!("{pk}/c"), sig, PIN, Prim::Line { ps: vec![add(e, Pt { x: (-d.y) * DECO, y: d.x * DECO }), add(e, Pt { x: d.x * DECO, y: d.y * DECO }), add(e, Pt { x: d.y * DECO, y: (-d.x) * DECO })], w }, &info);
    match x.arg(1) {
        "inverted" => { out.push(short()); out.push(bub()) }
        "clock" => { out.push(line()); out.push(clk()) }
        "inverted_clock" => { out.push(short()); out.push(bub()); out.push(clk()) }
        _ => out.push(line()),
    }
    // Pin.texts
    let (tn, tm) = (Tx::of(x.get("name")), Tx::of(x.get("number")));
    let mid = add(p, scale(d, len / 2.0));
    let h = d.x.abs() > 0.5;
    let (ang, along, up) = (readable(!h), if h { d.x } else { -d.y }, if h { Pt { x: 0.0, y: -1.0 } } else { Pt { x: -1.0, y: 0.0 } });
    let inside = lib.off > 0.0;
    let np = if inside { add(e, scale(d, lib.off)) } else { add(mid, scale(up, GAP)) };
    let mp = if inside { add(mid, scale(up, GAP)) } else { sub(mid, scale(up, GAP)) };
    let shown = if !lib.names || name == "~" { "" } else { name };
    text_items(out, shown, np.x, np.y, ang, &tn.just(if inside { if along > 0.0 { 0 } else { 2 } } else { 1 }, if inside { 1 } else { 2 }), &format!("{pk}/n"), sig, PIN_NAME, &info);
    text_items(out, if lib.nums { num } else { "" }, mp.x, mp.y, ang, &tm.just(1, if inside { 2 } else { 0 }), &format!("{pk}/#"), sig, PIN_NUM, &info);
}

// Fields
fn fld_centre(t: &Xf, p: Pt, at: &Sx, tw: f32, tx: &Tx) -> Pt {
    let l = t.unturn(at.f(0) - p.x, at.f(1) - p.y);
    let cu = tw * ((1.0 - fl::from_u(tx.hj)) * 0.5);
    let cv = tx.h * ((1.0 - fl::from_u(tx.vj)) * 0.5);
    let c = add(l, if vertical(at.f(2)) { Pt { x: cv, y: -cu } } else { Pt { x: cu, y: cv } });
    add(p, t.turn(c.x, c.y))
}
fn fields(out: &mut Vec<Item>, xs: &[Sx], t: &Xf, suffix: &str, key: &str, sig: u32, info: &Info) {
    for x in xs {
        if x.name() != "property" { continue }
        let n = x.arg(0);
        let v = format!("{}{}", x.arg(1), if n == "Reference" { suffix } else { "" });
        let s = if flag(x, "show_name") { format!("{n}: {v}") } else { v };
        let (at, tx) = (x.get("at"), Tx::of(x));
        let c = fld_centre(t, t.at(), at, strokefont::width(&s, tx.w), &tx);
        text_items(out, &s, c.x, c.y, readable(vertical(at.f(2)) ^ t.quarter()), &tx.just(1, 1), &format!("{key}/f/{n}"), sig, if n == "Reference" || n == "Value" { FIELD } else { FIELD_OTHER }, info);
    }
}
fn sym_mirror(m: &str) -> (f32, f32) { match m { "x" => (1.0, -1.0), "y" => (-1.0, 1.0), _ => (1.0, 1.0) } }
fn sym_xf(at: &Sx, m: (f32, f32)) -> Xf { Xf::new(at.f(0), at.f(1), rad(at.f(2)), m.0, m.1, -1.0) }
fn sym_num(x: &Sx, n: &str) -> u32 { fl::to_u32(or_f(x.get(n).f(0), 1.0)) }
fn sym_style(x: &Sx) -> u32 { fl::to_u32(or_f(x.get("body_style").f(0), or_f(x.get("convert").f(0), 1.0))) }
fn wants(v: u32, want: u32) -> bool { v == 0 || v == want }
fn suffix(units: u32, unit: u32) -> String { if units > 1 { char::from_u32(64u32.wrapping_add(unit)).unwrap_or('\u{fffd}').to_string() } else { String::new() } }
/// Sch.cache.id: an explicit lib_name is authoritative, else lib_id
fn cache_id<'s>(x: &'s Sx) -> &'s str { cache_name(x.attr("lib_name"), x.attr("lib_id")) }
fn cache_name<'s>(name: &'s str, id: &'s str) -> &'s str { if name.is_empty() { id } else { name } }

// Do-not-populate marks
struct Bb { x0: f32, y0: f32, x1: f32, y1: f32 }
impl Bb {
    fn pt(&mut self, p: Pt, r: f32) { self.x0 = fl::min(self.x0, p.x - r); self.y0 = fl::min(self.y0, p.y - r); self.x1 = fl::max(self.x1, p.x + r); self.y1 = fl::max(self.y1, p.y + r) }
    fn prim(&mut self, p: &Prim) {
        match p {
            Prim::Seg { a, b, .. } => { self.pt(*a, 0.0); self.pt(*b, 0.0) }
            Prim::Line { ps, .. } | Prim::Poly { ps, .. } | Prim::Tint { ps, .. } => for q in ps { self.pt(*q, 0.0) },
            Prim::Circ { c, r, .. } => self.pt(*c, *r),
        }
    }
}
fn symbol(out: &mut Vec<Item>, x: &Sx, key: &str, sig: u32, c: &Ctx) {
    let id = x.attr("lib_id");
    let m = c.lib.get(cache_id(x));
    let t = sym_xf(x.get("at"), sym_mirror(x.attr("mirror")));
    let (unit, style) = (sym_num(x, "unit"), sym_style(x));
    let units = m.map(|l| l.units).unwrap_or(1);
    let suf = suffix(units, unit);
    let rf = format!("{}{suf}", prop(x, "Reference"));
    let dnp = flag(x, "dnp");
    let info: Info = Arc::from(format!("symbol{}{}{}{}{}{}", kv("ref", &rf), kv("value", prop(x, "Value")), kv("lib", id), kv("footprint", prop(x, "Footprint")), if dnp { ";dnp=yes" } else { "" }, tag(c.nets, key)));
    let start = out.len();
    if let Some(lib) = m {
        for (pi, p) in lib.parts.iter().enumerate() {
            if !(wants(p.u, unit) && wants(p.s, style)) { continue }
            let base = (pi + 1) * 1000;
            for (j, s) in p.xs.iter().enumerate() {
                if s.name() == "pin" { pin(out, s, &t, lib, key, sig, &rf, c.nets) }
                else { shape(out, s, &t, &format!("{key}/g{}", base + j), sig, BODY, BODY_FILL, &info) }
            }
        }
    }
    if dnp {
        let mut b = Bb { x0: 1000000.0, y0: 1000000.0, x1: -1000000.0, y1: -1000000.0 };
        for i in &out[start..] { if i.layer == BODY || i.layer == PIN { b.prim(&i.prim) } }
        if b.x1 > b.x0 {
            let w = 0.3f32;
            out.push(item(format!("{key}/dnp"), sig, DNP, Prim::Seg { a: Pt { x: b.x0, y: b.y0 }, b: Pt { x: b.x1, y: b.y1 }, w }, &info));
            out.push(item(format!("{key}/dnp2"), sig, DNP, Prim::Seg { a: Pt { x: b.x0, y: b.y1 }, b: Pt { x: b.x1, y: b.y0 }, w }, &info));
        }
    }
    fields(out, x.args(), &t, &suf, key, sig, &info);
}

// Wires, junctions, labels
fn wire(out: &mut Vec<Item>, x: &Sx, key: &str, sig: u32, layer: u32, w: f32, info: &str) {
    out.push(Item { key: key.to_string(), sig, layer, prim: Prim::Line { ps: pts(x.get("pts").args(), &Xf::home()), w: width(x, w) }, info: Arc::from(info) })
}
/// Lbl.pt: a point in a label's frame: u along the label, away from its anchor, and v across it
fn lbl_pt(x: f32, y: f32, a: f32, p: Pt) -> Pt { let (c, s) = (fl::cos(a), fl::sin(a)); Pt { x: x + (p.x * c + p.y * s), y: (y + p.y * c) - p.x * s } }
fn lbl_pts(ps: &[Pt], x: f32, y: f32, a: f32) -> Vec<Pt> { ps.iter().map(|p| lbl_pt(x, y, a, *p)).collect() }
fn lbl_shape(s: &str) -> u32 { match s { "input" => 1, "output" => 2, "bidirectional" | "tri_state" => 3, _ => 0 } }
#[allow(clippy::too_many_arguments)]
fn lbl_text(out: &mut Vec<Item>, s: &str, x: f32, y: f32, d: f32, u: f32, tx: &Tx, key: &str, sig: u32, layer: u32, info: &Info) {
    let p = lbl_pt(x, y, rad(d), Pt { x: u, y: 0.0 });
    text_items(out, s, p.x, p.y, readable(vertical(d)), &tx.just(if deg(d) < 135.0 { 0 } else { 2 }, 1), key, sig, layer, info)
}
fn p(x: f32, y: f32) -> Pt { Pt { x, y } }
fn glb_outline(k: u32, x: f32, y: f32, hs: f32) -> Vec<Pt> {
    let ny = -y;
    match k {
        1 => vec![p(0.0, 0.0), p(hs, ny), p(x + hs, ny), p(x + hs, 0.0), p(x + hs, y), p(hs, y)],
        2 => vec![p(0.0, 0.0), p(0.0, ny), p(x, ny), p(x + hs, 0.0), p(x, y), p(0.0, y)],
        3 => vec![p(0.0, 0.0), p(hs, ny), p(x + hs, ny), p(x + 2.0 * hs, 0.0), p(x + hs, y), p(hs, y)],
        _ => vec![p(0.0, 0.0), p(0.0, ny), p(x, ny), p(x, 0.0), p(x, y), p(0.0, y)],
    }
}
fn global(out: &mut Vec<Item>, x: &Sx, key: &str, sig: u32, tg: &str) {
    let at = x.get("at");
    let (ax, ay, d, tx, s) = (at.f(0), at.f(1), at.f(2), Tx::of(x), x.arg(0));
    let k = lbl_shape(x.attr("shape"));
    let h = tx.h;
    let margin = 0.375 * h;
    let hs = h / 2.0 + margin;
    let pen = 0.1524f32;
    let info: Info = Arc::from(format!("global label{}{}{tg}", kv("text", s), kv("shape", x.attr("shape"))));
    let o = glb_outline(k, strokefont::width(s, tx.w) + (2.0 * margin + pen), hs + pen, hs);
    out.push(item(key.to_string(), sig, GLOBAL, Prim::Poly { ps: lbl_pts(&o, ax, ay, rad(d)), w: pen, fill: false }, &info));
    lbl_text(out, s, ax, ay, d, (if k == 1 || k == 3 { hs } else { 0.0 }) + (margin + pen), &tx, &format!("{key}/t"), sig, GLOBAL, &info);
}
fn hier_outline(k: u32, s: f32) -> Vec<Pt> {
    let (ns, s2) = (-s, 2.0 * s);
    match k {
        1 => vec![p(0.0, 0.0), p(s, ns), p(s2, ns), p(s2, s), p(s, s)],
        2 => vec![p(0.0, ns), p(s, ns), p(s2, 0.0), p(s, s), p(0.0, s)],
        3 => vec![p(0.0, 0.0), p(s, ns), p(s2, 0.0), p(s, s)],
        _ => vec![p(0.0, ns), p(s2, ns), p(s2, s), p(0.0, s)],
    }
}
#[allow(clippy::too_many_arguments)]
fn hier_draw(out: &mut Vec<Item>, s: &str, ax: f32, ay: f32, d: f32, k: u32, tx: &Tx, key: &str, sig: u32, layer: u32, info: &Info) {
    let h = tx.h;
    out.push(item(key.to_string(), sig, layer, Prim::Poly { ps: lbl_pts(&hier_outline(k, h / 2.0), ax, ay, rad(d)), w: 0.1524, fill: false }, info));
    lbl_text(out, s, ax, ay, d, h + 0.3 * h, tx, &format!("{key}/t"), sig, layer, info);
}
fn margin(x: &Sx, i: usize, d: f32) -> f32 { if has(x, "margins") { x.get("margins").f(i) } else { d } }
fn pick3(j: u32, lo: f32, mid: f32, hi: f32) -> f32 { if j == 0 { lo } else if j == 1 { mid } else { hi } }
fn textbox(out: &mut Vec<Item>, x: &Sx, key: &str, sig: u32) {
    let (a, sz) = (board::at(x, "at"), board::at(x, "size"));
    let b = add(a, sz);
    let tx = Tx::of(x);
    let d = 0.8 * tx.h;
    let (x0, y0, x1, y1) = (a.x + margin(x, 0, d), a.y + margin(x, 1, d), b.x - margin(x, 2, d), b.y - margin(x, 3, d));
    let w = width(x, 0.1524);
    let info: Info = Arc::from("text box");
    if tint_on(x) { tint_items(out, rect(a, b), x, key, sig, &info) }
    if !(x.get("stroke").get("width").f(0) < 0.0) { out.push(item(key.to_string(), sig, NOTE, Prim::Poly { ps: rect(a, b), w, fill: false }, &info)) }
    text_items(out, &strokefont::wrap(x.arg(0), tx.w, x1 - x0), pick3(tx.hj, x0, (x0 + x1) / 2.0, x1), pick3(tx.vj, y0, (y0 + y1) / 2.0, y1), 0.0, &tx, &format!("{key}/t"), sig, NOTE, &info);
}
pub fn sheet_prop<'s>(x: &'s Sx, a: &str, b: &str) -> &'s str { let v = prop(x, a); if v.is_empty() { prop(x, b) } else { v } }
fn sheet(out: &mut Vec<Item>, x: &Sx, key: &str, sig: u32, nets: &HashMap<String, String>) {
    let a = board::at(x, "at");
    let ps = rect(a, add(a, board::at(x, "size")));
    let sname = sheet_prop(x, "Sheetname", "Sheet name");
    let info: Info = Arc::from(format!("sheet{}{}", kv("name", sname), kv("file", sheet_prop(x, "Sheetfile", "Sheet file"))));
    out.push(item(format!("{key}/f"), sig, SHEET_FILL, Prim::Poly { ps: ps.clone(), w: 0.0, fill: true }, &info));
    out.push(item(key.to_string(), sig, SHEET, Prim::Poly { ps, w: width(x, 0.1524), fill: false }, &info));
    for c in x.args() {
        let (n, at, pn) = (c.name(), c.get("at"), c.arg(0));
        if n == "property" {
            let (k, layer, s) = if pn == "Sheetname" || pn == "Sheet name" { (format!("{key}/name"), SHEET_NAME, c.arg(1).to_string()) }
                else if pn == "Sheetfile" || pn == "Sheet file" { (format!("{key}/file"), SHEET_FILE, format!("File: {}", c.arg(1))) }
                else { (format!("{key}/f/{pn}"), FIELD_OTHER, c.arg(1).to_string()) };
            text_items(out, &s, at.f(0), at.f(1), readable(vertical(at.f(2))), &Tx::of(c), &k, sig, layer, &info);
        } else if n == "pin" {
            let shape = match lbl_shape(c.arg(1)) { 1 => 2, 2 => 1, k => k };
            let pk = format!("{key}/pin/{pn}");
            let pi: Info = Arc::from(format!("sheet pin{}{}{}{}", kv("name", pn), kv("shape", c.arg(1)), kv("sheet", sname), tag(nets, &pk)));
            hier_draw(out, pn, at.f(0), at.f(1), at.f(2) + 180.0, shape, &Tx::of(c), &pk, sig, SHEET_PIN, &pi);
        }
    }
}
/// Prop.all: a form's visible (property ..) fields, placed as written (unexpanded ${..} variables are left out)
fn props(out: &mut Vec<Item>, xs: &[Sx], key: &str, sig: u32, layer: u32, info: &Info) {
    for x in xs {
        if x.name() != "property" { continue }
        let (at, v, n) = (x.get("at"), x.arg(1), x.arg(0));
        let s = if v.starts_with("${") { String::new() } else if flag(x, "show_name") { format!("{n}: {v}") } else { v.to_string() };
        text_items(out, &s, at.f(0), at.f(1), readable(vertical(at.f(2))), &Tx::of(x), &format!("{key}/f/{n}"), sig, layer, info);
    }
}
fn netflag(out: &mut Vec<Item>, x: &Sx, key: &str, sig: u32) {
    let at = x.get("at");
    let (ax, ay) = (at.f(0), at.f(1));
    let a = rad(at.f(2) + 90.0);
    let len = or_f(x.get("length").f(0), 2.54);
    let info: Info = Arc::from(format!("netclass flag{}", kv("netclass", prop(x, "Netclass"))));
    let r = 0.4 * Tx::of(x).h;
    out.push(item(key.to_string(), sig, LABEL, Prim::Seg { a: p(ax, ay), b: lbl_pt(ax, ay, a, p(len, 0.0)), w: 0.1524 }, &info));
    let head = match x.attr("shape") {
        "dot" => Prim::Circ { c: lbl_pt(ax, ay, a, p(len + r, 0.0)), r, w: 0.0, fill: true },
        "diamond" => Prim::Poly { ps: lbl_pts(&[p(len, 0.0), p(len + r, -r), p(len + 2.0 * r, 0.0), p(len + r, r)], ax, ay, a), w: 0.1524, fill: false },
        "rectangle" => Prim::Poly { ps: lbl_pts(&[p(len, -r), p(len + 2.0 * r, -r), p(len + 2.0 * r, r), p(len, r)], ax, ay, a), w: 0.1524, fill: false },
        _ => Prim::Circ { c: lbl_pt(ax, ay, a, p(len + r, 0.0)), r, w: 0.1524, fill: false },
    };
    out.push(item(format!("{key}/h"), sig, LABEL, head, &info));
    props(out, x.args(), key, sig, FIELD_OTHER, &info);
}

// Page
fn page_named(n: &str) -> Pt {
    match n { "A5" => p(210.0, 148.0), "A3" => p(420.0, 297.0), "A2" => p(594.0, 420.0), "A1" => p(841.0, 594.0), "A0" => p(1189.0, 841.0), "A" | "USLetter" => p(279.4, 215.9),
        "B" | "USLedger" => p(431.8, 279.4), "USLegal" => p(355.6, 215.9), "C" => p(558.8, 431.8), "D" => p(863.6, 558.8), "E" => p(1117.6, 863.6), _ => p(297.0, 210.0) }
}
fn page_size(x: &Sx) -> Pt {
    if x.arg(0) == "User" { return p(x.f(1), x.f(2)) }
    let q = page_named(x.arg(0));
    if atom(x.args(), "portrait") { p(q.y, q.x) } else { q }
}
fn page_box(x0: f32, y0: f32, x1: f32, y1: f32) -> Prim { Prim::Poly { ps: vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)], w: 0.1524, fill: false } }
fn page_seg(x0: f32, y0: f32, x1: f32, y1: f32) -> Prim { Prim::Seg { a: p(x0, y0), b: p(x1, y1), w: 0.1524 } }
fn page_tx(h: f32, pen: f32, hj: u32, vj: u32) -> Tx { Tx { h, w: h, pen, hj, vj, hide: false, it: false } }
fn page_kv(k: &str, v: &str) -> String { if v.is_empty() { String::new() } else { format!("{k}{v}") } }
fn page_comment<'s>(tb: &'s Sx, n: &str) -> &'s str { tb.args().iter().find(|x| x.name() == "comment" && x.arg(0) == n).map(|x| x.arg(1)).unwrap_or("") }
fn page_prims(q: Pt, tb: &Sx, name: &str) -> Vec<Prim> {
    let (w, h) = (q.x, q.y);
    let (rx, ry) = (w - 12.0, h - 12.0);
    let bx = rx - 107.0;
    let small = page_tx(1.5, 0.15, 0, 2);
    let mut o = vec![page_box(10.0, 10.0, w - 10.0, h - 10.0), page_box(12.0, 12.0, rx, ry), page_box(rx - 108.0, ry - 32.0, rx, ry),
        page_seg(rx - 108.0, ry - 16.5, rx, ry - 16.5), page_seg(rx - 108.0, ry - 10.5, rx, ry - 10.5), page_seg(rx - 108.0, ry - 6.5, rx, ry - 6.5), page_seg(rx - 108.0, ry - 3.5, rx, ry - 3.5),
        page_seg(rx - 24.0, ry - 6.5, rx - 24.0, ry), page_seg(rx - 86.0, ry - 6.5, rx - 86.0, ry - 3.5)];
    let grid = page_tx(1.3, 0.1524, 1, 1);
    for i in 0..40u32 {
        let x = 10.0 + 50.0 * ((i + 1) as f32);
        let mx = ((x - 50.0) + fl::min(x, w - 12.0)) / 2.0;
        if x < w - 12.0 { o.push(page_seg(x, 10.0, x, 12.0)); o.push(page_seg(x, h - 12.0, x, h - 10.0)) }
        if x - 50.0 < w - 12.0 { let s = (i + 1).to_string(); o.extend(text(&s, mx, 11.0, 0.0, &grid)); o.extend(text(&s, mx, h - 11.0, 0.0, &grid)) }
    }
    for i in 0..40u32 {
        let y = 10.0 + 50.0 * ((i + 1) as f32);
        let my = ((y - 50.0) + fl::min(y, h - 12.0)) / 2.0;
        if y < h - 12.0 { o.push(page_seg(10.0, y, 12.0, y)); o.push(page_seg(w - 12.0, y, w - 10.0, y)) }
        if y - 50.0 < h - 12.0 { let s = char::from_u32(65 + i).unwrap().to_string(); o.extend(text(&s, 11.0, my, 0.0, &grid)); o.extend(text(&s, w - 11.0, my, 0.0, &grid)) }
    }
    o.extend(text(&page_kv("Title: ", tb.attr("title")), bx, ry - 7.7, 0.0, &Tx { h: 2.0, w: 2.0, pen: 0.4, hj: 0, vj: 2, hide: false, it: true }));
    o.extend(text(&format!("Size: {name}"), bx, ry - 4.15, 0.0, &small));
    o.extend(text(&page_kv("Date: ", tb.attr("date")), rx - 85.0, ry - 4.15, 0.0, &small));
    o.extend(text(&page_kv("Rev: ", tb.attr("rev")), rx - 22.0, ry - 4.15, 0.0, &small));
    o.extend(text(tb.attr("company"), bx, ry - 17.25, 0.0, &small));
    for (n, dy) in [("1", 20.25f32), ("2", 23.25), ("3", 26.25), ("4", 29.25)] { o.extend(text(page_comment(tb, n), bx, ry - dy, 0.0, &small)) }
    o
}

// Forms
/// Sch.forms' index of a form's name, 99 when it is not drawn
fn form_index(n: &[u8]) -> u32 {
    match n { b"wire" => 0, b"bus" => 1, b"bus_entry" => 2, b"junction" => 3, b"no_connect" => 4, b"label" => 5, b"global_label" => 6, b"hierarchical_label" => 7, b"text" => 8, b"text_box" => 9,
        b"polyline" => 10, b"rectangle" => 11, b"circle" => 12, b"arc" => 13, b"bezier" => 14, b"symbol" => 15, b"sheet" => 16, b"netclass_flag" => 17, _ => 99 }
}
/// Sch.head: a form's name, read from its bytes without parsing it
pub fn head(bs: &[u8]) -> &[u8] {
    if bs.is_empty() { return bs }
    let t = &bs[1..];
    let n = t.iter().position(|b| *b <= 32 || *b == b'(' || *b == b')').unwrap_or(t.len());
    &t[..n]
}
fn form(out: &mut Vec<Item>, i: u32, x: &Sx, key: &str, sig: u32, c: &Ctx) {
    match i {
        0 => wire(out, x, key, sig, WIRE, 0.1524, &format!("wire{}", tag(c.nets, key))),
        1 => wire(out, x, key, sig, BUS, 0.3048, "bus"),
        2 => { let a = board::at(x, "at"); out.push(Item { key: key.to_string(), sig, layer: BUS_ENTRY, prim: Prim::Seg { a, b: add(a, board::at(x, "size")), w: width(x, 0.1524) }, info: Arc::from("bus entry") }) }
        3 => out.push(Item { key: key.to_string(), sig, layer: JUNCTION, prim: Prim::Circ { c: board::at(x, "at"), r: or_f(x.get("diameter").f(0), 1.016) / 2.0, w: 0.0, fill: true }, info: Arc::from(format!("junction{}", tag(c.nets, key))) }),
        4 => { let q = board::at(x, "at"); let d = 0.635f32; let info: Info = Arc::from("no connect");
               out.push(item(key.to_string(), sig, NC, Prim::Seg { a: p(q.x - d, q.y - d), b: p(q.x + d, q.y + d), w: 0.1524 }, &info));
               out.push(item(format!("{key}/2"), sig, NC, Prim::Seg { a: p(q.x - d, q.y + d), b: p(q.x + d, q.y - d), w: 0.1524 }, &info)) }
        5 => { let (at, tx) = (x.get("at"), Tx::of(x)); let v = vertical(at.f(2)); let off = 0.15 * tx.h + tx.pen;
               let info: Info = Arc::from(format!("label{}{}", kv("text", x.arg(0)), tag(c.nets, key)));
               text_items(out, x.arg(0), at.f(0) - if v { off } else { 0.0 }, at.f(1) - if v { 0.0 } else { off }, readable(v), &tx, key, sig, LABEL, &info) }
        6 => global(out, x, key, sig, &tag(c.nets, key)),
        7 => { let at = x.get("at"); let info: Info = Arc::from(format!("hier label{}{}{}", kv("text", x.arg(0)), kv("shape", x.attr("shape")), tag(c.nets, key)));
               hier_draw(out, x.arg(0), at.f(0), at.f(1), at.f(2), lbl_shape(x.attr("shape")), &Tx::of(x), key, sig, HIER, &info) }
        8 => { let at = x.get("at"); text_items(out, x.arg(0), at.f(0), at.f(1), readable(vertical(at.f(2))), &Tx::of(x), key, sig, NOTE, &Arc::from("text")) }
        9 => textbox(out, x, key, sig),
        15 => symbol(out, x, key, sig, c),
        16 => sheet(out, x, key, sig, c.nets),
        17 => netflag(out, x, key, sig),
        99 => {}
        _ => top(out, x, key, sig),
    }
}

// Connections
enum Cn { W { id: String, pts: Vec<Pt> }, P { id: String, at: Pt, name: String }, K { id: String, name: String } }
impl Cn { fn id(&self) -> &str { match self { Cn::W { id, .. } | Cn::P { id, .. } | Cn::K { id, .. } => id } } }
fn um(v: f32) -> u32 { if v < 0.0 { 0u32.wrapping_sub(fl::to_u32(fl::round((-v) * 1000.0))) } else { fl::to_u32(fl::round(v * 1000.0)) } }
fn conn_key(q: Pt) -> String { format!("{},{}", um(q.x), um(q.y)) }
fn conn_sym(out: &mut Vec<Cn>, x: &Sx, key: &str, lib: &HashMap<&str, Lib>) {
    let (rf, val) = (prop(x, "Reference"), prop(x, "Value"));
    let power = rf.starts_with('#') && !rf.starts_with("#FLG");
    let name = if power { format!("g:{val}") } else { String::new() };
    if power { out.push(Cn::K { id: key.to_string(), name: name.clone() }) }
    let Some(l) = lib.get(cache_id(x)) else { return };
    let (unit, style) = (sym_num(x, "unit"), sym_style(x));
    let t = sym_xf(x.get("at"), sym_mirror(x.attr("mirror")));
    for part in &l.parts {
        if !(wants(part.u, unit) && wants(part.s, style)) { continue }
        for s in part.xs { if s.name() == "pin" { out.push(Cn::P { id: format!("{key}/pin/{}", s.get("number").arg(0)), at: t.sx(s.get("at")), name: name.clone() }) } }
    }
}
fn conn_form(out: &mut Vec<Cn>, i: u32, x: &Sx, key: String, lib: &HashMap<&str, Lib>) {
    match i {
        0 => out.push(Cn::W { pts: pts(x.get("pts").args(), &Xf::home()), id: key }),
        3 => out.push(Cn::P { at: board::at(x, "at"), id: key, name: String::new() }),
        5 | 6 | 7 => { let pre = match i { 5 => "l:", 6 => "g:", _ => "h:" }; out.push(Cn::P { at: board::at(x, "at"), name: format!("{pre}{}", x.arg(0)), id: key }) }
        15 => conn_sym(out, x, &key, lib),
        16 => for c in x.args() { if c.name() == "pin" { out.push(Cn::P { id: format!("{key}/pin/{}", c.arg(0)), at: board::at(c, "at"), name: String::new() }) } },
        _ => {}
    }
}
/// Conn.along: a point along a wire: strictly between its ends, within a micrometre of it
fn along(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> bool {
    let (dx, dy) = (bx - ax, by - ay);
    let l2 = dx * dx + dy * dy;
    let (qx, qy) = (px - ax, py - ay);
    let f = (qx * dx + qy * dy) / fl::max(l2, 0.000001);
    let cr = dx * qy - dy * qx;
    l2 > 0.000001 && f > 0.0001 && f < 0.9999 && cr * cr < 0.000001 * l2
}
fn find(u: &mut [u32], mut a: u32) -> u32 { while u[a as usize] != a { let g = u[u[a as usize] as usize]; u[a as usize] = g; a = g } a }
/// Conn.of.k: each connection thing's group tag, by its id. The group's tag is its first thing in list order, hashed; the walk over
/// neighbours in Bend marks exactly a connected component, so a union of the same edges gives the same answer.
fn conn_of(cs: &[Cn]) -> HashMap<String, String> {
    let mut ix: HashMap<&str, u32> = HashMap::new();
    let mut names: Vec<&str> = vec![];
    for c in cs { let id = c.id(); ix.entry(id).or_insert_with(|| { names.push(id); (names.len() - 1) as u32 }); }
    let mut u: Vec<u32> = (0..names.len() as u32).collect();
    fn join(u: &mut [u32], a: u32, b: u32) { let (x, y) = (find(u, a), find(u, b)); if x != y { u[x as usize] = y } }
    let mut at: HashMap<String, u32> = HashMap::new();
    let mut named: HashMap<&str, u32> = HashMap::new();
    let (mut qs, mut segs): (Vec<(u32, f32, f32)>, Vec<(u32, f32, f32, f32, f32, f32, f32, f32, f32)>) = (vec![], vec![]);
    for c in cs {
        let i = ix[c.id()];
        match c {
            Cn::W { pts, .. } => {
                for q in pts {
                    match at.entry(conn_key(*q)) { std::collections::hash_map::Entry::Occupied(e) => { let v = *e.get(); join(&mut u, i, v) } std::collections::hash_map::Entry::Vacant(e) => { e.insert(i); } }
                    qs.push((i, q.x, q.y));
                }
                for w in pts.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    segs.push((i, a.x, a.y, b.x, b.y, fl::min(a.x, b.x) - 0.001, fl::max(a.x, b.x) + 0.001, fl::min(a.y, b.y) - 0.001, fl::max(a.y, b.y) + 0.001));
                }
            }
            Cn::P { at: q, name, .. } => {
                match at.entry(conn_key(*q)) { std::collections::hash_map::Entry::Occupied(e) => { let v = *e.get(); join(&mut u, i, v) } std::collections::hash_map::Entry::Vacant(e) => { e.insert(i); } }
                qs.push((i, q.x, q.y));
                if !name.is_empty() { match named.get(name.as_str()) { Some(v) => { let v = *v; join(&mut u, i, v) } None => { named.insert(name, i); } } }
            }
            Cn::K { name, .. } => { if !name.is_empty() { match named.get(name.as_str()) { Some(v) => { let v = *v; join(&mut u, i, v) } None => { named.insert(name, i); } } } }
        }
    }
    for &(i, px, py) in &qs {
        for &(s, ax, ay, bx, by, x0, x1, y0, y1) in &segs {
            if px >= x0 && px <= x1 && py >= y0 && py <= y1 && i != s && along(px, py, ax, ay, bx, by) { join(&mut u, i, s) }
        }
    }
    let mut first: HashMap<u32, String> = HashMap::new();
    let mut out = HashMap::with_capacity(names.len());
    for (i, id) in names.iter().enumerate() { // names keep Conn.ids' order (the first of an id)
        let r = find(&mut u, i as u32);
        let t = first.entry(r).or_insert_with(|| { let mut h = board::FNV; for c in id.chars() { h = (h ^ c as u32).wrapping_mul(16777619) } h.to_string() }).clone();
        out.insert(id.to_string(), t);
    }
    out
}

/// Sch.page: the drawing sheet's items
fn page(paper: &[u8], tb: &[u8]) -> Vec<Item> {
    let (px, t) = (parse(paper), parse(tb));
    let name = if px.arg(0).is_empty() { "A4" } else { px.arg(0) };
    let mut o = vec![];
    put(&mut o, page_prims(page_size(&px), &t, name), "page", board::hash(tb, board::hash(paper, board::FNV)), PAGE, &Arc::from("page"));
    o
}
fn first<'b>(fs: &[&'b [u8]], n: &[u8]) -> &'b [u8] { fs.iter().find(|f| head(f) == n).copied().unwrap_or(&[]) }
/// Sch.items: every item of a schematic's forms. `par` runs one form to its items on any thread, in form order; `order` is the order Sch.par's hands leave the forms in.
pub fn items(fs: &[&[u8]], par: &dyn Fn(usize, &(dyn Fn(usize) -> Vec<Item> + Sync)) -> Vec<Vec<Item>>, order: &[u32]) -> Vec<Item> {
    let lb = first(fs, b"lib_symbols");
    let lx = parse(lb);
    let lib = lib_table(&lx);
    // Conn.things: each later form's things go before the earlier ones'
    let mut cs: Vec<Cn> = vec![];
    for (n, f) in fs.iter().enumerate().rev() {
        let i = form_index(head(f));
        if matches!(i, 0 | 3 | 5 | 6 | 7 | 15 | 16) { let x = parse(f); conn_form(&mut cs, i, &x, board::key(&x, || format!("n{n}")), &lib) }
    }
    let (lsig, nets) = (board::hash(lb, board::FNV), conn_of(&cs));
    drop(cs);
    let ctx = Ctx { lib: &lib, nets: &nets };
    let one = |n: usize| -> Vec<Item> {
        let f = fs[n];
        let i = form_index(head(f));
        let mut o = vec![];
        if i == 99 { return o }
        let x = parse(f);
        let h = board::hash(f, board::FNV);
        form(&mut o, i, &x, &board::key(&x, || format!("n{n}")), if i == 15 { h ^ lsig } else { h }, &ctx);
        o
    };
    let mut per = par(fs.len(), &one);
    let mut all = page(first(fs, b"paper"), first(fs, b"title_block"));
    for &i in order { all.append(&mut per[i as usize]) }
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    // laws fork_cache_legacy, fork_cache_alias: an explicit lib_name is authoritative; old files use lib_id
    #[test] fn fork_cache_legacy() { assert_eq!(cache_name("", "Device:R"), "Device:R") }
    #[test] fn fork_cache_alias() { assert_eq!(cache_name("local", "Device:R"), "local") }
    #[test] fn head_stops_at_space_and_paren() { assert_eq!(head(b"(wire (pts))"), b"wire"); assert_eq!(head(b"(sheet)"), b"sheet"); assert_eq!(head(b""), b""); assert_eq!(head(b"(\"a\" 1)"), b"\"a\"") }
    #[test] fn conn_tags_connected_things_alike() {
        let w = |id: &str, a: (f32, f32), b: (f32, f32)| Cn::W { id: id.into(), pts: vec![p(a.0, a.1), p(b.0, b.1)] };
        let cs = vec![w("w1", (0.0, 0.0), (10.0, 0.0)), w("w2", (10.0, 0.0), (10.0, 10.0)), w("w3", (5.0, 0.0), (5.0, 5.0)), w("w4", (20.0, 0.0), (30.0, 0.0)), Cn::P { id: "l1".into(), at: p(30.0, 0.0), name: "l:A".into() }, Cn::P { id: "l2".into(), at: p(0.0, 0.0), name: "l:A".into() }, w("w5", (40.0, -5.0), (40.0, 5.0)), w("w6", (35.0, 0.0), (45.0, 0.0))];
        let t = conn_of(&cs);
        for id in ["w2", "w3", "w4", "l1", "l2"] { assert_eq!(t[id], t["w1"], "{id}") } // ends meet, a T, one name
        assert_ne!(t["w5"], t["w1"]); assert_ne!(t["w5"], t["w6"]); // two wires crossing are not connected
    }
}

//! core/strokefont.bend's layout in Rust: KiCad's text markup, lines, placement and wrapping over the Hershey glyphs
//! (strokefont_data.rs, generated). Every sum runs in the order the Bend code runs it: the floats must come out the same.
use super::board::{Prim, Pt};
use super::fl;
use super::strokefont_data as data;

pub struct Glyph { pub l: u32, pub r: u32, pub strokes: &'static [&'static [(u8, u8)]] }
/// Stroke.glyph
fn glyph(cp: u32) -> &'static Glyph { if cp < 256 { data::low(cp) } else if cp == 937 || cp == 8486 { &data::OHM } else { &data::BOX } }

#[derive(Clone, Copy)]
struct Ch { cp: u32, bar: bool }
/// Mark.var: the character a {name} stands for (0: none)
fn var(n: &str) -> u32 {
    match n { "slash" => 47, "backslash" => 92, "dblquote" => 34, "quote" => 39, "lt" => 60, "gt" => 62, "colon" => 58, "comma" => 44, "space" => 32, "tab" => 32, "return" => 10, "brl" => 123, "brr" => 125, _ => 0 }
}
/// Mark.chars: ~{..} draws an overbar; _{..} and ^{..} are drawn inline; {slash}, {lt}, ... name a character; any other {..} as it is.
fn chars(s: &str) -> Vec<Ch> {
    let (mut st, mut skip, mut tin): (Vec<bool>, bool, bool) = (vec![], false, false);
    let (mut acc, mut tok): (Vec<Ch>, Vec<u32>) = (Vec::with_capacity(s.len()), vec![]);
    let cs: Vec<u32> = s.chars().map(|c| c as u32).collect();
    for (i, &c) in cs.iter().enumerate() {
        let nxt = cs.get(i + 1).copied().unwrap_or(0);
        let bars = st.iter().any(|b| *b);
        if skip { skip = false; tin = false; tok.clear() }
        else if tin && c == 125 {
            let name: String = tok.iter().filter_map(|c| char::from_u32(*c)).collect();
            let v = var(&name);
            if v != 0 { acc.push(Ch { cp: v, bar: bars }) }
            else { acc.push(Ch { cp: 123, bar: bars }); acc.extend(tok.iter().map(|c| Ch { cp: *c, bar: bars })); acc.push(Ch { cp: 125, bar: bars }) }
            tin = false; tok.clear();
        }
        else if tin { tok.push(c) }
        else if nxt == 123 && (c == 126 || c == 95 || c == 94) { st.push(c == 126); skip = true }
        else if c == 123 { tin = true; tok.clear() }
        else if c == 125 && !st.is_empty() { st.pop(); }
        else { acc.push(Ch { cp: c, bar: bars }) }
    }
    if tin { acc.push(Ch { cp: 123, bar: false }); acc.extend(tok.iter().map(|c| Ch { cp: *c, bar: false })) }
    acc
}
#[derive(Clone, Copy)]
struct Gc { g: &'static Glyph, bar: bool }
/// Lay.split over Lay.glyphs
fn lines(s: &str) -> Vec<Vec<Gc>> {
    let mut ls = vec![vec![]];
    for c in chars(s) { if c.cp == 10 { ls.push(vec![]) } else { ls.last_mut().unwrap().push(Gc { g: glyph(c.cp), bar: c.bar }) } }
    ls
}
/// Lay.units: a line's advance in glyph units, summed from its end
fn units(cs: &[Gc]) -> f32 { let mut a = 0.0f32; for c in cs.iter().rev() { a = (fl::from_u(c.g.r) - fl::from_u(c.g.l)) + a } a }
fn widest(ls: &[Vec<Gc>]) -> f32 { let mut a = 0.0f32; for l in ls.iter().rev() { a = fl::max(units(l), a) } a }

/// TF: origin, rotation (cos, sin), mirror (1 or -1), mm per glyph unit across and down, the pen width, the italic slant
struct Tf { x: f32, y: f32, c: f32, s: f32, m: f32, sx: f32, sy: f32, pen: f32, it: f32 }
impl Tf {
    fn pt(&self, u: f32, v: f32) -> Pt { let um = u * self.m; Pt { x: self.x + (um * self.c + v * self.s), y: (self.y + v * self.c) - um * self.s } }
    fn pts(&self, ps: &[(u8, u8)], u0: f32, l: f32, v0: f32) -> Vec<Pt> {
        ps.iter().map(|&(gx, gy)| { let dv = (gy as f32 - 73.0) * self.sy; self.pt((u0 + (gx as f32 - l) * self.sx) - dv * self.it, v0 + dv) }).collect()
    }
}
const PITCH: f32 = 1.62;
/// Stroke.text.full: text as strokes in millimetres; hj 0 left, 1 centre, 2 right; vj 0 top, 1 centre, 2 bottom; lines break at "\n".
#[allow(clippy::too_many_arguments)]
pub fn text_full(s: &str, x: f32, y: f32, h: f32, w: f32, pen: f32, angle: f32, hj: u32, vj: u32, mirror: bool, italic: bool) -> Vec<Prim> {
    let ls = lines(s);
    let bh = h + ((ls.len() as u32 as f32 - 1.0) * PITCH) * h;
    let tf = Tf { x, y, c: fl::cos(angle), s: fl::sin(angle), m: if mirror { -1.0 } else { 1.0 }, sx: (w * 1.0) / 21.0, sy: h / 21.0, pen, it: if italic { 0.2 } else { 0.0 } };
    let hjf = fl::from_u(hj) * 0.5;
    let mut v0 = h - bh * (fl::from_u(vj) * 0.5);
    let pitch = PITCH * h;
    let mut out = Vec::new();
    for l in &ls {
        let mut u = -((units(l) * tf.sx) * hjf);
        for c in l {
            let (lf, adv) = (fl::from_u(c.g.l), (fl::from_u(c.g.r) - fl::from_u(c.g.l)) * tf.sx);
            for st in c.g.strokes { out.push(Prim::Line { ps: tf.pts(st, u, lf, v0), w: tf.pen }) }
            if c.bar { let v = v0 - 26.0 * tf.sy; out.push(Prim::Seg { a: tf.pt(u, v), b: tf.pt(u + adv, v), w: tf.pen }) }
            u += adv;
        }
        v0 += pitch;
    }
    out
}
/// Stroke.width: the widest line's advance in mm, for character width w
pub fn width(s: &str, w: f32) -> f32 { widest(&lines(s)) * ((w * 1.0) / 21.0) }
/// Stroke.wrap: line breaks added at spaces so no line (of character width w) is wider than maxw mm, where words allow
pub fn wrap(s: &str, w: f32, maxw: f32) -> String {
    let sp = width(" ", w);
    let mut out: Vec<String> = vec![];
    for p in s.split('\n') {
        let (mut line, mut lw) = (String::new(), 0.0f32);
        for word in p.split(' ') {
            let ww = width(word, w);
            if line.is_empty() { line = word.to_string(); lw = ww }
            else if lw + (sp + ww) <= maxw { line.push(' '); line.push_str(word); lw += sp + ww }
            else { out.push(std::mem::replace(&mut line, word.to_string())); lw = ww }
        }
        out.push(line);
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(cs: Vec<Ch>) -> String { cs.iter().map(|c| char::from_u32(c.cp).unwrap()).collect() }
    #[test] fn markup() {
        assert_eq!(s(chars("A~{BC}D")), "ABCD"); let c = chars("A~{BC}D"); assert_eq!(c.iter().map(|c| c.bar).collect::<Vec<_>>(), [false, true, true, false]);
        assert_eq!(s(chars("x_{2}y^{3}")), "x2y3"); assert_eq!(s(chars("a{slash}b")), "a/b"); assert_eq!(s(chars("{other}")), "{other}"); assert_eq!(s(chars("{open")), "{open"); assert_eq!(s(chars("a}b")), "a}b");
    }
    #[test] fn wraps() { assert_eq!(wrap("aa bb", 1.27, 1000.0), "aa bb"); assert_eq!(wrap("aa bb", 1.27, 1.0), "aa\nbb"); assert_eq!(wrap("", 1.27, 1.0), ""); assert_eq!(wrap("a\nb", 1.27, 100.0), "a\nb"); }
    #[test] fn lines_break() { assert_eq!(lines("a\nb").len(), 2); assert_eq!(lines("").len(), 1); }
}

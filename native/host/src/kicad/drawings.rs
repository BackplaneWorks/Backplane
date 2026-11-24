//! A release's fabrication and drill drawings, made from the tagged board alone: its outline with overall dimensions, the stackup, the
//! fabrication notes and the drill table, as one-page PDFs whose text is KiCad's stroke font (no font file, no date: the same board, tag and
//! commit give the same bytes). Pure. A note the board does not answer (finish, mask colour, ...) is printed OPEN and listed by `open`: nothing
//! is filled with a default, since a default on a drawing reads as a decision. The assembly views are KiCad's own plots (cad.rs `asm-*`).
use super::board::{self, Place, Prim, Pt};
use super::sexpr::{self, Sx};
use super::{fl, strokefont};

#[derive(Clone, Debug, PartialEq)]
pub struct Hole { pub at: Pt, /** the drill's diameter (a slot's width) */ pub d: f32, /** a slot's length, 0 for a round hole */ pub len: f32, pub plated: bool, /** the layers a blind, buried or micro via joins ("F.Cu-In1.Cu"); "" for a hole through the board */ pub span: String }
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Ply { pub name: String, pub kind: String, pub thickness: f32, pub material: String, pub color: String }
#[derive(Clone, Debug, Default)]
pub struct Facts { /** the outline (Edge.Cuts) as polylines, in board millimetres */ pub edges: Vec<Vec<Pt>>, pub copper: usize, pub thickness: f32, pub stack: Vec<Ply>, pub finish: String, pub holes: Vec<Hole> }
#[derive(Clone, Debug, PartialEq)]
pub struct Tool { pub d: f32, pub len: f32, pub plated: bool, pub span: String, pub count: usize }
/// One fabrication note; an empty value is open: the board does not say
#[derive(Clone, Debug, PartialEq)]
pub struct Note { pub label: &'static str, pub value: String }
pub struct Meta<'a> { pub name: &'a str, pub tag: &'a str, pub commit: &'a str }

const OPEN: &str = "OPEN - not in the board";
/// how many points a circle or a curve of the outline is drawn with
const ROUND: u32 = 48;

fn xy(x: &Sx, name: &str, pl: Option<&Place>) -> Pt { let p = board::at(x, name); pl.map_or(p, |pl| pl.at(p)) }
/// One drawn shape of the outline (`gr_*`, or a footprint's `fp_*` at its place) as a polyline; none when it is on another layer
fn edge(x: &Sx, pl: Option<&Place>) -> Option<Vec<Pt>> {
    if x.attr("layer") != "Edge.Cuts" { return None }
    let at = |p: Pt| pl.map_or(p, |pl| pl.at(p));
    let ps = match x.name().split_once('_').map_or("", |(_, k)| k) {
        "line" => vec![xy(x, "start", pl), xy(x, "end", pl)],
        "rect" => { let (a, b) = (board::at(x, "start"), board::at(x, "end")); [(a.x, a.y), (b.x, a.y), (b.x, b.y), (a.x, b.y), (a.x, a.y)].iter().map(|&(x, y)| at(Pt { x, y })).collect() }
        "circle" => {
            let (c, e) = (board::at(x, "center"), board::at(x, "end")); let r = fl::hypot(e.x - c.x, e.y - c.y);
            (0..=ROUND).map(|i| { let a = 2.0 * fl::PI * (i as f32 / ROUND as f32); at(Pt { x: c.x + r * fl::cos(a), y: c.y + r * fl::sin(a) }) }).collect()
        }
        "arc" => board::arc_points(xy(x, "start", pl), xy(x, "mid", pl), xy(x, "end", pl)),
        "poly" => {
            let mut ps: Vec<Pt> = vec![];
            for c in x.get("pts").args() { match c.name() { "xy" => ps.push(at(board::pt(c))), "arc" => ps.extend(board::arc_points(xy(c, "start", pl), xy(c, "mid", pl), xy(c, "end", pl))), _ => {} } }
            if let Some(&first) = ps.first() { ps.push(first) }
            ps
        }
        "curve" => {
            let c: Vec<Pt> = x.get("pts").all("xy").map(|p| at(board::pt(p))).collect();
            if c.len() != 4 { c } else { (0..=ROUND).map(|i| { let t = i as f32 / ROUND as f32; let u = 1.0 - t; let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                Pt { x: w[0] * c[0].x + w[1] * c[1].x + w[2] * c[2].x + w[3] * c[3].x, y: w[0] * c[0].y + w[1] * c[1].y + w[2] * c[2].y + w[3] * c[3].y } }).collect() }
        }
        _ => vec![],
    };
    (ps.len() >= 2 && ps.iter().all(|p| p.x.is_finite() && p.y.is_finite())).then_some(ps)
}
/// A pad's drill: `(drill 0.8)`, `(drill oval 0.8 1.6)`; none for a pad with no hole. The hole is at the pad's place: a drill's `(offset ...)`
/// moves the copper about the hole, not the hole.
fn pad_hole(c: &Sx, pl: &Place) -> Option<Hole> {
    let plated = match c.arg(1) { "thru_hole" => true, "np_thru_hole" => false, _ => return None };
    let dr = c.get("drill");
    let (a, b) = if dr.arg(0) == "oval" { (dr.f(1), if dr.f(2) > 0.0 { dr.f(2) } else { dr.f(1) }) } else { (dr.f(0), dr.f(0)) };
    let (d, long) = (fl::min(a, b), fl::max(a, b));
    let at = pl.at(board::at(c, "at"));
    (d > 0.0 && long.is_finite() && at.x.is_finite() && at.y.is_finite()).then(|| Hole { at, d, len: if long > d { long } else { 0.0 }, plated, span: String::new() })
}
/// A via's hole. One that does not go through the board (`blind`, `buried`, `micro`, or layers other than front to back) keeps the layers
/// it joins: on the drawing it is a tool of its own, never a through hole.
fn via_hole(x: &Sx) -> Option<Hole> {
    let (d, at, ls) = (x.get("drill").f(0), board::at(x, "at"), x.get("layers"));
    let part = x.args().iter().any(|a| matches!(a, Sx::Atom(_)) && matches!(a.text(), "blind" | "buried" | "micro")) || (!ls.arg(0).is_empty() && (ls.arg(0), ls.arg(1)) != ("F.Cu", "B.Cu"));
    (d > 0.0 && d.is_finite() && at.x.is_finite() && at.y.is_finite()).then(|| Hole { at, d, len: 0.0, plated: true, span: if part { format!("{}-{}", ls.arg(0), ls.arg(1)) } else { String::new() } })
}
/// A stackup layer: a dielectric of several plies (`addsublayer`) is as thick as all of them, of every material named
fn ply(l: &Sx) -> Ply {
    let t: f32 = l.all("thickness").map(|t| t.f(0)).sum();
    Ply { name: l.arg(0).into(), kind: l.attr("type").into(), thickness: if t.is_finite() { t } else { 0.0 }, material: distinct(l.all("material").map(|m| m.arg(0).to_string())).join(" + "), color: l.attr("color").into() }
}
/// What the drawings say, read from a board file's bytes (a file still being written, or not a board, gives nothing)
pub fn facts(b: &[u8]) -> Facts {
    let mut f = Facts::default();
    let Some(forms) = sexpr::forms(b) else { return f };
    for form in forms {
        let x = sexpr::parse(form);
        match x.name() {
            "general" => f.thickness = x.get("thickness").f(0),
            "layers" => f.copper = x.args().iter().filter(|l| l.arg(0).ends_with(".Cu")).count(),
            "setup" => {
                let st = x.get("stackup");
                f.stack = st.all("layer").map(ply).collect();
                f.finish = st.attr("copper_finish").into();
            }
            "via" => f.holes.extend(via_hole(&x)),
            "footprint" => {
                let pl = board::fp_place(&x);
                for c in x.args() {
                    if c.name() == "pad" { f.holes.extend(pad_hole(c, &pl)) } else if c.name().starts_with("fp_") { f.edges.extend(edge(c, Some(&pl))) }
                }
            }
            n if n.starts_with("gr_") => f.edges.extend(edge(&x, None)),
            _ => {}
        }
    }
    f
}
/// The outline's bounds (x0, y0, x1, y1); none with no outline
pub fn bounds(f: &Facts) -> Option<(f32, f32, f32, f32)> {
    let mut it = f.edges.iter().flatten();
    let p = it.next()?;
    Some(it.fold((p.x, p.y, p.x, p.y), |b, p| (fl::min(b.0, p.x), fl::min(b.1, p.y), fl::max(b.2, p.x), fl::max(b.3, p.y))))
}
fn um(x: f32) -> i64 { (x as f64 * 1000.0).round() as i64 }
/// Millimetres as text: two decimals, a third and fourth only when they are not zero (1.60, 0.305, 0.0175: half-ounce copper is not 0.018)
pub fn mm(x: f32) -> String {
    let u = (x as f64 * 10000.0).round() as i64;
    let mut frac = format!("{:04}", u.abs() % 10000);
    while frac.len() > 2 && frac.ends_with('0') { frac.pop(); }
    format!("{}{}.{frac}", if u < 0 { "-" } else { "" }, u.abs() / 10000)
}
/// The drill table: one row per size, plating and span: plated through holes first, then the vias that stop inside the board, then the
/// non-plated; each by size. Holes of one size to the micrometre are one tool.
pub fn tools(holes: &[Hole]) -> Vec<Tool> {
    let mut m: std::collections::BTreeMap<(bool, &str, i64, i64), Tool> = Default::default();
    for h in holes { m.entry((!h.plated, h.span.as_str(), um(h.d), um(h.len))).or_insert(Tool { d: h.d, len: h.len, plated: h.plated, span: h.span.clone(), count: 0 }).count += 1 }
    m.into_values().collect()
}
fn tool_of(ts: &[Tool], h: &Hole) -> usize { ts.iter().position(|t| t.plated == h.plated && t.span == h.span && um(t.d) == um(h.d) && um(t.len) == um(h.len)).unwrap_or(0) }
/// A tool's letter in the table: A..Z, then A2..Z2, ...
pub fn tool_id(i: usize) -> String { let c = (b'A' + (i % 26) as u8) as char; if i < 26 { c.to_string() } else { format!("{c}{}", i / 26 + 1) } }
fn size_of(t: &Tool) -> String { if t.len > 0.0 { format!("{} x {} slot", mm(t.d), mm(t.len)) } else { mm(t.d) } }
fn distinct(xs: impl Iterator<Item = String>) -> Vec<String> { let mut o: Vec<String> = vec![]; for x in xs { if !x.is_empty() && !o.contains(&x) { o.push(x) } } o }
fn sides(f: &Facts, kind: &str) -> String {
    let ps: Vec<&Ply> = f.stack.iter().filter(|p| p.kind.contains(kind)).collect();
    if ps.is_empty() || ps.iter().any(|p| p.color.is_empty()) { return String::new() }
    let cs = distinct(ps.iter().map(|p| p.color.clone()));
    if cs.len() == 1 { format!("{}, {}", cs[0], if ps.len() > 1 { "both sides".to_string() } else { format!("{} only", ps[0].name) }) } else { ps.iter().map(|p| format!("{} {}", p.name, p.color)).collect::<Vec<_>>().join(", ") }
}
/// The fabrication notes, each with what the board says or nothing
pub fn notes(f: &Facts) -> Vec<Note> {
    let cu: Vec<&Ply> = f.stack.iter().filter(|p| p.kind == "copper" && p.thickness > 0.0).collect();
    let copper = match cu.len() {
        0 => String::new(),
        1 | 2 => distinct(cu.iter().map(|p| mm(p.thickness))).iter().map(|t| format!("{t} mm")).collect::<Vec<_>>().join(" / "),
        n => format!("outer {} mm, inner {} mm", distinct([cu[0], cu[n - 1]].iter().map(|p| mm(p.thickness))).join(" / "), distinct(cu[1..n - 1].iter().map(|p| mm(p.thickness))).join(" / ")),
    };
    let note = |label, value: String| Note { label, value };
    vec![
        note("Board size", bounds(f).map_or(String::new(), |b| format!("{} x {} mm", mm(b.2 - b.0), mm(b.3 - b.1)))),
        note("Copper layers", if f.copper > 0 { f.copper.to_string() } else { String::new() }),
        note("Finished thickness", if f.thickness > 0.0 { format!("{} mm", mm(f.thickness)) } else { String::new() }),
        note("Copper thickness", copper),
        note("Base material", distinct(f.stack.iter().filter(|p| matches!(p.kind.as_str(), "core" | "prepreg")).map(|p| p.material.clone())).join(", ")),
        note("Surface finish", if matches!(f.finish.as_str(), "None" | "User defined") { String::new() } else { f.finish.clone() }),
        note("Solder mask", sides(f, "Solder Mask")),
        note("Silkscreen", sides(f, "Silk Screen")),
    ]
}
/// The notes the board leaves open: the fab would have to guess them, so the person decides
pub fn open(f: &Facts) -> Vec<&'static str> { notes(f).into_iter().filter(|n| n.value.is_empty()).map(|n| n.label).collect() }

// The sheet: A4 landscape, millimetres, y down
const W: f32 = 297.0;
const H: f32 = 210.0;
const THIN: f32 = 0.13;
const MID: f32 = 0.25;
const BOLD: f32 = 0.4;
#[derive(Default)]
pub struct Sheet { pub ops: Vec<Prim> }
impl Sheet {
    fn line(&mut self, ps: &[(f32, f32)], w: f32) { self.ops.push(Prim::Line { ps: ps.iter().map(|&(x, y)| Pt { x, y }).collect(), w }) }
    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32) { self.line(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)], w) }
    fn circle(&mut self, x: f32, y: f32, r: f32, w: f32) { self.ops.push(Prim::Circ { c: Pt { x, y }, r, w, fill: false }) }
    /// text of height h with its baseline at y; hj 0 from x, 1 centred on it, 2 ending at it
    fn text(&mut self, s: &str, x: f32, y: f32, h: f32, hj: u32) { self.ops.extend(strokefont::text_full(s, x, y, h, h, h * 0.13, 0.0, hj, 2, false, false)) }
    /// text cut to fit `maxw` (its end becomes ".."), so a long name never runs over a table's line
    fn fit(&mut self, s: &str, x: f32, y: f32, h: f32, maxw: f32) {
        let mut t: String = s.chars().filter(|c| !c.is_control()).collect();
        if strokefont::width(&t, h) > maxw { while !t.is_empty() && strokefont::width(&format!("{t}.."), h) > maxw { t.pop(); } t.push_str("..") }
        self.text(&t, x, y, h, 0)
    }
    /// a table: its title, a header row and the rows, columns `cols` wide from x; rows shrink to stay above `bottom`. The y below it.
    fn table(&mut self, title: &str, x: f32, y: f32, bottom: f32, cols: &[(&str, f32)], rows: &[Vec<String>]) -> f32 {
        self.text(title, x, y, 3.0, 0);
        let top = y + 2.0;
        let rh = fl::min(5.0, (bottom - top) / (rows.len() + 1) as f32);
        let th = fl::min(2.2, rh * 0.55);
        let wide: f32 = cols.iter().map(|c| c.1).sum();
        let mut cy = top;
        for r in std::iter::once(cols.iter().map(|c| c.0.to_string()).collect::<Vec<_>>()).chain(rows.iter().cloned()) {
            let mut cx = x;
            for (cell, col) in r.iter().zip(cols) { self.fit(cell, cx + 1.0, cy + (rh + th) / 2.0, th, col.1 - 2.0); cx += col.1 }
            cy += rh;
            self.line(&[(x, cy), (x + wide, cy)], if cy - top < rh * 1.5 { MID } else { THIN });
        }
        self.rect(x, top, x + wide, cy, MID);
        let mut cx = x;
        for col in &cols[..cols.len() - 1] { cx += col.1; self.line(&[(cx, top), (cx, cy)], THIN) }
        cy
    }
    /// the border and the title block
    fn frame(&mut self, title: &str, m: &Meta, sheet: &str) {
        self.rect(10.0, 10.0, W - 10.0, H - 10.0, BOLD);
        let (x, y) = (W - 110.0, H - 40.0);
        self.rect(x, y, W - 10.0, H - 10.0, MID);
        for dy in [12.0, 21.0] { self.line(&[(x, y + dy), (W - 10.0, y + dy)], THIN) }
        self.fit(title, x + 3.0, y + 8.5, 5.0, 94.0);
        self.fit(&format!("Board: {}", m.name), x + 3.0, y + 18.0, 3.0, 94.0);
        self.fit(&format!("Release {}   commit {}", m.tag, m.commit.chars().take(10).collect::<String>()), x + 3.0, y + 26.0, 2.5, 70.0);
        self.text(sheet, W - 13.0, y + 26.0, 2.5, 2);
    }
    /// the board's outline fitted into a zone (x0, y0, x1, y1) with its overall dimensions; where a board point lands, or none with no outline
    fn board(&mut self, f: &Facts, zone: (f32, f32, f32, f32)) -> Option<View> {
        let Some(b) = bounds(f) else { self.text("No board outline (Edge.Cuts) in the board", zone.0, zone.1 + 6.0, 3.0, 0); return None };
        let (bw, bh) = (fl::max(b.2 - b.0, 0.001), fl::max(b.3 - b.1, 0.001));
        let s = fl::min((zone.2 - zone.0) / bw, (zone.3 - zone.1) / bh);
        let v = View { s, x: (zone.0 + zone.2) / 2.0 - (b.0 + b.2) / 2.0 * s, y: (zone.1 + zone.3) / 2.0 - (b.1 + b.3) / 2.0 * s };
        for e in &f.edges { let ps: Vec<(f32, f32)> = e.iter().map(|p| v.at(*p)).collect(); self.line(&ps, BOLD) }
        let ((x0, y0), (x1, y1)) = (v.at(Pt { x: b.0, y: b.1 }), v.at(Pt { x: b.2, y: b.3 }));
        // the width under the board, the height on its left: extension lines, the dimension line with a tick at each end, the value
        let (dy, dx) = (y1 + 7.0, x0 - 7.0);
        for x in [x0, x1] { self.line(&[(x, y1 + 1.5), (x, dy + 1.5)], THIN); self.line(&[(x - 1.0, dy + 1.0), (x + 1.0, dy - 1.0)], MID) }
        self.line(&[(x0, dy), (x1, dy)], THIN);
        self.text(&mm(b.2 - b.0), (x0 + x1) / 2.0, dy - 1.0, 2.5, 1);
        for y in [y0, y1] { self.line(&[(x0 - 1.5, y), (dx - 1.5, y)], THIN); self.line(&[(dx - 1.0, y + 1.0), (dx + 1.0, y - 1.0)], MID) }
        self.line(&[(dx, y0), (dx, y1)], THIN);
        self.text(&mm(b.3 - b.1), dx - 1.5, (y0 + y1) / 2.0 + 1.25, 2.5, 2);
        self.text(&format!("Scale {s:.2}:1   Dimensions in mm"), zone.0 - 20.0, zone.3 + 14.0, 2.2, 0);
        Some(v)
    }
    /// a tool's mark, r its half size: six shapes, then the same six with a dot
    fn mark(&mut self, i: usize, x: f32, y: f32, r: f32) {
        match i % 6 {
            0 => self.circle(x, y, r, THIN),
            1 => { self.line(&[(x - r, y), (x + r, y)], THIN); self.line(&[(x, y - r), (x, y + r)], THIN) }
            2 => { self.line(&[(x - r, y - r), (x + r, y + r)], THIN); self.line(&[(x - r, y + r), (x + r, y - r)], THIN) }
            3 => self.rect(x - r, y - r, x + r, y + r, THIN),
            4 => self.line(&[(x, y - r), (x + r, y), (x, y + r), (x - r, y), (x, y - r)], THIN),
            _ => self.line(&[(x, y - r), (x + r, y + r), (x - r, y + r), (x, y - r)], THIN),
        }
        if (i / 6) % 2 == 1 { self.circle(x, y, r * 0.3, THIN) }
    }
}
/// where a board point lands on the sheet
#[derive(Clone, Copy)]
struct View { s: f32, x: f32, y: f32 }
impl View { fn at(&self, p: Pt) -> (f32, f32) { (self.x + p.x * self.s, self.y + p.y * self.s) } }

/// the holes of one kind in a line: through plated (`plated`, not `part`), stopping inside the board (`part`), or non-plated
fn count(ts: &[Tool], plated: bool, part: bool) -> String {
    let ts: Vec<&Tool> = ts.iter().filter(|t| t.plated == plated && t.span.is_empty() != part).collect();
    let n: usize = ts.iter().map(|t| t.count).sum();
    if n == 0 { return "none".into() }
    let (lo, hi) = (ts.iter().map(|t| t.d).fold(f32::MAX, fl::min), ts.iter().map(|t| t.d).fold(0.0, fl::max));
    format!("{n} in {} size{}, {} to {} mm", ts.len(), if ts.len() == 1 { "" } else { "s" }, mm(lo), mm(hi))
}
/// The fabrication drawing's sheet: outline and dimensions, stackup, notes, drill summary
pub fn fabrication_sheet(f: &Facts, m: &Meta) -> Sheet {
    let mut s = Sheet::default();
    s.frame("Fabrication drawing", m, "Sheet 1 of 1");
    s.board(f, (38.0, 22.0, 150.0, 112.0));
    if f.stack.is_empty() { s.text("LAYER STACKUP", 162.0, 20.0, 3.0, 0); s.text(&format!("{OPEN}: the board has no stackup"), 162.0, 27.0, 2.2, 0) }
    else {
        let rows: Vec<Vec<String>> = f.stack.iter().map(|p| vec![p.name.clone(), p.kind.clone(), if p.material.is_empty() { p.color.clone() } else { p.material.clone() }, if p.thickness > 0.0 { mm(p.thickness) } else { String::new() }]).collect();
        s.table("LAYER STACKUP", 162.0, 20.0, 132.0, &[("Layer", 34.0), ("Type", 38.0), ("Material / colour", 32.0), ("mm", 16.0)], &rows);
    }
    s.text("FABRICATION NOTES", 15.0, 140.0, 3.0, 0);
    for (i, n) in notes(f).iter().enumerate() {
        s.fit(&format!("{}. {}: {}", i + 1, n.label, if n.value.is_empty() { OPEN } else { &n.value }), 15.0, 146.5 + i as f32 * 5.0, 2.5, 150.0);
    }
    s.fit("An OPEN note is not answered by the board: agree it with the fabricator before ordering.", 15.0, 146.5 + 8.0 * 5.0 + 2.0, 2.2, 165.0);
    let ts = tools(&f.holes);
    s.text("DRILL SUMMARY", 162.0, 142.0, 3.0, 0);
    let mut ls = vec![format!("Plated through holes: {}", count(&ts, true, false))];
    if ts.iter().any(|t| !t.span.is_empty()) { ls.push(format!("Blind, buried and micro vias: {}", count(&ts, true, true))) }
    ls.extend([format!("Non-plated holes: {}", count(&ts, false, false)), "Positions and sizes: see the drill drawing.".into()]);
    for (i, l) in ls.iter().enumerate() { s.fit(l, 162.0, 148.5 + i as f32 * 5.0, 2.5, 120.0) }
    s
}
/// The drill drawing's sheet: every hole at its place with its tool's mark, and the table of sizes, plating and counts
pub fn drill_sheet(f: &Facts, m: &Meta) -> Sheet {
    let mut s = Sheet::default();
    s.frame("Drill drawing", m, "Sheet 1 of 1");
    let ts = tools(&f.holes);
    if let Some(v) = s.board(f, (38.0, 22.0, 172.0, 172.0)) {
        for h in &f.holes { let (x, y) = v.at(h.at); s.mark(tool_of(&ts, h), x, y, fl::max(h.d * v.s / 2.0, 0.7)) }
    }
    if ts.is_empty() { s.text("DRILL TABLE", 184.0, 20.0, 3.0, 0); s.text("No holes in the board", 184.0, 27.0, 2.2, 0); return s }
    let mut rows: Vec<Vec<String>> = ts.iter().enumerate().map(|(i, t)| vec![String::new(), tool_id(i), size_of(t), if !t.plated { "NPTH".to_string() } else if t.span.is_empty() { "PTH".to_string() } else { format!("via {}", t.span) }, t.count.to_string()]).collect();
    let total = |p: bool| ts.iter().filter(|t| t.plated == p).map(|t| t.count).sum::<usize>();
    rows.push(vec![String::new(), String::new(), "Total".into(), format!("{} PTH / {} NPTH", total(true), total(false)), f.holes.len().to_string()]);
    let cols = [("", 9.0), ("ID", 10.0), ("Size mm", 30.0), ("Plating", 36.0), ("Count", 14.0)];
    let end = s.table("DRILL TABLE", 184.0, 20.0, 165.0, &cols, &rows);
    let rh = (end - 22.0) / (rows.len() + 1) as f32;
    for i in 0..ts.len() { s.mark(i, 188.5, 22.0 + rh * (i as f32 + 1.5), fl::min(1.4, rh * 0.32)) }
    s
}

// PDF: one page of stroked paths, written whole; nothing in it depends on the clock or the machine
fn esc(s: &str) -> String { s.chars().filter(|c| c.is_ascii() && !c.is_control()).flat_map(|c| if matches!(c, '(' | ')' | '\\') { vec!['\\', c] } else { vec![c] }).collect() }
/// A sheet's drawing as a PDF page's content: millimetres to points, y up
fn content(ops: &[Prim]) -> String {
    use std::fmt::Write;
    const K: f32 = 72.0 / 25.4;
    let p = |p: &Pt| format!("{:.2} {:.2}", p.x * K, (H - p.y) * K);
    let path = |o: &mut String, ps: &[Pt]| { for (i, q) in ps.iter().enumerate() { let _ = writeln!(o, "{} {}", p(q), if i == 0 { "m" } else { "l" }); } };
    let mut o = String::from("1 J 1 j 0 G 0 g\n");
    let mut pen = -1.0f32;
    let mut width = |o: &mut String, w: f32| { if w != pen { pen = w; let _ = writeln!(o, "{:.2} w", fl::max(w, 0.05) * K); } };
    for op in ops {
        match op {
            Prim::Seg { a, b, w } => { width(&mut o, *w); path(&mut o, &[*a, *b]); o.push_str("S\n") }
            Prim::Line { ps, w } => { if ps.len() < 2 { continue } width(&mut o, *w); path(&mut o, ps); o.push_str("S\n") }
            Prim::Poly { ps, w, fill } => { if ps.len() < 2 { continue } width(&mut o, *w); path(&mut o, ps); o.push_str(if *fill { "h f\n" } else { "h S\n" }) }
            Prim::Circ { c, r, w, fill } => {
                width(&mut o, *w);
                let k = 0.552_284_8 * r;
                let q = |dx: f32, dy: f32| p(&Pt { x: c.x + dx, y: c.y + dy });
                let _ = writeln!(o, "{} m", q(*r, 0.0));
                for (a, b, e) in [((*r, k), (k, *r), (0.0, *r)), ((-k, *r), (-*r, k), (-*r, 0.0)), ((-*r, -k), (-k, -*r), (0.0, -*r)), ((k, -*r), (*r, -k), (*r, 0.0))] { let _ = writeln!(o, "{} {} {} c", q(a.0, a.1), q(b.0, b.1), q(e.0, e.1)); }
                o.push_str(if *fill { "f\n" } else { "S\n" });
            }
            Prim::Tint { .. } => {}
        }
    }
    o
}
/// A sheet as a one-page PDF
pub fn pdf(title: &str, sheet: &Sheet) -> Vec<u8> {
    const K: f32 = 72.0 / 25.4;
    let body = content(&sheet.ops);
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Resources << >> /Contents 4 0 R >>", W * K, H * K),
        format!("<< /Length {} >>\nstream\n{body}\nendstream", body.len()),
        format!("<< /Title ({}) /Producer (Backplane) >>", esc(title)),
    ];
    let mut o: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut at = vec![];
    for (i, b) in objs.iter().enumerate() { at.push(o.len()); o.extend(format!("{} 0 obj\n{b}\nendobj\n", i + 1).bytes()) }
    let xref = o.len();
    o.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    for a in &at { o.extend(format!("{a:010} 00000 n \n").bytes()) }
    o.extend(format!("trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).bytes());
    o
}
pub fn fabrication(f: &Facts, m: &Meta) -> Vec<u8> { pdf(&format!("{} {} fabrication drawing", m.name, m.tag), &fabrication_sheet(f, m)) }
pub fn drill(f: &Facts, m: &Meta) -> Vec<u8> { pdf(&format!("{} {} drill drawing", m.name, m.tag), &drill_sheet(f, m)) }

#[cfg(test)]
mod tests {
    use super::*;
    const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew")
  (general (thickness 1.6))
  (layers (0 "F.Cu" signal) (1 "In1.Cu" signal) (2 "In2.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (setup (stackup
    (layer "F.SilkS" (type "Top Silk Screen") (color "White"))
    (layer "F.Mask" (type "Top Solder Mask") (color "Green") (thickness 0.01))
    (layer "F.Cu" (type "copper") (thickness 0.035))
    (layer "dielectric 1" (type "prepreg") (thickness 0.1) (material "FR4") (epsilon_r 4.5))
    (layer "In1.Cu" (type "copper") (thickness 0.0175))
    (layer "dielectric 2" (type "core") (thickness 1.24) (material "FR4"))
    (layer "In2.Cu" (type "copper") (thickness 0.0175))
    (layer "dielectric 3" (type "prepreg") (thickness 0.1) (material "FR4"))
    (layer "B.Cu" (type "copper") (thickness 0.035))
    (layer "B.Mask" (type "Bottom Solder Mask") (color "Green") (thickness 0.01))
    (layer "B.SilkS" (type "Bottom Silk Screen") (color "White"))
    (copper_finish "ENIG") (dielectric_constraints no)))
  (gr_rect (start 100 50) (end 140 80) (layer "Edge.Cuts"))
  (gr_line (start 0 0) (end 500 500) (layer "Dwgs.User"))
  (footprint "J" (layer "F.Cu") (at 110 60 90)
    (pad "1" thru_hole circle (at 0 0) (size 1.7 1.7) (drill 1))
    (pad "2" thru_hole circle (at 2.54 0) (size 1.7 1.7) (drill 1.0))
    (pad "3" thru_hole oval (at 5 0) (size 2 3) (drill oval 0.8 1.6))
    (pad "" np_thru_hole circle (at 0 5) (size 3.2 3.2) (drill 3.2))
    (pad "4" smd rect (at 9 9) (size 1 1)))
  (footprint "R" (layer "B.Cu") (at 120 70))
  (via (at 130 60) (size 0.6) (drill 0.3) (layers "F.Cu" "B.Cu"))
  (via (at 131 60) (size 0.6) (drill 0.3) (layers "F.Cu" "B.Cu"))
)"#;
    const META: Meta = Meta { name: "amp", tag: "v1", commit: "0123456789abcdef0123" };
    fn text_of(b: &[u8]) -> String { String::from_utf8_lossy(b).into_owned() }
    #[test] fn the_board_gives_its_outline_stackup_and_holes() {
        let f = facts(BOARD.as_bytes());
        assert_eq!(bounds(&f), Some((100.0, 50.0, 140.0, 80.0)), "only Edge.Cuts is the outline");
        assert_eq!((f.copper, f.thickness, f.finish.as_str(), f.stack.len()), (4, 1.6, "ENIG", 11));
        assert_eq!(f.holes.len(), 6);
        // the footprint is turned 90 degrees: its pad at (2.54, 0) lands above the origin
        assert_eq!((um(f.holes[1].at.x), um(f.holes[1].at.y)), (110_000, 57_460));
        assert_eq!(f.holes[2], Hole { at: f.holes[2].at, d: 0.8, len: 1.6, plated: true, span: String::new() });
        let ts = tools(&f.holes);
        assert_eq!(ts.iter().map(|t| (size_of(t), t.plated, t.count)).collect::<Vec<_>>(), [("0.30".to_string(), true, 2), ("0.80 x 1.60 slot".to_string(), true, 1), ("1.00".to_string(), true, 2), ("3.20".to_string(), false, 1)]);
        assert_eq!((tool_id(0), tool_id(25), tool_id(26)), ("A".to_string(), "Z".to_string(), "A2".to_string()));
    }
    #[test] fn notes_say_what_the_board_says_and_nothing_is_defaulted() {
        let f = facts(BOARD.as_bytes());
        let ns: Vec<(&str, String)> = notes(&f).into_iter().map(|n| (n.label, n.value)).collect();
        assert_eq!(ns, [("Board size", "40.00 x 30.00 mm".to_string()), ("Copper layers", "4".into()), ("Finished thickness", "1.60 mm".into()), ("Copper thickness", "outer 0.035 mm, inner 0.0175 mm".into()),
            ("Base material", "FR4".into()), ("Surface finish", "ENIG".into()), ("Solder mask", "Green, both sides".into()), ("Silkscreen", "White, both sides".into())]);
        assert!(open(&f).is_empty());
        // a board with no stackup and KiCad's "None" finish: every such note is open, none is guessed
        let bare = facts(br#"(kicad_pcb (general (thickness 1.6)) (layers (0 "F.Cu" signal) (31 "B.Cu" signal)) (setup (stackup (copper_finish "None"))) (gr_line (start 0 0) (end 10 0) (layer "Edge.Cuts")))"#);
        assert_eq!(open(&bare), ["Copper thickness", "Base material", "Surface finish", "Solder mask", "Silkscreen"]);
        assert_eq!(open(&facts(b"(kicad_pcb)")).len(), 8);
        assert_eq!(open(&facts(b"(kicad_pcb (general")).len(), 8, "a file cut short is read as nothing");
        let sheet = text_of(&fabrication(&bare, &META));
        assert!(sheet.starts_with("%PDF-1.4\n") && sheet.ends_with("%%EOF\n"));
    }
    #[test] fn a_via_that_stops_inside_the_board_is_never_a_through_hole() {
        let f = facts(br#"(kicad_pcb (via (at 1 1) (size 0.6) (drill 0.3) (layers "F.Cu" "B.Cu")) (via micro (at 2 1) (size 0.3) (drill 0.1) (layers "F.Cu" "In1.Cu"))
            (via blind (at 3 1) (size 0.6) (drill 0.3) (layers "F.Cu" "In2.Cu")) (via (at 4 1) (size 0.6) (drill 0.3) (layers "In1.Cu" "In2.Cu")) (via (at 5 1) (size 0.6) (drill 1e40) (layers "F.Cu" "B.Cu")))"#);
        let ts = tools(&f.holes);
        assert_eq!(ts.iter().map(|t| (t.span.as_str(), size_of(t), t.count)).collect::<Vec<_>>(), [("", "0.30".to_string(), 1), ("F.Cu-In1.Cu", "0.10".to_string(), 1), ("F.Cu-In2.Cu", "0.30".to_string(), 1), ("In1.Cu-In2.Cu", "0.30".to_string(), 1)]);
        assert_eq!((count(&ts, true, false), count(&ts, true, true), count(&ts, false, false)), ("1 in 1 size, 0.30 to 0.30 mm".to_string(), "3 in 3 sizes, 0.10 to 0.30 mm".to_string(), "none".to_string()));
    }
    #[test] fn a_dielectric_of_several_plies_is_as_thick_as_all_of_them() {
        let f = facts(br#"(kicad_pcb (setup (stackup (layer "F.Cu" (type "copper") (thickness 0.035)) (layer "dielectric 1" (type "core") (thickness 0.6) (material "FR4") (epsilon_r 4.5) addsublayer (thickness 0.4 locked) (material "FR408"))
            (layer "B.Cu" (type "copper") (thickness 0.035)) (layer "B.Mask" (type "Bottom Solder Mask") (color "Black")) (copper_finish "User defined"))))"#);
        assert_eq!((mm(f.stack[1].thickness), f.stack[1].material.as_str()), ("1.00".to_string(), "FR4 + FR408"));
        let ns = notes(&f);
        assert_eq!((ns[4].value.as_str(), ns[5].value.as_str(), ns[6].value.as_str()), ("FR4 + FR408", "", "Black, B.Mask only"));
    }
    #[test] fn millimetres_keep_more_than_two_decimals_only_when_needed() {
        assert_eq!([mm(1.6), mm(0.305), mm(0.0175), mm(0.0), mm(12.0), mm(-0.5), mm(0.00004)], ["1.60", "0.305", "0.0175", "0.00", "12.00", "-0.50", "0.00"]);
    }
    #[test] fn the_drawings_are_whole_pdfs_and_the_same_every_time() {
        let f = facts(BOARD.as_bytes());
        for (make, title) in [(fabrication as fn(&Facts, &Meta) -> Vec<u8>, "/Title (amp v1 fabrication drawing)"), (drill, "/Title (amp v1 drill drawing)")] {
            let a = make(&f, &META);
            assert_eq!(a, make(&facts(BOARD.as_bytes()), &META), "no clock, no machine");
            let t = text_of(&a);
            assert!(t.contains(title) && t.contains("/MediaBox [0 0 841.89 595.28]"), "{title}");
            // the cross-reference table names where each object starts, and the stream is as long as it says
            let xref: usize = t[t.rfind("startxref\n").unwrap() + 10..].lines().next().unwrap().parse().unwrap();
            assert!(t[xref..].starts_with("xref\n0 6\n"));
            for (i, l) in t[xref..].lines().skip(3).take(5).enumerate() { let at: usize = l[..10].parse().unwrap(); assert!(t[at..].starts_with(&format!("{} 0 obj\n", i + 1)), "object {}", i + 1) }
            let (len, s) = (t.find("/Length ").unwrap() + 8, t.find("stream\n").unwrap() + 7);
            let n: usize = t[len..].split(' ').next().unwrap().parse().unwrap();
            assert!(t[s + n..].starts_with("\nendstream"));
        }
        assert_ne!(drill(&f, &META), drill(&f, &Meta { tag: "v2", ..META }), "the title block names the release");
    }
    #[test] fn the_drill_drawing_marks_every_hole_and_lists_every_tool() {
        let f = facts(BOARD.as_bytes());
        let none = Facts { holes: vec![], ..f.clone() };
        let marks = |f: &Facts| drill_sheet(f, &META).ops.iter().filter(|p| matches!(p, Prim::Circ { .. })).count();
        // tool A (0.30, two vias) is the circle: one per via and one in the table
        assert_eq!(marks(&f) - marks(&none), 3);
        assert!(drill_sheet(&f, &META).ops.len() > drill_sheet(&none, &META).ops.len() + 6);
    }
    #[test] fn pdf_text_is_escaped_and_a_long_cell_is_cut() {
        assert_eq!(esc("a(b)\\c\né"), "a\\(b\\)\\\\c");
        let (mut a, mut b) = (Sheet::default(), Sheet::default());
        a.fit(&"W".repeat(200), 0.0, 0.0, 2.5, 30.0); b.fit(&"W".repeat(400), 0.0, 0.0, 2.5, 30.0);
        assert_eq!(a.ops.len(), b.ops.len(), "both are cut to the same width");
        let right = a.ops.iter().filter_map(|p| if let Prim::Line { ps, .. } = p { ps.iter().map(|q| q.x).reduce(f32::max) } else { None }).fold(0.0, f32::max);
        assert!(right <= 30.5, "{right}");
    }
}

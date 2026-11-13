//! Gerbers vs their board: each copper layer, the drills and the outline drawn on a fine grid from both (the board's own items, as the
//! board viewer has them, and the Gerber set's), and where they differ by more than a grid cell's slack marked: copper the board has and
//! the Gerbers do not (removed, red) and copper the Gerbers have and the board does not (added, green). Stale output (the board changed
//! after the export) shows as marks wherever it changed. Inner layers' pads and via rings count as optional (fab output may leave
//! unused ones out); a missing one is no difference, an extra one is.
use super::super::board::{self, Item, Prim, Pt};
use std::sync::Arc;

/// The grid's cell (mm): finer than any fab's smallest feature, coarse enough to stay quick on a large board.
pub const CELL: f32 = 0.025;
/// A difference narrower than this many cells is the two drawings' rounding (arc chords, stroke ends), not a difference.
const SLACK: usize = 2;
/// The smallest difference marked (mm, its larger side).
const MIN: f32 = 0.3;

pub struct Grid { x0: f32, y0: f32, pub w: usize, pub h: usize, bits: Vec<bool> }
impl Grid {
    pub fn new(b: [f32; 4]) -> Grid {
        let (w, h) = ((((b[2] - b[0]) / CELL).ceil() as usize + 2).min(8000), (((b[3] - b[1]) / CELL).ceil() as usize + 2).min(8000));
        Grid { x0: b[0] - CELL, y0: b[1] - CELL, w, h, bits: vec![false; w * h] }
    }
    fn cx(&self, i: usize) -> f32 { self.x0 + (i as f32 + 0.5) * CELL }
    fn cy(&self, j: usize) -> f32 { self.y0 + (j as f32 + 0.5) * CELL }
    fn span(&self, lo: f32, hi: f32, n: usize, o: f32) -> (usize, usize) { (((lo - o) / CELL).floor().max(0.0) as usize, (((hi - o) / CELL).ceil().max(0.0) as usize).min(n)) }
    pub fn get(&self, i: usize, j: usize) -> bool { self.bits[j * self.w + i] }
    fn set(&mut self, i: usize, j: usize, v: bool) { self.bits[j * self.w + i] = v }
    /// a round-ended stroke from a to b, r its half width (r of a disc when a == b)
    fn capsule(&mut self, a: Pt, b: Pt, r: f32, v: bool) {
        let r = r.max(CELL / 2.0);
        let (i0, i1) = self.span(a.x.min(b.x) - r, a.x.max(b.x) + r, self.w, self.x0);
        let (j0, j1) = self.span(a.y.min(b.y) - r, a.y.max(b.y) + r, self.h, self.y0);
        let (dx, dy) = (b.x - a.x, b.y - a.y); let l2 = dx * dx + dy * dy;
        for j in j0..j1 { for i in i0..i1 {
            let (px, py) = (self.cx(i) - a.x, self.cy(j) - a.y);
            let t = if l2 > 0.0 { ((px * dx + py * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let (qx, qy) = (px - t * dx, py - t * dy);
            if qx * qx + qy * qy <= r * r { self.set(i, j, v) }
        } }
    }
    /// a filled outline, even-odd, by rows
    fn poly(&mut self, ps: &[Pt], v: bool) {
        if ps.len() < 3 { return }
        let (ymin, ymax) = ps.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.y), a.1.max(p.y)));
        let (j0, j1) = self.span(ymin, ymax, self.h, self.y0);
        let mut xs: Vec<f32> = vec![];
        for j in j0..j1 {
            let y = self.cy(j); xs.clear();
            for k in 0..ps.len() {
                let (a, b) = (ps[k], ps[(k + 1) % ps.len()]);
                if (a.y <= y) != (b.y <= y) { xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x)) }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xs.chunks_exact(2) {
                let (i0, i1) = (((pair[0] - self.x0) / CELL - 0.5).ceil().max(0.0) as usize, (((pair[1] - self.x0) / CELL - 0.5).floor() as i64 + 1).clamp(0, self.w as i64) as usize);
                for i in i0..i1.max(i0) { self.set(i, j, v) }
            }
        }
    }
    pub fn draw(&mut self, p: &Prim, v: bool) {
        match p {
            Prim::Seg { a, b, w } => self.capsule(*a, *b, w / 2.0, v),
            Prim::Line { ps, w } => for s in ps.windows(2) { self.capsule(s[0], s[1], w / 2.0, v) },
            Prim::Circ { c, r, fill: true, .. } => self.capsule(*c, *c, *r, v),
            Prim::Circ { c, r, w, fill: false } => { let ring: Vec<Pt> = (0..=48).map(|k| { let t = std::f32::consts::TAU * k as f32 / 48.0; Pt { x: c.x + r * t.cos(), y: c.y + r * t.sin() } }).collect(); for s in ring.windows(2) { self.capsule(s[0], s[1], w / 2.0, v) } }
            Prim::Poly { ps, fill: true, .. } => self.poly(ps, v),
            Prim::Poly { ps, w, fill: false } => { for k in 0..ps.len() { self.capsule(ps[k], ps[(k + 1) % ps.len()], w / 2.0, v) } }
            Prim::Tint { ps, .. } => self.poly(ps, v),
        }
    }
}

/// The cells set in `a` and not in `b`, less the slack: a cell counts when every cell within SLACK of it does too.
fn only(a: &Grid, b: &Grid, keep: Option<&Grid>) -> Vec<bool> {
    let (w, h) = (a.w, a.h);
    let raw: Vec<bool> = (0..w * h).map(|k| a.bits[k] && !b.bits[k] && keep.map_or(true, |o| !o.bits[k])).collect();
    // the square of side 2 SLACK + 1 all set: rows first (a run of that length), then columns
    let run = 2 * SLACK + 1;
    let mut rows = vec![false; w * h];
    for j in 0..h { let mut n = 0usize; for i in 0..w { n = if raw[j * w + i] { n + 1 } else { 0 }; if n >= run { rows[j * w + i - SLACK] = true } } }
    let mut out = vec![false; w * h];
    for i in 0..w { let mut n = 0usize; for j in 0..h { n = if rows[j * w + i] { n + 1 } else { 0 }; if n >= run { out[(j - SLACK) * w + i] = true } } }
    out
}
/// The places (boxes, mm) where cells are set: connected groups (8 ways), each grown back by the slack.
fn places(g: &Grid, cells: &[bool]) -> Vec<[f32; 4]> {
    let (w, h) = (g.w, g.h);
    let mut seen = vec![false; w * h]; let mut out = vec![];
    for k in 0..w * h {
        if !cells[k] || seen[k] { continue }
        let (mut i0, mut j0, mut i1, mut j1) = (usize::MAX, usize::MAX, 0, 0);
        let mut st = vec![k]; seen[k] = true;
        while let Some(c) = st.pop() {
            let (i, j) = (c % w, c / w); i0 = i0.min(i); i1 = i1.max(i); j0 = j0.min(j); j1 = j1.max(j);
            for (dx, dy) in [(-1i64, -1i64), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                let (x, y) = (i as i64 + dx, j as i64 + dy);
                if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { continue }
                let n = y as usize * w + x as usize;
                if cells[n] && !seen[n] { seen[n] = true; st.push(n) }
            }
        }
        let s = (SLACK + 1) as f32 * CELL;
        let b = [g.cx(i0) - s, g.cy(j0) - s, g.cx(i1) + s, g.cy(j1) + s];
        // a speck (both sides under MIN) is the two drawings' detail, not a difference a fab would make
        if b[2] - b[0] < MIN && b[3] - b[1] < MIN { continue }
        out.push(b);
        if out.len() >= 500 { break }
    }
    out
}

/// What a comparison found on one layer: its differences, the board's (board only, or changed: the board's and the Gerbers' side by side)
/// and the Gerbers' alone.
#[derive(Debug, Default, PartialEq)]
pub struct Layer { pub name: String, pub removed: usize, pub added: usize, pub places: Vec<Place> }
/// One difference: its box (mm), whether the board has copper here the Gerbers have not (`board`) and the other way (`gerbers`), and what
/// it is, named from the board's items ("track on GND near U5", "pad C12.1 on GND", "zone fill on GND").
#[derive(Clone, Debug, PartialEq)]
pub struct Place { pub b: [f32; 4], pub board: bool, pub gerbers: bool, pub what: String }
impl Place {
    /// "board only", "Gerbers only", or "changed" (both: copper moved)
    pub fn side(&self) -> &'static str { match (self.board, self.gerbers) { (true, true) => "changed", (true, false) => "board only", _ => "Gerbers only" } }
}
/// Differences this close (mm) are one: a moved track is the board's copper and the Gerbers' side by side, a refilled zone many specks
/// along its edge. A difference grows no larger than MERGED_MAX on a side by joining.
const NEAR: f32 = 2.0;
const MERGED_MAX: f32 = 8.0;
fn merge(mut v: Vec<Place>) -> Vec<Place> {
    loop {
        let mut joined = false;
        let mut i = 0;
        while i < v.len() {
            let mut j = i + 1;
            while j < v.len() {
                let (a, b) = (v[i].b, v[j].b);
                let u = [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])];
                let near = a[0] - NEAR <= b[2] && b[0] - NEAR <= a[2] && a[1] - NEAR <= b[3] && b[1] - NEAR <= a[3];
                if near && u[2] - u[0] <= MERGED_MAX && u[3] - u[1] <= MERGED_MAX {
                    let o = v.swap_remove(j);
                    v[i].b = u; v[i].board |= o.board; v[i].gerbers |= o.gerbers; joined = true;
                } else { j += 1 }
            }
            i += 1;
        }
        if !joined { break }
    }
    // the largest first
    v.sort_by(|a, b| ((b.b[2] - b.b[0]) * (b.b[3] - b.b[1])).partial_cmp(&((a.b[2] - a.b[0]) * (a.b[3] - a.b[1]))).unwrap_or(std::cmp::Ordering::Equal));
    v
}
fn field<'a>(info: &'a str, k: &str) -> &'a str { info.split(';').skip(1).find_map(|f| f.strip_prefix(k).and_then(|r| r.strip_prefix('='))).unwrap_or("") }
/// A board item as a difference names it, on layer `l`: "pad C12.1 on GND", "track on GND", "via on GND", "zone fill on GND", "copper
/// shape"; on the drills the hole of it, on Edge.Cuts the outline.
pub fn describe(info: &str, l: u32) -> String {
    let head = info.split(';').next().unwrap_or("");
    let (net, r) = (field(info, "net"), field(info, "ref"));
    let on = if net.is_empty() { String::new() } else { format!(" on {net}") };
    if l == 22 { return "board outline".into() }
    let what = if let Some(n) = head.strip_prefix("pad ") { match (r.is_empty(), n.is_empty()) { (true, _) => format!("pad {n}{on}").replace("  ", " "), (false, true) => format!("pad of {r}{on}"), _ => format!("pad {r}.{n}{on}") } }
        else { match head { "via" => format!("via{on}"), "track" => format!("track{on}"), "zone" => format!("zone fill{on}"), "gr_poly" => "copper shape".into(),
            h if !r.is_empty() => format!("{} of {r}", h.trim_start_matches("fp_")), h => h.trim_start_matches("gr_").to_string() } };
    if l == 24 { format!("hole of {what}") } else { what }
}
/// The board's item that holds most of a difference's cells (`cells`, of grid `g`, within `b`): the one its name comes from.
fn holder<'a>(b: [f32; 4], g: &Grid, cells: &[bool], cands: &[(&'a Item, [f32; 4])]) -> Option<&'a Item> {
    let (i0, i1) = g.span(b[0], b[2], g.w, g.x0); let (j0, j1) = g.span(b[1], b[3], g.h, g.y0);
    if i1 <= i0 || j1 <= j0 { return None }
    let mut best: Option<(&Item, usize)> = None;
    for (it, ib) in cands {
        if ib[0] > b[2] || ib[2] < b[0] || ib[1] > b[3] || ib[3] < b[1] { continue }
        let mut sub = Grid { x0: g.x0 + i0 as f32 * CELL, y0: g.y0 + j0 as f32 * CELL, w: i1 - i0, h: j1 - j0, bits: vec![false; (i1 - i0) * (j1 - j0)] };
        sub.draw(&it.prim, true);
        let mut n = 0;
        for j in 0..sub.h { for i in 0..sub.w { if sub.bits[j * sub.w + i] && cells[(j + j0) * g.w + i + i0] { n += 1 } } }
        if n > 0 && best.map_or(true, |x| n > x.1) { best = Some((it, n)) }
    }
    best.map(|x| x.0)
}
/// The part nearest a place (within 5 mm), by its pads.
fn near(b: [f32; 4], pads: &[(Pt, &str)]) -> Option<String> {
    let c = Pt { x: (b[0] + b[2]) / 2.0, y: (b[1] + b[3]) / 2.0 };
    pads.iter().map(|(p, r)| ((p.x - c.x).hypot(p.y - c.y), *r)).filter(|x| x.0 <= 5.0).min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)).map(|x| x.1.to_string())
}
/// A difference's colours: the board's copper the Gerbers lack (solid red), the Gerbers' copper the board lacks (green, striped: told apart without colour).
pub const BOARD_ONLY: u32 = 0xe5484d;
pub const GERBERS_ONLY: u32 = 0x30a46c;
/// A difference's place as its info says it (the probe that lights and zooms on it): "at=x,y" to the hundredth of a mm.
pub fn at_text(b: [f32; 4]) -> String { format!("{:.2},{:.2}", (b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0) }
/// A difference drawn as itself: its cells (grid resolution) merged into rectangles, on the layer it is of, as tints (their own colour, shown and
/// hidden with the layer); `striped` leaves out every other pair of rows.
fn shapes(g: &Grid, raw: &[bool], b: [f32; 4], layer: u32, color: u32, striped: bool, info: &board::Info, key: &str, out: &mut Vec<Item>) {
    let (i0, i1) = g.span(b[0], b[2], g.w, g.x0); let (j0, j1) = g.span(b[1], b[3], g.h, g.y0);
    let mut open: Vec<(usize, usize, usize)> = vec![]; // (from, to, first row) of runs still growing down
    let mut emit = |a: usize, z: usize, r0: usize, r1: usize, out: &mut Vec<Item>| {
        let (x0, x1, y0, y1) = (g.x0 + a as f32 * CELL, g.x0 + z as f32 * CELL, g.y0 + r0 as f32 * CELL, g.y0 + r1 as f32 * CELL);
        let k = format!("{key}/{}", out.len());
        out.push(Item { sig: board::hash(k.as_bytes(), board::FNV), key: k, layer, prim: board::tint_of(vec![Pt { x: x0, y: y0 }, Pt { x: x1, y: y0 }, Pt { x: x1, y: y1 }, Pt { x: x0, y: y1 }], color, 235), info: info.clone() });
    };
    for j in j0..=j1 {
        let mut runs: Vec<(usize, usize)> = vec![];
        if j < j1 && !(striped && (j / 2) % 2 == 1) {
            let mut i = i0;
            while i < i1 { if raw[j * g.w + i] { let a = i; while i < i1 && raw[j * g.w + i] { i += 1 } runs.push((a, i)) } else { i += 1 } }
        }
        let mut next = vec![];
        for (a, z, r0) in open.drain(..) { if runs.contains(&(a, z)) { next.push((a, z, r0)) } else { emit(a, z, r0, j, out) } }
        for (a, z) in runs { if !next.iter().any(|x| x.0 == a && x.1 == z) { next.push((a, z, j)) } }
        open = next;
        if out.len() > 40_000 { return }
    }
}

fn bbox(items: &[&Item]) -> [f32; 4] {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    let mut add = |p: &Pt, r: f32| { b = [b[0].min(p.x - r), b[1].min(p.y - r), b[2].max(p.x + r), b[3].max(p.y + r)] };
    for it in items { match &it.prim {
        Prim::Seg { a, b: q, w } => { add(a, *w); add(q, *w) }
        Prim::Line { ps, w } => for p in ps { add(p, *w) },
        Prim::Circ { c, r, w, .. } => add(c, r + w),
        Prim::Poly { ps, w, .. } => for p in ps { add(p, *w) },
        Prim::Tint { ps, .. } => for p in ps { add(p, 0.0) },
    } }
    b
}

/// The layers compared: (name, the board's layer, whether its pads and vias may be left out of the Gerber).
fn compared() -> Vec<(&'static str, u32, bool)> {
    vec![("F.Cu", 12, false), ("In1.Cu", 11, true), ("In2.Cu", 10, true), ("In3.Cu", 9, true), ("In4.Cu", 8, true), ("In5.Cu", 7, true), ("In6.Cu", 6, true), ("B.Cu", 5, false), ("Drills", 24, false), ("Edge.Cuts", 22, false)]
}
const PADS: u32 = 13; const VIAS: u32 = 23;

/// The comparison of a Gerber set's items with its board's: marks (tints on the layers they are of: BOARD_ONLY, GERBERS_ONLY striped) and what each layer came to. Only the layers
/// the set has are compared (a set without inner layers says nothing of them).
/// What the board viewer's items leave out and fab output has: the board's top-level copper polygons (gr_poly on a copper layer), read
/// from the board file's forms here.
pub fn board_copper_polys(forms: &[&[u8]]) -> Vec<Item> {
    let mut out = vec![];
    for f in forms {
        if super::super::schematic::head(f) != b"gr_poly" { continue }
        let x = super::super::sexpr::parse(f);
        let layer = x.get("layer").arg(0);
        if !layer.ends_with(".Cu") { continue }
        let ps: Vec<Pt> = x.get("pts").all("xy").map(|q| Pt { x: q.f(0), y: q.f(1) }).collect();
        let fill = matches!(x.get("fill").arg(0), "yes" | "solid");
        let w = x.get("stroke").get("width").f(0).max(x.get("width").f(0));
        let key = format!("grpoly/{}", out.len());
        out.push(Item { sig: 0, key, layer: board::layer_id(layer), prim: Prim::Poly { ps, w, fill }, info: Arc::from(format!("gr_poly;layer={layer}").as_str()) });
    }
    out
}
/// A custom pad's own shape the board viewer does not draw (only its anchor): this far round its anchor nothing is called a difference.
const CUSTOM_PAD: f32 = 1.5;

/// One layer's comparison (None when the set has not got the layer).
fn one_layer(gerber: &[Item], board_items: &[Item], pads: &[(Pt, &str)], b: [f32; 4], name: &'static str, l: u32, optional: bool) -> Option<(Vec<Item>, Layer)> {
    let mut marks = vec![];
    let g_items: Vec<&Item> = gerber.iter().filter(|i| i.layer == l).collect();
    if g_items.is_empty() { return None }
    let copper = (5..=12).contains(&l);
    // the board's: the layer's own items, and on copper the pads of every layer (*.Cu) and the vias (their rings)
    let mut need = Grid::new(b); let mut may = Grid::new(b); let mut gg = Grid::new(b);
    let mut cands: Vec<(&Item, [f32; 4])> = vec![];
    for it in board_items {
        // a slot inside its pad's copper is that pad's (the board viewer draws an oval drill as a circle)
        if l == 24 && it.layer == PADS && !matches!(it.prim, Prim::Tint { .. }) { may.draw(&it.prim, true); continue }
        let shared = copper && (it.layer == PADS || it.layer == VIAS);
        // a non-plated hole's pad is no copper
        if it.info.contains("type=np_thru_hole") && l != 24 { continue }
        if it.layer == l || shared {
            if matches!(it.prim, Prim::Tint { .. }) { continue }
            // vias are optional on every layer (a blind or micro via's ring is on its own layers only; the drills are compared apart)
            if (shared && optional) || it.layer == VIAS { may.draw(&it.prim, true) } else { need.draw(&it.prim, true) }
            if copper && it.info.contains("shape=custom") { let c = match &it.prim { Prim::Circ { c, .. } => *c, Prim::Poly { ps, .. } if !ps.is_empty() => ps[0], _ => continue }; may.capsule(c, c, CUSTOM_PAD, true) }
            cands.push((it, bbox(&[it])));
        }
    }
    // the Gerber's, in order: a clear (a tint) takes away what is under it
    for it in &g_items { gg.draw(&it.prim, !matches!(it.prim, Prim::Tint { .. })) }
    let mut either = Grid::new(b); for k in 0..either.bits.len() { either.bits[k] = need.bits[k] || may.bits[k] }
    let removed = places(&need, &only(&need, &gg, None));
    let added = places(&gg, &only(&gg, &either, None));
    // the differences as themselves: the cells that differ, within each place, on the layer they are of
    let raw_removed: Vec<bool> = (0..need.bits.len()).map(|k| need.bits[k] && !gg.bits[k]).collect();
    let raw_added: Vec<bool> = (0..gg.bits.len()).map(|k| gg.bits[k] && !either.bits[k]).collect();
    let all: Vec<Place> = removed.iter().map(|r| Place { b: *r, board: true, gerbers: false, what: String::new() }).chain(added.iter().map(|r| Place { b: *r, board: false, gerbers: true, what: String::new() })).collect();
    let mut out = merge(all);
    for (i, p) in out.iter_mut().enumerate() {
        // named by the board's item under its board-side cells; copper only the Gerbers have, by the item it touches or the part it is near
        let held = if p.board { holder(p.b, &need, &raw_removed, &cands) } else { None };
        let held = held.or_else(|| { let g = [p.b[0] - 0.1, p.b[1] - 0.1, p.b[2] + 0.1, p.b[3] + 0.1]; holder(g, &need, &need.bits, &cands) });
        let named = held.map(|it| describe(&it.info, l));
        let by = near(p.b, pads).filter(|r| !named.as_deref().map_or(false, |n| n.contains(&format!(" {r}.")) || n.ends_with(&format!(" of {r}"))));
        p.what = match (named, p.board) {
            (Some(n), true) => n,
            (Some(n), false) => format!("{} beside {n}", match l { 24 => "hole", 22 => "outline", _ => "copper" }),
            (None, _) => match l { 24 => "hole", 22 => "outline", _ => "copper" }.into(),
        };
        if let Some(r) = by { p.what.push_str(&format!(" near {r}")) }
        let info: board::Info = Arc::from(format!("difference: {} · {};layer={name};at={};size={:.2}x{:.2} mm", p.side(), p.what, at_text(p.b), p.b[2] - p.b[0], p.b[3] - p.b[1]).as_str());
        if p.board { shapes(&need, &raw_removed, p.b, l, BOARD_ONLY, false, &info, &format!("cmp/{name}/b{i}"), &mut marks) }
        if p.gerbers { shapes(&gg, &raw_added, p.b, l, GERBERS_ONLY, true, &info, &format!("cmp/{name}/g{i}"), &mut marks) }
    }
    let removed = out.iter().filter(|p| p.board).count();
    Some((marks, Layer { name: name.into(), removed, added: out.len() - removed, places: out }))
}

pub fn compare(gerber: &[Item], board_items: &[Item]) -> (Vec<Item>, Vec<Layer>) {
    let (mut marks, mut out) = (vec![], vec![]);
    let all: Vec<&Item> = gerber.iter().chain(board_items.iter()).collect();
    let b = bbox(&all);
    if b[0] > b[2] { return (marks, out) }
    // the parts' pads (their middles), for "near U5"
    let pads: Vec<(Pt, &str)> = board_items.iter().filter(|i| i.layer == PADS).filter_map(|i| { let r = field(&i.info, "ref"); let q = bbox(&[i]); (!r.is_empty()).then(|| (Pt { x: (q[0] + q[2]) / 2.0, y: (q[1] + q[3]) / 2.0 }, r)) }).collect();
    // each layer on a thread of its own (a large board's ten layers at once)
    let per: Vec<Option<(Vec<Item>, Layer)>> = std::thread::scope(|sc| {
        let hs: Vec<_> = compared().into_iter().map(|(name, l, optional)| { let pads = &pads; sc.spawn(move || one_layer(gerber, board_items, pads, b, name, l, optional)) }).collect();
        hs.into_iter().map(|h| h.join().unwrap_or(None)).collect()
    });
    for (m, lay) in per.into_iter().flatten() { marks.extend(m); out.push(lay) }
    (marks, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn it(layer: u32, prim: Prim) -> Item { Item { key: String::new(), sig: 0, layer, prim, info: Arc::from("") } }
    fn p(x: f32, y: f32) -> Pt { Pt { x, y } }
    #[test] fn the_same_copper_drawn_two_ways_is_no_difference() {
        // the board's pad as a rectangle, the Gerber's as a region; the track as one stroke and as two halves
        let board = vec![it(12, Prim::Poly { ps: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 0.5), p(0.0, 0.5)], w: 0.0, fill: true }), it(12, Prim::Seg { a: p(2.0, 0.0), b: p(6.0, 0.0), w: 0.25 })];
        let gerb = vec![it(12, Prim::Poly { ps: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 0.5), p(0.0, 0.5), p(0.0, 0.0)], w: 0.0, fill: true }), it(12, Prim::Line { ps: vec![p(2.0, 0.0), p(4.0, 0.0), p(6.0, 0.0)], w: 0.25 })];
        let (m, l) = compare(&gerb, &board);
        assert!(m.is_empty(), "{:?}", m.iter().map(|i| i.info.to_string()).collect::<Vec<_>>());
        assert_eq!((l.len(), l[0].name.as_str(), l[0].removed, l[0].added), (1, "F.Cu", 0, 0));
    }
    #[test] fn a_moved_track_and_a_missing_pad_are_marked() {
        let board = vec![it(12, Prim::Seg { a: p(0.0, 0.0), b: p(5.0, 0.0), w: 0.2 }), it(13, Prim::Circ { c: p(10.0, 0.0), r: 0.5, w: 0.0, fill: true })];
        // the Gerber's track 0.3 mm over; its pad missing
        let gerb = vec![it(12, Prim::Seg { a: p(0.0, 0.3), b: p(5.0, 0.3), w: 0.2 })];
        let (m, l) = compare(&gerb, &board);
        // the track's two sides (the board's, the Gerbers') are one difference: changed; the pad another
        assert_eq!(l[0].places.iter().map(|p| p.side()).collect::<Vec<_>>(), ["changed", "board only"], "{:?}", l[0].places);
        assert!(m.iter().any(|i| i.layer == 12 && i.info.contains("board only") && i.info.contains("at=10.00,0.00")), "the pad, marked on F.Cu where it is");
        // drawn as itself: the marks lie within the pad's own extent (its cells), not a grown block
        let pad: Vec<&Item> = m.iter().filter(|i| i.info.contains("at=10.00,0.00")).collect();
        let ext = pad.iter().flat_map(|i| match &i.prim { Prim::Tint { ps, .. } => ps.clone(), _ => vec![] }).fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |a, q| [a[0].min(q.x), a[1].min(q.y), a[2].max(q.x), a[3].max(q.y)]);
        assert!(ext[0] >= 9.45 && ext[2] <= 10.55 && ext[1] >= -0.55 && ext[3] <= 0.55, "{ext:?}");
        // the Gerbers' side of the moved track striped green, the board's red
        let colors: std::collections::BTreeSet<u32> = m.iter().filter_map(|i| match i.prim { Prim::Tint { color, .. } => Some(color), _ => None }).collect();
        assert_eq!(colors.len(), 2, "red and green");
        // on an inner layer a missing pad is no difference (fab output leaves unused ones out), an extra one is
        let board = vec![it(13, Prim::Circ { c: p(10.0, 0.0), r: 0.5, w: 0.0, fill: true })];
        let gerb = vec![it(11, Prim::Circ { c: p(3.0, 0.0), r: 0.5, w: 0.0, fill: true })];
        let (_, l) = compare(&gerb, &board);
        assert_eq!((l[0].name.as_str(), l[0].removed, l[0].added), ("In1.Cu", 0, 1));
        assert_eq!(l[0].places[0].side(), "Gerbers only");
        // a layer the set has not got is not compared
        assert!(compare(&[], &board).1.is_empty());
    }
    #[test] fn a_clear_takes_away() {
        let board = vec![it(12, Prim::Poly { ps: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)], w: 0.0, fill: true })];
        let gerb = vec![it(12, Prim::Poly { ps: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)], w: 0.0, fill: true }), it(12, board::tint_of(vec![p(1.0, 1.0), p(2.0, 1.0), p(2.0, 2.0), p(1.0, 2.0)], 0x10141a, 255))];
        let (_, l) = compare(&gerb, &board);
        assert_eq!((l[0].removed, l[0].added), (1, 0));
    }
    /// A difference says what it is from the board: the item under it, its net, the part it is near; specks close together are one.
    #[test] fn differences_are_named_and_joined() {
        let named = |i: &str, p: Prim| Item { key: String::new(), sig: 0, layer: 12, prim: p, info: Arc::from(i) };
        let board = vec![
            named("track;net=GND;layer=F.Cu;width=0.25", Prim::Seg { a: p(0.0, 0.0), b: p(4.0, 0.0), w: 0.25 }),
            Item { layer: 13, ..named("pad 1;net=VBUS;type=smd;shape=rect;ref=C12;value=10u", Prim::Poly { ps: vec![p(9.5, -0.5), p(10.5, -0.5), p(10.5, 0.5), p(9.5, 0.5)], w: 0.0, fill: true }) },
            named("zone;net=GND;layer=F.Cu", Prim::Poly { ps: vec![p(20.0, 0.0), p(30.0, 0.0), p(30.0, 10.0), p(20.0, 10.0)], w: 0.0, fill: true }),
        ];
        // the Gerbers: the pad only, and the zone with three bites out of its edge 0.5 mm apart
        let mut gerb = vec![it(12, Prim::Poly { ps: vec![p(9.5, -0.5), p(10.5, -0.5), p(10.5, 0.5), p(9.5, 0.5)], w: 0.0, fill: true }), it(12, Prim::Poly { ps: vec![p(20.0, 0.0), p(30.0, 0.0), p(30.0, 10.0), p(20.0, 10.0)], w: 0.0, fill: true })];
        for k in 0..3 { let x = 22.0 + k as f32 * 0.9; gerb.push(it(12, board::tint_of(vec![p(x, 0.0), p(x + 0.4, 0.0), p(x + 0.4, 0.4), p(x, 0.4)], 0, 255))) }
        let (_, l) = compare(&gerb, &board);
        let mut what: Vec<String> = l[0].places.iter().map(|p| format!("{}: {}", p.side(), p.what)).collect(); what.sort();
        assert_eq!(what, ["board only: track on GND", "board only: zone fill on GND"], "the bites one difference, each named by its item");
        assert_eq!(describe("pad 1;net=VBUS;type=smd;ref=C12", 12), "pad C12.1 on VBUS");
        assert_eq!(describe("via;net=GND;size=0.6", 24), "hole of via on GND");
        assert_eq!(describe("pad ;type=thru_hole;ref=J5", 24), "hole of pad of J5", "a pad with no number");
        assert_eq!(describe("fp_line;layer=F.Cu;ref=U5", 12), "line of U5");
        assert_eq!(describe("gr_line;layer=Edge.Cuts", 22), "board outline");
        // a track near a part says so
        let board = vec![named("track;net=SDA", Prim::Seg { a: p(0.0, 0.0), b: p(2.0, 0.0), w: 0.25 }), Item { layer: 13, ..named("pad 3;net=SDA;ref=U5", Prim::Circ { c: p(3.0, 1.0), r: 0.3, w: 0.0, fill: true }) }];
        let gerb = vec![it(12, Prim::Circ { c: p(3.0, 1.0), r: 0.3, w: 0.0, fill: true })];
        let (_, l) = compare(&gerb, &board);
        assert_eq!(l[0].places[0].what, "track on SDA near U5");
    }
}

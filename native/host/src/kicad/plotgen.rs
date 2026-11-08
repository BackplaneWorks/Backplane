//! core/plot's making of a plot, and server/plot's `Pv.child`, in Rust: a board or schematic file to the same `Plot`
//! child prints, and the same info file. Items become pieces (integer micrometres), sorted by (layer, mode, width, bucket, key hash),
//! runs of one rank become chunks of delta-coded varints. Pieces of one rank and key hash keep the order sort leaves them in:
//! the forms are dealt into hands as Pcb2.par deals them and the items as Piece.psort does, and the sorts and merges are stable alike.
use super::board::{self, Info, Item, Prim, Pt};
use super::fl;
use super::plot::{Chunk, Plot, Rect};
use super::schematic;
use super::sexpr;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Num.um: millimetres to micrometres, two's complement
pub fn um(x: f32) -> u32 { if x < 0.0 { 0u32.wrapping_sub(fl::to_u32(fl::round((-x) * 1000.0))) } else { fl::to_u32(fl::round(x * 1000.0)) } }
fn zig(d: u32) -> u32 { (d << 1) ^ 0u32.wrapping_sub(d >> 31) }

// LO.Look
fn look_board(i: u32) -> (u32, u32) {
    match i {
        0 => (5791108, 0), 1 => (2549247, 0), 2 => (49858, 0), 3 => (15250087, 200), 4 => (196590, 0), 5 => (5078980, 190), 6 => (8421631, 150), 7 => (13148979, 150), 8 => (12900552, 150),
        9 => (8374015, 150), 10 => (13532460, 160), 11 => (8340095, 160), 12 => (13120564, 210), 13 => (13148212, 220), 14 => (14045866, 0), 15 => (11513775, 0), 16 => (15920545, 230),
        17 => (16721634, 0), 18 => (11513775, 0), 22 => (13685453, 255), 23 => (11842740, 255), 24 => (526603, 255), _ => (10000536, 90),
    }
}
fn look_plain(kind: u32, layer: u32) -> (u32, u32) { if kind == 1 { schematic::look(layer) } else { look_board(layer) } }
pub const DIFF: u32 = 8;
/// Look.of: a layer's colour and opacity in a board (0), a schematic (1), or a comparison of either (kind | DIFF)
pub fn look(kind: u32, layer: u32) -> (u32, u32) {
    if kind < DIFF { return look_plain(kind, layer) }
    if (26..=28).contains(&layer) { return (match layer { 26 => 15026253, 27 => 16098596, _ => 3187820 }, 235) }
    let (c, a) = look_plain(kind & 7, layer);
    (c, a / 3)
}

// Rank: layer, mode, width, bucket in one U32: 5 + 2 + 21 + 4 bits
fn rank_of(layer: u32, mode: u32, width: u32, kh: u32) -> u32 { (layer << 27) | (mode << 25) | (width.min(2097151) << 4) | (kh & 15) }
fn fnv(s: &str, mut h: u32) -> u32 { for c in s.chars() { h = (h ^ c as u32).wrapping_mul(16777619) } h }

pub struct Piece { rank: u32, kh: u32, n: u32, ints: Vec<u32>, info: Info }
fn ints(ps: &[Pt], closed: bool) -> (u32, Vec<u32>) {
    let n = ps.len() + (closed && !ps.is_empty()) as usize;
    let mut v = Vec::with_capacity(n * 2);
    for p in ps { v.push(um(p.x)); v.push(um(p.y)) }
    if closed { if let Some(p) = ps.first() { v.push(um(p.x)); v.push(um(p.y)) } }
    (n as u32, v)
}
fn ring(c: Pt, r: f32) -> Vec<Pt> { (0..=32u32).map(|i| { let a = (i as f32 / 32.0) * (2.0 * fl::PI); Pt { x: c.x + r * fl::cos(a), y: c.y + r * fl::sin(a) } }).collect() }
/// Piece.prim: how one primitive is drawn: its mode, width and points
fn piece(p: Prim, layer: u32, kh: u32, info: Info) -> Piece {
    let (mode, width, (n, xs)) = match p {
        Prim::Seg { a, b, w } => (1, um(w), ints(&[a, b], false)),
        Prim::Line { ps, w } => (1, um(w), ints(&ps, false)),
        Prim::Circ { c, r, fill: true, .. } => (2, um(r * 2.0), ints(&[c], false)),
        Prim::Circ { c, r, w, fill: false } => (1, um(w), ints(&ring(c, r), false)),
        Prim::Poly { ps, fill: true, .. } => (0, 0, ints(&ps, false)),
        Prim::Poly { ps, w, fill: false } => (1, um(w), ints(&ps, true)),
        Prim::Tint { ps, color, alpha } => (0, board::tint_code(color, alpha), ints(&ps, false)),
    };
    Piece { rank: rank_of(layer, mode, width, kh), kh, n, ints: xs, info }
}

/// Box: signed extents
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bx { x0: i32, y0: i32, x1: i32, y1: i32 }
impl Bx {
    pub fn empty() -> Bx { Bx { x0: i32::MAX, y0: i32::MAX, x1: i32::MIN, y1: i32::MIN } }
    fn pt(&mut self, x: u32, y: u32) { let (x, y) = (x as i32, y as i32); self.x0 = self.x0.min(x); self.y0 = self.y0.min(y); self.x1 = self.x1.max(x); self.y1 = self.y1.max(y) }
    fn with(&mut self, o: &Bx) { self.x0 = self.x0.min(o.x0); self.y0 = self.y0.min(o.y0); self.x1 = self.x1.max(o.x1); self.y1 = self.y1.max(o.y1) }
    fn rect(&self) -> Rect { Rect { x0: self.x0 as u32, y0: self.y0 as u32, x1: self.x1 as u32, y1: self.y1 as u32 } }
    fn edge_pts(&mut self, ps: &[Pt], r: f32) { for p in ps { self.pt(um(p.x - r), um(p.y - r)); self.pt(um(p.x + r), um(p.y + r)) } }
    /// Edge.prim: a board's rectangle takes a circle to its radius, stroke widths left out
    fn edge(&mut self, p: &Prim) {
        match p {
            Prim::Seg { a, b, .. } => self.edge_pts(&[*a, *b], 0.0),
            Prim::Line { ps, .. } | Prim::Poly { ps, .. } | Prim::Tint { ps, .. } => self.edge_pts(ps, 0.0),
            Prim::Circ { c, r, .. } => self.edge_pts(&[*c], *r),
        }
    }
}

// Work over the cores
const BLOCK: usize = 16;
fn workers() -> usize { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(8) }
/// f(0..n) on a few threads of their own (a panic comes back here), answers in order.
fn par_map<T: Send>(n: usize, f: &(dyn Fn(usize) -> T + Sync)) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let parts: Vec<Vec<(usize, T)>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..workers().min(n.max(1))).map(|_| std::thread::Builder::new().stack_size(16 << 20).spawn_scoped(s, || {
            let mut v = vec![];
            loop {
                let a = next.fetch_add(BLOCK, Ordering::Relaxed);
                if a >= n { break }
                for i in a..(a + BLOCK).min(n) { v.push((i, f(i))) }
            }
            v
        }).expect("a plot thread")).collect();
        hs.into_iter().map(|h| h.join().unwrap_or_else(|e| std::panic::resume_unwind(e))).collect()
    });
    let mut out: Vec<Option<T>> = (0..n).map(|_| None).collect();
    for p in parts { for (i, t) in p { out[i] = Some(t) } }
    out.into_iter().map(|t| t.unwrap()).collect()
}
/// Board.deal, Part.deal, Item.deal: alternately into two hands, each newest first
fn deal(fs: Vec<u32>) -> (Vec<u32>, Vec<u32>) {
    let (mut a, mut b) = (Vec::with_capacity(fs.len() / 2 + 1), Vec::with_capacity(fs.len() / 2 + 1));
    for f in fs { a.push(f); std::mem::swap(&mut a, &mut b) }
    a.reverse(); b.reverse();
    (a, b)
}
fn hands(d: u32, fs: Vec<u32>, out: &mut Vec<u32>) { if d == 0 { out.extend(fs) } else { let (a, b) = deal(fs); hands(d - 1, a, out); hands(d - 1, b, out) } }
/// the forms in the order Pcb2.par and Sch.par leave their items
fn form_order(n: usize) -> Vec<u32> { let mut o = Vec::with_capacity(n); hands(8, (0..n as u32).collect(), &mut o); o }

/// Piece.psort: items dealt into 2^d hands, each hand's pieces (backwards, as Piece.of makes them) sorted, hands merged, the first hand first among equals
fn psort(d: u32, its: Vec<u32>, ps: &[Option<Piece>]) -> Vec<(u64, u32)> {
    if d == 0 {
        let mut v: Vec<(u64, u32)> = its.iter().rev().filter_map(|i| ps[*i as usize].as_ref().map(|p| (((p.rank as u64) << 32) | p.kh as u64, *i))).collect();
        v.sort_by_key(|e| e.0);
        return v
    }
    let big = its.len() >= 1 << 16 && d >= 3;
    let (a, b) = deal(its);
    let (x, y) = if big { std::thread::scope(|s| { let h = s.spawn(|| psort(d - 1, a, ps)); let y = psort(d - 1, b, ps); (h.join().unwrap_or_else(|e| std::panic::resume_unwind(e)), y) }) } else { (psort(d - 1, a, ps), psort(d - 1, b, ps)) };
    let mut o = Vec::with_capacity(x.len() + y.len());
    let (mut i, mut j) = (0, 0);
    while i < x.len() && j < y.len() { if x[i].0 <= y[j].0 { o.push(x[i]); i += 1 } else { o.push(y[j]); j += 1 } }
    o.extend_from_slice(&x[i..]); o.extend_from_slice(&y[j..]);
    o
}

struct Enc<'p> { d: Vec<u8>, px: u32, py: u32, h: u32, tab: HashMap<&'p str, u32>, info: String }
impl<'p> Enc<'p> {
    fn put(&mut self, c: u8) { self.d.push(c); self.h = (self.h ^ c as u32).wrapping_mul(16777619) }
    fn num(&mut self, mut z: u32) { while z >= 128 { self.put((z & 127) as u8 | 128); z >>= 7 } self.put(z as u8) }
    fn pts(&mut self, xs: &[u32]) {
        for q in xs.chunks_exact(2) { let (x, y) = (q[0], q[1]); let (dx, dy) = (x.wrapping_sub(self.px), y.wrapping_sub(self.py)); self.px = x; self.py = y; self.num(zig(dx)); self.num(zig(dy)) }
    }
    fn idx(&mut self, s: &'p str) {
        let i = match self.tab.get(s) { Some(i) => *i, None => { let n = self.tab.len() as u32; self.tab.insert(s, n); self.info.push_str(s); self.info.push('\0'); n } };
        self.num(i)
    }
}
/// Chunk.tint: fills with a width are tints, their colour their own; the rest take their layer's
fn chunk_look(kind: u32, rank: u32) -> (u32, u32) {
    let (layer, mode, width) = (rank >> 27, (rank >> 25) & 3, (rank >> 4) & 2097151);
    if mode == 0 && width != 0 { (board::tint_color(width), board::tint_alpha(width)) } else { look(kind, layer) }
}
/// Chunk.of: one run of equal rank. Its sig is the running hash of the bytes, on through the info table last char first, xor the rank.
fn chunk(run: &[(u64, u32)], ps: &[Option<Piece>], kind: u32) -> (Chunk, String) {
    let rank = (run[0].0 >> 32) as u32;
    let (layer, mode, width) = (rank >> 27, (rank >> 25) & 3, (rank >> 4) & 2097151);
    let mut e = Enc { d: Vec::new(), px: 0, py: 0, h: board::FNV, tab: HashMap::new(), info: String::new() };
    for (_, i) in run {
        let p = ps[*i as usize].as_ref().unwrap();
        e.idx(&p.info);
        if mode != 2 { e.num(p.n) }
        e.pts(&p.ints);
    }
    let (color, alpha) = chunk_look(kind, rank);
    let mut h = e.h;
    for c in e.info.chars().rev() { h = (h ^ c as u32).wrapping_mul(16777619) }
    (Chunk { sig: h ^ rank, layer, color, alpha, mode, width, d: e.d, info: String::new() }, e.info)
}

/// Plot.of: the chunks of some pieces in item order (None: a hidden layer's), their extent, and the board's rectangle; beside it the info file (Infos.file).
fn plot_of(ps: Vec<Option<Piece>>, kind: u32, edge: Bx) -> (Plot, String) {
    let sorted = psort(5, (0..ps.len() as u32).collect(), &ps);
    let mut runs: Vec<&[(u64, u32)]> = vec![];
    let (mut a, mut bx) = (0, Bx::empty());
    for i in 1..=sorted.len() { if i == sorted.len() || sorted[i].0 >> 32 != sorted[a].0 >> 32 { runs.push(&sorted[a..i]); a = i } }
    for p in ps.iter().flatten() { for q in p.ints.chunks_exact(2) { bx.pt(q[0], q[1]) } }
    let done = par_map(runs.len(), &|i| chunk(runs[i], &ps, kind));
    let mut infos = String::new();
    let mut cs = Vec::with_capacity(done.len());
    for (c, t) in done { infos.push_str(&format!("{} {} ", c.sig, t.chars().count())); infos.push_str(&t); infos.push('\n'); cs.push(c) }
    (Plot { cs, b: bx.rect(), thick: 1600, solid: None, edge: edge.rect() }, infos)
}
/// a piece per item with its layer's opacity; the edge box beside; infos shared within the thread (a board has few distinct ones)
struct Shown { kind: u32, edge_layer: u32 }
thread_local! { static SEEN: std::cell::RefCell<HashSet<Info>> = std::cell::RefCell::new(HashSet::new()); }
fn shared(i: Info) -> Info { SEEN.with(|m| { let mut m = m.borrow_mut(); match m.get(&*i) { Some(a) => a.clone(), None => { m.insert(i.clone()); i } } }) }
impl Shown {
    fn new(kind: u32) -> Shown { Shown { kind, edge_layer: if kind & 7 == 0 { board::EDGE } else { u32::MAX } } }
    fn take(&self, it: Item, edge: &mut Bx) -> Option<Piece> {
        if it.layer == self.edge_layer { edge.edge(&it.prim) }
        if look(self.kind, it.layer).1 == 0 { return None }
        Some(piece(it.prim, it.layer, fnv(&it.key, board::FNV), shared(it.info)))
    }
}
/// a board's items, form by form, in any thread (Pcb2.one)
fn board_form<'b>(f: &'b [u8], i: usize, sig: bool) -> Vec<Item> {
    let mut o = vec![];
    let x = sexpr::parse(f);
    board::form(&mut o, &x, i, if sig { board::hash(f, board::FNV) } else { 0 });
    o
}
fn board_plot(fs: &[&[u8]]) -> (Plot, String) {
    let sh = Shown::new(0);
    let per: Vec<(Vec<Option<Piece>>, Bx)> = par_map(fs.len(), &|i| {
        let mut e = Bx::empty();
        let v: Vec<Option<Piece>> = board_form(fs[i], i, false).into_iter().map(|it| sh.take(it, &mut e)).collect();
        (v, e)
    });
    let (mut edge, total) = (Bx::empty(), per.iter().map(|p| p.0.len()).sum::<usize>());
    let mut slots: Vec<(Vec<Option<Piece>>, Bx)> = per;
    let mut ps = Vec::with_capacity(total);
    for i in form_order(fs.len()) { let (v, e) = &mut slots[i as usize]; edge.with(e); ps.append(v) }
    plot_of(ps, 0, edge)
}
/// Pcb2.items.text
pub fn board_items(fs: &[&[u8]]) -> Vec<Item> {
    let mut per = par_map(fs.len(), &|i| board_form(fs[i], i, true));
    let mut o = vec![];
    for i in form_order(fs.len()) { o.append(&mut per[i as usize]) }
    o
}
/// Sch.items over a file's forms
pub fn sch_items(fs: &[&[u8]]) -> Vec<Item> {
    schematic::items(fs, &|n, f| par_map(n, f), &form_order(fs.len()))
}
/// Plot.of over items
pub fn items_plot(its: Vec<Item>, kind: u32) -> (Plot, String) {
    let (sh, mut edge) = (Shown::new(kind), Bx::empty());
    let ps: Vec<Option<Piece>> = its.into_iter().map(|it| sh.take(it, &mut edge)).collect();
    SEEN.with(|m| m.borrow_mut().clear());
    plot_of(ps, kind, edge)
}
/// Pv.thick: the first (thickness ..) near a board's start, in micrometres (1.6 mm when there is none)
pub fn thick(b: &[u8]) -> u32 {
    let t = sexpr::decode(&b[..b.len().min(8192)]);
    let Some(seg) = t.split('(').find(|s| s.starts_with("thickness ")) else { return 1600 };
    let f = format!("({})", seg.split(')').next().unwrap_or(""));
    um(sexpr::parse(f.as_bytes()).f(0))
}

/// What reading a file came to: `Pv.child`'s exits.
pub enum Read { Plot(Box<Plot>, String), Half, Fail(String) }
pub const UNREAD: &str = "@bp.file-unread";
/// Pv.child.empty: an empty or unreadable live file: being saved if it is there, else gone.
pub fn empty(path: &str) -> Read {
    if std::fs::metadata(path).is_ok() { Read::Half } else { Read::Fail(format!("@bp.file-missing:{}", path.rsplit('/').next().unwrap_or(path))) }
}
/// a file's bytes as its plot; `fixed` (a version in a commit): what does not parse is unread rather than half written
pub fn of_bytes(kind: u32, b: &[u8], fixed: bool) -> Read {
    let Some(fs) = sexpr::forms(b) else { return if fixed { Read::Fail(UNREAD.into()) } else { Read::Half } };
    let (mut p, t) = if kind == 0 { board_plot(&fs) } else { items_plot(sch_items(&fs), kind) };
    p.thick = if kind == 0 { thick(b) } else { 0 };
    Read::Plot(Box::new(p), t)
}
/// Pv.write: a file written whole, then put in place (a reader never sees half of it)
pub fn write_infos(path: &str, text: &str) {
    if path.is_empty() { return }
    if let Some((d, _)) = path.rsplit_once('/') { let _ = std::fs::create_dir_all(d); }
    let new = format!("{path}.new");
    if std::fs::write(&new, text).is_ok() { let _ = crate::plat::fs::rename(&new, path); }
}
/// Line.plot: the lines child prints (a byte goes as the char of that number), for comparing with it
pub fn lines(p: &Plot) -> Vec<u8> {
    let mut o = format!("b{} {} {} {} {} {} {} {} {}\n", p.b.x0, p.b.y0, p.b.x1, p.b.y1, p.thick, p.edge.x0, p.edge.y0, p.edge.x1, p.edge.y1).into_bytes();
    for c in &p.cs {
        o.extend(format!("c{} {} {} {} {} {} {} {} ", c.sig, c.layer, c.color, c.alpha, c.mode, c.width, c.d.len(), c.info.chars().count()).into_bytes());
        for b in &c.d { if *b < 128 { o.push(*b) } else { o.push(0xc0 | (b >> 6)); o.push(0x80 | (b & 63)) } }
        o.extend(c.info.as_bytes()); o.push(b'\n');
    }
    if let Some(s) = &p.solid { // Line.solid
        o.extend(format!("m{} {} {} {} {} {} {} {} ", s.n, s.b[0], s.b[1], s.b[2], s.b[3], s.b[4], s.b[5], s.d.len()).into_bytes());
        for b in &s.d { if *b < 128 { o.push(*b) } else { o.push(0xc0 | (b >> 6)); o.push(0x80 | (b & 63)) } }
        o.push(b'\n');
    }
    o
}
/// Infos.find (`--plot-info`): piece idx's info of the chunk with this sig, from an info file's text; the last line of a sig wins, as the scan does
pub fn info_find(t: &str, sig: u32, idx: usize) -> String {
    let (mut rest, mut hit): (&str, Option<&str>) = (t, None);
    while !rest.is_empty() {
        let Some((s, r)) = rest.split_once(' ') else { break };
        let Some((n, r)) = r.split_once(' ') else { break };
        let n: usize = n.parse().unwrap_or(0);
        let end = r.char_indices().nth(n).map(|(i, _)| i).unwrap_or(r.len());
        if n > 0 && s.parse::<u32>().ok() == Some(sig) { hit = Some(&r[..end]) }
        rest = r[end..].strip_prefix('\n').unwrap_or(&r[end..]);
    }
    hit.unwrap_or("").split('\0').nth(idx).unwrap_or("").to_string()
}

/// Every piece an info file holds with the field `name=value` (a `;`-separated entry of its text), as (chunk sig, piece index): what a cross-probe lights.
/// The last line of a sig wins, as `info_find` does.
pub fn info_scan(t: &str, name: &str, value: &str) -> Vec<(u32, usize)> {
    let want = format!("{name}={value}");
    let mut by_sig: std::collections::HashMap<u32, &str> = std::collections::HashMap::new();
    let mut rest = t;
    while !rest.is_empty() {
        let Some((s, r)) = rest.split_once(' ') else { break };
        let Some((n, r)) = r.split_once(' ') else { break };
        let n: usize = n.parse().unwrap_or(0);
        let end = r.char_indices().nth(n).map(|(i, _)| i).unwrap_or(r.len());
        if n > 0 { if let Ok(sg) = s.parse::<u32>() { by_sig.insert(sg, &r[..end]); } }
        rest = r[end..].strip_prefix('\n').unwrap_or(&r[end..]);
    }
    let mut out = vec![];
    for (sig, text) in by_sig { for (i, piece) in text.split('\0').enumerate() { if piece.split(';').any(|f| f == want) { out.push((sig, i)) } } }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn numbers() {
        assert_eq!(um(1.6), 1600); assert_eq!(um(-1.6), 0u32.wrapping_sub(1600)); assert_eq!(um(0.0), 0); assert_eq!(um(0.0004), 0); assert_eq!(zig(0), 0); assert_eq!(zig(1), 2); assert_eq!(zig(u32::MAX), 1);
        let mut e = Enc { d: vec![], px: 0, py: 0, h: board::FNV, tab: HashMap::new(), info: String::new() };
        e.num(0); e.num(127); e.num(128); e.num(u32::MAX); assert_eq!(e.d, [0, 127, 128, 1, 255, 255, 255, 255, 15]);
    }
    #[test] fn deal_is_board_deal() {
        // [1,2,3,4,5]: a=[], b=[1]; a=[1], b=[2]; a=[2], b=[3,1]; a=[3,1], b=[4,2]; a=[4,2], b=[5,3,1]
        assert_eq!(deal(vec![1, 2, 3, 4, 5]), (vec![4, 2], vec![5, 3, 1]));
        let mut o = form_order(1000); o.sort(); assert_eq!(o, (0..1000).collect::<Vec<u32>>());
    }
    #[test] fn thickness() { assert_eq!(thick(b"(kicad_pcb (general (thickness 1.2) (x 1)))"), 1200); assert_eq!(thick(b"(kicad_pcb)"), 1600); }
    #[test] fn info_lookup() {
        let t = "7 6 a\0日\0x\0\n9 0 \n11 2 b\0\n7 2 c\0\n";
        assert_eq!(info_find(t, 7, 0), "c"); assert_eq!(info_find(t, 11, 0), "b"); assert_eq!(info_find(t, 11, 5), ""); assert_eq!(info_find(t, 9, 0), ""); assert_eq!(info_find(t, 1, 0), "");
    }
    // law plot_hidden_not_sent: an item on a layer the viewers hide is never sent
    #[test] fn plot_hidden_not_sent() {
        let (b, _) = board_plot(&[&b"(gr_line (start 0 0) (end 1 1) (layer \"F.CrtYd\") (width 0.1) (uuid \"a\"))"[..], &b"(gr_line (start 0 0) (end 1 1) (layer \"F.Cu\") (width 0.1) (uuid \"b\"))"[..]]);
        assert_eq!(b.cs.len(), 1); assert_eq!(b.cs[0].layer, 12);
        for kind in [0u32, 1, 8, 9] { for layer in 0..32u32 {
            let it = Item { key: "k".into(), sig: 0, layer, prim: Prim::Seg { a: Pt { x: 0.0, y: 0.0 }, b: Pt { x: 1.0, y: 1.0 }, w: 0.1 }, info: "i".into() };
            let mut e = Bx::empty();
            assert_eq!(Shown::new(kind).take(it, &mut e).is_none(), look(kind, layer).1 == 0, "{kind} {layer}");
        } }
    }
    // law plot_courtyard_hidden
    #[test] fn plot_courtyard_hidden() { assert_eq!(look(0, 17).1, 0) }
    // law plot_tint_own_colour: a coloured box goes in its own colour
    #[test] fn plot_tint_own_colour() { assert_eq!(chunk_look(1, rank_of(1, 0, board::tint_code(12386304, 16), 0)), (12386304, 16)) }
    // law tint_code_again: a tint once packed packs again the same
    #[test] fn tint_code_again() {
        assert_eq!(board::tint_code(board::tint_color(1523716), board::tint_alpha(1523716)), 1523716);
        for c in [0u32, 0x123456, 0xffffff, 0x800080] { for a in [0u32, 1, 100, 255, 300] {
            let k = board::tint_code(c, a); assert!(k != 0); assert_eq!(board::tint_code(board::tint_color(k), board::tint_alpha(k)), k);
        } }
    }
    // law plot_numbers_exact: numbers survive the wire exactly, small and large
    #[test] fn plot_numbers_exact() {
        let ns = [0u32, 1, 31, 32, 1024, 123456789, 4294967295];
        let mut e = Enc { d: vec![], px: 0, py: 0, h: board::FNV, tab: HashMap::new(), info: String::new() };
        for n in ns { e.num(n) }
        let (mut out, mut acc, mut sh) = (vec![], 0u32, 0u32);
        for b in &e.d { acc |= ((*b & 127) as u32) << sh; if *b >= 128 { sh += 7 } else { out.push(acc); acc = 0; sh = 0 } }
        assert_eq!(out, ns);
    }
    // law look_plain_board, look_plain_sch
    #[test] fn look_plain_board() { for l in 0..40 { assert_eq!(look(0, l), look_plain(0, l)) } }
    #[test] fn look_plain_sch() { for l in 0..40 { assert_eq!(look(1, l), look_plain(1, l)) } }
    // law look_marks_distinct
    #[test] fn look_marks_distinct() { let (a, b, c) = (look(8, 26).0, look(8, 27).0, look(8, 28).0); assert!(a != b && b != c && a != c) }
    #[test] fn info_scan_finds_a_field_across_pieces() {
        let (a, b) = ("pad 1;net=GND;ref=R1\0pad 2;net=VCC;ref=R1", "track;net=GND"); let t = &format!("7 {} {a}\n9 {} {b}", a.chars().count(), b.chars().count());
        let mut a = info_scan(t, "net", "GND"); a.sort(); assert_eq!(a, vec![(7, 0), (9, 0)]);
        assert_eq!(info_scan(t, "ref", "R1").len(), 2); assert!(info_scan(t, "net", "G").is_empty(), "whole values only");
    }
}

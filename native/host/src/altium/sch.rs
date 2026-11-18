//! .SchDoc: an Altium schematic sheet as the items a KiCad schematic draws (kicad/schematic.rs): the same layers, keys of the
//! same shape ("a/<unique id>", children "/pin/<n>", "/f/<field>", "/g<i>"), the same info strings (symbol, pin, label...) and
//! connected groups tagged alike (`g=`, and `net=` with the net's name), so the viewers, colours, hit tests and cross-probes
//! work on an Altium design unchanged.
//!
//! The `FileHeader` stream holds the sheet as text records (`RECORD=n`), indexed from 0 after the header (index 0 is the
//! sheet record); a child names its owner by `OWNERINDEX`. A component's children (pins, lines, labels, designator,
//! parameters) are stored placed: their coordinates are already on the sheet, rotated and mirrored, so they are drawn as
//! they are. A multi-part component draws only the children of its current part (`OWNERPARTID`) and display mode
//! (`OWNERPARTDISPLAYMODE`). Coordinates are in units of 10 mil, with `_FRAC` parts in 1/100000 of a unit, Y up from the
//! sheet's bottom left; they become mm with Y down from the top left. The `Additional` stream adds harnesses and blankets.
use super::{sch_records, Doc, Props, SchRec};
use crate::files::kicad::board::{self, kv, Info, Item, Prim, Pt};
use crate::files::kicad::{fl, strokefont};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// The schematic layers (kicad/schematic.rs).
const PAGE: u32 = 0; const SHEET_FILL: u32 = 1; const BODY_FILL: u32 = 2; const SHEET: u32 = 3; const GRAPHIC: u32 = 4; const NOTE: u32 = 5; const BODY: u32 = 6;
const PIN: u32 = 7; const PIN_NAME: u32 = 8; const PIN_NUM: u32 = 9; const FIELD: u32 = 10; const FIELD_OTHER: u32 = 11; const BUS: u32 = 12; const BUS_ENTRY: u32 = 13;
const WIRE: u32 = 14; const NC: u32 = 15; const JUNCTION: u32 = 16; const LABEL: u32 = 17; const GLOBAL: u32 = 18; const HIER: u32 = 19; const SHEET_NAME: u32 = 20;
const SHEET_FILE: u32 = 21; const SHEET_PIN: u32 = 22;

/// mm per Altium schematic unit (10 mil)
const U: f32 = 0.254;
const THIN: f32 = 0.1524;

/// A component, for the BOM and the names list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Part { pub refdes: String, pub value: String, pub footprint: String, pub lib_ref: String, pub description: String, pub fields: Vec<(String, String)> }

/// The sheet's records (index 0 is the sheet record) and the `Additional` stream's; None when the bytes are no schematic.
struct Recs { main: Vec<Props>, more: Vec<Props> }
fn text_records(b: &[u8]) -> Vec<Props> {
    // the first record is the header; a binary record keeps its place as an empty one, so indices stay right
    sch_records(b).into_iter().skip(1).map(|r| match r { SchRec::Text(p) => p, SchRec::Bin(_) => Props::default() }).collect()
}
fn load(bytes: &[u8]) -> Option<Recs> {
    let mut d = Doc::open(bytes.to_vec())?;
    let fh = d.stream("FileHeader")?;
    let head = sch_records(&fh);
    match head.first() { Some(SchRec::Text(p)) if p.get("HEADER").contains("Schematic") => {} _ => return None }
    let main = text_records(&fh);
    let more = d.stream("Additional").map(|b| text_records(&b)).unwrap_or_default();
    Some(Recs { main, more })
}

fn rec(p: &Props) -> i64 { if p.has("RECORD") { p.int("RECORD") } else { -1 } }
fn owner(p: &Props) -> i64 { if p.has("OWNERINDEX") { p.int("OWNERINDEX") } else { -1 } }
/// A length in units: KEY plus KEY_FRAC / 100000.
fn val(p: &Props, k: &str) -> f32 { p.int(k) as f32 + p.int(&format!("{k}_FRAC")) as f32 / 100000.0 }

// Text --------------------------------------------------------------------------------------------------------------------

/// Altium's text as KiCad's markup: braces kept literal, and a character followed by a backslash drawn under a bar
/// ("R\E\S\E\T\" is RESET with a bar over it).
fn markup(s: &str) -> String {
    // "~1" is a line break and "~2" a "|" (the record's separator), as text frames and labels store them; a tab is spaces
    let s = s.replace("~1", "\n").replace("~2", "|").replace('\t', "    ");
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 8);
    let (mut i, mut run) = (0, String::new());
    let esc = |c: char, o: &mut String| match c { '{' => o.push_str("{brl}"), '}' => o.push_str("{brr}"), _ => o.push(c) };
    let flush = |run: &mut String, o: &mut String| if !run.is_empty() { o.push_str("~{"); o.push_str(run); o.push('}'); run.clear() };
    while i < cs.len() {
        let c = cs[i];
        if c == '\\' { i += 1; continue }
        if cs.get(i + 1) == Some(&'\\') { esc(c, &mut run); i += 2; continue }
        flush(&mut run, &mut out);
        esc(c, &mut out);
        i += 1;
    }
    flush(&mut run, &mut out);
    out
}

#[derive(Clone, Copy)]
struct Font { h: f32, bold: bool, italic: bool, face: Face }
/// A font's family, for its characters' widths
#[derive(Clone, Copy, PartialEq, Debug)]
enum Face { Serif, Sans, Mono }
impl Face {
    fn of(name: &str) -> Face {
        let n = name.to_ascii_lowercase();
        if n.contains("courier") || n.contains("mono") || n.contains("consol") { Face::Mono }
        else if n.is_empty() || n.contains("times") || n.contains("serif") && !n.contains("sans") || n.contains("georgia") || n.contains("garamond") { Face::Serif }
        else { Face::Sans }
    }
    /// the em in lines (Altium's text box is the line: ascent and descent)
    fn em(self) -> f32 { match self { Face::Serif => 0.903, Face::Sans => 0.87, Face::Mono => 0.883 } }
}
/// Advance widths in thousandths of the em of printable ASCII (space to '~'): Times New Roman's and Arial's
const TIMES: [u16; 95] = [250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278, 500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444, 921, 722, 667, 667, 722, 611, 556, 722, 722, 333, 389, 722, 611, 889, 722, 722, 556, 722, 667, 556, 611, 722, 722, 944, 722, 722, 611, 333, 278, 333, 469, 500, 333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500, 278, 778, 500, 500, 500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541];
const ARIAL: [u16; 95] = [278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584];
/// The widest line of `s` (Altium's text: "~1" breaks a line, a backslash marks a bar) in ems
fn em_width(s: &str, face: Face) -> f32 {
    s.replace("~1", "\n").replace("~2", "|").split('\n').map(|l| l.chars().filter(|&c| c != '\\').map(|c| {
        let i = c as u32;
        if face == Face::Mono { 600 } else if (32..127).contains(&i) { (if face == Face::Serif { TIMES } else { ARIAL })[(i - 32) as usize] as u32 } else { 556 }
    }).sum::<u32>()).max().unwrap_or(0) as f32 / 1000.0
}
impl Font {
    /// A font of `size` units of line spacing: its capital height (Times' is about 0.6 of the spacing) in mm
    fn of(size: f32, bold: bool, italic: bool, face: Face) -> Font { Font { h: (if size > 0.0 { size } else { 10.0 }) * U * 0.6, bold, italic, face } }
    /// the character width: Altium's (Times) lines are about as long as the stroke font's at 0.95 of the capital height
    fn w(&self) -> f32 { self.h * 0.95 }
    /// the line's height (Altium's text box), in mm
    fn line(&self) -> f32 { self.h / 0.6 }
    fn pen(&self) -> f32 { self.h / if self.bold { 5.0 } else { 8.0 } }
}
/// Text at `at`: hj 0 left, 1 centre, 2 right; vj 0 top, 1 centre, 2 bottom; `up` reads bottom to top.
#[allow(clippy::too_many_arguments)]
///
/// Altium anchors a text's box (its line: the descent below the baseline, the ascent above the capitals), the stroke font
/// its baseline and capitals: a box's bottom sits a descent (0.195 of the line) below the baseline, its top 0.207 of the line
/// above the capitals.
fn text_prims(s: &str, at: Pt, up: bool, hj: u32, vj: u32, f: &Font) -> Vec<Prim> {
    let shift = match vj { 2 => -0.195, 0 => 0.207, _ => 0.0 } * f.line();
    let at = if up { p(at.x + shift, at.y) } else { p(at.x, at.y + shift) };
    let m = markup(s);
    // the characters as wide as Altium's font draws the line
    let (want, have) = (em_width(s, f.face) * f.face.em() * f.line(), strokefont::width(&m, f.w()));
    let w = if want > 0.0 && have > 0.0 { f.w() * (want / have).clamp(0.7, 1.5) } else { f.w() };
    strokefont::text_full(&m, at.x, at.y, f.h, w, f.pen(), if up { fl::PI / 2.0 } else { 0.0 }, hj, vj, false, f.italic)
}
/// Altium's placement (`ORIENTATION` quarter turns anticlockwise, `JUSTIFICATION` 0..8 from bottom left to top right) as
/// readable text: a half turn flips the anchor's side instead of drawing the text upside down.
fn placed(orient: i64, just: i64) -> (bool, u32, u32) {
    let j = just.clamp(0, 8) as u32;
    let (hj, vj) = (j % 3, 2 - j / 3);
    let o = orient.rem_euclid(4);
    if o >= 2 { (o == 3, 2 - hj, 2 - vj) } else { (o == 1, hj, vj) }
}

// Drawing helpers ---------------------------------------------------------------------------------------------------------

struct Out { items: Vec<Item>, keys: HashSet<String> }
#[cfg(test)]
thread_local! {
    /// Test aid: every text drawn (as written, its anchor, its layer), for the corpus inventory
    static DRAWN: std::cell::RefCell<Vec<(String, Pt, u32, [f32; 4])>> = const { std::cell::RefCell::new(Vec::new()) };
}
#[cfg(test)]
fn drawn(s: &str, at: Pt, layer: u32, ps: &[Prim]) {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    let mut g = |q: &Pt| { b[0] = b[0].min(q.x); b[1] = b[1].min(q.y); b[2] = b[2].max(q.x); b[3] = b[3].max(q.y) };
    for pr in ps { match pr { Prim::Seg { a, b, .. } => { g(a); g(b) } Prim::Line { ps, .. } | Prim::Poly { ps, .. } | Prim::Tint { ps, .. } => ps.iter().for_each(&mut g), Prim::Circ { c, .. } => g(c) } }
    DRAWN.with(|d| d.borrow_mut().push((s.to_string(), at, layer, b)))
}
#[cfg(not(test))]
fn drawn(_: &str, _: Pt, _: u32, _: &[Prim]) {}
impl Out {
    fn push(&mut self, key: String, sig: u32, layer: u32, prim: Prim, info: &Info) {
        let key = if self.keys.contains(&key) { let mut n = 2; while self.keys.contains(&format!("{key}#{n}")) { n += 1 } format!("{key}#{n}") } else { key };
        self.keys.insert(key.clone());
        self.items.push(Item { key, sig, layer, prim, info: info.clone() });
    }
    fn put(&mut self, ps: Vec<Prim>, key: &str, sig: u32, layer: u32, info: &Info) {
        for (i, prim) in ps.into_iter().enumerate() { self.push(format!("{key}/{i}"), sig, layer, prim, info) }
    }
    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, s: &str, at: Pt, up: bool, hj: u32, vj: u32, f: &Font, key: &str, sig: u32, layer: u32, info: &Info) {
        if s.trim().is_empty() { return }
        let ps = text_prims(s, at, up, hj, vj, f);
        drawn(s, at, layer, &ps);
        self.put(ps, key, sig, layer, info)
    }
    /// An outline and, by `fill` (0 none, 1 the outline's layer, 2 `lf`), its fill beneath it.
    #[allow(clippy::too_many_arguments)]
    fn shape(&mut self, ps: Vec<Pt>, closed: bool, w: f32, fill: u32, key: &str, sig: u32, ll: u32, lf: u32, info: &Info) {
        if ps.len() < 2 { return }
        if fill > 0 && closed && ps.len() > 2 { self.push(format!("{key}/f"), sig, if fill == 1 { ll } else { lf }, Prim::Poly { ps: ps.clone(), w: 0.0, fill: true }, info) }
        self.push(key.to_string(), sig, ll, if closed { Prim::Poly { ps, w, fill: false } } else { Prim::Line { ps, w } }, info)
    }
}
fn p(x: f32, y: f32) -> Pt { Pt { x, y } }
fn add(a: Pt, b: Pt) -> Pt { p(a.x + b.x, a.y + b.y) }
fn sc(a: Pt, k: f32) -> Pt { p(a.x * k, a.y * k) }
fn sig_of(pr: &Props) -> u32 {
    let mut kv: Vec<(&String, &String)> = pr.0.iter().collect();
    kv.sort();
    let mut h = board::FNV;
    for (k, v) in kv { h = board::hash(k.as_bytes(), h); h = board::hash(b"=", h); h = board::hash(v.as_bytes(), h); h = board::hash(b"|", h) }
    h
}
/// LINEWIDTH: smallest, small, medium, large
fn line_w(pr: &Props) -> f32 { match pr.int("LINEWIDTH") { i64::MIN..=0 => THIN, 1 => 0.254, 2 => 0.508, _ => 1.016 } }
fn wire_w(pr: &Props) -> f32 { match pr.int("LINEWIDTH") { i64::MIN..=1 => THIN, 2 => 0.3048, _ => 0.6 } }

// The sheet ---------------------------------------------------------------------------------------------------------------

/// SHEETSTYLE's drawing areas, in units, landscape
const STYLES: [(f32, f32); 18] = [(1150.0, 760.0), (1550.0, 1110.0), (2230.0, 1570.0), (3150.0, 2230.0), (4460.0, 3150.0), (950.0, 750.0), (1500.0, 950.0),
    (2000.0, 1500.0), (3200.0, 2000.0), (4200.0, 3200.0), (1100.0, 850.0), (1400.0, 850.0), (1700.0, 1100.0), (990.0, 790.0), (1540.0, 990.0), (2060.0, 1560.0),
    (3260.0, 2060.0), (4280.0, 3280.0)];
const STYLE_NAMES: [&str; 18] = ["A4", "A3", "A2", "A1", "A0", "A", "B", "C", "D", "E", "Letter", "Legal", "Tabloid", "OrCAD A", "OrCAD B", "OrCAD C", "OrCAD D", "OrCAD E"];
struct Sheet { w: f32, h: f32, fonts: Vec<Font>, sys: Font }
impl Sheet {
    fn of(pr: &Props) -> Sheet {
        let (mut w, mut h) = if pr.bool("USECUSTOMSHEET") { (val(pr, "CUSTOMX"), val(pr, "CUSTOMY")) } else { STYLES[pr.int("SHEETSTYLE").clamp(0, 17) as usize] };
        if !pr.bool("USECUSTOMSHEET") && pr.int("WORKSPACEORIENTATION") == 1 { std::mem::swap(&mut w, &mut h) }
        if w <= 0.0 || h <= 0.0 { (w, h) = STYLES[0] }
        let n = pr.int("FONTIDCOUNT").clamp(0, 1000);
        let fonts: Vec<Font> = (1..=n).map(|i| Font::of(pr.num(&format!("SIZE{i}")) as f32, pr.bool(&format!("BOLD{i}")), pr.bool(&format!("ITALIC{i}")), Face::of(pr.get(&format!("FONTNAME{i}"))))).collect();
        let sys = fonts.get((pr.int("SYSTEMFONT").max(1) - 1) as usize).copied().unwrap_or(Font::of(10.0, false, false, Face::Serif));
        Sheet { w, h, fonts, sys }
    }
    fn font(&self, id: i64) -> Font { if id >= 1 { self.fonts.get(id as usize - 1).copied().unwrap_or(self.sys) } else { self.sys } }
    /// a point in units (Y up) on the page in mm (Y down)
    fn pt(&self, x: f32, y: f32) -> Pt { p(x * U, (self.h - y) * U) }
    fn at(&self, pr: &Props, k: &str) -> Pt { self.pt(val(pr, &format!("{k}.X")), val(pr, &format!("{k}.Y"))) }
    fn loc(&self, pr: &Props) -> Pt { self.at(pr, "LOCATION") }
    fn pts(&self, pr: &Props) -> Vec<Pt> {
        let n = pr.int("LOCATIONCOUNT").clamp(0, 100000);
        let mut v: Vec<Pt> = (1..=n).map(|i| self.pt(val(pr, &format!("X{i}")), val(pr, &format!("Y{i}")))).collect();
        let e = pr.int("EXTRALOCATIONCOUNT").clamp(0, 100000);
        v.extend((n + 1..=n + e).map(|i| self.pt(val(pr, &format!("EX{i}")), val(pr, &format!("EY{i}")))));
        v
    }
}
fn sheet_of(rs: &[Props]) -> Sheet { rs.iter().find(|r| rec(r) == 31).map(Sheet::of).unwrap_or_else(|| Sheet::of(&Props::default())) }

/// A record's key: its UNIQUEID, else its kind and place ("a/r29@410,250"), which an insertion elsewhere leaves alone.
fn key_of(r: &Props) -> String {
    let u = r.get("UNIQUEID");
    if !u.is_empty() { return format!("a/{u}") }
    let (x, y) = if r.has("LOCATION.X") || r.has("LOCATION.Y") { ("LOCATION.X", "LOCATION.Y") } else { ("X1", "Y1") };
    format!("a/r{}@{},{}", rec(r), val(r, x), val(r, y))
}

/// The key of a record of the `Additional` stream ("a/x/<unique id>")
fn more_key(r: &Props) -> String { format!("a/x{}", &key_of(r)[1..]) }

// Records' relations ------------------------------------------------------------------------------------------------------

/// What a sheet's special strings ("=Title", "=DocumentName"...) resolve against beyond the sheet itself: the project's
/// parameters (its `.PrjPcb`'s `[ParameterN]` sections) and the file (its path, its modification date). A sheet read
/// without it shows the strings these would have answered as written, as Altium shows an unknown one.
#[derive(Clone, Debug, Default)]
pub struct Ctx {
    /// the project's parameters (name, value), in the project's order
    pub project: Vec<(String, String)>,
    /// the project file's name ("Board.PrjPcb"), "" for none
    pub project_file: String,
    /// the sheet's path, "" when unknown
    pub path: String,
    /// the file's modification time, seconds since 1970 (0 when unknown)
    pub modified: u64,
    /// the names of the sheet symbols that place this sheet when there are several (its channels: "CH1", "CH2")
    pub channels: Vec<String>,
}
impl Ctx {
    /// The context of the sheet at `path`: the project beside it that lists it, the file's time.
    pub fn of_path(path: &str) -> Ctx {
        let modified = std::fs::metadata(path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
        let (project, project_file, channels) = match super::prj::find_for(path) {
            Some(prj) => {
                let text = std::fs::read(&prj).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
                let chs = super::prj::placements(&prj, path);
                (super::prj::parameters(&text), prj.rsplit(['/', '\\']).next().unwrap_or("").to_string(), if chs.len() > 1 { chs } else { vec![] })
            }
            None => (vec![], String::new(), vec![]),
        };
        Ctx { project, project_file, path: path.to_string(), modified, channels }
    }
}

/// Altium's date and time as its special strings show them (`=CurrentDate` 11/18/2026, `=CurrentTime` 9:05:03 AM), for a
/// time in seconds since 1970 in the machine's zone.
fn altium_date(t: u64) -> String {
    let l = t as i64 + crate::plat::time::local_offset(t);
    let (y, m, d) = crate::google::cal::civil(l.div_euclid(86400));
    format!("{m}/{d}/{y}")
}
fn altium_time(t: u64) -> String {
    let s = (t as i64 + crate::plat::time::local_offset(t)).rem_euclid(86400);
    let (h, mi, se) = (s / 3600, s / 60 % 60, s % 60);
    format!("{}:{mi:02}:{se:02} {}", if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { "AM" } else { "PM" })
}

struct Doc2<'a> {
    rs: &'a [Props],
    sh: Sheet,
    keys: Vec<String>,
    kids: HashMap<i64, Vec<usize>>,
    /// sheet parameters by lower-cased name
    sheet_params: HashMap<String, String>,
    ctx: Ctx,
}
impl<'a> Doc2<'a> {
    fn new(rs: &'a [Props]) -> Doc2<'a> { Doc2::with(rs, Ctx::default()) }
    fn with(rs: &'a [Props], ctx: Ctx) -> Doc2<'a> {
        let mut seen = HashSet::new();
        let keys = rs.iter().map(|r| {
            let k = key_of(r);
            if seen.insert(k.clone()) { k } else { let mut n = 2; while !seen.insert(format!("{k}#{n}")) { n += 1 } format!("{k}#{n}") }
        }).collect();
        let mut kids: HashMap<i64, Vec<usize>> = HashMap::new();
        for (i, r) in rs.iter().enumerate() { let o = owner(r); if o >= 0 && (o as usize) < rs.len() { kids.entry(o).or_default().push(i) } }
        let mut sheet_params = HashMap::new();
        for r in rs { if rec(r) == 41 && owner(r) < 0 { sheet_params.insert(r.get("NAME").to_ascii_lowercase(), r.get("TEXT").to_string()); } }
        Doc2 { rs, sh: sheet_of(rs), keys, kids, sheet_params, ctx }
    }
    fn kids(&self, i: usize) -> &[usize] { self.kids.get(&(i as i64)).map(|v| v.as_slice()).unwrap_or(&[]) }
    fn owner_rec(&self, r: &Props) -> i64 { let o = owner(r); if o >= 0 && (o as usize) < self.rs.len() { rec(&self.rs[o as usize]) } else { -1 } }
    /// A parameter of record `i` by name (any case).
    fn param(&self, i: usize, name: &str) -> Option<&str> {
        self.kids(i).iter().map(|&k| &self.rs[k]).find(|r| rec(r) == 41 && r.get("NAME").eq_ignore_ascii_case(name)).map(|r| r.get("TEXT"))
    }
    /// A text as Altium shows it: "=Name" (a special string, any case) stands for the owner's parameter Name, else what
    /// Altium works out itself (the date, the file's name and path...), else the sheet's parameter, else the project's; one
    /// that names nothing, or a parameter with no value, shows as written. "*" (an unset parameter) shows as "*".
    fn resolve(&self, s: &str, own: i64) -> String { self.resolve_n(s, own, 0) }
    fn resolve_n(&self, s: &str, own: i64, depth: u32) -> String {
        let Some(n) = s.strip_prefix('=') else { return s.to_string() };
        let n = n.trim();
        if n.is_empty() || depth > 8 { return s.to_string() }
        let o = if own >= 0 && (own as usize) < self.rs.len() { self.param(own as usize, n).map(|x| x.to_string()) } else { None };
        let v = o.or_else(|| self.builtin(n)).or_else(|| self.sheet_params.get(&n.to_ascii_lowercase()).cloned())
            .or_else(|| self.ctx.project.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|(_, v)| v.clone()));
        match v {
            Some(v) if v.is_empty() => s.to_string(),
            Some(v) if v.starts_with('=') && !v.eq_ignore_ascii_case(s) => self.resolve_n(&v, own, depth + 1),
            Some(v) => v,
            None => s.to_string(),
        }
    }
    /// The special strings Altium answers itself (None: not one, or not known here)
    fn builtin(&self, n: &str) -> Option<String> {
        let path = self.ctx.path.as_str();
        let now = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        Some(match n.to_ascii_lowercase().as_str() {
            "currentdate" => altium_date(now()),
            "currenttime" => altium_time(now()),
            "documentname" if !path.is_empty() => path.rsplit(['/', '\\']).next().unwrap_or(path).to_string(),
            "documentfullpathandname" if !path.is_empty() => path.to_string(),
            "modifieddate" if self.ctx.modified > 0 => altium_date(self.ctx.modified),
            "projectname" if !self.ctx.project_file.is_empty() => self.ctx.project_file.clone(),
            // a managed project's item strings, which a project on disk cannot answer
            "item" | "itemrevision" | "itemandrevision" | "itemrevisionbase" => format!("{n} not interpreted"),
            _ => return None,
        })
    }
    /// A text's links to design objects (`@{"Id":"<document id>\<object id>","ObjectType":1}`, as a note or text frame
    /// keeps them) as the object's name when it is on this sheet (a component's designator, a label's or port's text), else "@?".
    fn mentions(&self, s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(i) = rest.find("@{") {
            let Some(j) = rest[i..].find('}') else { break };
            out.push_str(&rest[..i]);
            let tok = &rest[i..i + j + 1];
            let id = tok.split("\"Id\":\"").nth(1).and_then(|t| t.split('"').next()).unwrap_or("");
            let obj = id.rsplit('\\').next().unwrap_or("");
            let at = if obj.is_empty() { None } else { self.rs.iter().position(|r| r.get("UNIQUEID") == obj) };
            let name = at.map(|k| match rec(&self.rs[k]) { 1 => self.designator(k), 18 => self.rs[k].get("NAME").to_string(), _ => self.rs[k].get("TEXT").to_string() }).filter(|n| !n.is_empty());
            out.push_str(&name.unwrap_or_else(|| "@?".into()));
            rest = &rest[i + j + 1..];
        }
        out.push_str(rest);
        out
    }
    /// Whether a component's child is drawn: its part is the component's current one and its display mode the component's.
    fn shown_in(&self, comp: &Props, r: &Props) -> bool {
        let part = r.int("OWNERPARTID");
        (part <= 0 || part == comp.int("CURRENTPARTID")) && r.int("OWNERPARTDISPLAYMODE") == comp.int("DISPLAYMODE")
    }
    /// The designator as written ("U3")
    fn raw_designator(&self, i: usize) -> String { self.kids(i).iter().map(|&k| &self.rs[k]).find(|r| rec(r) == 34).map(|r| r.get("TEXT").to_string()).unwrap_or_default() }
    /// The designator as drawn: a part of a multi-part component adds its letter ("U3B")
    fn designator(&self, i: usize) -> String {
        let c = &self.rs[i];
        let d = self.raw_designator(i);
        let parts = c.int("PARTCOUNT") - 1;
        let cur = c.int("CURRENTPARTID");
        if parts > 1 && (1..=26).contains(&cur) { format!("{d}{}", (b'A' + (cur - 1) as u8) as char) } else { d }
    }
    /// The component's value (its Comment) as data: Altium's unset "*" is no value
    fn value(&self, i: usize) -> String { self.param(i, "Comment").map(|v| self.resolve(v, i as i64)).filter(|v| v != "*").unwrap_or_default() }
    fn footprint(&self, i: usize) -> String {
        let mut first = String::new();
        for &l in self.kids(i) {
            if rec(&self.rs[l]) != 44 { continue }
            for &m in self.kids(l) {
                let r = &self.rs[m];
                if rec(r) != 45 || !r.get("MODELTYPE").eq_ignore_ascii_case("PCBLIB") { continue }
                if r.bool("ISCURRENT") { return r.get("MODELNAME").to_string() }
                if first.is_empty() { first = r.get("MODELNAME").to_string() }
            }
        }
        first
    }
}

// Connectivity ------------------------------------------------------------------------------------------------------------

/// What connects: wires (their segments and vertices), points (pins' ends, labels, ports, junctions, entries) and names.
#[derive(Default)]
struct Conn { ids: Vec<String>, at: Vec<(Pt, usize)>, segs: Vec<(Pt, Pt, usize)>, names: Vec<(usize, u8, String)>, pins: Vec<(usize, String, String)> }
impl Conn {
    fn node(&mut self, id: &str) -> usize { self.ids.push(id.to_string()); self.ids.len() - 1 }
    fn wire(&mut self, id: &str, ps: &[Pt]) { let n = self.node(id); for q in ps { self.at.push((*q, n)) } for w in ps.windows(2) { self.segs.push((w[0], w[1], n)) } }
    fn point(&mut self, id: &str, q: Pt) -> usize { let n = self.node(id); self.at.push((q, n)); n }
    /// Each id's (net name, group tag). A group is named by its power port, else its net label, else its port, else
    /// Altium's "Net<designator>_<pin>" of its first pin.
    fn solve(&self) -> HashMap<String, (String, String)> {
        let n = self.ids.len();
        let mut u: Vec<usize> = (0..n).collect();
        fn find(u: &mut [usize], mut a: usize) -> usize { while u[a] != a { u[a] = u[u[a]]; a = u[a] } a }
        fn join(u: &mut [usize], a: usize, b: usize) { let (x, y) = (find(u, a), find(u, b)); if x != y { u[x.max(y)] = x.min(y) } }
        let um = |v: f32| (v * 1000.0).round() as i64;
        let mut by_at: HashMap<(i64, i64), usize> = HashMap::new();
        for &(q, i) in &self.at { match by_at.get(&(um(q.x), um(q.y))) { Some(&j) => join(&mut u, i, j), None => { by_at.insert((um(q.x), um(q.y)), i); } } }
        let mut by_name: HashMap<&str, usize> = HashMap::new();
        for (i, _, name) in &self.names { match by_name.get(name.as_str()) { Some(&j) => join(&mut u, *i, j), None => { by_name.insert(name.as_str(), *i); } } }
        // points along a segment (a T, a label or pin on a wire's middle): segments bucketed in 5 mm cells
        const CELL: f32 = 5.0;
        let cell = |v: f32| (v / CELL).floor() as i64;
        let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (si, (a, b, _)) in self.segs.iter().enumerate() {
            for cx in cell(a.x.min(b.x) - 0.01)..=cell(a.x.max(b.x) + 0.01) {
                for cy in cell(a.y.min(b.y) - 0.01)..=cell(a.y.max(b.y) + 0.01) { grid.entry((cx, cy)).or_default().push(si) }
            }
        }
        for &(q, i) in &self.at {
            let Some(ss) = grid.get(&(cell(q.x), cell(q.y))) else { continue };
            for &si in ss {
                let (a, b, s) = self.segs[si];
                if s != i && along(q, a, b) { join(&mut u, i, s) }
            }
        }
        // names: the best name of each group
        let mut best: HashMap<usize, (u8, String)> = HashMap::new();
        for (i, pr, name) in &self.names {
            let r = find(&mut u, *i);
            let e = best.entry(r).or_insert((u8::MAX, String::new()));
            if *pr < e.0 || (*pr == e.0 && *name < e.1) { *e = (*pr, name.clone()) }
        }
        for (i, rf, pin) in &self.pins {
            let r = find(&mut u, *i);
            let nm = format!("Net{rf}_{pin}");
            let e = best.entry(r).or_insert((u8::MAX, String::new()));
            if e.0 == u8::MAX && (e.1.is_empty() || nm < e.1) { e.1 = nm }
        }
        let mut out = HashMap::with_capacity(n);
        for i in 0..n {
            let r = find(&mut u, i);
            let name = best.get(&r).map(|b| b.1.clone()).unwrap_or_default();
            let tag = board::hash(if name.is_empty() { self.ids[r].as_bytes() } else { name.as_bytes() }, board::FNV).to_string();
            out.insert(self.ids[i].clone(), (name, tag));
        }
        out
    }
}
/// A point on a segment (ends included), within a few micrometres
fn along(q: Pt, a: Pt, b: Pt) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-9 { return false }
    let (qx, qy) = (q.x - a.x, q.y - a.y);
    let f = (qx * dx + qy * dy) / l2;
    let cr = dx * qy - dy * qx;
    (-1e-4..=1.0001).contains(&f) && cr * cr < 1e-5 * l2
}

/// A pin's electrical end and its direction (from the body outwards), on the page.
fn pin_geom(sh: &Sheet, r: &Props) -> (Pt, Pt, Pt, f32) {
    let o = r.int("PINCONGLOMERATE") & 3;
    let len = val(r, "PINLENGTH") * U;
    let b = sh.loc(r);
    let d = match o { 0 => p(1.0, 0.0), 1 => p(0.0, -1.0), 2 => p(-1.0, 0.0), _ => p(0.0, 1.0) };
    (b, add(b, sc(d, len)), d, len)
}
fn pin_hidden(r: &Props) -> bool { r.int("PINCONGLOMERATE") & 4 != 0 }

fn connect(d: &Doc2, more: &[Props]) -> HashMap<String, (String, String)> {
    let (rs, sh) = (d.rs, &d.sh);
    let mut c = Conn::default();
    let mut refs: HashMap<usize, String> = HashMap::new();
    for (i, r) in rs.iter().enumerate() {
        let key = &d.keys[i];
        match rec(r) {
            27 => c.wire(key, &sh.pts(r)),
            2 => {
                let o = owner(r);
                if o < 0 || o as usize >= rs.len() || rec(&rs[o as usize]) != 1 || pin_hidden(r) { continue }
                let comp = &rs[o as usize];
                if !d.shown_in(comp, r) { continue }
                let (_, e, _, _) = pin_geom(sh, r);
                let n = c.point(&format!("{}/pin/{}", d.keys[o as usize], r.get("DESIGNATOR")), e);
                let rf = refs.entry(o as usize).or_insert_with(|| d.raw_designator(o as usize)).clone();
                if !rf.is_empty() && !rf.contains('?') { c.pins.push((n, rf, r.get("DESIGNATOR").to_string())) }
            }
            29 => { c.point(key, sh.loc(r)); }
            25 | 17 => {
                let n = c.point(key, sh.loc(r));
                let t = r.get("TEXT");
                if !t.is_empty() { c.names.push((n, if rec(r) == 17 { 0 } else { 1 }, t.to_string())) }
            }
            18 => {
                let (a, b) = port_ends(sh, r);
                let n = c.point(key, a);
                c.at.push((b, n));
                if !r.get("NAME").is_empty() { c.names.push((n, 2, r.get("NAME").to_string())) }
            }
            16 => {
                let o = owner(r);
                if o >= 0 && (o as usize) < rs.len() && rec(&rs[o as usize]) == 15 {
                    let (q, _) = entry_at(sh, &rs[o as usize], r);
                    c.point(&format!("{}/pin/{}", d.keys[o as usize], r.get("NAME")), q);
                }
            }
            _ => {}
        }
    }
    for r in more { if rec(r) == 218 { c.wire(&more_key(r), &sh.pts(r)) } }
    c.solve()
}
fn net_kv(nets: &HashMap<String, (String, String)>, id: &str) -> String {
    match nets.get(id) { Some((n, g)) => format!("{}{}", kv("net", n), kv("g", g)), None => String::new() }
}

// Records -----------------------------------------------------------------------------------------------------------------

const PIN_TYPES: [&str; 8] = ["input", "bidirectional", "output", "open_collector", "passive", "tri_state", "open_emitter", "power_in"];

struct Cx<'a, 'b> { d: &'b Doc2<'a>, nets: HashMap<String, (String, String)> }

fn pin(o: &mut Out, cx: &Cx, r: &Props, ck: &str, rf: &str, sig: u32) {
    if pin_hidden(r) { return }
    let sh = &cx.d.sh;
    let (b, e, d, len) = pin_geom(sh, r);
    let (num, name) = (r.get("DESIGNATOR"), r.get("NAME"));
    let pk = format!("{ck}/pin/{num}");
    let ty = PIN_TYPES[r.int("ELECTRICAL").clamp(0, 7) as usize];
    let info: Info = Arc::from(format!("pin {num}{}{}{}{}", kv("name", name), kv("type", ty), kv("symbol", rf), net_kv(&cx.nets, &pk)));
    let rad = 0.6f32;
    let n = p(-d.y, d.x);
    let dot = r.int("SYMBOL_OUTEREDGE") == 1;
    let start = if dot { add(b, sc(d, 2.0 * rad)) } else { b };
    if len > 0.0 { o.push(pk.clone(), sig, PIN, Prim::Seg { a: if dot && len > 2.0 * rad { start } else { b }, b: e, w: THIN }, &info) }
    if dot { o.push(format!("{pk}/o"), sig, PIN, Prim::Circ { c: add(b, sc(d, rad)), r: rad, w: THIN, fill: false }, &info) }
    if r.int("SYMBOL_INNEREDGE") == 3 {
        o.push(format!("{pk}/c"), sig, PIN, Prim::Line { ps: vec![add(b, sc(n, rad)), add(b, sc(d, -1.5 * rad)), add(b, sc(n, -rad))], w: THIN }, &info);
    }
    let conglom = r.int("PINCONGLOMERATE");
    let f = sh.font(0);
    let f_name = if r.has("NAME_CUSTOMFONTID") { sh.font(r.int("NAME_CUSTOMFONTID")) } else { f };
    let f_num = if r.has("DESIGNATOR_CUSTOMFONTID") { sh.font(r.int("DESIGNATOR_CUSTOMFONTID")) } else { f };
    let horiz = d.x.abs() > 0.5;
    // the name inside the body, 5 units in (Altium's default pin margins); the number on the pin, its box on the pin's
    // line (left of it when the pin is upright), 8 units out from the body; a pin's own margin when it sets one (bit 0 of
    // its POSITIONCONGLOMERATE)
    let margin = |pos: &str, k: &str, def: f32| if r.int(pos) & 1 != 0 { val(r, k) * U } else { def * U };
    if conglom & 8 != 0 && !name.is_empty() {
        let at = add(b, sc(d, -margin("PINNAME_POSITIONCONGLOMERATE", "NAME_CUSTOMPOSITION_MARGIN", 5.0)));
        let hj = if horiz { if d.x > 0.0 { 2 } else { 0 } } else if d.y < 0.0 { 2 } else { 0 };
        o.text(name, at, !horiz, hj, 1, &f_name, &format!("{pk}/n"), sig, PIN_NAME, &info);
    }
    if conglom & 16 != 0 && !num.is_empty() {
        let at = add(b, sc(d, margin("PINDESIGNATOR_POSITIONCONGLOMERATE", "DESIGNATOR_CUSTOMPOSITION_MARGIN", 8.0)));
        let hj = if horiz { if d.x > 0.0 { 0 } else { 2 } } else if d.y < 0.0 { 0 } else { 2 };
        o.text(num, at, !horiz, hj, 2, &f_num, &format!("{pk}/#"), sig, PIN_NUM, &info);
    }
}

/// A graphic record's outline (and fill) on the page: in a component (`body`), on the sheet, or in the template (page).
#[allow(clippy::too_many_arguments)]
fn graphic(o: &mut Out, sh: &Sheet, r: &Props, key: &str, sig: u32, place: u32, info: &Info, fonts_text: Option<(&str, &Font)>) {
    let (ll, lf) = match place { 0 => (BODY, BODY_FILL), 1 => (GRAPHIC, SHEET_FILL), _ => (PAGE, PAGE) };
    let solid = r.bool("ISSOLID") && !r.bool("TRANSPARENT") && place != 2;
    let fill = if !solid { 0 } else if place == 0 && r.get("AREACOLOR") == r.get("COLOR") { 1 } else { 2 };
    let w = line_w(r);
    let c = sh.loc(r);
    match rec(r) {
        13 => o.push(key.to_string(), sig, ll, Prim::Seg { a: c, b: sh.at(r, "CORNER"), w }, info),
        6 => o.shape(sh.pts(r), false, w, 0, key, sig, ll, lf, info),
        7 => o.shape(sh.pts(r), true, w, fill, key, sig, ll, lf, info),
        5 => {
            let ps = sh.pts(r);
            let mut v = vec![];
            let mut i = 0;
            while i + 3 < ps.len() { for k in 0..=16 { if k > 0 || v.is_empty() { v.push(bez(k as f32 / 16.0, ps[i], ps[i + 1], ps[i + 2], ps[i + 3])) } } i += 3 }
            if v.len() < 2 { v = ps }
            o.shape(v, false, w, 0, key, sig, ll, lf, info)
        }
        14 | 30 => {
            let q = sh.at(r, "CORNER");
            let ps = vec![c, p(q.x, c.y), q, p(c.x, q.y)];
            o.shape(ps, true, if rec(r) == 30 { THIN } else { w }, if rec(r) == 30 { 0 } else { fill }, key, sig, ll, lf, info)
        }
        10 => {
            let q = sh.at(r, "CORNER");
            let (x0, x1, y0, y1) = (c.x.min(q.x), c.x.max(q.x), c.y.min(q.y), c.y.max(q.y));
            let rx = (val(r, "CORNERXRADIUS") * U).min((x1 - x0) / 2.0).max(0.0);
            let ry = (val(r, "CORNERYRADIUS") * U).min((y1 - y0) / 2.0).max(0.0);
            let mut ps = vec![];
            for (cx, cy, a0) in [(x1 - rx, y0 + ry, -90.0f32), (x1 - rx, y1 - ry, 0.0), (x0 + rx, y1 - ry, 90.0), (x0 + rx, y0 + ry, 180.0)] {
                for k in 0..=6 { let a = (a0 + 15.0 * k as f32).to_radians(); ps.push(p(cx + rx * a.cos(), cy + ry * a.sin())) }
            }
            o.shape(ps, true, w, fill, key, sig, ll, lf, info)
        }
        8 => {
            let (rx, ry) = (val(r, "RADIUS") * U, if r.has("SECONDARYRADIUS") { val(r, "SECONDARYRADIUS") * U } else { val(r, "RADIUS") * U });
            if (rx - ry).abs() < 1e-4 {
                if fill > 0 { o.push(format!("{key}/f"), sig, if fill == 1 { ll } else { lf }, Prim::Circ { c, r: rx, w: 0.0, fill: true }, info) }
                o.push(key.to_string(), sig, ll, Prim::Circ { c, r: rx, w, fill: false }, info)
            } else {
                let ps = (0..48).map(|k| { let a = k as f32 * fl::PI / 24.0; p(c.x + rx * a.cos(), c.y + ry * a.sin()) }).collect();
                o.shape(ps, true, w, fill, key, sig, ll, lf, info)
            }
        }
        12 | 11 | 9 => {
            let rx = val(r, "RADIUS") * U;
            let ry = if rec(r) == 11 { val(r, "SECONDARYRADIUS") * U } else { rx };
            let (a0, mut a1) = (r.num("STARTANGLE") as f32, r.num("ENDANGLE") as f32);
            let full = (a1 - a0).abs() < 1e-3 || ((a1 - a0).abs() - 360.0).abs() < 1e-3;
            if full { a1 = a0 + 360.0 } else { while a1 <= a0 { a1 += 360.0 } }
            if full && rec(r) == 12 { o.push(key.to_string(), sig, ll, Prim::Circ { c, r: rx, w, fill: false }, info); return }
            let steps = (((a1 - a0) / 7.5).ceil() as usize).clamp(4, 96);
            // the angles are the ellipse's own; Y up on the sheet is Y down here
            let mut ps: Vec<Pt> = (0..=steps).map(|k| {
                let a = (a0 + (a1 - a0) * k as f32 / steps as f32).to_radians();
                let t = (rx * a.sin()).atan2(ry * a.cos());
                p(c.x + rx * t.cos(), c.y - ry * t.sin())
            }).collect();
            if rec(r) == 9 { ps.push(c); o.shape(ps, true, w, fill, key, sig, ll, lf, info) } else { o.shape(ps, false, w, 0, key, sig, ll, lf, info) }
        }
        4 => {
            if let Some((s, f)) = fonts_text {
                let (up, hj, vj) = placed(r.int("ORIENTATION"), r.int("JUSTIFICATION"));
                o.text(s, c, up, hj, vj, f, key, sig, if place == 2 { PAGE } else { NOTE }, info)
            }
        }
        _ => {}
    }
}
fn bez(f: f32, a: Pt, b: Pt, c: Pt, d: Pt) -> Pt {
    let g = 1.0 - f;
    add(add(sc(a, g * g * g), sc(b, 3.0 * f * g * g)), add(sc(c, 3.0 * f * f * g), sc(d, f * f * f)))
}

fn component(o: &mut Out, cx: &Cx, i: usize) {
    let d = cx.d;
    let (c, ck) = (&d.rs[i], &d.keys[i]);
    let rf = d.designator(i);
    let csig = sig_of(c);
    let info: Info = Arc::from(format!("symbol{}{}{}{}{}", kv("ref", &rf), kv("value", &d.value(i)), kv("lib", c.get("LIBREFERENCE")), kv("footprint", &d.footprint(i)),
        kv("description", &c.get("COMPONENTDESCRIPTION").replace(';', ","))));
    for (n, &k) in d.kids(i).iter().enumerate() {
        let r = &d.rs[k];
        let sig = sig_of(r) ^ csig;
        let u = r.get("UNIQUEID");
        let gk = if u.is_empty() { format!("{ck}/g{n}") } else { format!("{ck}/g/{u}") };
        match rec(r) {
            2 => if d.shown_in(c, r) { pin(o, cx, r, ck, &rf, sig) },
            4..=14 => if d.shown_in(c, r) {
                let s = if rec(r) == 4 && !r.bool("ISHIDDEN") { d.resolve(r.get("TEXT"), i as i64) } else { String::new() };
                let f = d.sh.font(r.int("FONTID"));
                graphic(o, &d.sh, r, &gk, sig, 0, &info, Some((s.as_str(), &f)))
            },
            34 | 41 => {
                if r.bool("ISHIDDEN") || !r.has("LOCATION.X") && !r.has("LOCATION.Y") { continue }
                let name = r.get("NAME");
                let s = if rec(r) == 34 { rf.clone() } else { d.resolve(r.get("TEXT"), i as i64) };
                let s = if r.bool("SHOWNAME") { format!("{name}: {s}") } else { s };
                let (up, hj, vj) = placed(r.int("ORIENTATION"), r.int("JUSTIFICATION"));
                let layer = if rec(r) == 34 || name.eq_ignore_ascii_case("Comment") { FIELD } else { FIELD_OTHER };
                let fk = if rec(r) == 34 { "Reference".to_string() } else if name.eq_ignore_ascii_case("Comment") { "Value".to_string() } else { name.to_string() };
                let f = d.sh.font(r.int("FONTID"));
                let ps = if s.trim().is_empty() { vec![] } else { text_prims(&s, d.sh.loc(r), up, hj, vj, &f) };
                // a sheet placed several times (channels): Altium shows the first channel's designator after it, small,
                // at its top right ("R1 (R1_CH1, ...)")
                if rec(r) == 34 && !d.ctx.channels.is_empty() && !ps.is_empty() && !rf.contains('?') {
                    let (mut x0, mut y0, mut x1) = (f32::MAX, f32::MAX, f32::MIN);
                    for q in ps.iter().flat_map(|pr| match pr { Prim::Line { ps, .. } | Prim::Poly { ps, .. } => ps.clone(), Prim::Seg { a, b, .. } => vec![*a, *b], _ => vec![] }) { x0 = x0.min(q.x); x1 = x1.max(q.x); y0 = y0.min(q.y) }
                    let small = Font { h: f.h * 0.6, ..f };
                    let hint = format!("({rf}_{}, ...)", d.ctx.channels[0]);
                    let at = if up { p(x0, y0 - 0.4) } else { p(x1 + 0.4, y0) };
                    o.text(&hint, at, up, 0, 0, &small, &format!("{ck}/f/Channels"), sig, FIELD_OTHER, &info);
                }
                if !ps.is_empty() { drawn(&s, d.sh.loc(r), layer, &ps); o.put(ps, &format!("{ck}/f/{fk}"), sig, layer, &info) }
            }
            _ => {}
        }
    }
}

fn power_port(o: &mut Out, cx: &Cx, r: &Props, key: &str, sig: u32) {
    let sh = &cx.d.sh;
    let at = sh.loc(r);
    let t = r.get("TEXT");
    let info: Info = Arc::from(format!("global label{}{}{}", kv("text", t), kv("shape", "power"), net_kv(&cx.nets, key)));
    let orient = r.int("ORIENTATION").rem_euclid(4);
    let d = match orient { 0 => p(1.0, 0.0), 1 => p(0.0, -1.0), 2 => p(-1.0, 0.0), _ => p(0.0, 1.0) };
    let n = p(-d.y, d.x);
    // shapes in units along the port (a) and across it (c)
    let q = |a: f32, c: f32| add(at, add(sc(d, a * U), sc(n, c * U)));
    let style = if r.bool("ISCROSSSHEETCONNECTOR") { 1 } else { r.int("STYLE") };
    let mut ls: Vec<Vec<Pt>> = vec![];
    let mut circ = None;
    let mut fill = None;
    let reach = match style {
        0 => { ls.push(vec![q(0.0, 0.0), q(5.0, 0.0)]); circ = Some((q(7.5, 0.0), 2.5 * U)); 12.0 }
        1 | 7 => { ls.push(vec![q(0.0, 0.0), q(10.0, 0.0)]); fill = Some(vec![q(5.0, -2.5), q(10.0, 0.0), q(5.0, 2.5)]); 12.0 }
        3 => { ls.push(vec![q(0.0, 0.0), q(7.0, 0.0)]); ls.push((0..=16).map(|k| { let b = bez(k as f32 / 16.0, p(5.0, 3.0), p(8.7, 3.0), p(6.3, -3.0), p(10.0, -3.0)); q(b.x, b.y) }).collect()); 12.0 }
        4 | 8 => { ls.push(vec![q(0.0, 0.0), q(10.0, 0.0)]); for (a, c) in [(10.0, 10.0), (13.0, 7.0), (16.0, 4.0), (19.0, 1.0)] { ls.push(vec![q(a, -c), q(a, c)]) } 21.0 }
        5 => { ls.push(vec![q(0.0, 0.0), q(10.0, 0.0)]); ls.push(vec![q(10.0, -10.0), q(10.0, 10.0), q(20.0, 0.0), q(10.0, -10.0)]); 22.0 }
        6 | 9 => { ls.push(vec![q(0.0, 0.0), q(10.0, 0.0)]); ls.push(vec![q(20.0, -15.0), q(10.0, -10.0), q(10.0, 10.0), q(20.0, 5.0)]); ls.push(vec![q(10.0, 0.0), q(20.0, -5.0)]); 22.0 }
        _ => { ls.push(vec![q(0.0, 0.0), q(10.0, 0.0)]); ls.push(vec![q(10.0, -5.0), q(10.0, 5.0)]); 12.0 }
    };
    for (j, l) in ls.into_iter().enumerate() { o.push(if j == 0 { key.to_string() } else { format!("{key}/s{j}") }, sig, GLOBAL, Prim::Line { ps: l, w: THIN }, &info) }
    if let Some((c, rad)) = circ { o.push(format!("{key}/c"), sig, GLOBAL, Prim::Circ { c, r: rad, w: THIN, fill: false }, &info) }
    if let Some(ps) = fill { o.push(format!("{key}/a"), sig, GLOBAL, Prim::Poly { ps, w: THIN, fill: true }, &info) }
    if r.bool("SHOWNETNAME") || !r.has("SHOWNETNAME") {
        let f = sh.font(r.int("FONTID"));
        let tp = q(reach, 0.0);
        let (hj, vj) = match orient { 0 => (0, 1), 1 => (1, 2), 2 => (2, 1), _ => (1, 0) };
        o.text(t, tp, false, hj, vj, &f, &format!("{key}/t"), sig, GLOBAL, &info);
    }
}

/// A port's two ends: horizontal styles run right from LOCATION, vertical ones up from it.
fn port_ends(sh: &Sheet, r: &Props) -> (Pt, Pt) {
    let a = sh.loc(r);
    let w = val(r, "WIDTH") * U;
    if r.int("STYLE") >= 4 { (a, p(a.x, a.y - w)) } else { (a, p(a.x + w, a.y)) }
}
fn port(o: &mut Out, cx: &Cx, r: &Props, key: &str, sig: u32) {
    let sh = &cx.d.sh;
    let (a, b) = port_ends(sh, r);
    let name = r.get("NAME");
    let style = r.int("STYLE");
    let info: Info = Arc::from(format!("hier label{}{}{}", kv("text", name), kv("shape", ["", "output", "input", "bidirectional"][r.int("IOTYPE").clamp(0, 3) as usize]), net_kv(&cx.nets, key)));
    let len = ((b.x - a.x).abs() + (b.y - a.y).abs()).max(0.01);
    let h = (if r.has("HEIGHT") { val(r, "HEIGHT") } else { 10.0 }).max(2.0) * U / 2.0;
    let d = p((b.x - a.x) / len, (b.y - a.y) / len);
    let n = p(-d.y, d.x);
    let q = |u: f32, v: f32| add(a, add(sc(d, u), sc(n, v)));
    let s = style % 4; // 0 none, 1 point at the start, 2 at the end, 3 both
    let tip = h.min(len / 2.0);
    let (p0, p1) = (s == 1 || s == 3, s == 2 || s == 3);
    let mut ps = vec![];
    if p0 { ps.push(q(0.0, 0.0)) }
    ps.push(q(if p0 { tip } else { 0.0 }, -h));
    ps.push(q(if p1 { len - tip } else { len }, -h));
    if p1 { ps.push(q(len, 0.0)) }
    ps.push(q(if p1 { len - tip } else { len }, h));
    ps.push(q(if p0 { tip } else { 0.0 }, h));
    o.push(key.to_string(), sig, HIER, Prim::Poly { ps, w: THIN, fill: false }, &info);
    let f = sh.font(r.int("FONTID"));
    let vert = style >= 4;
    let pad = tip + 0.4;
    // ALIGNMENT: 0 centre, 1 right, 2 left (along the port)
    let (u, hj) = match r.int("ALIGNMENT") { 1 => (len - pad, 2), 2 => (pad, 0), _ => (len / 2.0, 1) };
    o.text(name, q(u, 0.0), vert, hj, 1, &f, &format!("{key}/t"), sig, HIER, &info);
}

/// A sheet entry's point on its sheet symbol's edge and its inward direction.
fn entry_at(sh: &Sheet, sym: &Props, r: &Props) -> (Pt, Pt) {
    let tl = sh.loc(sym);
    let (w, h) = (val(sym, "XSIZE") * U, val(sym, "YSIZE") * U);
    let dist = (r.int("DISTANCEFROMTOP") as f32 * 10.0 + r.int("DISTANCEFROMTOP_FRAC1") as f32 / 100000.0) * U;
    match r.int("SIDE") { 1 => (p(tl.x + w, tl.y + dist), p(-1.0, 0.0)), 2 => (p(tl.x + dist, tl.y), p(0.0, 1.0)), 3 => (p(tl.x + dist, tl.y + h), p(0.0, -1.0)), _ => (p(tl.x, tl.y + dist), p(1.0, 0.0)) }
}
fn sheet_box(sh: &Sheet, r: &Props) -> (f32, f32, f32, f32) {
    let tl = sh.loc(r);
    (tl.x, tl.y, tl.x + val(r, "XSIZE") * U, tl.y + val(r, "YSIZE") * U)
}
fn sheet_symbol(o: &mut Out, cx: &Cx, i: usize) {
    let d = cx.d;
    let (r, key) = (&d.rs[i], &d.keys[i]);
    let sig = sig_of(r);
    let (name, file) = sheet_names(d.rs, &d.kids, i);
    let info: Info = Arc::from(format!("sheet{}{}", kv("name", &name), kv("file", &file)));
    let (x0, y0, x1, y1) = sheet_box(&d.sh, r);
    let ps = vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
    o.push(format!("{key}/f"), sig, SHEET_FILL, Prim::Poly { ps: ps.clone(), w: 0.0, fill: true }, &info);
    o.push(key.clone(), sig, SHEET, Prim::Poly { ps, w: line_w(r).max(THIN), fill: false }, &info);
    for &k in d.kids(i) {
        let c = &d.rs[k];
        let csig = sig_of(c);
        match rec(c) {
            32 | 33 => {
                if c.bool("ISHIDDEN") { continue }
                let (up, hj, vj) = placed(c.int("ORIENTATION"), c.int("JUSTIFICATION"));
                let (k2, layer) = if rec(c) == 32 { (format!("{key}/name"), SHEET_NAME) } else { (format!("{key}/file"), SHEET_FILE) };
                o.text(c.get("TEXT"), d.sh.loc(c), up, hj, vj, &d.sh.font(c.int("FONTID")), &k2, csig, layer, &info);
            }
            16 => {
                let nm = c.get("NAME");
                let pk = format!("{key}/pin/{nm}");
                let pi: Info = Arc::from(format!("sheet pin{}{}{}{}", kv("name", nm), kv("shape", ["", "output", "input", "bidirectional"][c.int("IOTYPE").clamp(0, 3) as usize]), kv("sheet", &name), net_kv(&cx.nets, &pk)));
                let (q, inw) = entry_at(&d.sh, r, c);
                let f = d.sh.font(c.int("TEXTFONTID"));
                let s = 1.0f32; // half the entry's height
                let n = p(-inw.y, inw.x);
                let at = |u: f32, v: f32| add(q, add(sc(inw, u), sc(n, v)));
                // IOTYPE 1 output (points out), 2 input (points in), 3 both
                let io = c.int("IOTYPE");
                let l = 2.5f32;
                let ps = match io {
                    1 => vec![at(0.0, 0.0), at(s, -s), at(l, -s), at(l, s), at(s, s)],
                    2 => vec![at(0.0, -s), at(l - s, -s), at(l, 0.0), at(l - s, s), at(0.0, s)],
                    3 => vec![at(0.0, 0.0), at(s, -s), at(l - s, -s), at(l, 0.0), at(l - s, s), at(s, s)],
                    _ => vec![at(0.0, -s), at(l, -s), at(l, s), at(0.0, s)],
                };
                o.push(pk.clone(), csig, SHEET_PIN, Prim::Poly { ps, w: THIN, fill: false }, &pi);
                let tp = at(l + 0.6, 0.0);
                let up = inw.x.abs() < 0.5;
                let hj = if up { if inw.y < 0.0 { 0 } else { 2 } } else if inw.x > 0.0 { 0 } else { 2 };
                o.text(nm, tp, up, hj, 1, &f, &format!("{pk}/t"), csig, SHEET_PIN, &pi);
            }
            41 => {
                if c.bool("ISHIDDEN") { continue }
                let (up, hj, vj) = placed(c.int("ORIENTATION"), c.int("JUSTIFICATION"));
                o.text(&d.resolve(c.get("TEXT"), i as i64), d.sh.loc(c), up, hj, vj, &d.sh.font(c.int("FONTID")), &format!("{key}/f/{}", c.get("NAME")), csig, FIELD_OTHER, &info);
            }
            _ => {}
        }
    }
}
fn sheet_names(rs: &[Props], kids: &HashMap<i64, Vec<usize>>, i: usize) -> (String, String) {
    let (mut name, mut file) = (String::new(), String::new());
    for &k in kids.get(&(i as i64)).map(|v| v.as_slice()).unwrap_or(&[]) {
        match rec(&rs[k]) { 32 => name = rs[k].get("TEXT").to_string(), 33 => file = rs[k].get("TEXT").to_string(), _ => {} }
    }
    (name, file)
}

/// A directive (parameter set), at LOCATION pointing along ORIENTATION: a differential pair's is Altium's two-wave mark; any
/// other (a net class...) a stem and a ring, with its name beyond them, upright.
fn directive(o: &mut Out, d: &Doc2, i: usize, key: &str, sig: u32) {
    let r = &d.rs[i];
    let at = d.sh.loc(r);
    let name = r.get("NAME");
    let info: Info = Arc::from(format!("directive{}", kv("name", name)));
    let orient = r.int("ORIENTATION").rem_euclid(4);
    // along and across the directive, in units on the sheet (Y up), turned by its orientation, then onto the page
    let (ax, ay) = [(1.0f32, 0.0f32), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)][orient as usize];
    let q = |u: f32, v: f32| p(at.x + (u * ax - v * ay) * U, at.y - (u * ay + v * ax) * U);
    let diff = d.kids(i).iter().any(|&k| rec(&d.rs[k]) == 41 && d.rs[k].get("NAME").eq_ignore_ascii_case("DifferentialPair"));
    if diff {
        let wave = |lo: f32, hi: f32| vec![q(4.4, lo), q(8.8, lo), q(10.6, hi), q(15.6, hi), q(17.5, lo), q(21.2, lo)];
        o.push(key.to_string(), sig, NOTE, Prim::Line { ps: vec![q(0.0, 0.0), q(5.0, 4.7)], w: THIN }, &info);
        o.push(format!("{key}/a"), sig, NOTE, Prim::Line { ps: wave(3.7, 5.6), w: 0.2 }, &info);
        o.push(format!("{key}/b"), sig, NOTE, Prim::Line { ps: wave(3.1, 1.0), w: 0.2 }, &info);
        return
    }
    o.push(key.to_string(), sig, NOTE, Prim::Seg { a: q(0.0, 0.0), b: q(12.0, 0.0), w: THIN }, &info);
    o.push(format!("{key}/o"), sig, NOTE, Prim::Circ { c: q(16.0, 0.0), r: 4.0 * U, w: THIN, fill: false }, &info);
    let c = q(16.0, 0.0);
    o.push(format!("{key}/i"), sig, NOTE, Prim::Seg { a: p(c.x, c.y - 1.6 * U), b: p(c.x, c.y + 2.2 * U), w: THIN }, &info);
    let (hj, vj) = match orient { 0 => (0, 1), 1 => (1, 2), 2 => (2, 1), _ => (1, 0) };
    o.text(name, q(21.5, 0.0), false, hj, vj, &d.sh.font(r.int("FONTID")), &format!("{key}/t"), sig, NOTE, &info);
}

fn text_frame(o: &mut Out, cx: &Cx, r: &Props, key: &str, sig: u32) {
    let sh = &cx.d.sh;
    let (a, b) = (sh.loc(r), sh.at(r, "CORNER"));
    let (x0, x1, y0, y1) = (a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y));
    let ps = vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
    let info: Info = Arc::from("text box");
    if r.bool("ISSOLID") { o.push(format!("{key}/tint"), sig, SHEET_FILL, Prim::Poly { ps: ps.clone(), w: 0.0, fill: true }, &info) }
    if r.bool("SHOWBORDER") { o.push(key.to_string(), sig, NOTE, Prim::Poly { ps, w: line_w(r), fill: false }, &info) }
    let f = sh.font(r.int("FONTID"));
    let m = ((if r.has("TEXTMARGIN") || r.has("TEXTMARGIN_FRAC") { val(r, "TEXTMARGIN") } else { 5.0 }) * U).max(0.3);
    let shown = cx.d.mentions(&cx.d.resolve(r.get("TEXT"), -1));
    let s = &shown;
    // the characters as wide as Altium's font draws them (the widest line's measure), so lines wrap where Altium's do
    let (want, have) = (em_width(s, f.face) * f.face.em() * f.line(), strokefont::width(&markup(s), f.w()));
    let cw = if want > 0.0 && have > 0.0 { f.w() * (want / have).clamp(0.7, 1.5) } else { f.w() };
    let s = if r.bool("WORDWRAP") { strokefont::wrap(&markup(s), cw, (x1 - x0 - 2.0 * m).max(1.0)) } else { markup(s) };
    let (hj, x) = match r.int("ALIGNMENT") { 2 => (1, (x0 + x1) / 2.0), 3 => (2, x1 - m), _ => (0, x0 + m) };
    if s.trim().is_empty() { return }
    // already marked up: drawn as it is
    let ps = strokefont::text_full(&s, x, y0 + m + 0.207 * f.line(), f.h, cw, f.pen(), 0.0, hj, 0, false, f.italic);
    drawn(&shown, p(x, y0 + m), NOTE, &ps);
    o.put(ps, &format!("{key}/t"), sig, NOTE, &info);
}

// The page: border, zones and title block ------------------------------------------------------------------------------

fn page(o: &mut Out, d: &Doc2, sheet: &Props) {
    let sh = &d.sh;
    let (w, h) = (sh.w * U, sh.h * U);
    let info: Info = Arc::from("page");
    let sig = sig_of(sheet);
    let mut ps: Vec<Prim> = vec![];
    let bx = |x0: f32, y0: f32, x1: f32, y1: f32| Prim::Poly { ps: vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)], w: THIN, fill: false };
    let seg = |x0: f32, y0: f32, x1: f32, y1: f32| Prim::Seg { a: p(x0, y0), b: p(x1, y1), w: THIN };
    ps.push(bx(0.0, 0.0, w, h));
    let border = !sheet.has("BORDERON") || sheet.bool("BORDERON");
    // a custom sheet has its own margin and zones; a standard one's (whatever the CUSTOM fields say) go by its size
    let (m, xz, yz) = if sheet.bool("USECUSTOMSHEET") {
        let (xz, yz) = (sheet.int("CUSTOMXZONES").clamp(0, 64), sheet.int("CUSTOMYZONES").clamp(0, 64));
        ((if sheet.has("CUSTOMMARGINWIDTH") { val(sheet, "CUSTOMMARGINWIDTH") } else { 20.0 }) * U, if xz > 0 { xz } else { 6 }, if yz > 0 { yz } else { 4 })
    } else { let (m, xz, yz) = standard_zones(sh.w.max(sh.h)); (m * U, xz, yz) };
    if border && m > 0.0 && w > 4.0 * m && h > 4.0 * m {
        ps.push(bx(m, m, w - m, h - m));
        let zones = !sheet.has("REFERENCEZONESON") || sheet.bool("REFERENCEZONESON");
        if zones {
            let f = Font { h: (m * 0.45).min(2.0), bold: false, italic: false, face: Face::Serif };
            let zw = (w - 2.0 * m) / xz as f32;
            for i in 0..xz {
                let x = m + zw * i as f32;
                if i > 0 { ps.push(seg(x, 0.0, x, m)); ps.push(seg(x, h - m, x, h)) }
                let s = (i + 1).to_string();
                ps.extend(text_prims(&s, p(x + zw / 2.0, m / 2.0), false, 1, 1, &f));
                ps.extend(text_prims(&s, p(x + zw / 2.0, h - m / 2.0), false, 1, 1, &f));
            }
            let zh = (h - 2.0 * m) / yz as f32;
            for i in 0..yz {
                let y = m + zh * i as f32;
                if i > 0 { ps.push(seg(0.0, y, m, y)); ps.push(seg(w - m, y, w, y)) }
                let s = ((b'A' + (i as u8 % 26)) as char).to_string();
                ps.extend(text_prims(&s, p(m / 2.0, y + zh / 2.0), false, 1, 1, &f));
                ps.extend(text_prims(&s, p(w - m / 2.0, y + zh / 2.0), false, 1, 1, &f));
            }
        }
    }
    // Altium's standard title block, 350 by 80 units in the bottom right corner (a template draws its own instead)
    if sheet.bool("TITLEBLOCKON") {
        let (rx, ry) = (w - m, h - m);
        let (tw, th) = (350.0 * U, 80.0 * U);
        let (x0, y0) = (rx - tw, ry - th);
        ps.push(bx(x0, y0, rx, ry));
        let r1 = y0 + 30.0 * U; let r2 = r1 + 30.0 * U; let r3 = r2 + 10.0 * U;
        for y in [r1, r2, r3] { ps.push(seg(x0, y, rx, y)) }
        ps.push(seg(x0 + 50.0 * U, r1, x0 + 50.0 * U, r2));
        ps.push(seg(x0 + 250.0 * U, r1, x0 + 250.0 * U, r2));
        ps.push(seg(x0 + 200.0 * U, r2, x0 + 200.0 * U, ry));
        let small = Font { h: 1.1, bold: false, italic: false, face: Face::Serif };
        let big = Font { h: 2.2, bold: false, italic: false, face: Face::Serif };
        // the block's own fields are captions only (a sheet writes its title, number... as text on it), but for the size, the
        // date and the file, which Altium fills in itself
        let lab = |ps: &mut Vec<Prim>, s: &str, x: f32, y: f32| ps.extend(text_prims(s, p(x + 0.8, y + 0.8), false, 0, 0, &small));
        lab(&mut ps, "Title", x0, y0);
        lab(&mut ps, "Size", x0, r1); ps.extend(text_prims(STYLE_NAMES[sheet.int("SHEETSTYLE").clamp(0, 17) as usize], p(x0 + 2.0, r2 - 1.5), false, 0, 2, &big));
        lab(&mut ps, "Number", x0 + 50.0 * U, r1);
        lab(&mut ps, "Revision", x0 + 250.0 * U, r1);
        let mid = |a: f32, b: f32| (a + b) / 2.0;
        let date = d.builtin("CurrentDate").unwrap_or_default();
        ps.extend(text_prims(&format!("Date: {date}"), p(x0 + 0.8, mid(r2, r3)), false, 0, 1, &small));
        ps.extend(text_prims("Sheet    of", p(x0 + 200.0 * U + 0.8, mid(r2, r3)), false, 0, 1, &small));
        ps.extend(text_prims(&format!("File: {}", short_path(&d.ctx.path, 44)), p(x0 + 0.8, mid(r3, ry)), false, 0, 1, &small));
        ps.extend(text_prims("Drawn By:", p(x0 + 200.0 * U + 0.8, mid(r3, ry)), false, 0, 1, &small));
    }
    o.put(ps, "page", sig, PAGE, &info);
}

/// A standard sheet's margin (units) and reference zones across and down, by its longer side (units), as Altium draws them
fn standard_zones(long: f32) -> (f32, i64, i64) {
    let m = if long > 1500.0 { 30.0 } else { 20.0 };
    let xz = if long <= 1200.0 { 4 } else if long <= 2000.0 { 6 } else if long <= 3300.0 { 8 } else { 12 };
    (m, xz, 4)
}
/// A path cut to about `n` characters as Altium's title block cuts it: the start, "..", the file's name
fn short_path(path: &str, n: usize) -> String {
    if path.chars().count() <= n { return path.to_string() }
    let sep = if path.contains('\\') { '\\' } else { '/' };
    let name = path.rsplit(sep).next().unwrap_or(path);
    let mut head = String::new();
    for part in path.split(sep) {
        if head.chars().count() + part.chars().count() + name.chars().count() + 4 > n || part == name { break }
        head.push_str(part); head.push(sep);
    }
    format!("{head}..{sep}{name}")
}

// The whole sheet ---------------------------------------------------------------------------------------------------------

fn draw(rs: &Recs, ctx: Ctx) -> Vec<Item> {
    let d = Doc2::with(&rs.main, ctx);
    let nets = connect(&d, &rs.more);
    let cx = Cx { d: &d, nets };
    let mut o = Out { items: Vec::with_capacity(rs.main.len() * 4), keys: HashSet::new() };
    if let Some(sheet) = rs.main.iter().find(|r| rec(r) == 31) { page(&mut o, &d, sheet) }
    let templates: HashSet<usize> = rs.main.iter().enumerate().filter(|(_, r)| rec(r) == 39).map(|(i, _)| i).collect();
    let tinfo: Info = Arc::from("graphic");
    for (i, r) in rs.main.iter().enumerate() {
        let key = d.keys[i].as_str();
        let sig = sig_of(r);
        let own = owner(r);
        let in_template = own >= 0 && templates.contains(&(own as usize));
        // children of components, sheet symbols and implementation lists are drawn with their owner
        if !in_template && own >= 0 && !matches!(rec(r), 41) { if (own as usize) < rs.main.len() && rec(&rs.main[own as usize]) != 31 { continue } }
        match rec(r) {
            1 => component(&mut o, &cx, i),
            15 => sheet_symbol(&mut o, &cx, i),
            27 | 26 => {
                let (layer, w, info) = if rec(r) == 27 { (WIRE, wire_w(r), format!("wire{}", net_kv(&cx.nets, key))) } else { (BUS, wire_w(r).max(0.3048), "bus".to_string()) };
                o.shape(d.sh.pts(r), false, w, 0, key, sig, layer, layer, &Arc::from(info));
            }
            37 => o.push(key.to_string(), sig, BUS_ENTRY, Prim::Seg { a: d.sh.loc(r), b: d.sh.at(r, "CORNER"), w: wire_w(r) }, &Arc::from("bus entry")),
            29 => o.push(key.to_string(), sig, JUNCTION, Prim::Circ { c: d.sh.loc(r), r: 0.508, w: 0.0, fill: true }, &Arc::from(format!("junction{}", net_kv(&cx.nets, key)))),
            22 => {
                let q = d.sh.loc(r);
                let s = 0.635f32;
                let info: Info = Arc::from("no connect");
                o.push(key.to_string(), sig, NC, Prim::Seg { a: p(q.x - s, q.y - s), b: p(q.x + s, q.y + s), w: THIN }, &info);
                o.push(format!("{key}/2"), sig, NC, Prim::Seg { a: p(q.x - s, q.y + s), b: p(q.x + s, q.y - s), w: THIN }, &info);
            }
            25 => {
                let t = r.get("TEXT");
                let info: Info = Arc::from(format!("label{}{}", kv("text", t), net_kv(&cx.nets, key)));
                let (up, hj, vj) = placed(r.int("ORIENTATION"), r.int("JUSTIFICATION"));
                let f = d.sh.font(r.int("FONTID"));
                o.text(t, d.sh.loc(r), up, hj, vj, &f, key, sig, LABEL, &info);
            }
            17 => power_port(&mut o, &cx, r, key, sig),
            18 => port(&mut o, &cx, r, key, sig),
            28 | 209 => text_frame(&mut o, &cx, r, key, sig),
            43 => directive(&mut o, &d, i, key, sig),
            41 => {
                // the sheet's, directives' and ports' parameters (a port's cross reference, "Sheet[2B]"); a component's and a
                // sheet symbol's are drawn with their owner, pins' and models' never
                let or = d.owner_rec(r);
                if r.bool("ISHIDDEN") || !matches!(or, -1 | 31 | 43 | 18 | 17) || (!r.has("LOCATION.X") && !r.has("LOCATION.Y")) { continue }
                let (up, hj, vj) = placed(r.int("ORIENTATION"), r.int("JUSTIFICATION"));
                let info: Info = Arc::from(format!("parameter{}", kv("name", r.get("NAME"))));
                let s = d.resolve(r.get("TEXT"), own);
                let s = if r.bool("SHOWNAME") { format!("{}: {s}", r.get("NAME")) } else { s };
                o.text(&s, d.sh.loc(r), up, hj, vj, &d.sh.font(r.int("FONTID")), &format!("{key}/t"), sig, FIELD_OTHER, &info);
            }
            4..=14 | 30 => {
                let place = if in_template { 2 } else { 1 };
                let s = if rec(r) == 4 && !r.bool("ISHIDDEN") { d.resolve(r.get("TEXT"), own) } else { String::new() };
                let f = d.sh.font(r.int("FONTID"));
                let info = if in_template { Arc::from("page") } else if rec(r) == 4 { Arc::from("text") } else { tinfo.clone() };
                graphic(&mut o, &d.sh, r, key, sig, place, &info, Some((s.as_str(), &f)));
            }
            _ => {}
        }
    }
    additional(&mut o, &d, &rs.more);
    o.items
}

/// The `Additional` stream: harness connectors (215) with their entries (216) and type (217), signal harnesses (218)
/// and blankets (225).
fn additional(o: &mut Out, d: &Doc2, more: &[Props]) {
    let sh = &d.sh;
    let mut box_of: Option<(Pt, f32, f32, i64)> = None;
    for r in more {
        let key = more_key(r);
        let sig = sig_of(r);
        match rec(r) {
            215 => {
                let tl = sh.loc(r);
                let (w, h) = (val(r, "XSIZE") * U, val(r, "YSIZE") * U);
                let info: Info = Arc::from("harness connector");
                o.push(key, sig, SHEET, Prim::Poly { ps: vec![tl, p(tl.x + w, tl.y), p(tl.x + w, tl.y + h), p(tl.x, tl.y + h)], w: THIN, fill: false }, &info);
                box_of = Some((tl, w, h, r.int("HARNESSCONNECTORSIDE")));
            }
            216 => {
                let Some((tl, w, h, _)) = box_of else { continue };
                let dist = (r.int("DISTANCEFROMTOP") as f32 * 10.0 + r.int("DISTANCEFROMTOP_FRAC1") as f32 / 100000.0) * U;
                let info: Info = Arc::from(format!("harness entry{}", kv("name", r.get("NAME"))));
                let (q, hj) = match r.int("SIDE") { 1 => (p(tl.x + w - 0.6, tl.y + dist), 2), 2 => (p(tl.x + dist, tl.y + 0.6), 0), 3 => (p(tl.x + dist, tl.y + h - 0.6), 0), _ => (p(tl.x + 0.6, tl.y + dist), 0) };
                o.text(r.get("NAME"), q, false, hj, 1, &sh.font(r.int("TEXTFONTID")), &key, sig, SHEET_PIN, &info);
            }
            217 => {
                if r.bool("ISHIDDEN") { continue }
                o.text(r.get("TEXT"), sh.loc(r), false, 0, 2, &sh.font(r.int("FONTID")), &key, sig, SHEET_NAME, &Arc::from("harness type"));
            }
            218 => o.shape(sh.pts(r), false, 0.6, 0, &key, sig, BUS, BUS, &Arc::from("signal harness")),
            225 => o.shape(sh.pts(r), true, THIN, 0, &key, sig, NOTE, NOTE, &Arc::from("blanket")),
            _ => {}
        }
    }
}

// Public API --------------------------------------------------------------------------------------------------------------

/// A whole .SchDoc as schematic items (kicad/schematic.rs' layers and conventions); None when it is not a readable one.
pub fn items(bytes: &[u8]) -> Option<Vec<Item>> { items_in(bytes, Ctx::default()) }
/// `items` with the special strings answered from `ctx` (the project's parameters, the file's name and date)
pub fn items_in(bytes: &[u8], ctx: Ctx) -> Option<Vec<Item>> { load(bytes).map(|r| draw(&r, ctx)) }

/// The sheet symbols on this sheet: (sheet name, child file name as written), in file order.
pub fn sheets(bytes: &[u8]) -> Vec<(String, String)> {
    let Some(r) = load(bytes) else { return vec![] };
    let d = Doc2::new(&r.main);
    r.main.iter().enumerate().filter(|(_, x)| rec(x) == 15).map(|(i, _)| sheet_names(d.rs, &d.kids, i)).collect()
}
/// Each sheet symbol's box (x0, y0, x1, y1) in mm on the page (Y down), in the order `sheets` gives them.
pub fn sheet_boxes(bytes: &[u8]) -> Vec<(f32, f32, f32, f32)> {
    let Some(r) = load(bytes) else { return vec![] };
    let sh = sheet_of(&r.main);
    r.main.iter().filter(|x| rec(x) == 15).map(|x| sheet_box(&sh, x)).collect()
}

/// The components (one per designator; a multi-part component once), for the BOM and the names list. Graphical
/// components (COMPONENTKIND 2) are left out; power ports are not components.
pub fn parts(bytes: &[u8]) -> Vec<Part> {
    let Some(r) = load(bytes) else { return vec![] };
    let d = Doc2::new(&r.main);
    let mut seen = HashSet::new();
    let mut out = vec![];
    for (i, c) in r.main.iter().enumerate() {
        if rec(c) != 1 || c.int("COMPONENTKIND") == 2 { continue }
        let refdes = d.raw_designator(i);
        if refdes.is_empty() || !seen.insert(refdes.clone()) { continue }
        let fields = d.kids(i).iter().map(|&k| &d.rs[k]).filter(|x| rec(x) == 41 && !x.get("NAME").eq_ignore_ascii_case("Comment"))
            .map(|x| (x.get("NAME").to_string(), d.resolve(x.get("TEXT"), i as i64))).filter(|(n, v)| !n.is_empty() && !v.is_empty() && v != "*" && !v.starts_with('=')).collect();
        out.push(Part { refdes, value: d.value(i), footprint: d.footprint(i), lib_ref: c.get("LIBREFERENCE").to_string(), description: c.get("COMPONENTDESCRIPTION").to_string(), fields });
    }
    out
}

/// The net names the sheet writes: net labels, power ports and ports, each once, in file order.
pub fn nets(bytes: &[u8]) -> Vec<String> {
    let Some(r) = load(bytes) else { return vec![] };
    let mut seen = HashSet::new();
    r.main.iter().filter_map(|x| match rec(x) { 25 | 17 => Some(x.get("TEXT")), 18 => Some(x.get("NAME")), _ => None })
        .filter(|n| !n.is_empty() && seen.insert(n.to_string())).map(|n| n.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A .SchDoc made of these records (each "|KEY=V|..."), after the header.
    fn schdoc(recs: &[&str]) -> Vec<u8> {
        let mut s = Vec::new();
        let mut put = |body: &str| { let mut b = body.as_bytes().to_vec(); b.push(0); s.extend_from_slice(&(b.len() as u32).to_le_bytes()); s.extend_from_slice(&b) };
        put("|HEADER=Protel for Windows - Schematic Capture Binary File Version 5.0|WEIGHT=1");
        for r in recs { put(r) }
        let mut cf = cfb::CompoundFile::create(std::io::Cursor::new(Vec::new())).unwrap();
        cf.create_stream("/FileHeader").unwrap().write_all(&s).unwrap();
        cf.flush().unwrap();
        cf.into_inner().into_inner()
    }
    const SHEET: &str = "|RECORD=31|FONTIDCOUNT=1|SIZE1=10|FONTNAME1=Times New Roman|SYSTEMFONT=1|BORDERON=T|SHEETSTYLE=0";
    /// a resistor R1 (index 1) with two pins, a body, designator, comment and footprint; a wire from pin 2 to a GND power
    /// port; a net label on a wire from pin 1; a sheet symbol with its name, file and an entry
    fn sample() -> Vec<u8> {
        schdoc(&[
            SHEET,                                                                                                                                  // 0
            "|RECORD=1|LIBREFERENCE=Res|COMPONENTDESCRIPTION=Resistor|PARTCOUNT=3|CURRENTPARTID=1|LOCATION.X=100|LOCATION.Y=500|UNIQUEID=RRRRRRRR", // 1
            "|RECORD=2|OWNERINDEX=1|OWNERPARTID=1|ELECTRICAL=4|PINCONGLOMERATE=58|PINLENGTH=10|LOCATION.X=100|LOCATION.Y=500|NAME=A|DESIGNATOR=1",  // 2: left
            "|RECORD=2|OWNERINDEX=1|OWNERPARTID=1|ELECTRICAL=4|PINCONGLOMERATE=56|PINLENGTH=10|LOCATION.X=130|LOCATION.Y=500|NAME=B|DESIGNATOR=2",  // 3: right
            "|RECORD=14|OWNERINDEX=1|OWNERPARTID=1|LOCATION.X=100|LOCATION.Y=495|CORNER.X=130|CORNER.Y=505|ISSOLID=T|COLOR=128|AREACOLOR=11599871", // 4
            "|RECORD=34|OWNERINDEX=1|OWNERPARTID=-1|LOCATION.X=100|LOCATION.Y=510|TEXT=R1|NAME=Designator|FONTID=1",                               // 5
            "|RECORD=41|OWNERINDEX=1|OWNERPARTID=-1|LOCATION.X=100|LOCATION.Y=485|TEXT==Value|NAME=Comment|FONTID=1",                              // 6
            "|RECORD=41|OWNERINDEX=1|OWNERPARTID=-1|TEXT=10k|NAME=Value|ISHIDDEN=T",                                                               // 7
            "|RECORD=44|OWNERINDEX=1",                                                                                                             // 8
            "|RECORD=45|OWNERINDEX=8|MODELNAME=R0603|MODELTYPE=PCBLIB|ISCURRENT=T",                                                                // 9
            "|RECORD=2|OWNERINDEX=1|OWNERPARTID=2|PINCONGLOMERATE=58|PINLENGTH=10|LOCATION.X=300|LOCATION.Y=300|DESIGNATOR=9",                    // 10: another part's
            "|RECORD=27|LOCATIONCOUNT=2|X1=140|Y1=500|X2=200|Y2=500|UNIQUEID=WWWWWWW1",                                                            // 11
            "|RECORD=17|STYLE=4|ORIENTATION=3|SHOWNETNAME=T|LOCATION.X=200|LOCATION.Y=500|TEXT=GND|UNIQUEID=PPPPPPPP",                             // 12
            "|RECORD=27|LOCATIONCOUNT=3|X1=90|Y1=500|X2=50|Y2=500|X3=50|Y3=450|UNIQUEID=WWWWWWW2",                                                 // 13
            "|RECORD=25|LOCATION.X=50|LOCATION.Y=470|TEXT=SDA|UNIQUEID=LLLLLLLL",                                                                  // 14
            "|RECORD=15|LOCATION.X=600|LOCATION.Y=600|XSIZE=100|YSIZE=80|UNIQUEID=SSSSSSSS",                                                       // 15
            "|RECORD=32|OWNERINDEX=15|LOCATION.X=600|LOCATION.Y=605|TEXT=Power|FONTID=1",                                                          // 16
            "|RECORD=33|OWNERINDEX=15|LOCATION.X=600|LOCATION.Y=510|TEXT=Power.SchDoc|FONTID=1",                                                   // 17
            "|RECORD=16|OWNERINDEX=15|SIDE=0|DISTANCEFROMTOP=2|NAME=VIN|IOTYPE=2",                                                                 // 18
            "|RECORD=29|LOCATION.X=50|LOCATION.Y=480",                                                                                             // 19
        ])
    }
    fn find<'a>(its: &'a [Item], key: &str) -> &'a Item { its.iter().find(|i| i.key == key).unwrap_or_else(|| panic!("no {key}")) }

    #[test] fn units_and_flip() {
        let its = items(&sample()).unwrap();
        // A4 is 760 units high: a pin at (100, 500) pointing left (orientation 2) ends 10 units left of it
        let Prim::Seg { a, b, .. } = &find(&its, "a/RRRRRRRR/pin/1").prim else { panic!() };
        assert!((a.x - 25.4).abs() < 1e-3 && (a.y - 260.0 * U).abs() < 1e-3, "{a:?}");
        assert!((b.x - 90.0 * U).abs() < 1e-3 && (b.y - a.y).abs() < 1e-4, "{b:?}");
        assert!((val(&Props::parse(b"|X=3|X_FRAC=50000"), "X") - 3.5).abs() < 1e-6);
    }
    #[test] fn parts_skip_other_parts_and_hidden() {
        let its = items(&sample()).unwrap();
        assert!(!its.iter().any(|i| i.key.contains("/pin/9")), "another part's pin is not drawn");
        assert!(!its.iter().any(|i| i.key.contains("#2")), "a hidden parameter is not drawn");
        assert!(its.iter().any(|i| i.key.starts_with("a/RRRRRRRR/f/Value/")), "the comment is the value");
        assert!(its.iter().any(|i| i.key.starts_with("a/RRRRRRRR/f/Reference/") && i.layer == FIELD));
        let body = its.iter().filter(|i| i.key.starts_with("a/RRRRRRRR/g2")).map(|i| i.layer).collect::<Vec<_>>();
        assert_eq!(body, [BODY_FILL, BODY]);
        let ps = parts(&sample());
        assert_eq!(ps.len(), 1);
        assert_eq!((ps[0].refdes.as_str(), ps[0].value.as_str(), ps[0].footprint.as_str(), ps[0].lib_ref.as_str(), ps[0].description.as_str()), ("R1", "10k", "R0603", "Res", "Resistor"));
        assert!(ps[0].fields.contains(&("Value".to_string(), "10k".to_string())));
    }
    #[test] fn info_and_connectivity() {
        let its = items(&sample()).unwrap();
        let info = |k: &str| find(&its, k).info.to_string();
        assert!(info("a/RRRRRRRR/pin/2").starts_with("pin 2;name=B;type=passive;symbol=R1A;net=GND;g="), "{}", info("a/RRRRRRRR/pin/2"));
        let g = |k: &str| info(k).split(';').find_map(|f| f.strip_prefix("g=").map(String::from)).unwrap();
        assert_eq!(g("a/RRRRRRRR/pin/2"), g("a/WWWWWWW1"));
        assert_eq!(g("a/PPPPPPPP"), g("a/WWWWWWW1"));
        // the label sits on the second wire's middle and names it and pin 1
        assert!(info("a/WWWWWWW2").contains("net=SDA"));
        assert!(info("a/RRRRRRRR/pin/1").contains("net=SDA"));
        assert!(info("a/LLLLLLLL/0").starts_with("label;text=SDA;net=SDA"));
        assert_ne!(g("a/WWWWWWW2"), g("a/WWWWWWW1"));
        assert!(info("a/RRRRRRRR/g2").starts_with("symbol;ref=R1A;value=10k;lib=Res;footprint=R0603"), "{}", info("a/RRRRRRRR/g2"));
        // a record with no unique id is keyed by its kind and place
        assert!(its.iter().any(|i| i.key == "a/r29@50,480" && i.layer == JUNCTION));
        assert_eq!(nets(&sample()), ["GND", "SDA"]);
    }
    #[test] fn sheet_symbols() {
        let b = sample();
        assert_eq!(sheets(&b), [("Power".to_string(), "Power.SchDoc".to_string())]);
        let bx = sheet_boxes(&b);
        assert_eq!(bx.len(), 1);
        let (x0, y0, x1, y1) = bx[0];
        assert!((x0 - 600.0 * U).abs() < 1e-3 && (y0 - 160.0 * U).abs() < 1e-3 && (x1 - 700.0 * U).abs() < 1e-3 && (y1 - 240.0 * U).abs() < 1e-3);
        let its = items(&b).unwrap();
        let e = find(&its, "a/SSSSSSSS/pin/VIN");
        assert!(e.info.starts_with("sheet pin;name=VIN;shape=input;sheet=Power"));
        let Prim::Poly { ps, .. } = &e.prim else { panic!() };
        assert!((ps[0].x - x0).abs() < 1e-3 && (ps[0].y - (y0 + 20.0 * U) + 1.0).abs() < 1e-3, "{ps:?}");
        assert!(its.iter().any(|i| i.layer == SHEET_NAME) && its.iter().any(|i| i.layer == SHEET_FILE));
    }
    #[test] fn page_and_rejects() {
        let its = items(&sample()).unwrap();
        let Prim::Poly { ps, .. } = &find(&its, "page/0").prim else { panic!() };
        assert!((ps[2].x - 1150.0 * U).abs() < 1e-3 && (ps[2].y - 760.0 * U).abs() < 1e-3);
        assert!(items(b"(kicad_sch)").is_none());
        assert!(sheets(b"junk").is_empty() && parts(b"junk").is_empty());
    }
    #[test] fn text_markup_and_placement() {
        assert_eq!(markup("R\\E\\S\\E\\T\\"), "~{RESET}");
        assert_eq!(markup("\\C\\S\\ #"), "~{CS} #");
        assert_eq!(markup("A{1}"), "A{brl}1{brr}");
        assert_eq!(markup("EN"), "EN");
        assert_eq!(markup("A~2B~1C\tD"), "A|B\nC    D");
        assert_eq!(placed(0, 0), (false, 0, 2));
        assert_eq!(placed(1, 8), (true, 2, 0));
        assert_eq!(placed(2, 0), (false, 2, 0)); // a half turn reads the right way up, anchored on the other side
        assert_eq!(placed(3, 4), (true, 1, 1));
    }

    /// The texts a sheet draws (as written before markup), with the context given
    fn texts(b: &[u8], ctx: Ctx) -> Vec<String> {
        DRAWN.with(|d| d.borrow_mut().clear());
        items_in(b, ctx).unwrap();
        DRAWN.with(|d| std::mem::take(&mut *d.borrow_mut())).into_iter().map(|x| x.0).collect()
    }
    #[test] fn special_strings() {
        let b = schdoc(&[
            SHEET,
            "|RECORD=41|NAME=Title|TEXT=Power Stage|LOCATION.X=0|LOCATION.Y=0|ISHIDDEN=T",
            "|RECORD=41|NAME=Revision|TEXT=|ISHIDDEN=T",
            "|RECORD=41|NAME=DrawnBy|TEXT=*|ISHIDDEN=T",
            "|RECORD=41|NAME=Alias|TEXT==title|ISHIDDEN=T",
            "|RECORD=4|LOCATION.X=100|LOCATION.Y=100|TEXT==TITLE",
            "|RECORD=4|LOCATION.X=100|LOCATION.Y=120|TEXT==Revision",
            "|RECORD=4|LOCATION.X=100|LOCATION.Y=140|TEXT==DrawnBy",
            "|RECORD=4|LOCATION.X=100|LOCATION.Y=160|TEXT==NoSuchThing",
            "|RECORD=4|LOCATION.X=100|LOCATION.Y=180|TEXT==CompanyName",
            "|RECORD=4|LOCATION.X=100|LOCATION.Y=200|TEXT==DocumentName",
            "|RECORD=4|LOCATION.X=100|LOCATION.Y=220|TEXT==ProjectName",
            "|RECORD=4|LOCATION.X=100|LOCATION.Y=240|TEXT==Alias",
        ]);
        // without a context: the sheet's own parameters; an empty one and an unknown name show as written, "*" as itself
        let t = texts(&b, Ctx::default());
        for want in ["Power Stage", "=Revision", "*", "=NoSuchThing", "=CompanyName", "=DocumentName", "=ProjectName"] { assert!(t.contains(&want.to_string()), "{want} in {t:?}") }
        assert_eq!(t.iter().filter(|x| *x == "Power Stage").count(), 2, "=Alias leads on to =title: {t:?}");
        // with the project's parameters and the file's path
        let ctx = Ctx { project: vec![("CompanyName".into(), "Example Ltd".into())], project_file: "Board.PrjPcb".into(), path: "/home/user/board/Power.SchDoc".into(), ..Ctx::default() };
        let t = texts(&b, ctx);
        for want in ["Example Ltd", "Power.SchDoc", "Board.PrjPcb"] { assert!(t.contains(&want.to_string()), "{want} in {t:?}") }
        // the BOM's value leaves Altium's unset "*" out
        let p = schdoc(&[SHEET, "|RECORD=1|LIBREFERENCE=R|CURRENTPARTID=1|PARTCOUNT=2", "|RECORD=34|OWNERINDEX=1|TEXT=R9|LOCATION.X=1|LOCATION.Y=1", "|RECORD=41|OWNERINDEX=1|NAME=Comment|TEXT=*|LOCATION.X=1|LOCATION.Y=1"]);
        assert_eq!(parts(&p)[0].value, "");
        assert!(texts(&p, Ctx::default()).contains(&"*".to_string()));
    }
    #[test] fn port_cross_references_and_directives() {
        let b = schdoc(&[
            SHEET,
            "|RECORD=18|LOCATION.X=200|LOCATION.Y=390|NAME=TX|WIDTH=80|HEIGHT=10|UNIQUEID=PORT0001",
            "|RECORD=41|OWNERINDEX=1|NAME=CrossRef|TEXT=Top[3B]|JUSTIFICATION=5|LOCATION.X=198|LOCATION.Y=390|READONLYSTATE=3",
            "|RECORD=43|LOCATION.X=500|LOCATION.Y=400|NAME=FASTNETS|ORIENTATION=1|UNIQUEID=DIRCLASS",
            "|RECORD=41|OWNERINDEX=3|NAME=ClassName|TEXT=FASTNETS|ISHIDDEN=T|LOCATION.X=490|LOCATION.Y=390",
            "|RECORD=43|LOCATION.X=600|LOCATION.Y=400|NAME=DIFFPAIR|UNIQUEID=DIRDIFF1",
            "|RECORD=41|OWNERINDEX=5|NAME=DifferentialPair|TEXT=True|ISHIDDEN=T|LOCATION.X=600|LOCATION.Y=390",
        ]);
        let t = texts(&b, Ctx::default());
        assert!(t.contains(&"Top[3B]".to_string()), "a port's cross reference: {t:?}");
        assert!(t.contains(&"FASTNETS".to_string()), "a net class directive's name: {t:?}");
        assert!(!t.contains(&"DIFFPAIR".to_string()), "a differential pair directive is a mark, no text: {t:?}");
        let its = items(&b).unwrap();
        assert!(its.iter().any(|i| i.key == "a/DIRDIFF1/a"));
        // the class directive's name sits above it (it points up), centred on it
        let Prim::Circ { c, .. } = find(&its, "a/DIRCLASS/o").prim else { panic!() };
        assert!((c.x - 500.0 * U).abs() < 1e-3 && c.y < (760.0 - 400.0) * U);
    }
    #[test] fn mentions_and_channels() {
        let b = schdoc(&[
            SHEET,
            "|RECORD=1|LIBREFERENCE=R|CURRENTPARTID=1|PARTCOUNT=2|UNIQUEID=COMPAAAA|LOCATION.X=100|LOCATION.Y=100",
            "|RECORD=34|OWNERINDEX=1|TEXT=R7|LOCATION.X=100|LOCATION.Y=110",
            "|RECORD=28|LOCATION.X=300|LOCATION.Y=300|CORNER.X=500|CORNER.Y=400|TEXT=Fit @{\"Id\":\"DOCUMENT\\COMPAAAA\",\"ObjectType\":1} or @{\"Id\":\"DOCUMENT\\GONEGONE\",\"ObjectType\":1}",
        ]);
        let t = texts(&b, Ctx::default());
        assert!(t.contains(&"Fit R7 or @?".to_string()), "{t:?}");
        assert!(!t.iter().any(|x| x.contains("_CH")), "one placement: no channel hint");
        let t = texts(&b, Ctx { channels: vec!["CH1".into(), "CH2".into()], ..Ctx::default() });
        assert!(t.contains(&"(R7_CH1, ...)".to_string()), "{t:?}");
    }
    #[test] fn standard_sheet_zones_and_title_block() {
        assert_eq!(standard_zones(1150.0), (20.0, 4, 4));
        assert_eq!(standard_zones(1500.0), (20.0, 6, 4));
        assert_eq!(standard_zones(2000.0), (30.0, 6, 4));
        assert_eq!(standard_zones(3200.0), (30.0, 8, 4));
        assert_eq!(short_path("/home/user/a.SchDoc", 44), "/home/user/a.SchDoc");
        assert_eq!(short_path(r"C:\Projects\Some Long Folder Name\Another One\Board.SchDoc", 30), r"C:\Projects\..\Board.SchDoc");
        // A4 with the CUSTOM fields of another size left in the file: four zones across, as Altium draws it
        let b = schdoc(&["|RECORD=31|FONTIDCOUNT=1|SIZE1=10|SYSTEMFONT=1|BORDERON=T|SHEETSTYLE=0|CUSTOMXZONES=6|CUSTOMYZONES=4|TITLEBLOCKON=T",
            "|RECORD=41|NAME=DocumentNumber|TEXT=5|ISHIDDEN=T"]);
        let its = items(&b).unwrap();
        let segs = its.iter().filter(|i| i.key.starts_with("page/") && matches!(i.prim, Prim::Seg { a, b, .. } if (a.x - b.x).abs() < 1e-4 && a.y.abs() < 1e-4)).count();
        assert_eq!(segs, 3, "three dividers along the top: four zones");
    }
    /// The corpus' text inventory (BP_ALTIUM_CORPUS): every text-bearing record by kind, owner and what became of it (drawn,
    /// hidden, another part's, empty...), counts only. With BP_ALTIUM_OUT, each sheet's drawn texts go to `<sheet>.drawn.tsv`
    /// (text, x and y in mils from the sheet's bottom left) for comparing with what Altium shows.
    #[test] fn corpus_text_inventory() {
        let Some(root) = crate::files::altium::corpus() else { return };
        let out = std::env::var("BP_ALTIUM_OUT").ok();
        let mut files = vec![];
        fn walk(d: &std::path::Path, v: &mut Vec<std::path::PathBuf>) {
            let Ok(rd) = std::fs::read_dir(d) else { return };
            for e in rd.flatten() { let p = e.path(); if p.is_dir() { walk(&p, v) } else if p.extension().map(|x| x.eq_ignore_ascii_case("schdoc")).unwrap_or(false) { v.push(p) } }
        }
        walk(&root, &mut files);
        files.sort();
        let mut tally: std::collections::BTreeMap<String, usize> = Default::default();
        for f in &files {
            let b = std::fs::read(f).unwrap();
            DRAWN.with(|d| d.borrow_mut().clear());
            let _ = items_in(&b, Ctx::of_path(&f.to_string_lossy()));
            let dr = DRAWN.with(|d| std::mem::take(&mut *d.borrow_mut()));
            let r = load(&b).unwrap();
            let shown: HashSet<&str> = dr.iter().map(|x| x.0.as_str()).collect();
            let d = Doc2::with(&r.main, Ctx::of_path(&f.to_string_lossy()));
            let mut note = |kind: String, state: &str| *tally.entry(format!("{kind} {state}")).or_default() += 1;
            for (i, x) in r.main.iter().enumerate().chain(r.more.iter().enumerate().map(|(i, x)| (i + 1_000_000, x))) {
                let more = i >= 1_000_000;
                let rc = rec(x);
                let orc = if more { -2 } else { d.owner_rec(x) };
                let kind = format!("r{rc}{}", if orc >= 0 { format!("<r{orc}") } else if more { "(add)".into() } else { String::new() });
                let texts: Vec<(&str, String)> = match rc {
                    2 => vec![("name", x.get("NAME").to_string()), ("num", x.get("DESIGNATOR").to_string())],
                    18 | 16 | 216 | 43 | 215 => vec![("", x.get("NAME").to_string())],
                    _ if x.has("TEXT") => vec![("", x.get("TEXT").to_string())],
                    _ => continue,
                };
                for (sub, t) in texts {
                    let kind = if sub.is_empty() { kind.clone() } else { format!("{kind}.{sub}") };
                    let own = if orc >= 0 { owner(x) } else { -1 };
                    let v = if more { t.clone() } else if rc == 34 && orc == 1 { d.designator(owner(x) as usize) } else if rc == 28 || rc == 209 { d.mentions(&d.resolve(&t, -1)) } else { d.resolve(&t, own) };
                    let comp = if orc == 1 { Some(&r.main[owner(x) as usize]) } else { None };
                    let state = if t.trim().is_empty() { "empty" }
                        else if comp.map_or(false, |c| !d.shown_in(c, x)) && rc != 34 && rc != 41 { "other-part" }
                        else if x.bool("ISHIDDEN") { "hidden" }
                        else if rc == 2 && pin_hidden(x) { "hidden-pin" }
                        else if rc == 2 && sub == "name" && x.int("PINCONGLOMERATE") & 8 == 0 { "name-off" }
                        else if rc == 2 && sub == "num" && x.int("PINCONGLOMERATE") & 16 == 0 { "num-off" }
                        else if shown.contains(v.as_str()) || shown.iter().any(|s| s.ends_with(&format!(": {v}"))) { "drawn" }
                        else if t.starts_with('=') && v == t { "unresolved" }
                        else if t.eq_ignore_ascii_case("=CurrentTime") { "clock" }
                        else if rc == 41 && (44..=48).contains(&orc) { "model-param" }
                        else if rc == 43 && d.kids(i).iter().any(|&k| d.rs[k].get("NAME").eq_ignore_ascii_case("DifferentialPair")) { "diff-pair-mark" }
                        else if v.trim().is_empty() { "blank" }
                        else { "DROPPED" };
                    note(kind, state);
                }
            }
            if let Some(o) = &out {
                let name = f.strip_prefix(&root).unwrap().to_string_lossy().replace(['/', ' '], "_");
                let h = d.sh.h;
                let m = |x: f32| x / U * 10.0;
                let s: String = dr.iter().map(|(t, at, l, b)| format!("{}\t{:.0}\t{:.0}\t{l}\t{:.0}\t{:.0}\t{:.0}\t{:.0}\n", t.replace(['\t', '\n'], " "), m(at.x), h * 10.0 - m(at.y), m(b[0]), h * 10.0 - m(b[3]), m(b[2]), h * 10.0 - m(b[1]))).collect();
                let _ = std::fs::write(format!("{o}/{name}.drawn.tsv"), s);
            }
        }
        for (k, n) in &tally { println!("INV {k} {n}") }
    }
    /// Every .SchDoc of the private corpus (BP_ALTIUM_CORPUS) reads, with debug pictures in BP_ALTIUM_OUT.
    #[test] fn corpus_reads() {
        let Some(root) = crate::files::altium::corpus() else { return };
        let out = std::env::var("BP_ALTIUM_OUT").ok();
        let mut files = vec![];
        fn walk(d: &std::path::Path, v: &mut Vec<std::path::PathBuf>) {
            let Ok(rd) = std::fs::read_dir(d) else { return };
            for e in rd.flatten() { let p = e.path(); if p.is_dir() { walk(&p, v) } else if p.extension().map(|x| x.eq_ignore_ascii_case("schdoc")).unwrap_or(false) { v.push(p) } }
        }
        walk(&root, &mut files);
        files.sort();
        let (mut n, mut total, mut wired, mut all_wires) = (0, 0, 0, 0);
        for f in &files {
            let b = std::fs::read(f).unwrap();
            let t = std::time::Instant::now();
            let its = items_in(&b, Ctx::of_path(&f.to_string_lossy())).unwrap_or_else(|| panic!("unreadable {}", f.display()));
            let ms = t.elapsed().as_millis();
            assert!(its.len() > 10, "{} gave {} items", f.display(), its.len());
            let _ = (sheets(&b), parts(&b), nets(&b));
            n += 1; total += its.len();
            // connectivity: wires whose group holds a pin
            let g = |i: &Item| i.info.split(';').find_map(|f| f.strip_prefix("g=")).map(String::from);
            let pin_groups: HashSet<String> = its.iter().filter(|i| i.info.starts_with("pin ")).filter_map(g).collect();
            let wires: Vec<&Item> = its.iter().filter(|i| i.info.starts_with("wire")).collect();
            wired += wires.iter().filter(|w| g(w).map(|x| pin_groups.contains(&x)).unwrap_or(false)).count();
            all_wires += wires.len();
            if let Some(o) = &out {
                let name = f.strip_prefix(&root).unwrap().to_string_lossy().replace(['/', ' '], "_");
                crate::files::altium::debug_svg(&its, 1, &format!("{o}/{name}.svg"));
                println!("SCH {name} items={} ms={ms}", its.len());
            }
        }
        println!("SCH corpus: {n} sheets, {total} items, {wired} of {all_wires} wires reach a pin");
    }
}

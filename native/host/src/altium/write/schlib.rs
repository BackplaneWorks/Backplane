//! .SchLib: schematic symbols made from a model and added to (or replaced in) a library, and the symbols read back.
//!
//! A library is a compound file: `FileHeader` (one text record: the header, the font table and the symbol list
//! `CompCount`/`LibRefN`/`CompDescrN`/`PartCountN`), `Storage` (embedded images), `SectionKeys` (full names of symbols whose
//! storage name had to be cut to 31 characters) and one storage per symbol whose `Data` stream holds its records: the
//! component (RECORD=1), its graphics and parameters as text records, its pins as binary records (and fractional pin
//! coordinates in `PinFrac`), the designator, the comment, and the footprint links (44 implementation list, 45 a model,
//! 46 and 48 its empty map and parameter lists; 45 names its owner by record index).
//! Coordinates are in mils here; the records hold 10-mil units (`Location.X`) and 1/100000 of one (`Location.X_Frac`).
use super::{pstr, sch_bin, sch_text, section_key, Rec, Tree, Uids};
use std::collections::{HashMap, HashSet};
use std::io::Read;

/// A pin's electrical type, as Altium numbers them.
pub const ELECTRICAL: [&str; 8] = ["input", "io", "output", "open_collector", "passive", "hiz", "open_emitter", "power"];
pub fn electrical(word: &str) -> Option<u8> {
    let w = word.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    let w = match w.as_str() { "in" => "input", "out" => "output", "bidirectional" | "bidi" | "inout" | "i_o" => "io", "pwr" | "power_in" | "power_out" | "supply" | "gnd" | "ground" => "power", "oc" | "open_drain" => "open_collector", "tristate" | "tri_state" | "hi_z" => "hiz", "" => "passive", x => x };
    ELECTRICAL.iter().position(|e| *e == w).map(|i| i as u8)
}

/// A pin: `x`/`y` is where it leaves the body (Altium's `Location`), `dir` the way it points away from the body
/// (0 right, 1 up, 2 left, 3 down), `len` its length; the wire connects at the far end. Mils.
#[derive(Clone, Debug, PartialEq)]
pub struct Pin {
    pub number: String, pub name: String, pub x: f64, pub y: f64, pub dir: u8, pub len: f64, pub electrical: u8,
    pub hidden: bool, pub show_name: bool, pub show_number: bool, pub part: i64, pub description: String,
    /// IEEE symbols on the pin: inner edge (3 clock), outer edge (1 dot = inverted)
    pub inner_edge: u8, pub outer_edge: u8,
}
impl Pin {
    pub fn new(number: &str, name: &str, x: f64, y: f64, dir: u8) -> Pin {
        Pin { number: number.into(), name: name.into(), x, y, dir: dir & 3, len: 200.0, electrical: 4, hidden: false, show_name: true, show_number: true, part: 1, description: String::new(), inner_edge: 0, outer_edge: 0 }
    }
    /// The electrical end (where a wire connects).
    pub fn end(&self) -> (f64, f64) { let (dx, dy) = DIRS[(self.dir & 3) as usize]; (self.x + dx * self.len, self.y + dy * self.len) }
    fn conglomerate(&self) -> u8 { (self.dir & 3) | if self.hidden { 4 } else { 0 } | if self.show_name { 8 } else { 0 } | if self.show_number { 16 } else { 0 } }
}
pub const DIRS: [(f64, f64); 4] = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];

/// A drawn shape of a symbol (mils, Y up; `part` 0 means every part).
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Rect { x1: f64, y1: f64, x2: f64, y2: f64, filled: bool },
    Line { pts: Vec<(f64, f64)> },
    Polygon { pts: Vec<(f64, f64)>, filled: bool },
    /// angles in degrees, counter-clockwise from +X
    Arc { cx: f64, cy: f64, r: f64, a0: f64, a1: f64 },
    Ellipse { cx: f64, cy: f64, rx: f64, ry: f64, filled: bool },
    Text { x: f64, y: f64, text: String },
}

/// A footprint the symbol uses, by name, optionally from a named .PcbLib (else any library the project has).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Footprint { pub name: String, pub library: String, pub description: String }

/// A symbol to write.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Symbol {
    pub name: String, pub description: String,
    /// "U?", "R?"
    pub designator: String,
    /// the comment shown on the sheet ("*" or "=Value" or a part number)
    pub comment: String,
    /// number of parts (1 for an ordinary symbol)
    pub parts: u32,
    pub pins: Vec<Pin>, pub shapes: Vec<(Shape, i64)>, pub params: Vec<(String, String)>,
    /// the first is the current one
    pub footprints: Vec<Footprint>,
    /// where the designator and comment are drawn (mils); None: above and below the body
    pub designator_at: Option<(f64, f64)>, pub comment_at: Option<(f64, f64)>,
}

const BODY_COLOR: i64 = 128; // dark red outline
const BODY_FILL: i64 = 11599871; // Altium's pale yellow
const TEXT_COLOR: i64 = 8388608; // navy

impl Symbol {
    /// The extent of its body and pins (x0, y0, x1, y1), mils.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let mut grow = |x: f64, y: f64| { b.0 = b.0.min(x); b.1 = b.1.min(y); b.2 = b.2.max(x); b.3 = b.3.max(y); };
        for p in &self.pins { grow(p.x, p.y); let e = p.end(); grow(e.0, e.1) }
        for (s, _) in &self.shapes {
            match s {
                Shape::Rect { x1, y1, x2, y2, .. } => { grow(*x1, *y1); grow(*x2, *y2) }
                Shape::Line { pts } | Shape::Polygon { pts, .. } => pts.iter().for_each(|p| grow(p.0, p.1)),
                Shape::Arc { cx, cy, r, .. } => { grow(cx - r, cy - r); grow(cx + r, cy + r) }
                Shape::Ellipse { cx, cy, rx, ry, .. } => { grow(cx - rx, cy - ry); grow(cx + rx, cy + ry) }
                Shape::Text { x, y, .. } => grow(*x, *y),
            }
        }
        if b.0 > b.2 { (0.0, 0.0, 0.0, 0.0) } else { b }
    }

    /// The symbol's records, in Altium's order; `uids` gives each its unique id.
    pub fn records(&self, uids: &mut Uids) -> Vec<Entry> {
        let parts = self.parts.max(1) as i64;
        let pin_count = self.pins.len();
        let mut out = vec![Entry::Text(Rec::new(1).kv("LibReference", &self.name).kv_if("ComponentDescription", &self.description).kv("PartCount", parts + 1)
            .kv("DisplayModeCount", 1).kv("IndexInSheet", -1).kv("OwnerPartId", -1).kv("CurrentPartId", 1).kv("LibraryPath", "*").kv("SourceLibraryName", "*")
            .kv("SheetPartFileName", "*").kv("TargetFileName", "*").kv("UniqueID", uids.next()).kv("AreaColor", BODY_FILL).kv("Color", BODY_COLOR)
            .kv("PartIDLocked", "T").kv("AllPinCount", pin_count))];
        for (i, (k, v)) in self.params.iter().enumerate() {
            out.push(Entry::Text(Rec::new(41).nz("IndexInSheet", i as i64).kv("OwnerPartId", -1).kv("Color", TEXT_COLOR).kv("FontID", 1).kv("IsHidden", "T")
                .kv("Text", v).kv("Name", k).kv("UniqueID", uids.next())));
        }
        for (s, part) in &self.shapes {
            let head = |r: i64| Rec::new(r).kv("IsNotAccesible", "T").kv("OwnerPartId", if *part > 0 { *part } else { -1 });
            let pts = |mut r: Rec, ps: &[(f64, f64)]| { r = r.kv("LocationCount", ps.len()); for (i, p) in ps.iter().enumerate() { r = r.coord(&format!("X{}", i + 1), p.0).coord(&format!("Y{}", i + 1), p.1) } r };
            let r = match s {
                Shape::Rect { x1, y1, x2, y2, filled } => head(14).coord("Location.X", x1.min(*x2)).coord("Location.Y", y1.min(*y2)).coord("Corner.X", x1.max(*x2)).coord("Corner.Y", y1.max(*y2))
                    .kv("LineWidth", 1).kv("Color", BODY_COLOR).kv("AreaColor", BODY_FILL).flag("IsSolid", *filled),
                Shape::Line { pts: ps } => pts(head(6).kv("LineWidth", 1).kv("Color", BODY_COLOR), ps),
                Shape::Polygon { pts: ps, filled } => pts(head(7).kv("LineWidth", 1).kv("Color", BODY_COLOR).kv("AreaColor", BODY_FILL).flag("IsSolid", *filled), ps),
                Shape::Arc { cx, cy, r, a0, a1 } => head(12).coord("Location.X", *cx).coord("Location.Y", *cy).coord("Radius", *r).kv("LineWidth", 1)
                    .kv("StartAngle", format!("{a0:.3}")).kv("EndAngle", format!("{a1:.3}")).kv("Color", BODY_COLOR),
                Shape::Ellipse { cx, cy, rx, ry, filled } => head(8).coord("Location.X", *cx).coord("Location.Y", *cy).coord("Radius", *rx).coord("SecondaryRadius", *ry)
                    .kv("LineWidth", 1).kv("Color", BODY_COLOR).kv("AreaColor", BODY_FILL).flag("IsSolid", *filled),
                Shape::Text { x, y, text } => head(4).coord("Location.X", *x).coord("Location.Y", *y).kv("Color", TEXT_COLOR).kv("FontID", 1).kv("Text", text),
            };
            out.push(Entry::Text(r.kv("UniqueID", uids.next())));
        }
        for p in &self.pins { out.push(Entry::Pin(p.clone())) }
        let (x0, y0, x1, y1) = self.bounds();
        let d_at = self.designator_at.unwrap_or((x0.max(0.0), y1 + 20.0));
        // below the symbol, or beside it when pins leave the bottom (their wires go there)
        let bottom_pins = self.pins.iter().any(|p| p.dir == 3);
        let c_at = self.comment_at.unwrap_or(if bottom_pins { (x1 + 50.0, y0 + 50.0) } else { (x0.max(0.0), y0 - 80.0) });
        out.push(Entry::Text(Rec::new(34).kv("IndexInSheet", -1).kv("OwnerPartId", -1).coord("Location.X", d_at.0).coord("Location.Y", d_at.1).kv("Color", TEXT_COLOR)
            .kv("FontID", 1).kv("Text", if self.designator.is_empty() { "U?" } else { &self.designator }).kv("Name", "Designator").kv("ReadOnlyState", 1).kv("UniqueID", uids.next())));
        out.push(Entry::Text(Rec::new(41).kv("IndexInSheet", -1).kv("OwnerPartId", -1).coord("Location.X", c_at.0).coord("Location.Y", c_at.1).kv("Color", TEXT_COLOR)
            .kv("FontID", 1).kv("Text", if self.comment.is_empty() { "*" } else { &self.comment }).kv("Name", "Comment").kv("UniqueID", uids.next())));
        let list = out.len();
        out.push(Entry::Text(Rec::new(44)));
        for (i, f) in self.footprints.iter().enumerate() {
            let me = out.len();
            let mut r = Rec::new(45).kv("OwnerIndex", list).kv("IndexInSheet", -1).kv_if("Description", &f.description).kv("ModelName", &f.name).kv("ModelType", "PCBLIB").kv("DatafileCount", 1);
            if !f.library.is_empty() { r = r.kv("ModelDatafile0", &f.library) }
            r = r.kv("ModelDatafileEntity0", &f.name).kv("ModelDatafileKind0", "PCBLib").flag("IsCurrent", i == 0).kv("UniqueID", uids.next());
            out.push(Entry::Text(r));
            out.push(Entry::Text(Rec::new(46).kv("OwnerIndex", me)));
            out.push(Entry::Text(Rec::new(48).kv("OwnerIndex", me)));
        }
        out
    }
}

/// One record of a symbol: text, or a pin (binary in a library).
#[derive(Clone, Debug)]
pub enum Entry { Text(Rec), Pin(Pin) }

trait KvIf { fn kv_if(self, k: &str, v: &str) -> Self; }
impl KvIf for Rec { fn kv_if(self, k: &str, v: &str) -> Rec { if v.is_empty() { self } else { self.kv(k, v) } } }

/// A pin as a library's binary record (whole 10-mil units; fractions go to `PinFrac`).
pub fn pin_bin(p: &Pin) -> Vec<u8> {
    let mut b = 2i32.to_le_bytes().to_vec();
    b.push(0);
    b.extend((p.part.max(1) as i16).to_le_bytes());
    b.push(0); // display mode
    b.extend([p.inner_edge, p.outer_edge, 0, 0]);
    b.extend(pstr(&p.description));
    b.push(1); // formal type
    b.push(p.electrical);
    b.push(p.conglomerate());
    for v in [p.len, p.x, p.y] { b.extend((super::split_units(v).0 as i16).to_le_bytes()) }
    b.extend(0i32.to_le_bytes()); // colour
    b.extend(pstr(&p.name));
    b.extend(pstr(&p.number));
    b.extend(pstr("")); // swap id group
    b.extend(pstr("|&|"));
    b.extend(pstr("")); // default value
    b
}
/// A library's binary pin record as the text record a sheet holds (RECORD=2).
pub fn pin_rec(b: &[u8]) -> Option<Rec> {
    let mut r = super::super::Bin::new(b);
    if r.i32() != 2 { return None }
    r.skip(1);
    let part = r.u16() as i16;
    let mode = r.u8();
    let (ie, oe, ins, outs) = (r.u8(), r.u8(), r.u8(), r.u8());
    let ps = |r: &mut super::super::Bin| { let n = r.u8() as usize; super::super::cp1252(r.bytes(n)) };
    let desc = ps(&mut r);
    let (formal, elec, cong) = (r.u8(), r.u8(), r.u8());
    let (len, x, y) = (r.u16() as i16, r.u16() as i16, r.u16() as i16);
    let _color = r.i32();
    let (name, num) = (ps(&mut r), ps(&mut r));
    if r.left() > 0 { let _swap = ps(&mut r); }
    Some(Rec::new(2).kv("OwnerPartId", part).nz("OwnerPartDisplayMode", mode as i64).nz("SymbolInnerEdge", ie as i64).nz("SymbolOuterEdge", oe as i64)
        .nz("SymbolInside", ins as i64).nz("SymbolOutside", outs as i64).kv_if("Description", &desc).kv("FormalType", formal).kv("Electrical", elec)
        .kv("PinConglomerate", cong).kv("PinLength", len).nz("Location.X", x as i64).nz("Location.Y", y as i64).kv("Name", name).kv("Designator", num))
}
/// A pin from its text record (sheet or converted library pin).
pub fn pin_of(r: &Rec) -> Pin {
    let c = r.int("PinConglomerate") as u8;
    Pin { number: r.get("Designator").unwrap_or("").into(), name: r.get("Name").unwrap_or("").into(), x: r.mils("Location.X"), y: r.mils("Location.Y"), dir: c & 3,
        len: r.mils("PinLength"), electrical: r.int("Electrical") as u8, hidden: c & 4 != 0, show_name: c & 8 != 0, show_number: c & 16 != 0, part: r.int("OwnerPartId"),
        description: r.get("Description").unwrap_or("").into(), inner_edge: r.int("SymbolInnerEdge") as u8, outer_edge: r.int("SymbolOuterEdge") as u8 }
}

/// A symbol read from a library: its storage, its records (pins as text RECORD=2 records with their fractions applied).
#[derive(Clone, Debug)]
pub struct LibSym { pub name: String, pub description: String, pub key: String, pub recs: Vec<Rec> }
impl LibSym {
    pub fn pins(&self) -> Vec<Pin> { self.recs.iter().filter(|r| r.record() == 2).map(pin_of).collect() }
    /// The footprints it links to: (name, current).
    pub fn footprints(&self) -> Vec<(String, bool)> { self.recs.iter().filter(|r| r.record() == 45 && r.get("ModelType").map_or(false, |t| t.eq_ignore_ascii_case("PCBLIB"))).map(|r| (r.get("ModelName").unwrap_or("").to_string(), r.get("IsCurrent") == Some("T"))).collect() }
    pub fn designator(&self) -> String { self.recs.iter().find(|r| r.record() == 34).and_then(|r| r.get("Text")).unwrap_or("").to_string() }
    pub fn param(&self, n: &str) -> Option<&str> { self.recs.iter().find(|r| r.record() == 41 && r.get("Name").map_or(false, |x| x.eq_ignore_ascii_case(n))).and_then(|r| r.get("Text")) }
    pub fn parts(&self) -> i64 { (self.recs.first().map_or(2, |r| r.int("PartCount")) - 1).max(1) }
}

/// The library's header record and its symbols' (name, description, part count) list.
fn header(t: &Tree) -> Result<Rec, String> {
    let fh = t.get("FileHeader").ok_or("no FileHeader: not a schematic library")?;
    let recs = super::super::sch_records(fh);
    let Some(super::super::SchRec::Text(_)) = recs.first() else { return Err("the library header is unreadable".into()) };
    let n = u32::from_le_bytes([fh[0], fh[1], fh[2], 0]) as usize;
    let h = Rec::parse(&fh[4..4 + n.min(fh.len() - 4)]);
    if !h.get("HEADER").unwrap_or("").contains("Schematic Library") { return Err("not a schematic library (.SchLib)".into()) }
    Ok(h)
}
/// (symbol name, storage) from the header list and `SectionKeys`.
fn listing(t: &Tree, h: &Rec) -> Vec<(String, String)> {
    let mut keys: HashMap<String, String> = HashMap::new();
    if let Some(sk) = t.get("SectionKeys") {
        if sk.len() > 4 { let r = Rec::parse(&sk[4..]); for i in 0..r.int("KeyCount") { if let (Some(l), Some(k)) = (r.get(&format!("LibRef{i}")), r.get(&format!("SectionKey{i}"))) { keys.insert(l.to_string(), k.to_string()); } } }
    }
    (0..h.int("CompCount")).filter_map(|i| h.get(&format!("LibRef{i}")).map(|n| n.to_string())).map(|n| { let k = keys.get(&n).cloned().unwrap_or_else(|| section_key(&n, &HashSet::new())); (n, k) }).collect()
}

/// The fractional pin coordinates of a symbol (`PinFrac`): pin index -> (x, y, length) in 1/100000 of 10 mils.
fn pin_fracs(b: Option<&[u8]>) -> HashMap<usize, (i32, i32, i32)> {
    let mut out = HashMap::new();
    let Some(b) = b else { return out };
    let mut r = super::super::Bin::new(b);
    let n = r.u32() as usize; r.skip(n);
    while r.left() > 6 {
        let size = (r.u32() & 0xFFFFFF) as usize;
        let start = r.i;
        if r.u8() != 0xD0 { break }
        let n = r.u8() as usize;
        let idx: usize = super::super::cp1252(r.bytes(n)).parse().unwrap_or(usize::MAX);
        let zl = r.u32() as usize;
        let z = r.bytes(zl);
        let mut v = Vec::new();
        if flate2::read::ZlibDecoder::new(z).read_to_end(&mut v).is_ok() && v.len() >= 12 {
            let g = |i: usize| i32::from_le_bytes([v[i], v[i + 1], v[i + 2], v[i + 3]]);
            out.insert(idx, (g(0), g(4), g(8)));
        }
        r.i = start + size;
    }
    out
}

/// Every symbol of a library.
pub fn read(bytes: &[u8]) -> Result<Vec<LibSym>, String> {
    let t = Tree::read(bytes)?;
    let h = header(&t)?;
    let mut out = vec![];
    for (i, (name, key)) in listing(&t, &h).into_iter().enumerate() {
        let Some(data) = t.get(&format!("{key}/Data")) else { continue };
        let fr = pin_fracs(t.get(&format!("{key}/PinFrac")));
        let mut recs = vec![];
        let mut pin_i = 0;
        // the records raw (Props upper-cases keys; a symbol keeps Altium's spelling)
        let mut at = 0;
        while at + 4 <= data.len() {
            let hd = u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
            let (n, ty) = ((hd & 0xFFFFFF) as usize, hd >> 24);
            at += 4;
            if at + n > data.len() { break }
            let body = &data[at..at + n];
            at += n;
            if ty == 0 { recs.push(Rec::parse(body)) } else if let Some(mut p) = pin_rec(body) {
                if let Some(&(fx, fy, fl)) = fr.get(&pin_i) {
                    for (k, f) in [("Location.X", fx), ("Location.Y", fy), ("PinLength", fl)] { let m = p.mils(k) + f as f64 / 10000.0; p.set_coord(k, m) }
                }
                pin_i += 1;
                recs.push(p);
            } else { recs.push(Rec::default()) }
        }
        let description = h.get(&format!("CompDescr{i}")).map(|s| s.to_string()).or_else(|| recs.first().and_then(|r| r.get("ComponentDescription")).map(|s| s.to_string())).unwrap_or_default();
        out.push(LibSym { name, description, key, recs });
    }
    Ok(out)
}

/// The library header Altium writes for a new library.
fn new_header(uids: &mut Uids) -> Rec {
    Rec(vec![("HEADER".into(), "Protel for Windows - Schematic Library Editor Binary File Version 5.0".into())]).kv("Weight", 0).kv("MinorVersion", 9).kv("UniqueID", uids.next())
        .kv("FontIdCount", 1).kv("Size1", 10).kv("FontName1", "Times New Roman").kv("UseMBCS", "T").kv("IsBOC", "T").kv("SheetStyle", 9).kv("BorderOn", "T")
        .kv("SheetNumberSpaceSize", 12).kv("AreaColor", 16317695).kv("SnapGridOn", "T").kv("SnapGridSize", 10).kv("VisibleGridOn", "T").kv("VisibleGridSize", 10)
        .kv("CustomX", 18000).kv("CustomY", 18000).kv("UseCustomSheet", "T").kv("ReferenceZonesOn", "T").kv("Display_Unit", 0)
}

/// Adds symbols to a library (a symbol of the same name is replaced) and removes the named ones; `existing` None makes
/// a new library. The other symbols' storages are kept byte for byte. Returns the file's bytes.
pub fn write(existing: Option<&[u8]>, syms: &[Symbol], remove: &[String], seed: &str) -> Result<Vec<u8>, String> {
    let mut uids = Uids::new(seed);
    let (mut t, mut h) = match existing {
        Some(b) => { let t = Tree::read(b)?; let h = header(&t)?; (t, h) }
        None => { let mut t = Tree::default(); let h = new_header(&mut uids); t.put("FileHeader", vec![]); t.put("Storage", sch_text(&Rec(vec![("HEADER".into(), "Icon storage".into())]))); (t, h) }
    };
    for s in syms { if s.name.trim().is_empty() { return Err("a symbol needs a name".into()) } }
    // the current list: (name, storage, description, part count)
    let mut list: Vec<(String, String, String, String)> = listing(&t, &h).into_iter().enumerate()
        .map(|(i, (n, k))| (n, k, h.get(&format!("CompDescr{i}")).unwrap_or("").to_string(), h.get(&format!("PartCount{i}")).unwrap_or("2").to_string())).collect();
    let drop = |n: &str| remove.iter().any(|r| r.eq_ignore_ascii_case(n)) || syms.iter().any(|s| s.name.eq_ignore_ascii_case(n));
    for (n, k, _, _) in &list { if drop(n) { t.remove_dir(k) } }
    list.retain(|(n, ..)| !drop(n));
    super::mark_ids(&t, &mut uids);
    let mut taken: HashSet<String> = t.dirs().iter().map(|d| d.to_lowercase()).collect();
    for s in syms {
        let key = section_key(&s.name, &taken);
        taken.insert(key.to_lowercase());
        let mut data = vec![];
        let mut fracs = vec![];
        let mut pin_i = 0;
        for e in s.records(&mut uids) {
            match e {
                Entry::Text(r) => data.extend(sch_text(&r)),
                Entry::Pin(p) => {
                    data.extend(sch_bin(&pin_bin(&p)));
                    let f = [p.x, p.y, p.len].map(|v| super::split_units(v).1 as i32);
                    if f != [0, 0, 0] { fracs.push((pin_i, f)) }
                    pin_i += 1;
                }
            }
        }
        t.put(&format!("{key}/Data"), data);
        if !fracs.is_empty() { t.put(&format!("{key}/PinFrac"), pin_frac_stream(&fracs)) }
        list.push((s.name.clone(), key, s.description.clone(), (s.parts.max(1) + 1).to_string()));
    }
    // the header's list and the long names
    h.0.retain(|(k, _)| { let u = k.to_ascii_uppercase(); !(u == "COMPCOUNT" || ["LIBREF", "COMPDESCR", "PARTCOUNT"].iter().any(|p| u.strip_prefix(p).map_or(false, |r| r.chars().all(|c| c.is_ascii_digit()) && !r.is_empty()))) });
    h.set("CompCount", list.len());
    for (i, (n, _, d, pc)) in list.iter().enumerate() {
        h.0.push((format!("LibRef{i}"), n.clone()));
        if !d.is_empty() { h.0.push((format!("CompDescr{i}"), d.clone())) }
        h.0.push((format!("PartCount{i}"), pc.clone()));
    }
    let weight: usize = list.iter().map(|(_, k, ..)| t.get(&format!("{k}/Data")).map_or(0, |d| super::super::sch_records(d).len())).sum();
    h.set("Weight", weight);
    t.put("FileHeader", sch_text(&h));
    let long: Vec<&(String, String, String, String)> = list.iter().filter(|(n, k, ..)| n != k || n.chars().count() >= 31).collect();
    t.streams.retain(|(k, _)| k != "SectionKeys");
    if !long.is_empty() {
        let mut r = Rec(vec![("KeyCount".into(), long.len().to_string())]);
        for (i, (n, k, ..)) in long.iter().enumerate() { r = r.kv(&format!("LibRef{i}"), n).kv(&format!("SectionKey{i}"), k) }
        t.put("SectionKeys", sch_text(&r));
    }
    // FileHeader first, then the symbols, as Altium orders them
    t.streams.sort_by_key(|(k, _)| match k.as_str() { "FileHeader" => 0, "Storage" => 2, "SectionKeys" => 3, _ => 1 });
    t.build()
}

/// `PinFrac`: a header, then per pin a zlib block of its three fractions.
fn pin_frac_stream(fr: &[(usize, [i32; 3])]) -> Vec<u8> {
    use std::io::Write;
    let mut o = sch_text(&Rec(vec![("HEADER".into(), "PinFrac".into()), ("Weight".into(), fr.len().to_string())]));
    for (i, f) in fr {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        for v in f { let _ = z.write_all(&v.to_le_bytes()); }
        let z = z.finish().unwrap_or_default();
        let name = pstr(&i.to_string());
        let mut body = vec![0xD0];
        body.extend(name);
        body.extend((z.len() as u32).to_le_bytes());
        body.extend(z);
        o.extend(((body.len() as u32) | 0x0100_0000).to_le_bytes());
        o.extend(body);
    }
    o
}

// Layout ----------------------------------------------------------------------------------------------------------------

/// Pins placed around a box from their sides: `left`/`right` top to bottom, `top`/`bottom` left to right, 100 mil apart,
/// with a gap (`None`) where the spec asks for one. The box is sized to the names; the first left pin's end is at 0,0.
pub fn box_layout(sides: [Vec<Option<Pin>>; 4], pin_len: f64) -> (Vec<Pin>, Shape) {
    let [left, right, top, bottom] = sides;
    let g = 100.0;
    let width_of = |ps: &[Option<Pin>]| ps.iter().flatten().map(|p| if p.show_name { p.name.chars().count() } else { 0 }).max().unwrap_or(0) as f64 * 60.0;
    let snap = |v: f64| (v / g).ceil() * g;
    let rows = left.len().max(right.len()).max(1) as f64;
    let cols = top.len().max(bottom.len()) as f64;
    // across: the left names, the top/bottom pins in a band of their own (their names stand upright), the right names
    let (lw, rw) = (snap(width_of(&left) + 40.0), snap(width_of(&right) + 40.0));
    let band = if cols > 0.0 { (cols + 1.0) * g } else { g };
    let w = (lw + band + rw).max(400.0);
    // down: the side rows, and the top and bottom pins' upright names clear of each other
    let tall = if cols > 0.0 { snap(width_of(&top) + 40.0) + snap(width_of(&bottom) + 40.0) + g } else { 0.0 };
    let h = ((rows + 1.0) * g).max(tall).max(200.0);
    // the box's top-left corner: one grid step above the first left pin row
    let (bx, by) = (pin_len, g);
    // the top/bottom band starts after the left names, centred in what is left
    let x0 = bx + lw + snap(((w - lw - rw) - band) / 2.0).max(0.0);
    let mut out = vec![];
    let mut put = |ps: Vec<Option<Pin>>, f: &dyn Fn(usize) -> (f64, f64, u8)| for (i, p) in ps.into_iter().enumerate() { if let Some(mut p) = p { let (x, y, d) = f(i); p.x = x; p.y = y; p.dir = d; p.len = pin_len; out.push(p) } };
    put(left, &|i| (bx, by - g * (i as f64 + 1.0), 2));
    put(right, &|i| (bx + w, by - g * (i as f64 + 1.0), 0));
    put(top, &|i| (x0 + g * (i as f64 + 1.0), by, 1));
    put(bottom, &|i| (x0 + g * (i as f64 + 1.0), by - h, 3));
    (out, Shape::Rect { x1: bx, y1: by - h, x2: bx + w, y2: by, filled: true })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Symbol {
        let side = |v: &[(&str, &str, &str)]| v.iter().map(|(n, nm, e)| { let mut p = Pin::new(n, nm, 0.0, 0.0, 0); p.electrical = electrical(e).unwrap(); Some(p) }).collect::<Vec<_>>();
        let (pins, body) = box_layout([side(&[("1", "VIN", "power"), ("3", "EN", "input")]), side(&[("5", "VOUT", "power")]), vec![], side(&[("2", "GND", "power")])], 200.0);
        Symbol { name: "LDO_TEST".into(), description: "Test regulator".into(), designator: "U?".into(), comment: "LDO_TEST".into(), parts: 1, pins, shapes: vec![(body, 1)],
            params: vec![("Manufacturer".into(), "Acme".into())], footprints: vec![Footprint { name: "SOT-23-5".into(), ..Default::default() }], ..Default::default() }
    }
    #[test] fn layout() {
        let s = sample();
        let vin = s.pins.iter().find(|p| p.name == "VIN").unwrap();
        assert_eq!(vin.end(), (0.0, 0.0));
        assert_eq!(vin.dir, 2);
        let gnd = s.pins.iter().find(|p| p.name == "GND").unwrap();
        assert_eq!(gnd.dir, 3);
        assert!(s.pins.iter().all(|p| (p.end().0 % 100.0).abs() < 1e-9 && (p.end().1 % 100.0).abs() < 1e-9), "pin ends on the 100 mil grid");
    }
    #[test] fn binary_pin_round_trip() {
        let mut p = Pin::new("12", "SDA", 300.0, -200.0, 0);
        p.electrical = 1; p.outer_edge = 1; p.part = 2;
        let r = pin_rec(&pin_bin(&p)).unwrap();
        let q = pin_of(&r);
        assert_eq!((q.number.as_str(), q.name.as_str(), q.x, q.y, q.dir, q.len, q.electrical, q.part, q.outer_edge), ("12", "SDA", 300.0, -200.0, 0, 200.0, 1, 2, 1));
    }
    #[test] fn library_round_trip_and_edit() {
        let s = sample();
        let b = write(None, &[s.clone()], &[], "t").unwrap();
        // our readers: the library form and the drawing path
        let syms = read(&b).unwrap();
        assert_eq!(syms.len(), 1);
        let l = &syms[0];
        assert_eq!(l.name, "LDO_TEST"); assert_eq!(l.description, "Test regulator"); assert_eq!(l.designator(), "U?");
        assert_eq!(l.footprints(), vec![("SOT-23-5".to_string(), true)]);
        assert_eq!(l.param("Manufacturer"), Some("Acme"));
        let mut a = l.pins(); a.sort_by(|x, y| x.number.cmp(&y.number));
        let mut e = s.pins.clone(); e.sort_by(|x, y| x.number.cmp(&y.number));
        assert_eq!(a, e);
        // the footprint link names its owner by record index
        let i44 = l.recs.iter().position(|r| r.record() == 44).unwrap();
        assert_eq!(l.recs.iter().find(|r| r.record() == 45).unwrap().int("OwnerIndex") as usize, i44);
        // a second symbol with a long name; the first replaced; then one removed
        let mut long = sample(); long.name = "A_VERY_LONG_SYMBOL_NAME_OVER_31_CHARACTERS".into();
        let mut s2 = s.clone(); s2.description = "Changed".into();
        let b2 = write(Some(&b), &[long.clone(), s2], &[], "t2").unwrap();
        let syms = read(&b2).unwrap();
        assert_eq!(syms.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec![long.name.as_str(), "LDO_TEST"]);
        assert_eq!(syms[1].description, "Changed");
        assert!(syms[0].key.len() <= 31);
        let b3 = write(Some(&b2), &[], &["ldo_test".into()], "t3").unwrap();
        let syms = read(&b3).unwrap();
        assert_eq!(syms.len(), 1);
        assert!(Tree::read(&b3).unwrap().dirs().len() == 1);
    }
    #[test] fn fractional_pins() {
        let mut s = sample();
        s.pins[0].x += 5.0;
        let b = write(None, &[s.clone()], &[], "f").unwrap();
        let p = read(&b).unwrap()[0].pins().into_iter().find(|p| p.number == s.pins[0].number).unwrap();
        assert!((p.x - s.pins[0].x).abs() < 1e-6);
    }
}

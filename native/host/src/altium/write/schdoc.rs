//! .SchDoc: a schematic sheet made or edited without Altium: parts placed from a library symbol, wires, net labels, power
//! ports, junctions, no-ERC marks, pins hooked to nets, parts removed.
//!
//! `FileHeader` holds the header record, then the sheet's records indexed from 0 (0 is the sheet, RECORD=31); a child names
//! its owner by `OwnerIndex`. A placed part is the library symbol's records copied with their coordinates turned, mirrored
//! and moved onto the sheet (a sheet stores them placed), pins as text records, its footprint links' owner indices
//! shifted to the part's place. Everything else in the file is kept as it was. Coordinates are mils, Y up.
use super::schlib::{pin_of, LibSym, DIRS};
use super::{sch_bin, sch_text, Rec, Tree, Uids};

/// A sheet record: text, or a binary one kept as it is.
#[derive(Clone, Debug)]
pub enum Raw { Text(Rec), Bin(Vec<u8>) }
impl Raw { pub fn rec(&self) -> Option<&Rec> { match self { Raw::Text(r) => Some(r), Raw::Bin(_) => None } } }

/// The sheet sizes by name, as `SheetStyle` numbers them.
pub const SIZES: [&str; 18] = ["A4", "A3", "A2", "A1", "A0", "A", "B", "C", "D", "E", "Letter", "Legal", "Tabloid", "OrCAD A", "OrCAD B", "OrCAD C", "OrCAD D", "OrCAD E"];
/// Power port styles by name.
pub const POWER_STYLES: [&str; 11] = ["circle", "arrow", "bar", "wave", "ground", "signal_ground", "earth", "gost_arrow", "gost_ground", "gost_earth", "gost_bar"];

pub struct Sheet { pub tree: Tree, pub header: Rec, pub recs: Vec<Raw>, pub uids: Uids }

impl Sheet {
    /// A new, empty sheet of a standard size.
    pub fn new(size: &str, seed: &str) -> Result<Sheet, String> {
        let style = SIZES.iter().position(|s| s.eq_ignore_ascii_case(size.trim())).ok_or_else(|| format!("unknown sheet size {size:?}: one of {}", SIZES.join(", ")))?;
        let mut uids = Uids::new(seed);
        let header = Rec(vec![("HEADER".into(), "Protel for Windows - Schematic Capture Binary File Version 5.0".into())]).kv("Weight", 1).kv("MinorVersion", 9).kv("UniqueID", uids.next());
        let sheet = Rec::new(31).kv("FontIdCount", 1).kv("Size1", 10).kv("FontName1", "Times New Roman").kv("UseMBCS", "T").kv("IsBOC", "T").kv("HotSpotGridOn", "T")
            .kv("HotSpotGridSize", 4).kv("SystemFont", 1).kv("BorderOn", "T").kv("TitleBlockOn", "T").kv("SheetNumberSpaceSize", 12).kv("AreaColor", 16317695)
            .kv("SnapGridOn", "T").kv("SnapGridSize", 10).kv("VisibleGridOn", "T").kv("VisibleGridSize", 10).nz("SheetStyle", style as i64).kv("Display_Unit", 4);
        let mut tree = Tree::default();
        tree.put("FileHeader", vec![]);
        tree.put("Storage", sch_text(&Rec(vec![("HEADER".into(), "Icon storage".into())])));
        Ok(Sheet { tree, header, recs: vec![Raw::Text(sheet)], uids })
    }
    pub fn read(bytes: &[u8], seed: &str) -> Result<Sheet, String> {
        let tree = Tree::read(bytes)?;
        let fh = tree.get("FileHeader").ok_or("no FileHeader: not a schematic sheet")?;
        let mut all = vec![];
        let mut at = 0;
        while at + 4 <= fh.len() {
            let h = u32::from_le_bytes([fh[at], fh[at + 1], fh[at + 2], fh[at + 3]]);
            let (n, ty) = ((h & 0xFFFFFF) as usize, h >> 24);
            at += 4;
            if at + n > fh.len() { return Err("the sheet's records are cut short".into()) }
            let b = &fh[at..at + n];
            at += n;
            all.push(if ty == 0 { Raw::Text(Rec::parse(b)) } else { Raw::Bin(b.to_vec()) });
        }
        let mut it = all.into_iter();
        let header = match it.next() { Some(Raw::Text(h)) if h.get("HEADER").unwrap_or("").contains("Schematic Capture") => h, _ => return Err("not a schematic sheet (.SchDoc)".into()) };
        let mut uids = Uids::new(seed);
        super::mark_ids(&tree, &mut uids);
        Ok(Sheet { tree, header, recs: it.collect(), uids })
    }
    pub fn bytes(&mut self) -> Result<Vec<u8>, String> {
        self.header.set("Weight", self.recs.len());
        let mut fh = sch_text(&self.header);
        for r in &self.recs { fh.extend(match r { Raw::Text(r) => sch_text(r), Raw::Bin(b) => sch_bin(b) }) }
        self.tree.put("FileHeader", fh);
        self.tree.build()
    }
    fn push(&mut self, r: Rec) -> usize { self.recs.push(Raw::Text(r)); self.recs.len() - 1 }
    fn text(&self, i: usize) -> Option<&Rec> { self.recs.get(i).and_then(Raw::rec) }

    // reading ---------------------------------------------------------------------------------------------------------------

    /// The components: (index, designator).
    pub fn components(&self) -> Vec<(usize, String)> {
        let mut out = vec![];
        for (i, r) in self.recs.iter().enumerate() {
            if r.rec().map_or(true, |r| r.record() != 1) { continue }
            let d = self.recs.iter().filter_map(Raw::rec).find(|c| c.record() == 34 && c.int("OwnerIndex") as usize == i).and_then(|c| c.get("Text")).unwrap_or("").to_string();
            out.push((i, d));
        }
        out
    }
    pub fn find(&self, designator: &str) -> Option<usize> { self.components().into_iter().find(|(_, d)| d.eq_ignore_ascii_case(designator)).map(|(i, _)| i) }
    /// A placed pin's electrical end and the way it points from the body: by designator and pin number (or name).
    pub fn pin_end(&self, designator: &str, pin: &str) -> Result<((f64, f64), u8), String> {
        // every placed part of that designator (a multi-part symbol's parts each show their own pins)
        let comps: Vec<(usize, i64)> = self.components().into_iter().filter(|(_, d)| d.eq_ignore_ascii_case(designator)).map(|(i, _)| (i, self.text(i).map_or(1, |r| r.int("CurrentPartId").max(1)))).collect();
        if comps.is_empty() { return Err(format!("no part {designator} on the sheet")) }
        let pins: Vec<&Rec> = self.recs.iter().filter_map(Raw::rec).filter(|r| r.record() == 2 && comps.iter().any(|(c, part)| r.int("OwnerIndex") as usize == *c && { let p = r.int("OwnerPartId"); p <= 0 || p == *part })).collect();
        let hit = pins.iter().find(|r| r.get("Designator") == Some(pin)).or_else(|| pins.iter().find(|r| r.get("Name").map_or(false, |n| n.eq_ignore_ascii_case(pin))))
            .ok_or_else(|| format!("{designator} has no pin {pin} on the placed parts (pins: {})", pins.iter().map(|r| r.get("Designator").unwrap_or("?")).collect::<Vec<_>>().join(", ")))?;
        let p = pin_of(hit);
        Ok((p.end(), p.dir))
    }

    // editing ---------------------------------------------------------------------------------------------------------------

    /// Places a library symbol: its origin at `at`, turned `rot` quarter turns counter-clockwise, mirrored left-right
    /// first when `mirror`, showing part `part` (1 for a single-part symbol). Returns the component's index.
    pub fn place(&mut self, sym: &LibSym, library: &str, designator: &str, at: (f64, f64), rot: u8, mirror: bool, part: i64) -> Result<usize, String> {
        if designator.trim().is_empty() || designator.contains('?') { return Err(format!("{}: give the part a designator (U1, R3), not {designator:?}", sym.name)) }
        // another part of a multi-part symbol shares the designator (U2A, U2B); anything else may not
        for (i, _) in self.components().into_iter().filter(|(_, d)| d.eq_ignore_ascii_case(designator)) {
            let r = self.text(i).cloned().unwrap_or_default();
            let same = r.get("LibReference").map_or(false, |l| l.eq_ignore_ascii_case(&sym.name)) && r.int("CurrentPartId").max(1) != part.max(1);
            if !same { return Err(format!("{designator} is already on the sheet{}", if sym.parts() > 1 { " (another part of it takes a different `part`)" } else { "" })) }
        }
        let rot = rot & 3;
        let tf = |x: f64, y: f64| { let x = if mirror { -x } else { x }; let (x, y) = match rot { 1 => (-y, x), 2 => (-x, -y), 3 => (y, -x), _ => (x, y) }; (at.0 + x, at.1 + y) };
        let turn_dir = |d: u8| { let d = if mirror && d % 2 == 0 { 2 - d } else { d }; (d + rot) & 3 };
        let base = self.recs.len();
        let mut comp = sym.recs.first().cloned().filter(|r| r.record() == 1).ok_or_else(|| format!("{}: the library symbol has no component record", sym.name))?;
        comp.del("IndexInSheet");
        comp = comp.coord("Location.X", at.0).coord("Location.Y", at.1);
        comp.del("Orientation"); comp.del("IsMirrored");
        if rot != 0 { comp.set("Orientation", rot) }
        if mirror { comp.set("IsMirrored", "T") }
        comp.set("CurrentPartId", part.max(1));
        comp.set("SourceLibraryName", library);
        comp.set("DesignItemId", &sym.name);
        comp.set("UniqueID", self.uids.next());
        self.push(comp);
        let mut has_comment = false;
        for r in sym.recs.iter().skip(1) {
            let mut r = r.clone();
            let own = r.get("OwnerIndex").and_then(|v| v.parse::<usize>().ok());
            r.del("OwnerIndex");
            r.0.insert(1, ("OwnerIndex".into(), (base + own.unwrap_or(0)).to_string()));
            if r.record() < 0 { self.push(r); continue }
            // points (Altium leaves zero coordinates out, so a record of a placed kind is moved even without them)
            let rk = r.record();
            if matches!(rk, 2 | 4 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 30 | 34 | 41) || r.get("Location.X").is_some() || r.get("Location.Y").is_some() {
                let (x, y) = tf(r.mils("Location.X"), r.mils("Location.Y")); r.set_coord("Location.X", x); r.set_coord("Location.Y", y);
            }
            if matches!(rk, 10 | 13 | 14 | 30) { let (x, y) = tf(r.mils("Corner.X"), r.mils("Corner.Y")); r.set_coord("Corner.X", x); r.set_coord("Corner.Y", y); }
            for i in 1..=r.int("LocationCount") { let (kx, ky) = (format!("X{i}"), format!("Y{i}")); let (x, y) = tf(r.mils(&kx), r.mils(&ky)); r.set_coord(&kx, x); r.set_coord(&ky, y) }
            match r.record() {
                // boxes stay corner-ordered
                14 | 10 => {
                    let (a, b, c, d) = (r.mils("Location.X"), r.mils("Location.Y"), r.mils("Corner.X"), r.mils("Corner.Y"));
                    r.set_coord("Location.X", a.min(c)); r.set_coord("Location.Y", b.min(d)); r.set_coord("Corner.X", a.max(c)); r.set_coord("Corner.Y", b.max(d));
                }
                2 => { let c = r.int("PinConglomerate") as u8; r.set("PinConglomerate", (c & !3) | turn_dir(c & 3)) }
                12 => {
                    let ang = |k: &str, d: f64| r.get(k).and_then(|v| v.trim().parse::<f64>().ok()).unwrap_or(d);
                    let (a0, a1) = (ang("StartAngle", 0.0), ang("EndAngle", 360.0));
                    let sweep = { let s = (a1 - a0).rem_euclid(360.0); if s == 0.0 { 360.0 } else { s } };
                    let start = if mirror { 180.0 - a0 - sweep } else { a0 } + rot as f64 * 90.0;
                    let start = start.rem_euclid(360.0);
                    let end = if sweep >= 360.0 { start + 360.0 } else { (start + sweep).rem_euclid(360.0) };
                    r.set("StartAngle", format!("{start:.3}")); r.set("EndAngle", format!("{end:.3}"));
                }
                8 if rot % 2 == 1 => { let (a, b) = (r.mils("Radius"), r.mils("SecondaryRadius")); r.set_coord("Radius", b); r.set_coord("SecondaryRadius", a) }
                4 | 34 | 41 if rot != 0 => { let o = (r.int("Orientation") + rot as i64).rem_euclid(4); r.del("Orientation"); if o != 0 { r.set("Orientation", o) } }
                _ => {}
            }
            if r.record() == 34 { r.set("Text", designator) }
            if r.record() == 41 && r.get("Name") == Some("Comment") { has_comment = true }
            if r.get("UniqueID").is_some() || matches!(r.record(), 2 | 34 | 41 | 45) { r.set("UniqueID", self.uids.next()) }
            self.push(r);
        }
        if !has_comment { let id = self.uids.next(); self.push(Rec::new(41).kv("OwnerIndex", base).kv("OwnerPartId", -1).coord("Location.X", at.0).coord("Location.Y", at.1 - 100.0).kv("Color", 8388608).kv("FontID", 1).kv("Text", "*").kv("Name", "Comment").kv("UniqueID", id)); }
        Ok(base)
    }
    /// Sets a placed part's parameter (shown or hidden), adding it when it has none; "Comment" sets the comment.
    pub fn set_param(&mut self, designator: &str, name: &str, value: &str, show: Option<bool>) -> Result<(), String> {
        let c = self.find(designator).ok_or_else(|| format!("no part {designator} on the sheet"))?;
        let at = self.text(c).map(|r| (r.mils("Location.X"), r.mils("Location.Y"))).unwrap_or((0.0, 0.0));
        let hit = self.recs.iter().position(|r| r.rec().map_or(false, |r| r.record() == 41 && r.int("OwnerIndex") as usize == c && r.get("Name").map_or(false, |n| n.eq_ignore_ascii_case(name))));
        match hit {
            Some(i) => if let Raw::Text(r) = &mut self.recs[i] { r.set("Text", value); match show { Some(true) => r.del("IsHidden"), Some(false) => r.set("IsHidden", "T"), None => {} } },
            None => {
                let id = self.uids.next();
                let r = Rec::new(41).kv("OwnerIndex", c).kv("OwnerPartId", -1).coord("Location.X", at.0).coord("Location.Y", at.1).kv("Color", 8388608).kv("FontID", 1).flag("IsHidden", !show.unwrap_or(false)).kv("Text", value).kv("Name", name).kv("UniqueID", id);
                // after the part's last record, so its children stay together
                let last = (c..self.recs.len()).take_while(|&i| i == c || self.recs[i].rec().map_or(true, |r| r.int("OwnerIndex") as usize >= c && r.get("OwnerIndex").is_some())).last().unwrap_or(c);
                self.insert(last + 1, r);
            }
        }
        Ok(())
    }
    /// Inserts a record, shifting every owner index at or after it.
    fn insert(&mut self, at: usize, r: Rec) {
        for x in self.recs.iter_mut() { if let Raw::Text(x) = x { if let Some(o) = x.get("OwnerIndex").and_then(|v| v.parse::<usize>().ok()) { if o >= at { x.set("OwnerIndex", o + 1) } } } }
        self.recs.insert(at, Raw::Text(r));
    }
    /// Removes a part and everything it owns; owner indices after it close up.
    pub fn remove_part(&mut self, designator: &str) -> Result<(), String> {
        let cs: Vec<usize> = self.components().into_iter().filter(|(_, d)| d.eq_ignore_ascii_case(designator)).map(|(i, _)| i).collect();
        if cs.is_empty() { return Err(format!("no part {designator} on the sheet")) }
        let mut gone = vec![false; self.recs.len()];
        let c = cs[0];
        for &i in &cs { gone[i] = true }
        // owners come before what they own
        for i in c + 1..self.recs.len() { if let Some(o) = self.recs[i].rec().and_then(|r| r.get("OwnerIndex")).and_then(|v| v.parse::<usize>().ok()) { if o < gone.len() && gone[o] { gone[i] = true } } }
        self.remove_where(&gone);
        Ok(())
    }
    fn remove_where(&mut self, gone: &[bool]) {
        let mut map = vec![0usize; gone.len()];
        let mut n = 0;
        for i in 0..gone.len() { map[i] = n; if !gone[i] { n += 1 } }
        let old = std::mem::take(&mut self.recs);
        for (i, mut r) in old.into_iter().enumerate() {
            if gone[i] { continue }
            if let Raw::Text(x) = &mut r { if let Some(o) = x.get("OwnerIndex").and_then(|v| v.parse::<usize>().ok()) { if o < map.len() { x.set("OwnerIndex", map[o]) } } }
            self.recs.push(r);
        }
    }
    /// Removes the wires, net labels, power ports, junctions and no-ERC marks matching: a label/port's text, or objects
    /// touching a point.
    pub fn remove_objects(&mut self, text: Option<&str>, at: Option<(f64, f64)>) -> usize {
        let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5;
        let gone: Vec<bool> = self.recs.iter().map(|r| {
            let Some(r) = r.rec() else { return false };
            if r.get("OwnerIndex").is_some() { return false }
            let rk = r.record();
            if !matches!(rk, 17 | 25 | 27 | 29 | 22) { return false }
            let hit_text = text.map_or(false, |t| matches!(rk, 17 | 25) && r.get("Text").map_or(false, |x| x.eq_ignore_ascii_case(t)));
            let hit_at = at.map_or(false, |p| if rk == 27 { (1..=r.int("LocationCount")).any(|i| near((r.mils(&format!("X{i}")), r.mils(&format!("Y{i}"))), p)) } else { near((r.mils("Location.X"), r.mils("Location.Y")), p) });
            hit_text || hit_at
        }).collect();
        let n = gone.iter().filter(|g| **g).count();
        self.remove_where(&gone);
        n
    }
    pub fn wire(&mut self, pts: &[(f64, f64)]) -> Result<usize, String> {
        if pts.len() < 2 { return Err("a wire needs two points or more".into()) }
        let mut r = Rec::new(27).kv("OwnerPartId", -1).kv("LineWidth", 1).kv("Color", 8388608).kv("UniqueID", self.uids.next()).kv("LocationCount", pts.len());
        for (i, p) in pts.iter().enumerate() { r = r.coord(&format!("X{}", i + 1), p.0).coord(&format!("Y{}", i + 1), p.1) }
        Ok(self.push(r))
    }
    /// A net label; `rot` quarter turns.
    pub fn label(&mut self, text: &str, at: (f64, f64), rot: u8) -> usize {
        let r = Rec::new(25).kv("OwnerPartId", -1).coord("Location.X", at.0).coord("Location.Y", at.1).nz("Orientation", (rot & 3) as i64).kv("Color", 128).kv("FontID", 1).kv("Text", text).kv("UniqueID", self.uids.next());
        self.push(r)
    }
    /// A power port: `style` one of POWER_STYLES, pointing `rot` (0 right, 1 up, 2 left, 3 down).
    pub fn power(&mut self, net: &str, style: &str, at: (f64, f64), rot: u8) -> Result<usize, String> {
        let s = POWER_STYLES.iter().position(|x| x.eq_ignore_ascii_case(style.trim())).ok_or_else(|| format!("unknown power port style {style:?}: one of {}", POWER_STYLES.join(", ")))?;
        let r = Rec::new(17).kv("OwnerPartId", -1).nz("Style", s as i64).kv("ShowNetName", "T").coord("Location.X", at.0).coord("Location.Y", at.1).nz("Orientation", (rot & 3) as i64)
            .kv("Color", 128).kv("FontID", 1).kv("Text", net).kv("UniqueID", self.uids.next());
        Ok(self.push(r))
    }
    pub fn junction(&mut self, at: (f64, f64)) -> usize { self.push(Rec::new(29).kv("IndexInSheet", -1).kv("OwnerPartId", -1).coord("Location.X", at.0).coord("Location.Y", at.1).kv("Color", 128)) }
    pub fn no_erc(&mut self, at: (f64, f64)) -> usize { let id = self.uids.next(); self.push(Rec::new(22).kv("OwnerPartId", -1).coord("Location.X", at.0).coord("Location.Y", at.1).kv("Color", 128).kv("UniqueID", id)) }
    /// Hooks a pin to a net: a short wire out from the pin's end, and at its end a net label (`power` None) or a power
    /// port of that style. A net named "NC" gets a no-ERC mark on the pin instead.
    pub fn connect(&mut self, designator: &str, pin: &str, net: &str, power: Option<&str>, stub: f64) -> Result<(), String> {
        let (end, dir) = self.pin_end(designator, pin)?;
        if net.eq_ignore_ascii_case("NC") { self.no_erc(end); return Ok(()) }
        let (dx, dy) = DIRS[dir as usize];
        // a label lies along its wire, so the wire is at least as long as the label's text
        let stub = if power.is_none() { stub.max(((net.chars().count() as f64 * 60.0 + 40.0) / 100.0).ceil() * 100.0) } else { stub };
        let tip = (end.0 + dx * stub, end.1 + dy * stub);
        self.wire(&[end, tip])?;
        match power {
            Some(style) => {
                // a ground hangs down and a supply stands up, unless that would fold back over the pin's own wire
                let ground = style.to_ascii_lowercase().contains("ground") || style.to_ascii_lowercase().contains("earth");
                self.power(net, style, tip, if ground { if dir == 1 { 1 } else { 3 } } else if dir == 3 { 3 } else { 1 })?;
            }
            // the label reads along the wire from its left (or lower) end
            None => { self.label(net, (tip.0.min(end.0), tip.1.min(end.1)), if dir % 2 == 1 { 1 } else { 0 }); }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::altium::write::schlib::{self, box_layout, Pin, Symbol};
    pub fn lib() -> Vec<u8> {
        let side = |v: &[(&str, &str)]| v.iter().map(|(n, nm)| Some(Pin::new(n, nm, 0.0, 0.0, 0))).collect::<Vec<_>>();
        let (pins, body) = box_layout([side(&[("1", "VIN"), ("3", "EN")]), side(&[("5", "VOUT")]), vec![], side(&[("2", "GND")])], 200.0);
        let s = Symbol { name: "LDO".into(), designator: "U?".into(), comment: "LDO".into(), parts: 1, pins, shapes: vec![(body, 1), (schlib::Shape::Arc { cx: 300.0, cy: 0.0, r: 20.0, a0: 0.0, a1: 90.0 }, 1), (schlib::Shape::Line { pts: vec![(250.0, 0.0), (300.0, 50.0)] }, 1)],
            footprints: vec![schlib::Footprint { name: "SOT23-5".into(), ..Default::default() }], ..Default::default() };
        schlib::write(None, &[s], &[], "lib").unwrap()
    }
    #[test] fn sheet_round_trip() {
        let syms = schlib::read(&lib()).unwrap();
        let mut sh = Sheet::new("A4", "s").unwrap();
        sh.place(&syms[0], "parts.SchLib", "U1", (1000.0, 3000.0), 0, false, 1).unwrap();
        sh.place(&syms[0], "parts.SchLib", "U2", (3000.0, 3000.0), 1, true, 1).unwrap();
        assert!(sh.place(&syms[0], "parts.SchLib", "U1", (0.0, 0.0), 0, false, 1).is_err());
        assert_eq!(sh.pin_end("U1", "1").unwrap(), ((1000.0, 3000.0), 2));
        // turned a quarter and mirrored: the left pin points right first, then up... (mirror, then turn)
        let (e, d) = sh.pin_end("U2", "VIN").unwrap();
        assert_eq!((e, d), ((3000.0, 3000.0), 1));
        // every drawn point moved onto the sheet (both parts sit at x >= 1000)
        for r in sh.recs.iter().filter_map(Raw::rec).filter(|r| matches!(r.record(), 6 | 12 | 14 | 2)) {
            let x = if r.record() == 6 { r.mils("X1") } else { r.mils("Location.X") };
            assert!(x >= 700.0, "{r:?}");
        }
        sh.connect("U1", "1", "VBAT", None, 100.0).unwrap();
        sh.connect("U1", "GND", "GND", Some("ground"), 100.0).unwrap();
        sh.connect("U1", "EN", "VBAT", None, 100.0).unwrap();
        sh.connect("U1", "5", "NC", None, 100.0).unwrap();
        sh.set_param("U1", "Value", "3.3V", Some(true)).unwrap();
        sh.set_param("U1", "Comment", "TLV75533", None).unwrap();
        let b = sh.bytes().unwrap();
        // our reader: parts with designators and footprints, nets by their labels and ports
        let parts = crate::files::altium::sch::parts(&b);
        assert_eq!(parts.iter().map(|p| (p.refdes.as_str(), p.footprint.as_str())).collect::<Vec<_>>(), vec![("U1", "SOT23-5"), ("U2", "SOT23-5")]);
        assert_eq!(parts[0].value, "TLV75533");
        let nets = crate::files::altium::sch::nets(&b);
        assert!(nets.contains(&"VBAT".to_string()) && nets.contains(&"GND".to_string()), "{nets:?}");
        // the pin and the label share the net: the reader's connectivity
        let its = crate::files::altium::sch::items(&b).unwrap();
        let vin = its.iter().find(|i| i.info.starts_with("pin ") && i.info.contains("VIN") && i.info.contains("U1")).map(|i| i.info.to_string()).unwrap_or_default();
        assert!(vin.contains("net=VBAT"), "{vin}");
        // edit: read back, remove U2, the owner indices close up
        let mut sh2 = Sheet::read(&b, "e").unwrap();
        let n = sh2.recs.len();
        sh2.remove_part("U2").unwrap();
        assert!(sh2.recs.len() < n);
        assert_eq!(sh2.components().iter().map(|c| c.1.as_str()).collect::<Vec<_>>(), vec!["U1"]);
        for r in sh2.recs.iter().filter_map(Raw::rec) { if let Some(o) = r.get("OwnerIndex") { let o: usize = o.parse().unwrap(); assert!(sh2.text(o).map_or(false, |x| x.record() >= 1), "owner {o}") } }
        assert_eq!(sh2.remove_objects(Some("VBAT"), None), 2);
        let b2 = sh2.bytes().unwrap();
        assert_eq!(crate::files::altium::sch::parts(&b2).len(), 1);
        assert_eq!(Sheet::read(&b2, "x").unwrap().header.int("Weight") as usize, sh2.recs.len());
    }
}

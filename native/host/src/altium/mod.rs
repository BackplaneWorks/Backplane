//! Altium Designer files, read in-process: the same items the KiCad readers make, so every viewer, cross-probe, diff and
//! phone draws an Altium design with no client change.
//!
//! An Altium binary document (.SchDoc, .PcbDoc, .SchLib, .PcbLib) is an OLE compound file (MS-CFB): a small file system of
//! storages and streams. A schematic keeps its objects in one stream, `FileHeader`, as length-prefixed `|KEY=VALUE|` text
//! records; a board keeps one storage per object kind (`Tracks6`, `Pads6`, ...) whose `Data` stream holds that kind's
//! records, as text (board, nets, components, rules) or as binary (tracks, arcs, vias, pads, fills, texts, regions).
//! The project (.PrjPcb) is an INI file listing the documents.
//!
//! Format references: KiCad's import notes (https://dev-docs.kicad.org/en/import-formats/altium) and python-altium's
//! format.md (https://github.com/vadmium/python-altium/blob/master/format.md).
pub mod live;
pub mod prj;
pub mod sch;
pub mod pcb;

use std::collections::HashMap;
use std::io::{Cursor, Read};

/// The first eight bytes of every compound file.
pub const MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
pub fn is_cfb(b: &[u8]) -> bool { b.len() >= 8 && b[..8] == MAGIC }

/// What kind of Altium file a path names, by its extension (Altium writes them in any case).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind { Project, Schematic, Board, SchLib, PcbLib }
pub fn kind(path: &str) -> Option<Kind> {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    Some(match ext.as_str() {
        "prjpcb" => Kind::Project, "schdoc" => Kind::Schematic, "pcbdoc" => Kind::Board, "schlib" => Kind::SchLib, "pcblib" => Kind::PcbLib,
        _ => return None,
    })
}
pub fn is_altium(path: &str) -> bool { kind(path).is_some() }

/// An opened compound file: its streams by path ("FileHeader", "Tracks6/Data").
pub struct Doc { cf: cfb::CompoundFile<Cursor<Vec<u8>>> }
impl Doc {
    pub fn open(bytes: Vec<u8>) -> Option<Doc> {
        if !is_cfb(&bytes) { return None }
        cfb::CompoundFile::open(Cursor::new(bytes)).ok().map(|cf| Doc { cf })
    }
    pub fn read(path: &str) -> Option<Doc> { std::fs::read(path).ok().and_then(Doc::open) }
    /// A stream's bytes; None when the file has no such stream.
    pub fn stream(&mut self, path: &str) -> Option<Vec<u8>> {
        let p = format!("/{path}");
        if !self.cf.is_stream(&p) { return None }
        let mut s = self.cf.open_stream(&p).ok()?;
        let mut v = Vec::new();
        s.read_to_end(&mut v).ok()?;
        Some(v)
    }
    /// Every stream's path, storages walked (for diagnostics and tests).
    pub fn streams(&self) -> Vec<String> {
        self.cf.walk().filter(|e| e.is_stream()).map(|e| e.path().to_string_lossy().trim_start_matches('/').to_string()).collect()
    }
}

/// Windows-1252 text as a String (Altium's 8-bit strings; a `%UTF8%KEY` property carries the UTF-8 form when it differs).
pub fn cp1252(b: &[u8]) -> String {
    const HI: [char; 32] = ['€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
        '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ'];
    b.iter().map(|&c| if (0x80..0xA0).contains(&c) { HI[(c - 0x80) as usize] } else { c as char }).collect()
}

/// One record's properties: keys upper-cased; a `%UTF8%KEY` value replaces KEY's 8-bit one.
#[derive(Clone, Debug, Default)]
pub struct Props(pub HashMap<String, String>);
impl Props {
    /// `|KEY=VALUE|KEY=VALUE...` (a trailing NUL and empty fields are ignored)
    pub fn parse(b: &[u8]) -> Props {
        let b = match b.iter().position(|&c| c == 0) { Some(i) => &b[..i], None => b };
        let mut m = HashMap::new();
        let mut utf8 = Vec::new();
        for field in b.split(|&c| c == b'|') {
            let Some(eq) = field.iter().position(|&c| c == b'=') else { continue };
            let (k, v) = (&field[..eq], &field[eq + 1..]);
            let key = cp1252(k).trim().to_ascii_uppercase();
            if key.is_empty() { continue }
            if let Some(rest) = key.strip_prefix("%UTF8%") {
                utf8.push((rest.to_string(), String::from_utf8_lossy(v).into_owned()));
            } else {
                m.insert(key, cp1252(v));
            }
        }
        for (k, v) in utf8 { m.insert(k, v); }
        Props(m)
    }
    pub fn get(&self, k: &str) -> &str { self.0.get(k).map(|s| s.as_str()).unwrap_or("") }
    pub fn has(&self, k: &str) -> bool { self.0.contains_key(k) }
    pub fn int(&self, k: &str) -> i64 { self.get(k).trim().parse().unwrap_or(0) }
    pub fn num(&self, k: &str) -> f64 { parse_num(self.get(k)) }
    pub fn bool(&self, k: &str) -> bool { matches!(self.get(k).trim(), "T" | "TRUE" | "True" | "true" | "1") }
}

/// Altium's numbers: "12.5", "12.5mil", "0.3mm" (mm turned into mils), "1e+002". Unknown text is 0.
pub fn parse_num(s: &str) -> f64 {
    let s = s.trim();
    let (n, scale) = if let Some(v) = s.strip_suffix("mil") { (v, 1.0) } else if let Some(v) = s.strip_suffix("mm") { (v, 1.0 / 0.0254) } else { (s, 1.0) };
    n.trim().parse::<f64>().map(|v| v * scale).unwrap_or(0.0)
}

/// A schematic stream (`FileHeader`, `Additional`) as records: each is a little-endian u32 whose low 24 bits are the
/// payload's length and whose high byte is its type (0 text properties, 1 binary).
pub enum SchRec { Text(Props), Bin(Vec<u8>) }
pub fn sch_records(b: &[u8]) -> Vec<SchRec> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= b.len() {
        let h = u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        let (len, ty) = ((h & 0x00FF_FFFF) as usize, h >> 24);
        i += 4;
        if i + len > b.len() { break }
        let body = &b[i..i + len];
        out.push(if ty == 0 { SchRec::Text(Props::parse(body)) } else { SchRec::Bin(body.to_vec()) });
        i += len;
    }
    out
}

/// A board text stream (`Board6/Data`, `Nets6/Data`, `Components6/Data`, ...): each record is a little-endian u32
/// length then that many bytes of `|KEY=VALUE|` text.
pub fn pcb_text_records(b: &[u8]) -> Vec<Props> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= b.len() {
        let len = u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
        i += 4;
        if i + len > b.len() { break }
        out.push(Props::parse(&b[i..i + len]));
        i += len;
    }
    out
}

/// A little-endian cursor over a binary record; reads past the end give zeros (a short record is drawn, not fatal).
pub struct Bin<'a> { pub b: &'a [u8], pub i: usize }
impl<'a> Bin<'a> {
    pub fn new(b: &'a [u8]) -> Bin<'a> { Bin { b, i: 0 } }
    pub fn left(&self) -> usize { self.b.len().saturating_sub(self.i) }
    pub fn skip(&mut self, n: usize) { self.i += n }
    pub fn bytes(&mut self, n: usize) -> &'a [u8] {
        let s = self.i.min(self.b.len());
        let e = (self.i + n).min(self.b.len());
        self.i += n;
        &self.b[s..e]
    }
    fn arr<const N: usize>(&mut self) -> [u8; N] { let mut a = [0u8; N]; let s = self.bytes(N); a[..s.len()].copy_from_slice(s); a }
    pub fn u8(&mut self) -> u8 { self.arr::<1>()[0] }
    pub fn u16(&mut self) -> u16 { u16::from_le_bytes(self.arr()) }
    pub fn u32(&mut self) -> u32 { u32::from_le_bytes(self.arr()) }
    pub fn i32(&mut self) -> i32 { i32::from_le_bytes(self.arr()) }
    pub fn f64(&mut self) -> f64 { f64::from_le_bytes(self.arr()) }
    /// a sub-record: u32 length then its bytes
    pub fn sub(&mut self) -> &'a [u8] { let n = self.u32() as usize; self.bytes(n) }
}

/// Test aid: items as an SVG (layer colours from plotgen::look; dark ground) to eyeball a reader against Altium.
/// `rsvg-convert -w 2000 x.svg > x.png` turns it into a picture.
#[cfg(test)]
pub fn debug_svg(items: &[crate::files::kicad::board::Item], kind: u32, path: &str) {
    use crate::files::kicad::board::{Prim, Pt};
    use std::fmt::Write;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut grow = |p: &Pt| { x0 = x0.min(p.x); y0 = y0.min(p.y); x1 = x1.max(p.x); y1 = y1.max(p.y); };
    for it in items {
        match &it.prim {
            Prim::Seg { a, b, .. } => { grow(a); grow(b) }
            Prim::Line { ps, .. } | Prim::Poly { ps, .. } | Prim::Tint { ps, .. } => ps.iter().for_each(&mut grow),
            Prim::Circ { c, r, .. } => { grow(&Pt { x: c.x - r, y: c.y - r }); grow(&Pt { x: c.x + r, y: c.y + r }) }
        }
    }
    if x0 > x1 { (x0, y0, x1, y1) = (0.0, 0.0, 1.0, 1.0) }
    let m = ((x1 - x0).max(y1 - y0) * 0.02).max(0.5);
    let mut o = format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='{} {} {} {}'><rect x='{}' y='{}' width='{}' height='{}' fill='#1a1d22'/>",
        x0 - m, y0 - m, x1 - x0 + 2.0 * m, y1 - y0 + 2.0 * m, x0 - m, y0 - m, x1 - x0 + 2.0 * m, y1 - y0 + 2.0 * m);
    let mut sorted: Vec<&crate::files::kicad::board::Item> = items.iter().collect();
    sorted.sort_by_key(|i| i.layer);
    for it in sorted {
        let (c, a) = crate::files::kicad::plotgen::look(kind, it.layer);
        let col = format!("#{:06x}", c & 0xFFFFFF);
        let op = a as f32 / 255.0;
        let pts = |ps: &[Pt]| ps.iter().map(|p| format!("{},{}", p.x, p.y)).collect::<Vec<_>>().join(" ");
        match &it.prim {
            Prim::Seg { a, b, w } => { let _ = write!(o, "<line x1='{}' y1='{}' x2='{}' y2='{}' stroke='{col}' stroke-opacity='{op}' stroke-width='{}' stroke-linecap='round'/>", a.x, a.y, b.x, b.y, w.max(0.02)); }
            Prim::Line { ps, w } => { let _ = write!(o, "<polyline points='{}' fill='none' stroke='{col}' stroke-opacity='{op}' stroke-width='{}' stroke-linecap='round' stroke-linejoin='round'/>", pts(ps), w.max(0.02)); }
            Prim::Circ { c: ce, r, w, fill } => { if *fill { let _ = write!(o, "<circle cx='{}' cy='{}' r='{}' fill='{col}' fill-opacity='{op}'/>", ce.x, ce.y, r + w / 2.0); } else { let _ = write!(o, "<circle cx='{}' cy='{}' r='{}' fill='none' stroke='{col}' stroke-opacity='{op}' stroke-width='{}'/>", ce.x, ce.y, r, w.max(0.02)); } }
            Prim::Poly { ps, w, fill } => { if *fill { let _ = write!(o, "<polygon points='{}' fill='{col}' fill-opacity='{op}' stroke='{col}' stroke-opacity='{op}' stroke-width='{}'/>", pts(ps), w); } else { let _ = write!(o, "<polygon points='{}' fill='none' stroke='{col}' stroke-opacity='{op}' stroke-width='{}'/>", pts(ps), w.max(0.02)); } }
            Prim::Tint { ps, color, alpha } => { let _ = write!(o, "<polygon points='{}' fill='#{:06x}' fill-opacity='{}'/>", pts(ps), color & 0xFFFFFF, *alpha as f32 / 255.0); }
        }
    }
    o.push_str("</svg>");
    let _ = std::fs::write(path, o);
}

/// Test aid: the private corpus of real Altium projects named by BP_ALTIUM_CORPUS (never committed: people's designs).
#[cfg(test)]
pub fn corpus() -> Option<std::path::PathBuf> { std::env::var_os("BP_ALTIUM_CORPUS").map(std::path::PathBuf::from).filter(|p| p.is_dir()) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn props_parse_utf8_and_case() {
        let p = Props::parse(b"|RECORD=1|LibReference=R\xe9s|%UTF8%LibReference=R\xc3\xa9s\xe2\x82\xac|Location.X=100|\0junk");
        assert_eq!(p.get("RECORD"), "1");
        assert_eq!(p.get("LIBREFERENCE"), "Rés€");
        assert_eq!(p.int("LOCATION.X"), 100);
        assert_eq!(p.get("MISSING"), "");
    }
    #[test] fn nums_with_units() {
        assert_eq!(parse_num("10mil"), 10.0);
        assert!((parse_num("0.254mm") - 10.0).abs() < 1e-9);
        assert_eq!(parse_num("1e+002"), 100.0);
        assert_eq!(parse_num("x"), 0.0);
    }
    #[test] fn sch_and_pcb_framing() {
        let mut s = Vec::new();
        for (ty, body) in [(0u32, &b"|RECORD=31|\0"[..]), (1, &[1, 2, 3][..])] {
            s.extend_from_slice(&((body.len() as u32) | (ty << 24)).to_le_bytes()); s.extend_from_slice(body);
        }
        let r = sch_records(&s);
        assert!(matches!(&r[0], SchRec::Text(p) if p.get("RECORD") == "31"));
        assert!(matches!(&r[1], SchRec::Bin(b) if b == &[1, 2, 3]));
        let mut p = Vec::new();
        p.extend_from_slice(&7u32.to_le_bytes()); p.extend_from_slice(b"|NAME=A");
        assert_eq!(pcb_text_records(&p)[0].get("NAME"), "A");
    }
    #[test] fn kinds() {
        assert_eq!(kind("x/Top.SchDoc"), Some(Kind::Schematic));
        assert_eq!(kind("B.PCBDOC"), Some(Kind::Board));
        assert_eq!(kind("p.PrjPcb"), Some(Kind::Project));
        assert_eq!(kind("a.kicad_pcb"), None);
        assert!(!is_cfb(b"(kicad_pcb"));
    }
}

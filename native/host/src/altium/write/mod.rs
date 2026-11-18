//! Writing Altium files without Altium (any platform): schematic symbol libraries (.SchLib), footprint libraries (.PcbLib)
//! and schematic sheets (.SchDoc), made or edited from a small JSON spec (`spec.rs`, the `altium_write` tool).
//!
//! The files are the binary ones Altium itself saves: an OLE compound file (`cfb`) whose streams hold `|KEY=VALUE|` text
//! records and, for pins and board primitives, binary records. The layouts follow what Altium writes (checked against files
//! it saved) and AltiumSharp (OriginalCircuit.Altium, Apache-2.0, https://github.com/issus/AltiumSharp), whose writers
//! the binary pad, track and arc records are ported from. Editing keeps every stream it does not change byte for byte.
pub mod fp;
pub mod pcblib;
pub mod schdoc;
pub mod schlib;
pub mod spec;

use std::collections::HashSet;
use std::io::{Cursor, Read, Write};

/// A compound file's contents: its streams by path ("FileHeader", "R0603/Data") in order, and storages' CLSIDs.
#[derive(Clone, Debug, Default)]
pub struct Tree { pub streams: Vec<(String, Vec<u8>)>, pub clsids: Vec<(String, [u8; 16])> }
impl Tree {
    /// Every stream and storage CLSID of a compound file.
    pub fn read(bytes: &[u8]) -> Result<Tree, String> {
        if !super::is_cfb(bytes) { return Err("not an Altium binary file (no OLE compound file header)".into()) }
        let mut cf = cfb::CompoundFile::open(Cursor::new(bytes.to_vec())).map_err(|e| format!("unreadable compound file: {e}"))?;
        let entries: Vec<(String, bool, [u8; 16])> = cf.walk().map(|e| (e.path().to_string_lossy().trim_start_matches('/').to_string(), e.is_stream(), *e.clsid().as_bytes())).collect();
        let mut t = Tree::default();
        for (p, stream, clsid) in entries {
            if stream {
                let mut v = Vec::new();
                cf.open_stream(format!("/{p}")).and_then(|mut s| s.read_to_end(&mut v)).map_err(|e| format!("{p}: {e}"))?;
                t.streams.push((p, v));
            } else if clsid != [0; 16] { t.clsids.push((p, clsid)) }
        }
        Ok(t)
    }
    pub fn get(&self, p: &str) -> Option<&[u8]> { self.streams.iter().find(|(k, _)| k == p).map(|(_, v)| v.as_slice()) }
    /// Sets a stream, in place when it exists, else at the end.
    pub fn put(&mut self, p: &str, v: Vec<u8>) {
        match self.streams.iter_mut().find(|(k, _)| k == p) { Some(s) => s.1 = v, None => self.streams.push((p.to_string(), v)) }
    }
    /// Removes a storage (every stream under `dir/`).
    pub fn remove_dir(&mut self, dir: &str) {
        let pre = format!("{dir}/");
        self.streams.retain(|(k, _)| !k.starts_with(&pre));
        self.clsids.retain(|(k, _)| k != dir && !k.starts_with(&pre));
    }
    /// Top-level storage names.
    pub fn dirs(&self) -> Vec<String> {
        let mut out: Vec<String> = vec![];
        for (k, _) in &self.streams { if let Some((d, _)) = k.split_once('/') { if !out.iter().any(|o| o == d) { out.push(d.to_string()) } } }
        out
    }
    /// The compound file's bytes (version 3, 512-byte sectors, as Altium writes).
    pub fn build(&self) -> Result<Vec<u8>, String> {
        let err = |e: std::io::Error| format!("could not build the compound file: {e}");
        let mut cf = cfb::CompoundFile::create_with_version(cfb::Version::V3, Cursor::new(Vec::new())).map_err(err)?;
        for (p, v) in &self.streams {
            if let Some((dir, _)) = p.rsplit_once('/') { cf.create_storage_all(format!("/{dir}")).map_err(err)?; }
            let mut s = cf.create_stream(format!("/{p}")).map_err(err)?;
            s.write_all(v).map_err(err)?;
        }
        for (p, c) in &self.clsids {
            if cf.is_storage(format!("/{p}")) { cf.set_storage_clsid(format!("/{p}"), uuid::Uuid::from_bytes(*c)).map_err(err)?; }
        }
        cf.flush().map_err(err)?;
        Ok(cf.into_inner().into_inner())
    }
}

/// Writes a file next to its final place, then renames it over (a reader never sees half a file).
pub fn save(path: &str, bytes: &[u8]) -> Result<(), String> {
    let p = std::path::Path::new(path);
    if let Some(d) = p.parent().filter(|d| !d.as_os_str().is_empty()) { std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?; }
    let tmp = p.with_extension(format!("{}.bp-part", p.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default()));
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, p).map_err(|e| { let _ = std::fs::remove_file(&tmp); format!("{path}: {e}") })
}

/// Marks every `UniqueID=` already in a file as used, so new ids never repeat one.
pub fn mark_ids(t: &Tree, uids: &mut Uids) {
    for (_, v) in &t.streams {
        let mut i = 0;
        while let Some(at) = v[i..].windows(9).position(|w| w.eq_ignore_ascii_case(b"UniqueID=")) {
            let s = i + at + 9;
            if let Some(id) = v.get(s..s + 8) { uids.mark(&String::from_utf8_lossy(id)) }
            i = s;
        }
    }
}

// Records -------------------------------------------------------------------------------------------------------------

/// One `|KEY=VALUE|` record, keys in the order given (Altium's own spelling: `Location.X`, `OwnerPartId`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rec(pub Vec<(String, String)>);
impl Rec {
    pub fn new(record: i64) -> Rec { Rec(vec![("RECORD".into(), record.to_string())]) }
    pub fn kv(mut self, k: &str, v: impl ToString) -> Rec { self.set(k, v); self }
    /// Sets a key, in place when it is there.
    pub fn set(&mut self, k: &str, v: impl ToString) {
        let v = v.to_string();
        match self.0.iter_mut().find(|(a, _)| a.eq_ignore_ascii_case(k)) { Some(e) => e.1 = v, None => self.0.push((k.to_string(), v)) }
    }
    pub fn del(&mut self, k: &str) { self.0.retain(|(a, _)| !a.eq_ignore_ascii_case(k)) }
    pub fn get(&self, k: &str) -> Option<&str> { self.0.iter().find(|(a, _)| a.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str()) }
    pub fn int(&self, k: &str) -> i64 { self.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(0) }
    pub fn record(&self) -> i64 { self.get("RECORD").and_then(|v| v.trim().parse().ok()).unwrap_or(-1) }
    /// A flag: written as `T` when on, left out when off (Altium's way).
    pub fn flag(self, k: &str, on: bool) -> Rec { if on { self.kv(k, "T") } else { self } }
    /// A non-zero integer (Altium leaves zeros out).
    pub fn nz(self, k: &str, v: i64) -> Rec { if v != 0 { self.kv(k, v) } else { self } }
    /// A schematic coordinate in mils: `KEY` in 10-mil units and `KEY_Frac` in 1/100000 of one, zeros left out.
    pub fn coord(mut self, k: &str, mils: f64) -> Rec { self.set_coord(k, mils); self }
    pub fn set_coord(&mut self, k: &str, mils: f64) {
        let (whole, frac) = split_units(mils);
        self.del(k); self.del(&format!("{k}_Frac"));
        if whole != 0 { self.0.push((k.to_string(), whole.to_string())) }
        if frac != 0 { self.0.push((format!("{k}_Frac"), frac.to_string())) }
    }
    /// A coordinate in mils (`KEY` and `KEY_Frac`).
    pub fn mils(&self, k: &str) -> f64 { self.int(k) as f64 * 10.0 + self.int(&format!("{k}_Frac")) as f64 / 10000.0 }
    /// The text form, `|K=V|K=V`; a value that is not Windows-1252 also goes as `%UTF8%K` (as Altium writes it).
    pub fn text(&self) -> Vec<u8> {
        let mut o = Vec::new();
        for (k, v) in &self.0 {
            o.push(b'|'); o.extend_from_slice(&to_cp1252(k)); o.push(b'='); o.extend_from_slice(&to_cp1252(v));
            if !v.is_ascii() && !is_cp1252(v) { o.extend_from_slice(b"|%UTF8%"); o.extend_from_slice(k.as_bytes()); o.push(b'='); o.extend_from_slice(v.as_bytes()); }
        }
        o
    }
    /// A record parsed back, keys as written (not upper-cased); `%UTF8%` values replace their 8-bit ones.
    pub fn parse(b: &[u8]) -> Rec {
        let b = match b.iter().position(|&c| c == 0) { Some(i) => &b[..i], None => b };
        let mut r = Rec::default();
        let mut utf8 = vec![];
        for f in b.split(|&c| c == b'|') {
            let Some(eq) = f.iter().position(|&c| c == b'=') else { continue };
            let k = super::cp1252(&f[..eq]);
            if k.is_empty() { continue }
            match k.strip_prefix("%UTF8%") { Some(rest) => utf8.push((rest.to_string(), String::from_utf8_lossy(&f[eq + 1..]).into_owned())), None => r.0.push((k, super::cp1252(&f[eq + 1..]))) }
        }
        for (k, v) in utf8 { r.set(&k, v) }
        r
    }
}

/// mils as (10-mil units, 1/100000 of a unit), rounded to the 0.0001 mil Altium keeps.
pub fn split_units(mils: f64) -> (i64, i64) {
    let t = (mils * 10000.0).round() as i64; // 0.0001 mil
    (t / 100000, t % 100000)
}

const CP1252_HI: [char; 32] = ['€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ'];
fn cp_byte(c: char) -> Option<u8> {
    let u = c as u32;
    if u < 0x80 || (0xA0..0x100).contains(&u) { return Some(u as u8) }
    CP1252_HI.iter().position(|&h| h == c).map(|i| 0x80 + i as u8)
}
fn is_cp1252(s: &str) -> bool { s.chars().all(|c| cp_byte(c).is_some()) }
/// Windows-1252 bytes ('?' for what it cannot hold).
pub fn to_cp1252(s: &str) -> Vec<u8> { s.chars().map(|c| cp_byte(c).unwrap_or(b'?')).collect() }

/// A schematic stream record: u32 (payload length | kind << 24), then the payload (text records end in a NUL).
pub fn sch_text(r: &Rec) -> Vec<u8> { let mut p = r.text(); p.push(0); framed(&p, 0) }
pub fn sch_bin(p: &[u8]) -> Vec<u8> { framed(p, 1) }
fn framed(p: &[u8], kind: u32) -> Vec<u8> { let mut o = ((p.len() as u32) | (kind << 24)).to_le_bytes().to_vec(); o.extend_from_slice(p); o }
/// A board/library text block: u32 length, the text, a NUL.
pub fn pcb_text(r: &Rec) -> Vec<u8> { let mut p = r.text(); p.push(0); let mut o = (p.len() as u32).to_le_bytes().to_vec(); o.extend(p); o }
/// A length-prefixed string (one byte of length, Windows-1252, at most 255).
pub fn pstr(s: &str) -> Vec<u8> { let mut b = to_cp1252(s); b.truncate(255); let mut o = vec![b.len() as u8]; o.extend(b); o }
/// A string block: u32 length, then the short string.
pub fn str_block(s: &str) -> Vec<u8> { let p = pstr(s); let mut o = (p.len() as u32).to_le_bytes().to_vec(); o.extend(p); o }
/// A binary block: u32 length, then the bytes.
pub fn block(b: &[u8]) -> Vec<u8> { let mut o = (b.len() as u32).to_le_bytes().to_vec(); o.extend_from_slice(b); o }

/// A storage name for a library entry: at most 31 characters (the compound file's limit), no `/` (Altium's rule; the
/// full name is kept in `SectionKeys`), unique among `taken` (compared without case).
pub fn section_key(name: &str, taken: &HashSet<String>) -> String {
    let base: String = name.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '!' | '*') { '_' } else { c }).take(31).collect();
    let base = if base.is_empty() { "_".to_string() } else { base };
    if !taken.contains(&base.to_lowercase()) { return base }
    for i in 1.. {
        let suf = format!("~{i}");
        let k: String = base.chars().take(31 - suf.len()).collect::<String>() + &suf;
        if !taken.contains(&k.to_lowercase()) { return k }
    }
    unreachable!()
}

/// Altium's 8-letter unique ids, made from a seed so the same input gives the same file; never one already used.
pub struct Uids { state: u64, used: HashSet<String> }
impl Uids {
    pub fn new(seed: &str) -> Uids { Uids { state: crate::files::kicad::board::hash(seed.as_bytes(), 2166136261) as u64 ^ 0x9E37_79B9_7F4A_7C15, used: HashSet::new() } }
    pub fn mark(&mut self, id: &str) { self.used.insert(id.to_string()); }
    pub fn next(&mut self) -> String {
        loop {
            self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            let s: String = (0..8).map(|i| (b'A' + ((z >> (i * 8)) % 26) as u8) as char).collect();
            if self.used.insert(s.clone()) { return s }
        }
    }
    /// A GUID in Altium's `{XXXXXXXX-...}` form.
    pub fn guid(&mut self) -> String {
        let a = self.next(); let b = self.next();
        let h = crate::files::kicad::board::hash(format!("{a}{b}").as_bytes(), 2166136261) as u64;
        let h2 = crate::files::kicad::board::hash(format!("{b}{a}").as_bytes(), 2166136261) as u64;
        let h3 = crate::files::kicad::board::hash(format!("{a}/{b}").as_bytes(), 2166136261) as u64;
        format!("{{{:08X}-{:04X}-4{:03X}-{:04X}-{:012X}}}", h as u32, h2 as u16, (h2 >> 16) as u16 & 0xFFF, 0x8000 | (h3 as u16 & 0x3FFF), (h3 >> 16) & 0xFFFF_FFFF_FFFF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn tree_round_trip() {
        let mut t = Tree::default();
        t.put("FileHeader", b"head".to_vec());
        t.put("R1/Data", vec![1, 2, 3]);
        t.put("Library/Models/Data", vec![]);
        t.clsids.push(("R1".into(), [7; 16]));
        let b = t.build().unwrap();
        let back = Tree::read(&b).unwrap();
        assert_eq!(back.get("FileHeader"), Some(&b"head"[..]));
        assert_eq!(back.get("R1/Data"), Some(&[1u8, 2, 3][..]));
        assert_eq!(back.get("Library/Models/Data"), Some(&[][..]));
        assert_eq!(back.clsids, vec![("R1".to_string(), [7; 16])]);
        assert_eq!(&b[0x1A..0x1C], &[3, 0]); // version 3
    }
    #[test] fn records_and_units() {
        let r = Rec::new(1).kv("LibReference", "Ω-Res").coord("Location.X", 105.5).coord("Location.Y", 0.0).flag("IsHidden", false);
        assert_eq!(r.get("Location.X"), Some("10")); assert_eq!(r.get("Location.X_Frac"), Some("55000")); assert!(r.get("Location.Y").is_none());
        let t = r.text();
        let back = Rec::parse(&t);
        assert_eq!(back.get("LibReference"), Some("Ω-Res"));
        assert!((back.mils("Location.X") - 105.5).abs() < 1e-9);
        assert_eq!(split_units(-15.0), (-1, -50000));
        let f = sch_text(&Rec::new(31));
        assert_eq!(&f[..4], &[11, 0, 0, 0]); assert_eq!(*f.last().unwrap(), 0);
        assert_eq!(str_block("AB"), vec![3, 0, 0, 0, 2, b'A', b'B']);
    }
    #[test] fn keys_and_ids() {
        let mut taken = HashSet::new();
        let long = "RHDR40W65P254_2X20_5120X504X850P";
        let k = section_key(long, &taken);
        assert_eq!(k.len(), 31); taken.insert(k.to_lowercase());
        let k2 = section_key(long, &taken);
        assert!(k2.ends_with("~1") && k2.len() == 31);
        assert_eq!(section_key("a/b", &HashSet::new()), "a_b");
        let (mut u, mut v) = (Uids::new("x"), Uids::new("x"));
        let a = u.next();
        assert_eq!(a, v.next()); assert_eq!(a.len(), 8); assert!(a.chars().all(|c| c.is_ascii_uppercase()));
        assert_ne!(a, u.next());
        assert!(u.guid().starts_with('{'));
    }
}

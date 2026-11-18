//! .PcbLib: footprints made from a model and added to (or replaced in) a library, and the footprints read back.
//!
//! A library is a compound file: `FileHeader` (the version string and the library's id), `Library` (`Data`: the
//! library's properties, then the footprint count and names; `ComponentParamsTOC`, `LayerKindMapping`, `PadViaLibrary`,
//! `Models`, `Textures`, `ModelsNoEmbed`), `SectionKeys` (full names of footprints whose storage name had to be cut to 31
//! characters) and a storage per footprint: `Header` (its primitive count), `Parameters`, `WideStrings` and `Data`, its
//! name then its primitives, each a type byte and that type's sub-records (binary, as on a board: 1 arc, 2 pad, 4 track,
//! 6 fill, 12 component body). The binary layouts are AltiumSharp's (Apache-2.0), checked against libraries Altium saved.
//! Coordinates here are mm with Y up (the board's own sense); records hold 1/10000 mil.
use super::{block, pcb_text, section_key, str_block, Rec, Tree, Uids};
use std::collections::HashSet;

/// Altium's board layers by number.
pub const TOP: u8 = 1; pub const BOTTOM: u8 = 32; pub const TOP_OVERLAY: u8 = 33; pub const BOTTOM_OVERLAY: u8 = 34;
pub const TOP_PASTE: u8 = 35; pub const TOP_SOLDER: u8 = 37; pub const KEEPOUT: u8 = 56; pub const MULTI: u8 = 74;
/// Mechanical layer n (1..16).
pub fn mech(n: u8) -> u8 { 56 + n.clamp(1, 16) }
/// A layer by name: "top", "bottom", "multi", "top_overlay"/"silk", "bottom_overlay", "top_paste", "top_solder"/"mask",
/// "keepout", "mech13"/"mechanical13"; also Altium's own (`TopLayer`, `Mechanical15`).
pub fn layer(name: &str) -> Option<u8> {
    let n = name.trim().to_ascii_lowercase().replace([' ', '-', '_', '.'], "");
    if let Some(d) = n.strip_prefix("mechanical").or_else(|| n.strip_prefix("mech")) { return d.parse::<u8>().ok().filter(|d| (1..=16).contains(d)).map(mech) }
    Some(match n.as_str() {
        "top" | "toplayer" | "fcu" => TOP, "bottom" | "bottomlayer" | "bcu" => BOTTOM, "multi" | "multilayer" => MULTI,
        "topoverlay" | "silk" | "topsilk" | "fsilks" => TOP_OVERLAY, "bottomoverlay" | "bottomsilk" | "bsilks" => BOTTOM_OVERLAY,
        "toppaste" | "paste" | "fpaste" => TOP_PASTE, "bottompaste" | "bpaste" => 36, "topsolder" | "mask" | "fmask" => TOP_SOLDER, "bottomsolder" | "bmask" => 38,
        "keepout" | "keepoutlayer" => KEEPOUT, _ => return None,
    })
}
/// The layer's id as v7 files repeat it in each primitive.
fn v7_layer(l: u8) -> u32 {
    match l {
        32 => 0x0100_FFFF, 1..=31 => 0x0100_0000 + l as u32, 39..=54 => 0x0101_0000 + (l as u32 - 38), 57..=72 => 0x0102_0000 + (l as u32 - 56),
        33 => 0x0103_0006, 34 => 0x0103_0007, 35 => 0x0103_0008, 36 => 0x0103_0009, 37 => 0x0103_000A, 38 => 0x0103_000B,
        55 => 0x0103_000C, 56 => 0x0103_000D, 73 => 0x0103_000E, _ => 0x0103_000F,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PadShape { Rect, Round, Oct, /// corner radius as a percentage of half the shorter side (Altium's)
    RoundRect(u8) }
/// A pad: `hole` 0 is a surface pad on `layer` (TOP or BOTTOM), else a through pad on every layer; `slot` > `hole` makes the
/// hole a slot that long (along the pad's X before `rot`). mm, degrees.
#[derive(Clone, Debug, PartialEq)]
pub struct Pad { pub name: String, pub x: f64, pub y: f64, pub w: f64, pub h: f64, pub shape: PadShape, pub rot: f64, pub hole: f64, pub slot: f64, pub plated: bool, pub layer: u8 }
impl Pad {
    pub fn smd(name: &str, x: f64, y: f64, w: f64, h: f64, shape: PadShape) -> Pad { Pad { name: name.into(), x, y, w, h, shape, rot: 0.0, hole: 0.0, slot: 0.0, plated: true, layer: TOP } }
    pub fn thru(name: &str, x: f64, y: f64, d: f64, hole: f64, shape: PadShape) -> Pad { Pad { name: name.into(), x, y, w: d, h: d, shape, rot: 0.0, hole, slot: 0.0, plated: true, layer: MULTI } }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Line { pub layer: u8, pub a: (f64, f64), pub b: (f64, f64), pub w: f64 }
/// An arc, angles in degrees counter-clockwise from +X (0..360 is a circle).
#[derive(Clone, Debug, PartialEq)]
pub struct Arc { pub layer: u8, pub c: (f64, f64), pub r: f64, pub a0: f64, pub a1: f64, pub w: f64 }
/// An extruded 3D body: its outline (mm) from `z0` to `z1` above the board (mm).
#[derive(Clone, Debug, PartialEq)]
pub struct Body { pub outline: Vec<(f64, f64)>, pub z0: f64, pub z1: f64, pub layer: u8 }

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Footprint { pub name: String, pub description: String, pub height: f64, pub pads: Vec<Pad>, pub lines: Vec<Line>, pub arcs: Vec<Arc>, pub bodies: Vec<Body> }

fn raw(mm: f64) -> i32 { (mm / 0.0254 * 10000.0).round() as i32 }
fn mil_text(mm: f64) -> String { let s = format!("{:.4}", mm / 0.0254); format!("{}mil", s.trim_end_matches('0').trim_end_matches('.')) }
/// The 13 bytes every primitive starts with: layer, flags (unlocked, saved), no net, polygon or component, reserved.
fn head(l: u8) -> Vec<u8> { let mut b = vec![l, 0x0C, 0x00]; b.extend([0xFF; 10]); b }

// AltiumSharp's pad sub-record 5 tail (offsets 61-201), from a standard Altium pad; per-pad values are put over it.
const PAD_TAIL: [u8; 141] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xA0, 0x86, 0x01, 0x00, 0x04, 0x00, 0xA0, 0x86, 0x01,
    0x00, 0x40, 0x0D, 0x03, 0x00, 0x40, 0x0D, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x9C, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x0F, 0x00, 0x03, 0x01, 0x00, 0x00, 0x00, 0x40, 0x9C, 0x00, 0x00,
    0x00, 0x64, 0x9A, 0x92, 0x26, 0x10, 0xC7, 0xE4, 0x41, 0xA3, 0x2B, 0x29, 0x17, 0xA5, 0x35, 0x2E,
    0x67, 0x7F, 0xAB, 0x21, 0x20, 0xC3, 0x0B, 0x32, 0x47, 0xAD, 0xCE, 0x6C, 0xB7, 0xB8, 0xC9, 0x7E,
    0x68, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x7F, 0xFF, 0xFF, 0xFF, 0x7F, 0x00, 0x01, 0x1A,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

fn guid_bytes(uids: &mut Uids) -> [u8; 16] {
    let g = uids.guid();
    let hex: String = g.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let mut o = [0u8; 16];
    for i in 0..16 { o[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(0) }
    o
}

/// A pad record (type 2): its name, two empty sub-records, its net text, then the geometry (202 bytes) and, for a rounded
/// rectangle or a slot, the per-layer block (596 bytes).
pub fn pad_rec(p: &Pad, uids: &mut Uids) -> Vec<u8> {
    let mut o = vec![2u8];
    o.extend(str_block(&p.name));
    o.extend(str_block(""));
    o.extend(str_block("|&|0"));
    o.extend(block(&[0]));
    let thru = p.hole > 0.0;
    let l = if thru { MULTI } else { p.layer };
    let mut g = head(l);
    for v in [p.x, p.y, p.w, p.h, p.w, p.h, p.w, p.h, p.hole] { g.extend(raw(v).to_le_bytes()) }
    let sb = match p.shape { PadShape::Rect => 2u8, PadShape::Oct => 3, PadShape::Round | PadShape::RoundRect(_) => 1 };
    g.extend([sb, sb, sb]);
    g.extend(p.rot.to_le_bytes());
    g.push(p.plated as u8);
    let mut tail = PAD_TAIL;
    tail[114 - 61..118 - 61].copy_from_slice(&v7_layer(l).to_le_bytes());
    tail[126 - 61..142 - 61].copy_from_slice(&guid_bytes(uids));
    tail[142 - 61..158 - 61].copy_from_slice(&guid_bytes(uids));
    g.extend(tail);
    o.extend(block(&g));
    let slot = thru && p.slot > p.hole;
    let rr = match p.shape { PadShape::RoundRect(pct) => Some(pct.min(100)), _ => None };
    if rr.is_none() && !slot { o.extend(block(&[])); return o }
    let mut s = Vec::with_capacity(596);
    for _ in 0..29 { s.extend(raw(p.w).to_le_bytes()) }
    for _ in 0..29 { s.extend(raw(p.h).to_le_bytes()) }
    s.extend([sb; 29]);
    s.push(0);
    s.push(if slot { 2 } else { 0 });
    s.extend(raw(if slot { p.slot } else { 0.0 }).to_le_bytes());
    s.extend(0f64.to_le_bytes());
    s.extend([0u8; 32 * 4 * 2]);
    s.push(rr.is_some() as u8);
    s.extend([if rr.is_some() { 9 } else { sb }; 32]);
    s.extend([rr.unwrap_or(0); 32]);
    o.extend(block(&s));
    o
}
/// A track (type 4).
pub fn track_rec(t: &Line) -> Vec<u8> {
    let mut g = head(t.layer);
    for v in [t.a.0, t.a.1, t.b.0, t.b.1, t.w] { g.extend(raw(v).to_le_bytes()) }
    g.extend(0i16.to_le_bytes()); g.extend(0i32.to_le_bytes()); g.extend(0i16.to_le_bytes());
    g.extend(v7_layer(t.layer).to_le_bytes()); g.extend([0u8; 4]);
    let mut o = vec![4u8]; o.extend(block(&g)); o
}
/// An arc (type 1).
pub fn arc_rec(a: &Arc) -> Vec<u8> {
    let mut g = head(a.layer);
    for v in [a.c.0, a.c.1, a.r] { g.extend(raw(v).to_le_bytes()) }
    g.extend(a.a0.to_le_bytes()); g.extend(a.a1.to_le_bytes());
    g.extend(raw(a.w).to_le_bytes());
    g.extend(0i16.to_le_bytes()); g.extend(0i32.to_le_bytes()); g.push(0);
    g.extend(v7_layer(a.layer).to_le_bytes()); g.extend([0u8; 4]);
    let mut o = vec![1u8]; o.extend(block(&g)); o
}
/// An extruded component body (type 12): its properties then its outline as doubles.
pub fn body_rec(b: &Body, uids: &mut Uids) -> Vec<u8> {
    let mut g = head(b.layer);
    g.extend(0u32.to_le_bytes()); g.push(0);
    let name = format!("MECHANICAL{}", b.layer.saturating_sub(56).max(1));
    let props = format!("V7_LAYER={name}|NAME= |KIND=0|SUBPOLYINDEX=-1|UNIONINDEX=0|ARCRESOLUTION=0.5mil|ISSHAPEBASED=FALSE|CAVITYHEIGHT=0mil|STANDOFFHEIGHT={z0}|OVERALLHEIGHT={z1}|BODYPROJECTION=0|ARCRESOLUTION=0.5mil|BODYCOLOR3D=8421504|BODYOPACITY3D=1.000|IDENTIFIER=|TEXTURE=|TEXTURECENTERX=0mil|TEXTURECENTERY=0mil|TEXTURESIZEX=0mil|TEXTURESIZEY=0mil|TEXTUREROTATION= 0.00000000000000E+0000|MODELID={id}|MODEL.CHECKSUM=0|MODEL.EMBED=FALSE|MODEL.NAME=|MODEL.2D.X=0mil|MODEL.2D.Y=0mil|MODEL.2D.ROTATION=0.000|MODEL.3D.ROTX=0.000|MODEL.3D.ROTY=0.000|MODEL.3D.ROTZ=0.000|MODEL.3D.DZ=0mil|MODEL.MODELTYPE=0|MODEL.EXTRUDED.MINZ={z0}|MODEL.EXTRUDED.MAXZ={z1}",
        z0 = mil_text(b.z0), z1 = mil_text(b.z1), id = uids.guid());
    let mut pb = super::to_cp1252(&props); pb.push(0);
    g.extend((pb.len() as u32).to_le_bytes()); g.extend(pb);
    g.extend((b.outline.len() as u32).to_le_bytes());
    for (x, y) in &b.outline { g.extend((raw(*x) as f64).to_le_bytes()); g.extend((raw(*y) as f64).to_le_bytes()) }
    let mut o = vec![12u8]; o.extend(block(&g)); o
}

impl Footprint {
    fn count(&self) -> usize { self.pads.len() + self.lines.len() + self.arcs.len() + self.bodies.len() }
    /// The footprint's storage: Header, Parameters, WideStrings, Data, UniqueIDPrimitiveInformation.
    fn streams(&self, key: &str, uids: &mut Uids) -> Vec<(String, Vec<u8>)> {
        let mut data = str_block(&self.name);
        for a in &self.arcs { data.extend(arc_rec(a)) }
        for p in &self.pads { data.extend(pad_rec(p, uids)) }
        for t in &self.lines { data.extend(track_rec(t)) }
        for b in &self.bodies { data.extend(body_rec(b, uids)) }
        let params = Rec(vec![("PATTERN".into(), self.name.clone()), ("HEIGHT".into(), mil_text(self.height)), ("DESCRIPTION".into(), self.description.clone()), ("ITEMGUID".into(), String::new()), ("REVISIONGUID".into(), String::new())]);
        let mut ids = vec![];
        for (i, _) in self.pads.iter().enumerate() {
            let r = Rec(vec![("PRIMITIVEINDEX".into(), i.to_string()), ("PRIMITIVEOBJECTID".into(), "Pad".into()), ("UNIQUEID".into(), uids.next())]);
            ids.extend(pcb_text(&r));
        }
        let mut v = vec![
            (format!("{key}/Header"), (self.count() as u32).to_le_bytes().to_vec()),
            (format!("{key}/Parameters"), pcb_text(&params)),
            (format!("{key}/WideStrings"), vec![1, 0, 0, 0, 0]),
            (format!("{key}/Data"), data),
        ];
        if !self.pads.is_empty() {
            v.push((format!("{key}/UniqueIDPrimitiveInformation/Header"), (self.pads.len() as u32).to_le_bytes().to_vec()));
            v.push((format!("{key}/UniqueIDPrimitiveInformation/Data"), ids));
        }
        v
    }
}

/// A new library's own streams.
fn new_library(uids: &mut Uids) -> Tree {
    let mut t = Tree::default();
    let mut fh = Vec::new();
    let v = "PCB 6.0 Binary Library File";
    fh.extend((v.len() as u32).to_le_bytes()); fh.extend(super::pstr(v));
    fh.extend(5.01f64.to_le_bytes());
    let id = uids.next();
    fh.extend((id.len() as u32).to_le_bytes()); fh.extend(super::pstr(&id));
    t.put("FileHeader", fh);
    let one = 1u32.to_le_bytes().to_vec();
    let zero = 0u32.to_le_bytes().to_vec();
    t.put("Library/Header", one.clone());
    t.put("Library/Data", vec![]);
    let mut lkm = vec![];
    let ver: Vec<u8> = "1.0\0".encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    lkm.extend((ver.len() as u32).to_le_bytes()); lkm.extend(ver); lkm.extend([0u8; 8]);
    t.put("Library/LayerKindMapping/Header", one.clone());
    t.put("Library/LayerKindMapping/Data", lkm);
    let pvl = Rec(vec![("PADVIALIBRARY.LIBRARYID".into(), uids.guid()), ("PADVIALIBRARY.LIBRARYNAME".into(), "<Local>".into()), ("PADVIALIBRARY.DISPLAYUNITS".into(), "1".into())]);
    t.put("Library/PadViaLibrary/Header", zero.clone());
    t.put("Library/PadViaLibrary/Data", pcb_text(&pvl));
    for s in ["Models", "Textures", "ModelsNoEmbed"] { t.put(&format!("Library/{s}/Header"), zero.clone()); t.put(&format!("Library/{s}/Data"), vec![]); }
    t
}

/// The names in `Library/Data` (after its properties block) and the properties block itself.
fn names(t: &Tree) -> Result<(Vec<u8>, Vec<String>), String> {
    let d = t.get("Library/Data").ok_or("no Library/Data: not a footprint library")?;
    if d.len() < 4 { return Err("the library's Data is empty".into()) }
    let n = u32::from_le_bytes([d[0], d[1], d[2], d[3]]) as usize;
    if 4 + n > d.len() { return Err("the library's Data is cut short".into()) }
    let props = d[..4 + n].to_vec();
    let mut r = super::super::Bin::new(&d[4 + n..]);
    let count = r.u32() as usize;
    let mut out = vec![];
    for _ in 0..count.min(100_000) { let blk = r.sub(); if blk.is_empty() { break } out.push(super::super::cp1252(&blk[1..(1 + blk[0] as usize).min(blk.len())])) }
    Ok((props, out))
}
/// name -> storage from `SectionKeys` (a u32 count, then name and key string blocks).
fn section_keys(t: &Tree) -> Vec<(String, String)> {
    let Some(b) = t.get("SectionKeys") else { return vec![] };
    let mut r = super::super::Bin::new(b);
    let n = r.u32() as usize;
    let ps = |r: &mut super::super::Bin| { let blk = r.sub(); if blk.is_empty() { String::new() } else { super::super::cp1252(&blk[1..(1 + blk[0] as usize).min(blk.len())]) } };
    (0..n.min(100_000)).map(|_| { let a = ps(&mut r); let b = ps(&mut r); (a, b) }).collect()
}
fn storage_of(keys: &[(String, String)], name: &str) -> String {
    keys.iter().find(|(n, _)| n == name).map(|(_, k)| k.clone()).unwrap_or_else(|| section_key(name, &HashSet::new()))
}

/// Adds footprints to a library (one of the same name is replaced) and removes the named ones; `existing` None makes a new
/// library. The other footprints' storages are kept byte for byte. Returns the file's bytes.
pub fn write(existing: Option<&[u8]>, fps: &[Footprint], remove: &[String], seed: &str) -> Result<Vec<u8>, String> {
    let mut uids = Uids::new(seed);
    let mut t = match existing { Some(b) => Tree::read(b)?, None => new_library(&mut uids) };
    super::mark_ids(&t, &mut uids);
    let (props, mut list) = if existing.is_some() { names(&t)? } else {
        // Altium reports "Catastrophic failure" loading the Library section without a master layer stack (its defaults fill the rest)
        let p = Rec(vec![("KIND".into(), "Protel_Advanced_PCB_Library".into()), ("VERSION".into(), "3.00".into())]).kv("V9_MASTERSTACK_STYLE", 0).kv("V9_MASTERSTACK_ID", uids.guid())
            .kv("V9_MASTERSTACK_NAME", "Master layer stack").kv("V9_MASTERSTACK_SHOWTOPDIELECTRIC", "FALSE").kv("V9_MASTERSTACK_SHOWBOTTOMDIELECTRIC", "FALSE").kv("V9_MASTERSTACK_ISFLEX", "FALSE");
        (pcb_text(&p), vec![])
    };
    for f in fps { if f.name.trim().is_empty() { return Err("a footprint needs a name".into()) } }
    let keys = section_keys(&t);
    let mut storages: Vec<(String, String)> = list.iter().map(|n| (n.clone(), storage_of(&keys, n))).collect();
    let drop = |n: &str| remove.iter().any(|r| r.eq_ignore_ascii_case(n)) || fps.iter().any(|f| f.name.eq_ignore_ascii_case(n));
    for (n, k) in &storages { if drop(n) { t.remove_dir(k) } }
    storages.retain(|(n, _)| !drop(n));
    let mut taken: HashSet<String> = t.dirs().iter().map(|d| d.to_lowercase()).collect();
    for f in fps {
        let key = section_key(&f.name, &taken);
        taken.insert(key.to_lowercase());
        for (p, v) in f.streams(&key, &mut uids) { t.put(&p, v) }
        storages.push((f.name.clone(), key));
    }
    list = storages.iter().map(|(n, _)| n.clone()).collect();
    let mut data = props;
    data.extend((list.len() as u32).to_le_bytes());
    for n in &list { data.extend(str_block(n)) }
    t.put("Library/Data", data);
    // the table of contents Altium's library panel reads
    let mut toc = String::new();
    for (n, k) in &storages {
        let pads = t.get(&format!("{k}/Data")).map_or(0, |d| primitives(d).iter().filter(|(ty, _)| *ty == 2).count());
        let p = t.get(&format!("{k}/Parameters")).map(|b| if b.len() > 4 { Rec::parse(&b[4..]) } else { Rec::default() }).unwrap_or_default();
        let h = super::super::parse_num(p.get("HEIGHT").unwrap_or("0"));
        toc.push_str(&format!("Name={n}|Pad Count={pads}|Height={}|Description={}\r\n", (h * 10000.0).round() as i64, p.get("DESCRIPTION").unwrap_or("")));
    }
    let mut tb = super::to_cp1252(&toc); tb.push(0);
    t.put("Library/ComponentParamsTOC/Header", 1u32.to_le_bytes().to_vec());
    t.put("Library/ComponentParamsTOC/Data", block(&tb));
    let long: Vec<&(String, String)> = storages.iter().filter(|(n, k)| n != k || n.chars().count() >= 31).collect();
    t.streams.retain(|(k, _)| k != "SectionKeys");
    if !long.is_empty() {
        let mut b = (long.len() as u32).to_le_bytes().to_vec();
        for (n, k) in long { b.extend(str_block(n)); b.extend(str_block(k)) }
        t.put("SectionKeys", b);
    }
    t.streams.sort_by_key(|(k, _)| match k.split('/').next().unwrap_or("") { "FileHeader" => 0, "SectionKeys" => 1, "Library" => 2, "FileVersionInfo" => 4, _ => 3 });
    t.build()
}

/// A footprint's primitives: (type, the record's sub-records joined as the board streams hold them, with the type byte).
pub fn primitives(data: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut r = super::super::Bin::new(data);
    let n = r.u32() as usize; r.skip(n);
    let mut out = vec![];
    while r.left() > 0 {
        let start = r.i;
        let ty = r.u8();
        let subs = match ty { 2 => 6, 5 => 2, 1 | 3 | 4 | 6 | 11 | 12 => 1, _ => break };
        for _ in 0..subs { let _ = r.sub(); }
        if r.i > data.len() { break }
        out.push((ty, data[start..r.i].to_vec()));
    }
    out
}

/// A footprint read back: its name, description, height (mm) and its primitives drawn by the board reader.
pub struct LibFootprint { pub name: String, pub description: String, pub height: f64, pub src: super::super::pcb::Src }
impl LibFootprint {
    /// Its pads: (name, x, y, w, h, hole, layer), mm.
    pub fn pads(&self) -> Vec<(String, f64, f64, f64, f64, f64, u32)> {
        super::super::pcb::pad_recs(&self.src).into_iter().map(|p| (p.name, p.x, p.y, p.top.0, p.top.1, p.hole, p.layer)).collect()
    }
    /// What the board viewers would draw.
    #[cfg(test)]
    pub fn items(&self) -> Vec<crate::files::kicad::board::Item> { super::super::pcb::build(&self.src).0 }
}
/// Every footprint of a library.
pub fn read(bytes: &[u8]) -> Result<Vec<LibFootprint>, String> {
    let t = Tree::read(bytes)?;
    let (_, list) = names(&t)?;
    let keys = section_keys(&t);
    let mut out = vec![];
    for n in list {
        let k = storage_of(&keys, &n);
        let Some(d) = t.get(&format!("{k}/Data")) else { continue };
        let mut m: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
        m.insert("Board6".into(), pcb_text(&Rec::default()));
        for (ty, rec) in primitives(d) {
            let s = match ty { 1 => "Arcs6", 2 => "Pads6", 3 => "Vias6", 4 => "Tracks6", 5 => "Texts6", 6 => "Fills6", 11 => "Regions6", 12 => "ComponentBodies6", _ => continue };
            m.entry(s.into()).or_default().extend(rec);
        }
        let p = t.get(&format!("{k}/Parameters")).map(|b| if b.len() > 4 { Rec::parse(&b[4..]) } else { Rec::default() }).unwrap_or_default();
        out.push(LibFootprint { name: n, description: p.get("DESCRIPTION").unwrap_or("").into(), height: super::super::parse_num(p.get("HEIGHT").unwrap_or("0")) * 0.0254, src: super::super::pcb::Src(m) });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Footprint {
        let mut f = Footprint { name: "TEST-2".into(), description: "two pads".into(), height: 1.2, ..Default::default() };
        f.pads.push(Pad::smd("1", -0.75, 0.0, 0.8, 0.9, PadShape::RoundRect(25)));
        f.pads.push(Pad::smd("2", 0.75, 0.0, 0.8, 0.9, PadShape::Rect));
        let mut t = Pad::thru("3", 0.0, 2.0, 1.6, 1.0, PadShape::Round); t.slot = 2.0; f.pads.push(t);
        f.lines.push(Line { layer: TOP_OVERLAY, a: (-1.5, 1.0), b: (1.5, 1.0), w: 0.15 });
        f.arcs.push(Arc { layer: TOP_OVERLAY, c: (-1.6, 0.0), r: 0.1, a0: 0.0, a1: 360.0, w: 0.2 });
        f.bodies.push(Body { outline: vec![(-1.0, -0.5), (1.0, -0.5), (1.0, 0.5), (-1.0, 0.5)], z0: 0.0, z1: 1.2, layer: mech(13) });
        f
    }
    #[test] fn layers_by_name() {
        assert_eq!(layer("Top Layer"), Some(TOP)); assert_eq!(layer("silk"), Some(TOP_OVERLAY)); assert_eq!(layer("Mechanical15"), Some(71)); assert_eq!(layer("mech1"), Some(57));
        assert_eq!(layer("nope"), None);
        assert_eq!(v7_layer(57), 0x0102_0001);
    }
    #[test] fn record_sizes() {
        let mut u = Uids::new("s");
        let p = pad_rec(&Pad::smd("1", 0.0, 0.0, 1.0, 1.0, PadShape::Rect), &mut u);
        // type, 4 small sub-records, 202-byte geometry, an empty per-layer block
        assert_eq!(p.len(), 1 + 6 + 5 + 9 + 5 + (4 + 202) + 4);
        assert_eq!(track_rec(&Line { layer: 33, a: (0.0, 0.0), b: (1.0, 0.0), w: 0.1 }).len(), 1 + 4 + 49);
        assert_eq!(arc_rec(&Arc { layer: 33, c: (0.0, 0.0), r: 1.0, a0: 0.0, a1: 90.0, w: 0.1 }).len(), 1 + 4 + 60);
        let rr = pad_rec(&Pad::smd("1", 0.0, 0.0, 1.0, 1.0, PadShape::RoundRect(50)), &mut u);
        assert_eq!(rr.len(), p.len() + 596);
    }
    #[test] fn library_round_trip_and_edit() {
        let f = sample();
        let b = write(None, &[f.clone()], &[], "t").unwrap();
        let fps = read(&b).unwrap();
        assert_eq!(fps.len(), 1);
        let r = &fps[0];
        assert_eq!((r.name.as_str(), r.description.as_str()), ("TEST-2", "two pads"));
        assert!((r.height - 1.2).abs() < 1e-5);
        let pads = r.pads();
        assert_eq!(pads.len(), 3);
        let p1 = &pads[0];
        assert_eq!(p1.0, "1"); assert!((p1.1 + 0.75).abs() < 1e-5 && (p1.3 - 0.8).abs() < 1e-5 && (p1.4 - 0.9).abs() < 1e-5 && p1.6 == TOP as u32);
        assert!((pads[2].5 - 1.0).abs() < 1e-5 && pads[2].6 == MULTI as u32);
        // the board reader draws it: pads on copper, the outline on silk, a rounded pad as rounded, the slot as a slot
        let its = r.items();
        let info = |k: &str| its.iter().find(|i| i.key == k).map(|i| i.info.to_string()).unwrap_or_default();
        assert!(info("p/0").contains("roundrect"), "{}", info("p/0"));
        assert!(its.iter().any(|i| i.key == "t/0"));
        assert!(its.iter().any(|i| i.key == "a/0"));
        let pr = super::super::super::pcb::pad_recs(&r.src);
        assert_eq!((pr[2].hole_shape, (pr[2].slot * 1000.0).round()), (2, 2000.0));
        assert_eq!(super::super::super::board3d::bodies(&r.src).len(), 1);
        // add a long-named one, replace the first, then remove it
        let mut long = sample(); long.name = "RHDR40W65P254_2X20_5120X504X850P_EXTRA".into();
        let mut f2 = f.clone(); f2.pads.pop();
        let b2 = write(Some(&b), &[long.clone(), f2], &[], "t2").unwrap();
        let fps = read(&b2).unwrap();
        assert_eq!(fps.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), vec![long.name.as_str(), "TEST-2"]);
        assert_eq!(fps[1].pads().len(), 2);
        let toc = Tree::read(&b2).unwrap().get("Library/ComponentParamsTOC/Data").map(|d| String::from_utf8_lossy(d).into_owned()).unwrap();
        assert!(toc.contains("Name=TEST-2|Pad Count=2|"), "{toc}");
        let b3 = write(Some(&b2), &[], &["test-2".into()], "t3").unwrap();
        assert_eq!(read(&b3).unwrap().len(), 1);
    }
}


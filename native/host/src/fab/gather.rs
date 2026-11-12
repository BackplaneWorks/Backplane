//! Keeping profiles current: a fab's adapter names its published pages and, per value of its profile, where on which page the value
//! is (a row's label, the words before the number, which number, a scale for a mil or a ring). A gather fetches each page (plain HTTP
//! GETs through curl, with its ETag; nothing of the person's is ever sent), reads every value it can, and keeps the rest as they were,
//! with their old dates: never a guess. A value read again gets today's date; a changed one bumps the profile's revision, and the
//! history keeps every revision. Reading is pure (`read_page`, `apply`, tested on saved copies of the pages: test/fab/fixtures/);
//! `refresh` fetches and stores; `start` runs it a while after the hub starts and once a day. Laws `fab_gather_*`.
use super::model::{diff, Change, Limit, Ply, Profile, Stackup, When};
use super::store;
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Duration};

// ---- pages as lines ----

/// The line a table row starts with.
pub const ROW: &str = "⟨row⟩";
const ROW_MARK: &str = "\n⟨row⟩\n";

/// A page's text as lines, the way it reads: block elements, table rows and cells break lines, a cell ends with " |" (an empty cell is a
/// line "|", which ends a row), tags, scripts and
/// styles are dropped, entities decoded, runs of white space made one.
pub fn lines(html: &str) -> Vec<String> {
    let mut s = String::with_capacity(html.len() / 2);
    let low = html.to_ascii_lowercase();
    let b = html.as_bytes(); let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' {
            let end = match html[i..].find('>') { Some(e) => i + e + 1, None => break };
            let tag = low[i + 1..end - 1].trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/' || c == '>').next().unwrap_or("").to_string();
            let closing = low[i + 1..].starts_with('/');
            if !closing && matches!(tag.as_str(), "script" | "style" | "svg" | "noscript") {
                i = low[end..].find(&format!("</{tag}")).map_or(b.len(), |e| end + e); continue
            }
            match tag.as_str() {
                "td" | "th" => s.push_str(if closing { " |" } else { "\n" }),
                "tr" if !closing => s.push_str(ROW_MARK),
                "tr" | "br" | "p" | "li" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "section" | "table" | "ul" | "ol" | "header" => s.push('\n'),
                _ => {}
            }
            i = end;
        } else {
            let next = html[i..].find('<').map_or(b.len(), |e| i + e);
            s.push_str(&html[i..next]); i = next;
        }
    }
    unescape(&s).lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect()
}
fn unescape(s: &str) -> String {
    if !s.contains('&') { return s.to_string() }
    let mut out = String::with_capacity(s.len()); let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]); rest = &rest[i..];
        let Some(e) = rest.char_indices().take(12).find(|(_, c)| *c == ';').map(|(i, _)| i) else { out.push('&'); rest = &rest[1..]; continue };
        let ent = &rest[1..e];
        let ch = match ent { "amp" => Some('&'), "lt" => Some('<'), "gt" => Some('>'), "quot" => Some('"'), "apos" | "#39" => Some('\''), "nbsp" => Some(' '),
            _ if ent.starts_with("#x") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32), _ => None };
        match ch { Some(c) => { out.push(c); rest = &rest[e + 1..] } None => { out.push('&'); rest = &rest[1..] } }
    }
    out + rest
}

/// The text of a table row: from the line that starts with its label (a cell of it) to the next row's mark, at most 60 lines; the
/// label's own cell left out.
pub fn row(ls: &[String], label: &str) -> Option<String> {
    let at = ls.iter().position(|l| l.trim_end_matches('|').trim() == label || l.trim_end_matches('|').trim().starts_with(label))?;
    let first = ls[at].trim_end_matches('|').trim()[label.len()..].trim().to_string();
    let mut v = vec![first];
    for l in ls.iter().skip(at + 1).take(60) { if l == ROW { break } if l != "|" { v.push(l.clone()) } }
    Some(v.join("\n"))
}
/// The nth number after the words (the first number for no words); signs are not taken (+0.13 / -0.08 reads 0.13, 0.08).
pub fn num_after(text: &str, words: &str, nth: usize) -> Option<f64> {
    let start = if words.is_empty() { 0 } else { text.find(words)? + words.len() };
    let mut it = text[start..].char_indices().peekable(); let mut n = 0; let t = &text[start..];
    while let Some((i, c)) = it.next() {
        if c.is_ascii_digit() || (c == '.' && t[i + 1..].starts_with(|d: char| d.is_ascii_digit())) {
            let end = t[i..].find(|d: char| !(d.is_ascii_digit() || d == '.')).map_or(t.len(), |e| i + e);
            let num = t[i..end].trim_end_matches('.');
            if n == nth { return num.parse().ok() }
            n += 1; while it.peek().map_or(false, |(j, _)| *j < end) { it.next(); }
        }
    }
    None
}

// ---- adapters ----

/// Where one value of the profile is: (page, where it sits in the profile ("kind pcb", "assembly economic"), key, cost, its
/// conditions as JSON, how to read it).
pub struct Rule { pub page: &'static str, pub at: &'static str, pub key: &'static str, pub cost: &'static str, pub when: &'static str, pub read: Read }
pub enum Read {
    /// a table row: its label, the words before the number, which number after them, and a factor (mil to mm, a diameter to a ring)
    Row(&'static str, &'static str, usize, f64),
    /// a label on its own line with one line per service after it: which line after it, then the words and which number
    After(&'static str, usize, &'static str, usize),
    /// the same as text
    AfterText(&'static str, usize),
}
pub struct Adapter { pub fab: &'static str, pub pages: &'static [(&'static str, &'static str)], pub rules: &'static [Rule], pub stackups: Option<&'static str> }

const MIL: f64 = 0.0254;
macro_rules! r { ($p:expr, $at:expr, $k:expr, $c:expr, $w:expr, $read:expr) => { Rule { page: $p, at: $at, key: $k, cost: $c, when: $w, read: $read } } }
use Read::*;
const PCB: &str = "kind pcb";
const L12_1: &str = r#"{"layers":[1,2],"copper_oz":[1,1]}"#; const L4_1: &str = r#"{"layers":[4,32],"copper_oz":[1,1]}"#;
const L12_2: &str = r#"{"layers":[1,2],"copper_oz":[2,2]}"#; const L4_2: &str = r#"{"layers":[4,32],"copper_oz":[2,2]}"#;
const L2P: &str = r#"{"layers":[2,32]}"#; const THIN: &str = r#"{"layers":[2,32],"thickness_mm":[0,1.0],"finish":["enig","osp"]}"#;
/// The fab Backplane orders from.
pub const JLCPCB: Adapter = Adapter { fab: "jlcpcb", stackups: Some("impedance"), pages: &[
    ("rigid", "https://jlcpcb.com/capabilities/pcb-capabilities"), ("flex", "https://jlcpcb.com/capabilities/flex-pcb-capabilities"),
    ("assembly", "https://jlcpcb.com/capabilities/pcb-assembly-capabilities"), ("impedance", "https://jlcpcb.com/impedance")], rules: &[
    r!("rigid", PCB, "layers_max", "free", "{}", Row("Layer count", "", 1, 1.0)),
    r!("rigid", PCB, "track_min", "free", L12_1, Row("Min. track width and spacing (1 oz)", "1- and 2-layer:", 0, 1.0)),
    r!("rigid", PCB, "space_min", "free", L12_1, Row("Min. track width and spacing (1 oz)", "1- and 2-layer:", 1, 1.0)),
    r!("rigid", PCB, "track_min", "free", L4_1, Row("Min. track width and spacing (1 oz)", "Multilayer:", 0, 1.0)),
    r!("rigid", PCB, "space_min", "free", L4_1, Row("Min. track width and spacing (1 oz)", "Multilayer:", 1, 1.0)),
    r!("rigid", PCB, "track_min", "free", L12_2, Row("Min. track width and spacing (2 oz)", "2-layer:", 0, 1.0)),
    r!("rigid", PCB, "space_min", "free", L12_2, Row("Min. track width and spacing (2 oz)", "2-layer:", 1, 1.0)),
    r!("rigid", PCB, "track_min", "free", L4_2, Row("Min. track width and spacing (2 oz)", "Multilayer:", 0, 1.0)),
    r!("rigid", PCB, "space_min", "free", L4_2, Row("Min. track width and spacing (2 oz)", "Multilayer:", 1, 1.0)),
    r!("rigid", PCB, "track_min", "free", r#"{"layers":[2,2],"copper_oz":[2.5,2.5]}"#, Row("Min. track width and spacing (2.5 oz)", "2 layer:", 0, 1.0)),
    r!("rigid", PCB, "track_min", "free", r#"{"layers":[2,2],"copper_oz":[3.5,3.5]}"#, Row("Min. track width and spacing (3.5 oz)", "2 layer:", 0, 1.0)),
    r!("rigid", PCB, "track_min", "free", r#"{"layers":[2,2],"copper_oz":[4.5,4.5]}"#, Row("Min. track width and spacing (4.5 oz)", "2 layer:", 0, 1.0)),
    r!("rigid", PCB, "drill_min", "free", r#"{"layers":[1,1]}"#, Row("Drill Diameter", "1-layer:", 0, 1.0)),
    r!("rigid", PCB, "drill_min", "free", L2P, Row("Min. Via hole size/diameter", "Preferred Min. Via hole size:", 0, 1.0)),
    r!("rigid", PCB, "drill_min", "extra", L2P, Row("Drill Diameter", "2-layer:", 0, 1.0)),
    r!("rigid", PCB, "drill_min", "extra", THIN, Row("Drill Diameter", "PCBs is", 0, 1.0)),
    r!("rigid", PCB, "drill_max", "free", "{}", Row("Drill Diameter", "1-layer:", 1, 1.0)),
    r!("rigid", PCB, "via_hole_min", "free", L2P, Row("Min. Via hole size/diameter", "Preferred Min. Via hole size:", 0, 1.0)),
    r!("rigid", PCB, "via_diameter_min", "free", L2P, Row("Min. Via hole size/diameter", "via diameter less than", 0, 1.0)),
    r!("rigid", PCB, "via_hole_min", "extra", L2P, Row("Min. Via hole size/diameter", "Multilayer:", 0, 1.0)),
    r!("rigid", PCB, "via_diameter_min", "extra", L2P, Row("Min. Via hole size/diameter", "Multilayer:", 1, 1.0)),
    r!("rigid", PCB, "via_hole_min", "extra", THIN, Row("Min. Via hole size/diameter", "Multilayer:", 2, 1.0)),
    r!("rigid", PCB, "via_diameter_min", "extra", THIN, Row("Min. Via hole size/diameter", "Multilayer:", 3, 1.0)),
    r!("rigid", PCB, "via_ring_min", "free", "{}", Row("Min. Via hole size/diameter", "Via diameter should be", 0, 0.5)),
    r!("rigid", PCB, "pth_ring_min", "free", L12_1, Row("PTH annular ring", "2-layer:", 2, 1.0)),
    r!("rigid", PCB, "pth_ring_min", "free", L4_1, Row("PTH annular ring", "Multilayer:", 2, 1.0)),
    r!("rigid", PCB, "pth_ring_min", "free", r#"{"copper_oz":[2,4.5]}"#, Row("PTH annular ring", "2 oz:", 0, 1.0)),
    r!("rigid", PCB, "npth_min", "free", "{}", Row("Min. Non-plated holes", "", 0, 1.0)),
    r!("rigid", PCB, "plated_slot_min", "free", r#"{"layers":[1,2]}"#, Row("Min. Plated Slots Width", "2-layer:", 0, 1.0)),
    r!("rigid", PCB, "plated_slot_min", "free", r#"{"layers":[4,32]}"#, Row("Min. Plated Slots Width", "Multi-layer:", 0, 1.0)),
    r!("rigid", PCB, "npth_slot_min", "free", "{}", Row("Min. Non-Plated Slots", "", 0, 1.0)),
    r!("rigid", PCB, "via_hole_to_hole_min", "free", "{}", Row("Via Hole-to-Hole Spacing", "", 0, 1.0)),
    r!("rigid", PCB, "pad_hole_to_hole_min", "free", "{}", Row("Pad Hole-to-Hole Spacing", "", 0, 1.0)),
    r!("rigid", PCB, "castellated_hole_min", "free", "{}", Row("Min. Castellated Holes", "", 0, 1.0)),
    r!("rigid", PCB, "via_to_track_min", "free", "{}", Row("Via hole to Track", "", 0, 1.0)),
    r!("rigid", PCB, "pth_to_track_min", "free", "{}", Row("PTH to Track", "minimum", 0, 1.0)),
    r!("rigid", PCB, "npth_to_track_min", "free", "{}", Row("NPTH to Track", "", 0, 1.0)),
    r!("rigid", PCB, "pad_to_track_min", "free", "{}", Row("Pad to track clearance (1oz)", "", 0, 1.0)),
    r!("rigid", PCB, "smd_pad_to_pad_min", "free", "{}", Row("SMD pad to pad clearance (different nets)", "", 0, 1.0)),
    r!("rigid", PCB, "copper_to_edge_min", "free", "{}", Row("Routed", "routed board edges:", 0, 1.0)),
    r!("rigid", PCB, "copper_to_vcut_min", "free", "{}", Row("V-Cut", "V-cut board edges:", 0, 1.0)),
    r!("rigid", PCB, "silk_line_min", "free", "{}", Row("Minimum Line Width", "", 0, 1.0)),
    r!("rigid", PCB, "silk_text_height_min", "free", "{}", Row("Minimum text height", "(", 0, 1.0)),
    r!("rigid", PCB, "pad_to_silk_min", "free", "{}", Row("Pad To Silkscreen", "", 0, 1.0)),
    r!("rigid", PCB, "mask_bridge_min", "free", r#"{"copper_oz":[1,1]}"#, Row("Soldermask bridge", "1oz:", 0, 1.0)),
    r!("rigid", PCB, "mask_bridge_min", "free", r#"{"copper_oz":[1,1],"color":["black","white"]}"#, Row("Soldermask bridge", "1oz:", 1, 1.0)),
    r!("rigid", PCB, "mask_bridge_min", "free", r#"{"copper_oz":[2,4.5]}"#, Row("Soldermask bridge", "2oz:", 0, 1.0)),
    r!("rigid", PCB, "board_min", "free", "{}", Row("Minimum Dimensions", "FR4/Rogers/PTFE:", 0, 1.0)),
    r!("rigid", PCB, "board_max_x", "free", r#"{"layers":[1,1]}"#, Row("Maximum Dimensions", "FR4(1-layer):", 0, 1.0)),
    r!("rigid", PCB, "board_max_y", "free", r#"{"layers":[1,1]}"#, Row("Maximum Dimensions", "FR4(1-layer):", 1, 1.0)),
    r!("rigid", PCB, "board_max_x", "free", r#"{"layers":[2,2]}"#, Row("Maximum Dimensions", "FR4(2-layer):", 0, 1.0)),
    r!("rigid", PCB, "board_max_y", "free", r#"{"layers":[2,2]}"#, Row("Maximum Dimensions", "FR4(2-layer):", 1, 1.0)),
    r!("rigid", PCB, "board_max_x", "free", r#"{"layers":[4,4]}"#, Row("Maximum Dimensions", "FR4(4-layer):", 0, 1.0)),
    r!("rigid", PCB, "board_max_y", "free", r#"{"layers":[4,4]}"#, Row("Maximum Dimensions", "FR4(4-layer):", 1, 1.0)),
    r!("rigid", PCB, "board_max_x", "free", r#"{"layers":[6,32]}"#, Row("Maximum Dimensions", "FR4(6-layer and above):", 0, 1.0)),
    r!("rigid", PCB, "board_max_y", "free", r#"{"layers":[6,32]}"#, Row("Maximum Dimensions", "FR4(6-layer and above):", 1, 1.0)),
    r!("rigid", PCB, "hole_tolerance_plus", "free", "{}", Row("Hole size Tolerance", "Through-holes:", 0, 1.0)),
    r!("rigid", PCB, "hole_tolerance_minus", "free", "{}", Row("Hole size Tolerance", "Through-holes:", 1, 1.0)),
    r!("rigid", PCB, "impedance_tolerance", "free", r#"{"layers":[4,32]}"#, Row("Impedance Tolerance", "", 0, 1.0)),
    r!("rigid", "kind hdi", "track_min", "free", "{}", Row("HDI", "Minimum trace width / space:", 0, MIL)),
    r!("rigid", "kind hdi", "space_min", "free", "{}", Row("HDI", "Minimum trace width / space:", 1, MIL)),
    r!("rigid", "kind hdi", "track_min", "extra", "{}", Row("HDI", "Minimum trace width / space:", 2, MIL)),
    r!("rigid", "kind hdi", "space_min", "extra", "{}", Row("HDI", "Minimum trace width / space:", 3, MIL)),
    r!("rigid", "kind hdi", "microvia_hole_min", "free", "{}", Row("HDI", "Blind Via Diameter:", 0, 1.0)),
    r!("rigid", "kind hdi", "microvia_hole_max", "free", "{}", Row("HDI", "Blind Via Diameter:", 1, 1.0)),
    r!("rigid", "kind hdi", "buried_via_hole_min", "free", "{}", Row("HDI", "Standard range:", 0, 1.0)),
    r!("rigid", "kind hdi", "buried_via_hole_min", "extra", "{}", Row("HDI", "buried via down to", 0, 1.0)),
    r!("rigid", "kind hdi", "board_min", "free", "{}", Row("HDI", "Minimum dimension:", 0, 1.0)),
    r!("rigid", "kind hdi", "board_max_x", "free", "{}", Row("HDI", "Maximum dimension:", 0, 1.0)),
    r!("rigid", "kind hdi", "board_max_y", "free", "{}", Row("HDI", "Maximum dimension:", 1, 1.0)),
    r!("rigid", "kind aluminum", "drill_min", "free", "{}", Row("Drill Diameter", "aluminum-core PCBs is", 0, 1.0)),
    r!("rigid", "kind copper_core", "drill_min", "free", "{}", Row("Drill Diameter", "copper-core PCBs is", 0, 1.0)),
    r!("rigid", "kind aluminum", "npth_slot_min", "free", "{}", Row("Routed", "aluminum/copper core PCB:", 0, 1.0)),
    r!("rigid", "kind copper_core", "npth_slot_min", "free", "{}", Row("Routed", "aluminum/copper core PCB:", 0, 1.0)),
    r!("rigid", "kind aluminum", "board_min", "free", "{}", Row("Minimum Dimensions", "Aluminum/Copper PCB:", 0, 1.0)),
    r!("rigid", "kind copper_core", "board_min", "free", "{}", Row("Minimum Dimensions", "Aluminum/Copper PCB:", 0, 1.0)),
    r!("rigid", "kind aluminum", "board_max_x", "free", "{}", Row("Maximum Dimensions", "Aluminum PCB:", 0, 1.0)),
    r!("rigid", "kind aluminum", "board_max_y", "free", "{}", Row("Maximum Dimensions", "Aluminum PCB:", 1, 1.0)),
    r!("rigid", "kind copper_core", "board_max_x", "free", "{}", Row("Maximum Dimensions", "Copper PCB:", 0, 1.0)),
    r!("rigid", "kind copper_core", "board_max_y", "free", "{}", Row("Maximum Dimensions", "Copper PCB:", 1, 1.0)),
    r!("rigid", "kind rogers", "board_max_x", "free", "{}", Row("Maximum Dimensions", "Rogers / PTFE Teflon PCB:", 0, 1.0)),
    r!("rigid", "kind rogers", "board_max_y", "free", "{}", Row("Maximum Dimensions", "Rogers / PTFE Teflon PCB:", 1, 1.0)),
    r!("rigid", "kind ptfe", "board_max_x", "free", "{}", Row("Maximum Dimensions", "Rogers / PTFE Teflon PCB:", 0, 1.0)),
    r!("rigid", "kind ptfe", "board_max_y", "free", "{}", Row("Maximum Dimensions", "Rogers / PTFE Teflon PCB:", 1, 1.0)),
    r!("flex", "kind flex", "board_max_y", "free", "{}", Row("Maximum Dimensions", "Regular:", 0, 1.0)),
    r!("flex", "kind flex", "board_max_x", "free", "{}", Row("Maximum Dimensions", "Regular:", 1, 1.0)),
    r!("flex", "kind flex", "drill_min", "free", "{}", Row("Hole Diameter", "", 0, 1.0)),
    r!("flex", "kind flex", "drill_max", "free", "{}", Row("Hole Diameter", "", 1, 1.0)),
    r!("flex", "kind flex", "via_hole_min", "free", "{}", Row("Min. Via hole size/diameter", "Regular:", 0, 1.0)),
    r!("flex", "kind flex", "via_diameter_min", "free", "{}", Row("Min. Via hole size/diameter", "Regular:", 1, 1.0)),
    r!("flex", "kind flex", "via_hole_min", "extra", r#"{"layers":[2,2]}"#, Row("Min. Via hole size/diameter", "Extreme for 2-layer:", 0, 1.0)),
    r!("flex", "kind flex", "via_diameter_min", "extra", r#"{"layers":[2,2]}"#, Row("Min. Via hole size/diameter", "Extreme for 2-layer:", 1, 1.0)),
    r!("flex", "kind flex", "via_hole_min", "extra", r#"{"layers":[4,4]}"#, Row("Min. Via hole size/diameter", "Extreme for 4-layer:", 0, 1.0)),
    r!("flex", "kind flex", "via_diameter_min", "extra", r#"{"layers":[4,4]}"#, Row("Min. Via hole size/diameter", "Extreme for 4-layer:", 1, 1.0)),
    r!("flex", "kind flex", "via_ring_min", "free", "{}", Row("Min. Via hole size/diameter", "at least", 0, 0.5)),
    r!("flex", "kind flex", "pth_ring_min", "free", "{}", Row("Annular Ring for PTH", "absolute limit:", 0, 1.0)),
    r!("flex", "kind flex", "track_min", "free", r#"{"copper_oz":[1,1]}"#, Row("Minimum Trace Width/Spacing (1 oz)", "(1 oz) copper:", 0, MIL)),
    r!("flex", "kind flex", "space_min", "free", r#"{"copper_oz":[1,1]}"#, Row("Minimum Trace Width/Spacing (1 oz)", "(1 oz) copper:", 1, MIL)),
    r!("flex", "kind flex", "track_min", "free", r#"{"copper_oz":[0.5,0.5]}"#, Row("Minimum Trace Width/Spacing (1 oz)", "(0.5 oz) copper:", 0, MIL)),
    r!("flex", "kind flex", "space_min", "free", r#"{"copper_oz":[0.5,0.5]}"#, Row("Minimum Trace Width/Spacing (1 oz)", "(0.5 oz) copper:", 1, MIL)),
    r!("flex", "kind flex", "plated_slot_min", "free", "{}", Row("Minimum Plated Slot", "", 0, 1.0)),
    r!("flex", "kind flex", "castellated_hole_min", "free", "{}", Row("Castellated Holes", "hole diameter:", 0, 1.0)),
    r!("flex", "kind flex", "pad_to_track_min", "free", "{}", Row("Pad-to-Trace Clearance", "Exposed pad to trace", 0, 1.0)),
    r!("flex", "kind flex", "via_to_track_min", "free", "{}", Row("Pad-to-Trace Clearance", "Via ring to trace:", 0, 1.0)),
    r!("flex", "kind flex", "npth_to_track_min", "free", "{}", Row("NPTH to Copper Clearance", "", 0, 1.0)),
    r!("flex", "kind flex", "silk_text_height_min", "free", "{}", Row("Character Height", "", 0, 1.0)),
    r!("flex", "kind flex", "silk_line_min", "free", "{}", Row("Character Line Width", "", 0, 1.0)),
    r!("flex", "kind flex", "mask_bridge_min", "free", "{}", Row("Minimum solder bridge width", "", 0, 1.0)),
    r!("assembly", "assembly economic", "min_package", "free", "{}", AfterText("Minimum Package", 1)),
    r!("assembly", "assembly standard", "min_package", "free", "{}", AfterText("Minimum Package", 2)),
    r!("assembly", "assembly economic", "ic_pitch_min", "free", "{}", After("Minimum IC Pin Spacing", 1, "", 0)),
    r!("assembly", "assembly standard", "ic_pitch_min", "free", "{}", After("Minimum IC Pin Spacing", 2, "", 0)),
    r!("assembly", "assembly economic", "bga_pitch_min", "free", "{}", After("Minimum BGA Spacing", 1, "", 0)),
    r!("assembly", "assembly standard", "bga_pitch_min", "free", "{}", After("Minimum BGA Spacing", 2, "", 0)),
    r!("assembly", "assembly economic", "reflow_c", "free", "{}", After("Reflow temperature", 1, "", 0)),
    r!("assembly", "assembly standard", "reflow_c", "free", "{}", After("Reflow temperature", 2, "", 0)),
    r!("assembly", "assembly economic", "qty_min", "free", "{}", After("Order Volume", 1, "", 0)),
    r!("assembly", "assembly economic", "qty_max", "free", "{}", After("Order Volume", 1, "", 1)),
    r!("assembly", "assembly standard", "qty_min", "free", "{}", After("Order Volume", 2, "", 0)),
    r!("assembly", "assembly economic", "thickness_min", "free", "{}", After("Thickness", 1, "", 0)),
    r!("assembly", "assembly economic", "thickness_max", "free", "{}", After("Thickness", 1, "", 1)),
]};
pub const ADAPTERS: &[&Adapter] = &[&JLCPCB];
pub fn adapter(fab: &str) -> Option<&'static Adapter> { ADAPTERS.iter().copied().find(|a| a.fab == fab) }

fn cell(l: &str) -> &str { l.trim().trim_end_matches('|').trim() }
/// The lines with text, cell ends and row marks left out.
fn content(ls: &[String]) -> Vec<&str> { ls.iter().map(|l| cell(l)).filter(|l| !l.is_empty() && *l != ROW).collect() }
fn round(v: f64) -> f64 { (v * 1000.0).round() / 1000.0 }
/// What one rule reads off its page's lines (none: not there).
pub fn read(ls: &[String], how: &Read) -> Option<Value> {
    match how {
        Row(label, words, nth, scale) => {
            let t = row(ls, label)?;
            let v = num_after(&t, words, *nth)?;
            Some(json!(if (*scale - 1.0).abs() < 1e-12 { v } else { round(v * scale) }))
        }
        After(label, k, words, nth) => { let cl = content(ls); let at = cl.iter().position(|l| *l == *label)?; num_after(cl.get(at + k)?, words, *nth).map(|v| json!(v)) }
        AfterText(label, k) => { let cl = content(ls); let at = cl.iter().position(|l| *l == *label)?; cl.get(at + k).map(|v| json!(v)) }
    }
}

/// The published stackups of an impedance page: "N) <id> Stackup" blocks of copper, prepreg and core lines, the prepregs' Er from the
/// page's own table, the core's from its "Core dielectric constant".
pub fn stackups(ls: &[String], page: &str, url: &str, today: &str) -> Vec<Stackup> {
    let mut er: Vec<(String, f64)> = vec![];
    // "1.Prepreg dielectric constant: | Prepreg type | Dielectric constant | 7628 | 4.4 | 3313 | 4.1 ...": the pairs after the heading
    let cl = content(ls);
    let head = cl.iter().position(|l| l.contains("Prepreg dielectric constant"));
    if let Some(i) = head.and_then(|h| cl.iter().skip(h + 1).take(6).position(|l| *l == "Dielectric constant").map(|j| h + 1 + j)) {
        let mut k = i + 1;
        while k + 1 < cl.len() && cl[k].len() >= 3 && cl[k].chars().all(|d| d.is_ascii_digit()) { if let Ok(v) = cl[k + 1].parse::<f64>() { er.push((cl[k].to_string(), v)) } k += 2 }
    }
    let core_er = ls.iter().position(|l| l.trim_end_matches('|').trim() == "Core dielectric constant").and_then(|i| ls.iter().skip(i + 1).take(3).find_map(|l| l.trim_matches(|c: char| c == '|' || c.is_whitespace()).parse::<f64>().ok()));
    let thick = |l: &str| -> Option<f64> { l.strip_suffix("mm").and_then(|n| n.trim().parse().ok()) };
    let mut out: Vec<Stackup> = vec![];
    let mut i = 0;
    while i < ls.len() {
        let l = &ls[i];
        let id = l.split_once(") ").and_then(|(n, rest)| if n.chars().all(|c| c.is_ascii_digit()) { rest.strip_suffix(" Stackup") } else { None });
        let Some(id) = id.filter(|id| id.starts_with("JLC") && id.len() >= 9) else { i += 1; continue };
        let (layers, t) = (id[3..5].parse::<u32>().ok(), id[5..7].parse::<f64>().ok().map(|t| t / 10.0));
        let mut seq = vec![]; let mut j = i + 1;
        while j < ls.len() && !ls[j].ends_with(" Stackup") {
            let w = ls[j].as_str();
            if w == "Top Layer" || w == "Bottom Layer" { if let Some(t) = ls.get(j + 2).and_then(|x| thick(x)) { seq.push(Ply { kind: "copper".into(), material: String::new(), thickness_mm: t, er: None }); j += 3; continue } }
            if w == "Prepreg" { let mat = ls.get(j + 1).map(|m| m.split('*').next().unwrap_or("").to_string()).unwrap_or_default();
                if let Some(t) = ls.get(j + 2).and_then(|x| thick(x)) { seq.push(Ply { kind: "prepreg".into(), er: er.iter().find(|(m, _)| *m == mat).map(|x| x.1), material: mat, thickness_mm: t }); j += 3; continue } }
            // a core with no copper of its own: Core / Core / 0.7mm / "0.7 H/HOZ without copper"
            if w == "Core" && ls.get(j + 1).map(String::as_str) == Some("Core") { if let Some(t) = ls.get(j + 2).and_then(|x| thick(x)) { seq.push(Ply { kind: "core".into(), material: String::new(), thickness_mm: t, er: core_er }); j += 3; continue } }
            if w.starts_with("Inner Layer L") {
                // Inner Layer Ln / Core / Inner Layer Lm / Copper / Core / Copper / cu / core / cu / "<core>mm ..."
                let ts: Vec<f64> = ls.iter().skip(j).take(12).filter_map(|x| thick(x)).take(3).collect();
                if ts.len() == 3 { seq.push(Ply { kind: "copper".into(), material: String::new(), thickness_mm: ts[0], er: None }); seq.push(Ply { kind: "core".into(), material: String::new(), thickness_mm: ts[1], er: core_er }); seq.push(Ply { kind: "copper".into(), material: String::new(), thickness_mm: ts[2], er: None }) }
                j = ls.iter().enumerate().skip(j).find(|(_, x)| x.contains("mm H/H")).map_or(j + 1, |(k, _)| k + 1); continue
            }
            j += 1;
        }
        let coppers = seq.iter().filter(|p| p.kind == "copper").count() as u32;
        if let (Some(layers), Some(t)) = (layers, t) { if coppers == layers && !out.iter().any(|s| s.id == id) {
            out.push(Stackup { id: id.into(), layers, thickness_mm: t, outer_oz: 1.0, inner_oz: if id.as_bytes().get(8) == Some(&b'H') { 0.5 } else { 1.0 }, default: false, page: page.into(), source: url.into(), fetched: today.into(), seq })
        } }
        i = j;
    }
    out
}

/// One gather's outcome.
#[derive(Debug, Default)]
pub struct Report { pub read: usize, pub missing: Vec<String>, pub changes: Vec<Change>, pub pages_failed: Vec<String> }
impl Report { pub fn json(&self) -> Value { json!({"read": self.read, "missing": self.missing, "changes": self.changes.iter().map(Change::line).collect::<Vec<_>>(), "pages_failed": self.pages_failed}) } }

fn find_mut<'a>(p: &'a mut Profile, at: &str, key: &str, cost: &str, when: &When) -> Option<&'a mut Limit> {
    let s = if let Some(k) = at.strip_prefix("kind ") { p.kinds.get_mut(k)? } else { p.assembly.get_mut(at.strip_prefix("assembly ")?)? };
    s.limits.iter_mut().find(|l| l.key == key && l.cost == cost && l.when == *when)
}

/// Fab.gather.apply: a profile with every value its pages gave read again (today's date), the others as they were; pages that
/// could not be had are `None` and change nothing. The revision goes up when a value changed.
pub fn apply(old: &Profile, a: &Adapter, pages: &[(&str, Option<String>)], today: &str) -> (Profile, Report) {
    let mut p = old.clone(); let mut rep = Report::default();
    let lined: Vec<(&str, Option<Vec<String>>)> = pages.iter().map(|(id, h)| (*id, h.as_ref().map(|h| lines(h)))).collect();
    let get = |id: &str| lined.iter().find(|(i, _)| *i == id).and_then(|(_, l)| l.as_ref());
    for (id, h) in pages { if h.is_none() { rep.pages_failed.push(id.to_string()) } }
    for r in a.rules {
        let Some(ls) = get(r.page) else { continue };
        let when: When = serde_json::from_str(r.when).unwrap_or_default();
        let name = format!("{} {} ({})", r.at, r.key, r.cost);
        let Some(l) = find_mut(&mut p, r.at, r.key, r.cost, &when) else { rep.missing.push(format!("{name}: no such value in the profile")); continue };
        match read(ls, &r.read) {
            Some(v) => { l.value = v; l.fetched = today.into(); rep.read += 1 }
            None => rep.missing.push(format!("{name}: not found on {}", r.page)),
        }
    }
    if let Some(sp) = a.stackups { if let Some(ls) = get(sp) {
        let url = a.pages.iter().find(|(i, _)| *i == sp).map_or("", |x| x.1);
        let mut st = stackups(ls, sp, url, today);
        if st.is_empty() { rep.missing.push(format!("stackups: none found on {sp}")) } else {
            for s in st.iter_mut() { s.default = old.stackups.iter().any(|o| o.id == s.id && o.default) }
            // a stackup no longer published is dropped only when the page gave others
            p.stackups = st;
        }
    } }
    for (id, h) in pages { if h.is_some() { if let Some(pg) = p.pages.get_mut(*id) { pg.fetched = today.into(); pg.sha256 = sha(h.as_deref().unwrap_or("")) } } }
    rep.changes = diff(old, &p);
    if !rep.changes.is_empty() { p.rev = old.rev + 1 }
    if rep.read > 0 { p.updated = today.into() }
    (p, rep)
}
fn sha(s: &str) -> String { use sha2::{Digest, Sha256}; hex::encode(Sha256::digest(s.as_bytes())) }

// ---- fetching ----

pub const AGENT: &str = "Backplane-fab-profile/1 (+https://github.com/BackplaneWorks/Backplane; reads published capability pages)";
/// One GET: (status, body). The URL never on a command line (a curl config), the body to a file; 30 s at most.
fn get(home: &Path, url: &str) -> (String, String) {
    use crate::plat::fs::OpenOptionsExt;
    let dir = store::dir(home).join("tmp"); if std::fs::create_dir_all(&dir).is_err() { return ("000".into(), String::new()) }
    let n = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    let (cfg, out) = (dir.join(format!("g-{n}.curl")), dir.join(format!("g-{n}.html")));
    let q = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    let c = format!("url = {}\nuser-agent = {}\noutput = {}\nwrite-out = \"%{{http_code}}\"\nlocation\nmax-filesize = 8000000\nproto = \"=https\"\n", q(url), q(AGENT), q(&out.to_string_lossy()));
    let ok = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&cfg).and_then(|mut f| { use std::io::Write; f.write_all(c.as_bytes()) }).is_ok();
    let r = if ok { crate::proc::std_cmd(crate::proc::Kind::Net, "curl").args(["-sS", "--max-time", "30", "-K"]).arg(&cfg).output().ok() } else { None };
    let code = r.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|c| c.len() == 3).unwrap_or_else(|| "000".into());
    let body = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&cfg); let _ = std::fs::remove_file(&out);
    (code, body)
}
pub fn today() -> String {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) / 86400;
    // civil from days (Howard Hinnant)
    let z = d as i64 + 719468; let era = z.div_euclid(146097); let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; let y = yoe + era * 400; let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153; let dd = doy - (153 * mp + 2) / 5 + 1; let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, dd)
}
/// Whether this hub may reach the fab's pages: never on a copy or a test hub (stand-in agents), nor with BACKPLANE_FAB_OFFLINE=1.
pub fn may_fetch() -> bool { !crate::copy::on() && std::env::var_os("BACKPLANE_AGENTS").is_none() && std::env::var("BACKPLANE_FAB_OFFLINE").map_or(true, |v| v != "1") }

/// Fab.refresh: fetch a fab's pages (a second apart), read them, keep the new profile and say what changed. Blocking.
pub fn refresh(home: &Path, fab: &str) -> Result<(Profile, Report), String> {
    if !may_fetch() { return Err("this hub does not reach the fab's pages (a copy, a test hub or BACKPLANE_FAB_OFFLINE)".into()) }
    let a = adapter(fab).ok_or_else(|| format!("no gatherer for the fab {fab:?}"))?;
    let old = store::load(home, fab).ok_or_else(|| format!("no profile of the fab {fab:?}"))?;
    let mut pages: Vec<(&str, Option<String>)> = vec![];
    for (i, (id, url)) in a.pages.iter().enumerate() {
        if i > 0 { std::thread::sleep(Duration::from_secs(1)) }
        let (code, body) = get(home, url);
        pages.push((id, if code.starts_with('2') && body.len() > 1000 { Some(body) } else { None }));
    }
    if pages.iter().all(|(_, b)| b.is_none()) { return Err("none of the fab's pages could be read (offline?); the profile stays as it was".into()) }
    let (new, rep) = apply(&old, a, &pages, &today());
    store::save(home, &new)?;
    let _ = std::fs::write(store::dir(home).join(format!("gathered-{fab}.json")), serde_json::to_string_pretty(&json!({"at": today(), "rev": new.rev, "report": rep.json()})).unwrap_or_default());
    Ok((new, rep))
}
pub fn last(home: &Path, fab: &str) -> Value { std::fs::read_to_string(store::dir(home).join(format!("gathered-{fab}.json"))).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null) }

/// Refresh every fab with a gatherer, 3 minutes after start and then every day; each change is said to clients as
/// `{t:"fab.changed", fab, rev, changes, projects}` (projects: the open projects on that fab whose own values changed).
pub fn start(hub: &Arc<crate::hub::Hub>) {
    if !may_fetch() { return }
    let hub = hub.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(180)).await;
        loop {
            for a in ADAPTERS { let _ = refresh_said(&hub, a.fab).await; }
            tokio::time::sleep(Duration::from_secs(24 * 3600)).await;
        }
    });
}
/// One refresh, said to clients when something changed.
pub async fn refresh_said(hub: &Arc<crate::hub::Hub>, fab: &str) -> Result<Value, String> {
    let (home, f) = (hub.cfg.home.clone(), fab.to_string());
    let (p, rep) = tokio::task::spawn_blocking(move || refresh(&home, &f)).await.map_err(|_| "failed".to_string())??;
    if !rep.changes.is_empty() {
        let roots: Vec<(String, String)> = { let m = hub.model().await; m.projects.iter().filter(|(_, p)| !p.removed).map(|(id, p)| (id.clone(), p.root.clone())).collect() };
        let keys: Vec<String> = rep.changes.iter().map(|c| c.key.clone()).collect(); let fab2 = fab.to_string();
        let projects = tokio::task::spawn_blocking(move || roots.into_iter().filter(|(_, r)| { let c = super::choice(Path::new(r)); c.fab == fab2 }).map(|(id, _)| id).collect::<Vec<_>>()).await.unwrap_or_default();
        hub.push_all(&json!({"t": "fab.changed", "fab": fab, "rev": p.rev, "changes": rep.changes.iter().map(Change::line).collect::<Vec<_>>(), "keys": keys, "projects": projects}));
    }
    Ok(json!({"fab": fab, "rev": p.rev, "updated": p.updated, "report": rep.json()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(n: &str) -> String { std::fs::read_to_string(format!("{}/../../test/fab/fixtures/jlcpcb/{n}.html", env!("CARGO_MANIFEST_DIR"))).unwrap() }
    fn pages() -> Vec<(&'static str, Option<String>)> { JLCPCB.pages.iter().map(|(id, _)| (*id, Some(fixture(id)))).collect() }
    #[test] fn fab_gather_lines_read_a_page_as_text() {
        let ls = lines("<p>a &amp; b</p><script>x<y</script><table><tr><td>Via Hole-to-Hole Spacing</td><td>0.2mm</td></tr><tr><td></td></tr></table><div>&#8805;0.5&nbsp;mm</div>");
        assert_eq!(ls, ["a & b", ROW, "Via Hole-to-Hole Spacing |", "0.2mm |", ROW, "|", "≥0.5 mm"]);
        assert_eq!(num_after("Through-holes: +0.13 / -0.08 mm", "Through-holes:", 1), Some(0.08)); assert_eq!(num_after("FR4(2-layer): 670 × 600 mm", "FR4(2-layer):", 1), Some(600.0));
        assert_eq!(num_after("x", "nope", 0), None); assert_eq!(num_after("1.5mm.", "", 0), Some(1.5));
    }
    /// Every value the adapter reads is on the saved pages and is what the bundled profile says (the baseline was read off these pages).
    #[test] fn fab_gather_the_adapter_reads_every_value_of_the_saved_pages_as_the_baseline_has_them() {
        let base = store::baseline("jlcpcb").unwrap();
        let (p, rep) = apply(&base, &JLCPCB, &pages(), "2026-11-12");
        assert!(rep.missing.is_empty(), "{:#?}", rep.missing); assert_eq!(rep.read, JLCPCB.rules.len());
        let value_changes: Vec<String> = rep.changes.iter().filter(|c| !c.at.starts_with("stackup")).map(Change::line).collect();
        assert!(value_changes.is_empty(), "the saved pages say what the baseline says: {value_changes:#?}");
        for s in &base.stackups { let n = p.stackups.iter().find(|x| x.id == s.id).unwrap_or_else(|| panic!("{} not read", s.id)); assert_eq!(n.seq, s.seq, "{}", s.id); assert_eq!(n.default, s.default) }
        assert!(p.stackups.len() >= 10 && p.stackups.iter().any(|s| s.layers == 6), "every published stackup: {}", p.stackups.len());
        for s in &p.stackups { let t: f64 = s.seq.iter().map(|x| x.thickness_mm).sum(); assert!((t - s.thickness_mm).abs() <= 0.2, "{} adds up to {t} (the fab lists 2116E at 1.446 mm for a 1.6 mm board)", s.id); assert!(s.seq.iter().filter(|x| x.kind != "copper" && x.material != "2313").all(|x| x.er.is_some()), "{}: an Er for every dielectric the page's table names (2313 is not in it, so it has none)", s.id) }
    }
    #[test] fn fab_gather_a_changed_page_changes_the_value_and_the_revision_and_a_missing_one_keeps_the_old() {
        let base = store::baseline("jlcpcb").unwrap();
        let mut ps = pages(); let rigid = ps[0].1.take().unwrap();
        ps[0].1 = Some(rigid.replace("Copper clearance from routed board edges: ≧0.2 mm", "Copper clearance from routed board edges: ≧0.25 mm"));
        let (p, rep) = apply(&base, &JLCPCB, &ps, "2027-02-01");
        assert_eq!(rep.changes.iter().filter(|c| !c.at.starts_with("stackup")).map(Change::line).collect::<Vec<_>>(), ["kind pcb copper_to_edge_min (free): 0.2 -> 0.25"]);
        assert_eq!(p.rev, base.rev + 1); let l = p.kinds["pcb"].limits.iter().find(|l| l.key == "copper_to_edge_min").unwrap(); assert_eq!((l.fetched.as_str(), p.pages["rigid"].fetched.as_str()), ("2027-02-01", "2027-02-01"));
        // the flex page could not be had: its values keep their value and their old date; a row gone from a page is said, not guessed
        let mut ps = pages(); ps[1].1 = None; ps[0].1 = Some(rigid.replace("Pad To Silkscreen", "Pad Clearance To Legend"));
        let (p, rep) = apply(&base, &JLCPCB, &ps, "2027-02-01");
        assert_eq!(rep.pages_failed, ["flex"]); assert!(rep.missing.contains(&"kind pcb pad_to_silk_min (free): not found on rigid".to_string()), "{:?}", rep.missing);
        let f = p.kinds["flex"].limits.iter().find(|l| l.key == "drill_min").unwrap(); assert_eq!((f.fetched.as_str(), p.pages["flex"].fetched.as_str()), ("2026-11-12", "2026-11-12"));
        let s = p.kinds["pcb"].limits.iter().find(|l| l.key == "pad_to_silk_min").unwrap(); assert_eq!((s.value.clone(), s.fetched.as_str()), (json!(0.15), "2026-11-12"));
        assert_eq!(p.rev, base.rev + rep.changes.len().min(1) as u64);
    }
    #[test] fn fab_gather_today_is_a_civil_date() { let t = today(); assert_eq!((t.len(), &t[4..5], &t[7..8]), (10, "-", "-")); assert!(t.as_str() >= "2026-01-01") }
}
/// The stackups of the saved impedance page as JSON (to keep the bundled baseline's list whole).
#[cfg(test)]
#[test] #[ignore] fn fab_gather_dump_stackups() { let ls = lines(&std::fs::read_to_string(format!("{}/../../test/fab/fixtures/jlcpcb/impedance.html", env!("CARGO_MANIFEST_DIR"))).unwrap()); println!("STACKUPS {}", serde_json::to_string(&stackups(&ls, "impedance", "", "")).unwrap()) }
/// A real refresh into the home FAB_LIVE_HOME (a temp home), by hand only: it reaches the fab's pages.
#[cfg(test)]
#[test] #[ignore] fn fab_gather_live_refresh() { let Ok(h) = std::env::var("FAB_LIVE_HOME") else { return }; std::env::remove_var("BACKPLANE_AGENTS"); match refresh(Path::new(&h), "jlcpcb") { Ok((p, r)) => println!("LIVE rev {} {}", p.rev, r.json()), Err(e) => println!("LIVE error {e}") } }
#[cfg(test)]
#[test] #[ignore] fn fab_gather_dump_lines() { let n = std::env::var("FAB_PAGE").unwrap_or("rigid".into()); for (i, l) in lines(&std::fs::read_to_string(format!("{}/../../test/fab/fixtures/jlcpcb/{n}.html", env!("CARGO_MANIFEST_DIR"))).unwrap()).iter().enumerate() { eprintln!("{i}: {l}") } }

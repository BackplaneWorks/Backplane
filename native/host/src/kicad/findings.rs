//! Design findings: one list for everything that checks a design (DRC and ERC now; later the design review's datasheet, BOM, Gerber
//! and fab-rule checks), each finding anchored to the items and the place it is about, so any client (desktop panel, phones beside
//! their viewers) can list it, light it and cross-probe it, and an agent can be handed it to fix. Pure: kicad-cli's JSON reports in,
//! findings, counts, the re-run policy and the messages out. The hub side (runs, state, RPCs) is kicad/checkrun.rs.
use serde_json::{json, Value};
use std::collections::HashMap;

/// What a finding points at: one item of the design (a pad, a symbol pin, a track, a label), or only a place.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Anchor {
    /// "pcb" or "sch"
    pub doc: String,
    /// KiCad's own words for the item ("Pad 1 [GND] of C21 on B.Cu")
    pub text: String,
    /// the item's KIID, when the report has it
    pub uuid: String,
    pub reference: String,
    /// "<reference>.<pin>" ("U3.4")
    pub pin: String,
    pub net: String,
    pub layer: String,
    /// the schematic sheet's path ("/Power/")
    pub sheet: String,
    /// millimetres, in the document's own frame; `at` false when the report gave no position
    pub x: f64,
    pub y: f64,
    pub at: bool,
}

/// One finding. `source` says which check made it ("drc", "erc"; later "datasheet", "bom", "gerber", "fab"); `severity` is
/// "error", "warning", "info" or "excluded"; `rule` the checker's own name for it ("clearance", "pin_not_driven"); `id` is stable
/// across runs while the same items keep the same problem.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Finding { pub id: String, pub source: String, pub severity: String, pub rule: String, pub title: String, pub detail: String, pub anchors: Vec<Anchor>, pub fix: String }

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counts { pub error: usize, pub warning: usize, pub info: usize, pub excluded: usize }
impl Counts {
    pub fn to_json(&self) -> Value { json!({"error": self.error, "warning": self.warning, "info": self.info, "excluded": self.excluded}) }
    /// What is not excluded.
    pub fn open(&self) -> usize { self.error + self.warning + self.info }
}
pub fn counts(fs: &[Finding]) -> Counts {
    let mut c = Counts::default();
    for f in fs { match f.severity.as_str() { "error" => c.error += 1, "warning" => c.warning += 1, "excluded" => c.excluded += 1, _ => c.info += 1 } }
    c
}

// Reading KiCad's words for an item -------------------------------------------------------------------------------------------

fn is_ref(s: &str) -> bool { super::xprobe::is_reference(s) }
/// The word after the last " on " up to a comma or " - " ("Micro via [GND] on F.Cu - In1.Cu" -> "F.Cu").
fn layer_of(d: &str) -> String {
    let Some(i) = d.rfind(" on ") else { return String::new() };
    d[i + 4..].split([',', ' ']).next().unwrap_or("").to_string()
}
/// The first [..]: a board item's net ("<no net>" is none).
fn bracket(d: &str) -> String {
    let (Some(a), Some(b)) = (d.find('['), d.find(']')) else { return String::new() };
    if b <= a { return String::new() }
    let n = &d[a + 1..b];
    if n.starts_with('<') { String::new() } else { n.to_string() }
}
fn quoted(d: &str) -> String { let mut it = d.splitn(3, '\''); it.next(); match (it.next(), it.next()) { (Some(q), Some(_)) => q.to_string(), _ => String::new() } }
fn words(d: &str) -> Vec<&str> { d.split(|c: char| c == ' ' || c == ',').filter(|w| !w.is_empty()).collect() }

/// A board item: its part ("of C21", "Footprint C64"), pad ("Pad 1 ... of C21", "PTH pad SH ... of J5"), net, layer.
pub fn pcb_anchor(d: &str) -> Anchor {
    let ws = words(d);
    let mut a = Anchor { doc: "pcb".into(), text: d.into(), layer: layer_of(d), net: bracket(d), ..Default::default() };
    if let Some(i) = ws.iter().position(|w| *w == "of") { if let Some(r) = ws.get(i + 1).filter(|r| is_ref(r)) { a.reference = r.to_string() } }
    if a.reference.is_empty() && ws.first() == Some(&"Footprint") { if let Some(r) = ws.get(1).filter(|r| is_ref(r)) { a.reference = r.to_string() } }
    if !a.reference.is_empty() {
        if let Some(i) = ws.iter().position(|w| w.eq_ignore_ascii_case("pad")) {
            if let Some(n) = ws.get(i + 1).filter(|n| !n.starts_with('[') && **n != "of") { a.pin = format!("{}.{n}", a.reference) }
        }
    }
    a
}
/// A schematic item: "Symbol U5 Pin F2 [~{RESET}, Input, Line]", "Symbol D1 [LED_ARGB]", "Label 'SWD.CLK'", "Global Label 'X'".
pub fn sch_anchor(d: &str, sheet: &str) -> Anchor {
    let ws = words(d);
    let mut a = Anchor { doc: "sch".into(), text: d.into(), sheet: sheet.into(), ..Default::default() };
    if ws.first() == Some(&"Symbol") {
        if let Some(r) = ws.get(1).filter(|r| is_ref(r)) {
            a.reference = r.to_string();
            if ws.get(2) == Some(&"Pin") { if let Some(n) = ws.get(3).filter(|n| !n.starts_with('[')) { a.pin = format!("{r}.{n}") } }
        }
    } else if ws.iter().any(|w| *w == "Label") {
        a.net = quoted(d);
    } else if let Some(i) = ws.iter().position(|w| *w == "of") {
        if let Some(r) = ws.get(i + 1).filter(|r| is_ref(r)) { a.reference = r.to_string() }
    }
    a
}

// Ids, fixes, the reports ------------------------------------------------------------------------------------------------------

fn fnv64(s: &str) -> u64 { s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3)) }
/// A finding's id from what it is about, not how big it is: source, rule, sheet and the items' KIIDs (sorted), or with no KIIDs the
/// items' words. The measured numbers in the title ("actual 0.0250 mm") are left out, so moving a track a little keeps the id.
pub fn base_id(f: &Finding) -> String {
    let mut keys: Vec<String> = f.anchors.iter().map(|a| if a.uuid.is_empty() { a.text.clone() } else { a.uuid.clone() }).collect();
    keys.sort();
    let sheet = f.anchors.first().map(|a| a.sheet.as_str()).unwrap_or("");
    let what = if keys.is_empty() { f.title.clone() } else { keys.join("\u{1}") };
    format!("{}:{:016x}", f.source, fnv64(&format!("{}\u{2}{}\u{2}{sheet}\u{2}{what}", f.source, f.rule)))
}
/// Ids for a whole run: a repeat of the same id (two findings of one rule on the same items) gets "~2", "~3"... in report order.
pub fn assign_ids(fs: &mut [Finding]) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for f in fs.iter_mut() {
        let b = base_id(f);
        let n = seen.entry(b.clone()).or_insert(0); *n += 1;
        f.id = if *n == 1 { b } else { format!("{b}~{n}") };
    }
}
/// A short hint of the usual fix, for the person and the agent (empty when there is no general one).
pub fn fix_hint(source: &str, rule: &str) -> &'static str {
    match (source, rule) {
        ("drc", "clearance") | ("drc", "hole_clearance") | ("drc", "hole_to_hole") => "Move the items apart, or set the clearance the design really needs in the board's rules",
        ("drc", "shorting_items") => "Two nets touch: move or reroute one of the items",
        ("drc", "unconnected_items") => "Route the missing connection (or refill the zones if a zone should join them)",
        ("drc", "track_dangling") | ("drc", "via_dangling") => "Remove the stub, or route it to where it was going",
        ("drc", "courtyards_overlap") | ("drc", "pth_inside_courtyard") | ("drc", "npth_inside_courtyard") => "Move the parts apart so their courtyards no longer overlap",
        ("drc", "copper_outside_board") | ("drc", "copper_edge_clearance") => "Move the copper inside the board outline, keeping the edge clearance",
        ("drc", "silk_overlap") | ("drc", "silk_over_copper") | ("drc", "silk_edge_clearance") => "Move or shrink the silkscreen text or graphic",
        ("drc", "solder_mask_bridge") => "Separate the pads or adjust the solder mask expansion",
        ("drc", "lib_footprint_mismatch") | ("drc", "lib_footprint_issues") => "Update the footprint from its library, or accept the local change",
        ("drc", "isolated_copper") => "Remove the island or connect it (zone settings: remove islands)",
        ("drc", "starved_thermal") => "Give the pad more thermal spokes or a direct connection",
        ("drc", "extra_footprint") | ("drc", "missing_footprint") | ("drc", "footprint_symbol_mismatch") | ("drc", "net_conflict") => "Update the board from the schematic",
        ("erc", "pin_not_connected") => "Connect the pin, or place a no-connect flag on it",
        ("erc", "pin_not_driven") | ("erc", "power_pin_not_driven") => "Drive the net from an output or power pin, or add a power flag",
        ("erc", "label_dangling") | ("erc", "global_label_dangling") => "Connect the label to a wire, or remove it",
        ("erc", "wire_dangling") | ("erc", "unconnected_wire_endpoint") => "Connect or remove the dangling wire",
        ("erc", "lib_symbol_mismatch") | ("erc", "lib_symbol_issues") => "Update the symbol from its library, or accept the local change",
        ("erc", "footprint_link_issues") => "Set a footprint that exists in the libraries",
        ("erc", "duplicate_reference") => "Annotate the schematic again",
        ("erc", "pin_to_pin") => "Check the two pins' electrical types; one of them may be set wrong",
        _ => "",
    }
}
fn severity_of(v: &Value) -> String {
    if v["excluded"].as_bool().unwrap_or(false) { return "excluded".into() }
    match v["severity"].as_str().unwrap_or("warning") { "error" => "error", "exclusion" | "excluded" | "ignore" => "excluded", "info" | "action" => "info", _ => "warning" }.into()
}
fn finding_of(source: &str, v: &Value, anchor: impl Fn(&str) -> Anchor) -> Finding {
    let anchors: Vec<Anchor> = v["items"].as_array().into_iter().flatten().map(|i| {
        let mut a = anchor(i["description"].as_str().unwrap_or(""));
        a.uuid = i["uuid"].as_str().unwrap_or("").into();
        if let (Some(x), Some(y)) = (i["pos"]["x"].as_f64(), i["pos"]["y"].as_f64()) { a.x = x; a.y = y; a.at = true }
        a
    }).collect();
    let rule = v["type"].as_str().unwrap_or("").to_string();
    let detail = anchors.iter().map(|a| a.text.as_str()).collect::<Vec<_>>().join("; ");
    Finding { id: String::new(), source: source.into(), severity: severity_of(v), fix: fix_hint(source, &rule).into(), rule, title: v["description"].as_str().unwrap_or("").into(), detail, anchors }
}
/// A report's units to millimetres (KiCad's JSON says "mm" or "in"; a mils report says "mils").
fn scale(j: &Value) -> f64 { match j["coordinate_units"].as_str().unwrap_or("mm") { "in" => 25.4, "mils" => 0.0254, _ => 1.0 } }
fn scaled(mut fs: Vec<Finding>, k: f64) -> Vec<Finding> { if k != 1.0 { for f in &mut fs { for a in &mut f.anchors { a.x *= k; a.y *= k } } } fs }

/// `kicad-cli pcb drc --format json` read into findings (violations, unconnected items, schematic parity), with ids; None when it is
/// no DRC report (a failed run must never read as a clean board).
pub fn drc(text: &str) -> Option<Vec<Finding>> {
    let j: Value = serde_json::from_str(text).ok()?;
    if !j["violations"].is_array() || !j["unconnected_items"].is_array() { return None }
    let mut fs: Vec<Finding> = ["violations", "unconnected_items", "schematic_parity"].iter()
        .flat_map(|k| j[*k].as_array().cloned().unwrap_or_default()).map(|v| finding_of("drc", &v, pcb_anchor)).collect();
    assign_ids(&mut fs);
    Some(scaled(fs, scale(&j)))
}
/// `kicad-cli sch erc --format json` read into findings, each item with its sheet; None when it is no ERC report.
pub fn erc(text: &str) -> Option<Vec<Finding>> {
    let j: Value = serde_json::from_str(text).ok()?;
    let sheets = j["sheets"].as_array()?;
    let mut fs = vec![];
    for s in sheets {
        let path = s["path"].as_str().unwrap_or("/").to_string();
        for v in s["violations"].as_array().into_iter().flatten() { fs.push(finding_of("erc", v, |d| sch_anchor(d, &path))) }
    }
    assign_ids(&mut fs);
    Some(scaled(fs, scale(&j)))
}

// To clients -------------------------------------------------------------------------------------------------------------------

pub fn anchor_json(a: &Anchor) -> Value {
    let mut o = json!({"doc": a.doc, "text": a.text});
    for (k, v) in [("uuid", &a.uuid), ("reference", &a.reference), ("pin", &a.pin), ("net", &a.net), ("layer", &a.layer), ("sheet", &a.sheet)] { if !v.is_empty() { o[k] = json!(v) } }
    if a.at { o["x"] = json!(a.x); o["y"] = json!(a.y) }
    o
}
pub fn to_json(f: &Finding) -> Value {
    json!({"id": f.id, "source": f.source, "severity": f.severity, "rule": f.rule, "title": f.title, "detail": f.detail, "fix": f.fix, "anchors": f.anchors.iter().map(anchor_json).collect::<Vec<_>>()})
}
pub fn from_json(v: &Value) -> Finding {
    let s = |o: &Value, k: &str| o[k].as_str().unwrap_or("").to_string();
    Finding { id: s(v, "id"), source: s(v, "source"), severity: s(v, "severity"), rule: s(v, "rule"), title: s(v, "title"), detail: s(v, "detail"), fix: s(v, "fix"),
        anchors: v["anchors"].as_array().into_iter().flatten().map(|a| Anchor { doc: s(a, "doc"), text: s(a, "text"), uuid: s(a, "uuid"), reference: s(a, "reference"), pin: s(a, "pin"), net: s(a, "net"),
            layer: s(a, "layer"), sheet: s(a, "sheet"), x: a["x"].as_f64().unwrap_or(0.0), y: a["y"].as_f64().unwrap_or(0.0), at: a["x"].is_number() }).collect() }
}

/// Severity order: errors first, exclusions last.
pub fn rank(sev: &str) -> u8 { match sev { "error" => 0, "warning" => 1, "info" => 2, _ => 3 } }
/// The list a client asked for: `severity` ("" all, or "error" / "warning" / "info" / "excluded"), `q` words (each must appear in
/// the rule, title, detail or a name, any case), sorted by severity then rule (report order within a rule, so ids keep their place).
pub fn filter<'a>(fs: &'a [Finding], severity: &str, q: &str) -> Vec<&'a Finding> {
    let words: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
    let mut out: Vec<&Finding> = fs.iter().filter(|f| severity.is_empty() || f.severity == severity).filter(|f| {
        if words.is_empty() { return true }
        let hay = format!("{} {} {} {}", f.rule, f.title, f.detail, f.anchors.iter().map(|a| format!("{} {} {}", a.reference, a.pin, a.net)).collect::<Vec<_>>().join(" ")).to_lowercase();
        words.iter().all(|w| hay.contains(w))
    }).collect();
    out.sort_by(|a, b| rank(&a.severity).cmp(&rank(&b.severity)).then_with(|| a.rule.cmp(&b.rule)));
    out
}

/// What `editor.kicad.select` gets for a finding: its parts, pins and nets, in the editor it belongs to (DRC: the board, ERC: the
/// schematic). A pad selects the pad, not its whole part; a net only when the item has no part (a track, a via, a label), so a
/// clearance between a via and a pad zooms to the pad and the via's net, not to every item of the net. An item
/// with no name at all (a board edge) gives an empty selection: only the editor is shown.
pub fn select_params(f: &Finding) -> Value {
    let (mut refs, mut pins, mut nets): (Vec<String>, Vec<String>, Vec<String>) = (vec![], vec![], vec![]);
    let push = |v: &mut Vec<String>, s: &str| if !s.is_empty() && !v.iter().any(|x| x == s) { v.push(s.to_string()) };
    for a in &f.anchors {
        if !a.pin.is_empty() { push(&mut pins, &a.pin) } else if !a.reference.is_empty() { push(&mut refs, &a.reference) } else if !a.net.is_empty() { push(&mut nets, &a.net) }
    }
    let doc = f.anchors.first().map(|a| a.doc.as_str()).unwrap_or(if f.source == "erc" { "sch" } else { "pcb" });
    json!({"references": refs, "pins": pins, "nets": nets, "show": if doc == "sch" { "SCH" } else { "PCB" }, "zoom": true})
}

/// The message "Fix with agent" sends: everything the agent needs to find and fix this one finding, by name and place.
pub fn fix_message(f: &Finding, pro: &str) -> String {
    let what = match f.source.as_str() { "drc" => "DRC", "erc" => "ERC", s => s };
    let file = std::path::Path::new(pro).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut m = format!("Fix this {what} {} in {file} ({}): {}\n\nRule: {}\nFinding: {}\n", f.severity, pro, f.title, f.rule, f.id);
    if !f.anchors.is_empty() {
        m.push_str("Items:\n");
        for a in &f.anchors {
            let mut line = format!("- {}", a.text);
            let mut bits = vec![];
            if !a.sheet.is_empty() { bits.push(format!("sheet {}", a.sheet)) }
            if a.at { bits.push(format!("at ({:.3}, {:.3}) mm", a.x, a.y)) }
            if !a.uuid.is_empty() { bits.push(format!("KIID {}", a.uuid)) }
            if !bits.is_empty() { line.push_str(&format!(" ({})", bits.join(", "))) }
            m.push_str(&line); m.push('\n');
        }
    }
    if !f.fix.is_empty() { m.push_str(&format!("Usual fix: {}.\n", f.fix)) }
    m.push_str(&format!("\nChange only what this needs, save, and run {what} again to confirm it is gone."));
    m
}

// When to run ----------------------------------------------------------------------------------------------------------------

/// Quiet time after the last save before a run (saves come in bursts: Ctrl+S, an agent's several SaveDocuments).
pub const DEBOUNCE_MS: u64 = 1500;
/// No run while the person worked in KiCad this recently (a drag, a selection): the run takes cores and gigabytes.
pub const QUIET_MS: u64 = 1200;
/// How long a save waits for the agent turn that holds the document to end before it runs anyway.
pub const AGENT_MAX_MS: u64 = 120_000;

/// One check's schedule. Times are the hub's milliseconds; 0 is "never".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sched {
    /// the design revision the last result is of, and the one on disk now
    pub ran_rev: String,
    pub disk_rev: String,
    /// when `disk_rev` was first seen differing from the one before it (the last save)
    pub changed_at: u64,
    /// the person pressed Run (or a client asked for a first result)
    pub demand: bool,
    /// an agent turn holds the document's lock, since when
    pub agent_since: u64,
    /// the person's last action in the open KiCad
    pub person_at: u64,
    /// a run going: since when, and of which revision
    pub running_since: u64,
    pub running_rev: String,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Do { Nothing, Wait(u64), Run, Cancel }

/// The re-run policy: Run when the design on disk differs from the last result's (a save, an agent's turn end) and it has been quiet
/// for DEBOUNCE_MS, the person has not touched KiCad for QUIET_MS and no agent turn holds the document (or it has held it for
/// AGENT_MAX_MS); Run at once on demand. A run of a revision that is no longer the one on disk is cancelled (a newer save arrived);
/// never two runs of one check at once. Wait(ms) is when to look again.
pub fn next(now: u64, s: &Sched) -> Do {
    let pending = s.disk_rev != s.ran_rev;
    if s.running_since > 0 {
        return if pending && !s.running_rev.is_empty() && s.running_rev != s.disk_rev { Do::Cancel } else { Do::Nothing }
    }
    if s.demand { return Do::Run }
    if !pending { return Do::Nothing }
    let mut wait = 0u64;
    let until = |t: u64, d: u64| (t + d).saturating_sub(now);
    if s.changed_at > 0 { wait = wait.max(until(s.changed_at, DEBOUNCE_MS)) }
    if s.person_at > 0 { wait = wait.max(until(s.person_at, QUIET_MS)) }
    if s.agent_since > 0 && now < s.agent_since + AGENT_MAX_MS { wait = wait.max(until(s.agent_since, AGENT_MAX_MS).min(5000)) }
    if wait > 0 { Do::Wait(wait) } else { Do::Run }
}

/// A run's progress for the bar: kicad-cli says nothing while it runs, so the time so far against the last run's (capped below 1;
/// -1 when there was no last run).
pub fn fraction(elapsed_ms: u64, last_ms: u64) -> f64 { if last_ms == 0 { -1.0 } else { (elapsed_ms as f64 / last_ms as f64).min(0.95) } }

/// The chip's words: "DRC 12 · ERC 3" (errors and warnings, not exclusions), "DRC ✓" when clean, "DRC …" until a first result,
/// "DRC !" when the last run failed; a check the project has not got (no board) is left out. Each check: (state, counts, has a result).
pub fn chip(drc: Option<(&str, Counts, bool)>, erc: Option<(&str, Counts, bool)>) -> String {
    let one = |name: &str, x: Option<(&str, Counts, bool)>| x.map(|(st, c, had)| {
        if st == "failed" { format!("{name} !") } else if !had { format!("{name} …") } else if c.open() == 0 { format!("{name} ✓") } else { format!("{name} {}", c.open()) }
    });
    [one("DRC", drc), one("ERC", erc)].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    const DRC: &str = r#"{"coordinate_units":"mm","violations":[
      {"description":"Clearance violation (netclass 'Ground' clearance 0.1000 mm; actual 0.0250 mm)","items":[{"description":"Micro via [GND] on F.Cu - In1.Cu","pos":{"x":103.5,"y":131.5},"uuid":"25be"},{"description":"Pad 1 [/SHPHLD] of SW3 on F.Cu","pos":{"x":103.725,"y":132.175},"uuid":"b3a2"}],"severity":"error","type":"clearance"},
      {"description":"Courtyards overlap","items":[{"description":"Footprint C76","uuid":"c76"},{"description":"Footprint U3","uuid":"u3"}],"severity":"error","type":"courtyards_overlap"},
      {"description":"Track has unconnected end","items":[{"description":"Track [VDD_SENS] on F.Cu, length 1.4500 mm","uuid":"t1"}],"severity":"warning","type":"track_dangling"},
      {"description":"Silkscreen clearance","items":[],"severity":"exclusion","type":"silk_overlap"},
      {"description":"Silkscreen clearance","items":[],"severity":"warning","type":"silk_overlap","excluded":true}],
     "unconnected_items":[{"description":"Missing connection between items","items":[{"description":"PTH pad SH [GND] of J5","uuid":"j5sh"},{"description":"Zone [GND] on B.Cu, priority 0","uuid":"z"}],"severity":"error","type":"unconnected_items"}],
     "schematic_parity":[]}"#;
    const ERC: &str = r#"{"coordinate_units":"mm","sheets":[{"path":"/","violations":[]},{"path":"/Debugger/","violations":[
      {"description":"Input pin not driven by any Output pins","items":[{"description":"Symbol U5 Pin F2 [~{RESET}, Input, Line]","pos":{"x":129.54,"y":137.16},"uuid":"p1"}],"severity":"error","type":"pin_not_driven"},
      {"description":"Label not connected","items":[{"description":"Label 'SWD.CLK'","pos":{"x":63.5,"y":114.3},"uuid":"l1"}],"severity":"error","type":"label_dangling"},
      {"description":"Symbol 'LED_ARGB' doesn't match copy in library 'Device'","items":[{"description":"Symbol D1 [LED_ARGB]","uuid":"d1"}],"severity":"warning","type":"lib_symbol_mismatch"}]}]}"#;

    #[test] fn board_items_are_read_into_names() {
        let a = pcb_anchor("Pad 1 [/SHPHLD] of SW3 on F.Cu");
        assert_eq!((a.reference.as_str(), a.pin.as_str(), a.net.as_str(), a.layer.as_str()), ("SW3", "SW3.1", "/SHPHLD", "F.Cu"));
        let a = pcb_anchor("PTH pad SH [GND] of J5"); assert_eq!((a.pin.as_str(), a.net.as_str()), ("J5.SH", "GND"));
        let a = pcb_anchor("NPTH pad of J5"); assert_eq!((a.reference.as_str(), a.pin.as_str()), ("J5", ""));
        let a = pcb_anchor("Footprint C64"); assert_eq!(a.reference, "C64");
        let a = pcb_anchor("Micro via [GND] on F.Cu - In1.Cu"); assert_eq!((a.reference.as_str(), a.net.as_str(), a.layer.as_str()), ("", "GND", "F.Cu"));
        let a = pcb_anchor("Track [VDD_SENS] on F.Cu, length 1.4500 mm"); assert_eq!((a.net.as_str(), a.layer.as_str()), ("VDD_SENS", "F.Cu"));
        let a = pcb_anchor("Zone 'ref 3V3_DBG (U5)' [3V3_DBG] on In5.Cu, priority 5"); assert_eq!((a.net.as_str(), a.layer.as_str(), a.reference.as_str()), ("3V3_DBG", "In5.Cu", ""));
        let a = pcb_anchor("Segment of U6 on F.Silkscreen"); assert_eq!((a.reference.as_str(), a.layer.as_str()), ("U6", "F.Silkscreen"));
        let a = pcb_anchor("Pad 2 [<no net>] of R1 on F.Cu"); assert_eq!((a.net.as_str(), a.pin.as_str()), ("", "R1.2"));
    }
    #[test] fn schematic_items_are_read_into_names() {
        let a = sch_anchor("Symbol U5 Pin F2 [~{RESET}, Input, Line]", "/Debugger/");
        assert_eq!((a.reference.as_str(), a.pin.as_str(), a.net.as_str(), a.sheet.as_str()), ("U5", "U5.F2", "", "/Debugger/"));
        let a = sch_anchor("Symbol D1 [LED_ARGB]", "/"); assert_eq!((a.reference.as_str(), a.pin.as_str(), a.net.as_str()), ("D1", "", ""));
        assert_eq!(sch_anchor("Label 'SWD.CLK'", "/").net, "SWD.CLK");
        assert_eq!(sch_anchor("Global Label 'VBUS'", "/").net, "VBUS");
    }
    #[test] fn reports_are_read_with_severity_and_places() {
        let d = drc(DRC).unwrap();
        assert_eq!(d.len(), 6);
        let c = counts(&d); assert_eq!((c.error, c.warning, c.excluded), (3, 1, 2));
        assert_eq!(d[0].anchors[1].x, 103.725); assert!(d[0].anchors[1].at); assert!(!d[1].anchors[0].at);
        assert_eq!(d[0].fix, fix_hint("drc", "clearance")); assert!(!d[0].fix.is_empty());
        assert_eq!(d[5].rule, "unconnected_items");
        let e = erc(ERC).unwrap(); assert_eq!(e.len(), 3); assert_eq!(e[0].anchors[0].sheet, "/Debugger/"); assert_eq!(counts(&e).error, 2);
        assert!(drc("{}").is_none() && drc("nope").is_none() && erc("{}").is_none());
        assert_eq!(drc(r#"{"violations":[],"unconnected_items":[]}"#).unwrap().len(), 0);
        let inch = drc(r#"{"coordinate_units":"in","violations":[{"type":"x","severity":"error","description":"d","items":[{"description":"Footprint U1","pos":{"x":1,"y":2}}]}],"unconnected_items":[]}"#).unwrap();
        assert_eq!((inch[0].anchors[0].x, inch[0].anchors[0].y), (25.4, 50.8));
    }
    #[test] fn ids_are_stable_across_runs_and_unique_in_one() {
        let a = drc(DRC).unwrap();
        // the same report again, and one where the via moved (other numbers) and another violation came first
        let moved = DRC.replace("actual 0.0250 mm", "actual 0.0600 mm").replace("103.5", "103.4")
            .replace(r#""violations":["#, r#""violations":[{"description":"Hole clearance","items":[{"description":"Footprint R9","uuid":"r9"}],"severity":"error","type":"hole_clearance"},"#);
        let b = drc(&moved).unwrap();
        assert_eq!(a.iter().map(|f| f.id.clone()).collect::<Vec<_>>(), drc(DRC).unwrap().iter().map(|f| f.id.clone()).collect::<Vec<_>>());
        assert_eq!(b[1].id, a[0].id, "the clearance keeps its id when its numbers change and another finding comes first");
        let mut ids: Vec<&str> = a.iter().map(|f| f.id.as_str()).collect(); ids.sort(); ids.dedup(); assert_eq!(ids.len(), a.len());
        assert!(a[4].id.ends_with("~2"), "two silk findings with no items are told apart: {}", a[4].id);
        assert!(a[0].id.starts_with("drc:"));
        // the same items under another rule, or (ERC) on another sheet, are other findings
        let e1 = erc(ERC).unwrap(); let e2 = erc(&ERC.replace("/Debugger/", "/Debugger2/")).unwrap();
        assert_ne!(e1[0].id, e2[0].id);
        assert_eq!(from_json(&to_json(&a[0])), a[0]);
    }
    #[test] fn a_finding_selects_its_pads_parts_and_bare_nets() {
        let d = drc(DRC).unwrap();
        assert_eq!(select_params(&d[0]), json!({"references": [], "pins": ["SW3.1"], "nets": ["GND"], "show": "PCB", "zoom": true}));
        assert_eq!(select_params(&d[1])["references"], json!(["C76", "U3"]));
        assert_eq!(select_params(&d[3])["show"], "PCB");
        let e = erc(ERC).unwrap();
        assert_eq!(select_params(&e[0]), json!({"references": [], "pins": ["U5.F2"], "nets": [], "show": "SCH", "zoom": true}));
        assert_eq!(select_params(&e[1])["nets"], json!(["SWD.CLK"]));
    }
    #[test] fn the_fix_message_carries_everything() {
        let d = drc(DRC).unwrap();
        let m = fix_message(&d[0], "/p/Hubble_DK.kicad_pro");
        for want in ["Fix this DRC error in Hubble_DK", "/p/Hubble_DK.kicad_pro", "Clearance violation", "Rule: clearance", &d[0].id, "Pad 1 [/SHPHLD] of SW3 on F.Cu", "at (103.725, 132.175) mm", "KIID b3a2", "Usual fix:"] {
            assert!(m.contains(want), "{want} in {m}")
        }
        assert!(fix_message(&erc(ERC).unwrap()[0], "/p/a.kicad_pro").contains("sheet /Debugger/"));
    }
    #[test] fn the_list_filters_and_sorts() {
        let d = drc(DRC).unwrap();
        let all = filter(&d, "", ""); assert_eq!(all.len(), 6);
        assert_eq!(all.iter().map(|f| f.severity.as_str()).collect::<Vec<_>>(), ["error", "error", "error", "warning", "excluded", "excluded"]);
        assert_eq!(filter(&d, "error", "").len(), 3);
        assert_eq!(filter(&d, "", "sw3").len(), 1); assert_eq!(filter(&d, "", "GND clearance").len(), 1); assert_eq!(filter(&d, "", "J5.SH").len(), 1);
        assert_eq!(filter(&d, "warning", "nothing").len(), 0);
    }
    #[test] fn the_rerun_policy() {
        let fresh = Sched { ran_rev: "a".into(), disk_rev: "a".into(), ..Default::default() };
        assert_eq!(next(10_000, &fresh), Do::Nothing);
        assert_eq!(next(10_000, &Sched { demand: true, ..fresh.clone() }), Do::Run, "Run runs a fresh design too");
        // a save: wait out the burst, then run
        let saved = Sched { disk_rev: "b".into(), changed_at: 10_000, ..fresh.clone() };
        assert_eq!(next(10_500, &saved), Do::Wait(1000)); assert_eq!(next(11_500, &saved), Do::Run);
        // the person dragging in KiCad holds it back
        assert_eq!(next(11_500, &Sched { person_at: 11_000, ..saved.clone() }), Do::Wait(700));
        // an agent's turn holds the document: wait for its end (looked at again every 5 s), but not forever
        let agent = Sched { agent_since: 9_000, ..saved.clone() };
        assert_eq!(next(11_500, &agent), Do::Wait(5000));
        assert_eq!(next(9_000 + AGENT_MAX_MS, &agent), Do::Run);
        assert_eq!(next(11_500, &Sched { agent_since: 0, ..agent.clone() }), Do::Run, "the turn ended: run");
        // a run of "b" while "c" is saved is cancelled; a run of the newest is left alone; never two at once
        let running = Sched { running_since: 11_600, running_rev: "b".into(), ..saved.clone() };
        assert_eq!(next(12_000, &running), Do::Nothing);
        assert_eq!(next(12_000, &Sched { disk_rev: "c".into(), ..running.clone() }), Do::Cancel);
        assert_eq!(next(12_000, &Sched { demand: true, ..running.clone() }), Do::Nothing);
        // a first result (none ran yet) is pending too
        assert_eq!(next(20_000, &Sched { ran_rev: String::new(), disk_rev: "a".into(), ..Default::default() }), Do::Run);
        assert_eq!(fraction(500, 0), -1.0); assert_eq!(fraction(500, 1000), 0.5); assert_eq!(fraction(5000, 1000), 0.95);
    }
    #[test] fn the_chip() {
        let c = |e, w, x| Counts { error: e, warning: w, info: 0, excluded: x };
        assert_eq!(chip(Some(("fresh", c(2, 10, 3), true)), Some(("stale", c(0, 3, 0), true))), "DRC 12 · ERC 3");
        assert_eq!(chip(Some(("fresh", c(0, 0, 4), true)), Some(("failed", c(0, 0, 0), true))), "DRC ✓ · ERC !");
        assert_eq!(chip(Some(("running", c(0, 0, 0), false)), None), "DRC …");
        assert_eq!(chip(Some(("running", c(1, 0, 0), true)), None), "DRC 1");
        assert_eq!(chip(None, None), "");
    }
}

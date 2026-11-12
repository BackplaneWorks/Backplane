//! The fab's limits in the project's custom DRC rules (`<project>.kicad_dru`), merged, never overwritten: a marked section of whole-line
//! comments and rules, right after the file's `(version 1)`, regenerated in place; every byte outside the markers stays as the person
//! wrote it. Rules later in a rules file win in KiCad, so the person's own rules (after the section) keep precedence where they match.
//! Where the project's board setup minimum is already as strict, no rule is written (a comment says so): the project wins. Clearance
//! is written only between net classes whose own clearance is under the fab's, so no net class is ever loosened. The project's board
//! setup minimums under the fab's are suggestions for the person to approve, never written here; the person's rules that allow less
//! than the fab are named as conflicts, never changed. Everything written loads in stock KiCad: `#` comments, documented constraints
//! and expressions only. Pure but for `write`. Laws `fab_drc_*`.
use super::model::{label, Limit, Profile, Sel};
use super::project::Choice;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const BEGIN: &str = "# BEGIN Backplane fab profile";
pub const END: &str = "# END Backplane fab profile";

/// What the project already says: its board setup minimums and its net classes' clearances (from the .kicad_pro).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectRules { pub min: BTreeMap<String, f64>, pub classes: Vec<(String, f64)> }
pub fn project_rules(pro: &str) -> ProjectRules {
    let Ok(j) = serde_json::from_str::<Value>(pro) else { return ProjectRules::default() };
    let min = j.pointer("/board/design_settings/rules").and_then(Value::as_object).map(|m| m.iter().filter_map(|(k, v)| v.as_f64().map(|v| (k.clone(), v))).collect()).unwrap_or_default();
    let classes = j.pointer("/net_settings/classes").and_then(Value::as_array).map(|a| a.iter().filter_map(|c| Some((c["name"].as_str()?.to_string(), c["clearance"].as_f64()?))).collect()).unwrap_or_default();
    ProjectRules { min, classes }
}

/// One rule the fab's profile can feed: its name, KiCad constraint, condition, the profile key and the board setup minimum it compares with.
struct Map { name: &'static str, constraint: &'static str, cond: &'static str, key: &'static str, pro: &'static str }
const MAPS: &[Map] = &[
    Map { name: "track width", constraint: "track_width", cond: "", key: "track_min", pro: "min_track_width" },
    Map { name: "via diameter", constraint: "via_diameter", cond: "A.Type == 'Via'", key: "via_diameter_min", pro: "min_via_diameter" },
    Map { name: "via hole", constraint: "hole_size", cond: "A.Type == 'Via'", key: "via_hole_min", pro: "min_through_hole_diameter" },
    Map { name: "microvia hole", constraint: "hole_size", cond: "A.Type == 'Via' && A.Via_Type == 'Micro'", key: "microvia_hole_min", pro: "min_microvia_drill" },
    Map { name: "pad hole", constraint: "hole_size", cond: "A.Type == 'Pad' && A.Pad_Type == 'Through-hole'", key: "drill_min", pro: "min_through_hole_diameter" },
    Map { name: "non-plated hole", constraint: "hole_size", cond: "A.Type == 'Pad' && A.Pad_Type == 'NPTH, mechanical'", key: "npth_min", pro: "" },
    Map { name: "via ring", constraint: "annular_width", cond: "A.Type == 'Via'", key: "via_ring_min", pro: "min_via_annular_width" },
    Map { name: "plated hole ring", constraint: "annular_width", cond: "A.Type == 'Pad' && A.Pad_Type == 'Through-hole'", key: "pth_ring_min", pro: "" },
    Map { name: "hole to hole", constraint: "hole_to_hole", cond: "", key: "via_hole_to_hole_min", pro: "min_hole_to_hole" },
    Map { name: "pad hole to pad hole", constraint: "hole_to_hole", cond: "A.Type == 'Pad' && B.Type == 'Pad'", key: "pad_hole_to_hole_min", pro: "" },
    Map { name: "hole to copper", constraint: "hole_clearance", cond: "", key: "via_to_track_min", pro: "min_hole_clearance" },
    Map { name: "plated hole to copper", constraint: "hole_clearance", cond: "A.Type == 'Pad' && A.Pad_Type == 'Through-hole'", key: "pth_to_track_min", pro: "" },
    Map { name: "copper to edge", constraint: "edge_clearance", cond: "", key: "copper_to_edge_min", pro: "min_copper_edge_clearance" },
    Map { name: "silkscreen text height", constraint: "text_height", cond: "A.Layer == 'F.SilkS' || A.Layer == 'B.SilkS'", key: "silk_text_height_min", pro: "min_text_height" },
    Map { name: "silkscreen text line", constraint: "text_thickness", cond: "A.Layer == 'F.SilkS' || A.Layer == 'B.SilkS'", key: "silk_line_min", pro: "min_text_thickness" },
];
/// The profile keys a KiCad constraint type is checked against (for the person's rules that allow less).
fn keys_of(constraint: &str) -> &'static [&'static str] {
    match constraint {
        "track_width" => &["track_min"], "clearance" => &["space_min"], "via_diameter" => &["via_diameter_min"], "hole_size" => &["via_hole_min", "drill_min"],
        "annular_width" => &["via_ring_min"], "hole_to_hole" => &["via_hole_to_hole_min"], "hole_clearance" => &["via_to_track_min"],
        "edge_clearance" => &["copper_to_edge_min"], "text_height" => &["silk_text_height_min"], "text_thickness" => &["silk_line_min"], _ => &[],
    }
}

/// The value DRC holds a board to: the free one, or with tier "extra" the one that costs more where there is one.
fn at_tier<'a>(sel: &'a [Sel], key: &str, tier: &str) -> Option<&'a Limit> {
    let s = sel.iter().find(|s| s.key == key)?;
    (if tier == "extra" { s.extra.as_ref().or(s.free.as_ref()) } else { s.free.as_ref() }).filter(|l| l.num().is_some())
}
fn mm(v: f64) -> String { let s = format!("{:.4}", v); let s = s.trim_end_matches('0').trim_end_matches('.'); format!("{s}mm") }
fn plain(v: f64) -> String { mm(v).trim_end_matches("mm").to_string() }
fn src(l: &Limit) -> String { format!("{}, read {}{}", l.source, l.fetched, if l.confidence == "published" { String::new() } else { format!(", {}", l.confidence) }) }
fn rule(name: &str, cond: &str, constraint: &str, v: f64) -> String {
    let c = if cond.is_empty() { String::new() } else { format!("\n\t(condition \"{cond}\")") };
    format!("(rule \"fab: {name}\"{c}\n\t(constraint {constraint} (min {})))\n", mm(v))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plan { pub section: String, pub suggestions: Vec<Value>, pub conflicts: Vec<String>,
    /// the person's rules that allow less than the fab's base price but no less than it makes for more
    pub costs_more: Vec<String> }

/// Fab.drc.plan: the section for this profile and choice, given the project's own settings and the text of its rules file.
pub fn plan(p: &Profile, c: &Choice, pr: &ProjectRules, existing: &str) -> Plan {
    let o = c.opts(); let sel = super::model::select(p, &c.kind, &o); let tier = if c.tier == "extra" { "extra" } else { "free" };
    let mut s = format!("{BEGIN} (regenerated by Backplane from the fab's published limits; edits between these lines are replaced)\n");
    s += &format!("# profile rev {} of {}, for a {} board: {} layers, {} oz, {} mm, {}; {} limits\n", p.rev, p.updated, c.kind, plain(o.layers), plain(o.copper_oz), plain(o.thickness_mm), o.finish,
        if tier == "extra" { "costs-more" } else { "base-price" });
    let mut suggestions = vec![];
    let mut push_sug = |pro: &str, have: f64, l: &Limit| if !suggestions.iter().any(|s: &Value| s["key"] == pro) { suggestions.push(json!({"key": pro, "label": label(&l.key), "project": have, "fab": l.num(), "unit": l.unit, "source": l.source, "fetched": l.fetched})) };
    // clearance: only between net classes under the fab's
    if let Some(l) = at_tier(&sel, "space_min", tier) {
        let v = l.num().unwrap();
        let low: Vec<&str> = pr.classes.iter().filter(|(n, cl)| *cl < v - 1e-9 && !n.contains('\'') && !n.contains('"')).map(|(n, _)| n.as_str()).collect();
        if let Some(h) = pr.min.get("min_clearance") { if *h < v - 1e-9 { push_sug("min_clearance", *h, l) } }
        if low.is_empty() { s += &format!("# clearance: every net class is at or above the fab's {} ({})\n", mm(v), src(l)) }
        else {
            let side = |x: &str| low.iter().map(|n| format!("{x}.NetClass == '{n}'")).collect::<Vec<_>>().join(" || ");
            let cond = if low.len() == 1 { format!("{} && {}", side("A"), side("B")) } else { format!("({}) && ({})", side("A"), side("B")) };
            s += &format!("# clearance: the fab's {} ({}), for net classes under it: {}\n", mm(v), src(l), low.join(", "));
            s += &rule("clearance", &cond, "clearance", v);
        }
    }
    for m in MAPS {
        let Some(l) = at_tier(&sel, m.key, tier) else { continue };
        let v = l.num().unwrap();
        let have = if m.pro.is_empty() { None } else { pr.min.get(m.pro).copied() };
        // a microvia rule only for a board that has microvias
        if m.key == "microvia_hole_min" && c.kind != "hdi" { continue }
        match have {
            Some(h) if h >= v - 1e-9 => s += &format!("# {}: the project's {} is as strict as the fab's {} or stricter\n", m.name, mm(h), mm(v)),
            _ => {
                if let Some(h) = have { push_sug(m.pro, h, l) }
                s += &format!("# {}: the fab's {} ({})\n", m.name, mm(v), src(l));
                s += &rule(m.name, m.cond, m.constraint, v);
            }
        }
    }
    s += END; s += "\n";
    // the person's rules that allow less than the fab (outside the section)
    let (mut conflicts, mut costs_more) = (vec![], vec![]);
    for (name, constraint, v, cond) in own_rules(&outside(existing)) {
        for k in keys_for(&constraint, &cond) {
            if let Some(l) = at_tier(&sel, k, tier) { let f = l.num().unwrap(); if v < f - 1e-9 {
                let floor = at_tier(&sel, k, "extra").and_then(Limit::num).unwrap_or(f);
                let line = format!("rule \"{name}\" allows {constraint} {}, under the fab's {} ({})", mm(v), mm(f), label(k));
                if v >= floor - 1e-9 { costs_more.push(format!("{line}; the fab makes down to {} for more", mm(floor))) } else { conflicts.push(line) }
                break
            } }
        }
    }
    Plan { section: s, suggestions, conflicts, costs_more }
}

/// (start, end) byte offsets of the section, the END line's newline included.
pub fn find(text: &str) -> Option<(usize, usize)> {
    let mut at = 0; let mut b = None;
    for line in text.split_inclusive('\n') {
        if b.is_none() && line.starts_with(BEGIN) { b = Some(at) }
        else if b.is_some() && line.starts_with(END) { return Some((b.unwrap(), at + line.len())) }
        at += line.len();
    }
    None
}
/// The file without the section.
pub fn outside(text: &str) -> String { match find(text) { Some((a, b)) => format!("{}{}", &text[..a], &text[b..]), None => text.to_string() } }
pub fn section(text: &str) -> Option<&str> { find(text).map(|(a, b)| &text[a..b]) }

/// Fab.drc.merge: the section in place of the old one, else right after the `(version ...)` line, else first; an empty file gets `(version 1)`.
pub fn merge(text: &str, section: &str) -> String {
    if let Some((a, b)) = find(text) { return format!("{}{section}{}", &text[..a], &text[b..]) }
    if text.trim().is_empty() { return format!("(version 1)\n{section}{text}") }
    let at = version_end(text).unwrap_or(0);
    format!("{}{section}{}", &text[..at], &text[at..])
}
/// The byte after the newline ending the `(version N)` form, when it is the file's first form and its line ends.
fn version_end(text: &str) -> Option<usize> {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { at += line.len(); continue }
        return if t.starts_with("(version") && line.ends_with('\n') { Some(at + line.len()) } else { None };
    }
    None
}

/// The profile keys a constraint of a rule is held to, by what its condition is about: a microvia's or blind via's hole is held to the
/// microvia minimum and a buried via's to the buried minimum (their diameters and clearances have no published limit of their own).
fn keys_for(constraint: &str, cond: &str) -> &'static [&'static str] {
    let micro = cond.contains("'Micro'") || cond.contains("'Blind'") || cond.contains("'blind'") || cond.contains("'micro'");
    let buried = cond.contains("'Buried'") || cond.contains("'buried'");
    match (micro, buried, constraint) {
        (true, _, "hole_size") => &["microvia_hole_min"], (_, true, "hole_size") => &["buried_via_hole_min"],
        (true, _, _) | (_, true, _) => &[],
        _ => keys_of(constraint),
    }
}
/// The person's rules' minimums with each rule's condition: (rule name, constraint, value in mm, condition).
pub fn own_rules(text: &str) -> Vec<(String, String, f64, String)> {
    let clean: String = text.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join("\n");
    let mut out = vec![];
    for chunk in clean.split("(rule ").skip(1) {
        let cond = chunk.find("(condition ").map(|i| chunk[i + 11..].split('"').nth(1).unwrap_or("").to_string()).unwrap_or_default();
        for (n, c, v) in own_mins(&format!("(rule {chunk}")) { out.push((n, c, v, cond.clone())) }
    }
    out
}
/// The person's rules' minimums: (rule name, constraint, value in mm).
pub fn own_mins(text: &str) -> Vec<(String, String, f64)> {
    let clean: String = text.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join("\n");
    let mut out = vec![];
    for chunk in clean.split("(rule ").skip(1) {
        let name = chunk.trim_start().strip_prefix('"').and_then(|r| r.split('"').next()).unwrap_or_else(|| chunk.split_whitespace().next().unwrap_or("")).to_string();
        for c in chunk.split("(constraint ").skip(1) {
            let ty = c.split(|ch: char| ch.is_whitespace() || ch == ')' || ch == '(').next().unwrap_or("").to_string();
            let Some(m) = c.split("(min ").nth(1) else { continue };
            let tok = m.split(')').next().unwrap_or("").trim();
            if let Some(v) = length_mm(tok) { out.push((name.clone(), ty, v)) }
        }
    }
    out
}
fn length_mm(t: &str) -> Option<f64> {
    let (num, unit) = t.find(|c: char| c.is_ascii_alphabetic()).map_or((t, ""), |i| (&t[..i], &t[i..]));
    let v: f64 = num.trim().parse().ok()?;
    Some(match unit { "" | "mm" => v, "mil" | "mils" => v * 0.0254, "in" => v * 25.4, "um" => v / 1000.0, _ => return None })
}

/// The rules file of a KiCad project: beside its .kicad_pro, same name.
pub fn dru_path(pro: &str) -> Option<PathBuf> { pro.strip_suffix(".kicad_pro").map(|s| PathBuf::from(format!("{s}.kicad_dru"))) }

/// What the project's rules file holds against the plan.
pub fn status(text: &str, plan: &Plan) -> Value {
    let cur = section(text);
    json!({"present": cur.is_some(), "current": cur == Some(plan.section.as_str()), "suggestions": plan.suggestions, "conflicts": plan.conflicts, "costs_more": plan.costs_more})
}

/// Write the section into the project's rules file (a copy hub writes nothing). Answers the new text.
pub fn write(dru: &Path, plan: &Plan) -> Result<String, String> {
    if crate::copy::on() { return Err(crate::copy::HOME_ONLY.into()) }
    let old = match std::fs::read(dru) { Ok(b) => String::from_utf8(b).map_err(|_| format!("{} is not UTF-8 text; left as it is", dru.display()))?, Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(), Err(e) => return Err(e.to_string()) };
    let new = merge(&old, &plan.section);
    if new != old { let tmp = dru.with_extension("kicad_dru.tmp"); std::fs::write(&tmp, &new).map_err(|e| e.to_string())?; std::fs::rename(&tmp, dru).map_err(|e| e.to_string())? }
    Ok(new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fab::{project::Choice, store};
    fn choice(kind: &str, opts: Value, tier: &str) -> Choice { Choice { fab: "jlcpcb".into(), kind: kind.into(), options: opts, by: "picked".into(), rev_seen: 0, from: String::new(), tier: tier.into() } }
    const HAND: &str = "(version 1)\n# my own rules\n(rule \"BGA fanout\"\n\t(condition \"A.insideCourtyard('U1')\")\n\t(constraint clearance (min 0.08mm))\n\t(constraint track_width (min 3mil)))\n\n(rule HV (condition \"A.NetClass == 'HV'\") (constraint clearance (min 1mm)))\n";
    fn pro() -> ProjectRules { project_rules(r#"{"board":{"design_settings":{"rules":{"min_clearance":0.0,"min_track_width":0.2,"min_via_diameter":0.5,"min_through_hole_diameter":0.3,"min_copper_edge_clearance":0.5,"min_hole_clearance":0.25}}},"net_settings":{"classes":[{"name":"Default","clearance":0.2},{"name":"Fine","clearance":0.075},{"name":"HV","clearance":1.0}]}}"#) }

    #[test] fn fab_drc_a_hand_written_file_round_trips_byte_identical_outside_the_markers() {
        let p = store::baseline("jlcpcb").unwrap(); let pl = plan(&p, &choice("pcb", json!({"layers": 4}), "free"), &pro(), HAND);
        let once = merge(HAND, &pl.section);
        assert_eq!(outside(&once), HAND, "every byte outside the section is the person's");
        assert!(once.starts_with("(version 1)\n# BEGIN Backplane fab profile"), "right after the version");
        assert_eq!(merge(&once, &pl.section), once, "regenerating is idempotent");
        let pl2 = plan(&p, &choice("pcb", json!({"layers": 2}), "free"), &pro(), &once); let twice = merge(&once, &pl2.section);
        assert_eq!(outside(&twice), HAND); assert_eq!(section(&twice), Some(pl2.section.as_str()));
        assert_eq!(status(&twice, &pl2)["current"], true); assert_eq!(status(&twice, &pl)["current"], false);
        // the person edits their own rules around the section: kept
        let edited = twice.replace("(min 1mm)", "(min 1.5mm)") + "(rule late (constraint hole_size (min 0.4mm)))\n";
        let again = merge(&edited, &pl.section); assert_eq!(outside(&again), outside(&edited));
        // no version line: first; an empty file gets a version
        assert_eq!(outside(&merge("(rule a (constraint track_width (min 0.2mm)))\n", &pl.section)), "(rule a (constraint track_width (min 0.2mm)))\n");
        assert!(merge("", &pl.section).starts_with("(version 1)\n# BEGIN"));
    }
    #[test] fn fab_drc_the_project_wins_where_it_is_stricter_and_no_net_class_is_loosened() {
        let p = store::baseline("jlcpcb").unwrap(); let pl = plan(&p, &choice("pcb", json!({"layers": 4}), "free"), &pro(), HAND); let s = &pl.section;
        assert!(s.contains("# track width: the project's 0.2mm is as strict"), "{s}"); assert!(!s.contains("(rule \"fab: track width\""));
        assert!(s.contains("# copper to edge: the project's 0.5mm is as strict"));
        assert!(s.contains("(rule \"fab: clearance\"\n\t(condition \"A.NetClass == 'Fine' && B.NetClass == 'Fine'\")\n\t(constraint clearance (min 0.09mm)))"), "only the class under the fab: {s}");
        assert!(s.contains("(rule \"fab: non-plated hole\"\n\t(condition \"A.Type == 'Pad' && A.Pad_Type == 'NPTH, mechanical'\")\n\t(constraint hole_size (min 0.5mm)))"));
        assert!(s.contains("https://jlcpcb.com/capabilities/pcb-capabilities, read 2026-11-12"), "each value with its source and date");
        assert_eq!(pl.suggestions.iter().map(|v| v["key"].as_str().unwrap()).collect::<Vec<_>>(), ["min_clearance"], "board setup minimums under the fab's are suggestions");
        assert_eq!(pl.costs_more, ["rule \"BGA fanout\" allows clearance 0.08mm, under the fab's 0.09mm (copper to copper spacing); the fab makes down to 0.075mm for more", "rule \"BGA fanout\" allows track_width 0.0762mm, under the fab's 0.09mm (track width); the fab makes down to 0.075mm for more"]);
        assert!(pl.conflicts.is_empty(), "within what the fab makes for more: notes, not conflicts");
        let tight = "(version 1)\n(rule t (constraint clearance (min 0.05mm)))\n(rule m (condition \"A.Via_Type == 'Micro'\") (constraint hole_size (min 0.075mm)) (constraint via_diameter (min 0.2mm)))\n";
        let hdi = plan(&p, &choice("hdi", json!({"layers": 6}), "free"), &pro(), tight);
        assert_eq!(hdi.conflicts.len(), 1, "a microvia rule is held to the microvia minimum: {:?}", hdi.conflicts); assert!(hdi.conflicts[0].contains("rule \"t\""));
        // costs-more tier: the BGA fanout is within what the fab makes for more
        let ex = plan(&p, &choice("pcb", json!({"layers": 4}), "extra"), &pro(), HAND); assert!(ex.section.contains("costs-more limits")); assert!(ex.conflicts.is_empty(), "0.08 mm and 3 mil are both within what it makes for more: {:?}", ex.conflicts);
        // every net class at or above the fab: no clearance rule
        let wide = ProjectRules { classes: vec![("Default".into(), 0.2)], ..pro() }; assert!(!plan(&p, &choice("pcb", json!({}), "free"), &wide, "").section.contains("(rule \"fab: clearance\""));
    }
    /// Writes the section into a KiCad project folder named by FAB_DRC_PROJECT (a copy), for test/fab/stock_drc.sh to run stock KiCad's DRC on it.
    #[test] #[ignore] fn fab_drc_write_into_a_project_copy() {
        let Ok(dir) = std::env::var("FAB_DRC_PROJECT") else { return };
        let (kind, opts, tier) = (std::env::var("FAB_DRC_KIND").unwrap_or("pcb".into()), std::env::var("FAB_DRC_OPTIONS").unwrap_or("{}".into()), std::env::var("FAB_DRC_TIER").unwrap_or("free".into()));
        let pro = crate::files::kicad::project::read(&dir).pro; let dru = dru_path(&pro).unwrap();
        let p = store::baseline("jlcpcb").unwrap(); let text = std::fs::read_to_string(&dru).unwrap_or_default();
        let pl = plan(&p, &choice(&kind, serde_json::from_str(&opts).unwrap(), &tier), &project_rules(&std::fs::read_to_string(&pro).unwrap()), &text);
        write(&dru, &pl).unwrap(); eprintln!("wrote {}: {} suggestions, {} conflicts", dru.display(), pl.suggestions.len(), pl.conflicts.len());
    }
    #[test] fn fab_drc_own_minimums_read_units_and_skip_comments() {
        assert_eq!(own_mins("# (rule x (constraint clearance (min 9mm)))\n(rule \"a b\" (constraint hole_size (min 10mil)) (constraint via_diameter (opt 1mm)))"), [("a b".to_string(), "hole_size".to_string(), 0.254)]);
        assert_eq!(dru_path("/p/x.kicad_pro"), Some(PathBuf::from("/p/x.kicad_dru")));
    }
    #[test] fn fab_drc_write_keeps_the_file_and_adds_the_section() {
        let t = crate::plat::tmp::dir("fab-drc"); let d: &Path = &t; let f = d.join("x.kicad_dru"); std::fs::write(&f, HAND).unwrap();
        let p = store::baseline("jlcpcb").unwrap(); let pl = plan(&p, &choice("pcb", json!({}), "free"), &pro(), HAND);
        let new = write(&f, &pl).unwrap(); assert_eq!(std::fs::read_to_string(&f).unwrap(), new); assert_eq!(outside(&new), HAND);
        let g = d.join("new.kicad_dru"); write(&g, &pl).unwrap(); assert!(std::fs::read_to_string(&g).unwrap().starts_with("(version 1)\n"));
    }
}

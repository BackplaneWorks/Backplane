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

/// One rule of the section, as data (what `EditCustomRules` sends to an open KiCad): its name ("fab: ..."), condition, KiCad
/// constraint, minimum in mm and the comment that says where the value is from.
#[derive(Clone, Debug, PartialEq)]
pub struct FabRule { pub name: String, pub cond: String, pub constraint: String, pub min: f64, pub comment: String }
pub const PREFIX: &str = "fab: ";

#[derive(Clone, Debug, PartialEq)]
pub struct Plan { pub section: String, pub rules: Vec<FabRule>, pub suggestions: Vec<Value>, pub conflicts: Vec<String>,
    /// the person's rules that allow less than the fab's base price but no less than it makes for more
    pub costs_more: Vec<String> }

/// Fab.drc.plan: the section for this profile and choice, given the project's own settings and the text of its rules file.
pub fn plan(p: &Profile, c: &Choice, pr: &ProjectRules, existing: &str) -> Plan {
    let o = c.opts(); let sel = super::model::select(p, &c.kind, &o); let tier = if c.tier == "extra" { "extra" } else { "free" };
    let mut s = format!("{BEGIN} (regenerated by Backplane from the fab's published limits; edits between these lines are replaced)\n");
    s += &format!("# profile rev {} of {}, for a {} board: {} layers, {} oz, {} mm, {}; {} limits\n", p.rev, p.updated, c.kind, plain(o.layers), plain(o.copper_oz), plain(o.thickness_mm), o.finish,
        if tier == "extra" { "costs-more" } else { "base-price" });
    let mut suggestions = vec![]; let mut rules: Vec<FabRule> = vec![];
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
            let comment = format!("clearance: the fab's {} ({}), for net classes under it: {}", mm(v), src(l), low.join(", "));
            s += &format!("# {comment}\n");
            s += &rule("clearance", &cond, "clearance", v);
            rules.push(FabRule { name: format!("{PREFIX}clearance"), cond, constraint: "clearance".into(), min: v, comment });
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
                let comment = format!("{}: the fab's {} ({})", m.name, mm(v), src(l));
                s += &format!("# {comment}\n");
                s += &rule(m.name, m.cond, m.constraint, v);
                rules.push(FabRule { name: format!("{PREFIX}{}", m.name), cond: m.cond.into(), constraint: m.constraint.into(), min: v, comment });
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
    Plan { section: s, rules, suggestions, conflicts, costs_more }
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
        for (n, c, v) in own_mins(&format!("(rule {chunk}")) { if !n.starts_with(PREFIX) { out.push((n, c, v, cond.clone())) } }
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

/// The fab's rules in a rules file: every rule named "fab: ...", in the marked section or (written live through KiCad) not.
fn fab_rules_in(text: &str) -> Vec<(String, String, f64)> {
    own_mins(text).into_iter().filter(|(n, _, _)| n.starts_with(PREFIX)).collect()
}
/// Whether the file has the fab's rules at all.
pub fn present(text: &str) -> bool { section(text).is_some() || !fab_rules_in(text).is_empty() }
/// Whether the file holds exactly the plan's fab rules: its section as Backplane writes it, or (written live by KiCad, which formats
/// the rules its own way) the same rules by name, constraint and minimum.
pub fn current(text: &str, plan: &Plan) -> bool {
    if section(text) == Some(plan.section.as_str()) { return true }
    let mut have = fab_rules_in(text); let mut want: Vec<(String, String, f64)> = plan.rules.iter().map(|r| (r.name.clone(), r.constraint.clone(), r.min)).collect();
    let key = |x: &(String, String, f64)| format!("{}|{}", x.0, x.1);
    have.sort_by_key(key); want.sort_by_key(key);
    !want.is_empty() && have.len() == want.len() && have.iter().zip(&want).all(|(a, b)| a.0 == b.0 && a.1 == b.1 && (a.2 - b.2).abs() < 1e-6)
}
/// KiCad's constraint type for a rules-file constraint name (EditCustomRules' CustomRuleConstraintType).
fn crct(constraint: &str) -> Option<&'static str> {
    Some(match constraint {
        "clearance" => "CRCT_CLEARANCE", "track_width" => "CRCT_TRACK_WIDTH", "via_diameter" => "CRCT_VIA_DIAMETER", "hole_size" => "CRCT_HOLE_SIZE",
        "annular_width" => "CRCT_ANNULAR_WIDTH", "hole_to_hole" => "CRCT_HOLE_TO_HOLE", "hole_clearance" => "CRCT_HOLE_CLEARANCE",
        "edge_clearance" => "CRCT_EDGE_CLEARANCE", "text_height" => "CRCT_TEXT_HEIGHT", "text_thickness" => "CRCT_TEXT_THICKNESS", _ => return None,
    })
}
/// The EditCustomRules message for an open KiCad (bp.11 `rules-merge`): the plan's rules upserted by name, the fab's rules it no
/// longer has removed; every other rule stays as it is. KiCad replaces a rule of the same name where it stands, but appends a new
/// one after everything else, where it would win over the person's own rules (later rules win). So where the file has no fab rules
/// yet, a rule whose constraint one of the person's rules also sets is left out (named in the answer's second part): the person's
/// rule keeps deciding. Answers (message fields but the board, the rules left out).
pub fn edit_message(text: &str, plan: &Plan) -> (Value, Vec<String>) {
    let had: Vec<String> = fab_rules_in(text).into_iter().map(|r| r.0).collect();
    let appended = had.is_empty();
    let own: Vec<String> = own_rules(&outside(text)).into_iter().map(|r| r.1).collect();
    let (mut upsert, mut left) = (vec![], vec![]);
    for r in &plan.rules {
        let Some(ty) = crct(&r.constraint) else { continue };
        if appended && own.contains(&r.constraint) { left.push(format!("{} (your own {} rule keeps deciding)", r.name, r.constraint)); continue }
        let mut j = json!({"name": r.name, "constraints": [{"type": ty, "numeric": {"min": (r.min * 1e6).round() as i64}}], "comments": r.comment});
        if !r.cond.is_empty() { j["condition"] = json!(r.cond) }
        upsert.push(j);
    }
    let names: Vec<&str> = plan.rules.iter().map(|r| r.name.as_str()).collect();
    let mut remove: Vec<String> = had.into_iter().filter(|n| !names.contains(&n.as_str())).collect(); remove.dedup();
    (json!({"upsert": upsert, "remove": remove}), left)
}

/// What the project's rules file holds against the plan.
pub fn status(text: &str, plan: &Plan) -> Value {
    json!({"present": present(text), "current": current(text, plan), "suggestions": plan.suggestions, "conflicts": plan.conflicts, "costs_more": plan.costs_more})
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
    #[test] fn fab_drc_an_open_kicad_gets_the_rules_by_name_and_the_persons_rules_keep_deciding() {
        let p = store::baseline("jlcpcb").unwrap(); let pl = plan(&p, &choice("pcb", json!({"layers": 4}), "free"), &pro(), HAND);
        assert!(!pl.rules.is_empty() && pl.rules.iter().all(|r| r.name.starts_with(PREFIX)));
        // no fab rules yet: KiCad appends them after the person's, so one whose constraint the person's rules set is left out
        let (m, left) = edit_message(HAND, &pl);
        let names: Vec<&str> = m["upsert"].as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap()).collect();
        assert!(!names.contains(&"fab: clearance"), "the person's clearance rules keep deciding: {names:?}");
        assert!(left.iter().any(|l| l.starts_with("fab: clearance")), "{left:?}");
        assert!(names.contains(&"fab: via ring"), "{names:?}");
        let vh = m["upsert"].as_array().unwrap().iter().find(|r| r["name"] == "fab: via ring").unwrap();
        assert_eq!(vh["constraints"][0]["type"], "CRCT_ANNULAR_WIDTH"); assert_eq!(vh["condition"], "A.Type == 'Via'");
        assert_eq!(vh["constraints"][0]["numeric"]["min"], json!((pl.rules.iter().find(|r| r.name == "fab: via ring").unwrap().min * 1e6).round() as i64), "nanometres");
        assert!(vh["comments"].as_str().unwrap().contains("https://"), "the source goes with the rule");
        assert_eq!(m["remove"], json!([]));
        // the section already there: every rule replaced where it stands, and a fab rule the plan no longer has removed
        let with = merge(HAND, &pl.section).replace("(version 1)\n", "(version 1)\n(rule \"fab: old\" (constraint track_width (min 0.3mm)))\n");
        let (m2, left2) = edit_message(&with, &pl);
        assert!(left2.is_empty(), "{left2:?}"); assert_eq!(m2["upsert"].as_array().unwrap().len(), pl.rules.len()); assert_eq!(m2["remove"], json!(["fab: old"]));
        // KiCad's own formatting of the rules it wrote counts as current; the person's rules are never taken for the fab's
        let live: String = pl.rules.iter().map(|r| format!("(rule \"{}\"\n\t# {}\n\t(constraint {} (min {}mm)))\n", r.name, r.comment, r.constraint, r.min)).collect();
        let file = format!("{HAND}{live}");
        assert!(present(&file) && current(&file, &pl), "{file}");
        assert!(!current(HAND, &pl) && !present(HAND));
        assert!(own_rules(&file).iter().all(|r| !r.0.starts_with(PREFIX)), "the fab's rules are not the person's");
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

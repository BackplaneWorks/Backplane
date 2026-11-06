//! core/checks in Rust: kicad-cli's ERC and DRC JSON read into findings, the checklist (".backplane/checklist.md") ticked in place,
//! the custom checks and gate of ".backplane/checks.json", the report and the gate. Pure: the jobs (files/rv.rs) run the commands and
//! hand the texts in. Laws `checks_*`, `checklist_*`. A finding's position is left out: nothing here reads it.
use super::bj::{self, J};

#[derive(Clone, Debug, PartialEq)]
pub struct Finding { pub source: String, pub severity: String, pub kind: String, pub text: String, pub place: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Task { pub text: String, pub done: bool, pub group: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Custom { pub name: String, pub run: String, pub on: Vec<String> }
#[derive(Clone, Debug, PartialEq)]
pub struct Run { pub name: String, pub ok: bool, pub out: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Gate { pub erc: String, pub drc: String, pub checklist: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Config { pub checks: Vec<Custom>, pub gate: Gate }
pub const CAP: usize = 4;
pub const SOURCES: [&str; 5] = ["erc", "drc", "bom", "checklist", "custom"];

/// Checks.finding: a finding with no position
pub fn finding(source: &str, severity: &str, kind: &str, text: &str, place: &str) -> Finding { Finding { source: source.into(), severity: severity.into(), kind: kind.into(), text: text.into(), place: place.into() } }

/// Fd.layer.w: the word after the last "on" ("Pad 1 [GND] of C21 on B.Cu" -> "B.Cu"), up to a comma
fn layer_of(desc: &str) -> String {
    let (mut prev, mut cur) = (false, String::new());
    for w in desc.split(' ') { if prev { cur = w.split(',').next().unwrap_or("").to_string() } prev = w == "on" }
    cur
}
/// Fd.make: one violation; an empty place is read from the items (a DRC layer)
fn make(source: &str, place: &str, v: &J) -> Finding {
    let its = v.get_obj("items").elems();
    let desc = v.get_str("description", "");
    let ds: Vec<&str> = its.iter().map(|i| i.get_str("description", "")).collect();
    let text = if ds.is_empty() { desc.to_string() } else { format!("{desc}: {}", ds.join("; ")) };
    let place = if place.is_empty() { its.iter().map(|i| layer_of(i.get_str("description", ""))).find(|l| !l.is_empty()).unwrap_or_default() } else { place.to_string() };
    Finding { source: source.into(), severity: v.get_str("severity", "warning").into(), kind: v.get_str("type", "").into(), text, place }
}
/// Checks.erc: in report order; none when the text is not JSON
pub fn erc(text: &str) -> Vec<Finding> {
    let Some(j) = bj::parse(text) else { return vec![] };
    j.get_obj("sheets").elems().iter().flat_map(|s| { let p = s.get_str("path", "/").to_string(); s.get_obj("violations").elems().iter().map(move |v| make("erc", &p, v)) }).collect()
}
/// Checks.drc: violations, then unconnected items, then schematic parity issues
pub fn drc(text: &str) -> Vec<Finding> {
    let Some(j) = bj::parse(text) else { return vec![] };
    ["violations", "unconnected_items", "schematic_parity"].iter().flat_map(|k| j.get_obj(k).elems().iter().map(|v| make("drc", "", v)).collect::<Vec<_>>()).collect()
}
fn is_str(v: Option<&J>) -> bool { matches!(v, Some(J::Str(_))) }
fn is_arr(v: Option<&J>) -> bool { matches!(v, Some(J::Arr(_))) }
/// Report.string, Report.array
pub fn report_string(o: &J, k: &str) -> bool { is_str(o.get(k)) }
pub fn report_array(o: &J, k: &str) -> bool { is_arr(o.get(k)) }
fn violation(v: &J) -> bool { matches!(v.get_str("severity", ""), "error" | "warning" | "exclusion") && is_str(v.get("type")) && is_str(v.get("description")) && is_arr(v.get("items")) }
fn list(o: &J, k: &str) -> bool { is_arr(o.get(k)) && o.get_obj(k).elems().iter().all(violation) }
/// Checks.report_valid: a failed tool or a malformed report can never read as a clean design
pub fn report_valid(id: &str, text: &str) -> bool {
    let Some(o) = bj::parse(text) else { return false };
    match id {
        "erc" => is_arr(o.get("sheets")) && o.get_obj("sheets").elems().iter().all(|s| is_str(s.get("path")) && list(s, "violations")),
        "drc" => list(&o, "violations") && list(&o, "unconnected_items") && list(&o, "schematic_parity"),
        _ => false,
    }
}
/// Checks.execution: failures keep the failing check's identity, apart from design findings and their thresholds
pub fn execution(fs: &[Finding]) -> String { fs.iter().filter(|f| f.source == "execution").map(|f| format!("{}: {}\n", f.kind, f.text)).collect() }
/// Checks.count
pub fn count(fs: &[Finding], source: &str, sev: &str) -> usize { fs.iter().filter(|f| f.source == source && f.severity == sev).count() }
fn plural(n: usize, w: &str) -> String { format!("{n} {w}{}", if n == 1 { "" } else { "s" }) }
fn label(s: &str) -> &str { match s { "erc" => "ERC", "drc" => "DRC", "bom" => "BOM", "checklist" => "Checklist", "custom" => "Custom", s => s } }
/// Checks.line: "ERC 2 errors, 1 warning; DRC 3 errors", or "No findings"
pub fn line(fs: &[Finding]) -> String {
    let ps: Vec<String> = SOURCES.iter().filter_map(|s| {
        let parts: Vec<String> = [("error", "error"), ("warning", "warning"), ("info", "note"), ("exclusion", "excluded")].iter().map(|(v, w)| (count(fs, s, v), *w)).filter(|(n, _)| *n > 0).map(|(n, w)| plural(n, w)).collect();
        if parts.is_empty() { None } else { Some(format!("{} {}", label(s), parts.join(", "))) }
    }).collect();
    if ps.is_empty() { "No findings".into() } else { ps.join("; ") }
}

// Checklist
/// Cl.box: 0 not a task line, 1 an open box, 2 a ticked one
fn boxed(line: &str) -> u8 {
    let t = bj::trim_start(line);
    if t.starts_with("- [ ]") || t.starts_with("* [ ]") { 1 } else if ["- [x]", "- [X]", "* [x]", "* [X]"].iter().any(|p| t.starts_with(p)) { 2 } else { 0 }
}
/// Cl.flip: a task line with its box flipped, every other byte as it was
fn flip(line: &str) -> String {
    let Some((p, r)) = line.split_once('[') else { return line.to_string() };
    let mut cs = r.chars();
    match cs.next() { Some(c) => format!("{p}[{}{}", if c == ' ' { 'x' } else { ' ' }, cs.as_str()), None => format!("{p}[") }
}
/// Checklist.toggle: the text with the index-th box flipped, everything else byte for byte; an index past the last box changes nothing
pub fn toggle(text: &str, index: u64) -> String {
    let (mut n, mut done, mut out) = (index, false, Vec::new());
    for l in text.split('\n') {
        if boxed(l) == 0 || done { out.push(l.to_string()) } else if n == 0 { done = true; out.push(flip(l)) } else { n -= 1; out.push(l.to_string()) }
    }
    out.join("\n")
}
/// Checklist.parse: the boxes of a checklist file, in order, each with its heading
pub fn parse(text: &str) -> Vec<Task> {
    let (mut g, mut o) = (String::new(), vec![]);
    for l in text.split('\n') {
        let b = boxed(l); let t = bj::trim_start(l);
        if b > 0 { o.push(Task { text: bj::trim(&bj::drop(t, 5)).to_string(), done: b == 2, group: g.clone() }) }
        else if t.starts_with('#') { g = bj::trim(t.trim_start_matches('#')).to_string() }
    }
    o
}
/// Checklist.open: boxes not ticked
pub fn open(ts: &[Task]) -> usize { ts.iter().filter(|t| !t.done).count() }
/// Checklist.default
pub const DEFAULT: &str = "# Power\n- [ ] Every rail has a source, a load budget and an enable\n- [ ] Decoupling on every IC power pin, placed close\n- [ ] Input protection: reverse polarity, overvoltage, inrush\n# Connectors\n- [ ] Pinouts match the mating part, pin 1 marked\n- [ ] ESD protection on exposed signals\n# Layout\n- [ ] Mounting holes and board outline checked against the enclosure\n- [ ] Clearances, creepage and current capacity of power tracks\n- [ ] Test points on key nets\n# Fabrication\n- [ ] Fab notes: stackup, finish, thickness, impedance\n- [ ] Silkscreen: polarity marks, reference designators legible\n- [ ] Board and schematic revision updated\n# Assembly\n- [ ] BOM complete: every part has an MPN and a footprint\n- [ ] DNP parts marked\n# Checks\n- [ ] ERC clean\n- [ ] DRC clean\n";

// Custom checks and gate
pub fn gate_default() -> Gate { Gate { erc: "error".into(), drc: "error".into(), checklist: false } }
/// Checks.config: the defaults when missing, empty or malformed. A check without a command is dropped; with no name its command is its name
pub fn config(text: &str) -> Config {
    let Some(j) = bj::parse(text) else { return Config { checks: vec![], gate: gate_default() } };
    let checks = j.get_obj("checks").elems().iter().filter_map(|c| {
        let (run, name) = (c.get_str("run", ""), c.get_str("name", ""));
        if run.is_empty() { return None }
        let on = match c.get("on") { Some(a) => a.elems().iter().filter_map(|x| if let J::Str(s) = x { Some(s.clone()) } else { None }).collect(), None => vec!["pr".to_string(), "release".into(), "manual".into()] };
        Some(Custom { name: if name.is_empty() { run } else { name }.to_string(), run: run.to_string(), on })
    }).collect();
    let g = j.get_obj("gate");
    Config { checks, gate: Gate { erc: g.get_str("erc", "error").into(), drc: g.get_str("drc", "error").into(), checklist: g.get_bool("checklist", false) } }
}
/// Checks.for: the checks to run for a trigger
pub fn for_trigger(cs: &[Custom], trigger: &str) -> Vec<Custom> { cs.iter().filter(|c| c.on.iter().any(|t| t == trigger)).cloned().collect() }
fn reason(rs: &mut Vec<String>, name: &str, fs: &[Finding], source: &str, level: &str) {
    let (e, w) = (count(fs, source, "error"), count(fs, source, "warning"));
    let n = match level { "error" => e, "warning" => e + w, _ => 0 };
    if n > 0 { rs.push(format!("{name}: {}", if level == "warning" { format!("{n} errors or warnings") } else { plural(n, "error") })) }
}
fn reasons(fs: &[Finding], g: &Gate) -> Vec<String> { let mut r = vec![]; reason(&mut r, "ERC", fs, "erc", &g.erc); reason(&mut r, "DRC", fs, "drc", &g.drc); r }
/// Checks.gate: "" when the findings pass, else why not; exclusions and notes never block, "off" turns a source off
pub fn gate(fs: &[Finding], g: &Gate) -> String { reasons(fs, g).join("; ") }
/// Checks.gate.all: findings, unticked boxes when the config says so, failed custom checks
pub fn gate_all(fs: &[Finding], tasks: &[Task], runs: &[Run], g: &Gate) -> String {
    let mut r = reasons(fs, g);
    let n = if g.checklist { open(tasks) } else { 0 };
    if n > 0 { r.push(format!("Checklist: {n} unticked")) }
    let f = runs.iter().filter(|x| !x.ok).count();
    if f > 0 { r.push(format!("Custom: {f} failed")) }
    r.join("; ")
}

// Report
/// Ct.esc: a table cell: no pipes, no line breaks
fn esc(s: &str) -> String { s.replace('\n', " ").replace('\r', "").replace('|', "\\|") }
/// Ct.cell: a shortened cell is marked, so one finding cannot overflow the view
pub fn cell(s: &str) -> String { if bj::len(&bj::take(s, 241)) > 240 { format!("{}…", esc(&bj::take(s, 240))) } else { esc(s) } }
fn more(n: usize) -> String { if n == 0 { String::new() } else { format!("\n_{n} more not shown_\n") } }
fn section(source: &str, fs: &[Finding]) -> String {
    let n: usize = ["error", "warning", "info", "exclusion"].iter().map(|v| count(fs, source, v)).sum();
    if n == 0 { return String::new() }
    let (mut rows, mut left, mut over) = (String::new(), CAP, 0);
    for f in fs.iter().filter(|f| f.source == source) {
        if left == 0 { over += 1 } else { left -= 1; rows.push_str(&format!("| {} | {} | {} | {} |\n", cell(&f.severity), cell(&f.kind), cell(&f.place), cell(&f.text))) }
    }
    format!("\n## {} ({n})\n\n| Severity | Type | Where | Description |\n|---|---|---|---|\n{rows}{}", label(source), more(over))
}
fn checklist_md(ts: &[Task]) -> String {
    if ts.is_empty() { return String::new() }
    let mut o = format!("\n## Checklist ({}/{})\n", ts.iter().filter(|t| t.done).count(), ts.len());
    let mut last = "\0";
    for t in ts { if t.group != last { o.push_str(&format!("\n**{}**\n\n", t.group)) } o.push_str(if t.done { "- [x] " } else { "- [ ] " }); o.push_str(&t.text); o.push('\n'); last = &t.group }
    o
}
fn runs_md(rs: &[Run]) -> String {
    if rs.is_empty() { return String::new() }
    let mut o = String::from("\n## Custom checks\n\n| Check | Result | Output |\n|---|---|---|\n");
    for r in rs { o.push_str(&format!("| {} | {} | {} |\n", esc(&r.name), if r.ok { "pass" } else { "FAIL" }, esc(&bj::take(bj::trim(r.out.split('\n').next().unwrap_or("")), 120)))) }
    o
}
/// Checks.md: one summary line, a table per source with findings (4 rows at most, then "N more"), the checklist, the custom checks' results
pub fn md(fs: &[Finding], tasks: &[Task], runs: &[Run]) -> String {
    let secs: String = SOURCES.iter().map(|s| section(s, fs)).collect();
    format!("**Design checks:** {}\n{secs}{}{}", line(fs), checklist_md(tasks), runs_md(runs))
}

#[cfg(test)]
mod tests {
    use super::*;
    const EX: &str = "# Power\n- [ ] a\n* [x] b\nplain\n  - [ ] nested\n# Fab\n- [X] c\n";
    fn flood(n: usize) -> Vec<Finding> { vec![finding("drc", "warning", "silk", "a|b\nc", "F.Cu"); n] }
    fn erc_text() -> &'static str { r#"{"sheets":[{"path":"/","violations":[{"description":"Footprint missing","items":[{"description":"Symbol P1 [DB25_Female]","pos":{"x":3.8227,"y":1.8288},"uuid":"a"}],"severity":"warning","type":"footprint_link_issues"},{"description":"Pin not connected","items":[{"description":"Symbol U1 Pin 1"}],"severity":"error","type":"pin_not_connected"}]}]}"# }
    fn drc_text() -> &'static str { r#"{"violations":[{"description":"Thermal relief","items":[{"description":"Zone [GND] on bottom_copper, priority 0","pos":{"x":1,"y":2}},{"description":"PTH pad 16 [GND] of U5"}],"severity":"error","type":"starved_thermal"}],"unconnected_items":[{"description":"Missing connection","items":[{"description":"Pad 1 [GND] of C21 on B.Cu"}],"severity":"error","type":"unconnected_items"}],"schematic_parity":[{"description":"Extra","items":[],"severity":"warning","type":"extra_footprint"},{"description":"Excluded","items":[],"severity":"exclusion","type":"x"}]}"# }
    #[test] fn checklist_toggle_twice() { assert_eq!(toggle(&toggle("# A\n- [ ] x\n* [x] y\n", 1), 1), "# A\n- [ ] x\n* [x] y\n") }
    #[test] fn checklist_toggle_flips_one() { assert_eq!(toggle("# A\n- [ ] x\n* [x] y\n", 1), "# A\n- [ ] x\n* [ ] y\n") }
    #[test] fn checklist_toggle_past_end() { assert_eq!(toggle("- [ ] x\n", 5), "- [ ] x\n") }
    #[test] fn checks_gate_no_findings() { assert_eq!(gate(&[], &gate_default()), "") }
    #[test] fn checks_gate_no_findings_strict() { assert_eq!(gate(&[], &Gate { erc: "warning".into(), drc: "warning".into(), checklist: true }), "") }
    #[test] fn checks_md_empty() { assert_eq!(md(&[], &[], &[]), "**Design checks:** No findings\n") }
    #[test] fn checks_erc_error_counted() { assert_eq!(count(&erc(r#"{"sheets":[{"path":"/","violations":[{"type":"t","severity":"error","description":"d","items":[]}]}]}"#), "erc", "error"), 1) }
    #[test] fn checks_detail_cap() { assert_eq!(CAP, 4) }
    #[test] fn checks_omitted_visible() { assert_eq!(more(185), "\n_185 more not shown_\n") }
    #[test] fn checks_report_missing_erc() { assert!(!report_valid("erc", "{}")) }
    #[test] fn checks_report_missing_drc() { assert!(!report_valid("drc", "{}")) }
    #[test] fn checks_execution_keeps_identity() {
        for (kind, why) in [("erc", "exit 1"), ("", ""), ("drc", "a\nb"), ("é", "|")] { assert_eq!(execution(&[finding("execution", "error", kind, why, "")]), format!("{kind}: {why}\n")) }
    }
    #[test] fn reads_reports_as_checks_test_does() {
        let fs = erc(erc_text()); assert_eq!(fs[0], finding("erc", "warning", "footprint_link_issues", "Footprint missing: Symbol P1 [DB25_Female]", "/")); assert_eq!(fs[1].severity, "error");
        let ds = drc(drc_text()); assert_eq!(ds[0].place, "bottom_copper"); assert_eq!(ds[0].text, "Thermal relief: Zone [GND] on bottom_copper, priority 0; PTH pad 16 [GND] of U5"); assert_eq!(ds[1].place, "B.Cu"); assert_eq!(ds[2].text, "Extra");
        assert_eq!(erc("{oops").len(), 0); assert_eq!(drc("{}").len(), 0);
        let both: Vec<Finding> = fs.iter().chain(ds.iter()).cloned().collect();
        assert_eq!(line(&both), "ERC 1 error, 1 warning; DRC 2 errors, 1 warning, 1 excluded"); assert_eq!(line(&[]), "No findings");
        assert_eq!(gate(&both, &gate_default()), "ERC: 1 error; DRC: 2 errors");
        assert_eq!(gate(&both, &Gate { erc: "warning".into(), drc: "warning".into(), checklist: false }), "ERC: 2 errors or warnings; DRC: 3 errors or warnings");
        assert_eq!(gate(&both, &Gate { erc: "off".into(), drc: "off".into(), checklist: false }), ""); assert_eq!(gate(&both, &Gate { erc: "error".into(), drc: "off".into(), checklist: false }), "ERC: 1 error");
        assert!(report_valid("erc", erc_text()) && report_valid("drc", drc_text()) && !report_valid("x", "{}") && !report_valid("erc", "[") );
        assert!(!report_valid("drc", r#"{"violations":[{"severity":"fatal","type":"t","description":"d","items":[]}],"unconnected_items":[],"schematic_parity":[]}"#));
    }
    #[test] fn checklist_as_checks_test_says() {
        let ts = parse(EX); let show: Vec<String> = ts.iter().map(|t| format!("{}:{}:{}", t.group, t.text, t.done)).collect();
        assert_eq!(show, ["Power:a:false", "Power:b:true", "Power:nested:false", "Fab:c:true"]); assert_eq!(open(&ts), 2);
        assert_eq!(toggle(EX, 1), "# Power\n- [ ] a\n* [ ] b\nplain\n  - [ ] nested\n# Fab\n- [X] c\n"); assert_eq!(toggle(EX, 3), "# Power\n- [ ] a\n* [x] b\nplain\n  - [ ] nested\n# Fab\n- [ ] c\n");
        assert_eq!(toggle(EX, 9), EX); assert_eq!(toggle(&toggle(EX, 2), 2), EX); assert_eq!(toggle("- [ ] a\r\n- [ ] b\r\n", 1), "- [ ] a\r\n- [x] b\r\n");
        assert_eq!(toggle("- [x] a", 0), "- [ ] a"); assert_eq!(toggle("- [ ] see [1] and [x]", 0), "- [x] see [1] and [x]");
        assert_eq!((parse(DEFAULT).len(), open(&parse(DEFAULT))), (15, 15));
    }
    #[test] fn config_as_checks_test_says() {
        let c = config(r#"{"checks":[{"name":"lint","run":"make lint","on":["pr"]},{"run":"echo hi"},{"name":"nocmd"}],"gate":{"erc":"warning","drc":"off","checklist":true}}"#);
        assert_eq!(c.checks, vec![Custom { name: "lint".into(), run: "make lint".into(), on: vec!["pr".into()] }, Custom { name: "echo hi".into(), run: "echo hi".into(), on: vec!["pr".into(), "release".into(), "manual".into()] }]);
        assert_eq!(c.gate, Gate { erc: "warning".into(), drc: "off".into(), checklist: true }); assert_eq!(for_trigger(&c.checks, "release").len(), 1); assert_eq!(for_trigger(&c.checks, "pr").len(), 2);
        assert_eq!(config(""), Config { checks: vec![], gate: gate_default() }); assert_eq!(config(r#"{"gate":{"drc":"warning"}}"#).gate.drc, "warning");
        assert_eq!(config(r#"{"checks":[{"run":"x","on":"pr"}]}"#).checks[0].on.len(), 0); // an "on" that is no list runs on nothing
        let runs = [Run { name: "lint".into(), ok: true, out: "ok\nmore".into() }, Run { name: "fmt".into(), ok: false, out: "bad | pipe".into() }];
        assert_eq!(gate_all(&[], &parse(EX), &runs, &Gate { erc: "error".into(), drc: "error".into(), checklist: true }), "Checklist: 2 unticked; Custom: 1 failed"); assert_eq!(gate_all(&[], &parse(EX), &[], &gate_default()), "");
        assert!(md(&[], &[], &runs).ends_with("\n## Custom checks\n\n| Check | Result | Output |\n|---|---|---|\n| lint | pass | ok |\n| fmt | FAIL | bad \\| pipe |\n"));
    }
    #[test] fn report_as_checks_test_says() {
        assert_eq!(md(&flood(1), &[], &[]), "**Design checks:** DRC 1 warning\n\n## DRC (1)\n\n| Severity | Type | Where | Description |\n|---|---|---|---|\n| warning | silk | F.Cu | a\\|b c |\n");
        assert!(md(&flood(9), &[], &[]).contains("\n_5 more not shown_\n")); assert!(!md(&flood(4), &[], &[]).contains("more not shown"));
        assert_eq!(cell(&"x".repeat(241)), format!("{}…", "x".repeat(240))); assert_eq!(cell(&"x".repeat(240)), "x".repeat(240));
        assert!(md(&flood(205), &[], &[]).contains("DRC 205 warnings"));
        assert!(md(&[], &parse(EX), &[]).ends_with("\n## Checklist (2/4)\n\n**Power**\n\n- [ ] a\n- [x] b\n- [ ] nested\n\n**Fab**\n\n- [x] c\n"));
    }
}

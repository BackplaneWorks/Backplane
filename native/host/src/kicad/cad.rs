//! core/cad.bend in Rust: which tool a file belongs to, and KiCad's check and output commands as argv templates ({cli}, {pcb}, {sch},
//! {out}, {name}). Pure data; the jobs (files/rv.rs) run the templates. Laws `cad_of_*`, `cad_fill_*`, `cad_shell_*`.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit { pub tool: String, pub role: String, pub draw: bool }
pub struct Chk { pub id: &'static str, pub label: &'static str, pub argv: &'static [&'static str], pub report: &'static str }
pub struct Out { pub id: &'static str, pub label: &'static str, pub argv: &'static [&'static str], pub on: bool }

pub const CHECKS: [Chk; 2] = [
    Chk { id: "erc", label: "ERC", argv: &["{cli}", "sch", "erc", "--format", "json", "--severity-all", "--units", "mm", "-o", "{out}/erc.json", "{sch}"], report: "erc.json" },
    Chk { id: "drc", label: "DRC", argv: &["{cli}", "pcb", "drc", "--format", "json", "--severity-all", "--units", "mm", "--schematic-parity", "-o", "{out}/drc.json", "{pcb}"], report: "drc.json" },
];
pub const OUTS: [Out; 9] = [
    Out { id: "gerbers", label: "Gerbers", argv: &["{cli}", "pcb", "export", "gerbers", "-o", "{out}/gerbers/", "{pcb}"], on: true },
    Out { id: "drill", label: "Drill files", argv: &["{cli}", "pcb", "export", "drill", "--format", "excellon", "--excellon-separate-th", "-o", "{out}/gerbers/", "{pcb}"], on: true },
    Out { id: "cpl", label: "Placement (CPL)", argv: &["{cli}", "pcb", "export", "pos", "--format", "csv", "--units", "mm", "--side", "both", "--exclude-dnp", "-o", "{out}/{name}-cpl.csv", "{pcb}"], on: true },
    Out { id: "bom", label: "BOM", argv: &["{cli}", "sch", "export", "bom", "--fields", "Reference,Value,Footprint,${QUANTITY},MPN,Manufacturer,${DNP}", "--labels", "Refs,Value,Footprint,Qty,MPN,Manufacturer,DNP", "--group-by", "Value,Footprint,MPN,Manufacturer", "--exclude-dnp", "-o", "{out}/{name}-bom.csv", "{sch}"], on: true },
    Out { id: "sch-pdf", label: "Schematic PDF", argv: &["{cli}", "sch", "export", "pdf", "-o", "{out}/{name}-schematic.pdf", "{sch}"], on: true },
    Out { id: "pcb-pdf", label: "Board PDF", argv: &["{cli}", "pcb", "export", "pdf", "--mode-multipage", "-l", "F.Cu,B.Cu,F.Silkscreen,B.Silkscreen,F.Mask,B.Mask,Edge.Cuts", "--cl", "Edge.Cuts", "-o", "{out}/{name}-board.pdf", "{pcb}"], on: true },
    Out { id: "step", label: "STEP model", argv: &["{cli}", "pcb", "export", "step", "--subst-models", "-f", "-o", "{out}/{name}.step", "{pcb}"], on: true },
    Out { id: "ipc2581", label: "IPC-2581", argv: &["{cli}", "pcb", "export", "ipc2581", "-o", "{out}/{name}-ipc2581.xml", "{pcb}"], on: false },
    Out { id: "odb", label: "ODB++", argv: &["{cli}", "pcb", "export", "odb", "-o", "{out}/{name}-odb.zip", "{pcb}"], on: false },
];
/// Cad.tools: (name, draw, extensions with their roles), KiCad first; an extension two tools share is the first's.
const TOOLS: [(&str, bool, &[(&str, &str)]); 5] = [
    ("kicad", true, &[("kicad_pcb", "board"), ("kicad_sch", "schematic"), ("kicad_pro", "project"), ("kicad_sym", "symbols"), ("kicad_mod", "footprint")]),
    ("altium", false, &[("pcbdoc", "board"), ("schdoc", "schematic"), ("prjpcb", "project")]),
    ("orcad", false, &[("dsn", "schematic"), ("opj", "project")]),
    ("allegro", false, &[("brd", "board")]),
    ("eagle", false, &[("brd", "board"), ("sch", "schematic")]),
];
/// Cad.ext: the text after the last '.', lower case ("" when there is no dot anywhere)
pub fn ext(path: &str) -> String { if path.contains('.') { path.rsplit('.').next().unwrap_or("").to_ascii_lowercase() } else { String::new() } }
/// Cad.of
pub fn of(path: &str) -> Hit {
    let e = ext(path);
    for (n, d, xs) in TOOLS { if let Some((_, r)) = xs.iter().find(|(x, _)| *x == e) { return Hit { tool: n.into(), role: (*r).into(), draw: d } } }
    Hit { tool: String::new(), role: String::new(), draw: false }
}
/// Cad.sub: every "{key}" in s replaced by val
pub fn sub(s: &str, key: &str, val: &str) -> String {
    let mut it = s.split('{'); let mut o = it.next().unwrap_or("").to_string(); let close = format!("{key}}}");
    for seg in it { match seg.strip_prefix(&close) { Some(rest) => { o.push_str(val); o.push_str(rest) } None => { o.push('{'); o.push_str(seg) } } }
    o
}
/// Cad.fill: the variables go in one after the other, so a value holding "{out}" is filled by a later variable, as in Bend
pub fn fill(argv: &[&str], vars: &[(&str, &str)]) -> Vec<String> { argv.iter().map(|a| vars.iter().fold(a.to_string(), |s, (k, v)| sub(&s, k, v))).collect() }
/// Cad.quote
pub fn quote(s: &str) -> String { format!("'{}'", s.replace('\'', "'\\''")) }
/// Cad.shell
pub fn shell(xs: &[&str]) -> String { xs.iter().map(|x| quote(x)).collect::<Vec<_>>().join(" ") }

#[cfg(test)]
mod tests {
    use super::*;
    fn hit(t: &str, r: &str, d: bool) -> Hit { Hit { tool: t.into(), role: r.into(), draw: d } }
    #[test] fn cad_of_kicad_board() { assert_eq!(of("a/b.kicad_pcb"), hit("kicad", "board", true)) }
    #[test] fn cad_of_kicad_schematic() { assert_eq!(of("x.kicad_sch"), hit("kicad", "schematic", true)) }
    #[test] fn cad_of_case_ignored() { assert_eq!(of("X.KICAD_PRO"), hit("kicad", "project", true)) }
    #[test] fn cad_of_altium_not_drawn() { assert_eq!(of("Main.PcbDoc"), hit("altium", "board", false)) }
    #[test] fn cad_of_unknown() { assert_eq!(of("readme.md"), hit("", "", false)) }
    #[test] fn cad_fill_puts_vars_in() { assert_eq!(fill(&["{cli}", "{out}/x.csv"], &[("cli", "k"), ("out", "/o")]), ["k", "/o/x.csv"]) }
    #[test] fn cad_fill_keeps_other_braces() { assert_eq!(fill(&["${QUANTITY}"], &[("cli", "k")]), ["${QUANTITY}"]) }
    #[test] fn cad_shell_quotes_a_quote() { assert_eq!(quote("a'b"), "'a'\\''b'") }
    #[test] fn cad_shell_words() { assert_eq!(shell(&["a", "b c"]), "'a' 'b c'") }
    #[test] fn brd_is_allegros_and_sch_eagles() { assert_eq!(of("a.brd"), hit("allegro", "board", false)); assert_eq!(of("a.sch"), hit("eagle", "schematic", false)); assert_eq!(ext("noext"), ""); assert_eq!(ext("a.b/c"), "b/c") }
}

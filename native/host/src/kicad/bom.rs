//! core/bom.bend in Rust: a schematic's placed symbols as parts (one per instance reference), natural order (C2 before C10), BOM lines,
//! a diff by reference, the rules, CSV and GFM tables. Pure: `Sheet` holds a file's bytes. Laws `bom_*`.
use super::sexpr::{forms, parse, Sx};
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq)]
pub struct Fld { pub k: String, pub v: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Part { pub reference: String, pub value: String, pub footprint: String, pub mpn: String, pub mfr: String, pub fields: Vec<Fld>, pub dnp: bool, pub inbom: bool, pub lib: String, pub sheet: String, pub unit: u128 }
pub struct Sheet { pub path: String, pub text: Vec<u8> }
#[derive(Clone, Debug, PartialEq)]
pub struct Line { pub refs: Vec<String>, pub qty: usize, pub value: String, pub footprint: String, pub mpn: String, pub mfr: String, pub dnp: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Dl { pub field: String, pub was: String, pub now: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Chg { pub reference: String, pub kind: &'static str, pub deltas: Vec<Dl> }
#[derive(Clone, Debug, PartialEq)]
pub struct Sum { pub part: String, pub was: usize, pub now: usize }
#[derive(Clone, Debug, PartialEq)]
pub struct Fnd { pub id: &'static str, pub sev: &'static str, pub reference: String, pub msg: String }

// Natural order
/// Bom.rk: "R12a" is ("R", "12", "a"): the digits kept without leading zeros, so numbers of any size compare exactly
fn rk(s: &str) -> (String, String, String) {
    let (mut pre, mut num, mut post, mut ph) = (String::new(), String::new(), String::new(), 0);
    for c in s.chars() {
        let d = c.is_ascii_digit();
        match ph { 0 if !d => pre.push(c), 0 | 1 if d => { num.push(c); ph = 1 } _ => { post.push(c); ph = 2 } }
    }
    (pre, num.trim_start_matches('0').to_string(), post)
}
fn nat(a: &str, b: &str) -> Ordering { a.len().cmp(&b.len()).then_with(|| a.cmp(b)) }
/// Bom.cmp.ref
pub fn cmp_ref(a: &str, b: &str) -> Ordering { let (x, y) = (rk(a), rk(b)); x.0.cmp(&y.0).then_with(|| nat(&x.1, &y.1)).then_with(|| x.2.cmp(&y.2)) }
/// Bom.before
pub fn before(a: &str, b: &str) -> bool { cmp_ref(a, b) == Ordering::Less }
fn cmp_part(a: &Part, b: &Part) -> Ordering { cmp_ref(&a.reference, &b.reference).then(a.unit.cmp(&b.unit)).then_with(|| a.sheet.cmp(&b.sheet)).then_with(|| a.lib.cmp(&b.lib)) }
/// Bom.sort: stable, as List.sort is
fn sorted(ps: &[Part]) -> Vec<Part> { let mut v = ps.to_vec(); v.sort_by(cmp_part); v }
/// Bom.uniq: the first part of each reference of a sorted list
fn uniq(ps: Vec<Part>) -> Vec<Part> { let mut o: Vec<Part> = vec![]; for p in ps { if o.last().map(|l| l.reference != p.reference).unwrap_or(p.reference != "\0") { o.push(p) } } o }
/// Bom.norm: what a diff compares
fn norm(ps: &[Part]) -> Vec<Part> { uniq(sorted(ps)) }
/// Bom.fold: another unit of the same reference folds into the first; a unit seen twice stays (the duplicate rule reports it)
fn fold(ps: Vec<Part>) -> Vec<Part> {
    let (mut o, mut lr, mut lu) = (vec![], "\0".to_string(), 0u128);
    for p in ps { let (r, u) = (p.reference.clone(), p.unit); if !(r == lr && u != lu) { o.push(p) } lr = r; lu = u }
    o
}

// Reading a symbol
/// Bom.nz: KiCad writes "~" for an empty field
fn nz(s: &str) -> String { if s == "~" { String::new() } else { s.to_string() } }
/// Bom.get: the value of the first field called name (lower case), "" when none
fn get<'a>(fs: &'a [Fld], name: &str) -> &'a str { fs.iter().find(|f| f.k.to_ascii_lowercase() == name).map(|f| f.v.as_str()).unwrap_or("") }
/// Bom.first: the first of several names that has a value
fn first(names: &[&str], fs: &[Fld]) -> String { let mut acc = String::new(); for n in names { if acc.is_empty() { acc = nz(get(fs, n)) } } acc }
const MPN: [&str; 5] = ["mpn", "manufacturer_part_number", "manufacturer part number", "mfr pn", "pn"];
const MFR: [&str; 2] = ["manufacturer", "mfr"];
fn supplier(k: &str) -> bool { matches!(k.to_ascii_lowercase().as_str(), "lcsc" | "mouser" | "digikey") }
/// Bom.num: a number written in digits (each character as its code less 48, as Bend reads it), d when blank
fn num(s: &str, d: u128) -> u128 { if s.is_empty() { d } else { s.chars().fold(0u128, |a, c| a.wrapping_mul(10).wrapping_add((c as u32).wrapping_sub(48) as u128)) } }
/// Bom.insts: every instance of a symbol; those of `project` when it has any, else all
fn insts(x: &Sx, project: &str) -> Vec<(String, u128)> {
    let i = x.get("instances");
    let all: Vec<&Sx> = i.all("project").collect();
    let mine: Vec<&Sx> = all.iter().copied().filter(|p| p.arg(0) == project).collect();
    (if mine.is_empty() { all } else { mine }).iter().flat_map(|p| p.all("path").map(|q| (q.attr("reference").to_string(), num(q.attr("unit"), 1))).collect::<Vec<_>>()).collect()
}
/// Bom.sym
fn sym(sheet: &str, project: &str, x: &Sx) -> Vec<Part> {
    let lib = x.attr("lib_id");
    let fs: Vec<Fld> = x.all("property").map(|p| Fld { k: p.arg(0).to_string(), v: p.arg(1).to_string() }).collect();
    let (dnp, inbom) = (x.attr("dnp") == "yes", x.attr("in_bom") != "no");
    let mut ins = insts(x, project);
    if ins.is_empty() { ins.push((get(&fs, "reference").to_string(), num(x.attr("unit"), 1))) }
    ins.into_iter().filter(|(r, _)| !(lib.starts_with("power:") || r.starts_with('#'))).map(|(reference, unit)| Part {
        reference, value: nz(get(&fs, "value")), footprint: nz(get(&fs, "footprint")), mpn: first(&MPN, &fs), mfr: first(&MFR, &fs),
        fields: fs.iter().filter(|f| supplier(&f.k) && !nz(&f.v).is_empty()).cloned().collect(), dnp, inbom, lib: lib.to_string(), sheet: sheet.to_string(), unit }).collect()
}
/// Bom.issym: "(symbol " and not "(symbol_instances" or "(lib_symbols"
fn is_sym(b: &[u8]) -> bool { b.len() >= 8 && &b[..7] == b"(symbol" && matches!(b[7], b' ' | b'\n' | b'\t' | b'\r') }
/// Bom.sheet: every part of one sheet file, unsorted; project "" takes all instances
fn sheet(project: &str, sh: &Sheet) -> Vec<Part> {
    forms(&sh.text).map(|fs| fs.into_iter().filter(|f| is_sym(f)).flat_map(|f| sym(&sh.path, project, &parse(f))).collect()).unwrap_or_default()
}
/// Bom.parts: natural order, a multi-unit part once, parts KiCad leaves out of the BOM dropped; a reference two symbols share stays twice
pub fn parts(project: &str, shs: &[Sheet]) -> Vec<Part> {
    let ps: Vec<Part> = shs.iter().flat_map(|s| sheet(project, s)).filter(|p| p.inbom).collect();
    fold(sorted(&ps))
}

// BOM lines
fn cmp_key(a: &Part, b: &Part) -> Ordering { a.value.cmp(&b.value).then_with(|| a.footprint.cmp(&b.footprint)).then_with(|| a.mpn.cmp(&b.mpn)).then(a.dnp.cmp(&b.dnp)) }
fn line_of(p: &Part) -> Line { Line { refs: vec![p.reference.clone()], qty: 1, value: p.value.clone(), footprint: p.footprint.clone(), mpn: p.mpn.clone(), mfr: p.mfr.clone(), dnp: p.dnp } }
/// Bom.group: parts with the same value, footprint, MPN and DNP together, the lines by their first reference. The finished lines build
/// newest first, so lines whose first references compare equal ("R1", "R01") come out as Bom.group.go leaves them.
pub fn group(ps: &[Part]) -> Vec<Line> {
    let mut v = uniq(sorted(ps)); v.sort_by(|a, b| cmp_key(a, b).then_with(|| cmp_part(a, b)));
    let mut it = v.iter();
    let Some(p0) = it.next() else { return vec![] };
    let (mut cur, mut acc): (Line, Vec<Line>) = (line_of(p0), vec![]);
    for p in it {
        let same = cur.value == p.value && cur.footprint == p.footprint && cur.mpn == p.mpn && cur.dnp == p.dnp;
        if same { cur.refs.push(p.reference.clone()); cur.qty += 1 } else { acc.push(std::mem::replace(&mut cur, line_of(p))) }
    }
    acc.push(cur); acc.reverse();
    acc.sort_by(|a, b| cmp_ref(a.refs.first().map(String::as_str).unwrap_or(""), b.refs.first().map(String::as_str).unwrap_or("")));
    acc
}
/// Bom.qty.ok: every line's quantity is the number of its references
pub fn qty_ok(ls: &[Line]) -> bool { ls.iter().all(|l| l.qty == l.refs.len()) }

// Diff
fn dl(out: &mut Vec<Dl>, field: &str, was: &str, now: &str) { if was != now { out.push(Dl { field: field.into(), was: was.into(), now: now.into() }) } }
/// fields of the old part, then those only the new one has
fn fdiff(o: &[Fld], n: &[Fld], out: &mut Vec<Dl>) {
    for f in o { dl(out, &f.k, &f.v, get(n, &f.k.to_ascii_lowercase())) }
    for f in n { if get(o, &f.k.to_ascii_lowercase()).is_empty() { out.push(Dl { field: f.k.clone(), was: String::new(), now: f.v.clone() }) } }
}
fn yes(b: bool) -> &'static str { if b { "yes" } else { "no" } }
fn yes_if(b: bool) -> &'static str { if b { "yes" } else { "" } }
/// Bom.changes
fn changes(o: &Part, n: &Part) -> Vec<Dl> {
    let mut v = vec![];
    dl(&mut v, "value", &o.value, &n.value); dl(&mut v, "footprint", &o.footprint, &n.footprint); dl(&mut v, "mpn", &o.mpn, &n.mpn); dl(&mut v, "mfr", &o.mfr, &n.mfr); dl(&mut v, "dnp", yes(o.dnp), yes(n.dnp));
    fdiff(&o.fields, &n.fields, &mut v); v
}
/// Bom.facts: what a part is, as deltas from nothing
fn facts(p: &Part) -> Vec<Dl> {
    let mut v = vec![];
    dl(&mut v, "value", "", &p.value); dl(&mut v, "footprint", "", &p.footprint); dl(&mut v, "mpn", "", &p.mpn); dl(&mut v, "mfr", "", &p.mfr); dl(&mut v, "dnp", "", yes_if(p.dnp));
    fdiff(&[], &p.fields, &mut v); v
}
/// Bom.diff: the changes from one BOM to another, by reference, in natural order
pub fn diff(old: &[Part], new: &[Part]) -> Vec<Chg> {
    let (os, ns) = (norm(old), norm(new));
    let (mut i, mut j, mut acc) = (0, 0, vec![]);
    while i < os.len() || j < ns.len() {
        let side = match (os.get(i), ns.get(j)) { (Some(o), Some(n)) => cmp_ref(&o.reference, &n.reference), (Some(_), None) => Ordering::Less, _ => Ordering::Greater };
        match side {
            Ordering::Equal => { let ds = changes(&os[i], &ns[j]); if !ds.is_empty() { acc.push(Chg { reference: os[i].reference.clone(), kind: "changed", deltas: ds }) } i += 1; j += 1 }
            Ordering::Less => { let ds = facts(&os[i]).into_iter().map(|d| Dl { field: d.field, was: d.now, now: d.was }).collect(); acc.push(Chg { reference: os[i].reference.clone(), kind: "removed", deltas: ds }); i += 1 }
            Ordering::Greater => { acc.push(Chg { reference: ns[j].reference.clone(), kind: "added", deltas: facts(&ns[j]) }); j += 1 }
        }
    }
    acc
}
fn count(kind: &str, cs: &[Chg]) -> usize { cs.iter().filter(|c| c.kind == kind).count() }

// Parts summary
/// Bom.label: a part as the buyer sees it: its MPN, else its value and footprint
fn label(p: &Part) -> String { if p.mpn.is_empty() { format!("{} {}", p.value, p.footprint) } else { p.mpn.clone() } }
/// Bom.tally: quantity of each part (by label), labels ascending
fn tally(ps: &[Part]) -> Vec<(String, usize)> {
    let mut ls: Vec<String> = uniq(sorted(ps)).iter().map(label).collect(); ls.sort();
    let mut o: Vec<(String, usize)> = vec![];
    for l in ls { match o.last_mut() { Some(t) if t.0 == l => t.1 += 1, _ => o.push((l, 1)) } }
    o
}
/// Bom.summary: each part's quantity in the old BOM and the new
pub fn summary(old: &[Part], new: &[Part]) -> Vec<Sum> {
    let (os, ns) = (tally(old), tally(new));
    let (mut i, mut j, mut acc) = (0, 0, vec![]);
    while i < os.len() || j < ns.len() {
        let side = match (os.get(i), ns.get(j)) { (Some(o), Some(n)) => o.0.cmp(&n.0), (Some(_), None) => Ordering::Less, _ => Ordering::Greater };
        match side {
            Ordering::Equal => { acc.push(Sum { part: os[i].0.clone(), was: os[i].1, now: ns[j].1 }); i += 1; j += 1 }
            Ordering::Less => { acc.push(Sum { part: os[i].0.clone(), was: os[i].1, now: 0 }); i += 1 }
            Ordering::Greater => { acc.push(Sum { part: ns[j].0.clone(), was: 0, now: ns[j].1 }); j += 1 }
        }
    }
    acc
}

// Rules
fn fnd(id: &'static str, sev: &'static str, r: &str, msg: &str) -> Fnd { Fnd { id, sev, reference: r.into(), msg: msg.into() } }
/// Bom.r.mpnfp: one MPN on parts of different footprints; a run's references newest first, the newest finished run first
fn mpnfp(ps: &[Part]) -> Vec<Fnd> {
    let mut v = uniq(sorted(ps)); v.sort_by(|a, b| a.mpn.cmp(&b.mpn).then_with(|| cmp_part(a, b)));
    let (mut mpn, mut fp, mut mixed, mut refs, mut out): (String, String, bool, Vec<String>, Vec<Fnd>) = (String::new(), String::new(), false, vec![], vec![]);
    fn flush(mixed: bool, refs: &[String], mpn: &str, out: Vec<Fnd>) -> Vec<Fnd> {
        if !mixed { return out }
        let mut o: Vec<Fnd> = refs.iter().rev().map(|r| fnd("bom_mpn_footprint", "error", r, &format!("MPN {mpn} is used with different footprints"))).collect();
        o.extend(out); o
    }
    for p in &v {
        if p.mpn.is_empty() { continue }
        if p.mpn == mpn { mixed = mixed || fp != p.footprint; refs.push(p.reference.clone()) }
        else { out = flush(mixed, &refs, &mpn, out); mpn = p.mpn.clone(); fp = p.footprint.clone(); mixed = false; refs = vec![p.reference.clone()] }
    }
    flush(mixed, &refs, &mpn, out)
}
/// Bom.rules: every finding of the design checks, over `parts`
pub fn rules(ps: &[Part]) -> Vec<Fnd> {
    let mut o = vec![];
    let (mut last, mut rep) = ("\0".to_string(), false);
    for p in sorted(ps) { let same = p.reference == last; if same && !rep { o.push(fnd("bom_duplicate_ref", "error", &p.reference, "reference used twice")) } rep = same; last = p.reference }
    for p in ps { if p.reference.is_empty() || p.reference.ends_with('?') { o.push(fnd("bom_unannotated", "error", &p.reference, "reference not annotated")) } }
    for p in ps { if p.footprint.is_empty() { o.push(fnd("bom_missing_footprint", "warning", &p.reference, "no footprint")) } }
    for p in ps { if p.value.is_empty() { o.push(fnd("bom_missing_value", "warning", &p.reference, "no value")) } }
    if ps.iter().any(|p| !p.mpn.is_empty()) { for p in ps { if p.inbom && p.mpn.is_empty() { o.push(fnd("bom_missing_mpn", "warning", &p.reference, "no MPN, though other parts have one")) } } }
    o.extend(mpnfp(ps)); o
}

// Output
/// Bom.q: a CSV cell, quoted when it holds a comma, a quote or a line break
pub fn q(s: &str) -> String { if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() } }
/// Bom.csv
pub fn csv(ls: &[Line]) -> String {
    let mut o = String::from("Reference,Qty,Value,Footprint,MPN,Manufacturer,DNP\n");
    for l in ls { o.push_str(&format!("{},{},{},{},{},{},{}\n", q(&l.refs.join(", ")), l.qty, q(&l.value), q(&l.footprint), q(&l.mpn), q(&l.mfr), yes_if(l.dnp))) }
    o
}
/// Bom.c: a Markdown table cell: a pipe as "/", line breaks flattened
pub fn c(s: &str) -> String { s.chars().filter(|c| *c != '\r').map(|c| match c { '|' => '/', '\n' => ' ', c => c }).collect() }
fn row(cs: &[&str]) -> String { let mut o = String::new(); for x in cs { o.push_str("| "); o.push_str(&c(x)); o.push(' ') } o.push_str("|\n"); o }
/// Bom.md: a BOM's lines as a GFM table; MPN, Manufacturer and DNP only when some line has one
pub fn md(ls: &[Line]) -> String {
    let (km, kf, kd) = (ls.iter().any(|l| !l.mpn.is_empty()), ls.iter().any(|l| !l.mfr.is_empty()), ls.iter().any(|l| l.dnp));
    let pick = |cs: [&str; 7]| -> String {
        let mut v: Vec<&str> = cs[..4].to_vec();
        if km { v.push(cs[4]) } if kf { v.push(cs[5]) } if kd { v.push(cs[6]) }
        row(&v)
    };
    let mut o = pick(["Reference", "Qty", "Value", "Footprint", "MPN", "Manufacturer", "DNP"]) + &pick(["---"; 7]);
    for l in ls { let (r, n) = (l.refs.join(", "), l.qty.to_string()); o.push_str(&pick([&r, &n, &l.value, &l.footprint, &l.mpn, &l.mfr, yes_if(l.dnp)])) }
    o
}
/// Bom.line: "3 added, 1 removed, 2 changed", or "No changes"
pub fn line(a: usize, r: usize, ch: usize) -> String { if a + r + ch == 0 { "No changes".into() } else { format!("{a} added, {r} removed, {ch} changed") } }
fn dls(ds: &[Dl], was: bool) -> String { ds.iter().filter_map(|d| { let v = if was { &d.was } else { &d.now }; if v.is_empty() { None } else { Some(format!("{}: {v}", d.field)) } }).collect::<Vec<_>>().join("; ") }
/// Bom.diff.md: the summary line, the changes by reference, and the parts whose quantity moved
pub fn diff_md(old: &[Part], new: &[Part]) -> String {
    let (cs, ss) = (diff(old, new), summary(old, new));
    if cs.is_empty() { return "No changes".into() }
    let mut o = format!("{}\n\n{}| --- | --- | --- | --- |\n", line(count("added", &cs), count("removed", &cs), count("changed", &cs)), row(&["Ref", "Change", "Was", "Now"]));
    for ch in &cs { o.push_str(&row(&[&ch.reference, ch.kind, &dls(&ch.deltas, true), &dls(&ch.deltas, false)])) }
    if ss.iter().any(|s| s.was != s.now) {
        o.push('\n'); o.push_str(&row(&["Part", "Was", "Now"])); o.push_str("| --- | --- | --- |\n");
        for s in ss.iter().filter(|s| s.was != s.now) { o.push_str(&row(&[&s.part, &s.was.to_string(), &s.now.to_string()])) }
    }
    o
}
/// Rvc.one: one BOM: how many parts in how many lines, the lines, what the BOM rules found
pub fn one(ps: &[Part]) -> String {
    if ps.is_empty() { return "No parts.".into() }
    let ls = group(ps); let fs = rules(ps);
    let found = if fs.is_empty() { String::new() } else { format!("\n\nChecks\n\n{}", fs.iter().map(|f| format!("- **{}** {} _({})_", f.reference, f.msg, f.sev)).collect::<Vec<_>>().join("\n")) };
    format!("{} parts in {} lines\n\n{}{found}", ps.len(), ls.len(), md(&ls))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mk(r: &str, v: &str, f: &str) -> Part { Part { reference: r.into(), value: v.into(), footprint: f.into(), mpn: String::new(), mfr: String::new(), fields: vec![], dnp: false, inbom: true, lib: String::new(), sheet: String::new(), unit: 1 } }
    /// Bom.sample
    fn sample() -> Vec<Part> { vec![mk("R2", "10k", "0603"), mk("R1", "10k", "0603"), mk("C1", "1u", "0402")] }
    #[test] fn bom_nat_c2_c10() { assert!(before("C2", "C10")) }
    #[test] fn bom_nat_r10_r2() { assert!(!before("R10", "R2")) }
    #[test] fn bom_diff_self() { assert!(diff(&sample(), &sample()).is_empty()) }
    #[test] fn bom_group_qty() { assert!(qty_ok(&group(&sample()))) }
    #[test] fn bom_group_csv() { assert_eq!(csv(&group(&sample())), "Reference,Qty,Value,Footprint,MPN,Manufacturer,DNP\nC1,1,1u,0402,,,\n\"R1, R2\",2,10k,0603,,,\n") }
    #[test] fn bom_csv_quote() { assert_eq!(q("a,b"), "\"a,b\"") }
    #[test] fn bom_csv_quote_inner() { assert_eq!(q("5\" x"), "\"5\"\" x\"") }
    #[test] fn bom_csv_plain() { assert_eq!(q("10k"), "10k") }
    #[test] fn bom_diff_line() { assert_eq!(line(3, 1, 2), "3 added, 1 removed, 2 changed") }
    #[test] fn bom_diff_md_none() { assert_eq!(diff_md(&sample(), &sample()), "No changes") }
    #[test] fn bom_md_pipe() { assert_eq!(c("a|b"), "a/b") }

    // the fixtures and the expected text are test/bom_test.bend's
    fn symu(lib: &str, r: &str, v: &str, fp: &str, extra: &str, flags: &str, inst: &str, unit: &str) -> String {
        format!("(symbol (lib_id \"{lib}\") (at 10 10 0) (unit {unit}) {flags} (uuid \"u-{r}\") (property \"Reference\" \"{r}\" (at 0 0 0) (effects (font (size 1 1)))) (property \"Value\" \"{v}\" (at 0 0 0)) (property \"Footprint\" \"{fp}\" (at 0 0 0) (effects (hide yes))) (property \"Datasheet\" \"~\" (at 0 0 0)) {extra}{inst})\n")
    }
    fn sym(lib: &str, r: &str, v: &str, fp: &str, extra: &str, flags: &str, inst: &str) -> String { symu(lib, r, v, fp, extra, flags, inst, "1") }
    fn flags(dnp: &str) -> String { format!("(exclude_from_sim no) (in_bom yes) (on_board yes) (dnp {dnp})") }
    fn prop(k: &str, v: &str) -> String { format!("(property \"{k}\" \"{v}\" (at 0 0 0)) ") }
    fn r(rf: &str, v: &str, extra: &str, dnp: &str) -> String { sym("Device:R", rf, v, "Resistor_SMD:R_0603", extra, &flags(dnp), "") }
    fn sheet_of(path: &str, body: &str) -> Sheet {
        Sheet { path: path.into(), text: format!("(kicad_sch (version 20250114) (generator \"eeschema\") (uuid \"root\") (lib_symbols (symbol \"Device:R\" (property \"Reference\" \"R\" (at 0 0 0)) (symbol \"R_0_1\" (rectangle (start 0 0) (end 1 1))))) {body} (sheet_instances (path \"/\" (page \"1\"))))\n").into_bytes() }
    }
    fn u1() -> String { let (m, f) = (prop("MPN", "OPA2134"), flags("no")); sym("MCU:Dual", "U1", "OPA2", "Package_SO:SOIC-8", &m, &f, "") + &symu("MCU:Dual", "U1", "OPA2", "Package_SO:SOIC-8", &m, &f, "", "2") }
    fn pwr() -> String { let f = flags("no"); sym("power:GND", "#PWR01", "GND", "", "", &f, "") + &sym("Device:R", "#FLG02", "X", "", "", &f, "") }
    fn v1() -> String {
        r("R10", "22k", &prop("MPN", "RC-B"), "no") + &r("R2", "10k", &prop("MPN", "RC-A"), "no") + &r("R1", "10k", &prop("mpn", "RC-A"), "no") + &sym("Device:C", "C1", "100n", "Capacitor_SMD:C_0402", "", &flags("no"), "")
            + &sym("Device:LED", "D1", "LED", "LED_SMD:LED_0603", &(prop("Manufacturer", "Acme") + &prop("Datasheet", "x")), &flags("no"), "") + &u1() + &pwr()
    }
    fn v2() -> String {
        r("R10", "22k", &prop("MPN", "RC-B"), "yes") + &r("R2", "10k", &prop("MPN", "RC-A"), "no") + &r("R1", "4k7", &prop("MPN", "RC-C"), "no") + &sym("Device:C", "C2", "1u", "Capacitor_SMD:C_0603", "", &flags("no"), "")
            + &sym("Device:LED", "D1", "LED", "LED_SMD:LED_0603", &(prop("Manufacturer", "Acme") + &prop("LCSC", "C2286")), &flags("no"), "") + &u1() + &pwr()
    }
    fn p(body: &str) -> Vec<Part> { parts("", &[sheet_of("a.kicad_sch", body)]) }
    fn refs(ps: &[Part]) -> String { ps.iter().map(|x| format!("{} ", x.reference)).collect() }
    fn fnds(fs: &[Fnd]) -> String { fs.iter().map(|f| format!("{}/{}/{} ", f.id, f.sev, f.reference)).collect() }
    fn chg(cs: &[Chg]) -> String { cs.iter().map(|c| format!("{}:{}:{} ", c.reference, c.kind, c.deltas.iter().map(|d| format!("{}[{}>{}]", d.field, d.was, d.now)).collect::<String>())).collect() }
    #[test] fn parts_as_bom_test_says() {
        assert_eq!(refs(&p(&v1())), "C1 D1 R1 R2 R10 U1 ");
        let vals: String = p(&v1()).iter().map(|x| format!("{}={}/{}/{} ", x.reference, x.value, x.mpn, x.mfr)).collect();
        assert_eq!(vals, "C1=100n// D1=LED//Acme R1=10k/RC-A/ R2=10k/RC-A/ R10=22k/RC-B/ U1=OPA2/OPA2134/ ");
        assert_eq!(p(&v1()).iter().filter(|x| x.reference == "U1").count(), 1);
        let multi = sym("Device:R_Potentiometer", "RV?", "10k", "Potentiometer_THT:Pot", "", &flags("no"), "(instances (project \"main\" (path \"/a/b\" (reference \"RV201\") (unit 1)) (path \"/a/c\" (reference \"RV301\") (unit 1))) (project \"old\" (path \"/z\" (reference \"RV9\") (unit 1))))");
        assert_eq!(refs(&parts("main", &[sheet_of("m.kicad_sch", &multi)])), "RV201 RV301 "); assert_eq!(refs(&parts("", &[sheet_of("m.kicad_sch", &multi)])), "RV9 RV201 RV301 ");
        assert_eq!(refs(&parts("", &[sheet_of("a.kicad_sch", &r("R1", "1k", "", "no")), sheet_of("b.kicad_sch", &r("R2", "1k", "", "no"))])), "R1 R2 ");
        assert_eq!(refs(&p(&(sym("Mech:Hole", "H1", "M3", "", "", "(in_bom no) (dnp no)", "") + &r("R1", "1k", "", "no")))), "R1 ");
        assert!(before("C9", "R1"));
        assert_eq!(parts("", &[Sheet { path: "x".into(), text: b"(kicad_sch (symbol".to_vec() }]).len(), 0); // a file still being written
    }
    #[test] fn tables_as_bom_test_says() {
        assert_eq!(csv(&group(&p(&v1()))), "Reference,Qty,Value,Footprint,MPN,Manufacturer,DNP\nC1,1,100n,Capacitor_SMD:C_0402,,,\nD1,1,LED,LED_SMD:LED_0603,,Acme,\n\"R1, R2\",2,10k,Resistor_SMD:R_0603,RC-A,,\nR10,1,22k,Resistor_SMD:R_0603,RC-B,,\nU1,1,OPA2,Package_SO:SOIC-8,OPA2134,,\n");
        assert_eq!(md(&group(&p(&v2()))), "| Reference | Qty | Value | Footprint | MPN | Manufacturer | DNP |\n| --- | --- | --- | --- | --- | --- | --- |\n| C2 | 1 | 1u | Capacitor_SMD:C_0603 |  |  |  |\n| D1 | 1 | LED | LED_SMD:LED_0603 |  | Acme |  |\n| R1 | 1 | 4k7 | Resistor_SMD:R_0603 | RC-C |  |  |\n| R2 | 1 | 10k | Resistor_SMD:R_0603 | RC-A |  |  |\n| R10 | 1 | 22k | Resistor_SMD:R_0603 | RC-B |  | yes |\n| U1 | 1 | OPA2 | Package_SO:SOIC-8 | OPA2134 |  |  |\n");
        assert_eq!(md(&[Line { refs: vec!["R1".into()], qty: 1, value: "10k".into(), footprint: "R_0603".into(), mpn: String::new(), mfr: String::new(), dnp: false }]), "| Reference | Qty | Value | Footprint |\n| --- | --- | --- | --- |\n| R1 | 1 | 10k | R_0603 |\n");
    }
    #[test] fn diffs_as_bom_test_says() {
        let (a, b) = (p(&v1()), p(&v2())); let d = diff(&a, &b);
        assert_eq!(chg(&d), "C1:removed:value[100n>]footprint[Capacitor_SMD:C_0402>] C2:added:value[>1u]footprint[>Capacitor_SMD:C_0603] D1:changed:LCSC[>C2286] R1:changed:value[10k>4k7]mpn[RC-A>RC-C] R10:changed:dnp[no>yes] ");
        let sums: String = summary(&a, &b).iter().map(|s| format!("{}:{}>{} ", s.part, s.was, s.now)).collect();
        assert_eq!(sums, "100n Capacitor_SMD:C_0402:1>0 1u Capacitor_SMD:C_0603:0>1 LED LED_SMD:LED_0603:1>1 OPA2134:1>1 RC-A:2>1 RC-B:1>1 RC-C:0>1 ");
        assert_eq!(diff_md(&a, &b), "1 added, 1 removed, 3 changed\n\n| Ref | Change | Was | Now |\n| --- | --- | --- | --- |\n| C1 | removed | value: 100n; footprint: Capacitor_SMD:C_0402 |  |\n| C2 | added |  | value: 1u; footprint: Capacitor_SMD:C_0603 |\n| D1 | changed |  | LCSC: C2286 |\n| R1 | changed | value: 10k; mpn: RC-A | value: 4k7; mpn: RC-C |\n| R10 | changed | dnp: no | dnp: yes |\n\n| Part | Was | Now |\n| --- | --- | --- |\n| 100n Capacitor_SMD:C_0402 | 1 | 0 |\n| 1u Capacitor_SMD:C_0603 | 0 | 1 |\n| RC-A | 2 | 1 |\n| RC-C | 0 | 1 |\n");
        assert_eq!(diff_md(&a, &a), "No changes");
    }
    #[test] fn rules_as_bom_test_says() {
        let dup = r("R1", "1k", "", "no") + &r("R1", "2k", "", "no") + &r("R4?", "", "", "no") + &r("R5", "1k", &prop("MPN", "X1"), "yes") + &sym("Device:R", "R6", "1k", "Resistor_SMD:R_0402", &prop("MPN", "X1"), &flags("no"), "");
        assert_eq!(fnds(&rules(&parts("", &[sheet_of("d.kicad_sch", &dup)]))), "bom_duplicate_ref/error/R1 bom_unannotated/error/R4? bom_missing_value/warning/R4? bom_missing_mpn/warning/R1 bom_missing_mpn/warning/R1 bom_missing_mpn/warning/R4? bom_mpn_footprint/error/R6 bom_mpn_footprint/error/R5 ");
        assert_eq!(fnds(&rules(&p(&v1()))), "bom_missing_mpn/warning/C1 bom_missing_mpn/warning/D1 ");
        assert_eq!(fnds(&rules(&parts("", &[sheet_of("a.kicad_sch", &r("R1", "1k", "", "no")), sheet_of("b.kicad_sch", &r("R2", "1k", "", "no"))]))), "");
        assert_eq!(one(&[]), "No parts."); assert!(one(&p(&v1())).starts_with("6 parts in 5 lines\n\n| Reference | Qty |")); assert!(one(&p(&v1())).ends_with("\n\nChecks\n\n- **C1** no MPN, though other parts have one _(warning)_\n- **D1** no MPN, though other parts have one _(warning)_"));
    }
    #[test] fn numbers_of_any_size_order_exactly() {
        assert!(before("R99999999999999999999999999999999999998", "R99999999999999999999999999999999999999")); assert_eq!(cmp_ref("R007", "R7"), Ordering::Equal); assert!(before("R1", "R1a")); assert!(before("R", "R0") == false); assert_eq!(cmp_ref("R", "R0"), Ordering::Equal);
    }
}

//! core/release in Rust: a tag's name, the next tag to propose, the plan of commands that build a release's outputs from KiCad's
//! templates (cad.rs), the manifest, the notes and the `gh release create` command. Pure. Laws `release_*`.
use super::bj::{self, J};
use super::cad;
use std::cmp::Ordering;

/// Rel.badc: a character git refuses in a ref name, or whitespace and control characters
fn bad(c: char) -> bool { let u = c as u32; u <= 32 || u == 127 || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\') }
/// Release.valid
pub fn valid(t: &str) -> bool {
    !t.is_empty() && !t.chars().any(bad) && !t.contains("..") && !t.contains("@{") && !t.contains("//") && !t.contains("/.") && t != "@"
        && !t.starts_with('-') && !t.starts_with('/') && !t.starts_with('.') && !t.ends_with(".lock") && !t.ends_with('.') && !t.ends_with('/')
}
/// Release.object_valid: full Git object ids only; an empty refspec would delete a remote tag
pub fn object_valid(s: &str) -> bool { (s.len() == 40 || s.len() == 64) && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) }
/// Release.nocommit
pub fn nocommit(dir: &str) -> String { format!("No release: a release tags a commit, and {dir} has none (it is not a git repository yet, or nothing is committed). Make one first: cd {dir} && git init && git add -A && git commit -m start") }

/// Rel.inc on the reversed digit run: 9 carries; past the run a new leading 1
fn inc(s: &[char]) -> Vec<char> {
    match s.split_first() {
        None => vec!['1'],
        Some((c, t)) if !c.is_ascii_digit() => { let _ = t; let mut v = vec!['1']; v.extend_from_slice(s); v }
        Some(('9', t)) => { let mut v = vec!['0']; v.extend(inc(t)); v }
        Some((c, t)) => { let mut v = vec![char::from_u32(*c as u32 + 1).unwrap()]; v.extend_from_slice(t); v }
    }
}
/// Release.bump: a trailing number goes up, a trailing single letter goes to the next (Z and z to AA and aa); anything else stays
pub fn bump(t: &str) -> String {
    let r: Vec<char> = t.chars().rev().collect();
    let out = match r.split_first() {
        None => vec![],
        Some((c, _)) if c.is_ascii_digit() => inc(&r),
        Some((c, rest)) if c.is_ascii_alphabetic() && rest.first().map(|n| !n.is_ascii_alphabetic()).unwrap_or(true) => {
            let mut v = if *c == 'Z' || *c == 'z' { let a = char::from_u32(*c as u32 - 25).unwrap(); vec![a, a] } else { vec![char::from_u32(*c as u32 + 1).unwrap()] };
            v.extend_from_slice(rest); v
        }
        _ => r.clone(),
    };
    out.iter().rev().collect()
}
pub fn bumpable(t: &str) -> bool { valid(t) && bump(t) != t }
/// a tag as pieces: digit runs as numbers (no leading zeros, compared exactly), the text between as text
#[derive(Clone, PartialEq, Debug)]
enum Piece { Num(String), Txt(String) }
fn pieces(t: &str) -> Vec<Piece> {
    let (mut o, mut cur, mut num): (Vec<Piece>, String, bool) = (vec![], String::new(), false);
    let flush = |o: &mut Vec<Piece>, cur: &mut String, num: bool| { if num { o.push(Piece::Num(cur.trim_start_matches('0').to_string())) } else if !cur.is_empty() { o.push(Piece::Txt(cur.clone())) } cur.clear() };
    for c in t.chars() { let d = c.is_ascii_digit(); if d != num { flush(&mut o, &mut cur, num); num = d } cur.push(c) }
    flush(&mut o, &mut cur, num); o
}
fn cmp_piece(a: &Piece, b: &Piece) -> Ordering {
    match (a, b) { (Piece::Num(x), Piece::Num(y)) => x.len().cmp(&y.len()).then_with(|| x.cmp(y)), (Piece::Txt(x), Piece::Txt(y)) => x.cmp(y), (Piece::Num(_), _) => Ordering::Less, _ => Ordering::Greater }
}
fn newer(a: &str, b: &str) -> bool {
    let (x, y) = (pieces(a), pieces(b));
    for (p, q) in x.iter().zip(y.iter()) { match cmp_piece(p, q) { Ordering::Equal => {} o => return o == Ordering::Greater } }
    x.len() > y.len()
}
/// Release.next: the newest bumpable tag bumped, "v1" when there is none
pub fn next(ts: &[&str]) -> String {
    let mut best = "";
    for t in ts { if bumpable(t) && (best.is_empty() || newer(t, best)) { best = t } }
    if best.is_empty() { "v1".into() } else { bump(best) }
}
/// Release.dir
pub fn dir(repo: &str, tag: &str) -> String { format!("{repo}/.backplane/releases/{tag}") }
/// Release.stem: the file name without folder or extension
pub fn stem(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(""); let ps: Vec<&str> = base.split('.').collect();
    if ps.len() >= 2 { ps[..ps.len() - 1].join(".") } else { ps[0].to_string() }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Step { pub id: String, pub label: String, pub argv: Vec<String> }
/// Release.defaults: the ids of the outputs on by default
pub fn defaults() -> Vec<&'static str> { cad::OUTS.iter().filter(|o| o.on).map(|o| o.id).collect() }
/// Release.plan_at: the commands of the chosen outputs, in the tool's order, into `out`; those that need a file not given are left out
pub fn plan_at(ids: &[&str], out: &str, cli: &str, pcb: &str, sch: &str) -> Vec<Step> {
    let name = if pcb.is_empty() { stem(sch) } else { stem(pcb) };
    let vars = [("cli", cli), ("pcb", pcb), ("sch", sch), ("out", out), ("name", name.as_str())];
    cad::OUTS.iter().filter(|o| ids.contains(&o.id) && !(pcb.is_empty() && o.argv.contains(&"{pcb}")) && !(sch.is_empty() && o.argv.contains(&"{sch}")))
        .map(|o| Step { id: o.id.into(), label: o.label.into(), argv: cad::fill(o.argv, &vars) }).collect()
}
/// Release.manifest
pub fn manifest(tag: &str, commit: &str, date: &str, files: &[String]) -> String {
    J::obj(&[("tag", J::str(tag)), ("commit", J::str(commit)), ("date", J::str(date)), ("files", J::Arr(files.iter().map(|f| J::str(f)).collect()))]).show()
}
fn sec(title: &str, body: &str) -> String { let b = bj::trim(body); if b.is_empty() { String::new() } else { format!("\n## {title}\n\n{b}\n") } }
/// Release.notes: the tag, the commit, what it is since the previous tag, the checks' summary and the BOM changes
pub fn notes(tag: &str, prev: &str, commit: &str, bom: &str, checks: &str) -> String {
    let since = if prev.is_empty() { "First release.".to_string() } else { format!("Changes since {prev}.") };
    format!("# {tag}\n\nCommit `{commit}`. {since}\n{}{}", sec("Checks", checks), sec("BOM changes", bom))
}
/// Release.gh
pub fn gh(tag: &str, notes: &str, files: &[String]) -> Vec<String> {
    let mut v: Vec<String> = ["gh", "release", "create", tag, "--title", tag, "--notes-file", notes, "--verify-tag"].iter().map(|s| s.to_string()).collect(); v.extend_from_slice(files); v
}
/// Release.command_error
pub fn command_error(label: &str, code: i32, output: &str) -> String {
    if code == 0 { return String::new() }
    let t = bj::trim(output);
    if t.is_empty() { format!("{label} failed (exit {code})") } else { format!("{label} failed (exit {code}): {}", bj::trim(&bj::take(output, 300))) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn release_next_dotted() { assert_eq!(next(&["v1.1", "v1.2"]), "v1.3") }
    #[test] fn release_next_letter() { assert_eq!(next(&["rev-A"]), "rev-B") }
    #[test] fn release_next_plain() { assert_eq!(next(&["r7"]), "r8") }
    #[test] fn release_next_first() { assert_eq!(next(&[]), "v1") }
    #[test] fn release_tag_valid() { assert!(valid("v1.2")) }
    #[test] fn release_tag_no_space() { assert!(!valid("a b")) }
    #[test] fn release_tag_no_dotdot() { assert!(!valid("a..b")) }
    #[test] fn release_tag_no_dash_lead() { assert!(!valid("-x")) }
    #[test] fn release_tag_no_lock_end() { assert!(!valid("x.lock")) }
    #[test] fn release_tag_not_empty() { assert!(!valid("")) }
    #[test] fn release_dir_in_repo() { assert_eq!(dir("/r", "v1"), "/r/.backplane/releases/v1") }
    #[test] fn release_gh_command() { assert_eq!(gh("v1", "n.md", &["f".to_string()]), ["gh", "release", "create", "v1", "--title", "v1", "--notes-file", "n.md", "--verify-tag", "f"]) }
    #[test] fn tags_as_release_test_says() {
        for t in ["v1.2", "rev-A", "release/2026", "r7", "1", "v1.2-rc.1", "a.b"] { assert!(valid(t), "{t}") }
        for t in ["", "a b", "a..b", "-x", "x.lock", "a~b", "a^b", "a:b", "a?b", "a*b", "a[b", "a\\b", "@", "a@{b", "/x", "x/", "x.", "a//b", ".x", "a/.b", "a\tb", "a\nb"] { assert!(!valid(t), "{t:?}") }
        for (ts, n) in [(&["v1.2", "v1.1"][..], "v1.3"), (&["v1.9", "v1.10", "v1.2"], "v1.11"), (&["v1.9"], "v1.10"), (&["rev-Z"], "rev-AA"), (&["r009"], "r010"), (&["99"], "100"), (&["latest", "v2-rc", "v2"], "v3"), (&["latest"], "v1")] { assert_eq!(next(ts), n, "{ts:?}") }
        assert_eq!(bump("v1.2.9"), "v1.2.10"); assert_eq!(bump("rev-z"), "rev-aa"); assert_eq!(bump("ab"), "ab"); assert_eq!(bump(""), "");
        assert_eq!((stem("/r/a/board.kicad_pcb"), stem("x"), stem("a.b.kicad_sch"), stem("")), ("board".into(), "x".into(), "a.b".into(), "".into()));
        assert_eq!(nocommit("/p/uno"), "No release: a release tags a commit, and /p/uno has none (it is not a git repository yet, or nothing is committed). Make one first: cd /p/uno && git init && git add -A && git commit -m start");
    }
    #[test] fn plans_as_release_test_says() {
        assert_eq!(defaults(), ["gerbers", "drill", "cpl", "bom", "sch-pdf", "pcb-pdf", "step"]);
        let show = |ss: Vec<Step>| ss.iter().map(|s| format!("{}:{}", s.id, s.argv.join("|"))).collect::<Vec<_>>();
        let out = "/r/.backplane/releases/v1";
        let p = show(plan_at(&["gerbers", "bom", "cpl"], out, "kicad-cli", "/r/a.kicad_pcb", "/r/a.kicad_sch"));
        assert_eq!(p[0], "gerbers:kicad-cli|pcb|export|gerbers|-o|/r/.backplane/releases/v1/gerbers/|/r/a.kicad_pcb"); assert!(p[1].starts_with("cpl:") && p[2].starts_with("bom:") && p[2].contains("${QUANTITY}") && p[2].ends_with("-o|/r/.backplane/releases/v1/a-bom.csv|/r/a.kicad_sch"), "{p:?}");
        assert_eq!(show(plan_at(&["gerbers", "bom", "sch-pdf", "step"], out, "kicad-cli", "/r/a.kicad_pcb", "")).iter().map(|s| s.split(':').next().unwrap().to_string()).collect::<Vec<_>>(), ["gerbers", "step"]);
        assert_eq!(show(plan_at(&["cpl", "sch-pdf"], out, "kicad-cli", "", "/r/x.kicad_sch")), ["sch-pdf:kicad-cli|sch|export|pdf|-o|/r/.backplane/releases/v1/x-schematic.pdf|/r/x.kicad_sch"]);
        assert!(plan_at(&[], out, "kicad-cli", "/r/a.kicad_pcb", "").is_empty());
    }
    #[test] fn texts_as_release_test_says() {
        assert_eq!(manifest("v1", "abc123", "2026-10-12T00:00:00Z", &["gerbers/a.gbr".to_string(), "a-bom.csv".to_string()]), "{\"tag\":\"v1\",\"commit\":\"abc123\",\"date\":\"2026-10-12T00:00:00Z\",\"files\":[\"gerbers/a.gbr\",\"a-bom.csv\"]}");
        assert_eq!(notes("v1.3", "v1.2", "abc123", "| Ref | Change |\n|---|---|\n| R1 | 10k -> 22k |", "ERC 0; DRC 0"), "# v1.3\n\nCommit `abc123`. Changes since v1.2.\n\n## Checks\n\nERC 0; DRC 0\n\n## BOM changes\n\n| Ref | Change |\n|---|---|\n| R1 | 10k -> 22k |\n");
        assert_eq!(notes("v1", "", "abc123", "", "  "), "# v1\n\nCommit `abc123`. First release.\n");
        assert_eq!(command_error("x", 0, "out"), ""); assert_eq!(command_error("x", 2, "  "), "x failed (exit 2)"); assert_eq!(command_error("x", 2, " why \n"), "x failed (exit 2): why");
        assert!(object_valid(&"a".repeat(40)) && object_valid(&"0".repeat(64)) && !object_valid("") && !object_valid(&"A".repeat(40)) && !object_valid(&"a".repeat(39)));
    }
}

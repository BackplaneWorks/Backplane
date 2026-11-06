//! core/kexport in Rust: where a KiCad 10 copy of a project goes (<home>/exports/...), what kicad-cli's compatibility report says,
//! which words the person consented to, the flags that consent allows, and which folder may be cleared. Pure. Laws `kexport_*`.
use super::bj::{self, J};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Report { pub lower: Vec<String>, pub drop: Vec<String>, pub block: Vec<String>, pub kept: Vec<String> }
#[derive(Clone, Debug, PartialEq)]
pub struct Paths { pub base: String, pub output: String, pub report: String }

/// KX.hash: FNV over the code points
fn hash(s: &str, h: u32) -> u32 { s.chars().fold(h, |h, c| (h ^ c as u32).wrapping_mul(16777619)) }
/// KX.name: the project's stem
pub fn name(pro: &str) -> String { let f = pro.rsplit('/').next().unwrap_or("project.kicad_pro"); bj::take(f, bj::len(f).saturating_sub(10)) }
/// KX.slug: names in managed paths hold only ASCII letters, numbers, '-' and '_'
pub fn slug(s: &str) -> String { s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect() }
/// KX.paths
pub fn paths(home: &str, pro: &str) -> Paths {
    let base = format!("{home}/exports/{}-{}-{}", slug(&name(pro)), hash(pro, 2166136261), hash(pro, 131));
    Paths { output: format!("{base}/kicad10/"), report: format!("{base}/report.json"), base }
}
/// KX.report: unknown categories need consent
pub fn report(j: &J) -> Report {
    let mut r = Report::default();
    for e in j.get_obj("entries").elems() {
        let text = format!("{}: {} ({})", e.get_str("feature", "Unknown feature"), e.get_str("detail", ""), e.raw("count", "1"));
        match e.get_str("category", "unknown") {
            "lower" => r.lower.push(text), "drop" => r.drop.push(text), "block" => r.block.push(text), "keep" | "kept" => r.kept.push(text),
            cat => r.block.push(format!("Unknown report category {cat}: {text}")),
        }
    }
    r
}
/// KX.valid
pub fn valid(j: &J) -> bool { matches!(j.get("entries"), Some(J::Arr(_))) }
impl Report {
    /// KX.block, KX.empty
    pub fn blocked(&self) -> bool { !self.block.is_empty() }
    pub fn empty(&self) -> bool { self.lower.is_empty() && self.drop.is_empty() && self.block.is_empty() && self.kept.is_empty() }
}
/// KX.flags: nothing without consent
pub fn flags(r: &Report, anyway: bool) -> Vec<&'static str> { if !anyway { vec![] } else if r.blocked() { vec!["--force", "--drop-approximations"] } else { vec!["--force"] } }
/// KX.safe: the lexical guard; the IO boundary also checks physical ancestry and refuses links
pub fn safe(base: &str) -> bool {
    base.starts_with('/') && base.contains("/exports/") && !base.contains("/../") && !base.contains("/./") && !base.contains("//") && !base.ends_with('/') && !base.ends_with("/..")
}
/// KX.may_clear
pub fn may_clear(base: &str, marker: bool) -> bool { marker && safe(base) }
/// KX.within: the base is one managed folder under this home
pub fn within(home: &str, base: &str) -> bool {
    let prefix = format!("{home}/exports/");
    let leaf = bj::drop(base, bj::len(&prefix));
    base.starts_with(&prefix) && !leaf.is_empty() && leaf == slug(&leaf)
}
/// KX.clear.allowed
pub fn clear_allowed(inside: bool, base: &str, marker: bool) -> bool { inside && may_clear(base, marker) }
/// KX.may_clear_in
pub fn may_clear_in(home: &str, base: &str, marker: bool) -> bool { clear_allowed(within(home, base), base, marker) }
/// KX.lines
pub fn lines(r: &Report) -> Vec<String> {
    let mut o = vec![];
    for (label, xs) in [("Changed for KiCad 10", &r.lower), ("Removed", &r.drop), ("Can't be kept", &r.block), ("Kept beside the copy", &r.kept)] {
        if !xs.is_empty() { o.push(label.to_string()); o.extend(xs.iter().cloned()) }
    }
    o
}
/// KX.signature: consent follows the relevant report entries, ignoring future metadata
pub fn signature(j: &J) -> String { J::Arr(lines(&report(j)).into_iter().map(J::Str).collect()).show() }
/// KX.success
pub fn success(path: &str) -> String { format!("Exported a KiCad 10 copy to {path}. Changes made there don't come back into this project.") }

#[cfg(test)]
mod tests {
    use super::*;
    fn obj(s: &str) -> J { bj::parse(s).unwrap_or(J::Null) }
    fn sample() -> J { obj(r#"{"entries":[{"category":"lower","feature":"Text","detail":"Converted","count":2},{"category":"drop","feature":"Data","detail":"Removed","count":1},{"category":"keep","feature":"Companion","detail":"board.backplane.json","count":1}],"future":true}"#) }
    fn blocked() -> Report { report(&obj(r#"{"entries":[{"category":"block","feature":"Line endings","detail":"Not representable","count":1}]}"#)) }
    fn rep(l: &[&str], d: &[&str], b: &[&str]) -> Report { let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect(); Report { lower: v(l), drop: v(d), block: v(b), kept: vec![] } }
    const BASES: [&str; 9] = ["", "/", "/tmp/design", "/tmp/bp/exports/x", "/tmp/bp/exports/../design", "a", "/exports/", "/a/exports/x/", "/a/exports/./x"];
    #[test] fn kexport_no_unmarked_clear() { for b in BASES { assert!(!may_clear(b, false), "{b}") } }
    #[test] fn kexport_no_unconsented_force() { for r in [Report::default(), blocked(), report(&sample()), rep(&["a"], &["b"], &["c"])] { assert!(flags(&r, false).is_empty()) } }
    #[test] fn kexport_block_force() { assert_eq!(flags(&rep(&[], &[], &["blocked"]), true), ["--force", "--drop-approximations"]) }
    #[test] fn kexport_lossy_force() { assert_eq!(flags(&rep(&["changed"], &["removed"], &[]), true), ["--force"]) }
    #[test] fn kexport_no_outside_clear() { assert!(!may_clear("/tmp/design", true)) }
    #[test] fn kexport_no_foreign_base() { for b in BASES { for m in [true, false] { assert!(!clear_allowed(false, b, m), "{b} {m}") } } }
    #[test] fn as_kexport_test_says() {
        assert_eq!(name("/design/My Board.kicad_pro"), "My Board");
        let p = paths("/tmp/bp", "/design/My Board.kicad_pro");
        assert!(p.base.starts_with("/tmp/bp/exports/My-Board-")); assert_eq!(p.output, format!("{}/kicad10/", p.base)); assert_eq!(p.report, format!("{}/report.json", p.base));
        assert_ne!(paths("/tmp/bp", "/a/x.kicad_pro").base, paths("/tmp/bp", "/b/x.kicad_pro").base);
        assert!(report(&obj(r#"{"entries":[]}"#)).empty()); assert!(!valid(&obj(r#"{"future":[]}"#))); assert!(valid(&sample()));
        assert_eq!(lines(&report(&sample())).join("|"), "Changed for KiCad 10|Text: Converted (2)|Removed|Data: Removed (1)|Kept beside the copy|Companion: board.backplane.json (1)");
        assert_eq!(signature(&sample()), signature(&obj(r#"{"entries":[{"category":"lower","feature":"Text","detail":"Converted","count":2},{"category":"drop","feature":"Data","detail":"Removed","count":1},{"category":"keep","feature":"Companion","detail":"board.backplane.json","count":1}],"timestamp":99}"#)));
        assert!(report(&obj(r#"{"entries":[{"category":"future"}]}"#)).blocked()); assert!(blocked().blocked());
        assert!(flags(&blocked(), false).is_empty()); assert_eq!(flags(&report(&sample()), true), ["--force"]); assert_eq!(flags(&blocked(), true), ["--force", "--drop-approximations"]);
        assert!(!may_clear(&p.base, false)); assert!(may_clear(&p.base, true)); assert!(!may_clear("/tmp/design", true)); assert!(!may_clear("/tmp/bp/exports/../design", true));
        assert!(!may_clear_in("/tmp/home", "/elsewhere/exports/x", true)); assert!(!may_clear_in("/tmp/home", "/tmp/home/exports/x/y", true)); assert!(!may_clear("/", true));
        assert!(may_clear_in("/tmp/home", "/tmp/home/exports/My-Board-1-2", true));
        assert_eq!(success("/copy/"), "Exported a KiCad 10 copy to /copy/. Changes made there don't come back into this project.");
    }
    #[test] fn fnv_multiplies_in_32_bits() { assert_eq!(hash("", 7), 7); assert_eq!(hash("a", 2166136261), 0xe40c292c); assert_eq!(hash("é", 131), (131u32 ^ 233).wrapping_mul(16777619)) }
}

//! The check that a screen string does not bypass the catalog: every English-looking string literal in the client's source, outside tests, must
//! be drawn through the catalog (`tr`, `trs`, `t`, `tf`, or `keys::tip`, which translates its text), or be something that is not words for a person (an id, a comparison, a log line, a key
//! sent to the hub), or say why on its line with `// i18n-ok: <reason>`. `PENDING` is the list of files the move to the catalog has not reached yet;
//! it only shrinks, and a file leaves it when this test passes for it.
use std::path::{Path, PathBuf};

/// Files not yet moved to the catalog. Each lane removes its own as it finishes them.
const PENDING: &[&str] = &[
];

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() { let p = e.path(); if p.is_dir() { files(&p, out) } else if p.extension().map_or(false, |x| x == "rs") { out.push(p) } }
}
/// A literal that reads as words for a person: letters, and a capital or a space in it; not an identifier, path or key.
fn texty(s: &str) -> bool {
    /// Names, not words: the product, fonts, folders.
    const NAMES: &[&str] = &["Backplane", "Downloads", "Noto Sans Mono", "Noto Sans Mono CJK JP", "JetBrains Mono", "Fira Code", "IBM Plex Mono", "Source Code Pro", "Noto Sans", "Noto Sans CJK JP"];
    if NAMES.contains(&s) { return false }
    s.chars().filter(|c| c.is_alphabetic()).count() >= 3 && (s.chars().next().map_or(false, char::is_uppercase) || s.contains(' '))
        && !s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.:-/#".contains(c))
}
/// Whether the literal at `start..end` of `line` is not a screen string (or is drawn through the catalog).
fn fine(line: &str, start: usize, end: usize) -> bool {
    let before = line[..start].trim_end(); let after = line.get(end + 1..).unwrap_or("").trim_start();
    if ["tr(", "trs(", "t(", "tf(", "tr(&format!(", "trs(&format!(", "trs(&", "tr(&"].iter().any(|t| before.ends_with(t)) { return true }
    if ["keys::tip(", "==", "!=", "contains(", "starts_with(", "ends_with(", ".id(", "eprintln!(", "println!(", "write!(f,", "assert!(", "assert_eq!(", "panic!(", "expect(", "var_os(", "var(", "debug_assert!(", "log::info!(", "log::warn!(", "log::error!(", "log::debug!(", "unreachable!(", "unimplemented!(", "strip_prefix(", "strip_suffix(", "split(", "find(", "rsplit(", "split_once(", "trim_start_matches(", "trim_end_matches(", "replace(", "SharedString::from(", "Some(", "Composer::with_context(cx,"].iter().any(|t| before.ends_with(t)) { return true }
    if after.starts_with("=>") || after.starts_with("| \"") { return true }
    if line.contains("GPUI_LOG") || line.contains("i18n-ok") { return true }
    false
}
/// The screen strings in `src` that bypass the catalog: (line, literal).
pub fn bypasses(src: &str) -> Vec<(usize, String)> {
    let body = src.split("#[cfg(test)]\nmod ").next().unwrap_or(src);
    let mut out = Vec::new(); let mut in_doc = false;
    for (n, line) in body.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("//") { continue }
        if t.starts_with("/*") { in_doc = true } if in_doc { if t.contains("*/") { in_doc = false } continue }
        let b = line.as_bytes(); let mut i = 0;
        while i < b.len() {
            if b[i] == b'/' && b.get(i + 1) == Some(&b'/') { break } // a trailing comment
            if b[i] == b'\'' && b.get(i + 2) == Some(&b'\'') { i += 3; continue } // a char literal
            if b[i] == b'\'' && b.get(i + 1) == Some(&b'\\') { while i < b.len() && b[i] != b'\'' || i < 2 { i += 1; if b.get(i) == Some(&b'\'') && i > 0 && b[i - 1] != b'\\' { break } } i += 1; continue }
            if b[i] == b'"' {
                let start = i + 1; let mut j = start;
                while j < b.len() && b[j] != b'"' { if b[j] == b'\\' { j += 1 } j += 1 }
                if j >= b.len() { break } // a string that goes on over the next lines
                let lit = &line[start..j];
                if texty(lit) && !fine(line, start - 1, j) { out.push((n + 1, lit.to_string())); }
                i = j + 1; continue
            }
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_screen_string_outside_the_catalog_is_found_and_a_drawn_one_is_not() {
        let src = "fn a() { x.child(\"Open the file\"); y.child(trs(\"Open the file\")); if k == \"Settings page\" {} z.child(format!(\"{} done\", n)); w.child(\"ok to go\"); // \"Not me here\"\n let t = \"See all\"; // i18n-ok: an id\n}\n#[cfg(test)]\nmod t { fn x() { \"Hello there\"; } }";
        assert_eq!(bypasses(src), vec![(1, "Open the file".to_string()), (1, "{} done".to_string()), (1, "ok to go".to_string())]);
    }
    /// Every file the move has reached stays moved: no screen string in it bypasses the catalog.
    #[test] fn no_moved_file_draws_a_string_outside_the_catalog() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src"); let mut all = Vec::new(); files(&root, &mut all); all.sort();
        let mut bad = Vec::new();
        for p in all {
            let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
            if PENDING.contains(&rel.as_str()) || rel == "i18n_scan.rs" || rel == "i18n.rs" { continue }
            for (n, lit) in bypasses(&std::fs::read_to_string(&p).unwrap()) { bad.push(format!("{rel}:{n}: \"{lit}\"")); }
        }
        assert!(bad.is_empty(), "screen strings that bypass the catalog (wrap in tr()/tf(), or `// i18n-ok: why`):\n{}", bad.join("\n"));
    }
    /// What the source asks of the catalog exists in it: every `tr("English")` is a message (or a template), every `t("key")`/`tf("key", ..)` a key, and
    /// each has its Japanese.
    #[test] fn every_catalog_word_the_source_asks_for_is_in_the_catalog() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src"); let mut all = Vec::new(); files(&root, &mut all); all.sort();
        let c = crate::i18n::catalog(); let mut bad = Vec::new();
        for p in all {
            let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"); if rel == "i18n_scan.rs" || rel == "i18n.rs" { continue }
            let src = std::fs::read_to_string(&p).unwrap(); let body = src.split("#[cfg(test)]\nmod ").next().unwrap_or(&src);
            for (n, line) in body.lines().enumerate() {
                if line.trim_start().starts_with("//") { continue }
                for (open, key) in [("tr(\"", false), ("trs(\"", false), ("keys::tip(\"", false), ("t(\"", true), ("tf(\"", true)] {
                    let mut at = 0;
                    while let Some(i) = line[at..].find(open) {
                        let i = at + i; at = i + open.len();
                        if i > 0 && (line.as_bytes()[i - 1].is_ascii_alphanumeric() || line.as_bytes()[i - 1] == b'_') { continue }
                        let Some(j) = line[at..].find('"') else { continue }; let lit = &line[at..at + j];
                        if key { if !c.has(lit, false) || !c.has(lit, true) { bad.push(format!("{rel}:{}: key {lit}", n + 1)); } }
                        else if c.japanese(lit).is_none() { bad.push(format!("{rel}:{}: \"{lit}\" has no Japanese in the catalog", n + 1)); }
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
    #[test] fn pending_names_real_files() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for f in PENDING { assert!(root.join(f).is_file(), "{f} is not a file: drop it from PENDING"); }
    }
}

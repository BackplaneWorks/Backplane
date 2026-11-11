//! What each checkpoint's fold is made of. `build.rs` hashes these files into `ckpt::stamps`, and a checkpoint is trusted only by a build whose fold hashes the same (docs/STARTUP.md).
//!
//! The stamp must cover everything the fold's output depends on: a missed file is a stale checkpoint trusted again, which loses state silently, while a file too many costs one refold
//! of seconds. So when in doubt a file goes in. The test below walks every input and fails on any reach to a module that is neither an input nor written down in `NOT_INPUT` with why.
//! Nobody has to remember this: add a helper to a fold and the test says so.
#![allow(dead_code)]

const STATE: &[&str] = &["state/mod.rs", "state/autosettle.rs", "state/ext.rs", "state/fam.rs", "state/host.rs", "state/json.rs", "state/liveness.rs", "state/policy.rs", "state/star.rs",
    "state/summary.rs", "state/todo.rs", "state/turnend.rs", "state/uber.rs", "state/watchdog.rs", "shelf.rs", "signal.rs"];

/// Checkpoint name, then the files (from `src/`) its fold reads. `extra` lists are joined with `STATE` by `inputs`.
pub const FOLDS: &[(&str, &[&str], bool)] = &[
    ("MODEL", &["model.rs"], false),
    ("STATE", &[], true),
    ("COMMANDS", &["commands/fold.rs", "session/event_type.rs"], false),
    ("AGENTS", &["agents/world.rs", "agents/lines.rs", "agents/bg.rs", "session/event_type.rs"], false),
    ("BOTS", &["bots/world.rs", "bots/gate.rs", "bots/mail/mod.rs", "bots/mail/mailbox.rs", "session/event_type.rs"], true),
    ("WAKES", &["agents/wake.rs", "session/event_type.rs"], false),
    ("FEED", &["net/feed.rs", "net/far.rs"], false),
    ("PEERS", &["net/peers.rs", "net/machines.rs"], false),
];
pub fn inputs(name: &str) -> Vec<&'static str> {
    let (_, files, state) = FOLDS.iter().find(|f| f.0 == name).expect("a fold");
    let mut v: Vec<&'static str> = files.to_vec(); if *state { v.extend(STATE.iter().copied()); } v.sort(); v.dedup(); v
}

/// Reaches that shape no fold's output, whichever input makes them: the checkpointing itself, the log's line reader, timing, the copy switch, handler traits.
pub const INFRA: &[(&str, &str)] = &[
    ("crate::ckpt", "the checkpoint file format; it has its own VERSION"),
    ("crate::store", "hands the fold whole log lines; it decides nothing about them"),
    ("crate::startup", "timing lines on stderr"),
    ("crate::copy", "a process-wide switch, not a fold rule"),
    ("crate::api", "handler traits and types; no fold logic"),
];
/// One input's reach that is not part of its fold: (input, reach, why). Each is a decision somebody made, so a new reach stops and asks.
pub const NOT_INPUT: &[(&str, &str, &str)] = &[
    ("commands/fold.rs", "crate::settings::fallback::hold_key", "the key prefix a query (holds_of) reads by, when asked; the fold stores each logged key as it came"),
    ("state/host.rs", "crate::guard", "guard::Folding marks a fold in progress for /healthz; it reads nothing and decides nothing"),
    ("agents/wake.rs", "crate::guard::safe", "safe mode holds a wake back as it fires (Book::fire); Book::apply reads only the log line"),
    ("agents/wake.rs", "crate::plat::fs::is_abs", "checks a wake tool's file target when it is asked for (error()); Book::apply reads only the log line"),
    ("agents/wake.rs", "super::launch", "the text of a wake as it fires; Book::apply reads only the log line"),
    ("agents/wake.rs", "super::lines", "the id of a new wake; Book::apply reads only the log line"),
    ("agents/wake.rs", "super::mcp", "the wake tools' definitions; Book::apply reads only the log line"),
    ("agents/wake.rs", "super::now_s", "the clock, at a tool call; Book::apply reads only the log line"),
    ("agents/world.rs", "crate::bots::relocate", "cwd(), at read time; nothing from it is folded or saved"),
    ("agents/world.rs", "crate::bots::rpc", "cwd(), at read time; nothing from it is folded or saved"),
    ("bots/mail/mod.rs", "mod", "declares the mailroom's parts; the fold reads this file's types and mailbox.rs, both inputs"),
    ("net/feed.rs", "crate::hub", "the host and the push after a fold; Index::fold takes the event and far.rs"),
    ("net/feed.rs", "super::farstore", "a request's epoch; Index::fold takes the event and far.rs"),
    ("net/feed.rs", "super::outbox", "the push queue's type; Index::fold takes the event and far.rs"),
    ("net/feed.rs", "super::sign", "signing a reply; Index::fold takes the event and far.rs"),
    ("net/peers.rs", "crate::hub", "the host; Peers::fold takes the event"),
    ("state/host.rs", "crate::hub", "the host; the state folds log lines"),
    ("state/host.rs", "crate::agents::summarizer", "a ticker's job at run time; the state folds log lines"),
];

/// A file's code: everything before its test module, with blank lines and whole-line comments dropped and indentation ignored. Editing a test or a comment refolds nothing.
pub fn code(s: &str) -> String {
    let mut out = String::new(); let mut lines = s.lines().peekable();
    while let Some(l) = lines.next() {
        let t = l.trim();
        if t.starts_with("#[cfg(test)]") {
            let after = t["#[cfg(test)]".len()..].trim();
            let next = if after.is_empty() { lines.peek().map_or("", |n| n.trim()) } else { after };
            if next.starts_with("mod ") || next.starts_with("pub mod ") { break } // only the test module ends the code: an early `#[cfg(test)] use` must not cut a fold out of its own stamp
        }
        if t.is_empty() || t.starts_with("//") { continue }
        out.push_str(t); out.push('\n');
    }
    out
}
pub fn stamp<'a>(texts: impl Iterator<Item = &'a str>) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for t in texts { for b in code(t).bytes().chain([0xff]) { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); } }
    h | 1 // never 0, which is what a checkpoint from before stamps reads as
}

/// The paths a file's code reaches: `crate::a::b`, `super::a`, `use super::{a, b::c}`, and `mod a;`.
pub fn reaches(src: &str) -> Vec<String> {
    let c = code(src); let mut out = vec![];
    let word = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_';
    for root in ["crate::", "super::"] {
        let mut at = 0;
        while let Some(i) = c[at..].find(root) {
            let s = at + i; at = s + root.len();
            if s > 0 && (word(c.as_bytes()[s - 1]) || c.as_bytes()[s - 1] == b':') { continue } // `x::super::`, `mycrate::`
            let rest = &c[s + root.len()..];
            if let Some(g) = rest.strip_prefix('{') { // a group: every member, nested groups as their head
                let (mut depth, mut item, mut items) = (0, String::new(), vec![]);
                for ch in g.chars() { match ch { '{' => { depth += 1; item.push(ch) } '}' if depth == 0 => break, '}' => { depth -= 1; item.push(ch) } ',' if depth == 0 => items.push(std::mem::take(&mut item)), _ => item.push(ch) } }
                items.push(item);
                for it in items { let p = path_of(it.trim()); if !p.is_empty() { out.push(format!("{root}{p}")) } }
            } else { let p = path_of(rest); if !p.is_empty() { out.push(format!("{root}{p}")) } else if rest.starts_with('*') { out.push(format!("{root}*")) } }
        }
    }
    for l in c.lines() { let t = l.strip_prefix("pub ").unwrap_or(l); if let Some(n) = t.strip_prefix("mod ") { if let Some(n) = n.strip_suffix(';') { out.push(format!("mod::{n}")) } } }
    out.sort(); out.dedup(); out
}
/// The module part of a path: lower-case segments, up to the first type or anything else.
fn path_of(s: &str) -> String {
    let mut p: Vec<&str> = vec![];
    for seg in s.split("::") {
        let end = seg.find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')).unwrap_or(seg.len());
        if end == 0 { break } p.push(&seg[..end]); if end != seg.len() { break }
    }
    p.join("::")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    fn src() -> PathBuf { PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("run by cargo")).join("src") }
    /// The source file a reach lands in, longest match first.
    fn resolve(root: &Path, from: &str, reach: &str) -> Option<String> {
        let dir = Path::new(from).parent().unwrap_or(Path::new(""));
        let in_mod = from.ends_with("/mod.rs");
        let (base, rest): (PathBuf, &str) = if let Some(r) = reach.strip_prefix("crate::") { (PathBuf::new(), r) }
            else if let Some(r) = reach.strip_prefix("super::") { (if in_mod { dir.parent().unwrap_or(Path::new("")).to_path_buf() } else { dir.to_path_buf() }, r) }
            else if let Some(r) = reach.strip_prefix("mod::") { (if in_mod { dir.to_path_buf() } else { dir.join(Path::new(from).file_stem().unwrap()) }, r) } else { return None };
        let segs: Vec<&str> = rest.split("::").collect();
        if reach == "crate::statehost" || reach.starts_with("crate::statehost::") { return Some("state/host.rs".into()) } // `#[path]` in main.rs
        for n in (1..=segs.len()).rev() {
            let p = base.join(segs[..n].join("/"));
            for c in [p.with_extension("rs"), p.join("mod.rs")] { if root.join(&c).is_file() { return Some(c.to_string_lossy().replace('\\', "/")) } }
        }
        // no file by that name: an item of the module itself (`super::head` in bots/mail/mailbox.rs is bots/mail/mod.rs)
        for c in [base.join("mod.rs"), base.with_extension("rs"), PathBuf::from("main.rs")] { if root.join(&c).is_file() && c != Path::new(".rs") { return Some(c.to_string_lossy().replace('\\', "/")) } }
        None
    }
    #[test] fn every_fold_reach_is_an_input_or_written_down() {
        let root = src(); let mut bad = vec![];
        for (name, _, _) in FOLDS {
            let files = inputs(name);
            for f in &files {
                let text = std::fs::read_to_string(root.join(f)).unwrap_or_else(|_| panic!("{name}: {f} does not exist"));
                for r in reaches(&text) {
                    if INFRA.iter().any(|(p, _)| r == *p || r.starts_with(&format!("{p}::"))) { continue }
                    if NOT_INPUT.iter().any(|(i, p, _)| i == f && (r == *p || r.starts_with(&format!("{p}::")))) { continue }
                    match resolve(&root, f, &r) { Some(t) if files.contains(&t.as_str()) => {} Some(t) => bad.push(format!("{name}: {f} reaches {r} ({t})")), None if r.ends_with('*') => {} None => bad.push(format!("{name}: {f} reaches {r} (no such file)")) }
                }
            }
        }
        bad.sort(); bad.dedup();
        assert!(bad.is_empty(), "a fold's stamp must cover what it reads: add each to the fold's inputs, or to NOT_INPUT with why:\n{}", bad.join("\n"));
    }
    #[test] fn every_written_exception_still_exists() {
        let root = src();
        for (f, p, why) in NOT_INPUT {
            assert!(!why.is_empty(), "{f} {p}");
            let r = reaches(&std::fs::read_to_string(root.join(f)).unwrap());
            assert!(r.iter().any(|x| x == p || x.starts_with(&format!("{p}::"))), "{f} no longer reaches {p}: remove it from NOT_INPUT");
        }
    }
    #[test] fn a_new_file_in_the_state_folder_must_be_listed() {
        let mut on_disk: Vec<String> = std::fs::read_dir(src().join("state")).unwrap().map(|e| format!("state/{}", e.unwrap().file_name().to_string_lossy())).filter(|n| n.ends_with(".rs")).collect();
        on_disk.sort(); for f in on_disk { assert!(STATE.contains(&f.as_str()), "{f} is not in STATE"); }
    }
    #[test] fn the_scanner_finds_every_way_of_reaching_a_module() {
        let r = reaches("use crate::a::b::{C, d};\nuse super::{e, f::g, H};\nlet x = super::i::j(crate::k::L::new());\npub mod m;\nmod n;\nuse super::*;\nmod inline { }\n");
        for want in ["crate::a::b", "super::e", "super::f::g", "super::i::j", "crate::k", "mod::m", "mod::n", "super::*"] { assert!(r.contains(&want.to_string()), "{want} in {r:?}"); }
        assert!(!r.iter().any(|x| x.contains("inline")));
    }
    #[test] fn the_scanner_ignores_test_code_and_comments() {
        assert_eq!(reaches("// crate::a\nfn x() {}\n#[cfg(test)]\nmod tests { use crate::b; }\n"), Vec::<String>::new());
    }
    #[test] fn a_change_to_code_moves_the_stamp_and_a_comment_or_test_does_not() {
        let s = |t: &str| stamp([t].into_iter());
        let base = "fn f() { 1 }\n#[cfg(test)]\nmod tests { fn t() {} }\n";
        assert_ne!(s(base), s("fn f() { 2 }\n#[cfg(test)]\nmod tests { fn t() {} }\n"), "the fold changed");
        assert_eq!(s(base), s("// a note\n\n   fn f() { 1 }\n#[cfg(test)]\nmod tests { fn t() { assert!(true) } }\n"), "only a comment, spacing and a test");
        assert_eq!(s(base), s("fn f() { 1 }\n#[cfg(test)] mod tests { }\n"), "the attribute on the same line");
    }
    #[test] fn an_early_test_only_use_does_not_cut_a_fold_out_of_its_stamp() {
        let s = |t: &str| stamp([t].into_iter());
        assert_ne!(s("#[cfg(test)]\nuse x::Y;\nfn f() { 1 }\n"), s("#[cfg(test)]\nuse x::Y;\nfn f() { 2 }\n"));
        assert_ne!(s("#[cfg(test)] use x::Y;\nfn f() { 1 }\n"), s("#[cfg(test)] use x::Y;\nfn f() { 2 }\n"));
    }
    #[test] fn order_and_boundaries_count_and_a_stamp_is_never_zero() {
        assert_ne!(stamp(["ab", "c"].into_iter()), stamp(["a", "bc"].into_iter())); assert_ne!(stamp(["a", "b"].into_iter()), stamp(["b", "a"].into_iter())); assert_ne!(stamp([""].into_iter()), 0);
    }
}

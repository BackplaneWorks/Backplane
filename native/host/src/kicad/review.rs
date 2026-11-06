//! core/review in Rust: comments pinned to a place in a design, as Acts of tone "review": the op's name in the Act's kind, the op as
//! JSON in its summary. The summary is stored in the log, so its text is to the byte: keys in that order, F32.show for x and y
//! (fl::show), Json.esc for strings. Folding the ops oldest first gives the comments. Pure. Laws `review_*`.
use super::bj::{self, J};
use super::fl;

#[derive(Clone, Debug, PartialEq)]
pub struct Anchor { pub file: String, pub commit: String, pub kind: String, pub layer: String, pub x: f32, pub y: f32, pub item: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Comment { pub id: String, pub anchor: Anchor, pub text: String, pub by: String, pub at: u64, pub parent: String, pub resolved: bool }
#[derive(Clone, Debug, PartialEq)]
pub enum Op { Add(Comment), Resolve(String, String), Reopen(String, String), Edit(String, String), Drop(String) }
#[derive(Clone, Debug, PartialEq)]
pub struct Pin { pub id: String, pub x: f32, pub y: f32, pub layer: String, pub item: String, pub resolved: bool, pub count: usize }
pub const TONE: &str = "review";

/// Review.new_id
pub fn new_id(th: &str, n: u64) -> String { format!("rv:{th}:{n}") }

// Encoding
fn anchor_json(a: &Anchor) -> J {
    J::obj(&[("file", J::str(&a.file)), ("commit", J::str(&a.commit)), ("kind", J::str(&a.kind)), ("layer", J::str(&a.layer)), ("x", J::Num(fl::show(a.x))), ("y", J::Num(fl::show(a.y))), ("item", J::str(&a.item))])
}
/// Review.op.add
pub fn op_add(id: &str, a: &Anchor, text: &str, by: &str, parent: &str) -> String { J::obj(&[("id", J::str(id)), ("a", anchor_json(a)), ("text", J::str(text)), ("by", J::str(by)), ("parent", J::str(parent))]).show() }
/// Review.op.resolve, Review.op.reopen
pub fn op_resolve(id: &str, by: &str) -> String { J::obj(&[("id", J::str(id)), ("by", J::str(by))]).show() }
pub fn op_reopen(id: &str, by: &str) -> String { op_resolve(id, by) }
pub fn op_edit(id: &str, text: &str) -> String { J::obj(&[("id", J::str(id)), ("text", J::str(text))]).show() }
pub fn op_drop(id: &str) -> String { J::obj(&[("id", J::str(id))]).show() }

// Decoding
/// Rv.f32
fn f32_of(o: &J, key: &str) -> f32 { fl::read(o.raw(key, "")).unwrap_or(0.0) }
/// Rv.anchor.read
pub fn anchor_read(o: &J) -> Anchor {
    Anchor { file: o.get_str("file", "").into(), commit: o.get_str("commit", "").into(), kind: o.get_str("kind", "").into(), layer: o.get_str("layer", "").into(), x: f32_of(o, "x"), y: f32_of(o, "y"), item: o.get_str("item", "").into() }
}
/// Review.op.of: an op from its kind and JSON summary
pub fn op_of(kind: &str, summary: &str, at: u64) -> Option<Op> {
    let o = bj::parse(summary)?;
    let g = |k: &str| o.get_str(k, "").to_string();
    match kind {
        "add" => Some(Op::Add(Comment { id: g("id"), anchor: anchor_read(o.get_obj("a")), text: g("text"), by: g("by"), at, parent: g("parent"), resolved: false })),
        "resolve" => Some(Op::Resolve(g("id"), g("by"))), "reopen" => Some(Op::Reopen(g("id"), g("by"))), "edit" => Some(Op::Edit(g("id"), g("text"))), "drop" => Some(Op::Drop(g("id"))),
        _ => None,
    }
}
/// Review.of: a review Act read back (None for any other tone)
pub fn of(tone: &str, kind: &str, summary: &str, at: u64) -> Option<Op> { if tone == TONE { op_of(kind, summary, at) } else { None } }

// Folding
impl Comment { fn root(&self) -> bool { self.parent.is_empty() } }
fn step(cs: &mut Vec<Comment>, o: &Op) {
    match o {
        Op::Add(c) => {
            let parent_ok = c.parent.is_empty() || cs.iter().any(|x| x.id == c.parent && x.root());
            if !c.id.is_empty() && !cs.iter().any(|x| x.id == c.id) && parent_ok { cs.push(c.clone()) }
        }
        Op::Resolve(id, _) => cs.iter_mut().filter(|c| c.id == *id).for_each(|c| c.resolved = true),
        Op::Reopen(id, _) => cs.iter_mut().filter(|c| c.id == *id).for_each(|c| c.resolved = false),
        Op::Edit(id, t) => cs.iter_mut().filter(|c| c.id == *id).for_each(|c| c.text = t.clone()),
        Op::Drop(id) => cs.retain(|c| c.id != *id && c.parent != *id),
    }
}
/// Review.fold: ops oldest first; the comments in the order they were made
pub fn fold(ops: &[Op]) -> Vec<Comment> { let mut cs = vec![]; for o in ops { step(&mut cs, o) } cs }

// Queries
pub fn roots(cs: &[Comment]) -> Vec<&Comment> { cs.iter().filter(|c| c.root()).collect() }
pub fn open(cs: &[Comment]) -> Vec<&Comment> { cs.iter().filter(|c| c.root() && !c.resolved).collect() }
pub fn resolved(cs: &[Comment]) -> Vec<&Comment> { cs.iter().filter(|c| c.root() && c.resolved).collect() }
pub fn count_open(cs: &[Comment]) -> usize { open(cs).len() }
pub fn replies<'a>(cs: &'a [Comment], id: &str) -> Vec<&'a Comment> { cs.iter().filter(|c| c.parent == id).collect() }
/// Rv.pin
pub fn pin(all: &[Comment], c: &Comment) -> Pin { Pin { id: c.id.clone(), x: c.anchor.x, y: c.anchor.y, layer: c.anchor.layer.clone(), item: c.anchor.item.clone(), resolved: c.resolved, count: replies(all, &c.id).len() } }
/// Review.pins: the roots on `rel`, for one layer ("" for all; a pin with no layer shows on every layer)
pub fn pins(cs: &[Comment], rel: &str, layer: &str) -> Vec<Pin> {
    cs.iter().filter(|c| c.root() && c.anchor.file == rel && (layer.is_empty() || c.anchor.layer.is_empty() || c.anchor.layer == layer)).map(|c| pin(cs, c)).collect()
}
/// Review.moved: the item under the pin is among the keys Hist.mark marked
pub fn moved(c: &Comment, changed: &[&str]) -> bool { !c.anchor.item.is_empty() && changed.contains(&c.anchor.item.as_str()) }

// Text
/// Rv.mm
fn mm(v: f32) -> String { fl::show(fl::round(v * 100.0) / 100.0) }
/// Rv.where
fn place(c: &Comment) -> String {
    let a = &c.anchor;
    format!("({}, {} mm{})", mm(a.x), mm(a.y), if a.layer.is_empty() { String::new() } else { format!(", {}", a.layer) })
}
fn by_note(c: &Comment) -> String { if c.by.is_empty() { String::new() } else { format!(" _({})_", c.by) } }
fn rep_lines(rs: &[&Comment], pre: &str) -> String { rs.iter().map(|r| format!("{pre}{}{}\n", r.text, by_note(r))).collect() }
/// Review.prompt: the open roots as a block for the agent's prompt, "" when none
pub fn prompt(cs: &[Comment]) -> String {
    let os = open(cs);
    if os.is_empty() { return String::new() }
    let body: String = os.iter().map(|c| format!("- {} {} {}: {}{}\n{}", c.id, c.anchor.file, place(c), c.text, by_note(c), rep_lines(&replies(cs, &c.id), "    reply: "))).collect();
    format!("<review_comments>\nOpen comments on the design (id, file, place, text). Answer or fix them, then reply or resolve with review_comment.\n{body}</review_comments>")
}
/// Rv.md.part: grouped by file in order of first appearance
fn md_part(all: &[Comment], rs: &[&Comment]) -> String {
    let mut seen: Vec<&str> = vec![];
    for c in rs { if !seen.contains(&c.anchor.file.as_str()) { seen.push(&c.anchor.file) } }
    let mut o = String::new();
    for f in seen { for c in rs.iter().filter(|c| c.root() && c.anchor.file == f) { o.push_str(&format!("- **{}** {} — {}{}\n{}", c.anchor.file, place(c), c.text, by_note(c), rep_lines(&replies(all, &c.id), "  - "))) } }
    o
}
/// Review.md: the open comments, then "Resolved" and the rest
pub fn md(cs: &[Comment]) -> String {
    let (os, rs) = (open(cs), resolved(cs));
    let tail = if rs.is_empty() { String::new() } else { format!("\nResolved\n\n{}", md_part(cs, &rs)) };
    md_part(cs, &os) + &tail
}
/// Review.issue: a comment's issue title (first line, cut at 72 characters) and body
pub fn issue(all: &[Comment], c: &Comment) -> (String, String) {
    let line = c.text.split('\n').next().unwrap_or("");
    let title = if bj::len(line) > 72 { format!("{}...", bj::take(line, 72)) } else { line.to_string() };
    let commit = if c.anchor.commit.is_empty() { String::new() } else { format!(" @ {}", c.anchor.commit) };
    (title, format!("{}\n\n{} {}{}\n{}", c.text, c.anchor.file, place(c), commit, rep_lines(&replies(all, &c.id), "- ")))
}

// Requests
/// Review.op.summary: what a request records
pub fn op_summary(op: &str, aid: &str, p: &J, by: &str) -> String {
    let (id, text) = (p.get_str("id", ""), p.get_str("text", ""));
    match op {
        "add" => op_add(aid, &anchor_read(p), text, by, p.get_str("parent", "")),
        "resolve" => op_resolve(id, by), "reopen" => op_reopen(id, by), "edit" => op_edit(id, text), _ => op_drop(id),
    }
}
/// Review.op.ok: a known op, a text for an add or an edit, an id for the rest
pub fn op_ok(op: &str, p: &J) -> bool {
    match op { "add" | "edit" => !bj::trim(p.get_str("text", "")).is_empty(), "resolve" | "reopen" | "drop" => !p.get_str("id", "").is_empty(), _ => false }
}
/// Review.label: a review Act's label in every timeline (any other Act's own kind)
pub fn label(tone: &str, kind: &str, summary: &str) -> String {
    if tone != TONE { return kind.into() }
    let o = bj::parse(summary).unwrap_or(J::Null);
    match kind { "add" => if o.get_str("parent", "").is_empty() { "Comment" } else { "Reply" }, "edit" => "Edited", "resolve" => "Resolved", "reopen" => "Reopened", _ => "Removed" }.into()
}
/// Review.say: a review Act's line (any other Act's own summary)
pub fn say(tone: &str, kind: &str, summary: &str) -> String {
    if tone != TONE { return summary.into() }
    let o = bj::parse(summary).unwrap_or(J::Null);
    match kind { "add" | "edit" => o.get_str("text", "").into(), _ => "a comment".into() }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn an(f: &str, l: &str, x: f32, y: f32, i: &str) -> Anchor { Anchor { file: f.into(), commit: String::new(), kind: "board".into(), layer: l.into(), x, y, item: i.into() } }
    /// Review.mk
    fn mk(id: &str, parent: &str) -> Comment { Comment { id: id.into(), anchor: an("f", "F.Cu", 1.0, 2.0, "i"), text: "t".into(), by: "p".into(), at: 0, parent: parent.into(), resolved: false } }
    fn add(id: &str, parent: &str) -> Op { Op::Add(mk(id, parent)) }
    fn r(id: &str) -> Op { Op::Resolve(id.into(), "p".into()) }
    #[test] fn review_fold_empty() { assert!(fold(&[]).is_empty()) }
    #[test] fn review_prompt_none() { assert_eq!(prompt(&[]), "") }
    #[test] fn review_unknown_ignored() { assert!(fold(&[r("zz"), Op::Edit("zz".into(), "x".into()), Op::Drop("zz".into())]).is_empty()) }
    #[test] fn review_resolve() { assert_eq!(count_open(&fold(&[add("a", ""), r("a")])), 0) }
    #[test] fn review_reopen() { assert_eq!(count_open(&fold(&[add("a", ""), r("a"), Op::Reopen("a".into(), "p".into())])), 1) }
    #[test] fn review_prompt_resolved() { assert_eq!(prompt(&fold(&[add("a", ""), r("a")])), "") }
    #[test] fn review_drop() { assert!(fold(&[add("a", ""), add("b", "a"), Op::Drop("a".into())]).is_empty()) }
    #[test] fn review_reply_no_pin() { assert_eq!(pins(&fold(&[add("a", ""), add("b", "a")]), "f", "").len(), 1) }
    #[test] fn review_reply_counted() { assert_eq!(pin(&fold(&[add("a", ""), add("b", "a")]), &mk("a", "")).count, 1) }
    #[test] fn review_orphan_ignored() { assert!(fold(&[add("b", "a")]).is_empty()) }
    #[test] fn review_roundtrip_resolve() { assert_eq!(op_of("resolve", &op_resolve("c1", "claude"), 3), Some(Op::Resolve("c1".into(), "claude".into()))) }
    #[test] fn review_roundtrip_edit() { assert_eq!(op_of("edit", &op_edit("c1", "a b"), 3), Some(Op::Edit("c1".into(), "a b".into()))) }
    #[test] fn review_roundtrip_drop() { assert_eq!(op_of("drop", &op_drop("c1"), 3), Some(Op::Drop("c1".into()))) }
    #[test] fn review_other_tone() { assert_eq!(of("tool", "add", "{}", 1), None) }
    #[test] fn review_moved() { assert!(moved(&mk("a", ""), &["i"])) }
    #[test] fn review_not_moved() { assert!(!moved(&mk("a", ""), &["j"])) }

    /// test/review_test's log: replies, an edit, an unknown id, an orphan, a duplicate id, a bad summary, another tone, a drop
    fn log() -> Vec<Op> {
        let pcb = |x, y, l, i| an("hw/main.kicad_pcb", l, x, y, i);
        let sch = Anchor { file: "hw/power.kicad_sch".into(), commit: "abc123".into(), kind: "schematic".into(), layer: String::new(), x: 50.0, y: 25.5, item: String::new() };
        let acts: Vec<(&str, &str, String)> = vec![
            ("review", "add", op_add("c1", &pcb(10.5, 20.25, "F.Cu", "pad:U1.3"), "Trace too thin", "person", "")), ("review", "add", op_add("c2", &sch, "Add a pull-up", "claude", "")),
            ("review", "add", op_add("c1r", &pcb(0.0, 0.0, "", ""), "Widened to 0.4", "claude", "c1")), ("review", "resolve", op_resolve("c2", "person")),
            ("review", "add", op_add("c3", &pcb(1.0, 2.0, "B.Cu", ""), "Silk overlaps", "person", "")), ("review", "add", op_add("c3r", &pcb(0.0, 0.0, "", ""), "Will move", "claude", "c3")),
            ("review", "edit", op_edit("c1", "Trace too thin for 2A")), ("review", "resolve", op_resolve("nope", "person")), ("review", "add", op_add("c9", &sch, "orphan reply", "claude", "ghost")),
            ("review", "add", op_add("c1", &sch, "duplicate id", "claude", "")), ("review", "add", "{not json".into()), ("tool", "add", op_add("cx", &sch, "wrong tone", "claude", "")), ("review", "drop", op_drop("c3"))];
        acts.iter().enumerate().filter_map(|(i, (t, k, s))| of(t, k, s, i as u64 + 1)).collect()
    }
    // the expected text is test/review_test's
    #[test] fn comments_fold_as_review_test_says() {
        let cs = fold(&log());
        let show: Vec<String> = cs.iter().map(|c| format!("{}:{}:{}", c.id, c.text, if c.resolved { "r" } else { "o" })).collect();
        assert_eq!(show, ["c1:Trace too thin for 2A:o", "c2:Add a pull-up:r", "c1r:Widened to 0.4:o"]);
        assert_eq!(prompt(&cs), "<review_comments>\nOpen comments on the design (id, file, place, text). Answer or fix them, then reply or resolve with review_comment.\n- c1 hw/main.kicad_pcb (10.5, 20.25 mm, F.Cu): Trace too thin for 2A _(person)_\n    reply: Widened to 0.4 _(claude)_\n</review_comments>");
        assert_eq!(md(&cs), "- **hw/main.kicad_pcb** (10.5, 20.25 mm, F.Cu) — Trace too thin for 2A _(person)_\n  - Widened to 0.4 _(claude)_\n\nResolved\n\n- **hw/power.kicad_sch** (50, 25.5 mm) — Add a pull-up _(claude)_\n");
    }
    #[test] fn summaries_are_text_to_the_byte() {
        assert_eq!(op_add("c1", &Anchor { file: "a\"b".into(), commit: "c".into(), kind: "board".into(), layer: "F.Cu".into(), x: 10.5, y: 0.1, item: "é\n".into() }, "t", "person", ""),
            "{\"id\":\"c1\",\"a\":{\"file\":\"a\\\"b\",\"commit\":\"c\",\"kind\":\"board\",\"layer\":\"F.Cu\",\"x\":10.5,\"y\":0.1,\"item\":\"é\\n\"},\"text\":\"t\",\"by\":\"person\",\"parent\":\"\"}");
        assert_eq!(op_resolve("c", "p"), r#"{"id":"c","by":"p"}"#); assert_eq!(op_edit("c", "t"), r#"{"id":"c","text":"t"}"#); assert_eq!(op_drop("c"), r#"{"id":"c"}"#);
    }
    #[test] fn a_request_becomes_its_summary() {
        let p = bj::parse(r#"{"op":"add","file":"f","commit":"","kind":"board","layer":"F.Cu","x":1.5,"y":"2.25","item":"i","text":"t","parent":"c0"}"#).unwrap();
        assert_eq!(op_summary("add", "r1", &p, "person"), r#"{"id":"r1","a":{"file":"f","commit":"","kind":"board","layer":"F.Cu","x":1.5,"y":2.25,"item":"i"},"text":"t","by":"person","parent":"c0"}"#);
        let q = bj::parse(r#"{"id":"a","text":"new"}"#).unwrap();
        assert_eq!(op_summary("resolve", "x", &q, "b"), r#"{"id":"a","by":"b"}"#); assert_eq!(op_summary("edit", "x", &q, "b"), r#"{"id":"a","text":"new"}"#); assert_eq!(op_summary("anything", "x", &q, "b"), r#"{"id":"a"}"#);
        for (op, s, ok) in [("add", r#"{"text":"t"}"#, true), ("add", r#"{"text":" \n"}"#, false), ("edit", r#"{}"#, false), ("resolve", r#"{"id":"a"}"#, true), ("resolve", "{}", false), ("drop", r#"{"id":"a"}"#, true), ("nope", r#"{"id":"a","text":"t"}"#, false)] { assert_eq!(op_ok(op, &bj::parse(s).unwrap()), ok, "{op} {s}") }
    }
    #[test] fn a_number_that_does_not_read_is_zero_and_nan_stays_text() {
        let p = bj::parse(r#"{"text":"t","x":"abc","y":"nan"}"#).unwrap(); let s = op_summary("add", "i", &p, "b");
        assert!(s.contains("\"x\":0,\"y\":nan,"), "{s}"); // writes nan bare; reading it back, the word is null and 0
        match op_of("add", &s, 1).unwrap() { Op::Add(c) => assert_eq!((c.anchor.x, c.anchor.y), (0.0, 0.0)), o => panic!("{o:?}") }
    }
    #[test] fn labels_and_lines_in_the_thread() {
        assert_eq!(label("review", "add", r#"{"parent":""}"#), "Comment"); assert_eq!(label("review", "add", r#"{"parent":"a"}"#), "Reply"); assert_eq!(label("review", "edit", "{}"), "Edited"); assert_eq!(label("review", "resolve", "{}"), "Resolved"); assert_eq!(label("review", "reopen", "{}"), "Reopened"); assert_eq!(label("review", "drop", "{}"), "Removed"); assert_eq!(label("tool", "k", "s"), "k");
        assert_eq!(say("review", "add", r#"{"text":"t"}"#), "t"); assert_eq!(say("review", "resolve", "{}"), "a comment"); assert_eq!(say("tool", "k", "s"), "s");
    }
    #[test] fn an_issue_from_a_comment() {
        let long = "x".repeat(80); let mut c = mk("a", ""); c.text = format!("{long}\nmore"); c.anchor.commit = "abc".into();
        let all = vec![c.clone(), { let mut b = mk("b", "a"); b.text = "reply".into(); b }];
        let (t, b) = issue(&all, &c); assert_eq!(t, format!("{}...", "x".repeat(72))); assert_eq!(b, format!("{long}\nmore\n\nf (1, 2 mm, F.Cu) @ abc\n- reply _(p)_\n"));
    }
    #[test] fn a_pin_on_no_layer_shows_on_every_layer() {
        let mut a = mk("a", ""); a.anchor.layer = String::new(); let b = mk("b", "");
        let cs = fold(&[Op::Add(a), Op::Add(b)]); assert_eq!(pins(&cs, "f", "B.Cu").iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["a"]); assert_eq!(pins(&cs, "f", "F.Cu").len(), 2); assert_eq!(pins(&cs, "g", "").len(), 0);
    }
    #[test] fn a_reply_to_a_reply_is_ignored_and_resolving_a_reply_stays_on_it() {
        let cs = fold(&[add("a", ""), add("b", "a"), add("c", "b"), r("b")]); assert_eq!(cs.len(), 2); assert!(cs[1].resolved); assert_eq!(count_open(&cs), 1);
    }
}

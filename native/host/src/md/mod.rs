//! Markdown for the thread screen: src/web/dom's `Md.render` (blocks and inline) with the links of src/core/links and
//! the refs of src/core/ref, and `Mob.nodes`, which turns the nodes into the JSON the apps draw.
//! One parser for all clients in ; this is its Rust twin, checked against it on a corpus (test/screen/md_oracle.sh).
//! Strings count code points like ; `trim`, `is_space`, `is_alpha`, `is_digit` are Base's (ASCII only).
pub mod links;
pub mod refs;
pub mod doc;
mod html;
mod inline;
mod block;

use serde_json::{json, Map, Value};

/// A node of Dom: an element with its attributes (the click handlers are not kept: the JSON never shows them) or text
#[derive(Clone, Debug, PartialEq)]
pub enum Node { El { tag: &'static str, attrs: Vec<(&'static str, String)>, kids: Vec<Node> }, Txt(String) }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Loc { En, Ja }

pub fn el(tag: &'static str, kids: Vec<Node>) -> Node { Node::El { tag, attrs: vec![], kids } }

/// Md.render: markdown text as nodes
pub fn render(s: &str) -> Vec<Node> { block::render(Loc::En, s) }
pub fn render_loc(lc: Loc, s: &str) -> Vec<Node> { block::render(lc, s) }

/// Mob.nodes: the JSON a phone draws (tag, kids, then href/file, ref/bare or the ordered item's number)
pub fn nodes_json(ns: &[Node]) -> Vec<Value> { ns.iter().map(node_json).collect() }

fn node_json(n: &Node) -> Value {
    match n {
        Node::Txt(t) => json!({ "text": t }),
        Node::El { tag, attrs, kids } => {
            let mut m = Map::new();
            m.insert("tag".into(), json!(tag)); m.insert("kids".into(), Value::Array(nodes_json(kids)));
            href(attrs, &mut m);
            doc_attrs(tag, attrs, &mut m);
            Value::Object(m)
        }
    }
}

/// A document's attributes (doc.rs): a picture's, a block's alignment, an anchor, a task's mark, an alert's kind, a code block's language. The chat's
/// nodes never carry these, so its JSON is what it was; a document's link says itself whether it is a file (`#anchor` and `mailto:` are not).
fn doc_attrs(tag: &str, attrs: &[(&'static str, String)], m: &mut Map<String, Value>) {
    for (k, v) in attrs {
        match *k {
            "src" | "alt" | "width" | "height" | "align" | "id" | "checked" | "alert" | "lang" | "open" | "media" | "start" => { m.entry(k.to_string()).or_insert_with(|| json!(v)); }
            "title" if tag == "img" => { m.insert("title".into(), json!(v)); }
            "file" => { m.insert("file".into(), json!(v == "1")); }
            _ => {}
        }
    }
}

/// Mob.href: data-ref wins (ref, bare); else the first href (href, file: not a web address); an ordered item's value is its number "n"
fn href(attrs: &[(&'static str, String)], m: &mut Map<String, Value>) {
    if let Some((_, uri)) = attrs.iter().find(|(k, _)| *k == "data-ref") {
        if !uri.is_empty() {
            let bare = attrs.iter().find(|(k, _)| *k == "data-bare").map_or(false, |(_, v)| v == "1");
            m.insert("ref".into(), json!(uri)); m.insert("bare".into(), json!(bare)); return;
        }
    }
    for (k, v) in attrs {
        if *k == "href" { m.insert("href".into(), json!(v)); m.insert("file".into(), json!(!links::is_web(v))); return }
        if *k == "value" { m.insert("n".into(), json!(v)); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const GOLD_IN: [&str; 6] = ["a **b** *c* `d` e * f **g", "x [lab] y [l](http://e.x/a) **b `c` d** *e `f`* `unclosed", "**a*b** *x **y** z* ***w*** * lone", "path `src/a.rs` and [t1] *", "a*\n*b **c\n d** e`f\ng`", "tab\t*x*\t**y**"];
    const GOLD_OUT: [&str; 6] = [
        r##"[{"tag":"p","kids":[{"text":"a "},{"tag":"strong","kids":[{"text":"b"}]},{"text":" "},{"tag":"em","kids":[{"text":"c"}]},{"text":" "},{"tag":"code","kids":[{"text":"d"}]},{"text":" e * f "},{"text":"**g"}]}]"##,
        r##"[{"tag":"p","kids":[{"text":"x [lab] y "},{"tag":"a","kids":[{"text":"l"}],"href":"http://e.x/a","file":false},{"text":" "},{"tag":"strong","kids":[{"text":"b "},{"tag":"code","kids":[{"text":"c"}]},{"text":" d"}]},{"text":" "},{"tag":"em","kids":[{"text":"e "},{"tag":"code","kids":[{"text":"f"}]}]},{"text":" "},{"text":"`unclosed"}]}]"##,
        r##"[{"tag":"p","kids":[{"tag":"strong","kids":[{"text":"a*b"}]},{"text":" "},{"tag":"em","kids":[{"text":"x "}]},{"tag":"em","kids":[{"text":"y"}]},{"text":"* z* "},{"tag":"strong","kids":[{"text":"*w"}]},{"text":"* * lone"}]}]"##,
        r##"[{"tag":"p","kids":[{"text":"path "},{"tag":"a","kids":[{"tag":"code","kids":[{"text":"src/a.rs"}]}],"href":"src/a.rs","file":true},{"text":" and [t1] *"}]}]"##,
        r##"[{"tag":"p","kids":[{"text":"a"},{"tag":"em","kids":[{"text":"\n"}]},{"text":"b "},{"tag":"strong","kids":[{"text":"c\n d"}]},{"text":" e"},{"tag":"code","kids":[{"text":"f\ng"}]}]}]"##,
        r##"[{"tag":"p","kids":[{"text":"tab\t"},{"tag":"em","kids":[{"text":"x"}]},{"text":"\t"},{"tag":"strong","kids":[{"text":"y"}]}]}]"##,
    ];
    fn j(s: &str) -> String { serde_json::to_string(&nodes_json(&render(s))).unwrap() }
    #[test] fn paragraph_and_emphasis() { assert_eq!(j("a **b** *c*"), r#"[{"tag":"p","kids":[{"text":"a "},{"tag":"strong","kids":[{"text":"b"}]},{"text":" "},{"tag":"em","kids":[{"text":"c"}]}]}]"#); }
    #[test] fn heading_and_list() { assert_eq!(j("# T\n- a\n- b"), r#"[{"tag":"h3","kids":[{"text":"T"}]},{"tag":"ul","kids":[{"tag":"li","kids":[{"text":"a"}]},{"tag":"li","kids":[{"text":"b"}]}]}]"#); }
    #[test] fn nested_lists_sit_inside_their_item() {
        assert_eq!(j("- a\n  - b\n  - c\n- d"), r#"[{"tag":"ul","kids":[{"tag":"li","kids":[{"text":"a"},{"tag":"ul","kids":[{"tag":"li","kids":[{"text":"b"}]},{"tag":"li","kids":[{"text":"c"}]}]}]},{"tag":"li","kids":[{"text":"d"}]}]}]"#);
        let o = j("1. a\n   - x\n     1. deep\n2. b");
        assert!(o.contains(r#"{"text":"a"},{"tag":"ul""#), "{o}");
        assert!(o.contains(r#""deep""#) && o.contains(r#""n":"2""#), "{o}");
        // a kind change at one level is a second list inside the same item; a dedent below the first item starts a new list
        assert_eq!(j("- a\n  - b\n  1. c"), r#"[{"tag":"ul","kids":[{"tag":"li","kids":[{"text":"a"},{"tag":"ul","kids":[{"tag":"li","kids":[{"text":"b"}]}]},{"tag":"ol","kids":[{"tag":"li","kids":[{"text":"c"}]}]}]}]}]"#.replace(r#"{"tag":"li","kids":[{"text":"c"}]}"#, r#"{"tag":"li","kids":[{"text":"c"}],"n":"1"}"#));
        assert_eq!(j("  - a\n- b"), r#"[{"tag":"ul","kids":[{"tag":"li","kids":[{"text":"a"}]}]},{"tag":"ul","kids":[{"tag":"li","kids":[{"text":"b"}]}]}]"#);
    }
    #[test] fn ordered_numbers_kept() { assert!(j("3. x\n4. y").contains(r#""n":"3""#)); }
    // The inline pass copied its run for every character, so a paragraph cost the square of its length: 135 KB of lines with no blank line between them
    // took 3 s, and every picture of a thread rendered its page again. Twice the text now costs about twice the time; the bound here is far above a linear
    // pass on a loaded runner and below the quadratic one (about 7 s for this text).
    #[test] fn a_long_paragraph_renders_in_time_that_grows_with_its_length() {
        let lines: String = (0..5000).map(|i| format!("line {i} of plain words with nothing to mark up in it\n")).collect();
        assert!(lines.len() > 200_000);
        let t = std::time::Instant::now(); let n = render(&lines); let took = t.elapsed();
        assert_eq!(n.len(), 1, "one paragraph");
        assert!(took < std::time::Duration::from_secs(3), "{} KB of one paragraph took {took:?}", lines.len() / 1024);
        // and what it renders is what it rendered: emphasis, code and text runs, a star before a space kept as text
        // and what it renders is what it rendered before (taken from the copying pass): open and closed emphasis, code, labels, stars before spaces
        for (t, want) in GOLD_IN.iter().zip(GOLD_OUT) { assert_eq!(j(t), want, "{t:?}"); }
    }
}

//! Markdown for the thread screen: src/web/dom's `Md.render` (blocks and inline) with the links of src/core/links and
//! the refs of src/core/ref, and `Mob.nodes`, which turns the nodes into the JSON the apps draw.
//! One parser for all clients in ; this is its Rust twin, checked against it on a corpus (test/screen/md_oracle.sh).
//! Strings count code points like ; `trim`, `is_space`, `is_alpha`, `is_digit` are Base's (ASCII only).
pub mod links;
pub mod refs;
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
            Value::Object(m)
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
    fn j(s: &str) -> String { serde_json::to_string(&nodes_json(&render(s))).unwrap() }
    #[test] fn paragraph_and_emphasis() { assert_eq!(j("a **b** *c*"), r#"[{"tag":"p","kids":[{"text":"a "},{"tag":"strong","kids":[{"text":"b"}]},{"text":" "},{"tag":"em","kids":[{"text":"c"}]}]}]"#); }
    #[test] fn heading_and_list() { assert_eq!(j("# T\n- a\n- b"), r#"[{"tag":"h3","kids":[{"text":"T"}]},{"tag":"ul","kids":[{"tag":"li","kids":[{"text":"a"}]},{"tag":"li","kids":[{"text":"b"}]}]}]"#); }
    #[test] fn ordered_numbers_kept() { assert!(j("3. x\n4. y").contains(r#""n":"3""#)); }
}

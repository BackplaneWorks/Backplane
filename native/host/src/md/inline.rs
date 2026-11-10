//! Md.inline of src/web/dom: emphasis, code, labels and the links of `links::pieces`, as nodes.
use super::{el, links, refs, Node};

type S = Vec<char>;
fn st(s: &[char]) -> String { s.iter().collect() }

fn file(path: &str, kids: Vec<Node>) -> Node {
    Node::El { tag: "a", attrs: vec![("class", "link file".into()), ("href", path.into()), ("title", path.into()), ("data-value", path.into())], kids }
}

/// Dom.ref: an explicit reference navigates in place; a bare id stays text
fn dref(label: &str, uri: &str, bare: bool) -> Node {
    if bare { Node::Txt(label.into()) } else {
        Node::El { tag: "a", attrs: vec![("class", "ref".into()), ("href", uri.into()), ("data-ref", uri.into()), ("data-bare", "0".into()), ("data-value", uri.into())], kids: vec![Node::Txt(label.into())] }
    }
}

/// Dom.link: a ref navigates, a web address opens, anything else is a file
fn dlink(label: &str, url: &str) -> Node {
    if let Some(r) = refs::parse(url) { return dref(label, &refs::uri(&r), false) }
    if links::is_web(url) {
        Node::El { tag: "a", attrs: vec![("class", "link".into()), ("href", url.into()), ("target", "_blank".into()), ("rel", "noopener noreferrer".into()), ("title", url.into())], kids: vec![Node::Txt(label.into())] }
    } else { file(url, vec![Node::Txt(label.into())]) }
}

/// Md.text: a finished run of plain text, its links as anchors
fn text(acc: &[char], out: &mut Vec<Node>) {
    if acc.is_empty() { return }
    for p in links::pieces(&st(acc)) {
        out.push(match p { links::Piece::Text(s) => Node::Txt(s), links::Piece::Link(l, u) => dlink(&l, &u), links::Piece::Ref(l, u, b) => dref(&l, &u, b) });
    }
}

/// Md.code: inline code; a path in backticks is a file's link
fn code(acc: &[char]) -> Node {
    let s = st(acc);
    let c = el("code", vec![Node::Txt(s.clone())]);
    if links::is_path(&s) { file(&s, vec![c]) } else { c }
}

/// Md.emphasis: text runs and code spans inside ** or *
fn emphasis(acc: &[char]) -> Vec<Node> {
    let (mut code_on, mut cur, mut out): (bool, S, Vec<Node>) = (false, vec![], vec![]);
    for &c in acc {
        if c == '`' {
            if code_on { out.push(code(&cur)); cur.clear(); code_on = false } else { text(&cur, &mut out); cur.clear(); code_on = true }
        } else { cur.push(c) }
    }
    if code_on { let mut a = vec!['`']; a.extend_from_slice(&cur); text(&a, &mut out) } else { text(&cur, &mut out) }
    out
}

#[derive(Clone)]
enum Inl { Text(S), Star(S), Code(S), Bold(S), BoldStar(S), Em(S), Label(S) }

fn pre(p: &str, acc: &[char]) -> S { let mut a: S = p.chars().collect(); a.extend_from_slice(acc); a }
/// The run so far with `c` added. It takes the run and gives it back: a copy for every character made a paragraph's cost grow with the square of its length
/// (a 35 KB paragraph took 88 ms to render, a 135 KB one 3 s).
fn pushed(mut acc: S, c: char) -> S { acc.push(c); acc }

/// Md.igo: the inline state machine over a line or paragraph
fn igo(s: &[char]) -> Vec<Node> {
    let (mut state, mut out) = (Inl::Text(vec![]), Vec::<Node>::new());
    for &c in s {
        let tick = c == '`'; let star = c == '*'; let space = c == ' ' || c == '\t';
        state = match state {
            Inl::Label(acc) => if c == ']' { Inl::Text(pushed(acc, c)) } else { Inl::Label(pushed(acc, c)) },
            Inl::Text(acc) => {
                if tick { text(&acc, &mut out); Inl::Code(vec![]) }
                else if star { Inl::Star(acc) }
                else if space { Inl::Text(pushed(acc, c)) }
                else if c == '[' { Inl::Label(pushed(acc, c)) } else { Inl::Text(pushed(acc, c)) }
            }
            Inl::Star(acc) => {
                if star { text(&acc, &mut out); Inl::Bold(vec![]) }
                else if space { let mut a = acc; a.push('*'); a.push(c); Inl::Text(a) }
                else { text(&acc, &mut out); Inl::Em(vec![c]) }
            }
            Inl::Em(acc) => if star { out.push(el("em", emphasis(&acc))); Inl::Text(vec![]) } else { Inl::Em(pushed(acc, c)) },
            Inl::Code(acc) => if tick { out.push(code(&acc)); Inl::Text(vec![]) } else { Inl::Code(pushed(acc, c)) },
            Inl::Bold(acc) => if star { Inl::BoldStar(acc) } else { Inl::Bold(pushed(acc, c)) },
            Inl::BoldStar(acc) => if star { out.push(el("strong", emphasis(&acc))); Inl::Text(vec![]) } else { let mut a = acc; a.push('*'); a.push(c); Inl::Bold(a) },
        };
    }
    match state {
        Inl::Label(a) | Inl::Text(a) => text(&a, &mut out),
        Inl::Star(a) => { let mut x = a; x.push('*'); text(&x, &mut out) }
        Inl::Code(a) => text(&pre("`", &a), &mut out),
        Inl::Bold(a) => text(&pre("**", &a), &mut out),
        Inl::BoldStar(a) => { let mut x = pre("**", &a); x.push('*'); text(&x, &mut out) }
        Inl::Em(a) => text(&pre("*", &a), &mut out),
    }
    out
}

/// Md.label.nodes: nested anchors are flattened
fn label_nodes(ns: Vec<Node>) -> Vec<Node> {
    let mut out = vec![];
    for n in ns {
        match n {
            Node::Txt(_) => out.push(n),
            Node::El { tag: "a", kids, .. } => out.extend(label_nodes(kids)),
            Node::El { tag, attrs, kids } => out.push(Node::El { tag, attrs, kids: label_nodes(kids) }),
        }
    }
    out
}

/// Md.labels: an anchor with one text kid has that label parsed again for emphasis
fn labels(ns: Vec<Node>) -> Vec<Node> {
    ns.into_iter().map(|n| match n {
        Node::El { tag: "a", attrs, kids } if kids.len() == 1 && matches!(kids[0], Node::Txt(_)) => {
            let label = match &kids[0] { Node::Txt(t) => t.clone(), _ => unreachable!() };
            let cs: S = label.chars().collect();
            Node::El { tag: "a", attrs, kids: label_nodes(igo(&cs)) }
        }
        o => o,
    }).collect()
}

/// Md.inline
pub fn inline(s: &str) -> Vec<Node> { let cs: S = s.chars().collect(); labels(igo(&cs)) }

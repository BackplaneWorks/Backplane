//! Md.render's block layer: lines classified, then folded into paragraphs, lists, quotes, tables and code.
use super::{el, inline::inline, Loc, Node};

enum Line { Fence, Head(usize, String), Item(String), Num(String, String), Quote(String), Row(Vec<String>), Sep(String), Hr, Blank, Text(String) }

fn trim_start(s: &str) -> &str { s.trim_start_matches(|c: char| c == ' ' || ('\t'..='\r').contains(&c)) }
fn trim(s: &str) -> &str { trim_start(s).trim_end_matches(|c: char| c == ' ' || ('\t'..='\r').contains(&c)) }

/// Md.is_sep: a table's separator row, | --- | :-: |
fn is_sep(t: &str) -> bool { t.contains('-') && trim(t).chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) }
/// Md.is_hr: three or more of - * or _
fn is_hr(t: &str) -> bool { t.chars().count() >= 3 && ['-', '*', '_'].iter().any(|&u| t.chars().all(|c| c == u)) }

/// Md.cells: | a | b | gives a and b
fn cells(t: &str) -> Vec<String> {
    let mut v: Vec<&str> = trim(t).split('|').collect();
    v.remove(0);
    if v.last().map_or(false, |l| trim(l).is_empty()) && !v.is_empty() { v.pop(); }
    v.into_iter().map(|c| trim(c).to_string()).collect()
}

/// Md.line
fn classify(l: &str) -> Line {
    let t = trim_start(l);
    if t.starts_with("```") { return Line::Fence }
    let n = t.chars().take_while(|&c| c == '#').count();
    let r: String = t.chars().skip(n).collect();
    if n > 0 && n < 7 && r.starts_with(' ') { return Line::Head(n, trim_start(&r).to_string()) }
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") { return Line::Item(t.chars().skip(2).collect()) }
    if is_hr(trim(t)) { return Line::Hr }
    if t.starts_with('|') { return if is_sep(t) { Line::Sep(l.to_string()) } else { Line::Row(cells(t)) } }
    if t.starts_with('>') { return Line::Quote(trim_start(&t.chars().skip(1).collect::<String>()).to_string()) }
    let d = t.chars().take_while(|c| c.is_ascii_digit()).count();
    let r: String = t.chars().skip(d).collect();
    if d > 0 && d < 10 && (r.starts_with(". ") || r.starts_with(") ")) { return Line::Num(t.chars().take(d).collect(), r.chars().skip(2).collect()) }
    if trim(t).is_empty() { Line::Blank } else { Line::Text(l.to_string()) }
}

enum Blk { None, Para(Vec<String>), Code(Vec<String>), List(Vec<String>), Ol(Vec<(String, String)>), Quote(Vec<String>), Table(Vec<(bool, Vec<String>)>) }

fn codeblock(lc: Loc, ls: &[String]) -> Node {
    let text = ls.join("\n");
    let copy = Node::El { tag: "button", attrs: vec![("class", "copy".into()), ("title", copy_word(lc).into()), ("data-value", text.clone())], kids: vec![Node::Txt(copy_word(lc).into())] };
    Node::El { tag: "div", attrs: vec![("class", "codeblock".into())], kids: vec![copy, el("pre", vec![el("code", vec![Node::Txt(text)])])] }
}
fn copy_word(lc: Loc) -> &'static str { match lc { Loc::En => "Copy", Loc::Ja => "コピー" } }

fn close(lc: Loc, b: Blk, out: &mut Vec<Node>) {
    match b {
        Blk::None => {}
        Blk::Para(ls) => out.push(el("p", inline(&ls.join("\n")))),
        Blk::Code(ls) => out.push(codeblock(lc, &ls)),
        Blk::List(xs) => out.push(el("ul", xs.iter().map(|x| el("li", inline(x))).collect())),
        Blk::Ol(xs) => out.push(el("ol", xs.iter().map(|(n, t)| Node::El { tag: "li", attrs: vec![("value", n.clone())], kids: inline(t) }).collect())),
        Blk::Quote(ls) => out.push(el("blockquote", inline(&ls.join("\n")))),
        Blk::Table(rs) => out.push(el("table", rs.iter().map(|(head, cs)| el("tr", cs.iter().map(|c| el(if *head { "th" } else { "td" }, inline(c))).collect())).collect())),
    }
}

fn head(n: usize, t: &str) -> Node { el(match n { 1 => "h3", 2 => "h4", _ => "h5" }, inline(t)) }

/// Md.render: Diff.lines splits on \n (a final empty line included)
pub fn render(lc: Loc, s: &str) -> Vec<Node> {
    let (mut b, mut out) = (Blk::None, Vec::<Node>::new());
    for raw in s.split('\n') {
        let ln = classify(raw);
        b = match (b, ln) {
            (Blk::Code(ls), Line::Fence) => { out.push(codeblock(lc, &ls)); Blk::None }
            (Blk::Code(mut ls), _) => { ls.push(raw.to_string()); Blk::Code(ls) }
            (b, Line::Fence) => { close(lc, b, &mut out); Blk::Code(vec![]) }
            (b, Line::Head(n, t)) => { close(lc, b, &mut out); out.push(head(n, &t)); Blk::None }
            (Blk::List(mut xs), Line::Item(t)) => { xs.push(t); Blk::List(xs) }
            (b, Line::Item(t)) => { close(lc, b, &mut out); Blk::List(vec![t]) }
            (Blk::Ol(mut xs), Line::Num(n, t)) => { xs.push((n, t)); Blk::Ol(xs) }
            (b, Line::Num(n, t)) => { close(lc, b, &mut out); Blk::Ol(vec![(n, t)]) }
            (Blk::Quote(mut ls), Line::Quote(t)) => { ls.push(t); Blk::Quote(ls) }
            (b, Line::Quote(t)) => { close(lc, b, &mut out); Blk::Quote(vec![t]) }
            (Blk::Table(mut rs), Line::Row(cs)) => { rs.push((false, cs)); Blk::Table(rs) }
            (b, Line::Row(cs)) => { close(lc, b, &mut out); Blk::Table(vec![(false, cs)]) }
            // a separator after the first row makes it the head (Md.headed); after more rows it changes nothing
            (Blk::Table(mut rs), Line::Sep(_)) => { if rs.len() == 1 && !rs[0].0 { rs[0].0 = true } Blk::Table(rs) }
            (Blk::Para(mut ls), Line::Sep(t)) => { ls.push(t); Blk::Para(ls) }
            (b, Line::Sep(t)) => { close(lc, b, &mut out); Blk::Para(vec![t]) }
            (b, Line::Hr) => { close(lc, b, &mut out); out.push(el("hr", vec![])); Blk::None }
            (b, Line::Blank) => { close(lc, b, &mut out); Blk::None }
            (Blk::Para(mut ls), Line::Text(t)) => { ls.push(t); Blk::Para(ls) }
            (b, Line::Text(t)) => { close(lc, b, &mut out); Blk::Para(vec![t]) }
        };
    }
    close(lc, b, &mut out);
    out
}

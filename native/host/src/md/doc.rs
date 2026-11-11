//! A document's Markdown (a README, a note opened in the file viewer) as GitHub draws it: CommonMark with GitHub's extensions (tables, task
//! lists, strikethrough, footnotes, alerts) and the HTML subset of html.rs, as the same nodes the chat's Markdown makes ({tag, kids|text, ...}),
//! with the attributes a document adds: an image's `src` (a web address, or the absolute path of a file beside the document), `alt`, `title`,
//! `width`, `height`; `align` on blocks and cells; a heading's anchor `id`; a task item's `checked`; an alert's kind; a code block's `lang`;
//! `details` and its `summary`; `picture` with its `source`s. The chat's own parser (block.rs) is untouched: it is held to its oracle.
use super::{html::{self, Tok}, Node};
use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// Where a document's relative addresses point: the folder it is in, and the folder that `/x` means (the repository's, as on GitHub).
/// Files outside `root` are not linked or shown.
#[derive(Clone, Debug)]
pub struct Base { pub dir: PathBuf, pub root: PathBuf }
impl Base {
    /// A file's: its folder, and the repository it is in (the nearest folder above with `.git`), else its folder.
    pub fn for_file(p: &Path) -> Base {
        let dir = p.parent().map_or_else(|| PathBuf::from("/"), Path::to_path_buf);
        let root = dir.ancestors().find(|a| a.join(".git").exists()).map_or_else(|| dir.clone(), Path::to_path_buf);
        Base { dir, root }
    }
}

/// The document's nodes.
pub fn render(s: &str, base: &Base) -> Vec<Node> {
    let mut b = Builder::new(base);
    let opts = Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_GFM;
    let mut html_buf: Option<String> = None;
    for ev in Parser::new_ext(s, opts) {
        match ev {
            Event::Start(Tag::HtmlBlock) => html_buf = Some(String::new()),
            Event::End(TagEnd::HtmlBlock) => { if let Some(h) = html_buf.take() { b.html(&h) } }
            Event::Html(h) => match html_buf.as_mut() { Some(buf) => buf.push_str(&h), None => b.html(&h) },
            Event::InlineHtml(h) => b.html(&h),
            Event::Start(t) => b.start(t),
            Event::End(t) => b.end(t),
            Event::Text(t) => b.text(&t),
            Event::Code(t) => { b.open_md("code", vec![]); b.text(&t); b.close_md("code"); }
            Event::InlineMath(t) | Event::DisplayMath(t) => { b.open_md("code", vec![]); b.text(&t); b.close_md("code"); }
            Event::SoftBreak => b.text(" "),
            Event::HardBreak => b.void("br", vec![]),
            Event::Rule => b.void("hr", vec![]),
            Event::TaskListMarker(done) => b.checked(done),
            Event::FootnoteReference(l) => b.note_ref(&l),
        }
    }
    b.finish()
}

/// A document that is HTML (README.html): its body through the same tokens.
pub fn render_html(s: &str, base: &Base) -> Vec<Node> { let mut b = Builder::new(base); b.html(s); b.finish() }

struct Open { tag: &'static str, attrs: Vec<(&'static str, String)>, kids: Vec<Node>, md: bool }

struct Builder<'a> {
    base: &'a Base, stack: Vec<Open>,
    /// a table's column alignments, whether its head row is being read, and the column of the next cell
    aligns: Vec<&'static str>, head: bool, col: usize,
    /// the next number of an ordered list's items, innermost last
    nums: Vec<Option<u64>>,
    /// footnotes: each label's number (in order of first reference) and its definition's nodes
    notes: HashMap<String, usize>, defs: Vec<(String, Vec<Node>)>,
    slugs: HashMap<String, usize>,
}

const BLOCKS: &[&str] = &["p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "pre", "ul", "ol", "li", "table", "tr", "th", "td", "hr", "details", "summary", "dt", "dd", "picture"];
fn is_block(t: &str) -> bool { BLOCKS.contains(&t) }

impl<'a> Builder<'a> {
    fn new(base: &'a Base) -> Self { Self { base, stack: vec![Open { tag: "", attrs: vec![], kids: vec![], md: true }], aligns: vec![], head: false, col: 0, nums: vec![], notes: HashMap::new(), defs: vec![], slugs: HashMap::new() } }
    fn top(&mut self) -> &mut Open { self.stack.last_mut().expect("root") }
    fn push(&mut self, n: Node) { self.top().kids.push(n) }
    fn text(&mut self, t: &str) {
        if t.is_empty() { return }
        if let Some(Node::Txt(last)) = self.top().kids.last_mut() { last.push_str(t); return }
        self.push(Node::Txt(t.to_string()))
    }
    fn open(&mut self, tag: &'static str, attrs: Vec<(&'static str, String)>, md: bool) { self.stack.push(Open { tag, attrs, kids: vec![], md }) }
    fn open_md(&mut self, tag: &'static str, attrs: Vec<(&'static str, String)>) { self.open(tag, attrs, true) }
    /// Pop the top into its parent.
    fn pop(&mut self) {
        if self.stack.len() < 2 { return }
        let o = self.stack.pop().expect("open");
        let n = self.finish_el(o);
        if let Some(n) = n { self.push(n) }
    }
    /// The element as a node: an image's alt from its text, a heading's anchor, a footnote definition set aside.
    fn finish_el(&mut self, mut o: Open) -> Option<Node> {
        match o.tag {
            "img" if o.md => { let mut alt = String::new(); for k in &o.kids { plain(k, &mut alt) } o.kids.clear(); o.attrs.push(("alt", alt)); }
            "fndef" => { let label = o.attrs.first().map(|a| a.1.clone()).unwrap_or_default(); self.defs.push((label, o.kids)); return None }
            t if t.len() == 2 && t.starts_with('h') && !o.attrs.iter().any(|a| a.0 == "id") => {
                let mut s = String::new(); for k in &o.kids { plain(k, &mut s) }
                let id = self.slug(&s); o.attrs.push(("id", id));
            }
            _ => {}
        }
        Some(Node::El { tag: o.tag, attrs: o.attrs, kids: o.kids })
    }
    /// Close the innermost Markdown element named `tag`, and any HTML element left open inside it.
    fn close_md(&mut self, tag: &'static str) {
        let Some(at) = self.stack.iter().rposition(|o| o.md && o.tag == tag) else { return };
        if at == 0 { return }
        while self.stack.len() > at { self.pop() }
    }
    fn void(&mut self, tag: &'static str, attrs: Vec<(&'static str, String)>) { if tag == "hr" { self.close_p() } self.push(Node::El { tag, attrs, kids: vec![] }) }
    /// A block opening inside an HTML paragraph ends it, as a browser does.
    fn close_p(&mut self) { if let Some(o) = self.stack.last() { if o.tag == "p" && !o.md { self.pop() } } }

    fn start(&mut self, t: Tag) {
        match t {
            Tag::Paragraph => self.open_md("p", vec![]),
            Tag::Heading { level, id, .. } => {
                let tag = match level { HeadingLevel::H1 => "h1", HeadingLevel::H2 => "h2", HeadingLevel::H3 => "h3", HeadingLevel::H4 => "h4", HeadingLevel::H5 => "h5", HeadingLevel::H6 => "h6" };
                self.open_md(tag, id.map(|i| vec![("id", i.to_string())]).unwrap_or_default());
            }
            Tag::BlockQuote(kind) => {
                let k = kind.map(|k| match k { BlockQuoteKind::Note => "note", BlockQuoteKind::Tip => "tip", BlockQuoteKind::Important => "important", BlockQuoteKind::Warning => "warning", BlockQuoteKind::Caution => "caution" });
                self.open_md("blockquote", k.map(|k| vec![("alert", k.to_string())]).unwrap_or_default());
            }
            Tag::CodeBlock(k) => {
                let lang = match k { CodeBlockKind::Fenced(l) => l.split(|c: char| c.is_whitespace() || c == ',' || c == '{').next().unwrap_or("").to_string(), CodeBlockKind::Indented => String::new() };
                // GitHub colours only code that names its language
                self.open_md("pre", vec![("lang", if lang.is_empty() { "text".into() } else { lang })]);
            }
            Tag::List(start) => { self.nums.push(start); self.open_md(if start.is_some() { "ol" } else { "ul" }, vec![]) }
            Tag::Item => {
                let n = self.nums.last_mut().and_then(|n| n.as_mut().map(|v| { let x = *v; *v += 1; x }));
                self.open_md("li", n.map(|n| vec![("value", n.to_string())]).unwrap_or_default());
            }
            Tag::FootnoteDefinition(l) => self.open_md("fndef", vec![("label", l.to_string())]),
            Tag::Table(al) => {
                self.aligns = al.iter().map(|a| match a { pulldown_cmark::Alignment::Left => "left", pulldown_cmark::Alignment::Center => "center", pulldown_cmark::Alignment::Right => "right", pulldown_cmark::Alignment::None => "" }).collect();
                self.open_md("table", vec![]);
            }
            Tag::TableHead => { self.head = true; self.col = 0; self.open_md("tr", vec![]) }
            Tag::TableRow => { self.col = 0; self.open_md("tr", vec![]) }
            Tag::TableCell => {
                let a = self.aligns.get(self.col).copied().unwrap_or(""); self.col += 1;
                self.open_md(if self.head { "th" } else { "td" }, if a.is_empty() { vec![] } else { vec![("align", a.to_string())] });
            }
            Tag::Emphasis => self.open_md("em", vec![]),
            Tag::Strong => self.open_md("strong", vec![]),
            Tag::Strikethrough => self.open_md("del", vec![]),
            Tag::Superscript => self.open_md("sup", vec![]),
            Tag::Subscript => self.open_md("sub", vec![]),
            Tag::Link { dest_url, title, .. } => {
                let mut a = vec![];
                if let Some(h) = self.href(&dest_url) { a.extend(h) }
                if !title.is_empty() { a.push(("title", title.to_string())) }
                self.open_md("a", a);
            }
            Tag::Image { dest_url, title, .. } => {
                let mut a = vec![];
                if let Some(s) = self.src(&dest_url) { a.push(("src", s)) }
                if !title.is_empty() { a.push(("title", title.to_string())) }
                self.open_md("img", a);
            }
            Tag::DefinitionList => self.open_md("div", vec![]),
            Tag::DefinitionListTitle => self.open_md("dt", vec![]),
            Tag::DefinitionListDefinition => self.open_md("dd", vec![]),
            Tag::HtmlBlock | Tag::MetadataBlock(_) => self.open_md("meta", vec![]),
        }
    }
    fn end(&mut self, t: TagEnd) {
        let tag = match t {
            TagEnd::Paragraph => "p",
            TagEnd::Heading(l) => match l { HeadingLevel::H1 => "h1", HeadingLevel::H2 => "h2", HeadingLevel::H3 => "h3", HeadingLevel::H4 => "h4", HeadingLevel::H5 => "h5", HeadingLevel::H6 => "h6" },
            TagEnd::BlockQuote(_) => "blockquote", TagEnd::CodeBlock => "pre",
            TagEnd::List(_) => { self.nums.pop(); if self.stack.iter().rev().find(|o| o.md && matches!(o.tag, "ol" | "ul")).map_or(false, |o| o.tag == "ol") { "ol" } else { "ul" } }
            TagEnd::Item => "li", TagEnd::FootnoteDefinition => "fndef", TagEnd::Table => "table",
            TagEnd::TableHead => { self.head = false; "tr" } TagEnd::TableRow => "tr",
            TagEnd::TableCell => if self.head { "th" } else { "td" },
            TagEnd::Emphasis => "em", TagEnd::Strong => "strong", TagEnd::Strikethrough => "del", TagEnd::Superscript => "sup", TagEnd::Subscript => "sub",
            TagEnd::Link => "a", TagEnd::Image => "img",
            TagEnd::DefinitionList => "div", TagEnd::DefinitionListTitle => "dt", TagEnd::DefinitionListDefinition => "dd",
            TagEnd::HtmlBlock | TagEnd::MetadataBlock(_) => "meta",
        };
        if tag == "meta" {
            // front matter is not shown
            if let Some(at) = self.stack.iter().rposition(|o| o.md && o.tag == "meta") { if at > 0 { self.stack.truncate(at) } }
            return;
        }
        self.close_md(tag)
    }
    fn checked(&mut self, done: bool) {
        if let Some(li) = self.stack.iter_mut().rev().find(|o| o.tag == "li") { li.attrs.push(("checked", done.to_string())) }
    }
    fn note_ref(&mut self, label: &str) {
        let next = self.notes.len() + 1;
        let n = *self.notes.entry(label.to_string()).or_insert(next);
        let a = Node::El { tag: "a", attrs: vec![("href", format!("#fn-{}", slug_part(label))), ("file", "0".into()), ("id", format!("fnref-{}", slug_part(label)))], kids: vec![Node::Txt(n.to_string())] };
        self.push(Node::El { tag: "sup", attrs: vec![], kids: vec![a] });
    }

    /// HTML tokens into the tree: an open tag stays open across Markdown until its close tag or the end of what holds it.
    fn html(&mut self, s: &str) {
        for t in html::tokens(s) {
            match t {
                Tok::Text(x) => self.text(&x),
                Tok::Void(tag, a) => {
                    let a = self.attrs(tag, a);
                    if tag == "img" && !a.iter().any(|x| x.0 == "src") { let alt = a.iter().find(|x| x.0 == "alt").map(|x| x.1.clone()).unwrap_or_default(); self.text(&alt); continue }
                    self.void(tag, a)
                }
                Tok::Open(tag, a) => {
                    if is_block(tag) && tag != "picture" && tag != "summary" { self.close_p() }
                    if tag == "li" { if let Some(o) = self.stack.last() { if o.tag == "li" && !o.md { self.pop() } } }
                    let a = self.attrs(tag, a);
                    self.open(tag, a, false)
                }
                Tok::Close(tag) => {
                    // the innermost open element of that name, closing what is inside it; a stray close tag is dropped
                    let Some(at) = self.stack.iter().rposition(|o| o.tag == tag && !o.md) else { continue };
                    if at == 0 { continue }
                    while self.stack.len() > at { self.pop() }
                }
            }
        }
    }
    /// An HTML element's addresses resolved like Markdown's; an unusable one dropped.
    fn attrs(&mut self, tag: &str, a: Vec<(&'static str, String)>) -> Vec<(&'static str, String)> {
        let mut out = vec![];
        for (k, v) in a {
            match k {
                "href" => if let Some(h) = self.href(&v) { out.extend(h) },
                "src" => if let Some(s) = self.src(&v) { out.push(("src", s)) },
                "srcset" => { let first = v.split(',').next().unwrap_or("").split_whitespace().next().unwrap_or(""); if let Some(s) = self.src(first) { out.push(("src", s)) } }
                "id" if tag == "a" => out.push(("id", v)),
                "id" => out.push(("id", v)),
                _ => out.push((k, v)),
            }
        }
        out
    }
    /// A link's target: a web or mail address as written, `#anchor` within the page, else the file it names (absolute), when it is in the root.
    fn href(&self, u: &str) -> Option<Vec<(&'static str, String)>> {
        let u = u.trim();
        if u.is_empty() || !html::safe_url(u) { return None }
        if u.starts_with('#') { return Some(vec![("href", u.to_string()), ("file", "0".into())]) }
        let l = u.to_ascii_lowercase();
        if l.starts_with("http://") || l.starts_with("https://") || l.starts_with("mailto:") { return Some(vec![("href", u.to_string()), ("file", "0".into())]) }
        let p = self.local(u)?;
        Some(vec![("href", p), ("file", "1".into())])
    }
    /// A picture's source: a web address (a GitHub page of a file becomes the file's raw address), or the absolute path of a file in the root.
    fn src(&self, u: &str) -> Option<String> {
        let u = u.trim();
        if u.is_empty() || !html::safe_url(u) || u.starts_with('#') || u.to_ascii_lowercase().starts_with("mailto:") { return None }
        let l = u.to_ascii_lowercase();
        if l.starts_with("http://") || l.starts_with("https://") {
            if let Some(rest) = u.strip_prefix("https://github.com/") {
                let parts: Vec<&str> = rest.splitn(4, '/').collect();
                if parts.len() == 4 && parts[2] == "blob" && !rest.contains('?') { return Some(format!("https://raw.githubusercontent.com/{}/{}/{}", parts[0], parts[1], parts[3])) }
            }
            return Some(u.to_string())
        }
        self.local(u)
    }
    /// A relative address as an absolute path inside the root (its query and fragment dropped, %20 and the like decoded); None outside it.
    fn local(&self, u: &str) -> Option<String> {
        let path = u.split(['?', '#']).next().unwrap_or("");
        if path.is_empty() { return None }
        let path = pct_decode(path);
        let joined = if let Some(r) = path.strip_prefix('/') { self.base.root.join(r) } else { self.base.dir.join(&path) };
        let mut out = PathBuf::new();
        for c in joined.components() {
            match c { Component::ParentDir => { if !out.pop() { return None } } Component::CurDir => {} o => out.push(o) }
        }
        let root = normal(&self.base.root);
        if !out.starts_with(&root) { return None }
        Some(out.to_string_lossy().into_owned())
    }
    fn slug(&mut self, text: &str) -> String {
        let base = slug_part(text);
        let n = self.slugs.entry(base.clone()).or_insert(0);
        let s = if *n == 0 { base.clone() } else { format!("{base}-{n}") };
        *n += 1;
        s
    }

    fn finish(mut self) -> Vec<Node> {
        while self.stack.len() > 1 { self.pop() }
        let mut out = std::mem::take(&mut self.stack[0].kids);
        // footnotes, numbered by first reference, at the end (as GitHub puts them), each with a way back
        if !self.notes.is_empty() {
            let mut defs: Vec<(usize, String, Vec<Node>)> = std::mem::take(&mut self.defs).into_iter().filter_map(|(l, k)| self.notes.get(&l).map(|n| (*n, l, k))).collect();
            defs.sort_by_key(|d| d.0);
            let items: Vec<Node> = defs.into_iter().map(|(n, l, mut kids)| {
                let back = Node::El { tag: "a", attrs: vec![("href", format!("#fnref-{}", slug_part(&l))), ("file", "0".into())], kids: vec![Node::Txt("↩".into())] };
                match kids.last_mut() { Some(Node::El { tag: "p", kids: k, .. }) => { k.push(Node::Txt(" ".into())); k.push(back) } _ => kids.push(back) }
                Node::El { tag: "li", attrs: vec![("value", n.to_string()), ("id", format!("fn-{}", slug_part(&l)))], kids }
            }).collect();
            if !items.is_empty() { out.push(Node::El { tag: "hr", attrs: vec![], kids: vec![] }); out.push(Node::El { tag: "ol", attrs: vec![("class", "footnotes".into())], kids: items }); }
        }
        tidy(&mut out, true, false);
        out
    }
}

fn normal(p: &Path) -> PathBuf { let mut out = PathBuf::new(); for c in p.components() { match c { Component::ParentDir => { out.pop(); } Component::CurDir => {} o => out.push(o) } } out }
fn pct_decode(s: &str) -> String {
    let b = s.as_bytes(); let mut out = Vec::with_capacity(b.len()); let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 0 && i + 2 <= b.len() - 1 { if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) { out.push(v); i += 3; continue } }
        out.push(b[i]); i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}
fn plain(n: &Node, out: &mut String) { match n { Node::Txt(t) => out.push_str(t), Node::El { kids, .. } => for k in kids { plain(k, out) } } }

/// GitHub's heading anchor: lowercase, letters, digits, `-` and `_` kept, spaces to `-`, everything else dropped.
pub fn slug_part(text: &str) -> String {
    text.trim().chars().filter_map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { Some(c.to_lowercase().collect::<String>()) } else if c == ' ' { Some("-".into()) } else { None }).collect()
}

/// HTML whitespace: runs of it are one space; at the start and end of a block, and between blocks, none. Code keeps its own.
fn tidy(kids: &mut Vec<Node>, block: bool, pre: bool) {
    for k in kids.iter_mut() {
        if let Node::El { tag, kids: inner, .. } = k { let p = pre || *tag == "pre"; tidy(inner, is_block(tag) || *tag == "a" && false, p) }
    }
    if pre { return }
    for k in kids.iter_mut() { if let Node::Txt(t) = k { *t = collapse(t) } }
    // whitespace beside a block element, or at a block's edges, is no text
    let n = kids.len();
    let blockish = |k: &Node| matches!(k, Node::El { tag, .. } if is_block(tag));
    let flags: Vec<bool> = kids.iter().map(blockish).collect();
    let mut keep = vec![true; n];
    for i in 0..n {
        if let Node::Txt(t) = &kids[i] {
            if t.trim().is_empty() && ((block && (i == 0 || i == n - 1)) || (i > 0 && flags[i - 1]) || (i + 1 < n && flags[i + 1]) || t.is_empty()) { keep[i] = false }
        }
    }
    let mut i = 0; kids.retain(|_| { let k = keep[i]; i += 1; k });
    if block {
        if let Some(Node::Txt(t)) = kids.first_mut() { *t = t.trim_start().to_string() }
        if let Some(Node::Txt(t)) = kids.last_mut() { *t = t.trim_end().to_string() }
        kids.retain(|k| !matches!(k, Node::Txt(t) if t.is_empty()));
    }
}
fn collapse(t: &str) -> String {
    let mut out = String::with_capacity(t.len()); let mut sp = false;
    for c in t.chars() { if c == ' ' || c == '\n' || c == '\t' || c == '\r' { if !sp { out.push(' ') } sp = true } else { out.push(c); sp = false } }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> Base { Base { dir: PathBuf::from("/p/docs"), root: PathBuf::from("/p") } }
    fn j(s: &str) -> String { serde_json::to_string(&super::super::nodes_json(&render(s, &base()))).unwrap() }
    #[test] fn a_centred_html_picture_is_a_picture_beside_the_readme() {
        let o = j("## Day 1\n\nWi-Fi\n\n<p align=\"center\">\n  <img src=\"img/day1.png\" alt=\"Day 1\" title=\"day1.png\" width=\"300\">\n</p>\n\nnext");
        assert!(o.contains(r#"{"tag":"h2","kids":[{"text":"Day 1"}],"id":"day-1"}"#), "{o}");
        assert!(o.contains(r#"{"tag":"p","kids":[{"tag":"img","kids":[],"src":"/p/docs/img/day1.png","alt":"Day 1","title":"day1.png","width":"300"}],"align":"center"}"#), "{o}");
        assert!(!o.contains("&lt;") && !o.contains("<p"), "no raw HTML left: {o}");
        assert!(o.ends_with(r#"{"tag":"p","kids":[{"text":"next"}]}]"#), "{o}");
    }
    #[test] fn markdown_inside_details_is_markdown() {
        let o = j("<details>\n<summary>More</summary>\n\nSome **bold**\n\n- a\n\n</details>\n\nafter");
        assert!(o.starts_with(r#"[{"tag":"details","kids":[{"tag":"summary","kids":[{"text":"More"}]},{"tag":"p","kids":[{"text":"Some "},{"tag":"strong","kids":[{"text":"bold"}]}]},{"tag":"ul""#), "{o}");
        assert!(o.ends_with(r#"{"tag":"p","kids":[{"text":"after"}]}]"#), "{o}");
    }
    #[test] fn gfm_tables_tasks_alerts_strike_and_code() {
        let o = j("| a | b |\n|:-:|--:|\n| 1 | ~~2~~ |\n\n- [x] done\n- [ ] todo\n\n> [!WARNING]\n> Hot\n\n```rust\nfn x() {}\n```");
        assert!(o.contains(r#"{"tag":"th","kids":[{"text":"a"}],"align":"center"}"#), "{o}");
        assert!(o.contains(r#"{"tag":"td","kids":[{"tag":"del","kids":[{"text":"2"}]}],"align":"right"}"#), "{o}");
        assert!(o.contains(r#""checked":"true""#) && o.contains(r#""checked":"false""#), "{o}");
        assert!(o.contains(r#"{"tag":"blockquote","kids":[{"tag":"p","kids":[{"text":"Hot"}]}],"alert":"warning"}"#), "{o}");
        assert!(o.contains(r#"{"tag":"pre","kids":[{"text":"fn x() {}\n"}],"lang":"rust"}"#), "{o}");
    }
    #[test] fn footnotes_are_numbered_and_gathered_at_the_end() {
        let o = j("A[^x] and B[^y].\n\n[^y]: Why.\n[^x]: Ex.\n");
        assert!(o.contains(r##"{"tag":"sup","kids":[{"tag":"a","kids":[{"text":"1"}],"href":"#fn-x","file":false,"id":"fnref-x"}]}"##), "{o}");
        let tail = &o[o.find(r#"{"tag":"hr""#).unwrap()..];
        assert!(tail.find("Ex.").unwrap() < tail.find("Why.").unwrap(), "{tail}");
    }
    #[test] fn links_resolve_and_unsafe_ones_go() {
        let o = j("[a](../src/main.rs#L3) [b](https://x.io) [c](#usage) [d](javascript:alert(1)) [e](../../etc/passwd) ![i](https://github.com/o/r/blob/main/a.png)");
        assert!(o.contains(r#""href":"/p/src/main.rs","file":true"#), "{o}");
        assert!(o.contains(r##""href":"https://x.io","file":false"##) && o.contains(r##""href":"#usage","file":false"##), "{o}");
        assert!(o.contains(r#"{"tag":"a","kids":[{"text":"d"}]}"#) && o.contains(r#"{"tag":"a","kids":[{"text":"e"}]}"#), "{o}");
        assert!(o.contains(r#""src":"https://raw.githubusercontent.com/o/r/main/a.png","alt":"i""#), "{o}");
    }
    #[test] fn headings_get_github_anchors_and_repeats_are_numbered() {
        let o = j("# Hello, World!\n# Hello, World!\n## `x` y_z");
        assert!(o.contains(r#""id":"hello-world""#) && o.contains(r#""id":"hello-world-1""#) && o.contains(r#""id":"x-y_z""#), "{o}");
    }
    #[test] fn picture_badges_kbd_sub_and_breaks() {
        let o = j("<picture><source media=\"(prefers-color-scheme: dark)\" srcset=\"d.png\"><img src=\"l.png\"></picture>\n\nPress <kbd>Ctrl</kbd>+C, H<sub>2</sub>O<br>x\n\n[![CI](https://img.shields.io/x.svg)](https://ci.io)");
        assert!(o.contains(r#"{"tag":"picture","kids":[{"tag":"source","kids":[],"media":"(prefers-color-scheme: dark)","src":"/p/docs/d.png"},{"tag":"img","kids":[],"src":"/p/docs/l.png"}]}"#), "{o}");
        assert!(o.contains(r#"{"tag":"kbd","kids":[{"text":"Ctrl"}]}"#) && o.contains(r#"{"tag":"sub","kids":[{"text":"2"}]}"#) && o.contains(r#"{"tag":"br","kids":[]}"#), "{o}");
        assert!(o.contains(r#"{"tag":"a","kids":[{"tag":"img","kids":[],"src":"https://img.shields.io/x.svg","alt":"CI"}],"href":"https://ci.io","file":false}"#), "{o}");
    }
    #[test] fn scripts_never_reach_the_nodes() {
        let o = j("<script>alert(1)</script>\n\nok <img src=x onerror=alert(1)> <a href=\"javascript:x\">y</a>");
        assert!(!o.contains("alert") && !o.contains("javascript"), "{o}");
    }
    #[test] fn an_unclosed_html_block_closes_at_the_end_and_a_stray_close_is_dropped() {
        let o = j("<div align=\"center\">\n\n# T\n\n</p></span>");
        assert!(o.starts_with(r#"[{"tag":"div","kids":[{"tag":"h1""#), "{o}");
    }
    #[test] fn a_readme_html_is_drawn_too() {
        let o = serde_json::to_string(&super::super::nodes_json(&render_html("<h1>T</h1><p>a <b>b</b></p>", &base()))).unwrap();
        assert_eq!(o, r#"[{"tag":"h1","kids":[{"text":"T"}],"id":"t"},{"tag":"p","kids":[{"text":"a "},{"tag":"strong","kids":[{"text":"b"}]}]}]"#);
    }
}

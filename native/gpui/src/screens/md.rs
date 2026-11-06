//! The hub's Markdown nodes (`Md.render`, sent to the phones as {tag, kids|text, href, file, ref, value}) as a small
//! document the timeline draws: styled runs, lists, quotes, tables, code. Pure; no decision about content, only the
//! shape the nodes already have. Also the plain text of every block, for selection and copy.
use serde_json::Value;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Style { pub bold: bool, pub italic: bool, pub code: bool, pub link: Option<Link> }
#[derive(Clone, Debug, PartialEq)]
pub struct Link { pub href: String, /// a path on the hub's machine, not a web address
    pub file: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Span { pub text: String, pub style: Style }

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Para(Vec<Span>),
    Heading(u8, Vec<Span>),
    List { ordered: bool, start: u64, items: Vec<Vec<Span>> },
    Quote(Vec<Span>),
    /// The code and its highlight tokens, computed once when the block is parsed.
    Code(String, Vec<(Range<usize>, Tok)>),
    Table { head: Vec<Vec<Span>>, rows: Vec<Vec<Vec<Span>>> },
    Rule,
}

const MAX_DEPTH: usize = 16;

/// A person's own text (their message) cut into spans with web addresses and absolute file paths as links, as the other clients do
/// (links.bend: `http(s)://…`, paths from `/` or `~/`, a trailing `.,;:!?)` left out, an optional `:12` kept). Pure.
pub fn linkify(text: &str) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new(); let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Span>| if !plain.is_empty() { out.push(Span { text: std::mem::take(plain), style: Style::default() }); };
    let b = text.as_bytes(); let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let at_word = i == 0 || b[i - 1].is_ascii_whitespace() || matches!(b[i - 1], b'(' | b'[' | b'"' | b'\'' | b'`' | b'<');
        let kind = if !at_word { 0 } else if rest.starts_with("http://") || rest.starts_with("https://") { 1 } else if (rest.starts_with('/') && rest.len() > 1 && !rest[1..].starts_with(|c: char| c.is_whitespace() || c == '/')) || rest.starts_with("~/") { 2 } else { 0 };
        if kind == 0 { let ch = rest.chars().next().unwrap(); plain.push(ch); i += ch.len_utf8(); continue; }
        let end = rest.find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`' | '\'')).unwrap_or(rest.len());
        let mut word = &rest[..end];
        while word.ends_with(|c: char| matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']')) { word = &word[..word.len() - 1]; }
        let min = if kind == 1 { 9 } else { 3 };
        if word.len() < min || (kind == 2 && !word[1..].contains(|c: char| c.is_alphanumeric())) { let ch = rest.chars().next().unwrap(); plain.push(ch); i += ch.len_utf8(); continue; }
        flush(&mut plain, &mut out);
        out.push(Span { text: word.to_string(), style: Style { link: Some(Link { href: word.to_string(), file: kind == 2 }), ..Style::default() } });
        i += word.len();
    }
    flush(&mut plain, &mut out);
    out
}

/// Where the whole words of `words` sit in `s`, as byte ranges in order: a word starts after a space or an opening bracket or quote, and ends at the end, a space or punctuation.
pub fn word_ranges(s: &str, words: &[String]) -> Vec<(usize, usize)> {
    let opens = |c: char| c.is_whitespace() || matches!(c, '(' | '[' | '"' | '\'');
    let mut out: Vec<(usize, usize)> = vec![]; let mut at = 0;
    while at < s.len() {
        let hit = words.iter().filter(|w| !w.is_empty()).filter_map(|w| { let mut from = at;
            while let Some(i) = s[from..].find(w.as_str()) { let i = from + i; let end = i + w.len();
                let before = i == 0 || s[..i].ends_with(opens);
                let after = s[end..].chars().next().map_or(true, |c| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '/' | ':' | '+')) && !(c == '.' && s[end + 1..].chars().next().map_or(false, |d| d.is_alphanumeric())));
                if before && after { return Some((i, end)) } from = i + 1; }
            None }).min_by_key(|(i, e)| (*i, std::cmp::Reverse(*e)));
        match hit { Some((i, e)) => { out.push((i, e)); at = e; } None => break }
    }
    out
}

/// The reference words of a message (`&claude/opus`, `@bot`...) set apart as chips: each whole word of `words` in a plain span becomes a code-styled span of its own.
pub fn chips(spans: Vec<Span>, words: &[String]) -> Vec<Span> {
    if words.is_empty() { return spans }
    let mut out = Vec::new();
    for s in spans {
        if s.style != Style::default() { out.push(s); continue }
        let mut at = 0;
        for (i, e) in word_ranges(&s.text, words) {
            if i > at { out.push(Span { text: s.text[at..i].to_string(), style: Style::default() }); }
            out.push(Span { text: s.text[i..e].to_string(), style: Style { code: true, ..Style::default() } }); at = e;
        }
        if at < s.text.len() { out.push(Span { text: s.text[at..].to_string(), style: Style::default() }); }
    }
    out
}

fn inline(node: &Value, style: &Style, out: &mut Vec<Span>, depth: usize) {
    if depth > MAX_DEPTH { return; }
    if let Some(t) = node["text"].as_str() { push(out, t, style); return; }
    let mut s = style.clone();
    match node["tag"].as_str().unwrap_or("") {
        "strong" | "b" => s.bold = true,
        "em" | "i" => s.italic = true,
        "code" => s.code = true,
        "a" => if let Some(h) = node["href"].as_str().or_else(|| node["ref"].as_str()) { s.link = Some(Link { href: h.to_string(), file: node["file"].as_bool().unwrap_or(false) }); },
        "br" => { push(out, "\n", style); return; }
        _ => {}
    }
    for k in node["kids"].as_array().into_iter().flatten() { inline(k, &s, out, depth + 1); }
}
fn push(out: &mut Vec<Span>, text: &str, style: &Style) {
    if text.is_empty() { return; }
    match out.last_mut() { Some(l) if l.style == *style => l.text.push_str(text), _ => out.push(Span { text: text.to_string(), style: style.clone() }) }
}
pub fn spans(nodes: &[Value]) -> Vec<Span> { let mut o = Vec::new(); for n in nodes { inline(n, &Style::default(), &mut o, 0); } o }
fn kids(n: &Value) -> &[Value] { n["kids"].as_array().map_or(&[], |v| v.as_slice()) }
fn text_of(n: &Value, out: &mut String, depth: usize) {
    if depth > MAX_DEPTH { return; }
    if let Some(t) = n["text"].as_str() { out.push_str(t); }
    for k in kids(n) { text_of(k, out, depth + 1); }
}

fn blocks_into(n: &Value, out: &mut Vec<Block>, depth: usize) {
    if depth > MAX_DEPTH { return; }
    let tag = n["tag"].as_str().unwrap_or("");
    match tag {
        "" => { let s = spans(std::slice::from_ref(n)); if !s.is_empty() { out.push(Block::Para(s)); } }
        "p" => out.push(Block::Para(spans(kids(n)))),
        t if t.len() == 2 && t.starts_with('h') && t.as_bytes()[1].is_ascii_digit() => out.push(Block::Heading(t.as_bytes()[1] - b'0', spans(kids(n)))),
        "ul" | "ol" => {
            let start = kids(n).first().and_then(|li| li["value"].as_u64()).unwrap_or(1);
            out.push(Block::List { ordered: tag == "ol", start, items: kids(n).iter().map(|li| spans(kids(li))).collect() });
        }
        "blockquote" => out.push(Block::Quote(spans(kids(n)))),
        "pre" => { let mut t = String::new(); text_of(n, &mut t, 0); let t = t.trim_end_matches('\n').to_string(); let tokens = highlight(&t); out.push(Block::Code(t, tokens)); }
        "hr" => out.push(Block::Rule),
        "table" => {
            let rows: Vec<Vec<Vec<Span>>> = kids(n).iter().map(|tr| kids(tr).iter().map(|c| spans(kids(c))).collect()).collect();
            let headed = kids(n).first().map_or(false, |tr| kids(tr).iter().any(|c| c["tag"] == "th"));
            let mut it = rows.into_iter();
            let head = if headed { it.next().unwrap_or_default() } else { Vec::new() };
            out.push(Block::Table { head, rows: it.collect() });
        }
        // the copy button of a fenced block carries no text of its own; its sibling `pre` is the code
        "button" => {}
        _ => { if kids(n).iter().any(|k| k["tag"].is_string() && k["tag"] != "a" && k["tag"] != "span" && k["tag"] != "code" && k["tag"] != "strong" && k["tag"] != "em") { for k in kids(n) { blocks_into(k, out, depth + 1); } } else { let s = spans(std::slice::from_ref(n)); if !s.is_empty() { out.push(Block::Para(s)); } } }
    }
}
pub const MAX_BLOCKS: usize = 400;
/// At most MAX_BLOCKS blocks; the rest are said, not hidden.
pub fn parse(nodes: &[Value]) -> Vec<Block> {
    let mut o = Vec::new();
    for n in nodes.iter().take(MAX_BLOCKS) { blocks_into(n, &mut o, 0); }
    if nodes.len() > MAX_BLOCKS { o.push(Block::Para(vec![Span { text: format!("… {} more blocks", nodes.len() - MAX_BLOCKS), style: Style { italic: true, ..Default::default() } }])); }
    o
}

pub fn plain(spans: &[Span]) -> String { spans.iter().map(|s| s.text.as_str()).collect() }
/// The text of a block as copied: one line per item or row, tabs between cells.
pub fn block_text(b: &Block) -> String {
    match b {
        Block::Para(s) | Block::Heading(_, s) | Block::Quote(s) => plain(s),
        Block::List { ordered, start, items } => items.iter().enumerate().map(|(i, it)| if *ordered { format!("{}. {}", start + i as u64, plain(it)) } else { format!("- {}", plain(it)) }).collect::<Vec<_>>().join("\n"),
        Block::Code(t, _) => t.clone(),
        Block::Table { head, rows } => {
            let line = |r: &Vec<Vec<Span>>| r.iter().map(|c| plain(c)).collect::<Vec<_>>().join("\t");
            let mut lines: Vec<String> = Vec::new();
            if !head.is_empty() { lines.push(line(head)); }
            lines.extend(rows.iter().map(line));
            lines.join("\n")
        }
        Block::Rule => String::new(),
    }
}

// ---- code highlighting (the chat has no language tag; a plain scan of what any language has)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tok { Plain, Keyword, Str, Comment, Number }
const KEYWORDS: &[&str] = &["fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl", "trait", "match", "if", "else", "for", "while", "loop", "return", "const", "static", "async", "await", "def", "class", "import", "from", "as", "in", "not", "and", "or", "None", "True", "False", "function", "var", "export", "new", "this", "self", "true", "false", "null", "nil", "int", "void", "type", "interface", "try", "catch", "throw", "except", "with", "yield", "lambda", "case", "switch", "break", "continue", "package", "func", "go", "defer", "select", "range", "char", "bool", "string"];

/// Token ranges (byte offsets) covering the text in order; consecutive same-kind runs are merged.
pub fn highlight(text: &str) -> Vec<(Range<usize>, Tok)> {
    let b = text.as_bytes(); let mut out: Vec<(Range<usize>, Tok)> = Vec::new(); let mut i = 0;
    let mut add = |r: Range<usize>, t: Tok| { if r.is_empty() { return; } match out.last_mut() { Some((lr, lt)) if *lt == t && lr.end == r.start => lr.end = r.end, _ => out.push((r, t)) } };
    while i < b.len() {
        let c = b[i];
        let rest = &text[i..];
        if rest.starts_with("//") || (c == b'#' && b.get(i + 1) != Some(&b'[') && (i == 0 || b[i - 1] == b'\n' || b[i - 1] == b' ')) || rest.starts_with("--") && (i == 0 || b[i - 1] == b'\n') {
            let e = rest.find('\n').map_or(text.len(), |k| i + k); add(i..e, Tok::Comment); i = e;
        } else if rest.starts_with("/*") {
            let e = rest.find("*/").map_or(text.len(), |k| i + k + 2); add(i..e, Tok::Comment); i = e;
        } else if c == b'"' || c == b'\'' || c == b'`' {
            let mut j = i + 1;
            while j < b.len() && b[j] != c && !(b[j] == b'\n' && c != b'`') { if b[j] == b'\\' { j += 1; } j += 1; }
            let e = (j + 1).min(text.len()); let e = (e..=text.len()).find(|k| text.is_char_boundary(*k)).unwrap_or(text.len());
            // a lone apostrophe in prose is not a string: only close on the same line
            add(i..e, Tok::Str); i = e;
        } else if c.is_ascii_digit() && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) {
            let mut j = i; while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'.' || b[j] == b'_') { j += 1; } add(i..j, Tok::Number); i = j;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let mut j = i; while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') { j += 1; }
            add(i..j, if KEYWORDS.contains(&&text[i..j]) { Tok::Keyword } else { Tok::Plain }); i = j;
        } else { let n = text[i..].chars().next().map_or(1, char::len_utf8); add(i..i + n, Tok::Plain); i += n; }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn j(s: &str) -> Vec<Value> { serde_json::from_str(s).unwrap() }
    #[test] fn inline_styles_nest_and_adjacent_equal_runs_merge() {
        let n = j(r#"[{"tag":"p","kids":[{"text":"a "},{"tag":"strong","kids":[{"text":"b"},{"tag":"em","kids":[{"text":"c"}]}]},{"text":" "},{"tag":"code","kids":[{"text":"d"}]}]}]"#);
        let b = parse(&n);
        let Block::Para(s) = &b[0] else { panic!() };
        assert_eq!(plain(s), "a bc d"); assert_eq!(s.len(), 5);
        assert!(s[1].style.bold && !s[1].style.italic && s[2].style.bold && s[2].style.italic && s[4].style.code);
    }
    #[test] fn links_keep_href_and_file_flag() {
        let b = parse(&j(r#"[{"tag":"p","kids":[{"tag":"a","href":"https://x.io","file":false,"kids":[{"text":"x"}]},{"tag":"a","href":"/tmp/a.rs","file":true,"kids":[{"text":"a.rs"}]}]}]"#));
        let Block::Para(s) = &b[0] else { panic!() };
        assert_eq!(s[0].style.link, Some(Link { href: "https://x.io".into(), file: false })); assert!(s[1].style.link.as_ref().unwrap().file);
    }
    #[test] fn lists_keep_their_numbers_and_quotes_and_rules() {
        let b = parse(&j(r#"[{"tag":"ol","kids":[{"tag":"li","value":3,"kids":[{"text":"three"}]},{"tag":"li","kids":[{"text":"four"}]}]},{"tag":"ul","kids":[{"tag":"li","kids":[{"text":"x"}]}]},{"tag":"blockquote","kids":[{"text":"q"}]},{"tag":"hr","kids":[]}]"#));
        assert_eq!(block_text(&b[0]), "3. three\n4. four"); assert_eq!(block_text(&b[1]), "- x"); assert_eq!(block_text(&b[2]), "q"); assert_eq!(b[3], Block::Rule);
    }
    #[test] fn a_fenced_block_is_its_code_without_the_copy_button_text() {
        let b = parse(&j(r#"[{"tag":"div","kids":[{"tag":"button","kids":[{"text":"Copy"}]},{"tag":"pre","kids":[{"tag":"code","kids":[{"text":"fn main() {}\n"}]}]}]}]"#));
        assert!(matches!(&b[..], [Block::Code(t, toks)] if t == "fn main() {}" && !toks.is_empty()));
    }
    #[test] fn tables_have_a_head_only_when_th_cells_exist() {
        let t = |cell: &str| format!(r#"[{{"tag":"table","kids":[{{"tag":"tr","kids":[{{"tag":"{cell}","kids":[{{"text":"A"}}]}},{{"tag":"{cell}","kids":[{{"text":"B"}}]}}]}},{{"tag":"tr","kids":[{{"tag":"td","kids":[{{"text":"1"}}]}},{{"tag":"td","kids":[{{"text":"2"}}]}}]}}]}}]"#);
        let Block::Table { head, rows } = &parse(&j(&t("th")))[0] else { panic!() }; assert_eq!((head.len(), rows.len()), (2, 1));
        let Block::Table { head, rows } = &parse(&j(&t("td")))[0] else { panic!() }; assert_eq!((head.len(), rows.len()), (0, 2));
        assert_eq!(block_text(&parse(&j(&t("th")))[0]), "A\tB\n1\t2");
    }
    #[test] fn too_many_blocks_say_how_many_were_left_out() {
        let nodes: Vec<Value> = (0..MAX_BLOCKS + 7).map(|i| serde_json::from_str(&format!(r#"{{"tag":"p","kids":[{{"text":"{i}"}}]}}"#)).unwrap()).collect();
        let b = parse(&nodes); assert_eq!(b.len(), MAX_BLOCKS + 1); assert_eq!(block_text(b.last().unwrap()), "… 7 more blocks");
    }
    #[test] fn headings_and_rtl_text_survive() {
        let b = parse(&j(r#"[{"tag":"h2","kids":[{"text":"שלום 世界"}]}]"#));
        assert_eq!(b, vec![Block::Heading(2, vec![Span { text: "שלום 世界".into(), style: Style::default() }])]);
    }
    #[test] fn highlight_covers_the_text_exactly_and_finds_the_kinds() {
        let code = "fn main() { let s = \"hi\"; // note\n  let n = 42; }\n";
        let h = highlight(code);
        assert_eq!(h.first().unwrap().0.start, 0); assert_eq!(h.last().unwrap().0.end, code.len());
        assert!(h.windows(2).all(|w| w[0].0.end == w[1].0.start));
        let kind = |needle: &str| { let at = code.find(needle).unwrap(); h.iter().find(|(r, _)| r.contains(&at)).unwrap().1 };
        assert_eq!(kind("fn"), Tok::Keyword); assert_eq!(kind("\"hi\""), Tok::Str); assert_eq!(kind("// note"), Tok::Comment); assert_eq!(kind("42"), Tok::Number); assert_eq!(kind("main"), Tok::Plain);
    }
    #[test] fn highlight_survives_unicode_and_unclosed_strings() {
        for code in ["let s = \"unterminated 世界", "# 見出し\nx = 'é'", "/* open", "日本語 ok 12abc"] { let h = highlight(code); assert_eq!(h.last().unwrap().0.end, code.len(), "{code}"); }
    }
}

#[cfg(test)]
mod link_tests {
    use super::*; use core::prelude::v1::test;
    fn links(t: &str) -> Vec<(String, bool)> { linkify(t).into_iter().filter_map(|s| s.style.link.map(|l| (l.href, l.file))).collect() }
    #[test] fn web_addresses_and_paths_become_links_without_trailing_punctuation() {
        assert_eq!(links("see https://example.com/a?b=1, then /home/h/x.rs:12. ok"), vec![("https://example.com/a?b=1".to_string(), false), ("/home/h/x.rs:12".to_string(), true)]);
        assert_eq!(links("(~/notes/a.md)"), vec![("~/notes/a.md".to_string(), true)]);
    }
    #[test] fn plain_words_slashes_and_mid_word_urls_are_not_links() {
        assert!(links("and/or a / b // c http://").is_empty()); assert!(links("xhttps://example.com").is_empty());
    }
    #[test] fn the_spans_keep_every_character() { let t = "a https://x.io/p, b /tmp/q é"; assert_eq!(linkify(t).iter().map(|s| s.text.as_str()).collect::<String>(), t); }
}


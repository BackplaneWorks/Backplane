//! src/core/links in Rust: the links in a text (web addresses, markdown links, file paths, Backplane refs and bare ids).
//! keeps its scan buffers reversed; here they are forward `Vec<char>`, so every "append to the reversed text"
//! reads as a push and `reverse` calls disappear. Each function names def it ports.
use super::refs::{self, Ref};

#[derive(Clone, Debug, PartialEq)]
pub enum Piece { Text(String), Link(String, String), Ref(String, String, bool) }

type S = Vec<char>;
fn st(s: &[char]) -> String { s.iter().collect() }
fn sv(s: &str) -> S { s.chars().collect() }
fn starts(s: &[char], p: &str) -> bool { let p: Vec<char> = p.chars().collect(); s.len() >= p.len() && s[..p.len()] == p[..] }

pub fn is_web(s: &str) -> bool { s.starts_with("http://") || s.starts_with("https://") }
fn valid(s: &str) -> bool { is_web(s) && s != "http://" && s != "https://" }

fn pchar(c: char) -> bool { c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '~' | '+' | '@' | ':' | '#') }

/// Link.part: up to a line (:) or an anchor (#)
fn part(s: &str) -> String { s.chars().take_while(|&c| c != ':' && c != '#').collect() }

/// Link.ext: a dot then a letter
fn ext(s: &str) -> bool { let cs: Vec<char> = s.chars().collect(); cs.windows(2).any(|w| w[0] == '.' && w[1].is_ascii_alphabetic()) }

/// Link.sfx: what may follow the path
fn sfx(x: &str) -> bool { x.is_empty() || x.starts_with("#L") || (x.starts_with(':') && x.chars().nth(1).map_or(false, |c| c.is_ascii_digit())) }

fn last(p: &str) -> &str { p.rsplit('/').next().unwrap_or("") }

fn shape(p: &str) -> bool {
    p.starts_with("~/") || p.starts_with("./") || p.starts_with("../") || (p.starts_with('/') && p[1..].contains('/')) || ext(last(p))
}

fn ref_ok(p: &str, sfx_: &str, loose: bool) -> bool {
    sfx(sfx_) && !p.ends_with('.') && !p.starts_with("//") && ((p.contains('/') && shape(p)) || (loose && ext(p)))
}

/// Link.ref
pub fn is_ref(s: &str, loose: bool) -> bool {
    if !s.chars().all(pchar) || s.contains("://") { return false }
    let p = part(s); let n = p.chars().count();
    ref_ok(&p, &s.chars().skip(n).collect::<String>(), loose)
}

/// Link.is_path
pub fn is_path(s: &str) -> bool { is_ref(s, false) }

/// The file kinds the viewer draws, by name (present/filelink.rs opens them inside Backplane): board, schematic, project, model, gerber, or "" for none.
pub fn design_kind(name: &str) -> &'static str {
    let l = name.to_ascii_lowercase();
    let e = l.rsplit_once('.').map_or("", |(_, e)| e);
    // KiCad's inner copper layers: `.g1`…`.g30`
    let inner = e.strip_prefix('g').map_or(false, |n| !n.is_empty() && n.len() <= 2 && n.bytes().all(|b| b.is_ascii_digit()));
    match e {
        "kicad_pcb" => "board", "kicad_sch" => "schematic", "kicad_pro" => "project",
        "step" | "stp" | "glb" => "model",
        "gbr" | "gbrjob" | "gtl" | "gbl" | "gts" | "gbs" | "gto" | "gbo" | "gtp" | "gbp" | "gko" | "gm1" | "gml" | "drl" | "xln" => "gerber",
        _ if inner => "gerber",
        _ => "",
    }
}
/// A design file named alone (`main.kicad_sch`, `case.step:3`, no folder): a link too, the viewer draws it (present/filelink.rs).
pub fn is_design(s: &str) -> bool {
    if s.is_empty() || !s.chars().all(pchar) || s.contains("://") || s.starts_with("//") { return false }
    let p = part(s); let rest: String = s.chars().skip(p.chars().count()).collect();
    sfx(&rest) && !p.ends_with('/') && !design_kind(last(&p)).is_empty() && last(&p).split('.').next().map_or(false, |n| !n.is_empty())
}
/// A `file:///abs/path` address (a design file's or any): a link to that file.
pub fn file_url(s: &str) -> bool { s.strip_prefix("file://").map_or(false, |r| { let r = r.strip_prefix("localhost").unwrap_or(r); r.len() > 1 && r.starts_with('/') && !r.contains(char::is_whitespace) }) }

/// Link.target: a markdown target, file:// dropped
fn target(u: &str) -> String { u.strip_prefix("file://").unwrap_or(u).to_string() }

enum Scan { Text(S), Url(S, S), Lab(S, S), Mid(S, S), Href(S, S, S) }

fn text(acc: &[char], out: &mut Vec<Piece>) { if !acc.is_empty() { out.push(Piece::Text(st(acc))) } }
fn word_end(acc: &[char]) -> bool { acc.last().map_or(false, |c| c.is_ascii_alphanumeric()) }

/// Link.starts: h + ttp(s):// or b + ackplane://, not inside a word
fn starts_url(acc: &[char], t: &[char]) -> bool { (starts(t, "ttp://") || starts(t, "ttps://") || starts(t, "ackplane://")) && !word_end(acc) }

/// Link.text.step: one char of plain text
fn text_step(mut acc: S, out: &mut Vec<Piece>, c: char, t: &[char]) -> Scan {
    if (c == 'h' || c == 'b') && starts_url(&acc, t) { Scan::Url(acc, vec![c]) }
    else if c == '[' { Scan::Lab(acc, vec![]) }
    else { acc.push(c); let _ = out; Scan::Text(acc) }
}

fn stops(c: char) -> bool { c.is_ascii_whitespace() || c == '\u{b}' || matches!(c, '<' | '>' | '"' | '`') }
fn punct(c: char) -> bool { matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '\'' | ']') }

fn piece(m: Option<Ref>, label: String, url: String) -> Piece { match m { Some(r) => Piece::Ref(label, refs::uri(&r), false), None => Piece::Link(label, url) } }

/// Link.url_end: a bare address ended: trailing sentence punctuation (and a ) it did not open) goes back to the text
fn url_end(acc: S, u: S, out: &mut Vec<Piece>) -> S {
    // walk from the end, cutting while the char trims
    let mut keep = u.len();
    while keep > 0 {
        let c = u[keep - 1];
        let pre = &u[..keep];
        let cut = punct(c) || (c == ')' && pre.iter().filter(|&&x| x == ')').count() > pre.iter().filter(|&&x| x == '(').count());
        if cut { keep -= 1 } else { break }
    }
    let url = st(&u[..keep]); let moved: S = u[keep..].to_vec();
    if valid(&url) || refs::parse(&url).is_some() {
        text(&acc, out); out.push(piece(refs::parse(&url), url.clone(), url)); moved
    } else { let mut a = acc; a.extend_from_slice(&u); a }
}

fn bang(acc: &[char]) -> bool { acc.last() == Some(&'!') }

fn href_ok(u: &str, acc: &[char]) -> bool {
    if refs::parse(u).is_some() { !bang(acc) } else { valid(u) || (!bang(acc) && is_ref(&target(u), true)) }
}

fn lab_raw(acc: &[char], l: &[char]) -> S { let mut a = acc.to_vec(); a.push('['); a.extend_from_slice(l); a }
fn href_raw(acc: &[char], l: &[char], u: &[char]) -> S { let mut a = lab_raw(acc, l); a.push(']'); a.push('('); a.extend_from_slice(u); a }

/// Link.go: the scan over the whole text
fn scan(s: &[char]) -> Vec<Piece> {
    let mut out = Vec::new(); let mut sc = Scan::Text(vec![]);
    let mut i = 0;
    while i < s.len() {
        let c = s[i]; let t = &s[i + 1..];
        sc = match sc {
            Scan::Text(acc) => text_step(acc, &mut out, c, t),
            Scan::Url(acc, mut u) => {
                if stops(c) { let a2 = url_end(acc, u, &mut out); text_step(a2, &mut out, c, t) } else { u.push(c); Scan::Url(acc, u) }
            }
            Scan::Lab(acc, mut l) => {
                if c == ']' { Scan::Mid(acc, l) }
                else if c == '\n' || c == '[' { let a = lab_raw(&acc, &l); text_step(a, &mut out, c, t) }
                else { l.push(c); Scan::Lab(acc, l) }
            }
            Scan::Mid(acc, l) => {
                if c == '(' { Scan::Href(acc, l, vec![]) }
                else { let mut a = lab_raw(&acc, &l); a.push(']'); text_step(a, &mut out, c, t) }
            }
            Scan::Href(acc, l, mut u) => {
                if c == ')' {
                    let us = st(&u);
                    if href_ok(&us, &acc) {
                        text(&acc, &mut out);
                        let label = if l.is_empty() { us.clone() } else { st(&l) };
                        out.push(piece(refs::parse(&us), label, target(&us))); Scan::Text(vec![])
                    } else { let mut a = href_raw(&acc, &l, &u); a.push(')'); Scan::Text(a) }
                } else if stops(c) { let a = href_raw(&acc, &l, &u); text_step(a, &mut out, c, t) }
                else { u.push(c); Scan::Href(acc, l, u) }
            }
        };
        i += 1;
    }
    match sc {
        Scan::Text(acc) => text(&acc, &mut out),
        Scan::Url(acc, u) => { let a = url_end(acc, u, &mut out); text(&a, &mut out) }
        Scan::Lab(acc, l) => text(&lab_raw(&acc, &l), &mut out),
        Scan::Mid(acc, l) => { let mut a = lab_raw(&acc, &l); a.push(']'); text(&a, &mut out) }
        Scan::Href(acc, l, u) => text(&href_raw(&acc, &l, &u), &mut out),
    }
    out
}

// ---- bare paths and ids ----

struct Bw { txt: S, word: S, out: Vec<Piece>, image: u8, bang: bool }

fn wrap(c: char) -> bool { matches!(c, '(' | '[' | '"' | '\'' | '*' | '<' | '`') }
fn after(c: char) -> bool { punct(c) || matches!(c, '"' | ')' | '*' | '>' | '`') }
fn idchar(c: char) -> bool { c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '~') }

fn lead(w: &[char]) -> usize { w.iter().take_while(|&&c| wrap(c)).count() }
fn trail(w: &[char]) -> usize { w.iter().rev().take_while(|&&c| after(c)).count() }

/// Link.ids.part.flush: the word as a bare ref or back to text
fn ids_part(b: &mut Bw) {
    if b.word.is_empty() { return }
    let w = st(&b.word);
    match refs::bare(&w) { Some(r) => { text(&b.txt, &mut b.out); b.txt.clear(); b.out.push(Piece::Ref(w, refs::uri(&r), true)); }, None => b.txt.extend_from_slice(&b.word) }
    b.word.clear();
}

/// Link.ids.retry with a dot: try each dot-separated part by itself
fn ids_dots(b: &mut Bw, w: &[char]) {
    b.word.clear();
    for &c in w {
        if c == '.' { ids_part(b); b.txt.push('.') } else { b.word.push(c) }
    }
    ids_part(b);
}

/// Link.ids.test / end / miss / edge: a finished word that may be a bare id
fn ids_test(b: &mut Bw, muted: bool) {
    if b.word.is_empty() { return }
    let w = std::mem::take(&mut b.word);
    if muted { b.txt.extend_from_slice(&w); return }
    if let Some(r) = refs::bare(&st(&w)) { text(&b.txt, &mut b.out); b.txt.clear(); b.out.push(Piece::Ref(st(&w), refs::uri(&r), true)); return }
    if w.contains(&'.') {
        let tr = trail(&w);
        let stem = &w[..w.len() - tr];
        if let Some(r) = refs::bare(&st(stem)) {
            text(&b.txt, &mut b.out); b.txt = w[w.len() - tr..].to_vec();
            b.out.push(Piece::Ref(st(stem), refs::uri(&r), true));
        } else { b.txt_retry(&w) }
    } else { b.txt.extend_from_slice(&w) }
}
impl Bw { fn txt_retry(&mut self, w: &[char]) { ids_dots(self, w) } }

fn ids_push(b: &mut Bw, c: char) { b.txt.push(c); b.bang = false; }

fn ids_normal(b: &mut Bw, c: char, muted: bool) {
    if idchar(c) { b.word.push(c); b.bang = false }
    else {
        ids_test(b, muted);
        b.txt.push(c);
        if b.bang && c == '[' { b.image = 1 }
        b.bang = c == '!';
    }
}

fn ids_step(b: &mut Bw, c: char, muted: bool) {
    match b.image {
        0 => ids_normal(b, c, muted),
        1 => { ids_push(b, c); b.image = if c == ']' { 2 } else { 1 } }
        2 => if c == '(' { ids_push(b, c); b.image = 3 } else { b.image = 0; ids_normal(b, c, muted) }
        n => { ids_push(b, c); b.image = if c == ')' { 0 } else { n } }
    }
}

/// Link.ids.go over one word
fn ids_go(w: &[char], b: &mut Bw, muted: bool) {
    // call starts from the state with an empty word
    for &c in w { ids_step(b, c, muted) }
    ids_test(b, muted);
}

/// Link.bare.flush / word / core / hit
fn bare_flush(b: &mut Bw) {
    if b.word.is_empty() { return }
    let w = std::mem::take(&mut b.word);
    if w.contains(&'/') || w.contains(&'.') {
        let (l, t) = (lead(&w), trail(&w));
        let n = w.len();
        let mid_len = n.saturating_sub(l).saturating_sub(t);
        let core: S = w[l.min(n)..(l.min(n) + mid_len).min(n)].to_vec();
        let cs = st(&core);
        // a path, a design file named alone, or a file:// address
        if is_path(&cs) || is_design(&cs) || file_url(&cs) {
            let mut txt = std::mem::take(&mut b.txt); txt.extend_from_slice(&w[..l]);
            text(&txt, &mut b.out);
            let cs = st(&core); b.out.push(Piece::Link(cs.clone(), cs));
            b.txt = w[(l + core.len()).min(n)..].to_vec(); b.bang = false;
            return;
        }
    }
    let muted = st(&w).contains("://") && b.image == 0 && !st(&w).contains("![");
    ids_go(&w, b, muted);
}

fn bare_text(s: &str) -> Vec<Piece> {
    if !(s.contains('/') || s.contains('-') || s.split_whitespace().any(|w| is_design(w.trim_matches(|c: char| wrap(c) || after(c))))) { return vec![Piece::Text(s.to_string())] }
    let mut b = Bw { txt: vec![], word: vec![], out: vec![], image: 0, bang: false };
    for c in s.chars() {
        if c.is_ascii_whitespace() || c == '\u{b}' {
            bare_flush(&mut b);
            b.txt.push(c); if b.image == 2 { b.image = 0 } b.bang = false;
        } else { b.word.push(c) }
    }
    bare_flush(&mut b);
    text(&b.txt, &mut b.out);
    b.out
}

/// Link.pieces: text as runs of plain text and links, in order
pub fn pieces(s: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    for p in scan(&sv(s)) { match p { Piece::Text(t) => out.extend(bare_text(&t)), o => out.push(o) } }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn t(s: &str) -> Vec<Piece> { pieces(s) }
    #[test] fn urls_trim_sentence_punctuation() {
        assert_eq!(t("see https://a.b/c, ok"), vec![Piece::Text("see ".into()), Piece::Link("https://a.b/c".into(), "https://a.b/c".into()), Piece::Text(", ok".into())]);
    }
    #[test] fn markdown_links() { assert_eq!(t("[x](https://a.b)"), vec![Piece::Link("x".into(), "https://a.b".into())]); assert_eq!(t("![x](a.png)"), vec![Piece::Text("![x](a.png)".into())]); }
    #[test] fn paths() { assert_eq!(t("open src/main.rs now"), vec![Piece::Text("open ".into()), Piece::Link("src/main.rs".into(), "src/main.rs".into()), Piece::Text(" now".into())]); }
    #[test] fn design_files_named_alone_and_file_addresses_are_links() {
        assert_eq!(t("see main.kicad_sch now"), vec![Piece::Text("see ".into()), Piece::Link("main.kicad_sch".into(), "main.kicad_sch".into()), Piece::Text(" now".into())]);
        assert_eq!(t("(case.STEP)."), vec![Piece::Text("(".into()), Piece::Link("case.STEP".into(), "case.STEP".into()), Piece::Text(").".into())]);
        assert_eq!(t("open file:///home/h/a%20b.kicad_pcb"), vec![Piece::Text("open ".into()), Piece::Link("file:///home/h/a%20b.kicad_pcb".into(), "file:///home/h/a%20b.kicad_pcb".into())]);
        assert_eq!(t("[board](file:///w/a.kicad_pcb)"), vec![Piece::Link("board".into(), "/w/a.kicad_pcb".into())]);
        for plain in ["version 1.2 done", "e.g. this", "a .kicad_pcb file", "x.rs alone"] { assert!(t(plain).iter().all(|p| matches!(p, Piece::Text(_))), "{plain}: {:?}", t(plain)); }
    }
    #[test] fn bare_ids() { assert!(matches!(t("see t1700000000-1.")[1], Piece::Ref(..))); }
}

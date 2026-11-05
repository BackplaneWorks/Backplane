//! core/ref-origin.bend: the read projection of a mirrored message's text. Explicit `backplane://thread/<id>` and
//! `project/<id>` references are translated once, in the same direction as `far::sender`: an id of the mirroring
//! machine's own ("<link>~<id>") is local again, one of the owner's is named through the link, a foreign link's is
//! left. Code (inline and fenced, by backtick runs) is kept byte for byte, and so is everything else.
// (md/refs.rs is standalone; it is included by path until the md module is declared in main.rs)
#[path = "../md/refs.rs"]
#[allow(dead_code)]
mod refs;

/// Origin.id
pub fn id(i: &str, link: &str) -> String {
    if i.contains('~') {
        let pre = format!("{link}~");
        if i.starts_with(&pre) && i.chars().count() > 1 + link.chars().count() { i.chars().skip(1 + link.chars().count()).collect() } else { i.to_string() }
    } else { format!("{link}~{i}") }
}
/// Origin.uri: a thread or project reference with one id; any other reference, or text that is not one, is the original.
fn uri(u: &str, link: &str) -> String {
    match refs::parse(u) {
        Some(r) if r.rest.len() == 1 && (r.kind == "thread" || r.kind == "project") => {
            let next = id(&r.rest[0], link);
            if next == r.rest[0] { u.to_string() } else { refs::uri(&refs::Ref { kind: r.kind.clone(), rest: vec![next], frag: r.frag.clone() }) }
        }
        _ => u.to_string(),
    }
}
/// Link.stops: what ends a bare address (space, < > " `) and Origin.stops adds ( ) [ ] '.
fn stops(c: char) -> bool { c.is_ascii_whitespace() || c == '\u{b}' || matches!(c, '<' | '>' | '"' | '`' | '(' | ')' | '[' | ']' | '\'') }
/// Link.punct: what a sentence puts after an address.
fn punct(c: char) -> bool { matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '\'' | ']') }
/// Origin.flush: the translated address, then the punctuation that trailed it.
fn flush(token: &[char], link: &str, out: &mut Vec<char>) {
    let keep = token.len() - token.iter().rev().take_while(|c| punct(**c)).count();
    let url: String = token[..keep].iter().collect();
    out.extend(uri(&url, link).chars());
    out.extend(&token[keep..]);
}
/// Origin.mode: backtick runs fence code; a run of the same length closes it.
fn mode(code: usize, ticks: usize) -> usize { if ticks == 0 { code } else if code == 0 { ticks } else if code == ticks { 0 } else { code } }
/// Origin.boundary: a reference does not start inside a word, path, scheme or escape.
fn boundary(out: &[char]) -> bool { out.last().map_or(true, |c| !(c.is_ascii_alphanumeric() || matches!(c, '/' | ':' | '_' | '%'))) }

/// Origin.scan
fn scan(s: &[char], link: &str) -> String {
    let (mut out, mut token, mut code, mut ticks, mut url) = (Vec::with_capacity(s.len()), Vec::<char>::new(), 0usize, 0usize, false);
    for (i, &c) in s.iter().enumerate() {
        if c == '`' {
            if url { flush(&token, link, &mut out) }
            token.clear(); url = false; out.push(c); ticks += 1;
            continue;
        }
        let m = mode(code, ticks);
        if url {
            if stops(c) { flush(&token, link, &mut out); token.clear(); out.push(c); url = false } else { token.push(c) }
        } else if c == 'b' && m == 0 && boundary(&out) && s[i + 1..].starts_with(&"ackplane://".chars().collect::<Vec<_>>()) {
            token.clear(); token.push(c); url = true;
        } else { out.push(c) }
        code = m; ticks = 0;
    }
    if url { flush(&token, link, &mut out) }
    out.into_iter().collect()
}
/// Ref.link: a link id is a word (letters, digits, - . _ ~).
fn link_ok(l: &str) -> bool { !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~')) }
/// Origin.show
pub fn show(text: &str, link: &str) -> String {
    if link_ok(link) && text.contains("backplane://") { scan(&text.chars().collect::<Vec<_>>(), link) } else { text.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn ref_origin_no_link() { for s in ["", "see backplane://thread/t1", "x"] { assert_eq!(show(s, ""), s); } }
    #[test] fn ref_origin_return_local() { assert_eq!(id("peer~t", "peer"), "t"); }
    #[test] fn ref_origin_owner_remote() { assert_eq!(id("t", "peer"), "peer~t"); }
    #[test] fn ref_origin_foreign_untouched() { assert_eq!(id("other~t", "peer"), "other~t"); assert_eq!(id("peer~", "peer"), "peer~", "nothing after the link: not an id here"); }
    #[test] fn origin_translates_thread_and_project_references() {
        assert_eq!(show("see backplane://thread/t9 now", "L"), "see backplane://thread/L~t9 now");
        assert_eq!(show("see backplane://thread/L~t9 now", "L"), "see backplane://thread/t9 now");
        assert_eq!(show("backplane://thread/M~t9", "L"), "backplane://thread/M~t9");
        assert_eq!(show("backplane://project/p1.", "L"), "backplane://project/L~p1.", "trailing punctuation stays after the address");
        assert_eq!(show("(backplane://thread/t9)", "L"), "(backplane://thread/L~t9)", "parentheses delimit");
        assert_eq!(show("[x](backplane://thread/t9#m1)", "L"), "[x](backplane://thread/L~t9#m1)");
    }
    #[test] fn origin_leaves_other_references_and_words_alone() {
        for s in ["backplane://bot/b1", "backplane://room/r1", "abackplane://thread/t9", "x/backplane://thread/t9", "backplane://nope/x", "xbackplane://thread/t9 and :backplane://thread/t9"] {
            assert_eq!(show(s, "L"), s, "{s}");
        }
        assert_eq!(show("a_backplane://thread/t1", "L"), "a_backplane://thread/t1");
    }
    #[test] fn origin_keeps_code_byte_for_byte() {
        assert_eq!(show("`backplane://thread/t9` backplane://thread/t9", "L"), "`backplane://thread/t9` backplane://thread/L~t9");
        assert_eq!(show("```\nbackplane://thread/t9\n``` backplane://thread/t9", "L"), "```\nbackplane://thread/t9\n``` backplane://thread/L~t9");
        assert_eq!(show("``a ` backplane://thread/t9 `` backplane://thread/t9", "L"), "``a ` backplane://thread/t9 `` backplane://thread/L~t9", "a longer fence holds a single tick");
        assert_eq!(show("unclosed ` backplane://thread/t9", "L"), "unclosed ` backplane://thread/t9", "an open fence runs to the end");
    }
    #[test] fn origin_is_exact_on_text_without_references() {
        let s = "plain text with `ticks` and ünïcode, backplane:// alone and backplane://thread/ empty";
        assert_eq!(show(s, "L"), s);
        assert_eq!(show("backplane://thread/t9", "bad link"), "backplane://thread/t9", "a link that is not a word translates nothing");
    }
    #[test] fn origin_scales_linearly() {
        let t = "x ".repeat(200_000) + "backplane://thread/t1"; let start = std::time::Instant::now();
        assert!(show(&t, "L").ends_with("L~t1")); assert!(start.elapsed().as_secs() < 2);
    }
}

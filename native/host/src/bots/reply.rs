//! Reply blocks in a person's message: `<in_reply_to msg=ID chars=N>\nQUOTE\n</in_reply_to>\n\nBODY`.
const OPEN: &str = "<in_reply_to msg=";
const CLOSE: &str = "\n</in_reply_to>";
/// the most of a message a quote keeps
pub const MAX: usize = 2000;
fn cut(s: &str, sep: char) -> (&str, &str) { match s.split_once(sep) { Some((a, b)) => (a, b), None => (s, "") } }
fn drop_chars(s: &str, n: usize) -> &str { match s.char_indices().nth(n) { Some((i, _)) => &s[i..], None => "" } }
fn take_chars(s: &str, n: usize) -> &str { match s.char_indices().nth(n) { Some((i, _)) => &s[..i], None => s } }
/// Rto.body: up to two blank lines after the block belong to it
fn body(s: &str) -> &str { let one = |s: &'_ str| s.strip_prefix('\n').unwrap_or(s).to_string(); let _ = one; let s1 = s.strip_prefix('\n').unwrap_or(s); s1.strip_prefix('\n').unwrap_or(s1) }
/// Rto.of: (message replied to, quote, body) when the text starts with a whole block
pub fn of(text: &str) -> Option<(String, String, String)> {
    if !text.starts_with(OPEN) { return None }
    let (id, rest) = cut(drop_chars(text, OPEN.chars().count()), ' ');
    if id.is_empty() || !rest.starts_with("chars=") { return None }
    let (num, rest2) = cut(drop_chars(rest, 6), '>');
    if num.is_empty() || !num.bytes().all(|c| c.is_ascii_digit()) || !rest2.starts_with('\n') { return None }
    let n = num.bytes().fold(0usize, |a, c| a.saturating_mul(10).saturating_add((c - b'0') as usize));
    let r = &rest2[1..];
    let after = drop_chars(r, n);
    if !after.starts_with(CLOSE) { return None }
    Some((id.into(), take_chars(r, n).into(), body(drop_chars(after, CLOSE.chars().count())).into()))
}
/// Rto.text: what the person wrote (the text with no reply block)
pub fn text(t: &str) -> String { of(t).map_or_else(|| t.to_string(), |(_, _, b)| b) }
/// Rto.wrap: the block and the body, the quote cut to MAX (a reply to a reply quotes only what that one said)
pub fn wrap(to: &str, quote: &str, body: &str) -> String {
    let q = take_chars(&text(quote), MAX).to_string();
    format!("{OPEN}{to} chars={}>\n{q}{CLOSE}\n\n{body}", q.chars().count())
}
/// Rto.resolve: the text the hub keeps for a message sent in reply to `to`; `quoted` is that message's text when the thread has it
pub fn resolve(to: &str, quoted: Option<&str>, t: &str) -> String {
    if to.is_empty() { return t.to_string() }
    let body = text(t);
    match quoted { Some(q) => wrap(to, q, &body), None => body }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn rto_wrap_round_trips_and_counts_characters() {
        // laws rto_*: the quote's length is counted, never searched for
        let w = wrap("m1", "line1\n</in_reply_to>\nline3", "my answer");
        assert_eq!(of(&w), Some(("m1".into(), "line1\n</in_reply_to>\nline3".into(), "my answer".into())));
        assert_eq!(text(&w), "my answer"); assert_eq!(text("plain"), "plain");
        assert_eq!(wrap("m", "é€", "b"), "<in_reply_to msg=m chars=2>\né€\n</in_reply_to>\n\nb");
    }
    #[test] fn rto_resolve_quotes_the_message_or_keeps_the_text() {
        assert_eq!(resolve("", Some("q"), "t"), "t");
        assert_eq!(resolve("m", None, "t"), "t");
        assert_eq!(resolve("m", Some("quoted"), "t"), "<in_reply_to msg=m chars=6>\nquoted\n</in_reply_to>\n\nt");
        // a reply to a reply quotes only what that one said; a block already in the text is replaced
        let inner = resolve("a", Some("first"), "second");
        assert_eq!(resolve("b", Some(&inner), &inner), "<in_reply_to msg=b chars=6>\nsecond\n</in_reply_to>\n\nsecond");
        let long = "x".repeat(MAX + 5); assert_eq!(of(&wrap("m", &long, "b")).unwrap().1.chars().count(), MAX);
    }
}

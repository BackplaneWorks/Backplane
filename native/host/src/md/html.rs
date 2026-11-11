//! The HTML a README carries (GitHub's subset: `<p align>`, `<img>`, `<details>`, `<picture>`, `<br>`, `<sub>`, `<kbd>`...) cut into tokens and
//! sanitised: only the tags and attributes of `tag` and `attr` survive, the contents of `<script>`, `<style>`, `<iframe>`... are dropped whole,
//! any other tag is dropped and its text kept. Nothing here runs or fetches anything; the document builder (doc.rs) turns the tokens into nodes.

/// A token of HTML, its tag already mapped to the allowed name (`b` and `strong` stay apart, `strike` is `del`, `center` a centred `div`).
#[derive(Clone, Debug, PartialEq)]
pub enum Tok { Open(&'static str, Vec<(&'static str, String)>), Close(&'static str), Void(&'static str, Vec<(&'static str, String)>), Text(String) }

/// The allowed tags as the nodes name them; None for a tag that is dropped (its text kept).
pub fn tag(name: &str) -> Option<&'static str> {
    Some(match name {
        "p" => "p", "div" => "div", "center" => "div", "h1" => "h1", "h2" => "h2", "h3" => "h3", "h4" => "h4", "h5" => "h5", "h6" => "h6",
        "blockquote" => "blockquote", "pre" => "pre", "ul" => "ul", "ol" => "ol", "li" => "li", "table" => "table", "tr" => "tr", "th" => "th", "td" => "td",
        "hr" => "hr", "details" => "details", "summary" => "summary", "picture" => "picture", "source" => "source", "dl" => "div", "dt" => "dt", "dd" => "dd",
        "a" => "a", "img" => "img", "br" => "br", "strong" => "strong", "b" => "strong", "em" => "em", "i" => "em", "code" => "code", "tt" => "code", "samp" => "code",
        "kbd" => "kbd", "sub" => "sub", "sup" => "sup", "del" => "del", "s" => "del", "strike" => "del", "ins" => "ins", "u" => "ins", "mark" => "mark",
        _ => return None,
    })
}
/// Tags whose whole content is dropped, not only the tag.
fn hidden(name: &str) -> bool { matches!(name, "script" | "style" | "iframe" | "object" | "embed" | "noscript" | "template" | "textarea" | "select" | "button" | "form" | "svg" | "math" | "title" | "head" | "frameset" | "frame" | "applet" | "audio" | "video" | "canvas") }
pub fn void(t: &str) -> bool { matches!(t, "img" | "br" | "hr" | "source") }

/// The attributes a tag keeps, by its node name.
fn attr(t: &str, name: &str) -> Option<&'static str> {
    let a: &'static str = match name { "align" => "align", "src" => "src", "srcset" => "srcset", "alt" => "alt", "title" => "title", "width" => "width", "height" => "height",
        "href" => "href", "name" => "id", "id" => "id", "open" => "open", "media" => "media", "start" => "start", _ => return None };
    let ok = match t {
        "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "th" | "td" => matches!(a, "align" | "id"),
        "img" => matches!(a, "src" | "alt" | "title" | "width" | "height" | "align"),
        "a" => matches!(a, "href" | "title" | "id"),
        "source" => matches!(a, "srcset" | "media"),
        "details" => a == "open", "ol" => a == "start",
        _ => false,
    };
    ok.then_some(a)
}

/// A link or picture address that may be kept: a web address, a mail address, an anchor or a relative or absolute path; never `javascript:`,
/// `data:` or any other scheme.
pub fn safe_url(u: &str) -> bool {
    let l = u.trim().to_ascii_lowercase();
    if l.starts_with("http://") || l.starts_with("https://") || l.starts_with("mailto:") || l.starts_with('#') { return true }
    // a scheme is letters then ':' before any '/', '?' or '#'
    match l.find(':') { Some(i) => l[..i].contains(['/', '?', '#']), None => true }
}

/// Character references: the numeric ones and the names a README uses.
pub fn unescape(s: &str) -> String {
    if !s.contains('&') { return s.to_string() }
    let mut out = String::with_capacity(s.len()); let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]); rest = &rest[i..];
        let end = rest[1..].find(|c: char| !(c.is_ascii_alphanumeric() || c == '#')).map(|k| k + 1).unwrap_or(rest.len());
        let name = &rest[1..end];
        let ch: Option<String> = if let Some(n) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) { u32::from_str_radix(n, 16).ok().and_then(char::from_u32).map(String::from) }
            else if let Some(n) = name.strip_prefix('#') { n.parse::<u32>().ok().and_then(char::from_u32).map(String::from) }
            else { named(name).map(String::from) };
        match ch.filter(|c| c != "\0") {
            Some(c) => { out.push_str(&c); rest = &rest[end..]; if rest.starts_with(';') { rest = &rest[1..] } }
            None => { out.push('&'); rest = &rest[1..]; }
        }
    }
    out.push_str(rest);
    out
}
fn named(n: &str) -> Option<&'static str> {
    Some(match n { "amp" => "&", "lt" => "<", "gt" => ">", "quot" => "\"", "apos" => "'", "nbsp" => "\u{a0}", "copy" => "©", "reg" => "®", "trade" => "™", "mdash" => "—", "ndash" => "–",
        "hellip" => "…", "middot" => "·", "bull" => "•", "rarr" => "→", "larr" => "←", "uarr" => "↑", "darr" => "↓", "times" => "×", "deg" => "°", "plusmn" => "±", "micro" => "µ",
        "ohm" => "Ω", "Omega" => "Ω", "le" => "≤", "ge" => "≥", "ne" => "≠", "laquo" => "«", "raquo" => "»", "zwj" => "\u{200d}", "ensp" => "\u{2002}", "emsp" => "\u{2003}", "thinsp" => "\u{2009}", _ => return None })
}

/// The tokens of `s`. Comments, doctypes and processing instructions are dropped; a `<` that opens no tag is text.
pub fn tokens(s: &str) -> Vec<Tok> {
    let mut out = Vec::new(); let mut i = 0; let b = s.as_bytes(); let mut text = String::new();
    let flush = |text: &mut String, out: &mut Vec<Tok>| if !text.is_empty() { out.push(Tok::Text(unescape(&std::mem::take(text)))) };
    while i < s.len() {
        let rest = &s[i..];
        if !rest.starts_with('<') {
            let n = rest.find('<').unwrap_or(rest.len()); text.push_str(&rest[..n]); i += n; continue;
        }
        if rest.starts_with("<!--") { flush(&mut text, &mut out); i += rest.find("-->").map_or(rest.len(), |k| k + 3); continue }
        if rest.starts_with("<!") || rest.starts_with("<?") { flush(&mut text, &mut out); i += rest.find('>').map_or(rest.len(), |k| k + 1); continue }
        let close = rest.starts_with("</");
        let at = if close { 2 } else { 1 };
        let nlen = rest[at..].find(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).unwrap_or(rest.len() - at);
        if nlen == 0 || !b[i + at].is_ascii_alphabetic() { text.push('<'); i += 1; continue }
        let name = rest[at..at + nlen].to_ascii_lowercase();
        let (attrs, len, self_closed) = match attributes(&rest[at + nlen..]) { Some(x) => x, None => { text.push('<'); i += 1; continue } };
        i += at + nlen + len;
        flush(&mut text, &mut out);
        if hidden(&name) {
            if !close && !self_closed {
                // skip to its closing tag (or the end)
                let want = format!("</{name}"); let lower = s[i..].to_ascii_lowercase();
                i += match lower.find(&want) { Some(k) => k + s[i + k..].find('>').map_or(s.len() - i - k, |e| e + 1), None => s.len() - i };
            }
            continue;
        }
        let Some(t) = tag(&name) else { continue };
        if close { if !void(t) { out.push(Tok::Close(t)) } continue }
        let mut kept: Vec<(&'static str, String)> = attrs.into_iter().filter_map(|(k, v)| attr(t, &k).map(|a| (a, v)))
            .filter(|(k, v)| !matches!(*k, "src" | "href" | "srcset") || safe_url(v)).collect();
        if name == "center" { kept.push(("align", "center".into())) }
        if void(t) || self_closed { out.push(Tok::Void(t, kept)) } else { out.push(Tok::Open(t, kept)) }
    }
    flush(&mut text, &mut out);
    out
}

/// The attributes after a tag's name, up to and including its `>`: (name lowercased, value unescaped), the bytes read, and whether it ends in `/>`.
/// None when the tag never closes.
fn attributes(s: &str) -> Option<(Vec<(String, String)>, usize, bool)> {
    let b = s.as_bytes(); let mut i = 0; let mut out = Vec::new();
    loop {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') { i += 1; }
        if i >= b.len() { return None }
        if b[i] == b'>' { return Some((out, i + 1, i > 0 && b[i - 1] == b'/')) }
        let start = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && !matches!(b[i], b'=' | b'>' | b'/') { i += 1; }
        let name = s[start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() { i += 1; }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1; while i < b.len() && b[i].is_ascii_whitespace() { i += 1; }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i]; let vs = i + 1; i = vs; while i < b.len() && b[i] != q { i += 1; }
                if i >= b.len() { return None }
                value = unescape(&s[vs..i]); i += 1;
            } else { let vs = i; while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' { i += 1; } value = unescape(&s[vs..i]); }
        }
        if !name.is_empty() { out.push((name, value)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_centred_picture_keeps_its_tag_and_attributes() {
        let t = tokens("<p align=\"center\">\n  <img src=\"img/day1.png\" alt=\"Day 1 &amp; more\" title='day1.png' onerror=\"x()\">\n</p>\n");
        assert_eq!(t, vec![Tok::Open("p", vec![("align", "center".into())]), Tok::Text("\n  ".into()),
            Tok::Void("img", vec![("src", "img/day1.png".into()), ("alt", "Day 1 & more".into()), ("title", "day1.png".into())]), Tok::Text("\n".into()), Tok::Close("p"), Tok::Text("\n".into())]);
    }
    #[test] fn scripts_styles_and_their_contents_are_gone() {
        let t = tokens("a<script>alert(1)</script>b<style>p{}</style><iframe src=x></iframe>c<!-- hid -->d<SCRIPT >x</SCRIPT>e");
        let text: String = t.iter().map(|x| match x { Tok::Text(s) => s.as_str(), _ => "" }).collect();
        assert_eq!(text, "abcde"); assert!(t.iter().all(|x| matches!(x, Tok::Text(_))));
    }
    #[test] fn unsafe_addresses_and_event_attributes_are_dropped() {
        assert_eq!(tokens("<a href=\"javascript:alert(1)\" onclick=\"x\">x</a>"), vec![Tok::Open("a", vec![]), Tok::Text("x".into()), Tok::Close("a")]);
        assert_eq!(tokens("<img src=\"data:image/png;base64,AA\">"), vec![Tok::Void("img", vec![])]);
        for ok in ["https://x.io/a.png", "img/a.png", "../a.png", "/a.png", "#top"] { assert!(safe_url(ok), "{ok}") }
        for bad in ["javascript:x", " JavaScript:x", "vbscript:x", "data:x", "file:///etc/passwd"] { assert!(!safe_url(bad), "{bad}") }
    }
    #[test] fn unknown_tags_keep_their_text_and_a_lone_angle_is_text() {
        assert_eq!(tokens("<span class=x>hi</span> 1 < 2 <3"), vec![Tok::Text("hi".into()), Tok::Text(" 1 < 2 <3".into())]);
        assert_eq!(tokens("<br/><BR><center>x</center>"), vec![Tok::Void("br", vec![]), Tok::Void("br", vec![]), Tok::Open("div", vec![("align", "center".into())]), Tok::Text("x".into()), Tok::Close("div")]);
    }
    #[test] fn entities() { assert_eq!(unescape("a &lt;b&gt; &#169; &#x263A; &copy &bogus; &"), "a <b> © ☺ © &bogus; &"); }
    #[test] fn an_unclosed_tag_is_text() { assert_eq!(tokens("<img src=\"a"), vec![Tok::Text("<img src=\"a".into())]); }
}

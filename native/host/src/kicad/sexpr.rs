//! core/sexpr.bend in Rust: KiCad S-expressions over bytes. A file is cut into its top-level forms by one flat pass (which
//! also tells a finished file from one still being written); each form parses on its own, as slices of the file's bytes:
//! nothing is copied but a text with an escape or a bad byte.
use super::fl;
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq)]
pub enum Sx<'a> { Atom(Cow<'a, str>), Str(Cow<'a, str>), List(Vec<Sx<'a>>), End }
static END: Sx<'static> = Sx::End;
/// Deeper than any KiCad file: past it a form is not read (it would overflow the stack of whatever walks or drops it; the Bend child
/// was its own process and could die of that alone).
pub const DEEPEST: usize = 4096;

/// Sx.forms (plot_cut): the byte ranges of the top-level forms, or None when the file is unfinished (unbalanced, or cut in a string).
pub fn forms(b: &[u8]) -> Option<Vec<&[u8]>> {
    let (mut d, mut s, mut e, mut start) = (0u32, false, false, 0usize);
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if s {
            if e { e = false } else if c == b'\\' { e = true } else if c == b'"' { s = false }
        } else {
            match c {
                b'"' => s = true,
                b'(' => { if d == 1 { start = i } d = d.wrapping_add(1) }
                b')' => { if d == 2 { out.push(&b[start..=i]) } d = d.wrapping_sub(1) }
                _ => {}
            }
        }
        i += 1;
    }
    if d != 0 || s { None } else { Some(out) }
}

/// Utf8.decode: a bad byte reads as U+FFFD; overlong forms pass, a surrogate or a value past U+10FFFF is U+FFFD.
pub fn decode(bs: &[u8]) -> String {
    let (mut o, mut cp, mut owe) = (String::with_capacity(bs.len()), 0u32, 0u32);
    for &x in bs {
        if owe > 0 {
            if x & 192 == 128 {
                cp = (cp << 6) | (x as u32 & 63); owe -= 1;
                if owe == 0 { o.push(char::from_u32(cp).unwrap_or('\u{fffd}')) }
                continue
            }
            o.push('\u{fffd}'); owe = 0;
        }
        if x < 128 { o.push(x as char) }
        else if x & 224 == 192 { cp = x as u32 & 31; owe = 1 }
        else if x & 240 == 224 { cp = x as u32 & 15; owe = 2 }
        else if x & 248 == 240 { cp = x as u32 & 7; owe = 3 }
        else { o.push('\u{fffd}') }
    }
    if owe > 0 { o.push('\u{fffd}') }
    o
}
fn text(bs: &[u8]) -> Cow<'_, str> { match std::str::from_utf8(bs) { Ok(s) => Cow::Borrowed(s), Err(_) => Cow::Owned(decode(bs)) } }

fn add<'a>(st: &mut Vec<Vec<Sx<'a>>>, x: Sx<'a>) { match st.last_mut() { Some(t) => t.push(x), None => st.push(vec![x]) } }

/// Sx.parse: one form's bytes as a tree (End when malformed).
pub fn parse(bs: &[u8]) -> Sx<'_> {
    let mut st: Vec<Vec<Sx>> = vec![vec![]];
    let (mut i, n) = (0, bs.len());
    while i < n {
        let c = bs[i];
        if c <= 32 { i += 1 }
        else if c == b'(' { if st.len() > DEEPEST { return Sx::End } st.push(Vec::with_capacity(4)); i += 1 }
        else if c == b')' { if let Some(top) = st.pop() { add(&mut st, Sx::List(top)) } i += 1 }
        else if c == b'"' {
            let start = i + 1;
            let mut j = start;
            while j < n && bs[j] != b'"' && bs[j] != b'\\' { j += 1 }
            if j >= n { break } // cut in a string: Lx.fin keeps nothing of it
            if bs[j] == b'"' { add(&mut st, Sx::Str(text(&bs[start..j]))); i = j + 1; continue }
            let mut acc = bs[start..j].to_vec();
            let mut closed = false;
            while j < n {
                let b = bs[j];
                if b == b'\\' { j += 1; if j < n { acc.push(if bs[j] == b'n' { 10 } else { bs[j] }); j += 1 } }
                else if b == b'"' { j += 1; closed = true; break }
                else { acc.push(b); j += 1 }
            }
            if !closed { break }
            add(&mut st, Sx::Str(Cow::Owned(match String::from_utf8(acc) { Ok(s) => s, Err(e) => decode(e.as_bytes()) })));
            i = j;
        } else {
            let start = i;
            while i < n && bs[i] > 32 && !matches!(bs[i], b'(' | b')' | b'"') { i += 1 }
            add(&mut st, Sx::Atom(text(&bs[start..i])));
        }
    }
    if st.len() == 1 && st[0].len() == 1 { st.pop().unwrap().pop().unwrap() } else { Sx::End }
}

impl<'a> Sx<'a> {
    /// a list's head atom ("segment", "at", ...), or ""
    pub fn name(&self) -> &str { match self { Sx::List(v) => match v.first() { Some(Sx::Atom(s)) => &**s, _ => "" }, _ => "" } }
    /// a list's children after its name
    pub fn args(&self) -> &[Sx<'a>] { match self { Sx::List(v) if !v.is_empty() => &v[1..], _ => &[] } }
    pub fn text(&self) -> &str { match self { Sx::Atom(s) | Sx::Str(s) => &**s, _ => "" } }
    /// the first child list called name (End when there is none)
    pub fn get(&self, name: &str) -> &Sx<'a> { self.args().iter().find(|x| x.name() == name).unwrap_or(&END) }
    /// the i-th argument's text ("" when absent)
    pub fn arg(&self, i: usize) -> &str { self.args().get(i).map(|x| x.text()).unwrap_or("") }
    /// the i-th argument as a number (0 when absent)
    pub fn f(&self, i: usize) -> f32 { fl::num(self.arg(i)) }
    /// the text of (name "value") under x
    pub fn attr(&self, name: &str) -> &str { self.get(name).arg(0) }
    pub fn all<'s>(&'s self, name: &'s str) -> impl Iterator<Item = &'s Sx<'a>> + 's { self.args().iter().filter(move |x| x.name() == name) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn cuts_top_level_forms() {
        let f = forms(br#"(kicad_pcb (a 1) (b "x)(") (c (d)))"#).unwrap();
        assert_eq!(f, vec![&b"(a 1)"[..], &br#"(b "x)(")"#[..], &b"(c (d))"[..]]);
        assert_eq!(forms(b"(a (b)"), None); assert_eq!(forms(b"(a \"b)"), None);
        assert_eq!(forms(b"").unwrap().len(), 0); assert_eq!(forms(br#"(a (b "\"") )"#).unwrap().len(), 1);
    }
    #[test] fn parses_as_lx_and_tree() {
        let x = parse(br#"(seg (start 1 2.5) "a\nb\t\"c" tail"str"(end))"#);
        assert_eq!(x.name(), "seg"); assert_eq!(x.get("start").f(1), 2.5); assert_eq!(x.arg(1), "a\nbt\"c"); assert_eq!(x.arg(2), "tail"); assert_eq!(x.arg(3), "str"); assert_eq!(x.get("end").name(), "end");
        assert_eq!(x.get("none"), &Sx::End); assert_eq!(x.get("none").arg(0), ""); assert_eq!(parse(b""), Sx::End); assert_eq!(parse(b"(a) (b)"), Sx::End);
        assert_eq!(parse(b"(\"s\" 1)").name(), ""); assert_eq!(parse(b"()").args().len(), 0);
    }
    #[test] fn bad_bytes_read_as_replacement() {
        assert_eq!(decode(&[0x41, 0xff, 0xc3, 0xa9, 0xe2, 0x82]), "A\u{fffd}é\u{fffd}");
        assert_eq!(decode(&[0xc3, 0x41]), "\u{fffd}A"); assert_eq!(decode(&[0xed, 0xa0, 0x80]), "\u{fffd}"); assert_eq!(decode(&[0xc0, 0x80]), "\0");
        let x = parse(b"(n \"a\xffb\")"); assert_eq!(x.arg(0), "a\u{fffd}b");
    }
    #[test] fn a_very_deep_form_is_not_read() {
        let mut s = vec![b'('; DEEPEST + 10]; s.extend(vec![b')'; DEEPEST + 10]); assert_eq!(parse(&s), Sx::End);
    }
}

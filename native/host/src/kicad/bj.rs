//! core/json in Rust, for the review rules: text kept exactly (a number stays its raw text, keys stay in order, the first of two
//! equal keys wins) and the same lenient reader (commas and colons are only separators, a bare word is null, a lone surrogate is
//! U+FFFD), which serde_json is not. One difference: a value nested deeper than DEEPEST is not read (it would overflow the stack of
//! whatever walks or drops it; child was its own process).
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum J { Null, Flag(bool), Num(String), Str(String), Arr(Vec<J>), Obj(Vec<(String, J)>) }
pub const DEEPEST: usize = 4096;
static NULL: J = J::Null;

enum Tok { Open(bool), Close, Sep, Str(String), Num(String), Word(String), Bad }
enum Lex { Idle, Str, Esc, Uni { left: u8, code: u32, hi: u32 }, Hi(u32), HiE(u32), Atom(bool), Bad }

fn is_hi(u: u32) -> bool { (55296..=56319).contains(&u) }
fn is_lo(u: u32) -> bool { (56320..=57343).contains(&u) }
fn push(acc: &mut String, u: u32) { acc.push(char::from_u32(u).unwrap_or('\u{fffd}')) }
/// Lex.unit: a finished unit with no high surrogate waiting
fn unit(code: u32, acc: &mut String) -> Lex {
    if is_hi(code) { return Lex::Hi(code) }
    push(acc, if is_lo(code) { 0xfffd } else { code }); Lex::Str
}
/// Lex.esc
fn esc(c: char, acc: &mut String) -> Lex {
    match c {
        'u' => return Lex::Uni { left: 4, code: 0, hi: 0 },
        'n' => acc.push('\n'), 't' => acc.push('\t'), 'r' => acc.push('\r'), 'b' => acc.push('\u{8}'), 'f' => acc.push('\u{c}'),
        c => acc.push(c),
    }
    Lex::Str
}
fn structural(c: char) -> bool { matches!(c, '[' | '{' | ']' | '}' | '"' | ':' | ',') || c as u32 <= 32 }

/// Json.lex
fn lex(s: &str) -> Vec<Tok> {
    let (mut st, mut acc, mut out) = (Lex::Idle, String::new(), Vec::new());
    for c in s.chars() {
        if let Lex::Atom(num) = st {
            if !structural(c) { acc.push(c); continue }
            let t = std::mem::take(&mut acc); out.push(if num { Tok::Num(t) } else { Tok::Word(t) }); st = Lex::Idle;
        }
        st = match st {
            Lex::Bad => Lex::Bad,
            Lex::Idle => match c {
                '[' => { out.push(Tok::Open(false)); Lex::Idle } '{' => { out.push(Tok::Open(true)); Lex::Idle }
                ']' | '}' => { out.push(Tok::Close); Lex::Idle } ':' | ',' => { out.push(Tok::Sep); Lex::Idle }
                '"' => Lex::Str,
                c if c as u32 <= 32 => Lex::Idle,
                c => { acc.push(c); Lex::Atom(c == '-' || c.is_ascii_digit()) }
            },
            Lex::Str => match c { '"' => { out.push(Tok::Str(std::mem::take(&mut acc))); Lex::Idle } '\\' => Lex::Esc, c => { acc.push(c); Lex::Str } },
            Lex::Esc => esc(c, &mut acc),
            Lex::Uni { left, code, hi } => match c.to_digit(16) {
                None => Lex::Bad,
                Some(d) => { let code = code << 4 | d;
                    if left > 1 { Lex::Uni { left: left - 1, code, hi } }
                    else if hi == 0 { unit(code, &mut acc) }
                    else if is_lo(code) { push(&mut acc, ((hi - 55296) << 10) + 65536 + (code - 56320)); Lex::Str }
                    else { acc.push('\u{fffd}'); unit(code, &mut acc) } }
            },
            Lex::Hi(hi) => if c == '\\' { Lex::HiE(hi) } else {
                acc.push('\u{fffd}');
                match c { '"' => { out.push(Tok::Str(std::mem::take(&mut acc))); Lex::Idle } c => { acc.push(c); Lex::Str } }
            },
            Lex::HiE(hi) => if c == 'u' { Lex::Uni { left: 4, code: 0, hi } } else { acc.push('\u{fffd}'); esc(c, &mut acc) },
            Lex::Atom(_) => unreachable!(),
        };
    }
    match st { Lex::Idle => {} Lex::Atom(true) => out.push(Tok::Num(acc)), Lex::Atom(false) => out.push(Tok::Word(acc)), _ => out.push(Tok::Bad) }
    out
}

enum Frame { Arr(Vec<J>), Obj(Vec<(String, J)>, Option<String>) }
/// Parse.put: a finished value lands in the enclosing frame, or becomes the top; false when an object's key is not a string
fn put(stack: &mut Vec<Frame>, top: &mut Option<J>, v: J) -> bool {
    match stack.last_mut() {
        None => { *top = Some(v); true }
        Some(Frame::Arr(a)) => { a.push(v); true }
        Some(Frame::Obj(a, k)) => match k.take() {
            Some(k) => { a.push((k, v)); true }
            None => match v { J::Str(s) => { *k = Some(s); true } _ => false },
        },
    }
}
/// Json.parse: the value of a JSON text, or None when it is malformed
pub fn parse(s: &str) -> Option<J> {
    let (mut stack, mut top): (Vec<Frame>, Option<J>) = (vec![], None);
    for t in lex(s) {
        let ok = match t {
            Tok::Open(o) => { top = None; if stack.len() >= DEEPEST { return None } stack.push(if o { Frame::Obj(vec![], None) } else { Frame::Arr(vec![]) }); true }
            Tok::Close => { top = None; match stack.pop() {
                None => false,
                Some(Frame::Arr(a)) => put(&mut stack, &mut top, J::Arr(a)),
                Some(Frame::Obj(a, _)) => put(&mut stack, &mut top, J::Obj(a)) } }
            Tok::Sep => { top = None; true }
            Tok::Str(x) => { top = None; put(&mut stack, &mut top, J::Str(x)) }
            Tok::Num(x) => { top = None; put(&mut stack, &mut top, J::Num(x)) }
            Tok::Word(w) => { top = None; put(&mut stack, &mut top, match w.as_str() { "true" => J::Flag(true), "false" => J::Flag(false), _ => J::Null }) }
            Tok::Bad => false,
        };
        if !ok { return None }
    }
    if stack.is_empty() { top } else { None }
}

/// Json.esc: only control characters, the quote and the backslash are escaped
pub fn quote(s: &str, o: &mut String) {
    o.push('"');
    for c in s.chars() {
        match c {
            '\n' => o.push_str("\\n"), '\r' => o.push_str("\\r"), '\t' => o.push_str("\\t"), '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"),
            c if (c as u32) < 32 => { let n = c as u32; o.push_str("\\u00"); for d in [n >> 4, n & 15] { o.push(char::from_digit(d, 16).unwrap()) } }
            c => o.push(c),
        }
    }
    o.push('"');
}

impl J {
    pub fn str(s: impl Into<String>) -> J { J::Str(s.into()) }
    /// Json.obj
    pub fn obj(kv: &[(&str, J)]) -> J { J::Obj(kv.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()) }
    /// Json.get: the first match, None on anything but an object
    pub fn get(&self, key: &str) -> Option<&J> { match self { J::Obj(f) => f.iter().find(|(k, _)| k == key).map(|(_, v)| v), _ => None } }
    /// Json.get_obj
    pub fn get_obj(&self, key: &str) -> &J { self.get(key).unwrap_or(&NULL) }
    /// Json.get_str
    pub fn get_str<'a>(&'a self, key: &str, d: &'a str) -> &'a str { match self.get(key) { Some(J::Str(s)) => s, _ => d } }
    /// Json.get_bool
    pub fn get_bool(&self, key: &str, d: bool) -> bool { match self.get(key) { Some(J::Flag(b)) => *b, _ => d } }
    /// Json.as_raw(get(key)): a number's text or a string
    pub fn raw<'a>(&'a self, key: &str, d: &'a str) -> &'a str { match self.get(key) { Some(J::Num(r)) | Some(J::Str(r)) => r, _ => d } }
    /// Json.elems
    pub fn elems(&self) -> &[J] { match self { J::Arr(a) => a, _ => &[] } }
    /// Json.show
    pub fn show(&self) -> String { let mut o = String::new(); self.emit(&mut o); o }
    fn emit(&self, o: &mut String) {
        match self {
            J::Null => o.push_str("null"), J::Flag(b) => o.push_str(if *b { "true" } else { "false" }), J::Num(r) => o.push_str(r), J::Str(s) => quote(s, o),
            J::Arr(a) => { o.push('['); for (i, x) in a.iter().enumerate() { if i > 0 { o.push(',') } x.emit(o) } o.push(']') }
            J::Obj(f) => { o.push('{'); for (i, (k, v)) in f.iter().enumerate() { if i > 0 { o.push(',') } quote(k, o); o.push(':'); v.emit(o) } o.push('}') }
        }
    }
    /// What a request's parameters are as Json: a number keeps the text it came with where the Value holds it (arbitrary_precision).
    pub fn of(v: &Value) -> J {
        match v {
            Value::Null => J::Null, Value::Bool(b) => J::Flag(*b), Value::Number(n) => J::Num(n.to_string()), Value::String(s) => J::Str(s.clone()),
            Value::Array(a) => J::Arr(a.iter().map(J::of).collect()), Value::Object(m) => J::Obj(m.iter().map(|(k, v)| (k.clone(), J::of(v))).collect()),
        }
    }
    /// The Value a reply carries; what is not JSON to serde_json reads as null there.
    pub fn value(&self) -> Value { serde_json::from_str(&self.show()).unwrap_or(Value::Null) }
}

/// String.trim's characters
pub fn space(c: char) -> bool { c == ' ' || ('\t'..='\r').contains(&c) }
pub fn trim(s: &str) -> &str { s.trim_matches(space) }
pub fn trim_start(s: &str) -> &str { s.trim_start_matches(space) }
pub fn trim_end(s: &str) -> &str { s.trim_end_matches(space) }
/// String.take, String.drop: by characters
pub fn take(s: &str, n: usize) -> String { s.chars().take(n).collect() }
pub fn drop(s: &str, n: usize) -> String { s.chars().skip(n).collect() }
pub fn len(s: &str) -> usize { s.chars().count() }
/// Nat.read: digits only; past 2^48-1 it does not read
pub fn nat(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) { return None }
    let t = s.trim_start_matches('0');
    if t.len() > 15 || (t.len() == 15 && t > "281474976710655") { return None }
    Some(t.parse().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn reads_as_json_does() {
        assert_eq!(parse(r#"{"a":1.50,"a":2,"b":[true,false,null,nope]}"#).unwrap().show(), r#"{"a":1.50,"a":2,"b":[true,false,null,null]}"#);
        assert_eq!(parse(r#"{"a":1,"a":2}"#).unwrap().raw("a", ""), "1"); // first match wins
        assert_eq!(parse(r#"{"a" 1 "b" 2}"#).unwrap().show(), r#"{"a":1,"b":2}"#); // separators are only separators
        assert_eq!(parse("[1 2]").unwrap().show(), "[1,2]");
        for bad in ["", " ", ",", "[", "{", "]", "}", "\"a", "\"\\u12", "\"\\uzzzz\"", "{1:2}", "[1],"] { assert_eq!(parse(bad), None, "{bad:?}") }
        assert_eq!(parse("1 2").unwrap().show(), "2"); assert_eq!(parse("-x").unwrap(), J::Num("-x".into())); assert_eq!(parse("nan").unwrap(), J::Null);
        assert_eq!(parse(r#""a\u00e9\n\/\q""#).unwrap(), J::str("aé\n/q"));
    }
    #[test] fn surrogates_as_lex_does() {
        assert_eq!(parse(r#""\ud83d\ude00""#).unwrap(), J::str("😀"));
        assert_eq!(parse(r#""\ud83d""#).unwrap(), J::str("\u{fffd}")); assert_eq!(parse(r#""\ude00x""#).unwrap(), J::str("\u{fffd}x"));
        assert_eq!(parse(r#""\ud83dx""#).unwrap(), J::str("\u{fffd}x")); assert_eq!(parse(r#""\ud83d\n""#).unwrap(), J::str("\u{fffd}\n"));
        assert_eq!(parse(r#""\ud83d\u0041""#).unwrap(), J::str("\u{fffd}A")); assert_eq!(parse(r#""\ud83d\ud83d\ude00""#).unwrap(), J::str("\u{fffd}😀"));
    }
    #[test] fn shows_as_json_does() {
        assert_eq!(J::str("a\"\\\n\r\t\u{1}\u{1f}é/\u{7f}").show(), "\"a\\\"\\\\\\n\\r\\t\\u0001\\u001fé/\u{7f}\"");
        assert_eq!(J::obj(&[("k", J::Num("nan".into())), ("a", J::Arr(vec![J::Null, J::Flag(false)]))]).show(), r#"{"k":nan,"a":[null,false]}"#);
    }
    #[test] fn a_very_deep_value_is_not_read() { assert_eq!(parse(&"[".repeat(DEEPEST + 10)), None); assert!(parse(&format!("{}{}", "[".repeat(100), "]".repeat(100))).is_some()) }
    #[test] fn reads_a_nat_as_nat_read_does() {
        for (s, v) in [("0", Some(0)), ("007", Some(7)), ("281474976710655", Some(281474976710655)), ("281474976710656", None), ("", None), ("+1", None), ("1.0", None), ("-1", None), ("١", None)] { assert_eq!(nat(s), v, "{s:?}") }
    }
    #[test] fn trims_what_char_is_space_says() { assert_eq!(trim(" \t\n\u{b}\u{c}\r a\u{a0} \n"), "a\u{a0}"); assert_eq!(trim("\u{85}x"), "\u{85}x") }
}

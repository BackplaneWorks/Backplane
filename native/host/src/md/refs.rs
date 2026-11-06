//! src/core/ref in Rust: Backplane references (`backplane://thread/<id>#<msg>`, bare ids), parsed and written.
//! Strings are Vec<char> where counts code points; `acc` buffers that keeps reversed are kept forward here.
#[derive(Clone, Debug, PartialEq)]
pub struct Ref { pub kind: String, pub rest: Vec<String>, pub frag: String }

const KINDS: [&str; 16] = ["thread", "bot", "room", "person", "project", "routine", "task", "pr", "work", "file", "machine", "conv", "mail", "mailroom", "decision", "charter"];

fn safe(c: char) -> bool { c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') }

/// Ref.pct: everything but A-Z a-z 0-9 - . _ ~ as %XX of its UTF-8 bytes
pub fn pct(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if safe(c) { o.push(c) } else { let mut b = [0u8; 4]; for x in c.encode_utf8(&mut b).bytes() { o.push_str(&format!("%{:02X}", x)); } }
    }
    o
}

struct Dec { acc: Vec<char>, cp: u32, owe: u32, floor: u32 }
impl Dec {
    fn new() -> Dec { Dec { acc: vec![], cp: 0, owe: 0, floor: 0 } }
    fn lead(&mut self, u: u32) {
        if u < 128 { self.acc.push(char::from_u32(u).unwrap()); self.cp = 0; self.owe = 0; self.floor = 0 }
        else if (194..=223).contains(&u) { self.cp = u & 31; self.owe = 1; self.floor = 128 }
        else if (224..=239).contains(&u) { self.cp = u & 15; self.owe = 2; self.floor = 2048 }
        else if (240..=244).contains(&u) { self.cp = u & 7; self.owe = 3; self.floor = 65536 }
        else { self.acc.push('\u{FFFD}'); self.cp = 0; self.owe = 0; self.floor = 0 }
    }
    fn end(&mut self, cp: u32) {
        let bad = cp < self.floor || cp > 0x10FFFF || (0xD800..=0xDFFF).contains(&cp);
        self.acc.push(if bad { '\u{FFFD}' } else { char::from_u32(cp).unwrap() });
        self.cp = 0; self.owe = 0; self.floor = 0;
    }
    fn byte(&mut self, u: u32) {
        if self.owe == 0 { self.lead(u); return }
        if (128..=191).contains(&u) {
            let cp = (self.cp << 6) | (u & 63);
            if self.owe == 1 { self.end(cp) } else { self.cp = cp; self.owe -= 1 }
        } else { self.acc.push('\u{FFFD}'); self.lead(u) }
    }
    fn raw(&mut self, c: char) {
        if self.owe != 0 { self.acc.push('\u{FFFD}') }
        self.acc.push(c); self.cp = 0; self.owe = 0; self.floor = 0;
    }
}

fn unhex(c: char) -> u32 { match c { '0'..='9' => c as u32 - 48, 'A'..='F' => c as u32 - 55, 'a'..='f' => c as u32 - 87, _ => 16 } }

/// Ref.unpct: invalid hex stays literal, invalid UTF-8 becomes U+FFFD
pub fn unpct(s: &str) -> String {
    let mut d = Dec::new(); let (mut mode, mut hi) = (0u8, '0');
    for c in s.chars() {
        if mode == 0 { if c == '%' { mode = 1 } else { d.raw(c) } continue }
        let n = unhex(c);
        if n < 16 {
            if mode == 1 { mode = 2; hi = c } else { d.byte(unhex(hi) * 16 + n); mode = 0 }
        } else {
            d.raw('%'); if mode == 2 { d.raw(hi) }
            if c == '%' { mode = 1 } else { d.raw(c); mode = 0 }
        }
    }
    if mode == 1 { d.raw('%') } else if mode == 2 { d.raw('%'); d.raw(hi) }
    if d.owe != 0 { d.acc.push('\u{FFFD}') }
    d.acc.into_iter().collect()
}

/// Ref.cut: before the first sep and after it ("" when there is none)
fn cut(s: &str, sep: char) -> (&str, &str) { match s.split_once(sep) { Some((a, b)) => (a, b), None => (s, "") } }

fn untrail(s: &str) -> &str { s.strip_suffix('/').unwrap_or(s) }

fn path(k: &str, xs: &[String]) -> String { let mut o = k.to_string(); for x in xs { o.push('/'); o.push_str(&pct(x)); } o }

pub fn uri(r: &Ref) -> String { format!("backplane://{}{}", path(&r.kind, &r.rest), if r.frag.is_empty() { String::new() } else { format!("#{}", pct(&r.frag)) }) }

fn shape(k: &str, xs: &[String]) -> bool {
    match xs.len() {
        0 => k == "person",
        1 => !["person", "pr", "file", "mail", "decision"].contains(&k),
        2 => ["pr", "file", "mail", "decision"].contains(&k),
        _ => k == "file",
    }
}

fn checked(k: &str, xs: Vec<String>, f: String) -> Option<Ref> {
    if KINDS.contains(&k) && shape(k, &xs) && xs.iter().all(|x| !x.is_empty()) && (f.is_empty() || k == "thread") { Some(Ref { kind: k.into(), rest: xs, frag: f }) } else { None }
}

fn parse_path(p: &str, f: &str) -> Option<Ref> {
    let xs: Vec<&str> = untrail(p).split('/').collect();
    checked(xs[0], xs[1..].iter().map(|x| unpct(x)).collect(), unpct(f))
}

fn web_origin(rest: &str) -> Option<Ref> {
    let (host, tail) = cut(rest, '#');
    let h = untrail(host);
    if !h.is_empty() && !h.contains('/') && tail.starts_with('/') { let (p, f) = cut(&tail[1..], '#'); parse_path(p, f) } else { None }
}

/// Ref.parse
pub fn parse(s: &str) -> Option<Ref> {
    if let Some(r) = s.strip_prefix("backplane://") {
        if let Some(id) = r.strip_prefix("open?thread=") { return if id.is_empty() { None } else { checked("thread", vec![unpct(id)], String::new()) } }
        let (p, f) = cut(r, '#'); parse_path(p, f)
    } else if let Some(r) = s.strip_prefix("http://") { web_origin(r) }
    else if let Some(r) = s.strip_prefix("https://") { web_origin(r) }
    else { None }
}

fn link_ok(s: &str) -> bool { !s.is_empty() && s.chars().all(safe) }
fn digits(s: &str) -> bool { !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) }

fn bare_local(s: &str, id: &str) -> Option<Ref> {
    let mut cs = s.chars(); let c = cs.next()?; let t = cs.as_str();
    let kind = match c { 't' => "thread", 'b' => "bot", 'p' => "project", 'r' => "room", 'o' => "routine", _ => "" };
    let xs: Vec<&str> = t.split('-').collect();
    match xs.len() {
        2 => if !kind.is_empty() && digits(xs[0]) && digits(xs[1]) { Some(Ref { kind: kind.into(), rest: vec![id.into()], frag: String::new() }) } else { None },
        3 => if kind == "bot" && xs.iter().all(|x| digits(x)) { Some(Ref { kind: "task".into(), rest: vec![id.into()], frag: String::new() }) } else { None },
        _ => None,
    }
}

/// Ref.bare: a bare thread/bot/project/room/routine/task id (t1700000000-1), remote ones as link~id
pub fn bare(w: &str) -> Option<Ref> {
    if w.contains('~') { let (link, tail) = cut(w, '~'); bare_local(if link_ok(link) { tail } else { "" }, w) } else { bare_local(w, w) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn pct_and_back() { assert_eq!(pct("a b/é"), "a%20b%2F%C3%A9"); assert_eq!(unpct("a%20b%2F%C3%A9"), "a b/é"); assert_eq!(unpct("%zz%4"), "%zz%4"); assert_eq!(unpct("%C3"), "\u{FFFD}"); }
    #[test] fn parse_forms() {
        assert_eq!(uri(&parse("backplane://thread/t1-2#m3").unwrap()), "backplane://thread/t1-2#m3");
        assert!(parse("backplane://thread").is_none()); assert!(parse("backplane://person").is_some());
        assert_eq!(parse("http://host/#/thread/x").unwrap().rest, vec!["x".to_string()]);
        assert!(parse("http://host/thread/x").is_none());
    }
    #[test] fn bare_ids() { assert!(bare("t1700000000-1").is_some()); assert!(bare("b1-2-3").unwrap().kind == "task"); assert!(bare("x1-2").is_none()); assert!(bare("t1-2~").is_none()); }
}

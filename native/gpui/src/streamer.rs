//! Streamer mode: while it is on, what would identify the person or open their accounts is not drawn. Every string gpui lays out goes
//! through `Masker::edits` (the vendored gpui's text filter, `gpui::set_text_filter`); editable fields and the terminal use `same_width`,
//! which keeps each string's length so carets and cells stay where they are. The identifiers come from the hub (`privacy.ids`: account
//! emails, Google accounts, machine names, the hub's home and user) and from this computer (its home, user and host name); patterns catch
//! the rest (any email, home path, tailnet name, IP address, phone number, key-shaped token, a link's secret, a pairing code).
//! A pure function: the same identifiers and text give the same edits. On or off, and why, is decided here too (`Mode::on`).
use std::ops::Range;
use std::sync::{Arc, RwLock};

/// What one match becomes. Placeholders say what was there without any of it.
pub const EMAIL: &str = "•••@•••";
pub const HOME: &str = "~";
pub const MACHINE: &str = "machine";
pub const USER: &str = "user";
pub const HIDDEN: &str = "•••";
pub const IP: &str = "•.•.•.•";
pub const TAILNET: &str = "•••.ts.net";

/// The person's own identifiers. `names`: machine and host names, the person's name (whole words, any case); `users`: login names
/// (whole words when 4+ characters, else only as `user@` and in home paths); `emails` are masked by the email pattern anyway and also
/// matched exactly (an address the pattern misses, a quoted one).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ids { pub emails: Vec<String>, pub names: Vec<String>, pub users: Vec<String>, pub homes: Vec<String>, /** account labels, in the hub's order: drawn "account N" */ pub labels: Vec<String> } // i18n-ok: a doc word
impl Ids {
    /// The hub's `privacy.ids` answer.
    pub fn from_hub(v: &serde_json::Value) -> Ids {
        let l = |k: &str| v[k].as_array().into_iter().flatten().filter_map(|x| x.as_str()).map(str::to_string).collect::<Vec<_>>();
        Ids { emails: l("emails"), names: l("names"), users: l("users"), homes: l("homes"), labels: l("labels") }
    }
    /// This computer's own: its home folder, login name and host name.
    pub fn local() -> Ids {
        let mut ids = Ids::default();
        if let Some(h) = crate::plat::user_home() { ids.homes.push(h.to_string_lossy().into_owned()); }
        for k in ["USER", "USERNAME", "LOGNAME"] { if let Ok(u) = std::env::var(k) { ids.users.push(u) } } // i18n-ok: environment names
        if let Some(h) = host_name() { ids.names.push(h) }
        ids
    }
    pub fn merge(&mut self, o: Ids) {
        for (a, b) in [(&mut self.emails, o.emails), (&mut self.names, o.names), (&mut self.users, o.users), (&mut self.homes, o.homes), (&mut self.labels, o.labels)] {
            for x in b { let x = x.trim().to_string(); if !x.is_empty() && !a.iter().any(|y| y.eq_ignore_ascii_case(&x)) { a.push(x) } }
        }
    }
}

#[cfg(unix)]
fn host_name() -> Option<String> {
    let mut b = [0u8; 256];
    // SAFETY: the buffer is ours and its length is passed.
    if unsafe { libc::gethostname(b.as_mut_ptr() as *mut libc::c_char, b.len()) } != 0 { return None }
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    let h = String::from_utf8_lossy(&b[..n]).into_owned();
    Some(h.split('.').next().unwrap_or("").to_string()).filter(|h| !h.is_empty() && h != "localhost")
}
#[cfg(windows)]
fn host_name() -> Option<String> { std::env::var("COMPUTERNAME").ok().filter(|h| !h.is_empty()) }

/// The identifiers made ready for matching (lower case, too-short ones dropped).
#[derive(Debug, Clone, Default)]
pub struct Masker { words: Vec<(String, &'static str)>, users: Vec<String>, homes: Vec<String>, /** (label, "account N"): exact case, whole words */ labels: Vec<(String, String)> } // i18n-ok: a doc word
impl Masker {
    pub fn new(ids: &Ids) -> Masker {
        let mut words: Vec<(String, &'static str)> = vec![];
        for e in &ids.emails { if e.contains('@') && e.len() >= 3 { words.push((e.to_ascii_lowercase(), EMAIL)) } }
        // a machine called by a common word ("desktop", "laptop") names nobody, and masking the word would garble ordinary text
        const GENERIC: &[&str] = &["desktop", "laptop", "server", "workstation", "computer", "home", "office", "mini", "macbook", "imac", "ubuntu", "debian", "fedora", "arch", "raspberrypi", "router", "nas", "localhost", "machine", "user"];
        for n in &ids.names { if n.chars().count() >= 3 && !GENERIC.contains(&n.trim().to_ascii_lowercase().as_str()) {
            // a machine's address or tailnet name is drawn as the patterns draw one, so the same thing always looks the same
            let with = if n.parse::<std::net::IpAddr>().is_ok() { IP } else if n.to_ascii_lowercase().ends_with(".ts.net") { TAILNET } else if n.trim().contains(' ') { HIDDEN } else { MACHINE };
            words.push((n.to_ascii_lowercase(), with)) } }
        let users: Vec<String> = ids.users.iter().filter(|u| !u.is_empty() && !matches!(u.as_str(), "root" | "user" | "admin")).map(|u| u.to_ascii_lowercase()).collect();
        for u in &users { if u.chars().count() >= 4 { words.push((u.clone(), USER)) } }
        // the longest first: "boat-hub" before "boat"
        words.sort_by(|a, b| b.0.len().cmp(&a.0.len())); words.dedup_by(|a, b| a.0 == b.0);
        let homes = ids.homes.iter().map(|h| h.trim_end_matches(['/', '\\']).to_string()).filter(|h| h.len() >= 3).collect();
        // an account's label is the person's own word for it ("Work", "helmut-max"): exact case, whole words; a provider's name is no label to hide
        let labels = ids.labels.iter().enumerate().filter(|(_, l)| l.chars().count() >= 3 && !matches!(l.to_ascii_lowercase().as_str(), "claude" | "codex" | "default" | "default account")) // i18n-ok: provider names compared
            .map(|(i, l)| (l.clone(), crate::i18n::tf("gpui.streamer.account", &[("n", (i + 1).into())]))).collect();
        Masker { words, users, homes, labels }
    }

    /// Every part of `s` to hide, with what is drawn instead: sorted, not overlapping, each on character boundaries.
    pub fn edits(&self, s: &str) -> Vec<(Range<usize>, String)> {
        if s.len() < 2 { return vec![] }
        let mut found: Vec<(Range<usize>, &str)> = vec![];
        let lower = s.to_ascii_lowercase(); // (the same byte offsets: only ASCII changes)
        for h in &self.homes { for (i, _) in s.match_indices(h.as_str()) { let e = i + h.len(); if e >= s.len() || !path_char(s.as_bytes()[e]) { found.push((i..e, HOME)) } } }
        home_paths(s, &mut found);
        emails(s, &mut found);
        tailnet(&lower, &mut found);
        for (w, with) in &self.words { for (i, _) in lower.match_indices(w.as_str()) { if word_at(&lower, i, w.len()) { found.push((i..i + w.len(), *with)) } } }
        for (l, with) in &self.labels { for (i, _) in s.match_indices(l.as_str()) { if word_at(s, i, l.len()) { found.push((i..i + l.len(), with.as_str())) } } }
        for u in &self.users { for (i, _) in lower.match_indices(u.as_str()) {
            let end = i + u.len();
            if word_at(&lower, i, u.len()) && lower[end..].starts_with('@') { found.push((i..end, USER)) }
        } }
        ipv4(s, &mut found); ipv6(s, &mut found);
        keys(s, &mut found); url_secrets(&lower, &mut found); phones(s, &mut found); codes(&lower, s, &mut found);
        merge(found)
    }
    /// `s` as drawn.
    pub fn mask(&self, s: &str) -> String { apply(s, &self.edits(s)) }
    /// `s` with each hidden byte drawn as one `•`-like ASCII mark, so its length (and every caret and cell position) is kept:
    /// for editable text and the terminal's grid. `cells`: one mark per character instead of per byte (a terminal cell is a character).
    pub fn same_width(&self, s: &str, cells: bool) -> String {
        let mut out = String::with_capacity(s.len()); let mut at = 0;
        for (r, _) in self.edits(s) {
            out.push_str(&s[at..r.start]);
            let n = if cells { s[r.clone()].chars().count() } else { r.len() };
            out.extend(std::iter::repeat('*').take(n)); at = r.end;
        }
        out.push_str(&s[at..]); out
    }
}

/// `s` with the edits made.
pub fn apply(s: &str, edits: &[(Range<usize>, String)]) -> String {
    let mut out = String::with_capacity(s.len()); let mut at = 0;
    for (r, with) in edits { out.push_str(&s[at..r.start]); out.push_str(with); at = r.end; }
    out.push_str(&s[at..]); out
}

/// Overlapping finds become one: the earliest start wins and the span grows to cover the others; its placeholder is the widest find's.
fn merge(mut f: Vec<(Range<usize>, &str)>) -> Vec<(Range<usize>, String)> {
    f.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));
    let mut out: Vec<(Range<usize>, &str, usize)> = vec![];
    for (r, w) in f {
        if r.is_empty() { continue }
        match out.last_mut() {
            Some(last) if r.start < last.0.end => { if r.end > last.0.end { if r.len() > last.2 { last.1 = w; last.2 = r.len(); } last.0.end = r.end; } }
            _ => { let n = r.len(); out.push((r, w, n)) }
        }
    }
    out.into_iter().map(|(r, w, _)| (r, w.to_string())).collect()
}

fn is_word(c: u8) -> bool { c.is_ascii_alphanumeric() || c >= 0x80 }
/// Whether `n` bytes at `i` stand as a word of their own (no letter or digit just before or after; a `-` joins words: "boat-hub2").
fn word_at(s: &str, i: usize, n: usize) -> bool {
    let b = s.as_bytes();
    let before = i == 0 || !(is_word(b[i - 1]) || (b[i - 1] == b'-' && i >= 2 && is_word(b[i - 2])));
    let after = i + n >= b.len() || !(is_word(b[i + n]) || (b[i + n] == b'-' && i + n + 1 < b.len() && is_word(b[i + n + 1])));
    before && after && s.is_char_boundary(i) && s.is_char_boundary(i + n)
}
fn path_char(c: u8) -> bool { !(c.is_ascii_whitespace() || matches!(c, b'/' | b'\\' | b'"' | b'\'' | b'`' | b')' | b']' | b'>' | b'<' | b',' | b';' | b':' | b'*' | b'|')) }

/// `/home/NAME`, `/Users/NAME`, `X:\Users\NAME` (either slash): the folder of any user, drawn `~`.
fn home_paths<'a>(s: &str, out: &mut Vec<(Range<usize>, &'a str)>) {
    let b = s.as_bytes();
    for pre in ["/home/", "/Users/", ":\\Users\\", ":/Users/", "\\\\Users\\\\"] {
        for (i, _) in s.match_indices(pre) {
            let start = if pre.starts_with(':') { if i == 0 || !b[i - 1].is_ascii_alphabetic() { continue } i - 1 } else { i };
            let mut e = i + pre.len(); while e < b.len() && path_char(b[e]) { e += 1 }
            if e > i + pre.len() && s.is_char_boundary(e) { out.push((start..e, HOME)) }
        }
    }
}

fn local_char(c: u8) -> bool { c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'%' | b'+' | b'-') }
fn domain_char(c: u8) -> bool { c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-') }
/// name@domain.tld (the domain has a dot and ends in two letters or more); `git@` is a protocol, not a person.
fn emails<'a>(s: &str, out: &mut Vec<(Range<usize>, &'a str)>) {
    let b = s.as_bytes();
    for (at, _) in s.match_indices('@') {
        let mut l = at; while l > 0 && local_char(b[l - 1]) { l -= 1 }
        while l < at && b[l] == b'.' { l += 1 }
        let mut r = at + 1; while r < b.len() && domain_char(b[r]) { r += 1 }
        while r > at + 1 && matches!(b[r - 1], b'.' | b'-') { r -= 1 }
        if l == at || r == at + 1 { continue }
        let dom = &s[at + 1..r];
        let Some(dot) = dom.rfind('.') else { continue };
        let tld = &dom[dot + 1..];
        if tld.len() < 2 || !tld.bytes().all(|c| c.is_ascii_alphabetic()) || dot == 0 { continue }
        if &s[l..at] == "git" { continue }
        out.push((l..r, EMAIL));
    }
}
/// Any `NAME.TAILNET.ts.net` (the tailnet's name identifies its owner).
fn tailnet<'a>(lower: &str, out: &mut Vec<(Range<usize>, &'a str)>) {
    let b = lower.as_bytes();
    for (i, _) in lower.match_indices(".ts.net") {
        let end = i + 7; if end < b.len() && domain_char(b[end]) && b[end] != b'.' { continue }
        let mut l = i; while l > 0 && domain_char(b[l - 1]) { l -= 1 }
        if l < i { out.push((l..end, TAILNET)) }
    }
}
fn digits_at(b: &[u8], i: usize) -> usize { b[i..].iter().take_while(|c| c.is_ascii_digit()).count() }
/// A.B.C.D (each 0-255, not inside a longer dotted number), with a `:port` after it; loopback and 0.0.0.0 stay (they name nothing).
fn ipv4<'a>(s: &str, out: &mut Vec<(Range<usize>, &'a str)>) {
    let b = s.as_bytes(); let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() || (i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'.')) { i += 1; continue }
        let (start, mut j, mut parts, mut ok) = (i, i, 0, true);
        while parts < 4 {
            let n = digits_at(b, j);
            if n == 0 || n > 3 || s[j..j + n].parse::<u32>().map_or(true, |v| v > 255) { ok = false; break }
            j += n; parts += 1;
            if parts < 4 { if j < b.len() && b[j] == b'.' { j += 1 } else { ok = false; break } }
        }
        if ok && (j >= b.len() || !(b[j].is_ascii_alphanumeric() || (b[j] == b'.' && j + 1 < b.len() && b[j + 1].is_ascii_digit()))) {
            let ip = &s[start..j];
            if !(ip.starts_with("127.") || ip == "0.0.0.0") {
                let mut e = j; if e + 1 < b.len() && b[e] == b':' && b[e + 1].is_ascii_digit() { e += 1 + digits_at(b, e + 1) }
                out.push((start..e, IP));
            }
            i = j;
        } else { i = j.max(i + 1) }
    }
}
/// An IPv6 address: hex groups and colons, 3 colons or more, a hex letter or `::` in it (a time "12:30:45" is no address); `::1` stays.
fn ipv6<'a>(s: &str, out: &mut Vec<(Range<usize>, &'a str)>) {
    let b = s.as_bytes(); let mut i = 0;
    while i < b.len() {
        if !(b[i].is_ascii_hexdigit() || b[i] == b':') || (i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b':')) { i += 1; continue }
        let mut j = i; while j < b.len() && (b[j].is_ascii_hexdigit() || b[j] == b':') { j += 1 }
        let t = &s[i..j];
        let groups_ok = t.split(':').all(|g| g.len() <= 4);
        let colons = t.bytes().filter(|&c| c == b':').count();
        let after_ok = j >= b.len() || !(b[j].is_ascii_alphanumeric() || b[j] == b'_');
        if groups_ok && colons >= 3 && after_ok && (t.contains("::") || t.bytes().any(|c| c.is_ascii_alphabetic())) && t.split(':').filter(|g| !g.is_empty()).count() >= 3 && t != "::1" {
            out.push((i..j, IP));
        }
        i = j.max(i + 1);
    }
}
/// Tokens shaped like a provider's key (and what follows `Bearer `).
fn keys<'a>(s: &str, out: &mut Vec<(Range<usize>, &'a str)>) {
    const PRE: [&str; 14] = ["sk-", "ghp_", "gho_", "ghu_", "ghs_", "github_pat_", "xoxb-", "xoxp-", "tskey-", "AKIA", "AIza", "glpat-", "hf_", "Bearer "]; // i18n-ok: key prefixes
    let b = s.as_bytes();
    let tok = |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b'/' | b'+' | b'=');
    for p in PRE { for (i, _) in s.match_indices(p) {
        if i > 0 && tok(b[i - 1]) && p != "Bearer " { continue }
        let mut e = i + p.len(); while e < b.len() && tok(b[e]) { e += 1 }
        let start = if p == "Bearer " { i + p.len() } else { i };
        if e - (i + p.len()) >= 16 { out.push((start..e, HIDDEN)) }
    } }
}
/// A link's secret part: the value after `token=`, `invite=`, `code=`, `key=` ... in a query or a `#` fragment.
fn url_secrets<'a>(lower: &str, out: &mut Vec<(Range<usize>, &'a str)>) {
    const KEYS: [&str; 10] = ["token=", "invite=", "code=", "key=", "secret=", "auth=", "password=", "sig=", "access_token=", "otp="];
    let b = lower.as_bytes();
    for k in KEYS { for (i, _) in lower.match_indices(k) {
        if i == 0 || !matches!(b[i - 1], b'?' | b'&' | b'#' | b'_') { continue }
        let s0 = i + k.len(); let mut e = s0;
        while e < b.len() && !(b[e].is_ascii_whitespace() || matches!(b[e], b'&' | b'#' | b'"' | b'\'' | b')' | b'>' | b'<' | b'`')) { e += 1 }
        if e > s0 && lower.is_char_boundary(e) { out.push((s0..e, HIDDEN)) }
    } }
}
/// Phone numbers: `+` and 8 to 15 digits (spaces, dots, dashes and brackets between), or the North American `(555) 123-4567` / `555-123-4567`.
fn phones<'a>(s: &str, out: &mut Vec<(Range<usize>, &'a str)>) {
    let b = s.as_bytes(); let mut i = 0;
    while i < b.len() {
        let intl = b[i] == b'+' && i + 1 < b.len() && b[i + 1].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_alphanumeric());
        let nanp = (b[i] == b'(' || b[i].is_ascii_digit()) && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || matches!(b[i - 1], b'.' | b'-' | b'/' | b':')));
        if !intl && !nanp { i += 1; continue }
        let mut j = if intl { i + 1 } else { i }; let (mut digits, mut last_digit) = (0, j);
        while j < b.len() && (b[j].is_ascii_digit() || matches!(b[j], b' ' | b'-' | b'.' | b'(' | b')')) {
            if b[j].is_ascii_digit() { digits += 1; last_digit = j + 1 }
            if b[j] == b' ' && j + 1 < b.len() && b[j + 1] == b' ' { break }
            j += 1;
        }
        let t = &s[i..last_digit];
        let ok = if intl { (8..=15).contains(&digits) } else { digits == 10 && (is_nanp(t)) };
        let after_ok = last_digit >= b.len() || !b[last_digit].is_ascii_alphanumeric();
        if ok && after_ok { out.push((i..last_digit, HIDDEN)); i = last_digit } else { i += 1 }
    }
}
fn is_nanp(t: &str) -> bool {
    let shape: String = t.chars().map(|c| if c.is_ascii_digit() { 'd' } else { c }).collect();
    matches!(shape.as_str(), "(ddd) ddd-dddd" | "(ddd)ddd-dddd" | "ddd-ddd-dddd" | "ddd.ddd.dddd")
}
/// A pairing or sign-in code: the word after "code" ("Short code K7QP2MXA", "Code: ABCD-EFGH"), letters and digits, 6 to 12 of them.
fn codes<'a>(lower: &str, s: &str, out: &mut Vec<(Range<usize>, &'a str)>) {
    let b = s.as_bytes();
    for (i, _) in lower.match_indices("code") {
        if i > 0 && is_word(b[i - 1]) { continue }
        let mut j = i + 4; if j < b.len() && b[j] == b':' { j += 1 }
        if j >= b.len() || b[j] != b' ' { continue } j += 1;
        let mut e = j; while e < b.len() && (b[e].is_ascii_uppercase() || b[e].is_ascii_digit() || b[e] == b'-') { e += 1 }
        let n = s[j..e].bytes().filter(|c| *c != b'-').count();
        if (6..=12).contains(&n) && (e >= b.len() || !is_word(b[e])) { out.push((j..e, HIDDEN)) }
    }
}

// ---- on or off ---------------------------------------------------------------------------------------------------------------------------

/// What the person chose on this device (local settings) and what the platform saw.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Mode { pub manual: bool, pub auto_share: bool, pub auto_apps: bool }
/// What the detector saw: a screen-cast session (portal / PipeWire), or a capture app by name.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Seen { pub share: bool, pub app: Option<String> }
impl Mode {
    /// On, and why (the indicator's words): the person's switch, a shared screen, or a recording app.
    pub fn on(&self, seen: &Seen) -> Option<String> {
        if self.manual { return Some(String::new()) }
        if self.auto_share && seen.share { return Some("screen shared".into()) }
        if self.auto_apps { if let Some(a) = &seen.app { return Some(a.clone()) } }
        None
    }
}

/// The masker in force (None: off). gpui's filter reads it for every string it lays out.
static ACTIVE: RwLock<Option<Arc<Masker>>> = RwLock::new(None);
pub fn active() -> Option<Arc<Masker>> { ACTIVE.read().ok().and_then(|g| g.clone()) }
pub fn is_on() -> bool { ACTIVE.read().map_or(false, |g| g.is_some()) }
/// Turn masking on with these identifiers, or off. gpui's text filter follows; the caller redraws every window.
pub fn set(m: Option<Masker>) {
    let m = m.map(Arc::new);
    if let Ok(mut g) = ACTIVE.write() { *g = m.clone() }
    gpui::set_text_filter(m.map(|m| Arc::new(move |s: &str| m.edits(s)) as gpui::TextFilter));
}
/// A string drawn through anything that is not a gpui text element (a window title): masked while on.
pub fn shown(s: &str) -> String { active().map_or_else(|| s.to_string(), |m| m.mask(s)) }
/// Editable text and terminal rows: masked to the same length while on.
pub fn same_width(s: &str, cells: bool) -> Option<String> { active().map(|m| m.same_width(s, cells)).filter(|t| t != s) }

// ---- what the platform can see -------------------------------------------------------------------------------------------------------------

/// Watching for a screen being captured. What each platform can and cannot see is said in Settings (`what_auto_sees`).
pub mod detect {
    use super::Seen;
    /// Known recording and streaming programs, by process name (Linux `comm` is cut to 15 bytes), and the name the indicator shows.
    pub const APPS: &[(&str, &str)] = &[
        ("obs", "OBS"), ("obs-browser-pag", "OBS"), ("obs64.exe", "OBS"), ("obs32.exe", "OBS"), ("OBS", "OBS"), ("Streamlabs OBS.exe", "Streamlabs"), ("Streamlabs Desktop", "Streamlabs"), // i18n-ok: program names
        ("simplescreenrec", "SimpleScreenRecorder"), ("kazam", "Kazam"), ("vokoscreenNG", "vokoscreen"), ("peek", "Peek"), ("kooha", "Kooha"), ("gpu-screen-reco", "GPU Screen Recorder"), // i18n-ok: program names
        ("wf-recorder", "wf-recorder"), ("recordmydesktop", "recordMyDesktop"), ("byzanz-record", "Byzanz"), // i18n-ok: program names
        ("Loom", "Loom"), ("Loom.exe", "Loom"), ("ScreenFlow", "ScreenFlow"), ("CleanShot X", "CleanShot"), ("Kap", "Kap"), ("Ecamm Live", "Ecamm Live"), // i18n-ok: program names
        ("XSplit.Core.exe", "XSplit"), ("bdcam.exe", "Bandicam"), ("CamtasiaRecorder.exe", "Camtasia"), // i18n-ok: program names
    ];
    /// The indicator's name for a process name, if it is a known capture app.
    pub fn app_of(name: &str) -> Option<&'static str> { APPS.iter().find(|(p, _)| *p == name).map(|(_, n)| *n) }
    /// `ffmpeg -f x11grab …` / `-f gdigrab` / avfoundation screen input is a recording too.
    pub fn grabbing(cmdline: &str) -> bool { cmdline.contains("x11grab") || cmdline.contains("gdigrab") || cmdline.contains("kmsgrab") }

    /// From `pw-dump`'s JSON: a PipeWire video source that is no camera (no device behind it) and is being read now. That is what a
    /// portal screen cast (GNOME, KDE, wlroots, Hyprland) makes while OBS, a browser or a meeting app takes the screen.
    pub fn screencast_in(pw_dump: &str) -> bool {
        let Ok(serde_json::Value::Array(objs)) = serde_json::from_str::<serde_json::Value>(pw_dump) else { return false };
        objs.iter().any(|o| {
            let (info, props) = (&o["info"], &o["info"]["props"]);
            o["type"] == "PipeWire:Interface:Node" && props["media.class"] == "Video/Source" && info["state"] == "running"
                && props.get("device.id").is_none() && props.get("device.api").is_none() && !props["node.name"].as_str().map_or(false, |n| n.starts_with("v4l2_") || n.starts_with("libcamera"))
        })
    }

    /// What can be seen now. `share`: look for screen casts; `apps`: look for capture apps. Blocking (reads /proc, runs `pw-dump`): call it off the UI thread.
    pub fn look(share: bool, apps: bool) -> Seen {
        let mut seen = Seen::default();
        #[cfg(target_os = "linux")] {
            if share { seen.share = std::process::Command::new("pw-dump").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok().filter(|o| o.status.success()).map_or(false, |o| screencast_in(&String::from_utf8_lossy(&o.stdout))); }
            if apps { seen.app = linux_app(); }
        }
        #[cfg(target_os = "macos")] { let _ = share; if apps { seen.app = mac_app(); } }
        #[cfg(windows)] { let _ = share; if apps { seen.app = win_app(); } }
        seen
    }
    #[cfg(target_os = "linux")]
    fn linux_app() -> Option<String> {
        for e in std::fs::read_dir("/proc").ok()?.flatten() {
            let name = e.file_name(); let Some(pid) = name.to_str().filter(|n| n.bytes().all(|b| b.is_ascii_digit())) else { continue };
            let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
            if let Some(a) = app_of(comm.trim_end()) { return Some(a.to_string()) }
            if comm.trim_end() == "ffmpeg" && grabbing(&std::fs::read(format!("/proc/{pid}/cmdline")).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()) { return Some("ffmpeg".into()) }
        }
        None
    }
    #[cfg(target_os = "macos")]
    fn mac_app() -> Option<String> {
        let mut pids = vec![0 as libc::pid_t; 8192];
        // SAFETY: the buffer and its size in bytes are ours; proc_name writes at most the length given.
        let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr() as *mut libc::c_void, (pids.len() * std::mem::size_of::<libc::pid_t>()) as libc::c_int) };
        for &pid in pids.iter().take(n.max(0) as usize) {
            let mut b = [0u8; 256];
            let l = unsafe { libc::proc_name(pid, b.as_mut_ptr() as *mut libc::c_void, b.len() as u32) };
            if l > 0 { if let Some(a) = app_of(&String::from_utf8_lossy(&b[..l as usize])) { return Some(a.to_string()) } }
        }
        None
    }
    #[cfg(windows)]
    fn win_app() -> Option<String> {
        use windows_sys::Win32::{Foundation::{CloseHandle, INVALID_HANDLE_VALUE}, System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS}};
        // SAFETY: a snapshot handle we close; the entry's size is set as the API asks.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0); if snap == INVALID_HANDLE_VALUE { return None }
            let mut e: PROCESSENTRY32W = std::mem::zeroed(); e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found = None; let mut ok = Process32FirstW(snap, &mut e) != 0;
            while ok { let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len()); if let Some(a) = app_of(&String::from_utf16_lossy(&e.szExeFile[..n])) { found = Some(a.to_string()); break } ok = Process32NextW(snap, &mut e) != 0; }
            CloseHandle(snap); found
        }
    }
    /// What auto-detect can see on this platform, as Settings says it.
    pub fn what_auto_sees() -> &'static str {
        /* i18n-ok: drawn through trs by row_el */
        if cfg!(target_os = "linux") { "Linux: a screen shared through the desktop's portal (PipeWire screen cast: GNOME, KDE, wlroots) is seen. A capture that reads X11 directly (OBS's Screen Capture (XSHM), ffmpeg x11grab) is not: turn on the recording-app check below, or switch it on by hand." } // i18n-ok: drawn through trs by row_el
        else if cfg!(target_os = "macos") { "macOS does not tell an app that the screen is being recorded or shared. Only the recording-app check below works here: it sees OBS, Loom, ScreenFlow, CleanShot, Kap and Ecamm while they run." } // i18n-ok: drawn through trs by row_el
        else { "Windows does not tell an app that the screen is being captured. Only the recording-app check below works here: it sees OBS, Streamlabs, XSplit, Bandicam, Camtasia and Loom while they run." } // i18n-ok: drawn through trs by row_el
    }
}

// ---- in the window ---------------------------------------------------------------------------------------------------------------------------

/// The window's streamer state: the hub's identifiers, what the detector saw, why it is on.
#[derive(Default)]
pub struct Ui { pub hub: Ids, pub seen: Seen, /** turned off by hand while auto-detect saw this: off until it sees something else */ pub dismissed: Option<Seen>, pub why: Option<String>, pub asked: Option<std::time::Instant>, pub watching: bool, pub applied: Option<Ids> }
pub const TAG: &str = "privacy";
/// How often the detector looks (it notifies only on a change), and how often the hub's identifiers are asked again while on.
const LOOK_EVERY: std::time::Duration = std::time::Duration::from_secs(4);
const ASK_EVERY: std::time::Duration = std::time::Duration::from_secs(300);

impl crate::Backplane {
    fn streamer_mode(&self) -> Mode { Mode { manual: self.local.streamer, auto_share: self.local.streamer_share, auto_apps: self.local.streamer_apps } }
    /// Masking on or off from the switch and what was seen; every window redrawn when that, or the identifiers, changed.
    pub fn streamer_apply(&mut self, cx: &mut gpui::Context<Self>) {
        let ui = &mut self.screens.streamer;
        if ui.dismissed.as_ref().map_or(false, |d| *d != ui.seen) { ui.dismissed = None; }
        let mode = self.streamer_mode();
        let why = if !mode.manual && self.screens.streamer.dismissed.is_some() { None } else { mode.on(&self.screens.streamer.seen) };
        let ids = why.as_ref().map(|_| { let mut i = Ids::local(); i.merge(self.screens.streamer.hub.clone()); i });
        let ui = &mut self.screens.streamer;
        let changed = ui.why != why || ui.applied != ids;
        if why.is_some() && ui.asked.map_or(true, |t| t.elapsed() > ASK_EVERY) {
            ui.asked = Some(std::time::Instant::now());
            let _ = self.tx.try_send(crate::hub::Command::Rpc("privacy.ids".into(), crate::screens::settings::Secret(serde_json::json!({})), TAG));
        }
        let ui = &mut self.screens.streamer;
        if changed {
            set(ids.as_ref().map(Masker::new)); ui.why = why; ui.applied = ids;
            self.ui_epoch += 1; cx.refresh_windows();
            eprintln!("GPUI_LOG {}", serde_json::json!({"streamer": self.screens.streamer.why.is_some()}));
        }
        if (self.local.streamer_share || self.local.streamer_apps) && !self.screens.streamer.watching { self.streamer_watch(cx) }
    }
    /// The hub's `privacy.ids` answer.
    pub fn streamer_ids(&mut self, v: &serde_json::Value, cx: &mut gpui::Context<Self>) { self.screens.streamer.hub = Ids::from_hub(v); self.streamer_apply(cx); }
    /// A new connection: the hub's identifiers may differ (another hub), so they are asked again.
    pub fn streamer_hello(&mut self, cx: &mut gpui::Context<Self>) { self.screens.streamer.asked = None; if self.screens.streamer.why.is_some() { self.streamer_apply(cx) } }
    /// Look every few seconds while an automatic turn-on is chosen; the loop ends when neither is.
    fn streamer_watch(&mut self, cx: &mut gpui::Context<Self>) {
        self.screens.streamer.watching = true;
        cx.spawn(async move |weak, cx| loop {
            let Ok((share, apps)) = weak.update(cx, |this, _| (this.local.streamer_share, this.local.streamer_apps)) else { return };
            if !share && !apps { let _ = weak.update(cx, |this, cx| { this.screens.streamer.watching = false; this.screens.streamer.seen = Seen::default(); this.streamer_apply(cx) }); return }
            let seen = cx.background_executor().spawn(async move { detect::look(share, apps) }).await;
            if weak.update(cx, |this, cx| if this.screens.streamer.seen != seen { this.screens.streamer.seen = seen; this.streamer_apply(cx) }).is_err() { return }
            cx.background_executor().timer(LOOK_EVERY).await;
        }).detach();
    }
    /// The palette's and the indicator's switch: on by hand, or off. Off while auto-detect sees a capture holds until it sees something
    /// else (the capture ends, or another starts): the person's word wins while that capture lasts.
    pub fn streamer_toggle(&mut self, cx: &mut gpui::Context<Self>) {
        let on = self.screens.streamer.why.is_some();
        self.local.streamer = !on;
        self.screens.streamer.dismissed = on.then(|| self.screens.streamer.seen.clone());
        self.local.save(); self.streamer_apply(cx); cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m() -> Masker {
        Masker::new(&Ids { emails: vec!["helmut@backplaneworks.com".into(), "build@amemb.com".into()], names: vec!["boat-hub".into(), "deskbox".into(), "Helmut Example".into()],
            users: vec!["h".into(), "i2cjak".into()], homes: vec!["/home/h".into()], labels: vec![] })
    }
    fn none_left(out: &str) { for bad in ["helmut", "backplaneworks", "amemb", "boat-hub", "deskbox", "/home/h", "i2cjak", "tail9f3a", "100.64.84.41", "sk-ant-", "K7QP2MXA", "555", "4155550123"] { assert!(!out.to_lowercase().contains(&bad.to_lowercase()), "{bad} left in {out:?}"); } }

    #[test] fn each_identifier_type_is_masked_in_plain_text() {
        let m = m();
        assert_eq!(m.mask("signed in as helmut@backplaneworks.com (max)"), "signed in as •••@••• (max)");
        assert_eq!(m.mask("mail from Someone.Else+tag@example.co.uk today"), "mail from •••@••• today", "any email, not only the person's");
        assert_eq!(m.mask("git@github.com:org/repo"), "git@github.com:org/repo", "a git remote names no person");
        assert_eq!(m.mask("saved to /home/h/Documents/Backplane/x.kicad_pcb"), "saved to ~/Documents/Backplane/x.kicad_pcb");
        assert_eq!(m.mask("/home/someone/.cache"), "~/.cache", "any user's home");
        assert_eq!(m.mask(r"C:\Users\Helmut\AppData"), r"~\AppData");
        assert_eq!(m.mask("/Users/hx/Library"), "~/Library");
        assert_eq!(m.mask("h@deskbox:~$ ls"), "user@machine:~$ ls", "a shell prompt: a short login name only before @");
        assert_eq!(m.mask("the h key, the hub"), "the h key, the hub", "a one-letter login is no word to mask");
        assert_eq!(m.mask("run it on boat-hub, not boat"), "run it on machine, not boat");
        assert_eq!(m.mask("BOAT-HUB is up"), "machine is up", "any case");
        assert_eq!(m.mask("boat-hubs and boat-hub2"), "boat-hubs and boat-hub2", "whole words only");
        assert_eq!(m.mask("commits by i2cjak"), "commits by user");
        let g = Masker::new(&Ids { names: vec!["desktop".into(), "Laptop".into()], ..Default::default() });
        assert_eq!(g.mask("a Desktop app on the laptop"), "a Desktop app on the laptop", "a machine called by a common word names nobody");
        let n = Masker::new(&Ids { names: vec!["100.64.84.41".into(), "tail9e0cf2.ts.net".into()], ..Default::default() });
        assert_eq!(n.mask("ssh build@100.64.84.41 on tail9e0cf2.ts.net"), "ssh build@•.•.•.• on •••.ts.net", "a machine's address looks like any address");
        assert_eq!(m.mask("Helmut Example wrote"), "••• wrote", "a person's name");
        assert_eq!(m.mask("open https://boat-hub.tail9f3a.ts.net:8443/x"), "open https://•••.ts.net:8443/x");
        assert_eq!(m.mask("hub at 100.64.84.41:3799 and 192.168.1.20"), "hub at •.•.•.• and •.•.•.•");
        assert_eq!(m.mask("listen 127.0.0.1:3787 and 0.0.0.0"), "listen 127.0.0.1:3787 and 0.0.0.0", "loopback names nothing");
        assert_eq!(m.mask("version 1.2.3 and 2026.11.13"), "version 1.2.3 and 2026.11.13");
        assert_eq!(m.mask("addr fd7a:115c:a1e0::5a01:2b3c up"), "addr •.•.•.• up");
        assert_eq!(m.mask("at 12:30:45 use std::sync::Arc"), "at 12:30:45 use std::sync::Arc");
        assert_eq!(m.mask("key sk-ant-api03-AbCdEfGhIjKlMnOpQrSt end"), "key ••• end");
        assert_eq!(m.mask("ghp_0123456789abcdef0123 and tskey-auth-kAbc123456789012"), "••• and •••");
        assert_eq!(m.mask("Authorization: Bearer abcdefghijklmnop1234"), "Authorization: Bearer •••");
        assert_eq!(m.mask("sk-learn is a library"), "sk-learn is a library", "short tokens are words");
        assert_eq!(m.mask("http://h:3799/pair#invite=Zx81kQ_secret&x=1"), "http://h:3799/pair#invite=•••&x=1");
        assert_eq!(m.mask("https://x/?a=1&token=abc123"), "https://x/?a=1&token=•••");
        assert_eq!(m.mask("call +1 415 555 0123 or (415) 555-0123 or 415-555-0123"), "call ••• or ••• or •••");
        assert_eq!(m.mask("+49 30 1234567"), "•••");
        assert_eq!(m.mask("R1 4700, C3 100nF, 0603, LM358-DR"), "R1 4700, C3 100nF, 0603, LM358-DR", "part values are no phone numbers");
        assert_eq!(m.mask("Short code K7QP2MXA, for ten minutes"), "Short code •••, for ten minutes");
        assert_eq!(m.mask("Code: ABCD-EFGH"), "Code: •••");
        assert_eq!(m.mask("the code is fine"), "the code is fine");
    }
    #[test] fn the_same_identifiers_in_markdown_are_masked_in_every_piece_drawn() {
        let m = m();
        let md = "**Signed in** as `helmut@backplaneworks.com` on _boat-hub_.\n\n- path: [/home/h/x](file:///home/h/x)\n- ip: `100.64.84.41`\n\n```\nh@deskbox:~$ cat ~/.ssh/config\n```\n> mail Someone@example.org or +1 415 555 0123";
        none_left(&m.mask(md));
        // as the client draws it: the hub's markdown nodes become spans, each drawn as its own text
        let nodes: Vec<serde_json::Value> = serde_json::from_str(r#"[{"tag":"p","kids":[{"tag":"strong","kids":[{"text":"Signed in"}]},{"text":" as "},{"tag":"code","kids":[{"text":"helmut@backplaneworks.com"}]},{"text":" on "},{"tag":"em","kids":[{"text":"boat-hub"}]},{"text":"."}]},
            {"tag":"ul","kids":[{"tag":"li","kids":[{"text":"path: "},{"tag":"a","href":"file:///home/h/x","kids":[{"text":"/home/h/x"}]}]},{"tag":"li","kids":[{"text":"ip: "},{"tag":"code","kids":[{"text":"100.64.84.41"}]}]}]},
            {"tag":"pre","kids":[{"tag":"code","kids":[{"text":"h@deskbox:~$ cat ~/.ssh/config"}]}]}]"#).unwrap();
        for b in crate::screens::md::parse(&nodes) { let t = crate::screens::md::block_text(&b); none_left(&m.mask(&t)); }
        for n in &nodes { for s in crate::screens::md::spans(std::slice::from_ref(n)) { none_left(&m.mask(&s.text)); } }
    }
    #[test] fn an_accounts_label_is_account_n_everywhere_and_a_providers_name_stays() {
        let m = Masker::new(&Ids { labels: vec!["Work".into(), "Claude".into(), "helmut-max".into()], ..Default::default() });
        assert_eq!(m.mask("Account: helmut-max · Work"), "Account: account 3 · account 1");
        assert_eq!(m.mask("Claude on Workshop, work"), "Claude on Workshop, work", "whole words, exact case; a provider's name is no label");
    }
    #[test] fn gpuis_index_map_turns_indices_both_ways_around_replaced_pieces() {
        // "ab[cdef]gh[ij]k" drawn "ab[X]gh[YYYY]k": a click on the drawn text lands on the original's index, a link's range is drawn where it is
        let m = gpui::TextMap(vec![(2..6, 2..3), (8..10, 5..9)]);
        assert_eq!([0, 2, 4, 6, 8, 10, 11].map(|i| m.to_shown(i)), [0, 2, 2, 3, 5, 9, 10]);
        assert_eq!([0, 2, 3, 5, 7, 9, 10].map(|i| m.to_orig(i)), [0, 2, 6, 8, 8, 10, 11]);
    }
    #[test] fn edits_are_sorted_apart_and_on_character_boundaries() {
        let m = m();
        let s = "ünï /home/h/ä helmut@backplaneworks.com — boat-hub 100.64.84.41 日本";
        let e = m.edits(s);
        for w in e.windows(2) { assert!(w[0].0.end <= w[1].0.start) }
        for (r, _) in &e { assert!(s.is_char_boundary(r.start) && s.is_char_boundary(r.end)) }
        assert_eq!(apply(s, &e), "ünï ~/ä •••@••• — machine •.•.•.• 日本");
    }
    #[test] fn same_width_keeps_every_offset() {
        let m = m();
        for s in ["cd /home/h/x && ssh h@deskbox", "ä helmut@backplaneworks.com ü", "nothing here"] {
            assert_eq!(m.same_width(s, false).len(), s.len(), "{s}"); assert_eq!(m.same_width(s, true).chars().count(), s.chars().count(), "{s}");
            none_left(&m.same_width(s, false));
        }
        assert_eq!(m.same_width("ssh h@deskbox", true), "ssh *@*******");
    }
    #[test] fn masking_twice_changes_nothing_more() {
        let m = m();
        for s in ["helmut@backplaneworks.com at /home/h on boat-hub", "Short code K7QP2MXA", "+1 415 555 0123"] { let once = m.mask(s); assert_eq!(m.mask(&once), once); }
    }
    #[test] fn on_is_the_switch_or_what_auto_detect_saw() {
        let seen = Seen { share: true, app: Some("obs".into()) };
        assert_eq!(Mode::default().on(&seen), None);
        assert_eq!(Mode { manual: true, ..Default::default() }.on(&Seen::default()), Some(String::new()));
        assert_eq!(Mode { auto_share: true, ..Default::default() }.on(&seen).as_deref(), Some("screen shared"));
        assert_eq!(Mode { auto_apps: true, ..Default::default() }.on(&seen).as_deref(), Some("obs"));
        assert_eq!(Mode { auto_apps: true, ..Default::default() }.on(&Seen::default()), None);
    }
    #[test] fn ids_merge_without_duplicates_and_read_the_hubs_answer() {
        let mut a = Ids::from_hub(&serde_json::json!({"emails":["A@x.com"],"names":["boat-hub"],"users":["h"],"homes":["/home/h"]}));
        a.merge(Ids { emails: vec!["a@x.com".into(), "b@y.com".into()], names: vec![" ".into()], ..Default::default() });
        assert_eq!(a.emails, vec!["A@x.com", "b@y.com"]); assert_eq!(a.names, vec!["boat-hub"]);
    }
    #[test] fn a_screen_cast_is_a_running_video_source_with_no_device_behind_it() {
        let node = |class: &str, state: &str, extra: &str| format!(r#"{{"type":"PipeWire:Interface:Node","info":{{"state":"{state}","props":{{"media.class":"{class}","node.name":"x"{extra}}}}}}}"#);
        assert!(detect::screencast_in(&format!("[{}]", node("Video/Source", "running", ""))));
        assert!(!detect::screencast_in(&format!("[{}]", node("Video/Source", "suspended", ""))), "a cast nobody reads");
        assert!(!detect::screencast_in(&format!("[{}]", node("Video/Source", "running", r#","device.api":"v4l2""#))), "a camera");
        assert!(!detect::screencast_in(&format!("[{}]", node("Audio/Source", "running", ""))));
        assert!(!detect::screencast_in("not json"));
    }
    #[test] fn capture_apps_are_known_by_process_name() {
        assert_eq!(detect::app_of("obs"), Some("OBS")); assert_eq!(detect::app_of("obs64.exe"), Some("OBS")); assert_eq!(detect::app_of("gpu-screen-reco"), Some("GPU Screen Recorder"));
        assert_eq!(detect::app_of("firefox"), None); assert!(detect::grabbing("ffmpeg\0-f\0x11grab\0-i\0:0")); assert!(!detect::grabbing("ffmpeg -i in.mp4 out.webm"));
    }
}

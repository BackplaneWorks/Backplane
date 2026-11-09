//! The client's language: the catalogs `i18n/en.msg` (English, the source and the key list) and `i18n/ja.msg`, read once from the copies
//! compiled in, and a device-local choice (Automatic, English, 日本語; Automatic follows the system language).
//!
//! Three ways in, all pure and cached:
//!   * `t(key)` / `tf(key, args)`: a message by its key, in the language in force (English when the catalog has no entry).
//!   * `tr(text)`: text written in English in the code, or sent by the hub for display, found in the catalog by its English words (an exact
//!     message, or a message with `{arguments}`, `plural` and `select` matched as a template and rendered in the language in force with what
//!     it captured). Text the catalog does not know (an agent's words, a file name, a title) comes back unchanged.
//!   * `Lang`: the choice itself, kept in `local::Local::lang` and applied with `set_choice`.
//! English is never touched: `tr` returns its argument. Only chrome goes through `tr`; content never does.
use std::{borrow::Cow, collections::HashMap, sync::{atomic::{AtomicU8, Ordering}, Mutex, OnceLock}};

pub const EN_MSG: &str = include_str!("../../../i18n/en.msg");
pub const JA_MSG: &str = include_str!("../../../i18n/ja.msg");

/// The choices, as kept in the local settings file and shown in Settings: (value, catalog key of its label).
pub const CHOICES: [(&str, &str); 3] = [("auto", "settings.lang.auto"), ("en", "settings.lang.en"), ("ja", "settings.lang.ja")];

// ---- the message grammar -----------------------------------------------------------------------------------------------------------------
#[derive(Clone, Debug, PartialEq)]
enum Node { Text(String), Arg(String, bool), Hash, Plural(String, Vec<(Sel, Vec<Node>)>), Select(String, Vec<(String, Vec<Node>)>) }
#[derive(Clone, Debug, PartialEq)]
enum Sel { Exact(u64), Cat(String) }

/// An argument of a message.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg { Num(u64), Str(String) }
impl From<u64> for Arg { fn from(n: u64) -> Arg { Arg::Num(n) } }
impl From<usize> for Arg { fn from(n: usize) -> Arg { Arg::Num(n as u64) } }
impl From<u32> for Arg { fn from(n: u32) -> Arg { Arg::Num(n as u64) } }
impl From<i32> for Arg { fn from(n: i32) -> Arg { Arg::Num(n.max(0) as u64) } }
impl From<&str> for Arg { fn from(s: &str) -> Arg { Arg::Str(s.to_string()) } }
impl From<String> for Arg { fn from(s: String) -> Arg { Arg::Str(s) } }
impl From<&String> for Arg { fn from(s: &String) -> Arg { Arg::Str(s.clone()) } }

fn parse(src: &str) -> Vec<Node> { let c: Vec<char> = src.chars().collect(); let mut i = 0; seq(&c, &mut i, false) }
/// Nodes up to the `}` that closes the enclosing block (not consumed) or the end.
fn seq(c: &[char], i: &mut usize, nested: bool) -> Vec<Node> {
    let mut out = Vec::new(); let mut text = String::new();
    macro_rules! flush { () => { if !text.is_empty() { out.push(Node::Text(std::mem::take(&mut text))); } } }
    while *i < c.len() {
        match c[*i] {
            '}' if nested => break,
            '\\' if *i + 1 < c.len() => { *i += 1; text.push(match c[*i] { 's' => ' ', 'n' => '\n', o => o }); *i += 1; }
            '\'' => {
                if c.get(*i + 1) == Some(&'\'') { text.push('\''); *i += 2; }
                else if matches!(c.get(*i + 1), Some('{' | '}' | '#' | '|')) {
                    *i += 1; while *i < c.len() && c[*i] != '\'' { text.push(c[*i]); *i += 1; } *i += 1;
                } else { text.push('\''); *i += 1; }
            }
            '#' if nested => { flush!(); out.push(Node::Hash); *i += 1; }
            '{' => { flush!(); *i += 1; out.push(argument(c, i)); }
            ch => { text.push(ch); *i += 1; }
        }
    }
    flush!(); out
}
/// After the `{`: `name}`, `name, number}`, `name, plural, ...}` or `name, select, ...}`; leaves `i` past the closing `}`.
fn argument(c: &[char], i: &mut usize) -> Node {
    let word = |i: &mut usize| { let mut s = String::new(); while *i < c.len() && !matches!(c[*i], ',' | '}') { s.push(c[*i]); *i += 1; } s.trim().to_string() };
    let name = word(i);
    if c.get(*i) == Some(&'}') { *i += 1; return Node::Arg(name, false); }
    *i += 1; let kind = word(i);
    if kind == "number" { if c.get(*i) == Some(&',') { *i += 1; word(i); } *i += 1; return Node::Arg(name, true); }
    *i += 1; // the comma before the branches
    let (mut plural, mut select) = (Vec::new(), Vec::new());
    loop {
        while *i < c.len() && c[*i].is_whitespace() { *i += 1; }
        if *i >= c.len() || c[*i] == '}' { *i += 1; break }
        let mut sel = String::new(); while *i < c.len() && !c[*i].is_whitespace() && c[*i] != '{' { sel.push(c[*i]); *i += 1; }
        while *i < c.len() && c[*i].is_whitespace() { *i += 1; }
        if c.get(*i) != Some(&'{') { *i += 1; continue }
        *i += 1; let body = seq(c, i, true); *i += 1; // the branch's `}`
        if kind == "plural" { plural.push((match sel.strip_prefix('=').and_then(|n| n.parse().ok()) { Some(n) => Sel::Exact(n), None => Sel::Cat(sel) }, body)); } else { select.push((sel, body)); }
    }
    if kind == "plural" { Node::Plural(name, plural) } else { Node::Select(name, select) }
}

fn group(n: u64) -> String { let s = n.to_string(); let mut out = String::new(); for (k, ch) in s.chars().enumerate() { if k > 0 && (s.len() - k) % 3 == 0 { out.push(','); } out.push(ch); } out }
/// English plural categories (Japanese has only `other`; its messages say so).
fn category(n: u64) -> &'static str { if n == 1 { "one" } else { "other" } }

fn render(nodes: &[Node], args: &[(&str, Arg)], hash: Option<u64>, out: &mut String) {
    for n in nodes { match n {
        Node::Text(t) => out.push_str(t),
        Node::Hash => if let Some(h) = hash { out.push_str(&group(h)) } else { out.push('#') },
        Node::Arg(name, number) => match args.iter().find(|(k, _)| k == name).map(|(_, v)| v) {
            Some(Arg::Num(v)) => out.push_str(&if *number { group(*v) } else { v.to_string() }),
            Some(Arg::Str(s)) => out.push_str(s), None => {} },
        Node::Plural(name, branches) => {
            let v = match args.iter().find(|(k, _)| k == name).map(|(_, v)| v) { Some(Arg::Num(v)) => *v, Some(Arg::Str(s)) => s.parse().unwrap_or(0), None => 0 };
            let pick = branches.iter().find(|(s, _)| *s == Sel::Exact(v)).or_else(|| branches.iter().find(|(s, _)| *s == Sel::Cat(category(v).into()))).or_else(|| branches.iter().find(|(s, _)| *s == Sel::Cat("other".into())));
            if let Some((_, body)) = pick { render(body, args, Some(v), out); }
        }
        Node::Select(name, branches) => {
            let v = match args.iter().find(|(k, _)| k == name).map(|(_, v)| v) { Some(Arg::Str(s)) => s.clone(), Some(Arg::Num(n)) => n.to_string(), None => String::new() };
            if let Some((_, body)) = branches.iter().find(|(s, _)| *s == v).or_else(|| branches.iter().find(|(s, _)| s == "other")) { render(body, args, hash, out); }
        }
    } }
}

// ---- matching English text against a message (the reverse of render) ------------------------------------------------------------------------
type Caps = Vec<(String, Arg)>;
/// The plural a `#` belongs to: its argument's name and what its number must be for the branch it is in.
type Hash<'a> = Option<(&'a str, &'a dyn Fn(u64) -> bool)>;
/// Whether `text` is exactly what `nodes` render in English for some arguments, pushing those arguments on `caps`. Backtracking, longest number
/// and shortest capture first; `k` is what must hold of the rest (the continuation).
fn matches(nodes: &[Node], text: &str, hash: Hash, caps: &mut Caps, k: &mut dyn FnMut(&str, &mut Caps) -> bool) -> bool {
    let Some((first, rest)) = nodes.split_first() else { return k(text, caps) };
    match first {
        Node::Text(t) => text.strip_prefix(t.as_str()).map_or(false, |r| matches(rest, r, hash, caps, k)),
        Node::Hash => {
            let Some((name, want)) = hash else { return text.strip_prefix('#').map_or(false, |r| matches(rest, r, hash, caps, k)) };
            for (n, r) in number_prefix(text) { if want(n) { caps.push((name.to_string(), Arg::Num(n))); if matches(rest, r, hash, caps, k) { return true } caps.pop(); } }
            false
        }
        Node::Arg(name, true) => {
            for (n, r) in number_prefix(text) { caps.push((name.clone(), Arg::Num(n))); if matches(rest, r, hash, caps, k) { return true } caps.pop(); }
            false
        }
        Node::Arg(name, false) => {
            for end in text.char_indices().map(|(i, c)| i + c.len_utf8()) {
                caps.push((name.clone(), Arg::Str(text[..end].to_string())));
                if matches(rest, &text[end..], hash, caps, k) { return true } caps.pop();
            }
            false
        }
        Node::Plural(name, branches) => branches.iter().any(|(sel, body)| {
            let want = |n: u64| match sel { Sel::Exact(e) => n == *e, Sel::Cat(c) => c == category(n) && (c != "other" || !branches.iter().any(|(s, _)| *s == Sel::Exact(n))) };
            let mut kk = |r: &str, c: &mut Caps| matches(rest, r, hash, c, k);
            if has_hash(body) { matches(body, text, Some((name.as_str(), &want)), caps, &mut kk) }
            else {
                // a branch with no `#` (`=0 {None}`, `one {One}`) stands for its own number
                let n = match sel { Sel::Exact(e) => Some(*e), Sel::Cat(c) if c == "one" => Some(1), _ => None };
                caps.push((name.clone(), Arg::Num(n.unwrap_or(0))));
                if matches(body, text, None, caps, &mut kk) { true } else { caps.pop(); false }
            }
        }),
        Node::Select(name, branches) => branches.iter().any(|(key, body)| {
            caps.push((name.clone(), Arg::Str(key.clone())));
            let mut kk = |r: &str, c: &mut Caps| matches(rest, r, hash, c, k);
            if matches(body, text, hash, caps, &mut kk) { true } else { caps.pop(); false }
        }),
    }
}
fn has_hash(nodes: &[Node]) -> bool { nodes.iter().any(|n| match n { Node::Hash => true, Node::Plural(_, bs) => bs.iter().any(|(_, b)| has_hash(b)), Node::Select(_, bs) => bs.iter().any(|(_, b)| has_hash(b)), _ => false }) }
/// The digit run (commas allowed) at the start of `text`, with every shorter prefix too: (value, rest).
fn number_prefix(text: &str) -> impl Iterator<Item = (u64, &str)> {
    let len = text.char_indices().take_while(|(_, c)| c.is_ascii_digit() || *c == ',').last().map_or(0, |(i, _)| i + 1);
    let digits = &text[..len];
    (1..=len).rev().filter(move |n| digits.as_bytes()[n - 1].is_ascii_digit()).filter_map(move |n| digits[..n].replace(',', "").parse::<u64>().ok().map(|v| (v, &text[n..])))
}

// ---- the catalogs -------------------------------------------------------------------------------------------------------------------------
/// Keys the reverse lookup never offers: their English is a frame with a free slot that would swallow other sentences.
const NO_REVERSE: &[&str] = &["proj.addcand", "native.hub.connection", "native.task.parameter", "secret.asks", "voice.model.ready", "android.quiet.range"];
pub struct Catalog {
    en: HashMap<String, Vec<Node>>, ja: HashMap<String, Vec<Node>>,
    /// English text of a message with no argument → its key.
    exact: HashMap<String, String>,
    /// Messages with arguments, the longest fixed text first (the most specific wins): key and the fixed text's length.
    templates: Vec<(String, usize)>,
}
fn entries(src: &str) -> Vec<(String, String)> {
    src.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).filter_map(|l| l.split_once(" = ").map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))).collect()
}
fn fixed_len(nodes: &[Node]) -> usize { nodes.iter().map(|n| match n { Node::Text(t) => t.chars().count(), Node::Plural(_, bs) => bs.iter().map(|(_, b)| fixed_len(b)).min().unwrap_or(0), Node::Select(_, bs) => bs.iter().map(|(_, b)| fixed_len(b)).min().unwrap_or(0), _ => 0 }).sum() }
fn plain(nodes: &[Node]) -> Option<String> { let mut s = String::new(); for n in nodes { match n { Node::Text(t) => s.push_str(t), _ => return None } } Some(s) }

impl Catalog {
    pub fn from(en: &str, ja: &str) -> Catalog {
        let en: HashMap<String, Vec<Node>> = entries(en).into_iter().map(|(k, v)| (k, parse(&v))).collect();
        let ja: HashMap<String, Vec<Node>> = entries(ja).into_iter().map(|(k, v)| (k, parse(&v))).collect();
        let mut exact: HashMap<String, String> = HashMap::new(); let mut templates = Vec::new();
        let mut keys: Vec<&String> = en.keys().collect(); keys.sort();
        for k in keys {
            // messages too short to be anyone's sentence (`{from} to {to}`, `Add {name}`) are for `t()` by key, never for a stranger's text
            if NO_REVERSE.contains(&k.as_str()) { continue }
            match plain(&en[k]) {
                // two keys with one English text (`common.delete` and a thread's Delete): the first key in order whose Japanese is not English wins
                Some(t) => { if !t.is_empty() { exact.entry(t).or_insert_with(|| k.clone()); } }
                None => { let n = fixed_len(&en[k]); if n >= 3 { templates.push((k.clone(), n)); } }
            }
        }
        templates.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        Catalog { en, ja, exact, templates }
    }
    pub fn keys(&self) -> Vec<&String> { let mut k: Vec<&String> = self.en.keys().collect(); k.sort(); k }
    pub fn has(&self, key: &str, lang_ja: bool) -> bool { if lang_ja { self.ja.contains_key(key) } else { self.en.contains_key(key) } }
    pub fn message(&self, key: &str, ja: bool, args: &[(&str, Arg)]) -> Option<String> {
        let nodes = if ja { self.ja.get(key).or_else(|| self.en.get(key))? } else { self.en.get(key)? };
        let mut s = String::new(); render(nodes, args, None, &mut s); Some(s)
    }
    /// The key of the message that says exactly this in English, with the arguments it captured.
    pub fn find(&self, text: &str) -> Option<(String, Caps)> {
        if let Some(k) = self.exact.get(text) { return Some((k.clone(), Vec::new())) }
        for (k, _) in &self.templates {
            let mut caps = Caps::new();
            if matches(&self.en[k], text, None, &mut caps, &mut |rest, _| rest.is_empty()) { return Some((k.clone(), caps)) }
        }
        None
    }
    /// `find`, but a message whose key starts with `prefix` wins when several say the same English (a screen knows which of them it means).
    pub fn find_pref(&self, text: &str, prefix: &str) -> Option<(String, Caps)> {
        if let Some(k) = self.exact.get(text) { return Some((k.clone(), Vec::new())) }
        for (k, _) in self.templates.iter().filter(|(k, _)| k.starts_with(prefix)) {
            let mut caps = Caps::new();
            if matches(&self.en[k], text, None, &mut caps, &mut |rest, _| rest.is_empty()) { return Some((k.clone(), caps)) }
        }
        self.find(text)
    }
    /// `tr` for a screen that means the messages under `prefix` (`history.`, `gpui.imp.`).
    pub fn trp(text: &str, prefix: &str) -> String {
        if !is_ja() { return text.to_string() }
        let c = catalog();
        match c.find_pref(text, prefix) { Some((k, caps)) => { let args: Vec<(&str, Arg)> = caps.iter().map(|(n, v)| (n.as_str(), v.clone())).collect(); c.message(&k, true, &args).unwrap_or_else(|| text.to_string()) } None => trs(text) }
    }
    pub fn japanese(&self, text: &str) -> Option<String> {
        let (k, caps) = self.find(text)?;
        let args: Vec<(&str, Arg)> = caps.iter().map(|(n, v)| (n.as_str(), v.clone())).collect();
        self.message(&k, true, &args)
    }
}
pub fn catalog() -> &'static Catalog { static C: OnceLock<Catalog> = OnceLock::new(); C.get_or_init(|| Catalog::from(EN_MSG, JA_MSG)) }

// ---- the language in force ----------------------------------------------------------------------------------------------------------------
const AUTO: u8 = 0; const EN: u8 = 1; const JA: u8 = 2;
/// A test reads English unless it says otherwise (`with_lang`), whatever language the machine it runs on speaks.
static CHOICE: AtomicU8 = AtomicU8::new(if cfg!(test) { EN } else { AUTO });
#[cfg(test)]
thread_local! { static ONLY_HERE: std::cell::Cell<Option<u8>> = const { std::cell::Cell::new(None) }; }
/// Run `f` with this thread's language "en" or "ja" (other test threads keep theirs).
#[cfg(test)]
pub fn with_lang<R>(lang: &str, f: impl FnOnce() -> R) -> R {
    let prev = ONLY_HERE.with(|c| c.replace(Some(if lang == "ja" { JA } else { EN })));
    if let Ok(mut c) = cache().lock() { c.clear(); }
    let r = f();
    ONLY_HERE.with(|c| c.set(prev)); if let Ok(mut c) = cache().lock() { c.clear(); }
    r
}
fn current() -> u8 {
    #[cfg(test)] if let Some(v) = ONLY_HERE.with(|c| c.get()) { return v }
    CHOICE.load(Ordering::Relaxed)
}
fn cache() -> &'static Mutex<HashMap<String, String>> { static C: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new(); C.get_or_init(Default::default) }

/// The device's choice: "auto", "en" or "ja" (anything else is "auto"). Clears what was translated.
pub fn set_choice(v: &str) { CHOICE.store(match v { "en" => EN, "ja" => JA, _ => AUTO }, Ordering::Relaxed); if let Ok(mut c) = cache().lock() { c.clear(); } }
pub fn choice() -> &'static str { match current() { EN => "en", JA => "ja", _ => "auto" } }
/// Whether the interface is in Japanese now: the choice, and for Automatic the system's language.
pub fn is_ja() -> bool { match current() { JA => true, EN => false, _ => system_is_ja() } }
pub fn tag() -> &'static str { if is_ja() { "ja" } else { "en" } }

/// Whether the system's language is Japanese. The environment first (LC_ALL, LC_MESSAGES, LANGUAGE, LANG: Linux and a terminal start), else the
/// platform's own setting: macOS's AppleLanguages (an app started from the Dock has no LANG), Windows' user locale name.
pub fn system_is_ja() -> bool { static S: OnceLock<bool> = OnceLock::new(); *S.get_or_init(|| language_is_ja(&system_language())) }
pub fn language_is_ja(tag: &str) -> bool { let t = tag.trim().to_ascii_lowercase(); t == "ja" || t.starts_with("ja_") || t.starts_with("ja-") || t.starts_with("ja.") || t.starts_with("ja@") }
fn system_language() -> String {
    for k in ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"] {
        if let Some(v) = std::env::var_os(k).map(|v| v.to_string_lossy().to_string()).filter(|v| !v.is_empty() && v != "C" && v != "POSIX") {
            // LANGUAGE is a list ("ja:en"): its first entry
            return v.split(':').next().unwrap_or("").to_string()
        }
    }
    platform_language()
}
#[cfg(target_os = "macos")]
fn platform_language() -> String {
    // `defaults read -g AppleLanguages` prints a plist array: ( "ja-JP", "en-US" )
    std::process::Command::new("defaults").args(["read", "-g", "AppleLanguages"]).output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string()).and_then(|s| s.split('"').nth(1).map(str::to_string)).unwrap_or_default()
}
#[cfg(target_os = "windows")]
fn platform_language() -> String {
    let mut buf = [0u16; 85];
    let n = unsafe { windows_sys::Win32::Globalization::GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
    if n > 1 { String::from_utf16_lossy(&buf[..n as usize - 1]) } else { String::new() }
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_language() -> String { String::new() }

/// A message by its key in the language in force; the key itself when the catalog has none (a test refuses that).
pub fn t(key: &str) -> String { tf(key, &[]) }
pub fn tf(key: &str, args: &[(&str, Arg)]) -> String { catalog().message(key, is_ja(), args).unwrap_or_else(|| key.to_string()) }

/// English text, drawn in the language in force. Unknown text comes back unchanged.
pub fn tr(text: &str) -> Cow<'_, str> {
    if text.is_empty() || !is_ja() { return Cow::Borrowed(text) }
    if let Some(hit) = cache().lock().ok().and_then(|c| c.get(text).cloned()) { return if hit.is_empty() { Cow::Borrowed(text) } else { Cow::Owned(hit) } }
    let out = translate(text);
    if out.is_none() { miss(text); }
    if let Ok(mut c) = cache().lock() { if c.len() > 8000 { c.clear(); } c.insert(text.to_string(), out.clone().unwrap_or_default()); }
    match out { Some(s) => Cow::Owned(s), None => Cow::Borrowed(text) }
}
/// `tr` for places that want an owned String.
pub fn trs(text: &str) -> String { tr(text).into_owned() }
/// `BACKPLANE_GPUI_I18N_MISSES=<file>`: every English text that came to `tr` in Japanese and was not in the catalog is appended there once (a rig walks the screens
/// and reads the list; nothing is written otherwise). Words with no letter (numbers, paths) are not asked about.
fn miss(text: &str) {
    static FILE: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    static SEEN: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    let Some(p) = FILE.get_or_init(|| std::env::var_os("BACKPLANE_GPUI_I18N_MISSES").filter(|v| !v.is_empty()).map(Into::into)) else { return };
    if !text.chars().any(|c| c.is_ascii_alphabetic()) || text.chars().any(|c| c as u32 >= 0x3000) { return } // (already in Japanese)
    let seen = SEEN.get_or_init(Default::default);
    if seen.lock().map_or(true, |mut s| !s.insert(text.to_string())) { return }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) { let _ = writeln!(f, "{}", text.replace('\n', "\\n")); }
}
fn translate(text: &str) -> Option<String> {
    let c = catalog();
    if let Some(s) = c.japanese(text) { return Some(s) }
    // a trailing ellipsis, colon or period the catalog's message may not carry, and the label of a count
    let t = text.trim_end_matches(['…', ':', '.', '。']);
    if t.len() != text.len() && !t.is_empty() { if let Some(s) = c.japanese(t) { return Some(format!("{s}{}", &text[t.len()..])) } }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cat() -> Catalog { Catalog::from(EN_MSG, JA_MSG) }
    #[test] fn a_message_with_plural_select_and_number_renders_in_both_languages() {
        let c = Catalog::from("a.n = {n, plural, =0 {None} one {# thread} other {# threads}}\na.who = {who, select, you {You} agent {Agent} other {Someone}}\na.big = {n, number} more\na.q = don''t '{x}' \\sgo\\n", "a.n = {n}件\na.who = {who, select, you {自分} other {不明}}\na.big = さらに{n, number}件\na.q = 行く");
        let m = |k: &str, ja: bool, a: &[(&str, Arg)]| c.message(k, ja, a).unwrap();
        assert_eq!(m("a.n", false, &[("n", 0.into())]), "None"); assert_eq!(m("a.n", false, &[("n", 1.into())]), "1 thread"); assert_eq!(m("a.n", false, &[("n", 2500.into())]), "2,500 threads");
        assert_eq!(m("a.n", true, &[("n", 3.into())]), "3件");
        assert_eq!(m("a.who", false, &[("who", "agent".into())]), "Agent"); assert_eq!(m("a.who", false, &[("who", "zzz".into())]), "Someone"); assert_eq!(m("a.who", true, &[("who", "you".into())]), "自分");
        assert_eq!(m("a.big", false, &[("n", 12345.into())]), "12,345 more");
        assert_eq!(m("a.q", false, &[]), "don't {x}  go\n");
    }
    #[test] fn english_text_is_found_in_the_catalog_whole_or_as_a_template_and_comes_back_in_japanese() {
        let c = Catalog::from("x.cancel = Cancel\nx.count = {n, plural, one {# thread} other {# threads}}\nx.del = Delete {title}?\nx.by = {who, select, you {You} other {Someone}} wrote this\nx.files = {n, plural, =0 {No files changed} one {# file changed} other {# files changed}}\n",
            "x.cancel = キャンセル\nx.count = スレッド{n}件\nx.del = 「{title}」を削除しますか？\nx.by = {who, select, you {自分} other {他の人}}が書いた\nx.files = {n, plural, =0 {変更なし} other {変更ファイル#件}}\n");
        assert_eq!(c.japanese("Cancel").as_deref(), Some("キャンセル"));
        assert_eq!(c.japanese("1 thread").as_deref(), Some("スレッド1件")); assert_eq!(c.japanese("12 threads").as_deref(), Some("スレッド12件"));
        assert_eq!(c.japanese("Delete my plan?").as_deref(), Some("「my plan」を削除しますか？"));
        assert_eq!(c.japanese("You wrote this").as_deref(), Some("自分が書いた")); assert_eq!(c.japanese("Someone wrote this").as_deref(), Some("他の人が書いた"));
        assert_eq!(c.japanese("No files changed").as_deref(), Some("変更なし")); assert_eq!(c.japanese("3 files changed").as_deref(), Some("変更ファイル3件"));
        assert_eq!(c.japanese("1 file changed").as_deref(), Some("変更ファイル1件"));
        assert_eq!(c.japanese("an agent said something").as_deref(), None, "content is not chrome");
        assert_eq!(c.japanese("1 threads").as_deref(), None, "the wrong plural is no message");
    }
    #[test] fn the_real_catalogs_parse_and_the_known_words_translate() {
        let c = cat();
        assert!(c.keys().len() > 1000);
        assert_eq!(c.message("common.cancel", true, &[]).as_deref(), Some("キャンセル")); assert_eq!(c.message("common.cancel", false, &[]).as_deref(), Some("Cancel"));
        assert_eq!(c.japanese("New thread").as_deref(), Some("新しいスレッド")); assert!(c.japanese("Delete my plan?").unwrap().contains("my plan"));
        assert_eq!(c.japanese("2 threads").as_deref(), Some("スレッド2件"));
    }
    /// The release check: every English message has a Japanese one with the same arguments, and no Japanese message is left over.
    #[test] fn every_key_has_a_japanese_entry_and_the_same_arguments() {
        let (en, ja) = (entries(EN_MSG), entries(JA_MSG));
        let jam: HashMap<&String, &String> = ja.iter().map(|(k, v)| (k, v)).collect(); let enm: HashMap<&String, &String> = en.iter().map(|(k, v)| (k, v)).collect();
        let mut bad = Vec::new();
        for (k, v) in &en {
            match jam.get(k) {
                None => bad.push(format!("{k}: no Japanese")),
                Some(j) => { let (a, b) = (names(&parse(v)), names(&parse(j))); if a != b { bad.push(format!("{k}: arguments {a:?} in English, {b:?} in Japanese")); } }
            }
        }
        for (k, _) in &ja { if !enm.contains_key(k) { bad.push(format!("{k}: Japanese without English")); } }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
    fn names(nodes: &[Node]) -> std::collections::BTreeSet<String> {
        let mut s = std::collections::BTreeSet::new();
        for n in nodes { match n { Node::Arg(a, _) => { s.insert(a.clone()); } Node::Plural(a, bs) => { s.insert(a.clone()); for (_, b) in bs { s.extend(names(b)); } } Node::Select(a, bs) => { s.insert(a.clone()); for (_, b) in bs { s.extend(names(b)); } } _ => {} } }
        s
    }
    #[test] fn the_language_in_force_decides_what_tr_returns() {
        assert_eq!(tr("Theme"), "Theme", "English is untouched");
        with_lang("ja", || { assert_eq!(tr("Theme"), "テーマ"); assert_eq!(trs("Open"), "開く"); assert_eq!(tr("a title nobody catalogued"), "a title nobody catalogued"); assert_eq!(tf("gpui.keys.fixed", &[("key", "Esc".into())]), "Esc。固定です。"); assert_eq!(tr("264 px."), "264 pxです。"); });
        assert_eq!(tf("gpui.keys.fixed", &[("key", "Esc".into())]), "Esc. Fixed.");
        with_lang("en", || assert_eq!(tr("Theme"), "Theme"));
    }
    #[test] fn the_system_language_is_read_from_a_locale_tag() {
        for t in ["ja", "ja_JP.UTF-8", "ja-JP", "JA_jp", "ja@x"] { assert!(language_is_ja(t), "{t}"); }
        for t in ["en_US.UTF-8", "jam", "", "C", "de_DE", "zh_CN"] { assert!(!language_is_ja(t), "{t}"); }
    }
}

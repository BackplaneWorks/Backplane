//! Dictation's words (pure; 0.14's core/voice): the plain dictionary, which words of a transcript to ask about, whole-word replacement and how a
//! finished segment reads on from the draft. Lengths and positions are in characters, as the clients count them.
use serde_json::{json, Value};

pub const MAX_KEYWORDS: usize = 100;
pub const MAX_UNSURE: usize = 5;

/// A word to ask about; `guess` is empty when there is no suggestion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsure { pub word: String, pub guess: String }

pub fn chars(s: &str) -> Vec<char> { s.chars().collect() }
pub fn len(s: &str) -> usize { s.chars().count() }
pub fn lower(s: &str) -> String { s.chars().flat_map(char::to_lowercase).collect() }

/// A letter or digit; non-ASCII counts as a letter, except punctuation and symbols (U+2000..U+2BFF).
pub fn alnum(c: char) -> bool { let x = c as u32; c.is_ascii_alphanumeric() || (x >= 192 && !(8192..=11263).contains(&x)) }
/// `-`, `_`, `.`, `'` and `’` join letters inside a word.
fn joiner(c: char) -> bool { matches!(c, '-' | '_' | '.' | '\'' | '’') }
/// The form terms are compared in: trimmed, lower case.
pub fn key(s: &str) -> String { lower(s.trim()) }

// ---- the plain dictionary ----
/// The term is in the dictionary (any case).
pub fn has<S: AsRef<str>>(ds: &[S], term: &str) -> bool { let k = key(term); ds.iter().any(|d| lower(d.as_ref()) == k) }
/// The trimmed term appended, unless blank or already there.
pub fn dict_add(mut ds: Vec<String>, term: &str) -> Vec<String> { let t = term.trim(); if !t.is_empty() && !has(&ds, t) { ds.push(t.to_string()) } ds }
/// Every entry equal to the term (any case) dropped.
pub fn dict_remove(ds: Vec<String>, term: &str) -> Vec<String> { let k = key(term); ds.into_iter().filter(|d| lower(d) != k).collect() }
/// One term per line (\r\n too), trimmed, blanks and repeats (any case) dropped.
pub fn dict_parse(text: &str) -> Vec<String> { text.split('\n').fold(vec![], |ds, l| dict_add(ds, l)) }
pub fn keywords(ds: &[String]) -> &[String] { &ds[..ds.len().min(MAX_KEYWORDS)] }

// ---- edit distance ----
/// Levenshtein distance (exact; callers bound the lengths first).
pub fn lev(a: &str, b: &str) -> usize {
    let b = chars(b); let mut row: Vec<usize> = (0..=b.len()).collect();
    for ca in a.chars() {
        let (mut diag, mut left) = (row[0], row[0] + 1); row[0] = left;
        for (j, cb) in b.iter().enumerate() { let v = (diag + usize::from(ca != *cb)).min(row[j + 1] + 1).min(left + 1); diag = row[j + 1]; row[j + 1] = v; left = v; }
    }
    row[b.len()]
}
/// Within k edits; strings whose lengths differ by more are never walked.
pub fn near(a: &str, b: &str, k: usize) -> bool { len(a).abs_diff(len(b)) <= k && lev(a, b) <= k }
/// How many edits a term tolerates: 1 up to 5 characters, else 2.
fn bound(t: &str) -> usize { if len(t) <= 5 { 1 } else { 2 } }

// ---- dictionary terms, prepared ----
struct Dt { term: String, low: String, spaced: String, flat: String, multi: bool }
/// A space before each capital that follows a lower-case letter ("KiCad" → "Ki Cad").
fn camel(s: &str) -> String { let (mut out, mut lo) = (String::new(), false); for c in s.chars() { if c.is_uppercase() && lo { out.push(' ') } out.push(c); lo = c.is_lowercase(); } out }
pub fn unspace(s: &str) -> String { s.chars().filter(|c| *c != ' ').collect() }
fn dt(d: &str) -> Dt { let sp = lower(&camel(d)); Dt { term: d.into(), low: lower(d), flat: unspace(&sp), multi: sp.contains(' '), spaced: sp } }

// ---- words of a transcript ----
/// A word as it appeared, and whether exactly one space separates it from the word before (only such pairs are read as one term).
struct Tok { w: String, near: bool }
/// Runs of letters and digits, with - _ . ' inside them (not at the ends).
fn tokens(text: &str) -> Vec<Tok> {
    let (mut cur, mut pend, mut sp, mut nr, mut out) = (String::new(), String::new(), false, false, vec![]);
    for c in text.chars() {
        if alnum(c) { if cur.is_empty() { nr = sp; sp = false; pend.clear(); } else { cur.push_str(&pend); pend.clear(); } cur.push(c); }
        else if cur.is_empty() { sp = false }
        else if joiner(c) { pend.push(c) }
        else { sp = pend.is_empty() && c == ' '; pend.clear(); out.push(Tok { w: std::mem::take(&mut cur), near: nr }); nr = false; }
    }
    if !cur.is_empty() { out.push(Tok { w: cur, near: nr }) }
    out
}

// ---- what a word looks like ----
const UNITS: [&str; 8] = ["st", "nd", "rd", "th", "s", "am", "pm", "k"];
/// Capitals that are ordinary words.
const STOP: [&str; 20] = ["I", "A", "OK", "AM", "PM", "US", "UK", "TV", "ID", "PC", "AI", "OS", "FAQ", "FYI", "CEO", "ASAP", "ETA", "BTW", "LOL", "OMG"];
/// A number with an ordinary suffix: 2nd, 1990s, 5pm, 10k.
fn unit(w: &str) -> bool { let rest = w.trim_start_matches(|c: char| c.is_ascii_digit()); rest.len() != w.len() && has(&UNITS, rest) }
/// Looks technical: letters with digits (stm32f4, R12), all capitals (not a stop word), or a capital inside a word (iPhone).
pub fn tech(w: &str) -> bool {
    let (mut alpha, mut digit, mut ups, mut low, mut hump, mut prev) = (false, false, 0, false, false, false);
    for c in w.chars() { let (u, l, d) = (c.is_uppercase(), c.is_lowercase(), c.is_ascii_digit()); alpha |= alnum(c) && !d; digit |= d; ups += usize::from(u); low |= l; hump |= u && prev; prev = l; }
    (alpha && digit && !unit(w)) || (ups >= 2 && !low && !has(&STOP, w)) || hump
}

// ---- clarifications ----
fn one(dts: &[Dt], w: &str) -> Option<String> { dts.iter().find(|d| w != d.low && near(w, &d.low, bound(&d.low))).map(|d| d.term.clone()) }
enum Pair { No, In, Guess(String, String) }
/// A word and the next one, when one space apart ("key cad" ~ KiCad).
fn pair(a: &str, next: Option<&Tok>, dts: &[Dt]) -> Pair {
    let Some(Tok { w: b, near: true }) = next else { return Pair::No };
    let (wd, fl) = (format!("{a} {b}"), lower(&format!("{a}{b}"))); let sp = lower(&wd);
    for d in dts {
        if sp == d.low { return Pair::In }
        if d.multi && (near(&sp, &d.spaced, bound(&d.spaced)) || near(&fl, &d.flat, bound(&d.flat))) { return Pair::Guess(wd, d.term.clone()) }
    }
    Pair::No
}
/// One word: the term it nearly is (3+ characters), else "" when it looks technical, else nothing to ask.
fn single(a: &str, dts: &[Dt]) -> Option<String> { (if len(a) >= 3 { one(dts, &lower(a)) } else { None }).or_else(|| tech(a).then(String::new)) }
/// The words of a transcript to ask about, in order, at most `MAX_UNSURE`, never one in the dictionary or ignored.
pub fn unsure<S: AsRef<str>, T: AsRef<str>>(ds: &[S], ig: &[T], text: &str) -> Vec<Unsure> {
    let (ts, dts): (Vec<Tok>, Vec<Dt>) = (tokens(text), ds.iter().map(|d| dt(d.as_ref())).collect());
    let mut found: Vec<Unsure> = vec![]; let mut skip = false;
    let add = |found: &mut Vec<Unsure>, word: String, guess: String| { let k = key(&word); if !has(ds, &word) && !has(ig, &word) && !found.iter().any(|f| lower(&f.word) == k) { found.push(Unsure { word, guess }) } };
    for (i, t) in ts.iter().enumerate() {
        if std::mem::take(&mut skip) || found.len() >= MAX_UNSURE { continue }
        match pair(&t.w, ts.get(i + 1), &dts) {
            Pair::In => skip = true,
            Pair::Guess(w, g) => { skip = true; add(&mut found, w, g) }
            Pair::No => if let Some(g) = single(&t.w, &dts) { add(&mut found, t.w.clone(), g) },
        }
    }
    found
}

// ---- corrections ----
/// Every whole-word `w` (exact case, may hold spaces) in `t` replaced by `r`; an empty word or `r == w` leaves the text as it is.
pub fn fix(t: &str, w: &str, r: &str) -> String {
    if w.is_empty() || w == r { return t.to_string() }
    let (cs, ws) = (chars(t), chars(w)); let (mut out, mut i) = (String::with_capacity(t.len()), 0);
    while i < cs.len() {
        let hit = cs[i..].starts_with(&ws) && (i == 0 || !alnum(cs[i - 1])) && cs.get(i + ws.len()).map_or(true, |c| !alnum(*c));
        if hit { out.push_str(r); i += ws.len() } else { out.push(cs[i]); i += 1 }
    }
    out
}

// ---- json and prose ----
pub fn unsure_json(us: &[Unsure]) -> Value { Value::Array(us.iter().map(|u| json!({"word": u.word, "guess": u.guess})).collect()) }
/// One line for an agent: Possibly misheard: "key cad" (KiCad?), "stm32f4". Ask if unsure.  ("" when there is nothing to ask)
pub fn note(us: &[Unsure]) -> String {
    if us.is_empty() { return String::new() }
    let items: Vec<String> = us.iter().map(|u| if u.guess.is_empty() { format!("\"{}\"", u.word) } else { format!("\"{}\" ({}?)", u.word, u.guess) }).collect();
    format!("Possibly misheard: {}. Ask if unsure.", items.join(", "))
}

// ---- into the draft ----
/// A segment placed: the whole new text, and where the caret lands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ins { pub text: String, pub caret: usize }
fn space(c: char) -> bool { matches!(c, ' ' | '\t' | '\n' | '\r') }
fn opens(c: char) -> bool { matches!(c, '(' | '[' | '{' | '"' | '“' | '‘') }
/// A space goes before the segment.
pub fn gap(before: &str) -> bool { before.chars().last().map_or(false, |c| !space(c) && !opens(c)) }
/// What comes next starts a sentence: nothing before it but a sentence's end, . ! ? or a new line.
pub fn starts(before: &str) -> bool { before.ends_with('\n') || before.trim_end().chars().last().map_or(true, |c| matches!(c, '.' | '!' | '?')) }
/// The first word is a plain capitalised word.
fn plain(s: &str) -> bool {
    let mut it = s.chars(); if !it.next().map_or(false, |c| c.is_ascii_uppercase()) { return false }
    for c in it { if c.is_ascii_lowercase() || c == '\'' || c == '’' { continue } return !alnum(c) }
    true
}
/// "I", "I'm", "I've": the capital stays.
fn own(s: &str) -> bool { let mut it = s.chars(); it.next() == Some('I') && !it.next().map_or(false, |c| c.is_ascii_lowercase()) }
fn lower1(s: &str) -> String { let mut it = s.chars(); it.next().map_or(String::new(), |c| c.to_lowercase().chain(it).collect()) }
/// The segment as it reads after `before`: mid-sentence its first word loses the capital the transcriber gives every segment, unless it is "I", a dictionary term (`firm`) or not a plain word.
pub fn casing(firm: bool, before: &str, s: &str) -> String { if !starts(before) && !firm && plain(s) && !own(s) { lower1(s) } else { s.to_string() } }
/// The closing period dropped before a lower-case word (an ellipsis stays).
fn unstop(next_lower: bool, s: String) -> String { if next_lower && s.ends_with('.') && !s.ends_with("..") { s[..s.len() - 1].to_string() } else { s } }
/// A segment into the draft at character `at` (past the end means the end); blank leaves it.
pub fn put(firm: bool, draft: &str, at: usize, text: &str) -> Ins {
    let cs = chars(draft); let at = at.min(cs.len()); let seg = text.trim();
    if seg.is_empty() { return Ins { text: draft.to_string(), caret: at } }
    let (before, after): (String, String) = (cs[..at].iter().collect(), cs[at..].iter().collect());
    let s = unstop(after.trim_start().chars().next().map_or(false, |c| c.is_ascii_lowercase()), casing(firm, &before, seg));
    let lead = if gap(&before) { " " } else { "" };
    let tail = if after.chars().next().map_or(false, alnum) { " " } else { "" };
    Ins { caret: at + lead.len() + len(&s), text: format!("{before}{lead}{s}{tail}{after}") }
}
/// The segment opens with a dictionary term, whose capital stays.
pub fn firm<S: AsRef<str>>(ds: &[S], text: &str) -> bool { let s = key(text); ds.iter().any(|h| { let h = lower(h.as_ref()); !h.is_empty() && s.starts_with(&h) }) }

/// An OpenAI key: "sk-", over 20 characters, no spaces or control characters (the only kind the hub keeps).
pub fn openai_ok(k: &str) -> bool { k.starts_with("sk-") && len(k) > 20 && !k.chars().any(|c| c as u32 <= 32 || c as u32 == 127) }
/// The key as a client may see it: its first three and last four characters.
pub fn mask(k: &str) -> String { let n = len(k); if n == 0 { String::new() } else if n < 12 { "set".into() } else { let c = chars(k); format!("{}…{}", c[..3].iter().collect::<String>(), c[n - 4..].iter().collect::<String>()) } }

#[cfg(test)]
mod tests {
    use super::*;
    const D: [&str; 4] = ["KiCad", "Pebble Index", "Backplane", "STM32"];
    const NONE: [&str; 0] = [];
    fn dict() -> Vec<String> { D.iter().map(|s| s.to_string()).collect() }
    fn us(ds: &[&str], ig: &[&str], t: &str) -> String { unsure(ds, ig, t).iter().map(|u| format!("{}={};", u.word, u.guess)).collect() }
    fn p(d: &str, at: usize, t: &str) -> String { let i = put(false, d, at, t); format!("{}|{}", i.text, i.caret) }

    #[test] fn the_dictionary() {
        assert_eq!(dict_parse("  KiCad \r\nPebble Index\n\nkicad\nSTM32\n  \nPEBBLE index\r\n").join("\n"), "KiCad\nPebble Index\nSTM32"); assert!(dict_parse("").is_empty());
        assert!(has(&D, "  pebble INDEX ")); assert!(!has(&D, "pebble"));
        assert_eq!(dict_add(dict(), " Bend ").join("\n"), "KiCad\nPebble Index\nBackplane\nSTM32\nBend");
        assert_eq!(dict_add(dict(), "kicad"), dict()); assert_eq!(dict_add(dict(), "  "), dict());
        assert_eq!(dict_remove(dict(), " backplane").join("\n"), "KiCad\nPebble Index\nSTM32");
        assert_eq!(keywords(&vec!["x".to_string(); 150]).len(), 100); assert_eq!(keywords(&dict()), &dict()[..]);
    }
    #[test] fn edit_distance() { assert_eq!((lev("kitten", "sitting"), lev("", "abc"), lev("abc", "abc")), (3, 3, 0)); }
    #[test] fn which_words_to_ask_about() {
        assert_eq!(us(&D, &NONE, "open it in Backplain now"), "Backplain=Backplane;");
        assert_eq!(us(&["Bend"], &NONE, "the bond and the band"), "bond=Bend;band=Bend;");
        assert_eq!(us(&D, &NONE, "open the key cad board"), "key cad=KiCad;");
        assert_eq!(us(&D, &NONE, "my pebble indexes ring."), "pebble indexes=Pebble Index;");
        assert_eq!(us(&D, &NONE, "the key, cad"), ""); assert_eq!(us(&D, &NONE, "the Pebble Index ring"), "");
        assert_eq!(us(&D, &NONE, "the stm32f4 has R12, an iPhone, a USB port; OK I will at 5pm on the 2nd"), "stm32f4=;R12=;iPhone=;USB=;");
        assert_eq!(us(&D, &NONE, "KiCad and kicad and STM32 in Backplane"), "");
        assert_eq!(us(&D, &["key cad", "R12"], "the key cad has R12 and R13"), "R13=;");
        assert_eq!(us(&D, &NONE, "R12 r12 R12 Backplain backplain"), "R12=;Backplain=Backplane;");
        assert_eq!(us(&D, &NONE, "A1 B2 C3 D4 E5 F6 G7"), "A1=;B2=;C3=;D4=;E5=;");
        assert_eq!(us(&D, &NONE, "(Backplain). it's a v1.2-rc thing..."), "Backplain=Backplane;v1.2-rc=;");
        assert_eq!(us(&D, &NONE, &format!("{}R12", "key cad ".repeat(7000))), "key cad=KiCad;R12=;");
    }
    #[test] fn whole_word_replacement() {
        assert_eq!(fix("open key cad now", "key cad", "KiCad"), "open KiCad now");
        assert_eq!(fix("cad cadence, cad. scad cad", "cad", "CAD"), "CAD cadence, CAD. scad CAD");
        assert_eq!(fix("Cad cad", "cad", "X"), "Cad X"); assert_eq!(fix("a b a", "a", "a"), "a b a"); assert_eq!(fix("a b", "", "z"), "a b");
        assert_eq!(len(&fix(&"key cad ".repeat(7000), "key cad", "KiCad")), 42000);
    }
    #[test] fn json_and_the_note() {
        let u = [Unsure { word: "key cad".into(), guess: "KiCad".into() }, Unsure { word: "R12".into(), guess: "".into() }];
        assert_eq!(unsure_json(&u).to_string(), r#"[{"word":"key cad","guess":"KiCad"},{"word":"R12","guess":""}]"#);
        assert_eq!(note(&[Unsure { word: "key cad".into(), guess: "KiCad".into() }, Unsure { word: "stm32f4".into(), guess: "".into() }]), "Possibly misheard: \"key cad\" (KiCad?), \"stm32f4\". Ask if unsure.");
        assert_eq!(note(&[]), "");
    }
    #[test] fn a_segment_reads_on_from_the_draft() {
        for (d, at, t, want) in [("", 0, " Hello there. ", "Hello there.|12"), ("fix the regulator", 17, "Hello from voice.", "fix the regulator hello from voice.|35"), ("Fix it.", 7, "Then run DRC.", "Fix it. Then run DRC.|21"),
            ("Why? ", 5, "Because", "Why? Because|12"), ("fix it ", 7, "Please", "fix it please|13"), ("fix\n", 4, "Hello", "fix\nHello|9"), ("open board", 5, "The KiCad.", "open the KiCad board|14"),
            ("fix the regulator", 7, "Hello from voice.", "fix the hello from voice regulator|24"), ("fix it. Then go", 7, "Now.", "fix it. Now. Then go|12"), ("open, then", 4, "The board", "open the board, then|14"),
            ("so", 2, "I think.", "so I think.|11"), ("so", 2, "I'm in.", "so I'm in.|10"), ("so", 2, "It works", "so it works|11"), ("check", 5, "USB power", "check USB power|15"), ("open", 4, "KiCad", "open KiCad|10"),
            ("abc", 1, "  ", "abc|1"), ("abc", 99, "Go", "abc go|6"), ("see (", 5, "Figure one", "see (figure one|15"), ("wait for it", 5, "Well...", "wait well... for it|12")] { assert_eq!(p(d, at, t), want, "{d:?} {t:?}"); }
        let i = put(true, "open", 4, "Backplane now"); assert_eq!((i.text.as_str(), i.caret), ("open Backplane now", 18));
    }
    #[test] fn firm_and_the_key() {
        assert!(firm(&D, " kicad board")); assert!(firm(&D, "Pebble Index ring")); assert!(!firm(&D, "Open it"));
        assert!(openai_ok("sk-abcdefghijklmnopqrstuvwx")); assert!(!openai_ok("sk-short")); assert!(!openai_ok("sk-abcdefghij klmnopqrstuvwx")); assert!(!openai_ok("pk-abcdefghijklmnopqrstuvwx"));
        assert_eq!(mask("sk-abcdefghijklmnopqrstuvwx"), "sk-…uvwx"); assert_eq!(mask("short"), "set"); assert_eq!(mask(""), "");
    }
}

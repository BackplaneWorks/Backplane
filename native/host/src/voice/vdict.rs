//! The dictionary and what dictation does with it (pure; 0.14's core/vdict). Entries are settings, so they live in the hub's log and every client and engine
//! sees the same ones: `voice.e.<key>` one entry as typed ("KiCad", "KiCad = key cad, kai cad", "jason -> JSON", "!um"), `voice.l.<heard>` a learned fix
//! "<to>\t<confirmed>\t<undone>", which applies by itself once confirmed twice and never undone. An empty value removes either.
//!
//! A final segment goes through `VDict::fix`: drop words, replacements and how terms sound (exact), terms written another way ("kicad", "stm 32"), then near
//! misses that sound alike ("Kai CAD"), applied by themselves only where the engine was unsure, else offered as a guess. Live words settle in `Settle`: words
//! two hypotheses in a row agree on, far enough from the end, are committed and never change; the rest is the tail, which alone may be revised.
use super::words::{self as v, chars, len, lower, Unsure};
use serde_json::{json, Value};
use std::collections::HashMap;

pub const E_PRE: &str = "voice.e.";
pub const L_PRE: &str = "voice.l.";
/// Below this the engine was unsure enough for a sound-alike to apply by itself.
const UNSURE_P: u32 = 85;
/// Words the engine was this unsure of are asked about.
const LOW_P: u32 = 50;
const TRUST: u32 = 2;
/// Far enough from the end of what was heard to be settled (cs).
const MARGIN: u32 = 40;

/// A heard word: as written, its probability (percent; 100 unknown), start and end (centiseconds from the segment's start; 0 0 unknown).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VW { pub w: String, pub p: u32, pub s: u32, pub e: u32 }
/// A fix made, for the chips under the composer (and their undo). `out` empty: the word was dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VFx { pub heard: String, pub out: String }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VFix { pub words: Vec<VW>, pub fixes: Vec<VFx>, pub ask: Vec<Unsure> }
#[derive(Clone)]
struct VR { out: String, auto: bool }
struct VF { key: String, flat: String, out: String }
/// The prepared dictionary.
#[derive(Default)]
pub struct VDict { exact: HashMap<String, VR>, flat: HashMap<String, String>, fuzzy: Vec<VF>, terms: Vec<String>, lead: Vec<String> }

// ---- characters ----
fn punct(c: char) -> bool { matches!(c, '.' | ',' | '!' | '?' | ';' | ':' | '"' | '\'' | '(' | ')' | '[' | ']' | '“' | '”' | '‘' | '’' | '…') }
/// A word's leading punctuation, its core, its trailing punctuation.
fn split(w: &str) -> (String, String, String) {
    let cs = chars(w); let a = cs.iter().take_while(|c| punct(**c)).count(); let b = cs[a..].iter().rev().take_while(|c| punct(**c)).count();
    (cs[..a].iter().collect(), cs[a..cs.len() - b].iter().collect(), cs[cs.len() - b..].iter().collect())
}
fn core(w: &str) -> String { split(w).1 }
/// A word as compared: no punctuation around it, lower case.
pub fn norm(w: &str) -> String { lower(&core(w)) }
/// Runs of spaces made one, trimmed, lower case (a heard phrase).
pub fn phrase(s: &str) -> String { s.trim().split(' ').map(norm).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ") }

// ---- sound keys ----
/// A letter's sound class: 0 vowel (kept only first, as 'a'), 1 silent (h w), else the letter it stands for (b/p, d/t, c/g/k/q, f/v, s/z, x ks).
fn cls(c: char) -> u32 { match c { 'a' | 'e' | 'i' | 'o' | 'u' | 'y' => 0, 'h' | 'w' => 1, 'b' => 'p' as u32, 'd' => 't' as u32, 'c' | 'g' | 'q' => 'k' as u32, 'v' => 'f' as u32, 'z' => 's' as u32, c => c as u32 } }
/// How a word sounds, roughly ("kicad", "kaicad" and "keycad" all "kt").
pub fn sound(flat: &str) -> String {
    let (mut out, mut last, mut first) = (String::new(), 2u32, true);
    let mut put = |k: u32, out: &mut String| { if k != last { out.extend(char::from_u32(k)); last = k; } };
    for c in flat.chars() {
        let k = cls(c);
        if std::mem::take(&mut first) { put(if k == 0 { 'a' as u32 } else { k }, &mut out) }
        else if k <= 1 { continue }
        else if k == 'x' as u32 { put('k' as u32, &mut out); put('s' as u32, &mut out) }
        else { put(k, &mut out) }
    }
    out
}

// ---- entries ----
fn left<'a>(s: &'a str, sep: &str) -> &'a str { s.split_once(sep).map_or(s, |(a, _)| a) }
fn right<'a>(s: &'a str, sep: &str) -> &'a str { s.split_once(sep).map_or("", |(_, b)| b) }
/// An entry's setting key: a term by its spelling, a replacement by what is heard, a drop word by "!" and the word (all lower case).
pub fn ekey(line: &str) -> String {
    let l = line.trim();
    if let Some(r) = l.strip_prefix('!') { format!("!{}", phrase(r)) } else if l.contains("->") { format!("{}>", phrase(left(l, "->"))) } else { lower(left(l, "=").trim()) }
}
pub fn ekey_setting(line: &str) -> String { format!("{E_PRE}{}", ekey(line)) }
/// Blank lines and # comments are nothing.
pub fn line_ok(line: &str) -> bool { let t = line.trim(); !t.is_empty() && !t.starts_with('#') }

/// The words of a phrase but its last: committing one of them waits for the next, so the phrase is matched whole.
fn leads(ws: &[&str], acc: &mut Vec<String>) { for h in ws.iter().take(ws.len().saturating_sub(1)) { if !v::has(acc, h) { acc.insert(0, h.to_string()) } } }
/// A part number may arrive as two words ("NRF 52840"): its letter prefix stays open until the digits arrive.
fn part_lead(s: &str) -> String { match s.find(|c: char| c.is_ascii_digit()) { Some(i) => s[..i].to_string(), None => String::new() } }

impl VDict {
    /// The dictionary from entry lines and learned (heard, value) pairs; learned go first, so an entry the person wrote wins over one it learned.
    pub fn of<L: AsRef<str>, H: AsRef<str>, W: AsRef<str>>(lines: &[L], learned: &[(H, W)]) -> VDict {
        let mut d = VDict::default();
        for (h, val) in learned { let x = Learned::parse(val.as_ref()); let t = x.trusted(); d.put_exact(h.as_ref(), &x.out, t); }
        d.add_lines(lines); d
    }
    pub fn add_lines<L: AsRef<str>>(&mut self, lines: &[L]) { for l in lines { self.add(l.as_ref()) } }
    fn put_exact(&mut self, heard: &str, out: &str, auto: bool) {
        if !heard.is_empty() { self.exact.insert(heard.to_string(), VR { out: out.to_string(), auto }); }
        leads(&heard.split(' ').collect::<Vec<_>>(), &mut self.lead);
    }
    fn put_term(&mut self, t: &str) {
        let fl = v::unspace(&lower(t));
        self.flat.insert(fl.clone(), t.to_string()); self.fuzzy.insert(0, VF { key: sound(&fl), flat: fl.clone(), out: t.to_string() }); self.terms.push(t.to_string());
        let p = part_lead(&fl); if !p.is_empty() { self.lead.insert(0, p) }
        leads(&phrase(t).split(' ').collect::<Vec<_>>(), &mut self.lead);
    }
    /// One entry line into the dictionary.
    pub fn add(&mut self, line: &str) {
        if !line_ok(line) { return }
        let l = line.trim();
        if let Some(r) = l.strip_prefix('!') { self.put_exact(&phrase(r), "", true) }
        else if l.contains("->") { self.put_exact(&phrase(left(l, "->")), right(l, "->").trim(), true) }
        else { let t = left(l, "=").trim().to_string(); self.put_term(&t); for h in right(l, "=").split(',') { self.put_exact(&phrase(h), &t, true) } }
    }
    pub fn terms(&self) -> &[String] { &self.terms }
    /// The words that wait for a phrase (Settle).
    pub fn waits(&self) -> &[String] { &self.lead }

    fn fz(&self, k: &str, fl: &str) -> Option<&str> {
        let ok = |a: &str, b: &str| v::near(a, b, if len(b) <= 5 { 1 } else { len(b) / 3 });
        self.fuzzy.iter().find(|f| f.key == k && f.flat != fl && ok(fl, &f.flat)).map(|f| f.out.as_str())
    }
    /// n words at the head, matched: what to write, and whether only as a guess.
    fn attempt(&self, n: usize, ws: &[VW]) -> Option<(usize, String, bool)> {
        if n > ws.len() { return None }
        let taken = &ws[..n];
        let ph = taken.iter().map(|w| norm(&w.w)).collect::<Vec<_>>().join(" ");
        let (surf, p, fl) = (surface(taken), pmin(taken), v::unspace(&ph));
        if let Some(r) = self.exact.get(&ph).filter(|r| r.auto || !r.out.is_empty()) { return Some((n, r.out.clone(), !r.auto)) }
        if let Some(o) = self.flat.get(&fl).filter(|o| !o.is_empty()) { return (*o != surf).then(|| (n, o.clone(), false)) }
        if len(&fl) < 3 { return None }
        // a sound-alike applies by itself where the engine was unsure, or where the word looks technical ("SBI" for SPI, "ASP32" for ESP32); else it is offered
        self.fz(&sound(&fl), &fl).map(|o| (n, o.to_string(), p >= UNSURE_P && !(n == 1 && v::tech(&surf))))
    }
    /// A segment's words corrected.
    pub fn fix(&self, ws: &[VW]) -> VFix {
        let (mut f, mut i) = (VFix::default(), 0);
        while i < ws.len() {
            let rest = &ws[i..];
            let Some((n, o, ask)) = self.attempt(3, rest).or_else(|| self.attempt(2, rest)).or_else(|| self.attempt(1, rest)) else { f.words.push(rest[0].clone()); i += 1; continue };
            let taken = &rest[..n]; i += n;
            if ask { f.words.extend_from_slice(taken); f.ask.push(Unsure { word: surface(taken), guess: o }); continue }
            let o = cap(o, &rest[0].w); let (first, last) = (&taken[0], &taken[n - 1]);
            f.fixes.push(VFx { heard: surface(taken), out: o.clone() });
            if o.is_empty() { if let Some(w) = f.words.last_mut() { w.w.push_str(&split(&last.w).2) } } // a dropped word's closing punctuation goes onto the word before it
            else { f.words.push(VW { w: format!("{}{o}{}", split(&first.w).0, split(&last.w).2), p: pmin(taken), s: first.s, e: last.e }) }
        }
        f
    }
    /// What to ask about after a segment: the guesses the pass offered, then the words the engine was unsure of, then near misses and technical words; each once, at most five.
    pub fn asks<S: AsRef<str>>(&self, ig: &[S], fx: &VFix) -> Vec<Unsure> {
        let low = fx.words.iter().filter(|w| w.p < LOW_P).map(|w| Unsure { word: core(&w.w), guess: String::new() });
        let mut out: Vec<Unsure> = vec![];
        for u in fx.ask.iter().cloned().chain(low).chain(v::unsure(&self.terms, ig, &text(&fx.words))) { if !v::has(ig, &u.word) && !out.iter().any(|o| lower(&o.word) == lower(&u.word)) { out.push(u) } }
        out.truncate(v::MAX_UNSURE); out
    }
}

fn surface(ws: &[VW]) -> String { ws.iter().map(|w| core(&w.w)).collect::<Vec<_>>().join(" ") }
fn pmin(ws: &[VW]) -> u32 { ws.iter().map(|w| w.p).fold(100, u32::min) }
/// What to write, when all lower case, keeps the capital the first heard word had (a sentence's start); anything with a capital is written as it is.
fn cap(o: String, first: &str) -> String {
    if core(first).chars().next().map_or(false, |c| c.is_ascii_uppercase()) && o == lower(&o) { let mut it = o.chars(); it.next().map_or(String::new(), |c| c.to_uppercase().chain(it).collect()) } else { o }
}
/// The words as text.
pub fn text(ws: &[VW]) -> String { ws.iter().map(|w| w.w.as_str()).collect::<Vec<_>>().join(" ") }
/// A text with no timings (the cloud's): its words split at spaces.
pub fn words_text(s: &str) -> Vec<VW> { s.split(' ').map(str::trim).filter(|w| !w.is_empty()).map(|w| VW { w: w.into(), p: 100, s: 0, e: 0 }).collect() }
/// `[[word, start cs, end cs, percent], ...]`
pub fn words_json(j: &Value) -> Vec<VW> {
    let n = |x: &Value| x.as_u64().or_else(|| x.as_f64().map(|f| f as u64)).unwrap_or(0) as u32;
    j.as_array().map_or(vec![], |a| a.iter().filter_map(|x| { let x = x.as_array()?; Some(VW { w: x.first()?.as_str()?.to_string(), s: n(x.get(1)?), e: n(x.get(2)?), p: n(x.get(3)?) }) }).collect())
}
pub fn fixes_json(fs: &[VFx]) -> Value { Value::Array(fs.iter().map(|f| json!({"heard": f.heard, "out": f.out})).collect()) }

/// A learned fix: "<to>\t<confirmed>\t<undone>".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Learned { pub out: String, pub yes: u32, pub no: u32 }
impl Learned {
    pub fn parse(v: &str) -> Learned {
        let num = |s: Option<&str>| s.map_or(0, |s| s.chars().filter_map(|c| c.to_digit(10)).fold(0u32, |a, d| a.saturating_mul(10).saturating_add(d)));
        let mut p = v.split('\t'); Learned { out: p.next().unwrap_or("").to_string(), yes: num(p.next()), no: num(p.next()) }
    }
    pub fn show(&self) -> String { format!("{}\t{}\t{}", self.out, self.yes, self.no) }
    /// Confirmed at least twice, never undone.
    pub fn trusted(&self) -> bool { self.yes >= TRUST && self.no == 0 }
}
/// "heard → out" confirmed or undone once more; a different out starts over.
pub fn learn(old: &str, out: &str, yes: bool) -> String {
    let x = Learned::parse(old);
    let n = if old.is_empty() || x.out != out { Learned { out: out.to_string(), yes: u32::from(yes), no: u32::from(!yes) } } else { Learned { yes: x.yes + u32::from(yes), no: x.no + u32::from(!yes), ..x } };
    n.show()
}
/// The local models: v2 unless v3 is asked for.
pub fn model_id(m: &str) -> &'static str { if m == "parakeet-v3" { "parakeet-v3" } else { "parakeet-v2" } }

// ---- settle: live words that never change ----
/// The segment's item, where its committed words end (cs; timed engines), how many are committed (untimed), the last committed word, the last hypothesis.
#[derive(Clone, Debug, Default)]
pub struct Settle { pub item: String, cut: u32, n: usize, last: String, prev: Vec<VW> }
/// What a hypothesis gives: words to commit now, the tail.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Step { pub commit: Vec<VW>, pub tail: Vec<VW> }
fn mid(x: &VW) -> u32 { (x.s + x.e) / 2 }
impl Settle {
    fn tail(&self, timed: bool, ws: &[VW]) -> Vec<VW> {
        if !timed { return ws.iter().skip(self.n).cloned().collect() }
        let mut t: Vec<VW> = ws.iter().filter(|w| mid(w) >= self.cut).cloned().collect();
        // a word committed last, heard again a little later, is not new
        if t.first().map_or(false, |h| !self.last.is_empty() && norm(&h.w) == self.last && h.s < self.cut + 10) { t.remove(0); }
        t
    }
    fn begin(&mut self, item: &str) { if self.item != item { *self = Settle { item: item.to_string(), ..Settle::default() } } }
    /// A new hypothesis of the open segment (end: how much audio it covers, cs; lead: `VDict::waits`).
    pub fn hyp<S: AsRef<str>>(&mut self, timed: bool, item: &str, ws: Vec<VW>, end: u32, lead: &[S]) -> Step {
        self.begin(item);
        let (tail, prev) = (self.tail(timed, &ws), self.tail(timed, &self.prev));
        let agree = tail.iter().zip(&prev).take_while(|(a, b)| a.w == b.w).count();
        let room = if timed { tail.len().saturating_sub(1).min(tail.iter().take_while(|w| w.e <= end.saturating_sub(MARGIN)).count()) } else { tail.len().saturating_sub(2) };
        // unsure words, and words that open a phrase, at the end of what would be committed wait for the next ones, so "Kai CAD" or "differential pair" is corrected as one
        let mut k = agree.min(room); while k > 0 && (tail[k - 1].p < UNSURE_P || v::has(lead, &norm(&tail[k - 1].w))) { k -= 1 }
        let (c, rest) = (tail[..k].to_vec(), tail[k..].to_vec());
        if let Some(l) = c.last() { if timed { self.cut = l.e } self.n += k; self.last = norm(&l.w); }
        self.prev = ws;
        Step { commit: c, tail: rest }
    }
    /// The segment's last hypothesis: whatever is not committed is, now.
    pub fn done(&mut self, timed: bool, item: &str, ws: &[VW]) -> Step { self.begin(item); let commit = self.tail(timed, ws); *self = Settle::default(); Step { commit, tail: vec![] } }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dict() -> VDict { VDict::of(&["KiCad = key cad", "STM32", "nRF52840", "SPI", "ESP32", "jason -> JSON", "!um", "# a comment", ""], &[("backplain", "Backplane\t2\t0"), ("bend", "Bend\t1\t0"), ("kick ad", "KiCad\t3\t1"), ("differential pair", "diff pair\t2\t0")]) }
    fn w(s: &str, p: u32) -> VW { VW { w: s.into(), p, s: 0, e: 0 } }
    fn t(s: &str, a: u32, b: u32, p: u32) -> VW { VW { w: s.into(), p, s: a, e: b } }
    fn sure(s: &str) -> Vec<VW> { words_text(s) }
    fn fix(ws: &[VW]) -> String { text(&dict().fix(ws).words) }
    fn fixed(ws: &[VW]) -> String { dict().fix(ws).fixes.iter().map(|f| format!("{}>{};", f.heard, f.out)).collect() }
    fn us(u: &[Unsure]) -> String { u.iter().map(|x| format!("{}={};", x.word, x.guess)).collect() }
    fn asked(ws: &[VW]) -> String { us(&dict().fix(ws).ask) }
    fn cm(x: Step) -> String { format!("{}|{}", text(&x.commit), text(&x.tail)) }
    const NO: [&str; 0] = [];
    fn h1() -> Vec<VW> { vec![t("Open", 0, 24, 95), t("the", 24, 32, 100), t("board.", 32, 56, 90)] }
    fn h2() -> Vec<VW> { vec![t("Open", 0, 24, 95), t("the", 32, 40, 100), t("board", 40, 64, 100), t("in", 64, 80, 85), t("Kai", 80, 104, 72), t("Kat.", 112, 136, 29)] }
    fn h3() -> Vec<VW> { vec![t("Open", 0, 40, 98), t("the", 40, 48, 100), t("board", 48, 72, 100), t("in", 72, 88, 99), t("Kai", 88, 112, 57), t("Cad", 120, 136, 53), t("and", 144, 168, 100)] }
    fn h4() -> Vec<VW> { vec![t("Open", 0, 40, 99), t("the", 40, 48, 100), t("board", 48, 72, 100), t("in", 72, 88, 100), t("Kai", 88, 112, 82), t("CAD", 120, 152, 59), t("and", 152, 160, 100), t("run", 160, 168, 100), t("the", 168, 176, 100)] }
    fn h5() -> Vec<VW> { vec![t("Open", 0, 40, 99), t("the", 40, 48, 100), t("board", 48, 72, 100), t("in", 72, 88, 100), t("Kai", 88, 128, 74), t("CAD", 128, 160, 62), t("and", 160, 168, 100), t("run", 168, 184, 100), t("the", 184, 192, 100), t("design", 192, 230, 100)] }
    fn hp() -> Vec<VW> { vec![t("the", 0, 20, 100), t("differential", 20, 60, 100), t("pair", 60, 80, 100)] }
    fn s(n: usize) -> Settle { let mut x = Settle::default(); for (h, e) in [(h1(), 60), (h2(), 140), (h3(), 170), (h4(), 200)].into_iter().take(n) { x.hyp(true, "a", h, e, &NO); } x }

    #[test] fn sound_keys_and_entry_keys() {
        assert_eq!(["kicad", "kaicad", "keycad", "spi", "sbi", "esp32", "asp32"].map(sound).join(" "), "kt kt kt sp sp asp32 asp32");
        assert_eq!(format!("{} {} {}", ekey("KiCad = key cad"), ekey(" Jason  -> JSON"), ekey("!Um")), "kicad jason> !um");
    }
    #[test] fn the_correction_pass() {
        assert_eq!(fix(&sure("Open the board and run the check.")), "Open the board and run the check.");
        assert_eq!(fix(&sure("Open the board in key cad now.")), "Open the board in KiCad now.");
        assert_eq!(fix(&sure("The NRF52840 and the stm32.")), "The nRF52840 and the STM32.");
        assert_eq!(format!("{}/{}/{}", fix(&sure("NRF52840")), fix(&sure("the NRF52840")), fix(&sure("NRF52840 and"))), "nRF52840/the nRF52840/nRF52840 and");
        assert_eq!(fix(&sure("an stm 32 board")), "an STM32 board"); assert_eq!(fix(&sure("the NRF 52840 over SPI")), "the nRF52840 over SPI");
        assert_eq!(fix(&sure("send the jason, please")), "send the JSON, please"); assert_eq!(fix(&sure("Jason is fine")), "JSON is fine");
        assert_eq!(fix(&sure("so um, the board")), "so, the board"); assert_eq!(fix(&sure("Um the board")), "the board");
        assert_eq!(fix(&sure("open backplain")), "open Backplane");
        assert_eq!(format!("{} / {}", fix(&sure("the bend thing")), asked(&sure("the bend thing"))), "the bend thing / bend=Bend;");
        assert_eq!(asked(&sure("a kick ad file")), "kick ad=KiCad;");
        assert_eq!(fix(&[w("over", 100), w("SBI", 70), w("at", 100)]), "over SPI at");
        let spy = [w("the", 100), w("spy", 99)]; assert_eq!(format!("{} / {}", fix(&spy), asked(&spy)), "the spy / spy=SPI;");
        assert_eq!(fix(&[w("in", 100), w("Kai", 72), w("CAD", 59), w("and", 100)]), "in KiCad and");
        assert_eq!(fix(&[w("the", 100), w("ASP32.", 80)]), "the ESP32."); assert_eq!(fix(&[w("the", 100), w("kid", 40)]), "the kid");
        assert_eq!(fix(&[w("over", 100), w("ASP32", 99)]), "over ESP32");
        assert_eq!(fixed(&sure("um the key cad and jason")), "um>;key cad>KiCad;jason>JSON;");
        let d = dict(); assert_eq!(us(&d.asks(&NO, &d.fix(&[w("the", 100), w("role", 40), w("R12", 100)]))), "role=;R12=;");
    }
    #[test] fn learning() {
        assert_eq!(learn("", "KiCad", true), "KiCad\t1\t0"); assert_eq!(learn("KiCad\t1\t0", "KiCad", true), "KiCad\t2\t0");
        assert_eq!(learn("KiCad\t2\t0", "KiCad", false), "KiCad\t2\t1"); assert_eq!(learn("KiCad\t2\t1", "Kicad", true), "Kicad\t1\t0");
        assert!(Learned::parse("x\t2\t0").trusted()); assert!(!Learned::parse("x\t1\t0").trusted()); assert!(!Learned::parse("x\t3\t1").trusted());
        let d = VDict::of(&["kick ad -> KiCad!"], &[("kick ad", "KiCad\t1\t0")]); assert_eq!(text(&d.fix(&sure("a kick ad file")).words), "a KiCad! file", "what the person wrote wins over what it learned");
    }
    #[test] fn settling_timed_words() {
        assert_eq!(cm(Settle::default().hyp(true, "a", h1(), 60, &NO)), "|Open the board.");
        assert_eq!(cm(s(1).hyp(true, "a", h2(), 140, &NO)), "Open the|board in Kai Kat.");
        assert_eq!(cm(s(2).hyp(true, "a", h3(), 170, &NO)), "board in|Kai Cad and");
        assert_eq!(cm(s(3).hyp(true, "a", h4(), 200, &NO)), "|Kai CAD and run the");
        assert_eq!(cm(s(4).hyp(true, "a", h5(), 240, &NO)), "Kai CAD and run the|design");
        assert_eq!(cm(s(4).done(true, "a", &h5())), "Kai CAD and run the design|");
        assert_eq!(cm(s(4).done(true, "b", &h1())), "Open the board.|");
    }
    #[test] fn settling_untimed_words_and_phrases() {
        let mut c = Settle::default(); c.hyp(false, "c", sure("one two three"), 0, &NO);
        assert_eq!(cm(c.clone().hyp(false, "c", sure("one two three four"), 0, &NO)), "one two|three four");
        c.hyp(false, "c", sure("one two three four"), 0, &NO); assert_eq!(cm(c.done(false, "c", &sure("one two three four five."))), "three four five.|");
        let d = dict(); let mut p = Settle::default(); p.hyp(true, "p", hp(), 200, d.waits()); assert_eq!(cm(p.hyp(true, "p", hp(), 200, d.waits())), "the|differential pair");
        let mut n = Settle::default(); n.hyp(false, "p", sure("the NRF 52840 over"), 0, d.waits()); assert_eq!(cm(n.hyp(false, "p", sure("the NRF 52840 over"), 0, d.waits())), "the|NRF 52840 over");
    }
    #[test] fn words_from_the_helper() { assert_eq!(words_json(&json!([["Open", 0, 24, 95], "junk", ["the", 24, 32]])), vec![t("Open", 0, 24, 95)]); }
}

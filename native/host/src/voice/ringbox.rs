//! Ring notes kept (Nov 2026): every recording a Pebble Index ring's webhook brings (INDEX_WEBHOOK_API.md, version 1), under `<home>/ring/<id>/`:
//! `audio.m4a` as it came and `note.json`, what is known of it (when it was said, the gesture, the ring's own words, each hearing with its model,
//! segments and confidence, and where it went). A note's id comes from its hook and the app's stable `X-Index-Delivery`, so a delivery the app
//! retries (every minute, up to ten times, after the phone was offline or the hub was down) is the same note: answered 2xx and not heard again.
//! Everything in a form is untrusted: names, ids and sizes are cut down here before anything is kept, and nothing of it ever names a path.
use crate::net::{botnet, owner};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// One hearing of a note: the engine (`parakeet-v2`, `gpt-live-transcribe`, `ring` for the app's own words), the words after the dictionary,
/// what to ask about, the least sure segment (percent, when the engine says) and its segments: start and end in centiseconds, percent, words.
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct Heard {
    pub model: String,
    pub text: String,
    #[serde(default)] pub unsure: String,
    #[serde(default)] pub conf: Option<i64>,
    #[serde(default)] pub segs: Vec<(i64, i64, i64, String)>,
    pub at: u64,
}

/// What a note went through: `new` (kept, not heard yet), `heard`, `sent` (the bot has it), `failed` (`error` says why), `test` (the app's test button).
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct Note {
    pub id: String,
    pub delivery: String,
    pub hook: String,
    #[serde(default)] pub trigger: String,
    /// When it was said (the form's `recordedAt`, ms), else when it came.
    pub recorded_ms: u64,
    /// When it came (s).
    pub at: u64,
    #[serde(default)] pub signed: bool,
    /// The recording's bytes (0: the form had none).
    #[serde(default)] pub audio: u64,
    #[serde(default)] pub ring_text: String,
    #[serde(default)] pub heard: Vec<Heard>,
    pub state: String,
    #[serde(default)] pub bot: String,
    #[serde(default)] pub thread: String,
    #[serde(default)] pub msg: String,
    #[serde(default)] pub error: String,
    #[serde(default)] pub tries: u32,
}
impl Note {
    /// The words it is read by: the newest hearing with words, else the ring's own.
    pub fn words(&self) -> &str { self.heard.iter().rev().find(|h| !h.text.is_empty()).map_or(self.ring_text.as_str(), |h| h.text.as_str()) }
    pub fn best(&self) -> Option<&Heard> { self.heard.iter().rev().find(|h| !h.text.is_empty()) }
}

pub fn root(home: &Path) -> PathBuf { home.join("ring") }
fn dir(home: &Path, id: &str) -> Option<PathBuf> { id_ok(id).then(|| root(home).join(id)) }
pub fn audio_path(home: &Path, id: &str) -> Option<PathBuf> { dir(home, id).map(|d| d.join("audio.m4a")) }
/// An id this store made: `n` and 16 hex digits.
pub fn id_ok(id: &str) -> bool { id.len() == 17 && id.starts_with('n') && id[1..].bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) }

// ---- the form, as bytes ----

/// One part of a multipart body.
#[derive(Debug, Default, PartialEq)]
pub struct Part { pub name: String, pub filename: String, pub ctype: String, pub data: Vec<u8> }
fn find(h: &[u8], n: &[u8], from: usize) -> Option<usize> { if n.is_empty() || h.len() < n.len() { return None } (from..=h.len() - n.len()).find(|&i| &h[i..i + n.len()] == n) }
/// The boundary of a `multipart/form-data` content type ("" when it has none).
pub fn boundary(ct: &str) -> String {
    ct.split(';').map(str::trim).find_map(|p| p.strip_prefix("boundary=")).map(|b| b.trim_matches('"').to_string()).filter(|b| !b.is_empty() && b.len() <= 70).unwrap_or_default()
}
fn header_param(h: &str, key: &str) -> String {
    let pat = format!("{key}=\"");
    h.split(';').map(str::trim).find_map(|p| p.strip_prefix(&pat[..]).map(|v| v.trim_end_matches('"').to_string())).unwrap_or_default()
}
/// The parts of a body, its bytes kept as they are (a recording is binary): at most 16 parts; a part with no end is cut at the body's end.
pub fn parts(raw: &[u8], ct: &str) -> Vec<Part> {
    let b = boundary(ct); if b.is_empty() { return vec![] }
    let delim = format!("--{b}").into_bytes(); let next = format!("\r\n--{b}").into_bytes();
    let Some(mut at) = find(raw, &delim, 0) else { return vec![] };
    let mut out = vec![];
    while out.len() < 16 {
        at += delim.len();
        if raw[at..].starts_with(b"--") { break }
        let Some(hend) = find(raw, b"\r\n\r\n", at) else { break };
        let head = String::from_utf8_lossy(&raw[at..hend]).into_owned();
        let start = hend + 4; let end = find(raw, &next, start).unwrap_or(raw.len());
        let (mut name, mut filename, mut ctype) = (String::new(), String::new(), String::new());
        for l in head.split("\r\n") {
            let Some((k, v)) = l.split_once(':') else { continue };
            match k.trim().to_ascii_lowercase().as_str() { "content-disposition" => { name = header_param(v, "name"); filename = header_param(v, "filename"); } "content-type" => ctype = v.trim().to_string(), _ => {} }
        }
        out.push(Part { name, filename, ctype, data: raw[start..end].to_vec() });
        if end >= raw.len() { break }
        at = end + 2;
    }
    out
}
/// A part's text, as text (empty when there is none): the first of that name.
pub fn text_of(ps: &[Part], name: &str) -> String { ps.iter().find(|p| p.name == name && p.filename.is_empty()).map(|p| String::from_utf8_lossy(&p.data).trim().to_string()).unwrap_or_default() }
pub fn audio_of(ps: &[Part]) -> Option<&Part> { ps.iter().find(|p| p.name == "audio" && !p.data.is_empty()) }
/// Is this the Pebble app's form: `client` is `ring`, or the request has the Index protocol's headers.
pub fn is_ring(ps: &[Part], version: &str) -> bool { text_of(ps, "client") == "ring" || !version.is_empty() }

/// What names the recording: the app's `X-Index-Delivery`, else the audio's file name (`<recordingId>.m4a`), else the body's hash; kept to
/// letters, digits and `.:_-`, at most 96.
pub fn delivery_of(header: &str, filename: &str, raw: &[u8]) -> String {
    let clean = |s: &str| s.chars().filter(|c| c.is_ascii_alphanumeric() || ".:_-".contains(*c)).take(96).collect::<String>();
    let h = clean(header.trim()); if !h.is_empty() { return h }
    let f = clean(filename.trim().trim_end_matches(".m4a")); if !f.is_empty() { return format!("file:{f}") }
    format!("sha:{}", &hex::encode(Sha256::digest(raw))[..32])
}
pub fn id_of(hook: &str, delivery: &str) -> String { format!("n{}", &hex::encode(Sha256::digest(format!("{hook}\n{delivery}").as_bytes()))[..16]) }
/// `single-click-hold` and `double-click-hold`, as the app's settings say them.
pub fn gesture(trigger: &str) -> &'static str { match trigger { "single-click-hold" => "Hold & talk", "double-click-hold" => "Double click & hold", "test-event" => "Test", _ => "" } }
pub fn trigger_ok(t: &str) -> String { t.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(32).collect() }
/// The form's `recordedAt` (ms), when it is a time near enough to be one (2020 to a day ahead of `now`), else `now`.
pub fn recorded(v: &str, now_s: u64) -> u64 { v.trim().parse::<u64>().ok().filter(|ms| *ms >= 1_577_836_800_000 && *ms <= (now_s + 86400) * 1000).unwrap_or(now_s * 1000) }

// ---- on disk ----

pub fn load(home: &Path, id: &str) -> Option<Note> { dir(home, id).and_then(|d| std::fs::read_to_string(d.join("note.json")).ok()).and_then(|t| serde_json::from_str(&t).ok()) }
pub fn exists(home: &Path, id: &str) -> bool { dir(home, id).map_or(false, |d| d.join("note.json").exists()) }
pub fn save(home: &Path, n: &Note) -> bool {
    let Some(d) = dir(home, &n.id) else { return false }; botnet::mkdir(&d);
    serde_json::to_string_pretty(n).map_or(false, |t| botnet::write_secret(&d.join("note.json"), &t))
}
/// A new note with its recording; false when it could not be kept (the app then gets a 503 and tries again).
pub fn put(home: &Path, n: &Note, audio: Option<&[u8]>) -> bool {
    let Some(d) = dir(home, &n.id) else { return false }; botnet::mkdir(&d);
    if let Some(a) = audio { if !botnet::write_secret_bytes(&d.join("audio.m4a"), a) { return false } }
    save(home, n)
}
/// Every note, the newest said first.
pub fn list(home: &Path) -> Vec<Note> {
    let mut v: Vec<Note> = std::fs::read_dir(root(home)).into_iter().flatten().flatten().filter_map(|e| e.file_name().to_str().and_then(|id| load(home, id))).collect();
    v.sort_by(|a, b| b.recorded_ms.cmp(&a.recorded_ms).then(b.id.cmp(&a.id))); v
}
/// The notes every word of `q` is in (any case), in the words heard, the ring's, or what it was sent as.
pub fn search<'a>(notes: &'a [Note], q: &str) -> Vec<&'a Note> {
    let terms: Vec<String> = q.split_whitespace().map(str::to_lowercase).collect();
    notes.iter().filter(|n| { let hay = format!("{}\n{}\n{}", n.heard.iter().map(|h| h.text.as_str()).collect::<Vec<_>>().join("\n"), n.ring_text, n.id).to_lowercase(); terms.iter().all(|t| hay.contains(t)) }).collect()
}
/// Notes not yet with their bot that are worth another try: not failed for good, under 5 tries, said in the last 3 days.
pub fn pending(notes: &[Note], now_s: u64) -> Vec<String> {
    notes.iter().filter(|n| (n.state == "new" || n.state == "heard") && n.tries < 5 && n.recorded_ms / 1000 + 3 * 86400 > now_s).map(|n| n.id.clone()).collect()
}

/// The key of the recordings' links (`<home>/secrets/ring.key`, made the first time): a link plays one recording and nothing else.
pub fn link_key(home: &Path) -> String {
    let p = home.join("secrets/ring.key"); let k = std::fs::read_to_string(&p).unwrap_or_default();
    if k.trim().len() >= 32 { return k.trim().to_string() }
    botnet::mkdir(&home.join("secrets")); let k = hex::encode(crate::net::pairing::urandom(32)); if botnet::write_secret(&p, &k) { k } else { String::new() }
}
pub fn cap(key: &str, id: &str) -> String { if key.is_empty() { return String::new() } hex::encode(owner::hmac_sha256(key.as_bytes(), format!("ring-audio\n{id}").as_bytes()))[..32].to_string() }
pub fn cap_ok(key: &str, id: &str, k: &str) -> bool { !key.is_empty() && id_ok(id) && owner::same(&cap(key, id), k) }

// ---- what the bot hears and the clients see ----

fn ago(s: u64) -> String { if s < 120 { format!("{s} s") } else if s < 7200 { format!("{} min", s / 60) } else if s < 172800 { format!("{} h", s / 3600) } else { format!("{} days", s / 86400) } }
/// The message the ring's bot gets: a header saying what this is (and that it is the person's own voice), then their words.
/// `when`: the time it was said, as the person reads it.
pub fn message(n: &Note, when: &str) -> String {
    let h = n.best();
    let mut head = format!("[voice note from your Pebble ring · ring note {} · said {when}", n.id);
    let g = gesture(&n.trigger); if !g.is_empty() { head.push_str(&format!(" · {g}")) }
    if let Some(h) = h { head.push_str(&format!(" · heard by {}", h.model)); if let Some(c) = h.conf { head.push_str(&format!(", {c}% sure")) } }
    let late = n.at.saturating_sub(n.recorded_ms / 1000); if late > 600 { head.push_str(&format!(" · came {} after it was said (the phone was away)", ago(late))) }
    head.push(']');
    let w = n.words();
    if w.is_empty() { return format!("{head}\nIt came without words: the recording could not be heard here (set up Settings, Voice) and the app sent no transcription. The recording is kept; it can be heard again from Settings, Ring.") }
    let mut m = format!("{head}\n{w}");
    if let Some(h) = h { if !h.unsure.is_empty() { m.push_str(&format!("\n\n[{}]", h.unsure)) } }
    if let (Some(h), false) = (h, n.ring_text.is_empty()) { if h.model != "ring" && !n.ring_text.eq_ignore_ascii_case(&h.text) { m.push_str(&format!("\n\n[the Pebble app heard: {}]", n.ring_text.chars().take(2000).collect::<String>())) } }
    m
}
/// A note for the clients' list: its words cut to 600 characters, the link that plays it (`base` + a capability), where it went.
pub fn summary(n: &Note, base: &str, key: &str) -> Value {
    let h = n.best(); let w: String = n.words().chars().take(600).collect();
    let play = if n.audio > 0 && !base.is_empty() { format!("{base}/ring/audio/{}?k={}", n.id, cap(key, &n.id)) } else { String::new() };
    json!({"id": n.id, "said": n.recorded_ms / 1000, "at": n.at, "gesture": gesture(&n.trigger), "words": w, "model": h.map_or("", |h| h.model.as_str()), "conf": h.and_then(|h| h.conf),
        "unsure": h.map_or("", |h| h.unsure.as_str()), "state": n.state, "error": n.error, "thread": n.thread, "msg": n.msg, "play": play, "heard": n.heard.len()})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn form(parts: &[(&str, &str, &[u8])]) -> Vec<u8> {
        let mut b = vec![];
        for (k, f, v) in parts {
            let fname = if f.is_empty() { String::new() } else { format!("; filename=\"{f}\"") };
            let ct = if f.is_empty() { String::new() } else { "\r\nContent-Type: audio/mp4".to_string() };
            b.extend(format!("--xyz\r\nContent-Disposition: form-data; name=\"{k}\"{fname}{ct}\r\n\r\n").as_bytes()); b.extend_from_slice(v); b.extend(b"\r\n");
        }
        b.extend(b"--xyz--\r\n"); b
    }
    const CT: &str = "multipart/form-data; boundary=xyz";
    #[test] fn the_parts_of_a_form_keep_the_recordings_bytes() {
        let audio: &[u8] = b"\x00\x01\r\n--xy\xff\xfeftyp";
        let b = form(&[("transcription", "", b" Buy milk "), ("audio", "rec-42.m4a", audio), ("recordedAt", "", b"1790000000000"), ("client", "", b"ring")]);
        let ps = parts(&b, CT);
        assert_eq!(ps.len(), 4); assert_eq!(text_of(&ps, "transcription"), "Buy milk"); assert_eq!(text_of(&ps, "client"), "ring");
        let a = audio_of(&ps).unwrap(); assert_eq!(a.data, audio); assert_eq!(a.filename, "rec-42.m4a"); assert_eq!(a.ctype, "audio/mp4");
        assert!(is_ring(&ps, "")); assert!(is_ring(&[], "1")); assert!(!is_ring(&parts(&form(&[("x", "", b"y")]), CT), ""));
        assert!(parts(&b, "multipart/form-data").is_empty(), "no boundary, no parts"); assert_eq!(boundary("multipart/form-data; boundary=\"q q\""), "q q");
        let cut = &b[..b.len() - 30]; assert!(parts(cut, CT).len() >= 3, "a body cut short keeps what it has");
    }
    #[test] fn a_recording_is_named_by_its_delivery_and_its_id_by_the_hook() {
        assert_eq!(delivery_of(" abc-123 ", "x.m4a", b""), "abc-123"); assert_eq!(delivery_of("../../etc", "", b""), "....etc");
        assert_eq!(delivery_of("", "rec-42.m4a", b""), "file:rec-42"); assert!(delivery_of("", "", b"body").starts_with("sha:"));
        let a = id_of("h1", "d1"); assert!(id_ok(&a)); assert_eq!(a, id_of("h1", "d1")); assert_ne!(a, id_of("h2", "d1")); assert!(!id_ok("../n0")); assert!(!id_ok("nABCDEF0123456789"));
        assert_eq!(recorded("1790000000000", 1790000100), 1790000000000); assert_eq!(recorded("soon", 1790000100), 1790000100000); assert_eq!(recorded("99999999999999", 1790000100), 1790000100000);
        assert_eq!(trigger_ok("single-click-hold\n<x>"), "single-click-holdx"); assert_eq!(gesture("double-click-hold"), "Double click & hold");
    }
    fn note(id: &str, ms: u64, words: &str) -> Note { Note { id: id.into(), delivery: "d".into(), hook: "h".into(), recorded_ms: ms, at: ms / 1000, state: "sent".into(), heard: vec![Heard { model: "parakeet-v2".into(), text: words.into(), at: 1, ..Default::default() }], ..Default::default() } }
    #[test] fn notes_are_kept_listed_newest_first_and_found_by_every_word() {
        let home = std::env::temp_dir().join(format!("ringbox-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home);
        let a = note(&id_of("h", "a"), 1_790_000_000_000, "Order the STM32N657 samples"); let b = note(&id_of("h", "b"), 1_790_000_500_000, "Call Jeff about the board");
        assert!(put(&home, &a, Some(b"m4a"))); assert!(put(&home, &b, None)); assert!(exists(&home, &a.id));
        assert_eq!(std::fs::read(audio_path(&home, &a.id).unwrap()).unwrap(), b"m4a");
        let l = list(&home); assert_eq!(l.iter().map(|n| n.id.clone()).collect::<Vec<_>>(), [b.id.clone(), a.id.clone()]);
        assert_eq!(search(&l, "stm32n657 ORDER").len(), 1); assert_eq!(search(&l, "the").len(), 2); assert!(search(&l, "jeff stm32").is_empty());
        let mut c = l[0].clone(); c.state = "new".into(); let mut d = l[1].clone(); d.state = "heard".into(); d.tries = 5;
        assert_eq!(pending(&[c.clone(), d], 1_790_000_600), [c.id.clone()]); assert!(pending(&[c], 1_790_000_600 + 4 * 86400).is_empty(), "too old to send now");
        let _ = std::fs::remove_dir_all(&home);
    }
    #[test] fn a_link_plays_one_recording() {
        let k = "k".repeat(64); let c = cap(&k, "n0123456789abcdef");
        assert!(cap_ok(&k, "n0123456789abcdef", &c)); assert!(!cap_ok(&k, "n0123456789abcdee", &c)); assert!(!cap_ok("", "n0123456789abcdef", &c)); assert!(!cap_ok(&k, "n0123456789abcdef", ""));
    }
    #[test] fn the_bot_hears_what_it_is_when_it_was_said_and_how_sure() {
        let mut n = note("n0123456789abcdef", 1_790_000_000_000, "Remind me to call Jeff at five"); n.trigger = "single-click-hold".into(); n.heard[0].conf = Some(91); n.at = 1_790_000_005;
        assert_eq!(message(&n, "Tue 10 Nov 14:03 EST"), "[voice note from your Pebble ring · ring note n0123456789abcdef · said Tue 10 Nov 14:03 EST · Hold & talk · heard by parakeet-v2, 91% sure]\nRemind me to call Jeff at five");
        n.at += 3 * 3600; n.ring_text = "remind me to call chef at five".into(); n.heard[0].unsure = "Possibly misheard: \"Jeff\"".into();
        let m = message(&n, "x"); assert!(m.contains("came 3 h after it was said")); assert!(m.ends_with("[Possibly misheard: \"Jeff\"]\n\n[the Pebble app heard: remind me to call chef at five]"), "{m}");
        let mut e = n.clone(); e.heard.clear(); e.ring_text.clear(); assert!(message(&e, "x").contains("It came without words"));
        let s = summary(&n, "https://desk:8451", "kk"); assert_eq!(s["play"], ""); n.audio = 10; let s = summary(&n, "https://desk:8451", "kk");
        assert_eq!(s["play"], format!("https://desk:8451/ring/audio/n0123456789abcdef?k={}", cap("kk", "n0123456789abcdef"))); assert_eq!(s["conf"], 91);
    }
}

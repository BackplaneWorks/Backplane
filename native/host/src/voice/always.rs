//! Always-on dictation (pure; docs/voice.md "Always on"): the microphone stays open and a message starts by itself at the first real words, goes into
//! a fresh draft of the open thread (or the one named first: "Bub, …", "KiCad thread, …"), and end of turn (eot.rs) sends it; then it listens again.
//! What is heard before a message starts is not written and goes nowhere: fillers ("um", "uh"), coughs, keys and fans never start one.
use super::page::Prefs;
use super::vdict::VW;
use super::words::{alnum, lower};

pub const ON: &str = "voice.always";
pub const WORDS: &str = "voice.always.words";
pub const SPEECH_MS: &str = "voice.always.ms";
pub const SPEAKER: &str = "voice.always.speaker";
pub const PAUSE_CALL: &str = "voice.always.pause.call";
pub const PAUSE_LOCK: &str = "voice.always.pause.lock";
pub const PAUSE_AWAY: &str = "voice.always.pause.away";
pub const TARGET: &str = "voice.always.target";
/// Apps whose use of the microphone does not pause always-on (comma separated, matched in their names).
pub const IGNORE: &str = "voice.always.ignore";
/// A word as sure as this (the engine's percent) counts toward starting a message.
pub const MIN_P: u32 = 60;
const FILLERS: [&str; 14] = ["um", "uh", "uhm", "umm", "er", "erm", "ah", "ahh", "hmm", "hm", "mm", "mhm", "oh", "eh"];

#[derive(Clone, Debug, PartialEq)]
pub struct Cfg { pub ignore: Vec<String>, pub on: bool, pub gate: Gate, pub speaker: bool, pub pause_call: bool, pub pause_lock: bool, pub pause_away: bool, /** "open" (the open thread, else Bub) or "bub" */ pub target: String }
impl Cfg {
    pub fn of(p: &Prefs) -> Cfg {
        let get = |k: &str| p.get(k).map_or("", String::as_str);
        Cfg { ignore: get(IGNORE).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(), on: get(ON) == "on", gate: Gate { min_words: get(WORDS).parse().unwrap_or(1).clamp(1, 5), min_ms: get(SPEECH_MS).parse().unwrap_or(0).min(3000), min_p: MIN_P },
            speaker: get(SPEAKER) != "off", pause_call: get(PAUSE_CALL) != "off", pause_lock: get(PAUSE_LOCK) != "off", pause_away: get(PAUSE_AWAY) == "on",
            target: if get(TARGET) == "bub" { "bub".into() } else { "open".into() } }
    }
}

/// What starts a message: this many real words (not fillers, sure enough), spanning this much speech.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate { pub min_words: usize, pub min_ms: u32, pub min_p: u32 }
fn word(w: &str) -> String { lower(w).chars().filter(|c| alnum(*c) || *c == '\'').collect() }
pub fn filler(w: &str) -> bool { let k = word(w); k.is_empty() || FILLERS.contains(&k.as_str()) }
impl Gate {
    /// The words of an utterance so far start a message.
    pub fn passes(&self, ws: &[VW]) -> bool {
        let real: Vec<&VW> = ws.iter().filter(|w| !filler(&w.w) && w.p >= self.min_p).collect();
        if real.len() < self.min_words { return false }
        // timed words (the local engine): the speech they span, in centiseconds; the cloud's carry no times
        let timed = ws.iter().any(|w| w.e > 0);
        !timed || self.min_ms == 0 || (real.last().map_or(0, |l| l.e).saturating_sub(real.first().map_or(0, |f| f.s)) * 10) >= self.min_ms
    }
}

/// A name said first, before a comma ("Bub, …", "KiCad thread, …"): how many words it takes and the name (lower case, without "thread").
pub fn route(ws: &[VW]) -> Option<(usize, String)> {
    // one or two words, or three when the third is "thread" ("KiCad board thread,")
    let at = ws.iter().take(3).position(|w| w.w.ends_with(','))?;
    if at == 2 && word(&ws[2].w) != "thread" { return None }
    if at + 1 >= ws.len() { return None } // the message itself must follow
    let name: Vec<String> = ws[..=at].iter().map(|w| word(&w.w)).filter(|w| !w.is_empty() && w != "thread" && w != "hey" && w != "ok" && w != "okay").collect();
    (!name.is_empty()).then(|| (at + 1, name.join(" ")))
}
/// The thread (or bot chat) a spoken name means, from (id, title) pairs: a whole-word match of the title's start, else of any of its words; none
/// when two match as well.
pub fn resolve<'a>(name: &str, targets: &'a [(String, String)]) -> Option<&'a str> {
    let n = name.trim(); if n.is_empty() { return None }
    let norm = |t: &str| t.split_whitespace().map(word).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
    let starts: Vec<&(String, String)> = targets.iter().filter(|(_, t)| { let t = norm(t); t == n || t.starts_with(&format!("{n} ")) }).collect();
    if starts.len() == 1 { return Some(&starts[0].0) }
    let has: Vec<&(String, String)> = targets.iter().filter(|(_, t)| format!(" {} ", norm(t)).contains(&format!(" {n} "))).collect();
    (starts.is_empty() && has.len() == 1).then(|| has[0].0.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn w(s: &str, p: u32, a: u32, b: u32) -> VW { VW { w: s.into(), p, s: a, e: b } }
    fn ws(s: &str) -> Vec<VW> { s.split_whitespace().enumerate().map(|(i, x)| w(x, 95, i as u32 * 20, i as u32 * 20 + 18)).collect() }
    #[test] fn fillers_noise_and_unsure_words_start_nothing() {
        let g = Gate { min_words: 1, min_ms: 0, min_p: MIN_P };
        assert!(!g.passes(&ws("um")) && !g.passes(&ws("Uh, hmm.")) && !g.passes(&[]) && !g.passes(&ws("...")));
        assert!(!g.passes(&[w("the", 30, 0, 10)]), "a word the engine was unsure of (a cough, a fan) is not one");
        assert!(g.passes(&ws("Okay")) && g.passes(&ws("um, add")));
    }
    #[test] fn two_words_or_400_ms_when_asked() {
        let g = Gate { min_words: 2, min_ms: 0, min_p: MIN_P }; assert!(!g.passes(&ws("Hey")) && g.passes(&ws("Hey there")));
        let g = Gate { min_words: 1, min_ms: 400, min_p: MIN_P }; assert!(!g.passes(&[w("Hi", 99, 0, 25)])); assert!(g.passes(&[w("Hello", 99, 0, 20), w("there", 99, 22, 45)]));
        assert!(g.passes(&[w("Hello", 100, 0, 0)]), "the cloud's words have no times: the words alone");
    }
    #[test] fn a_name_first_routes_the_message() {
        assert_eq!(route(&ws("Bub, what is on today?")), Some((1, "bub".into())));
        assert_eq!(route(&ws("KiCad thread, move R12 left.")), Some((2, "kicad".into())));
        assert_eq!(route(&ws("Okay so, this is long")), Some((2, "so".into())), "a name is whatever comes before the comma; resolve decides if it is one");
        assert_eq!(route(&ws("Add a button, then route it")), None, "a comma after the fourth word is not a name");
        assert_eq!(route(&ws("Bub,")), None);
        let t = vec![("t1".to_string(), "KiCad board review".to_string()), ("t2".to_string(), "Firmware".to_string()), ("bub".to_string(), "Bub".to_string()), ("t3".to_string(), "Fix the KiCad export".to_string())];
        assert_eq!(resolve("kicad", &t), Some("t1"), "the title's start wins"); assert_eq!(resolve("bub", &t), Some("bub")); assert_eq!(resolve("firmware", &t), Some("t2"));
        assert_eq!(resolve("export", &t), Some("t3")); assert_eq!(resolve("so", &t), None); assert_eq!(resolve("", &t), None);
    }
    #[test] fn settings() {
        let c = Cfg::of(&Prefs::new()); assert!(!c.on && c.speaker && c.pause_call && c.pause_lock && !c.pause_away); assert_eq!((c.gate.min_words, c.gate.min_ms, c.target.as_str()), (1, 0, "open"));
        let c = Cfg::of(&[(ON, "on"), (WORDS, "2"), (SPEECH_MS, "400"), (TARGET, "bub")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()); assert!(c.on); assert_eq!((c.gate.min_words, c.gate.min_ms, c.target.as_str()), (2, 400, "bub"));
    }
}

// ---- auto-pause: what pauses always-on, read from the system (Linux; elsewhere nothing pauses it but the person)

/// Other apps capturing a microphone now: a call, a meeting, a voice chat, a recording. From `pactl -f json list source-outputs` (`properties`)
/// or `pw-dump` (capture streams: `info.props` with media.class Stream/Input/Audio). Ours (pw-record, parecord, arecord, backplane) and the
/// person's `ignore` list are not counted; monitors of outputs are not microphones. (Discord keeps its stream while in a voice channel, and it
/// cannot be told from a call by its state: both run uncorked; 2026-11-11.)
pub fn capturing(json: &str, ignore: &[String]) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    let mut out: Vec<String> = vec![];
    for o in v.as_array().into_iter().flatten() {
        let p = if o["info"]["props"].is_object() { if !o["info"]["props"]["media.class"].as_str().map_or(false, |c| c.starts_with("Stream/Input/Audio")) { continue } &o["info"]["props"] } else { &o["properties"] };
        let s = |k: &str| p[k].as_str().unwrap_or("").to_string();
        let (app, bin) = (s("application.name"), s("application.process.binary"));
        let ours = ["pw-record", "parecord", "arecord", "backplane-voice", "backplane"].iter().any(|x| bin == *x || app.eq_ignore_ascii_case(x)) || app.to_lowercase().contains("backplane");
        let monitor = s("target.object").ends_with(".monitor") || o["source"].as_str().map_or(false, |x| x.ends_with(".monitor"));
        let ignored = ignore.iter().any(|i| !i.trim().is_empty() && (app.to_lowercase().contains(&i.trim().to_lowercase()) || bin.to_lowercase().contains(&i.trim().to_lowercase())));
        if ours || monitor || ignored { continue }
        let name = if !app.is_empty() { app } else if !bin.is_empty() { bin } else { "another app".into() };
        if !out.contains(&name) { out.push(name) }
    }
    out
}
/// A graphical session of this user is locked (`loginctl show-session <id> -p LockedHint -p Type` for each of `loginctl list-sessions`).
pub fn locked(show_session_outputs: &[String]) -> bool {
    show_session_outputs.iter().any(|o| { let g = |k: &str| o.lines().find_map(|l| l.strip_prefix(k)).unwrap_or("").trim().to_string(); g("LockedHint=") == "yes" && matches!(g("Type=").as_str(), "x11" | "wayland" | "mir" | "") })
}
/// Why always-on is paused now ("" when it listens), from what the system says and the settings.
pub fn pause_reason(c: &Cfg, apps: &[String], locked: bool, focused: bool) -> String {
    if c.pause_lock && locked { return "the screen is locked".into() }
    if c.pause_call && !apps.is_empty() { return format!("{} is using the microphone", apps.join(", ")) }
    if c.pause_away && !focused { return "Backplane is in the background".into() }
    String::new()
}

#[cfg(test)]
mod pause_tests {
    use super::*;
    #[test] fn another_app_on_the_microphone_pauses_and_ours_does_not() {
        let j = r#"[{"index":1,"source":"alsa_input.usb-mic","properties":{"application.name":"ZOOM VoiceEngine","application.process.binary":"zoom"}},
            {"index":2,"source":"alsa_input.usb-mic","properties":{"application.name":"pw-record","application.process.binary":"pw-record"}},
            {"index":3,"source":"alsa_output.hdmi.monitor","properties":{"application.name":"OBS","application.process.binary":"obs"}},
            {"index":4,"properties":{"application.name":"Google Chrome","application.process.binary":"chrome"}}]"#;
        assert_eq!(capturing(j, &[]), ["ZOOM VoiceEngine", "Google Chrome"]); assert!(capturing("[]", &[]).is_empty()); assert!(capturing("nonsense", &[]).is_empty());
        assert_eq!(capturing(j, &["chrome".into()]), ["ZOOM VoiceEngine"], "the person's ignore list");
        // pw-dump, as this desktop gave it (2026-11-11): Discord in a voice channel, a game's voice chat, and our own recorder
        let pw = r#"[{"type":"PipeWire:Interface:Node","info":{"state":"running","props":{"media.class":"Stream/Input/Audio","application.name":"WEBRTC VoiceEngine","application.process.binary":"Discord"}}},
            {"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Stream/Input/Audio","application.name":"pw-record"}}},
            {"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Audio/Source","node.name":"alsa_input.usb"}}}]"#;
        assert_eq!(capturing(pw, &[]), ["WEBRTC VoiceEngine"]);
        let c = Cfg::of(&Prefs::new());
        assert_eq!(pause_reason(&c, &capturing(j, &[]), false, true), "ZOOM VoiceEngine, Google Chrome is using the microphone");
        assert_eq!(pause_reason(&c, &[], true, true), "the screen is locked"); assert_eq!(pause_reason(&c, &[], false, false), "", "in the background only when asked");
        let away = Cfg { pause_away: true, ..c.clone() }; assert_eq!(pause_reason(&away, &[], false, false), "Backplane is in the background");
        let off = Cfg { pause_call: false, pause_lock: false, ..c }; assert_eq!(pause_reason(&off, &["zoom".into()], true, true), "");
    }
    #[test] fn a_locked_graphical_session() {
        assert!(locked(&["Type=wayland\nLockedHint=yes\n".into()])); assert!(!locked(&["Type=wayland\nLockedHint=no\n".into(), "Type=tty\nLockedHint=yes\n".into()]));
    }
}

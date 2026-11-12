//! End of turn (docs/voice.md "End of turn"): when the person has finished a message, it sends itself. At each pause the helper's end-of-turn model
//! (Smart Turn, native/voice/src/turn.rs) gives a probability from the audio; the hub asks Jev (TypeSafe's System One model) with the draft so far, the
//! pause, that probability and the thread's last reply; at or over the threshold a countdown runs, new speech cancels it, then the message goes with
//! the undo window of every voice send. Jev slow (over 400 ms), failing or off: the audio model decides alone; no model at all: a full stop and
//! two seconds of silence. A low score keeps listening however long the pause. Measured 2026-11-11 on 200 labelled clips (smart-turn-data-v3.1-test,
//! English): Jev on the words and the pause alone 83 % right (18 % false sends), the audio model 96 %, Jev given both 97.5 % (3 % false, 2 % missed).
//!
//! Pure: what to ask, how to read the answer, the decision, the threshold and what the undo log teaches it. The asking is session.rs's.
use super::page::Prefs;
use serde_json::{json, Value};

pub const ON: &str = "voice.eot";
pub const JEV: &str = "voice.eot.jev";
pub const MODEL: &str = "voice.eot.model";
pub const THRESHOLD: &str = "voice.eot.threshold";
/// What the undo log taught: a nudge to the threshold, "<nudge>\t<sends>\t<undone>" (never leaves the hub).
pub const LEARNED: &str = "voice.eot.learned";
/// The vault secret with the TypeSafe key (Settings, Voice writes it; the hub reads it only to call Jev).
pub const KEY: &str = "typesafe-api-key";
pub const MODEL_DEFAULT: &str = "jev-latest";
pub const THRESHOLD_DEFAULT: f64 = 0.65;
pub const CHOICES: [(f64, &str); 5] = [(0.5, "Eager"), (0.6, ""), (0.65, "Default"), (0.75, ""), (0.85, "Patient")];
/// Jev's answer later than this is not waited for.
pub const JEV_LIMIT_MS: u64 = 400;
/// The countdown at the default threshold; a more patient threshold waits a little longer.
pub fn countdown_ms(threshold: f64) -> u64 { (1000.0 + (threshold - THRESHOLD_DEFAULT).max(0.0) * 2500.0).round() as u64 }
/// With no model at all: a sentence's end and this much silence.
pub const SILENCE_MS: u64 = 2000;
pub const QUESTION: &str = "The speaker has finished their message and is ready to send it";

#[derive(Clone, Debug, PartialEq)]
pub struct Cfg { pub on: bool, pub jev: bool, pub model: String, pub threshold: f64 }
impl Cfg {
    pub fn of(p: &Prefs) -> Cfg {
        let base = p.get(THRESHOLD).and_then(|s| s.parse::<f64>().ok()).filter(|t| (0.3..=0.95).contains(t)).unwrap_or(THRESHOLD_DEFAULT);
        Cfg { on: p.get(ON).map_or(true, |v| v != "off"), jev: p.get(JEV).map_or(false, |v| v == "on"), model: p.get(MODEL).filter(|m| !m.trim().is_empty()).cloned().unwrap_or_else(|| MODEL_DEFAULT.into()),
            threshold: (base + Learned::parse(p.get(LEARNED).map_or("", String::as_str)).nudge).clamp(0.3, 0.95) }
    }
}

/// The undo log's lesson: an auto-send undone makes the threshold a little more patient, sends kept bring it back slowly. Bounded both ways.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Learned { pub nudge: f64, pub sends: u32, pub undone: u32 }
pub const UNDO_STEP: f64 = 0.02;
pub const KEEP_STEP: f64 = 0.002;
pub const NUDGE_MAX: f64 = 0.15;
pub const NUDGE_MIN: f64 = -0.05;
impl Learned {
    pub fn parse(s: &str) -> Learned { let v: Vec<&str> = s.split('\t').collect(); Learned { nudge: v.first().and_then(|x| x.parse().ok()).unwrap_or(0.0), sends: v.get(1).and_then(|x| x.parse().ok()).unwrap_or(0), undone: v.get(2).and_then(|x| x.parse().ok()).unwrap_or(0) } }
    pub fn text(&self) -> String { format!("{:.3}\t{}\t{}", self.nudge, self.sends, self.undone) }
    /// An auto-send went (kept) or was undone.
    pub fn after(mut self, undone: bool) -> Learned {
        if undone { self.undone += 1; self.nudge = (self.nudge + UNDO_STEP).min(NUDGE_MAX) } else { self.sends += 1; self.nudge = (self.nudge - KEEP_STEP).max(NUDGE_MIN) }
        self
    }
}

/// What Jev is asked: the draft, the pause, the audio model's word, the thread's last reply (for context, shortened).
pub fn state(draft: &str, pause_ms: u64, audio_p: Option<f32>, last_reply: &str) -> String {
    let mut s = format!("Transcript so far: {}. Silence since last word: {pause_ms} ms.", json!(draft.trim()));
    if let Some(p) = audio_p { s.push_str(&format!(" An audio end-of-turn model, which hears intonation, gives {p:.2} that the speaker has finished.")) }
    let r: String = last_reply.trim().chars().take(400).collect();
    if !r.is_empty() { s.push_str(&format!(" The assistant's last message, which the speaker may be answering: {}.", json!(r))) }
    s
}
pub fn request(model: &str, state: &str) -> Value { json!({"model": model, "state": state, "questions": {"done": {"type": "noul", "instructions": QUESTION}}}) }
/// Jev's probability from its answer.
pub fn answer(v: &Value) -> Option<f64> { v["answers"]["done"]["noul"].as_f64().filter(|p| (0.0..=1.0).contains(p)) }

/// Who decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum By { Jev, Audio, Silence }
impl By { pub fn name(self) -> &'static str { match self { By::Jev => "jev", By::Audio => "audio", By::Silence => "silence" } } }
/// The decision at a pause: Some(by) to start the countdown. `jev`: its probability when it answered in time; `audio`: the helper's.
pub fn decide(c: &Cfg, jev: Option<f64>, audio: Option<f32>, draft: &str, silence_ms: u64) -> Option<By> {
    if !c.on || draft.trim().is_empty() { return None }
    if let Some(p) = jev { return (p >= c.threshold).then_some(By::Jev) }
    if let Some(p) = audio { return (p as f64 >= c.threshold.max(0.5)).then_some(By::Audio) }
    let t = draft.trim_end(); (silence_ms >= SILENCE_MS && t.ends_with(['.', '?', '!'])).then_some(By::Silence)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prefs(kv: &[(&str, &str)]) -> Prefs { kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    #[test] fn on_by_default_jev_when_turned_on_threshold_and_model_from_settings() {
        let c = Cfg::of(&prefs(&[])); assert!(c.on && !c.jev); assert_eq!((c.model.as_str(), c.threshold), ("jev-latest", 0.65));
        let c = Cfg::of(&prefs(&[(ON, "off"), (JEV, "on"), (MODEL, "jev-1.14.0"), (THRESHOLD, "0.8")])); assert!(!c.on && c.jev); assert_eq!((c.model.as_str(), c.threshold), ("jev-1.14.0", 0.8));
        assert_eq!(Cfg::of(&prefs(&[(THRESHOLD, "7")])).threshold, 0.65, "nonsense is the default");
    }
    #[test] fn the_decision_jev_first_then_the_audio_model_then_silence() {
        let c = Cfg::of(&prefs(&[]));
        assert_eq!(decide(&c, Some(0.71), Some(0.2), "Add a reset button.", 500), Some(By::Jev), "Jev in time decides");
        assert_eq!(decide(&c, Some(0.15), Some(0.99), "Add a reset button and um", 500), None, "a low score keeps listening");
        assert_eq!(decide(&c, None, Some(0.9), "Add a reset button.", 500), Some(By::Audio), "Jev slow or off: the audio model");
        assert_eq!(decide(&c, None, Some(0.3), "Add a reset button.", 9000), None, "the audio model low: no silence send, however long");
        assert_eq!(decide(&c, None, None, "Add a reset button.", 1000), None); assert_eq!(decide(&c, None, None, "Add a reset button.", 2000), Some(By::Silence));
        assert_eq!(decide(&c, None, None, "Add a reset button and", 9000), None);
        assert_eq!(decide(&Cfg { on: false, ..c.clone() }, Some(1.0), Some(1.0), "Done.", 9000), None); assert_eq!(decide(&c, Some(1.0), None, "  ", 500), None, "nothing to send");
    }
    #[test] fn the_undo_log_nudges_the_threshold_within_bounds() {
        let mut l = Learned::default(); for _ in 0..3 { l = l.after(true) } assert!((l.nudge - 0.06).abs() < 1e-9); assert_eq!(l.undone, 3);
        let c = Cfg::of(&prefs(&[(LEARNED, &l.text())])); assert!((c.threshold - 0.71).abs() < 1e-9, "{}", c.threshold);
        for _ in 0..100 { l = l.after(true) } assert_eq!(l.nudge, NUDGE_MAX);
        for _ in 0..1000 { l = l.after(false) } assert_eq!(l.nudge, NUDGE_MIN); assert_eq!(Learned::parse(&l.text()), l);
    }
    #[test] fn what_jev_is_asked_and_how_its_answer_is_read() {
        let s = state("Please add a reset button and \"um\"", 900, Some(0.12), &"x".repeat(600));
        assert!(s.starts_with(r#"Transcript so far: "Please add a reset button and \"um\"". Silence since last word: 900 ms. An audio end-of-turn model"#), "{s}");
        assert!(s.contains("gives 0.12") && s.matches('x').count() == 400);
        assert_eq!(request("jev-latest", "s")["questions"]["done"]["type"], "noul");
        assert_eq!(answer(&json!({"model":"jev-1.14.0","answers":{"done":{"type":"noul","noul":0.79}}})), Some(0.79)); assert_eq!(answer(&json!({"error":"x"})), None);
        assert_eq!(countdown_ms(0.65), 1000); assert_eq!(countdown_ms(0.85), 1500); assert_eq!(countdown_ms(0.5), 1000);
    }
}

//! Spoken messages (docs/voice.md "Spoken messages"): a message sent by dictation (end of turn, a voice command, hold to talk, always on, the
//! Send button on a dictated draft, a Pebble ring note) reaches the agent wrapped, so every provider and every bot is told the same way that it
//! was spoken and which words to doubt:
//!
//! ```text
//! <voice_message via="eot" stt="parakeet-v2" confidence="0.86" doubt="PA0; Kati" edited="no">
//! Route the reset to PA0 and tell Kati.
//! </voice_message>
//! (Spoken and transcribed: expect transcription errors such as homophones and misheard part numbers, above all in the doubtful words. Ask if something is ambiguous.)
//! ```
//!
//! The log keeps the message as the agent got it; the model's rows (model.rs `row`) show its words alone and carry what it says (`voice`) for the
//! clients' mic mark.
use serde_json::{json, Value};

pub const OPEN: &str = "<voice_message";
const CLOSE: &str = "</voice_message>";
pub const NOTE: &str = "(Spoken and transcribed: expect transcription errors such as homophones and misheard part numbers, above all in the doubtful words. Ask if something is ambiguous.)";
/// A word the engine was less sure of than this (percent) is one to doubt.
pub const DOUBT_P: u32 = 70;

/// What is known of a spoken message: how it was sent, the engine, its mean confidence (0..1), the words to doubt, whether the person typed in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta { pub via: String, pub stt: String, pub conf: Option<f64>, pub doubt: Vec<String>, pub edited: bool }
fn attr(v: &str) -> String { v.replace(['"', '<', '>', '\n'], " ").trim().to_string() }
impl Meta {
    pub fn of(v: &Value) -> Option<Meta> {
        if !v.is_object() { return None }
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        Some(Meta { via: if s("via").is_empty() { "voice".into() } else { s("via") }, stt: s("stt"), conf: v["conf"].as_f64().filter(|c| (0.0..=1.0).contains(c)),
            doubt: v["doubt"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(|w| attr(w))).filter(|w| !w.is_empty()).take(12).collect()).unwrap_or_default(), edited: v["edited"] == true })
    }
    pub fn json(&self) -> Value { json!({"via": self.via, "stt": self.stt, "conf": self.conf, "doubt": self.doubt, "edited": self.edited}) }
}
/// The message as the agent gets it.
pub fn wrap(text: &str, m: &Meta) -> String {
    let mut a = format!(" via=\"{}\"", attr(&m.via));
    if !m.stt.is_empty() { a.push_str(&format!(" stt=\"{}\"", attr(&m.stt))) }
    if let Some(c) = m.conf { a.push_str(&format!(" confidence=\"{c:.2}\"")) }
    if !m.doubt.is_empty() { a.push_str(&format!(" doubt=\"{}\"", m.doubt.iter().map(|w| attr(w)).collect::<Vec<_>>().join("; "))) }
    a.push_str(&format!(" edited=\"{}\"", if m.edited { "yes" } else { "no" }));
    format!("{OPEN}{a}>\n{}\n{CLOSE}\n{NOTE}", text.trim())
}
/// A message's words and, when it was spoken, what its wrapper said.
pub fn split(text: &str) -> (String, Option<Meta>) {
    let Some(rest) = text.strip_prefix(OPEN) else { return (text.to_string(), None) };
    let (Some(gt), Some(close)) = (rest.find('>'), rest.rfind(CLOSE)) else { return (text.to_string(), None) };
    if close < gt { return (text.to_string(), None) }
    let head = &rest[..gt]; let body = rest[gt + 1..close].trim().to_string();
    let get = |k: &str| head.split(&format!("{k}=\"")).nth(1).and_then(|x| x.split('"').next()).unwrap_or("").to_string();
    let doubt = get("doubt"); let after = rest[close + CLOSE.len()..].trim();
    let body = if after.is_empty() || after == NOTE { body } else { format!("{body}\n{after}") };
    (body, Some(Meta { via: get("via"), stt: get("stt"), conf: get("confidence").parse().ok(), doubt: if doubt.is_empty() { vec![] } else { doubt.split("; ").map(String::from).collect() }, edited: get("edited") == "yes" }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_spoken_message_is_wrapped_for_the_agent_and_split_back_for_the_eye() {
        let m = Meta { via: "eot".into(), stt: "parakeet-v2".into(), conf: Some(0.861), doubt: vec!["PA0".into(), "Kati".into()], edited: false };
        let w = wrap(" Route the reset to PA0 and tell Kati. ", &m);
        assert_eq!(w, "<voice_message via=\"eot\" stt=\"parakeet-v2\" confidence=\"0.86\" doubt=\"PA0; Kati\" edited=\"no\">\nRoute the reset to PA0 and tell Kati.\n</voice_message>\n".to_string() + NOTE);
        let (t, back) = split(&w); assert_eq!(t, "Route the reset to PA0 and tell Kati.");
        let b = back.unwrap(); assert_eq!((b.via.as_str(), b.stt.as_str(), b.conf, b.doubt.len(), b.edited), ("eot", "parakeet-v2", Some(0.86), 2, false));
        assert_eq!(split("Typed, with <voice_message> inside"), ("Typed, with <voice_message> inside".to_string(), None), "only a wrapper at the start counts");
        assert_eq!(split("plain").1, None);
        let q = Meta { via: "hold".into(), doubt: vec!["a\"b".into()], edited: true, ..Meta::default() }; let (t, b) = split(&wrap("x", &q)); assert_eq!(t, "x"); let b = b.unwrap(); assert!(b.edited && b.doubt == ["a b"] && b.conf.is_none());
        assert_eq!(Meta::of(&json!({"stt":"parakeet-v2","conf":0.9,"doubt":["R12"]})).unwrap().via, "voice"); assert_eq!(Meta::of(&json!("x")), None);
    }
}

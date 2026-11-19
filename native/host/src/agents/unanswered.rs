//! #1304: a message the person sends into a running turn gets an answer. Claude Code takes a `priority: "now"` line as one more text block after
//! the tool results of the step in flight, and Fable 5.2 answered it in its thinking, which the person does not see, then went on with its tools
//! (measured: 86 s, 90 s and nothing in 300 s before a visible reply; in the issue, 15 minutes). A note asking for a text reply did not change that
//! (71 s, 155 s, 262 s); a note asking for a tool call did: the model is calling tools, and `thread_reply` posts its words in the thread at once
//! (5 s, three runs of three). Codex starts its turn again with the message as the prompt; it gets the same note (and called the tool with
//! `message` for `text`: both are taken).
//! So the person's own mid-turn message goes to the agent with the note ahead of it (`wrap`); a bot's, another thread's or a report's does not: no one
//! waits on those. And when the agent still has not answered after `after_ms`, the thread says so with what to do (`picture`), from the setting
//! `unanswered.<thread>` the thread's actor keeps (agents/mod.rs: armed by the message, cleared by an assistant message, `thread_reply` or the turn's end).
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// What the agent is told ahead of the person's mid-turn message.
pub const NOTE: &str = "The person sent this message while you work and is waiting for your answer. Answer them now, before any other tool call: call the Backplane tool `thread_reply` (mcp__backplane__thread_reply) with your answer as `text` (it posts it in the thread at once; they do not see your thinking). Answer what they ask (asked for a status, say where the work stands and what is next), briefly, then carry on with the work.";
/// The person's mid-turn message as the agent gets it: the note, then their words as they wrote them.
pub fn wrap(text: &str) -> String { format!("<sent_while_you_work>\n{NOTE}\n</sent_while_you_work>\n\n{text}") }

/// How long a mid-turn message from the person may go without an assistant message before the thread says so.
pub const AFTER_MS: u64 = 60_000;
static AFTER: AtomicU64 = AtomicU64::new(AFTER_MS);
pub fn after_ms() -> u64 { AFTER.load(Relaxed) }
/// Tests: a shorter wait (the process's own; tests that read it set the same value).
#[cfg(test)] pub fn set_after_ms(ms: u64) { AFTER.store(ms, Relaxed) }

pub fn key(thread: &str) -> String { format!("unanswered.{thread}") }
pub const HEAD: &str = "The agent hasn't answered your message yet";

/// The setting's value: the messages not answered (oldest first), their words, and when the first was sent.
pub fn value(msgs: &[(String, String, u64)]) -> String {
    if msgs.is_empty() { return String::new() }
    json!({"msgs": msgs.iter().map(|m| m.0.clone()).collect::<Vec<_>>(), "text": msgs.iter().map(|m| m.1.as_str()).collect::<Vec<_>>().join("\n\n"), "at": msgs[0].2}).to_string()
}

/// The thread's notice (the hub's picture, `thread.unanswered`): null unless the turn runs and the setting holds a message. Interrupt stops the
/// turn; Send again stops it and sends the words again as a new turn (`thread.again`).
pub fn picture(setting: &str, running: bool) -> Value {
    if !running || setting.is_empty() { return Value::Null }
    let Ok(v) = serde_json::from_str::<Value>(setting) else { return Value::Null };
    let text = v["text"].as_str().unwrap_or("");
    if text.trim().is_empty() { return Value::Null }
    let first: String = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(120).collect();
    json!({"head": HEAD, "text": first, "since": v["at"], "msgs": v["msgs"],
        "buttons": [{"label": "Interrupt", "action": "interrupt", "value": "", "primary": false},
                    {"label": "Send again as a new turn", "action": "again", "value": "", "primary": true}]})
}

/// Whether events a thread's actor commits answer the person: an assistant message (not the text a cut-in kept from before it) or the turn's end.
pub fn answers(ev: &[Value]) -> bool {
    ev.iter().any(|e| (e["$"] == "MessagePosted" && e["role"] == "assistant" && !e["msg"].as_str().unwrap_or("").ends_with(":before") && !e["text"].as_str().unwrap_or("").trim().is_empty())
        || (e["$"] == "TurnChanged" && e["state"] != "running"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_note_comes_first_and_the_words_stay_whole() {
        let w = wrap("Status?\nand the pins?"); assert!(w.starts_with("<sent_while_you_work>\n")); assert!(w.contains("before any other tool call")); assert!(w.contains("`thread_reply`")); assert!(w.ends_with("\n\nStatus?\nand the pins?"));
    }
    #[test] fn the_notice_shows_only_while_the_turn_runs() {
        let v = value(&[("m1".into(), "Status?".into(), 100), ("m2".into(), "twist lock?".into(), 130)]);
        let p = picture(&v, true); assert_eq!(p["head"], HEAD); assert_eq!(p["text"], "Status?"); assert_eq!(p["since"], 100); assert_eq!(p["msgs"], json!(["m1", "m2"]));
        assert_eq!(p["buttons"].as_array().unwrap().iter().map(|b| b["action"].as_str().unwrap()).collect::<Vec<_>>(), ["interrupt", "again"]);
        assert!(picture(&v, false).is_null(), "a turn that ended shows none"); assert!(picture("", true).is_null()); assert_eq!(value(&[]), "");
    }
    #[test] fn an_assistant_message_or_the_end_answers_and_the_kept_text_does_not() {
        assert!(answers(&[json!({"$":"MessagePosted","role":"assistant","msg":"a1","text":"on step 4"})]));
        assert!(answers(&[json!({"$":"TurnChanged","state":"interrupted"})]));
        assert!(!answers(&[json!({"$":"MessagePosted","role":"assistant","msg":"m1:before","text":"what streamed"})]));
        assert!(!answers(&[json!({"$":"TurnChanged","state":"running"}), json!({"$":"ActivityLogged","tone":"tool"})]));
    }
}

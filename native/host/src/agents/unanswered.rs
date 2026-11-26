//! #1304: a message the person sends into a running turn gets an answer. Claude Code takes a `priority: "now"` line as one more text block after
//! the tool results of the step in flight, and Fable 5.2 answered it in its thinking, which the person does not see, then went on with its tools
//! (measured: 86 s, 90 s and nothing in 300 s before a visible reply; in the issue, 15 minutes). A note asking for a text reply did not change that
//! (71 s, 155 s, 262 s); a note asking for a tool call did: the model is calling tools, and `thread_reply` posts its words in the thread at once
//! (5 s, three runs of three). Codex starts its turn again with the message as the prompt; it gets the same note (and called the tool with
//! `message` for `text`: both are taken).
//! So the person's own mid-turn message goes to the agent with the note ahead of it (`wrap`); a bot's, another thread's or a report's does not: no one
//! waits on those. And when the agent still has not answered, the thread says so with what to do (`picture`), from the setting
//! `unanswered.<thread>` the thread's actor keeps (agents/mod.rs: armed by the message, cleared by an assistant message, `thread_reply` or the turn's end).
//! When is decided by `due`: the failure is a model that went on with its tools, so a quiet agent is not taken for one. Claude reads the line only
//! once the step in flight is done (a long build first), and a model may think a minute or more before it writes. The notice shows once the agent
//! started `TOOLS` tool calls after the message and `after_ms` passed, or after `quiet_ms` with no answer at all. Only Claude's turns arm it: any
//! other provider's turn starts again with the message as its prompt (Steer.go), so it cannot have gone past it, and Send again would only repeat that.
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// What the agent is told ahead of the person's mid-turn message.
pub const NOTE: &str = "The person sent this message while you work and is waiting for your answer. Answer them now, before any other tool call: call the Backplane tool `thread_reply` (mcp__backplane__thread_reply) with your answer as `text` (it posts it in the thread at once; they do not see your thinking). Answer what they ask (asked for a status, say where the work stands and what is next), briefly, then carry on with the work.";
/// The person's mid-turn message as the agent gets it: the note, then their words as they wrote them.
pub fn wrap(text: &str) -> String { format!("<sent_while_you_work>\n{NOTE}\n</sent_while_you_work>\n\n{text}") }

/// How long a mid-turn message from the person may go without an assistant message, while the agent goes on with its tools, before the thread says so.
pub const AFTER_MS: u64 = 90_000;
/// Tool calls the agent starts after the message, with no answer, that show it went on past it.
pub const TOOLS: u32 = 2;
/// With fewer tool calls than that (a long tool still running, a long think), the thread says so only after this long: `after_ms` × 10/3 (300 s).
pub fn quiet_ms() -> u64 { after_ms() * 10 / 3 }
/// Unanswered.due: `None` the thread says so now; `Some(ms)` look again after ms (it may come sooner: `TOOLS` reached). `waited` since the oldest
/// unanswered message, `tools` started since it.
pub fn due(waited: u64, tools: u32, after: u64, quiet: u64) -> Option<u64> {
    if waited >= quiet || (tools >= TOOLS && waited >= after) { None } else if tools >= TOOLS { Some(after - waited) } else { Some(quiet - waited) }
}
/// Whether an event is a tool call the agent started (the timeline's tool row).
pub fn is_tool(e: &Value) -> bool { e["$"] == "ActivityLogged" && e["tone"] == "tool" }
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
    #[test] fn a_quiet_agent_gets_time_and_one_that_goes_on_with_its_tools_does_not() {
        let (a, q) = (90_000, 300_000);
        assert_eq!(due(10_000, 0, a, q), Some(290_000), "nothing yet: look again at the quiet limit");
        assert_eq!(due(10_000, 2, a, q), Some(80_000), "went on with its tools: at after_ms");
        assert_eq!(due(95_000, 1, a, q), Some(205_000), "one tool call (the step in flight's) is not enough");
        assert_eq!(due(95_000, 2, a, q), None); assert_eq!(due(300_000, 0, a, q), None, "silent this long: say so anyway");
        assert!(is_tool(&json!({"$":"ActivityLogged","tone":"tool","kind":"shell"}))); assert!(!is_tool(&json!({"$":"ActivityLogged","tone":"info","kind":"steer"})));
    }
    #[test] fn an_assistant_message_or_the_end_answers_and_the_kept_text_does_not() {
        assert!(answers(&[json!({"$":"MessagePosted","role":"assistant","msg":"a1","text":"on step 4"})]));
        assert!(answers(&[json!({"$":"TurnChanged","state":"interrupted"})]));
        assert!(!answers(&[json!({"$":"MessagePosted","role":"assistant","msg":"m1:before","text":"what streamed"})]));
        assert!(!answers(&[json!({"$":"TurnChanged","state":"running"}), json!({"$":"ActivityLogged","tone":"tool"})]));
    }
}

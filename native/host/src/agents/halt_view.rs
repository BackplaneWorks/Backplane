//! Halt cards, the part a view needs (agents/halt.rs has the rest): whether a provider's error says the context is full, a card's timeline
//! entry and its setting's key, and the words a card's actions send. Nothing here reaches the hub, so the screen audit builds it too.
use serde_json::{json, Value};

/// A full context: tokens used and the model's window when the text or the thread's last reading says them (0: not known), and whether it was
/// the agent's own compaction that failed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ctx { pub used: u64, pub limit: u64, pub compact_failed: bool }

/// The words providers use for a context that no longer fits (lowercase). Claude ("Prompt is too long", "prompt is too long: 213456 tokens >
/// 200000 maximum", "input length and `max_tokens` exceed context limit", Claude Code's "Context limit reached" and "Conversation too long"),
/// Codex ("ran out of room in the model's context window", `context_length_exceeded`), OpenAI-style ("maximum context length is N tokens"),
/// xAI/Grok ("maximum prompt length is N but the request contains M tokens"), Gemini/Antigravity ("The input token count (N) exceeds the
/// maximum number of tokens allowed (M)"), OpenCode (ContextOverflowError), Pi and Oh My Pi (their provider's text, as pi-ai's overflow list).
const FULL: &[&str] = &["prompt is too long", "conversation too long", "context limit reached", "input is too long", "context_length_exceeded",
    "context length exceeded", "context_window_exceeded", "contextwindowexceeded", "maximum context length", "maximum prompt length",
    "exceeds the maximum number of tokens", "exceed context limit", "exceeds context limit", "exceeds the context", "context overflow",
    "contextoverflow", "token limit exceeded", "exceeded model token limit", "reduce the length of the messages", "greater than the context length",
    "exceeds the available context size", "too many tokens", "out of room in the model's context", "context window is full", "context is full"];
/// The agent's own compaction failing (Claude Code's "Error during compaction", Pi's compaction_end with an error, "/compact" refused).
const COMPACT_FAILED: &[&str] = &["error during compaction", "could not compact", "compaction failed", "failed to compact", "unable to compact"];

/// Whether `text` (an error a provider ended its turn with) says the conversation no longer fits the model's context. A rate limit counted in
/// tokens ("tokens per minute") is not this.
pub fn context_full(provider: &str, text: &str) -> Option<Ctx> {
    let _ = provider; // one list serves every provider today; the parameter keeps the per-provider tests honest when one needs its own word
    let t = text.to_lowercase();
    if t.contains("per minute") || t.contains("per min") || t.contains("rate limit") || t.contains("rate_limit") || t.contains(" tpm") { return None }
    let compact_failed = COMPACT_FAILED.iter().any(|w| t.contains(w));
    let window = t.contains("context window") && ["exceed", "full", "too large", "too long", "out of room", "overflow"].iter().any(|w| t.contains(w));
    let input_count = t.contains("input token count") && t.contains("exceed");
    if !(compact_failed || window || input_count || FULL.iter().any(|w| t.contains(w))) { return None }
    let (used, limit) = numbers(&t);
    Some(Ctx { used, limit, compact_failed })
}

/// Token counts in a provider's words: "213456 tokens > 200000 maximum", "190000 + 21333 > 200000", "maximum context length is 128000 tokens.
/// However, your messages resulted in 130512 tokens", "input token count (1048977) exceeds the maximum number of tokens allowed (1048576)".
/// Numbers under 1000 (an HTTP status, a minute) are not counts. (used, limit); 0 where not said.
fn numbers(t: &str) -> (u64, u64) {
    let mut ns: Vec<(usize, u64)> = vec![]; let b = t.as_bytes(); let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            let s = i; let mut v: u64 = 0;
            while i < b.len() && (b[i].is_ascii_digit() || (b[i] == b',' && i + 1 < b.len() && b[i + 1].is_ascii_digit())) { if b[i] != b',' { v = v.saturating_mul(10).saturating_add((b[i] - b'0') as u64) } i += 1 }
            if i < b.len() && b[i].is_ascii_alphabetic() && !t[i..].starts_with("tokens") { continue } // "4o", "1m": a name, not a count
            if v >= 1000 { ns.push((s, v)) }
        } else { i += 1 }
    }
    match ns.as_slice() {
        [] => (0, 0),
        [(at, v)] => { let before = &t[..*at]; if ["maximum", "limit", "allowed", "window", "length is"].iter().any(|w| before.rfind(w).map_or(false, |p| *at - p < 40)) { (0, *v) } else { (*v, 0) } }
        [.., (_, last)] => {
            let first = ns[0].1;
            if t[ns[0].0..].contains('>') { (first, *last) } // "a tokens > b", "a + b > c": the request, then the most the model takes
            else { let (a, b) = (first, *last); (a.max(b), a.min(b)) } // otherwise what overflowed is the larger
        }
    }
}

/// 213456 -> "213,456"
pub fn group(n: u64) -> String { let s = n.to_string(); let mut o = String::new(); for (i, c) in s.chars().enumerate() { if i > 0 && (s.len() - i) % 3 == 0 { o.push(',') } o.push(c) } o }

/// A size for people: "38.4 GB", "400 MB".
pub fn human(bytes: u64) -> String { let mb = bytes >> 20; if mb >= 1024 { format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64) } else { format!("{mb} MB") } }
/// Whether the card can offer Compact: Claude Code takes `/compact` as a message.
pub fn can_compact(provider: &str) -> bool { provider == "claude" || provider.is_empty() }

/// The setting a halt row's card is kept in: `halt.<thread>.<act>` (activity ids repeat across threads).
pub fn key(th: &str, act: &str) -> String { format!("halt.{th}.{act}") }
/// A card's state after one of its actions: the action taken (`took`), so the card shows what was done in place of its buttons.
pub fn took(card: &str, what: &str) -> String { let mut v: Value = serde_json::from_str(card).unwrap_or(json!({})); v["took"] = json!(what); v.to_string() }

/// The timeline entry for a halt row (present/thread.rs): head, the plain sentence, facts (label, value, and a fill 0..1 for a meter), the
/// actions (`action`/`value` for the client's Command::Action: `halt` with `<act>|<do>`, or the composer's `retry`), and the raw text for Details.
pub fn entry(id: &str, text: &str, card: &Value) -> Value {
    let cause = card["cause"].as_str().unwrap_or("");
    let b = |label: &str, action: &str, value: String, primary: bool| json!({"label": label, "action": action, "value": value, "primary": primary});
    let go = |what: &str| format!("{id}|{what}");
    let mut facts: Vec<Value> = vec![];
    let (head, buttons) = if cause == "memory" {
        let why = card["why"].as_str().unwrap_or("");
        if let Some(p) = card["proc"].as_str() { facts.push(json!({"label": "Process", "value": format!("{p} · {}", human(card["mb"].as_u64().unwrap_or(0) << 20))})) }
        let cap = card["capBytes"].as_u64().unwrap_or(0);
        if why == "cap" && cap > 0 { let used = card["mb"].as_u64().unwrap_or(0) << 20; facts.push(json!({"label": "Limit", "value": card["cap"], "fill": (used as f64 / cap as f64).min(1.0)})) }
        else if let Some(c) = card["cap"].as_str() { facts.push(json!({"label": "Limit", "value": c})) }
        facts.push(json!({"label": "Cause", "value": match why { "cap" => "this thread's memory limit", "shared" => "the limit all agents share", "machine" => "the computer ran out of memory", _ => "killed (signal 9)" }}));
        let mut v = vec![b("Resume", "halt", go("resume"), true), b("Retry last message", "retry", String::new(), false)];
        if let Some(r) = card["raiseShown"].as_str().filter(|_| why == "cap") { v.push(b(&format!("Raise limit to {r}"), "halt", go("raise"), false)) }
        ("Out of memory", v)
    } else {
        let (used, window) = (card["used"].as_u64().unwrap_or(0), card["window"].as_u64().unwrap_or(0));
        if window > 0 { let mut f = json!({"label": "Context", "value": if used > 0 { format!("{} / {} tokens", group(used), group(window)) } else { format!("{} tokens", group(window)) }}); if used > 0 { f["fill"] = json!((used as f64 / window as f64).min(1.0)) } facts.push(f) }
        let mut v = vec![];
        if card["compact"] == true { v.push(b("Compact", "halt", go("compact"), true)) }
        v.push(b("Start fresh with summary", "halt", go("fresh"), card["compact"] != true));
        v.push(b("Retry", "retry", String::new(), false));
        (if card["compactFailed"] == true { "Compaction failed" } else { "Context full" }, v)
    };
    let took = card["took"].as_str().unwrap_or("");
    let done = match took { "resume" => "Resumed", "raise" => "Limit raised; resumed", "compact" => "Compacting", "fresh" => "Started fresh with a summary", _ => "" };
    json!({"id": id, "kind": "halt", "cause": cause, "head": head, "text": text, "facts": facts, "buttons": if done.is_empty() { Value::Array(buttons) } else { json!([]) }, "done": done, "detail": card["detail"]})
}

/// A context card says it all: the provider's own words for it earlier in the same turn (Claude's "Prompt is too long" reply and its API
/// error row) are left out of the timeline.
pub fn drop_echoes(entries: &mut Vec<Value>) {
    let mut drop = std::collections::HashSet::new();
    for (i, e) in entries.iter().enumerate() {
        if !(e["kind"] == "halt" && e["cause"] == "context") { continue }
        for p in entries[..i].iter().rev() {
            if p["kind"] == "user" { break }
            if p["kind"] == "halt" && p["cause"] == "context" { drop.insert(p["id"].as_str().unwrap_or("").to_string()); continue } // a card of the same turn written before: the newest says it
            let echo = (p["kind"] == "assistant" || (p["kind"] == "act" && p["tone"].as_str().map_or(false, |t| t.contains("error")))) && context_full("", p["text"].as_str().unwrap_or("")).is_some();
            if echo { drop.insert(p["id"].as_str().unwrap_or("").to_string()); }
        }
    }
    if !drop.is_empty() { entries.retain(|e| !drop.contains(e["id"].as_str().unwrap_or(""))) }
}

/// What a halt action sends the agent (its message, from Backplane): Resume after a memory kill; the handover header for a fresh session.
pub const RESUME_TEXT: &str = "[Backplane] Your process was stopped for memory before your last turn ended (the Out of memory card in this thread says what and why). Your session is resumed: carry on where you left off. Keep memory use down: avoid loading very large files or outputs whole, and stream or split big jobs.";
pub const FRESH_HEAD: &str = "The conversation no longer fit in the model's context, so you start a new session. Recent conversation (at most 128 items), oldest first; use thread_read with threadId {th} for earlier history:";


#[cfg(test)]
mod tests {
    use super::*;
    fn full(p: &str, t: &str) -> Ctx { context_full(p, t).unwrap_or_else(|| panic!("{p}: not seen as a full context: {t}")) }
    fn not(p: &str, t: &str) { assert!(context_full(p, t).is_none(), "{p}: wrongly seen as a full context: {t}") }

    #[test] fn claude_says_its_context_is_full() {
        assert_eq!(full("claude", "Prompt is too long"), Ctx::default());
        assert_eq!(full("claude", r#"API Error: 400 {"type":"error","error":{"type":"invalid_request_error","message":"prompt is too long: 213456 tokens > 200000 maximum"}}"#), Ctx { used: 213456, limit: 200000, compact_failed: false });
        assert_eq!(full("claude", "input length and `max_tokens` exceed context limit: 190000 + 21333 > 200000, decrease input length or `max_tokens` and try again"), Ctx { used: 190000, limit: 200000, compact_failed: false });
        assert!(full("claude", "Context limit reached · /compact or /clear to continue").limit == 0);
        assert!(full("claude", "Error during compaction: Error: Conversation too long. Press esc twice to go up a few messages and try again.").compact_failed);
        not("claude", "API Error: 529 {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}");
        not("claude", "This request would exceed the rate limit for your organization of 400,000 input tokens per minute");
        not("claude", "error_max_turns");
    }
    #[test] fn codex_says_its_context_is_full() {
        assert_eq!(full("codex", "Codex ran out of room in the model's context window. Start a new conversation or clear earlier history before retrying."), Ctx::default());
        assert!(full("codex", r#"{"error":{"code":"context_length_exceeded","message":"Your input exceeds the context window of this model. Please adjust your input and try again."}}"#).limit == 0);
        assert_eq!(full("codex", "This model's maximum context length is 128000 tokens. However, your messages resulted in 130512 tokens. Please reduce the length of the messages."), Ctx { used: 130512, limit: 128000, compact_failed: false });
        not("codex", "You exceeded your current quota, please check your plan and billing details");
        not("codex", "Rate limit reached for gpt-5 on tokens per min (TPM): Limit 30000, Used 29000");
    }
    #[test] fn grok_says_its_context_is_full() {
        assert_eq!(full("grok", "This model's maximum prompt length is 131072 but the request contains 145210 tokens."), Ctx { used: 145210, limit: 131072, compact_failed: false });
        not("grok", "Authentication required (sign in once with `grok login`)");
    }
    #[test] fn antigravity_says_its_context_is_full() {
        assert_eq!(full("antigravity", "The input token count (1048977) exceeds the maximum number of tokens allowed (1048576)."), Ctx { used: 1048977, limit: 1048576, compact_failed: false });
        not("antigravity", "missing API key (Settings › Agents › Antigravity: add a key or sign in)");
    }
    #[test] fn opencode_says_its_context_is_full() {
        assert!(full("opencode", "ContextOverflowError: prompt is too long for the model").used == 0);
        assert!(full("opencode", "The prompt is too long for the selected model's context window.").limit == 0);
        not("opencode", "No models are available (sign in once with `opencode auth login`)");
    }
    #[test] fn pi_and_oh_my_pi_say_their_context_is_full() {
        assert_eq!(full("pi", "400 Input is too long for requested model."), Ctx::default());
        assert!(full("pi", "Pi could not compact the conversation: too big").compact_failed);
        assert_eq!(full("omp", "Request exceeds the available context size, try increasing it (n_ctx: 32768, tokens: 40961)"), Ctx { used: 40961, limit: 32768, compact_failed: false }, "a local model's window (llama.cpp)");
        assert!(full("omp", "Your input exceeds the context window of this model").limit == 0);
        not("pi", "Pi aborted the turn"); not("omp", "provider down");
    }
    #[test] fn muse_says_its_context_is_full() {
        assert!(context_full("muse", "context window exceeded").is_some());
        assert!(context_full("muse", "token limit exceeded for this model (limit 200000)").map_or(false, |c| c.limit == 200000));
        not("muse", "Not authenticated (sign in once with `muse login`)"); not("muse", "muse turn cancelled");
    }
    #[test] fn a_context_card_hides_the_providers_own_words_for_it_in_its_turn() {
        let mut es = vec![json!({"id":"u","kind":"user","text":"Prompt is too long"}), json!({"id":"a","kind":"assistant","text":"Prompt is too long"}),
            json!({"id":"r","kind":"act","tone":"error","text":"Prompt is too long"}), json!({"id":"h","kind":"halt","cause":"context","text":"The conversation no longer fits"})];
        drop_echoes(&mut es); assert_eq!(es.iter().map(|e| e["id"].as_str().unwrap()).collect::<Vec<_>>(), ["u", "h"], "the person's own words stay");
        let mut twice = vec![json!({"id":"h1","kind":"halt","cause":"context","text":""}), json!({"id":"h2","kind":"halt","cause":"context","text":""})];
        drop_echoes(&mut twice); assert_eq!(twice.len(), 1); assert_eq!(twice[0]["id"], "h2");
        let mut m = vec![json!({"id":"a","kind":"assistant","text":"Prompt is too long"}), json!({"id":"h","kind":"halt","cause":"memory","text":""})];
        drop_echoes(&mut m); assert_eq!(m.len(), 2);
    }
}

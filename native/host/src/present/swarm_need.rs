//! What a swarm wants of the person (docs/SWARM.md, Needs you): one plain line saying what happened, and the one or two actions that resolve it. Pure, so the
//! sidebar's sections, the project page, the plane and the phones say the same thing. The hub writes the facts the line is made from in `room.need.<room>`
//! (bots/swarm.rs: which part of the budget ran out, who a pause is for, the members that failed and why); the room's state and reason, and each member's
//! turn and asks, come from the fold.
use serde_json::{json, Value};

/// The setting the hub keeps a swarm's need facts in: `{"for": "person"|"starter", "part": "posts"|"turns"|"tokens", "failed": [{thread, cause, say, at}]}`.
pub fn key(room: &str) -> String { format!("room.need.{room}") }

/// One button of a need. `act`: `resume`, `raise` (the budget, then resume), `board` (open the swarm's page), `signin` (the member's provider sign-in),
/// `move` (the member's work to a Claude member), `retry` (the member's turn again), `open` (the member's thread). Each but `board` and `open` is the
/// `swarm.act` request `{room, action: act, thread}`.
#[derive(Clone, Debug, PartialEq)]
pub struct Act { pub label: String, pub act: &'static str, pub thread: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Need { pub line: String, pub acts: Vec<Act>, pub why: &'static str }
impl Need {
    /// The row's `need` field: `{line, why, actions: [{label, act, room, thread}]}`, the room and thread ids with the hub's prefix `hub` ("" or "<hub>|",
    /// as the row's own id has it).
    pub fn json(&self, hub: &str, room: &str) -> Value {
        let q = |id: &str| if id.is_empty() { String::new() } else { format!("{hub}{id}") };
        json!({"line": self.line, "why": self.why, "actions": self.acts.iter().map(|a| json!({"label": a.label, "act": a.act, "room": q(room), "thread": q(&a.thread)})).collect::<Vec<_>>()})
    }
}

/// A member as the need reads it: its thread, its name on the board, its provider, whether its last turn failed and whether it asks the person now.
#[derive(Clone, Debug, Default)]
pub struct Member<'a> { pub thread: &'a str, pub name: &'a str, pub provider: &'a str, pub failed: bool, pub asking: bool }

/// The provider's name as the person knows the sign-in (Antigravity runs Gemini on a Google sign-in).
pub fn provider_say(p: &str) -> &'static str {
    match p { "antigravity" | "agy" => "Gemini", "codex" => "Codex", "grok" => "Grok", "opencode" => "OpenCode", "pi" => "Pi", "omp" => "Oh My Pi", "muse" => "Muse", _ => "Claude" }
}

/// Whether a failed turn's words are a sign-in or account problem the person has to fix (an expired or missing sign-in, a key that is refused), not the work's.
pub fn signin_cause(text: &str) -> bool {
    let l = text.to_lowercase();
    ["not logged in", "not logged into", "not signed in", "sign-in expired", "sign in again", "sign-in needed", "login expired", "please log in", "please login", "run `agy`", "/login",
     "oauth token has expired", "token has expired", "invalid api key", "invalid_api_key", "missing api key", "authentication failed", "authentication_error", "unauthenticated",
     "401 unauthorized", "status 401", "silent auth failed", "invalid_grant", "refresh token", "credentials", "subscription_required"]
        .iter().any(|k| l.contains(k))
}

/// The failed record of one member in the hub's facts (`failed[]`), if any.
fn failed_rec<'a>(rec: &'a Value, thread: &str) -> Option<&'a Value> { rec["failed"].as_array()?.iter().find(|f| f["thread"] == thread) }

fn act(label: impl Into<String>, act: &'static str, thread: &str) -> Act { Act { label: label.into(), act, thread: thread.into() } }

/// The need of a live swarm in `state` ("" active, "paused") for `reason` (idle, budget, circular, chatter, person), with the hub's facts `rec` and its members.
/// None: nothing waits on the person (it runs, the person paused it, or the swarm's starter, a bot, handles a routine pause by policy).
/// Order: a member that waits on the person's sign-in (only they can do it), a member's question, then the pause, then any other failure.
pub fn of(state: &str, reason: &str, rec: &Value, members: &[Member]) -> Option<Need> {
    if state == "stopped" || state == "done" { return None }
    let starter = rec["for"] == "starter";
    for m in members.iter().filter(|m| m.failed) {
        if failed_rec(rec, m.thread).map_or(false, |f| f["cause"] == "sign-in") {
            let p = provider_say(m.provider);
            let mut acts = vec![act(format!("Sign in to {p}"), "signin", m.thread)];
            if p != "Claude" { acts.push(act("Move to Claude", "move", m.thread)) }
            return Some(Need { line: format!("Member {} failed: {p} sign-in expired", m.name), acts, why: "sign-in" })
        }
    }
    if let Some(m) = members.iter().find(|m| m.asking) {
        return Some(Need { line: format!("{} asks you something", m.name), acts: vec![act("Answer", "open", m.thread)], why: "question" })
    }
    if state == "paused" && !starter {
        let need = |line: String, acts: Vec<Act>, why| Some(Need { line, acts, why });
        match reason {
            "person" | "" => {}
            "idle" => return need("Paused: all members idle 10 min".into(), vec![act("Resume", "resume", "")], "idle"),
            "budget" => {
                let part = rec["part"].as_str().filter(|p| !p.is_empty()).unwrap_or("posts/turns/tokens");
                return need(format!("Paused: budget used ({part})"), vec![act("Raise budget", "raise", ""), act("Resume", "resume", "")], "budget")
            }
            "circular" => return need("Paused: going in circles".into(), vec![act("Read board", "board", ""), act("Resume", "resume", "")], "circular"),
            "chatter" => return need("Paused: talking with no claim or done".into(), vec![act("Read board", "board", ""), act("Resume", "resume", "")], "chatter"),
            r => return need(format!("Paused: {r}"), vec![act("Read board", "board", ""), act("Resume", "resume", "")], "paused"),
        }
    }
    if starter { return None }
    if let Some(m) = members.iter().find(|m| m.failed) {
        let say = failed_rec(rec, m.thread).and_then(|f| f["say"].as_str()).map(short).filter(|s| !s.is_empty()).unwrap_or_else(|| "its last turn failed".into());
        return Some(Need { line: format!("Member {} failed: {say}", m.name), acts: vec![act("Retry", "retry", m.thread), act("Read board", "board", "")], why: "failed" })
    }
    None
}

/// A failure's words cut to one short clause (its first sentence, at most 80 characters).
pub fn short(t: &str) -> String {
    let first = t.trim().lines().next().unwrap_or("").trim();
    let cut = first.find(". ").map_or(first, |i| &first[..i]).trim_end_matches('.');
    let mut s: String = cut.chars().take(80).collect(); if cut.chars().count() > 80 { s.push('…') } s
}

/// Who a routine pause of a swarm started by a bot is for, and what the hub does about it by itself (bots/swarm.rs applies it). The person hears only what
/// needs them: money past the cap. `auto`: the times this swarm was resumed by policy since the person last acted on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Policy {
    /// the person's: the line and its actions in Needs you
    Person,
    /// the starter's: it is told, the person is not
    Starter,
    /// resume it now (the starter is told)
    Resume,
    /// raise the budget's `part` to the value given, then resume
    Raise(u64),
}
/// How many idle pauses the hub resumes by itself before it leaves the swarm paused for its starter.
pub const AUTO_RESUMES: u64 = 2;
/// The cap of an automatic raise: this many times the swarm's default budget of the part (money past it is the person's to spend).
pub const RAISE_CAP: u64 = 4;
/// `by_bot`: the swarm was started by a bot or the uberbot. `limit`/`default`: the budget part that ran out now and its default.
pub fn policy(by_bot: bool, reason: &str, auto: u64, limit: u64, default: u64) -> Policy {
    if !by_bot { return Policy::Person }
    match reason {
        "idle" => if auto < AUTO_RESUMES { Policy::Resume } else { Policy::Starter },
        "budget" => { let cap = default.saturating_mul(RAISE_CAP); let step = (limit / 2).max(default / 2).max(1);
            if limit >= cap { Policy::Person } else { Policy::Raise((limit + step).min(cap)) } }
        // a loop or chatter: the starter reads the board and decides; it is no question of money or sign-in
        _ => Policy::Starter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m<'a>(t: &'a str, n: &'a str, p: &'a str) -> Member<'a> { Member { thread: t, name: n, provider: p, failed: false, asking: false } }
    fn labels(n: &Need) -> Vec<(&str, &str)> { n.acts.iter().map(|a| (a.label.as_str(), a.act)).collect() }

    #[test] fn every_pause_says_what_happened_in_one_line_with_the_action_that_resolves_it() {
        let ms = [m("a", "opus", "claude"), m("b", "gemini", "antigravity")];
        let idle = of("paused", "idle", &Value::Null, &ms).unwrap();
        assert_eq!((idle.line.as_str(), labels(&idle)), ("Paused: all members idle 10 min", vec![("Resume", "resume")]));
        let b = of("paused", "budget", &json!({"part":"tokens"}), &ms).unwrap();
        assert_eq!((b.line.as_str(), labels(&b)), ("Paused: budget used (tokens)", vec![("Raise budget", "raise"), ("Resume", "resume")]));
        assert_eq!(of("paused", "budget", &Value::Null, &ms).unwrap().line, "Paused: budget used (posts/turns/tokens)", "an older pause, no part recorded");
        let c = of("paused", "circular", &Value::Null, &ms).unwrap();
        assert_eq!((c.line.as_str(), labels(&c)), ("Paused: going in circles", vec![("Read board", "board"), ("Resume", "resume")]));
        assert!(of("paused", "person", &Value::Null, &ms).is_none(), "the person's own pause asks nothing of them");
        assert!(of("", "", &Value::Null, &ms).is_none(), "a running swarm needs nobody");
        assert!(of("done", "", &Value::Null, &ms).is_none());
    }
    #[test] fn a_member_that_failed_on_its_sign_in_says_so_and_offers_the_sign_in_and_a_move() {
        let mut g = m("b", "gemini", "antigravity"); g.failed = true;
        let rec = json!({"failed":[{"thread":"b","cause":"sign-in","say":"You are not logged into Antigravity"}]});
        let n = of("", "", &rec, &[m("a", "opus", "claude"), g.clone()]).unwrap();
        assert_eq!((n.line.as_str(), labels(&n), n.why), ("Member gemini failed: Gemini sign-in expired", vec![("Sign in to Gemini", "signin"), ("Move to Claude", "move")], "sign-in"));
        assert_eq!(n.acts[0].thread, "b");
        assert_eq!(n.json("h|", "r1")["actions"][0], json!({"label":"Sign in to Gemini","act":"signin","room":"h|r1","thread":"h|b"}));
        assert_eq!(n.json("", "r1")["actions"][1]["room"], "r1");
        let n2 = of("paused", "idle", &json!({"for":"starter","failed":[{"thread":"b","cause":"sign-in"}]}), &[g.clone()]).unwrap();
        assert_eq!(n2.why, "sign-in", "a sign-in is the person's even in a bot's swarm, and before its pause");
        let other = of("", "", &json!({"failed":[{"thread":"b","cause":"other","say":"The build broke. Then more."}]}), &[g.clone()]).unwrap();
        assert_eq!((other.line.as_str(), labels(&other)), ("Member gemini failed: The build broke", vec![("Retry", "retry"), ("Read board", "board")]));
        assert_eq!(of("", "", &Value::Null, &[g]).unwrap().line, "Member gemini failed: its last turn failed", "before the hub has read why");
        let mut c = m("a", "opus", "claude"); c.failed = true;
        assert_eq!(labels(&of("", "", &json!({"failed":[{"thread":"a","cause":"sign-in"}]}), &[c]).unwrap()), vec![("Sign in to Claude", "signin")], "no move to the provider it is on");
    }
    #[test] fn a_bots_routine_pause_is_its_starters_and_a_question_is_the_persons() {
        let ms = [m("a", "opus", "claude")];
        assert!(of("paused", "idle", &json!({"for":"starter"}), &ms).is_none());
        assert!(of("paused", "budget", &json!({"for":"starter","part":"turns"}), &ms).is_none());
        let mut q = m("a", "opus", "claude"); q.asking = true;
        let n = of("paused", "idle", &json!({"for":"starter"}), &[q]).unwrap();
        assert_eq!((n.line.as_str(), labels(&n)), ("opus asks you something", vec![("Answer", "open")]));
    }
    #[test] fn the_policy_resumes_and_raises_a_bots_swarm_and_brings_money_past_the_cap_to_the_person() {
        assert_eq!(policy(false, "idle", 0, 0, 0), Policy::Person, "the person's own swarm is theirs");
        assert_eq!(policy(true, "idle", 0, 0, 0), Policy::Resume);
        assert_eq!(policy(true, "idle", AUTO_RESUMES, 0, 0), Policy::Starter, "idle again and again: the starter decides");
        assert_eq!(policy(true, "budget", 0, 120, 120), Policy::Raise(180));
        assert_eq!(policy(true, "budget", 0, 400, 120), Policy::Raise(480), "the cap: four times the default");
        assert_eq!(policy(true, "budget", 0, 480, 120), Policy::Person, "past the cap it is money: the person's");
        assert_eq!(policy(true, "circular", 0, 0, 0), Policy::Starter);
    }
    #[test] fn sign_in_words_are_told_from_the_works() {
        for t in ["You are not logged into Antigravity (silent auth failed)", "Claude sign-in expired: tap to sign in again", "Error: 401 Unauthorized", "invalid api key", "OAuth token has expired"] { assert!(signin_cause(t), "{t}") }
        for t in ["cargo test failed", "agent was killed (signal 9, likely out of memory)", "Context full"] { assert!(!signin_cause(t), "{t}") }
        assert_eq!(short("  The build broke. Then more.\nline 2"), "The build broke");
    }
}

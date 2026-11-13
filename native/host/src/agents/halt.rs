//! Halt cards: a turn that ended because the agent ran out of memory (the kernel killed it: its scope's cap, the cap every agent shares, or the
//! whole machine out of memory; memscope.rs) or out of context (the model's window full, or its compaction failed). The error row says it in
//! plain words, so every reader of the row gets the cause as it is: the sidebar's why, a launched thread's report to its parent or bot, the
//! phones. The card's facts ride beside it in the setting `halt.<thread>.<act>` (JSON), which the presenter turns into a card with its actions
//! (`entry`, present/thread.rs); the actions are the hub's `thread.halt` (resume, fresh, compact, raise) and the composer's Retry.
//! Each provider's wording for a full context is matched here (`context_full`, tests below); a provider's raw text goes only under Details.
use serde_json::{json, Value};
pub use super::halt_view::*;

/// The plain sentence for a full context (the error row's text: what every reader of the thread hears).
pub fn context_say(provider: &str, c: &Ctx) -> String {
    let who = crate::settings::fallback::provider_name(provider);
    let how = if c.used > 0 && c.limit > 0 { format!(" ({} of {} tokens)", group(c.used), group(c.limit)) } else if c.limit > 0 { format!(" (its window is {} tokens)", group(c.limit)) } else { String::new() };
    if c.compact_failed { format!("{who} could not compact the conversation to fit the model's context{how}, so the turn stopped. Nothing was lost; start fresh with a summary or retry.") }
    else { format!("The conversation no longer fits in the model's context{how}, so {who} stopped the turn. Nothing was lost; compact it or start fresh with a summary.") }
}
/// The plain sentence for an agent killed for memory: what was killed, how big it was, the limit it hit (or that the machine ran out), and that
/// nothing else was. `kill` None: it died of SIGKILL with no scope to say why (scopes off: macOS, no user systemd).
pub fn memory_say(provider: &str, kill: Option<&crate::memscope::Kill>) -> String {
    let who = crate::settings::fallback::provider_name(provider);
    let Some(k) = kill else { return format!("{who} was killed (signal 9) before the turn ended. That most often means the computer ran out of memory. The session is kept: resume it or retry.") };
    let what = match &k.big { Some(b) => format!("{} was using about {}", b.name, crate::memscope::human(b.mb << 20)), None => "it".to_string() };
    let why = match k.why {
        crate::memscope::Why::Cap => format!("and reached this thread's memory limit ({})", k.cap),
        crate::memscope::Why::Shared => "and the agents together reached the memory limit they share".to_string(),
        crate::memscope::Why::Machine => "when the computer ran out of memory".to_string(),
    };
    format!("{who} was stopped for memory: {what} {why}. Only this thread's agent was stopped; the session is kept, so it can resume.")
}

/// The card's facts for a memory kill (the setting `halt.<act>`): `raise` is the next cap when the cap was the cause and one is left.
pub fn memory_card(provider: &str, kill: Option<&crate::memscope::Kill>, raise: Option<(String, u64)>, detail: &str) -> Value {
    let mut c = json!({"cause": "memory", "provider": provider, "why": kill.map_or("unknown", |k| k.why.key()), "detail": detail});
    if let Some(k) = kill {
        if let Some(b) = &k.big { c["proc"] = json!(b.name); c["mb"] = json!(b.mb); }
        c["cap"] = json!(k.cap); c["capBytes"] = json!(k.cap_bytes);
    }
    if let Some((to, bytes)) = raise { c["raise"] = json!(to); c["raiseShown"] = json!(crate::memscope::human(bytes)); }
    c
}
/// The card's facts for a full context. `compact`: the provider takes a compaction the card can ask for (Claude's `/compact`).
pub fn context_card(provider: &str, c: &Ctx, compact: bool, detail: &str) -> Value {
    json!({"cause": "context", "provider": provider, "used": c.used, "window": c.limit, "compactFailed": c.compact_failed, "compact": compact && !c.compact_failed, "detail": detail})
}
/// Halt.enrich: a turn that ends (the batch has its TurnChanged failed) on an error that says the context is full: the error row becomes a halt
/// row (kind "halt") in plain words, and the card's facts go beside it. `usage`: the thread's last reading (used, window), for a provider that
/// does not say the counts.
pub fn enrich(ev: &mut Vec<Value>, provider: &str, usage: (u64, u64), at: u64) {
    if !ev.iter().any(|e| e["$"] == "TurnChanged" && e["state"] == "failed") { return }
    let mut add = vec![];
    // one card a turn: the last error row that says the context is full (Claude says it twice, its API error reply's row and the result's)
    let last = ev.iter().rposition(|e| e["$"] == "ActivityLogged" && e["tone"] == "error" && e["kind"] != "halt" && context_full(provider, e["summary"].as_str().unwrap_or("")).is_some());
    for (i, e) in ev.iter_mut().enumerate() {
        if Some(i) != last { continue }
        let raw = e["summary"].as_str().unwrap_or("").to_string();
        let Some(mut c) = context_full(provider, &raw) else { continue };
        if c.used == 0 && c.limit == 0 { (c.used, c.limit) = usage } else if c.limit == 0 && usage.1 > 0 { c.limit = usage.1 }
        let act = e["act"].as_str().unwrap_or("").to_string(); if act.is_empty() { continue }
        e["kind"] = json!("halt"); e["summary"] = json!(context_say(provider, &c));
        let th = e["thread"].as_str().unwrap_or("").to_string();
        let detail: String = raw.chars().take(1500).collect();
        add.push(json!({"$":"SettingSet","key":key(&th, &act),"value":context_card(provider, &c, can_compact(provider), &detail).to_string(),"at":at}));
    }
    ev.extend(add);
}

impl super::Agents {
    /// `thread.halt {thread, act, do}`: a halt card's action. resume: the session goes on with a note of what happened; raise: the cap of this
    /// thread's agent and commands goes up a step (`memory.agent.max`, for every thread from its next process), then resume; compact: Claude's
    /// `/compact`; fresh: a new session with the recent conversation handed over, and the person's last message run again. The card then says
    /// what was done in place of its buttons.
    pub async fn halt_act(self: &std::sync::Arc<Self>, th: &str, act: &str, what: &str) -> Result<(), String> {
        let (card, running) = { let w = self.world().await; (w.setting(&key(th, act)).to_string(), w.running(th)) };
        if card.is_empty() { return Err("no such card".into()) }
        if running { return Err("the thread is working now; stop it first".into()) }
        let now = super::now_s(); let id = format!("t:backplane:{}", super::hub_id("hl", now, self.seq()));
        let origin = Some(("backplane".to_string(), "halt".to_string(), String::new()));
        let set = |k: String, v: String| json!({"$":"SettingSet","key":k,"value":v,"at":now});
        // the message whose turn the kill ended is not "Failed" any more: its turn goes on
        let last = { let w = self.world().await; w.threads.get(th).map(|t| t.last_user.clone()).unwrap_or_default() };
        if !last.is_empty() && matches!(what, "resume" | "raise") { self.commit_events(vec![set(format!("failed.{last}"), String::new())]).await }
        match what {
            "resume" => self.deliver(th, &id, RESUME_TEXT, origin, vec![]).await?,
            "raise" => {
                let t = crate::memscope::total_ram().unwrap_or(0);
                let to = crate::memscope::raised(&crate::memscope::scope_max(), &crate::memscope::slice_max(), t).ok_or("the limit is already as high as the agents' shared limit")?;
                crate::memscope::set_agent_cap(&to);
                self.commit_events(vec![set("memory.agent.max".into(), to)]).await;
                self.deliver(th, &id, RESUME_TEXT, origin, vec![]).await?
            }
            "compact" => self.deliver(th, &id, "/compact", origin, vec![]).await?,
            "fresh" => {
                let (ctx, last) = crate::commands::fresh_context(&self.hub, th, &FRESH_HEAD.replace("{th}", th)).await;
                let mut ev = vec![json!({"$":"SessionSet","id":th,"session":"","at":now})];
                if !ctx.is_empty() { ev.push(json!({"$":"TurnQueued","thread":th,"msg":format!("handoff:fresh:{now}"),"text":ctx,"at":now})) }
                self.commit_events(ev).await;
                let text = if last.is_empty() { "[Backplane] Carry on from the conversation handed over above.".to_string() } else { last };
                self.deliver(th, &id, &text, None, vec![]).await?
            }
            _ => return Err("unknown action".into()),
        }
        self.commit_events(vec![set(key(th, act), took(&card, what))]).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_words_are_plain_and_say_the_numbers() {
        let s = context_say("claude", &Ctx { used: 213456, limit: 200000, compact_failed: false });
        assert_eq!(s, "The conversation no longer fits in the model's context (213,456 of 200,000 tokens), so Claude stopped the turn. Nothing was lost; compact it or start fresh with a summary.");
        assert!(context_say("pi", &Ctx { used: 0, limit: 0, compact_failed: true }).starts_with("Pi could not compact the conversation"));
        let k = crate::memscope::Kill { big: Some(crate::memscope::Big { name: "claude".into(), pid: 9, mb: 412 }), cap: "400 MB".into(), cap_bytes: 400 << 20, why: crate::memscope::Why::Cap, whole: true };
        assert_eq!(memory_say("claude", Some(&k)), "Claude was stopped for memory: claude was using about 412 MB and reached this thread's memory limit (400 MB). Only this thread's agent was stopped; the session is kept, so it can resume.");
        let m = crate::memscope::Kill { why: crate::memscope::Why::Machine, ..k.clone() };
        assert!(memory_say("codex", Some(&m)).contains("when the computer ran out of memory"));
        assert!(memory_say("grok", None).starts_with("Grok was killed (signal 9)"));
        for s in [memory_say("claude", Some(&k)), context_say("codex", &Ctx::default())] { assert!(!s.contains("panicked") && !s.contains("stack") && !s.contains('\n'), "{s}") }
    }
    #[test] fn a_turn_ending_on_a_full_context_becomes_a_halt_row_with_its_card() {
        let mut ev = vec![json!({"$":"ActivityLogged","thread":"t","act":"e1","tone":"error","kind":"result","summary":"Prompt is too long","at":5}), json!({"$":"TurnChanged","thread":"t","state":"failed","at":5})];
        enrich(&mut ev, "claude", (198000, 200000), 5);
        assert_eq!((ev[0]["kind"].as_str(), ev[0]["summary"].as_str().map(|s| s.contains("198,000 of 200,000 tokens"))), (Some("halt"), Some(true)), "{ev:?}");
        assert_eq!(ev[2]["key"], "halt.t.e1"); let c: Value = serde_json::from_str(ev[2]["value"].as_str().unwrap()).unwrap();
        assert_eq!((c["cause"].as_str(), c["compact"].as_bool(), c["detail"].as_str()), (Some("context"), Some(true), Some("Prompt is too long")));
        let mut two = vec![json!({"$":"ActivityLogged","thread":"t","act":"a","tone":"error","kind":"api","summary":"Prompt is too long","at":5}), json!({"$":"ActivityLogged","thread":"t","act":"e","tone":"error","kind":"result","summary":"Prompt is too long","at":5}), json!({"$":"TurnChanged","thread":"t","state":"failed","at":5})];
        enrich(&mut two, "claude", (0, 0), 5); assert_eq!((two[0]["kind"].as_str(), two[1]["kind"].as_str(), two.len()), (Some("api"), Some("halt"), 4), "one card a turn: the row that ends it");
        let mut mid = vec![json!({"$":"ActivityLogged","thread":"t","act":"a","tone":"error","kind":"api","summary":"Prompt is too long","at":5})];
        enrich(&mut mid, "claude", (0, 0), 5); assert_eq!(mid.len(), 1, "only the row that ends the turn"); assert_eq!(mid[0]["kind"], "api");
        let mut other = vec![json!({"$":"ActivityLogged","thread":"t","act":"e","tone":"error","kind":"codex","summary":"boom","at":5}), json!({"$":"TurnChanged","thread":"t","state":"failed","at":5})];
        enrich(&mut other, "codex", (0, 0), 5); assert_eq!(other.len(), 2); assert_eq!(other[0]["summary"], "boom");
    }
    #[test] fn the_card_offers_what_fits_the_cause() {
        let labels = |e: &Value| e["buttons"].as_array().unwrap().iter().map(|b| b["label"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        let k = crate::memscope::Kill { big: Some(crate::memscope::Big { name: "bun".into(), pid: 9, mb: 398 }), cap: "400 MB".into(), cap_bytes: 400 << 20, why: crate::memscope::Why::Cap, whole: true };
        let e = entry("x1", "said", &memory_card("claude", Some(&k), Some(("600M".into(), 600 << 20)), "signal 9"));
        assert_eq!(labels(&e), ["Resume", "Retry last message", "Raise limit to 600 MB"]); assert_eq!(e["buttons"][0]["value"], "x1|resume"); assert_eq!(e["head"], "Out of memory");
        assert!(e["facts"][1]["fill"].as_f64().unwrap() > 0.99);
        let m = crate::memscope::Kill { why: crate::memscope::Why::Machine, ..k };
        assert_eq!(labels(&entry("x", "", &memory_card("claude", Some(&m), Some(("600M".into(), 1)), ""))), ["Resume", "Retry last message"], "a limit raised does not help when the machine ran out");
        let c = entry("x2", "", &context_card("codex", &Ctx { used: 130512, limit: 128000, compact_failed: false }, can_compact("codex"), "raw"));
        assert_eq!(labels(&c), ["Start fresh with summary", "Retry"]); assert_eq!(c["facts"][0]["value"], "130,512 / 128,000 tokens");
        assert_eq!(labels(&entry("x3", "", &context_card("claude", &Ctx::default(), true, ""))), ["Compact", "Start fresh with summary", "Retry"]);
        let done = entry("x3", "", &serde_json::from_str(&took(&context_card("claude", &Ctx::default(), true, "").to_string(), "compact")).unwrap());
        assert_eq!((done["done"].as_str(), done["buttons"].as_array().map(Vec::len)), (Some("Compacting"), Some(0)));
    }
    #[test] fn a_raised_limit_steps_up_to_the_shared_one() {
        let g = 1u64 << 30;
        assert_eq!(crate::memscope::raised("40%", "75%", 64 * g).as_deref(), Some("55%")); assert_eq!(crate::memscope::raised("70%", "75%", 64 * g).as_deref(), Some("75%"));
        assert_eq!(crate::memscope::raised("75%", "75%", 64 * g), None);
        assert_eq!(crate::memscope::raised("400M", "3G", 0).as_deref(), Some("600M")); assert_eq!(crate::memscope::raised("2G", "3G", 0).as_deref(), Some("3G"));
        assert_eq!(crate::memscope::raised("24G", "75%", 64 * g).as_deref(), Some("36G"));
        assert_eq!(crate::memscope::human(400 << 20), "400 MB"); assert_eq!(crate::memscope::human(38 * g + g / 2), "38.5 GB");
    }
}

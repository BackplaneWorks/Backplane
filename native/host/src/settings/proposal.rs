//! Settings propose / approve (core/settingsproposal.bend, mcp.bend `Settings.*`, hub.bend `Settings.answer`): an agent proposes a hub-wide
//! fallback change (`fallback.on`, `fallback.chain`); the person approves it once. The tool never applies anything. The proposal is stored in
//! an ask (tool `settings_proposal`) with the settings it was made against and an expiry; any write to either setting, or the clock, makes it stale.
use super::fallback;
use crate::hub::Hub;
use serde_json::{json, Value};
use std::sync::Arc;

pub const TOOL: &str = "settings_proposal";
pub const TTL: u64 = 600;

#[derive(Clone, Debug, PartialEq)]
pub struct Proposal { pub old_on: String, pub old_chain: String, pub new_on: String, pub new_chain: String, pub expires: u64 }
fn on_valid(s: &str) -> bool { s == "on" || s == "off" }
fn chain_valid(chain: &str, caps: &Value) -> bool { let t = fallback::parse(chain); chain.chars().count() <= 2000 && t.len() <= 16 && fallback::valid(&t, caps) }
impl Proposal {
    pub fn changed(&self) -> bool { self.old_on != self.new_on || self.old_chain != self.new_chain }
    /// Proposal.valid: only the two supported settings enter.
    pub fn valid(&self, caps: &Value) -> bool {
        on_valid(&self.old_on) && on_valid(&self.new_on) && chain_valid(&self.old_chain, caps) && chain_valid(&self.new_chain, caps) && (self.new_on == "off" || !self.new_chain.is_empty())
    }
    /// Proposal.snapshot: still current: not expired, and the settings are as they were.
    pub fn snapshot(&self, now: u64, on: &str, chain: &str) -> bool { now < self.expires && self.old_on == on && self.old_chain == chain }
    /// Proposal.json: the closed record (no free-form field).
    pub fn json(&self) -> Value { json!({"old_on":self.old_on,"old_chain":self.old_chain,"new_on":self.new_on,"new_chain":self.new_chain,"expires":self.expires}) }
    pub fn from(o: &Value) -> Proposal { let s = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string(); Proposal { old_on: s("old_on"), old_chain: s("old_chain"), new_on: s("new_on"), new_chain: s("new_chain"), expires: o.get("expires").and_then(Value::as_u64).unwrap_or(0) } }
    /// Proposal.detail: what the person reads.
    pub fn detail(&self) -> String {
        format!("Hub-wide provider fallback proposal\nEnabled: {} → {}\nHierarchy: {} → {}\nValues are effective settings; an unset switch defaults to off and an unset hierarchy is empty. Unchanged settings are left untouched.\nExpires at Unix time {}.\nOn a confirmed provider limit, another eligible account of the same provider may be tried first, independently of this setting. If none is available, enabled fallback may switch active threads and bots to a later eligible provider/model; an exhausted hierarchy holds work. This change does not itself start or interrupt a conversation.",
            self.old_on, self.new_on, self.old_chain, self.new_chain, self.expires)
    }
}

/// The effective values (an unset switch is "off", an unset chain is empty).
async fn effective(hub: &Arc<Hub>) -> (String, String) {
    let on = hub.setting("fallback.on").await.filter(|s| !s.is_empty()).unwrap_or_else(|| "off".into());
    (on, hub.setting("fallback.chain").await.unwrap_or_default())
}

/// settings_inspect: the supported settings and the installed models. Never credentials.
pub async fn tool_inspect(hub: &Arc<Hub>, models: &Value) -> (bool, Value) {
    let (on, chain) = effective(hub).await;
    if !on_valid(&on) || !chain_valid(&chain, models) { return (false, json!({"error":"Fallback settings are invalid or their model catalog is unavailable; inspect them in Settings."})) }
    (true, json!({"scope":"this hub","on":on,"chain":chain,"models":models}))
}

/// Pend.fix: PendingChanged for every thread whose asks the batch closes, counting the asks that stay open (so the thread is not
/// counted as blocked once its proposal is answered or made stale).
pub fn pending_after(open_asks: &[(String, String)], batch: &[Value], at: u64) -> Vec<Value> {
    let closed: Vec<&str> = batch.iter().filter(|e| e["$"] == "AskClosed").filter_map(|e| e["id"].as_str()).collect();
    let mut threads: Vec<&str> = vec![];
    for (id, th) in open_asks { if closed.contains(&id.as_str()) && !threads.contains(&th.as_str()) { threads.push(th) } }
    threads.into_iter().map(|th| json!({"$":"PendingChanged","thread":th,"count":open_asks.iter().filter(|(id, t)| t == th && !closed.contains(&id.as_str())).count(),"at":at})).collect()
}
/// The batch with its PendingChanged events (from the agents' world).
pub async fn with_pending(ag: &Arc<crate::agents::Agents>, mut batch: Vec<Value>, at: u64) -> Vec<Value> {
    let open: Vec<(String, String)> = ag.world().await.asks.iter().map(|(i, k)| (i.clone(), k.thread.clone())).collect();
    let p = pending_after(&open, &batch, at); batch.extend(p); batch
}

/// settings_propose {on, chain}: opens the ask (never applies) and counts it as pending on the thread. `ask_id` is the hub's id for it.
pub async fn tool_propose(ag: &Arc<crate::agents::Agents>, thread: &str, models: &Value, a: &Value, now: u64, ask_id: &str) -> (bool, Value) {
    let hub = &ag.hub;
    let (on, chain) = effective(hub).await;
    let g = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let p = Proposal { old_on: on, old_chain: chain, new_on: g("on"), new_chain: g("chain"), expires: now + TTL };
    if !(p.changed() && p.valid(models)) { return (false, json!({"error":"Invalid fallback proposal or model catalog unavailable; no proposal was stored."})) }
    let ev = json!({"$":"AskOpened","id":ask_id,"thread":thread,"kind":"approval","tool":TOOL,"detail":p.detail(),"options":p.json().to_string(),"at":now});
    let pend = ag.pending_ev(thread, 1, "", now).await;
    if hub.commit_many(&[ev, pend]).await.is_err() { return (false, json!({"error":"could not store the proposal"})) }
    (true, json!({"id":ask_id,"status":"awaiting person","proposal":p.json()}))
}

/// The person answers a settings proposal (Settings.answer): the events to write and the error to reply with, if any.
/// `current` is the effective (on, chain) now. Accepting a stale proposal changes nothing.
pub fn answer_events(accept: bool, ask: &str, thread: &str, options: &str, now: u64, current: (&str, &str)) -> (Vec<Value>, Option<&'static str>) {
    let act = |text: &str| json!({"$":"ActivityLogged","thread":thread,"act":format!("settings:{ask}"),"tone":"info","kind":"settings","summary":text,"at":now});
    let closed = |why: &str| json!({"$":"AskClosed","id":ask,"answer":why,"at":now});
    if !accept { return (vec![closed("decline"), act("Settings proposal declined; no change was applied.")], None) }
    let p = Proposal::from(&serde_json::from_str::<Value>(options).unwrap_or(Value::Null));
    if !p.snapshot(now, current.0, current.1) {
        return (vec![closed("stale"), act("Settings proposal expired or the settings changed; no change was applied.")], Some("Settings proposal expired or the settings changed; request a new proposal."))
    }
    let mut ev = vec![closed("accept")];
    if p.old_chain != p.new_chain { ev.push(json!({"$":"SettingSet","key":"fallback.chain","value":p.new_chain,"at":now})) }
    if p.old_on != p.new_on { ev.push(json!({"$":"SettingSet","key":"fallback.on","value":p.new_on,"at":now})) }
    ev.push(act("Approved fallback settings applied."));
    (ev, None)
}

/// The agents' `answer` for an ask whose tool is `settings_proposal`: closes it and applies it when accepted and still current.
/// `open` is every open settings proposal (the ask being answered included): an accepted proposal writes fallback.*, which closes the
/// others "stale" in the same batch, as Bend's per-step Proposal.invalidate does. Rust has no per-step hook, so EVERY writer of
/// `fallback.on` / `fallback.chain` must call `invalidate` on its batch (setting.set and this are the only two today).
pub async fn answer_ask(ag: &Arc<crate::agents::Agents>, ask: &str, thread: &str, options: &str, answer: &str, now: u64, open: &[String]) -> Result<(), String> {
    let hub = &ag.hub;
    let (on, chain) = effective(hub).await;
    let (mut ev, err) = answer_events(answer == "accept", ask, thread, options, now, (&on, &chain));
    let extra = invalidate(&ev, open, now); ev.extend(extra);
    let ev = with_pending(ag, ev, now).await;
    hub.commit_many(&ev).await.map_err(|e| format!("{e:#}"))?;
    match err { Some(e) => Err(e.to_string()), None => Ok(()) }
}

/// Proposal.invalidate: any accepted write to either setting, even one that restores a value, closes the open proposals in the same batch
/// (an ask already closed in that batch is left alone so its decision is never overwritten).
pub fn invalidate(batch: &[Value], open_proposals: &[String], now: u64) -> Vec<Value> {
    let touched = batch.iter().any(|e| e["$"] == "SettingSet" && matches!(e["key"].as_str(), Some("fallback.on" | "fallback.chain")));
    if !touched { return vec![] }
    open_proposals.iter().filter(|id| !batch.iter().any(|e| e["$"] == "AskClosed" && e["id"].as_str() == Some(id.as_str())))
        .map(|id| json!({"$":"AskClosed","id":id,"answer":"stale","at":now})).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn caps() -> Value { json!({"claude":["opus","sonnet"],"codex":["gpt-6"]}) }
    fn p(on: &str, chain: &str) -> Proposal { Proposal { old_on: "off".into(), old_chain: "".into(), new_on: on.into(), new_chain: chain.into(), expires: 700 } }
    #[test] fn only_a_valid_change_is_a_proposal() {
        assert!(p("on", "claude/opus,codex/gpt-6").valid(&caps()) && p("on", "claude/opus").changed());
        assert!(!p("on", "").valid(&caps()), "on needs a chain"); assert!(p("off", "").valid(&caps()));
        assert!(!p("maybe", "claude/opus").valid(&caps())); assert!(!p("on", "claude/nope").valid(&caps()));
        assert!(!p("off", "").changed(), "nothing to change");
        assert!(!p("on", &vec!["claude/opus"; 17].join(",")).valid(&caps()));
    }
    #[test] fn the_record_is_closed_and_round_trips() { let x = p("on", "claude/opus"); assert_eq!(Proposal::from(&x.json()), x); assert_eq!(x.json().as_object().unwrap().len(), 5); assert!(x.detail().starts_with("Hub-wide provider fallback proposal\nEnabled: off → on\nHierarchy:  → claude/opus")); }
    #[test] fn a_snapshot_is_current_only_until_it_expires_or_the_settings_move() {
        let x = p("on", "claude/opus"); assert!(x.snapshot(699, "off", "")); assert!(!x.snapshot(700, "off", "")); assert!(!x.snapshot(10, "on", "")); assert!(!x.snapshot(10, "off", "codex/gpt-6"));
    }
    #[test] fn settings_proposal_decline_no_write() {
        let (ev, err) = answer_events(false, "s1", "t1", &p("on", "claude/opus").json().to_string(), 100, ("off", "")); assert!(err.is_none());
        assert!(ev.iter().all(|e| e["$"] != "SettingSet") && ev[0]["answer"] == "decline");
    }
    #[test] fn an_accepted_proposal_writes_only_what_changed() {
        let o = p("on", "claude/opus").json().to_string();
        let (ev, err) = answer_events(true, "s1", "t1", &o, 100, ("off", "")); assert!(err.is_none());
        let keys: Vec<&str> = ev.iter().filter(|e| e["$"] == "SettingSet").map(|e| e["key"].as_str().unwrap()).collect(); assert_eq!(keys, ["fallback.chain", "fallback.on"]);
        let mut same_chain = p("on", ""); same_chain.old_chain = "".into(); let (ev, _) = answer_events(true, "s1", "t1", &same_chain.json().to_string(), 100, ("off", ""));
        assert_eq!(ev.iter().filter(|e| e["$"] == "SettingSet").count(), 1, "the chain is unchanged: untouched");
    }
    #[test] fn a_stale_or_expired_acceptance_applies_nothing_and_says_so() {
        let o = p("on", "claude/opus").json().to_string();
        for (now, cur) in [(800u64, ("off", "")), (100, ("on", "")), (100, ("off", "codex/gpt-6"))] {
            let (ev, err) = answer_events(true, "s1", "t1", &o, now, cur); assert_eq!(err, Some("Settings proposal expired or the settings changed; request a new proposal."));
            assert!(ev.iter().all(|e| e["$"] != "SettingSet") && ev[0]["answer"] == "stale");
        }
    }
    #[test] fn settings_proposal_write_invalidates() {
        let w = vec![json!({"$":"SettingSet","key":"fallback.on","value":"off","at":1})]; // even a write that restores a value
        let open = vec!["a".to_string(), "b".to_string()];
        let ev = invalidate(&w, &open, 5); assert_eq!(ev.len(), 2); assert!(ev.iter().all(|e| e["answer"] == "stale"));
        assert!(invalidate(&[json!({"$":"SettingSet","key":"other","value":"x","at":1})], &open, 5).is_empty());
        let both = vec![w[0].clone(), json!({"$":"AskClosed","id":"a","answer":"accept","at":1})];
        let ev = invalidate(&both, &open, 5); assert_eq!(ev.len(), 1); assert_eq!(ev[0]["id"], "b", "an ask closed in the same batch keeps its decision");
    }

    #[test] fn settings_proposal_deadline_exclusive() { assert!(!Proposal { old_on: "off".into(), old_chain: "".into(), new_on: "off".into(), new_chain: "".into(), expires: 100 }.snapshot(100, "off", "")); }
    #[test] fn an_accepted_proposal_closes_the_other_open_ones_stale_in_the_same_batch() {
        let o = p("on", "claude/opus").json().to_string();
        let (mut ev, _) = answer_events(true, "s1", "t1", &o, 100, ("off", ""));
        let extra = invalidate(&ev, &["s1".to_string(), "s2".to_string()], 100); ev.extend(extra);
        let closed: Vec<(&str, &str)> = ev.iter().filter(|e| e["$"] == "AskClosed").map(|e| (e["id"].as_str().unwrap(), e["answer"].as_str().unwrap())).collect();
        assert_eq!(closed, [("s1", "accept"), ("s2", "stale")], "the accepted ask keeps its decision");
    }

    #[test] fn closing_asks_counts_what_stays_open_per_thread() {
        let open = vec![("a".to_string(), "t1".to_string()), ("b".to_string(), "t1".to_string()), ("c".to_string(), "t2".to_string()), ("d".to_string(), "t3".to_string())];
        let batch = vec![json!({"$":"AskClosed","id":"a","answer":"accept","at":1}), json!({"$":"AskClosed","id":"c","answer":"stale","at":1})];
        let p = pending_after(&open, &batch, 9);
        assert_eq!(p, vec![json!({"$":"PendingChanged","thread":"t1","count":1,"at":9}), json!({"$":"PendingChanged","thread":"t2","count":0,"at":9})], "t3 is untouched");
    }
}

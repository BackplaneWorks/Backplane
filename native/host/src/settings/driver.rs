//! The provider-limit driver: what happens when a running turn's agent line says its account ran out of allowance or its login failed. The agents' actor calls it at three
//! points: `on_line` for each line right after the line's events are built, `on_exit` when a process ended a running turn, and `spawn_act` after the batch is committed.
//! The decisions are the pure planner's in fallback.rs and `next` below.
//!
//! * A limit seen in a turn that has not ended is only remembered (`seen`, kept by the actor, and `fallback.pending.<thread>` or `account.failover.pending.<thread>`): since Claude
//!   Code 2.1.286 a turn ends on its command's lifecycle line, after the result line that carried the error, and often in the same read.
//! * A turn that ended well forgets it. A turn that failed with one (on its own line or remembered) is acted on: the hold is written, on the thread's account or on a
//!   provider/model, the turn is Interrupted instead of Failed (marked `failover`: its task and launcher are not told it ended), and `spawn_act` carries on.
//! * `next` says where: another connected account of the provider with allowance left that this request has not run out on, else (opt-in chain) the next chain target, else the
//!   thread waits (`provider.resume.<thread>`, continued by the ticker at the earliest reset, on whatever is free then) and says so.
//! * The switch itself is `accounts.choose` / `fallback.switch` (commands/plan.rs): the same planner as the person's pick, which keeps each account's native session (a session
//!   belongs to one account's own store; the CLI cannot resume it under another login) and hands over what the account taking over has not seen. The turn then starts again.
//! * Once the account a thread left has reset, the thread goes back to it between turns (`returns`).
//! NOT ported: the preflight that also chooses on a manual start of a held thread (a person's message to a held account finds out on the provider's answer, then fails over).
use super::{accounts, fallback as fb, models};
use crate::{agents::{rules, Agents}, api::{Caller, Fut, Ticker}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Default)]
pub struct Verdict { pub events: Vec<Value>, pub drop_turn_end: bool, pub act: Option<Act> }
#[derive(Debug, Clone, PartialEq)]
pub enum Act { Restart { text: String } }

/// Fallback.continue: what the restarted turn is told.
pub fn continue_text(original: &str) -> String {
    let head = "[Backplane] The account this turn ran on stopped (usage limit or sign-in), so the turn starts again. Continue the interrupted work from its current state. Read the handed-over tool results before acting. Do not repeat completed tool actions or restart finished work. If the request is already satisfied, report that.";
    if original.trim().is_empty() { head.to_string() } else { format!("{head}\n\nOriginal request (context, not a new task):\n{original}") }
}
fn set(k: &str, v: &str, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }
/// The turn stops for a switch and goes on: `failover` says it has not ended, for whoever waits on its end (tasks.rs `ended_last`, launch.rs).
fn turn(th: &str, state: &str, at: u64) -> Value { json!({"$":"TurnChanged","thread":th,"state":state,"failover":true,"at":at}) }
fn act_row(th: &str, tone: &str, kind: &str, text: &str, at: u64) -> Value { json!({"$":"ActivityLogged","thread":th,"act":format!("{kind}{at}"),"tone":tone,"kind":kind,"summary":text,"at":at}) }
pub fn tried_key(th: &str) -> String { format!("account.failover.tried.{th}") }

/// Pending.due: a limit still in force (reset unknown, or in the future).
pub fn due(lim: &fb::Limit, now: u64) -> bool { lim.reset == 0 || now < lim.reset }

/// What the driver reads of the hub for one decision.
pub struct World { pub now: u64, pub thread: String, pub provider: String, pub model: String, pub settings: HashMap<String, String>, pub caps: Value }
impl World {
    pub fn get(&self, k: &str) -> String { self.settings.get(k).cloned().unwrap_or_default() }
    fn inventory(&self) -> Vec<Value> { serde_json::from_str::<Value>(&self.get("accounts.inventory")).ok().and_then(|v| v["profiles"].as_array().cloned()).unwrap_or_default() }
    fn binding(&self) -> (String, u64) { let b: Value = serde_json::from_str(&self.get(&accounts::binding_key(&self.thread, &self.provider))).unwrap_or(Value::Null); (b["id"].as_str().unwrap_or("").to_string(), b["epoch"].as_u64().unwrap_or(0)) }
    /// AF.Provider.managed: the thread is on a named account, or the provider has any connected profile at all.
    pub fn managed(&self) -> bool { !self.binding().0.is_empty() || self.inventory().iter().any(|p| p["provider"] == self.provider.as_str()) }
    fn chain_on(&self) -> bool { fb::chain_on(self.settings.get("fallback.on").map(String::as_str)) }
    fn chain(&self) -> Vec<fb::Target> { fb::chain_of(self.settings.get("fallback.chain").map(String::as_str)) }
    /// A target of the order can run here: its CLI lists models (and this one, if named), or it is a provider alone with an account connected.
    fn offered(&self, t: &fb::Target) -> bool { fb::caps_valid(&self.caps, t) || (t.model.is_empty() && self.inventory().iter().any(|p| p["provider"] == t.provider.as_str() && p["state"] == "connected")) }
    /// Where `t` would run now: None when it cannot (not offered, held, or every account of it out), Some(None) on the CLI's own login (a provider with no accounts), else the account.
    fn usable(&self, t: &fb::Target) -> Option<Option<(String, u64)>> {
        let get = |k: &str| self.get(k); let profiles = self.inventory();
        if !self.offered(t) || fb::target_held(&get, t, self.now) { return None }
        if !profiles.iter().any(|p| p["provider"] == t.provider.as_str()) { return Some(None) }
        fb::pick_account(&profiles, &get, self.now, &t.provider, &t.model).map(|p| Some((p["id"].as_str().unwrap_or("").to_string(), p["epoch"].as_u64().unwrap_or(0))))
    }
    /// The first target of the order, from its top, that can run now and is not where the thread is out (its own target, or its provider alone). `above`: only the targets ranked
    /// above the thread's provider (the way back up once one has reset).
    fn best(&self, above: bool) -> Option<(fb::Target, Option<(String, u64)>)> {
        let chain = self.chain();
        let stop = if above { chain.iter().position(|t| t.provider == self.provider).unwrap_or(0) } else { chain.len() };
        chain[..stop].iter().filter(|t| !(t.provider == self.provider && (t.model.is_empty() || t.model == self.model))).find_map(|t| self.usable(t).map(|a| (t.clone(), a)))
    }
    /// "Codex (Work)", "Grok": a target and the account it runs on.
    fn target_name(&self, t: &fb::Target, account: &Option<(String, u64)>) -> String {
        let p = if t.model.is_empty() { fb::provider_name(&t.provider) } else { format!("{} {}", fb::provider_name(&t.provider), t.model) };
        match account { Some((id, _)) => format!("{p} ({})", self.name(id)), None => p }
    }
    /// The logins this request has run out on.
    fn tried(&self) -> Vec<String> { serde_json::from_str(&self.get(&tried_key(&self.thread))).unwrap_or_default() }
    fn name(&self, id: &str) -> String { accounts::label(id, &accounts::profile(&self.get("accounts.inventory"), id)) }
    fn zone(&self) -> String { self.get("tz.machine") }
    /// The settings with `events` applied (the next decision sees the hold just written).
    #[cfg(test)]
    fn clone_at(&self, now: u64) -> World { World { now, thread: self.thread.clone(), provider: self.provider.clone(), model: self.model.clone(), settings: self.settings.clone(), caps: self.caps.clone() } }
    #[cfg(test)]
    fn with(&self, events: &[Value]) -> World {
        let mut s = self.settings.clone();
        for e in events { if e["$"] == "SettingSet" { s.insert(e["key"].as_str().unwrap_or("").to_string(), e["value"].as_str().unwrap_or("").to_string()); } }
        World { now: self.now, thread: self.thread.clone(), provider: self.provider.clone(), model: self.model.clone(), settings: s, caps: self.caps.clone() }
    }
}

/// The remembered limit of a thread (a limit seen before the turn ended): `fallback.pending.<th>` or `account.failover.pending.<th>`.
fn pending_of(w: &World) -> Option<fb::Limit> {
    let o: Value = serde_json::from_str(&w.get(&format!("account.failover.pending.{}", w.thread))).ok().filter(|v: &Value| v["scope"].as_str().map_or(false, |s| !s.is_empty()))
        .or_else(|| serde_json::from_str(&w.get(&format!("fallback.pending.{}", w.thread))).ok().filter(|v: &Value| v["scope"].as_str().map_or(false, |s| !s.is_empty())))?;
    Some(fb::Limit { scope: o["scope"].as_str().unwrap_or("").into(), reason: o["reason"].as_str().unwrap_or("usage limit reached").into(), reset: o["reset"].as_u64().unwrap_or(0), auth: o["auth"] == true })
}

/// A limit seen on a line. `terminal`: the turn failed on it. The events to write now and, for a terminal one in force, the restart to make.
pub fn on_limit(w: &World, lim: &fb::Limit, terminal: bool, original: &str) -> Verdict {
    let (th, at) = (w.thread.as_str(), w.now);
    if lim.auth && !w.managed() { return Verdict::default() } // with no other account to go to, a failed sign-in stays the failed turn it is, and says why
    if !terminal {
        let key = if w.managed() { format!("account.failover.pending.{th}") } else { format!("fallback.pending.{th}") };
        let mut rec: Value = serde_json::from_str(&lim.json(at)).unwrap_or(Value::Null);
        if w.managed() { rec["provider"] = json!(w.provider); rec["binding"] = json!(accounts::binding_show(&w.binding().0, w.binding().1)); }
        return Verdict { events: vec![set(&key, &rec.to_string(), at)], ..Default::default() };
    }
    if !due(lim, at) { return Verdict::default() }
    let mut ev: Vec<Value> = vec![];
    if w.managed() {
        // AccountFallback.limited: the hold on this login (or this model on it), then the same request again; the restart picks what is still eligible
        let (id, epoch) = w.binding();
        let mut lim = lim.clone();
        if lim.reset == 0 && !lim.auth { lim.reset = fb::window_reset(&accounts::profile(&w.get("accounts.inventory"), &id), &w.model, at) } // the limit's own words had none: the window that filled has
        ev.push(fb::hold_mark(&w.provider, &id, epoch, &lim, at, at));
        let mut tried = w.tried(); let me = fb::tried_id(&id, epoch); if !tried.contains(&me) { tried.push(me) }
        ev.push(set(&format!("account.failover.pending.{th}"), "", at)); ev.push(set(&tried_key(th), &json!(tried).to_string(), at));
    } else if w.chain_on() {
        // Fallback.limited: the outage is the provider's (or the model's) until its reset
        let held = |s: &str| fb::hold_active(&serde_json::from_str(&w.get(&fb::Limit::key(s))).unwrap_or(Value::Null), at);
        if !held(&lim.scope) { ev.push(set(&fb::Limit::key(&lim.scope), &lim.json(at), at)) }
        ev.push(set(&format!("fallback.pending.{th}"), "", at));
    } else {
        // ProviderResume.hold: no fallback configured; the thread waits for the reported reset (or for the person when none came)
        ev.push(set(&format!("provider.resume.{th}"), &json!({"target":format!("{}/{}", w.provider, w.model),"reset":if lim.reset > at { lim.reset } else { 0 },"reason":lim.reason.chars().take(400).collect::<String>()}).to_string(), at));
        ev.push(set(&format!("fallback.pending.{th}"), "", at));
        let known = lim.reset > at;
        ev.push(if known { turn(th, "interrupted", at) } else { json!({"$":"TurnChanged","thread":th,"state":"interrupted","at":at}) }); // no reset to wait for: this turn has ended, as far as anyone waiting on it can tell
        ev.push(act_row(th, "warn", "provider-wait", &if known { format!("Provider limit reached; continuing at {} when it resets.", rules::when(&w.zone(), lim.reset, at)) } else { "Provider limit reached and no reset time is known; waiting for you.".to_string() }, at));
        return Verdict { events: ev, drop_turn_end: true, act: None };
    }
    ev.push(turn(th, "interrupted", at));
    Verdict { events: ev, drop_turn_end: true, act: Some(Act::Restart { text: continue_text(original) }) }
}

/// Where a stopped request goes on.
#[derive(Debug, PartialEq)]
pub enum Next {
    /// Nothing moves: the thread's own account or provider is free.
    Stay,
    /// Another account of the provider, with the line the timeline shows.
    Account { id: String, epoch: u64, say: String },
    /// The next target of the opt-in chain, on the account it should use there (None: not a provider with accounts, or none named).
    Chain { to: fb::Target, account: Option<(String, u64)>, say: String },
    /// Nothing can run: the events that say so and hold the thread until the reset.
    Wait { events: Vec<Value>, reset: u64 },
}
/// The timeline's line for an automatic switch. "Switched to Personal: Work ran out (resets 15:40)".
pub fn switched_line(to: &str, from: &str, reset: u64, auth: bool, zone: &str, now: u64) -> String {
    if auth { format!("Switched to {to}: {from} could not sign in") }
    else if reset > now { format!("Switched to {to}: {from} ran out (resets {})", rules::when(zone, reset, now)) }
    else { format!("Switched to {to}: {from} ran out (reset unknown)") }
}
/// What holds the login: when it ends, and whether it failed to sign in.
fn hold_of(w: &World, id: &str, epoch: u64) -> (u64, bool) {
    let get = |k: &str| w.get(k);
    let auth = serde_json::from_str::<Value>(&w.get(&fb::hold_key(&w.provider, id, epoch))).map_or(false, |o| o["auth"] == true);
    (fb::hold_until(&get, &w.provider, id, epoch, &w.model), auth)
}
/// The thread waits: every account of the provider is out. Said plainly, and with the time when one is known.
fn wait_all(w: &World, profiles: &[Value], from: &str, cur_id: &str, cur_epoch: u64) -> Next {
    let (th, at) = (w.thread.as_str(), w.now); let get = |k: &str| w.get(k);
    // a named account whose sign-in is dead: the thread waits for the person to sign it in again, and goes on by itself once they have (`signed_in`)
    let signin = !cur_id.is_empty() && hold_of(w, cur_id, cur_epoch).1;
    let until = fb::next_reset(profiles, &get, at, &w.provider, &w.model, cur_id, cur_epoch);
    let until = if until > at { until } else { 0 };
    let mut names: Vec<String> = profiles.iter().filter(|p| p["provider"] == w.provider.as_str() && p["state"] == "connected").filter_map(|p| p["label"].as_str().map(str::to_string)).collect();
    // the CLI's own login counts when the thread is on it or this request already ran out on it
    if cur_id.is_empty() || w.tried().contains(&fb::tried_id("", 0)) { names.insert(0, "Default account".into()) }
    let list = if names.is_empty() { String::new() } else { format!(" ({})", names.join(", ")) };
    let say = if signin && until == 0 { format!("{}'s sign-in expired and no other {} account can take over. Continuing once you sign in again.", w.name(cur_id), w.provider) }
        else if until > 0 { format!("Every {} account is out{list}. Continuing at {}, when the first one resets.", w.provider, rules::when(&w.zone(), until, at)) } else { format!("Every {} account is out{list} and no reset time is known. Waiting for you.", w.provider) };
    let mut rec = json!({"target":from,"reset":until,"reason":if signin { "sign-in expired" } else { "account usage limit" }});
    if signin { rec["signin"] = json!(cur_id) }
    let mut events = vec![set(&format!("provider.resume.{th}"), &rec.to_string(), at)];
    if until == 0 { events.push(json!({"$":"TurnChanged","thread":th,"state":"interrupted","at":at})) } // nothing will start it again: its task and launcher hear it stopped
    events.push(act_row(th, "warn", "provider-wait", &say, at));
    Next::Wait { events, reset: until }
}
/// What a restart does first: where the request goes on. Never a login this request ran out on, never a target before this one in the chain.
pub fn next(w: &World) -> Next {
    let (th, at) = (w.thread.as_str(), w.now);
    let get = |k: &str| w.get(k);
    let from = fb::Target { provider: w.provider.clone(), model: w.model.clone() };
    let (profiles, tried) = (w.inventory(), w.tried());
    let (cur_id, cur_epoch) = w.binding();
    if w.managed() {
        if let Some(p) = fb::pick_untried(&profiles, &tried, &get, at, &w.provider, &w.model) {
            let (id, epoch) = (p["id"].as_str().unwrap_or("").to_string(), p["epoch"].as_u64().unwrap_or(0));
            if id == cur_id && epoch == cur_epoch { return Next::Stay }
            let (reset, auth) = hold_of(w, &cur_id, cur_epoch);
            let say = switched_line(&w.name(&id), &w.name(&cur_id), reset, auth, &w.zone(), at);
            return Next::Account { id, epoch, say };
        }
        // the thread's own login, when this request did not run out on it: the wait below is over before it began
        if !tried.contains(&fb::tried_id(&cur_id, cur_epoch)) && !fb::hold_current(&get, &w.provider, &cur_id, cur_epoch, &w.model, at) { return Next::Stay }
        if !w.chain_on() { return wait_all(w, &profiles, &from.key(), &cur_id, cur_epoch) }
    } else if !(w.chain_on() && fb::target_held(&get, &from, at)) {
        return Next::Stay;
    }
    // the order, from its top: the first other provider (or model) that is offered, not held, and (a provider with accounts) has one with allowance left. A provider out earlier
    // that has reset since is taken again: the order is the person's preference, not a path walked once.
    if let Some((to, account)) = w.best(false) {
        let (reset, auth) = if w.managed() { hold_of(w, &cur_id, cur_epoch) } else { (provider_hold(w, &from), false) };
        let here = fb::provider_name(&w.provider); let from_name = if w.managed() { format!("{here} ({})", w.name(&cur_id)) } else { here };
        let say = switched_line(&w.target_name(&to, &account), &from_name, reset, auth, &w.zone(), at);
        return Next::Chain { to, account, say };
    }
    wait_everything(w, &profiles, &from.key(), &cur_id, cur_epoch)
}
/// When the hold on a provider with no accounts (or on its model) ends: 0 unknown.
fn provider_hold(w: &World, t: &fb::Target) -> u64 {
    let rec = |scope: &str| serde_json::from_str::<Value>(&w.get(&fb::Limit::key(scope))).unwrap_or(Value::Null);
    [rec(&t.provider), rec(&t.key())].iter().filter(|o| fb::hold_active(o, w.now)).map(fb::hold_end).filter(|e| *e > 0).min().unwrap_or(0)
}
/// Every target of the order is out too: one line with every provider's accounts and the earliest reset among all of them, and the thread goes on by itself then (`resume_on`
/// takes the best that is free at that time, which may be a provider above this one). With no other provider in the order this is the provider's own wait (`wait_all`).
fn wait_everything(w: &World, profiles: &[Value], from: &str, cur_id: &str, cur_epoch: u64) -> Next {
    let (th, at) = (w.thread.as_str(), w.now); let get = |k: &str| w.get(k);
    let others: Vec<fb::Target> = w.chain().into_iter().filter(|t| t.provider != w.provider && w.offered(t)).collect();
    if others.is_empty() && w.managed() { return wait_all(w, profiles, from, cur_id, cur_epoch) }
    let earliest = |a: u64, b: u64| if a == 0 { b } else if b == 0 { a } else { a.min(b) };
    let names_of = |p: &str| -> Vec<String> { profiles.iter().filter(|x| x["provider"] == p && x["state"] == "connected").filter_map(|x| x["label"].as_str().map(str::to_string)).collect() };
    let mut until = if w.managed() { fb::next_reset(profiles, &get, at, &w.provider, &w.model, cur_id, cur_epoch) } else { provider_hold(w, &fb::Target { provider: w.provider.clone(), model: w.model.clone() }) };
    let mut parts = vec![]; let mut seen: Vec<String> = vec![];
    let mut own = names_of(&w.provider); if w.managed() && (cur_id.is_empty() || w.tried().contains(&fb::tried_id("", 0))) { own.insert(0, "Default account".into()) }
    parts.push(if own.is_empty() { fb::provider_name(&w.provider) } else { format!("{}: {}", fb::provider_name(&w.provider), own.join(", ")) }); seen.push(w.provider.clone());
    for t in &others {
        let r = if profiles.iter().any(|p| p["provider"] == t.provider.as_str()) { fb::next_reset(profiles, &get, at, &t.provider, &t.model, "", 0) } else { provider_hold(w, t) };
        until = earliest(until, if r > at { r } else { 0 });
        if !seen.contains(&t.provider) { seen.push(t.provider.clone()); let n = names_of(&t.provider); parts.push(if n.is_empty() { fb::provider_name(&t.provider) } else { format!("{}: {}", fb::provider_name(&t.provider), n.join(", ")) }) }
    }
    let until = if until > at { until } else { 0 };
    // a dead sign-in on this account and nothing with a reset to wait for: the sign-in is what it waits for
    if until == 0 && !cur_id.is_empty() && hold_of(w, cur_id, cur_epoch).1 { return wait_all(w, profiles, from, cur_id, cur_epoch) }
    let list = parts.join("; ");
    let mut events = vec![set(&format!("provider.resume.{th}"), &json!({"target":from,"reset":until,"reason":"every account in the failover order is out"}).to_string(), at)];
    if until > 0 {
        events.push(act_row(th, "warn", "provider-wait", &format!("Every account is out ({list}). Continuing at {}, when the first one resets.", rules::when(&w.zone(), until, at)), at));
    } else {
        events.push(json!({"$":"TurnChanged","thread":th,"state":"interrupted","at":at})); // nothing will start it again: its task and launcher hear it stopped
        events.push(act_row(th, "error", "fallback-blocked", &format!("Every account is out ({list}) and no reset time is known. Waiting for you."), at));
    }
    Next::Wait { events, reset: until }
}

// ---- the hub side ----

/// What a decision reads: every global key of the fallback, and of the `account.` keys only this thread's and the holds (a log has one binding per thread, and each read goes to the store).
async fn world_of(hub: &Arc<Hub>, thread: &str, provider: String, model: String) -> World {
    let keys: Vec<String> = { let m = hub.model().await;
        let mut v: Vec<String> = ["fallback.", "accounts.inventory", "tz.machine"].iter().flat_map(|p| m.settings_with_prefix(p).map(|(k, _)| k.to_string()).collect::<Vec<_>>()).filter(|k| !k.starts_with("fallback.input.")).collect();
        v.extend(m.settings_with_prefix("account.limit.").map(|(k, _)| k.to_string()));
        v.extend([accounts::binding_key(thread, &provider), format!("account.failover.pending.{thread}"), tried_key(thread), fb::preference_key(thread), format!("effort.{thread}")]); v };
    let mut settings = HashMap::new();
    for k in keys { if let Some(v) = hub.setting(&k).await { settings.insert(k, v); } }
    let mut caps = serde_json::from_str::<Value>(&models::info()).unwrap_or(Value::Null);
    if let Some(o) = caps.as_object_mut() { o.retain(|_, v| v.is_array()); }
    World { now: now_ms() / 1000, thread: thread.into(), provider, model, settings, caps }
}

/// The original request of the interrupted turn: what it was started with.
async fn original(hub: &Arc<Hub>, thread: &str) -> String { hub.setting(&format!("fallback.input.{thread}")).await.unwrap_or_default() }

/// A limit remembered earlier knows its reset when the failing line did not say one, and an account-wide one stays that: the protocol's event says so where the words may not.
fn merged(line: Option<fb::Limit>, seen: Option<fb::Limit>) -> Option<fb::Limit> {
    match (line, seen) { (Some(mut l), Some(s)) => { if l.reset == 0 { l.reset = s.reset } if !s.scope.contains('/') && !l.auth { l.scope = s.scope } Some(l) } (l, s) => l.or(s) }
}
/// Called by the agents' actor for every line of a running turn, right after the line's own events are built. `end`: the state this line ended the turn in. `seen`: the limit the
/// actor remembers for this turn (the settings written for an earlier line of the same read are not committed yet). Empty unless the line is a provider's for a running thread, and the thread's own (not a subagent's).
/// A dead login on a named account: the account says so (accounts_io `expire`), so no status check calls it connected while it cannot run a turn, and the person is asked to sign
/// it in again. The CLI's own login is not an account here.
fn expire_login(acc: Option<&Arc<crate::settings::accounts_io::Accounts>>, w: &World, lim: &fb::Limit) {
    if !lim.auth { return }
    let (id, epoch) = w.binding();
    if let (false, Some(m)) = (id.is_empty(), acc) { m.expire(&id, epoch); }
}
pub async fn on_line(hub: &Arc<Hub>, acc: Option<&Arc<crate::settings::accounts_io::Accounts>>, thread: &str, v: &Value, end: Option<&str>, seen: &mut Option<fb::Limit>) -> Verdict {
    if !v["parent_tool_use_id"].as_str().unwrap_or("").is_empty() { return Verdict::default() }
    // a cheap look first: only an error envelope, a rejected-limit event or an assistant line can be a limit; any line that ends the turn decides one remembered
    let t = v["type"].as_str().unwrap_or("");
    let shaped = matches!(t, "turn.failed" | "error" | "result" | "rate_limit_event" | "assistant");
    if !shaped && end.is_none() { return Verdict::default() }
    let (provider, model, running) = { let m = hub.model().await; match m.threads.get(thread) { Some(t) => (t.provider.clone(), t.model.clone(), t.turn == "running"), None => return Verdict::default() } };
    if !running { return Verdict::default() }
    let lim = if shaped { fb::classify(&provider, &model, v).or_else(|| fb::auth_failure(&provider, v)) } else { None };
    // Claude Code cancels its own command after a limit error (Nov 10: "You've hit your weekly limit", then the command's lifecycle `cancelled`), and the turn ends Interrupted on
    // that line. The person's Stop never ends a turn on a line (cmds.rs: their Stop makes the command a tomb), so a turn the CLI stopped with a limit seen in it ran out.
    let ran_out = end == Some("interrupted") && (lim.is_some() || seen.is_some() || {
        let mut any = false; for k in [format!("account.failover.pending.{thread}"), format!("fallback.pending.{thread}")] { any |= !hub.setting(&k).await.unwrap_or_default().is_empty() } any });
    if end.is_some() && end != Some("failed") && !ran_out { // the turn went well or the person stopped it: what was remembered is over
        // `seen` too: a record written for a line of this same read is not committed yet, so the store does not show it, and left behind it would fail a later turn over
        let had = seen.take().is_some(); let mut ev = vec![];
        for k in [format!("account.failover.pending.{thread}"), format!("fallback.pending.{thread}")] { if had || !hub.setting(&k).await.unwrap_or_default().is_empty() { ev.push(set(&k, "", now_ms() / 1000)) } }
        // a turn that completed on its login is the proof the login is not out: its holds are over (they had outlived the limit by a day on Nov 7)
        if end == Some("completed") && !had { ev.extend(proven_events(hub, thread, &provider, &model).await) }
        return Verdict { events: ev, ..Default::default() };
    }
    if end.is_none() {
        let Some(lim) = lim else { return Verdict::default() };
        *seen = merged(Some(lim.clone()), seen.take());
        let w = world_of(hub, thread, provider, model).await;
        expire_login(acc, &w, &lim);
        return on_limit(&w, seen.as_ref().unwrap_or(&lim), false, "");
    }
    let lim = merged(lim, seen.take());
    let w = world_of(hub, thread, provider, model).await;
    let Some(lim) = lim.or_else(|| pending_of(&w).filter(|p| due(p, w.now))) else { return Verdict::default() };
    expire_login(acc, &w, &lim);
    let orig = original(hub, thread).await;
    on_limit(&w, &lim, true, &orig)
}
/// The events that end the limit holds on a thread's login (and model) after a turn completed on it (fallback.rs `proven`).
async fn proven_events(hub: &Arc<Hub>, thread: &str, provider: &str, model: &str) -> Vec<Value> {
    let b: Value = serde_json::from_str(&hub.setting(&accounts::binding_key(thread, provider)).await.unwrap_or_default()).unwrap_or(Value::Null);
    let (id, epoch) = (b["id"].as_str().unwrap_or("").to_string(), b["epoch"].as_u64().unwrap_or(0));
    let keys: Vec<String> = [fb::hold_key(provider, &id, epoch), fb::hold_model_key(provider, &id, epoch, model)].into_iter().collect();
    let mut held = HashMap::new();
    for k in &keys { if let Some(v) = hub.setting(k).await { if !v.is_empty() { held.insert(k.clone(), v); } } }
    if held.is_empty() { return vec![] }
    let names: Vec<String> = held.keys().cloned().collect(); let get = |k: &str| held.get(k).cloned().unwrap_or_default();
    let now = now_ms() / 1000;
    fb::proven(&names, &get, provider, &id, epoch, model).into_iter().map(|k| set(&k, "", now)).collect()
}
/// A running turn's process ended with no line ending the turn: a limit remembered, or one it left on stderr, is acted on as the line would have been.
pub async fn on_exit(hub: &Arc<Hub>, acc: Option<&Arc<crate::settings::accounts_io::Accounts>>, thread: &str, stderr: &str, seen: &mut Option<fb::Limit>) -> Verdict {
    let (provider, model) = { let m = hub.model().await; match m.threads.get(thread) { Some(t) if t.turn == "running" => (t.provider.clone(), t.model.clone()), _ => return Verdict::default() } };
    let Some(lim) = merged(fb::from_stderr(&provider, &model, stderr), seen.take()) else { return Verdict::default() };
    let w = world_of(hub, thread, provider, model).await; let orig = original(hub, thread).await;
    expire_login(acc, &w, &lim);
    on_limit(&w, &lim, true, &orig)
}

fn inner() -> Caller { Caller { gen: "driver".into(), client: 0 } }
/// A hub-side request (`accounts.choose`, `fallback.switch`): the error it answered with, if any.
async fn ask(hub: &Arc<Hub>, method: &str, p: Value) -> Result<(), String> {
    let Some(h) = hub.rpc_for(method) else { return Err(format!("{method} is not available")) };
    let r = h.call(hub, &inner(), &json!(0), method, &p).await.into_iter().next().unwrap_or(Value::Null);
    if r["ok"] == true { Ok(()) } else { Err(r["error"].as_str().unwrap_or("refused").to_string()) }
}
/// Moves the thread as `next` said. Err: the thread could not be moved; it stays where it is.
async fn apply(hub: &Arc<Hub>, w: &World, step: Next) -> Result<bool, String> {
    let th = w.thread.as_str();
    match step {
        Next::Stay => Ok(true),
        Next::Wait { events, .. } => { let _ = hub.commit_many(&events).await; Ok(false) }
        Next::Account { id, epoch, say } => {
            let (cur_id, cur_epoch) = w.binding();
            let pref = fb::preference_save(&w.get(&fb::preference_key(th)), &w.provider, &w.model, &w.get(&format!("effort.{th}")), &accounts::binding_show(&cur_id, cur_epoch), &accounts::binding_show(&id, epoch));
            let _ = hub.commit_many(&[set(&fb::preference_key(th), &pref, w.now)]).await;
            ask(hub, "accounts.choose", json!({"thread":th,"profile":id,"epoch":epoch,"by":"failover","say":say})).await.map(|_| true)
        }
        Next::Chain { to, account, say } => {
            let intent = { let s = w.get(&format!("fallback.intent.{th}")); if s.is_empty() { format!("{}/{}", w.provider, w.model) } else { s } };
            ask(hub, "fallback.switch", json!({"thread":th,"provider":to.provider,"model":to.model,"say":say})).await?;
            let _ = hub.commit_many(&[set(&format!("fallback.intent.{th}"), &intent, w.now)]).await;
            if let Some((id, epoch)) = account { ask(hub, "accounts.choose", json!({"thread":th,"profile":id,"epoch":epoch,"by":"failover"})).await? }
            Ok(true)
        }
    }
}

/// After the batch with the Interrupted turn is committed: choose what is still eligible, write the switch, and start the same request again.
pub fn spawn_act(ag: Arc<Agents>, thread: String, act: Act) {
    tokio::spawn(async move {
        let Act::Restart { text } = act;
        let hub = ag.hub.clone();
        let _ = ag.stop(&thread).await;
        // the thread is no longer running here: read it as it is, with the hold just written
        let (provider, model) = { let m = hub.model().await; match m.threads.get(&thread) { Some(t) => (t.provider.clone(), t.model.clone()), None => return } };
        let w = world_of(&hub, &thread, provider, model).await;
        match apply(&hub, &w, next(&w)).await {
            // the queue stays: what the person queued is not this request's; what a switch handed over goes with this start
            Ok(true) => { if let Err(e) = ag.resume_with(&thread, &text).await { eprintln!("driver: {thread}: could not carry on: {e}") } }
            Ok(false) => {}
            Err(e) => { let at = now_ms() / 1000; let _ = hub.commit_many(&[json!({"$":"TurnChanged","thread":thread,"state":"interrupted","at":at}), act_row(&thread, "error", "fallback-blocked", &format!("Could not switch accounts: {e}"), at)]).await; }
        }
    });
}

/// The reset a waiting thread was waiting for has come: it goes on where it can, which is not always where it was (the earliest reset may be another account's). Nothing free: it waits on, said once.
pub(crate) async fn resume_on(ag: &Arc<Agents>, th: &str, msg: &str, ev: Vec<Value>, now_s: u64) {
    let hub = ag.hub.clone();
    let (provider, model) = { let m = hub.model().await; match m.threads.get(th) { Some(t) => (t.provider.clone(), t.model.clone()), None => return } };
    let mut w = world_of(&hub, th, provider, model).await;
    w.now = now_s; w.settings.remove(&tried_key(th)); // a window reset: every account may be asked again
    match apply(&hub, &w, next(&w)).await {
        Ok(true) => { let _ = ag.deliver(th, msg, RESUME_TEXT, None, ev).await; }
        Ok(false) => {}
        Err(e) => eprintln!("driver: {th}: the resume could not switch: {e}"),
    }
}
pub const SIGNIN_TEXT: &str = "[Backplane] This account was signed in again after its sign-in expired, so the turn starts again. Continue the interrupted work from its current state. Check completed tool results and any work still running before repeating an action.";
/// SignIn.restart: which waiting threads a sign-in ends. A record names the account whose expired sign-in stopped the thread (`wait_all`); the thread is still as it was left
/// (alive, not held, dormant or ephemeral, its turn Interrupted with nothing queued, the record's target still its own).
pub fn signin_ready(f: &ResumeFacts, rec: &Value, profile: &str) -> bool {
    !profile.is_empty() && rec["signin"].as_str() == Some(profile) && f.exists && f.live && !(f.held || f.dormant || f.ephemeral) && f.interrupted && f.pending == 0 && rec["target"].as_str().unwrap_or("") == f.target
}
/// An account whose sign-in had expired is connected again (accounts_hub `found`): every thread its expiry stopped goes on, on that account at its new epoch, with the
/// conversation handed over (a new sign-in is a new store: the old native session is not there). A person's pin on the account moves to the new epoch with it.
pub(crate) async fn signed_in(ag: &Arc<Agents>, profile: &str, now_s: u64) -> usize {
    let hub = ag.hub.clone();
    let keys: Vec<String> = { let m = hub.model().await; m.settings_with_prefix("provider.resume.").map(|(k, _)| k.to_string()).collect() };
    let inv = hub.setting("accounts.inventory").await.unwrap_or_default();
    let p = accounts::profile(&inv, profile);
    let (epoch, provider) = (p["epoch"].as_u64().unwrap_or(0), p["provider"].as_str().unwrap_or("").to_string());
    if p["state"] != "connected" { return 0 }
    let mut n = 0;
    for k in keys {
        let th = k.trim_start_matches("provider.resume.").to_string();
        let rec: Value = serde_json::from_str(&hub.setting(&k).await.unwrap_or_default()).unwrap_or(Value::Null);
        if !signin_ready(&resume_facts(&hub, &th).await, &rec, profile) { continue }
        let chosen = hub.setting(&accounts::chosen_key(&th, &provider)).await.unwrap_or_default();
        let c: Value = serde_json::from_str(&chosen).unwrap_or(Value::Null);
        if c["by"] == "person" && c["id"] == profile { let _ = hub.commit_many(&[set(&accounts::chosen_key(&th, &provider), &accounts::chosen_show(profile, epoch, "person", c["who"].as_str().unwrap_or(""), now_s), now_s)]).await; }
        let name = accounts::label(profile, &p);
        if let Err(e) = ask(&hub, "accounts.choose", json!({"thread":th,"profile":profile,"epoch":epoch,"by":"failover","say":format!("Back on {name}: signed in again")})).await { eprintln!("driver: {th}: could not go back to its account after the sign-in: {e}"); continue }
        let seq = hub.revision().await;
        let ev = vec![set(&k, "", now_s), set(&tried_key(&th), "", now_s), json!({"$":"ActivityLogged","thread":th,"act":crate::agents::lines::hub_id("signin", now_s, seq),"tone":"info","kind":"provider-resume","summary":format!("{name} is signed in again; continuing from the last step"),"at":now_s})];
        let orig = original(&hub, &th).await;
        let text = if orig.trim().is_empty() || orig.starts_with("[Backplane]") { SIGNIN_TEXT.to_string() } else { format!("{SIGNIN_TEXT}\n\nOriginal request (context, not a new task):\n{orig}") };
        if ag.deliver(&th, &crate::agents::lines::hub_id("signin", now_s, seq), &text, None, ev).await.is_ok() { n += 1 }
    }
    n
}
/// Holds with no reset that a newer fact has ended (fallback.rs `clearable`).
pub(crate) async fn sweep_holds(hub: &Arc<Hub>, now_s: u64) {
    let keys: Vec<String> = { let m = hub.model().await; m.settings_with_prefix("account.limit.").map(|(k, _)| k.to_string()).collect() };
    if keys.is_empty() { return }
    let mut held = HashMap::new();
    for k in &keys { if let Some(v) = hub.setting(k).await { if !v.is_empty() { held.insert(k.clone(), v); } } }
    if held.is_empty() { return }
    let inv: Value = serde_json::from_str(&hub.setting("accounts.inventory").await.unwrap_or_default()).unwrap_or(Value::Null);
    let get = |k: &str| held.get(k).cloned().unwrap_or_default(); let names: Vec<String> = held.keys().cloned().collect();
    // the CLI's own login too: its reading is the default allowance (on Nov 7 its weekly-limit hold, with no reset, outlived the limit by a day)
    let default = fb::default_profile("claude", &hub.setting("allowance.claude").await.unwrap_or_default());
    let ev: Vec<Value> = inv["profiles"].as_array().into_iter().flatten().chain(std::iter::once(&default)).flat_map(|p| fb::clearable(p, &names, &get, now_s)).map(|k| set(&k, "", now_s)).collect();
    if !ev.is_empty() { let _ = hub.commit_many(&ev).await; }
}
/// The account a thread left has reset: the thread goes back to it, between turns. A thread the order moved to another provider (`fallback.intent`) goes back up the order
/// first: to the highest provider above the one it is on that has an account free again, and stays marked until it is back where it started.
pub(crate) async fn returns(hub: &Arc<Hub>, now_s: u64) {
    let intents: Vec<String> = { let m = hub.model().await; m.settings_with_prefix("fallback.intent.").map(|(k, _)| k.to_string()).collect() };
    for k in intents {
        let intent = hub.setting(&k).await.unwrap_or_default(); if intent.is_empty() { continue }
        let th = k.trim_start_matches("fallback.intent.").to_string();
        let (provider, model, idle) = { let m = hub.model().await; match m.threads.get(&th) { Some(t) if !t.deleted && !t.archived => (t.provider.clone(), t.model.clone(), t.turn != "running"), _ => { drop(m); let _ = hub.commit_many(&[set(&k, "", now_s)]).await; continue } } };
        if !idle || !hub.setting(&format!("provider.resume.{th}")).await.unwrap_or_default().is_empty() { continue }
        let mut w = world_of(hub, &th, provider, model).await; w.now = now_s;
        if !w.chain_on() { continue }
        let Some((to, account)) = w.best(true) else { continue };
        let say = format!("Back on {}: it has reset", w.target_name(&to, &account));
        if let Err(e) = ask(hub, "fallback.switch", json!({"thread":th,"provider":to.provider,"model":to.model,"say":say})).await { eprintln!("driver: {th}: could not go back up the order: {e}"); continue }
        if let Some((id, epoch)) = account { if let Err(e) = ask(hub, "accounts.choose", json!({"thread":th,"profile":id,"epoch":epoch,"by":"return"})).await { eprintln!("driver: {th}: could not choose the account it went back to: {e}") } }
        // back where it started: nothing to return to any more (the switch cleared the mark); somewhere in between: still marked, it goes on up as more resets come
        if fb::Target::parse(&intent).provider != to.provider { let _ = hub.commit_many(&[set(&k, &intent, now_s)]).await; }
    }
    let keys: Vec<String> = { let m = hub.model().await; m.settings_with_prefix("account.failover.preference.").map(|(k, _)| k.to_string()).collect() };
    for k in keys {
        let raw = hub.setting(&k).await.unwrap_or_default(); if raw.is_empty() { continue }
        let th = k.trim_start_matches("account.failover.preference.").to_string();
        let (provider, model, idle) = { let m = hub.model().await; match m.threads.get(&th) { Some(t) if !t.deleted && !t.archived => (t.provider.clone(), t.model.clone(), t.turn != "running"), _ => { drop(m); let _ = hub.commit_many(&[set(&k, "", now_s)]).await; continue } } };
        if !idle || !hub.setting(&format!("provider.resume.{th}")).await.unwrap_or_default().is_empty() { continue }
        let w = world_of(hub, &th, provider, model).await; let get = |k: &str| w.get(k);
        let (cur_id, cur_epoch) = w.binding();
        let Some((id, epoch)) = fb::preference_ready(&raw, &w.provider, &accounts::binding_show(&cur_id, cur_epoch), &w.inventory(), &get, now_s) else { continue };
        let say = format!("Back on {}: it has reset", w.name(&id));
        if let Err(e) = ask(hub, "accounts.choose", json!({"thread":th,"profile":id,"epoch":epoch,"by":"return","say":say})).await { eprintln!("driver: {th}: could not go back to its account: {e}") }
    }
}

/// What ProviderResume.ready reads of one thread.
#[derive(Debug, Clone, Default)]
pub struct ResumeFacts { pub exists: bool, pub live: bool, pub held: bool, pub dormant: bool, pub ephemeral: bool, pub interrupted: bool, pub pending: u64, pub target: String }
/// ProviderResume.ready: the thread is alive and not held, dormant or ephemeral, its turn is exactly Interrupted with nothing queued, the record's reset has come, and the
/// record's target is still the thread's current provider/model (a person who changed it is not resumed on the old one).
pub fn resume_ready(f: &ResumeFacts, rec: &Value, now: u64) -> bool {
    let reset = rec["reset"].as_u64().unwrap_or(0);
    f.exists && f.live && !(f.held || f.dormant || f.ephemeral) && f.interrupted && reset > 0 && now >= reset && f.pending == 0 && rec["target"].as_str().unwrap_or("") == f.target
}
pub const RESUME_TEXT: &str = "[Backplane] The provider's reported limit reset has arrived. Continue from your last step. Check completed tool results and any work still running before repeating an action.";
/// ProviderResume.one: the record is cleared and the note logged in the same commit as the message that runs the thread again.
/// Ids as Hub.id(prefix, now, Hub.seq): the log's length at that step, so threads resumed in one tick (they usually share a reset) never share a message or act id.
pub fn resume_events(key: &str, th: &str, now: u64, seq: u64) -> Vec<Value> {
    vec![set(key, "", now), json!({"$":"ActivityLogged","thread":th,"act":crate::agents::lines::hub_id("quota", now, seq),"tone":"info","kind":"provider-resume","summary":"Provider limit reset; continuing from the last step","at":now})]
}
/// `ephemeral` is by name only: Ephemeral.is also counts a mail run or private run by record (MR.Run.is), which is not ported yet; whoever ports Run.is uses it here.
async fn resume_facts(hub: &Arc<Hub>, th: &str) -> ResumeFacts {
    let held = hub.setting(&format!("thread.config.hold.{th}")).await.as_deref() == Some("on");
    let t = { let m = hub.model().await; match m.threads.get(th) { Some(t) => t.clone(), None => return ResumeFacts::default() } };
    let dormant = hub.setting(&format!("project.dormant.{}", t.project)).await.as_deref() == Some("on");
    ResumeFacts { exists: true, live: !(t.archived || t.deleted), held, dormant, ephemeral: th.starts_with("run") || th.starts_with("mailcoord-"), interrupted: t.turn == "interrupted", pending: t.pending, target: format!("{}/{}", t.provider, t.model) }
}

/// ProviderResume: a thread that waited for a reported reset continues once it has passed. Unknown resets wait for the person.
/// NOT on a copy of a real log (BACKPLANE_COPY=1, and so not in side-by-side either): the records `provider.resume.*` are the real hub's, and delivering one starts a
/// real turn nobody asked for. The tick returns at once and clears nothing.
pub struct ResumeTicker;
/// The ready resumes: `(record key, thread)`. Reads only.
pub async fn ready_resumes(hub: &Arc<Hub>, now_s: u64) -> Vec<(String, String)> {
    let keys: Vec<String> = { let m = hub.model().await; m.settings_with_prefix("provider.resume.").map(|(k, _)| k.to_string()).collect() };
    let mut out = vec![];
    for k in keys {
        let th = k.trim_start_matches("provider.resume.").to_string();
        let rec: Value = serde_json::from_str(&hub.setting(&k).await.unwrap_or_default()).unwrap_or(Value::Null);
        if resume_ready(&resume_facts(hub, &th).await, &rec, now_s) { out.push((k, th)) }
    }
    out
}
/// One tick: `deliver(thread, message id, events)` runs each ready resume (production: `Agents::deliver`). A copy delivers nothing.
pub async fn run_resumes<F, Fut>(copy: bool, hub: &Arc<Hub>, now_s: u64, deliver: F) where F: Fn(String, String, Vec<Value>) -> Fut, Fut: std::future::Future<Output = ()> {
    if copy { return }
    for (k, th) in ready_resumes(hub, now_s).await {
        // the message and the record's clearing are one commit; a failed start leaves the record for the next tick
        // the log's length is read at each thread: the commit before it moved it
        let seq = hub.revision().await;
        deliver(th.clone(), crate::agents::lines::hub_id("resume", now_s, seq), resume_events(&k, &th, now_s, seq)).await;
    }
}
impl Ticker for ResumeTicker {
    fn every_s(&self) -> u64 { 30 }
    fn tick<'a>(&'a self, hub: &'a Arc<Hub>, now_s: u64) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if crate::copy::on() { return vec![] }
            let Some(ag) = crate::agents::handle() else { return vec![] };
            run_resumes(false, hub, now_s, |th, msg, ev| { let ag = ag.clone(); async move { resume_on(&ag, &th, &msg, ev, now_s).await } }).await;
            sweep_holds(hub, now_s).await; returns(hub, now_s).await;
            vec![]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The order off unless the test names it: these are one provider's rules. `ordered` is the hub as it starts, with the default order on.
    fn world(provider: &str, model: &str, settings: &[(&str, String)]) -> World {
        let mut w = ordered(provider, model, settings); w.settings.entry("fallback.on".into()).or_insert_with(|| "off".into()); w
    }
    fn ordered(provider: &str, model: &str, settings: &[(&str, String)]) -> World {
        World { now: 1000, thread: "t1".into(), provider: provider.into(), model: model.into(), settings: settings.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(), caps: json!({"claude":["opus","sonnet"],"codex":["gpt-6"],"grok":["g"]}) }
    }
    fn lim(scope: &str, reset: u64) -> fb::Limit { fb::Limit { scope: scope.into(), reason: "You've hit your usage limit".into(), reset, auth: false } }
    fn inv(profiles: Value) -> (&'static str, String) { ("accounts.inventory", json!({"version":1,"revision":1,"profiles":profiles,"error":""}).to_string()) }
    fn keys(v: &[Value]) -> Vec<String> { v.iter().map(|e| e["key"].as_str().or(e["$"].as_str()).unwrap_or("").to_string()).collect() }

    #[test] fn a_limit_before_the_turn_ends_is_only_remembered() {
        let w = world("claude", "opus", &[]); let v = on_limit(&w, &lim("claude", 0), false, "do it");
        assert_eq!((keys(&v.events), v.drop_turn_end, v.act), (vec!["fallback.pending.t1".to_string()], false, None));
        let (k, i) = inv(json!([{"id":"w","provider":"claude","state":"connected","epoch":1}])); let wm = world("claude", "opus", &[(k, i)]);
        assert_eq!(keys(&on_limit(&wm, &lim("claude", 0), false, "x").events), ["account.failover.pending.t1"], "a managed provider keeps it with its own record");
    }
    #[test] fn without_fallback_a_terminal_limit_holds_the_thread_until_the_reset() {
        let w = world("claude", "opus", &[]);
        let v = on_limit(&w, &lim("claude", 5000), true, "do it"); assert!(v.drop_turn_end && v.act.is_none());
        assert_eq!(keys(&v.events), ["provider.resume.t1", "fallback.pending.t1", "TurnChanged", "ActivityLogged"]);
        let rec: Value = serde_json::from_str(v.events[0]["value"].as_str().unwrap()).unwrap(); assert_eq!((rec["reset"].clone(), rec["target"].clone()), (json!(5000), json!("claude/opus")));
        assert!(on_limit(&w, &lim("claude", 500), true, "x").events.is_empty(), "a reset already passed is not a limit in force");
    }
    #[test] fn with_the_chain_on_a_terminal_limit_marks_the_outage_and_restarts_the_same_request() {
        let w = world("claude", "opus", &[("fallback.on", "on".into()), ("fallback.chain", "claude/opus,codex/gpt-6".into())]);
        let v = on_limit(&w, &lim("claude", 0), true, "build the board");
        assert_eq!(keys(&v.events), ["fallback.outage.claude", "fallback.pending.t1", "TurnChanged"]); assert!(v.drop_turn_end);
        assert_eq!((v.events[2]["state"].clone(), v.events[2]["failover"].clone()), (json!("interrupted"), json!(true)), "its task and launcher are not told it ended");
        match v.act.unwrap() { Act::Restart { text } => assert!(text.starts_with("[Backplane] The account this turn ran on stopped") && text.ends_with("build the board")) }
    }
    #[test] fn the_restart_moves_to_the_next_chain_target_and_never_wraps() {
        let hold = ("fallback.outage.claude", json!({"scope":"claude","reason":"r","reset":0,"at":1}).to_string());
        let w = world("claude", "opus", &[("fallback.on", "on".into()), ("fallback.chain", "claude/opus,codex/gpt-6".into()), hold.clone()]);
        match next(&w) { Next::Chain { to, account, .. } => assert_eq!((to.key(), account), ("codex/gpt-6".to_string(), None)), n => panic!("{n:?}") }
        // codex is held too: nothing is left, the work waits and says so; the chain does not wrap back to claude
        let both = world("codex", "gpt-6", &[("fallback.on", "on".into()), ("fallback.chain", "claude/opus,codex/gpt-6".into()), hold, ("fallback.outage.codex", json!({"scope":"codex","reset":0,"at":900}).to_string())]);
        match next(&both) { Next::Wait { events, reset } => {
            assert_eq!(reset, 1 + fb::UNKNOWN_HOLD_S, "a limit with no reset is tried again an hour after it was met: Claude's, met first, is the earliest");
            assert!(events.iter().any(|e| e["kind"] == "provider-wait" && e["summary"].as_str().unwrap().starts_with("Every account is out (Codex; Claude). Continuing at ")) && events.iter().any(|e| e["key"] == "provider.resume.t1"), "{events:?}") } n => panic!("{n:?}") }
    }
    #[test] fn a_chain_target_whose_accounts_are_all_held_is_passed_over() {
        let at = now_ms() / 1000; let (k, i) = inv(json!([{"id":"c","provider":"codex","state":"connected","epoch":1}]));
        let chain = [("fallback.on", "on".to_string()), ("fallback.chain", "claude/opus,codex/gpt-6,grok/g".to_string()), ("fallback.outage.claude", json!({"scope":"claude","reset":0,"at":at}).to_string()), (k, i)];
        let mut w = world("claude", "opus", &chain); w.now = at;
        match next(&w) { Next::Chain { to, account, .. } => assert_eq!((to.provider.as_str(), account), ("codex", Some(("c".to_string(), 1))), "it runs on the account it has there"), n => panic!("{n:?}") }
        let held = w.with(&[set(&fb::hold_key("codex", "c", 1), &json!({"scope":"codex","reset":at + 100}).to_string(), 1)]);
        match next(&held) { Next::Chain { to, account, .. } => assert_eq!((to.provider.as_str(), account), ("grok", None), "codex has nothing to run on"), n => panic!("{n:?}") }
    }
    fn mixed(at: u64) -> (&'static str, String) { inv(json!([
        {"id":"w","provider":"claude","state":"connected","epoch":2,"label":"Work","allowance":{"state":"ok","at":at,"provider":"claude","windows":[{"scope":"five_hour","used_percent":50.0}]}},
        {"id":"p","provider":"claude","state":"connected","epoch":1,"label":"Personal","allowance":{"state":"ok","at":at,"provider":"claude","windows":[{"scope":"five_hour","used_percent":10.0}]}},
        {"id":"c","provider":"codex","state":"connected","epoch":1,"label":"Codex Work","allowance":{"state":"ok","at":at,"provider":"codex","windows":[{"scope":"five_hour","used_percent":5.0}]}}])) }
    fn hold(p: &str, id: &str, epoch: u64, reset: u64) -> Value { set(&fb::hold_key(p, id, epoch), &json!({"scope":p,"reset":reset,"at":1}).to_string(), 1) }
    #[test] fn the_default_order_tries_every_account_of_the_provider_then_the_next_provider_then_waits_for_the_earliest_reset_anywhere() {
        let at = now_ms() / 1000; let (k, i) = mixed(at);
        let mut w = ordered("claude", "", &[(k, i), ("account.thread.t1.claude", accounts::binding_show("w", 2))]); w.now = at;
        assert!(w.chain_on() && fb::show(&w.chain()) == fb::DEFAULT_CHAIN, "on, in the default order, until the person sets one");
        // 1. the same provider first
        let w1 = w.with(&on_limit(&w, &lim("claude", at + 3000), true, "go").events);
        match next(&w1) { Next::Account { id, .. } => assert_eq!(id, "p"), n => panic!("{n:?}") }
        // 2. both Claude accounts out: Codex, on its account
        let w2 = w1.with(&[hold("claude", "p", 1, at + 2000), set(&tried_key("t1"), &json!([fb::tried_id("w", 2), fb::tried_id("p", 1)]).to_string(), 1)]);
        match next(&w2) { Next::Chain { to, account, say } => { assert_eq!((to.label(), account), ("codex".to_string(), Some(("c".to_string(), 1)))); assert!(say.starts_with("Switched to Codex (Codex Work): Claude (Work) ran out (resets "), "{say}") } n => panic!("{n:?}") }
        // 3. on Codex, its account out too (Antigravity and Grok not installed): one line, the earliest reset of any of them, and it goes on by itself then
        let mut w3 = ordered("codex", "", &[mixed(at), ("account.thread.t1.codex", accounts::binding_show("c", 1))]); w3.now = at; w3.caps = json!({"claude":["opus"],"codex":["gpt-6"]});
        let w3 = w3.with(&[hold("claude", "w", 2, at + 3000), hold("claude", "p", 1, at + 2000), hold("codex", "c", 1, at + 4000), set(&tried_key("t1"), &json!([fb::tried_id("c", 1)]).to_string(), 1)]);
        match next(&w3) { Next::Wait { events, reset } => {
            assert_eq!(reset, at + 2000, "Personal resets first");
            let line = events.iter().find(|e| e["kind"] == "provider-wait").unwrap()["summary"].as_str().unwrap().to_string();
            assert!(line.starts_with("Every account is out (Codex: Codex Work; Claude: Work, Personal). Continuing at "), "{line}");
            assert!(!events.iter().any(|e| e["$"] == "TurnChanged"), "it will start again: its turn has not ended");
        } n => panic!("{n:?}") }
        // the reset comes: the resume (tried cleared) takes the provider above, Claude, on the account that reset
        let mut w4 = w3.with(&[set(&tried_key("t1"), "", 1)]); w4.now = at + 2001; w4.settings.remove(&tried_key("t1"));
        match next(&w4) { Next::Chain { to, account, .. } => assert_eq!((to.provider.as_str(), account), ("claude", Some(("p".to_string(), 1)))), n => panic!("{n:?}") }
        // 4. between turns, a thread the order moved down goes back up once an account above has reset, and not before
        assert_eq!(w3.best(true), None, "Claude is still out");
        let mut later = w3.clone_at(at + 2001); later.settings.remove(&tried_key("t1"));
        assert_eq!(later.best(true).map(|(t, a)| (t.provider, a)), Some(("claude".to_string(), Some(("p".to_string(), 1)))));
        let mut top = ordered("claude", "", &[mixed(at)]); top.now = at; assert_eq!(top.best(true), None, "nothing is above the top");
    }
    #[test] fn the_order_turned_off_keeps_a_thread_on_its_provider() {
        let at = now_ms() / 1000; let (k, i) = mixed(at);
        let mut w = ordered("claude", "", &[(k, i), ("fallback.on", "off".into()), ("account.thread.t1.claude", accounts::binding_show("w", 2))]); w.now = at;
        let w = w.with(&[hold("claude", "w", 2, at + 3000), hold("claude", "p", 1, at + 2000), set(&tried_key("t1"), &json!([fb::tried_id("w", 2), fb::tried_id("p", 1)]).to_string(), 1)]);
        match next(&w) { Next::Wait { events, .. } => assert!(events.iter().any(|e| e["summary"].as_str().map_or(false, |s| s.starts_with("Every claude account is out"))), "{events:?}"), n => panic!("{n:?}") }
    }
    #[test] fn a_thread_not_held_is_restarted_where_it_was() { let w = world("claude", "opus", &[("fallback.on", "on".into()), ("fallback.chain", "claude/opus,codex/gpt-6".into())]); assert_eq!(next(&w), Next::Stay); }
    fn two() -> (&'static str, String) { inv(json!([{"id":"w","provider":"claude","state":"connected","epoch":2,"label":"Work","catalog":[{"id":"opus"}]},{"id":"p","provider":"claude","state":"connected","epoch":1,"label":"Personal","catalog":[{"id":"opus"}],
        "allowance":{"state":"ok","at":now_ms() / 1000,"provider":"claude","windows":[{"scope":"five_hour","used_percent":10.0}]}}])) }
    #[test] fn on_a_managed_provider_the_hold_is_on_the_account_and_the_restart_moves_to_the_best_other_account() {
        let (k, i) = two(); let binding = ("account.thread.t1.claude", accounts::binding_show("w", 2));
        let mut w = world("claude", "opus", &[(k, i), binding]); w.now = now_ms() / 1000; let at = w.now;
        let v = on_limit(&w, &lim("claude", at + 3600), true, "go"); assert_eq!(keys(&v.events), ["account.limit.6:claude1:w2:", "account.failover.pending.t1", "account.failover.tried.t1", "TurnChanged"]);
        let after = w.with(&v.events);
        match next(&after) { Next::Account { id, epoch, say } => { assert_eq!((id.as_str(), epoch), ("p", 1)); assert!(say.starts_with("Switched to Personal: Work ran out (resets "), "{say}") } n => panic!("{n:?}") }
        // both held: the thread waits for the earliest reset, says so with every account's name, and the chain may not be left when it is off
        let both = after.with(&[set(&fb::hold_key("claude", "p", 1), &json!({"scope":"claude","reset":at + 4000}).to_string(), 1)]);
        match next(&both) { Next::Wait { events, reset } => {
            assert_eq!(reset, at + 3600, "the earliest of them");
            assert!(events[1]["summary"].as_str().unwrap().starts_with("Every claude account is out (Work, Personal). Continuing at "), "{events:?}"); assert_eq!(events.len(), 2, "it will start again: its turn has not ended");
        } n => panic!("{n:?}") }
    }
    #[test] fn a_request_never_goes_back_to_an_account_it_ran_out_on_even_with_its_hold_over() {
        let (k, i) = two(); let mut w = world("claude", "opus", &[(k, i), ("account.thread.t1.claude", accounts::binding_show("p", 1)), ("account.failover.tried.t1", json!([fb::tried_id("w", 2), fb::tried_id("p", 1)]).to_string())]); w.now = now_ms() / 1000;
        match next(&w) { Next::Wait { events, reset } => { assert_eq!(reset, 0); assert!(events.iter().any(|e| e["$"] == "TurnChanged" && e["failover"].is_null()), "nothing will start it again: whoever waits on it hears"); assert!(events.last().unwrap()["summary"].as_str().unwrap().ends_with("no reset time is known. Waiting for you.")) } n => panic!("{n:?}") }
    }
    #[test] fn the_line_says_what_happened_to_the_account() {
        assert_eq!(switched_line("B", "A", 3600, false, "", 0), "Switched to B: A ran out (resets 01:00)"); assert_eq!(switched_line("B", "A", 0, false, "", 100), "Switched to B: A ran out (reset unknown)");
        assert_eq!(switched_line("B", "A", 3600, false, "", 0).len(), switched_line("B", "A", 7200, false, "UTC", 0).len()); assert_eq!(switched_line("B", "A", 0, true, "", 100), "Switched to B: A could not sign in");
        assert_eq!(switched_line("B", "A", 90000, false, "", 0), "Switched to B: A ran out (resets Jan 2 01:00)", "another day says which");
    }
    #[test] fn a_failed_sign_in_with_no_other_account_stays_the_failed_turn_it_is() {
        let l = fb::Limit { scope: "claude".into(), reason: "Please run /login".into(), reset: 0, auth: true };
        let v = on_limit(&world("claude", "opus", &[]), &l, true, "go"); assert!(v.events.is_empty() && !v.drop_turn_end && v.act.is_none());
        let (k, i) = two(); let w = world("claude", "opus", &[(k, i), ("account.thread.t1.claude", accounts::binding_show("w", 2))]);
        let v = on_limit(&w, &l, true, "go"); let rec: Value = serde_json::from_str(v.events[0]["value"].as_str().unwrap()).unwrap(); assert_eq!((rec["auth"].clone(), rec["reset"].clone()), (json!(true), json!(0)));
    }
    #[test] fn a_dead_sign_in_with_no_account_to_take_over_waits_for_the_sign_in_and_names_it() {
        let (k, i) = two(); let l = fb::Limit { scope: "claude".into(), reason: "Failed to authenticate: OAuth session expired and could not be refreshed".into(), reset: 0, auth: true };
        let mut w = world("claude", "opus", &[(k, i), ("account.thread.t1.claude", accounts::binding_show("w", 2))]); w.now = now_ms() / 1000;
        let held = w.with(&on_limit(&w, &l, true, "go").events).with(&[set(&fb::hold_key("claude", "p", 1), &json!({"scope":"claude","reset":0,"auth":true}).to_string(), 1)]);
        match next(&held) { Next::Wait { events, reset } => {
            assert_eq!(reset, 0);
            let rec: Value = serde_json::from_str(events[0]["value"].as_str().unwrap()).unwrap(); assert_eq!((rec["signin"].as_str(), rec["reason"].as_str()), (Some("w"), Some("sign-in expired")), "the sign-in it waits for");
            assert_eq!(events.last().unwrap()["summary"], "Work's sign-in expired and no other claude account can take over. Continuing once you sign in again.");
        } n => panic!("{n:?}") }
        // a limit, not a sign-in: no account named
        let lim_w = w.with(&on_limit(&w, &lim("claude", 0), true, "go").events).with(&[set(&fb::hold_key("claude", "p", 1), &json!({"scope":"claude","reset":0,"at":w.now}).to_string(), 1)]);
        match next(&lim_w) { Next::Wait { events, .. } => { let rec: Value = serde_json::from_str(events[0]["value"].as_str().unwrap()).unwrap(); assert!(rec["signin"].is_null()) } n => panic!("{n:?}") }
    }
    #[test] fn a_sign_in_goes_on_with_exactly_the_threads_it_stopped() {
        let r = json!({"target":"claude/opus","reset":0,"signin":"w"});
        assert!(signin_ready(&facts(), &r, "w")); assert!(!signin_ready(&facts(), &r, "p"), "another account's"); assert!(!signin_ready(&facts(), &rec(0), "w"), "a limit's wait");
        assert!(!signin_ready(&ResumeFacts { pending: 1, ..facts() }, &r, "w"), "the person queued something: theirs goes first");
        assert!(!signin_ready(&ResumeFacts { target: "claude/sonnet".into(), ..facts() }, &r, "w"), "the person changed it"); assert!(!signin_ready(&ResumeFacts { interrupted: false, ..facts() }, &r, "w"));
        assert!(!signin_ready(&ResumeFacts { ephemeral: true, ..facts() }, &r, "w"));
    }
    #[test] fn a_limit_with_no_reset_of_its_own_takes_the_one_of_the_window_that_filled() {
        let at = 1000; let (k, i) = inv(json!([{"id":"w","provider":"claude","state":"connected","epoch":2,"label":"Work","allowance":{"windows":[{"scope":"five_hour","used_percent":100.0,"resets_at":5000}]}}]));
        let w = world("claude", "opus", &[(k, i), ("account.thread.t1.claude", accounts::binding_show("w", 2))]); assert_eq!(w.now, at);
        let rec: Value = serde_json::from_str(on_limit(&w, &lim("claude", 0), true, "go").events[0]["value"].as_str().unwrap()).unwrap(); assert_eq!(rec["reset"], 5000);
    }
    #[test] fn a_remembered_limit_lends_its_reset_to_the_line_that_fails_the_turn() {
        assert_eq!(merged(Some(lim("c", 0)), Some(lim("c", 77))).unwrap().reset, 77); assert_eq!(merged(Some(lim("c", 5)), Some(lim("c", 77))).unwrap().reset, 5, "the line's own wins");
        assert_eq!(merged(None, Some(lim("c", 77))).unwrap().reset, 77); assert_eq!(merged(None, None), None);
        assert_eq!(merged(Some(lim("c/opus", 0)), Some(lim("c", 77))).unwrap().scope, "c", "the account-wide event outranks the words"); assert_eq!(merged(Some(lim("c", 0)), Some(lim("c/opus", 77))).unwrap().scope, "c");
    }
    #[test] fn the_ticker_continues_only_what_waited_for_a_reset_that_has_come() { /* the ticker is exercised with the agents; here: the record's reset gate */ assert!(due(&lim("c", 0), 5) && due(&lim("c", 9), 5) && !due(&lim("c", 5), 5)); }
    fn facts() -> ResumeFacts { ResumeFacts { exists: true, live: true, interrupted: true, target: "claude/opus".into(), ..Default::default() } }
    fn rec(reset: u64) -> Value { json!({"target":"claude/opus","reset":reset,"reason":"x"}) }
    #[test] fn a_resume_waits_for_exactly_an_interrupted_thread_whose_reset_has_come_and_whose_target_is_unchanged() {
        assert!(resume_ready(&facts(), &rec(50), 50), "the reset has come");
        assert!(!resume_ready(&facts(), &rec(51), 50), "not yet"); assert!(!resume_ready(&facts(), &rec(0), 50), "an unknown reset waits for the person");
        for (name, f) in [("completed", ResumeFacts { interrupted: false, ..facts() }), ("held", ResumeFacts { held: true, ..facts() }), ("dormant", ResumeFacts { dormant: true, ..facts() }),
            ("ephemeral", ResumeFacts { ephemeral: true, ..facts() }), ("archived or deleted", ResumeFacts { live: false, ..facts() }), ("a queued message", ResumeFacts { pending: 1, ..facts() }),
            ("the person changed provider/model", ResumeFacts { target: "codex/gpt-6".into(), ..facts() }), ("no thread", ResumeFacts::default())] {
            assert!(!resume_ready(&f, &rec(50), 50), "{name}: refused");
        }
    }
    #[test] fn a_resume_clears_its_record_and_logs_the_note_in_one_batch() {
        let ev = resume_events("provider.resume.t1", "t1", 77, 5);
        assert_eq!(keys(&ev), ["provider.resume.t1", "ActivityLogged"]); assert_eq!((ev[0]["value"].clone(), ev[1]["kind"].clone(), ev[1]["summary"].clone()), (json!(""), json!("provider-resume"), json!("Provider limit reset; continuing from the last step")));
        let other = resume_events("provider.resume.t2", "t2", 77, 6);
        assert_ne!(ev[1]["act"], other[1]["act"], "two resumes in one tick get different act ids"); assert_ne!(crate::agents::lines::hub_id("resume", 77, 5), crate::agents::lines::hub_id("resume", 77, 6), "and different message ids");
        assert!(RESUME_TEXT.starts_with("[Backplane] The provider's reported limit reset has arrived."));
    }
}

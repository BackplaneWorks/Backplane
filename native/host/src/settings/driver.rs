//! The provider-limit driver (hub.bend `Fallback.line` / `AccountFallback.line` / `ProviderResume.*` / `Fallback.use` / `AccountFallback.use`): what happens when
//! a running turn's agent line says its provider ran out of allowance. It reads each line the agents' actor already parsed (two touch points there: `on_line`
//! right after the line's events are built, `spawn_act` after the batch is committed) and decides with the pure planner in fallback.rs.
//! NOT WIRED YET: `on_line` and `spawn_act` have no caller (the agents lane adds the two hooks once this is on `gpui`), so the limit fallback does not run in this hub;
//! only `ResumeTicker` is registered, and it follows ProviderResume.ready / .one.
//!
//! * A limit seen in a turn that has not ended is only remembered (`fallback.pending.<thread>` or `account.failover.pending.<thread>`).
//! * A limit on the line that ends the turn (or a remembered one still in force when the turn ends badly) is acted on: the hold is written (a provider/model outage,
//!   or a hold on the thread's named account), the turn is Interrupted instead of Failed, and the same request is started again (`Act::Restart`), by `spawn_act`, on
//!   whatever is still eligible: another connected account of the provider with allowance left, else (opt-in chain, no account to use) the next chain target, else the
//!   thread waits (`provider.resume.<thread>`, continued on a reset by the ticker) or is blocked with a plain sentence.
//! NOT ported: reading a reset time out of the limit's words (`Reset.fill`; an unknown reset is held until cleared, never invented), the handed-over transcript for a
//!   switch to another provider (`Hand.switch`; the new provider gets the continue note and the original request), the alert records, and the preflight that also
//!   chooses on a manual start of a held thread (this driver chooses when it restarts).
use super::{accounts, fallback as fb, models};
use crate::{agents::Agents, api::{Fut, Ticker}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Default)]
pub struct Verdict { pub events: Vec<Value>, pub drop_turn_end: bool, pub act: Option<Act> }
#[derive(Debug, Clone, PartialEq)]
pub enum Act { Restart { text: String } }

/// Fallback.continue: what the restarted turn is told.
pub fn continue_text(original: &str) -> String {
    format!("[Backplane] The provider reached its usage limit. Continue the interrupted work from its current state. Read the handed-over tool results before acting. Do not repeat completed tool actions or restart finished work. If the request is already satisfied, report that.\n\nOriginal request (context, not a new task):\n{original}")
}
fn set(k: &str, v: &str, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }
fn turn(th: &str, state: &str, at: u64) -> Value { json!({"$":"TurnChanged","thread":th,"state":state,"at":at}) }
fn act_row(th: &str, tone: &str, kind: &str, text: &str, at: u64) -> Value { json!({"$":"ActivityLogged","thread":th,"act":format!("{kind}{at}"),"tone":tone,"kind":kind,"summary":text,"at":at}) }

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
    fn chain_on(&self) -> bool { self.get("fallback.on") == "on" }
    fn chain(&self) -> Vec<fb::Target> { fb::parse(&self.get("fallback.chain")) }
    /// The settings with `events` applied (the next decision sees the hold just written).
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
    Some(fb::Limit { scope: o["scope"].as_str().unwrap_or("").into(), reason: o["reason"].as_str().unwrap_or("usage limit reached").into(), reset: o["reset"].as_u64().unwrap_or(0) })
}

/// A limit seen on a line. `terminal`: the line ends the turn. The events to write now and, for a terminal one in force, the restart to make.
pub fn on_limit(w: &World, lim: &fb::Limit, terminal: bool, original: &str) -> Verdict {
    let (th, at) = (w.thread.as_str(), w.now);
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
        ev.push(fb::hold_mark(&w.provider, &id, epoch, lim, at, at));
        ev.push(set(&format!("account.failover.pending.{th}"), "", at)); ev.push(set(&format!("account.failover.continuing.{th}"), "yes", at));
    } else if w.chain_on() {
        // Fallback.limited: the outage is the provider's (or the model's) until its reset
        let held = |s: &str| fb::limit_active(&serde_json::from_str(&w.get(&fb::Limit::key(s))).unwrap_or(Value::Null), at);
        if !held(&lim.scope) { ev.push(set(&fb::Limit::key(&lim.scope), &lim.json(at), at)) }
        ev.push(set(&format!("fallback.pending.{th}"), "", at)); ev.push(set(&format!("fallback.continuing.{th}"), "yes", at));
    } else {
        // ProviderResume.hold: no fallback configured; the thread waits for the reported reset (or for the person when none came)
        ev.push(set(&format!("provider.resume.{th}"), &json!({"target":format!("{}/{}", w.provider, w.model),"reset":if lim.reset > at { lim.reset } else { 0 },"reason":lim.reason.chars().take(400).collect::<String>()}).to_string(), at));
        ev.push(set(&format!("fallback.pending.{th}"), "", at));
        ev.push(turn(th, "interrupted", at));
        ev.push(act_row(th, "warn", "provider-wait", &format!("Provider limit reached; {}", if lim.reset > at { "will continue when the reported reset arrives." } else { "no future reset supplied; waiting for updated provider evidence or your action." }), at));
        return Verdict { events: ev, drop_turn_end: true, act: None };
    }
    ev.push(turn(th, "interrupted", at));
    Verdict { events: ev, drop_turn_end: true, act: Some(Act::Restart { text: continue_text(original) }) }
}

/// What a restart does first: move the thread to what is still eligible. Events to write, and the text to start (None: nothing can run; the thread waits).
pub fn plan_switch(w: &World, text: &str) -> (Vec<Value>, Option<String>) {
    let (th, at) = (w.thread.as_str(), w.now);
    let get = |k: &str| w.get(k);
    if w.managed() {
        let profiles = w.inventory(); let (cur_id, cur_epoch) = w.binding();
        // the CLI's own login is one more candidate when the thread is on it and it is not held; a named profile is picked by allowance
        if let Some(p) = fb::pick_account(&profiles, &get, at, &w.provider, &w.model) {
            let (id, epoch) = (p["id"].as_str().unwrap_or(""), p["epoch"].as_u64().unwrap_or(0));
            if id == cur_id && epoch == cur_epoch { return (vec![], Some(text.to_string())) }
            let name = |i: &str| if i.is_empty() { "Default account".to_string() } else { profiles.iter().find(|x| x["id"] == i).and_then(|x| x["label"].as_str()).unwrap_or("account").to_string() };
            let say = format!("Usage limit reached on {}; automatically switched to {} ({}). Continuing the request.", name(&cur_id), name(id), w.provider);
            // each account has its own native session (its own config folder): the old one is kept, the new one's restored (a fresh account starts a new session and is handed the conversation)
            let new_session = get(&accounts::session_key(th, &w.provider, id, epoch));
            let ev = vec![set(&accounts::session_key(th, &w.provider, &cur_id, cur_epoch), &get("thread.session"), at),
                set(&accounts::binding_key(th, &w.provider), &accounts::binding_show(id, epoch), at),
                json!({"$":"SessionSet","id":th,"session":new_session,"at":at}), act_row(th, "info", "fallback", &say, at)];
            return (ev, Some(text.to_string()));
        }
        // nothing of this provider is eligible: an opt-in chain may leave it (Pick.may_cross), otherwise the thread waits
        if !(w.chain_on() && fb::may_cross(&profiles, &get, at, &w.provider, &w.model)) {
            let until = fb::next_reset(&profiles, &get, at, &w.provider, &w.model, &cur_id, cur_epoch);
            let why = if until > at { "every account is at its limit; will continue when the first resets." } else { "every account is at its limit and no reset was supplied; waiting for updated evidence or your action." };
            return (vec![set(&format!("provider.resume.{th}"), &json!({"target":format!("{}/{}", w.provider, w.model),"reset":until,"reason":"account usage limit"}).to_string(), at), turn(th, "interrupted", at), act_row(th, "warn", "provider-wait", &format!("Provider limit reached; {why}"), at)], None);
        }
    } else if !(w.chain_on() && fb::target_held(&get, &fb::Target { provider: w.provider.clone(), model: w.model.clone() }, at)) {
        return (vec![], Some(text.to_string()));
    }
    // the opt-in chain: the targets after this one, the first that is offered and not held (never wrapping)
    let from = fb::Target { provider: w.provider.clone(), model: w.model.clone() };
    let rest = fb::suffix(&w.chain(), &from.key());
    match fb::pick(&rest, &get, &w.caps, at) {
        None => {
            let say = format!("Action needed: all configured fallbacks are unavailable. {th}: usage limit reached.");
            (vec![set(&format!("provider.resume.{th}"), &json!({"target":from.key(),"reset":0,"reason":"all fallbacks unavailable"}).to_string(), at), turn(th, "interrupted", at), act_row(th, "error", "fallback-blocked", &say, at)], None)
        }
        Some(to) => {
            let say = format!("Automatic fallback: {} → {}. {th}: usage limit reached.", from.key(), to.key());
            let intent = { let s = w.get(&format!("fallback.intent.{th}")); if s.is_empty() { from.key() } else { s } };
            let session_now = w.get(&format!("fallback.session.{th}.{}", to.key()));
            (vec![set(&format!("fallback.intent.{th}"), &intent, at), json!({"$":"ProviderSet","id":th,"provider":to.provider,"model":to.model,"at":at}),
                json!({"$":"SessionSet","id":th,"session":session_now,"at":at}), act_row(th, "info", "fallback", &say, at)], Some(text.to_string()))
        }
    }
}

// ---- the hub side ----

async fn load(hub: &Arc<Hub>, thread: &str) -> Option<World> {
    let (provider, model, running) = { let m = hub.model().await; let t = m.threads.get(thread)?; (t.provider.clone(), t.model.clone(), t.turn == "running") };
    if !running { return None }
    let keys: Vec<String> = { let m = hub.model().await;
        let mut v: Vec<String> = ["fallback.", "account.", "accounts.inventory", "provider.resume."].iter().flat_map(|p| m.settings_with_prefix(p).map(|(k, _)| k.to_string()).collect::<Vec<_>>()).collect();
        v.retain(|k| !k.starts_with("account.session.") && !k.starts_with("account.seen.")); v };
    let mut settings = HashMap::new();
    for k in keys { if let Some(v) = hub.setting(&k).await { settings.insert(k, v); } }
    let mut caps = serde_json::from_str::<Value>(&models::info()).unwrap_or(Value::Null);
    if let Some(o) = caps.as_object_mut() { o.retain(|_, v| v.is_array()); }
    Some(World { now: now_ms() / 1000, thread: thread.into(), provider, model, settings, caps })
}

/// The original request of the interrupted turn: what it was started with.
async fn original(hub: &Arc<Hub>, thread: &str) -> String { hub.setting(&format!("fallback.input.{thread}")).await.unwrap_or_default() }

/// Called by the agents' actor for every line of a running turn, right after the line's own events are built. Empty unless the line is a provider limit
/// envelope for a running thread and the line is the thread's own (not a subagent's).
pub async fn on_line(hub: &Arc<Hub>, thread: &str, v: &Value, terminal: bool) -> Verdict {
    if !v["parent_tool_use_id"].as_str().unwrap_or("").is_empty() { return Verdict::default() }
    // a cheap look first: only an error envelope, a rejected-limit event or a rate-limited assistant line can be a limit, and a terminal line may close a remembered one
    let t = v["type"].as_str().unwrap_or("");
    let shaped = matches!(t, "turn.failed" | "error" | "result" | "rate_limit_event" | "assistant");
    if !shaped { return Verdict::default() }
    // classify first: it needs only the thread's provider and model; the settings are read only for a limit, or for a terminal line that may close a remembered one
    let (provider, model, running) = { let m = hub.model().await; match m.threads.get(thread) { Some(t) => (t.provider.clone(), t.model.clone(), t.turn == "running"), None => return Verdict::default() } };
    if !running { return Verdict::default() }
    let mut lim = fb::classify(&provider, &model, v);
    if lim.is_none() {
        if !terminal { return Verdict::default() }
        let mut mine = HashMap::new();
        for k in [format!("account.failover.pending.{thread}"), format!("fallback.pending.{thread}")] { if let Some(x) = hub.setting(&k).await { mine.insert(k, x); } }
        if mine.is_empty() { return Verdict::default() }
        let probe = World { now: now_ms() / 1000, thread: thread.into(), provider: provider.clone(), model: model.clone(), settings: mine, caps: Value::Null };
        lim = pending_of(&probe).filter(|p| due(p, probe.now));
    }
    let Some(lim) = lim else { return Verdict::default() };
    let Some(w) = load(hub, thread).await else { return Verdict::default() };
    let orig = original(hub, thread).await;
    on_limit(&w, &lim, terminal, &orig)
}

/// After the batch with the Interrupted turn is committed: choose what is still eligible, write the switch, and start the same request again.
pub fn spawn_act(ag: Arc<Agents>, thread: String, act: Act) {
    tokio::spawn(async move {
        let Act::Restart { text } = act;
        let hub = ag.hub.clone();
        let _ = ag.stop(&thread).await;
        // the thread is no longer running here: read it as it was, with the hold just written
        let (provider, model) = { let m = hub.model().await; match m.threads.get(&thread) { Some(t) => (t.provider.clone(), t.model.clone()), None => return } };
        let mut w = match load_idle(&hub, &thread, provider, model).await { Some(w) => w, None => return };
        w.now = now_ms() / 1000;
        let (ev, start) = plan_switch(&w, &text);
        let mut start = start;
        let key = accounts::binding_key(&thread, &w.provider);
        let switched = ev.iter().any(|e| e["$"] == "SettingSet" && e["key"] == key.as_str());
        let fresh = ev.iter().any(|e| e["$"] == "SessionSet" && e["session"] == "");
        if !ev.is_empty() { let _ = hub.commit_many(&ev).await; }
        if switched && fresh { // another account has no session of this conversation: it is told what was said, then the request
            if let (Some(t), Some(cx)) = (start.take(), crate::commands::entries(&hub, &thread).await) {
                let why = format!("This thread was worked on under another account until now, which reached its usage limit; you take it over. Its conversation so far, oldest first:");
                start = Some(format!("{}\n\n{}", crate::commands::hand_text(&why, &cx.entries, &thread), t));
            }
        }
        if let Some(t) = start { let _ = ag.start_queued(&thread, &t).await; }
    });
}
async fn load_idle(hub: &Arc<Hub>, thread: &str, provider: String, model: String) -> Option<World> {
    let keys: Vec<String> = { let m = hub.model().await; ["fallback.", "account.", "accounts.inventory", "provider.resume."].iter().flat_map(|p| m.settings_with_prefix(p).map(|(k, _)| k.to_string()).collect::<Vec<_>>()).collect() };
    let mut settings = HashMap::new();
    for k in keys { if let Some(v) = hub.setting(&k).await { settings.insert(k, v); } }
    let mut caps = serde_json::from_str::<Value>(&models::info()).unwrap_or(Value::Null);
    if let Some(o) = caps.as_object_mut() { o.retain(|_, v| v.is_array()); }
    let session = { let m = hub.model().await; m.threads.get(thread).map(|t| t.session.clone()).unwrap_or_default() };
    settings.insert("thread.session".into(), session);
    Some(World { now: now_ms() / 1000, thread: thread.into(), provider, model, settings, caps })
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
/// `ephemeral` is by name only: Bend's Ephemeral.is also counts a mail run or private run by record (MR.Run.is), which is not ported yet; whoever ports Run.is uses it here.
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
            run_resumes(false, hub, now_s, |th, msg, ev| { let ag = ag.clone(); async move { let _ = ag.deliver(&th, &msg, RESUME_TEXT, None, ev).await; } }).await;
            vec![]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world(provider: &str, model: &str, settings: &[(&str, String)]) -> World {
        World { now: 1000, thread: "t1".into(), provider: provider.into(), model: model.into(), settings: settings.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(), caps: json!({"claude":["opus","sonnet"],"codex":["gpt-6"],"grok":["g"]}) }
    }
    fn lim(scope: &str, reset: u64) -> fb::Limit { fb::Limit { scope: scope.into(), reason: "You've hit your usage limit".into(), reset } }
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
        assert_eq!(keys(&v.events), ["fallback.outage.claude", "fallback.pending.t1", "fallback.continuing.t1", "TurnChanged"]); assert!(v.drop_turn_end);
        assert_eq!(v.events[3]["state"], "interrupted"); match v.act.unwrap() { Act::Restart { text } => assert!(text.starts_with("[Backplane] The provider reached its usage limit.") && text.ends_with("build the board")) }
    }
    #[test] fn the_restart_moves_to_the_next_chain_target_and_never_wraps() {
        let hold = ("fallback.outage.claude", json!({"scope":"claude","reason":"r","reset":0,"at":1}).to_string());
        let w = world("claude", "opus", &[("fallback.on", "on".into()), ("fallback.chain", "claude/opus,codex/gpt-6".into()), hold.clone(), ("fallback.session.t1.codex/gpt-6", "s-codex".into())]);
        let (ev, start) = plan_switch(&w, "go"); assert_eq!(start.as_deref(), Some("go"));
        let ps = ev.iter().find(|e| e["$"] == "ProviderSet").unwrap(); assert_eq!((ps["provider"].clone(), ps["model"].clone()), (json!("codex"), json!("gpt-6")));
        assert_eq!(ev.iter().find(|e| e["$"] == "SessionSet").unwrap()["session"], "s-codex", "the session that target had before is resumed");
        assert_eq!(ev[0]["value"], "claude/opus", "the intent to return to is kept");
        // codex is held too: nothing is left, the work waits and says so; the chain does not wrap back to claude
        let both = world("codex", "gpt-6", &[("fallback.on", "on".into()), ("fallback.chain", "claude/opus,codex/gpt-6".into()), hold, ("fallback.outage.codex", json!({"scope":"codex","reset":0}).to_string())]);
        let (ev, start) = plan_switch(&both, "go"); assert!(start.is_none()); assert!(ev.iter().any(|e| e["kind"] == "fallback-blocked") && ev.iter().any(|e| e["key"] == "provider.resume.t1"));
    }
    #[test] fn a_thread_not_held_is_restarted_where_it_was() { let w = world("claude", "opus", &[("fallback.on", "on".into()), ("fallback.chain", "claude/opus,codex/gpt-6".into())]); assert_eq!(plan_switch(&w, "go"), (vec![], Some("go".to_string()))); }
    #[test] fn on_a_managed_provider_the_hold_is_on_the_account_and_the_restart_moves_to_the_best_other_account() {
        let profiles = json!([{"id":"w","provider":"claude","state":"connected","epoch":2,"label":"Work","catalog":[{"id":"opus"}]},{"id":"p","provider":"claude","state":"connected","epoch":1,"label":"Personal","catalog":[{"id":"opus"}],
            "allowance":{"state":"ok","at":now_ms() / 1000,"provider":"claude","windows":[{"scope":"five_hour","used_percent":10.0}]}}]);
        let (k, i) = inv(profiles); let binding = ("account.thread.t1.claude", accounts::binding_show("w", 2));
        let w = world("claude", "opus", &[(k, i), binding.clone()]);
        let v = on_limit(&w, &lim("claude", 0), true, "go"); assert_eq!(keys(&v.events), ["account.limit.6:claude1:w2:", "account.failover.pending.t1", "account.failover.continuing.t1", "TurnChanged"]);
        let after = w.with(&v.events); let (ev, start) = plan_switch(&after, "go");
        assert_eq!(start.as_deref(), Some("go")); let at_key = |k: &str| ev.iter().find(|e| e["key"] == k).cloned().unwrap_or(Value::Null);
        assert_eq!(at_key("account.thread.t1.claude")["value"], json!(accounts::binding_show("p", 1)));
        assert_eq!(at_key(&accounts::session_key("t1", "claude", "w", 2))["value"], "", "the old account's session is kept under its own key");
        assert!(ev.iter().any(|e| e["$"] == "SessionSet" && e["session"] == ""), "the other account starts its own session");
        assert_eq!(ev.last().unwrap()["summary"], "Usage limit reached on Work; automatically switched to Personal (claude). Continuing the request.");
        // both held: the thread waits for the earliest reset, and the chain may not be left when it is off
        let both = after.with(&[set(&fb::hold_key("claude", "p", 1), &json!({"scope":"claude","reset":now_ms() / 1000 + 4000}).to_string(), 1)]);
        let (ev, start) = plan_switch(&both, "go"); assert!(start.is_none()); let rec: Value = serde_json::from_str(ev[0]["value"].as_str().unwrap()).unwrap(); assert_eq!(rec["reset"].as_u64().unwrap() / 10, (now_ms() / 1000 + 4000) / 10, "an unknown reset is skipped (Reset.earliest): the known one is waited for");
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

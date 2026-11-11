//! The hub side of accounts: the `accounts` request, the
//! live snapshot every client is told (info `accounts.live`), the inventory kept in the log (setting `accounts.inventory`, so a restart still knows the profiles' ids and
//! epochs), a default that follows its profile away, and the threads whose account's identity changed or disconnected: their running turn is stopped, with the reason.
//! The JS hub offers `accounts*` only to a client the hub knows to be an account-capable web view (thin mode `view-web` and a current `accountProfiles` version, kept per
//! client). Here a connection is granted by a request in its view session that names the accounts wire version (`accountProfiles: 1`, as the web view sends it; `Hub::request`
//! calls `note_request`), the grant is kept until the connection detaches, and every other client gets the named-account message. The live profiles are not broadcast:
//! a granted client reads them with the `accounts` request (`op: list`), which is how the settings page gets them.
use super::{accounts, accounts_io::Accounts, fallback as fb};
use crate::{agents::Agents, api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::sync::Arc;

/// Accounts.public: what a client is told of the snapshot (no attempts: their links and codes are for the connection that asked).
pub fn public(snapshot: &Value) -> String { json!({"version":1,"revision":snapshot["revision"].as_u64().unwrap_or(0),"profiles":snapshot["profiles"].clone(),"error":snapshot["error"].as_str().unwrap_or("")}).to_string() }

/// Accounts.changes: the inventory when it moved, and a default whose profile is gone (only from a snapshot without an error).
pub fn changes(get: &dyn Fn(&str) -> String, snapshot: &Value, at: u64) -> Vec<Value> {
    let value = public(snapshot);
    let set = |k: &str, v: String| json!({"$":"SettingSet","key":k,"value":v,"at":at});
    let mut out = vec![];
    if get("accounts.inventory") != value { out.push(set("accounts.inventory", value)) }
    if snapshot["error"].as_str().unwrap_or("").is_empty() {
        let ids: Vec<&str> = snapshot["profiles"].as_array().map(|a| a.iter().filter_map(|p| p["id"].as_str()).collect()).unwrap_or_default();
        for provider in ["claude", "codex"] {
            let key = format!("account.default.{provider}");
            let id = serde_json::from_str::<Value>(&get(&key)).ok().and_then(|v| v["id"].as_str().map(str::to_string)).unwrap_or_default();
            let known = !id.is_empty() && accounts::profile(&get("accounts.inventory"), &id)["id"].as_str() == Some(id.as_str());
            if known && !ids.contains(&id.as_str()) { out.push(set(&key, accounts::binding_show("", 0))) }
        }
    }
    out
}
/// Hub.accounts.allowance: the default Claude login's windows, kept as a setting once the collector has a reading.
pub fn allowance(get: &dyn Fn(&str) -> String, snapshot: &Value, now: u64, at: u64) -> Option<Value> {
    let o = &snapshot["default_allowance"];
    if o["provider"] == "claude" && matches!(o["source"].as_str(), Some("claude-cli-usage" | "claude-oauth-usage")) && o["attempted_at"].as_u64().unwrap_or(0) <= now && get("allowance.claude") != o.to_string() {
        Some(json!({"$":"SettingSet","key":"allowance.claude","value":o.to_string(),"at":at}))
    } else { None }
}
/// AccountsRpc.retired: a thread bound to a named account whose profile is gone, disconnected, or at another epoch.
pub fn retired(binding_raw: &str, inventory: &str) -> bool {
    let b: Value = serde_json::from_str(binding_raw).unwrap_or(Value::Null);
    let id = b["id"].as_str().unwrap_or("");
    if id.is_empty() { return false }
    let p = accounts::profile(inventory, id);
    b["epoch"].as_u64().unwrap_or(0) != p["epoch"].as_u64().unwrap_or(0) || p["id"].as_str().unwrap_or("").is_empty() || p["state"] == "disconnected"
}

/// The threads running under this profile (Hub.accounts.active).
async fn active(hub: &Arc<Hub>, profile: &str) -> bool {
    let running: Vec<(String, String)> = { let m = hub.model().await; m.threads.iter().filter(|(_, t)| !t.deleted && t.turn == "running").map(|(id, t)| (id.clone(), t.provider.clone())).collect() };
    for (id, provider) in running { if serde_json::from_str::<Value>(&hub.setting(&accounts::binding_key(&id, &provider)).await.unwrap_or_default()).map_or(false, |b| b["id"] == profile) { return true } }
    false
}

// ---- a dead sign-in: the one alert, and the threads it stopped going on once it is back ----

/// What the last snapshot said of each profile: its state, and whether its sign-in is dead (expired, or a sign-in under way after it was).
pub type Seen = std::collections::HashMap<String, (String, bool)>;
/// SignIn.steps: of this snapshot, the profiles whose sign-in is newly dead and the ones that are back (connected after it was dead). `prev` becomes this snapshot.
pub fn signin_steps(prev: &mut Seen, snapshot: &Value) -> (Vec<String>, Vec<String>) {
    let (mut dead, mut back, mut next) = (vec![], vec![], Seen::new());
    for p in snapshot["profiles"].as_array().into_iter().flatten() {
        let (id, state) = (p["id"].as_str().unwrap_or("").to_string(), p["state"].as_str().unwrap_or("").to_string());
        let was = prev.get(&id).map_or(false, |(_, d)| *d);
        let now = state == "expired" || (state == "connecting" && was);
        if now && !was { dead.push(id.clone()) }
        if was && state == "connected" { back.push(id.clone()) }
        next.insert(id, (state, now));
    }
    *prev = next; (dead, back)
}
/// The setting that says the person was alerted that a provider's sign-in expired (one alert a provider, until one of its accounts is back); the push module sends it.
pub fn alert_key(provider: &str) -> String { format!("account.signin.alert.{provider}") }
pub fn provider_title(provider: &str) -> &'static str { match provider { "codex" => "Codex", _ => "Claude" } }
/// SignIn.alert: the alert to record for a dead account, once (none recorded yet), whether or not another account of its provider takes its work over (Nov 11: a broken
/// sign-in is the person's to fix, so they hear of it once, in the uberbot's name; a usage limit only waits for its reset). Its text is what the phone shows; `link` opens
/// that account's sign-in.
pub fn signin_alert(profiles: &[Value], get: &dyn Fn(&str) -> String, now: u64, id: &str) -> Option<Value> {
    let p = profiles.iter().find(|p| p["id"] == id)?;
    let provider = p["provider"].as_str().unwrap_or("");
    // a renewal reminder gives way to a sign-in that is dead now
    let cur = get(&alert_key(provider)); let renew = serde_json::from_str::<Value>(&cur).map_or(false, |r| r["renew"] == true);
    if !cur.is_empty() && !renew { return None }
    Some(json!({"profile":id,"epoch":p["epoch"],"provider":provider,"label":p["label"],"text":format!("{} sign-in expired: tap to sign in again", provider_title(provider)),"link":format!("account:{id}"),"at":now}))
}

/// How long before a sync token's end the person is asked to renew it.
pub const RENEW_AHEAD: u64 = 30 * 86400;
/// The renewal alert for the sync token nearest its end, a month ahead (`RENEW_AHEAD`), when no Claude sign-in alert is recorded: the sign-in alert's own record
/// (`account.signin.alert.claude`, `renew: true`), so the push module sends it as it sends an expired sign-in, and its link opens the account.
pub fn renew_alert(profiles: &[Value], get: &dyn Fn(&str) -> String, zone: &str, now: u64) -> Option<Value> {
    if !get(&alert_key("claude")).is_empty() { return None }
    let p = profiles.iter().filter(|p| p["kind"] == super::accounts_io::SYNC && p["state"] != "disconnected").filter(|p| { let e = p["tokenExpiresAt"].as_u64().unwrap_or(0); e > 0 && now + RENEW_AHEAD >= e })
        .min_by_key(|p| p["tokenExpiresAt"].as_u64().unwrap_or(0))?;
    let (id, ends, lab) = (p["id"].as_str().unwrap_or(""), p["tokenExpiresAt"].as_u64().unwrap_or(0), p["label"].as_str().unwrap_or(""));
    let when = if ends > now { format!("ends {}", crate::agents::rules::when(zone, ends, now)) } else { "has ended".to_string() };
    Some(json!({"profile":id,"epoch":p["epoch"],"provider":"claude","label":lab,"text":format!("Claude sync token {lab} {when}: renew the sync token"),"link":format!("account:{id}"),"at":now,"renew":true}))
}
/// A renewal alert that is over: its account was renewed (its end is more than a month away) or is gone. Any other record is not this one's to clear.
pub fn renew_over(rec: &str, profiles: &[Value], now: u64) -> bool {
    let Ok(r) = serde_json::from_str::<Value>(rec) else { return false };
    if r["renew"] != true { return false }
    match profiles.iter().find(|p| p["id"] == r["profile"]) { None => true, Some(p) => p["state"] == "disconnected" || p["tokenExpiresAt"].as_u64().unwrap_or(0) > now + RENEW_AHEAD }
}

/// Where a start may go when its own account cannot start a turn: the provider's other saved accounts that may (connected, or not yet checked since the restart: the
/// launch checks it), not held by a limit that is still on, in the order they were saved. `(id, epoch, label)`.
pub async fn others(hub: &Arc<Hub>, mgr: &Accounts, provider: &str, except: &str) -> Vec<(String, u64, String)> {
    let snap = mgr.snapshot(); let now = now_ms() / 1000;
    let keys: Vec<String> = { let m = hub.model().await; m.settings_with_prefix("account.limit.").map(|(k, _)| k.to_string()).collect() };
    let mut held = std::collections::HashMap::new();
    for k in keys { if let Some(v) = hub.setting(&k).await { held.insert(k, v); } }
    let get = |k: &str| held.get(k).cloned().unwrap_or_default();
    snap["profiles"].as_array().into_iter().flatten().filter(|p| p["provider"] == provider && p["id"] != except && matches!(p["state"].as_str(), Some("connected" | "unchecked")))
        .filter(|p| !fb::hold_current(&get, provider, p["id"].as_str().unwrap_or(""), p["epoch"].as_u64().unwrap_or(0), "", now))
        .map(|p| (p["id"].as_str().unwrap_or("").to_string(), p["epoch"].as_u64().unwrap_or(0), p["label"].as_str().unwrap_or("").to_string())).collect()
}
/// What a start that cannot run on any login records, once: the same record an expired sign-in makes (`signin_alert`), so the push module sends the person the one
/// needs_you alert in the uberbot's name. None when one is recorded already (until an account is back and the record is emptied).
pub fn start_alert(get: &dyn Fn(&str) -> String, provider: &str, profile: &str, title: &str, why: &str, now: u64) -> Option<Value> {
    if !get(&alert_key(provider)).is_empty() { return None }
    let link = if profile.is_empty() { String::new() } else { format!("account:{profile}") };
    Some(json!({"profile":profile,"provider":provider,"text":format!("{} sign-in needed: no account could start {title} ({why})", provider_title(provider)),"link":link,"at":now,"start":true}))
}
static ALERTING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// Records `start_alert` for a thread that could not start, checked and written under one lock so two threads failing at once make one alert.
pub async fn alert_start(hub: &Arc<Hub>, provider: &str, profile: &str, title: &str, why: &str) -> bool {
    let _g = ALERTING.lock().await;
    let cur = hub.setting(&alert_key(provider)).await.unwrap_or_default();
    let Some(rec) = start_alert(&|k: &str| if k == alert_key(provider) { cur.clone() } else { String::new() }, provider, profile, title, why, now_ms() / 1000) else { return false };
    hub.commit_many(&[json!({"$":"SettingSet","key":alert_key(provider),"value":rec.to_string(),"at":now_ms() / 1000})]).await.is_ok()
}
/// The setting a start that fell back writes: the login this thread's process runs on now, while its binding still names its own account. The watch below does not take
/// that thread for one whose account was retired under it.
pub fn fallback_key(thread: &str) -> String { format!("account.fallback.{thread}") }

/// The info `accounts.live`: each account's id, provider, label, kind, state and who, nothing else (no sign-in attempt, link or code, no catalog,
/// no allowance; never a token, which no snapshot holds). Setup's checks and the home move's readiness read it (settings/onboard.rs, net/homemove.rs).
/// Nov 11: nothing ever set it, so a hub whose only Claude login was a sync account (boat-hub) read as not signed in and the move was refused.
pub fn live_info(snapshot: &Value) -> String {
    let profiles: Vec<Value> = snapshot["profiles"].as_array().into_iter().flatten().map(|p| json!({"id":p["id"],"provider":p["provider"],"label":p["label"],"kind":p["kind"],"origin":p["origin"],
        "state":p["state"],"epoch":p["epoch"],"identity":{"email":p["identity"]["email"],"plan":p["identity"]["plan"]}})).collect();
    json!({"version":1,"revision":snapshot["revision"].as_u64().unwrap_or(0),"error":snapshot["error"].as_str().unwrap_or(""),"profiles":profiles,"attempts":[]}).to_string()
}
/// One snapshot that changed: the log, the clients, and the turns that lost their account.
pub async fn found(hub: &Arc<Hub>, agents: &Option<Arc<Agents>>, snapshot: &Value) { found_seen(hub, agents, snapshot, &mut Seen::new()).await }
pub async fn found_seen(hub: &Arc<Hub>, agents: &Option<Arc<Agents>>, snapshot: &Value, seen: &mut Seen) {
    let at = now_ms() / 1000;
    let mut settings = std::collections::HashMap::new();
    for k in ["accounts.inventory", "account.default.claude", "account.default.codex", "allowance.claude"] { settings.insert(k.to_string(), hub.setting(k).await.unwrap_or_default()); }
    let get = |k: &str| settings.get(k).cloned().unwrap_or_default();
    let mut evs = changes(&get, snapshot, at);
    if let Some(e) = allowance(&get, snapshot, at, at) { evs.push(e) }
    if !evs.is_empty() { let _ = hub.commit_many(&evs).await; }
    let inventory = public(snapshot);
    // not broadcast: the live profiles go only to clients granted account access (the `accounts` request)
    // threads whose account changed under them
    let running: Vec<(String, String)> = { let m = hub.model().await; m.threads.iter().filter(|(_, t)| !t.deleted && t.turn == "running").map(|(id, t)| (id.clone(), t.provider.clone())).collect() };
    for (id, provider) in running {
        let raw = hub.setting(&accounts::binding_key(&id, &provider)).await.unwrap_or_default();
        if !retired(&raw, &inventory) { continue }
        if !hub.setting(&fallback_key(&id)).await.unwrap_or_default().is_empty() { continue } // it runs on another login already: its own account's change is not its business now
        // only a turn whose process runs on the retired account is cut. A turn that is still choosing its login (it shows running from the moment the message is taken, before
        // its spawn has tried the account) goes through account_route, which takes another account or says why it cannot start: cutting it here interrupted a turn that
        // was about to start on Personal right after Work was signed out (restart_e2e, under load).
        let on = serde_json::from_str::<Value>(&raw).ok().and_then(|b| b["id"].as_str().map(str::to_string)).unwrap_or_default();
        if let Some(m) = agents.as_ref().and_then(|a| a.accounts.get()) { if m.runs_on(&id).as_deref() != Some(on.as_str()) { continue } }
        let _ = hub.commit_many(&[json!({"$":"TurnChanged","thread":id,"state":"interrupted","at":at}),
            json!({"$":"ActivityLogged","thread":id,"act":format!("account-retired:{at}"),"tone":"error","kind":"account","summary":"This account's verified identity changed or disconnected. Check status and explicitly select an account to continue.","at":at})]).await;
        if let Some(a) = agents { let _ = a.stop(&id).await; }
    }
    if !snapshot["error"].as_str().unwrap_or("").is_empty() { return }
    let (_, back) = signin_steps(seen, snapshot);
    let profiles = snapshot["profiles"].as_array().cloned().unwrap_or_default();
    // back: the alert is over, and every thread the expiry stopped goes on
    for id in &back {
        let provider = profiles.iter().find(|p| p["id"] == id.as_str()).and_then(|p| p["provider"].as_str()).unwrap_or("").to_string();
        if !hub.setting(&alert_key(&provider)).await.unwrap_or_default().is_empty() { let _ = hub.commit_many(&[json!({"$":"SettingSet","key":alert_key(&provider),"value":"","at":at})]).await; }
        if let Some(a) = agents { let n = super::driver::signed_in(a, id, at).await; if n > 0 { eprintln!("accounts: {n} thread(s) go on after a sign-in") } }
    }
    // a sync token near its end: the same one alert, a month ahead, until it is renewed (or removed)
    {
        let (cur, zone) = (hub.setting(&alert_key("claude")).await.unwrap_or_default(), hub.setting("tz.machine").await.unwrap_or_default());
        if renew_over(&cur, &profiles, at) { let _ = hub.commit_many(&[json!({"$":"SettingSet","key":alert_key("claude"),"value":"","at":at})]).await; }
        else if let Some(rec) = renew_alert(&profiles, &|k: &str| if k == alert_key("claude") { cur.clone() } else { String::new() }, &zone, at) {
            let _ = hub.commit_many(&[json!({"$":"SettingSet","key":alert_key("claude"),"value":rec.to_string(),"at":at})]).await;
        }
    }
    // still dead with nothing to take over: the one alert (the push module sends it in the uberbot's name)
    let dead: Vec<&str> = seen.iter().filter(|(_, (_, d))| *d).map(|(id, _)| id.as_str()).collect();
    if dead.is_empty() { return }
    let keys: Vec<String> = { let m = hub.model().await; m.settings_with_prefix("account.limit.").map(|(k, _)| k.to_string()).chain(["claude", "codex"].iter().map(|p| alert_key(p))).collect() };
    let mut held = std::collections::HashMap::new();
    for k in keys { if let Some(v) = hub.setting(&k).await { held.insert(k, v); } }
    for id in dead {
        let get = |k: &str| held.get(k).cloned().unwrap_or_default();
        if let Some(rec) = signin_alert(&profiles, &get, at, id) {
            let key = alert_key(rec["provider"].as_str().unwrap_or(""));
            let _ = hub.commit_many(&[json!({"$":"SettingSet","key":key,"value":rec.to_string(),"at":at})]).await; held.insert(key, rec.to_string());
        }
    }
}

/// Clients allowed to manage accounts: `(generation, client)`, kept for the life of the connection (Client.accounts). Process-wide so the request path can grant and
/// `Hub::detach` can revoke.
static GRANTED: std::sync::Mutex<Vec<(String, u64)>> = std::sync::Mutex::new(Vec::new());
pub fn grant_client(gen: &str, client: u64) { let mut g = GRANTED.lock().unwrap(); if !g.iter().any(|(a, c)| a == gen && *c == client) { g.push((gen.to_string(), client)) } }
pub fn revoke_client(client: u64) { GRANTED.lock().unwrap().retain(|(_, c)| *c != client) }
pub fn is_granted(who: &Caller) -> bool { GRANTED.lock().unwrap().iter().any(|(a, c)| a == &who.gen && *c == who.client) }
/// Accounts.offer: a request that names the accounts wire version (`accountProfiles: 1`) on a connection that is in a view session grants the connection, as the JS
/// hub does for a thin web view (`view-web` + `TVW.Wire.version`). This hub has one renderer (`mobile`), so the mode is the view session itself; the grant is kept.
pub fn offered(p: &Value) -> bool { p.get("accountProfiles") == Some(&json!(1)) }
pub fn note_request(gen: &str, client: u64, in_view: bool, msg: &Value) { if in_view && offered(msg.get("p").unwrap_or(&Value::Null)) { grant_client(gen, client) } }

/// Marks a profile as in use by a running child until dropped (login, disconnect and removal refuse while one is).
pub struct Busy(Arc<Accounts>, String, Option<(String, u64)>);
impl Busy {
    pub fn new(m: Arc<Accounts>, id: &str) -> Busy { m.child_started(id); Busy(m, id.to_string(), None) }
    /// Also records that thread `th`'s process runs on it (`Accounts::runs_on`), which the retire watch below asks before it interrupts a turn.
    pub fn for_thread(m: Arc<Accounts>, id: &str, th: &str) -> Busy { m.child_started(id); let t = m.run_on(th, id); Busy(m, id.to_string(), Some((th.to_string(), t))) }
}
impl Drop for Busy { fn drop(&mut self) { self.0.child_ended(&self.1); if let Some((th, t)) = &self.2 { self.0.run_off(th, *t) } } }

pub struct AccountsRpc { pub mgr: Arc<Accounts> }
impl AccountsRpc {
    pub fn new(mgr: Arc<Accounts>) -> Self { Self { mgr } }
    pub fn grant(&self, who: &Caller) { grant_client(&who.gen, who.client) }
    pub fn revoke(&self, who: &Caller) { revoke_client(who.client) }
    fn allowed(&self, who: &Caller) -> bool { is_granted(who) }
}
impl RpcHandler for AccountsRpc {
    fn methods(&self) -> &'static [&'static str] { &["accounts"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if !self.allowed(who) { return vec![json!({"t":"reply","id":id,"ok":false,"error":accounts::GATE})] }
            let op = p.get("op").and_then(Value::as_str).unwrap_or("");
            let pid = p.get("profile").and_then(Value::as_str).unwrap_or("");
            // a dead login may be signed in again while turns of it are still winding down: they can do nothing with it
            let dead = op == "login" && self.mgr.state_of(pid) == "expired";
            if matches!(op, "remove" | "login" | "disconnect") && !dead && active(hub, pid).await {
                return vec![json!({"t":"reply","id":id,"ok":false,"error":"Stop this account's active turns before changing its login."})]
            }
            let (kind, origin) = self.mgr.kind_of(pid);
            let mut snap = self.mgr.request(p).await;
            // a sync account another machine made, removed here: that machine is asked to remove it too, so it leaves every machine
            if op == "remove" && kind == super::accounts_io::SYNC && !origin.is_empty() && snap["error"].as_str().unwrap_or("").is_empty() { super::accounts_sync::dropped(hub, &origin, pid).await; }
            // a client that names a thread is also told what the thread is bound to (the account chip and its menu need the binding to send `accounts.select` with `expected`)
            snap["defaults"] = json!({"claude": hub.setting("account.default.claude").await.unwrap_or_default(), "codex": hub.setting("account.default.codex").await.unwrap_or_default()});
            let mut model = String::new();
            if let Some(th) = p.get("thread").and_then(Value::as_str).map(|t| t.rsplit('|').next().unwrap_or(t)).filter(|t| !t.is_empty()) { // a picture names a thread `hub|id`
                let (provider, m) = { let m = hub.model().await; m.threads.get(th).map(|t| (t.provider.clone(), t.model.clone())).unwrap_or_default() }; model = m;
                let raw = if accounts::supported(&provider) { hub.setting(&accounts::binding_key(th, &provider)).await.unwrap_or_default() } else { String::new() };
                let chosen = if accounts::supported(&provider) { hub.setting(&accounts::chosen_key(th, &provider)).await.unwrap_or_default() } else { String::new() };
                snap["thread"] = json!({"id": th, "provider": provider, "binding": raw, "pinned": serde_json::from_str::<Value>(&chosen).map_or(false, |c| c["by"] == "person")});
            }
            snap["holds"] = holds(hub, &snap["profiles"], &model).await;
            vec![json!({"t":"reply","id":id,"ok":snap["error"].as_str().unwrap_or("").is_empty(),"result":snap})]
        })
    }
}

/// Which accounts are out now, for the menus and Settings: `{profile id: {"until": seconds, "text": "limited until 15:40"}}`. The client shows the words; the hub knows the zone and
/// the holds. The CLI's own login is id "" (each provider's: a hold names its provider); it is out when its hold is, as a named account is.
pub async fn holds(hub: &Arc<Hub>, profiles: &Value, model: &str) -> Value {
    let keys: Vec<String> = { let m = hub.model().await; m.settings_with_prefix("account.limit.").map(|(k, _)| k.to_string()).collect() };
    let mut settings = std::collections::HashMap::new();
    for k in keys { if let Some(v) = hub.setting(&k).await { settings.insert(k, v); } }
    let get = |k: &str| settings.get(k).cloned().unwrap_or_default();
    let (now, zone) = (now_ms() / 1000, hub.setting("tz.machine").await.unwrap_or_default());
    let mut out = serde_json::Map::new();
    let own: Vec<Value> = ["claude", "codex"].iter().map(|pv| json!({"id":"","provider":pv,"epoch":0})).collect();
    for p in own.iter().chain(profiles.as_array().into_iter().flatten()) { let (id, pv, e) = (p["id"].as_str().unwrap_or(""), p["provider"].as_str().unwrap_or(""), p["epoch"].as_u64().unwrap_or(0));
        if out.contains_key(id) || !fb::hold_current(&get, pv, id, e, model, now) { continue }
        let until = fb::hold_until(&get, pv, id, e, model);
        out.insert(id.to_string(), json!({"until":until,"text":limited_text(&zone, until, now)}));
    }
    Value::Object(out)
}

/// A held login's words: "limited until Nov 12, 03:00" (the reset in the machine's zone), "limited" when no end is known.
pub fn limited_text(zone: &str, until: u64, now: u64) -> String { if until > now { format!("limited until {}", crate::agents::rules::when(zone, until, now)) } else { "limited".to_string() } }

static LIVE: std::sync::Mutex<Vec<(std::path::PathBuf, std::sync::Weak<Accounts>)>> = std::sync::Mutex::new(Vec::new());
/// The account manager running for `home` (the 0.14 import adds the profiles it copied to it).
pub fn live(home: &std::path::Path) -> Option<Arc<Accounts>> { LIVE.lock().unwrap().iter().find(|(h, _)| h == home).and_then(|(_, w)| w.upgrade()) }
/// The manager for this hub's home (provider CLIs resolved like the agents'), its RPC, and the 1-second watch that tells everyone of a change.
pub fn start(hub: &Arc<Hub>, agents: Option<Arc<Agents>>) -> Arc<Accounts> { start_cli(hub, agents, crate::agents::proc::Cli::from_env()) }
/// The hub's one account manager, opened on first use (a cheap read of the private registry). The agents take it before their recovery carries any turn on, settings::start
/// takes the same one: neither start waits on the other. Nov 7: the manager was made by settings::start, after `Agents::start` returned, while that start's recovery waited a
/// minute per carried turn for it; every turn a restart cut on a saved account failed with "Accounts are not available".
pub fn manager(hub: &Arc<Hub>, cli: crate::agents::proc::Cli) -> Arc<Accounts> { hub.accounts.get_or_init(|| Arc::new(Accounts::open(&hub.cfg.home, Arc::new(move |p: &str| cli.resolve(p))))).clone() }
/// `start` with the provider CLIs found by `cli` (a test gives it a folder of stand-ins).
pub fn start_cli(hub: &Arc<Hub>, agents: Option<Arc<Agents>>, cli: crate::agents::proc::Cli) -> Arc<Accounts> {
    let mgr = manager(hub, cli);
    LIVE.lock().unwrap().retain(|(h, w)| w.strong_count() > 0 && h != &hub.cfg.home); LIVE.lock().unwrap().push((hub.cfg.home.clone(), Arc::downgrade(&mgr)));
    if let Some(a) = &agents { let _ = a.accounts.set(mgr.clone()); } // the agents' start overran its budget: they take the same manager from the hub when they get to it
    hub.register_rpc(Arc::new(AccountsRpc::new(mgr.clone())));
    super::accounts_sync::start(hub, mgr.clone());
    let (h, m) = (hub.clone(), mgr.clone());
    tokio::spawn(async move {
        let (mut last, mut seen) = (String::new(), Seen::new());
        loop {
            // the background recheck of connected accounts: never on a copy of a log, never on a test hub (stand-in agents) unless its rig asks (BACKPLANE_USAGE_READ)
            if !crate::copy::on() && (std::env::var_os("BACKPLANE_AGENTS").is_none() || std::env::var("BACKPLANE_USAGE_READ").map_or(false, |v| v == "1")) { let m2 = m.clone(); tokio::spawn(async move { m2.refresh().await }); }
            let snap = m.snapshot(); let text = snap.to_string();
            if text != last { last = text; found_seen(&h, &agents, &snap, &mut seen).await; h.set_info("accounts.live", &live_info(&snap)).await; }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
    mgr
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snap(profiles: Value, error: &str) -> Value { json!({"version":1,"revision":3,"profiles":profiles,"error":error,"attempts":[{"id":"a","url":"https://claude.ai/x","userCode":"C"}]}) }
    #[test] fn the_live_info_names_each_account_and_its_state_and_nothing_of_a_sign_in() {
        let s = json!({"revision":4,"error":"","profiles":[{"id":"4f1b","provider":"claude","label":"K sync","kind":"sync","origin":"pc1","state":"connected","epoch":1,"identity":{"email":"k@x.com","plan":"max","auth":"oauth_token"},"catalog":[{"id":"opus"}],"allowance":{"windows":[]}}],
            "attempts":[{"id":"a","profile":"4f1b","url":"https://claude.com/x","userCode":"C"}]});
        let v: Value = serde_json::from_str(&live_info(&s)).unwrap();
        assert_eq!((v["profiles"][0]["state"].as_str(), v["profiles"][0]["kind"].as_str(), v["profiles"][0]["identity"]["email"].as_str()), (Some("connected"), Some("sync"), Some("k@x.com")));
        assert!(v["attempts"].as_array().unwrap().is_empty() && v["profiles"][0].get("catalog").is_none() && !v.to_string().contains("claude.com/x"), "{v}");
        assert!(live_info(&s).len() < 64 * 1024);
    }
    async fn ask(rpc: &AccountsRpc, hub: &Arc<Hub>, who: &Caller) -> Value { rpc.call(hub, who, &json!(1), "accounts", &json!({"op":"list","accountProfiles":true})).await.remove(0) }
    #[tokio::test(flavor = "multi_thread")] async fn only_a_granted_client_may_manage_accounts() {
        let home = crate::plat::tmp::dir("acc-hub");
        let hub = Hub::start(crate::hub::Cfg { home: home.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
        let rpc = AccountsRpc::new(Arc::new(Accounts::open(&home, Arc::new(|p: &str| Ok(std::path::PathBuf::from(p))))));
        let (a, b) = (Caller { gen: "g".into(), client: 9001 }, Caller { gen: "g".into(), client: 9002 });
        assert_eq!(ask(&rpc, &hub, &a).await["error"], accounts::GATE, "naming the capability on the accounts request itself grants nothing");
        note_request("g", 9002, false, &json!({"m":"accounts","p":{"accountProfiles":1}})); assert!(!is_granted(&b), "not outside a view session");
        note_request("g", 9002, true, &json!({"m":"view.open","p":{"accountProfiles":2}})); assert!(!is_granted(&b), "only the current wire version");
        note_request("g", 9002, true, &json!({"m":"view.open","p":{"accountProfiles":1}})); assert!(is_granted(&b), "a view session that offers the version is granted, and it stays");
        assert_eq!(ask(&rpc, &hub, &b).await["ok"], true); revoke_client(9002); assert!(!is_granted(&b), "detach revokes");
        rpc.grant(&a); assert_eq!(ask(&rpc, &hub, &a).await["ok"], true); assert_eq!(ask(&rpc, &hub, &b).await["error"], accounts::GATE, "per connection");
        rpc.revoke(&a); assert_eq!(ask(&rpc, &hub, &a).await["error"], accounts::GATE); let _ = std::fs::remove_dir_all(&home);
    }
    #[test] fn clients_are_told_the_profiles_but_never_an_attempts_link_or_code() { let t = public(&snap(json!([{"id":"w"}]), "")); assert!(!t.contains("userCode") && !t.contains("https://") && t.contains("\"revision\":3") && t.contains("\"profiles\":[{\"id\":\"w\"}]")); }
    #[test] fn the_inventory_is_kept_when_it_moves_and_a_default_follows_its_profile_away() {
        let s = snap(json!([{"id":"w","epoch":2}]), "");
        let get = |k: &str| match k { "accounts.inventory" => json!({"version":1,"revision":2,"profiles":[{"id":"w","epoch":1},{"id":"gone","epoch":1}],"error":""}).to_string(), "account.default.claude" => accounts::binding_show("gone", 1), "account.default.codex" => accounts::binding_show("w", 1), _ => String::new() };
        let c = changes(&get, &s, 9); let keys: Vec<&str> = c.iter().map(|e| e["key"].as_str().unwrap()).collect();
        assert_eq!(keys, ["accounts.inventory", "account.default.claude"], "codex's default names a profile that is still there");
        assert_eq!(c[1]["value"], accounts::binding_show("", 0));
        assert!(changes(&|k| if k == "accounts.inventory" { public(&s) } else { String::new() }, &s, 9).is_empty(), "an unchanged inventory writes nothing");
        assert_eq!(changes(&get, &snap(json!([]), "The private account registry could not be verified."), 9).len(), 1, "a snapshot with an error removes no default");
    }
    #[test] fn a_thread_is_retired_when_its_account_changed_disconnected_or_vanished() {
        let inv = json!({"profiles":[{"id":"w","epoch":2,"state":"connected"},{"id":"d","epoch":1,"state":"disconnected"}]}).to_string();
        assert!(!retired(&accounts::binding_show("w", 2), &inv)); assert!(retired(&accounts::binding_show("w", 1), &inv), "another epoch");
        assert!(retired(&accounts::binding_show("d", 1), &inv), "disconnected"); assert!(retired(&accounts::binding_show("x", 1), &inv), "gone");
        assert!(!retired("", &inv) && !retired(&accounts::binding_show("", 0), &inv), "the CLI's own login is never retired");
    }
    #[test] fn a_dead_sign_in_is_seen_once_and_its_return_through_a_new_sign_in_is_seen() {
        let snap = |st: &str| json!({"profiles":[{"id":"w","state":st},{"id":"p","state":"connected"}],"error":""});
        let mut seen = Seen::new();
        assert_eq!(signin_steps(&mut seen, &snap("connected")), (vec![], vec![]));
        assert_eq!(signin_steps(&mut seen, &snap("expired")), (vec!["w".to_string()], vec![]));
        assert_eq!(signin_steps(&mut seen, &snap("expired")), (vec![], vec![]), "once");
        assert_eq!(signin_steps(&mut seen, &snap("connecting")), (vec![], vec![]), "a sign-in under way is still the dead one");
        assert_eq!(signin_steps(&mut seen, &snap("connected")), (vec![], vec!["w".to_string()]));
        assert_eq!(signin_steps(&mut seen, &snap("connecting")), (vec![], vec![]), "a sign-in of a working account is not a return");
        assert_eq!(signin_steps(&mut seen, &snap("connected")), (vec![], vec![]));
        let mut fresh = Seen::new(); assert_eq!(signin_steps(&mut fresh, &snap("expired")).0, ["w"], "a restart that finds it dead");
    }
    #[test] fn a_broken_sign_in_is_told_once_even_when_another_account_takes_over() {
        let ok = json!({"state":"ok","at":9_999_999_999u64,"windows":[{"scope":"five_hour","used_percent":10.0}]});
        let ps = vec![json!({"id":"w","provider":"claude","label":"Work","epoch":2,"state":"expired"}), json!({"id":"p","provider":"claude","label":"Personal","epoch":1,"state":"connected","allowance":ok}), json!({"id":"c","provider":"codex","label":"C","epoch":1,"state":"connected"})];
        assert!(signin_alert(&ps, &|_| String::new(), 100, "w").is_some(), "Personal takes the work over, and the person still hears the sign-in broke");
        let held = fb::hold_key("claude", "p", 1); let rec = json!({"scope":"claude","reset":0,"at":90}).to_string();
        let get = |k: &str| if k == held { rec.clone() } else { String::new() };
        let a = signin_alert(&ps, &get, 100, "w").expect("Personal is out too");
        assert_eq!((a["text"].as_str().unwrap(), a["link"].as_str().unwrap(), a["provider"].as_str().unwrap()), ("Claude sign-in expired: tap to sign in again", "account:w", "claude"));
        let told = a.to_string(); let get2 = |k: &str| if k == held { rec.clone() } else if k == alert_key("claude") { told.clone() } else { String::new() };
        assert!(signin_alert(&ps, &get2, 100, "w").is_none(), "one alert a provider until one of its accounts is back");
    }
    #[test] fn a_sync_token_near_its_end_asks_for_a_renewal_a_month_ahead_once_and_gives_way_to_a_dead_sign_in() {
        let now = 1_800_000_000; let day = 86400;
        let ps = vec![json!({"id":"s","provider":"claude","kind":"sync","label":"Work sync","state":"connected","epoch":1,"tokenExpiresAt":now + 20 * day}),
            json!({"id":"far","provider":"claude","kind":"sync","label":"Later","state":"connected","epoch":1,"tokenExpiresAt":now + 200 * day}), json!({"id":"w","provider":"claude","kind":"","label":"Work","state":"connected","epoch":2})];
        let none = |_: &str| String::new();
        let r = renew_alert(&ps, &none, "UTC", now).unwrap();
        assert_eq!((r["profile"].as_str().unwrap(), r["link"].as_str().unwrap(), r["renew"].as_bool().unwrap()), ("s", "account:s", true)); assert!(r["text"].as_str().unwrap().starts_with("Claude sync token Work sync ends ") && r["text"].as_str().unwrap().ends_with(": renew the sync token"), "{r}");
        assert!(renew_alert(&ps[1..], &none, "UTC", now).is_none(), "more than a month left: nothing yet");
        assert!(renew_alert(&ps[1..], &none, "UTC", now + 171 * day).is_some(), "a month ahead of its end");
        assert!(renew_alert(&ps, &|_: &str| r.to_string(), "UTC", now).is_none(), "once");
        assert!(renew_alert(&ps, &none, "UTC", now + 21 * day).unwrap()["text"].as_str().unwrap().contains("has ended"));
        assert!(!renew_over(&r.to_string(), &ps, now), "still due");
        let renewed = vec![json!({"id":"s","kind":"sync","state":"connected","tokenExpiresAt":now + 365 * day})]; assert!(renew_over(&r.to_string(), &renewed, now), "renewed: over");
        assert!(renew_over(&r.to_string(), &[], now), "removed: over"); assert!(!renew_over(&json!({"profile":"w","text":"x"}).to_string(), &[], now), "a sign-in alert is not this one's to clear");
        let dead = vec![json!({"id":"w","provider":"claude","state":"expired","epoch":2,"label":"Work"})];
        assert!(signin_alert(&dead, &|k: &str| if k == alert_key("claude") { r.to_string() } else { String::new() }, now, "w").is_some(), "a dead sign-in takes the alert over a reminder");
    }
    #[test] fn the_default_claude_allowance_is_kept_as_a_setting_once() {
        let s = json!({"default_allowance":{"provider":"claude","source":"claude-cli-usage","attempted_at":5,"windows":[]}});
        let e = allowance(&|_| String::new(), &s, 10, 10).unwrap(); assert_eq!(e["key"], "allowance.claude");
        assert!(allowance(&|_| s["default_allowance"].to_string(), &s, 10, 10).is_none()); assert!(allowance(&|_| String::new(), &s, 4, 4).is_none(), "an attempt from the future is not kept"); assert!(allowance(&|_| String::new(), &json!({}), 10, 10).is_none());
    }
}

//! The hub side of accounts (hub.bend `Hub.accounts.*`, server.bend `AccountsJob`, core/accounts.bend `Accounts.public/changes/created`): the `accounts` request, the
//! live snapshot every client is told (info `accounts.live`), the inventory kept in the log (setting `accounts.inventory`, so a restart still knows the profiles' ids and
//! epochs), a default that follows its profile away, and the threads whose account's identity changed or disconnected: their running turn is stopped, with the reason.
//! The JS hub offers `accounts*` only to a client the hub knows to be an account-capable web view (thin mode `view-web` and a current `accountProfiles` version, kept per
//! client). Here a connection is granted by a request in its view session that names the accounts wire version (`accountProfiles: 1`, as the web view sends it; `Hub::request`
//! calls `note_request`), the grant is kept until the connection detaches, and every other client gets the named-account message. The live profiles are not broadcast:
//! a granted client reads them with the `accounts` request (`op: list`), which is how the settings page gets them.
use super::{accounts, accounts_io::Accounts};
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
    if o["provider"] == "claude" && o["source"] == "claude-cli-usage" && o["attempted_at"].as_u64().unwrap_or(0) <= now && get("allowance.claude") != o.to_string() {
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

/// One snapshot that changed: the log, the clients, and the turns that lost their account.
pub async fn found(hub: &Arc<Hub>, agents: &Option<Arc<Agents>>, snapshot: &Value) {
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
        let _ = hub.commit_many(&[json!({"$":"TurnChanged","thread":id,"state":"interrupted","at":at}),
            json!({"$":"ActivityLogged","thread":id,"act":format!("account-retired:{at}"),"tone":"error","kind":"account","summary":"This account's verified identity changed or disconnected. Check status and explicitly select an account to continue.","at":at})]).await;
        if let Some(a) = agents { let _ = a.stop(&id).await; }
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
pub struct Busy(Arc<Accounts>, String);
impl Busy { pub fn new(m: Arc<Accounts>, id: &str) -> Busy { m.child_started(id); Busy(m, id.to_string()) } }
impl Drop for Busy { fn drop(&mut self) { self.0.child_ended(&self.1) } }

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
            if matches!(op, "remove" | "login" | "disconnect") && active(hub, p.get("profile").and_then(Value::as_str).unwrap_or("")).await {
                return vec![json!({"t":"reply","id":id,"ok":false,"error":"Stop this account's active turns before changing its login."})]
            }
            let mut snap = self.mgr.request(p).await;
            // a client that names a thread is also told what the thread is bound to (the account chip and its menu need the binding to send `accounts.select` with `expected`)
            snap["defaults"] = json!({"claude": hub.setting("account.default.claude").await.unwrap_or_default(), "codex": hub.setting("account.default.codex").await.unwrap_or_default()});
            if let Some(th) = p.get("thread").and_then(Value::as_str).map(|t| t.rsplit('|').next().unwrap_or(t)).filter(|t| !t.is_empty()) { // a picture names a thread `hub|id`
                let provider = { let m = hub.model().await; m.threads.get(th).map(|t| t.provider.clone()).unwrap_or_default() };
                let raw = if accounts::supported(&provider) { hub.setting(&accounts::binding_key(th, &provider)).await.unwrap_or_default() } else { String::new() };
                snap["thread"] = json!({"id": th, "provider": provider, "binding": raw});
            }
            vec![json!({"t":"reply","id":id,"ok":snap["error"].as_str().unwrap_or("").is_empty(),"result":snap})]
        })
    }
}

/// The manager for this hub's home (provider CLIs resolved like the agents'), its RPC, and the 1-second watch that tells everyone of a change.
pub fn start(hub: &Arc<Hub>, agents: Option<Arc<Agents>>) -> Arc<Accounts> { start_cli(hub, agents, crate::agents::proc::Cli::from_env()) }
/// `start` with the provider CLIs found by `cli` (a test gives it a folder of stand-ins).
pub fn start_cli(hub: &Arc<Hub>, agents: Option<Arc<Agents>>, cli: crate::agents::proc::Cli) -> Arc<Accounts> {
    let mgr = Arc::new(Accounts::open(&hub.cfg.home, Arc::new(move |p: &str| cli.resolve(p))));
    if let Some(a) = &agents { let _ = a.accounts.set(mgr.clone()); }
    hub.register_rpc(Arc::new(AccountsRpc::new(mgr.clone())));
    let (h, m) = (hub.clone(), mgr.clone());
    tokio::spawn(async move {
        let mut last = String::new();
        loop {
            // the background recheck of connected accounts: never on a copy of a log, never on a test hub (stand-in agents)
            if !crate::copy::on() && std::env::var_os("BACKPLANE_AGENTS").is_none() { let m2 = m.clone(); tokio::spawn(async move { m2.refresh().await }); }
            let snap = m.snapshot(); let text = snap.to_string();
            if text != last { last = text; found(&h, &agents, &snap).await; }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
    mgr
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snap(profiles: Value, error: &str) -> Value { json!({"version":1,"revision":3,"profiles":profiles,"error":error,"attempts":[{"id":"a","url":"https://claude.ai/x","userCode":"C"}]}) }
    async fn ask(rpc: &AccountsRpc, hub: &Arc<Hub>, who: &Caller) -> Value { rpc.call(hub, who, &json!(1), "accounts", &json!({"op":"list","accountProfiles":true})).await.remove(0) }
    #[tokio::test(flavor = "multi_thread")] async fn only_a_granted_client_may_manage_accounts() {
        let home = std::env::temp_dir().join(format!("bp-acc-hub-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let hub = Hub::start(crate::hub::Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
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
    #[test] fn the_default_claude_allowance_is_kept_as_a_setting_once() {
        let s = json!({"default_allowance":{"provider":"claude","source":"claude-cli-usage","attempted_at":5,"windows":[]}});
        let e = allowance(&|_| String::new(), &s, 10, 10).unwrap(); assert_eq!(e["key"], "allowance.claude");
        assert!(allowance(&|_| s["default_allowance"].to_string(), &s, 10, 10).is_none()); assert!(allowance(&|_| String::new(), &s, 4, 4).is_none(), "an attempt from the future is not kept"); assert!(allowance(&|_| String::new(), &json!({}), 10, 10).is_none());
    }
}

//! Accounts (core/accounts.bend, server.bend `AccountsRpc.default.step`): a thread may be bound to a named login ("profile") of a provider
//! (claude, codex); an empty binding is the CLI's own login and keeps every legacy session key and launch behavior. Accounts are settings
//! (`accounts.inventory`, `account.default.<provider>`, `account.thread.<thread>.<provider>`), not part of the thread's state.
//!
//! PORTED: the pure rules (which profile is ready, binding text, the bindings a new or forked thread freezes, the label and the unavailable
//! reasons), and `accounts.default` {provider, profile, epoch}: make a connected profile the default for new threads.
//! NOT PORTED, and why: `accounts` (status, login, disconnect, remove, refresh: it runs the provider CLIs' login and identity probes, in private
//! profile folders, and keeps a live snapshot of attempts and URLs), `accounts.select` (switches a thread's session between accounts: it needs the
//! agents' session handling and a history count) and the fallback runtime (`AccountFallback`: availability holds on a confirmed limit, clearing a
//! hold). They need real provider CLIs, which no test here may run; with no profiles the hub is on legacy bindings everywhere, which is the
//! JS hub's behavior with no profile too. The settings they would write are folded and kept as they are.
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::sync::Arc;

pub fn supported(provider: &str) -> bool { provider == "claude" || provider == "codex" }
pub fn binding_key(thread: &str, provider: &str) -> String { format!("account.thread.{thread}.{provider}") }
/// Accounts.binding.show
pub fn binding_show(id: &str, epoch: u64) -> String { json!({"id":id,"epoch":epoch}).to_string() }
/// Tuple components have separate length prefixes; labels never enter a key (Accounts.part).
fn part(s: &str) -> String { format!("{}:{}", s.chars().count(), s) }
/// Accounts.session.key / Accounts.seen.key: the native session, and how much of the conversation the account has seen, per thread, provider, profile and epoch.
pub fn session_key(th: &str, provider: &str, id: &str, epoch: u64) -> String { format!("account.session.{}{}{}{}", part(th), part(provider), part(id), epoch) }
pub fn seen_key(th: &str, provider: &str, id: &str, epoch: u64) -> String { format!("account.seen.{}{}{}{}", part(th), part(provider), part(id), epoch) }
/// Accounts.profile: the inventory's profile of that id (Null when none).
pub fn profile(inventory: &str, id: &str) -> Value {
    serde_json::from_str::<Value>(inventory).ok().and_then(|v| v["profiles"].as_array().cloned()).unwrap_or_default().into_iter().find(|p| p["id"].as_str() == Some(id)).unwrap_or(Value::Null)
}
/// Accounts.ready: the profile is the provider's, connected, and still at the epoch the caller saw.
pub fn ready(p: &Value, provider: &str, epoch: u64) -> bool { p["provider"] == provider && p["state"] == "connected" && p["epoch"].as_u64().unwrap_or(0) == epoch }
/// Accounts.allowed: a legacy (empty) binding always; a named one only while its profile is ready at the bound epoch.
pub fn allowed(id: &str, p: &Value, provider: &str, epoch: u64) -> bool { id.is_empty() || ready(p, provider, epoch) }
/// Accounts.label / Accounts.short
pub fn label(id: &str, p: &Value) -> String { if id.is_empty() { "Default account".into() } else { p["label"].as_str().unwrap_or("Unavailable account").to_string() } }
pub fn short(label: &str) -> String { if label == "Default account" { "Default".into() } else { label.into() } }
/// Accounts.unavailable: why a profile cannot be chosen.
pub fn unavailable(p: &Value, epoch: u64) -> &'static str {
    let state = p["state"].as_str().unwrap_or("");
    if p["id"].as_str().unwrap_or("").is_empty() { "This saved account is no longer available. Choose another account." }
    else if state == "unchecked" { "This saved sign-in has not been checked yet. Reopen the account menu to refresh it." }
    else if state == "connecting" { "Finish this account's sign-in in Settings → Agents before selecting it." }
    else if state != "connected" { "This account is not connected. Use Status / Refresh or Connect / Login in Settings → Agents." }
    else if p["epoch"].as_u64().unwrap_or(0) != epoch { "This account's sign-in changed. Reopen the account menu and select it again." }
    else { "The thread or account selection changed. Reopen the account menu and try again." }
}
/// Accounts.created: the bindings a new or forked thread freezes (old threads with none stay legacy). `get` reads a setting.
pub fn created(change: &Value, get: &dyn Fn(&str) -> String) -> Vec<Value> {
    let legacy = binding_show("", 0);
    let set = |k: String, v: String, at: u64| json!({"$":"SettingSet","key":k,"value":v,"at":at});
    match change["$"].as_str() {
        Some("ThreadCreated") if supported(change["provider"].as_str().unwrap_or("")) => {
            let (id, p, at) = (change["id"].as_str().unwrap_or(""), change["provider"].as_str().unwrap_or(""), change["at"].as_u64().unwrap_or(0));
            let d = get(&format!("account.default.{p}")); vec![set(binding_key(id, p), if d.is_empty() { legacy } else { d }, at)]
        }
        Some("ThreadForked") => {
            let (id, src, at) = (change["id"].as_str().unwrap_or(""), change["source"].as_str().unwrap_or(""), change["at"].as_u64().unwrap_or(0));
            ["claude", "codex"].iter().map(|p| { let b = get(&binding_key(src, p)); set(binding_key(id, p), if b.is_empty() { legacy.clone() } else { b }, at) }).collect()
        }
        _ => vec![],
    }
}

/// `accounts.default`. In the JS hub every `accounts*` request from a client that has not declared the accounts capability is refused with the named-account
/// message (server.bend `Hub.rpc.account.client`, `Accounts.client.allowed`); only the web renderer's view requests declare it (`accountProfiles`), and phones and
/// the GPUI client never do. Here a connection is granted when a request in its view session names `accountProfiles: 1` (accounts_hub.rs `note_request`, revoked at detach):
/// granted, `make_default` answers; any other client gets the refusal below.
pub struct AccountsDefault;
pub const GATE: &str = "This thread uses a named account. Use updated Backplane web or desktop to continue; your draft is preserved.";
impl RpcHandler for AccountsDefault {
    fn methods(&self) -> &'static [&'static str] { &["accounts.default"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if !super::accounts_hub::is_granted(who) { return vec![json!({"t":"reply","id":id,"ok":false,"error":GATE})] }
            vec![make_default(hub, id, p).await]
        })
    }
}
/// AccountsRpc.default.step: make a connected profile (or the CLI's own login) the default for new threads.
pub async fn make_default(hub: &Arc<Hub>, id: &Value, p: &Value) -> Value {
    let g = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let (provider, prof, epoch) = (g("provider"), g("profile"), p.get("epoch").and_then(Value::as_u64).unwrap_or(0));
    let inventory = hub.setting("accounts.inventory").await.unwrap_or_default();
    let ok = supported(&provider) && ((prof.is_empty() && epoch == 0) || ready(&profile(&inventory, &prof), &provider, epoch));
    if !ok { return json!({"t":"reply","id":id,"ok":false,"error":"Check and reconnect this account before making it the default."}) }
    match hub.commit(&json!({"$":"SettingSet","key":format!("account.default.{provider}"),"value":binding_show(&prof, epoch),"at":now_ms() / 1000})).await {
        Ok(_) => json!({"t":"reply","id":id,"ok":true}), Err(e) => json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};
    fn inv() -> String { json!({"version":1,"revision":1,"profiles":[{"id":"w","provider":"claude","state":"connected","epoch":3,"label":"Work"},{"id":"x","provider":"codex","state":"connecting","epoch":1,"label":"X"}]}).to_string() }
    #[test] fn accounts_legacy_allowed() { assert_eq!(binding_show("", 0), r#"{"id":"","epoch":0}"#); assert!(allowed("", &Value::Null, "claude", 0), "no binding: always allowed"); assert!(!allowed("zz", &Value::Null, "claude", 0)); assert!(supported("claude") && supported("codex") && !supported("grok")); }
    #[test] fn a_profile_is_ready_when_connected_at_the_epoch_the_caller_saw() {
        let w = profile(&inv(), "w"); assert!(ready(&w, "claude", 3)); assert!(!ready(&w, "claude", 2), "its sign-in changed"); assert!(!ready(&w, "codex", 3)); assert!(!ready(&profile(&inv(), "x"), "codex", 1)); assert!(!ready(&profile(&inv(), "nope"), "claude", 0));
        assert_eq!(unavailable(&Value::Null, 0), "This saved account is no longer available. Choose another account."); assert_eq!(unavailable(&profile(&inv(), "x"), 1), "Finish this account's sign-in in Settings → Agents before selecting it.");
        assert_eq!(unavailable(&w, 2), "This account's sign-in changed. Reopen the account menu and select it again."); assert_eq!(short(&label("", &Value::Null)), "Default"); assert_eq!(label("zz", &Value::Null), "Unavailable account");
    }
    #[test] fn a_new_thread_freezes_the_default_and_a_fork_copies_its_source() {
        let get = |k: &str| match k { "account.default.claude" => binding_show("w", 3), "account.thread.src.claude" => binding_show("w", 3), _ => String::new() };
        let c = created(&json!({"$":"ThreadCreated","id":"t1","provider":"claude","at":5}), &get); assert_eq!(c.len(), 1); assert_eq!((c[0]["key"].clone(), c[0]["value"].clone()), (json!("account.thread.t1.claude"), json!(binding_show("w", 3))));
        assert_eq!(created(&json!({"$":"ThreadCreated","id":"t2","provider":"codex","at":5}), &get)[0]["value"], json!(binding_show("", 0)), "no default: legacy");
        assert!(created(&json!({"$":"ThreadCreated","id":"t3","provider":"grok","at":5}), &get).is_empty());
        let f = created(&json!({"$":"ThreadForked","id":"t4","source":"src","at":6}), &get); assert_eq!(f.len(), 2); assert_eq!(f[0]["value"], json!(binding_show("w", 3))); assert_eq!(f[1]["value"], json!(binding_show("", 0)));
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_client_without_the_accounts_capability_is_refused_as_the_js_hub_refuses_it() {
        let home = std::env::temp_dir().join(format!("bp-acct2-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let r = AccountsDefault.call(&hub, &Caller { gen: "1".into(), client: 1 }, &json!(2), "accounts.default", &json!({"provider":"claude","profile":"","epoch":0})).await.remove(0);
        assert_eq!(r["error"], GATE); assert_eq!(hub.setting("account.default.claude").await, None); let _ = std::fs::remove_dir_all(&home);
    }
    #[tokio::test(flavor = "multi_thread")] async fn only_a_connected_profile_or_the_legacy_login_becomes_the_default() {
        let home = std::env::temp_dir().join(format!("bp-acct-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        hub.commit(&json!({"$":"SettingSet","key":"accounts.inventory","value":inv(),"at":1})).await.unwrap();
        let call = |p: Value| { let hub = hub.clone(); async move { make_default(&hub, &json!(2), &p).await } };
        let bad = "Check and reconnect this account before making it the default.";
        assert_eq!(call(json!({"provider":"claude","profile":"x","epoch":1})).await["error"], bad, "not that provider's"); assert_eq!(call(json!({"provider":"claude","profile":"w","epoch":2})).await["error"], bad, "stale epoch"); assert_eq!(call(json!({"provider":"grok","profile":"","epoch":0})).await["error"], bad);
        assert_eq!(call(json!({"provider":"claude","profile":"w","epoch":3})).await["ok"], true); assert_eq!(hub.setting("account.default.claude").await.as_deref(), Some(binding_show("w", 3).as_str()));
        assert_eq!(call(json!({"provider":"claude","profile":"","epoch":0})).await["ok"], true); assert_eq!(hub.setting("account.default.claude").await.as_deref(), Some(r#"{"id":"","epoch":0}"#), "back to the CLI's own login");
        let _ = std::fs::remove_dir_all(&home);
    }
}

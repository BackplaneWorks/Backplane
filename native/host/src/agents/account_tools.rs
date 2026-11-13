//! Accounts for agents and bots: `account_list` (each account with what it has left) and the `account` argument of thread_launch, delegate_task and thread_update, by id, email or label.
//! The person's own choice stands: a thread they pinned (settings/accounts.rs `pinned_to`) is moved only when their own latest message names the account. An agent cannot write that
//! message for them: what it sends to a thread carries a `MessageFrom` that says so. Who chose is kept beside the binding and said on the timeline.
use super::{lines::s, now_s, Agents};
use crate::settings::{accounts as ac, fallback as fb};
use serde_json::{json, Value};
use std::sync::Arc;

fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }
/// An account named for a new thread: the provider it is of, the binding, and what the timeline calls it.
#[derive(Debug, Clone, PartialEq)]
pub struct Chosen { pub provider: String, pub id: String, pub epoch: u64, pub label: String }

/// What `account_list` says of one login. `get` reads a setting. No credential or store path: label, email, plan, state and the provider's own windows.
/// `all`: every profile (a login's holds count for the others of its identity, `fb::same_logins`).
pub fn account_json(p: &Value, all: &[Value], get: &dyn Fn(&str) -> String, now: u64, model: &str, current: Option<(&str, u64)>, default: bool) -> Value {
    let (id, provider, epoch) = (p["id"].as_str().unwrap_or(""), p["provider"].as_str().unwrap_or(""), p["epoch"].as_u64().unwrap_or(0));
    let held = fb::login_held(get, all, provider, id, epoch, model, now);
    let until = |i: &str, e: u64| if fb::hold_current(get, provider, i, e, model, now) { fb::hold_until(get, provider, i, e, model) } else { 0 };
    let held_until = std::iter::once((id.to_string(), epoch)).chain(fb::same_logins(all, provider, id, epoch)).map(|(i, e)| until(&i, e)).max().unwrap_or(0);
    let windows: Vec<Value> = p["allowance"]["windows"].as_array().into_iter().flatten().map(|w| json!({"scope":w["scope"],"usedPercent":w["used_percent"],"resetsAt":w["resets_at"]})).collect();
    json!({"id":id,"label":p["label"].as_str().unwrap_or(if id.is_empty() { "Default account" } else { "" }),"provider":provider,"email":p["identity"]["email"],"plan":p["identity"]["plan"],"state":p["state"],
        "remainingPercent":fb::usage_score(p, now, model).map(|x| x as f64 / 1000.0),"held":held,"heldUntil":if held { held_until } else { 0 },"windows":windows,
        "current":current == Some((id, epoch)),"default":default,"sameAsDefault":p["sameAsDefault"] == true})
}

impl Agents {
    /// The accounts as they are now (the manager's snapshot; the log's copy on a hub without one).
    async fn accounts_now(&self) -> Vec<Value> { self.accounts_snapshot().await.0 }
    /// The profiles and who each provider's CLI login is (`default_login`).
    async fn accounts_snapshot(&self) -> (Vec<Value>, Value) {
        if let Some(m) = self.accounts.get() { let s = m.snapshot(); return (s["profiles"].as_array().cloned().unwrap_or_default(), s["default_login"].clone()) }
        let v = serde_json::from_str::<Value>(&self.hub.setting("accounts.inventory").await.unwrap_or_default()).unwrap_or(Value::Null);
        (v["profiles"].as_array().cloned().unwrap_or_default(), v["defaultLogin"].clone())
    }
    /// Who is choosing: a bot by its name, else the agent of a thread, named by the thread.
    pub(crate) async fn chooser(&self, thread: &str) -> (String, String) {
        let w = self.world().await; let bot = w.bot(thread);
        if !bot.is_empty() { let name = w.bots.get(bot).map(|b| b.0.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| bot.to_string()); return ("bot".into(), name) }
        ("agent".into(), format!("The agent of \"{}\"", w.threads.get(thread).map(|t| t.title.clone()).unwrap_or_else(|| thread.to_string())))
    }
    /// `account` of a tool call: None when it names none. A saved sign-in not checked since the hub started is checked now, no model turn.
    pub(crate) async fn account_arg(&self, a: &Value, provider: &str) -> Result<Option<Chosen>, String> {
        let name = s(a, "account"); if name.trim().is_empty() { return Ok(None) }
        let profiles = self.accounts_now().await;
        let Some(p) = ac::find(&profiles, provider, name)? else {
            if !ac::supported(provider) { return Err("name the provider too: the default account is one per provider (claude, codex)".into()) }
            return Ok(Some(Chosen { provider: provider.to_string(), id: String::new(), epoch: 0, label: "Default account".into() }))
        };
        let (id, prov) = (p["id"].as_str().unwrap_or("").to_string(), p["provider"].as_str().unwrap_or("").to_string());
        let mut p = p.clone();
        if p["state"] == "unchecked" { if let Some(m) = self.accounts.get() { let snap = m.request(&json!({"op":"status","profile":id})).await; if let Some(x) = snap["profiles"].as_array().and_then(|v| v.iter().find(|x| x["id"] == id.as_str())) { p = x.clone() } } }
        let epoch = p["epoch"].as_u64().unwrap_or(0);
        if !ac::ready(&p, &prov, epoch) { return Err(ac::unavailable(&p, epoch).to_string()) }
        Ok(Some(Chosen { provider: prov, id, epoch, label: p["label"].as_str().unwrap_or("account").to_string() }))
    }
    /// The events a new thread's account takes: the one named, recorded with who named it, else the provider's default frozen as for any new thread.
    pub(crate) async fn thread_account(&self, launcher: &str, th: &str, provider: &str, chosen: Option<&Chosen>, at: u64) -> Vec<Value> {
        if !ac::supported(provider) { return vec![] }
        let set = |k: String, v: String| json!({"$":"SettingSet","key":k,"value":v,"at":at});
        let Some(c) = chosen else {
            let d = self.hub.setting(&format!("account.default.{provider}")).await.filter(|d| !d.is_empty()).unwrap_or_else(|| ac::binding_show("", 0));
            return vec![set(ac::binding_key(th, provider), d)]
        };
        let (by, who) = self.chooser(launcher).await;
        vec![set(ac::binding_key(th, provider), ac::binding_show(&c.id, c.epoch)), set(ac::chosen_key(th, provider), ac::chosen_show(&c.id, c.epoch, &by, &who, at)),
            json!({"$":"ActivityLogged","thread":th,"act":format!("account{at}-{th}"),"tone":"info","kind":"account","summary":format!("{who} started this thread on {}", c.label),"at":at})]
    }
    /// The newest message in `thread` that a person wrote, by the log's own record: what any thread, bot or the hub sends there carries a `MessageFrom` of another kind.
    async fn person_said(&self, thread: &str) -> String {
        let rows: Vec<_> = { let m = self.hub.model().await; m.page(&self.hub.store, thread, 0, 64).map(|p| p.rows).unwrap_or_default() };
        let mut rows: Vec<_> = rows.into_iter().filter(|r| r.kind == "msg" && r.role == "user" && r.thread == thread && r.src.is_empty()).collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.seq));
        for r in rows {
            let near = self.hub.store.range(r.seq as usize, r.seq as usize + 3).unwrap_or_default();
            let from = near.iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|e| e["$"] == "MessageFrom" && e["msg"] == r.id.as_str());
            if from.map_or(true, |e| e["kind"] == "person") { return r.text }
        }
        String::new()
    }
    /// thread_update {account}: `accounts.choose`, the same step as the person's own pick, as this caller.
    pub(crate) async fn tool_update_account(&self, thread: &str, target: &str, a: &Value) -> (bool, Value) {
        let provider = { let w = self.world().await; match w.threads.get(target) { Some(t) => t.provider.clone(), None => return fail("No such thread.") } };
        if !ac::supported(&provider) { return fail(&format!("{provider} threads have no named accounts")) }
        let c = match self.account_arg(a, &provider).await { Ok(Some(c)) => c, Ok(None) => return fail("name the account"), Err(e) => return fail(&e) };
        let profiles = self.accounts_now().await; let p = profiles.iter().find(|p| p["id"] == c.id.as_str()).cloned().unwrap_or(json!({"label":"default"}));
        // the person may ask in the thread itself, or in the one that is asking on their behalf
        let asked = ac::asked_for(&self.person_said(target).await, &p) || (thread != target && ac::asked_for(&self.person_said(thread).await, &p));
        let (by, who) = self.chooser(thread).await;
        let Some(h) = self.hub.rpc_for("accounts.choose") else { return fail("accounts are not available") };
        let r = h.call(&self.hub, &crate::api::Caller { gen: "agents".into(), client: 0 }, &json!(0), "accounts.choose", &json!({"thread":target,"profile":c.id,"epoch":c.epoch,"by":by,"who":who,"asked":asked})).await.into_iter().next().unwrap_or(Value::Null);
        if r["ok"] == true { (true, json!({"threadId":target,"account":c.label,"provider":c.provider,"by":by})) } else { fail(r["error"].as_str().unwrap_or("the account could not be selected")) }
    }
    /// The tool account_list {provider?, threadId?}: every login with what it has left, and which of them `threadId` (default: this thread) is on.
    pub async fn tool_accounts(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let target = if s(a, "threadId").is_empty() { thread } else { s(a, "threadId") };
        let (tprov, model) = { let w = self.world().await; w.threads.get(target).map(|t| (t.provider.clone(), t.model.clone())).unwrap_or_default() };
        let binding: Value = serde_json::from_str(&self.hub.setting(&ac::binding_key(target, &tprov)).await.unwrap_or_default()).unwrap_or(json!({"id":"","epoch":0}));
        let current = (binding["id"].as_str().unwrap_or("").to_string(), binding["epoch"].as_u64().unwrap_or(0));
        let chosen: Value = serde_json::from_str(&self.hub.setting(&ac::chosen_key(target, &tprov)).await.unwrap_or_default()).unwrap_or(Value::Null);
        let mut settings = std::collections::HashMap::new();
        let keys: Vec<String> = { let m = self.hub.model().await; m.settings_with_prefix("account.limit.").map(|(k, _)| k.to_string()).collect() };
        for k in keys.into_iter().chain(["account.default.claude".to_string(), "account.default.codex".to_string()]) { if let Some(v) = self.hub.setting(&k).await { settings.insert(k, v); } }
        let get = |k: &str| settings.get(k).cloned().unwrap_or_default();
        let is_default = |p: &str, id: &str, e: u64| serde_json::from_str::<Value>(&get(&format!("account.default.{p}"))).map_or(id.is_empty(), |d| d["id"].as_str().unwrap_or("") == id && d["epoch"].as_u64().unwrap_or(0) == e);
        let want = s(a, "provider"); let now = now_s();
        let mut out = vec![]; let (all, logins) = self.accounts_snapshot().await;
        for prov in ["claude", "codex"] { // the CLI's own login first, as the menus list it; its windows are kept for Claude only
            if !want.is_empty() && want != prov { continue }
            let al: Value = if prov == "claude" { serde_json::from_str(&self.hub.setting("allowance.claude").await.unwrap_or_default()).unwrap_or(Value::Null) } else { Value::Null };
            // the CLI's own login says who it is when its CLI could; with no reading of its own, an added account of the same identity has the same windows
            let al = if al.is_null() { all.iter().find(|p| p["provider"] == prov && p["sameAsDefault"] == true).map(|p| p["allowance"].clone()).unwrap_or(Value::Null) } else { al };
            let who = &logins[prov];
            let own = json!({"id":"","epoch":0,"provider":prov,"label":"Default account","state":"connected","allowance":al,"identity":{"email":who["email"],"plan":who["plan"]}});
            let mut row = account_json(&own, &all, &get, now, &model, (prov == tprov).then_some(("", 0)).filter(|_| current.0.is_empty()), is_default(prov, "", 0));
            if who["state"] != "known" { row["whoUnknown"] = json!(who["reason"].as_str().unwrap_or(if who["state"] == "signed-out" { "the CLI is signed out" } else { "not checked yet" })) }
            out.push(row);
        }
        for p in all.iter() {
            let prov = p["provider"].as_str().unwrap_or(""); if !want.is_empty() && want != prov { continue }
            let on = (prov == tprov).then_some((current.0.as_str(), current.1));
            out.push(account_json(p, &all, &get, now, &model, on, is_default(prov, p["id"].as_str().unwrap_or(""), p["epoch"].as_u64().unwrap_or(0))));
        }
        (true, json!({"accounts":out,"threadId":target,"pinnedByPerson":chosen["by"] == "person"}))
    }
}

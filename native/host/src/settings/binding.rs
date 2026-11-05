//! Accounts.created for every writer (server.bend runs it on every step): a new thread freezes the default account of its provider as
//! `account.thread.<id>.<provider>`, and a fork copies its source's bindings. One `Transform` (`before_commit`) so a thread made by a command, a bot, a launch or a
//! task all get it; a batch that already carries a thread's binding is left alone, so the callers that add it themselves keep working and can be removed later.
//!
//! The Transform is synchronous, so it reads the settings it needs (`account.default.*`, `account.thread.*`) from a mirror kept by an Observer over committed
//! batches and filled once at start from the log; within a batch, earlier events count (a default set and a thread created in one commit).
use super::accounts::{binding_key, created};
use crate::{api::{Observer, Transform}, hub::Hub};
use serde_json::Value;
use std::{collections::HashMap, sync::{Arc, Mutex}};

#[derive(Default)]
pub struct Bindings { vals: Mutex<HashMap<String, String>> }
fn mine(k: &str) -> bool { k.starts_with("account.default.") || k.starts_with("account.thread.") }

impl Bindings {
    pub fn new() -> Arc<Self> { Arc::new(Self::default()) }
    /// Fill from the log without overwriting what a committed batch already told the mirror.
    pub async fn seed(&self, hub: &Arc<Hub>) {
        let keys: Vec<String> = { let m = hub.model().await; ["account.default.", "account.thread."].iter().flat_map(|p| m.settings_with_prefix(p).map(|(k, _)| k.to_string()).collect::<Vec<_>>()).collect() };
        for k in keys { if let Some(v) = hub.setting(&k).await { self.vals.lock().unwrap().entry(k).or_insert(v); } }
    }
    fn apply(&self, events: &[Value]) { let mut g = self.vals.lock().unwrap(); for e in events { if e["$"] == "SettingSet" { let k = e["key"].as_str().unwrap_or(""); if mine(k) { g.insert(k.to_string(), e["value"].as_str().unwrap_or("").to_string()); } } } }
    /// The batch with each new or forked thread's bindings added right after the event that made it (unless the batch has them).
    pub fn bind(&self, events: Vec<Value>) -> Vec<Value> {
        if !events.iter().any(|e| matches!(e["$"].as_str(), Some("ThreadCreated" | "ThreadForked"))) { return events }
        let mut seen: HashMap<String, String> = HashMap::new();
        let has: std::collections::HashSet<String> = events.iter().filter(|e| e["$"] == "SettingSet").filter_map(|e| e["key"].as_str().map(str::to_string)).collect();
        let mut out = Vec::with_capacity(events.len() + 2);
        for e in events {
            if e["$"] == "SettingSet" { let k = e["key"].as_str().unwrap_or(""); if mine(k) { seen.insert(k.to_string(), e["value"].as_str().unwrap_or("").to_string()); } }
            let add = if matches!(e["$"].as_str(), Some("ThreadCreated" | "ThreadForked")) {
                let g = self.vals.lock().unwrap();
                let get = |k: &str| seen.get(k).or_else(|| g.get(k)).cloned().unwrap_or_default();
                created(&e, &get).into_iter().filter(|s| !has.contains(s["key"].as_str().unwrap_or(""))).collect::<Vec<_>>()
            } else { vec![] };
            out.push(e);
            for s in add { seen.insert(s["key"].as_str().unwrap_or("").to_string(), s["value"].as_str().unwrap_or("").to_string()); out.push(s); }
        }
        out
    }
}
impl Transform for Bindings {
    fn before_commit(&self, _first_seq: u64, events: Vec<Value>) -> Vec<Value> { self.bind(events) }
    fn commit_failed(&self, _first_seq: u64) {}
}
impl Observer for Bindings {
    fn committed(&self, _hub: &Arc<Hub>, events: &[Value], _first: u32) { self.apply(events) }
}
/// The key a thread's binding for `provider` lives at (re-exported for tests).
pub fn key(id: &str, provider: &str) -> String { binding_key(id, provider) }

#[cfg(test)]
mod tests {
    use super::*; use serde_json::json;
    fn with(defaults: &[(&str, &str)]) -> Arc<Bindings> { let b = Bindings::new(); for (k, v) in defaults { b.vals.lock().unwrap().insert(k.to_string(), v.to_string()); } b }
    fn keys(v: &[Value]) -> Vec<String> { v.iter().map(|e| e["key"].as_str().or(e["$"].as_str()).unwrap_or("").to_string()).collect() }
    #[test] fn a_new_thread_from_any_writer_freezes_the_default_right_after_its_event() {
        let b = with(&[("account.default.claude", r#"{"id":"w","epoch":2}"#)]);
        let out = b.bind(vec![json!({"$":"ThreadCreated","id":"t1","provider":"claude","at":5}), json!({"$":"MessagePosted","thread":"t1"})]);
        assert_eq!(keys(&out), ["ThreadCreated", "account.thread.t1.claude", "MessagePosted"]); assert_eq!(out[1]["value"], r#"{"id":"w","epoch":2}"#);
    }
    #[test] fn a_fork_copies_its_source_for_both_providers() {
        let b = with(&[("account.thread.src.claude", r#"{"id":"w","epoch":3}"#)]);
        let out = b.bind(vec![json!({"$":"ThreadForked","id":"f1","source":"src","at":5})]);
        assert_eq!(keys(&out), ["ThreadForked", "account.thread.f1.claude", "account.thread.f1.codex"]);
        assert_eq!((out[1]["value"].clone(), out[2]["value"].clone()), (json!(r#"{"id":"w","epoch":3}"#), json!(r#"{"id":"","epoch":0}"#)), "no binding on the source: legacy");
    }
    #[test] fn a_batch_that_already_carries_the_binding_is_left_alone_and_the_batch_itself_counts() {
        let b = with(&[]);
        let own = vec![json!({"$":"ThreadCreated","id":"t1","provider":"claude","at":5}), json!({"$":"SettingSet","key":"account.thread.t1.claude","value":"x","at":5})];
        assert_eq!(b.bind(own.clone()), own, "nothing added twice");
        let out = b.bind(vec![json!({"$":"SettingSet","key":"account.default.claude","value":"D","at":5}), json!({"$":"ThreadCreated","id":"t2","provider":"claude","at":5})]);
        assert_eq!(out[2]["value"], "D", "a default set earlier in the batch is the one frozen");
        assert_eq!(b.bind(vec![json!({"$":"MessagePosted"})]), vec![json!({"$":"MessagePosted"})]);
    }
    #[test] fn the_mirror_follows_committed_batches_and_an_unsupported_provider_gets_nothing() {
        let b = with(&[]); b.apply(&[json!({"$":"SettingSet","key":"account.default.codex","value":"C","at":1}), json!({"$":"SettingSet","key":"other","value":"z","at":1})]);
        assert_eq!(b.bind(vec![json!({"$":"ThreadCreated","id":"t3","provider":"codex","at":2})])[1]["value"], "C");
        assert_eq!(b.bind(vec![json!({"$":"ThreadCreated","id":"t4","provider":"grok","at":2})]).len(), 1);
        assert!(!b.vals.lock().unwrap().contains_key("other"));
    }
    #[tokio::test(flavor = "multi_thread")] async fn a_launched_thread_and_a_fork_get_their_binding_through_a_real_commit() {
        use crate::{hub::Cfg, presenter::Canned};
        let home = std::env::temp_dir().join(format!("bp-bind-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let b = Bindings::new(); hub.register_transform(b.clone()); hub.register_observer(b.clone()); b.seed(&hub).await;
        hub.commit_many(&[json!({"$":"SettingSet","key":"account.default.claude","value":r#"{"id":"w","epoch":2}"#,"at":1})]).await.unwrap();
        // what agents/launch.rs and agents/tasks.rs commit: a bare ThreadCreated, no binding of their own
        hub.commit_many(&[json!({"$":"ThreadCreated","id":"launched","project":"p","provider":"claude","at":2})]).await.unwrap();
        assert_eq!(hub.setting("account.thread.launched.claude").await.as_deref(), Some(r#"{"id":"w","epoch":2}"#));
        // the default moves on: the launched thread keeps what it froze, and a fork of it copies that, not the new default
        hub.commit_many(&[json!({"$":"SettingSet","key":"account.default.claude","value":r#"{"id":"p","epoch":1}"#,"at":3})]).await.unwrap();
        hub.commit_many(&[json!({"$":"ThreadForked","id":"fork","source":"launched","at":4})]).await.unwrap();
        assert_eq!(hub.setting("account.thread.fork.claude").await.as_deref(), Some(r#"{"id":"w","epoch":2}"#));
        assert_eq!(hub.setting("account.thread.launched.claude").await.as_deref(), Some(r#"{"id":"w","epoch":2}"#));
        let _ = std::fs::remove_dir_all(&home);
    }
}

//! `setting.set` (core/hub.bend `Hub.setting`): a client writes one setting; the hub logs a SettingSet and every client hears it in the
//! log. `peer.own.*` is the hub's own; the telemetry keys go through the choice rules (telemetry.rs); `account.*` / `accounts.*` are
//! refused from a client (server.bend `AccountsRpc.reserved`: the account gate answers). An empty value removes the setting.
use super::telemetry;
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::sync::Arc;

pub const ACCOUNT_GATE: &str = "This thread uses a named account. Use updated Backplane web or desktop to continue; your draft is preserved.";

pub struct SettingSet;
impl RpcHandler for SettingSet {
    fn methods(&self) -> &'static [&'static str] { &["setting.set"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let (key, value) = (p.get("key").and_then(Value::as_str).unwrap_or(""), p.get("value").and_then(Value::as_str).unwrap_or(""));
            if key.starts_with("account.") || key.starts_with("accounts.") { return vec![json!({"t":"reply","id":id,"ok":false,"error":ACCOUNT_GATE})] }
            if key.starts_with("peer.own.") { return vec![json!({"t":"reply","id":id,"ok":false,"error":"that setting is the hub's own"})] }
            if telemetry::reserved(key) { return vec![telemetry::legacy(hub, id, key, value).await] }
            let at = now_ms() / 1000;
            match hub.commit(&json!({"$":"SettingSet","key":key,"value":value,"at":at})).await {
                Ok(_) => vec![json!({"t":"reply","id":id,"ok":true})],
                Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")})],
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};
    async fn start(home: &std::path::Path) -> Arc<Hub> {
        Hub::start(Cfg { home: home.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap()
    }
    async fn call(hub: &Arc<Hub>, m: &'static str, p: Value) -> Value {
        hub.register_rpc(Arc::new(SettingSet)); hub.register_rpc(Arc::new(telemetry::TelemetryRpc));
        let h = hub.rpc_for(m).unwrap(); h.call(hub, &Caller { gen: "1".into(), client: 1 }, &json!(5), m, &p).await.remove(0)
    }
    // the tour's Skip: the app sends setting.set onboard.done=1 (and onboard.uber=1); it must be answered, logged and still there after a restart
    #[tokio::test(flavor = "multi_thread")] async fn the_tours_skip_is_answered_logged_and_survives_a_restart() {
        let home = std::env::temp_dir().join(format!("bp-set-skip-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let hub = start(&home).await;
        for k in ["onboard.done", "onboard.uber"] { let r = call(&hub, "setting.set", json!({"key":k,"value":"1"})).await; assert_eq!(r, json!({"t":"reply","id":5,"ok":true})); }
        assert_eq!(hub.setting("onboard.done").await.as_deref(), Some("1"));
        let log = std::fs::read_to_string(home.join("events.jsonl")).unwrap();
        assert!(log.contains("\"key\":\"onboard.done\",\"value\":\"1\"") && log.contains("\"key\":\"onboard.uber\""), "{log}");
        drop(hub);
        let again = start(&home).await;
        assert_eq!(again.setting("onboard.done").await.as_deref(), Some("1"), "the tour does not come back");
        // the hub's own and the account keys are refused, not written
        assert_eq!(call(&again, "setting.set", json!({"key":"peer.own.x","value":"1"})).await["ok"], false);
        assert_eq!(call(&again, "setting.set", json!({"key":"account.thread.t1.claude","value":"x"})).await["error"], ACCOUNT_GATE);
        // an empty value removes
        call(&again, "setting.set", json!({"key":"onboard.done","value":""})).await; assert_eq!(again.setting("onboard.done").await, None);
        // the telemetry answer: Off applies at once and is remembered
        let r = call(&again, "telemetry.choice", json!({"value":"off","expected":"","decision":"d1","popup":true})).await;
        assert_eq!(r["ok"], true); assert_eq!(again.setting("telemetry.share").await.as_deref(), Some("off")); assert_eq!(again.setting("telemetry.asked").await.as_deref(), Some("1"));
        let _ = std::fs::remove_dir_all(&home);
    }
}

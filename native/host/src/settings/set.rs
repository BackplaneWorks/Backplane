//! `setting.set`: a client writes one setting; the hub logs a SettingSet and every client hears it in the
//! log. `peer.own.*` is the hub's own; the telemetry keys go through the choice rules (telemetry.rs); `account.*` / `accounts.*` are
//! refused from a client. An empty value removes the setting.
use super::telemetry;
use crate::{api::{Caller, Fut, RpcHandler}, hub::{now_ms, Hub}};
use serde_json::{json, Value};
use std::sync::Arc;

pub const ACCOUNT_GATE: &str = "This thread uses a named account. Use updated Backplane web or desktop to continue; your draft is preserved.";

pub struct SettingSet { pub agents: Option<Arc<crate::agents::Agents>> }
impl RpcHandler for SettingSet {
    fn methods(&self) -> &'static [&'static str] { &["setting.set"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let (key, value) = (p.get("key").and_then(Value::as_str).unwrap_or(""), p.get("value").and_then(Value::as_str).unwrap_or(""));
            if key.starts_with("account.") || key.starts_with("accounts.") { return vec![json!({"t":"reply","id":id,"ok":false,"error":ACCOUNT_GATE})] }
            if crate::push::reserved(key) { return vec![json!({"t":"reply","id":id,"ok":false,"error":"that setting is the hub's own"})] }
            // quiet hours are a window the push gate can read, or off; urgent is on or off: a value it could not read would silently turn the respect off
            if key == crate::push::QUIET && !(value.is_empty() || value == "off" || crate::push::window(value).is_some()) { return vec![json!({"t":"reply","id":id,"ok":false,"error":"quiet hours are HH:MM-HH:MM, or off"})] }
            if key == crate::push::QUIET_URGENT && !matches!(value, "" | "on" | "off") { return vec![json!({"t":"reply","id":id,"ok":false,"error":"urgent is on or off"})] }
            if key.starts_with("peer.own.") { return vec![json!({"t":"reply","id":id,"ok":false,"error":"that setting is the hub's own"})] }
            // a zone is only right once its rule is known: person.tz goes through zone.set (settings/zone.rs), which looks the rule up and refuses a name that is not a zone
            if key == "person.tz" { if let Some(z) = hub.rpc_for("zone.set") { return z.call(hub, _who, id, "zone.set", &json!({"zone": value})).await } }
            if telemetry::reserved(key) { return vec![telemetry::legacy(hub, id, key, value).await] }
            let at = now_ms() / 1000;
            let mut batch = vec![json!({"$":"SettingSet","key":key,"value":value,"at":at})];
            // a write to either fallback setting closes the open settings proposals in the same batch (Proposal.invalidate)
            if let Some(a) = &self.agents { let open: Vec<String> = a.world().await.asks.iter().filter(|(_, k)| k.tool == super::proposal::TOOL).map(|(i, _)| i.clone()).collect(); let extra = super::proposal::invalidate(&batch.clone(), &open, at); batch.extend(extra); batch = super::proposal::with_pending(a, batch, at).await; }
            match hub.commit_many(&batch).await {
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
        hub.register_rpc(Arc::new(SettingSet { agents: None })); hub.register_rpc(Arc::new(telemetry::TelemetryRpc));
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

    // setting.set person.tz is zone.set: the rule is looked up and stored with the zone, a name that is no zone is refused and nothing is written
    #[tokio::test(flavor = "multi_thread")] async fn setting_set_person_tz_resolves_the_rule_like_zone_set() {
        let home = std::env::temp_dir().join(format!("bp-set-tz-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let hub = start(&home).await; hub.register_rpc(Arc::new(super::super::zone::ZoneRpc));
        let r = call(&hub, "setting.set", json!({"key":"person.tz","value":"Not/AZone"})).await; assert_eq!(r["ok"], false, "{r}"); assert_eq!(hub.setting("person.tz").await, None);
        let r = call(&hub, "setting.set", json!({"key":"person.tz","value":"Asia/Tokyo"})).await;
        if r["ok"] == true { assert_eq!(hub.setting("person.tz").await.as_deref(), Some("Asia/Tokyo")); assert_eq!(hub.setting("tz.rule.Asia/Tokyo").await.as_deref(), Some("JST-9"), "the rule came with the zone") }
        else { assert!(r["error"].as_str().unwrap().contains("time zone"), "no zoneinfo on this machine: {r}"); assert_eq!(hub.setting("person.tz").await, None) }
        let r = call(&hub, "setting.set", json!({"key":"person.tz","value":""})).await; assert_eq!(r["ok"], true); assert_eq!(hub.setting("person.tz").await, None);
        let _ = std::fs::remove_dir_all(&home);
    }

    // slim's `private` scenario: a client writes a turn's input; the JS hub logs it whole and sends it blank
    #[tokio::test(flavor = "multi_thread")] async fn a_turn_input_written_by_a_client_is_logged_whole_and_goes_out_blank() {
        let home = std::env::temp_dir().join(format!("bp-set-input-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("events.jsonl"), "{\"$\":\"SettingSet\",\"key\":\"launch.input.t1\",\"value\":\"PRIVLAUNCH-0004 a turn input\",\"at\":1}\n").unwrap();
        let hub = start(&home).await;
        let mut rx = hub.frames.subscribe();
        for k in ["launch.input.t1", "fallback.input.t1", "ride.t1"] {
            let r = call(&hub, "setting.set", json!({"key":k,"value":format!("PRIVLIVE-0007 live for {k}")})).await; assert_eq!(r["ok"], true);
            assert_eq!(hub.setting(k).await.unwrap(), format!("PRIVLIVE-0007 live for {k}"), "the log keeps the value: {k}");
        }
        let log = std::fs::read_to_string(home.join("events.jsonl")).unwrap();
        assert_eq!(log.matches("PRIVLIVE-0007").count(), 3, "three events in the log, value whole: {log}");
        // a client's frame holds the value blank (Hub::wire_item)
        let line = log.lines().find(|l| l.contains("launch.input.t1") && l.contains("PRIVLIVE")).unwrap();
        assert_eq!(Hub::wire_item(line).unwrap()["value"], "", "blank on the wire");
        // clearing it is an event too, and an empty value over nothing is not 'no change' for these keys (they are kept blank, not removed, in the log)
        assert_eq!(call(&hub, "setting.set", json!({"key":"launch.input.t9","value":""})).await["ok"], true);
        assert!(std::fs::read_to_string(home.join("events.jsonl")).unwrap().contains("\"key\":\"launch.input.t9\",\"value\":\"\""), "an empty write is logged");
        let _ = rx.try_recv(); let _ = std::fs::remove_dir_all(&home);
    }
}

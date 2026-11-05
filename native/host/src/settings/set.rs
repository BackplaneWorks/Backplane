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

//! The settings lane: `setting.set`, the telemetry choice, API keys, the secret vault and the masks that keep a stored value out of every
//! event, reply and tool answer. Registers its handlers with the hub through api.rs; the seam in main.rs is `settings::start(hub, agents)`.
//!
//! Masking is the one thing other lanes must do: `mask::events(&mut events)` before a step's events are written (Hub.step.go,
//! Secret.changes), `mask::json(&mut answer)` on every MCP answer (Mcp.masked), `mask::mask(line)` on every agent output line and every
//! line to an agent, and `secret::prompt()` / `keys::prompt(..)` in the prompt. See docs/gpui/HUB-API.md.
pub mod accounts;
pub mod diagnostics;
pub mod fallback;
pub mod improvements;
pub mod keys;
#[cfg(test)]
mod e2e;
pub mod mask;
pub mod misc;
pub mod models;
pub mod proposal;
pub mod secret;
pub mod set;
pub mod telemetry;
pub mod vault;
pub mod webhook;
pub mod zone;

use crate::hub::Hub;
use std::sync::Arc;

/// Opens the vault and starts the masks. Call it before agents start, so no turn resumes unmasked (a bad vault is locked, never repaired).
pub fn open(home: &std::path::Path) {
    let st = vault::open(home);
    mask::set_api_path(keys::path(home));
    let _ = diagnostics::start(home);
    if st["locked"] == true { eprintln!("vault: locked: {}", st["why"].as_str().unwrap_or("")); }
}

/// Registers the handlers (after `open`).
pub fn start(hub: Arc<Hub>, agents: Option<Arc<crate::agents::Agents>>) {
    hub.register_rpc(Arc::new(set::SettingSet { agents: agents.clone() }));
    hub.register_rpc(Arc::new(telemetry::TelemetryRpc));
    hub.register_rpc(Arc::new(diagnostics::DiagnosticsRpc));
    hub.register_rpc(Arc::new(zone::ZoneRpc));
    hub.register_rpc(Arc::new(accounts::AccountsDefault));
    hub.register_rpc(Arc::new(misc::SettingsMisc));
    hub.register_rpc(Arc::new(improvements::ImprovementsRpc));
    hub.register_rpc(Arc::new(secret::SecretRpc { agents }));
    if hub.rpc_for("bots.voice").is_none() { hub.register_rpc(Arc::new(keys_rpc::VoiceKeys)); }
}

/// `bots.voice` carries Settings' API keys (ops `api-add`, `api-del`) beside the voice ops. Until the voice owner registers the whole
/// handler, this one answers the key ops and the status's `apiKeys`; the other ops answer "unknown voice request".
pub mod keys_rpc {
    use super::keys;
    use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
    use serde_json::{json, Value};
    use std::sync::Arc;
    pub struct VoiceKeys;
    impl RpcHandler for VoiceKeys {
        fn methods(&self) -> &'static [&'static str] { &["bots.voice"] }
        fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
            Box::pin(async move { vec![answer(&hub.cfg.home, id, p)] })
        }
    }
    /// VoiceJob.op for the key ops: the status goes back with the names only (never a value).
    pub fn answer(home: &std::path::Path, id: &Value, p: &Value) -> Value {
        let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let r = match s("op").as_str() {
            "api-add" => keys::add(home, &s("name"), &s("value")),
            "api-del" => keys::del(home, &s("name")),
            "status" => Ok(()),
            _ => return json!({"t":"reply","id":id,"ok":false,"error":"unknown voice request"}),
        };
        match r {
            Ok(()) => json!({"t":"reply","id":id,"ok":true,"apiKeys":keys::names_kept(home)}),
            Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e.strip_prefix("error: ").unwrap_or(&e)}),
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test] fn the_key_ops_answer_the_names_and_never_a_value() {
            let h = std::env::temp_dir().join(format!("bp-keysrpc-{}", std::process::id())); let _ = std::fs::remove_dir_all(&h);
            let r = answer(&h, &json!(1), &json!({"op":"api-add","name":"OPENAI_KEY","value":"sk-secret-value-123"}));
            assert_eq!(r["ok"], true); assert_eq!(r["apiKeys"], "OPENAI_KEY"); assert!(!r.to_string().contains("sk-secret"));
            let r = answer(&h, &json!(2), &json!({"op":"api-add","name":"bad name","value":"x"})); assert_eq!(r["ok"], false); assert_eq!(r["error"], "Use the environment variable name: BAD_NAME");
            let r = answer(&h, &json!(3), &json!({"op":"api-del","name":"OPENAI_KEY"})); assert_eq!(r["apiKeys"], "");
            assert_eq!(answer(&h, &json!(4), &json!({"op":"start"}))["error"], "unknown voice request");
            let _ = std::fs::remove_dir_all(&h);
        }
    }
}

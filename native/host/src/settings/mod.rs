//! The settings lane: `setting.set`, the telemetry choice, API keys, the secret vault and the masks that keep a stored value out of every
//! event, reply and tool answer. Registers its handlers with the hub through api.rs; the seam in main.rs is `settings::start(hub, agents)`.
//!
//! Masking is the one thing other lanes must do: `mask::events(&mut events)` before a step's events are written (Hub.step.go,
//! Secret.changes), `mask::json(&mut answer)` on every MCP answer (Mcp.masked), `mask::mask(line)` on every agent output line and every
//! line to an agent, and `secret::prompt()` / `keys::prompt(..)` in the prompt. See docs/HUB-API.md.
pub mod accounts;
pub mod alerts;
pub mod accounts_hub;
pub mod binding;
pub mod mentions;
#[cfg(test)]
mod copy_gate;
pub mod accounts_io;
pub mod accounts_phone;
pub mod claude_usage;
pub mod diagnostics;
pub mod driver;
pub mod fab; pub mod skill_install;
pub mod order;
pub mod order_view;
pub mod fallback;
pub mod improvements;
pub mod issue_report;
pub mod diagnostics_http;
pub mod keys;
#[cfg(test)]
mod e2e;
#[cfg(test)]
mod failover_e2e;
#[cfg(test)]
mod restart_e2e;
pub mod mask;
pub mod misc;
pub mod onboard;
pub mod tools_install;
pub mod models;
pub mod models_probe;
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

/// The Settings pages' status reads. The JS hub's replies to `secret.list` / `secret.set` / `secret.del` (`secrets` and `vault`, JSON texts) and to `bots.voice`
/// (`apiKeys`, `voiceKey`, ...) are FLAT: the client's Ui reads those fields from the reply itself (Sbox.reply, Voice.reply). The view's policy applies them
/// as reads (session::record_read, kinds "secret.list" and "bots.voice"); `body` is the reply without `t`, `id` and `ok`, which the policy must merge flat
/// into the reply it builds (it wraps `result` for the other kinds). A secret.set/del reply carries the same two fields as the list, so it is kind "secret.list" too.
/// A failure keeps its `error`. Values never come from the client.
pub fn read_of(method: &str, reply: &serde_json::Value) -> Option<(&'static str, bool, serde_json::Value)> {
    let kind = match method { "secret.list" | "secret.set" | "secret.del" => "secret.list", "bots.voice" => "bots.voice", _ => return None };
    if reply["t"] != "reply" { return None }
    let mut body = reply.clone();
    if let Some(o) = body.as_object_mut() { for k in ["t", "id", "ok"] { o.remove(k); } }
    Some((kind, reply["ok"].as_bool().unwrap_or(true), body))
}

#[cfg(test)]
mod read_tests {
    use super::*; use serde_json::json;
    #[test] fn the_secret_box_and_voice_replies_are_status_reads_with_their_flat_fields() {
        let r = json!({"t":"reply","id":9,"ok":true,"secrets":"[]","vault":"{\"n\":0}"});
        assert_eq!(read_of("secret.list", &r), Some(("secret.list", true, json!({"secrets":"[]","vault":"{\"n\":0}"}))));
        assert_eq!(read_of("secret.set", &r).unwrap().0, "secret.list"); assert_eq!(read_of("secret.del", &r).unwrap().0, "secret.list");
        let v = json!({"t":"reply","id":3,"ok":true,"apiKeys":"A\nB"}); assert_eq!(read_of("bots.voice", &v), Some(("bots.voice", true, json!({"apiKeys":"A\nB"}))));
        let e = json!({"t":"reply","id":4,"ok":false,"error":"A secret is at least 8 characters"}); assert_eq!(read_of("secret.set", &e), Some(("secret.list", false, json!({"error":"A secret is at least 8 characters"}))));
        assert_eq!(read_of("setting.set", &r), None); assert_eq!(read_of("secret.list", &json!({"t":"info"})), None);
    }
    #[test] fn a_planted_value_is_never_part_of_a_read() { let r = json!({"t":"reply","id":1,"ok":true,"secrets":"[{\"name\":\"k\"}]","vault":"{}"}); assert!(!read_of("secret.set", &r).unwrap().2.to_string().contains("value")); }
}

/// Registers the handlers (after `open`).
pub fn start(hub: Arc<Hub>, agents: Option<Arc<crate::agents::Agents>>) {
    hub.register_rpc(Arc::new(set::SettingSet { agents: agents.clone() }));
    hub.register_rpc(Arc::new(telemetry::TelemetryRpc));
    hub.register_rpc(Arc::new(diagnostics::DiagnosticsRpc));
    hub.register_rpc(Arc::new(zone::ZoneRpc));
    let _desks = alerts::start(&hub);
    hub.register_rpc(Arc::new(fab::FabRpc));
    hub.register_rpc(Arc::new(skill_install::SkillRpc));
    hub.register_rpc(Arc::new(accounts::AccountsDefault));
    let _accounts = accounts_hub::start(&hub, agents.clone());
    // Accounts.created for every writer: a new or forked thread freezes its account binding (settings/binding.rs)
    let bindings = binding::Bindings::new();
    hub.register_transform(bindings.clone()); hub.register_observer(bindings.clone());
    { let (h, b) = (hub.clone(), bindings); tokio::spawn(async move { b.seed(&h).await }); }
    hub.register_ticker(Arc::new(driver::ResumeTicker));
    hub.register_rpc(Arc::new(misc::SettingsMisc));
    crate::update::svc::install(&hub);
    hub.register_rpc(Arc::new(onboard::OnboardRpc));
    hub.register_rpc(Arc::new(improvements::ImprovementsRpc));
    hub.register_http(Arc::new(diagnostics_http::DiagnosticsJson));
    hub.register_rpc(Arc::new(secret::SecretRpc { agents }));
    if hub.rpc_for("bots.voice").is_none() { hub.register_rpc(Arc::new(keys_rpc::VoiceKeys)); }
}

/// `bots.voice` carries Settings' API keys (ops `api-add`, `api-del`) beside the voice ops. voice/session.rs registers the whole handler and calls
/// `answer` for the key ops; this one stands in on a hub started without it (tests), where the other ops answer "unknown voice request".
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

//! The `models` info key: the picker's models per agent, what is installed, and the labels.
//! Claude's list is built in and does not depend on an installed CLI; the other agents list what their CLI reports, so with no CLI
//! installed (a stand-in test hub, a fresh machine) they are empty. The hub's detection (models_probe.rs) replaces the built-in value
//! with what the machine's CLIs report (`set_live`); `info` and `all` answer the value in force.
use serde_json::{json, Value};
use std::sync::RwLock;

/// Claude Code's aliases, for a CLI that cannot say its own list.
pub const CLAUDE: &[&str] = &["fable", "opus", "sonnet", "haiku"];

/// The detected info and its `models.all`, once the hub has detected (or read its cache); None: the built-in value.
static LIVE: RwLock<Option<(String, String)>> = RwLock::new(None);
/// Whether the hub detects its agents' models (the binary's start, models_probe::start): a test hub keeps the built-in value and asks no CLI.
static DETECTS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn live() -> bool { DETECTS.load(std::sync::atomic::Ordering::Relaxed) }
pub fn set_detects() { DETECTS.store(true, std::sync::atomic::Ordering::Relaxed) }
/// The info in force becomes `info`; answers whether it changed.
pub fn set_live(info: &str) -> bool {
    let mut g = LIVE.write().unwrap_or_else(|e| e.into_inner());
    if g.as_ref().map_or(false, |(i, _)| i == info) { return false }
    let all = all_of(&serde_json::from_str::<Value>(info).unwrap_or(Value::Null));
    *g = Some((info.to_string(), all)); true
}

/// Agy.version: the pinned Antigravity agent.
pub const AGY_PIN: &str = "1.3.0";

pub fn about() -> Value {
    json!({
        "claude":{"cli":"","labels":{"default":"Claude Code's own default","fable":"Fable 5.1 \u{b7} claude-fable-5-1","opus":"Opus 5.5 \u{b7} claude-opus-5-5","sonnet":"Sonnet 5 \u{b7} claude-sonnet-5","haiku":"Haiku 4.5 \u{b7} claude-haiku-4-5"}},
        "codex":{"cli":"","labels":{"default":"Codex's configured default"}},
        "grok":{"cli":"","labels":{}},
        "muse":{"cli":"","labels":{}},
        "antigravity":{"cli":"","labels":{"default":"Antigravity's own default"}},
        "opencode":{"cli":"","version":"","labels":{"default":"opencode's configured default"},"error":""},
        "pi":{"cli":"","labels":{"default":"Pi's configured default"},"efforts":{"default":["off","minimal","low","medium","high"]}},
        "omp":{"cli":"","labels":{"default":"Oh My Pi's configured default"},"efforts":{"default":["off","auto","minimal","low","medium","high"]}}
    })
}

/// The info value in force (a JSON text, as the JS hub sends it): what the hub detected, else the built-in one.
pub fn info() -> String { LIVE.read().ok().and_then(|g| g.as_ref().map(|(i, _)| i.clone())).unwrap_or_else(builtin) }
/// The info for a machine whose CLIs have not been asked: Claude's aliases and nothing else.
pub fn builtin() -> String {
    json!({"claude":["fable","opus","sonnet","haiku"],"codex":[],"grok":[],"muse":[],"antigravity":[],"opencode":[],"pi":[],"omp":[],
        "agy":{"pin":AGY_PIN,"have":"","old":"","plat":true,"accts":[],"signed":[]},"about":about()}).to_string()
}

/// The `models.all` info (new, not in the JS hub): every provider's models as rows the `&` mention offers, `{provider: [{value, label, ok, why}]}`. `value` is the id the
/// `model` actions use; `ok` is false, with `why`, when the agent cannot be used now (its CLI is not installed). Claude's list is built in and always usable.
pub fn all() -> String { LIVE.read().ok().and_then(|g| g.as_ref().map(|(_, a)| a.clone())).unwrap_or_else(|| all_of(&serde_json::from_str::<Value>(&builtin()).unwrap_or(Value::Null))) }
pub fn all_of(models: &Value) -> String {
    let mut out = serde_json::Map::new();
    let Some(o) = models.as_object() else { return "{}".into() };
    for (agent, ids) in o {
        let Some(ids) = ids.as_array() else { continue };
        let about = &models["about"][agent];
        let installed = agent == "claude" || about["cli"].as_str().map_or(false, |c| !c.is_empty());
        let rows: Vec<Value> = ids.iter().filter_map(Value::as_str).map(|id| {
            let label = about["labels"][id].as_str().unwrap_or(id);
            json!({"value":id,"label":label,"ok":installed,"why":if installed { "" } else { "its CLI is not installed on this machine" }})
        }).collect();
        out.insert(agent.clone(), Value::Array(rows));
    }
    Value::Object(out).to_string()
}

#[cfg(test)]
mod tests {
    /// The JS hub's `models` info with no agent CLI installed (build/gpui/results/parity-current/recA/session-mobile.json).
    const JS: &str = r#"{"claude":["fable","opus","sonnet","haiku"],"codex":[],"grok":[],"muse":[],"antigravity":[],"opencode":[],"pi":[],"omp":[],"agy":{"pin":"1.3.0","have":"","old":"","plat":true,"accts":[],"signed":[]},"about":{"claude":{"cli":"","labels":{"default":"Claude Code's own default","fable":"Fable 5.1 · claude-fable-5-1","opus":"Opus 5.5 · claude-opus-5-5","sonnet":"Sonnet 5 · claude-sonnet-5","haiku":"Haiku 4.5 · claude-haiku-4-5"}},"codex":{"cli":"","labels":{"default":"Codex's configured default"}},"grok":{"cli":"","labels":{}},"muse":{"cli":"","labels":{}},"antigravity":{"cli":"","labels":{"default":"Antigravity's own default"}},"opencode":{"cli":"","version":"","labels":{"default":"opencode's configured default"},"error":""},"pi":{"cli":"","labels":{"default":"Pi's configured default"},"efforts":{"default":["off","minimal","low","medium","high"]}},"omp":{"cli":"","labels":{"default":"Oh My Pi's configured default"},"efforts":{"default":["off","auto","minimal","low","medium","high"]}}}}"#;
    #[test] fn models_info_is_the_js_hubs_for_a_machine_with_no_cli() { assert_eq!(super::builtin(), JS); }
    #[test] fn the_picker_gets_five_claude_models() { let v: serde_json::Value = serde_json::from_str(&super::builtin()).unwrap(); assert_eq!(v["claude"].as_array().unwrap().len(), 4); assert!(v["about"]["claude"]["labels"]["default"].is_string()); }
    #[test] fn models_all_lists_claude_usable_and_other_agents_by_their_cli() {
        let v: serde_json::Value = serde_json::from_str(&super::all()).unwrap();
        assert_eq!(v["claude"][1], serde_json::json!({"value":"opus","label":"Opus 5.5 \u{b7} claude-opus-5-5","ok":true,"why":""}));
        assert_eq!(v["codex"], serde_json::json!([]), "no CLI listing: nothing to offer");
        let m = serde_json::json!({"codex":["gpt-6"],"about":{"codex":{"cli":"","labels":{}}}});
        assert_eq!(serde_json::from_str::<serde_json::Value>(&super::all_of(&m)).unwrap()["codex"][0], serde_json::json!({"value":"gpt-6","label":"gpt-6","ok":false,"why":"its CLI is not installed on this machine"}));
        let m = serde_json::json!({"codex":["gpt-6"],"about":{"codex":{"cli":"/bin/codex","labels":{"gpt-6":"GPT 6"}}}});
        assert_eq!(serde_json::from_str::<serde_json::Value>(&super::all_of(&m)).unwrap()["codex"][0]["ok"], true);
    }
}

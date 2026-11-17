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
        let installed = is_installed(models, agent);
        let rows: Vec<Value> = ids.iter().filter_map(Value::as_str).map(|id| {
            let label = about["labels"][id].as_str().unwrap_or(id);
            json!({"value":id,"label":label,"ok":installed,"why":if installed { "" } else { "its CLI is not installed on this machine" }})
        }).collect();
        out.insert(agent.clone(), Value::Array(rows));
    }
    Value::Object(out).to_string()
}

/// Whether `agent` can run here (present/keys.rs `is_installed`: the one answer the picker, `&` mentions, Settings and a new thread's agent all read).
pub fn is_installed(models: &Value, agent: &str) -> bool { crate::present::keys::is_installed(models, agent) }
/// The agents found here, in the picker's order; None until the CLIs have been asked (nothing is known to be missing yet).
pub fn installed(models: &Value) -> Option<Vec<&'static str>> {
    (models["probed"] == true).then(|| crate::present::keys::AGENTS.iter().map(|a| a.0).filter(|a| is_installed(models, a)).collect())
}
/// The providers with a connected account in the accounts inventory (the setting `accounts.inventory`, the same snapshot Settings › Accounts draws): a named
/// profile whose state is connected, or the CLI's own login (`defaultLogin.<provider>.state` known).
pub fn connected(inventory: &str) -> Vec<String> {
    let v: Value = serde_json::from_str(inventory).unwrap_or(Value::Null);
    let mut out: Vec<String> = v["profiles"].as_array().into_iter().flatten().filter(|p| p["state"] == "connected").filter_map(|p| p["provider"].as_str().map(str::to_string)).collect();
    for (k, d) in v["defaultLogin"].as_object().into_iter().flatten() { if d["state"] == "known" && !out.contains(k) { out.push(k.clone()) } }
    out
}
/// The agent a new thread (or bot) gets: `chosen` (the `agent.provider` setting; "" is Claude) when it is installed here, or as it is when nothing is known yet;
/// else the first installed agent with a connected account, else the first installed one; with none installed, `chosen` (its turn then says it is missing).
/// A machine with only Codex gets Codex, never Claude it does not have.
pub fn default_agent(chosen: &str, models: &Value, connected: &[String]) -> String {
    let Some(have) = installed(models) else { return chosen.to_string() }; // nothing known yet: the setting as it is (a stored "" stays "", M.Setting.get)
    let chosen = if chosen.is_empty() { "claude" } else { chosen };
    if have.contains(&chosen) { return chosen.to_string() }
    have.iter().find(|a| connected.iter().any(|c| c == *a)).or(have.first()).map_or(chosen.to_string(), |a| a.to_string())
}
/// default_agent for the models info in force.
pub fn default_now(chosen: &str, inventory: &str) -> String {
    default_agent(chosen, &serde_json::from_str::<Value>(&info()).unwrap_or(Value::Null), &connected(inventory))
}

#[cfg(test)]
mod tests {
    /// The JS hub's `models` info with no agent CLI installed (build/gpui/results/parity-current/recA/session-mobile.json).
    const JS: &str = r#"{"claude":["fable","opus","sonnet","haiku"],"codex":[],"grok":[],"muse":[],"antigravity":[],"opencode":[],"pi":[],"omp":[],"agy":{"pin":"1.3.0","have":"","old":"","plat":true,"accts":[],"signed":[]},"about":{"claude":{"cli":"","labels":{"default":"Claude Code's own default","fable":"Fable 5.1 · claude-fable-5-1","opus":"Opus 5.5 · claude-opus-5-5","sonnet":"Sonnet 5 · claude-sonnet-5","haiku":"Haiku 4.5 · claude-haiku-4-5"}},"codex":{"cli":"","labels":{"default":"Codex's configured default"}},"grok":{"cli":"","labels":{}},"muse":{"cli":"","labels":{}},"antigravity":{"cli":"","labels":{"default":"Antigravity's own default"}},"opencode":{"cli":"","version":"","labels":{"default":"opencode's configured default"},"error":""},"pi":{"cli":"","labels":{"default":"Pi's configured default"},"efforts":{"default":["off","minimal","low","medium","high"]}},"omp":{"cli":"","labels":{"default":"Oh My Pi's configured default"},"efforts":{"default":["off","auto","minimal","low","medium","high"]}}}}"#;
    #[test] fn models_info_is_the_js_hubs_for_a_machine_with_no_cli() { assert_eq!(super::builtin(), JS); }
    #[test] fn connected_is_what_settings_shows_connected() {
        let inv = serde_json::json!({"profiles":[{"provider":"codex","state":"connected"},{"provider":"claude","state":"expired"}],"defaultLogin":{"claude":{"state":"signed-out"},"grok":{"state":"known"}}}).to_string();
        assert_eq!(super::connected(&inv), ["codex", "grok"]);
        let info = serde_json::json!({"probed": true, "about": {"claude": {"cli": "2.1.322"}, "codex": {"cli": "codex-cli 0.169.0"}}});
        assert_eq!(super::default_agent("grok", &info, &super::connected(&inv)), "codex", "grok not installed: the installed and connected one before Claude");
        assert_eq!(super::default_agent("", &info, &[]), "claude", "Claude installed: the default stays");
    }
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

/// The picker as the hub draws it (present/keys.rs) from this detection: here, not beside keys.rs, whose tests also build into the small bins without `settings`.
#[cfg(test)]
mod picker_tests {
    use crate::present::keys::{picker_with, picker_agents, AGENTS};
    use serde_json::{json, Value};
    /// 0.16.0 (the person, Ubuntu): only Codex installed and connected; the picker had Claude ticked and Codex "not installed". Detected as it is: Codex
    /// installed with its models, Claude not installed (its built-in aliases do not make it installed), and a new thread's agent is Codex.
    #[test] fn only_codex_installed_shows_codex_with_its_models_and_selects_it() {
        let cache = r#"{"models":[{"slug":"gpt-6.1-sol","visibility":"list","display_name":"GPT-6.1-Sol","supported_reasoning_levels":[{"effort":"low"},{"effort":"high"}]}]}"#;
        let f = crate::settings::models_probe::Found { codex: Some(("codex-cli 0.169.0".into(), cache.into())), ..Default::default() };
        let info: Value = serde_json::from_str(&crate::settings::models_probe::info_of(&f)).unwrap();
        let inv = json!({"profiles":[{"id":"k","provider":"codex","state":"connected","label":"alex@example.net"}]}).to_string();
        let to = crate::settings::models::default_agent("", &info, &crate::settings::models::connected(&inv));
        assert_eq!(to, "codex", "the first installed and connected agent, not Claude");
        let p = picker_with(&to, "", "", "", Some(&info));
        let row = |a: &str| p["providers"].as_array().unwrap().iter().find(|r| r["label"] == a).unwrap().clone();
        assert_eq!((row("codex")["ok"].clone(), row("codex")["on"].clone(), row("codex")["note"].clone()), (json!(true), json!(true), json!("Codex CLI")));
        assert_eq!((row("claude")["ok"].clone(), row("claude")["on"].clone(), row("claude")["note"].clone()), (json!(false), json!(false), json!("not installed")));
        for a in ["grok", "muse", "antigravity", "opencode", "pi", "omp"] { assert_eq!(row(a)["ok"], false, "{a}") }
        assert_eq!(p["models"].as_array().unwrap().iter().map(|m| m["value"].as_str().unwrap()).collect::<Vec<_>>(), ["", "gpt-6.1-sol"]);
        let agents = picker_agents(Some(&info));
        assert_eq!(agents["codex"]["ok"], true); assert_eq!(agents["codex"]["models"].as_array().unwrap().len(), 2); assert_eq!(agents["claude"]["ok"], false);
        assert_eq!(crate::settings::models::default_agent("claude", &info, &[]), "codex", "a default set to Claude it does not have: the installed one");
    }
    /// The same for each agent alone: whichever is the only one installed is the one a new thread gets and the picker offers.
    #[test] fn each_agent_alone_is_the_default_and_the_only_one_offered() {
        for (a, _) in AGENTS {
            let mut info: Value = serde_json::from_str(&crate::settings::models_probe::info_of(&Default::default())).unwrap();
            info["about"][*a]["cli"] = json!(format!("{a} 99.0.0"));
            assert_eq!(crate::settings::models::default_agent("", &info, &[]), *a);
            assert_eq!(crate::settings::models::installed(&info).unwrap(), [*a]);
            let p = picker_with(a, "", "", "", Some(&info));
            for r in p["providers"].as_array().unwrap() { assert_eq!(r["ok"] == true, r["label"] == *a, "{a}: {r}") }
        }
        let none: Value = serde_json::from_str(&crate::settings::models_probe::info_of(&Default::default())).unwrap();
        assert_eq!(crate::settings::models::default_agent("codex", &none, &[]), "codex", "nothing installed: the chosen one, whose turn says it is missing");
        let unasked: Value = serde_json::from_str(&crate::settings::models::builtin()).unwrap();
        assert_eq!(crate::settings::models::default_agent("", &unasked, &[]), "", "before detection: the setting as it is");
    }
    #[test] fn an_old_cli_is_offered_with_update_needed_and_a_newer_one_with_a_quiet_note() {
        let mut info: Value = serde_json::from_str(&crate::settings::models_probe::info_of(&crate::settings::models_probe::Found { codex: Some(("codex-cli 0.100.0".into(), String::new())), opencode: Some(("opencode v9.0.0".into(), String::new())), ..Default::default() })).unwrap();
        let p = picker_with("codex", "", "", "", Some(&info));
        let row = |p: &Value, a: &str| p["providers"].as_array().unwrap().iter().find(|r| r["label"] == a).unwrap().clone();
        assert_eq!((row(&p, "codex")["ok"].clone(), row(&p, "codex")["note"].clone()), (json!(true), json!("update needed: v0.100.0, needs ≥ v0.169.0")), "never 'not installed'");
        assert_eq!(row(&p, "opencode")["note"], "OpenCode · v9.0.0, newer than tested v2.3.0");
        assert_eq!(picker_agents(Some(&info))["codex"]["update"], "update needed: v0.100.0, needs ≥ v0.169.0"); assert_eq!(picker_agents(Some(&info))["opencode"]["update"], Value::Null);
        info["about"]["codex"]["cli"] = json!("codex-cli 0.169.0"); info["about"]["codex"]["support"] = Value::Null;
        assert_eq!(row(&picker_with("codex", "", "", "", Some(&info)), "codex")["note"], "Codex CLI");
    }
}

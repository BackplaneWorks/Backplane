//! Speed modes (issue #1166): Normal, Fast and UltraFast, as each provider's CLI offers them, per model. The table is checked against each CLI's
//! docs and `--help` as of 2026-11-17; the picker shows a model only the modes it has here, and a turn passes the mode by the provider's own mechanism.
//!
//! | provider | modes | mechanism | cost |
//! |---|---|---|---|
//! | claude   | fast on Fable 5.2, Mythos 5.2, Opus 5.5, Opus 5, Opus 4.8 (Claude Code's fast mode, code.claude.com/docs/en/fast-mode) | `--settings {"fastMode":true}`; its result says `fast_mode_state` | ×2 (usage credits) |
//! | codex    | the tiers its account's catalog lists per model (`~/.codex/models_cache.json` `additional_speed_tiers`: fast; ultrafast on GPT-6 Astra and GPT-6.1 Sol where the plan has it) | `-c service_tier="fast"` / `"ultrafast"` | fast ×2.5, ultrafast ×8 of the plan's limits |
//! | grok     | fast where `grok models` lists `<model>-build-fast` (Grok 4.7 Fast: "the same model served on faster infrastructure") | that model | ×2 |
//! | opencode | fast where `opencode models` lists `<model>-fast` (models.dev's Fast mode, OpenAI's priority tier) | that model | priority rate |
//! | omp      | fast on anthropic/ models Claude runs fast and on openai/ and openai-codex/ models; ultrafast on their GPT-6 Astra and GPT-6.1 Sol | `--service-tier priority` / `ultrafast` | as Claude / Codex |
//! | antigravity, muse, pi | none: agy 1.5 removed its `/fast` (an execution mode, not a speed); Muse has no speed knob; Pi's CLI exposes no service tier | | |
//!
//! The mode is the thread's setting `speed.<thread>` ("" Normal, "fast", "ultrafast"); a mode the thread's model does not take runs as Normal.
use serde_json::{json, Value};

/// The modes past Normal, slowest first.
pub const MODES: &[&str] = &["fast", "ultrafast"];

/// A mode's name as the dial labels it.
pub fn label(mode: &str) -> &'static str { match mode { "fast" => "Fast", "ultrafast" => "UltraFast", _ => "Normal" } }

/// What a mode costs against Normal on `provider`, where it is known ("" Normal, or unknown).
pub fn cost(provider: &str, mode: &str) -> &'static str {
    match (provider, mode) {
        ("claude", "fast") | ("grok", "fast") => "×2 cost",
        ("codex", "fast") => "×2.5 usage",
        ("codex", "ultrafast") => "×8 usage",
        ("opencode", "fast") | ("omp", "fast") => "priority rate",
        ("omp", "ultrafast") => "×8 usage",
        _ => "",
    }
}

/// A Claude model id (or what its alias stands for) that Claude Code runs in fast mode.
fn claude_fast(id: &str) -> bool {
    let m = id.to_lowercase();
    let m = m.split('[').next().unwrap_or("");
    ["opus-5", "opus-4-8", "fable-5-2", "mythos-5-2"].iter().any(|k| m.contains(k))
}
/// The model id an alias or "" stands for, from the CLI's label ("Opus 5.5 · claude-opus-5-5"); the id itself otherwise.
fn resolved<'a>(info: &'a Value, provider: &str, model: &'a str) -> &'a str {
    let key = if model.is_empty() { "default" } else { model };
    let l = info["about"][provider]["labels"][key].as_str().unwrap_or("");
    match l.rsplit(" \u{b7} ").next().filter(|x| l.contains(" \u{b7} ") && !x.contains(' ')) { Some(id) => id, None => model }
}
/// The fast variant of `model` in `provider`'s listed models (Grok's `-build-fast`, OpenCode's `-fast`), if it lists one.
pub fn variant(info: &Value, provider: &str, model: &str) -> Option<String> {
    if model.is_empty() { return None }
    let list = info[provider].as_array()?;
    let has = |x: &str| list.iter().any(|v| v.as_str() == Some(x));
    let tries: &[&str] = match provider { "grok" => &["-build-fast", "-fast"], "opencode" => &["-fast"], _ => &[] };
    tries.iter().map(|t| format!("{model}{t}")).find(|v| has(v))
}
/// The modes past Normal one model takes, before the default's rule.
fn own(info: &Value, provider: &str, model: &str) -> Vec<&'static str> {
    match provider {
        "claude" => if claude_fast(resolved(info, provider, model)) { vec!["fast"] } else { vec![] },
        "codex" => { let got = &info["about"]["codex"]["speeds"][model];
            MODES.iter().copied().filter(|m| got.as_array().map_or(false, |a| a.iter().any(|x| x == m))).collect() }
        "grok" | "opencode" => if variant(info, provider, model).is_some() { vec!["fast"] } else { vec![] },
        "omp" => {
            let (fam, id) = model.split_once('/').unwrap_or(("", model));
            let ultra = ["gpt-6-astra", "gpt-6.1-sol"].iter().any(|u| id == *u || id.starts_with(&format!("{u}-")));
            match fam { "anthropic" => if claude_fast(id) { vec!["fast"] } else { vec![] },
                "openai" | "openai-codex" => if ultra { vec!["fast", "ultrafast"] } else { vec!["fast"] }, _ => vec![] }
        }
        _ => vec![],
    }
}
/// The modes past Normal that `model` of `provider` takes ("" its default: what the default stands for when the CLI says, else what every listed model takes).
pub fn takes(info: &Value, provider: &str, model: &str) -> Vec<&'static str> {
    if !model.is_empty() || provider == "claude" { return own(info, provider, model) }
    // Grok names its default among its models ("grok-4.7 (default)": models_probe.rs grok_about)
    if let Some(d) = info["about"][provider]["labels"].as_object().and_then(|l| l.iter().find(|(_, v)| *v == "Grok's default model")).map(|(k, _)| k.clone()) { return own(info, provider, &d) }
    let ids: Vec<&str> = info[provider].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|m| !m.is_empty() && *m != "default").collect();
    if ids.is_empty() { return vec![] }
    MODES.iter().copied().filter(|m| ids.iter().all(|id| own(info, provider, id).contains(m))).collect()
}
/// The mode a turn runs at: `want` when the model takes it, else Normal ("").
pub fn effective(info: &Value, provider: &str, model: &str, want: &str) -> &'static str {
    takes(info, provider, model).into_iter().find(|m| *m == want).unwrap_or("")
}
/// The dial's detents for a model: Normal then each mode it takes, each with its label and cost, `on` at the thread's mode; empty when it takes Normal only.
pub fn detents(info: &Value, provider: &str, model: &str, want: &str) -> Vec<Value> {
    let ms = takes(info, provider, model);
    if ms.is_empty() { return vec![] }
    let now = effective(info, provider, model, want);
    std::iter::once("").chain(ms).map(|m| json!({"label": label(m), "value": m, "on": m == now, "cost": cost(provider, m)})).collect()
}

/// How a turn of `provider` passes mode `mode` (already `effective`): the model to run instead, and arguments to add.
#[derive(Debug, Default, PartialEq)]
pub struct How { pub model: Option<String>, pub args: Vec<String> }
pub fn how(info: &Value, provider: &str, model: &str, mode: &str) -> How {
    if mode.is_empty() { return How::default() }
    match provider {
        // the tier and its feature flag ("Set service_tier = \"fast\" and [features].fast_mode = true"; ultrafast_mode is UltraFast's)
        "codex" => How { model: None, args: vec!["-c".into(), format!("features.{mode}_mode=true"), "-c".into(), format!("service_tier=\"{mode}\"")] },
        "grok" | "opencode" => How { model: variant(info, provider, model), args: vec![] },
        "omp" => How { model: None, args: vec!["--service-tier".into(), if mode == "fast" { "priority".into() } else { mode.into() }] },
        _ => How::default(), // claude: rules::claude_args writes its fastMode setting
    }
}
/// Claude's `--settings` value: fast mode on or off, said every time, so the person's own saved `/fast` never makes a Normal thread fast.
pub fn claude_settings(mode: &str) -> String { format!("{{\"fastMode\":{}}}", mode == "fast") }

/// The modes `info` says now (the hub's detected models info, or the built-in one).
pub fn info_now() -> Value { serde_json::from_str(&crate::settings::models::info()).unwrap_or(Value::Null) }

#[cfg(test)]
mod tests {
    use super::*;
    fn info() -> Value {
        json!({"claude": ["opus", "fable", "sonnet", "haiku", "claude-opus-4-7", "claude-opus-4-8", "claude-fable-5-1"],
            "codex": ["gpt-6.1-sol", "gpt-6-astra", "gpt-6-luna"], "grok": ["grok-4.7", "grok-4.7-build-fast", "grok-4.6"],
            "opencode": ["openai/gpt-5", "openai/gpt-5-fast", "openai/gpt-5-flex", "cerebras/gpt-oss-120b"], "omp": [], "pi": ["x/y"], "muse": ["muse-spark-1.3"], "antigravity": ["claude-opus-5-5"],
            "about": {"claude": {"labels": {"default": "Default (recommended) \u{b7} claude-sonnet-5-5", "opus": "Opus 5.5 \u{b7} claude-opus-5-5", "fable": "Fable 5.2 \u{b7} claude-fable-5-2", "sonnet": "Sonnet 5.5 \u{b7} claude-sonnet-5-5", "haiku": "Haiku 5.5 \u{b7} claude-haiku-5-5"}},
                "codex": {"speeds": {"gpt-6.1-sol": ["fast", "ultrafast"], "gpt-6-astra": ["fast"], "gpt-6-luna": ["fast"]}}}})
    }
    #[test] fn each_provider_and_model_takes_only_its_own_modes() {
        let i = info();
        let t = |p: &str, m: &str| takes(&i, p, m);
        // Claude: by what an alias stands for; Opus 4.7, Fable 5.1, Sonnet and Haiku have no fast mode; the default (Sonnet 5.5) neither
        assert_eq!(t("claude", "opus"), ["fast"]); assert_eq!(t("claude", "fable"), ["fast"]); assert_eq!(t("claude", "claude-opus-4-8"), ["fast"]);
        assert_eq!(t("claude", "claude-opus-5-5[1m]"), ["fast"], "a window tag is the same model");
        for m in ["sonnet", "haiku", "claude-opus-4-7", "claude-fable-5-1", ""] { assert!(t("claude", m).is_empty(), "claude {m}") }
        // Codex: its account's catalog, per model; the default takes what every listed model takes
        assert_eq!(t("codex", "gpt-6.1-sol"), ["fast", "ultrafast"]); assert_eq!(t("codex", "gpt-6-astra"), ["fast"]); assert_eq!(t("codex", ""), ["fast"]);
        assert!(t("codex", "gpt-unlisted").is_empty());
        // Grok and OpenCode: a model with a listed fast variant; the variant itself and the rest have none
        let mut g = i.clone(); g["about"]["grok"] = json!({"labels": {"grok-4.7": "Grok's default model", "grok-4.6": "Grok"}});
        assert_eq!(takes(&g, "grok", ""), ["fast"], "Grok's default is the model it names");
        assert_eq!(t("grok", "grok-4.7"), ["fast"]); assert!(t("grok", "grok-4.7-build-fast").is_empty()); assert!(t("grok", "grok-4.6").is_empty()); assert!(t("grok", "").is_empty());
        assert_eq!(t("opencode", "openai/gpt-5"), ["fast"]); assert!(t("opencode", "cerebras/gpt-oss-120b").is_empty(), "flex is slower, not a speed up");
        // Oh My Pi: by the model's provider family
        assert_eq!(t("omp", "anthropic/claude-opus-5-5"), ["fast"]); assert!(t("omp", "anthropic/claude-sonnet-5-5").is_empty());
        assert_eq!(t("omp", "openai-codex/gpt-6.1-sol"), ["fast", "ultrafast"]); assert_eq!(t("omp", "openai/gpt-6-luna"), ["fast"]); assert!(t("omp", "google/gemini-4-carbon").is_empty());
        // no speed mode in these CLIs
        for (p, m) in [("antigravity", "claude-opus-5-5"), ("muse", "muse-spark-1.3"), ("pi", "x/y")] { assert!(t(p, m).is_empty(), "{p}") }
    }
    #[test] fn the_dial_has_a_detent_a_mode_with_its_cost_and_none_for_normal_only() {
        let i = info();
        let d = detents(&i, "codex", "gpt-6.1-sol", "ultrafast");
        assert_eq!(d.iter().map(|x| x["label"].as_str().unwrap()).collect::<Vec<_>>(), ["Normal", "Fast", "UltraFast"]);
        assert_eq!(d[2]["on"], true); assert_eq!(d[1]["cost"], "×2.5 usage"); assert_eq!(d[2]["cost"], "×8 usage"); assert_eq!(d[0]["cost"], "");
        assert_eq!(detents(&i, "claude", "opus", "").len(), 2, "Normal and Fast"); assert_eq!(detents(&i, "claude", "opus", "")[0]["on"], true, "Normal by default");
        assert!(detents(&i, "claude", "haiku", "fast").is_empty(), "Normal only: no dial");
        // a mode the model does not take runs as Normal
        assert_eq!(effective(&i, "codex", "gpt-6-astra", "ultrafast"), ""); assert_eq!(effective(&i, "claude", "sonnet", "fast"), ""); assert_eq!(effective(&i, "claude", "opus", "fast"), "fast");
    }
    #[test] fn each_provider_gets_the_mode_its_own_way() {
        let i = info();
        assert_eq!(how(&i, "codex", "gpt-6.1-sol", "ultrafast").args, ["-c", "features.ultrafast_mode=true", "-c", "service_tier=\"ultrafast\""]);
        assert_eq!(how(&i, "grok", "grok-4.7", "fast").model.as_deref(), Some("grok-4.7-build-fast"));
        assert_eq!(how(&i, "opencode", "openai/gpt-5", "fast").model.as_deref(), Some("openai/gpt-5-fast"));
        assert_eq!(how(&i, "omp", "openai/gpt-6.1-sol", "fast").args, ["--service-tier", "priority"]);
        assert_eq!(how(&i, "codex", "gpt-6.1-sol", ""), How::default(), "Normal adds nothing");
        assert_eq!(claude_settings("fast"), r#"{"fastMode":true}"#); assert_eq!(claude_settings(""), r#"{"fastMode":false}"#);
    }
}

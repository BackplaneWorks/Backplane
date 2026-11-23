//! Provider/model compatibility check and sanitizer.
//!
//! Ensures a model belonging to one provider (for example `opus` or `claude-opus-5-5` on `codex`) is never
//! stored or passed to the wrong provider. Pure so both `backplane-hub` and small bins like `screen-audit`
//! can share the exact same rule.

use serde_json::Value;

pub const PROVIDERS: &[&str] = &[
    "claude",
    "codex",
    "grok",
    "muse",
    "antigravity",
    "opencode",
    "pi",
    "omp",
];

const CLAUDE_ALIASES: &[&str] = &["opus", "sonnet", "haiku", "fable", "mythos", "opusplan"];

const AGY_VARIANTS: &[&str] = &["minimal", "low", "medium", "high", "xhigh", "max", "thinking"];

fn strip_agy_variant(low: &str) -> &str {
    match low.rsplit_once('-') {
        Some((base, suf)) if !base.is_empty() && AGY_VARIANTS.contains(&suf) => base,
        _ => low,
    }
}

fn is_o_series(low: &str) -> bool {
    low.starts_with('o')
        && low.as_bytes().get(1).is_some_and(|b| b.is_ascii_digit())
        && low
            .as_bytes()
            .get(2)
            .map_or(true, |b| matches!(b, b'-' | b'.' | b'_'))
}

fn model_prefix_owner(low: &str) -> Option<&'static str> {
    let base = low.split('[').next().unwrap_or(low);
    if base.starts_with("claude-")
        || CLAUDE_ALIASES
            .iter()
            .any(|a| base == *a || base.starts_with(&format!("{a}-")))
    {
        return Some("claude");
    }
    if base.starts_with("gpt-oss-") {
        return Some("antigravity");
    }
    if base.starts_with("gpt-")
        || base.starts_with("codex-")
        || base == "codex"
        || base.starts_with("chatgpt-")
        || is_o_series(base)
    {
        return Some("codex");
    }
    if base.starts_with("gemini-") {
        return Some("antigravity");
    }
    if base.starts_with("grok-") {
        return Some("grok");
    }
    if base.starts_with("muse-") {
        return Some("muse");
    }
    None
}

fn catalog_contains(info: &Value, provider: &str, model: &str) -> bool {
    info.get(provider)
        .and_then(Value::as_array)
        .is_some_and(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .any(|s| s.eq_ignore_ascii_case(model))
        })
}

fn catalog_other_has(info: &Value, provider: &str, model: &str) -> bool {
    PROVIDERS
        .iter()
        .any(|&p| p != provider && catalog_contains(info, p, model))
}

/// Whether `model` is compatible with `provider` given the optional `models` info catalog.
///
/// Order of checks:
/// 1. Empty / `"default"` is always compatible.
/// 2. Target provider's own catalog allow-list (including Antigravity variant-stripped base IDs).
/// 3. Prefix/syntax ownership (`provider/model` for opencode/pi/omp; Antigravity-hosted `gemini-*`/`claude-*`/`gpt-oss-*`
///    even when the catalog is cold; `claude-*`/aliases, `gpt-*`/`codex-*`/`o3`, `gemini-*`, `grok-*`, `muse-*`).
///    Checked before other-provider catalog exclusion so `("claude", "claude-opus-5-5")` stays valid even when Antigravity also lists it.
/// 4. Foreign catalog exclusion (rejects models listed under another provider's catalog when not listed here).
/// 5. Otherwise compatible (preserves uncached new models and generic test placeholders).
pub fn model_compatible(provider: &str, model: &str, models: Option<&Value>) -> bool {
    let m = model.trim();
    if m.is_empty() || m.eq_ignore_ascii_case("default") {
        return true;
    }
    let p_low = provider.trim().to_ascii_lowercase();
    let p = if p_low.is_empty() {
        "claude"
    } else {
        p_low.as_str()
    };
    let low = m.to_ascii_lowercase();

    if models.is_some_and(|info| {
        catalog_contains(info, p, m)
            || (p == "antigravity" && catalog_contains(info, "antigravity", strip_agy_variant(&low)))
    }) {
        return true;
    }

    if low.contains('/') {
        return matches!(p, "opencode" | "pi" | "omp");
    }
    if p == "antigravity"
        && (low.starts_with("gemini-")
            || low.starts_with("claude-")
            || low.starts_with("gpt-oss-"))
    {
        return true;
    }
    if let Some(owner) = model_prefix_owner(&low) {
        return owner == p;
    }

    if models.is_some_and(|info| catalog_other_has(info, p, m)) {
        return false;
    }

    true
}

/// Returns `model` trimmed if compatible with `provider`, otherwise `""` (the provider's default).
pub fn sanitize_model(provider: &str, model: &str, models: Option<&Value>) -> String {
    let m = model.trim();
    if m.is_empty() || !model_compatible(provider, m, models) {
        String::new()
    } else {
        m.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog() -> Value {
        json!({
            "claude": ["opus", "claude-fable-5-2[1m]", "sonnet", "haiku"],
            "codex": ["gpt-6.2-sol", "gpt-6.1-sol", "gpt-6-luna"],
            "grok": ["grok-4.7", "grok-4.7-build-fast"],
            "muse": ["muse-wonder-1.0", "muse-spark-1.3"],
            "antigravity": ["gemini-4-neon", "gemini-3.8-flash", "claude-opus-5-5", "claude-sonnet-5-5", "gpt-oss-120b"],
            "opencode": ["openai/gpt-6", "anthropic/claude-sonnet-5"],
            "pi": ["inference-net/deepseek-v4.1-flash-1017-cyber"],
            "omp": ["openai/gpt-6.1-sol"]
        })
    }

    #[test]
    fn rejects_claude_models_on_other_providers() {
        let cat = catalog();
        for m in [
            "opus",
            "sonnet",
            "haiku",
            "fable",
            "mythos",
            "opusplan",
            "opus[1m]",
            "claude-opus-5-5",
            "claude-fable-5-2[1m]",
            "opus-5-5",
        ] {
            for p in ["codex", "grok", "muse", "opencode", "pi", "omp"] {
                assert!(
                    !model_compatible(p, m, Some(&cat)),
                    "{m} must be rejected on {p}"
                );
                assert_eq!(sanitize_model(p, m, Some(&cat)), "");
            }
            assert!(
                model_compatible("claude", m, Some(&cat)),
                "{m} must be accepted on claude"
            );
            assert!(
                model_compatible("claude", m, None),
                "{m} must be accepted on claude without catalog"
            );
            assert!(
                !model_compatible("codex", m, None),
                "{m} must be rejected on codex even without catalog"
            );
        }
    }

    #[test]
    fn rejects_codex_gemini_grok_muse_cross_provider_pairs() {
        let cat = catalog();
        for m in ["gpt-6.2-sol", "codex-mini", "codex", "chatgpt-4o", "o3", "o4-mini"] {
            assert!(model_compatible("codex", m, Some(&cat)), "{m} on codex");
            for p in ["claude", "grok", "muse", "antigravity"] {
                assert!(!model_compatible(p, m, Some(&cat)), "{m} rejected on {p}");
            }
        }
        for m in ["gemini-4-neon", "gemini-pro-orchestrator"] {
            assert!(model_compatible("antigravity", m, Some(&cat)));
            for p in ["claude", "codex", "grok", "muse"] {
                assert!(!model_compatible(p, m, Some(&cat)), "{m} rejected on {p}");
            }
        }
        assert!(model_compatible("grok", "grok-4.7", Some(&cat)));
        assert!(!model_compatible("codex", "grok-4.7", Some(&cat)));
        assert!(model_compatible("muse", "muse-wonder-1.0", Some(&cat)));
        assert!(!model_compatible("claude", "muse-wonder-1.0", Some(&cat)));
    }

    #[test]
    fn antigravity_cross_family_variants_cold_catalog_and_claude_full_id_work() {
        let cat = catalog();
        assert!(model_compatible("antigravity", "claude-opus-5-5", Some(&cat)));
        assert!(model_compatible("antigravity", "claude-opus-5-5-high", Some(&cat)));
        assert!(model_compatible("antigravity", "claude-opus-5-5-high", None));
        assert!(model_compatible("antigravity", "claude-opus-5-5", None));
        assert!(model_compatible("antigravity", "gpt-oss-120b", Some(&cat)));
        assert!(model_compatible("antigravity", "gpt-oss-120b", None));
        assert!(model_compatible("antigravity", "gpt-oss-120b-medium", Some(&cat)));
        assert!(model_compatible("claude", "claude-opus-5-5", Some(&cat)));
        assert!(model_compatible("claude", "opusplan", Some(&cat)));
        assert!(!model_compatible("codex", "opusplan", Some(&cat)));
        assert!(!model_compatible("antigravity", "opus", Some(&cat)));
        assert!(!model_compatible("antigravity", "opus", None));
        assert!(!model_compatible("antigravity", "gpt-6.2-sol", Some(&cat)));
        assert!(!model_compatible("codex", "gpt-oss-120b", Some(&cat)));
        assert!(!model_compatible("codex", "claude-opus-5-5", Some(&cat)));
    }

    #[test]
    fn slash_models_only_for_multi_provider_harnesses() {
        let cat = catalog();
        let slash = "inference-net/deepseek-v4.1-flash-1017-cyber";
        for p in ["opencode", "pi", "omp"] {
            assert!(model_compatible(p, slash, Some(&cat)), "{slash} on {p}");
        }
        for p in ["claude", "codex", "grok", "muse", "antigravity"] {
            assert!(!model_compatible(p, slash, Some(&cat)), "{slash} rejected on {p}");
        }
    }

    #[test]
    fn foreign_catalog_only_id_rejected_and_placeholders_kept() {
        let cat = json!({
            "claude": ["opus"],
            "codex": ["custom-reasoning-alpha"]
        });
        assert!(!model_compatible("claude", "custom-reasoning-alpha", Some(&cat)));
        assert!(model_compatible("codex", "custom-reasoning-alpha", Some(&cat)));
        assert!(model_compatible("codex", "", Some(&cat)));
        assert!(model_compatible("codex", "default", Some(&cat)));
        assert!(model_compatible("codex", "m", Some(&cat)));
        assert!(model_compatible("claude", "m", Some(&cat)));
        assert_eq!(sanitize_model("codex", "  gpt-6.2-sol  ", Some(&cat)), "gpt-6.2-sol");
        assert_eq!(sanitize_model("codex", "opus", Some(&cat)), "");
    }
}

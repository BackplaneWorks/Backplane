//! The agent CLIs Backplane supports, by version: one table (docs/providers.md says the same in words). The detection (models_probe.rs) reads each
//! CLI's `--version` once, and the verdict here rides in the models info as `about.<agent>.support`, which the picker, Settings and setup all read:
//! older than `min` is "update needed" with an Update button that runs the CLI's own updater; newer than `tested` is allowed, with a quiet note.
//! An old CLI is still installed and still offered: it is never called "not installed".
use serde_json::{json, Value};

/// One agent CLI: the oldest version Backplane supports and the newest it was tested with; how to update it (its own updater's arguments, or "" for
/// none: Settings opens `page`).
pub struct Support { pub id: &'static str, pub title: &'static str, pub min: &'static str, pub tested: &'static str, pub update: &'static [&'static str], pub page: &'static str }

/// Bump `tested` when a release is run against a newer CLI; raise `min` when Backplane starts to need something a CLI added (and say what in docs/providers.md).
pub const TABLE: &[Support] = &[
    // 2.1.268: the first that reports its isolated account directory (settings/accounts_io.rs DIRECTORY_ERR)
    Support { id: "claude", title: "Claude Code", min: "2.1.268", tested: "2.1.322", update: &["update"], page: "https://code.claude.com/docs/en/setup" },
    Support { id: "codex", title: "Codex CLI", min: "0.169.0", tested: "0.169.0", update: &["update"], page: "https://github.com/openai/codex/releases/latest" },
    Support { id: "grok", title: "Grok CLI", min: "1.0.88", tested: "1.0.88", update: &["update"], page: "https://x.ai/cli" },
    Support { id: "muse", title: "Muse Code", min: "1.4.23", tested: "1.4.23", update: &[], page: "https://dev.meta.ai/docs/muse-code" },
    Support { id: "antigravity", title: "Antigravity CLI (agy)", min: "1.5.10", tested: "1.5.10", update: &["update"], page: "https://antigravity.google" },
    Support { id: "opencode", title: "OpenCode", min: "2.3.0", tested: "2.3.0", update: &["upgrade"], page: "https://opencode.ai" },
    Support { id: "pi", title: "Pi", min: "1.4.0", tested: "1.4.0", update: &["update", "--self"], page: "https://pi.dev" },
    Support { id: "omp", title: "Oh My Pi", min: "18.18.8", tested: "18.18.8", update: &["update"], page: "https://github.com/can1357/oh-my-pi" },
];
pub fn of(id: &str) -> Option<&'static Support> { TABLE.iter().find(|s| s.id == id) }
/// The program an agent runs as (Antigravity's is `agy`).
pub fn program(id: &str) -> &str { if id == "antigravity" { "agy" } else { id } }

/// The first version number in a `--version` line ("codex-cli 0.169.0", "opencode v2.3.0", "omp/18.18.8", "2.1.322 (Claude Code)"): its parts.
pub fn parse(line: &str) -> Option<Vec<u64>> {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let start_ok = b[i].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'v');
        if start_ok {
            let end = line[i..].find(|c: char| !(c.is_ascii_digit() || c == '.')).map_or(line.len(), |e| i + e);
            let parts: Vec<u64> = line[i..end].trim_end_matches('.').split('.').filter_map(|p| p.parse().ok()).collect();
            if parts.len() >= 2 { return Some(parts) }
            i = end;
        }
        i += 1;
    }
    None
}
fn cmp(a: &[u64], b: &[u64]) -> std::cmp::Ordering {
    for i in 0..a.len().max(b.len()) { let o = a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0)); if o.is_ne() { return o } }
    std::cmp::Ordering::Equal
}
fn dotted(v: &[u64]) -> String { v.iter().map(u64::to_string).collect::<Vec<_>>().join(".") }

/// The verdict on an installed CLI's version line: {version, min, tested, state: ok | old | newer | unknown, note}. `unknown`: no version could be read
/// (a `--version` that failed says why in the line); it is not taken for old.
pub fn verdict(id: &str, line: &str) -> Value {
    let Some(s) = of(id) else { return Value::Null };
    let Some(v) = parse(line) else { return json!({"version": "", "min": s.min, "tested": s.tested, "state": "unknown", "note": ""}) };
    let (min, tested) = (parse(s.min).unwrap_or_default(), parse(s.tested).unwrap_or_default());
    let ver = dotted(&v);
    let (state, note) = if cmp(&v, &min).is_lt() { ("old", format!("update needed: v{ver}, needs ≥ v{}", s.min)) }
        else if cmp(&v, &tested).is_gt() { ("newer", format!("v{ver}, newer than tested v{}", s.tested)) }
        else { ("ok", String::new()) };
    json!({"version": ver, "min": s.min, "tested": s.tested, "state": state, "note": note})
}
/// The models info with each installed agent's verdict at `about.<agent>.support`.
pub fn mark(info: &mut Value) {
    for s in TABLE {
        let cli = info["about"][s.id]["cli"].as_str().unwrap_or("").to_string();
        if cli.is_empty() || !info["about"][s.id].is_object() { continue }
        // Antigravity's line is "Antigravity CLI 1.5.10"; a pinned agy agent's says its own version the same way
        info["about"][s.id]["support"] = verdict(s.id, &cli);
    }
}
/// Whether the models info says `id` needs an update, and the words for it.
pub fn update_note(models: &Value, id: &str) -> Option<String> {
    let s = &models["about"][id]["support"];
    (s["state"] == "old").then(|| s["note"].as_str().unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn every_agent_has_one_row_with_a_min_no_newer_than_tested() {
        for id in crate::present::keys::AGENTS.iter().map(|a| a.0) { let s = of(id).unwrap_or_else(|| panic!("{id} has no row")); assert!(cmp(&parse(s.min).unwrap(), &parse(s.tested).unwrap()).is_le(), "{id}"); }
        assert_eq!(TABLE.len(), crate::present::keys::AGENTS.len());
    }
    #[test] fn version_lines_of_each_cli_parse() {
        for (l, v) in [("2.1.322 (Claude Code)", "2.1.322"), ("codex-cli 0.169.0", "0.169.0"), ("grok 1.0.88 (2fe3608f7398) [stable]", "1.0.88"), ("Antigravity CLI 1.5.10", "1.5.10"),
            ("Muse Code 1.4.23 (1.4.23-R8977.1)", "1.4.23"), ("opencode v2.3.0", "2.3.0"), ("1.4.0", "1.4.0"), ("omp/18.18.8", "18.18.8")] { assert_eq!(parse(l).map(|p| dotted(&p)).as_deref(), Some(v), "{l}"); }
        assert_eq!(parse("/x/pi (--version failed: exit 127)"), None);
    }
    #[test] fn old_is_update_needed_newer_is_a_quiet_note_and_unreadable_is_not_old() {
        assert_eq!(verdict("claude", "2.1.200 (Claude Code)")["state"], "old"); assert_eq!(verdict("claude", "2.1.200 (Claude Code)")["note"], "update needed: v2.1.200, needs ≥ v2.1.268");
        assert_eq!(verdict("claude", "2.1.300 (Claude Code)")["state"], "ok");
        assert_eq!(verdict("codex", "codex-cli 0.200.1")["state"], "newer"); assert_eq!(verdict("codex", "codex-cli 0.200.1")["note"], "v0.200.1, newer than tested v0.169.0");
        assert_eq!(verdict("pi", "/x/pi (--version failed: exit 1)")["state"], "unknown");
        let mut i = json!({"about": {"codex": {"cli": "codex-cli 0.100.0"}, "grok": {"cli": ""}}}); mark(&mut i);
        assert_eq!(update_note(&i, "codex").as_deref(), Some("update needed: v0.100.0, needs ≥ v0.169.0")); assert!(i["about"]["grok"].get("support").is_none(), "not installed: no verdict");
    }
}

//! Clear zero-provider notification and actionable send guard across desktop, web, iOS, and Android.
//! Pure detection over `(models, accounts.inventory)` so every client renders the hub's single source of truth
//! without false positives during startup or in test fixtures before CLI detection completes.
use crate::hub::Hub;
use serde_json::{json, Value};
use std::sync::Arc;

pub const INFO_KEY: &str = "provider.notice";
pub const ACCOUNTS_URL: &str = "backplane://settings/accounts";
pub const HEAD: &str = "No AI provider connected";
pub const TEXT_NONE_INSTALLED: &str = "Connect Claude, Codex, Antigravity, or another provider to start chatting.";
pub const TEXT_SIGNED_OUT: &str = "Sign in to Claude, Codex, or another provider in Settings → Accounts to start chatting.";
pub const BUTTON: &str = "Open Settings → Accounts";
pub const SEND_GUARD_ERROR: &str = "No AI provider connected yet. Open Settings → Accounts to connect Claude, Codex, Antigravity, or another provider.";
pub const SEND_GUARD_CODE: &str = "no_provider";
pub const SEND_GUARD_REMOTE_CODE: &str = "no_provider_remote";

/// Refusal copy for a send routed to a linked machine that has zero providers connected.
pub fn remote_send_guard_error(machine: &str) -> String {
    let who = if machine.trim().is_empty() { "that machine" } else { machine.trim() };
    format!(
        "No AI provider connected on {who}. Open Settings → Accounts on {who} to connect Claude, Codex, Antigravity, or another provider."
    )
}

/// RPC reply frame for a local send-guard refusal, preserving string `error` plus structured `errorCode`.
pub fn rpc_error_reply(id: &Value, why: &str) -> Value {
    json!({"t": "reply", "id": id, "ok": false, "error": why, "errorCode": SEND_GUARD_CODE})
}

/// True when a reply frame is a local zero-provider refusal from a hub.
pub fn is_local_refusal_reply(frame: &Value) -> bool {
    frame["ok"] == false
        && (frame["errorCode"].as_str() == Some(SEND_GUARD_CODE)
            || frame["error"].as_str() == Some(SEND_GUARD_ERROR))
}

/// Rewrites a forwarded destination-hub zero-provider refusal so it names `machine` instead of directing the user to local Settings.
pub fn rewrite_remote_reply(mut frame: Value, machine: &str) -> Value {
    if is_local_refusal_reply(&frame) {
        if let Some(o) = frame.as_object_mut() {
            o.insert("error".into(), json!(remote_send_guard_error(machine)));
            o.insert("errorCode".into(), json!(SEND_GUARD_REMOTE_CODE));
            if !machine.trim().is_empty() {
                o.insert("machine".into(), json!(machine.trim()));
            }
        }
    }
    frame
}

const NON_ACCOUNT_PROVIDERS: &[&str] = &["grok", "muse", "antigravity", "opencode", "pi", "omp"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub reason: &'static str, // "none_installed" | "signed_out"
    pub head: &'static str,
    pub text: &'static str,
    pub button: &'static str,
    pub url: &'static str,
}

impl Notice {
    pub fn json(&self) -> Value {
        json!({
            "reason": self.reason,
            "head": self.head,
            "text": self.text,
            "button": self.button,
            "url": self.url,
        })
    }
}

fn provider_usable(inv: &Value, provider: &str) -> bool {
    let profiles = inv["profiles"].as_array();
    if profiles.into_iter().flatten().any(|p| {
        p["provider"].as_str() == Some(provider)
            && matches!(p["state"].as_str(), Some("connected" | "unchecked"))
    }) {
        return true;
    }
    match inv["defaultLogin"][provider]["state"].as_str() {
        Some("signed-out") => false,
        // Not yet checked, known login, or unknown auth (e.g. API key / env var): never false-positive.
        None | Some(_) => true,
    }
}

/// Pure zero-provider check over probed `models` info and `accounts.inventory`.
pub fn check(models: &Value, inventory: &str) -> Option<Notice> {
    if models["probed"] != true {
        return None;
    }
    if NON_ACCOUNT_PROVIDERS.iter().any(|p| crate::present::keys::is_installed(models, p)) {
        return None;
    }
    let claude_cli = crate::present::keys::is_installed(models, "claude");
    let codex_cli = crate::present::keys::is_installed(models, "codex");
    if !claude_cli && !codex_cli {
        return Some(Notice {
            reason: "none_installed",
            head: HEAD,
            text: TEXT_NONE_INSTALLED,
            button: BUTTON,
            url: ACCOUNTS_URL,
        });
    }
    let inv: Value = serde_json::from_str(inventory).unwrap_or(Value::Null);
    if !inv["error"].as_str().unwrap_or("").is_empty() {
        return None;
    }
    if (claude_cli && provider_usable(&inv, "claude")) || (codex_cli && provider_usable(&inv, "codex")) {
        return None;
    }
    Some(Notice {
        reason: "signed_out",
        head: HEAD,
        text: TEXT_SIGNED_OUT,
        button: BUTTON,
        url: ACCOUNTS_URL,
    })
}

/// Pure JSON value for presenter `shown["noProvider"]` (Null when a provider is usable or detection has not run).
pub fn from_screen(models_raw: Option<&str>, probed_here: Option<bool>, inventory: &str) -> Value {
    if probed_here == Some(false) {
        return Value::Null;
    }
    let models = models_raw
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or(Value::Null);
    check(&models, inventory).map_or(Value::Null, |n| n.json())
}

/// Pure send guard predicate for testability without mutating process-global `models` state.
pub fn guard_for(models: &Value, inventory: &str, client: u64, retry: bool, has_content: bool) -> Option<&'static str> {
    if client == 0 || retry || !has_content {
        return None;
    }
    check(models, inventory).map(|_| SEND_GUARD_ERROR)
}

#[cfg(test)]
static TEST_MODELS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<usize, (u64, bool, Value)>>> =
    std::sync::LazyLock::new(Default::default);
#[cfg(test)]
static TEST_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[cfg(test)]
pub struct TestOverride {
    key: usize,
    seq: u64,
}

#[cfg(test)]
impl Drop for TestOverride {
    fn drop(&mut self) {
        if let Ok(mut g) = TEST_MODELS.lock() {
            if g.get(&self.key).is_some_and(|(s, _, _)| *s == self.seq) {
                g.remove(&self.key);
            }
        }
    }
}

/// Per-hub test override for `(probed_here, models)` without mutating global `models::LIVE` or `PROBED_HERE`.
#[cfg(test)]
pub fn override_for_test(hub: &Arc<Hub>, probed_here: bool, models: Value) -> TestOverride {
    let key = Arc::as_ptr(hub) as usize;
    let seq = TEST_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    TEST_MODELS.lock().unwrap().insert(key, (seq, probed_here, models));
    TestOverride { key, seq }
}

pub async fn for_hub(hub: &Arc<Hub>) -> Option<Notice> {
    #[cfg(test)]
    let test_override = TEST_MODELS
        .lock()
        .unwrap()
        .get(&(Arc::as_ptr(hub) as usize))
        .map(|(_, probed, models)| (*probed, models.clone()));
    #[cfg(not(test))]
    let test_override: Option<(bool, Value)> = None;

    let (probed_here, models) = match test_override {
        Some(v) => v,
        None => (
            super::models::probed_here(),
            serde_json::from_str::<Value>(&super::models::info()).unwrap_or(Value::Null),
        ),
    };
    if !probed_here {
        return None;
    }
    let inventory = hub.setting("accounts.inventory").await.unwrap_or_default();
    check(&models, &inventory)
}

pub async fn send_guard(hub: &Arc<Hub>) -> Option<&'static str> {
    for_hub(hub).await.map(|_| SEND_GUARD_ERROR)
}

pub async fn publish(hub: &Arc<Hub>) {
    let val = for_hub(hub).await.map_or_else(String::new, |n| n.json().to_string());
    let _ = hub.set_info(INFO_KEY, &val).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probed_with(installed: &[&str]) -> Value {
        let mut about = serde_json::Map::new();
        for (agent, _) in crate::present::keys::AGENTS {
            let cli = if installed.contains(agent) { format!("/usr/bin/{agent}") } else { String::new() };
            about.insert((*agent).to_string(), json!({"cli": cli}));
        }
        json!({"probed": true, "about": about})
    }

    #[test]
    fn unprobed_models_never_false_positive() {
        let builtin: Value = serde_json::from_str(&crate::settings::models::builtin()).unwrap();
        assert_eq!(check(&builtin, ""), None);
        assert_eq!(check(&Value::Null, ""), None);
        assert_eq!(from_screen(None, None, ""), Value::Null);
        assert_eq!(from_screen(Some(&probed_with(&[]).to_string()), Some(false), ""), Value::Null);
    }

    #[test]
    fn zero_installed_clis_returns_none_installed_notice() {
        let m = probed_with(&[]);
        let n = check(&m, "").expect("expected notice with no CLIs installed");
        assert_eq!(n.reason, "none_installed");
        assert_eq!(n.head, HEAD);
        assert_eq!(n.text, TEXT_NONE_INSTALLED);
        assert_eq!(n.button, BUTTON);
        assert_eq!(n.url, ACCOUNTS_URL);
        assert_eq!(from_screen(Some(&m.to_string()), Some(true), ""), n.json());
    }

    #[test]
    fn signed_out_claude_and_codex_with_no_connected_profiles_returns_signed_out_notice() {
        let m = probed_with(&["claude", "codex"]);
        let inv = json!({
            "version": 1,
            "revision": 2,
            "profiles": [
                {"id": "p1", "provider": "claude", "state": "disconnected"},
                {"id": "p2", "provider": "codex", "state": "expired"}
            ],
            "defaultLogin": {
                "claude": {"state": "signed-out"},
                "codex": {"state": "signed-out"}
            },
            "error": ""
        }).to_string();
        let n = check(&m, &inv).expect("expected signed_out notice");
        assert_eq!(n.reason, "signed_out");
        assert_eq!(n.text, TEXT_SIGNED_OUT);
        assert_eq!(n.url, ACCOUNTS_URL);
    }

    #[test]
    fn usable_profiles_and_logins_stay_quiet() {
        let m = probed_with(&["claude"]);
        let inv_signed_out = json!({"defaultLogin": {"claude": {"state": "signed-out"}}}).to_string();
        assert!(check(&m, &inv_signed_out).is_some());

        // Connected profile overrides signed-out default login
        let inv_connected = json!({
            "profiles": [{"id": "w", "provider": "claude", "state": "connected"}],
            "defaultLogin": {"claude": {"state": "signed-out"}}
        }).to_string();
        assert_eq!(check(&m, &inv_connected), None);

        // Unchecked profile on startup avoids flash
        let inv_unchecked = json!({
            "profiles": [{"id": "w", "provider": "claude", "state": "unchecked"}],
            "defaultLogin": {"claude": {"state": "signed-out"}}
        }).to_string();
        assert_eq!(check(&m, &inv_unchecked), None);

        // Known, unknown, or missing defaultLogin stays quiet
        for st in ["known", "unknown"] {
            let inv = json!({"defaultLogin": {"claude": {"state": st}}}).to_string();
            assert_eq!(check(&m, &inv), None, "state {st}");
        }
        assert_eq!(check(&m, ""), None, "defaultLogin not yet checked");

        // Inventory error stays quiet
        let inv_err = json!({"defaultLogin": {"claude": {"state": "signed-out"}}, "error": "read failed"}).to_string();
        assert_eq!(check(&m, &inv_err), None);
    }

    #[test]
    fn other_installed_provider_or_codex_connected_stays_quiet() {
        let both = probed_with(&["claude", "codex"]);
        let inv_codex_ok = json!({
            "profiles": [{"id": "c1", "provider": "codex", "state": "connected"}],
            "defaultLogin": {"claude": {"state": "signed-out"}, "codex": {"state": "signed-out"}}
        }).to_string();
        assert_eq!(check(&both, &inv_codex_ok), None);

        for p in NON_ACCOUNT_PROVIDERS {
            let m = probed_with(&[p]);
            assert_eq!(check(&m, ""), None, "non-account provider {p} installed");
        }
    }

    #[test]
    fn send_guard_triggers_only_for_non_retry_person_messages_with_content() {
        let none = probed_with(&[]);
        assert_eq!(guard_for(&none, "", 1, false, true), Some(SEND_GUARD_ERROR));
        assert_eq!(guard_for(&none, "", 0, false, true), None, "internal hub sends allowed");
        assert_eq!(guard_for(&none, "", 1, true, true), None, "retry allowed");
        assert_eq!(guard_for(&none, "", 1, false, false), None, "empty allowed");

        let ok = probed_with(&["grok"]);
        assert_eq!(guard_for(&ok, "", 1, false, true), None);
    }

    #[test]
    fn remote_rewrite_tags_and_names_destination_machine_without_changing_other_errors() {
        let local_reply = rpc_error_reply(&json!(7), SEND_GUARD_ERROR);
        assert_eq!(local_reply["errorCode"], SEND_GUARD_CODE);
        assert!(is_local_refusal_reply(&local_reply));

        let rewritten = rewrite_remote_reply(local_reply, "boat-hub");
        assert_eq!(rewritten["ok"], false);
        assert_eq!(rewritten["errorCode"], SEND_GUARD_REMOTE_CODE);
        assert_eq!(rewritten["machine"], "boat-hub");
        assert_eq!(
            rewritten["error"].as_str().unwrap(),
            "No AI provider connected on boat-hub. Open Settings → Accounts on boat-hub to connect Claude, Codex, Antigravity, or another provider."
        );
        assert!(!rewritten["error"].as_str().unwrap().starts_with("No AI provider connected yet."));

        let other = json!({"t":"reply","id":7,"ok":false,"error":"permission denied"});
        assert!(!is_local_refusal_reply(&other));
        assert_eq!(rewrite_remote_reply(other.clone(), "boat-hub"), other);
    }

    #[tokio::test]
    async fn per_hub_override_gates_notice_on_current_process_scan() {
        let dir = crate::plat::tmp::dir("pn-override");
        let hub = Hub::start(
            crate::hub::Cfg {
                home: dir.to_path_buf(),
                version: "0.1.0-dev".into(),
                commit: "c".into(),
                listen: "x".into(),
                os: "linux".into(),
                arch: "x64".into(),
            },
            Arc::new(crate::presenter::Canned { reply: json!({}) }),
        )
        .await
        .unwrap();

        let _cached = override_for_test(&hub, false, probed_with(&[]));
        assert_eq!(for_hub(&hub).await, None, "cached negative probe must not emit notice before live scan");
        assert_eq!(send_guard(&hub).await, None);

        let _scanned = override_for_test(&hub, true, probed_with(&[]));
        let notice = for_hub(&hub).await.expect("live scan with zero providers emits notice");
        assert_eq!(notice.reason, "none_installed");
        assert_eq!(send_guard(&hub).await, Some(SEND_GUARD_ERROR));
    }
}

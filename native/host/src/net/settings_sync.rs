//! Home P2: shared-settings allowlist over the signed owner link (`settings.sync`, docs/remote-architecture.md §9.2, §9.7–9.8).
//!
//! Only explicitly allowlisted user preferences sync between the owner's machines:
//! - theme (`ui.theme`, `theme`)
//! - language (`ui.lang`, `lang`)
//! - quiet hours & notifications (`notify.quiet`, `notify.quiet.urgent`, `notify.<kind>`)
//! - default models (`agent.provider`, `agent.model`, `agent.thinking`)
//! - project pins, icons, colors, and groups (`sidebar.view`, `sidebar.groups`, `sidebar.attention`,
//!   `sidebar.pin.*`, `sidebar.group.of.*`, `sidebar.group.name.*`, `sidebar.group.pick.*`,
//!   `sidebar.archived.*`, `sidebar.fold.*`, `project.icon.*`, `project.color.*`, `project.pick.*`)
//!
//! Machine-local folders/paths, ports, KiCad pane state, local builds, accounts, and secrets stay local.
//! Conflict resolution is per-key Last-Write-Wins by `at` (unix seconds; tie-break by value), including
//! empty-value clears so unpinning or clearing a preference propagates cleanly.
use crate::hub::Hub;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

const MAX_ENTRIES: usize = 4096;
const MAX_VAL_LEN: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub value: String,
    pub at: u64,
}

/// Exact approved shared-settings allowlist (docs/remote-architecture.md §9.7).
pub fn is_shared(key: &str) -> bool {
    if key.is_empty() || key.len() > 256 || crate::push::reserved(key) {
        return false;
    }
    if matches!(
        key,
        "ui.theme"
            | "theme"
            | "ui.lang"
            | "lang"
            | "agent.provider"
            | "agent.model"
            | "agent.thinking"
            | "sidebar.view"
            | "sidebar.groups"
            | "sidebar.attention"
    ) {
        return true;
    }
    if let Some(rest) = key.strip_prefix("notify.") {
        return !rest.is_empty();
    }
    const PREFIXES: &[&str] = &[
        "sidebar.pin.",
        "sidebar.group.of.",
        "sidebar.group.name.",
        "sidebar.group.pick.",
        "sidebar.archived.",
        "sidebar.fold.",
        "project.icon.",
        "project.color.",
        "project.pick.",
    ];
    PREFIXES
        .iter()
        .any(|p| key.strip_prefix(p).is_some_and(|r| !r.is_empty()))
}

/// Validate a shared setting's value before accepting it from a peer (same rules as `setting.set`).
pub fn value_ok(key: &str, value: &str) -> bool {
    if value.len() > MAX_VAL_LEN {
        return false;
    }
    if key == crate::push::QUIET {
        return value.is_empty() || value == "off" || crate::push::window(value).is_some();
    }
    if key == crate::push::QUIET_URGENT {
        return matches!(value, "" | "on" | "off");
    }
    true
}

/// Pure LWW merge: returns the `SettingSet` events needed to bring `local` up to date with `remote`.
pub fn merge(
    local: &BTreeMap<String, Entry>,
    remote: &BTreeMap<String, Entry>,
) -> Vec<Value> {
    let mut out = Vec::new();
    for (key, rem) in remote {
        if !is_shared(key) || !value_ok(key, &rem.value) {
            continue;
        }
        let take = match local.get(key) {
            None => true,
            Some(cur) => {
                rem.value != cur.value
                    && (rem.at > cur.at || (rem.at == cur.at && rem.value > cur.value))
            }
        };
        if take {
            out.push(json!({
                "$": "SettingSet",
                "key": key,
                "value": rem.value,
                "at": rem.at,
            }));
        }
    }
    let effective = |k: &str| -> (String, u64) {
        if let Some(ev) = out.iter().find(|e| e["key"].as_str() == Some(k)) {
            return (
                ev["value"].as_str().unwrap_or("").to_string(),
                ev["at"].as_u64().unwrap_or(0),
            );
        }
        local
            .get(k)
            .map(|e| (e.value.clone(), e.at))
            .unwrap_or_default()
    };
    let (eff_prov_raw, eff_prov_at) = effective("agent.provider");
    let eff_prov = if eff_prov_raw.is_empty() { "claude" } else { eff_prov_raw.as_str() };
    let (eff_model, eff_model_at) = effective("agent.model");
    let info = crate::settings::models::info_value();
    if !eff_model.is_empty()
        && !crate::settings::models::model_compatible(eff_prov, &eff_model, Some(&info))
    {
        out.retain(|e| e["key"].as_str() != Some("agent.model"));
        if local.get("agent.model").is_some_and(|e| {
            !e.value.is_empty()
                && !crate::settings::models::model_compatible(eff_prov, &e.value, Some(&info))
        }) {
            out.push(json!({
                "$": "SettingSet",
                "key": "agent.model",
                "value": "",
                "at": eff_prov_at.max(eff_model_at),
            }));
        }
    }
    out
}

#[derive(Default)]
struct Cached {
    lines: usize,
    entries: BTreeMap<String, Entry>,
}

fn cache() -> &'static Mutex<HashMap<PathBuf, Cached>> {
    static C: OnceLock<Mutex<HashMap<PathBuf, Cached>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Incrementally collect all allowlisted shared settings (including empty-value clears) from `hub.store`.
pub fn collect(hub: &Hub) -> BTreeMap<String, Entry> {
    let total = hub.store.len();
    let mut map = cache().lock().unwrap();
    let entry = map.entry(hub.cfg.home.clone()).or_default();
    if entry.lines > total {
        entry.lines = 0;
        entry.entries.clear();
    }
    for i in entry.lines..total {
        let Ok(line) = hub.store.get(i) else { break };
        if !line.contains("\"SettingSet\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if v["$"] != "SettingSet" {
            continue;
        }
        let key = v["key"].as_str().unwrap_or("");
        if !is_shared(key) {
            continue;
        }
        let value = v["value"].as_str().unwrap_or("").to_string();
        let at = v["at"].as_u64().unwrap_or(0);
        entry.entries.insert(key.to_string(), Entry { value, at });
    }
    entry.lines = total;
    entry.entries.clone()
}

fn parse_entries(v: &Value) -> BTreeMap<String, Entry> {
    let mut out = BTreeMap::new();
    let Some(obj) = v.as_object() else {
        return out;
    };
    for (k, val) in obj.iter().take(MAX_ENTRIES) {
        if !is_shared(k) {
            continue;
        }
        let Some(value) = val.get("value").and_then(Value::as_str) else {
            continue;
        };
        let at = val.get("at").and_then(Value::as_u64).unwrap_or(0);
        if value_ok(k, value) {
            out.insert(
                k.clone(),
                Entry {
                    value: value.to_string(),
                    at,
                },
            );
        }
    }
    out
}

async fn apply_remote(hub: &Arc<Hub>, remote: &BTreeMap<String, Entry>) -> Result<usize, String> {
    let local = collect(hub);
    let events = merge(&local, remote);
    if events.is_empty() {
        return Ok(0);
    }
    let n = events.len();
    hub.commit_many(&events)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(n)
}

/// Owning/receiving side of `settings.sync` on `/far/rpc` (signed owner-only link).
pub async fn serve(hub: &Arc<Hub>, p: &Value) -> Value {
    let remote = parse_entries(&p["entries"]);
    if let Err(error) = apply_remote(hub, &remote).await {
        return json!({"t":"reply","ok":false,"error":error});
    }
    let mine = collect(hub);
    json!({"t":"reply","ok":true,"result":{"entries":mine}})
}

/// Exchange shared settings with one linked owner machine.
pub async fn sync_with(hub: &Arc<Hub>, link: &str) -> Result<usize, String> {
    if crate::copy::on() || link.is_empty() {
        return Ok(0);
    }
    let mine = collect(hub);
    let (code, body) = hub
        .far_raw_paired(link, "settings.sync", &json!({"entries": mine}))
        .await;
    if crate::net::rpc::unavailable(code) || code == 501 || code == 403 {
        return Ok(0);
    }
    if code != 200 {
        return Err(format!("settings.sync to {link}: {code}"));
    }
    let reply: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    if reply["ok"] != true {
        return Err(reply["error"]
            .as_str()
            .unwrap_or("invalid settings.sync reply")
            .to_string());
    }
    let remote = parse_entries(&reply["result"]["entries"]);
    apply_remote(hub, &remote).await
}

/// Trigger an immediate sync pass after a local allowlisted `setting.set` write.
pub fn push_now(hub: Arc<Hub>) {
    if crate::copy::on() {
        return;
    }
    tokio::spawn(async move {
        let mut links: Vec<String> = hub
            .far_machines()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        let home: Value = serde_json::from_str(&hub.setting("home.at").await.unwrap_or_default())
            .unwrap_or(Value::Null);
        if let Some(hl) = home["link"].as_str().filter(|s| !s.is_empty()) {
            if !links.iter().any(|l| l == hl) {
                links.push(hl.to_string());
            }
        }
        for link in links {
            let _ = sync_with(&hub, &link).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};

    async fn start(tag: &str) -> (crate::plat::tmp::TempDir, Arc<Hub>) {
        let dir = crate::plat::tmp::dir(&format!("settings-sync-{tag}"));
        let hub = Hub::start(
            Cfg {
                home: dir.to_path_buf(),
                version: "0.1.0-dev".into(),
                commit: "c".into(),
                listen: "x".into(),
                os: "linux".into(),
                arch: "x64".into(),
            },
            Arc::new(Canned { reply: json!({}) }),
        )
        .await
        .unwrap();
        (dir, hub)
    }

    #[test]
    fn allowlist_includes_approved_categories_and_excludes_local_and_secrets() {
        for k in [
            "ui.theme",
            "theme",
            "ui.lang",
            "lang",
            "notify.quiet",
            "notify.quiet.urgent",
            "notify.all",
            "notify.ask",
            "agent.provider",
            "agent.model",
            "agent.thinking",
            "sidebar.view",
            "sidebar.groups",
            "sidebar.attention",
            "sidebar.pin.github.com/org/repo",
            "sidebar.group.of.github.com/org/repo",
            "sidebar.group.name.custom:hw",
            "sidebar.group.pick.custom:hw",
            "sidebar.archived.github.com/org/repo",
            "sidebar.fold.github.com/org/repo",
            "project.icon.p1",
            "project.color.p1",
            "project.pick.p1",
        ] {
            assert!(is_shared(k), "expected shared: {k}");
        }
        for k in [
            "",
            "project.link.p1",
            "machine.id",
            "peer.own.x",
            "home.at",
            "home.here",
            "accounts.claude",
            "account.thread.t1.claude",
            "push.apns.key",
            "push.book",
            "ring.front",
            "ring.front.link",
            "kicad.embed.pane",
            "layout:p1",
            "probe:x",
            "restart.continue",
            "update.channel",
            "sandbox.id",
            "launch.input.t1",
            "tok.secret",
            "notify.",
            "sidebar.pin.",
            "project.icon.",
        ] {
            assert!(!is_shared(k), "expected local-only: {k}");
        }
    }

    #[test]
    fn merge_uses_last_write_wins_and_propagates_clears() {
        let mut local = BTreeMap::new();
        local.insert("ui.theme".into(), Entry { value: "dark".into(), at: 100 });
        local.insert("ui.lang".into(), Entry { value: "en".into(), at: 200 });
        local.insert("sidebar.pin.org/repo".into(), Entry { value: "100".into(), at: 100 });

        let mut remote = BTreeMap::new();
        remote.insert("ui.theme".into(), Entry { value: "light".into(), at: 150 }); // newer wins
        remote.insert("ui.lang".into(), Entry { value: "ja".into(), at: 150 }); // older ignored
        remote.insert("sidebar.pin.org/repo".into(), Entry { value: "".into(), at: 120 }); // clear wins
        remote.insert("agent.model".into(), Entry { value: "opus".into(), at: 50 }); // missing locally -> taken
        remote.insert("machine.id".into(), Entry { value: "evil".into(), at: 999 }); // not allowlisted -> ignored
        remote.insert("notify.quiet".into(), Entry { value: "not-a-window".into(), at: 999 }); // invalid -> ignored

        let evs = merge(&local, &remote);
        assert_eq!(evs.len(), 3, "{evs:?}");
        assert!(evs.iter().any(|e| e["key"] == "ui.theme" && e["value"] == "light" && e["at"] == 150));
        assert!(evs.iter().any(|e| e["key"] == "sidebar.pin.org/repo" && e["value"] == "" && e["at"] == 120));
        assert!(evs.iter().any(|e| e["key"] == "agent.model" && e["value"] == "opus" && e["at"] == 50));
    }

    #[test]
    fn merge_drops_or_clears_incompatible_agent_model_when_provider_is_codex() {
        let mut local = BTreeMap::new();
        local.insert("agent.provider".into(), Entry { value: "codex".into(), at: 200 });
        local.insert("agent.model".into(), Entry { value: "opus".into(), at: 100 });
        let mut remote = BTreeMap::new();
        remote.insert("agent.model".into(), Entry { value: "opus".into(), at: 150 });
        let evs = merge(&local, &remote);
        assert!(evs.iter().any(|e| e["key"] == "agent.model" && e["value"] == ""), "{evs:?}");

        let mut local2 = BTreeMap::new();
        local2.insert("agent.provider".into(), Entry { value: "codex".into(), at: 200 });
        let evs2 = merge(&local2, &remote);
        assert!(!evs2.iter().any(|e| e["key"] == "agent.model" && e["value"] == "opus"), "{evs2:?}");

        // Remote incompatible model must NOT wipe a compatible local model
        let mut local3 = BTreeMap::new();
        local3.insert("agent.provider".into(), Entry { value: "codex".into(), at: 200 });
        local3.insert("agent.model".into(), Entry { value: "gpt-6.1-sol".into(), at: 210 });
        let mut remote3 = BTreeMap::new();
        remote3.insert("agent.model".into(), Entry { value: "opus".into(), at: 250 });
        let evs3 = merge(&local3, &remote3);
        assert!(!evs3.iter().any(|e| e["key"] == "agent.model"), "compatible local model must be preserved: {evs3:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn two_hub_settings_sync_round_trip_and_offline_catchup() {
        let (_da, a) = start("a").await;
        let (_db, b) = start("b").await;

        a.commit_many(&[
            json!({"$":"SettingSet","key":"ui.theme","value":"light","at":10}),
            json!({"$":"SettingSet","key":"notify.quiet","value":"22:00-07:00","at":10}),
            json!({"$":"SettingSet","key":"machine.id","value":"m-a","at":10}),
        ])
        .await
        .unwrap();
        b.commit_many(&[
            json!({"$":"SettingSet","key":"ui.lang","value":"ja","at":20}),
            json!({"$":"SettingSet","key":"ui.theme","value":"dark","at":5}), // older than A's 10
            json!({"$":"SettingSet","key":"machine.id","value":"m-b","at":20}),
        ])
        .await
        .unwrap();

        // A calls serve on B, then applies B's reply (exact mirror of sync_with over /far/rpc)
        let reply = serve(&b, &json!({"entries": collect(&a)})).await;
        assert_eq!(reply["ok"], true);
        let remote_from_b = parse_entries(&reply["result"]["entries"]);
        apply_remote(&a, &remote_from_b).await.unwrap();

        // Both hubs now converge on shared keys while keeping local-only machine.id untouched
        assert_eq!(a.setting("ui.theme").await.as_deref(), Some("light"));
        assert_eq!(b.setting("ui.theme").await.as_deref(), Some("light"));
        assert_eq!(a.setting("ui.lang").await.as_deref(), Some("ja"));
        assert_eq!(b.setting("ui.lang").await.as_deref(), Some("ja"));
        assert_eq!(b.setting("notify.quiet").await.as_deref(), Some("22:00-07:00"));
        assert_eq!(a.setting("machine.id").await.as_deref(), Some("m-a"));
        assert_eq!(b.setting("machine.id").await.as_deref(), Some("m-b"));

        // Offline edit on B (clearing notify.quiet + changing theme at newer timestamp) catches up on next round
        b.commit_many(&[
            json!({"$":"SettingSet","key":"notify.quiet","value":"","at":30}),
            json!({"$":"SettingSet","key":"ui.theme","value":"dark","at":30}),
        ])
        .await
        .unwrap();
        let reply2 = serve(&b, &json!({"entries": collect(&a)})).await;
        apply_remote(&a, &parse_entries(&reply2["result"]["entries"])).await.unwrap();
        assert_eq!(a.setting("notify.quiet").await, None);
        assert_eq!(a.setting("ui.theme").await.as_deref(), Some("dark"));
    }
}

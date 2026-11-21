//! Peer version & capability reporting across linked hubs:
//! each peer reports its `version` (semver), `commit` (short SHA), and supported peer `features`
//! during `/far/ws` handshake, `machine_list`, and `home.state`, surfaced in Settings → Machines
//! and the `machine_list` MCP tool.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

/// Peer-to-peer capabilities supported by this hub build.
pub const PEER_FEATURES: &[&str] = &[
    "peerws",
    "blobs",
    "keptChunks",
    "remoteKicad",
    "sharedSettings",
];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub version: String,
    pub commit: String,
    pub features: Vec<String>,
    pub seen_at: u64,
}

fn store() -> &'static Mutex<HashMap<(PathBuf, String), PeerInfo>> {
    static S: OnceLock<Mutex<HashMap<(PathBuf, String), PeerInfo>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn now_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Record a peer's reported version, commit, and capability list.
pub fn record(home: &Path, link: &str, version: &str, commit: &str, features: Vec<String>) {
    if link.is_empty() || (version.is_empty() && commit.is_empty() && features.is_empty()) {
        return;
    }
    let short_commit: String = commit.chars().take(10).collect();
    store().lock().unwrap().insert(
        (home.to_path_buf(), link.to_string()),
        PeerInfo {
            version: version.trim().to_string(),
            commit: short_commit,
            features,
            seen_at: now_s(),
        },
    );
}

/// Record from a JSON object carrying `version`, `commit`, and/or `features`.
pub fn record_json(home: &Path, link: &str, v: &Value) {
    let ver = v["version"].as_str().unwrap_or("");
    let commit = v["commit"].as_str().unwrap_or("");
    let feats: Vec<String> = v["features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect();
    record(home, link, ver, commit, feats);
}

pub fn get(home: &Path, link: &str) -> Option<PeerInfo> {
    store()
        .lock()
        .unwrap()
        .get(&(home.to_path_buf(), link.to_string()))
        .cloned()
}

pub fn forget(home: &Path, link: &str) {
    store()
        .lock()
        .unwrap()
        .remove(&(home.to_path_buf(), link.to_string()));
}

/// Parse leading `major.minor.patch` numeric tuple from a version string like `0.16.0-dev`.
pub fn semver_tuple(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches('v').split('-').next()?;
    let mut parts = core.split('.');
    let maj = parts.next()?.parse().ok()?;
    let min = parts.next().unwrap_or("0").parse().ok()?;
    let pat = parts.next().unwrap_or("0").parse().ok()?;
    Some((maj, min, pat))
}

/// True when the peer is behind `local_version` or missing any capability in `PEER_FEATURES`.
pub fn is_older(local_version: &str, peer: &PeerInfo) -> bool {
    if let (Some(loc), Some(rem)) = (semver_tuple(local_version), semver_tuple(&peer.version)) {
        if rem < loc {
            return true;
        }
    }
    PEER_FEATURES
        .iter()
        .any(|f| !peer.features.iter().any(|pf| pf == f))
}

/// Format a concise version label (`v0.16.0 (d0ac5f6)`) and whether the peer is older.
pub fn summary(local_version: &str, peer: &PeerInfo) -> (String, bool) {
    let older = is_older(local_version, peer);
    let mut label = String::new();
    if !peer.version.is_empty() {
        if peer.version.starts_with('v') {
            label.push_str(&peer.version);
        } else {
            label.push('v');
            label.push_str(&peer.version);
        }
    }
    let short: String = peer.commit.chars().take(7).collect();
    if !short.is_empty() && short != "unknown" {
        if !label.is_empty() {
            label.push(' ');
        }
        label.push('(');
        label.push_str(&short);
        label.push(')');
    }
    (label, older)
}

/// Attach `version`, `commit`, `features`, and `older` onto a linked-machine JSON object.
pub fn enrich_json(home: &Path, local_version: &str, link: &str, obj: &mut Value) {
    let Some(info) = get(home, link) else { return };
    let (label, older) = summary(local_version, &info);
    if !info.version.is_empty() {
        obj["version"] = json!(info.version);
    }
    if !info.commit.is_empty() {
        obj["commit"] = json!(info.commit);
    }
    obj["features"] = json!(info.features);
    obj["versionLabel"] = json!(label);
    if older {
        obj["older"] = json!(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_and_feature_comparison_detects_older_peers() {
        let full: Vec<String> = PEER_FEATURES.iter().map(|s| s.to_string()).collect();
        let current = PeerInfo {
            version: "0.16.0".into(),
            commit: "d0ac5f66b3".into(),
            features: full.clone(),
            seen_at: 1,
        };
        assert!(!is_older("0.16.0", &current));
        let (lbl, older) = summary("0.16.0", &current);
        assert_eq!(lbl, "v0.16.0 (d0ac5f6)");
        assert!(!older);

        let older_ver = PeerInfo {
            version: "0.15.9".into(),
            commit: "1234567".into(),
            features: full,
            seen_at: 1,
        };
        assert!(is_older("0.16.0", &older_ver));

        let missing_feat = PeerInfo {
            version: "0.16.0".into(),
            commit: "abcdef1".into(),
            features: vec!["peerws".into()],
            seen_at: 1,
        };
        assert!(is_older("0.16.0", &missing_feat));
    }

    #[test]
    fn record_get_and_forget_are_scoped_per_home() {
        let ha = PathBuf::from("/tmp/peerver-test-a");
        let hb = PathBuf::from("/tmp/peerver-test-b");
        record(&ha, "p1", "0.16.0", "d0ac5f66b3", vec!["peerws".into()]);
        assert_eq!(get(&ha, "p1").unwrap().version, "0.16.0");
        assert!(get(&hb, "p1").is_none());
        forget(&ha, "p1");
        assert!(get(&ha, "p1").is_none());
    }
}

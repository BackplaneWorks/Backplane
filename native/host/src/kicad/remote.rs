//! Remote KiCad Access (docs/remote-architecture.md §10): local native KiCad wired to a remote project's design files.
//!
//! Architecture:
//! - Owning remote hub serves `kicad.manifest`, `kicad.blob`, and `kicad.push` over the signed owner link (`/far/ws` / `/far/rpc`).
//! - Local hub pulls only changed design files (`canonical::is_design`) into `<home>/far-kicad/<link>/<project>/`,
//!   tracks each file's `baseSha256` in `.bp-sync.json`, opens local native KiCad on the synced folder at full native speed,
//!   watches for local saves, and pushes changed files back with optimistic concurrency (`baseSha256` conflict detection).
//! - Conflicts are never silently clobbered: the user chooses Reload remote copy (`kicad.remote.reload`) or
//!   Save as labelled copy (`kicad.remote.push {copyLabel}`).
use super::{canonical, open, project};
use crate::{
    api::{Caller, Fut, RpcHandler},
    hub::Hub,
    net::botnet,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

const MAX_DESIGN_FILE: u64 = 16 << 20; // 16 MiB per design file
const MAX_MANIFEST_FILES: usize = 2048;
const META_FILE: &str = ".bp-sync.json";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestEntry {
    pub rel: String,
    pub sha256: String,
    pub bytes: u64,
    pub mtime: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConflictEntry {
    pub rel: String,
    #[serde(rename = "baseSha256")]
    pub base_sha256: String,
    #[serde(rename = "remoteSha256")]
    pub remote_sha256: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SyncMeta {
    pub link: String,
    pub project: String,
    pub bases: BTreeMap<String, String>,
    #[serde(default)]
    pub conflicts: Vec<ConflictEntry>,
}

/// Safe relative path check: no leading `/`, no `..`, no null bytes, no backslashes.
pub fn safe_rel(rel: &str) -> bool {
    !rel.is_empty()
        && rel.len() <= 512
        && !rel.starts_with('/')
        && !rel.contains('\\')
        && !rel.contains('\0')
        && rel.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

/// Whether `rel` is a syncable KiCad design file (reuses `canonical::is_design` plus `.backplane.json`).
pub fn is_syncable(rel: &str) -> bool {
    if !safe_rel(rel) {
        return false;
    }
    let name = rel.rsplit('/').next().unwrap_or(rel);
    if name.starts_with('.') && name != ".backplane.json" {
        return false;
    }
    if name.ends_with(".lck") || name.ends_with(".bak") || name.ends_with(".tmp") {
        return false;
    }
    if rel.split('/').any(|s| s == ".git" || s == "target" || s.ends_with("-backups")) {
        return false;
    }
    name == ".backplane.json" || canonical::is_design(rel)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(sha256_bytes(&bytes))
}

/// Walk `root` and build a deterministic manifest of all syncable design files.
pub fn build_manifest(root: &Path) -> Vec<ManifestEntry> {
    let mut out = Vec::new();
    let Ok(canon_root) = crate::plat::fs::canonicalize(root) else {
        return out;
    };
    let mut stack = vec![(canon_root.clone(), String::new())];
    while let Some((dir, prefix)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for ent in entries {
            if out.len() >= MAX_MANIFEST_FILES {
                break;
            }
            let name = ent.file_name().to_string_lossy().into_owned();
            if name == ".git" || name == "target" || name == "node_modules" || name.ends_with("-backups") || name == "copies" {
                continue;
            }
            let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
            let path = ent.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                if !name.starts_with('.') {
                    stack.push((path, rel));
                }
            } else if meta.is_file() && meta.len() <= MAX_DESIGN_FILE && is_syncable(&rel) {
                if let Ok(bytes) = std::fs::read(&path) {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_secs());
                    out.push(ManifestEntry {
                        rel,
                        sha256: sha256_bytes(&bytes),
                        bytes: bytes.len() as u64,
                        mtime,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

async fn resolve_project_root(hub: &Arc<Hub>, project_id: &str) -> Option<PathBuf> {
    let root = {
        let m = hub.model().await;
        m.projects
            .get(project_id)
            .filter(|p| !p.removed && !p.root.is_empty())
            .map(|p| p.root.clone())
    }?;
    let design = canonical::design_root(&root);
    let p = PathBuf::from(design);
    p.is_dir().then_some(p)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("bp-tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    crate::plat::fs::rename(&tmp, path)
}

// ---- Remote owning hub side (/far/rpc & /far/ws) ----

pub async fn serve(hub: &Arc<Hub>, m: &str, p: &Value) -> Value {
    let project_id = p["project"].as_str().unwrap_or("");
    if project_id.is_empty() {
        return json!({"t":"reply","ok":false,"error":"project is required"});
    }
    let Some(root) = resolve_project_root(hub, project_id).await else {
        return json!({"t":"reply","ok":false,"error":"no such project here"});
    };
    match m {
        "kicad.manifest" => {
            let r = root.clone();
            let files = tokio::task::spawn_blocking(move || build_manifest(&r))
                .await
                .unwrap_or_default();
            json!({"t":"reply","ok":true,"result":{"root":root.to_string_lossy(),"files":files}})
        }
        "kicad.blob" => {
            let rel = p["rel"].as_str().unwrap_or("").to_string();
            if !is_syncable(&rel) {
                return json!({"t":"reply","ok":false,"error":"file is not a syncable design file"});
            }
            let target = root.join(&rel);
            let want_sha = p["sha256"].as_str().unwrap_or("").to_string();
            let res = tokio::task::spawn_blocking(move || -> Result<(String, Vec<u8>), String> {
                let meta = std::fs::metadata(&target).map_err(|_| "file not found".to_string())?;
                if !meta.is_file() || meta.len() > MAX_DESIGN_FILE {
                    return Err("file unavailable or too large".into());
                }
                let bytes = std::fs::read(&target).map_err(|e| e.to_string())?;
                let sha = sha256_bytes(&bytes);
                if !want_sha.is_empty() && sha != want_sha {
                    return Err("file changed during sync".into());
                }
                Ok((sha, bytes))
            })
            .await;
            match res {
                Ok(Ok((sha, bytes))) => json!({
                    "t":"reply","ok":true,
                    "result":{"rel":rel,"sha256":sha,"data":base64::engine::general_purpose::STANDARD.encode(bytes)}
                }),
                Ok(Err(e)) => json!({"t":"reply","ok":false,"error":e}),
                Err(_) => json!({"t":"reply","ok":false,"error":"blob read failed"}),
            }
        }
        "kicad.push" => {
            let Some(arr) = p["files"].as_array() else {
                return json!({"t":"reply","ok":false,"error":"files array required"});
            };
            let copy_label = p["copyLabel"].as_str().unwrap_or("").trim().to_string();
            if !copy_label.is_empty() && !botnet::safe(&copy_label) {
                return json!({"t":"reply","ok":false,"error":"invalid copyLabel"});
            }
            let mut decoded: Vec<(String, String, String, Vec<u8>)> = Vec::with_capacity(arr.len());
            for item in arr {
                let rel = item["rel"].as_str().unwrap_or("").to_string();
                let base_sha = item["baseSha256"].as_str().unwrap_or("").to_string();
                let sha = item["sha256"].as_str().unwrap_or("").to_string();
                let data_b64 = item["data"].as_str().unwrap_or("");
                if !is_syncable(&rel) {
                    return json!({"t":"reply","ok":false,"error":format!("not syncable: {rel}")});
                }
                let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data_b64) else {
                    return json!({"t":"reply","ok":false,"error":format!("invalid base64 for {rel}")});
                };
                if bytes.len() as u64 > MAX_DESIGN_FILE || sha256_bytes(&bytes) != sha {
                    return json!({"t":"reply","ok":false,"error":format!("checksum mismatch for {rel}")});
                }
                decoded.push((rel, base_sha, sha, bytes));
            }
            let res = tokio::task::spawn_blocking(move || -> Value {
                // Optimistic concurrency check when writing to canonical files.
                if copy_label.is_empty() {
                    let mut conflicts = Vec::new();
                    for (rel, base_sha, _, _) in &decoded {
                        let cur = sha256_file(&root.join(rel)).unwrap_or_default();
                        if cur != *base_sha {
                            conflicts.push(ConflictEntry {
                                rel: rel.clone(),
                                base_sha256: base_sha.clone(),
                                remote_sha256: cur,
                            });
                        }
                    }
                    if !conflicts.is_empty() {
                        return json!({"t":"reply","ok":false,"error":"conflict","conflict":conflicts});
                    }
                }
                let dest_root = if copy_label.is_empty() {
                    root
                } else {
                    root.join("copies").join(&copy_label)
                };
                let mut written = Vec::new();
                for (rel, _, sha, bytes) in decoded {
                    if let Err(e) = write_atomic(&dest_root.join(&rel), &bytes) {
                        return json!({"t":"reply","ok":false,"error":format!("write {rel}: {e}")});
                    }
                    written.push(json!({"rel": rel, "sha256": sha}));
                }
                json!({"t":"reply","ok":true,"result":{"written":written,"copyLabel":copy_label}})
            })
            .await;
            res.unwrap_or_else(|_| json!({"t":"reply","ok":false,"error":"push failed"}))
        }
        _ => json!({"t":"reply","ok":false,"error":"unknown kicad remote method"}),
    }
}

// ---- Local hub side (synced working copy + save watcher) ----

pub fn cache_dir(home: &Path, link: &str, project_id: &str) -> PathBuf {
    home.join("far-kicad").join(link).join(project_id)
}

fn load_meta(dir: &Path, link: &str, project_id: &str) -> SyncMeta {
    std::fs::read_to_string(dir.join(META_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| SyncMeta {
            link: link.to_string(),
            project: project_id.to_string(),
            bases: BTreeMap::new(),
            conflicts: Vec::new(),
        })
}

fn save_meta(dir: &Path, meta: &SyncMeta) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(meta).unwrap_or_default();
    write_atomic(&dir.join(META_FILE), &bytes)
}

/// Pull remote design files into `<home>/far-kicad/<link>/<project>/`, fetching only missing/changed hashes.
pub async fn sync_pull(hub: &Arc<Hub>, link: &str, project_id: &str) -> Result<Value, String> {
    if !botnet::safe(link) || !botnet::safe(project_id) {
        return Err("invalid link or project id".into());
    }
    let (code, body) = hub
        .far_raw(link, "kicad.manifest", &json!({"project": project_id}))
        .await;
    if code != 200 {
        return Err(format!("remote manifest failed ({code})"));
    }
    let reply: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    if reply["ok"] != true {
        return Err(reply["error"].as_str().unwrap_or("manifest error").into());
    }
    let manifest: Vec<ManifestEntry> =
        serde_json::from_value(reply["result"]["files"].clone()).map_err(|e| e.to_string())?;
    let dir = cache_dir(&hub.cfg.home, link, project_id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut meta = load_meta(&dir, link, project_id);
    let mut pulled = 0usize;
    let mut unchanged = 0usize;

    for entry in &manifest {
        let local_path = dir.join(&entry.rel);
        let cur_sha = sha256_file(&local_path).unwrap_or_default();
        if cur_sha == entry.sha256 {
            meta.bases.insert(entry.rel.clone(), entry.sha256.clone());
            unchanged += 1;
            continue;
        }
        let (bc, bbody) = hub
            .far_raw(
                link,
                "kicad.blob",
                &json!({"project": project_id, "rel": entry.rel, "sha256": entry.sha256}),
            )
            .await;
        if bc != 200 {
            return Err(format!("fetch {} failed ({bc})", entry.rel));
        }
        let brep: Value = serde_json::from_str(&bbody).map_err(|e| e.to_string())?;
        if brep["ok"] != true {
            return Err(brep["error"].as_str().unwrap_or("blob error").into());
        }
        let b64 = brep["result"]["data"].as_str().unwrap_or("");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| e.to_string())?;
        if sha256_bytes(&bytes) != entry.sha256 {
            return Err(format!("checksum mismatch fetching {}", entry.rel));
        }
        write_atomic(&local_path, &bytes).map_err(|e| e.to_string())?;
        meta.bases.insert(entry.rel.clone(), entry.sha256.clone());
        pulled += 1;
    }
    meta.conflicts.clear();
    save_meta(&dir, &meta).map_err(|e| e.to_string())?;
    Ok(json!({
        "dir": dir.to_string_lossy(),
        "pulled": pulled,
        "unchanged": unchanged,
        "total": manifest.len(),
        "conflicts": [],
    }))
}

/// Push locally modified design files back to the remote owning hub with `baseSha256` conflict checking.
pub async fn sync_push(
    hub: &Arc<Hub>,
    link: &str,
    project_id: &str,
    copy_label: &str,
) -> Result<Value, String> {
    let dir = cache_dir(&hub.cfg.home, link, project_id);
    if !dir.is_dir() {
        return Err("remote project is not synced locally yet".into());
    }
    let mut meta = load_meta(&dir, link, project_id);
    let current = build_manifest(&dir);
    let mut payload = Vec::new();
    for entry in current {
        let base = meta.bases.get(&entry.rel).cloned().unwrap_or_default();
        if base == entry.sha256 {
            continue;
        }
        let bytes = std::fs::read(dir.join(&entry.rel)).map_err(|e| e.to_string())?;
        payload.push(json!({
            "rel": entry.rel,
            "baseSha256": base,
            "sha256": entry.sha256,
            "data": base64::engine::general_purpose::STANDARD.encode(bytes),
        }));
    }
    if payload.is_empty() {
        return Ok(json!({"pushed": 0, "conflicts": meta.conflicts}));
    }
    let (code, body) = hub
        .far_raw(
            link,
            "kicad.push",
            &json!({"project": project_id, "files": payload, "copyLabel": copy_label}),
        )
        .await;
    if code != 200 {
        return Err(format!("remote push failed ({code})"));
    }
    let reply: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    if reply["ok"] != true {
        if let Ok(conflicts) = serde_json::from_value::<Vec<ConflictEntry>>(reply["conflict"].clone()) {
            if !conflicts.is_empty() {
                meta.conflicts = conflicts.clone();
                let _ = save_meta(&dir, &meta);
                return Ok(json!({"pushed": 0, "conflict": true, "conflicts": conflicts}));
            }
        }
        return Err(reply["error"].as_str().unwrap_or("push refused").into());
    }
    if copy_label.is_empty() {
        for w in reply["result"]["written"].as_array().into_iter().flatten() {
            if let (Some(rel), Some(sha)) = (w["rel"].as_str(), w["sha256"].as_str()) {
                meta.bases.insert(rel.to_string(), sha.to_string());
            }
        }
        meta.conflicts.clear();
        let _ = save_meta(&dir, &meta);
    }
    Ok(json!({
        "pushed": reply["result"]["written"].as_array().map_or(0, Vec::len),
        "copyLabel": copy_label,
        "conflicts": [],
    }))
}

fn active_watchers() -> &'static Mutex<HashSet<(PathBuf, String, String)>> {
    static W: OnceLock<Mutex<HashSet<(PathBuf, String, String)>>> = OnceLock::new();
    W.get_or_init(Default::default)
}

/// Start a background local save-back watcher for `(link, project_id)` (idempotent).
pub fn start_watcher(hub: Arc<Hub>, link: String, project_id: String) {
    let key = (hub.cfg.home.clone(), link.clone(), project_id.clone());
    if !active_watchers().lock().unwrap().insert(key.clone()) {
        return;
    }
    tokio::spawn(async move {
        let dir = cache_dir(&hub.cfg.home, &link, &project_id);
        let mut idle_ticks = 0u32;
        loop {
            tokio::time::sleep(Duration::from_millis(750)).await;
            if !dir.is_dir() {
                break;
            }
            let meta = load_meta(&dir, &link, &project_id);
            if !meta.conflicts.is_empty() {
                idle_ticks += 1;
                if idle_ticks > 2400 {
                    break;
                }
                continue;
            }
            match sync_push(&hub, &link, &project_id, "").await {
                Ok(v) if v["pushed"].as_u64().unwrap_or(0) > 0 => {
                    idle_ticks = 0;
                    hub.push_all(&json!({"t":"info","info":{"remoteKicad":v}}));
                }
                Ok(v) if v["conflict"] == true => {
                    idle_ticks = 0;
                    hub.push_all(&json!({"t":"info","info":{"remoteKicadConflict":v["conflicts"]}}));
                }
                _ => {
                    idle_ticks += 1;
                    if idle_ticks > 2400 {
                        break;
                    }
                }
            }
        }
        active_watchers().lock().unwrap().remove(&key);
    });
}

fn parse_target(p: &Value) -> Option<(String, String)> {
    let link = p["link"].as_str().unwrap_or("");
    let project = p["project"].as_str().unwrap_or("");
    if !link.is_empty() && !project.is_empty() {
        return Some((link.to_string(), project.to_string()));
    }
    let raw = p["project"].as_str().or_else(|| p["thread"].as_str()).unwrap_or("");
    raw.split_once('~').filter(|(l, id)| !l.is_empty() && !id.is_empty()).map(|(l, id)| (l.to_string(), id.to_string()))
}

pub struct RemoteKicadRpc;

impl RpcHandler for RemoteKicadRpc {
    fn methods(&self) -> &'static [&'static str] {
        &[
            "kicad.remote.sync",
            "kicad.remote.open",
            "kicad.remote.push",
            "kicad.remote.reload",
            "kicad.remote.status",
        ]
    }
    fn call<'a>(
        &'a self,
        hub: &'a Arc<Hub>,
        _who: &'a Caller,
        id: &'a Value,
        method: &'a str,
        p: &'a Value,
    ) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let Some((link, project_id)) = parse_target(p) else {
                return vec![json!({"t":"reply","id":id,"ok":false,"error":"link and project are required"})];
            };
            match method {
                "kicad.remote.sync" | "kicad.remote.reload" => {
                    match sync_pull(hub, &link, &project_id).await {
                        Ok(r) => vec![json!({"t":"reply","id":id,"ok":true,"result":r})],
                        Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e})],
                    }
                }
                "kicad.remote.push" => {
                    let label = p["copyLabel"].as_str().unwrap_or("");
                    match sync_push(hub, &link, &project_id, label).await {
                        Ok(r) => vec![json!({"t":"reply","id":id,"ok":true,"result":r})],
                        Err(e) => vec![json!({"t":"reply","id":id,"ok":false,"error":e})],
                    }
                }
                "kicad.remote.status" => {
                    let dir = cache_dir(&hub.cfg.home, &link, &project_id);
                    let meta = load_meta(&dir, &link, &project_id);
                    vec![json!({"t":"reply","id":id,"ok":true,"result":{
                        "dir": dir.to_string_lossy(),
                        "synced": dir.is_dir(),
                        "tracked": meta.bases.len(),
                        "conflicts": meta.conflicts,
                    }})]
                }
                "kicad.remote.open" => {
                    let synced = match sync_pull(hub, &link, &project_id).await {
                        Ok(r) => r,
                        Err(e) => return vec![json!({"t":"reply","id":id,"ok":false,"error":e})],
                    };
                    let dir = cache_dir(&hub.cfg.home, &link, &project_id);
                    let files = project::read(&dir.to_string_lossy());
                    let Some((tool, file)) = open::pick(&files) else {
                        return vec![json!({"t":"reply","id":id,"ok":false,"error":"no KiCad board or schematic in remote project"})];
                    };
                    let rt = super::ipc::runtime(hub).await;
                    let Some(bin) = open::program(&rt.cli, tool) else {
                        return vec![json!({"t":"reply","id":id,"ok":false,"error":"KiCad is not installed on this machine"})];
                    };
                    if let Err(e) = open::launch(&bin, &file) {
                        return vec![json!({"t":"reply","id":id,"ok":false,"error":e})];
                    }
                    start_watcher(hub.clone(), link, project_id);
                    vec![json!({"t":"reply","id":id,"ok":true,"result":{"opened":file,"tool":tool,"sync":synced}})]
                }
                _ => vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown method"})],
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};

    async fn hub_with_project(tag: &str) -> (crate::plat::tmp::TempDir, Arc<Hub>, PathBuf) {
        let dir = crate::plat::tmp::dir(&format!("rkicad-{tag}"));
        let proj = dir.join("proj");
        std::fs::create_dir_all(proj.join(".git")).unwrap();
        std::fs::create_dir_all(proj.join("libs.pretty")).unwrap();
        std::fs::write(proj.join("board.kicad_pro"), b"{\"version\":1}").unwrap();
        std::fs::write(proj.join("board.kicad_pcb"), b"(kicad_pcb (version 2024))").unwrap();
        std::fs::write(proj.join("board.kicad_sch"), b"(kicad_sch (version 2024))").unwrap();
        std::fs::write(proj.join("libs.pretty/R1.kicad_mod"), b"(footprint R1)").unwrap();
        std::fs::write(proj.join(".backplane.json"), b"{\"kind\":\"hardware\"}").unwrap();
        // Should be skipped:
        std::fs::write(proj.join("board.kicad_pcb.lck"), b"lock").unwrap();
        std::fs::write(proj.join(".git/HEAD"), b"ref: refs/heads/main").unwrap();
        std::fs::write(proj.join("notes.txt"), b"not a design file").unwrap();

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
        hub.commit(&json!({
            "$":"ProjectCreated","id":"p1","title":"Amp","root":proj.to_string_lossy(),"at":1
        }))
        .await
        .unwrap();
        (dir, hub, proj)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn manifest_includes_design_files_and_skips_git_locks_and_non_design() {
        let (_d, hub, _proj) = hub_with_project("man").await;
        let rep = serve(&hub, "kicad.manifest", &json!({"project":"p1"})).await;
        assert_eq!(rep["ok"], true, "{rep}");
        let rels: Vec<String> = rep["result"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["rel"].as_str().unwrap().to_string())
            .collect();
        assert!(rels.contains(&"board.kicad_pcb".into()), "{rels:?}");
        assert!(rels.contains(&"board.kicad_sch".into()), "{rels:?}");
        assert!(rels.contains(&"board.kicad_pro".into()), "{rels:?}");
        assert!(rels.contains(&"libs.pretty/R1.kicad_mod".into()), "{rels:?}");
        assert!(rels.contains(&".backplane.json".into()), "{rels:?}");
        assert!(!rels.iter().any(|r| r.contains(".lck") || r.contains(".git") || r == "notes.txt"), "{rels:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn push_applies_clean_save_detects_conflict_and_writes_labelled_copy() {
        let (_d, hub, proj) = hub_with_project("push").await;
        let base_sha = sha256_file(&proj.join("board.kicad_pcb")).unwrap();

        // 1. Clean save-back with matching baseSha256 succeeds
        let v2 = b"(kicad_pcb (version 2024) (track 1))";
        let v2_sha = sha256_bytes(v2);
        let r1 = serve(
            &hub,
            "kicad.push",
            &json!({
                "project":"p1",
                "files":[{"rel":"board.kicad_pcb","baseSha256":base_sha,"sha256":v2_sha,"data":base64::engine::general_purpose::STANDARD.encode(v2)}]
            }),
        )
        .await;
        assert_eq!(r1["ok"], true, "{r1}");
        assert_eq!(std::fs::read(proj.join("board.kicad_pcb")).unwrap(), v2);

        // 2. Concurrent edit with stale baseSha256 returns conflict and does NOT overwrite canonical file
        let v3 = b"(kicad_pcb (version 2024) (track 99))";
        let v3_sha = sha256_bytes(v3);
        let r2 = serve(
            &hub,
            "kicad.push",
            &json!({
                "project":"p1",
                "files":[{"rel":"board.kicad_pcb","baseSha256":base_sha,"sha256":v3_sha,"data":base64::engine::general_purpose::STANDARD.encode(v3)}]
            }),
        )
        .await;
        assert_eq!(r2["ok"], false, "{r2}");
        assert_eq!(r2["error"], "conflict");
        assert_eq!(r2["conflict"][0]["remoteSha256"], v2_sha);
        assert_eq!(std::fs::read(proj.join("board.kicad_pcb")).unwrap(), v2, "canonical file untouched");

        // 3. Explicit labelled copy saves conflict safely under copies/<label>/ without touching canonical
        let r3 = serve(
            &hub,
            "kicad.push",
            &json!({
                "project":"p1",
                "copyLabel":"laptop-edit-1",
                "files":[{"rel":"board.kicad_pcb","baseSha256":base_sha,"sha256":v3_sha,"data":base64::engine::general_purpose::STANDARD.encode(v3)}]
            }),
        )
        .await;
        assert_eq!(r3["ok"], true, "{r3}");
        assert_eq!(std::fs::read(proj.join("copies/laptop-edit-1/board.kicad_pcb")).unwrap(), v3);
        assert_eq!(std::fs::read(proj.join("board.kicad_pcb")).unwrap(), v2);
    }
}

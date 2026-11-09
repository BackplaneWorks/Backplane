//! The secrets vault (server/effects/vault.js, docs/secrets.md). Values the person pasted into the secret box are sealed in
//! `<home>/secrets/vault.json` (0600, folder 0700) with AES-256-GCM, the name as associated data, under a 32-byte master key kept in the
//! OS keychain (Secret Service through `secret-tool`, the login keychain through `security`, Windows' Credential Manager), else `<home>/secrets/vault.key` (0600).
//! The file format is the JS hub's: a vault written by either hub opens in the other. The JS hub's Windows DPAPI key is not read.
//!
//! Fail closed: a vault.json that cannot be read, is not JSON, has another version or shape, names a secret twice, or holds a value that
//! does not open with the key, makes the vault "locked": it keeps what it read (names for Settings, every value that did open for masking)
//! and refuses every change and every run. Only a missing file is a new vault. Writes are staged: a temporary file, flushed, renamed over
//! vault.json, the folder flushed; only then does the change replace what is in memory (and so the masking set).
//! Values live decrypted in this module's memory only. Nothing here logs, prints or returns a value except `run`, which injects it
//! into a child and masks that child's output before answering.
use aes_gcm::{aead::{Aead, KeyInit, Payload}, Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{io::{Read, Write}, path::{Path, PathBuf}, sync::Mutex}; use crate::plat::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

const BACKENDS: [&str; 5] = ["keyring", "keychain", "dpapi", "file", "wincred"];

#[derive(Clone)]
struct Item { name: String, purpose: String, by: String, at: String, used_at: String, used_by: String, iv: String, ct: String, value: Option<String> }
struct Vault { home: PathBuf, items: Vec<Item>, key: Option<[u8; 32]>, backend: String, locked: bool, why: String, flush: String }
static V: Mutex<Option<Vault>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut Vault) -> T) -> T {
    let mut g = V.lock().unwrap_or_else(|e| e.into_inner());
    let v = g.get_or_insert_with(|| Vault { home: PathBuf::new(), items: vec![], key: None, backend: String::new(), locked: false, why: String::new(), flush: String::new() });
    f(v)
}

/// A name the hub keeps for itself (`hub.`: Google's refresh tokens, google/store.rs): never run, listed, asked for or set through the secret box,
/// so no agent reaches it with secret_run; only the module that owns it reads it (`own_get`, `own_put`, `own_del`).
pub fn reserved(n: &str) -> bool { n.starts_with("hub.") }
/// A secret's name: `[a-z0-9._-]{1,64}`.
pub fn valid_name(n: &str) -> bool { !n.is_empty() && n.len() <= 64 && n.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_' || b == b'-') }

fn spath(v: &Vault, f: &str) -> PathBuf { v.home.join("secrets").join(f) }
fn vault_id(v: &Vault) -> String { hex::encode(Sha256::digest(v.home.to_string_lossy().as_bytes()))[..16].to_string() }

fn which(cmd: &str) -> bool {
    std::env::var_os("PATH").map_or(false, |p| std::env::split_paths(&p).any(|d| { let f = d.join(cmd); std::fs::metadata(&f).map_or(false, |m| crate::plat::fs::runnable(&m)) }))
}

/// Runs a key-store command (5 s), the input on its stdin. (ok, stdout).
fn sh(cmd: &str, args: &[&str], input: Option<&str>) -> (bool, String) {
    use std::process::{Command, Stdio};
    let Ok(mut c) = crate::proc::std_cmd(crate::proc::Kind::Local, cmd).args(args).stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else { return (false, String::new()) };
    if let (Some(i), Some(mut s)) = (input, c.stdin.take()) { let _ = s.write_all(i.as_bytes()); }
    let mut out = c.stdout.take();
    let reader = std::thread::spawn(move || { let mut b = String::new(); if let Some(o) = out.as_mut() { let _ = o.read_to_string(&mut b); } b });
    let start = std::time::Instant::now();
    let status = loop {
        match c.try_wait() { Ok(Some(s)) => break Some(s), Ok(None) => { if start.elapsed() > std::time::Duration::from_secs(5) { let _ = c.kill(); let _ = c.wait(); break None } std::thread::sleep(std::time::Duration::from_millis(10)) } Err(_) => break None }
    };
    let text = reader.join().unwrap_or_default();
    (status.map_or(false, |s| s.success()), text)
}

/// The backend this machine offers, in order of preference.
fn offer() -> String { offer_with(crate::copy::on()) }
/// A hub on a copy of a real log (BACKPLANE_COPY=1) acts on nothing outside its home: the OS keyring / keychain is the user's, so a copy keeps its vault key in a
/// file in its own home and never reads or writes the real keyring.
fn offer_with(copy: bool) -> String {
    if copy { return "file".into() }
    if std::env::var("BACKPLANE_VAULT_KEYRING").as_deref() == Ok("file") { return "file".into() }
    if cfg!(target_os = "macos") && which("security") { return "keychain".into() }
    if cfg!(windows) { return "wincred".into() }
    if cfg!(target_os = "linux") && which("secret-tool") && (std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() || std::env::var_os("XDG_RUNTIME_DIR").is_some()) { return "keyring".into() }
    "file".into()
}

fn hex_key(s: &str) -> Option<[u8; 32]> { let t = s.trim(); if t.len() != 64 { return None } let b = hex::decode(t).ok()?; b.try_into().ok() }

fn key_get(v: &Vault, backend: &str) -> Option<[u8; 32]> { key_get_with(crate::copy::on(), v, backend) }
fn key_get_with(copy: bool, v: &Vault, backend: &str) -> Option<[u8; 32]> {
    if copy && backend != "file" { return None } // a copy never reads the real keyring: a vault whose key lives there stays locked
    match backend {
        "file" => hex_key(&std::fs::read_to_string(spath(v, "vault.key")).ok()?),
        "keyring" => { let (ok, o) = sh("secret-tool", &["lookup", "service", "backplane-vault", "home", &vault_id(v)], None); if ok { hex_key(&o) } else { None } }
        "keychain" => { let (ok, o) = sh("security", &["find-generic-password", "-s", "backplane-vault", "-a", &vault_id(v), "-w"], None); if ok { hex_key(&o) } else { None } }
        "wincred" => hex_key(&crate::plat::creds::get(&format!("backplane-vault/{}", vault_id(v)))?),
        _ => None,
    }
}

fn key_put(v: &Vault, backend: &str, key: &[u8; 32]) -> bool { key_put_with(crate::copy::on(), v, backend, key) }
fn key_put_with(copy: bool, v: &Vault, backend: &str, key: &[u8; 32]) -> bool {
    if copy && backend != "file" { return false } // nor writes it
    let h = hex::encode(key);
    let stored = match backend {
        "file" => { let r = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(spath(v, "vault.key")).and_then(|mut f| f.write_all(format!("{h}\n").as_bytes())); r.is_ok() }
        "keyring" => sh("secret-tool", &["store", "--label=Backplane vault", "service", "backplane-vault", "home", &vault_id(v)], Some(&h)).0,
        // through security's own prompt on stdin, so the key is never in argv
        "keychain" => sh("security", &["-i"], Some(&format!("add-generic-password -s backplane-vault -a {} -l \"Backplane vault\" -w {h}\n", vault_id(v)))).0,
        "wincred" => crate::plat::creds::put(&format!("backplane-vault/{}", vault_id(v)), &h),
        _ => false,
    };
    stored && key_get(v, backend) == Some(*key)
}

/// Random bytes from the OS. No fallback: a master key or an iv from anything weaker would be predictable (crypto.randomBytes throws too).
fn random(n: usize) -> Result<Vec<u8>, String> {
    let mut b = vec![0u8; n];
    crate::plat::fs::random(&mut b).map_err(|_| "no secure random source".to_string())?;
    Ok(b)
}

fn seal(key: &[u8; 32], name: &str, value: &str) -> Result<(String, String), String> {
    let iv = random(12)?;
    let c = Aes256Gcm::new_from_slice(key).map_err(|_| "bad key".to_string())?;
    let ct = c.encrypt(Nonce::from_slice(&iv), Payload { msg: value.as_bytes(), aad: name.as_bytes() }).map_err(|_| "could not seal".to_string())?;
    Ok((B64.encode(&iv), B64.encode(ct)))
}

fn open1(key: &[u8; 32], name: &str, iv: &str, ct: &str) -> Result<String, String> {
    let all = B64.decode(ct).map_err(|_| "ct".to_string())?;
    let iv = B64.decode(iv).map_err(|_| "iv".to_string())?;
    if all.len() < 17 || iv.len() != 12 { return Err("short".into()) }
    let c = Aes256Gcm::new_from_slice(key).map_err(|_| "bad key".to_string())?;
    let pt = c.decrypt(Nonce::from_slice(&iv), Payload { msg: &all, aad: name.as_bytes() }).map_err(|_| "does not open".to_string())?;
    String::from_utf8(pt).map_err(|_| "utf8".to_string())
}

fn meta(it: &Item) -> Value { json!({"name":it.name,"purpose":it.purpose,"by":it.by,"at":it.at,"usedAt":it.used_at,"usedBy":it.used_by}) }

/// new Date().toISOString()
pub fn iso_now() -> String {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let (secs, ms) = (d.as_secs() as i64, d.subsec_millis());
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let z = days + 719468; let era = z.div_euclid(146097); let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1; let m = if mp < 10 { mp + 3 } else { mp - 9 }; let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{day:02}T{:02}:{:02}:{:02}.{ms:03}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn fault(step: &str) -> Result<(), std::io::Error> {
    if std::env::var("BACKPLANE_VAULT_TEST").as_deref() != Ok("1") { return Ok(()) }
    let f = std::env::var("BACKPLANE_VAULT_FAULT").unwrap_or_default();
    if step == "dirsync" && (f == "dirsync" || f == "dirsync-unsupported") { return Err(std::io::Error::from_raw_os_error(if f == "dirsync" { crate::plat::errno::EIO } else { crate::plat::errno::EINVAL })) }
    if f == step { return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("injected {step} failure"))) }
    Ok(())
}

fn encode(items: &[Item], backend: &str) -> String {
    use serde::Serialize;
    let list: Vec<Value> = items.iter().map(|it| { let mut m = meta(it); if let Some(o) = m.as_object_mut() { o.insert("iv".into(), json!(it.iv)); o.insert("ct".into(), json!(it.ct)); } m }).collect();
    let doc = json!({"v":1,"key":backend,"items":list});
    let mut buf = vec![];
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    let _ = doc.serialize(&mut ser);
    let mut t = String::from_utf8(buf).unwrap_or_default(); t.push('\n'); t
}

/// Writes items as vault.json durably; Err on any failure before the rename (the temporary file removed). Ok(flush outcome).
fn write(v: &Vault, items: &[Item], backend: &str) -> Result<String, String> {
    let text = encode(items, backend);
    let dir = v.home.join("secrets");
    let tmp = dir.join(format!("vault.json.{}.tmp", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    let r: Result<(), String> = (|| {
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp).map_err(|e| e.to_string())?;
        fault("write").map_err(|e| e.to_string())?;
        f.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        fault("fsync").map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        fault("rename").map_err(|e| e.to_string())?;
        crate::plat::fs::rename(&tmp, dir.join("vault.json")).map_err(|e| e.to_string())
    })();
    if let Err(e) = r { let _ = std::fs::remove_file(&tmp); return Err(e) }
    // the rename is committed: from here the new items are what is on disk. The folder flush decides only what is claimed.
    let flushed = fault("dirsync").and_then(|_| crate::plat::fs::sync_dir(&dir));
    Ok(match flushed {
        Ok(()) => "durable".into(),
        Err(e) => { let code = match e.raw_os_error() { Some(c) if c == crate::plat::errno::EINVAL => "EINVAL".to_string(), Some(c) if c == crate::plat::errno::ENOTSUP => "ENOTSUP".to_string(), Some(c) if c == crate::plat::errno::EISDIR => "EISDIR".to_string(), _ => "EIO".to_string() };
            format!("{} {code}", if ["EINVAL", "ENOTSUP", "EISDIR"].contains(&code.as_str()) { "unsupported" } else { "unflushed" }) }
    })
}

fn ack(v: &mut Vault, outcome: String) -> String {
    v.flush = outcome.clone();
    if let Some(code) = outcome.strip_prefix("unflushed ") { format!("committed the folder flush failed ({code}): saved, but not known to survive a crash") } else { "ok".into() }
}

fn lock(v: &mut Vault, why: &str) { v.locked = true; if v.why.is_empty() { v.why = why.into() } }

/// The schema, strictly: {v: 1, key: <backend>, items: [{name, iv, ct, purpose?, by?, at?, usedAt?, usedBy?}]}, names unique; "" when it holds.
fn bad_doc(doc: &Value) -> String {
    let Some(o) = doc.as_object() else { return "vault.json is not an object".into() };
    if o.get("v") != Some(&json!(1)) { return format!("vault.json has version {}, not 1", o.get("v").map_or("undefined".to_string(), |x| x.to_string())) }
    if !o.get("key").and_then(Value::as_str).map_or(false, |k| BACKENDS.contains(&k)) { return "vault.json names no known key store".into() }
    let Some(items) = o.get("items").and_then(Value::as_array) else { return "vault.json has no item list".into() };
    let mut seen = std::collections::HashSet::new();
    for it in items {
        let Some(m) = it.as_object() else { return "vault.json has an item that is not an object".into() };
        let Some(name) = m.get("name").and_then(Value::as_str).filter(|n| valid_name(n)) else { return "vault.json has an item without a valid name".into() };
        if !seen.insert(name.to_string()) { return format!("vault.json names {name} twice") }
        if !m.get("iv").map_or(false, Value::is_string) || !m.get("ct").map_or(false, Value::is_string) { return format!("vault.json's {name} has no sealed value") }
        for k in ["purpose", "by", "at", "usedAt", "usedBy"] { if let Some(x) = m.get(k) { if !x.is_string() { return format!("vault.json's {name} has a bad {k}") } } }
    }
    String::new()
}

fn item_of(m: &Value) -> Item {
    let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Item { name: s("name"), purpose: s("purpose"), by: s("by"), at: s("at"), used_at: s("usedAt"), used_by: s("usedBy"), iv: s("iv"), ct: s("ct"), value: None }
}

/// Every value is masked in what agents read; a hub-owned item that is a JSON object (google/store.rs) has its secret field (`refresh`) masked on its own too.
fn publish_masks(v: &Vault) {
    let mut vals: Vec<String> = v.items.iter().filter_map(|i| i.value.clone()).collect();
    for i in v.items.iter().filter(|i| reserved(&i.name)) { if let Some(r) = i.value.as_deref().and_then(|t| serde_json::from_str::<Value>(t).ok()).and_then(|j| j["refresh"].as_str().map(str::to_string)) { vals.push(r) } }
    super::mask::set_vault_values(vals);
}

fn status_of(v: &Vault) -> Value { json!({"key": if v.backend.is_empty() { offer() } else { v.backend.clone() }, "locked": v.locked, "why": v.why, "n": v.items.len(), "flush": v.flush}) }

/// VaultFx.open(home): read the vault and its key once; the status. The hub opens it before any turn resumes.
pub fn open(home: &Path) -> Value {
    with(|v| {
        *v = Vault { home: home.to_path_buf(), items: vec![], key: None, backend: String::new(), locked: false, why: String::new(), flush: String::new() };
        let text = match std::fs::read_to_string(spath(v, "vault.json")) {
            Ok(t) => Some(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => { let c = e.raw_os_error().map_or("error".to_string(), |c| c.to_string()); lock(v, &format!("vault.json could not be read ({c})")); None }
        };
        let Some(text) = text else { publish_masks(v); return status_of(v) };
        let Ok(doc) = serde_json::from_str::<Value>(&text) else { lock(v, "vault.json is not valid JSON"); return status_of(v) };
        let bad = bad_doc(&doc);
        if !bad.is_empty() {
            lock(v, &bad);
            // what can be shown is kept for Settings (names only), nothing opened
            if let Some(items) = doc.get("items").and_then(Value::as_array) { for it in items { if it.get("name").and_then(Value::as_str).map_or(false, valid_name) && !v.items.iter().any(|x| Some(x.name.as_str()) == it.get("name").and_then(Value::as_str)) { v.items.push(item_of(it)); } } }
            return status_of(v);
        }
        v.backend = doc["key"].as_str().unwrap_or("").to_string();
        for it in doc["items"].as_array().cloned().unwrap_or_default() { v.items.push(item_of(&it)); }
        if v.items.is_empty() { return status_of(v) }
        v.key = key_get(v, &v.backend.clone());
        let Some(key) = v.key else { let why = if v.backend == "file" { "the vault's key file is missing".to_string() } else { format!("the {} did not give the vault's key", v.backend) }; lock(v, &why); return status_of(v) };
        let mut failed = 0;
        for it in v.items.iter_mut() { match open1(&key, &it.name, &it.iv, &it.ct) { Ok(x) => it.value = Some(x), Err(_) => { it.value = None; failed += 1 } } }
        if failed > 0 { let n = v.items.len(); lock(v, &format!("{failed} of {n} secrets do not open with the vault's key")); }
        publish_masks(v);
        status_of(v)
    })
}

/// The 0.14 import: every secret of another home's vault (`<home>/secrets/vault.json`), opened with that home's own key (its keychain entry is
/// named by its path, or its vault.key file): (name, purpose, value). Reads only; Err says why it would not open (no vault is Ok(empty)).
pub fn read_other(home: &Path) -> Result<Vec<(String, String, String)>, String> {
    let other = Vault { home: home.to_path_buf(), items: vec![], key: None, backend: String::new(), locked: false, why: String::new(), flush: String::new() };
    let text = match std::fs::read_to_string(spath(&other, "vault.json")) { Ok(t) => t, Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]), Err(e) => return Err(format!("its vault could not be read ({e})")) };
    let doc: Value = serde_json::from_str(&text).map_err(|_| "its vault is not valid JSON".to_string())?;
    let bad = bad_doc(&doc); if !bad.is_empty() { return Err(bad) }
    let items: Vec<Item> = doc["items"].as_array().cloned().unwrap_or_default().iter().map(item_of).collect();
    if items.is_empty() { return Ok(vec![]) }
    let backend = doc["key"].as_str().unwrap_or("").to_string();
    if backend == "dpapi" { return Err("its key is in Windows DPAPI, which this version cannot open".into()) }
    let key = key_get(&other, &backend).ok_or_else(|| if backend == "file" { "its key file is missing".to_string() } else { format!("the {backend} did not give its key") })?;
    items.iter().map(|it| open1(&key, &it.name, &it.iv, &it.ct).map(|v| (it.name.clone(), it.purpose.clone(), v)).map_err(|_| format!("{} does not open with its key", it.name))).collect()
}
pub fn status() -> Value { with(|v| status_of(v)) }
/// VaultFx.list: the metadata, never a value.
pub fn list() -> Value { with(|v| Value::Array(v.items.iter().filter(|i| !reserved(&i.name)).map(meta).collect())) }
/// The names the secret box knows (never a reserved one).
pub fn names() -> Vec<String> { with(|v| v.items.iter().filter(|i| !reserved(&i.name)).map(|i| i.name.clone()).collect()) }
/// A hub-owned item's value (`reserved` names only); None when absent, unopened or the vault is locked.
pub fn own_get(name: &str) -> Option<String> { if !reserved(name) { return None } with(|v| if v.locked { None } else { v.items.iter().find(|i| i.name == name).and_then(|i| i.value.clone()) }) }
/// The reserved names held, with their prefix.
pub fn own_names(prefix: &str) -> Vec<String> { with(|v| v.items.iter().filter(|i| reserved(&i.name) && i.name.starts_with(prefix)).map(|i| i.name.clone()).collect()) }
/// A person-set secret's value, for the hub's own use (Google's OAuth client, google/store.rs). Never a reserved one.
pub fn person_get(name: &str) -> Option<String> { if reserved(name) { return None } with(|v| if v.locked { None } else { v.items.iter().find(|i| i.name == name).and_then(|i| i.value.clone()) }) }
pub fn own_put(name: &str, value: &str, purpose: &str) -> String { if !reserved(name) { return "fail not a hub name".into() } put_any(name, value, purpose, "hub") }
pub fn own_del(name: &str) -> String { if !reserved(name) { return "fail not a hub name".into() } del_any(name) }

fn errs(e: &str) -> String { e.replace(['\r', '\n'], " ").chars().take(200).collect() }

/// VaultFx.put: "ok", "committed <why>" or "fail <why>". The caller has checked the name and the value (`secret::bad`). A reserved name is refused.
pub fn put(name: &str, value: &str, purpose: &str, by: &str) -> String { if reserved(name) { return "fail that name is Backplane's own".into() } put_any(name, value, purpose, by) }
fn put_any(name: &str, value: &str, purpose: &str, by: &str) -> String {
    with(|v| {
        if v.locked { return format!("fail the vault is locked: {}", v.why) }
        if !valid_name(name) { return "fail not a secret's name".into() }
        let (key, backend) = match v.key {
            Some(k) => (k, v.backend.clone()),
            None => {
                // a new vault (no item yet): its key, from the store chosen now
                let mut backend = if v.backend.is_empty() { offer() } else { v.backend.clone() };
                if let Err(e) = std::fs::create_dir_all(v.home.join("secrets")).and_then(|_| std::fs::set_permissions(v.home.join("secrets"), std::fs::Permissions::from_mode(0o700))) { return format!("fail {}", errs(&e.to_string())) }
                let key = match key_get(v, &backend) {
                    Some(k) => k,
                    None => {
                        let Ok(k) = random(32).and_then(|r| <[u8; 32]>::try_from(r).map_err(|_| "len".to_string())) else { return "fail no secure random source".into() };
                        if !key_put(v, &backend, &k) {
                            if backend == "file" { return "fail could not keep the vault's key".into() }
                            // a keychain that will not take it: the key file instead
                            backend = "file".into();
                            match key_get(v, "file") { Some(old) => old, None => { if !key_put(v, "file", &k) { return "fail could not keep the vault's key".into() } k } }
                        } else { k }
                    }
                };
                (key, backend)
            }
        };
        let old = v.items.iter().find(|i| i.name == name).cloned();
        let (iv, ct) = match seal(&key, name, value) { Ok(x) => x, Err(e) => return format!("fail {e}") };
        let it = Item { name: name.into(), purpose: if purpose.is_empty() { old.as_ref().map_or(String::new(), |o| o.purpose.clone()) } else { purpose.into() },
            by: if by.is_empty() { old.as_ref().map_or(String::new(), |o| o.by.clone()) } else { by.into() }, at: iso_now(),
            used_at: old.as_ref().map_or(String::new(), |o| o.used_at.clone()), used_by: old.as_ref().map_or(String::new(), |o| o.used_by.clone()), iv, ct, value: Some(value.into()) };
        let mut staged = v.items.clone();
        match staged.iter().position(|i| i.name == name) { Some(p) => staged[p] = it, None => staged.push(it) }
        match write(v, &staged, &backend) {
            Err(e) => format!("fail {}", errs(&e)),
            Ok(outcome) => { v.items = staged; v.key = Some(key); v.backend = backend; publish_masks(v); ack(v, outcome) }
        }
    })
}

/// VaultFx.del: "ok", "committed <why>" or "fail <why>". A reserved name is refused (its owner removes it).
pub fn del(name: &str) -> String { if reserved(name) { return format!("fail no secret named {name}") } del_any(name) }
fn del_any(name: &str) -> String {
    with(|v| {
        if v.locked { return format!("fail the vault is locked: {}", v.why) }
        if !v.items.iter().any(|i| i.name == name) { return format!("fail no secret named {name}") }
        let staged: Vec<Item> = v.items.iter().filter(|i| i.name != name).cloned().collect();
        let backend = if v.backend.is_empty() { offer() } else { v.backend.clone() };
        match write(v, &staged, &backend) {
            Err(e) => format!("fail {}", errs(&e)),
            Ok(outcome) => { v.items = staged; publish_masks(v); ack(v, outcome) }
        }
    })
}

// ---- secret_run ----

fn encode_uri_component(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() { if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) { o.push(b as char) } else { o.push_str(&format!("%{b:02X}")) } }
    o
}

/// The forms of a value masked in a run's output: as it is, JSON-escaped and percent-encoded, 8+ characters.
fn forms(vals: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for v in vals {
        if v.chars().count() < super::mask::MIN { continue }
        let js = serde_json::to_string(v).map(|s| s[1..s.len() - 1].to_string()).unwrap_or_default();
        for f in [v.clone(), js, encode_uri_component(v)] { if !f.is_empty() && !out.contains(&f) { out.push(f) } }
    }
    out
}

fn env_names(o: &Value) -> Vec<(String, String)> {
    let ok = |k: &str| k.chars().next().map_or(false, |c| c.is_ascii_alphabetic() || c == '_') && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    o.as_object().map(|m| m.iter().filter_map(|(k, v)| v.as_str().filter(|_| ok(k)).map(|s| (k.clone(), s.to_string()))).collect()).unwrap_or_default()
}

/// Signal a run's own process group: only a real child's pid (> 1); never 0, 1 or a missing pid.
pub fn kill_group(pid: Option<u32>, sig: i32) -> bool {
    match pid { Some(p) if p > 1 => crate::plat::proc::kill_group(p, sig), _ => false }
}

struct Tail { bytes: Vec<u8>, cut: usize }
const CAP: usize = 65536;
fn tail_text(t: &Tail) -> String {
    let s = String::from_utf8_lossy(&t.bytes).to_string();
    let n = s.chars().count();
    if n > CAP { let kept: String = s.chars().skip(n - CAP).collect(); format!("[… {} characters cut]\n{kept}", t.cut + (n - CAP)) }
    else if t.cut > 0 { format!("[… {} characters cut]\n{s}", t.cut) } else { s }
}

/// VaultFx.run: `{command, env?, files?, cwd?, timeout?, thread?}` -> `{code, timedOut, stdout, stderr}` or `{error}`, every string masked
/// with the values this run injected and every value now (so a value replaced or deleted during the run stays masked).
pub async fn run(spec: Value) -> Value {
    use tokio::io::AsyncReadExt;
    let command = spec.get("command").and_then(Value::as_str).unwrap_or("").to_string();
    if command.trim().is_empty() { return json!({"error":"command is required"}) }
    let (envs, files) = (env_names(&spec["env"]), env_names(&spec["files"]));
    let use_names: Vec<String> = envs.iter().chain(files.iter()).map(|(_, n)| n.clone()).collect();
    // look the values up under the lock, then let go of it
    let looked: Result<Vec<String>, String> = with(|v| {
        if v.locked { return Err(format!("the vault is locked: {}", v.why)) }
        let mut out = vec![];
        for n in &use_names { if reserved(n) { return Err(format!("no secret named {} (ask for it with secret_request)", n.chars().take(64).collect::<String>())) } match v.items.iter().find(|i| &i.name == n).and_then(|i| i.value.clone()) { Some(x) => out.push(x), None => return Err(format!("no secret named {} (ask for it with secret_request)", n.chars().take(64).collect::<String>())) } }
        Ok(out)
    });
    let injected = match looked { Ok(x) => x, Err(e) => return json!({"error":e}) };
    let answer = |o: Value| -> Value {
        let mut all = injected.clone(); all.extend(super::mask::all());
        let f = forms(&all);
        let mut m = serde_json::Map::new();
        if let Some(obj) = o.as_object() { for (k, x) in obj { m.insert(k.clone(), match x { Value::String(s) => Value::String(super::mask::mask_with(&f, s)), other => other.clone() }); } }
        Value::Object(m)
    };
    let mut cmd = crate::proc::tokio_cmd(crate::proc::Kind::Shell, "/bin/sh");
    cmd.kill_on_drop(true).arg("-c").arg(&command).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    if let Some(cwd) = spec.get("cwd").and_then(Value::as_str).filter(|c| !c.is_empty()) { cmd.current_dir(cwd); }
    for (var, name) in &envs { if let Some(i) = use_names.iter().position(|n| n == name) { cmd.env(var, &injected[i]); } }
    let mut dir: Option<PathBuf> = None;
    // dropped with the future (a cancelled MCP handler): the folder goes and the child's group is killed
    struct Guard { dir: Option<PathBuf>, pid: Option<u32> }
    impl Drop for Guard { fn drop(&mut self) { if let Some(p) = self.pid { kill_group(Some(p), crate::plat::proc::KILL); } if let Some(d) = &self.dir { let _ = std::fs::remove_dir_all(d); } } }
    let mut guard = Guard { dir: None, pid: None };
    if !files.is_empty() {
        let home = with(|v| v.home.clone());
        let made: std::io::Result<PathBuf> = (|| {
            let base = home.join("secrets").join("run");
            std::fs::create_dir_all(&base)?; let _ = std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700));
            let d = base.join(format!("r-{}-{}", std::process::id(), format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()))));
            std::fs::DirBuilder::new().mode(0o700).create(&d)?;
            Ok(d)
        })();
        match made {
            Ok(d) => {
                for (var, _name) in &files {
                    let f = d.join(var.to_lowercase());
                    let w = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&f).and_then(|mut h| h.write_all(injected[envs.len() + files.iter().position(|(v2, _)| v2 == var).unwrap_or(0)].as_bytes()));
                    if let Err(e) = w { let _ = std::fs::remove_dir_all(&d); return answer(json!({"error": format!("could not prepare the run: {}", errs(&e.to_string()))})) }
                    cmd.env(var, &f);
                }
                guard.dir = Some(d.clone()); dir = Some(d);
            }
            Err(e) => return answer(json!({"error": format!("could not prepare the run: {}", errs(&e.to_string()))})),
        }
    }
    // last use, written only if the write succeeds (a failure changes nothing)
    with(|v| {
        if v.locked { return }
        let now = iso_now(); let by = spec.get("thread").and_then(Value::as_str).unwrap_or("").to_string();
        let mut staged = v.items.clone();
        for it in staged.iter_mut() { if use_names.contains(&it.name) { it.used_at = now.clone(); it.used_by = by.clone(); } }
        let backend = if v.backend.is_empty() { offer() } else { v.backend.clone() };
        if write(v, &staged, &backend).is_ok() { v.items = staged; }
    });
    let secs = spec.get("timeout").and_then(|t| t.as_f64().or_else(|| t.as_str().and_then(|s| s.parse().ok()))).filter(|t| *t > 0.0).unwrap_or(120.0).clamp(1.0, 600.0);
    // a group of its own, like the JS hub's `detached: true`
    crate::plat::proc::own_group_tokio(&mut cmd);
    let cleanup = |d: &Option<PathBuf>| { if let Some(d) = d { let _ = std::fs::remove_dir_all(d); } };
    let mut child = match cmd.spawn() { Ok(c) => c, Err(e) => { cleanup(&dir); return answer(json!({"error": format!("could not start: {}", errs(&e.to_string()))})) } };
    let pid = child.id(); guard.pid = pid; crate::plat::proc::adopt(pid.unwrap_or(0), false);
    let (out, err) = (std::sync::Arc::new(Mutex::new(Tail { bytes: vec![], cut: 0 })), std::sync::Arc::new(Mutex::new(Tail { bytes: vec![], cut: 0 })));
    let pump = |r: Option<tokio::process::ChildStdout>, t: std::sync::Arc<Mutex<Tail>>| tokio::spawn(async move {
        let Some(mut r) = r else { return };
        let mut buf = vec![0u8; 8192];
        while let Ok(n) = r.read(&mut buf).await { if n == 0 { break } let mut g = t.lock().unwrap(); g.bytes.extend_from_slice(&buf[..n]); if g.bytes.len() > 8 * CAP { let drop = g.bytes.len() - 4 * CAP; g.bytes.drain(..drop); g.cut += drop; } }
    });
    let pump_err = |r: Option<tokio::process::ChildStderr>, t: std::sync::Arc<Mutex<Tail>>| tokio::spawn(async move {
        let Some(mut r) = r else { return };
        let mut buf = vec![0u8; 8192];
        while let Ok(n) = r.read(&mut buf).await { if n == 0 { break } let mut g = t.lock().unwrap(); g.bytes.extend_from_slice(&buf[..n]); if g.bytes.len() > 8 * CAP { let drop = g.bytes.len() - 4 * CAP; g.bytes.drain(..drop); g.cut += drop; } }
    });
    let (mut h_out, mut h_err) = (pump(child.stdout.take(), out.clone()), pump_err(child.stderr.take(), err.clone()));
    let sleep = |ms: u64| tokio::time::sleep(std::time::Duration::from_millis(ms));
    let mut timed_out = false;
    let status = tokio::select! {
        s = child.wait() => s.ok(),
        _ = sleep((secs * 1000.0) as u64) => {
            timed_out = true;
            kill_group(pid, crate::plat::proc::TERM);
            tokio::select! {
                s = child.wait() => s.ok(),
                _ = sleep(2000) => { kill_group(pid, crate::plat::proc::KILL); tokio::select! { s = child.wait() => s.ok(), _ = sleep(3000) => None } }
            }
        }
    };
    // the command is done; its streams get 2 s, then the group goes
    let both = async { let _ = (&mut h_out).await; let _ = (&mut h_err).await; };
    if tokio::time::timeout(std::time::Duration::from_millis(if timed_out { 100 } else { 2000 }), both).await.is_err() { kill_group(pid, crate::plat::proc::KILL); h_out.abort(); h_err.abort(); }
    cleanup(&dir); guard.pid = None; guard.dir = None;
    let code: i64 = if timed_out { 124 } else { match status { Some(s) => crate::plat::exit_code(&s) as i64, None => 255 } };
    let (o, e) = (tail_text(&out.lock().unwrap()), tail_text(&err.lock().unwrap()));
    answer(json!({"code": code, "timedOut": timed_out, "stdout": o, "stderr": e}))
}

#[cfg(test)]
pub(crate) fn test_home(tag: &str) -> PathBuf {
    let h = std::env::temp_dir().join(format!("bp-vault-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&h); std::fs::create_dir_all(&h).unwrap(); h
}
/// The vault is process-wide, so tests that open one take this lock.
#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    fn env_file() { std::env::set_var("BACKPLANE_VAULT_KEYRING", "file"); }

    /// Windows: the master key goes to the Credential Manager and comes back, and a copy of a log neither reads nor writes it there.
    #[cfg(windows)]
    #[test] fn the_windows_credential_manager_keeps_the_key_and_a_copy_never_touches_it() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("bp-wincred-{}-{}", std::process::id(), iso_now().replace([':', '.'], "-"))); std::fs::create_dir_all(&dir).unwrap();
        let v = Vault { home: dir.clone(), items: vec![], key: None, backend: String::new(), locked: false, why: String::new(), flush: String::new() };
        std::env::remove_var("BACKPLANE_VAULT_KEYRING"); assert_eq!(offer_with(false), "wincred");
        let target = format!("backplane-vault/{}", vault_id(&v));
        assert!(!key_put_with(true, &v, "wincred", &[9u8; 32]) && key_get_with(true, &v, "wincred").is_none(), "a copy: nothing written or read");
        assert!(crate::plat::creds::get(&target).is_none());
        assert!(key_put_with(false, &v, "wincred", &[7u8; 32]), "stored and read back"); assert_eq!(key_get_with(false, &v, "wincred"), Some([7u8; 32]));
        assert!(crate::plat::creds::delete(&target)); assert!(key_get_with(false, &v, "wincred").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test] fn a_copy_never_touches_the_real_keyring() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("bp-fakekr-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        let mark = dir.join("called");
        for n in ["secret-tool", "security"] { let f = dir.join(n); std::fs::write(&f, format!("#!/bin/sh\necho $0 >> {}\nexit 1\n", crate::plat::fs::script_path(&mark))).unwrap(); std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        let old = std::env::var_os("PATH").unwrap_or_default(); let mut p = vec![dir.clone()]; p.extend(std::env::split_paths(&old)); std::env::set_var("PATH", std::env::join_paths(p).unwrap());
        std::env::remove_var("BACKPLANE_VAULT_KEYRING");
        let v = Vault { home: dir.clone(), items: vec![], key: None, backend: String::new(), locked: false, why: String::new(), flush: String::new() };
        assert_eq!(offer_with(true), "file", "a copy keeps its key in its own home");
        for b in ["keyring", "keychain"] { assert_eq!(key_get_with(true, &v, b), None, "{b}: not read"); assert!(!key_put_with(true, &v, b, &[7u8; 32]), "{b}: not written"); }
        assert!(!mark.exists(), "neither secret-tool nor security was run");
        // the same fake tools ARE reached when this is not a copy, so the check above can fail
        let _ = key_get_with(false, &v, "keyring"); assert!(mark.exists(), "the probe works: a real hub does reach the keyring tool");
        std::env::set_var("PATH", old); let _ = std::fs::remove_dir_all(&dir);
    }
    #[test] fn iso_now_has_the_javascript_shape() { let t = iso_now(); assert_eq!(t.len(), 24); assert!(t.ends_with('Z') && &t[10..11] == "T" && t.starts_with("20")); }
    #[test] fn a_value_is_sealed_at_rest_and_opens_again() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); env_file();
        let h = test_home("seal");
        assert_eq!(open(&h)["n"], 0);
        assert_eq!(put("tailscale-bub", "tskey-auth-ABCDEF123456", "join the tailnet", "person"), "ok");
        let raw = std::fs::read_to_string(h.join("secrets/vault.json")).unwrap();
        assert!(!raw.contains("tskey-auth"), "the value is not in the file");
        assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(h.join("secrets/vault.json")).unwrap().permissions().mode() & 0o777, 0o600), 0o600);
        assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(h.join("secrets/vault.key")).unwrap().permissions().mode() & 0o777, 0o600), 0o600);
        // a fresh open (a restart) opens it and masks it
        let s = open(&h); assert_eq!((s["locked"].clone(), s["n"].clone()), (json!(false), json!(1)));
        assert_eq!(super::super::mask::mask_with(&super::super::mask::all(), "key tskey-auth-ABCDEF123456 end"), format!("key {} end", "\u{2022}".repeat(23)));
        assert_eq!(list()[0]["purpose"], "join the tailnet"); assert!(!list().to_string().contains("tskey"));
        assert_eq!(del("tailscale-bub"), "ok"); assert_eq!(list(), json!([]));
        assert!(!super::super::mask::all().contains(&"tskey-auth-ABCDEF123456".to_string()));
        let _ = std::fs::remove_dir_all(&h);
    }
    #[test] fn a_vault_the_js_hub_sealed_opens_here() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); env_file();
        let h = test_home("js"); let key = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        std::fs::create_dir_all(h.join("secrets")).unwrap(); std::fs::write(h.join("secrets/vault.key"), format!("{key}\n")).unwrap();
        let k = hex_key(key).unwrap(); let (iv, ct) = seal(&k, "from-js", "value-from-the-js-hub").unwrap();
        // sealed by the same layout vault.js writes (iv 12 bytes, ct || tag, the name as associated data)
        let doc = json!({"v":1,"key":"file","items":[{"name":"from-js","purpose":"p","by":"person","at":"2026-11-05T00:00:00.000Z","usedAt":"","usedBy":"","iv":iv,"ct":ct}]});
        std::fs::write(h.join("secrets/vault.json"), serde_json::to_string_pretty(&doc).unwrap()).unwrap();
        assert_eq!(open(&h)["locked"], false);
        assert_eq!(super::super::mask::mask_with(&super::super::mask::all(), "x value-from-the-js-hub"), format!("x {}", "\u{2022}".repeat(21)));
        let _ = std::fs::remove_dir_all(&h);
    }
    /// The 0.14 import reads another home's vault with that home's own key and leaves it, and this vault, as they were.
    #[test] fn another_homes_vault_is_read_with_its_own_key() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); env_file();
        let h = test_home("other"); assert_eq!(read_other(&h), Ok(vec![]), "no vault: nothing");
        let key = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";
        std::fs::create_dir_all(h.join("secrets")).unwrap(); std::fs::write(h.join("secrets/vault.key"), format!("{key}\n")).unwrap();
        let k = hex_key(key).unwrap(); let (iv, ct) = seal(&k, "fab-token", "tok-123456").unwrap();
        let doc = json!({"v":1,"key":"file","items":[{"name":"fab-token","purpose":"orders","iv":iv,"ct":ct}]});
        std::fs::write(h.join("secrets/vault.json"), doc.to_string()).unwrap();
        let before = std::fs::read(h.join("secrets/vault.json")).unwrap();
        assert_eq!(read_other(&h), Ok(vec![("fab-token".into(), "orders".into(), "tok-123456".into())]));
        assert_eq!(std::fs::read(h.join("secrets/vault.json")).unwrap(), before);
        std::fs::write(h.join("secrets/vault.key"), format!("{}\n", "00".repeat(32))).unwrap();
        assert!(read_other(&h).unwrap_err().contains("does not open"));
        std::fs::write(h.join("secrets/vault.json"), json!({"v":1,"key":"dpapi","items":[{"name":"x","iv":"a","ct":"b"}]}).to_string()).unwrap();
        assert!(read_other(&h).unwrap_err().contains("DPAPI"));
        let _ = std::fs::remove_dir_all(&h);
    }
    #[test] fn a_vault_that_does_not_parse_or_open_locks_and_nothing_is_written_over_it() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); env_file();
        for (tag, body, why) in [("junk", "not json", "not valid JSON"), ("ver", "{\"v\":2,\"key\":\"file\",\"items\":[]}", "version 2"), ("key", "{\"v\":1,\"key\":\"x\",\"items\":[]}", "no known key store"), ("dup", "{\"v\":1,\"key\":\"file\",\"items\":[{\"name\":\"a\",\"iv\":\"\",\"ct\":\"\"},{\"name\":\"a\",\"iv\":\"\",\"ct\":\"\"}]}", "twice")] {
            let h = test_home(tag); std::fs::create_dir_all(h.join("secrets")).unwrap(); std::fs::write(h.join("secrets/vault.json"), body).unwrap();
            let s = open(&h); assert_eq!(s["locked"], true, "{tag}"); assert!(s["why"].as_str().unwrap().contains(why), "{tag}: {s}");
            assert!(put("new-one", "abcdefgh-1234", "", "person").starts_with("fail the vault is locked"));
            assert!(del("a").starts_with("fail the vault is locked"));
            assert_eq!(std::fs::read_to_string(h.join("secrets/vault.json")).unwrap(), body, "evidence kept");
            let _ = std::fs::remove_dir_all(&h);
        }
        // a value that does not open with the key
        let h = test_home("wrongkey"); env_file(); open(&h); assert_eq!(put("one-one", "abcdefgh-1234", "", "person"), "ok");
        std::fs::write(h.join("secrets/vault.key"), format!("{}\n", "ab".repeat(32))).unwrap();
        let s = open(&h); assert_eq!(s["locked"], true); assert!(s["why"].as_str().unwrap().contains("do not open"));
        assert_eq!(list()[0]["name"], "one-one"); let _ = std::fs::remove_dir_all(&h);
    }
    #[test] fn a_missing_key_file_locks_the_vault() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); env_file();
        let h = test_home("nokey"); open(&h); put("one-one", "abcdefgh-1234", "", "person"); std::fs::remove_file(h.join("secrets/vault.key")).unwrap();
        let s = open(&h); assert_eq!(s["locked"], true); assert!(s["why"].as_str().unwrap().contains("key file is missing")); let _ = std::fs::remove_dir_all(&h);
    }
    #[test] fn a_failed_write_leaves_memory_disk_and_masking_as_they_were() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); env_file();
        for step in ["write", "fsync", "rename"] {
            let h = test_home(step); open(&h); assert_eq!(put("keep-one", "keepvalue-111", "", "person"), "ok");
            let before = std::fs::read_to_string(h.join("secrets/vault.json")).unwrap();
            std::env::set_var("BACKPLANE_VAULT_TEST", "1"); std::env::set_var("BACKPLANE_VAULT_FAULT", step);
            let r = put("new-one", "newvalue-2222", "", "person");
            std::env::remove_var("BACKPLANE_VAULT_FAULT"); std::env::remove_var("BACKPLANE_VAULT_TEST");
            assert!(r.starts_with("fail"), "{step}: {r}");
            assert_eq!(std::fs::read_to_string(h.join("secrets/vault.json")).unwrap(), before, "{step}: disk");
            assert_eq!(names(), vec!["keep-one"], "{step}: memory");
            assert!(!super::super::mask::all().contains(&"newvalue-2222".to_string()), "{step}: masking");
            assert!(std::fs::read_dir(h.join("secrets")).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().ends_with(".tmp")), "{step}: temp file removed");
            let _ = std::fs::remove_dir_all(&h);
        }
    }
    #[test] fn a_failed_folder_flush_is_committed_not_ok_and_an_unsupported_one_is_accepted() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); env_file();
        let h = test_home("dirsync"); open(&h);
        std::env::set_var("BACKPLANE_VAULT_TEST", "1"); std::env::set_var("BACKPLANE_VAULT_FAULT", "dirsync");
        let r = put("one-one", "abcdefgh-1234", "", "person");
        assert!(r.starts_with("committed ") && r.contains("EIO"), "{r}"); assert_eq!(status()["flush"], "unflushed EIO"); assert_eq!(names(), vec!["one-one"]);
        std::env::set_var("BACKPLANE_VAULT_FAULT", "dirsync-unsupported");
        assert_eq!(put("two-two", "abcdefgh-5678", "", "person"), "ok"); assert_eq!(status()["flush"], "unsupported EINVAL");
        std::env::remove_var("BACKPLANE_VAULT_FAULT"); std::env::remove_var("BACKPLANE_VAULT_TEST");
        let _ = std::fs::remove_dir_all(&h);
    }
    #[test] fn replacing_keeps_the_purpose_and_the_last_use() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); env_file();
        let h = test_home("replace"); open(&h); put("one-one", "abcdefgh-1234", "why", "person"); put("one-one", "abcdefgh-9999", "", "");
        let l = list(); assert_eq!(l.as_array().unwrap().len(), 1); assert_eq!(l[0]["purpose"], "why"); assert_eq!(l[0]["by"], "person");
        assert!(super::super::mask::all().contains(&"abcdefgh-9999".to_string()) && !super::super::mask::all().contains(&"abcdefgh-1234".to_string()));
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test] async fn secret_run_injects_by_name_masks_the_output_and_removes_the_files() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); env_file();
        let h = test_home("run"); open(&h); put("api-token", "tok-abcdefgh-1234", "", "person");
        let r = run(json!({"command":"echo \"v=$T\"; cat \"$F\"; echo err >&2; exit 3","env":{"T":"api-token"},"files":{"F":"api-token"},"thread":"t1"})).await;
        assert_eq!(r["code"], 3); assert_eq!(r["timedOut"], false); assert_eq!(r["stderr"], "err\n");
        let out = r["stdout"].as_str().unwrap(); assert!(!out.contains("tok-abcdefgh"), "{out}"); assert!(out.contains(&format!("v={}", "\u{2022}".repeat(17))), "{out}");
        assert!(!h.join("secrets/run").read_dir().map(|mut d| d.next().is_some()).unwrap_or(false), "the run folder is removed");
        assert_eq!(list()[0]["usedBy"], "t1");
        assert!(run(json!({"command":"true","env":{"T":"nope"}})).await["error"].as_str().unwrap().starts_with("no secret named nope"));
        assert_eq!(run(json!({"command":"  "})).await["error"], "command is required");
        let _ = std::fs::remove_dir_all(&h);
    }
    #[tokio::test] async fn secret_run_masks_the_json_and_percent_forms_and_a_timeout_is_124() {
        let _l = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); env_file();
        let h = test_home("forms"); open(&h); put("odd-one", "pa/ss+word&12345", "", "person");
        let r = run(json!({"command":"printf '%s\\n' 'pa/ss+word&12345' 'pa%2Fss%2Bword%2612345'","env":{"T":"odd-one"}})).await;
        let out = r["stdout"].as_str().unwrap(); assert!(!out.contains("pa/ss") && !out.contains("pa%2Fss"), "{out}");
        let t = run(json!({"command":"sleep 30","env":{"T":"odd-one"},"timeout":1})).await; assert_eq!(t["code"], 124); assert_eq!(t["timedOut"], true);
        let _ = std::fs::remove_dir_all(&h);
    }
    #[test] fn the_kill_guard_never_signals_a_pid_of_0_or_1_or_none() { assert!(!kill_group(None, 0)); assert!(!kill_group(Some(0), 0)); assert!(!kill_group(Some(1), 0)); }
}

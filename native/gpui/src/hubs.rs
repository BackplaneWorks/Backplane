//! The hubs this client knows: this computer's own and every one it paired with. The first is the one in use. The list rules (adopt, use, forget) and the
//! addresses an invitation is tried at are the phones' (mobile/android HubList.kt, mobile/ios HubList.swift), held to the same answers by
//! test/mobile/hubs_cases.json. Each paired hub has its own credential in its own owner-only file under `hubs/`; none is ever in a hub's `token` file.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io::{self, Write}, path::{Path, PathBuf}};

/// The key of this computer's own hub.
pub const LOCAL: &str = "local";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hub {
    /// host:port for a paired hub, `local` for this computer's own.
    pub key: String,
    /// The paired hub's address (http:// or https://); empty for this computer's own.
    #[serde(default)]
    pub origin: String,
    /// This computer's own hub only: the token file of the hub the client was started on (`origin` is its address). Empty until the client is started with one.
    /* i18n-ok: a serde attribute */ #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token_file: String,
}
impl Hub {
    pub fn local() -> Hub { Hub { key: LOCAL.into(), origin: String::new(), token_file: String::new() } }
    pub fn remote(origin: &str) -> Hub { Hub { key: crate::pairing::host_port(origin).0, origin: origin.trim_end_matches('/').to_string(), token_file: String::new() } }
    pub fn is_local(&self) -> bool { self.key == LOCAL }
    /// What the picker and the Hubs page call it.
    /* i18n-ok: the label is what drivers press; drawn through trs */ pub fn label(&self) -> String { if self.is_local() { "This computer".into() } else { self.key.clone() } }
    /// ws:// or wss:// address of a paired hub.
    pub fn ws(&self) -> String { crate::pairing::ws_of(&self.origin) }
}

// ---- the phones' pure rules, over any item with a key ----

/// A newly paired hub goes first; an older pairing with the same key is replaced, every other hub is kept.
pub fn adopt<T: Clone>(list: &[T], item: T, key: impl Fn(&T) -> Option<String>) -> Vec<T> {
    let k = key(&item);
    let mut out = vec![item];
    out.extend(list.iter().filter(|x| key(x) != k).cloned());
    out
}
/// Switching to a paired hub moves it first; an unknown key changes nothing.
pub fn use_hub<T: Clone>(list: &[T], k: &str, key: impl Fn(&T) -> Option<String>) -> Vec<T> {
    match list.iter().find(|x| key(x).as_deref() == Some(k)) {
        Some(hit) => { let mut out = vec![hit.clone()]; out.extend(list.iter().filter(|x| key(x).as_deref() != Some(k)).cloned()); out }
        None => list.to_vec(),
    }
}
pub fn forget<T: Clone>(list: &[T], k: &str, key: impl Fn(&T) -> Option<String>) -> Vec<T> { list.iter().filter(|x| key(x).as_deref() != Some(k)).cloned().collect() }

/// host:port of a link, the key the phones use for a hub.
pub fn link_key(link: &str) -> Option<String> { crate::pairing::origin_of(link.trim()).map(|o| crate::pairing::host_port(&o).0) }

// ---- the client's list ----

fn key_of(h: &Hub) -> Option<String> { Some(h.key.clone()) }
/// This computer's own hub is always on the list (last, until it is used).
fn with_local(mut list: Vec<Hub>) -> Vec<Hub> {
    let mut seen = std::collections::HashSet::new();
    list.retain(|h| seen.insert(h.key.clone()));
    if !list.iter().any(Hub::is_local) { list.push(Hub::local()); }
    list
}
pub fn adopted(list: &[Hub], origin: &str) -> Vec<Hub> { with_local(adopt(list, Hub::remote(origin), key_of)) }
pub fn used(list: &[Hub], key: &str) -> Vec<Hub> { with_local(use_hub(list, key, key_of)) }
/// Forgetting a hub; this computer's own cannot be forgotten.
pub fn forgotten(list: &[Hub], key: &str) -> Vec<Hub> { if key == LOCAL { list.to_vec() } else { with_local(forget(list, key, key_of)) } }

/// Whether an address names a hub on this machine.
pub fn is_loopback(url: &str) -> bool {
    let (host, _) = crate::pairing::host_port(url);
    let h = host.rsplit_once(':').map_or(host.as_str(), |(h, _)| h);
    matches!(h, "127.0.0.1" | "localhost" | "[::1]")
}
/// The window's hub when it is not this computer's (any address off loopback: a hub chosen in the picker, or `--url`): its short name for
/// the persistent mark ("desktop", the first label of its host) and its address for the hover. None: this computer's, drawn as always.
pub fn elsewhere(url: &str) -> Option<(String, String)> {
    if url.is_empty() || is_loopback(url) { return None }
    let (addr, tls) = crate::pairing::host_port(url);
    let host = addr.rsplit_once(':').map_or(addr.as_str(), |(h, _)| h).trim_matches(['[', ']']).to_string();
    let short = if host.parse::<std::net::IpAddr>().is_ok() { host.clone() } else { host.split('.').next().unwrap_or(&host).to_string() };
    let full = if (tls && addr.ends_with(":443")) || (!tls && addr.ends_with(":80")) { host } else { addr };
    Some((short, full))
}
/// A client started on a local hub (`--url`, `--token-file`) keeps that as "This computer": the new list, None when nothing changes or the address is not local.
pub fn remembered(list: &[Hub], url: &str, token_file: &str) -> Option<Vec<Hub>> {
    if token_file.is_empty() || !is_loopback(url) { return None }
    let origin = crate::pairing::http_of(url).trim_end_matches('/').to_string();
    let mut out = list.to_vec();
    let l = out.iter_mut().find(|h| h.is_local())?;
    if l.origin == origin && l.token_file == token_file { return None }
    l.origin = origin; l.token_file = token_file.to_string();
    Some(out)
}
/// What a relaunch adds to name the hub in use: when it is this computer's and a local target was kept, that target; else nothing (a paired hub connects by its saved credential).
pub fn local_args(list: &[Hub]) -> Vec<String> {
    match list.first() {
        Some(h) if h.is_local() && !h.origin.is_empty() && !h.token_file.is_empty() => vec!["--url".into(), h.origin.clone(), "--token-file".into(), h.token_file.clone()],
        _ => vec![],
    }
}
/// The arguments to start again with, given the list in use.
pub fn relaunch_with(args: &[String], list: &[Hub]) -> Vec<String> {
    let mut out = crate::pairing::relaunch_args(args); out.extend(local_args(list)); out
}
/// The local hub this client was started on, when it is the one in use and its token file is still there.
pub fn local_target(list: &[Hub]) -> Option<(String, PathBuf)> {
    let h = list.first().filter(|h| h.is_local() && !h.origin.is_empty() && !h.token_file.is_empty())?;
    let t = PathBuf::from(&h.token_file);
    t.is_file().then(|| (h.origin.clone(), t))
}

/// What a start connects to.
#[derive(Debug, PartialEq, Clone)]
pub enum Target { Local, Remote { key: String, ws: String } }
/// The hub in use: the first that can be reached with what is here (a paired hub needs its credential file).
pub fn target(list: &[Hub], has_credential: impl Fn(&str) -> bool) -> Target {
    for h in list {
        if h.is_local() { return Target::Local }
        if has_credential(&h.key) { return Target::Remote { key: h.key.clone(), ws: h.ws() } }
    }
    Target::Local
}

/// What choosing a hub in the picker does, decided here: nothing for the hub already in use or an unknown one, else the new list to keep and restart on.
pub fn switch(list: &[Hub], key: &str) -> Option<Vec<Hub>> {
    if list.first().map_or(true, |h| h.key == key) || !list.iter().any(|h| h.key == key) { return None }
    Some(used(list, key))
}
/// What forgetting does: the new list, and whether the hub in use changes (the client must restart on the next one).
pub fn drop_hub(list: &[Hub], key: &str) -> Option<(Vec<Hub>, bool)> {
    if key == LOCAL || !list.iter().any(|h| h.key == key) { return None }
    Some((forgotten(list, key), list.first().map_or(false, |h| h.key == key)))
}

/// A row of the Hubs page and of the picker's menu, decided here.
#[derive(Debug, PartialEq, Clone)]
pub struct Entry { pub key: String, pub label: String, pub in_use: bool, pub can_forget: bool }
pub fn entries(list: &[Hub]) -> Vec<Entry> {
    list.iter().enumerate().map(|(i, h)| Entry { key: h.key.clone(), label: h.label(), in_use: i == 0, can_forget: !h.is_local() }).collect()
}

// ---- the saved list ----

fn list_path(dir: &Path) -> PathBuf { dir.join("hubs.json") }
fn old_pairing(dir: &Path) -> PathBuf { dir.join("pairing.json") }
fn old_token(dir: &Path) -> PathBuf { dir.join("device-token") }
fn safe(key: &str) -> String { key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect() }
/// A paired hub's credential file.
pub fn credential_path(dir: &Path, key: &str) -> PathBuf { dir.join("hubs").join(format!("{}.token", safe(key))) }

fn write_private(path: &Path, body: &str) -> io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; if d.file_name().map_or(false, |n| n == "hubs") { let _ = std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700)); } }
    }
    let tmp = path.with_extension("tmp");
    let mut o = std::fs::OpenOptions::new(); o.write(true).create(true).truncate(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; o.mode(0o600); }
    let mut f = o.open(&tmp)?; f.write_all(body.as_bytes())?; f.sync_all()?; drop(f);
    crate::plat::make_private(&tmp)?; // a rename keeps it
    std::fs::rename(&tmp, path)
}
fn parse_list(text: &str) -> Option<Vec<Hub>> {
    let v: Value = serde_json::from_str(text).ok()?;
    let hubs: Vec<Hub> = serde_json::from_value(v["hubs"].clone()).ok()?;
    Some(with_local(hubs.into_iter().filter(|h| h.is_local() || (h.origin.starts_with("http://") || h.origin.starts_with("https://"))).collect()))
}
pub fn save_list(dir: &Path, list: &[Hub]) -> io::Result<()> { write_private(&list_path(dir), &json!({"hubs": list}).to_string()) }

/// The single pairing the client kept before the list (`pairing.json` + `device-token`), as a list: that hub in use, this computer's after it.
pub fn migrated(pairing_json: &str, has_token: bool) -> Option<Vec<Hub>> {
    let j: Value = serde_json::from_str(pairing_json).ok()?;
    let hub = j["hub"].as_str().filter(|h| h.starts_with("ws://") || h.starts_with("wss://"))?;
    if !has_token { return None }
    Some(with_local(vec![Hub::remote(&crate::pairing::http_of(hub))]))
}
/// The list in `dir`. The single old pairing becomes the list without pairing again: its credential moves to its own file, then the old files go.
pub fn load_from(dir: &Path) -> Vec<Hub> {
    if let Some(l) = std::fs::read_to_string(list_path(dir)).ok().as_deref().and_then(parse_list) { return l }
    let token = std::fs::read_to_string(old_token(dir)).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let old = std::fs::read_to_string(old_pairing(dir)).ok();
    if let Some(list) = old.as_deref().and_then(|p| migrated(p, token.is_some())) {
        let (k, t) = (list[0].key.clone(), token.unwrap_or_default());
        // the credential first, then the list, then the old files: a failure part way leaves the old pairing to be migrated again
        if write_private(&credential_path(dir, &k), &t).is_ok() && save_list(dir, &list).is_ok() { let _ = std::fs::remove_file(old_pairing(dir)); let _ = std::fs::remove_file(old_token(dir)); }
        return list;
    }
    vec![Hub::local()]
}
/// Keep a hub's new credential and put it first. Pairing it again replaces only its own credential.
pub fn adopt_in(dir: &Path, origin: &str, token: &str) -> io::Result<Vec<Hub>> {
    let list = adopted(&load_from(dir), origin);
    write_private(&credential_path(dir, &list[0].key), token)?;
    save_list(dir, &list)?;
    Ok(list)
}
pub fn use_in(dir: &Path, key: &str) -> io::Result<Option<Vec<Hub>>> {
    let Some(list) = switch(&load_from(dir), key) else { return Ok(None) };
    save_list(dir, &list)?; Ok(Some(list))
}
pub fn forget_in(dir: &Path, key: &str) -> io::Result<Option<(Vec<Hub>, bool)>> {
    let Some((list, moved)) = drop_hub(&load_from(dir), key) else { return Ok(None) };
    save_list(dir, &list)?;
    let _ = std::fs::remove_file(credential_path(dir, key));
    Ok(Some((list, moved)))
}
pub fn has_credential_in(dir: &Path, key: &str) -> bool { credential_path(dir, key).is_file() }
pub fn target_in(dir: &Path) -> Target { target(&load_from(dir), |k| has_credential_in(dir, k)) }

fn dir() -> Option<PathBuf> { crate::pairing::dir() }
pub fn load() -> Vec<Hub> { dir().map(|d| load_from(&d)).unwrap_or_else(|| vec![Hub::local()]) }
pub fn adopt_saved(origin: &str, token: &str) -> io::Result<Vec<Hub>> { adopt_in(&dir().ok_or(io::ErrorKind::NotFound)?, origin, token) }
pub fn use_saved(key: &str) -> io::Result<Option<Vec<Hub>>> { use_in(&dir().ok_or(io::ErrorKind::NotFound)?, key) }
pub fn forget_saved(key: &str) -> io::Result<Option<(Vec<Hub>, bool)>> { forget_in(&dir().ok_or(io::ErrorKind::NotFound)?, key) }
pub fn target_saved() -> Target { dir().map(|d| target_in(&d)).unwrap_or(Target::Local) }
pub fn remember_saved(url: &str, token_file: &str) {
    let Some(d) = dir() else { return };
    if let Some(l) = remembered(&load_from(&d), url, token_file) { let _ = save_list(&d, &l); }
}
pub fn credential_saved(key: &str) -> Option<PathBuf> { dir().map(|d| credential_path(&d, key)) }

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    const SECRET: &str = "0123456789abcdef0123456789abcdef";
    fn cases() -> Value { serde_json::from_str(include_str!("../../../test/mobile/hubs_cases.json")).unwrap() }
    fn strs(v: &Value) -> Vec<String> { v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect() }
    #[test] fn a_window_on_another_hub_knows_it_and_this_computers_does_not() {
        assert_eq!(elsewhere("wss://desktop.tail9e0cf2.ts.net"), Some(("desktop".into(), "desktop.tail9e0cf2.ts.net".into())));
        assert_eq!(elsewhere("ws://framework-1.tail.ts.net:8451"), Some(("framework-1".into(), "framework-1.tail.ts.net:8451".into())));
        assert_eq!(elsewhere("ws://100.64.0.2:3800").map(|e| e.0), Some("100.64.0.2".into()));
        assert_eq!((elsewhere("ws://127.0.0.1:3800"), elsewhere("http://localhost:8450"), elsewhere("")), (None, None, None));
    }
    #[test] fn the_phones_adopt_table() {
        for c in cases()["adopt"].as_array().unwrap() {
            assert_eq!(adopt(&strs(&c["links"]), c["link"].as_str().unwrap().to_string(), |l| link_key(l)), strs(&c["want"]), "{}", c["name"]);
        }
    }
    #[test] fn the_phones_use_table() {
        for c in cases()["use"].as_array().unwrap() { assert_eq!(use_hub(&strs(&c["links"]), c["key"].as_str().unwrap(), |l| link_key(l)), strs(&c["want"]), "{}", c["name"]); }
    }
    #[test] fn the_phones_forget_table() {
        for c in cases()["forget"].as_array().unwrap() { assert_eq!(forget(&strs(&c["links"]), c["key"].as_str().unwrap(), |l| link_key(l)), strs(&c["want"]), "{}", c["name"]); }
    }
    #[test] fn the_phones_routes_table() {
        for c in cases()["routes"].as_array().unwrap() {
            let got = crate::pairing::routes(c["text"].as_str().unwrap());
            assert_eq!(got, strs(&c["want"]), "{}", c["name"]);
            assert_eq!(crate::pairing::only_local(&got), c["local"].as_bool().unwrap(), "{}", c["name"]);
        }
    }
    fn tmp(name: &str) -> PathBuf {
        let base = std::env::var_os("BP_TMP").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let d = base.join(format!("bp-hubs-{name}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d
    }
    #[test] fn the_old_single_pairing_becomes_the_list_without_pairing_again() {
        let d = tmp("migrate");
        std::fs::write(d.join("pairing.json"), r#"{"hub":"wss://box.ts.net:8451"}"#).unwrap();
        std::fs::write(d.join("device-token"), SECRET).unwrap();
        let l = load_from(&d);
        assert_eq!(l, vec![Hub { key: "box.ts.net:8451".into(), origin: "https://box.ts.net:8451".into(), token_file: String::new() }, Hub::local()], "the paired hub stays in use");
        assert_eq!(std::fs::read_to_string(credential_path(&d, "box.ts.net:8451")).unwrap(), SECRET, "its credential moved to its own file");
        assert!(!d.join("pairing.json").exists() && !d.join("device-token").exists());
        assert_eq!(load_from(&d), l, "the list is what is read next time");
        assert_eq!(target_in(&d), Target::Remote { key: "box.ts.net:8451".into(), ws: "wss://box.ts.net:8451".into() });
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(credential_path(&d, "box.ts.net:8451")).unwrap().permissions().mode() & 0o777, 0o600); }
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn a_pairing_without_its_credential_is_not_migrated() {
        assert_eq!(migrated(r#"{"hub":"ws://h:1"}"#, false), None);
        assert_eq!(migrated(r#"{"hub":"http://h:1"}"#, true), None);
        assert_eq!(migrated("nonsense", true), None);
        let d = tmp("nocred"); std::fs::write(d.join("pairing.json"), r#"{"hub":"ws://h:1"}"#).unwrap();
        assert_eq!(load_from(&d), vec![Hub::local()]); let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn nothing_saved_is_this_computers_hub() {
        let d = tmp("none"); assert_eq!(load_from(&d), vec![Hub::local()]); assert_eq!(target_in(&d), Target::Local); let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn pairing_again_replaces_only_that_hubs_credential() {
        let d = tmp("repair");
        adopt_in(&d, "http://a:1", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        adopt_in(&d, "http://b:2", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
        let l = adopt_in(&d, "http://a:1", "cccccccccccccccccccccccccccccccc").unwrap();
        assert_eq!(l.iter().map(|h| h.key.as_str()).collect::<Vec<_>>(), ["a:1", "b:2", LOCAL]);
        assert_eq!(std::fs::read_to_string(credential_path(&d, "a:1")).unwrap(), "cccccccccccccccccccccccccccccccc");
        assert_eq!(std::fs::read_to_string(credential_path(&d, "b:2")).unwrap(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        assert!(!d.join("token").exists(), "a hub's own token file is never written");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn switching_keeps_the_choice_and_forgetting_falls_back() {
        let d = tmp("switch");
        adopt_in(&d, "http://a:1", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        adopt_in(&d, "https://b.ts.net:8451", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();
        assert!(matches!(target_in(&d), Target::Remote { ref key, .. } if key == "b.ts.net:8451"));
        use_in(&d, LOCAL).unwrap().unwrap();
        assert_eq!(target_in(&d), Target::Local, "this computer's hub in use survives a restart");
        use_in(&d, "a:1").unwrap().unwrap();
        assert_eq!(target_in(&d), Target::Remote { key: "a:1".into(), ws: "ws://a:1".into() });
        assert_eq!(use_in(&d, "a:1").unwrap(), None, "the hub in use is not a switch");
        assert_eq!(use_in(&d, "z:9").unwrap(), None);
        let (l, moved) = forget_in(&d, "a:1").unwrap().unwrap();
        assert!(moved && l[0].key == LOCAL, "forgetting the hub in use falls back to the next");
        assert!(!has_credential_in(&d, "a:1"));
        assert_eq!(forget_in(&d, LOCAL).unwrap(), None, "this computer's hub stays");
        assert!(!forget_in(&d, "b.ts.net:8451").unwrap().unwrap().1, "forgetting another leaves the one in use");
        assert_eq!(target_in(&d), Target::Local);
        let _ = std::fs::remove_dir_all(&d);
    }
    fn args(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }
    /// The bug: --url A --token-file F, switch to a remote, switch back, and the window started a new hub on the default home.
    #[test] fn switching_to_a_remote_and_back_relaunches_on_the_local_hub_it_started_with() {
        let d = tmp("roundtrip");
        let start = args(&["--url", "http://127.0.0.1:3797", "--token-file", "/h/.backplane-gpui-fresh/home/token", "--theme", "dark"]);
        remember_saved_in(&d, "http://127.0.0.1:3797", "/h/.backplane-gpui-fresh/home/token");
        adopt_in(&d, "https://desktop.ts.net:8451", "dddddddddddddddddddddddddddddddd").unwrap();
        // on the remote, the window runs with no --url: switching back relaunches from those args
        let on_remote = relaunch_with(&start, &load_from(&d));
        assert_eq!(on_remote, args(&["--theme", "dark"]), "a paired hub connects by its credential");
        let list = use_in(&d, LOCAL).unwrap().unwrap();
        assert_eq!(relaunch_with(&on_remote, &list), args(&["--theme", "dark", "--url", "http://127.0.0.1:3797", "--token-file", "/h/.backplane-gpui-fresh/home/token"]));
        assert_eq!(load_from(&d)[0].origin, "http://127.0.0.1:3797", "kept across restarts");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn a_plain_install_relaunches_as_before() {
        let l = vec![Hub::local()];
        assert_eq!(relaunch_with(&args(&["--theme", "dark", "backplane://pair?url=x"]), &l), args(&["--theme", "dark"]));
        assert_eq!(local_target(&l), None);
        assert_eq!(remembered(&l, "http://100.64.0.1:3797", "/t"), None, "a hub on another machine is not this computer's");
        assert_eq!(remembered(&l, "http://127.0.0.1:3797", ""), None);
    }
    fn remember_saved_in(dir: &Path, url: &str, tf: &str) { if let Some(l) = remembered(&load_from(dir), url, tf) { save_list(dir, &l).unwrap(); } }
    #[test] fn a_hub_without_its_credential_is_passed_over() {
        let l = vec![Hub::remote("http://a:1"), Hub::remote("http://b:2"), Hub::local()];
        assert_eq!(target(&l, |k| k == "b:2"), Target::Remote { key: "b:2".into(), ws: "ws://b:2".into() });
        assert_eq!(target(&l, |_| false), Target::Local);
    }
    #[test] fn the_picker_lists_the_hub_in_use_first_and_only_a_paired_hub_can_be_forgotten() {
        let l = vec![Hub::remote("https://b.ts.net:8451"), Hub::local()];
        assert_eq!(entries(&l), vec![Entry { key: "b.ts.net:8451".into(), label: "b.ts.net:8451".into(), in_use: true, can_forget: true }, Entry { key: LOCAL.into(), label: "This computer".into(), in_use: false, can_forget: false }]);
        assert_eq!(switch(&l, "b.ts.net:8451"), None); assert_eq!(switch(&l, "q:1"), None);
        assert_eq!(switch(&l, LOCAL).unwrap()[0], Hub::local());
        assert_eq!(drop_hub(&l, LOCAL), None);
    }
}

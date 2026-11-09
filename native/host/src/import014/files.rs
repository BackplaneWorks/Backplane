//! The files that come with a 0.14 import. Each is copied, never moved, and never over a file this home has already (a re-run copies only what is
//! missing): attachments and message images; each imported bot's folder into the bots' folder (`bots::relocate::root`), unless it is larger than
//! `BOT_MAX`; the Claude and Codex account profiles (their stores and session history), without the login files unless keys may come; then the
//! profiles are added to the running registry, 0.14's paired devices to the running device registry, and (keys only) the vault's secrets, the
//! voice key and the imported webhooks' secrets.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// A bot folder larger than this stays where it is (its threads and memory still come): 0.14's working folders reached tens of gigabytes.
pub const BOT_MAX: u64 = 2 << 30;
/// The files that hold a Claude or Codex login inside a profile's store.
const LOGINS: &[&str] = &[".credentials.json", "auth.json", "login.json"];

#[derive(Default, Clone)]
pub struct Copied { pub copied: usize, pub notes: Vec<Value>, pub profiles: Vec<Value> }

thread_local! {
    /// Files copied by this thread's `copy_counted`, and who hears about them (every 25 files).
    static COPIED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static HEARS: std::cell::RefCell<Option<Box<dyn Fn(u64) + Send>>> = const { std::cell::RefCell::new(None) };
}
fn copied_one() { COPIED.with(|c| { let n = c.get() + 1; c.set(n); if n % 25 == 0 { HEARS.with(|h| if let Some(f) = h.borrow().as_ref() { f(n) }) } }) }
/// `copy`, telling `at` how many files it has copied as it goes.
pub fn copy_counted(old: &Path, home: &Path, bots: &[(String, String)], hooks: &[String], secrets: bool, have_profiles: &[String], at: Box<dyn Fn(u64) + Send>) -> Copied {
    COPIED.with(|c| c.set(0)); HEARS.with(|h| *h.borrow_mut() = Some(at));
    let r = copy(old, home, bots, hooks, secrets, have_profiles);
    HEARS.with(|h| *h.borrow_mut() = None); r
}

/// Copies `src` into `dst`: folders and files (links are skipped), never over a file that is there. `skip` names files left out. Returns files copied.
pub fn copy_tree(src: &Path, dst: &Path, skip: &[&str]) -> std::io::Result<usize> {
    let mut n = 0; std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?; let ty = e.file_type()?; let name = e.file_name(); let (s, d) = (e.path(), dst.join(&name));
        if ty.is_symlink() || skip.iter().any(|k| name.to_str() == Some(*k)) { continue }
        if ty.is_dir() { n += copy_tree(&s, &d, skip)?; continue }
        if d.exists() { continue }
        std::fs::copy(&s, &d)?; n += 1; copied_one();
    }
    Ok(n)
}
/// Whether a folder holds more than `cap` bytes; stops counting once it does.
pub fn larger_than(p: &Path, cap: u64) -> Option<u64> {
    let mut total = 0u64; let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let Ok(ty) = e.file_type() else { continue }; if ty.is_symlink() { continue }
            if ty.is_dir() { stack.push(e.path()) } else { total += e.metadata().map_or(0, |m| m.len()); if total > cap { return Some(total) } }
        }
    }
    None
}

/// The file copies, in a blocking thread. `have_profiles`: account ids the registry here has already.
pub fn copy(old: &Path, home: &Path, bots: &[(String, String)], hooks: &[String], secrets: bool, have_profiles: &[String]) -> Copied {
    let mut r = Copied::default();
    let mut cp = |s: PathBuf, d: PathBuf, skip: &[&str], r: &mut Copied| { if s.is_dir() { match copy_tree(&s, &d, skip) { Ok(n) => r.copied += n, Err(e) => r.notes.push(json!({"k":"copy-failed","path":s.to_string_lossy(),"why":e.to_string()})) } } };
    for f in ["attachments", "message-images"] { cp(old.join(f), home.join(f), &[], &mut r) }
    let root = crate::bots::relocate::root(home);
    let mut moved: Vec<(String, String)> = vec![]; // (old folder, new folder) whose agent history comes along
    for (id, name) in bots {
        if !crate::bots::rpc::safe_id(id) { continue }
        let (s, d) = (old.join("bots").join(id), root.join(id));
        if !s.is_dir() { continue }
        if let Some(size) = larger_than(&s, BOT_MAX) { r.notes.push(json!({"k":"bot-folder","name":name,"gb":size / (1 << 30),"path":s.to_string_lossy()})); continue }
        if !d.exists() { cp(s.clone(), d.clone(), &[], &mut r) }
        moved.push((s.to_string_lossy().into_owned(), d.to_string_lossy().into_owned()));
    }
    // account profiles: their folders (stores, session history), then the registry entries for `adopt_accounts`
    let reg = old.join("providers/accounts-v1");
    let list: Vec<Value> = std::fs::read_to_string(reg.join("profiles.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v["profiles"].as_array().cloned()).unwrap_or_default();
    let dest = home.join("providers/accounts-v1");
    for p in list {
        let id = p["id"].as_str().unwrap_or("").to_string();
        // a profile here already (an earlier import without keys) still gets its missing files when keys come now: its login
        if !crate::settings::accounts_io::valid_id(&id) || (have_profiles.contains(&id) && !secrets) { continue }
        let (s, d) = (reg.join(&id), dest.join(&id));
        if s.is_dir() { cp(s, d.clone(), if secrets { &[] } else { LOGINS }, &mut r); private(&d); }
        // a bot's agent history is filed under the folder it worked in, which moved
        for proj in std::fs::read_dir(&d).into_iter().flatten().flatten().map(|e| e.path().join("projects")).chain([d.join("projects")]).filter(|p| p.is_dir()) {
            for (from, to) in &moved { let (a, b) = (proj.join(crate::bots::relocate::munge(from)), proj.join(crate::bots::relocate::munge(to))); if a.is_dir() { cp(a, b, &[], &mut r) } }
        }
        r.profiles.push(p);
    }
    if secrets {
        let sd = home.join("secrets");
        let key = old.join("secrets/openai.key");
        if key.is_file() && !sd.join("openai.key").exists() { if private_copy(&key, &sd.join("openai.key")) { r.copied += 1 } }
        for h in hooks { for f in [h.clone(), format!("{h}.token")] { let s = old.join("secrets/hooks").join(&f); if s.is_file() && !sd.join("hooks").join(&f).exists() && private_copy(&s, &sd.join("hooks").join(&f)) { r.copied += 1 } } }
    }
    r
}
/// 0.14's quotes and orders (`orders/q_*.json`, the same records this hub keeps) of the threads that came across, so their Buy now links and
/// their progress (paid, placed, shipped) go on here; never over a record that is here. Private as the hub keeps them: 0600 in a 0700 folder.
pub fn copy_orders(old: &Path, home: &Path, threads: &std::collections::HashSet<String>) -> usize {
    let mut n = 0;
    for e in std::fs::read_dir(old.join("orders")).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(id) = name.strip_suffix(".json") else { continue };
        if !crate::settings::order::id_ok(id) || !e.file_type().map_or(false, |t| t.is_file()) { continue }
        let Some(j) = std::fs::read_to_string(e.path()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { continue };
        if j["id"].as_str() != Some(id) || !threads.contains(j["thread"].as_str().unwrap_or("")) { continue }
        let d = home.join("orders").join(&name); if d.exists() { continue }
        if private_copy(&e.path(), &d) { n += 1 }
    }
    n
}
/// A folder only its owner may open (0700).
fn private(d: &Path) { use crate::plat::fs::PermissionsExt; let _ = std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o700)); }
/// A secret file copied with its folder 0700 and itself 0600.
fn private_copy(s: &Path, d: &Path) -> bool {
    let Some(dir) = d.parent() else { return false };
    if std::fs::create_dir_all(dir).is_err() { return false }
    private(dir);
    let ok = std::fs::copy(s, d).is_ok();
    if ok { use crate::plat::fs::PermissionsExt; let _ = std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o600)); }
    ok
}

#[derive(Default)]
pub struct Accounts { pub added: usize, pub signin: Vec<String>, pub note: Option<Value> }
/// The copied profiles into the running account registry. Without keys their logins were left out: each asks for a sign-in, and its history is here.
pub fn adopt_accounts(home: &Path, secrets: bool, c: &Copied) -> Accounts {
    let mut a = Accounts::default();
    if c.profiles.is_empty() { return a }
    let Some(mgr) = crate::settings::accounts_hub::live(home) else { a.note = Some(json!({"k":"accounts-later"})); return a };
    match mgr.adopt(&c.profiles, secrets) {
        Ok(n) => { a.added = n; if !secrets { a.signin = c.profiles.iter().filter_map(|p| p["label"].as_str().map(str::to_string)).collect(); if n > 0 { a.note = Some(json!({"k":"sign-in","names":a.signin})) } } }
        Err(()) => a.note = Some(json!({"k":"accounts-later"})),
    }
    a
}
/// 0.14's live paired devices (not revoked), for the count before an import.
pub fn live_devices(old: &Path) -> usize { std::fs::read_to_string(old.join("devices-v1.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v["devices"].as_array().map(|a| a.iter().filter(|d| !crate::net::pairing::truthy(&d["revoked"])).count())).unwrap_or(0) }
/// 0.14's paired devices into the running device registry: how many were added.
pub fn adopt_devices(home: &Path, old: &Path) -> usize {
    let Some(reg) = crate::net::start::registry(home) else { return 0 };
    let ds: Vec<Value> = std::fs::read_to_string(old.join("devices-v1.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v["devices"].as_array().cloned()).unwrap_or_default();
    reg.adopt(&ds).unwrap_or(0)
}
#[derive(Default)]
pub struct Keys { pub added: usize, pub note: Option<Value> }
/// The old vault's secrets into this one, each name not set here yet.
pub fn adopt_vault(old: &Path) -> Keys {
    let mut k = Keys::default();
    match crate::settings::vault::read_other(old) {
        Ok(items) => {
            let have = crate::settings::vault::names();
            for (name, purpose, value) in items { if have.contains(&name) { continue } if crate::settings::secret::saved_is(&crate::settings::vault::put(&name, &value, &purpose, "import")) { k.added += 1 } else { k.note = Some(json!({"k":"vault-failed","why":"this vault would not take them"})) } }
        }
        Err(why) => k.note = Some(json!({"k":"vault-failed","why":why})),
    }
    k
}

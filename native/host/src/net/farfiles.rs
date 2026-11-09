//! Files of a thread whose project is on another of the owner's machines: its Files page lists that machine's folder, and a file opened from it
//! is shown here from a copy.
//!
//! Owning side: the Files page's own `files.dir {thread, path}` lists a folder of the thread's folder (`files/mod.rs`, its `fmEntries`), and
//! `files.get {thread, path}` (here) gives one file of it (at most `GET_MAX`, base64). From another machine both come only through /far/rpc
//! (`far::METHODS`; `Feed::allowed`: a thread this hub shares). (`files.dir` stays files/mod.rs's: a second handler would replace it.)
//!
//! Asking side: the presenter is synchronous, so the Files page of such a thread is drawn from what that machine said last (`listing`); a folder not
//! asked yet, or asked more than `FRESH` ago, is asked for in the background and the views are told when the answer differs (`Hub::bump_views`).
//! A file opened is kept under `<home>/far-files/<link>/` (`copy`) and drawn from there, as a file of this machine is.
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, LazyLock, Mutex, OnceLock, Weak}, time::{Duration, Instant}};

/// The largest file another machine is given to show (an image, a PDF, a note).
pub const GET_MAX: u64 = 24 << 20;
const FRESH: Duration = Duration::from_secs(3);
const KEEP: usize = 64;

struct Seen { at: Instant, got: Option<Result<String, String>>, asking: bool }
type Key = (String, String);
static DIRS: LazyLock<Mutex<HashMap<Key, Seen>>> = LazyLock::new(Default::default);
static FILES: LazyLock<Mutex<HashMap<Key, Seen>>> = LazyLock::new(Default::default);
static BOUND: OnceLock<(Weak<Hub>, Arc<super::farsync::FarSync>)> = OnceLock::new();

pub fn bind(hub: &Arc<Hub>, sync: Arc<super::farsync::FarSync>) { let _ = BOUND.set((Arc::downgrade(hub), sync)); hub.register_rpc(Arc::new(Own)); }

/// What a folder of a thread elsewhere (`<link>~<id>`) holds, as `files::dir::list` words it; None until that machine has answered once.
pub fn listing(thread: &str, dir: &str) -> Option<Result<String, String>> { seen(&DIRS, thread, dir, "files.dir", json!({"path": dir})) }
/// This machine's copy of a file of a thread elsewhere (its absolute path here); None while it is on its way, Err when that machine refused.
pub fn copy(thread: &str, rel: &str) -> Option<Result<String, String>> { seen(&FILES, thread, rel, "files.get", json!({"path": rel})) }

fn seen(map: &'static LazyLock<Mutex<HashMap<Key, Seen>>>, thread: &str, what: &str, m: &'static str, mut p: Value) -> Option<Result<String, String>> {
    let (link, local) = match thread.split_once('~') { Some((l, t)) if !l.is_empty() && !t.is_empty() => (l.to_string(), t.to_string()), _ => return Some(Err("not a thread of another machine".into())) };
    let key = (thread.to_string(), what.to_string());
    let mut g = map.lock().unwrap();
    if g.len() >= KEEP && !g.contains_key(&key) { if let Some(old) = g.iter().filter(|(_, s)| !s.asking).min_by_key(|(_, s)| s.at).map(|(k, _)| k.clone()) { g.remove(&old); } }
    let e = g.entry(key.clone()).or_insert(Seen { at: Instant::now(), got: None, asking: false });
    // a file's copy is fetched once; a folder is asked again when what was drawn is older than FRESH
    let stale = e.got.is_none() || (m == "files.dir" && e.at.elapsed() > FRESH);
    if stale && !e.asking {
        if let Some((hub, sync)) = BOUND.get().and_then(|(h, s)| h.upgrade().map(|h| (h, s.clone()))) {
            e.asking = true;
            p["thread"] = json!(local);
            let body = json!({"id": 0, "m": m, "p": p}).to_string();
            tokio::spawn(async move {
                let (code, text) = super::rpc::post(&sync, &hub, &link, &body).await;
                let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                let why = || v["error"].as_str().or_else(|| v["result"]["fmError"].as_str()).filter(|e| !e.is_empty()).map_or_else(|| "That machine did not answer.".to_string(), str::to_string);
                let got = if code == 0 { Err("That machine is away.".to_string()) } else if code != 200 || v["ok"] != true { Err(why()) }
                    else if m == "files.dir" { Ok(v["result"]["fmEntries"].as_str().unwrap_or("").to_string()) } else { keep_copy(&hub.cfg.home, &key.0, &key.1, &v["result"]) };
                let changed = { let mut g = map.lock().unwrap(); match g.get_mut(&key) { Some(e) => { let ch = e.got.as_ref() != Some(&got); e.got = Some(got); e.at = Instant::now(); e.asking = false; ch } None => false } };
                if changed { hub.bump_views(); }
            });
        }
    }
    e.got.clone()
}

/// The bytes another machine gave, written under `<home>/far-files/<link>/<thread>/<path>` (its folders made safe), returned as that path.
fn keep_copy(home: &std::path::Path, thread: &str, rel: &str, r: &Value) -> Result<String, String> {
    use base64::Engine;
    if !crate::files::fm::files_safe(rel) || rel.is_empty() { return Err("not a file of the project".into()) }
    let bytes = base64::engine::general_purpose::STANDARD.decode(r["b64"].as_str().unwrap_or("")).map_err(|_| "the file came back unreadable".to_string())?;
    let safe = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '_' }).collect::<String>();
    let p = home.join("far-files").join(safe(thread)).join(rel);
    if let Some(d) = p.parent() { std::fs::create_dir_all(d).map_err(|e| e.to_string())?; }
    std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
    Ok(p.to_string_lossy().into_owned())
}

/// The owning side's file for another machine. Only another machine's /far/rpc reaches it (it names the thread by this hub's own id).
struct Own;
impl RpcHandler for Own {
    fn methods(&self) -> &'static [&'static str] { &["files.get"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let fail = |e: &str| vec![json!({"t":"reply","id":id,"ok":false,"error":e})];
            let th = p["thread"].as_str().unwrap_or("");
            let root = { let m = hub.model().await; m.threads.get(th).filter(|t| !t.deleted).map(|t| if !t.worktree.is_empty() { t.worktree.clone() } else { m.projects.get(&t.project).map_or(String::new(), |x| x.root.clone()) }).unwrap_or_default() };
            if root.is_empty() { return fail("This thread has no project folder.") }
            let root = std::path::PathBuf::from(root);
            use base64::Engine;
            let rel = p["path"].as_str().unwrap_or("").to_string();
            if rel.is_empty() || !crate::files::fm::files_safe(&rel) { return fail("not a file of the project") }
            let read = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, &'static str> {
                let fm = crate::files::fm::parent(&rel);
                let abs = crate::files::fm::contained(&root, &fm).ok_or("not a file of the project")?.join(crate::files::fm::last(&rel));
                let md = std::fs::symlink_metadata(&abs).map_err(|_| "no such file")?;
                if !md.is_file() { return Err("not a file") }
                if md.len() > GET_MAX { return Err("too large to show from another machine") }
                std::fs::read(&abs).map_err(|_| "could not read it")
            }).await;
            match read { Ok(Ok(b)) => vec![json!({"t":"reply","id":id,"ok":true,"result":{"b64": base64::engine::general_purpose::STANDARD.encode(b)}})], Ok(Err(e)) => fail(e), Err(_) => fail("could not read it") }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_copy_is_kept_under_the_home_and_never_outside_it() {
        use base64::Engine;
        let home = std::env::temp_dir().join(format!("bp-farfiles-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home);
        let r = json!({"b64": base64::engine::general_purpose::STANDARD.encode(b"hello")});
        let p = keep_copy(&home, "L~t1", "docs/a.md", &r).unwrap();
        assert!(p.starts_with(&home.join("far-files").to_string_lossy().into_owned()) && p.ends_with("docs/a.md")); assert_eq!(std::fs::read(&p).unwrap(), b"hello");
        for bad in ["../x", "/etc/passwd", "", "a/../../b"] { assert!(keep_copy(&home, "L~t1", bad, &r).is_err(), "{bad}"); }
        let _ = std::fs::remove_dir_all(&home);
    }
    #[test] fn a_thread_of_this_machine_is_not_asked_elsewhere() {
        assert!(matches!(listing("t1", ""), Some(Err(_)))); assert!(matches!(copy("~t1", "a"), Some(Err(_))));
        assert_eq!(listing("L~t1", ""), None, "not answered yet (nothing is bound in a test: nothing is asked)");
    }
}

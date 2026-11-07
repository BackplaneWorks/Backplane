//! The HTTP side of message images (mimg.rs) and attachments (attach.rs): GET /message-image, the subscriber that keeps a snapshot of each
//! image an assistant message names, and (once api::Body exists in the tree) POST /attach.
use super::{fm, mimg, serve};
use crate::{api::{Fut, HttpHandler, HttpReply}, auth::{Auth, Req}, hub::Hub};
use serde_json::Value;
use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};

pub fn start(hub: &Arc<Hub>) {
    // `retained`: how many log lines the snapshot loop has read; a request waits for it to reach the log's length it saw on arrival
    let retained = Arc::new(AtomicUsize::new(hub.store.len()));
    hub.register_http(Arc::new(MessageImage { retained: retained.clone() }));
    hub.register_http(Arc::new(super::attach_http::Attach));
    let h = hub.clone();
    tokio::spawn(async move { retain_loop(h, retained).await });
}
/// The longest a request waits for the snapshot loop to catch up with the log (a message naming several 24 MiB images is read, hashed and written one at a time).
const CATCH_UP: std::time::Duration = std::time::Duration::from_secs(5);
const CHUNK: usize = 200;

/// Conn.get.k i=8: /message-image needs the hub's own credential on top of Conn.allowed (Auth.credential: bearer, ?token= or cookie equal to
/// the hub token; a paired device's token does not open it).
fn root_credential(hub: &Hub, req: &Req) -> bool {
    let tok = std::fs::read_to_string(hub.cfg.home.join("token")).unwrap_or_default(); let tok = tok.trim();
    let given = Auth::credential(req);
    !tok.is_empty() && given.len() == tok.len() && given.bytes().zip(tok.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}
fn pct(s: &str) -> String { // Att.unpct: percent-decoded, `+` stays, a bad escape stays
    let b = s.as_bytes(); let mut o = Vec::with_capacity(b.len()); let mut i = 0;
    let hx = |c: u8| (c as char).to_digit(16);
    while i < b.len() { match (b[i], b.get(i + 1).and_then(|c| hx(*c)), b.get(i + 2).and_then(|c| hx(*c))) { (b'%', Some(h), Some(l)) => { o.push((h * 16 + l) as u8); i += 3 } _ => { o.push(b[i]); i += 1 } } }
    String::from_utf8_lossy(&o).into_owned()
}

struct MessageImage { retained: Arc<AtomicUsize> }
impl HttpHandler for MessageImage {
    fn routes(&self) -> &'static [&'static str] { &["/message-image"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            if !root_credential(hub, req) { return HttpReply::text("401 Unauthorized", "this Backplane needs its pairing token") }
            let (entry, path) = (pct(&req.param("entry")), pct(&req.param("path")));
            let unavailable = || HttpReply::text("404 Not Found", "Image unavailable");
            if !mimg::is_image(&path) || path.contains("/../") || path.contains("/./") || path.contains('\\') { return unavailable() }
            let home = hub.cfg.home.clone();
            // snapshots before it announces the message; here the loop runs just after the commit. Wait (up to CATCH_UP) while the loop is
            // behind the log as it stood when this request arrived, then look once: an image the message named is there by then.
            let target = hub.store.len(); let t0 = std::time::Instant::now();
            while self.retained.load(Ordering::Acquire) < target && t0.elapsed() < CATCH_UP { tokio::time::sleep(std::time::Duration::from_millis(25)).await }
            let (h, e, p) = (home.clone(), entry.clone(), path.clone());
            let looked = tokio::task::spawn_blocking(move || mimg::lookup(&h, &e, &p)).await;
            // no ref: a message from before this hub kept refs. Find it in the recent log, as the
            // hub reads store.db, and let the message's own text authorize the path (Img.message_allowed); bounded so a bad id costs little.
            let (hub2, e, p) = (hub.clone(), entry.clone(), path.clone());
            let s = match looked {
                Ok(Ok(s)) => match tokio::task::spawn_blocking(move || adopt_ref(&hub2, s, &e, &p)).await { Ok(Some(s)) => s, _ => return unavailable() },
                _ => {
                    if missed(&entry, &path) { return unavailable() }
                    match tokio::task::spawn_blocking(move || snapshot_from_log(&hub2, &e, &p)).await { Ok(Some(s)) => s, _ => { remember_miss(&entry, &path); return unavailable() } } } };
            // a message of a deleted thread grants nothing
            if let Some(t) = &s.thread { if hub.model().await.threads.get(t).map_or(false, |m| m.deleted) { return unavailable() } }
            let (p, ct) = (s.path.to_string_lossy().into_owned(), fm::ctype(&s.path.to_string_lossy()));
            tokio::task::spawn_blocking(move || serve::send(&p, ct, "")).await.unwrap_or_else(|_| unavailable())
        })
    }
}

/// MessageImage.changes: after a commit, each image an assistant message names gets its snapshot (a missing or rejected image never stops delivery).
/// Lines are read CHUNK at a time (a lagging receiver or a long gap never loads everything missed at once); `retained` follows the log.
async fn retain_loop(hub: Arc<Hub>, retained: Arc<AtomicUsize>) {
    let mut rx = hub.subscribe();
    let mut seen = retained.load(Ordering::Acquire);
    loop {
        match rx.recv().await { Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {} Err(_) => return }
        loop {
            let now = hub.store.len();
            if now <= seen { break }
            let to = (seen + CHUNK).min(now);
            let lines = hub.store.range(seen, to).unwrap_or_default(); seen = to;
            let home = hub.cfg.home.clone();
            let _ = tokio::task::spawn_blocking(move || { for l in lines { retain_line(&home, &l) } }).await;
            retained.store(seen, Ordering::Release);
        }
    }
}
/// How far back a request looks for a message with no ref: lines and time.
const SCAN_LINES: usize = 200_000;
const SCAN_TIME: std::time::Duration = std::time::Duration::from_secs(2);
fn find_message(hub: &Hub, entry: &str) -> Option<(String, String)> {
    let (mut to, floor, t0) = (hub.store.len(), hub.store.len().saturating_sub(SCAN_LINES), std::time::Instant::now());
    let needle = format!("\"{}\"", entry.replace('\\', "\\\\").replace('"', "\\\""));
    while to > floor && t0.elapsed() < SCAN_TIME {
        let from = to.saturating_sub(CHUNK * 5).max(floor);
        for l in hub.store.range(from, to).ok()?.iter().rev() {
            if !l.contains("\"MessagePosted\"") || !l.contains(&needle) { continue }
            let v: Value = serde_json::from_str(l).ok()?;
            if v["$"] == "MessagePosted" && v["role"] == "assistant" && v["msg"].as_str() == Some(entry) { return Some((v["thread"].as_str().unwrap_or("").to_string(), v["text"].as_str().unwrap_or("").to_string())) }
        }
        to = from;
    }
    None
}
fn deleted(hub: &Hub, thread: &str) -> bool { hub.model.blocking_lock().threads.get(thread).map_or(true, |t| t.deleted) }
/// No ref: find the message in the recent log; its own text authorises the path (Img.message_allowed); a deleted thread grants nothing.
fn snapshot_from_log(hub: &Hub, entry: &str, path: &str) -> Option<mimg::Snap> {
    let (thread, text) = find_message(hub, entry)?;
    if deleted(hub, &thread) || !mimg::message_allowed(&text, path) { return None }
    mimg::snapshot(&hub.cfg.home, entry, path, &thread).ok()
}
/// A ref hub wrote names no thread: look the message up, refuse a deleted thread's, and write the thread into the ref (then it is checked without a scan).
fn adopt_ref(hub: &Hub, s: mimg::Snap, entry: &str, path: &str) -> Option<mimg::Snap> {
    if s.thread.is_some() { return Some(s) }
    match find_message(hub, entry) {
        Some((thread, _)) => { if deleted(hub, &thread) { return None } mimg::set_thread(&hub.cfg.home, entry, path, &thread); Some(mimg::Snap { thread: Some(thread), ..s }) }
        None => Some(s), // older than the scan reaches: the ref proves a message named it; the thread cannot be checked
    }
}
/// A miss that cost a scan is remembered for a minute (a page of old images must not cost a scan each).
static MISSES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<(String, String), std::time::Instant>>> = std::sync::LazyLock::new(Default::default);
const MISS_FOR: std::time::Duration = std::time::Duration::from_secs(60);
fn missed(entry: &str, path: &str) -> bool { MISSES.lock().unwrap().get(&(entry.to_string(), path.to_string())).map_or(false, |t| t.elapsed() < MISS_FOR) }
fn remember_miss(entry: &str, path: &str) { let mut m = MISSES.lock().unwrap(); if m.len() >= 1024 { m.retain(|_, t| t.elapsed() < MISS_FOR); if m.len() >= 1024 { m.clear() } } m.insert((entry.to_string(), path.to_string()), std::time::Instant::now()); }
pub fn retain_line(home: &std::path::Path, line: &str) {
    if !line.contains("\"MessagePosted\"") || !line.contains("assistant") { return }
    let Ok(v) = serde_json::from_str::<Value>(line) else { return };
    if v["$"] != "MessagePosted" || v["role"] != "assistant" { return }
    let (thread, msg, text) = (v["thread"].as_str().unwrap_or(""), v["msg"].as_str().unwrap_or(""), v["text"].as_str().unwrap_or(""));
    if msg.is_empty() || !mimg::may(text) { return }
    for p in mimg::paths(text) { let _ = mimg::snapshot(home, msg, &p, thread); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test] fn only_assistant_messages_make_snapshots() {
        let home = std::env::temp_dir().join(format!("bp-ir-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap(); let home = crate::plat::fs::canonicalize(&home).unwrap();
        let src = home.join("src"); std::fs::create_dir_all(&src).unwrap();
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0, 4, 0, 0, 0, 4, 8, 6, 0, 0, 0]; png.extend([0u8; 16]);
        std::fs::write(src.join("a.png"), &png).unwrap(); let p = src.join("a.png").to_string_lossy().into_owned();
        let line = |role: &str, msg: &str| serde_json::json!({"$":"MessagePosted","thread":"t1","msg":msg,"role":role,"text":format!("here {p} ok"),"at":1}).to_string();
        retain_line(&home, &line("user", "m1")); assert!(mimg::lookup(&home, "m1", &p).is_err());
        retain_line(&home, &line("assistant", "m2")); let s = mimg::lookup(&home, "m2", &p).unwrap(); assert_eq!(s.thread.as_deref(), Some("t1")); assert_eq!(std::fs::read(&s.path).unwrap(), png);
        assert!(mimg::lookup(&home, "m2", "/other.png").is_err()); // only the exact spelling the message used
        assert_eq!(pct("%2Fa%20b+c%zz"), "/a b+c%zz");
    }

    fn png() -> Vec<u8> { let mut p = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0, 4, 0, 0, 0, 4, 8, 6, 0, 0, 0]; p.extend([0u8; 16]); p }
    async fn get(hub: &Arc<Hub>, tok: &str, entry: &str, path: &str) -> HttpReply {
        let req = Req { method: "GET".into(), target: format!("/message-image?entry={entry}&path={}&token={tok}", path.replace('/', "%2F")), headers: vec![], body: vec![], peer: None, trusted: false };
        MessageImage { retained: Arc::new(AtomicUsize::new(usize::MAX)) }.call(hub, &req).await
    }
    // Hub.thread: a message committed live is snapshotted by the loop; one that is only in the log (no ref) is found by the bounded scan;
    // deleted threads, other entries, other spellings and a wrong credential get nothing
    #[tokio::test] async fn message_images_live_scanned_and_refused() {
        let home = std::env::temp_dir().join(format!("bp-mih-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap(); let home = crate::plat::fs::canonicalize(&home).unwrap();
        let (a, b) = (home.join("src/a.png"), home.join("src/b.png")); std::fs::create_dir_all(a.parent().unwrap()).unwrap(); std::fs::write(&a, png()).unwrap(); std::fs::write(&b, png()).unwrap();
        let (pa, pb) = (a.to_string_lossy().into_owned(), b.to_string_lossy().into_owned());
        let ev = |v: Value| v.to_string();
        std::fs::write(home.join("events.jsonl"), [ev(json!({"$":"ProjectCreated","id":"p1","title":"p","root":"/x","at":1})), ev(json!({"$":"ThreadCreated","id":"t1","project":"p1","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2})),
            ev(json!({"$":"ThreadCreated","id":"t2","project":"p1","title":"D","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":3})), ev(json!({"$":"ThreadDeleted","id":"t2","at":4})),
            ev(json!({"$":"MessagePosted","thread":"t1","msg":"old1","role":"assistant","text":format!("older {pb}"),"at":5})), ev(json!({"$":"MessagePosted","thread":"t2","msg":"gone1","role":"assistant","text":format!("gone {pb}"),"at":6}))].join("\n") + "\n").unwrap();
        let cfg = crate::hub::Cfg { home: home.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
        start(&hub);
        std::fs::write(home.join("token"), "0123456789abcdef-test-token\n").unwrap(); // the hub token file (main.rs makes it at start)
        let tok = std::fs::read_to_string(home.join("token")).unwrap().trim().to_string();
        // live: committed after start, snapshotted by the loop
        hub.commit(&json!({"$":"MessagePosted","thread":"t1","msg":"live1","role":"assistant","text":format!("see {pa} please"),"at":7})).await.unwrap();
        for _ in 0..100 { if mimg::lookup(&home, "live1", &pa).is_ok() { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
        let r = get(&hub, &tok, "live1", &pa).await; assert_eq!((r.status, r.body.as_slice()), ("200 OK", png().as_slice())); assert!(r.extra.contains("Accept-Ranges: bytes") && r.ctype == "image/png");
        // an older message: no ref, found in the log, authorised by its own text
        let r = get(&hub, &tok, "old1", &pb).await; assert_eq!(r.status, "200 OK");
        // a ref hub wrote (no thread) of a deleted thread's message: the scan finds the thread and refuses; a live thread's gets its thread written
        let dir = home.join("message-images"); let snap = mimg::snapshot(&home, "gone1", &pb, "t2").unwrap(); let nm = snap.path.file_name().unwrap().to_string_lossy().into_owned();
        std::fs::write(dir.join(mimg::ref_name("gone1", &pb)), format!("{{\"image\":\"{nm}\"}}")).unwrap(); assert!(mimg::lookup(&home, "gone1", &pb).unwrap().thread.is_none());
        assert_eq!(get(&hub, &tok, "gone1", &pb).await.status, "404 Not Found");
        let snap = mimg::snapshot(&home, "old1", &pb, "t1").unwrap(); let nm = snap.path.file_name().unwrap().to_string_lossy().into_owned();
        std::fs::write(dir.join(mimg::ref_name("old1", &pb)), format!("{{\"image\":\"{nm}\"}}")).unwrap();
        assert_eq!(get(&hub, &tok, "old1", &pb).await.status, "200 OK"); assert_eq!(mimg::lookup(&home, "old1", &pb).unwrap().thread.as_deref(), Some("t1"));
        // refusals: a deleted thread's message, a path the message did not name, an unknown entry, a wrong or missing credential, a non-image path
        for (e, p) in [("gone1", pb.as_str()), ("old1", pa.as_str()), ("live1", pb.as_str()), ("nope", pa.as_str()), ("live1", "/etc/passwd"), ("live1", "/x/../a.png")] { assert_eq!(get(&hub, &tok, e, p).await.status, "404 Not Found", "{e} {p}") }
        assert_eq!(get(&hub, "wrong", "live1", &pa).await.status, "401 Unauthorized"); assert_eq!(get(&hub, "", "live1", &pa).await.status, "401 Unauthorized");
    }
}

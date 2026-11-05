//! The HTTP side of message images (mimg.rs) and attachments (attach.rs): GET /message-image, the subscriber that keeps a snapshot of each
//! image an assistant message names, and (once api::Body exists in the tree) POST /attach.
use super::{fm, mimg, serve};
use crate::{api::{Fut, HttpHandler, HttpReply}, auth::{Auth, Req}, hub::Hub};
use serde_json::Value;
use std::sync::Arc;

pub fn start(hub: &Arc<Hub>) {
    hub.register_http(Arc::new(MessageImage));
    let h = hub.clone();
    tokio::spawn(async move { retain_loop(h).await });
}

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

struct MessageImage;
impl HttpHandler for MessageImage {
    fn routes(&self) -> &'static [&'static str] { &["/message-image"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            if !root_credential(hub, req) { return HttpReply::text("401 Unauthorized", "this Backplane needs its pairing token") }
            let (entry, path) = (pct(&req.param("entry")), pct(&req.param("path")));
            let unavailable = || HttpReply::text("404 Not Found", "Image unavailable");
            if !mimg::is_image(&path) || path.contains("/../") || path.contains("/./") || path.contains('\\') { return unavailable() }
            let home = hub.cfg.home.clone();
            // the message's own commit makes the snapshot a moment after the commit: ask again briefly before answering "unavailable"
            let mut snap = None;
            for i in 0..6 {
                let (h, e, p) = (home.clone(), entry.clone(), path.clone());
                if let Ok(Ok(s)) = tokio::task::spawn_blocking(move || mimg::lookup(&h, &e, &p)).await { snap = Some(s); break }
                if i < 5 { tokio::time::sleep(std::time::Duration::from_millis(100)).await }
            }
            let Some(s) = snap else { return unavailable() };
            // a message of a deleted thread grants nothing (the Bend hub joins `thread.deleted = 0`)
            if let Some(t) = &s.thread { if hub.model().await.threads.get(t).map_or(false, |m| m.deleted) { return unavailable() } }
            let (p, ct) = (s.path.to_string_lossy().into_owned(), fm::ctype(&s.path.to_string_lossy()));
            tokio::task::spawn_blocking(move || serve::send(&p, ct, "")).await.unwrap_or_else(|_| unavailable())
        })
    }
}

/// MessageImage.changes: after a commit, each image an assistant message names gets its snapshot (a missing or rejected image never stops delivery).
async fn retain_loop(hub: Arc<Hub>) {
    let mut rx = hub.subscribe();
    let mut seen = hub.store.len();
    loop {
        match rx.recv().await { Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {} Err(_) => return }
        let now = hub.store.len();
        if now <= seen { continue }
        let lines = hub.store.range(seen, now).unwrap_or_default(); seen = now;
        let home = hub.cfg.home.clone();
        let _ = tokio::task::spawn_blocking(move || { for l in lines { retain_line(&home, &l) } }).await;
    }
}
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
    #[test] fn only_assistant_messages_make_snapshots() {
        let home = std::env::temp_dir().join(format!("bp-ir-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let src = home.join("src"); std::fs::create_dir_all(&src).unwrap();
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0, 4, 0, 0, 0, 4, 8, 6, 0, 0, 0]; png.extend([0u8; 16]);
        std::fs::write(src.join("a.png"), &png).unwrap(); let p = src.join("a.png").to_string_lossy().into_owned();
        let line = |role: &str, msg: &str| serde_json::json!({"$":"MessagePosted","thread":"t1","msg":msg,"role":role,"text":format!("here {p} ok"),"at":1}).to_string();
        retain_line(&home, &line("user", "m1")); assert!(mimg::lookup(&home, "m1", &p).is_err());
        retain_line(&home, &line("assistant", "m2")); let s = mimg::lookup(&home, "m2", &p).unwrap(); assert_eq!(s.thread.as_deref(), Some("t1")); assert_eq!(std::fs::read(&s.path).unwrap(), png);
        assert!(mimg::lookup(&home, "m2", "/other.png").is_err()); // only the exact spelling the message used
        assert_eq!(pct("%2Fa%20b+c%zz"), "/a b+c%zz");
    }
}

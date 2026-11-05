//! POST /attach (see attach.rs) on the hub's streaming-body API (api::HttpHandler::call_body, commit c7bf5f574 on gpui-core).
//! The hub has already checked POST (405), the credential (401), Content-Length present and no Transfer-Encoding (411) and len <= STREAM_MAX (413).
use super::attach;
use crate::{api::{Body, Fut, HttpHandler, HttpReply}, auth::Req, hub::Hub};
use std::sync::Arc;

pub struct Attach;

fn pct(s: &str) -> String { // Att.unpct
    let b = s.as_bytes(); let mut o = Vec::with_capacity(b.len()); let mut i = 0; let hx = |c: u8| (c as char).to_digit(16);
    while i < b.len() { match (b[i], b.get(i + 1).and_then(|c| hx(*c)), b.get(i + 2).and_then(|c| hx(*c))) { (b'%', Some(h), Some(l)) => { o.push((h * 16 + l) as u8); i += 3 } _ => { o.push(b[i]); i += 1 } } }
    String::from_utf8_lossy(&o).into_owned()
}
fn json(status: &'static str, v: serde_json::Value) -> HttpReply { HttpReply { status, ctype: "application/json".into(), extra: String::new(), body: v.to_string().into_bytes() } }

/// A refusal that leaves the body unread would close the socket with data still in flight, and the client may get a reset instead of the reply.
/// Read (and drop) what the client already sent, up to 1 MiB and 2 s; a bigger body is the client's to stop (it waits for 100 Continue, or is cut off).
async fn drain(body: &mut Body<'_>) {
    use tokio::io::AsyncReadExt;
    let mut left = body.left().min(1 << 20) as usize; let mut buf = [0u8; 16384];
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async { while left > 0 { match body.read(&mut buf[..left.min(16384)]).await { Ok(0) | Err(_) => break, Ok(n) => left -= n } } }).await;
}

impl HttpHandler for Attach {
    fn routes(&self) -> &'static [&'static str] { &["/attach"] }
    fn takes_body(&self) -> bool { true }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _req: &'a Req) -> Fut<'a, HttpReply> { Box::pin(async { HttpReply::text("405 Method Not Allowed", "POST only") }) }
    fn call_body<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req, mut body: Body<'a>) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            let (thread, key, name) = (pct(&req.param("thread")), pct(&req.param("key")), pct(&req.param("name")));
            // PR #960: an unknown, empty or deleted thread gets no attachment folder
            let known = !thread.is_empty() && hub.model().await.threads.get(&thread).map_or(false, |t| !t.deleted);
            if !known { drain(&mut body).await; return json("404 Not Found", attach::failed_plain(&thread, "No folder for this thread or project: unknown, deleted, removed or unavailable")) }
            let lim = hub.setting(attach::KEY).await.unwrap_or_default();
            let len = body.len;
            // refuse a body that cannot fit before telling the client to send it (Upload.into.k: 413 before 100 Continue)
            if len > attach::max_bytes(&lim) { drain(&mut body).await; return json("413 Payload Too Large", attach::failure(&thread, "upload.cap", &attach::mb(&lim).to_string())) }
            let _ = body.proceed().await;
            let (status, v) = attach::receive(&hub.cfg.home, &thread, &key, &name, len, &lim, u64::MAX, &mut body).await;
            json(status, v)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    // A real client socket against the handler: Content-Length body streamed to <home>/attachments/<thread>/<key>-<name>, the reply a plain
    // Content-Length response (not chunked), 100-continue answered after the size is accepted, unknown/deleted threads and over-cap sizes refused.
    #[tokio::test] async fn an_upload_over_a_real_socket() {
        let home = std::env::temp_dir().join(format!("bp-att-http-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        let ev = |v: Value| v.to_string();
        std::fs::write(home.join("events.jsonl"), [ev(json!({"$":"ProjectCreated","id":"p1","title":"p","root":"/x","at":1})), ev(json!({"$":"ThreadCreated","id":"t1","project":"p1","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2})),
            ev(json!({"$":"ThreadCreated","id":"t2","project":"p1","title":"D","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":3})), ev(json!({"$":"ThreadDeleted","id":"t2","at":4})),
            ev(json!({"$":"SettingSet","key":"files.max.mb","value":"1","at":5}))].join("\n") + "\n").unwrap();
        let cfg = crate::hub::Cfg { home: home.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
        let h2 = hub.clone();
        tokio::spawn(async move { loop { let Ok((mut s, _)) = l.accept().await else { return };
            let h = h2.clone(); tokio::spawn(async move {
                let mut head = Vec::new(); let mut b = [0u8; 1]; while !head.ends_with(b"\r\n\r\n") { if s.read(&mut b).await.unwrap_or(0) == 0 { return } head.push(b[0]) }
                let text = String::from_utf8_lossy(&head).into_owned();
                let target = text.lines().next().unwrap().split(' ').nth(1).unwrap().to_string();
                let len: u64 = text.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap())).unwrap();
                let expect = text.to_ascii_lowercase().contains("expect: 100-continue");
                let req = Req { method: "POST".into(), target, headers: vec![], body: vec![], peer: None, trusted: false };
                let r = { let body = crate::api::Body::new(&mut s, len, expect); Attach.call_body(&h, &req, body).await };
                let _ = s.write_all(&crate::wire::response_for(r.status, &r.ctype, &r.extra, &r.body)).await; }); } });
        let post = |q: String, len: usize, body: Vec<u8>, expect: bool| async move {
            let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            s.write_all(format!("POST /attach?{q} HTTP/1.1\r\nHost: x\r\nContent-Length: {len}\r\n{}Connection: close\r\n\r\n", if expect { "Expect: 100-continue\r\n" } else { "" }).as_bytes()).await.unwrap();
            if expect { let mut c = [0u8; 25]; s.read_exact(&mut c).await.unwrap(); assert_eq!(&c, b"HTTP/1.1 100 Continue\r\n\r\n"); }
            s.write_all(&body).await.unwrap(); s.shutdown().await.unwrap(); // the client is done sending: a short body ends instead of waiting
            let mut out = Vec::new(); let _ = s.read_to_end(&mut out).await; /* a reset after the reply is read is no failure of the reply */ let t = String::from_utf8_lossy(&out).into_owned();
            let (head, body) = t.split_once("\r\n\r\n").unwrap(); (head.to_string(), serde_json::from_str::<Value>(body).unwrap()) };
        let data: Vec<u8> = (0..300_000u32).map(|i| (i % 253) as u8).collect();
        let (head, j) = post("thread=t1&key=k9&name=pic.png".into(), data.len(), data.clone(), true).await;
        assert!(head.starts_with("HTTP/1.1 200") && head.contains("Content-Length:") && !head.to_ascii_lowercase().contains("transfer-encoding"), "{head}");
        assert_eq!(j["attached"].as_str().unwrap(), home.join("attachments/t1/k9-pic.png").to_string_lossy()); assert_eq!(j["thread"], "t1");
        assert_eq!(std::fs::read(home.join("attachments/t1/k9-pic.png")).unwrap(), data);
        // the setting says 1 MB: 2 MB is refused before the client is told to send it
        let (head, j) = post("thread=t1&key=k&name=big".into(), 2 << 20, vec![], false).await; assert!(head.starts_with("HTTP/1.1 413"), "{head}"); assert_eq!((j["uiError"].as_str().unwrap(), j["uiDetail"].as_str().unwrap()), ("upload.cap", "1")); assert!(!home.join("attachments/t1/k-big").exists());
        // a body that stops short is refused and leaves no file; unknown and deleted threads get no folder
        let (head, j) = post("thread=t1&key=k&name=short".into(), 5000, vec![1; 100], false).await; assert!(head.starts_with("HTTP/1.1 400"), "{head}"); assert_eq!(j["uiError"], "upload.incomplete");
        for t in ["nope", "t2", ""] { let (head, _) = post(format!("thread={t}&key=k&name=n"), 3, b"abc".to_vec(), false).await; assert!(head.starts_with("HTTP/1.1 404"), "{t}: {head}"); }
        assert!(!home.join("attachments/nope").exists() && !home.join("attachments/t2").exists());
    }
}

//! GET /debug/diagnostics.json (server.bend, docs/diagnostics-backend.md): the sanitized, bounded local diagnostics document, read by the client
//! only when the person elects diagnostics for a report. The hub has already checked the pairing token. No raw logs; `diagnostics::export()` re-allowlists
//! the persisted bytes and is capped at 1 MiB.
use crate::{api::{Fut, HttpHandler, HttpReply}, auth::Req, hub::Hub};
use std::sync::Arc;

pub struct DiagnosticsJson;
impl HttpHandler for DiagnosticsJson {
    fn routes(&self) -> &'static [&'static str] { &["/debug/diagnostics.json"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        // GET only, like Bend's route: anything else is not found
        if req.method != "GET" { return Box::pin(async { HttpReply::text("404 Not Found", "not found") }) }
        Box::pin(async { match tokio::task::spawn_blocking(super::diagnostics::export).await {
            Ok(text) => HttpReply { status: "200 OK", ctype: "application/json".into(), extra: "Cache-Control: no-store\r\n".into(), body: text.into_bytes() },
            Err(_) => HttpReply::text("503 Service Unavailable", "diagnostics unavailable") } })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}};
    const TOKEN: &str = "tok-improvements-0123456789";
    // The route through the hub's real HTTP dispatch: default-deny without the token, the document with it, GET only.
    #[tokio::test] async fn the_route_needs_the_token_answers_json_and_only_to_get() {
        let dir = std::env::temp_dir().join(format!("bp-diag-http-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("token"), TOKEN).unwrap(); std::fs::write(dir.join("events.jsonl"), "").unwrap();
        let cfg = crate::hub::Cfg { home: dir.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: serde_json::json!({}) })).await.unwrap();
        hub.register_http(Arc::new(DiagnosticsJson));
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
        { let hub = hub.clone(); tokio::spawn(async move { loop { let (s, _) = l.accept().await.unwrap(); let h = hub.clone(); tokio::spawn(async move { let _ = crate::wire::serve(h, s).await; }); } }); }
        let ask = |method: &'static str, auth: bool| async move {
            let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let a = if auth { format!("Authorization: Bearer {TOKEN}\r\n") } else { String::new() };
            s.write_all(format!("{method} /debug/diagnostics.json HTTP/1.1\r\nHost: x\r\n{a}Content-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
            let mut out = Vec::new(); let _ = tokio::time::timeout(std::time::Duration::from_secs(5), s.read_to_end(&mut out)).await; String::from_utf8_lossy(&out).into_owned() };
        assert!(ask("GET", false).await.starts_with("HTTP/1.1 401"), "no token");
        let ok = ask("GET", true).await; assert!(ok.starts_with("HTTP/1.1 200") && ok.to_ascii_lowercase().contains("application/json") && ok.contains("no-store"), "{ok}");
        let doc: serde_json::Value = serde_json::from_str(ok.split_once("\r\n\r\n").unwrap().1).unwrap(); assert_eq!(doc["version"], 1); assert!(doc["events"].is_array());
        assert!(ask("POST", true).await.starts_with("HTTP/1.1 404"), "GET only");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

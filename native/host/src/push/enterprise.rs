//! The Enterprise push gateway: when this hub has no APNs key of its own, it hands the alert to the person's Enterprise server (`POST <url>/v1/push/send`), which holds the
//! key and the phone's token (the phone registers it there directly). Both URL and bearer token live in the machine-local secret file `<home>/secrets/enterprise.json`
//! (`{"url":"https://...","token":"..."}`), never a shared/synced setting. The token goes to `curl` on stdin, never on a command line, and is never logged.
use super::apns::Frame;
use crate::hub::{now_ms, Hub};
use serde_json::{json, Value};
use std::{io::Write, path::Path, sync::{atomic::{AtomicU64, Ordering}, Arc}}; use crate::plat::fs::{DirBuilderExt, OpenOptionsExt};

static SEQ: AtomicU64 = AtomicU64::new(0);

fn is_loopback_http(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else { return false };
    let host_port = rest.split('/').next().unwrap_or("");
    if host_port.starts_with("[::1]") { return true }
    let host = host_port.split(':').next().unwrap_or("");
    matches!(host, "127.0.0.1" | "localhost")
}

fn allowed_url(url: &str) -> bool {
    let u = url.trim_end_matches('/');
    (u.starts_with("https://") && u.len() > "https://".len()) || is_loopback_http(u)
}

/// The server's URL and the person's token, when both are set in `<home>/secrets/enterprise.json`.
pub async fn credentials(hub: &Arc<Hub>) -> Option<(String, String)> {
    let text = std::fs::read_to_string(hub.cfg.home.join("secrets/enterprise.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let raw_url = v["url"].as_str()?.trim();
    if !allowed_url(raw_url) { return None }
    let url = raw_url.trim_end_matches('/').to_string();
    let token = v["token"].as_str()?.trim().to_string();
    (!token.is_empty()).then_some((url, token))
}

fn safe_reason(s: &str) -> &str {
    let long_hex = s.split(|c: char| !c.is_ascii_hexdigit()).any(|seg| seg.len() >= 32);
    if s.contains("://") || s.contains("/3/device/") || long_hex { "Enterprise push delivery failed" } else { s }
}

/// What the gateway's answer means: how many phones it reached, and why none was (None when the person simply has no phone registered there or unmuted).
pub fn outcome(code: u16, body: &str) -> (u64, Option<String>) {
    match code {
        200 => {
            let v: Value = serde_json::from_str(body).unwrap_or_default();
            let (sent, failed) = (v["sent"].as_u64().unwrap_or(0), v["failed"].as_u64().unwrap_or(0));
            if sent > 0 { return (sent, None) }
            if failed == 0 { return (0, None) }
            let reason = v["results"].as_array()
                .and_then(|a| a.iter().find_map(|r| (r["status"].as_u64() != Some(200)).then(|| r["reason"].as_str().filter(|s| !s.is_empty())).flatten()))
                .or_else(|| v["errors"].as_array().and_then(|a| a.first()).and_then(|e| e.as_str().or_else(|| e["reason"].as_str())).filter(|s| !s.is_empty()))
                .map(safe_reason)
                .unwrap_or("Enterprise push delivery failed");
            (0, Some(reason.to_string()))
        }
        401 | 403 => (0, Some("Enterprise push sign-in expired or revoked".into())),
        429 => (0, Some("Enterprise push rate limited".into())),
        _ => (0, Some(format!("Enterprise push HTTP {code}"))),
    }
}

fn post(home: &Path, url: &str, token: &str, body: &str) -> Result<(u16, String), String> {
    let tmp = home.join("tmp"); std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let d = tmp.join(format!("push-ent-{}-{:x}-{}", now_ms(), std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
    std::fs::DirBuilder::new().mode(0o700).create(&d).map_err(|e| e.to_string())?;
    let file = d.join("body.json");
    let r = (|| {
        std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&file).and_then(|mut x| x.write_all(body.as_bytes())).map_err(|e| e.to_string())?;
        let mut c = crate::proc::std_cmd(crate::proc::Kind::Net, "curl");
        c.args(["-sS", "--proto", "=http,https", "--max-time", "10", "--max-redirs", "0", "-X", "POST", "-H", "@-", "--data-binary"]).arg(format!("@{}", file.display()))
            .args(["-w", "\n%{http_code}", "--url"]).arg(format!("{url}/v1/push/send")).current_dir(&d)
            .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
        let headers = format!("Authorization: Bearer {token}\nContent-Type: application/json\n");
        let out = super::apns::pipe_and_wait(&mut c, headers.as_bytes(), "curl is missing", "the Enterprise server did not answer")?;
        let text = String::from_utf8_lossy(&out); let (b, code) = text.rsplit_once('\n').unwrap_or(("", text.as_ref()));
        Ok((code.trim().parse::<u16>().unwrap_or(0), b.to_string()))
    })();
    let _ = std::fs::remove_dir_all(&d);
    r
}

/// Send one frame through the gateway: how many phones it reached, and why it reached none (never the token).
pub async fn send(hub: &Arc<Hub>, url: &str, token: &str, f: &Frame, through: bool) -> (u64, Option<String>) {
    if crate::copy::on() { return (0, Some("a copy of a log sends nothing".into())) }
    let body = json!({"pushType": f.push_type, "priority": f.priority, "collapseId": f.collapse, "payload": f.payload, "through": through}).to_string();
    let (home, url, token) = (hub.cfg.home.clone(), url.to_string(), token.to_string());
    match tokio::task::spawn_blocking(move || post(&home, &url, &token, &body)).await {
        Ok(Ok((code, body))) => { let r = outcome(code, &body); eprintln!("push: Enterprise {} {}: HTTP {code}", f.push_type, f.collapse); r }
        Ok(Err(e)) => { eprintln!("push: Enterprise not sent: {e}"); (0, Some(format!("Enterprise push not sent: {e}"))) }
        Err(_) => (0, Some("Enterprise push task failed".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};
    use std::io::{Read, Write as _};

    async fn hub(tag: &str) -> crate::hub::TestHub {
        let dir = crate::plat::tmp::dir(&format!("ent-push-{tag}"));
        std::fs::write(dir.join("events.jsonl"), "").unwrap(); std::fs::write(dir.join("token"), "tok").unwrap();
        let h = Hub::start(Cfg { home: dir.to_path_buf(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        crate::hub::TestHub::new(h, dir)
    }
    fn frame() -> Frame { Frame { push_type: "alert", priority: 10, collapse: "k1".into(), payload: json!({"aps":{"badge":1}}) } }
    /// A server that answers one request with `status` and `reply`, and hands back the raw request it got.
    fn serve(status: u16, reply: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        (url, std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap(); let mut got = Vec::new(); let mut buf = [0u8; 4096];
            loop {
                let n = s.read(&mut buf).unwrap(); got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some((head, rest)) = text.split_once("\r\n\r\n") {
                    let len = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                    if rest.len() >= len { break }
                }
                if n == 0 { break }
            }
            let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len());
            String::from_utf8_lossy(&got).to_string()
        }))
    }

    #[tokio::test] async fn a_delivered_push_posts_the_frame_with_the_bearer_and_counts_phones() {
        let h = hub("ok").await; let (url, srv) = serve(200, r#"{"sent":1,"failed":0}"#);
        let r = send(&h, &url, "bpe_u_secret", &frame(), true).await; assert_eq!(r, (1, None));
        let req = srv.join().unwrap();
        assert!(req.starts_with("POST /v1/push/send "), "{req}");
        assert!(req.to_ascii_lowercase().contains("authorization: bearer bpe_u_secret"), "{req}");
        let v: Value = serde_json::from_str(req.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(v, json!({"pushType":"alert","priority":10,"collapseId":"k1","payload":{"aps":{"badge":1}},"through":true}));
        assert!(std::fs::read_dir(h.cfg.home.join("tmp")).unwrap().next().is_none(), "the per-send folder is removed");
    }
    #[tokio::test] async fn no_registered_phone_is_not_a_failure() {
        let h = hub("none").await; let (url, srv) = serve(200, r#"{"sent":0,"failed":0}"#);
        assert_eq!(send(&h, &url, "t", &frame(), false).await, (0, None)); srv.join().unwrap();
    }
    #[tokio::test] async fn a_failed_send_gives_its_reason() {
        let h = hub("fail").await; let (url, srv) = serve(200, r#"{"sent":0,"failed":1,"results":[{"handle":"abcdef012345","status":400,"reason":"BadDeviceToken"}]}"#);
        assert_eq!(send(&h, &url, "t", &frame(), false).await, (0, Some("BadDeviceToken".into()))); srv.join().unwrap();
        let (url, srv) = serve(200, r#"{"sent":0,"failed":1,"errors":["APNs unreachable"]}"#);
        assert_eq!(send(&h, &url, "t", &frame(), false).await, (0, Some("APNs unreachable".into()))); srv.join().unwrap();
        let (url, srv) = serve(200, r#"{"sent":0,"failed":2}"#);
        assert_eq!(send(&h, &url, "t", &frame(), false).await, (0, Some("Enterprise push delivery failed".into()))); srv.join().unwrap();
        let leaked_url = format!(r#"{{"sent":0,"failed":1,"results":[{{"handle":"h1","status":502,"reason":"error for url (https://api.push.apple.com/3/device/{})"}}]}}"#, "aa".repeat(32));
        assert_eq!(outcome(200, &leaked_url), (0, Some("Enterprise push delivery failed".into())));
    }
    #[tokio::test] async fn a_refused_sign_in_is_said_plainly() {
        let h = hub("401").await; let (url, srv) = serve(401, r#"{"error":"no"}"#);
        assert_eq!(send(&h, &url, "t", &frame(), false).await, (0, Some("Enterprise push sign-in expired or revoked".into()))); srv.join().unwrap();
    }
    #[tokio::test] async fn a_rate_limit_and_other_statuses() {
        let h = hub("429").await; let (url, srv) = serve(429, "{}");
        assert_eq!(send(&h, &url, "t", &frame(), false).await, (0, Some("Enterprise push rate limited".into()))); srv.join().unwrap();
        let (url, srv) = serve(500, "{}");
        assert_eq!(send(&h, &url, "t", &frame(), false).await, (0, Some("Enterprise push HTTP 500".into()))); srv.join().unwrap();
    }
    #[tokio::test] async fn credentials_come_from_the_secret_json_file() {
        let h = hub("cred").await;
        assert_eq!(credentials(&h).await, None, "no file yet");
        let p = h.cfg.home.join("secrets/enterprise.json");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, r#"{"url":"  https://enterprise.example.com/ ","token":" bpe_u_x \n"}"#).unwrap();
        assert_eq!(credentials(&h).await, Some(("https://enterprise.example.com".into(), "bpe_u_x".into())));
        std::fs::write(&p, r#"{"url":"http://127.0.0.1:8080/","token":"t"}"#).unwrap();
        assert_eq!(credentials(&h).await, Some(("http://127.0.0.1:8080".into(), "t".into())));
        std::fs::write(&p, r#"{"url":"http://enterprise.example.com","token":"t"}"#).unwrap();
        assert_eq!(credentials(&h).await, None, "plain http rejected outside loopback");
        std::fs::write(&p, r#"{"url":"ftp://enterprise.example.com","token":"t"}"#).unwrap();
        assert_eq!(credentials(&h).await, None, "only https or loopback http is allowed");
    }
    #[test] fn the_server_is_machine_local() { assert!(!crate::net::settings_sync::is_shared("enterprise.url")); }
}

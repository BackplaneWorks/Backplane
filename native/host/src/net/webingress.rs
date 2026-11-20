//! Request checks shared by all dedicated HTTPS ingress transports.
//! Production accepts the proxy's Tailscale-User-Login assertion over a Unix socket in
//! a 0700 directory reserved for its Tailscale backend. A loopback TCP backend is forbidden.
//! Same-user processes already have access to the native hub token and fall inside the local
//! trust boundary. Directory permissions alone cannot identify the proxy among those processes.
//! Before reading headers, the production transport must check OS peer credentials where available
//! against the expected proxy child or dedicated system tailscaled identity and reject strangers.
//! The login still requires a paired device cookie for protected data and native approval for enrollment.
use crate::auth::Req;

/// The transport supplies its configured public authority and verifies the
/// identity of the incoming connection before passing the request here.
pub(crate) struct BrowserIngress<'a> {
    pub authority: &'a str,
    pub verified_login: &'a str,
}

fn single_header<'a>(req: &'a Req, name: &str) -> Option<&'a str> {
    let mut values = req.headers.iter().filter(|(key, _)| key.eq_ignore_ascii_case(name));
    let value = values.next()?.1.as_str();
    values.next().is_none().then_some(value)
}

impl BrowserIngress<'_> {
    pub(crate) fn accepts(&self, req: &Req, upgrade: bool) -> bool {
        if self.verified_login.is_empty() || self.authority.is_empty()
            || single_header(req, "host") != Some(self.authority)
        {
            return false;
        }
        let needs_origin = upgrade || req.method != "GET";
        let mut origins = req.headers.iter().filter(|(key, _)| key.eq_ignore_ascii_case("origin"));
        match origins.next() {
            None => !needs_origin,
            Some((_, value)) => origins.next().is_none()
                && value.strip_prefix("https://") == Some(self.authority),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_lazy_assets_require_paired_session() {
        for path in ["/assets/terminal.js", "/assets/xterm.css"] {
            assert!(static_asset(path, false).is_none(), "{path} must require pairing");
            assert!(static_asset(path, true).is_some(), "{path} must be served when paired");
        }
    }

    #[tokio::test]
    async fn ws_upgrade_targets_enforce_exact_allowlist_and_auth() {
        use futures_util::{SinkExt, StreamExt};
        use serde_json::{json, Value};
        use std::sync::Arc;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

        let dir = crate::plat::tmp::dir("webingress-ws-targets");
        std::fs::write(dir.join("token"), "tok").unwrap();
        std::fs::write(
            dir.join("events.jsonl"),
            "{\"$\":\"SettingSet\",\"key\":\"web.enabled\",\"value\":\"1\",\"at\":1700000040}\n",
        )
        .unwrap();
        let cfg = crate::hub::Cfg {
            home: dir.to_path_buf(),
            version: "0.16.4".into(),
            commit: "test".into(),
            listen: "127.0.0.1:0".into(),
            os: "linux".into(),
            arch: "x64".into(),
        };
        let hub = crate::hub::Hub::start(cfg, Arc::new(crate::presenter::Canned { reply: json!({}) }))
            .await
            .unwrap();
        let (reg, _routes) = crate::net::start::open(&hub.cfg.home, "127.0.0.1", 0).unwrap();
        let (req_json, poll_cookie) = reg.browser_request("person@example.com", "Test Browser").unwrap();
        assert!(reg.browser_decide(req_json["id"].as_str().unwrap(), true));
        let session_cookie = reg.browser_wait("person@example.com", &poll_cookie).await.unwrap().unwrap();
        let authority = "web.example.test:8443";

        let raw_status = |target: String, cookie: Option<String>| {
            let hub = hub.clone();
            async move {
                let (mut client, server) = tokio::io::duplex(16 * 1024);
                let srv = tokio::spawn(async move { serve(hub, server, authority).await });
                let mut req = format!(
                    "GET {target} HTTP/1.1\r\nHost: {authority}\r\nOrigin: https://{authority}\r\n\
                     Tailscale-User-Login: person@example.com\r\nConnection: Upgrade\r\n\
                     Upgrade: websocket\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                     Sec-WebSocket-Version: 13\r\n"
                );
                if let Some(c) = cookie {
                    req.push_str(&format!("Cookie: __Host-bp_web={c}\r\n"));
                }
                req.push_str("\r\n");
                client.write_all(req.as_bytes()).await.unwrap();
                let mut resp = Vec::new();
                client.read_to_end(&mut resp).await.unwrap();
                let _ = srv.await;
                let text = String::from_utf8_lossy(&resp);
                text.lines().next().unwrap_or("").to_owned()
            }
        };

        for target in ["/ws?data=1&enc=json", "/ws?data=1&enc=json&z=deflate"] {
            // Unpaired upgrade must get 401 on both exact URLs.
            let status = raw_status(target.to_owned(), None).await;
            assert!(status.starts_with("HTTP/1.1 401 "), "{target} unpaired: got {status}");

            // Paired upgrade must succeed and answer data/1 RPC uncompressed.
            let (client_io, server_io) = tokio::io::duplex(64 * 1024);
            let h2 = hub.clone();
            let srv = tokio::spawn(async move { serve(h2, server_io, authority).await });
            let mut ws_req = format!("ws://{authority}{target}").into_client_request().unwrap();
            let headers = ws_req.headers_mut();
            headers.insert("Origin", format!("https://{authority}").parse().unwrap());
            headers.insert("Tailscale-User-Login", "person@example.com".parse().unwrap());
            headers.insert("Cookie", format!("__Host-bp_web={session_cookie}").parse().unwrap());
            let (mut ws, _) = tokio_tungstenite::client_async(ws_req, client_io).await.unwrap();
            let hello: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
            assert_eq!(hello["t"], "hello", "{target}: expected hello frame");
            assert!(hello.get("z").is_none(), "{target}: browser ingress stays uncompressed");
            ws.send(Message::Text(json!({"id":1,"m":"projects.list","p":{}}).to_string().into())).await.unwrap();
            let reply: Value = serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
            assert_eq!(reply["t"], "reply", "{target}: expected data/1 reply");
            assert_eq!(reply["id"], 1);
            assert_eq!(reply["ok"], true);
            let _ = ws.close(None).await;
            let _ = tokio::time::timeout(std::time::Duration::from_secs(1), srv).await;
        }

        // Unknown query parameters must still be refused with 404 even when paired.
        for bad in [
            "/ws?data=1&enc=json&z=deflate&extra=1",
            "/ws?data=1&enc=json&z=gzip",
            "/ws?data=1&enc=cbor",
        ] {
            let status = raw_status(bad.to_owned(), Some(session_cookie.clone())).await;
            assert!(status.starts_with("HTTP/1.1 404 "), "{bad} must be refused, got {status}");
        }
    }

    #[tokio::test]
    async fn pairing_limits_and_deadline() {
        use crate::net::pairing::{Registry, GUESS_ALL, GUESS_CLIENT, urandom};
        use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
        let home = crate::plat::tmp::dir("browser-limits");
        let now = Arc::new(AtomicU64::new(1_000_000));
        let clock = now.clone();
        let reg = Registry::with(&home, "fixture", Box::new(move || clock.load(Ordering::SeqCst)), Box::new(urandom)).unwrap();
        let make = |peer: &str| reg.browser_request(peer, "Browser");
        let consume = |req: &serde_json::Value| reg.browser_decide(req["id"].as_str().unwrap(), false);
        for i in 0..GUESS_ALL {
            let peer = format!("fixture-{}@example.test", i / GUESS_CLIENT);
            let (req, cookie) = make(&peer).unwrap();
            assert!(consume(&req));
            assert_eq!(reg.browser_wait(&peer, &cookie).await.unwrap_err(), "denied");
            if i + 1 == GUESS_CLIENT { assert_eq!(make(&peer).unwrap_err(), "rate_limited"); }
        }
        assert_eq!(make("fresh@example.test").unwrap_err(), "rate_limited");
        now.fetch_add(60_000, Ordering::SeqCst);
        let (req, cookie) = make("fresh@example.test").unwrap();
        let start = now.load(Ordering::SeqCst);
        assert_eq!(req["expiresAt"].as_u64(), Some(start + 120_000));
        now.store(start + 120_000, Ordering::SeqCst);
        assert!(!reg.browser_decide(req["id"].as_str().unwrap(), true));
        assert_eq!(reg.browser_wait("fresh@example.test", &cookie).await.unwrap_err(), "expired");
    }

    #[test]
    fn unix_proxy_requires_each_forwarded_field_once() {
        let authority = "bp.example.test:8443";
        let request = Req::new("POST", "/", vec![("Host".into(), "localhost".into()),
            ("X-Forwarded-Host".into(), authority.into()), ("X-Forwarded-Proto".into(), "https".into())]);
        for i in 0..3 {
            let mut duplicate = request.clone();
            duplicate.headers.push(request.headers[i].clone());
            assert!(!normalize_unix_authority(&mut duplicate, authority));
            let mut wrong = request.clone();
            wrong.headers[i].1 = "other".into();
            assert!(!normalize_unix_authority(&mut wrong, authority));
        }
        let mut req = request;
        assert!(normalize_unix_authority(&mut req, authority));
        let ingress = BrowserIngress { authority, verified_login: "person@example.com" };
        assert!(!ingress.accepts(&req, false));
        req.headers.push(("Origin".into(), format!("https://{authority}")));
        assert!(ingress.accepts(&req, false));
    }

    #[test]
    fn dedicated_authority_and_requester_required() {
        let request = Req::new("GET", "/", vec![("Host".into(), "web.example.test".into())]);
        let ingress = BrowserIngress { authority: "web.example.test", verified_login: "person@example.com" };
        assert!(ingress.accepts(&request, false));
        assert!(!BrowserIngress { verified_login: "", ..ingress }.accepts(&request, false));
        assert!(!BrowserIngress { authority: "other.example.test", ..ingress }.accepts(&request, false));
        let mut ambiguous = request.clone();
        ambiguous.headers.push(("host".into(), ingress.authority.into()));
        assert!(!ingress.accepts(&ambiguous, false));
    }

    #[test]
    fn csrf_sensitive_operations_require_one_exact_https_origin() {
        let ingress = BrowserIngress { authority: "web.example.test:8443", verified_login: "person@example.com" };
        for (method, upgrade) in [("POST", false), ("GET", true)] {
            let mut request = Req::new(method, "/", vec![("Host".into(), ingress.authority.into())]);
            assert!(!ingress.accepts(&request, upgrade));
            for origin in ["http://web.example.test:8443", "https://web.example.test", "null"] {
                request.headers.truncate(1);
                request.headers.push(("Origin".into(), origin.into()));
                assert!(!ingress.accepts(&request, upgrade));
            }
            request.headers.truncate(1);
            request.headers.push(("Origin".into(), "https://web.example.test:8443".into()));
            assert!(ingress.accepts(&request, upgrade));
            request.headers.push(request.headers[1].clone());
            assert!(!ingress.accepts(&request, upgrade));
        }
    }
}

const SECURITY_HEADERS: &str = concat!(
    "Cache-Control: private, no-store\r\n",
    "Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; ",
    "img-src 'self' blob: data:; font-src 'self'; worker-src 'self'; connect-src 'self'; ",
    "object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'\r\n",
    "X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n",
);

fn json_response(status: &str, body: serde_json::Value, extra: &str) -> Vec<u8> {
    crate::wire::response_for(status, "application/json", &format!("{SECURITY_HEADERS}{extra}"), body.to_string().as_bytes())
}

fn cookie_header(name: &str, value: &str, seconds: u32) -> String {
    format!("Set-Cookie: {name}={value}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={seconds}\r\n")
}

fn poll_cookie(req: &Req) -> Option<&str> {
    let mut cookies = single_header(req, "cookie")?.split(';')
        .filter_map(|field| field.trim().strip_prefix("__Host-bp_web_pair="));
    let cookie = cookies.next()?;
    cookies.next().is_none().then_some(cookie)
}

async fn pair_response(hub: &crate::hub::Hub, req: &Req, login: &str) -> Vec<u8> {
    use serde_json::json;
    let Some(registry) = crate::net::start::registry(&hub.cfg.home) else {
        return json_response("503 Service Unavailable", json!({"state":"unavailable"}), "");
    };
    let error = match req.route() {
        "/auth/web/request" => {
            let input = serde_json::from_slice::<serde_json::Value>(&req.body).ok();
            let Some(name) = input.as_ref().and_then(|v| v["name"].as_str()) else {
                return json_response("400 Bad Request", json!({"state":"invalid_request"}), "");
            };
            match registry.browser_request(login, name) {
                Ok((request, cookie)) => return json_response("200 OK", request, &cookie_header("__Host-bp_web_pair", &cookie, 120)),
                Err(error) => error,
            }
        }
        "/auth/web/poll" => {
            match registry.browser_wait(login, poll_cookie(req).unwrap_or("")).await {
                Ok(Some(cookie)) => {
                    if hub.setting("web.enabled").await.as_deref() != Some("1") {
                        return json_response("404 Not Found", json!({"state":"disabled"}), "");
                    }
                    let headers = cookie_header("__Host-bp_web", &cookie, 90 * 86400)
                        + &cookie_header("__Host-bp_web_pair", "", 0);
                    return json_response("200 OK", json!({"state":"approved"}), &headers);
                }
                Ok(None) => return json_response("202 Accepted", json!({"state":"pending"}), ""),
                Err(error) => error,
            }
        }
        "/auth/web/logout" => {
            let Some(device) = crate::auth::browser_device(&registry, req, login) else {
                return json_response("401 Unauthorized", json!({"state":"unpaired"}), "");
            };
            let result = registry.revoke(&device);
            if result["ok"] == true {
                let clients = result["clients"].as_array().into_iter().flatten().filter_map(|id| id.as_u64()).collect::<Vec<_>>();
                hub.close_clients(&clients);
                return json_response("200 OK", json!({"state":"revoked"}), &cookie_header("__Host-bp_web", "", 0));
            }
            "storage_unavailable"
        }
        _ => return json_response("404 Not Found", json!({}), ""),
    };
    let status = match error {
        "rate_limited" => "429 Too Many Requests",
        "denied" | "expired" | "unknown_request" | "unverified_peer" => "401 Unauthorized",
        _ => "503 Service Unavailable",
    };
    let headers = if req.route() == "/auth/web/poll" && matches!(error, "expired" | "denied" | "unknown_request") {
        cookie_header("__Host-bp_web_pair", "", 0)
    } else {
        String::new()
    };
    json_response(status, json!({"state":error}), &headers)
}

/// Only the dedicated listener calls this, after authenticating its TLS proxy.
pub(crate) async fn serve<S>(hub: std::sync::Arc<crate::hub::Hub>, stream: S, authority: &str)
    -> anyhow::Result<()>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send {
    serve_request(hub, stream, authority, false).await
}

/// Called only after the transport authenticates the Unix peer.
#[cfg(unix)]
pub(crate) async fn serve_unix(hub: std::sync::Arc<crate::hub::Hub>, stream: tokio::net::UnixStream, authority: &str) -> anyhow::Result<()> {
    serve_request(hub, stream, authority, true).await
}

fn normalize_unix_authority(req: &mut Req, authority: &str) -> bool {
    if single_header(req, "host") != Some("localhost")
        || single_header(req, "x-forwarded-host") != Some(authority)
        || single_header(req, "x-forwarded-proto") != Some("https") {
        return false;
    }
    for (name, value) in &mut req.headers {
        if name.eq_ignore_ascii_case("host") { *value = authority.to_owned(); }
    }
    true
}

async fn serve_request<S>(hub: std::sync::Arc<crate::hub::Hub>, stream: S, authority: &str, unix_proxy: bool)
    -> anyhow::Result<()>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use serde_json::json;
    use std::time::Duration;
    let (mut reader, mut writer) = tokio::io::split(stream);
    let mut bytes = Vec::new();
    let parsed = tokio::time::timeout(Duration::from_secs(5), async {
        let mut chunk = [0u8; 2048];
        while bytes.len() < 16384 {
            let n = reader.read(&mut chunk).await?;
            if n == 0 { break }
            bytes.extend_from_slice(&chunk[..n]);
            let mut headers = [httparse::EMPTY_HEADER; 64];
            let mut http = httparse::Request::new(&mut headers);
            if let httparse::Status::Complete(end) = http.parse(&bytes)? {
                let headers = http.headers.iter().map(|h| Ok((h.name.to_owned(), std::str::from_utf8(h.value)?.to_owned())))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                return Ok((end, Req::new(http.method.unwrap_or(""), http.path.unwrap_or(""), headers)));
            }
        }
        anyhow::bail!("invalid browser request")
    }).await??;
    let (end, mut req) = parsed;
    let login = single_header(&req, "tailscale-user-login").unwrap_or("").to_owned();
    let ingress = BrowserIngress { authority, verified_login: &login };
    let upgrade = req.route() == "/ws";
    let reply = if hub.setting("web.enabled").await.as_deref() != Some("1") {
        json_response("404 Not Found", json!({}), "")
    } else if (unix_proxy && !normalize_unix_authority(&mut req, authority)) || !ingress.accepts(&req, upgrade) || login == "tagged-devices" {
        json_response("401 Unauthorized", json!({"state":"unauthorized"}), "")
    } else if req.target.split_once('?').is_some_and(|(_, query)| query.split('&').any(|field| {
        let key = field.split('=').next().unwrap_or("");
        crate::files::fm::from_uri(key).eq_ignore_ascii_case("token")
    })) {
        json_response("400 Bad Request", json!({"state":"invalid_request"}), "")
    } else if req.method == "GET" && req.target == "/auth/web/info" {
        json_response("200 OK", json!({"address":format!("https://{authority}")}), "")
    } else if upgrade && req.method == "GET" && is_browser_ws_target(&req.target) {
        let registry = crate::net::start::registry(&hub.cfg.home);
        if let Some(device_id) = registry.and_then(|r| crate::auth::browser_device(&r, &req, &login)) {
            let stream = tokio::io::join(std::io::Cursor::new(bytes).chain(reader), writer);
            return crate::data::socket_with(hub, stream, true, crate::data::Principal::Browser { device_id }, authority.into()).await;
        }
        json_response("401 Unauthorized", json!({"state":"unpaired"}), "")
    } else if req.method == "GET" && (req.target == "/" || req.target.starts_with("/assets/")) {
        let paired = crate::net::start::registry(&hub.cfg.home)
            .and_then(|r| crate::auth::browser_device(&r, &req, &login)).is_some();
        match static_asset(&req.target, paired) {
            Some((mime, body)) => crate::wire::response_for("200 OK", mime, SECURITY_HEADERS, body),
            None if !paired => json_response("401 Unauthorized", json!({"state":"unpaired"}), ""),
            None => json_response("404 Not Found", json!({}), ""),
        }
    } else if req.method == "GET" && is_protected_asset(req.route()) {
        let registry = crate::net::start::registry(&hub.cfg.home);
        if registry.and_then(|r| crate::auth::browser_device(&r, &req, &login)).is_none() {
            json_response("401 Unauthorized", json!({"state":"unpaired"}), "")
        } else {
            let result = if req.route() == "/message-image" {
                crate::files::imgroutes::snapshot_response(&hub, &req, None).await
            } else if let Some(handler) = hub.http_for(req.route()) {
                handler.call(&hub, &req).await
            } else {
                crate::api::HttpReply::text("404 Not Found", "Unavailable")
            };
            // Only carry the content disposition from the underlying resource.
            let mut headers = SECURITY_HEADERS.to_owned();
            for line in result.extra.lines() {
                if line.to_ascii_lowercase().starts_with("content-disposition:") {
                    headers.push_str(line.trim_end());
                    headers.push_str("\r\n");
                }
            }
            if result.ctype.eq_ignore_ascii_case("image/svg+xml") {
                headers.push_str("Content-Disposition: attachment\r\n");
            }
            crate::wire::response_for(result.status, &result.ctype, &headers, &result.body)
        }
    } else if req.method == "POST" && req.target == "/voice/in" {
        let registry = crate::net::start::registry(&hub.cfg.home);
        let device = registry.as_ref().and_then(|r| crate::auth::browser_device(r, &req, &login));
        let size = single_header(&req, "content-length").and_then(|v| v.parse::<usize>().ok());
        let status = if let (Some(device), Some(reg)) = (device, registry) {
            if size.is_some_and(|n| n > 1 << 20) { "413 Payload Too Large" }
            else if let Some(len) = size.filter(|&n| n >= bytes.len() - end
                && !req.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("transfer-encoding"))) {
                let mut body = bytes.split_off(end);
                let buffered = body.len();
                body.resize(len, 0);
                tokio::time::timeout(Duration::from_secs(5), reader.read_exact(&mut body[buffered..])).await??;
                if hub.setting("web.enabled").await.as_deref() != Some("1")
                    || crate::auth::browser_device(&reg, &req, &login).as_ref() != Some(&device) { "401 Unauthorized" }
                else if let (Some(service), Some(key)) = (hub.voice_service.get(), single_header(&req, "x-voice-session")) {
                    let end_audio = single_header(&req, "x-voice-end") == Some("1");
                    match service.browser_chunk(&device, key, body, end_audio) {
                        Ok(client) => {
                            let activity = crate::data::webpolicy::policy("POST /voice/in", &serde_json::Value::Null)
                                == crate::data::webpolicy::Policy::Activity;
                            reg.browser_connection(&device, client, activity);
                            "200 OK"
                        }
                        Err(status) => status,
                    }
                } else { "403 Forbidden" }
            } else { "400 Bad Request" }
        } else { "401 Unauthorized" };
        json_response(status, json!({}), "")
    } else if req.method == "POST" && req.target == req.route() && req.route().starts_with("/auth/web/") {
        let size = single_header(&req, "content-length").and_then(|v| v.parse::<usize>().ok());
        if let Some(len) = size.filter(|&n| n <= 4096 && n >= bytes.len() - end
            && !req.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("transfer-encoding"))) {
            req.body = bytes.split_off(end);
            let buffered = req.body.len();
            req.body.resize(len, 0);
            tokio::time::timeout(Duration::from_secs(5), reader.read_exact(&mut req.body[buffered..])).await??;
            pair_response(&hub, &req, &login).await
        } else {
            json_response("400 Bad Request", json!({"state":"invalid_request"}), "")
        }
    } else {
        json_response("404 Not Found", json!({}), "")
    };
    writer.write_all(&reply).await?;
    writer.shutdown().await?;
    Ok(())
}


fn is_browser_ws_target(target: &str) -> bool {
    matches!(target, "/ws?data=1&enc=json" | "/ws?data=1&enc=json&z=deflate")
}

fn is_protected_asset(path: &str) -> bool {
    matches!(path, "/files/get" | "/message-image") ||
        path.strip_prefix("/bots/").and_then(|p| p.strip_suffix("/screen.jpg"))
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
}

fn static_asset(path: &str, paired: bool) -> Option<(&'static str, &'static [u8])> {
    macro_rules! asset {
        ($mime:literal, $path:literal) => {
            Some(($mime, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/dist/", $path))))
        }
    }
    match path {
        "/" if paired => asset!("text/html; charset=utf-8", "index.html"),
        "/" => asset!("text/html; charset=utf-8", "pair.html"),
        "/assets/pair.js" => asset!("application/javascript", "assets/pair.js"),
        "/assets/app.js" if paired => asset!("application/javascript", "assets/app.js"),
        "/assets/mesh-worker.js" if paired => asset!("application/javascript", "assets/mesh-worker.js"),
        "/assets/terminal.js" if paired => asset!("application/javascript", "assets/terminal.js"),
        "/assets/xterm.css" if paired => asset!("text/css", "assets/xterm.css"),
        "/assets/style.css" => asset!("text/css", "assets/style.css"),
        "/assets/favicon.svg" => asset!("image/svg+xml", "assets/favicon.svg"),
        "/assets/favicon-light.svg" => asset!("image/svg+xml", "assets/favicon-light.svg"),
        _ => None,
    }
}

#[cfg(all(test, unix))]
static FIXTURE_AUTHORITY: std::sync::OnceLock<String> = std::sync::OnceLock::new();

#[cfg(all(test, unix))]
pub(crate) fn start_fixture_proxy(hub: &std::sync::Arc<crate::hub::Hub>) -> anyhow::Result<()> {
    let Some(authority) = FIXTURE_AUTHORITY.get() else { return Ok(()) };
    std::fs::write(hub.cfg.home.join("fixture-http-routes.json"), serde_json::to_vec(&hub.http_routes())?)?;
    let value = std::env::var("BP_WEB_MASK_FIXTURE")?;
    anyhow::ensure!(std::env::var("BACKPLANE_VAULT_KEYRING").as_deref() == Ok("file"));
    anyhow::ensure!(crate::settings::vault::put("browser-fixture", &value, "Synthetic masking test", "fixture") == "ok");
    let marker = format!("Mask check: {value}");
    let listener = tokio::net::UnixListener::bind(hub.cfg.home.join("browser.sock"))?;
    let hub = hub.clone();
    tokio::spawn(async move {
        hub.commit(&serde_json::json!({"$":"MessagePosted","thread":"t1","msg":"mask-check",
            "role":"assistant","text":marker,"at":1700000100})).await.expect("fixture entry");
        while let Ok((stream, _)) = listener.accept().await {
            let hub = hub.clone();
            tokio::spawn(async move { let _ = serve(hub, stream, authority).await; });
        }
    });
    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "started by the test/web browser rig"]
async fn isolated_browser_hub() {
    use std::path::PathBuf;
    assert!(std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none());
    let root = PathBuf::from(std::env::var_os("BP_WEB_FIXTURE").expect("test/web rig directory"));
    let home = root.join("hub");
    std::fs::create_dir(&home).expect("new hub home");
    let port = std::env::var("BP_WEB_NATIVE_PORT").unwrap().parse::<u16>().unwrap();
    let tls = std::env::var("BP_WEB_TLS_PORT").unwrap().parse::<u16>().unwrap();
    assert!(port >= 3799 && tls >= 3799 && port != tls);
    assert_eq!(PathBuf::from(std::env::var_os("BACKPLANE_AGENTS").unwrap()), root.join("agents"));
    FIXTURE_AUTHORITY.set(format!("backplane.example.test:{tls}")).unwrap();
    let project = root.join("project");
    assert!(project.join("ecc83-pp.kicad_pro").is_file());
    std::fs::write(project.join("sample.txt"), "Backplane fixture").unwrap();
    let mut log = [
        serde_json::json!({"$":"ProjectCreated","id":"p1","title":"Browser fixture","root":project,"at":1700000000}),
        serde_json::json!({"$":"ThreadCreated","id":"t1","project":"p1","title":"Browser test","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1700000001}),
    ].map(|e| e.to_string()).join("\n") + "\n";
    for i in 0..45 {
        let entry = serde_json::json!({"$":"MessagePosted","thread":"t1","msg":format!("history-{i}"),
            "role":"assistant","text":format!("History entry {i}"),"at":1700000010+i});
        log.push_str(&entry.to_string());
        log.push('\n');
    }
    std::fs::write(home.join("events.jsonl"), log).unwrap();
    crate::run(crate::Args { window: None, home, port, host: "127.0.0.1".into(), version: "test".into(),
        commit: "test".into(), tailscale: false, https: false }).await.unwrap();
}

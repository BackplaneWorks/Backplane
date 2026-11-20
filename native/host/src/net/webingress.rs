//! Request checks shared by all dedicated HTTPS ingress transports.
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
    } else if !ingress.accepts(&req, upgrade) || login == "tagged-devices" {
        json_response("401 Unauthorized", json!({"state":"unauthorized"}), "")
    } else if req.target.split_once('?').is_some_and(|(_, query)| query.split('&').any(|field| {
        let key = field.split('=').next().unwrap_or("");
        crate::files::fm::from_uri(key).eq_ignore_ascii_case("token")
    })) {
        json_response("400 Bad Request", json!({"state":"invalid_request"}), "")
    } else if upgrade && req.method == "GET" && req.target == "/ws?data=1&enc=json" {
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
    let listener = tokio::net::UnixListener::bind(hub.cfg.home.join("browser.sock"))?;
    let hub = hub.clone();
    tokio::spawn(async move {
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
    std::fs::create_dir(&project).unwrap();
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

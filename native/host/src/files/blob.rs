//! Phase 2b: content-addressed immutable `/blob/<sha256>` HTTP route (docs/remote-architecture.md §5.2).
//!
//! Replaces base64-in-JSON `attachment.get` and `message.image` payloads with raw binary HTTP responses
//! carrying `Cache-Control: private, max-age=31536000, immutable`, `ETag: "<sha256>"` (with `304 Not Modified`),
//! and the hardened F-002 / F-011 media headers (`X-Content-Type-Options: nosniff`,
//! `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox`, `Referrer-Policy: no-referrer`).
//!
//! Security:
//! - Requires standard hub authentication (`own_auth() == false`).
//! - Only serves files explicitly registered by an authorized hub handler (attachments, message image snapshots,
//!   authorized design files) for THIS hub's home directory; arbitrary filesystem paths can never be queried.
use crate::{
    api::{Fut, HttpHandler, HttpReply},
    auth::Req,
    hub::Hub,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

const MAX_BLOBS: usize = 4096;
const MAX_BLOB_BYTES: u64 = 16 << 20; // 16 MiB

#[derive(Clone)]
struct Entry {
    path: PathBuf,
    mime: String,
}

#[derive(Default)]
struct Store {
    by_key: HashMap<(PathBuf, String), Entry>,
    order: VecDeque<(PathBuf, String)>,
}

fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn sanitize_mime(mime: &str) -> String {
    if mime.is_empty() || mime.contains(['\r', '\n', '\0']) {
        "application/octet-stream".to_string()
    } else {
        mime.to_string()
    }
}

fn valid_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Register an authorized file under its content SHA-256 for `/blob/<sha256>`. Returns `(sha256, len)`.
pub fn register_file(home: &Path, path: PathBuf, mime: &str) -> Option<(String, u64)> {
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MAX_BLOB_BYTES {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let sha = hex::encode(Sha256::digest(&bytes));
    let key = (home.to_path_buf(), sha.clone());
    let mut g = store().lock().unwrap();
    if !g.by_key.contains_key(&key) {
        g.order.push_back(key.clone());
        while g.order.len() > MAX_BLOBS {
            if let Some(old) = g.order.pop_front() {
                g.by_key.remove(&old);
            }
        }
    }
    g.by_key.insert(
        key,
        Entry {
            path,
            mime: sanitize_mime(mime),
        },
    );
    Some((sha, bytes.len() as u64))
}

/// Register already-read authorized bytes with their backing path (avoids a second disk read).
pub fn register_bytes(home: &Path, path: PathBuf, mime: &str, bytes: &[u8]) -> String {
    let sha = hex::encode(Sha256::digest(bytes));
    let key = (home.to_path_buf(), sha.clone());
    let mut g = store().lock().unwrap();
    if !g.by_key.contains_key(&key) {
        g.order.push_back(key.clone());
        while g.order.len() > MAX_BLOBS {
            if let Some(old) = g.order.pop_front() {
                g.by_key.remove(&old);
            }
        }
    }
    g.by_key.insert(
        key,
        Entry {
            path,
            mime: sanitize_mime(mime),
        },
    );
    sha
}

fn blob_reply(status: &'static str, ctype: String, etag: &str, body: Vec<u8>) -> HttpReply {
    HttpReply {
        status,
        ctype,
        extra: format!(
            "ETag: {etag}\r\nCache-Control: private, max-age=31536000, immutable\r\n{}",
            super::serve::MEDIA_SECURITY_HEADERS
        ),
        body,
    }
}

fn blob_error(status: &'static str, msg: &str) -> HttpReply {
    HttpReply {
        status,
        ctype: "text/plain; charset=utf-8".into(),
        extra: format!(
            "Cache-Control: private, no-store\r\n{}",
            super::serve::MEDIA_SECURITY_HEADERS
        ),
        body: msg.as_bytes().to_vec(),
    }
}

pub struct BlobHttp;

impl HttpHandler for BlobHttp {
    fn routes(&self) -> &'static [&'static str] {
        &["/blob"]
    }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            let hash = req.route().strip_prefix("/blob/").unwrap_or("");
            if !valid_sha256(hash) {
                return blob_error("404 Not Found", "unknown blob");
            }
            let entry = store()
                .lock()
                .unwrap()
                .by_key
                .get(&(hub.cfg.home.clone(), hash.to_string()))
                .cloned();
            let Some(entry) = entry else {
                return blob_error("404 Not Found", "unknown blob");
            };
            let etag = format!("\"{hash}\"");
            let inm = req.header("if-none-match").trim().to_string();
            let path = entry.path.clone();
            let mime = entry.mime.clone();
            let hash_s = hash.to_string();
            match tokio::task::spawn_blocking(move || std::fs::read(&path)).await {
                Ok(Ok(bytes)) if (bytes.len() as u64) <= MAX_BLOB_BYTES => {
                    let actual = hex::encode(Sha256::digest(&bytes));
                    if actual != hash_s {
                        return blob_error("404 Not Found", "blob changed");
                    }
                    if inm == etag || inm == hash_s || inm == format!("W/{etag}") {
                        return blob_reply("304 Not Modified", mime, &etag, Vec::new());
                    }
                    blob_reply("200 OK", mime, &etag, bytes)
                }
                _ => blob_error("404 Not Found", "blob unavailable"),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};
    use serde_json::json;

    async fn test_hub(dir: &Path) -> Arc<Hub> {
        Hub::start(
            Cfg {
                home: dir.to_path_buf(),
                version: "0.1.0-dev".into(),
                commit: "c".into(),
                listen: "127.0.0.1:0".into(),
                os: "linux".into(),
                arch: "x64".into(),
            },
            Arc::new(Canned { reply: json!({}) }),
        )
        .await
        .unwrap()
    }

    fn assert_hardened_media_headers(extra: &str, expected_cache: &str) {
        assert!(extra.contains(expected_cache), "missing {expected_cache:?}: {extra}");
        assert!(!extra.contains("Cache-Control: public"), "must not be public: {extra}");
        assert!(extra.contains(crate::files::serve::MEDIA_SECURITY_HEADERS), "missing MEDIA_SECURITY_HEADERS: {extra}");
        assert!(extra.contains("Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox\r\n"), "{extra}");
        assert!(extra.contains("Referrer-Policy: no-referrer\r\n"), "{extra}");
        assert!(extra.contains("X-Content-Type-Options: nosniff\r\n"), "{extra}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn blob_serves_immutable_bytes_honors_etag_and_rejects_unregistered() {
        let dir = crate::plat::tmp::dir("blob-http");
        let hub = test_hub(&dir).await;

        let p = dir.join("sample.png");
        let data = b"\x89PNG\r\n\x1a\nfake-png-bytes";
        std::fs::write(&p, data).unwrap();
        let (sha, len) = register_file(&dir, p, "image/png").unwrap();
        assert_eq!(len, data.len() as u64);

        let h = BlobHttp;
        // 1. Registered hash -> 200 OK + private immutable + hardened media headers + ETag
        let req = Req::new("GET", &format!("/blob/{sha}"), vec![]);
        let rep = h.call(&hub, &req).await;
        assert_eq!(rep.status, "200 OK");
        assert_eq!(rep.ctype, "image/png");
        assert_eq!(rep.body, data);
        assert_hardened_media_headers(&rep.extra, "Cache-Control: private, max-age=31536000, immutable\r\n");
        assert!(rep.extra.contains(&format!("ETag: \"{sha}\"")), "{}", rep.extra);

        // 2. If-None-Match (quoted, bare, weak) -> 304 Not Modified (0 bytes) + private immutable + hardened headers
        for inm in [format!("\"{sha}\""), sha.clone(), format!("W/\"{sha}\"")] {
            let req304 = Req::new(
                "GET",
                &format!("/blob/{sha}"),
                vec![("If-None-Match".into(), inm.clone())],
            );
            let rep304 = h.call(&hub, &req304).await;
            assert_eq!(rep304.status, "304 Not Modified", "{inm}");
            assert!(rep304.body.is_empty(), "{inm}");
            assert_hardened_media_headers(&rep304.extra, "Cache-Control: private, max-age=31536000, immutable\r\n");
            assert!(rep304.extra.contains(&format!("ETag: \"{sha}\"")), "{}", rep304.extra);
        }

        // 3. Unknown or invalid hash -> 404 with hardened error headers
        for bad in ["/blob/not-a-hash", "/blob/../etc/passwd", &format!("/blob/{}", "0".repeat(64))] {
            let r = h.call(&hub, &Req::new("GET", bad, vec![])).await;
            assert_eq!(r.status, "404 Not Found", "{bad}");
            assert_hardened_media_headers(&r.extra, "Cache-Control: private, no-store\r\n");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn blob_svg_matches_img_hardened_headers_and_sanitizes_mime_and_hash_changes() {
        let dir = crate::plat::tmp::dir("blob-svg");
        let hub = test_hub(&dir).await;

        let svg = b"<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>";
        let svg_path = dir.join("evil.svg");
        std::fs::write(&svg_path, svg).unwrap();
        let sha = register_bytes(&dir, svg_path.clone(), "image/svg+xml", svg);

        let img_rep = crate::files::serve::send(svg_path.to_str().unwrap(), "image/svg+xml", "");
        let blob_rep = BlobHttp.call(&hub, &Req::new("GET", &format!("/blob/{sha}"), vec![])).await;
        assert_eq!(blob_rep.status, "200 OK");
        assert_eq!(blob_rep.ctype, "image/svg+xml");
        assert_eq!(blob_rep.body, svg);
        assert_hardened_media_headers(&blob_rep.extra, "Cache-Control: private, max-age=31536000, immutable\r\n");
        // Verify exact header line match against /img (serve::head) for CSP, Referrer-Policy, and nosniff.
        for header_prefix in ["Content-Security-Policy:", "Referrer-Policy:", "X-Content-Type-Options:"] {
            let img_line = img_rep.extra.lines().find(|l| l.starts_with(header_prefix)).unwrap();
            let blob_line = blob_rep.extra.lines().find(|l| l.starts_with(header_prefix)).unwrap();
            assert_eq!(blob_line, img_line, "mismatch on {header_prefix}");
        }

        // Caller-supplied MIME containing CRLF/NUL cannot inject response headers.
        let crlf_path = dir.join("crlf.bin");
        std::fs::write(&crlf_path, b"hello").unwrap();
        let crlf_sha = register_bytes(&dir, crlf_path.clone(), "image/svg+xml\r\nX-Injected: 1", b"hello");
        let crlf_rep = BlobHttp.call(&hub, &Req::new("GET", &format!("/blob/{crlf_sha}"), vec![])).await;
        assert_eq!(crlf_rep.status, "200 OK");
        assert_eq!(crlf_rep.ctype, "application/octet-stream");

        // Backing file modification (hash mismatch) and removal -> 404 with hardened headers (for both unconditional and conditional GET).
        std::fs::write(&svg_path, b"<svg>changed</svg>").unwrap();
        for hdrs in [vec![], vec![("If-None-Match".into(), format!("\"{sha}\""))]] {
            let changed = BlobHttp.call(&hub, &Req::new("GET", &format!("/blob/{sha}"), hdrs)).await;
            assert_eq!(changed.status, "404 Not Found");
            assert_eq!(changed.body, b"blob changed");
            assert_hardened_media_headers(&changed.extra, "Cache-Control: private, no-store\r\n");
        }

        std::fs::remove_file(&svg_path).unwrap();
        for hdrs in [vec![], vec![("If-None-Match".into(), format!("\"{sha}\""))]] {
            let missing = BlobHttp.call(&hub, &Req::new("GET", &format!("/blob/{sha}"), hdrs)).await;
            assert_eq!(missing.status, "404 Not Found");
            assert_eq!(missing.body, b"blob unavailable");
            assert_hardened_media_headers(&missing.extra, "Cache-Control: private, no-store\r\n");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn blob_http_wire_enforces_auth_single_private_cache_control_and_csp() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::{TcpListener, TcpStream};

        let dir = crate::plat::tmp::dir("blob-wire");
        let tok = "blob-wire-secret-token";
        std::fs::write(dir.join("token"), tok).unwrap();
        let hub = test_hub(&dir).await;
        hub.register_http(Arc::new(BlobHttp));

        let svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>";
        let svg_path = dir.join("wire.svg");
        std::fs::write(&svg_path, svg).unwrap();
        let sha = register_bytes(&dir, svg_path, "image/svg+xml", svg);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let h_srv = hub.clone();
        let srv = tokio::spawn(async move {
            loop {
                let Ok((s, _)) = listener.accept().await else { break };
                let h = h_srv.clone();
                tokio::spawn(async move {
                    let _ = crate::wire::serve(h, s).await;
                });
            }
        });

        let send_path = |target: String, extra_hdrs: String| async move {
            let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let raw = format!(
                "GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{extra_hdrs}Connection: close\r\n\r\n"
            );
            s.write_all(raw.as_bytes()).await.unwrap();
            let mut out = Vec::new();
            s.read_to_end(&mut out).await.unwrap();
            let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
            (
                String::from_utf8_lossy(&out[..split + 2]).into_owned(),
                out[split + 4..].to_vec(),
            )
        };
        let send_req = |extra_hdrs: String| send_path(format!("/blob/{sha}"), extra_hdrs);

        // 1. Unauthenticated -> 401 with hardened error headers
        let (h_unauth, _) = send_req(String::new()).await;
        assert!(h_unauth.starts_with("HTTP/1.1 401 "), "{h_unauth}");
        assert_eq!(h_unauth.matches("Cache-Control:").count(), 1, "{h_unauth}");
        assert!(h_unauth.contains("Cache-Control: private, no-store\r\n"), "{h_unauth}");
        assert!(h_unauth.contains("Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox\r\n"), "{h_unauth}");
        assert!(h_unauth.contains("Referrer-Policy: no-referrer\r\n"), "{h_unauth}");
        assert!(h_unauth.contains("X-Content-Type-Options: nosniff\r\n"), "{h_unauth}");

        // 2. Authenticated 200 OK -> single private immutable Cache-Control, CSP, Referrer-Policy, nosniff, ETag
        let auth = format!("Authorization: Bearer {tok}\r\n");
        let (h200, b200) = send_req(auth.clone()).await;
        assert!(h200.starts_with("HTTP/1.1 200 OK\r\n"), "{h200}");
        assert_eq!(b200, svg);
        assert!(h200.contains("Content-Type: image/svg+xml\r\n"), "{h200}");
        assert_eq!(h200.matches("Cache-Control:").count(), 1, "{h200}");
        assert!(h200.contains("Cache-Control: private, max-age=31536000, immutable\r\n"), "{h200}");
        assert!(!h200.contains("Cache-Control: public"), "{h200}");
        assert!(h200.contains("Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox\r\n"), "{h200}");
        assert!(h200.contains("Referrer-Policy: no-referrer\r\n"), "{h200}");
        assert!(h200.contains("X-Content-Type-Options: nosniff\r\n"), "{h200}");
        assert!(h200.contains(&format!("ETag: \"{sha}\"\r\n")), "{h200}");

        // 3. Authenticated 304 Not Modified -> empty body, omits Content-Length (RFC 9110 §8.6), same hardened headers
        let (h304, b304) = send_req(format!("{auth}If-None-Match: \"{sha}\"\r\n")).await;
        assert!(h304.starts_with("HTTP/1.1 304 Not Modified\r\n"), "{h304}");
        assert!(b304.is_empty());
        assert!(!h304.to_ascii_lowercase().contains("content-length:"), "304 must not send Content-Length: {h304}");
        assert_eq!(h304.matches("Cache-Control:").count(), 1, "{h304}");
        assert!(h304.contains("Cache-Control: private, max-age=31536000, immutable\r\n"), "{h304}");
        assert!(!h304.contains("Cache-Control: public"), "{h304}");
        assert!(h304.contains("Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox\r\n"), "{h304}");
        assert!(h304.contains("Referrer-Policy: no-referrer\r\n"), "{h304}");
        assert!(h304.contains("X-Content-Type-Options: nosniff\r\n"), "{h304}");
        assert!(h304.contains(&format!("ETag: \"{sha}\"\r\n")), "{h304}");

        // 4. Authenticated 404 Not Found -> hardened error headers + private, no-store
        let (h404, _) = send_path("/blob/not-a-hash".to_string(), auth).await;
        assert!(h404.starts_with("HTTP/1.1 404 Not Found\r\n"), "{h404}");
        assert_eq!(h404.matches("Cache-Control:").count(), 1, "{h404}");
        assert!(h404.contains("Cache-Control: private, no-store\r\n"), "{h404}");
        assert!(h404.contains("Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox\r\n"), "{h404}");
        assert!(h404.contains("Referrer-Policy: no-referrer\r\n"), "{h404}");
        assert!(h404.contains("X-Content-Type-Options: nosniff\r\n"), "{h404}");

        srv.abort();
    }
}

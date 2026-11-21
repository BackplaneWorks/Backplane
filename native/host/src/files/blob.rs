//! Phase 2b: content-addressed immutable `/blob/<sha256>` HTTP route (docs/remote-architecture.md §5.2).
//!
//! Replaces base64-in-JSON `attachment.get` and `message.image` payloads with raw binary HTTP responses
//! carrying `Cache-Control: public, max-age=31536000, immutable` and `ETag: "<sha256>"` (with `304 Not Modified`).
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
            mime: mime.to_string(),
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
            mime: mime.to_string(),
        },
    );
    sha
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
                return HttpReply::text("404 Not Found", "unknown blob");
            }
            let entry = store()
                .lock()
                .unwrap()
                .by_key
                .get(&(hub.cfg.home.clone(), hash.to_string()))
                .cloned();
            let Some(entry) = entry else {
                return HttpReply::text("404 Not Found", "unknown blob");
            };
            let etag = format!("\"{hash}\"");
            let inm = req.header("if-none-match").trim();
            if inm == etag || inm == hash || inm == format!("W/{etag}") {
                return HttpReply {
                    status: "304 Not Modified",
                    ctype: entry.mime,
                    extra: format!(
                        "ETag: {etag}\r\nCache-Control: public, max-age=31536000, immutable\r\nX-Content-Type-Options: nosniff\r\n"
                    ),
                    body: Vec::new(),
                };
            }
            let path = entry.path.clone();
            let mime = entry.mime.clone();
            let hash_s = hash.to_string();
            match tokio::task::spawn_blocking(move || std::fs::read(&path)).await {
                Ok(Ok(bytes)) if (bytes.len() as u64) <= MAX_BLOB_BYTES => {
                    let actual = hex::encode(Sha256::digest(&bytes));
                    if actual != hash_s {
                        return HttpReply::text("404 Not Found", "blob changed");
                    }
                    HttpReply {
                        status: "200 OK",
                        ctype: mime,
                        extra: format!(
                            "ETag: {etag}\r\nCache-Control: public, max-age=31536000, immutable\r\nX-Content-Type-Options: nosniff\r\n"
                        ),
                        body: bytes,
                    }
                }
                _ => HttpReply::text("404 Not Found", "blob unavailable"),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};
    use serde_json::json;

    #[tokio::test(flavor = "multi_thread")]
    async fn blob_serves_immutable_bytes_honors_etag_and_rejects_unregistered() {
        let dir = crate::plat::tmp::dir("blob-http");
        let hub = Hub::start(
            Cfg {
                home: dir.to_path_buf(),
                version: "0.1.0-dev".into(),
                commit: "c".into(),
                listen: "x".into(),
                os: "linux".into(),
                arch: "x64".into(),
            },
            Arc::new(Canned { reply: json!({}) }),
        )
        .await
        .unwrap();

        let p = dir.join("sample.png");
        let data = b"\x89PNG\r\n\x1a\nfake-png-bytes";
        std::fs::write(&p, data).unwrap();
        let (sha, len) = register_file(&dir, p, "image/png").unwrap();
        assert_eq!(len, data.len() as u64);

        let h = BlobHttp;
        // 1. Registered hash -> 200 OK + immutable + ETag
        let req = Req::new("GET", &format!("/blob/{sha}"), vec![]);
        let rep = h.call(&hub, &req).await;
        assert_eq!(rep.status, "200 OK");
        assert_eq!(rep.ctype, "image/png");
        assert_eq!(rep.body, data);
        assert!(rep.extra.contains("immutable"), "{}", rep.extra);
        assert!(rep.extra.contains(&format!("ETag: \"{sha}\"")), "{}", rep.extra);

        // 2. If-None-Match -> 304 Not Modified (0 bytes)
        let req304 = Req::new(
            "GET",
            &format!("/blob/{sha}"),
            vec![("If-None-Match".into(), format!("\"{sha}\""))],
        );
        let rep304 = h.call(&hub, &req304).await;
        assert_eq!(rep304.status, "304 Not Modified");
        assert!(rep304.body.is_empty());

        // 3. Unknown or invalid hash -> 404
        for bad in ["/blob/not-a-hash", "/blob/../etc/passwd", &format!("/blob/{}", "0".repeat(64))] {
            let r = h.call(&hub, &Req::new("GET", bad, vec![])).await;
            assert_eq!(r.status, "404 Not Found", "{bad}");
        }
    }
}

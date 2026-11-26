//! Phase 2a: persistent hub-to-hub binary WebSocket (`/far/ws`, docs/remote-architecture.md §5.2, §9.8).
//!
//! Replaces per-call `curl` + `0600` temp file + `fsync` in `net/botnet.rs` and eliminates `viewer.pull`
//! base64 long-polling when a persistent socket is connected between two of the owner's machines, while
//! falling back transparently to HTTP `/far/*` for older hubs or `https://` peers without an inbound WS.
//!
//! Wire framing (WebSocket Binary messages, 1-byte tag):
//! - `0x00` + UTF-8 JSON control/RPC message (uncompressed, <= 256 B)
//! - `0x01` + raw-DEFLATE(UTF-8 JSON) control/RPC message (compressed, > 256 B)
//! - `0x02` + `sub: u64 (BE)` + raw binary bytes (viewer CBOR plot frame — zero base64!)
//! - `0x03` + `sub: u64 (BE)` + UTF-8 JSON bytes (viewer JSON frame)
//!
//! Control JSON messages:
//! - `{"k":"req","id":u64,"path":"/far/rpc"|"/far/push"|"/far/log","body":"..."}`
//! - `{"k":"rep","id":u64,"code":u16,"body":"..."}`
//! - `{"k":"live","thread":"<local_id>","text":"..."}` (0-RTT live streamed Bub / agent text)
//! - `{"k":"live_end","thread":"<local_id>"}`
use super::{botnet, far, farsync::FarSync, sign};
use crate::{auth::Req, hub::Hub};
use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::{mpsc, oneshot, watch, Semaphore},
};
use tokio_tungstenite::{
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
    WebSocketStream,
};

const TAG_JSON: u8 = 0x00;
const TAG_JSON_Z: u8 = 0x01;
const TAG_VRAW: u8 = 0x02;
const TAG_VJSON: u8 = 0x03;
const COMPRESS_MIN: usize = 256;
const MAX_DECODED: usize = 16 << 20; // 16 MiB cap against decompression bombs
const MAX_RPC_BODY: usize = 256 << 10;
const MAX_INFLIGHT_REQ: usize = 32;
const REQ_TIMEOUT: Duration = Duration::from_secs(20);
const IDLE_TIMEOUT: Duration = Duration::from_secs(75);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const RECONNECT_MIN: Duration = Duration::from_secs(2);
const RECONNECT_MAX: Duration = Duration::from_secs(30);

static NEXT_REQ_ID: AtomicU64 = AtomicU64::new(1);

/// Encode a JSON control frame (`0x00` raw, or `0x01` raw-DEFLATE when > 256 B).
pub fn encode_json(v: &Value) -> Vec<u8> {
    let raw = v.to_string();
    let bytes = raw.as_bytes();
    if bytes.len() > COMPRESS_MIN {
        let mut enc = DeflateEncoder::new(Vec::with_capacity(bytes.len() / 2 + 16), Compression::fast());
        if enc.write_all(bytes).is_ok() {
            if let Ok(z) = enc.finish() {
                if z.len() < bytes.len() {
                    let mut out = Vec::with_capacity(1 + z.len());
                    out.push(TAG_JSON_Z);
                    out.extend_from_slice(&z);
                    return out;
                }
            }
        }
    }
    let mut out = Vec::with_capacity(1 + bytes.len());
    out.push(TAG_JSON);
    out.extend_from_slice(bytes);
    out
}

/// Encode a raw binary viewer frame (`0x02` + sub BE + payload, zero base64).
pub fn encode_vraw(sub: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(9 + payload.len());
    out.push(TAG_VRAW);
    out.extend_from_slice(&sub.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// Encode a JSON viewer frame (`0x03` + sub BE + UTF-8 JSON).
pub fn encode_vjson(sub: u64, v: &Value) -> Vec<u8> {
    let raw = v.to_string();
    let mut out = Vec::with_capacity(9 + raw.len());
    out.push(TAG_VJSON);
    out.extend_from_slice(&sub.to_be_bytes());
    out.extend_from_slice(raw.as_bytes());
    out
}

#[derive(Debug)]
pub enum Frame {
    Json(Value),
    VRaw(u64, Vec<u8>),
    VJson(u64, Value),
}

/// Decode one binary frame from `/far/ws`.
pub fn decode_frame(data: &[u8]) -> Option<Frame> {
    let (&tag, rest) = data.split_first()?;
    match tag {
        TAG_JSON => serde_json::from_slice(rest).ok().map(Frame::Json),
        TAG_JSON_Z => {
            let mut dec = DeflateDecoder::new(rest);
            let mut out = Vec::new();
            let mut limited = (&mut dec).take((MAX_DECODED + 1) as u64);
            limited.read_to_end(&mut out).ok()?;
            if out.len() > MAX_DECODED {
                return None;
            }
            serde_json::from_slice(&out).ok().map(Frame::Json)
        }
        TAG_VRAW => {
            if rest.len() < 8 {
                return None;
            }
            let sub = u64::from_be_bytes(rest[..8].try_into().ok()?);
            Some(Frame::VRaw(sub, rest[8..].to_vec()))
        }
        TAG_VJSON => {
            if rest.len() < 8 {
                return None;
            }
            let sub = u64::from_be_bytes(rest[..8].try_into().ok()?);
            let v = serde_json::from_slice(&rest[8..]).ok()?;
            Some(Frame::VJson(sub, v))
        }
        _ => None,
    }
}

struct PeerHandle {
    tx: mpsc::Sender<Vec<u8>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<(u16, String)>>>>,
    shutdown_tx: watch::Sender<bool>,
}

#[derive(Default)]
struct Registry {
    syncs: HashMap<PathBuf, Arc<FarSync>>,
    peers: HashMap<(PathBuf, String), Arc<PeerHandle>>,
}

fn reg() -> &'static Mutex<Registry> {
    static R: OnceLock<Mutex<Registry>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Register a hub's `FarSync` and start background outbound `/far/ws` maintenance + live-delta streaming.
pub fn start(hub: &Arc<Hub>, sync: Arc<FarSync>) {
    reg().lock().unwrap().syncs.insert(hub.cfg.home.clone(), sync.clone());
    if crate::copy::on() {
        return;
    }
    // 1. Live streamed text forwarder: broadcasts local `delta` / `delta_end` frames to all connected owner peers in 0 RTT.
    {
        let hub_w = Arc::downgrade(hub);
        let home = hub.cfg.home.clone();
        let mut rx = hub.frames.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(v) => {
                        let t = v["t"].as_str().unwrap_or("");
                        if t != "delta" && t != "delta_end" {
                            continue;
                        }
                        let thread = v["thread"].as_str().unwrap_or("");
                        // Never re-broadcast mirrored far threads (prevents loops) or empty thread ids.
                        if thread.is_empty() || far::is(thread) {
                            continue;
                        }
                        if hub_w.upgrade().is_none() {
                            return;
                        }
                        let msg = if t == "delta" {
                            json!({"k":"live","thread":thread,"text":v["text"].as_str().unwrap_or("")})
                        } else {
                            json!({"k":"live_end","thread":thread})
                        };
                        let bytes = encode_json(&msg);
                        let handles: Vec<Arc<PeerHandle>> = reg()
                            .lock()
                            .unwrap()
                            .peers
                            .iter()
                            .filter(|((h, _), _)| h == &home)
                            .map(|(_, p)| p.clone())
                            .collect();
                        for p in handles {
                            let _ = p.tx.try_send(bytes.clone());
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                }
            }
        });
    }

    // 2. Outbound `/far/ws` connector for `http://` owner peers (Tailscale/LAN/loopback).
    {
        let hub_w = Arc::downgrade(hub);
        tokio::spawn(async move {
            let mut dialing: HashMap<String, std::time::Instant> = HashMap::new();
            loop {
                tokio::time::sleep(Duration::from_secs(3)).await;
                let Some(hub) = hub_w.upgrade() else { return };
                let links = super::rpc::machines_of(&sync);
                for (link, _, url) in links {
                    if !url.starts_with("http://") {
                        continue; // https:// peers stay on HTTP unless they connect inbound
                    }
                    if connected(&hub.cfg.home, &link) {
                        dialing.remove(&link);
                        continue;
                    }
                    if dialing.get(&link).is_some_and(|t| t.elapsed() < RECONNECT_MIN) {
                        continue;
                    }
                    dialing.insert(link.clone(), std::time::Instant::now());
                    let (h2, s2, l2, u2) = (hub.clone(), sync.clone(), link.clone(), url.clone());
                    tokio::spawn(async move {
                        let _ = connect_outbound(h2, s2, l2, u2).await;
                    });
                }
            }
        });
    }
}

/// Whether a live `/far/ws` socket is currently open for `(home, link)`.
pub fn connected(home: &Path, link: &str) -> bool {
    reg()
        .lock()
        .unwrap()
        .peers
        .contains_key(&(home.to_path_buf(), link.to_string()))
}

/// Send a signed-authenticated RPC (`/far/rpc`, `/far/push`, `/far/log`) over the persistent `/far/ws` socket if open.
/// Returns `None` when no socket is open so the caller falls back cleanly to HTTP `botnet::signed`.
pub async fn call(home: &Path, link: &str, path: &str, body: &str) -> Option<(u16, String)> {
    let handle = reg()
        .lock()
        .unwrap()
        .peers
        .get(&(home.to_path_buf(), link.to_string()))
        .cloned()?;
    let id = NEXT_REQ_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = oneshot::channel();
    handle.pending.lock().unwrap().insert(id, tx);
    let frame = encode_json(&json!({"k":"req","id":id,"path":path,"body":body}));
    if handle.tx.send(frame).await.is_err() {
        handle.pending.lock().unwrap().remove(&id);
        return None;
    }
    match tokio::time::timeout(REQ_TIMEOUT, rx).await {
        Ok(Ok(resp)) => Some(resp),
        _ => {
            handle.pending.lock().unwrap().remove(&id);
            None
        }
    }
}

/// Push a viewer frame directly to a connected peer over `/far/ws` (zero base64, zero long-poll).
pub fn push_viewer_raw(home: &Path, link: &str, sub: u64, payload: &[u8]) -> bool {
    let handle = reg()
        .lock()
        .unwrap()
        .peers
        .get(&(home.to_path_buf(), link.to_string()))
        .cloned();
    match handle {
        Some(h) => h.tx.try_send(encode_vraw(sub, payload)).is_ok(),
        None => false,
    }
}

pub fn push_viewer_json(home: &Path, link: &str, sub: u64, v: &Value) -> bool {
    let handle = reg()
        .lock()
        .unwrap()
        .peers
        .get(&(home.to_path_buf(), link.to_string()))
        .cloned();
    match handle {
        Some(h) => h.tx.try_send(encode_vjson(sub, v)).is_ok(),
        None => false,
    }
}

/// Accept an inbound `/far/ws` WebSocket upgrade on the hub's HTTP listener.
pub async fn accept(hub: Arc<Hub>, mut s: TcpStream, req: Req, head_len: usize) -> anyhow::Result<()> {
    let sync = reg().lock().unwrap().syncs.get(&hub.cfg.home).cloned();
    let Some(sync) = sync else {
        reject(&mut s, head_len, "503 Service Unavailable", "far sync not ready").await?;
        return Ok(());
    };
    if crate::copy::on() {
        reject(&mut s, head_len, "403 Forbidden", "a hub on a copied log does not take /far/ws").await?;
        return Ok(());
    }
    let preq = super::peerhttp::PeerRoutes::preq(&req);
    let link = match sync.peering.signed_in("far-ws", &preq) {
        Ok(p) => p,
        Err(reply) => {
            reject(&mut s, head_len, reply.status, &String::from_utf8_lossy(&reply.body)).await?;
            return Ok(());
        }
    };
    if !sync.own(&link) {
        reject(&mut s, head_len, "403 Forbidden", "only the owner's own machines may open /far/ws").await?;
        return Ok(());
    }
    let _ = s.set_nodelay(true);
    let ws = tokio_tungstenite::accept_async_with_config(s, Some(ws_config())).await?;
    run_session(hub, sync, link, ws).await;
    Ok(())
}

fn ws_config() -> tokio_tungstenite::tungstenite::protocol::WebSocketConfig {
    let mut cfg = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();
    // Allow 16 MiB decoded/raw payloads plus the 9-byte viewer frame header.
    cfg.max_message_size = Some(MAX_DECODED + 64);
    cfg.max_frame_size = Some(MAX_DECODED + 64);
    cfg
}

async fn reject(s: &mut TcpStream, head_len: usize, status: &str, body: &str) -> anyhow::Result<()> {
    use tokio::io::AsyncReadExt;
    let mut sink = vec![0u8; head_len];
    s.read_exact(&mut sink).await?;
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(resp.as_bytes()).await?;
    s.shutdown().await.ok();
    Ok(())
}

/// Establish an outbound `/far/ws` connection to an `http://` linked owner peer.
pub async fn connect_outbound(
    hub: Arc<Hub>,
    sync: Arc<FarSync>,
    link: String,
    url: String,
) -> anyhow::Result<()> {
    if crate::copy::on() || !url.starts_with("http://") {
        return Ok(());
    }
    let secret = botnet::peer_secret(&hub.cfg.home, &link);
    if secret.is_empty() {
        return Ok(());
    }
    let ws_url = format!("{}/far/ws", botnet::url_base(&url).replacen("http://", "ws://", 1));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let sig = sign::header(&secret, now, "");
    let req = {
        let mut req = ws_url.into_client_request()?;
        req.headers_mut()
            .insert("X-Backplane-Peer", HeaderValue::from_str(&link)?);
        req.headers_mut()
            .insert("X-Backplane-Timestamp", HeaderValue::from_str(&now.to_string())?);
        req.headers_mut()
            .insert("X-Backplane-Signature", HeaderValue::from_str(&sig)?);
        req
    };
    let host = req
        .uri()
        .host()
        .ok_or_else(|| anyhow::anyhow!("peerws: missing host in {url}"))?
        .trim_start_matches('[')
        .trim_end_matches(']');
    let port = req.uri().port_u16().unwrap_or(80);
    let tcp = tokio::time::timeout(Duration::from_secs(10), TcpStream::connect((host, port))).await??;
    let _ = tcp.set_nodelay(true);
    let (ws, _) = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::client_async_with_config(req, tcp, Some(ws_config())),
    )
    .await??;
    if connected(&hub.cfg.home, &link) {
        return Ok(());
    }
    run_session(hub, sync, link, ws).await;
    Ok(())
}

async fn run_session<S>(hub: Arc<Hub>, sync: Arc<FarSync>, link: String, ws: WebSocketStream<S>)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(256);
    let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<(u16, String)>>>> = Default::default();
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let req_sem = Arc::new(Semaphore::new(MAX_INFLIGHT_REQ));
    let writer_shutdown = shutdown_tx.clone();
    let handle = Arc::new(PeerHandle {
        tx: tx.clone(),
        pending: pending.clone(),
        shutdown_tx,
    });
    let key = (hub.cfg.home.clone(), link.clone());
    if let Some(prev) = reg().lock().unwrap().peers.insert(key.clone(), handle.clone()) {
        let _ = prev.shutdown_tx.send(true);
    }
    hub.mirrors().set_away(&link, false);
    hub.mirrors().publish(&hub).await;

    // Immediately announce our version, commit, and supported peer features on the handshake.
    let _ = tx.try_send(encode_json(&json!({
        "k": "hello",
        "version": hub.cfg.version,
        "commit": hub.cfg.commit,
        "features": super::peerver::PEER_FEATURES,
    })));

    let writer = tokio::spawn(async move {
        let mut ping = tokio::time::interval(Duration::from_secs(25));
        ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                msg = rx.recv() => match msg {
                    Some(b) => {
                        if !matches!(tokio::time::timeout(WRITE_TIMEOUT, sink.send(Message::Binary(b))).await, Ok(Ok(()))) { break }
                    }
                    None => break,
                },
                _ = ping.tick() => {
                    if !matches!(tokio::time::timeout(WRITE_TIMEOUT, sink.send(Message::Ping(Vec::new()))).await, Ok(Ok(()))) { break }
                }
            }
        }
        let _ = writer_shutdown.send(true);
    });

    let mut last_rx = tokio::time::Instant::now();
    loop {
        let msg = tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() { break }
                continue
            }
            _ = tokio::time::sleep_until(last_rx + IDLE_TIMEOUT) => {
                break
            }
            next = stream.next() => match next {
                Some(Ok(m)) => {
                    last_rx = tokio::time::Instant::now();
                    m
                }
                Some(Err(_)) | None => break,
            }
        };
        match msg {
            Message::Binary(data) => {
                let Some(frame) = decode_frame(&data) else { continue };
                match frame {
                    Frame::Json(v) => match v["k"].as_str().unwrap_or("") {
                        "hello" => {
                            super::peerver::record_json(&hub.cfg.home, &link, &v);
                            hub.bump_views();
                        }
                        "req" => {
                            let id = v["id"].as_u64().unwrap_or(0);
                            let path = v["path"].as_str().unwrap_or("").to_string();
                            let body = v["body"].as_str().unwrap_or("").to_string();
                            let Ok(permit) = req_sem.clone().try_acquire_owned() else {
                                let rep = encode_json(&json!({"k":"rep","id":id,"code":503,"body":"peer busy"}));
                                let _ = tx.try_send(rep);
                                continue;
                            };
                            let (h2, s2, l2, tx2) = (hub.clone(), sync.clone(), link.clone(), tx.clone());
                            tokio::spawn(async move {
                                let _permit = permit;
                                let (code, resp_body) = dispatch_req(&s2, &h2, &l2, &path, body.as_bytes()).await;
                                let rep = encode_json(&json!({"k":"rep","id":id,"code":code,"body":resp_body}));
                                let _ = tx2.send(rep).await;
                            });
                        }
                        "rep" => {
                            let id = v["id"].as_u64().unwrap_or(0);
                            let code = v["code"].as_u64().unwrap_or(0) as u16;
                            let body = v["body"].as_str().unwrap_or("").to_string();
                            if let Some(waiter) = pending.lock().unwrap().remove(&id) {
                                let _ = waiter.send((code, body));
                            }
                        }
                        "live" => {
                            let th = v["thread"].as_str().unwrap_or("");
                            let txt = v["text"].as_str().unwrap_or("");
                            if !th.is_empty() && !txt.is_empty() && sync.own(&link) {
                                let _ = hub.live_delta(&far::id(&link, th), txt).await;
                            }
                        }
                        "live_end" => {
                            let th = v["thread"].as_str().unwrap_or("");
                            if !th.is_empty() && sync.own(&link) {
                                let _ = hub.live_end(&far::id(&link, th)).await;
                            }
                        }
                        _ => {}
                    },
                    Frame::VRaw(sub, bytes) => {
                        if sync.own(&link) {
                            hub.push_raw(sub, bytes);
                        }
                    }
                    Frame::VJson(sub, mut j) => {
                        if sync.own(&link) {
                            if j["t"] == "term" {
                                if let Some(t) = j["thread"].as_str().filter(|t| !far::is(t)).map(|t| far::id(&link, t)) {
                                    j["thread"] = Value::String(t);
                                }
                            }
                            hub.push(sub, j);
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    writer.abort();
    // Only remove our own handle (a newer reconnect may have already replaced it).
    {
        let mut r = reg().lock().unwrap();
        if r.peers.get(&key).is_some_and(|p| Arc::ptr_eq(p, &handle)) {
            r.peers.remove(&key);
        }
    }
    // Fail any in-flight callers immediately so they can retry/fallback cleanly.
    pending.lock().unwrap().clear();
    let _ = RECONNECT_MAX;
}

async fn dispatch_req(
    sync: &Arc<FarSync>,
    hub: &Arc<Hub>,
    link: &str,
    path: &str,
    body: &[u8],
) -> (u16, String) {
    if body.len() > MAX_RPC_BODY {
        return (413, "at most 256 KB".to_string());
    }
    let reply = match path {
        "/far/rpc" => super::rpc::serve_authenticated(crate::copy::on(), sync, hub, link, body).await,
        "/far/push" => sync.push_authenticated(hub, link, body).await,
        "/far/log" => sync.log_authenticated(hub, link, body).await,
        _ => crate::api::HttpReply::text("404 Not Found", "not found"),
    };
    let code = reply
        .status
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(500);
    (code, String::from_utf8_lossy(&reply.body).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_frames_round_trip_small_compressed_and_viewer_raw() {
        let small = json!({"k":"live","thread":"t1","text":"hi"});
        let enc_s = encode_json(&small);
        assert_eq!(enc_s[0], TAG_JSON);
        match decode_frame(&enc_s).unwrap() {
            Frame::Json(v) => assert_eq!(v, small),
            other => panic!("expected json, got {other:?}"),
        }

        let big = json!({"k":"rep","id":7,"code":200,"body":"hello world ".repeat(80)});
        let enc_b = encode_json(&big);
        assert_eq!(enc_b[0], TAG_JSON_Z);
        assert!(enc_b.len() < big.to_string().len() / 4, "compressed well: {}", enc_b.len());
        match decode_frame(&enc_b).unwrap() {
            Frame::Json(v) => assert_eq!(v, big),
            other => panic!("expected json, got {other:?}"),
        }

        let payload = vec![0xde, 0xad, 0xbe, 0xef, 1, 2, 3];
        let enc_r = encode_vraw(42, &payload);
        assert_eq!(enc_r[0], TAG_VRAW);
        match decode_frame(&enc_r).unwrap() {
            Frame::VRaw(sub, b) => {
                assert_eq!(sub, 42);
                assert_eq!(b, payload);
            }
            other => panic!("expected vraw, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn two_hubs_exchange_rpc_and_live_deltas_over_persistent_far_ws() {
        use crate::net::{
            farstore::FarStore,
            peering::tests::{know, link_by_code, node_extra},
        };

        let a = node_extra("peerws-a", false, false, None).await;
        let b = node_extra("peerws-b", false, false, None).await;
        know(&a, &b);
        know(&b, &a);
        let link_id = link_by_code(&b, &a).await;

        let sa = FarSync::new(FarStore::open(&a.home).unwrap(), a.p.clone());
        let sb = FarSync::new(FarStore::open(&b.home).unwrap(), b.p.clone());
        start(&a.hub, sa.clone());
        start(&b.hub, sb.clone());

        // Serve Hub B's wire listener (supports /far/ws upgrade)
        let lb = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port_b = lb.local_addr().unwrap().port();
        let url_b = format!("http://127.0.0.1:{port_b}");
        let hb_srv = b.hub.clone();
        let srv = tokio::spawn(async move {
            loop {
                let Ok((s, _)) = lb.accept().await else { break };
                let h = hb_srv.clone();
                tokio::spawn(async move { let _ = crate::wire::serve(h, s).await; });
            }
        });

        // Connect Hub A -> Hub B over /far/ws
        let ha_c = a.hub.clone();
        let sa_c = sa.clone();
        let l_c = link_id.clone();
        let u_c = url_b.clone();
        let conn = tokio::spawn(async move {
            let _ = connect_outbound(ha_c, sa_c, l_c, u_c).await;
        });

        for _ in 0..50 {
            if connected(&a.home, &link_id) && connected(&b.home, &link_id) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
        assert!(connected(&a.home, &link_id) && connected(&b.home, &link_id), "both hubs hold persistent /far/ws");

        // 1. RPC over /far/ws (zero curl!): A calls home.state on B
        let (code, body) = call(&a.home, &link_id, "/far/rpc", &json!({"id":0,"m":"home.state","p":{}}).to_string())
            .await
            .expect("call went over /far/ws");
        assert_eq!(code, 200, "{body}");
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["ok"], true, "{v}");

        // 2. Live streamed delta from B arrives on A as `<link>~t1` in 0 RTT!
        let mut rx_a = a.hub.frames.subscribe();
        b.hub.live_delta("t1", "hello from B").await.unwrap();
        let mut got_delta = false;
        for _ in 0..20 {
            if let Ok(Ok(f)) = tokio::time::timeout(Duration::from_millis(200), rx_a.recv()).await {
                if f["t"] == "delta" && f["thread"] == format!("{link_id}~t1") && f["text"] == "hello from B" {
                    got_delta = true;
                    break;
                }
            }
        }
        assert!(got_delta, "A received live streamed delta from B over /far/ws");

        // 3. Oversized /far/rpc payload (> 256 KB) is rejected with 413 without breaking the socket.
        let huge = "x".repeat(MAX_RPC_BODY + 16);
        let (code413, _) = call(&a.home, &link_id, "/far/rpc", &huge)
            .await
            .expect("oversized call answered 413");
        assert_eq!(code413, 413);

        conn.abort();
        srv.abort();
    }
}

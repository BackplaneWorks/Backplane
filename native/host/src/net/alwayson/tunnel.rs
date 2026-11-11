//! The reverse link: a hub that its linked machine cannot reach (the desktop, on its own loopback or a tailnet the Boat is not on) keeps an
//! outbound, signed connection to that machine, and that machine sends its calls to the hub back over it: far calls, far-sends, pushes, the
//! registry, everything `botnet::signed` carries. A call waits at most for the next poll, so it arrives within moments, not at the 5-minute pull.
//!
//! The connection is a held HTTPS request rather than a WebSocket: this hub's WebSocket library is built without TLS, and the always-on
//! machine's public address is HTTPS. It uses the same signed requests as every other far call (`botnet::to_peer`, the link's secret):
//! - the hub POSTs `/far/tunnel {wait: true, url, replies: []}`; the other end holds it up to `HOLD` seconds and answers at once with any
//!   calls it has queued for this link: `{requests: [{id, method, path, headers, body}], reach}`;
//! - each call runs here against an untrusted loopback listener of this hub's own (no loopback trust: its signature is checked by the route
//!   as if it had come over the network), and its answer goes back at once as `/far/tunnel {wait: false, replies: [{id, code, body}]}`;
//! - `reach`: every 5 minutes the other end tries this hub's own address. When it can reach it, this hub stops polling it for 5 minutes and
//!   the calls go directly as before.
//!
//! Only a verified link may poll (an approved pairing, the owner's own machine), and a call is answered only to the link it was queued for.
use crate::{api::{Fut, HttpHandler, HttpReply}, auth::Req, hub::Hub};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};
use std::{collections::{HashMap, VecDeque}, path::PathBuf, sync::{Arc, Mutex}, time::{Duration, Instant}};
use tokio::sync::{oneshot, Notify};

/// How long a poll is held (under curl's 15 s in `botnet::args`).
pub const HOLD: Duration = Duration::from_secs(10);
/// A link whose last poll is older than this is not taken to be listening.
const LIVE: Duration = Duration::from_secs(30);
/// How long a call waits for its answer.
const ANSWER: Duration = Duration::from_secs(30);
const REACH_EVERY: Duration = Duration::from_secs(300);
/// How long a queued call waits to be taken by a poll before it goes directly: a polling link holds a poll nearly all the time (it polls again
/// at once), so a call is taken at once; one not taken in this long means that link stopped polling.
const TAKEN: Duration = Duration::from_secs(3);

#[derive(Default)]
struct Link { polled: Option<Instant>, queue: VecDeque<Value>, waiting: HashMap<u64, oneshot::Sender<(u16, String)>>, wake: Arc<Notify>, reach: Option<(Instant, bool)>, probing: bool }

/// The always-on end: the links that poll it, their queued calls and the callers waiting for answers.
#[derive(Default)]
pub struct Ends { links: Mutex<HashMap<String, Link>>, next: std::sync::atomic::AtomicU64 }
static ENDS: std::sync::LazyLock<Ends> = std::sync::LazyLock::new(Default::default);
pub fn ends() -> &'static Ends { &ENDS }

impl Ends {
    /// A link is polling now.
    pub fn live(&self, peer: &str) -> bool { self.links.lock().unwrap().get(peer).and_then(|l| l.polled).map_or(false, |t| t.elapsed() < LIVE) }
    /// Send a call to `peer` over its poll: None when it is not polling (the caller goes directly); else its answer (code 0: no answer in time).
    pub async fn call(&self, peer: &str, method: &str, path: &str, headers: &str, body: &str) -> Option<(u16, String)> {
        if !self.live(peer) { return None }
        let id = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let (tx, rx) = oneshot::channel();
        let hs: Vec<Value> = headers.lines().filter_map(|l| l.split_once(": ")).map(|(k, v)| json!([k, v])).collect();
        {
            let mut g = self.links.lock().unwrap();
            let l = g.entry(peer.to_string()).or_default();
            l.waiting.insert(id, tx);
            l.queue.push_back(json!({"id": id, "method": method, "path": path, "headers": hs, "body": B64.encode(body)}));
            l.wake.notify_one();
        }
        // taken by a poll within one hold (and a little): else that link stopped polling (it reaches this hub directly again, or it restarted)
        // and the call goes directly; it never waits ANSWER in a queue nobody reads
        let taken_by = tokio::time::Instant::now() + TAKEN;
        let mut rx = rx;
        loop {
            let queued = self.links.lock().unwrap().get(peer).map_or(false, |l| l.queue.iter().any(|q| q["id"] == id));
            if !queued { break }
            if tokio::time::Instant::now() >= taken_by {
                if let Some(l) = self.links.lock().unwrap().get_mut(peer) { l.waiting.remove(&id); l.queue.retain(|q| q["id"] != id); l.polled = None; }
                eprintln!("tunnel: {peer} stopped polling: the call goes directly");
                return None
            }
            if let Ok(r) = tokio::time::timeout(Duration::from_millis(200), &mut rx).await { return Some(r.unwrap_or((0, String::new()))) }
        }
        let r = tokio::time::timeout(ANSWER, rx).await;
        if let Some(l) = self.links.lock().unwrap().get_mut(peer) { l.waiting.remove(&id); l.queue.retain(|q| q["id"] != id); }
        Some(r.ok().and_then(Result::ok).unwrap_or((0, String::new())))
    }
    /// A poll from `peer` (already verified): its answers delivered, then (when it waits) the calls queued for it, held up to `HOLD` for one.
    pub async fn poll(&self, home: &std::path::Path, peer: &str, p: &Value) -> Value {
        let wake = {
            let mut g = self.links.lock().unwrap();
            let l = g.entry(peer.to_string()).or_default();
            for r in p["replies"].as_array().into_iter().flatten() {
                let Some(id) = r["id"].as_u64() else { continue };
                if let Some(tx) = l.waiting.remove(&id) { let body = B64.decode(r["body"].as_str().unwrap_or("")).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default(); let _ = tx.send((r["code"].as_u64().unwrap_or(0) as u16, body)); }
            }
            if p["wait"] != true { return json!({"requests": []}) }
            l.polled = Some(Instant::now());
            l.wake.clone()
        };
        let reach = self.reach(home, peer, p["url"].as_str().unwrap_or(""));
        let end = Instant::now() + HOLD;
        loop {
            let got: Vec<Value> = { let mut g = self.links.lock().unwrap(); let l = g.entry(peer.to_string()).or_default(); l.polled = Some(Instant::now()); let n = l.queue.len().min(16); l.queue.drain(..n).collect() };
            if !got.is_empty() || Instant::now() >= end { return json!({"requests": got, "reach": reach}) }
            let _ = tokio::time::timeout(end.saturating_duration_since(Instant::now()), wake.notified()).await;
        }
    }
    /// Whether this end can reach `peer` at `url` directly: known from a probe at most 5 minutes old, else a probe is started (and None said now).
    fn reach(&self, home: &std::path::Path, peer: &str, url: &str) -> Option<bool> {
        let mut g = self.links.lock().unwrap();
        let l = g.entry(peer.to_string()).or_default();
        if let Some((at, ok)) = l.reach { if at.elapsed() < REACH_EVERY { return Some(ok) } }
        if l.probing || !crate::net::botnet::url_ok(url) || crate::net::machines::loopback(url) || crate::net::peering::over_link(url) { return l.reach.map(|r| r.1) }
        l.probing = true;
        let (home, peer, url) = (home.to_path_buf(), peer.to_string(), url.to_string());
        tokio::spawn(async move {
            let r = super::boat::send(&home, super::boat::Call::new("GET", format!("{}/healthz", url.trim_end_matches('/')), "").secs(5)).await;
            let ok = matches!(r.code.as_str(), "200" | "503");
            let mut g = ENDS.links.lock().unwrap(); let l = g.entry(peer).or_default(); l.reach = Some((Instant::now(), ok)); l.probing = false;
        });
        None
    }
}

/// `POST /far/tunnel`: a verified link's signature, then `Ends::poll`. `secret_of`: a verified link's secret, None for any other.
pub struct Route { pub secret_of: Arc<dyn Fn(&str) -> Option<String> + Send + Sync>, pub home: PathBuf }
impl HttpHandler for Route {
    fn routes(&self) -> &'static [&'static str] { &["/far/tunnel"] }
    fn own_auth(&self) -> bool { true }
    fn body_max(&self) -> usize { 2 << 20 }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            let reply = |status: &'static str, v: Value| HttpReply { status, ctype: "application/json".into(), extra: "Cache-Control: no-store\r\n".into(), body: v.to_string().into_bytes() };
            if req.method != "POST" { return reply("405 Method Not Allowed", json!({"error": "POST only"})) }
            let (peer, ts, sig) = (req.header("x-backplane-peer").to_string(), req.header("x-backplane-timestamp").parse::<u64>().unwrap_or(0), req.header("x-backplane-signature").to_string());
            let body = String::from_utf8_lossy(&req.body).into_owned();
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let Some(secret) = (self.secret_of)(&peer).filter(|s| !s.is_empty()) else { return reply("401 Unauthorized", json!({"error": "not a linked machine"})) };
            if !crate::net::sign::check(&secret, now, ts, &body, &sig) { return reply("401 Unauthorized", json!({"error": "bad signature"})) }
            let p: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            reply("200 OK", ends().poll(&self.home, &peer, &p).await)
        })
    }
}

// ----- the polling end -----

/// The links this hub polls: `(id, url, secret)` of each verified link.
pub type Links = Arc<dyn Fn() -> Vec<(String, String, String)> + Send + Sync>;

/// An untrusted loopback listener of this hub's own, for the calls that came over a poll: they meet the same checks as over the network.
async fn local_door(hub: &Arc<Hub>) -> Option<u16> {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.ok()?;
    let port = l.local_addr().ok()?.port();
    let hub = hub.clone();
    tokio::spawn(async move { loop { let Ok((s, _)) = l.accept().await else { tokio::time::sleep(Duration::from_millis(50)).await; continue }; let h = hub.clone(); tokio::spawn(async move { let _ = crate::wire::serve_with(h, s, false).await; }); } });
    Some(port)
}
/// One call run against the local door: its status and body.
pub async fn run_local(port: u16, q: &Value) -> (u16, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let body = B64.decode(q["body"].as_str().unwrap_or("")).unwrap_or_default();
    let method = match q["method"].as_str().unwrap_or("") { "GET" => "GET", _ => "POST" };
    let path = q["path"].as_str().unwrap_or("/");
    if !path.starts_with('/') || path.contains(['\r', '\n', ' ']) { return (400, String::new()) }
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    for h in q["headers"].as_array().into_iter().flatten() {
        let (k, v) = (h[0].as_str().unwrap_or(""), h[1].as_str().unwrap_or(""));
        // only what a signed far call carries: its signature and its type; nothing that could ask for trust
        if matches!(k.to_ascii_lowercase().as_str(), "x-backplane-peer" | "x-backplane-timestamp" | "x-backplane-signature" | "content-type") && !v.contains(['\r', '\n']) { head.push_str(&format!("{k}: {v}\r\n")) }
    }
    head.push_str("\r\n");
    let Ok(Ok(mut s)) = tokio::time::timeout(Duration::from_secs(5), tokio::net::TcpStream::connect(("127.0.0.1", port))).await else { return (0, String::new()) };
    if s.write_all(head.as_bytes()).await.is_err() || s.write_all(&body).await.is_err() { return (0, String::new()) }
    let mut out = vec![];
    if tokio::time::timeout(ANSWER, s.read_to_end(&mut out)).await.is_err() { return (0, String::new()) }
    let t = String::from_utf8_lossy(&out);
    let code = t.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    (code, t.split_once("\r\n\r\n").map(|x| x.1.to_string()).unwrap_or_default())
}

/// Poll every link (each its own loop) for as long as it is linked; one that can reach this hub is left alone for 5 minutes, one that
/// does not know the route (an older build) for 10, one that does not answer is tried again after a growing wait.
pub fn start(hub: &Arc<Hub>, links: Links, here: Arc<dyn Fn() -> Fut<'static, String> + Send + Sync>) {
    if crate::copy::on() { return }
    let hub = hub.clone();
    tokio::spawn(async move {
        let Some(port) = local_door(&hub).await else { eprintln!("backplane-hub: the reverse link has no local door"); return };
        let running: Arc<Mutex<std::collections::HashSet<String>>> = Default::default();
        loop {
            for (id, url, secret) in links() {
                if !running.lock().unwrap().insert(id.clone()) { continue }
                let (hub, links, here, running) = (hub.clone(), links.clone(), here.clone(), running.clone());
                tokio::spawn(async move {
                    let mut back = 5u64;
                    loop {
                        // still linked (a removed link ends its loop)
                        let Some((_, url, secret)) = links().into_iter().find(|l| l.0 == id) else { break };
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                        // this hub's own address, for the far end's reach probe; a loopback one would be the far end's own loopback (its own hub
                        // answering there), so it is never sent: a hub nobody can reach keeps polling
                        let mine = here().await;
                        let body = json!({"wait": true, "url": if crate::net::machines::loopback(&mine) { String::new() } else { mine }, "replies": []}).to_string();
                        let r = crate::net::botnet::signed(&hub.cfg.home, &url, &id, &secret, now, "/far/tunnel", true, &body).await;
                        match r.code {
                            200 => back = 5,
                            404 => { tokio::time::sleep(Duration::from_secs(600)).await; continue }
                            _ => { tokio::time::sleep(Duration::from_secs(back)).await; back = (back * 2).min(300); continue }
                        }
                        let j: Value = serde_json::from_str(&r.body).unwrap_or(Value::Null);
                        for q in j["requests"].as_array().cloned().unwrap_or_default() {
                            let (hub, url, id, secret) = (hub.clone(), url.clone(), id.clone(), secret.clone());
                            tokio::spawn(async move {
                                let (code, out) = run_local(port, &q).await;
                                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                                let body = json!({"wait": false, "replies": [{"id": q["id"], "code": code, "body": B64.encode(out)}]}).to_string();
                                let _ = crate::net::botnet::signed(&hub.cfg.home, &url, &id, &secret, now, "/far/tunnel", true, &body).await;
                            });
                        }
                        if j["reach"] == true { tokio::time::sleep(REACH_EVERY).await }
                    }
                    running.lock().unwrap().remove(&id);
                });
                let _ = (url, secret);
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}

/// The hub's own wiring: the route for links that poll it, and its own polls to every verified link.
pub fn install(hub: &Arc<Hub>, peering: Arc<crate::net::peering::Peering>) {
    let home = hub.cfg.home.clone();
    let (p1, h1) = (peering.clone(), home.clone());
    hub.register_http(Arc::new(Route { secret_of: Arc::new(move |peer: &str| {
        let ok = p1.table.with(|t| t.peer(peer).map_or(false, |r| !r.revoked) && t.verified(peer));
        ok.then(|| crate::net::botnet::peer_secret(&h1, peer))
    }), home: home.clone() }));
    let (p2, h2) = (peering.clone(), home.clone());
    let links: Links = Arc::new(move || p2.table.with(|t| t.live().into_iter().filter(|x| t.verified(&x.id)).map(|x| (x.id.clone(), x.url.clone(), crate::net::botnet::peer_secret(&h2, &x.id))).filter(|l| !l.2.is_empty() && crate::net::botnet::url_ok(&l.1) && !crate::net::peering::over_link(&l.1)).collect()));
    let p3 = peering;
    let here: Arc<dyn Fn() -> Fut<'static, String> + Send + Sync> = Arc::new(move || { let p = p3.clone(); Box::pin(async move { crate::net::botnet::url_here(p.tailnet, &p.host, p.port).await }) });
    start(hub, links, here);
}

#[cfg(test)]
mod taken_tests {
    use super::*;
    /// A link that polled a moment ago and stopped (it restarted, or reaches this hub directly again): a call is not left in its queue for ANSWER
    /// (30 s), it goes directly within TAKEN; one a poll takes is answered over the poll.
    #[tokio::test] async fn a_call_no_poll_takes_goes_directly_within_moments() {
        let e = Ends::default();
        e.links.lock().unwrap().entry("p".into()).or_default().polled = Some(Instant::now());
        let t = Instant::now();
        assert!(e.call("p", "POST", "/far/rpc", "", "{}").await.is_none(), "not taken: the caller goes directly");
        assert!(t.elapsed() < TAKEN + Duration::from_secs(1), "{:?}", t.elapsed());
        assert!(!e.live("p"), "and the link is no longer taken to be polling");
    }
}

//! Two throwaway hubs, each behind a tiny HTTP front on loopback that hands requests to `Peering::handle`
//! (the real wire layer needs the Req body/trusted seam; this front stands in for it). Curl really connects. No real
//! peers, no tailnet, no pushes.
use super::*;
use crate::{hub::{Cfg, Hub}, presenter::Canned};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};

pub(crate) struct Node { pub(crate) hub: Arc<Hub>, pub(crate) p: Arc<Peering>, pub(crate) port: u16, pub(crate) home: PathBuf }
/// A handler tried before `Peering::handle` (a test's stand-in for the other routes).
pub(crate) type Extra = Arc<dyn Fn(Arc<Hub>, PReq) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<HttpReply>> + Send>> + Send + Sync>;
fn home(tag: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-peering-{tag}-{}-{}", std::process::id(), hex::encode(super::super::pairing::urandom(3)))); std::fs::create_dir_all(&p).unwrap(); p }

/// A node: a hub on a fresh home, its token, registry, peer table, and a front that serves `Peering::handle`.
/// `trusted`: the front tags every socket as the owner's own device (what the tailnet listener would do after whois).
pub(crate) async fn node(tag: &str, trusted: bool) -> Node {
    node_with(tag, trusted, trusted).await
}
pub(crate) async fn node_with(tag: &str, trusted: bool, own_device: bool) -> Node { node_extra(tag, trusted, own_device, None).await }
/// `trusted`: loopback-style trust; `own_device`: the tailnet listener's whois verdict.
/// A test that needs its hub on a fixed port (the JS-peer test, ports 3915-3919) sets this before `node_extra`; 0 is any port.
pub(crate) static FIXED_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);
pub(crate) async fn node_extra(tag: &str, trusted: bool, own_device: bool, extra: Option<Extra>) -> Node {
    let home = home(tag);
    std::fs::write(home.join("token"), format!("token-{tag}\n")).unwrap();
    let hub = Hub::start(Cfg { home: home.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() },
        Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let l = TcpListener::bind(("127.0.0.1", FIXED_PORT.load(std::sync::atomic::Ordering::SeqCst))).await.unwrap(); let port = l.local_addr().unwrap().port();
    let (reg, routes) = super::super::start::open(&home, "127.0.0.1", port).unwrap();
    let table = PeerTable::new(); table.start(&hub).await;
    let p = Arc::new(Peering::new(home.clone(), reg, table, routes, false, "127.0.0.1", port));
    p.set_own_hosts("127.0.0.1", "");
    let (p2, h2, ex) = (p.clone(), hub.clone(), extra);
    tokio::spawn(async move { loop {
        let Ok((mut s, _)) = l.accept().await else { return };
        let (p, h, ex) = (p2.clone(), h2.clone(), ex.clone());
        tokio::spawn(async move {
            let mut buf = vec![]; let mut chunk = [0u8; 8192];
            let (head_end, len) = loop {
                let n = s.read(&mut chunk).await.unwrap_or(0); if n == 0 { return } buf.extend_from_slice(&chunk[..n]);
                if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..at]).to_string();
                    let len = head.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                    break (at + 4, len);
                }
            };
            while buf.len() < head_end + len { let n = s.read(&mut chunk).await.unwrap_or(0); if n == 0 { break } buf.extend_from_slice(&chunk[..n]); }
            let head = String::from_utf8_lossy(&buf[..head_end - 4]).to_string(); let mut lines = head.lines();
            let first: Vec<&str> = lines.next().unwrap_or("").split(' ').collect();
            let headers = lines.filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))).collect();
            let req = PReq { method: first[0].into(), target: first.get(1).unwrap_or(&"/").to_string(), headers, body: buf[head_end..].to_vec(), trusted, own_device };
            let via = match &ex { Some(f) => f(h.clone(), PReq { method: req.method.clone(), target: req.target.clone(), headers: req.headers.clone(), body: req.body.clone(), trusted: req.trusted, own_device: req.own_device }).await, None => None };
            let rep = match via { Some(r) => r, None => match p.handle(&h, &req).await { Some(r) => r, None => HttpReply::text("404 Not Found", "not found") } };
            let _ = s.write_all(format!("HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n", rep.status, rep.ctype, rep.body.len(), rep.extra).as_bytes()).await;
            let _ = s.write_all(&rep.body).await;
        });
    } });
    Node { hub, p, port, home }
}
pub(crate) fn url(n: &Node) -> String { format!("http://127.0.0.1:{}", n.port) }
/// `a` knows `b` as one of the owner's machines (named, so a no-tailnet hub pairs with it).
pub(crate) fn know(a: &Node, b: &Node) { a.p.routes.machines(json!({"self":{"name":"a","url":url(a),"domain":""},"peers":[{"name":"127.0.0.1","url":url(b),"version":"1","named":true}]}), vec![]); }
fn peerset(n: &Node, id: &str) -> Option<crate::net::peers::PeerRow> { n.p.table.with(|p| p.peer(id).cloned()) }
pub(crate) fn live_ids(n: &Node) -> Vec<String> { n.p.table.with(|p| p.live().into_iter().map(|x| x.id).collect()) }
/// `b` pairs with `a` the real way (a code, a request, the person's approval on `a`); the link's id.
pub(crate) async fn link_by_code(a: &Node, b: &Node) -> String {
    let code = a.p.make_code().await.unwrap()["code"].as_str().unwrap().to_string();
    let (attempt, _) = b.p.pair_with(&b.hub, &url(a), &code).await.unwrap();
    for _ in 0..100 { if let Some(r) = a.p.page_data()["requests"].as_array().and_then(|r| r.first().cloned()) { a.p.approve(&a.hub, r["id"].as_str().unwrap()).await.unwrap(); break } tokio::time::sleep(std::time::Duration::from_millis(30)).await }
    for _ in 0..100 { if b.p.attempt(&attempt)["state"] == "linked" { break } tokio::time::sleep(std::time::Duration::from_millis(30)).await }
    settle().await;
    live_ids(a).into_iter().next().expect("linked")
}
pub(crate) async fn settle() { for _ in 0..50 { tokio::time::sleep(std::time::Duration::from_millis(20)).await; } }

#[tokio::test] async fn pair_link_is_signed_fresh_and_once() {
    let (a, b) = (node("a7", true).await, node("b7", false).await);
    know(&a, &b); know(&b, &a);
    // a hand-made invite from a (what the invite route does), used by b
    let (inv, _) = a.p.invite_for("").await.unwrap();
    let i = sign::invite_parse(&inv).unwrap();
    // an unsigned or wrongly signed /bots/link is refused
    let r = botnet::plain(&format!("{}/bots/link", url(&a))).await; assert_eq!(r.code, 401);
    let now = (a.p.now)();
    let bad = botnet::signed(&b.home, &url(&a), &i.id, &"00".repeat(32), now, "/bots/link", true, "{\"url\":\"http://x:1\"}").await;
    assert_eq!(bad.code, 401, "{}", bad.body);
    // the right secret links; the same signature again is a replay; a second link is refused
    let ok = b.p.join(&b.hub, &i, "").await; assert!(ok.is_ok(), "{ok:?}");
    let r = botnet::signed(&b.home, &url(&a), &i.id, &i.secret, now, "/bots/link", true, "{\"url\":\"http://127.0.0.1:1\",\"name\":\"z\"}").await;
    assert_eq!(r.code, 409, "{}", r.body);
    assert_eq!(b.p.join(&b.hub, &i, "").await.unwrap_err(), "already linked to that machine");
}
#[tokio::test] async fn pair_signed_in_refuses_replays_stale_and_oversize() {
    let a = node("a8", false).await;
    let secret = botnet::fresh_secret(); assert!(botnet::peer_save(&a.home, "p1", &secret));
    let now = (a.p.now)();
    let req = |ts: u64, sig: Option<String>, body: &str| PReq { method: "POST".into(), target: "/x".into(), trusted: false, own_device: false, body: body.as_bytes().to_vec(),
        headers: vec![("X-Backplane-Peer".into(), "p1".into()), ("X-Backplane-Timestamp".into(), ts.to_string()), ("X-Backplane-Signature".into(), sig.unwrap_or_else(|| sign::header(&secret, ts, body)))] };
    assert_eq!(a.p.signed_in("deliver", &req(now, None, "{}")).ok().unwrap(), "p1");
    assert_eq!(a.p.signed_in("deliver", &req(now, None, "{}")).unwrap_err().body, b"already seen");
    assert_eq!(a.p.signed_in("deliver", &req(now - 400, None, "{}")).unwrap_err().body, b"bad or stale signature");
    assert_eq!(a.p.signed_in("deliver", &req(now, Some("sha256=00".into()), "{}")).unwrap_err().status, "401 Unauthorized");
    assert_eq!(a.p.signed_in("deliver", &req(now, None, &"x".repeat(262145))).unwrap_err().status, "413 Payload Too Large");
    let mut unknown = req(now, None, "{}"); unknown.headers[0].1 = "../etc".into();
    assert_eq!(a.p.signed_in("deliver", &unknown).unwrap_err().body, b"unknown sender");
    let mut nobody = req(now, None, "{}"); nobody.headers[0].1 = "p9".into();
    assert_eq!(a.p.signed_in("deliver", &nobody).unwrap_err().status, "401 Unauthorized", "no secret for that peer lets nothing in");
    // a different kind is a different key: the same signature signs one kind once
    assert!(a.p.signed_in("link", &req(now, None, "{}")).is_ok());
}
#[test] fn pair_unpct() {
    assert_eq!(unpct("a%20b%2Fc%E2%9C%93"), "a b/c✓"); assert_eq!(unpct("100%"), "100%"); assert_eq!(unpct("%zz"), "%zz");
}
#[test] fn pair_loopback_is_never_announced_across_the_network() {
    assert!(refuse_loopback("http://127.0.0.1:3787", "http://100.64.0.2:3787").is_some());
    assert!(refuse_loopback("http://localhost:1", "http://box.ts.net:1").is_some());
    assert!(refuse_loopback("http://[::1]:1", "http://box.ts.net:1").is_some());
    assert!(refuse_loopback("http://127.0.0.1:1", "http://127.0.0.1:2").is_none(), "two hubs on one computer");
    assert!(refuse_loopback("http://100.64.0.1:1", "http://100.64.0.2:1").is_none());
}
#[tokio::test] async fn pair_a_loopback_hub_refuses_to_join_a_network_invite() {
    let b = node("b10", false).await;
    let i = sign::Inv { url: "http://100.64.0.2:3787".into(), id: "p0123456789abcdef".into(), secret: "ab".repeat(32) };
    let e = b.p.join(&b.hub, &i, "").await.unwrap_err();
    assert!(e.contains("loopback"), "{e}");
    assert!(botnet::peer_secret(&b.home, &i.id).is_empty(), "nothing kept");
}

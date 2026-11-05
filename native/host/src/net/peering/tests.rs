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
pub(crate) async fn settle() { for _ in 0..50 { tokio::time::sleep(std::time::Duration::from_millis(20)).await; } }

#[tokio::test] async fn pair_two_hubs_link_through_the_owner_path() {
    let (a, b) = (node("a1", true).await, node("b1", false).await);
    know(&a, &b); know(&b, &a);
    // b asks a; a's front tags the socket trusted and the Host is one of a's own authorities
    b.p.ask(&b.hub, &url(&a), &url(&b), true).await;
    settle().await;
    let (la, lb) = (live_ids(&a), live_ids(&b));
    assert_eq!((la.len(), lb.len()), (1, 1), "a:{la:?} b:{lb:?}");
    assert_eq!(la, lb, "both hubs hold the same link id");
    assert_eq!(peerset(&a, &la[0]).unwrap().url, url(&b));
    assert_eq!(peerset(&b, &lb[0]).unwrap().url, url(&a));
    assert_eq!(a.p.table.with(|p| p.own_how(&la[0]).to_string()), "pair", "a vouches: Tailscale said whose it is");
    assert_eq!(b.p.table.with(|p| p.own_how(&lb[0]).to_string()), "pair", "b asked over the tailnet");
    // both keep the same 0600 secret
    assert_eq!(botnet::peer_secret(&a.home, &la[0]), botnet::peer_secret(&b.home, &la[0]));
    assert_eq!(botnet::peer_secret(&a.home, &la[0]).len(), 64);
    // a second ask finds a live link: 409
    let again = botnet::plain(&format!("{}/bots/pair?name=x&url={}", url(&a), url(&b))).await;
    assert_eq!(again.code, 409, "{}", again.body);
}
#[tokio::test] async fn pair_two_hubs_link_through_a_pairing_proof() {
    // neither is trusted: a checks b's proof by calling b back
    let (a, b) = (node("a2", false).await, node("b2", false).await);
    know(&a, &b); know(&b, &a);
    b.p.ask(&b.hub, &url(&a), &url(&b), false).await;
    settle().await;
    let (la, lb) = (live_ids(&a), live_ids(&b));
    assert_eq!((la.len(), lb.len()), (1, 1), "a:{la:?} b:{lb:?}");
    assert_eq!(a.p.table.with(|p| p.own_how(&la[0]).to_string()), "", "a link made on a proof is not taken as the owner's: Settings asks");
    assert_eq!(b.p.table.with(|p| p.own_how(&lb[0]).to_string()), "");
}
#[tokio::test] async fn pair_a_forged_or_stale_proof_is_refused() {
    let (a, b) = (node("a3", false).await, node("b3", false).await);
    know(&a, &b); know(&b, &a);
    let to = url(&a); let now = (b.p.now)();
    let ask = |proof: String, at: u64, to: String| { let u = format!("{}/bots/pair?name=x&url={}&to={}&at={}&proof={}", url(&a), url(&b), to, at, proof); async move { botnet::plain(&u).await } };
    // a proof made with the wrong token: b does not confirm it
    let r = ask(owner::pair_proof("not-b's-token", &to, now), now, to.clone()).await;
    assert_eq!((r.code, r.body.as_str()), (403, "the asking hub did not confirm this request"));
    // a proof for another port is none of this hub's
    let r = ask(owner::pair_proof("token-b3", "http://127.0.0.1:1", now), now, "http://127.0.0.1:1".into()).await;
    assert_eq!((r.code, r.body.as_str()), (403, "not a pairing request for this hub"));
    // a stale proof: b made it, but outside the minute
    let r = ask(owner::pair_proof("token-b3", &to, now - 120), now - 120, to.clone()).await;
    assert_eq!(r.code, 403);
    // a malformed proof
    let r = ask("zz".into(), now, to.clone()).await;
    assert_eq!((r.code, r.body.as_str()), (403, "not a pairing request for this hub"));
    assert!(live_ids(&a).is_empty() && live_ids(&b).is_empty());
}
#[tokio::test] async fn pair_loopback_trust_is_not_owner_trust() {
    // a local process reaches the main listener (trusted: loopback) with the right Host and no credential: no invite as the owner
    let (a, b) = (node_with("a9", true, false).await, node("b9", false).await);
    know(&a, &b);
    a.p.set_own_hosts("127.0.0.1", "");
    let r = botnet::plain(&format!("{}/bots/pair?name=127.0.0.1&url={}", url(&a), url(&b))).await;
    assert_eq!(r.code, 401, "loopback is trusted, not the owner's device: {}", r.body);
    assert!(live_ids(&a).is_empty());
    // with a proof it takes the lan path and the link is NOT recorded as the owner's
    b.p.ask(&b.hub, &url(&a), &url(&b), false).await; settle().await;
    let l = live_ids(&a); assert_eq!(l.len(), 1);
    assert_eq!(a.p.table.with(|p| p.own_how(&l[0]).to_string()), "", "unverified: the owner answers in Settings");
}
#[tokio::test] async fn pair_ask_is_refused_without_credential_trust_or_proof() {
    let (a, b) = (node("a4", false).await, node("b4", false).await);
    know(&a, &b);
    let r = botnet::plain(&format!("{}/bots/pair?name=x&url={}", url(&a), url(&b))).await;
    assert_eq!(r.code, 401, "{}", r.body);
    // a stranger a never found
    let (c, d) = (node("c4", true).await, node("d4", false).await);
    know(&c, &b);
    let r = botnet::plain(&format!("{}/bots/pair?name=x&url={}", url(&c), url(&d))).await;
    assert_eq!((r.code, r.body.as_str()), (403, "not one of the machines this hub found"));
}
#[tokio::test] async fn pair_sync_off_refuses_and_the_trusted_path_needs_an_own_authority() {
    let (a, b) = (node("a5", true).await, node("b5", false).await);
    know(&a, &b);
    a.hub.commit(&json!({"$":"SettingSet","key":"sync.127.0.0.1","value":"off","at":1})).await.unwrap();
    let r = botnet::plain(&format!("{}/bots/pair?name=127.0.0.1&url={}", url(&a), url(&b))).await;
    assert_eq!((r.code, r.body.as_str()), (403, "sync with that machine is off here"));
    // trusted socket but the Host is not an own authority: no owner trust (and no proof): 401
    a.p.set_own_hosts("100.9.9.9", "");
    a.hub.commit(&json!({"$":"SettingSet","key":"sync.127.0.0.1","value":"","at":2})).await.unwrap();
    let r = botnet::plain(&format!("{}/bots/pair?name=127.0.0.1&url={}", url(&a), url(&b))).await;
    assert_eq!(r.code, 401, "a rebound Host does not get owner trust: {}", r.body);
}
#[tokio::test] async fn pair_a_credential_may_ask_for_an_invite_too() {
    let a = node("a6", false).await; let b = node("b6", false).await; know(&a, &b);
    let u = format!("{}/bots/pair?name=127.0.0.1&url={}", url(&a), url(&b));
    let out = tokio::process::Command::new("curl").args(["-s", "-X", "POST", "-H", "Authorization: Bearer token-a6", "-w", "\n%{http_code}", &u]).output().await.unwrap();
    let t = String::from_utf8_lossy(&out.stdout).to_string(); let (body, code) = t.rsplit_once('\n').unwrap();
    assert_eq!(code, "200", "{body}"); assert!(body.contains("\"invite\":\"bp1:"));
}
#[tokio::test] async fn pair_link_is_signed_fresh_and_once() {
    let (a, b) = (node("a7", true).await, node("b7", false).await);
    know(&a, &b); know(&b, &a);
    // a hand-made invite from a (what the invite route does), used by b
    let inv = a.p.invite(&a.hub, true).await; let body: Value = serde_json::from_slice(&inv.body).unwrap();
    let i = sign::invite_parse(body["invite"].as_str().unwrap()).unwrap();
    // an unsigned or wrongly signed /bots/link is refused
    let r = botnet::plain(&format!("{}/bots/link", url(&a))).await; assert_eq!(r.code, 401);
    let now = (a.p.now)();
    let bad = botnet::signed(&b.home, &url(&a), &i.id, &"00".repeat(32), now, "/bots/link", true, "{\"url\":\"http://x:1\"}").await;
    assert_eq!(bad.code, 401, "{}", bad.body);
    // the right secret links; the same signature again is a replay; a second link is refused
    let ok = b.p.join(&b.hub, &i, false).await; assert!(ok.is_ok(), "{ok:?}");
    let r = botnet::signed(&b.home, &url(&a), &i.id, &i.secret, now, "/bots/link", true, "{\"url\":\"http://127.0.0.1:1\",\"name\":\"z\"}").await;
    assert_eq!(r.code, 409, "{}", r.body);
    assert_eq!(b.p.join(&b.hub, &i, false).await.unwrap_err(), "already linked to that machine");
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
#[test] fn pair_unpct_and_plain() {
    assert_eq!(unpct("a%20b%2Fc%E2%9C%93"), "a b/c✓"); assert_eq!(unpct("100%"), "100%"); assert_eq!(unpct("%zz"), "%zz");
    assert!(plain("http://h:1", "5", &"a".repeat(64)));
    for bad in [("http://h:1?x", "5", "a".repeat(64)), ("http://h:1", "05", "a".repeat(64)), ("http://h:1", "5", "a".repeat(63)), ("ftp://h", "5", "a".repeat(64)), ("http://h:1#f", "5", "a".repeat(64)), ("http://h:1", "5", "a/".repeat(32))] {
        assert!(!plain(bad.0, bad.1, &bad.2), "{bad:?}");
    }
}

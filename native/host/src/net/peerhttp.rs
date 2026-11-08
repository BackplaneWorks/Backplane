//! The HTTP face of the peer handshake (`/bots/pair/*`, `/bots/link`) and the machine finder (every two minutes), which only
//! suggests machines. Each route decides its own admission (`own_auth`): a machine code, a signature, or the hub's credential;
//! the body is capped at 256 KB while it is read.
use super::{peering::{PReq, Peering}, peers::PeerTable, tailnet};
use crate::{api::{Fut, HttpHandler, HttpReply}, auth::Req, hub::Hub};
use std::{sync::Arc, time::Duration};

pub struct PeerRoutes(pub Arc<Peering>);
impl PeerRoutes {
    /// Req → PReq (the socket's `trusted` comes from the acceptor; never a credential by itself).
    pub fn preq(r: &Req) -> PReq { PReq { method: r.method.clone(), target: r.target.clone(), headers: r.headers.clone(), body: r.body.clone(), trusted: r.trusted, own_device: super::listener::is_owner_conn(r.peer) } }
}
impl HttpHandler for PeerRoutes {
    fn routes(&self) -> &'static [&'static str] { &["/bots/pair", "/bots/link"] }
    fn own_auth(&self) -> bool { true }
    fn body_max(&self) -> usize { 256 << 10 }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move { self.0.handle(hub, &Self::preq(req)).await.unwrap_or_else(|| HttpReply::text("404 Not Found", "not found")) })
    }
}

/// The machines list, kept fresh: tailscale's view of the owner's machines plus BACKPLANE_PEERS, each asked /hello.
async fn find_once(hub: &Arc<Hub>, peering: &Peering, table: &PeerTable) { find_with(hub, peering, table, tailnet::status_json().await, std::env::var("BACKPLANE_PEERS").unwrap_or_default()).await }
/// (`status`: `tailscale status --json`, "" when there is none; a test passes its own so it never depends on the host)
async fn find_with(hub: &Arc<Hub>, peering: &Peering, table: &PeerTable, status: String, extra: String) {
    // (this machine first, at once, but only while nothing has been published: a later pass would take every peer off the list for the
    // seconds the probes take and bring them back, a flicker in the switcher; publishes only after a probe)
    if peering.routes.machines_value().get("self").is_none() { hub.set_info("machines", &tailnet::machines_json(&status, peering.port, &[]).to_string()).await; }
    let j = tailnet::find(&status, peering.port, &extra).await;
    let mut j = j;
    let links: Vec<(String, String)> = table.with(|p| p.live().into_iter().filter(|x| p.verified(&x.id)).map(|x| (x.name, x.url)).collect());
    // an approved link is looked for at its own address too: a hub that listens on loopback behind `tailscale serve` (https, another port) is never at
    // `http://<tailnet ip>:<this hub's port>`, and without this the finder never lists it and its requests are refused as not the owner's
    for (name, url) in links {
        let have = j["peers"].as_array().map_or(false, |a| a.iter().any(|m| super::machines::hostport(m["url"].as_str().unwrap_or("")) == super::machines::hostport(&url)));
        if have || !link_up(&url).await { continue }
        if let Some(a) = j["peers"].as_array_mut() { a.push(serde_json::json!({"name": name, "url": url, "version": "linked", "named": true, "same_user": true})) }
    }
    let verified: Vec<String> = table.with(|p| p.live().into_iter().filter(|x| p.verified(&x.id)).map(|x| x.url).collect());
    // the machines list clients switch between (info "machines": {"self":{name,url,domain},"peers":[...]}; self.name is "" with no tailnet)
    hub.set_info("machines", &j.to_string()).await;
    peering.routes.machines(j, verified);
}
/// Whether a hub answers GET /hello at `url` (http or https; the link's own address, not a guess)
async fn link_up(url: &str) -> bool {
    if !super::botnet::url_ok(url) { return false }
    let a = ["-sS", "--proto", "=http,https", "-m", "4", "-H", "@-", "-w", "\n%{http_code}", "--url"].map(String::from).into_iter().chain([format!("{}/hello", url.trim_end_matches('/'))]).collect();
    super::botnet::curl(a, String::new()).await.code == 200
}
/// Whether an approved link is not among the machines just found (both machines were restarting together, or it is away): the finder asks again sooner.
fn missing_own(peering: &Peering, table: &PeerTable) -> bool {
    let j = peering.routes.machines_value();
    table.with(|p| p.live().into_iter().any(|r| p.verified(&r.id) && !p.sync_off(&r.name) && !super::farsync::own_link(p, &j, &r.id)))
}
/// How long until the next pass of the finder: every two minutes, every 15 seconds while an approved link is not found.
fn next_find(missing: bool) -> Duration { Duration::from_secs(if missing { 15 } else { 120 }) }
pub fn spawn_loops(hub: Arc<Hub>, table: Arc<PeerTable>, peering: Arc<Peering>) {
    let (t2, p2) = (table.clone(), peering.clone());
    tokio::spawn(async move {
        table.start(&hub).await; // the peer table first: the rules read it
        // (only the finder: it suggests machines; nothing here links to one by itself)
        loop { find_once(&hub, &p2, &t2).await; tokio::time::sleep(next_find(missing_own(&p2, &t2))).await }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_finder_asks_again_soon_while_an_approved_link_is_not_found() { assert_eq!(next_find(true), Duration::from_secs(15)); assert_eq!(next_find(false), Duration::from_secs(120)); }
    #[test] fn peer_routes_decide_for_themselves_and_cap_their_bodies() {
        let r = PeerRoutes(Arc::new(unsafe_peering()));
        assert!(r.own_auth() && r.body_max() == 262144 && r.routes() == ["/bots/pair", "/bots/link"]);
    }
    fn unsafe_peering() -> Peering {
        let h = std::env::temp_dir().join(format!("bp-peerhttp-{}", std::process::id())); std::fs::create_dir_all(&h).unwrap(); std::fs::write(h.join("token"), "t").unwrap();
        let (reg, routes) = super::super::start::open(&h, "127.0.0.1", 1).unwrap();
        Peering::new(h, reg, PeerTable::new(), routes, false, "127.0.0.1", 1)
    }
    /// info "machines" is the switcher's JSON, as hub publishes it: self.name "" with no tailnet, its own url, peers named in BACKPLANE_PEERS.
    #[tokio::test] async fn machines_info_is_published_in_the_switchers_shape() {
        let a = super::super::peering::tests::node("minfo", false).await;
        find_with(&a.hub, &a.p, &a.p.table, String::new(), String::new()).await;
        let info = a.hub.info_frame()["info"]["machines"].as_str().expect("info machines is a string").to_string();
        let j: serde_json::Value = serde_json::from_str(&info).unwrap();
        assert_eq!(j["self"], serde_json::json!({"name": "", "url": format!("http://127.0.0.1:{}", a.port), "domain": ""}), "{info}");
        assert_eq!(j["peers"], serde_json::json!([]));
        assert!(info.starts_with(r#"{"self":{"name":"","url":"http://127.0.0.1:"#), "key order as : {info}");
    }
    /// A later pass never shows a list without the peers the first found.
    #[tokio::test] async fn machines_info_never_drops_the_peers_between_passes() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let a = super::super::peering::tests::node("minfo2", false).await;
        // a stand-in hub that answers /hello
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let other = format!("http://{}", l.local_addr().unwrap());
        tokio::spawn(async move { loop { let Ok((mut c, _)) = l.accept().await else { return }; tokio::spawn(async move { let mut b = [0u8; 512]; let _ = c.read(&mut b).await; let body = crate::cbor::encode(&serde_json::json!({"backplane":"0.14.0"}));
            let _ = c.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes()).await; let _ = c.write_all(&body).await; }); } });
        let (conn, _) = a.hub.connect(true); let mut ob = a.hub.attach(conn.client);
        find_with(&a.hub, &a.p, &a.p.table, String::new(), other.clone()).await;       // pass 1: self, then self + the peer
        find_with(&a.hub, &a.p, &a.p.table, String::new(), other.clone()).await;       // pass 2
        let mut seen = vec![];
        while let Ok(f) = ob.try_recv() { if let crate::hub::Out::Value(v) = f { if let Some(m) = v["info"]["machines"].as_str() { seen.push(serde_json::from_str::<serde_json::Value>(m).unwrap()) } } }
        assert_eq!(seen.len(), 2, "self once, then the probed list; pass 2 found the same list (no change, no frame) and published no self-only one in between: {seen:?}");
        assert_eq!(seen[0]["peers"].as_array().unwrap().len(), 0);
        assert!(seen[1..].iter().all(|j| j["peers"].as_array().unwrap().len() == 1), "after the first pass no list is without the peer: {seen:?}");
    }
    #[test] fn peer_preq_carries_the_sockets_trust_and_body() {
        let mut r = Req::new("POST", "/bots/link", vec![("Host".into(), "h".into())]); r.body = b"{}".to_vec(); r.trusted = true; // (loopback-style trust)
        let p = PeerRoutes::preq(&r);
        assert_eq!((p.method.as_str(), p.route(), p.body.as_slice(), p.trusted, p.own_device), ("POST", "/bots/link", &b"{}"[..], true, false), "trusted is not owner trust");
    }
}

#[cfg(test)]
mod link_tests {
    use super::*;
    /// An approved link whose hub is not where the tailnet probe looks (another port, as behind `tailscale serve`) is found at its own address.
    #[tokio::test] async fn an_approved_link_is_found_at_its_own_address() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let a = super::super::peering::tests::node("linkfind", false).await;
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let other = format!("http://{}", l.local_addr().unwrap());
        tokio::spawn(async move { loop { let Ok((mut c, _)) = l.accept().await else { return }; tokio::spawn(async move { let mut b = [0u8; 512]; let _ = c.read(&mut b).await;
            let _ = c.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").await; }); } });
        a.hub.commit_many(&[serde_json::json!({"$":"PeerSet","id":"pfar","name":"far","url":other,"revoked":false,"at":1}), serde_json::json!({"$":"SettingSet","key":"peer.own.pfar","value":"approved","at":1})]).await.unwrap();
        super::super::peering::tests::settle().await;
        find_with(&a.hub, &a.p, &a.p.table, String::new(), String::new()).await;
        let own = a.p.table.with(|p| super::super::farsync::own_link(p, &a.p.routes.machines_value(), "pfar"));
        assert!(own, "{}", a.p.routes.machines_value());
        assert!(!link_up("http://127.0.0.1:1").await);
    }
}

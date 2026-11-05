//! The HTTP face of the peer handshake (`/bots/pair`, `/bots/pair/check`, `/bots/link`) and the two loops that keep it
//! going: the machine finder (every two minutes) and the pairing ask (every minute, once per machine not yet linked).
//! Each route decides its own admission (`own_auth`): a credential, `Auth.owner.pair`, or a pairing proof the asker
//! confirms; the body is capped at 256 KB while it is read.
use super::{machines, peering::{PReq, Peering}, peers::PeerTable, tailnet};
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
async fn find_once(hub: &Arc<Hub>, peering: &Peering, table: &PeerTable) {
    let status = tailnet::status_json().await;
    let extra = std::env::var("BACKPLANE_PEERS").unwrap_or_default();
    // (this machine first, at once: the probes of the others take seconds)
    hub.set_info("machines", &tailnet::machines_json(&status, peering.port, &[]).to_string()).await;
    let j = tailnet::find(&status, peering.port, &extra).await;
    let verified: Vec<String> = table.with(|p| p.live().into_iter().filter(|x| p.verified(&x.id)).map(|x| x.url).collect());
    // the machines list clients switch between (info "machines": {"self":{name,url,domain},"peers":[...]}; self.name is "" with no tailnet)
    hub.set_info("machines", &j.to_string()).await;
    peering.routes.machines(j, verified);
}
/// Pair.go: every machine of the owner's this hub should link to and is not linked to yet is asked for an invite.
async fn pair_once(hub: &Arc<Hub>, peering: &Arc<Peering>, table: &PeerTable) {
    let j = peering.routes.machines_value();
    let (peers, off, no) = table.with(|p| (p.live().into_iter().map(|x| machines::Peer { id: x.id, name: x.name, url: x.url }).collect::<Vec<_>>(), p.sync_off_names(), p.live().into_iter().filter(|x| p.own_how(&x.id) == "no").map(|x| x.id).collect::<std::collections::HashSet<_>>()));
    let due = machines::due(peering.tailnet, &j, &peers, &|n| off.contains(&n.to_lowercase()), &|id| no.contains(id));
    let me = tailnet::announce(peering.tailnet, j["self"]["url"].as_str().unwrap_or(""));
    for (_, url) in due {
        let named = j["peers"].as_array().into_iter().flatten().any(|m| m["url"] == url.as_str() && m["named"] == true);
        let (h, p, me) = (hub.clone(), peering.clone(), me.clone());
        tokio::spawn(async move { p.ask(&h, &url, &me, !named).await });
    }
}
pub fn spawn_loops(hub: Arc<Hub>, table: Arc<PeerTable>, peering: Arc<Peering>) {
    let (t2, p2) = (table.clone(), peering.clone());
    tokio::spawn(async move {
        table.start(&hub).await; // the peer table first: the rules read it
        let (h2, t3, p3, h3) = (hub.clone(), t2.clone(), p2.clone(), hub.clone());
        tokio::spawn(async move { loop { find_once(&h3, &p3, &t3).await; tokio::time::sleep(Duration::from_secs(120)).await } });
        loop { tokio::time::sleep(Duration::from_secs(60)).await; pair_once(&h2, &p2, &t2).await }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn peer_routes_decide_for_themselves_and_cap_their_bodies() {
        let r = PeerRoutes(Arc::new(unsafe_peering()));
        assert!(r.own_auth() && r.body_max() == 262144 && r.routes() == ["/bots/pair", "/bots/link"]);
    }
    fn unsafe_peering() -> Peering {
        let h = std::env::temp_dir().join(format!("bp-peerhttp-{}", std::process::id())); std::fs::create_dir_all(&h).unwrap(); std::fs::write(h.join("token"), "t").unwrap();
        let (reg, routes) = super::super::start::open(&h, "127.0.0.1", 1).unwrap();
        Peering::new(h, reg, PeerTable::new(), routes, false, "127.0.0.1", 1)
    }
    /// info "machines" is the switcher's JSON, as the Bend hub publishes it: self.name "" with no tailnet, its own url, peers named in BACKPLANE_PEERS.
    #[tokio::test] async fn machines_info_is_published_in_the_switchers_shape() {
        let a = super::super::peering::tests::node("minfo", false).await;
        find_once(&a.hub, &a.p, &a.p.table).await;
        let info = a.hub.info_frame()["info"]["machines"].as_str().expect("info machines is a string").to_string();
        let j: serde_json::Value = serde_json::from_str(&info).unwrap();
        assert_eq!(j["self"], serde_json::json!({"name": "", "url": format!("http://127.0.0.1:{}", a.port), "domain": ""}), "{info}");
        assert_eq!(j["peers"], serde_json::json!([]));
        assert!(info.starts_with(r#"{"self":{"name":"","url":"http://127.0.0.1:"#), "key order as Bend's: {info}");
    }
    #[test] fn peer_preq_carries_the_sockets_trust_and_body() {
        let mut r = Req::new("POST", "/bots/link", vec![("Host".into(), "h".into())]); r.body = b"{}".to_vec(); r.trusted = true; // (loopback-style trust)
        let p = PeerRoutes::preq(&r);
        assert_eq!((p.method.as_str(), p.route(), p.body.as_slice(), p.trusted, p.own_device), ("POST", "/bots/link", &b"{}"[..], true, false), "trusted is not owner trust");
    }
}

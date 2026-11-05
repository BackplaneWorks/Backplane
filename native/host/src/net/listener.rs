//! The tailnet listener (server.bend `Tailnet.listen*`, `Accept.whois`): this hub also listens on its own tailnet
//! address, and every connection there is tagged `trusted` only when `tailscale whois` says its peer is a device of
//! the same person (`Tailnet.trusts`: an owner on both sides, never tagged devices). The tag reaches handlers as
//! `Req.trusted` through `wire::serve_with`; it is never a credential. The listener's own authorities (tailnet IP and
//! MagicDNS name, with the port) are what `Auth.owner.pair` accepts as `Host` (`Peering::set_own_hosts`).
//! A main listener that already covers the tailnet address (--host 0.0.0.0 or the address itself) adds none, and
//! its connections get no owner trust: the pairing then needs a proof, as in the Bend hub.
use super::{peering::Peering, routes::AuthRoutes, tailnet};
use crate::{hub::Hub, wire};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::{net::{TcpListener, TcpStream}, process::Command};

type Fut<T> = Pin<Box<dyn Future<Output = T> + Send>>;
/// What the listener asks of Tailscale (a fake in tests; the CLI in production, nothing at all on a copied log).
pub trait Tailscale: Send + Sync {
    fn ip4(&self) -> Fut<String>;
    fn whois(&self, ip: String) -> Fut<String>;
    fn status(&self) -> Fut<String>;
}
pub struct Cli;
async fn run(args: Vec<String>) -> String {
    if crate::copy::on() { return String::new() }
    match tokio::time::timeout(Duration::from_secs(5), Command::new("tailscale").args(&args).stderr(std::process::Stdio::null()).output()).await {
        Ok(Ok(o)) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(), _ => String::new() }
}
impl Tailscale for Cli {
    fn ip4(&self) -> Fut<String> { Box::pin(async { run(vec!["ip".into(), "-4".into()]).await.lines().next().unwrap_or("").trim().to_string() }) }
    fn whois(&self, ip: String) -> Fut<String> { Box::pin(async move { run(vec!["whois".into(), "--json".into(), ip]).await }) }
    fn status(&self) -> Fut<String> { Box::pin(async { run(vec!["status".into(), "--json".into()]).await }) }
}

/// One connection: the verdict is the owner's own device, decided per connection (nothing cached).
pub async fn verdict(ts: &dyn Tailscale, owner: &str, peer_ip: &str) -> bool {
    !owner.is_empty() && tailnet::trusts(owner, &tailnet::login(&ts.whois(peer_ip.to_string()).await))
}

/// The state clients are told (`tailscale` info): reachable, unavailable.
#[derive(Debug, PartialEq, Clone)]
pub enum State { Reachable { link: String }, Unavailable }

/// Tailnet.listen: find the address, learn the owner, set the authorities, bind (unless the main listener covers it)
/// and serve. Returns the state, and runs the accept loop in the background.
pub async fn start(hub: Arc<Hub>, ts: Arc<dyn Tailscale>, peering: Arc<Peering>, routes: Arc<AuthRoutes>, host: &str, port: u16, token: &str) -> State {
    let ip = ts.ip4().await;
    if ip.is_empty() { return State::Unavailable }
    let owner = tailnet::login(&ts.whois(ip.clone()).await);
    peering.set_own_hosts(&ip, &tailnet::dns_name(&ts.status().await));
    let base = format!("http://{ip}:{port}");
    routes.address(&base);
    if tailnet::covered(host, &ip) { return State::Reachable { link: String::new() } }
    let Ok(l) = TcpListener::bind((ip.as_str(), port)).await else { return State::Unavailable };
    let gate = Arc::new(tokio::sync::Semaphore::new(32));
    tokio::spawn(accept(hub, ts, l, owner, gate));
    State::Reachable { link: tailnet::link(&base, token) }
}
async fn accept(hub: Arc<Hub>, ts: Arc<dyn Tailscale>, l: TcpListener, owner: String, gate: Arc<tokio::sync::Semaphore>) {
    loop {
        let Ok((s, peer)) = l.accept().await else { continue };
        let (hub, ts, owner, gate) = (hub.clone(), ts.clone(), owner.clone(), gate.clone());
        tokio::spawn(async move { conn(hub, ts, s, peer.ip().to_string(), owner, gate).await });
    }
}
async fn conn(hub: Arc<Hub>, ts: Arc<dyn Tailscale>, s: TcpStream, peer: String, owner: String, gate: Arc<tokio::sync::Semaphore>) {
    // (a bounded number of whois lookups at once: a flood of connections cannot spawn a flood of processes)
    let Ok(_p) = gate.acquire().await else { return };
    let trusted = verdict(ts.as_ref(), &owner, &peer).await;
    drop(_p);
    let _ = wire::serve_with(hub, s, trusted).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{api::{Fut as AFut, HttpHandler, HttpReply}, auth::Req, hub::Cfg, presenter::Canned};
    use serde_json::json;

    struct Fake { ip: String, whois: std::collections::HashMap<String, String>, status: String }
    impl Tailscale for Fake {
        fn ip4(&self) -> Fut<String> { let v = self.ip.clone(); Box::pin(async move { v }) }
        fn whois(&self, ip: String) -> Fut<String> { let v = self.whois.get(&ip).cloned().unwrap_or_default(); Box::pin(async move { v }) }
        fn status(&self) -> Fut<String> { let v = self.status.clone(); Box::pin(async move { v }) }
    }
    fn who(login: &str) -> String { json!({"UserProfile":{"LoginName":login}}).to_string() }
    struct Echo;
    impl HttpHandler for Echo {
        fn routes(&self) -> &'static [&'static str] { &["/_trust"] }
        fn own_auth(&self) -> bool { true }
        fn call<'a>(&'a self, _h: &'a Arc<Hub>, r: &'a Req) -> AFut<'a, HttpReply> { Box::pin(async move { HttpReply::text("200 OK", &format!("trusted={}", r.trusted)) }) }
    }
    async fn node(tag: &str) -> (Arc<Hub>, Arc<Peering>, Arc<AuthRoutes>, std::path::PathBuf) {
        let h = std::env::temp_dir().join(format!("bp-listener-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&h); std::fs::create_dir_all(&h).unwrap();
        std::fs::write(h.join("token"), "tok\n").unwrap();
        let hub = Hub::start(Cfg { home: h.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        hub.register_http(Arc::new(Echo));
        let (reg, routes) = super::super::start::open(&h, "0.0.0.0", 1).unwrap();
        let p = Arc::new(Peering::new(h.clone(), reg, super::super::peers::PeerTable::new(), routes.clone(), true, "0.0.0.0", 1));
        (hub, p, routes, h)
    }
    async fn get(addr: &str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = TcpStream::connect(addr).await.unwrap();
        s.write_all(format!("GET /_trust HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n").as_bytes()).await.unwrap();
        let mut b = vec![]; s.read_to_end(&mut b).await.unwrap(); String::from_utf8_lossy(&b).into_owned()
    }
    /// A free port, found by binding and letting go.
    fn free() -> u16 { std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() }

    #[tokio::test] async fn tailnet_trust_comes_from_whois_of_each_connection() {
        let (hub, p, routes, h) = node("trust").await; let port = free();
        let ts = Arc::new(Fake { ip: "127.0.0.1".into(), whois: [("127.0.0.1".to_string(), who("me@x.com"))].into(), status: r#"{"Self":{"DNSName":"box.t.ts.net."}}"#.into() });
        let st = start(hub.clone(), ts, p.clone(), routes.clone(), "127.0.0.1x", port, "tok").await;
        assert_eq!(st, State::Reachable { link: format!("http://127.0.0.1:{port}/#token=tok") });
        // the peer (127.0.0.1) and this node (127.0.0.1) are the same owner: trusted
        assert!(get(&format!("127.0.0.1:{port}")).await.contains("trusted=true"));
        let _ = std::fs::remove_dir_all(h);
    }
    #[tokio::test] async fn tailnet_a_stranger_or_a_failed_whois_is_not_trusted() {
        let (hub, p, routes, h) = node("stranger").await; let port = free();
        // this node's owner is me@x.com, but the peer 127.0.0.1 resolves to someone else after the first lookup
        struct Flip(std::sync::atomic::AtomicUsize);
        impl Tailscale for Flip {
            fn ip4(&self) -> Fut<String> { Box::pin(async { "127.0.0.1".to_string() }) }
            fn whois(&self, _ip: String) -> Fut<String> { let n = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst); Box::pin(async move { match n { 0 => who("me@x.com"), 1 => who("friend@y.com"), 2 => who("tagged-devices"), _ => String::new() } }) }
            fn status(&self) -> Fut<String> { Box::pin(async { String::new() }) }
        }
        start(hub.clone(), Arc::new(Flip(0.into())), p, routes, "127.0.0.1x", port, "tok").await;
        for _ in 0..3 { assert!(get(&format!("127.0.0.1:{port}")).await.contains("trusted=false")); }
        let _ = std::fs::remove_dir_all(h);
    }
    #[tokio::test] async fn tailnet_no_address_is_unavailable_and_sets_nothing() {
        let (hub, p, routes, h) = node("none").await;
        let ts = Arc::new(Fake { ip: String::new(), whois: Default::default(), status: String::new() });
        assert_eq!(start(hub, ts, p.clone(), routes.clone(), "127.0.0.1", 1, "tok").await, State::Unavailable);
        assert!(routes.machines_value().get("x").is_none());
        let _ = std::fs::remove_dir_all(h);
    }
    #[tokio::test] async fn tailnet_a_covering_main_listener_adds_no_second_one() {
        let (hub, p, routes, h) = node("cover").await; let port = free();
        let ts = Arc::new(Fake { ip: "127.0.0.1".into(), whois: [("127.0.0.1".to_string(), who("me@x.com"))].into(), status: String::new() });
        assert_eq!(start(hub, ts, p, routes, "0.0.0.0", port, "tok").await, State::Reachable { link: String::new() });
        assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err(), "nothing listens: the main listener covers the address");
        let _ = std::fs::remove_dir_all(h);
    }
    #[tokio::test] async fn tailnet_own_authorities_follow_the_listener() {
        let (hub, p, routes, h) = node("auth").await; let port = free(); p.set_own_hosts("", "");
        let ts = Arc::new(Fake { ip: "127.0.0.1".into(), whois: [("127.0.0.1".to_string(), who("me@x.com"))].into(), status: r#"{"Self":{"DNSName":"Box.t.ts.net."}}"#.into() });
        start(hub, ts, p.clone(), routes, "0.0.0.0", port, "tok").await;
        // Auth.owner.hosts: the ip and the MagicDNS name, each bare and with the listener's port (port 1 here is Peering's own)
        let own = p.own_hosts_for_test(); assert!(own.contains(&"127.0.0.1".to_string()) && own.contains(&"box.t.ts.net".to_string()) && own.contains(&"127.0.0.1:1".to_string()), "{own:?}");
        let _ = std::fs::remove_dir_all(h);
    }
}

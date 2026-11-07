//! The tailnet listener: this hub also listens on its own tailnet
//! address, and every connection there is tagged `trusted` only when `tailscale whois` says its peer is a device of
//! the same person (`Tailnet.trusts`: an owner on both sides, never tagged devices). The tag reaches handlers as
//! `Req.trusted` through `wire::serve_with`; it is never a credential. The listener's own authorities (tailnet IP and
//! MagicDNS name, with the port) are what `Auth.owner.pair` accepts as `Host` (`Peering::set_own_hosts`).
//! A main listener that already covers the tailnet address (--host 0.0.0.0 or the address itself) adds none, and
//! its connections get no owner trust: the pairing then needs a proof, as in hub.
use super::{peering::Peering, routes::AuthRoutes, tailnet};
use crate::{hub::Hub, wire};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::{net::{TcpListener, TcpStream}, process::Command};

type Fut<T> = Pin<Box<dyn Future<Output = T> + Send>>;
/// Connections the tailnet listener has verified as the owner's own device, by the peer's socket address (unique while
/// the connection lives; removed when it ends). Owner trust (`Auth.owner.pair`/`peer`) belongs to THIS listener alone:
/// the main listener's loopback `Req.trusted` is not it, or any local process could ask for an invite as the owner.
static OWNED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<std::net::SocketAddr>>> = std::sync::OnceLock::new();
fn owned() -> &'static std::sync::Mutex<std::collections::HashSet<std::net::SocketAddr>> { OWNED.get_or_init(Default::default) }
/// Whether a connection with this peer address came through the tailnet listener and passed its whois verdict.
pub fn is_owner_conn(peer: Option<std::net::SocketAddr>) -> bool { peer.map_or(false, |p| owned().lock().unwrap().contains(&p)) }
struct Marked(std::net::SocketAddr);
impl Marked { fn new(a: std::net::SocketAddr) -> Marked { owned().lock().unwrap().insert(a); Marked(a) } }
impl Drop for Marked { fn drop(&mut self) { owned().lock().unwrap().remove(&self.0); } }

/// What the listener asks of Tailscale (a fake in tests; the CLI in production, nothing at all on a copied log).
pub trait Tailscale: Send + Sync {
    fn ip4(&self) -> Fut<String>;
    fn whois(&self, ip: String) -> Fut<String>;
    fn status(&self) -> Fut<String>;
    /// `tailscale serve --bg --https=<https> http://127.0.0.1:<port>` (443 unless the test sets another): HTTPS on this node's MagicDNS name, proxied to the hub. Whether it took.
    fn serve(&self, _port: u16, _https: u16) -> Fut<bool> { Box::pin(async { false }) }
    /// `tailscale serve status --json`: what this node already serves ("" when it cannot be read)
    fn serve_status(&self) -> Fut<String> { Box::pin(async { String::new() }) }
}
pub struct Cli;
async fn run(args: Vec<String>) -> String {
    if crate::copy::on() { return String::new() }
    match tokio::time::timeout(Duration::from_secs(5), crate::proc::tokio_cmd(crate::proc::Kind::Net, "tailscale").args(&args).stderr(std::process::Stdio::null()).output()).await {
        Ok(Ok(o)) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(), _ => String::new() }
}
async fn run_ok(args: Vec<String>) -> bool {
    if crate::copy::on() { return false }
    matches!(tokio::time::timeout(Duration::from_secs(15), crate::proc::tokio_cmd(crate::proc::Kind::Net, "tailscale").args(&args).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()).await, Ok(Ok(s)) if s.success())
}
impl Tailscale for Cli {
    fn serve(&self, port: u16, https: u16) -> Fut<bool> { Box::pin(async move { run_ok(vec!["serve".into(), "--bg".into(), format!("--https={https}"), format!("http://127.0.0.1:{port}")]).await }) }
    fn ip4(&self) -> Fut<String> { Box::pin(async { run(vec!["ip".into(), "-4".into()]).await.lines().next().unwrap_or("").trim().to_string() }) }
    fn whois(&self, ip: String) -> Fut<String> { Box::pin(async move { run(vec!["whois".into(), "--json".into(), ip]).await }) }
    fn status(&self) -> Fut<String> { Box::pin(async { run(vec!["status".into(), "--json".into()]).await }) }
    fn serve_status(&self) -> Fut<String> { Box::pin(async { run(vec!["serve".into(), "status".into(), "--json".into()]).await }) }
}

/// `--tailscale-https`: put the hub behind `tailscale serve` on HTTPS and give a phone that address, which
/// works off the tailnet's IP names and is encrypted. The MagicDNS name comes from `tailscale status`; the address goes first among the
/// invitation routes (`AuthRoutes::address`), and the pairing link is `https://<name>/#token=<token>`. None when serve did not take
/// or the node reports no DNS name (said, not guessed).
/// The HTTPS port `tailscale serve` takes, from `BACKPLANE_HTTPS_PORT`: unset is 443; a value that is not a port 1-65535 is None, and
/// `https` then serves nothing, because falling back to 443 would take over the mapping of a hub already serving there, which is the thing the variable is for.
pub fn https_port_of(v: Option<&str>) -> Option<u16> { match v { None => Some(443), Some(v) => v.trim().parse::<u16>().ok().filter(|p| *p != 0) } }
pub async fn https(ts: &dyn Tailscale, routes: &AuthRoutes, port: u16, token: &str) -> Option<String> {
    let v = std::env::var("BACKPLANE_HTTPS_PORT").ok();
    let Some(hp) = https_port_of(v.as_deref()) else { eprintln!("BACKPLANE_HTTPS_PORT={:?} is not a port: no tailscale serve, no https address", v.unwrap_or_default()); return None };
    https_on(ts, routes, port, hp, token).await
}
/// Whether the HTTPS port `hp` of `tailscale serve status --json` is already taken by something other than a proxy to this hub (`http://127.0.0.1:<port>`): another
/// hub's, a dev server's, the person's. Serving on it would replace that mapping, and our later `off` would remove it. Some(what holds it), None when free or ours.
pub fn serve_taken(status: &str, hp: u16, port: u16) -> Option<String> {
    let Ok(j) = serde_json::from_str::<serde_json::Value>(status) else { return None };
    let ours = format!("http://127.0.0.1:{port}");
    let mut held: Vec<String> = vec![];
    for (k, v) in j["Web"].as_object().into_iter().flatten() {
        if k.rsplit(':').next().and_then(|p| p.parse::<u16>().ok()) != Some(hp) { continue }
        for h in v["Handlers"].as_object().into_iter().flat_map(|m| m.values()) {
            let what = h["Proxy"].as_str().or(h["Path"].as_str()).or(h["Text"].as_str()).unwrap_or("a handler").to_string();
            if what != ours { held.push(what) }
        }
    }
    if held.is_empty() && j["TCP"].get(hp.to_string()).is_some() && j["Web"].as_object().map_or(true, |w| !w.keys().any(|k| k.rsplit(':').next().and_then(|p| p.parse::<u16>().ok()) == Some(hp))) { held.push("a TCP forward".into()) }
    if held.is_empty() { None } else { Some(held.join(", ")) }
}
async fn https_on(ts: &dyn Tailscale, routes: &AuthRoutes, port: u16, hp: u16, token: &str) -> Option<String> {
    if let Some(what) = serve_taken(&ts.serve_status().await, hp, port) { eprintln!("tailscale serve port {hp} is already used here ({what}): not taking it; choose another BACKPLANE_HTTPS_PORT"); return None }
    if !ts.serve(port, hp).await { return None }
    let name = tailnet::dns_name(&ts.status().await);
    if name.is_empty() { return None }
    let base = if hp == 443 { format!("https://{name}") } else { format!("https://{name}:{hp}") };
    routes.address(&base);
    Some(tailnet::link(&base, token))
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
    let base = format!("http://{ip}:{port}");
    routes.address(&base);
    // a main listener that covers the address gets no owner trust: the authorities are set only once this listener is bound
    if tailnet::covered(host, &ip) { return State::Reachable { link: String::new() } }
    let Ok(l) = TcpListener::bind((ip.as_str(), port)).await else { return State::Unavailable };
    crate::guard::listening(&format!("{ip}:{port}")); // rescue answers here too, while the hub is down
    peering.set_own_hosts(&ip, &tailnet::dns_name(&ts.status().await));
    let gate = Arc::new(tokio::sync::Semaphore::new(32));
    let waiting = Arc::new(tokio::sync::Semaphore::new(128));
    tokio::spawn(accept(hub, ts, l, owner, gate, waiting));
    State::Reachable { link: tailnet::link(&base, token) }
}
async fn accept(hub: Arc<Hub>, ts: Arc<dyn Tailscale>, l: TcpListener, owner: String, gate: Arc<tokio::sync::Semaphore>, waiting: Arc<tokio::sync::Semaphore>) {
    loop {
        // (an accept error such as EMFILE would spin: back off)
        let Ok((s, peer)) = l.accept().await else { tokio::time::sleep(Duration::from_millis(50)).await; continue };
        // at most 128 connections wait for their whois verdict; more are dropped, not held open
        let Ok(w) = waiting.clone().try_acquire_owned() else { continue };
        let (hub, ts, owner, gate) = (hub.clone(), ts.clone(), owner.clone(), gate.clone());
        tokio::spawn(async move { conn(hub, ts, s, peer, owner, gate, w).await });
    }
}
async fn conn(hub: Arc<Hub>, ts: Arc<dyn Tailscale>, s: TcpStream, peer: std::net::SocketAddr, owner: String, gate: Arc<tokio::sync::Semaphore>, waiting: tokio::sync::OwnedSemaphorePermit) {
    // (a bounded number of whois lookups at once: a flood of connections cannot spawn a flood of processes)
    let Ok(p) = gate.acquire().await else { return };
    let trusted = verdict(ts.as_ref(), &owner, &peer.ip().to_string()).await;
    drop(p); drop(waiting);
    // the owner mark lives exactly as long as the connection
    let _mark = trusted.then(|| Marked::new(peer));
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
        fn call<'a>(&'a self, _h: &'a Arc<Hub>, r: &'a Req) -> AFut<'a, HttpReply> { Box::pin(async move { HttpReply::text("200 OK", &format!("trusted={} owner_conn={}", r.trusted, is_owner_conn(r.peer))) }) }
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
    /// A free port, found by binding and letting go (another test may take it before the listener does: `up` tries again).
    fn free() -> u16 { std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() }
    /// `start` on a free port, again on another when the bind lost the race to a parallel test.
    async fn up(hub: &Arc<Hub>, ts: Arc<dyn Tailscale>, p: &Arc<Peering>, routes: &Arc<AuthRoutes>, host: &str) -> (State, u16) {
        for _ in 0..10 { let port = free(); let st = start(hub.clone(), ts.clone(), p.clone(), routes.clone(), host, port, "tok").await; if st != State::Unavailable { return (st, port) } }
        panic!("no free port to listen on")
    }

    #[tokio::test] async fn tailnet_trust_comes_from_whois_of_each_connection() {
        let (hub, p, routes, h) = node("trust").await;
        let ts = Arc::new(Fake { ip: "127.0.0.1".into(), whois: [("127.0.0.1".to_string(), who("me@x.com"))].into(), status: r#"{"Self":{"DNSName":"box.t.ts.net."}}"#.into() });
        let (st, port) = up(&hub, ts, &p, &routes, "127.0.0.1x").await;
        assert_eq!(st, State::Reachable { link: format!("http://127.0.0.1:{port}/#token=tok") });
        // the peer (127.0.0.1) and this node (127.0.0.1) are the same owner: trusted
        let t = get(&format!("127.0.0.1:{port}")).await; assert!(t.contains("trusted=true") && t.contains("owner_conn=true"), "{t}");
        assert!(!is_owner_conn(Some("127.0.0.1:9".parse().unwrap())), "the mark is gone with the connection (and never for another address)");
        let _ = std::fs::remove_dir_all(h);
    }
    #[tokio::test] async fn tailnet_a_stranger_or_a_failed_whois_is_not_trusted() {
        let (hub, p, routes, h) = node("stranger").await;
        // this node's owner is me@x.com, but the peer 127.0.0.1 resolves to someone else after the first lookup
        struct Flip(std::sync::atomic::AtomicUsize);
        impl Tailscale for Flip {
            fn ip4(&self) -> Fut<String> { Box::pin(async { "127.0.0.1".to_string() }) }
            fn whois(&self, _ip: String) -> Fut<String> { let n = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst); Box::pin(async move { match n { 0 => who("me@x.com"), 1 => who("friend@y.com"), 2 => who("tagged-devices"), _ => String::new() } }) }
            fn status(&self) -> Fut<String> { Box::pin(async { String::new() }) }
        }
        let (_, port) = up(&hub, Arc::new(Flip(0.into())), &p, &routes, "127.0.0.1x").await;
        for _ in 0..3 { let t = get(&format!("127.0.0.1:{port}")).await; assert!(t.contains("trusted=false") && t.contains("owner_conn=false"), "{t}"); }
        let _ = std::fs::remove_dir_all(h);
    }
    /// What a phone is handed. The hub's main routes are on one port (the Host a client reached it by), the tailnet listener on
    /// its own: once the listener is up its address is among the routes, and it is the one the link names.
    #[tokio::test] async fn tailnet_the_pairing_link_and_qr_name_the_listeners_address_not_the_hosts() {
        let (hub, p, _unused, h) = node("link").await;
        let h2 = std::env::temp_dir().join(format!("bp-listener-link2-{}", std::process::id())); let _ = std::fs::remove_dir_all(&h2); std::fs::create_dir_all(&h2).unwrap(); std::fs::write(h2.join("token"), "tok\n").unwrap();
        let ts = Arc::new(Fake { ip: "127.0.0.1".into(), whois: [("127.0.0.1".to_string(), who("me@x.com"))].into(), status: String::new() });
        let main_port = free();
        let (_, routes) = super::super::start::open(&h2, "127.0.0.1", main_port).unwrap();
        let (mut port, mut up_) = (0, false);
        for _ in 0..10 { port = free(); if port != main_port && start(hub.clone(), ts.clone(), p.clone(), routes.clone(), "127.0.0.1x", port, "tok").await != State::Unavailable { up_ = true; break } }
        assert!(up_ && port != main_port, "a listener port of its own");
        let req = |host: u16| Req { method: "POST".into(), target: "/auth/invitations".into(), headers: vec![("Authorization".into(), "Bearer tok".into()), ("Host".into(), format!("127.0.0.1:{host}"))], body: vec![], peer: None, trusted: false };
        let out = routes.handle(&req(main_port), 1).unwrap(); assert_eq!(out.reply.status, "200 OK", "{}", String::from_utf8_lossy(&out.reply.body));
        let b: serde_json::Value = serde_json::from_slice(&out.reply.body).unwrap();
        let (l_addr, m_addr) = (format!("http://127.0.0.1:{port}"), format!("http://127.0.0.1:{main_port}"));
        assert_eq!(b["routes"], json!([l_addr, m_addr]), "the listener's confirmed address, then the one this request came to: {b}");
        assert!(b["link"].as_str().unwrap().starts_with(&format!("{l_addr}/pair#invite=")), "the link names the listener's address first: {b}");
        assert_eq!(b["qr"], crate::net::qr::text(b["link"].as_str().unwrap()), "the QR encodes that link");
        // without the listener (never started) the link would name only the address the request came to
        let (_, bare) = super::super::start::open(&h2, "127.0.0.1", main_port).unwrap();
        let o2: serde_json::Value = serde_json::from_slice(&bare.handle(&req(main_port), 1).unwrap().reply.body).unwrap();
        assert_eq!(o2["routes"], json!([m_addr]), "so the listener is what puts its address there");
        let _ = std::fs::remove_dir_all(h); let _ = std::fs::remove_dir_all(h2);
    }
    struct Serve { took: bool, status: String }
    impl Tailscale for Serve {
        fn ip4(&self) -> Fut<String> { Box::pin(async { String::new() }) }
        fn whois(&self, _ip: String) -> Fut<String> { Box::pin(async { String::new() }) }
        fn status(&self) -> Fut<String> { let v = self.status.clone(); Box::pin(async move { v }) }
        fn serve(&self, _port: u16, _https: u16) -> Fut<bool> { let t = self.took; Box::pin(async move { t }) }
    }
    #[tokio::test] async fn tailnet_https_gives_a_phone_the_magicdns_address() {
        let (_hub, _p, routes, h) = node("https").await;
        let ts = Serve { took: true, status: r#"{"Self":{"DNSName":"box.tail9.ts.net."}}"#.into() };
        let link = https_on(&ts, &routes, 3787, 443, "tok").await;
        assert_eq!(link.as_deref(), Some("https://box.tail9.ts.net/#token=tok"));
        // an invitation then names it first, ahead of the address the request came to (a phone off the tailnet IP can use it)
        let req = Req { method: "POST".into(), target: "/auth/invitations".into(), headers: vec![("Authorization".into(), "Bearer tok".into()), ("Host".into(), "127.0.0.1:3787".into())], body: vec![], peer: None, trusted: false };
        let b: serde_json::Value = serde_json::from_slice(&routes.handle(&req, 1).unwrap().reply.body).unwrap();
        assert!(b["link"].as_str().unwrap().starts_with("https://box.tail9.ts.net/pair#invite="), "{b}");
        assert_eq!(b["routes"][0], "https://box.tail9.ts.net");
        // serve refused, or no DNS name: nothing is claimed
        assert_eq!(https_on(&Serve { took: false, status: r#"{"Self":{"DNSName":"x.ts.net."}}"#.into() }, &routes, 3787, 443, "tok").await, None);
        assert_eq!(https_on(&Serve { took: true, status: "{}".into() }, &routes, 3787, 443, "tok").await, None);
        let _ = std::fs::remove_dir_all(h);
    }
    #[test] fn a_serve_port_in_use_is_not_taken_and_ours_is_not_taken_twice_over() {
        let st = |p: u16, to: &str| json!({"TCP":{p.to_string():{"HTTPS":true}},"Web":{format!("box.ts.net:{p}"):{"Handlers":{"/":{"Proxy":to}}}}}).to_string();
        assert_eq!(serve_taken(&st(8451, "http://127.0.0.1:3797"), 8451, 3793), Some("http://127.0.0.1:3797".into()), "another hub's mapping on the port");
        assert_eq!(serve_taken(&st(8451, "http://127.0.0.1:3797"), 8451, 3797), None, "our own mapping (a restart) is fine");
        assert_eq!(serve_taken(&st(8451, "http://127.0.0.1:3797"), 8453, 3798), None, "another port is free");
        assert_eq!(serve_taken("{}", 8451, 3797), None); assert_eq!(serve_taken("", 8451, 3797), None); assert_eq!(serve_taken("not json", 8451, 3797), None);
        assert_eq!(serve_taken(&json!({"TCP":{"8451":{"TCPForward":"127.0.0.1:22"}}}).to_string(), 8451, 3797), Some("a TCP forward".into()));
    }
    #[tokio::test] async fn https_does_not_serve_on_a_port_someone_else_holds() {
        struct Busy(std::sync::atomic::AtomicBool);
        impl Tailscale for Busy {
            fn ip4(&self) -> Fut<String> { Box::pin(async { String::new() }) }
            fn whois(&self, _: String) -> Fut<String> { Box::pin(async { String::new() }) }
            fn status(&self) -> Fut<String> { Box::pin(async { r#"{"Self":{"DNSName":"box.ts.net."}}"#.into() }) }
            fn serve_status(&self) -> Fut<String> { Box::pin(async { json!({"Web":{"box.ts.net:8451":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:3797"}}}}}).to_string() }) }
            fn serve(&self, _: u16, _: u16) -> Fut<bool> { self.0.store(true, std::sync::atomic::Ordering::SeqCst); Box::pin(async { true }) }
        }
        let (_hub, _p, routes, h) = node("busy").await; let ts = Busy(Default::default());
        assert_eq!(https_on(&ts, &routes, 3793, 8451, "tok").await, None);
        assert!(!ts.0.load(std::sync::atomic::Ordering::SeqCst), "serve was never run on a taken port");
        let _ = std::fs::remove_dir_all(h);
    }
    #[test] fn https_port_unset_is_443_and_a_bad_value_is_refused_never_443() {
        assert_eq!(https_port_of(None), Some(443));
        assert_eq!(https_port_of(Some("3917")), Some(3917));
        for bad in ["", "0", "39x7", "65536", "-1", "443 80"] { assert_eq!(https_port_of(Some(bad)), None, "{bad:?}") }
    }
    #[tokio::test] async fn https_on_another_port_is_in_the_base_the_link_and_the_routes() {
        let (_hub, _p, routes, h) = node("httpsport").await;
        let ts = Serve { took: true, status: r#"{"Self":{"DNSName":"box.tail9.ts.net."}}"#.into() };
        assert_eq!(https_on(&ts, &routes, 3787, 3917, "tok").await.as_deref(), Some("https://box.tail9.ts.net:3917/#token=tok"));
        let req = Req { method: "POST".into(), target: "/auth/invitations".into(), headers: vec![("Authorization".into(), "Bearer tok".into()), ("Host".into(), "127.0.0.1:3787".into())], body: vec![], peer: None, trusted: false };
        let b: serde_json::Value = serde_json::from_slice(&routes.handle(&req, 1).unwrap().reply.body).unwrap();
        assert_eq!(b["routes"][0], "https://box.tail9.ts.net:3917");
        assert!(b["link"].as_str().unwrap().starts_with("https://box.tail9.ts.net:3917/pair#invite="), "{b}");
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
        // The test holds the port throughout: a second listener could not bind it (start would answer Unavailable, not the covered state),
        // and no other test can take it in between. (It was freed and probed with a connect, which a busy machine's next test could answer.)
        let (hub, p, routes, h) = node("cover").await;
        let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); let port = held.local_addr().unwrap().port();
        let ts = Arc::new(Fake { ip: "127.0.0.1".into(), whois: [("127.0.0.1".to_string(), who("me@x.com"))].into(), status: String::new() });
        assert_eq!(start(hub, ts, p.clone(), routes, "0.0.0.0", port, "tok").await, State::Reachable { link: String::new() }, "the main listener covers the address: no bind, no link");
        assert!(p.own_hosts_for_test().is_empty(), "no owner authorities: they are set only once a tailnet listener is bound");
        drop(held);
        let _ = std::fs::remove_dir_all(h);
    }
    #[tokio::test] async fn tailnet_own_authorities_follow_the_listener() {
        let (hub, p, routes, h) = node("auth").await; let port = free(); p.set_own_hosts("", "");
        let ts = Arc::new(Fake { ip: "127.0.0.1".into(), whois: [("127.0.0.1".to_string(), who("me@x.com"))].into(), status: r#"{"Self":{"DNSName":"Box.t.ts.net."}}"#.into() });
        start(hub, ts, p.clone(), routes, "0.0.0.0", port, "tok").await;
        // Auth.owner.hosts: the ip and the MagicDNS name, each bare and with the listener's port (port 1 here is Peering's own)
        assert!(p.own_hosts_for_test().is_empty(), "a covering main listener sets no owner authorities");
        let ts2 = Arc::new(Fake { ip: "127.0.0.1".into(), whois: [("127.0.0.1".to_string(), who("me@x.com"))].into(), status: r#"{"Self":{"DNSName":"Box.t.ts.net."}}"#.into() });
        let (hub2, p2, routes2, h2) = node("auth2").await;
        up(&hub2, ts2, &p2, &routes2, "127.0.0.1x").await;
        let own = p2.own_hosts_for_test(); let _ = std::fs::remove_dir_all(h2); assert!(own.contains(&"127.0.0.1".to_string()) && own.contains(&"box.t.ts.net".to_string()) && own.contains(&"127.0.0.1:1".to_string()), "{own:?}");
        let _ = std::fs::remove_dir_all(h);
    }
}

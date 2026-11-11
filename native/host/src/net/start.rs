//! Installs the lane on the hub: the device registry behind `Auth` (so enroll and revoke act at once and
//! devices-v1.json is not read again), and the /auth routes. Thin wrappers over `pairing::Registry` and
//! `routes::AuthRoutes`; the hub depends on `api::Pairing` / `api::AuthRoute`, not on these types.
use super::{pairing::Registry, routes::AuthRoutes};
use crate::{api::{AuthOutcome, AuthRoute, Pairing}, auth::Req, hub::Hub};
use std::sync::Arc;

impl Pairing for Registry {
    fn holds(&self, token: &str) -> bool { Registry::holds(self, token) }
    fn resolve(&self, token: &str, client: u64, route: &str) -> String { Registry::resolve(self, token, client, route) }
    fn active(&self, client: u64) -> bool { Registry::active(self, client) }
    fn forget(&self, client: u64) { Registry::forget(self, client) }
}
/// The routes plus the hub, for the one thing a revoke asks of it: removing the link a machine invite made.
pub struct Wired { routes: Arc<AuthRoutes>, hub: std::sync::Weak<Hub> }
impl AuthRoute for Wired {
    fn handle(&self, req: &Req, client: u64) -> Option<AuthOutcome> {
        self.routes.handle(req, client).map(|o| AuthOutcome { reply: o.reply, close: o.close, unpeer: o.unpeer })
    }
    /// Conn.devices.unpeer: the link is removed as bots.unpeer removes it (its secret is gone already).
    fn unpeer(&self, link: &str) {
        let (Some(hub), Ok(rt)) = (self.hub.upgrade(), tokio::runtime::Handle::try_current()) else { return };
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let ev = unpeer_event(link, at);
        rt.spawn(async move { let _ = hub.commit(&ev).await; });
    }
}
/// M.PeerSet{peer, "", "", revoked True, now}
pub fn unpeer_event(link: &str, at: u64) -> serde_json::Value { serde_json::json!({"$":"PeerSet","id":link,"name":"","url":"","revoked":true,"at":at}) }

static REGS: std::sync::Mutex<Vec<(std::path::PathBuf, std::sync::Weak<Registry>)>> = std::sync::Mutex::new(Vec::new());
/// The device registry running for `home` (the 0.14 import adds 0.14's paired devices to it).
pub fn registry(home: &std::path::Path) -> Option<Arc<Registry>> { REGS.lock().unwrap().iter().find(|(h, _)| h == home).and_then(|(_, w)| w.upgrade()) }
/// Pairing.start: a registry that is invalid, or belongs to another hub token, stops the hub with its
/// reason and leaves the file as it was ("existing pairings were preserved").
pub fn open(home: &std::path::Path, host: &str, port: u16) -> Result<(Arc<Registry>, Arc<AuthRoutes>), String> {
    let root = std::fs::read_to_string(home.join("token")).unwrap_or_default().trim().to_string();
    // the registry's owner is the hub token: with no token yet there is no owner to bind it to
    if root.is_empty() { return Err("the hub token does not exist yet; the device registry starts after it".to_string()) }
    let reg = Arc::new(Registry::open(home, &root).map_err(|_| "Cannot read or initialize device registry; existing pairings were preserved".to_string())?);
    REGS.lock().unwrap().retain(|(h, w)| w.strong_count() > 0 && h != home); REGS.lock().unwrap().push((home.to_path_buf(), Arc::downgrade(&reg)));
    Ok((reg.clone(), Arc::new(AuthRoutes::new(reg, host, port, super::qr::text))))
}

/// Call after the token exists and before the listener accepts. Returns the routes so the listener can
/// report its confirmed addresses (`address`) and the machine finder the owner's machines (`machines`);
/// until it does, invitations carry only the Host header's route and /auth/known knows this listener only.
static PEERING: std::sync::OnceLock<Arc<super::peering::Peering>> = std::sync::OnceLock::new();
/// The peer machinery (handshake, signed intake), for the far routes and the tailnet listener.
pub fn peering() -> Option<Arc<super::peering::Peering>> { PEERING.get().cloned() }
static ROUTES: std::sync::OnceLock<Arc<AuthRoutes>> = std::sync::OnceLock::new();
/// The installed routes, for the listener (`address(link)` once Serve/the tailnet bind is confirmed) and the
/// machine finder (`machines(json, peers)`). None before `start`.
pub fn routes() -> Option<Arc<AuthRoutes>> { ROUTES.get().cloned() }
static HUB: std::sync::OnceLock<std::sync::Weak<Hub>> = std::sync::OnceLock::new();
/// A revoke's consequences on the hub: its connections close and the link it made goes (`routes::revoked` says which).
pub fn settle_revoke(hub: &Hub, ar: &dyn AuthRoute, op: &str, j: &serde_json::Value) {
    let (close, unpeer) = super::routes::revoked(op, j);
    hub.close_clients(&close);
    if let Some(link) = unpeer { ar.unpeer(&link) }
}
/// Settings → Devices: `op` is "revoke" (a device id) or "uninvite" (an invitation id), acted on as POST /auth/revoke acts on it.
pub fn revoke(op: &str, id: &str) {
    let Some(routes) = routes() else { return };
    let j = if op == "uninvite" { routes.reg.uninvite(id) } else { routes.reg.revoke(id) };
    if let Some((hub, ar)) = HUB.get().and_then(std::sync::Weak::upgrade).and_then(|h| h.auth_route().map(|a| (h, a))) { settle_revoke(&hub, ar.as_ref(), op, &j) }
}

pub fn start(hub: &Arc<Hub>, host: &str, port: u16) -> Arc<AuthRoutes> { start_with(hub, host, port, false) }
/// `tailnet`: this hub uses Tailscale (started without --no-tailscale): it names itself by its MagicDNS name and pairs with
/// every machine of the owner's it finds, not only those BACKPLANE_PEERS names.
pub fn start_with(hub: &Arc<Hub>, host: &str, port: u16, tailnet: bool) -> Arc<AuthRoutes> { start_full(hub, host, port, tailnet, false) }
/// `https`: `--tailscale-https`, the hub is also put behind `tailscale serve` on HTTPS and pairing links name that address.
pub fn start_full(hub: &Arc<Hub>, host: &str, port: u16, tailnet: bool, https: bool) -> Arc<AuthRoutes> {
    match open(&hub.cfg.home, host, port) {
        Ok((reg, routes)) => {
            // (the wire blanks DeviceRegistered/DeviceDropped for clients: Hub::wire_item, laws device_token_never_sent/device_drop_never_sent)
            hub.register_rpc(Arc::new(super::devices::Devices));
            hub.register_rpc(Arc::new(super::devicespage::DevicesPage));
            let table = super::peers::PeerTable::new();
            let peering = Arc::new(super::peering::Peering::new(hub.cfg.home.clone(), reg.clone(), table.clone(), routes.clone(), tailnet, host, port));
            hub.register_http(Arc::new(super::peerhttp::PeerRoutes(peering.clone())));
            hub.register_rpc(Arc::new(super::peerrpc::PeerRpc(peering.clone())));
            hub.set_pairing(reg, Arc::new(Wired { routes: routes.clone(), hub: Arc::downgrade(hub) }));
            // the local network addresses the person let the hub answer on (Settings → Devices), listened on again
            let lan = Arc::new(super::lan::Lan::new(&hub.cfg.home, host, port, Arc::downgrade(hub)));
            routes.set_lan(lan.clone());
            if tokio::runtime::Handle::try_current().is_ok() { lan.boot(); }
            let _ = ROUTES.set(routes.clone());
            let _ = HUB.set(Arc::downgrade(hub));
            let _ = PEERING.set(peering.clone());
            if tokio::runtime::Handle::try_current().is_ok() {
                super::peerhttp::spawn_loops(hub.clone(), table.clone(), peering.clone());
                super::farhttp_start(hub.clone(), peering.clone());
                // the reverse link: polls to the linked machines that cannot reach this one, and the route for those that poll it
                super::alwayson::tunnel::install(hub, peering.clone());
                if tailnet {
                    // the tailnet listener: owner trust by whois, the pairing link, and what clients are told ("tailscale")
                    let (h, r, host) = (hub.clone(), routes.clone(), host.to_string()); let r2 = routes.clone();
                    tokio::spawn(async move {
                        let token = std::fs::read_to_string(h.cfg.home.join("token")).unwrap_or_default().trim().to_string();
                        h.set_info("tailnet", "on").await;
                        h.set_info("tailscale", "checking").await;
                        let st = super::listener::start(h.clone(), Arc::new(super::listener::Cli), peering, r, &host, port, &token).await;
                        let mut state = if matches!(st, super::listener::State::Reachable { .. }) { "reachable" } else { "unavailable" };
                        if https {
                            if let Some(link) = super::listener::https(&super::listener::Cli, &r2, port, &token).await {
                                println!("Backplane over HTTPS on your tailnet: {} (pair in Settings)", link.trim_end_matches(&format!("/#token={token}")));
                                h.set_info("tailnet", "https").await; state = "reachable";
                            } else { eprintln!("backplane: tailscale serve did not give an HTTPS address") }
                        }
                        h.set_info("tailscale", state).await;
                    });
                }
            }
            routes
        }
        // the file stays as it is, and the hub comes up on what `Auth::load` read from it: the hub token and the paired devices it could parse still sign in, and nothing can be paired until it is fixed
        Err(e) => { crate::guard::degraded("pairing", &e); Arc::new(AuthRoutes::new(Arc::new(super::pairing::Registry::empty(&hub.cfg.home)), host, port, super::qr::text)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Pairing;
    #[test] fn pair_unpeer_event_is_a_revoked_peerset() {
        assert_eq!(unpeer_event("p0123456789abcdef", 7).to_string(), r#"{"$":"PeerSet","id":"p0123456789abcdef","name":"","url":"","revoked":true,"at":7}"#);
    }
    /// The Settings door and the HTTP door share `settle_revoke`: a device's live connection is told to close, on either kind of revoke, and a used machine invite's link goes.
    #[tokio::test] async fn pair_settings_revoke_closes_the_devices_connection_and_removes_the_link() {
        use crate::{hub::{Cfg, Out}, presenter::Canned};
        struct Links(std::sync::Mutex<Vec<String>>);
        impl AuthRoute for Links {
            fn handle(&self, _r: &Req, _c: u64) -> Option<AuthOutcome> { None }
            fn unpeer(&self, link: &str) { self.0.lock().unwrap().push(link.to_string()) }
        }
        let dir = crate::plat::tmp::dir("net-settle");
        std::fs::write(dir.join("token"), "roottoken").unwrap();
        let hub = Hub::start(Cfg { home: dir.to_path_buf(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: serde_json::json!({}) })).await.unwrap();
        let (reg, _) = open(&dir, "127.0.0.1", 3787).unwrap();
        let links = Links(Default::default());
        let routes = vec!["https://m.ts.net".to_string()];
        for (n, op) in ["revoke", "uninvite"].iter().enumerate() {
            let inv = reg.create(&routes);
            let dev = reg.enroll(inv["code"].as_str().unwrap(), "Phone");
            let client = 9000 + n as u64;
            let mut outbox = hub.attach(client);
            assert_eq!(reg.resolve(dev["token"].as_str().unwrap(), client, "/ws"), dev["token"].as_str().unwrap(), "the socket is known as this device's");
            let j = if *op == "revoke" { reg.revoke(dev["id"].as_str().unwrap()) } else { reg.uninvite(inv["id"].as_str().unwrap()) };
            assert!(outbox.try_recv().is_err(), "{op}: the registry alone closes nothing");
            settle_revoke(&hub, &links, op, &j);
            assert!(matches!(outbox.try_recv(), Ok(Out::Close)), "{op}: the device's open connection is told to close");
            assert!(!reg.holds(dev["token"].as_str().unwrap()), "{op}: and its credential no longer holds");
        }
        assert!(links.0.lock().unwrap().is_empty(), "a device has no link to remove");
        let peer = "p0123456789abcdef";
        let m = reg.machine(peer); assert_eq!(reg.link(peer, "box")["ok"], true);
        settle_revoke(&hub, &links, "uninvite", &reg.uninvite(m["id"].as_str().unwrap()));
        assert_eq!(*links.0.lock().unwrap(), vec![peer.to_string()], "a used machine invite's link goes with it");
    }
    #[test] fn pair_start_needs_the_token() {
        let dir = crate::plat::tmp::dir("net-notoken");
        assert!(open(&dir, "127.0.0.1", 3787).is_err(), "no token file: no registry");
        std::fs::write(dir.join("token"), "\n").unwrap();
        assert!(open(&dir, "127.0.0.1", 3787).is_err());
        assert!(!dir.join("devices-v1.json").exists());
    }
    #[test] fn pair_holds_means_token_or_live_device() {
        let dir = crate::plat::tmp::dir("net-holds");
        std::fs::write(dir.join("token"), "roottoken").unwrap();
        let (reg, _) = open(&dir, "127.0.0.1", 3787).unwrap();
        assert!(reg.holds("roottoken") && !reg.holds("") && !reg.holds("nope"));
        let tok = reg.enroll(reg.create(&["https://m.ts.net".to_string()])["code"].as_str().unwrap(), "P")["token"].as_str().unwrap().to_string();
        assert!(reg.holds(&tok));
    }
    #[test] fn pair_start_refuses_a_registry_it_cannot_own_and_keeps_it() {
        let dir = crate::plat::tmp::dir("net-start");
        std::fs::write(dir.join("token"), "roottoken\n").unwrap();
        let (reg, _) = open(&dir, "127.0.0.1", 3787).unwrap();
        // the hub asks the trait object: the device the registry knows is held, and a stranger is not
        let p: Arc<dyn Pairing> = reg.clone();
        assert_eq!(p.resolve("", 1, "/x"), "roottoken");
        assert!(!p.holds("0123456789abcdef0123456789abcdef"));
        drop(p); drop(reg);
        std::fs::write(dir.join("token"), "othertoken").unwrap();
        let (r, _) = open(&dir, "127.0.0.1", 3787).unwrap();
        r.create(&["https://m.ts.net".to_string()]);
        std::fs::write(dir.join("token"), "third").unwrap();
        let kept = std::fs::read_to_string(dir.join("devices-v1.json")).unwrap();
        assert!(open(&dir, "127.0.0.1", 3787).is_err());
        assert_eq!(std::fs::read_to_string(dir.join("devices-v1.json")).unwrap(), kept);
    }
}

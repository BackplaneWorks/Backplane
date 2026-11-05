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

/// Pairing.start: a registry that is invalid, or belongs to another hub token, stops the hub with its
/// reason and leaves the file as it was ("existing pairings were preserved").
pub fn open(home: &std::path::Path, host: &str, port: u16) -> Result<(Arc<Registry>, Arc<AuthRoutes>), String> {
    let root = std::fs::read_to_string(home.join("token")).unwrap_or_default().trim().to_string();
    // the registry's owner is the hub token: with no token yet there is no owner to bind it to
    if root.is_empty() { return Err("the hub token does not exist yet; the device registry starts after it".to_string()) }
    let reg = Arc::new(Registry::open(home, &root).map_err(|_| "Cannot read or initialize device registry; existing pairings were preserved".to_string())?);
    Ok((reg.clone(), Arc::new(AuthRoutes::new(reg, host, port, super::qr::text))))
}

/// Call after the token exists and before the listener accepts. Returns the routes so the listener can
/// report its confirmed addresses (`address`) and the machine finder the owner's machines (`machines`);
/// until it does, invitations carry only the Host header's route and /auth/known knows this listener only.
pub fn start(hub: &Arc<Hub>, host: &str, port: u16) -> Arc<AuthRoutes> {
    match open(&hub.cfg.home, host, port) {
        Ok((reg, routes)) => { hub.set_pairing(reg, Arc::new(Wired { routes: routes.clone(), hub: Arc::downgrade(hub) })); routes }
        Err(e) => { eprintln!("backplane: {e}"); std::process::exit(1) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Pairing;
    #[test] fn pair_unpeer_event_is_a_revoked_peerset() {
        assert_eq!(unpeer_event("p0123456789abcdef", 7).to_string(), r#"{"$":"PeerSet","id":"p0123456789abcdef","name":"","url":"","revoked":true,"at":7}"#);
    }
    #[test] fn pair_start_needs_the_token() {
        let dir = std::env::temp_dir().join(format!("bp-net-notoken-{}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
        assert!(open(&dir, "127.0.0.1", 3787).is_err(), "no token file: no registry");
        std::fs::write(dir.join("token"), "\n").unwrap();
        assert!(open(&dir, "127.0.0.1", 3787).is_err());
        assert!(!dir.join("devices-v1.json").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test] fn pair_holds_means_token_or_live_device() {
        let dir = std::env::temp_dir().join(format!("bp-net-holds-{}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("token"), "roottoken").unwrap();
        let (reg, _) = open(&dir, "127.0.0.1", 3787).unwrap();
        assert!(reg.holds("roottoken") && !reg.holds("") && !reg.holds("nope"));
        let tok = reg.enroll(reg.create(&["https://m.ts.net".to_string()])["code"].as_str().unwrap(), "P")["token"].as_str().unwrap().to_string();
        assert!(reg.holds(&tok));
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test] fn pair_start_refuses_a_registry_it_cannot_own_and_keeps_it() {
        let dir = std::env::temp_dir().join(format!("bp-net-start-{}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
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
        let _ = std::fs::remove_dir_all(dir);
    }
}

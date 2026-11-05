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
impl AuthRoute for AuthRoutes {
    fn handle(&self, req: &Req, client: u64) -> Option<AuthOutcome> {
        AuthRoutes::handle(self, req, client).map(|o| AuthOutcome { reply: o.reply, close: o.close, unpeer: o.unpeer })
    }
}

/// Pairing.start: a registry that is invalid, or belongs to another hub token, stops the hub with its
/// reason and leaves the file as it was ("existing pairings were preserved").
pub fn open(home: &std::path::Path, host: &str, port: u16) -> Result<(Arc<Registry>, Arc<AuthRoutes>), String> {
    let root = std::fs::read_to_string(home.join("token")).unwrap_or_default().trim().to_string();
    let reg = Arc::new(Registry::open(home, &root).map_err(|_| "Cannot read or initialize device registry; existing pairings were preserved".to_string())?);
    Ok((reg.clone(), Arc::new(AuthRoutes::new(reg, host, port, super::qr::text))))
}

pub fn start(hub: &Arc<Hub>, host: &str, port: u16) {
    match open(&hub.cfg.home, host, port) {
        Ok((reg, routes)) => hub.set_pairing(reg, routes),
        Err(e) => { eprintln!("backplane: {e}"); std::process::exit(1) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Pairing;
    #[test] fn pair_start_refuses_a_registry_it_cannot_own_and_keeps_it() {
        let dir = std::env::temp_dir().join(format!("bp-net-start-{}", std::process::id())); std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("token"), "roottoken\n").unwrap();
        let (reg, _) = open(&dir, "127.0.0.1", 3787).unwrap();
        // the hub asks the trait object: the device the registry knows is held, and a stranger is not
        let p: Arc<dyn Pairing> = reg.clone();
        assert_eq!(p.resolve("", 1, "/x"), "roottoken");
        assert!(!p.holds("0123456789abcdef0123456789abcdef"));
        drop(p); drop(reg);
        let before = std::fs::read_to_string(dir.join("devices-v1.json")).ok();
        std::fs::write(dir.join("token"), "othertoken").unwrap();
        // (no registry file yet: nothing to refuse) create one then change the token
        let _ = before;
        let (r, _) = open(&dir, "127.0.0.1", 3787).unwrap();
        r.create(&["https://m.ts.net".to_string()]);
        std::fs::write(dir.join("token"), "third").unwrap();
        let kept = std::fs::read_to_string(dir.join("devices-v1.json")).unwrap();
        assert!(open(&dir, "127.0.0.1", 3787).is_err());
        assert_eq!(std::fs::read_to_string(dir.join("devices-v1.json")).unwrap(), kept);
        let _ = std::fs::remove_dir_all(dir);
    }
}

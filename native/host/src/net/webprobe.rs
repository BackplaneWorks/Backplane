//! One process-local forwarding proof. The requester must also observe local
//! consumption, so an unrelated HTTPS server returning 204 cannot pass.
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, Instant},
};

static PENDING: Mutex<Weak<Probe>> = Mutex::new(Weak::new());
pub struct Probe {
    home: PathBuf,
    authority: String,
    nonce: String,
    expires: Instant,
    consumed: AtomicBool,
}
impl Probe {
    pub fn start(home: &Path, authority: &str) -> Arc<Self> {
        let probe = Arc::new(Self {
            home: home.into(),
            authority: authority.into(),
            nonce: hex::encode(crate::net::pairing::urandom(32)),
            expires: Instant::now() + Duration::from_secs(30),
            consumed: AtomicBool::new(false),
        });
        *PENDING.lock().unwrap() = Arc::downgrade(&probe);
        probe
    }
    pub fn header(&self) -> String {
        format!("X-Backplane-Setup: {}\n", self.nonce)
    }
    pub fn consumed(&self) -> bool {
        self.consumed.load(Ordering::Acquire)
    }
    fn take(&self, home: &Path, authority: &str, nonce: &str, now: Instant) -> bool {
        now < self.expires
            && self.home == home
            && self.authority == authority
            && crate::net::owner::same(&self.nonce, nonce)
            && !self.consumed.swap(true, Ordering::AcqRel)
    }
}
pub fn consume(home: &Path, authority: &str, nonce: &str) -> bool {
    PENDING
        .lock()
        .unwrap()
        .upgrade()
        .is_some_and(|p| p.take(home, authority, nonce, Instant::now()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proof_requires_same_hub_authority_and_fresh_nonce_once() {
        let home = Path::new("/fixture");
        let p = Probe::start(home, "fixture");
        assert!(!p.take(home, "other", &p.nonce, Instant::now()));
        assert!(!p.take(home, "fixture", &p.nonce, p.expires));
        assert!(!p.take(home, "fixture", "wrong", Instant::now()));
        assert!(consume(home, "fixture", &p.nonce));
        assert!(!consume(home, "fixture", &p.nonce));
    }
}

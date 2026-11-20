//! A six-digit code compares two screens. A separate random cookie redeems approval.
use super::*;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tokio::sync::watch;

const WAIT: u64 = 120_000;
const DAY: u64 = 86_400_000;

#[derive(Default)]
pub(super) struct Requests {
    pending: Vec<Request>,
    starts: Budget,
    polls: Budget,
}

struct Request {
    id: String,
    name: String,
    login: String,
    code: String,
    hash: String,
    until: u64,
    decision: Option<bool>,
    waiting: Arc<AtomicBool>,
    wake: watch::Sender<()>,
}

impl Request {
    fn public(&self) -> Value {
        json!({"id":self.id, "name":self.name, "code":self.code, "expiresAt":self.until})
    }
}

fn valid_credentials(login: &str, cookie: &str) -> bool {
    !login.is_empty() && login.len() <= 320 && login != "tagged-devices" && cookie.len() == 64
        && cookie.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

struct PollGuard(Arc<AtomicBool>);
impl Drop for PollGuard {
    fn drop(&mut self) { self.0.store(false, Ordering::Release); }
}

fn current(device: &Value, at: u64) -> bool {
    let within = |field: &str, span| device[field].as_u64().and_then(|start| at.checked_sub(start)).is_some_and(|age| age < span);
    device["kind"] == "browser" && live(device) && within("createdAt", 90 * DAY) && within("lastSeen", 30 * DAY)
}

impl Registry {
    pub fn browser_request(&self, login: &str, name: &str) -> Result<(Value, String), &'static str> {
        if self.root.is_empty() || login.is_empty() || login.len() > 320 || login == "tagged-devices" { return Err("unverified_peer") }
        let mut g = self.inner.lock().unwrap();
        let at = (self.now)();
        let requests = &mut g.browser;
        requests.pending.retain(|r| r.until > at);
        if !requests.starts.take(login, at) || requests.pending.len() >= OPEN_LIMIT { return Err("rate_limited") }
        let bytes: [u8; 4] = (self.random)(4).try_into().expect("four random bytes");
        let code = format!("{:06}", u32::from_le_bytes(bytes) % 1_000_000);
        if requests.pending.iter().any(|r| r.code == code) { return Err("retry") }
        let cookie = self.rand_hex(32);
        let req = Request { id: self.rand_hex(16), name: clean(name, 80), login: login.into(), code,
            hash: digest(&cookie), until: at + WAIT, decision: None, waiting: Default::default(), wake: watch::channel(()).0 };
        let public = req.public();
        requests.pending.push(req);
        Ok((public, cookie))
    }

    pub fn browser_pending(&self) -> Value {
        let g = self.inner.lock().unwrap();
        let at = (self.now)();
        Value::Array(g.browser.pending.iter().filter(|r| r.until > at && r.decision.is_none()).map(|r| {
            let mut row = r.public();
            row["login"] = json!(r.login);
            row
        }).collect())
    }

    pub fn browser_clear_pending(&self) { self.inner.lock().unwrap().browser.pending.clear(); }

    pub fn browser_decide(&self, id: &str, approve: bool) -> bool {
        let mut g = self.inner.lock().unwrap();
        let at = (self.now)();
        let pending = &mut g.browser.pending;
        let Some(i) = pending.iter().position(|r| r.id == id && r.until > at && r.decision.is_none()) else { return false };
        pending[i].decision = Some(approve);
        pending[i].wake.send_replace(());
        true
    }

    /// The guard is released even when the HTTP task is cancelled.
    pub async fn browser_wait(&self, login: &str, cookie: &str) -> Result<Option<String>, &'static str> {
        let (_guard, mut changed, remaining) = {
            let mut g = self.inner.lock().unwrap();
            let at = (self.now)();
            let hash = digest(cookie);
            let request = g.browser.pending.iter().find(|r| valid_credentials(login, cookie) && r.login == login && same(&r.hash, &hash));
            let Some(r) = request else {
                return Err(if g.browser.polls.take(login, at) { "unknown_request" } else { "rate_limited" });
            };
            if r.waiting.swap(true, Ordering::AcqRel) { return Err("rate_limited") }
            (PollGuard(r.waiting.clone()), r.wake.subscribe(), r.until.saturating_sub(at).min(25_000))
        };
        match self.browser_poll(login, cookie)? {
            Some(token) => Ok(Some(token)),
            None => {
                let _ = tokio::time::timeout(std::time::Duration::from_millis(remaining), changed.changed()).await;
                self.browser_poll(login, cookie)
            }
        }
    }

    pub fn browser_clients(&self) -> Vec<u64> {
        let g = self.inner.lock().unwrap();
        g.clients.iter().filter(|(_,id)| g.devices.iter().any(|d| d["kind"] == "browser" && d["id"] == id.as_str())).map(|(client,_)| *client).collect()
    }

    fn browser_poll(&self, login: &str, cookie: &str) -> Result<Option<String>, &'static str> {
        let mut g = self.inner.lock().unwrap();
        let at = (self.now)();
        let hash = digest(cookie);
        let Some(i) = g.browser.pending.iter().position(|r| same(&r.hash, &hash) && r.login == login) else { return Err("unknown_request") };
        let req = &g.browser.pending[i];
        if req.until <= at { return Err("expired") }
        match req.decision { None => return Ok(None), Some(false) => { g.browser.pending.swap_remove(i); return Err("denied") }, Some(true) => {} }
        if g.devices.iter().filter(|d| live(d) && (d["kind"] != "browser" || current(d, at))).count() >= DEVICE_LIMIT { return Err("device_limit") }
        let token = self.rand_hex(32);
        let mut devices = g.devices.clone();
        devices.push(json!({"id": self.rand_hex(16), "kind":"browser", "name":req.name, "login":login,
            "createdAt":at, "lastSeen":at, "revoked":false, "hash":digest(&token)}));
        let invites = g.invites.clone();
        self.persist(&mut g, devices, invites).map_err(|_| "storage_unavailable")?;
        g.browser.pending.swap_remove(i);
        Ok(Some(token))
    }

    pub fn browser_verify(&self, login: &str, cookie: &str, client: Option<u64>, activity: bool) -> Option<String> {
        if !valid_credentials(login, cookie) { return None }
        let mut g = self.inner.lock().unwrap();
        let at = (self.now)();
        let hash = digest(cookie);
        let i = g.devices.iter().position(|d| current(d, at) && d["login"] == login && same(d["hash"].as_str().unwrap_or(""), &hash))?;
        self.browser_seen(&mut g, i, at, client, activity)
    }

    pub fn browser_connection(&self, device_id: &str, client: u64, activity: bool) -> bool {
        let mut g = self.inner.lock().unwrap();
        let at = (self.now)();
        let Some(i) = g.devices.iter().position(|d| d["id"] == device_id && current(d, at)) else { return false };
        self.browser_seen(&mut g, i, at, Some(client), activity).is_some()
    }

    fn browser_seen(&self, g: &mut Inner, i: usize, at: u64, client: Option<u64>, activity: bool) -> Option<String> {
        let id = g.devices[i]["id"].as_str()?.to_owned();
        if activity && at.saturating_sub(g.devices[i]["lastSeen"].as_u64()?) >= 60_000 {
            let mut devices = g.devices.clone();
            devices[i]["lastSeen"] = json!(at);
            self.persist(g, devices, g.invites.clone()).ok()?;
        }
        if let Some(client) = client { g.clients.insert(client, id.clone()); }
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

    #[tokio::test]
    async fn pending_poll_is_exclusive_cancellable_and_woken_by_decision() {
        let home = crate::plat::tmp::dir("browser-long-poll");
        let r = Registry::open(&home, "test").unwrap();
        let login = "person@example.com";
        let (req, cookie) = r.browser_request(login, "Browser").unwrap();
        let id = req["id"].as_str().unwrap();
        let mut first = Box::pin(r.browser_wait(login, &cookie));
        tokio::select! {
            biased;
            _ = &mut first => panic!("pending request returned"),
            _ = std::future::ready(()) => {}
        }
        assert_eq!(r.browser_wait(login, &cookie).await, Err("rate_limited"));
        drop(first);
        let mut retry = Box::pin(r.browser_wait(login, &cookie));
        tokio::select! {
            biased;
            _ = &mut retry => panic!("retry did not wait"),
            _ = std::future::ready(()) => {}
        }
        assert!(r.browser_decide(id, false));
        assert_eq!(tokio::time::timeout(std::time::Duration::from_secs(1), retry).await.unwrap(), Err("denied"));
    }

    #[test]
    fn browser_pairing_lifecycle() {
        let home = crate::plat::tmp::dir("browser-pairing");
        let clock = Arc::new(AtomicU64::new(DAY));
        let open = || {
            let time = clock.clone();
            Registry::with(&home, "test", Box::new(move || time.load(Ordering::SeqCst)), Box::new(urandom)).unwrap()
        };
        let r = open();
        let login = "person@example.com";
        let (req, poll) = r.browser_request(login, "Browser").unwrap();
        assert_eq!(req["code"].as_str().unwrap().len(), 6);
        assert_eq!(r.browser_poll(login, &poll), Ok(None));
        assert!(r.browser_decide(req["id"].as_str().unwrap(), true));
        assert!(r.browser_poll("other@example.com", &poll).is_err());
        let token = r.browser_poll(login, &poll).unwrap().unwrap();
        assert!(!r.holds(&token));
        assert!(r.browser_poll(login, &poll).is_err());
        let persisted = std::fs::read_to_string(home.join("devices-v1.json")).unwrap();
        assert!(!persisted.contains(&token) && !persisted.contains(&poll));
        let r = open();
        assert!(r.browser_verify("other@example.com", &token, None, false).is_none());
        let id = r.browser_verify(login, &token, Some(42), false).unwrap();
        clock.fetch_add(29 * DAY, Ordering::SeqCst);
        assert!(r.browser_connection(&id, 42, false));
        clock.fetch_add(DAY, Ordering::SeqCst);
        assert!(!r.browser_connection(&id, 42, true));
        assert_eq!(r.revoke(&id)["clients"], json!([42]));
    }
}

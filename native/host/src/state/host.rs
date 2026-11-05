//! The state policies in the Rust hub: a thin shell around `policy::Policy`. It registers a `Transform` (the policy state advances inside
//! `before_commit` on the writer, after the secret mask, so what a batch derives is written in that batch; no IO and no await there), a
//! `Ticker` (the minute tick) and hands out `Policies` (Auto.read for the viewers). The log is folded at start in `spawn_blocking`, read
//! AFTER the transform is registered; batches seen meanwhile are replayed past the folded length. A failed commit makes the copy fold the
//! log again. Declared from main.rs as `#[path = "state/host.rs"] mod statehost;`, started with `statehost::start(hub.clone())`.
use crate::api::{Fut, Ticker, Transform};
use crate::hub::Hub;
use crate::state::fam::policy::{Core, Policy};
use serde_json::Value;
use std::sync::{Arc, OnceLock};

struct Host { policy: Arc<Policy>, hub: OnceLock<Arc<Hub>> }
impl Transform for Host {
    fn before_commit(&self, first_seq: u64, events: Vec<Value>) -> Vec<Value> { self.policy.before_commit(first_seq, events) }
    fn commit_failed(&self, first_seq: u64) {
        if self.policy.commit_failed(first_seq) {
            eprintln!("state policies: a commit failed (log line {first_seq}); folding the log again");
            if let Some(h) = self.hub.get() { tokio::spawn(boot_task(self.policy.clone(), h.clone())); }
        }
    }
}
impl Ticker for Host {
    fn tick<'a>(&'a self, hub: &'a Arc<Hub>, now_s: u64) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if self.policy.take_poisoned() { eprintln!("state policies: the policy lock was poisoned by a panic; folding the log again"); tokio::spawn(boot_task(self.policy.clone(), hub.clone())); }
            self.policy.tick(now_s)
        })
    }
}

/// the lean fold of the whole log, on a blocking thread; the length is read first and the fold stops there
fn read_log(hub: &Hub) -> (Core, u64) {
    let mut core = Core::new();
    let len = hub.store.len();
    let mut at = 0usize;
    while at < len {
        let to = (at + 2000).min(len);
        match hub.store.range(at, to) { Ok(ls) => for l in ls { core.fold_line(&l); }, Err(e) => eprintln!("state policies: reading log lines {at}..{to} failed: {e}") }
        at = to;
    }
    (core, len as u64)
}

/// What other modules ask of the policies. `read` is Auto.read: the person opened a thread's newest reply (the view and desk-look
/// handlers call it, then commit the events it returns with their own).
pub struct Policies { policy: Arc<Policy> }
impl Policies {
    /// Watchdog.line: call for every provider line of a thread's agent (the stream-json line as parsed), commit the events it returns
    pub fn line(&self, thread: &str, now_s: u64, line: &Value) -> Vec<Value> { self.policy.line(thread, now_s, line) }
    pub async fn read(&self, thread: &str, now_s: u64) -> Vec<Value> { self.policy.read(thread, now_s) }
}

async fn boot_task(policy: Arc<Policy>, hub: Arc<Hub>) {
    let h = hub.clone();
    let (core, len) = match tokio::task::spawn_blocking(move || read_log(&h)).await { Ok(x) => x, Err(e) => { eprintln!("state policies: the log fold failed, the policies stay off: {e}"); return } };
    let now_s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let derived = policy.finish_boot(core, len, now_s);
    if !derived.is_empty() { let _ = hub.commit_many(&derived).await; }
}

static POLICIES: OnceLock<Arc<Policies>> = OnceLock::new();
/// The policies, once `start` has run: for actors that cannot be handed them (agents: `Policies::line` per provider line, `read` on a view open)
pub fn policies() -> Option<Arc<Policies>> { POLICIES.get().cloned() }

pub fn start(hub: Arc<Hub>) -> Arc<Policies> {
    let policy = Arc::new(Policy::new());
    let host = Arc::new(Host { policy: policy.clone(), hub: OnceLock::new() });
    let _ = host.hub.set(hub.clone());
    hub.register_transform(host.clone());   // first: the log length is read after this, so no batch is missed
    hub.register_ticker(host);
    tokio::spawn(boot_task(policy.clone(), hub));
    let p = Arc::new(Policies { policy });
    let _ = POLICIES.set(p.clone());
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::Cfg;
    use crate::presenter::Canned;
    use serde_json::json;
    // Hub.autosettle.upgrade runs once at start: on a log that holds launch mails and no autosettle.version, a real Hub start (bootstrap, then the
    // policies' boot fold) writes the delivery settings and the version, once. The bootstrap must not set the version itself (it would hide the mails).
    #[tokio::test] async fn a_legacy_home_is_upgraded_once_at_a_real_start() {
        let dir = std::env::temp_dir().join(format!("bp-statehost-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        let log = [json!({"$":"ProjectCreated","id":"p","title":"p","root":"/r","at":1}),
            json!({"$":"ThreadCreated","id":"boss","project":"p","title":"boss","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}),
            json!({"$":"ThreadCreated","id":"w1","project":"p","title":"w1","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}),
            json!({"$":"ThreadLaunched","id":"w1","by":"boss","report":true,"at":2}),
            json!({"$":"MessagePosted","thread":"w1","msg":"mw1","role":"user","text":"go","at":3}),
            json!({"$":"MessagePosted","thread":"w1","msg":"aw1","role":"assistant","text":"done","at":4}),
            json!({"$":"TurnChanged","thread":"w1","state":"completed","at":5}),
            json!({"$":"MailPosted","id":"t:w1:lr-1","conv":"c","from":"thread:w1","to":"thread:boss","kind":"launch","ref":"","want":"","head":"","size":0,"at":9})];
        std::fs::write(dir.join("events.jsonl"), log.iter().map(|e| e.to_string() + "\n").collect::<String>()).unwrap();
        let cfg = Cfg { home: dir.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let _p = start(hub.clone());
        let mut text = String::new();
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            text = hub.store.range(0, hub.store.len()).unwrap().join("\n");
            if text.contains("autosettle.delivery.") { break }
        }
        assert!(text.contains(r#""key":"autosettle.delivery.w1\tthread:boss","value":"9""#), "the launch mail became a receipt: {}", text.lines().filter(|l| l.contains("autosettle")).collect::<Vec<_>>().join("\n"));
        assert_eq!(text.matches(r#""key":"autosettle.version""#).count(), 1, "the version is set once, by the upgrade");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

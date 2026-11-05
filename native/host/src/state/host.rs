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
    fn tick<'a>(&'a self, _hub: &'a Arc<Hub>, now_s: u64) -> Fut<'a, Vec<Value>> { Box::pin(async move { self.policy.tick(now_s) }) }
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
    pub async fn read(&self, thread: &str, now_s: u64) -> Vec<Value> { self.policy.read(thread, now_s) }
}

async fn boot_task(policy: Arc<Policy>, hub: Arc<Hub>) {
    let h = hub.clone();
    let (core, len) = match tokio::task::spawn_blocking(move || read_log(&h)).await { Ok(x) => x, Err(e) => { eprintln!("state policies: the log fold failed, the policies stay off: {e}"); return } };
    let derived = policy.finish_boot(core, len);
    if !derived.is_empty() { let _ = hub.commit_many(&derived).await; }
}

pub fn start(hub: Arc<Hub>) -> Arc<Policies> {
    let policy = Arc::new(Policy::new());
    let host = Arc::new(Host { policy: policy.clone(), hub: OnceLock::new() });
    let _ = host.hub.set(hub.clone());
    hub.register_transform(host.clone());   // first: the log length is read after this, so no batch is missed
    hub.register_ticker(host);
    tokio::spawn(boot_task(policy.clone(), hub));
    Arc::new(Policies { policy })
}

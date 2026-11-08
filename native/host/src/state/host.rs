//! The state policies in the Rust hub: a thin shell around `policy::Policy`. It registers a `Transform` (the policy state advances inside
//! `before_commit` on the writer, after the secret mask, so what a batch derives is written in that batch; no IO and no await there), a
//! `Ticker` (the minute tick) and hands out `Policies` (Auto.read for the viewers). The log is folded at start in `spawn_blocking`, read
//! AFTER the transform is registered; batches seen meanwhile are replayed past the folded length. A failed commit makes the copy fold the
//! log again. Declared from main.rs as `#[path = "state/host.rs"] mod statehost;`, started with `statehost::start(hub.clone())`.
use crate::api::{Caller, Fut, RpcHandler, Ticker, Transform};
use crate::hub::Hub;
use crate::state::fam::policy::{Core, Policy};
use serde_json::Value;
use std::sync::{Arc, OnceLock};

struct Host { policy: Arc<Policy>, hub: OnceLock<Arc<Hub>> }
impl Transform for Host {
    fn before_commit(&self, first_seq: u64, events: Vec<Value>) -> Vec<Value> {
        let (out, asks) = self.policy.before_commit_asks(first_seq, events);
        let asks = self.policy.summaries_to_ask(asks);
        if !asks.is_empty() {
            // Sum.out: the writer is asked for once the batch is written (a failed write only costs a summary the next due end asks for again)
            if let (Some(hub), Ok(rt)) = (self.hub.get(), tokio::runtime::Handle::try_current()) {
                for (id, input) in asks { rt.spawn(summarize(self.policy.clone(), hub.clone(), "thread", id, input)); }
            }
        }
        out
    }
    fn commit_failed(&self, first_seq: u64) {
        if self.policy.commit_failed(first_seq) {
            eprintln!("state policies: a commit failed (log line {first_seq}); folding the log again");
            if let Some(h) = self.hub.get() { tokio::spawn(boot_task(self.policy.clone(), h.clone(), crate::guard::Folding::begin())); }
        }
    }
}
impl Ticker for Host {
    fn tick<'a>(&'a self, hub: &'a Arc<Hub>, now_s: u64) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if self.policy.take_poisoned() { eprintln!("state policies: the policy lock was poisoned by a panic; folding the log again"); tokio::spawn(boot_task(self.policy.clone(), hub.clone(), crate::guard::Folding::begin())); }
            self.policy.tick(now_s)
        })
    }
}

fn state_path(store: &crate::store::Store) -> std::path::PathBuf { crate::ckpt::dir(store.path()).join("state.json") }
/// The saved fold (ckpt/state.json) and the number of log lines it covers, when it is of this log.
fn load_state(store: &crate::store::Store) -> Option<(crate::state::State, usize)> {
    #[derive(serde::Deserialize)] struct Snap { v: u64, #[serde(default)] stamp: u64, lines: usize, fp: u64, st: crate::state::State }
    let snap: Snap = serde_json::from_slice(&std::fs::read(state_path(store)).ok()?).ok()?;
    if snap.v != crate::ckpt::VERSION || snap.stamp != crate::ckpt::stamps::STATE || snap.lines > store.len() || snap.fp != crate::ckpt::fingerprint(store, snap.lines) { return None }
    Some((snap.st, snap.lines))
}
fn save_state(store: &crate::store::Store, st: &crate::state::State, lines: usize) {
    #[derive(serde::Serialize)] struct Snap<'a> { v: u64, stamp: u64, lines: usize, fp: u64, st: &'a crate::state::State }
    let r = serde_json::to_vec(&Snap { v: crate::ckpt::VERSION, stamp: crate::ckpt::stamps::STATE, lines, fp: crate::ckpt::fingerprint(store, lines), st }).map_err(std::io::Error::from).and_then(|b| crate::ckpt::write_atomic(&state_path(store), &[&b]));
    if let Err(e) = r { eprintln!("checkpoint: state not saved: {e}") }
}
/// Fold the log lines [from, len) into `core`, `len` read first.
fn fold_tail(store: &crate::store::Store, core: &mut Core, from: usize, len: usize) {
    let mut at = from;
    while at < len {
        let to = (at + 2000).min(len);
        match store.range(at, to) { Ok(ls) => for (i, l) in ls.iter().enumerate() { store.fold(at + i, l, |l| { core.fold_line(l); }); }, Err(e) => eprintln!("state policies: reading log lines {at}..{to} failed: {e}") }
        at = to;
    }
}
/// A saved fold that is far behind the log is brought forward and saved again (a lean fold, line by line, so it is exactly what a fold of the whole log gives).
/// Called by the checkpoint task; nothing when the saved fold is within `slack` lines of the log's end or there is none yet (the start saves that).
pub fn refresh_state(store: &crate::store::Store, slack: usize) {
    let len = store.len();
    let Some((st, lines)) = load_state(store) else { return };
    if len < lines + slack { return }
    let mut core = Core { st };
    fold_tail(store, &mut core, lines, len);
    save_state(store, &core.st, len);
}

/// Begin reading the saved fold now (the start joins it when the policies boot).
pub fn preload(store: &crate::store::Store) { let s = store.clone(); crate::ckpt::preload("state", store, move || load_state(&s)); }

/// the lean fold of the log, on a blocking thread, from the saved fold when there is one; the length is read first and the fold stops there
fn read_log(hub: &Hub) -> (Core, u64) {
    let len = hub.store.len();
    let (mut core, from) = match crate::ckpt::take("state", &hub.store, || load_state(&hub.store)) { Some((st, lines)) => { crate::startup::mark(&format!("policy checkpoint loaded ({lines} lines)")); (Core { st }, lines) } None => (Core::new(), 0) };
    fold_tail(&hub.store, &mut core, from, len);
    if from > 0 && crate::ckpt::check_on() {
        let (store, a) = (hub.store.clone(), serde_json::to_value(&core.st).unwrap_or_default());
        std::thread::spawn(move || { let mut c = Core::new(); fold_tail(&store, &mut c, 0, len); crate::ckpt::check("policy state", a, serde_json::to_value(&c.st).unwrap_or_default()) });
    }
    // a long fold is not repeated at the next start
    if len - from > 20_000 { save_state(&hub.store, &core.st, len); }
    (core, len as u64)
}

/// What other modules ask of the policies. `read` is Auto.read: the person opened a thread's newest reply (the view and desk-look
/// handlers call it, then commit the events it returns with their own).
pub struct Policies { policy: Arc<Policy> }
impl Policies {
    /// whether the policies have folded the log (diagnostics, tests)
    pub fn booted(&self) -> bool { self.policy.booted() }
    /// The star nudge's count for one accepted human prompt (see `Policy::star_prompt`): commit the events with the prompt, then `hub.push(client, frame)`
    /// the returned offer frame to that connection only. Call it at the accepted-input boundary of turn.start and bots.say / post / tell from a client
    /// (not for a retry, an agent's mail or a forwarded hub). Returns nothing until the policies have folded the log.
    pub fn star_prompt(&self, msg_id: &str, now_s: u64) -> (Vec<Value>, Option<Value>) { self.policy.star_prompt(msg_id, now_s) }
    /// The star offer every client shows while it is out and undecided (`star::offer`), null otherwise
    pub fn star_offer(&self) -> Value { self.policy.star_offer() }
    /// Borrow the existing lean fold; no state clone or second log projection. None during boot/resync.
    pub fn with_state<R>(&self, f: impl FnOnce(&crate::state::State) -> R) -> Option<R> { self.policy.with_state(f) }
    /// Watchdog.line: call for every provider line of a thread's agent (the stream-json line as parsed), commit the events it returns
    pub fn line(&self, thread: &str, now_s: u64, line: &Value) -> Vec<Value> { self.policy.line(thread, now_s, line) }
    /// Side-by-side mode: no tick, boot or summary action on its own (see `Policy::passive`)
    pub fn set_passive(&self, on: bool) { self.policy.set_passive(on) }
    pub async fn read(&self, thread: &str, now_s: u64) -> Vec<Value> { self.policy.read(thread, now_s) }
}

/// Sum.go: ask the text model (the agents lane's `Summarizer`) for a thread's or project's summary, keep its answer (`Policy::summary_done`) and, for a
/// thread whose project is due, ask for the project's next. Nothing when no summarizer is up or it answers nothing.
async fn summarize(policy: Arc<Policy>, hub: Arc<Hub>, kind: &'static str, id: String, input: String) {
    let (mut kind, mut id, mut input) = (kind, id, input);
    loop {
        let Some(sm) = crate::agents::summarizer() else { return };
        let Some(text) = sm.summarize(kind, &id, &input).await else { return };
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let (ev, next) = policy.summary_done(kind, &id, &text, now);
        if !ev.is_empty() && hub.commit_many(&ev).await.is_err() { return }
        match next { Some((p, i)) if kind == "thread" => { kind = "project"; id = p; input = i; } _ => return }
    }
}

async fn boot_task(policy: Arc<Policy>, hub: Arc<Hub>, _folding: crate::guard::Folding) {
    let h = hub.clone();
    let (core, len) = match tokio::task::spawn_blocking(move || read_log(&h)).await { Ok(x) => x, Err(e) => { eprintln!("state policies: the log fold failed, the policies stay off: {e}"); return } };
    let now_s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    crate::startup::mark("policy state folded");
    let derived = policy.finish_boot(core, len, now_s);
    crate::startup::mark("policy state booted");
    if !derived.is_empty() && !policy.passive() { let _ = hub.commit_many(&derived).await; }
}

/// star.nudge {token, action}: the person opened the suggestion or dismissed it (terminal; reliable, so the reply follows the commit); every client
/// is then told to hide it (`{"t":"star","offer":null}`). star.offer: `{offer}`, what a (re)connecting client shows (`star::offer`).
struct StarRpc { policy: Arc<Policy> }
impl RpcHandler for StarRpc {
    fn methods(&self) -> &'static [&'static str] { &["star.nudge", "star.offer", "star.replay"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, params: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            // Settings, Developer: offered again now, to every client (the other devices show it too)
            if method == "star.replay" {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                let ev = self.policy.star_replay(now);
                if ev.is_empty() { return vec![serde_json::json!({"t":"reply","id":id,"ok":false,"error":"the hub is still starting"})] }
                if hub.commit_many(&ev).await.is_err() { return vec![serde_json::json!({"t":"reply","id":id,"ok":false,"error":"store write failed"})] }
                hub.push_all(&crate::state::fam::star::frame(self.policy.star_offer()));
                return vec![serde_json::json!({"t":"reply","id":id,"ok":true})]
            }
            if method == "star.offer" { return vec![serde_json::json!({"t":"reply","id":id,"ok":true,"result":{"offer":self.policy.star_offer()}})] }
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let ev = self.policy.star_decide(params.get("token").and_then(|v| v.as_str()).unwrap_or(""), params.get("action").and_then(|v| v.as_str()).unwrap_or(""), now);
            if !ev.is_empty() && hub.commit_many(&ev).await.is_err() { return vec![serde_json::json!({"t":"reply","id":id,"ok":false,"error":"store write failed"})] }
            if !ev.is_empty() { hub.push_all(&crate::state::fam::star::frame(Value::Null)); }
            vec![serde_json::json!({"t":"reply","id":id,"ok":true})]
        })
    }
}

static POLICIES: OnceLock<Arc<Policies>> = OnceLock::new();
/// The 0.14 import (`Hub::import_lines`) writes history no transform sees. `refold_begin` before its first line holds the policies' batches;
/// `refold_end` after its last folds the whole log again, as a start does. True from begin when this call owns the fold.
static FOLDS: std::sync::Mutex<Vec<(std::path::PathBuf, std::sync::Weak<Policy>)>> = std::sync::Mutex::new(Vec::new());
fn policy_of(hub: &Hub) -> Option<Arc<Policy>> { FOLDS.lock().unwrap().iter().find(|(h, _)| h == &hub.cfg.home).and_then(|(_, w)| w.upgrade()) }
/// Lines the 0.14 import wrote, into the policies' state (the views read it), with nothing derived: `Policy::fold_quiet`.
pub fn fold_quiet(hub: &Hub, first_seq: u64, events: Vec<serde_json::Value>) -> u64 { policy_of(hub).map_or(0, |p| p.fold_quiet(first_seq, events)) }
/// Whether the policies' start fold is over (an import that begins by itself waits for it).
pub fn booted(hub: &Hub) -> bool { policy_of(hub).map_or(true, |p| p.with_state(|_| ()).is_some()) }
pub fn refold_begin(hub: &Hub) -> bool { policy_of(hub).map_or(false, |p| p.begin_refold()) }
pub fn refold_end(hub: &Arc<Hub>, owned: bool) { if !owned { return } if let Some(p) = policy_of(hub) { tokio::spawn(boot_task(p, hub.clone(), crate::guard::Folding::begin())); } }
/// The policies, once `start` has run: for actors that cannot be handed them (agents: `Policies::line` per provider line, `read` on a view open)
pub fn policies() -> Option<Arc<Policies>> { POLICIES.get().cloned() }

pub fn start(hub: Arc<Hub>) -> Arc<Policies> { start_with(hub, None) }
/// `start`, with side-by-side mode decided by the caller (`Some(true)` passive, `Some(false)` active); `None` leaves the env (`BACKPLANE_PASSIVE`) to decide
pub fn start_with(hub: Arc<Hub>, passive: Option<bool>) -> Arc<Policies> {
    let policy = Arc::new(Policy::new());
    if let Some(on) = passive { policy.set_passive(on) }
    policy.set_no_outward(crate::copy::on());
    let host = Arc::new(Host { policy: policy.clone(), hub: OnceLock::new() });
    { let mut f = FOLDS.lock().unwrap(); f.retain(|(h, w)| w.strong_count() > 0 && h != &hub.cfg.home); f.push((hub.cfg.home.clone(), Arc::downgrade(&policy))); }
    let _ = host.hub.set(hub.clone());
    hub.register_transform(host.clone());   // first: the log length is read after this, so no batch is missed
    hub.register_rpc(Arc::new(StarRpc { policy: policy.clone() }));
    hub.register_ticker(host);
    tokio::spawn(boot_task(policy.clone(), hub, crate::guard::Folding::begin()));
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

    #[tokio::test] async fn a_passive_start_commits_nothing_on_a_legacy_home() {
        let dir = std::env::temp_dir().join(format!("bp-statehost-passive-{}", std::process::id()));
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
        let p = start_with(hub.clone(), Some(true));
        let mut text = String::new();
        for _ in 0..100 { tokio::time::sleep(std::time::Duration::from_millis(50)).await; if p.booted() { break } }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        text.push_str(&hub.store.range(0, hub.store.len()).unwrap().join("\n"));
        assert!(p.booted(), "the policies folded the log");
        assert!(!text.contains("autosettle.delivery.") && !text.contains("autosettle.version"), "passive: the boot commits nothing, the upgrade included: {}", text.lines().filter(|l| l.contains("autosettle")).collect::<Vec<_>>().join("\n"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // The records the JS hub writes next to a message — liveness.<t> and autosettle.quiet.<t> — are written by the Rust hub through a real Hub start:
    // once the policies have folded the log, a user and an assistant message are committed and the log shows both records, in the same batch.
    #[tokio::test] async fn a_real_hub_writes_liveness_and_quiet_next_to_a_message() {
        let dir = std::env::temp_dir().join(format!("bp-statehost-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        let cfg = Cfg { home: dir.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let p = start(hub.clone());
        for _ in 0..200 { if p.booted() { break } tokio::time::sleep(std::time::Duration::from_millis(25)).await; }
        assert!(p.booted(), "the policies folded the log");
        hub.commit_many(&[json!({"$":"ProjectCreated","id":"p1","title":"p","root":"/r","at":1000}),
            json!({"$":"ThreadCreated","id":"t1","project":"p1","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1000})]).await.unwrap();
        let n0 = hub.store.len();
        hub.commit_many(&[json!({"$":"MessagePosted","thread":"t1","msg":"mu1","role":"user","text":"hi","at":1001})]).await.unwrap();
        hub.commit_many(&[json!({"$":"MessagePosted","thread":"t1","msg":"ma1","role":"assistant","text":"yo","at":1002})]).await.unwrap();
        let text = hub.store.range(n0, hub.store.len()).unwrap().join("\n");
        for want in [r#""key":"autosettle.quiet.t1""#, r#""key":"liveness.t1""#, r#""key":"autosettle.audience.t1""#, r#""key":"autosettle.output.t1""#] { assert!(text.contains(want), "{want} missing in:\n{text}"); }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

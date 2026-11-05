//! The policy core: one lean copy of the fold and the decisions made on it, owned by a single task. The hub's Observer only sends it each
//! committed batch (a channel send, never a lock on the writer's path); the Ticker asks it for the tick's events through the same queue; what
//! it derives it commits in order through one sink. No mutex: batches, ticks and commits are serialized by the task.
//! Pure of the hub (the sink is a closure), so it is tested here with a recording sink.
use super::fam::autosettle;
use super::State;
use serde_json::Value;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::{mpsc, oneshot};

/// Settings the lean copy keeps: what the policies read (autosettle, liveness, wakes, locks, launches, the star ledger, told/alert/summary
/// records) and nothing that grows with the log for its own sake (`stopped.<msg>`, `failed.<msg>`, token totals, uber op lines, session and
/// seen counts...). A policy that starts reading another family adds its prefix here.
const KEEP: &[&str] = &["settle.", "autosettle.", "launch.", "launched.", "thread.parent.", "bg.", "watch.", "order.wait.", "wake.", "wakes.", "lock.", "locks.",
    "secret.ask.", "liveness.", "star.nudge.", "restart.retry", "provider.resume.", "watchdog.", "alert.", "told.", "summary.", "notify.", "push.webhook"];
pub fn lean_keeps(k: &str) -> bool { KEEP.iter().any(|p| k.starts_with(p)) }

pub struct Core {
    pub st: State,
    /// log lines folded so far (1-based count): a batch already inside it is skipped
    applied: u64,
    /// events this core derived and folded already, waiting to come back through the log (skipped once, in order)
    expect: VecDeque<Value>,
}
impl Core {
    pub fn new() -> Core { let mut st = State::new(); st.lean = true; Core { st, applied: 0, expect: VecDeque::new() } }
    pub fn fold_line(&mut self, l: &str) { self.st.apply_line(l); self.applied += 1; }
    /// fold a batch the hub committed and return the events the policies derive from it (already folded here)
    pub fn batch(&mut self, first_seq: u64, events: &[Value]) -> Vec<Value> {
        if events.is_empty() { return vec![] }
        let end = first_seq + events.len() as u64 - 1;
        if end <= self.applied { return vec![] }
        let skip = self.applied.saturating_sub(first_seq.saturating_sub(1)) as usize;
        let events = &events[skip.min(events.len())..];
        let quiet = autosettle::quiet(&self.st, events);
        let mut derived = vec![];
        for e in events {
            if self.expect.front() == Some(e) { self.expect.pop_front(); continue }
            let meta = autosettle::metadata(&self.st, e);
            self.st.apply(e);
            for m in &meta { self.st.apply(m); }
            derived.extend(meta);
        }
        for q in &quiet { self.st.apply(q); }
        derived.extend(quiet);
        self.applied = end;
        for d in &derived { self.expect.push_back(d.clone()); }
        derived
    }
    /// a commit of derived events failed: they will not come back, so stop waiting for them
    pub fn failed(&mut self, derived: &[Value]) { for d in derived { if let Some(i) = self.expect.iter().position(|x| x == d) { self.expect.remove(i); } } }
    pub fn tick(&self, now_s: u64) -> Vec<Value> { autosettle::changes(&self.st, now_s) }
}

pub enum Msg { Batch(u64, Vec<Value>), Tick(u64, oneshot::Sender<Vec<Value>>), Read(String, u64, oneshot::Sender<Vec<Value>>), #[cfg(test)] Waiting(oneshot::Sender<usize>) }
pub type Sink = Box<dyn FnMut(Vec<Value>) -> Pin<Box<dyn Future<Output = bool> + Send>> + Send>;

/// The task: handle messages in arrival order; commit what a batch derives before the next message is looked at.
pub async fn run(mut rx: mpsc::UnboundedReceiver<Msg>, mut core: Core, mut sink: Sink) {
    while let Some(m) = rx.recv().await {
        match m {
            Msg::Batch(seq, evs) => {
                let derived = core.batch(seq, &evs);
                if !derived.is_empty() && !sink(derived.clone()).await { core.failed(&derived); }
            }
            Msg::Tick(now, tx) => { let _ = tx.send(core.tick(now)); }
            Msg::Read(th, now, tx) => { let _ = tx.send(autosettle::read(&core.st, &th, now)); }
            #[cfg(test)] Msg::Waiting(tx) => { let _ = tx.send(core.expect.len()); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    fn msg(th: &str, id: &str, role: &str, at: u64) -> Value { json!({"$":"MessagePosted","thread":th,"msg":id,"role":role,"text":"hello","at":at}) }

    #[test] fn lean_keeps_what_policies_read_and_drops_what_grows() {
        for k in ["autosettle.quiet.t1", "liveness.t1", "wake.w1", "lock.merge", "launch.t2", "told.k1", "settle.done"] { assert!(lean_keeps(k), "{k}"); }
        for k in ["stopped.m1", "failed.m1", "tok.t.t1", "uber.op.u.m.1", "usage.t1", "todos.t1", "session.t1.claude"] { assert!(!lean_keeps(k), "{k}"); }
    }
    #[test] fn an_ordered_commit_of_two_back_to_back_batches() {
        // the first batch's commit is slow, the second's fast: the log must still get the first's events first, and every echo is skipped in turn
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
        rt.block_on(async {
            let (tx, rx) = mpsc::unbounded_channel();
            let log: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(vec![]));
            let (l2, tx2) = (log.clone(), tx.clone());
            let sink: Sink = Box::new(move |evs: Vec<Value>| {
                let (l, tx) = (l2.clone(), tx2.clone());
                Box::pin(async move {
                    let slow = evs.iter().any(|e| e["value"].as_str() == Some("a1"));
                    if slow { tokio::time::sleep(std::time::Duration::from_millis(60)).await; }
                    let first = { let mut g = l.lock().unwrap(); g.extend(evs.iter().cloned()); g.len() as u64 - evs.len() as u64 + 1 };
                    let _ = tx.send(Msg::Batch(first, evs)); // the observer's echo of our own commit
                    true
                })
            });
            let mut core = Core::new();
            for l in [r#"{"$":"ProjectCreated","id":"p","title":"p","root":"/r","at":1}"#, r#"{"$":"ThreadCreated","id":"t1","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}"#] {
                core.fold_line(l); log.lock().unwrap().push(serde_json::from_str(l).unwrap());
            }
            // the hub commits both messages at once, as two back-to-back batches
            let (a1, a2) = (msg("t1", "a1", "assistant", 10), msg("t1", "a2", "assistant", 11));
            log.lock().unwrap().push(a1.clone()); log.lock().unwrap().push(a2.clone());
            let h = tokio::spawn(run(rx, core, sink));
            tx.send(Msg::Batch(3, vec![a1])).unwrap();
            tx.send(Msg::Batch(4, vec![a2])).unwrap();
            let (ttx, trx) = oneshot::channel(); tx.send(Msg::Tick(20, ttx)).unwrap(); let _ = trx.await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let (wtx, wrx) = oneshot::channel(); tx.send(Msg::Waiting(wtx)).unwrap();
            assert_eq!(wrx.await.unwrap(), 0, "every echo of a derived event was matched in order and skipped");
            h.abort();
            let outs: Vec<String> = log.lock().unwrap().iter().filter(|e| e["key"].as_str().map_or(false, |k| k.starts_with("autosettle.output."))).map(|e| e["value"].as_str().unwrap().to_string()).collect();
            assert_eq!(outs, vec!["a1".to_string(), "a2".to_string()], "output marks reach the log in the order of their batches");
        });
    }
    #[test] fn a_batch_already_in_the_folded_log_is_skipped() {
        let mut c = Core::new();
        c.fold_line(r#"{"$":"ProjectCreated","id":"p","title":"p","root":"/r","at":1}"#);
        c.fold_line(r#"{"$":"ThreadCreated","id":"t1","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}"#);
        assert!(c.batch(1, &[json!({"$":"ProjectCreated","id":"p","title":"p","root":"/r","at":1})]).is_empty());
        assert_eq!(c.batch(2, &[json!({"$":"ThreadCreated","id":"t1","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}), msg("t1", "a1", "assistant", 5)]).len(), 2, "only the message past the folded log counts: output + quiet");
    }
    #[test] fn a_failed_commit_stops_the_wait_for_its_echo() {
        let mut c = Core::new();
        let d = c.batch(1, &[msg("t1", "a1", "assistant", 5)]); assert!(!d.is_empty());
        c.failed(&d);
        assert!(c.expect.is_empty());
    }
}

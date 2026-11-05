//! The policy core: one lean copy of the fold and the decisions made on it. It advances inside the hub's `Transform::before_commit` (on the
//! writer, after the secret mask, in commit order), so it is always at the log's head and what a batch derives (quiet marks, message
//! metadata) joins that batch. The Ticker and `Auto.read` take the same short lock. `before_commit` does no IO and no await: it folds and decides.
//! Pure of the hub, so it is tested here.
use super::autosettle;
use super::json::gnat;
use super::watchdog;
use crate::state::State;
use serde_json::Value;
use std::sync::Mutex;

/// Settings the lean copy keeps: what the policies read (autosettle, liveness, wakes, locks, launches, the star ledger, told/alert/summary
/// records) and nothing that grows with the log for its own sake (`stopped.<msg>`, `failed.<msg>`, token totals, uber op lines, session and
/// seen counts...). A policy that starts reading another family adds its prefix here.
const KEEP: &[&str] = &["settle.", "autosettle.", "launch.", "launched.", "thread.parent.", "bg.", "watch.", "order.wait.", "wake.", "wakes.", "lock.", "locks.",
    "secret.ask.", "liveness.", "star.nudge.", "restart.retry", "provider.resume.", "thread.config.hold.", "watchdog.", "alert.", "summary.", "notify.", "push.webhook"];
// `told.` is a ledger that grows with the log until the minute tick prunes it (Told.prune: 30 days, 2000 records); it is kept again when the told policy
// is ported with that prune, not before.
pub fn lean_keeps(k: &str) -> bool { KEEP.iter().any(|p| k.starts_with(p)) }

pub struct Core { pub st: State }
impl Core {
    pub fn new() -> Core { let mut st = State::new(); st.lean = true; Core { st } }
    pub fn fold_line(&mut self, l: &str) { self.st.apply_line(l); }
    /// fold a batch and return the events the policies derive from it (already folded here), quiet marks first and then the message metadata:
    /// the marks are judged against the state before the batch, the metadata against the state before each change (Auto.messages.metadata)
    pub fn derive(&mut self, events: &[Value]) -> Vec<Value> {
        // all judged against the state before the batch, as Bend's Hub.step does: liveness progress (Watchdog.after), then quiet marks
        let now = events.iter().map(|e| gnat(e, "at", 0)).max().unwrap_or(0);
        let live = watchdog::after(&self.st, now, events);
        let quiet = autosettle::quiet(&self.st, events);
        let mut meta = vec![];
        for e in events {
            let m = autosettle::metadata(&self.st, e); // against the state before each change (Auto.messages.metadata)
            self.st.apply(e);
            for x in &m { self.st.apply(x); }
            meta.extend(m);
        }
        for x in live.iter().chain(&quiet) { self.st.apply(x); }
        // the log order is Bend's: the batch, the liveness records, the quiet marks, then the message metadata
        live.into_iter().chain(quiet).chain(meta).collect()
    }
    pub fn tick(&self, now_s: u64) -> Vec<Value> { autosettle::changes(&self.st, now_s) }
}

struct Inner {
    core: Core,
    /// while the log is being folded (at start, and again after a failed commit): the batches `before_commit` saw, with their first line
    booting: Option<Vec<(u64, Vec<Value>)>>,
}
/// The policies behind one short lock. `before_commit` is the writer's call; `tick` and `read` are the ticker's and the viewers'.
pub struct Policy { inner: Mutex<Inner> }
impl Policy {
    pub fn new() -> Policy { Policy { inner: Mutex::new(Inner { core: Core::new(), booting: Some(vec![]) }) } }
    /// The batch about to be written: the batch plus what the policies derive from it. While the log is being folded the batch is
    /// remembered (and returned as it came); `finish_boot` derives for it afterwards.
    pub fn before_commit(&self, first_seq: u64, mut events: Vec<Value>) -> Vec<Value> {
        let mut g = self.inner.lock().unwrap();
        if let Some(buf) = g.booting.as_mut() { buf.push((first_seq, events.clone())); return events }
        let extra = g.core.derive(&events);
        events.extend(extra);
        events
    }
    /// A batch returned by `before_commit` could not be written. The copy now holds marks the log lacks: fold the log again. True when the
    /// caller must start that fold (`begin_resync` was needed).
    pub fn commit_failed(&self, first_seq: u64) -> bool {
        let mut g = self.inner.lock().unwrap();
        if let Some(buf) = g.booting.as_mut() { buf.retain(|(s, _)| *s < first_seq); return false } // the writer calls this once with the batch's base: nothing at or past it was written
        g.booting = Some(vec![]);
        true
    }
    /// The log through line `len` was folded into `core` (read AFTER the transform was registered): install it, replay the batches seen
    /// since past `len`, and return the events they derive (to be committed by the caller).
    pub fn finish_boot(&self, core: Core, len: u64) -> Vec<Value> {
        let mut g = self.inner.lock().unwrap();
        g.core = core;
        let held = g.booting.take().unwrap_or_default();
        let mut derived = vec![];
        for (seq, evs) in held {
            if evs.is_empty() || seq + evs.len() as u64 - 1 <= len { continue } // already inside the folded log
            derived.extend(g.core.derive(&evs));
        }
        derived
    }
    pub fn tick(&self, now_s: u64) -> Vec<Value> {
        let g = self.inner.lock().unwrap();
        if g.booting.is_some() { vec![] } else { g.core.tick(now_s) }
    }
    pub fn read(&self, thread: &str, now_s: u64) -> Vec<Value> {
        let g = self.inner.lock().unwrap();
        if g.booting.is_some() { vec![] } else { autosettle::read(&g.core.st, thread, now_s) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn msg(th: &str, id: &str, role: &str, at: u64) -> Value { json!({"$":"MessagePosted","thread":th,"msg":id,"role":role,"text":"hello","at":at}) }
    const P: &str = r#"{"$":"ProjectCreated","id":"p","title":"p","root":"/r","at":1}"#;
    const T: &str = r#"{"$":"ThreadCreated","id":"t1","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}"#;
    fn booted() -> Policy { let p = Policy::new(); let mut c = Core::new(); c.fold_line(P); c.fold_line(T); assert!(p.finish_boot(c, 2).is_empty()); p }
    fn keys(evs: &[Value]) -> Vec<String> { evs.iter().filter_map(|e| e["key"].as_str().map(String::from)).collect() }

    #[test] fn lean_keeps_what_policies_read_and_drops_what_grows() {
        for k in ["autosettle.quiet.t1", "liveness.t1", "wake.w1", "lock.merge", "launch.t2", "alert.t1", "settle.done"] { assert!(lean_keeps(k), "{k}"); }
        for k in ["stopped.m1", "failed.m1", "told.k1", "tok.t.t1", "uber.op.u.m.1", "usage.t1", "todos.t1", "session.t1.claude"] { assert!(!lean_keeps(k), "{k}"); }
    }
    #[test] fn a_batch_gets_its_derived_events_in_the_same_batch() {
        let p = booted();
        let out = p.before_commit(3, vec![msg("t1", "a1", "assistant", 10)]);
        assert_eq!(out.len(), 4, "the message, then the policies' events");
        assert_eq!(out[0]["msg"], "a1");
        assert_eq!(keys(&out), vec!["liveness.t1".to_string(), "autosettle.quiet.t1".to_string(), "autosettle.output.t1".to_string()], "Bend's order: the batch, the liveness record, the quiet marks, then the metadata");
    }
    #[test] fn two_back_to_back_batches_derive_in_order_from_the_state_the_first_left() {
        let p = booted();
        let a = p.before_commit(3, vec![msg("t1", "a1", "assistant", 10)]);
        let b = p.before_commit(6, vec![msg("t1", "a2", "assistant", 11)]);
        assert_eq!(a.iter().find(|e| e["key"].as_str() == Some("autosettle.output.t1")).unwrap()["value"], "a1");
        assert_eq!(b.iter().find(|e| e["key"].as_str() == Some("autosettle.output.t1")).unwrap()["value"], "a2");
        // the second batch's quiet mark is judged against the first's (10 -> 11), so it is written; a repeat at 11 is not
        assert!(keys(&b).contains(&"autosettle.quiet.t1".to_string()));
        let c = p.before_commit(9, vec![msg("t1", "a3", "assistant", 11)]);
        assert!(!keys(&c).contains(&"autosettle.quiet.t1".to_string()));
    }
    #[test] fn batches_seen_while_booting_are_replayed_past_the_folded_log() {
        let p = Policy::new();
        // lines 1-2 are in the log the boot read (len 2); batches at 3 and 4 were transformed meanwhile, one of them already inside len
        assert_eq!(p.before_commit(2, vec![msg("t1", "inside", "assistant", 5)]).len(), 1, "returned as it came while booting");
        assert_eq!(p.before_commit(3, vec![msg("t1", "a1", "assistant", 10)]).len(), 1);
        assert!(p.tick(100).is_empty() && p.read("t1", 100).is_empty(), "nothing is decided while booting");
        let mut c = Core::new(); c.fold_line(P); c.fold_line(T);
        let derived = p.finish_boot(c, 2);
        assert_eq!(keys(&derived), vec!["liveness.t1".to_string(), "autosettle.quiet.t1".to_string(), "autosettle.output.t1".to_string()], "only the batch past line 2 derives");
        let again = p.before_commit(4, vec![msg("t1", "a2", "assistant", 11)]);
        assert_eq!(again.len(), 4, "after boot the batch carries its own events");
    }
    #[test] fn a_failed_commit_asks_for_a_resync_and_buffers_until_it_is_done() {
        let p = booted();
        let _ = p.before_commit(3, vec![msg("t1", "a1", "assistant", 10)]);
        assert!(p.commit_failed(3), "the copy holds marks the log lacks: fold again");
        assert_eq!(p.before_commit(3, vec![msg("t1", "a2", "assistant", 12)]).len(), 1, "buffered while resyncing");
        assert!(!p.commit_failed(3), "a failed batch seen while resyncing just leaves the buffer");
        let mut c = Core::new(); c.fold_line(P); c.fold_line(T);
        assert!(p.finish_boot(c, 2).is_empty(), "the failed batch is gone from the buffer");
    }
    #[test] fn a_failed_batch_with_several_buffered_commits_forgets_all_of_them() {
        // the writer's batch held three commits (seq 3, 4, 5) and calls commit_failed once with its base, 3; an earlier commit (2) was written
        let p = Policy::new();
        for (s, id) in [(2u64, "w"), (3, "x"), (4, "y"), (5, "z")] { let _ = p.before_commit(s, vec![msg("t1", id, "assistant", 10 + s)]); }
        assert!(!p.commit_failed(3), "still booting: just forget");
        let mut c = Core::new(); c.fold_line(P); c.fold_line(T);
        let derived = p.finish_boot(c, 1);
        let outs: Vec<&str> = derived.iter().filter(|e| e["key"].as_str() == Some("autosettle.output.t1")).map(|e| e["value"].as_str().unwrap()).collect();
        assert_eq!(outs, vec!["w"], "only the commit before the failed base is replayed; x, y, z were never written");
    }
}

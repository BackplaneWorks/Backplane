//! The policy core: one lean copy of the fold and the decisions made on it. It advances inside the hub's `Transform::before_commit` (on the
//! writer, after the secret mask, in commit order), so it is always at the log's head and what a batch derives (quiet marks, message
//! metadata) joins that batch. The Ticker and `Auto.read` take the same short lock. `before_commit` does no IO and no await: it folds and decides.
//! Pure of the hub, so it is tested here.
use super::autosettle;
use super::json::gnat;
use super::turnend;
use super::watchdog;
use crate::state::State;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

/// Settings the lean copy keeps: what the policies read (autosettle, liveness, wakes, locks, launches, the star ledger, told/alert/summary
/// records) and nothing that grows with the log for its own sake (`stopped.<msg>`, `failed.<msg>`, token totals, uber op lines, session and
/// seen counts...). A policy that starts reading another family adds its prefix here.
const KEEP: &[&str] = &["settle.", "autosettle.", "launch.", "launched.", "thread.parent.", "bg.", "watch.", "order.wait.", "wake.", "wakes.", "lock.", "locks.",
    "secret.ask.", "liveness.", "star.nudge.", "restart.retry", "provider.resume.", "thread.config.hold.", "watchdog.", "alert.", "summary.", "notify.", "push.webhook"];
// `told.` is a ledger that grows with the log until the minute tick prunes it (Told.prune: 30 days, 2000 records); it is kept again when the told policy
// is ported with that prune, not before.
pub fn lean_keeps(k: &str) -> bool { KEEP.iter().any(|p| k.starts_with(p)) }

/// What `Core::derive` makes of a batch, in log order (see there)
pub struct Derived { pub core: Vec<Value>, pub end: Vec<Value>, pub marks: Vec<Value>, pub after: Vec<Value> }
pub struct Core { pub st: State }
impl Core {
    pub fn new() -> Core { let mut st = State::new(); st.lean = true; Core { st } }
    pub fn fold_line(&mut self, l: &str) { self.st.apply_line(l); }
    /// fold a batch and return what the policies add to it. Judged against the state before the batch, as Bend's Hub.step does. The parts are in
    /// Bend's log order: `core` (the batch without a failed end's marks), `end` (the summary clock and alert record of a marked end), `marks` (that
    /// failed end's `failed.<msg>`, which Bend writes after the alert), then `after`: liveness records, told marks, quiet marks, message metadata.
    pub fn derive(&mut self, events: &[Value]) -> Derived {
        let now = events.iter().map(|e| gnat(e, "at", 0)).max().unwrap_or(0);
        let (core, marks) = turnend::split_fail_marks(events);
        let live = watchdog::after(&self.st, now, events);
        let quiet = autosettle::quiet(&self.st, events);
        let ended: Vec<String> = turnend::ended_threads(&core).into_iter().map(String::from).collect();
        let ctx = turnend::told_ctx(&self.st, &core, &ended);
        let seq = self.st.seq + core.len() as u64; // Alert.seq(post): the changes applied after the step's own, a failed end's mark not among them
        let pre_running: Vec<(String, bool)> = ended.iter().map(|t| (t.clone(), turnend::was_running(&self.st, t))).collect();
        let pre_due: Vec<(String, bool)> = ended.iter().map(|t| (t.clone(), turnend::sum_due(&self.st, t, now))).collect();
        let mut meta = vec![];
        for e in &core {
            let m = autosettle::metadata(&self.st, e); // against the state before each change (Auto.messages.metadata)
            self.st.apply(e);
            for x in &m { self.st.apply(x); }
            meta.extend(m);
        }
        let end = turnend::ended_events_from(&self.st, &pre_running, &pre_due, seq, now, &core);
        let mut list: Vec<Value> = core.clone();
        list.extend(end.iter().cloned()); list.extend(marks.iter().cloned()); list.extend(live.iter().cloned());
        let told = turnend::told_marks(&ctx, &list);
        for x in end.iter().chain(&marks).chain(&live).chain(&told).chain(&quiet) { self.st.apply(x); }
        let after = live.into_iter().chain(told).chain(quiet).chain(meta).collect();
        Derived { core, end, marks, after }
    }
    pub fn tick(&self, now_s: u64) -> Vec<Value> { autosettle::changes(&self.st, now_s) }
}

struct Inner {
    core: Core,
    /// while the log is being folded (at start, and again after a failed commit): the batches `before_commit` saw, with their first line
    booting: Option<Vec<(u64, Vec<Value>)>>,
}
/// The policies behind one short lock. `before_commit` is the writer's call; `tick` and `read` are the ticker's and the viewers'.
pub struct Policy { inner: Mutex<Inner>, poisoned: AtomicBool }
impl Policy {
    pub fn new() -> Policy { Policy { inner: Mutex::new(Inner { core: Core::new(), booting: Some(vec![]) }), poisoned: AtomicBool::new(false) } }
    /// The lock, even after a panic inside it: the copy may be half-folded then, so it is flagged and the host folds the log again
    /// (`take_poisoned`); until that finishes the policies decide nothing, and a later batch never panics on the poisoned lock.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| {
            self.poisoned.store(true, Ordering::SeqCst);
            self.inner.clear_poison(); // the flag, not the lock, remembers it from here
            let mut g = p.into_inner(); g.booting = Some(vec![]); g
        })
    }
    /// true once after the lock was found poisoned: the caller starts the fold of the log again
    pub fn take_poisoned(&self) -> bool { self.poisoned.swap(false, Ordering::SeqCst) }
    /// The batch about to be written: the batch plus what the policies derive from it. While the log is being folded the batch is
    /// remembered (and returned as it came); `finish_boot` derives for it afterwards.
    pub fn before_commit(&self, first_seq: u64, mut events: Vec<Value>) -> Vec<Value> {
        let mut g = self.lock();
        if let Some(buf) = g.booting.as_mut() { buf.push((first_seq, events.clone())); turnend::strip_marks(&mut events); return events }
        let d = g.core.derive(&events);
        let mut out: Vec<Value> = d.core;
        out.extend(d.end); out.extend(d.marks); out.extend(d.after);
        turnend::strip_marks(&mut out);
        out
    }
    /// A batch returned by `before_commit` could not be written. The copy now holds marks the log lacks: fold the log again. True when the
    /// caller must start that fold (`begin_resync` was needed).
    pub fn commit_failed(&self, first_seq: u64) -> bool {
        let mut g = self.lock();
        if let Some(buf) = g.booting.as_mut() { buf.retain(|(s, _)| *s < first_seq); return false } // the writer calls this once with the batch's base: nothing at or past it was written
        g.booting = Some(vec![]);
        true
    }
    /// The log through line `len` was folded into `core` (read AFTER the transform was registered): install it, replay the batches seen
    /// since past `len`, and return the events they derive (to be committed by the caller).
    pub fn finish_boot(&self, core: Core, len: u64) -> Vec<Value> {
        let mut g = self.lock();
        g.core = core;
        let held = g.booting.take().unwrap_or_default();
        let mut derived = vec![];
        for (seq, evs) in held {
            if evs.is_empty() || seq + evs.len() as u64 - 1 <= len { continue } // already inside the folded log
            let d = g.core.derive(&evs);
            derived.extend(d.end); derived.extend(d.after); // the batch itself is already in the log, and so are its marks
        }
        derived
    }
    pub fn tick(&self, now_s: u64) -> Vec<Value> {
        let g = self.lock();
        if g.booting.is_some() { vec![] } else { g.core.tick(now_s) }
    }
    pub fn read(&self, thread: &str, now_s: u64) -> Vec<Value> {
        let g = self.lock();
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
    #[test] fn a_turns_end_writes_the_clock_the_alert_and_the_told_mark_before_the_quiet_marks() {
        let p = Policy::new(); let mut c = Core::new();
        for l in [P, T, r#"{"$":"MessagePosted","thread":"t1","msg":"mu1","role":"user","text":"do it","at":5}"#,
                  r#"{"$":"MessagePosted","thread":"t1","msg":"ma1","role":"assistant","text":"Done.\nmore","at":6}"#, r#"{"$":"TurnChanged","thread":"t1","state":"running","at":7}"#] { c.fold_line(l); }
        assert!(p.finish_boot(c, 5).is_empty());
        let end = json!({"$":"TurnChanged","thread":"t1","state":"completed","_end":true,"at":100});
        let out = p.before_commit(6, vec![end, json!({"$":"UsageSet","thread":"t1","used":1,"window":2,"at":100})]);
        let ks = keys(&out);
        assert_eq!(ks[0..3].to_vec(), vec!["summary.at.t1".to_string(), "alert.t1".to_string(), "told.k100-5-3".to_string()], "the clock, the alert record, then its mark numbered by its place (3)");
        let alert = out.iter().find(|e| e["key"] == "alert.t1").unwrap()["value"].as_str().unwrap().to_string();
        assert_eq!(alert, "done\tturn-t1\tmu1\t100.7\t", "kind, key, the person's message it settled, <now>.<changes applied after the step: 5 + 2>");
        let told = out.iter().find(|e| e["key"].as_str().map_or(false, |k| k.starts_with("told."))).unwrap()["value"].as_str().unwrap().to_string();
        assert_eq!(told, r#"{"key":"turn-t1","text":"T: Done.","channel":"alert","bot":"","at":100}"#);
        // the same person message does not alert twice: the memory holds it
        let again = p.before_commit(10, vec![json!({"$":"TurnChanged","thread":"t1","state":"completed","_end":true,"at":200})]);
        assert!(!keys(&again).contains(&"alert.t1".to_string()));
    }
    fn ended_world() -> Policy {
        let p = Policy::new(); let mut c = Core::new();
        for l in [P, T, r#"{"$":"MessagePosted","thread":"t1","msg":"mu1","role":"user","text":"do it","at":5}"#,
                  r#"{"$":"MessagePosted","thread":"t1","msg":"ma1","role":"assistant","text":"Done.","at":6}"#, r#"{"$":"TurnChanged","thread":"t1","state":"running","at":7}"#] { c.fold_line(l); }
        assert!(p.finish_boot(c, 5).is_empty()); p
    }
    #[test] fn a_stop_and_a_spawn_failure_write_no_clock_and_no_alert() {
        // Bend does not run Ended.out for Retire.turn.f (Stop) or Hub.spawned.fail: their TurnChanged carries no end marker
        let p = ended_world();
        let stop = p.before_commit(6, vec![json!({"$":"TurnChanged","thread":"t1","state":"interrupted","at":100}), json!({"$":"SettingSet","key":"stopped.mu1","value":"1","at":100})]);
        assert!(!keys(&stop).iter().any(|k| k.starts_with("summary.at.") || k.starts_with("alert.") || k.starts_with("told.")), "{:?}", keys(&stop));
        let p = ended_world();
        let spawn = p.before_commit(6, vec![json!({"$":"ActivityLogged","thread":"t1","act":"g1","tone":"error","kind":"spawn","summary":"no cli","at":100}), json!({"$":"TurnChanged","thread":"t1","state":"failed","at":100}), json!({"$":"SettingSet","key":"failed.mu1","value":"1","at":100})]);
        assert!(!keys(&spawn).iter().any(|k| k.starts_with("summary.at.") || k.starts_with("alert.") || k.starts_with("told.")), "{:?}", keys(&spawn));
    }
    #[test] fn a_failed_end_has_bends_order_stamp_and_told_index_and_loses_its_marker() {
        let p = ended_world();
        let out = p.before_commit(6, vec![json!({"$":"ActivityLogged","thread":"t1","act":"g1","tone":"error","kind":"agent","summary":"boom","at":100}),
            json!({"$":"TurnChanged","thread":"t1","state":"failed","_end":true,"at":100}), json!({"$":"SettingSet","key":"failed.mu1","value":"1","at":100})]);
        let ks = keys(&out);
        assert_eq!(ks[0..4].to_vec(), vec!["summary.at.t1".to_string(), "alert.t1".to_string(), "failed.mu1".to_string(), "liveness.t1".to_string()], "the batch, the clock, the alert, THEN the failed mark (Stamp.fail.mark), then liveness");
        assert!(out.iter().all(|e| e.get("_end").is_none()), "the marker is not written");
        let alert = out.iter().find(|e| e["key"] == "alert.t1").unwrap()["value"].as_str().unwrap().to_string();
        assert_eq!(alert, "fail\tturn-t1\tmu1\t100.7\t", "stamp: 5 + the 2 events of the end, the mark not counted");
        let told = out.iter().find(|e| e["key"].as_str().map_or(false, |k| k.starts_with("told."))).unwrap();
        assert_eq!(told["key"], "told.k100-5-3", "the alert sits at index 3 of [act, turn, summary.at, alert]... as the list Bend marks");
    }
    #[test] fn a_poisoned_lock_does_not_panic_the_next_batch_and_asks_for_a_resync() {
        let p = std::sync::Arc::new(booted());
        let q = p.clone();
        let _ = std::thread::spawn(move || { let _g = q.inner.lock().unwrap(); panic!("a panic inside the policy"); }).join();
        let out = p.before_commit(3, vec![json!({"$":"TurnChanged","thread":"t1","state":"completed","_end":true,"at":9})]);
        assert_eq!(out.len(), 1, "the batch goes through as it came, with no records and no marker");
        assert!(out[0].get("_end").is_none(), "the marker is stripped even then");
        assert!(p.take_poisoned(), "the host is told to fold the log again"); assert!(!p.take_poisoned());
        assert!(p.tick(10).is_empty(), "nothing is decided until the fold is done");
    }
}

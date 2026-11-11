//! The policy core: one lean copy of the fold and the decisions made on it. It advances inside the hub's `Transform::before_commit` (on the
//! writer, after the secret mask, in commit order), so it is always at the log's head and what a batch derives (quiet marks, message
//! metadata) joins that batch. The Ticker and `Auto.read` take the same short lock. `before_commit` does no IO and no await: it folds and decides.
//! Pure of the hub, so it is tested here.
use super::autosettle;
use super::json::gnat;
use super::summary;
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
    "secret.ask.", "liveness.", "star.nudge.", "restart.retry", "provider.resume.", "thread.config.hold.", "watchdog.", "alert.", "summary.", "notify.", "push.webhook", "told.", "project.dormant.",
    "order.enabled", "order.estimates", "account.thread.", "allowance.", "sidebar.pin.", "sidebar.fold.", "sidebar.archived.", "sidebar.group.", "sidebar.hide.", "sidebar.looked.", "sidebar.cut.", "auth.wait.", "carry.wait.", "accounts.inventory", "fork.", "usage.", "effort.", "buy."]; // fork.<id>: the source thread id, a thread screen's "back" button; the action dispatcher's facts (Fab.at / Fab.want, Accounts.named): tiny values, one account key per thread
// A thread screen must see its refusal gate's configuration even in the lean copy. Exact global keys keep their values;
// per-thread keys keep only presence, never the possibly large body. stopped./failed. remain outside this allowlist.
const GATE_EXACT: &[&str] = &["files.max.mb", "person.tz", "person.tz.device", "tz.machine", "think", "agent.thinking", "permissions.default"];
const GATE_PRESENCE: &[&str] = &["effort:", "wait.", "stop.", "fail.", "stopped.", "failed."]; // stopped./failed.<msg>: how a turn ended, for the reply stamps (one per stopped or failed turn, presence only)
pub fn gate_presence(k: &str) -> bool { GATE_PRESENCE.iter().any(|p| k.starts_with(p)) }
// `told.` is a ledger that grows with the log until the minute tick prunes it (Told.prune: 30 days, 2000 records, run by `Core::tick`): only
// its keys are kept (`lean_value`), the prune reads nothing else. A turn's input (launch.input.<t>, fallback.input.<t>: 10-20 KB each) is kept the
// same way, key only, for Input.prune (the minute tick removes those of threads at rest); a thread's riding mail (ride.) is never kept.
pub fn lean_keeps(k: &str) -> bool {
    if GATE_EXACT.contains(&k) || gate_presence(k) { return true }
    // the to-do panel reads the thread's list and whether it shows (small); the merge base (`todos.base.`) stays out
    if k.starts_with("todos.") && !k.starts_with("todos.base.") { return true }
    // The GPUI recent root and existing sidebar read only bounded navigation settings.
    // a home move (net/homemove.rs): the uberbot's row says where it is going, and a moved chat is drawn from its new home
    if ["home.moving", "home.at"].contains(&k) { return true }
    if ["ui.theme", "sidebar.view", "mail.on", "uber.bot", "order.enabled", "order.estimates", "import.v014.ack"].contains(&k)
        || ["thread.tree.", "project.icon.", "project.color.", "project.pick.", "project.link.", "buy.", "order.quote.", "mail.private.owner.", "account.thread."].iter().any(|p| k.starts_with(p)) { return true }
    // the Context panel's readings (present/tokstats.rs): one short string a thread, and the epoch that names the thread totals' key
    if ["tok.epoch", "tok.recount"].contains(&k) || ["turn.last.", "turn.speed.", "turn.models.", "turn.live."].iter().any(|p| k.starts_with(p)) || (k.starts_with("tok.") && [".t.", ".td.", ".tr.", ".bl."].iter().any(|p| k.starts_with(&format!("tok{p}")) || k.contains(p))) { return true }
    // the Settings pages' values (present/settings.rs): short strings, one per control
    if ["settle.idle.minutes", "settle.personal.minutes", "settle.done", "restart.continue", "thread.env", "agent.provider", "agent.model", "telemetry.share", "telemetry.usage.asked", "telemetry.asked", "telemetry.usage.generation", "telemetry.choice.revision", "telemetry.reset", "telemetry.notice", "onboard.usage", "update.channel", "text.provider", "text.model", "kicad.path", "freecad.path", "kicad.ipc", "viewer.board3d", "fallback.chain", "fallback.on", "fallback.add.provider", "fallback.add.model"].contains(&k) || ["push.webhook.", "notify.", "tz.rule."].iter().any(|p| k.starts_with(p)) { return true }
    // Settings, Voice (voice/page.rs, drawn by present/settings.rs from this copy): the engine, the model and the microphone, and the dictionary's
    // entries and learned fixes (a short line each, a removed one is gone, `blank_gone`). Without them every choice there looked unchosen (matrix S09).
    if ["voice.engine", "voice.model", "voice.device"].contains(&k) || k.starts_with("voice.e.") || k.starts_with("voice.l.") { return true }
    // Settings, Rules and task board: the person's standing rules and the task board's items (a few, each at most a few KB; a removed one is gone, `blank_gone`)
    if k.starts_with("rule.") || k.starts_with("board.") { return true }
    // the viewer's probe, one per board (present/viewer.rs `probe_action`: the tapped piece's info line, at most 2000 characters)
    if k.starts_with("probe:") { return true }
    // setup (present/onboard.rs): the step reached, the skipped and seen lists, done, and the last checks of this machine (a few KB, replaced each time)
    if k.starts_with("onboard.") || k == "freecad.path" { return true }
    k.starts_with("launch.input.") || k.starts_with("fallback.input.") || (!k.starts_with("ride.") && KEEP.iter().any(|p| k.starts_with(p)))
}
/// the value the lean copy keeps for a setting: a told record keeps a placeholder (its key carries the time), unless it is not one of ours
pub fn lean_value(k: &str, v: &str) -> String {
    if v.is_empty() { return String::new() } // an empty value removes the key, as for the full state
    // wait.<t>: the person's messages the agent has not read yet, a few ids apart by spaces; the thread view says "not read yet" under them (r05)
    if k.starts_with("wait.") && v.len() <= 2048 { return v.to_string() }
    if gate_presence(k) || k.starts_with("launch.input.") || k.starts_with("fallback.input.") { return "1".to_string() }
    match k.strip_prefix("told.") { Some(id) if turnend::told_stamp(id).is_some() => "1".to_string(), _ => v.to_string() }
}

/// What `Core::derive` makes of a batch, in log order (see there)
pub struct Derived { pub core: Vec<Value>, pub end: Vec<Value>, pub marks: Vec<Value>, pub after: Vec<Value>, pub asks: Vec<(String, String)> }
pub struct Core { pub st: State }
impl Core {
    pub fn new() -> Core { let mut st = State::new(); st.lean = true; Core { st } }
    pub fn fold_line(&mut self, l: &str) { self.st.apply_line(l); }
    /// fold a batch and return what the policies add to it. Judged against the state before the batch, as Hub.step does. The parts are in
    /// log order: `core` (the batch without a failed end's marks), `end` (the summary clock and alert record of a marked end), `marks` (that
    /// failed end's `failed.<msg>`, which writes after the alert), then `after`: liveness records, told marks, quiet marks, message metadata.
    pub fn derive(&mut self, events: &[Value]) -> Derived {
        let now = events.iter().map(|e| gnat(e, "at", 0)).max().unwrap_or(0);
        let (core, marks) = turnend::split_fail_marks(events);
        let live = watchdog::after(&self.st, now, events);
        let quiet = autosettle::quiet(&self.st, events);
        let reopen = autosettle::reopen(&self.st, events);
        let ended: Vec<String> = turnend::ended_threads(&core).into_iter().map(String::from).collect();
        let ctx = turnend::told_ctx(&self.st, &core, &ended);
        let seq_before = self.st.seq;
        let seq = seq_before + core.len() as u64; // Alert.seq(post): the changes applied after the step's own, a failed end's mark not among them
        let pre_running: Vec<(String, bool)> = ended.iter().map(|t| (t.clone(), turnend::was_running(&self.st, t))).collect();
        let pre_due: Vec<(String, bool)> = ended.iter().map(|t| (t.clone(), turnend::sum_due(&self.st, t, now))).collect();
        let mut meta = vec![];
        for e in &core {
            let mut m = autosettle::metadata(&self.st, e); // against the state before each change (Auto.messages.metadata)
            m.extend(crate::state::visible::meta(&self.st, e)); // a turn that starts ends a restart's cut mark (the sidebar's "Cut by a restart")
            self.st.apply(e);
            for x in &m { self.st.apply(x); }
            meta.extend(m);
        }
        let end = turnend::ended_events_from(&self.st, &pre_running, &pre_due, seq, now, &core);
        let mut list: Vec<Value> = core.clone();
        list.extend(end.iter().cloned()); list.extend(marks.iter().cloned()); list.extend(live.iter().cloned());
        // the asks the step opens get their alert records (Alerts.asks, over the state after the changes so far), then every record that alerts is marked
        for x in end.iter().chain(&marks).chain(&live) { self.st.apply(x); }
        let asks = turnend::alert_asks(&self.st, seq_before + list.len() as u64, &list);
        let mut all = list.clone(); all.extend(asks.iter().cloned());
        let told = turnend::told_marks(&ctx, &all);
        for x in asks.iter().chain(&told).chain(&quiet).chain(&reopen) { self.st.apply(x); }
        let after = live.into_iter().chain(asks).chain(told).chain(quiet).chain(meta).chain(reopen).collect();
        // Sum.out: a thread whose end stamped its summary clock is asked for, over the state after the whole step
        let asks = summary::asked(&end).into_iter().map(|id| { let input = summary::thread_input_of(&self.st, &id); (id, input) }).collect();
        Derived { core, end, marks, after, asks }
    }
    pub fn tick(&self, now_s: u64) -> Vec<Value> { let mut v = autosettle::changes(&self.st, now_s); v.extend(autosettle::tick_post(&self.st, now_s)); v.extend(turnend::told_prune(&self.st, now_s)); v.extend(turnend::input_prune(&self.st, now_s)); v }
}

struct Inner {
    core: Core,
    /// while the log is being folded (at start, and again after a failed commit): the batches `before_commit` saw, with their first line
    booting: Option<Vec<(u64, Vec<Value>)>>,
}
/// The policies behind one short lock. `before_commit` is the writer's call; `tick` and `read` are the ticker's and the viewers'.
pub struct Policy { inner: Mutex<Inner>, poisoned: AtomicBool, passive: AtomicBool, no_outward: AtomicBool }
impl Policy {
    pub fn new() -> Policy { Policy { inner: Mutex::new(Inner { core: Core::new(), booting: Some(vec![]) }), poisoned: AtomicBool::new(false), passive: AtomicBool::new(std::env::var("BACKPLANE_PASSIVE").map_or(false, |v| v == "1")), no_outward: AtomicBool::new(false) } }
    /// Read-only presentation over the same committed state the policies use. The closure must do no IO.
    pub fn with_state<R>(&self, f: impl FnOnce(&State) -> R) -> Option<R> {
        let g = self.lock();
        if g.booting.is_some() { None } else { Some(f(&g.core.st)) }
    }
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
    /// The star nudge's count (STARN Star.accept) for one accepted human prompt (the caller's accepted-input boundary: turn.start, bots.say/post/tell from a
    /// client, not a retry, not agent mail): the events to commit WITH the prompt, and the targeted offer frame when this prompt made it the fifth (sent). The
    /// caller commits first, then pushes the frame to that connection only.
    pub fn star_prompt(&self, msg_id: &str, now_s: u64) -> (Vec<Value>, Option<Value>) {
        let g = self.lock();
        if g.booting.is_some() { return (vec![], None) }
        let ev = super::star::accept(&g.core.st.get(super::star::KEY, ""), msg_id, now_s);
        let sent = ev.first().and_then(|e| e["value"].as_str()).map_or(false, |v| v.contains("\"status\":\"sent\""));
        let offer = if sent { ev.first().and_then(|e| e["value"].as_str()).map(|v| super::star::frame(super::star::offer(v))) } else { None };
        (ev, offer)
    }
    /// star.offer: the offer while it is out and undecided (null otherwise, and while the log is folded)
    pub fn star_offer(&self) -> Value {
        let g = self.lock();
        if g.booting.is_some() { Value::Null } else { super::star::offer(&g.core.st.get(super::star::KEY, "")) }
    }
    /// star.replay: the offer made again (Settings, Developer)
    pub fn star_replay(&self, now_s: u64) -> Vec<Value> { if self.lock().booting.is_some() { vec![] } else { super::star::replay(now_s) } }
    /// star.nudge {token, action}: the decision (opened / dismissed), terminal, only for the offered token
    pub fn star_decide(&self, token: &str, action: &str, now_s: u64) -> Vec<Value> {
        let g = self.lock();
        if g.booting.is_some() { vec![] } else { super::star::decide(&g.core.st.get(super::star::KEY, ""), token, action, now_s) }
    }
    /// whether the log has been folded: until then the policies decide nothing (and a batch is only remembered)
    pub fn booted(&self) -> bool { self.lock().booting.is_none() }
    pub fn take_poisoned(&self) -> bool { self.poisoned.swap(false, Ordering::SeqCst) }
    /// The batch about to be written: the batch plus what the policies derive from it. While the log is being folded the batch is
    /// remembered (and returned as it came); `finish_boot` derives for it afterwards.
    pub fn before_commit(&self, first_seq: u64, events: Vec<Value>) -> Vec<Value> { self.before_commit_asks(first_seq, events).0 }
    /// `before_commit`, and the summaries to ask the text model for once the batch is written: (thread, the writer's input)
    pub fn before_commit_asks(&self, first_seq: u64, mut events: Vec<Value>) -> (Vec<Value>, Vec<(String, String)>) {
        let mut g = self.lock();
        if let Some(buf) = g.booting.as_mut() { buf.push((first_seq, events.clone())); turnend::strip_marks(&mut events); return (events, vec![]) }
        let d = g.core.derive(&events);
        let mut out: Vec<Value> = d.core;
        out.extend(d.end); out.extend(d.marks); out.extend(d.after);
        turnend::strip_marks(&mut out);
        (out, d.asks)
    }
    /// Sum.got: the writer's answer for a thread (`kind` "thread") or project: the events to commit and, for a thread whose project is due, the
    /// project's summary to ask for next (project, input). Judged against the state now.
    pub fn summary_done(&self, kind: &str, id: &str, text: &str, now_s: u64) -> (Vec<Value>, Option<(String, String)>) {
        let mut g = self.lock();
        if g.booting.is_some() { return (vec![], None) }
        let (ev, next) = if kind == "project" { (summary::project_done(now_s, id, text), None) } else { summary::thread_done(&g.core.st, now_s, id, text) };
        for e in &ev { g.core.st.apply(e); }
        (ev, next.map(|n| (n.project, n.input)))
    }
    /// A batch returned by `before_commit` could not be written. The copy now holds marks the log lacks: fold the log again. True when the
    /// caller must start that fold (`begin_resync` was needed).
    pub fn commit_failed(&self, first_seq: u64) -> bool {
        let mut g = self.lock();
        if let Some(buf) = g.booting.as_mut() { buf.retain(|(s, _)| *s < first_seq); return false } // the writer calls this once with the batch's base: nothing at or past it was written
        g.booting = Some(vec![]);
        true
    }
    /// The 0.14 import wrote these lines (no transform saw them): they join the state the views read, and nothing is derived from them (they are history, not
    /// news). While the start's own fold runs they are kept for it like any batch.
    /// The lock is taken per 100 events, so a client's batch waits behind one slice, not the chunk. Answers the longest it held the lock (ms).
    pub fn fold_quiet(&self, first_seq: u64, events: Vec<Value>) -> u64 {
        { let mut g = self.lock(); if let Some(buf) = g.booting.as_mut() { buf.push((first_seq, events)); return 0 } }
        let mut most = 0;
        for slice in events.chunks(100) {
            let t = std::time::Instant::now(); let mut g = self.lock();
            if let Some(buf) = g.booting.as_mut() { buf.push((first_seq, slice.to_vec())); continue } // (a refold began meanwhile: it replays these)
            for e in slice { g.core.st.apply(e); }
            drop(g); most = most.max(t.elapsed().as_millis() as u64);
        }
        most
    }
    /// The 0.14 import is about to write lines no transform sees: hold every batch from now on, as a start does, until `finish_boot` folds the
    /// log again (`host::refold_end`). False when a fold is already pending (its own `finish_boot` covers the import too).
    pub fn begin_refold(&self) -> bool { let mut g = self.lock(); if g.booting.is_some() { return false } g.booting = Some(vec![]); true }
    /// The log through line `len` was folded into `core` (read AFTER the transform was registered): install it, replay the batches seen
    /// since past `len`, and return the events they derive (to be committed by the caller).
    pub fn finish_boot(&self, core: Core, len: u64, now_s: u64) -> Vec<Value> {
        let mut g = self.lock();
        g.core = core;
        let held = g.booting.take().unwrap_or_default();
        let mut derived = vec![];
        for (seq, evs) in held {
            if evs.is_empty() || seq + evs.len() as u64 - 1 <= len { continue } // already inside the folded log
            let d = g.core.derive(&evs);
            derived.extend(d.end); derived.extend(d.after); // the batch itself is already in the log, and so are its marks
        }
        // Hub.autosettle.upgrade (Hub.recover.v): once at start, after the log is folded; nothing at version 1. On a fresh home it sets the version.
        derived.extend(autosettle::upgrade(&g.core.st, now_s));
        derived
    }
    /// Side-by-side mode (env `BACKPLANE_PASSIVE=1`, or set by the switchover): nothing the policies do on their own happens. The minute tick writes
    /// nothing, the boot commits nothing (no Auto.upgrade), no summary is asked for. What a person's own action derives in the same batch still does.
    pub fn passive(&self) -> bool { self.passive.load(Ordering::SeqCst) }
    pub fn set_passive(&self, on: bool) { self.passive.store(on, Ordering::SeqCst) }
    /// A copy of a real log (`BACKPLANE_COPY=1`, set by the host from `crate::copy::on()`): nothing that leaves the machine (the summary model call)
    pub fn set_no_outward(&self, on: bool) { self.no_outward.store(on, Ordering::SeqCst) }
    /// the summaries to ask the text model for: none while passive, and none on a copy of a real log (`BACKPLANE_COPY=1`: the call leaves the machine)
    pub fn summaries_to_ask(&self, asks: Vec<(String, String)>) -> Vec<(String, String)> { if self.passive() || self.no_outward.load(Ordering::SeqCst) { vec![] } else { asks } }
    pub fn tick(&self, now_s: u64) -> Vec<Value> {
        if self.passive() { return vec![] }
        let g = self.lock();
        if g.booting.is_some() { vec![] } else { g.core.tick(now_s) }
    }
    /// Watchdog.line for one provider line of a thread's agent: the events that move its liveness record (to be committed by the caller)
    pub fn line(&self, thread: &str, now_s: u64, o: &Value) -> Vec<Value> {
        let g = self.lock();
        if g.booting.is_some() { vec![] } else { watchdog::line(&g.core.st, now_s, thread, o) }
    }
    pub fn read(&self, thread: &str, now_s: u64) -> Vec<Value> {
        let g = self.lock();
        if g.booting.is_some() { vec![] } else { let mut v = autosettle::read(&g.core.st, thread, now_s); v.extend(crate::state::visible::look(&g.core.st, thread, now_s)); v }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn msg(th: &str, id: &str, role: &str, at: u64) -> Value { json!({"$":"MessagePosted","thread":th,"msg":id,"role":role,"text":"hello","at":at}) }
    const P: &str = r#"{"$":"ProjectCreated","id":"p","title":"p","root":"/r","at":1}"#;
    const T: &str = r#"{"$":"ThreadCreated","id":"t1","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}"#;
    fn booted() -> Policy { let p = Policy::new(); let mut c = Core::new(); c.fold_line(P); c.fold_line(T); assert!(p.finish_boot(c, 2, 1000).iter().all(|e| e["key"] == "autosettle.version")); p }
    fn keys(evs: &[Value]) -> Vec<String> { evs.iter().filter_map(|e| e["key"].as_str().map(String::from)).collect() }

    #[test] fn lean_keeps_what_policies_read_and_drops_what_grows() {
        for k in ["sidebar.hide.L~t1", "sidebar.fold.k1", "todos.t1", "todos.visible.t1", "text.provider", "text.model", "kicad.path", "kicad.ipc", "fallback.chain", "fallback.on", "fallback.add.provider", "fallback.add.model", "autosettle.quiet.t1", "liveness.t1", "wake.w1", "lock.merge", "launch.t2", "alert.t1", "told.k1-1", "settle.done", "launch.input.t1", "fallback.input.t1", "order.enabled", "order.estimates", "account.thread.t1.claude", "files.max.mb", "person.tz", "tz.machine", "think", "permissions.default", "effort:t1", "wait.t1", "stop.t1", "fail.t1"] { assert!(lean_keeps(k), "{k}"); }
        for k in ["tz.rule.Europe/Helsinki", "tz.rule.Pacific/Auckland"] { assert!(lean_keeps(k), "{k}: the thread view reads the person's zone rule, without it every time stamp is dropped"); }
        for k in ["allowance.claude", "allowance.codex", "accounts.inventory"] { assert!(lean_keeps(k), "{k}: the composer's limits chip reads it"); }
        for k in ["rule.r1", "board.b1"] { assert!(lean_keeps(k), "{k}: Settings, Rules and task board lists it"); }
        for k in ["voice.engine", "voice.model", "voice.device", "voice.e.kicad", "voice.l.jason"] { assert!(lean_keeps(k), "{k}: Settings, Voice shows it"); }
        for k in ["tok.t.t1", "tok.1b.t.t1", "turn.last.t1", "turn.speed.t1", "turn.models.t1", "turn.live.t1", "tok.epoch", "tok.recount"] { assert!(lean_keeps(k), "{k}"); }
        for k in ["onboard.step", "onboard.skipped", "onboard.seen", "onboard.done", "onboard.checks", "onboard.usage", "freecad.path"] { assert!(lean_keeps(k), "{k}: setup reads it, and without it every press forgets the last"); }
        for k in ["ride.t1", "tok.basis.t1", "uber.op.u.m.1", "todos.base.t1", "session.t1.claude"] { assert!(!lean_keeps(k), "{k}"); }
    }
    #[test] fn lean_thread_screen_gate_preserves_exact_values_and_only_thread_key_presence() {
        let mut core=Core::new();
        for (key,value) in [("files.max.mb","10"),("person.tz","America/New_York"),("person.tz.device","Etc/UTC"),("tz.machine","Etc/UTC"),("think","off"),("permissions.default","ask")] {
            assert!(core.st.apply(&json!({"$":"SettingSet","key":key,"value":value,"at":1})));
            assert_eq!(core.st.settings.get(key).map(String::as_str),Some(value));
            assert!(!lean_keeps(&format!("{key}.unrelated")),"global config key is exact: {key}");
        }
        let large="x".repeat(20_000);
        for key in ["effort:t1","wait.t1","stop.t1","fail.t1"] {
            assert!(core.st.apply(&json!({"$":"SettingSet","key":key,"value":large,"at":2})));
            assert_eq!(core.st.settings.get(key).map(String::as_str),Some("1"),"{key}");
            assert!(core.st.apply(&json!({"$":"SettingSet","key":key,"value":"","at":3})));
            assert!(!core.st.settings.contains_key(key),"cleared gate key must disappear: {key}");
        }
        assert!(core.st.apply(&json!({"$":"SettingSet","key":"wait.t1","value":"m1 m2","at":4})));
        assert_eq!(core.st.settings.get("wait.t1").map(String::as_str),Some("m1 m2"),"the ids of the messages not read yet are kept: the thread view marks them");
        for key in ["stopped.m1","failed.m1"] {
            core.st.apply(&json!({"$":"SettingSet","key":key,"value":large,"at":4}));
            assert_eq!(core.st.settings.get(key).map(String::as_str),Some("1"),"per-message end marks keep presence only (the reply stamps read them): {key}");
        }
        // The full digest fold's empty-value semantics have not changed.
        let mut full=State::new(); full.apply(&json!({"$":"SettingSet","key":"stop.t1","value":"","at":1}));
        assert_eq!(full.settings.get("stop.t1").map(String::as_str),Some(""));
    }
    #[test] fn passive_mode_ticks_nothing_and_asks_for_no_summary() {
        let p = booted();
        // a turn input of an idle thread: the minute tick prunes it unless passive
        p.before_commit(3, vec![json!({"$":"SettingSet","key":"launch.input.t1","value":"x","at":1100})]);
        let ev = p.tick(2_000_000_000);
        assert!(keys(&ev).contains(&"launch.input.t1".to_string()), "active: the tick prunes the input: {ev:?}");
        p.set_passive(true);
        assert!(p.tick(2_000_000_000).is_empty(), "passive: the same tick writes nothing");
        let ask = vec![("t1".to_string(), "input".to_string())];
        assert_eq!(p.summaries_to_ask(ask.clone()), vec![], "passive: no summary asked for");
        p.set_passive(false);
        assert_eq!(p.summaries_to_ask(ask.clone()), ask);
    }
    #[test] fn a_batch_gets_its_derived_events_in_the_same_batch() {
        let p = booted();
        let out = p.before_commit(3, vec![msg("t1", "a1", "assistant", 10)]);
        assert_eq!(out.len(), 4, "the message, then the policies' events");
        assert_eq!(out[0]["msg"], "a1");
        assert_eq!(keys(&out), vec!["liveness.t1".to_string(), "autosettle.quiet.t1".to_string(), "autosettle.output.t1".to_string()], "order: the batch, the liveness record, the quiet marks, then the metadata");
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
        let derived = p.finish_boot(c, 2, 1000);
        assert_eq!(keys(&derived).into_iter().filter(|k| k != "autosettle.version").collect::<Vec<_>>(), vec!["liveness.t1".to_string(), "autosettle.quiet.t1".to_string(), "autosettle.output.t1".to_string()], "only the batch past line 2 derives");
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
        assert!(p.finish_boot(c, 2, 1000).iter().all(|e| e["key"] == "autosettle.version"), "the failed batch is gone from the buffer");
    }
    #[test] fn a_failed_batch_with_several_buffered_commits_forgets_all_of_them() {
        // the writer's batch held three commits (seq 3, 4, 5) and calls commit_failed once with its base, 3; an earlier commit (2) was written
        let p = Policy::new();
        for (s, id) in [(2u64, "w"), (3, "x"), (4, "y"), (5, "z")] { let _ = p.before_commit(s, vec![msg("t1", id, "assistant", 10 + s)]); }
        assert!(!p.commit_failed(3), "still booting: just forget");
        let mut c = Core::new(); c.fold_line(P); c.fold_line(T);
        let derived = p.finish_boot(c, 1, 1000);
        let outs: Vec<&str> = derived.iter().filter(|e| e["key"].as_str() == Some("autosettle.output.t1")).map(|e| e["value"].as_str().unwrap()).collect();
        assert_eq!(outs, vec!["w"], "only the commit before the failed base is replayed; x, y, z were never written");
    }
    #[test] fn a_turns_end_writes_the_clock_the_alert_and_the_told_mark_before_the_quiet_marks() {
        let p = Policy::new(); let mut c = Core::new();
        for l in [P, T, r#"{"$":"MessagePosted","thread":"t1","msg":"mu1","role":"user","text":"do it","at":5}"#,
                  r#"{"$":"MessagePosted","thread":"t1","msg":"ma1","role":"assistant","text":"Done.\nmore","at":6}"#, r#"{"$":"TurnChanged","thread":"t1","state":"running","at":7}"#] { c.fold_line(l); }
        assert!(p.finish_boot(c, 5, 1000).iter().all(|e| e["key"] == "autosettle.version"));
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
    // E2 #14: the reply and the turn's end in one batch: the told mark quotes that reply, not the one before it
    #[test] fn a_reply_that_lands_with_the_turns_end_is_the_one_told() {
        let p = Policy::new(); let mut c = Core::new();
        for l in [P, T, r#"{"$":"MessagePosted","thread":"t1","msg":"mu1","role":"user","text":"do it","at":5}"#,
                  r#"{"$":"MessagePosted","thread":"t1","msg":"ma1","role":"assistant","text":"Old reply.","at":6}"#,
                  r#"{"$":"MessagePosted","thread":"t1","msg":"mu2","role":"user","text":"again","at":7}"#, r#"{"$":"TurnChanged","thread":"t1","state":"running","at":8}"#] { c.fold_line(l); }
        assert!(p.finish_boot(c, 6, 1000).iter().all(|e| e["key"] == "autosettle.version"));
        let out = p.before_commit(7, vec![json!({"$":"MessagePosted","thread":"t1","msg":"ma2","role":"assistant","text":"one0 one1","at":100}), json!({"$":"TurnChanged","thread":"t1","state":"completed","_end":true,"at":100})]);
        let told = out.iter().find(|e| e["key"].as_str().map_or(false, |k| k.starts_with("told."))).unwrap()["value"].as_str().unwrap().to_string();
        assert!(told.contains("one0 one1") && !told.contains("Old reply"), "{told}");
    }
    fn ended_world() -> Policy {
        let p = Policy::new(); let mut c = Core::new();
        for l in [P, T, r#"{"$":"MessagePosted","thread":"t1","msg":"mu1","role":"user","text":"do it","at":5}"#,
                  r#"{"$":"MessagePosted","thread":"t1","msg":"ma1","role":"assistant","text":"Done.","at":6}"#, r#"{"$":"TurnChanged","thread":"t1","state":"running","at":7}"#] { c.fold_line(l); }
        assert!(p.finish_boot(c, 5, 1000).iter().all(|e| e["key"] == "autosettle.version")); p
    }
    #[test] fn a_stop_and_a_spawn_failure_write_no_clock_and_no_alert() {
        // does not run Ended.out for Retire.turn.f (Stop) or Hub.spawned.fail: their TurnChanged carries no end marker
        let p = ended_world();
        let stop = p.before_commit(6, vec![json!({"$":"TurnChanged","thread":"t1","state":"interrupted","at":100}), json!({"$":"SettingSet","key":"stopped.mu1","value":"1","at":100})]);
        assert!(!keys(&stop).iter().any(|k| k.starts_with("summary.at.") || k.starts_with("alert.") || k.starts_with("told.")), "{:?}", keys(&stop));
        let p = ended_world();
        let spawn = p.before_commit(6, vec![json!({"$":"ActivityLogged","thread":"t1","act":"g1","tone":"error","kind":"spawn","summary":"no cli","at":100}), json!({"$":"TurnChanged","thread":"t1","state":"failed","at":100}), json!({"$":"SettingSet","key":"failed.mu1","value":"1","at":100})]);
        assert!(!keys(&spawn).iter().any(|k| k.starts_with("summary.at.") || k.starts_with("alert.") || k.starts_with("told.")), "{:?}", keys(&spawn));
    }
    #[test] fn a_failed_end_has_order_stamp_and_told_index_and_loses_its_marker() {
        let p = ended_world();
        let out = p.before_commit(6, vec![json!({"$":"ActivityLogged","thread":"t1","act":"g1","tone":"error","kind":"agent","summary":"boom","at":100}),
            json!({"$":"TurnChanged","thread":"t1","state":"failed","_end":true,"at":100}), json!({"$":"SettingSet","key":"failed.mu1","value":"1","at":100})]);
        let ks = keys(&out);
        assert_eq!(ks[0..4].to_vec(), vec!["summary.at.t1".to_string(), "alert.t1".to_string(), "failed.mu1".to_string(), "liveness.t1".to_string()], "the batch, the clock, the alert, THEN the failed mark (Stamp.fail.mark), then liveness");
        assert!(out.iter().all(|e| e.get("_end").is_none()), "the marker is not written");
        let alert = out.iter().find(|e| e["key"] == "alert.t1").unwrap()["value"].as_str().unwrap().to_string();
        assert_eq!(alert, "fail\tturn-t1\tmu1\t100.7\t", "stamp: 5 + the 2 events of the end, the mark not counted");
        let told = out.iter().find(|e| e["key"].as_str().map_or(false, |k| k.starts_with("told."))).unwrap();
        assert_eq!(told["key"], "told.k100-5-3", "the alert sits at index 3 of [act, turn, summary.at, alert]... as the list marks");
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
    #[test] fn the_legacy_upgrade_runs_once_at_start_after_the_fold() {
        // a home below version 1 with a launched thread and its launch mail: finish_boot returns Auto.upgrade's events; at version 1 it returns none
        let p = Policy::new(); let mut c = Core::new();
        for l in [P, T, r#"{"$":"ThreadLaunched","id":"t1","by":"boss","report":true,"at":2}"#,
                  r#"{"$":"MailPosted","id":"t:t1:lr-1","conv":"c","from":"thread:t1","to":"thread:boss","kind":"launch","ref":"","want":"","head":"","size":0,"at":9}"#] { c.fold_line(l); }
        let up = p.finish_boot(c, 4, 5000);
        assert!(up.iter().any(|e| e["key"] == "autosettle.delivery.t1\tthread:boss" && e["value"] == "9"), "{:?}", keys(&up));
        assert_eq!(up.iter().filter(|e| e["key"] == "autosettle.version").count(), 1, "the version is set once");
        let p2 = Policy::new(); let mut c2 = Core::new();
        for l in [P, T, r#"{"$":"SettingSet","key":"autosettle.version","value":"1","at":2}"#] { c2.fold_line(l); }
        assert!(p2.finish_boot(c2, 3, 5000).is_empty(), "nothing at version 1");
        let p3 = Policy::new(); let c3 = Core::new();
        assert_eq!(keys(&p3.finish_boot(c3, 0, 5000)), vec!["autosettle.version".to_string()], "a fresh home gets the version, as Auto.upgrade does on an empty state");
    }
    #[test] fn the_fifth_human_prompt_commits_with_a_sent_ledger_and_offers_once() {
        let p = booted();
        let apply = |p: &Policy, evs: &[Value]| { for e in evs { let _ = p.before_commit(99, vec![e.clone()]); } };
        for i in 1..=4 { let (ev, offer) = p.star_prompt(&format!("m{i}"), 10 + i); assert_eq!(ev.len(), 1); assert!(offer.is_none()); apply(&p, &ev); }
        let (ev, offer) = p.star_prompt("m5", 20);
        assert!(ev[0]["value"].as_str().unwrap().contains(r#""status":"sent""#));
        assert_eq!(p.star_offer(), Value::Null, "not before the ledger commits");
        assert_eq!(offer.unwrap(), json!({"t":"star","offer":{"token":"m5","text":"Like Backplane? Give us a star on GitHub!","url":"https://github.com/BackplaneWorks/Backplane","label":"Star on GitHub"}}), "the targeted frame");
        apply(&p, &ev);
        assert_eq!(p.star_offer()["token"], "m5", "a reconnecting client asks and gets the same offer");
        assert_eq!(p.star_prompt("m6", 21), (vec![], None), "after the offer nothing more is counted or offered");
        let done = p.star_decide("m5", "open", 22); assert!(done[0]["value"].as_str().unwrap().contains("opened"));
        assert!(p.star_decide("stale", "open", 22).is_empty());
        apply(&p, &done); assert_eq!(p.star_offer(), Value::Null, "decided: no client shows it again");
    }
}

//! The command ledger (src/core/cmds.bend): which of the user lines a Claude process was sent it still owes an answer for, so a
//! turn ends when its own command does. Claude Code 2.1.286+ reports each command's `command_lifecycle` (queued, started, then
//! completed or cancelled), and a command's `result` comes BEFORE its lifecycle completes; a turn Claude starts by itself (a background
//! task's notification, a wakeup) has no lifecycle and no echo. An old CLI (no lifecycle) keeps the legacy rule: verdict 0.
//! A command's st: 0 accepted (not yet written), 1 sent (or queued by the CLI), 2 started, 3 resulted, 4 a tomb (the user stopped it).
//! Verdicts: 0 legacy, 1 quiet (a stopped or foreign turn's line), 2 stream (current work), 3 end ok, 4 end failed, 5 end stopped
//! (the CLI cancelled the command itself), 6 a wake turn starts. Laws cmds_*.
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Cmd { pub uuid: String, pub st: u8, pub err: bool, pub text: String }
/// mode: 0 not known yet, 1 lifecycle, 2 legacy
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cmds { pub cs: Vec<Cmd>, pub mode: u8, pub wake: bool }

fn s<'a>(o: &'a Value, k: &str) -> &'a str { o.get(k).and_then(Value::as_str).unwrap_or("") }
pub const MIN: &str = "2.1.286";

/// Cmds.lifecycle: whether a `claude_code_version` reports command lifecycles (digits read by hand: "286-beta" is 286).
pub fn lifecycle(v: &str) -> bool {
    let nums = |x: &str| -> Vec<u64> { x.trim().split('.').map(|p| p.chars().take_while(|c| c.is_ascii_digit()).fold(0u64, |a, c| a * 10 + c.to_digit(10).unwrap() as u64)).collect() };
    let (a, b) = (nums(v), nums(MIN)); let part = |n: &Vec<u64>, i: usize| n.get(i).copied().unwrap_or(0);
    for i in 0..3 { if part(&a, i) != part(&b, i) { return part(&a, i) > part(&b, i) } } true
}
fn is(o: &Value, t: &str, sub: &str) -> bool { s(o, "type") == t && s(o, "subtype") == sub }
fn is_lc(o: &Value) -> bool { s(o, "type") == "command_lifecycle" }
/// Cmds.kind: what a line does to the ledger: 0 lifecycle, 1 notification, 2 init, 3 result, 4 the interrupt's answer, 5 nothing.
pub fn kind(o: &Value) -> u8 { if is_lc(o) { 0 } else if is(o, "system", "task_notification") { 1 } else if is(o, "system", "init") { 2 } else if s(o, "type") == "result" { 3 } else if s(o, "type") == "control_response" { 4 } else { 5 } }
/// a line that can change which commands wait: a lifecycle, a result, the interrupt's answer
pub fn moves(o: &Value) -> bool { kind(o) < 5 }
fn lc_state(o: &Value) -> u8 { ["queued", "started", "completed", "cancelled"].iter().position(|x| *x == s(o, "state")).unwrap_or(4) as u8 }

impl Cmds {
    pub fn new() -> Cmds { Cmds::default() }
    fn outstanding(st: u8) -> bool { st < 4 }
    /// a command not yet answered (st 0..3), other than `but`
    pub fn open(&self, but: &str) -> bool { self.cs.iter().any(|c| Cmds::outstanding(c.st) && c.uuid != but) }
    pub fn started(&self) -> bool { self.cs.iter().any(|c| c.st == 2) }
    pub fn find(&self, u: &str) -> Option<&Cmd> { self.cs.iter().find(|c| c.uuid == u) }
    /// a command's st (5: not one of ours)
    pub fn state(&self, u: &str) -> u8 { self.find(u).map_or(5, |c| c.st) }
    pub fn has(&self, u: &str) -> bool { self.state(u) < 5 }
    fn flow(&self) -> u8 { if self.started() || self.wake { 2 } else { 1 } }

    fn mode_see(&self, o: &Value) -> Cmds {
        let mode = if is_lc(o) { 1 } else if is(o, "system", "init") && self.mode == 0 { if lifecycle(s(o, "claude_code_version")) { 1 } else { 2 } } else { self.mode };
        // an old CLI reports no lifecycle: the commands already written are forgotten (those being prepared, and tombs, stay)
        let cs = if mode == 2 { self.cs.iter().filter(|c| c.st == 0 || c.st >= 4).cloned().collect() } else { self.cs.clone() };
        Cmds { cs, mode, wake: self.wake }
    }
    fn end_v(done: bool, st: u8, others: bool, err: bool) -> u8 { if st >= 4 || others { 1 } else if done { if err { 4 } else { 3 } } else { 5 } }
    fn result_v(&self, err: bool) -> u8 { if self.started() { 1 } else if self.wake { if self.open("") { 1 } else if err { 4 } else { 3 } } else { 1 } }
    /// Cmds.verdict: what a line does to the turn (run: the thread's turn is running), before the line is seen.
    pub fn verdict(&self, run: bool, o: &Value) -> u8 {
        let c = self.mode_see(o); if c.mode != 1 { return 0 }
        match kind(o) {
            0 => { let u = s(o, "command_uuid"); match lc_state(o) {
                1 => if c.has(u) && !run { 6 } else { 2 },
                2 => Cmds::end_v(true, c.state(u), c.open(u) || c.wake, c.find(u).map_or(false, |x| x.err)),
                3 => Cmds::end_v(false, c.state(u), c.open(u), false),
                _ => 2 } }
            1 => if !run && !c.started() { 6 } else { c.flow() },
            2 => if !run && !c.open("") { 6 } else { c.flow() },
            3 => c.result_v(o.get("is_error").and_then(Value::as_bool).unwrap_or(false)),
            _ => c.flow(),
        }
    }
    /// An ending's text: the result line's own, or what the command's result said when its lifecycle completes.
    pub fn text(&self, o: &Value) -> String { if is_lc(o) { self.find(s(o, "command_uuid")).map(|c| c.text.clone()).unwrap_or_default() } else { s(o, "result").to_string() } }
    /// Cmds.see: a line seen.
    pub fn see(&self, o: &Value) -> Cmds {
        let mut c = self.mode_see(o);
        match kind(o) {
            0 => { let u = s(o, "command_uuid"); match lc_state(o) { 1 => for x in c.cs.iter_mut().filter(|x| x.uuid == u) { x.st = 2 }, 2 | 3 => c.cs.retain(|x| x.uuid != u), _ => {} } }
            1 => if !c.started() { c.wake = true },
            2 => if !c.open("") { c.wake = true },
            3 => if c.started() { let (err, text) = (o.get("is_error").and_then(Value::as_bool).unwrap_or(false), s(o, "result").to_string());
                    if let Some(x) = c.cs.iter_mut().find(|x| x.st == 2) { x.st = 3; x.err = err; x.text = text } } else { c.wake = false },
            4 => { let still: Vec<String> = o.get("response").and_then(|r| r.get("response")).and_then(|r| r.get("still_queued")).and_then(Value::as_array).map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
                   for x in c.cs.iter_mut().filter(|x| x.st == 4 && still.contains(&x.uuid)) { x.st = 1 } }
            _ => {}
        }
        c
    }
    /// the hub accepted a message
    pub fn accept(&self, u: &str) -> Cmds { let mut c = self.clone(); c.cs.push(Cmd { uuid: u.into(), st: 0, err: false, text: String::new() }); c }
    /// written to the process (an old CLI never answers it: forgotten)
    pub fn sent(&self, u: &str) -> Cmds { let mut c = self.clone(); if c.mode == 2 { c.cs.retain(|x| x.uuid != u) } else { for x in c.cs.iter_mut().filter(|x| x.uuid == u && x.st == 0) { x.st = 1 } } c }
    /// the commands written but never started (the CLI holds them queued)
    pub fn waiting(&self) -> Vec<String> { self.cs.iter().filter(|c| c.st == 1).map(|c| c.uuid.clone()).collect() }
    /// the commands the CLI holds behind another one it has not answered (or behind a turn it started by itself); oldest first
    pub fn unread(&self) -> Vec<String> { let mut ahead = self.wake; let mut out = vec![]; for c in &self.cs { if ahead && c.st == 1 { out.push(c.uuid.clone()) } ahead = ahead || Cmds::outstanding(c.st) } out }
    /// the user's Stop: every unanswered command not still queued is a tomb, and a wake turn is over
    pub fn stop(&self, still: &[String]) -> Cmds { let mut c = self.clone(); for x in c.cs.iter_mut() { if Cmds::outstanding(x.st) && !still.contains(&x.uuid) { x.st = 4 } } c.wake = false; c }
    /// Stuck turns: lifecycle mode, no wake turn, no command at work
    pub fn idle(&self) -> bool { self.mode == 1 && !self.wake && !self.cs.iter().any(|c| c.st < 3) }
    /// how such a turn ends, as its verdict would have said: 5 stopped, 4 failed, else 3 ok
    pub fn idle_end(&self) -> u8 { if self.cs.iter().any(|c| c.st == 4) { 5 } else if self.cs.iter().any(|c| c.st == 3 && c.err) { 4 } else { 3 } }
    /// Stuck.code.w: 0 nothing to do; else the verdict that ends a running turn whose ledger shows nothing at work after `wait` seconds of silence with no work out
    pub fn stuck(&self, run: bool, heard: u64, now: u64, busy: usize, wait: u64) -> u8 { if run && self.idle() && busy == 0 && now >= heard + wait { self.idle_end() } else { 0 } }
    pub fn cleared(&self) -> Cmds { Cmds { cs: vec![], mode: self.mode, wake: false } }
    /// Kept across a restart in place (cmds_json_round)
    pub fn json(&self) -> Value { serde_json::json!({"cs":self.cs.iter().map(|c| serde_json::json!({"u":c.uuid,"s":c.st,"e":c.err,"t":c.text})).collect::<Vec<_>>(),"m":self.mode,"w":self.wake}) }
    pub fn read(o: &Value) -> Cmds {
        Cmds { cs: o.get("cs").and_then(Value::as_array).map(|a| a.iter().map(|x| Cmd { uuid: s(x, "u").into(), st: x.get("s").and_then(Value::as_u64).unwrap_or(0) as u8, err: x.get("e").and_then(Value::as_bool).unwrap_or(false), text: s(x, "t").into() }).collect()).unwrap_or_default(),
            mode: o.get("m").and_then(Value::as_u64).unwrap_or(0) as u8, wake: o.get("w").and_then(Value::as_bool).unwrap_or(false) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn lc(u: &str, st: &str) -> Value { json!({"type":"command_lifecycle","command_uuid":u,"state":st}) }
    fn result(err: bool) -> Value { json!({"type":"result","is_error":err,"result":"ok"}) }
    fn init(v: &str) -> Value { json!({"type":"system","subtype":"init","claude_code_version":v}) }
    fn note() -> Value { json!({"type":"system","subtype":"task_notification","task_id":"b1","summary":"done"}) }
    fn one(u: &str, st: u8, wake: bool) -> Cmds { Cmds { cs: vec![Cmd { uuid: u.into(), st, err: false, text: String::new() }], mode: 1, wake } }
    fn two(a: u8, b: u8) -> Cmds { Cmds { cs: vec![Cmd { uuid: "u1".into(), st: a, err: false, text: String::new() }, Cmd { uuid: "u2".into(), st: b, err: false, text: String::new() }], mode: 1, wake: false } }

    #[test] fn versions_decide_whether_a_cli_reports_lifecycles() { assert!(lifecycle("2.1.286") && lifecycle("2.1.286-beta") && lifecycle("2.2.0") && lifecycle("3.0.0") && !lifecycle("2.1.285") && !lifecycle("2.0.9") && !lifecycle("")); }
    #[test] fn an_old_init_makes_the_ledger_legacy_and_forgets_written_commands() {
        let c = Cmds::new().accept("u1").sent("u1"); assert_eq!(c.verdict(true, &init("2.0.0")), 0);
        let s = c.see(&init("2.0.0")); assert_eq!(s.mode, 2); assert!(s.cs.is_empty()); assert_eq!(Cmds::new().accept("u2").see(&init("2.0.0")).cs.len(), 1, "one being prepared stays");
        assert_eq!(s.accept("u3").sent("u3").cs.len(), 0, "in legacy mode a command is forgotten once written");
    }
    #[test] fn a_command_ends_its_turn_when_its_lifecycle_completes() {
        let c = one("u1", 2, false); assert_eq!(c.verdict(true, &lc("u1", "completed")), 3);          // cmds_end_ok
        let mut f = c.clone(); f.cs[0].err = true; assert_eq!(f.verdict(true, &lc("u1", "completed")), 4);
        assert_eq!(c.verdict(true, &lc("u1", "cancelled")), 5);                                        // the CLI cancelled it itself: stopped
        assert_eq!(two(2, 1).verdict(true, &lc("u1", "completed")), 1);                                 // another command is outstanding: the thread goes on
    }
    #[test] fn a_result_while_a_command_runs_never_ends_the_thread() {
        assert_eq!(one("u1", 2, false).verdict(true, &result(false)), 1);
        let after = one("u1", 2, false).see(&result(true)); assert_eq!((after.cs[0].st, after.cs[0].err, after.cs[0].text.as_str()), (3, true, "ok"));
        assert_eq!(after.text(&lc("u1", "completed")), "ok"); assert_eq!(after.verdict(true, &lc("u1", "completed")), 4, "the lifecycle ends it, failed because its result was");
    }
    #[test] fn a_notification_or_an_init_with_nothing_outstanding_starts_a_wake_turn() {
        let c = Cmds { cs: vec![], mode: 1, wake: false };
        assert_eq!(c.verdict(false, &note()), 6); assert_eq!(c.verdict(false, &init("2.1.286")), 6); assert!(c.see(&note()).wake);
        assert_eq!(c.verdict(true, &note()), 1, "a running turn folds it in");
        let w = c.see(&note()); assert_eq!(w.verdict(true, &result(false)), 3, "its result ends it"); assert_eq!(w.verdict(true, &result(true)), 4);
        assert_eq!(Cmds { wake: true, ..two(1, 1) }.verdict(true, &result(false)), 1, "unless something is outstanding");
    }
    #[test] fn a_tomb_the_cli_starts_after_all_makes_the_display_follow_the_process() {
        let c = one("u1", 4, false); assert_eq!(c.verdict(false, &lc("u1", "started")), 6); assert_eq!(c.verdict(true, &lc("u1", "started")), 2); assert_eq!(c.verdict(true, &lc("u1", "completed")), 1, "a tomb's end is quiet");
    }
    #[test] fn stop_makes_tombs_of_what_is_unanswered_unless_the_cli_kept_it_queued() {
        let c = two(2, 1).stop(&[]); assert_eq!(c.cs.iter().map(|x| x.st).collect::<Vec<_>>(), [4, 4]);
        let back = c.see(&json!({"type":"control_response","response":{"response":{"still_queued":["u2"]}}})); assert_eq!(back.cs.iter().map(|x| x.st).collect::<Vec<_>>(), [4, 1]);
        assert_eq!(two(2, 1).stop(&["u2".to_string()]).cs[1].st, 1); assert!(!Cmds { wake: true, ..two(2, 1) }.stop(&[]).wake);
    }
    #[test] fn the_commands_behind_another_wait_unread() {
        assert_eq!(two(1, 1).unread(), ["u2"], "the oldest written is next: never listed"); assert_eq!(two(2, 1).unread(), ["u2"]); assert!(one("u1", 1, false).unread().is_empty());
        assert_eq!(Cmds { wake: true, ..one("u1", 1, false) }.unread(), ["u1"], "behind a wake turn every one waits"); assert_eq!(two(1, 1).waiting(), ["u1", "u2"]);
        assert!(moves(&lc("u", "started")) && moves(&result(false)) && !moves(&json!({"type":"assistant"})));
    }
    #[test] fn a_turn_whose_ledger_shows_nothing_at_work_is_stuck_and_ends_as_its_ending_would() {
        assert!(!one("u1", 2, false).idle() && one("u1", 3, false).idle() && !Cmds { wake: true, ..one("u1", 3, false) }.idle() && !Cmds::new().idle());
        let mut e = one("u1", 3, false); e.cs[0].err = true; assert_eq!(e.idle_end(), 4); assert_eq!(one("u1", 4, false).idle_end(), 5); assert_eq!(one("u1", 3, false).idle_end(), 3); assert!(e.cleared().cs.is_empty());
    }
    // law cmds_json_round
    #[test] fn cmds_json_round() { let c = Cmds { cs: vec![Cmd { uuid: "u1".into(), st: 2, err: false, text: String::new() }, Cmd { uuid: "u2".into(), st: 3, err: true, text: "it \"failed\"\n".into() }], mode: 1, wake: true }; assert_eq!(Cmds::read(&c.json()), c); }
}

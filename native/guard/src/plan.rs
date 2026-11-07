//! What to do after a hub exits. Pure: no clock, no files, so every step of the ladder has a test.
//!
//! The ladder: the same build again (with a pause), then the last build that started cleanly, then safe mode, then rescue (the guard itself answers on the hub's
//! port and keeps trying). A failure of the machine rather than of the build (port taken, disk full) never marks a build bad.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode { Normal, Safe }
impl Mode { pub fn name(self) -> &'static str { match self { Mode::Normal => "normal", Mode::Safe => "safe" } } }

/// How a hub ended.
#[derive(Clone, PartialEq, Debug)]
pub enum End {
    /// the guard stopped it on request: not a failure
    Asked,
    /// someone else ended it with a signal, or it left with 0: it comes back, and it does not count
    External,
    /// the machine, not the build (exit 75: port or home lock held, disk full)
    Machine(String),
    /// a refusal that a retry cannot change (exit 78: a copy gate, a bad flag)
    Refused(String),
    /// it died (a panic, a signal, out of memory), or it hung and was killed
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct Tuning { pub tries: usize, pub window: u64, pub pause: u64, pub machine_pause: u64, pub rescue_retry: u64 }
impl Default for Tuning { fn default() -> Tuning { Tuning { tries: 3, window: 600, pause: 1, machine_pause: 5, rescue_retry: 120 } } }

/// One failed start or run, for counting.
#[derive(Clone, Debug, PartialEq)]
pub struct Fail { pub at: u64, pub build: String, pub mode: Mode, pub machine: bool }

#[derive(Clone, PartialEq, Debug)]
pub enum Next {
    /// the same build and mode, after this many seconds
    Again { after: u64 },
    /// `from` failed `tries` times: mark it bad and run `to`
    Fallback { from: String, to: String },
    /// no build left to try: safe mode on this one
    Safe { build: String, bad: Option<String> },
    /// nothing starts: answer on the port and try again later
    Rescue { after: u64, why: String },
}

fn recent<'a>(hist: &'a [Fail], now: u64, window: u64) -> impl Iterator<Item = &'a Fail> { hist.iter().filter(move |f| now.saturating_sub(f.at) < window) }

/// The newest build known to start on this home that is not bad, is not `not`, and still exists (`good` is oldest first).
pub fn fallback<'a>(good: &'a [String], bad: &[String], not: &str, exists: &dyn Fn(&str) -> bool) -> Option<&'a String> {
    good.iter().rev().find(|b| b.as_str() != not && !bad.iter().any(|x| x == *b) && exists(b))
}

/// `hist` already holds the failure that just happened.
pub fn next(t: &Tuning, hist: &[Fail], end: &End, build: &str, mode: Mode, good: &[String], bad: &[String], exists: &dyn Fn(&str) -> bool, now: u64) -> Next {
    next_on(t, hist, end, build, mode, good, bad, exists, now, None)
}
/// `probation`: the build an update left, while the new one has not proved itself. The first failure goes straight back to it: no retries, and the update is marked bad.
pub fn next_on(t: &Tuning, hist: &[Fail], end: &End, build: &str, mode: Mode, good: &[String], bad: &[String], exists: &dyn Fn(&str) -> bool, now: u64, probation: Option<&str>) -> Next {
    if let (End::Failed(_), Some(back)) = (end, probation) { if exists(back) { return Next::Fallback { from: build.into(), to: back.into() } } }
    match end {
        End::Asked | End::External => Next::Again { after: 0 },
        End::Refused(why) => Next::Rescue { after: t.rescue_retry, why: why.clone() },
        End::Machine(why) => {
            let n = recent(hist, now, t.window).filter(|f| f.machine).count();
            if n >= t.tries { Next::Rescue { after: t.machine_pause * 6, why: why.clone() } } else { Next::Again { after: t.machine_pause } }
        }
        End::Failed(why) => {
            let n = recent(hist, now, t.window).filter(|f| !f.machine && f.build == build && f.mode == mode).count();
            if n < t.tries { return Next::Again { after: t.pause << (n.saturating_sub(1).min(5)) } }
            match mode {
                Mode::Normal => {
                    // another build failed here a moment ago as well: the fault is in the home, not in a build, so no third build is tried
                    let other = recent(hist, now, t.window).any(|f| !f.machine && f.build != build && f.mode == Mode::Normal);
                    match fallback(good, bad, build, exists) {
                        Some(to) if !other => Next::Fallback { from: build.into(), to: to.clone() },
                        _ => Next::Safe { build: build.into(), bad: None },
                    }
                }
                Mode::Safe => Next::Rescue { after: t.rescue_retry, why: why.clone() },
            }
        }
    }
}

/// An update on probation must answer as the build it was staged as (`EXPECT` in its folder, from the signed manifest): `Some(why)` when it does not. Fields left out of
/// EXPECT are not checked. EXPECT holds the full hash the manifest signed: the hub's commit (its short hash, or the full one) must be a prefix of it, of 7 or more
/// characters; never the other way round, or a short EXPECT would accept less than was signed.
pub fn identity_mismatch(expect: &serde_json::Value, said: &serde_json::Value) -> Option<String> {
    let s = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap_or("").trim().to_string();
    let (ev, ec, gv, gc) = (s(expect, "version"), s(expect, "commit"), s(said, "version"), s(said, "commit"));
    let commit_ok = ec.is_empty() || (!gc.is_empty() && gc != "unknown" && ec.starts_with(&gc) && gc.len() >= 7);
    let version_ok = ev.is_empty() || ev == gv;
    if commit_ok && version_ok { return None }
    let or = |x: &str| if x.is_empty() { "?".to_string() } else { x.to_string() };
    Some(format!("answered as {}/{}, expected {}/{}", or(&gv), or(&gc), or(&ev), or(&ec)))
}

/// A filesystem the hub writes to is nearly full: under `floor` (1 GiB), or under 5 % and under 10 GiB (a big disk with 90 GB left is not full). The temp folder counts as much as the home's disk: on a desktop where /tmp is a RAM
/// tmpfs shared by every agent, it fills first, and every agent's writes then fail (ENOSPC, EDQUOT) as they would on a full disk.
pub fn low_space(free: u64, total: u64, floor: u64) -> bool { free < floor || (free.saturating_mul(20) < total && free < 10 << 30) }
pub fn gb(n: u64) -> String { format!("{:.1} GB", n as f64 / 1e9) }

/// The parts an update came up without that the build before it had running (both from /healthz `degraded`). A part that was already down is the home's, not the update's.
pub fn lost_parts(before: &[String], now: &[String]) -> Vec<String> { now.iter().filter(|p| !before.contains(p)).cloned().collect() }

/// What a person or a client may ask for.
#[derive(Clone, PartialEq, Debug)]
pub enum Action { Restart, Safe, Rollback, Use(String), Update(String), Stop }
impl Action {
    pub fn parse(s: &str, build: &str) -> Option<Action> {
        match s { "restart" | "normal" => Some(Action::Restart), "safe" => Some(Action::Safe), "rollback" => Some(Action::Rollback), "stop" => Some(Action::Stop), "use" if !build.is_empty() => Some(Action::Use(build.into())), "update" if !build.is_empty() => Some(Action::Update(build.into())), _ => None }
    }
}

/// A build that can be reached again from far away: its hub has the control route. A remote rollback goes to such a build only, or the person could not come back.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate { pub build: String, pub control: bool }

/// Why a request from another machine or a phone is refused; None lets it through. A remote request can never stop the hub, can only roll back to a build that can be reached
/// again, and a client that keeps asking cannot hold the hub down. The person at the machine is not limited.
pub fn refuse_remote(a: &Action, recent_remote: &[u64], now: u64, target: Option<&Candidate>) -> Option<String> {
    if *a == Action::Stop { return Some("stopping the hub is only possible at its machine: nothing could bring it back from here".into()) }
    if recent_remote.iter().filter(|t| now.saturating_sub(**t) < 300).count() >= 3 { return Some("three restarts in five minutes from other devices: wait, or use `backplane hub` at the machine".into()) }
    if let Action::Rollback | Action::Use(_) | Action::Update(_) = a {
        match target { None => return Some("no build to go back to".into()), Some(c) if !c.control => return Some(format!("{} has no remote control: going back to it from here would lock you out", c.build)), _ => {} }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn f(at: u64, b: &str, m: Mode) -> Fail { Fail { at, build: b.into(), mode: m, machine: false } }
    fn all(_: &str) -> bool { true }
    fn none(_: &str) -> bool { false }
    fn s(x: &[&str]) -> Vec<String> { x.iter().map(|y| y.to_string()).collect() }
    const T: fn() -> Tuning = Tuning::default;
    fn died() -> End { End::Failed("panic".into()) }

    #[test] fn a_first_failure_tries_again_with_a_growing_pause() {
        assert_eq!(next(&T(), &[f(100, "b2", Mode::Normal)], &died(), "b2", Mode::Normal, &s(&["b1"]), &[], &all, 100), Next::Again { after: 1 });
        assert_eq!(next(&T(), &[f(99, "b2", Mode::Normal), f(100, "b2", Mode::Normal)], &died(), "b2", Mode::Normal, &s(&["b1"]), &[], &all, 100), Next::Again { after: 2 });
    }
    #[test] fn three_failures_fall_back_to_the_last_good_build() {
        let h = [f(98, "b2", Mode::Normal), f(99, "b2", Mode::Normal), f(100, "b2", Mode::Normal)];
        assert_eq!(next(&T(), &h, &died(), "b2", Mode::Normal, &s(&["b0", "b1"]), &[], &all, 100), Next::Fallback { from: "b2".into(), to: "b1".into() });
    }
    #[test] fn old_failures_do_not_count() {
        let h = [f(1, "b2", Mode::Normal), f(2, "b2", Mode::Normal), f(1000, "b2", Mode::Normal)];
        assert_eq!(next(&T(), &h, &died(), "b2", Mode::Normal, &s(&["b1"]), &[], &all, 1000), Next::Again { after: 1 });
    }
    #[test] fn a_fallback_skips_bad_gone_and_failing_builds() {
        assert_eq!(fallback(&s(&["a", "b", "c"]), &s(&["b"]), "c", &all), Some(&"a".to_string()), "c is the failing one, b is bad");
        assert_eq!(fallback(&s(&["a"]), &[], "c", &none), None, "pruned from disk");
        assert_eq!(fallback(&[], &[], "c", &all), None);
    }
    #[test] fn with_nothing_to_fall_back_to_it_goes_to_safe_mode() {
        let h = [f(98, "b", Mode::Normal), f(99, "b", Mode::Normal), f(100, "b", Mode::Normal)];
        assert_eq!(next(&T(), &h, &died(), "b", Mode::Normal, &[], &[], &all, 100), Next::Safe { build: "b".into(), bad: None });
    }
    #[test] fn when_the_fallback_fails_too_the_home_is_at_fault_and_safe_mode_follows() {
        // b2 failed, b1 (the last good build) failed the same way: no third build, safe mode on b1
        let h = [f(90, "b2", Mode::Normal), f(91, "b2", Mode::Normal), f(92, "b2", Mode::Normal), f(98, "b1", Mode::Normal), f(99, "b1", Mode::Normal), f(100, "b1", Mode::Normal)];
        assert_eq!(next(&T(), &h, &died(), "b1", Mode::Normal, &s(&["b0", "b1"]), &s(&["b2"]), &all, 100), Next::Safe { build: "b1".into(), bad: None });
    }
    #[test] fn a_safe_mode_that_fails_ends_in_rescue_and_never_gives_up() {
        let h = [f(98, "b", Mode::Safe), f(99, "b", Mode::Safe), f(100, "b", Mode::Safe)];
        assert!(matches!(next(&T(), &h, &died(), "b", Mode::Safe, &s(&["a"]), &[], &all, 100), Next::Rescue { after: 120, .. }));
    }
    #[test] fn the_machine_failing_never_blames_a_build() {
        let m = |at| Fail { at, build: "b".into(), mode: Mode::Normal, machine: true };
        let end = End::Machine("port 3797 is taken".into());
        assert_eq!(next(&T(), &[m(100)], &end, "b", Mode::Normal, &s(&["a"]), &[], &all, 100), Next::Again { after: 5 });
        match next(&T(), &[m(98), m(99), m(100)], &end, "b", Mode::Normal, &s(&["a"]), &[], &all, 100) { Next::Rescue { why, .. } => assert!(why.contains("3797")), o => panic!("{o:?}") }
    }
    #[test] fn a_refusal_is_not_retried_at_once_and_blames_nothing() {
        assert!(matches!(next(&T(), &[], &End::Refused("a copy".into()), "b", Mode::Normal, &s(&["a"]), &[], &all, 100), Next::Rescue { .. }));
    }
    #[test] fn a_requested_stop_is_no_failure() { assert_eq!(next(&T(), &[], &End::Asked, "b", Mode::Normal, &[], &[], &all, 1), Next::Again { after: 0 }); }

    #[test] fn an_update_on_probation_goes_back_at_the_first_failure() {
        let h = [f(100, "new", Mode::Normal)];
        assert_eq!(next_on(&T(), &h, &died(), "new", Mode::Normal, &s(&["old"]), &[], &all, 100, Some("old")), Next::Fallback { from: "new".into(), to: "old".into() });
        assert_eq!(next_on(&T(), &h, &End::Machine("disk".into()), "new", Mode::Normal, &s(&["old"]), &[], &all, 100, Some("old")), Next::Again { after: 5 }, "the machine failing proves nothing about the update");
        assert_eq!(next_on(&T(), &h, &died(), "new", Mode::Normal, &[], &[], &none, 100, Some("old")), Next::Again { after: 1 }, "the old build is gone: the normal ladder");
    }
    #[test] fn a_remote_request_cannot_stop_the_hub() { assert!(refuse_remote(&Action::Stop, &[], 100, None).unwrap().contains("only possible at its machine")); }
    #[test] fn a_remote_rollback_refuses_a_build_it_could_not_come_back_from() {
        let old = Candidate { build: "old".into(), control: false }; let new = Candidate { build: "new".into(), control: true };
        assert!(refuse_remote(&Action::Rollback, &[], 100, Some(&old)).unwrap().contains("lock you out"));
        assert!(refuse_remote(&Action::Rollback, &[], 100, None).is_some(), "nothing to go back to");
        assert_eq!(refuse_remote(&Action::Rollback, &[], 100, Some(&new)), None);
        assert!(refuse_remote(&Action::Use("old".into()), &[], 100, Some(&old)).is_some());
    }
    #[test] fn a_client_that_keeps_asking_cannot_hold_the_hub_down() {
        assert_eq!(refuse_remote(&Action::Restart, &[90, 95], 100, None), None);
        assert!(refuse_remote(&Action::Restart, &[90, 95, 99], 100, None).is_some());
        assert_eq!(refuse_remote(&Action::Restart, &[1, 2, 3], 1000, None), None, "the limit passes");
    }
    #[test] fn an_update_must_answer_as_the_build_it_was_staged_as() {
        use serde_json::json;
        let e = json!({"version": "0.15.0", "commit": "4369a55773"});
        assert_eq!(identity_mismatch(&e, &json!({"version": "0.15.0", "commit": "4369a55773"})), None);
        let full = json!({"version": "0.15.0", "commit": "4369a55773abcdef0123456789abcdef01234567"});
        assert_eq!(identity_mismatch(&full, &json!({"version": "0.15.0", "commit": "4369a55773"})), None, "the hub's short hash against the signed full one");
        assert_eq!(identity_mismatch(&full, &json!({"version": "0.15.0", "commit": "4369a55773abcdef0123456789abcdef01234567"})), None);
        assert!(identity_mismatch(&e, &json!({"version": "0.15.0", "commit": "4369a55773abcdef"})).is_some(), "a short EXPECT does not vouch for a longer commit");
        assert_eq!(identity_mismatch(&e, &json!({"version": "0.14.2", "commit": "4369a55773"})).unwrap(), "answered as 0.14.2/4369a55773, expected 0.15.0/4369a55773");
        assert!(identity_mismatch(&e, &json!({"version": "0.15.0", "commit": "unknown"})).is_some(), "a build that cannot say its commit is not proven");
        assert!(identity_mismatch(&e, &json!({"version": "0.15.0"})).is_some(), "a hub that did not say (no credential, an old build)");
        assert!(identity_mismatch(&e, &json!({"version": "0.15.0", "commit": "43"})).is_some(), "too short to mean anything");
        assert_eq!(identity_mismatch(&json!({"version": "0.15.0"}), &json!({"version": "0.15.0", "commit": "x"})), None, "only what EXPECT names is checked");
    }
    #[test] fn a_full_temp_folder_counts_as_a_full_disk() {
        let g = 1u64 << 30;
        assert!(low_space(g / 2, 2000 * g, g), "under 1 GiB, whatever the size");
        assert!(low_space(g * 14 / 10, 30 * g, g), "a 30 GB tmpfs with 1.4 GB left: under 5 %");
        assert!(!low_space(10 * g, 30 * g, g)); assert!(!low_space(90 * g, 1800 * g, g), "90 GB free of 1.8 TB is under 5 % and not low");
    }
    #[test] fn an_update_that_loses_a_part_fails_and_one_the_home_had_down_does_not() {
        assert_eq!(lost_parts(&[], &s(&["bots"])), s(&["bots"]), "bots panicked at start: the update goes back");
        assert!(lost_parts(&s(&["pairing"]), &s(&["pairing"])).is_empty(), "pairing was down before as well: the home's, not the update's");
        assert_eq!(lost_parts(&s(&["pairing"]), &s(&["pairing", "agents"])), s(&["agents"]));
        assert!(lost_parts(&s(&["bots"]), &[]).is_empty(), "coming up with more is fine");
    }
    #[test] fn actions_parse() {
        assert_eq!(Action::parse("restart", ""), Some(Action::Restart)); assert_eq!(Action::parse("normal", ""), Some(Action::Restart));
        assert_eq!(Action::parse("use", "b"), Some(Action::Use("b".into()))); assert_eq!(Action::parse("use", ""), None); assert_eq!(Action::parse("x", ""), None);
    }
}

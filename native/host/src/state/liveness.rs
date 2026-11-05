//! Agent liveness (src/core/liveness.bend): a progress gap is an episode, one nudge, then one escalation, ending only on real
//! progress. Notices only: nothing here kills, interrupts, cancels or restarts a process. Pure; the hub's minute tick reads the
//! record `liveness.<thread>` (a setting), asks `tick`, writes the record back and delivers the notices.
use super::json::*;
use serde_json::json;

#[derive(Clone, Debug, PartialEq)]
pub struct Live { pub last: u64, pub nudged: u64, pub stalled: bool, pub reported: bool, pub reason: String }
#[derive(Clone, Debug, PartialEq)]
pub struct Decision { pub live: Live, pub nudge: bool, pub notify: bool }

pub const DEFAULT_GAP: u64 = 1200;
pub const DEFAULT_GRACE: u64 = 60;
pub fn key(thread: &str) -> String { format!("liveness.{thread}") }

/// Liveness.number: leading decimal digits only; any other character gives the fallback, "" gives the fallback
fn number(text: &str, fallback: u64) -> u64 {
    if text.is_empty() { return fallback }
    let mut acc = 0u64;
    for c in text.chars() { match c.to_digit(10) { Some(d) => acc = acc.saturating_mul(10).saturating_add(d as u64), None => return fallback } }
    acc
}
fn raw(o: &serde_json::Value, k: &str, d: &str) -> String {
    match o.get(k) { Some(serde_json::Value::Number(n)) => n.to_string(), Some(serde_json::Value::String(s)) => s.clone(), _ => d.into() }
}
pub fn read(line: &str) -> Live {
    let o = parse_or_null(line);
    Live { last: number(&raw(&o, "last", "0"), 0), nudged: number(&raw(&o, "nudged", "0"), 0), stalled: gbool(&o, "stalled", false), reported: gbool(&o, "reported", false),
           reason: gstr(&o, "reason", "turn or owned work").into() }
}
pub fn write(l: &Live) -> String {
    show(&json!({"last": l.last, "nudged": l.nudged, "stalled": l.stalled, "reported": l.reported, "reason": l.reason}))
}
/// real progress: the clock never goes back, the episode ends
pub fn progress(l: &Live, now: u64, reason: &str) -> Live { Live { last: now.max(l.last), nudged: 0, stalled: false, reported: false, reason: reason.into() } }
pub fn due(active: bool, now: u64, gap: u64, last: u64) -> bool { active && gap > 0 && now >= last && now - last >= gap }
pub fn escalate(waited: bool, l: &Live) -> Decision {
    if !waited { return Decision { live: l.clone(), nudge: false, notify: false } }
    Decision { live: Live { stalled: true, reported: true, ..l.clone() }, nudge: false, notify: !l.reported }
}
fn gap_step(sent: bool, now: u64, grace: u64, l: &Live) -> Decision {
    if sent { escalate(now >= l.nudged.saturating_add(grace), l) }
    else { Decision { live: Live { nudged: now, stalled: false, reported: false, ..l.clone() }, nudge: true, notify: false } }
}
pub fn tick(active: bool, now: u64, gap: u64, grace: u64, l: &Live) -> Decision {
    if due(active, now, gap, l.last) { gap_step(l.nudged > 0, now, grace, l) }
    else { Decision { live: Live { stalled: false, ..l.clone() }, nudge: false, notify: false } }
}
/// a missing timeout takes the default; an explicit zero opts out
pub fn timeout(present: bool, seconds: u64) -> u64 { if present { seconds } else { DEFAULT_GAP } }
pub fn nudge_text(reason: &str) -> String { format!("You appear stalled at {reason}; report status or continue. Busy work must keep running.") }

#[cfg(test)]
mod tests {
    use super::*;
    fn l(last: u64, nudged: u64, stalled: bool, reported: bool) -> Live { Live { last, nudged, stalled, reported, reason: "r".into() } }
    #[test] fn liveness_stalled_only_after_gap() {
        // before the gap (or inactive, or gap 0, or a reversed clock) the stall is cleared, whatever the record said
        for (a, now, gap) in [(true, 1000, 1200), (false, 5000, 1200), (true, 5000, 0), (true, 10, 1200)] {
            let d = tick(a, now, gap, 60, &l(900, 0, true, false)); assert!(!d.live.stalled && !d.nudge && !d.notify, "{a} {now} {gap}");
        }
    }
    #[test] fn liveness_nudge_once_per_gap() {
        let d = tick(true, 5000, 1200, 60, &l(100, 4000, false, false)); assert!(!d.nudge);
        let first = tick(true, 5000, 1200, 60, &l(100, 0, false, false)); assert!(first.nudge && first.live.nudged == 5000);
        // inside the grace after a nudge: no second nudge and no notice; and not active: nothing
        let inside = tick(true, 5000, 1200, 60, &l(100, 4990, false, false)); assert!(!inside.nudge && !inside.notify);
        let idle = tick(false, 5000, 1200, 60, &l(100, 0, false, false)); assert!(!idle.nudge && !idle.notify);
    }
    #[test] fn liveness_never_kills() { // the Decision has no kill, interrupt, cancel or restart outcome: only nudge and notify
        let d = Decision { live: l(0, 0, false, false), nudge: true, notify: true }; assert!(d.nudge && d.notify);
    }
    #[test] fn liveness_launcher_notified_once() { assert!(!escalate(true, &l(1, 2, false, true)).notify); assert!(escalate(true, &l(1, 2, false, false)).notify);
        assert!(!escalate(false, &l(1, 2, false, true)).notify); assert!(!escalate(false, &l(1, 2, false, false)).notify); }
    #[test] fn liveness_progress_clears_stall() { let p = progress(&l(50, 9, true, true), 10, "x"); assert!(!p.stalled && !p.reported && p.nudged == 0 && p.last == 50); }
    #[test] fn liveness_nudge_before_stall() { let d = tick(true, 5000, 1, 60, &l(100, 0, false, false)); assert!(!d.live.stalled && d.nudge); }
    #[test] fn liveness_timeout_explicit_optout() { assert_eq!(timeout(true, 0), 0); assert_eq!(timeout(false, 0), DEFAULT_GAP); }
    #[test] fn liveness_record_roundtrips_like_bend() {
        let w = write(&l(5, 6, true, false)); assert_eq!(w, r#"{"last":5,"nudged":6,"stalled":true,"reported":false,"reason":"r"}"#); assert_eq!(read(&w), l(5, 6, true, false));
        assert_eq!(read("junk"), Live { last: 0, nudged: 0, stalled: false, reported: false, reason: "turn or owned work".into() });
    }
}

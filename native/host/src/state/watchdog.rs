//! Liveness progress writes (src/core/watchdog.bend `Watchdog.after`): a change that shows an agent is alive (its output, a tool act, a request,
//! a wait it set up) moves that thread's `liveness.<t>` record to now, ends an episode, clears a restart-retry count, and does the same for the
//! up-to-four owners above it that are themselves at work. Pure: `st` is the state BEFORE the batch (as Bend's `s`), `now` the batch's time.
//! Not here (PARITY.md, open): the lock heartbeat `Watchdog.progress.k` also sends, the stalled-thread tick and its deliveries.
use super::json::*;
use super::liveness;
use crate::state::{State, Turn};
use serde_json::{json, Value};

fn take(s: &str, n: usize) -> String { s.chars().take(n).collect() }
const CARRY: &str = "Backplane restarted while you were working. Continue where you left off.";

/// Watchdog.source: the thread a change is output of ("" for none)
fn source(c: &Value) -> String {
    match gstr(c, "$", "") {
        "MessagePosted" => {
            let (th, msg, text) = (gstr(c, "thread", ""), gstr(c, "msg", ""), gstr(c, "text", ""));
            match gstr(c, "role", "") {
                "user" => if msg.contains(":lr-") || text.starts_with("[Backplane]") || take(text, CARRY.chars().count()) == CARRY { String::new() } else { th.into() },
                "assistant" => th.into(),
                _ => String::new(), // Role.of reads anything else as System
            }
        }
        "AskOpened" => gstr(c, "thread", "").into(),
        "WakeSet" => gstr(c, "by", "").into(),
        "ActivityLogged" => {
            let (kind, tone) = (gstr(c, "kind", ""), gstr(c, "tone", ""));
            if kind != "restart" && kind != "watchdog" && (kind == "provider-wait" || matches!(tone, "tool" | "error" | "progress" | "say")) { gstr(c, "thread", "").into() } else { String::new() }
        }
        _ => String::new(),
    }
}
/// Watchdog.reason
fn reason(c: &Value) -> String {
    match gstr(c, "$", "") {
        "ActivityLogged" => format!("{}: {}", gstr(c, "kind", ""), take(gstr(c, "summary", ""), 180)),
        "AskOpened" => format!("waiting for user ({}): {}", gstr(c, "kind", ""), gstr(c, "detail", "")),
        "WakeSet" => format!("waiting for {} {}", gstr(c, "kind", ""), gstr(c, "target", "")),
        _ => "agent output".into(),
    }
}
fn owner(th: &str) -> &str { th.strip_prefix("@sub:").unwrap_or(th) }

fn live(st: &State, th: &str) -> liveness::Live { liveness::read(&st.get(&liveness::key(th), "")) }
/// Watchdog.parent: who launched the thread, else the parent of its unfinished task
fn parent(st: &State, th: &str) -> String {
    let by: String = st.get(&format!("launch.{th}"), "").chars().skip(1).collect();
    if !by.is_empty() { return by }
    st.tasks.values().find(|t| t.1 == th).map_or(String::new(), |t| if t.2 { String::new() } else { t.0.clone() })
}
fn rows(st: &State, th: &str) -> usize {
    let n = |k: &str| st.get(&format!("{k}{th}"), "").split('\n').filter(|l| !l.is_empty()).count();
    n("bg.") + n("watch.") + if st.get(&format!("order.wait.{th}"), "").is_empty() { 0 } else { 1 }
}
/// Watchdog.active: a thread doing work the agent owns: not archived or deleted, not held or an ephemeral run or waiting on a provider's limit,
/// executing (a turn, a start waiting on its worktree, tasks or rows), nothing queued as pending, no open request
fn active(st: &State, th: &str) -> bool {
    let blank; let t = match st.threads.get(th) { Some(t) => t, None => { blank = crate::state::Thread::blank(); &blank } };
    if t.archived || t.deleted { return false }
    let ephemeral = st.private_thread(th) || st.runs.contains_key(th) || th.starts_with("run") || th.starts_with("mailcoord-");
    if st.get(&format!("thread.config.hold.{th}"), "") == "on" || ephemeral || !st.get(&format!("provider.resume.{th}"), "").is_empty() { return false }
    let waiting = st.queue.get(th).map_or(false, |q| q.iter().any(|(m, _)| !m.starts_with("handoff:")));
    let starting = waiting && t.worktree_mode && t.worktree.is_empty();
    let kids = st.tasks.values().filter(|k| k.0 == th && !k.2).count();
    let executing = t.turn == Turn::Running || starting || kids + rows(st, th) > 0;
    executing && t.pending == 0 && !st.asks.values().any(|(a, open)| *open && a == th)
}
/// Watchdog.progress.k: the record moves to `now`; a restart-retry count is cleared with it
fn progress_k(st: &State, now: u64, th: &str, reason: &str, out: &mut Vec<Value>) {
    let l = liveness::progress(&live(st, th), now, &take(reason, 240));
    out.push(json!({"$": "SettingSet", "key": liveness::key(th), "value": liveness::write(&l), "at": now}));
    if !st.get(&format!("restart.retry.{th}"), "").is_empty() || num(&st.get(&format!("restart.retry.n.{th}"), "0")) > 0 {
        out.push(json!({"$": "SettingSet", "key": format!("restart.retry.{th}"), "value": "", "at": now}));
        out.push(json!({"$": "SettingSet", "key": format!("restart.retry.n.{th}"), "value": "0", "at": now}));
    }
}
/// Watchdog.progress: the thread itself (if the clock moved or an episode is open), then up to four owners that are at work
fn progress(st: &State, now: u64, th: &str, why: &str, out: &mut Vec<Value>) {
    let l = live(st, th);
    if now > l.last || l.nudged > 0 { progress_k(st, now, th, why, out); }
    let why = format!("owned thread {th}: {why}");
    let mut p = parent(st, th);
    for _ in 0..4 {
        if !p.is_empty() && active(st, &p) { progress_k(st, now, &p, &why, out); }
        p = parent(st, &p);
    }
}
/// Watchdog.after: the progress events for a batch. A batch that already writes a liveness record is the watchdog's own and adds nothing.
pub fn after(st: &State, now: u64, batch: &[Value]) -> Vec<Value> {
    let mut out = vec![];
    if batch.iter().any(|c| gstr(c, "$", "") == "SettingSet" && gstr(c, "key", "").starts_with("liveness.")) { return out }
    for c in batch {
        if gstr(c, "$", "") == "AskClosed" {
            let th = st.asks.get(gstr(c, "id", "")).map(|a| a.0.clone()).unwrap_or_default();
            if !th.is_empty() { progress_k(st, now, &th, "request resolved", &mut out); }
            continue;
        }
        let src = source(c);
        if !src.is_empty() { progress(st, now, owner(&src), &reason(c), &mut out); }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fold(lines: &[&str]) -> State { let mut s = State::new(); for l in lines { s.apply_line(l); } s }
    const BASE: [&str; 4] = [r#"{"$":"ProjectCreated","id":"p","title":"p","root":"/r","at":1}"#,
        r#"{"$":"ThreadCreated","id":"t1","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}"#,
        r#"{"$":"ThreadCreated","id":"t2","project":"p","title":"U","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}"#,
        r#"{"$":"TurnChanged","thread":"t1","state":"running","at":2}"#];
    fn m(th: &str, id: &str, role: &str, text: &str) -> Value { json!({"$":"MessagePosted","thread":th,"msg":id,"role":role,"text":text,"at":50}) }

    #[test] fn an_assistant_message_moves_the_threads_record() {
        let st = fold(&BASE);
        let ev = after(&st, 50, &[m("t1", "a1", "assistant", "x")]);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0]["value"], r#"{"last":50,"nudged":0,"stalled":false,"reported":false,"reason":"agent output"}"#);
    }
    #[test] fn a_synthetic_message_or_a_repeated_second_writes_nothing() {
        let st = fold(&BASE);
        assert!(after(&st, 50, &[m("t1", "t:x:lr-1", "user", "x")]).is_empty());
        assert!(after(&st, 50, &[m("t1", "u1", "user", "[Backplane] note")]).is_empty());
        assert!(after(&st, 50, &[m("t1", "u1", "user", CARRY)]).is_empty());
        let st = fold(&[BASE[0], BASE[1], r#"{"$":"SettingSet","key":"liveness.t1","value":"{\"last\":50,\"nudged\":0,\"stalled\":false,\"reported\":false,\"reason\":\"agent output\"}","at":50}"#]);
        assert!(after(&st, 50, &[m("t1", "a1", "assistant", "x")]).is_empty(), "the same second: no write");
        assert!(after(&st, 51, &[m("t1", "a1", "assistant", "x"), json!({"$":"SettingSet","key":"liveness.t1","value":"{}","at":51})]).is_empty(), "the watchdog's own batch");
    }
    #[test] fn a_working_owner_is_moved_too_and_an_idle_one_is_not() {
        let st = fold(&[BASE[0], BASE[1], BASE[2], BASE[3], r#"{"$":"TaskDelegated","task":"k","parent":"t2","child":"t1","depth":1,"provider":"claude","model":"","at":3}"#]);
        // t2 has an unfinished child task: at work, so it is moved with "owned thread" as its reason
        let ev = after(&st, 60, &[m("t1", "a1", "assistant", "x")]);
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[1]["key"], "liveness.t2");
        assert!(ev[1]["value"].as_str().unwrap().contains("owned thread t1: agent output"));
        let st = fold(&[BASE[0], BASE[1], BASE[2], BASE[3], r#"{"$":"TaskDelegated","task":"k","parent":"t2","child":"t1","depth":1,"provider":"claude","model":"","at":3}"#, r#"{"$":"TaskStateSet","task":"k","state":"done","summary":""}"#]);
        assert_eq!(after(&st, 60, &[m("t1", "a1", "assistant", "x")]).len(), 1, "a finished task has no owner above it");
    }
    #[test] fn a_tool_act_a_request_and_a_resolved_request() {
        let st = fold(&[BASE[0], BASE[1], BASE[3], r#"{"$":"AskOpened","id":"q1","thread":"t1","kind":"approval","tool":"Bash","detail":"rm x","options":""}"#, r#"{"$":"SettingSet","key":"restart.retry.t1","value":"2","at":4}"#]);
        let act = json!({"$":"ActivityLogged","thread":"t1","act":"g1","tone":"tool","kind":"Bash","summary":"ls","at":60});
        let ev = after(&st, 60, &[act, json!({"$":"AskClosed","id":"q1","answer":"y","at":60})]);
        assert_eq!(ev.len(), 6, "two liveness writes, each with the retry clears");
        assert!(ev[0]["value"].as_str().unwrap().contains(r#""reason":"Bash: ls""#));
        assert_eq!(ev[1]["key"], "restart.retry.t1"); assert_eq!(ev[2]["key"], "restart.retry.n.t1");
        assert!(ev[3]["value"].as_str().unwrap().contains("request resolved"));
        let quiet = json!({"$":"ActivityLogged","thread":"t1","act":"g2","tone":"info","kind":"git","summary":"x","at":60});
        assert!(after(&st, 61, &[quiet]).is_empty(), "an info act is not output");
    }
}

//! Liveness progress writes: a change that shows an agent is alive (its output, a tool act, a request,
//! a wait it set up) moves that thread's `liveness.<t>` record to now, ends an episode, clears a restart-retry count, and does the same for the
//! up-to-four owners above it that are themselves at work. Pure: `st` is the state BEFORE the batch, `now` the batch's time.
//! Not here (PARITY.md, open): the stalled-thread tick and its deliveries.
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
    super::ext::task_of_child(st, th).map_or(String::new(), |(p, finished)| if finished { String::new() } else { p.to_string() })
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
    let kids = super::ext::open_kids(st, th);
    let executing = t.turn == Turn::Running || starting || kids + rows(st, th) > 0;
    executing && t.pending == 0 && !st.asks.values().any(|(a, open)| *open && a == th)
}
/// Locks.lease.seconds: the lock lease, the setting locks.lease.minutes (digits; 0 or unreadable: 20) in seconds
fn lease_seconds(st: &State) -> u64 { let m = num_digits(&st.get("locks.lease.minutes", "20")); (if m == 0 { 20 } else { m }) * 60 }
fn num_digits(s: &str) -> u64 { if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) { s.parse().unwrap_or(0) } else { 0 } }
/// Locks.heartbeat: progress renews the lease of every lock party the thread owns (not a client's), so a working holder is not told its lease went stale;
/// a party is renewed only once less than half its lease is left (or none, or one is unread), so a working thread writes a LockChanged about once a
/// half-lease, not on every step; a holder at work at least once a half-lease never reaches its lease
fn heartbeat(st: &State, now: u64, th: &str, out: &mut Vec<Value>) {
    let span = lease_seconds(st); let lease = now + span;
    for name in st.get("locks.names", "").split('\n').filter(|l| !l.is_empty()) {
        let old = st.get(&format!("lock.{name}"), "");
        let mut l = parse_or_null(&old);
        let mut changed = false;
        let mut renew = |p: &mut Value| {
            let due = p["lease"].as_u64().map_or(true, |at| at < now + span / 2); // Locks.heartbeat.half: half the lease or less is left
            if due && gstr(p, "thread", "") == th && gstr(p, "kind", "thread") != "client" { if let Some(o) = p.as_object_mut() { o.insert("lease".into(), serde_json::Value::Number(lease.into())); changed = true; } }
        };
        if let Some(o) = l.as_object_mut() {
            if let Some(h) = o.get_mut("holder") { if !h.is_null() { renew(h); } }
            if let Some(Value::Array(ws)) = o.get_mut("waiters") { for w in ws { renew(w); } }
        }
        if changed && show(&l) != old { out.push(json!({"$": "LockChanged", "name": name, "lock": l, "at": now})); }
    }
}

/// Watchdog.progress.k: the record moves to `now`; a restart-retry count is cleared with it
fn progress_k(st: &State, now: u64, th: &str, reason: &str, out: &mut Vec<Value>) {
    let l = liveness::progress(&live(st, th), now, &take(reason, 240));
    out.push(json!({"$": "SettingSet", "key": liveness::key(th), "value": liveness::write(&l), "at": now}));
    if !st.get(&format!("restart.retry.{th}"), "").is_empty() || num(&st.get(&format!("restart.retry.n.{th}"), "0")) > 0 {
        out.push(json!({"$": "SettingSet", "key": format!("restart.retry.{th}"), "value": "", "at": now}));
        out.push(json!({"$": "SettingSet", "key": format!("restart.retry.n.{th}"), "value": "0", "at": now}));
    }
    heartbeat(st, now, th, out); // B.Locks.heartbeat: the same step also renews the thread's lock leases
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

// ---- Watchdog.line: streamed output with no committed message ----

fn nonempty(o: &Value, k: &str) -> bool { !gstr(o, k, "").is_empty() }
/// Watchdog.block.content: a block with text, content (a string or a non-empty list) or a tool name
fn block_content(b: &Value) -> bool {
    nonempty(b, "text") || nonempty(b, "content") || !elems(gobj(b, "content")).is_empty() || nonempty(b, "name")
}
/// Watchdog.blocks: any block that is output (for a user line only tool results count)
fn blocks(xs: &[Value], user: bool) -> bool { xs.iter().any(|b| (!user || gstr(b, "type", "") == "tool_result") && block_content(b)) }
/// Watchdog.output: a provider line that is real output (text, thinking, partial tool input, a command or its output), never a ping or a quota line
pub fn line_is_output(o: &Value) -> bool {
    match gstr(o, "type", "") {
        "assistant" => blocks(elems(gobj(gobj(o, "message"), "content")), false),
        "user" => blocks(elems(gobj(gobj(o, "message"), "content")), true),
        "stream_event" => { let d = gobj(gobj(o, "event"), "delta"); nonempty(d, "text") || nonempty(d, "thinking") || nonempty(d, "partial_json") }
        "item.delta" => nonempty(o, "text"),
        "item.started" => nonempty(gobj(o, "item"), "command"),
        "item.completed" => { let i = gobj(o, "item"); nonempty(i, "text") || nonempty(i, "aggregated_output") || nonempty(i, "command") }
        "item.updated" => nonempty(gobj(o, "item"), "text"),
        _ => false,
    }
}
/// Prog.is: a progress line of the thread's own agent (a subagent's goes to its log)
fn is_progress(o: &Value) -> bool { gstr(o, "type", "") == "system" && gstr(o, "subtype", "") == "task_progress" && gstr(o, "parent_tool_use_id", "").is_empty() }
/// Watchdog.line: the thread's record moves at five-second precision on real output, or on a task's progress line that says something new;
/// an open episode ends on the first real output at once. `st` is the current state. The agents lane calls this for each provider line and
/// commits the events it returns.
pub fn line(st: &State, now: u64, th: &str, o: &Value) -> Vec<Value> {
    let task = is_progress(o) && {
        let id = gstr(o, "task_id", "");
        let rows = st.get(&format!("bg.{th}"), "");
        let known = rows.split('\n').filter(|l| !l.is_empty()).find(|row| row.split('\t').next() == Some(id)).map_or("", |row| row.split('\t').nth(1).unwrap_or(""));
        gstr(o, "description", "") != known
    };
    if !(line_is_output(o) || task) { return vec![] } // most lines are not output: no record is read for them
    let l = live(st, th);
    if !(now >= l.last.saturating_add(5) || l.nudged > 0) { return vec![] }
    let mut out = vec![];
    progress(st, now, th, "agent tool or stream output", &mut out);
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
    #[test] fn streamed_output_moves_the_record_at_five_second_precision() {
        let st = fold(&[BASE[0], BASE[1], BASE[3], r#"{"$":"SettingSet","key":"liveness.t1","value":"{\"last\":100,\"nudged\":0,\"stalled\":false,\"reported\":false,\"reason\":\"r\"}","at":100}"#]);
        let text = json!({"type":"stream_event","event":{"delta":{"text":"hi"}}});
        assert!(line(&st, 104, "t1", &text).is_empty(), "inside five seconds");
        let ev = line(&st, 105, "t1", &text); assert_eq!(ev.len(), 1);
        assert!(ev[0]["value"].as_str().unwrap().contains(r#""last":105"#) && ev[0]["value"].as_str().unwrap().contains("agent tool or stream output"));
        // a ping or an empty delta is no output; an open episode ends at once
        assert!(line(&st, 200, "t1", &json!({"type":"stream_event","event":{"delta":{"text":""}}})).is_empty());
        assert!(line(&st, 200, "t1", &json!({"type":"system","subtype":"init"})).is_empty());
        let st2 = fold(&[BASE[0], BASE[1], r#"{"$":"SettingSet","key":"liveness.t1","value":"{\"last\":100,\"nudged\":101,\"stalled\":true,\"reported\":true,\"reason\":\"r\"}","at":100}"#]);
        assert_eq!(line(&st2, 102, "t1", &text).len(), 1, "an open episode (nudged) ends on the first output even inside five seconds");
    }
    #[test] fn which_provider_lines_are_output() {
        for o in [json!({"type":"assistant","message":{"content":[{"type":"text","text":"x"}]}}), json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}),
                  json!({"type":"user","message":{"content":[{"type":"tool_result","content":"out"}]}}), json!({"type":"stream_event","event":{"delta":{"thinking":"t"}}}),
                  json!({"type":"stream_event","event":{"delta":{"partial_json":"{"}}}), json!({"type":"item.delta","text":"x"}), json!({"type":"item.started","item":{"command":"ls"}}),
                  json!({"type":"item.completed","item":{"aggregated_output":"o"}}), json!({"type":"item.updated","item":{"text":"x"}})] { assert!(line_is_output(&o), "{o}"); }
        for o in [json!({"type":"user","message":{"content":[{"type":"text","text":"a person"}]}}), json!({"type":"assistant","message":{"content":[]}}), json!({"type":"result"}), json!({"type":"item.started","item":{}})] { assert!(!line_is_output(&o), "{o}"); }
        // a task progress line counts when its description is new
        let st = fold(&[BASE[0], BASE[1], r#"{"$":"SettingSet","key":"bg.t1","value":"task9\tsame\tx","at":1}"#]);
        let prog = |d: &str| json!({"type":"system","subtype":"task_progress","task_id":"task9","description":d});
        assert!(line(&st, 50, "t1", &prog("same")).is_empty()); assert_eq!(line(&st, 50, "t1", &prog("new")).len(), 1);
    }
    #[test] fn progress_renews_the_threads_lock_leases_and_only_when_they_change() {
        let lock = r#"{"$":"LockChanged","name":"merge","lock":{"holder":{"id":"a","kind":"thread","thread":"t1","note":"","since":1,"lease":5},"waiters":[{"id":"b","kind":"client","thread":"t1","note":"","since":2,"lease":6},{"id":"c","kind":"thread","thread":"t2","note":"","since":3,"lease":7}],"ready":false},"at":1}"#;
        let st = fold(&[BASE[0], BASE[1], BASE[3], lock]);
        let ev = after(&st, 1000, &[m("t1", "a1", "assistant", "x")]);
        let lk = ev.iter().find(|e| e["$"] == "LockChanged").expect("a LockChanged");
        assert_eq!(lk["lock"]["holder"]["lease"], 1000 + 1200, "now + 20 minutes");
        assert_eq!(lk["lock"]["waiters"][0]["lease"], 6, "a client's party is not renewed"); assert_eq!(lk["lock"]["waiters"][1]["lease"], 7, "another thread's is not either");
        let st2 = fold(&[BASE[0], BASE[1], BASE[3], lock, r#"{"$":"SettingSet","key":"locks.lease.minutes","value":"1","at":2}"#]);
        assert_eq!(after(&st2, 1000, &[m("t1", "a1", "assistant", "x")]).iter().find(|e| e["$"] == "LockChanged").unwrap()["lock"]["holder"]["lease"], 1060);
        let st3 = fold(&[BASE[0], BASE[1], BASE[3], r#"{"$":"LockChanged","name":"merge","lock":{"holder":{"id":"a","kind":"thread","thread":"t1","note":"","since":1,"lease":2200},"waiters":[],"ready":false},"at":1}"#]);
        assert!(after(&st3, 1000, &[m("t1", "a1", "assistant", "x")]).iter().all(|e| e["$"] != "LockChanged"), "a lease already at now + 20 minutes is unchanged: no event");
        let st4 = fold(&[BASE[0], BASE[1], BASE[3], &held("t1", 1601)]);
        assert!(after(&st4, 1000, &[m("t1", "a1", "assistant", "x")]).iter().all(|e| e["$"] != "LockChanged"), "more than half the lease left: not renewed yet");
    }
    fn held(th: &str, lease: u64) -> String { json!({"$":"LockChanged","name":"merge","lock":{"holder":{"id":th,"kind":"thread","thread":th,"note":"","since":1,"lease":lease},"waiters":[],"ready":true},"at":1}).to_string() }
    fn lock_events(ev: &[Value]) -> usize { ev.iter().filter(|e| e["$"] == "LockChanged").count() }
    #[test] fn a_hundred_progress_steps_in_a_minute_write_at_most_one_lock_change() {
        let mut st = fold(&[BASE[0], BASE[1], BASE[3], &held("t1", 5)]);
        let mut n = 0;
        for i in 0..100u64 {
            let ev = after(&st, 1000 + i * 60 / 100, &[m("t1", &format!("a{i}"), "assistant", "x")]);
            n += lock_events(&ev);
            for e in &ev { st.apply_line(&e.to_string()); }
        }
        assert_eq!(n, 1, "the first step renews the stale lease, the other 99 leave it");
        assert_eq!(parse_or_null(&st.get("lock.merge", ""))["holder"]["lease"], 1000 + 1200);
    }
    #[test] fn a_lease_near_expiry_is_renewed_and_a_working_holder_never_goes_stale() {
        let st = fold(&[BASE[0], BASE[1], BASE[3], &held("t1", 1599)]);
        let ev = after(&st, 1000, &[m("t1", "a1", "assistant", "x")]);
        assert_eq!(lock_events(&ev), 1); assert_eq!(ev.iter().find(|e| e["$"] == "LockChanged").unwrap()["lock"]["holder"]["lease"], 2200, "under half left: renewed to now + 20 minutes");
        // a step every 9 minutes for 3 hours: each lease is still running when the next step comes
        let mut st = fold(&[BASE[0], BASE[1], BASE[3], &held("t1", 1000 + 1200)]);
        for k in 1..=20u64 {
            let now = 1000 + k * 540;
            let lease = parse_or_null(&st.get("lock.merge", ""))["holder"]["lease"].as_u64().unwrap();
            assert!(lease > now, "step {k}: the lease {lease} is still running at {now}");
            for e in after(&st, now, &[m("t1", &format!("a{k}"), "assistant", "x")]) { st.apply_line(&e.to_string()); }
        }
    }
    #[test] fn a_clients_or_another_threads_party_is_never_renewed() {
        let lock = r#"{"$":"LockChanged","name":"merge","lock":{"holder":{"id":"c","kind":"thread","thread":"t2","note":"","since":3,"lease":7},"waiters":[{"id":"b","kind":"client","thread":"t1","note":"","since":2,"lease":6},{"id":"w","kind":"work","thread":"t2","note":"","since":3,"lease":0}],"ready":true},"at":1}"#;
        let st = fold(&[BASE[0], BASE[1], BASE[3], lock]);
        for now in [1000, 5000, 100_000] { assert_eq!(lock_events(&after(&st, now, &[m("t1", "a1", "assistant", "x")])), 0, "at {now}: t1 works, but none of these parties is its own"); }
    }
}

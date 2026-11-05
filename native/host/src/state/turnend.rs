//! What a turn's end writes (hub.bend `Ended.out`, `Alerts.changes`, core/notice.bend `Policy.end`, core/told.bend `Told.alerts`): the thread's
//! summary clock `summary.at.<t>` when a summary is due, the alert record `alert.<t>` (done, fail, or the quiet record that moves the settle
//! memory), and one `told.<id>` mark for every alert record that has a kind. Pure over the state BEFORE the batch (as Bend's `s`).
//! Deliberate or open (docs/gpui/PARITY.md): `Triage.managed` reads false until mail lands; `notify.all` (the older `Notice.of` rule) reads off;
//! the ask alert records of `Alerts.asks`; `Run.settle` (mail runs); the summary text itself (`Sum.out`, a seam of the agents lane).
use super::json::*;
use crate::state::{State, Thread, Turn};
use serde_json::{json, Value};

fn take(s: &str, n: usize) -> String { s.chars().take(n).collect() }

/// Policy.direct: a message the person typed (no sender thread; not an envelope unless it is the person's own)
pub fn policy_direct(id: &str, text: &str) -> bool { crate::state::link_from(id).is_empty() && (!text.starts_with('[') || text.starts_with("[message from person@")) }
/// Told.first: the first line of a reply, at most 200 characters
pub fn told_first(text: &str) -> String { take(take(text.trim(), 400).split('\n').next().unwrap_or("").trim(), 200) }
fn told_line(title: &str, line: &str) -> String { if line.is_empty() { title.to_string() } else { format!("{title}: {line}") } }
fn text_cut(t: &str) -> String { take(t.trim(), 280).trim().to_string() }
/// Told.key.norm: trimmed, lower case, each run of other characters one "-", at most 120 characters
pub fn key_norm(s: &str) -> String {
    let (mut n, mut dash, mut out) = (0usize, false, String::new());
    for c in s.trim().chars() {
        if n >= 120 { break }
        let l = if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c };
        if l.is_ascii_lowercase() || l.is_ascii_digit() || matches!(l, '.' | '_' | ':' | '-') { out.push(l); n += 1; dash = false; }
        else if !dash { out.push('-'); n += 1; dash = true; }
    }
    out
}
fn fr_local(thread: &str) -> &str { thread.split_once('~').map_or(thread, |(_, t)| t) }

/// Sum.due: never made, or made 600 seconds ago or more
pub fn sum_due(st: &State, id: &str, now: u64) -> bool {
    let at = st.get(&format!("summary.at.{id}"), "");
    at.is_empty() || now.saturating_sub(num(&at)) >= 600
}
/// Ended.state: how the thread's turn ended in the batch ("" for none). A stop already ended its turn; only a turn still running is interrupted.
pub fn ended_state(batch: &[Value], thread: &str, was_running: bool) -> &'static str {
    for c in batch {
        if gstr(c, "$", "") != "TurnChanged" || gstr(c, "thread", "") != thread { continue }
        match gstr(c, "state", "") { "completed" => return "completed", "failed" => return "failed", "interrupted" if was_running => return "interrupted", _ => {} }
    }
    ""
}

/// Policy.on: the setting notify.<name>; needs, settled and reminders default on, all off
fn notify_on(st: &State, name: &str) -> bool {
    let dflt = name != "all";
    match st.get(&format!("notify.{name}"), "").as_str() { "on" => true, "off" => false, _ => dflt }
}
/// Alert.get/parse: the record `kind \t key \t settled \t stamp \t text`
struct Rec<'a> { kind: &'a str, key: &'a str, settled: &'a str, text: &'a str }
fn parse(v: &str) -> Rec<'_> {
    let mut it = v.splitn(5, '\t');
    let (kind, key, settled, _stamp) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
    Rec { kind, key, settled, text: it.next().unwrap_or("") }
}
fn lines(text: &str) -> Vec<&str> { text.split('\n').filter(|l| !l.is_empty()).collect() }
fn rows(st: &State, th: &str) -> usize {
    let n = |k: &str| lines(&st.get(&format!("{k}{th}"), "")).len();
    n("bg.") + n("watch.") + if st.get(&format!("order.wait.{th}"), "").is_empty() { 0 } else { 1 }
}
/// Work.ready: not running, not starting, nothing out
fn work_ready(st: &State, th: &str) -> bool {
    let blank; let t = match st.threads.get(th) { Some(t) => t, None => { blank = Thread::blank(); &blank } };
    let waiting = st.queue.get(th).map_or(false, |q| q.iter().any(|(m, _)| !m.starts_with("handoff:")));
    let starting = waiting && t.worktree_mode && t.worktree.is_empty();
    t.turn != Turn::Running && !starting && super::ext::open_kids(st, th) + rows(st, th) == 0
}
/// Policy.ready: the thread's work is all done: no request open, no reporting helper it launched still works
fn ready(st: &State, th: &str) -> bool {
    if !work_ready(st, th) || st.asks.values().any(|(a, open)| *open && a == th) { return false }
    lines(&st.get(&format!("launched.{th}"), "")).iter().all(|id| {
        let v = st.get(&format!("launch.{id}"), ""); let reports = v.starts_with('1') && v.chars().count() > 1;
        !(reports && !work_ready(st, id))
    })
}
/// Alert.set: the thread's record with a stamp `<now>.<seq>` (seq: the changes the state had applied after the step's own: Alert.seq(post))
fn alert_set(seq: u64, now: u64, thread: &str, kind: &str, key: &str, settled: &str, text: &str) -> Value {
    json!({"$": "SettingSet", "key": format!("alert.{thread}"), "value": format!("{kind}\t{key}\t{settled}\t{now}.{seq}\t{text}"), "at": now})
}
/// Policy.end: what a turn's end records, over the state after the batch: zero or one setting
pub fn policy_end(post: &State, seq: u64, now: u64, thread: &str, state: &str) -> Vec<Value> {
    // Triage.managed reads false until mail lands (open work)
    let pid = post.ext.sum.get(thread).map_or("", |s| s.person_id.as_str());
    let mem = parse(&post.get(&format!("alert.{thread}"), "")).settled.to_string();
    let fresh = !pid.is_empty() && pid != mem;
    let ready = ready(post, thread);
    let n = if !fresh { 0 } else if state == "completed" { if ready { 1 } else { 0 } } else if state == "failed" { 2 } else if ready && (state == "interrupted" || state == "stopped") { 3 } else { 0 };
    // the quiet record: the settle memory moves to the person's message
    let quiet = |mem: &str, old: &str| if mem == old { vec![] } else { vec![alert_set(seq, now, thread, "", "", mem, "")] };
    let settle = |on: bool, kind: &str| if on { vec![alert_set(seq, now, thread, kind, &format!("turn-{}", fr_local(thread)), pid, "")] } else { quiet(pid, &mem) };
    match n {
        0 => vec![], // notify.all (Notice.of) reads off: with mem == old the quiet record writes nothing
        1 => settle(notify_on(post, "settled"), "done"),
        2 => settle(notify_on(post, "settled"), "fail"),
        _ => quiet(pid, &mem),
    }
}

/// What Told.alerts needs of the state before the step, for the threads whose alert records are in it
pub struct ToldCtx { pub seq: u64, items: std::collections::HashMap<String, (String, String, String)> }
pub fn told_ctx(pre: &State, batch: &[Value], extra_threads: &[String]) -> ToldCtx {
    let mut ths: Vec<String> = extra_threads.to_vec();
    for c in batch { if gstr(c, "$", "") == "SettingSet" { if let Some(t) = gstr(c, "key", "").strip_prefix("alert.") { ths.push(t.to_string()); } } }
    let mut items = std::collections::HashMap::new();
    for th in ths {
        let title = pre.threads.get(&th).map_or(String::new(), |t| t.title.clone());
        let reply = pre.ext.sum.get(&th).map_or(String::new(), |s| s.reply_line.clone());
        let bot = pre.bot_threads.iter().find(|(_, t)| **t == th).map(|(b, _)| b.clone()).unwrap_or_default();
        items.insert(th, (title, reply, bot));
    }
    ToldCtx { seq: pre.seq, items }
}
/// Told.alerts: a mark for every record in the list that alerts (a kind), numbered by its place in the list
pub fn told_marks(ctx: &ToldCtx, list: &[Value]) -> Vec<Value> {
    let mut out = vec![];
    for (i, c) in list.iter().enumerate() {
        if gstr(c, "$", "") != "SettingSet" { continue }
        let (k, v, at) = (gstr(c, "key", ""), gstr(c, "value", ""), gnat(c, "at", 0));
        let Some(th) = k.strip_prefix("alert.") else { continue };
        if v.is_empty() || v.starts_with('\t') { continue }
        let r = parse(v);
        let (title, reply, bot) = ctx.items.get(th).cloned().unwrap_or_default();
        let said = if r.text.is_empty() { told_line(&title, &reply) } else { told_line(&title, r.text) };
        let raw_key = if r.key.is_empty() { format!("turn-{th}") } else { r.key.to_string() };
        let key = key_norm(&raw_key);
        let id = format!("k{at}-{}-{i}", ctx.seq);
        let rec = json!({"key": key, "text": text_cut(&said), "channel": "alert", "bot": bot, "at": at});
        out.push(json!({"$": "SettingSet", "key": format!("told.{id}"), "value": show(&rec), "at": at}));
    }
    out
}

/// Ended.out for a batch: for each thread whose turn ended in it, the summary clock and the alert record. `post` is the state after the batch;
/// whether the turn was running and whether a summary was due are read before it (`pre_running`, `pre_due`); `seq` is the changes applied after it.
pub fn ended_events_from(post: &State, pre_running: &[(String, bool)], pre_due: &[(String, bool)], seq: u64, now: u64, batch: &[Value]) -> Vec<Value> {
    let mut seen: Vec<&str> = vec![]; let mut out = vec![];
    for th in ended_threads(batch) {
        if seen.contains(&th) { continue }
        seen.push(th);
        let running = pre_running.iter().find(|x| x.0 == th).map_or(false, |x| x.1);
        let state = ended_state(batch, th, running);
        if state.is_empty() { continue }
        if pre_due.iter().find(|x| x.0 == th).map_or(false, |x| x.1) { out.push(json!({"$": "SettingSet", "key": format!("summary.at.{th}"), "value": now.to_string(), "at": now})); }
        out.extend(policy_end(post, seq, now, th, state));
    }
    out
}
/// the same from the state before the batch and after it (the probe's form)
pub fn ended_events(pre: &State, post: &State, seq: u64, now: u64, batch: &[Value]) -> Vec<Value> {
    let ths: Vec<String> = ended_threads(batch).into_iter().map(String::from).collect();
    let run: Vec<(String, bool)> = ths.iter().map(|t| (t.clone(), was_running(pre, t))).collect();
    let due: Vec<(String, bool)> = ths.iter().map(|t| (t.clone(), sum_due(pre, t, now))).collect();
    ended_events_from(post, &run, &due, seq, now, batch)
}
/// the threads with a TurnChanged in the batch, in order
pub fn ended_threads(batch: &[Value]) -> Vec<&str> { batch.iter().filter(|c| gstr(c, "$", "") == "TurnChanged").map(|c| gstr(c, "thread", "")).filter(|t| !t.is_empty()).collect() }
pub fn was_running(pre: &State, th: &str) -> bool { pre.threads.get(th).map_or(false, |t| t.turn == Turn::Running) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn told_keys_and_lines_follow_bend() {
        assert_eq!(key_norm("  Lunch: Dana!!  "), "lunch:-dana-"); assert_eq!(key_norm("turn-t1"), "turn-t1");
        assert_eq!(key_norm(&"a".repeat(130)).len(), 120);
        assert_eq!(told_first("  Fixture long: first line\nsecond"), "Fixture long: first line"); assert_eq!(told_first(""), "");
        assert_eq!(told_line("T", ""), "T"); assert_eq!(told_line("T", "x"), "T: x");
        assert!(policy_direct("m1", "hello") && !policy_direct("m1", "[routine: x]") && policy_direct("m1", "[message from person@box] hi") && !policy_direct("t:a:1", "hello"));
    }
    #[test] fn ended_state_reads_the_first_end_of_the_thread() {
        let b = [json!({"$":"TurnChanged","thread":"a","state":"interrupted"}), json!({"$":"TurnChanged","thread":"t","state":"interrupted"}), json!({"$":"TurnChanged","thread":"t","state":"failed"})];
        assert_eq!(ended_state(&b, "t", false), "failed", "a stop already ended its turn: not a second end");
        assert_eq!(ended_state(&b, "t", true), "interrupted"); assert_eq!(ended_state(&b, "x", true), "");
    }
}

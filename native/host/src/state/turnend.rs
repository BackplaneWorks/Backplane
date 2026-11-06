//! What a turn's end writes: the thread's
//! summary clock `summary.at.<t>` when a summary is due, the alert record `alert.<t>` (done, fail, or the quiet record that moves the settle
//! memory), and one `told.<id>` mark for every alert record that has a kind. Pure over the state BEFORE the batch.
//! Deliberate or open (docs/PARITY.md): `Triage.managed` reads false until mail lands; `notify.all` (the older `Notice.of` rule) reads off;
//! the ask alert records of `Alerts.asks`; `Run.settle` (mail runs); the summary text itself (`Sum.out`, a seam of the agents lane).
use super::json::*;
use crate::state::{State, Thread, Turn};
use serde_json::{json, Value};

fn take(s: &str, n: usize) -> String { s.chars().take(n).collect() }
/// String.trim: Char.is_space is only the space and 9..13 (tab, newline, vertical tab, form feed, carriage return)
pub fn trim_ws(s: &str) -> &str { s.trim_matches(|c: char| c == ' ' || ('\t'..='\r').contains(&c)) }

/// Policy.direct: a message the person typed (no sender thread; not an envelope unless it is the person's own)
pub fn policy_direct(id: &str, text: &str) -> bool { crate::state::link_from(id).is_empty() && (!text.starts_with('[') || text.starts_with("[message from person@")) }
/// Told.first: the first line of a reply, at most 200 characters
pub fn told_first(text: &str) -> String { take(take(trim_ws(text), 400).split('\n').next().map_or("", trim_ws), 200) }
fn told_line(title: &str, line: &str) -> String { if line.is_empty() { title.to_string() } else { format!("{title}: {line}") } }
fn text_cut(t: &str) -> String { trim_ws(&take(trim_ws(t), 280)).to_string() }
/// Told.key.norm: trimmed, lower case, each run of other characters one "-", at most 120 characters
pub fn key_norm(s: &str) -> String {
    let (mut n, mut dash, mut out) = (0usize, false, String::new());
    for c in trim_ws(s).chars() {
        if n >= 120 { break }
        let l = if c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c };
        if l.is_ascii_lowercase() || l.is_ascii_digit() || matches!(l, '.' | '_' | ':' | '-') { out.push(l); n += 1; dash = false; }
        else if !dash { out.push('-'); n += 1; dash = true; }
    }
    out
}
fn fr_local(thread: &str) -> &str { thread.split_once('~').map_or(thread, |(_, t)| t) }

/// Told.stamp.id: the time in a record's id "k<time>-<sequence>" (None when the id is not one of ours)
pub fn told_stamp(id: &str) -> Option<u64> {
    let rest = id.strip_prefix('k')?;
    let (mut acc, mut any) = (0u64, false);
    for c in rest.chars() {
        if c.is_ascii_digit() { acc = acc.wrapping_mul(10).wrapping_add(c as u64 - 48); any = true; }
        else if c == '-' && any { return Some(acc) }
        else { return None }
    }
    None // never reached a dash: Told.stamp.of wants the state "done"
}
pub const TOLD_KEEP_S: u64 = 2_592_000; // 30 days
pub const TOLD_MAX: usize = 2000;
/// Told.prune: a record older than 30 days, and the oldest ones beyond the newest 2000, are blanked. Reads only the keys (a record's time is in
/// its id; a record written by hand with another id is read from its value's "at"), newest first.
pub fn told_prune(st: &State, now: u64) -> Vec<Value> {
    let mut ks: Vec<(String, u64)> = st.settings.iter().filter_map(|(k, v)| {
        let id = k.strip_prefix("told.")?;
        let at = told_stamp(id).unwrap_or_else(|| gnat(&parse_or_null(v), "at", 0));
        Some((k.clone(), at))
    }).collect();
    ks.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ks.into_iter().enumerate().filter(|(i, (_, at))| *i >= TOLD_MAX || at.saturating_add(TOLD_KEEP_S) < now)
        .map(|(_, (k, _))| json!({"$": "SettingSet", "key": k, "value": "", "at": now})).collect()
}

pub const INPUT_MAX: usize = 200;
/// Input.prune: the removals of the turn inputs (launch.input.<t>, then fallback.input.<t>, each in key order) of threads at rest (not running,
/// nothing queued), at most 200 a tick. Reads only the keys.
pub fn input_prune(st: &State, now: u64) -> Vec<Value> {
    let mut out = vec![];
    for pre in ["launch.input.", "fallback.input."] {
        let mut ks: Vec<&String> = st.settings.keys().filter(|k| k.starts_with(pre)).collect();
        ks.sort();
        for k in ks {
            let th = &k[pre.len()..];
            let busy = st.threads.get(th).map_or(false, |t| t.turn == Turn::Running || t.pending > 0);
            if !busy { out.push(json!({"$": "SettingSet", "key": k, "value": "", "at": now})); }
        }
    }
    out.truncate(INPUT_MAX);
    out
}

/// Sum.due: never made, or made 600 seconds ago or more
pub fn sum_due(st: &State, id: &str, now: u64) -> bool {
    let at = st.get(&format!("summary.at.{id}"), "");
    at.is_empty() || now.saturating_sub(num(&at)) >= 600
}
/// The marker the writer of an end sets on its TurnChanged event (`"_end": true`) when hub runs `Ended.out` for that end: only the
/// agent line handlers and agent-gone do. A Stop (Retire.turn.f), a spawn failure
/// (Hub.spawned.fail), a restart cut and a task cancel commit their TurnChanged without it and get no summary clock and no alert record.
/// `Policy::before_commit` removes the marker before the batch is written.
pub const END_MARK: &str = "_end";
pub fn is_end(c: &Value) -> bool { gstr(c, "$", "") == "TurnChanged" && c.get(END_MARK).and_then(|v| v.as_bool()) == Some(true) }
/// the batch without the end markers (what is written)
pub fn strip_marks(events: &mut [Value]) { for e in events { if let Some(o) = e.as_object_mut() { o.remove(END_MARK); } } }

/// Ended.state: how the thread's turn ended in the batch ("" for none), over the ends the writer marked. A stop already ended its turn; only a
/// turn still running is interrupted.
pub fn ended_state(batch: &[Value], thread: &str, was_running: bool) -> &'static str {
    for c in batch {
        if !is_end(c) || gstr(c, "thread", "") != thread { continue }
        match gstr(c, "state", "") { "completed" => return "completed", "failed" => return "failed", "interrupted" if was_running => return "interrupted", _ => {} }
    }
    ""
}

/// A failed end's mark (`failed.<msg>`): writes it (Stamp.fail.mark) AFTER the alert record, outside the state the policy decided on; the
/// agents put it in the batch next to the TurnChanged. Split the batch into what the end decides on and those marks, so the stamp, the order
/// and the told numbering are . Empty marks when no marked end failed.
pub fn split_fail_marks(events: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let failed = events.iter().any(|c| is_end(c) && gstr(c, "state", "") == "failed");
    if !failed { return (events.to_vec(), vec![]) }
    let is_mark = |c: &Value| gstr(c, "$", "") == "SettingSet" && gstr(c, "key", "").starts_with("failed.");
    (events.iter().filter(|c| !is_mark(c)).cloned().collect(), events.iter().filter(|c| is_mark(c)).cloned().collect())
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

/// Alert.clean / Alert.text: a text as one line (tab and newline become a space), cut to max characters after trimming
fn alert_text(t: &str, max: usize) -> String { take(trim_ws(t), max).chars().map(|c| if c == '\t' || c == '\n' { ' ' } else { c }).collect() }
/// Alert.ask.say: what an ask's alert says: a paused browser task its one-line question, an order's question or approval its detail, any other its tool and detail
fn ask_say(tool: &str, detail: &str) -> String {
    if tool == "task_pause" { return gstr(&parse_or_null(detail), "question", "").to_string() }
    let plain = tool.starts_with("order_ask") || tool == "order_quote" || tool == "checks_ok";
    if plain || tool.is_empty() { detail.to_string() } else { format!("{tool} {detail}") }
}
/// Alerts.asks: the alert record of every ask the step opens (kind "ask", or the quiet record when the person turned the needs alerts off).
/// `post` is the state after the step's changes so far, `seq` the changes applied after them (Alert.seq(post)). Triage.managed reads false until mail lands.
pub fn alert_asks(post: &State, seq: u64, list: &[Value]) -> Vec<Value> {
    let mut out = vec![];
    for c in list {
        if gstr(c, "$", "") != "AskOpened" { continue }
        let (th, tool, detail, at) = (gstr(c, "thread", ""), gstr(c, "tool", ""), gstr(c, "detail", ""), gnat(c, "at", 0));
        let mem = parse(&post.get(&format!("alert.{th}"), "")).settled.to_string();
        let on = notify_on(post, "needs");
        out.push(if tool == "settings_proposal" {
            if on { alert_set(seq, at, th, "ask", &format!("ask-{}", fr_local(th)), &mem, &alert_text("A settings proposal needs your review.", 180)) } else { alert_set(seq, at, th, "", "", &mem, "") }
        } else if on { alert_set(seq, at, th, "ask", &format!("ask-{}", fr_local(th)), &mem, &alert_text(&ask_say(tool, detail), 180)) } else { alert_set(seq, at, th, "", "", &mem, "") });
    }
    out
}

/// What Told.alerts needs of the state before the step, for the threads whose alert records are in it
pub struct ToldCtx { pub seq: u64, items: std::collections::HashMap<String, (String, String, String)> }
pub fn told_ctx(pre: &State, batch: &[Value], extra_threads: &[String]) -> ToldCtx {
    let mut ths: Vec<String> = extra_threads.to_vec();
    for c in batch { if gstr(c, "$", "") == "SettingSet" { if let Some(t) = gstr(c, "key", "").strip_prefix("alert.") { ths.push(t.to_string()); } } }
    // the threads of the asks the step opens get an alert record (Alerts.asks) and so a told mark
    for c in batch { if gstr(c, "$", "") == "AskOpened" { ths.push(gstr(c, "thread", "").to_string()); } }
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
/// the threads with a marked end in the batch, in order
pub fn ended_threads(batch: &[Value]) -> Vec<&str> { batch.iter().filter(|c| is_end(c)).map(|c| gstr(c, "thread", "")).filter(|t| !t.is_empty()).collect() }
pub fn was_running(pre: &State, th: &str) -> bool { pre.threads.get(th).map_or(false, |t| t.turn == Turn::Running) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn told_keys_and_lines_follow() {
        assert_eq!(key_norm("  Lunch: Dana!!  "), "lunch:-dana-"); assert_eq!(key_norm("turn-t1"), "turn-t1");
        assert_eq!(key_norm(&"a".repeat(130)).len(), 120);
        assert_eq!(told_first("  Fixture long: first line\nsecond"), "Fixture long: first line"); assert_eq!(told_first(""), "");
        assert_eq!(told_line("T", ""), "T"); assert_eq!(told_line("T", "x"), "T: x");
        assert!(policy_direct("m1", "hello") && !policy_direct("m1", "[routine: x]") && policy_direct("m1", "[message from person@box] hi") && !policy_direct("t:a:1", "hello"));
    }
    #[test] fn ended_state_reads_the_first_end_of_the_thread() {
        let b = [json!({"$":"TurnChanged","thread":"a","state":"interrupted","_end":true}), json!({"$":"TurnChanged","thread":"t","state":"interrupted","_end":true}), json!({"$":"TurnChanged","thread":"t","state":"failed","_end":true})];
        assert_eq!(ended_state(&b, "t", false), "failed", "a stop already ended its turn: not a second end");
        assert_eq!(ended_state(&b, "t", true), "interrupted"); assert_eq!(ended_state(&b, "x", true), "");
    }
    #[test] fn only_a_marked_end_is_an_end() {
        let stop = [json!({"$":"TurnChanged","thread":"t","state":"interrupted"})];
        assert_eq!(ended_state(&stop, "t", true), "", "a Stop (Retire.turn.f) is not Ended.out");
        let mut marked = vec![json!({"$":"TurnChanged","thread":"t","state":"completed","_end":true})];
        assert_eq!(ended_state(&marked, "t", true), "completed"); strip_marks(&mut marked);
        assert!(marked[0].get("_end").is_none(), "the marker is not written");
    }
    #[test] fn trim_is_the_space_and_nine_to_thirteen_only() {
        assert_eq!(trim_ws(" \t\n\r\x0b\x0cx \n"), "x");
        assert_eq!(trim_ws("\u{a0}x\u{3000}"), "\u{a0}x\u{3000}", "NBSP and U+3000 are not spaces to ");
        assert_eq!(told_first("\u{a0}lead"), "\u{a0}lead");
    }
    #[test] fn a_failed_ends_marks_are_split_off() {
        let b = vec![json!({"$":"ActivityLogged","thread":"t","act":"g","tone":"error","kind":"x","summary":"s","at":1}), json!({"$":"TurnChanged","thread":"t","state":"failed","_end":true,"at":1}), json!({"$":"SettingSet","key":"failed.m1","value":"1","at":1})];
        let (core, marks) = split_fail_marks(&b);
        assert_eq!((core.len(), marks.len()), (2, 1));
        let ok = vec![b[1].clone(), b[2].clone()]; let mut ok2 = ok.clone(); ok2[0]["state"] = json!("completed");
        assert_eq!(split_fail_marks(&ok2).1.len(), 0, "a completed end keeps whatever else is in its batch");
    }
    #[test] fn told_ids_carry_their_time() {
        assert_eq!(told_stamp("k1793846698-190-3"), Some(1793846698)); assert_eq!(told_stamp("k5-1"), Some(5));
        assert_eq!(told_stamp("kabc"), None); assert_eq!(told_stamp("x5-1"), None); assert_eq!(told_stamp("k55"), None, "no dash: not one of ours"); assert_eq!(told_stamp("k-1"), None);
    }
    #[test] fn input_prune_removes_the_inputs_of_threads_at_rest_only() {
        let mut st = State::new(); st.lean = true;
        for l in [r#"{"$":"SettingSet","key":"launch.input.a","value":"x","at":1}"#, r#"{"$":"SettingSet","key":"fallback.input.a","value":"x","at":1}"#,
                  r#"{"$":"SettingSet","key":"launch.input.b","value":"x","at":1}"#, r#"{"$":"TurnChanged","thread":"b","state":"running","at":2}"#] { st.apply_line(l); }
        let gone: Vec<String> = input_prune(&st, 9).iter().map(|e| e["key"].as_str().unwrap().to_string()).collect();
        assert_eq!(gone, ["launch.input.a", "fallback.input.a"]);
    }
    #[test] fn told_prune_drops_the_old_and_all_but_the_newest_2000() {
        let mut st = State::new();
        let now = 10_000_000u64;
        st.apply_line(&format!(r#"{{"$":"SettingSet","key":"told.k{}-1","value":"x","at":1}}"#, now - TOLD_KEEP_S - 1)); // 30 days and a second old
        st.apply_line(&format!(r#"{{"$":"SettingSet","key":"told.k{}-1","value":"x","at":1}}"#, now - TOLD_KEEP_S)); // exactly 30 days: kept
        st.apply_line(&format!(r#"{{"$":"SettingSet","key":"told.hand","value":"{{\"at\":{}}}","at":1}}"#, now - TOLD_KEEP_S - 5)); // by hand: its value's time
        for i in 0..2001u64 { st.apply_line(&format!(r#"{{"$":"SettingSet","key":"told.k{}-{i}","value":"x","at":1}}"#, now - 100 - i)); }
        let gone: Vec<String> = told_prune(&st, now).iter().map(|e| e["key"].as_str().unwrap().to_string()).collect();
        assert!(gone.contains(&format!("told.k{}-1", now - TOLD_KEEP_S - 1))); assert!(gone.contains(&"told.hand".to_string()));
        // indexes (newest first): 0..=1999 the recent ones keep; 2000 (the 2001st recent), the exactly-30-days record (2001), the 30-days-and-a-second record
        // and the by-hand one go: the first by the cap, the others by the cap and by age
        assert!(gone.contains(&format!("told.k{}-1", now - TOLD_KEEP_S)), "past the newest 2000 the oldest go even when they are young enough");
        assert_eq!(gone.len(), 4, "{gone:?}");
        assert_eq!(told_prune(&st, now)[0]["value"], "");
    }
    #[test] fn an_opened_ask_gets_its_alert_record() {
        let mut st = State::new();
        st.apply_line(r#"{"$":"SettingSet","key":"alert.t1","value":"done\tturn-t1\tmem1\t1.1\t","at":1}"#);
        let ask = |tool: &str, detail: &str| json!({"$":"AskOpened","id":"q","thread":"t1","kind":"approval","tool":tool,"detail":detail,"options":"","at":50});
        let rec = |c: Value| alert_asks(&st, 9, &[c])[0]["value"].as_str().unwrap().to_string();
        assert_eq!(rec(ask("Bash", "rm -rf x")), "ask\task-t1\tmem1\t50.9\tBash rm -rf x", "kind, key, the settle memory kept, stamp, tool and detail");
        assert_eq!(rec(ask("order_ask", "Ship to?")), "ask\task-t1\tmem1\t50.9\tShip to?", "an order's question is plain words");
        assert_eq!(rec(ask("task_pause", r#"{"question":"Log in?"}"#)), "ask\task-t1\tmem1\t50.9\tLog in?");
        assert_eq!(rec(ask("settings_proposal", "x")), "ask\task-t1\tmem1\t50.9\tA settings proposal needs your review.");
        assert_eq!(rec(ask("Bash", "a\tb\nc")), "ask\task-t1\tmem1\t50.9\tBash a b c", "one line");
        st.apply_line(r#"{"$":"SettingSet","key":"notify.needs","value":"off","at":2}"#);
        assert_eq!(alert_asks(&st, 9, &[ask("Bash", "x")])[0]["value"], "\t\tmem1\t50.9\t", "needs alerts off: the quiet record that keeps the memory");
    }
}

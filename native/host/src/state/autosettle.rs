//! Automatic settling: which completed, idle threads settle by themselves, and
//! which wake subscriptions end. Pure: the caller hands the folded `State` and the time, and commits the events returned.
//! The legacy upgrade (`Auto.upgrade`, `autosettle.version` below "1") is ported too: until it has run, a launched thread's report receipts are
//! read from the launch mails the log holds (`ext.reports`, `Auto.old.reports`), and `upgrade` turns them into `autosettle.delivery.*` and sets
//! the version, as hub does at start. The tick runs it first on such a home.
use super::ext::Sum;
use super::json::*;
use crate::state::{Override, State, Thread, Turn};
use crate::shelf::{shelf_of, Shelf};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

fn lines(text: &str) -> Vec<&str> { text.split('\n').filter(|l| !l.is_empty()).collect() }
fn num_setting(st: &State, key: &str, fallback: u64) -> u64 { nat_read(&st.get(key, "")).unwrap_or(fallback) }
/// Auto.seconds: minutes (at most a year) as seconds
fn seconds(st: &State, key: &str, fallback_min: u64) -> u64 { num_setting(st, key, fallback_min).min(525600) * 60 }
fn keys_pre<'a>(st: &'a State, pre: &str) -> Vec<&'a String> { st.settings.keys().filter(|k| k.starts_with(pre)).collect() }

fn improvement(th: &str) -> bool {
    th.strip_prefix("improvement-").map_or(false, |k| k.chars().count() == 32 && k.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)))
}
/// Auto.seed.id: the prompt id a task-backed child was started with ("t<stamp>-<seq>" has "m<stamp>-<seq+1>")
fn seed_id(known: bool, th: &str) -> String {
    if !known { return String::new() }
    let Some(rest) = th.strip_prefix('t') else { return String::new() };
    let ps: Vec<&str> = rest.split('-').collect();
    if ps.len() != 2 { return String::new() }
    match (nat_read(ps[0]), nat_read(ps[1])) { (Some(_), Some(n)) => format!("m{}-{}", ps[0], n + 1), _ => String::new() }
}
fn seed_for(st: &State, th: &str, workers: &HashSet<String>) -> String {
    let dflt = if improvement(th) { format!("improvement-request-{}", &th[12..]) } else { seed_id(workers.contains(th), th) };
    st.get(&format!("autosettle.seed.{th}"), &dflt)
}
/// Auto.direct.seed: the seed message is not the person's own word
fn direct_seed(id: &str, text_is_direct: bool, seed: &str) -> bool { if !seed.is_empty() && id == seed { false } else { text_is_direct } }

/// Launch.by: who launched a thread (the setting launch.<id> is the report flag and then the launcher)
fn launch_by(st: &State, th: &str) -> String { st.get(&format!("launch.{th}"), "").chars().skip(1).collect() }
fn launch_reports(st: &State, th: &str) -> bool { let v = st.get(&format!("launch.{th}"), ""); v.starts_with('1') && v.chars().count() > 1 }

pub struct Index { workers: HashSet<String>, owned: HashSet<String>, work: HashSet<String> }

/// Auto.index: workers (tasks' children), the threads that hold something open, and the owners those pull in
pub fn index(st: &State) -> Index {
    let mut workers = HashSet::new(); let mut live: HashSet<String> = HashSet::new();
    let mut parents: HashMap<String, String> = HashMap::new();
    for (id, (p, c, terminal, _)) in &st.tasks {
        workers.insert(c.clone());
        if !(*terminal && st.ext.delivered.contains(id)) { live.insert(c.clone()); live.insert(p.clone()); }
        parents.insert(c.clone(), p.clone());
    }
    // locks: holder and waiters
    for name in lines(&st.get("locks.names", "")) {
        let l = parse_or_null(&st.get(&format!("lock.{name}"), ""));
        if let h @ (Value::Object(_) | Value::String(_) | Value::Bool(_) | Value::Number(_) | Value::Array(_)) = gobj(&l, "holder") { live.insert(gstr(h, "thread", "").to_string()); }
        for w in elems(gobj(&l, "waiters")) { live.insert(gstr(w, "thread", "").to_string()); }
    }
    for (th, open) in st.asks.values() { if *open { live.insert(th.clone()); } }
    for k in keys_pre(st, "secret.ask.") { live.insert(gstr(&parse_or_null(&st.get(k, "")), "thread", "").to_string()); }
    for (th, q) in &st.queue { if !q.is_empty() { live.insert(th.clone()); } }
    for (id, t) in &st.threads { if t.turn == Turn::Running || t.pending > 0 { live.insert(id.clone()); } }
    // Work.rows.map: a thread's background rows, watched work and a quote on its way
    let rows = |th: &str| lines(&st.get(&format!("bg.{th}"), "")).len() + lines(&st.get(&format!("watch.{th}"), "")).len() + if st.get(&format!("order.wait.{th}"), "").is_empty() { 0 } else { 1 };
    for (pre, n) in [("bg.", 3), ("watch.", 6), ("order.wait.", 11)] {
        for k in keys_pre(st, pre) { let th = &k[n..]; if !th.is_empty() && rows(th) > 0 { live.insert(th.to_string()); } }
    }
    // the ids of running work: the first field of every bg./watch. row
    let mut work = HashSet::new();
    for pre in ["watch.", "bg."] { for k in keys_pre(st, pre) { for row in lines(&st.get(k, "")) { work.insert(row.split('\t').next().unwrap_or("").to_string()); } } }
    // wake subscriptions: thread waits link, work waits keep the subscriber live while the work runs, unknown kinds hold
    let mut links: HashMap<String, Vec<String>> = HashMap::new();
    for key in keys_pre(st, "wakes.by.") {
        for id in lines(&st.get(key, "")) {
            let o = parse_or_null(&st.get(&format!("wake.{id}"), ""));
            let (kind, target, by) = (gstr(&o, "kind", ""), gstr(&o, "target", ""), gstr(&o, "by", ""));
            match kind {
                "thread" => links.entry(target.to_string()).or_default().insert(0, by.to_string()),
                "work" => if work.contains(target) { live.insert(by.to_string()); },
                "file" | "pr" => {}
                _ => { live.insert(by.to_string()); }
            }
        }
    }
    let parent = |id: &str| { let by = launch_by(st, id); if by.is_empty() { parents.get(id).cloned().unwrap_or_default() } else { by } };
    let mut owned: HashSet<String> = HashSet::new();
    let mut pending: Vec<String> = live.into_iter().collect();
    while let Some(id) = pending.pop() {
        if id.is_empty() || owned.contains(&id) { continue }
        owned.insert(id.clone());
        pending.push(parent(&id));
        if let Some(ns) = links.get(&id) { pending.extend(ns.iter().cloned()); }
    }
    Index { workers, owned, work }
}

/// Auto.allow: a completed, idle, read, reported, quiet thread on the active shelf with no override settles
pub fn allow(turn: Turn, work: bool, ask: bool, unread: bool, delivered: bool, quiet: bool, shelf: Shelf, o: Override) -> bool {
    turn == Turn::Completed && !work && !ask && !unread && delivered && quiet && shelf == Shelf::Active && o == Override::None
}
/// Auto.wake.active.kind: a subscription that still holds its subscriber (work and thread waits while the target owns work; file and pr
/// subscriptions are passive; unknown kinds are held)
pub fn wake_active_kind(kind: &str, target: &str, work: &HashSet<String>, owned: &HashSet<String>) -> bool {
    match kind { "work" => work.contains(target), "thread" => owned.contains(target), "file" | "pr" => false, _ => true }
}
/// Auto.expired.work: a work dependency whose target is no longer running work, past the worker grace
pub fn expired_work(kind: &str, live: bool, old: bool) -> bool { kind == "work" && !live && old }

fn worker(st: &State, th: &str, ix: &Index) -> bool {
    improvement(th) || !launch_by(st, th).is_empty() || !st.get(&format!("thread.parent.{th}"), "").is_empty() || ix.workers.contains(th) || th.starts_with("run") || th.starts_with("mailcoord-")
}
fn addr(st: &State, by: &str) -> String {
    if let Some((bot, _)) = st.bot_threads.iter().find(|(_, t)| t.as_str() == by) { return format!("bot:{bot}") }
    if by.is_empty() || by == "person" || by == "you" { "person".into() } else if by == "backplane" { "backplane".into() } else if by.contains(':') { by.into() } else { format!("thread:{by}") }
}
/// Auto.report: a launched thread reports back to its launcher before it settles
fn reported(st: &State, th: &str, sum: &Sum, activity: u64) -> bool {
    let by = launch_by(st, th);
    if by.is_empty() || !launch_reports(st, th) { return true }
    let after = activity.max(sum.output_at);
    let rep = st.get(&format!("autosettle.report.{th}"), "");
    if !rep.is_empty() && nat_read(&rep).unwrap_or(0) >= after { return true }
    let report = sum.sent.get(&by).copied().unwrap_or(0);
    let a = addr(st, &by);
    let delivery = num_setting(st, &format!("autosettle.delivery.{th}\t{a}"), 0);
    // Auto.old.reports: before the migration the launch mails themselves are the receipts
    let mail = if st.get("autosettle.version", "") != "1" { st.ext.reports.get(&format!("{th}\t{a}")).copied().unwrap_or(0) } else { 0 };
    report.max(mail.max(delivery)) >= after
}
fn snooze(st: &State, th: &str) -> u64 { st.snooze_until.get(th).copied().unwrap_or(0) }

/// Auto.due: the thread settles now
pub fn due(st: &State, th: &str, t: &Thread, now: u64, ix: &Index) -> bool {
    if !(t.turn == Turn::Completed && shelf_of(t) == Shelf::Active && t.override_ == Override::None) { return false }
    let empty = Sum::default(); let sum = st.ext.sum.get(th).unwrap_or(&empty);
    let w = worker(st, th, ix);
    let seed = seed_for(st, th, &ix.workers);
    // personal: one of its own user messages that is the person's and not the seed (the first two are kept)
    let personal = sum.person.iter().any(|id| direct_seed(id, true, &seed));
    let output = st.get(&format!("autosettle.output.{th}"), &sum.output);
    let audience = st.get(&format!("autosettle.audience.{th}"), "");
    let person = personal || audience == "person" || (!sum.found && audience.is_empty());
    let unread = (person || !w) && !output.is_empty() && output != st.get(&format!("autosettle.read.{th}"), "");
    let idle = seconds(st, "settle.idle.minutes", 15);
    let window = if w { idle } else if !unread && st.get("settle.done", "off") == "on" { idle } else { seconds(st, "settle.personal.minutes", 0) };
    let quiet = window > 0 && now >= sum.quiet.max(t.activity).max(num_setting(st, &format!("autosettle.quiet.{th}"), 0).max(snooze(st, th))).saturating_add(window);
    allow(t.turn, ix.owned.contains(th), t.pending > 0, unread, reported(st, th, sum, t.activity), quiet, shelf_of(t), t.override_)
}

/// Auto.go: ThreadSettled for the threads that are due
fn settles(st: &State, now: u64, ix: &Index) -> Vec<Value> {
    st.threads.iter().filter(|(th, t)| due(st, th, t, now, ix)).map(|(th, _)| json!({"$": "ThreadSettled", "id": th, "manual": false, "at": now})).collect()
}
/// Auto.upgrade: on a home below version 1, the settles by the old (mail) receipts, the receipts of threads still on a shelf kept as
/// `autosettle.delivery.*`, and the version. Nothing at version 1.
pub fn upgrade(st: &State, now: u64) -> Vec<Value> {
    if st.get("autosettle.version", "") == "1" { return vec![] }
    let ix = index(st);
    let mut out = settles(st, now, &ix);
    let mut rs: Vec<(&String, &u64)> = st.ext.reports.iter().collect(); rs.sort();
    for (key, at) in rs {
        let th = key.split('\t').next().unwrap_or("");
        let blank = Thread::blank(); let t = st.threads.get(th).unwrap_or(&blank);
        if matches!(shelf_of(t), Shelf::Active | Shelf::Pinned | Shelf::Snoozed) { out.push(set_ev(format!("autosettle.delivery.{key}"), at.to_string(), now)); }
    }
    out.push(set_ev("autosettle.version".into(), "1".into(), now));
    out
}

/// Auto.changes: ThreadSettled for the threads that are due, WakeDropped for the subscriptions that ended
pub fn changes(st: &State, now: u64) -> Vec<Value> {
    let ix = index(st);
    let mut out = settles(st, now, &ix);
    let window = seconds(st, "settle.idle.minutes", 15);
    for key in keys_pre(st, "wakes.by.") {
        let th = &key[9..];
        let blank = Thread::blank(); let t = st.threads.get(th).unwrap_or(&blank);
        let retired = shelf_of(t) == Shelf::Settled && t.turn == Turn::Completed && !ix.owned.contains(th);
        for id in lines(&st.get(key, "")) {
            let o = parse_or_null(&st.get(&format!("wake.{id}"), ""));
            let (kind, target) = (gstr(&o, "kind", ""), gstr(&o, "target", ""));
            let active = wake_active_kind(kind, target, &ix.work, &ix.owned);
            let old = window > 0 && now >= gnat(&o, "created", now).saturating_add(window);
            let drop = if retired { !active } else { expired_work(kind, ix.work.contains(target), old) };
            if drop { out.push(json!({"$": "WakeDropped", "id": id, "at": now})); }
        }
    }
    out
}

fn put(m: &mut HashMap<String, u64>, th: &str, at: u64) { let e = m.entry(th.to_string()).or_insert(0); *e = (*e).max(at); }
fn set_ev(key: String, value: String, at: u64) -> Value { json!({"$": "SettingSet", "key": key, "value": value, "at": at}) }
fn improvement_seed(th: &str) -> String { if improvement(th) { format!("improvement-request-{}", &th[12..]) } else { String::new() } }

/// Shelve.tick.post (as Shelve.auto.tick keeps it, only the wakes): a settled thread that works again wakes, unless its project is dormant. A manual
/// settle holds until a message came after it; an automatic one never holds against work. `working` is Lifecycle.working: a turn, a pending ask
/// count, a queued message, unfinished child tasks or rows.
pub fn tick_post(st: &State, now: u64) -> Vec<Value> {
    let mut out: Vec<(&String, &Thread)> = st.threads.iter().filter(|(_, t)| {
        if shelf_of(t) != Shelf::Settled { return false }
        let lapsed = if t.override_ == Override::ForcedSettled { t.unsettled_at > t.settled_at } else { true };
        lapsed
    }).collect();
    out.sort_by(|a, b| a.0.cmp(b.0));
    out.into_iter().filter(|(id, t)| working(st, id, t) && st.get(&format!("project.dormant.{}", t.project), "") != "on")
        .map(|(id, _)| json!({"$": "ThreadWoke", "id": id, "at": now})).collect()
}
fn working(st: &State, id: &str, t: &Thread) -> bool {
    let rows = |k: &str| st.get(&format!("{k}{id}"), "").split('\n').filter(|l| !l.is_empty()).count();
    let waiting = st.queue.get(id).map_or(false, |q| q.iter().any(|(m, _)| !m.starts_with("handoff:")));
    let busy = super::ext::open_kids(st, id) + rows("bg.") + rows("watch.") + if st.get(&format!("order.wait.{id}"), "").is_empty() { 0 } else { 1 };
    t.turn == Turn::Running || t.pending > 0 || waiting || busy > 0
}

/// Shelve.auto.messages: a settled thread reopens (ThreadWoke) when a message is posted to it, unless the settle is a manual one made after the message,
/// the batch settles it again (Shelve.held: a ThreadSettled or ThreadMerged for it), or the message was already posted (Shelve.input.seen). `pre` is the
/// state before the batch; `list` the batch as it stands. The step's own wakes (Shelve.step: a turn starting, work out again...) are not ported yet.
pub fn reopen(pre: &State, list: &[Value]) -> Vec<Value> {
    let mut out = vec![];
    for c in list {
        if gstr(c, "$", "") != "MessagePosted" { continue }
        let (th, id, at) = (gstr(c, "thread", ""), gstr(c, "msg", ""), gnat(c, "at", 0));
        let Some(t) = pre.threads.get(th) else { continue };
        if shelf_of(t) != Shelf::Settled { continue }
        let recent = if t.override_ == Override::ForcedSettled { at >= t.settled_at } else { true };
        let held = list.iter().any(|x| matches!(gstr(x, "$", ""), "ThreadSettled" | "ThreadMerged") && gstr(x, "id", "") == th);
        let seen = pre.ext.sum.get(th).map_or(false, |s| s.recent_msgs.iter().any(|m| m == id));
        if recent && !held && !seen { out.push(json!({"$": "ThreadWoke", "id": th, "at": at})); }
    }
    out
}

/// Auto.read: the person read the thread's newest reply (it opened in a viewer): the mark that stops it counting as unread. `st` is current.
pub fn read(st: &State, th: &str, now: u64) -> Vec<Value> {
    let output = st.get(&format!("autosettle.output.{th}"), st.ext.sum.get(th).map_or("", |s| s.output.as_str()));
    if !output.is_empty() && output != st.get(&format!("autosettle.read.{th}"), "") { vec![set_ev(format!("autosettle.read.{th}"), output, now)] } else { vec![] }
}

/// Auto.message.metadata: what a committed change leaves behind for the settle decision. `st` is the state before the change.
pub fn metadata(st: &State, c: &Value) -> Vec<Value> {
    let at = gnat(c, "at", 0);
    match gstr(c, "$", "") {
        "TaskDelegated" => { let child = gstr(c, "child", ""); vec![set_ev(format!("autosettle.seed.{child}"), seed_id(true, child), at)] }
        "MessagePosted" => {
            let (th, id) = (gstr(c, "thread", ""), gstr(c, "msg", ""));
            match gstr(c, "role", "") {
                "user" => {
                    let seed = st.get(&format!("autosettle.seed.{th}"), &improvement_seed(th));
                    let personal = direct_seed(id, super::ext::direct(id, gstr(c, "text", "")), &seed);
                    if personal || st.get(&format!("autosettle.audience.{th}"), "").is_empty() {
                        vec![set_ev(format!("autosettle.audience.{th}"), if personal { "person" } else { "worker" }.into(), at)]
                    } else { vec![] }
                }
                "assistant" => vec![set_ev(format!("autosettle.output.{th}"), id.into(), at)],
                _ => vec![],
            }
        }
        _ => vec![],
    }
}
/// Auto.quiet.collect + Auto.quiet.changes: a thread's quiet mark moves to the time of a change that shows it is alive.
/// `st` is the state before the batch. Sorted by thread for a stable order.
pub fn quiet(st: &State, batch: &[Value]) -> Vec<Value> {
    let mut m: HashMap<String, u64> = HashMap::new();
    let lock_parties = |m: &mut HashMap<String, u64>, l: &Value, at: u64| {
        if !matches!(gobj(l, "holder"), Value::Null) { put(m, gstr(gobj(l, "holder"), "thread", ""), at); }
        for w in elems(gobj(l, "waiters")) { put(m, gstr(w, "thread", ""), at); }
    };
    for c in batch {
        let at = gnat(c, "at", 0);
        match gstr(c, "$", "") {
            "MessagePosted" | "PendingChanged" => put(&mut m, gstr(c, "thread", ""), at),
            "WorkSet" => { let (th, kind) = (gstr(c, "thread", ""), gstr(c, "kind", "")); if gstr(c, "rows", "") != st.get(&format!("{kind}.{th}"), "") { put(&mut m, th, at); } }
            "TaskStateSet" | "TaskDelivered" => {
                let id = gstr(c, "task", "");
                let (p, ch) = st.tasks.get(id).map(|t| (t.0.clone(), t.1.clone())).unwrap_or_default();
                put(&mut m, &p, at); put(&mut m, &ch, at);
            }
            "AskClosed" => { let id = gstr(c, "id", ""); if let Some((th, _)) = st.asks.get(id) { put(&mut m, th, at); } }
            "WakeDropped" => { let by = gstr(&parse_or_null(&st.get(&format!("wake.{}", gstr(c, "id", "")), "")), "by", "").to_string(); put(&mut m, &by, at); }
            "LockChanged" => {
                let old = parse_or_null(&st.get(&format!("lock.{}", gstr(c, "name", "")), ""));
                lock_parties(&mut m, &old, at); lock_parties(&mut m, gobj(c, "lock"), at);
            }
            _ => {}
        }
    }
    let mut out: Vec<(String, u64)> = m.into_iter().filter(|(th, at)| !th.is_empty() && *at > num_setting(st, &format!("autosettle.quiet.{th}"), 0)).collect();
    out.sort();
    out.into_iter().map(|(th, at)| set_ev(format!("autosettle.quiet.{th}"), at.to_string(), at)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    const TURNS: [Turn; 5] = [Turn::Idle, Turn::Running, Turn::Completed, Turn::Interrupted, Turn::Failed];
    const SHELVES: [Shelf; 5] = [Shelf::Pinned, Shelf::Active, Shelf::Settled, Shelf::Hidden, Shelf::Snoozed];
    const OVERRIDES: [Override; 3] = [Override::None, Override::ForcedSettled, Override::ForcedActive];
    const B: [bool; 2] = [false, true];
    fn each(f: impl Fn(bool, bool, bool, bool)) { for a in B { for b in B { for c in B { for d in B { f(a, b, c, d) } } } } }

    #[test] fn autosettle_owned_work_held() { for t in TURNS { each(|a, u, d, q| { for s in SHELVES { for o in OVERRIDES { assert!(!allow(t, true, a, u, d, q, s, o)); } } }); } }
    #[test] fn autosettle_pending_ask_held() { each(|u, d, q, _| assert!(!allow(Turn::Completed, false, true, u, d, q, Shelf::Active, Override::None))); }
    #[test] fn autosettle_unread_held() { each(|d, q, _, _| assert!(!allow(Turn::Completed, false, false, true, d, q, Shelf::Active, Override::None))); }
    #[test] fn autosettle_failed_held() { each(|w, a, u, d| for q in B { assert!(!allow(Turn::Failed, w, a, u, d, q, Shelf::Active, Override::None)); }); }
    #[test] fn autosettle_interrupted_held() { each(|w, a, u, d| for q in B { assert!(!allow(Turn::Interrupted, w, a, u, d, q, Shelf::Active, Override::None)); }); }
    #[test] fn autosettle_report_held() { for q in B { assert!(!allow(Turn::Completed, false, false, false, false, q, Shelf::Active, Override::None)); } }
    #[test] fn autosettle_grace_held() { assert!(!allow(Turn::Completed, false, false, false, true, false, Shelf::Active, Override::None)); }
    #[test] fn autosettle_ready() { assert!(allow(Turn::Completed, false, false, false, true, true, Shelf::Active, Override::None)); }
    #[test] fn autosettle_allow_needs_every_other_condition() { // beyond the laws: only the one input combination settles
        let mut ok = 0;
        for t in TURNS { for s in SHELVES { for o in OVERRIDES { each(|w, a, u, d| for q in B { if allow(t, w, a, u, d, q, s, o) { assert!(t == Turn::Completed && s == Shelf::Active && o == Override::None && !w && !a && !u && d && q); } }); ok += 1; } } }
        assert_eq!(ok, 75);
    }
    #[test] fn autosettle_delegation_seed_not_personal() {
        for text in ["", "hello", "[message from person@box] hi", "More work"] { assert!(!direct_seed("m100-4", super::super::ext::direct("m100-4", text), "m100-4")); }
    }
    #[test] fn autosettle_human_followup_is_personal() { assert!(direct_seed("human-followup", super::super::ext::direct("human-followup", "More work"), "m100-4")); }
    #[test] fn autosettle_finished_work_wake_not_running() {
        let none = HashSet::new();
        for owned in [HashSet::new(), ["ended".to_string()].into_iter().collect(), ["x".to_string()].into_iter().collect()] { assert!(!wake_active_kind("work", "ended", &none, &owned)); }
    }
    #[test] fn autosettle_live_thread_wake_held() {
        let owned: HashSet<String> = ["t".to_string()].into_iter().collect();
        assert!(wake_active_kind("thread", "t", &HashSet::new(), &owned));
    }
    #[test] fn autosettle_live_work_subscription_never_expires() { for old in B { assert!(!expired_work("work", true, old)); } }
    #[test] fn autosettle_active_file_subscription_never_expires() { for live in B { for old in B { assert!(!expired_work("file", live, old)); } } }
    #[test] fn a_settled_thread_that_works_wakes_unless_its_project_is_dormant() {
        let mk = |id: &str, p: &str| format!(r#"{{"$":"ThreadCreated","id":"{id}","project":"{p}","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}}"#);
        let mut st = State::new();
        for l in [mk("auto", "p"), mk("manual", "p"), mk("msg", "p"), mk("idle", "p"), mk("dorm", "pd"), mk("queued", "p")] { st.apply_line(&l); }
        for (id, manual, at) in [("auto", false, 5), ("manual", true, 5), ("msg", true, 5), ("idle", false, 5), ("dorm", false, 5), ("queued", false, 5)] {
            st.apply_line(&format!(r#"{{"$":"ThreadSettled","id":"{id}","manual":{manual},"at":{at}}}"#));
        }
        for id in ["auto", "manual", "msg", "dorm"] { st.apply_line(&format!(r#"{{"$":"TurnChanged","thread":"{id}","state":"running","at":6}}"#)); }
        st.apply_line(r#"{"$":"TurnQueued","thread":"queued","msg":"q1","text":"x","at":6}"#);
        st.apply_line(r#"{"$":"MessagePosted","thread":"msg","msg":"m9","role":"user","text":"more","at":7}"#); // after the manual settle: it gives way
        st.apply_line(r#"{"$":"SettingSet","key":"project.dormant.pd","value":"on","at":1}"#);
        let woke: Vec<String> = tick_post(&st, 50).iter().map(|e| e["id"].as_str().unwrap().to_string()).collect();
        assert_eq!(woke, vec!["auto".to_string(), "msg".to_string(), "queued".to_string()], "auto (never holds), msg (a message after the manual settle), queued; not manual (no message since), idle, or dormant");
    }
    #[test] fn the_legacy_upgrade_turns_launch_mails_into_receipts_and_sets_the_version() {
        let mk = |id: &str| format!(r#"{{"$":"ThreadCreated","id":"{id}","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}}"#);
        let mut st = State::new();
        for l in [mk("boss"), mk("w1"), mk("w2")] { st.apply_line(&l); }
        for w in ["w1", "w2"] {
            st.apply_line(&format!(r#"{{"$":"ThreadLaunched","id":"{w}","by":"boss","report":true,"at":2}}"#));
            st.apply_line(&format!(r#"{{"$":"MessagePosted","thread":"{w}","msg":"m{w}","role":"user","text":"go","at":3}}"#));
            st.apply_line(&format!(r#"{{"$":"MessagePosted","thread":"{w}","msg":"a{w}","role":"assistant","text":"done","at":4}}"#));
            st.apply_line(&format!(r#"{{"$":"TurnChanged","thread":"{w}","state":"completed","at":5}}"#));
            st.apply_line(&format!(r#"{{"$":"SettingSet","key":"autosettle.read.{w}","value":"a{w}","at":5}}"#));
        }
        // w1's launch mail came after its activity (a receipt), w2 has none
        st.apply_line(r#"{"$":"MailPosted","id":"t:w1:lr-1","conv":"c","from":"thread:w1","to":"thread:boss","kind":"launch","ref":"","want":"","head":"","size":0,"at":9}"#);
        st.apply_line(r#"{"$":"MailPosted","id":"t:w1:x","conv":"c","from":"thread:w1","to":"thread:boss","kind":"fyi","ref":"","want":"","head":"","size":0,"at":99}"#); // not a launch mail
        let up = upgrade(&st, 100_000);
        let settled: Vec<&str> = up.iter().filter(|e| e["$"] == "ThreadSettled").map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(settled, vec!["w1"], "w2 never reported; w1's launch mail is its receipt");
        assert!(up.iter().any(|e| e["key"] == "autosettle.delivery.w1\tthread:boss" && e["value"] == "9"));
        assert_eq!(up.last().unwrap()["key"], "autosettle.version"); assert_eq!(up.last().unwrap()["value"], "1");
        st.apply_line(r#"{"$":"SettingSet","key":"autosettle.version","value":"1","at":9}"#);
        assert!(upgrade(&st, 100_000).is_empty(), "nothing at version 1");
        assert!(!changes(&st, 100_000).iter().any(|e| e["id"] == "w1" && e["$"] == "ThreadSettled"), "at version 1 the mail is no longer read: only the delivery setting counts, and it is not set here");
    }
    #[test] fn a_message_reopens_a_settled_thread_unless_held_seen_or_older_than_a_manual_settle() {
        let mk = |id: &str| format!(r#"{{"$":"ThreadCreated","id":"{id}","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}}"#);
        let mut st = State::new();
        for id in ["auto", "manual", "active", "seen"] { st.apply_line(&mk(id)); }
        st.apply_line(r#"{"$":"MessagePosted","thread":"seen","msg":"dup","role":"user","text":"x","at":2}"#);
        for (id, manual) in [("auto", false), ("manual", true), ("seen", false)] { st.apply_line(&format!(r#"{{"$":"ThreadSettled","id":"{id}","manual":{manual},"at":10}}"#)); }
        let m = |th: &str, id: &str, at: u64| json!({"$":"MessagePosted","thread":th,"msg":id,"role":"assistant","text":"x","at":at});
        let woke = |l: &[Value]| reopen(&st, l).iter().map(|e| e["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(woke(&[m("auto", "a1", 11), m("active", "a2", 11), m("seen", "dup", 11), m("manual", "a3", 9), m("manual", "a4", 10)]), vec!["auto", "manual"], "auto: yes; active: not settled; seen: already posted; manual before its settle: no; at its settle second: yes");
        assert!(woke(&[m("auto", "a1", 11), json!({"$":"ThreadSettled","id":"auto","manual":false,"at":11})]).is_empty(), "the batch settles it again: held");
        assert!(woke(&[m("ghost", "a1", 11)]).is_empty());
    }
    #[test] fn auto_read_marks_the_newest_reply_once() {
        let mut st = State::new();
        st.apply_line(r#"{"$":"MessagePosted","thread":"t1","msg":"a1","role":"assistant","text":"x","at":5}"#);
        let ev = read(&st, "t1", 9); assert_eq!(ev.len(), 1);
        assert_eq!((ev[0]["key"].as_str(), ev[0]["value"].as_str()), (Some("autosettle.read.t1"), Some("a1")));
        st.apply(&ev[0]); assert!(read(&st, "t1", 10).is_empty());
        assert!(read(&st, "nothing", 10).is_empty());
    }
}

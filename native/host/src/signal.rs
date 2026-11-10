//! The thread signal: what a sidebar dot and status word say (src/core/signal `Signal.index`, `Signal.name`,
//! `Signal.display.name`). The index is a pure function of threads, tasks, settings, queues and asks.
//! Values: bits 1 = actual work, 2 = needs the person; 3 = both; 4 settled; 5 finished; 6 waiting (active only through
//! owned work or children's); 7 waiting (finished or idle with something still out).
use crate::shelf::{shelf_of, Shelf};
use crate::state::{State, Thread, Turn};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

pub fn compose(blocked: bool, active: bool, settled: bool) -> u8 {
    match (blocked, active, settled) { (true, true, _) => 3, (true, false, _) => 2, (false, true, _) => 1, (false, false, true) => 4, (false, false, false) => 0 }
}
/// Signal.done: a finished thread (last turn completed, nothing queued) that has nothing else to show
pub fn done(finished: bool, n: u8) -> u8 { if finished && n == 0 { 5 } else { n } }
pub fn bits(n: u8) -> u8 { if n < 4 { n } else if n == 6 { 1 } else { 0 } }
pub fn waits_is(n: u8) -> bool { n == 6 || n == 7 }
pub fn active(n: u8) -> bool { bits(n) % 2 == 1 }
pub fn blocked(n: u8) -> bool { bits(n) >= 2 }
pub fn merge(a: u8, b: u8) -> u8 { (if active(a) || active(b) { 1 } else { 0 }) + (if blocked(a) || blocked(b) { 2 } else { 0 }) }
/// Signal.out.k: idle (0) or finished (5) with work out waits (7)
pub fn out_k(out: bool, n: u8) -> u8 { if out && (n == 0 || n == 5) { 7 } else { n } }
/// Signal.waits: active (1) with its own turn not running waits (6) on the work it started or its children's
pub fn waits(n: u8, running: bool) -> u8 { if n == 1 && !running { 6 } else { n } }
/// Signal.name: the word, which is also the colour everywhere
pub fn name(n: u8) -> &'static str {
    if blocked(n) { "blocked" } else if waits_is(n) { "waiting" } else if active(n) { "active" } else if n == 4 { "settled" } else if n == 5 { "finished" } else { "inactive" }
}
/// Signal.shown.k: how the last turn ended can say more than the signal
pub fn shown_k(name: &'static str, turn: Turn, pending: u64) -> &'static str {
    match turn { Turn::Interrupted if name == "inactive" => "stopped", Turn::Failed if name == "blocked" && pending == 0 => "failed", _ => name }
}
/// Signal.dot.name: a failed turn keeps the interaction colour with its own truthful name
pub fn dot_name(name: &'static str, turn: Turn, pending: u64) -> &'static str { if shown_k(name, turn, pending) == "failed" { "failed" } else { name } }

// ---- background rows ("bg.<thread>", "watch.<thread>"): lines of tab-separated fields, the second says what the row does
const ENDED: &str = "\u{1}ended:";
fn lines(text: &str) -> Vec<&str> { text.split('\n').filter(|l| !l.is_empty()).collect() }
fn doing(l: &str) -> &str { l.split('\t').nth(1).unwrap_or("") }
fn ended(l: &str) -> bool { doing(l).starts_with(ENDED) }
fn running(ls: &[&str]) -> usize { ls.iter().filter(|l| !ended(l)).count() }
/// Signal.watch: watching/cancelling rows confirm an owned process; launching/queued/unknown do not
fn watching(ls: &[&str]) -> bool { ls.iter().any(|l| !ended(l) && (doing(l).starts_with("watching") || doing(l) == "cancelling")) }
fn held(q: &[(String, String)]) -> bool { q.iter().any(|(id, _)| id.starts_with("fallback:held:")) }

/// Liveness.read: the stalled flag of "liveness.<thread>"
pub fn stalled(st: &State, id: &str) -> bool {
    let hidden = st.threads.get(id).map_or(true, |t| t.archived || t.deleted);
    !hidden && st.settings.get(&format!("liveness.{id}")).and_then(|v| serde_json::from_str::<Value>(v).ok()).and_then(|v| v.get("stalled").and_then(|b| b.as_bool())).unwrap_or(false)
}

/// The setting `<prefix><id>`, its key written into `key` (one buffer for a whole pass over the threads: the index asks five settings of every thread).
fn setting_in<'a>(st: &'a State, key: &mut String, prefix: &str, id: &str) -> &'a str { key.clear(); key.push_str(prefix); key.push_str(id); st.settings.get(key.as_str()).map(|s| s.as_str()).unwrap_or("") }

/// Signal.own: one thread's value from its own state
fn own(st: &State, asks: &HashMap<&str, u8>, kids: &HashMap<&str, u32>, id: &str, t: &Thread, key: &mut String) -> u8 {
    if t.deleted { return 0 }
    let bg = lines(setting_in(st, key, "bg.", id)); let watch = lines(setting_in(st, key, "watch.", id));
    let owned = running(&bg) > 0 || watching(&watch);
    let q: &[(String, String)] = st.queue.get(id).map(|v| v.as_slice()).unwrap_or(&[]);
    let is_blocked = t.turn == Turn::Failed || t.pending > 0 || blocked(asks.get(id).copied().unwrap_or(0)) || !setting_in(st, key, "auth.wait.", id).is_empty() || held(q);
    let is_active = owned || (t.turn == Turn::Running && !is_blocked);
    let settled = shelf_of(t) == Shelf::Settled;
    let base = done(t.turn == Turn::Completed && q.is_empty(), compose(is_blocked, is_active, settled));
    // Signal.out: what the hub still waits on, confirmed running or not
    let out = kids.get(id).copied().unwrap_or(0) > 0 || !bg.is_empty() || !watch.is_empty() || !setting_in(st, key, "order.wait.", id).is_empty();
    out_k(out, base)
}

/// Signal.index: every thread's signal. Own values first, then bits flow from children (delegated tasks, launches) to their parents.
pub fn index(st: &State) -> HashMap<String, u8> {
    let mut asks: HashMap<&str, u8> = HashMap::new();
    for (th, open) in st.asks.values() { if *open { asks.insert(th.as_str(), 2); } }
    let mut kids: HashMap<&str, u32> = HashMap::new();
    for (p, _, term, _) in st.tasks.values() { if !*term { *kids.entry(p.as_str()).or_default() += 1; } }
    let mut ids: Vec<&String> = st.threads.keys().collect(); ids.sort();
    let mut m: HashMap<String, u8> = HashMap::with_capacity(ids.len()); let mut key = String::new();
    for id in &ids { m.insert((*id).clone(), own(st, &asks, &kids, id, &st.threads[*id], &mut key)); }
    // edges child -> parents (newest edge first, as Signal.edge prepends)
    let mut parents: HashMap<String, Vec<String>> = HashMap::new();
    let mut tasks: Vec<_> = st.tasks.values().collect(); tasks.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    for (p, c, term, _) in tasks { if !*term && m.contains_key(c) && m.contains_key(p) { parents.entry(c.clone()).or_default().insert(0, p.clone()); } }
    for id in &ids { let by = setting_in(st, &mut key, "launch.", id); let by = by.get(1..).unwrap_or(""); if !by.is_empty() && m.contains_key(by) { parents.entry((*id).clone()).or_default().insert(0, by.to_string()); } }
    // drain: each node can acquire each of two bits once, so three visits per node suffice (the fuel also stops malformed cycles)
    // (only a thread with parents can pass anything on, and the queue holds names, not copies: most homes have a thousand threads and a handful of edges)
    let mut queue: VecDeque<&str> = ids.iter().map(|s| s.as_str()).collect();
    let mut fuel = 3 * ids.len();
    while fuel > 0 {
        let Some(id) = queue.pop_front() else { break };
        fuel -= 1;
        let Some(ps) = parents.get(id) else { continue };
        let b = bits(m[id]);
        for p in ps {
            let old = m.get(p).copied().unwrap_or(0); let value = merge(old, b);
            if bits(old) != value { if let Some(v) = m.get_mut(p) { *v = value; } else { m.insert(p.clone(), value); } queue.push_front(p.as_str()); }
        }
    }
    // Signal.waits: active (1) with its own turn not running waits (6) on the work it started or its children's
    for id in &ids { if let Some(v) = m.get_mut(id.as_str()) { *v = waits(*v, st.threads[*id].turn == Turn::Running); } }
    m
}

/// Signal.display.name: "stalled" when the liveness overlay says so, else the dot's word
pub fn display_name(st: &State, n: u8, id: &str) -> &'static str {
    if stalled(st, id) { return "stalled" }
    match st.threads.get(id) { Some(t) => dot_name(name(n), t.turn, t.pending), None => name(n) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fold(lines: &[&str]) -> State { let mut s = State::new(); for l in lines { s.apply_line(l); } s }
    const PC: &str = r#"{"$":"ProjectCreated","id":"p1","title":"proj","root":"/r","at":10}"#;
    fn tc(id: &str) -> String { format!(r#"{{"$":"ThreadCreated","id":"{id}","project":"p1","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":11}}"#) }

    // ---- laws, restated ----
    #[test] fn signal_finished_named() { assert_eq!(name(done(true, 0)), "finished"); }
    #[test] fn signal_finished_keeps_rest() { assert_eq!([1, 2, 3, 4].map(|n| done(true, n)), [1, 2, 3, 4]); }
    #[test] fn signal_finished_needs_completed() { for n in 0..=255u8 { assert_eq!(done(false, n), n); } }
    #[test] fn signal_finished_quiet() { assert!(!(active(5) || blocked(5))); }
    #[test] fn signal_waiting_named() { assert_eq!([name(6), name(7)], ["waiting", "waiting"]); }
    #[test] fn signal_waiting_active() { assert_eq!([active(6), blocked(6), active(7), blocked(7)], [true, false, false, false]); }
    #[test] fn signal_waiting_not_finished() { assert_eq!([out_k(true, done(true, 0)), out_k(true, 0)], [7, 7]); }
    #[test] fn signal_waiting_keeps_rest() { assert_eq!([1, 2, 3, 4].map(|n| out_k(true, n)), [1, 2, 3, 4]); }
    #[test] fn signal_waiting_turn_over() { assert_eq!([waits(1, false), waits(1, true), waits(3, false)], [6, 1, 3]); }
    #[test] fn signal_failed_dot_truthful() { assert_eq!(dot_name("blocked", Turn::Failed, 0), "failed"); }
    #[test] fn signal_asked_dot_stays_blocked() { assert_eq!(dot_name("blocked", Turn::Failed, 1), "blocked"); }
    #[test] fn signal_failed_display_name() {
        let s = fold(&[&tc("t"), r#"{"$":"TurnChanged","thread":"t","state":"failed","at":2}"#]); assert_eq!(display_name(&s, 2, "t"), "failed");
    }
    #[test] fn signal_pending_display_blocked() {
        let s = fold(&[&tc("t"), r#"{"$":"TurnChanged","thread":"t","state":"failed","at":2}"#, r#"{"$":"PendingChanged","thread":"t","count":1,"at":3}"#]); assert_eq!(display_name(&s, 2, "t"), "blocked");
    }
    #[test] fn the_value_table_names_its_words() {
        for (n, w) in [(0, "inactive"), (1, "active"), (2, "blocked"), (3, "blocked"), (4, "settled"), (5, "finished"), (6, "waiting"), (7, "waiting")] { assert_eq!(name(n), w, "n={n}"); }
        assert_eq!((bits(6), bits(7), bits(4), bits(5), bits(3)), (1, 0, 0, 0, 3));
        assert_eq!(merge(5, 1), 1); assert_eq!(merge(0, 2), 2); assert_eq!(merge(1, 2), 3);
    }
    #[test] fn failure_and_stop_keep_their_own_words() {
        assert_eq!(dot_name("blocked", Turn::Failed, 0), "failed"); assert_eq!(dot_name("blocked", Turn::Failed, 1), "blocked");
        assert_eq!(dot_name("inactive", Turn::Interrupted, 0), "inactive"); assert_eq!(shown_k("inactive", Turn::Interrupted, 0), "stopped");
    }
    #[test] fn a_running_thread_is_active_and_a_completed_one_finished_then_waiting_when_work_is_out() {
        let s = fold(&[PC, &tc("t1"), r#"{"$":"TurnChanged","thread":"t1","state":"running","at":12}"#]);
        assert_eq!(index(&s)["t1"], 1);
        let s = fold(&[PC, &tc("t1"), r#"{"$":"TurnChanged","thread":"t1","state":"completed","at":12}"#]); assert_eq!(index(&s)["t1"], 5);
        let s = fold(&[PC, &tc("t1"), r#"{"$":"TurnChanged","thread":"t1","state":"completed","at":12}"#, r#"{"$":"WorkSet","thread":"t1","kind":"bg","rows":"a\tx","at":13}"#]);
        assert_eq!(index(&s)["t1"], 6, "a running background row is owned work; the turn is over, so it waits");
        assert_eq!(display_name(&s, index(&s)["t1"], "t1"), "waiting");
    }
    #[test] fn an_open_ask_blocks_and_a_failed_turn_without_one_is_failed() {
        let s = fold(&[PC, &tc("t1"), r#"{"$":"AskOpened","id":"a1","thread":"t1","kind":"approval","tool":"Bash","detail":"","options":"","at":12}"#]);
        assert_eq!(name(index(&s)["t1"]), "blocked");
        let s = fold(&[PC, &tc("t1"), r#"{"$":"TurnChanged","thread":"t1","state":"failed","at":12}"#]);
        assert_eq!(display_name(&s, index(&s)["t1"], "t1"), "failed");
    }
    #[test] fn a_child_task_lights_its_parent_and_a_finished_one_does_not() {
        let d = r#"{"$":"TaskDelegated","task":"k1","parent":"t1","child":"t2","depth":1,"provider":"claude","model":"","at":12}"#;
        let s = fold(&[PC, &tc("t1"), &tc("t2"), d, r#"{"$":"TurnChanged","thread":"t2","state":"running","at":13}"#]);
        let ix = index(&s); assert_eq!(ix["t2"], 1); assert_eq!(name(ix["t1"]), "waiting");
        let s = fold(&[PC, &tc("t1"), &tc("t2"), d, r#"{"$":"TurnChanged","thread":"t2","state":"running","at":13}"#, r#"{"$":"TaskStateSet","task":"k1","state":"done","summary":"","at":14}"#]);
        assert_eq!(index(&s)["t1"], 0);
    }
}

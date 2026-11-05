//! The keys of `shown.thread` that follow the thread's own state (src/mobile/screen.bend `Mob.thread`): `state`, `tools` (core/tools.bend), `menu`,
//! `working` and `phase` (core/signal.bend), computed from `state::State` for a thread with no asks, task links, queue, background rows or
//! launch (`plain_signal` is `signal::own` for exactly that case). `None`: a case this does not compute (a failed or stopped turn, a stalled thread,
//! a forked thread whose fork record a lean State may have dropped): the caller refuses it, retryably.
use crate::shelf::{shelf_of, Shelf};
use crate::signal;
use crate::state::{Override, State, Turn};
use serde_json::{json, Map, Value};

/// Mob.state
pub fn state_word(t: Turn) -> &'static str { match t { Turn::Running => "run", Turn::Failed => "fail", Turn::Interrupted => "stop", _ => "idle" } }

fn tool(icon: &str, label: &str, tip: &str, action: &str, value: &str, on: bool, danger: bool) -> Value {
    json!({"icon": icon, "label": label, "tip": tip, "action": action, "value": value, "on": on, "danger": danger})
}

/// Tools.settled: settled by hand, or by itself (an answer the person has seen)
pub fn settled(st: &State, id: &str) -> bool {
    let Some(t) = st.threads.get(id) else { return false };
    match t.override_ { Override::ForcedSettled => true, Override::None => t.settled_at > t.unsettled_at, Override::ForcedActive => false }
}

/// Mob.head.loc: the turn's own button, pin, settle, archive, fork, and "back" for a forked thread (English)
pub fn tools(turn: Turn, pinned: bool, settled: bool, archived: bool, forked: bool) -> Vec<Value> {
    let mut v = Vec::new();
    match turn {
        Turn::Running => v.push(tool("stop", "Stop", "Stop the running turn · Ctrl+.", "interrupt", "", false, true)),
        Turn::Failed => v.push(tool("repeat", "Retry", "The turn failed: send your last message again", "retry", "", false, false)),
        _ => {}
    }
    v.push(if pinned { tool("pin", "Pinned", "Unpin: it sorts with the rest again", "unpin", "", true, false) } else { tool("pin", "Pin", "Keep it at the top of the list", "pin", "", false, false) });
    v.push(if settled { tool("circle-check", "Settled", "Bring it back to the active threads", "unsettle", "", true, false) } else { tool("circle-check", "Settle", "Done for now: it leaves the active threads", "settle", "", false, false) });
    v.push(if archived { tool("archive", "Archived", "Bring it back from the archive", "unarchive", "", true, false) } else { tool("archive", "Archive", "Put it away in the archive", "archive", "", false, false) });
    v.push(tool("git-fork", "Fork thread", "A new thread that goes on from here", "fork", "", false, false));
    if forked { v.push(tool("undo", "Back to the original", "Go back to the thread this one forked from", "back", "", false, false)) }
    v
}

fn item(icon: &str, label: &str, action: &str, value: &str, on: bool, danger: bool, options: Value) -> Value {
    json!({"icon": icon, "label": label, "action": action, "value": value, "on": on, "danger": danger, "options": options})
}
/// Mob.menu.loc without the project card's "mini" items: diff, terminal, find, snooze (or wake), delete
pub fn menu(id: &str, snoozed: bool) -> Vec<Value> {
    vec![
        item("diff", "Diff", "diff", "", false, false, json!([])),
        item("terminal", "Terminal", "term-toggle", "", false, false, json!([])),
        item("search", "Find file", "find-open", "files", false, false, json!([])),
        if snoozed { item("clock", "Snoozed", "snooze", "off", true, false, json!([])) }
        else { item("clock", "Snooze", "snooze", "", false, false, json!([{"label": "1 hour", "value": "1h"}, {"label": "Tomorrow", "value": "tomorrow"}, {"label": "1 week", "value": "1w"}])) },
        item("trash", "Delete", "row-delete", id, false, true, json!([])),
    ]
}

/// signal::own for a thread with no deleted flag, asks, pending, queue, bg/watch rows, children or order wait: its own turn and shelf alone
fn plain_signal(st: &State, id: &str) -> Option<u8> {
    let t = st.threads.get(id)?;
    if t.deleted || t.pending > 0 || st.queue.get(id).map_or(false, |q| !q.is_empty()) { return None }
    for k in ["bg", "watch", "auth.wait", "order.wait", "launch"] { if st.settings.get(&format!("{k}.{id}")).map_or(false, |v| !v.is_empty()) { return None } }
    if st.asks.values().any(|(th, _)| th == id) || st.tasks.values().any(|(p, c, _, _)| p == id || c == id) { return None }
    let blocked = t.turn == Turn::Failed;
    let active = t.turn == Turn::Running && !blocked;
    let settled = shelf_of(t) == Shelf::Settled;
    Some(signal::done(t.turn == Turn::Completed, signal::compose(blocked, active, settled)))
}

/// the sentence of Signal.label for a name (English); failed/stopped are not computed here
fn label(name: &str) -> Option<&'static str> {
    Some(match name {
        "active" => "Working: agent or owned work active", "inactive" => "Inactive: no confirmed active work", "finished" => "Finished: ready for you to look at",
        "settled" => "Settled", "waiting" => "Waiting: on subagents or work it started", "blocked" => "Blocked: needs interaction", _ => return None })
}

/// `state`, `tools`, `menu`, `working`, `phase` for a thread at clock `now_s`, or None for a case not computed here.
pub fn keys(st: &State, id: &str, now_s: u64, why: &str) -> Option<Map<String, Value>> {
    let t = st.threads.get(id)?;
    if signal::stalled(st, id) { return None }
    // a lean State may have dropped the fork record: then "forked" cannot be told
    let fork_key = format!("fork.{id}");
    if st.lean && !crate::state::fam::policy::lean_keeps(&fork_key) { return None }
    let forked = st.settings.get(&fork_key).map_or(false, |v| !v.is_empty());
    let n = plain_signal(st, id)?;
    let phase = signal::display_name(st, n, id);
    // Signal.label.shown: how the last turn ended says more than the signal when nothing else is going on
    let shown = signal::shown_k(signal::name(n), t.turn, t.pending);
    let working = match shown {
        "stopped" => "Stopped: nothing is running".to_string(),
        "failed" => format!("Failed{}{}", if why.is_empty() { String::new() } else { format!(": {why}") }, if signal::active(n) { "; work also active" } else { "" }),
        _ => label(phase)?.to_string(),
    };
    let until = st.snooze_until.get(id).copied().unwrap_or(0);
    let mut m = Map::new();
    m.insert("state".into(), json!(state_word(t.turn)));
    m.insert("tools".into(), Value::Array(tools(t.turn, t.pinned, settled(st, id), t.archived, forked)));
    m.insert("menu".into(), Value::Array(menu(id, now_s < until)));
    m.insert("working".into(), json!(working)); m.insert("phase".into(), json!(phase));
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fx() -> Value { serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/gpui/slim/fixtures/thread-variants.json"))).unwrap() }
    const T: &str = "t1700000003-8";
    /// every variant the helper was dumped for: the keys that differ from the base are the ones computed here
    #[test] fn each_variants_state_keys_are_the_helpers() {
        let v = fx(); let log = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test/gpui/slim/fixtures/thread-log.jsonl"));
        let base = { let mut st = State::new(); for l in log.lines() { st.apply_line(l); } keys(&st, T, 1_700_100_000, "").expect("base") };
        let mut checked = 0;
        for (name, x) in v.as_object().unwrap() {
            if x["thread"].is_null() && x.get("diff").is_none() { continue }
            let mut st = State::new(); for l in log.lines() { st.apply_line(l); }
            for e in x["extra"].as_array().unwrap() { st.apply(e); }
            let Some(got) = keys(&st, T, 1_700_100_000, "") else { assert!(matches!(name.as_str(), "queued"), "{name} should be computed"); continue };
            let diff = x["diff"].as_object().unwrap();
            for k in ["state", "tools", "menu", "working", "phase"] { assert_eq!(Some(&got[k]), diff.get(k).or(base.get(k)), "{name}: {k}"); }
            checked += 1;
        }
        assert!(checked >= 6, "{checked}");
    }
}

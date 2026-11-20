//! How far a thread's memory tree is written, as every client draws it: the bot's Settings tab, the thread's context chip on the
//! desktop, the phones' and the web's composer (data/1 `thread.memory`). The engine (mod.rs) measures a `Progress` and keeps its
//! words here, by hub home and thread, so the synchronous readers (data/mod.rs `thread_json`, the presenter's params) take them
//! without reading the log.
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::Mutex};
use super::tree::HIGH;

/// A thread's tree as last measured.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    /// its messages
    pub messages: u64,
    /// the tree's lines that have their line (written, or a short message that is its own), of `total`
    pub done: u64, pub total: u64,
    /// the lines the compactor may write now
    pub left: u64,
    /// the messages whose own line is not written yet (the view shows them cut)
    pub behind: u64,
    /// the view's bytes, as the next turn would read it, before the cut at HIGH
    pub view: u64,
    /// held after a failed call: when it is asked again (unix s)
    pub retry: Option<u64>,
}

/// Every line of a tree of `count` messages: a node per pair, merged up to its roots (2·count less one per root).
pub fn lines(count: u64) -> u64 { (2 * count).saturating_sub(count.count_ones() as u64) }

fn kb(b: u64) -> u64 { (b + 500) / 1000 }
fn plural(n: u64, one: &str, many: &str) -> String { format!("{n} {}", if n == 1 { one } else { many }) }

/// The words and numbers the clients draw (the UI is dumb: each says what the hub gives). `clock` writes a time of day in the
/// person's zone. `pct` is 100 only when every line is there.
pub fn json(p: &Progress, clock: &dyn Fn(u64) -> String) -> Value {
    let pct = if p.done >= p.total { 100 } else { (p.done * 100 / p.total.max(1)).min(99) };
    let (state, status) = match p.retry {
        Some(t) => ("held", format!("held: retry at {}", clock(t))),
        None if p.left == 0 && p.done >= p.total => ("caught", "caught up".to_string()),
        None if p.behind > 0 => ("behind", plural(p.behind, "message behind", "messages behind")),
        None => ("behind", plural(p.left.max(1), "line to merge", "lines to merge")),
    };
    let over = p.view > HIGH as u64;
    json!({"pct": pct, "done": p.done, "total": p.total, "left": p.left, "behind": p.behind, "messages": p.messages,
        "view": p.view, "viewMax": HIGH, "viewPct": (p.view * 100 / HIGH as u64).min(100), "over": over,
        "state": state, "status": status, "retryAt": p.retry.map_or(0, |t| t * 1000),
        "line": format!("memory: {pct}% written · view {}/{} KB", kb(p.view), kb(HIGH as u64)),
        "short": format!("memory {pct}%"),
        "lines": format!("{} of {} lines written", p.done, p.total),
        "viewLine": if over { format!("view {} KB of {} KB: the oldest lines are left out until the compactor merges them", kb(p.view), kb(HIGH as u64)) } else { format!("view {} KB of {} KB", kb(p.view), kb(HIGH as u64)) },
        "tone": if p.retry.is_some() || over { "warn" } else if state == "caught" { "ok" } else { "" }})
}

/// The words as last measured, by hub home and thread (only threads with infinite context are here).
static SHOWN: Mutex<Option<HashMap<PathBuf, HashMap<String, Value>>>> = Mutex::new(None);
/// Keeps a thread's words; true when they changed.
pub fn keep(home: &Path, thread: &str, v: Value) -> bool {
    let mut g = SHOWN.lock().unwrap();
    let m = g.get_or_insert_with(HashMap::new).entry(home.to_path_buf()).or_default();
    if m.get(thread) == Some(&v) { return false }
    m.insert(thread.to_string(), v); true
}
/// Drops the threads that no longer have infinite context; answers them.
pub fn retain(home: &Path, on: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut g = SHOWN.lock().unwrap();
    let Some(m) = g.as_mut().and_then(|g| g.get_mut(home)) else { return vec![] };
    let gone: Vec<String> = m.keys().filter(|t| !on(t)).cloned().collect();
    for t in &gone { m.remove(t); }
    gone
}
/// A thread's words, None for a thread without infinite context (or not measured yet).
pub fn shown(home: &Path, thread: &str) -> Option<Value> { SHOWN.lock().unwrap().as_ref()?.get(home)?.get(thread).cloned() }

#[cfg(test)]
mod tests {
    use super::*;
    fn at(_: u64) -> String { "14:05".into() }
    #[test] fn a_tree_has_two_lines_a_message_less_one_a_root() {
        assert_eq!(lines(0), 0); assert_eq!(lines(1), 1); assert_eq!(lines(2), 3); assert_eq!(lines(4), 7); assert_eq!(lines(13), 23);
    }
    #[test] fn the_words_say_how_far_it_is_written_and_what_holds_it() {
        let p = Progress { messages: 100, done: 92, total: 100, left: 5, behind: 3, view: 64_000, retry: None };
        let v = json(&p, &at);
        assert_eq!((v["pct"].as_u64(), v["state"].as_str(), v["status"].as_str()), (Some(92), Some("behind"), Some("3 messages behind")));
        assert_eq!(v["line"], "memory: 92% written · view 64/128 KB"); assert_eq!(v["viewPct"], 50); assert_eq!(v["lines"], "92 of 100 lines written");
        let v = json(&Progress { behind: 1, ..p }, &at); assert_eq!(v["status"], "1 message behind");
        let v = json(&Progress { behind: 0, left: 2, ..p }, &at); assert_eq!(v["status"], "2 lines to merge");
        let v = json(&Progress { retry: Some(1000), ..p }, &at); assert_eq!((v["state"].as_str(), v["status"].as_str(), v["tone"].as_str(), v["retryAt"].as_u64()), (Some("held"), Some("held: retry at 14:05"), Some("warn"), Some(1_000_000)));
        let v = json(&Progress { done: 100, left: 0, behind: 0, ..p }, &at); assert_eq!((v["pct"].as_u64(), v["status"].as_str(), v["tone"].as_str()), (Some(100), Some("caught up"), Some("ok")));
        // 99.9% is not 100%: only a whole tree says it
        let v = json(&Progress { done: 999, total: 1000, ..p }, &at); assert_eq!(v["pct"], 99);
        let v = json(&Progress { view: 140_000, ..p }, &at); assert_eq!((v["over"].as_bool(), v["viewPct"].as_u64(), v["tone"].as_str()), (Some(true), Some(100), Some("warn")));
        let v = json(&Progress::default(), &at); assert_eq!((v["pct"].as_u64(), v["state"].as_str()), (Some(100), Some("caught")));
    }
    #[test] fn the_words_are_kept_by_home_and_thread_and_dropped_when_it_is_off() {
        let (a, b) = (Path::new("/home/user/a-progress"), Path::new("/home/user/b-progress"));
        assert!(keep(a, "t1", json!({"pct": 1}))); assert!(!keep(a, "t1", json!({"pct": 1}))); assert!(keep(a, "t1", json!({"pct": 2})));
        assert!(shown(b, "t1").is_none()); assert_eq!(shown(a, "t1"), Some(json!({"pct": 2})));
        assert_eq!(retain(a, &|t| t != "t1"), vec!["t1".to_string()]); assert!(shown(a, "t1").is_none());
    }
}

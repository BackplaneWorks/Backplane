//! Search by what was said: `thread.search {q, thread?, limit?}` finds the messages (the person's and the agents', `MessagePosted`) whose text
//! has every word of `q`, newest first, across every live thread or in one (`thread`). 0.14's Find ("Type a word said in any thread") and the
//! thread menu's "Search in thread". The messages come from the index of what was said (index.rs), never a read of the whole log; titles are the
//! sidebar's rows plus `titles`: every live thread whose title has every word (the sidebar holds only its first page). Answer:
//! `{hits: [{thread, title, project, msg, role, text, at}], more, titles: [{thread, title, project}]}`; `text` is a line around the first word.
//! `thread.find {thread, q}`: find in one thread (Ctrl+F): every message with every word, oldest first, for next and previous (`find_for`).
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Hits per thread across all threads (one thread's own search has no such cap).
const PER_THREAD: usize = 3;

/// The words of a query, lower case; none for a query shorter than two characters.
pub fn words(q: &str) -> Vec<String> {
    let q = q.trim().to_lowercase();
    if q.chars().count() < 2 { return vec![] }
    q.split_whitespace().map(str::to_string).collect()
}

/// A line around the first word: up to 16 characters before it and 110 after (the word stays in view when the line is cut short), on one line, with … where it was cut.
pub fn snippet(text: &str, first: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let low = flat.to_lowercase();
    let chars: Vec<char> = flat.chars().collect();
    // the word's place in characters (lower-casing can change byte lengths, so count in the lowered text up to the hit)
    let at = low.find(first).map(|b| low[..b].chars().count()).unwrap_or(0).min(chars.len());
    let (a, b) = (at.saturating_sub(16), (at + 110).min(chars.len()));
    format!("{}{}{}", if a > 0 { "…" } else { "" }, chars[a..b].iter().collect::<String>(), if b < chars.len() { "…" } else { "" })
}

/// Title hits per answer.
const TITLES: usize = 12;
/// The threads (id, title, last activity) whose title has every word, newest first, `n` at most.
pub fn titled<'a>(threads: impl Iterator<Item = (&'a str, &'a str, u64)>, ws: &[String], n: usize) -> Vec<String> {
    if ws.is_empty() { return vec![] }
    let mut v: Vec<(u64, &str)> = threads.filter(|(_, t, _)| { let low = t.to_lowercase(); ws.iter().all(|w| low.contains(w.as_str())) }).map(|(i, _, a)| (a, i)).collect();
    v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    v.into_iter().take(n).map(|(_, i)| i.to_string()).collect()
}

/// The live threads a search may show: (id → (title, project)), their last activity, and the ones the sidebar never lists (a bot's private run).
async fn live(hub: &Arc<Hub>) -> (HashMap<String, (String, String)>, HashMap<String, String>, HashMap<String, u64>, HashSet<String>) {
    let (meta, projects, activity): (HashMap<String, (String, String)>, HashMap<String, String>, HashMap<String, u64>) = {
        let m = hub.model().await;
        (m.threads.iter().filter(|(_, t)| !t.deleted).map(|(i, t)| (i.clone(), (t.title.clone(), t.project.clone()))).collect(),
         m.projects.iter().map(|(i, p)| (i.clone(), p.title.clone())).collect(),
         m.threads.iter().filter(|(_, t)| !t.deleted).map(|(i, t)| (i.clone(), t.activity.max(t.created))).collect())
    };
    let hidden: HashSet<String> = crate::statehost::policies().and_then(|x| x.with_state(|st| meta.keys().filter(|i| st.run_hidden(i)).cloned().collect::<HashSet<String>>())).unwrap_or_default();
    (meta, projects, activity, hidden)
}
fn place(projects: &HashMap<String, String>, pid: &str) -> String { if pid == crate::sidebar::SCRATCH { "Conversation history".to_string() } else { projects.get(pid).cloned().unwrap_or_default() } }

/// The search across every live thread (or one, `thread`), from the index (index.rs): newest first, `limit` hits at most, at most 3 a thread
/// across threads; `more` when it stopped at the limit.
pub async fn search_for(hub: &Arc<Hub>, q: &str, thread: &str, limit: usize) -> Value {
    let ws = words(q);
    if ws.is_empty() { return json!({"hits": [], "more": false, "titles": []}) }
    let (meta, projects, activity, hidden) = live(hub).await;
    let store = hub.store.clone();
    let (one, ws2) = (thread.to_string(), ws.clone());
    let keep_meta: HashSet<String> = meta.keys().filter(|i| !hidden.contains(*i)).cloned().collect();
    let (hits, more) = tokio::task::spawn_blocking(move || {
        let i = super::index::synced(&store); let i = i.lock().unwrap_or_else(|e| e.into_inner());
        let keep = |t: &str| keep_meta.contains(t);
        i.query(&store, &ws2, (!one.is_empty()).then_some(one.as_str()), &keep, if one.is_empty() { Some(PER_THREAD) } else { None }, limit, true)
    }).await.unwrap_or_default();
    let hits: Vec<Value> = hits.into_iter().map(|h| {
        let (title, pid) = meta.get(&h.thread).cloned().unwrap_or_default();
        json!({"thread": h.thread, "title": title, "project": place(&projects, &pid), "msg": h.msg, "role": h.role, "text": snippet(&h.text, &ws[0]), "at": h.at})
    }).collect();
    // every live thread whose title has every word, newest first: the sidebar only holds its first page, settled and older threads too are found by name
    let titles: Vec<Value> = if !thread.is_empty() { vec![] } else {
        titled(meta.iter().filter(|(i, _)| !hidden.contains(*i)).map(|(i, (t, _))| (i.as_str(), t.as_str(), activity.get(i).copied().unwrap_or(0))), &ws, TITLES)
            .into_iter().map(|i| { let (title, pid) = meta.get(&i).cloned().unwrap_or_default(); json!({"thread": i, "title": title, "project": place(&projects, &pid)}) }).collect()
    };
    json!({"hits": hits, "more": more, "titles": titles})
}

/// The most matches one thread's find answers.
pub const FIND_MAX: usize = 1000;
/// Find in a thread (Ctrl+F, the phones' find): every message of `thread` with every word of `q`, oldest first (the thread's order, for next and
/// previous; past FIND_MAX the newest FIND_MAX), each `{msg, role, at, text}` with `text` a line around the first word, and `total` (more than FIND_MAX: `more`).
pub async fn find_for(hub: &Arc<Hub>, thread: &str, q: &str) -> Value {
    let ws = words(q);
    if ws.is_empty() || thread.is_empty() { return json!({"q": q, "matches": [], "total": 0, "more": false}) }
    let store = hub.store.clone();
    let (one, ws2) = (thread.to_string(), ws.clone());
    let (found, more) = tokio::task::spawn_blocking(move || {
        let i = super::index::synced(&store); let i = i.lock().unwrap_or_else(|e| e.into_inner());
        // the newest FIND_MAX when there are more (a find starts at the newest), put back in the thread's order
        let (mut v, more) = i.query(&store, &ws2, Some(one.as_str()), &|_| true, None, FIND_MAX, true); v.reverse(); (v, more)
    }).await.unwrap_or_default();
    let matches: Vec<Value> = found.iter().map(|h| json!({"msg": h.msg, "role": h.role, "at": h.at, "text": snippet(&h.text, &ws[0])})).collect();
    json!({"q": q, "matches": matches, "total": found.len(), "more": more})
}

pub struct Search;
impl RpcHandler for Search {
    fn methods(&self) -> &'static [&'static str] { &["thread.search", "thread.find"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let q = p["q"].as_str().unwrap_or("");
            if q.len() > 256 { return vec![json!({"t":"reply","id":id,"ok":false,"error":"The search is too long."})] }
            let thread = p["thread"].as_str().unwrap_or("");
            let r = if method == "thread.find" { find_for(hub, thread, q).await } else { search_for(hub, q, thread, p["limit"].as_u64().unwrap_or(40).clamp(1, 200) as usize).await };
            vec![json!({"t":"reply","id":id,"ok":true,"result":r})]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_title_with_every_word_is_found_newest_first_wherever_the_thread_is() {
        let ts = [("t1", "cabinet design rev2", 50), ("t2", "Rev D cabinet: detailed part models", 90), ("t3", "Power budget", 99), ("t4", "Cabinet DESIGN notes", 10)];
        assert_eq!(titled(ts.iter().copied(), &words("cabinet design"), 12), vec!["t1", "t4"]);
        assert_eq!(titled(ts.iter().copied(), &words("cabinet"), 12), vec!["t2", "t1", "t4"]);
        assert_eq!(titled(ts.iter().copied(), &words("cabinet design rev2"), 12), vec!["t1"]);
        assert_eq!(titled(ts.iter().copied(), &words("cabinet"), 1), vec!["t2"]);
        assert!(titled(ts.iter().copied(), &words("x"), 12).is_empty(), "a one-letter search finds nothing, as the messages' search");
    }
    #[test] fn the_snippet_is_a_line_around_the_first_word() {
        assert_eq!(snippet("short\ntext with Word", "word"), "short text with Word");
        let long = format!("{} needle {}", "a".repeat(100), "b".repeat(200));
        let s = snippet(&long, "needle"); assert!(s.starts_with('…') && s.ends_with('…') && s.contains("needle")); assert!(s.chars().count() <= 128);
        assert_eq!(snippet("ÄÖÜ needle", "needle"), "ÄÖÜ needle", "characters, not bytes");
    }
}

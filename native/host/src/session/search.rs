//! Search by what was said: `thread.search {q, thread?, limit?}` finds the messages (the person's and the agents', `MessagePosted`) whose text
//! has every word of `q`, newest first, across every live thread or in one (`thread`). 0.14's Find ("Type a word said in any thread") and the
//! thread menu's "Search in thread". The log is read from its end in blocks, so a common word answers from the newest lines; titles are the
//! sidebar's own search. Answer: `{hits: [{thread, title, project, msg, role, text, at}], more}`; `text` is a line around the first word.
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Lines read per block, from the end of the log.
const BLOCK: usize = 4096;
/// Hits per thread across all threads (one thread's own search has no such cap).
const PER_THREAD: usize = 3;

#[derive(Debug, PartialEq)]
pub struct Hit { pub thread: String, pub msg: String, pub role: String, pub text: String, pub at: u64 }

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

/// The hits in a block of log lines, newest (last) first. `keep` says whether a thread is searched; `seen` holds the messages already
/// answered (a message posted again counts once: its newest matching text). `per` counts each thread's hits.
pub fn scan(lines: &[String], ws: &[String], keep: &dyn Fn(&str) -> bool, cap: Option<usize>, seen: &mut HashSet<(String, String)>, per: &mut HashMap<String, usize>, out: &mut Vec<Hit>, limit: usize) {
    if ws.is_empty() { return }
    for line in lines.iter().rev() {
        if out.len() >= limit { return }
        if !line.contains("\"MessagePosted\"") { continue }
        let low = line.to_lowercase();
        if !ws.iter().all(|w| low.contains(w.as_str())) { continue }
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if v["$"] != "MessagePosted" { continue }
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        let (thread, msg, role, text) = (s("thread"), s("msg"), s("role"), s("text"));
        if !matches!(role.as_str(), "user" | "assistant") || !keep(&thread) { continue }
        if !seen.insert((thread.clone(), msg.clone())) { continue }
        let tl = text.to_lowercase();
        if !ws.iter().all(|w| tl.contains(w.as_str())) { continue }
        let n = per.entry(thread.clone()).or_default();
        if cap.map_or(false, |c| *n >= c) { continue }
        *n += 1;
        out.push(Hit { text: snippet(&text, &ws[0]), thread, msg, role, at: v["at"].as_u64().unwrap_or(0) });
    }
}

/// The search over the hub's log: newest first, `limit` hits at most; `more` when it stopped at the limit.
pub async fn search(hub: &Arc<Hub>, q: &str, thread: &str, limit: usize) -> Value {
    let ws = words(q);
    if ws.is_empty() { return json!({"hits": [], "more": false}) }
    // the live threads and their names (the model's metadata; no entry is read for this)
    let (meta, projects): (HashMap<String, (String, String)>, HashMap<String, String>) = {
        let m = hub.model().await;
        (m.threads.iter().filter(|(_, t)| !t.deleted).map(|(i, t)| (i.clone(), (t.title.clone(), t.project.clone()))).collect(),
         m.projects.iter().map(|(i, p)| (i.clone(), p.title.clone())).collect())
    };
    let store = hub.store.clone();
    let (one, ws2) = (thread.to_string(), ws.clone());
    // a thread the sidebar never lists (a bot's private run) is not searched either
    let hidden: HashSet<String> = crate::statehost::policies().and_then(|x| x.with_state(|st| meta.keys().filter(|i| st.run_hidden(i)).cloned().collect::<HashSet<String>>())).unwrap_or_default();
    let keep_meta: HashSet<String> = meta.keys().filter(|i| !hidden.contains(*i)).cloned().collect();
    let (hits, more) = tokio::task::spawn_blocking(move || {
        let keep = |t: &str| keep_meta.contains(t) && (one.is_empty() || t == one);
        let cap = if one.is_empty() { Some(PER_THREAD) } else { None };
        let (mut seen, mut per, mut out) = (HashSet::new(), HashMap::new(), vec![]);
        let mut end = store.len();
        while end > 0 && out.len() < limit {
            let a = end.saturating_sub(BLOCK);
            let Ok(lines) = store.range(a, end) else { break };
            scan(&lines, &ws2, &keep, cap, &mut seen, &mut per, &mut out, limit);
            end = a;
        }
        let more = out.len() >= limit && end > 0;
        (out, more)
    }).await.unwrap_or_default();
    let hits: Vec<Value> = hits.into_iter().map(|h| {
        let (title, pid) = meta.get(&h.thread).cloned().unwrap_or_default();
        let project = if pid == crate::sidebar::SCRATCH { "Conversation history".to_string() } else { projects.get(&pid).cloned().unwrap_or_default() };
        json!({"thread": h.thread, "title": title, "project": project, "msg": h.msg, "role": h.role, "text": h.text, "at": h.at})
    }).collect();
    json!({"hits": hits, "more": more})
}

pub struct Search;
impl RpcHandler for Search {
    fn methods(&self) -> &'static [&'static str] { &["thread.search"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let q = p["q"].as_str().unwrap_or("");
            if q.len() > 256 { return vec![json!({"t":"reply","id":id,"ok":false,"error":"The search is too long."})] }
            let limit = p["limit"].as_u64().unwrap_or(40).clamp(1, 200) as usize;
            let r = search(hub, q, p["thread"].as_str().unwrap_or(""), limit).await;
            vec![json!({"t":"reply","id":id,"ok":true,"result":r})]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn line(th: &str, msg: &str, role: &str, text: &str, at: u64) -> String { json!({"$":"MessagePosted","thread":th,"msg":msg,"role":role,"text":text,"at":at}).to_string() }
    fn run(lines: &[String], q: &str, keep: &dyn Fn(&str) -> bool, cap: Option<usize>, limit: usize) -> Vec<Hit> {
        let (mut seen, mut per, mut out) = (HashSet::new(), HashMap::new(), vec![]);
        scan(lines, &words(q), keep, cap, &mut seen, &mut per, &mut out, limit); out
    }
    #[test] fn every_word_is_found_in_what_was_said_newest_first() {
        let l = vec![line("t1", "m1", "user", "Where does the Pinecone footprint go?", 10), line("t2", "m2", "assistant", "The pinecone sits by U3.", 11),
            r#"{"$":"ThreadRenamed","id":"t1","title":"pinecone","at":12}"#.to_string(), line("t1", "m3", "system", "pinecone restarted", 13), line("t3", "m4", "user", "nothing here", 14)];
        let all = |_: &str| true;
        let h = run(&l, "PINECONE", &all, None, 10);
        assert_eq!(h.iter().map(|x| (x.thread.as_str(), x.msg.as_str())).collect::<Vec<_>>(), [("t2", "m2"), ("t1", "m1")], "messages only, the person's and the agents', newest first");
        assert_eq!(run(&l, "pinecone footprint", &all, None, 10).len(), 1, "every word");
        assert!(run(&l, "p", &all, None, 10).is_empty(), "one letter finds nothing");
        assert_eq!(run(&l, "pinecone", &|t| t == "t1", None, 10).len(), 1, "one thread's own search");
        assert_eq!(run(&l, "pinecone", &all, None, 1).len(), 1, "the limit");
    }
    #[test] fn a_message_counts_once_and_a_thread_gives_at_most_its_cap() {
        let l = vec![line("t1", "m1", "assistant", "draft about the pinecone", 1), line("t1", "m1", "assistant", "final: the pinecone moves", 2),
            line("t1", "m2", "user", "pinecone again", 3), line("t1", "m3", "user", "pinecone thrice", 4), line("t1", "m4", "user", "pinecone four", 5)];
        let h = run(&l, "pinecone", &|_| true, None, 10);
        assert_eq!(h.len(), 4); assert!(h.iter().any(|x| x.text.starts_with("final")), "the newest text of a message posted twice");
        assert_eq!(run(&l, "pinecone", &|_| true, Some(PER_THREAD), 10).len(), PER_THREAD);
    }
    #[test] fn the_snippet_is_a_line_around_the_first_word() {
        assert_eq!(snippet("short\ntext with Word", "word"), "short text with Word");
        let long = format!("{} needle {}", "a".repeat(100), "b".repeat(200));
        let s = snippet(&long, "needle"); assert!(s.starts_with('…') && s.ends_with('…') && s.contains("needle")); assert!(s.chars().count() <= 128);
        assert_eq!(snippet("ÄÖÜ needle", "needle"), "ÄÖÜ needle", "characters, not bytes");
    }
}

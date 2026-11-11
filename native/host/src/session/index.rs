//! The index of what was said: every message (the person's and the agents', `MessagePosted`) by the words in it, so a search across thousands of
//! threads reads only the messages that hold the words, never the whole log. Built once from the log on a thread of its own as the hub starts, then
//! kept up by reading only the lines appended since (`sync`, before every query and every checkpoint round); nothing is saved (a home of 150 MB
//! of log builds in well under a second, docs/HUB-API.md).
//!
//! A word is found where 0.14's Find found it: anywhere inside what was said, any case. The index maps each token (a run of letters and digits,
//! lower case) to the messages holding it; a query word is looked up in every token containing its own tokens, and the candidates are then read
//! back from the log and checked against the whole text, so the answer is exactly "the text has every word". A message posted again (its text
//! replaced) is found by its newest text only.
use crate::store::Store;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Tokens longer than this are cut (hashes and base64 would only bloat the vocabulary; a word past the cut is still found when the rest of the query narrows it).
const TOKEN_MAX: usize = 48;

#[derive(Clone, Copy)]
struct Doc { thread: u32, line: u32, at: u32, user: bool, live: bool }

#[derive(Default)]
pub struct Index {
    /// log lines read
    through: usize,
    threads: Vec<Box<str>>, thread_ix: HashMap<Box<str>, u32>,
    docs: Vec<Doc>,
    /// (thread, message) → its newest doc
    newest: HashMap<(u32, Box<str>), u32>,
    /// token → docs holding it, ascending
    terms: HashMap<Box<str>, Vec<u32>>,
    /// thread → its docs, ascending
    by_thread: Vec<Vec<u32>>,
}

/// A message found: its thread, the message, who said it, when (unix s), and its whole text.
#[derive(Debug, Clone, PartialEq)]
pub struct Found { pub thread: String, pub msg: String, pub role: String, pub text: String, pub at: u64 }

/// The tokens of a text: runs of letters and digits, lower case, each cut at TOKEN_MAX characters.
pub fn tokens(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).map(|t| t.chars().flat_map(char::to_lowercase).take(TOKEN_MAX).collect())
}

/// What a query asks of a message: every word in its text, any case.
pub fn has_all(text_lower: &str, words: &[String]) -> bool { words.iter().all(|w| text_lower.contains(w.as_str())) }

impl Index {
    pub fn lines(&self) -> usize { self.through }
    pub fn messages(&self) -> usize { self.newest.len() }

    /// Read the lines appended since the last call.
    pub fn sync(&mut self, store: &Store) {
        let total = store.len();
        while self.through < total {
            let b = (self.through + 4096).min(total);
            let Ok(lines) = store.range(self.through, b) else { return };
            for (i, l) in lines.iter().enumerate() { self.apply(self.through + i, l) }
            self.through = b;
        }
    }

    /// One log line (0-based `at`).
    pub fn apply(&mut self, at: usize, line: &str) {
        if crate::session::event_type(line) != Some("MessagePosted") { return }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        let s = |k: &str| v[k].as_str().unwrap_or("");
        let role = s("role");
        if !matches!(role, "user" | "assistant") { return }
        let (thread, msg) = (s("thread"), s("msg"));
        let t = match self.thread_ix.get(thread) { Some(t) => *t, None => { let t = self.threads.len() as u32; self.threads.push(thread.into()); self.thread_ix.insert(thread.into(), t); self.by_thread.push(vec![]); t } };
        let d = self.docs.len() as u32;
        if let Some(old) = self.newest.insert((t, msg.into()), d) { self.docs[old as usize].live = false }
        self.docs.push(Doc { thread: t, line: at as u32, at: v["at"].as_u64().unwrap_or(0).min(u32::MAX as u64) as u32, user: role == "user", live: true });
        self.by_thread[t as usize].push(d);
        let mut ts: Vec<String> = tokens(s("text")).collect(); ts.sort_unstable(); ts.dedup();
        for tok in ts {
            match self.terms.get_mut(tok.as_str()) { Some(p) => p.push(d), None => { self.terms.insert(tok.into(), vec![d]); } }
        }
    }

    /// The docs that may hold every word, as a bit set (None: the words have no letters or digits, so any message may).
    fn candidates(&self, words: &[String]) -> Option<Vec<u64>> {
        let n = self.docs.len();
        let mut set: Option<Vec<u64>> = None;
        let mut parts: Vec<String> = words.iter().flat_map(|w| tokens(w).collect::<Vec<_>>()).collect();
        // the longest first: it narrows most, and a set gone empty stops the rest
        parts.sort_by(|a, b| b.len().cmp(&a.len())); parts.dedup();
        for p in parts {
            let mut s = vec![0u64; n.div_ceil(64)];
            for (term, docs) in &self.terms { if term.contains(p.as_str()) { for d in docs { s[*d as usize >> 6] |= 1 << (d & 63) } } }
            set = Some(match set { None => s, Some(mut a) => { for (x, y) in a.iter_mut().zip(s) { *x &= y } a } });
            if set.as_ref().map_or(false, |a| a.iter().all(|x| *x == 0)) { break }
        }
        set
    }

    /// The messages with every word, read back from the log: in one thread (`thread`) or every thread `keep` allows, newest first (`newest`)
    /// or oldest first, at most `per` of a thread and `limit` in all. Answers the hits and whether more were left.
    pub fn query(&self, store: &Store, words: &[String], thread: Option<&str>, keep: &dyn Fn(&str) -> bool, per: Option<usize>, limit: usize, newest: bool) -> (Vec<Found>, bool) {
        let mut out = vec![];
        if words.is_empty() { return (out, false) }
        let set = self.candidates(words);
        let is = |d: u32| set.as_ref().map_or(true, |s| s[d as usize >> 6] & (1 << (d & 63)) != 0);
        let docs: Box<dyn Iterator<Item = u32>> = match thread {
            Some(t) => { let Some(ix) = self.thread_ix.get(t) else { return (out, false) }; let v = &self.by_thread[*ix as usize];
                if newest { Box::new(v.iter().rev().copied()) } else { Box::new(v.iter().copied()) } }
            None => if newest { Box::new((0..self.docs.len() as u32).rev()) } else { Box::new(0..self.docs.len() as u32) },
        };
        let mut count: HashMap<u32, usize> = HashMap::new();
        for d in docs {
            let doc = self.docs[d as usize];
            if !doc.live || !is(d) { continue }
            if per.map_or(false, |c| count.get(&doc.thread).copied().unwrap_or(0) >= c) { continue }
            let name = &self.threads[doc.thread as usize];
            if !keep(name) { continue }
            let Ok(line) = store.get(doc.line as usize) else { continue };
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            let text = v["text"].as_str().unwrap_or("");
            if !has_all(&text.to_lowercase(), words) { continue }
            if out.len() >= limit { return (out, true) }
            *count.entry(doc.thread).or_default() += 1;
            out.push(Found { thread: name.to_string(), msg: v["msg"].as_str().unwrap_or("").to_string(), role: if doc.user { "user" } else { "assistant" }.into(), text: text.to_string(), at: doc.at as u64 });
        }
        (out, false)
    }
}

/// The index of a log: one per store (a test runs many hubs in one process), built on first use.
static ALL: Mutex<Vec<(std::path::PathBuf, Arc<Mutex<Index>>)>> = Mutex::new(Vec::new());
pub fn of(store: &Store) -> Arc<Mutex<Index>> {
    let mut all = ALL.lock().unwrap();
    if let Some((_, i)) = all.iter().find(|(p, _)| p == store.path()) { return i.clone() }
    if all.len() > 32 { all.remove(0); }
    let i = Arc::new(Mutex::new(Index::default())); all.push((store.path().to_path_buf(), i.clone())); i
}
/// Bring the index up to the log now (blocking: call it off the async threads). The first call reads the whole log once.
pub fn synced(store: &Store) -> Arc<Mutex<Index>> {
    let i = of(store);
    i.lock().unwrap_or_else(|e| e.into_inner()).sync(store);
    i
}
/// At start: build the index on a thread of its own, so the first search does not wait for it, and keep it up with each checkpoint round.
pub fn start(store: &Store) {
    let s = store.clone();
    std::thread::spawn(move || { let t = std::time::Instant::now(); let i = synced(&s); let g = i.lock().unwrap_or_else(|e| e.into_inner());
        eprintln!("search index: {} messages from {} lines in {} ms", g.messages(), g.lines(), t.elapsed().as_millis()); });
    crate::ckpt::register(|store| { synced(store); });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn rig(t: &str, lines: &[String]) -> (crate::plat::tmp::TempDir, Store) {
        let d = crate::plat::tmp::dir(&format!("index-{t}"));
        let p = d.join("events.jsonl"); std::fs::write(&p, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap(); let s = Store::open(&p).unwrap(); (d, s)
    }
    pub fn line(th: &str, msg: &str, role: &str, text: &str, at: u64) -> String { json!({"$":"MessagePosted","thread":th,"msg":msg,"role":role,"text":text,"at":at}).to_string() }
    fn w(q: &str) -> Vec<String> { q.split_whitespace().map(|x| x.to_lowercase()).collect() }
    fn ids(h: &[Found]) -> Vec<(&str, &str)> { h.iter().map(|x| (x.thread.as_str(), x.msg.as_str())).collect() }
    #[test] fn a_word_is_found_anywhere_inside_what_was_said_any_case() {
        let (d, s) = rig("inside", &[line("t1", "m1", "user", "Where does the Pinecone footprint go?", 10), line("t2", "m2", "assistant", "The pinecone sits by U3.", 11),
            r#"{"$":"ThreadRenamed","id":"t1","title":"pinecone","at":12}"#.into(), line("t1", "m3", "system", "pinecone restarted", 13), line("t3", "m4", "user", "日本語のクエリ", 14)]);
        let mut i = Index::default(); i.sync(&s);
        let all = |_: &str| true;
        assert_eq!(ids(&i.query(&s, &w("PINECONE"), None, &all, None, 10, true).0), [("t2", "m2"), ("t1", "m1")], "messages only, the person's and the agents', newest first");
        assert_eq!(ids(&i.query(&s, &w("necon"), None, &all, None, 10, true).0).len(), 2, "inside a word");
        assert_eq!(ids(&i.query(&s, &w("pinecone footprint"), None, &all, None, 10, true).0), [("t1", "m1")], "every word");
        assert_eq!(ids(&i.query(&s, &w("u3."), None, &all, None, 10, true).0), [("t2", "m2")], "a word with its punctuation is checked against the text");
        assert!(i.query(&s, &w("u3,"), None, &all, None, 10, true).0.is_empty());
        assert_eq!(ids(&i.query(&s, &w("クエリ"), None, &all, None, 10, true).0), [("t3", "m4")], "text without spaces");
        assert_eq!(ids(&i.query(&s, &w("pinecone"), Some("t1"), &all, None, 10, true).0), [("t1", "m1")], "one thread's own");
        assert_eq!(ids(&i.query(&s, &w("pinecone"), None, &|t| t != "t2", None, 10, true).0), [("t1", "m1")], "a thread kept out");
        let (h, more) = i.query(&s, &w("pinecone"), None, &all, None, 1, true); assert_eq!((h.len(), more), (1, true), "the limit, and more left");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn new_lines_are_read_as_they_come_and_a_message_posted_again_is_its_newest_text() {
        let (d, s) = rig("grow", &[line("t1", "m1", "assistant", "draft about the pinecone", 1)]);
        let mut i = Index::default(); i.sync(&s);
        assert_eq!(i.query(&s, &w("pinecone"), None, &|_| true, None, 10, true).0.len(), 1);
        s.append(&line("t1", "m1", "assistant", "final: the acorn moves", 2)).unwrap(); s.append(&line("t1", "m2", "user", "pinecone again", 3)).unwrap();
        i.sync(&s); assert_eq!(i.lines(), 3, "only the new lines");
        assert_eq!(ids(&i.query(&s, &w("pinecone"), None, &|_| true, None, 10, true).0), [("t1", "m2")], "m1 no longer says it");
        assert_eq!(ids(&i.query(&s, &w("acorn"), None, &|_| true, None, 10, true).0), [("t1", "m1")]);
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn a_thread_gives_at_most_its_cap_and_oldest_first_is_the_threads_order() {
        let l: Vec<String> = (0..6).map(|n| line(if n % 2 == 0 { "a" } else { "b" }, &format!("m{n}"), "user", &format!("pinecone {n}"), n)).collect();
        let (d, s) = rig("cap", &l);
        let mut i = Index::default(); i.sync(&s);
        let h = i.query(&s, &w("pinecone"), None, &|_| true, Some(2), 10, true).0;
        assert_eq!(ids(&h), [("b", "m5"), ("a", "m4"), ("b", "m3"), ("a", "m2")]);
        assert_eq!(ids(&i.query(&s, &w("pinecone"), Some("a"), &|_| true, None, 10, false).0), [("a", "m0"), ("a", "m2"), ("a", "m4")]);
        let _ = std::fs::remove_dir_all(&d);
    }
    /// `BACKPLANE_INDEX_BENCH=<a copy of a log>`: how long the index takes to build and to answer (cargo test -- --ignored --nocapture index_bench).
    #[test] #[ignore] fn index_bench() {
        let Ok(p) = std::env::var("BACKPLANE_INDEX_BENCH") else { return };
        let s = Store::open(std::path::Path::new(&p)).unwrap();
        let t = std::time::Instant::now(); let mut i = Index::default(); i.sync(&s);
        eprintln!("built: {} messages, {} terms, {} lines in {} ms", i.messages(), i.terms.len(), i.lines(), t.elapsed().as_millis());
        for q in ["the", "pinecone", "gpui branch", "e", "kicad footprint"] { let t = std::time::Instant::now(); let (h, _) = i.query(&s, &w(q), None, &|_| true, Some(3), 40, true); eprintln!("{q:?}: {} hits in {} us", h.len(), t.elapsed().as_micros()); }
    }
}

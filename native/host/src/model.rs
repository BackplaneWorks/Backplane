//! The Rust read model. Entries stay in events.jsonl; the model holds only indexes of them:
//! per thread (and per sender thread, for messages that came from another) the (seq, ord) of each
//! entry. `seq` is the 1-based line of the event, `ord` the entry ordinal (core/thin.bend Index.post
//! numbers every message, activity and reminder in log order), exactly the store.db the Bend hub
//! keeps; a page is thinq.bend's Q.sql.page over those indexes (law: same log, same pages).
use crate::store::Store;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::borrow::Cow;
use std::collections::HashMap;

pub const TOP: u64 = 140737488355328; // Q.top: no bound
pub const LOOKAHEAD: usize = 17;      // Ts.page: 17 more rows than the 32 shown

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ent { pub seq: u32, pub ord: u32 }

#[derive(Debug, Clone, PartialEq)]
pub struct Row { pub seq: u64, pub ord: u64, pub id: String, pub thread: String, pub src: String, pub kind: &'static str, pub role: String, pub akind: String, pub text: String, pub at: u64 }
impl Row {
    pub fn json(&self) -> Value { json!([self.seq, self.ord, self.id, self.thread, self.src, self.kind, self.role, self.akind, self.text, self.at]) }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page { pub ids: Vec<String>, pub below: u64, pub above: u64, pub rows: Vec<Row> }

#[derive(Deserialize)]
struct Ev<'a> {
    #[serde(rename = "$", borrow)] t: Cow<'a, str>,
    #[serde(borrow)] thread: Option<Cow<'a, str>>, #[serde(borrow)] msg: Option<Cow<'a, str>>, #[serde(borrow)] act: Option<Cow<'a, str>>,
    #[serde(borrow)] id: Option<Cow<'a, str>>, at: Option<u64>,
    #[serde(borrow)] title: Option<Cow<'a, str>>, #[serde(borrow)] project: Option<Cow<'a, str>>, #[serde(borrow)] root: Option<Cow<'a, str>>,
    #[serde(borrow)] env: Option<Cow<'a, str>>, #[serde(borrow)] branch: Option<Cow<'a, str>>, #[serde(borrow)] worktree: Option<Cow<'a, str>>,
    #[serde(borrow)] provider: Option<Cow<'a, str>>, #[serde(borrow)] model: Option<Cow<'a, str>>, #[serde(borrow)] session: Option<Cow<'a, str>>,
    #[serde(borrow)] state: Option<Cow<'a, str>>, count: Option<u64>, archived: Option<bool>,
    #[serde(borrow)] key: Option<Cow<'a, str>>, #[serde(borrow)] value: Option<Cow<'a, str>>, #[serde(borrow)] role: Option<Cow<'a, str>>,
}

/// core/model.bend Link.from: "t:<thread>:<rest>" names the thread a message came from.
pub fn link_from(id: &str) -> &str {
    match id.strip_prefix("t:") { Some(r) if r.contains(':') => r.split(':').next().unwrap_or(""), _ => "" }
}

/// What the hub needs to know of a thread without reading its entries (agents: where to run; views: its state).
#[derive(Default, Clone, Debug)]
pub struct ThreadMeta { pub project: String, pub title: String, pub env: String, pub branch: String, pub worktree: String, pub provider: String, pub model: String,
    pub session: String, pub last_user_msg: String, pub last_entry: String, pub turn: String, pub pending: u64, pub created: u64, pub activity: u64, pub archived: bool, pub deleted: bool }
#[derive(Default, Clone, Debug)]
pub struct ProjectMeta { pub title: String, pub root: String, pub removed: bool }

#[derive(Default)]
pub struct Model {
    pub next_ord: u32,
    all: Vec<u32>,                          // all[ord-1] = seq
    own: HashMap<String, Vec<Ent>>,         // thread -> its entries (seq ascending)
    sent: HashMap<String, Vec<Ent>>,        // thread -> messages it sent into other threads
    pub threads: HashMap<String, ThreadMeta>,
    pub projects: HashMap<String, ProjectMeta>,
    /// setting key -> the 1-based log line of its latest value (an empty value removes the key). Bounded by the
    /// number of distinct keys, not by events; the value is read from the log on demand (Hub::setting).
    settings: std::collections::BTreeMap<String, u32>,
}

fn before_seq(v: &[Ent], s: u64) -> usize { v.partition_point(|e| (e.seq as u64) < s) }

impl Model {
    pub fn build(store: &Store) -> Result<Model> {
        let mut m = Model::default();
        for a in (0..store.len()).step_by(4096) {
            for (i, l) in store.range(a, a + 4096)?.iter().enumerate() { m.apply_line((a + i + 1) as u32, l); }
        }
        Ok(m)
    }
    pub fn apply_line(&mut self, seq: u32, line: &str) { if let Ok(e) = serde_json::from_str::<Ev>(line) { self.apply(seq, &e) } }

    fn apply(&mut self, seq: u32, e: &Ev) {
        let t = |o: &Option<Cow<str>>| o.as_deref().unwrap_or("").to_string();
        let (thread, id) = match &*e.t {
            "MessagePosted" => (t(&e.thread), t(&e.msg)),
            "ActivityLogged" => (t(&e.thread), t(&e.act)),
            "Reminded" => (t(&e.thread), format!("remind:{}:{}", t(&e.id), e.at.unwrap_or(0))),
            "SettingSet" => { let k = t(&e.key); if e.value.as_deref().unwrap_or("").is_empty() { self.settings.remove(&k); } else { self.settings.insert(k, seq); } return }
            "ProjectCreated" => { self.projects.insert(t(&e.id), ProjectMeta { title: t(&e.title), root: t(&e.root), removed: false }); return }
            "ProjectRemoved" => { if let Some(p) = self.projects.get_mut(&t(&e.id)) { p.removed = true } return }
            "ThreadCreated" => { self.threads.insert(t(&e.id), ThreadMeta { project: t(&e.project), title: t(&e.title), env: t(&e.env), branch: t(&e.branch), worktree: t(&e.worktree),
                provider: t(&e.provider), model: t(&e.model), turn: "idle".into(), created: e.at.unwrap_or(0), activity: e.at.unwrap_or(0), ..Default::default() }); return }
            "ThreadRenamed" => { if let Some(th) = self.threads.get_mut(&t(&e.id)) { th.title = t(&e.title) } return }
            "TurnChanged" => { if let Some(th) = self.threads.get_mut(&t(&e.thread)) { th.turn = t(&e.state); th.activity = e.at.unwrap_or(th.activity) } return }
            "PendingChanged" => { if let Some(th) = self.threads.get_mut(&t(&e.thread)) { th.pending = e.count.unwrap_or(0) } return }
            "SessionSet" => { if let Some(th) = self.threads.get_mut(&t(&e.id)) { th.session = t(&e.session) } return }
            "WorktreeSet" => { if let Some(th) = self.threads.get_mut(&t(&e.id)) { th.branch = t(&e.branch); th.worktree = t(&e.worktree) } return }
            "ProviderSet" => { if let Some(th) = self.threads.get_mut(&t(&e.id)) { th.provider = t(&e.provider); th.model = t(&e.model) } return }
            "ThreadArchived" => { if let Some(th) = self.threads.get_mut(&t(&e.id)) { th.archived = e.archived.unwrap_or(true) } return }
            "ThreadDeleted" => { if let Some(th) = self.threads.get_mut(&t(&e.id)) { th.deleted = true } return }
            _ => return,
        };
        if let Some(th) = self.threads.get_mut(&thread) {
            th.activity = e.at.unwrap_or(th.activity); th.last_entry = id.clone();
            if &*e.t == "MessagePosted" && e.role.as_deref() == Some("user") { th.last_user_msg = id.clone(); }
        }
        self.next_ord += 1;
        let ent = Ent { seq, ord: self.next_ord };
        self.all.push(seq);
        let from = link_from(&id).to_string();
        self.own.entry(thread.clone()).or_default().push(ent);
        if !from.is_empty() && from != thread { self.sent.entry(from).or_default().push(ent); }
    }

    /// The full row of an entry (text not cut).
    pub fn row(&self, store: &Store, ent: Ent, thread: &str, cut: bool) -> Result<Row> {
        let v: Value = serde_json::from_str(&store.get(ent.seq as usize - 1)?)?;
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let at = v.get("at").and_then(Value::as_u64).unwrap_or(0);
        let (id, kind, role, akind, text) = match v["$"].as_str().unwrap_or("") {
            "MessagePosted" => (s("msg"), "msg", s("role"), String::new(), s("text")),
            "ActivityLogged" => (s("act"), "act", s("tone"), s("kind"), s("summary")),
            _ => (format!("remind:{}:{}", s("id"), at), "act", "remind".into(), "reminder".into(), s("text")),
        };
        let text = if cut && kind == "act" { text.chars().take(513).collect() } else { text };
        let _ = thread; let th = s("thread");
        let src = { let f = link_from(&id).to_string(); if !f.is_empty() && f != th { f } else { String::new() } };
        Ok(Row { seq: ent.seq as u64, ord: ent.ord as u64, id, thread: th, src, kind, role, akind, text, at })
    }

    /// ts.page / Ts.page: the thread's rows older than ordinal `before` (0: newest), `limit` capped at 32
    /// plus the 17-row lookahead, with the counts below and above.
    pub fn page(&self, store: &Store, thread: &str, before: u64, limit: usize) -> Result<Page> {
        let cap = limit.min(32);
        let lim = cap + LOOKAHEAD;
        let bound = if before == 0 { TOP } else { before };
        let seq_at = if bound > self.next_ord as u64 { TOP } else { self.all[bound as usize - 1] as u64 }; // first entry with ord >= bound
        let (own, sent) = (self.own.get(thread).map(Vec::as_slice).unwrap_or(&[]), self.sent.get(thread).map(Vec::as_slice).unwrap_or(&[]));
        let (a, b) = (before_seq(own, seq_at), before_seq(sent, seq_at));
        let mut picks: Vec<Ent> = own[a.saturating_sub(lim)..a].iter().chain(sent[b.saturating_sub(lim)..b].iter()).copied().collect();
        picks.sort_by(|x, y| y.seq.cmp(&x.seq)); picks.truncate(lim);
        let rows = picks.iter().map(|e| self.row(store, *e, thread, true)).collect::<Result<Vec<_>>>()?;
        let below = if rows.len() == lim { let l = rows.last().unwrap().seq; (before_seq(own, l) + before_seq(sent, l)) as u64 } else { 0 };
        let above = if before != 0 { ((own.len() - a) + (sent.len() - b)) as u64 } else { 0 };
        let mut ords: Vec<u64> = rows.iter().map(|r| r.ord).collect(); ords.sort_unstable_by(|x, y| y.cmp(x)); ords.dedup();
        let ids = ords.into_iter().filter(|n| before == 0 || *n < before).take(cap).map(|n| n.to_string()).collect();
        Ok(Page { ids, below, above, rows })
    }
    pub fn entries(&self) -> u32 { self.next_ord }
    /// The log line (1-based) of a setting's latest value, if it has one.
    pub fn setting_seq(&self, key: &str) -> Option<u32> { self.settings.get(key).copied() }
    /// Keys starting with `prefix`, in order, with the log line of their latest value.
    pub fn settings_with_prefix<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = (&'a str, u32)> + 'a {
        self.settings.range(prefix.to_string()..).take_while(move |(k, _)| k.starts_with(prefix)).map(|(k, s)| (k.as_str(), *s))
    }
    pub fn settings_count(&self) -> usize { self.settings.len() }
    pub fn all_ents(&self) -> impl Iterator<Item = Ent> + '_ { self.all.iter().enumerate().map(|(i, s)| Ent { seq: *s, ord: i as u32 + 1 }) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    fn fixtures() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/gpui/hub/fixtures") }
    fn store() -> Store {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let t = std::env::temp_dir().join(format!("bp-model-{}-{}.jsonl", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        std::fs::copy(fixtures().join("pages.jsonl"), &t).unwrap(); Store::open(&t).unwrap()
    }
    #[test] fn link_from_matches_bend() {
        assert_eq!(link_from("t:t3:c0"), "t3"); assert_eq!(link_from("t:t3"), ""); assert_eq!(link_from("m1"), ""); assert_eq!(link_from("tu1:2"), ""); assert_eq!(link_from("t:"), "");
    }
    // same log, same entries: every row of the JS hub's store.db (seq, ord, id, thread, src, kind, role, akind, text, at)
    #[test] fn history_entries_match_the_store_db() {
        let s = store(); let m = Model::build(&s).unwrap();
        let want: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(fixtures().join("entries.json")).unwrap()).unwrap();
        let got: Vec<Value> = m.all_ents().map(|e| m.row(&s, e, "", false).unwrap().json()).collect();
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(&want) { assert_eq!(g, w); }
    }
    // same log, same pages: ids, counts and rows for every thread at several cursors
    #[test] fn history_pages_match_the_store_db() {
        let s = store(); let m = Model::build(&s).unwrap();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(fixtures().join("pages.json")).unwrap()).unwrap();
        let mut n = 0;
        for p in doc["pages"].as_array().unwrap() {
            let page = m.page(&s, p["thread"].as_str().unwrap(), p["before"].as_u64().unwrap(), 32).unwrap();
            let ctx = format!("thread {} before {}", p["thread"], p["before"]);
            assert_eq!(json!(page.ids), p["ids"], "ids {ctx}"); assert_eq!(json!(page.below), p["below"], "below {ctx}"); assert_eq!(json!(page.above), p["above"], "above {ctx}");
            assert_eq!(Value::Array(page.rows.iter().map(Row::json).collect()), p["rows"], "rows {ctx}"); n += 1;
        }
        assert!(n >= 80, "{n} pages compared");
    }
    #[test] fn settings_hold_the_latest_value_by_key_and_removal() {
        let t = std::env::temp_dir().join(format!("bp-model-set-{}.jsonl", std::process::id()));
        std::fs::write(&t, "{\"$\":\"SettingSet\",\"key\":\"restart.continue\",\"value\":\"on\",\"at\":1}\n{\"$\":\"SettingSet\",\"key\":\"tok.a\",\"value\":\"1\",\"at\":1}\n{\"$\":\"SettingSet\",\"key\":\"restart.continue\",\"value\":\"off\",\"at\":2}\n{\"$\":\"SettingSet\",\"key\":\"tok.a\",\"value\":\"\",\"at\":3}\n").unwrap();
        let s = Store::open(&t).unwrap(); let m = Model::build(&s).unwrap();
        assert_eq!(m.setting_seq("restart.continue"), Some(3)); assert_eq!(m.setting_seq("tok.a"), None); assert_eq!(m.settings_count(), 1);
        assert!(s.get(m.setting_seq("restart.continue").unwrap() as usize - 1).unwrap().contains("\"off\""));
    }
    #[test] fn threads_know_their_last_user_message_and_newest_entry() {
        let s = store(); let m = Model::build(&s).unwrap();
        let t7 = &m.threads["t7"]; assert_eq!(t7.last_user_msg, "u19"); assert_eq!(t7.last_entry, "u19");
    }
    #[test] fn a_torn_tail_does_not_move_the_ordinals() {
        let t = std::env::temp_dir().join(format!("bp-model-torn-{}.jsonl", std::process::id()));
        let mut b = std::fs::read(fixtures().join("pages.jsonl")).unwrap(); b.extend(b"{\"$\":\"MessagePosted\",\"thread\":\"t1\",\"ms"); std::fs::write(&t, b).unwrap();
        let s = Store::open(&t).unwrap(); let m = Model::build(&s).unwrap(); assert_eq!(m.entries(), 895);
    }
}

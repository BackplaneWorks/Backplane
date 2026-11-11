//! The Rust read model. Entries stay in events.jsonl; the model holds only indexes of them:
//! per thread (and per sender thread, for messages that came from another) the (seq, ord) of each
//! entry. `seq` is the 1-based line of the event, `ord` the entry ordinal (core/thin Index.post
//! numbers every message, activity and reminder in log order), exactly the store.db hub
//! keeps; a page is thinq's Q.sql.page over those indexes (law: same log, same pages).
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

/// core/model Link.from: "t:<thread>:<rest>" names the thread a message came from.
pub fn link_from(id: &str) -> &str {
    match id.strip_prefix("t:") { Some(r) if r.contains(':') => r.split(':').next().unwrap_or(""), _ => "" }
}

/// What the hub needs to know of a thread without reading its entries (agents: where to run; views: its state).
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ThreadMeta { pub project: String, pub title: String, pub env: String, pub branch: String, pub worktree: String, pub provider: String, pub model: String,
    pub session: String, pub last_user_msg: String, pub last_entry: String, pub turn: String, pub pending: u64, pub created: u64, pub activity: u64, pub archived: bool, pub deleted: bool }
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProjectMeta { pub title: String, pub root: String, pub removed: bool }

#[derive(Default)]
pub struct Model {
    pub next_ord: u32,
    /// the log lines folded so far (every line counts, entries or not): where a checkpoint resumes
    pub lines: u32,
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
    pub fn build(store: &Store) -> Result<Model> { Self::fold_from(Model::default(), store) }
    /// The checkpoint (ckpt/model.bin) when it is of this log, then only the lines past it; else the whole log.
    pub fn open(store: &Store) -> Result<Model> {
        // a checkpoint is only ever a shortcut: one that cannot be read, however badly, is one that is not there
        let from = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Self::load(store))).unwrap_or_else(|_| { eprintln!("model: the checkpoint broke its reader: folding the log"); None });
        if from.is_some() { crate::startup::mark(&format!("model checkpoint loaded ({} lines)", from.as_ref().map_or(0, |m| m.lines))); }
        let loaded = from.is_some();
        let m = Self::fold_from(from.unwrap_or_default(), store)?;
        if loaded && crate::ckpt::check_on() { let s = store.clone(); let a = m.digest(); std::thread::spawn(move || { if let Ok(b) = Self::build(&s) { crate::ckpt::check("model", a, b.digest()) } }); }
        Ok(m)
    }
    /// Everything the model holds, as JSON (the long lists as their lengths and a hash): for `ckpt::check`.
    pub fn digest(&self) -> Value {
        let h = |v: &[Ent]| { let mut b = Vec::with_capacity(v.len() * 8); for e in v { b.extend(e.seq.to_le_bytes()); b.extend(e.ord.to_le_bytes()); } crate::ckpt::fnv(&b) };
        let hm = |m: &HashMap<String, Vec<Ent>>| -> Value { m.iter().map(|(k, v)| (k.clone(), json!([v.len(), h(v)]))).collect::<serde_json::Map<_, _>>().into() };
        let all: Vec<u8> = self.all.iter().flat_map(|s| s.to_le_bytes()).collect();
        json!({"next_ord": self.next_ord, "lines": self.lines, "all": [self.all.len(), crate::ckpt::fnv(&all)], "own": hm(&self.own), "sent": hm(&self.sent), "threads": self.threads, "projects": self.projects, "settings": self.settings})
    }
    fn fold_from(mut m: Model, store: &Store) -> Result<Model> {
        let from = m.lines as usize;
        for a in (from..store.len()).step_by(4096) {
            for (i, l) in store.range(a, a + 4096)?.iter().enumerate() { store.fold(a + i, l, |l| m.apply_line((a + i + 1) as u32, l)); }
        }
        Ok(m)
    }
    fn path(store: &Store) -> std::path::PathBuf { crate::ckpt::dir(store.path()).join("model.bin") }
    /// File: a JSON header line {v, lines, fp, next_ord, meta, all, own:[[thread, n]...], sent:[...]}, `meta` bytes of JSON (threads, projects, settings),
    /// then `all` as u32 LE, then every own and sent entry as (seq, ord) u32 LE pairs, in the header's order.
    pub fn save(&self, store: &Store) -> Result<bool> {
        if self.lines as usize != store.len() { return Ok(false) } // a write is between the log and the model: the next round
        let meta = serde_json::to_vec(&json!({"threads": self.threads, "projects": self.projects, "settings": self.settings}))?;
        let mut own: Vec<(&String, &Vec<Ent>)> = self.own.iter().collect(); own.sort_by(|a, b| a.0.cmp(b.0));
        let mut sent: Vec<(&String, &Vec<Ent>)> = self.sent.iter().collect(); sent.sort_by(|a, b| a.0.cmp(b.0));
        let header = serde_json::to_vec(&json!({"v": crate::ckpt::VERSION, "stamp": crate::ckpt::stamps::MODEL, "lines": self.lines, "fp": crate::ckpt::fingerprint(store, self.lines as usize), "next_ord": self.next_ord,
            "meta": meta.len(), "all": self.all.len(), "own": own.iter().map(|(k, v)| json!([k, v.len()])).collect::<Vec<_>>(), "sent": sent.iter().map(|(k, v)| json!([k, v.len()])).collect::<Vec<_>>()}))?;
        let mut bin = Vec::with_capacity(self.all.len() * 4 + (self.own.values().chain(self.sent.values()).map(Vec::len).sum::<usize>()) * 8);
        for s in &self.all { bin.extend(s.to_le_bytes()); }
        for (_, v) in own.iter().chain(sent.iter()) { for e in v.iter() { bin.extend(e.seq.to_le_bytes()); bin.extend(e.ord.to_le_bytes()); } }
        let mut header: Value = serde_json::from_slice(&header)?; header["sum"] = json!(crate::ckpt::fnv(&[&meta[..], &bin[..]].concat())); let header = serde_json::to_vec(&header)?;
        crate::ckpt::write_atomic(&Self::path(store), &[&header, b"\n", &meta, &bin])?;
        Ok(true)
    }
    fn load(store: &Store) -> Option<Model> {
        let raw = std::fs::read(Self::path(store)).ok()?;
        let nl = raw.iter().position(|b| *b == b'\n')?;
        let h: Value = serde_json::from_slice(&raw[..nl]).ok()?;
        if h["v"].as_u64()? != crate::ckpt::VERSION || h["stamp"].as_u64() != Some(crate::ckpt::stamps::MODEL) { return None }
        let lines = h["lines"].as_u64()? as usize;
        if lines > store.len() || h["fp"].as_u64()? != crate::ckpt::fingerprint(store, lines) { return None }
        let (meta_len, all_len) = (h["meta"].as_u64()? as usize, h["all"].as_u64()? as usize);
        if let Some(sum) = h["sum"].as_u64() { if crate::ckpt::fnv(raw.get(nl + 1..)?) != sum { return None } } // files from before the sum have only the checks below
        #[derive(Deserialize)] struct Meta { threads: HashMap<String, ThreadMeta>, projects: HashMap<String, ProjectMeta>, settings: std::collections::BTreeMap<String, u32> }
        let meta: Meta = serde_json::from_slice(raw.get(nl + 1..(nl + 1).checked_add(meta_len)?)?).ok()?;
        let mut at = nl + 1 + meta_len;
        let bin = &raw[at..]; at = 0;
        if bin.len() < all_len.checked_mul(4)? { return None }
        let all: Vec<u32> = (0..all_len).map(|i| crate::ckpt::le32(bin, i * 4)).collect(); at += all_len * 4;
        let mut read = |list: &Value| -> Option<HashMap<String, Vec<Ent>>> {
            let mut out = HashMap::new();
            for pair in list.as_array()? {
                let (name, n) = (pair[0].as_str()?.to_string(), pair[1].as_u64()? as usize);
                if bin.len() < at.checked_add(n.checked_mul(8)?)? { return None }
                out.insert(name, (0..n).map(|i| Ent { seq: crate::ckpt::le32(bin, at + i * 8), ord: crate::ckpt::le32(bin, at + i * 8 + 4) }).collect()); at += n * 8;
            }
            Some(out)
        };
        let (own, sent) = (read(&h["own"])?, read(&h["sent"])?);
        if at != bin.len() { return None }
        Some(Model { next_ord: h["next_ord"].as_u64()? as u32, lines: lines as u32, all, own, sent,
            threads: meta.threads, projects: meta.projects, settings: meta.settings })
    }
    pub fn apply_line(&mut self, seq: u32, line: &str) { self.lines = self.lines.max(seq); if let Ok(e) = serde_json::from_str::<Ev>(line) { self.apply(seq, &e) } }

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
        let text = if cut && kind == "act" && role != "handoff" /* a handoff's card is its text, whole */ { text.chars().take(513).collect() } else { text };
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
    /// How many entries a thread (or a log such as `@sub:<thread>`) holds.
    pub fn count(&self, thread: &str) -> usize { self.own.get(thread).map_or(0, Vec::len) }
    /// A thread's (or a log's) newest `n` entries, newest first, their text whole.
    pub fn tail(&self, store: &Store, thread: &str, n: usize) -> Vec<Row> {
        let own = self.own.get(thread).map(Vec::as_slice).unwrap_or(&[]);
        own.iter().rev().take(n).filter_map(|e| self.row(store, *e, thread, false).ok()).collect()
    }
    /// The thread's newest entries, newest first, up to the latest human prompt and the latest assistant text.
    pub fn latest_context(&self, store: &Store, thread: &str) -> Result<Vec<Row>> {
        let (own, sent) = (self.own.get(thread).map(Vec::as_slice).unwrap_or(&[]), self.sent.get(thread).map(Vec::as_slice).unwrap_or(&[]));
        let mut all: Vec<Ent> = own.iter().chain(sent.iter()).copied().collect();
        all.sort_by(|a, b| b.seq.cmp(&a.seq));
        let (mut human, mut agent, mut out) = (false, false, vec![]);
        for e in all {
            if human && agent { break }
            let r = self.row(store, e, thread, true)?;
            if r.kind == "msg" && r.role == "user" { human = true } else if r.kind == "msg" && r.role == "assistant" && !r.text.trim().is_empty() { agent = true }
            out.push(r);
        }
        Ok(out)
    }
    pub fn entries(&self) -> u32 { self.next_ord }
    /// A thread's own entries (log line and ordinal), oldest first: for a caller that reads them off the model's lock (kicad/histsteps.rs).
    pub fn own_ents(&self, thread: &str) -> Vec<Ent> { self.own.get(thread).cloned().unwrap_or_default() }
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
    fn fixtures() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/hub/fixtures") }
    struct TestStore { s: Store, _t: crate::plat::tmp::TempFile }
    impl std::ops::Deref for TestStore { type Target = Store; fn deref(&self) -> &Store { &self.s } }
    fn store() -> TestStore {
        let _t = crate::plat::tmp::file("model", "pages.jsonl");
        std::fs::copy(fixtures().join("pages.jsonl"), &_t).unwrap();
        let s = Store::open(&_t).unwrap();
        TestStore { s, _t }
    }
    #[test] fn link_from_matches() {
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
    // a model saved and loaded is the model, and the lines past the save are folded onto it
    #[test] fn a_model_saved_by_another_fold_is_folded_again() {
        let d = crate::plat::tmp::dir("model-st");
        let p = d.join("events.jsonl"); std::fs::copy(fixtures().join("pages.jsonl"), &p).unwrap();
        let s = Store::open(&p).unwrap(); let m = Model::open(&s).unwrap(); assert!(m.save(&s).unwrap()); assert!(Model::load(&s).is_some(), "its own fold: it loads");
        let f = Model::path(&s); let raw = std::fs::read(&f).unwrap(); let nl = raw.iter().position(|b| *b == b'\n').unwrap();
        let mut h: Value = serde_json::from_slice(&raw[..nl]).unwrap();
        let rewrite = |h: &Value| { let mut b = serde_json::to_vec(h).unwrap(); b.extend_from_slice(&raw[nl..]); std::fs::write(&f, b).unwrap(); };
        h["stamp"] = json!(2); rewrite(&h); assert!(Model::load(&s).is_none(), "another fold's");
        h.as_object_mut().unwrap().shift_remove("stamp"); rewrite(&h); assert!(Model::load(&s).is_none(), "from before stamps");
        assert_eq!(Model::open(&s).unwrap().digest(), m.digest(), "the log gives the same model");
    }
    #[test] fn a_saved_model_loads_and_takes_the_tail() {
        let d = crate::plat::tmp::dir("model-ck");
        let p = d.join("events.jsonl"); let all = std::fs::read_to_string(fixtures().join("pages.jsonl")).unwrap();
        let lines: Vec<&str> = all.lines().collect(); let cut = lines.len() / 2;
        std::fs::write(&p, lines[..cut].iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        let s = Store::open(&p).unwrap(); let m = Model::open(&s).unwrap(); assert!(m.save(&s).unwrap());
        { use std::io::Write; let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap(); for l in &lines[cut..] { writeln!(f, "{l}").unwrap(); } }
        let s2 = Store::open(&p).unwrap(); let loaded = Model::open(&s2).unwrap(); assert_eq!(loaded.lines as usize, s2.len(), "the tail was folded");
        let fresh = Model::build(&s2).unwrap();
        assert_eq!(loaded.digest(), fresh.digest(), "checkpoint plus tail is the whole fold");
        std::fs::write(&p, "{\"$\":\"ProjectCreated\",\"id\":\"q\",\"title\":\"q\",\"root\":\"/q\",\"at\":1}\n").unwrap();
        let other = Model::open(&Store::open(&p).unwrap()).unwrap(); assert_eq!(other.projects.len(), 1, "a checkpoint of another log is not used");
    }
    #[test] fn settings_hold_the_latest_value_by_key_and_removal() {
        let t = crate::plat::tmp::file("model-set", "events.jsonl");
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
        let t = crate::plat::tmp::file("model-torn", "events.jsonl");
        let mut b = std::fs::read(fixtures().join("pages.jsonl")).unwrap(); b.extend(b"{\"$\":\"MessagePosted\",\"thread\":\"t1\",\"ms"); std::fs::write(&t, b).unwrap();
        let s = Store::open(&t).unwrap(); let m = Model::build(&s).unwrap(); assert_eq!(m.entries(), 895);
    }
}

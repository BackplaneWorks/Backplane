//! The owner's side of threads elsewhere: which lines of this hub's log go to the owner's other machines
//! (`Fr.shared`, judged with the state after each line), the project each belongs to (`Fr.scope`, for the catalog),
//! and the answer to a pull (`POST /far/log`): the shared lines after `since`, a page at a time, each in its wire form
//! (`Hub::wire_item`: a token or a turn's input never leaves), with the catalog and this log's epoch.
//!
//! The index is one `u32` per shared line plus the small state `shared` needs (threads, asks, tasks); it is built from
//! the log in chunks off the async workers and kept by an observer, with the same buffering by sequence number as
//! `PeerTable` (a commit during the build is applied after it, once).
use super::{far::{self, Owned}, sign};
use crate::{api::Observer, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex}};

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Index {
    threads: HashMap<String, String>,   // thread -> its project
    asks: HashMap<String, String>,      // ask -> its thread (a closed ask stays: Fr.ask.shared reads it after the close)
    tasks: HashMap<String, String>,     // delegated task -> its parent thread
    projects: HashMap<String, (String, u64)>, // project -> (title, created)
    shared: Vec<u32>,                   // the log lines (1-based) that go
    lasts: HashMap<String, u64>,        // project -> its last shared line
    applied: u32, built: bool,
}
impl Index {
    /// `Fr.project.shared`: a project of this hub's own (not a mirror's) that exists
    fn project_shared(&self, id: &str) -> bool { !id.is_empty() && self.projects.contains_key(id) && !far::is(id) }
}
impl Owned for Index {
    fn thread(&self, id: &str) -> bool { self.threads.contains_key(id) }
    fn ask_thread(&self, id: &str) -> Option<String> { self.asks.get(id).cloned() }
    fn task_parent(&self, id: &str) -> Option<String> { self.tasks.get(id).cloned() }
}
fn s<'a>(e: &'a Value, k: &str) -> &'a str { e[k].as_str().unwrap_or("") }

impl Index {
    /// The project a shared change belongs to ("" for none: bots, routines, memory, space): the project itself, or the
    /// project of the thread it is about (Fr.scope).
    fn scope(&self, e: &Value) -> String {
        let th = |t: &str| self.threads.get(t).cloned().unwrap_or_default();
        match s(e, "$") {
            "ProjectCreated" | "ProjectRemoved" => s(e, "id").to_string(),
            "ThreadCreated" | "ThreadRenamed" | "ThreadSettled" | "ThreadUnsettled" | "ThreadWoke" | "ThreadArchived" | "ThreadPinned" | "ThreadForked" | "ThreadDeleted"
            | "ModesSet" | "ProviderSet" | "ThreadParentSet" | "ThreadLaunched" => th(s(e, "id")),
            "MessagePosted" | "MessageFrom" | "TurnChanged" | "PendingChanged" | "ActivityLogged" | "TurnQueued" | "QueueTaken" | "QueueEdited" | "QueueRemoved"
            | "QueueMoved" | "QueueTakenOne" | "TodosChanged" | "TodosSet" | "WorkSet" | "AskOpened" => th(s(e, "thread")),
            "TaskDelegated" => th(s(e, "parent")),
            "TaskStateSet" | "TaskDelivered" => th(self.tasks.get(s(e, "task")).map(String::as_str).unwrap_or("")),
            "AskClosed" => th(self.asks.get(s(e, "id")).map(String::as_str).unwrap_or("")),
            _ => String::new(),
        }
    }
    /// Fold one log line (1-based `seq`): the state first, then whether the change goes (the state after it).
    pub fn fold(&mut self, seq: u32, e: &Value) {
        match s(e, "$") {
            "ProjectCreated" => { self.projects.insert(s(e, "id").into(), (s(e, "title").into(), e["at"].as_u64().unwrap_or(0))); }
            "ThreadCreated" => { self.threads.insert(s(e, "id").into(), s(e, "project").into()); }
            "AskOpened" => { self.asks.insert(s(e, "id").into(), s(e, "thread").into()); }
            "TaskDelegated" => { self.tasks.insert(s(e, "task").into(), s(e, "parent").into()); }
            _ => {}
        }
        if far::shared(self, e) {
            self.shared.push(seq);
            let sc = self.scope(e);
            self.lasts.insert(sc, seq as u64);
        }
        self.applied = seq;
    }
    pub fn shared_after(&self, since: u64) -> &[u32] { let i = self.shared.partition_point(|n| (*n as u64) <= since); &self.shared[i..] }
    pub fn last(&self, project: &str) -> u64 { self.lasts.get(project).copied().unwrap_or(0) }
}

pub struct Feed { st: Mutex<(Index, Vec<(u32, Vec<Value>)>)>, tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<super::outbox::Batch>>> }
const CHUNK: usize = 20_000;
/// Line numbers copied from the index per slice (the lock is held for the copy only).
const SLICE: usize = 4096;
fn nums_last(v: &[(u32, String)]) -> Option<u32> { v.last().map(|x| x.0) }
/// A page is cut at about this many characters (one bigger line goes alone).
pub const PAGE_CHARS: usize = 400_000;

impl Feed {
    /// `Fr.allowed`: whether this hub takes the request `m` with params `p` from one of the owner's machines: only the methods of
    /// `Fr.methods`, and only about something this hub shares (a thread of its own, an ask in one, a project of its own, a bot); browsing and adding
    /// folders on this machine claims nothing.
    pub fn allowed(&self, m: &str, p: &Value) -> bool {
        if !far::may(m) { return false }
        let g = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("");
        let st = self.st.lock().unwrap();
        let idx = &st.0;
        match m {
            "ask.answer" => idx.ask_thread(g("id")).map_or(false, |t| idx.thread(&t) && !far::is(&t)),
            "bots.space" | "bots.routine_run" => !(if g("bot").is_empty() { g("id") } else { g("bot") }).is_empty(),
            // a post in a room of this hub's (the bots lane says whether the room exists)
            "bots.post" => { let r = g("room"); !r.is_empty() && !r.starts_with("dm:") && !far::is(r) }
            "thread.create" | "project.remove" => idx.project_shared(g("project")),
            "project.add" | "project.new" | "fs.list" | "fs.mkdir" | "fs.find" => true,
            _ => idx.thread(g("thread")) && !far::is(g("thread")),
        }
    }

    pub fn new() -> Arc<Feed> { Arc::new(Feed { st: Mutex::new((Index::default(), vec![])), tx: Mutex::new(None) }) }
    pub fn ready(&self) -> bool { self.st.lock().unwrap().0.built }
    pub fn with<R>(&self, f: impl FnOnce(&Index) -> R) -> R { f(&self.st.lock().unwrap().0) }
    pub fn observe(&self, events: &[Value], first_seq: u32) {
        let mut g = self.st.lock().unwrap();
        if !g.0.built { g.1.push((first_seq, events.to_vec())); return }
        let before = g.0.shared.len();
        for (i, e) in events.iter().enumerate() { let seq = first_seq + i as u32; if seq > g.0.applied { g.0.fold(seq, e) } }
        // what this commit shares goes to the owner's other machines (the push machine, outbox.rs)
        let ns: Vec<u32> = g.0.shared[before..].to_vec();
        drop(g);
        if !ns.is_empty() {
            let items: Vec<super::far::Item> = ns.iter().filter_map(|n| { let e = &events[(*n - first_seq) as usize]; Hub::wire_item(&e.to_string()).map(|c| super::far::Item { n: *n as u64, c }) }).collect();
            if let Some(tx) = self.tx.lock().unwrap().as_ref() { let _ = tx.send((first_seq as u64 - 1, first_seq as u64 + events.len() as u64 - 1, items)); }
        }
    }
    /// The receiver of what each commit shares (one batch per commit that shares anything).
    pub fn pushes(&self) -> tokio::sync::mpsc::UnboundedReceiver<super::outbox::Batch> { let (tx, rx) = tokio::sync::mpsc::unbounded_channel(); *self.tx.lock().unwrap() = Some(tx); rx }
    fn finish(&self, mut local: Index) {
        let mut g = self.st.lock().unwrap();
        local.built = true;
        let pending = std::mem::take(&mut g.1);
        g.0 = local;
        for (first, evs) in pending { for (i, e) in evs.iter().enumerate() { let seq = first + i as u32; if seq > g.0.applied { g.0.fold(seq, e) } } }
    }
    pub async fn start(self: &Arc<Self>, hub: &Arc<Hub>) {
        hub.register_observer(self.clone());
        let h = hub.clone();
        let me = self.clone();
        let local = tokio::task::spawn_blocking(move || {
            let len = h.store.len();
            let path = crate::ckpt::dir(h.store.path()).join("feed.json");
            let mut ix = crate::ckpt::load_json(&path, &h.store, crate::ckpt::stamps::FEED, |i: &Index| i.applied as usize).unwrap_or_default();
            crate::ckpt::saved("feed", ix.applied as usize);
            let mut at = ix.applied as usize;
            crate::ckpt::register(move |store| { let g = me.st.lock().unwrap(); let n = g.0.applied as usize; if g.0.built && crate::ckpt::due("feed", n) { crate::ckpt::save_json(&path, store, n, crate::ckpt::stamps::FEED, &g.0); crate::ckpt::saved("feed", n) } });
            while at < len {
                let to = (at + CHUNK).min(len);
                for (i, l) in h.store.range(at, to).unwrap_or_default().iter().enumerate() {
                    // (an undecodable line is skipped but still numbers the log: store.len() counts decodable lines only)
                    if let Ok(e) = serde_json::from_str::<Value>(l) { ix.fold((at + i + 1) as u32, &e) }
                }
                at = to;
            }
            ix.applied = len as u32; ix
        }).await.unwrap_or_default();
        self.finish(local);
    }
    /// The answer to a pull. `None`: not ready (the asker pulls again). `since`/`epoch` are the asker's. The index lock is
    /// held only to copy a few line numbers; the lines are read from the log off the async workers, so a pull never holds
    /// up a commit's observer.
    pub async fn answer(&self, hub: &Hub, since: u64, asked_epoch: &str, catalog: Value) -> Option<Value> {
        // The log's end is read ONCE, before any line: a line committed meanwhile is above `top`, so it is neither in the items
        // nor under the head, and the push of that line (n > head) still lands.
        let top = hub.store.len() as u64;
        self.answer_at(hub, top, since, asked_epoch, catalog).await
    }
    pub async fn answer_at(&self, hub: &Hub, top: u64, since: u64, asked_epoch: &str, catalog: Value) -> Option<Value> {
        let own = epoch(hub);
        let from = if super::farstore::epoch_changed(asked_epoch, &own) { 0 } else { since };
        if !self.ready() { return None }
        let store = hub.store.clone();
        let (mut items, mut room, mut more, mut last, mut cursor) = (vec![], PAGE_CHARS as i64, false, from, from);
        'pages: loop {
            // a bounded slice of line numbers, copied under the lock
            let nums: Vec<u32> = { let g = self.st.lock().unwrap(); g.0.shared_after(cursor).iter().take(SLICE).copied().take_while(|n| (*n as u64) <= top).collect() };
            if nums.is_empty() { break }
            let (s, r, page_empty) = (store.clone(), room, items.is_empty());
            let (got, left, stop) = tokio::task::spawn_blocking(move || {
                let (mut out, mut room, mut stop) = (vec![], r, false);
                for n in nums.iter().copied() {
                    let Ok(line) = s.get(n as usize - 1) else { continue };
                    let len = line.chars().count() as i64;
                    // (a PAGE's first line goes alone however big; every later one only while it fits, across slices too)
                    if !(page_empty && out.is_empty()) && len > room { stop = true; break }
                    room -= len; out.push((n, line));
                }
                (out, room, stop)
            }).await.ok()?;
            let full = nums_last(&got);
            room = left;
            for (n, line) in got {
                last = n as u64;
                let Some(c) = Hub::wire_item(&line) else { continue };
                items.push(json!({"n": n, "c": c}));
            }
            if stop { more = true; break 'pages }
            match full { Some(n) => cursor = n as u64, None => break }
        }
        let head = if more { last } else { top };
        Some(json!({"since": from, "catalog": catalog, "epoch": own, "head": head, "items": items, "more": more}))
    }
    /// Fr.catalog: every project this hub shares, so a mirror can list them all at once before their threads arrive.
    pub async fn catalog(&self, hub: &Arc<Hub>) -> Value {
        let m = hub.model().await;
        let g = self.st.lock().unwrap();
        let mut acts: HashMap<&str, u64> = HashMap::new();
        for t in m.threads.values() { let a = acts.entry(t.project.as_str()).or_insert(0); *a = (*a).max(t.activity) }
        let mut out: Vec<Value> = m.projects.iter().filter(|(id, p)| !p.removed && !far::is(id)).map(|(id, p)| {
            let created = g.0.projects.get(id.as_str()).map_or(0, |x| x.1);
            json!({"id": id, "title": p.title, "at": created.max(acts.get(id.as_str()).copied().unwrap_or(0)), "n": g.0.last(id)})
        }).collect();
        out.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        Value::Array(out)
    }
}
impl Observer for Feed { fn committed(&self, _h: &Arc<Hub>, events: &[Value], first_seq: u32) { self.observe(events, first_seq) } }

/// Far.history: the first durable change (its text), with a mark when the log holds lines this build cannot decode,
/// identifies the history; its signature is the epoch a peer pins. "" for an empty log. `view:N` is what is shared: a build that shares more
/// (view:4, the uberbot and the rooms) is another history to its mirrors, which start over and so get the older lines they never had.
pub fn epoch_of(first: &str, len: usize, seq: usize) -> String {
    let body = format!("{}{}|view:4", first, if len == seq { String::new() } else { format!("|unknown:{}", len.saturating_sub(seq)) });
    sign::header("00", 0, &body)
}
pub fn epoch(hub: &Hub) -> String {
    if hub.store.len() == 0 { return String::new() }
    epoch_of(&hub.store.get(0).unwrap_or_default(), hub.store.len() + hub.store.bad, hub.store.len())
}

#[cfg(test)]
mod tests;

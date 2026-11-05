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

#[derive(Default)]
pub struct Index {
    threads: HashMap<String, String>,   // thread -> its project
    asks: HashMap<String, String>,      // ask -> its thread (a closed ask stays: Fr.ask.shared reads it after the close)
    tasks: HashMap<String, String>,     // delegated task -> its parent thread
    projects: HashMap<String, (String, u64)>, // project -> (title, created)
    shared: Vec<u32>,                   // the log lines (1-based) that go
    lasts: HashMap<String, u64>,        // project -> its last shared line
    applied: u32, built: bool,
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

pub struct Feed { st: Mutex<(Index, Vec<(u32, Vec<Value>)>)> }
const CHUNK: usize = 20_000;
/// A page is cut at about this many characters (one bigger line goes alone).
pub const PAGE_CHARS: usize = 400_000;

impl Feed {
    pub fn new() -> Arc<Feed> { Arc::new(Feed { st: Mutex::new((Index::default(), vec![])) }) }
    pub fn ready(&self) -> bool { self.st.lock().unwrap().0.built }
    pub fn with<R>(&self, f: impl FnOnce(&Index) -> R) -> R { f(&self.st.lock().unwrap().0) }
    pub fn observe(&self, events: &[Value], first_seq: u32) {
        let mut g = self.st.lock().unwrap();
        if !g.0.built { g.1.push((first_seq, events.to_vec())); return }
        for (i, e) in events.iter().enumerate() { let seq = first_seq + i as u32; if seq > g.0.applied { g.0.fold(seq, e) } }
    }
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
        let local = tokio::task::spawn_blocking(move || {
            let (mut ix, len) = (Index::default(), h.store.len());
            let mut at = 0;
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
    /// The answer to a pull. `None`: not ready (the asker pulls again). `since`/`epoch` are the asker's.
    pub fn answer(&self, hub: &Hub, since: u64, asked_epoch: &str, catalog: Value) -> Option<Value> {
        let own = epoch(hub);
        let from = if super::farstore::epoch_changed(asked_epoch, &own) { 0 } else { since };
        let g = self.st.lock().unwrap();
        if !g.0.built { return None }
        let mut items = vec![]; let (mut room, mut more, mut last) = (PAGE_CHARS as i64, false, from);
        for &n in g.0.shared_after(from) {
            let Ok(line) = hub.store.get(n as usize - 1) else { continue };
            if !items.is_empty() && (line.chars().count() as i64) > room { more = true; break }
            room -= line.chars().count() as i64; last = n as u64;
            let Some(c) = Hub::wire_item(&line) else { continue };
            items.push(json!({"n": n, "c": c}));
        }
        let head = if more { last } else { hub.store.len() as u64 };
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
/// identifies the history; its signature is the epoch a peer pins. "" for an empty log.
pub fn epoch_of(first: &str, len: usize, seq: usize) -> String {
    let body = format!("{}{}|view:3", first, if len == seq { String::new() } else { format!("|unknown:{}", len.saturating_sub(seq)) });
    sign::header("00", 0, &body)
}
pub fn epoch(hub: &Hub) -> String {
    if hub.store.len() == 0 { return String::new() }
    epoch_of(&hub.store.get(0).unwrap_or_default(), hub.store.len() + hub.store.bad, hub.store.len())
}

#[cfg(test)]
mod tests;

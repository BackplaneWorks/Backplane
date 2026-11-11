//! The slice of the log the thread, project and queue requests decide from, folded here and kept by
//! catching up with the store (like agents/world.rs): sidebar order keys and snoozes, queues, open asks, the few
//! settings these rules read. Bounded by threads, projects and queued messages, not by the log.
use crate::store::Store;
use anyhow::Result;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct T { pub pinned: bool, pub forced: u8, pub settled_at: u64, pub unsettled_at: u64, pub session: String, pub project: String, pub title: String, pub provider: String, pub model: String, pub branch: String, pub worktree: String, pub env: String, pub turn: String,
    pub archived: bool, pub deleted: bool, pub runtime: String, pub interaction: String }
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct P { pub title: String, pub root: String, pub at: u64, pub removed: bool }
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Nav { pub until: u64, pub pinned: String, pub active: String }
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Ask { pub thread: String, pub kind: String, pub tool: String }

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Fold {
    seen: usize,
    pub threads: BTreeMap<String, T>, pub projects: BTreeMap<String, P>, pub navs: HashMap<String, Nav>,
    pub queues: HashMap<String, Vec<(String, String)>>, pub asks: BTreeMap<String, Ask>, pub forks: HashMap<String, String>,
    pub st: HashMap<String, String>, /// Switch.count: entries filed under each thread by that thread
    pub count: HashMap<String, usize>, /// the number the request's ids are built from (Hub::alloc_n), else the log length
    #[serde(skip)] pub alloc: Option<usize>, /// the first messages of home threads ("home-<msg>"), so a retry knows whether the message arrived
    pub home_msgs: std::collections::HashSet<String>,
}

const KEYS: &[&str] = &["thread.parent.", "bg.", "watch.", "carry.wait.", "thread.env", "agent.provider", "agent.model", "projects.missing", "merged.", "provider.resume.", "restart.retry.", "resume.", "wt.again.", "thread.config.hold.", "seen.", "tz.machine", "account.thread.", "account.default.", "accounts.inventory", "account.session.", "account.seen.", "account.chosen.", "account.limit.", "fallback.intent."];
fn tracked(k: &str) -> bool { KEYS.iter().any(|p| k == *p || (p.ends_with('.') && k.starts_with(p))) }

impl Fold {
    fn path(store: &Store) -> std::path::PathBuf { crate::ckpt::dir(store.path()).join("commands.json") }
    /// The saved fold (ckpt/commands.json) when it is of this log; `sync` folds what is past it.
    pub fn load(store: &Store) -> Fold { let f = crate::ckpt::load_json(&Self::path(store), store, crate::ckpt::stamps::COMMANDS, |f: &Fold| f.seen); if let Some(f) = &f { crate::ckpt::saved("commands", f.seen);
        if crate::ckpt::check_on() { let (s, n, a) = (store.clone(), f.seen, serde_json::to_value(f).unwrap_or_default()); std::thread::spawn(move || { let mut g = Fold::default(); let _ = g.sync_to(&s, n); crate::ckpt::check("commands fold", a, serde_json::to_value(&g).unwrap_or_default()) }); } } f.unwrap_or_default() }
    /// Bring it up to the log and save it (the first round of a fold nobody has asked for yet reads the whole log once, here, off the request path).
    pub fn save(&mut self, store: &Store) { let _ = self.sync(store); if crate::ckpt::due("commands", self.seen) { crate::ckpt::save_json(&Self::path(store), store, self.seen, crate::ckpt::stamps::COMMANDS, self); crate::ckpt::saved("commands", self.seen) } }
    /// Events seen: hub's `Hub.seq` (the id source for generated ids).
    pub fn n(&self) -> usize { self.alloc.unwrap_or(self.seen) }
    pub fn setting(&self, k: &str) -> &str { self.st.get(k).map(String::as_str).unwrap_or("") }
    /// M.Setting.get(s, k, d): a stored value, an empty one included, else d.
    pub fn get<'a>(&'a self, k: &str, d: &'a str) -> &'a str { self.st.get(k).map(String::as_str).unwrap_or(d) }
    pub fn sync(&mut self, store: &Store) -> Result<()> { self.sync_to(store, store.len()) }
    pub fn sync_to(&mut self, store: &Store, total: usize) -> Result<()> {
        while self.seen < total {
            let b = (self.seen + 4096).min(total);
            let from = self.seen; for (i, l) in store.range(from, b)?.iter().enumerate() { store.fold(from + i, l, |l| self.apply(l)) }
            self.seen = b;
        }
        Ok(())
    }
    /// Queue.texts: the waiting messages as one text, a blank line between; an empty one at the head is skipped (Queue.join).
    pub fn queue_text(&self, th: &str) -> String { self.queue(th).iter().fold(String::new(), |acc, (_, t)| if acc.is_empty() { t.clone() } else { format!("{acc}\n\n{t}") }) }
    /// Queue.waiting: a message waits (handed-over context alone never starts a turn).
    /// The holds on one login, its model holds with them (settings/fallback.rs `hold_key`): what the person's own choice of it overrules.
    pub fn holds_of(&self, provider: &str, id: &str, epoch: u64) -> Vec<String> { let k = crate::settings::fallback::hold_key(provider, id, epoch); self.st.iter().filter(|(x, v)| x.starts_with(&k) && !v.is_empty()).map(|(x, _)| x.clone()).collect() }
    pub fn waiting(&self, th: &str) -> bool { self.queue(th).iter().any(|(m, _)| !m.starts_with("handoff:")) }
    pub fn queue(&self, th: &str) -> &[(String, String)] { self.queues.get(th).map(Vec::as_slice).unwrap_or(&[]) }
    pub fn nav(&self, id: &str) -> Nav { self.navs.get(id).cloned().unwrap_or_default() }

    pub fn apply(&mut self, line: &str) {
        let Some(t) = crate::session::event_type(line) else { return };
        const READ: &[&str] = &["ProjectCreated", "ProjectRemoved", "ThreadCreated", "ThreadRenamed", "ThreadArchived", "ThreadDeleted", "ThreadForked", "ThreadSnoozed", "ThreadOrdered", "ProjectOrdered",
            "TurnQueued", "QueueTaken", "QueueEdited", "QueueRemoved", "QueueMoved", "QueueTakenOne", "AskOpened", "AskClosed", "SettingSet", "ProviderSet", "TurnChanged", "WorktreeSet", "ModesSet", "WorkSet", "WorktreeSkipped", "ThreadParentSet", "ThreadPinned", "ThreadSettled", "ThreadUnsettled", "ThreadMerged", "WorktreeRemoved", "SessionSet", "MessagePosted", "ActivityLogged", "Reminded"];
        if !READ.contains(&t) { return }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        let g = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let at = v.get("at").and_then(Value::as_u64).unwrap_or(0);
        match t {
            "ProjectCreated" => { self.projects.insert(g("id"), P { title: g("title"), root: g("root"), at, removed: false }); }
            "ProjectRemoved" => if let Some(p) = self.projects.get_mut(&g("id")) { p.removed = true },
            "ThreadCreated" => { self.threads.insert(g("id"), T { project: g("project"), title: g("title"), env: g("env"), branch: g("branch"), worktree: g("worktree"), provider: g("provider"), model: g("model"),
                turn: "idle".into(), runtime: "full-access".into(), interaction: "default".into(), ..Default::default() }); }
            "ThreadRenamed" => if let Some(x) = self.threads.get_mut(&g("id")) { x.title = g("title") },
            "ThreadArchived" => if let Some(x) = self.threads.get_mut(&g("id")) { x.archived = v.get("archived").and_then(Value::as_bool).unwrap_or(false) },
            "ThreadDeleted" => if let Some(x) = self.threads.get_mut(&g("id")) { x.deleted = true },
            "ThreadForked" => { self.forks.insert(g("id"), g("source")); }
            "ThreadSnoozed" => self.navs.entry(g("id")).or_default().until = v.get("until").and_then(Value::as_u64).unwrap_or(0),
            "ThreadOrdered" => { let n = self.navs.entry(g("id")).or_default(); if g("section") == "pinned" { n.pinned = g("key") } else { n.active = g("key") } }
            "ProjectOrdered" => self.navs.entry(format!("project:{}", g("id"))).or_default().active = g("key"),
            "TurnQueued" => self.queues.entry(g("thread")).or_default().push((g("msg"), g("text"))),
            "QueueTaken" => { self.queues.insert(g("thread"), vec![]); }
            "QueueEdited" => if let Some(q) = self.queues.get_mut(&g("thread")) { for i in q.iter_mut() { if i.0 == g("msg") { i.1 = g("text") } } },
            "QueueRemoved" | "QueueTakenOne" => if let Some(q) = self.queues.get_mut(&g("thread")) { q.retain(|i| i.0 != g("msg")) },
            "QueueMoved" => if let Some(q) = self.queues.get_mut(&g("thread")) {
                let msg = g("msg"); let to = v.get("to").and_then(Value::as_u64).unwrap_or(0) as usize;
                if let Some(i) = q.iter().position(|x| x.0 == msg) { let it = q.remove(i); let to = to.min(q.len()); q.insert(to, it) }
            },
            "AskOpened" => { self.asks.insert(g("id"), Ask { thread: g("thread"), kind: g("kind"), tool: g("tool") }); }
            "AskClosed" => { self.asks.remove(&g("id")); }
            "SettingSet" => { let k = g("key"); if tracked(&k) { self.st.insert(k, g("value")); } }
            "WorkSet" => { let (k, th) = (g("kind"), g("thread")); if k == "bg" || k == "watch" { let val = g("rows"); let key = format!("{k}.{th}"); if val.is_empty() { self.st.remove(&key); } else { self.st.insert(key, val); } } }
            "ThreadParentSet" => { let (k, val) = (format!("thread.parent.{}", g("id")), g("parent")); if val.is_empty() { self.st.remove(&k); } else { self.st.insert(k, val); } }
            // State.switch.n: the provider left keeps its session and how many entries it had seen; the one taken up resumes its own session
            "ProviderSet" => {
                let th = g("id"); let n = self.count.get(&th).copied().unwrap_or(0);
                if let Some(x) = self.threads.get(&th).cloned() {
                    self.st.insert(format!("session.{th}.{}", x.provider), x.session.clone()); self.st.insert(format!("seen.{th}.{}", x.provider), n.to_string());
                    let new_se = self.st.get(&format!("session.{th}.{}", g("provider"))).cloned().unwrap_or_default();
                    let x = self.threads.get_mut(&th).unwrap(); x.provider = g("provider"); x.model = g("model"); x.session = new_se;
                }
            }
            "ThreadPinned" => if let Some(x) = self.threads.get_mut(&g("id")) { x.pinned = v.get("pinned").and_then(Value::as_bool).unwrap_or(false) },
            // Thread.settled: a manual settle forces the shelf, clears the pin; unsettled puts it back on top of the active shelf
            "ThreadSettled" => if let Some(x) = self.threads.get_mut(&g("id")) { if v.get("manual").and_then(Value::as_bool).unwrap_or(false) { x.forced = 1 } x.settled_at = at; x.pinned = false },
            "ThreadUnsettled" => if let Some(x) = self.threads.get_mut(&g("id")) { x.forced = 2; x.unsettled_at = x.settled_at.max(at) },
            "ThreadMerged" => { let id = g("id"); if let Some(x) = self.threads.get_mut(&id) { x.forced = 1; x.settled_at = at; x.pinned = false } self.st.insert(format!("merged.{id}"), g("url")); }
            "WorktreeRemoved" => if let Some(x) = self.threads.get_mut(&g("id")) { x.worktree = String::new() },
            "SessionSet" => if let Some(x) = self.threads.get_mut(&g("id")) { x.session = g("session") },
            "MessagePosted" | "ActivityLogged" | "Reminded" => { let th = g("thread"); if !th.is_empty() { if t == "MessagePosted" && th.starts_with("home-") { self.home_msgs.insert(format!("{th}\u{1}{}", g("msg"))); } if t == "MessagePosted" && g("role") == "user" { if let Some(x) = self.threads.get_mut(&th) { x.unsettled_at = at } } *self.count.entry(th).or_default() += 1 } }
            "TurnChanged" => if let Some(x) = self.threads.get_mut(&g("thread")) { x.turn = g("state") },
            "WorktreeSet" => if let Some(x) = self.threads.get_mut(&g("id")) { x.branch = g("branch"); x.worktree = g("worktree") },
            "ModesSet" => if let Some(x) = self.threads.get_mut(&g("id")) { x.runtime = g("runtime"); x.interaction = g("interaction"); x.model = g("model") },
            _ => {}
        }
    }
}

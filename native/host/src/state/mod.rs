//! The read model's fold in Rust: projects, threads, entry order, settings and the sequence number,
//! equal to Backplane's `M.State.replay` for the events it covers.
//! Its twin is checked by the differential oracle (test/parity/oracle): both fold the same
//! events.jsonl and must print the same canonical digest (test/parity/oracle/DIGEST.md).
//!
//! Covered: every event decoder knows counts toward `seq` (State.tick); these change state:
//! ProjectCreated/Removed, ThreadCreated/Renamed/Settled/Unsettled/Woke/Archived/Pinned/Deleted, MessagePosted,
//! ActivityLogged, TurnChanged (+ the todos bookkeeping it does to settings), PendingChanged, SessionSet,
//! WorktreeSet/Skipped/Removed, ThreadMerged, SettingSet, ThreadParentSet, ThreadForked, ProviderSet, ModesSet, UsageSet (the usage.<thread> setting). Not folded here: bots,
//! mail, tasks, asks, devices, nav, queues, TurnUsage's token totals (tok.*), deploy, routines, todos lists.
use serde_json::Value;
use std::collections::HashMap;

// the digest-skipped families (UberOp, wakes, locks, TurnUsage, deploys, todos by tools/person/agent, Reminded): the state lane's
pub mod fam;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Turn { Idle, Running, Completed, Interrupted, Failed }
impl Turn {
    pub fn word(self) -> &'static str { match self { Turn::Idle => "idle", Turn::Running => "running", Turn::Completed => "completed", Turn::Interrupted => "interrupted", Turn::Failed => "failed" } }
    fn parse(s: &str) -> Option<Turn> { Some(match s { "idle" => Turn::Idle, "running" => Turn::Running, "completed" => Turn::Completed, "interrupted" => Turn::Interrupted, "failed" => Turn::Failed, _ => return None }) }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TaskSt { Queued, Running, Done, Failed, Cancelled }
impl TaskSt {
    pub fn terminal(self) -> bool { matches!(self, TaskSt::Done | TaskSt::Failed | TaskSt::Cancelled) }
    fn parse(s: &str) -> Option<TaskSt> { Some(match s { "queued" => TaskSt::Queued, "running" => TaskSt::Running, "done" => TaskSt::Done, "failed" => TaskSt::Failed, "cancelled" => TaskSt::Cancelled, _ => return None }) }
    /// TaskState.next: terminal states stay; a running task never goes back to queued
    fn next(cur: TaskSt, want: TaskSt) -> TaskSt { if cur.terminal() { cur } else if cur == TaskSt::Running && want == TaskSt::Queued { TaskSt::Running } else { want } }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Override { None, ForcedSettled, ForcedActive }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Project { pub title: String, pub root: String, pub created: u64, pub removed: bool }
/// Navigation metadata only: no bot persona, posts, room history or second world fold.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BotNav { pub name: String, pub thread: String, pub look: u64, pub created: u64, pub talked: u64 }
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RoomNav { pub name: String, pub members: usize }
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Thread {
    pub project: String, pub title: String, pub created: u64, pub activity: u64, pub turn: Turn, pub pending: u64,
    pub archived: bool, pub override_: Override, pub settled_at: u64, pub unsettled_at: u64, pub pinned: bool,
    pub worktree_mode: bool, pub branch: String, pub worktree: String, pub provider: String, pub model: String, pub session: String, pub deleted: bool,
}
impl Thread {
    /// Thread.blank: what an event for an unknown thread finds (hidden: archived and deleted).
    fn blank() -> Thread {
        Thread { project: String::new(), title: String::new(), created: 0, activity: 0, turn: Turn::Idle, pending: 0, archived: true, override_: Override::None,
                 settled_at: 0, unsettled_at: 0, pinned: false, worktree_mode: false, branch: String::new(), worktree: String::new(),
                 provider: String::new(), model: String::new(), session: String::new(), deleted: true }
    }
}

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct State {
    pub projects: HashMap<String, Project>,
    pub threads: HashMap<String, Thread>,
    /// entry ids per thread, oldest first (State.mine is newest first)
    pub entries: HashMap<String, Vec<String>>,
    pub settings: HashMap<String, String>,
    pub seq: u64,
    /// AskOpened/AskClosed: ask id -> (thread, open)
    pub asks: HashMap<String, (String, bool)>,
    /// TaskDelegated/TaskStateSet: task id -> (parent thread, child thread, terminal)
    pub tasks: HashMap<String, (String, String, bool, TaskSt)>,
    /// TurnQueued and its edits: a thread's waiting messages (msg id, text), next first
    pub queue: HashMap<String, Vec<(String, String)>>,
    /// bot id -> its thread (BotSet/BotDeleted): a task delegated by a bot's thread seeds no thread.parent
    bot_threads: HashMap<String, String>,
    pub bots_nav: HashMap<String, BotNav>,
    pub rooms_nav: HashMap<String, RoomNav>,
    /// ThreadSnoozed: the time a thread's snooze ends (Nav.until); 0 / absent: never snoozed
    pub snooze_until: HashMap<String, u64>,
    /// ModesSet: a thread's permission mode when it is not the default ("approval-required", "auto-accept-edits", "auto"); absent: full access
    pub runtime: HashMap<String, String>,
    /// AskOpened/AskClosed: the open asks' content, oldest first: (id, thread, kind, tool, detail, options). Bounded by the asks that are open.
    pub ask_data: Vec<(String, String, String, String, String, String)>,
    /// ThreadOrdered/ProjectOrdered: manual order keys (hex) per thread id, and per "project:<id>" for a project's place; (pinned key, active key)
    pub order_keys: HashMap<String, (String, String)>,
    /// RunSet: thread -> the bot it runs for (a mail run's thread)
    pub runs: HashMap<String, String>,
    /// entries whose own thread is the key (Switch.count: the ones filed under a sender for other threads do not count)
    own: HashMap<String, u64>,
    /// events the fold counts but does not (fully) model, by kind: reported by rust-digest so a gap is never silent
    pub unmodelled: HashMap<String, u64>,
    /// what the hub-side policies read that the digest does not show (fam.rs): bounded by tasks, threads and thread pairs
    pub ext: fam::Ext,
    /// the hub-side policy copy keeps no entry ids (the digest does): set by state::host, entries stay empty
    pub lean: bool,
}

/// every event name decoder knows
const KNOWN: &[&str] = &["ProjectCreated", "ProjectRemoved", "ThreadCreated", "ThreadRenamed", "MessagePosted", "TurnChanged", "PendingChanged", "ActivityLogged",
    "ThreadSettled", "ThreadUnsettled", "ThreadArchived", "ThreadPinned", "ThreadDeleted", "WorktreeSet", "SessionSet", "ModesSet", "TaskDelegated", "TaskStateSet",
    "TaskDelivered", "SettingSet", "TurnQueued", "QueueTaken", "AskOpened", "AskClosed", "DeviceRegistered", "DeviceDropped", "ThreadSnoozed", "ThreadOrdered",
    "WorktreeSkipped", "ThreadMerged", "WorktreeRemoved", "ProjectOrdered", "QueueEdited", "QueueRemoved", "QueueMoved", "QueueTakenOne", "ThreadForked", "ProviderSet",
    "TodosSet", "UsageSet", "BotSet", "BotDeleted", "RoomSet", "RoomDeleted", "RoomPosted", "HookSet", "HookHit", "RoutineSet", "RoutineDeleted", "RoutineFired",
    "MemorySaved", "MemoryForgot", "SpaceSet", "PeerSet", "BotWoke", "SpacePageSet", "WorkSet", "UberSet", "UberOp", "ThreadLaunched", "WakeSet", "WakeDropped",
    "LockChanged", "RoutinePlan", "RoutineSlot", "RoutineDone", "Reminded", "TaskSet", "TaskDeleted", "MessageFrom", "ConvSet", "MailPosted", "MailMarked", "MailTagged",
    "RunSet", "CharterSet", "DecisionLogged", "TurnUsage", "DeployAsked", "DeployBuilt", "ThreadParentSet", "ThreadWoke", "TodosChanged", "DevicePrefs", "PushOrdered", "PushSent", "PushCleared"];

/// Link.from: the sender in a link id "t:<from>:<n>" ("" for any other id): the second ':'-field when the first is "t" and a third follows
fn link_from(id: &str) -> &str {
    let Some(rest) = id.strip_prefix("t:") else { return "" };
    match rest.split_once(':') { Some((from, _)) => from, None => "" }
}

// as Change.decode: a missing or mistyped field reads "" / 0 (get_str / get_nat defaults), so an event still counts in seq
fn s<'a>(e: &'a Value, k: &str) -> Option<&'a str> { Some(e.get(k).and_then(|v| v.as_str()).unwrap_or("")) }
fn n(e: &Value, k: &str) -> Option<u64> { Some(fam::json::gnat(e, k, 0)) }

/// Setting.blank_gone: an empty value removes these keys
fn blank_gone(k: &str) -> bool {
    ["told.", "rule.", "board.", "launch.input.", "fallback.input.", "voice.e.", "voice.l."].iter().any(|p| k.starts_with(p))
}

impl State {
    pub fn new() -> State { State::default() }
    /// Uber.is: the uberbot of this machine
    fn uber_is(&self, bot: &str) -> bool { !bot.is_empty() && !bot.contains('~') && self.settings.get("uber.bot").map_or(false, |b| b == bot) }
    /// Private.is: a coordinator's back office is a thread of its own, never the person's chat
    pub fn private_thread(&self, thread: &str) -> bool {
        let owner = self.settings.get(&format!("mail.private.owner.{thread}")).map(|s| s.as_str()).unwrap_or("");
        !owner.is_empty() && self.uber_is(owner) && thread.starts_with(&format!("mailcoord-{owner}-"))
    }
    /// Run.is: a private thread, or a mail run the person's setting or triage makes visible work of a bot (Runs.keep hides these from the sidebar)
    pub fn run_hidden(&self, thread: &str) -> bool {
        if self.private_thread(thread) { return true }
        match self.runs.get(thread) {
            Some(bot) => self.settings.get("mail.on").map_or(false, |v| v == "on") || (self.uber_is(bot) && self.bot_threads.get(bot).map_or(false, |t| !t.is_empty())),
            None => false,
        }
    }
    /// BotSet's thread: a task delegated by a bot's thread is not hidden under it
    pub fn bot_threads_has(&self, thread: &str) -> bool { self.bot_threads.values().any(|t| t == thread) }
    pub fn bot_of_thread(&self, thread: &str) -> Option<String> { self.bot_threads.iter().find(|(_, t)| t.as_str() == thread).map(|(b, _)| b.clone()) }

    fn thread_mut(&mut self, id: &str) -> &mut Thread { self.threads.entry(id.to_string()).or_insert_with(Thread::blank) }
    fn set(&mut self, k: &str, v: &str) {
        if self.lean && !fam::policy::lean_keeps(k) { return }
        let kept; let v = if self.lean { kept = fam::policy::lean_value(k, v); kept.as_str() } else { v };
        if v.is_empty() && (blank_gone(k) || self.lean && fam::policy::gate_presence(k)) { self.settings.remove(k); } else { self.settings.insert(k.to_string(), v.to_string()); }
    }
    fn get(&self, k: &str, d: &str) -> String { self.settings.get(k).cloned().unwrap_or_else(|| d.to_string()) }

    /// One event line. Returns false when decoder would refuse it (it then counts nowhere).
    pub fn apply_line(&mut self, line: &str) -> bool {
        match serde_json::from_str::<Value>(line) { Ok(v) => self.apply(&v), Err(_) => false }
    }

    pub fn apply(&mut self, e: &Value) -> bool {
        let Some(kind) = s(e, "$") else { return false };
        if !KNOWN.contains(&kind) { return false }
        // Change.decode is lenient: a missing or mistyped field takes its default and the event still counts; only an unknown `$` is dropped
        match self.fold(kind, e) { Some(()) => { self.seq += 1; true } None => false }
    }

    fn fold(&mut self, kind: &str, e: &Value) -> Option<()> {
        match kind {
            "ProjectCreated" => { let id = s(e, "id")?; self.projects.insert(id.into(), Project { title: s(e, "title")?.into(), root: s(e, "root")?.into(), created: n(e, "at")?, removed: false }); }
            "ProjectRemoved" => { let id = s(e, "id")?; let p = self.projects.entry(id.into()).or_insert(Project { title: String::new(), root: String::new(), created: 0, removed: true }); p.removed = true; }
            "ThreadCreated" => {
                let id = s(e, "id")?; let at = n(e, "at")?;
                let mode = s(e, "env")?;
                self.threads.insert(id.into(), Thread { project: s(e, "project")?.into(), title: s(e, "title")?.into(), created: at, activity: at, turn: Turn::Idle, pending: 0,
                    archived: false, override_: Override::None, settled_at: 0, unsettled_at: 0, pinned: false, worktree_mode: mode == "worktree",
                    branch: s(e, "branch")?.into(), worktree: s(e, "worktree")?.into(), provider: s(e, "provider")?.into(), model: s(e, "model")?.into(), session: String::new(), deleted: false });
            }
            "ThreadRenamed" => { let id = s(e, "id")?; let t = s(e, "title")?.to_string(); self.thread_mut(id).title = t; }
            "MessagePosted" => {
                let th = s(e, "thread")?; let id = s(e, "msg")?; let role = s(e, "role")?; let at = n(e, "at")?; let text = s(e, "text")?;
                if role == "user" { let t = self.thread_mut(th); t.activity = at; t.unsettled_at = at; }
                self.post(th, id); fam::on_post(self, th, id, at, Some((role, text)));
            }
            "TurnChanged" => {
                let th = s(e, "thread")?; let st = Turn::parse(s(e, "state")?).unwrap_or(Turn::Idle); let at = n(e, "at")?;
                self.todos_turn(st, th, at);
                let t = self.thread_mut(th); t.activity = t.activity.max(at); t.turn = st;
            }
            "PendingChanged" => { let th = s(e, "thread")?; let c = n(e, "count")?; self.thread_mut(th).pending = c; }
            "ActivityLogged" => { let th = s(e, "thread")?; let id = s(e, "act")?; s(e, "tone")?; s(e, "kind")?; s(e, "summary")?; let at = n(e, "at")?; self.post(th, id); fam::on_post(self, th, id, at, None); }
            "ThreadSettled" => {
                let id = s(e, "id")?; let manual = fam::json::gbool(e, "manual", false); let at = n(e, "at")?;
                let t = self.thread_mut(id); if manual { t.override_ = Override::ForcedSettled } t.settled_at = at; t.pinned = false;
            }
            "ThreadUnsettled" => {
                let id = s(e, "id")?; let at = n(e, "at")?;
                let t = self.thread_mut(id); t.activity = t.activity.max(at); t.override_ = Override::ForcedActive; t.unsettled_at = t.settled_at.max(at);
            }
            "ThreadWoke" => {
                let id = s(e, "id")?; let at = n(e, "at")?;
                let t = self.thread_mut(id); t.override_ = Override::None;
                if !(t.unsettled_at >= t.settled_at) { t.unsettled_at = t.settled_at.max(at); }
            }
            "ThreadArchived" => { let id = s(e, "id")?; let b = fam::json::gbool(e, "archived", false); self.thread_mut(id).archived = b; }
            "ThreadPinned" => { let id = s(e, "id")?; let b = fam::json::gbool(e, "pinned", false); self.thread_mut(id).pinned = b; }
            "ThreadDeleted" => { let id = s(e, "id")?; self.thread_mut(id).deleted = true; }
            "WorktreeSet" => { let id = s(e, "id")?; let (b, w) = (s(e, "branch")?.to_string(), s(e, "worktree")?.to_string()); let t = self.thread_mut(id); t.worktree_mode = true; t.branch = b; t.worktree = w; }
            "WorktreeSkipped" => { let id = s(e, "id")?; let t = self.thread_mut(id); t.worktree_mode = false; t.branch.clear(); t.worktree.clear(); }
            "WorktreeRemoved" => { let id = s(e, "id")?; self.thread_mut(id).worktree.clear(); }
            "SessionSet" => { let id = s(e, "id")?; let se = s(e, "session")?.to_string(); self.thread_mut(id).session = se; }
            "ModesSet" => { let id = s(e, "id")?; let m = s(e, "model")?.to_string(); let rt = s(e, "runtime")?.to_string(); s(e, "interaction")?; self.thread_mut(id).model = m;
                // the permission mode (Thread.runtime): only a mode other than the default is kept, so the map is bounded by the threads that changed it
                if matches!(rt.as_str(), "approval-required" | "auto-accept-edits" | "auto") { self.runtime.insert(id.to_string(), rt); } else { self.runtime.remove(id); } }
            "ThreadMerged" => {
                let id = s(e, "id")?; let url = s(e, "url")?.to_string(); let at = n(e, "at")?;
                let t = self.thread_mut(id); t.override_ = Override::ForcedSettled; t.settled_at = at; t.pinned = false;
                self.set(&format!("merged.{id}"), &url);
            }
            "SettingSet" => { let (k, v) = (s(e, "key")?, s(e, "value")?); self.set(k, v); }
            "ThreadParentSet" => { let (id, p) = (s(e, "id")?, s(e, "parent")?); self.set(&format!("thread.parent.{id}"), p); }
            "ThreadForked" => { let (id, src) = (s(e, "id")?, s(e, "source")?); self.set(&format!("fork.{id}"), src); }
            "ThreadSnoozed" => { let (id, until) = (s(e, "id")?, n(e, "until")?); self.snooze_until.insert(id.into(), until); }
            "ThreadOrdered" => {
                let (id, section, key) = (s(e, "id")?, s(e, "section")?, s(e, "key")?); n(e, "at")?;
                let k = self.order_keys.entry(id.into()).or_default(); if section == "pinned" { k.0 = key.into(); } else { k.1 = key.into(); }
            }
            "ProjectOrdered" => { let (id, key) = (s(e, "id")?, s(e, "key")?); n(e, "at")?; self.order_keys.entry(format!("project:{id}")).or_default().1 = key.into(); }
            "RunSet" => { let (th, bot) = (s(e, "thread")?, s(e, "bot")?); s(e, "conv")?; s(e, "mail")?; n(e, "at")?; self.runs.insert(th.into(), bot.into()); }
            "UberSet" => { let (bot, uid) = (s(e, "bot")?, s(e, "uid")?); self.set("uber.bot", bot); self.set("uber.uid", uid); }
            "BotSet" => { let (id, th) = (s(e, "id")?, s(e, "thread")?); // Bot.keep.k: an edit with an empty thread keeps the one the bot has
                if !th.is_empty() || !self.bot_threads.contains_key(id) { self.bot_threads.insert(id.into(), th.into()); }
                let old = self.bots_nav.get(id);
                self.bots_nav.insert(id.into(), BotNav { name: s(e,"name")?.into(), thread: self.bot_threads[id].clone(), look:n(e,"look")?,
                    created:old.map_or(n(e,"at")?,|b|b.created), talked:old.map_or(0,|b|b.talked) }); }
            "BotDeleted" => { let id = s(e, "id")?; self.bot_threads.remove(id); self.bots_nav.remove(id); }
            "RoomSet" => { self.rooms_nav.insert(s(e,"id")?.into(), RoomNav { name:s(e,"name")?.into(), members:s(e,"members")?.split('\n').filter(|s|!s.is_empty()).count() }); }
            "RoomDeleted" => { self.rooms_nav.remove(s(e,"id")?); }
            "RoomPosted" => { if let Some(bot)=self.bots_nav.get_mut(s(e,"from")?) { bot.talked=n(e,"at")?; } }
            "TaskDelegated" => {
                let (task, parent, child) = (s(e, "task")?, s(e, "parent")?, s(e, "child")?); n(e, "depth")?;
                let before = fam::ext::task_before(self, task);
                self.tasks.insert(task.into(), (parent.into(), child.into(), false, TaskSt::Queued));
                fam::ext::task_index(self, task, before);
                // State.delegated -> State.task_parent: seed thread.parent.<child> once, unless a bot's thread delegated
                let (parent, child) = (s(e, "parent")?, s(e, "child")?);
                if !self.bot_threads.values().any(|t| t == parent) {
                    let k = format!("thread.parent.{child}");
                    if !self.settings.contains_key(&k) { self.set(&k, parent); }
                }
            }
            "TaskStateSet" => { let (task, want) = (s(e, "task")?, TaskSt::parse(s(e, "state")?).unwrap_or(TaskSt::Cancelled)); s(e, "summary")?;
                let before = fam::ext::task_before(self, task);
                let t = self.tasks.entry(task.into()).or_insert((String::new(), String::new(), true, TaskSt::Cancelled)); t.3 = TaskSt::next(t.3, want); t.2 = t.3.terminal();
                fam::ext::task_index(self, task, before); }
            "AskOpened" => { let (id, th) = (s(e, "id")?, s(e, "thread")?); self.asks.insert(id.into(), (th.into(), true));
                self.ask_data.retain(|a| a.0 != id); self.ask_data.push((id.into(), th.into(), s(e, "kind")?.into(), s(e, "tool")?.into(), s(e, "detail")?.into(), s(e, "options")?.into())); }
            "AskClosed" => { let id = s(e, "id")?; self.ask_data.retain(|a| a.0 != id); if self.lean { self.asks.remove(id); } else if let Some(a) = self.asks.get_mut(id) { a.1 = false; } else { self.asks.insert(id.into(), (String::new(), false)); } }
            "TurnQueued" => { let (th, msg, text) = (s(e, "thread")?, s(e, "msg")?, s(e, "text")?); self.queue.entry(th.into()).or_default().push((msg.into(), text.into())); }
            "QueueTaken" => { let th = s(e, "thread")?; self.queue.insert(th.into(), vec![]); }
            "QueueEdited" => { let (th, msg, text) = (s(e, "thread")?, s(e, "msg")?, s(e, "text")?); for q in self.queue.entry(th.into()).or_default() { if q.0 == msg { q.1 = text.into(); } } }
            "QueueRemoved" | "QueueTakenOne" => { let (th, msg) = (s(e, "thread")?, s(e, "msg")?); self.queue.entry(th.into()).or_default().retain(|q| q.0 != msg); }
            "QueueMoved" => {
                let (th, msg, to) = (s(e, "thread")?, s(e, "msg")?, n(e, "to")? as usize);
                let q = self.queue.entry(th.into()).or_default();
                if let Some(item) = q.iter().find(|x| x.0 == msg).cloned() { q.retain(|x| x.0 != msg); let at = to.min(q.len()); q.insert(at, item); }
            }
            "WorkSet" => { let (th, kind, rows) = (s(e, "thread")?, s(e, "kind")?, s(e, "rows")?); self.set(&format!("{kind}.{th}"), rows); }
            "ThreadLaunched" => {
                // Launch.fold: launch.<id> = report flag + launcher; launched.<by> lists the ids, oldest first, once each
                let (id, by) = (s(e, "id")?, s(e, "by")?); let report = fam::json::gbool(e, "report", true);
                self.set(&format!("launch.{id}"), &format!("{}{by}", if report { "1" } else { "0" }));
                let lk = format!("launched.{by}"); let old = self.get(&lk, "");
                if !old.split('\n').any(|l| l == id) { let v = if old.is_empty() { id.to_string() } else { format!("{old}\n{id}") }; self.set(&lk, &v); }
            }
            "TodosSet" => { let (th, todos) = (s(e, "thread")?, s(e, "todos")?); self.todos_put(th, todos, todos, "1"); }
            "TodosChanged" => {
                let (th, who, data) = (s(e, "thread")?, s(e, "who")?, s(e, "data")?);
                if who == "snapshot" {
                    let p: Value = serde_json::from_str(data).unwrap_or(Value::Null);
                    let g = |k: &str, d: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or(d).to_string();
                    let (c, b, v) = (g("current", ""), g("base", ""), g("visible", "0"));
                    self.todos_put(th, &c, &b, &v);
                } else { return fam::fold(self, kind, e); }
            }
            "UsageSet" => {
                // State.usage: a new reading over the old one; 0 keeps the old part
                let (th, used, window) = (s(e, "thread")?, n(e, "used")?, n(e, "window")?);
                let key = format!("usage.{th}");
                let old = self.get(&key, "");
                let mut parts = old.split('/').map(|x| x.parse::<u64>().unwrap_or(0));
                let (ou, ow) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
                let keep = |new: u64, old: u64| if new == 0 { old } else { new };
                self.set(&key, &format!("{}/{}", keep(used, ou), keep(window, ow)));
            }
            "ProviderSet" => { self.switch(s(e, "id")?, s(e, "provider")?, s(e, "model")?); }
            _ => return fam::fold(self, kind, e),
        }
        Some(())
    }

    /// State.post -> Idx.post: an entry is filed under its thread, and a message another thread sent ("t:<from>:<n>") under the sender too.
    fn post(&mut self, th: &str, id: &str) {
        if !self.lean { self.entries.entry(th.into()).or_default().push(id.into()); }
        *self.own.entry(th.into()).or_default() += 1;
        let from = link_from(id);
        if !from.is_empty() && from != th && !from.is_empty() { self.sent_link(from, id, th); }
    }
    fn sent_link(&mut self, from: &str, id: &str, _th: &str) { if !self.lean { self.entries.entry(from.into()).or_default().push(id.into()); } }

    fn todos_put(&mut self, th: &str, current: &str, base: &str, visible: &str) {
        self.set(&format!("todos.{th}"), current); self.set(&format!("todos.base.{th}"), base); self.set(&format!("todos.visible.{th}"), visible);
    }
    /// Todos.of: statuses of the thread's list (Todo.status.of: completed and in_progress stay, anything else is pending)
    fn todos_status(&self, th: &str) -> Vec<&'static str> {
        let text = self.get(&format!("todos.{th}"), "");
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Array(xs)) => xs.iter().map(|x| match x.get("status").and_then(|v| v.as_str()).unwrap_or("") { "completed" => "completed", "in_progress" => "in_progress", _ => "pending" }).collect(),
            _ => vec![],
        }
    }
    /// Todos.turn: a turn starting hides a list nobody has touched; a turn ending clears a completed or never-shown one.
    fn todos_turn(&mut self, st: Turn, th: &str, at: u64) {
        if st == Turn::Running {
            let was = self.threads.get(th).map_or(false, |t| t.turn == Turn::Running);
            if !was { self.set(&format!("todos.visible.{th}"), "0"); }
        } else {
            let list = self.todos_status(th);
            let complete = !list.is_empty() && list.iter().all(|x| *x == "completed");
            let visible = self.get(&format!("todos.visible.{th}"), "1") != "0";
            if complete || !visible {
                let empty = list.is_empty();
                self.todos_put(th, "", "", "0");
                if !empty {
                    let gk = format!("todos.gen.{th}"); let old = self.get(&gk, "");
                    let next = if old == at.to_string() { format!("{old}+") } else { at.to_string() };
                    self.set(&gk, &next);
                }
            }
        }
    }

    /// State.switch: the old provider's session and seen-count are kept as settings, the new provider's session is taken.
    /// The seen-count is Switch.count: the thread's own entries.
    fn switch(&mut self, th: &str, p: &str, m: &str) {
        let (old, oldse) = { let t = self.thread_mut(th); (t.provider.clone(), t.session.clone()) };
        let count = self.own.get(th).copied().unwrap_or(0);
        self.set(&format!("session.{th}.{old}"), &oldse);
        self.set(&format!("seen.{th}.{old}"), &count.to_string());
        let se = self.get(&format!("session.{th}.{p}"), "");
        let t = self.thread_mut(th); t.provider = p.into(); t.model = m.into(); t.session = se;
    }

    /// The canonical state JSON (DIGEST.md): entries newest first, as State.mine lists them.
    pub fn digest_json(&self) -> Value {
        use serde_json::json;
        let mut projects: Vec<Value> = self.projects.iter().map(|(id, p)| json!({"id": id, "title": p.title, "root": p.root, "removed": p.removed})).collect();
        projects.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        let mut threads: Vec<Value> = self.threads.iter().map(|(id, t)| {
            let ids: Vec<&String> = self.entries.get(id).map(|v| v.iter().rev().collect()).unwrap_or_default();
            json!({"id": id, "project": t.project, "title": t.title, "turn": t.turn.word(), "pending": t.pending, "archived": t.archived, "pinned": t.pinned,
                   "deleted": t.deleted, "provider": t.provider, "model": t.model, "session": t.session, "entries": ids})
        }).collect();
        threads.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        let mut settings: Vec<Value> = self.settings.iter().map(|(k, v)| json!({"key": k, "value": v})).collect();
        settings.sort_by(|a, b| a["key"].as_str().cmp(&b["key"].as_str()));
        json!({"projects": projects, "threads": threads, "settings": settings, "seq": self.seq})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fold(lines: &[&str]) -> State { let mut s = State::new(); for l in lines { s.apply_line(l); } s }
    const PC: &str = r#"{"$":"ProjectCreated","id":"p1","title":"proj","root":"/r","at":10}"#;
    const TC: &str = r#"{"$":"ThreadCreated","id":"t1","project":"p1","title":"New thread","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":11}"#;

    #[test] fn seq_counts_known_events_only() {
        let s = fold(&[PC, TC, r#"{"$":"Nope"}"#, "not json", r#"{"$":"SettingSet","key":"a","value":"b","at":1}"#]);
        assert_eq!(s.seq, 3);
    }
    #[test] fn an_event_for_an_unknown_thread_makes_a_hidden_blank() {
        let s = fold(&[r#"{"$":"ThreadRenamed","id":"ghost","title":"x","at":1}"#]);
        let t = &s.threads["ghost"]; assert!(t.archived && t.deleted && t.title == "x");
    }
    #[test] fn a_user_message_touches_the_thread_an_assistant_one_does_not() {
        let s = fold(&[PC, TC, r#"{"$":"MessagePosted","thread":"t1","msg":"m1","role":"assistant","text":"a","at":50}"#]);
        assert_eq!(s.threads["t1"].activity, 11);
        let s = fold(&[PC, TC, r#"{"$":"MessagePosted","thread":"t1","msg":"m1","role":"user","text":"a","at":50}"#]);
        assert_eq!((s.threads["t1"].activity, s.threads["t1"].unsettled_at), (50, 50));
    }
    #[test] fn entries_come_out_newest_first() {
        let s = fold(&[PC, TC, r#"{"$":"MessagePosted","thread":"t1","msg":"m1","role":"user","text":"a","at":50}"#, r#"{"$":"ActivityLogged","thread":"t1","act":"g1","tone":"tool","kind":"Bash","summary":"x","at":51}"#]);
        assert_eq!(s.digest_json()["threads"][0]["entries"], serde_json::json!(["g1", "m1"]));
    }
    #[test] fn turn_changed_keeps_the_later_activity_and_hides_the_todos_on_start() {
        let s = fold(&[PC, TC, r#"{"$":"TurnChanged","thread":"t1","state":"running","at":5}"#]);
        assert_eq!((s.threads["t1"].activity, s.threads["t1"].turn), (11, Turn::Running));
        assert_eq!(s.settings["todos.visible.t1"], "0");
    }
    #[test] fn settling_clears_the_pin_and_forces_only_when_manual() {
        let s = fold(&[PC, TC, r#"{"$":"ThreadPinned","id":"t1","pinned":true,"at":1}"#, r#"{"$":"ThreadSettled","id":"t1","manual":false,"at":60}"#]);
        assert!(!s.threads["t1"].pinned); assert_eq!(s.threads["t1"].override_, Override::None);
        let s = fold(&[PC, TC, r#"{"$":"ThreadSettled","id":"t1","manual":true,"at":60}"#]); assert_eq!(s.threads["t1"].override_, Override::ForcedSettled);
    }
    #[test] fn usage_keeps_the_old_part_when_the_new_one_is_zero() {
        let s = fold(&[r#"{"$":"UsageSet","thread":"t1","used":100,"window":200,"at":1}"#, r#"{"$":"UsageSet","thread":"t1","used":0,"window":300,"at":2}"#, r#"{"$":"UsageSet","thread":"t1","used":7,"window":0,"at":3}"#]);
        assert_eq!(s.settings["usage.t1"], "7/300");
    }
    #[test] fn a_link_message_is_filed_under_its_sender_too() {
        let s = fold(&[PC, TC, r#"{"$":"MessagePosted","thread":"t1","msg":"t:tOther:7","role":"user","text":"x","at":5}"#]);
        assert_eq!(s.entries["tOther"], vec!["t:tOther:7".to_string()]); assert_eq!(s.entries["t1"].len(), 1);
        assert_eq!(link_from("t:a:b"), "a"); assert_eq!(link_from("t:a"), ""); assert_eq!(link_from("m:a:b"), ""); assert_eq!(link_from("ta:b:c"), "");
    }
    #[test] fn a_completed_list_clears_at_the_end_of_the_turn_with_a_generation() {
        let done = r#"[{"id":"1","text":"a","form":"","status":"completed"}]"#;
        let ev = format!(r#"{{"$":"TodosSet","thread":"t1","todos":{},"at":1}}"#, serde_json::to_string(done).unwrap());
        let s = fold(&[PC, TC, &ev, r#"{"$":"TurnChanged","thread":"t1","state":"completed","at":9}"#]);
        assert_eq!((s.settings["todos.t1"].as_str(), s.settings["todos.visible.t1"].as_str(), s.settings["todos.gen.t1"].as_str()), ("", "0", "9"));
    }
    #[test] fn launches_list_each_thread_once_and_delegation_seeds_the_parent_once() {
        let s = fold(&[r#"{"$":"ThreadLaunched","id":"a","by":"b","report":true,"at":1}"#, r#"{"$":"ThreadLaunched","id":"c","by":"b","report":false,"at":2}"#, r#"{"$":"ThreadLaunched","id":"a","by":"b","report":true,"at":3}"#]);
        assert_eq!(s.settings["launched.b"], "a\nc"); assert_eq!(s.settings["launch.c"], "0b");
    }
    #[test] fn an_empty_ledger_setting_removes_the_key_others_keep_the_empty_value() {
        let s = fold(&[r#"{"$":"SettingSet","key":"told.k1","value":"x","at":1}"#, r#"{"$":"SettingSet","key":"told.k1","value":"","at":2}"#, r#"{"$":"SettingSet","key":"other","value":"","at":3}"#]);
        assert!(!s.settings.contains_key("told.k1")); assert_eq!(s.settings["other"], "");
    }
    #[test] fn a_mistyped_or_missing_field_still_counts_and_takes_default() {
        // one line of each kind the decoder is lenient about: seq and the folded value, as Change.decode reads them
        let s = fold(&[PC, TC, r#"{"$":"TurnChanged","thread":"t1","state":"bogus","at":5}"#]);
        assert_eq!((s.seq, s.threads["t1"].turn), (3, Turn::Idle));
        let s = fold(&[PC, TC, r#"{"$":"ThreadSettled","id":"t1","at":60}"#]);
        assert_eq!((s.seq, s.threads["t1"].override_, s.threads["t1"].settled_at), (3, Override::None, 60));
        let s = fold(&[PC, TC, r#"{"$":"ThreadArchived","id":"t1","on":true,"at":1}"#, r#"{"$":"ThreadPinned","id":"t1","on":true,"at":2}"#]);
        assert_eq!((s.seq, s.threads["t1"].archived, s.threads["t1"].pinned), (4, false, false));
        let s = fold(&[r#"{"$":"TaskStateSet","task":"k","state":"bogus","summary":"","at":1}"#]);
        assert_eq!((s.seq, s.tasks["k"].3), (1, TaskSt::Cancelled));
        let s = fold(&[r#"{"$":"ThreadLaunched","id":"a","by":"b","at":1}"#]);
        assert_eq!((s.seq, s.settings["launch.a"].as_str()), (1, "1b"));
        let s = fold(&[PC, TC, r#"{"$":"ThreadSettled","id":"t1","manual":true,"at":"60"}"#]);
        assert_eq!((s.seq, s.threads["t1"].settled_at), (3, 60));
    }
    #[test] fn a_bot_edit_with_an_empty_thread_keeps_its_thread() {
        // Bot.keep.k: bots.edit logs thread "" and must not unhook the bot from its thread
        let s = fold(&[r#"{"$":"BotSet","id":"b1","name":"Bub","thread":"tb","look":0,"persona":"","at":1}"#, r#"{"$":"BotSet","id":"b1","name":"Bub2","thread":"","look":1,"persona":"x","at":2}"#]);
        assert_eq!(s.bot_threads["b1"], "tb");
        let s = fold(&[r#"{"$":"BotSet","id":"b1","name":"Bub","thread":"tb","look":0,"persona":"","at":1}"#, r#"{"$":"BotSet","id":"b1","name":"Bub","thread":"tc","look":0,"persona":"","at":2}"#]);
        assert_eq!(s.bot_threads["b1"], "tc", "a non-empty thread replaces it");
        assert_eq!(fold(&[r#"{"$":"BotSet","id":"b2","name":"N","thread":"","look":0,"persona":"","at":1}"#]).bot_threads["b2"], "", "a new bot with no thread has none");
    }
}

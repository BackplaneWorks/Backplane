//! The read model's fold in Rust: projects, threads, entry order, settings and the sequence number,
//! equal to Backplane's `M.State.replay` (src/core/model.bend) for the events it covers.
//! Its twin is checked by the differential oracle (test/gpui/parity/oracle): both fold the same
//! events.jsonl and must print the same canonical digest (test/gpui/parity/oracle/DIGEST.md).
//!
//! Covered: every event the Bend decoder knows counts toward `seq` (State.tick); these change state:
//! ProjectCreated/Removed, ThreadCreated/Renamed/Settled/Unsettled/Woke/Archived/Pinned/Deleted, MessagePosted,
//! ActivityLogged, TurnChanged (+ the todos bookkeeping it does to settings), PendingChanged, SessionSet,
//! WorktreeSet/Skipped/Removed, ThreadMerged, SettingSet, ThreadParentSet, ThreadForked, ProviderSet (approximate:
//! see `switch`), ModesSet. Not folded here (they touch parts of the Bend State the digest does not print): bots,
//! mail, tasks, asks, devices, nav, queues, usage, deploy, routines, todos lists.
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Turn { Idle, Running, Completed, Interrupted, Failed }
impl Turn {
    pub fn word(self) -> &'static str { match self { Turn::Idle => "idle", Turn::Running => "running", Turn::Completed => "completed", Turn::Interrupted => "interrupted", Turn::Failed => "failed" } }
    fn parse(s: &str) -> Option<Turn> { Some(match s { "idle" => Turn::Idle, "running" => Turn::Running, "completed" => Turn::Completed, "interrupted" => Turn::Interrupted, "failed" => Turn::Failed, _ => return None }) }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Override { None, ForcedSettled, ForcedActive }

#[derive(Clone, Debug)]
pub struct Project { pub title: String, pub root: String, pub created: u64, pub removed: bool }
#[derive(Clone, Debug)]
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

#[derive(Default, Debug)]
pub struct State {
    pub projects: HashMap<String, Project>,
    pub threads: HashMap<String, Thread>,
    /// entry ids per thread, oldest first (State.mine is newest first)
    pub entries: HashMap<String, Vec<String>>,
    pub settings: HashMap<String, String>,
    pub seq: u64,
}

/// every event name the Bend decoder (core/codec.bend Change.decode) knows
const KNOWN: &[&str] = &["ProjectCreated", "ProjectRemoved", "ThreadCreated", "ThreadRenamed", "MessagePosted", "TurnChanged", "PendingChanged", "ActivityLogged",
    "ThreadSettled", "ThreadUnsettled", "ThreadArchived", "ThreadPinned", "ThreadDeleted", "WorktreeSet", "SessionSet", "ModesSet", "TaskDelegated", "TaskStateSet",
    "TaskDelivered", "SettingSet", "TurnQueued", "QueueTaken", "AskOpened", "AskClosed", "DeviceRegistered", "DeviceDropped", "ThreadSnoozed", "ThreadOrdered",
    "WorktreeSkipped", "ThreadMerged", "WorktreeRemoved", "ProjectOrdered", "QueueEdited", "QueueRemoved", "QueueMoved", "QueueTakenOne", "ThreadForked", "ProviderSet",
    "TodosSet", "UsageSet", "BotSet", "BotDeleted", "RoomSet", "RoomDeleted", "RoomPosted", "HookSet", "HookHit", "RoutineSet", "RoutineDeleted", "RoutineFired",
    "MemorySaved", "MemoryForgot", "SpaceSet", "PeerSet", "BotWoke", "SpacePageSet", "WorkSet", "UberSet", "UberOp", "ThreadLaunched", "WakeSet", "WakeDropped",
    "LockChanged", "RoutinePlan", "RoutineSlot", "RoutineDone", "Reminded", "TaskSet", "TaskDeleted", "MessageFrom", "ConvSet", "MailPosted", "MailMarked", "MailTagged",
    "RunSet", "CharterSet", "DecisionLogged", "TurnUsage", "DeployAsked", "DeployBuilt", "ThreadParentSet", "ThreadWoke", "TodosChanged"];

fn s<'a>(e: &'a Value, k: &str) -> Option<&'a str> { e.get(k)?.as_str() }
fn n(e: &Value, k: &str) -> Option<u64> { e.get(k)?.as_u64() }

/// Setting.blank_gone: an empty value removes these keys
fn blank_gone(k: &str) -> bool {
    ["told.", "rule.", "board.", "launch.input.", "fallback.input.", "voice.e.", "voice.l."].iter().any(|p| k.starts_with(p))
}

impl State {
    pub fn new() -> State { State::default() }

    fn thread_mut(&mut self, id: &str) -> &mut Thread { self.threads.entry(id.to_string()).or_insert_with(Thread::blank) }
    fn set(&mut self, k: &str, v: &str) {
        if v.is_empty() && blank_gone(k) { self.settings.remove(k); } else { self.settings.insert(k.to_string(), v.to_string()); }
    }
    fn get(&self, k: &str, d: &str) -> String { self.settings.get(k).cloned().unwrap_or_else(|| d.to_string()) }

    /// One event line. Returns false when the Bend decoder would refuse it (it then counts nowhere).
    pub fn apply_line(&mut self, line: &str) -> bool {
        match serde_json::from_str::<Value>(line) { Ok(v) => self.apply(&v), Err(_) => false }
    }

    pub fn apply(&mut self, e: &Value) -> bool {
        let Some(kind) = s(e, "$") else { return false };
        if !KNOWN.contains(&kind) { return false }
        // the fields the Bend decoder insists on, for the events the model reads
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
                let th = s(e, "thread")?; let id = s(e, "msg")?; let role = s(e, "role")?; let at = n(e, "at")?; s(e, "text")?;
                if role == "user" { let t = self.thread_mut(th); t.activity = at; t.unsettled_at = at; }
                self.entries.entry(th.into()).or_default().push(id.into());
            }
            "TurnChanged" => {
                let th = s(e, "thread")?; let st = Turn::parse(s(e, "state")?)?; let at = n(e, "at")?;
                self.todos_turn(st, th, at);
                let t = self.thread_mut(th); t.activity = t.activity.max(at); t.turn = st;
            }
            "PendingChanged" => { let th = s(e, "thread")?; let c = n(e, "count")?; self.thread_mut(th).pending = c; }
            "ActivityLogged" => { let th = s(e, "thread")?; let id = s(e, "act")?; self.entries.entry(th.into()).or_default().push(id.into()); }
            "ThreadSettled" => {
                let id = s(e, "id")?; let manual = e.get("manual")?.as_bool()?; let at = n(e, "at")?;
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
            "ThreadArchived" => { let id = s(e, "id")?; let b = e.get("archived").or_else(|| e.get("on")).and_then(|v| v.as_bool())?; self.thread_mut(id).archived = b; }
            "ThreadPinned" => { let id = s(e, "id")?; let b = e.get("pinned").or_else(|| e.get("on")).and_then(|v| v.as_bool())?; self.thread_mut(id).pinned = b; }
            "ThreadDeleted" => { let id = s(e, "id")?; self.thread_mut(id).deleted = true; }
            "WorktreeSet" => { let id = s(e, "id")?; let (b, w) = (s(e, "branch")?.to_string(), s(e, "worktree")?.to_string()); let t = self.thread_mut(id); t.worktree_mode = true; t.branch = b; t.worktree = w; }
            "WorktreeSkipped" => { let id = s(e, "id")?; let t = self.thread_mut(id); t.worktree_mode = false; t.branch.clear(); t.worktree.clear(); }
            "WorktreeRemoved" => { let id = s(e, "id")?; self.thread_mut(id).worktree.clear(); }
            "SessionSet" => { let id = s(e, "id")?; let se = s(e, "session")?.to_string(); self.thread_mut(id).session = se; }
            "ModesSet" => { let id = s(e, "id")?; self.thread_mut(id); }
            "ThreadMerged" => {
                let id = s(e, "id")?; let url = s(e, "url")?.to_string(); let at = n(e, "at")?;
                let t = self.thread_mut(id); t.override_ = Override::ForcedSettled; t.settled_at = at; t.pinned = false;
                self.set(&format!("merged.{id}"), &url);
            }
            "SettingSet" => { let (k, v) = (s(e, "key")?, s(e, "value")?); self.set(k, v); }
            "ThreadParentSet" => { let (id, p) = (s(e, "id")?, s(e, "parent")?); self.set(&format!("thread.parent.{id}"), p); }
            "ThreadForked" => { let (id, src) = (s(e, "id")?, s(e, "source")?); self.set(&format!("fork.{id}"), src); }
            "ProviderSet" => { self.switch(s(e, "id")?, s(e, "provider")?, s(e, "model")?); }
            _ => {}
        }
        Some(())
    }

    /// Todos.turn: a turn starting hides a list nobody has touched; a turn ending clears a completed or never-shown one.
    /// The lists themselves are not modelled: an empty list is the only case this covers exactly (see PORT.md).
    fn todos_turn(&mut self, st: Turn, th: &str, _at: u64) {
        if st == Turn::Running {
            let was = self.threads.get(th).map_or(false, |t| t.turn == Turn::Running);
            if !was { self.set(&format!("todos.visible.{th}"), "0"); }
        } else {
            let visible = self.get(&format!("todos.visible.{th}"), "1") != "0";
            if !visible {
                self.set(&format!("todos.{th}"), ""); self.set(&format!("todos.base.{th}"), ""); self.set(&format!("todos.visible.{th}"), "0");
            }
        }
    }

    /// State.switch: the old provider's session and seen-count are kept as settings, the new provider's session is taken.
    /// Approximation: the seen-count is the thread's entry count (Switch.count in model.bend reads more).
    fn switch(&mut self, th: &str, p: &str, m: &str) {
        let (old, oldse) = { let t = self.thread_mut(th); (t.provider.clone(), t.session.clone()) };
        let count = self.entries.get(th).map_or(0, |v| v.len());
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
    #[test] fn an_empty_ledger_setting_removes_the_key_others_keep_the_empty_value() {
        let s = fold(&[r#"{"$":"SettingSet","key":"told.k1","value":"x","at":1}"#, r#"{"$":"SettingSet","key":"told.k1","value":"","at":2}"#, r#"{"$":"SettingSet","key":"other","value":"","at":3}"#]);
        assert!(!s.settings.contains_key("told.k1")); assert_eq!(s.settings["other"], "");
    }
}

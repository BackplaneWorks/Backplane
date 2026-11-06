//! The slice of the read model the agent rules look at, folded from the log by this module alone
//! (threads' provider/model/modes/session/turn/worktree, the settings these rules read). Kept by catching up
//! from the store: `sync` reads the lines it has not seen. Bounded by threads and the few setting keys, not by the log.
use super::lines::{num, Cx};
use crate::store::Store;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct T {
    pub act: u64, pub project: String, pub provider: String, pub model: String, pub session: String, pub worktree: String,
    pub runtime: String, pub interaction: String, pub turn: String, pub archived: bool, pub deleted: bool,
    pub title: String, pub last_user: String, pub newest: String, pub carry_text_last: bool,
    pub env: String, pub branch: String, pub last_assistant: String,
    /// when the running turn began (the turn that is running now): a restart carries on the oldest first. 0 from a checkpoint older than this field: they go by id
    #[serde(default)] pub running_at: u64,
}

/// A delegated task: its state is one of queued, running, done, failed, cancelled; a terminal state sticks (TaskState.next).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Tk { pub parent: String, pub child: String, pub depth: u64, pub state: String, pub summary: String, pub delivered: bool, pub provider: String, pub model: String }
impl Tk { pub fn terminal(&self) -> bool { matches!(self.state.as_str(), "done" | "failed" | "cancelled") } }

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Ask { pub thread: String, pub kind: String, pub tool: String, pub detail: String, pub options: String }

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct World { pub home: String, pub bots: HashMap<String, (String, String, bool)>, pub rooms: HashMap<String, (String, bool)>, pub bot_of: HashMap<String, String>, pub said_to: HashMap<String, std::collections::HashSet<String>>, pub reviews: HashMap<String, Vec<(String, String, u64)>>, pub queued: HashMap<String, Vec<String>>, pub parents: HashMap<String, String>, pub td_msgs: std::collections::HashSet<String>, pub launched: HashMap<String, (String, bool)>, pub projects: HashMap<String, (String, bool)>, pub tasks: HashMap<String, Tk>, pub deadlines: std::collections::HashSet<String>, pub asks: HashMap<String, Ask>, pub sessions: std::collections::HashSet<(String, String)>, pub threads: HashMap<String, T>, pub roots: HashMap<String, String>, pub st: HashMap<String, String>, seen: usize }

const KEYS: &[&str] = &["carry.wait.", "summary.", "rule.", "watch.", "thread.parent.", "bg.", "wait.", "work.timeout.seconds", "allowance.claude", "effort.", "buy.", "kicad.ipc", "order.enabled", "agent.lang", "tz.machine", "agent.thinking", "restart.continue", "tok.epoch", "tok.recount", "tok.basis.", "turn.speed.", "turn.models.", "limit.claude.", "ride.", "agy.", "session.", "launch.n."];
fn tracked(k: &str) -> bool { KEYS.iter().any(|p| k == *p || (p.ends_with('.') && k.starts_with(p))) || k.starts_with("usage.") || k.starts_with("tok.key.") }

pub const CARRY_TEXT: &str = "Backplane restarted while you were working. Continue where you left off.";

/// The providers a message can pick up where a cut turn stopped: each resumes its own session by id (a process per turn), Claude with `--resume`.
pub const CARRIES: [&str; 8] = ["claude", "codex", "grok", "muse", "antigravity", "opencode", "pi", "omp"];
/// A thread that may go on: the setting is on, it is live, has made progress since the last carry, and its agent can resume. One of the process-per-turn providers
/// needs a session to resume: with none the turn was cut before it began, and the note "continue where you left off" would reach an agent that knows nothing of it.
pub fn carry_may(t: &T, restart_continue: &str) -> bool {
    restart_continue != "off" && !t.archived && !t.deleted && !t.carry_text_last && CARRIES.contains(&t.provider.as_str()) && (t.provider == "claude" || !t.session.is_empty())
}
/// Carry.will: a turn the restart cut short carries on when it was running and may go on.
pub fn carry_will(t: &T, restart_continue: &str) -> bool { t.turn == "running" && carry_may(t, restart_continue) }

impl World {
    pub fn seen(&self) -> usize { self.seen }
    /// The bot a thread belongs to ("" for a thread that is no bot's).
    pub fn bot(&self, th: &str) -> &str { self.bot_of.get(th).map(String::as_str).unwrap_or("") }
    pub fn setting(&self, k: &str) -> &str { self.st.get(k).map(String::as_str).unwrap_or("") }

    /// Catch up with the store (whole lines past what was seen). A cheap substring test skips lines no rule reads.
    fn ckpt_path(store: &Store) -> std::path::PathBuf { crate::ckpt::dir(store.path()).join("agents.json") }
    /// The saved world (ckpt/agents.json) when it is of this log; `sync` folds what is past it.
    pub fn load(store: &Store) -> Option<World> { let w = crate::ckpt::load_json(&Self::ckpt_path(store), store, |w: &World| w.seen); if let Some(w) = &w { crate::ckpt::saved("agents", w.seen);
        if crate::ckpt::check_on() { let (s, n, a) = (store.clone(), w.seen, serde_json::to_value(w).unwrap_or_default()); std::thread::spawn(move || { let mut f = World::default(); let _ = f.sync_to(&s, n); crate::ckpt::check("agents world", a, serde_json::to_value(&f).unwrap_or_default()) }); } } w }
    /// Save what is folded (the log lines `seen` covers, which the caller's `sync` has just brought up to date).
    pub fn save(&self, store: &Store) { if crate::ckpt::due("agents", self.seen) { crate::ckpt::save_json(&Self::ckpt_path(store), store, self.seen, self); crate::ckpt::saved("agents", self.seen) } }
    pub fn sync(&mut self, store: &Store) -> Result<()> { self.sync_to(store, store.len()) }
    pub fn sync_to(&mut self, store: &Store, total: usize) -> Result<()> {
        while self.seen < total {
            let b = (self.seen + 4096).min(total);
            for l in store.range(self.seen, b)? { self.apply(&l) }
            self.seen = b;
        }
        Ok(())
    }

    pub fn apply(&mut self, line: &str) {
        let Some(t) = crate::session::event_type(line) else { return };
        const READ: [&str; 36] = ["BotSet", "BotDeleted", "RoomSet", "RoomDeleted", "TurnQueued", "QueueTaken", "QueueRemoved", "QueueTakenOne", "ThreadParentSet", "ProjectRemoved", "ThreadLaunched", "TaskDelegated", "TaskStateSet", "TaskDelivered", "WorkSet", "AskOpened", "AskClosed", "ThreadRenamed", "ProjectCreated", "ThreadCreated", "WorktreeSet", "ProviderSet", "ModesSet", "SessionSet", "TurnChanged", "MessagePosted", "ActivityLogged", "UsageSet", "SettingSet", "TurnUsage", "ThreadArchived", "ThreadDeleted", "ThreadForked", "QueueTaken", "TurnQueued", "ThreadWoke"];
        if !READ.contains(&t) { return }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        let g = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        if let Some(x) = self.threads.get_mut(&g("thread")) { x.act = x.act.max(v.get("at").and_then(Value::as_u64).unwrap_or(0)) } // Refs.t.act: the thread's newest event
        match t {
            "ProjectCreated" => { self.roots.insert(g("id"), g("root")); self.projects.insert(g("id"), (g("title"), false)); }
            "TurnQueued" => self.queued.entry(g("thread")).or_default().push(g("msg")),
            "QueueTaken" => { self.queued.remove(&g("thread")); }
            "QueueRemoved" | "QueueTakenOne" => if let Some(q) = self.queued.get_mut(&g("thread")) { let m = g("msg"); q.retain(|x| *x != m) },
            "ThreadParentSet" => { self.parents.insert(g("id"), g("parent")); }
            "ProjectRemoved" => if let Some(p) = self.projects.get_mut(&g("id")) { p.1 = true },
            "BotSet" => { let th = g("thread"); if !th.is_empty() { self.bot_of.insert(th.clone(), g("id")); } let old = self.bots.get(&g("id")).cloned(); let (name, thread, _) = old.unwrap_or_default(); self.bots.insert(g("id"), (if g("name").is_empty() { name } else { g("name") }, if th.is_empty() { thread } else { th }, false)); }
            "BotDeleted" => { let e = self.bots.entry(g("id")).or_default(); e.2 = true; }
            "RoomSet" => { let e = self.rooms.entry(g("id")).or_default(); if !g("name").is_empty() { e.0 = g("name") } e.1 = false; }
            "RoomDeleted" => { let e = self.rooms.entry(g("id")).or_default(); e.1 = true; }
            "ThreadLaunched" => { self.launched.insert(g("id"), (g("by"), v.get("report").and_then(Value::as_bool).unwrap_or(true))); }
            "TaskDelegated" => { self.tasks.insert(g("task"), Tk { parent: g("parent"), child: g("child"), depth: v.get("depth").and_then(Value::as_u64).unwrap_or(0), state: "queued".into(), provider: g("provider"), model: g("model"), ..Default::default() }); }
            "TaskStateSet" => if let Some(k) = self.tasks.get_mut(&g("task")) { if !k.terminal() { k.state = g("state") } k.summary = g("summary") }, // Task.step: a terminal state sticks; the summary is replaced
            "TaskDelivered" => if let Some(k) = self.tasks.get_mut(&g("task")) { k.delivered = true },
            "ThreadCreated" => { self.threads.insert(g("id"), T { act: v.get("at").and_then(Value::as_u64).unwrap_or(0), project: g("project"), title: g("title"), provider: g("provider"), model: g("model"), worktree: g("worktree"), env: g("env"), branch: g("branch"), runtime: "full-access".into(), interaction: "default".into(), turn: "idle".into(), ..Default::default() }); }
            "WorktreeSet" => if let Some(x) = self.threads.get_mut(&g("id")) { x.worktree = g("worktree") },
            "ProviderSet" => { // State.switch.n: the old provider's session is kept as `session.<th>.<old>` and the new provider's is taken
                let th = g("id");
                if let Some(x) = self.threads.get_mut(&th) {
                    self.st.insert(format!("session.{th}.{}", x.provider), x.session.clone());
                    x.session = self.st.get(&format!("session.{th}.{}", g("provider"))).cloned().unwrap_or_default();
                    x.provider = g("provider"); x.model = g("model");
                }
            }
            "ModesSet" => if let Some(x) = self.threads.get_mut(&g("id")) { x.runtime = g("runtime"); x.interaction = g("interaction"); x.model = g("model") },
            "SessionSet" => if let Some(x) = self.threads.get_mut(&g("id")) { x.session = g("session") },
            "ThreadRenamed" => if let Some(x) = self.threads.get_mut(&g("id")) { x.title = g("title") },
            "AskOpened" => { self.asks.insert(g("id"), Ask { thread: g("thread"), kind: g("kind"), tool: g("tool"), detail: g("detail"), options: g("options") }); }
            "AskClosed" => if let Some(a) = self.asks.remove(&g("id")) { if a.kind == "approval" && g("answer") == "session" { self.sessions.insert((a.thread, a.tool)); } },
            "ThreadArchived" => if let Some(x) = self.threads.get_mut(&g("id")) { x.archived = v.get("archived").and_then(Value::as_bool).unwrap_or(false) },
            "ThreadDeleted" => if let Some(x) = self.threads.get_mut(&g("id")) { x.deleted = true },
            "TurnChanged" => if let Some(x) = self.threads.get_mut(&g("thread")) { let st = g("state"); if st == "running" && x.turn != "running" { x.running_at = v.get("at").and_then(Value::as_u64).unwrap_or(0) } x.turn = st },
            "MessagePosted" => if let Some(x) = self.threads.get_mut(&g("thread")) {
                x.newest = g("msg");
                if v.get("role").and_then(Value::as_str) == Some("user") { // Lr.said: what a thread has written to others since its newest user message
                    let (th, from) = (g("thread"), g("msg").strip_prefix("t:").and_then(|r| r.split(':').next()).unwrap_or("").to_string());
                    self.said_to.remove(&th);
                    if !from.is_empty() && from != th && !g("msg").contains(":lr-") { self.said_to.entry(from).or_default().insert(th); }
                }
                if g("msg").contains(":td-") { self.td_msgs.insert(g("msg")); } // Task.holds: the task results a thread already holds
                if v.get("role").and_then(Value::as_str) == Some("assistant") { x.last_assistant = g("text") } // Task.last
                x.carry_text_last = v.get("role").and_then(Value::as_str) == Some("user") && g("text").starts_with(CARRY_TEXT);
                if v.get("role").and_then(Value::as_str) == Some("user") { x.last_user = g("msg") }
            },
            "ActivityLogged" => if let Some(x) = self.threads.get_mut(&g("thread")) {
                if g("tone") == "review" { self.reviews.entry(g("thread")).or_default().push((g("kind"), g("summary"), v.get("at").and_then(Value::as_u64).unwrap_or(0))) } // Idx.review: a thread's review ops, in order
                x.newest = g("act"); if g("kind") != "restart" { x.carry_text_last = false } },
            "UsageSet" => { let k = format!("usage.{}", g("thread")); let old = self.setting(&k).to_string(); let (u, w) = (v.get("used").and_then(Value::as_u64).unwrap_or(0), v.get("window").and_then(Value::as_u64).unwrap_or(0));
                let keep = |new: u64, old: u64| if new == 0 { old } else { new };
                let ps: Vec<u64> = old.split('/').map(num).collect(); self.st.insert(k, format!("{}/{}", keep(u, *ps.first().unwrap_or(&0)), keep(w, *ps.get(1).unwrap_or(&0)))); }
            "TurnUsage" => { self.st.insert(format!("tok.key.{}", g("thread")), g("key")); self.st.insert(format!("tok.basis.{}", g("thread")), g("basis")); }
            "WorkSet" => if g("kind") == "bg" || g("kind") == "watch" { self.st.insert(format!("{}.{}", g("kind"), g("thread")), g("rows")); },
            "SettingSet" => { let k = g("key"); if tracked(&k) { self.st.insert(k.clone(), g("value")); } if k.starts_with("work.deadline.") { if g("value").is_empty() { self.deadlines.remove(&k); } else { self.deadlines.insert(k); } } }
            _ => {}
        }
    }

    /// Where a thread's agent works: its worktree, else its project's folder (Hub.cwd).
    /// Task.depth: how deep a thread's delegation chain is (0 for a thread nobody delegated).
    pub fn depth(&self, th: &str) -> u64 { self.tasks.values().find(|k| k.child == th).map_or(0, |k| k.depth) }
    /// Mcp.project.find: a live project by id or name (any case).
    pub fn project_find(&self, name: &str) -> Option<String> {
        let n = name.trim().to_lowercase(); if n.is_empty() { return None }
        self.projects.iter().find(|(id, (title, removed))| !*removed && (id.to_lowercase() == n || title.to_lowercase() == n)).map(|(id, _)| id.clone())
    }
    /// Queue.waiting: a real message waits (handed-over context alone never starts a turn).
    pub fn queue_waiting(&self, th: &str) -> bool { self.queued.get(th).map_or(false, |q| q.iter().any(|m| !m.starts_with("handoff:"))) }
    /// Tree.raw: a thread's parent as saved (the setting thread.parent.<id> overrides; empty is detached).
    pub fn tree_raw(&self, id: &str) -> String { let k = format!("thread.parent.{id}"); if self.st.contains_key(&k) { self.st[&k].clone() } else { self.parents.get(id).cloned().unwrap_or_default() } }
    /// Tree.chain(63, raw(parent), parent): the launcher's ancestry ends within 63 steps and never returns to it.
    pub fn tree_chain_ok(&self, launcher: &str) -> bool {
        let mut cur = self.tree_raw(launcher);
        for _ in 0..63 { if cur.is_empty() { return true } if cur == launcher { return false } cur = self.tree_raw(&cur) }
        cur.is_empty()
    }
    /// Work.ready: the thread has no running work of its own (background or watched rows, tasks it delegated that still run).
    pub fn ready(&self, th: &str) -> bool { self.work(th).0 == 0 && !self.queue_waiting(th) && !self.tasks.values().any(|k| k.parent == th && !k.terminal()) }
    pub fn cwd(&self, th: &str) -> String {
        let Some(t) = self.threads.get(th) else { return String::new() };
        // a bot's own thread works in `<home>/bots/<bot>`, whatever a WorktreeSet once said (never a folder made elsewhere)
        if let Some(b) = self.bot_of.get(th) { if !self.home.is_empty() && crate::bots::rpc::safe_id(b) && self.bots.get(b).map_or(false, |x| x.1 == th) { let d = std::path::Path::new(&self.home).join("bots").join(b); let _ = std::fs::create_dir_all(&d); return d.to_string_lossy().into_owned() } }
        if !t.worktree.is_empty() { t.worktree.clone() } else { self.roots.get(&t.project).cloned().unwrap_or_default() }
    }
    /// Work.busy (without delegated tasks, which this module does not run): the thread's background and watched rows, ended ones included
    /// (their outcome is still owed), and what they are called.
    pub fn work(&self, th: &str) -> (usize, Vec<String>) {
        let rows: Vec<String> = super::bg::lines(self.setting(&format!("bg.{th}"))).into_iter().chain(super::bg::lines(self.setting(&format!("watch.{th}")))).collect();
        (rows.len(), rows.iter().map(|l| super::bg::work_title(l)).collect())
    }
    pub fn running(&self, th: &str) -> bool { self.threads.get(th).map_or(false, |t| t.turn == "running") }

    /// The thread's slice for `Cx`.
    pub fn cx(&self, th: &str) -> Cx {
        let u = self.setting(&format!("usage.{th}")); let ps: Vec<u64> = u.split('/').map(num).collect();
        let t = self.threads.get(th).cloned().unwrap_or_default();
        Cx { thread: th.into(), running: t.turn == "running", session: t.session, model: t.model, think: self.setting("agent.thinking") == "on",
            used: *ps.first().unwrap_or(&0), window: *ps.get(1).unwrap_or(&0),
            tok_basis: self.setting(&format!("tok.basis.{th}")).into(), tok_key: self.setting(&format!("tok.key.{th}")).into(), speed: self.setting(&format!("turn.speed.{th}")).into(),
            models: self.setting(&format!("turn.models.{th}")).into(), tok_epoch_set: !self.setting("tok.epoch").is_empty(), tok_recount_set: !self.setting("tok.recount").is_empty(),
            bg: super::bg::lines(self.setting(&format!("bg.{th}"))), bg_text: self.setting(&format!("bg.{th}")).to_string(),
            deadlines: self.deadlines.iter().filter(|k| k.starts_with(&format!("work.deadline.{th}."))).cloned().collect(), timeout_setting: self.setting("work.timeout.seconds").to_string(), allowance: self.setting("allowance.claude").to_string(), default_account: true,
            limits: self.st.iter().filter(|(k, _)| k.starts_with("limit.claude.")).map(|(k, v)| (k.clone(), v.clone())).collect(), seq: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn th(turn: &str) -> T { T { turn: turn.into(), provider: "claude".into(), ..Default::default() } }
    // the GPUI "send is dropped" report: a hand-written log (json.dumps puts a space after the colon) must fold like a compact one, as Model does
    #[test] fn a_spaced_log_line_folds_like_a_compact_one() {
        let mut w = World::default();
        for l in [r#"{"$": "ProjectCreated", "id": "p1", "title": "ecc", "root": "/r", "at": 1}"#, r#"{"$": "ThreadCreated", "id": "t1", "project": "p1", "title": "T", "env": "local", "branch": "", "worktree": "", "provider": "claude", "model": "", "at": 1}"#] { w.apply(l) }
        assert!(w.threads.contains_key("t1"), "turn.start for t1 would be answered unknown thread otherwise");
        assert_eq!(w.threads["t1"].provider, "claude");
    }
    #[test] fn a_provider_switch_keeps_each_providers_session_a_b_a() { // State.switch.n
        let mut w = World::default();
        for l in [r#"{"$":"ThreadCreated","id":"t","project":"p","title":"T","provider":"claude","model":"","at":1}"#, r#"{"$":"SessionSet","id":"t","session":"S-claude","at":2}"#,
                  r#"{"$":"ProviderSet","id":"t","provider":"codex","model":"m","at":3}"#] { w.apply(l) }
        assert_eq!(w.threads["t"].session, "", "a provider never seen has no session: the new one starts fresh");
        w.apply(r#"{"$":"SessionSet","id":"t","session":"S-codex","at":4}"#);
        w.apply(r#"{"$":"ProviderSet","id":"t","provider":"claude","model":"","at":5}"#);
        assert_eq!((w.threads["t"].provider.as_str(), w.threads["t"].session.as_str()), ("claude", "S-claude"), "coming back resumes the first session");
        w.apply(r#"{"$":"ProviderSet","id":"t","provider":"codex","model":"m","at":6}"#);
        assert_eq!(w.threads["t"].session, "S-codex"); assert_eq!(w.setting("session.t.claude"), "S-claude");
    }
    #[test] fn carry_continues() { assert!(carry_will(&th("running"), "")); assert!(carry_will(&th("running"), "on")); }
    #[test] fn carry_leaves_idle() { for s in ["completed", "idle", "failed", "interrupted"] { assert!(!carry_will(&th(s), "on")) } }
    #[test] fn carry_every_provider_that_resumes() {
        for p in CARRIES { let mut t = th("running"); t.provider = p.into(); t.session = "S".into(); assert!(carry_will(&t, "on"), "{p}"); if p != "claude" { assert!(super::super::providers::driver(p).is_some(), "{p} is run here"); t.session.clear(); assert!(!carry_will(&t, "on"), "{p} has nothing to resume") } }
        let mut t = th("running"); t.provider = "nonesuch".into(); t.session = "S".into(); assert!(!carry_will(&t, "on"));
    }
    #[test] fn carry_off_nothing() { assert!(!carry_will(&th("running"), "off")); }
    #[test] fn carry_once() { let mut t = th("running"); t.carry_text_last = true; assert!(!carry_will(&t, "on")); }
    #[test] fn carry_skips_archived() { let mut t = th("running"); t.archived = true; assert!(!carry_will(&t, "on")); t.archived = false; t.deleted = true; assert!(!carry_will(&t, "on")); }
    // law carry_twice_once: carrying on twice in a row carries on once (the second restart finds the continue message newest, past the restart note)
    #[test] fn carry_twice_once() {
        let mut w = World::default();
        for l in [r#"{"$":"ThreadCreated","id":"t","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}"#, r#"{"$":"MessagePosted","thread":"t","msg":"m","role":"user","text":"go","at":3}"#, r#"{"$":"TurnChanged","thread":"t","state":"running","at":3}"#] { w.apply(l) }
        assert!(carry_will(&w.threads["t"], "on"));
        for l in [format!(r#"{{"$":"MessagePosted","thread":"t","msg":"c","role":"user","text":"{} x","at":4}}"#, CARRY_TEXT), r#"{"$":"ActivityLogged","thread":"t","act":"r","tone":"info","kind":"restart","summary":"Continuing","at":4}"#.to_string()] { w.apply(&l) }
        assert!(!carry_will(&w.threads["t"], "on"));
    }
    #[test] fn a_thread_is_folded_from_its_events() {
        let mut w = World::default();
        for l in [r#"{"$":"ProjectCreated","id":"p","title":"x","root":"/r","at":1}"#, r#"{"$":"ThreadCreated","id":"t","project":"p","title":"T","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2}"#,
            r#"{"$":"TurnChanged","thread":"t","state":"running","at":3}"#, r#"{"$":"SessionSet","id":"t","session":"S","at":3}"#, r#"{"$":"UsageSet","thread":"t","used":5,"window":0,"at":3}"#, r#"{"$":"UsageSet","thread":"t","used":0,"window":9,"at":3}"#] { w.apply(l) }
        assert_eq!(w.cwd("t"), "/r"); assert!(w.running("t")); let c = w.cx("t"); assert_eq!((c.session.as_str(), c.used, c.window), ("S", 5, 9));
    }
    #[test] fn unread_event_kinds_are_skipped_without_parsing() { let mut w = World::default(); w.apply(r#"{"$":"RoomPosted","x":1}"#); w.apply("not json"); assert!(w.threads.is_empty()); }
    #[test] fn a_bots_thread_always_works_in_the_homes_bots_folder() {
        let home = std::env::temp_dir().join(format!("bp-botcwd-{}", std::process::id())); let _ = std::fs::remove_dir_all(&home);
        let mut w = World::default(); w.home = home.to_string_lossy().into_owned();
        w.apply(r#"{"$":"ThreadCreated","id":"t","project":"","title":"Bub","provider":"claude","model":"","worktree":"","at":1}"#);
        w.apply(r#"{"$":"BotSet","id":"bub","name":"Bub","thread":"t","look":1,"persona":"","at":2}"#);
        w.apply(r#"{"$":"WorktreeSet","id":"t","branch":"","worktree":"/elsewhere/lanes/bub","at":3}"#);
        assert_eq!(w.cwd("t"), home.join("bots").join("bub").to_string_lossy()); assert!(home.join("bots/bub").is_dir());
        let _ = std::fs::remove_dir_all(&home);
    }
}

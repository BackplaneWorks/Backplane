//! The slice of the read model the agent rules look at, folded from the log by this module alone
//! (threads' provider/model/modes/session/turn/worktree, the settings these rules read). Kept by catching up
//! from the store: `sync` reads the lines it has not seen. Bounded by threads and the few setting keys, not by the log.
use super::lines::{num, Cx};
use crate::store::Store;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct T {
    pub project: String, pub provider: String, pub model: String, pub session: String, pub worktree: String,
    pub runtime: String, pub interaction: String, pub turn: String, pub archived: bool, pub deleted: bool,
    pub title: String, pub last_user: String, pub newest: String, pub carry_text_last: bool,
}

#[derive(Default)]
pub struct World { pub threads: HashMap<String, T>, pub roots: HashMap<String, String>, pub st: HashMap<String, String>, seen: usize }

const KEYS: [&str; 9] = ["tz.machine", "agent.thinking", "restart.continue", "tok.epoch", "tok.recount", "tok.basis.", "turn.speed.", "turn.models.", "limit.claude."];
fn tracked(k: &str) -> bool { KEYS.iter().any(|p| k == *p || (p.ends_with('.') && k.starts_with(p))) || k.starts_with("usage.") || k.starts_with("tok.key.") }

pub const CARRY_TEXT: &str = "Backplane restarted while you were working. Continue where you left off.";

/// Carry.will: a turn the restart cut short carries on when the setting is on, the thread was running, is live and has made
/// progress since the last carry. Claude threads only here (others are not run by this hub yet).
pub fn carry_will(t: &T, restart_continue: &str) -> bool {
    restart_continue != "off" && t.turn == "running" && !t.archived && !t.deleted && !t.carry_text_last && t.provider == "claude"
}

impl World {
    pub fn seen(&self) -> usize { self.seen }
    pub fn setting(&self, k: &str) -> &str { self.st.get(k).map(String::as_str).unwrap_or("") }

    /// Catch up with the store (whole lines past what was seen). A cheap substring test skips lines no rule reads.
    pub fn sync(&mut self, store: &Store) -> Result<()> {
        let total = store.len();
        while self.seen < total {
            let b = (self.seen + 4096).min(total);
            for l in store.range(self.seen, b)? { self.apply(&l) }
            self.seen = b;
        }
        Ok(())
    }

    pub fn apply(&mut self, line: &str) {
        let Some(t) = line.strip_prefix("{\"$\":\"").and_then(|r| r.split('"').next()) else { return };
        const READ: [&str; 19] = ["ThreadRenamed", "ProjectCreated", "ThreadCreated", "WorktreeSet", "ProviderSet", "ModesSet", "SessionSet", "TurnChanged", "MessagePosted", "ActivityLogged", "UsageSet", "SettingSet", "TurnUsage", "ThreadArchived", "ThreadDeleted", "ThreadForked", "QueueTaken", "TurnQueued", "ThreadWoke"];
        if !READ.contains(&t) { return }
        let Ok(v) = serde_json::from_str::<Value>(line) else { return };
        let g = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        match t {
            "ProjectCreated" => { self.roots.insert(g("id"), g("root")); }
            "ThreadCreated" => { self.threads.insert(g("id"), T { project: g("project"), title: g("title"), provider: g("provider"), model: g("model"), worktree: g("worktree"), runtime: "full-access".into(), interaction: "default".into(), turn: "idle".into(), ..Default::default() }); }
            "WorktreeSet" => if let Some(x) = self.threads.get_mut(&g("id")) { x.worktree = g("worktree") },
            "ProviderSet" => if let Some(x) = self.threads.get_mut(&g("id")) { x.provider = g("provider"); x.model = g("model") },
            "ModesSet" => if let Some(x) = self.threads.get_mut(&g("id")) { x.runtime = g("runtime"); x.interaction = g("interaction"); x.model = g("model") },
            "SessionSet" => if let Some(x) = self.threads.get_mut(&g("id")) { x.session = g("session") },
            "ThreadRenamed" => if let Some(x) = self.threads.get_mut(&g("id")) { x.title = g("title") },
            "ThreadArchived" => if let Some(x) = self.threads.get_mut(&g("id")) { x.archived = v.get("archived").and_then(Value::as_bool).unwrap_or(false) },
            "ThreadDeleted" => if let Some(x) = self.threads.get_mut(&g("id")) { x.deleted = true },
            "TurnChanged" => if let Some(x) = self.threads.get_mut(&g("thread")) { x.turn = g("state") },
            "MessagePosted" => if let Some(x) = self.threads.get_mut(&g("thread")) {
                x.newest = g("msg");
                x.carry_text_last = v.get("role").and_then(Value::as_str) == Some("user") && g("text").starts_with(CARRY_TEXT);
                if v.get("role").and_then(Value::as_str) == Some("user") { x.last_user = g("msg") }
            },
            "ActivityLogged" => if let Some(x) = self.threads.get_mut(&g("thread")) { x.newest = g("act"); if g("kind") != "restart" { x.carry_text_last = false } },
            "UsageSet" => { let k = format!("usage.{}", g("thread")); let old = self.setting(&k).to_string(); let (u, w) = (v.get("used").and_then(Value::as_u64).unwrap_or(0), v.get("window").and_then(Value::as_u64).unwrap_or(0));
                let keep = |new: u64, old: u64| if new == 0 { old } else { new };
                let ps: Vec<u64> = old.split('/').map(num).collect(); self.st.insert(k, format!("{}/{}", keep(u, *ps.first().unwrap_or(&0)), keep(w, *ps.get(1).unwrap_or(&0)))); }
            "TurnUsage" => { self.st.insert(format!("tok.key.{}", g("thread")), g("key")); self.st.insert(format!("tok.basis.{}", g("thread")), g("basis")); }
            "SettingSet" => { let k = g("key"); if tracked(&k) { self.st.insert(k, g("value")); } }
            _ => {}
        }
    }

    /// Where a thread's agent works: its worktree, else its project's folder (Hub.cwd).
    pub fn cwd(&self, th: &str) -> String {
        let Some(t) = self.threads.get(th) else { return String::new() };
        if !t.worktree.is_empty() { t.worktree.clone() } else { self.roots.get(&t.project).cloned().unwrap_or_default() }
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
            limits: self.st.iter().filter(|(k, _)| k.starts_with("limit.claude.")).map(|(k, v)| (k.clone(), v.clone())).collect(), seq: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn th(turn: &str) -> T { T { turn: turn.into(), provider: "claude".into(), ..Default::default() } }
    #[test] fn carry_continues() { assert!(carry_will(&th("running"), "")); assert!(carry_will(&th("running"), "on")); }
    #[test] fn carry_leaves_idle() { for s in ["completed", "idle", "failed", "interrupted"] { assert!(!carry_will(&th(s), "on")) } }
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
}

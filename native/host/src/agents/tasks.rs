//! Delegated tasks: `delegate_task` makes a child thread of the caller
//! on any provider, `task_status` reads one, `task_cancel` stops it; when the child's turn ends the result goes to the parent as a message, once (TaskDelivered last).
//! The end is noticed by a once-a-second look at the log (`watch`), which is also Task.repair after a restart: a finished task not yet delivered is delivered then.
use super::{hub_id, lines::s, now_s, rules, Agents};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

/// Task.max_depth
pub const MAX_DEPTH: u64 = 3;
fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }
/// Task.note
pub fn note(id: &str, provider: &str, model: &str, word: &str, summary: &str) -> String {
    format!("[Backplane] Delegated task {id} ({provider}{}) {word}:\n\n{summary}", if model.is_empty() { String::new() } else { format!("/{model}") })
}
/// Task.msg: M.Link.id(child, "td-<id>")
pub fn msg(child: &str, id: &str) -> String { format!("t:{child}:td-{id}") }
fn task_json(id: &str, k: &super::world::Tk) -> Value {
    json!({"taskId":id,"parentThreadId":k.parent,"childThreadId":k.child,"depth":k.depth,"state":k.state,"provider":k.provider,"model":k.model,"summary":k.summary})
}

impl Agents {
    /// The tool delegate_task {prompt, provider, model, title}: a child thread in the caller's project and folder, its first message the prompt.
    /// (Bots, which name a project of their own, are not in the Rust hub yet.)
    pub async fn tool_delegate(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let prompt = s(a, "prompt").to_string();
        let (t, depth) = { let w = self.world().await; let Some(t) = w.threads.get(thread).cloned() else { return fail("unknown thread") }; (t, w.depth(thread)) };
        if depth >= MAX_DEPTH { return fail("delegation depth limit reached: finish this task yourself") }
        if prompt.trim().is_empty() { return fail("prompt is required") }
        let asked = if s(a, "provider").is_empty() { if a.get("account").is_some() && !s(a, "account").eq_ignore_ascii_case("default") { String::new() } else { t.provider.clone() } } else { s(a, "provider").to_string() };
        let chosen = match self.account_arg(a, &asked).await { Ok(c) => c, Err(e) => return fail(&e) };
        let provider = chosen.as_ref().map(|c| c.provider.clone()).unwrap_or(if asked.is_empty() { t.provider.clone() } else { asked });
        let model = if !s(a, "model").is_empty() { s(a, "model").to_string() } else if provider == t.provider { t.model.clone() } else { String::new() };
        if provider != "claude" && super::providers::driver(&provider).is_none() { return fail(&format!("{provider} threads are not run by the Rust hub yet")) }
        let (now, seq) = (now_s(), self.seq_n());
        let (task, child, depth1) = (hub_id("k", now, seq), hub_id("t", now, seq + 1), depth + 1);
        let title = if s(a, "title").is_empty() { rules::title_of(&prompt) } else { s(a, "title").to_string() };
        let acct = self.thread_account(thread, &child, &provider, chosen.as_ref(), now).await;
        let ev = vec![
            json!({"$":"ThreadCreated","id":child,"project":t.project,"title":title,"env":t.env,"branch":t.branch,"worktree":t.worktree,"provider":provider,"model":model,"at":now})].into_iter().chain(acct).chain([
            json!({"$":"ModesSet","id":child,"runtime":t.runtime,"interaction":t.interaction,"model":model,"at":now}), // the parent's modes
            json!({"$":"TaskDelegated","task":task,"parent":thread,"child":child,"depth":depth1,"provider":provider,"model":model,"at":now}),
            json!({"$":"TaskStateSet","task":task,"state":"running","summary":"","at":now}),
            // the child sits under its parent in the sidebar, the parent's children open (as thread_launch does)
            json!({"$":"ThreadParentSet","id":child,"parent":thread,"at":now}),
            json!({"$":"SettingSet","key":format!("thread.tree.{thread}"),"value":"1","at":now}),
        ]).collect::<Vec<Value>>();
        self.commit_events(ev).await;
        if let Err(e) = self.deliver_as("user", &child, &hub_id("m", now, seq + 2), &prompt, None, vec![]).await {
            self.commit_events(vec![json!({"$":"TaskStateSet","task":task,"state":"failed","summary":format!("could not start the child: {e}"),"at":now_s()})]).await; // never left running with no turn
            return fail(&format!("could not start the child: {e}"))
        }
        (true, json!({"taskId":task,"childThreadId":child,"state":"running","provider":provider,"model":model,"account":chosen.map(|c| c.label),"depth":depth1,
            "note":"The result will arrive in this thread as a message when the child finishes. End your turn instead of polling."}))
    }
    /// The tool task_status {taskId}: only a task this thread delegated.
    pub async fn tool_task_status(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let w = self.world().await; match w.tasks.get(s(a, "taskId")).filter(|k| k.parent == thread) { Some(k) => (true, task_json(s(a, "taskId"), k)), None => fail("no such task of this thread") }
    }
    /// The tool task_cancel {taskId} (Mcp.cancel): the step always logs TaskStateSet cancelled ("cancelled by the parent"; a finished task keeps its state but its summary is replaced),
    /// TurnChanged interrupted on the child, and the child's agent stops. The reply keeps the task's old summary.
    pub async fn tool_task_cancel(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let id = s(a, "taskId").to_string();
        let k = { let w = self.world().await; w.tasks.get(&id).filter(|k| k.parent == thread).cloned() };
        let Some(mut k) = k else { return fail("no such task of this thread") };
        let now = now_s();
        self.commit_events(vec![json!({"$":"TaskStateSet","task":id,"state":"cancelled","summary":"cancelled by the parent","at":now}), json!({"$":"TurnChanged","thread":k.child,"state":"interrupted","at":now})]).await;
        let _ = self.stop(&k.child).await;
        if !k.terminal() { k.state = "cancelled".into() }
        (true, task_json(&id, &k))
    }

    /// Task.finish.w: a child's turn ended (`ended`: completed | failed | interrupted). A completed turn that left work out keeps the task open; a failure or a stop ends it at once.
    pub async fn task_end(self: &Arc<Self>, child: &str, ended: &str) {
        let plan = { let w = self.world().await; finish_plan(&w, child, ended) };
        let Some(f) = plan else { return };
        self.commit_events(vec![json!({"$":"TaskStateSet","task":f.id,"state":f.want,"summary":f.summary,"at":now_s()})]).await;
        if let Some(text) = f.deliver { self.deliver_result(&f.id, &f.parent, &f.child, &text).await }
    }
    /// The result reaches the parent first; TaskDelivered only once the message is in (a failed delivery stays owed and is repaired at the next start).
    async fn deliver_result(self: &Arc<Self>, id: &str, parent: &str, child: &str, text: &str) {
        if self.inbound("system", parent, &msg(child, id), text, (child.to_string(), "task".into(), String::new()), vec![]).await.is_ok() {
            self.commit_events(vec![json!({"$":"TaskDelivered","task":id,"at":now_s()})]).await;
        }
    }
    /// Task.repair: run the plan taken at start (before recovery).
    pub async fn task_repair_run(self: &Arc<Self>, plan: Vec<Repair>) {
        for r in plan {
            let now = now_s();
            if let Some((st, sum)) = &r.state_set { self.commit_events(vec![json!({"$":"TaskStateSet","task":r.id,"state":st,"summary":sum,"at":now})]).await }
            match (r.has, r.wake) {
                (true, true) => { // the parent already holds the message, idle with it newest: its turn starts with the text
                    if self.resume_with(&r.parent, &r.text).await.is_ok() { self.commit_events(vec![json!({"$":"TaskDelivered","task":r.id,"at":now_s()})]).await }
                }
                (true, false) => self.commit_events(vec![json!({"$":"TaskDelivered","task":r.id,"at":now})]).await,
                (false, _) => self.deliver_result(&r.id, &r.parent, &r.child, &r.text).await,
            }
        }
    }
}

/// Startup backfill: a delegated child with no parent of its own (no ThreadParentSet, no thread.parent setting: an empty one is a detach the
/// person made) is put under the thread that delegated it, once. Delegations logged before delegate_task wrote ThreadParentSet only had the task
/// record, so thread_list, the commands' fold and linked machines showed them top level. A bot's delegated work stays a thread of its own (as State does).
pub fn parent_backfill(w: &super::world::World, now: u64) -> Vec<Value> {
    let mut ks: Vec<(&String, &super::world::Tk)> = w.tasks.iter().collect(); ks.sort_by(|a, b| a.0.cmp(b.0));
    let mut done = std::collections::HashSet::new(); let mut ev = vec![];
    for (_, k) in ks {
        if k.parent.is_empty() || k.child == k.parent || w.bot_of.contains_key(&k.parent) || !done.insert(k.child.clone()) { continue }
        if w.parents.contains_key(&k.child) || w.st.contains_key(&format!("thread.parent.{}", k.child)) { continue }
        let (Some(c), Some(p)) = (w.threads.get(&k.child), w.threads.get(&k.parent)) else { continue };
        if c.deleted || p.deleted || c.project != p.project { continue }
        let mut cur = k.parent.clone(); let mut ok = false; // never a cycle
        for _ in 0..64 { if cur.is_empty() { ok = true; break } if cur == k.child { break } cur = w.tree_raw(&cur) }
        if ok { ev.push(json!({"$":"ThreadParentSet","id":k.child,"parent":k.parent,"at":now})) }
    }
    ev
}

/// Task.finish.w as a plan over the state: what to log for a child's turn that ended, or None.
pub struct Finish { pub id: String, pub parent: String, pub child: String, pub want: &'static str, pub summary: String, pub deliver: Option<String> }
pub fn finish_plan(w: &super::world::World, child: &str, ended: &str) -> Option<Finish> {
    let (id, k) = w.tasks.iter().find(|(_, k)| k.child == child)?;
    if k.state == "cancelled" { return None } // the parent's own cancel is not an end to report (Mcp.cancel runs no Task.finish)
    // decides inside the step; here the child must still be at the end it was read at, with no ride that starts its next turn
    if w.threads.get(child).map_or(true, |c| c.turn != ended) || !w.setting(&format!("ride.{child}")).is_empty() { return None }
    if ended == "completed" && !w.ready(child) { return None } // task_waits_for_child_work
    let (want, word) = if ended == "completed" { ("done", "finished") } else { ("failed", "failed") }; // task_failed_ends_even_busy
    let last = w.threads.get(child).map(|t| t.last_assistant.as_str()).unwrap_or("");
    let why = w.threads.get(child).map(|t| t.why.as_str()).unwrap_or("");
    let last = &if want == "failed" && !why.is_empty() { super::launch::with_why(why, last) } else { last.to_string() };
    let pending = !k.delivered; // terminal after next(): the state sticks or becomes the wanted terminal one
    Some(Finish { id: id.clone(), parent: k.parent.clone(), child: child.into(), want, summary: last.to_string(), deliver: if pending { Some(note(id, &k.provider, &k.model, word, last)) } else { None } })
}
/// The last TurnChanged of `thread` in a step's events ended it, unless a later change runs it again (task_ended_last_change).
pub fn ended_last(events: &[Value], thread: &str) -> Option<&'static str> {
    let last = events.iter().rev().find(|e| e["$"] == "TurnChanged" && e["thread"] == thread)?;
    if last["failover"] == true { return None } // it stopped to change accounts and goes on: it has not ended
    let st = last["state"].as_str()?;
    match st { "completed" => Some("completed"), "failed" => Some("failed"), "interrupted" => Some("interrupted"), _ => None }
}

/// One owed delivery found at start (Task.repair.one).
#[derive(Debug, Clone, PartialEq)]
pub struct Repair { pub id: String, pub parent: String, pub child: String, pub state_set: Option<(String, String)>, pub text: String, pub has: bool, pub wake: bool }
fn word_of(state: &str) -> &'static str { if state == "done" { "finished" } else { "failed" } }
/// Task.repair over the state the hub started from: a finished task not delivered is delivered (the parent that already holds the message only gets TaskDelivered, and, idle with it newest,
/// its turn starts); a task torn before its TaskStateSet is finished when its child's turn ended and its work is done. A child the restart cut short is the restart's business.
pub fn repair_plan(w: &super::world::World) -> Vec<Repair> {
    let mut ids: Vec<&String> = w.tasks.keys().collect(); ids.sort();
    let mut out = vec![];
    for id in ids {
        let k = &w.tasks[id]; let last = w.threads.get(&k.child).map(|t| t.last_assistant.clone()).unwrap_or_default();
        let (state_set, text) = if k.terminal() {
            if k.delivered { continue }
            (None, note(id, &k.provider, &k.model, word_of(&k.state), &if k.summary.is_empty() { last } else { k.summary.clone() }))
        } else {
            let turn = w.threads.get(&k.child).map(|t| t.turn.as_str()).unwrap_or("");
            match (turn, w.ready(&k.child)) {
                ("completed", true) => (Some(("done".to_string(), last.clone())), note(id, &k.provider, &k.model, "finished", &last)),
                ("failed", _) | ("interrupted", _) => (Some(("failed".to_string(), last.clone())), note(id, &k.provider, &k.model, "failed", &last)),
                _ => continue,
            }
        };
        let m = msg(&k.child, id); let p = w.threads.get(&k.parent);
        let has = w.td_msgs.contains(&m);
        let wake = p.map_or(false, |p| p.turn != "running" && p.newest == m) && !w.queue_waiting(&k.parent);
        out.push(Repair { id: id.clone(), parent: k.parent.clone(), child: k.child.clone(), state_set, text, has, wake });
    }
    out
}

/// The look at the log (Task.finish for a live end): every TurnChanged that ends a delegated child. Starts after recovery, so the interrupted turns a restart writes are the restart's business.
pub fn watch(ag: &Arc<Agents>) {
    let weak = Arc::downgrade(ag); let mut rx = ag.hub.subscribe(); let mut seen = ag.hub.store.len();
    tokio::spawn(async move {
        loop {
            match rx.recv().await { Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {} Err(_) => break }
            let Some(ag) = weak.upgrade() else { break };
            let total = ag.hub.store.len(); if total <= seen { continue }
            let mut evs: Vec<Value> = vec![];
            while seen < total { // read in chunks; a failed read leaves `seen` where it was and is tried again at the next commit
                let to = (seen + 4096).min(total);
                let Ok(lines) = ag.hub.store.range(seen, to) else { break };
                evs.extend(lines.iter().filter(|l| matches!(crate::session::event_type(l), Some("TurnChanged" | "AskOpened"))).filter_map(|l| serde_json::from_str::<Value>(l).ok())); seen = to;
            }
            if evs.is_empty() { continue }
            let mut kids: Vec<String> = evs.iter().filter_map(|e| e["thread"].as_str().map(str::to_string)).collect(); kids.sort(); kids.dedup();
            for kid in kids {
                if let Some(end) = ended_last(&evs, &kid) { let a = ag.clone(); a.task_end(&kid, end).await }
                let from = ag.stopped_by.lock().unwrap().remove(&kid).unwrap_or_default(); // the launcher that stopped the thread itself is not told
                ag.launch_tell(&kid, &evs, &from).await;
            }
        }
    });
}

#[cfg(test)]
mod laws {
    use super::*;
    use crate::agents::world::World;
    fn world(extra: &[&str]) -> World {
        let mut w = World::default();
        for l in ["{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"p\",\"root\":\"/p\",\"at\":1}", "{\"$\":\"ThreadCreated\",\"id\":\"t0\",\"project\":\"p1\",\"title\":\"x\",\"provider\":\"codex\",\"model\":\"\",\"at\":1}",
            "{\"$\":\"ThreadCreated\",\"id\":\"t1\",\"project\":\"p1\",\"title\":\"y\",\"provider\":\"codex\",\"model\":\"\",\"at\":1}",
            "{\"$\":\"TaskDelegated\",\"task\":\"k1\",\"parent\":\"t0\",\"child\":\"t1\",\"depth\":1,\"provider\":\"codex\",\"model\":\"\",\"at\":1}", "{\"$\":\"TaskStateSet\",\"task\":\"k1\",\"state\":\"running\",\"summary\":\"\",\"at\":1}",
            "{\"$\":\"MessagePosted\",\"thread\":\"t1\",\"msg\":\"a1\",\"role\":\"assistant\",\"text\":\"done it\",\"at\":2}"] { w.apply(l) }
        for l in extra { w.apply(l) } w
    }
    fn busy() -> &'static str { "{\"$\":\"TaskDelegated\",\"task\":\"k2\",\"parent\":\"t1\",\"child\":\"t2\",\"depth\":2,\"provider\":\"codex\",\"model\":\"\",\"at\":1}" }
    const DONE: &str = "{\"$\":\"TaskStateSet\",\"task\":\"k1\",\"state\":\"done\",\"summary\":\"done it\",\"at\":3}";
    const MSG: &str = "{\"$\":\"MessagePosted\",\"thread\":\"t0\",\"msg\":\"t:t1:td-k1\",\"role\":\"system\",\"text\":\"note\",\"at\":3}";
    fn tc(th: &str, st: &str) -> String { format!("{{\"$\":\"TurnChanged\",\"thread\":\"{th}\",\"state\":\"{st}\",\"at\":9}}") }
    fn demo(stage: u8) -> World { let mut v = vec![DONE]; let r = tc("t0", "running"); if stage >= 1 { v.push(MSG) } if stage >= 2 { v.push(&r) } let d = "{\"$\":\"TaskDelivered\",\"task\":\"k1\",\"at\":3}"; if stage >= 3 { v.push(d) } world(&v) }

    #[test] fn task_terminal_absorbing() { let w = world(&[DONE, "{\"$\":\"TaskStateSet\",\"task\":\"k1\",\"state\":\"failed\",\"summary\":\"later\",\"at\":4}"]); assert_eq!(w.tasks["k1"].state, "done"); assert_eq!(w.tasks["k1"].summary, "later", "the summary is replaced; the state sticks") }
    #[test] fn task_delivered_once() { let w = world(&[DONE, "{\"$\":\"TaskDelivered\",\"task\":\"k1\",\"at\":4}", &tc("t1", "completed")]); assert!(finish_plan(&w, "t1", "completed").unwrap().deliver.is_none()); assert!(repair_plan(&w).is_empty()) }
    #[test] fn task_ended_last_change() {
        let evs = |v: &[&str]| v.iter().map(|s| serde_json::from_str::<Value>(s).unwrap()).collect::<Vec<_>>();
        assert_eq!(ended_last(&evs(&[&tc("t1", "completed"), "{\"$\":\"QueueTaken\",\"thread\":\"t1\",\"at\":9}", &tc("t1", "running")]), "t1"), None);
        assert_eq!(ended_last(&evs(&[&tc("t1", "running"), &tc("t1", "completed")]), "t1"), Some("completed")); assert_eq!(ended_last(&evs(&[&tc("t2", "completed")]), "t1"), None);
    }
    #[test] fn task_waits_for_child_work() { assert!(finish_plan(&world(&[busy(), &tc("t1", "completed")]), "t1", "completed").is_none(), "the child delegated work of its own that still runs") }
    #[test] fn task_ends_when_ready() { let f = finish_plan(&world(&[&tc("t1", "completed")]), "t1", "completed").unwrap(); assert_eq!((f.want, f.summary.as_str()), ("done", "done it")); assert!(f.deliver.unwrap().starts_with("[Backplane] Delegated task k1 (codex) finished:\n\ndone it")) }
    #[test] fn task_failed_ends_even_busy() { let f = finish_plan(&world(&[busy(), &tc("t1", "failed")]), "t1", "failed").unwrap(); assert_eq!(f.want, "failed"); assert!(finish_plan(&world(&[busy(), &tc("t1", "interrupted")]), "t1", "interrupted").is_some()) }
    #[test] fn task_repair_delivers() { let p = repair_plan(&demo(0)); assert_eq!(p.len(), 1); assert!(!p[0].has && p[0].state_set.is_none() && p[0].text.contains("finished:\n\ndone it")) }
    #[test] fn task_repair_activates_idle() { let p = repair_plan(&demo(1)); assert_eq!(p.len(), 1); assert!(p[0].has && p[0].wake) }
    #[test] fn task_repair_running_marks_only() { let p = repair_plan(&demo(2)); assert_eq!(p.len(), 1); assert!(p[0].has && !p[0].wake) }
    #[test] fn task_repair_quiet_when_done() { assert!(repair_plan(&demo(3)).is_empty()) }
    #[test] fn task_repair_holds_through_later_entries() { // Task.holds looks through all entries, not only the newest
        let w = world(&[DONE, MSG, "{\"$\":\"MessagePosted\",\"thread\":\"t0\",\"msg\":\"m9\",\"role\":\"user\",\"text\":\"later\",\"at\":4}"]);
        let p = repair_plan(&w); assert_eq!(p.len(), 1); assert!(p[0].has && !p[0].wake, "the message is there, something followed it: only TaskDelivered") }
    #[test] fn task_repair_finishes_ended_child() { let w = world(&[&tc("t1", "completed")]); let p = repair_plan(&w); assert_eq!(p.len(), 1); assert_eq!(p[0].state_set.as_ref().unwrap().0, "done") }
    #[test] fn task_repair_leaves_working_child() { assert!(repair_plan(&world(&[&tc("t1", "running")])).is_empty()); assert!(repair_plan(&world(&[])).is_empty()) }

    #[test] fn a_cancelled_task_is_not_reported_by_its_interrupted_turn() { let w = world(&["{\"$\":\"TaskStateSet\",\"task\":\"k1\",\"state\":\"cancelled\",\"summary\":\"cancelled by the parent\",\"at\":3}", &tc("t1", "interrupted")]); assert!(finish_plan(&w, "t1", "interrupted").is_none()) }
    #[test] fn the_end_is_judged_against_the_child_as_it_is_now() {
        assert!(finish_plan(&world(&[&tc("t1", "running")]), "t1", "completed").is_none(), "the child goes on");
        assert!(finish_plan(&world(&[&tc("t1", "completed"), "{\"$\":\"SettingSet\",\"key\":\"ride.t1\",\"value\":\"[{}]\",\"at\":9}"]), "t1", "completed").is_none(), "a ride starts its next turn");
        assert!(finish_plan(&world(&[&tc("t1", "completed")]), "t1", "completed").is_some()) }
    #[test] fn a_queued_message_holds_the_child_and_the_parent() {
        let q = "{\"$\":\"TurnQueued\",\"thread\":\"t1\",\"msg\":\"m1\",\"text\":\"x\",\"at\":9}";
        assert!(finish_plan(&world(&[&tc("t1", "completed"), q]), "t1", "completed").is_none(), "Work.ready: a queued message keeps the task open");
        let handoff = "{\"$\":\"TurnQueued\",\"thread\":\"t1\",\"msg\":\"handoff:1\",\"text\":\"x\",\"at\":9}"; assert!(finish_plan(&world(&[&tc("t1", "completed"), handoff]), "t1", "completed").is_some(), "handed-over context alone never holds");
        let pq = "{\"$\":\"TurnQueued\",\"thread\":\"t0\",\"msg\":\"m2\",\"text\":\"x\",\"at\":9}"; let w = world(&[DONE, MSG, pq]); let p = repair_plan(&w); assert!(p[0].has && !p[0].wake, "Queue.waiting(parent) keeps the idle parent from waking");
        let taken = "{\"$\":\"QueueTaken\",\"thread\":\"t0\",\"at\":9}"; assert!(repair_plan(&world(&[DONE, MSG, pq, taken]))[0].wake) }
    #[test] fn a_delegated_child_without_a_parent_is_backfilled_once() {
        let ev = parent_backfill(&world(&[]), 5); assert_eq!(ev, vec![json!({"$":"ThreadParentSet","id":"t1","parent":"t0","at":5})]);
        let mut w = world(&[]); for e in &ev { w.apply(&e.to_string()) } assert!(parent_backfill(&w, 6).is_empty(), "once"); assert_eq!(w.tree_raw("t1"), "t0");
        assert!(parent_backfill(&world(&["{\"$\":\"ThreadParentSet\",\"id\":\"t1\",\"parent\":\"\",\"at\":2}"]), 5).is_empty(), "a detach is the person's");
        assert!(parent_backfill(&world(&["{\"$\":\"SettingSet\",\"key\":\"thread.parent.t1\",\"value\":\"\",\"at\":2}"]), 5).is_empty());
        assert!(parent_backfill(&world(&["{\"$\":\"BotSet\",\"id\":\"b1\",\"name\":\"b\",\"thread\":\"t0\",\"at\":2}"]), 5).is_empty(), "a bot's work stays its own thread");
    }
    #[test] fn the_newest_parent_move_wins_over_an_older_setting() {
        let w = world(&["{\"$\":\"SettingSet\",\"key\":\"thread.parent.t1\",\"value\":\"\",\"at\":2}", "{\"$\":\"ThreadParentSet\",\"id\":\"t1\",\"parent\":\"t0\",\"at\":3}"]); assert_eq!(w.tree_raw("t1"), "t0") }
    #[test] fn the_launch_chain_is_bounded_and_acyclic() {
        let mut w = world(&[]); for i in 0..70 { w.apply(&format!("{{\"$\":\"ThreadParentSet\",\"id\":\"c{i}\",\"parent\":\"c{}\",\"at\":1}}", i + 1)) }
        assert!(!w.tree_chain_ok("c0"), "a chain beyond 63 is refused"); assert!(w.tree_chain_ok("c10"), "a short one is fine"); assert!(w.tree_chain_ok("t1"));
        w.apply("{\"$\":\"ThreadParentSet\",\"id\":\"t0\",\"parent\":\"t1\",\"at\":1}"); w.apply("{\"$\":\"ThreadParentSet\",\"id\":\"t1\",\"parent\":\"t0\",\"at\":1}"); assert!(!w.tree_chain_ok("t0"), "a cycle is refused") }
}

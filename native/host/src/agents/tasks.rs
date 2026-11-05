//! Delegated tasks (mcp.bend Mcp.delegate / Mcp.status / Mcp.cancel; hub.bend Task.finish, Task.note, Task.msg): `delegate_task` makes a child thread of the caller
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
        let (t, depth, cwd) = { let w = self.world().await; let Some(t) = w.threads.get(thread).cloned() else { return fail("unknown thread") }; (t, w.depth(thread), w.cwd(thread)) };
        if depth >= MAX_DEPTH { return fail("delegation depth limit reached: finish this task yourself") }
        if prompt.trim().is_empty() { return fail("prompt is required") }
        let provider = if s(a, "provider").is_empty() { t.provider.clone() } else { s(a, "provider").to_string() };
        let model = if !s(a, "model").is_empty() { s(a, "model").to_string() } else if provider == t.provider { t.model.clone() } else { String::new() };
        if provider != "claude" && super::providers::driver(&provider).is_none() { return fail(&format!("{provider} threads are not run by the Rust hub yet")) }
        let (now, seq) = (now_s(), self.seq_n());
        let (task, child, depth1) = (hub_id("k", now, seq), hub_id("t", now, seq + 1), depth + 1);
        let title = if s(a, "title").is_empty() { rules::title_of(&prompt) } else { s(a, "title").to_string() };
        let ev = vec![
            json!({"$":"ThreadCreated","id":child,"project":t.project,"title":title,"env":t.env,"branch":t.branch,"worktree":t.worktree,"provider":provider,"model":model,"at":now}),
            json!({"$":"ModesSet","id":child,"runtime":t.runtime,"interaction":"default","model":model,"at":now}),
            json!({"$":"TaskDelegated","task":task,"parent":thread,"child":child,"depth":depth1,"provider":provider,"model":model,"at":now}),
            json!({"$":"TaskStateSet","task":task,"state":"running","summary":"","at":now}),
        ];
        self.commit_events(ev).await; let _ = cwd;
        if let Err(e) = self.deliver_as("user", &child, &hub_id("m", now, seq + 2), &prompt, None, vec![]).await { return fail(&format!("could not start the child: {e}")) }
        (true, json!({"taskId":task,"childThreadId":child,"state":"running","provider":provider,"model":model,"depth":depth1,
            "note":"The result will arrive in this thread as a message when the child finishes. End your turn instead of polling."}))
    }
    /// The tool task_status {taskId}: only a task this thread delegated.
    pub async fn tool_task_status(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let w = self.world().await; match w.tasks.get(s(a, "taskId")).filter(|k| k.parent == thread) { Some(k) => (true, task_json(s(a, "taskId"), k)), None => fail("no such task of this thread") }
    }
    /// The tool task_cancel {taskId}: the child stops; a finished task keeps its result.
    pub async fn tool_task_cancel(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let id = s(a, "taskId").to_string();
        let k = { let w = self.world().await; w.tasks.get(&id).filter(|k| k.parent == thread).cloned() };
        let Some(mut k) = k else { return fail("no such task of this thread") };
        if !k.terminal() {
            self.commit_events(vec![json!({"$":"TaskStateSet","task":id,"state":"cancelled","summary":"cancelled by the parent","at":now_s()})]).await;
            let _ = self.interrupt(&k.child).await; let _ = self.stop(&k.child).await;
            k.state = "cancelled".into(); k.summary = "cancelled by the parent".into();
        }
        (true, task_json(&id, &k))
    }

    /// Task.finish / Task.repair for every task whose child's turn has ended (or that ended and was not delivered): the state is set, the result goes to the parent once, TaskDelivered last.
    pub async fn tasks_tick(self: &Arc<Self>) {
        let todo: Vec<(String, super::world::Tk, String, String)> = { let w = self.world().await;
            w.tasks.iter().filter(|(_, k)| !k.delivered).filter_map(|(id, k)| {
                let c = w.threads.get(&k.child)?; let ended = matches!(c.turn.as_str(), "completed" | "failed" | "interrupted");
                if !(k.terminal() || ended) { return None }
                Some((id.clone(), k.clone(), c.turn.clone(), c.last_assistant.clone())) }).collect() };
        for (id, k, turn, last) in todo {
            let now = now_s(); let (st, word) = if turn == "completed" { ("done", "finished") } else { ("failed", "failed") };
            let summary = if k.terminal() && k.state == "cancelled" && k.summary.starts_with("cancelled by the") { k.summary.clone() } else { last };
            if !k.terminal() { self.commit_events(vec![json!({"$":"TaskStateSet","task":id,"state":st,"summary":summary,"at":now})]).await }
            let (word, summary) = if k.state == "cancelled" && k.summary == "cancelled by the user" { ("cancelled by the user", String::new()) } else { (word, summary) };
            let text = note(&id, &k.provider, &k.model, word, &summary);
            // the result reaches the parent before TaskDelivered: a step torn by a crash leaves it undelivered, and the next look delivers it once more only if the message is not there
            let m = msg(&k.child, &id);
            let exists = { let w = self.world().await; w.threads.get(&k.parent).map_or(false, |p| p.newest == m) };
            if !exists { let _ = self.deliver_as("system", &k.parent, &m, &text, Some((k.child.clone(), "task".into(), String::new())), vec![]).await; }
            self.commit_events(vec![json!({"$":"TaskDelivered","task":id,"at":now_s()})]).await;
        }
    }
}

/// The once-a-second look (Task.finish for a live end, Task.repair after a restart). It stops when the Agents are dropped.
pub fn watch(ag: &Arc<Agents>) {
    let weak = Arc::downgrade(ag);
    tokio::spawn(async move { loop { tokio::time::sleep(Duration::from_secs(1)).await; let Some(ag) = weak.upgrade() else { break }; ag.tasks_tick().await; } });
}

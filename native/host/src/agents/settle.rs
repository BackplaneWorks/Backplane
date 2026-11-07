//! Settling a thread stops everything it owns, through the paths that already stop each thing (the tools' own cancels), so none of it can report back and
//! start the thread again: watched work and its run/lock rows (work_cancel), locks it holds or waits for, delegated tasks (task_cancel), the threads it
//! launched (interrupt, unreported), and its wake subscriptions (wake_cancel). The running turn and the agent process (with its background shells) are
//! stopped by the caller (commands/mod.rs `plan.stop`, or `tool_settle`). The same is done for what those threads own in turn.
use super::{bg, Agents};
use serde_json::json;
use std::{collections::HashSet, sync::Arc};

impl Agents {
    /// Everything `th` owns, and what its tasks and launched threads own, stops. Call it after ThreadSettled is logged: a settled thread files the reports that
    /// the stopping causes (agents/mailroom.rs) instead of starting a turn for them.
    pub async fn settle_work(self: &Arc<Self>, th: &str) {
        if crate::copy::on() { return }
        let (mut todo, mut seen) = (vec![th.to_string()], HashSet::new());
        while let Some(t) = todo.pop() {
            if !seen.insert(t.clone()) { continue }
            todo.extend(self.stop_owned(&t).await);
        }
    }
    /// One thread's own work; the threads it handed work to are returned.
    async fn stop_owned(self: &Arc<Self>, th: &str) -> Vec<String> {
        // wakes first: nothing it waited for may fire into it while the rest stops
        for set in self.tool_sets() {
            if !set.owns("wake_list") { continue }
            let (_, l) = set.call(th, "wake_list", &json!({})).await;
            for id in l["wakes"].as_array().into_iter().flatten().filter_map(|w| w["id"].as_str()) { let _ = set.call(th, "wake_cancel", &json!({"id":id})).await; }
        }
        // watched work (commands, pids, files, queued behind a lock)
        for l in self.rows_of(th).await.iter().filter(|l| !bg::ended(l)) { let _ = self.tool_work_cancel(th, &json!({"id":bg::field(l, 0)})).await; }
        self.locks_drop(th).await;
        // delegated tasks, and the threads it launched
        let (tasks, launched): (Vec<(String, String)>, Vec<String>) = { let w = self.world().await;
            (w.tasks.iter().filter(|(_, k)| k.parent == th && !k.terminal()).map(|(i, k)| (i.clone(), k.child.clone())).collect(),
             w.launched.iter().filter(|(c, (by, _))| by == th && w.threads.get(*c).map_or(false, |t| !t.archived && !t.deleted)).map(|(c, _)| c.clone()).collect()) };
        let mut kids = vec![];
        for (id, child) in tasks { let _ = self.tool_task_cancel(th, &json!({"taskId":id})).await; kids.push(child) }
        for c in launched {
            if self.world().await.running(&c) {
                self.stopped_by.lock().unwrap().insert(c.clone(), th.to_string()); // the launcher is not told
                let r = self.interrupt(&c).await; if let Some(m) = self.mailroom() { m.stopping(&c).await }
                if r.is_err() { self.stopped_by.lock().unwrap().remove(&c); }
            }
            kids.push(c);
        }
        kids
    }
}

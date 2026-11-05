//! thread_launch (mcp.bend Mcp.launch): a continuing subthread of the caller (or a top-level thread with `subthread:false`) with its first message.
//! Not done here: the launcher's reports when the child's turn ends (hub.bend Launch.tell / Lr.*), a worktree for the new thread (Hub.new.env "auto"), the depth/chain
//! check beyond "same project, launcher alive" (ST.Tree.chain). They are listed in PARITY.md.
use super::{hub_id, lines::s, now_s, rules, Agents};
use serde_json::{json, Value};
use std::sync::Arc;

fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }

impl Agents {
    pub async fn tool_launch(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let prompt = s(a, "prompt").to_string();
        let (caller, proj, default_prov, default_model, scratch_ok) = { let w = self.world().await;
            let c = w.threads.get(thread).cloned();
            let asked = s(a, "project");
            let proj = if !asked.is_empty() { w.project_find(asked) } else { c.as_ref().map(|c| c.project.clone()) };
            (c, proj, w.setting("agent.provider").to_string(), w.setting("agent.model").to_string(), asked.trim().eq_ignore_ascii_case("scratch")) };
        if scratch_ok { return fail("Scratch is retired: name a project (machine_list), or ask the uberbot") }
        let Some(proj) = proj.filter(|p| !p.is_empty()) else { return fail("name an existing project (project_list shows them)") };
        let parent = if a.get("subthread").and_then(Value::as_bool).unwrap_or(true) && caller.is_some() { thread.to_string() } else { String::new() };
        if !parent.is_empty() { let ok = caller.as_ref().map_or(false, |c| !c.archived && !c.deleted && c.project == proj); if !ok { return fail("A subthread needs its launcher in the same project (maximum depth 64). Use subthread false only for an explicitly requested top-level conversation.") } }
        if prompt.trim().is_empty() { return fail("a prompt is needed") }
        let provider = if s(a, "provider").is_empty() { if default_prov.is_empty() { "claude".to_string() } else { default_prov } } else { s(a, "provider").to_string() };
        let model = if !s(a, "model").is_empty() { s(a, "model").to_string() } else if s(a, "provider").is_empty() { default_model } else { String::new() };
        if provider != "claude" && super::providers::driver(&provider).is_none() { return fail(&format!("{provider} threads are not run by the Rust hub yet")) }
        let (now, report) = (now_s(), a.get("report").and_then(Value::as_bool).unwrap_or(true));
        let seq = self.seq_n(); let th = hub_id("t", now, seq);
        let title = if s(a, "title").is_empty() { rules::title_of(&prompt) } else { s(a, "title").to_string() };
        let by = if parent.is_empty() { String::new() } else { thread.to_string() };
        let mut ev = vec![json!({"$":"ThreadCreated","id":th,"project":proj,"title":title,"env":"local","branch":"","worktree":"","provider":provider,"model":model,"at":now})];
        if !by.is_empty() { ev.push(json!({"$":"ThreadLaunched","id":th,"by":by,"report":report,"at":now})) }
        if !parent.is_empty() { ev.push(json!({"$":"ThreadParentSet","id":th,"parent":parent,"at":now})); ev.push(json!({"$":"SettingSet","key":format!("thread.tree.{parent}"),"value":"1","at":now})) }
        let mid = hub_id("m", now, seq + 1);
        ev.push(json!({"$":"SettingSet","key":format!("autosettle.seed.{th}"),"value":mid,"at":now}));
        self.commit_events(ev).await;
        if let Err(e) = self.deliver_as("user", &th, &mid, &prompt, None, vec![]).await { return fail(&format!("could not start the thread: {e}")) }
        let note = if report && !by.is_empty() { "Its reports will come to you as messages when its turn ends or it asks the person something. End your turn instead of polling." } else { "" };
        (true, json!({"threadId":th,"parent":parent,"title":title,"provider":provider,"state":"started","note":note}))
    }
}

//! thread_launch: a continuing subthread of the caller (or a top-level thread with `subthread:false`) with its first message.
//! The launcher's reports are `launch_tell`/`report_plan` below. Not done here: reports to a launcher on another machine (Lr.far), a worktree for the new thread (Hub.new.env "auto"), the depth/chain
//! check beyond "same project, launcher alive" (ST.Tree.chain). They are listed in PARITY.md.
use super::{hub_id, lines::s, now_s, rules, Agents};
use serde_json::{json, Value};
use std::sync::Arc;

fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }

impl Agents {
    pub async fn tool_launch(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let launcher = match self.mailroom() { Some(m) if !crate::net::far::is(thread) => m.launcher(thread).await, _ => thread.to_string() }; let thread = launcher.as_str(); // a back office launches as its owner
        let prompt = s(a, "prompt").to_string();
        // `_launch`: the id a linked machine gave a launch it held while this one was away and may post again (its answer lost): started once
        let once = s(a, "_launch"); let once = (crate::net::far::is(thread) && !once.is_empty() && once.len() <= 64 && once.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))).then(|| format!("far.launched.{once}"));
        if let Some(k) = &once { let had = self.world().await.setting(k).to_string(); if !had.is_empty() { return (true, json!({"threadId":had,"state":"started","note":"started already"})) } }
        let (caller, proj, default_prov, default_model, scratch_ok) = { let w = self.world().await;
            let c = w.threads.get(thread).cloned();
            let asked = s(a, "project");
            let proj = if !asked.is_empty() { w.project_find(asked).or_else(|| crate::net::registry::local_id(asked)) } else { c.as_ref().map(|c| c.project.clone()) };
            (c, proj, w.setting("agent.provider").to_string(), w.setting("agent.model").to_string(), asked.trim().eq_ignore_ascii_case("scratch")) };
        if scratch_ok { return fail("Scratch is retired: name a project (machine_list), or ask the uberbot") }
        let Some(proj) = proj.filter(|p| !p.is_empty()) else { return fail("name an existing project (project_list shows them)") };
        // Mcp.launch.parent: a bot works across the person's projects and has no local parent; it hears from what it launched all the same
        let far_by = crate::net::far::is(thread); // a launcher on another machine has no thread here; it hears from what it launched through launch_report
        let as_bot = match self.mailroom() { Some(m) if !far_by => m.reach(thread, "").await == Some(true), _ => false };
        let parent = if a.get("subthread").and_then(Value::as_bool).unwrap_or(true) && caller.is_some() && !as_bot { thread.to_string() } else { String::new() };
        if !parent.is_empty() { let ok = caller.as_ref().map_or(false, |c| !c.archived && !c.deleted && c.project == proj); if !ok { return fail("A subthread needs its launcher in the same project (maximum depth 64). Use subthread false only for an explicitly requested top-level conversation.") } }
        if !parent.is_empty() && !self.world().await.tree_chain_ok(&parent) { return fail("A subthread needs its launcher in the same project (maximum depth 64). Use subthread false only for an explicitly requested top-level conversation.") } // ST.Tree.chain
        if prompt.trim().is_empty() { return fail("a prompt is needed") }
        let default_prov = if default_prov.is_empty() { "claude".to_string() } else { default_prov };
        // an account names its provider: an account of another one than asked for is refused, and with none asked for it decides
        let asked = if s(a, "provider").is_empty() { if a.get("account").is_some() && !s(a, "account").eq_ignore_ascii_case("default") { String::new() } else { default_prov.clone() } } else { s(a, "provider").to_string() };
        let chosen = match self.account_arg(a, &asked).await { Ok(c) => c, Err(e) => return fail(&e) };
        let provider = chosen.as_ref().map(|c| c.provider.clone()).unwrap_or(if asked.is_empty() { default_prov.clone() } else { asked });
        let model = if !s(a, "model").is_empty() { s(a, "model").to_string() } else if s(a, "provider").is_empty() && provider == default_prov { default_model } else { String::new() };
        if provider != "claude" && super::providers::driver(&provider).is_none() { return fail(&format!("{provider} threads are not run by the Rust hub yet")) }
        let (now, report) = (now_s(), a.get("report").and_then(Value::as_bool).unwrap_or(true));
        let seq = self.seq_n();
        // `_id`: a swarm names its member threads before they start (the room lists them first, so the agent's first tools/list has the room's tools)
        let given = s(a, "_id"); let th = if !given.is_empty() && given.len() <= 64 && given.starts_with('t') && given[1..].chars().all(|c| c.is_ascii_digit() || c == '-') && !self.world().await.threads.contains_key(given) { given.to_string() } else { hub_id("t", now, seq) };
        let title = if s(a, "title").is_empty() { rules::title_of(&prompt) } else { s(a, "title").to_string() };
        let by = if caller.is_some() || far_by { thread.to_string() } else { String::new() }; // Mcp.launch.go: the launcher, parent or not
        let mut ev = vec![json!({"$":"ThreadCreated","id":th,"project":proj,"title":title,"env":"local","branch":"","worktree":"","provider":provider,"model":model,"at":now})];
        ev.extend(self.thread_account(thread, &th, &provider, chosen.as_ref(), now).await);
        if !by.is_empty() { ev.push(json!({"$":"ThreadLaunched","id":th,"by":by,"report":report,"at":now})) }
        if !parent.is_empty() { ev.push(json!({"$":"ThreadParentSet","id":th,"parent":parent,"at":now})); ev.push(json!({"$":"SettingSet","key":format!("thread.tree.{parent}"),"value":"1","at":now})) }
        let mid = hub_id("m", now, seq + 1);
        ev.push(json!({"$":"SettingSet","key":format!("autosettle.seed.{th}"),"value":mid,"at":now}));
        if let Some(k) = once { ev.push(json!({"$":"SettingSet","key":k,"value":th,"at":now})) }
        self.commit_events(ev).await;
        if let Err(e) = self.deliver_as("user", &th, &mid, &prompt, None, vec![]).await { return fail(&format!("could not start the thread: {e}")) }
        let note = if report && !by.is_empty() { "Its reports will come to you as messages when its turn ends or it asks the person something. End your turn instead of polling." } else { "" };
        (true, json!({"threadId":th,"parent":parent,"title":title,"provider":provider,"account":chosen.map(|c| c.label),"state":"started","note":note}))
    }
}


// ---- Launch.tell: the launcher hears when a launched thread's turn ends, an ask opens or the agent exits ----
/// Lr.loops: launcher-started turns in a row before the reports stop.
pub const LOOPS: u64 = 20;
/// A report to send: what the launcher is told, and the settings it moves first.
#[derive(Debug, PartialEq)]
pub struct Report { pub to: String, pub text: String, pub before: Vec<Value> }

fn head(id: &str, title: &str) -> String { format!("[Backplane] Launched thread \"{title}\" ({id})") }
/// Lr.word
pub fn word(e: u8, reply: &str) -> &'static str { match e { 1 => if reply.trim().ends_with('?') { "finished its turn and waits for an answer" } else { "completed" }, 2 => "failed", _ => "was interrupted" } }
/// Lr.note: the state and the last reply, whole.
pub fn note(id: &str, title: &str, word: &str, reply: &str) -> String { format!("{} {}{}", head(id, title), word, if reply.trim().is_empty() { ".".to_string() } else { format!(":\n\n{reply}") }) }
/// A failed thread's report: why it failed (the error it ended on, plain words), then its last reply under it.
pub fn with_why(why: &str, reply: &str) -> String { let why = why.trim(); if reply.trim().is_empty() { why.to_string() } else { format!("{why}\n\nIts last reply:\n\n{reply}") } }
/// Lr.ask.note
pub fn ask_note(id: &str, title: &str, ask: &str) -> String { format!("{} needs the person: {}\nIt waits until they answer in that thread.", head(id, title), ask) }
/// Lr.loop.note
pub fn loop_note(id: &str) -> String { format!("\n\nReports from this thread stop here: it went back and forth with you {LOOPS} times with no word from the person. thread_update {{threadId: {id}, report: true}} starts them again.") }

/// Launch.tell over a step's events about `thread` (`from`: the thread whose call made the step, "" for the agent's own line or the person's Stop): None when nothing is owed.
/// An ask opened is told at once; otherwise the last turn change of the thread (completed 1, failed 2, interrupted 3; running or none: nothing). Exceptions: reports off, a launcher that
/// stopped the thread itself, a completed turn that left work out of hand, a thread that already wrote to its launcher this turn. The launcher-started turn counter moves; at Lr.loops() the reports stop.
pub fn report_plan(w: &super::world::World, thread: &str, events: &[Value], from: &str, now: u64) -> Option<Report> { let by = w.launched.get(thread)?.0.clone(); report_plan_to(w, thread, events, from, now, &by, false) }
/// `to`: Lr.dest of the launcher (a launcher elsewhere is not reached: Lr.far); `coordinated`: `to` is one of the uberbot's threads, and an ask reaches it through triage instead (Lr.ask.send)
pub fn report_plan_to(w: &super::world::World, thread: &str, events: &[Value], from: &str, now: u64, to: &str, coordinated: bool) -> Option<Report> {
    let (by, on) = w.launched.get(thread).cloned()?; if !on || by.is_empty() { return None }
    let to = to.to_string();
    if to == thread || (!crate::net::far::is(&to) && !w.threads.contains_key(&to)) { return None }
    if !from.is_empty() && from == by { return None } // a launcher stopping its own thread is not told
    let ask = events.iter().find(|e| e["$"] == "AskOpened" && e["thread"] == thread).map(|e| {
        let (k, tool, d) = (e["kind"].as_str().unwrap_or(""), e["tool"].as_str().unwrap_or(""), e["detail"].as_str().unwrap_or(""));
        format!("{}{}", if tool.is_empty() { k.to_string() } else { format!("{k} ({tool})") }, if d.is_empty() { String::new() } else { format!(": {}", d.chars().take(600).collect::<String>()) }) });
    let title = w.threads.get(thread).map(|t| t.title.clone()).unwrap_or_default();
    if ask.is_some() && coordinated { return None }
    if let Some(a) = ask { return Some(Report { to, text: ask_note(thread, &title, &a), before: vec![setting(format!("autosettle.report.{thread}"), now.to_string(), now)] }) }
    let e = match events.iter().rev().find(|e| e["$"] == "TurnChanged" && e["thread"] == thread).filter(|e| e["failover"] != true).and_then(|e| e["state"].as_str()) { Some("completed") => 1u8, Some("failed") => 2, Some("interrupted") => 3, _ => return None };
    let busy = w.work(thread).0 + w.tasks.values().filter(|k| k.parent == thread && !k.terminal()).count();
    if e == 1 && busy > 0 { return None } // launch_waits_for_work
    if w.said_to.get(thread).map_or(false, |s| s.contains(&to)) { return None } // launch_said_quiet
    let reply = w.threads.get(thread).map(|t| t.last_assistant.clone()).unwrap_or_default();
    // a failure says why first, in plain words (a halt card's: out of memory, out of context), then the last reply
    let why = events.iter().rev().find(|x| x["$"] == "ActivityLogged" && x["thread"] == thread && x["tone"] == "error").and_then(|x| x["summary"].as_str()).map(str::to_string)
        .or_else(|| w.threads.get(thread).map(|t| t.why.clone())).unwrap_or_default();
    let reply = if e == 2 && !why.trim().is_empty() { with_why(&why, &reply) } else { reply };
    let own = !by.is_empty() && w.threads.get(thread).map_or(false, |t| t.last_user.strip_prefix("t:").and_then(|r| r.split(':').next()) == Some(by.as_str()));
    let prev: u64 = w.setting(&format!("launch.n.{thread}")).parse().unwrap_or(0);
    let n = if own { prev + 1 } else { 0 };
    let mut text = note(thread, &title, word(e, &reply), &reply); let mut before = vec![setting(format!("autosettle.report.{thread}"), now.to_string(), now)];
    if n >= LOOPS { before.insert(0, json!({"$":"ThreadLaunched","id":thread,"by":by,"report":false,"at":now})); before.insert(0, setting(format!("launch.n.{thread}"), "0".into(), now)); text.push_str(&loop_note(thread)) }
    else if n != prev { before.insert(0, setting(format!("launch.n.{thread}"), n.to_string(), now)) }
    Some(Report { to, text, before })
}
fn setting(k: String, v: String, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }

impl Agents {
    /// Launch.tell for the events just committed about `thread`: the report goes to the launcher as a message from the thread (kind "launch"), one per end.
    pub async fn launch_tell(self: &Arc<Self>, thread: &str, events: &[Value], from: &str) {
        let by = { let w = self.world().await; w.launched.get(thread).map(|x| x.0.clone()) }; let Some(by) = by else { return };
        let far_by = crate::net::far::is(&by);
        let (to, coordinated) = if far_by { (by.clone(), false) } else { match self.mailroom() { Some(m) => { let to = m.dest(&by).await; let c = m.chat_is(&to).await; (to, c) } None => (by, false) } };
        let plan = { let w = self.world().await; report_plan_to(&w, thread, events, from, now_s(), &to, coordinated) };
        let Some(r) = plan else { return };
        if far_by { // the launcher is on another machine: its settings move here, the report goes there
            if !r.before.is_empty() { self.commit_events(r.before.clone()).await }
            self.send_far_report(thread, &r.to, &r.text); return
        }
        let msg = format!("t:{thread}:lr-{}", hub_id("r", now_s(), self.seq_n()));
        let _ = self.inbound("user", &r.to, &msg, &r.text, (thread.to_string(), "launch".into(), String::new()), r.before).await;
    }
}

#[cfg(test)]
mod laws {
    use super::*;
    use crate::agents::world::World;
    fn demo(report: bool, own: bool, busy: bool, more: &[&str]) -> World {
        let mut w = World::default();
        let m1 = if own { "t:t0:m1" } else { "m1" };
        let mut ls: Vec<String> = vec!["{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"p\",\"root\":\"/p\",\"at\":1}".into(),
            "{\"$\":\"ThreadCreated\",\"id\":\"t0\",\"project\":\"p1\",\"title\":\"boss\",\"provider\":\"claude\",\"model\":\"\",\"at\":1}".into(),
            "{\"$\":\"ThreadCreated\",\"id\":\"t1\",\"project\":\"p1\",\"title\":\"work\",\"provider\":\"claude\",\"model\":\"\",\"at\":1}".into(),
            format!("{{\"$\":\"ThreadLaunched\",\"id\":\"t1\",\"by\":\"t0\",\"report\":{report},\"at\":1}}"),
            format!("{{\"$\":\"MessagePosted\",\"thread\":\"t1\",\"msg\":\"{m1}\",\"role\":\"user\",\"text\":\"go\",\"at\":1}}"),
            "{\"$\":\"TurnChanged\",\"thread\":\"t1\",\"state\":\"running\",\"at\":1}".into(),
            "{\"$\":\"MessagePosted\",\"thread\":\"t1\",\"msg\":\"a1\",\"role\":\"assistant\",\"text\":\"done it\",\"at\":2}".into()];
        if busy { ls.push("{\"$\":\"TurnChanged\",\"thread\":\"t0\",\"state\":\"running\",\"at\":2}".into()) }
        for l in ls { w.apply(&l) } for l in more { w.apply(l) } w
    }
    fn tc(st: &str) -> Value { json!({"$":"TurnChanged","thread":"t1","state":st,"at":9}) }
    fn plan(w: &World, evs: &[Value]) -> Option<Report> { report_plan(w, "t1", evs, "", 9) }
    #[test] fn launch_reports_turn_end() { let r = plan(&demo(true, false, false, &[]), &[tc("completed")]).unwrap(); assert_eq!(r.to, "t0"); assert_eq!(r.text, "[Backplane] Launched thread \"work\" (t1) completed:\n\ndone it") }
    #[test] fn launch_ask_at_once() { let r = plan(&demo(true, false, false, &[]), &[json!({"$":"AskOpened","id":"q1","thread":"t1","kind":"question","tool":"AskUserQuestion","detail":"which board?","options":"","at":9})]).unwrap(); assert!(r.text.contains("needs the person: question (AskUserQuestion): which board?")) }
    #[test] fn launch_fail_told() { assert!(plan(&demo(true, false, false, &[]), &[tc("failed")]).unwrap().text.contains(" failed:")) }
    #[test] fn a_failure_is_told_with_its_cause_in_plain_words() {
        let why = "Claude was stopped for memory: claude was using about 412 MB and reached this thread's memory limit (400 MB).";
        let ev = [json!({"$":"ActivityLogged","thread":"t1","act":"oom1","tone":"error","kind":"halt","summary":why,"at":3}), tc("failed")];
        let t = plan(&demo(true, false, false, &[]), &ev).unwrap().text;
        assert!(t.contains(&format!(" failed:\n\n{why}")), "{t}");
        assert_eq!(with_why(" x ", "r"), "x\n\nIts last reply:\n\nr"); assert_eq!(with_why("x", ""), "x");
    }
    #[test] fn launch_once_per_end() { assert!(plan(&demo(true, false, false, &[]), &[json!({"$":"MessagePosted","thread":"t1","msg":"a2","role":"assistant","text":"more","at":9})]).is_none()); assert!(plan(&demo(true, false, false, &[]), &[tc("completed"), tc("running")]).is_none(), "the last change is a start") }
    #[test] fn launch_muted_quiet() { assert!(plan(&demo(false, false, false, &[]), &[tc("completed")]).is_none()) }
    #[test] fn launch_waits_for_work() { let w = demo(true, false, false, &["{\"$\":\"SettingSet\",\"key\":\"bg.t1\",\"value\":\"a\\tx\",\"at\":2}"]); assert!(plan(&w, &[tc("completed")]).is_none()); assert!(plan(&w, &[tc("failed")]).is_some(), "a failure ends it even busy") }
    #[test] fn launch_said_quiet() { let w = demo(true, false, false, &["{\"$\":\"MessagePosted\",\"thread\":\"t0\",\"msg\":\"t:t1:m5\",\"role\":\"user\",\"text\":\"here it is\",\"at\":2}"]); assert!(plan(&w, &[tc("completed")]).is_none()) }
    #[test] fn launch_own_stop_quiet() { let w = demo(true, false, false, &[]); assert!(report_plan(&w, "t1", &[tc("interrupted")], "t0", 9).is_none()); assert!(report_plan(&w, "t1", &[tc("interrupted")], "", 9).unwrap().text.contains("was interrupted")) } // launch_stop_told too
    #[test] fn launch_loop_bounded() {
        let w = demo(true, true, false, &["{\"$\":\"SettingSet\",\"key\":\"launch.n.t1\",\"value\":\"19\",\"at\":1}"]); let r = plan(&w, &[tc("completed")]).unwrap();
        assert!(r.text.contains("Reports from this thread stop here") && r.before.iter().any(|e| e["$"] == "ThreadLaunched" && e["report"] == false), "{r:?}");
        let r2 = plan(&demo(true, true, false, &[]), &[tc("completed")]).unwrap(); assert!(r2.before.iter().any(|e| e["key"] == "launch.n.t1" && e["value"] == "1"));
        let r3 = plan(&demo(true, false, false, &["{\"$\":\"SettingSet\",\"key\":\"launch.n.t1\",\"value\":\"7\",\"at\":1}"]), &[tc("completed")]).unwrap(); assert!(r3.before.iter().any(|e| e["key"] == "launch.n.t1" && e["value"] == "0"), "a person's turn resets the count");
    }
    #[test] fn the_word_follows_a_question() { assert_eq!(word(1, "ok?"), "finished its turn and waits for an answer"); assert_eq!(word(1, "ok"), "completed"); assert_eq!(word(3, ""), "was interrupted") }
}

//! The MCP tools that act on threads (mcp.bend: Mcp.list/read/send/wait/interrupt/update/caps), and answering an ask.
use super::{lines::hub_id, mcp, now_s, Agents};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

fn sv<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }
/// Wait.clamp: how many seconds a thread_wait lasts: at least one, at most ten minutes
pub fn wait_clamp(ms: u64) -> u64 { if ms < 1000 { 1 } else if ms > 600000 { 600 } else { ms / 1000 } }
const NOT_HERE: &str = "that thread is not in this project";

impl Agents {
    pub fn seq_n(&self) -> u64 { self.seq() }
    pub fn gen_of(&self, thread: &str) -> Option<String> { self.gens.lock().unwrap().get(thread).cloned() }
    pub fn set_gen(&self, thread: &str, gen: &str) { self.gens.lock().unwrap().insert(thread.into(), gen.into()); }
    pub async fn commit_events(&self, ev: Vec<Value>) { if let Err(e) = self.hub.commit_many(&ev).await { self.stats.commit_errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed); eprintln!("agents: could not write {} events: {e:#}", ev.len()) } }

    async fn same_project(&self, a: &str, b: &str) -> bool { let w = self.world().await; match (w.threads.get(a), w.threads.get(b)) { (Some(x), Some(y)) => !y.deleted && x.project == y.project, _ => false } }

    /// Mcp.phase: approval > input > by the turn (tasks and owned work are not ported).
    async fn phase(&self, th: &str, turn: &str, pending: u64) -> &'static str {
        let w = self.world().await; let on = |k: &str| w.asks.values().any(|a| a.thread == th && a.kind == k); let busy = w.work(th).0;
        if pending > 0 || on("approval") { "approval" } else if on("input") || on("plan") || on("browser") { "input" }
        else { match turn { "running" => "working", "failed" => "failed", _ if busy > 0 => "monitoring", "completed" => "complete", "interrupted" => "stopped", _ => "idle" } }
    }
    /// Mcp.thread_json.w (without `parent`: the JS binary the fixtures come from does not write it)
    async fn thread_json(&self, id: &str) -> Option<Value> {
        let m = self.hub.model().await; let t = m.threads.get(id)?.clone(); drop(m);
        let phase = self.phase(id, &t.turn, t.pending).await;
        let parent = { let w = self.world().await; let p = w.setting(&format!("thread.parent.{id}")).to_string(); // Tree.raw: a link to a live thread of the same project, never to itself
            if !p.is_empty() && p != id && w.threads.get(&p).map_or(false, |x| !x.deleted && x.project == t.project) && w.threads.get(id).map_or(false, |x| !x.deleted) { p } else { String::new() } };
        let (busy, titles) = self.world().await.work(id);
        Some(json!({"id":id,"project":t.project,"title":t.title,"provider":t.provider,"model":t.model,"turn":t.turn,"activity":t.activity,"parent":parent,"phase":phase,"waiting":busy,"work":titles}))
    }

    pub async fn tool_caps(&self, thread: &str) -> (bool, Value) {
        let me = self.thread_json(thread).await.map(|mut v| { v.as_object_mut().map(|o| { o.remove("parent"); o.remove("phase"); o.remove("waiting"); o.remove("work"); }); v }).unwrap_or(Value::Null);
        (true, json!({"providers":self.host.models,"maxDepth":3,"thread":me,"depth":0}))
    }
    pub async fn tool_list(&self, thread: &str) -> (bool, Value) {
        let project = { let w = self.world().await; match w.threads.get(thread) { Some(t) => t.project.clone(), None => return fail(NOT_HERE) } };
        let mut ids: Vec<String> = { let m = self.hub.model().await; m.threads.iter().filter(|(_, t)| t.project == project && !t.deleted).map(|(k, _)| k.clone()).collect() };
        ids.sort(); let mut out = vec![]; for id in ids { if let Some(j) = self.thread_json(&id).await { out.push(j) } }
        (true, json!({"threads":out}))
    }
    pub async fn tool_read(&self, thread: &str, a: &Value) -> (bool, Value) {
        let target = sv(a, "threadId"); if !self.same_project(thread, target).await { return fail(NOT_HERE) }
        let limit = a.get("limit").and_then(|l| l.as_u64().or_else(|| l.as_str().and_then(|s| s.parse().ok()))).unwrap_or(40).min(1000) as usize;
        let mut rows: Vec<crate::model::Row> = vec![]; let mut before = 0u64;
        while rows.len() < limit {
            let page = { let m = self.hub.model().await; match m.page(&self.hub.store, target, before, 32) { Ok(p) => p, Err(_) => break } };
            if page.rows.is_empty() { break }
            let oldest = page.rows.iter().map(|r| r.ord).min().unwrap_or(0); rows.extend(page.rows.into_iter().filter(|r| before == 0 || r.ord < before));
            if oldest <= 1 || page.below == 0 { break } before = oldest;
        }
        rows.sort_by_key(|r| r.seq); rows.dedup_by_key(|r| r.seq); let skip = rows.len().saturating_sub(limit);
        let entries: Vec<Value> = rows.into_iter().skip(skip).map(|r| if r.kind == "msg" { json!({"kind":"message","role":r.role,"text":r.text,"at":r.at}) } else { json!({"kind":"activity","tone":r.role,"tool":r.akind,"summary":r.text,"at":r.at}) }).collect();
        (true, json!({"threadId":target,"status":self.thread_json(target).await.unwrap_or(Value::Null),"entries":entries}))
    }
    pub async fn tool_send(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let target = sv(a, "threadId"); let text = if sv(a, "text").trim().is_empty() { sv(a, "message") } else { sv(a, "text") };
        if text.trim().is_empty() { return fail("thread_send needs a non-empty `text` (the message); nothing was sent") }
        if !self.same_project(thread, target).await { return fail(NOT_HERE) }
        let msg = format!("t:{thread}:{}", hub_id("m", now_s(), self.seq()));
        match self.deliver(target, &msg, text, Some((format!("thread:{thread}"), "thread".into(), String::new())), vec![]).await { Ok(()) => (true, json!({"threadId":target,"state":"sent"})), Err(e) => fail(&e) }
    }
    pub async fn tool_update(&self, thread: &str, a: &Value) -> (bool, Value) {
        let target = if sv(a, "threadId").is_empty() { thread } else { sv(a, "threadId") };
        if !self.same_project(thread, target).await { return fail(NOT_HERE) }
        if a.get("provider").is_some() || a.get("model").is_some() { return fail("changing a thread's provider or model is not available in the Rust hub yet") }
        let title = sv(a, "title").trim().to_string();
        let flag = match a.get("report") { Some(Value::Bool(b)) => Some(*b), Some(Value::String(v)) if v == "true" => Some(true), Some(Value::String(v)) if v == "false" => Some(false), _ => None };
        if let (Some(on), true) = (flag, title.is_empty()) { // Mcp.report.k: a launched thread's reports turned off or on (its launcher stays)
            let by = self.world().await.launched.get(target).map(|x| x.0.clone()).unwrap_or_default();
            if by.is_empty() { return fail("nobody launched that thread, so it has no reports to change") }
            self.commit_events(vec![json!({"$":"ThreadLaunched","id":target,"by":by,"report":on,"at":now_s()})]).await;
            return (true, json!({"threadId":target,"report":on}))
        } if title.is_empty() { return fail("writing a title is not available in the Rust hub yet; give the new title") }
        self.commit_events(vec![json!({"$":"ThreadRenamed","id":target,"title":title,"at":now_s()})]).await;
        (true, json!({"threadId":target,"title":title}))
    }
    pub async fn tool_interrupt(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let target = sv(a, "threadId"); if !self.same_project(thread, target).await { return fail(NOT_HERE) }
        if !self.world().await.running(target) { return (true, json!({"threadId":target,"state":"not running"})) }
        self.stopped_by.lock().unwrap().insert(target.to_string(), thread.to_string()); // Launch.tell: a launcher that stops its own thread is not told
        let r = self.interrupt(target).await; match r { Ok(()) => (true, json!({"threadId":target,"state":"interrupted"})), Err(e) => { self.stopped_by.lock().unwrap().remove(target); fail(&e) } } // an interrupt that failed leaves no mark: the thread's next real end is reported
    }
    pub async fn tool_wait(&self, thread: &str, a: &Value) -> (bool, Value) {
        let target = sv(a, "threadId"); if !self.same_project(thread, target).await { return fail(NOT_HERE) }
        let end = tokio::time::Instant::now() + Duration::from_secs(wait_clamp(a.get("timeoutMs").and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))).unwrap_or(30000)));
        loop {
            let (running, (busy, titles)) = { let w = self.world().await; (w.running(target), w.work(target)) }; let done = !running && busy == 0; // Mcp.done.w: nothing out of hand either
            if done || tokio::time::Instant::now() >= end {
                let last = self.last_reply(target).await; let turn = self.hub.model().await.threads.get(target).map(|t| t.turn.clone()).unwrap_or_default();
                return (true, json!({"threadId":target,"turn":turn,"done":done,"waiting":busy,"work":titles,"lastReply":last.chars().take(8000).collect::<String>()}))
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
    async fn last_reply(&self, target: &str) -> String {
        let page = { let m = self.hub.model().await; m.page(&self.hub.store, target, 0, 32).ok() };
        page.and_then(|p| p.rows.into_iter().filter(|r| r.kind == "msg" && r.role == "assistant" && r.thread == target).max_by_key(|r| r.seq)).map(|r| r.text).unwrap_or_default()
    }

    /// Hub.answer: the person answers an ask. It closes once; a parked agent request gets its reply; a plan answered "implement" leaves plan mode and asks for the work.
    pub async fn answer(self: &Arc<Self>, ask: &str, answer: &str) -> Result<(), String> {
        let a = { let w = self.world().await; w.asks.get(ask).cloned() };
        let Some(a) = a else { return Ok(()) }; // ask_closes_once: an ask already closed (or never opened) changes nothing
        if a.kind == "approval" && !["accept", "session", "decline", "deny"].contains(&answer) { return Err(format!("\"{answer}\" is not an answer to an approval: use accept, session or decline")) } // not silently a decline: the ask stays open
        if a.kind == "browser" && a.tool == "task_pause" { if let Some(d) = crate::term::driver::get(&self.hub) { d.answered(ask, answer).await; } } // the paused task tool waits for this answer
        if crate::files::rv::checks_answer(&self.hub.cfg.home.to_string_lossy(), ask, answer) { // Checks.answer.k: a project's own checks ask closes with no "answer" activity
            let pend = self.pending_ev(&a.thread, 0, ask, now_s()).await; self.commit_events(vec![json!({"$":"AskClosed","id":ask,"answer":answer,"at":now_s()}), pend]).await;
            if let Some(p) = self.parked.lock().unwrap().remove(ask) { let _ = p.tx.send(answer.to_string()); }
            return Ok(())
        }
        let (now, th) = (now_s(), a.thread.clone());
        if a.tool == crate::settings::proposal::TOOL { return { let open: Vec<String> = self.world().await.asks.iter().filter(|(_, k)| k.tool == crate::settings::proposal::TOOL).map(|(i, _)| i.clone()).collect(); crate::settings::proposal::answer_ask(self, ask, &th, &a.options, answer, now, &open).await } } // settings lane: Settings.answer
        let mut ev = vec![json!({"$":"AskClosed","id":ask,"answer":answer,"at":now})];
        let implement = a.kind == "plan" && answer == "implement";
        if implement { let t = { let w = self.world().await; w.threads.get(&th).cloned() }; if let Some(t) = t { ev.push(json!({"$":"ModesSet","id":th,"runtime":t.runtime,"interaction":"default","model":t.model,"at":now})) } }
        else if !th.is_empty() && (a.kind == "input" || a.kind == "approval") { ev.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("k", now, self.seq()),"tone":"info","kind":"answer","summary":mcp::ans_line(&a.kind, &a.tool, &a.detail, answer),"at":now})) }
        let pend = self.pending_ev(&th, 0, ask, now).await; ev.push(pend);
        self.commit_events(ev).await;
        if let Some(p) = self.parked.lock().unwrap().remove(ask) { let _ = p.tx.send(answer.to_string()); }
        let th_of = self.driver_asks.lock().unwrap().remove(ask); if let Some(t) = th_of { let _ = self.actor(&t).try_send(super::Cmd::Answer { ask: ask.to_string(), answer: answer.to_string() }); }
        if implement { return self.deliver(&th, &hub_id("m", now, self.seq()), "Implement the proposed plan.", None, vec![]).await }
        Ok(())
    }
}

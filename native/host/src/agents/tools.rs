//! The MCP tools that act on threads, and answering an ask.
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

    async fn same_project(&self, a: &str, b: &str) -> bool { if crate::net::far::is(a) { return true } /* a thread of another of the owner's machines (net/rpc.rs vouched for the link) */ if let Some(m) = self.mailroom() { if let Some(v) = m.reach(a, b).await { return v } } let w = self.world().await; match (w.threads.get(a), w.threads.get(b)) { (Some(x), Some(y)) => !y.deleted && x.project == y.project, _ => false } }

    /// Mcp.phase: approval > input > by the turn (tasks and owned work are not ported).
    async fn phase(&self, th: &str, turn: &str, pending: u64) -> &'static str {
        let w = self.world().await; let on = |k: &str| w.asks.values().any(|a| a.thread == th && a.kind == k); let busy = w.work(th).0;
        if pending > 0 || on("approval") { "approval" } else if on("input") || on("plan") || on("browser") { "input" }
        else { match turn { "running" => "working", "failed" => "failed", _ if busy > 0 => "monitoring", "completed" => "complete", "interrupted" => "stopped", _ => "idle" } }
    }
    /// Mcp.thread_json.w (without `parent`: the JS binary the fixtures come from does not write it)
    pub(super) async fn thread_json(&self, id: &str) -> Option<Value> {
        let m = self.hub.model().await; let t = m.threads.get(id)?.clone(); drop(m);
        let phase = self.phase(id, &t.turn, t.pending).await;
        let parent = { let w = self.world().await; let p = w.tree_raw(id); // Tree.raw: a link to a live thread of the same project, never to itself
            if !p.is_empty() && p != id && w.threads.get(&p).map_or(false, |x| !x.deleted && x.project == t.project) && w.threads.get(id).map_or(false, |x| !x.deleted) { p } else { String::new() } };
        let (busy, titles, model) = { let w = self.world().await; let (b, ti) = w.work(id); (b, ti, w.threads.get(id).map_or(t.model.clone(), |x| x.model.clone())) }; // the model as the agents run it: a model picked alone is a ModesSet, which the hub's model does not fold
        Some(json!({"id":id,"project":t.project,"title":t.title,"provider":t.provider,"model":model,"turn":t.turn,"activity":t.activity,"parent":parent,"phase":phase,"waiting":busy,"work":titles}))
    }

    pub async fn tool_caps(&self, thread: &str) -> (bool, Value) {
        let me = self.thread_json(thread).await.map(|mut v| { v.as_object_mut().map(|o| { o.remove("parent"); o.remove("phase"); o.remove("waiting"); o.remove("work"); }); v }).unwrap_or(Value::Null);
        (true, json!({"providers":self.host.models,"maxDepth":3,"thread":me,"depth":0}))
    }
    pub async fn tool_list(&self, thread: &str) -> (bool, Value) {
        let project = { let w = self.world().await; match w.threads.get(thread) { Some(t) => t.project.clone(), None => return fail(NOT_HERE) } };
        let room = self.mailroom(); let bot = match &room { Some(m) => m.reach(thread, thread).await == Some(true), None => false }; // Mcp.scope "*": a bot sees every project's threads
        let mut ids: Vec<String> = { let m = self.hub.model().await; m.threads.iter().filter(|(_, t)| (if bot { !t.project.is_empty() } else { t.project == project }) && !t.deleted).map(|(k, _)| k.clone()).collect() };
        ids.sort(); let mut out = vec![];
        for id in ids {
            if let Some(m) = &room { if m.hidden(thread, &id).await { continue } } // Mcp.run.threads
            if let Some(j) = self.thread_json(&id).await { out.push(j) }
        }
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
        let to_uber = match self.mailroom() { Some(m) => m.chat_is(target).await, None => false }; // Lr.may: any thread may write to the uberbot's own
        if !to_uber && !self.same_project(thread, target).await { return fail(NOT_HERE) }
        // `_msg`: the id a linked machine gave a send it may post again (its answer lost): the same message, taken once (Hub.dup)
        let given = sv(a, "_msg"); let msg = if !given.is_empty() && given.len() <= 64 && given.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) { format!("t:{thread}:{given}") } else { format!("t:{thread}:{}", hub_id("m", now_s(), self.seq())) };
        match self.inbound("user", target, &msg, text, (format!("thread:{thread}"), "thread".into(), String::new()), vec![]).await { Ok(()) => (true, json!({"threadId":target,"state":"sent"})), Err(e) => fail(&e) }
    }
    pub async fn tool_update(&self, thread: &str, a: &Value) -> (bool, Value) {
        let target = if sv(a, "threadId").is_empty() { thread } else { sv(a, "threadId") };
        if crate::net::far::is(target) { return fail("name the thread (threadId)") }
        if !self.same_project(thread, target).await { return fail(NOT_HERE) }
        if a.get("provider").is_some() || a.get("model").is_some() { return self.tool_update_provider(thread, target, a).await }
        if a.get("account").is_some() { if a.get("title").is_some() || a.get("report").is_some() || a.get("parent").is_some() { return fail("account cannot be combined with title, report or parent") } return self.tool_update_account(thread, target, a).await }
        // pinned: the sidebar's Pin and Unpin (thread.pin / thread.unpin); alone, like account
        if let Some(pv) = a.get("pinned") {
            let on = match pv { Value::Bool(b) => *b, Value::String(v) if v == "true" => true, Value::String(v) if v == "false" => false, _ => return fail("pinned is true or false") };
            if a.get("title").is_some() || a.get("report").is_some() || a.get("parent").is_some() { return fail("pinned cannot be combined with title, report or parent") }
            let (was, keyed) = pin_state(target);
            if was != on { let now = now_s(); let mut ev = vec![json!({"$":"ThreadPinned","id":target,"pinned":on,"at":now})];
                if on && keyed { ev.push(json!({"$":"ThreadOrdered","id":target,"section":"pinned","key":"","at":now})) }
                self.commit_events(ev).await; }
            return (true, json!({"threadId":target,"pinned":on}))
        }
        // parent alone: Hub.thread_parent, as the sidebar's move (thread.reparent / thread.detach): empty detaches; the title is left alone
        if let Some(pv) = a.get("parent") {
            if a.get("title").is_some() || a.get("report").is_some() { return fail("parent cannot be combined with title or report") }
            let Some(parent) = pv.as_str().map(str::trim) else { return fail("parent is a thread id, or empty for top level") };
            if !parent.is_empty() {
                let w = self.world().await;
                let live = |id: &str| w.threads.get(id).filter(|t| !t.deleted);
                let (Some(t), Some(p)) = (live(target), live(parent)) else { return fail("parent must be a live conversation in the same project") };
                if parent == target || t.project != p.project { return fail("parent must be a live conversation in the same project") }
                let mut cur = parent.to_string(); // the new ancestry never comes back to the thread (at most 64 deep)
                for _ in 0..64 { if cur.is_empty() { break } if cur == target { return fail("that parent would make a cycle") } cur = w.tree_raw(&cur) }
                if !cur.is_empty() { return fail("that parent would make a cycle") }
            }
            let now = now_s(); let mut ev = vec![json!({"$":"ThreadParentSet","id":target,"parent":parent,"at":now})];
            if !parent.is_empty() { ev.push(json!({"$":"SettingSet","key":format!("thread.tree.{parent}"),"value":"1","at":now})) } // its parent opens, so the moved row shows
            self.commit_events(ev).await;
            return (true, json!({"threadId":target,"parent":parent}))
        }
        let title = sv(a, "title").trim().to_string();
        let flag = match a.get("report") { Some(Value::Bool(b)) => Some(*b), Some(Value::String(v)) if v == "true" => Some(true), Some(Value::String(v)) if v == "false" => Some(false), _ => None };
        if let (Some(on), true) = (flag, title.is_empty()) { // Mcp.report.k: a launched thread's reports turned off or on (its launcher stays)
            let by = self.world().await.launched.get(target).map(|x| x.0.clone()).unwrap_or_default();
            if by.is_empty() { return fail("nobody launched that thread, so it has no reports to change") }
            self.commit_events(vec![json!({"$":"ThreadLaunched","id":target,"by":by,"report":on,"at":now_s()})]).await;
            return (true, json!({"threadId":target,"report":on}))
        } if title.is_empty() { // regenerate: the title model writes a new one from the conversation; a title the person typed is kept unless `force` is true
            let force = matches!(a.get("force"), Some(Value::Bool(true)));
            return match self.retitle(target, None, force).await { Ok(t) => (true, json!({"threadId":target,"title":t})), Err(e) => fail(&e) } }
        self.commit_events(vec![json!({"$":"ThreadRenamed","id":target,"title":title,"at":now_s()})]).await;
        (true, json!({"threadId":target,"title":title}))
    }
    /// thread_update {provider?, model?, account?}: the person's own switch (`thread.provider`), between turns. The conversation is kept as a switch
    /// by hand or by the fallback chain keeps it: the new provider gets what it has not seen with the next message. A choice made this way is the
    /// thread's own from then on (the chain's bookkeeping is cleared, so it is not moved back up by itself). Answers the saved status.
    async fn tool_update_provider(&self, thread: &str, target: &str, a: &Value) -> (bool, Value) {
        if a.get("title").is_some() || a.get("report").is_some() || a.get("parent").is_some() || a.get("pinned").is_some() { return fail("provider and model cannot be combined with title, report, parent or pinned") }
        let (from, was_model, running) = { let w = self.world().await; match w.threads.get(target) { Some(t) => (t.provider.clone(), t.model.clone(), w.running(target)), None => return fail("No such thread.") } };
        let to = { let p = sv(a, "provider").trim().to_lowercase(); if p.is_empty() { from.clone() } else { p } };
        if to != "claude" && super::providers::driver(&to).is_none() { return fail(&format!("{to} is not a provider this hub runs (orchestrator_capabilities lists them)")) }
        let model = match a.get("model") { Some(Value::String(m)) => m.trim().to_string(), Some(Value::Null) | None => if to == from { was_model.clone() } else { String::new() }, Some(_) => return fail("model is a string (empty for the provider's default)") };
        if running { return fail(if target == thread { "a thread's provider and model change between turns, and this is its turn: ask from another thread, or the person, once it ends" } else { "a provider changes between turns: that thread is running; thread_interrupt it first, or wait for its turn to end" }) }
        if to != from || model != was_model {
            let Some(h) = self.hub.rpc_for("thread.provider") else { return fail("switching providers is not available") };
            let r = h.call(&self.hub, &crate::api::Caller { gen: "agents".into(), client: 0 }, &json!(0), "thread.provider", &json!({"thread":target,"provider":to,"model":model})).await.into_iter().next().unwrap_or(Value::Null);
            if r["ok"] != true { return fail(r["error"].as_str().unwrap_or("the provider could not be changed")) }
        }
        let mut out = self.thread_json(target).await.unwrap_or(Value::Null); out["threadId"] = json!(target);
        if a.get("account").is_some() { // the account is the new provider's, chosen as thread_update {account} alone chooses one
            let (ok, v) = self.tool_update_account(thread, target, a).await;
            if !ok { let e = v["error"].as_str().unwrap_or("the account could not be selected"); return fail(&format!("now on {to}{}, but the account was not changed: {e}", if model.is_empty() { String::new() } else { format!(" ({model})") })) }
            out["account"] = v["account"].clone();
        }
        (true, out)
    }
    pub async fn tool_interrupt(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let target = sv(a, "threadId"); if !self.same_project(thread, target).await { return fail(NOT_HERE) }
        if !self.world().await.running(target) { return (true, json!({"threadId":target,"state":"not running"})) }
        self.stopped_by.lock().unwrap().insert(target.to_string(), thread.to_string()); // Launch.tell: a launcher that stops its own thread is not told
        let r = self.interrupt(target).await; if let Some(m) = self.mailroom() { m.stopping(target).await } /* Run.queue.hold */ match r { Ok(()) => (true, json!({"threadId":target,"state":"interrupted"})), Err(e) => { self.stopped_by.lock().unwrap().remove(target); fail(&e) } } // an interrupt that failed leaves no mark: the thread's next real end is reported
    }
    /// The Settle button for an agent: this thread (or another of the project) is settled, as `thread.settle` does.
    pub async fn tool_settle(self: &Arc<Self>, thread: &str, a: &Value) -> (bool, Value) {
        let target = if sv(a, "threadId").is_empty() { thread } else { sv(a, "threadId") };
        if crate::net::far::is(target) { return fail("name the thread (threadId)") }
        if !self.same_project(thread, target).await { return fail(NOT_HERE) }
        if pin_state(target).0 { return fail("the person pinned that thread, so it stays where it is; thread_update {pinned: false} first if they asked to settle it") }
        self.commit_events(vec![json!({"$":"ThreadSettled","id":target,"manual":true,"at":now_s()})]).await;
        // another thread is stopped as the person's Settle stops it (its turn, its agent process); the caller's own turn ends by itself
        if target != thread {
            if self.world().await.running(target) { self.stopped_by.lock().unwrap().insert(target.to_string(), thread.to_string()); let _ = self.interrupt(target).await; if let Some(m) = self.mailroom() { m.stopping(target).await } }
            let _ = self.stop(target).await;
        }
        self.settle_work(target).await;
        (true, json!({"threadId":target,"settled":true}))
    }
    pub async fn tool_wait(&self, thread: &str, a: &Value) -> (bool, Value) {
        let target = sv(a, "threadId"); if !self.same_project(thread, target).await { return fail(NOT_HERE) }
        let end = tokio::time::Instant::now() + if a.get("_now").and_then(Value::as_bool).unwrap_or(false) { Duration::ZERO } else { Duration::from_secs(wait_clamp(a.get("timeoutMs").and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))).unwrap_or(30000))) };
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

    /// Rules.refusal for a tool call by this thread: the rules that apply, the person's newest message in the thread. "" lets it through.
    pub async fn rule_refusal(self: &Arc<Self>, thread: &str, tool: &str) -> String {
        if tool.is_empty() { return String::new() }
        let rules = { let w = self.world().await; let project = w.threads.get(thread).map(|t| t.project.clone()).unwrap_or_default(); super::rules::rules_for(&w.st, w.bot(thread), &project) };
        if !rules.iter().any(|o| o.get("enforce").and_then(Value::as_bool).unwrap_or(false)) { return String::new() }
        let page = { let m = self.hub.model().await; m.page(&self.hub.store, thread, 0, 64).ok() };
        let last = page.and_then(|p| p.rows.into_iter().filter(|r| r.kind == "msg" && r.role == "user" && r.thread == thread).max_by_key(|r| r.seq)).map(|r| r.text).unwrap_or_default();
        super::rules::refusal(&rules, tool, &last)
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
        if a.tool == crate::mcpx::tools::TOOL { return crate::mcpx::tools::answer(self, ask, &th, &a.options, answer).await } // Settings > MCP: an agent's server change, confirmed or not
        if a.tool == crate::settings::proposal::TOOL { return { let open: Vec<String> = self.world().await.asks.iter().filter(|(_, k)| k.tool == crate::settings::proposal::TOOL).map(|(i, _)| i.clone()).collect(); crate::settings::proposal::answer_ask(self, ask, &th, &a.options, answer, now, &open).await } } // settings lane: Settings.answer
        if a.tool.starts_with("order_ask") { // Hub.answer.order: it parks nothing: the answer is the person's message, which starts (or queues) the agent's turn however long they took
            let pend = self.pending_ev(&th, 0, ask, now).await; self.commit_events(vec![json!({"$":"AskClosed","id":ask,"answer":answer,"at":now}), pend]).await;
            return self.deliver(&th, &hub_id("m", now, self.seq()), crate::settings::order::answer_text(answer), None, vec![]).await
        }
        let mut ev = vec![json!({"$":"AskClosed","id":ask,"answer":answer,"at":now})];
        let implement = a.kind == "plan" && answer == "implement";
        if implement { let t = { let w = self.world().await; w.threads.get(&th).cloned() }; if let Some(t) = t { ev.push(json!({"$":"ModesSet","id":th,"runtime":t.runtime,"interaction":"default","model":t.model,"at":now})) } }
        else if !th.is_empty() && (a.kind == "input" || a.kind == "approval") { ev.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("k", now, self.seq()),"tone":"info","kind":"answer","summary":mcp::ans_line(&a.kind, &a.tool, &a.detail, answer),"at":now})) }
        let pend = self.pending_ev(&th, 0, ask, now).await; ev.push(pend);
        self.commit_events(ev).await;
        if let Some(p) = self.parked.lock().unwrap().remove(ask) { let _ = p.tx.send(answer.to_string()); }
        let th_of = self.driver_asks.lock().unwrap().remove(ask); if let Some(t) = th_of { let _ = self.actor(&t).try_send(super::Cmd::Answer { ask: ask.to_string(), answer: answer.to_string() }); }
        if a.tool == "order_quote" { crate::settings::order::answered(self, ask, &th, answer) } // the only thing that sends the files: the person's own Send
        if implement { return self.deliver(&th, &hub_id("m", now, self.seq()), "Implement the proposed plan.", None, vec![]).await }
        Ok(())
    }
}

/// (pinned, has a place a drag gave it on the Pinned shelf), from the policies' fold; (false, false) before it is up
fn pin_state(id: &str) -> (bool, bool) {
    crate::statehost::policies().and_then(|p| p.with_state(|st| (st.threads.get(id).map_or(false, |t| t.pinned), st.order_keys.get(id).map_or(false, |k| !k.0.is_empty())))).unwrap_or((false, false))
}

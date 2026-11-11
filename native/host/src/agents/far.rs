//! Threads on another of the owner's machines, for the tools: an agent's (or a bot's) `thread_*` call that names `<link>~<id>` or a `machine`
//! is asked of that machine as a signed `mcp.call` over /far/rpc (`Agents::far_route`), and the owning machine runs it for the caller
//! (`FarServe`, installed with `Hub::set_far_mcp`): the caller there is the thread `<link>~<its id>`, which is no thread of that machine's project
//! and may reach any of them. Everything is gated on the link being one of the owner's own machines (net/rpc.rs).
//! A thread launched from elsewhere reports back the same way a local one does: its turn ends and asks go to the launcher as `launch_report`.
use super::{lines::hub_id, now_s, Agents};
use crate::net::far;
use serde_json::{json, Value};
use std::sync::{Arc, Weak};

fn sv<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn fail(m: &str) -> (bool, Value) { (false, json!({"error":m})) }

/// The tools that take a thread, and so may name one elsewhere
const ON_THREAD: &[&str] = &["thread_send", "thread_read", "thread_wait", "thread_interrupt", "thread_update", "thread_settle"];
/// The tools that need the file they name on this machine
const LOCAL_THINGS: &[&str] = &["kicad_ipc", "kicad_ipc_status", "kicad_select", "kicad_bring_back", "kicad_export10", "viewer_show"];
/// The tools that take a `machine`
const ON_MACHINE: &[&str] = &["thread_list", "thread_launch"];

/// The owning side of a tool call from another machine.
pub struct FarServe(pub Weak<Agents>);
impl crate::api::FarMcp for FarServe {
    fn serve<'a>(&'a self, link: &'a str, from: &'a str, tool: &'a str, args: &'a Value) -> crate::api::Fut<'a, (bool, Value)> {
        Box::pin(async move {
            let Some(ag) = self.0.upgrade() else { return fail("agents are not running") };
            ag.far_serve(link, from, tool, args).await
        })
    }
}

/// `args` with the thread ids made local to `link` (the request as the owning machine takes it)
fn localized(link: &str, a: &Value) -> Value {
    let mut a = a.clone();
    if let Some(o) = a.as_object_mut() { for k in ["threadId", "parent"] { if let Some(s) = o.get(k).and_then(Value::as_str) { if far::is(s) && far::link(s) == link { o.insert(k.into(), Value::String(far::local(s))); } } } o.remove("machine"); }
    a
}
/// The owning machine's answer with its thread ids made `<link>~<id>` here
pub fn prefixed(link: &str, mut v: Value) -> Value {
    fn pre(link: &str, o: &mut Value, k: &str) { if let Some(s) = o.get(k).and_then(Value::as_str) { if !s.is_empty() && !far::is(s) { o[k] = Value::String(far::id(link, s)) } } }
    pre(link, &mut v, "threadId"); pre(link, &mut v, "parent");
    if let Some(s) = v.get_mut("status") { pre(link, s, "id"); pre(link, s, "parent") }
    if let Some(a) = v.get_mut("threads").and_then(Value::as_array_mut) { for t in a { pre(link, t, "id"); pre(link, t, "parent") } }
    v
}

impl Agents {
    /// The link a `machine` word names (a machine's name in any case, or its link id), among the owner's linked machines.
    pub fn machine_link(&self, word: &str) -> Option<String> {
        let w = word.trim().trim_start_matches('~').to_lowercase(); let ww = crate::bots::mesh::word(&w);
        self.hub.far_machines().into_iter().find(|(id, name, _)| id.to_lowercase() == w || name.to_lowercase() == w || (!ww.is_empty() && crate::bots::mesh::word(name) == ww)).map(|(id, _, _)| id)
    }

    /// A call that is about another machine: Some(answer) when it was sent there (or could not be), None when it is about this one.
    pub async fn far_route(self: &Arc<Self>, thread: &str, name: &str, a: &Value) -> Option<(bool, Value)> {
        // a launch named by project key, remote or name with no machine (Bub's "launch in Backplane"): the machine that has it (net/registry.rs
        // `resolve`: this one's copy, else the most recently active machine's); sent there as a launch with that machine, held if it is away
        let resolved; let a = if name == "thread_launch" && sv(a, "machine").trim().is_empty() && !sv(a, "project").trim().is_empty() && self.world().await.project_find(sv(a, "project")).is_none() {
            match crate::net::registry::resolve(&self.hub, sv(a, "project")) { Some((l, id)) if !l.is_empty() => { let mut b = a.clone(); b["machine"] = json!(l); b["project"] = json!(id); resolved = b; &resolved } _ => a }
        } else { a };
        let (link, tool) = if ON_THREAD.contains(&name) && far::is(sv(a, "threadId")) { (far::link(sv(a, "threadId")), name) }
            else if ON_MACHINE.contains(&name) && !sv(a, "machine").trim().is_empty() {
                match self.machine_link(sv(a, "machine")) { Some(l) => (l, name), None => return Some(fail(&format!("no machine of yours called {} is linked here (machine_list shows them)", sv(a, "machine")))) }
            } else { return None };
        if far::is(thread) { return Some(fail("a thread on another machine cannot ask a third")) }
        let args = localized(&link, a);
        if name == "thread_wait" { return Some(self.far_wait(&link, thread, &args).await) }
        // a message to a thread there: one id for every try, so a send whose answer was lost and goes again is taken once
        let args = if name == "thread_send" { let mut x = args; x["_msg"] = json!(hub_id("fs", now_s(), self.seq_n())); x } else { args };
        // a launch there: one id for every try too (`_launch`: the owner starts it once, however many times it is posted)
        let args = if name == "thread_launch" { let mut x = args; x["_launch"] = json!(hub_id("fl", now_s(), self.seq_n())); x } else { args };
        Some(match self.hub.far_call(&link, thread, tool, &args).await {
            Ok((ok, body)) => (ok, prefixed(&link, body)),
            // that machine cannot be reached now: the message waits here (net/farsend.rs) and goes, cutting into whatever runs there, the moment it is back
            Err(e) if name == "thread_send" && e == "that machine is away" && crate::net::farsend::global().is_some() => {
                let out = crate::net::farsend::global().unwrap(); let id = args["_msg"].as_str().unwrap_or("").to_string();
                out.hold_m(sv(a, "threadId"), &link, "mcp.call", &id, json!({"tool": "thread_send", "from": thread, "args": args}), crate::net::farsend::now_ms());
                (true, json!({"threadId": sv(a, "threadId"), "state": "held", "note": format!("{e}: it waits here and is delivered, as an interrupt of whatever runs there, when that machine is back")}))
            }
            // a launch on a machine that is away (the desktop asleep): held the same way, started there when it is back; its reports come to the launcher as any launch's do
            Err(e) if name == "thread_launch" && e == "that machine is away" && crate::net::farsend::global().is_some() => {
                let out = crate::net::farsend::global().unwrap(); let id = args["_launch"].as_str().unwrap_or("").to_string();
                out.hold_m(thread, &link, "mcp.call", &id, json!({"tool": "thread_launch", "from": thread, "args": args}), crate::net::farsend::now_ms());
                (true, json!({"state": "queued", "machine": sv(a, "machine"), "note": format!("{e}: the launch waits here and starts there when that machine is back. Its reports come to you as messages then; end your turn rather than wait.")}))
            }
            Err(e) => fail(&e),
        })
    }
    /// A tool that works on a local file (KiCad, the viewer) asked by a thread of no project (the uberbot, a bot) for a path this machine does not have:
    /// the error that says where it is done instead, a thread on the machine that has it. None: the tool runs here as before.
    pub async fn not_here(&self, thread: &str, name: &str, a: &Value) -> Option<String> {
        if !LOCAL_THINGS.contains(&name) { return None }
        let path = sv(a, "path").trim(); if path.is_empty() { return None }
        let no_project = self.world().await.threads.get(thread).map_or(false, |t| t.project.is_empty());
        if !no_project || std::path::Path::new(path).exists() { return None }
        let here = crate::net::botnet::name().await;
        let elsewhere: Vec<String> = self.hub.far_machines().into_iter().map(|(_, n, _)| n).collect();
        Some(format!("{path} is not on this machine ({here}): {name} runs where the file is. Start a thread there with thread_launch {{machine, project}} (machine_list shows {}) and ask it to do this.",
            if elsewhere.is_empty() { "the machines".to_string() } else { elsewhere.join(", ") }))
    }
    /// thread_wait on a thread elsewhere: asked again every second until it is done or the time is up
    async fn far_wait(&self, link: &str, thread: &str, a: &Value) -> (bool, Value) {
        let end = tokio::time::Instant::now() + std::time::Duration::from_secs(super::tools::wait_clamp(a.get("timeoutMs").and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))).unwrap_or(30000)));
        loop {
            match self.hub.far_call(link, thread, "thread_wait_now", a).await {
                Ok((true, body)) => { if body["done"].as_bool().unwrap_or(false) || tokio::time::Instant::now() >= end { return (true, prefixed(link, body)) } }
                Ok((false, body)) => return (false, body),
                Err(e) => return fail(&e),
            }
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }
    }

    /// machine_list: this machine and the linked ones, each with its projects (ids for thread_launch)
    pub async fn tool_machines(self: &Arc<Self>, _thread: &str) -> (bool, Value) {
        let mut out = vec![json!({"name":"this machine","here":true,"projects":self.local_projects().await})];
        for (id, name, url) in self.hub.far_machines() {
            let r = self.hub.far_call(&id, "machine_list", "machine_list", &json!({})).await;
            out.push(machine_row(&id, &name, &url, r));
        }
        (true, json!({"machines":out}))
    }
    pub async fn local_projects(&self) -> Value {
        let here = self.hub.mirrors().here(); // (`key`: the project's one name across machines, present/merged.rs; thread_launch takes it)
        let w = self.world().await;
        let mut v: Vec<_> = w.projects.iter().filter(|(_, p)| !p.1).map(|(id, p)| { let folder = w.roots.get(id).cloned().unwrap_or_default(); let origin = crate::mirror::origin_of_root(&folder); let key = crate::present::merged::key_of(&origin, w.st.get(&format!("project.link.{id}")).map(String::as_str).unwrap_or(""), "", &here, id); json!({"id":id,"name":p.0,"folder":folder,"origin":origin,"key":key}) }).collect();
        v.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        Value::Array(v)
    }
    /// thread_list for a thread elsewhere: every project's threads, or the one `project` names
    async fn list_for_far(&self, a: &Value) -> (bool, Value) {
        let want = { let p = sv(a, "project").trim(); if p.is_empty() { None } else { let w = self.world().await; match w.project_find(p) { Some(id) => Some(id), None => return fail(&format!("no project {p} on this machine (machine_list shows them)")) } } };
        let mut ids: Vec<String> = { let m = self.hub.model().await; m.threads.iter().filter(|(_, t)| !t.deleted && !t.project.is_empty() && want.as_ref().map_or(true, |p| &t.project == p)).map(|(k, _)| k.clone()).collect() };
        ids.sort();
        let mut out = vec![]; for id in ids { if let Some(j) = self.thread_json(&id).await { out.push(j) } }
        (true, json!({"threads":out}))
    }

    /// The owning side: `tool` for the thread `from` (named `<link>~<id>`) of another machine.
    pub async fn far_serve(self: &Arc<Self>, link: &str, from: &str, tool: &str, a: &Value) -> (bool, Value) {
        match tool {
            "thread_send" => self.tool_send(from, a).await,
            "thread_read" => self.tool_read(from, a).await,
            "thread_wait_now" => { let mut a = a.clone(); a["_now"] = json!(true); self.tool_wait(from, &a).await }
            "thread_interrupt" => self.tool_interrupt(from, a).await,
            "thread_update" => self.tool_update(from, a).await,
            "thread_settle" => self.tool_settle(from, a).await,
            "thread_launch" => self.tool_launch(from, a).await,
            "thread_list" => self.list_for_far(a).await,
            "machine_list" => (true, json!({"projects":self.local_projects().await})),
            "launch_report" => self.far_report(link, from, a).await,
            _ => fail("not something this machine shares"),
        }
    }
    /// A report from a thread this thread launched on another machine: a message to it from `<link>~<thread>`
    async fn far_report(self: &Arc<Self>, _link: &str, from: &str, a: &Value) -> (bool, Value) {
        let to = sv(a, "to"); let text = sv(a, "text");
        if text.is_empty() || !self.world().await.threads.contains_key(to) { return fail("that thread is not here") }
        let msg = format!("t:{}:lr-{}", sv(a, "thread"), hub_id("r", now_s(), self.seq_n()));
        let thread = sv(a, "thread").to_string(); let _ = from;
        match self.inbound("user", to, &msg, text, (thread, "launch".into(), String::new()), vec![]).await { Ok(()) => (true, json!({"reported":true})), Err(e) => fail(&e) }
    }
    /// A report for a launcher on another machine (`to` is `<link>~<id>`): asked of that machine a few times, since the launcher may be away for a moment
    pub fn send_far_report(self: &Arc<Self>, thread: &str, to: &str, text: &str) {
        let (ag, thread, to, text) = (self.clone(), thread.to_string(), to.to_string(), text.to_string());
        tokio::spawn(async move {
            let link = far::link(&to); let args = json!({"to": far::local(&to), "text": text, "thread": far::id(&link, &thread)});
            match ag.hub.far_call(&link, &thread, "launch_report", &args).await {
                Ok((true, _)) => {}
                Ok((false, b)) => eprintln!("agents: report to {to} refused: {b}"),
                // that machine is away: the report waits in the far outbox (net/farsend.rs) and goes when it is back, cutting into the launcher's turn there
                Err(e) if e != "that machine is away" => eprintln!("agents: report to {to} could not be sent: {e}"),
                Err(e) => match crate::net::farsend::global() {
                    Some(out) => out.hold_m(&to, &link, "mcp.call", &hub_id("fr", now_s(), ag.seq_n()), json!({"tool": "launch_report", "from": thread, "args": args}), crate::net::farsend::now_ms()),
                    None => eprintln!("agents: report to {to} could not be sent: {e}"),
                },
            }
        });
    }
}

/// One linked machine's row in machine_list, from what asking it for its projects gave: its projects; up but on an older
/// Backplane (`older`, with why, so it is not called away); or away.
pub fn machine_row(id: &str, name: &str, url: &str, r: Result<(bool, Value), String>) -> Value {
    match r {
        Ok((true, b)) => json!({"link":id,"name":name,"url":url,"away":false,"projects":b["projects"]}),
        Err(e) if e.starts_with(crate::net::rpc::OLDER) => json!({"link":id,"name":name,"url":url,"away":false,"older":true,"why":e,"projects":[]}),
        _ => json!({"link":id,"name":name,"url":url,"away":true,"projects":[]}),
    }
}
#[cfg(test)]
mod machine_row_tests {
    use super::*;
    #[test] fn a_machine_that_answers_on_an_older_build_is_not_away() {
        let older = machine_row("p1", "desktop", "http://d:3797", Err(format!("{}: mcp.call is not ported on this hub yet", crate::net::rpc::OLDER)));
        assert_eq!((older["away"].as_bool(), older["older"].as_bool()), (Some(false), Some(true)), "{older}");
        assert!(older["why"].as_str().unwrap().contains("update it there"));
        let away = machine_row("p1", "desktop", "http://d:3797", Err("that machine is away".into()));
        assert_eq!(away["away"], true); assert!(away.get("older").is_none());
        let up = machine_row("p1", "desktop", "http://d:3797", Ok((true, json!({"projects":[{"id":"x"}]}))));
        assert_eq!((up["away"].as_bool(), up["projects"][0]["id"].as_str()), (Some(false), Some("x")));
    }
}

//! The planners: request in, events and reply out. Each cites def it follows.
use super::{fold::Fold, key};
use serde_json::{json, Value};

pub struct Plan { pub events: Vec<Value>, pub reply: Value, /// a running turn to stop before the events (Retire.archive)
    pub stop: Option<String>, /// a worktree to make after the events are durable: (thread, project folder, mode, fork source)
    pub wt: Option<(String, String, String, String, super::worktree::Opts)>, /// a scratch thread's own folder to make: (thread)
    pub scratch: Option<String>, /// what the agents module does after the events: ("steer" | "resume", thread, text)
    pub act: Option<(&'static str, String, String)> }

/// One entry of a thread as the handoff text reads it: a message or an activity row.
#[derive(Clone, Debug)]
pub struct Line { pub msg: bool, pub role: String, pub kind: String, pub text: String }
/// The entries of the thread a request names (`thread`), newest first, for the requests that hand its conversation over.
#[derive(Default)]
pub struct Ctx { pub entries: Vec<Line>, pub buy: Buy }
/// What `order.buy` asks of the settings (read from the hub's model: the fold keeps only a few, and a saved fold would not know older ones).
#[derive(Default)]
pub struct Buy { pub on: bool, pub buying: bool, pub last: String }

/// The requests that read the thread's whole conversation.
pub fn needs_entries(m: &str) -> bool { matches!(m, "thread.fork" | "thread.back" | "thread.provider" | "accounts.select" | "accounts.choose" | "fallback.switch") }

fn ok(id: &Value) -> Value { json!({"t":"reply","id":id,"ok":true}) }
fn made(id: &Value, th: &str) -> Value { json!({"t":"reply","id":id,"ok":true,"thread":th}) }
fn err(id: &Value, m: &str) -> Value { json!({"t":"reply","id":id,"ok":false,"error":m}) }
fn s(p: &Value, k: &str, d: &str) -> String { p.get(k).and_then(Value::as_str).unwrap_or(d).to_string() }
/// J.Json.get_nat: a number, or a string of digits, else the default.
fn nat(p: &Value, k: &str) -> u64 { match p.get(k) { Some(Value::Number(n)) => n.as_u64().unwrap_or(0), Some(Value::String(t)) => t.parse().unwrap_or(0), _ => 0 } }
fn done(events: Vec<Value>, id: &Value) -> Option<Plan> { Some(Plan { events, reply: ok(id), stop: None, wt: None, scratch: None, act: None }) }
/// Merge.found: the events that settle a thread whose branch is merged, once (none when it is not due).
pub fn merged(f: &Fold, th: &str, url: &str, at: u64) -> Vec<Value> {
    let t = f.threads.get(th).cloned().unwrap_or_default();
    let shelf_ok = !(t.deleted || t.archived) && t.forced != 1 && (t.pinned || t.forced == 2 || t.settled_at <= t.unsettled_at);
    if !(shelf_ok && !t.branch.is_empty() && f.setting(&format!("merged.{th}")).is_empty()) { return vec![] }
    vec![json!({"$":"ThreadMerged","id":th,"url":url,"at":at}),
        json!({"$":"ActivityLogged","thread":th,"act":hub_id("g", at, f.n()),"tone":"info","kind":"git","summary":format!("merged{}", if url.is_empty() { String::new() } else { format!(": {url}") }),"at":at})]
}
pub fn hub_id(prefix: &str, at: u64, seq: usize) -> String { format!("{prefix}{at}-{seq}") }

/// Whether a thread's snooze is still on at `now` (M.Snooze.on).
fn snoozed(until: u64, now: u64) -> bool { now < until }

/// Snooze.tomorrow: 9:00 local time tomorrow, from the machine's zone.
fn tomorrow(now: u64) -> u64 {
    let off = crate::plat::time::local_offset(now);
    let day = (now as i64 + off).div_euclid(86400);
    ((day + 1) * 86400 + 32400 - off).max(0) as u64
}

/// Archive.changes + Retire.asks + Bg.clear + Watch.retire's row: what archiving or deleting a thread logs besides the flag.
fn retire(f: &Fold, th: &str, at: u64, first: Value, wake: bool) -> Vec<Value> {
    let mut ev = vec![first];
    if wake && snoozed(f.nav(th).until, at) { ev.push(json!({"$":"ThreadSnoozed","id":th,"until":0,"at":at})) }
    for (aid, a) in &f.asks { if a.thread == th && a.kind != "plan" && !a.tool.starts_with("order_ask") { ev.push(json!({"$":"AskClosed","id":aid,"answer":"","at":at})) } }
    if !f.setting(&format!("bg.{th}")).is_empty() { ev.push(json!({"$":"WorkSet","thread":th,"kind":"bg","rows":"","at":at})) }
    ev.push(json!({"$":"WorkSet","thread":th,"kind":"watch","rows":"","at":at}));
    ev
}
fn running(f: &Fold, th: &str) -> bool { f.threads.get(th).map_or(false, |t| t.turn == "running") }

/// Tree.alive: a thread the log knows that is not deleted.
fn alive(f: &Fold, th: &str) -> bool { f.threads.get(th).map_or(false, |t| !t.deleted) }
fn tree_project(f: &Fold, th: &str) -> String { f.threads.get(th).map(|t| t.project.clone()).unwrap_or_default() }
/// Tree.raw: the saved parent, kept only when it is another live thread of the same project.
fn tree_raw(f: &Fold, id: &str) -> String {
    let p = f.setting(&format!("thread.parent.{id}"));
    if !p.is_empty() && p != id && alive(f, id) && alive(f, p) && tree_project(f, id) == tree_project(f, p) { p.to_string() } else { String::new() }
}
/// Tree.chain: walking up from `id` (at most `fuel` steps) never meets `origin`.
fn chain(f: &Fold, mut fuel: u32, mut id: String, origin: &str) -> bool {
    loop {
        if id.is_empty() { return true }
        if fuel == 0 || id == origin { return false }
        fuel -= 1; id = tree_raw(f, &id);
    }
}
fn parent_valid(f: &Fold, th: &str, parent: &str) -> bool {
    alive(f, th) && (parent.is_empty() || (alive(f, parent) && tree_project(f, th) == tree_project(f, parent) && chain(f, 64, parent.to_string(), th)))
}

/// Fallback.manual: an explicit provider, model or mode choice, and a turn that starts,
/// clear the automatic-fallback bookkeeping of that thread; appended after the step's own events.
pub fn step_tail(mut ev: Vec<Value>, at: u64) -> Vec<Value> {
    let set = |k: String| json!({"$":"SettingSet","key":k,"value":"","at":at});
    let mut tail = vec![];
    for e in &ev {
        let id = e.get("id").or_else(|| e.get("thread")).and_then(Value::as_str).unwrap_or("").to_string();
        match e["$"].as_str().unwrap_or("") {
            "ProviderSet" => tail.extend([set(format!("account.failover.preference.{id}")), set(format!("restart.retry.{id}")), set(format!("provider.resume.{id}")), set(format!("fallback.intent.{id}"))]),
            "TurnChanged" if e["state"] == "running" => tail.extend([set(format!("restart.retry.{id}")), set(format!("provider.resume.{id}"))]),
            "ModesSet" => tail.extend([set(format!("account.failover.preference.{id}")), set(format!("fallback.intent.{id}"))]),
            _ => {}
        }
    }
    ev.extend(tail); ev
}

/// Pend.fix: every step that closes an ask also sets its thread's pending count to the asks still open, so it never reads as blocked.
pub fn pend_fix(f: &Fold, mut ev: Vec<Value>) -> Vec<Value> {
    let closed: Vec<String> = ev.iter().filter(|e| e["$"] == "AskClosed").filter_map(|e| e["id"].as_str().map(str::to_string)).collect();
    let mut tail = vec![];
    for e in ev.iter().filter(|e| e["$"] == "AskClosed") {
        let Some(a) = e["id"].as_str().and_then(|i| f.asks.get(i)) else { continue };
        let open = f.asks.iter().filter(|(i, x)| x.thread == a.thread && !closed.contains(i)).count();
        tail.push(json!({"$":"PendingChanged","thread":a.thread,"count":open,"at":e["at"]}));
    }
    ev.extend(tail); ev
}

pub fn plan(f: &Fold, at: u64, id: &Value, method: &str, p: &Value, cx: &Ctx) -> Option<Plan> {
    let mut pl = plan_inner(f, at, id, method, p, cx)?; pl.events = step_tail(pend_fix(f, pl.events), at); Some(pl)
}

fn plan_inner(f: &Fold, at: u64, id: &Value, method: &str, p: &Value, cx: &Ctx) -> Option<Plan> {
    let th = s(p, "thread", "");
    match method {
        "thread.settle" => done(vec![json!({"$":"ThreadSettled","id":th,"manual":true,"at":at})], id),
        "thread.unsettle" => done(vec![json!({"$":"ThreadUnsettled","id":th,"at":at})], id),
        "thread.pin" => done(vec![json!({"$":"ThreadPinned","id":th,"pinned":true,"at":at})], id),
        "thread.unpin" => done(vec![json!({"$":"ThreadPinned","id":th,"pinned":false,"at":at})], id),
        "thread.unarchive" => done(vec![json!({"$":"ThreadArchived","id":th,"archived":false,"at":at})], id),
        // a rename: the title trimmed; a blank one changes nothing (Rename.changes)
        "thread.rename" => { let t = s(p, "title", ""); let t = t.trim(); done(if t.is_empty() || th.is_empty() { vec![] } else { vec![json!({"$":"ThreadRenamed","id":th,"title":t,"at":at}), json!({"$":"SettingSet","key":format!("title.person.{th}"),"value":"1","at":at})] }, id) } // the person's own title: no titler replaces it
        "thread.archive" | "thread.delete" => {
            let stop = Some(th.clone()); // Retire.archive: AgentStop every time, a running turn is interrupted first (see mod.rs)
            let first = if method == "thread.archive" { json!({"$":"ThreadArchived","id":th,"archived":true,"at":at}) } else { json!({"$":"ThreadDeleted","id":th,"at":at}) };
            Some(Plan { events: retire(f, &th, at, first, method == "thread.archive"), reply: ok(id), stop, wt: None, scratch: None, act: None })
        }
        "thread.snooze" => {
            // "tomorrow" is a server job (the machine's zone): a copy of a log tells the client no (Copy.git.reads), anywhere else it answers {thread}
            let tomorrow_job = s(p, "for", "") == "tomorrow";
            if tomorrow_job && crate::copy::on() { return Some(Plan { events: vec![], reply: err(id, crate::copy::GIT_TOLD), stop: None, wt: None, scratch: None, act: None }) }
            let until = match s(p, "for", "").as_str() { "1h" => at + 3600, "1w" => at + 604800, "off" => 0, "tomorrow" => tomorrow(at), _ => nat(p, "until") };
            let reply = if tomorrow_job { json!({"t":"reply","id":id,"ok":true,"result":{"thread":th}}) } else { ok(id) };
            Some(Plan { events: vec![json!({"$":"ThreadSnoozed","id":th,"until":until,"at":at})], reply, stop: None, wt: None, scratch: None, act: None })
        }
        // a shelf's new order after a drag: {section: "pinned" | "active", thread: the one moved, ids: the shelf's threads in their new order}
        "thread.reorder" => {
            let section = s(p, "section", "");
            if section != "pinned" && section != "active" { return Some(Plan { events: vec![], reply: err(id, "only pinned and active threads reorder"), stop: None, wt: None, scratch: None, act: None }) }
            let xs: Vec<(String, String)> = p.get("ids").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str)
                .map(|i| { let n = f.nav(i); (i.to_string(), if section == "pinned" { n.pinned } else { n.active }) }).collect();
            done(key::plan(&xs, &th).into_iter().map(|(i, k)| json!({"$":"ThreadOrdered","id":i,"section":section,"key":k,"at":at})).collect(), id)
        }
        // the sidebar's projects in their new order after a drag: {project: the one moved, ids: every project in the new order}
        "project.reorder" => {
            let xs: Vec<(String, String)> = p.get("ids").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str)
                .map(|i| (i.to_string(), f.nav(&format!("project:{i}")).active)).collect();
            done(key::plan(&xs, &s(p, "project", "")).into_iter().map(|(i, k)| json!({"$":"ProjectOrdered","id":i,"key":k,"at":at})).collect(), id)
        }
        "project.remove" => done(vec![json!({"$":"ProjectRemoved","id":s(p, "project", ""),"at":at})], id),
        // Home.rename.project: the id, root and creation time stay; a blank title changes nothing
        "project.rename" => {
            let pid = s(p, "project", ""); let t = s(p, "title", ""); let t = t.trim();
            done(match f.projects.get(&pid) { Some(pr) if !pr.removed && !t.is_empty() => vec![json!({"$":"ProjectCreated","id":pid,"title":t,"root":pr.root,"at":pr.at})], _ => vec![] }, id)
        }
        // Roots.move: a project pointed at its folder's new place; its worktrees under the old root move with it
        "project.move" => {
            let pid = s(p, "project", ""); let raw = s(p, "path", ""); let t = raw.trim(); let path = if t.ends_with('/') && t.len() > 1 { &t[..t.len() - 1] } else { t };
            if !path.starts_with('/') { return Some(Plan { events: vec![], reply: err(id, "Type the folder's full path, starting with /"), stop: None, wt: None, scratch: None, act: None }) }
            if pid.contains('~') || pid == "scratch" { return Some(Plan { events: vec![], reply: err(id, "Type the folder's full path, starting with /"), stop: None, wt: None, scratch: None, act: None }) }
            let Some(pr) = f.projects.get(&pid).filter(|x| !x.removed) else { return Some(Plan { events: vec![], reply: err(id, "Type the folder's full path, starting with /"), stop: None, wt: None, scratch: None, act: None }) };
            let mut ev = vec![json!({"$":"ProjectCreated","id":pid,"title":pr.title,"root":path,"at":at})];
            for (tid, t) in &f.threads { if t.project == pid && t.worktree.starts_with(&format!("{}/", pr.root)) {
                ev.push(json!({"$":"WorktreeSet","id":tid,"branch":t.branch,"worktree":format!("{path}{}", &t.worktree[pr.root.len()..]),"at":at})) } }
            let missing = f.setting("projects.missing"); let rest: Vec<&str> = missing.split(' ').filter(|w| !w.is_empty() && *w != pid).collect();
            ev.push(json!({"$":"SettingSet","key":"projects.missing","value":if rest.is_empty() { String::new() } else { format!(" {} ", rest.join(" ")) },"at":at}));
            done(ev, id)
        }
        // Hub.thread_parent: empty parent detaches; the whole new ancestry is checked first
        "thread.detach" | "thread.reparent" => {
            let parent = if method == "thread.detach" { String::new() } else { s(p, "parent", "") };
            if !parent_valid(f, &th, &parent) { return Some(Plan { events: vec![], reply: err(id, "Parent must be a live conversation in the same project without a cycle (maximum depth 64)."), stop: None, wt: None, scratch: None, act: None }) }
            let mut ev = vec![json!({"$":"ThreadParentSet","id":th,"parent":parent,"at":at})];
            if !parent.is_empty() { ev.push(json!({"$":"SettingSet","key":format!("thread.tree.{parent}"),"value":"1","at":at})) }
            done(ev, id)
        }
        // Todos.edit.rpc: the person's add, delete, move, set on the agent's list; the fold applies it (state/todo.rs `edit`). A new item has no id yet: it gets one the agent will not make.
        "todos.edit" => {
            if th.is_empty() || !f.threads.contains_key(&th) { return Some(Plan { events: vec![], reply: err(id, "No such thread."), stop: None, wt: None, scratch: None, act: None }) }
            let op = s(p, "op", ""); if !["add", "delete", "move", "set"].contains(&op.as_str()) { return Some(Plan { events: vec![], reply: err(id, "Unknown to-do edit."), stop: None, wt: None, scratch: None, act: None }) }
            let tid = if op == "add" && s(p, "id", "").is_empty() { format!("u{at}-{}", f.threads.len()) } else { s(p, "id", "") };
            let data = json!({"op": op, "id": tid, "text": s(p, "text", ""), "status": s(p, "status", ""), "to": nat(p, "to")}).to_string();
            done(vec![json!({"$":"TodosChanged","thread":th,"who":"person","data":data,"at":at})], id)
        }
        "queue.edit" => done(vec![json!({"$":"QueueEdited","thread":th,"msg":s(p, "msg", ""),"text":s(p, "text", ""),"at":at})], id),
        "queue.remove" => done(vec![json!({"$":"QueueRemoved","thread":th,"msg":s(p, "msg", ""),"at":at})], id),
        "queue.move" => done(vec![json!({"$":"QueueMoved","thread":th,"msg":s(p, "msg", ""),"to":nat(p, "to"),"at":at})], id),
        "thread.create" => create(f, at, id, p, None, None),
        "thread.titled" => {
            // Title.changes: the writer's title replaces the placeholder only while the thread still has it, and only when there is an answer
            let (cur, ph) = (f.threads.get(&th).map(|t| t.title.clone()).unwrap_or_default(), s(p, "placeholder", ""));
            let clean = title_clean(&s(p, "title", ""));
            done(if cur == ph && !ph.is_empty() && !clean.is_empty() { vec![json!({"$":"ThreadRenamed","id":th,"title":clean,"at":at})] } else { vec![] }, id)
        }
        "thread.unrooted" => {
            let mut ev = vec![json!({"$":"WorktreeRemoved","id":th,"at":at})];
            if !f.setting(&format!("wt.again.{th}")).is_empty() { ev.push(json!({"$":"SettingSet","key":format!("wt.again.{th}"),"value":"","at":at})) } // Wt.answered
            done(ev, id)
        }
        // Merge.found: gh saw the thread's branch merged; it settles, once. (Policy.merged's alert record is not written: the notification records are not ported.)
        "thread.merged" => done(merged(f, &th, &s(p, "url", ""), at), id),
        "project.start" => start(f, at, id, p),
        // Queue.steer: a queued message sent now (into the running turn, or as a turn of its own); the agents module runs it
        "queue.steer" => {
            let m = s(p, "msg", "");
            let Some((_, text)) = f.queue(&th).iter().find(|(i, _)| *i == m).cloned() else { return Some(Plan { events: vec![], reply: err(id, "no such queued message"), stop: None, wt: None, scratch: None, act: None }) };
            Some(Plan { events: vec![json!({"$":"QueueTakenOne","thread":th,"msg":m,"at":at})], reply: ok(id), stop: None, wt: None, scratch: None, act: Some((if running(f, &th) { "steer" } else { "start_one" }, th.clone(), text)) })
        }
        // Hub.resume: the held queue of an idle thread (kept across a restart, or after a stop) starts now
        "queue.resume" => {
            let ev: Vec<Value> = if f.setting(&format!("thread.config.hold.{th}")) == "on" { vec![json!({"$":"SettingSet","key":format!("thread.config.hold.{th}"),"value":"","at":at})] } else { vec![] };
            let go = !running(f, &th) && f.queue(&th).iter().any(|(m, _)| !m.starts_with("handoff:"));
            let text = f.queue_text(&th); // Queue.texts
            Some(Plan { events: ev, reply: ok(id), stop: None, wt: None, scratch: None, act: go.then(|| ("resume", th.clone(), text)) })
        }
        "thread.modes" => modes(f, at, id, p),
        "thread.effort" => {
            let provider = f.threads.get(&th).map(|t| t.provider.as_str()).unwrap_or("");
            done(vec![json!({"$":"SettingSet","key":format!("effort.{th}"),"value":effort_of(provider, &s(p, "effort", "")),"at":at})], id)
        }
        "thread.provider" => switch(f, at, id, p, cx, false),
        "fallback.switch" => switch(f, at, id, p, cx, true),
        "accounts.select" => account_select(f, at, id, p, cx, "person"),
        "accounts.choose" => { let by = s(p, "by", ""); account_select(f, at, id, p, cx, &by) }
        "thread.fork" => fork(f, at, id, &th, cx),
        "thread.back" => back(f, at, id, &th, cx),
        "order.buy" => buy(f, at, id, &th, &cx.buy),
        _ => None,
    }
}

/// Hub.thread_create: a thread of a project, as the person asked; a retry with the same creationKey is the same thread.
fn create(f: &Fold, at: u64, id: &Value, p: &Value, new_root: Option<(&str, usize)>, forced_id: Option<&str>) -> Option<Plan> {
    let project = s(p, "project", ""); let parent = s(p, "parent", ""); let key = s(p, "creationKey", "");
    let refuse = |m: &str| Some(Plan { events: vec![], reply: err(id, m), stop: None, wt: None, scratch: None, act: None });
    let root = new_root.map(|r| r.0.to_string()).unwrap_or_else(|| f.projects.get(&project).map(|x| x.root.clone()).unwrap_or_default());
    let scratch = project == "scratch";
    let n = new_root.map_or(f.n(), |r| r.1);
    let mut th = forced_id.map(str::to_string).unwrap_or_else(|| hub_id("t", at, n));
    if !key.is_empty() {
        let hex = key.len() == 32 && key.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c));
        let live = f.projects.get(&project).map_or(false, |x| !x.removed && !x.root.is_empty()) && !scratch;
        if !(hex && parent.is_empty() && live) { return refuse("Choose an existing project and retry this request.") }
        th = format!("improvement-{key}");
        if f.threads.contains_key(&th) {
            return Some(if alive(f, &th) && tree_project(f, &th) == project { Plan { events: vec![], reply: made(id, &th), stop: None, wt: None, scratch: None, act: None } }
                else { Plan { events: vec![], reply: err(id, "This request belongs to a different or removed conversation. Open that conversation before retrying."), stop: None, wt: None, scratch: None, act: None } });
        }
    } else if !parent.is_empty()
        && !(alive(f, &parent) && tree_project(f, &parent) == project && chain(f, 63, tree_raw(f, &parent), &parent)) {
        return refuse("Subthread parent must be an existing conversation in the same project (maximum depth 64).");
    }
    // J.Json.get_str: the default when the field is missing or not a string; M.Setting.get keeps a stored ""
    let gs = |k: &str, d: &str| p.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
    let env = gs("env", f.get("thread.env", "auto"));
    let title = gs("title", "New thread");
    let provider = gs("provider", f.get("agent.provider", "claude"));
    let model = gs("model", f.get("agent.model", ""));
    let mode = if scratch || env == "local" { "local" } else { "worktree" };
    let wt = if scratch { format!("{root}/{th}") } else { String::new() };
    let mut ev = vec![json!({"$":"ThreadCreated","id":th,"project":project,"title":title,"env":mode,"branch":"","worktree":wt,"provider":provider,"model":model,"at":at})];
    ev.extend(account_created(f, &th, &provider, at)); // Accounts.created: the binding follows the thread
    if !key.is_empty() || parent.is_empty() {} else {
        ev.push(json!({"$":"ThreadParentSet","id":th,"parent":parent,"at":at}));
        ev.push(json!({"$":"SettingSet","key":format!("thread.tree.{parent}"),"value":"1","at":at}));
    }
    // the server's worktree job follows (it also says so when the folder is not a repository)
    // an Improvements send names its base (a local branch that exists: the handler checked) and its branch; only with a creation key
    let prefer = { let own = f.get(&format!("worktree.base.{project}"), ""); if own.is_empty() { f.get("worktree.base", "") } else { own } }.to_string(); // the project's default base (a local branch), when no base is asked for
    let opts = if !key.is_empty() { super::worktree::Opts { base: s(p, "base", ""), branch: super::worktree::sanitize(&s(p, "branch", "")), prefer: String::new() } } else { super::worktree::Opts { prefer, ..Default::default() } };
    let wt = (mode == "worktree" && !scratch).then(|| (th.clone(), root.clone(), env.clone(), String::new(), opts));
    Some(Plan { events: ev, reply: made(id, &th), stop: None, wt, scratch: scratch.then(|| th.clone()), act: None })
}


// ---- providers, modes, effort -------------------------------------------------------------------------------------------

fn levels(provider: &str) -> &'static [&'static str] {
    match provider { "claude" => &["low", "medium", "high", "xhigh", "max"], "codex" => &["low", "medium", "high", "xhigh"], "grok" => &["low", "medium", "high"],
        "muse" => &["minimal", "low", "medium", "high", "xhigh", "max"], "pi" => &["off", "minimal", "low", "medium", "high", "xhigh", "max"],
        "omp" => &["off", "auto", "minimal", "low", "medium", "high", "xhigh", "max"], _ => &[] }
}
/// AG.Effort.of: the effort a provider takes: e when it offers it, else its default ("").
fn effort_of(provider: &str, e: &str) -> String { if levels(provider).contains(&e) { e.to_string() } else { String::new() } }

fn runtime_of(x: &str) -> &'static str { ["approval-required", "auto-accept-edits", "auto", "full-access"].into_iter().find(|r| *r == x).unwrap_or("full-access") }

/// Config.waits: something the thread would pick up on its own (the explicit choice must not release it).
fn waits(f: &Fold, th: &str) -> bool {
    f.setting(&format!("thread.config.hold.{th}")) == "on" || f.threads.get(th).map_or(false, |t| t.turn == "interrupted") || f.queue(th).iter().any(|(m, _)| !m.starts_with("handoff:"))
        || !f.setting(&format!("provider.resume.{th}")).is_empty() || !f.setting(&format!("restart.retry.{th}")).is_empty() || !f.setting(&format!("resume.{th}")).is_empty()
}
/// Config.hold: an explicit provider or model choice is not permission to release old work.
fn hold(f: &Fold, th: &str, at: u64) -> Vec<Value> {
    if !running(f, th) && waits(f, th) && f.setting(&format!("thread.config.hold.{th}")) != "on" { vec![json!({"$":"SettingSet","key":format!("thread.config.hold.{th}"),"value":"on","at":at})] } else { vec![] }
}

/// Hub.modes: what the request leaves out stays as it is.
fn modes(f: &Fold, at: u64, id: &Value, p: &Value) -> Option<Plan> {
    let th = s(p, "thread", ""); let t = f.threads.get(&th).cloned().unwrap_or_default();
    let (rt, im, mo) = (if t.runtime.is_empty() { "full-access".to_string() } else { t.runtime.clone() }, if t.interaction.is_empty() { "default".to_string() } else { t.interaction.clone() }, t.model.clone());
    let mut ev = vec![json!({"$":"ModesSet","id":th,"runtime":runtime_of(&s(p, "runtime", &rt)),"interaction":if s(p, "interaction", &im) == "plan" { "plan" } else { "default" },"model":s(p, "model", &mo),"at":at})];
    if p.get("model").is_some() { ev.extend(hold(f, &th, at)) }
    done(ev, id)
}

// ---- handing a conversation over --------------------------------------------------------------------

fn cut(t: &str, n: usize) -> String { if t.chars().count() > n { format!("{} ...[{} more characters]", t.chars().take(n).collect::<String>(), t.chars().count() - n) } else { t.to_string() } }
fn hand_line(l: &Line) -> String {
    if l.msg { format!("{}: {}", match l.role.as_str() { "user" => "User", "assistant" => "Assistant", _ => "Note" }, cut(&l.text, 4000)) }
    else { format!("Tool {}{}: {}", l.kind, if l.role == "error" { " (failed)" } else { "" }, cut(&l.text, 300)) }
}
/// Hand.text: why it is here, the newest entries that fit the budget (oldest first), how many older ones were left out, and where it ends.
/// `es` newest first.
pub fn hand_text(why: &str, es: &[Line], th: &str) -> String {
    let (mut left, mut lines, mut full, mut omitted) = (16000usize, Vec::<String>::new(), false, 0usize);
    for e in es {
        let l = hand_line(e); let n = 1 + l.chars().count();
        if !full && n <= left { left -= n; lines.insert(0, l) } else { full = true; omitted += 1 }
    }
    let first = es.iter().rev().find(|e| e.msg && e.role == "user").map(hand_line).unwrap_or_default();
    let mut body: Vec<String> = vec![];
    if omitted > 0 { body.push(cut(&first, 2000)); body.push(format!("[{omitted} earlier items left out; thread_read with threadId {th} has them all]")) }
    body.extend(lines);
    format!("[Backplane] {why}\n\n{}\n\n[End of the handed-over context. The user's message follows.]", body.join("\n\n"))
}

/// Buy.title.of: a project's next buy thread is "Buy: <project> · 2".
fn buy_title(f: &Fold, project: &str) -> String {
    let base = format!("Buy: {}", f.projects.get(project).map_or("", |p| p.title.as_str()));
    let n = f.threads.values().filter(|t| t.project == project && t.title.starts_with(&base)).count();
    if n == 0 { base } else { format!("{base} · {}", n + 1) }
}
/// Buy.recent: a thread id says when it was made (`t<at>-<n>`). A rapid repeat of the same purchase opens the thread just made, also when two clients press before either hears back.
fn buy_recent(th: &str, now: u64) -> bool { th.strip_prefix('t').and_then(|r| r.split('-').next()).and_then(|a| a.parse::<u64>().ok()).map_or(false, |made| now >= made && now - made <= 2) }
/// Buy.start: a thread of its own in the source's folder, marked buy.<thread>, started by the person's "Buy it"; the client that asked goes to it.
fn buy(f: &Fold, at: u64, id: &Value, src: &str, b: &Buy) -> Option<Plan> {
    let no = |m: &str| Some(Plan { events: vec![], reply: err(id, m), stop: None, wt: None, scratch: None, act: None });
    let there = |th: &str| Some(Plan { events: vec![], reply: made(id, th), stop: None, wt: None, scratch: None, act: None });
    let Some(t) = f.threads.get(src).filter(|t| !t.deleted) else { return no("no such thread") };
    if !b.on { return no("Ordering is off: turn it on in Settings to buy") }
    if b.buying { return there(src) } // a queued click after the client went to its buy thread: it has no Buy of its own
    if buy_recent(&b.last, at) && f.threads.get(&b.last).map_or(false, |l| !l.deleted) { return there(&b.last) }
    let th = hub_id("t", at, f.n());
    // the same folder, as a delegated task has; a source with no worktree of its own runs in the project folder, and a thread marked worktree with none would wait for one forever
    let env = if t.worktree.is_empty() { "local" } else { t.env.as_str() };
    let mut ev = vec![json!({"$":"ThreadCreated","id":th,"project":t.project,"title":buy_title(f, &t.project),"env":env,"branch":t.branch,"worktree":t.worktree,"provider":t.provider,"model":t.model,"at":at})];
    ev.extend(account_created(f, &th, &t.provider, at)); ev.extend(account_forked(f, &th, src, at));
    ev.extend([json!({"$":"ModesSet","id":th,"runtime":if t.runtime.is_empty() { "full-access" } else { &t.runtime },"interaction":if t.interaction.is_empty() { "default" } else { &t.interaction },"model":t.model,"at":at}),
        json!({"$":"SettingSet","key":format!("buy.{th}"),"value":src,"at":at}), json!({"$":"SettingSet","key":format!("buy.last.{src}"),"value":th,"at":at})]);
    Some(Plan { events: ev, reply: made(id, &th), stop: None, wt: None, scratch: None, act: Some(("post", th, "Buy it".to_string())) })
}

fn fork(f: &Fold, at: u64, id: &Value, src: &str, cx: &Ctx) -> Option<Plan> {
    let Some(t) = f.threads.get(src) else { return Some(Plan { events: vec![], reply: err(id, "no such thread"), stop: None, wt: None, scratch: None, act: None }) };
    let n = f.n(); let th = hub_id("t", at, n);
    let scratch = t.project == "scratch"; let root = f.projects.get(&t.project).map(|x| x.root.clone()).unwrap_or_default();
    let env = if scratch || t.worktree.is_empty() { "local" } else { "worktree" };
    let why = format!("This thread is a fork of \"{}\" ({src}). You continue its work from where it stands; its conversation so far, oldest first:", t.title);
    let mut ev = vec![
        json!({"$":"ThreadCreated","id":th,"project":t.project,"title":format!("Fork: {}", t.title),"env":env,"branch":"","worktree":if scratch { format!("{root}/{th}") } else { String::new() },"provider":t.provider,"model":t.model,"at":at})];
    ev.extend(account_created(f, &th, &t.provider, at));
    ev.extend([
        json!({"$":"ModesSet","id":th,"runtime":if t.runtime.is_empty() { "full-access" } else { &t.runtime },"interaction":if t.interaction.is_empty() { "default" } else { &t.interaction },"model":t.model,"at":at}),
        json!({"$":"ThreadForked","id":th,"source":src,"at":at})]);
    ev.extend(account_forked(f, &th, src, at));
    ev.extend([
        json!({"$":"TurnQueued","thread":th,"msg":format!("handoff:{src}"),"text":hand_text(&why, &cx.entries, src),"at":at}),
        json!({"$":"ActivityLogged","thread":src,"act":hub_id("k", at, n),"tone":"info","kind":"fork","summary":format!("Forked into \"Fork: {}\"", t.title),"at":at})]);
    // a fork with no worktree of its own shares the source's folder: it says so
    if !scratch && t.worktree.is_empty() {
        ev.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("g", at, n),"tone":"info","kind":"git","summary":format!("working in the project folder with \"{}\" (no worktree of its own): its edits change the same files", t.title),"at":at}));
    }
    let wt = (!scratch && !t.worktree.is_empty()).then(|| (th.clone(), root, "fork".to_string(), src.to_string(), Default::default()));
    Some(Plan { events: ev, reply: made(id, &th), stop: None, wt, scratch: scratch.then(|| th.clone()), act: None })
}

fn back(f: &Fold, at: u64, id: &Value, fork: &str, cx: &Ctx) -> Option<Plan> {
    let src = f.forks.get(fork).cloned().unwrap_or_default();
    if src.is_empty() { return Some(Plan { events: vec![], reply: err(id, "this thread is not a fork"), stop: None, wt: None, scratch: None, act: None }) }
    let n = f.n(); let title = |x: &str| f.threads.get(x).map(|t| t.title.clone()).unwrap_or_default();
    let why = format!("Work done in a fork of this thread, \"{}\" ({fork}), brought back here. Its files stay on its own branch; what was said and done there, oldest first:", title(fork));
    done(vec![
        json!({"$":"TurnQueued","thread":src,"msg":format!("handoff:back:{fork}:{n}"),"text":hand_text(&why, &cx.entries, fork),"at":at}),
        json!({"$":"ActivityLogged","thread":src,"act":hub_id("k", at, n),"tone":"info","kind":"fork","summary":format!("The work of \"{}\" goes with the next message", title(fork)),"at":at}),
        json!({"$":"ActivityLogged","thread":fork,"act":hub_id("k", at, n + 1),"tone":"info","kind":"fork","summary":format!("Brought back to \"{}\"", title(&src)),"at":at})], id)
}

/// Hub.switch.as: the thread goes on with provider `to` from its next turn, on `model` and, when named, that effort: all of it between turns or none of it.
/// `auto`: the fallback chain moved it (`fallback.switch`): the turn starts again at once, so nothing is held, and `say` is the timeline's line.
fn switch(f: &Fold, at: u64, id: &Value, p: &Value, cx: &Ctx, auto: bool) -> Option<Plan> {
    let th = s(p, "thread", ""); let to = s(p, "provider", ""); let model = s(p, "model", "");
    let from = f.threads.get(&th).map(|t| t.provider.clone()).unwrap_or_default();
    if running(f, &th) { return Some(Plan { events: vec![], reply: err(id, "a provider changes between turns: stop this one first"), stop: None, wt: None, scratch: None, act: None }) }
    if from == to { return done(hold(f, &th, at), id) }
    let n = f.n();
    let seen: usize = f.setting(&format!("seen.{th}.{to}")).parse().unwrap_or(0);
    let count = f.count.get(&th).copied().unwrap_or(0); let missed = count.saturating_sub(seen);
    let mut ev: Vec<Value> = vec![];
    for (m, _) in f.queue(&th) { if m.starts_with("handoff:switch:") { ev.push(json!({"$":"QueueRemoved","thread":th,"msg":m,"at":at})) } }
    ev.push(json!({"$":"ProviderSet","id":th,"provider":to,"model":model,"at":at}));
    if let Some(e) = p.get("effort").and_then(Value::as_str) { ev.push(json!({"$":"SettingSet","key":format!("effort.{th}"),"value":effort_of(&to, e),"at":at})) }
    if missed != 0 {
        let why = if seen == 0 { format!("This thread was worked on with {from} until now; you ({to}) take it over. Its conversation so far, oldest first:") }
            else { format!("This thread went on with {from} since you ({to}) last took part. What happened meanwhile, oldest first:") };
        ev.push(json!({"$":"TurnQueued","thread":th,"msg":format!("handoff:switch:{n}"),"text":hand_text(&why, &cx.entries[..missed.min(cx.entries.len())], &th),"at":at}))
    }
    let waiting = !auto && waits(f, &th); // Switch.go: AgentStop unless Config.waits, on the state before the hold
    if !auto { ev.extend(hold(f, &th, at)) }
    if auto { ev.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("p", at, n),"tone":"info","kind":"fallback","summary":s(p, "say", ""),"at":at})) }
    else if count > 0 { ev.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("p", at, n),"tone":"info","kind":"provider","summary":format!("Now on {to}{}", if missed == 0 { "" } else { "; it gets what it missed with the next message" }),"at":at})) }
    Some(Plan { events: ev, reply: ok(id), stop: if waiting { None } else { Some(th.clone()) }, wt: None, scratch: None, act: None })
}

/// Hub.accounts.select: the thread goes on under another account of its provider from its next message, between turns. The old account's native session and how much
/// of the conversation it has seen are kept under its own key, the new account's are restored, and what the new account has not seen is handed over with the next message.
/// A stale menu (binding, epoch, state) is refused; another provider's account is picked after the agent is switched (thread.provider).
/// `by` says who chose: "person" (`accounts.select`: their pick is the thread's pin), or, through `accounts.choose` (hub-side only), "agent" and "bot" (a choice a pin refuses unless the person
/// asked, which the tool checks and says with `asked`), "failover" (the driver, past a pin: the pin is where the thread returns) and "return" (the driver, back to where it came from).
fn account_select(f: &Fold, at: u64, id: &Value, p: &Value, cx: &Ctx, by: &str) -> Option<Plan> {
    use crate::settings::accounts as ac;
    let refuse = |m: &str| Some(Plan { events: vec![], reply: err(id, m), stop: None, wt: None, scratch: None, act: None });
    let th = s(p, "thread", ""); let th = th.rsplit('|').next().unwrap_or("").to_string(); let profile = s(p, "profile", ""); let epoch = p.get("epoch").and_then(Value::as_u64).unwrap_or(0);
    let Some(t) = f.threads.get(&th).filter(|t| !t.deleted).cloned() else { return refuse("The thread or account selection changed. Reopen the account menu and try again.") };
    if running(f, &th) { return refuse("Finish or stop the current turn before selecting another account.") }
    let inventory = f.setting("accounts.inventory").to_string();
    let prof = ac::profile(&inventory, &profile);
    let provider = if profile.is_empty() { t.provider.clone() } else { prof["provider"].as_str().unwrap_or("").to_string() };
    if !ac::supported(&provider) { return refuse("The thread or account selection changed. Reopen the account menu and try again.") }
    if provider != t.provider { return refuse("Switch this thread to that agent first, then pick the account.") }
    let key = ac::binding_key(&th, &provider);
    let raw = f.setting(&key).to_string();
    if by == "person" && s(p, "expected", "") != raw { return refuse("The thread or account selection changed. Reopen the account menu and try again.") }
    if !profile.is_empty() && !ac::ready(&prof, &provider, epoch) { return refuse(ac::unavailable(&prof, epoch)) }
    let chooser = matches!(by, "agent" | "bot");
    let chosen_key = ac::chosen_key(&th, &provider);
    if chooser && !p.get("asked").and_then(Value::as_bool).unwrap_or(false) { if let Some(name) = ac::pinned_to(f.setting(&chosen_key), &inventory, &profile, epoch) { return refuse(&ac::pin_refusal(&name)) } }
    let cur: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    let (cur_id, cur_epoch) = (cur["id"].as_str().unwrap_or("").to_string(), cur["epoch"].as_u64().unwrap_or(0));
    let set = |k: String, v: String| json!({"$":"SettingSet","key":k,"value":v,"at":at});
    // who chose, and what their choice ends: the person's pick is the end of any failover and of a hold they have overruled
    let mut after: Vec<Value> = vec![];
    if matches!(by, "person" | "agent" | "bot") { after.push(set(chosen_key, ac::chosen_show(&profile, epoch, by, &s(p, "who", ""), at))) }
    if matches!(by, "person" | "return") { after.push(set(crate::settings::fallback::preference_key(&th), String::new())) }
    if by == "person" { for k in f.holds_of(&provider, &profile, epoch) { after.push(set(k, String::new())) } after.push(set(crate::settings::driver::tried_key(&th), String::new())) }
    if cur_id == profile && cur_epoch == epoch { return done(after, id) }
    let count = f.count.get(&th).copied().unwrap_or(0);
    let seen: usize = f.setting(&ac::seen_key(&th, &provider, &profile, epoch)).parse().unwrap_or(0);
    let n = f.n();
    let mut ev: Vec<Value> = vec![];
    for (m, _) in f.queue(&th) { if m.starts_with("handoff:switch:") { ev.push(json!({"$":"QueueRemoved","thread":th,"msg":m,"at":at})) } }
    let session_new = f.setting(&ac::session_key(&th, &provider, &profile, epoch)).to_string();
    ev.extend([
        set(ac::session_key(&th, &provider, &cur_id, cur_epoch), t.session.clone()), set(ac::seen_key(&th, &provider, &cur_id, cur_epoch), count.to_string()),
        set(key, ac::binding_show(&profile, epoch)),
        set(format!("session.{th}.{provider}"), session_new.clone()), set(format!("seen.{th}.{provider}"), seen.to_string()),
        json!({"$":"SessionSet","id":th,"session":session_new,"at":at})]);
    if count != seen {
        let missed = (count - seen.min(count)).min(128);
        let why = "Selected another account. Recent conversation since this account last participated (at most 128 items); use thread_read with threadId ".to_string() + &th + " for earlier history:";
        ev.push(json!({"$":"TurnQueued","thread":th,"msg":format!("handoff:switch:account{at}-{n}"),"text":hand_text(&why, &cx.entries[..missed.min(cx.entries.len())], &th),"at":at}));
    }
    let name = if profile.is_empty() { "Default account".to_string() } else { prof["label"].as_str().unwrap_or("account").to_string() };
    let said = s(p, "say", "");
    let summary = match by { "person" => format!("Selected {name}; conversation context is handed to this account with the next message."), "agent" | "bot" => format!("{} selected {name}; conversation context is handed to this account with the next message.", s(p, "who", "An agent")), _ => said.clone() };
    if !summary.is_empty() { ev.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("account", at, n + 1),"tone":"info","kind":"account","summary":summary,"at":at})) }
    ev.extend(after);
    Some(Plan { events: ev, reply: ok(id), stop: Some(th), wt: None, scratch: None, act: None })
}

const NO_ACCOUNT: &str = "{\"id\":\"\",\"epoch\":0}";
/// Accounts.created.one for a ThreadCreated: a thread of a provider with accounts starts on the default account.
pub fn account_created(f: &Fold, th: &str, provider: &str, at: u64) -> Vec<Value> {
    if provider != "claude" && provider != "codex" { return vec![] }
    vec![json!({"$":"SettingSet","key":format!("account.thread.{th}.{provider}"),"value":f.get(&format!("account.default.{provider}"), NO_ACCOUNT),"at":at})]
}
/// Accounts.created.one for a ThreadForked: both bindings copy the source's.
fn account_forked(f: &Fold, th: &str, src: &str, at: u64) -> Vec<Value> {
    ["claude", "codex"].iter().map(|p| json!({"$":"SettingSet","key":format!("account.thread.{th}.{p}"),"value":f.get(&format!("account.thread.{src}.{p}"), NO_ACCOUNT),"at":at})).collect()
}

/// Hub.root.norm: a root without trailing slashes ("/" stays).
pub fn norm_root(r: &str) -> String {
    if !(r.len() > 1 && r.ends_with('/')) { return r.to_string() }
    // Folder.root.fin: nothing left is "/", a bare drive ("C:") keeps its root slash
    let t = r.trim_end_matches('/');
    let b = t.as_bytes();
    if t.is_empty() { "/".into() } else if b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':' { format!("{t}/") } else { t.to_string() }
}

/// Hub.project_add (`project.create`): a folder becomes a project once (the same root twice is refused; a removed project's folder brings that
/// project back with its threads); a project made afresh starts with a thread, made as thread.create makes one.
pub fn project_create(f: &Fold, at: u64, id: &Value, p: &Value, root: &str) -> Plan {
    let refuse = |m: String| Plan { events: vec![], reply: err(id, &m), stop: None, wt: None, scratch: None, act: None };
    if root.trim().is_empty() { return refuse("no folder given".into()) }
    let root_n = norm_root(root);
    if f.projects.values().any(|x| !x.removed && norm_root(&x.root) == root_n) { return refuse(format!("already a project: {root}")) }
    let title = p.get("title").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| { let b = root.trim_end_matches(crate::plat::fs::SEPS).rsplit(crate::plat::fs::SEPS).next().unwrap_or(root); b.to_string() });
    let n = f.n();
    let old = f.projects.iter().find(|(_, x)| x.removed && norm_root(&x.root) == root_n).map(|(i, _)| i.clone());
    let mut pl = match old {
        Some(pid) => Plan { events: vec![json!({"$":"ProjectCreated","id":pid,"title":title,"root":root_n,"at":at})], reply: ok(id), stop: None, wt: None, scratch: None, act: None },
        None => {
            let pid = hub_id("p", at, n);
            let mut pl = create(f, at, id, &json!({"project":pid,"title":"New thread"}), Some((&root_n, n + 1)), None).unwrap();
            pl.events.insert(0, json!({"$":"ProjectCreated","id":pid,"title":title,"root":root_n,"at":at})); pl
        }
    };
    pl.events = step_tail(pend_fix(f, pl.events), at); pl
}


/// AG.Title.clean: the writer's answer as a title: its first line without quotes, markdown, a "Title:" label or a trailing period, spaces collapsed, at most 60 characters.
pub fn title_clean(answer: &str) -> String {
    let first = answer.trim().lines().next().unwrap_or("").trim().to_string();
    let bare: String = first.chars().filter(|c| !matches!(c, '`' | '"' | '*')).collect();
    let unhash = bare.trim_start_matches(|c| c == '#' || c == ' ').to_string();
    let prefixed = if unhash.to_lowercase().starts_with("title:") { unhash.chars().skip(6).collect::<String>().trim().to_string() } else { unhash };
    let unq = if prefixed.chars().count() >= 2 && prefixed.starts_with('\'') && prefixed.ends_with('\'') { prefixed.chars().skip(1).take(prefixed.chars().count() - 2).collect::<String>().trim().to_string() } else { prefixed };
    let squash = unq.replace('\t', " ").split(' ').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
    let unp = if squash.ends_with('.') && !squash.ends_with("..") { squash.chars().take(squash.chars().count() - 1).collect::<String>().trim().to_string() } else { squash };
    if unp.chars().count() > 60 { format!("{}...", unp.chars().take(57).collect::<String>()) } else { unp }
}

/// Home.start: one real conversation and its first message, made atomically; the retained message id resolves to the same thread on every retry.
fn start(f: &Fold, at: u64, id: &Value, p: &Value) -> Option<Plan> {
    let (project, msg, text) = (s(p, "project", ""), s(p, "msg", ""), s(p, "text", ""));
    let refuse = |m: &str| Some(Plan { events: vec![], reply: err(id, m), stop: None, wt: None, scratch: None, act: None });
    let live_root = f.projects.get(&project).filter(|x| !x.removed).map_or(false, |x| !x.root.is_empty());
    let token = !msg.is_empty() && msg.chars().count() <= 160 && msg.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !(live_root && token && !text.trim().is_empty()) { return refuse("Choose a project and enter a message before starting a thread.") }
    let th = format!("home-{msg}");
    if f.threads.contains_key(&th) {
        let ok = f.threads.get(&th).map_or(false, |t| !t.deleted && t.project == project);
        // a retry finds the thread made: the message goes in if it never arrived (the hub died, or turn.start failed, between the two)
        let posted = f.home_msgs.contains(&format!("{th}\u{1}{msg}"));
        return if ok { Some(Plan { events: vec![], reply: made(id, &th), stop: None, wt: None, scratch: None, act: (!posted).then(|| ("post", th.clone(), text.clone())) }) } else { refuse("This request belongs to a different or removed conversation. Open that conversation before retrying.") };
    }
    let title: String = text.chars().take(80).collect();
    let mut pl = create(f, at, id, &json!({"project": project, "title": title}), None, Some(&th))?;
    // the first message is the agents' turn.start (queued while the worktree is made)
    pl.act = Some(("post", th.clone(), text)); Some(pl)
}

#[cfg(test)] mod buy_tests {
    use super::*; use super::super::fold::{P, T};
    fn fold() -> Fold { let mut f = Fold::default(); f.projects.insert("p".into(), P { title: "Lamp".into(), root: "/r".into(), ..Default::default() });
        f.threads.insert("t1".into(), T { project: "p".into(), title: "Work".into(), provider: "claude".into(), model: "m".into(), env: "worktree".into(), branch: "b".into(), worktree: "/r/wt".into(), ..Default::default() }); f }
    fn on() -> Buy { Buy { on: true, ..Default::default() } }
    fn go(f: &Fold, at: u64, b: &Buy) -> Plan { buy(f, at, &json!(1), "t1", b).unwrap() }
    #[test] fn a_buy_thread_works_in_its_sources_folder_and_says_the_persons_buy_it() {
        let p = go(&fold(), 100, &on()); let th = p.reply["thread"].as_str().unwrap().to_string();
        let c = &p.events[0]; assert_eq!((c["title"].as_str(), c["env"].as_str(), c["branch"].as_str(), c["worktree"].as_str()), (Some("Buy: Lamp"), Some("worktree"), Some("b"), Some("/r/wt")));
        assert!(p.events.iter().any(|e| e["key"] == format!("buy.{th}") && e["value"] == "t1")); assert!(p.events.iter().any(|e| e["key"] == "buy.last.t1" && e["value"] == th.as_str()));
        assert_eq!(p.act, Some(("post", th, "Buy it".to_string()))); assert!(p.wt.is_none(), "no worktree of its own");
    }
    #[test] fn a_source_with_no_worktree_of_its_own_runs_in_the_project_folder() {
        let mut f = fold(); let t = f.threads.get_mut("t1").unwrap(); t.worktree.clear(); assert_eq!(go(&f, 100, &on()).events[0]["env"], "local", "marked worktree with none it would wait forever");
    }
    #[test] fn a_projects_next_buy_thread_is_numbered() {
        let mut f = fold(); assert_eq!(buy_title(&f, "p"), "Buy: Lamp");
        f.threads.insert("b1".into(), T { project: "p".into(), title: "Buy: Lamp".into(), ..Default::default() }); assert_eq!(buy_title(&f, "p"), "Buy: Lamp · 2");
        f.threads.insert("b2".into(), T { project: "p".into(), title: "Buy: Lamp · 2".into(), ..Default::default() }); assert_eq!(buy_title(&f, "p"), "Buy: Lamp · 3");
        f.threads.insert("o".into(), T { project: "q".into(), title: "Buy: Lamp".into(), ..Default::default() }); assert_eq!(buy_title(&f, "p"), "Buy: Lamp · 3", "another project's do not count");
    }
    #[test] fn a_rapid_repeat_opens_the_thread_just_made_and_a_later_one_is_a_new_order() {
        let mut f = fold(); f.threads.insert("t100-1".into(), T { project: "p".into(), title: "Buy: Lamp".into(), ..Default::default() });
        let b = Buy { on: true, last: "t100-1".into(), ..Default::default() };
        let p = go(&f, 102, &b); assert!(p.events.is_empty()); assert_eq!(p.reply["thread"], "t100-1");
        assert!(!go(&f, 103, &b).events.is_empty(), "three seconds on is a new order"); assert!(!buy_recent("", 100)); assert!(!buy_recent("improvement-x", 100));
        f.threads.get_mut("t100-1").unwrap().deleted = true; assert!(!go(&f, 101, &b).events.is_empty(), "a deleted one is not opened");
    }
    #[test] fn nothing_is_made_with_ordering_off_for_a_deleted_thread_or_from_a_buy_thread() {
        let mut f = fold(); let off = go(&f, 100, &Buy::default()); assert!(off.events.is_empty()); assert_eq!(off.reply["error"], "Ordering is off: turn it on in Settings to buy");
        let again = go(&f, 100, &Buy { on: true, buying: true, ..Default::default() }); assert!(again.events.is_empty() && again.act.is_none()); assert_eq!(again.reply["thread"], "t1");
        f.threads.get_mut("t1").unwrap().deleted = true; assert_eq!(go(&f, 100, &on()).reply["error"], "no such thread");
    }
}

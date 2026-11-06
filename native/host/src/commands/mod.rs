//! Thread, project and queue requests: `thread.*`, `project.*` and `queue.*` from src/core/hub.bend's `Hub.request.k`
//! (and `Hub.nav`), as pure planners over the log fold (fold.rs) that return the events the Bend hub logs and the reply it
//! sends; the handler commits them with `Hub::commit_many` and acknowledges after that returns. Field order is the codec's
//! (core/codec.bend `Change.encode`), so a log either hub wrote reads in the other.
pub mod fold;
pub mod key;
pub mod plan;
pub mod actions;
pub mod fs;
pub mod reorder;
pub mod home;
pub mod seam;
pub mod worktree;
#[cfg(test)]
mod tests;

use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

pub use plan::{plan, Ctx, Line, Plan};

pub fn start(hub: Arc<Hub>) { start_with(hub, None) }
/// `seam`: the agents module to call instead of the process-wide one (tests that run a real one).
pub fn start_with(hub: Arc<Hub>, seam: Option<Arc<dyn seam::Seam>>) {
    hub.register_rpc(Arc::new(Commands { fold: { let f = Arc::new(tokio::sync::Mutex::new(fold::Fold::load(&hub.store))); let f2 = f.clone(); crate::ckpt::register(move |store| f2.blocking_lock().save(store)); f }, seam }));
}

pub fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

struct Commands { fold: Arc<tokio::sync::Mutex<fold::Fold>>, seam: Option<Arc<dyn seam::Seam>> }

impl Commands { fn agents(&self) -> Option<Arc<dyn seam::Seam>> { self.seam.clone().or_else(seam::current) } }

impl RpcHandler for Commands {
    fn methods(&self) -> &'static [&'static str] { &["thread.pin", "thread.unpin", "thread.settle", "thread.unsettle", "thread.archive", "thread.unarchive", "thread.rename", "thread.delete", "thread.snooze",
        "thread.reorder", "thread.detach", "thread.reparent", "thread.create", "thread.modes", "thread.effort", "thread.provider", "accounts.select", "thread.fork", "thread.back", "project.remove", "project.rename",
        "project.reorder", "project.move", "project.add", "project.new", "project.create", "project.start", "project.home", "queue.edit", "queue.remove", "queue.move", "queue.steer", "queue.resume", "fs.list", "fs.mkdir", "fs.find"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            // one request at a time: the plan reads the fold the commit then moves on
            // the picker's folder requests (FsJob): the answer carries `result`, ok when it holds no `error`
            let answer = |r: Value| vec![json!({"t":"reply","id":id,"ok":r.get("error").is_none(),"result":r})];
            let path = p.get("path").and_then(Value::as_str).unwrap_or("").to_string();
            if method == "project.home" {
                // Home.request: a project that exists and is not removed; the snapshot is read off the log's folder (the Git job "project-home")
                let root = { let m = hub.model().await; m.projects.get(p.get("project").and_then(Value::as_str).unwrap_or("")).filter(|x| !x.removed).map(|x| x.root.clone()).unwrap_or_default() };
                if root.is_empty() { return vec![json!({"t":"reply","id":id,"ok":false,"error":"Choose an existing project."})] }
                let r = home::read(&root, p).await;
                return vec![json!({"t":"reply","id":id,"ok":true,"result":r})]; // GitJob.reply(..., True, r): the error is inside the result
            }
            if method == "accounts.select" && !crate::settings::accounts_hub::is_granted(who) { return vec![json!({"t":"reply","id":id,"ok":false,"error":crate::settings::accounts::GATE})] }
            let create_in: Option<String> = match method {
                "fs.list" => return answer(fs::list(&path).await),
                "fs.mkdir" => return answer(fs::mkdir(&path).await),
                "fs.find" => return answer(fs::find().await),
                "project.add" | "project.new" => {
                    let r = if method == "project.add" { fs::resolve(&path).await } else { fs::make(&path).await };
                    match r { Ok(d) if fs::is_dir(&d) => Some(d), Ok(d) => return answer(json!({"error": format!("invalid project folder: {d}")})), Err(e) => return answer(e) }
                }
                "project.create" => Some(path.clone()),
                _ => None,
            };
            // the conversation is read before the fold is taken: a long thread must not hold up every command
            let cx = if plan::needs_entries(method) { entries_of(hub, &p["thread"].as_str().unwrap_or("").to_string()).await } else { Ctx::default() };
            let mut f = self.fold.lock().await;
            // thread.create with a creation key and a base (an Improvements send): the base must be a local branch of the project's repository, else the request is refused and no thread is made
            if method == "thread.create" && p["creationKey"].as_str().map_or(false, |k| !k.is_empty()) { if let Some(base) = p["base"].as_str().filter(|b| !b.is_empty()) {
                let _ = f.sync(&hub.store);
                let root = f.projects.get(p["project"].as_str().unwrap_or("")).filter(|x| !x.removed).map(|x| x.root.clone()).unwrap_or_default();
                if let Err(why) = check_base(&root, base).await { return vec![json!({"t":"reply","id":id,"ok":false,"error":why})] }
            } }
            if let Err(e) = f.sync(&hub.store) { return vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")})] }
            f.alloc = Some(hub.alloc_n(4) as usize); // p<at>-<n>, t<at>-<n+1>, g<at>-<n+3>: a number no other request shares
            if let Some(root) = create_in {
                let pl = plan::project_create(&f, now_s(), id, p, &root);
                if !pl.events.is_empty() && hub.commit_many(&pl.events).await.is_err() { return vec![json!({"t":"reply","id":id,"ok":false,"error":"store write failed"})] }
                if let Some((th, r, mode, src, op)) = pl.wt { let (pv, bd) = capture(&f, &th, &pl.events); worktree::spawn(hub.clone(), self.fold.clone(), th, r, mode, src, pv, bd, op) }
                return vec![pl.reply];
            }
            let Some(mut pl) = plan(&f, now_s(), id, method, p, &cx) else { return vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown method"})] };
            // Retire.archive: a running turn is interrupted first (the stop logged as Stop logs it), then AgentStop every time so an idle
            // process goes too; the agents module owns both (`agents.stop` when it registers one)
            if let Some(th) = pl.stop.clone() {
                let running = f.threads.get(&th).map_or(false, |t| t.turn == "running");
                if running {
                    if let Some(s) = self.agents() { s.interrupt(&th).await }
                    if let Err(e) = f.sync(&hub.store) { return vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")})] }
                    if let Some(again) = plan(&f, now_s(), id, method, p, &cx) { pl = again }
                }
                if let Some(s) = self.agents() { s.stop(&th).await }
            }
            if !pl.events.is_empty() && hub.commit_many(&pl.events).await.is_err() { return vec![json!({"t":"reply","id":id,"ok":false,"error":"store write failed"})] }
            // BotFx gone (thread.delete): the thread's browser page closes (the term lane's `browser.close`; no events)
            let wtc = pl.wt.clone().map(|(th, root, mode, src, op)| { let (pv, bd) = capture(&f, &th, &pl.events); (th, root, mode, src, pv, bd, op) });
            drop(f); // what follows may wait on an agent or a helper: the next command must not wait with it
            // BotFx gone (thread.delete): the page closes after the reply, in a task (a slow helper must not hold anything)
            if method == "thread.delete" { if let Some(h) = hub.rpc_for("browser.close") { let (hub, th) = (hub.clone(), p["thread"].as_str().unwrap_or("").to_string()); tokio::spawn(async move { let _ = h.call(&hub, &Caller { gen: String::new(), client: 0 }, &json!(0), "browser.close", &json!({"thread": th})).await; }); } }
            // BotFx scratch (an out, so after the changes): the folder exists before the reply, and before anything runs in it
            if let Some(th) = pl.scratch.clone() { scratch_folder(hub, &th); }
            if let Some((th, root, mode, src, pv, bd, op)) = wtc { worktree::spawn(hub.clone(), self.fold.clone(), th, root, mode, src, pv, bd, op) }
            // Home.start: the first message is a turn.start of the new thread (the agents module posts it, queued while the worktree is made); its failure is the reply's
            if let Some(("post", th, text)) = pl.act.clone() {
                let msg = th.strip_prefix("home-").unwrap_or("").to_string();
                let r = match hub.rpc_for("turn.start") { Some(h) => h.call(hub, who, &json!(0), "turn.start", &json!({"thread": th, "text": text, "msg": msg})).await.into_iter().next().unwrap_or(Value::Null), None => json!({"ok": false, "error": "no agent module is running"}) };
                if r["ok"] != true { return vec![json!({"t":"reply","id":id,"ok":false,"error":r["error"].as_str().unwrap_or("the message could not be delivered; send it again")})] }
            } else if let Some((kind, th, text)) = pl.act.clone() {
                if let Some(s) = self.agents() { match kind { "steer" => s.steer(&th, &text).await, "start_one" => s.start_one(&th, &text).await, _ => s.start_queued(&th, &text).await } }
            }
            vec![pl.reply]
        })
    }
}

/// The conversation of a thread, newest first, for a handoff to another account (settings/driver.rs).
pub async fn entries(hub: &Arc<Hub>, th: &str) -> Option<Ctx> { Some(entries_of(hub, th).await) }
pub use plan::hand_text;
/// A thread's own entries, newest first (core/handoff.bend `Hand.mine`): the rows filed under it that it wrote, not the messages it sent elsewhere.
async fn entries_of(hub: &Arc<Hub>, th: &str) -> Ctx { entries_upto(hub, th, usize::MAX).await }
/// The newest `max` or so entries (whole pages of 32), newest first.
async fn entries_upto(hub: &Arc<Hub>, th: &str, max: usize) -> Ctx {
    let mut out = vec![]; let mut before = 0u64;
    loop {
        let (page, rows) = { let m = hub.model().await; match m.page(&hub.store, th, before, 32) {
            Ok(p) => { // the page cuts activity text at 513 characters; the handoff cuts its own (Hand.line)
                let rows: Vec<_> = p.rows.iter().filter(|r| p.ids.contains(&r.ord.to_string()) && r.thread == th)
                    .filter_map(|r| m.row(&hub.store, crate::model::Ent { seq: r.seq as u32, ord: r.ord as u32 }, th, false).ok()).collect();
                (p, rows) }
            Err(_) => break } };
        for r in rows { out.push(Line { msg: r.kind == "msg", role: r.role.clone(), kind: r.akind.clone(), text: r.text.clone() }) }
        if out.len() >= max { break }
        match page.ids.last().and_then(|l| l.parse::<u64>().ok()) { Some(l) if page.ids.len() == 32 && l > 1 => before = l, _ => break }
    }
    out.truncate(max);
    Ctx { entries: out }
}

/// The recent conversation of a thread whose agent session is gone, as the text that goes ahead of the message being run again (at most 128 items, the message itself left out):
/// "" when the thread has nothing yet. The same hand-over a fork or a provider switch makes (plan.rs `hand_text`).
pub(crate) async fn lost_session_context(hub: &Arc<Hub>, th: &str, sent: &str) -> String {
    let mut es = entries_upto(hub, th, 129).await.entries;
    if es.first().map_or(false, |l| l.msg && l.role != "assistant" && l.text.trim() == sent.trim()) { es.remove(0); } // the message about to run again is the newest entry
    es.truncate(128);
    if es.is_empty() { return String::new() }
    plan::hand_text(&format!("The agent session this thread was resuming is no longer available, so you start a new one. Recent conversation (at most 128 items), oldest first; use thread_read with threadId {th} for earlier history:"), &es, th)
}

/// BotJob.scratch: `<home>/scratch/<thread>` made at once (a name that is not a plain folder name makes nothing), a git repository, and the thread's folder recorded.
fn scratch_folder(hub: &Arc<Hub>, th: &str) {
    let plain = !th.is_empty() && !th.starts_with('.') && th.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !plain { return }
    let path = hub.cfg.home.join("scratch").join(th);
    if std::fs::create_dir_all(&path).is_err() { return }
    let (hub, th, path) = (hub.clone(), th.to_string(), path.to_string_lossy().into_owned());
    tokio::spawn(async move {
        let _ = crate::files::git::proc_ro("git", &["init", "-q"], &path, &[]).await; // inside <home>/scratch: a home write
        let _ = hub.commit(&json!({"$":"WorktreeSet","id":th,"branch":"","worktree":path,"at":now_s()})).await;
    });
}

/// The provider and account binding a worktree job starts with (Hub.job.ready compares them when it reports): the thread's, as this step's events set them.
fn capture(f: &fold::Fold, th: &str, events: &[Value]) -> (String, String) {
    let provider = events.iter().find(|e| e["$"] == "ThreadCreated" && e["id"] == th).and_then(|e| e["provider"].as_str()).unwrap_or("").to_string();
    let key = format!("account.thread.{th}.{provider}");
    let binding = events.iter().find(|e| e["key"] == key.as_str()).and_then(|e| e["value"].as_str()).unwrap_or_else(|| f.setting(&key)).to_string();
    (provider, binding)
}

/// A base is a plain local branch name that exists in the project's repository (never origin's default, never "").
async fn check_base(root: &str, base: &str) -> Result<(), String> {
    let plain = !base.starts_with('-') && !base.contains("..") && !base.ends_with('/') && !base.ends_with(".lock") && base.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'));
    if !plain { return Err(format!("base \"{base}\" is not a local branch name")) }
    if root.is_empty() { return Err("Choose an existing project and retry this request.".into()) }
    let r = crate::files::git::run_ro(root, &["rev-parse", "--verify", "-q", &format!("refs/heads/{base}^{{commit}}")]).await;
    if r.code == 0 { Ok(()) } else { Err(format!("base branch \"{base}\" does not exist in this project")) }
}

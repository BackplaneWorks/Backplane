//! Thread, project and queue requests: `thread.*`, `project.*` and `queue.*` from src/core/hub.bend's `Hub.request.k`
//! (and `Hub.nav`), as pure planners over the log fold (fold.rs) that return the events the Bend hub logs and the reply it
//! sends; the handler commits them with `Hub::commit_many` and acknowledges after that returns. Field order is the codec's
//! (core/codec.bend `Change.encode`), so a log either hub wrote reads in the other.
pub mod fold;
pub mod key;
pub mod plan;
pub mod fs;
pub mod seam;
pub mod worktree;
#[cfg(test)]
mod tests;

use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

pub use plan::{plan, Ctx, Line, Plan};

pub fn start(hub: Arc<Hub>) {
    hub.register_rpc(Arc::new(Commands { fold: Arc::new(tokio::sync::Mutex::new(fold::Fold::default())) }));
}

pub fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

struct Commands { fold: Arc<tokio::sync::Mutex<fold::Fold>> }

impl RpcHandler for Commands {
    fn methods(&self) -> &'static [&'static str] { plan::METHODS }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            // one request at a time: the plan reads the fold the commit then moves on
            // the picker's folder requests (FsJob): the answer carries `result`, ok when it holds no `error`
            let answer = |r: Value| vec![json!({"t":"reply","id":id,"ok":r.get("error").is_none(),"result":r})];
            let path = p.get("path").and_then(Value::as_str).unwrap_or("").to_string();
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
            if let Err(e) = f.sync(&hub.store) { return vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")})] }
            f.alloc = Some(hub.alloc_n(4) as usize); // p<at>-<n>, t<at>-<n+1>, g<at>-<n+3>: a number no other request shares
            if let Some(root) = create_in {
                let pl = plan::project_create(&f, now_s(), id, p, &root);
                if !pl.events.is_empty() && hub.commit_many(&pl.events).await.is_err() { return vec![json!({"t":"reply","id":id,"ok":false,"error":"store write failed"})] }
                if let Some((th, r, mode, src)) = pl.wt { let (pv, bd) = capture(&f, &th, &pl.events); worktree::spawn(hub.clone(), self.fold.clone(), th, r, mode, src, pv, bd) }
                return vec![pl.reply];
            }
            let Some(mut pl) = plan(&f, now_s(), id, method, p, &cx) else { return vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown method"})] };
            // Retire.archive: a running turn is interrupted first (the stop logged as Stop logs it), then AgentStop every time so an idle
            // process goes too; the agents module owns both (`agents.stop` when it registers one)
            if let Some(th) = pl.stop.clone() {
                let running = f.threads.get(&th).map_or(false, |t| t.turn == "running");
                if running {
                    if let Some(s) = seam::current() { s.interrupt(&th).await }
                    if let Err(e) = f.sync(&hub.store) { return vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{e:#}")})] }
                    if let Some(again) = plan(&f, now_s(), id, method, p, &cx) { pl = again }
                }
                if let Some(s) = seam::current() { s.stop(&th).await }
            }
            if !pl.events.is_empty() && hub.commit_many(&pl.events).await.is_err() { return vec![json!({"t":"reply","id":id,"ok":false,"error":"store write failed"})] }
            // BotFx scratch (an out, so after the changes): the folder exists before the reply, and before anything runs in it
            if let Some(th) = pl.scratch.clone() { scratch_folder(hub, &th); }
            if let Some((kind, th, text)) = pl.act.clone() {
                if let Some(s) = seam::current() { match kind { "steer" => s.steer(&th, &text).await, "start_one" => s.start_one(&th, &text).await, _ => s.start_queued(&th, &text).await } }
            }
            if let Some((th, root, mode, src)) = pl.wt.clone() { let (pv, bd) = capture(&f, &th, &pl.events); worktree::spawn(hub.clone(), self.fold.clone(), th, root, mode, src, pv, bd) }
            vec![pl.reply]
        })
    }
}

/// A thread's own entries, newest first (core/handoff.bend `Hand.mine`): the rows filed under it that it wrote, not the messages it sent elsewhere.
async fn entries_of(hub: &Arc<Hub>, th: &str) -> Ctx {
    let mut out = vec![]; let mut before = 0u64;
    loop {
        let (page, rows) = { let m = hub.model().await; match m.page(&hub.store, th, before, 32) {
            Ok(p) => { // the page cuts activity text at 513 characters; the handoff cuts its own (Hand.line)
                let rows: Vec<_> = p.rows.iter().filter(|r| p.ids.contains(&r.ord.to_string()) && r.thread == th)
                    .filter_map(|r| m.row(&hub.store, crate::model::Ent { seq: r.seq as u32, ord: r.ord as u32 }, th, false).ok()).collect();
                (p, rows) }
            Err(_) => break } };
        for r in rows { out.push(Line { msg: r.kind == "msg", role: r.role.clone(), kind: r.akind.clone(), text: r.text.clone() }) }
        match page.ids.last().and_then(|l| l.parse::<u64>().ok()) { Some(l) if page.ids.len() == 32 && l > 1 => before = l, _ => break }
    }
    Ctx { entries: out }
}

/// BotJob.scratch: `<home>/scratch/<thread>` made at once (a name that is not a plain folder name makes nothing), a git repository, and the thread's folder recorded.
fn scratch_folder(hub: &Arc<Hub>, th: &str) {
    let plain = !th.is_empty() && !th.starts_with('.') && th.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !plain { return }
    let path = hub.cfg.home.join("scratch").join(th);
    if std::fs::create_dir_all(&path).is_err() { return }
    let (hub, th, path) = (hub.clone(), th.to_string(), path.to_string_lossy().into_owned());
    tokio::spawn(async move {
        let _ = crate::files::git::proc("git", &["init", "-q"], &path, &[]).await;
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

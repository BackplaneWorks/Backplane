//! M6 files stream: the Files tab (`files.dir`), `/img`, and what follows (plots, git, KiCad IPC).
//! Registers its handlers with the hub through api.rs; the one line in main.rs is `files::start`.
pub mod dir;
pub mod fm;
pub mod git;
pub mod misc;
pub mod serve;
pub mod threadpath;
#[path = "../kicad/mod.rs"]
pub mod kicad;

use crate::{api::{Caller, Fut, HttpHandler, HttpReply, RpcHandler}, auth::Req, hub::Hub};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};

pub fn start(hub: Arc<Hub>) {
    hub.register_http(Arc::new(Img));
    hub.register_http(Arc::new(Downloads));
    hub.register_rpc(Arc::new(Files));
    hub.register_rpc(Arc::new(GitRpc));
    hub.register_rpc(Arc::new(misc::Misc));
    kicad::start(&hub);
}

/// ThreadPath.of for the read model (threadpath.rs): the folder of a thread or `project-home-<id>`, None when it does not resolve.
pub async fn root_of(hub: &Arc<Hub>, id: &str) -> Option<PathBuf> {
    let m = hub.model().await;
    threadpath::resolve(id,
        |i| m.threads.get(i).map(|t| threadpath::T { project: &t.project, worktree: &t.worktree, deleted: t.deleted }),
        |i| m.projects.get(i).map(|p| threadpath::P { root: &p.root, removed: p.removed })).map(PathBuf::from)
}

struct Img;
impl HttpHandler for Img {
    fn routes(&self) -> &'static [&'static str] { &["/img"] }
    /// Img.serve: the hub already checked the pairing token. The path is `from_uri`-decoded, must be
    /// viewable (image, PDF or text note, absolute or `~/`), and is read without following a final link.
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            let path = fm::from_uri(&req.param("path"));
            if !fm::viewable(&path) { return HttpReply::text("404 Not Found", "not an image") }
            let real = match path.strip_prefix("~/") {
                Some(rest) => format!("{}/{rest}", std::env::var("HOME").unwrap_or_default()),
                None => path.clone(),
            };
            let (ct, range) = (fm::ctype(&path), req.header("range").to_string());
            tokio::task::spawn_blocking(move || serve::send(&real, ct, &range)).await.unwrap_or_else(|_| HttpReply::text("500 Internal Server Error", "cannot read preview"))
        })
    }
}

/// /files/get and /files/zip (the web's downloads) and /view/* (a window on another machine): no native client calls them, so the Rust hub
/// serves none; it still keeps the boundary: a path outside the project is 400, a thread with no folder is 404 "no such thread" (FilesHttp.safe,
/// FilesHttp.root), and only then 501.
struct Downloads;
impl HttpHandler for Downloads {
    fn routes(&self) -> &'static [&'static str] { &["/files/get", "/files/zip", "/view/get", "/view/ver", "/view/git"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            let thread = fm::from_uri(&req.param("thread"));
            if req.route().starts_with("/files/") && !fm::files_safe(&fm::from_uri(&req.param("path"))) { return HttpReply::text("400 Bad Request", "that path is not in this project") }
            if root_of(hub, &thread).await.is_none() { return HttpReply::text("404 Not Found", "no such thread") }
            HttpReply::text("501 Not Implemented", "this route is not served by this hub")
        })
    }
}

struct Files;
impl RpcHandler for Files {
    fn methods(&self) -> &'static [&'static str] { &["files.dir"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            match method {
                "files.dir" => vec![files_dir(hub, id, p).await],
                _ => vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown method"})],
            }
        })
    }
}

/// DirJob.sent: `{fmEntries, fmDir, fmRoot, thread}` on success; on failure ok=false with
/// `{text, fmEntries:"", fmError, fmDir, thread}`.
async fn files_dir(hub: &Arc<Hub>, id: &Value, p: &Value) -> Value {
    let (thread, rel) = (p.get("thread").and_then(Value::as_str).unwrap_or(""), p.get("path").and_then(Value::as_str).unwrap_or(""));
    // Hub.dir.k: a path that fails Fm.files.safe is refused first, then a thread with no folder (Hub.folder.job)
    if !fm::files_safe(rel) { return json!({"t":"reply","id":id,"ok":false,"error":"that folder is not in this project"}) }
    let Some(root) = root_of(hub, thread).await else { return threadpath::refusal(id) };
    let (r, rel_o) = (root.clone(), rel.to_string());
    let listed = tokio::task::spawn_blocking(move || dir::list(&r, &rel_o)).await.unwrap_or(Err(dir::UNAVAILABLE));
    match listed {
        Ok(text) => json!({"t":"reply","id":id,"ok":true,"result":{"fmEntries":text,"fmDir":rel,"fmRoot":root.to_string_lossy(),"thread":thread}}),
        Err(e) => json!({"t":"reply","id":id,"ok":false,"result":{"text":e,"fmEntries":"","fmError":e,"fmDir":rel,"thread":thread}}),
    }
}

/// git.status, git.diff, git.revert, history.refs (server.bend's GitJob / RefsJob, hub.bend's Hub.more).
struct GitRpc;
impl RpcHandler for GitRpc {
    fn methods(&self) -> &'static [&'static str] { &["git.status", "git.diff", "git.revert", "history.refs", "git.act"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let raw = |k: &str, d: &str| match p.get(k) { Some(Value::String(x)) => x.clone(), Some(Value::Number(n)) => n.to_string(), _ => d.to_string() };
            let thread = s("thread");
            let reply = |ok: bool, r: Value| json!({"t":"reply","id":id,"ok":ok,"result":r});
            // Hub.git -> Hub.folder.job: with no resolved folder nothing is scheduled (law thread_folder_refuses_job)
            let Some(cwd_p) = root_of(hub, &thread).await else { return vec![threadpath::refusal(id)] };
            let cwd = cwd_p.to_string_lossy().into_owned();
            if method == "history.refs" {
                let out = git::refs(&cwd, &thread).await;
                let ls: Vec<&str> = out.split('\n').collect(); let at = |i: usize| ls.get(i).copied().unwrap_or("");
                let f = kicad::project::read(&cwd);
                return vec![reply(true, json!({"thread":thread,"git":at(0),"top":at(1),"refs":ls.iter().skip(2).copied().collect::<Vec<_>>().join("\n"),"pcb":f.pcb,"sch":f.sch}))];
            }
            let init = method == "git.act" && s("action") == "init";
            if !init && !git::is_repo(&cwd).await {
                return vec![if method == "git.diff" { reply(true, json!({"diff":"","thread":thread,"nogit":cwd})) } else { reply(false, json!({"text":format!("not a git repository: {cwd}")})) }];
            }
            match method {
                "git.status" => vec![reply(true, json!({"status":git::status(&cwd).await,"thread":thread}))],
                "git.diff" => { let r = git::diff(&cwd, &thread, &raw("from", "0"), &raw("to", "-1")).await; vec![reply(!git::failed(&r), json!({"diff":r,"thread":thread}))] }
                "git.act" => { // GitJob 5n / AgentJob: no message means the thread's title (the model-written message is the agents stream's)
                    let action = if p.get("action").is_some() { s("action") } else { "commit".into() };
                    let title = { let m = hub.model().await; m.threads.get(&thread).map(|t| t.title.clone()).unwrap_or_default() };
                    let message = match p.get("message").and_then(Value::as_str) { Some(m) if !m.is_empty() => m.to_string(), _ => if action == "init" { "start".into() } else if title.is_empty() { "Backplane: changes from this thread".into() } else { title } };
                    let r = git::act(&cwd, &action, &message, "", "").await;
                    note(hub, &thread, if git::failed(&r) { "error" } else { "info" }, &format!("{action}{}", if r.is_empty() { String::new() } else { format!(": {}", r.trim()) })).await;
                    vec![reply(!git::failed(&r), json!({"text":r,"thread":thread}))]
                }
                _ => { // git.revert
                    let n: u64 = raw("turn", "0").parse().unwrap_or(0);
                    let r = git::checkpoint_revert(&cwd, &thread, n).await;
                    let what = format!("reverted to turn {n}");
                    note(hub, &thread, if git::failed(&r) { "error" } else { "info" }, &format!("{what}{}", if r.is_empty() { String::new() } else { format!(": {}", r.trim()) })).await;
                    vec![reply(!git::failed(&r), json!({"text":r,"thread":thread}))]
                }
            }
        })
    }
}
/// GitJob.note: a line in the thread (hub.bend's thread.note: an ActivityLogged of kind "git").
pub async fn note(hub: &Arc<Hub>, thread: &str, tone: &str, summary: &str) {
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let seq = hub.store.len();
    let _ = hub.commit(&json!({"$":"ActivityLogged","thread":thread,"act":format!("g{at}-{seq}"),"tone":tone,"kind":"git","summary":summary,"at":at})).await;
}

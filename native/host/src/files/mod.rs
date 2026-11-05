//! M6 files stream: the Files tab (`files.dir`), `/img`, and what follows (plots, git, KiCad IPC).
//! Registers its handlers with the hub through api.rs; the one line in main.rs is `files::start`.
pub mod dir;
pub mod fm;
pub mod serve;

use crate::{api::{Caller, Fut, HttpHandler, HttpReply, RpcHandler}, auth::Req, hub::Hub};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};

pub fn start(hub: Arc<Hub>) {
    hub.register_http(Arc::new(Img));
    hub.register_rpc(Arc::new(Files));
}

/// Hub.files.of: the folder a thread's Files tab reads (its worktree, else its project's root); a project
/// id lists the project's folder. "" when the thread is unknown, deleted or its project is removed.
pub async fn root_of(hub: &Arc<Hub>, id: &str) -> PathBuf {
    let m = hub.model().await;
    if let Some(t) = m.threads.get(id) {
        if t.deleted { return PathBuf::new() }
        let p = m.projects.get(&t.project).filter(|p| !p.removed);
        return if !t.worktree.is_empty() { PathBuf::from(&t.worktree) } else { p.map(|p| PathBuf::from(&p.root)).unwrap_or_default() };
    }
    m.projects.get(id).filter(|p| !p.removed).map(|p| PathBuf::from(&p.root)).unwrap_or_default()
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
    // Hub.dir.k: a path that fails Fm.safe is refused before any job runs, with Reply.err
    if !fm::safe(rel) { return json!({"t":"reply","id":id,"ok":false,"error":"that folder is not in this project"}) }
    let root = root_of(hub, thread).await;
    let listed = if root.as_os_str().is_empty() { Err("no such folder") } else {
        let (r, rel) = (root.clone(), rel.to_string());
        tokio::task::spawn_blocking(move || dir::list(&r, &rel)).await.unwrap_or(Err("no such folder"))
    };
    match listed {
        Ok(text) => json!({"t":"reply","id":id,"ok":true,"result":{"fmEntries":text,"fmDir":rel,"fmRoot":root.to_string_lossy(),"thread":thread}}),
        Err(e) => json!({"t":"reply","id":id,"ok":false,"result":{"text":e,"fmEntries":"","fmError":e,"fmDir":rel,"thread":thread}}),
    }
}

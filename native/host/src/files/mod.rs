//! M6 files stream: the Files tab (`files.dir`), `/img`, and what follows (plots, git, KiCad IPC).
//! Registers its handlers with the hub through api.rs; the one line in main.rs is `files::start`.
pub mod dir;
pub mod export10;
pub mod attach;
pub mod attach_http;
pub mod fm;
pub mod imgroutes;
pub mod mimg;
pub mod ops;
pub mod watch_dir;
pub mod tools;
pub mod git;
pub mod misc;
pub mod rv;
pub mod serve;
pub mod threadpath;
pub mod webimg;
#[path = "../kicad/mod.rs"]
pub mod kicad;

use crate::{api::{Caller, Fut, HttpHandler, HttpReply, RpcHandler}, auth::Req, hub::Hub};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};

pub fn start(hub: Arc<Hub>) {
    watch_dir::start(hub.clone());
    hub.register_http(Arc::new(Img));
    hub.register_http(Arc::new(webimg::WebImg));
    hub.register_http(Arc::new(Downloads));
    hub.register_rpc(Arc::new(Files));
    hub.register_rpc(Arc::new(GitRpc));
    hub.register_rpc(Arc::new(misc::Misc));
    hub.register_rpc(Arc::new(rv::Rv));
    hub.register_rpc(Arc::new(export10::Export10));
    kicad::start(&hub);
    imgroutes::start(&hub);
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
                Some(rest) => format!("{}/{rest}", crate::plat::dirs::home_or_empty()),
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
            if root_of(hub, &thread).await.is_none() {
                // /view/*: Hub.bot.view's refusal; /files/*: FilesHttp.root
                return if req.route().starts_with("/view/") { HttpReply::text("403 Forbidden", "not a thread of this machine, or not a file the viewers read") } else { HttpReply::text("404 Not Found", "no such thread") };
            }
            if req.route() == "/files/get" {
                let (root, rel) = (root_of(hub, &thread).await.unwrap(), fm::from_uri(&req.param("path")));
                return match tokio::task::spawn_blocking(move || ops::read(&root, &rel).map(|b| (b, rel))).await {
                    Ok(Ok((b, rel))) => HttpReply { status: "200 OK", ctype: "application/octet-stream".into(), extra: format!("Content-Disposition: attachment; filename=\"{}\"\r\nX-Content-Type-Options: nosniff\r\n", fm::last(&rel).replace(['"', '\r', '\n', '\\'], "_")), body: b },
                    Ok(Err(e)) => HttpReply::text("404 Not Found", &e),
                    Err(_) => HttpReply::text("500 Internal Server Error", "cannot read file"),
                };
            }
            if req.route() == "/files/zip" {
                let (root, rel) = (root_of(hub, &thread).await.unwrap(), fm::from_uri(&req.param("path")));
                return match tokio::task::spawn_blocking(move || ops::zip_dir(&root, &rel).map(|b| (b, rel))).await {
                    Ok(Ok((b, rel))) => HttpReply { status: "200 OK", ctype: "application/zip".into(), extra: format!("Content-Disposition: attachment; filename=\"{}.zip\"\r\nX-Content-Type-Options: nosniff\r\n", { let n = fm::last(&rel); if n.is_empty() { "project" } else { n } }.replace(['"', '\r', '\n', '\\'], "_")), body: b },
                    Ok(Err(e)) => HttpReply::text("404 Not Found", &e),
                    Err(_) => HttpReply::text("500 Internal Server Error", "cannot zip"),
                };
            }
            HttpReply::text("501 Not Implemented", "this route is not served by this hub")
        })
    }
}

struct Files;
impl RpcHandler for Files {
    fn methods(&self) -> &'static [&'static str] { &["files.dir", "files.op", "files.read"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            match method {
                "files.dir" => vec![files_dir(hub, id, p).await],
                "files.op" | "files.read" => vec![files_op(hub, id, method, p).await],
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

/// git.status, git.diff, git.revert, history.refs.
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
                let f = kicad::canonical::for_cwd(&cwd); // history names main's canonical board, never the worktree twin
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
                    // GitJob.text.pr: a pull request just opened gets its parts change, after the answer's note
                    if !git::failed(&r) && git::last_line(&r).starts_with("pr: http") {
                        let w = rv::pr_note(&cwd, git::last_line(&r)[4..].trim()).await;
                        if !w.is_empty() { note(hub, &thread, "info", &w).await }
                    }
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
/// GitJob.note: a line in the thread.
pub async fn note(hub: &Arc<Hub>, thread: &str, tone: &str, summary: &str) {
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let seq = hub.store.len();
    let _ = hub.commit(&json!({"$":"ActivityLogged","thread":thread,"act":format!("g{at}-{seq}"),"tone":tone,"kind":"git","summary":summary,"at":at})).await;
}

/// PR #960 on the real handlers (8aa5a45900): every folder route, asked about a thread or project home that does not resolve, answers the one refusal and
/// schedules nothing; the same routes still work for a thread that does. The handlers are the registered ones, the hub is a real `Hub` on a temp home.
#[cfg(test)]
mod boundary_tests {
    use super::*;
    use crate::{hub::Cfg, presenter::Canned};
    /// a folder per call: tests of this module run side by side, and one sharing a folder wipes the other's files
    fn tmp(n: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("boundary-{n}")) }
    async fn world() -> (crate::hub::TestHub, crate::plat::tmp::TempDir) {
        let (home, proj) = (tmp("home"), tmp("proj"));
        std::fs::write(proj.join("a.txt"), "x").unwrap();
        let cfg = Cfg { home: home.to_path_buf(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        start(hub.clone());
        let root = proj.to_string_lossy().into_owned();
        let tc = |id: &str, project: &str, wt: &str| json!({"$":"ThreadCreated","id":id,"project":project,"title":"T","env":"local","branch":"","worktree":wt,"provider":"claude","model":"","at":2});
        hub.commit_many(&[json!({"$":"ProjectCreated","id":"p1","title":"P","root":root,"at":1}), json!({"$":"ProjectCreated","id":"p2","title":"Q","root":root,"at":1}), json!({"$":"ProjectCreated","id":"p3","title":"R","root":"relative/root","at":1}),
            tc("ok", "p1", ""), tc("gone", "p1", ""), tc("rm", "p2", ""), tc("badwt", "p1", "relative/wt"), tc("badroot", "p3", ""), tc("priv", "", ""),
            json!({"$":"ThreadDeleted","id":"gone","at":3}), json!({"$":"ProjectRemoved","id":"p2","at":3})]).await.unwrap();
        (crate::hub::TestHub { hub, _tmp: home }, proj)
    }
    const ROUTES: &[&str] = &["files.dir", "files.op", "files.read", "files.list", "git.status", "git.diff", "git.act", "git.revert", "history.refs", "history.resolve", "kicad.components", "boards.list", "mech.list", "mech.render",
        "review.bom", "review.checks", "review.tick", "review.comment", "review.release", "kicad.export10", "pdf.page", "file.open"];
    #[tokio::test] async fn every_folder_route_refuses_what_does_not_resolve() {
        let (hub, _proj) = world().await;
        let who = Caller { gen: "g".into(), client: 1 };
        let refusal = threadpath::refusal(&json!(9));
        // empty, unknown, deleted thread, removed project, relative worktree, relative project root, no project and no folder, unknown and removed project homes
        for thread in ["", "nope", "gone", "rm", "badwt", "badroot", "priv", "project-home-", "project-home-nope", "project-home-p2", "project-home-p3"] {
            for m in ROUTES {
                if matches!(*m, "pdf.page" | "file.open") { continue } // explicit-path viewer APIs, not thread lookups (the doc's /img note)
                if *m == "review.comment" && thread.is_empty() { continue } // judged first: an empty thread is a malformed comment, not a folder (Rv.comment.k)
                let Some(h) = hub.rpc_for(m) else { panic!("{m} is not registered") };
                let r = h.call(&hub, &who, &json!(9), m, &json!({"thread":thread,"path":"a.txt","action":"commit","message":"x","from":"0","to":"-1","a":"x","b":"y","ref":"HEAD","n":1,"op":"add","text":"x"})).await;
                assert_eq!(r, vec![refusal.clone()], "{m} for {thread:?}");
            }
        }
        // a path outside the project is refused first, as the JS hub's Hub.dir.k does
        let r = hub.rpc_for("files.dir").unwrap().call(&hub, &who, &json!(9), "files.dir", &json!({"thread":"","path":"../etc"})).await;
        assert_eq!(r[0]["error"], "that folder is not in this project");
    }
    #[tokio::test] async fn a_thread_that_resolves_still_works() {
        let (hub, proj) = world().await;
        let who = Caller { gen: "g".into(), client: 1 };
        for t in ["ok", "project-home-p1"] {
            let r = hub.rpc_for("files.dir").unwrap().call(&hub, &who, &json!(1), "files.dir", &json!({"thread":t,"path":""})).await;
            assert_eq!(r[0]["ok"], true, "{t}"); assert_eq!(r[0]["result"]["fmEntries"], "f\t1\ta.txt\n", "{t}");
            let r = hub.rpc_for("git.status").unwrap().call(&hub, &who, &json!(2), "git.status", &json!({"thread":t})).await;
            assert_ne!(r[0]["error"], threadpath::NO_FOLDER, "{t}");
        }
        assert_eq!(root_of(&hub, "ok").await.unwrap(), *proj);
        // an archived thread keeps its folder
        hub.commit(&json!({"$":"ThreadArchived","id":"ok","archived":true,"at":4})).await.unwrap();
        assert_eq!(root_of(&hub, "ok").await.unwrap(), *proj);
    }
    #[tokio::test] async fn files_op_writes_inside_the_project_and_files_get_and_zip_serve_it() {
        let (hub, proj) = world().await;
        let who = Caller { gen: "g".into(), client: 1 };
        let call = |m: &'static str, p: Value| { let (hub, who) = (hub.clone(), Caller { gen: "g".into(), client: 1 }); async move { hub.rpc_for(m).unwrap().call(&hub, &who, &json!(7), m, &p).await.remove(0) } };
        let _ = &who;
        assert_eq!(call("files.op", json!({"thread":"ok","op":"mkdir","path":"sub"})).await["ok"], true);
        let w = call("files.op", json!({"thread":"ok","op":"write","path":"sub/n.txt","data":"aGVsbG8="})).await; assert_eq!(w["ok"], true, "{w}");
        assert_eq!(call("files.op", json!({"thread":"ok","op":"write","path":"sub/n.txt","data":"aGVsbG8="})).await["ok"], false, "never replaces");
        assert_eq!(call("files.op", json!({"thread":"ok","op":"rename","path":"sub/n.txt","to":"m.txt"})).await["result"]["path"], "sub/m.txt");
        assert_eq!(call("files.read", json!({"thread":"ok","path":"sub/m.txt"})).await["result"]["text"], "hello");
        assert_eq!(call("files.op", json!({"thread":"ok","op":"write","path":"../x","data":"aA=="})).await["ok"], false);
        assert_eq!(call("files.op", json!({"thread":"nope","op":"mkdir","path":"z"})).await, threadpath::refusal(&json!(7)));
        let dl = |route: &'static str, path: &'static str| { let req = Req::new("GET", &format!("{route}?thread=ok&path={path}"), vec![]); let hub = hub.clone(); async move { hub.http_for(route).unwrap().call(&hub, &req).await } };
        let g = dl("/files/get", "sub/m.txt").await; assert!(g.status.starts_with("200") && g.body == b"hello", "{}", g.status);
        let z = dl("/files/zip", "sub").await; assert!(z.status.starts_with("200") && z.body.starts_with(b"PK"), "{}", z.status);
        assert!(dl("/files/get", "..%2Fetc").await.status.starts_with("400"));
        assert_eq!(call("files.op", json!({"thread":"ok","op":"delete","path":"sub"})).await["ok"], true); assert!(!proj.join("sub").exists());
    }
    #[tokio::test] async fn downloads_and_viewer_sources_keep_the_boundary() {
        let (hub, _proj) = world().await;
        let who = Caller { gen: "g".into(), client: 1 };
        let dl = |route: &str, thread: &str| { let req = Req::new("GET", &format!("{route}?thread={thread}&path=a.txt"), vec![]); let hub = hub.clone(); let route = route.to_string(); async move { let h = hub.http_for(&route).expect("route"); h.call(&hub, &req).await } };
        for t in ["", "nope", "gone", "rm", "badwt"] {
            for r in ["/files/get", "/files/zip"] { assert!(dl(r, t).await.status.starts_with("404"), "{r} {t:?}") }
            for r in ["/view/get", "/view/ver", "/view/git"] { assert!(dl(r, t).await.status.starts_with("403"), "{r} {t:?}") }
        }
        // kicad.watch: an active request with neither an absolute root nor an absolute source is refused; a stop is not
        let h = hub.rpc_for("kicad.watch").unwrap();
        for (root, path) in [("", ""), ("rel", ""), ("", "rel/a.kicad_pcb"), ("rel", "rel/a.kicad_pcb")] {
            let r = h.call(&hub, &who, &json!(3), "kicad.watch", &json!({"kind":"board","root":root,"path":path})).await;
            assert_eq!(r, vec![threadpath::refusal(&json!(3))], "{root:?} {path:?}");
        }
        assert_eq!(h.call(&hub, &who, &json!(4), "kicad.watch", &json!({"kind":""})).await[0]["ok"], true);
    }
    /// A copy writes nothing into a project folder: mech.render (renders/), review.tick (.backplane/checklist.md) and both intent writers
    /// (fab.intent, the project home's intent op) are refused, and the folder is byte for byte what it was. Run in a child with BACKPLANE_COPY=1 (copy::on reads it once).
    #[test] fn copy_writes_probe() {
        if std::env::var("BP_COPY_PROBE").ok().as_deref() != Some("1") { return }
        assert!(crate::copy::on());
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        rt.block_on(async {
            let (hub, proj) = world().await;
            std::fs::write(proj.join("m.step"), "ISO-10303-21;").unwrap(); std::fs::create_dir_all(proj.join(".backplane")).unwrap();
            fn snap(d: &std::path::Path) -> Vec<(String, Vec<u8>)> { let mut v = vec![]; fn go(d: &std::path::Path, root: &std::path::Path, v: &mut Vec<(String, Vec<u8>)>) { for e in std::fs::read_dir(d).unwrap().flatten() { let p = e.path(); if p.is_dir() { go(&p, root, v) } else { v.push((p.strip_prefix(root).unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap())) } } } go(d, d, &mut v); v.sort(); v }
            let before = snap(&proj);
            let who = Caller { gen: "g".into(), client: 1 };
            let call = |m: &'static str, p: Value| { let (hub, who) = (hub.clone(), Caller { gen: who.gen.clone(), client: 1 }); async move { hub.rpc_for(m).unwrap().call(&hub, &who, &json!(5), m, &p).await } };
            let r = call("mech.render", json!({"thread":"ok","path":"m.step"})).await; assert_eq!(r[0]["ok"], false, "{r:?}"); assert_eq!(r[0]["result"]["text"], crate::copy::HOME_ONLY);
            let r = call("review.tick", json!({"thread":"ok","index":0})).await; assert_eq!(r[0]["ok"], false, "{r:?}");
            let r = crate::settings::fab::reply(&hub, &json!(5), "fab.intent", &json!({"thread":"ok","text":"# Goal\nx"})).await; assert_eq!(r["ok"], false, "{r:?}"); assert_eq!(r["error"], crate::copy::HOME_ONLY);
            let (v, _, _) = crate::commands::home::read_files_pub(&proj.to_string_lossy(), &json!({"op":"intent","text":"x","revision":"","editor":0,"removeBytes":0,"project":"p1"}));
            assert_eq!(v["error"], crate::copy::HOME_ONLY, "{v:?}");
            assert_eq!(snap(&proj), before, "something was written into the project");
            assert!(!proj.join("renders").exists());
        });
    }
    #[test] fn a_copy_writes_no_render_checklist_or_intent_into_a_project() {
        let o = std::process::Command::new(std::env::current_exe().unwrap()).args(["files::boundary_tests::copy_writes_probe", "--exact", "--nocapture", "--test-threads=1"])
            .env("BACKPLANE_COPY", "1").env("BP_COPY_PROBE", "1").env_remove("BACKPLANE_AGENTS").output().unwrap();
        assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    }
}

/// files.op {thread, op: mkdir|rename|delete|write, path, to?, data?} and files.read {thread, path, max?}: the Files tab's writes (ops.rs).
/// Ok: `{ok:true, result:{path, thread, op}}` (files.read: `{text, path}`); a refusal says why in `result.text`. A hub on a copied log writes nothing.
async fn files_op(hub: &Arc<Hub>, id: &Value, method: &str, p: &Value) -> Value {
    let thread = p.get("thread").and_then(Value::as_str).unwrap_or("");
    let Some(root) = root_of(hub, thread).await else { return threadpath::refusal(id) };
    if method == "files.op" && crate::copy::on() { let e = crate::copy::HOME_ONLY; return json!({"t":"reply","id":id,"ok":false,"error":e,"result":{"text":e,"thread":thread}}) }
    let (p2, m) = (p.clone(), method.to_string());
    let r = tokio::task::spawn_blocking(move || if m == "files.read" {
        let rel = p2.get("path").and_then(Value::as_str).unwrap_or("");
        ops::read_text(&root, rel, p2.get("max").and_then(Value::as_u64).unwrap_or(65536).min(1 << 20) as usize).map(|t| json!({"text":t,"path":rel}))
    } else { ops::run(&root, &p2).map(|path| json!({"path":path,"op":p2.get("op").cloned().unwrap_or(Value::Null)})) }).await.unwrap_or(Err("failed".into()));
    match r {
        Ok(mut v) => { v["thread"] = json!(thread); json!({"t":"reply","id":id,"ok":true,"result":v}) }
        Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e,"result":{"text":e,"thread":thread}}),
    }
}

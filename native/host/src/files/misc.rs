//! The smaller Files-tab and Mechanical-tab requests: `file.open` (a file link opened or shown on the hub's own machine),
//! `pdf.page` (a PDF page drawn by poppler for the window), `mech.list` and `mech.render` (server's FileJob, PdfJob,
//! Mech.list, Mech.render). The shell text is hub's (scripts/*.sh, extracted by test/files/extract_scripts.py);
//! `mech.render` draws in this process (kicad/render.rs), one render at a time; `BACKPLANE_PLOT_NATIVE=0` runs renderer
//! child again (`backplane --render`, BACKPLANE_PLOT_BIN is the same binary).
use super::{fm, git::{proc, proc_ro}};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

const FILE_OPEN: &str = include_str!("scripts/file_open.sh");
const FILE_REVEAL: &str = include_str!("scripts/file_reveal.sh");
const PDF: &str = include_str!("scripts/pdf.sh");
const MECH_LIST: &str = include_str!("scripts/mech_list.sh");
const RESOLVE: &str = include_str!("scripts/resolve.sh");
/// a render takes seconds of every core and memory for the whole model: one at a time, the next waits
static RENDER: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

/// Mech.error: "" when the render worked, else the child's own `backplane --render:` line, else its first 300 chars.
pub fn mech_error(code: i32, out: &str) -> String {
    if code == 0 { return String::new() }
    let text = out.trim();
    let fallback = if text.is_empty() { format!("backplane --render failed (exit {code})") } else { text.chars().take(300).collect() };
    text.split('\n').map(str::trim).find(|l| l.starts_with("backplane --render:")).map(|l| l.chars().take(300).collect()).unwrap_or(fallback)
}
/// FileList.cmd outside a repository: files up to eight folders down, not under a hidden folder or node_modules, 3000 at most (sorted: find's own order is unspecified).
fn walk_files(root: &std::path::Path) -> Vec<String> {
    fn go(dir: &std::path::Path, rel: &str, depth: usize, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut es: Vec<_> = rd.flatten().collect(); es.sort_by_key(|e| e.file_name());
        for e in es {
            let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
            if name.starts_with('.') || out.len() >= 3000 { continue }
            let Ok(ft) = e.file_type() else { continue };
            let p = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if ft.is_dir() { if depth < 8 && name != "node_modules" { go(&e.path(), &p, depth + 1, out) } } else if ft.is_file() { out.push(p) }
        }
    }
    let mut v = vec![]; go(root, "", 1, &mut v); v
}
/// PdfJob.why
fn pdf_why(out: &str) -> String { if out.contains("nopoppler") { "PDFs show here with poppler's pdftoppm (the poppler or poppler-utils package)".into() } else { out.trim().into() } }

pub struct Misc;
impl RpcHandler for Misc {
    fn methods(&self) -> &'static [&'static str] { &["file.open", "pdf.page", "mech.list", "mech.render", "history.resolve", "boards.list", "files.list", "kicad.components"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let reply = |ok: bool, r: Value| vec![json!({"t":"reply","id":id,"ok":ok,"result":r})];
            match method {
                "file.open" => {
                    // a hub on a copied log acts outside its home nowhere
                    if std::env::var("BACKPLANE_COPY").as_deref() == Ok("1") { return reply(false, json!({"text":"refused: this hub runs on a copy of a log"})) }
                    let path = s("path"); if path.contains('\0') { return reply(false, json!({"text":"bad path"})) }
                    let script = if s("how") == "reveal" { FILE_REVEAL } else { FILE_OPEN };
                    let r = proc("sh", &["-c", script, "sh", &path], "/", &[]).await;
                    if r.code == 0 { reply(true, json!({"text":""})) } else { reply(false, json!({"text":r.out.trim()})) }
                }
                "pdf.page" => {
                    let path = s("path"); let page = match p.get("page") { Some(Value::Number(n)) => n.to_string(), Some(Value::String(x)) => x.clone(), _ => "1".into() };
                    let ok_page = !page.is_empty() && page.len() <= 6 && page.bytes().all(|b| b.is_ascii_digit());
                    if !(crate::plat::fs::is_abs(&path) && path.to_lowercase().ends_with(".pdf")) || path.contains('\0') || !ok_page {
                        return reply(true, json!({"pdfPath":path,"pdfPage":page,"pdfErr":"not a PDF path"}))
                    }
                    let dir = hub.cfg.home.join("pdf");
                    let r = proc_ro("sh", &["-c", PDF, "sh", &path, &page, &dir.to_string_lossy()], "/", &[]).await;
                    if r.code == 0 {
                        let mut it = r.out.split('\t'); let png = it.next().unwrap_or("").to_string(); let pages = it.next().unwrap_or("0").trim().to_string();
                        reply(true, json!({"pdfPath":path,"pdfPage":page,"pdfPng":png,"pdfPages":pages}))
                    } else { reply(true, json!({"pdfPath":path,"pdfPage":page,"pdfErr":pdf_why(&r.out)})) }
                }
                "history.resolve" => { // RvJob.resolve: a commit, branch or tag typed in, as the commit it names
                    let (thread, side, rf) = (s("thread"), s("side"), s("ref"));
                    let Some(cwd_p) = super::root_of(hub, &thread).await else { return vec![super::threadpath::refusal(id)] };
                    let cwd = cwd_p.to_string_lossy().into_owned();
                    let err = |why: String| reply(false, json!({"error":why}));
                    if rf.is_empty() || rf.starts_with('-') || rf.contains('\0') { return err(format!("no commit, branch or tag named {rf}")) }
                    let r = proc_ro("sh", &["-c", RESOLVE, "sh", &cwd, &rf], "/", &[]).await;
                    if r.code == 0 { let ls: Vec<&str> = r.out.split('\n').collect(); let at = |i: usize| ls.get(i).map(|x| x.trim()).unwrap_or("");
                        reply(true, json!({"resolved":{"thread":thread,"side":side,"ref":rf,"commit":at(0),"git":at(1),"top":at(2)}})) }
                    else { err(if r.out.trim().is_empty() { format!("could not resolve {rf}") } else { r.out.trim().to_string() }) }
                }
                // the file picker's list (FileList.cmd): git's view of the tree in a repository (cached + untracked, not ignored), else a bounded walk; 3000 at most
                "files.list" => {
                    let thread = s("thread");
                    let Some(cwd_p) = super::root_of(hub, &thread).await else { return vec![super::threadpath::refusal(id)] };
                    let cwd = cwd_p.to_string_lossy().into_owned();
                    let files: Vec<String> = if super::git::is_repo(&cwd).await {
                        let r = super::git::run_ro(&cwd, &["ls-files", "-z", "--cached", "--others", "--exclude-standard"]).await;
                        if r.code != 0 { return reply(false, json!({"text":r.out})) }
                        r.out.split('\0').filter(|x| !x.is_empty()).take(3000).map(str::to_string).collect()
                    } else { tokio::task::spawn_blocking(move || walk_files(std::path::Path::new(&cwd))).await.unwrap_or_default() };
                    reply(true, json!({"files":files,"thread":thread}))
                }
                // jobs the Rust hub does not run yet: the folder is resolved first (a thread without one is refused as everywhere), then they say so
                "kicad.components" => {
                    if super::root_of(hub, &s("thread")).await.is_none() { return vec![super::threadpath::refusal(id)] }
                    // kicad.components and kicad.renderAsset are called by the web viewer only (src/web/viewer-rpc.js); no GPUI, iOS or Android client sends them
                    vec![json!({"t":"reply","id":id,"ok":false,"error":format!("{method} is served to the web viewer only; this hub has none")})]
                }
                "boards.list" => { // BoardsJob: the project's boards read again, as the card finds them
                    let thread = s("thread");
                    let Some(cwd_p) = super::root_of(hub, &thread).await else { return vec![super::threadpath::refusal(id)] };
                    let cwd = cwd_p.to_string_lossy().into_owned();
                    let found = crate::files::kicad::boards::find(std::path::Path::new(&cwd));
                    let f = crate::files::kicad::project::read(&cwd); // Od.pcb: the folder's own board, from its root
                    let bd = f.pcb.strip_prefix(&format!("{cwd}/")).unwrap_or("").to_string();
                    reply(true, json!({"thread":thread,"board_list":crate::files::kicad::boards::pick(&found, &bd),"board_default":bd}))
                }
                _ => { // mech.list, mech.render
                    let thread = s("thread");
                    let Some(cwd_p) = super::root_of(hub, &thread).await else { return vec![super::threadpath::refusal(id)] };
                    let cwd = cwd_p.to_string_lossy().into_owned();
                    let (mut rendered, mut err) = (String::new(), String::new());
                    if method == "mech.render" {
                        // mech.render writes renders/<part>.<view>.png into the project's folder: outside a copy's home, so a copy is told, not run
                        if crate::copy::on() { return reply(false, json!({"text":crate::copy::HOME_ONLY})) }
                        let path = s("path");
                        // the model to draw lies in the project: a path from its root, or an absolute path under it
                        // Hub.folder.path.ok, then Mech.render.at: a path from the root or absolute under it, and a model that really lies inside (no link out)
                        if !super::threadpath::path_in(&cwd, &path) { return vec![json!({"t":"reply","id":id,"ok":false,"error":"that path is not in this project"})] }
                        let rel = path.strip_prefix(&format!("{cwd}/")).unwrap_or(&path);
                        if fm::contained(std::path::Path::new(&cwd), rel).is_none() { return reply(false, json!({"text":"that model is unavailable or outside this project"})) }
                        let (code, out) = if crate::files::kicad::watch::native() {
                            let _one = RENDER.acquire().await.ok();
                            let (m, c, home) = (path.clone(), cwd.clone(), hub.cfg.home.clone());
                            let r = tokio::task::spawn_blocking(move || { use crate::files::kicad::{render, stepglb};
                                let r = std::panic::catch_unwind(|| render::run(&[m.as_str()], &c, &stepglb::machine(&home)));
                                #[cfg(all(target_os = "linux", target_env = "gnu"))] unsafe { libc::malloc_trim(0); }
                                r }).await;
                            match r { Ok(Ok(r)) => (r.code, r.out), _ => (1, "backplane --render: the renderer stopped".to_string()) }
                        } else {
                            let bin = std::env::var_os("BACKPLANE_PLOT_BIN").map(std::path::PathBuf::from).unwrap_or_else(|| "backplane-serve".into());
                            let r = proc(&bin.to_string_lossy(), &["--render", &path], &cwd, &[]).await; (r.code, r.out)
                        };
                        err = mech_error(code, &out); rendered = path;
                    }
                    let l = proc_ro("sh", &["-c", MECH_LIST], &cwd, &[]).await;
                    let cfg = std::fs::read_to_string(format!("{cwd}/.backplane.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.get("mech").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
                    if method == "mech.render" { reply(true, json!({"mech":l.out,"root":cwd,"thread":thread,"rendered":rendered,"error":err,"primary":cfg})) }
                    else if l.code == 0 { reply(true, json!({"mech":l.out,"root":cwd,"thread":thread,"primary":cfg})) } else { reply(false, json!({"text":l.out})) }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // laws mech_error_*: Mech.error
    #[test] fn mech_render_success_quiet() { for out in ["", "anything", "backplane --render: nothing", "/p/renders/a.iso.png\n"] { assert_eq!(mech_error(0, out), "") } }
    #[test] fn mech_render_failure_visible() { assert!(!mech_error(1, "").is_empty()) }
    #[test] fn mech_render_failure_diagnostic() { assert_eq!(mech_error(1, "/p/renders/fixture.iso.png\nbackplane --render: could not write /p/renders/fixture.front.png\n"), "backplane --render: could not write /p/renders/fixture.front.png") }
    #[test] fn mech_error_words() {
        assert_eq!(mech_error(0, "anything"), ""); assert_eq!(mech_error(2, ""), "backplane --render failed (exit 2)");
        assert_eq!(mech_error(1, "noise\n  backplane --render: no GLB \nmore"), "backplane --render: no GLB");
        assert_eq!(mech_error(1, &"x".repeat(500)).len(), 300);
    }
    #[tokio::test] async fn mech_list_script_lists_models_and_renders_sorted() {
        let d = std::env::temp_dir().join(format!("bp-mech-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d);
        for f in ["mech/a.step", "mech/b.png", "mech/sub/c.glb", "mech/.hid/x.step", "mech/readme.txt"] { std::fs::create_dir_all(d.join(f).parent().unwrap()).unwrap(); std::fs::write(d.join(f), "x").unwrap() }
        let r = proc("sh", &["-c", MECH_LIST], &d.to_string_lossy(), &[]).await; assert_eq!(r.code, 0);
        let names: Vec<&str> = r.out.lines().map(|l| l.rsplit(' ').next().unwrap()).collect(); assert_eq!(names, ["mech/a.step", "mech/b.png", "mech/sub/c.glb"], "{}", r.out);
        let e = std::env::temp_dir().join(format!("bp-mech-empty-{}", std::process::id())); std::fs::create_dir_all(&e).unwrap();
        assert_eq!(proc("sh", &["-c", MECH_LIST], &e.to_string_lossy(), &[]).await.out, "");
    }
    #[test] fn file_picker_walk_skips_hidden_and_node_modules() {
        let d = std::env::temp_dir().join(format!("bp-walk-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d);
        for f in ["a.txt", "sub/b.txt", ".hid/c.txt", "node_modules/n.js", "sub/.dot", "x/y/z/w.txt"] { std::fs::create_dir_all(d.join(f).parent().unwrap()).unwrap(); std::fs::write(d.join(f), "").unwrap() }
        assert_eq!(walk_files(&d), ["a.txt", "sub/b.txt", "x/y/z/w.txt"]);
    }
    #[tokio::test] async fn resolve_script_names_a_ref_as_a_commit() {
        let d = std::env::temp_dir().join(format!("bp-res-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); let cwd = d.to_string_lossy().into_owned();
        let g = |a: &[&str]| { let o = std::process::Command::new("git").args(a).current_dir(&d).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap(); String::from_utf8_lossy(&o.stdout).trim().to_string() };
        g(&["init", "-q"]); std::fs::write(d.join("f"), "x").unwrap(); g(&["add", "."]); g(&["commit", "-qm", "i"]); g(&["tag", "v1"]); let head = g(&["rev-parse", "HEAD"]);
        let r = proc("sh", &["-c", RESOLVE, "sh", &cwd, "v1"], "/", &[]).await; let ls: Vec<&str> = r.out.split('\n').collect(); assert_eq!((r.code, ls[0]), (0, head.as_str())); assert!(ls[1].ends_with(".git") && ls[2].len() > 1);
        let r = proc("sh", &["-c", RESOLVE, "sh", &cwd, "nope"], "/", &[]).await; assert_eq!(r.code, 1); assert!(r.out.contains("no commit, branch or tag named nope"));
    }
    #[tokio::test] async fn file_open_refuses_a_missing_file_and_the_pdf_script_a_missing_pdf() {
        let r = proc("sh", &["-c", FILE_OPEN, "sh", "/nonexistent/file"], "/", &[]).await; assert_eq!(r.code, 1); assert!(r.out.contains("No such file"));
        let d = std::env::temp_dir().join(format!("bp-pdf-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap();
        let r = proc("sh", &["-c", PDF, "sh", "/nonexistent.pdf", "1", &d.to_string_lossy()], "/", &[]).await; assert_ne!(r.code, 0);
    }
}

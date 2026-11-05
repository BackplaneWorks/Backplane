//! The smaller Files-tab and Mechanical-tab requests: `file.open` (a file link opened or shown on the hub's own machine),
//! `pdf.page` (a PDF page drawn by poppler for the window), `mech.list` and `mech.render` (server.bend's FileJob, PdfJob,
//! Mech.list, Mech.render). The shell text is the Bend hub's (scripts/*.sh, extracted by test/gpui/files/extract_scripts.py);
//! `mech.render` stays the existing Bend renderer child (`backplane --render`, BACKPLANE_PLOT_BIN is the same binary).
use super::{fm, git::proc};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

const FILE_OPEN: &str = include_str!("scripts/file_open.sh");
const FILE_REVEAL: &str = include_str!("scripts/file_reveal.sh");
const PDF: &str = include_str!("scripts/pdf.sh");
const MECH_LIST: &str = include_str!("scripts/mech_list.sh");
const RESOLVE: &str = include_str!("scripts/resolve.sh");

/// Mech.error: "" when the render worked, else the child's own `backplane --render:` line, else its first 300 chars.
pub fn mech_error(code: i32, out: &str) -> String {
    if code == 0 { return String::new() }
    let text = out.trim();
    let fallback = if text.is_empty() { format!("backplane --render failed (exit {code})") } else { text.chars().take(300).collect() };
    text.split('\n').map(str::trim).find(|l| l.starts_with("backplane --render:")).map(|l| l.chars().take(300).collect()).unwrap_or(fallback)
}
/// PdfJob.why
fn pdf_why(out: &str) -> String { if out.contains("nopoppler") { "PDFs show here with poppler's pdftoppm (the poppler or poppler-utils package)".into() } else { out.trim().into() } }

pub struct Misc;
impl RpcHandler for Misc {
    fn methods(&self) -> &'static [&'static str] { &["file.open", "pdf.page", "mech.list", "mech.render", "history.resolve"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let reply = |ok: bool, r: Value| vec![json!({"t":"reply","id":id,"ok":ok,"result":r})];
            match method {
                "file.open" => {
                    // a hub on a copied log acts outside its home nowhere (core/copy.bend)
                    if std::env::var("BACKPLANE_COPY").as_deref() == Ok("1") { return reply(false, json!({"text":"refused: this hub runs on a copy of a log"})) }
                    let path = s("path"); if path.contains('\0') { return reply(false, json!({"text":"bad path"})) }
                    let script = if s("how") == "reveal" { FILE_REVEAL } else { FILE_OPEN };
                    let r = proc("sh", &["-c", script, "sh", &path], "/", &[]).await;
                    if r.code == 0 { reply(true, json!({"text":""})) } else { reply(false, json!({"text":r.out.trim()})) }
                }
                "pdf.page" => {
                    let path = s("path"); let page = match p.get("page") { Some(Value::Number(n)) => n.to_string(), Some(Value::String(x)) => x.clone(), _ => "1".into() };
                    let ok_page = !page.is_empty() && page.len() <= 6 && page.bytes().all(|b| b.is_ascii_digit());
                    if !(path.starts_with('/') && path.to_lowercase().ends_with(".pdf")) || path.contains('\0') || !ok_page {
                        return reply(true, json!({"pdfPath":path,"pdfPage":page,"pdfErr":"not a PDF path"}))
                    }
                    let dir = hub.cfg.home.join("pdf");
                    let r = proc("sh", &["-c", PDF, "sh", &path, &page, &dir.to_string_lossy()], "/", &[]).await;
                    if r.code == 0 {
                        let mut it = r.out.split('\t'); let png = it.next().unwrap_or("").to_string(); let pages = it.next().unwrap_or("0").trim().to_string();
                        reply(true, json!({"pdfPath":path,"pdfPage":page,"pdfPng":png,"pdfPages":pages}))
                    } else { reply(true, json!({"pdfPath":path,"pdfPage":page,"pdfErr":pdf_why(&r.out)})) }
                }
                "history.resolve" => { // RvJob.resolve: a commit, branch or tag typed in, as the commit it names
                    let (thread, side, rf) = (s("thread"), s("side"), s("ref"));
                    let cwd_p = super::root_of(hub, &thread).await; let cwd = cwd_p.to_string_lossy().into_owned();
                    let err = |why: String| reply(false, json!({"error":why}));
                    if rf.is_empty() || rf.starts_with('-') || cwd.is_empty() || rf.contains('\0') { return err(format!("no commit, branch or tag named {rf}")) }
                    let r = proc("sh", &["-c", RESOLVE, "sh", &cwd, &rf], "/", &[]).await;
                    if r.code == 0 { let ls: Vec<&str> = r.out.split('\n').collect(); let at = |i: usize| ls.get(i).map(|x| x.trim()).unwrap_or("");
                        reply(true, json!({"resolved":{"thread":thread,"side":side,"ref":rf,"commit":at(0),"git":at(1),"top":at(2)}})) }
                    else { err(if r.out.trim().is_empty() { format!("could not resolve {rf}") } else { r.out.trim().to_string() }) }
                }
                _ => { // mech.list, mech.render
                    let thread = s("thread"); let cwd_p = super::root_of(hub, &thread).await; let cwd = cwd_p.to_string_lossy().into_owned();
                    if cwd.is_empty() { return reply(false, json!({"text":"no folder"})) }
                    let (mut rendered, mut err) = (String::new(), String::new());
                    if method == "mech.render" {
                        let path = s("path");
                        // the model to draw lies in the project: a path from its root, or an absolute path under it
                        let inside = if let Some(rel) = path.strip_prefix(&format!("{cwd}/")) { fm::contained(std::path::Path::new(&cwd), rel).is_some() } else { fm::safe(&path) && !path.is_empty() && !path.contains('\\') };
                        if !inside { return reply(false, json!({"text":"that file is not in this project"})) }
                        let bin = std::env::var_os("BACKPLANE_PLOT_BIN").map(std::path::PathBuf::from).unwrap_or_else(|| "backplane-serve".into());
                        let r = proc(&bin.to_string_lossy(), &["--render", &path], &cwd, &[]).await;
                        err = mech_error(r.code, &r.out); rendered = path;
                    }
                    let l = proc("sh", &["-c", MECH_LIST], &cwd, &[]).await;
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

//! The smaller Files-tab and Mechanical-tab requests: `file.open` (a file link opened or shown on the hub's own machine),
//! `pdf.page` (a PDF page drawn by poppler for the window), `mech.list` and `mech.render` (server's FileJob, PdfJob,
//! Mech.list, Mech.render). The shell text is hub's (scripts/*.sh, extracted by test/files/extract_scripts.py);
//! `mech.render` draws in this process (kicad/render.rs), one render at a time; `BACKPLANE_PLOT_NATIVE=0` runs renderer
//! child again (`backplane --render`, BACKPLANE_PLOT_BIN is the same binary).
use super::{fm, git::{proc, proc_ro}};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

#[cfg(not(windows))]
const FILE_OPEN: &str = include_str!("scripts/file_open.sh");
#[cfg(not(windows))]
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

/// A path as a message link gives it, as the opener wants it: from the thread's folder when relative, without a `:line[:col]` tail when the file
/// is not named that way. `~/` and absolute paths stay as they are.
pub fn link_path(path: &str, root: &str) -> String {
    let is_abs_or_home = |p: &str| p == "~" || p.starts_with("~/") || p.starts_with("~\\") || p.starts_with('/') || crate::plat::dirs::win_abs(p);
    let abs = |p: &str| if is_abs_or_home(p) { p.to_string() } else { format!("{}/{}", root.trim_end_matches(['/', '\\']), p.trim_start_matches("./").trim_start_matches(".\\")) };
    let full = abs(path);
    let exists = |p: &str| {
        let q = if p == "~" {
            crate::plat::dirs::home().unwrap_or_default()
        } else if let Some(r) = p.strip_prefix("~/").or_else(|| p.strip_prefix("~\\")) {
            crate::plat::dirs::home().map(|h| format!("{}/{r}", h.trim_end_matches(['/', '\\']))).unwrap_or_default()
        } else {
            p.to_string()
        };
        !q.is_empty() && std::fs::symlink_metadata(q).is_ok()
    };
    let mut cut = full.as_str();
    while !exists(cut) {
        let Some((head, tail)) = cut.rsplit_once(':') else { return full };
        if tail.is_empty() || !tail.bytes().all(|b| b.is_ascii_digit()) { return full }
        cut = head;
    }
    cut.to_string()
}

/// Normalize a Windows folder or file path for `explorer.exe`: expands `~`, converts `/` to `\`,
/// strips `\\?\` / `\\?\UNC\` verbatim prefixes, collapses duplicate separators (preserving leading `\\`),
/// and trims trailing backslashes except on a drive root like `C:\`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn normalize_win_path(path: &str, home: Option<&str>) -> String {
    let expanded = if let Some(h) = home.filter(|h| !h.is_empty()) {
        let base = h.trim_end_matches(['/', '\\']);
        if path == "~" {
            if base.len() == 2 && base.as_bytes()[1] == b':' { format!("{base}\\") } else { base.to_string() }
        } else if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
            format!("{base}\\{rest}")
        } else {
            path.to_string()
        }
    } else {
        path.to_string()
    };
    let slashed = expanded.replace('/', "\\");
    let stripped = if let Some(rest) = slashed.strip_prefix(r"\\?\") {
        if rest.get(..4).is_some_and(|u| u.eq_ignore_ascii_case(r"UNC\")) {
            format!(r"\\{}", &rest[4..])
        } else if rest.as_bytes().get(1) == Some(&b':') && rest.as_bytes()[0].is_ascii_alphabetic() {
            rest.to_string()
        } else {
            slashed
        }
    } else {
        slashed
    };
    let mut collapsed = String::with_capacity(stripped.len());
    let mut rest = stripped.as_str();
    if rest.starts_with(r"\\") {
        collapsed.push_str(r"\\");
        rest = rest.trim_start_matches('\\');
    }
    let mut prev_sep = false;
    for ch in rest.chars() {
        if ch == '\\' {
            if !prev_sep { collapsed.push('\\'); }
            prev_sep = true;
        } else {
            prev_sep = false;
            collapsed.push(ch);
        }
    }
    if collapsed == r"\\" { return r"\\".to_string() }
    if collapsed == r"\" { return r"\".to_string() }
    let trimmed = collapsed.trim_end_matches('\\');
    let tb = trimmed.as_bytes();
    if tb.len() == 2 && tb[0].is_ascii_alphabetic() && tb[1] == b':' && collapsed.as_bytes().get(2) == Some(&b'\\') {
        format!("{trimmed}\\")
    } else {
        trimmed.to_string()
    }
}

/// Resolve the full path to `explorer.exe` from `SystemRoot`/`WINDIR`, falling back to `"explorer.exe"`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn win_explorer_exe(system_root: Option<&str>) -> String {
    match system_root.map(str::trim).filter(|r| !r.is_empty()) {
        Some(r) => {
            let norm = r.replace('/', "\\");
            let base = norm.trim_end_matches('\\');
            if base.is_empty() { "explorer.exe".to_string() } else { format!("{base}\\explorer.exe") }
        }
        None => "explorer.exe".to_string(),
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn is_win_drive_root(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\'
}

#[cfg_attr(not(windows), allow(dead_code))]
fn quote_win_arg(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    let mut bs = 0usize;
    for ch in s.chars() {
        if ch == '\\' {
            bs += 1;
        } else if ch == '"' {
            for _ in 0..(bs * 2 + 1) { out.push('\\'); }
            bs = 0;
            out.push('"');
        } else {
            for _ in 0..bs { out.push('\\'); }
            bs = 0;
            out.push(ch);
        }
    }
    for _ in 0..(bs * 2) { out.push('\\'); }
    out.push('"');
    out
}

/// Format the raw command-line argument passed to `explorer.exe` for opening or revealing `path`.
/// Uses `/select,"<path>"` (or unquoted `<drive>:\`) so Explorer handles spaces and commas without
/// Rust's `Command::arg` wrapping `/select,` inside the quotes.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn win_explorer_raw_arg(path: &str, home: Option<&str>, reveal: bool) -> String {
    let norm = normalize_win_path(path, home);
    if is_win_drive_root(&norm) {
        if reveal { format!("/select,{norm}") } else { norm }
    } else {
        let q = quote_win_arg(&norm);
        if reveal { format!("/select,{q}") } else { q }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
fn is_win_executable(path: &str) -> bool {
    let base = path.rsplit(['\\', '/']).next().unwrap_or(path).trim_end_matches([' ', '.']);
    let Some((_, ext)) = base.rsplit_once('.') else { return false };
    const EXTS: &[&str] = &[
        "exe", "bat", "cmd", "com", "msi", "msp", "msu", "ps1", "psm1", "psd1",
        "vbs", "vbe", "js", "jse", "wsf", "wsh", "scr", "pif", "hta", "lnk",
        "url", "reg", "msc", "cpl", "inf", "jar",
    ];
    EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e))
}

/// Validate and format the `explorer.exe` raw argument for opening or revealing `path`.
/// Non-directory files with executable/script/shortcut extensions are forced to `/select,"..."`
/// so `how == "open"` on a message link never executes an arbitrary file.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn win_explorer_open_arg(path: &str, home: Option<&str>, reveal: bool, is_dir: bool) -> Result<String, String> {
    let norm = normalize_win_path(path, home);
    if norm.is_empty() {
        return Err("No such file: ".into());
    }
    if norm.contains('\0') || norm.contains('"') {
        return Err(format!("bad path: {norm}"));
    }
    let effective_reveal = reveal || (!is_dir && is_win_executable(&norm));
    Ok(win_explorer_raw_arg(&norm, None, effective_reveal))
}

#[cfg(windows)]
fn open_on_windows(path: &str, reveal: bool) -> Result<(), String> {
    let norm = normalize_win_path(path, crate::plat::dirs::home().as_deref());
    if norm.is_empty() {
        return Err("No such file: ".into());
    }
    if norm.contains('\0') || norm.contains('"') {
        return Err(format!("bad path: {norm}"));
    }
    let meta = std::fs::metadata(&norm)
        .or_else(|_| std::fs::symlink_metadata(&norm))
        .map_err(|_| format!("No such file: {norm}"))?;
    let is_dir = meta.is_dir();
    if let Ok(custom) = std::env::var("BACKPLANE_OPEN") {
        if !custom.is_empty() {
            // Mirror file_reveal.sh's fallback when revealing a file by passing its parent directory.
            let target = if reveal && !is_dir {
                std::path::Path::new(&norm)
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| norm.clone())
            } else {
                norm.clone()
            };
            let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, &custom);
            c.arg(&target)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            return c.spawn().map(|_| ()).map_err(|e| format!("could not open {norm}: {e}"));
        }
    }
    let sys_root = std::env::var("SystemRoot").ok().or_else(|| std::env::var("WINDIR").ok());
    let candidate = win_explorer_exe(sys_root.as_deref());
    let exe = if std::path::Path::new(&candidate).is_file() { candidate } else { "explorer.exe".to_string() };
    let raw = win_explorer_open_arg(&norm, None, reveal, is_dir)?;
    let mut c = crate::proc::std_cmd(crate::proc::Kind::Local, &exe);
    use std::os::windows::process::CommandExt;
    // explorer.exe is a GUI process; clear std_cmd's default CREATE_NO_WINDOW so its window opens normally.
    c.creation_flags(0);
    c.raw_arg(&raw);
    c.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    c.spawn().map(|_| ()).map_err(|e| format!("could not open {norm}: {e}"))
}

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
                    let mut path = s("path"); if path.contains('\0') { return reply(false, json!({"text":"bad path"})) }
                    // a link in a message names a path from the thread's folder, maybe with a `:line` after it
                    let thread = s("thread");
                    if !thread.is_empty() { if let Some(root) = super::root_of(hub, &thread).await { path = link_path(&path, &root.to_string_lossy()) } }
                    let reveal = s("how") == "reveal";
                    #[cfg(windows)]
                    {
                        let res = tokio::task::spawn_blocking(move || open_on_windows(&path, reveal)).await
                            .unwrap_or_else(|e| Err(format!("could not open folder: {e}")));
                        match res {
                            Ok(()) => reply(true, json!({"text":""})),
                            Err(e) => reply(false, json!({"text":e})),
                        }
                    }
                    #[cfg(not(windows))]
                    {
                        let script = if reveal { FILE_REVEAL } else { FILE_OPEN };
                        let r = proc("sh", &["-c", script, "sh", &path], "/", &[]).await;
                        if r.code == 0 { reply(true, json!({"text":""})) } else { reply(false, json!({"text":r.out.trim()})) }
                    }
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
                    // a hardware project's boards are in its main checkout: the thread's code worktree holds none of them (design files stay
                    // on main, kicad/canonical.rs), so the list read there was empty and the picker showed only the default board
                    let cwd = crate::files::kicad::canonical::design_root(&cwd_p.to_string_lossy());
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
        let d = crate::plat::tmp::dir("mech");
        for f in ["mech/a.step", "mech/b.png", "mech/d.sc", "mech/sub/c.glb", "mech/.hid/x.step", "mech/readme.txt"] { std::fs::create_dir_all(d.join(f).parent().unwrap()).unwrap(); std::fs::write(d.join(f), "x").unwrap() }
        let r = proc("sh", &["-c", MECH_LIST], &d.to_string_lossy(), &[]).await; assert_eq!(r.code, 0);
        let names: Vec<&str> = r.out.lines().map(|l| l.rsplit(' ').next().unwrap()).collect(); assert_eq!(names, ["mech/a.step", "mech/b.png", "mech/d.sc", "mech/sub/c.glb"], "{}", r.out);
        let e = crate::plat::tmp::dir("mech-empty");
        assert_eq!(proc("sh", &["-c", MECH_LIST], &e.to_string_lossy(), &[]).await.out, "");
    }
    #[test] fn file_picker_walk_skips_hidden_and_node_modules() {
        let d = crate::plat::tmp::dir("walk");
        for f in ["a.txt", "sub/b.txt", ".hid/c.txt", "node_modules/n.js", "sub/.dot", "x/y/z/w.txt"] { std::fs::create_dir_all(d.join(f).parent().unwrap()).unwrap(); std::fs::write(d.join(f), "").unwrap() }
        assert_eq!(walk_files(&d), ["a.txt", "sub/b.txt", "x/y/z/w.txt"]);
    }
    #[tokio::test] async fn resolve_script_names_a_ref_as_a_commit() {
        let d = crate::plat::tmp::dir("res"); let cwd = d.to_string_lossy().into_owned();
        let g = |a: &[&str]| { let o = std::process::Command::new("git").args(a).current_dir(&d).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap(); String::from_utf8_lossy(&o.stdout).trim().to_string() };
        g(&["init", "-q"]); std::fs::write(d.join("f"), "x").unwrap(); g(&["add", "."]); g(&["commit", "-qm", "i"]); g(&["tag", "v1"]); let head = g(&["rev-parse", "HEAD"]);
        let r = proc("sh", &["-c", RESOLVE, "sh", &cwd, "v1"], "/", &[]).await; let ls: Vec<&str> = r.out.split('\n').collect(); assert_eq!((r.code, ls[0]), (0, head.as_str())); assert!(ls[1].ends_with(".git") && ls[2].len() > 1);
        let r = proc("sh", &["-c", RESOLVE, "sh", &cwd, "nope"], "/", &[]).await; assert_eq!(r.code, 1); assert!(r.out.contains("no commit, branch or tag named nope"));
    }
    #[test] fn a_links_path_is_made_whole_and_loses_its_line() {
        let d = crate::plat::tmp::dir("lp"); std::fs::create_dir_all(d.join("mfg")).unwrap(); std::fs::write(d.join("mfg/a.zip"), "x").unwrap();
        let root = d.to_string_lossy().into_owned();
        assert_eq!(link_path("mfg/a.zip", &root), format!("{root}/mfg/a.zip")); assert_eq!(link_path("./mfg/a.zip:12:3", &root), format!("{root}/mfg/a.zip"));
        assert_eq!(link_path("/abs/none.rs:5", &root), "/abs/none.rs:5"); assert_eq!(link_path("~/x", &root), "~/x");
    }
    #[tokio::test] async fn file_open_refuses_a_missing_file_and_the_pdf_script_a_missing_pdf() {
        #[cfg(windows)]
        {
            let err = open_on_windows(r"C:\nonexistent\backplane_missing_file", false).unwrap_err();
            assert!(err.contains("No such file"), "{err}");
        }
        #[cfg(not(windows))]
        {
            let r = proc("sh", &["-c", FILE_OPEN, "sh", "/nonexistent/file"], "/", &[]).await; assert_eq!(r.code, 1); assert!(r.out.contains("No such file"));
        }
        let d = crate::plat::tmp::dir("pdf");
        let r = proc("sh", &["-c", PDF, "sh", "/nonexistent.pdf", "1", &d.to_string_lossy()], "/", &[]).await; assert_ne!(r.code, 0);
    }
    #[test] fn link_path_preserves_windows_absolute_paths_and_joins_relative_paths() {
        for p in [
            r"C:\Users\person\My Project",
            "C:/Users/person/proj",
            r"\\server\share\proj",
            r"\\?\C:\Users\person\proj",
            r"~\proj",
        ] {
            assert_eq!(link_path(p, r"C:\other\root"), p, "absolute/home path preserved: {p}");
        }
        assert_eq!(link_path("sub/file.txt", r"C:\other\root\"), "C:\\other\\root/sub/file.txt");
        assert_eq!(link_path(r".\sub\file.txt", r"C:\other\root\"), "C:\\other\\root/sub\\file.txt");
        assert_eq!(link_path(r"C:\x:5", r"C:\other\root"), r"C:\x:5");
        assert_eq!(link_path(r"C:\Users\person\a.rs:12:3", r"C:\other\root"), r"C:\Users\person\a.rs:12:3");
    }
    #[test] fn win_explorer_open_arg_guards_executables_and_bad_paths() {
        assert!(win_explorer_open_arg("", None, false, true).unwrap_err().contains("No such file"));
        assert!(win_explorer_open_arg(r#"C:\Users\person\bad"dir"#, None, false, true).unwrap_err().contains("bad path"));
        assert!(win_explorer_open_arg("C:\\Users\\person\\bad\0dir", None, false, true).unwrap_err().contains("bad path"));
        for p in [r"C:\proj\mfg\x.bat", r"C:\a\run.EXE", r"C:\a\script.ps1", r"C:\a\shortcut.lnk"] {
            let expected = format!(r#"/select,"{p}""#);
            assert_eq!(win_explorer_open_arg(p, None, false, false).unwrap(), expected, "{p}");
        }
        assert_eq!(win_explorer_open_arg(r"C:\a\dir.exe", None, false, true).unwrap(), r#""C:\a\dir.exe""#);
        assert_eq!(win_explorer_open_arg(r"C:\a\board.pdf", None, false, false).unwrap(), r#""C:\a\board.pdf""#);
        assert_eq!(win_explorer_open_arg(r"C:\a\img.png", None, false, false).unwrap(), r#""C:\a\img.png""#);
        assert_eq!(win_explorer_open_arg(r"C:\a\img.png", None, true, false).unwrap(), r#"/select,"C:\a\img.png""#);
    }
    #[test] fn normalize_win_path_handles_slashes_verbatim_prefixes_and_home() {
        let home = Some(r"C:\Users\person\");
        assert_eq!(normalize_win_path(r"C:\Users\person\My Project\", None), r"C:\Users\person\My Project");
        assert_eq!(normalize_win_path("C:/Users/person/proj/", None), r"C:\Users\person\proj");
        assert_eq!(normalize_win_path("C:/Users\\person//My Project\\", None), r"C:\Users\person\My Project");
        assert_eq!(normalize_win_path(r"C:\", None), r"C:\");
        assert_eq!(normalize_win_path("C:/", None), r"C:\");
        assert_eq!(normalize_win_path(r"\\?\C:\", None), r"C:\");
        assert_eq!(normalize_win_path(r"\\?\C:\Users\person\My Project\", None), r"C:\Users\person\My Project");
        assert_eq!(normalize_win_path(r"\\?\UNC\server\share\My Project\", None), r"\\server\share\My Project");
        assert_eq!(normalize_win_path(r"\\?\unc\server\share\My Project\", None), r"\\server\share\My Project");
        assert_eq!(normalize_win_path("//server/share/dir/", None), r"\\server\share\dir");
        assert_eq!(normalize_win_path(r"\\server\\share\\\dir\", None), r"\\server\share\dir");
        assert_eq!(normalize_win_path("~/My Project", home), r"C:\Users\person\My Project");
        assert_eq!(normalize_win_path(r"~\My Project\", home), r"C:\Users\person\My Project");
        assert_eq!(normalize_win_path("~", home), r"C:\Users\person");
    }
    #[test] fn win_explorer_raw_arg_formats_open_and_reveal_for_all_path_shapes() {
        assert_eq!(win_explorer_raw_arg(r"C:\Users\person\proj", None, false), r#""C:\Users\person\proj""#);
        assert_eq!(win_explorer_raw_arg(r"C:\Users\person\proj", None, true), r#"/select,"C:\Users\person\proj""#);
        assert_eq!(win_explorer_raw_arg(r"C:\Users\person\My Project", None, false), r#""C:\Users\person\My Project""#);
        assert_eq!(win_explorer_raw_arg(r"C:\Users\person\My Project", None, true), r#"/select,"C:\Users\person\My Project""#);
        assert_eq!(win_explorer_raw_arg(r"C:\Users\Doe, Jane\proj", None, false), r#""C:\Users\Doe, Jane\proj""#);
        assert_eq!(win_explorer_raw_arg(r"C:\Users\Doe, Jane\proj", None, true), r#"/select,"C:\Users\Doe, Jane\proj""#);
        assert_eq!(win_explorer_raw_arg("C:/Users/person/My Project/", None, false), r#""C:\Users\person\My Project""#);
        assert_eq!(win_explorer_raw_arg(r"\\?\C:\Users\person\proj", None, true), r#"/select,"C:\Users\person\proj""#);
        assert_eq!(win_explorer_raw_arg(r"\\?\UNC\server\share\My Project", None, false), r#""\\server\share\My Project""#);
        assert_eq!(win_explorer_raw_arg(r"\\server\share\dir", None, true), r#"/select,"\\server\share\dir""#);
        assert_eq!(win_explorer_raw_arg(r"C:\", None, false), r"C:\");
        assert_eq!(win_explorer_raw_arg("C:/", None, true), r"/select,C:\");
    }
    #[test] fn win_explorer_exe_resolves_system_root() {
        assert_eq!(win_explorer_exe(Some(r"C:\Windows")), r"C:\Windows\explorer.exe");
        assert_eq!(win_explorer_exe(Some("C:/Windows/")), r"C:\Windows\explorer.exe");
        assert_eq!(win_explorer_exe(None), "explorer.exe");
        assert_eq!(win_explorer_exe(Some("")), "explorer.exe");
    }
}

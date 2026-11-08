//! The viewer's Files tab: a complete file browser. The hub owns the listing and the navigation (`fm-cd`, `fm-up`, `fm-open`, `fm`: the same view
//! actions the web sends); this module draws it (icons, breadcrumbs, filter, keyboard selection, right-click menu, drop zone), previews what the
//! hub opened (image, PDF page, text) and does the writes through the hub's `files.op` RPC (new folder, rename, delete, upload) and the downloads
//! through `/files/get` and `/files/zip`. Pure rules (filter, selection keys, names, paths) are first and tested; the element code only lays out.
use super::{attach::Target, composer::{Composer, ComposerEvent}, images::Slot, settings::Secret, viewer::{self, FileRow}};
use crate::{hub::Command, theme, Backplane};
use gpui::{prelude::*, *};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf};

pub const MAX_ROWS: usize = 5000;
pub const MAX_TEXT: usize = 256 << 10;

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Mode { #[default] Browse, Rename(String), NewFolder, Delete(String) }

#[derive(Clone, Debug)]
pub enum Text { Loading, Ready(String), Failed(String) }

#[derive(Clone, Debug, Default)]
pub struct Pdf { pub path: String, pub page: u32, pub pages: u32, pub png: String, pub err: String, pub asked: bool }

#[derive(Default)]
pub struct FilesUi {
    pub filter: Option<Entity<Composer>>, pub name: Option<Entity<Composer>>, pub focus: Option<FocusHandle>,
    /// The selected row, as an index into the *visible* rows (after the filter).
    pub sel: Option<usize>,
    pub menu: Option<(usize, Point<Pixels>)>,
    pub mode: Mode,
    pub status: String,
    pub scroll: ScrollHandle,
    /// The folder last drawn: a new one clears the selection and the filter.
    pub dir: String,
    pub texts: HashMap<String, Text>,
    pub pdf: Pdf,
}

// ------------------------------------------------------------------ pure rules

/// The rows a filter keeps, as indexes into `rows`: a case-insensitive substring of the name; an empty filter keeps all.
pub fn visible(rows: &[FileRow], q: &str) -> Vec<usize> {
    let q = q.trim().to_lowercase();
    rows.iter().enumerate().filter(|(_, r)| q.is_empty() || r.label.to_lowercase().contains(&q)).map(|(i, _)| i).collect()
}
/// The selection after a navigation key over `len` rows: up/down move by one (down from nothing takes the first), home/end the ends,
/// pageup/pagedown ten. Always inside the list; None for an empty one.
pub fn step(sel: Option<usize>, len: usize, key: &str) -> Option<usize> {
    if len == 0 { return None }
    let last = len - 1;
    Some(match (key, sel) {
        ("down", None) | ("home", _) => 0,
        ("up", None) | ("end", _) => last,
        ("down", Some(i)) => (i + 1).min(last),
        ("up", Some(i)) => i.saturating_sub(1),
        ("pagedown", Some(i)) => (i + 10).min(last),
        ("pageup", Some(i)) => i.saturating_sub(10),
        ("pagedown", None) => 9.min(last),
        ("pageup", None) => 0,
        (_, Some(i)) => i.min(last),
        (_, None) => return None,
    })
}
/// A name a new file or folder may have here: the hub's `plain_name` rule (one visible component).
pub fn valid_name(n: &str) -> bool { !n.is_empty() && n != "." && n != ".." && !n.starts_with('.') && !n.contains(['/', '\\', '\0', '\n', '\t', '\r']) && n.len() <= 255 }
/// The folder on show: the last breadcrumb's path ("" is the project root).
pub fn cur_dir(files: &Value) -> String { files["trail"].as_array().and_then(|t| t.last()).and_then(|c| c["value"].as_str()).unwrap_or("").to_string() }
pub fn join(dir: &str, name: &str) -> String { if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") } }
/// The Lucide icon for a row's kind and the colour token it takes.
pub fn glyph(kind: &str) -> (&'static str, &'static str) {
    match kind { "folder" => ("folder", "ok"), "board" => ("circuit-board", "accent"), "schematic" => ("cpu", "accent"), "3d" => ("box", "accent"),
        "image" => ("image", "text"), "pdf" | "text" => ("file-text", "text"), _ => ("file", "dim") }
}
/// The context menu of a row: (id, label).
pub fn menu_items(r: &FileRow) -> Vec<(&'static str, &'static str)> {
    let mut v = vec![];
    if !r.action.is_empty() { v.push(("open", "Open")) }
    if !r.down().is_empty() { v.push(("download", if r.kind == "folder" { "Download as zip" } else { "Download" })) }
    v.push(("rename", "Rename")); v.push(("copy", "Copy path")); v.push(("delete", "Delete"));
    v
}
/// The decoded `path` of a `/img?path=…` url (the open file's absolute path).
pub fn path_of_url(url: &str) -> String {
    let q = url.split_once("path=").map_or("", |(_, r)| r.split('&').next().unwrap_or(""));
    let b = q.as_bytes(); let mut o = Vec::with_capacity(b.len()); let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() { if let Ok(x) = u8::from_str_radix(&q[i + 1..i + 3], 16) { o.push(x); i += 3; continue } }
        o.push(b[i]); i += 1;
    }
    String::from_utf8_lossy(&o).into_owned()
}
/// `name` made unique in `taken` by a " (n)" before its extension.
pub fn unique(name: &str, taken: &dyn Fn(&str) -> bool) -> String {
    if !taken(name) { return name.to_string() }
    let (stem, ext) = match name.rfind('.') { Some(i) if i > 0 => (&name[..i], &name[i..]), _ => (name, "") };
    (1..).map(|n| format!("{stem} ({n}){ext}")).find(|c| !taken(c)).unwrap()
}
/// A name the saver may write: no path parts.
pub fn save_name(n: &str) -> String { let n = n.rsplit(['/', '\\']).next().unwrap_or(""); let n = n.trim_start_matches('.'); if n.is_empty() { "download".into() } else { n.replace(['\0', '\n', '\r'], "_") } }
pub fn downloads_dir() -> PathBuf { std::env::var_os("BACKPLANE_GPUI_DOWNLOADS").map(PathBuf::from).unwrap_or_else(|| crate::plat::user_home().unwrap_or_default().join("Downloads")) }

impl FileRow { pub fn down(&self) -> String { self.down.clone() } }

// ------------------------------------------------------------------ state, events

fn thread_id(this: &Backplane) -> String { let t = this.data["thread"]["id"].as_str().unwrap_or(""); t.rsplit_once('|').map_or(t, |(_, i)| i).to_string() }
fn rows_now(this: &Backplane) -> (Vec<FileRow>, String) { let f = &this.data["thread"]["viewer"]["files"]; (viewer::file_rows(f), cur_dir(f)) }
fn filter_text(this: &Backplane, cx: &App) -> String { this.screens.files.filter.as_ref().map_or(String::new(), |f| f.read(cx).content.to_string()) }

/// Called each frame (glue.rs): make the fields once, reset on a new folder, start the fetch a preview needs.
pub fn sync(this: &mut Backplane, cx: &mut Context<Backplane>) {
    if this.screens.files.filter.is_none() {
        let f = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        f.update(cx, |i, _| i.set_placeholder("Filter this folder"));
        cx.observe(&f, |this, _, cx| { this.screens.files.sel = None; cx.notify(); }).detach();
        let n = cx.new(|cx| Composer::with_context(cx, "AskInput"));
        cx.subscribe(&n, |this, _, ev: &ComposerEvent, cx| if matches!(ev, ComposerEvent::Submit) { commit_name(this, cx); }).detach();
        this.screens.files.filter = Some(f); this.screens.files.name = Some(n); this.screens.files.focus = Some(cx.focus_handle());
    }
    let viewer = &this.data["thread"]["viewer"];
    let open = viewer["open"].as_str().unwrap_or("").to_string();
    if open == "files" {
        let dir = cur_dir(&viewer["files"]);
        if dir != this.screens.files.dir { this.screens.files.dir = dir; this.screens.files.sel = None; this.screens.files.menu = None; this.screens.files.mode = Mode::Browse;
            if let Some(f) = this.screens.files.filter.clone() { f.update(cx, |i, cx| i.set(String::new(), cx)); } }
    }
    let file = viewer["file"].clone();
    let url = file["url"].as_str().unwrap_or("").to_string();
    match open.as_str() {
        "text" if !url.is_empty() && !this.screens.files.texts.contains_key(&url) => fetch_text(this, url, cx),
        "pdf" if !url.is_empty() => {
            let path = path_of_url(&url);
            if this.screens.files.pdf.path != path { this.screens.files.pdf = Pdf { path: path.clone(), page: 1, ..Default::default() }; }
            if !this.screens.files.pdf.asked && !path.is_empty() { this.screens.files.pdf.asked = true; ask_pdf(this, 1); }
        }
        _ => {}
    }
}

fn ask_pdf(this: &mut Backplane, page: u32) {
    let path = this.screens.files.pdf.path.clone(); this.screens.files.pdf.page = page; this.screens.files.pdf.err.clear();
    let _ = this.tx.try_send(Command::Rpc("pdf.page".into(), Secret(json!({"path": path, "page": page})), "files-pdf"));
}
fn fetch_text(this: &mut Backplane, url: String, cx: &mut Context<Backplane>) {
    this.screens.files.texts.insert(url.clone(), Text::Loading);
    let Some(t) = this.screens.attacher.target.clone() else { this.screens.files.texts.insert(url, Text::Failed("no hub address".into())); return };
    cx.spawn(async move |weak, cx| {
        let u = url.clone();
        let r = cx.background_executor().spawn(async move { http_get(&t, &u, MAX_TEXT) }).await;
        let _ = weak.update(cx, |this, cx| { this.screens.files.texts.insert(url, match r { Ok(b) => Text::Ready(String::from_utf8_lossy(&b).into_owned()), Err(e) => Text::Failed(e) }); cx.notify(); });
    }).detach();
}
/// GET `url` on the hub with the token, at most `max` bytes.
pub fn http_get(t: &Target, url: &str, max: usize) -> Result<Vec<u8>, String> {
    if !super::images::safe_url(url) { return Err("not a hub path".into()) }
    use std::io::{Read, Write};
    let mut s = t.stream().map_err(|e| e.to_string())?; s.set_timeouts(std::time::Duration::from_secs(60)).map_err(|e| e.to_string())?;
    s.write_all(super::images::request(t, url).as_bytes()).map_err(|e| e.to_string())?;
    let mut b = Vec::new(); s.take(max as u64 + 65536).read_to_end(&mut b).map_err(|e| e.to_string())?;
    let (status, body) = super::images::split(&b).ok_or("the hub gave no answer")?;
    if status != 200 { return Err(format!("the hub answered {status}")) }
    Ok(body[..body.len().min(max)].to_vec())
}

fn say(this: &mut Backplane, s: impl Into<String>, cx: &mut Context<Backplane>) { this.screens.files.status = s.into(); cx.notify(); }
fn refresh(this: &mut Backplane) { let dir = this.screens.files.dir.clone(); let _ = this.tx.try_send(Command::Action("fm-cd".into(), dir)); }
fn op(this: &mut Backplane, p: Value) {
    let mut p = p; p["thread"] = json!(thread_id(this));
    let _ = this.tx.try_send(Command::Rpc("files.op".into(), Secret(p), "files-op"));
}

/// A reply to one of our requests: `files-op` (ok: reload the folder; else say why), `files-pdf` (the page's png).
pub fn rpc_done(this: &mut Backplane, tag: &str, ok: bool, why: &str, cx: &mut Context<Backplane>) {
    match tag {
        "files-op" => {
            if ok { let v: Value = serde_json::from_str(why).unwrap_or(Value::Null); let o = v["op"].as_str().unwrap_or("");
                say(this, match o { "mkdir" => "Folder created", "rename" => "Renamed", "delete" => "Deleted", _ => "Uploaded" }, cx); refresh(this); }
            else { say(this, if why.is_empty() { "The hub refused".to_string() } else { why.to_string() }, cx); }
        }
        "files-pdf" => {
            let v: Value = serde_json::from_str(why).unwrap_or(Value::Null);
            let p = &mut this.screens.files.pdf;
            if ok && v["pdfPath"].as_str() == Some(p.path.as_str()) { p.png = v["pdfPng"].as_str().unwrap_or("").to_string(); p.pages = v["pdfPages"].as_u64().unwrap_or(0) as u32; p.err = v["pdfErr"].as_str().unwrap_or("").to_string(); }
            else if !ok { p.err = why.to_string(); }
            cx.notify();
        }
        _ => {}
    }
}

fn commit_name(this: &mut Backplane, cx: &mut Context<Backplane>) {
    let Some(name) = this.screens.files.name.clone() else { return };
    let text = name.read(cx).content.trim().to_string();
    let dir = this.screens.files.dir.clone();
    match this.screens.files.mode.clone() {
        Mode::NewFolder => { if !valid_name(&text) { return say(this, "A folder name is one plain name, not starting with a dot", cx) } op(this, json!({"op":"mkdir","path":join(&dir, &text)})); }
        Mode::Rename(path) => { if !valid_name(&text) { return say(this, "A name is one plain name, not starting with a dot", cx) } op(this, json!({"op":"rename","path":path,"to":text})); }
        _ => return,
    }
    this.screens.files.mode = Mode::Browse; cx.notify();
}

/// Escape (the window's ClosePopup action runs before any key listener): while the Files page has the keys (its list, filter or name field focused) it clears the
/// filter, cancels a rename or a delete, or goes up a folder, and nothing else sees the key. True when it took it.
pub fn escape(this: &mut Backplane, window: &mut Window, cx: &mut Context<Backplane>) -> bool {
    if this.data["thread"]["viewer"]["open"].as_str() != Some("files") { return false }
    let f = &this.screens.files;
    let mine = f.focus.as_ref().map_or(false, |h| h.is_focused(window)) || f.filter.as_ref().map_or(false, |e| e.read(cx).focus_handle(cx).is_focused(window)) || f.name.as_ref().map_or(false, |e| e.read(cx).focus_handle(cx).is_focused(window));
    if !mine { return false }
    act(this, Do::Cancel, Some(window), cx);
    if let Some(h) = this.screens.files.focus.clone() { window.focus(&h); }
    true
}

/// A press the page can make; every one is also a script step (script.rs).
#[derive(Clone, Debug)]
pub enum Do { Open(usize), Up, Back, Download(usize), DownloadDir, Rename(usize), NewFolder, Delete(usize), ConfirmDelete, Copy(usize), Upload(Vec<PathBuf>), PickUpload, Cancel, Select(usize), Key(String) }

pub fn act(this: &mut Backplane, d: Do, window: Option<&mut Window>, cx: &mut Context<Backplane>) {
    let (rows, dir) = rows_now(this);
    let vis = visible(&rows, &filter_text(this, cx));
    let row = |i: usize| vis.get(i).and_then(|&k| rows.get(k)).cloned();
    this.screens.files.menu = None;
    match d {
        Do::Select(i) => { this.screens.files.sel = Some(i); if let Some(w) = window { if let Some(f) = &this.screens.files.focus { w.focus(f); } } }
        Do::Open(i) => if let Some(r) = row(i) { if !r.action.is_empty() { let _ = this.tx.try_send(Command::Action(r.action.clone(), r.value.clone())); } else { say(this, format!("{} has no preview here", r.label), cx) } },
        Do::Up => { let _ = this.tx.try_send(Command::Action("fm-up".into(), String::new())); }
        Do::Back => { let _ = this.tx.try_send(Command::Action("fm".into(), String::new())); }
        Do::Download(i) => if let Some(r) = row(i) { download(this, r.down(), r.label.clone(), r.kind == "folder", cx) },
        Do::DownloadDir => { let url = this.data["thread"]["viewer"]["files"]["zip"].as_str().unwrap_or("").to_string(); let n = this.data["thread"]["viewer"]["files"]["zipName"].as_str().unwrap_or("project.zip").to_string(); download(this, url, n, true, cx) }
        Do::Rename(i) => if let Some(r) = row(i) { this.screens.files.mode = Mode::Rename(r.value.clone()); if let Some(n) = this.screens.files.name.clone() { n.update(cx, |x, cx| x.set(r.label.clone(), cx)); if let Some(w) = window { w.focus(&n.read(cx).focus_handle(cx)); } } },
        Do::NewFolder => { this.screens.files.mode = Mode::NewFolder; if let Some(n) = this.screens.files.name.clone() { n.update(cx, |x, cx| x.set(String::new(), cx)); if let Some(w) = window { w.focus(&n.read(cx).focus_handle(cx)); } } }
        Do::Delete(i) => if let Some(r) = row(i) { this.screens.files.mode = Mode::Delete(r.value.clone()) },
        Do::ConfirmDelete => { if let Mode::Delete(p) = this.screens.files.mode.clone() { op(this, json!({"op":"delete","path":p})); } this.screens.files.mode = Mode::Browse; }
        Do::Copy(i) => if let Some(r) = row(i) { cx.write_to_clipboard(ClipboardItem::new_string(r.value.clone())); say(this, format!("Copied {}", r.value), cx) },
        Do::PickUpload => { let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: None });
            cx.spawn(async move |weak, cx| { if let Ok(Ok(Some(paths))) = rx.await { let _ = weak.update(cx, |this, cx| act(this, Do::Upload(paths), None, cx)); } }).detach(); }
        Do::Upload(paths) => upload(this, paths, &dir, cx),
        Do::Cancel => { if this.screens.files.mode != Mode::Browse { this.screens.files.mode = Mode::Browse } else if let Some(f) = this.screens.files.filter.clone() { if !f.read(cx).content.is_empty() { f.update(cx, |i, cx| i.set(String::new(), cx)); } else if !this.screens.files.dir.is_empty() { let _ = this.tx.try_send(Command::Action("fm-up".into(), String::new())); } } }
        Do::Key(k) if k == "name-ok" => return commit_name(this, cx),
        Do::Key(k) => {
            let sel = this.screens.files.sel;
            match k.as_str() {
                "up" | "down" | "home" | "end" | "pageup" | "pagedown" => { let n = step(sel, vis.len(), &k); this.screens.files.sel = n; if let Some(i) = n { this.screens.files.scroll.scroll_to_item(i); } }
                "enter" => if let Some(i) = sel { return act(this, Do::Open(i), window, cx) },
                "backspace" | "left" => return act(this, Do::Up, window, cx),
                "right" => if let Some(i) = sel { if row(i).map_or(false, |r| r.kind == "folder") { return act(this, Do::Open(i), window, cx) } },
                "delete" => if let Some(i) = sel { return act(this, Do::Delete(i), window, cx) },
                "f2" => if let Some(i) = sel { return act(this, Do::Rename(i), window, cx) },
                "escape" => return act(this, Do::Cancel, window, cx),
                "find" => { if let (Some(f), Some(w)) = (this.screens.files.filter.clone(), window) { w.focus(&f.read(cx).focus_handle(cx)); } }
                _ => {}
            }
        }
    }
    cx.notify();
}

/// Save `url` (a hub path) as a file in the downloads folder (never over a file). The window says where it went.
pub fn download(this: &mut Backplane, url: String, name: String, zip: bool, cx: &mut Context<Backplane>) {
    let Some(t) = this.screens.attacher.target.clone() else { return say(this, "No hub address to download from", cx) };
    if url.is_empty() { return say(this, "This entry cannot be downloaded", cx) }
    let mut file = save_name(&name); if zip && !file.to_lowercase().ends_with(".zip") { file.push_str(".zip") }
    say(this, format!("Downloading {file}…"), cx);
    cx.spawn(async move |weak, cx| {
        let r = cx.background_executor().spawn(async move {
            let b = http_get(&t, &url, 260 << 20)?;
            let dir = downloads_dir(); std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let name = unique(&file, &|n| dir.join(n).exists()); let p = dir.join(&name);
            std::fs::write(&p, &b).map_err(|e| e.to_string())?; Ok::<_, String>((p, b.len()))
        }).await;
        let _ = weak.update(cx, |this, cx| { let (m, ok) = match &r { Ok((p, n)) => (format!("Saved {} ({} bytes)", p.display(), n), true), Err(e) => (format!("Download failed: {e}"), false) };
            this.m("files_download", json!({"ok": ok, "message": m, "path": r.as_ref().ok().map(|(p, _)| p.display().to_string()), "bytes": r.as_ref().ok().map(|x| x.1)})); say(this, m, cx); });
    }).detach();
}

fn upload(this: &mut Backplane, paths: Vec<PathBuf>, dir: &str, cx: &mut Context<Backplane>) {
    let thread = thread_id(this); let dir = dir.to_string();
    say(this, format!("Uploading {} file(s)…", paths.len()), cx);
    cx.spawn(async move |weak, cx| {
        let read = cx.background_executor().spawn(async move {
            use base64::Engine;
            paths.into_iter().map(|p| { let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if !valid_name(&name) { return Err(format!("\"{name}\" cannot be uploaded (hidden or odd name)")) }
                let md = std::fs::metadata(&p).map_err(|e| e.to_string())?; if !md.is_file() { return Err(format!("{name} is not a file")) }
                if md.len() > 24 << 20 { return Err(format!("{name} is over 24 MB")) }
                let b = std::fs::read(&p).map_err(|e| e.to_string())?; Ok((name, base64::engine::general_purpose::STANDARD.encode(b))) }).collect::<Vec<Result<(String, String), String>>>()
        }).await;
        let _ = weak.update(cx, |this, cx| {
            for r in read { match r {
                Ok((name, data)) => { let _ = this.tx.try_send(Command::Rpc("files.op".into(), Secret(json!({"thread": thread, "op":"write", "path": join(&dir, &name), "data": data})), "files-op")); }
                Err(e) => say(this, e, cx),
            } }
        });
    }).detach();
}

// ------------------------------------------------------------------ elements

fn small_btn(cx: &mut Context<Backplane>, light: bool, id: &str, icon: &'static str, label: &str, d: Do) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    div().id(SharedString::from(id.to_string())).flex().items_center().gap_1().px_2().py_1().text_size(px(12.)).cursor_pointer().border_1().border_color(c("line")).bg(c("raise")).hover(|s| s.bg(c("panel")))
        .child(crate::art::icon(icon, 13., c("text"))).child(label.to_string())
        .on_click(cx.listener(move |this, _, w, cx| act(this, d.clone(), Some(w), cx))).into_any_element()
}

/// `actions`: the viewer pane's own controls (screens::viewer), at the end of the page's bar.
pub fn files_page(cx: &mut Context<Backplane>, light: bool, files: &Value, ui: &FilesUi, actions: AnyElement) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let rows = viewer::file_rows(files);
    let q = ui.filter.as_ref().map(|f| f.read(cx).content.to_string()).unwrap_or_default();
    let vis = visible(&rows, &q);
    let trail = viewer::crumbs(files);
    let focus = ui.focus.clone();
    // bar: Up, breadcrumbs, then the actions
    let mut crumbs = div().flex().flex_wrap().items_center().flex_1().min_w_0().text_size(px(12.));
    for (i, cr) in trail.iter().enumerate() {
        let (a, v) = (cr.action.clone(), cr.value.clone());
        crumbs = crumbs.child(div().id(SharedString::from(format!("crumb-{i}"))).px_1().cursor_pointer().text_color(c(if i + 1 == trail.len() { "text" } else { "accent" })).hover(|s| s.bg(c("raise"))).child(cr.label.clone())
            .on_click(cx.listener(move |this, _, _, cx| { let _ = this.tx.try_send(Command::Action(a.clone(), v.clone())); cx.notify(); })));
        if i + 1 < trail.len() { crumbs = crumbs.child(div().text_color(c("faint")).child("/")); }
    }
    let at_root = trail.len() <= 1;
    let mut bar = div().flex().items_center().gap_2().px_3().py_2().border_b_1().border_color(c("line"))
        .child(div().id("fm-up").px_1().cursor_pointer().when(at_root, |d| d.opacity(0.35)).child(crate::art::icon("arrow-up", 14., c("text"))).on_click(cx.listener(|this, _, w, cx| act(this, Do::Up, Some(w), cx))))
        .child(crumbs);
    bar = bar.child(small_btn(cx, light, "fm-new", "folder-plus", "New", Do::NewFolder)).child(small_btn(cx, light, "fm-upload", "arrow-up", "Upload", Do::PickUpload)).child(small_btn(cx, light, "fm-zip", "download", "Zip", Do::DownloadDir)).child(actions);
    let mut col = div().size_full().flex().flex_col().relative();
    if let Some(f) = focus.clone() { col = col.track_focus(&f); }
    col = col.key_context("FilesPage").on_key_down(cx.listener(|this, e: &KeyDownEvent, w, cx| {
        let k = e.keystroke.key.as_str();
        let typing = |this: &Backplane| this.screens.files.filter.as_ref().map_or(false, |f| f.read(cx).focus_handle(cx).is_focused(w)) || this.screens.files.name.as_ref().map_or(false, |f| f.read(cx).focus_handle(cx).is_focused(w));
        if typing(this) { if k == "escape" { act(this, Do::Cancel, Some(w), cx); if let Some(f) = &this.screens.files.focus { w.focus(f); } } else if k == "down" { if let Some(f) = &this.screens.files.focus { w.focus(f); } act(this, Do::Key("down".into()), Some(w), cx); } return; }
        let m = &e.keystroke.modifiers;
        let key = if (m.control || m.platform) && k == "f" { "find".to_string() } else if m.control || m.alt || m.platform { return } else { k.to_string() };
        act(this, Do::Key(key), Some(w), cx);
    }));
    col = col.child(bar);
    // filter + status line
    let filt = div().flex().items_center().gap_2().px_3().py_1().border_b_1().border_color(c("line")).child(crate::art::icon("search", 13., c("dim")))
        .child(div().flex_1().min_w_0().px_2().py_1().border_1().border_color(c("line")).bg(c("panel")).text_size(px(12.)).children(ui.filter.clone()))
        .child(div().text_size(px(11.)).text_color(c("dim")).child(if q.trim().is_empty() { format!("{} items", rows.len()) } else { format!("{} of {}", vis.len(), rows.len()) }));
    col = col.child(filt);
    match &ui.mode {
        Mode::Rename(_) | Mode::NewFolder => {
            col = col.child(div().flex().items_center().gap_2().px_3().py_1().border_b_1().border_color(c("line")).bg(c("raise")).text_size(px(12.))
                .child(if ui.mode == Mode::NewFolder { "New folder" } else { "Rename to" })
                .child(div().flex_1().min_w_0().px_2().py_1().border_1().border_color(c("accent")).bg(c("view")).children(ui.name.clone()))
                .child(small_btn(cx, light, "fm-name-ok", "plus", "OK", Do::Key("name-ok".into())))
                .child(small_btn(cx, light, "fm-name-cancel", "x", "Cancel", Do::Cancel)));
        }
        Mode::Delete(p) => {
            col = col.child(div().flex().items_center().gap_2().px_3().py_1().border_b_1().border_color(c("line")).bg(c("raise")).text_size(px(12.)).child(div().flex_1().min_w_0().child(format!("Delete {p}? This cannot be undone.")))
                .child(small_btn(cx, light, "fm-del-yes", "trash", "Delete", Do::ConfirmDelete)).child(small_btn(cx, light, "fm-del-no", "x", "Keep", Do::Cancel)));
        }
        Mode::Browse => {}
    }
    if !ui.status.is_empty() { col = col.child(div().px_3().py_1().text_size(px(11.)).text_color(c("dim")).border_b_1().border_color(c("line")).child(ui.status.clone())); }
    // the rows: their own scroll pane
    let mut list = div().id("files-list").flex_1().min_h_0().overflow_y_scroll().track_scroll(&ui.scroll).flex().flex_col();
    for (n, &k) in vis.iter().take(MAX_ROWS).enumerate() {
        let r = &rows[k];
        let (ic, tok) = glyph(&r.kind);
        let on = ui.sel == Some(n);
        let (a_open, has_open) = (n, !r.action.is_empty());
        list = list.child(div().id(SharedString::from(format!("file-{n}"))).flex().items_center().gap_3().px_3().py_1().cursor_pointer().hover(|s| s.bg(c("raise"))).when(on, |d| d.bg(c("raise")).border_l_2().border_color(c("accent")))
            .child(crate::art::icon(ic, 15., c(tok)))
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_color(c(if has_open || r.kind == "folder" { "text" } else { "dim" })).child(r.label.clone()))
            .child(div().text_size(px(11.)).text_color(c("dim")).child(r.detail.clone()))
            .on_click(cx.listener(move |this, e: &ClickEvent, w, cx| { if e.click_count() >= 2 || this.screens.files.sel == Some(a_open) { act(this, Do::Open(a_open), Some(w), cx) } else { act(this, Do::Select(a_open), Some(w), cx) } }))
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, e: &MouseDownEvent, w, cx| { this.screens.files.sel = Some(a_open); this.screens.files.menu = Some((a_open, e.position)); if let Some(f) = &this.screens.files.focus { w.focus(f); } cx.notify(); })));
    }
    if rows.len() > MAX_ROWS { list = list.child(div().px_3().py_1().text_color(c("dim")).child(format!("… {} more; filter to find them", rows.len() - MAX_ROWS))); }
    if vis.is_empty() { list = list.child(div().px_3().py_2().text_color(c("dim")).child(if files["ready"] == false { "Loading…".to_string() } else if !files["error"].as_str().unwrap_or("").is_empty() { files["error"].as_str().unwrap_or("").to_string() } else if rows.is_empty() { "Empty folder. Drop files here to upload.".to_string() } else { "Nothing matches".to_string() })); }
    list = list.on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| { this.m("files_drop", json!({"paths": paths.paths().iter().map(|p| p.display().to_string()).collect::<Vec<_>>()})); act(this, Do::Upload(paths.paths().to_vec()), None, cx) }));
    col = col.child(list);
    if let Some((i, pos)) = ui.menu {
        if let Some(r) = vis.get(i).and_then(|&k| rows.get(k)) {
            let items = menu_items(r);
            let menu = div().id("fm-menu").occlude().min_w(px(150.)).py_1().border_1().border_color(c("line")).bg(c("view")).shadow_md().text_size(px(12.))
                .on_mouse_down_out(cx.listener(|this, _, _, cx| { this.screens.files.menu = None; cx.notify(); }))
                .children(items.into_iter().map(|(id, label)| div().id(SharedString::from(format!("fm-menu-{id}"))).px_3().py_1().cursor_pointer().hover(|s| s.bg(c("raise"))).text_color(c(if id == "delete" { "bad" } else { "text" })).child(label)
                    .on_click(cx.listener(move |this, _, w, cx| { let d = match id { "open" => Do::Open(i), "download" => Do::Download(i), "rename" => Do::Rename(i), "copy" => Do::Copy(i), _ => Do::Delete(i) }; act(this, d, Some(w), cx) }))));
            col = col.child(deferred(anchored().position(pos).snap_to_window_with_margin(px(8.)).child(menu)).with_priority(2));
        }
    }
    col.into_any_element()
}

/// An image, PDF or text file the hub opened from the Files tab: a bar (Back, name, Download) over the preview.
pub fn preview_page(cx: &mut Context<Backplane>, reg: &super::timeline::Reg, light: bool, open: &str, viewer: &Value, ui: &FilesUi, images: &super::images::Images, target: Option<&Target>, actions: AnyElement) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let name = viewer["file"]["name"].as_str().unwrap_or("").to_string();
    let url = viewer["file"]["url"].as_str().unwrap_or("").to_string();
    let mut bar = div().flex().items_center().gap_2().px_3().py_2().border_b_1().border_color(c("line"))
        .child(small_btn(cx, light, "fm-back", "chevron-left", "Files", Do::Back))
        .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_size(px(12.)).child(name.clone()));
    if open == "pdf" && ui.pdf.pages > 0 {
        let (p, n) = (ui.pdf.page, ui.pdf.pages);
        bar = bar.child(div().text_size(px(11.)).text_color(c("dim")).child(format!("{p} / {n}")))
            .child(div().id("pdf-prev").px_2().cursor_pointer().when(p <= 1, |d| d.opacity(0.35)).child(crate::art::icon("chevron-left", 14., c("text"))).on_click(cx.listener(move |this, _, _, cx| { if p > 1 { ask_pdf(this, p - 1); cx.notify(); } })))
            .child(div().id("pdf-next").px_2().cursor_pointer().when(p >= n, |d| d.opacity(0.35)).child(crate::art::icon("chevron-right", 14., c("text"))).on_click(cx.listener(move |this, _, _, cx| { if p < n { ask_pdf(this, p + 1); cx.notify(); } })));
    }
    let bar = bar.child(actions);
    let img_el = |url: &str| -> AnyElement {
        match images.get(target, url) {
            Slot::Ready(img) => { let (w, h) = super::images::dims(&img.bytes).map_or((480., 360.), |(w, h)| super::images::fit(w, h, 480., 4000.)); div().p_3().child(div().w(px(w)).h(px(h)).flex_none().child(gpui::img(ImageSource::Image(img)).size_full().object_fit(ObjectFit::Contain))).into_any_element() }
            Slot::Loading => div().p_3().text_color(c("dim")).child("Loading…").into_any_element(),
            Slot::Failed => div().p_3().text_color(c("bad")).child("Could not load the file").into_any_element(),
        }
    };
    let body: AnyElement = match open {
        "image" => img_el(&url),
        "pdf" => if !ui.pdf.err.is_empty() { div().p_3().text_color(c("bad")).child(ui.pdf.err.clone()).into_any_element() }
            else if ui.pdf.png.is_empty() { div().p_3().text_color(c("dim")).child("Drawing the page…").into_any_element() }
            else { img_el(&format!("/img?path={}", super::attach::enc(&ui.pdf.png))) },
        _ if viewer["file"]["blocks"].as_array().map_or(false, |b| !b.is_empty()) => {
            // a Markdown note: the hub sent its nodes, drawn as the chat draws a reply
            let blocks: Vec<Value> = viewer["file"]["blocks"].as_array().cloned().unwrap_or_default();
            let parsed = super::md::parse(&blocks);
            div().flex().flex_col().gap_2().p_3().children(super::timeline::blocks(cx, reg, light, &parsed, "file")).into_any_element()
        }
        _ => match ui.texts.get(&url) {
            Some(Text::Ready(t)) => text_block(light, &name, t),
            Some(Text::Failed(e)) => div().p_3().text_color(c("bad")).child(e.clone()).into_any_element(),
            _ => div().p_3().text_color(c("dim")).child("Loading…").into_any_element(),
        },
    };
    div().size_full().flex().flex_col().child(bar).child(div().id("fm-preview").flex_1().min_h_0().overflow_y_scroll().child(body)).into_any_element()
}

/// Text lines in the mono font; a Markdown note bolds its headings and dims its rules.
fn text_block(light: bool, name: &str, t: &str) -> AnyElement {
    let c = |n: &str| theme::color(light, n);
    let md = name.to_lowercase().ends_with(".md") || name.to_lowercase().ends_with(".markdown");
    div().flex().flex_col().p_3().text_size(px(12.)).children(t.lines().take(5000).map(|l| {
        let h = md && l.starts_with('#');
        let mut d = div().font(crate::art::mono_font()).whitespace_nowrap().min_h(px(16.));
        if h { d = d.font_weight(FontWeight::BOLD).text_color(c("accent")); }
        d.child(l.to_string()).into_any_element()
    }).collect::<Vec<_>>()).into_any_element()
}

/// A script step `files`: "select:N", "open:N", "key:down", "filter:text", "new:name", "rename:N:name", "delete:N", "confirm_delete", "menu:N", "upload:PATH",
/// "download:N" (a row's index or its name), "zip", "copy:N", "up", "back", "cancel". N counts the visible rows.
pub fn script(this: &mut Backplane, spec: &str, cx: &mut Context<Backplane>) -> Result<(), String> {
    let (cmd, arg) = spec.split_once(':').unwrap_or((spec, ""));
    let idx = |a: &str, this: &Backplane, cx: &App| -> Result<usize, String> {
        if let Ok(n) = a.parse::<usize>() { return Ok(n) }
        let (rows, _) = rows_now(this); let vis = visible(&rows, &filter_text(this, cx));
        vis.iter().position(|&k| rows[k].label == a).ok_or_else(|| format!("no row named {a}"))
    };
    match cmd {
        "select" => { let i = idx(arg, this, cx)?; act(this, Do::Select(i), None, cx) }
        "open" => { let i = idx(arg, this, cx)?; act(this, Do::Open(i), None, cx) }
        "key" => act(this, Do::Key(arg.to_string()), None, cx),
        "filter" => { let f = this.screens.files.filter.clone().ok_or("no filter field")?; f.update(cx, |i, cx| i.set(arg.to_string(), cx)); this.screens.files.sel = None; cx.notify() }
        "new" => { act(this, Do::NewFolder, None, cx); let n = this.screens.files.name.clone().ok_or("no name field")?; n.update(cx, |i, cx| i.set(arg.to_string(), cx)); commit_name(this, cx) }
        "rename" => { let (a, to) = arg.split_once(':').ok_or("rename:N:name")?; let i = idx(a, this, cx)?; act(this, Do::Rename(i), None, cx); let n = this.screens.files.name.clone().ok_or("no name field")?; n.update(cx, |x, cx| x.set(to.to_string(), cx)); commit_name(this, cx) }
        "delete" => { let i = idx(arg, this, cx)?; act(this, Do::Delete(i), None, cx) }
        "confirm_delete" => act(this, Do::ConfirmDelete, None, cx),
        "menu" => { let i = idx(arg, this, cx)?; this.screens.files.menu = Some((i, point(px(300.), px(200.)))); this.screens.files.sel = Some(i); cx.notify() }
        "upload" => act(this, Do::Upload(vec![PathBuf::from(arg)]), None, cx),
        "download" => { let i = idx(arg, this, cx)?; act(this, Do::Download(i), None, cx) }
        "zip" => act(this, Do::DownloadDir, None, cx),
        "copy" => { let i = idx(arg, this, cx)?; act(this, Do::Copy(i), None, cx) }
        "shot" => { let out = std::process::Command::new("import").args(["-window", "root", arg]).status().map_err(|e| e.to_string())?; if !out.success() { return Err("import failed".into()) } }
        "up" => act(this, Do::Up, None, cx),
        "back" => act(this, Do::Back, None, cx),
        "cancel" => act(this, Do::Cancel, None, cx),
        _ => return Err(format!("unknown files step {cmd}")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    fn row(l: &str, k: &str) -> FileRow { FileRow { label: l.into(), action: if k == "folder" { "fm-cd".into() } else { "fm-open".into() }, value: l.into(), detail: String::new(), kind: k.into(), down: format!("/files/get?thread=t&path={l}"), } }
    #[test] fn the_filter_keeps_names_that_contain_the_text_in_any_case() {
        let rs = vec![row("Alpha.txt", "text"), row("beta", "folder"), row("gamma.kicad_pcb", "board")];
        assert_eq!(visible(&rs, ""), vec![0, 1, 2]); assert_eq!(visible(&rs, "A"), vec![0, 1, 2]); assert_eq!(visible(&rs, " KICAD "), vec![2]); assert_eq!(visible(&rs, "zzz"), Vec::<usize>::new());
    }
    #[test] fn arrow_keys_stay_inside_the_list() {
        assert_eq!(step(None, 3, "down"), Some(0)); assert_eq!(step(None, 3, "up"), Some(2)); assert_eq!(step(Some(2), 3, "down"), Some(2)); assert_eq!(step(Some(0), 3, "up"), Some(0));
        assert_eq!(step(Some(1), 5, "end"), Some(4)); assert_eq!(step(Some(4), 5, "home"), Some(0)); assert_eq!(step(Some(1), 50, "pagedown"), Some(11)); assert_eq!(step(Some(3), 50, "pageup"), Some(0));
        assert_eq!(step(Some(1), 0, "down"), None); assert_eq!(step(Some(9), 3, "x"), Some(2)); assert_eq!(step(None, 3, "x"), None);
    }
    #[test] fn names_follow_the_hubs_rule() {
        for n in ["a", "my file.txt", "x.kicad_pcb"] { assert!(valid_name(n), "{n}") }
        for n in ["", ".", "..", ".env", "a/b", "a\\b", "a\tb", "a\nb"] { assert!(!valid_name(n), "{n:?}") }
        assert!(!valid_name(&"x".repeat(256)));
    }
    #[test] fn the_folder_on_show_is_the_last_crumb() {
        let f: Value = serde_json::from_str(r#"{"trail":[{"label":"p","action":"fm-cd","value":""},{"label":"docs","action":"fm-cd","value":"docs"},{"label":"sub","action":"fm-cd","value":"docs/sub"}]}"#).unwrap();
        assert_eq!(cur_dir(&f), "docs/sub"); assert_eq!(cur_dir(&Value::Null), ""); assert_eq!(join("docs/sub", "a"), "docs/sub/a"); assert_eq!(join("", "a"), "a");
    }
    #[test] fn kinds_have_their_icons_and_menus_their_items() {
        assert_eq!(glyph("folder").0, "folder"); assert_eq!(glyph("board").0, "circuit-board"); assert_eq!(glyph("schematic").0, "cpu"); assert_eq!(glyph("3d").0, "box"); assert_eq!(glyph("image").0, "image"); assert_eq!(glyph("pdf").0, "file-text"); assert_eq!(glyph("").0, "file");
        let m: Vec<_> = menu_items(&row("d", "folder")).into_iter().map(|x| x.0).collect(); assert_eq!(m, ["open", "download", "rename", "copy", "delete"]);
        let mut r = row("f", "text"); r.action.clear(); r.down.clear(); let m: Vec<_> = menu_items(&r).into_iter().map(|x| x.0).collect(); assert_eq!(m, ["rename", "copy", "delete"]);
        assert_eq!(menu_items(&row("d", "folder"))[1].1, "Download as zip");
    }
    #[test] fn a_pdfs_path_comes_back_out_of_its_url() {
        assert_eq!(path_of_url("/img?path=%2Fhome%2Fh%2Fa%20b.pdf"), "/home/h/a b.pdf"); assert_eq!(path_of_url("/img?path=/x/y.pdf&z=1"), "/x/y.pdf"); assert_eq!(path_of_url("/nothing"), "");
    }
    #[test] fn downloads_never_take_a_name_that_exists_or_a_path() {
        let taken = |n: &str| ["a.txt", "a (1).txt", "b"].contains(&n);
        assert_eq!(unique("a.txt", &taken), "a (2).txt"); assert_eq!(unique("c.txt", &taken), "c.txt"); assert_eq!(unique("b", &taken), "b (1)");
        assert_eq!(save_name("../../etc/passwd"), "passwd"); assert_eq!(save_name(".bashrc"), "bashrc"); assert_eq!(save_name(""), "download"); assert_eq!(save_name("dir/"), "download");
    }
}

//! A file clicked in a message or a note (`link-file`, and a `backplane://file/...` link): it opens INSIDE Backplane, never in the system's app.
//! The project that holds the file is found (the longest project folder it lies in); its thread is opened (the one on show when it is that
//! project's, else the project's newest), and the viewer tab that draws the file loads it: a board on Board, a schematic on Schematic, a
//! KiCad project on its board (else its schematic), a STEP, STP or GLB on Mech, a picture, PDF or note on Files; Gerbers on the project's Board.
//! A design file in no project (or a project with no thread yet) is drawn in the thread on show, scoped to that file (`allow`: the watch
//! takes that one file). Any other file outside every project goes to the system as before (`Os`). Pure but for `allow`.
use crate::state::State;
use serde_json::{json, Value};
use std::sync::Mutex;

/// What a file link does.
#[derive(Debug, PartialEq)]
pub enum Plan {
    /// open `thread` (its project unfolded) and run the viewer action `action` `value` there (`fm-open <rel>`, `view mech\t<path>`, `view board`)
    Open { thread: String, open: Vec<(String, bool)>, action: &'static str, value: String },
    /// a design file in no project: drawn in `thread` (the one on show) from its own path
    Scoped { thread: String, kind: &'static str, path: String },
    /// not a design file and in no project: the system opens it (the old way)
    Os(String),
    Note(&'static str),
}

pub use crate::md::links::design_kind;

/// The path a link names, absolute: `file://` and `%XX` taken off, a `:12`/`#L12` dropped, `~/` this home, a relative path from `base` (the
/// thread's folder). None when it is no path or a relative one has no base.
pub fn absolute(href: &str, base: &str, home: &str) -> Option<String> {
    let h = href.trim();
    let h = match h.strip_prefix("file://") { Some(r) => crate::md::refs::unpct(r.strip_prefix("localhost").unwrap_or(r)), None => h.to_string() };
    if h.is_empty() || h.contains(['\0', '\n']) || h.contains("://") { return None }
    let (p, _) = crate::data::link::split_line(&h);
    let p = p.to_string();
    let abs = if let Some(r) = p.strip_prefix("~/") { if home.is_empty() { return None } format!("{}/{r}", home.trim_end_matches('/')) }
        else if crate::plat::fs::is_abs(&p) { p }
        else { if base.is_empty() { return None } format!("{}/{}", base.trim_end_matches('/'), p.trim_start_matches("./")) };
    // `..` is resolved by words, never followed out by a link
    let mut parts: Vec<&str> = vec![];
    for x in abs.split('/') { match x { "" | "." => {}, ".." => { parts.pop(); }, x => parts.push(x) } }
    Some(format!("/{}", parts.join("/")))
}

fn under(path: &str, root: &str) -> bool { let r = root.trim_end_matches('/'); !r.is_empty() && path.len() > r.len() && path.starts_with(r) && path.as_bytes()[r.len()] == b'/' }
/// The project the path lies in (the deepest folder wins: a project inside another's folder is its own), with the path from its root.
pub fn owner<'a>(st: &'a State, path: &str) -> Option<(&'a str, String)> {
    st.projects.iter().filter(|(id, p)| !p.removed && !id.contains('~') && under(path, &p.root)).max_by_key(|(_, p)| p.root.trim_end_matches('/').len())
        .map(|(id, p)| (id.as_str(), path[p.root.trim_end_matches('/').len() + 1..].to_string()))
}
/// The folder a thread's files are from (its worktree's design root, else its project's).
pub fn root_of(st: &State, sel: &str) -> String {
    let Some(t) = st.threads.get(sel) else { return String::new() };
    if !t.worktree.is_empty() { return crate::files::kicad::canonical::design_root(&t.worktree) }
    st.projects.get(&t.project).map(|p| p.root.clone()).unwrap_or_default()
}
/// The thread a file of project `pid` opens in: the one on show when it is that project's, else the project's newest (pinned first).
pub fn thread_for(st: &State, pid: &str, sel: &str) -> Option<String> {
    if st.threads.get(sel).map_or(false, |t| t.project == pid && !t.deleted) { return Some(sel.to_string()) }
    st.threads.iter().filter(|(_, t)| t.project == pid && !t.archived && !t.deleted).max_by_key(|(i, t)| (t.pinned, t.activity, i.to_string())).map(|(i, _)| i.clone())
}

/// What a click on `href` opens, with `sel` the thread on show (its raw id). `exists` says whether a path is a file (the tests' world).
pub fn plan(st: &State, sel: &str, href: &str, home: &str, exists: &dyn Fn(&str) -> bool) -> Plan {
    let sel = if sel.contains('~') { "" } else { sel };
    let base = root_of(st, sel);
    let Some(path) = absolute(href, &base, home) else { return Plan::Note("That link does not open here.") };
    let name = path.rsplit('/').next().unwrap_or("");
    let kind = design_kind(name);
    let preview = crate::files::fm::preview(name);
    if let Some((pid, rel)) = owner(st, &path) {
        let open = vec![(format!("project-closed:{pid}"), false)];
        if let Some(thread) = thread_for(st, pid, sel) {
            let go = |action: &'static str, value: String| Plan::Open { thread: thread.clone(), open: open.clone(), action, value };
            return match kind {
                "board" | "schematic" => go("fm-open", rel),
                "model" => go("view", format!("mech\t{path}")),
                // a KiCad project: its board, else its schematic, else its folder
                "project" => { let stem = rel.strip_suffix(".kicad_pro").or_else(|| rel.strip_suffix(".KICAD_PRO")).unwrap_or(&rel).to_string();
                    let root = &path[..path.len() - rel.len()];
                    if exists(&format!("{root}{stem}.kicad_pcb")) { go("fm-open", format!("{stem}.kicad_pcb")) }
                    else if exists(&format!("{root}{stem}.kicad_sch")) { go("fm-open", format!("{stem}.kicad_sch")) }
                    else { go("fm-cd", crate::files::fm::parent(&rel)) } }
                // a Gerber or drill file: drawn itself on the Board page (kicad/gerber), as the Files tab opens it
                "gerber" => go("fm-open", rel),
                _ if !preview.is_empty() => go("fm-open", rel),
                _ => Plan::Os(path),
            }
        }
    }
    // in no project, or one with no thread yet: a design file is drawn in the thread on show, scoped to itself
    let scoped = match kind { "board" => "board", "schematic" => "schematic", "model" => "mech", "project" => {
            let stem = path.strip_suffix(".kicad_pro").unwrap_or(&path);
            if exists(&format!("{stem}.kicad_pcb")) { return scoped_plan(st, sel, "board", format!("{stem}.kicad_pcb")) }
            if exists(&format!("{stem}.kicad_sch")) { return scoped_plan(st, sel, "schematic", format!("{stem}.kicad_sch")) }
            return Plan::Note("That KiCad project has no board or schematic beside it.") }
        "gerber" => "board",
        _ => "" };
    if scoped.is_empty() { return Plan::Os(path) }
    scoped_plan(st, sel, scoped, path)
}
fn scoped_plan(st: &State, sel: &str, kind: &'static str, path: String) -> Plan {
    if root_of(st, sel).is_empty() { return Plan::Note("Open a thread first: that file is in no project here.") }
    Plan::Scoped { thread: sel.to_string(), kind, path }
}

/// A design file named alone (`main.kicad_sch`, no folder) that is not at the root: the first of that name below it (breadth first, four levels,
/// hidden folders and build trees skipped), as an absolute path. Agents name a board by its file name; it lives in `boards/<name>/`.
pub fn find_named(root: &str, name: &str) -> Option<String> {
    if root.is_empty() || name.is_empty() || name.contains('/') || design_kind(name).is_empty() { return None }
    let mut level = vec![std::path::PathBuf::from(root)];
    for _ in 0..5 {
        let mut next = vec![];
        for d in level {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            let mut es: Vec<_> = rd.flatten().collect(); es.sort_by_key(|e| e.file_name());
            for e in es {
                let n = e.file_name().to_string_lossy().to_string();
                let Ok(ft) = e.file_type() else { continue };
                if ft.is_file() && n == name { return Some(e.path().to_string_lossy().into_owned()) }
                if ft.is_dir() && !n.starts_with('.') && !matches!(n.as_str(), "target" | "node_modules" | "build" | "backup" | "backups") { next.push(e.path()); }
            }
            if next.len() > 400 { next.truncate(400); }
        }
        level = next;
    }
    None
}

/// A `backplane://file/<project or thread id>/<path…>` link as a path: from that project's (or thread's) folder.
pub fn file_ref(st: &State, rest: &[String]) -> Option<String> {
    let (scope, path) = rest.split_first()?;
    let root = st.projects.get(scope).filter(|p| !p.removed).map(|p| p.root.clone()).or_else(|| Some(root_of(st, scope)).filter(|r| !r.is_empty()))?;
    Some(format!("{}/{}", root.trim_end_matches('/'), path.join("/")))
}

// ---- the files a person opened by a link, outside every project: the watch takes these (kicad/mod.rs `known_root`) ----
static OPENED: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// The watch may draw this file (the person clicked it); the newest 64 are kept.
pub fn allow(path: &str) { if let Ok(mut v) = OPENED.lock() { v.retain(|p| p != path); v.push(path.to_string()); let n = v.len(); if n > 64 { v.drain(..n - 64); } } }
pub fn allowed(path: &str) -> bool { OPENED.lock().map_or(false, |v| v.iter().any(|p| p == path)) }

/// The session after a scoped open: `thread`'s viewer on `kind` with `path` its source, and the watch for it (Mech: the part is the path).
pub fn scoped_view(st: &State, hub: &str, session: &Value, kind: &str, path: &str) -> Option<super::viewer::ViewOut> {
    allow(path);
    if kind == "mech" { return super::viewer::view_action(st, hub, session, &format!("mech\t{path}")) }
    let sel = session["sel"].as_str().unwrap_or("");
    let root = root_of(st, sel); if root.is_empty() { return None }
    let n = if kind == "board" { "0" } else { "1" };
    let want = format!("{n}|{root}|{path}");
    let mut rid = session["rid"].as_u64().unwrap_or(0);
    let mut drafts = serde_json::Map::new();
    drafts.insert(format!("@view:{sel}"), json!(kind)); drafts.insert(format!("@view.file:{sel}"), json!(path)); drafts.insert(format!("@view.sheet:{sel}"), json!(""));
    for k in ["@pick", "@card", "@pick.rid"] { drafts.insert(k.into(), json!("")); }
    let mut before = vec![];
    if want != session["drafts"]["@view.at"].as_str().unwrap_or("") {
        let p = json!({"kind": kind, "root": root, "path": path, "tree": ""});
        before.push(json!({"type": "send", "hub": hub, "text": json!({"id": rid, "m": "kicad.watch", "p": p}).to_string()}));
        rid += 1; drafts.insert("@view.at".into(), json!(want));
    }
    Some(super::viewer::ViewOut { patch: json!({"rid": rid, "drafts": drafts}), before, boards_look: false })
}

#[cfg(test)]
#[path = "filelink_tests.rs"]
mod tests;

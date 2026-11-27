//! `link.resolve {url, thread?, project?}` (feature "links"): what a link or path in a message opens, decided here so clients hold no rule.
//! → `{kind: "thread", thread}` | `{kind: "project", project}` | `{kind: "room", room}` | `{kind: "share", route, name, size}`
//!   | `{kind: "file", thread, project?, path, absPath?, line}` | `{kind: "download", path, name, size, project?}` | `{kind: "none", note}`.
//! `url` may be a `backplane://` link (including `backplane://file/<scope>/<path>`), a share route `/s/<48-hex>/<name>`, a bare id (`t...`, `lab~t9`),
//! a `file://` URL, or a relative/absolute path (`src/x.rs:12`, `src/x.rs#L12`). Cross-project paths resolve against the owning project (preferring `project` hint,
//! active thread/worktree, or falling back to `project-home-<pid>`). Anything outside allowed roots or secrets returns `none` with a note.
use super::*;

fn none(note: &str) -> Result<Value, Value> { Ok(json!({"kind":"none","note":note})) }

/// `src/x.rs:12:3`, `src/x.rs#L12`, `src/x.rs#L12-L20` → (path, line). A colon followed by anything but digits belongs to the name.
pub(crate) fn split_line(t: &str) -> (&str, u64) {
    if let Some((p, f)) = t.rsplit_once("#L") { let n: String = f.chars().take_while(|c| c.is_ascii_digit()).collect(); if let Ok(l) = n.parse() { if !p.is_empty() { return (p, l) } } }
    let mut p = t; let mut line = 0;
    for _ in 0..2 {
        match p.rsplit_once(':') { Some((a, b)) if !a.is_empty() && !b.is_empty() && b.chars().all(|c| c.is_ascii_digit()) => { line = b.parse().unwrap_or(0); p = a } _ => break }
    }
    (p, line)
}

/// The path from the project's root, if `path` names a file inside `root` (relative, or absolute under it); never through `..`.
pub(crate) fn inside(root: &str, path: &str) -> Option<String> {
    if root.is_empty() || path.is_empty() || path.contains(['\0', '\n']) { return None }
    let root_c = std::fs::canonicalize(root).ok()?;
    let full = if crate::plat::fs::is_abs(path) { std::path::PathBuf::from(path) } else {
        if path.split('/').any(|x| x == "..") { return None }
        root_c.join(path)
    };
    let c = std::fs::canonicalize(&full).ok()?;
    if !c.is_file() { return None }
    let rel = c.strip_prefix(&root_c).ok()?;
    Some(rel.components().map(|x| x.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"))
}

pub(super) async fn resolve(hub: &Arc<Hub>, p: &Value) -> Result<Value, Value> {
    let url = s(p, "url").trim(); let th = s(p, "thread"); let target_proj = s(p, "project").trim();
    if url.is_empty() || url.len() > 4096 { return none("That link is empty.") }

    // 1. Share links (`/s/<48-hex-token>/<name>`) resolve against the share store rather than the thread's project folder.
    if let Some((route, token)) = crate::files::share::share_route_parts(url) {
        let rate_key = format!("link.resolve:{th}");
        if crate::files::share::share_rate_limited(&rate_key) {
            return none("Too many unknown links; wait a minute.");
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let found = crate::files::share::load(&hub.cfg.home).into_iter().find(|s| {
            s.expires > now
                && s.token.len() == token.len()
                && s.token.bytes().zip(token.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
        });
        let Some(sh) = found else {
            crate::files::share::note_share_miss(&rate_key);
            return none("This link has ended or never was.");
        };
        let extra = hub.setting("files.share.roots").await.unwrap_or_default();
        let home = hub.cfg.home.clone();
        let roots = { let g = hub.model().await; crate::files::share::roots_of(&g, &home, &extra) };
        let share_path = sh.path.clone();
        let ok = tokio::task::spawn_blocking(move || {
            crate::files::share::check(&roots, &home, &share_path)
                .ok()
                .filter(|(real, _)| real == std::path::Path::new(&share_path))
                .map(|(_, md)| md.len())
        }).await.ok().flatten();
        return match ok {
            Some(size) => Ok(json!({"kind": "share", "route": route, "name": sh.name, "size": size})),
            None => {
                crate::files::share::note_share_miss(&rate_key);
                none("This link has ended or never was.")
            }
        };
    }

    // 2. `backplane://` references (including `backplane://file/<project-or-thread>/<path...>`).
    let (bp_clean, bp_line) = if url.starts_with("backplane://file/") {
        let (base_u, l) = split_line(url);
        (base_u.split('#').next().unwrap_or(base_u), l)
    } else {
        (url, 0)
    };
    let parsed = if bp_clean.starts_with("backplane://") { crate::md::refs::parse(bp_clean) } else { crate::md::refs::bare(bp_clean) };
    let mut bp_file: Option<(String, String, u64)> = None;
    if let Some(r) = parsed {
        if r.kind == "file" {
            let Some((scope, rest_parts)) = r.rest.split_first() else { return none("That file is not on this hub.") };
            if rest_parts.is_empty() || rest_parts.iter().any(|seg| seg.is_empty() || seg == "." || seg == ".." || seg.contains(['/', '\\', '\0'])) {
                return none("That file is not in this project.");
            }
            let g = hub.model().await;
            let resolved = if let Some(proj) = g.projects.get(scope).filter(|p| !p.removed && !scope.contains('~') && !p.root.is_empty()) {
                Some((scope.clone(), proj.root.clone()))
            } else if let Some(root) = crate::files::threadpath::resolve(
                scope,
                |i| g.threads.get(i).map(|t| crate::files::threadpath::T { project: &t.project, worktree: &t.worktree, deleted: t.deleted }),
                |i| g.projects.get(i).map(|p| crate::files::threadpath::P { root: &p.root, removed: p.removed }),
            ) {
                let pid = g.threads.get(scope).map(|t| t.project.clone()).unwrap_or_default();
                Some((pid, root))
            } else {
                None
            };
            let Some((pid, root)) = resolved else { return none("That file is not on this hub.") };
            let joined = format!("{}/{}", root.trim_end_matches('/'), rest_parts.join("/"));
            let (clean_joined, tail_line) = split_line(&joined);
            bp_file = Some((pid, clean_joined.to_string(), if bp_line > 0 { bp_line } else { tail_line }));
        } else {
            let id = r.rest.first().cloned().unwrap_or_default();
            let g = hub.model().await;
            return match r.kind.as_str() {
                "thread" => {
                    // (a linked machine's thread by its link id or that machine's name; this machine's own name is this machine: mirror::thread_ref)
                    let m = hub.mirrors(); let here = m.here();
                    let named = m.shown(|far| crate::mirror::thread_ref(&id, &here, &|t| g.threads.get(t).map_or(false, |x| !x.deleted), far));
                    if let Some(t) = named { Ok(json!({"kind":"thread","thread":t,"msg":r.frag})) }
                    else if id.contains('~') && hub.far().map_or(false, |f| f.has(&id)) { Ok(json!({"kind":"thread","thread":id})) }
                    else if id.contains('~') { none("That thread is on a machine this hub does not mirror (yet).") } else { none("That thread is not on this hub.") }
                }
                "project" => if g.projects.get(&id).map_or(false, |p| !p.removed) { Ok(json!({"kind":"project","project":id})) } else if id.contains('~') { none("That project is on another machine.") } else { none("That project is not on this hub.") },
                "room" | "conv" | "bot" => {
                    drop(g);
                    let dir = crate::bots::api::directory(hub).await;
                    // a conversation id: `room:<id>`, `bot:<id>`, `pair:bot:<id>|person` (the bot's own chat)
                    let (kind, bid) = if r.kind == "conv" { match id.split_once(':') { Some(("pair", rest)) => ("bot", rest.strip_prefix("bot:").unwrap_or(rest).split('|').next().unwrap_or("").to_string()), Some((k, rest)) => (k, rest.to_string()), None => ("", String::new()) } } else { (r.kind.as_str(), id.clone()) };
                    let bid = bid.strip_prefix("bot:").unwrap_or(&bid).to_string();
                    if kind == "room" { if dir["rooms"].as_array().map_or(false, |a| a.iter().any(|x| x["id"] == bid.as_str())) { return Ok(json!({"kind":"room","room":bid})) } return none("That room is not on this hub.") }
                    if kind == "bot" { if let Some(b) = dir["bots"].as_array().and_then(|a| a.iter().find(|x| x["id"] == bid.as_str())) { return Ok(json!({"kind":"thread","thread":b["thread"]})) } return none("That conversation is not on this hub.") }
                    none("That conversation has no screen on the phone.")
                }
                "settings" => Ok(json!({"kind":"settings","page":id})),
                _ => none("The phone has no screen for that link."),
            };
        }
    }
    if bp_file.is_none() && url.starts_with("backplane://") { return none("That link is not one this hub knows.") }

    // 3. A path in a project: against `project`, `thread`, or any registered project / share root allowed by files::share::check.
    let (hint_proj, abs_input, line) = if let Some((pid, path, l)) = bp_file {
        (pid, path, l)
    } else {
        let normalized = if let Some(rest) = url.get(..7).filter(|s| s.eq_ignore_ascii_case("file://")).map(|_| &url[7..]) {
            let (before_frag, frag) = match rest.split_once('#') {
                Some((a, f)) if f.starts_with('L') && f[1..].chars().next().is_some_and(|c| c.is_ascii_digit()) => (a, Some(f)),
                Some((a, _)) => (a, None),
                None => (rest, None),
            };
            let before_query = before_frag.split('?').next().unwrap_or(before_frag);
            let decoded = crate::present::filelink::url_path(before_query);
            match frag { Some(f) => format!("{decoded}#{f}"), None => decoded }
        } else { url.to_string() };
        let (path, line) = split_line(&normalized);
        (target_proj.to_string(), path.to_string(), line)
    };
    let extra = hub.setting("files.share.roots").await.unwrap_or_default();
    let home = hub.cfg.home.clone();
    let home_dir = crate::plat::dirs::home_or_empty();
    let (roots, projects, abs_path, cands) = {
        let g = hub.model().await;
        use crate::present::filelink as fl;
        let roots = crate::files::share::roots_of(&g, &home, &extra);
        let projects = crate::files::share::projects_of(&g);
        let th_root = crate::files::threadpath::resolve(
            th,
            |i| g.threads.get(i).map(|t| crate::files::threadpath::T { project: &t.project, worktree: &t.worktree, deleted: t.deleted }),
            |i| g.projects.get(i).map(|p| crate::files::threadpath::P { root: &p.root, removed: p.removed }),
        ).unwrap_or_default();
        let th_proj_root = g.threads.get(th).filter(|t| !t.deleted).and_then(|t| g.projects.get(&t.project)).filter(|p| !p.removed).map(|p| p.root.clone()).unwrap_or_default();
        let hint_root = if !hint_proj.is_empty() && !hint_proj.contains('~') {
            g.projects.get(&hint_proj).filter(|p| !p.removed && !p.root.is_empty()).map(|p| p.root.clone()).unwrap_or_default()
        } else { String::new() };
        let base = if !hint_root.is_empty() { hint_root } else if !th_root.is_empty() { th_root.clone() } else { th_proj_root };
        let is_rel = !crate::plat::fs::is_abs(&abs_input) && !abs_input.starts_with("~/");
        if is_rel && base.is_empty() { return none("There is no project to look for that file in.") }
        if is_rel && abs_input.split(['/', '\\']).any(|x| x == "..") { return none("That file is not in this project.") }
        let Some(abs_path) = fl::absolute(&abs_input, &base, &home_dir) else { return none("That file is not in this project.") };
        let mut cands: Vec<(String, String, String)> = vec![];
        let push_proj_threads = |cands: &mut Vec<(String, String, String)>, pid: &str, p: &crate::model::ProjectMeta| {
            let mut ts: Vec<(&String, &crate::model::ThreadMeta)> = g.threads.iter().filter(|(_, t)| !t.deleted && !t.archived && t.project == pid).collect();
            ts.sort_by(|a, b| {
                let ra = if !a.1.worktree.is_empty() { &a.1.worktree } else { &p.root };
                let rb = if !b.1.worktree.is_empty() { &b.1.worktree } else { &p.root };
                std::path::Path::new(rb).components().count().cmp(&std::path::Path::new(ra).components().count())
                    .then_with(|| b.1.activity.cmp(&a.1.activity))
                    .then_with(|| b.0.cmp(a.0))
            });
            for (tid, t) in ts {
                if cands.iter().any(|(x, _, _)| x == tid) { continue }
                let root = if !t.worktree.is_empty() { t.worktree.clone() } else { p.root.clone() };
                cands.push((tid.clone(), pid.to_string(), root));
            }
        };
        if !hint_proj.is_empty() && !hint_proj.contains('~') {
            if let Some(p) = g.projects.get(&hint_proj).filter(|p| !p.removed && !p.root.is_empty()) {
                push_proj_threads(&mut cands, &hint_proj, p);
            }
        }
        if !th.is_empty() && !th_root.is_empty() {
            if let Some(t) = g.threads.get(th).filter(|t| !t.deleted) {
                if !cands.iter().any(|(x, _, _)| x == th) {
                    cands.push((th.to_string(), t.project.clone(), th_root));
                }
            }
        }
        let mut projs: Vec<(&String, &crate::model::ProjectMeta)> = g.projects.iter().filter(|(id, p)| !p.removed && !id.contains('~') && !p.root.is_empty()).collect();
        projs.sort_by(|a, b| b.1.root.trim_end_matches('/').len().cmp(&a.1.root.trim_end_matches('/').len()));
        for (pid, p) in projs {
            push_proj_threads(&mut cands, pid, p);
        }
        (roots, projects, abs_path, cands)
    };
    let res = tokio::task::spawn_blocking(move || -> Result<Value, &'static str> {
        let (real, md) = crate::files::share::check(&roots, &home, &abs_path).map_err(|e| match e {
            "that file is not shared" => "That file is not shared.",
            "that file is not in a project" => "That file is not in a project.",
            "no such file" => "That file was not found.",
            "not a file" => "That path is not a file.",
            _ => "That file is not in this project.",
        })?;
        let real_str = real.to_string_lossy().into_owned();
        let owner = crate::files::share::owner_of(&projects, &real);
        let owner_pid = owner.as_ref().map(|(pid, _)| pid.as_str()).unwrap_or("");
        for (cand_th, cand_pid, cand_root) in &cands {
            if !owner_pid.is_empty() && cand_pid != owner_pid { continue }
            if let Some(rel) = inside(cand_root, &real_str) {
                let proj = if !owner_pid.is_empty() { owner_pid } else { cand_pid.as_str() };
                return Ok(json!({"kind": "file", "thread": cand_th, "project": proj, "path": rel, "absPath": real_str, "line": line}));
            }
        }
        if let Some((pid, rel)) = owner {
            return Ok(json!({"kind": "file", "thread": format!("project-home-{pid}"), "project": pid, "path": rel, "absPath": real_str, "line": line}));
        }
        let name = real.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
        Ok(json!({"kind": "download", "path": real_str, "name": name, "size": md.len()}))
    }).await;
    match res {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(note)) => none(note),
        Err(_) => none("That file is not in this project."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn lines_split_off() {
        assert_eq!(split_line("src/x.rs:12"), ("src/x.rs", 12));
        assert_eq!(split_line("src/x.rs:12:3"), ("src/x.rs", 12));
        assert_eq!(split_line("src/x.rs#L40-L50"), ("src/x.rs", 40));
        assert_eq!(split_line("src/x.rs"), ("src/x.rs", 0));
        assert_eq!(split_line("a:b"), ("a:b", 0));
        assert_eq!(split_line(":12"), (":12", 0));
    }
    #[test] fn inside_only_files_of_the_project() {
        let base = crate::plat::tmp::dir("link");
        let d = base.join("proj"); let out = base.join("out");
        std::fs::create_dir_all(d.join("src")).unwrap(); std::fs::write(d.join("src/x.rs"), "a\nb\n").unwrap(); std::fs::write(&out, "x").unwrap();
        let root = d.to_string_lossy().to_string();
        assert_eq!(inside(&root, "src/x.rs").as_deref(), Some("src/x.rs"));
        assert_eq!(inside(&root, &format!("{root}/src/x.rs")).as_deref(), Some("src/x.rs"));
        assert_eq!(inside(&root, "src"), None, "a folder is no file");
        assert_eq!(inside(&root, "src/none.rs"), None);
        assert_eq!(inside(&root, "../x"), None);
        assert_eq!(inside(&root, &out.to_string_lossy()), None, "an absolute path outside");
        assert_eq!(inside("", "src/x.rs"), None);
    }
}

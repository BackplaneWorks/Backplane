//! `link.resolve {url, thread?}` (feature "links"): what a link or path in a message opens on a phone, decided here so the apps hold no rule.
//! → `{kind: "thread", thread}` | `{kind: "project", project}` | `{kind: "room", room}` | `{kind: "file", thread, path, line}` | `{kind: "none", note}`.
//! `url` is a `backplane://` link (or a bare id as a message writes it: `t1700000000-1`, `lab~t9`), or a path: `src/x.rs:12`, `src/x.rs#L12`, an absolute path under
//! the thread's project. A thread of a linked machine (`<link>~<id>`) opens when this hub mirrors it; a file path resolves against `thread`'s project folder and only
//! when it is a file inside it. Anything else is a `none` with a short note the app shows.
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
    let url = s(p, "url").trim(); let th = s(p, "thread");
    if url.is_empty() || url.len() > 4096 { return none("That link is empty.") }
    let parsed = if url.starts_with("backplane://") { crate::md::refs::parse(url) } else { crate::md::refs::bare(url) };
    if let Some(r) = parsed {
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
    if url.starts_with("backplane://") { return none("That link is not one this hub knows.") }
    // a path in a project: against the thread's folder, or any registered project / share root allowed by files::share::check
    // (a file:// address or absolute path in another project, e.g. from a bot conversation, opens in that project's thread or downloads via /file)
    let normalized = if let Some(rest) = url.get(..7).filter(|s| s.eq_ignore_ascii_case("file://")).map(|_| &url[7..]) {
        let (before_frag, frag) = match rest.split_once('#') {
            Some((a, f)) if f.starts_with('L') && f[1..].chars().next().is_some_and(|c| c.is_ascii_digit()) => (a, Some(f)),
            Some((a, _)) => (a, None),
            None => (rest, None),
        };
        let decoded = crate::present::filelink::url_path(before_frag);
        match frag { Some(f) => format!("{decoded}#{f}"), None => decoded }
    } else { url.to_string() };
    let (path, line) = split_line(&normalized);
    let extra = hub.setting("files.share.roots").await.unwrap_or_default();
    let home = hub.cfg.home.clone();
    let home_dir = crate::plat::dirs::home_or_empty();
    let (roots, abs_path, cands) = {
        let g = hub.model().await;
        use crate::present::filelink as fl;
        let roots = crate::files::share::roots_of(&g, &home, &extra);
        let th_root = crate::files::threadpath::resolve(
            th,
            |i| g.threads.get(i).map(|t| crate::files::threadpath::T { project: &t.project, worktree: &t.worktree, deleted: t.deleted }),
            |i| g.projects.get(i).map(|p| crate::files::threadpath::P { root: &p.root, removed: p.removed }),
        ).unwrap_or_default();
        let th_proj_root = g.threads.get(th).filter(|t| !t.deleted).and_then(|t| g.projects.get(&t.project)).filter(|p| !p.removed).map(|p| p.root.clone()).unwrap_or_default();
        let base = if !th_root.is_empty() { th_root.clone() } else { th_proj_root };
        let is_rel = !crate::plat::fs::is_abs(path) && !path.starts_with("~/");
        if is_rel && base.is_empty() { return none("There is no project to look for that file in.") }
        if is_rel && path.split(['/', '\\']).any(|x| x == "..") { return none("That file is not in this project.") }
        let Some(abs_path) = fl::absolute(path, &base, &home_dir) else { return none("That file is not in this project.") };
        let mut cands: Vec<(String, String)> = vec![];
        if !th.is_empty() && !th_root.is_empty() && g.threads.get(th).is_some_and(|t| !t.deleted) {
            cands.push((th.to_string(), th_root));
        }
        let mut projs: Vec<(&String, &crate::model::ProjectMeta)> = g.projects.iter().filter(|(id, p)| !p.removed && !id.contains('~') && !p.root.is_empty()).collect();
        projs.sort_by(|a, b| b.1.root.trim_end_matches('/').len().cmp(&a.1.root.trim_end_matches('/').len()));
        for (pid, p) in projs {
            let mut ts: Vec<(&String, &crate::model::ThreadMeta)> = g.threads.iter().filter(|(_, t)| !t.deleted && !t.archived && &t.project == pid).collect();
            ts.sort_by(|a, b| {
                let ra = if !a.1.worktree.is_empty() { &a.1.worktree } else { &p.root };
                let rb = if !b.1.worktree.is_empty() { &b.1.worktree } else { &p.root };
                std::path::Path::new(rb).components().count().cmp(&std::path::Path::new(ra).components().count())
                    .then_with(|| b.1.activity.cmp(&a.1.activity))
                    .then_with(|| b.0.cmp(a.0))
            });
            for (tid, t) in ts {
                if cands.iter().any(|(x, _)| x == tid) { continue }
                let root = if !t.worktree.is_empty() { t.worktree.clone() } else { p.root.clone() };
                cands.push((tid.clone(), root));
            }
        }
        (roots, abs_path, cands)
    };
    let res = tokio::task::spawn_blocking(move || -> Result<Value, &'static str> {
        let (real, md) = crate::files::share::check(&roots, &home, &abs_path).map_err(|e| match e {
            "that file is not shared" => "That file is not shared.",
            "that file is not in a project" => "That file is not in a project.",
            _ => "That file is not in this project.",
        })?;
        let real_str = real.to_string_lossy().into_owned();
        for (cand_th, cand_root) in &cands {
            if let Some(rel) = inside(cand_root, &real_str) {
                return Ok(json!({"kind": "file", "thread": cand_th, "path": rel, "line": line}));
            }
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

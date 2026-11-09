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
            _ => none("The phone has no screen for that link."),
        };
    }
    if url.starts_with("backplane://") { return none("That link is not one this hub knows.") }
    // a path in a project: against the thread's folder
    let (path, line) = split_line(url);
    if th.is_empty() { return none("There is no project to look for that file in.") }
    let root = { let g = hub.model().await; g.threads.get(th).filter(|t| !t.deleted).and_then(|t| g.projects.get(&t.project)).map(|p| p.root.clone()) };
    let Some(root) = root else { return none("That file is on another machine.") };
    let r2 = root.clone(); let pp = path.to_string();
    match tokio::task::spawn_blocking(move || inside(&r2, &pp)).await.ok().flatten() {
        Some(rel) => Ok(json!({"kind":"file","thread":th,"path":rel,"line":line})),
        None => none("That file is not in this project."),
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
        let d = std::env::temp_dir().join(format!("bp-link-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("src")).unwrap(); std::fs::write(d.join("src/x.rs"), "a\nb\n").unwrap(); std::fs::write(d.parent().unwrap().join(format!("bp-link-out-{}", std::process::id())), "x").unwrap();
        let root = d.to_string_lossy().to_string();
        assert_eq!(inside(&root, "src/x.rs").as_deref(), Some("src/x.rs"));
        assert_eq!(inside(&root, &format!("{root}/src/x.rs")).as_deref(), Some("src/x.rs"));
        assert_eq!(inside(&root, "src"), None, "a folder is no file");
        assert_eq!(inside(&root, "src/none.rs"), None);
        assert_eq!(inside(&root, "../x"), None);
        assert_eq!(inside(&root, &format!("{}/bp-link-out-{}", d.parent().unwrap().to_string_lossy(), std::process::id())), None, "an absolute path outside");
        assert_eq!(inside("", "src/x.rs"), None);
        let _ = std::fs::remove_dir_all(&d); let _ = std::fs::remove_file(d.parent().unwrap().join(format!("bp-link-out-{}", std::process::id())));
    }
}

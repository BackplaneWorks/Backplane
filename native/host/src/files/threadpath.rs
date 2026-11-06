//! The thread-folder boundary (docs/thread-folder-boundary.md on main, PR #960; core/threadpath, core/folder and
//! hub's `Hub.folder.job`). One resolver for every thread- or project-scoped route: it answers a folder or None, and a route
//! with None schedules no IO at all (law thread_folder_refuses_job). There is no fallback to the hub's startup folder.
//!   * empty or unknown id, a deleted thread, a removed project, a root or worktree that is not an absolute folder: None
//!   * an archived thread keeps its folder; `project-home-<project>` names a project's own folder
//!   * a thread with no project (private coordination threads) carries its own absolute folder
use serde_json::{json, Value};

pub const NO_FOLDER: &str = "No folder for this thread or project: unknown, deleted, removed or unavailable";

/// What the resolver needs of a thread and a project (the read model's ThreadMeta / ProjectMeta).
pub struct T<'a> { pub project: &'a str, pub worktree: &'a str, pub deleted: bool }
pub struct P<'a> { pub root: &'a str, pub removed: bool }

/// Folder.absolute: native absolute folders only (no NUL; "/", "\\\\" or "C:/", "C:\\").
pub fn absolute(p: &str) -> bool {
    if p.contains('\0') { return false }
    let b = p.as_bytes();
    p.starts_with('/') || p.starts_with("\\\\") || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'/' || b[2] == b'\\'))
}
fn folder(p: &str) -> Option<String> { if absolute(p) { Some(p.to_string()) } else { None } }

/// ThreadPath.of: a project home's folder or a thread's.
pub fn resolve<'a>(id: &str, thread: impl Fn(&str) -> Option<T<'a>>, project: impl Fn(&str) -> Option<P<'a>>) -> Option<String> {
    let proj = |pid: &str| project(pid).filter(|p| !p.removed).and_then(|p| folder(p.root));
    if let Some(pid) = id.strip_prefix("project-home-") { return proj(pid) }
    if id.is_empty() { return None }
    let t = thread(id)?;
    if t.deleted { return None }
    if t.project.is_empty() { return folder(t.worktree) }
    let root = proj(t.project)?;
    if t.worktree.is_empty() { Some(root) } else { folder(t.worktree) }
}

/// The refusal every folder route answers (Reply.err).
pub fn refusal(id: &Value) -> Value { json!({"t":"reply","id":id,"ok":false,"error":NO_FOLDER}) }

/// Hub.folder.job: the job runs only with a resolved folder; with None it is not scheduled and the reply is the refusal.
pub fn job<R>(at: Option<String>, id: &Value, run: impl FnOnce(String) -> R) -> Result<R, Value> { match at { Some(root) => Ok(run(root)), None => Err(refusal(id)) } }

/// Hub.folder.path: a path given to a folder job stays beneath the resolved root: relative and safe, or absolute under the root.
pub fn path_in(root: &str, path: &str) -> bool {
    (!path.is_empty() && super::fm::files_safe(path)) || (path.starts_with(&format!("{root}/")) && super::fm::files_safe(&path[root.len() + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    struct World { threads: Vec<(&'static str, &'static str, &'static str, bool)>, projects: Vec<(&'static str, &'static str, bool)> }
    fn at(w: &World, id: &str) -> Option<String> {
        resolve(id, |i| w.threads.iter().find(|t| t.0 == i).map(|t| T { project: t.1, worktree: t.2, deleted: t.3 }), |i| w.projects.iter().find(|p| p.0 == i).map(|p| P { root: p.1, removed: p.2 }))
    }
    fn world() -> World { World {
        threads: vec![("t1", "p1", "", false), ("wt", "p1", "/p/.backplane/worktrees/w", false), ("gone", "p1", "", true), ("rm", "p2", "", false), ("badwt", "p1", "relative/wt", false),
            ("priv", "", "/priv", false), ("privrel", "", "rel", false), ("privempty", "", "", false), ("badroot", "p3", "", false), ("nul", "p4", "", false)],
        projects: vec![("p1", "/p", false), ("p2", "/q", true), ("p3", "", false), ("p4", "/a\0b", false), ("p5", "/five", false)] } }
    // law thread_folder_refuses_job: when the folder does not resolve, no job is scheduled whatever the job, thread or arguments
    #[test] fn thread_folder_refuses_job() {
        let w = world();
        let unresolved = ["", "unknown", "t1x", " ", "T1", "nope/../t1", "p1", "p5", "project-home-", "project-home-nope", "project-home-p2", "project-home-p3", "project-home-p4", "project-home-p1/../..", "gone", "rm", "badwt", "privrel", "privempty", "badroot", "nul", "/", "..", "\0", "t1\0", "\u{1F600}"];
        for id in unresolved {
            assert_eq!(at(&w, id), None, "{id:?}");
            for jobname in ["dir", "files", "mech", "render", "refs", "kicad", "boards", "kexport", "status", "diff", "revert", "act", "checkpoint", "step", "resolve", ""] {
                let ran = Cell::new(0);
                let reply_id = json!(7);
                let r = job(at(&w, id), &reply_id, |_root| { ran.set(ran.get() + 1); jobname });
                assert_eq!(ran.get(), 0, "job {jobname:?} ran for {id:?}");
                assert_eq!(r.unwrap_err(), json!({"t":"reply","id":7,"ok":false,"error":"No folder for this thread or project: unknown, deleted, removed or unavailable"}));
            }
        }
        // what does resolve: a thread of a live project, its worktree, an archived thread (not a deleted one), a private thread's own folder, a project home
        for (id, root) in [("t1", "/p"), ("wt", "/p/.backplane/worktrees/w"), ("priv", "/priv"), ("project-home-p1", "/p"), ("project-home-p5", "/five")] { assert_eq!(at(&w, id).as_deref(), Some(root), "{id}") }
        let ran = Cell::new(false); assert_eq!(job(at(&w, "t1"), &json!(1), |r| { ran.set(true); r }).unwrap(), "/p"); assert!(ran.get());
    }
    #[test] fn absolute_folders_only() {
        for p in ["/", "/a", "\\\\host\\share", "C:/x", "c:\\x"] { assert!(absolute(p), "{p}") }
        for p in ["", "a", "./a", "~/a", "C:", "C:x", "1:/x", "/a\0b", "\0"] { assert!(!absolute(p), "{p:?}") }
    }
    #[test] fn paths_stay_beneath_the_root() {
        assert!(path_in("/p", "sub/a.kicad_sch")); assert!(path_in("/p", "/p/sub/a")); assert!(path_in("/p", "/p/a"));
        for p in ["", "..", "../x", "/etc/passwd", "/pp/a", "/p/../etc", "a\\b", "C:x", "/p/a\\b"] { assert!(!path_in("/p", p), "{p:?}") }
    }
}

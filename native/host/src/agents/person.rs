//! Side-by-side mode (docs/SIDE-BY-SIDE.md): the checks a person's `turn.start` passes before a REAL agent (`proc::Kind::PersonTurn`) starts on a copy of the real data,
//! and the environment that child gets. Everything else on a copy stays `Kind::Agent`, stand-in only.
//!   1 trigger (agents/mod.rs: the client's turn.start only; delivery to a live process is a door too), 2 folder (`check_folder`), 3 fresh session (no `--resume`),
//!   4 permissions (`runtime_for`), 5 the live log's tail (`live_conflict`), 6 environment (`child_env`).
use serde_json::Value;
use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}};

/// The real HOME the person's agent uses, set by the script (`BACKPLANE_PERSON_HOME`); the hub itself runs with a fake one.
pub fn person_home() -> Option<PathBuf> { std::env::var_os("BACKPLANE_PERSON_HOME").filter(|v| !v.is_empty()).map(PathBuf::from) }
/// The live hub's log (`BACKPLANE_LIVE_LOG`), read-only, for the same-folder check.
pub fn live_log() -> Option<PathBuf> { std::env::var_os("BACKPLANE_LIVE_LOG").filter(|v| !v.is_empty()).map(PathBuf::from) }

/// Check 4: a copied thread's runtime never reaches `bypassPermissions` / `--dangerously-bypass-approvals-and-sandbox` here: empty and full-access are mapped down to approval-required.
pub fn runtime_for(_copied: &str) -> &'static str { "approval-required" } // whatever the copied thread says: an allowlist of one (the person raising it by hand is not built)

fn under(p: &Path, root: &Path) -> bool { p == root || p.starts_with(root) }

/// Check 2: the thread's folder exists, resolves (symlinks followed) under a project root named by the copy or under the copy's own scratch, is not `/`, and is not under the live
/// home (`~/.backplane-bend`: scratch and bot threads live there) nor the copy's own home itself or its control files.
pub fn check_folder(cwd: &str, roots: &[String], copy_home: &Path, live_home: Option<&Path>) -> Result<PathBuf, String> {
    if cwd.is_empty() { return Err("the thread has no folder".into()) }
    let real = crate::plat::fs::canonicalize(cwd).map_err(|_| format!("the thread's folder {cwd} is gone"))?;
    if !real.is_dir() { return Err(format!("{cwd} is not a folder")) }
    if real == Path::new("/") { return Err("the thread's folder is /".into()) }
    let canon = |p: &Path| crate::plat::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if let Some(l) = live_home { let l = canon(l); if under(&real, &l) { return Err(format!("{cwd} is inside the live Backplane's home ({})", l.display())) } }
    let ch = canon(copy_home);
    let scratch = ch.join("scratch");
    let in_root = roots.iter().filter(|r| !r.is_empty()).any(|r| under(&real, &canon(Path::new(r))));
    if real == ch || (under(&real, &ch) && !under(&real, &scratch)) { return Err(format!("{cwd} is the copy's own home or one of its control folders")) }
    if !(in_root || under(&real, &scratch)) { return Err(format!("{cwd} is not under a project folder of this copy")) }
    Ok(real)
}

/// What the check on the live log knows: per thread its folder and whether its turn is running (as of the copy point or later).
#[derive(Default, Debug)]
pub struct LiveFold { pub folder: HashMap<String, String>, pub running: HashSet<String>, pub roots: HashMap<String, String>, pub project: HashMap<String, String> }
impl LiveFold {
    /// Fold the live log's tail lines (whole lines only). `base`: threads the copy already knew: their folder and whether they were running at the copy point.
    pub fn fold(base: &[(String, String, bool)], lines: &[&str]) -> LiveFold {
        let mut f = LiveFold::default(); for (t, folder, running) in base { if !folder.is_empty() { f.folder.insert(t.clone(), folder.clone()); } if *running { f.running.insert(t.clone()); } }
        for l in lines {
            let Some(ty) = crate::session::event_type(l) else { continue };
            if !["ProjectCreated", "ThreadCreated", "WorktreeSet", "TurnChanged"].contains(&ty) { continue }
            let Ok(v) = serde_json::from_str::<Value>(l) else { continue }; let g = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            match ty {
                "ProjectCreated" => { f.roots.insert(g("id"), g("root")); }
                "ThreadCreated" => { f.project.insert(g("id"), g("project")); let wt = g("worktree"); let folder = if !wt.is_empty() { wt } else { f.roots.get(&g("project")).cloned().unwrap_or_default() }; if !folder.is_empty() { f.folder.insert(g("id"), folder); } }
                "WorktreeSet" => { f.folder.insert(g("id"), g("worktree")); }
                "TurnChanged" => { if g("state") == "running" { f.running.insert(g("thread")); } else { f.running.remove(&g("thread")); } }
                _ => {}
            }
        }
        f
    }
}
/// Check 5: a live turn running in the same folder, or one under or over it. `me`: the thread asking (its own live turn is not a conflict with itself in the copy).
pub fn live_conflict(f: &LiveFold, folder: &Path) -> Option<String> {
    let canon = |p: &str| crate::plat::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p));
    for t in &f.running {
        let Some(other) = f.folder.get(t) else { return Some(format!("a turn of the live Backplane is running in thread {t}, whose folder is not known here: refused rather than guessed")) }; let o = canon(other);
        if under(folder, &o) || under(&o, folder) { return Some(format!("a turn of the live Backplane is running in {} (thread {t}): don't run the same folder in both apps", o.display())) }
    }
    None
}
/// Read the tail of the live log (at most `cap` bytes from the end, whole lines only). Err: missing, unreadable, or a tail with no whole line: the check then REFUSES.
pub fn read_tail(path: &Path, cap: u64) -> Result<Vec<String>, String> { read_tail_whole(path, cap).map(|(l, _)| l) }
/// The tail and whether it is the whole file.
pub fn read_tail_whole(path: &Path, cap: u64) -> Result<(Vec<String>, bool), String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).map_err(|e| format!("the live log {} cannot be read ({e})", path.display()))?;
    let len = f.metadata().map_err(|e| format!("the live log cannot be read ({e})"))?.len();
    let start = len.saturating_sub(cap); f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut buf = Vec::new(); f.take(cap).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    let mut ls: Vec<String> = text.split('\n').map(str::to_string).collect();
    if !text.ends_with('\n') { ls.pop(); } // a torn last line is not a whole one
    if start > 0 { if !ls.is_empty() { ls.remove(0); } } // the first line of a mid-file read is cut
    if ls.iter().all(|l| l.trim().is_empty()) && len > 0 { return Err("the live log's tail holds no whole line".into()) }
    Ok((ls.into_iter().filter(|l| !l.trim().is_empty()).collect(), start == 0))
}
/// The live log's lines since the copy point: all of a small log; of a big one the lines after the copy's own last lines (its hub may have added some after the copy point), and an Err when the tail does not reach back to it (default-deny).
pub fn lines_since(path: &Path, cap: u64, copy_last: &[String]) -> Result<Vec<String>, String> {
    let (ls, whole) = read_tail_whole(path, cap)?; if whole { return Ok(ls) }
    match ls.iter().rposition(|l| copy_last.contains(l)) { Some(i) => Ok(ls[i + 1..].to_vec()), _ => Err("the live log has grown past what can be read back to the copy point: refused rather than guessed".into()) }
}

/// Check 6: the child's environment: the real HOME, the merged PATH, and none of this hub's `BACKPLANE_*`.
pub fn child_env(home: &Path, path: &std::ffi::OsStr, current: impl Iterator<Item = (std::ffi::OsString, std::ffi::OsString)>) -> (Vec<std::ffi::OsString>, Vec<(String, std::ffi::OsString)>) {
    let remove: Vec<std::ffi::OsString> = current.map(|(k, _)| k).filter(|k| k.to_string_lossy().starts_with("BACKPLANE_")).collect();
    (remove, vec![("HOME".into(), home.as_os_str().to_owned()), ("PATH".into(), path.to_owned())])
}
/// The directories a person's agent CLI is looked for in: the hub's PATH, the person's login shell's (run with the REAL home) and the usual install directories under the real home.
pub fn person_dirs(home: &Path) -> Vec<PathBuf> {
    let cur: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    super::proc::merge_dirs(&[cur, super::proc::login_dirs_with(Some(home)), super::proc::usual_dirs(home)])
}
/// Extra arguments a PersonTurn's Claude gets (Requirement in SIDE-BY-SIDE.md): only this hub's MCP config, and no user-level settings (hooks, MCP servers, permission rules); the
/// project's own `.claude` settings in the real folder do load.
pub const CLAUDE_STRICT: [&str; 3] = ["--strict-mcp-config", "--setting-sources", "project,local"];

#[cfg(test)]
mod tests {
    use super::*;
    fn dir(n: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-person-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); crate::plat::fs::canonicalize(&p).unwrap() }
    #[test] fn the_folder_must_be_a_project_folder_of_the_copy_and_nothing_of_the_live_home_or_root() {
        let (proj, home, live, other) = (dir("proj"), dir("home"), dir("live"), dir("other")); std::fs::create_dir_all(proj.join("sub")).unwrap(); std::fs::create_dir_all(home.join("scratch/t1")).unwrap(); std::fs::create_dir_all(live.join("scratch/t9")).unwrap();
        let roots = vec![proj.to_string_lossy().into_owned()];
        assert!(check_folder(proj.to_str().unwrap(), &roots, &home, Some(&live)).is_ok()); assert!(check_folder(proj.join("sub").to_str().unwrap(), &roots, &home, Some(&live)).is_ok());
        assert!(check_folder(home.join("scratch/t1").to_str().unwrap(), &roots, &home, Some(&live)).is_ok(), "the copy's own scratch");
        for (bad, why) in [("/", "root"), (live.join("scratch/t9").to_str().unwrap(), "under the live home"), (other.to_str().unwrap(), "not a project folder"), (home.to_str().unwrap(), "the copy's home"), (home.join("attachments").to_str().unwrap(), "control folder"), ("/no/such/folder", "missing"), ("", "empty")] {
            assert!(check_folder(bad, &roots, &home, Some(&live)).is_err(), "{why}: {bad}");
        }
        std::fs::create_dir_all(home.join("attachments")).unwrap(); assert!(check_folder(home.join("attachments").to_str().unwrap(), &roots, &home, Some(&live)).is_err());
        let link = proj.join("to-live"); crate::plat::fs::symlink(live.join("scratch/t9"), &link).unwrap(); assert!(check_folder(link.to_str().unwrap(), &roots, &home, Some(&live)).is_err(), "a symlink into the live home is followed and refused");
    }
    #[test] fn permissions_are_forced_down_from_full_access_and_empty() { assert_eq!(runtime_for("full-access"), "approval-required"); assert_eq!(runtime_for(""), "approval-required"); for r in ["auto-accept-edits", "auto", "garbage", "", "full-access", "bypassPermissions"] { assert_eq!(runtime_for(r), "approval-required", "{r}") } assert_eq!(runtime_for("approval-required"), "approval-required") }
    #[test] fn a_live_turn_in_the_same_folder_or_one_over_or_under_it_conflicts_and_a_finished_one_does_not() {
        let d = dir("live-fold"); std::fs::create_dir_all(d.join("a/b")).unwrap(); std::fs::create_dir_all(d.join("c")).unwrap();
        let lines = [format!(r#"{{"$":"ProjectCreated","id":"p9","title":"n","root":{},"at":1}}"#, serde_json::to_string(d.join("a").to_str().unwrap()).unwrap()),
            r#"{"$":"ThreadCreated","id":"t9","project":"p9","title":"x","provider":"claude","model":"","at":2}"#.to_string(), r#"{"$":"TurnChanged","thread":"t9","state":"running","at":3}"#.to_string()];
        let ls: Vec<&str> = lines.iter().map(String::as_str).collect(); let f = LiveFold::fold(&[], &ls);
        assert!(live_conflict(&f, &d.join("a")).is_some() && live_conflict(&f, &d.join("a/b")).is_some() && live_conflict(&f, &d).is_some(), "same, under and over");
        assert!(live_conflict(&f, &d.join("c")).is_none());
        let done = [ls[0], ls[1], ls[2], r#"{"$":"TurnChanged","thread":"t9","state":"completed","at":4}"#]; assert!(live_conflict(&LiveFold::fold(&[], &done), &d.join("a")).is_none(), "ended");
        let base = [("t1".to_string(), d.join("c").to_string_lossy().into_owned(), true)]; assert!(live_conflict(&LiveFold::fold(&base, &[]), &d.join("c")).is_some(), "running at the copy point and not seen to end");
        let ends = [r#"{"$":"TurnChanged","thread":"t1","state":"completed","at":9}"#]; assert!(live_conflict(&LiveFold::fold(&base, &ends), &d.join("c")).is_none());
        let wt = [r#"{"$":"ThreadCreated","id":"tw","project":"p0","title":"x","provider":"claude","model":"","worktree":"/w/x","at":2}"#, r#"{"$":"WorktreeSet","id":"tw","worktree":"/w/y","at":3}"#, r#"{"$":"TurnChanged","thread":"tw","state":"running","at":4}"#]; let f2 = LiveFold::fold(&[], &wt); assert_eq!(f2.folder["tw"], "/w/y");
    }
    #[test] fn the_live_tail_is_read_whole_lines_only_and_an_unreadable_log_refuses() {
        let d = dir("tail"); let p = d.join("events.jsonl"); std::fs::write(&p, "{\"$\":\"A\"}\n{\"$\":\"B\"}\n{\"$\":\"C\"").unwrap();
        assert_eq!(read_tail(&p, 1 << 20).unwrap(), vec!["{\"$\":\"A\"}", "{\"$\":\"B\"}"], "a torn last line is dropped");
        std::fs::write(&p, "x".repeat(100) + "\n{\"$\":\"B\"}\n").unwrap(); assert_eq!(read_tail(&p, 14).unwrap(), vec!["{\"$\":\"B\"}"], "a cut first line is dropped");
        assert!(read_tail(&d.join("missing"), 100).is_err()); std::fs::write(&p, "{\"$\":\"half").unwrap(); assert!(read_tail(&p, 100).is_err(), "a log with no whole line cannot be trusted");
    }
    #[test] fn a_running_turn_with_an_unknown_folder_refuses_and_a_big_log_must_reach_the_copy_point() {
        let d = dir("unknown"); std::fs::create_dir_all(d.join("a")).unwrap();
        let start = [r#"{"$":"TurnChanged","thread":"tx","state":"running","at":5}"#]; let f = LiveFold::fold(&[], &start);
        assert!(live_conflict(&f, &d.join("a")).unwrap().contains("not known"), "a thread started after the copy, folder unknown: default-deny");
        let known = [("tx".to_string(), d.join("zz").to_string_lossy().into_owned(), false)]; assert!(live_conflict(&LiveFold::fold(&known, &start), &d.join("a")).is_none(), "its folder from the copy: elsewhere, fine");
        assert!(live_conflict(&LiveFold::fold(&[("tx".to_string(), d.join("a").to_string_lossy().into_owned(), false)], &start), &d.join("a")).is_some(), "its folder from the copy: here");
        let p = d.join("live.jsonl"); let big = "x".repeat(200); std::fs::write(&p, format!("{big}\n{{\"$\":\"A\"}}\n{{\"$\":\"B\"}}\n")).unwrap();
        assert!(lines_since(&p, 30, &["{\"$\":\"A\"}".to_string()]).unwrap() == vec!["{\"$\":\"B\"}"], "after the copy point's line");
        assert!(lines_since(&p, 30, &["{\"$\":\"Z\"}".to_string()]).is_err() && lines_since(&p, 30, &[]).is_err(), "a tail that does not reach the copy point refuses");
        assert_eq!(lines_since(&p, 1 << 20, &[]).unwrap().len(), 3, "a small log is read whole");
    }
    #[test] fn the_child_loses_every_backplane_variable_and_gets_the_real_home_and_path() {
        let cur = vec![(std::ffi::OsString::from("BACKPLANE_HOME"), std::ffi::OsString::from("x")), ("BACKPLANE_COPY".into(), "1".into()), ("USER".into(), "u".into())];
        let (rm, set) = child_env(Path::new("/home/real"), std::ffi::OsStr::new("/a:/b"), cur.into_iter());
        assert_eq!(rm.len(), 2); assert!(rm.iter().all(|k| k.to_string_lossy().starts_with("BACKPLANE_"))); assert_eq!(set, vec![("HOME".to_string(), "/home/real".into()), ("PATH".to_string(), "/a:/b".into())]);
    }
}

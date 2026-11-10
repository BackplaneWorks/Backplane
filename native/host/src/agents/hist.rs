//! Design history steps: after a tool call, at a turn's start and at its end the
//! thread's KiCad files are snapshotted (files::git::step_capture, the files stream's port of Git.step.capture); a change is logged in the
//! thread as an Act of tone "step" whose summary is the step's record. Turn checkpoints (Git checkpoint0 / checkpoint) go through
//! files::git too. Everything here runs off the actor, in the background; nothing is waited for.
use crate::files::git as fgit;
use serde_json::{json, Value};

/// SFile.pairs: git's -z output, status then path, a NUL after each.
fn files(rest: &str) -> Vec<Value> {
    let xs: Vec<&str> = rest.split('\0').collect(); let mut out = vec![]; let mut i = 0;
    while i + 1 < xs.len() { let (st, p) = (xs[i], xs[i + 1]); if !(st.is_empty() || p.is_empty()) { out.push(json!(format!("{st}\t{p}"))) } i += 2 }
    out
}
/// Hist.record.at: the capture script's output (commit, previous commit, git dir and top a line each, then the files changed) as a step's
/// record; "" when nothing changed. `when`: "start" at a turn's start (before the agent runs), "" after a tool call or at the turn's end.
pub fn record(out: &str, entry: &str, when: &str) -> String {
    let ls: Vec<&str> = out.split('\n').collect(); let at = |i: usize| ls.get(i).map(|x| x.trim()).unwrap_or("");
    if at(0).is_empty() { return String::new() }
    let rest = ls.iter().skip(4).copied().collect::<Vec<_>>().join("\n");
    json!({"e":entry,"c":at(0),"p":at(1),"g":at(2),"t":at(3),"f":files(&rest),"w":when}).to_string()
}

/// StepJob.run: one capture, logged as an Act when something changed.
pub async fn step(hub: std::sync::Arc<crate::hub::Hub>, thread: String, cwd: String, entry: String, home: String, when: &str) {
    if cwd.is_empty() || crate::copy::on() { return } // a copy runs no git that writes (Copy.git.reads): no hidden refs, no .backplane/history.git in a real folder
    let reg = format!("{home}/history.trusted");
    if let Some(old) = crate::import014::source(std::path::Path::new(&home)) { adopt(&reg, &old.join("history.trusted")) }
    let out = fgit::step_capture(&cwd, &thread, &reg).await;
    let rec = record(&out, &entry, when); if rec.is_empty() { return }
    let commit = out.split('\n').next().unwrap_or("").trim().to_string();
    let _ = hub.commit(&json!({"$":"ActivityLogged","thread":thread,"act":format!("step:{commit}"),"tone":"step","kind":"kicad","summary":rec,"at":super::now_s()})).await;
}
/// The history folders 0.14 made are trusted here too: its registry's lines this one lacks are added (without them the next step in a
/// project 0.14 worked on moved its history.git aside and the bar lost every step before).
pub fn adopt(reg: &str, old: &std::path::Path) {
    let Ok(theirs) = std::fs::read_to_string(old) else { return };
    let mine = std::fs::read_to_string(reg).unwrap_or_default();
    let have: std::collections::HashSet<&str> = mine.lines().collect();
    let add: Vec<&str> = theirs.lines().filter(|l| { let mut f = l.splitn(2, ' '); f.next().is_some_and(|k| k.contains(':')) && f.next().is_some_and(|p| p.starts_with('/') || p.contains(":\\")) && !have.contains(l) }).collect();
    if add.is_empty() { return }
    let mut out = mine; if !out.is_empty() && !out.ends_with('\n') { out.push('\n') }
    for l in add { out.push_str(l); out.push('\n') }
    let _ = std::fs::write(reg, out);
}
/// Where a thread's turn checkpoints go: its code worktree when it works in the main checkout for
/// design files (agents/design.rs: the worktree's main checkout is its folder), else its folder.
pub async fn code_dir(hub: &crate::hub::Hub, th: &str, cwd: &str) -> String {
    let wt = { let m = hub.model().await; m.threads.get(th).map(|t| t.worktree.clone()).unwrap_or_default() };
    if !wt.is_empty() && wt != cwd && crate::files::kicad::canonical::main_of(&wt) == cwd && std::path::Path::new(&wt).is_dir() { wt } else { cwd.to_string() }
}
/// GitJob checkpoint0: a new thread's first turn is checkpoint 0 (inside a git repository only).
pub async fn checkpoint0(cwd: String, thread: String) { if cwd.is_empty() || crate::copy::on() || !fgit::is_repo(&cwd).await { return } if fgit::checkpoint_count(&cwd, &thread).await == 0 { let _ = fgit::checkpoint_capture(&cwd, &thread, 0).await; } }
/// GitJob checkpoint: the work tree at a turn's end, as the next turn number.
pub async fn checkpoint(cwd: String, thread: String) { if cwd.is_empty() || crate::copy::on() || !fgit::is_repo(&cwd).await { return } let n = fgit::checkpoint_count(&cwd, &thread).await; let _ = fgit::checkpoint_capture(&cwd, &thread, n).await; }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn the_0_14_registry_is_adopted_once() {
        let d = std::env::temp_dir().join(format!("bp-adopt-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap();
        let (reg, old) = (d.join("history.trusted"), d.join("old.trusted"));
        std::fs::write(&reg, "57:1 /a/.backplane/history.git").unwrap();
        std::fs::write(&old, "58:2 /b/.backplane/history.git\n57:1 /a/.backplane/history.git\nnonsense\n").unwrap();
        let r = reg.to_string_lossy().into_owned();
        adopt(&r, &old); adopt(&r, &old);
        assert_eq!(std::fs::read_to_string(&reg).unwrap(), "57:1 /a/.backplane/history.git\n58:2 /b/.backplane/history.git\n");
        adopt(&r, &d.join("none")); assert!(std::fs::read_to_string(&reg).unwrap().ends_with("history.git\n"));
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test] fn a_step_record_has_the_commit_its_parent_the_places_and_the_files() {
        let out = "c1\np0\n/r/.git\n/r\nM\0a.kicad_pcb\0A\0b.kicad_sch\0";
        let v: Value = serde_json::from_str(&record(out, "tool1", "start")).unwrap();
        assert_eq!(v, json!({"e":"tool1","c":"c1","p":"p0","g":"/r/.git","t":"/r","f":["M\ta.kicad_pcb","A\tb.kicad_sch"],"w":"start"}));
        assert_eq!(record("", "x", "start"), ""); assert_eq!(record("  \nx", "x", "start"), "");
        assert_eq!(serde_json::from_str::<Value>(&record("c1\n\n/g\n/t\n", "", "start")).unwrap()["f"], json!([]));
    }
}

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
    let out = fgit::step_capture(&cwd, &thread, &reg).await;
    let rec = record(&out, &entry, when); if rec.is_empty() { return }
    let commit = out.split('\n').next().unwrap_or("").trim().to_string();
    let _ = hub.commit(&json!({"$":"ActivityLogged","thread":thread,"act":format!("step:{commit}"),"tone":"step","kind":"kicad","summary":rec,"at":super::now_s()})).await;
}
/// GitJob checkpoint0: a new thread's first turn is checkpoint 0 (inside a git repository only).
pub async fn checkpoint0(cwd: String, thread: String) { if cwd.is_empty() || crate::copy::on() || !fgit::is_repo(&cwd).await { return } if fgit::checkpoint_count(&cwd, &thread).await == 0 { let _ = fgit::checkpoint_capture(&cwd, &thread, 0).await; } }
/// GitJob checkpoint: the work tree at a turn's end, as the next turn number.
pub async fn checkpoint(cwd: String, thread: String) { if cwd.is_empty() || crate::copy::on() || !fgit::is_repo(&cwd).await { return } let n = fgit::checkpoint_count(&cwd, &thread).await; let _ = fgit::checkpoint_capture(&cwd, &thread, n).await; }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_step_record_has_the_commit_its_parent_the_places_and_the_files() {
        let out = "c1\np0\n/r/.git\n/r\nM\0a.kicad_pcb\0A\0b.kicad_sch\0";
        let v: Value = serde_json::from_str(&record(out, "tool1", "start")).unwrap();
        assert_eq!(v, json!({"e":"tool1","c":"c1","p":"p0","g":"/r/.git","t":"/r","f":["M\ta.kicad_pcb","A\tb.kicad_sch"],"w":"start"}));
        assert_eq!(record("", "x", "start"), ""); assert_eq!(record("  \nx", "x", "start"), "");
        assert_eq!(serde_json::from_str::<Value>(&record("c1\n\n/g\n/t\n", "", "start")).unwrap()["f"], json!([]));
    }
}

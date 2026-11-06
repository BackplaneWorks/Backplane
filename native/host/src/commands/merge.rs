//! A thread whose branch is merged into main settles by itself, and its worktree and branch go (the person's word, 2026-11-06).
//! The Bend hub asked gh for merged pull requests (server.bend `MergeJob`); a branch that reaches main locally (a fast-forward, a rebase)
//! has no pull request, so this reads git itself: the branch made commits of its own (its tip is not where it was created) and its tip
//! is in main. Once settled, a clean worktree of ours is checkpointed (a hidden ref), removed, and the branch deleted.
//! Nothing here runs on a copy of a log (it writes git).
use super::{fold::Fold, plan};
use crate::{api::{Fut, Ticker}, files::git, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct MergeTicker { pub fold: Arc<tokio::sync::Mutex<Fold>> }

const MAINS: &[&str] = &["refs/heads/main", "refs/heads/master", "refs/remotes/origin/main", "refs/remotes/origin/master"];

/// The branch's tip when it is merged into one of the main lines, else None.
pub async fn merged_tip(root: &str, branch: &str) -> Option<String> {
    let r = git::run_ro(root, &["rev-parse", "--verify", "-q", &format!("refs/heads/{branch}^{{commit}}")]).await;
    if r.code != 0 { return None }
    let tip = r.out.trim().to_string();
    // the commit the branch was created at: the oldest line of its reflog; a tip still there made no commit of its own
    let r = git::run_ro(root, &["reflog", "show", "--format=%H", &format!("refs/heads/{branch}")]).await;
    let born = if r.code == 0 { r.out.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("").to_string() } else { String::new() };
    if born.is_empty() || born == tip { return None }
    for m in MAINS {
        if git::run_ro(root, &["rev-parse", "--verify", "-q", m]).await.code != 0 { continue }
        if git::run_ro(root, &["merge-base", "--is-ancestor", &tip, m]).await.code == 0 { return Some(tip) }
    }
    None
}

fn ours(wt: &str) -> bool { wt.contains("/.backplane/worktrees/") && !wt.contains("..") }

/// Removes a merged thread's worktree and branch: nothing when the worktree has uncommitted work. True when it went.
async fn clean_up(root: &str, th: &str, branch: &str, wt: &str) -> bool {
    if ours(wt) && std::path::Path::new(wt).join(".git").exists() {
        let st = git::run_ro(wt, &["status", "--porcelain", "-uall"]).await;
        if st.code != 0 || !st.out.trim().is_empty() { return false }
        let n = git::checkpoint_count(wt, th).await;
        if git::checkpoint_capture(wt, th, n).await.is_err() { return false }
        if git::run(root, &["worktree", "remove", wt]).await.code != 0 { return false }
    }
    let _ = git::run(root, &["worktree", "prune"]).await;
    // merged was checked against main just before: -D, since a branch main contains but HEAD does not trips -d
    git::run(root, &["branch", "-D", branch]).await.code == 0
}

impl Ticker for MergeTicker {
    fn every_s(&self) -> u64 { 60 }
    fn tick<'a>(&'a self, hub: &'a Arc<Hub>, now: u64) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if crate::copy::on() { return vec![] }
            let cands: Vec<(String, String, String, String, bool)> = {
                let mut f = self.fold.lock().await;
                let _ = f.sync(&hub.store);
                f.threads.iter().filter(|(_, t)| !t.deleted && !t.branch.is_empty() && t.turn != "running")
                    .filter_map(|(th, t)| {
                        let p = f.projects.get(&t.project).filter(|p| !p.removed && !p.root.is_empty())?;
                        if f.queue(th).iter().any(|_| true) { return None }
                        let recorded = !f.setting(&format!("merged.{th}")).is_empty();
                        // not yet merged: due as Merge.due; merged already: only its leftovers (a worktree) are looked at
                        let due = !plan::merged(&f, th, "", now).is_empty();
                        if !due && !(recorded && !t.worktree.is_empty()) { return None }
                        Some((th.clone(), p.root.clone(), t.branch.clone(), t.worktree.clone(), recorded))
                    }).collect()
            };
            for (th, root, branch, wt, recorded) in cands {
                if merged_tip(&root, &branch).await.is_none() { continue }
                if !recorded {
                    let ev = { let f = self.fold.lock().await; plan::merged(&f, &th, "", now) };
                    if ev.is_empty() || hub.commit_many(&ev).await.is_err() { continue }
                }
                if clean_up(&root, &th, &branch, &wt).await {
                    let mut ev = vec![json!({"$":"WorktreeRemoved","id":th,"at":now})];
                    ev.push(json!({"$":"ActivityLogged","thread":th,"act":plan::hub_id("g", now, hub.revision().await as usize),"tone":"info","kind":"git","summary":format!("worktree and branch {branch} removed after the merge"),"at":now}));
                    let _ = hub.commit_many(&ev).await;
                }
            }
            vec![]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sh(d: &std::path::Path, a: &[&str]) { assert!(std::process::Command::new("git").args(a).current_dir(d).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap().status.success(), "{a:?}") }
    #[tokio::test] async fn a_branch_with_commits_in_main_is_merged_and_goes_with_its_worktree() {
        let d = std::env::temp_dir().join(format!("bp-merge-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
        let r = d.to_str().unwrap();
        sh(&d, &["init", "-q", "-b", "main"]); sh(&d, &["commit", "-q", "--allow-empty", "-m", "a"]);
        let wt = format!("{r}/.backplane/worktrees/w1");
        sh(&d, &["worktree", "add", "-q", "-b", "w1", &wt]);
        assert!(merged_tip(r, "w1").await.is_none(), "a fresh branch is not merged");
        sh(std::path::Path::new(&wt), &["commit", "-q", "--allow-empty", "-m", "b"]);
        assert!(merged_tip(r, "w1").await.is_none(), "work main does not have is not merged");
        sh(&d, &["merge", "-q", "--ff-only", "w1"]);
        assert!(merged_tip(r, "w1").await.is_some());
        assert!(clean_up(r, "t1", "w1", &wt).await);
        assert!(!std::path::Path::new(&wt).exists());
        assert!(git::run_ro(r, &["rev-parse", "--verify", "-q", "refs/heads/w1"]).await.code != 0, "the branch is deleted");
    }
}

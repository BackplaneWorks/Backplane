//! A thread whose branch is merged into main settles by itself, and its worktree and branch go (the person's word, 2026-11-06).
//! The Bend hub asked gh for merged pull requests (server.bend `MergeJob`); a branch that reaches main locally (a fast-forward, a rebase)
//! has no pull request, so this reads git itself: the branch made commits of its own (its tip is not where it was created) and its tip
//! is in main. Once settled, a clean worktree of ours is checkpointed (a hidden ref), removed, and the branch deleted.
//! Nothing here runs on a copy of a log (it writes git).
use super::{fold::Fold, plan};
use crate::{api::{Fut, Ticker}, files::git, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

/// Still at work: waiting messages, an open ask, background or watched rows, a child thread running.
fn busy(f: &Fold, th: &str) -> bool {
    !f.queue(th).is_empty() || f.asks.values().any(|a| a.thread == th)
        || !f.setting(&format!("bg.{th}")).is_empty() || !f.setting(&format!("watch.{th}")).is_empty()
        || f.threads.iter().any(|(c, t)| t.turn == "running" && f.setting(&format!("thread.parent.{c}")) == th)
}

pub struct MergeTicker { pub fold: Arc<tokio::sync::Mutex<Fold>>, /// per thread: the entry count and the tool-call texts read at it
    pub seen: std::sync::Mutex<std::collections::HashMap<String, (usize, Vec<String>)>> }

const MAINS: &[&str] = &["refs/heads/main", "refs/heads/master", "refs/remotes/origin/main", "refs/remotes/origin/master"];

/// Where the thread's work stands: its tip (the worktree's HEAD while the worktree is there, so a branch deleted later is still known;
/// else the branch) and the commit the work began at (the oldest line of that reflog). Neither known: None, and the thread is left alone.
async fn tip_born(root: &str, branch: &str, wt: &str) -> Option<(String, String)> {
    let has_wt = ours(wt) && std::path::Path::new(wt).join(".git").exists();
    let mut places: Vec<(&str, String)> = vec![];
    if has_wt { places.push((wt, "HEAD".into())) }
    places.push((root, format!("refs/heads/{branch}")));
    for (cwd, r) in places {
        let tip = git::run_ro(cwd, &["rev-parse", "--verify", "-q", &format!("{r}^{{commit}}")]).await;
        if tip.code != 0 { continue }
        let log = git::run_ro(cwd, &["reflog", "show", "--format=%H", &r]).await;
        let born = if log.code == 0 { log.out.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("").to_string() } else { String::new() };
        if born.is_empty() { continue }
        return Some((tip.out.trim().to_string(), born));
    }
    None
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Lane { /// no commit of its own, or its tip is not known
    Empty, /// every commit it made is in main, by this way
    Landed(&'static str), /// a commit main lacks
    Pending }

/// How one lane's work stands against the main lines (`main`, `master`, `origin/main`, `origin/master`: a push moves the latter):
/// "ancestor" (a fast-forward or a merge: the tip is in main), "cherry-pick" (every commit it has that main lacks has a patch-id twin in main)
/// or "squash" (its whole change is one commit of main). `Pending` while any of its commits is not in main.
pub async fn lane(root: &str, branch: &str, wt: &str) -> Lane {
    let Some((tip, born)) = tip_born(root, branch, wt).await else { return Lane::Empty };
    if tip == born { return Lane::Empty }
    for m in MAINS {
        if git::run_ro(root, &["rev-parse", "--verify", "-q", &format!("{m}^{{commit}}")]).await.code != 0 { continue }
        if git::run_ro(root, &["merge-base", "--is-ancestor", &tip, m]).await.code == 0 { return Lane::Landed("ancestor") }
        let c = git::run_ro(root, &["cherry", m, &tip]).await;
        if c.code == 0 && c.out.lines().any(|l| l.starts_with('-')) && !c.out.lines().any(|l| l.starts_with('+')) { return Lane::Landed("cherry-pick") }
        let sq = git::sh_ro("mb=$(git merge-base \"$1\" \"$2\") || exit 1; d=$(git diff \"$mb\" \"$2\" | git patch-id --stable | cut -d' ' -f1); [ -n \"$d\" ] || exit 1; \
git log -p --no-merges -n 300 --format='commit %H' \"$mb..$1\" | git patch-id --stable | cut -d' ' -f1 | grep -qx \"$d\"", &[m, &tip], root, &[]).await;
        if sq.code == 0 { return Lane::Landed("squash") }
    }
    Lane::Pending
}
pub async fn landed(root: &str, branch: &str, wt: &str) -> Option<&'static str> { match lane(root, branch, wt).await { Lane::Landed(h) => Some(h), _ => None } }

/// The branches and worktrees a thread made itself, read from its own tool calls: a path under `.backplane/worktrees/<name>`, and the name
/// after `-b`, `-B` or `-c` (`git worktree add -b`, `git checkout -b`, `git switch -c`). Each is (branch, worktree folder or "").
pub fn made_lanes(texts: &[String], root: &str, skip_wt: &str) -> Vec<(String, String)> {
    let ok = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    let mut names: Vec<String> = vec![];
    let mut add = |n: &str| { let n = n.trim_matches(|c| c == '.' || c == '-'); if !n.is_empty() && !matches!(n, "main" | "master") && !names.iter().any(|x| x == n) { names.push(n.to_string()) } };
    for t in texts {
        let mut rest = t.as_str();
        while let Some(i) = rest.find(".backplane/worktrees/") { rest = &rest[i + 21..]; let n: String = rest.chars().take_while(|c| ok(*c)).collect(); add(&n) }
        let tok: Vec<&str> = t.split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ';').filter(|x| !x.is_empty()).collect();
        for w in tok.windows(2) { if matches!(w[0], "-b" | "-B" | "-c") { let n = w[1].trim_start_matches("refs/heads/"); if n.chars().all(|c| ok(c) || c == '/') { add(n) } } }
    }
    names.into_iter().map(|n| { let wt = format!("{root}/.backplane/worktrees/{n}"); (n, if wt == skip_wt { String::new() } else { wt }) }).collect()
}

fn ours(wt: &str) -> bool { wt.contains("/.backplane/worktrees/") && !wt.contains("..") }

/// Whether a merged thread still lives in its worktree. A merge is not the thread's end: a turn the restart cut goes on there, background work and watches run there, a queued message or an open
/// ask will start a turn there. `running` alone is not enough: a restart turns a running thread `interrupted` first, and this ticker used to take its folder before it could carry on.
pub fn in_use(f: &Fold, th: &str) -> bool {
    let Some(t) = f.threads.get(th) else { return false };
    // (`settled` is no help here: ThreadMerged settles a thread the moment the merge is seen, whatever it is doing)
    matches!(t.turn.as_str(), "running" | "interrupted") || !f.setting(&format!("carry.wait.{th}")).is_empty() || !f.queue(th).is_empty()
        || f.asks.values().any(|a| a.thread == th)
        || ["bg", "watch"].iter().any(|k| f.setting(&format!("{k}.{th}")).split('\n').any(|l| !l.is_empty() && !crate::agents::bg::ended(l)))
}
/// Whether a process works in this folder (a shell, a build, a monitor the log knows nothing of). Linux only: elsewhere the log's word stands.
fn process_in(wt: &str) -> bool {
    let Ok(d) = std::fs::read_dir("/proc") else { return false };
    let inside = |p: &std::path::Path| p.starts_with(wt);
    d.flatten().filter(|e| e.file_name().to_string_lossy().bytes().all(|b| b.is_ascii_digit())).any(|e| std::fs::read_link(e.path().join("cwd")).map_or(false, |c| inside(&c)))
}

/// Removes a landed lane's worktree and branch: nothing when the worktree has uncommitted work. True when it went.
async fn clean_up(root: &str, th: &str, branch: &str, wt: &str) -> bool {
    let mut branches = vec![branch.to_string()];
    if ours(wt) && std::path::Path::new(wt).join(".git").exists() {
        let st = git::run_ro(wt, &["status", "--porcelain", "-uall"]).await;
        if st.code != 0 || !st.out.trim().is_empty() { return false }
        let on = git::run_ro(wt, &["symbolic-ref", "--short", "-q", "HEAD"]).await; // the branch it is really on
        if on.code == 0 && !on.out.trim().is_empty() { branches.push(on.out.trim().to_string()) }
        let n = git::checkpoint_count(wt, th).await;
        if git::checkpoint_capture(wt, th, n).await.is_err() { return false }
        if git::run(root, &["worktree", "remove", wt]).await.code != 0 { return false }
    }
    let _ = git::run(root, &["worktree", "prune"]).await;
    // landed was checked against main just before: -D, since a branch main contains (by patch-id too) but HEAD does not trips -d; one
    // already deleted is as good; main itself and the branch the project folder is on stay
    let here = git::run_ro(root, &["symbolic-ref", "--short", "-q", "HEAD"]).await.out.trim().to_string();
    let mut all = true;
    for b in branches.iter().filter(|b| !matches!(b.as_str(), "main" | "master") && **b != here) {
        if git::run_ro(root, &["rev-parse", "--verify", "-q", &format!("refs/heads/{b}")]).await.code == 0 && git::run(root, &["branch", "-D", b]).await.code != 0 { all = false }
    }
    all
}

impl MergeTicker {
    /// The thread's tool calls (its activity rows), read again only when the thread has new entries.
    async fn calls(&self, hub: &Arc<Hub>, th: &str, count: usize) -> Vec<String> {
        if let Some((n, v)) = self.seen.lock().unwrap().get(th) { if *n == count { return v.clone() } }
        let v: Vec<String> = super::entries_of(hub, th).await.entries.into_iter().filter(|l| !l.msg).map(|l| l.text).collect();
        self.seen.lock().unwrap().insert(th.to_string(), (count, v.clone()));
        v
    }
}

impl Ticker for MergeTicker {
    fn every_s(&self) -> u64 { 60 }
    fn tick<'a>(&'a self, hub: &'a Arc<Hub>, now: u64) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if crate::copy::on() { return vec![] }
            // (thread, project folder, hub-assigned branch, its worktree, merge recorded, entry count)
            let cands: Vec<(String, String, String, String, bool, usize)> = {
                let mut f = self.fold.lock().await;
                let _ = f.sync(&hub.store);
                f.threads.iter().filter(|(_, t)| !t.deleted && !t.branch.is_empty() && t.turn != "running")
                    .filter_map(|(th, t)| {
                        let p = f.projects.get(&t.project).filter(|p| !p.removed && !p.root.is_empty())?;
                        if busy(&f, th) { return None }
                        let recorded = !f.setting(&format!("merged.{th}")).is_empty();
                        // not yet merged: due as Merge.due; merged already: only its leftovers (a worktree) are looked at
                        let due = !plan::merged(&f, th, "", now).is_empty();
                        if !due && !(recorded && !t.worktree.is_empty()) { return None }
                        Some((th.clone(), p.root.clone(), t.branch.clone(), t.worktree.clone(), recorded, f.count.get(th).copied().unwrap_or(0)))
                    }).collect()
            };
            for (th, root, branch, wt, recorded, count) in cands {
                // the lanes: the hub's branch and worktree, and any the thread made itself (its commits may be on a branch it chose)
                let mut lanes = vec![(branch.clone(), wt.clone())];
                lanes.extend(made_lanes(&self.calls(hub, &th, count).await, &root, &wt));
                let mut states = vec![];
                for (b, w) in &lanes { states.push(lane(&root, b, w).await) }
                // settled: some lane landed and none has a commit main lacks
                if !states.iter().any(|s| matches!(s, Lane::Landed(_))) || states.iter().any(|s| *s == Lane::Pending) { continue }
                if !recorded {
                    let ev = { let f = self.fold.lock().await; plan::merged(&f, &th, "", now) };
                    if ev.is_empty() || hub.commit_many(&ev).await.is_err() { continue }
                }
                // the merge is recorded; the folders wait until the thread is done with them (this ticker comes back every minute while a worktree stands)
                { let mut f = self.fold.lock().await; let _ = f.sync(&hub.store); if in_use(&f, &th) { continue } }
                for ((b, w), s) in lanes.iter().zip(&states) {
                    if !w.is_empty() && process_in(w) { continue }
                    let went = match s { Lane::Pending => false, Lane::Empty if w.is_empty() => false, _ => clean_up(&root, &th, b, w).await };
                    if went && *w == wt && !wt.is_empty() {
                        let ev = vec![json!({"$":"WorktreeRemoved","id":th,"at":now}),
                            json!({"$":"ActivityLogged","thread":th,"act":plan::hub_id("g", now, hub.revision().await as usize),"tone":"info","kind":"git","summary":format!("worktree and branch {b} removed after the merge"),"at":now})];
                        let _ = hub.commit_many(&ev).await;
                    }
                }
            }
            vec![]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    fn sh(d: &Path, a: &[&str]) -> String { let o = std::process::Command::new("git").args(a).current_dir(d).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap(); assert!(o.status.success(), "{a:?}: {}", String::from_utf8_lossy(&o.stderr)); String::from_utf8_lossy(&o.stdout).trim().to_string() }
    fn repo(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("bp-merge-{name}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
        sh(&d, &["init", "-q", "-b", "main"]); std::fs::write(d.join("f"), "a\n").unwrap(); sh(&d, &["add", "f"]); sh(&d, &["commit", "-q", "-m", "a"]); d
    }
    fn mk(d: &Path, name: &str) -> String { let wt = format!("{}/.backplane/worktrees/{name}", d.display()); sh(d, &["worktree", "add", "-q", "-b", name, &wt]); wt }
    fn commit(wt: &str, file: &str, text: &str, msg: &str) { std::fs::write(Path::new(wt).join(file), text).unwrap(); sh(Path::new(wt), &["add", file]); sh(Path::new(wt), &["commit", "-q", "-m", msg]); }

    #[tokio::test] async fn a_fast_forward_lands_and_goes_with_its_worktree_and_branch() {
        let d = repo("ff"); let r = d.to_str().unwrap(); let wt = mk(&d, "w1");
        assert!(landed(r, "w1", &wt).await.is_none(), "a fresh branch is not landed, though its tip is in main");
        commit(&wt, "b", "b\n", "b");
        assert!(landed(r, "w1", &wt).await.is_none(), "work main does not have is not landed");
        sh(&d, &["merge", "-q", "--ff-only", "w1"]);
        assert_eq!(landed(r, "w1", &wt).await, Some("ancestor"));
        assert!(clean_up(r, "t1", "w1", &wt).await);
        assert!(!Path::new(&wt).exists());
        assert!(git::run_ro(r, &["rev-parse", "--verify", "-q", "refs/heads/w1"]).await.code != 0, "the branch is deleted");
    }
    #[tokio::test] async fn main_moving_on_does_not_land_a_fresh_branch() {
        let d = repo("moved"); let r = d.to_str().unwrap(); let wt = mk(&d, "w1");
        std::fs::write(d.join("g"), "g\n").unwrap(); sh(&d, &["add", "g"]); sh(&d, &["commit", "-q", "-m", "g"]);
        assert!(landed(r, "w1", &wt).await.is_none());
    }
    #[tokio::test] async fn a_cherry_pick_lands_and_a_half_landed_branch_does_not() {
        let d = repo("cp"); let r = d.to_str().unwrap(); let wt = mk(&d, "w1");
        commit(&wt, "b", "b\n", "b"); commit(&wt, "c", "c\n", "c");
        let (b, c) = (sh(&d, &["rev-parse", "w1~1"]), sh(&d, &["rev-parse", "w1"]));
        std::fs::write(d.join("g"), "g\n").unwrap(); sh(&d, &["add", "g"]); sh(&d, &["commit", "-q", "-m", "g"]); // main has moved: the picks are new commits
        sh(&d, &["cherry-pick", &b]);
        assert!(landed(r, "w1", &wt).await.is_none(), "one of two commits is on main");
        sh(&d, &["cherry-pick", &c]);
        assert_eq!(landed(r, "w1", &wt).await, Some("cherry-pick"));
    }
    #[tokio::test] async fn a_squash_lands() {
        let d = repo("sq"); let r = d.to_str().unwrap(); let wt = mk(&d, "w1");
        commit(&wt, "b", "b\n", "b"); commit(&wt, "c", "c\n", "c");
        sh(&d, &["merge", "-q", "--squash", "w1"]); sh(&d, &["commit", "-q", "-m", "w1 squashed"]);
        assert_eq!(landed(r, "w1", &wt).await, Some("squash"));
    }
    #[tokio::test] async fn a_branch_deleted_after_landing_is_still_known_by_its_worktree() {
        let d = repo("del"); let r = d.to_str().unwrap(); let wt = mk(&d, "w1");
        commit(&wt, "b", "b\n", "b"); sh(&d, &["merge", "-q", "--ff-only", "w1"]);
        sh(Path::new(&wt), &["checkout", "-q", "--detach"]); sh(&d, &["branch", "-D", "w1"]);
        assert_eq!(landed(r, "w1", &wt).await, Some("ancestor"));
        assert!(clean_up(r, "t1", "w1", &wt).await && !Path::new(&wt).exists());
    }
    #[tokio::test] async fn origin_main_counts_and_uncommitted_work_is_kept() {
        let d = repo("or"); let r = d.to_str().unwrap(); let wt = mk(&d, "w1");
        commit(&wt, "b", "b\n", "b");
        sh(&d, &["update-ref", "refs/remotes/origin/main", "w1"]); // main itself has not moved
        assert_eq!(landed(r, "w1", &wt).await, Some("ancestor"));
        std::fs::write(Path::new(&wt).join("wip"), "x").unwrap();
        assert!(!clean_up(r, "t1", "w1", &wt).await && Path::new(&wt).exists(), "uncommitted work stays");
    }
    #[test] fn a_thread_lanes_are_read_from_its_own_commands() {
        let r = "/p"; let t = vec!["git worktree add .backplane/worktrees/keys -b keys-short".to_string(), "cd /p/.backplane/worktrees/keys && git commit -m x".into(), "git checkout -b other/side".into(), "git checkout -b main".into()];
        let l = made_lanes(&t, r, "/p/.backplane/worktrees/hub-given");
        assert_eq!(l, vec![("keys".to_string(), "/p/.backplane/worktrees/keys".to_string()), ("keys-short".into(), "/p/.backplane/worktrees/keys-short".into()), ("other/side".into(), "/p/.backplane/worktrees/other/side".into())]);
        assert!(made_lanes(&["cd /p/.backplane/worktrees/hub-given && ls".to_string()], r, "/p/.backplane/worktrees/hub-given").iter().all(|(_, w)| w.is_empty()), "the hub's own worktree is not a lane of its own");
    }
    #[tokio::test] async fn work_done_on_a_branch_the_thread_made_lands_the_thread_and_pending_work_holds_it() {
        let d = repo("own"); let r = d.to_str().unwrap(); let given = mk(&d, "given");
        let own = format!("{r}/.backplane/worktrees/keys"); sh(&d, &["worktree", "add", "-q", "-b", "keys", &own]);
        commit(&own, "k", "k\n", "keys 1");
        assert_eq!((lane(r, "given", &given).await, lane(r, "keys", &own).await), (Lane::Empty, Lane::Pending));
        sh(&d, &["merge", "-q", "--ff-only", "keys"]);
        assert_eq!((lane(r, "given", &given).await, lane(r, "keys", &own).await), (Lane::Empty, Lane::Landed("ancestor")));
        commit(&own, "k2", "k2\n", "keys 2"); // a follow-up after the merge
        assert_eq!(lane(r, "keys", &own).await, Lane::Pending, "commits made after the merge hold it until they land too");
        sh(&d, &["merge", "-q", "--ff-only", "keys"]);
        assert!(clean_up(r, "t1", "keys", &own).await && clean_up(r, "t1", "given", &given).await);
        assert!(!Path::new(&own).exists() && !Path::new(&given).exists());
    }
    fn fold(lines: &[&str]) -> Fold { let mut f = Fold::default(); f.threads.insert("t1".into(), Default::default()); for l in lines { f.apply(l); } f }
    fn set(k: &str, v: &str) -> String { json!({"$":"SettingSet","key":k,"value":v,"at":1}).to_string() }
    #[test] fn a_merged_thread_keeps_its_folder_while_it_still_lives_there() {
        let turn = |s: &str| json!({"$":"TurnChanged","thread":"t1","state":s,"at":1}).to_string();
        assert!(!in_use(&fold(&[&turn("completed")]), "t1"), "done with it: the folder may go");
        assert!(in_use(&fold(&[&turn("running")]), "t1"));
        assert!(in_use(&fold(&[&turn("interrupted")]), "t1"), "the restart cut it: it goes on here (the Bub storage thread's folder went this way)");
        assert!(in_use(&fold(&[&json!({"$":"ThreadMerged","id":"t1","url":"","at":5}).to_string(), &turn("interrupted")]), "t1"), "a merge settles the thread; the cut turn still goes on here");
        assert!(in_use(&fold(&[&turn("completed"), &set("carry.wait.t1", "1")]), "t1"), "waiting its turn to carry on");
        assert!(in_use(&fold(&[&turn("completed"), &set("watch.t1", "w1\trunning\tsleep 9")]), "t1"), "monitoring: its turn is over, its work is not");
        assert!(in_use(&fold(&[&turn("completed"), &set("bg.t1", "b1\trunning\tmake")]), "t1"));
        assert!(!in_use(&fold(&[&turn("completed"), &set("watch.t1", &format!("w1\t{}done\tsleep 9", crate::agents::bg::ENDED))]), "t1"), "that work ended");
        let ask = json!({"$":"AskOpened","id":"a1","thread":"t1","kind":"approval","tool":"Bash"}).to_string();
        assert!(in_use(&fold(&[&turn("completed"), &ask]), "t1"), "an open ask comes back to this folder");
        assert!(!in_use(&fold(&[]), "nobody"));
    }
    #[test] fn a_process_in_the_folder_counts() {
        let d = std::env::current_dir().unwrap();
        // Linux reads /proc; elsewhere there is no way to ask and the log's word stands (false)
        assert_eq!(process_in(d.to_str().unwrap()), cfg!(target_os = "linux"), "this test runs here");
        assert!(!process_in("/no/such/folder"));
    }
}

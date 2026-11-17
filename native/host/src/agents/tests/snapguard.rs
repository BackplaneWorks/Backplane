//! #1163: a review request on a copied-in KiCad project that was no git repository removed ~90% of the board. The hub now makes the
//! folder a repository with its work committed, snapshots before every turn, runs a review read-only, flags a mass loss with an ask to
//! restore, and restores to before a turn as a new commit (snapguard.rs, files/snapshot.rs).
use super::*;
use crate::files::snapshot as sn;

/// A stand-in claude that, on a user line, does what the message says: `nuke` rewrites board.kicad_pcb with one footprint left (what the
/// person saw), `edit` changes a.txt and adds new.txt; then it answers.
const DOER: &str = r#"#!/bin/sh
sid="d-$$"
while IFS= read -r line; do
  case "$line" in *'"type":"user"'*)
    case "$line" in *nuke*) printf '(kicad_pcb (version 20241229)\n (footprint "R1" (at 1 1))\n)\n' > board.kicad_pcb ;; esac
    case "$line" in *edit*) echo agent > a.txt; echo new > new.txt ;; esac
    printf '%s\n' '{"type":"system","subtype":"init","session_id":"'"$sid"'","cwd":"/tmp","tools":["Bash"],"model":"fake","permissionMode":"default"}'
    printf '%s\n' '{"type":"assistant","message":{"id":"m'"$$"'","role":"assistant","content":[{"type":"text","text":"ok"}]},"session_id":"'"$sid"'"}'
    printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"'"$sid"'","modelUsage":{"fake":{"contextWindow":200000}}}' ;;
  esac
done
"#;
fn board(n: usize) -> String { let mut s = "(kicad_pcb (version 20241229)\n (net 0 \"\")\n".to_string(); for i in 0..n { s += &format!(" (footprint \"R{i}\" (at {i} 1) (pad \"1\" smd rect))\n (segment (start {i} 0) (end {i} 1))\n") } s + ")\n" }
fn git_out(d: &Path, a: &[&str]) -> String { let o = std::process::Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(a).current_dir(d).output().unwrap(); assert!(o.status.success(), "git {a:?}: {}", String::from_utf8_lossy(&o.stderr)); String::from_utf8_lossy(&o.stdout).into_owned() }
async fn turn(hub: &Arc<Hub>, text: &str) -> Vec<Value> {
    let from = len(hub).await;
    assert_eq!(rpc(hub, "turn.start", json!({"thread":T1,"text":text})).await["ok"], true);
    until(hub, from, 30, |ev| ev.iter().any(|e| turn_is(e, "completed") || turn_is(e, "failed"))).await;
    tokio::time::sleep(Duration::from_millis(300)).await; // the turn's end guard runs beside the end
    log_from(hub, from).await
}

/// The issue's scenario: a review request on a board that is not in a repository. Nothing of the person's is lost: the folder becomes a
/// repository with the board committed first, the turn has a snapshot, the hub's own write tools refuse the review's writes, the board's
/// loss opens an ask to restore, and Restore brings the board back as a new commit.
#[tokio::test(flavor = "multi_thread")] async fn issue_1163_a_review_of_a_board_that_is_no_repository_loses_nothing_and_restores() {
    let bin = dir("sg-bin"); script(&bin, "claude", DOER);
    let full = board(50);
    let (hub, ag, _home, work) = start("sg-1163", |w| { std::fs::write(w.join("board.kicad_pcb"), &full).unwrap(); std::fs::write(w.join("board.kicad_pro"), "{}").unwrap(); std::fs::write(w.join("board-backups.zip"), "x").unwrap(); base_log(w, 1, "") }, |_| Cli::standin(&bin)).await;
    assert!(!work.join(".git").exists());
    // the stand-in plays the agent that "nuked 90% of the existing design" in a turn the person asked only for a review
    let ev = turn(&hub, "Do a review of where we are at and create a task list of things to work on. nuke").await;
    // a repository, its first commit the work that was there
    assert!(work.join(".git").is_dir(), "the folder is a git repository now");
    let first = git_out(&work, &["log", "--reverse", "--format=%s", "-1"]); let root = git_out(&work, &["rev-list", "--max-parents=0", "HEAD"]);
    assert_eq!(git_out(&work, &["log", "--format=%s", root.trim()]).lines().last(), Some(sn::FIRST), "{first}");
    assert_eq!(git_out(&work, &["show", &format!("{}:board.kicad_pcb", root.trim())]), full, "the board as it was is committed");
    assert!(std::fs::read_to_string(work.join(".gitignore")).unwrap().contains("*-backups/"));
    assert!(ev.iter().any(|e| is(e, "ActivityLogged") && e["summary"].as_str().is_some_and(|s| s.starts_with("Made this folder a git repository"))), "said: {ev:?}");
    // the turn's snapshot
    let snaps = sn::snaps(&work.to_string_lossy(), T1).await; assert_eq!(snaps.len(), 1, "{snaps:?}");
    assert_eq!(git_out(&work, &["show", &format!("{}:board.kicad_pcb", sn::snap_ref(T1, 1))]), full);
    // a review: read-only to the hub's tools
    assert_eq!(ag.world().await.setting(&super::super::snapguard::review_key(T1)), "1");
    let b = work.join("board.kicad_pcb").to_string_lossy().into_owned();
    let why = ag.write_guard(T1, &b).await.unwrap_err(); assert!(why.contains("this turn is a review") && why.contains("board.kicad_pcb"), "{why}");
    let why = ag.mass_guard(T1, &b, "kiapi.common.commands.DeleteItems", &json!({"item_ids":[1, 2]})).await.unwrap_err(); assert!(why.contains("read-only"), "{why}");
    assert!(ag.mass_guard(T1, &b, "kiapi.common.commands.GetItems", &json!({})).await.is_ok(), "reads go on");
    assert!(ag.files_write_guard(T1, &[".backplane/review.md"]).await.is_ok(), "the review's own report is no design file");
    // the loss is flagged with an ask to restore
    let ask = until(&hub, 0, 10, |ev| ev.iter().any(|e| is(e, "AskOpened") && e["tool"] == super::super::snapguard::RESTORE_TOOL)).await.into_iter().find(|e| is(e, "AskOpened") && e["tool"] == super::super::snapguard::RESTORE_TOOL).expect("a restore ask");
    let detail = ask["detail"].as_str().unwrap();
    assert!(detail.contains("removed a large part of the design") && detail.contains("board.kicad_pcb: 99 of its 100 items removed") && detail.contains("was a review but changed design files: board.kicad_pcb"), "{detail}");
    // Restore: the board is back, as a new commit; the damaged state is kept too
    let head_before = git_out(&work, &["rev-parse", "HEAD"]);
    ag.answer(ask["id"].as_str().unwrap(), super::super::snapguard::RESTORE_YES).await.unwrap();
    let said: Vec<String> = log_from(&hub, 0).await.iter().filter(|e| is(e, "ActivityLogged")).map(|e| e["summary"].as_str().unwrap_or("").to_string()).collect();
    assert_eq!(std::fs::read_to_string(work.join("board.kicad_pcb")).unwrap(), full, "the board is back: {said:?}");
    let log = git_out(&work, &["log", "--format=%s"]);
    assert!(log.lines().next().unwrap().starts_with("Backplane: restore"), "{log}");
    assert_ne!(git_out(&work, &["rev-parse", "HEAD"]), head_before, "a new commit");
    let kept = git_out(&work, &["show", &format!("{}-before-restore:board.kicad_pcb", sn::snap_ref(T1, 1))]);
    assert_eq!(sn::count_items(&kept, false), 1, "the state before the restore is a commit too");
    assert_eq!(git_out(&work, &["status", "--porcelain"]), "", "nothing left uncommitted in Backplane's own repository");
    ag.shutdown();
}

/// A repository the person already has, with uncommitted, staged and untracked work: the snapshot is a commit on its own ref; the branch,
/// the index, the stash and the files are as they were.
#[tokio::test(flavor = "multi_thread")] async fn an_existing_repository_with_uncommitted_work_is_snapshotted_without_touching_branch_index_or_stash() {
    let bin = dir("sg2-bin"); script(&bin, "claude", DOER);
    let (hub, ag, _home, work) = start("sg-existing", |w| {
        std::fs::write(w.join("a.txt"), "one\n").unwrap(); std::fs::write(w.join("board.kicad_pcb"), board(5)).unwrap();
        git_out(w, &["init", "-q", "-b", "main"]); git_out(w, &["add", "."]); git_out(w, &["commit", "-qm", "mine"]);
        std::fs::write(w.join("board.kicad_pcb"), board(6)).unwrap();      // modified, not staged
        std::fs::write(w.join("a.txt"), "staged\n").unwrap(); git_out(w, &["add", "a.txt"]); std::fs::write(w.join("a.txt"), "staged then more\n").unwrap(); // staged, then changed again
        std::fs::write(w.join("notes.txt"), "untracked\n").unwrap();         // untracked
        base_log(w, 1, "") }, |_| Cli::standin(&bin)).await;
    let (head, status, cached) = (git_out(&work, &["rev-parse", "HEAD"]), git_out(&work, &["status", "--porcelain"]), git_out(&work, &["diff", "--cached"]));
    turn(&hub, "hello").await;
    assert_eq!(git_out(&work, &["rev-parse", "HEAD"]), head, "the person's branch did not move");
    assert_eq!(git_out(&work, &["status", "--porcelain"]), status, "the index and work tree are as they were");
    assert_eq!(git_out(&work, &["diff", "--cached"]), cached, "the staged change is still staged");
    assert_eq!(git_out(&work, &["stash", "list"]), "", "nothing stashed");
    assert!(!sn::owned(&work.to_string_lossy()).await, "not marked as Backplane's own");
    let r = sn::snap_ref(T1, 1);
    assert_eq!(git_out(&work, &["rev-parse", &format!("{r}^")]), head, "its parent is HEAD");
    assert_eq!(git_out(&work, &["show", &format!("{r}:board.kicad_pcb")]), board(6), "the uncommitted board is in it");
    assert_eq!(git_out(&work, &["show", &format!("{r}:a.txt")]), "staged then more\n", "the work tree's a.txt");
    assert_eq!(git_out(&work, &["show", &format!("{r}:notes.txt")]), "untracked\n", "the untracked file too");
    assert!(git_out(&work, &["log", "-1", "--format=%an %s", &r]).starts_with("Backplane Backplane: snapshot before t1700000001-2 turn 1"));
    ag.shutdown();
}

/// An agent's edit, then /restore: the files are back as they were before the turn, the files it added are gone, the edited state is a
/// commit of its own, and the person's branch never moved.
#[tokio::test(flavor = "multi_thread")] async fn an_agent_edit_followed_by_restore_brings_the_files_back_and_keeps_the_edit() {
    let bin = dir("sg3-bin"); script(&bin, "claude", DOER);
    let (hub, ag, _home, work) = start("sg-restore", |w| {
        std::fs::write(w.join("a.txt"), "one\n").unwrap(); git_out(w, &["init", "-q", "-b", "main"]); git_out(w, &["add", "."]); git_out(w, &["commit", "-qm", "mine"]);
        std::fs::write(w.join("a.txt"), "mine, uncommitted\n").unwrap(); std::fs::write(w.join("big.bin"), vec![0u8; 64]).unwrap();
        base_log(w, 1, "") }, |_| Cli::standin(&bin)).await;
    let head = git_out(&work, &["rev-parse", "HEAD"]);
    turn(&hub, "please edit the notes").await;
    assert_eq!(std::fs::read_to_string(work.join("a.txt")).unwrap(), "agent\n"); assert!(work.join("new.txt").exists());
    let from = len(&hub).await;
    assert_eq!(rpc(&hub, "turn.start", json!({"thread":T1,"text":"/restore"})).await["ok"], true);
    let ev = log_from(&hub, from).await;
    assert!(!ev.iter().any(|e| turn_is(e, "running")), "the hub restores; no agent turn: {ev:?}");
    let said = ev.iter().find(|e| is(e, "ActivityLogged") && e["summary"].as_str().is_some_and(|s| s.starts_with("Restored the project to before turn 1"))).unwrap_or_else(|| panic!("said: {ev:?}"));
    assert!(said["summary"].as_str().unwrap().contains("Removed 1 file added since."), "{said}");
    assert_eq!(std::fs::read_to_string(work.join("a.txt")).unwrap(), "mine, uncommitted\n", "the person's uncommitted work as it was before the turn");
    assert!(!work.join("new.txt").exists(), "what the agent added is gone");
    assert!(work.join("big.bin").exists(), "an untracked file of the person's stays");
    assert_eq!(git_out(&work, &["rev-parse", "HEAD"]), head, "the person's branch never moved");
    let r = sn::snap_ref(T1, 1);
    assert_eq!(git_out(&work, &["show", &format!("{r}-before-restore:a.txt")]), "agent\n", "the agent's edit is kept as a commit");
    assert_eq!(git_out(&work, &["show", &format!("{r}-restored:a.txt")]), "mine, uncommitted\n", "the restore is a new commit");
    ag.shutdown();
}

/// A mass deletion through KiCad's IPC waits for the person: deletions a dropped commit gave back do not count, a large net loss opens an
/// approval, and a decline refuses the call.
#[tokio::test(flavor = "multi_thread")] async fn a_mass_deletion_needs_the_persons_yes_and_a_dropped_commit_gives_its_deletions_back() {
    let bin = dir("sg4-bin"); script(&bin, "claude", DOER);
    let (hub, ag, _home, work) = start("sg-mass", |w| { std::fs::write(w.join("board.kicad_pcb"), board(25)).unwrap(); base_log(w, 1, "") }, |_| Cli::standin(&bin)).await;
    turn(&hub, "route the board").await;
    let b = work.join("board.kicad_pcb").to_string_lossy().into_owned();
    let ids = |n: usize| json!({"item_ids": (0..n).collect::<Vec<_>>()});
    let c = "kiapi.common.commands.";
    ag.mass_guard(T1, &b, &format!("{c}BeginCommit"), &json!({})).await.unwrap();
    ag.mass_guard(T1, &b, &format!("{c}DeleteItems"), &ids(9)).await.unwrap();
    ag.mass_guard(T1, &b, &format!("{c}EndCommit"), &json!({"action":"CMA_DROP"})).await.unwrap();
    ag.mass_guard(T1, &b, &format!("{c}DeleteItems"), &ids(9)).await.unwrap();         // 9 of 50: under the floor and a fifth
    ag.mass_guard(T1, &b, &format!("{c}CreateItems"), &json!({"items":[1, 2, 3]})).await.unwrap(); // a reroute adds back
    let from = len(&hub).await;
    let (ag2, b2) = (ag.clone(), b.clone());
    let call = tokio::spawn(async move { ag2.mass_guard(T1, &b2, "kiapi.common.commands.DeleteItems", &json!({"item_ids": (0..10).collect::<Vec<_>>()})).await });
    let ev = until(&hub, from, 10, |ev| ev.iter().any(|e| is(e, "AskOpened") && e["tool"] == super::super::snapguard::MASS_TOOL)).await;
    let ask = ev.iter().find(|e| is(e, "AskOpened") && e["tool"] == super::super::snapguard::MASS_TOOL).expect("the person is asked");
    assert!(ask["detail"].as_str().unwrap().contains("board.kicad_pcb: 16 of its 50 items removed (32%)"), "{ask}");
    ag.answer(ask["id"].as_str().unwrap(), "decline").await.unwrap();
    let why = call.await.unwrap().unwrap_err(); assert!(why.contains("declined") && why.contains("nothing was deleted"), "{why}");
    ag.shutdown();
}

/// An Install Git ask a git-less turn opened closes at the next turn once git runs (installed from setup meanwhile): the thread is no
/// longer "Needs you" for it (bp-win, Nov 17: it stayed open after Git was installed and the next turn had run).
#[tokio::test(flavor = "multi_thread")] async fn an_install_git_ask_closes_once_git_runs() {
    let bin = dir("sg5-bin"); script(&bin, "claude", DOER);
    let (hub, ag, _home, _work) = start("sg-gitask", |w| { std::fs::write(w.join("a.txt"), "one\n").unwrap(); base_log(w, 1, "") }, |_| Cli::standin(&bin)).await;
    hub.commit(&json!({"$":"AskOpened","id":"qgit1","thread":T1,"kind":"input","tool":crate::agents::snapguard::GIT_TOOL,"detail":crate::agents::snapguard::GIT_MISSING,"options":"Install Git\nNot now","at":1700000100})).await.unwrap();
    let ev = turn(&hub, "hello").await;
    assert!(ev.iter().any(|e| is(e, "AskClosed") && e["id"] == "qgit1"), "the ask closed: {ev:?}");
    assert!(ev.iter().any(|e| turn_is(e, "completed")), "the turn ran: {ev:?}");
    ag.shutdown();
}

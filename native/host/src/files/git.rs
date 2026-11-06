//! server/git in Rust: the hub's own git (status, per-thread checkpoints and their diffs, revert, the design-history steps
//! and refs, `git show` of a version). The shell scripts are hub's text, extracted from git into scripts/*.sh
//! (test/files/extract_scripts.py --check keeps them equal), so the history.git trust registry, the filter blanking
//! (`bp_safe`) and the capped diff behave exactly as there. Every command runs with Proc.git.env: no fsmonitor, no hooks,
//! no pager. stdout and stderr come back together, as P.Proc.run gives them. Answers are text: the result, or "error: <git's output>".
use std::path::Path;
use tokio::process::Command;

pub const SAFE: &str = include_str!("scripts/safe.sh");
const INDEX_SEED: &str = include_str!("scripts/index_seed.sh");
const STEP: &str = include_str!("scripts/step.sh");
const REFS: &str = include_str!("scripts/refs.sh");
const INIT: &str = include_str!("scripts/init.sh");
const DIFF_CAP: &str = include_str!("scripts/diff_cap.sh");
pub const STEP_MIGRATE: &str = include_str!("scripts/step_migrate.sh");

pub const GIT_ENV: [(&str, &str); 12] = [("GIT_CONFIG_COUNT", "5"), ("GIT_CONFIG_KEY_0", "core.fsmonitor"), ("GIT_CONFIG_VALUE_0", "false"), ("GIT_CONFIG_KEY_1", "core.hooksPath"), ("GIT_CONFIG_VALUE_1", "/dev/null"),
    ("GIT_CONFIG_KEY_2", "core.pager"), ("GIT_CONFIG_VALUE_2", "cat"), ("GIT_CONFIG_KEY_3", "diff.ignoreSubmodules"), ("GIT_CONFIG_VALUE_3", "dirty"),
    ("GIT_CONFIG_KEY_4", "log.showSignature"), ("GIT_CONFIG_VALUE_4", "false"), ("GIT_PAGER", "cat")];

pub struct Ran { pub code: i32, pub out: String }

/// P.Proc.run: `cmd args` in `cwd`, stdin closed, stdout then stderr, code 127 when it cannot start.
async fn spawn(cmd: &str, args: &[&str], cwd: &str, env: &[(&str, &str)], kill: bool, write: bool) -> Ran {
    // default-deny on a copy (BACKPLANE_COPY=1): only a call that says it is a read (`read`, `proc_ro`, `run_ro`, `sh_ro`) runs; `proc`, `run`, `run_idx`, `sh` are writes or unknown
    if let Some(why) = write_refused(crate::copy::on(), write, cmd) { return Ran { code: 126, out: why } }
    // Proc.run.folder: an empty working folder is refused, never the hub's own directory
    if cwd.is_empty() { return Ran { code: 2, out: "No working folder: refusing to use the hub working directory".into() } }
    // a copy of a log runs no network command (copy.rs: curl, gh, ssh, git push/fetch..., also through sh -c)
    if crate::copy::refused(cmd, args) { return Ran { code: 126, out: crate::copy::refusal(cmd) } }
    let mut c = crate::proc::tokio_cmd(crate::proc::Kind::Local, cmd);
    c.args(args).current_dir(cwd).stdin(std::process::Stdio::null()).envs(GIT_ENV.iter().copied()).envs(env.iter().copied()).kill_on_drop(kill);
    // a listed read on a copy must not refresh the real repository's index or take its lock while a live agent works there (git status would)
    if crate::copy::on() { c.env("GIT_OPTIONAL_LOCKS", "0"); }
    match c.output().await {
        Ok(o) => { let mut out = String::from_utf8_lossy(&o.stdout).into_owned(); out.push_str(&String::from_utf8_lossy(&o.stderr)); Ran { code: o.status.code().unwrap_or(1), out } }
        Err(e) => Ran { code: 127, out: e.to_string() },
    }
}
/// A command, left running if the future waiting for it is dropped: a write (commit, push, worktree, checkpoint, export, release) must finish once it has
/// started, or it can die mid-way and leave `index.lock` or a half-made worktree. For a read a caller wants to time out, use `read`.
pub async fn proc(cmd: &str, args: &[&str], cwd: &str, env: &[(&str, &str)]) -> Ran { spawn(cmd, args, cwd, env, false, true).await }
/// A listed READ (it changes nothing outside the hub's home; it may fill a cache inside it): the only kind of `proc` a copy runs. Not killed on drop.
pub async fn proc_ro(cmd: &str, args: &[&str], cwd: &str, env: &[(&str, &str)]) -> Ran { spawn(cmd, args, cwd, env, false, false).await }
/// Copy.write: why a copy refuses `cmd`, None when it may run. A copy writes nothing outside its home, so a command is allowed only when its caller listed it as a read.
pub fn write_refused(copy: bool, write: bool, cmd: &str) -> Option<String> {
    if copy && write { Some(format!("backplane: a hub on a copied log (BACKPLANE_COPY=1) writes nothing outside its home and does not run {cmd} here")) } else { None }
}
/// A READ, killed when the future waiting for it is dropped: a caller's `tokio::time::timeout` (commands/home.rs wraps `git log` and `gh pr list` in
/// 1.5 s, as the JS effect's execFile timeout does) must not leave the child running. Only for commands that change nothing.
pub async fn read(cmd: &str, args: &[&str], cwd: &str, env: &[(&str, &str)]) -> Ran { spawn(cmd, args, cwd, env, true, false).await }
async fn sh(script: &str, args: &[&str], cwd: &str, env: &[(&str, &str)]) -> Ran {
    let mut a = vec!["-c", script, "sh"]; a.extend(args);
    proc("sh", &a, cwd, env).await
}
/// A listed read through `sh -c script`.
pub async fn sh_ro(script: &str, args: &[&str], cwd: &str, env: &[(&str, &str)]) -> Ran {
    let mut a = vec!["-c", script, "sh"]; a.extend(args);
    proc_ro("sh", &a, cwd, env).await
}
/// Git.run: a write, or unknown (refused on a copy); `run_ro` is the read.
pub async fn run(cwd: &str, args: &[&str]) -> Ran { sh(&format!("{SAFE}bp_safe; exec git \"$@\""), args, cwd, &[]).await }
/// Git.run for git that only reads (rev-parse, status, for-each-ref, cat-file, ls-files, a diff between two refs).
pub async fn run_ro(cwd: &str, args: &[&str]) -> Ran { sh_ro(&format!("{SAFE}bp_safe; exec git \"$@\""), args, cwd, &[]).await }
/// Git.run.idx: git with a private index file
pub async fn run_idx(cwd: &str, idx: &str, args: &[&str]) -> Ran {
    // the index file is $0 (Git.run.idx): `-c script idx args..`
    let script = format!("{SAFE}bp_safe; GIT_INDEX_FILE=$0 exec git \"$@\"");
    let mut a = vec!["-c", script.as_str(), idx]; a.extend(args);
    proc("sh", &a, cwd, &[]).await
}
/// Git.seq: a non-zero exit is "error: <output>", else the trimmed output (raw keeps it untouched, for diffs).
fn seq(r: Ran, raw: bool) -> Result<String, String> { if r.code == 0 { Ok(if raw { r.out } else { r.out.trim().to_string() }) } else { Err(format!("error: {}", r.out.trim())) } }
pub fn flat(r: Result<String, String>) -> String { match r { Ok(s) | Err(s) => s } }
/// Git.failed
pub fn failed(s: &str) -> bool { s.starts_with("error: ") || s.contains("\nerror: ") }

/// Git.b64url: base64url of the UTF-8 bytes, no padding.
pub fn b64url(s: &str) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let b = s.as_bytes(); let mut o = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        o.push(A[(n >> 18) as usize & 63] as char); o.push(A[(n >> 12) as usize & 63] as char);
        if c.len() > 1 { o.push(A[(n >> 6) as usize & 63] as char) } if c.len() > 2 { o.push(A[n as usize & 63] as char) }
    }
    o
}
pub fn checkpoint_ref(thread: &str, n: u64) -> String { format!("refs/backplane/checkpoints/{}/turn/{n}", b64url(thread)) }
fn checkpoint_prev(thread: &str, n: u64) -> String { if n == 0 { "refs/backplane/none".into() } else { checkpoint_ref(thread, n - 1) } }

pub async fn is_repo(cwd: &str) -> bool { let r = run_ro(cwd, &["rev-parse", "--is-inside-work-tree"]).await; r.code == 0 && r.out.trim() == "true" }
/// Git.status: `git status --porcelain=v1 --branch -uall` text, or "error: ...".
pub async fn status(cwd: &str) -> String { flat(seq(run_ro(cwd, &["-c", "core.quotepath=false", "status", "--porcelain=v1", "--branch", "-uall"]).await, true)) }

async fn index_seed(cwd: &str, idx: &str) -> Ran { sh(INDEX_SEED, &[idx], cwd, &[]).await }
/// Git.snapshot: the work tree (tracked, ignored tracked, untracked non-ignored) as a tree id, through the thread's private index.
pub async fn snapshot(cwd: &str, thread: &str) -> Result<String, String> {
    let gd = seq(run(cwd, &["rev-parse", "--absolute-git-dir"]).await, false)?;
    let idx = format!("{gd}/backplane-{}.index", b64url(thread));
    seq(index_seed(cwd, &idx).await, false)?;
    seq(run_idx(cwd, &idx, &["add", "-A"]).await, false)?;
    seq(run_idx(cwd, &idx, &["write-tree"]).await, false)
}
/// Git.checkpoint.capture: the work tree as turn n of a thread (no commit lands on any branch); answers the commit id.
pub async fn checkpoint_capture(cwd: &str, thread: &str, n: u64) -> Result<String, String> {
    let tree = snapshot(cwd, thread).await?;
    let prev = run(cwd, &["rev-parse", "--verify", "-q", &checkpoint_prev(thread, n)]).await;
    let parents: Vec<String> = if prev.code == 0 { vec!["-p".into(), prev.out.trim().into()] } else { vec![] };
    let msg = format!("backplane checkpoint {thread} turn {n}");
    let mut a: Vec<&str> = vec!["GIT_AUTHOR_NAME=Backplane", "GIT_AUTHOR_EMAIL=backplane@localhost", "GIT_COMMITTER_NAME=Backplane", "GIT_COMMITTER_EMAIL=backplane@localhost", "git", "commit-tree", "--no-gpg-sign", "-m", &msg, &tree];
    a.extend(parents.iter().map(String::as_str));
    let sha = seq(proc("env", &a, cwd, &[]).await, false)?;
    seq(run(cwd, &["update-ref", &checkpoint_ref(thread, n), &sha]).await, false)?;
    Ok(sha)
}
/// How many checkpoints a thread has (its next turn number): GitJob.count.
pub async fn checkpoint_count(cwd: &str, thread: &str) -> u64 {
    let r = proc_ro("git", &["for-each-ref", "--format=%(refname)", &format!("refs/backplane/checkpoints/{}/turn/", b64url(thread))], cwd, &[]).await;
    if r.code == 0 { r.out.lines().filter(|l| !l.trim().is_empty()).count() as u64 } else { 0 }
}
fn diff_args<'a>(a: &'a str, b: &'a str) -> Vec<&'a str> { vec!["-c", "core.quotepath=false", "diff", "--no-color", "--no-ext-diff", "--no-textconv", "--src-prefix=a/", "--dst-prefix=b/", "-M", a, b, "--"] }
/// Git.run.capped: git through the cap (a file past 64 KB of changes, a diff past 128 KB, more than 150 files, are summarised).
async fn run_capped(cwd: &str, args: &[&str]) -> Ran { sh_ro(&format!("{SAFE}{DIFF_CAP}"), args, cwd, &[]).await }
/// Git.checkpoint.diff: unified diff from checkpoint a to b ("" when equal).
pub async fn checkpoint_diff(cwd: &str, thread: &str, a: u64, b: u64) -> String {
    let (ra, rb) = (checkpoint_ref(thread, a), checkpoint_ref(thread, b));
    flat(seq(run_capped(cwd, &diff_args(&ra, &rb)).await, true))
}
/// Git.checkpoint.diff_worktree: from checkpoint n to the files as they are now.
pub async fn checkpoint_diff_worktree(cwd: &str, thread: &str, n: u64) -> String {
    match snapshot(cwd, thread).await { Err(e) => e, Ok(tree) => { let r = checkpoint_ref(thread, n); flat(seq(run_capped(cwd, &diff_args(&r, &tree)).await, true)) } }
}
/// GitJob.diff: b = -1 means "against the work tree now"; a missing checkpoint (git says 1) is an empty diff.
pub async fn diff(cwd: &str, thread: &str, a: &str, b: &str) -> String {
    let an: u64 = a.parse().unwrap_or(0);
    if b == "-1" {
        let r = run_ro(cwd, &["rev-parse", "--verify", "-q", &checkpoint_ref(thread, an)]).await;
        if r.code == 1 { String::new() } else { checkpoint_diff_worktree(cwd, thread, an).await }
    } else { checkpoint_diff(cwd, thread, an, b.parse().unwrap_or(0)).await }
}
/// Git.checkpoint.revert: the work tree exactly as checkpoint n had it; HEAD, the branch and the real index do not move. "" when done.
pub async fn checkpoint_revert(cwd: &str, thread: &str, n: u64) -> String {
    let go = async {
        let gd = seq(run(cwd, &["rev-parse", "--absolute-git-dir"]).await, false)?;
        let idx = format!("{gd}/backplane-{}.index", b64url(thread));
        let tree = seq(run(cwd, &["rev-parse", "--verify", &format!("{}^{{tree}}", checkpoint_ref(thread, n))]).await, false)?;
        seq(index_seed(cwd, &idx).await, false)?;
        seq(run_idx(cwd, &idx, &["add", "-A"]).await, false)?;
        seq(run_idx(cwd, &idx, &["read-tree", "-u", "--reset", &tree]).await, false)?;
        Ok::<String, String>(String::new())
    };
    flat(go.await)
}
/// Git.step.capture: a step of the thread's KiCad files ("" when nothing changed); `reg` is the hub's history.trusted file.
pub async fn step_capture(cwd: &str, thread: &str, reg: &str) -> String {
    if !super::threadpath::absolute(cwd) { return String::new() } let r = sh(STEP, &[cwd, &b64url(thread), reg], "/", &[]).await; if r.code == 0 { r.out } else { String::new() } }
/// Git.refs: what a file may be compared with (git dir, top, then `kind\tlabel\tcommit` lines).
pub async fn refs(cwd: &str, thread: &str) -> String {
    if !super::threadpath::absolute(cwd) { return String::new() } let r = sh_ro(REFS, &[cwd, &b64url(thread)], "/", &[]).await; if r.code == 0 { r.out } else { String::new() } }
/// Git.show: a file as a commit has it (`git --git-dir G show OBJ`), None when it is not there.
pub async fn show(gitdir: &str, obj: &str) -> Option<Vec<u8>> {
    let o = crate::proc::tokio_cmd(crate::proc::Kind::Local, "sh").args(["-c", "exec 2>/dev/null; git --git-dir=\"$1\" show \"$2\"", "sh", gitdir, obj]).current_dir("/").stdin(std::process::Stdio::null()).envs(GIT_ENV.iter().copied()).kill_on_drop(true).output().await.ok()?; // a pure read: killed with its future
    if o.status.success() { Some(o.stdout) } else { None }
}

// --- stacked actions (Git.act): commit | push | create_pr | commit_push | commit_push_pr | init
pub fn last_line(s: &str) -> String { s.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("").to_string() }
fn label(name: &str, r: Result<String, String>) -> Result<String, String> { r.map_err(|e| format!("error: {name}: {}", e.strip_prefix("error: ").unwrap_or(&e))) }
/// Network commands (push, gh) are refused on a hub running on a copied log, as P.Proc.run does.
fn copy_refused() -> bool { std::env::var("BACKPLANE_COPY").as_deref() == Ok("1") }
const QUIET: [(&str, &str); 3] = [("GH_PROMPT_DISABLED", "1"), ("GH_NO_UPDATE_NOTIFIER", "1"), ("GIT_TERMINAL_PROMPT", "0")];
/// GhRun.plain: the command as it is, without prompts. (The several-accounts choice of core/gh, GhRun.try, is not ported: it
/// only acts when `gh auth status` lists two or more accounts for the remote's host.)
pub async fn plain(cwd: &str, cmd: &str, args: &[&str]) -> Ran { if copy_refused() { return Ran { code: 126, out: format!("refused: {cmd} on a copy of a log") } } proc(cmd, args, cwd, &QUIET).await }

/// GhRun.push (git push and ls-remote): as the user's git has it set up, refused on a copied log. (Its several-accounts retry is GhRun.try, not ported.)
pub async fn push(cwd: &str, args: &[&str]) -> Ran { plain(cwd, "git", args).await }

async fn step_commit(cwd: &str, msg: &str) -> Result<String, String> {
    let msg = if msg.trim().is_empty() { "Update from Backplane" } else { msg };
    label("commit", async {
        seq(run(cwd, &["add", "-A"]).await, false)?;
        let q = run(cwd, &["diff", "--no-ext-diff", "--no-textconv", "--cached", "--quiet"]).await;
        if q.code == 0 { return Ok("commit: nothing to commit".into()) }
        seq(run(cwd, &["commit", "-m", msg]).await, false)?;
        Ok(format!("commit: {}", seq(run(cwd, &["log", "-1", "--format=%h %s"]).await, false)?))
    }.await)
}
async fn branch(cwd: &str) -> Result<String, String> { let b = seq(run(cwd, &["symbolic-ref", "--short", "-q", "HEAD"]).await, false).unwrap_or_default(); if b.is_empty() { Err("error: detached HEAD".into()) } else { Ok(b) } }
async fn step_push(cwd: &str) -> Result<String, String> {
    label("push", async {
        let b = branch(cwd).await?;
        let up = run(cwd, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).await;
        if up.code == 0 { seq(plain(cwd, "git", &["push"]).await, false)?; Ok(format!("push: {b}")) }
        else { seq(plain(cwd, "git", &["push", "-u", "origin", &b]).await, false)?; Ok(format!("push: {b} -> origin/{b}")) }
    }.await)
}
async fn step_pr(cwd: &str, title: &str, body: &str) -> Result<String, String> {
    label("pr", async {
        let b = branch(cwd).await?;
        let args: Vec<&str> = if title.trim().is_empty() { vec!["pr", "create", "--fill", "--head", &b] } else { vec!["pr", "create", "--title", title, "--body", body, "--head", &b] };
        let out = seq(plain(cwd, "gh", &args).await, false)?;
        Ok(format!("pr: {}", last_line(&out)))
    }.await)
}
async fn step_init(cwd: &str) -> Result<String, String> {
    let out = seq(proc("sh", &["-c", INIT], cwd, &[]).await, false)?;
    Ok(format!("{cwd} is a git repository now (commit {out}); new threads get their own worktree"))
}
/// Git.act: one line per step ("commit: <sha> <subject>", "push: <branch>", "pr: <url>"); the first failing step ends it with "error: <step>: ...".
pub async fn act(cwd: &str, action: &str, message: &str, title: &str, body: &str) -> String {
    let steps: &[&str] = match action { "commit" => &["c"], "push" => &["p"], "create_pr" => &["r"], "commit_push" => &["c", "p"], "commit_push_pr" => &["c", "p", "r"], "init" => &["i"], _ => return format!("error: unknown action {action}") };
    let mut log = String::new();
    for s in steps {
        let line = match *s { "c" => step_commit(cwd, message).await, "p" => step_push(cwd).await, "r" => step_pr(cwd, title, body).await, _ => step_init(cwd).await };
        let (l, bad) = match line { Ok(l) => (l, false), Err(e) => (e, true) };
        log = if log.is_empty() { l } else { format!("{log}\n{l}") };
        if bad { return log }
    }
    log
}
pub fn exists(p: &str) -> bool { Path::new(p).exists() }

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> String { let p = std::env::temp_dir().join(format!("bp-git-{}-{n}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p.to_string_lossy().into_owned() }
    async fn g(cwd: &str, a: &[&str]) -> Ran { proc("git", a, cwd, &[("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")]).await }
    #[test] fn b64url_matches_rfc4648() {
        for (s, e) in [("", ""), ("f", "Zg"), ("fo", "Zm8"), ("foo", "Zm9v"), ("foobar", "Zm9vYmFy"), ("t1793832639-1684539", "dDE3OTM4MzI2MzktMTY4NDUzOQ"), ("a?>b", "YT8-Yg")] { assert_eq!(b64url(s), e) }
        assert_eq!(checkpoint_ref("t1", 3), "refs/backplane/checkpoints/dDE/turn/3");
    }
    #[test] fn failed_words() { assert!(failed("error: x")); assert!(failed("a\nerror: b")); assert!(!failed("fine error: x")); assert!(!failed("")) }
    // commands/home.rs times out git log / gh pr list: a READ's child must be gone when the future is dropped; a plain call (a write) must outlive the timeout
    #[cfg(target_os = "linux")] fn alive(pid: i32) -> bool { std::fs::read_to_string(format!("/proc/{pid}/stat")).map_or(false, |s| !s.contains(") Z ")) }
    #[cfg(target_os = "linux")] async fn gone_within(pid: i32, ms: u64) -> bool { for _ in 0..ms / 25 { if !alive(pid) { return true } tokio::time::sleep(std::time::Duration::from_millis(25)).await } !alive(pid) }
    #[cfg(target_os = "linux")]
    #[tokio::test] async fn a_read_is_killed_by_a_timeout_and_a_write_is_not() {
        let d = tmp("kill"); let (a, b) = (format!("{d}/read.pid"), format!("{d}/write.pid"));
        let script = "echo $$ > \"$0\"; exec sleep $1";
        let t = tokio::time::timeout(std::time::Duration::from_millis(300), read("sh", &["-c", script, &a, "30"], "/", &[])).await; assert!(t.is_err(), "the timeout fired");
        let pid: i32 = std::fs::read_to_string(&a).unwrap().trim().parse().unwrap(); assert!(gone_within(pid, 2000).await, "read child {pid} still running after the timeout");
        let t = tokio::time::timeout(std::time::Duration::from_millis(300), proc("sh", &["-c", script, &b, "2"], "/", &[])).await; assert!(t.is_err());
        let pid: i32 = std::fs::read_to_string(&b).unwrap().trim().parse().unwrap(); assert!(alive(pid), "a plain command must outlive its caller's timeout");
        assert!(gone_within(pid, 5000).await, "and end by itself");
    }
    #[tokio::test] async fn checkpoints_diff_revert_and_status() {
        let d = tmp("cp"); g(&d, &["init", "-q"]).await; std::fs::write(format!("{d}/a.txt"), "one\n").unwrap(); g(&d, &["add", "."]).await; g(&d, &["commit", "-qm", "i"]).await;
        assert!(is_repo(&d).await); assert_eq!(checkpoint_count(&d, "th").await, 0);
        let c0 = checkpoint_capture(&d, "th", 0).await.unwrap(); assert_eq!(c0.len(), 40);
        std::fs::write(format!("{d}/a.txt"), "two\n").unwrap(); std::fs::write(format!("{d}/new.txt"), "n\n").unwrap();
        let df = diff(&d, "th", "0", "-1").await; assert!(df.contains("-one") && df.contains("+two") && df.contains("new.txt"), "{df}");
        checkpoint_capture(&d, "th", 1).await.unwrap(); assert_eq!(checkpoint_count(&d, "th").await, 2);
        let d01 = diff(&d, "th", "0", "1").await; assert!(d01.contains("+two"), "{d01}"); assert_eq!(diff(&d, "th", "1", "-1").await, "");
        assert!(status(&d).await.contains("## ")); // the real index was never touched: new.txt is untracked, a.txt unstaged
        assert!(status(&d).await.contains("?? new.txt") && status(&d).await.contains(" M a.txt"));
        assert_eq!(checkpoint_revert(&d, "th", 0).await, ""); assert_eq!(std::fs::read_to_string(format!("{d}/a.txt")).unwrap(), "one\n"); assert!(!Path::new(&format!("{d}/new.txt")).exists());
        assert!(status(&d).await.lines().count() == 1, "HEAD and index unmoved: {}", status(&d).await);
        // a missing checkpoint is an empty diff; outside a repository git's error
        assert_eq!(diff(&d, "other", "0", "-1").await, "");
        let none = tmp("nogit"); assert!(!is_repo(&none).await); assert!(failed(&status(&none).await));
    }
    #[tokio::test] async fn big_diffs_are_capped() {
        let d = tmp("cap"); g(&d, &["init", "-q"]).await; std::fs::write(format!("{d}/big.txt"), "x\n").unwrap(); g(&d, &["add", "."]).await; g(&d, &["commit", "-qm", "i"]).await;
        checkpoint_capture(&d, "th", 0).await.unwrap();
        std::fs::write(format!("{d}/big.txt"), "y\n".repeat(200_000)).unwrap();
        let df = diff(&d, "th", "0", "-1").await; assert!(df.contains("Too big to show here"), "{}", &df[..df.len().min(300)]); assert!(df.len() < 1000);
    }
    #[tokio::test] async fn stacked_actions_against_a_local_bare_remote() {
        for (k, v) in [("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")] { std::env::set_var(k, v) }
        let d = tmp("act"); let bare = tmp("bare"); g(&bare, &["init", "-q", "--bare"]).await; g(&d, &["init", "-q", "-b", "work"]).await;
        g(&d, &["remote", "add", "origin", &bare]).await; std::fs::write(format!("{d}/a.txt"), "1\n").unwrap();
        let r = act(&d, "commit_push", "first", "", "").await; let l: Vec<&str> = r.lines().collect();
        assert!(l[0].starts_with("commit: ") && l[0].ends_with(" first"), "{r}"); assert_eq!(l[1], "push: work -> origin/work", "{r}");
        assert!(g(&bare, &["rev-parse", "work"]).await.out.trim().len() == 40);
        assert_eq!(act(&d, "commit", "", "", "").await, "commit: nothing to commit");
        std::fs::write(format!("{d}/a.txt"), "2\n").unwrap(); let r = act(&d, "commit_push", "second", "", "").await; assert!(r.lines().nth(1).unwrap() == "push: work", "{r}"); // upstream now set
        assert_eq!(g(&bare, &["log", "-1", "--format=%s", "work"]).await.out.trim(), "second");
        assert!(act(&d, "nope", "", "", "").await.starts_with("error: unknown action"));
        // a failing push ends the stack with the step named; a detached HEAD is refused
        g(&d, &["remote", "set-url", "origin", "/nonexistent/remote.git"]).await; std::fs::write(format!("{d}/a.txt"), "3\n").unwrap();
        let r = act(&d, "commit_push", "third", "", "").await; assert!(r.lines().nth(1).unwrap().starts_with("error: push: "), "{r}");
        g(&d, &["checkout", "-q", "--detach"]).await; assert_eq!(act(&d, "push", "", "", "").await, "error: push: detached HEAD");
        // the pull request goes through gh: a stand-in on PATH answers
        let fake = tmp("fakegh"); std::fs::write(format!("{fake}/gh"), "#!/bin/sh\necho creating\necho https://example.invalid/pr/7\n").unwrap();
        use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(format!("{fake}/gh"), std::fs::Permissions::from_mode(0o755)).unwrap();
        g(&d, &["checkout", "-q", "work"]).await; std::env::set_var("PATH", format!("{fake}:{}", std::env::var("PATH").unwrap()));
        assert_eq!(act(&d, "create_pr", "", "", "").await, "pr: https://example.invalid/pr/7");
    }
    #[tokio::test] async fn init_makes_a_repository_only_where_there_is_none() {
        let d = tmp("init"); std::fs::write(format!("{d}/f"), "x").unwrap(); let env = [("GIT_AUTHOR_NAME", "t"), ("GIT_AUTHOR_EMAIL", "t@t"), ("GIT_COMMITTER_NAME", "t"), ("GIT_COMMITTER_EMAIL", "t@t")];
        let r = proc("sh", &["-c", INIT], &d, &env).await; assert_eq!(r.code, 0, "{}", r.out); assert!(is_repo(&d).await);
        let again = proc("sh", &["-c", INIT], &d, &env).await; assert_eq!(again.code, 1); assert!(again.out.contains("already a git repository"));
    }
    #[tokio::test] async fn steps_refs_and_show() {
        let d = tmp("step"); let home = tmp("stephome"); let reg = format!("{home}/history.trusted");
        g(&d, &["init", "-q"]).await;
        assert_eq!(step_capture(&d, "th", &reg).await, ""); // no KiCad files: left at once
        std::fs::write(format!("{d}/b.kicad_pcb"), "(kicad_pcb)").unwrap();
        let s1 = step_capture(&d, "th", &reg).await; let l: Vec<&str> = s1.split('\n').collect();
        assert_eq!(l[0].len(), 40); assert_eq!(l[1], ""); assert!(l[2].ends_with(".git")); assert!(l[3].len() > 1 && s1.contains("b.kicad_pcb"), "{s1:?}");
        assert_eq!(step_capture(&d, "th", &reg).await, ""); // nothing changed
        std::fs::write(format!("{d}/b.kicad_pcb"), "(kicad_pcb 2)").unwrap();
        let s2 = step_capture(&d, "th", &reg).await; assert_eq!(s2.split('\n').nth(1).unwrap(), l[0]);
        let r = refs(&d, "th").await; assert!(r.lines().nth(0).unwrap().ends_with(".git") && r.contains("\n") );
        let gd = l[2]; assert_eq!(show(gd, &format!("{}:b.kicad_pcb", l[0])).await.unwrap(), b"(kicad_pcb)"); assert!(show(gd, "nope:x").await.is_none());
        // a folder outside git keeps its steps in .backplane/history.git
        let f = tmp("shadow"); std::fs::write(format!("{f}/s.kicad_sch"), "(kicad_sch)").unwrap();
        let s3 = step_capture(&f, "th", &reg).await; assert!(s3.contains(".backplane/history.git"), "{s3:?}"); assert!(Path::new(&format!("{f}/.backplane/history.git")).exists());
        // an untrusted history.git (one a zip could ship) is moved aside, not used
        let z = tmp("zip"); std::fs::write(format!("{z}/s.kicad_sch"), "(s)").unwrap(); std::fs::create_dir_all(format!("{z}/.backplane/history.git")).unwrap();
        let s4 = step_capture(&z, "th", &reg).await; assert!(s4.contains(".backplane/history.git") && std::fs::read_dir(format!("{z}/.backplane")).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains("untrusted")), "{s4:?}");
    }
    // Copy.write: a copy runs only what a caller listed as a read; everything else is refused, whatever it is
    #[test] fn a_copy_refuses_every_command_not_listed_as_a_read() {
        assert!(write_refused(true, true, "touch").is_some()); assert!(write_refused(true, true, "git").unwrap().contains("BACKPLANE_COPY=1"));
        assert_eq!(write_refused(true, false, "git"), None); assert_eq!(write_refused(false, true, "touch"), None); assert_eq!(write_refused(false, false, "git"), None);
    }
    #[test] fn copy_child_probe() {
        if std::env::var("BP_COPY_PROBE").ok().as_deref() != Some("1") { return }
        assert!(crate::copy::on());
        let d = std::env::temp_dir().join(format!("bp-copyprobe-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); let cwd = d.to_string_lossy().into_owned();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            // writes and unknowns: refused with code 126, nothing created
            for r in [proc("touch", &["w1"], &cwd, &[]).await, proc("sh", &["-c", "touch w2"], &cwd, &[]).await, run(&cwd, &["init", "-q"]).await, run_idx(&cwd, "i", &["add", "-A"]).await,
                      sh("touch w3", &[], &cwd, &[]).await, checkpoint_capture(&cwd, "t", 0).await.map_or_else(|e| Ran { code: 126, out: e }, |s| Ran { code: 0, out: s }),
                      step_init(&cwd).await.map_or_else(|e| Ran { code: 126, out: e }, |s| Ran { code: 0, out: s })] { assert_eq!(r.code, 126, "{}", r.out); }
            assert!(!d.join("w1").exists() && !d.join("w2").exists() && !d.join("w3").exists() && !d.join(".git").exists(), "something was written");
            // listed reads still run
            assert_eq!(proc_ro("sh", &["-c", "echo ok"], &cwd, &[]).await.out.trim(), "ok");
            assert_eq!(read("sh", &["-c", "echo ok"], &cwd, &[]).await.out.trim(), "ok");
            assert_eq!(run_ro(&cwd, &["--version"]).await.code, 0);
            assert_eq!(proc_ro("sh", &["-c", "echo $GIT_OPTIONAL_LOCKS"], &cwd, &[]).await.out.trim(), "0"); assert_eq!(read("sh", &["-c", "echo $GIT_OPTIONAL_LOCKS"], &cwd, &[]).await.out.trim(), "0");
        });
    }
    #[test] fn a_copy_run_writes_nothing_outside_its_home() {
        let o = std::process::Command::new(std::env::current_exe().unwrap()).args(["files::git::tests::copy_child_probe", "--exact", "--nocapture", "--test-threads=1"])
            .env("BACKPLANE_COPY", "1").env("BP_COPY_PROBE", "1").env_remove("BACKPLANE_AGENTS").output().unwrap();
        assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    }
}

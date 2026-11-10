//! A new thread's worktree: a branch of its own from upstream in
//! `<repo>/.backplane/worktrees`, the folder's uncommitted code work carried over (never its
//! KiCad documents: those live once in the main folder and every thread edits that copy —
//! kicad/canonical.rs); or the thread works in the project folder.
//! Runs after the thread is committed and logs what the server's job reports: `WorktreeSet` / `WorktreeSkipped` and a note.
//! Nothing here runs on a copy of a log (it writes git).
use crate::{files::git::{self, Ran, SAFE}, hub::Hub};
use serde_json::json;
use std::sync::Arc;

fn mix(mut h: u32) -> u32 { h ^= h >> 16; h = h.wrapping_mul(2246822507); h ^= h >> 13; h = h.wrapping_mul(3266489909); h ^ (h >> 16) }
/// B.Wt.seed: the digits of the thread id, read as a number (wrapping).
pub fn seed(id: &str) -> u32 { id.bytes().filter(u8::is_ascii_digit).fold(0u32, |a, c| a.wrapping_mul(10).wrapping_add((c - 48) as u32)) }
/// Git.branch.temp: backplane/<8 hex> from the seed.
pub fn temp_branch(seed: u32) -> String { format!("backplane/{:08x}", mix(seed)) }

/// Git.branch.sanitize: lowercase [a-z0-9._/-], each run of separators one ('/' over '-' over '.'), none leading or trailing, at most 64 chars.
pub fn sanitize(s: &str) -> String {
    let cls = |c: char| if c.is_ascii_digit() || c.is_ascii_lowercase() || c == '_' { 0 } else if c == '.' { 1 } else if c == '/' { 3 } else { 2 };
    let (mut out, mut pend, mut started) = (String::new(), 0u8, false);
    for c in s.chars().map(|c| c.to_ascii_lowercase()) {
        let k = cls(c);
        if k == 0 { if started && pend > 0 { out.push(match pend { 3 => '/', 2 => '-', _ => '.' }) } pend = 0; out.push(c); started = true } else if started { pend = pend.max(k) }
    }
    // a pending separator at the end is dropped; the first 64 characters are kept, and separators at the end of those too
    let t: String = out.chars().take(64).collect();
    t.trim_end_matches(|c| c == '.' || c == '/' || c == '-').to_string()
}

const UPSTREAM: &str = "bp_safe; bp_fetchable() { c=$(git config --show-scope --name-only --list 2>/dev/null) || return 1; ! printf '%s\\n' \"$c\" | awk -F '\\t' '$1 == \"local\" || $1 == \"worktree\" { print tolower($2) }' | grep -Eq '^(core[.](sshcommand|gitproxy|askpass|alternaterefscommand)|credential[.].*|protocol[.].*|url[.].*|ssh[.].*|uploadpack[.].*|remote[.].*[.](uploadpack|receivepack|vcs|proxy))$'; }; [ \"$1\" = 1 ] && git ls-files -m -o -d --exclude-standard | grep -v -e '-backups/' -e '\\.lck$' -e 'fp-info-cache$' -e '_autosave-' | grep -q . && exit 0; git remote get-url origin >/dev/null 2>&1 || exit 0; b=$(git symbolic-ref -q --short refs/remotes/origin/HEAD); b=${b:-origin/main}; t=; command -v timeout >/dev/null 2>&1 && t='timeout 30'; bp_fetchable && GIT_TERMINAL_PROMPT=0 $t git fetch -q --no-recurse-submodules origin \"${b#origin/}\" </dev/null >/dev/null 2>&1; git rev-parse -q --verify \"refs/remotes/$b\" >/dev/null && printf %s \"$b\"; exit 0";
const EXCLUDE: &str = "f=\"$1/info/exclude\"; mkdir -p \"$1/info\" && { grep -qxF '/.backplane/' \"$f\" 2>/dev/null || printf '\\n# Backplane thread worktrees\\n/.backplane/\\n' >> \"$f\"; }";
const CARRY: &str = "set -eu\nexport GIT_OPTIONAL_LOCKS=0\ncd \"$1\"\nbp_safe\ntest -d \"$2\"\nd=$(mktemp -d \"${TMPDIR:-/tmp}/bp-carry.XXXXXX\")\ntrap 'rm -rf \"$d\"' 0\n\
git diff --no-renames --name-only -z --diff-filter=d HEAD -- > \"$d/changed\"\ngit ls-files -z -o --exclude-standard >> \"$d/changed\"\n\
perl -0ne 'chomp; print \"$_\\0\" unless m{-backups/|\\.lck$|fp-info-cache$|_autosave-|(?i:(?:^|/)(?:mech|mfg)/|\\.pretty/|\\.3dshapes/|\\.(?:kicad_(?:pcb|sch|pro|prl|sym|mod|dru|wks)|step|stp|wrl|fcstd)$|(?:^|/)(?:fp|sym)-lib-table$|\\.fab\\.json$|[^/]\\.backplane\\.json$)}' \"$d/changed\" > \"$d/copy\"\n\
if [ -s \"$d/copy\" ]; then tar -c --null -T \"$d/copy\" -f \"$d/files.tar\"; tar -x -f \"$d/files.tar\" -C \"$2\"; fi\n\
git diff --no-renames --name-only -z --diff-filter=D HEAD -- > \"$d/deleted\"\n\
perl -0ne 'chomp; print \"$_\\0\" unless m{(?i:(?:^|/)(?:mech|mfg)/|\\.pretty/|\\.3dshapes/|\\.(?:kicad_(?:pcb|sch|pro|prl|sym|mod|dru|wks)|step|stp|wrl|fcstd)$|(?:^|/)(?:fp|sym)-lib-table$|\\.fab\\.json$|[^/]\\.backplane\\.json$)}' \"$d/deleted\" > \"$d/del\"\n\
xargs -0 sh -c 'd=$1; shift; for p do if [ ! -e \"$p\" ] && [ ! -L \"$p\" ]; then rm -f -- \"$d/$p\" || exit; fi; done' sh \"$2\" < \"$d/del\"\n\
git diff --no-renames --name-only -z --diff-filter=A HEAD -- > \"$d/added\"\n\
perl -0ne 'chomp; print \"$_\\0\" unless m{-backups/|\\.lck$|fp-info-cache$|_autosave-|(?i:(?:^|/)(?:mech|mfg)/|\\.pretty/|\\.3dshapes/|\\.(?:kicad_(?:pcb|sch|pro|prl|sym|mod|dru|wks)|step|stp|wrl|fcstd)$|(?:^|/)(?:fp|sym)-lib-table$|\\.fab\\.json$|[^/]\\.backplane\\.json$)}' \"$d/added\" > \"$d/track\"\n\
xargs -0 sh -c 'd=$1; shift; for p do if [ -e \"$d/$p\" ] || [ -L \"$d/$p\" ]; then git -C \"$d\" add -N -f -- \"$p\" || exit; fi; done' sh \"$2\" < \"$d/track\"";

fn err(r: &Ran) -> String { format!("error: {}", r.out.trim()) }
fn sub(prefix: &str) -> &str { prefix.strip_suffix('/').unwrap_or(prefix) }
fn is_tree(r: &Ran) -> bool { r.code == 0 && r.out.trim() == "tree" }

/// Git.thread_wt: the thread's directory, "skip: <why>" when there is no repository to branch, or "error: ...".
pub async fn thread_wt(cwd: &str, carry: bool, branch: &str, onto: &str) -> String {
    let b = sanitize(branch);
    let r = git::run_ro(cwd, &["rev-parse", "--show-toplevel"]).await;
    let top = if r.code == 0 { r.out.trim().to_string() } else { String::new() };
    if top.is_empty() { return "skip: not a git repository".into() }
    let r = git::run_ro(cwd, &["rev-parse", "--show-prefix"]).await; if r.code != 0 { return err(&r) }
    let prefix = r.out.trim().to_string();
    // a folder inside another repository that does not track it (ignored, or never added) is no part of it: a worktree of that repository would not have it
    if !prefix.is_empty() && !is_tree(&git::run_ro(&top, &["cat-file", "-t", &format!("HEAD:{}", sub(&prefix))]).await) { return format!("skip: not a git repository (inside {top}, which does not track it)") }
    if git::run_ro(cwd, &["rev-parse", "--verify", "-q", "HEAD"]).await.code != 0 { return "skip: no commit yet".into() }
    // Git.exclude: /.backplane/ in the repository's info/exclude, once
    let r = git::run_ro(&top, &["rev-parse", "--path-format=absolute", "--git-common-dir"]).await; if r.code != 0 { return err(&r) }
    let common = r.out.trim().to_string();
    let r = git::proc("sh", &["-c", EXCLUDE, "sh", &common], &top, &[]).await; if r.code != 0 { return err(&r) }
    // A named base (an Improvements send): that LOCAL branch and nothing else: no fetch, no origin default, no carried work; a base that is not there is an error, never main
    if !onto.is_empty() {
        let r = git::run_ro(&top, &["rev-parse", "--verify", "-q", &format!("refs/heads/{onto}^{{commit}}")]).await;
        if r.code != 0 { return format!("error: base branch {onto} does not exist") }
    }
    // Git.upstream: origin's default branch, fetched first; "" (HEAD) when there is no origin or uncommitted work is carried
    let script = format!("{SAFE}{UPSTREAM}");
    let r = if onto.is_empty() { git::proc("sh", &["-c", &script, "sh", if carry { "1" } else { "0" }], &top, &[]).await } else { git::Ran { code: 0, out: onto.to_string() } };
    let mut base = if r.code == 0 { r.out.trim().to_string() } else { String::new() };
    // a subdirectory project may only exist on the checked-out branch
    if onto.is_empty() && !base.is_empty() && !prefix.is_empty() {
        let r = git::run_ro(&top, &["cat-file", "-t", &format!("{base}:{}", sub(&prefix))]).await;
        if !is_tree(&r) {
            let h = git::run_ro(&top, &["cat-file", "-t", &format!("HEAD:{}", sub(&prefix))]).await;
            if is_tree(&h) { base = String::new() } else { return format!("error: project directory {} is not a directory in {base} or HEAD", sub(&prefix)) }
        }
    }
    let path = format!("{top}/.backplane/worktrees/{}", b.replace('/', "-"));
    let mut args = vec!["worktree", "add", "--no-track", "-b", b.as_str(), path.as_str()]; if !base.is_empty() { args.push(&base) }
    let r = git::run(&top, &args).await; if r.code != 0 { return err(&r) }
    let cwd_out = if prefix.is_empty() { path.clone() } else { format!("{path}/{}", sub(&prefix)) };
    if carry && onto.is_empty() {
        let script = format!("{SAFE}{CARRY}");
        let r = git::proc("sh", &["-c", &script, "sh", &top, &path], &top, &[]).await; if r.code != 0 { return err(&r) }
    }
    cwd_out
}

fn hardware(root: &str) -> bool { crate::files::kicad::canonical::hardware(root) }

async fn note(hub: &Arc<Hub>, th: &str, tone: &str, summary: String) -> Vec<serde_json::Value> {
    let at = super::now_s();
    vec![json!({"$":"ActivityLogged","thread":th,"act":super::plan::hub_id("g", at, hub.alloc_n(1) as usize),"tone":tone,"kind":"git","summary":summary,"at":at})]
}

/// Hub.job.ready: the job's report lands only if the thread still has the provider and account binding it started with; then the change, the
/// record that the worktree was asked for again is cleared (Wt.answered) and the thread's waiting messages start (Wt.start, the agents'
/// `agents.start_queued`). A stale report only commits the change and the clearing.
async fn ready(hub: &Arc<Hub>, fold: &Arc<tokio::sync::Mutex<super::fold::Fold>>, th: &str, provider: &str, binding: &str, change: serde_json::Value) {
    let (same, answered, text) = {
        let mut f = fold.lock().await; let _ = f.sync(&hub.store);
        let now_provider = f.threads.get(th).map(|t| t.provider.clone()).unwrap_or_default();
        let now_binding = f.setting(&format!("account.thread.{th}.{now_provider}")).to_string();
        // Wt.start: the thread's waiting messages start unless held, running, or only handed-over context waits
        let go = f.setting(&format!("thread.config.hold.{th}")) != "on" && f.threads.get(th).map_or(false, |t| t.turn != "running") && f.queue(th).iter().any(|(m, _)| !m.starts_with("handoff:"));
        (now_provider == provider && now_binding == binding, !f.setting(&format!("wt.again.{th}")).is_empty(), go.then(|| f.queue_text(th)))
    };
    let mut ev = vec![change];
    if answered { ev.push(json!({"$":"SettingSet","key":format!("wt.again.{th}"),"value":"","at":super::now_s()})) }
    let _ = hub.commit_many(&ev).await;
    if let (true, Some(text), Some(s)) = (same, text, super::seam::current()) { s.start_queued(th, &text).await }
}

/// WtJob.run for a new thread (`mode` the env the person asked: "auto" or "worktree") or a fork (`mode` "fork", `src` its source).
/// The job's reports are logged as the server's job does: the note, then the thread's folder (or its skip).
/// Tests of the requests keep the job quiet (its reports arrive on their own time).
pub static SUPPRESS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// What an Improvements send names: the local branch to start from and the branch to make (both empty: today's worktree).
#[derive(Clone, Default, Debug)]
/// `prefer`: the project's default base (setting worktree.base.<project> or worktree.base), used when no base was asked for and that local branch exists
pub struct Opts { pub base: String, pub branch: String, pub prefer: String }

pub fn spawn(hub: Arc<Hub>, fold: Arc<tokio::sync::Mutex<super::fold::Fold>>, th: String, root: String, mode: String, src: String, provider: String, binding: String, opts: Opts) {
    if cfg!(test) && SUPPRESS.load(std::sync::atomic::Ordering::Relaxed) { return }
    spawn_with(crate::copy::on(), hub, fold, th, root, mode, src, provider, binding, opts)
}
pub fn spawn_with(copy: bool, hub: Arc<Hub>, fold: Arc<tokio::sync::Mutex<super::fold::Fold>>, th: String, root: String, mode: String, src: String, provider: String, binding: String, opts: Opts) {
    if root.is_empty() { return }
    // a copy of a log runs no git that writes: its job runs on "/" (Copy.nowhere) and says at once that the thread uses the project folder
    let root = if copy { "/".to_string() } else { root };
    tokio::spawn(async move {
        let mut opts = opts;
        if opts.base.is_empty() && !opts.prefer.is_empty() && !copy && git::run_ro(&root, &["rev-parse", "--verify", "-q", &format!("refs/heads/{}^{{commit}}", opts.prefer)]).await.code == 0 { opts.base = opts.prefer.clone() }
        let named = !opts.base.is_empty();
        let carry = mode != "fork" && !named && { let r = root.clone(); tokio::task::spawn_blocking(move || hardware(&r)).await.unwrap_or(false) };
        let branch = if sanitize(&opts.branch).is_empty() { temp_branch(seed(&th)) } else { opts.branch.clone() };
        let mut r = thread_wt(&root, carry, &branch, &opts.base).await;
        if mode == "fork" && !git::failed(&r) && !r.starts_with("skip: ") {
            let n = git::checkpoint_count(&r, &src).await;
            if n > 0 { let _ = git::checkpoint_revert(&r, &src, n - 1).await; }
        }
        if r.starts_with("skip: ") || git::failed(&r) {
            let (tone, why) = if r.starts_with("skip: ") { (if mode == "worktree" { "error" } else { "info" }, r[6..].to_string()) } else { r = r.trim().to_string(); ("error", r.clone()) };
            if named && !r.starts_with("skip: ") {
                // a base was asked for in a repository: this thread never falls back to the project folder from here (the folder is the guarded line); a folder that is no repository has nothing to guard
                let _ = hub.commit_many(&note(&hub, &th, "error", format!("could not make the worktree from {}: {why}", opts.base)).await).await;
                return;
            }
            let _ = hub.commit_many(&note(&hub, &th, tone, format!("working in the project folder: {why}")).await).await;
            ready(&hub, &fold, &th, &provider, &binding, json!({"$":"WorktreeSkipped","id":th,"at":super::now_s()})).await;
        } else {
            // a hardware project: the thread works in the main checkout for design files, and this
            // worktree (with its design files taken out) holds its code (agents/design.rs)
            let hw = { let rr = root.clone(); tokio::task::spawn_blocking(move || hardware(&rr)).await.unwrap_or(false) };
            let stripped = if hw { crate::files::kicad::canonical::strip_design(&r).await.ok() } else { None };
            if stripped.is_some() { let _ = hub.commit_many(&[json!({"$":"SettingSet","key":crate::agents::design::main_key(&th),"value":"on","at":super::now_s()})]).await; }
            ready(&hub, &fold, &th, &provider, &binding, json!({"$":"WorktreeSet","id":th,"branch":sanitize(&branch),"worktree":r,"at":super::now_s()})).await;
            let what = match stripped { Some(n) => format!("code worktree {r} ({n} design file{} left in the main checkout, where this thread edits them)", if n == 1 { "" } else { "s" }), None => format!("worktree {r}{}", if carry { " (with the folder's uncommitted work)" } else { "" }) };
            let _ = hub.commit_many(&note(&hub, &th, "info", what).await).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    // Git.branch.sanitize / Git.branch.temp: the same names hub gives
    #[test] fn branch_names() {
        assert_eq!(sanitize("backplane/ab12cd34"), "backplane/ab12cd34"); assert_eq!(sanitize("  Fix The--Thing!! "), "fix-the-thing"); assert_eq!(sanitize("a//b..c"), "a/b.c"); assert_eq!(sanitize("---"), ""); assert_eq!(sanitize("a/-.b"), "a/b");
        assert_eq!(seed("t12-3"), 123); assert_eq!(temp_branch(0).len(), "backplane/".len() + 8);
        assert_eq!(sanitize(&"a".repeat(70)).len(), 64);
    }
    #[tokio::test] async fn a_folder_that_is_not_a_repository_is_skipped() {
        let d = std::env::temp_dir().join(format!("bp-wt-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap();
        assert_eq!(thread_wt(d.to_str().unwrap(), false, "backplane/x", "").await, "skip: not a git repository");
    }
    #[tokio::test] async fn a_folder_inside_a_repository_that_does_not_track_it_is_skipped_and_a_tracked_one_is_not() {
        let d = std::env::temp_dir().join(format!("bp-wt-nest-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(d.join("build/board")).unwrap(); std::fs::create_dir_all(d.join("boards/main")).unwrap();
        let g = |a: &[&str]| { let o = std::process::Command::new("git").args(a).current_dir(&d).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap(); assert!(o.status.success(), "{a:?}"); };
        std::fs::write(d.join(".gitignore"), "build/\n").unwrap(); std::fs::write(d.join("build/board/b.txt"), "b").unwrap(); std::fs::write(d.join("boards/main/m.txt"), "m").unwrap();
        g(&["init", "-q"]); g(&["add", "."]); g(&["commit", "-qm", "x"]);
        let r = thread_wt(d.join("build/board").to_str().unwrap(), false, "backplane/n1", "").await; assert!(r.starts_with("skip: not a git repository"), "{r}");
        let r = thread_wt(d.join("boards/main").to_str().unwrap(), false, "backplane/n2", "").await; assert!(r.ends_with("/.backplane/worktrees/backplane-n2/boards/main"), "{r}");
        let _ = std::fs::remove_dir_all(&d);
    }
    #[tokio::test] async fn a_repository_gets_a_worktree_on_its_own_branch_and_carries_code_but_never_kicad_documents() {
        // The hybrid split: uncommitted code rides into the thread's worktree; KiCad documents
        // do not (they live once in the main folder — kicad/canonical.rs).
        let d = std::env::temp_dir().join(format!("bp-wt2-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
        let p = d.to_str().unwrap();
        let g = |a: &[&str]| { let o = std::process::Command::new("git").args(a).current_dir(p).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap(); assert!(o.status.success(), "{a:?}"); };
        g(&["init", "-q"]); std::fs::write(d.join("a.kicad_pro"), "{}").unwrap(); g(&["add", "."]); g(&["commit", "-qm", "x"]);
        assert!(thread_wt(p, false, "backplane/none", "").await.starts_with("skip:") == false);
        std::fs::write(d.join("new.kicad_sch"), "uncommitted").unwrap();
        std::fs::write(d.join("code.rs"), "uncommitted code").unwrap();
        let r = thread_wt(p, true, "backplane/carry1", "").await;
        assert!(r.ends_with("/.backplane/worktrees/backplane-carry1"), "{r}");
        assert_eq!(std::fs::read_to_string(format!("{r}/code.rs")).unwrap(), "uncommitted code");
        assert!(!std::path::Path::new(&format!("{r}/new.kicad_sch")).exists(), "a KiCad document is never carried into a worktree");
        assert_eq!(std::fs::read_to_string(d.join("new.kicad_sch")).unwrap(), "uncommitted", "and stays in the main folder");
    }
}

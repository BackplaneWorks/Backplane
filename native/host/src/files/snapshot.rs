//! Every project is a git repository, and the hub snapshots it before an agent can change it (#1163: a review request on a copied-in
//! KiCad project that was no repository removed most of the board, and nothing could bring it back).
//!
//! - `ensure_repo`: a project folder that is in no repository is made one, its existing files committed first ("Backplane: snapshot of
//!   existing work before first use") with a default .gitignore for KiCad's scratch and build outputs. Files over `BIG` are left out and
//!   named, never silently. A home folder, the filesystem root, or a folder of more than `MAX_FILES` files is not made a repository (said).
//! - `snapshot`: the whole work tree (tracked and untracked, not ignored, not over `BIG`) as a commit, taken by the hub before an agent turn
//!   and before the turn's first write. In a repository Backplane made it is a commit on the checked-out branch; in one the person had
//!   already, it is a commit on a ref of its own (`refs/backplane/snapshots/<thread>/<n>`, parent HEAD), so the person's branch, index and
//!   stash are never touched. Either way `snap_ref(thread, n)` names it, and nothing is stashed, reset or discarded.
//! - `restore`: the work tree as a snapshot had it, made as a new snapshot after the current state is snapshotted first: nothing is lost
//!   either way.
//! - Review requests run read-only (`is_review`), and a mass deletion is counted (`count_items`, `mass_loss`).
//!
//! Git runs with the hub's safe environment (no hooks, no repository filters: files/git.rs `SAFE`), through a private index, as
//! Backplane (`AUTHOR`), so a machine where git was never told a name still gets its snapshots.
use super::git::{self, Ran};
use std::path::Path;

/// A file larger than this is left out of a snapshot (and named in its note).
pub const BIG: u64 = 50 * 1024 * 1024;
/// A folder with more files than this (outside ignored folders) is not made a repository on its own.
pub const MAX_FILES: usize = 50_000;
/// Who the hub's snapshot commits are by.
pub const AUTHOR: [(&str, &str); 4] = [("GIT_AUTHOR_NAME", "Backplane"), ("GIT_AUTHOR_EMAIL", "backplane@localhost"), ("GIT_COMMITTER_NAME", "Backplane"), ("GIT_COMMITTER_EMAIL", "backplane@localhost")];
/// The first commit of a repository Backplane made.
pub const FIRST: &str = "Backplane: snapshot of existing work before first use";
/// The git config key that marks a repository Backplane made (its snapshots go on the branch).
pub const OWNED_KEY: &str = "backplane.created";
/// The .gitignore a repository Backplane makes starts with (appended to one the folder already has, once).
pub const GITIGNORE: &str = "# Backplane: KiCad's scratch, local history and build outputs (added when Backplane made this a git repository)
*-backups/
_autosave-*
\\#auto_saved_files\\#
*.lck
~*.lck
*-bak
*.bak
fp-info-cache
*.kicad_prl
.history/
build/
target/
node_modules/
__pycache__/
*.pyc
.DS_Store
Thumbs.db
";

/// Serializes the hub's own git writes on one repository (snapshots, restores, the design commits of agents/design.rs): two of them at
/// once would race for `index.lock`.
pub async fn repo_lock(top: &str) -> tokio::sync::OwnedMutexGuard<()> {
    static M: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>> = std::sync::OnceLock::new();
    let m = { let mut g = M.get_or_init(Default::default).lock().unwrap(); g.entry(top.to_string()).or_default().clone() };
    m.lock_owned().await
}

/// git with the safe environment, an optional private index, and Backplane as author and committer.
async fn g(cwd: &str, idx: Option<&str>, args: &[&str]) -> Ran {
    let script = format!("{}bp_safe; exec git \"$@\"", git::SAFE);
    let mut a = vec!["-c", script.as_str(), "sh"]; a.extend(args);
    let mut env: Vec<(&str, &str)> = AUTHOR.to_vec();
    if let Some(i) = idx { env.push(("GIT_INDEX_FILE", i)) }
    git::proc("sh", &a, cwd, &env).await
}
fn ok(r: Ran, what: &str) -> Result<String, String> { if r.code == 0 { Ok(r.out.trim().to_string()) } else { Err(format!("git {what}: {}", r.out.trim())) } }

/// The top of the repository `dir` is in, None when it is in none.
pub async fn top_of(dir: &str) -> Option<String> {
    if dir.is_empty() || !Path::new(dir).is_dir() { return None }
    let r = git::run_ro(dir, &["rev-parse", "--show-toplevel"]).await;
    (r.code == 0 && !r.out.trim().is_empty()).then(|| r.out.trim().to_string())
}
/// Whether Backplane made this repository (`ensure_repo`): its snapshots are commits on its branch.
pub async fn owned(top: &str) -> bool { git::run_ro(top, &["config", "--bool", OWNED_KEY]).await.out.trim() == "true" }

/// The ref of a thread's snapshot before its turn `n`.
pub fn snap_ref(thread: &str, n: u64) -> String { format!("refs/backplane/snapshots/{}/{n}", git::b64url(thread)) }
fn snap_prefix(thread: &str) -> String { format!("refs/backplane/snapshots/{}/", git::b64url(thread)) }
/// The snapshots a thread has: (turn number, commit), oldest first.
pub async fn snaps(top: &str, thread: &str) -> Vec<(u64, String)> {
    let r = git::run_ro(top, &["for-each-ref", "--format=%(refname) %(objectname)", &snap_prefix(thread)]).await;
    let mut v: Vec<(u64, String)> = r.out.lines().filter_map(|l| { let (name, c) = l.split_once(' ')?; Some((name.rsplit('/').next()?.parse().ok()?, c.trim().to_string())) }).collect();
    v.sort(); v
}
/// The next turn number of a thread's snapshots.
pub async fn next_turn(top: &str, thread: &str) -> u64 { snaps(top, thread).await.last().map_or(1, |(n, _)| n + 1) }

/// Why a folder is not made a repository on its own, None when it may be.
pub fn not_a_project(dir: &Path, user_home: Option<&Path>) -> Option<String> {
    let d = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    if d.parent().is_none() { return Some("it is the filesystem's root".into()) }
    if let Some(h) = user_home { if h.canonicalize().ok().as_deref() == Some(d.as_path()) { return Some("it is your home folder".into()) } }
    // a system or temporary folder itself (a folder inside one, such as a test's project, may be one)
    let tmp = std::env::temp_dir(); let tmp = tmp.canonicalize().unwrap_or(tmp);
    let sys = ["/tmp", "/var/tmp", "/usr", "/etc", "/var", "/opt", "/bin", "/Applications", "/System", "/Library", "C:\\Windows", "C:\\Program Files", "C:\\Program Files (x86)"];
    if d == tmp || sys.iter().any(|x| Path::new(x).canonicalize().ok().as_deref() == Some(d.as_path())) { return Some("it is a system folder".into()) }
    let n = count_files(&d, MAX_FILES + 1);
    if n > MAX_FILES { return Some(format!("it has more than {MAX_FILES} files")) }
    None
}
/// Files under `dir` (KiCad's backups, build outputs and dot folders not counted), stopping at `cap`.
fn count_files(dir: &Path, cap: usize) -> usize {
    let mut n = 0; let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() { if !name.starts_with('.') && !name.ends_with("-backups") && !matches!(name.as_str(), "node_modules" | "target" | "build") { stack.push(e.path()) } }
            else { n += 1; if n >= cap { return n } }
        }
    }
    n
}

/// What `ensure_repo` did.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Ensured { pub top: String, pub made: bool, pub commit: String, pub skipped: Vec<String>, pub not_made: String }
impl Ensured {
    /// The line the thread (or project) shows, "" when there is nothing to say.
    pub fn line(&self) -> String {
        if !self.not_made.is_empty() { return format!("Not made a git repository: {}. Backplane cannot snapshot this folder before agents change it.", self.not_made) }
        if !self.made { return String::new() }
        format!("Made this folder a git repository and committed the work already in it as {} (\"{FIRST}\"), so every agent change can be undone.{}", self.commit, skipped_note(&self.skipped))
    }
}
/// The note for files a snapshot left out.
pub fn skipped_note(skipped: &[String]) -> String {
    if skipped.is_empty() { return String::new() }
    let names: Vec<&str> = skipped.iter().take(5).map(String::as_str).collect();
    format!(" Left out, over {} MB each: {}{}.", BIG / 1024 / 1024, names.join(", "), if skipped.len() > 5 { format!(" and {} more", skipped.len() - 5) } else { String::new() })
}

/// The folder `root` is a git repository afterwards, or says why not. A folder already inside a repository is left as it is (its
/// uncommitted work is the next `snapshot`'s). One that is in none: `git init`, the default .gitignore, `/.backplane/` excluded, and a
/// first commit of everything there (huge files left out, named).
pub async fn ensure_repo(root: &str, user_home: Option<&Path>) -> Result<Ensured, String> {
    if crate::copy::on() { return Err("a hub on a copy makes no repositories".into()) }
    if root.is_empty() || !Path::new(root).is_dir() { return Err(format!("{root} is not a folder")) }
    if let Some(top) = top_of(root).await { return Ok(Ensured { top, ..Default::default() }) }
    let (r2, h2) = (std::path::PathBuf::from(root), user_home.map(Path::to_path_buf));
    if let Some(why) = tokio::task::spawn_blocking(move || not_a_project(&r2, h2.as_deref())).await.unwrap_or(None) { return Ok(Ensured { not_made: why, ..Default::default() }) }
    let _l = repo_lock(root).await;
    if let Some(top) = top_of(root).await { return Ok(Ensured { top, ..Default::default() }) } // made meanwhile
    ok(g(root, None, &["init", "-q"]).await, "init")?;
    ok(g(root, None, &["config", OWNED_KEY, "true"]).await, "config")?;
    let excl = Path::new(root).join(".git/info/exclude");
    if let Some(d) = excl.parent() { let _ = std::fs::create_dir_all(d); }
    let mut ex = std::fs::read_to_string(&excl).unwrap_or_default();
    if !ex.contains("/.backplane/") { if !ex.is_empty() && !ex.ends_with('\n') { ex.push('\n') } ex.push_str("/.backplane/\n"); let _ = std::fs::write(&excl, ex); }
    let gi = Path::new(root).join(".gitignore");
    let have = std::fs::read_to_string(&gi).unwrap_or_default();
    if !have.contains("# Backplane: KiCad's scratch") {
        let text = if have.is_empty() { GITIGNORE.to_string() } else { format!("{have}{}\n{GITIGNORE}", if have.ends_with('\n') { "" } else { "\n" }) };
        std::fs::write(&gi, text).map_err(|e| format!("could not write .gitignore: {e}"))?;
    }
    let top = top_of(root).await.ok_or("git init made no repository")?;
    let s = commit_all(&top, FIRST, true).await?;
    Ok(Ensured { top, made: true, commit: s.0, skipped: s.1, not_made: String::new() })
}

/// The work tree's files over `BIG` that are not ignored (repo-relative).
async fn big_files(top: &str) -> Vec<String> {
    let r = git::run_ro(top, &["ls-files", "-z", "--cached", "--others", "--exclude-standard"]).await;
    let top2 = top.to_string(); let list = r.out;
    tokio::task::spawn_blocking(move || {
        let mut v: Vec<String> = list.split('\0').filter(|p| !p.is_empty()).filter(|p| std::fs::metadata(Path::new(&top2).join(p)).map_or(false, |m| m.is_file() && m.len() > BIG)).map(str::to_string).collect();
        v.sort(); v.dedup(); v
    }).await.unwrap_or_default()
}
/// The work tree as a tree id, through a private index seeded from the real one (its stat cache saves rehashing): every file but the
/// ignored and the huge (a huge one HEAD has keeps HEAD's copy). Answers (tree, huge files left out).
async fn work_tree(top: &str, tag: &str) -> Result<(String, Vec<String>), String> {
    let gd = ok(git::run_ro(top, &["rev-parse", "--absolute-git-dir"]).await, "rev-parse")?;
    let idx = format!("{gd}/backplane-snap-{tag}.index");
    let seed = git::proc("sh", &["-c", &format!("{}bp_safe; {}", git::SAFE, include_str!("scripts/index_seed.sh")), "sh", &idx], top, &[]).await;
    if seed.code != 0 { let _ = std::fs::remove_file(&idx); }
    if !Path::new(&idx).exists() { ok(g(top, Some(&idx), &["read-tree", "--empty"]).await, "read-tree")?; }
    ok(g(top, Some(&idx), &["add", "-A", "--", "."]).await, "add")?;
    let big = big_files(top).await;
    let has_head = git::run_ro(top, &["rev-parse", "-q", "--verify", "HEAD"]).await.code == 0;
    for p in &big {
        let in_head = has_head && git::run_ro(top, &["cat-file", "-e", &format!("HEAD:{p}")]).await.code == 0;
        if in_head { let _ = g(top, Some(&idx), &["reset", "-q", "HEAD", "--", p]).await; } else { let _ = g(top, Some(&idx), &["rm", "-q", "--cached", "--ignore-unmatch", "--", p]).await; }
    }
    let tree = ok(g(top, Some(&idx), &["write-tree"]).await, "write-tree");
    let _ = std::fs::remove_file(&idx);
    Ok((tree?, big))
}
/// The work tree now as a tree id (what a snapshot would hold), without committing anything.
pub async fn tree_now(top: &str, tag: &str) -> Result<String, String> { work_tree(top, tag).await.map(|x| x.0) }
/// A commit of the whole work tree (`work_tree`) on the checked-out branch, the real index then matching it. `first`: the repository has
/// no commit yet. Answers (short commit, huge files left out); the commit is HEAD's when nothing changed.
async fn commit_all(top: &str, msg: &str, first: bool) -> Result<(String, Vec<String>), String> {
    let (tree, big) = work_tree(top, "branch").await?;
    let head = if first { String::new() } else { git::run_ro(top, &["rev-parse", "-q", "--verify", "HEAD"]).await.out.trim().to_string() };
    if !head.is_empty() && git::run_ro(top, &["rev-parse", &format!("{head}^{{tree}}")]).await.out.trim() == tree { return Ok((short(top, &head).await, big)) }
    let mut a = vec!["commit-tree", "--no-gpg-sign", "-m", msg, tree.as_str()];
    if !head.is_empty() { a.extend(["-p", head.as_str()]) }
    let c = ok(g(top, None, &a).await, "commit-tree")?;
    let mut u = vec!["update-ref", "-m", msg, "HEAD", c.as_str()]; if !head.is_empty() { u.push(head.as_str()) }
    ok(g(top, None, &u).await, "update-ref")?;
    let _ = g(top, None, &["reset", "-q"]).await; // the real index is the new HEAD (it is this repository's own: Backplane made it)
    Ok((short(top, &c).await, big))
}
async fn short(top: &str, c: &str) -> String { git::run_ro(top, &["rev-parse", "--short", c]).await.out.trim().to_string() }

/// A snapshot taken.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Snap { pub top: String, pub turn: u64, pub commit: String, pub reference: String, /** a new commit was made (the tree was dirty) */ pub made: bool, pub on_branch: bool, pub skipped: Vec<String> }
impl Snap {
    pub fn line(&self) -> String {
        let place = if self.on_branch { "on the branch" } else { "on its own ref; your branch is untouched" };
        if self.made { format!("Snapshot before this turn: {} ({place}). Restore to before this turn at any time: /restore{}", self.commit, skipped_note(&self.skipped)) }
        else { format!("Nothing uncommitted before this turn: it starts from {}. /restore brings this point back.{}", self.commit, skipped_note(&self.skipped)) }
    }
}
/// The subject of a thread's snapshot commit.
pub fn snap_msg(thread: &str, turn: u64, title: &str, when: &str) -> String {
    let t: String = title.chars().take(72).collect();
    format!("Backplane: snapshot before {thread} turn {turn}{}{}", if when.is_empty() { String::new() } else { format!(", {when}") }, if t.is_empty() || t == "New thread" { String::new() } else { format!("\n\nThread: {t}") })
}

/// The work tree of `top` as a snapshot of thread `thread`, named by `reference` (`snap_ref` for the one before a turn): committed when it
/// differs from HEAD (on the branch in a repository Backplane made, else as its own commit whose parent is HEAD), HEAD itself when clean.
pub async fn snapshot(top: &str, thread: &str, reference: &str, msg: &str) -> Result<Snap, String> {
    if crate::copy::on() { return Err("a hub on a copy takes no snapshots".into()) }
    let _l = repo_lock(top).await;
    let mine = owned(top).await;
    let on_branch = mine && git::run_ro(top, &["symbolic-ref", "-q", "HEAD"]).await.code == 0;
    let (commit, made, skipped) = if on_branch {
        let before = git::run_ro(top, &["rev-parse", "-q", "--verify", "HEAD"]).await.out.trim().to_string();
        let first = before.is_empty();
        let (c, big) = commit_all(top, msg, first).await?;
        let after = git::run_ro(top, &["rev-parse", "HEAD"]).await.out.trim().to_string();
        (c, after != before, big)
    } else {
        let (tree, big) = work_tree(top, &git::b64url(thread)).await?;
        let head = git::run_ro(top, &["rev-parse", "-q", "--verify", "HEAD"]).await.out.trim().to_string();
        let same = !head.is_empty() && git::run_ro(top, &["rev-parse", &format!("{head}^{{tree}}")]).await.out.trim() == tree;
        if same { (head.clone(), false, big) } else {
            let mut a = vec!["commit-tree", "--no-gpg-sign", "-m", msg, tree.as_str()];
            if !head.is_empty() { a.extend(["-p", head.as_str()]) }
            (ok(g(top, None, &a).await, "commit-tree")?, true, big)
        }
    };
    let full = git::run_ro(top, &["rev-parse", &commit]).await.out.trim().to_string();
    ok(g(top, None, &["update-ref", "-m", msg.lines().next().unwrap_or(msg), reference, &full]).await, "update-ref")?;
    let turn = reference.rsplit('/').next().and_then(|n| n.parse().ok()).unwrap_or(0);
    Ok(Snap { top: top.to_string(), turn, commit: short(top, &full).await, reference: reference.to_string(), made, on_branch, skipped })
}

/// The files of a commit's tree (repo-relative).
async fn files_of(top: &str, c: &str) -> Result<std::collections::BTreeSet<String>, String> {
    let r = git::run_ro(top, &["ls-tree", "-r", "-z", "--name-only", c]).await;
    if r.code != 0 { return Err(format!("git ls-tree: {}", r.out.trim())) }
    Ok(r.out.split('\0').filter(|p| !p.is_empty()).map(str::to_string).collect())
}
/// What a restore did.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Restored { pub to: String, pub saved: String, pub commit: String, pub removed: usize }
/// The work tree of `top` exactly as `to_ref` had it, as a new snapshot. The current state is snapshotted first (`<ref>-before-restore`),
/// then every file `to_ref` has is written and every file of that current snapshot that `to_ref` lacks is removed (ignored and huge files
/// are in neither, so never touched), and the result is snapshotted again (`<ref>-restored`): on the branch in a repository Backplane made,
/// else on that ref, the person's branch untouched. Nothing is lost either way: the state before the restore is a commit too.
pub async fn restore(top: &str, thread: &str, to_ref: &str, what: &str) -> Result<Restored, String> {
    if crate::copy::on() { return Err("a hub on a copy restores nothing".into()) }
    let to = ok(git::run_ro(top, &["rev-parse", "--verify", &format!("{to_ref}^{{commit}}")]).await, "rev-parse")?;
    let before = snapshot(top, thread, &format!("{to_ref}-before-restore"), &format!("Backplane: snapshot before restoring {what}")).await
        .map_err(|e| format!("not restored: the current state could not be snapshotted first ({e})"))?;
    let removed = {
        let _l = repo_lock(top).await;
        let now = files_of(top, &before.reference).await?;
        let want = files_of(top, &to).await?;
        let gd = ok(git::run_ro(top, &["rev-parse", "--absolute-git-dir"]).await, "rev-parse")?;
        let idx = format!("{gd}/backplane-restore-{}.index", git::b64url(thread));
        let rt = g(top, Some(&idx), &["read-tree", &to]).await;
        let co = if rt.code == 0 { g(top, Some(&idx), &["checkout-index", "-a", "-f"]).await } else { rt };
        let _ = std::fs::remove_file(&idx);
        ok(co, "checkout-index")?;
        let gone: Vec<&String> = now.difference(&want).collect();
        for p in &gone { let _ = std::fs::remove_file(Path::new(top).join(p)); }
        for p in &gone { let mut d = Path::new(top).join(p); while d.pop() && d != Path::new(top) { if std::fs::remove_dir(&d).is_err() { break } } }
        gone.len()
    };
    let after = snapshot(top, thread, &format!("{to_ref}-restored"), &format!("Backplane: restore {what}")).await?;
    Ok(Restored { to: short(top, &to).await, saved: before.commit, commit: after.commit, removed })
}

/// Whether a request only asks for a review, an assessment or a summary: such a turn runs with the design files read-only. A request
/// that also asks for a change (fix, update, route, delete...) is not one.
pub fn is_review(text: &str) -> bool {
    let t = format!(" {} ", text.to_lowercase().replace(|c: char| !c.is_alphanumeric() && c != '\'', " "));
    const ASK: &[&str] = &[" review", " audit", " assess", " critique", " look over ", " check over ", " sanity check", " summarize", " summarise", " summary", " where we are", " where are we", " task list", " todo list", " to do list", " what's left", " what is left", " feedback on", " evaluate", " inspect"];
    const CHANGE: &[&str] = &[" fix", " change ", " changes to", " apply", " update ", " add ", " remove", " delete", " route", " reroute", " place ", " move ", " implement", " go ahead", " modify", " redo", " rework", " start over", " replace", " rewrite", " refactor", " edit ", " make it", " make the", " correct ", " clean up", " tidy", " build ", " create a board", " create the board", " draw "];
    ASK.iter().any(|w| t.contains(w)) && !CHANGE.iter().any(|w| t.contains(w))
}
/// What a review turn's refused write says.
pub fn review_refusal(what: &str) -> String {
    format!("Refused: this turn is a review, so the project's design files are read-only (Backplane enforces it). {what} was not changed. Report what should change instead; if the person wants the change made, they say so in their next message (\"go ahead and fix it\") and that turn may edit.")
}

/// Items of a KiCad board (footprints, tracks, vias, arcs, zones, drawings, text) or schematic (symbols, wires, labels, junctions, sheets,
/// buses) at the top level of its text: what a mass deletion is measured in.
pub fn count_items(text: &str, schematic: bool) -> usize {
    const PCB: &[&str] = &["footprint", "segment", "via", "arc", "zone", "gr_line", "gr_arc", "gr_circle", "gr_rect", "gr_poly", "gr_curve", "gr_text", "gr_text_box", "dimension", "group", "image"];
    const SCH: &[&str] = &["symbol", "wire", "bus", "junction", "label", "global_label", "hierarchical_label", "no_connect", "sheet", "text", "text_box", "bus_entry", "polyline", "rectangle", "circle", "arc", "image"];
    let heads = if schematic { SCH } else { PCB };
    let (mut depth, mut n, mut in_str, mut esc) = (0i32, 0usize, false, false);
    let b = text.as_bytes(); let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if in_str { if esc { esc = false } else if c == b'\\' { esc = true } else if c == b'"' { in_str = false } i += 1; continue }
        match c {
            b'"' => in_str = true,
            b'(' => {
                depth += 1;
                if depth == 2 { let rest = &text[i + 1..]; let w: &str = rest.split(|ch: char| ch.is_whitespace() || ch == '(' || ch == ')').next().unwrap_or(""); if heads.contains(&w) && !(schematic && w == "symbol" && rest.starts_with("symbol \"")) { n += 1 } }
            }
            b')' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    n
}
/// The smallest loss that counts as a mass deletion, whatever the share: a small board's few parts are not guarded by a ratio.
pub const MASS_FLOOR: usize = 10;
/// A mass deletion: more than a fifth of a document's items gone (net: what was added back counts), and at least `MASS_FLOOR`. The line
/// that says so, None when it is not one.
pub fn mass_loss(file: &str, before: usize, after: usize) -> Option<String> {
    let lost = before.saturating_sub(after);
    (lost >= MASS_FLOOR && lost * 5 > before).then(|| format!("{file}: {lost} of its {before} items removed ({}%)", lost * 100 / before.max(1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn review_requests_are_told_from_change_requests() {
        for r in ["Please review the design and create a task list of things to work on", "Do a review of where we are at and summarize the progress towards our objective",
                  "review my project", "Can you audit the schematic?", "Review this board for manufacturing", "what's left to do on this board?", "Give me feedback on the layout"] {
            assert!(is_review(r), "{r}")
        }
        for c in ["Review the board and fix what you find", "review the DRC errors then reroute the USB pair", "delete the old footprints", "route the board", "Make the changes from the review",
                  "add a decoupling cap to U3", "go ahead", "start over with a new layout"] {
            assert!(!is_review(c), "{c}")
        }
    }
    #[test] fn items_are_counted_at_the_top_level_and_a_mass_deletion_needs_a_fifth_and_the_floor() {
        let pcb = "(kicad_pcb (version 20241229)\n (net 0 \"\")\n (footprint \"R\" (at 1 2) (pad \"1\" smd rect))\n (footprint \"C\" (property \"Reference\" \"(segment\"))\n (segment (start 0 0) (end 1 1))\n (via (at 1 1))\n (zone (net 1))\n (gr_line (start 0 0))\n)";
        assert_eq!(count_items(pcb, false), 6, "a '(segment' inside a string or a pad inside a footprint is no item");
        let sch = "(kicad_sch (version 1) (lib_symbols (symbol \"Device:R\" (symbol \"R_0_1\")))\n (symbol (lib_id \"Device:R\") (at 1 1))\n (wire (pts))\n (label \"A\")\n (junction (at 1 1)))";
        assert_eq!(count_items(sch, true), 4, "library symbols are not placed symbols");
        assert_eq!(mass_loss("b.kicad_pcb", 460, 46).as_deref(), Some("b.kicad_pcb: 414 of its 460 items removed (90%)"));
        assert_eq!(mass_loss("b", 100, 85), None, "15% is not a mass deletion");
        assert_eq!(mass_loss("b", 20, 12), None, "under the floor");
        assert_eq!(mass_loss("b", 2, 0), None, "a two-part board: the floor");
        assert_eq!(mass_loss("b", 100, 120), None, "growth");
    }
    #[tokio::test] async fn a_folder_is_made_a_repository_once_and_a_snapshot_right_after_is_clean() {
        let d = crate::plat::tmp::dir("snap-init"); let root = d.to_string_lossy().into_owned();
        std::fs::write(d.join("b.kicad_pcb"), "(kicad_pcb)").unwrap(); std::fs::create_dir_all(d.join("b-backups")).unwrap(); std::fs::write(d.join("b-backups/b.zip"), "z").unwrap();
        std::fs::write(d.join("fp-info-cache"), "c").unwrap(); std::fs::write(d.join(".gitignore"), "secret.txt\n").unwrap(); std::fs::write(d.join("secret.txt"), "s").unwrap();
        let e = ensure_repo(&root, None).await.unwrap();
        assert!(e.made && !e.commit.is_empty(), "{e:?}");
        assert!(owned(&e.top).await);
        let files = files_of(&e.top, "HEAD").await.unwrap();
        assert_eq!(files.iter().cloned().collect::<Vec<_>>(), [".gitignore", "b.kicad_pcb"], "KiCad's scratch and the person's own ignores are left out");
        let gi = std::fs::read_to_string(d.join(".gitignore")).unwrap(); assert!(gi.starts_with("secret.txt\n") && gi.contains("*-backups/"), "appended, never rewritten: {gi}");
        assert_eq!(ensure_repo(&root, None).await.unwrap().made, false, "once");
        let s = snapshot(&e.top, "t1", &snap_ref("t1", 1), "x").await.unwrap();
        assert!(!s.made, "nothing changed since the first commit: {s:?}");
        let st = git::run_ro(&e.top, &["status", "--porcelain"]).await.out; assert_eq!(st, "", "the repository Backplane made is clean");
    }
    #[tokio::test] async fn a_huge_file_is_left_out_and_named() {
        let d = crate::plat::tmp::dir("snap-big"); let root = d.to_string_lossy().into_owned();
        std::fs::write(d.join("a.kicad_sch"), "(kicad_sch)").unwrap();
        let f = std::fs::File::create(d.join("scan.raw")).unwrap(); f.set_len(BIG + 1).unwrap(); // sparse: no disk used
        let e = ensure_repo(&root, None).await.unwrap();
        assert_eq!(e.skipped, ["scan.raw"]); assert!(e.line().contains("Left out, over 50 MB each: scan.raw."), "{}", e.line());
        assert!(!files_of(&e.top, "HEAD").await.unwrap().contains("scan.raw"));
        assert!(d.join("scan.raw").exists(), "left on disk");
        // a restore never removes a file no snapshot holds
        std::fs::write(d.join("a.kicad_sch"), "(kicad_sch changed)").unwrap();
        restore(&e.top, "t1", "HEAD", "t1 test").await.unwrap();
        assert_eq!(std::fs::read_to_string(d.join("a.kicad_sch")).unwrap(), "(kicad_sch)"); assert!(d.join("scan.raw").exists());
    }
    /// BP_COUNT_BOARD=<a .kicad_pcb> cargo test count_items_cost -- --ignored --nocapture: what the first write of a turn spends counting a board.
    #[test] #[ignore] fn count_items_cost() {
        let Ok(p) = std::env::var("BP_COUNT_BOARD") else { return };
        let t0 = std::time::Instant::now(); let text = std::fs::read_to_string(&p).unwrap(); let read = t0.elapsed();
        let t1 = std::time::Instant::now(); let n = count_items(&text, false); let count = t1.elapsed();
        println!("{p}: {} bytes, {n} items, read {read:?}, count {count:?}", text.len());
    }
    #[test] fn a_home_or_the_root_is_no_project() {
        let d = crate::plat::tmp::dir("snap-home");
        assert_eq!(not_a_project(&d, Some(&d)).as_deref(), Some("it is your home folder"));
        assert_eq!(not_a_project(Path::new("/"), None).as_deref(), Some("it is the filesystem's root"));
        assert_eq!(not_a_project(&d, None), None);
        assert_eq!(not_a_project(&std::env::temp_dir(), None).as_deref(), Some("it is a system folder"));
        #[cfg(unix)] assert_eq!(not_a_project(Path::new("/usr"), None).as_deref(), Some("it is a system folder"));
    }
    #[test] fn the_lines_say_what_was_done() {
        let e = Ensured { top: "/p".into(), made: true, commit: "abc1234".into(), skipped: vec!["video.mp4".into()], not_made: String::new() };
        assert_eq!(e.line(), "Made this folder a git repository and committed the work already in it as abc1234 (\"Backplane: snapshot of existing work before first use\"), so every agent change can be undone. Left out, over 50 MB each: video.mp4.");
        assert_eq!(Ensured::default().line(), "");
        assert!(Ensured { not_made: "it is your home folder".into(), ..Default::default() }.line().starts_with("Not made a git repository: it is your home folder."));
        assert_eq!(snap_msg("t1", 3, "Review the board", ""), "Backplane: snapshot before t1 turn 3\n\nThread: Review the board");
        assert_eq!(snap_msg("t1", 3, "New thread", "before its first edit"), "Backplane: snapshot before t1 turn 3, before its first edit");
    }
}

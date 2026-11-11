//! Design files live once, in the project's main checkout (docs/git.md, "Design files on main"):
//! KiCad documents, libraries, 3D models, mech/, mfg/ and fab files (`is_design`). A hardware
//! thread works there for them and in its own worktree for code (agents/design.rs). A design path
//! named from inside a worktree (`<main>/.backplane/worktrees/<name>/…`) resolves to the main
//! checkout's copy, for the IPC route and the direct-file tools alike; code paths are left alone.
//! The hub commits design files straight to main (`commit_design`), so a thread's code branch
//! never carries board changes, and one writer at a time holds each board or schematic (`doc_lock`).
use super::{kserve, project::{self, Files}};

const MARK: &str = "/.backplane/worktrees/";

fn dir_of(p: &str) -> &str { p.rsplit_once('/').map_or("", |x| x.0) }
fn under(p: &str, dir: &str) -> bool { !dir.is_empty() && p.starts_with(&format!("{dir}/")) }

/// `<main>/.backplane/worktrees/<name>/<rest>` split as (main, rest); None outside a worktree.
pub fn split_worktree(p: &str) -> Option<(String, String)> {
    let (main, after) = p.split_once(MARK)?;
    let rest = after.split_once('/').map_or("", |x| x.1);
    Some((main.to_string(), rest.to_string()))
}
/// A path's main-folder equivalent: a worktree path with the `worktrees/<name>` step removed.
pub fn main_of(p: &str) -> String {
    match split_worktree(p) { Some((m, r)) if !r.is_empty() => format!("{m}/{r}"), Some((m, _)) => m, None => p.to_string() }
}
/// The canonical files a working folder's KiCad work belongs to: the main folder's, always.
pub fn for_cwd(cwd: &str) -> Files { project::read(&main_of(cwd)) }

/// Is `mapped` (a main-folder path) the canonical document, or in the canonical document's
/// folder (a referenced sheet, a `<file>.backplane.json` companion), for some project root
/// between `main` and the path itself? A project may sit below the repository root, so each
/// ancestor is tried as the project root the way `project::read` defines it.
fn is_canonical(main: &str, mapped: &str) -> bool {
    let mut roots = vec![main.to_string()];
    let mut cur = main.to_string();
    for seg in mapped[main.len()..].trim_start_matches('/').split('/').filter(|s| !s.is_empty()) {
        cur = format!("{cur}/{seg}"); roots.push(dir_of(&cur).to_string());
    }
    roots.dedup();
    roots.iter().any(|r| {
        let f = project::read(r);
        !f.pro.is_empty() && ([f.pro.as_str(), f.pcb.as_str(), f.sch.as_str()].contains(&mapped)
            || under(mapped, dir_of(&f.pro)) || under(mapped, dir_of(&f.pcb)) || under(mapped, dir_of(&f.sch)))
    })
}
/// Resolve a requested KiCad path: canonical twins named from a worktree land on main's copy.
pub fn route(requested: &str) -> String {
    let Some((main, rest)) = split_worktree(requested) else { return requested.to_string() };
    let mapped = main_of(requested);
    if mapped != requested && ((is_design(&rest) && hardware_cached(&main)) || is_canonical(&main, &mapped)) { mapped } else { requested.to_string() }
}
/// Direct-file tools work root+relative: (root, relative) to act on, routed the same way.
pub fn direct(root: &str, rel: &str) -> (String, String) {
    let abs = if rel.starts_with('/') { rel.to_string() } else { format!("{root}/{rel}") };
    let routed = route(&abs);
    if routed == abs { return (root.to_string(), rel.to_string()) }
    match split_worktree(&abs) {
        Some((main, _)) => { let rel2 = routed[main.len()..].trim_start_matches('/').to_string(); (main, rel2) }
        None => (root.to_string(), rel.to_string()),
    }
}
/// Is this relative path a canonical document (or its companion) of `root`'s project?
pub fn is_canonical_rel(root: &str, rel: &str) -> bool {
    let abs = main_of(&format!("{root}/{rel}"));
    let f = for_cwd(root);
    [f.pro.as_str(), f.pcb.as_str(), f.sch.as_str()].iter().any(|c| !c.is_empty() && (abs == *c || abs == format!("{c}.backplane.json")))
        || route(&format!("{root}/{rel}")) != format!("{root}/{rel}")
}
/// The project root and canonical files that own a `.kicad_pro`: the shallowest ancestor whose
/// own reading (`.backplane.json` at that root included) names this project.
pub fn for_pro(pro: &str) -> (String, Files) {
    let mut parts: Vec<&str> = pro.split('/').collect();
    parts.pop();
    for n in 1..=parts.len() {
        let root = parts[..n].join("/");
        let root = if root.is_empty() { "/".to_string() } else { root };
        let f = project::read(&root);
        if f.pro == pro { return (root, f) }
    }
    (dir_of(pro).to_string(), project::read(dir_of(pro)))
}
/// Design files: what a hardware thread edits in the main checkout and the hub commits straight
/// to main (KiCad documents, libraries and their tables, 3D models, mech/ and mfg/, fab files,
/// `.backplane.json` companions). Everything else is code and goes through the thread's worktree.
/// KiCad's own scratch (backups, locks, autosaves, the footprint cache) is neither.
pub fn is_design(rel: &str) -> bool {
    let p = rel.trim_start_matches("./"); let low = p.to_ascii_lowercase();
    if is_scratch(&low) { return false }
    let name = low.rsplit('/').next().unwrap_or(&low);
    let segs: Vec<&str> = low.split('/').collect();
    let dirs = &segs[..segs.len().saturating_sub(1)];
    DESIGN_EXT.iter().any(|x| name.ends_with(x)) || matches!(name, "fp-lib-table" | "sym-lib-table" | "design-rules")
        || name.ends_with(".fab.json") || name.ends_with(".backplane.json") && name != ".backplane.json"
        || dirs.iter().any(|d| *d == "mech" || *d == "mfg" || d.ends_with(".pretty") || d.ends_with(".3dshapes"))
}
const DESIGN_EXT: &[&str] = &[".kicad_pcb", ".kicad_sch", ".kicad_pro", ".kicad_prl", ".kicad_sym", ".kicad_mod", ".kicad_dru", ".kicad_wks", ".step", ".stp", ".wrl", ".fcstd"];
fn is_scratch(low: &str) -> bool { low.contains("-backups/") || low.ends_with(".lck") || low.ends_with("fp-info-cache") || low.contains("_autosave-") || low.contains("/.backplane/") || low.starts_with(".backplane/") }
/// The documents one writer at a time holds: boards and schematics.
pub fn is_document(p: &str) -> bool { p.ends_with(".kicad_pcb") || p.ends_with(".kicad_sch") }

/// Hardware: a project with KiCad files (a `.kicad_pro` up to four folders down, or a board or
/// schematic in its root). Such a project's threads work in the main checkout.
pub fn hardware(root: &str) -> bool {
    !project::found(std::path::Path::new(root)).is_empty()
        || std::fs::read_dir(root).map(|d| d.flatten().any(|e| { let n = e.file_name().to_string_lossy().into_owned(); n.ends_with(".kicad_pcb") || n.ends_with(".kicad_sch") })).unwrap_or(false)
}
static HW: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, bool)>>> = std::sync::OnceLock::new();
/// `hardware`, remembered for ten seconds (the viewer asks on every page it builds).
pub fn hardware_cached(root: &str) -> bool {
    let m = HW.get_or_init(Default::default);
    if let Some((at, v)) = m.lock().unwrap().get(root) { if at.elapsed() < std::time::Duration::from_secs(10) { return *v } }
    let v = hardware(root); m.lock().unwrap().insert(root.to_string(), (std::time::Instant::now(), v)); v
}
/// Where a thread's design is shown and opened: the main checkout for a hardware project, even
/// when `root` is the thread's code worktree; any other folder as it is.
pub fn design_root(root: &str) -> String {
    let m = main_of(root);
    if m != root && hardware_cached(&m) { m } else { root.to_string() }
}

/// The named lock of one board or schematic: `kicad-<stem>-<pcb|sch>-<digest8>` (64 at most).
pub fn doc_lock(doc: &str) -> String {
    let file = doc.rsplit('/').next().unwrap_or("board");
    let (stem, kind) = file.rsplit_once('.').map_or((file, "doc"), |(s, x)| (s, x.strip_prefix("kicad_").unwrap_or(x)));
    let stem: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).take(40).collect();
    format!("kicad-{stem}-{kind}-{}", &kserve::digest(doc)[..8]).chars().take(64).collect()
}
/// The document a KiCad request writes: a board or schematic itself, else (a `.kicad_pro`) its
/// project's board, else its schematic.
pub fn doc_of(path: &str) -> String {
    if is_document(path) { return path.to_string() }
    let f = if path.ends_with(".kicad_pro") { let (_, f) = for_pro(path); f } else { Files::default() };
    if !f.pcb.is_empty() { f.pcb } else { f.sch }
}
/// Is this lock one of a KiCad document's?
pub fn is_doc_lock(name: &str) -> bool { name.starts_with("kicad-") && (name.contains("-pcb-") || name.contains("-sch-")) }
/// Why a write to `doc` is refused, naming the holder; None when `caller` holds its lock.
/// Nothing blocks: the caller is told who holds the document and queues with `lock_acquire`,
/// whose grant message wakes it; meanwhile it reads only (reads need no lock).
pub fn write_blocked(lock: &crate::agents::lockrec::Lock, caller: &str, name: &str, doc: &str) -> Option<String> {
    match &lock.holder {
        Some(h) if h.id == caller && lock.ready => None,
        Some(h) if h.id == caller => Some(format!("lock {name} for {doc} is yours but its grant is not ready yet; end your turn and its grant message wakes you")),
        Some(h) => Some(format!("{doc} is being edited by {} (lock {name}). One writer at a time: read only for now and say so, or lock_acquire {name} to queue and end your turn; its grant wakes you when the document is yours.", if h.thread.is_empty() { h.id.as_str() } else { h.thread.as_str() })),
        None => Some(format!("writes to {doc} hold its lock first (one writer at a time): lock_acquire {name}, edit and save; it is released when your turn ends.")),
    }
}

/// Idle holders of document locks, first seen idle at: freed after IDLE_GRACE (locks.rs's tick).
static IDLE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, u64>>> = std::sync::OnceLock::new();
pub const IDLE_GRACE: u64 = 45;
/// A document lock's holder seen idle (true) or busy (false) at `now`: true once it has been idle
/// for IDLE_GRACE, so a grant that has not started its turn yet keeps the document.
pub fn idle_long(name: &str, holder: &str, idle: bool, now: u64) -> bool {
    let m = IDLE.get_or_init(Default::default); let mut m = m.lock().unwrap(); let k = format!("{name}\0{holder}");
    if !idle { m.remove(&k); return false }
    let since = *m.entry(k.clone()).or_insert(now);
    if now >= since + IDLE_GRACE { m.remove(&k); true } else { false }
}

/// Tests: the holder was first seen idle at `at`.
#[cfg(test)] pub fn idle_seen(name: &str, holder: &str, at: u64) { IDLE.get_or_init(Default::default).lock().unwrap().insert(format!("{name}\0{holder}"), at); }
/// The design files changed in the checkout at `top` (git status, porcelain -z): repo-relative.
async fn dirty_design(top: &str) -> Result<Vec<String>, String> {
    use crate::files::git;
    let r = git::run_ro(top, &["status", "--porcelain", "-z", "--untracked-files=all", "--no-renames"]).await;
    if r.code != 0 { return Err(format!("git status: {}", r.out.trim())) }
    Ok(r.out.split('\0').filter(|e| e.len() > 3).map(|e| e[3..].to_string()).filter(|p| is_design(p)).collect())
}
/// What a commit to main did: the commit (short) and its files, and whether it was pushed.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Recorded { pub commit: String, pub files: Vec<String>, pub pushed: String, pub new_boards: u64 }
/// The board families (`<dir>/<name>` of a `.kicad_pro`/`.kicad_pcb`) among changed files, each with whether the last commit had any file of
/// it and whether any is on disk now.
pub type Family = (String, bool, bool);
/// Usage: how many board families a design commit created: families the previous commit had none of and that are on disk, less the families
/// it removed (a rename is a removal and an addition: it creates nothing). A repository's first commit is a baseline: none.
pub fn new_board_families(fams: &[Family], had_head: bool) -> u64 {
    if !had_head { return 0 }
    let added = fams.iter().filter(|(_, before, now)| !before && *now).count() as u64;
    let removed = fams.iter().filter(|(_, before, now)| *before && !now).count() as u64;
    added.saturating_sub(removed)
}
/// Commit the design files changed in the main checkout of `root` straight onto its branch, as
/// one commit, then push when the branch has an upstream. `skip`: documents another thread
/// holds the lock of (their files stay for that thread's own commit). Only the main checkout
/// (never a worktree), only on a branch, only design files: nothing else staged there is swept in.
/// Err: what stopped it. Ok with no commit: nothing to record.
pub async fn commit_design(root: &str, msg: &str, skip: &[String]) -> Result<Recorded, String> {
    use crate::files::git;
    if crate::copy::on() { return Err("a hub on a copy records no commits".into()) }
    if split_worktree(root).is_some() { return Err("that folder is a worktree, not the main checkout".into()) }
    let r = git::run_ro(root, &["rev-parse", "--show-toplevel"]).await;
    if r.code != 0 { return Err("the project is not in a git repository".into()) }
    let top = r.out.trim().to_string();
    let r = git::run_ro(&top, &["symbolic-ref", "--short", "-q", "HEAD"]).await;
    if r.code != 0 || r.out.trim().is_empty() { return Err("the main checkout is not on a branch".into()) }
    let branch = r.out.trim().to_string();
    let skipped = |p: &str| { let abs = format!("{top}/{p}"); skip.iter().any(|d| abs == *d || abs.starts_with(&format!("{d}.")) ) };
    let files: Vec<String> = dirty_design(&top).await?.into_iter().filter(|p| !skipped(p)).collect();
    if files.is_empty() { return Ok(Recorded::default()) }
    // the board families this commit adds or removes, seen before it is made (usage counting: settings/telemetry_usage.rs)
    let had_head = git::run_ro(&top, &["rev-parse", "-q", "--verify", "HEAD"]).await.code == 0;
    let mut stems: Vec<String> = files.iter().filter_map(|p| p.strip_suffix(".kicad_pro").or_else(|| p.strip_suffix(".kicad_pcb")).map(str::to_string)).collect(); stems.sort(); stems.dedup();
    let mut fams: Vec<Family> = vec![];
    for st in stems {
        let mut before = false; for ext in [".kicad_pro", ".kicad_pcb"] { if had_head && git::run_ro(&top, &["cat-file", "-e", &format!("HEAD:{st}{ext}")]).await.code == 0 { before = true } }
        let now = [".kicad_pro", ".kicad_pcb"].iter().any(|ext| std::path::Path::new(&format!("{top}/{st}{ext}")).is_file());
        fams.push((st, before, now));
    }
    let new_boards = new_board_families(&fams, had_head);
    let mut add = vec!["add", "-A", "--"]; add.extend(files.iter().map(String::as_str));
    let r = git::run(&top, &add).await;
    if r.code != 0 { return Err(format!("git add: {}", r.out.trim())) }
    let mut ci = vec!["commit", "--only", "-q", "--no-verify", "-m", msg, "--"]; ci.extend(files.iter().map(String::as_str));
    let r = git::run(&top, &ci).await;
    if r.code != 0 { return Err(format!("git commit: {}", r.out.trim())) }
    let commit = git::run_ro(&top, &["rev-parse", "--short", "HEAD"]).await.out.trim().to_string();
    let up = git::run_ro(&top, &["rev-parse", "--abbrev-ref", "-q", &format!("{branch}@{{upstream}}")]).await;
    let pushed = if up.code == 0 && !up.out.trim().is_empty() {
        let (remote, rb) = up.out.trim().split_once('/').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or_default();
        let p = git::run(&top, &["push", "-q", &remote, &format!("{branch}:{rb}")]).await;
        if p.code == 0 { up.out.trim().to_string() } else { format!("not pushed: {}", p.out.trim().lines().last().unwrap_or("")) }
    } else { String::new() };
    Ok(Recorded { commit, files, pushed, new_boards })
}

/// A hardware thread's code worktree carries no design files: each tracked one is marked
/// skip-worktree in that worktree's own index and removed from its folder, so a board is never
/// edited there by mistake (the main checkout has it; nothing in the repository's config changes).
/// Answers how many were taken out.
pub async fn strip_design(wt: &str) -> Result<usize, String> {
    use crate::files::git;
    if split_worktree(wt).is_none() { return Err("not a thread worktree".into()) }
    let r = git::run_ro(wt, &["ls-files", "-z"]).await;
    if r.code != 0 { return Err(format!("git ls-files: {}", r.out.trim())) }
    let ds: Vec<&str> = r.out.split('\0').filter(|p| !p.is_empty() && is_design(p)).collect();
    for chunk in ds.chunks(200) {
        let mut a = vec!["update-index", "--skip-worktree", "--"]; a.extend(chunk);
        let r = git::run(wt, &a).await; if r.code != 0 { return Err(format!("git update-index: {}", r.out.trim())) }
    }
    for p in &ds { let _ = std::fs::remove_file(format!("{wt}/{p}")); }
    for p in &ds { let mut d = std::path::Path::new(wt).join(p); while d.pop() && d != std::path::Path::new(wt) { if std::fs::remove_dir(&d).is_err() { break } } }
    Ok(ds.len())
}

/// An older hardware worktree (made before design files lived on main): its design files that
/// differ from the main checkout's, repo-relative. Empty: nothing to bring back.
pub fn worktree_changes(main: &str, wt: &str) -> Vec<String> {
    fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path(); let Ok(ft) = e.file_type() else { continue };
            let n = e.file_name().to_string_lossy().into_owned();
            if ft.is_dir() { if !n.starts_with('.') && n != "node_modules" && n != "target" && !n.ends_with("-backups") { walk(base, &p, out) } }
            else if ft.is_file() { if let Ok(rel) = p.strip_prefix(base) { let rel = rel.to_string_lossy().replace('\\', "/"); if is_design(&rel) { out.push(rel) } } }
        }
    }
    let mut all = vec![]; walk(std::path::Path::new(wt), std::path::Path::new(wt), &mut all); all.sort();
    all.into_iter().filter(|rel| std::fs::read(format!("{wt}/{rel}")).ok() != std::fs::read(format!("{main}/{rel}")).ok()).collect()
}
/// Bring an older worktree's design changes back to the main checkout: main's own copies of those
/// files go to `<main>/.backplane/backup/<stamp>-<thread>/` first (and the worktree's, as they
/// were, beside them), then the worktree's files are copied in. Answers the backup folder and the files.
pub fn bring_back(main: &str, wt: &str, thread: &str, stamp: &str) -> Result<(String, Vec<String>), String> {
    let files = worktree_changes(main, wt);
    if files.is_empty() { return Ok((String::new(), files)) }
    let bk = format!("{main}/.backplane/backup/{stamp}-{}", thread.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>());
    let cp = |from: &str, to: &str| -> Result<(), String> { if let Some(d) = std::path::Path::new(to).parent() { std::fs::create_dir_all(d).map_err(|e| e.to_string())? } std::fs::copy(from, to).map(|_| ()).map_err(|e| format!("{to}: {e}")) };
    for rel in &files {
        let m = format!("{main}/{rel}");
        if std::path::Path::new(&m).is_file() { cp(&m, &format!("{bk}/main/{rel}"))? }
        cp(&format!("{wt}/{rel}"), &format!("{bk}/worktree/{rel}"))?;
    }
    for rel in &files { cp(&format!("{wt}/{rel}"), &format!("{main}/{rel}"))? }
    Ok((bk, files))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_new_board_family_counts_once_a_rename_or_baseline_counts_nothing() {
        let f = |n: &str, b: bool, now: bool| (n.to_string(), b, now);
        assert_eq!(new_board_families(&[f("boards/a/a", false, true)], true), 1, "a .kicad_pro/.kicad_pcb pair is one family");
        assert_eq!(new_board_families(&[f("boards/a/a", true, true)], true), 0, "an edit, or a later counterpart");
        assert_eq!(new_board_families(&[f("old/x", true, false), f("new/x", false, true)], true), 0, "a rename");
        assert_eq!(new_board_families(&[f("boards/a/a", false, true)], false), 0, "a repository's first commit is a baseline");
    }
    use crate::agents::lockrec::{Lock, Party};
    fn tmp(n: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("kc-{n}")) }
    fn touch(p: std::path::PathBuf, t: &str) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, t).unwrap() }
    /// A fixture project: boards/main/ in the main folder, .backplane.json naming it, and a
    /// worktree twin, exactly the shape a thread sees.
    fn fixture(n: &str) -> (crate::plat::tmp::TempDir, String, String) {
        let r = tmp(n);
        for base in ["", ".backplane/worktrees/lane/"] {
            touch(r.join(format!("{base}boards/main/main.kicad_pro")), "{}");
            touch(r.join(format!("{base}boards/main/main.kicad_pcb")), "(pcb)");
            touch(r.join(format!("{base}boards/main/main.kicad_sch")), "(sch)");
        }
        touch(r.join("boards/main/sub.kicad_sch"), "(sub)");
        std::fs::write(r.join(".backplane.json"), r#"{"project":"boards/main/main.kicad_pro"}"#).unwrap();
        let main = r.to_str().unwrap().to_string();
        (r, main.clone(), format!("{main}/.backplane/worktrees/lane"))
    }

    #[test] fn subfolder_boards_project_is_canonical_from_the_worktree() {
        let (_r, main, wt) = fixture("sub");
        let f = for_cwd(&wt);
        assert_eq!(f.pro, format!("{main}/boards/main/main.kicad_pro"));
        assert_eq!(f.pcb, format!("{main}/boards/main/main.kicad_pcb"));
        assert_eq!(f.sch, format!("{main}/boards/main/main.kicad_sch"));
        // and a subfolder project named only by discovery, board+schematic beside the .kicad_pro
        let r2 = tmp("disc"); touch(r2.join("boards/aux/aux.kicad_pro"), "{}"); touch(r2.join("boards/aux/aux.kicad_pcb"), "x");
        let f2 = for_cwd(r2.to_str().unwrap());
        assert!(f2.pro.ends_with("boards/aux/aux.kicad_pro") && f2.pcb.ends_with("boards/aux/aux.kicad_pcb"), "{f2:?}");
    }

    #[test] fn a_worktree_named_canonical_path_routes_to_main_for_both_routes() {
        let (_r, main, wt) = fixture("route");
        // the IPC route gets an absolute path
        assert_eq!(route(&format!("{wt}/boards/main/main.kicad_pcb")), format!("{main}/boards/main/main.kicad_pcb"));
        assert_eq!(route(&format!("{wt}/boards/main/main.kicad_pro")), format!("{main}/boards/main/main.kicad_pro"));
        // a referenced sheet in the canonical folder rides along; the companion metadata too
        assert_eq!(route(&format!("{wt}/boards/main/sub.kicad_sch")), format!("{main}/boards/main/sub.kicad_sch"));
        assert_eq!(route(&format!("{wt}/boards/main/main.kicad_pcb.backplane.json")), format!("{main}/boards/main/main.kicad_pcb.backplane.json"));
        // already-main and non-worktree paths are untouched
        assert_eq!(route(&format!("{main}/boards/main/main.kicad_pcb")), format!("{main}/boards/main/main.kicad_pcb"));
        // any design file named in a worktree is main's (a board experiment is a labelled copy on main too);
        // code is the worktree's own
        assert_eq!(route(&format!("{wt}/experiment/copy.kicad_pcb")), format!("{main}/experiment/copy.kicad_pcb"));
        assert_eq!(route(&format!("{wt}/mech/case.py")), format!("{main}/mech/case.py"));
        assert_eq!(route(&format!("{wt}/firmware/src/main.c")), format!("{wt}/firmware/src/main.c"));
        // the direct-file route gets root+relative and must land in the same place
        assert_eq!(direct(&wt, "boards/main/main.kicad_pcb"), (main.clone(), "boards/main/main.kicad_pcb".to_string()));
        assert_eq!(direct(&wt, "src/main.rs"), (wt.clone(), "src/main.rs".to_string()));
        assert!(is_canonical_rel(&wt, "boards/main/main.kicad_sch"));
        assert!(!is_canonical_rel(&wt, "firmware/src/main.c"));
    }

    #[test] fn a_second_writer_is_told_who_holds_the_document() {
        let doc = "/proj/boards/main/main.kicad_pcb";
        let name = doc_lock(doc);
        assert!(crate::agents::lockrec::name_ok(&name) && name.starts_with("kicad-main-pcb-") && is_doc_lock(&name), "{name}");
        assert_ne!(doc_lock("/proj/boards/main/main.kicad_sch"), name, "a board and its schematic are two documents, two locks");
        assert_eq!(doc_lock("/p/boards/main/main.kicad_pcb"), "kicad-main-pcb-9942661e", "test/e2e/kicad_main.py names it the same way");
        let mut l = Lock::default();
        l.acquire(Party { id: "t-holder".into(), kind: "thread".into(), thread: "t-holder".into(), ..Default::default() });
        l.ready = true;
        let why = write_blocked(&l, "t-waiter", &name, doc).expect("a second writer is blocked");
        assert!(why.contains("t-holder") && why.contains(&name) && why.contains("lock_acquire") && why.contains("read only"), "{why}");
        assert!(write_blocked(&l, "t-holder", &name, doc).is_none());
        let free = write_blocked(&Lock::default(), "t-waiter", &name, doc).expect("writes hold the lock first");
        assert!(free.contains(&name) && free.contains("lock_acquire"), "{free}");
    }

    #[test] fn design_files_are_told_from_code_by_type() {
        for d in ["boards/main/main.kicad_pcb", "a.kicad_sch", "x/y.kicad_pro", "lib/my.kicad_sym", "lib/my.pretty/R_0402.kicad_mod", "fp-lib-table", "sym-lib-table",
                  "mech/case.py", "mech/case.step", "mfg/gerbers/top.gbr", "boards/main/main.fab.json", "boards/main/main.kicad_pcb.backplane.json", "models/usb.3dshapes/usb.wrl", "enclosure.FCStd"] {
            assert!(is_design(d), "{d} is design")
        }
        for c in ["firmware/src/main.c", "src/lib.rs", "Cargo.toml", "CMakeLists.txt", "zephyr/prj.conf", ".backplane.json", "README.md", "boards/main/main-backups/x.kicad_pcb", "boards/main/~main.kicad_pcb.lck", "fp-info-cache", "boards/_autosave-main.kicad_sch", ".backplane/worktrees/x/a.kicad_pcb"] {
            assert!(!is_design(c), "{c} is not design")
        }
    }

    #[test] fn an_idle_holder_keeps_its_document_for_a_grace_then_loses_it() {
        let n = "kicad-t-pcb-00000001";
        assert!(!idle_long(n, "t1", true, 100), "first seen idle");
        assert!(!idle_long(n, "t1", true, 100 + IDLE_GRACE - 1));
        assert!(!idle_long(n, "t1", false, 100 + IDLE_GRACE), "busy again: the clock starts over");
        assert!(!idle_long(n, "t1", true, 200));
        assert!(idle_long(n, "t1", true, 200 + IDLE_GRACE), "idle for the whole grace");
    }

    fn git(dir: &str, a: &[&str]) -> String {
        let o = std::process::Command::new("git").args(a).current_dir(dir).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap();
        assert!(o.status.success(), "{a:?}: {}", String::from_utf8_lossy(&o.stderr)); String::from_utf8_lossy(&o.stdout).trim().to_string()
    }
    /// A mixed repository: a board in boards/main/, firmware beside it, mech/ for the case.
    fn mixed(n: &str) -> (crate::plat::tmp::TempDir, String) {
        let r = tmp(n); let main = r.to_str().unwrap().to_string();
        for (f, t) in [("boards/main/main.kicad_pro", "{}"), ("boards/main/main.kicad_pcb", "(pcb)"), ("boards/main/main.kicad_sch", "(sch)"), ("mech/case.py", "# case"), ("firmware/src/main.c", "int main(){}"), ("firmware/CMakeLists.txt", "project(fw)")] { touch(r.join(f), t) }
        std::fs::write(r.join(".backplane.json"), r#"{"project":"boards/main/main.kicad_pro"}"#).unwrap();
        git(&main, &["init", "-q", "-b", "main"]); git(&main, &["config", "user.name", "t"]); git(&main, &["config", "user.email", "t@t"]);
        std::fs::write(r.join(".git/info/exclude"), "/.backplane/\n").unwrap();
        git(&main, &["add", "-A"]); git(&main, &["commit", "-qm", "base"]);
        (r, main)
    }

    #[tokio::test] async fn a_mixed_repo_keeps_the_board_on_main_and_the_firmware_in_the_worktree() {
        let (_r, main) = mixed("mixed");
        let wt = format!("{main}/.backplane/worktrees/lane");
        git(&main, &["worktree", "add", "-q", "-b", "backplane/lane", &wt]);
        // the code worktree carries no design files, and git there sees nothing changed
        assert_eq!(strip_design(&wt).await.unwrap(), 4);
        assert!(!std::path::Path::new(&format!("{wt}/boards/main/main.kicad_pcb")).exists() && !std::path::Path::new(&format!("{wt}/mech/case.py")).exists());
        assert!(std::path::Path::new(&format!("{wt}/firmware/src/main.c")).exists());
        assert_eq!(git(&wt, &["status", "--porcelain"]), "", "the stripped worktree is clean");
        // the thread edits the board in the main checkout and the firmware in its worktree
        std::fs::write(format!("{main}/boards/main/main.kicad_pcb"), "(pcb v2)").unwrap();
        std::fs::write(format!("{main}/boards/main/main.kicad_pcb.backplane.json"), "{}").unwrap();
        std::fs::write(format!("{main}/mech/case.step"), "ISO-10303").unwrap();
        std::fs::write(format!("{wt}/firmware/src/main.c"), "int main(){return 1;}").unwrap();
        // a stray code edit in the main checkout is never swept into the design commit
        std::fs::write(format!("{main}/firmware/CMakeLists.txt"), "project(fw2)").unwrap();
        let r = commit_design(&main, "design: test (t1)", &[]).await.unwrap();
        assert_eq!(r.files, vec!["boards/main/main.kicad_pcb", "boards/main/main.kicad_pcb.backplane.json", "mech/case.step"]);
        assert_eq!(r.pushed, "", "no upstream: nothing pushed");
        assert_eq!(git(&main, &["show", "--name-only", "--format=%s", "HEAD"]), "design: test (t1)\n\nboards/main/main.kicad_pcb\nboards/main/main.kicad_pcb.backplane.json\nmech/case.step");
        assert_eq!(git(&main, &["status", "--porcelain"]), "M firmware/CMakeLists.txt");
        // the code lands the normal way: a commit on the thread's branch carries no board
        git(&wt, &["commit", "-qam", "fw"]);
        assert_eq!(git(&main, &["diff", "--name-only", "main...backplane/lane"]), "firmware/src/main.c");
        git(&main, &["stash", "push", "-q", "-m", "bp-test", "--", "firmware/CMakeLists.txt"]);
        git(&main, &["merge", "-q", "--no-edit", "backplane/lane"]);
        assert_eq!(std::fs::read_to_string(format!("{main}/boards/main/main.kicad_pcb")).unwrap(), "(pcb v2)", "the merge kept main's board");
        assert_eq!(std::fs::read_to_string(format!("{main}/firmware/src/main.c")).unwrap(), "int main(){return 1;}");
        // nothing more to record
        assert_eq!(commit_design(&main, "x", &[]).await.unwrap(), Recorded::default());
        // a design path the thread names from its worktree is main's
        assert_eq!(route(&format!("{wt}/boards/main/main.kicad_pcb")), format!("{main}/boards/main/main.kicad_pcb"));
        assert_eq!(design_root(&wt), main);
    }

    #[tokio::test] async fn a_document_another_thread_holds_is_left_for_it_and_main_pushes_to_its_upstream() {
        let (_r, main) = mixed("push");
        let bare = crate::plat::tmp::uncreated("push-remote");
        git(&main, &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
        git(&main, &["remote", "add", "origin", bare.to_str().unwrap()]); git(&main, &["push", "-q", "-u", "origin", "main"]);
        std::fs::write(format!("{main}/boards/main/main.kicad_pcb"), "(pcb other thread)").unwrap();
        std::fs::write(format!("{main}/boards/main/main.kicad_sch"), "(sch mine)").unwrap();
        let rec = commit_design(&main, "design: mine (t2)", &[format!("{main}/boards/main/main.kicad_pcb")]).await.unwrap();
        assert_eq!(rec.files, vec!["boards/main/main.kicad_sch"]);
        assert_eq!(rec.pushed, "origin/main");
        assert_eq!(git(bare.to_str().unwrap(), &["log", "-1", "--format=%s", "main"]), "design: mine (t2)");
        assert_eq!(git(&main, &["status", "--porcelain"]), "M boards/main/main.kicad_pcb", "the held board waits for its writer");
    }

    #[tokio::test] async fn an_older_worktree_brings_its_board_back_with_a_backup() {
        let (_r, main) = mixed("back");
        let wt = format!("{main}/.backplane/worktrees/old");
        git(&main, &["worktree", "add", "-q", "-b", "backplane/old", &wt]);
        // the older way: the thread edited its worktree's board
        std::fs::write(format!("{wt}/boards/main/main.kicad_pcb"), "(pcb from the worktree)").unwrap();
        std::fs::write(format!("{wt}/firmware/src/main.c"), "code stays").unwrap();
        assert_eq!(worktree_changes(&main, &wt), vec!["boards/main/main.kicad_pcb"]);
        let (bk, files) = bring_back(&main, &wt, "t-old", "20261110-120000").unwrap();
        assert_eq!(files, vec!["boards/main/main.kicad_pcb"]);
        assert_eq!(std::fs::read_to_string(format!("{bk}/main/boards/main/main.kicad_pcb")).unwrap(), "(pcb)", "main's copy is backed up first");
        assert_eq!(std::fs::read_to_string(format!("{bk}/worktree/boards/main/main.kicad_pcb")).unwrap(), "(pcb from the worktree)");
        assert_eq!(std::fs::read_to_string(format!("{main}/boards/main/main.kicad_pcb")).unwrap(), "(pcb from the worktree)");
        assert_eq!(std::fs::read_to_string(format!("{main}/firmware/src/main.c")).unwrap(), "int main(){}", "code is not moved");
        assert!(worktree_changes(&main, &wt).is_empty());
        assert_eq!(git(&main, &["status", "--porcelain"]), "M boards/main/main.kicad_pcb", "the backup sits under .backplane, out of git's sight");
    }
}

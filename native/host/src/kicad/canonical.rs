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
const DESIGN_EXT: &[&str] = &[".kicad_pcb", ".kicad_sch", ".kicad_pro", ".kicad_prl", ".kicad_sym", ".kicad_mod", ".kicad_dru", ".kicad_wks", ".step", ".stp", ".wrl", ".fcstd",
    ".prjpcb", ".pcbdoc", ".schdoc", ".schlib", ".pcblib", ".intlib", ".outjob", ".prjpcbstructure"]; // Altium's project, documents and libraries
fn is_scratch(low: &str) -> bool { (low.starts_with("history/") || low.contains("/history/")) && (low.ends_with(".zip") || low.contains(".schdoc") || low.contains(".pcbdoc") || low.contains(".prjpcb")) || low.contains("__previews/") || low.ends_with(".schdocpreview") || low.ends_with(".pcbdocpreview") || low.contains("-backups/") || low.ends_with(".lck") || low.ends_with("fp-info-cache") || low.contains("_autosave-") || low.contains("/.backplane/") || low.starts_with(".backplane/") }
/// The documents one writer at a time holds: boards and schematics.
pub fn is_document(p: &str) -> bool { p.ends_with(".kicad_pcb") || p.ends_with(".kicad_sch") || { let l = p.to_ascii_lowercase(); l.ends_with(".pcbdoc") || l.ends_with(".schdoc") } }

/// Hardware: a project with KiCad files (a `.kicad_pro` up to four folders down, or a board or
/// schematic in its root). Such a project's threads work in the main checkout.
pub fn hardware(root: &str) -> bool {
    !project::found(std::path::Path::new(root)).is_empty() || !project::altium_found(std::path::Path::new(root)).is_empty()
        || std::fs::read_dir(root).map(|d| d.flatten().any(|e| { let n = e.file_name().to_string_lossy().into_owned(); n.ends_with(".kicad_pcb") || n.ends_with(".kicad_sch") || is_document(&n) })).unwrap_or(false)
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

/// One file, however it is named: the path with every symlink resolved (macOS's `/var` is `/private/var`; a symlinked home, a
/// project folder linked elsewhere), or, for a file not there yet, its folder resolved and its name as given; else the path as given.
/// Locks and "the same document" compare this, so two spellings of one board are never two documents.
pub fn same_file(p: &str) -> String {
    // Windows: canonicalize answers `\\?\` paths the hub's keys do not use; its paths are taken as given
    if cfg!(windows) { return p.to_string() }
    let path = std::path::Path::new(p);
    if let Ok(c) = path.canonicalize() { return c.to_string_lossy().into_owned() }
    match (path.parent(), path.file_name()) {
        (Some(d), Some(n)) if !d.as_os_str().is_empty() => d.canonicalize().map(|cd| cd.join(n).to_string_lossy().into_owned()).unwrap_or_else(|_| p.to_string()),
        _ => p.to_string(),
    }
}

/// The named lock of one board or schematic: `kicad-<stem>-<pcb|sch>-<digest8>` (64 at most), of the file itself (`same_file`):
/// two spellings of one board are one lock, so they can never let two writers in.
pub fn doc_lock(doc: &str) -> String {
    let doc = &same_file(doc);
    let file = doc.rsplit('/').next().unwrap_or("board");
    let (stem, kind) = file.rsplit_once('.').map_or((file, "doc"), |(s, x)| (s, x.strip_prefix("kicad_").unwrap_or(x)));
    // an Altium board or sheet takes the same lock names as KiCad's (one writer per document either way)
    let kind = match kind.to_ascii_lowercase().as_str() { "pcbdoc" => "pcb", "schdoc" => "sch", _ => kind };
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
pub struct Recorded { pub commit: String, pub files: Vec<String>, pub pushed: String, pub new_boards: u64, /** the ref it went on, "" for the branch (`off_branch`) */ pub reference: String }
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
/// A design commit's words about its change: a one-line subject and a line per file for the body.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Described { pub subject: String, pub detail: Vec<String> }
/// A design commit's message: the subject, the per-file detail when the subject could not hold it all, then `body` (the thread it came
/// from, with its trailers).
pub fn full_message(d: &Described, body: &str) -> String {
    let mut m = d.subject.clone();
    if !d.detail.is_empty() && d.subject != format!("design: {}", d.detail.join("; ")) { m.push_str("\n\n"); m.push_str(&d.detail.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")) }
    let body = body.trim(); if !body.is_empty() { m.push_str("\n\n"); m.push_str(body) }
    m
}
/// What changed between two versions of a board or schematic, in words: "12 tracks added, 1 footprint changed, 2 vias removed". Items
/// (files/snapshot.rs `items`, the scanner #1163's `count_items` counts with) are matched by their own uuid, so a moved track or footprint
/// is "changed", not one removed and one added; items with none match by their text. "" when no item differs (a setting, the title block,
/// the file's format). Each version is scanned once.
pub fn item_changes(old: &str, new: &str, schematic: bool) -> String {
    use std::collections::{BTreeMap, HashMap};
    let (a, b) = (crate::files::snapshot::items(old, schematic), crate::files::snapshot::items(new, schematic));
    let mut n: BTreeMap<&str, [usize; 3]> = BTreeMap::new(); // label -> added, changed, removed
    let by_id = |v: &[(&'static str, &str, &str)]| -> HashMap<String, (&'static str, String)> { v.iter().filter(|x| !x.2.is_empty()).map(|x| (x.2.to_string(), (x.0, x.1.to_string()))).collect() };
    let (ia, ib) = (by_id(&a), by_id(&b));
    for (id, (l, t)) in &ib { match ia.get(id) { None => n.entry(l).or_default()[0] += 1, Some((_, t0)) if t0 != t => n.entry(l).or_default()[1] += 1, _ => {} } }
    for (id, (l, _)) in &ia { if !ib.contains_key(id) { n.entry(l).or_default()[2] += 1 } }
    let mut bag: HashMap<(&str, &str), i64> = HashMap::new();
    for x in a.iter().filter(|x| x.2.is_empty()) { *bag.entry((x.0, x.1)).or_default() -= 1 }
    for x in b.iter().filter(|x| x.2.is_empty()) { *bag.entry((x.0, x.1)).or_default() += 1 }
    for ((l, _), d) in bag { if d > 0 { n.entry(l).or_default()[0] += d as usize } else if d < 0 { n.entry(l).or_default()[2] += (-d) as usize } }
    let plural = |l: &str, k: usize| if k == 1 { l.to_string() } else if l.ends_with('s') { format!("{l}es") } else { format!("{l}s") };
    let order = ["footprint", "symbol", "track", "wire", "via", "zone", "label", "junction", "no-connect", "bus", "sheet", "text", "drawing", "dimension", "group", "image"];
    let mut words = vec![];
    for (i, verb) in ["added", "changed", "removed"].iter().enumerate() {
        let mut parts: Vec<(usize, String)> = n.iter().filter(|(_, c)| c[i] > 0).map(|(l, c)| (order.iter().position(|o| o == l).unwrap_or(99), format!("{} {}", c[i], plural(l, c[i])))).collect();
        parts.sort();
        if !parts.is_empty() { words.push(format!("{} {verb}", parts.into_iter().map(|p| p.1).collect::<Vec<_>>().join(", "))) }
    }
    words.join(", ")
}
/// A design commit's subject is one line of at most this many characters; the detail goes in the body.
pub const SUBJECT_MAX: usize = 72;
/// The most board and schematic text (both versions of every one, together) the item summary reads; past it, file names only.
pub const SUMMARY_MAX_BYTES: u64 = 64 << 20;
/// The longest the item summary may take; past it the commit goes ahead with file names only.
pub const SUMMARY_MAX_MS: u64 = 1500;
/// The subject and detail from each file's (rank, path, line), most telling first (0: a document's items, 1: a document named only,
/// 2: any other file): every line when they fit `SUBJECT_MAX`, else the first and a count, else as many names as fit and a count.
fn compose(mut parts: Vec<(u8, String, String)>) -> Described {
    parts.sort_by_key(|p| p.0);
    let detail: Vec<String> = parts.iter().map(|p| p.2.clone()).collect();
    let fits = |s: &str| s.chars().count() <= SUBJECT_MAX;
    let more = |k: usize| if k == 0 { String::new() } else { format!(" and {k} more file{}", if k == 1 { "" } else { "s" }) };
    let all = format!("design: {}", detail.join("; "));
    let subject = if detail.is_empty() { "design: files updated".to_string() }
        else if fits(&all) { all }
        else if fits(&format!("design: {};{}", detail[0], more(detail.len() - 1))) { format!("design: {};{}", detail[0], more(detail.len() - 1)) }
        else {
            let mut s = String::from("design: "); let mut k = 0;
            for (i, (_, path, _)) in parts.iter().enumerate() {
                let n = path.rsplit('/').next().unwrap_or(path);
                let next = format!("{s}{}{n}", if i == 0 { "" } else { ", " });
                if i > 0 && !fits(&format!("{next}{}", more(parts.len() - i - 1))) { break }
                s = next; k = i + 1;
            }
            let s = format!("{s}{}", more(parts.len() - k));
            if fits(&s) { s } else { s.chars().take(SUBJECT_MAX - 1).collect::<String>() + "…" }
        };
    Described { subject, detail }
}
/// A design commit's words: what changed in each file since `base` ("" for none: every file is new). A board or schematic says its
/// items ("main.kicad_pcb: 12 tracks added, 1 footprint changed"); any other file whether it was added, changed or removed. Bounded: past
/// `SUMMARY_MAX_BYTES` of board and schematic text, past `SUMMARY_MAX_MS`, or on any error, the documents too are named only ("changed"),
/// so the summary never fails or holds up the commit. Never the thread's title: that is the body's.
pub async fn describe(top: &str, base: &str, files: &[String]) -> Described {
    use crate::files::git;
    let doc = |f: &str| f.ends_with(".kicad_pcb") || f.ends_with(".kicad_sch");
    let gd = if base.is_empty() { String::new() } else { git::run_ro(top, &["rev-parse", "--absolute-git-dir"]).await.out.trim().to_string() };
    let mut plain: Vec<(u8, String, String)> = vec![]; // (rank, name, line)
    let mut read: Vec<(String, bool, bool)> = vec![]; // documents to summarize: (path, schematic, in base)
    let mut bytes = 0u64;
    for f in files {
        let name = f.rsplit('/').next().unwrap_or(f).to_string();
        let now_len = std::fs::metadata(std::path::Path::new(top).join(f)).ok().filter(|m| m.is_file()).map(|m| m.len());
        let in_base = !base.is_empty() && git::run_ro(top, &["cat-file", "-e", &format!("{base}:{f}")]).await.code == 0;
        let w = match (in_base, now_len) { (false, None) => continue, (false, Some(_)) => "added", (true, None) => "removed", (true, Some(_)) => "changed" };
        plain.push((if doc(f) { 1 } else { 2 }, f.clone(), format!("{name} {w}")));
        if let (true, Some(n)) = (doc(f), now_len) {
            let old = if in_base { git::run_ro(top, &["cat-file", "-s", &format!("{base}:{f}")]).await.out.trim().parse::<u64>().unwrap_or(u64::MAX) } else { 0 };
            bytes = bytes.saturating_add(old).saturating_add(n);
            read.push((f.clone(), f.ends_with(".kicad_sch"), in_base));
        }
    }
    if read.is_empty() || bytes > SUMMARY_MAX_BYTES { return compose(plain) }
    let mut texts = vec![];
    for (f, sch, in_base) in read {
        let Ok(now) = std::fs::read(std::path::Path::new(top).join(&f)) else { return compose(plain) };
        let was = if in_base { match git::show(&gd, &format!("{base}:{f}")).await { Some(x) => Some(x), None => return compose(plain) } } else { None };
        let name = f.rsplit('/').next().unwrap_or(&f).to_string(); texts.push((f, name, sch, was, now));
    }
    // each version scanned once, off the async threads, within the time bound; late or failed: names only, and the commit goes ahead
    let work = tokio::task::spawn_blocking(move || texts.into_iter().map(|(path, name, sch, was, now)| {
        let now = String::from_utf8_lossy(&now);
        let (rank, line) = match was {
            None => { let k = crate::files::snapshot::items(&now, sch).len(); (0, format!("{name} added ({k} item{})", if k == 1 { "" } else { "s" })) }
            Some(o) => { let w = item_changes(&String::from_utf8_lossy(&o), &now, sch); if w.is_empty() { (1, format!("{name} edited")) } else { (0, format!("{name}: {w}")) } }
        };
        (path, rank, line)
    }).collect::<Vec<_>>());
    let Ok(Ok(said)) = tokio::time::timeout(std::time::Duration::from_millis(SUMMARY_MAX_MS), work).await else { return compose(plain) };
    compose(plain.into_iter().map(|(k, n, l)| match said.iter().find(|(sn, _, _)| *sn == n) { Some((_, r, sl)) => (*r, n, sl.clone()), None => (k, n, l) }).collect())
}
/// Why git refused the design commit, for the thread's line. A machine where git was never told a name and email (many
/// people who draw boards have never used git) gets the one thing to do first, since the line is cut at the pane's width;
/// any other refusal stays as git said it.
pub fn commit_refused(out: &str) -> String {
    if crate::files::git::no_identity(out).is_some() { return "git does not know your name and email on this machine yet. Set them once (git config --global user.name \"Your Name\" and git config --global user.email you@example.com); the next turn or save commits them.".into() }
    format!("git commit: {}", out.trim())
}
/// Commit the design files changed in the main checkout of `root` straight onto its branch, as
/// one commit, then, when `push` (the setting `git.auto_push`, off by default: settings/autopush.rs)
/// and the branch has an upstream, push it. `skip`: documents another thread
/// holds the lock of (their files stay for that thread's own commit). Only the main checkout
/// (never a worktree), only on a branch, only design files: nothing else staged there is swept in.
/// The subject says what changed (`describe`: "design: main.kicad_pcb: 12 tracks added, 1 footprint changed"); `body` follows it (the
/// thread that made the change). `off_branch` (`git.off_branch`, off by default: settings/offbranch.rs, #1188): the same commit goes on
/// `refs/backplane/design/<branch>` instead (files/snapshot.rs `record_on_ref`), the branch, index and work tree untouched, and is never pushed.
/// Err: what stopped it. Ok with no commit: nothing to record.
pub async fn commit_design(root: &str, body: &str, skip: &[String], push: bool, off_branch: bool) -> Result<Recorded, String> {
    use crate::files::git;
    if crate::copy::on() { return Err("a hub on a copy records no commits".into()) }
    if split_worktree(root).is_some() { return Err("that folder is a worktree, not the main checkout".into()) }
    let r = git::run_ro(root, &["rev-parse", "--show-toplevel"]).await;
    if r.code != 0 { return Err("the project is not in a git repository".into()) }
    let top = r.out.trim().to_string();
    let _l = crate::files::snapshot::repo_lock(&top).await; // one hub git write at a time on a repository: the turn snapshots (#1163) use the same index
    let r = git::run_ro(&top, &["symbolic-ref", "--short", "-q", "HEAD"]).await;
    // a detached HEAD: never committed onto (as before); kept off the branch, the change is recorded on refs/backplane/design/detached
    if (r.code != 0 || r.out.trim().is_empty()) && !off_branch { return Err("the main checkout is not on a branch".into()) }
    let branch = if r.code != 0 || r.out.trim().is_empty() { "detached".to_string() } else { r.out.trim().to_string() };
    // git names the checkout with its symlinks resolved (`--show-toplevel`): the held documents are compared as files, not spellings
    let skip: Vec<String> = skip.iter().map(|d| same_file(d)).collect();
    let skipped = |p: &str| { let abs = same_file(&format!("{top}/{p}")); skip.iter().any(|d| abs == *d || abs.starts_with(&format!("{d}.")) ) };
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
    if off_branch {
        let reference = crate::files::snapshot::design_ref(&branch);
        let (t2, b2) = (top.clone(), body.to_string());
        let rec = crate::files::snapshot::record_on_ref(&top, &reference, &files, |base, changed| async move { full_message(&describe(&t2, &base, &changed).await, &b2) }).await?;
        let Some((commit, changed)) = rec else { return Ok(Recorded::default()) };
        return Ok(Recorded { commit, files: changed, pushed: String::new(), new_boards, reference })
    }
    let base = if had_head { "HEAD".to_string() } else { String::new() };
    let msg = full_message(&describe(&top, &base, &files).await, body);
    let msg = msg.as_str();
    let mut add = vec!["add", "-A", "--"]; add.extend(files.iter().map(String::as_str));
    let r = git::run(&top, &add).await;
    if r.code != 0 { return Err(format!("git add: {}", r.out.trim())) }
    let mut ci = vec!["commit", "--only", "-q", "--no-verify", "-m", msg, "--"]; ci.extend(files.iter().map(String::as_str));
    let r = git::run(&top, &ci).await;
    if r.code != 0 { return Err(commit_refused(&r.out)) }
    let commit = git::run_ro(&top, &["rev-parse", "--short", "HEAD"]).await.out.trim().to_string();
    let pushed = if push { push_upstream(&top, &branch).await } else { String::new() };
    Ok(Recorded { commit, files, pushed, new_boards, reference: String::new() })
}
/// The upstream (`origin/main`) of `branch` in the checkout at `top`, "" for none.
async fn upstream(top: &str, branch: &str) -> String {
    let up = crate::files::git::run_ro(top, &["rev-parse", "--abbrev-ref", "-q", &format!("{branch}@{{upstream}}")]).await;
    if up.code == 0 { up.out.trim().to_string() } else { String::new() }
}
/// Push `branch` of the checkout at `top` to its upstream: the upstream pushed to, "" when it has none, `not pushed: <why>` when git refused.
pub async fn push_upstream(top: &str, branch: &str) -> String {
    let up = upstream(top, branch).await;
    if up.is_empty() { return up }
    let (remote, rb) = up.split_once('/').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or_default();
    let p = crate::files::git::run(top, &["push", "-q", &remote, &format!("{branch}:{rb}")]).await;
    if p.code == 0 { up } else { format!("not pushed: {}", p.out.trim().lines().last().unwrap_or("")) }
}
/// The main checkout of `root`: its top folder and branch, None when it is no git checkout on a branch (or a worktree).
pub async fn main_branch(root: &str) -> Option<(String, String)> {
    use crate::files::git;
    if split_worktree(root).is_some() { return None }
    let top = git::run_ro(root, &["rev-parse", "--show-toplevel"]).await; if top.code != 0 { return None }
    let top = top.out.trim().to_string();
    let b = git::run_ro(&top, &["symbolic-ref", "--short", "-q", "HEAD"]).await; if b.code != 0 || b.out.trim().is_empty() { return None }
    Some((top, b.out.trim().to_string()))
}
/// The commits on the main checkout's branch that its upstream lacks, and that upstream: None when there is no upstream (nothing to push to).
pub async fn unpushed(root: &str) -> Option<(u64, String)> {
    let (top, branch) = main_branch(root).await?;
    let up = upstream(&top, &branch).await; if up.is_empty() { return None }
    let n = crate::files::git::run_ro(&top, &["rev-list", "--count", &format!("{up}..{branch}")]).await;
    if n.code != 0 { return None }
    Some((n.out.trim().parse().unwrap_or(0), up))
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
    #[test] fn altium_files_are_design_documents() {
        for f in ["hw/Board.PcbDoc", "Top.SchDoc", "x.PrjPcb", "lib/parts.SchLib", "lib/parts.PcbLib", "out.OutJob"] { assert!(is_design(f), "{f}") }
        for f in ["History/Board.PcbDoc.zip", "hw/History/Top~1.SchDoc", "__Previews/Top.SchDocPreview"] { assert!(!is_design(f), "{f}") }
        assert!(is_document("/p/Board.PcbDoc") && is_document("/p/Top.SCHDOC") && !is_document("/p/x.PrjPcb"));
        assert!(is_doc_lock(&doc_lock("/p/Board.PcbDoc")) && is_doc_lock(&doc_lock("/p/Top.SchDoc")));
    }
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

    #[test] fn a_design_commit_git_refused_for_no_identity_says_what_to_set() {
        let said = commit_refused("Author identity unknown\n\n*** Please tell me who you are.\n\nRun\n\n  git config --global user.email \"you@example.com\"\n\nfatal: unable to auto-detect email address (got 'k@h.(none)')");
        assert!(said.starts_with("git does not know your name and email") && said.contains("git config --global user.name") && !said.contains("Author identity"), "{said}");
        assert_eq!(commit_refused("error: pathspec 'x' did not match any file(s) known to git\n"), "git commit: error: pathspec 'x' did not match any file(s) known to git");
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
        let r = commit_design(&main, "Recorded by Backplane at the end of an agent's turn, thread t1: test", &[], true, false).await.unwrap();
        assert_eq!(r.files, vec!["boards/main/main.kicad_pcb", "boards/main/main.kicad_pcb.backplane.json", "mech/case.step"]);
        assert_eq!(r.pushed, "", "no upstream: nothing pushed");
        assert_eq!(git(&main, &["show", "--name-only", "--format=%s", "HEAD"]), "design: main.kicad_pcb edited; and 2 more files\n\nboards/main/main.kicad_pcb\nboards/main/main.kicad_pcb.backplane.json\nmech/case.step");
        assert_eq!(git(&main, &["status", "--porcelain"]), "M firmware/CMakeLists.txt");
        // the code lands the normal way: a commit on the thread's branch carries no board
        git(&wt, &["commit", "-qam", "fw"]);
        assert_eq!(git(&main, &["diff", "--name-only", "main...backplane/lane"]), "firmware/src/main.c");
        git(&main, &["stash", "push", "-q", "-m", "bp-test", "--", "firmware/CMakeLists.txt"]);
        git(&main, &["merge", "-q", "--no-edit", "backplane/lane"]);
        assert_eq!(std::fs::read_to_string(format!("{main}/boards/main/main.kicad_pcb")).unwrap(), "(pcb v2)", "the merge kept main's board");
        assert_eq!(std::fs::read_to_string(format!("{main}/firmware/src/main.c")).unwrap(), "int main(){return 1;}");
        // nothing more to record
        assert_eq!(commit_design(&main, "x", &[], true, false).await.unwrap(), Recorded::default());
        // a design path the thread names from its worktree is main's
        assert_eq!(route(&format!("{wt}/boards/main/main.kicad_pcb")), format!("{main}/boards/main/main.kicad_pcb"));
        assert_eq!(design_root(&wt), main);
    }

    #[cfg(unix)] #[test] fn two_spellings_of_one_board_are_one_lock_and_one_held_document() {
        let real = crate::plat::tmp::uncreated("samefile-real"); std::fs::create_dir_all(real.join("boards")).unwrap();
        let link = crate::plat::tmp::uncreated("samefile-link"); std::os::unix::fs::symlink(&real, &link).unwrap();
        let (r, l) = (format!("{}/boards/b.kicad_pcb", real.display()), format!("{}/boards/b.kicad_pcb", link.display()));
        std::fs::write(&r, "(pcb)").unwrap();
        assert_eq!(doc_lock(&r), doc_lock(&l), "a symlinked project folder names the same board");
        assert_eq!(same_file(&l), same_file(&r));
        // a document not written yet: its folder resolved, its name kept
        let (rn, ln) = (format!("{}/boards/new.kicad_sch", real.display()), format!("{}/boards/new.kicad_sch", link.display()));
        assert_eq!(doc_lock(&rn), doc_lock(&ln));
        assert_ne!(doc_lock(&r), doc_lock(&rn), "two documents stay two");
        assert_eq!(same_file("/no/such/dir/x.kicad_pcb"), "/no/such/dir/x.kicad_pcb", "nothing there: the path as given");
        let _ = std::fs::remove_file(&link); let _ = std::fs::remove_dir_all(&real);
    }

    #[cfg(unix)] #[tokio::test] async fn a_held_document_named_through_a_symlink_is_still_left_for_its_holder() {
        let (_r, main) = mixed("symheld");
        let link = crate::plat::tmp::uncreated("symheld-link"); std::os::unix::fs::symlink(&main, &link).unwrap();
        std::fs::write(format!("{main}/boards/main/main.kicad_pcb"), "(pcb other thread)").unwrap();
        std::fs::write(format!("{main}/boards/main/main.kicad_sch"), "(sch mine)").unwrap();
        // the holder named the board through the link; git names the checkout by its real folder
        let rec = commit_design(&main, "", &[format!("{}/boards/main/main.kicad_pcb", link.display())], false, false).await.unwrap();
        assert_eq!(rec.files, vec!["boards/main/main.kicad_sch"], "the held board is not swept into another thread's commit");
        let _ = std::fs::remove_file(&link);
    }

    #[tokio::test] async fn a_document_another_thread_holds_is_left_for_it_and_main_pushes_to_its_upstream() {
        let (_r, main) = mixed("push");
        let bare = crate::plat::tmp::uncreated("push-remote");
        git(&main, &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
        git(&main, &["remote", "add", "origin", bare.to_str().unwrap()]); git(&main, &["push", "-q", "-u", "origin", "main"]);
        std::fs::write(format!("{main}/boards/main/main.kicad_pcb"), "(pcb other thread)").unwrap();
        std::fs::write(format!("{main}/boards/main/main.kicad_sch"), "(sch mine)").unwrap();
        let rec = commit_design(&main, "thread t2", &[format!("{main}/boards/main/main.kicad_pcb")], true, false).await.unwrap();
        assert_eq!(rec.files, vec!["boards/main/main.kicad_sch"]);
        assert_eq!(rec.pushed, "origin/main");
        assert_eq!(git(bare.to_str().unwrap(), &["log", "-1", "--format=%s", "main"]), "design: main.kicad_sch edited");
        assert_eq!(git(&main, &["status", "--porcelain"]), "M boards/main/main.kicad_pcb", "the held board waits for its writer");
    }

    #[tokio::test] async fn with_auto_push_off_a_turn_commits_locally_and_pushes_nothing_until_push_is_pressed() {
        let (_r, main) = mixed("nopush");
        let bare = crate::plat::tmp::uncreated("nopush-remote");
        git(&main, &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
        git(&main, &["remote", "add", "origin", bare.to_str().unwrap()]); git(&main, &["push", "-q", "-u", "origin", "main"]);
        let before = git(bare.to_str().unwrap(), &["rev-parse", "main"]);
        assert_eq!(unpushed(&main).await, Some((0, "origin/main".into())));
        std::fs::write(format!("{main}/boards/main/main.kicad_pcb"), "(pcb local)").unwrap();
        let rec = commit_design(&main, "thread t3", &[], false, false).await.unwrap();
        assert!(!rec.commit.is_empty() && rec.pushed.is_empty(), "committed, not pushed: {rec:?}");
        assert_eq!(git(&main, &["log", "-1", "--format=%s%n%b"]).trim(), "design: main.kicad_pcb edited\nthread t3", "auto-commit is unchanged");
        assert_eq!(git(bare.to_str().unwrap(), &["rev-parse", "main"]), before, "the remote did not move");
        assert_eq!(unpushed(&main).await, Some((1, "origin/main".into())), "the project page says 1 commit not pushed");
        // the page's Push: one deliberate click
        let (top, branch) = main_branch(&main).await.unwrap();
        assert_eq!(push_upstream(&top, &branch).await, "origin/main");
        assert_eq!(git(bare.to_str().unwrap(), &["log", "-1", "--format=%s", "main"]), "design: main.kicad_pcb edited");
        assert_eq!(unpushed(&main).await, Some((0, "origin/main".into())));
        // no upstream: nothing to count or push
        git(&main, &["branch", "-q", "--unset-upstream"]);
        assert_eq!(unpushed(&main).await, None); assert_eq!(push_upstream(&top, &branch).await, "");
    }

    #[test] fn a_design_commit_subject_says_which_items_changed() {
        let old = "(kicad_pcb (version 1) (net 0 \"\")\n (footprint \"R\" (at 1 2) (uuid \"f1\") (pad \"1\" smd rect (uuid \"p1\")))\n (footprint \"C\" (at 5 5) (uuid \"f2\"))\n (segment (start 0 0) (end 1 1) (uuid \"s1\"))\n (via (at 1 1) (uuid \"v1\"))\n)";
        let new = "(kicad_pcb (version 1) (net 0 \"\")\n (footprint \"R\" (at 3 4) (uuid \"f1\") (pad \"1\" smd rect (uuid \"p1\")))\n (footprint \"C\" (at 5 5) (uuid \"f2\"))\n (segment (start 0 0) (end 1 1) (uuid \"s1\"))\n (segment (start 1 1) (end 2 2) (uuid \"s2\"))\n (segment (start 2 2) (end 3 3) (uuid \"s3\"))\n)";
        assert_eq!(item_changes(old, new, false), "2 tracks added, 1 footprint changed, 1 via removed", "a moved footprint is its uuid's change, not a removal and an addition");
        assert_eq!(item_changes(old, old, false), "");
        let sch0 = "(kicad_sch (lib_symbols (symbol \"Device:R\"))\n (wire (pts (xy 0 0) (xy 1 0)))\n)";
        let sch1 = "(kicad_sch (lib_symbols (symbol \"Device:R\") (symbol \"Device:C\"))\n (symbol (lib_id \"Device:C\") (uuid \"c1\"))\n (wire (pts (xy 0 0) (xy 1 0)))\n (wire (pts (xy 1 0) (xy 2 0)))\n (no_connect (at 2 0))\n)";
        assert_eq!(item_changes(sch0, sch1, true), "1 symbol, 1 wire, 1 no-connect added", "a library symbol is not a placed one; items without a uuid match by text");
        let d = |s: &str, l: &[&str]| Described { subject: s.into(), detail: l.iter().map(|x| x.to_string()).collect() };
        assert_eq!(full_message(&d("design: a added", &["a added"]), ""), "design: a added", "the subject holds it all: no detail repeated");
        assert_eq!(full_message(&d("design: a added; and 1 more file", &["a added", "b changed"]), "by t1\n"), "design: a added; and 1 more file\n\n- a added\n- b changed\n\nby t1");
    }
    #[test] fn a_subject_is_one_line_of_72_characters_and_the_rest_goes_in_the_body() {
        let p = |r: u8, f: &str, l: &str| (r, f.to_string(), l.to_string());
        let one = compose(vec![p(2, "mech/case.step", "case.step changed"), p(0, "b/main.kicad_pcb", "main.kicad_pcb: 12 tracks added, 1 footprint changed")]);
        assert_eq!(one.subject, "design: main.kicad_pcb, case.step", "both lines (79) and the first with a count (77) are over 72: the names");
        assert_eq!(one.detail, ["main.kicad_pcb: 12 tracks added, 1 footprint changed", "case.step changed"]);
        let two = compose(vec![p(0, "b/main.kicad_pcb", "main.kicad_pcb: 2 tracks added"), p(2, "mech/case.step", "case.step changed")]);
        assert_eq!(two.subject, "design: main.kicad_pcb: 2 tracks added; case.step changed");
        let long = compose(vec![p(0, "b/main.kicad_pcb", "main.kicad_pcb: 120 tracks added, 14 footprints changed, 3 vias removed, 2 zones changed"), p(0, "b/main.kicad_sch", "main.kicad_sch: 4 symbols added"), p(2, "mech/case.step", "case.step changed")]);
        assert_eq!(long.subject, "design: main.kicad_pcb, main.kicad_sch, case.step");
        assert_eq!(long.detail.len(), 3, "every file's words are in the body");
        let many: Vec<_> = (0..20).map(|i| p(2, &format!("mech/part{i:02}.step"), &format!("part{i:02}.step added"))).collect();
        let m = compose(many);
        assert!(m.subject.chars().count() <= SUBJECT_MAX && m.subject.ends_with("more files") && !m.subject.contains('\n'), "{}", m.subject);
        assert_eq!(compose(vec![]).subject, "design: files updated");
    }

    #[tokio::test] async fn by_default_a_turn_commits_on_the_branch_with_a_subject_that_says_what_changed() {
        let (_r, main) = mixed("bydefault");
        let head = git(&main, &["rev-parse", "HEAD"]);
        std::fs::write(format!("{main}/boards/main/main.kicad_pcb"), "(kicad_pcb\n (footprint \"R\" (uuid \"f1\"))\n (segment (uuid \"s1\"))\n (segment (uuid \"s2\"))\n)").unwrap();
        std::fs::write(format!("{main}/mech/case.step"), "ISO-10303").unwrap();
        let rec = commit_design(&main, "Recorded by Backplane at the end of an agent's turn, thread t9: Can you test the USB footprint", &[], false, false).await.unwrap();
        assert_eq!(rec.reference, "", "on the branch, as before #1188");
        assert_eq!(git(&main, &["rev-parse", "HEAD^"]), head, "one commit on the branch");
        assert_eq!(git(&main, &["log", "-1", "--format=%s"]), "design: main.kicad_pcb: 1 footprint, 2 tracks added; case.step added");
        assert_eq!(git(&main, &["log", "-1", "--format=%b"]), "Recorded by Backplane at the end of an agent's turn, thread t9: Can you test the USB footprint", "the thread's first words are the body, never the subject");
        assert_eq!(git(&main, &["status", "--porcelain"]), "", "recorded: nothing left uncommitted");
        assert!(git(&main, &["for-each-ref", "refs/backplane/design"]).is_empty(), "no ref of Backplane's own by default");
    }

    #[tokio::test] async fn keep_off_my_branch_records_every_change_on_backplanes_ref_and_never_touches_the_branch() {
        let (_r, main) = mixed("offbranch");
        let bare = crate::plat::tmp::uncreated("offbranch-remote");
        git(&main, &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
        git(&main, &["remote", "add", "origin", bare.to_str().unwrap()]); git(&main, &["push", "-q", "-u", "origin", "main"]);
        let (head, remote) = (git(&main, &["rev-parse", "HEAD"]), git(bare.to_str().unwrap(), &["rev-parse", "main"]));
        std::fs::write(format!("{main}/firmware/CMakeLists.txt"), "project(fw3)").unwrap(); git(&main, &["add", "firmware/CMakeLists.txt"]); // the person's own staged change stays staged as it was
        std::fs::write(format!("{main}/boards/main/main.kicad_pcb"), "(kicad_pcb\n (via (uuid \"v1\"))\n)").unwrap();
        let index = git(&main, &["write-tree"]);
        // auto-push on as well: Backplane's ref is never pushed, and the person's branch is not pushed for them
        let rec = commit_design(&main, "thread t4", &[], true, true).await.unwrap();
        assert_eq!(rec.reference, "refs/backplane/design/main"); assert_eq!(rec.files, vec!["boards/main/main.kicad_pcb"]); assert_eq!(rec.pushed, "");
        assert_eq!(git(&main, &["rev-parse", "HEAD"]), head, "the branch did not move");
        assert_eq!(git(&main, &["write-tree"]), index, "the index is the person's as it was");
        assert_eq!(git(&main, &["status", "--porcelain"]), "M boards/main/main.kicad_pcb\nM  firmware/CMakeLists.txt", "the change stays in the work tree, uncommitted, for the person to commit");
        assert_eq!(git(bare.to_str().unwrap(), &["rev-parse", "main"]), remote, "nothing pushed");
        assert_eq!(git(&main, &["log", "-1", "--format=%s%n%b", "refs/backplane/design/main"]), "design: main.kicad_pcb: 1 via added\nthread t4");
        assert_eq!(git(&main, &["show", "refs/backplane/design/main:boards/main/main.kicad_pcb"]), "(kicad_pcb\n (via (uuid \"v1\"))\n)", "the change is recorded");
        assert_eq!(git(&main, &["rev-parse", "refs/backplane/design/main^"]), head, "on top of the branch");
        // nothing new: nothing recorded
        assert_eq!(commit_design(&main, "thread t4", &[], true, true).await.unwrap(), Recorded::default());
        // the next turn's change grows the chain and names only what it changed
        let tip = git(&main, &["rev-parse", "refs/backplane/design/main"]);
        std::fs::write(format!("{main}/boards/main/main.kicad_sch"), "(kicad_sch\n (wire (pts))\n)").unwrap();
        let rec = commit_design(&main, "thread t4", &[], false, true).await.unwrap();
        assert_eq!(rec.files, vec!["boards/main/main.kicad_sch"]);
        assert_eq!(git(&main, &["rev-parse", "refs/backplane/design/main^"]), tip);
        assert_eq!(git(&main, &["log", "-1", "--format=%s", "refs/backplane/design/main"]), "design: main.kicad_sch: 1 wire added");
        // the person commits when they choose; the next record starts from their commit and keeps the old chain as a second parent
        git(&main, &["commit", "-qam", "my own words"]);
        let (mine, tip) = (git(&main, &["rev-parse", "HEAD"]), git(&main, &["rev-parse", "refs/backplane/design/main"]));
        std::fs::write(format!("{main}/mech/case.step"), "ISO-10303").unwrap();
        commit_design(&main, "thread t4", &[], false, true).await.unwrap();
        assert_eq!(git(&main, &["rev-list", "--parents", "-n1", "refs/backplane/design/main"]).split(' ').skip(1).collect::<Vec<_>>(), [mine.as_str(), tip.as_str()], "nothing recorded is unreachable");
        assert_eq!(git(&main, &["log", "-1", "--format=%s", "refs/backplane/design/main"]), "design: case.step added", "measured from the person's commit");
        assert_eq!(git(&main, &["rev-parse", "HEAD"]), mine);
    }

    #[tokio::test] async fn a_detached_head_is_never_committed_onto_and_kept_off_the_branch_is_recorded_on_its_own_ref() {
        let (_r, main) = mixed("detached");
        git(&main, &["checkout", "-q", "--detach"]); let head = git(&main, &["rev-parse", "HEAD"]);
        std::fs::write(format!("{main}/boards/main/main.kicad_pcb"), "(kicad_pcb\n (via (uuid \"v1\"))\n)").unwrap();
        assert_eq!(commit_design(&main, "", &[], false, false).await, Err("the main checkout is not on a branch".into()), "by default: as before");
        let rec = commit_design(&main, "", &[], false, true).await.unwrap();
        assert_eq!(rec.reference, "refs/backplane/design/detached"); assert_eq!(git(&main, &["rev-parse", "HEAD"]), head);
        assert_eq!(git(&main, &["rev-parse", "refs/backplane/design/detached^"]), head);
    }

    #[tokio::test] async fn the_thread_is_a_trailer_and_the_commit_is_the_repositorys_own_identity_on_or_off_the_branch() {
        let (_r, main) = mixed("trailer");
        // whoever git says commits here (the repository's config, or the environment over it): the hub's own commits must be that identity
        let who = |r: &str| git(&main, &["log", "-1", "--format=%an <%ae> / %cn <%ce>", r]);
        let ident = |k: &str| { let o = std::process::Command::new("git").args(["var", k]).current_dir(&main).output().unwrap(); let l = String::from_utf8_lossy(&o.stdout).to_string(); l.rsplitn(3, ' ').nth(2).unwrap_or("").to_string() };
        let want = format!("{} / {}", ident("GIT_AUTHOR_IDENT"), ident("GIT_COMMITTER_IDENT"));
        assert!(!want.contains("backplane@localhost") && want.contains('@'), "{want}");
        let body = crate::agents::design::design_body("t1794981671-222", "Can you test the USB footprint", "turn", "");
        std::fs::write(format!("{main}/boards/main/main.kicad_pcb"), "(kicad_pcb (via (uuid \"v1\")))").unwrap();
        commit_design(&main, &body, &[], false, false).await.unwrap();
        let trailer = |r: &str| git(&main, &["log", "-1", "--format=%(trailers:key=Backplane-Thread,valueonly)", r]);
        assert_eq!(trailer("HEAD"), "t1794981671-222", "machine-findable by a fixed key");
        assert!(!git(&main, &["log", "-1", "--format=%s", "HEAD"]).contains("USB"), "the first message is not the subject");
        assert_eq!(who("HEAD"), want, "on the branch: the repository's identity, as before");
        std::fs::write(format!("{main}/boards/main/main.kicad_pcb"), "(kicad_pcb (via (uuid \"v1\")) (via (uuid \"v2\")))").unwrap();
        commit_design(&main, &body, &[], false, true).await.unwrap();
        assert_eq!(trailer("refs/backplane/design/main"), "t1794981671-222");
        assert_eq!(who("refs/backplane/design/main"), want, "off the branch: the same identity (in the Backplane repository, i2cjak <build@amemb.com>), never Backplane's");
    }

    /// Hubble_DK's timing (#1188): BP_SUMMARY_BOARD=<a .kicad_pcb> cargo test --bin backplane-hub summary_time -- --ignored --nocapture
    #[tokio::test] #[ignore] async fn summary_time_on_a_large_board() {
        let src = std::env::var("BP_SUMMARY_BOARD").expect("BP_SUMMARY_BOARD");
        let text = std::fs::read_to_string(&src).unwrap();
        let r = tmp("summary-time"); let main = r.to_str().unwrap().to_string();
        std::fs::write(r.join("b.kicad_pcb"), &text).unwrap();
        git(&main, &["init", "-q", "-b", "main"]); git(&main, &["add", "-A"]); git(&main, &["commit", "-qm", "base"]);
        // move every track's start a little: each is "changed" by its uuid
        let moved = text.replace("(segment\n\t\t(start ", "(segment\n\t\t(start 1");
        std::fs::write(r.join("b.kicad_pcb"), &moved).unwrap();
        let t = std::time::Instant::now(); let d = describe(&main, "HEAD", &["b.kicad_pcb".to_string()]).await; let ms = t.elapsed().as_millis();
        let t = std::time::Instant::now(); let n = crate::files::snapshot::count_items(&text, false); let scan = t.elapsed().as_millis();
        println!("summary_time: {} bytes, {n} items; describe {ms} ms ({}); one scan {scan} ms", text.len(), d.subject);
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

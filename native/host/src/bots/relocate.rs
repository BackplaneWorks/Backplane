//! Where a bot lives: `~/.backplane/bots/<bot>` (its working folder, tasks and runs), and the one-time move of what older builds kept
//! elsewhere (`<hub home>/bots/<bot>`, or a folder a WorktreeSet once named, like a lane checkout) with the agent's own per-folder
//! session history and memory (`<home>/providers/accounts-v1/*/*/projects/<folder with / and . as ->`), which is keyed by the working
//! folder and would be orphaned by a plain switch.
//!
//! The move is: back up, copy, verify, then mark. Nothing is deleted; the old folders stay as they were. A bot whose marker exists is skipped.
use std::path::{Path, PathBuf};

/// The bots' folder. `BACKPLANE_BOTS_DIR` wins; a copy of a real log (BACKPLANE_COPY=1) and a home under the temp dir (tests) keep
/// `<home>/bots`, so only a real hub writes `~/.backplane`.
pub fn root(home: &Path) -> PathBuf {
    if let Ok(d) = std::env::var("BACKPLANE_BOTS_DIR") { if !d.is_empty() { return PathBuf::from(d) } }
    let own = home.join("bots");
    if crate::copy::on() || home.starts_with(std::env::temp_dir()) { return own }
    match std::env::var_os("HOME") { Some(h) if !h.is_empty() => Path::new(&h).join(".backplane").join("bots"), _ => own }
}

/// The agent CLI's name for a working folder's history: every character outside [A-Za-z0-9] becomes '-'.
pub fn munge(p: &str) -> String { p.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect() }

/// Copies `src` into `dst` (files and folders; symlinks are skipped), never overwriting a file already there with the same size.
fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<usize> {
    let mut n = 0; std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?; let ty = e.file_type()?; let (s, d) = (e.path(), dst.join(e.file_name()));
        if ty.is_symlink() { continue }
        if ty.is_dir() { n += copy_tree(&s, &d)?; continue }
        let sl = e.metadata()?.len(); if std::fs::metadata(&d).map_or(false, |m| m.len() == sl) { continue }
        std::fs::copy(&s, &d)?; n += 1;
    }
    Ok(n)
}
/// Every file of `src` is in `dst` with the same size.
fn verify_tree(src: &Path, dst: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(src) else { return false };
    for e in rd.flatten() {
        let Ok(ty) = e.file_type() else { return false }; let d = dst.join(e.file_name());
        if ty.is_symlink() { continue }
        if ty.is_dir() { if !verify_tree(&e.path(), &d) { return false } } else if std::fs::metadata(&d).map(|m| m.len()).ok() != e.metadata().ok().map(|m| m.len()) { return false }
    }
    true
}

/// The agent history folders (`projects` of each account profile) under the hub home.
fn project_dirs(home: &Path) -> Vec<PathBuf> {
    let mut v = vec![]; let base = home.join("providers").join("accounts-v1");
    for a in std::fs::read_dir(&base).into_iter().flatten().flatten() { for b in std::fs::read_dir(a.path()).into_iter().flatten().flatten() { let p = b.path().join("projects"); if p.is_dir() { v.push(p) } } }
    v
}

/// Moves one bot. `old_cwd` is where its thread worked until now ("" if unknown). Returns a line for the log.
pub fn move_bot(home: &Path, root: &Path, bot: &str, old_cwd: &str, stamp: u64) -> String {
    let dest = root.join(bot); let marker = dest.join(".moved.json");
    if marker.exists() { return format!("bots: {bot} already at {}", dest.display()) }
    let old_home = home.join("bots").join(bot);
    let mut from: Vec<PathBuf> = vec![]; // folders whose files come over
    if old_home != dest && old_home.is_dir() { from.push(old_home.clone()) }
    let mut cwds: Vec<String> = vec![]; // folders whose session history comes over
    for c in [old_cwd.to_string(), old_home.to_string_lossy().into_owned()] { if !c.is_empty() && Path::new(&c) != dest && !cwds.contains(&c) { cwds.push(c) } }
    let dest_key = munge(&dest.to_string_lossy());
    let hist: Vec<(PathBuf, PathBuf)> = project_dirs(home).into_iter().flat_map(|p| cwds.iter().map(|c| (p.join(munge(c)), p.join(&dest_key))).collect::<Vec<_>>()).filter(|(s, _)| s.is_dir()).collect();
    let backup = root.parent().unwrap_or(root).join("backups").join(format!("bots-move-{stamp}")).join(bot);
    let mut copied = 0usize;
    let step = (|| -> std::io::Result<bool> {
        if !from.is_empty() || !hist.is_empty() {
            for (i, f) in from.iter().enumerate() { copy_tree(f, &backup.join(format!("folder{i}")))?; }
            for (i, (s, _)) in hist.iter().enumerate() { copy_tree(s, &backup.join(format!("history{i}")))?; }
        }
        std::fs::create_dir_all(&dest)?;
        for f in &from { copied += copy_tree(f, &dest)?; }
        for (s, d) in &hist { copied += copy_tree(s, d)?; }
        Ok(from.iter().all(|f| verify_tree(f, &dest)) && hist.iter().all(|(s, d)| verify_tree(s, d)))
    })();
    match step {
        Ok(true) => {
            let m = serde_json::json!({"bot":bot,"from":from.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),"oldCwd":old_cwd,"history":hist.iter().map(|(s, _)| s.to_string_lossy()).collect::<Vec<_>>(),"files":copied,"backup":backup.to_string_lossy(),"at":stamp});
            let _ = std::fs::write(&marker, m.to_string());
            format!("bots: {bot} moved to {} ({copied} files; backup {})", dest.display(), backup.display())
        }
        Ok(false) => format!("bots: {bot}: copy did not verify; left unmarked, old folders untouched (backup {})", backup.display()),
        Err(e) => format!("bots: {bot}: move failed ({e}); left unmarked, old folders untouched"),
    }
}

/// The start-up move for every bot: `bots` is (bot id, the folder its thread last worked in). A no-op when the root is `<home>/bots`
/// itself, or on a copy of a real log.
pub fn run(home: &Path, bots: &[(String, String)]) -> Vec<String> {
    let r = root(home);
    if crate::copy::on() || r == home.join("bots") { return vec![] }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let _ = std::fs::create_dir_all(&r);
    bots.iter().filter(|(b, _)| super::rpc::safe_id(b)).map(|(b, c)| move_bot(home, &r, b, c, stamp)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn w(p: &Path, s: &str) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, s).unwrap() }

    #[test] fn munge_matches_the_agents_folder_names() {
        assert_eq!(munge("/home/h/.backplane-gpui-fresh/home/bots/b1-2"), "-home-h--backplane-gpui-fresh-home-bots-b1-2");
    }

    #[test] fn a_bot_moves_with_its_history_a_backup_is_kept_and_a_second_run_does_nothing() {
        let t = std::env::temp_dir().join(format!("bp-relocate-{}", std::process::id())); let _ = std::fs::remove_dir_all(&t);
        let home = t.join("hub"); let root = t.join("dot/bots"); let lane = t.join("lanes/bub");
        w(&home.join("bots/b1/tasks/a.json"), "{}"); w(&lane.join("note.txt"), "x");
        let pj = home.join("providers/accounts-v1/acc/prof/projects");
        w(&pj.join(munge(&lane.to_string_lossy())).join("s1.jsonl"), "history"); w(&pj.join(munge(&home.join("bots/b1").to_string_lossy())).join("memory/m.md"), "mem");
        let a = move_bot(&home, &root, "b1", &lane.to_string_lossy(), 7); assert!(a.contains("moved"), "{a}");
        assert_eq!(std::fs::read_to_string(root.join("b1/tasks/a.json")).unwrap(), "{}");
        let key = pj.join(munge(&root.join("b1").to_string_lossy()));
        assert_eq!(std::fs::read_to_string(key.join("s1.jsonl")).unwrap(), "history"); assert_eq!(std::fs::read_to_string(key.join("memory/m.md")).unwrap(), "mem");
        assert!(t.join("dot/backups/bots-move-7/b1").is_dir() && home.join("bots/b1/tasks/a.json").exists() && lane.join("note.txt").exists(), "old data and backup stay");
        assert!(move_bot(&home, &root, "b1", &lane.to_string_lossy(), 8).contains("already"));
        assert!(!t.join("dot/backups/bots-move-8").exists());
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test] fn a_bot_with_nothing_old_just_gets_its_folder() {
        let t = std::env::temp_dir().join(format!("bp-relocate0-{}", std::process::id())); let _ = std::fs::remove_dir_all(&t);
        let a = move_bot(&t.join("hub"), &t.join("dot/bots"), "b2", "", 1); assert!(a.contains("moved"), "{a}");
        assert!(t.join("dot/bots/b2/.moved.json").exists() && !t.join("dot/backups").exists());
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test] fn a_home_under_the_temp_dir_keeps_its_own_bots_folder() {
        let h = std::env::temp_dir().join("bp-relocate-root"); if std::env::var("BACKPLANE_BOTS_DIR").is_err() { assert_eq!(root(&h), h.join("bots")); }
    }

    /// Dry run on a copy: BP_RELOCATE_HOME=<copied hub home> BP_RELOCATE_ROOT=<new bots folder> BP_RELOCATE_BOT=<id> BP_RELOCATE_CWD=<old folder>.
    #[test] #[ignore] fn on_a_copied_home() {
        let g = |k: &str| std::env::var(k).unwrap();
        println!("{}", move_bot(Path::new(&g("BP_RELOCATE_HOME")), Path::new(&g("BP_RELOCATE_ROOT")), &g("BP_RELOCATE_BOT"), &g("BP_RELOCATE_CWD"), 1));
    }
}

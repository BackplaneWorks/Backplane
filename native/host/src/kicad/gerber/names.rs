//! Which files and folders are fab output (Gerbers, drills, job files), and a project's newest set: by name and listing only, no reading,
//! so the presenter and the Files tab can ask (kicad/gerber reads them).
use std::path::{Path, PathBuf};

/// The fab output files by name: Gerbers (KiCad's .gbr and the Protel extensions), drills, the job file.
pub fn is_gerber(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let ext = n.rsplit_once('.').map_or("", |x| x.1);
    matches!(ext, "gbr" | "ger" | "pho" | "art" | "gtl" | "gbl" | "gts" | "gbs" | "gto" | "gbo" | "gtp" | "gbp" | "gta" | "gba" | "gm1" | "gm2" | "gm" | "gko" | "gml" | "gtv")
        || (ext.len() >= 2 && ext.starts_with('g') && ext[1..].bytes().all(|c| c.is_ascii_digit()))
        || (ext.len() >= 3 && ext.starts_with("gl") && ext[2..].bytes().all(|c| c.is_ascii_digit()))
}
pub fn is_drill(name: &str) -> bool { let n = name.to_ascii_lowercase(); n.ends_with(".drl") || n.ends_with(".xln") || n.ends_with(".exc") || n.ends_with(".drd") }
pub fn is_job(name: &str) -> bool { name.to_ascii_lowercase().ends_with(".gbrjob") }
pub fn is_fab_file(name: &str) -> bool { is_gerber(name) || is_drill(name) || is_job(name) }

/// Canonicalise `p` only if it resolves inside `canon_root` (skips symlinks escaping the project root).
pub(crate) fn canon_in(p: &Path, canon_root: &Path) -> Option<PathBuf> {
    let c = std::fs::canonicalize(p).ok()?;
    c.starts_with(canon_root).then_some(c)
}

/// The files of a set: a folder's fab files (and its subfolders', two deep: mfg/gerbers/), a job file's folder's, or the one file.
/// Every directory and file read must canonicalise inside `root` (or inside the set's own canonical folder when `root` is `None`).
/// Returns canonical paths so callers open the verified target directly.
pub fn set_files_in(path: &Path, root: Option<&Path>) -> Vec<PathBuf> {
    let bound = match root {
        Some(r) => match std::fs::canonicalize(r) { Ok(c) => c, Err(_) => return vec![] },
        None => {
            let base = if path.is_file() { path.parent().unwrap_or(path) } else { path };
            match std::fs::canonicalize(base) { Ok(c) => c, Err(_) => return vec![] }
        }
    };
    let Some(canon_path) = canon_in(path, &bound) else { return vec![] };
    if canon_path.is_file() {
        let n = canon_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        return if is_job(&n) { canon_path.parent().map(|d| set_files_in(d, Some(&bound))).unwrap_or_default() } else { vec![canon_path] }
    }
    let mut out = vec![];
    fn walk(d: &Path, depth: u32, bound: &Path, out: &mut Vec<PathBuf>) {
        let Some(cd) = canon_in(d, bound) else { return };
        let Ok(rd) = std::fs::read_dir(&cd) else { return };
        let mut es: Vec<_> = rd.flatten().collect();
        es.sort_by_key(|e| e.file_name());
        for e in es {
            let p = e.path(); let n = e.file_name().to_string_lossy().into_owned();
            let Some(cp) = canon_in(&p, bound) else { continue };
            if cp.is_dir() { if depth > 0 && !n.starts_with('.') { walk(&cp, depth - 1, bound, out) } } else if is_fab_file(&n) { out.push(cp) }
            if out.len() >= 200 { return }
        }
    }
    walk(&canon_path, 2, &bound, &mut out);
    out
}

/// The files of a set (containment checked against the set's own canonical folder).
pub fn set_files(path: &Path) -> Vec<PathBuf> { set_files_in(path, None) }

/// Whether a path is a Gerber set this reader draws: a fab file, or a folder holding some (directly).
pub fn is_set_path(p: &Path) -> bool {
    if p.is_dir() { return std::fs::read_dir(p).map_or(false, |rd| rd.flatten().any(|e| { let n = e.file_name().to_string_lossy().into_owned(); is_gerber(&n) || is_drill(&n) })) }
    p.file_name().map_or(false, |n| is_fab_file(&n.to_string_lossy()))
}
/// Fast presence check: whether a project has at least one eligible Gerber set inside its canonical root (stops at the first match).
pub fn has_gerbers(root: &Path) -> bool {
    let Ok(canon_root) = std::fs::canonicalize(root) else { return false };
    fn walk(d: &Path, depth: u32, canon_root: &Path) -> bool {
        let Some(cd) = canon_in(d, canon_root) else { return false };
        let Ok(rd) = std::fs::read_dir(&cd) else { return false };
        for e in rd.flatten() {
            let p = e.path(); let n = e.file_name().to_string_lossy().into_owned();
            let Some(cp) = canon_in(&p, canon_root) else { continue };
            if cp.is_dir() { if depth > 0 && !n.starts_with('.') && walk(&cp, depth - 1, canon_root) { return true } }
            else if is_gerber(&n) || is_drill(&n) { return true }
        }
        false
    }
    ["mfg", "gerbers", "gerber"].iter().any(|top| walk(&canon_root.join(top), 3, &canon_root))
}
/// A project's Gerbers: the folder under `<root>/mfg` (three deep) or `<root>/gerbers` holding fab files inside the canonical root, the one written last.
/// Returns the path spelled under `root` (keeping watch keys consistent when `root` itself is a symlink) after verifying canonical containment.
pub fn default_set(root: &Path) -> Option<PathBuf> {
    let canon_root = std::fs::canonicalize(root).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    fn walk(d: &Path, depth: u32, canon_root: &Path, best: &mut Option<(std::time::SystemTime, PathBuf)>) {
        let Some(cd) = canon_in(d, canon_root) else { return };
        let Ok(rd) = std::fs::read_dir(&cd) else { return };
        let mut newest: Option<std::time::SystemTime> = None;
        for e in rd.flatten() {
            let p = e.path(); let n = e.file_name().to_string_lossy().into_owned();
            let Some(cp) = canon_in(&p, canon_root) else { continue };
            if cp.is_dir() { if depth > 0 && !n.starts_with('.') { walk(&cp, depth - 1, canon_root, best) } }
            else if is_gerber(&n) || is_drill(&n) { if let Ok(t) = std::fs::metadata(&cp).and_then(|m| m.modified()) { newest = Some(newest.map_or(t, |x| x.max(t))) } }
        }
        if let Some(t) = newest { if best.as_ref().map_or(true, |b| t > b.0) { *best = Some((t, cd)) } }
    }
    for top in ["mfg", "gerbers", "gerber"] { walk(&canon_root.join(top), 3, &canon_root, &mut best) }
    best.and_then(|(_, cd)| cd.strip_prefix(&canon_root).ok().map(|rel| root.join(rel)))
}


/// The board a set was made from: the .kicad_pcb beside the first .kicad_pro in the set's folder or up to four ancestors,
/// never leaving `root` when provided. Returns the canonical path when `root` is provided.
pub fn board_of_in(set: &Path, root: Option<&Path>) -> Option<std::path::PathBuf> {
    let canon_root = match root {
        Some(r) => Some(std::fs::canonicalize(r).ok()?),
        None => None,
    };
    let start = match &canon_root {
        Some(cr) => canon_in(set, cr)?,
        None => set.to_path_buf(),
    };
    for d in start.ancestors().take(5) {
        if let Some(cr) = &canon_root { if !d.starts_with(cr) { break } }
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map_or(false, |x| x == "kicad_pro") {
                let b = p.with_extension("kicad_pcb");
                match &canon_root {
                    Some(cr) => { if let Some(cb) = canon_in(&b, cr) { return Some(cb) } }
                    None => { if b.is_file() { return Some(b) } }
                }
            }
        }
    }
    None
}

/// The board a set was made from (unbounded ancestors; prefer `board_of_in` when the project root is known).
pub fn board_of(set: &Path) -> Option<std::path::PathBuf> { board_of_in(set, None) }
/// The path of a set compared with its board (kicad/plotread.rs reads it here): "gerbercmp\n<set>\n<board>".
/// What "Open in GerbView" opens of a set (Backplane KiCad's in-process GerbView, `OpenGerbers`): its job file, which brings all its
/// layers, else its Gerber and drill files, confined to `root` when provided.
pub fn gerbview_files_in(set: &Path, root: Option<&Path>) -> Vec<String> {
    let fs = set_files_in(set, root);
    if let Some(j) = fs.iter().find(|f| is_job(&f.to_string_lossy())) { return vec![j.to_string_lossy().into_owned()] }
    fs.iter().map(|f| f.to_string_lossy().into_owned()).collect()
}
pub fn gerbview_files(set: &Path) -> Vec<String> { gerbview_files_in(set, None) }
pub const CMP: &str = "gerbercmp\n";
pub fn cmp_path(set: &str, board: &str) -> String { format!("{CMP}{set}\n{board}") }
/// The set a viewer's file names: itself, or the set of a comparison.
pub fn set_of(file: &str) -> &str { file.strip_prefix(CMP).map_or(file, |r| r.split_once('\n').map_or(r, |x| x.0)) }

/// The last comparison of each set with its board, layer by layer: (name, on the board only, in the Gerbers only), for the layer list.
static COUNTS: std::sync::Mutex<Vec<(String, Vec<(String, usize, usize)>)>> = std::sync::Mutex::new(Vec::new());
pub fn set_counts(set: &str, v: Vec<(String, usize, usize)>) { let mut c = COUNTS.lock().unwrap(); c.retain(|x| x.0 != set); c.push((set.to_string(), v)); if c.len() > 16 { c.remove(0); } }
pub fn counts_of(set: &str) -> Vec<(String, usize, usize)> { COUNTS.lock().unwrap().iter().find(|x| x.0 == set).map(|x| x.1.clone()).unwrap_or_default() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn gerbview_opens_a_sets_job_else_its_files() {
        let t = crate::plat::tmp::dir("gerbview-files"); let d: &Path = &t;
        for f in ["b-F_Cu.gbr", "b-B_Cu.gbr", "b-PTH.drl", "notes.txt"] { std::fs::write(d.join(f), "x").unwrap() }
        let mut v = gerbview_files(d); v.sort();
        assert_eq!(v.iter().map(|f| f.rsplit(['/', '\\']).next().unwrap()).collect::<Vec<_>>(), ["b-B_Cu.gbr", "b-F_Cu.gbr", "b-PTH.drl"]);
        std::fs::write(d.join("b-job.gbrjob"), "{}").unwrap();
        assert_eq!(gerbview_files(d).iter().map(|f| f.rsplit(['/', '\\']).next().unwrap()).collect::<Vec<_>>(), ["b-job.gbrjob"]);
    }

    #[cfg(unix)]
    #[test] fn gerber_discovery_members_and_companion_board_stay_inside_project_root() {
        use std::os::unix::fs::symlink;
        let base = crate::plat::tmp::dir("gerber-contain");
        let outside = base.join("outside");
        let proj = base.join("proj");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(outside.join("ext-F_Cu.gbr"), "G04 ext*\nM02*\n").unwrap();

        // 1. Linked set directory pointing outside project root is skipped
        symlink(&outside, proj.join("gerbers")).unwrap();
        assert!(!has_gerbers(&proj), "linked outside set dir must not report has_gerbers");
        assert!(default_set(&proj).is_none(), "linked outside set dir must be skipped by default_set");
        assert!(set_files_in(&proj.join("gerbers"), Some(&proj)).is_empty());

        // 2. Linked member file inside a valid in-root set is skipped while in-root members stay
        let mfg = proj.join("mfg");
        std::fs::create_dir_all(&mfg).unwrap();
        std::fs::write(mfg.join("in-F_Cu.gbr"), "G04 in*\nM02*\n").unwrap();
        symlink(outside.join("ext-F_Cu.gbr"), mfg.join("leak-B_Cu.gbr")).unwrap();
        assert!(has_gerbers(&proj));
        let set = default_set(&proj).expect("valid in-root mfg set");
        let names: Vec<String> = set_files_in(&set, Some(&proj))
            .iter().filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
        assert_eq!(names, vec!["in-F_Cu.gbr"]);

        // 3. Companion board in an ancestor outside project root is skipped; one inside root is found
        std::fs::write(base.join("parent.kicad_pro"), "{}").unwrap();
        std::fs::write(base.join("parent.kicad_pcb"), "(kicad_pcb)").unwrap();
        assert!(board_of_in(&set, Some(&proj)).is_none(), "companion board outside root must be skipped");
        // Comparison reader also refuses an out-of-root comparison board at the read boundary
        let bad_cmp = cmp_path(&set.to_string_lossy(), &base.join("parent.kicad_pcb").to_string_lossy());
        assert!(matches!(super::super::plot_in(&bad_cmp, Some(&proj)), super::super::super::plotgen::Read::Fail(_)), "plot_in must refuse comparison board outside root");
        std::fs::write(proj.join("local.kicad_pro"), "{}").unwrap();
        std::fs::write(proj.join("local.kicad_pcb"), "(kicad_pcb)").unwrap();
        assert_eq!(board_of_in(&set, Some(&proj)).unwrap().file_name().unwrap(), "local.kicad_pcb");

        // 4. Project root that is itself a symlink keeps watch-key spelling under the symlink while I/O reads canonical files
        let sym_root = base.join("proj-link");
        symlink(&proj, &sym_root).unwrap();
        let sym_set = default_set(&sym_root).expect("symlinked root finds its in-root set");
        assert!(sym_set.starts_with(&sym_root), "watch-key spelling stays under symlinked root: {sym_set:?}");
        let sym_files = set_files_in(&sym_set, Some(&sym_root));
        assert_eq!(sym_files.len(), 1);
        assert_eq!(sym_files[0], std::fs::canonicalize(mfg.join("in-F_Cu.gbr")).unwrap(), "I/O paths returned are canonical");
    }
}

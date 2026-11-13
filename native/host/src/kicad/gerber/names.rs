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

/// The files of a set: a folder's fab files (and its subfolders', two deep: mfg/gerbers/), a job file's folder's, or the one file.
pub fn set_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        let n = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        return if is_job(&n) { path.parent().map(set_files).unwrap_or_default() } else { vec![path.to_path_buf()] }
    }
    let mut out = vec![];
    fn walk(d: &Path, depth: u32, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        let mut es: Vec<_> = rd.flatten().collect();
        es.sort_by_key(|e| e.file_name());
        for e in es {
            let p = e.path(); let n = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() { if depth > 0 && !n.starts_with('.') { walk(&p, depth - 1, out) } } else if is_fab_file(&n) { out.push(p) }
            if out.len() >= 200 { return }
        }
    }
    walk(path, 2, &mut out);
    out
}

/// Whether a path is a Gerber set this reader draws: a fab file, or a folder holding some (directly).
pub fn is_set_path(p: &Path) -> bool {
    if p.is_dir() { return std::fs::read_dir(p).map_or(false, |rd| rd.flatten().any(|e| { let n = e.file_name().to_string_lossy().into_owned(); is_gerber(&n) || is_drill(&n) })) }
    p.file_name().map_or(false, |n| is_fab_file(&n.to_string_lossy()))
}
/// A project's Gerbers: the folder under `<root>/mfg` (three deep) or `<root>/gerbers` holding fab files, the one written last.
pub fn default_set(root: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    fn walk(d: &Path, depth: u32, best: &mut Option<(std::time::SystemTime, PathBuf)>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        let mut newest: Option<std::time::SystemTime> = None;
        for e in rd.flatten() {
            let p = e.path(); let n = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() { if depth > 0 && !n.starts_with('.') { walk(&p, depth - 1, best) } }
            else if is_gerber(&n) || is_drill(&n) { if let Ok(t) = e.metadata().and_then(|m| m.modified()) { newest = Some(newest.map_or(t, |x| x.max(t))) } }
        }
        if let Some(t) = newest { if best.as_ref().map_or(true, |b| t > b.0) { *best = Some((t, d.to_path_buf())) } }
    }
    for top in ["mfg", "gerbers", "gerber"] { walk(&root.join(top), 3, &mut best) }
    best.map(|b| b.1)
}


/// The board a set was made from: the .kicad_pcb beside the first .kicad_pro in the set's folder or the four above it.
pub fn board_of(set: &Path) -> Option<std::path::PathBuf> {
    for d in set.ancestors().take(5) {
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        for e in rd.flatten() { let p = e.path(); if p.extension().map_or(false, |x| x == "kicad_pro") { let b = p.with_extension("kicad_pcb"); if b.is_file() { return Some(b) } } }
    }
    None
}
/// The path of a set compared with its board (kicad/plotread.rs reads it here): "gerbercmp\n<set>\n<board>".
/// What "Open in GerbView" opens of a set (Backplane KiCad's in-process GerbView, `OpenGerbers`): its job file, which brings all its
/// layers, else its Gerber and drill files.
pub fn gerbview_files(set: &Path) -> Vec<String> {
    let fs = set_files(set);
    if let Some(j) = fs.iter().find(|f| is_job(&f.to_string_lossy())) { return vec![j.to_string_lossy().into_owned()] }
    fs.iter().map(|f| f.to_string_lossy().into_owned()).collect()
}
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
        assert_eq!(v.iter().map(|f| f.rsplit('/').next().unwrap()).collect::<Vec<_>>(), ["b-B_Cu.gbr", "b-F_Cu.gbr", "b-PTH.drl"]);
        std::fs::write(d.join("b-job.gbrjob"), "{}").unwrap();
        assert_eq!(gerbview_files(d).iter().map(|f| f.rsplit('/').next().unwrap()).collect::<Vec<_>>(), ["b-job.gbrjob"]);
    }
}

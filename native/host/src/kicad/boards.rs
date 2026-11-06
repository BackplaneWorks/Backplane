//! boards.list: the boards the viewer's picker offers.
//! Pure twin of rule until the rules process is wired; defs are named at each function. Law boards_list_offered.
use std::path::Path;

pub const MOST: usize = 24;
/// Boards.copies on a folder's name
fn copies(d: &str) -> bool { super::project::copies(&d.to_lowercase()) }
/// Boards.copy: the path is inside a folder of copies (its file name is not a folder)
fn copy(p: &str) -> bool { let parts: Vec<&str> = p.split('/').collect(); parts[..parts.len() - 1].iter().any(|d| copies(d)) }
fn pcb(p: &str) -> bool { p.ends_with(".kicad_pcb") }
/// Boards.pro: the project file a board would have beside it
fn pro(p: &str) -> String { format!("{}.kicad_pro", &p[..p.len() - 10.min(p.len())]) }

/// Boards.real: a KiCad project's own boards (a project file beside them), outside folders of copies. Only these count as something to order: a repository holding
/// board files as test fixtures is no KiCad project, and offering it Buy it is what the person asked to be rid of (the picker below still shows them).
pub fn own(all: &[String]) -> Vec<String> { all.iter().filter(|p| pcb(p) && !copy(p) && all.contains(&pro(p))).cloned().collect() }

/// Boards.pick: all = the hub's list (boards and project files, from the folder); bd = the default board.
pub fn pick(all: &[String], bd: &str) -> Vec<String> {
    let real = own(all);
    let outside: Vec<String> = all.iter().filter(|p| pcb(p) && !copy(p)).cloned().collect();
    let pcbs: Vec<String> = all.iter().filter(|p| pcb(p)).cloned().collect();
    let chosen = if !real.is_empty() { real } else if !outside.is_empty() { outside } else { pcbs };
    let mut v: Vec<String> = if bd.is_empty() { chosen } else { std::iter::once(bd.to_string()).chain(chosen.into_iter().filter(|x| x != bd)).collect() };
    v.truncate(MOST); v
}

/// Od.boards: `find . -maxdepth 5` for *.kicad_pcb and *.kicad_pro (hidden folders, node_modules, ./extra and ./mfg pruned; `_autosave-*`
/// and `*backup*` files left out), LC_ALL=C sorted, 2000 at most, paths from the folder.
pub fn find(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, rel: &str, depth: usize, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
            let Ok(ft) = e.file_type() else { continue };
            let p = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if ft.is_dir() { // a symlink to a folder is not -type d: find lists nothing for it
                if depth < 5 && !name.starts_with('.') && name != "node_modules" && p != "extra" && p != "mfg" { walk(&e.path(), &p, depth + 1, out) }
            } else if (name.ends_with(".kicad_pcb") || name.ends_with(".kicad_pro")) && !name.starts_with("_autosave-") && !name.contains("backup") { out.push(p) }
        }
    }
    let mut v = vec![]; walk(root, "", 1, &mut v); v.sort(); v.truncate(2000); v
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(xs: &[&str]) -> Vec<String> { xs.iter().map(|x| x.to_string()).collect() }
    // law boards_list_offered: the project's own boards lead with the default, copies stay out, at most MOST
    #[test] fn boards_list_offered() {
        let all = s(&["a/x.kicad_pcb", "a/x.kicad_pro", "archive/x.kicad_pcb", "archive/x.kicad_pro", "b/y.kicad_pcb", "b/y.kicad_pro", "stray.kicad_pcb", "old/z.kicad_pcb"]);
        assert_eq!(pick(&all, ""), s(&["a/x.kicad_pcb", "b/y.kicad_pcb"])); assert_eq!(pick(&all, "b/y.kicad_pcb"), s(&["b/y.kicad_pcb", "a/x.kicad_pcb"]));
        assert_eq!(pick(&s(&["stray.kicad_pcb", "build/q.kicad_pcb"]), ""), s(&["stray.kicad_pcb"])); // none a project's own: every board outside copies
        assert_eq!(pick(&s(&["build/q.kicad_pcb", "old/z.kicad_pcb"]), ""), s(&["build/q.kicad_pcb", "old/z.kicad_pcb"])); // only copies: all of them
        assert_eq!(own(&all), s(&["a/x.kicad_pcb", "b/y.kicad_pcb"])); assert!(own(&s(&["test/files/fixtures/a.kicad_pcb"])).is_empty(), "a fixture is no project");
        assert_eq!(pick(&[], ""), Vec::<String>::new()); assert_eq!(pick(&[], "d.kicad_pcb"), s(&["d.kicad_pcb"]));
        let many: Vec<String> = (0..40).flat_map(|i| [format!("p{i:02}/b.kicad_pcb"), format!("p{i:02}/b.kicad_pro")]).collect(); assert_eq!(pick(&many, "").len(), MOST);
        assert!(copy("Archive_2/x.kicad_pcb") && copy("a/my-backups/x.kicad_pcb") && !copy("old.kicad_pcb") && !copy("a/exporter/x") == false);
    }
    #[test] fn find_prunes_and_sorts() {
        let d = std::env::temp_dir().join(format!("bp-boards-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d);
        for f in ["b/y.kicad_pcb", "a/x.kicad_pcb", "a/x.kicad_pro", ".hid/h.kicad_pcb", "node_modules/n.kicad_pcb", "extra/e.kicad_pcb", "mfg/m.kicad_pcb", "a/_autosave-x.kicad_pcb", "a/x-backup.kicad_pcb", "a/b/c/d/e/deep.kicad_pcb", "top.kicad_pro", "a/notes.txt"] { std::fs::create_dir_all(d.join(f).parent().unwrap()).unwrap(); std::fs::write(d.join(f), "").unwrap() }
        assert_eq!(find(&d), s(&["a/x.kicad_pcb", "a/x.kicad_pro", "b/y.kicad_pcb", "top.kicad_pro"]));
    }
}

//! server/kicad's `Files.read`: a project's canonical design files. `.backplane.json` ("project", "pcb",
//! "schematic") wins; else the `.kicad_pro` in the root, else the shallowest found up to four folders down
//! (copies folders last, hidden folders, `*-backups` and node_modules skipped; a worktree under `.backplane` is not the project's).
use std::path::Path;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Files { pub pro: String, pub pcb: String, pub sch: String, pub all: Vec<String> }

/// Boards.copies on a lowercased folder name.
pub fn copies(l: &str) -> bool {
    l.starts_with("archive") || l.starts_with("build") || l.contains("backup") || matches!(l, "old" | "out" | "tmp" | "temp" | "staging" | "scratch" | "renders") || l.starts_with("output") || l.starts_with("export")
}
/// Files.found: every .kicad_pro, shallowest first after the copies flag, byte order, 50 at most.
pub fn found(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, rel: &str, depth: usize, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
            let Ok(ft) = e.file_type() else { continue };
            let p = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if ft.is_dir() { if depth < 4 && !name.starts_with('.') && name != "node_modules" && !name.ends_with("-backups") { walk(&e.path(), &p, depth + 1, out) } }
            else if name.ends_with(".kicad_pro") { out.push(p) } // find -type f/other: a link to a file is not -type d, so it prints too
        }
    }
    let mut v = vec![]; walk(root, "", 1, &mut v);
    let key = |p: &String| { let parts: Vec<&str> = p.split('/').collect(); let c = parts[..parts.len() - 1].iter().any(|d| copies(&d.to_lowercase())); (c as u8, parts.len(), p.clone()) };
    v.sort_by_key(key); v.truncate(50); v
}
fn names(dir: &Path) -> Vec<String> { let mut v: Vec<String> = std::fs::read_dir(dir).map(|r| r.flatten().filter_map(|e| e.file_name().to_str().map(str::to_string)).collect()).unwrap_or_default(); v.sort(); v }
fn dir_of(p: &str) -> &str { p.rsplit_once('/').map(|(d, _)| d).unwrap_or("") }
fn last_of(p: &str) -> &str { p.rsplit('/').next().unwrap_or(p) }
fn abs(root: &str, rel: &str) -> String {
    if rel.is_empty() { return String::new() }
    if crate::plat::fs::is_abs(rel) { return rel.into() }
    if root.contains('\\') && !root.contains('/') {
        format!("{}\\{}", root.trim_end_matches('\\'), rel.replace('/', "\\"))
    } else {
        format!("{root}/{rel}")
    }
}
fn pick(ns: &[String], ext: &str) -> String { ns.iter().find(|n| n.ends_with(ext)).cloned().unwrap_or_default() }
fn named(ns: &[String], stem: &str, ext: &str) -> String { let w = format!("{stem}{ext}"); if ns.contains(&w) { w } else { pick(ns, ext) } }
fn in_dir(dir: &str, name: &str) -> String { if dir.is_empty() || name.is_empty() { name.into() } else { format!("{dir}/{name}") } }

/// Files.altium_found: every Altium project (.PrjPcb, any case), as found() orders KiCad's.
pub fn altium_found(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, rel: &str, depth: usize, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let Some(name) = e.file_name().to_str().map(str::to_string) else { continue };
            let Ok(ft) = e.file_type() else { continue };
            let p = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if ft.is_dir() { if depth < 4 && !name.starts_with('.') && name != "node_modules" && !name.starts_with("__") && !copies(&name.to_lowercase()) { walk(&e.path(), &p, depth + 1, out) } }
            else if name.to_lowercase().ends_with(".prjpcb") { out.push(p) }
        }
    }
    let mut v = vec![]; walk(root, "", 1, &mut v);
    v.sort_by_key(|p| (p.split('/').count(), p.clone())); v.truncate(50); v
}
/// Files.altium: an Altium project's board (its first .PcbDoc) and top sheet (the .SchDoc no other sheet of it places)
fn altium(root: &str, pro: &str, pcb: &str, sch: &str) -> Files {
    use crate::files::altium::{prj, sch as asch, Kind};
    let text = std::fs::read(abs(root, pro)).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let d = dir_of(pro);
    let pcb = if pcb.is_empty() { prj::of_kind(&text, Kind::Board).first().map(|f| in_dir(d, f)).unwrap_or_default() } else { pcb.to_string() };
    let sch = if !sch.is_empty() { sch.to_string() } else {
        let sheets: Vec<String> = prj::of_kind(&text, Kind::Schematic).into_iter().map(|f| in_dir(d, &f)).collect();
        let lower = |s: &str| s.rsplit('/').next().unwrap_or(s).to_lowercase();
        let placed: Vec<String> = sheets.iter().flat_map(|f| std::fs::read(abs(root, f)).map(|b| asch::sheets(&b)).unwrap_or_default()).map(|(_, f)| lower(&f.replace('\\', "/"))).collect();
        sheets.iter().find(|f| !placed.contains(&lower(f))).or(sheets.first()).cloned().unwrap_or_default()
    };
    Files { pro: abs(root, pro), pcb: abs(root, &pcb), sch: abs(root, &sch), all: vec![] }
}
/// Files.is_altium: the project is an Altium one (its files are read by altium/, edited through Altium)
pub fn is_altium(f: &Files) -> bool { f.pro.to_lowercase().ends_with(".prjpcb") }

pub fn read(root: &str) -> Files {
    let cfg: serde_json::Value = std::fs::read_to_string(format!("{root}/.backplane.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null);
    let g = |k: &str| cfg.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let all = found(Path::new(root));
    let top = pick(&names(Path::new(root)), ".kicad_pro");
    let named_pro = g("project");
    // an Altium project: named, or the folder has no KiCad project (KiCad's wins when both are there)
    if named_pro.to_lowercase().ends_with(".prjpcb") { return altium(root, &named_pro, &g("pcb"), &g("schematic")) }
    if named_pro.is_empty() && top.is_empty() && all.is_empty() {
        let ns = names(Path::new(root));
        let atop = ns.iter().find(|n| n.to_lowercase().ends_with(".prjpcb")).cloned();
        if let Some(a) = atop.or_else(|| altium_found(Path::new(root)).into_iter().next()) { return altium(root, &a, &g("pcb"), &g("schematic")) }
    }
    let pro = if !named_pro.is_empty() { named_pro } else if !top.is_empty() { top } else { all.first().cloned().unwrap_or_default() };
    let folder = if dir_of(&pro).is_empty() { root.to_string() } else { abs(root, dir_of(&pro)) };
    let ns = names(Path::new(&folder));
    let stem = last_of(&pro).strip_suffix(".kicad_pro").unwrap_or(last_of(&pro)).to_string();
    let (pcb, sch) = (g("pcb"), g("schematic"));
    Files { pro: abs(root, &pro),
        pcb: abs(root, &if pcb.is_empty() { in_dir(dir_of(&pro), &named(&ns, &stem, ".kicad_pcb")) } else { pcb }),
        sch: abs(root, &if sch.is_empty() { in_dir(dir_of(&pro), &named(&ns, &stem, ".kicad_sch")) } else { sch }), all }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(n: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::dir(&format!("kp-{n}")) }
    fn touch(p: std::path::PathBuf) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, "").unwrap() }
    #[test] fn project_in_the_root_wins_and_prefers_the_matching_name() {
        let r = tmp("a"); for f in ["x.kicad_pro", "x.kicad_pcb", "other.kicad_pcb", "x.kicad_sch", "sub/y.kicad_pro", "sub/y.kicad_pcb"] { touch(r.join(f)) }
        let f = read(r.to_str().unwrap()); let root = r.to_str().unwrap();
        assert_eq!((f.pro.as_str(), f.pcb.as_str(), f.sch.as_str()), (&format!("{root}/x.kicad_pro")[..], &format!("{root}/x.kicad_pcb")[..], &format!("{root}/x.kicad_sch")[..]));
        assert_eq!(f.all, vec!["x.kicad_pro".to_string(), "sub/y.kicad_pro".to_string()]); // find lists the root level too
    }
    #[test] fn shallowest_below_with_copies_last_and_hidden_skipped() {
        let r = tmp("b"); for f in ["archive/a.kicad_pro", "deep/er/c.kicad_pro", "hw/m.kicad_pro", "hw/m.kicad_pcb", ".backplane/worktrees/w/h.kicad_pro", "node_modules/z.kicad_pro", "k-backups/z.kicad_pro", "a/b/c/d/e/too.kicad_pro"] { touch(r.join(f)) }
        let f = read(r.to_str().unwrap());
        assert_eq!(f.all, vec!["hw/m.kicad_pro", "deep/er/c.kicad_pro", "archive/a.kicad_pro"]);
        assert!(f.pro.ends_with("hw/m.kicad_pro") && f.pcb.ends_with("hw/m.kicad_pcb") && f.sch.is_empty());
    }
    #[test] fn altium_project_when_there_is_no_kicad_one() {
        let r = tmp("alt"); let root = r.to_str().unwrap().to_string();
        touch(r.join("hw/Board.PcbDoc")); touch(r.join("hw/Top.SchDoc")); touch(r.join("Output/x.PrjPcb"));
        std::fs::write(r.join("hw/Thing.PrjPcb"), "[Design]\r\n[Document1]\r\nDocumentPath=Top.SchDoc\r\n[Document2]\r\nDocumentPath=Board.PcbDoc\r\n").unwrap();
        let f = read(&root);
        assert_eq!((f.pro.clone(), f.pcb.clone(), f.sch.clone()), (format!("{root}/hw/Thing.PrjPcb"), format!("{root}/hw/Board.PcbDoc"), format!("{root}/hw/Top.SchDoc")));
        assert!(is_altium(&f));
        touch(r.join("k.kicad_pro")); let f = read(&root); assert!(f.pro.ends_with("k.kicad_pro") && !is_altium(&f), "KiCad's project wins");
    }
    #[test] fn backplane_json_names_win() {
        let r = tmp("c"); touch(r.join("x.kicad_pro")); touch(r.join("custom/b.kicad_pcb"));
        std::fs::write(r.join(".backplane.json"), r#"{"pcb":"custom/b.kicad_pcb","schematic":"s/top.kicad_sch"}"#).unwrap();
        let f = read(r.to_str().unwrap()); let root = r.to_str().unwrap();
        assert_eq!(f.pcb, format!("{root}/custom/b.kicad_pcb")); assert_eq!(f.sch, format!("{root}/s/top.kicad_sch"));
        let e = tmp("d"); let f = read(e.to_str().unwrap()); assert_eq!((f.pro.as_str(), f.pcb.as_str()), ("", ""));
        assert_eq!(abs(r"C:\bp\proj", "sub/a.kicad_pcb"), r"C:\bp\proj\sub\a.kicad_pcb");
        assert_eq!(abs(r"C:\bp\proj\", "ecc83-pp.kicad_pro"), r"C:\bp\proj\ecc83-pp.kicad_pro");
    }
}

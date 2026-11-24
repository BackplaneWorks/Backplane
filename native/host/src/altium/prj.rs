//! .PrjPcb: an INI file. `[DocumentN]` sections name the project's documents (`DocumentPath=Sheet.SchDoc`, relative to
//! the project, Windows separators); `[Design]` and others hold settings this reader ignores.

/// The project's documents in file order, as paths relative to the project with '/' separators.
pub fn documents(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_doc = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') { in_doc = l.len() > 9 && l[1..].to_ascii_lowercase().starts_with("document"); continue }
        if !in_doc { continue }
        if let Some((k, v)) = l.split_once('=') {
            if k.trim().eq_ignore_ascii_case("DocumentPath") && !v.trim().is_empty() { out.push(v.trim().replace('\\', "/")) }
        }
    }
    out
}
/// The project's parameters (`[ParameterN]` sections' `Name` and `Value`), in file order: what a sheet's special strings
/// ("=CompanyName") fall back to.
pub fn parameters(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur: Option<(String, String)> = None;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            if let Some((n, v)) = cur.take() { if !n.is_empty() { out.push((n, v)) } }
            let sec = l.trim_matches(['[', ']']).to_ascii_lowercase();
            if sec.starts_with("parameter") && sec[9..].bytes().all(|b| b.is_ascii_digit()) && sec.len() > 9 { cur = Some((String::new(), String::new())) }
            continue
        }
        let (Some(c), Some((k, v))) = (cur.as_mut(), l.split_once('=')) else { continue };
        if k.trim().eq_ignore_ascii_case("Name") { c.0 = v.trim().to_string() } else if k.trim().eq_ignore_ascii_case("Value") { c.1 = v.to_string() }
    }
    if let Some((n, v)) = cur { if !n.is_empty() { out.push((n, v)) } }
    out
}
/// How many folders above a sheet's own `find_for` looks in for its project (Altium projects keep sheets in `Sch/`, `Schematics/`...).
const UP: usize = 4;
/// The project file (.PrjPcb) that lists a sheet, if any: in the sheet's folder or one of the `UP` above it, nearest first.
/// A project in a folder above must list this very file (its `DocumentPath` joined to the project's folder is the sheet's path,
/// any case); in the sheet's own folder a project listing a sheet of that name still counts, as it always did. The walk stops after
/// a repository's or a Backplane project's root (a folder with `.git` or `.backplane.json`) and goes no higher than the folder
/// below the home folder (as its path is spelled) or Unix's `/` (a Windows drive or share root, a usual place for a project, is
/// looked in); the sheet's own folder is always looked in. An unrelated project higher up is never taken.
pub fn find_for(sheet: &str) -> Option<String> {
    use std::path::{Component, Path, PathBuf};
    let p = Path::new(sheet);
    let name = p.file_name()?.to_string_lossy().to_lowercase();
    // the lexical form (`..` and `.` folded; no file system access), and lower-cased for comparing a project's entry with the sheet
    let lex = |q: &Path| -> PathBuf {
        let mut out = PathBuf::new();
        for c in q.components() { match c { Component::CurDir => {}, Component::ParentDir => { out.pop(); }, c => out.push(c) } }
        out
    };
    let norm = |q: &Path| -> String { lex(q).to_string_lossy().to_lowercase() };
    let want = norm(p);
    let home = crate::plat::dirs::home().map(|h| norm(Path::new(&h)));
    let projects = |dir: &Path| -> Vec<(PathBuf, Vec<String>)> {
        let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
        let mut ps: Vec<_> = rd.flatten().map(|e| e.path()).filter(|q| q.extension().map_or(false, |x| x.eq_ignore_ascii_case("prjpcb"))).collect();
        ps.sort();
        ps.into_iter().filter_map(|q| std::fs::read(&q).ok().map(|b| { let d = of_kind(&String::from_utf8_lossy(&b), super::Kind::Schematic); (q, d) })).collect()
    };
    // walked up from the folder's lexical form, so `a/b/../c/` goes up through `a`, not `a/b`
    let own = lex(p.parent()?);
    if own.as_os_str().is_empty() { return None }
    let mut fallback = None;
    let mut cur = Some(own.as_path());
    for level in 0..=UP {
        let Some(dir) = cur.filter(|d| !d.as_os_str().is_empty()) else { break };
        if level > 0 && (Some(norm(dir)) == home || cfg!(unix) && dir.parent().is_none()) { break }
        for (q, docs) in projects(dir) {
            if docs.iter().any(|d| norm(&dir.join(d)) == want) { return Some(q.to_string_lossy().into_owned()) }
            if dir == own && fallback.is_none() && docs.iter().any(|d| d.rsplit('/').next().unwrap_or(d).to_lowercase() == name) { fallback = Some(q) }
        }
        if dir.join(".git").exists() || dir.join(".backplane.json").exists() { break }
        cur = dir.parent();
    }
    fallback.map(|q| q.to_string_lossy().into_owned())
}
static ROOTS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, Vec<String>)>>> = std::sync::OnceLock::new();
/// The project's sheets no other sheet of it places (its top-level sheets: the top of a hierarchy, and every sheet of a flat design), in
/// the project's order, as paths in the project's folder. Kept 10 s (the sheet tree and the BOM ask while they walk).
pub fn roots(prj: &str) -> Vec<String> {
    let m = ROOTS.get_or_init(Default::default);
    if let Some((at, v)) = m.lock().unwrap().get(prj) { if at.elapsed() < std::time::Duration::from_secs(10) { return v.clone() } }
    let text = std::fs::read(prj).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let dir = prj.rfind(['/', '\\']).map(|i| &prj[..i]).unwrap_or("");
    let sep = if prj.contains('/') { "/" } else { "\\" };
    let sheets: Vec<String> = of_kind(&text, super::Kind::Schematic).into_iter().map(|d| if dir.is_empty() { d } else { format!("{dir}{sep}{}", if sep == "/" { d } else { d.replace('/', "\\") }) }).collect();
    let base = |s: &str| { let l = s.rsplit(['/', '\\']).next().unwrap_or(s).to_lowercase(); if l.ends_with(".schdoc") { l } else { format!("{l}.schdoc") } };
    let placed: std::collections::HashSet<String> = sheets.iter().flat_map(|f| std::fs::read(f).map(|b| super::sch::sheets(&b)).unwrap_or_default()).map(|(_, f)| base(&f)).collect();
    let v: Vec<String> = sheets.into_iter().filter(|s| !placed.contains(&base(s))).collect();
    m.lock().unwrap().insert(prj.to_string(), (std::time::Instant::now(), v.clone()));
    v
}
static PLACED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, std::collections::HashMap<String, Vec<String>>)>>> = std::sync::OnceLock::new();
/// The names of the sheet symbols of the project that place `sheet` (a file name, any case, with or without `.SchDoc`), in the
/// project's order: a sheet placed twice is two channels ("CH1", "CH2"). Kept 10 s, like `roots`.
pub fn placements(prj: &str, sheet: &str) -> Vec<String> {
    let base = |s: &str| { let l = s.rsplit(['/', '\\']).next().unwrap_or(s).to_lowercase(); if l.ends_with(".schdoc") { l } else { format!("{l}.schdoc") } };
    let m = PLACED.get_or_init(Default::default);
    if let Some((at, v)) = m.lock().unwrap().get(prj) { if at.elapsed() < std::time::Duration::from_secs(10) { return v.get(&base(sheet)).cloned().unwrap_or_default() } }
    let text = std::fs::read(prj).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let dir = std::path::Path::new(prj).parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let mut by: std::collections::HashMap<String, Vec<String>> = Default::default();
    for d in of_kind(&text, super::Kind::Schematic) {
        let Ok(b) = std::fs::read(dir.join(&d)) else { continue };
        for (name, file) in super::sch::sheets(&b) { by.entry(base(&file)).or_default().push(name) }
    }
    let v = by.get(&base(sheet)).cloned().unwrap_or_default();
    m.lock().unwrap().insert(prj.to_string(), (std::time::Instant::now(), by));
    v
}
/// The documents of one kind (by extension, any case).
pub fn of_kind(text: &str, k: super::Kind) -> Vec<String> { documents(text).into_iter().filter(|d| super::kind(d) == Some(k)).collect() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn lists_documents() {
        let t = "[Design]\r\nVersion=1.0\r\n[Document1]\r\nDocumentPath=Top.SchDoc\r\n[Document2]\r\nDocumentPath=sub\\Power.SchDoc\r\n[Document3]\r\nDocumentPath=Board.PcbDoc\r\n[Generic_SmartPDF]\r\nDocumentPath=x.SchDoc\r\n";
        assert_eq!(documents(t), ["Top.SchDoc", "sub/Power.SchDoc", "Board.PcbDoc"]);
        assert_eq!(of_kind(t, crate::files::altium::Kind::Board), ["Board.PcbDoc"]);
    }
    #[test] fn project_parameters() {
        let t = "[Design]\r\nVersion=1.0\r\n[Parameter1]\r\nName=CompanyName\r\nValue=Example Ltd\r\n\r\n[Parameter2]\r\nName=Rev\r\nValue=\r\n[ParameterSettings]\r\nName=x\r\n[Parameter3]\r\nValue=A=B\r\nName=Note\r\n";
        assert_eq!(parameters(t), [("CompanyName".to_string(), "Example Ltd".to_string()), ("Rev".to_string(), String::new()), ("Note".to_string(), "A=B".to_string())]);
    }
    #[test] fn find_for_in_parent_folder() {
        // a sheet in Sch/ under the project's folder (issue #1381); an unrelated project two folders up that lists a sheet of the same
        // name elsewhere is not taken, and nothing is found above a repository's root
        let t = crate::plat::tmp::dir("alt-find");
        let d: &std::path::Path = &t;
        let top = d.join("ws");
        let prj = top.join("Board");
        std::fs::create_dir_all(prj.join("Sch")).unwrap();
        std::fs::write(prj.join("Sch").join("Power.SchDoc"), b"").unwrap();
        std::fs::write(prj.join("Board.PrjPcb"), "[Document1]\r\nDocumentPath=Sch\\power.SchDoc\r\n[Document2]\r\nDocumentPath=Board.PcbDoc\r\n").unwrap();
        std::fs::write(top.join("Other.PrjPcb"), "[Document1]\r\nDocumentPath=Elsewhere\\Power.SchDoc\r\n").unwrap();
        let sheet = prj.join("Sch").join("Power.SchDoc").to_string_lossy().into_owned();
        assert_eq!(find_for(&sheet).as_deref(), Some(prj.join("Board.PrjPcb").to_string_lossy().as_ref()));
        // the project lists it by name only, from another folder: not this sheet's project
        std::fs::write(prj.join("Board.PrjPcb"), "[Document1]\r\nDocumentPath=Old\\Power.SchDoc\r\n").unwrap();
        assert_eq!(find_for(&sheet), None);
        // a project above a repository's root is never reached
        std::fs::write(top.join("Other.PrjPcb"), "[Document1]\r\nDocumentPath=Board\\Sch\\Power.SchDoc\r\n").unwrap();
        assert_eq!(find_for(&sheet).as_deref(), Some(top.join("Other.PrjPcb").to_string_lossy().as_ref()));
        std::fs::create_dir_all(prj.join(".git")).unwrap();
        assert_eq!(find_for(&sheet), None);
        // in the sheet's own folder, a project naming the sheet from another folder still counts (as before)
        std::fs::write(prj.join("Sch").join("Local.PrjPcb"), "[Document1]\r\nDocumentPath=..\\Old\\Power.SchDoc\r\n").unwrap();
        assert_eq!(find_for(&sheet).as_deref(), Some(prj.join("Sch").join("Local.PrjPcb").to_string_lossy().as_ref()));
    }
    #[test] fn find_for_stops_where_it_should() {
        let t = crate::plat::tmp::dir("alt-find-up");
        let d: &std::path::Path = &t;
        // a sheet five folders below a project that lists it: four folders up are looked in, not five
        let deep = d.join("p").join("a").join("b").join("c").join("e").join("f");
        std::fs::create_dir_all(&deep).unwrap();
        let sheet = deep.join("S.SchDoc").to_string_lossy().into_owned();
        std::fs::write(d.join("p").join("Far.PrjPcb"), "[Document1]\r\nDocumentPath=a\\b\\c\\e\\f\\S.SchDoc\r\n").unwrap();
        assert_eq!(find_for(&sheet), None);
        std::fs::write(d.join("p").join("a").join("Near.PrjPcb"), "[Document1]\r\nDocumentPath=b\\c\\e\\f\\S.SchDoc\r\n").unwrap();
        assert_eq!(find_for(&sheet).as_deref(), Some(d.join("p").join("a").join("Near.PrjPcb").to_string_lossy().as_ref()));
        // a Backplane project's root stops the walk too, after its own folder is looked in
        std::fs::write(d.join("p").join("a").join("b").join(".backplane.json"), "{}").unwrap();
        assert_eq!(find_for(&sheet), None);
        // a project listing the sheet through `..`, and a sheet path with `..` in it, walked from its real folder
        std::fs::write(d.join("p").join("a").join("b").join("c").join("Up.PrjPcb"), "[Document1]\r\nDocumentPath=..\\c\\e\\f\\S.SchDoc\r\n").unwrap();
        let odd = d.join("p").join("a").join("b").join("c").join("x").join("..").join("e").join("f").join("S.SchDoc").to_string_lossy().into_owned();
        assert_eq!(find_for(&odd).as_deref(), Some(d.join("p").join("a").join("b").join("c").join("Up.PrjPcb").to_string_lossy().as_ref()));
    }
}

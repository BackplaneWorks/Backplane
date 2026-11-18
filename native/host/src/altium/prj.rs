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
/// The project file (.PrjPcb) in a sheet's folder that lists that sheet, if any.
pub fn find_for(sheet: &str) -> Option<String> {
    let p = std::path::Path::new(sheet);
    let (dir, name) = (p.parent()?, p.file_name()?.to_string_lossy().to_lowercase());
    let mut ps: Vec<_> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|q| q.extension().map_or(false, |x| x.eq_ignore_ascii_case("prjpcb"))).collect();
    ps.sort();
    ps.into_iter().find(|q| std::fs::read(q).map(|b| of_kind(&String::from_utf8_lossy(&b), super::Kind::Schematic).iter().any(|d| d.rsplit('/').next().unwrap_or(d).to_lowercase() == name)).unwrap_or(false))
        .map(|q| q.to_string_lossy().into_owned())
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
}

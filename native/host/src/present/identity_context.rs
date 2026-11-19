//! Source facts for the active viewer identity. Paths crossing the peer boundary are project-relative.
use crate::{files::{fm, kicad}, state::State};
use serde_json::{json, Value};
use std::path::Path;

fn safe_relative(path: &str) -> bool {
    !path.starts_with('/') && fm::files_safe(path) && !path.chars().any(char::is_control)
}

pub(crate) fn local_documents(root: &str) -> Value {
    let mut docs = json!({"board":"","sheet":"","hasBoard":false,"hasSheet":false});
    if root.is_empty() { return docs }
    let files = kicad::project::read(root);
    for (name, has, path) in [("board","hasBoard",files.pcb), ("sheet","hasSheet",files.sch)] {
        let abs = Path::new(&path);
        if let Some(rel) = abs.strip_prefix(root).ok().and_then(Path::to_str) {
            let rel = rel.replace('\\', "/");
            if !rel.is_empty() && safe_relative(&rel) {
                docs[name] = json!(rel);
                docs[has] = json!(abs.is_file());
            }
        }
    }
    docs
}

fn revision(source: &str) -> Option<&str> {
    let mut fields = source.strip_prefix("git\t")?.splitn(4, '\t');
    fields.next()?;
    let rev = fields.next()?;
    fields.next()?.parse::<usize>().ok()?;
    fields.next()?;
    (!rev.is_empty() && rev.len() <= 64 && rev.bytes().all(|b| b.is_ascii_hexdigit())).then_some(rev)
}

fn history(source: &str) -> (String, Value) {
    if let Some(diff) = source.strip_prefix("diff\n") {
        let mut lines = diff.lines();
        let endpoint = |s: &str| if s.starts_with("git\t") { revision(s) } else { (!s.is_empty()).then_some("Working copy") }.map(str::to_owned);
        if let (Some(from), Some(to)) = (lines.next().and_then(endpoint), lines.next().and_then(endpoint)) {
            return (String::new(), json!({"from":from,"to":to}))
        }
    }
    (revision(source).unwrap_or("").to_string(), Value::Null)
}

/// Directory entries come from the peer. A capped listing cannot prove an entry is absent.
pub fn far_exists(sel: &str, rel: &str) -> Option<bool> {
    if !sel.contains('~') || !safe_relative(rel) || rel.is_empty() { return None }
    let listing = crate::net::farfiles::listing(sel, fm::parent(rel).as_ref())?.ok()?;
    let mut files = 0;
    let mut dirs = 0;
    for line in listing.lines() {
        let mut fields = line.splitn(3, '\t');
        match fields.next()? { "d" => dirs += 1, "f" => files += 1, _ => return None }
        fields.next()?;
        if fields.next()? == fm::last(rel) { return Some(true) }
    }
    (dirs < crate::files::dir::MAX && files < crate::files::dir::MAX && !fm::last(rel).starts_with('.')).then_some(false)
}

/// Keep only identity fields from a successful watch. Absolute paths stay on the wire to the renderer.
pub(crate) fn watched(p: &Value) -> Option<Value> {
    let source = p["path"].as_str()?;
    let (version, comparison) = history(source);
    if version.is_empty() && comparison.is_null() { return None }
    let side = source.strip_prefix("diff\n").and_then(|s| s.lines().nth(1)).unwrap_or(source);
    let path = side.strip_prefix("git\t").and_then(|s| s.splitn(4,'\t').nth(3)).unwrap_or(side).replace('\\', "/");
    let root = p["root"].as_str()?.replace('\\', "/");
    if root.is_empty() { return None }
    let rel = path.strip_prefix(&format!("{}/", root.trim_end_matches('/'))).filter(|s| safe_relative(s))?;
    Some(json!({"kind":p["kind"],"version":version,"comparison":comparison,"historyRel":rel}))
}

/// Browser helper info is available in the presenter's hubInfo, or in session.info.
pub fn browser(info: &Value, sel: &str) -> (String, String) {
    let key = format!("browser.tabs.{sel}");
    let parsed: Value = serde_json::from_str(info[&key].as_str().unwrap_or("")).unwrap_or(Value::Null);
    let Some(tab) = parsed["tabs"].as_array().and_then(|tabs| tabs.iter().find(|t| t["active"] == true)) else { return Default::default() };
    let url = tab["url"].as_str().unwrap_or("");
    if !(url.starts_with("https://") || url.starts_with("http://") || url == "about:blank") { return Default::default() }
    let title = tab["title"].as_str().unwrap_or("");
    let private_path = |s: &str| s.contains("/home/") || s.contains("/Users/") || s.contains(":\\");
    let url = if private_path(url) { "" } else { url };
    let title = if private_path(title) { "" } else { title };
    (title.into(), url.into())
}

pub fn facts(st: &State, session: &Value, sel: &str) -> Value {
    let info = &session["info"];
    let drafts = &session["drafts"];
    let field = |obj: &Value, k: &str| obj[format!("{k}:{sel}")].as_str().unwrap_or("").to_string();
    let mut kind = field(info, "view.kind");
    let mut source = field(info, "view.path");
    if kind.is_empty() {
        kind = field(drafts, "@view");
        source = field(drafts, "@view.file");
        if source.is_empty() && kind == "schematic" { source = field(drafts, "@view.sheet"); }
    }
    let docs = if !matches!(kind.as_str(), "board" | "split" | "schematic" | "3d" | "simulation" | "gerbers") {
        Value::Null
    } else if sel.contains('~') {
        crate::net::farfiles::documents(sel).unwrap_or(Value::Null)
    } else {
        local_documents(&super::filelink::root_of(st, sel))
    };
    let name = |k: &str| docs[k].as_str().filter(|p| safe_relative(p)).unwrap_or("");
    let reported = &session["watchedHistory"];

    let selected = if reported["kind"] == kind { Some(reported) } else { None };
    let (version, comparison) = if matches!(kind.as_str(), "board" | "schematic" | "3d") {
        history(&source)
    } else { (String::new(), Value::Null) };
    let (title, url) = browser(info, sel);
    json!({"board":name("board"),"sheet":name("sheet"),"hasBoard":docs["hasBoard"].as_bool(),
        "hasSheet":docs["hasSheet"].as_bool(),"version":selected.map_or(json!(version), |v| v["version"].clone()),
        "comparison":selected.map_or(comparison, |v| v["comparison"].clone()),"historyRel":selected.and_then(|v| v["historyRel"].as_str()).unwrap_or(""),
        "browserTitle":title,"browserUrl":url})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_documents_keep_relative_paths_and_independent_existence() {
        let root = crate::plat::tmp::dir("identity-docs");
        std::fs::write(root.join(".backplane.json"), r#"{"pcb":"boards/main.kicad_pcb","schematic":"docs/top.kicad_sch"}"#).unwrap();
        std::fs::create_dir(root.join("boards")).unwrap();
        std::fs::write(root.join("boards/main.kicad_pcb"), "").unwrap();
        assert_eq!(local_documents(root.to_str().unwrap()), json!({
            "board":"boards/main.kicad_pcb","hasBoard":true,"sheet":"docs/top.kicad_sch","hasSheet":false
        }));
    }

    #[test]
    fn history_only_exposes_the_displayed_revisions() {
        let old = "git\t/private/project/.git\tabc123\t16\t/private/project/file";
        let new = old.replace("abc123", "def456");
        let request = json!({"kind":"board","root":"/private/project","path":old});
        assert_eq!(watched(&request).unwrap()["historyRel"], "file");
        assert_eq!(history(old), ("abc123".into(), Value::Null));
        assert_eq!(history(&format!("diff\n{old}\n{new}\n4")).1, json!({"from":"abc123","to":"def456"}));
        assert_eq!(history(&format!("diff\n{old}\n/private/project/file")).1["to"], "Working copy");
        assert_eq!(history("/private/project/file"), (String::new(), Value::Null));
    }

    #[test]
    fn active_tab_supplies_both_browser_fields() {
        let info = json!({"browser.tabs.t":json!({"tabs":[{"title":"Other"},{"title":"Data","url":"https://example.com","active":true}]}).to_string()});
        assert_eq!(browser(&info, "t"), ("Data".into(), "https://example.com".into()));
    }
}

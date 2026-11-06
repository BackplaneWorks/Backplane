//! The Add project folder picker's picture (`shown.folders`, which the client draws): a path field, the folders under it, ".." and an Add or Create row.
//! The session holds only `@proj.open` and `@proj.text` (commands/actions.rs); the rows are made here from the folder on disk. Values are absolute paths
//! (`~` written out), so the request that follows needs no guessing. Pure but for reading one directory.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn expand(text: &str, home: &str) -> String {
    if text == "~" { home.to_string() } else if let Some(r) = text.strip_prefix("~/") { format!("{}/{}", home.trim_end_matches('/'), r) } else { text.to_string() }
}
/// (the folder being typed in, the part of a name typed so far): "~/pro" is ("<home>/", "pro"); "~/proj/" is ("<home>/proj/", "").
fn split(expanded: &str) -> (String, String) {
    if expanded.ends_with('/') { return (expanded.to_string(), String::new()) }
    match expanded.rfind('/') { Some(i) => (expanded[..=i].to_string(), expanded[i + 1..].to_string()), None => ("/".to_string(), expanded.to_string()) }
}
fn parent(dir: &str) -> String {
    let t = dir.trim_end_matches('/');
    match t.rfind('/') { Some(0) | None => "/".to_string(), Some(i) => format!("{}/", &t[..i]) }
}
fn name_ok(n: &str) -> bool { !n.is_empty() && n != "." && n != ".." && !n.contains('/') && !n.contains('\0') }

pub fn folders(drafts: &Value, home: &str, roots: &[&str]) -> Value {
    if drafts["@proj.open"].as_str() != Some("1") { return Value::Null }
    let text = drafts["@proj.text"].as_str().filter(|t| !t.is_empty()).unwrap_or("~/").to_string();
    let x = expand(&text, home);
    let (dir, part) = split(&x);
    let mut names: Vec<String> = std::fs::read_dir(&dir).map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).filter_map(|e| e.file_name().into_string().ok()).collect()).unwrap_or_default();
    names.retain(|n| (part.starts_with('.') || !n.starts_with('.')) && n.to_lowercase().starts_with(&part.to_lowercase()));
    names.sort_by_key(|n| n.to_lowercase()); names.truncate(200);
    let mut items: Vec<Value> = vec![];
    let whole = x.trim_end_matches('/'); let whole = if whole.is_empty() { "/" } else { whole };
    if Path::new(whole).is_dir() {
        if roots.iter().any(|r| r.trim_end_matches('/') == whole) { items.push(json!({"label": format!("{whole} is a project already"), "action": "", "value": "", "kind": "note"})) }
        else { items.push(json!({"label": format!("Add {whole}"), "action": "proj-add", "value": whole, "kind": "add"})) }
    } else if name_ok(&part) && Path::new(&dir).is_dir() {
        items.push(json!({"label": format!("Create {}{}", dir, part), "action": "proj-new", "value": format!("{dir}{part}"), "kind": "add"}));
    }
    if dir != "/" { items.push(json!({"label": "..", "action": "proj-cd", "value": parent(&dir), "kind": "up"})) }
    for n in names { items.push(json!({"label": format!("{n}/"), "action": "proj-cd", "value": format!("{dir}{n}/"), "kind": "dir"})) }
    let error = if Path::new(&dir).is_dir() { String::new() } else { format!("{dir} is not a folder") };
    json!({"text": text, "hint": "Path of a folder, e.g. ~/projects/board", "error": error, "items": items})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn open(t: &str) -> Value { json!({"@proj.open": "1", "@proj.text": t}) }
    fn d(tag: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-picker-{}-{tag}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(p.join("alpha/inner")).unwrap(); std::fs::create_dir_all(p.join("beta")).unwrap(); std::fs::create_dir_all(p.join(".hidden")).unwrap(); p }
    #[test] fn closed_is_null_and_open_lists_folders_with_add_and_up() {
        assert!(folders(&json!({}), "/h", &[]).is_null());
        let home = d("list"); let h = home.to_str().unwrap();
        let f = folders(&open("~/"), h, &[]);
        let labels: Vec<&str> = f["items"].as_array().unwrap().iter().map(|i| i["label"].as_str().unwrap()).collect();
        assert_eq!(labels[0], format!("Add {h}")); assert_eq!(&labels[1..], ["..", "alpha/", "beta/"], "hidden folders are not listed");
        let alpha = &f["items"][2]; assert_eq!((alpha["action"].as_str(), alpha["value"].as_str().unwrap()), (Some("proj-cd"), format!("{h}/alpha/").as_str()));
    }
    #[test] fn a_typed_name_filters_and_a_new_one_can_be_created_or_a_project_is_noted() {
        let home = d("typed"); let h = home.to_str().unwrap();
        let f = folders(&open("~/al"), h, &[]); let items = f["items"].as_array().unwrap();
        assert_eq!(items[0]["action"], "proj-new"); assert_eq!(items[0]["value"], format!("{h}/al")); assert!(items.iter().any(|i| i["label"] == "alpha/") && !items.iter().any(|i| i["label"] == "beta/"));
        let f = folders(&open("~/alpha"), h, &[&format!("{h}/alpha")]); assert_eq!(f["items"][0]["action"], ""); assert!(f["items"][0]["label"].as_str().unwrap().contains("already"));
        let f = folders(&open("~/nope/x"), h, &[]); assert!(f["error"].as_str().unwrap().contains("not a folder")); assert!(f["items"].as_array().unwrap().iter().all(|i| i["action"] != "proj-add" && i["action"] != "proj-new"));
    }
    #[test] fn up_stops_at_the_root_and_names_are_checked() {
        assert_eq!(parent("/a/b/"), "/a/"); assert_eq!(parent("/a/"), "/"); assert_eq!(parent("/"), "/");
        assert!(name_ok("x") && !name_ok("..") && !name_ok("a/b") && !name_ok(""));
    }
}

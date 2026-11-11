//! The Add project folder picker's picture (`shown.folders`, which the client draws): a path field, the folders under it, ".." and an Add or Create row.
//! The session holds only `@proj.open` and `@proj.text` (commands/actions.rs); the rows are made here from the folder on disk. Values are absolute paths
//! (`~` written out), so the request that follows needs no guessing. Pure but for reading that folder, which is kept (`Cache`) so a presentation that changes nothing reads no directory.
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The field opens holding "~/", so a whole path pasted or typed into it lands behind that: "~//home/me/board", "~/~/board". A path that
/// starts again after the "~/" is the path meant (as a shell's file prompt reads "//"), not a folder under the home.
fn expand(text: &str, home: &str) -> String {
    if text == "~" { home.to_string() }
    else if let Some(r) = text.strip_prefix("~/") { if r.starts_with('/') || r == "~" || r.starts_with("~/") || crate::plat::fs::is_abs(r) { expand(r, home) } else { format!("{}/{}", home.trim_end_matches('/'), r) } }
    else { text.to_string() }
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
/// A path as the person reads it: under the home it is written "~/..." (the field and the labels; the values stay absolute).
fn tilde(p: &str, home: &str) -> String {
    let h = home.trim_end_matches('/');
    if h.is_empty() { return p.to_string() }
    if p == h { "~".to_string() } else if let Some(r) = p.strip_prefix(h).filter(|r| r.starts_with('/')) { format!("~{r}") } else { p.to_string() }
}
fn name_ok(n: &str) -> bool { !n.is_empty() && n != "." && n != ".." && !n.contains('/') && !n.contains('\0') }

/// One folder's sub-folders as last read: every name (hidden ones too, sorted without case), the folder's mtime then, and the tags found so far.
struct Listing { mtime: Option<std::time::SystemTime>, at: std::time::Instant, names: std::sync::Arc<Vec<String>>, tags: std::collections::HashMap<String, &'static str> }
/// The listings kept between presentations (the picker is drawn again on every one while it is open): a folder is read from disk once, then
/// again only when its mtime moved or `TTL` passed (a `.git` made inside a child does not move the parent's mtime). Asking costs one stat.
#[derive(Default)]
pub struct Cache { dirs: std::collections::HashMap<String, Listing>, #[cfg(test)] reads: usize }
const TTL: std::time::Duration = std::time::Duration::from_secs(20);
const KEPT: usize = 16;
/// How many of a child's entries are looked at for its tag.
const PEEK: usize = 256;
impl Cache {
    fn fresh(&mut self, dir: &str) -> &mut Listing {
        let mtime = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
        let ok = self.dirs.get(dir).map_or(false, |l| l.mtime == mtime && l.at.elapsed() < TTL);
        if !ok {
            if self.dirs.len() >= KEPT && !self.dirs.contains_key(dir) { if let Some(old) = self.dirs.iter().min_by_key(|(_, l)| l.at).map(|(k, _)| k.clone()) { self.dirs.remove(&old); } }
            #[cfg(test)] { self.reads += 1; }
            self.dirs.insert(dir.to_string(), Listing { mtime, at: std::time::Instant::now(), names: std::sync::Arc::new(read_names(dir)), tags: Default::default() });
        }
        self.dirs.get_mut(dir).expect("just put")
    }
    /// The sub-folders of `dir` (ending in "/").
    fn names(&mut self, dir: &str) -> std::sync::Arc<Vec<String>> { self.fresh(dir).names.clone() }
    /// Forget `dir`'s listing when `name` is a folder in it that the listing lacks (made just now, as Create does: a folder whose mtime has a
    /// coarse clock may not have moved), so it is read again.
    fn check(&mut self, dir: &str, name: &str) {
        if name_ok(name) && self.dirs.get(dir).map_or(false, |l| !l.names.iter().any(|n| n == name)) && Path::new(&format!("{dir}{name}")).is_dir() { self.dirs.remove(dir); }
    }
    /// What the child `name` of `dir` looks like: "KiCad", "git" or "".
    fn tag(&mut self, dir: &str, name: &str) -> &'static str {
        let l = self.fresh(dir);
        if let Some(t) = l.tags.get(name) { return t }
        let t = look(&format!("{dir}{name}")); l.tags.insert(name.to_string(), t); t
    }
}
/// The folders in `dir`: the entry's own type says it (no stat), and only a link is followed.
fn read_names(dir: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).map(|rd| rd.filter_map(|e| e.ok()).filter(|e| match e.file_type() { Ok(t) if t.is_dir() => true, Ok(t) if t.is_symlink() => e.path().is_dir(), _ => false })
        .filter_map(|e| e.file_name().into_string().ok()).collect()).unwrap_or_default();
    names.sort_by_cached_key(|n| n.to_lowercase()); names
}
/// A folder that looks like a project: it holds a KiCad project ("KiCad") or a repository ("git"). One read of it, its first `PEEK` entries.
fn look(path: &str) -> &'static str {
    let mut git = false;
    for e in std::fs::read_dir(path).into_iter().flatten().filter_map(|e| e.ok()).take(PEEK) {
        let n = e.file_name(); let n = n.to_string_lossy();
        if n.ends_with(".kicad_pro") { return "KiCad" }
        if n == ".git" { git = true }
    }
    if git { "git" } else { "" }
}
/// How many rows of folders are sent; the rest are counted (`more`).
const ROWS: usize = 200;
/// The names shown for what is typed: those that start with it (case aside), hidden ones only once a dot is typed; at most `ROWS`, and how many match in all.
fn shown<'a>(names: &'a [String], part: &str) -> (Vec<&'a str>, usize) {
    let p = part.to_lowercase();
    let mut all = names.iter().filter(|n| (part.starts_with('.') || !n.starts_with('.')) && n.to_lowercase().starts_with(&p));
    let out: Vec<&str> = all.by_ref().take(ROWS).map(String::as_str).collect();
    let n = out.len() + all.count(); (out, n)
}

pub fn folders(drafts: &Value, home: &str, roots: &[&str]) -> Value {
    if drafts["@proj.open"].as_str() != Some("1") { return Value::Null }
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Cache>> = std::sync::OnceLock::new();
    let mut c = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    folders_in(&mut c, drafts, home, roots)
}
fn folders_in(cache: &mut Cache, drafts: &Value, home: &str, roots: &[&str]) -> Value {
    let text = drafts["@proj.text"].as_str().filter(|t| !t.is_empty()).unwrap_or("~/").to_string();
    let x = expand(&text, home);
    let (dir, part) = split(&x);
    cache.check(&dir, &part);
    let all = cache.names(&dir);
    let (names, total) = shown(&all, &part);
    let t = |p: &str| tilde(p, home);
    let more = total - names.len();
    let mut items: Vec<Value> = vec![];
    let whole = x.trim_end_matches('/'); let whole = if whole.is_empty() { "/" } else { whole };
    let is_root = |p: &str| roots.iter().any(|r| r.trim_end_matches('/') == p);
    // Clone it here (the machine picker: `@proj.clone` holds the remote): the folder is confirmed first; a folder that is there is taken only when it
    // is that repository (it is added as it is), never cloned into
    let clone = drafts["@proj.clone"].as_str().unwrap_or("");
    if !clone.is_empty() {
        if Path::new(whole).exists() {
            if crate::mirror::origin_of_root(whole) == clone { items.push(json!({"label": format!("Add {} (already a clone of {clone})", t(whole)), "action": "proj-add", "value": whole, "kind": "add"})) }
            else { items.push(json!({"label": format!("{} is there and holds something else: pick another folder", t(whole)), "action": "", "value": "", "kind": "note"})) }
        } else if name_ok(&part) && Path::new(&dir).is_dir() {
            items.push(json!({"label": format!("Clone {clone} into {}", t(&format!("{dir}{part}"))), "action": "proj-clone-go", "value": format!("{clone}\u{1f}{dir}{part}"), "kind": "add"}));
        }
    } else if Path::new(whole).is_dir() {
        if is_root(whole) { items.push(json!({"label": format!("{} is a project already", t(whole)), "action": "", "value": "", "kind": "note"})) }
        else { items.push(json!({"label": format!("Add {}", t(whole)), "action": "proj-add", "value": whole, "kind": "add"})) }
    } else if name_ok(&part) && Path::new(&dir).is_dir() {
        items.push(json!({"label": format!("Create {}", t(&format!("{dir}{part}"))), "action": "proj-new", "value": format!("{dir}{part}"), "kind": "add"}));
    }
    // `field`: what the path field shows on going into the row (the "~/" form); `tag`: a folder that is a project here already, or looks like
    // one (older clients draw the rows without either)
    if dir != "/" { let up = parent(&dir); items.push(json!({"label": "..", "action": "proj-cd", "field": t(&up), "value": up, "kind": "up"})) }
    for n in names {
        let tag = if is_root(&format!("{dir}{n}")) { "project" } else { cache.tag(&dir, n) };
        let v = format!("{dir}{n}/");
        let mut it = json!({"label": format!("{n}/"), "action": "proj-cd", "field": t(&v), "value": v, "kind": "dir"});
        if !tag.is_empty() { it["tag"] = json!(tag); }
        items.push(it);
    }
    let error = if Path::new(&dir).is_dir() { String::new() } else { format!("{} is not a folder", t(&dir)) };
    let hint = if clone.is_empty() { "Path of a folder, e.g. ~/projects/board".to_string() } else { format!("Where to clone {clone}") };
    // `more`: the folders that match past the rows sent (typing narrows them)
    json!({"text": text, "hint": hint, "error": error, "items": items, "more": more})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn open(t: &str) -> Value { json!({"@proj.open": "1", "@proj.text": t}) }
    fn d(tag: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-picker-{}-{tag}", std::process::id())); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(p.join("alpha/inner")).unwrap(); std::fs::create_dir_all(p.join("beta")).unwrap(); std::fs::create_dir_all(p.join(".hidden")).unwrap(); p }
    #[test] fn clone_it_here_confirms_the_folder_and_refuses_one_that_holds_something_else() {
        let home = d("clone"); let h = home.to_str().unwrap();
        let c = |t: &str| folders(&json!({"@proj.open": "1", "@proj.text": t, "@proj.clone": "github.com/o/r"}), h, &[]);
        let f = c("~/r"); assert_eq!((f["items"][0]["action"].as_str(), f["items"][0]["value"].as_str().unwrap()), (Some("proj-clone-go"), format!("github.com/o/r\u{1f}{h}/r").as_str()));
        assert_eq!(f["hint"], "Where to clone github.com/o/r");
        let f = c("~/alpha"); assert_eq!((f["items"][0]["kind"].as_str(), f["items"][0]["action"].as_str()), (Some("note"), Some("")), "a folder that is there and is not that repository");
        std::fs::create_dir_all(home.join("beta/.git")).unwrap(); std::fs::write(home.join("beta/.git/config"), "[remote \"origin\"]\n\turl = git@github.com:o/r.git\n").unwrap();
        let f = c("~/beta"); assert_eq!(f["items"][0]["action"], "proj-add", "already a clone of it: added as it is");
        let _ = std::fs::remove_dir_all(&home);
    }
    #[test] fn a_listing_is_read_once_until_the_folder_changes_and_links_to_folders_count() {
        let home = d("cache"); let h = home.to_str().unwrap(); let mut c = Cache::default();
        std::fs::write(home.join("file.txt"), "x").unwrap();
        #[cfg(unix)] std::os::unix::fs::symlink(home.join("beta"), home.join("link")).unwrap();
        let labels = |f: &Value| f["items"].as_array().unwrap().iter().filter(|i| i["kind"] == "dir").map(|i| i["label"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        let f = folders_in(&mut c, &open("~/"), h, &[]); assert_eq!(c.reads, 1);
        #[cfg(unix)] assert_eq!(labels(&f), ["alpha/", "beta/", "link/"], "a link to a folder is a folder; a file is not");
        for t in ["~/", "~/a", "~/al", "~/.h"] { folders_in(&mut c, &open(t), h, &[]); }
        assert_eq!(c.reads, 1, "typing in the same folder reads nothing again");
        std::thread::sleep(std::time::Duration::from_millis(30)); std::fs::create_dir_all(home.join("gamma")).unwrap();
        let f = folders_in(&mut c, &open("~/"), h, &[]); assert_eq!(c.reads, 2, "the folder's mtime moved"); assert!(labels(&f).contains(&"gamma/".to_string()));
        for i in 0..KEPT + 4 { let p = home.join(format!("alpha/n{i}")); std::fs::create_dir_all(&p).unwrap(); c.names(&format!("{}/", p.to_str().unwrap())); }
        assert!(c.dirs.len() <= KEPT, "the cache stays small");
        let _ = std::fs::remove_dir_all(&home);
    }
    #[test] fn folders_that_are_or_look_like_projects_carry_a_tag() {
        let home = d("tags"); let h = home.to_str().unwrap(); let mut c = Cache::default();
        std::fs::create_dir_all(home.join("alpha/.git")).unwrap(); std::fs::write(home.join("alpha/board.kicad_pro"), "{}").unwrap();
        std::fs::create_dir_all(home.join("beta/.git")).unwrap(); std::fs::create_dir_all(home.join("gamma")).unwrap(); std::fs::create_dir_all(home.join("mine")).unwrap();
        let f = folders_in(&mut c, &open("~/"), h, &[&format!("{h}/mine/")]);
        let tags: Vec<(&str, Option<&str>)> = f["items"].as_array().unwrap().iter().filter(|i| i["kind"] == "dir").map(|i| (i["label"].as_str().unwrap(), i["tag"].as_str())).collect();
        assert_eq!(tags, [("alpha/", Some("KiCad")), ("beta/", Some("git")), ("gamma/", None), ("mine/", Some("project"))]);
        assert!(f["items"][0].get("tag").is_none() && f["items"][0]["kind"] == "add", "the other rows are as they were");
        assert_eq!(look(&format!("{h}/nope")), "");
        let _ = std::fs::remove_dir_all(&home);
    }
    #[test] fn closed_is_null_and_open_lists_folders_with_add_and_up() {
        assert!(folders(&json!({}), "/h", &[]).is_null());
        let home = d("list"); let h = home.to_str().unwrap();
        let f = folders(&open("~/"), h, &[]);
        let labels: Vec<&str> = f["items"].as_array().unwrap().iter().map(|i| i["label"].as_str().unwrap()).collect();
        assert_eq!(labels[0], "Add ~", "paths under the home read \"~/\""); assert_eq!(&labels[1..], ["..", "alpha/", "beta/"], "hidden folders are not listed");
        let alpha = &f["items"][2]; assert_eq!((alpha["action"].as_str(), alpha["value"].as_str().unwrap(), alpha["field"].as_str()), (Some("proj-cd"), format!("{h}/alpha/").as_str(), Some("~/alpha/")));
        let f = folders(&open("~/alpha/"), h, &[]); assert_eq!((f["items"][0]["label"].as_str(), f["items"][1]["field"].as_str()), (Some("Add ~/alpha"), Some("~/")), "the Add row and \"..\" in the \"~/\" form");
        assert_eq!(f["more"], 0);
        assert_eq!(tilde("/h/x", "/h"), "~/x"); assert_eq!(tilde("/h", "/h/"), "~"); assert_eq!(tilde("/hx/y", "/h"), "/hx/y", "only the home itself"); assert_eq!(tilde("/a", ""), "/a");
    }
    #[test] fn a_typed_name_filters_and_a_new_one_can_be_created_or_a_project_is_noted() {
        let home = d("typed"); let h = home.to_str().unwrap();
        let f = folders(&open("~/al"), h, &[]); let items = f["items"].as_array().unwrap();
        assert_eq!(items[0]["action"], "proj-new"); assert_eq!(items[0]["value"], format!("{h}/al")); assert_eq!(items[0]["label"], "Create ~/al"); assert!(items.iter().any(|i| i["label"] == "alpha/") && !items.iter().any(|i| i["label"] == "beta/"));
        let f = folders(&open("~/alpha"), h, &[&format!("{h}/alpha")]); assert_eq!(f["items"][0]["action"], ""); assert!(f["items"][0]["label"].as_str().unwrap().contains("already"));
        let f = folders(&open("~/nope/x"), h, &[]); assert!(f["error"].as_str().unwrap().contains("not a folder")); assert!(f["items"].as_array().unwrap().iter().all(|i| i["action"] != "proj-add" && i["action"] != "proj-new"));
    }
    #[test] fn a_whole_path_pasted_behind_the_fields_own_tilde_is_the_path_meant() {
        let home = d("pasted"); let h = home.to_str().unwrap();
        let f = folders(&open(&format!("~/{h}/alpha")), h, &[]);
        assert_eq!(f["error"], ""); assert_eq!((f["items"][0]["action"].as_str(), f["items"][0]["value"].as_str().unwrap()), (Some("proj-add"), format!("{h}/alpha").as_str()), "\"~//abs/path\" is /abs/path");
        let f = folders(&open("~/~/beta"), h, &[]); assert_eq!(f["items"][0]["value"], format!("{h}/beta"), "and a second ~/ is the home once");
        let f = folders(&open(&format!("~/{h}/new-board")), h, &[]); assert_eq!((f["items"][0]["action"].as_str(), f["items"][0]["value"].as_str().unwrap()), (Some("proj-new"), format!("{h}/new-board").as_str()));
        assert_eq!(expand("~/x", "/h"), "/h/x"); assert_eq!(expand("~//x", "/h"), "/x"); assert_eq!(expand("~", "/h"), "/h");
    }
    #[test] fn a_folder_made_since_the_listing_is_seen_at_once_and_past_200_rows_are_counted() {
        let home = d("made"); let h = home.to_str().unwrap(); let mut c = Cache::default();
        folders_in(&mut c, &open("~/"), h, &[]); assert_eq!(c.reads, 1);
        // the listing is still within its time and the clock may not move the mtime: a folder named in the field that the listing lacks is read again
        std::fs::create_dir_all(home.join("fresh")).unwrap(); c.dirs.get_mut(&format!("{h}/")).unwrap().mtime = std::fs::metadata(&home).unwrap().modified().ok();
        let f = folders_in(&mut c, &open("~/fresh"), h, &[]); assert!(f["items"].as_array().unwrap().iter().any(|i| i["label"] == "fresh/"), "{f}");
        folders_in(&mut c, &open("~/zz"), h, &[]); let n = c.reads; folders_in(&mut c, &open("~/zzz"), h, &[]); assert_eq!(c.reads, n, "a name that is not a folder reads nothing");
        for i in 0..230 { std::fs::create_dir_all(home.join(format!("many/d{i:03}"))).unwrap(); }
        let f = folders_in(&mut c, &open("~/many/"), h, &[]); assert_eq!((f["items"].as_array().unwrap().iter().filter(|i| i["kind"] == "dir").count(), f["more"].as_u64()), (200, Some(30)));
        let f = folders_in(&mut c, &open("~/many/d22"), h, &[]); assert_eq!(f["more"], 0);
        let _ = std::fs::remove_dir_all(&home);
    }
    #[test] fn up_stops_at_the_root_and_names_are_checked() {
        assert_eq!(parent("/a/b/"), "/a/"); assert_eq!(parent("/a/"), "/"); assert_eq!(parent("/"), "/");
        assert!(name_ok("x") && !name_ok("..") && !name_ok("a/b") && !name_ok(""));
    }
}

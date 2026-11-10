//! The agents' side of the Files tab: files_list, files_mkdir, files_rename, files_delete, files_write, files_read and files_show, the same
//! operations (and the same containment rules) as the client's `files.op` / `files.dir` (ops.rs, dir.rs). Registered with `register`.
use super::{dir, ops, root_of};
use crate::{agents::mcp::ToolSet, api::Fut, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;

pub const NAMES: &[&str] = &["files_list", "files_mkdir", "files_rename", "files_delete", "files_write", "files_read", "files_show"];
pub struct Tools(pub Arc<Hub>);

pub fn register(hub: &Arc<Hub>, ag: &Arc<crate::agents::Agents>) { ag.add_tools(Arc::new(Tools(hub.clone()))) }

fn schema(name: &str, desc: &str, props: Value, req: &[&str]) -> Value { json!({"name":name,"description":desc,"inputSchema":{"type":"object","properties":props,"required":req}}) }
fn s(d: &str) -> Value { json!({"type":"string","description":d}) }

/// The listing text ("d|f<TAB>size<TAB>name" lines) as rows.
pub fn rows(text: &str) -> Vec<Value> {
    text.lines().filter_map(|l| { let mut it = l.splitn(3, '\t'); let (k, sz, n) = (it.next()?, it.next()?, it.next()?); Some(json!({"name":n,"kind":if k == "d" {"folder"} else {"file"},"size":sz.parse::<u64>().unwrap_or(0)})) }).collect()
}

impl ToolSet for Tools {
    fn list(&self, _thread: &str) -> Vec<Value> { vec![
        schema("files_list", "List a folder of this thread's project (folders first). Same listing as the Files tab. path is from the project root, \"\" for the root.", json!({"path":s("folder, from the project root")}), &[]),
        schema("files_mkdir", "Create a folder in the project (visible in the Files tab).", json!({"path":s("new folder, from the project root")}), &["path"]),
        schema("files_rename", "Rename a file or folder in the project (same folder; to is a plain name).", json!({"path":s("existing path"),"to":s("new name")}), &["path", "to"]),
        schema("files_delete", "Delete a file or folder (recursively) in the project.", json!({"path":s("path to delete")}), &["path"]),
        schema("files_write", "Create a new text file in the project; never replaces an existing file.", json!({"path":s("new file path"),"text":s("contents")}), &["path", "text"]),
        schema("files_read", "Read the start of a text file in the project.", json!({"path":s("file path"),"max":{"type":"number","description":"bytes, default 65536"}}), &["path"]),
        schema("files_show", "Open the Files tab on a folder, for the person.", json!({"path":s("folder, from the project root")}), &[]),
    ] }
    fn owns(&self, name: &str) -> bool { NAMES.contains(&name) }
    fn call<'a>(&'a self, thread: &'a str, name: &'a str, a: &'a Value) -> Fut<'a, (bool, Value)> {
        Box::pin(async move {
            let bad = |e: String| (false, json!({"error":e}));
            let Some(root) = root_of(&self.0, thread).await else { return bad("this thread has no project folder".into()) };
            if crate::copy::on() && matches!(name, "files_mkdir" | "files_rename" | "files_delete" | "files_write") { return bad(crate::copy::HOME_ONLY.into()) }
            let g = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let (name, a2) = (name.to_string(), a.clone());
            if name == "files_show" { self.0.push_all(&json!({"t":"view","thread":thread,"path":g("path"),"kind":"files"})); return (true, json!({"shown":g("path")})) }
            // KiCad's canonical documents are main's one copy: a read/write that names them in a
            // worktree acts on main's file; renaming or deleting them is not a thread's to do
            let root_s = root.to_string_lossy().into_owned();
            if matches!(name.as_str(), "files_rename" | "files_delete") && super::kicad::canonical::is_canonical_rel(&root_s, &g("path")) {
                return bad("that is the project's canonical KiCad document on main; threads edit it through KiCad, never rename or delete it".into())
            }
            let (droot, drel) = if matches!(name.as_str(), "files_write" | "files_read") { let (r, p) = super::kicad::canonical::direct(&root_s, &g("path")); (std::path::PathBuf::from(r), p) } else { (root.clone(), g("path")) };
            let r = tokio::task::spawn_blocking(move || -> Result<Value, String> {
                let g = |k: &str| a2.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                match name.as_str() {
                    "files_list" => { let p = g("path"); if !super::fm::files_safe(&p) { return Err("that folder is not in this project".into()) } Ok(json!({"path":p,"entries":rows(&dir::list(&root, &p).map_err(String::from)?)})) }
                    "files_mkdir" => ops::mkdir(&root, &g("path")).map(|p| json!({"created":p})),
                    "files_rename" => ops::rename(&root, &g("path"), &g("to")).map(|p| json!({"renamed":p})),
                    "files_delete" => ops::delete(&root, &g("path")).map(|p| json!({"deleted":p})),
                    "files_write" => ops::write(&droot, &drel, g("text").as_bytes()).map(|p| json!({"written":p})),
                    _ => ops::read_text(&droot, &drel, a2.get("max").and_then(Value::as_u64).unwrap_or(65536).min(1 << 20) as usize).map(|t| json!({"text":t})),
                }
            }).await.unwrap_or(Err("failed".into()));
            match r { Ok(v) => (true, v), Err(e) => bad(e) }
        })
    }
}
#[cfg(test)]
mod tests { use super::*;
    #[test] fn rows_parse_the_listing() { let r = rows("d\t0\tdocs\nf\t12\ta.txt\n"); assert_eq!(r[0]["kind"], "folder"); assert_eq!(r[1]["size"], 12); assert_eq!(r[1]["name"], "a.txt"); }
}

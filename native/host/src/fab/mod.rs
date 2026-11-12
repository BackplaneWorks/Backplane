//! Fab profiles (docs/fab-profiles.md): what each fab (and its assembly service) can make, gathered from its published pages, every value
//! with its source and date; the project's fab, picked once; the view clients and agents read (`fab.profile`, the tool `fab_profile`).
//! model.rs is the data and its pure rules, store.rs where profiles are kept, project.rs the project's choice.
pub mod model;
pub mod project;
pub mod store;

use crate::{agents::mcp::ToolSet, api::{Caller, Fut, RpcHandler}, hub::Hub};
use model::{diff, sel_json, select, select_assembly, stackups, Profile};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Arc};

pub fn start(hub: &Arc<Hub>, ag: Option<&Arc<crate::agents::Agents>>) {
    hub.register_rpc(Arc::new(Rpc));
    if let Some(ag) = ag { ag.add_tools(Arc::new(Tools(hub.clone()))) }
}

/// The project's fab files (as settings::fab reads them) and the head of its KiCad board.
fn project_inputs(root: &Path) -> (Vec<(String, String)>, String) {
    let files = crate::settings::fab::find_fab(root).into_iter().map(|p| { let t = std::fs::read_to_string(root.join(&p)).unwrap_or_default(); (p, t) }).collect();
    let pcb = crate::files::kicad::project::read(&root.to_string_lossy()).pcb;
    let head = std::fs::File::open(&pcb).ok().map(|f| { use std::io::Read; let mut s = Vec::new(); let _ = f.take(256 * 1024).read_to_end(&mut s); String::from_utf8_lossy(&s).into_owned() }).unwrap_or_default();
    (files, head)
}
pub fn choice(root: &Path) -> project::Choice { let (f, b) = project_inputs(root); project::choice_in(root, &f, &b, &store::ordering_fab()) }

/// Fab.view: the active profile of a project as clients and agents get it: the fab, how it was chosen, and per capability the free value
/// and the one that costs more, each with its source, date and confidence; the stackups that fit; the assembly services (rigid boards);
/// what changed since the revision the project last saw.
pub fn view_of(p: &Profile, c: &project::Choice, home: &Path) -> Value {
    let o = c.opts();
    let changes = if c.rev_seen > 0 && c.rev_seen < p.rev { store::revision(home, &p.fab, c.rev_seen).map(|old| diff(&old, p)).unwrap_or_default() } else { vec![] };
    let changed: Vec<String> = changes.iter().map(|x| x.key.clone()).collect();
    let limits: Vec<Value> = select(p, &c.kind, &o).iter().map(|s| sel_json(s, &changed)).collect();
    let assembly = if c.kind == "pcb" { p.assembly.keys().map(|k| (k.clone(), json!(select_assembly(p, k, &o).iter().map(|s| sel_json(s, &changed)).collect::<Vec<_>>()))).collect::<serde_json::Map<_, _>>() } else { Default::default() };
    json!({"fab": p.fab, "rev": p.rev, "updated": p.updated, "choice": c.json(), "limits": limits,
        "stackups": stackups(p, &o).iter().map(|s| serde_json::to_value(s).unwrap_or_default()).collect::<Vec<_>>(),
        "assembly": assembly, "pages": p.pages, "changed": changes.iter().map(|x| x.line()).collect::<Vec<_>>()})
}
pub fn view_in(home: &Path, root: Option<&Path>) -> Result<Value, String> {
    let Some(root) = root else { return Err("this thread has no project folder".into()) };
    let c = choice(root);
    let p = store::load(home, &c.fab).ok_or_else(|| format!("Backplane has no profile of the fab {:?} (it has {})", c.fab, store::fabs(home).join(", ")))?;
    let mut v = view_of(&p, &c, home); v["fabs"] = json!(store::fabs(home)); v["kinds"] = json!(model::KINDS); Ok(v)
}

/// The design root of a thread (main's checkout for a hardware thread) or of a project by id.
async fn root_for(hub: &Arc<Hub>, thread: &str, project: &str) -> Option<PathBuf> {
    if !project.is_empty() { let m = hub.model().await; return m.projects.get(project).filter(|p| !p.removed).map(|p| PathBuf::from(&p.root)) }
    let r = crate::files::root_of(hub, thread).await?;
    Some(PathBuf::from(crate::files::kicad::canonical::design_root(&r.to_string_lossy())))
}

fn sv<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }

/// get (default), pick {fab?, kind?, options?} (kept with the project; marks the current revision seen), seen (the changes are read).
pub async fn act(hub: &Arc<Hub>, thread: &str, project: &str, a: &Value) -> Result<Value, String> {
    let root = root_for(hub, thread, project).await;
    let home = hub.cfg.home.clone(); let a = a.clone();
    tokio::task::spawn_blocking(move || {
        let action = match sv(&a, "action") { "" => "get", x => x };
        match action {
            "get" => view_in(&home, root.as_deref()),
            "pick" | "seen" => {
                let Some(r) = root.as_deref() else { return Err("this thread has no project folder".to_string()) };
                let c = choice(r);
                let fab = if sv(&a, "fab").is_empty() || action == "seen" { c.fab.clone() } else { sv(&a, "fab").to_string() };
                let p = store::load(&home, &fab).ok_or_else(|| format!("Backplane has no profile of the fab {fab:?} (it has {})", store::fabs(&home).join(", ")))?;
                let kind = if sv(&a, "kind").is_empty() || action == "seen" { c.kind.clone() } else { sv(&a, "kind").to_string() };
                let options = match a.get("options") { Some(o @ Value::Object(_)) if action == "pick" => o.clone(), _ => c.options.clone() };
                project::write(r, &fab, &kind, &options, p.rev)?;
                view_in(&home, Some(r))
            }
            x => Err(format!("unknown action {x:?}: get, pick or seen")),
        }
    }).await.unwrap_or_else(|_| Err("failed".into()))
}

pub struct Rpc;
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["fab.profile"] }
    /// `fab.profile {thread | project, action?: get|pick|seen, fab?, kind?, options?}` → the view (Fab.view).
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if sv(p, "action") != "get" && !sv(p, "action").is_empty() && crate::copy::on() { return vec![json!({"t":"reply","id":id,"ok":false,"error":crate::copy::HOME_ONLY})] }
            vec![match act(hub, sv(p, "thread"), sv(p, "project"), p).await { Ok(v) => json!({"t":"reply","id":id,"ok":true,"result":v}), Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}) }]
        })
    }
}

pub struct Tools(pub Arc<Hub>);
pub const TOOL: &str = "fab_profile";
pub fn tool_def() -> Value {
    json!({"name": TOOL, "description": "The project's fab profile: what the fab it is made at can make, read from the fab's published pages. Per capability (track width, spacing, drills, via hole and diameter, rings, hole and edge clearances, silkscreen, solder mask, board size, finishes, via types, ...) the free limit and the one that costs more, each with its unit, source page, the day it was read and its confidence (published, derived or inherited); the stackups that fit the board (dielectric thicknesses and Er); the assembly services' limits. Use these numbers, with their source, instead of remembered ones. get (default): the profile for the project's fab, board kind and options (picked, else its board fab file's, else its KiCad board's layers and thickness). pick: keep the fab, kind (pcb, flex, hdi, aluminum, copper_core, rogers, ptfe) and options (layers, copper_oz, thickness_mm, finish, color) with the project; only when the person chose them. seen: mark the changes since the project last looked as read.",
        "inputSchema": {"type": "object", "properties": {
            "action": {"type": "string", "description": "get (default), pick or seen"},
            "fab": {"type": "string", "description": "pick: the fab's id (get lists them in fabs)"},
            "kind": {"type": "string", "description": "pick: the board kind"},
            "options": {"type": "object", "description": "pick: the board's options in fab.json words, e.g. {\"layers\": 4, \"copper_oz\": 1, \"thickness_mm\": 1.6, \"finish\": \"enig\"}"}}, "required": []}})
}
impl ToolSet for Tools {
    fn list(&self, _thread: &str) -> Vec<Value> { vec![tool_def()] }
    fn owns(&self, name: &str) -> bool { name == TOOL }
    fn call<'a>(&'a self, thread: &'a str, _name: &'a str, a: &'a Value) -> Fut<'a, (bool, Value)> {
        Box::pin(async move {
            if !matches!(sv(a, "action"), "" | "get") && crate::copy::on() { return (false, json!({"error": crate::copy::HOME_ONLY})) }
            match act(&self.0, thread, "", a).await { Ok(v) => (true, v), Err(e) => (false, json!({"error": e})) }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(p: PathBuf, t: &str) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, t).unwrap() }
    #[test] fn fab_view_of_a_project_names_its_fab_tier_and_sources() {
        let t = crate::plat::tmp::dir("fab-view"); let r: &Path = &t; let h = crate::plat::tmp::dir("fab-view-home"); let home: &Path = &h;
        put(r.join("boards/a/a.kicad_pro"), "{}"); put(r.join("boards/a/a.kicad_pcb"), "(kicad_pcb\n\t(general\n\t\t(thickness 1.6)\n\t)\n\t(layers\n\t\t(0 \"F.Cu\" signal)\n\t\t(4 \"In1.Cu\" signal)\n\t\t(6 \"In2.Cu\" signal)\n\t\t(2 \"B.Cu\" signal)\n\t)\n)");
        let v = view_in(home, Some(r)).unwrap();
        assert_eq!((v["fab"].clone(), v["choice"]["by"].clone(), v["choice"]["options"]["layers"].clone()), (json!("jlcpcb"), json!("board"), json!(4.0)));
        let track = v["limits"].as_array().unwrap().iter().find(|l| l["key"] == "track_min").unwrap();
        assert_eq!((track["free"]["value"].clone(), track["extra"]["value"].clone()), (json!(0.09), json!(0.076)));
        assert!(track["free"]["source"].as_str().unwrap().starts_with("https://") && track["free"]["fetched"].as_str().unwrap().len() == 10, "{track}");
        assert!(v["stackups"].as_array().unwrap().iter().any(|s| s["id"] == "JLC04161H-7628"), "a 4-layer 1.6 mm board gets the published stackups");
        assert!(v["assembly"]["economic"].as_array().unwrap().iter().any(|l| l["key"] == "min_package"));
        assert!(view_in(home, None).is_err());
        // a newer gathered revision shows what changed since the project last looked
        project::write(r, "jlcpcb", "pcb", &json!({"layers": 4}), 1).unwrap();
        let mut p = store::load(home, "jlcpcb").unwrap(); p.rev = 2; p.updated = "2099-01-01".into();
        p.kinds.get_mut("pcb").unwrap().limits.iter_mut().find(|l| l.key == "copper_to_edge_min").unwrap().value = json!(0.3);
        store::save(home, &p).unwrap();
        let v = view_in(home, Some(r)).unwrap();
        assert_eq!(v["changed"], json!(["kind pcb copper_to_edge_min (free): 0.2 -> 0.3"]));
        assert_eq!(v["limits"].as_array().unwrap().iter().filter(|l| l["changed"] == true).map(|l| l["key"].clone()).collect::<Vec<_>>(), [json!("copper_to_edge_min")]);
        let _ = std::fs::remove_dir_all(home.join("fab"));
    }
    #[test] fn fab_view_the_tool_is_described_for_agents() { let d = tool_def(); assert_eq!(d["name"], TOOL); assert!(d["description"].as_str().unwrap().contains("source")) }
}

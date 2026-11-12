//! Fab profiles (docs/fab-profiles.md): what each fab (and its assembly service) can make, gathered from its published pages, every value
//! with its source and date; the project's fab, picked once; the view clients and agents read (`fab.profile`, the tool `fab_profile`).
//! model.rs is the data and its pure rules, store.rs where profiles are kept, project.rs the project's choice.
pub mod drc;
pub mod gather;
pub mod model;
pub mod panel;
pub mod project;
pub mod setup;
pub mod stackup;
pub mod store;

use crate::{agents::mcp::ToolSet, api::{Caller, Fut, RpcHandler}, hub::Hub};
use model::{diff, sel_json, select, select_assembly, stackups, Profile};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Arc};

pub fn start(hub: &Arc<Hub>, ag: Option<&Arc<crate::agents::Agents>>) {
    hub.register_rpc(Arc::new(Rpc));
    gather::start(hub); // the fab's published pages, read again a while after start and then daily
    crate::files::kicad::mgmt::register(Arc::new(panel::FabPanel)); // the Fab tab beside the design
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
    let sel = select(p, &c.kind, &o);
    let (tm, sm) = (model::free_num(&sel, "track_min").unwrap_or(0.0), model::free_num(&sel, "space_min").unwrap_or(0.0));
    json!({"fab": p.fab, "rev": p.rev, "updated": p.updated, "choice": c.json(), "limits": limits,
        "stackups": stackups(p, &o).iter().map(|s| json!({"id": s.id, "layers": s.layers, "thickness_mm": s.thickness_mm, "default": s.default})).collect::<Vec<_>>(),
        "stackup": stackup::view(p, &o, &c.options, tm, sm),
        "assembly": assembly, "pages": p.pages, "changed": changes.iter().map(|x| x.line()).collect::<Vec<_>>()})
}
pub fn profile_of(home: &Path, fab: &str) -> Result<Profile, String> { store::load(home, fab).ok_or_else(|| format!("Backplane has no profile of the fab {fab:?} (it has {})", store::fabs(home).join(", "))) }
pub fn view_in(home: &Path, root: Option<&Path>) -> Result<Value, String> {
    let Some(root) = root else { return Err("this thread has no project folder".into()) };
    let c = choice(root);
    let p = profile_of(home, &c.fab)?;
    let mut v = view_of(&p, &c, home); v["fabs"] = json!(store::fabs(home)); v["kinds"] = json!(model::KINDS); v["gathered"] = gather::last(home, &p.fab);
    if let Some((dru, plan)) = drc_plan(root, &p, &c) { let text = std::fs::read_to_string(&dru).unwrap_or_default(); let mut s = drc::status(&text, &plan); s["file"] = json!(rel(root, &dru)); v["drc"] = s }
    Ok(v)
}
fn rel(root: &Path, p: &Path) -> String { p.strip_prefix(root).unwrap_or(p).to_string_lossy().into_owned() }
/// The project's rules file and the section the profile makes for it (none without a KiCad project).
pub fn drc_plan(root: &Path, p: &Profile, c: &project::Choice) -> Option<(PathBuf, drc::Plan)> {
    let pro = crate::files::kicad::project::read(&root.to_string_lossy()).pro;
    let dru = drc::dru_path(&pro)?;
    let rules = drc::project_rules(&std::fs::read_to_string(&pro).unwrap_or_default());
    let text = std::fs::read_to_string(&dru).unwrap_or_default();
    Some((dru, drc::plan(p, c, &rules, &text)))
}

/// The design root of a thread (main's checkout for a hardware thread) or of a project by id.
async fn root_for(hub: &Arc<Hub>, thread: &str, project: &str) -> Option<PathBuf> {
    if !project.is_empty() { let m = hub.model().await; return m.projects.get(project).filter(|p| !p.removed).map(|p| PathBuf::from(&p.root)) }
    let r = crate::files::root_of(hub, thread).await?;
    Some(PathBuf::from(crate::files::kicad::canonical::design_root(&r.to_string_lossy())))
}

fn sv<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }

/// get (default), pick {fab?, kind?, options?} (kept with the project; marks the current revision seen), seen (the changes are read).
pub async fn act(hub: &Arc<Hub>, thread: &str, project: &str, a: &Value) -> Result<Value, String> { let root = root_for(hub, thread, project).await; act_in(hub, root, a).await }
pub async fn act_in(hub: &Arc<Hub>, root: Option<PathBuf>, a: &Value) -> Result<Value, String> {
    // a board setup suggestion the person approved (the panel's Apply)
    if sv(a, "action") == "apply" { let r = root.as_deref().ok_or("this thread has no project folder")?; return setup::apply(hub, r, sv(a, "key")).await }
    // read the fab's published pages again now (the project's fab, or the one named)
    if sv(a, "action") == "refresh" {
        let fab = if !sv(a, "fab").is_empty() { sv(a, "fab").to_string() } else { match &root { Some(r) => { let r = r.clone(); tokio::task::spawn_blocking(move || choice(&r).fab).await.unwrap_or_default() } None => store::ordering_fab() } };
        let mut v = gather::refresh_said(hub, &fab).await?;
        if let Some(r) = root.as_deref() { let (home, r) = (hub.cfg.home.clone(), r.to_path_buf()); if let Ok(Ok(view)) = tokio::task::spawn_blocking(move || view_in(&home, Some(&r))).await { v["profile"] = view } }
        return Ok(v)
    }
    let home = hub.cfg.home.clone(); let a = a.clone();
    tokio::task::spawn_blocking(move || {
        let action = match sv(&a, "action") { "" => "get", x => x };
        match action {
            "get" => view_in(&home, root.as_deref()),
            // the person takes a proposed stackup (the Fab panel's Use): kept with the project's other options
            "stackup" => {
                let Some(r) = root.as_deref() else { return Err("this thread has no project folder".to_string()) };
                let c = choice(r); let p = profile_of(&home, &c.fab)?; let id = sv(&a, "id");
                if !p.stackups.iter().any(|s| s.id == id) { return Err(format!("{id:?} is not a stackup the fab publishes")) }
                let mut o = if c.options.is_object() { c.options.clone() } else { json!({}) }; o["stackup"] = json!(id);
                project::write(r, &c.fab, &c.kind, &o, &c.tier, c.rev_seen)?;
                view_in(&home, Some(r))
            }
            "pick" | "seen" => {
                let Some(r) = root.as_deref() else { return Err("this thread has no project folder".to_string()) };
                let c = choice(r);
                let fab = if sv(&a, "fab").is_empty() || action == "seen" { c.fab.clone() } else { sv(&a, "fab").to_string() };
                let p = store::load(&home, &fab).ok_or_else(|| format!("Backplane has no profile of the fab {fab:?} (it has {})", store::fabs(&home).join(", ")))?;
                let kind = if sv(&a, "kind").is_empty() || action == "seen" { c.kind.clone() } else { sv(&a, "kind").to_string() };
                let options = match a.get("options") { Some(o @ Value::Object(_)) if action == "pick" => o.clone(), _ => c.options.clone() };
                let tier = if action == "pick" && !sv(&a, "tier").is_empty() { sv(&a, "tier").to_string() } else { c.tier.clone() };
                project::write(r, &fab, &kind, &options, &tier, p.rev)?;
                // a rules file that has the section follows the new pick
                if let Some((dru, plan)) = drc_plan(r, &p, &choice(r)) { if drc::section(&std::fs::read_to_string(&dru).unwrap_or_default()).is_some() { drc::write(&dru, &plan)?; } }
                view_in(&home, Some(r))
            }
            // the fab's limits into the project's rules file, its own rules kept (drc.rs)
            "drc" => {
                let Some(r) = root.as_deref() else { return Err("this thread has no project folder".to_string()) };
                let c = choice(r); let p = profile_of(&home, &c.fab)?;
                let Some((dru, plan)) = drc_plan(r, &p, &c) else { return Err("this project has no KiCad project (.kicad_pro) to keep DRC rules beside".to_string()) };
                drc::write(&dru, &plan)?;
                let mut v = view_in(&home, Some(r))?; v["drc"]["written"] = json!(true); Ok(v)
            }
            x => Err(format!("unknown action {x:?}: get, pick, seen, drc or refresh")),
        }
    }).await.unwrap_or_else(|_| Err("failed".into()))
}

pub struct Rpc;
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["fab.profile"] }
    /// `fab.profile {thread | project | pro, action?: get|pick|seen|drc|refresh|apply, fab?, kind?, options?, tier?, key?}` → the view (Fab.view),
    /// or what an apply did. `pro` (a .kicad_pro, the Fab panel's) publishes the panel again after the action.
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, _method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if sv(p, "action") != "get" && !sv(p, "action").is_empty() && crate::copy::on() { return vec![json!({"t":"reply","id":id,"ok":false,"error":crate::copy::HOME_ONLY})] }
            // the panel names the project by its .kicad_pro
            let pro = sv(p, "pro");
            let r = if !pro.is_empty() && sv(p, "action") == "review" { panel::review(hub, pro).await }
                else if pro.is_empty() { act(hub, sv(p, "thread"), sv(p, "project"), p).await } else { match panel::root_of_pro(hub, pro).await { Some(root) => act_in(hub, Some(root), p).await, None => Err("not in a project this hub knows".into()) } };
            if !pro.is_empty() && r.is_ok() { panel::publish(hub, pro).await }
            vec![match r { Ok(v) => json!({"t":"reply","id":id,"ok":true,"result":v}), Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}) }]
        })
    }
}

pub struct Tools(pub Arc<Hub>);
pub const TOOL: &str = "fab_profile";
pub fn tool_def() -> Value {
    json!({"name": TOOL, "description": "The project's fab profile: what the fab it is made at can make, read from the fab's published pages. Per capability (track width, spacing, drills, via hole and diameter, rings, hole and edge clearances, silkscreen, solder mask, board size, finishes, via types, ...) the free limit and the one that costs more, each with its unit, source page, the day it was read and its confidence (published, derived or inherited); the stackups that fit the board; stackup: the one picked, else proposed (best first in proposals, each with why; propose it to the person, pick it only when they agree), and per copper layer (F.Cu, In1.Cu, ... B.Cu) its reference planes above and below with the dielectric's height and Er, and the trace widths for the project's impedances (key 50, pairs as 90d at diff_gap_mm; closed-form estimates): feed these to floorplanning, stackup and routing; the assembly services' limits. Use these numbers, with their source, instead of remembered ones. get (default): the profile for the project's fab, board kind and options (picked, else its board fab file's, else its KiCad board's layers and thickness). pick: keep the fab, kind (pcb, flex, hdi, aluminum, copper_core, rogers, ptfe), options (layers, copper_oz, thickness_mm, finish, color; and for impedance work impedance [ohms], diff_impedance [ohms], diff_gap_mm, stackup: a published stackup's id) and tier (free: DRC holds the board to the fab's base-price limits, the default; extra: to what it makes for more) with the project; only when the person chose them. seen: mark the changes since the project last looked as read. drc: write the fab's limits into the project's .kicad_dru as a marked section (the project's own rules and stricter settings are kept and win); drc in the answer says whether that section is current, the board setup minimums under the fab's (suggestions: tell the person, never change the board setup yourself) and the project's own rules that allow less than the fab (conflicts). refresh: read the fab's published pages again now (Backplane also does daily); values it cannot read keep their old value and date.",
        "inputSchema": {"type": "object", "properties": {
            "action": {"type": "string", "description": "get (default), pick, seen, drc or refresh"},
            "tier": {"type": "string", "description": "pick: free (default) or extra"},
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
            if sv(a, "action") == "apply" { return (false, json!({"error": "applying a board setup suggestion is the person's (the Fab panel's Apply): tell them the suggestion instead"})) }
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
        assert_eq!((track["free"]["value"].clone(), track["extra"]["value"].clone()), (json!(0.09), json!(0.075)));
        assert!(track["free"]["source"].as_str().unwrap().starts_with("https://") && track["free"]["fetched"].as_str().unwrap().len() == 10, "{track}");
        assert!(v["stackups"].as_array().unwrap().iter().any(|s| s["id"] == "JLC04161H-7628"), "a 4-layer 1.6 mm board gets the published stackups");
        assert!(v["assembly"]["economic"].as_array().unwrap().iter().any(|l| l["key"] == "min_package"));
        assert!(view_in(home, None).is_err());
        assert_eq!((v["drc"]["present"].clone(), v["drc"]["file"].clone()), (json!(false), json!("boards/a/a.kicad_dru")), "the rules file beside the project, not written yet");
        let (dru, plan) = drc_plan(r, &store::load(home, "jlcpcb").unwrap(), &choice(r)).unwrap(); drc::write(&dru, &plan).unwrap();
        assert_eq!(view_in(home, Some(r)).unwrap()["drc"]["current"], json!(true));
        // a newer gathered revision shows what changed since the project last looked
        project::write(r, "jlcpcb", "pcb", &json!({"layers": 4}), "free", 1).unwrap();
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

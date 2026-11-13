//! The Plan management panel (docs/MANAGEMENT.md, docs/floorplanner.md): per layer the plan's corridors and plane regions with their
//! state, net count, routing progress and reference plane, and what awaits the person's approval. Published as info `mgmt.plan.<digest>`
//! for the desktop and the phones; its buttons are `plan.approve` (an item, a layer, all: the person's), `plan.stop`, `plan.focus` (the
//! item's nets and parts selected and zoomed in KiCad). The plan comes from the open KiCad (its live plan, derived data and progress) and,
//! without it, from the saved `<board>.kicad_plan`. Pure `data`, tested.
use crate::{files::kicad::mgmt, hub::Hub};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

pub struct PlanPanel;
impl mgmt::Panel for PlanPanel {
    fn id(&self) -> &'static str { "plan" }
    fn label(&self) -> &'static str { "Plan" }
    fn watch(&self, hub: &Arc<Hub>, project: &str) { let (h, p) = (hub.clone(), project.to_string()); tokio::spawn(async move { publish(&h, &p).await; }); }
}
fn st<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }

/// An item's state as the panel draws it: proposed, edited (by the person, not yet judged), approved, routing (with %), routed, verified,
/// rejected, failed, or blocked (problems the plan's audit or DRC found).
pub fn state(item: &Value, derived: &Value) -> &'static str {
    let problems = derived["problems"].as_array().map_or(0, Vec::len) + derived["problem_count"].as_u64().unwrap_or(0) as usize;
    match st(item, "status") {
        _ if problems > 0 && !matches!(st(item, "status"), "rejected") => "blocked",
        "proposed" if item["edited"] == true => "edited",
        "proposed" => "proposed", "approved" => "approved", "routing" => "routing", "routed" => "routed", "verified" => "verified", "rejected" => "rejected", "failed" => "failed",
        _ => "proposed",
    }
}
/// The panel's data from a plan (with its `derived` when KiCad gave it).
pub fn data(plan: &Value, asks: &[Value], live: bool) -> Value {
    let groups = plan["groups"].as_array().cloned().unwrap_or_default();
    let group = |id: &str| groups.iter().find(|g| st(g, "id") == id).cloned().unwrap_or(Value::Null);
    let der = |id: &str| plan["derived"]["items"][id].clone();
    let mut layers: Vec<String> = plan["layers"].as_array().into_iter().flatten().map(|l| st(l, "layer").to_string()).collect();
    let mut rows: Vec<(String, Value)> = vec![];
    for c in plan["corridors"].as_array().into_iter().flatten() {
        let g = group(st(c, "group")); let d = der(st(c, "id"));
        let (r, t) = (d["routed"].as_u64().or(c["progress"]["routed"].as_u64()), d["total"].as_u64().or(c["progress"]["total"].as_u64()));
        rows.push((st(c, "layer").to_string(), json!({"id": c["id"], "kind": "corridor", "name": g["name"].as_str().unwrap_or(st(c, "group")), "color": g["color"].as_str().unwrap_or("#8a8a8a"),
            "nets": g["nets"].as_array().map_or(0, Vec::len), "state": state(c, &d), "locked": c["locked"] == true, "order": c["order"], "routed": r, "total": t,
            "problems": d["problems"].as_array().map_or(0, Vec::len)})));
    }
    for p in plan["planes"].as_array().into_iter().flatten() {
        let d = der(st(p, "id"));
        rows.push((st(p, "layer").to_string(), json!({"id": p["id"], "kind": "plane", "name": p["net"], "color": p["color"].as_str().unwrap_or("#5a7a9a"), "nets": 1, "state": state(p, &d),
            "locked": p["locked"] == true, "reference": p["reference"], "problems": d["problems"].as_array().map_or(0, Vec::len)})));
    }
    for (l, _) in &rows { if !layers.contains(l) { layers.push(l.clone()) } }
    let stack = |l: &str| plan["derived"]["stackup"][l].as_str().map(str::to_string).unwrap_or_default();
    let ls: Vec<Value> = layers.iter().map(|l| {
        let its: Vec<&Value> = rows.iter().filter(|(x, _)| x == l).map(|(_, v)| v).collect();
        let lv = plan["layers"].as_array().into_iter().flatten().find(|x| st(x, "layer") == l).cloned().unwrap_or(json!({"status": "proposed", "role": ""}));
        let done = its.iter().filter(|v| matches!(v["state"].as_str(), Some("routed") | Some("verified"))).count();
        json!({"layer": l, "role": lv["role"], "status": lv["status"], "stackup": stack(l), "done": done, "items": its})
    }).collect();
    let awaiting = rows.iter().filter(|(_, v)| matches!(v["state"].as_str(), Some("proposed") | Some("edited"))).count();
    let blocked = rows.iter().filter(|(_, v)| v["state"] == "blocked").count();
    let routing = rows.iter().any(|(_, v)| v["state"] == "routing");
    json!({"badge": {"n": awaiting + blocked, "tone": if blocked > 0 { "bad" } else if awaiting > 0 { "warn" } else { "" }}, "revision": plan["revision"], "live": live,
        "awaiting": awaiting, "blocked": blocked, "routing": routing, "asks": asks, "layers": ls, "empty": rows.is_empty()})
}

/// The board of a project's .kicad_pro.
pub fn pcb_of(pro: &str) -> String { crate::files::kicad::project::read(&Path::new(pro).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()).pcb }
/// The plan as it is now: the open KiCad's (live, with derived data) when the board is open there, else the saved `<board>.kicad_plan`
/// (no background KiCad is started for it). Null without either.
pub async fn plan_now(hub: &Arc<Hub>, pcb: &str) -> (Value, bool) {
    let pro = crate::files::kicad::project::read(&Path::new(pcb).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()).pro;
    if crate::files::kicad::embed::gui_route(&pro).is_some() { if let Ok(p) = super::read_plan(hub, None, pcb, true).await { return (p, true) } }
    let f = Path::new(pcb).with_extension("kicad_plan");
    (tokio::task::spawn_blocking(move || std::fs::read_to_string(f).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok())).await.ok().flatten().unwrap_or(Value::Null), false)
}
/// Publish the panel for a project (its .kicad_pro): the open KiCad's plan, else the saved one.
pub async fn publish(hub: &Arc<Hub>, pro: &str) {
    let pcb = pcb_of(pro); if pcb.is_empty() { return }
    let (plan, live) = plan_now(hub, &pcb).await;
    let d = if plan.is_null() { json!({"badge": {"n": 0, "tone": ""}, "empty": true, "live": live, "layers": []}) } else { data(&plan, &super::events::open_asks(&pcb), live) };
    mgmt::publish(hub, "plan", pro, d).await;
}

/// `plan.focus {pro, id}`: the item's nets (a corridor's group, a plane's net) and the parts it runs between, selected and zoomed in KiCad.
pub async fn focus(hub: &Arc<Hub>, pro: &str, id: &str) -> Result<Value, String> {
    let pcb = pcb_of(pro);
    let plan = super::read_plan(hub, None, &pcb, false).await?;
    let (mut nets, mut refs): (Vec<Value>, Vec<Value>) = (vec![], vec![]);
    if let Some(c) = plan["corridors"].as_array().into_iter().flatten().find(|c| st(c, "id") == id) {
        if let Some(g) = plan["groups"].as_array().into_iter().flatten().find(|g| st(g, "id") == st(c, "group")) { nets = g["nets"].as_array().cloned().unwrap_or_default() }
        for k in ["from", "to"] { refs.extend(c[k].as_array().cloned().unwrap_or_default()) }
    } else if let Some(p) = plan["planes"].as_array().into_iter().flatten().find(|p| st(p, "id") == id) { nets.push(p["net"].clone()) }
    else { return Err(format!("no item {id} in the plan")) }
    crate::files::kicad::xprobe::select(hub, &json!({"project": pro, "references": refs, "nets": nets, "pins": [], "show": "PCB", "zoom": true})).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn plan_panel_states_layers_and_what_awaits_the_person() {
        let plan = json!({"revision": 4, "layers": [{"layer": "In1.Cu", "role": "signal", "status": "proposed"}, {"layer": "In2.Cu", "role": "power", "status": "approved"}],
            "groups": [{"id": "g1", "name": "DDR DQ0-7", "color": "#e8a33d", "nets": ["A", "B", "C"]}],
            "corridors": [{"id": "c1", "group": "g1", "layer": "In1.Cu", "status": "proposed"}, {"id": "c2", "group": "g1", "layer": "In1.Cu", "status": "proposed", "edited": true},
                {"id": "c3", "group": "g1", "layer": "In1.Cu", "status": "routing", "progress": {"routed": 9, "total": 14}}, {"id": "c4", "group": "g1", "layer": "In1.Cu", "status": "approved"}],
            "planes": [{"id": "p1", "net": "GND", "layer": "In2.Cu", "status": "approved", "reference": "In1.Cu"}],
            "derived": {"items": {"c4": {"problems": [{"kind": "crossing"}]}}, "stackup": {"In1.Cu": "Stripline between GND2 and GND3"}}});
        let d = data(&plan, &[], true);
        let l1 = &d["layers"][0]; assert_eq!((l1["layer"].clone(), l1["stackup"].clone()), (json!("In1.Cu"), json!("Stripline between GND2 and GND3")));
        assert_eq!(l1["items"].as_array().unwrap().iter().map(|i| i["state"].as_str().unwrap()).collect::<Vec<_>>(), ["proposed", "edited", "routing", "blocked"]);
        assert_eq!((l1["items"][2]["routed"].clone(), l1["items"][2]["total"].clone(), l1["items"][0]["nets"].clone(), l1["items"][0]["name"].clone()), (json!(9), json!(14), json!(3), json!("DDR DQ0-7")));
        assert_eq!(d["layers"][1]["items"][0]["reference"], "In1.Cu");
        assert_eq!((d["awaiting"].clone(), d["blocked"].clone(), d["routing"].clone()), (json!(2), json!(1), json!(true)));
        assert_eq!(d["badge"], json!({"n": 3, "tone": "bad"}));
        assert_eq!(data(&json!({}), &[], false)["empty"], true);
    }
}

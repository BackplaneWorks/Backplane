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
/// First human-readable audit/DRC problem text on an item's derived entry, if any.
fn problem_text(d: &Value) -> String {
    d["problems"].as_array().and_then(|a| a.first()).and_then(|p| {
        p["message"].as_str().or_else(|| p["text"].as_str()).or_else(|| p["kind"].as_str())
    }).unwrap_or("").to_string()
}

/// The panel's data from a plan (with its `derived` when KiCad gave it), optionally enriched with `.kicad_pcb` geometry.
pub fn data(plan: &Value, asks: &[Value], live: bool) -> Value {
    data_with_board(plan, asks, live, None)
}

pub fn data_with_board(plan: &Value, asks: &[Value], live: bool, pcb: Option<&str>) -> Value {
    let cached = pcb.and_then(super::geom::load);
    let geom = cached.as_ref().map(|b| &b.geom);
    let guides = cached.as_ref().map(|b| super::geom::guides_for_plan(b, plan)).unwrap_or_default();

    let groups = plan["groups"].as_array().cloned().unwrap_or_default();
    let group = |id: &str| groups.iter().find(|g| st(g, "id") == id).cloned().unwrap_or(Value::Null);
    let der = |id: &str| plan["derived"]["items"][id].clone();
    let mut layers: Vec<String> = plan["layers"].as_array().into_iter().flatten().map(|l| st(l, "layer").to_string()).collect();
    let mut rows: Vec<(String, Value)> = vec![];
    for c in plan["corridors"].as_array().into_iter().flatten() {
        let g = group(st(c, "group")); let d = der(st(c, "id"));
        let (r, t) = (d["routed"].as_u64().or(c["progress"]["routed"].as_u64()), d["total"].as_u64().or(c["progress"]["total"].as_u64()));
        rows.push((st(c, "layer").to_string(), json!({"id": c["id"], "kind": "corridor", "name": g["name"].as_str().unwrap_or(st(c, "group")), "color": g["color"].as_str().unwrap_or("#8a8a8a"),
            "nets": g["nets"].as_array().map_or(0, Vec::len), "state": state(c, &d), "locked": c["locked"] == true, "edited": c["edited"] == true, "order": c["order"], "routed": r, "total": t,
            "width": c["width"].as_f64().or_else(|| g["width"].as_f64()), "impedance": g["impedance"].clone(),
            "problems": d["problems"].as_array().map_or(0, Vec::len), "problem_text": problem_text(&d)})));
    }
    for p in plan["planes"].as_array().into_iter().flatten() {
        let d = der(st(p, "id"));
        rows.push((st(p, "layer").to_string(), json!({"id": p["id"], "kind": "plane", "name": p["net"], "color": p["color"].as_str().unwrap_or("#5a7a9a"), "nets": 1, "state": state(p, &d),
            "locked": p["locked"] == true, "edited": p["edited"] == true, "reference": p["reference"], "priority": p["priority"],
            "problems": d["problems"].as_array().map_or(0, Vec::len), "problem_text": problem_text(&d)})));
    }
    for (l, _) in &rows { if !layers.contains(l) { layers.push(l.clone()) } }
    let stack = |l: &str| plan["derived"]["stackup"][l].as_str().map(str::to_string).unwrap_or_default();
    let ls: Vec<Value> = layers.iter().enumerate().map(|(li, l)| {
        let its: Vec<&Value> = rows.iter().filter(|(x, _)| x == l).map(|(_, v)| v).collect();
        let lv = plan["layers"].as_array().into_iter().flatten().find(|x| st(x, "layer") == l).cloned().unwrap_or(json!({"status": "proposed", "role": ""}));
        let done = its.iter().filter(|v| matches!(v["state"].as_str(), Some("routed") | Some("verified"))).count();
        let approved = its.iter().filter(|v| v["state"] == "approved").count();
        let cl = geom.and_then(|g| g.copper_layers.iter().find(|c| c.layer == *l));
        let explicit_role = st(&lv, "role");
        let inferred_role = if !explicit_role.is_empty() {
            explicit_role
        } else if !its.is_empty() && its.iter().all(|v| v["kind"] == "plane") {
            "power"
        } else if its.iter().any(|v| v["kind"] == "corridor") {
            "signal"
        } else {
            cl.map(|c| c.role.as_str()).unwrap_or("")
        };
        let stackup_txt = stack(l);
        let subtitle = if !stackup_txt.is_empty() {
            stackup_txt.clone()
        } else {
            cl.map(|c| super::geom::subtitle_for_layer(c, inferred_role)).unwrap_or_default()
        };
        let title = cl.map(|c| c.title.clone()).unwrap_or_else(|| format!("L{} · {}", li + 1, l));
        let index = cl.map(|c| c.index).unwrap_or(li + 1);
        json!({"layer": l, "index": index, "title": title, "subtitle": subtitle, "role": if explicit_role.is_empty() { json!(inferred_role) } else { lv["role"].clone() },
            "status": lv["status"], "stackup": stackup_txt, "done": done, "approved": approved, "total": its.len(), "items": its})
    }).collect();
    let total = rows.len();
    let awaiting = rows.iter().filter(|(_, v)| matches!(v["state"].as_str(), Some("proposed") | Some("edited"))).count();
    let approved = rows.iter().filter(|(_, v)| v["state"] == "approved").count();
    let blocked = rows.iter().filter(|(_, v)| v["state"] == "blocked").count();
    let routing = rows.iter().any(|(_, v)| v["state"] == "routing");
    let done = rows.iter().filter(|(_, v)| matches!(v["state"].as_str(), Some("routed") | Some("verified"))).count();
    let routed = rows.iter().filter(|(_, v)| v["state"] == "routed").count();
    let verified = rows.iter().filter(|(_, v)| v["state"] == "verified").count();
    let failed = rows.iter().filter(|(_, v)| v["state"] == "failed").count();
    let rejected = rows.iter().filter(|(_, v)| matches!(v["state"].as_str(), Some("rejected") | Some("failed"))).count();
    let status = if total == 0 { "empty" }
        else if blocked > 0 || failed > 0 { "blocked" }
        else if routing { "routing" }
        else if awaiting > 0 { "awaiting" }
        else if verified == total { "verified" }
        else if done == total { "routed" }
        else if approved > 0 { "ready" }
        else if done > 0 { "partial" }
        else { "rejected" };
    let stackup_summary = plan["derived"]["stackup_summary"].as_str().map(str::to_string).unwrap_or_else(|| {
        ls.iter().filter_map(|l| {
            let txt = l["stackup"].as_str().filter(|s| !s.is_empty()).or_else(|| l["subtitle"].as_str()).unwrap_or("");
            (!txt.is_empty()).then(|| format!("{}: {txt}", l["layer"].as_str().unwrap_or("")))
        }).collect::<Vec<_>>().join(" · ")
    });
    let impedances = plan["derived"]["impedances"].clone();
    let draw = draw_enriched(plan, &rows, &guides);
    let board = geom
        .map(|g| super::geom::board_json(g, plan, 180))
        .unwrap_or_else(|| json!({"bbox": super::geom::fallback_bbox(plan), "outline": [], "parts": []}));
    let copper_layers: Value = if let Some(g) = geom {
        json!(g.copper_layers)
    } else {
        json!(ls.iter().map(|l| json!({"layer": l["layer"], "index": l["index"], "role": l["role"], "title": l["title"], "subtitle": l["subtitle"]})).collect::<Vec<_>>())
    };
    json!({"draw": draw, "board": board, "copper_layers": copper_layers, "notes": plan["notes"], "constraints": plan["constraints"],
        "badge": {"n": awaiting + blocked, "tone": if blocked > 0 { "bad" } else if awaiting > 0 { "warn" } else { "" }}, "revision": plan["revision"], "live": live,
        "total": total, "awaiting": awaiting, "approved": approved, "blocked": blocked, "routing": routing, "done": done, "routed": routed, "verified": verified, "rejected": rejected,
        "status": status, "stackup_summary": stackup_summary, "impedances": impedances, "asks": asks, "layers": ls, "empty": rows.is_empty()})
}

/// What the viewers draw over the board (bands and regions, the floorplanner's look per state): corridors as bands along their
/// path at their width, plane regions as their outline; millimetres, board coordinates. At most `MAX_POINTS` points in all (the info's bound).
pub const MAX_POINTS: usize = 3000;
pub fn draw(plan: &Value, rows: &[(String, Value)]) -> Value {
    draw_enriched(plan, rows, &std::collections::HashMap::new())
}

fn take_pts(left: &mut usize, pts: &Value) -> Option<Value> {
    let n = pts.as_array().map_or(0, Vec::len);
    if n == 0 || n > *left { return None }
    *left -= n;
    Some(pts.clone())
}

fn draw_enriched(plan: &Value, rows: &[(String, Value)], guides: &std::collections::HashMap<String, Vec<[f64; 4]>>) -> Value {
    let row = |id: &str| rows.iter().find(|(_, v)| v["id"] == id).map(|(_, v)| v);
    let state = |id: &str| row(id).map(|v| v["state"].clone()).unwrap_or(json!("proposed"));
    let colour = |id: &str| row(id).map(|v| v["color"].clone()).unwrap_or(json!("#8a8a8a"));
    let nets = |id: &str| row(id).map(|v| v["nets"].clone()).unwrap_or(json!(0));
    let mut left = MAX_POINTS;
    let bands: Vec<Value> = plan["corridors"].as_array().into_iter().flatten().filter_map(|c| {
        let id = st(c, "id");
        let r = row(id);
        let g = guides.get(id).cloned().unwrap_or_default();
        let path = take_pts(&mut left, &c["path"])?;
        let vias = c["vias"].as_array().map(|a| {
            let take_n = a.len().min(64).min(left);
            left -= take_n;
            Value::Array(a.iter().take(take_n).cloned().collect())
        }).unwrap_or_else(|| c["vias"].clone());
        Some(json!({"id": id, "layer": c["layer"], "width": c["width"].as_f64().unwrap_or(1.0), "path": path, "state": state(id), "color": colour(id), "nets": nets(id), "vias": vias,
            "name": r.map(|v| v["name"].clone()).unwrap_or(json!(id)), "group": c["group"], "locked": c["locked"] == true, "edited": c["edited"] == true, "order": c["order"],
            "from": c["from"], "to": c["to"], "impedance": r.map(|v| v["impedance"].clone()).unwrap_or(Value::Null), "guides": g,
            "problem_text": r.map(|v| v["problem_text"].clone()).unwrap_or(json!(""))}))
    }).collect();
    let regions: Vec<Value> = plan["planes"].as_array().into_iter().flatten().filter_map(|p| {
        let id = st(p, "id");
        let r = row(id);
        let rings = match super::norm_outline(&p["outline"]) {
            Ok(Value::Array(rs)) => rs,
            _ => return None,
        };
        let outer = take_pts(&mut left, rings.first()?)?;
        let mut holes = Vec::new();
        for h in rings.iter().skip(1) {
            if let Some(kept) = take_pts(&mut left, h) {
                holes.push(kept);
            } else {
                break;
            }
        }
        Some(json!({"id": id, "layer": p["layer"], "net": p["net"], "outline": outer, "holes": holes, "state": state(id), "color": colour(id),
            "locked": p["locked"] == true, "edited": p["edited"] == true, "reference": p["reference"], "priority": p["priority"],
            "problem_text": r.map(|v| v["problem_text"].clone()).unwrap_or(json!(""))}))
    }).collect();
    json!({"bands": bands, "regions": regions})
}

/// Keep the published `mgmt.plan.<digest>` payload safely within the 64 KiB `hub.set_info` limit.
pub fn fit_info_budget(d: &mut Value) {
    const BUDGET: usize = 58 * 1024;
    if d.to_string().len() <= BUDGET { return }
    d["board_trimmed"] = json!(true);

    if let Some(bands) = d["draw"]["bands"].as_array_mut() {
        for b in bands.iter_mut() {
            if let Some(gs) = b["guides"].as_array_mut() {
                if gs.len() > 6 { gs.truncate(6); }
            }
        }
    }
    if d.to_string().len() <= BUDGET { return }

    if let Some(bands) = d["draw"]["bands"].as_array_mut() {
        for b in bands.iter_mut() { b["guides"] = json!([]); }
    }
    if d.to_string().len() <= BUDGET { return }

    for cap in [80usize, 40, 0] {
        if let Some(parts) = d["board"]["parts"].as_array_mut() {
            if parts.len() > cap { parts.truncate(cap); }
        }
        if d.to_string().len() <= BUDGET { return }
    }

    if let Some(regions) = d["draw"]["regions"].as_array_mut() {
        for r in regions.iter_mut() { r["holes"] = json!([]); }
    }
    if let Some(bands) = d["draw"]["bands"].as_array_mut() {
        for b in bands.iter_mut() { b["vias"] = json!([]); }
    }
    if let Some(board) = d.get_mut("board").and_then(Value::as_object_mut) {
        board.insert("outline".into(), json!([]));
    }
    if d.to_string().len() <= BUDGET { return }

    if let Some(draw) = d.get_mut("draw").and_then(Value::as_object_mut) {
        draw.insert("bands".into(), json!([]));
        draw.insert("regions".into(), json!([]));
    }
    if d.to_string().len() <= BUDGET { return }

    if let Some(obj) = d.as_object_mut() {
        obj.insert("notes".into(), json!([]));
        obj.insert("constraints".into(), json!([]));
        obj.insert("impedances".into(), json!([]));
        obj.insert("stackup_summary".into(), json!(""));
    }
    if d.to_string().len() <= BUDGET { return }

    if let Some(layers) = d["layers"].as_array_mut() {
        for l in layers.iter_mut() {
            if let Some(items) = l["items"].as_array_mut() {
                for it in items.iter_mut() {
                    if let Some(obj) = it.as_object_mut() {
                        obj.insert("problem_text".into(), json!(""));
                    }
                }
            }
        }
    }
    while d.to_string().len() > BUDGET {
        let Some(layers) = d["layers"].as_array_mut() else { break };
        let mut shrunk = false;
        for l in layers.iter_mut().rev() {
            if let Some(items) = l["items"].as_array_mut() {
                if !items.is_empty() {
                    let keep = items.len() / 2;
                    items.truncate(keep);
                    shrunk = true;
                    break;
                }
            }
        }
        if !shrunk {
            layers.clear();
            break;
        }
    }
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
    let mut d = if plan.is_null() { json!({"badge": {"n": 0, "tone": ""}, "empty": true, "live": live, "layers": []}) } else { data_with_board(&plan, &super::events::open_asks(&pcb), live, Some(&pcb)) };
    // the person's switch, and plans an agent asked about on the project's other boards (the viewer shows one board; the plan may be another's)
    d["on"] = json!(super::is_on(hub.setting(super::SETTING).await.as_deref().unwrap_or("")));
    let top = project_top(pro);
    d["elsewhere"] = json!(elsewhere(&super::events::asking_boards(), &pcb, &top));
    if d["empty"] == true && !d["elsewhere"].as_array().map_or(true, Vec::is_empty) { d["badge"] = json!({"n": d["elsewhere"].as_array().map_or(0, Vec::len), "tone": "warn"}) }
    // a KiCad whose own keys in plan view are not the plan's: the panel says to approve here
    if let Some(n) = crate::files::kicad::embed::gui_route(pro).and_then(|r| super::keys_note(&r.caps)) { d["keys_note"] = json!(n); }
    fit_info_budget(&mut d);
    mgmt::publish(hub, "plan", pro, d).await;
}

/// The project a .kicad_pro belongs to: the nearest folder above it holding `.git` or `.backplane.json` (a worktree path as its main checkout's).
pub fn project_top(pro: &str) -> String {
    let main = crate::files::kicad::canonical::main_of(pro);
    Path::new(&main).ancestors().skip(1).find(|d| d.join(".git").exists() || d.join(".backplane.json").exists())
        .map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()
}
/// Of the boards with an agent's open request for approval, the others under the same folder as this one: [{pcb, name}].
pub fn elsewhere(asking: &[String], pcb: &str, top: &str) -> Vec<Value> {
    let top = top.trim_end_matches('/');
    asking.iter().filter(|b| b.as_str() != pcb && !top.is_empty() && b.starts_with(&format!("{top}/")))
        .map(|b| json!({"pcb": b, "rel": &b[top.len() + 1..], "name": Path::new(b).file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default()})).collect()
}
/// `plan.focus {pro, id}`: the item's nets (a corridor's group, a plane's net) and the parts it runs between, selected and zoomed in KiCad.
pub async fn focus(hub: &Arc<Hub>, pro: &str, id: &str) -> Result<Value, String> {
    let pcb = pcb_of(pro);
    let plan = super::read_plan(hub, None, &pcb, false).await?;
    let (mut nets, mut refs, mut layer): (Vec<Value>, Vec<Value>, String) = (vec![], vec![], String::new());
    if let Some(c) = plan["corridors"].as_array().into_iter().flatten().find(|c| st(c, "id") == id) {
        if let Some(g) = plan["groups"].as_array().into_iter().flatten().find(|g| st(g, "id") == st(c, "group")) { nets = g["nets"].as_array().cloned().unwrap_or_default() }
        for k in ["from", "to"] { refs.extend(c[k].as_array().cloned().unwrap_or_default()) }
        layer = st(c, "layer").to_string();
    } else if let Some(p) = plan["planes"].as_array().into_iter().flatten().find(|p| st(p, "id") == id) { nets.push(p["net"].clone()); layer = st(p, "layer").to_string() }
    else { return Err(format!("no item {id} in the plan")) }
    // selecting the board's copper ends KiCad's plan tool, which (bp.13 batch 5 on) leaves plan view: a person who pressed the row in
    // plan view is put back in it (bp.13 batch 11 `plan-view-api` serves the toggle there), the zoom kept
    let in_view = |m: &Value| m["in_plan_view"] == true;
    let was = super::call(hub, None, &pcb, "GetPlan", json!({"board": crate::fab::setup::spec(&pcb), "derived": false})).await.map_or(false, |m| in_view(&m));
    // a KiCad with `plan-select` (bp.13 batch 12): in plan view the row selects the plan item itself, as a click would (W/A/R act on it),
    // its layer active, zoomed, plan view kept. Without it: the copper selected and, from plan view, the re-entry below.
    if let (true, Some(route)) = (was, crate::files::kicad::embed::gui_route(pro).filter(|r| r.caps.iter().any(|c| c == PLAN_SELECT))) {
        let o = crate::files::kicad::embed::gui_call(&route, &format!("{}SelectPlanItems", super::NS), json!({"board": crate::fab::setup::spec(&pcb), "ids": [id], "zoom": true})).await;
        if o["ok"] == true {
            let m = &o["message"];
            let active = m["active_layer"].as_str().unwrap_or(&layer).to_string();
            return Ok(json!({"kicad": true, "via": "SelectPlanItems", "plan_view": if m["in_plan_view"] == true { "kept" } else { "left" }, "layer": active, "layer_active": true,
                "selected": m["selected"].clone(), "missing": if m["missing"].is_null() { json!([]) } else { m["missing"].clone() }}))
        }
        if o["status"] == "AS_BAD_REQUEST" { return Err(format!("KiCad has no plan item {id}")) }
    }
    let mut v = crate::files::kicad::xprobe::select(hub, &json!({"project": pro, "references": refs, "nets": nets, "pins": [], "show": "PCB", "zoom": true})).await?;
    if was {
        let now = super::call(hub, None, &pcb, "GetPlan", json!({"board": crate::fab::setup::spec(&pcb), "derived": false})).await.map_or(true, |m| in_view(&m));
        if !now { if let Some(route) = crate::files::kicad::embed::gui_route(pro) {
            let o = crate::files::kicad::embed::gui_call(&route, "kiapi.common.commands.RunAction", json!({"action": "pcbnew.Plan.togglePlanView"})).await;
            v["plan_view"] = json!(if o["ok"] == true { "kept" } else { "left" });
        } } else { v["plan_view"] = json!("kept") }
    }
    // the item's layer made active, so in plan view the person sees (and can click) the item itself; a KiCad that refuses it (plan view
    // busy to the API before bp.13 batch 11) keeps its layer, and the answer says so
    if let (false, Some(route)) = (layer.is_empty(), crate::files::kicad::embed::gui_route(pro)) {
        let o = crate::files::kicad::embed::gui_call(&route, "kiapi.board.commands.SetActiveLayer", json!({"board": crate::fab::setup::spec(&pcb), "layer": board_layer(&layer)})).await;
        v["layer"] = json!(layer); v["layer_active"] = json!(o["ok"].as_bool().unwrap_or(false));
        if o["ok"] != true { v["layer_note"] = json!(format!("KiCad kept its layer: {}", o["error"].as_str().or(o["status"].as_str()).unwrap_or("no answer"))) }
    }
    Ok(v)
}
/// The runtime capability for selecting plan items by id (kiapi.backplane.plan.SelectPlanItems {board, ids, zoom} -> {selected, missing,
/// active_layer, in_plan_view}; bp.13 batch 12, else bp.13.1).
pub const PLAN_SELECT: &str = "plan-select";
/// KiCad's API name of a board layer ("B.Cu" -> "BL_B_Cu", "In1.Cu" -> "BL_In1_Cu").
pub fn board_layer(name: &str) -> String { format!("BL_{}", name.replace('.', "_")) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn plan_panel_focus_names_kicads_layer() { assert_eq!(board_layer("B.Cu"), "BL_B_Cu"); assert_eq!(board_layer("In1.Cu"), "BL_In1_Cu"); assert_eq!(board_layer("F.Cu"), "BL_F_Cu") }
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
        let mut p2 = plan.clone(); p2["corridors"][0]["path"] = json!([[10.0, 20.0], [30.0, 20.0]]); p2["corridors"][0]["width"] = json!(2.4); p2["planes"][0]["outline"] = json!([[[0, 0], [5, 0], [5, 5]]]);
        let dr = data(&p2, &[], true)["draw"].clone();
        assert_eq!((dr["bands"].as_array().unwrap().len(), dr["bands"][0]["width"].clone(), dr["bands"][0]["state"].clone(), dr["bands"][0]["color"].clone()), (1, json!(2.4), json!("proposed"), json!("#e8a33d")), "corridors without a path are not drawn");
        assert_eq!(dr["regions"][0]["outline"].as_array().unwrap().len(), 3);
    }
    #[test] fn plan_panel_names_a_plan_waiting_on_another_board_of_the_project() {
        let asking = vec!["/p/Nodes/A_new/A_new.kicad_pcb".to_string(), "/p/Nodes/A/A.kicad_pcb".to_string(), "/q/B/B.kicad_pcb".to_string()];
        let e = elsewhere(&asking, "/p/Nodes/A/A.kicad_pcb", "/p");
        assert_eq!(e, vec![json!({"pcb": "/p/Nodes/A_new/A_new.kicad_pcb", "rel": "Nodes/A_new/A_new.kicad_pcb", "name": "A_new"})], "this board and other projects' boards are left out");
        assert!(elsewhere(&asking, "/p/x.kicad_pcb", "").is_empty());
    }
    #[test] fn plan_panel_counts_summary_impedances_and_item_details() {
        let plan = json!({
            "revision": 7,
            "layers": [{"layer": "F.Cu", "role": "signal", "status": "approved"}],
            "groups": [{"id": "usb", "name": "USB_DP/DM", "color": "#4f8df5", "nets": ["USB_P", "USB_N"], "width": 0.25, "impedance": "90 ohm diff"}],
            "corridors": [
                {"id": "c1", "group": "usb", "layer": "F.Cu", "status": "verified", "progress": {"routed": 2, "total": 2}},
                {"id": "c2", "group": "usb", "layer": "F.Cu", "status": "approved"}
            ],
            "planes": [{"id": "p1", "net": "GND", "layer": "F.Cu", "status": "routed"}],
            "derived": {
                "stackup": {"F.Cu": "Microstrip over GND"},
                "impedances": ["90 ohm diff USB", "50 ohm SE"],
                "items": {"c2": {"problems": [{"message": "Clearance violation near U3"}]}}
            }
        });
        let d = data(&plan, &[], true);
        assert_eq!((d["total"].as_u64(), d["done"].as_u64(), d["verified"].as_u64(), d["routed"].as_u64(), d["blocked"].as_u64()), (Some(3), Some(2), Some(1), Some(1), Some(1)));
        assert_eq!(d["status"], "blocked");
        assert_eq!(d["stackup_summary"], "F.Cu: Microstrip over GND");
        assert_eq!(d["impedances"], json!(["90 ohm diff USB", "50 ohm SE"]));
        let c1 = &d["layers"][0]["items"][0];
        assert_eq!((c1["width"].as_f64(), c1["impedance"].as_str()), (Some(0.25), Some("90 ohm diff")));
        assert_eq!(d["layers"][0]["items"][1]["problem_text"], "Clearance violation near U3");
        assert!(crate::api::RpcHandler::methods(&crate::plan::Rpc).contains(&"plan.verify"));

        let rejected_only = json!({"corridors": [{"id": "c1", "group": "usb", "layer": "F.Cu", "status": "rejected"}]});
        assert_eq!(data(&rejected_only, &[], false)["status"], "rejected");
        let failed_item = json!({"corridors": [{"id": "c1", "group": "usb", "layer": "F.Cu", "status": "failed"}]});
        assert_eq!(data(&failed_item, &[], false)["status"], "blocked");
        let partial_plan = json!({"corridors": [
            {"id": "c1", "group": "usb", "layer": "F.Cu", "status": "routed"},
            {"id": "c2", "group": "usb", "layer": "F.Cu", "status": "rejected"}
        ]});
        assert_eq!(data(&partial_plan, &[], false)["status"], "partial");
    }
    #[test] fn plan_panel_enriches_board_geometry_and_respects_info_budget() {
        let pcb = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/files/fixtures/plot/ecc83-pp.kicad_pcb");
        let plan = json!({
            "revision": 2,
            "layers": [{"layer": "F.Cu", "role": "signal", "status": "proposed"}],
            "groups": [{"id": "g1", "name": "SignalA", "color": "#14b8a6", "nets": ["Net-(U1A-K)", "GND"]}],
            "corridors": [{"id": "c1", "group": "g1", "layer": "F.Cu", "path": [[120.0, 100.0], [150.0, 100.0]], "width": 2.0, "from": ["U1"], "to": ["P2"], "status": "proposed"}]
        });
        let d = data_with_board(&plan, &[], false, Some(pcb.to_str().unwrap()));
        assert!(!d["board"]["outline"].as_array().unwrap().is_empty());
        assert!(!d["board"]["parts"].as_array().unwrap().is_empty());
        assert_eq!(d["layers"][0]["title"], "L1 · F.Cu");
        assert!(d["layers"][0]["subtitle"].as_str().unwrap().contains("1510 µm"));
        assert_eq!(d["draw"]["bands"][0]["name"], "SignalA");

        // Ensure oversized payload (>58 KiB) is shrunk safely under 64 KiB
        let huge_parts: Vec<Value> = (0..1200).map(|i| json!({"r": format!("R{i}"), "s": "F", "b": [1.0, 2.0, 3.0, 4.0], "o": false})).collect();
        let huge_guides: Vec<Value> = (0..1500).map(|i| json!([i as f64, 1.0, (i + 1) as f64, 2.0])).collect();
        let mut big = json!({
            "board": {"bbox": [0, 0, 100, 100], "outline": [], "parts": huge_parts},
            "draw": {"bands": [{"id": "c1", "guides": huge_guides, "path": vec![json!([0, 0]); 200]}], "regions": []}
        });
        assert!(big.to_string().len() > 58 * 1024);
        fit_info_budget(&mut big);
        assert!(big.to_string().len() <= 58 * 1024);
        assert_eq!(big["board_trimmed"], true);

        // Verify holes count against MAX_POINTS and massive notes/holes/items are trimmed <= 58 KiB
        let hole_pts: Vec<Value> = (0..(MAX_POINTS - 5)).map(|i| json!([i as f64, 1.0])).collect();
        let mut huge_plan = json!({
            "layers": [{"layer": "In1.Cu", "role": "power", "status": "proposed"}],
            "planes": [{"id": "p1", "net": "GND", "layer": "In1.Cu", "outline": [[[0, 0], [10, 0], [10, 10], [0, 10]], hole_pts.clone(), hole_pts], "status": "proposed"}]
        });
        let dr_holes = data(&huge_plan, &[], false);
        assert_eq!(dr_holes["draw"]["regions"][0]["holes"].as_array().unwrap().len(), 1, "second huge hole exceeds MAX_POINTS and is dropped");
        huge_plan["notes"] = json!((0..400).map(|i| json!({"id": format!("n{i}"), "text": "a".repeat(250)})).collect::<Vec<_>>());
        huge_plan["corridors"] = json!((0..250).map(|i| json!({"id": format!("c{i}"), "group": "g1", "layer": "F.Cu", "path": [[1, 1], [2, 2]], "status": "proposed"})).collect::<Vec<_>>());
        let mut huge_d = data(&huge_plan, &[], false);
        assert!(huge_d.to_string().len() > 58 * 1024);
        fit_info_budget(&mut huge_d);
        assert!(huge_d.to_string().len() <= 58 * 1024);
        assert_eq!(huge_d["board_trimmed"], true);
    }
}

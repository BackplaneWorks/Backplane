//! The routing floorplanner, Backplane's side (docs/floorplanner.md; the fork's docs/backplane/floorplanner.md and
//! api/proto/backplane/plan.proto): agents propose a plan (corridors for net groups on signal layers, plane regions on power layers), the
//! person steers and approves it, routing follows only what they approved.
//!
//! The agents' tools `plan_get`, `plan_propose`, `plan_update`, `plan_approve`, `plan_route`, `plan_stop`, `plan_verify` wrap the fork's
//! `kiapi.backplane.plan` commands through `kicad_ipc` (the open KiCad, else the project's headless one; the agent's own actor, its
//! holds and pauses), gated on the runtime's `routing-plan` capability. Each answers what changed in the plan since that thread last read
//! it (`changes`). Approval is the person's alone: an agent's `plan_approve` only asks them; `plan.approve` (the Plan panel, a phone) sends
//! KiCad's ApprovePlan as the person. `plan_route` is refused here, before KiCad is asked, unless what it would route is approved.
//! Laws `plan_*`.
pub mod events;

use crate::{agents::mcp::ToolSet, api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex, OnceLock}};

pub const CAP: &str = "routing-plan";
const NS: &str = "kiapi.backplane.plan.";

// ---- the plan, pure ----

/// One corridor or plane region as the gate and the diff see it.
#[derive(Clone, Debug, PartialEq)]
pub struct Item { pub id: String, pub kind: &'static str, pub layer: String, pub group: String, pub status: String, pub edited: bool, pub locked: bool, pub nets: usize, pub progress: Option<(u64, u64)> }
fn st<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
pub fn layer_status<'a>(plan: &'a Value, layer: &str) -> &'a str { plan["layers"].as_array().into_iter().flatten().find(|l| st(l, "layer") == layer).map_or("proposed", |l| st(l, "status")) }
fn group<'a>(plan: &'a Value, id: &str) -> Option<&'a Value> { plan["groups"].as_array().into_iter().flatten().find(|g| st(g, "id") == id) }
pub fn items(plan: &Value) -> Vec<Item> {
    let mut v = vec![];
    for c in plan["corridors"].as_array().into_iter().flatten() {
        let nets = group(plan, st(c, "group")).and_then(|g| g["nets"].as_array()).map_or(0, Vec::len);
        v.push(Item { id: st(c, "id").into(), kind: "corridor", layer: st(c, "layer").into(), group: st(c, "group").into(), status: st(c, "status").into(), edited: c["edited"] == true, locked: c["locked"] == true, nets,
            progress: c["progress"]["total"].as_u64().map(|t| (c["progress"]["routed"].as_u64().unwrap_or(0), t)) });
    }
    for p in plan["planes"].as_array().into_iter().flatten() {
        v.push(Item { id: st(p, "id").into(), kind: "plane", layer: st(p, "layer").into(), group: st(p, "net").into(), status: st(p, "status").into(), edited: p["edited"] == true, locked: p["locked"] == true, nets: 1,
            progress: p["progress"]["total"].as_u64().map(|t| (p["progress"]["routed"].as_u64().unwrap_or(0), t)) });
    }
    v
}
/// The fork's own rule (BOARD_PLAN::Approved): a corridor routes when it is approved, its group not rejected and its layer not
/// rejected; a plane when it is approved and its layer not rejected. Items already routing, routed or verified go on.
pub fn routable(plan: &Value, it: &Item) -> bool {
    if matches!(it.status.as_str(), "routing" | "routed" | "verified") { return true }
    if it.status != "approved" || layer_status(plan, &it.layer) == "rejected" { return false }
    it.kind == "plane" || group(plan, &it.group).map_or(true, |g| st(g, "status") != "rejected")
}
/// Plan.gate: what `plan_route` may route: the ids asked (all of them approved), or with none every approved item (at least one).
/// Err names what waits for the person's approval.
pub fn gate(plan: &Value, ids: &[String]) -> Result<Vec<String>, String> {
    let all = items(plan);
    if ids.is_empty() {
        let ok: Vec<String> = all.iter().filter(|i| routable(plan, i) && !matches!(i.status.as_str(), "routed" | "verified")).map(|i| i.id.clone()).collect();
        if ok.is_empty() {
            let waiting: Vec<&str> = all.iter().filter(|i| i.status == "proposed").map(|i| i.id.as_str()).collect();
            return Err(format!("nothing in the plan is approved to route{}: the person approves corridors and planes (the Plan panel, or KiCad's Routing plan panel); wait for it (plan_get answers what changed)", if waiting.is_empty() { String::new() } else { format!(" ({} waiting: {})", waiting.len(), waiting.iter().take(12).cloned().collect::<Vec<_>>().join(", ")) }))
        }
        return Ok(ok)
    }
    let mut bad = vec![];
    for id in ids { match all.iter().find(|i| &i.id == id) { None => bad.push(format!("{id} (no such item)")), Some(i) if !routable(plan, i) => bad.push(format!("{id} ({}{})", i.status, if layer_status(plan, &i.layer) == "rejected" { ", its layer rejected" } else { "" })), _ => {} } }
    if bad.is_empty() { Ok(ids.to_vec()) } else { Err(format!("not approved by the person, so not routed: {}. Ask them (plan_approve) and wait", bad.join(", "))) }
}
/// Plan.diff: what changed between two readings of a plan, in words an agent acts on: items added, removed, their status, the person's
/// edits (`edited`, `author: person`), locks, progress, layers judged, notes and constraints added.
pub fn diff(old: &Value, new: &Value) -> Vec<Value> {
    let mut out = vec![];
    let (a, b) = (items(old), items(new));
    for i in &b {
        match a.iter().find(|x| x.id == i.id) {
            None => out.push(json!({"id": i.id, "kind": i.kind, "change": "added", "status": i.status, "layer": i.layer})),
            Some(o) => {
                if o.status != i.status { out.push(json!({"id": i.id, "kind": i.kind, "change": "status", "from": o.status, "to": i.status})) }
                if !o.edited && i.edited { out.push(json!({"id": i.id, "kind": i.kind, "change": "edited by the person"})) }
                if o.layer != i.layer { out.push(json!({"id": i.id, "kind": i.kind, "change": "layer", "from": o.layer, "to": i.layer})) }
                if o.locked != i.locked { out.push(json!({"id": i.id, "kind": i.kind, "change": if i.locked { "locked" } else { "unlocked" }})) }
                if o.progress != i.progress { if let Some((r, t)) = i.progress { out.push(json!({"id": i.id, "kind": i.kind, "change": "progress", "routed": r, "total": t})) } }
            }
        }
    }
    for o in &a { if !b.iter().any(|x| x.id == o.id) { out.push(json!({"id": o.id, "kind": o.kind, "change": "removed"})) } }
    for l in new["layers"].as_array().into_iter().flatten() { let was = layer_status(old, st(l, "layer")); if was != st(l, "status") { out.push(json!({"layer": st(l, "layer"), "change": "layer status", "from": was, "to": st(l, "status")})) } }
    for k in ["notes", "constraints"] {
        let ids: Vec<&str> = old[k].as_array().into_iter().flatten().map(|n| st(n, "id")).collect();
        for n in new[k].as_array().into_iter().flatten().filter(|n| !ids.contains(&st(n, "id"))) { out.push(json!({"id": st(n, "id"), "change": if k == "notes" { "note" } else { "constraint" }, "text": n["text"], "author": n["author"], "target": n["target"]})) }
    }
    out
}
/// What a plan says at a glance: items by status, what awaits the person.
pub fn summary(plan: &Value) -> Value {
    let all = items(plan); let mut by: HashMap<String, usize> = HashMap::new();
    for i in &all { *by.entry(if i.edited && i.status == "proposed" { "edited".into() } else { i.status.clone() }).or_default() += 1 }
    json!({"revision": plan["revision"], "items": all.len(), "by_status": by, "awaiting_approval": all.iter().filter(|i| i.status == "proposed").map(|i| i.id.clone()).collect::<Vec<_>>()})
}

// ---- the tools ----

/// What each thread last read of each board's plan (for `changes`).
static SEEN: OnceLock<Mutex<HashMap<(String, String), Value>>> = OnceLock::new();
fn seen() -> &'static Mutex<HashMap<(String, String), Value>> { SEEN.get_or_init(Default::default) }

pub const NAMES: &[&str] = &["plan_get", "plan_propose", "plan_update", "plan_approve", "plan_route", "plan_stop", "plan_verify"];
fn sv<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn s(d: &str) -> Value { json!({"type": "string", "description": d}) }
fn def(name: &str, desc: &str, props: Value) -> Value { json!({"name": name, "description": desc, "inputSchema": {"type": "object", "properties": props, "required": []}}) }
pub fn defs() -> Vec<Value> {
    let path = || s("the board (.kicad_pcb) or project; default the thread's project's board");
    vec![
        def("plan_get", "The board's routing plan (the floorplan before copper: corridors per net group on signal layers, plane regions per net on power layers), with its derived data (stack-up per layer, audit, progress) and `changes`: what changed since you last read it (the person's edits, approvals, rejections, notes and constraints, progress). Read it before proposing and after each wait.", json!({"path": path(), "derived": {"type": "boolean", "description": "default true"}})),
        def("plan_propose", "Propose a plan or part of one (JSON as the plan file: layers, groups, corridors, planes, splits, notes; mm, board coordinates). mode merge (default: upsert by id) or replace (drop your unlocked, still-proposed items first). Items the person locked or edited are never replaced: they come back in `refused` with the reason. Then ask the person to approve (plan_approve) and wait.", json!({"path": path(), "plan": {"type": "object", "description": "the plan JSON (any subset)"}, "mode": s("merge or replace")})),
        def("plan_update", "Change plan items: ops as the plan API's JSON array ({op: set|add|delete|status|progress, id, ...}), with base = the revision you read. An op on an item the person changed after base, or a locked one, is refused (the person's edit wins).", json!({"path": path(), "base": {"type": "number"}, "ops": {"type": "array", "items": {"type": "object"}}})),
        def("plan_approve", "Ask the person to approve the plan (a layer, items, or all). Approval is theirs alone: this only asks them on their screen; they approve in the Plan panel or in KiCad. Then wait (plan_get tells you when they did).", json!({"path": path(), "layer": s("a layer to approve, e.g. In1.Cu"), "ids": {"type": "array", "items": {"type": "string"}}, "note": s("one line on what to look at")})),
        def("plan_route", "Route the approved plan, one step (up to max_nets nets of the next approved corridor); call again until done. Refused unless what it would route is approved by the person (the ids given, or with none at least one approved item).", json!({"path": path(), "ids": {"type": "array", "items": {"type": "string"}}, "max_nets": {"type": "number"}})),
        def("plan_stop", "Stop routing the plan (back to planning): finished copper stays.", json!({"path": path()})),
        def("plan_verify", "Check the routed groups (DRC on their nets, unrouted connections, length and skew against their targets); groups that pass become verified.", json!({"path": path()})),
    ]
}

/// The board a call is about: `path` (a board or any file of a project), else the thread's project's board.
async fn board_of(hub: &Arc<Hub>, thread: &str, path: &str) -> Result<String, String> {
    let root = if !path.is_empty() { PathBuf::from(path) } else { let r = crate::files::root_of(hub, thread).await.ok_or("this thread has no project folder")?; PathBuf::from(crate::files::kicad::canonical::design_root(&r.to_string_lossy())) };
    let base = if root.is_dir() { root.clone() } else { root.parent().map(Path::to_path_buf).unwrap_or_default() };
    if root.extension().map_or(false, |x| x == "kicad_pcb") { return Ok(root.to_string_lossy().into_owned()) }
    let f = crate::files::kicad::project::read(&base.to_string_lossy());
    if f.pcb.is_empty() { Err("no KiCad board in this project".into()) } else { Ok(f.pcb) }
}
fn spec(pcb: &str) -> Value { crate::fab::setup::spec(pcb) }
/// Whether the KiCad this board goes to has the plan API.
async fn capable(hub: &Arc<Hub>, thread: Option<&str>, pcb: &str) -> Result<(), String> {
    let st = crate::files::kicad::ipc::kicad_ipc_status_in(hub, thread, pcb).await?;
    let caps = st["capabilities"]["capabilities"].as_array().or(st["capabilities"].as_array()).cloned().unwrap_or_default();
    let txt = st["capabilities"].to_string();
    if caps.iter().any(|c| c == CAP) || txt.contains(CAP) { Ok(()) } else { Err(format!("this KiCad ({}) has no routing plan: it needs Backplane KiCad bp.11 or later", st["version"].as_str().unwrap_or("unknown version"))) }
}
async fn call(hub: &Arc<Hub>, thread: Option<&str>, pcb: &str, ty: &str, msg: Value) -> Result<Value, String> {
    let o = crate::files::kicad::ipc::kicad_ipc_in(hub, thread, pcb, &format!("{NS}{ty}"), msg).await?;
    Ok(o.get("message").cloned().unwrap_or(o))
}
pub async fn read_plan(hub: &Arc<Hub>, thread: Option<&str>, pcb: &str, derived: bool) -> Result<Value, String> {
    let m = call(hub, thread, pcb, "GetPlan", json!({"board": spec(pcb), "derived": derived})).await?;
    let mut v: Value = serde_json::from_str(m["json"].as_str().unwrap_or("{}")).map_err(|_| "KiCad's plan is not JSON")?;
    if v["revision"].is_null() { v["revision"] = m["revision"].clone() }
    Ok(v)
}
/// Read the plan and answer it with what changed since this thread last read it.
async fn read_with_changes(hub: &Arc<Hub>, thread: &str, pcb: &str, derived: bool) -> Result<(Value, Vec<Value>), String> {
    let p = read_plan(hub, Some(thread), pcb, derived).await?;
    let key = (thread.to_string(), pcb.to_string());
    let old = seen().lock().unwrap().insert(key, p.clone());
    let ch = match old { Some(o) => diff(&o, &p), None => vec![] };
    Ok((p, ch))
}

pub async fn tool(hub: &Arc<Hub>, thread: &str, name: &str, a: &Value) -> Result<Value, String> {
    let pcb = board_of(hub, thread, sv(a, "path")).await?;
    if name == "plan_approve" {
        // the agent asks; the person approves (the Plan panel, KiCad, a phone): nothing is approved here
        let what = if !sv(a, "layer").is_empty() { format!("layer {}", sv(a, "layer")) } else if let Some(ids) = a["ids"].as_array().filter(|x| !x.is_empty()) { ids.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ") } else { "the plan".into() };
        events::ask(hub, thread, &pcb, &what, sv(a, "note")).await;
        return Ok(json!({"asked": true, "what": what, "next": "The person approves in the Plan panel or in KiCad; end your turn or wait: plan_get answers their decision in `changes`."}))
    }
    capable(hub, Some(thread), &pcb).await?;
    let ids: Vec<String> = a["ids"].as_array().map(|x| x.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let reply = match name {
        "plan_get" => { let (p, ch) = read_with_changes(hub, thread, &pcb, a["derived"] != false).await?; return Ok(json!({"summary": summary(&p), "changes": ch, "plan": p})) }
        "plan_propose" => {
            let plan = if a["plan"].is_string() { a["plan"].as_str().unwrap_or("").to_string() } else { a["plan"].to_string() };
            if plan.len() < 3 { return Err("plan: the plan JSON to propose".into()) }
            let mode = if sv(a, "mode") == "replace" { "PM_REPLACE_PROPOSED" } else { "PM_MERGE" };
            call(hub, Some(thread), &pcb, "ProposePlan", json!({"board": spec(&pcb), "json": plan, "mode": mode})).await?
        }
        "plan_update" => call(hub, Some(thread), &pcb, "UpdatePlan", json!({"board": spec(&pcb), "base": a["base"].as_u64().unwrap_or(0), "ops": a["ops"].to_string()})).await?,
        "plan_route" => {
            // the audit gate, here, before KiCad is asked
            let p = read_plan(hub, Some(thread), &pcb, false).await?;
            gate(&p, &ids)?;
            call(hub, Some(thread), &pcb, "RoutePlan", json!({"board": spec(&pcb), "ids": ids, "max_nets": a["max_nets"].as_i64().unwrap_or(0)})).await?
        }
        "plan_stop" => call(hub, Some(thread), &pcb, "StopPlanRouting", json!({"board": spec(&pcb)})).await?,
        "plan_verify" => call(hub, Some(thread), &pcb, "VerifyPlan", json!({"board": spec(&pcb)})).await?,
        _ => return Err("unknown tool".into()),
    };
    let (p, ch) = read_with_changes(hub, thread, &pcb, false).await.unwrap_or((Value::Null, vec![]));
    Ok(json!({"reply": reply, "summary": if p.is_null() { Value::Null } else { summary(&p) }, "changes": ch}))
}

pub struct Tools(pub Arc<Hub>);
impl ToolSet for Tools {
    fn list(&self, _thread: &str) -> Vec<Value> { defs() }
    fn owns(&self, name: &str) -> bool { NAMES.contains(&name) }
    fn call<'a>(&'a self, thread: &'a str, name: &'a str, a: &'a Value) -> Fut<'a, (bool, Value)> {
        Box::pin(async move { match tool(&self.0, thread, name, a).await { Ok(v) => (true, v), Err(e) => (false, json!({"error": e})) } })
    }
}

/// The person's decision (the Plan panel, a phone): `plan.approve {pro|path, layer?, ids?, approve}` -> KiCad's ApprovePlan as the person.
pub async fn approve(hub: &Arc<Hub>, pcb: &str, layer: &str, ids: &[String], yes: bool) -> Result<Value, String> {
    let msg = json!({"board": spec(pcb), "layer": layer, "ids": ids, "approve": yes});
    let pro = crate::files::kicad::project::read(&Path::new(pcb).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()).pro;
    let o = match crate::files::kicad::embed::gui_route(&pro) {
        Some(r) => { let o = crate::files::kicad::embed::gui_call_as(&r, &format!("{NS}ApprovePlan"), msg, &crate::files::kicad::coedit::client_name(&crate::files::kicad::coedit::Who::Person)).await; if o["ok"].as_bool().unwrap_or(false) { o["message"].clone() } else { return Err(format!("KiCad: {}", o["error"].as_str().unwrap_or("no answer"))) } }
        None => call(hub, None, pcb, "ApprovePlan", msg).await?,
    };
    events::changed(hub, pcb).await;
    Ok(o)
}
pub struct Rpc;
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["plan.approve", "plan.stop", "plan.get"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let path = if !sv(p, "pro").is_empty() { sv(p, "pro") } else { sv(p, "path") };
            let r = async {
                if crate::copy::on() && m != "plan.get" { return Err(crate::copy::HOME_ONLY.to_string()) }
                let pcb = board_of(hub, "", path).await?;
                let ids: Vec<String> = p["ids"].as_array().map(|x| x.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                match m {
                    "plan.approve" => approve(hub, &pcb, sv(p, "layer"), &ids, p["approve"] != false).await,
                    "plan.stop" => { let r = call(hub, None, &pcb, "StopPlanRouting", json!({"board": spec(&pcb)})).await; events::changed(hub, &pcb).await; r }
                    _ => read_plan(hub, None, &pcb, true).await,
                }
            }.await;
            vec![match r { Ok(v) => json!({"t": "reply", "id": id, "ok": true, "result": v}), Err(e) => json!({"t": "reply", "id": id, "ok": false, "error": e}) }]
        })
    }
}

pub fn start(hub: &Arc<Hub>, ag: Option<&Arc<crate::agents::Agents>>) {
    hub.register_rpc(Arc::new(Rpc));
    if let Some(ag) = ag { ag.add_tools(Arc::new(Tools(hub.clone()))) }
    events::start(hub);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> Value { json!({"revision": 3, "layers": [{"layer": "In1.Cu", "status": "proposed"}, {"layer": "In2.Cu", "status": "rejected"}],
        "groups": [{"id": "g1", "nets": ["A", "B", "C"], "status": "proposed"}, {"id": "g2", "nets": ["D"], "status": "rejected"}],
        "corridors": [{"id": "c1", "group": "g1", "layer": "In1.Cu", "status": "approved"}, {"id": "c2", "group": "g1", "layer": "In1.Cu", "status": "proposed"},
            {"id": "c3", "group": "g2", "layer": "In1.Cu", "status": "approved"}, {"id": "c4", "group": "g1", "layer": "In2.Cu", "status": "approved"}],
        "planes": [{"id": "p1", "net": "GND", "layer": "In1.Cu", "status": "approved"}, {"id": "p2", "net": "+3V3", "layer": "In1.Cu", "status": "proposed"}]}) }
    #[test] fn plan_gate_routes_only_what_the_person_approved() {
        let p = plan();
        assert_eq!(gate(&p, &[]).unwrap(), ["c1", "p1"], "approved, its group and layer not rejected");
        let e = gate(&p, &["c2".into(), "c3".into(), "c4".into(), "zz".into()]).unwrap_err();
        assert!(e.contains("c2 (proposed)") && e.contains("c3 (approved)") && e.contains("c4 (approved, its layer rejected)") && e.contains("zz (no such item)"), "{e}");
        assert_eq!(gate(&p, &["c1".into()]).unwrap(), ["c1"]);
        let none = json!({"corridors": [{"id": "c9", "status": "proposed", "layer": "F.Cu"}]});
        assert!(gate(&none, &[]).unwrap_err().contains("1 waiting: c9"));
    }
    #[test] fn plan_diff_says_what_the_person_did() {
        let a = plan(); let mut b = plan();
        b["corridors"][1]["status"] = json!("approved"); b["corridors"][0]["edited"] = json!(true); b["corridors"][0]["locked"] = json!(true);
        b["layers"][0]["status"] = json!("approved"); b["planes"].as_array_mut().unwrap().remove(1);
        b["corridors"].as_array_mut().unwrap().push(json!({"id": "c5", "group": "g1", "layer": "In1.Cu", "status": "proposed"}));
        b["notes"] = json!([{"id": "n1", "text": "keep away from U2", "author": "person", "target": "c1"}]);
        b["corridors"][2]["progress"] = json!({"routed": 2, "total": 4});
        let d = diff(&a, &b); let ch: Vec<String> = d.iter().map(|x| format!("{} {}", x["id"].as_str().or(x["layer"].as_str()).unwrap_or(""), x["change"].as_str().unwrap())).collect();
        assert_eq!(ch, ["c1 edited by the person", "c1 locked", "c2 status", "c3 progress", "c5 added", "p2 removed", "In1.Cu layer status", "n1 note"]);
        assert!(diff(&a, &a).is_empty());
        assert_eq!(summary(&a)["awaiting_approval"], json!(["c2", "p2"]));
    }
}

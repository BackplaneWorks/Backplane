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
pub mod geom;
pub mod panel;

use crate::{agents::mcp::ToolSet, api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex, OnceLock}};

pub const CAP: &str = "routing-plan";
/// When a board is routed by plan: 4 or more copper layers, or more than 60 nets (what agents are told; `needs_plan` is the rule).
pub const LAYERS: usize = 4;
pub const NETS: usize = 60;
/// Whether a KiCad (its status and version) has the plan API: Backplane KiCad from bp.11.
/// Whether the runtime has the plan API: its manifest's `routing-plan` capability; a runtime whose manifest lists no capabilities is
/// told by its version (bp.11 on; "10.99.0-bp.13", "10.99.0-unknown-bp.13", "10.99.0-bp.12-851cbefda6" and a describe name alike).
pub fn plans(status: &str, version: &str, caps: &[String]) -> bool {
    // the version by the one reader (ks::bp_number): a test build ("-bp.12-851cbefda6") and a describe name ("...-dirty-bp.13") alike
    status == "ok" && (caps.iter().any(|c| c == CAP) || (caps.is_empty() && crate::files::kicad::kserve::bp_number(version) >= 11))
}
/// The person's switch for planning before routing (Settings › CAD, the Plan panel): off unless they turn it on. Off, agents route as
/// asked and are told to use the plan_* tools only when the person asks for a routing plan; on, they plan first and route only what the
/// person approved (`guidance`).
pub const SETTING: &str = "plan.routing";
pub fn is_on(v: &str) -> bool { v == "on" }
pub fn needs_plan(copper_layers: usize, nets: usize) -> bool { copper_layers >= LAYERS || nets > NETS }
/// In plan view the plan tool's keys (A, R, W, Shift+W, [, ], N, L) are its own, never KiCad's defaults: the fork's `plan-keys` (bp.12.2,
/// bp.13 batch 6). Without it (bp.12.1: the tool registered as pcbnew.PlanTool, its actions pcbnew.Plan.*) A opens Add Footprint and R
/// rotates; the Plan panel's Approve and Reject work (KiCad's ApprovePlan), so they stay, and the panel says where to approve.
pub const KEYS_CAP: &str = "plan-keys";
pub const KEYS_NOTE: &str = "On this KiCad version, approve from this panel: in plan view KiCad's A, R and W keys aren't the plan's yet (A opens Add Footprint). Update KiCad.";
pub fn keys_note(caps: &[String]) -> Option<&'static str> {
    let has = |c: &str| caps.iter().any(|x| x == c);
    (has(CAP) && !has(KEYS_CAP)).then_some(KEYS_NOTE)
}
/// After an approval KiCad accepted: the items it named still awaiting the person (proposed or edited) mean it was not recorded.
/// `ids` empty and `layer` empty: every item; `layer`: that layer's status and its items.
pub fn not_recorded(plan: &Value, layer: &str, ids: &[String]) -> Vec<String> {
    let waiting = |v: &Value| matches!(v["status"].as_str().unwrap_or("proposed"), "proposed" | "edited");
    let items = ["corridors", "planes"].iter().flat_map(|k| plan[*k].as_array().cloned().unwrap_or_default()).collect::<Vec<_>>();
    if !layer.is_empty() {
        let l = plan["layers"].as_array().into_iter().flatten().find(|x| x["layer"] == layer);
        return if l.is_some_and(|l| waiting(l)) { vec![layer.to_string()] } else { vec![] }
    }
    items.iter().filter(|i| ids.is_empty() || ids.iter().any(|d| i["id"] == d.as_str())).filter(|i| waiting(i)).filter_map(|i| i["id"].as_str().map(str::to_string)).collect()
}
/// The agents' instructions for a project with a KiCad board on Backplane KiCad (agents/prompt.rs adds it).
pub fn guidance() -> String {
    format!("<backplane_routing_plan>The person turned on Plan before routing (Settings › CAD, or the Plan panel beside the board). Plan before copper on a board with {LAYERS} or more copper layers or more than {NETS} nets (fewer: route as asked): before routing anything, read the stackup with the backplane tool `fab_profile` (its `stackup.layers`: each copper layer's reference planes above and below with the dielectric height and Er, and the trace widths for the project's impedances) and the plan with `plan_get`; then propose with `plan_propose`: on signal layers, one corridor per bus or net group (its nets, the parts it runs between, a path and a width, vias and escapes, the routing order), each next to a solid reference plane, impedance nets at the stackup's widths; on power layers, plane regions per net with their reference; one layer at a time is fine. Ask the person to approve with `plan_approve` and end your turn: Backplane tells you when they have decided. Their edits, locks, notes and constraints win: read `changes` from `plan_get` and never undo them. Route only what they approved, with `plan_route` (one step per call, until done), inside its corridor; nothing else, and no copper outside the plan. `plan_stop` stops; `plan_verify` checks the routed groups. The plan needs the board open in KiCad.</backplane_routing_plan>")
}
/// The agents' words while the person has Plan before routing off (the default).
pub fn guidance_off() -> &'static str {
    "<backplane_routing_plan>Plan before routing is off (the person's choice; Settings › CAD or the Plan panel turns it on): route a board as the person asks, without proposing a routing plan or waiting for an approval first. Use the plan_* tools only when the person asks for a routing plan.</backplane_routing_plan>"
}
/// The link a plan review is asked with (`backplane://plan/<the board from the project folder>`): the desktop draws it as a button that opens the board
/// with its Plan panel (gpui screens/linkroute.rs `Route::Plan`).
pub fn review_link(root: &str, pcb: &str) -> String {
    let root = root.trim_end_matches('/');
    let rel = if !root.is_empty() { pcb.strip_prefix(&format!("{root}/")).unwrap_or(pcb) } else { pcb };
    let rel = rel.trim_start_matches('/');
    crate::md::refs::uri(&crate::md::refs::Ref { kind: "plan".into(), rest: rel.split('/').filter(|x| !x.is_empty()).map(str::to_string).collect(), frag: String::new() })
}
const NS: &str = "kiapi.backplane.plan.";

// ---- the plan, pure ----

/// One corridor or plane region as the gate and the diff see it.
#[derive(Clone, Debug, PartialEq)]
pub struct Item { pub id: String, pub kind: &'static str, pub layer: String, pub group: String, pub status: String, pub edited: bool, pub locked: bool, pub nets: usize, pub progress: Option<(u64, u64)>, pub geom_hash: u64 }
fn st<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
pub fn layer_status<'a>(plan: &'a Value, layer: &str) -> &'a str { plan["layers"].as_array().into_iter().flatten().find(|l| st(l, "layer") == layer).map_or("proposed", |l| st(l, "status")) }
fn group<'a>(plan: &'a Value, id: &str) -> Option<&'a Value> { plan["groups"].as_array().into_iter().flatten().find(|g| st(g, "id") == id) }
fn fnv_val(v: &Value, mut h: u64) -> u64 {
    for b in v.to_string().as_bytes() { h = (h ^ *b as u64).wrapping_mul(1099511628211); }
    h
}
fn item_geom_hash(v: &Value, keys: &[&str]) -> u64 {
    let mut h = 14695981039346656037u64;
    for k in keys { h = fnv_val(&v[*k], h ^ k.len() as u64); }
    h
}
pub fn items(plan: &Value) -> Vec<Item> {
    let mut v = vec![];
    for c in plan["corridors"].as_array().into_iter().flatten() {
        let nets = group(plan, st(c, "group")).and_then(|g| g["nets"].as_array()).map_or(0, Vec::len);
        v.push(Item { id: st(c, "id").into(), kind: "corridor", layer: st(c, "layer").into(), group: st(c, "group").into(), status: st(c, "status").into(), edited: c["edited"] == true, locked: c["locked"] == true, nets,
            progress: c["progress"]["total"].as_u64().map(|t| (c["progress"]["routed"].as_u64().unwrap_or(0), t)),
            geom_hash: item_geom_hash(c, &["path", "width", "vias"]) });
    }
    for p in plan["planes"].as_array().into_iter().flatten() {
        v.push(Item { id: st(p, "id").into(), kind: "plane", layer: st(p, "layer").into(), group: st(p, "net").into(), status: st(p, "status").into(), edited: p["edited"] == true, locked: p["locked"] == true, nets: 1,
            progress: p["progress"]["total"].as_u64().map(|t| (p["progress"]["routed"].as_u64().unwrap_or(0), t)),
            geom_hash: item_geom_hash(p, &["outline", "reference", "priority"]) });
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
                if (!o.edited && i.edited) || (i.edited && o.geom_hash != i.geom_hash) { out.push(json!({"id": i.id, "kind": i.kind, "change": "edited by the person"})) }
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
    let o = crate::files::kicad::ipc::kicad_ipc_in(hub, thread, pcb, &format!("{NS}{ty}"), msg).await
        .map_err(|e| if e.contains("AS_UNHANDLED") { "the routing plan needs the board open in KiCad (Backplane KiCad's background server has no plan API yet): ask the person to open it (Open in KiCad), then try again".to_string() } else { e })?;
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
        let root = crate::files::root_of(hub, thread).await.map(|r| crate::files::kicad::canonical::design_root(&r.to_string_lossy())).unwrap_or_default();
        let link = review_link(&root, &pcb);
        return Ok(json!({"asked": true, "what": what, "link": link, "next": format!("The person approves in the Plan panel beside the board or in KiCad. End your turn with one short line for them that ends with exactly this link (Backplane draws it as a button that opens the plan): [Review the routing plan]({link}). Once they have judged it, Backplane sends you a message saying what they approved or rejected (plan_get answers the details in `changes`).")}))
    }
    capable(hub, Some(thread), &pcb).await?;
    let ids: Vec<String> = a["ids"].as_array().map(|x| x.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let reply = match name {
        "plan_get" => { let (p, ch) = read_with_changes(hub, thread, &pcb, a["derived"] != false).await?; events::changed(hub, &pcb).await; return Ok(json!({"summary": summary(&p), "changes": ch, "plan": p})) }
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
            call(hub, Some(thread), &pcb, "RoutePlan", json!({"board": spec(&pcb), "ids": ids, "max_nets": a["max_nets"].as_i64().unwrap_or(0)})).await.map_err(|e| route_busy(&e))?
        }
        "plan_stop" => call(hub, Some(thread), &pcb, "StopPlanRouting", json!({"board": spec(&pcb)})).await?,
        "plan_verify" => call(hub, Some(thread), &pcb, "VerifyPlan", json!({"board": spec(&pcb)})).await?,
        _ => return Err("unknown tool".into()),
    };
    let (p, ch) = read_with_changes(hub, thread, &pcb, false).await.unwrap_or((Value::Null, vec![]));
    events::changed(hub, &pcb).await;
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

fn round3(v: f64) -> f64 { (v * 1000.0).round() / 1000.0 }
fn norm_pt(v: &Value) -> Result<Value, String> {
    let a = v.as_array().filter(|x| x.len() >= 2).ok_or_else(|| "point must be [x, y]".to_string())?;
    let (x, y) = (a[0].as_f64().unwrap_or(f64::NAN), a[1].as_f64().unwrap_or(f64::NAN));
    if !x.is_finite() || !y.is_finite() { return Err("point coordinates must be finite".into()) }
    Ok(json!([round3(x), round3(y)]))
}
fn norm_ring(v: &Value, min_len: usize) -> Result<Vec<Value>, String> {
    let arr = v.as_array().ok_or_else(|| "points must be an array".to_string())?;
    if arr.len() < min_len { return Err(format!("need at least {min_len} points")) }
    arr.iter().map(norm_pt).collect()
}
fn is_flat_outline(v: &Value) -> bool {
    v.as_array()
        .filter(|a| !a.is_empty())
        .and_then(|a| a[0].as_array())
        .and_then(|p| p.first())
        .and_then(Value::as_f64)
        .is_some()
}
pub(super) fn norm_outline(v: &Value) -> Result<Value, String> {
    let arr = v.as_array().filter(|a| !a.is_empty()).ok_or_else(|| "outline must be a polygon".to_string())?;
    if is_flat_outline(v) {
        Ok(Value::Array(vec![Value::Array(norm_ring(v, 3)?)]))
    } else {
        let mut rings = Vec::with_capacity(arr.len());
        for ring in arr.iter() {
            rings.push(Value::Array(norm_ring(ring, 3)?));
        }
        Ok(Value::Array(rings))
    }
}

static PLAN_NOTE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone)]
struct NormOp {
    wire: Value,
    preserve_holes: bool,
}

fn next_note_id() -> String {
    let seq = PLAN_NOTE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("note-{}-{seq}", crate::hub::now_ms())
}

fn normalize_person_ops_checked(ops: &[Value]) -> Result<Vec<NormOp>, String> {
    if ops.is_empty() { return Err("ops cannot be empty".into()) }
    let mut out = Vec::with_capacity(ops.len());
    for op in ops.iter() {
        let kind = op["op"].as_str().unwrap_or("");
        match kind {
            "note" => {
                let text = op["text"].as_str().unwrap_or("").trim();
                if text.is_empty() { return Err("note text cannot be empty".into()) }
                let id = op["id"].as_str().filter(|s| !s.is_empty()).map(str::to_string).unwrap_or_else(next_note_id);
                let mut item = json!({"id": id, "text": text, "author": "person"});
                if let Some(t) = op.get("target").filter(|v| !v.is_null()) { item["target"] = t.clone(); }
                out.push(NormOp { wire: json!({"op": "add", "kind": "note", "item": item}), preserve_holes: false });
            }
            "add" => {
                let k = op["kind"].as_str().unwrap_or("");
                if k.is_empty() || !op["item"].is_object() { return Err("add needs kind and item".into()) }
                let mut item = op["item"].clone();
                if k == "note" {
                    if item["text"].as_str().unwrap_or("").trim().is_empty() { return Err("note text cannot be empty".into()) }
                    if item["id"].as_str().unwrap_or("").is_empty() { item["id"] = json!(next_note_id()); }
                    item["author"] = json!("person");
                }
                out.push(NormOp { wire: json!({"op": "add", "kind": k, "item": item}), preserve_holes: false });
            }
            "set" => {
                let id = op["id"].as_str().unwrap_or("");
                if id.is_empty() { return Err("set op needs id".into()) }
                let src = op["fields"].as_object().or_else(|| op.as_object()).ok_or_else(|| "set op needs fields".to_string())?;
                let mut fields = serde_json::Map::new();
                let mut preserve_holes = false;
                for k in ["path", "width", "outline", "layer", "locked", "vias", "reference", "status"] {
                    let Some(v) = src.get(k) else { continue };
                    let clean = match k {
                        "path" => Value::Array(norm_ring(v, 2)?),
                        "width" => {
                            let w = v.as_f64().unwrap_or(f64::NAN);
                            if !w.is_finite() || !(0.1..=50.0).contains(&w) { return Err("width must be between 0.1 and 50.0 mm".into()) }
                            json!(round3(w))
                        }
                        "outline" => {
                            preserve_holes = is_flat_outline(v);
                            norm_outline(v)?
                        }
                        "layer" | "reference" | "status" => {
                            let s = v.as_str().map(str::trim).unwrap_or("");
                            if s.is_empty() && k == "layer" { return Err("layer cannot be empty".into()) }
                            json!(s)
                        }
                        "locked" => json!(v.as_bool().ok_or_else(|| "locked must be a boolean".to_string())?),
                        "vias" => Value::Array(v.as_array().ok_or_else(|| "vias must be an array".to_string())?.iter().map(norm_pt).collect::<Result<Vec<_>, _>>()?),
                        _ => v.clone(),
                    };
                    fields.insert(k.to_string(), clean);
                }
                if fields.is_empty() { return Err(format!("set op for {id} has no supported fields")) }
                out.push(NormOp { wire: json!({"op": "set", "id": id, "fields": Value::Object(fields)}), preserve_holes });
            }
            "status" | "delete" => {
                let id = op["id"].as_str().unwrap_or("");
                if id.is_empty() { return Err(format!("{kind} op needs id")) }
                out.push(NormOp { wire: op.clone(), preserve_holes: false });
            }
            other => return Err(format!("unsupported plan op '{other}'")),
        }
    }
    Ok(out)
}

/// Validate and normalize person steering ops into KiCad `UpdatePlan` JSON ops (`set` uses `"fields"`, `"note"` becomes `add` note).
pub fn normalize_person_ops(ops: &[Value]) -> Result<Vec<Value>, String> {
    Ok(normalize_person_ops_checked(ops)?.into_iter().map(|n| n.wire).collect())
}

fn ensure_layer_entry(plan: &mut Value, layer: &str, role: &str) {
    if layer.is_empty() { return }
    let arr = plan.as_object_mut().and_then(|m| {
        if !m.contains_key("layers") || !m["layers"].is_array() { m.insert("layers".into(), json!([])); }
        m.get_mut("layers").and_then(Value::as_array_mut)
    });
    if let Some(layers) = arr {
        if !layers.iter().any(|l| l["layer"] == layer) {
            layers.push(json!({"layer": layer, "role": role, "status": "proposed"}));
        }
    }
}

/// Apply normalized person ops directly to a saved `<board>.kicad_plan` JSON document.
pub fn apply_person_ops(plan: &mut Value, ops: &[Value]) -> Result<Vec<String>, String> {
    let wrapped: Vec<NormOp> = ops.iter().map(|op| {
        let mut wire = op.clone();
        let preserve_holes = if wire["op"] == "set" {
            let raw_o = wire["fields"].get("outline").or_else(|| wire.get("outline")).cloned();
            if let Some(ov) = raw_o {
                let flat = is_flat_outline(&ov);
                if flat {
                    let norm = norm_outline(&ov)?;
                    if let Some(flds) = wire.get_mut("fields").and_then(Value::as_object_mut) {
                        flds.insert("outline".into(), norm);
                    } else if let Some(obj) = wire.as_object_mut() {
                        obj.insert("outline".into(), norm);
                    }
                }
                flat
            } else {
                false
            }
        } else {
            false
        };
        Ok(NormOp { wire, preserve_holes })
    }).collect::<Result<Vec<_>, String>>()?;
    apply_norm_ops(plan, &wrapped)
}

fn apply_norm_ops(plan: &mut Value, ops: &[NormOp]) -> Result<Vec<String>, String> {
    if !plan.is_object() { return Err("invalid routing plan JSON".into()) }
    let mut touched = Vec::new();
    for nop in ops {
        let op = &nop.wire;
        match op["op"].as_str().unwrap_or("") {
            "set" => {
                let id = op["id"].as_str().unwrap_or("");
                let fields = op["fields"].as_object().or_else(|| op.as_object()).ok_or_else(|| "set op needs fields".to_string())?;
                let mut new_layer = String::new();
                let role = if let Some(it) = plan["corridors"].as_array_mut().and_then(|a| a.iter_mut().find(|x| x["id"] == id)) {
                    if let Some(obj) = it.as_object_mut() {
                        for (k, v) in fields {
                            if matches!(k.as_str(), "op" | "id" | "fields") { continue }
                            if k == "layer" { if let Some(s) = v.as_str() { new_layer = s.to_string(); } }
                            obj.insert(k.clone(), v.clone());
                        }
                        obj.insert("edited".into(), json!(true));
                        obj.insert("author".into(), json!("person"));
                    }
                    "signal"
                } else if let Some(it) = plan["planes"].as_array_mut().and_then(|a| a.iter_mut().find(|x| x["id"] == id)) {
                    if let Some(obj) = it.as_object_mut() {
                        for (k, v) in fields {
                            if matches!(k.as_str(), "op" | "id" | "fields") { continue }
                            if k == "layer" { if let Some(s) = v.as_str() { new_layer = s.to_string(); } }
                            if k == "outline" && nop.preserve_holes {
                                if let (Some(new_rings), Some(old_raw)) = (v.as_array(), obj.get("outline")) {
                                    if new_rings.len() == 1 {
                                        if let Ok(Value::Array(old_rings)) = norm_outline(old_raw) {
                                            if old_rings.len() > 1 {
                                                let mut merged = Vec::with_capacity(old_rings.len());
                                                merged.push(new_rings[0].clone());
                                                merged.extend(old_rings[1..].iter().cloned());
                                                obj.insert(k.clone(), Value::Array(merged));
                                                continue;
                                            }
                                        }
                                    }
                                }
                            }
                            obj.insert(k.clone(), v.clone());
                        }
                        obj.insert("edited".into(), json!(true));
                        obj.insert("author".into(), json!("person"));
                    }
                    "power"
                } else {
                    return Err(format!("no such item: {id}"))
                };
                if !new_layer.is_empty() { ensure_layer_entry(plan, &new_layer, role); }
                touched.push(id.to_string());
            }
            "add" => {
                let kind = op["kind"].as_str().unwrap_or("");
                let arr_k = match kind { "note" => "notes", "constraint" => "constraints", "corridor" => "corridors", "plane" => "planes", _ => return Err(format!("unsupported add kind '{kind}'")) };
                let Some(obj) = plan.as_object_mut() else { return Err("invalid plan".into()) };
                if !obj.get(arr_k).is_some_and(Value::is_array) { obj.insert(arr_k.into(), json!([])); }
                obj.get_mut(arr_k).and_then(Value::as_array_mut).unwrap().push(op["item"].clone());
                if let Some(id) = op["item"]["id"].as_str() { touched.push(id.to_string()); }
            }
            "status" => {
                let id = op["id"].as_str().unwrap_or("");
                let mut ok = false;
                for arr_k in ["corridors", "planes"] {
                    if let Some(it) = plan[arr_k].as_array_mut().and_then(|a| a.iter_mut().find(|x| x["id"] == id)) {
                        it["status"] = op["status"].clone();
                        it["author"] = json!("person");
                        ok = true;
                        break;
                    }
                }
                if !ok { return Err(format!("no such item: {id}")) }
                touched.push(id.to_string());
            }
            "delete" => {
                let id = op["id"].as_str().unwrap_or("");
                let mut removed = false;
                for arr_k in ["corridors", "planes", "notes", "constraints"] {
                    if let Some(arr) = plan[arr_k].as_array_mut() {
                        let before = arr.len();
                        arr.retain(|x| x["id"] != id);
                        if arr.len() < before { removed = true; }
                    }
                }
                if !removed { return Err(format!("no such item: {id}")) }
                touched.push(id.to_string());
            }
            other => return Err(format!("unsupported plan op '{other}'")),
        }
    }
    plan["revision"] = json!(plan["revision"].as_u64().unwrap_or(0) + 1);
    Ok(touched)
}

/// Apply person approval/rejection to a saved `<board>.kicad_plan` JSON document.
pub fn apply_person_approve(plan: &mut Value, layer: &str, ids: &[String], yes: bool) -> Vec<String> {
    let target = if yes { "approved" } else { "rejected" };
    let mut changed = Vec::new();
    let keep_done = |v: &Value| matches!(v["status"].as_str(), Some("routed" | "verified"));
    if !layer.is_empty() && ids.is_empty() {
        if let Some(layers) = plan["layers"].as_array_mut() {
            if let Some(lv) = layers.iter_mut().find(|l| l["layer"] == layer) {
                lv["status"] = json!(target);
            } else {
                layers.push(json!({"layer": layer, "status": target}));
            }
        }
        for k in ["corridors", "planes"] {
            for it in plan[k].as_array_mut().into_iter().flatten() {
                if it["layer"] == layer && !keep_done(it) {
                    it["status"] = json!(target);
                    if let Some(id) = it["id"].as_str() { changed.push(id.to_string()); }
                }
            }
        }
    } else if !ids.is_empty() {
        for k in ["corridors", "planes"] {
            for it in plan[k].as_array_mut().into_iter().flatten() {
                let matched = it["id"].as_str().filter(|id| ids.iter().any(|x| x == id)).map(str::to_string);
                if let Some(id) = matched {
                    if !keep_done(it) {
                        it["status"] = json!(target);
                        changed.push(id);
                    }
                }
            }
        }
    } else {
        for lv in plan["layers"].as_array_mut().into_iter().flatten() {
            lv["status"] = json!(target);
        }
        for k in ["corridors", "planes"] {
            for it in plan[k].as_array_mut().into_iter().flatten() {
                if !keep_done(it) {
                    it["status"] = json!(target);
                    if let Some(id) = it["id"].as_str() { changed.push(id.to_string()); }
                }
            }
        }
    }
    plan["revision"] = json!(plan["revision"].as_u64().unwrap_or(0) + 1);
    changed
}

static PLAN_FILE_LOCK: Mutex<()> = Mutex::new(());
static PLAN_TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn write_plan_file(pcb: &str, plan: &Value) -> Result<(), String> {
    let f = Path::new(pcb).with_extension("kicad_plan");
    if let Ok(md) = std::fs::symlink_metadata(&f) {
        if md.file_type().is_symlink() {
            return Err("refusing to write routing plan through a symlink".into());
        }
    }
    let seq = PLAN_TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = f.with_extension(format!("kicad_plan.{}.{seq}.tmp", std::process::id()));
    let txt = serde_json::to_string_pretty(plan).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::write(&tmp, txt) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    if let Err(e) = crate::plat::fs::rename(&tmp, &f) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    Ok(())
}

fn update_saved_plan_sync(pcb: &str, base: Option<u64>, ops: &[NormOp]) -> Result<(Value, Vec<String>), String> {
    let f = Path::new(pcb).with_extension("kicad_plan");
    let _guard = PLAN_FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(md) = std::fs::symlink_metadata(&f) {
        if md.file_type().is_symlink() {
            return Err("refusing to access routing plan through a symlink".into());
        }
    }
    let raw = std::fs::read_to_string(&f).map_err(|_| "no saved routing plan for this board".to_string())?;
    let mut plan: Value = serde_json::from_str(&raw).map_err(|_| "saved routing plan is not valid JSON".to_string())?;
    if let Some(want) = base {
        let cur = plan["revision"].as_u64().unwrap_or(0);
        if cur != want {
            return Err(format!("routing plan changed (expected revision {want}, current is {cur}); refresh and try again"));
        }
    }
    let touched = apply_norm_ops(&mut plan, ops)?;
    write_plan_file(pcb, &plan)?;
    Ok((plan["revision"].clone(), touched))
}

fn merge_preserved_holes_into_ops(plan: &Value, ops: &[NormOp]) -> Vec<Value> {
    let mut plane_rings: HashMap<String, Vec<Value>> = HashMap::new();
    for p in plan["planes"].as_array().into_iter().flatten() {
        if let Some(id) = p["id"].as_str() {
            if let Ok(Value::Array(rs)) = norm_outline(&p["outline"]) {
                plane_rings.insert(id.to_string(), rs);
            }
        }
    }
    ops.iter().map(|nop| {
        let kind = nop.wire["op"].as_str().unwrap_or("");
        match kind {
            "add" if nop.wire["kind"] == "plane" => {
                if let Some(id) = nop.wire["item"]["id"].as_str() {
                    if let Ok(Value::Array(rs)) = norm_outline(&nop.wire["item"]["outline"]) {
                        plane_rings.insert(id.to_string(), rs);
                    }
                }
                nop.wire.clone()
            }
            "delete" => {
                if let Some(id) = nop.wire["id"].as_str() {
                    plane_rings.remove(id);
                }
                nop.wire.clone()
            }
            "set" => {
                let id = nop.wire["id"].as_str().unwrap_or("");
                if nop.wire["fields"].get("outline").is_none() {
                    return nop.wire.clone();
                }
                let mut wire = nop.wire.clone();
                let new_rings = wire["fields"]["outline"].as_array().cloned().unwrap_or_default();
                if nop.preserve_holes && new_rings.len() == 1 {
                    if let Some(old_rings) = plane_rings.get(id) {
                        if old_rings.len() > 1 {
                            let mut merged = Vec::with_capacity(old_rings.len());
                            merged.push(new_rings[0].clone());
                            merged.extend(old_rings[1..].iter().cloned());
                            plane_rings.insert(id.to_string(), merged.clone());
                            if let Some(flds) = wire["fields"].as_object_mut() {
                                flds.insert("outline".into(), Value::Array(merged));
                            }
                            return wire;
                        }
                    }
                }
                plane_rings.insert(id.to_string(), new_rings);
                wire
            }
            _ => nop.wire.clone(),
        }
    }).collect()
}

/// The person's steering edits (the Plan canvas/inspector): sends KiCad's `UpdatePlan` as the person when open,
/// else updates `<board>.kicad_plan` atomically on disk and broadcasts the settled panel update.
pub async fn update(hub: &Arc<Hub>, pcb: &str, base: Option<u64>, ops: &[Value]) -> Result<Value, String> {
    let norm_ops = normalize_person_ops_checked(ops)?;
    let pro = crate::files::kicad::project::read(&Path::new(pcb).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()).pro;
    if let Some(r) = crate::files::kicad::embed::gui_route(&pro) {
        let needs_holes = norm_ops.iter().any(|n| n.preserve_holes);
        let live_plan = if needs_holes {
            Some(read_plan(hub, None, pcb, false).await?)
        } else if base.is_none() {
            read_plan(hub, None, pcb, false).await.ok()
        } else {
            None
        };
        let rev = match base {
            Some(b) => b,
            None => live_plan.as_ref().and_then(|p| p["revision"].as_u64()).unwrap_or(0),
        };
        let wire_ops: Vec<Value> = if let Some(lp) = live_plan.as_ref().filter(|_| needs_holes) {
            merge_preserved_holes_into_ops(lp, &norm_ops)
        } else {
            norm_ops.iter().map(|n| n.wire.clone()).collect()
        };
        let msg = json!({"board": spec(pcb), "base": rev, "ops": Value::Array(wire_ops).to_string()});
        let o = crate::files::kicad::embed::gui_call_as(&r, &format!("{NS}UpdatePlan"), msg, &crate::files::kicad::coedit::client_name(&crate::files::kicad::coedit::Who::Person)).await;
        if o["ok"].as_bool() != Some(true) {
            return Err(refused(o["error"].as_str().unwrap_or("no answer")));
        }
        let m = &o["message"];
        if m["refused"].as_array().is_some_and(|a| !a.is_empty()) {
            let why = m["reasons"].as_array().map(|rs| rs.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("; ")).filter(|s| !s.is_empty()).unwrap_or_else(|| "KiCad refused plan update".into());
            return Err(why);
        }
        events::settle(hub, pcb).await;
        return Ok(m.clone());
    }
    let pcb_s = pcb.to_string();
    let res = tokio::task::spawn_blocking(move || update_saved_plan_sync(&pcb_s, base, &norm_ops)).await.map_err(|e| e.to_string())??;
    events::settle(hub, pcb).await;
    Ok(json!({"revision": res.0, "applied": res.1, "saved": true}))
}

/// The person's decision (the Plan panel, a phone): `plan.approve {pro|path, layer?, ids?, approve}` -> KiCad's ApprovePlan as the person.
pub async fn approve(hub: &Arc<Hub>, pcb: &str, layer: &str, ids: &[String], yes: bool) -> Result<Value, String> {
    let msg = json!({"board": spec(pcb), "layer": layer, "ids": ids, "approve": yes});
    let pro = crate::files::kicad::project::read(&Path::new(pcb).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()).pro;
    let o = match crate::files::kicad::embed::gui_route(&pro) {
        Some(r) => { let o = crate::files::kicad::embed::gui_call_as(&r, &format!("{NS}ApprovePlan"), msg, &crate::files::kicad::coedit::client_name(&crate::files::kicad::coedit::Who::Person)).await; if o["ok"].as_bool().unwrap_or(false) { o["message"].clone() } else { return Err(refused(o["error"].as_str().unwrap_or("no answer"))) } }
        None => {
            let f = Path::new(pcb).with_extension("kicad_plan");
            if f.exists() {
                let (pcb_s, layer_s, ids_v) = (pcb.to_string(), layer.to_string(), ids.to_vec());
                let res = tokio::task::spawn_blocking(move || -> Result<Value, String> {
                    let _guard = PLAN_FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                    let raw = std::fs::read_to_string(&f).map_err(|e| e.to_string())?;
                    let mut plan: Value = serde_json::from_str(&raw).map_err(|_| "saved routing plan is not valid JSON".to_string())?;
                    let changed = apply_person_approve(&mut plan, &layer_s, &ids_v, yes);
                    write_plan_file(&pcb_s, &plan)?;
                    Ok(json!({"revision": plan["revision"], "changed": changed, "saved": true}))
                }).await.map_err(|e| e.to_string())??;
                events::settle(hub, pcb).await;
                return Ok(res);
            }
            call(hub, None, pcb, "ApprovePlan", msg).await?
        }
    };
    events::settle(hub, pcb).await;
    // KiCad said yes: the plan must say so too (a runtime that accepts and records nothing, as bp.12.1 did, is told, never a silent success)
    if let Ok(p) = read_plan(hub, None, pcb, false).await {
        let left = not_recorded(&p, layer, ids);
        if !left.is_empty() { return Err(format!("KiCad accepted the {} but the plan did not record it: {} still awaiting approval. Update KiCad.", if yes { "approval" } else { "rejection" }, left.join(", "))) }
    }
    Ok(o)
}
/// RoutePlan refused AS_BUSY: nothing was routed. Before bp.13 batch 11 KiCad's plan view is busy to the API for as long as it is on
/// (from batch 11 only while the person drags a plan item), and a dialog or a running tool does the same: said so, never as routed.
pub fn route_busy(err: &str) -> String {
    if !err.contains("AS_BUSY") { return err.to_string() }
    format!("Nothing was routed: KiCad is busy ({}). Its plan view, a dialog or a tool the person is using holds it; plan view is the person's to leave (Ctrl+Shift+R). End your turn and call plan_route again when they are done; do not report this step as routed.", err.split("AS_BUSY").nth(1).unwrap_or("").trim_start_matches(':').trim())
}
/// KiCad's refusal of the person's ApprovePlan, said in the Plan panel: a dialog KiCad shows (AS_BUSY) is named with the way out.
pub fn refused(err: &str) -> String {
    match err.split('\'').nth(1).filter(|_| err.contains("showing the dialog")) {
        Some(d) => format!("KiCad is showing the dialog '{d}': close it in KiCad, then approve again."),
        None => format!("KiCad refused: {err}"),
    }
}
pub struct Rpc;
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["plan.approve", "plan.update", "plan.stop", "plan.verify", "plan.get", "plan.focus", "plan.enable"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let path = if !sv(p, "pro").is_empty() { sv(p, "pro") } else { sv(p, "path") };
            if m == "plan.enable" {
                // the person's switch: kept as a setting, the Plan panel of the project shown follows at once
                let v = if p["on"] == true { "on" } else { "off" };
                let r = hub.commit(&json!({"$": "SettingSet", "key": SETTING, "value": v, "at": crate::agents::now_s()})).await;
                if !sv(p, "pro").is_empty() { panel::publish(hub, sv(p, "pro")).await }
                return vec![match r { Ok(_) => json!({"t": "reply", "id": id, "ok": true, "result": {"on": v == "on"}}), Err(e) => json!({"t": "reply", "id": id, "ok": false, "error": e.to_string()}) }]
            }
            let r = async {
                if crate::copy::on() && m != "plan.get" { return Err(crate::copy::HOME_ONLY.to_string()) }
                let pcb = board_of(hub, "", path).await?;
                let ids: Vec<String> = p["ids"].as_array().map(|x| x.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                match m {
                    "plan.approve" => approve(hub, &pcb, sv(p, "layer"), &ids, p["approve"] != false).await,
                    "plan.update" => {
                        let ops = p["ops"].as_array().cloned().ok_or_else(|| "ops must be a JSON array".to_string())?;
                        update(hub, &pcb, p["base"].as_u64(), &ops).await
                    }
                    "plan.focus" => panel::focus(hub, if sv(p, "pro").is_empty() { sv(p, "path") } else { sv(p, "pro") }, sv(p, "id")).await,
                    "plan.stop" => { let r = call(hub, None, &pcb, "StopPlanRouting", json!({"board": spec(&pcb)})).await; events::settle(hub, &pcb).await; r }
                    "plan.verify" => { let r = call(hub, None, &pcb, "VerifyPlan", json!({"board": spec(&pcb)})).await; events::settle(hub, &pcb).await; r }
                    _ if p["board"] == true || p["panel"] == true => {
                        let (plan, live) = panel::plan_now(hub, &pcb).await;
                        let pan = if plan.is_null() { Value::Null } else { panel::data_with_board(&plan, &events::open_asks(&pcb), live, Some(&pcb)) };
                        Ok(json!({"plan": plan, "panel": pan, "pro": path}))
                    }
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
    crate::files::kicad::mgmt::register(Arc::new(panel::PlanPanel)); // the Plan tab beside the design
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> Value { json!({"revision": 3, "layers": [{"layer": "In1.Cu", "status": "proposed"}, {"layer": "In2.Cu", "status": "rejected"}],
        "groups": [{"id": "g1", "nets": ["A", "B", "C"], "status": "proposed"}, {"id": "g2", "nets": ["D"], "status": "rejected"}],
        "corridors": [{"id": "c1", "group": "g1", "layer": "In1.Cu", "status": "approved"}, {"id": "c2", "group": "g1", "layer": "In1.Cu", "status": "proposed"},
            {"id": "c3", "group": "g2", "layer": "In1.Cu", "status": "approved"}, {"id": "c4", "group": "g1", "layer": "In2.Cu", "status": "approved"}],
        "planes": [{"id": "p1", "net": "GND", "layer": "In1.Cu", "status": "approved"}, {"id": "p2", "net": "+3V3", "layer": "In1.Cu", "status": "proposed"}]}) }
    #[test] fn plan_route_refused_busy_is_never_routed() {
        let m = route_busy("KiCad answered AS_BUSY: KiCad is busy and cannot respond to API requests right now");
        assert!(m.starts_with("Nothing was routed") && m.contains("cannot respond") && m.contains("do not report"), "{m}");
        assert_eq!(route_busy("no plan"), "no plan");
    }
    #[test] fn plan_needs_a_plan_on_big_boards() { assert!(needs_plan(4, 10)); assert!(needs_plan(2, 61)); assert!(!needs_plan(2, 60)); assert!(guidance().contains("plan_approve") && guidance().contains("fab_profile")); }
    #[test] fn a_plan_review_is_asked_with_a_link_to_its_board() {
        assert_eq!(review_link("/p", "/p/Nodes/A b/A.kicad_pcb"), "backplane://plan/Nodes/A%20b/A.kicad_pcb");
        assert_eq!(review_link("/p/", "/p/a.kicad_pcb"), "backplane://plan/a.kicad_pcb");
        assert!(crate::md::refs::parse(&review_link("/p", "/p/Nodes/A b/A.kicad_pcb")).is_some(), "a link the markdown keeps");
    }
    #[test] fn plan_before_routing_is_off_unless_turned_on() {
        assert!(!is_on("") && !is_on("off") && is_on("on"));
        assert!(guidance_off().contains("only when the person asks") && !guidance_off().contains("plan_approve"));
        assert!(guidance().contains("turned on Plan before routing"));
    }
    #[test] fn plan_api_told_by_capability_then_version() {
        let caps = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(plans("ok", "10.99.0-unknown-bp.13", &caps(&["embed", "routing-plan"])));
        assert!(!plans("ok", "10.99.0-bp.13", &caps(&["embed"])), "a manifest that lists capabilities is believed");
        assert!(plans("ok", "10.99.0-unknown-bp.13", &[])); assert!(plans("ok", "10.99.0-bp.11", &[])); assert!(!plans("ok", "10.99.0-bp.10", &[]));
        // test builds and describe names by the shared reader: bp.12-<hash> is bp.12, the describe name's last "bp." part is the build's
        assert!(plans("ok", "10.99.0-bp.12-851cbefda6", &[]) && plans("ok", "backplane-kicad-v10.99.0-bp.7-15-g8d903542-dirty-bp.13", &[]) && plans("ok", "10.99.0-bp.12.1", &[]));
        assert!(!plans("ok", "10.99.0-bp.10-851cbefda6", &[]) && !plans("ok", "10.0.6", &[]) && !plans("broken", "10.99.0-bp.13", &[]));
        assert!(!plans("ok", "10.99.0-bp.12-851cbefda6", &caps(&["embed"])), "capabilities first, even on a test build");
        assert!(!plans("stock", "10.0.7", &caps(&["routing-plan"])));
    }
    #[test] fn plan_keys_note_where_kicads_own_keys_are_not_the_plans() {
        let c = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(keys_note(&c(&["api-server", "routing-plan"])), Some(KEYS_NOTE), "bp.12.1");
        assert_eq!(keys_note(&c(&["routing-plan", "plan-keys"])), None, "bp.12.2, bp.13 batch 6");
        assert_eq!(keys_note(&c(&["api-server"])), None, "no plan: nothing to say");
        assert_eq!(refused("KiCad is showing the dialog 'Footprint Chooser (15451 items loaded)'; only reads and item edits are served until it is closed"),
            "KiCad is showing the dialog 'Footprint Chooser (15451 items loaded)': close it in KiCad, then approve again.");
        assert_eq!(refused("no answer"), "KiCad refused: no answer");
    }
    #[test] fn plan_approval_not_recorded_is_told() {
        let p = json!({"layers": [{"layer": "F.Cu", "status": "proposed"}, {"layer": "B.Cu", "status": "approved"}],
            "corridors": [{"id": "c-p", "status": "proposed"}, {"id": "c-q", "status": "approved"}, {"id": "c-r", "status": "edited"}], "planes": [{"id": "p-gnd", "status": "rejected"}]});
        assert_eq!(not_recorded(&p, "", &["c-p".into(), "c-q".into()]), ["c-p"]);
        assert_eq!(not_recorded(&p, "", &[]), ["c-p", "c-r"], "all: every item still awaiting");
        assert_eq!(not_recorded(&p, "F.Cu", &[]), ["F.Cu"]);
        assert!(not_recorded(&p, "B.Cu", &[]).is_empty());
        assert!(not_recorded(&p, "", &["p-gnd".into()]).is_empty(), "a rejection recorded");
    }
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
    #[test] fn person_ops_normalize_apply_approve_and_reedit_diff() {
        let mut p = json!({
            "revision": 3,
            "layers": [{"layer": "In1.Cu", "role": "signal", "status": "proposed"}, {"layer": "In2.Cu", "role": "power", "status": "proposed"}],
            "groups": [{"id": "g1", "nets": ["A", "B"], "status": "proposed"}],
            "corridors": [{"id": "c1", "group": "g1", "layer": "In1.Cu", "path": [[10.0, 20.0], [30.0, 20.0]], "width": 2.0, "status": "proposed"}],
            "planes": [{"id": "p1", "net": "GND", "layer": "In2.Cu", "outline": [[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]], "status": "proposed"}]
        });
        let ops = normalize_person_ops(&[
            json!({"op": "set", "id": "c1", "path": [[10.0, 20.0], [20.0, 25.0], [30.0, 20.0]], "width": 2.5, "locked": true, "layer": "In3.Cu"}),
            json!({"op": "set", "id": "p1", "fields": {"outline": [[0.0, 0.0], [15.0, 0.0], [15.0, 10.0]]}}),
            json!({"op": "note", "target": "c1", "text": "Steer clear of U2"})
        ]).unwrap();
        assert!(ops[0]["fields"].is_object());
        assert_eq!(ops[1]["fields"]["outline"], json!([[[0.0, 0.0], [15.0, 0.0], [15.0, 10.0]]]));
        assert_eq!(ops[2]["op"], "add");

        let before = p.clone();
        let touched = apply_person_ops(&mut p, &ops).unwrap();
        assert_eq!(touched.len(), 3);
        assert_eq!(p["revision"], 4);
        assert_eq!(p["corridors"][0]["edited"], true);
        assert_eq!(p["corridors"][0]["author"], "person");
        assert_eq!(p["corridors"][0]["locked"], true);
        assert_eq!(p["corridors"][0]["layer"], "In3.Cu");
        assert_eq!(p["notes"][0]["text"], "Steer clear of U2");
        assert!(p["layers"].as_array().unwrap().iter().any(|l| l["layer"] == "In3.Cu"));

        let d1 = diff(&before, &p);
        assert!(d1.iter().any(|x| x["id"] == "c1" && x["change"] == "edited by the person"));
        assert!(d1.iter().any(|x| x["id"] == "c1" && x["change"] == "locked"));
        assert!(d1.iter().any(|x| x["change"] == "note"));

        // Further geometry edit on an already-edited item is still surfaced by diff
        let mid = p.clone();
        let ops2 = normalize_person_ops(&[json!({"op": "set", "id": "c1", "width": 3.2})]).unwrap();
        apply_person_ops(&mut p, &ops2).unwrap();
        let d2 = diff(&mid, &p);
        assert!(d2.iter().any(|x| x["id"] == "c1" && x["change"] == "edited by the person"));

        let approved = apply_person_approve(&mut p, "In3.Cu", &[], true);
        assert_eq!(approved, ["c1"]);
        assert_eq!(p["corridors"][0]["status"], "approved");
        assert!(normalize_person_ops(&[json!({"op": "set", "id": "c1", "width": 0.01})]).is_err());
        assert!(normalize_person_ops(&[json!({"op": "set", "id": "c1", "path": [[0.0, 0.0]]})]).is_err());
        assert!(crate::api::RpcHandler::methods(&Rpc).contains(&"plan.update"));
    }

    #[test] fn plane_outline_edit_preserves_existing_holes() {
        let hole = json!([[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]]);
        let mut p = json!({
            "revision": 1,
            "layers": [{"layer": "In2.Cu", "role": "power", "status": "proposed"}],
            "planes": [{"id": "p1", "net": "GND", "layer": "In2.Cu", "outline": [[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]], hole], "status": "proposed"}]
        });
        // Flat outline -> edits outer ring only, preserves existing holes
        let flat_ops = normalize_person_ops_checked(&[
            json!({"op": "set", "id": "p1", "outline": [[0.0, 0.0], [12.0, 0.0], [12.0, 10.0], [0.0, 10.0]]})
        ]).unwrap();
        assert!(flat_ops[0].preserve_holes);
        apply_norm_ops(&mut p, &flat_ops).unwrap();
        let rings = p["planes"][0]["outline"].as_array().unwrap();
        assert_eq!(rings.len(), 2);
        assert_eq!(rings[0], json!([[0.0, 0.0], [12.0, 0.0], [12.0, 10.0], [0.0, 10.0]]));
        assert_eq!(rings[1], hole);

        // Explicit 1-ring array -> replaces all rings and clears existing holes
        let clear_ops = normalize_person_ops_checked(&[
            json!({"op": "set", "id": "p1", "outline": [[[0.0, 0.0], [13.0, 0.0], [13.0, 10.0]]]})
        ]).unwrap();
        assert!(!clear_ops[0].preserve_holes);
        apply_norm_ops(&mut p, &clear_ops).unwrap();
        assert_eq!(p["planes"][0]["outline"], json!([[[0.0, 0.0], [13.0, 0.0], [13.0, 10.0]]]));

        // Flat stored outline must never treat vertices 1..N as holes when edited
        let mut p_flat = json!({
            "revision": 1,
            "layers": [{"layer": "In2.Cu", "role": "power", "status": "proposed"}],
            "planes": [{"id": "p1", "net": "GND", "layer": "In2.Cu", "outline": [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], "status": "proposed"}]
        });
        let flat_ops2 = normalize_person_ops_checked(&[
            json!({"op": "set", "id": "p1", "outline": [[0.0, 0.0], [15.0, 0.0], [15.0, 10.0]]})
        ]).unwrap();
        apply_norm_ops(&mut p_flat, &flat_ops2).unwrap();
        assert_eq!(p_flat["planes"][0]["outline"], json!([[[0.0, 0.0], [15.0, 0.0], [15.0, 10.0]]]));

        let new_hole = json!([[3.0, 3.0], [5.0, 3.0], [5.0, 5.0]]);
        let multi_ops = normalize_person_ops_checked(&[
            json!({"op": "set", "id": "p1", "outline": [[[0.0, 0.0], [14.0, 0.0], [14.0, 10.0]], new_hole]})
        ]).unwrap();
        apply_norm_ops(&mut p, &multi_ops).unwrap();
        let rings2 = p["planes"][0]["outline"].as_array().unwrap();
        assert_eq!(rings2.len(), 2);
        assert_eq!(rings2[1], new_hole);
    }

    #[test] fn saved_plan_update_checks_base_revision_and_unique_note_ids() {
        let dir = std::env::temp_dir().join(format!("bp-plan-mod-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pcb = dir.join("demo.kicad_pcb");
        let plan_f = dir.join("demo.kicad_plan");
        std::fs::write(&pcb, "").unwrap();
        std::fs::write(&plan_f, serde_json::to_string_pretty(&json!({
            "revision": 5,
            "corridors": [{"id": "c1", "group": "g1", "layer": "In1.Cu", "path": [[0.0, 0.0], [10.0, 0.0]], "width": 1.0, "status": "proposed"}]
        })).unwrap()).unwrap();

        let nops = normalize_person_ops_checked(&[
            json!({"op": "set", "id": "c1", "width": 2.0}),
            json!({"op": "note", "target": "c1", "text": "Note A"}),
            json!({"op": "note", "target": "c1", "text": "Note B"})
        ]).unwrap();
        assert_ne!(nops[1].wire["item"]["id"], nops[2].wire["item"]["id"]);
        assert_eq!(nops[1].wire["item"]["target"], "c1");

        let pcb_s = pcb.to_string_lossy().into_owned();
        // Stale base=4 is rejected without overwriting disk
        let err = update_saved_plan_sync(&pcb_s, Some(4), &nops).unwrap_err();
        assert!(err.contains("expected revision 4, current is 5"), "{err}");
        let disk_before: Value = serde_json::from_str(&std::fs::read_to_string(&plan_f).unwrap()).unwrap();
        assert_eq!(disk_before["revision"], 5);

        // Matching base=5 succeeds and increments revision
        let (rev, _) = update_saved_plan_sync(&pcb_s, Some(5), &nops).unwrap();
        assert_eq!(rev, 6);
        let disk_after: Value = serde_json::from_str(&std::fs::read_to_string(&plan_f).unwrap()).unwrap();
        assert_eq!(disk_after["revision"], 6);
        assert_eq!(disk_after["notes"].as_array().unwrap().len(), 2);
        assert_eq!(disk_after["notes"][0]["target"], "c1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test] fn live_merge_preserved_holes_keeps_authoritative_holes_and_allows_explicit_clear() {
        let hole1 = json!([[2.0, 2.0], [3.0, 2.0], [3.0, 3.0]]);
        let hole2 = json!([[6.0, 6.0], [7.0, 6.0], [7.0, 7.0]]);
        let live_plan = json!({
            "planes": [{"id": "p1", "outline": [[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], hole1, hole2]}]
        });
        let flat = normalize_person_ops_checked(&[json!({"op": "set", "id": "p1", "outline": [[0.0, 0.0], [12.0, 0.0], [12.0, 10.0]]})]).unwrap();
        let merged = merge_preserved_holes_into_ops(&live_plan, &flat);
        let rings = merged[0]["fields"]["outline"].as_array().unwrap();
        assert_eq!(rings.len(), 3);
        assert_eq!(rings[1], hole1);
        assert_eq!(rings[2], hole2);

        let clear = normalize_person_ops_checked(&[json!({"op": "set", "id": "p1", "outline": [[[0.0, 0.0], [12.0, 0.0], [12.0, 10.0]]]})]).unwrap();
        let cleared = merge_preserved_holes_into_ops(&live_plan, &clear);
        assert_eq!(cleared[0]["fields"]["outline"].as_array().unwrap().len(), 1);

        let batch = normalize_person_ops_checked(&[
            json!({"op": "set", "id": "p1", "outline": [[[0.0, 0.0], [12.0, 0.0], [12.0, 10.0]]]}),
            json!({"op": "set", "id": "p1", "outline": [[0.0, 0.0], [14.0, 0.0], [14.0, 10.0]]}),
        ]).unwrap();
        let batch_out = merge_preserved_holes_into_ops(&live_plan, &batch);
        assert_eq!(batch_out[0]["fields"]["outline"].as_array().unwrap().len(), 1);
        assert_eq!(batch_out[1]["fields"]["outline"].as_array().unwrap().len(), 1, "subsequent flat move must keep holes cleared");
    }
}

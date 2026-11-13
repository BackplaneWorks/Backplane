//! The plan as hub data and the person's side of it: an agent's request for approval (`ask`), the plan published after each change
//! (`changed`, info `plan.<digest>`: summary, items, the open requests), so the Plan panel, the desktop viewer and the phones draw the same.
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex, OnceLock}};

pub fn info_key(pcb: &str) -> String { format!("plan.{}", crate::files::kicad::kserve::digest(pcb)) }
/// The requests for approval open per board: (thread, what, note, at).
static ASKS: OnceLock<Mutex<HashMap<String, Vec<Value>>>> = OnceLock::new();
fn asks() -> &'static Mutex<HashMap<String, Vec<Value>>> { ASKS.get_or_init(Default::default) }
pub fn open_asks(pcb: &str) -> Vec<Value> { asks().lock().unwrap().get(pcb).cloned().unwrap_or_default() }

/// An agent asks the person to approve: kept (one per thread and thing), said in the thread's activity, published.
pub async fn ask(hub: &Arc<Hub>, thread: &str, pcb: &str, what: &str, note: &str) {
    let now = crate::agents::now_s();
    { let mut m = asks().lock().unwrap(); let v = m.entry(pcb.to_string()).or_default(); v.retain(|x| !(x["thread"] == thread && x["what"] == what)); v.push(json!({"thread": thread, "what": what, "note": note, "at": now})); }
    let _ = hub.commit(&json!({"$": "ActivityLogged", "thread": thread, "act": format!("plan-ask:{now}"), "tone": "info", "kind": "kicad", "summary": format!("Asked you to approve {what} in the routing plan{}", if note.is_empty() { String::new() } else { format!(": {note}") }), "at": now * 1000})).await;
    changed(hub, pcb).await;
}
/// The plan changed (or was asked about): read it and publish it; requests the person answered are dropped.
pub async fn changed(hub: &Arc<Hub>, pcb: &str) {
    let plan = super::read_plan(hub, None, pcb, true).await.unwrap_or(Value::Null);
    if !plan.is_null() {
        let waiting: Vec<String> = super::items(&plan).into_iter().filter(|i| i.status == "proposed").map(|i| i.id).collect();
        let mut m = asks().lock().unwrap(); if let Some(v) = m.get_mut(pcb) { if waiting.is_empty() { v.clear() } }
    }
    let v = json!({"board": pcb, "summary": if plan.is_null() { Value::Null } else { super::summary(&plan) }, "items": super::items(&plan).iter().map(item_json).collect::<Vec<_>>(), "asks": open_asks(pcb), "revision": plan["revision"]});
    let _ = hub.set_info(&info_key(pcb), &v.to_string()).await;
}
pub fn item_json(i: &super::Item) -> Value { json!({"id": i.id, "kind": i.kind, "layer": i.layer, "group": i.group, "status": i.status, "edited": i.edited, "locked": i.locked, "nets": i.nets, "progress": i.progress.map(|(r, t)| json!([r, t]))}) }
pub fn start(_hub: &Arc<Hub>) {}

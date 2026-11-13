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
    let plan = super::panel::plan_now(hub, pcb).await.0;
    let v = json!({"board": pcb, "summary": if plan.is_null() { Value::Null } else { super::summary(&plan) }, "items": super::items(&plan).iter().map(item_json).collect::<Vec<_>>(), "asks": open_asks(pcb), "revision": plan["revision"]});
    let _ = hub.set_info(&info_key(pcb), &v.to_string()).await;
    // the Plan panel follows (its project is the board's .kicad_pro)
    let pro = crate::files::kicad::project::read(&std::path::Path::new(pcb).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()).pro;
    if !pro.is_empty() { super::panel::publish(hub, &pro).await }
}
pub fn item_json(i: &super::Item) -> Value { json!({"id": i.id, "kind": i.kind, "layer": i.layer, "group": i.group, "status": i.status, "edited": i.edited, "locked": i.locked, "nets": i.nets, "progress": i.progress.map(|(r, t)| json!([r, t]))}) }
static HUB: OnceLock<std::sync::Weak<Hub>> = OnceLock::new();
pub fn start(hub: &Arc<Hub>) { let _ = HUB.set(Arc::downgrade(hub)); }
/// Boards with a plan change being taken in (several ET_PLAN_CHANGED of one drag become one).
static PENDING: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

/// KiCad's ET_PLAN_CHANGED (the open editor's plan changed: the person's edit, approval, an agent's update, routing progress): the plan
/// is published again (after 1.5 s of quiet), and each thread that asked for approval is woken once what it asked about was judged.
/// Answers whether the event was this one.
pub fn on_event(pro: &str, ev: &Value) -> bool {
    if !(ev["type"] == "ET_PLAN_CHANGED" || ev["type"].as_i64() == Some(22)) { return false }
    let Some(hub) = HUB.get().and_then(|w| w.upgrade()) else { return true };
    let pcb = crate::files::kicad::project::read(&std::path::Path::new(pro).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()).pcb;
    if pcb.is_empty() { return true }
    { let mut p = PENDING.get_or_init(Default::default).lock().unwrap(); if p.contains(&pcb) { return true } p.push(pcb.clone()); }
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        PENDING.get_or_init(Default::default).lock().unwrap().retain(|x| x != &pcb);
        settle(&hub, &pcb).await;
    });
    true
}
/// What an ask is about, judged in this plan: (decided, the words for it) — decided once none of its items (or its layer) is still proposed.
pub fn judged(plan: &Value, what: &str) -> Option<String> {
    let all = super::items(plan);
    let picked: Vec<&super::Item> = if let Some(l) = what.strip_prefix("layer ") { all.iter().filter(|i| i.layer == l).collect() }
        else if what == "the plan" { all.iter().collect() } else { let ids: Vec<&str> = what.split(", ").collect(); all.iter().filter(|i| ids.contains(&i.id.as_str())).collect() };
    if picked.is_empty() || picked.iter().any(|i| i.status == "proposed") { return None }
    let say = |s: &str| picked.iter().filter(|i| i.status == s).map(|i| i.id.as_str()).collect::<Vec<_>>();
    let (a, r) = (say("approved"), say("rejected"));
    Some(format!("The person judged {what} in the routing plan: {}{}. plan_get answers the changes (their edits, notes and constraints too); route only what is approved.",
        if a.is_empty() { "nothing approved".to_string() } else { format!("approved {}", a.join(", ")) }, if r.is_empty() { String::new() } else { format!("; rejected {}", r.join(", ")) }))
}
/// Publish the plan and wake the threads whose asks were judged (each told once, in its own words).
pub async fn settle(hub: &Arc<Hub>, pcb: &str) {
    let plan = super::panel::plan_now(hub, pcb).await.0;
    let mut tell: Vec<(String, String)> = vec![];
    if !plan.is_null() {
        let mut m = asks().lock().unwrap();
        if let Some(v) = m.get_mut(pcb) { v.retain(|a| match judged(&plan, a["what"].as_str().unwrap_or("")) { Some(words) => { tell.push((a["thread"].as_str().unwrap_or("").to_string(), words)); false } None => true }) }
    }
    changed(hub, pcb).await;
    let Some(ag) = crate::agents::handle() else { return };
    for (th, words) in tell {
        if th.is_empty() { continue }
        let msg = format!("t:{th}:plan-{}", crate::hub::now_ms());
        let _ = ag.deliver_as("user", &th, &msg, &words, Some((th.clone(), "plan".into(), pcb.to_string())), vec![]).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn plan_events_an_ask_is_answered_once_its_items_are_judged() {
        let p = json!({"layers": [{"layer": "In1.Cu", "status": "approved"}], "corridors": [{"id": "c1", "layer": "In1.Cu", "status": "approved"}, {"id": "c2", "layer": "In1.Cu", "status": "rejected"}, {"id": "c3", "layer": "In2.Cu", "status": "proposed"}]});
        assert_eq!(judged(&p, "c1, c2").unwrap(), "The person judged c1, c2 in the routing plan: approved c1; rejected c2. plan_get answers the changes (their edits, notes and constraints too); route only what is approved.");
        assert!(judged(&p, "layer In1.Cu").unwrap().contains("approved c1"));
        assert!(judged(&p, "c3").is_none(), "still proposed: keep waiting"); assert!(judged(&p, "the plan").is_none()); assert!(judged(&p, "zz").is_none());
        assert!(on_event("/x/a.kicad_pro", &json!({"type": "ET_PLAN_CHANGED"}))); assert!(on_event("/x/a.kicad_pro", &json!({"type": 22}))); assert!(!on_event("/x/a.kicad_pro", &json!({"type": "ET_SELECTION_CHANGED"})));
    }
}

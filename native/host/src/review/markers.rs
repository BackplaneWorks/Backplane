//! The design review's findings as markers in the open KiCad (Backplane KiCad bp.13: `SetHostMarkers`, the fork's BACKPLANE_EMBED.md):
//! one named set, "review", per editor (the board's findings in the PCB editor, the schematic's in the schematic editor), rings in the
//! host's colours by severity, never written to the design (host state: not in the file, DRC/ERC results or undo). Sent after each review
//! run, cleared when the run has none; only where the runtime has the command. A click on one (ET_MARKER_CLICKED, set "review") opens the
//! Checks panel on that finding. Pure `markers`, tested.
use crate::files::kicad::findings::Finding;
use crate::hub::Hub;
use serde_json::{json, Value};
use std::sync::Arc;

pub const SET: &str = "review";
/// Markers sent per editor at most (the worst first).
pub const MAX: usize = 400;

/// The markers of the findings for one editor ("pcb" or "sch"): each finding anchored there, its id, title, severity, items (KIIDs),
/// place (nanometres, when it has one and no items), layer, sheet, detail with the fix.
pub fn markers(fs: &[Finding], doc: &str) -> Vec<Value> {
    let mut v: Vec<(u8, Value)> = vec![];
    for f in fs {
        let mine: Vec<_> = f.anchors.iter().filter(|a| a.doc == doc).collect();
        if mine.is_empty() || f.severity == "excluded" { continue }
        let items: Vec<Value> = mine.iter().filter(|a| !a.uuid.is_empty()).map(|a| json!({"value": a.uuid})).collect();
        let at = mine.iter().find(|a| a.at);
        if items.is_empty() && at.is_none() { continue } // nowhere to put it
        let sev = match f.severity.as_str() { "error" => "MS_ERROR", "warning" => "MS_WARNING", _ => "MS_INFO" };
        let mut m = json!({"id": f.id, "title": format!("{} ({})", f.title, f.source), "severity": sev, "items": items,
            "detail": if f.fix.is_empty() { f.detail.clone() } else { format!("{} Fix: {}", f.detail, f.fix).trim().to_string() }});
        if let Some(a) = at { if items_empty(&m) { m["at_nm"] = json!({"x_nm": (a.x * 1e6).round() as i64, "y_nm": (a.y * 1e6).round() as i64}) } }
        if let Some(l) = mine.iter().map(|a| a.layer.as_str()).find(|l| !l.is_empty()) { if doc == "pcb" { m["layer"] = json!(l) } }
        // the sheet only for a marker without items (ERC's reports name sheets by path, KiCad's field wants KIIDs; items carry their own)
        if doc == "sch" && items_empty(&m) { if let Some(s) = mine.iter().map(|a| a.sheet.as_str()).find(|s| s.starts_with('/')) { m["sheet_path"] = json!(s) } }
        v.push((crate::files::kicad::findings::rank(&f.severity), m));
    }
    v.sort_by_key(|x| x.0);
    v.into_iter().take(MAX).map(|x| x.1).collect()
}
fn items_empty(m: &Value) -> bool { m["items"].as_array().map_or(true, Vec::is_empty) }
/// Whether the open KiCad has the command (its capabilities name it).
pub fn capable(caps: &Value) -> bool { caps.to_string().contains("SetHostMarkers") }

/// Send a run's findings to the open KiCad of the project, if it can show them (the hub's own actor).
pub async fn send(hub: &Arc<Hub>, pro: &str, fs: &[Finding]) {
    let Some(route) = crate::files::kicad::embed::gui_route(pro) else { return };
    let caps = crate::files::kicad::ipc::kicad_ipc_status_in(hub, None, pro).await.map(|v| v["capabilities"].clone()).unwrap_or(Value::Null);
    if !capable(&caps) { return }
    let me = crate::files::kicad::coedit::client_name(&crate::files::kicad::coedit::Who::Hub("review"));
    for (doc, kind) in [("pcb", "FK_PCB"), ("sch", "FK_SCH")] {
        let ms = markers(fs, doc);
        let o = crate::files::kicad::embed::gui_call_as(&route, "kiapi.backplane.embed.SetHostMarkers", json!({"kind": kind, "set": SET, "markers": ms}), &me).await;
        if !o["ok"].as_bool().unwrap_or(false) { eprintln!("review: KiCad took no {doc} markers: {}", o["error"].as_str().unwrap_or("no answer")) }
    }
}

/// ET_MARKER_CLICKED on one of ours: the Checks panel opens on the project, and the finding is published as the one to show
/// (info `review.clicked.<digest>`: {id, at}). Answers whether the event was ours.
pub fn on_event(pro: &str, ev: &Value) -> bool {
    if !(ev["type"] == "ET_MARKER_CLICKED" || ev["type"].as_i64() == Some(26)) || ev["marker"]["set"] != SET { return false }
    let id = ev["marker"]["id"].as_str().unwrap_or("").to_string();
    let Some(ag) = crate::agents::handle() else { return true };
    let (hub, pro) = (ag.hub.clone(), pro.to_string());
    tokio::spawn(async move {
        use crate::api::RpcHandler;
        let _ = crate::files::kicad::mgmt::MgmtRpc.call(&hub, &crate::api::Caller { gen: String::new(), client: 0 }, &json!(0), "mgmt.panel", &json!({"project": pro, "open": "checks"})).await;
        let _ = hub.set_info(&format!("review.clicked.{}", crate::files::kicad::kserve::digest(&pro)), &json!({"id": id, "at": crate::hub::now_ms()}).to_string()).await;
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::kicad::findings::Anchor;
    fn f(id: &str, sev: &str, anchors: Vec<Anchor>) -> Finding { Finding { id: id.into(), source: "fab".into(), severity: sev.into(), rule: "r".into(), title: format!("t {id}"), detail: "d".into(), anchors, fix: "fix it".into() } }
    #[test] fn review_markers_one_set_per_editor_worst_first_placed_by_items_or_point() {
        let pcb = |uuid: &str, at: bool| Anchor { doc: "pcb".into(), uuid: uuid.into(), layer: "F.Cu".into(), x: 10.5, y: 20.0, at, ..Default::default() };
        let fs = vec![f("a", "info", vec![pcb("", true)]), f("b", "error", vec![pcb("k1", false)]), f("c", "warning", vec![Anchor { doc: "sch".into(), uuid: "k2".into(), sheet: "/x/".into(), ..Default::default() }]),
            f("d", "warning", vec![Anchor { doc: "file".into(), text: "mfg/x".into(), ..Default::default() }]), f("e", "excluded", vec![pcb("k3", false)])];
        let p = markers(&fs, "pcb");
        assert_eq!(p.iter().map(|m| m["id"].as_str().unwrap()).collect::<Vec<_>>(), ["b", "a"], "errors first; a file's finding has no place; exclusions are not drawn");
        assert_eq!((p[0]["severity"].clone(), p[0]["items"].clone(), p[0]["layer"].clone()), (json!("MS_ERROR"), json!([{"value": "k1"}]), json!("F.Cu")));
        assert_eq!(p[1]["at_nm"], json!({"x_nm": 10500000, "y_nm": 20000000}), "no items: its point, in nanometres");
        assert!(p[0]["detail"].as_str().unwrap().ends_with("Fix: fix it"));
        let s = markers(&fs, "sch"); assert_eq!((s.len(), s[0]["sheet_path"].clone()), (1, Value::Null), "items carry their sheet");
        assert!(capable(&json!({"commands": ["kiapi.backplane.embed.SetHostMarkers"]}))); assert!(!capable(&json!({"commands": []})));
        assert!(!on_event("/p/a.kicad_pro", &json!({"type": "ET_MARKER_CLICKED", "marker": {"set": "agent:router", "id": "x"}})), "another set is not ours");
        assert!(!on_event("/p/a.kicad_pro", &json!({"type": "ET_SELECTION_CHANGED"})));
    }
}

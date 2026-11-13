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
        // the sheet as KIIDs ("/<root>/<sheet>", the symbol's instance: the right one of a sheet used twice); a sheet named by its
        // names ("/Power/", ERC's) only for a marker without items, which carry their own
        if doc == "sch" { if let Some(s) = mine.iter().map(|a| a.sheet.as_str()).find(|s| kiid_path(s) || (s.starts_with('/') && items_empty(&m))) { m["sheet_path"] = json!(s) } }
        v.push((crate::files::kicad::findings::rank(&f.severity), m));
    }
    v.sort_by_key(|x| x.0);
    v.into_iter().take(MAX).map(|x| x.1).collect()
}
/// "/<uuid>/<uuid>": a sheet path of KIIDs
fn kiid_path(s: &str) -> bool { s.starts_with('/') && s.len() > 1 && s.trim_matches('/').split('/').all(|k| k.len() == 36 && k.chars().all(|c| c.is_ascii_hexdigit() || c == '-')) }
fn items_empty(m: &Value) -> bool { m["items"].as_array().map_or(true, Vec::is_empty) }
/// Whether the open KiCad shows host markers: its own GetCapabilities (the embedded editor's, not the headless server's) lists the
/// capability `embed-host-markers` or the command SetHostMarkers.
pub fn capable(caps: &Value) -> bool {
    let has = |k: &str, f: &dyn Fn(&str) -> bool| caps[k].as_array().map_or(false, |a| a.iter().filter_map(Value::as_str).any(|s| f(s)));
    has("capabilities", &|s| s == "embed-host-markers") || has("commands", &|s| s.ends_with("SetHostMarkers"))
}

/// Send a run's findings to the open KiCad of the project, if it can show them (the hub's own actor).
pub async fn send(_hub: &Arc<Hub>, pro: &str, fs: &[Finding]) {
    let Some(route) = crate::files::kicad::embed::gui_route(pro) else { return };
    // the route's capabilities (read when the editor came up), else the editor's own GetCapabilities
    let caps = if capable(&json!({"capabilities": route.caps})) { json!({"capabilities": route.caps}) } else {
        crate::files::kicad::embed::gui_call(&route, "kiapi.backplane.GetCapabilities", json!({})).await["message"].clone()
    };
    if !capable(&caps) { eprintln!("review: no markers: the open KiCad does not list embed-host-markers ({} capabilities)", route.caps.len()); return }
    let me = crate::files::kicad::coedit::client_name(&crate::files::kicad::coedit::Who::Hub("review"));
    for (doc, kind) in [("pcb", "FK_PCB"), ("sch", "FK_SCH")] {
        let ms = markers(fs, doc);
        let o = crate::files::kicad::embed::gui_call_as(&route, "kiapi.backplane.embed.SetHostMarkers", json!({"kind": kind, "set": SET, "markers": ms}), &me).await;
        if o["ok"].as_bool().unwrap_or(false) { eprintln!("review: {} {doc} markers sent to KiCad", ms.len()) } else { eprintln!("review: KiCad took no {doc} markers: {}", o["error"].as_str().unwrap_or("no answer")) }
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
        let k = "11111111-2222-3333-4444-555555555555"; let kp = format!("/{k}/{k}");
        let s = markers(&[f("m", "error", vec![Anchor { doc: "sch".into(), uuid: "k4".into(), sheet: kp.clone(), reference: "R5".into(), ..Default::default() }])], "sch");
        assert_eq!((s[0]["items"].clone(), s[0]["sheet_path"].clone()), (json!([{"value": "k4"}]), json!(kp)), "a symbol's instance sheet, as KIIDs, goes with its items");
        assert!(capable(&json!({"commands": ["kiapi.backplane.embed.SetHostMarkers"]}))); assert!(!capable(&json!({"commands": []})));
        assert!(capable(&json!({"capabilities": ["routing-plan", "embed-host-markers"], "commands": []})));
        assert!(!capable(&json!({"capabilities": ["routing-plan"], "commands": ["kiapi.backplane.embed.GetMarkers"]})));
        assert!(!on_event("/p/a.kicad_pro", &json!({"type": "ET_MARKER_CLICKED", "marker": {"set": "agent:router", "id": "x"}})), "another set is not ours");
        assert!(!on_event("/p/a.kicad_pro", &json!({"type": "ET_SELECTION_CHANGED"})));
    }
}

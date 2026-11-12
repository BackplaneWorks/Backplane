//! The Fab management panel (docs/MANAGEMENT.md, docs/fab-profiles.md): the project's fab and tier, its limits with source and date,
//! what changed since the project last looked, the DRC section's state, the board setup suggestions and the conflicts. Published as
//! info `mgmt.fab.<digest>` for the desktop and the phones; the panel's buttons are `fab.profile {pro, action}` (drc, apply, seen,
//! refresh, stackup), after which it is published again. Pure `data`, tested.
use crate::{files::kicad::mgmt, hub::Hub};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Arc};

pub struct FabPanel;
impl mgmt::Panel for FabPanel {
    fn id(&self) -> &'static str { "fab" }
    fn label(&self) -> &'static str { "Fab" }
    fn watch(&self, hub: &Arc<Hub>, project: &str) { let (h, p) = (hub.clone(), project.to_string()); tokio::spawn(async move { publish(&h, &p).await; }); }
}

/// A value as the panel shows it: "0.09 mm", "32", "hasl, enig".
pub fn shown(v: &Value, unit: &str) -> String {
    let s = match v { Value::Array(a) => a.iter().map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())).collect::<Vec<_>>().join(", "), Value::String(s) => s.clone(), Value::Bool(b) => if *b { "yes".into() } else { "no".into() }, x => x.to_string() };
    if unit.is_empty() || matches!(v, Value::Array(_) | Value::Bool(_)) { s } else if unit == "%" { format!("±{s}%") } else { format!("{s} {unit}") }
}
/// The panel's data from the project's view (Fab.view).
pub fn data(v: &Value) -> Value {
    let ch = &v["choice"]; let o = &ch["options"];
    let n = |x: &Value| x.as_f64().map(|f| { let s = format!("{f}"); s.trim_end_matches(".0").to_string() }).unwrap_or_default();
    let board = format!("{} · {} layers · {} oz · {} mm · {}", ch["kind"].as_str().unwrap_or(""), n(&o["layers"]), n(&o["copper_oz"]), n(&o["thickness_mm"]), o["finish"].as_str().unwrap_or(""));
    let row = |l: &Value| {
        let side = |c: &str| { let x = &l[c]; if x.is_null() { Value::Null } else { json!({"text": shown(&x["value"], x["unit"].as_str().unwrap_or("")), "source": x["source"], "fetched": x["fetched"], "confidence": x["confidence"], "note": x["note"]}) } };
        json!({"key": l["key"], "label": l["label"], "free": side("free"), "extra": side("extra"), "changed": l["changed"] == true, "inherited": l["inherited"] == true})
    };
    let limits: Vec<Value> = v["limits"].as_array().into_iter().flatten().map(row).collect();
    let d = &v["drc"];
    let drc = if d.is_null() { "noproject" } else if d["current"] == true { "current" } else if d["present"] == true { "stale" } else { "none" };
    let sugg: Vec<Value> = d["suggestions"].as_array().cloned().unwrap_or_default();
    let conf: Vec<Value> = d["conflicts"].as_array().cloned().unwrap_or_default();
    let changed: Vec<Value> = v["changed"].as_array().cloned().unwrap_or_default();
    let n_bad = conf.len(); let n_warn = sugg.len() + changed.len() + usize::from(drc == "stale");
    json!({"badge": {"n": n_bad + n_warn, "tone": if n_bad > 0 { "bad" } else if n_warn > 0 { "warn" } else { "" }},
        "fab": v["fab"], "rev": v["rev"], "updated": v["updated"], "by": ch["by"], "tier": ch["tier"], "board": board,
        "gathered": v["gathered"]["at"], "missing": v["gathered"]["report"]["missing"].as_array().map_or(0, Vec::len), "notes": v["gathered"]["report"]["notes"],
        "drc": {"state": drc, "file": d["file"]}, "suggestions": sugg, "conflicts": conf, "changed": changed, "limits": limits,
        "stackups": v["stackups"].as_array().map_or(0, Vec::len),
        "stackup": {"chosen": v["stackup"]["chosen"], "by": v["stackup"]["by"], "why": v["stackup"]["proposals"].as_array().and_then(|a| a.iter().find(|x| x["id"] == v["stackup"]["chosen"])).map_or(Value::Null, |x| x["why"].clone()),
            "outer_50_mm": v["stackup"]["layers"][0]["widths"]["50"]}})
}

/// The project folder Backplane knows that holds a .kicad_pro (the deepest root that contains it).
pub async fn root_of_pro(hub: &Arc<Hub>, pro: &str) -> Option<PathBuf> {
    let m = hub.model().await;
    m.projects.values().filter(|p| !p.removed && !p.root.is_empty() && Path::new(pro).starts_with(&p.root)).max_by_key(|p| p.root.len()).map(|p| PathBuf::from(crate::files::kicad::canonical::design_root(&p.root)))
}
/// Publish the panel for a project (its .kicad_pro).
pub async fn publish(hub: &Arc<Hub>, pro: &str) {
    let Some(root) = root_of_pro(hub, pro).await else { return };
    crate::review::policy::watch(&root); // a shown design is reviewed on save
    let home = hub.cfg.home.clone();
    let r0 = root.clone(); let v = tokio::task::spawn_blocking(move || super::view_in(&home, Some(&r0))).await;
    let mut d = match v { Ok(Ok(v)) => data(&v), Ok(Err(e)) => json!({"badge": {"n": 0, "tone": ""}, "error": e}), Err(_) => return };
    // the design review's last run: its verdict and counts (its findings are in the Checks list)
    let r2 = root.clone(); d["review"] = tokio::task::spawn_blocking(move || crate::review::last(&r2)).await.unwrap_or(Value::Null);
    d["review"]["running"] = json!(REVIEWING.lock().unwrap().contains(&root));
    mgmt::publish(hub, "fab", pro, d).await;
}
/// The projects whose design review the panel started and is running now.
pub static REVIEWING: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
/// The panel's Run review: a full review in the background, the panel published when it starts and when it ends.
pub async fn review(hub: &Arc<Hub>, pro: &str) -> Result<Value, String> {
    let root = root_of_pro(hub, pro).await.ok_or("not in a project this hub knows")?;
    { let mut r = REVIEWING.lock().unwrap(); if r.contains(&root) { return Ok(json!({"running": true})) } r.push(root.clone()) }
    let (h, p, r) = (hub.clone(), pro.to_string(), root.clone());
    tokio::spawn(async move {
        let out = crate::review::run(&h, &r, "full").await;
        REVIEWING.lock().unwrap().retain(|x| x != &r);
        if let Err(e) = out { eprintln!("review: {e}") }
        publish(&h, &p).await;
    });
    Ok(json!({"running": true}))
}
/// Publish again for every project shown on that fab (after a gather changed it).
pub async fn publish_all(hub: &Arc<Hub>) {
    let pros: Vec<String> = { let info = hub.info_now(); info.as_object().map(|o| o.iter().filter(|(k, _)| k.starts_with("mgmt.fab.")).filter_map(|(_, v)| v.as_str().and_then(|t| serde_json::from_str::<Value>(t).ok()).and_then(|v| v["project"].as_str().map(str::to_string))).collect()).unwrap_or_default() };
    for p in pros { publish(hub, &p).await }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn fab_panel_data_says_the_fab_the_drc_state_and_what_needs_the_person() {
        let v = json!({"fab": "jlcpcb", "rev": 2, "updated": "2026-11-12", "choice": {"kind": "pcb", "options": {"layers": 4.0, "copper_oz": 1.0, "thickness_mm": 1.6, "finish": "enig"}, "by": "board", "tier": "free"},
            "limits": [{"key": "track_min", "label": "track width", "free": {"value": 0.09, "unit": "mm", "source": "https://x", "fetched": "2026-11-12", "confidence": "published"}, "extra": {"value": 0.076, "unit": "mm", "source": "https://y", "fetched": "2026-11-12", "confidence": "derived"}, "changed": true},
                {"key": "finishes", "label": "surface finishes", "free": {"value": ["enig", "osp"], "unit": "", "source": "https://x", "fetched": "2026-11-12", "confidence": "published"}}],
            "drc": {"present": true, "current": false, "file": "a.kicad_dru", "suggestions": [{"key": "min_clearance"}], "conflicts": ["rule \"x\" allows clearance 0.08mm"]},
            "changed": ["kind pcb track_min (free): 0.1 -> 0.09"], "stackups": [{}, {}], "gathered": {"at": "2026-11-12", "report": {"missing": []}}});
        let d = data(&v);
        assert_eq!(d["board"], "pcb · 4 layers · 1 oz · 1.6 mm · enig"); assert_eq!(d["drc"]["state"], "stale");
        assert_eq!(d["badge"], json!({"n": 4, "tone": "bad"}), "a conflict is bad; a suggestion, a change and a stale section are warnings");
        assert_eq!((d["limits"][0]["free"]["text"].clone(), d["limits"][0]["extra"]["text"].clone(), d["limits"][0]["changed"].clone()), (json!("0.09 mm"), json!("0.076 mm"), json!(true)));
        assert_eq!(d["limits"][1]["free"]["text"], "enig, osp"); assert_eq!(d["limits"][1]["extra"], Value::Null);
        let quiet = data(&json!({"choice": {}, "drc": {"present": true, "current": true}})); assert_eq!(quiet["badge"], json!({"n": 0, "tone": ""})); assert_eq!(quiet["drc"]["state"], "current");
        assert_eq!(data(&json!({"choice": {}}))["drc"]["state"], "noproject");
        assert_eq!(shown(&json!(10), "%"), "±10%"); assert_eq!(shown(&json!(true), ""), "yes");
    }
}

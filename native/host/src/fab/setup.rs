//! A board setup suggestion the person approved, applied: one minimum of the board's design rules raised to the fab's. With the
//! person's KiCad up it goes through KiCad's API (GetBoardDesignRules, then SetBoardDesignRules with the constraints alone, so nothing
//! else of the board setup moves): the board shows as modified and they save it (KiCad keeps design rules out of its undo history, so
//! the answer says what it was). Without it (the headless KiCad has no design-rules API), only while no thread holds the board's lock,
//! the headless KiCad is retired and the one number is changed in the .kicad_pro, every other byte kept. Only on the person's press (the panel's Apply, `fab.profile {action: "apply"}`); agents
//! are told to name suggestions, never to apply them.
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};

/// The .kicad_pro's board minimum (`board.design_settings.rules.<key>`) as KiCad's API names it in `MinimumConstraints`.
pub fn field(pro_key: &str) -> Option<&'static str> {
    Some(match pro_key {
        "min_clearance" => "min_clearance", "min_track_width" => "min_track_width", "min_via_annular_width" => "min_via_annular_width",
        "min_via_diameter" => "min_via_size", "min_through_hole_diameter" => "min_through_drill", "min_microvia_diameter" => "min_microvia_size",
        "min_microvia_drill" => "min_microvia_drill", "min_copper_edge_clearance" => "copper_edge_clearance", "min_hole_clearance" => "hole_clearance",
        "min_hole_to_hole" => "hole_to_hole_min", "min_silk_clearance" => "silk_clearance", "min_text_height" => "min_silk_text_height",
        "min_text_thickness" => "min_silk_text_thickness", _ => return None,
    })
}
/// The SetBoardDesignRules message: the board's own constraints (from GetBoardDesignRules' answer) with one raised to `mm`.
/// Err when the answer has no constraints or the value would lower the board's own.
pub fn set_message(board: &Value, got: &Value, pro_key: &str, mm: f64) -> Result<(Value, f64), String> {
    let f = field(pro_key).ok_or_else(|| format!("{pro_key} is not a board setup minimum Backplane can set"))?;
    let mut c = got.pointer("/rules/constraints").cloned().filter(Value::is_object).ok_or("KiCad's answer has no design rule constraints")?;
    let nm = |v: &Value| v["value_nm"].as_i64().or_else(|| v["value_nm"].as_str().and_then(|s| s.parse().ok())).unwrap_or(0);
    let was = nm(&c[f]) as f64 / 1e6; let to = (mm * 1e6).round() as i64;
    if (to as f64) < nm(&c[f]) as f64 { return Err(format!("the board's {pro_key} is already {was} mm, stricter than {mm} mm")) }
    c[f] = json!({"value_nm": to});
    Ok((json!({"board": board, "rules": {"constraints": c}}), was))
}
/// The board's document specifier for KiCad's API.
pub fn spec(pcb: &str) -> Value {
    let p = Path::new(pcb); let dir = p.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    json!({"type": "DOCTYPE_PCB", "board_filename": name, "project": {"name": name.trim_end_matches(".kicad_pcb"), "path": dir}})
}

/// Apply one suggestion of the project's plan (its key) and answer what was done.
pub async fn apply(hub: &Arc<Hub>, root: &Path, key: &str) -> Result<Value, String> {
    if crate::copy::on() { return Err(crate::copy::HOME_ONLY.into()) }
    let (home, r, k) = (hub.cfg.home.clone(), root.to_path_buf(), key.to_string());
    let (sug, pcb, pro) = tokio::task::spawn_blocking(move || -> Result<(Value, String, String), String> {
        let c = super::choice(&r); let p = super::profile_of(&home, &c.fab)?;
        let (_, plan) = super::drc_plan(&r, &p, &c).ok_or("this project has no KiCad project")?;
        let s = plan.suggestions.iter().find(|s| s["key"] == k.as_str()).cloned().ok_or_else(|| format!("{k} is not a suggestion now (the board setup may already meet the fab)"))?;
        let f = crate::files::kicad::project::read(&r.to_string_lossy()); Ok((s, f.pcb, f.pro))
    }).await.map_err(|_| "failed".to_string())??;
    let mm = sug["fab"].as_f64().ok_or("the suggestion has no value")?;
    let gui = crate::files::kicad::embed::gui_route(&pro).is_some();
    if !gui {
        let name = crate::files::kicad::canonical::doc_lock(&pcb);
        let l = crate::agents::lockrec::Lock::read(&hub.setting(&format!("lock.{name}")).await.unwrap_or_default());
        if let Some(h) = &l.holder { return Err(format!("the board is being edited ({} holds {name}); apply this when it is free", if h.thread.is_empty() { &h.id } else { &h.thread })) }
    }
    if gui {
        let board = spec(&pcb);
        let got = crate::files::kicad::ipc::kicad_ipc_in(hub, None, &pcb, "kiapi.board.commands.GetBoardDesignRules", json!({"board": board})).await?;
        let (msg, was) = set_message(&board, got.get("message").unwrap_or(&got), key, mm)?;
        crate::files::kicad::ipc::kicad_ipc_in(hub, None, &pcb, "kiapi.board.commands.SetBoardDesignRules", msg).await?;
        return Ok(json!({"applied": key, "was_mm": was, "now_mm": mm, "in": "kicad", "saved": false, "note": format!("set in KiCad's Board Setup; save the board to keep it (it was {was} mm)")}))
    }
    // no KiCad of the person's: the headless KiCad (which has no design-rules API) is retired first so it holds no older copy, then the
    // one value is changed in the project file, every other byte kept
    let _ = crate::files::kicad::ipc::retire(&pro).await;
    let (k, p2) = (key.to_string(), pro.clone());
    let was = tokio::task::spawn_blocking(move || -> Result<f64, String> {
        let text = std::fs::read_to_string(&p2).map_err(|e| e.to_string())?;
        let (new, was) = edit_pro(&text, &k, mm)?;
        let tmp = format!("{p2}.fab-tmp"); std::fs::write(&tmp, new).map_err(|e| e.to_string())?; std::fs::rename(&tmp, &p2).map_err(|e| e.to_string())?; Ok(was)
    }).await.map_err(|_| "failed".to_string())??;
    Ok(json!({"applied": key, "was_mm": was, "now_mm": mm, "in": "project file", "saved": true, "note": format!("set in the project's board setup (it was {was} mm)")}))
}

/// The project file with one board minimum (`board.design_settings.rules.<key>`) raised to `mm`: only that number's text changes.
pub fn edit_pro(text: &str, key: &str, mm: f64) -> Result<(String, f64), String> {
    let v: Value = serde_json::from_str(text).map_err(|_| "the project file is not JSON")?;
    let was = v.pointer(&format!("/board/design_settings/rules/{key}")).and_then(Value::as_f64).ok_or_else(|| format!("the project file has no {key}"))?;
    if was >= mm - 1e-9 { return Err(format!("the board's {key} is already {was} mm")) }
    let ds = text.find("\"design_settings\"").ok_or("no design_settings")?;
    let rules = ds + text[ds..].find("\"rules\"").ok_or("no rules")?;
    let at = rules + text[rules..].find(&format!("\"{key}\"")).ok_or("no key")?;
    let colon = at + text[at..].find(':').ok_or("no value")? + 1;
    let start = colon + text[colon..].len() - text[colon..].trim_start().len();
    let end = start + text[start..].find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))).unwrap_or(text.len() - start);
    let out = format!("{}{}{}", &text[..start], mm, &text[end..]);
    let check: Value = serde_json::from_str(&out).map_err(|_| "the edit would break the project file")?;
    if check.pointer(&format!("/board/design_settings/rules/{key}")).and_then(Value::as_f64) != Some(mm) { return Err("the edit missed its value".into()) }
    Ok((out, was))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn fab_setup_one_minimum_is_raised_and_the_rest_kept() {
        let got = json!({"rules": {"constraints": {"min_clearance": {"value_nm": "0"}, "min_track_width": {"value_nm": 90000}, "copper_edge_clearance": {"value_nm": 500000}}, "predefined_sizes": {"tracks": []}}});
        let b = spec("/p/boards/a/a.kicad_pcb"); assert_eq!(b, json!({"type":"DOCTYPE_PCB","board_filename":"a.kicad_pcb","project":{"name":"a","path":"/p/boards/a"}}));
        let (m, was) = set_message(&b, &got, "min_clearance", 0.09).unwrap();
        assert_eq!(was, 0.0); assert_eq!(m["rules"]["constraints"]["min_clearance"], json!({"value_nm": 90000})); assert_eq!(m["rules"]["constraints"]["min_track_width"], json!({"value_nm": 90000}));
        assert!(m["rules"].get("predefined_sizes").is_none(), "only the constraints are sent, so nothing else of the setup moves");
        assert!(set_message(&b, &got, "min_copper_edge_clearance", 0.2).unwrap_err().contains("stricter"), "never lowers the board's own");
        assert!(set_message(&b, &json!({}), "min_clearance", 0.1).is_err()); assert!(set_message(&b, &got, "min_groove_width", 0.1).is_err());
        assert_eq!(field("min_via_diameter"), Some("min_via_size"));
    }
    #[test] fn fab_setup_the_project_file_changes_in_one_number() {
        let pro = "{\n  \"board\": {\n    \"design_settings\": {\n      \"defaults\": {\"min_clearance\": 5.0},\n      \"rules\": {\n        \"min_clearance\": 0.0,\n        \"min_track_width\": 0.2\n      }\n    }\n  },\n  \"meta\": {\"version\": 3}\n}\n";
        let (out, was) = edit_pro(pro, "min_clearance", 0.1).unwrap();
        assert_eq!(was, 0.0); assert_eq!(out, pro.replace("\"min_clearance\": 0.0", "\"min_clearance\": 0.1"), "only that number, in rules, not the one before it");
        assert!(edit_pro(pro, "min_track_width", 0.1).unwrap_err().contains("already")); assert!(edit_pro(pro, "min_hole_to_hole", 0.2).is_err()); assert!(edit_pro("x", "min_clearance", 0.1).is_err());
    }
}

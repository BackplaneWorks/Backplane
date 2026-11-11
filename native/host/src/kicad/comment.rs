//! "Comment to Backplane…" (the fork's ET_COMMENT, bp.9+): the person selected things in KiCad, right-clicked and wrote a note. The event's
//! screenshot (KiCad keeps only its 32 newest) is copied beside the hub's data, and the comment is published as info `kicad.comment.<digest>`
//! {seq, project, frame, note, names, items, bbox, sheet_path, image}; the window that shows the project's editor posts it into the open
//! thread as the person's message (viewers/kicad_pane.rs `comment_text`), its references links that cross-probe.
use super::kserve as ks;
use crate::hub::Hub;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, Weak};

static HUB: Mutex<Option<Weak<Hub>>> = Mutex::new(None);
pub fn start(hub: &Arc<Hub>) { *HUB.lock().unwrap() = Some(Arc::downgrade(hub)); }

/// Whether `ev` is a comment (ET_COMMENT = 19).
pub fn is_comment(ev: &Value) -> bool { ev["type"] == "ET_COMMENT" || ev["type"].as_i64() == Some(19) }

/// The published comment from the event: the note, the selection's names and up to 40 items, with `image` the copied screenshot.
pub fn published(pro: &str, ev: &Value, image: &str) -> Value {
    let c = &ev["comment"];
    let items: Vec<Value> = c["items"].as_array().into_iter().flatten().take(40).cloned().collect();
    json!({"seq": ev["seq"], "project": pro, "frame": ev["frame"]["kind"], "note": c["note"].as_str().or(ev["detail"].as_str()).unwrap_or(""),
           "names": c["names"], "items": items, "truncated": c["truncated"].as_bool().unwrap_or(false) || c["items"].as_array().map_or(0, Vec::len) > 40,
           "bbox": c["bbox"], "sheet_path": c["sheet_path"], "empty": c["empty_selection"].as_bool().unwrap_or(false), "image": image, "at": crate::hub::now_ms()})
}

/// From the fork's events loop (embed.rs): true when it was a comment (handled here).
pub fn on_event(pro: &str, ev: &Value) -> bool {
    if !is_comment(ev) { return false }
    let Some(hub) = HUB.lock().unwrap().as_ref().and_then(Weak::upgrade) else { return true };
    let (pro, ev) = (pro.to_string(), ev.clone());
    tokio::spawn(async move {
        let src = ev["comment"]["screenshot"].as_str().unwrap_or("").to_string();
        let digest = ks::digest(&pro);
        let image = if src.is_empty() { String::new() } else {
            let dir = hub.cfg.home.join("kicad-comments"); let to = dir.join(format!("{digest}-{}.png", ev["seq"].as_str().map(String::from).unwrap_or_else(|| ev["seq"].to_string())));
            let ok = tokio::task::spawn_blocking({ let to = to.clone(); move || std::fs::create_dir_all(&dir).and_then(|_| std::fs::copy(&src, &to)).is_ok() }).await.unwrap_or(false);
            if ok { to.to_string_lossy().into_owned() } else { String::new() }
        };
        hub.set_info(&format!("kicad.comment.{digest}"), &published(&pro, &ev, &image).to_string()).await;
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_comment_event_is_published_with_its_note_names_and_copied_image() {
        let ev = json!({"seq": "4", "type": "ET_COMMENT", "detail": "move this", "frame": {"kind": "FK_PCB"},
            "comment": {"note": "move this", "names": {"references": ["U3"], "nets": ["GND"]}, "items": [{"kind": "FOOTPRINT", "reference": "U3", "layer": "F.Cu", "x_mm": 10.0, "y_mm": 20.0}], "bbox": {"x_min": 9.0}}});
        assert!(is_comment(&ev) && !is_comment(&json!({"type": "ET_SELECTION_CHANGED"})));
        let p = published("/p/a.kicad_pro", &ev, "/h/kicad-comments/x.png");
        assert_eq!(p["note"], "move this"); assert_eq!(p["names"]["references"][0], "U3"); assert_eq!(p["image"], "/h/kicad-comments/x.png"); assert_eq!(p["frame"], "FK_PCB");
    }
}

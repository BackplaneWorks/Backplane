//! What a save writes besides the change it is for. KiCad's GUI rewrites documents nobody changed: an unmodified schematic saved is
//! upgraded to the runtime's format (Collar's collar_main.kicad_sch: 20260306 -> 20261025, 15,776 lines, its symbol paths re-keyed and
//! implied junctions given fresh UUIDs on every save) while KiCad says unmodified throughout (DMS_UNMODIFIED, no change in the co-edit
//! log); and a board opened in the GUI carries its load-time 3D model migration (coedit::LOAD_MIGRATION). So the hub never saves a document
//! KiCad says is unmodified, and a save that does go into a file older than the runtime's format says so, a line in its result and in
//! the commit Backplane makes, so a diff that is mostly KiCad's is not taken for the edit (integrator, Nov 14; bp.12.3 / batch 7 make
//! an unmodified save write nothing, the hub does not depend on it).
use serde_json::{json, Value};
use std::path::Path;

/// The `(version N)` near the top of a .kicad_pcb / .kicad_sch, None when unreadable.
pub fn file_version(text: &str) -> Option<u64> {
    let head: String = text.chars().take(4096).collect();
    let i = head.find("(version ")? + "(version ".len();
    head[i..].split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}
/// The files a save of `doc` writes: the board, or the schematic and every sheet beside it.
pub fn files_of(doc: &str) -> Vec<String> {
    if doc.ends_with(".kicad_pcb") { return vec![doc.to_string()] }
    // the root and the sheets it names (each sheet's "Sheetfile", relative to the file naming it: Hubble_DK keeps its sheets in sheets/)
    let mut v = vec![doc.to_string()]; let mut i = 0;
    while i < v.len() && v.len() < 400 {
        let f = v[i].clone(); i += 1;
        let Ok(t) = std::fs::read_to_string(&f) else { continue };
        let dir = Path::new(&f).parent().map(Path::to_path_buf).unwrap_or_default();
        for name in sheet_files(&t) {
            let p = dir.join(&name).to_string_lossy().into_owned();
            if !v.contains(&p) { v.push(p) }
        }
    }
    v
}
/// The sheet files a schematic names (its sheets' "Sheetfile" properties).
pub fn sheet_files(text: &str) -> Vec<String> {
    text.match_indices("(property \"Sheetfile\" \"").filter_map(|(i, m)| { let rest = &text[i + m.len()..]; rest.find('"').map(|e| rest[..e].to_string()) }).collect()
}
/// The line a save says when it upgrades the files' format: (oldest version among them, the runtime's), None when all are current.
pub fn upgrade_line(versions: &[u64], runtime: u64) -> Option<String> {
    let old = versions.iter().copied().filter(|v| *v > 0 && *v < runtime).min()?;
    Some(format!("KiCad also upgraded the file format in this save (version {old} to {runtime}): most of the diff is KiCad's rewrite, not the edit."))
}
/// The same for a document about to be saved through the open KiCad (`route`): the runtime's own format from GetCapabilities.
pub async fn upgrade_of(route: &super::embed::GuiRoute, doc: &str) -> Option<String> {
    let caps = super::embed::gui_call(route, "kiapi.backplane.GetCapabilities", json!({})).await;
    let field = if doc.ends_with(".kicad_pcb") { "board_file_version" } else { "schematic_file_version" };
    let runtime = caps["message"][field].as_u64().or_else(|| caps["message"][field].as_str().and_then(|s| s.parse().ok()))?;
    let files = files_of(doc);
    let versions = tokio::task::spawn_blocking(move || files.iter().filter_map(|f| std::fs::read_to_string(f).ok().and_then(|t| file_version(&t))).collect::<Vec<u64>>()).await.ok()?;
    upgrade_line(&versions, runtime)
}
/// Whether the open KiCad says `document` (a DocumentSpecifier) is modified; None when it cannot say (an older runtime: the save goes on).
pub async fn modified(route: &super::embed::GuiRoute, document: &Value) -> Option<bool> {
    let o = super::embed::gui_call(route, "kiapi.common.commands.GetDocumentModifiedState", json!({"document": document})).await;
    if o["ok"] != true { return None }
    match o["message"]["state"].as_str() { Some("DMS_MODIFIED") => Some(true), Some("DMS_UNMODIFIED") => Some(false), _ => None }
}
/// Whether KiCad says a save wrote nothing (bp.12.3 / batch 7: AS_OK, its status message "not modified: ..."); older runtimes always write.
pub fn wrote_nothing(reply: &Value) -> bool { reply["message"]["_status"].as_str().is_some_and(|s| s.starts_with("not modified")) }
/// What a skipped save says.
pub const NOTHING_CHANGED: &str = "nothing to save: KiCad says the document is unmodified, and a save would write only KiCad's own rewrite of it (its file format, its load-time changes), so it was not saved";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test] fn the_files_format_and_the_upgrade_line() {
        assert_eq!(file_version("(kicad_sch\n\t(version 20260306)\n\t(generator \"eeschema\")"), Some(20260306));
        assert_eq!(file_version("(kicad_pcb (version 20241229) (generator pcbnew)"), Some(20241229));
        assert_eq!(file_version("nothing"), None);
        assert_eq!(upgrade_line(&[20261025, 20260306], 20261025).unwrap(), "KiCad also upgraded the file format in this save (version 20260306 to 20261025): most of the diff is KiCad's rewrite, not the edit.");
        assert_eq!(upgrade_line(&[20261025], 20261025), None, "current: nothing to say");
        assert_eq!(sheet_files("(sheet (property \"Sheetname\" \"P\") (property \"Sheetfile\" \"sheets/Power.kicad_sch\"))"), ["sheets/Power.kicad_sch"]);
        assert!(wrote_nothing(&json!({"ok": true, "message": {"_status": "not modified: nothing written"}})) && !wrote_nothing(&json!({"ok": true, "message": {}})));
    }
}

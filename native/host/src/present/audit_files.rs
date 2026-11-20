//! The files modules the presenter needs, for the screen-audit bin (which does not carry the whole crate).
#[path="../files/fm.rs"] pub mod fm;
#[path="../files/dir.rs"] pub mod dir;
#[path="../files/ops.rs"] pub mod ops;
#[path="../files/mimg.rs"] pub mod mimg;
/// The audit serves no files: no message names a file it links.
pub mod share { pub fn linkable(_: &str) -> Vec<serde_json::Value> { vec![] } pub fn listed_now() -> Vec<serde_json::Value> { vec![] } pub fn revoke_now(_: &str) -> Result<(), String> { Ok(()) } }
/// The folder watcher needs the hub; the audit has none, so its calls do nothing.
pub mod watch_dir { pub fn touch(_root: &str, _dir: &str, _sig: u64) {} pub fn signature(_listing: &str) -> u64 { 0 } }
/// Pure version wire encoder matching `files/rv.rs` `hist_git`.
pub mod rv { pub fn hist_git(g: &str, c: &str, top: &str, rel: &str) -> String { format!("git\t{g}\t{c}\t{}\t{top}/{rel}", top.chars().count()) } pub fn version_has_file(_: &str, _: &str, _: &str) -> bool { false } }
/// The audit reads no project folders: a thread's design is shown from the folder it names.
pub mod kicad { pub mod canonical { pub fn design_root(root: &str) -> String { root.to_string() } } pub mod gerber { pub use super::super::gerber_names::*; } pub mod embed { pub fn resolve_pro(_: &str) -> Result<String, String> { Err(String::new()) } pub fn rail_simulation(_: &str) -> Option<&'static str> { None } pub fn runtime_picture(_: &str, _: &str) -> serde_json::Value { serde_json::Value::Null } pub fn session_state(_: &str) -> serde_json::Value { serde_json::Value::Null } } pub mod project { pub struct Files { pub pcb: String, pub sch: String }
    pub fn read(_: &str) -> Files { Files { pcb: String::new(), sch: String::new() } }
    pub fn is_altium(_: &Files) -> bool { false } } pub use super::follow_real as follow; pub use super::watch; }
#[path="../kicad/follow.rs"] #[allow(dead_code)] pub mod follow_real;
/// What follow.rs's embedded-KiCad part reaches for (its `super::`): the audit has no KiCad, so no pane holds one and nothing is sent.
#[allow(dead_code)] pub mod ipc { pub fn pro_of(_: &str) -> String { String::new() } }
#[allow(dead_code)] pub mod embed { use serde_json::Value; #[derive(Clone)] pub struct GuiRoute { pub pro: String }
    /// kicad/embed.rs's own mapping (pure), so follow.rs's tests read GetFrames the same here
    pub fn canon_frame(raw: &str) -> &'static str { match raw.trim().to_ascii_uppercase().as_str() {
        "" | "0" | "PCB" | "FK_PCB" | "BOARD" => "PCB", "1" | "SCH" | "FK_SCH" | "SCHEMATIC" => "SCH", "2" | "PCB_3D" | "FK_PCB_3D" | "3D" | "3D_VIEWER" => "PCB_3D",
        "SYMBOL_EDITOR" | "FK_SYMBOL_EDITOR" | "SYM" => "SYMBOL_EDITOR", "FOOTPRINT_EDITOR" | "FK_FOOTPRINT_EDITOR" | "FP" => "FOOTPRINT_EDITOR",
        "GERBVIEW" | "FK_GERBVIEW" => "GERBVIEW", "SIMULATOR" | "FK_SIMULATOR" | "SIMULATION" | "SIM" => "SIMULATOR", _ => "OTHER" } }
    pub fn kind_of_fk_val(v: &Value) -> &'static str { if let Some(s) = v.as_str() { return canon_frame(s) }
        match v.as_i64().unwrap_or(0) { 1 => "PCB", 2 => "SCH", 3 => "SYMBOL_EDITOR", 4 => "FOOTPRINT_EDITOR", 5 => "PCB_3D", 7 => "GERBVIEW", 8 => "SIMULATOR", _ => "OTHER" } }
    pub fn pane_key(_: &str) -> Option<String> { None } pub fn norm_pro_key(p: &str) -> String { p.to_string() }
    pub fn gui_route(_: &str) -> Option<GuiRoute> { None } pub async fn gui_call(_: &GuiRoute, _: &str, _: Value) -> Value { Value::Null } pub fn session_state(_: &str) -> Value { Value::Null } }
#[allow(dead_code)] pub mod watch { pub fn pane_frame_of(_: &str, _: &str) -> Option<String> { None } pub const MAX_WATCH_SOURCES: usize = 12; pub struct Plots; impl Plots { pub fn keys_for(_: u32, _: &str, _: &str) -> Vec<String> { vec![] } } }
#[allow(dead_code)] pub mod xprobe { pub fn has_select_items(_: &super::embed::GuiRoute) -> bool { false } }
#[allow(dead_code)] pub mod coedit { pub fn person_selection_at(_: &str, _: &str) -> Option<std::time::Instant> { None } }
#[path="../kicad/gerber/names.rs"] #[allow(dead_code)] pub mod gerber_names;

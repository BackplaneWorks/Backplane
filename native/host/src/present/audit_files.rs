//! The files modules the presenter needs, for the screen-audit bin (which does not carry the whole crate).
#[path="../files/fm.rs"] pub mod fm;
#[path="../files/dir.rs"] pub mod dir;
#[path="../files/ops.rs"] pub mod ops;
#[path="../files/mimg.rs"] pub mod mimg;
/// The folder watcher needs the hub; the audit has none, so its calls do nothing.
pub mod watch_dir { pub fn touch(_root: &str, _dir: &str, _sig: u64) {} pub fn signature(_listing: &str) -> u64 { 0 } }
/// The audit reads no project folders: a thread's design is shown from the folder it names.
pub mod kicad { pub mod canonical { pub fn design_root(root: &str) -> String { root.to_string() } } pub mod gerber { pub use super::super::gerber_names::*; } pub mod embed { pub fn resolve_pro(_: &str) -> Result<String, String> { Err(String::new()) } pub fn rail_simulation(_: &str) -> Option<&'static str> { None } } pub mod project { pub struct Files { pub pcb: String, pub sch: String }
    pub fn read(_: &str) -> Files { Files { pcb: String::new(), sch: String::new() } }
    pub fn is_altium(_: &Files) -> bool { false } } pub use super::follow_real as follow; }
#[path="../kicad/follow.rs"] #[allow(dead_code)] pub mod follow_real;
/// What follow.rs's embedded-KiCad part reaches for (its `super::`): the audit has no KiCad, so no pane holds one and nothing is sent.
#[allow(dead_code)] pub mod ipc { pub fn pro_of(_: &str) -> String { String::new() } }
#[allow(dead_code)] pub mod embed { use serde_json::Value; #[derive(Clone)] pub struct GuiRoute { pub pro: String }
    pub fn kind_of_fk_val(_: &Value) -> &'static str { "" } pub fn pane_client(_: &str) -> Option<u64> { None } pub fn norm_pro_key(p: &str) -> String { p.to_string() }
    pub fn gui_route(_: &str) -> Option<GuiRoute> { None } pub async fn gui_call(_: &GuiRoute, _: &str, _: Value) -> Value { Value::Null } pub fn session_state(_: &str) -> Value { Value::Null } }
#[allow(dead_code)] pub mod watch { pub fn pane_frame_of(_: u64, _: &str) -> Option<String> { None } }
#[allow(dead_code)] pub mod xprobe { pub fn has_select_items(_: &super::embed::GuiRoute) -> bool { false } }
#[allow(dead_code)] pub mod coedit { pub fn person_selection_at(_: &str, _: &str) -> Option<std::time::Instant> { None } }
#[path="../kicad/gerber/names.rs"] #[allow(dead_code)] pub mod gerber_names;

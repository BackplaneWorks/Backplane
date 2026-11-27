//! SolidCraft (github.com/tangency-labs/solidcraft, Apache-2.0/MIT): the desktop's CAD pane and the hub's headless CAD program.
//!   - `runtime`: where its programs are (a managed install under `<home>/solidcraft/<version>`, else the system's), and the install of
//!     the pinned release (downloaded, checked against the digest pinned here and the release's SHA256SUMS, unpacked, moved into place).
//!   - `embed`: one GUI SolidCraft per project in the desktop's pane (`editor.solidcraft.*`, info `editor.solidcraft.<digest>`), started
//!     with its control channel on a port and a token the hub chose, so the hub (file follow) and the agent's MCP bridge reach that window.
//!   - `render`: pictures of a model from `solidcraft-cli snapshot`, with no window (viewer_render).
//! The hub does no window work: the client adopts the program's window by its pid, as it does stock KiCad's.
pub mod embed;
pub mod render;
pub mod runtime;
pub mod web;

use crate::hub::Hub;
use std::sync::Arc;

/// What SolidCraft opens, by extension without regard to case: its own designs, B-rep exchange files and meshes.
pub const CAD_EXT: &[&str] = &[".sc", ".solidcraft", ".step", ".stp", ".iges", ".igs", ".stl", ".3mf", ".obj", ".glb"];
/// SolidCraft's own design files: the ones with a timeline and parameters to edit.
pub const DESIGN_EXT: &[&str] = &[".sc", ".solidcraft"];
/// Meshes: triangles only, measured but not edited as solids.
pub const MESH_EXT: &[&str] = &[".stl", ".3mf", ".obj", ".glb"];

fn ends(path: &str, exts: &[&str]) -> bool { let l = path.to_ascii_lowercase(); exts.iter().any(|e| l.len() > e.len() && l.ends_with(e)) }
/// Whether the desktop's CAD pane opens this file.
pub fn is_cad(path: &str) -> bool { ends(path, CAD_EXT) }
/// Whether this is a SolidCraft design (`.sc`, `.solidcraft`).
pub fn is_design(path: &str) -> bool { ends(path, DESIGN_EXT) }
/// Whether this is a mesh file.
pub fn is_mesh(path: &str) -> bool { ends(path, MESH_EXT) }
/// Whether `solidcraft-cli snapshot` is asked to draw this file: designs, STEP and STL (seen drawn by 0.50.0). The other mesh and
/// exchange formats keep the hub's own mesher, and so does any file the CLI refuses (`render::snapshot`'s Err).
pub fn cli_reads(path: &str) -> bool { ends(path, &[".sc", ".solidcraft", ".step", ".stp", ".stl"]) }

pub fn start(hub: &Arc<Hub>) {
    let home = hub.cfg.home.clone();
    crate::files::kicad::solidread::set_solidcraft_hook(Box::new(move || {
        let cli = runtime::cli_path()?;
        if !crate::copy::on() { let _ = embed::seed_settings(&home); }
        Some((cli, embed::data_env(&home)))
    }));
    runtime::start(hub);
    embed::start(hub);
    web::start(hub);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extensions() {
        for p in ["a.step", "A.STP", "mech/case.sc", "x.solidcraft", "p.iges", "p.igs", "m.stl", "m.3mf", "m.obj", "m.GLB"] { assert!(is_cad(p), "{p}"); }
        for p in ["a.kicad_pcb", "notes.md", "step", ".sc", "a.scad", "a.step.bak"] { assert!(!is_cad(p), "{p}"); }
        assert!(is_design("a.SC") && !is_design("a.step"));
        assert!(is_mesh("a.stl") && !is_mesh("a.step"));
        assert!(cli_reads("a.stp") && cli_reads("a.sc") && cli_reads("a.stl") && !cli_reads("a.glb"));
    }
}

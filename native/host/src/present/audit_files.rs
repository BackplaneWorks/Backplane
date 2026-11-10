//! The files modules the presenter needs, for the screen-audit bin (which does not carry the whole crate).
#[path="../files/fm.rs"] pub mod fm;
#[path="../files/dir.rs"] pub mod dir;
#[path="../files/ops.rs"] pub mod ops;
#[path="../files/mimg.rs"] pub mod mimg;
/// The folder watcher needs the hub; the audit has none, so its calls do nothing.
pub mod watch_dir { pub fn touch(_root: &str, _dir: &str, _sig: u64) {} pub fn signature(_listing: &str) -> u64 { 0 } }
/// The audit reads no project folders: a thread's design is shown from the folder it names.
pub mod kicad { pub mod canonical { pub fn design_root(root: &str) -> String { root.to_string() } } }

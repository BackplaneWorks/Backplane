//! The project registry (docs/ALWAYS-ON.md section 5, D7): the uberbot's home keeps every machine's projects and threads, so a machine that is
//! off still shows them. This file holds the copy this hub last read back from the home (`cached`), which `present::merged` folds in.
use crate::present::merged::RegProject;
use std::sync::{OnceLock, RwLock};

fn slot() -> &'static RwLock<Vec<RegProject>> { static S: OnceLock<RwLock<Vec<RegProject>>> = OnceLock::new(); S.get_or_init(Default::default) }
/// The merged registry as the home last sent it (empty before the first sync).
pub fn cached() -> Vec<RegProject> { slot().read().unwrap().clone() }
/// Replace the cached copy (after a sync with the home).
pub fn set_cached(v: Vec<RegProject>) { *slot().write().unwrap() = v; }

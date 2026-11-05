//! The `muse` provider's driver (contract: providers/mod.rs; reference: providers/codex.rs). Owned by its lane.
use super::Made;
pub fn driver(_provider: &str) -> Option<Made> { None }

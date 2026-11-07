//! What the sidebar shows of the owner's other machines: one display-only `State` per link, folded from the mirrored events (already
//! renamed to `<link>~<id>`). It is NOT the policy state: autosettle, turn-end, star and the rest read only `statehost::policies()`, so
//! they never act on another machine's threads, and nothing here reaches the log. Fed beside `Hub::fold_mirror` (start replay, pushes,
//! pulls) and given again after a rebuild (`Hub::forget_mirror`), exactly as the mirrors are. Lean, so a mirror is folded, not kept.
use crate::state::State;
use serde_json::Value;
use std::{collections::HashMap, sync::{Arc, OnceLock, RwLock}};

/// The links whose rows are shown: `(link id, machine name)` in display order. The net lane answers: verified, sync not off.
pub type Names = Arc<dyn Fn() -> Vec<(String, String)> + Send + Sync>;

#[derive(Default)]
pub struct Mirrors { st: RwLock<HashMap<String, State>>, names: OnceLock<Names> }

impl Mirrors {
    pub fn set_names(&self, n: Names) { let _ = self.names.set(n); }
    /// Fold renamed events of one link.
    pub fn fold(&self, link: &str, events: &[Value]) {
        let mut g = self.st.write().unwrap();
        let st = g.entry(link.to_string()).or_insert_with(|| { let mut s = State::new(); s.lean = true; s });
        for e in events { st.apply(e); }
    }
    /// A link's history was replaced.
    pub fn forget(&self, link: &str) { self.st.write().unwrap().remove(link); }
    /// Every mirror is given again after a rebuild.
    pub fn clear(&self) { self.st.write().unwrap().clear(); }
    /// `(link, thread count, project count)` of what is folded, for tests and diagnostics.
    pub fn counts(&self) -> Vec<(String, usize, usize)> {
        let mut v: Vec<_> = self.st.read().unwrap().iter().map(|(l, s)| (l.clone(), s.threads.len(), s.projects.len())).collect(); v.sort(); v
    }
    /// The shown machines' states, in `names` order: `f(&[(machine, state)])`. A link with no name callback yet (or not in it) is not shown.
    pub fn shown<R>(&self, f: impl FnOnce(&[(String, &State)]) -> R) -> R {
        let names = self.names.get().map(|n| n()).unwrap_or_default();
        let g = self.st.read().unwrap();
        let v: Vec<(String, &State)> = names.iter().filter_map(|(l, n)| g.get(l).map(|s| (n.clone(), s))).collect();
        f(&v)
    }
}

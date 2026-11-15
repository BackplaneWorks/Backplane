//! The seam to the memory tree (bots/recall): a thread with infinite context runs each new turn in a fresh session whose first
//! message starts with its view (the whole conversation as one-line summaries) instead of resuming the provider's session,
//! which the provider compacts and a switch hands over. A steer or a ride into a running turn stays in that turn's session.
//! With no recall installed (a test hub) every thread runs as before.
use super::Agents;
use crate::api::Fut;
use std::sync::Arc;

pub trait Recall: Send + Sync {
    /// Whether the thread runs with infinite context (its bot's Settings, or `recall.thread.<id>`), as last folded.
    fn on(&self, thread: &str) -> bool;
    /// The block a fresh turn of the thread starts with: how to read the view, then the view in <chat>; "" when it has no conversation yet.
    fn view<'a>(&'a self, thread: &'a str) -> Fut<'a, String>;
}

impl Agents {
    pub fn set_recall(&self, r: Arc<dyn Recall>) { let _ = self.recall.set(r); }
    pub fn recall(&self) -> Option<Arc<dyn Recall>> { self.recall.get().cloned() }
    /// Whether `thread` runs with infinite context (never in the person's own side-by-side mode).
    pub fn recall_on(&self, thread: &str) -> bool { self.recall.get().map_or(false, |r| r.on(thread)) }
    /// The view a fresh turn of `thread` starts with ("" for none).
    pub async fn recall_view(&self, thread: &str) -> String { match self.recall() { Some(r) => r.view(thread).await, None => String::new() } }
}

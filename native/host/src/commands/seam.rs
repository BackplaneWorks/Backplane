//! What the thread requests ask of the agents module (agents/mod.rs `stop`, `interrupt`, `start_queued`, `steer`). A trait so tests inject a recorder:
//! `agents::handle()` is process-wide.
use crate::api::Fut;
use std::sync::{Arc, Mutex};

pub trait Seam: Send + Sync {
    fn stop<'a>(&'a self, th: &'a str) -> Fut<'a, ()>;
    fn interrupt<'a>(&'a self, th: &'a str) -> Fut<'a, ()>;
    fn start_queued<'a>(&'a self, th: &'a str, text: &'a str) -> Fut<'a, ()>;
    fn steer<'a>(&'a self, th: &'a str, text: &'a str) -> Fut<'a, ()>;
    /// Queue.steer.k's idle branch: one queued message runs as a turn of its own (TurnChanged running, the run); nothing is stopped, no "Steered" note,
    /// the rest of the queue stays.
    fn start_one<'a>(&'a self, th: &'a str, text: &'a str) -> Fut<'a, ()>;
}
impl Seam for Arc<crate::agents::Agents> {
    fn stop<'a>(&'a self, th: &'a str) -> Fut<'a, ()> { Box::pin(async move { let _ = crate::agents::Agents::stop(self, th).await; }) }
    fn interrupt<'a>(&'a self, th: &'a str) -> Fut<'a, ()> { Box::pin(async move { let _ = crate::agents::Agents::interrupt(self, th).await; }) }
    fn start_queued<'a>(&'a self, th: &'a str, text: &'a str) -> Fut<'a, ()> { Box::pin(async move { let _ = crate::agents::Agents::start_queued(self, th, text).await; }) }
    fn steer<'a>(&'a self, th: &'a str, text: &'a str) -> Fut<'a, ()> { Box::pin(async move { let _ = crate::agents::Agents::steer(self, th, text).await; }) }
    fn start_one<'a>(&'a self, th: &'a str, text: &'a str) -> Fut<'a, ()> { Box::pin(async move { let _ = crate::agents::Agents::start_one(self, th, text).await; }) }
}

static OVERRIDE: Mutex<Option<Arc<dyn Seam>>> = Mutex::new(None);
/// Tests: a recorder in place of the agents module.
#[cfg(test)]
pub fn inject(s: Option<Arc<dyn Seam>>) { *OVERRIDE.lock().unwrap() = s; }
/// The agents module of this process, once it has started.
pub fn current() -> Option<Arc<dyn Seam>> {
    if let Some(s) = OVERRIDE.lock().unwrap().clone() { return Some(s) }
    crate::agents::handle().map(|a| Arc::new(a) as Arc<dyn Seam>)
}

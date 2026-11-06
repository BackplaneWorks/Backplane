//! The presenter: the one part of the hub that draws pictures, behind a trait so screens can be replaced one by one.
//! The Rust screens (`present::Routed`) answer everything; there is no helper process.
use anyhow::Result;
use serde_json::Value;
use std::{future::Future, pin::Pin};

pub type Fut<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub trait Presenter: Send + Sync {
    fn name(&self) -> &'static str;
    /// Fold one hub wire frame (log, changes, delta, info, live...) into the presenter's state.
    fn fold<'a>(&'a self, frame: &'a Value) -> Fut<'a, ()>;
    /// view.open / view.update / view.action parameters -> {remoteReady, shown|nextTarget, session, cmds}
    fn view<'a>(&'a self, params: &'a Value) -> Fut<'a, Value>;
    fn ping(&self) -> Fut<'_, ()>;
    /// Whether the presenter wants the hub's log folded into it at start.
    fn wants_log(&self) -> bool { true }
    /// The presenter lost its state (its process died): `restart` it, then the hub folds the log into it again.
    fn is_down(&self) -> bool { false }
    /// Why it went down, once, for the log: how the helper process ended (a signal, an exit code) and the call that found it dead.
    fn why_down(&self) -> Option<String> { None }
    fn restart(&self) -> Fut<'_, ()> { Box::pin(async { Ok(()) }) }
    /// There is no helper at all : nothing a request is sent to can be waited for, so the Rust screens answer everything they can, reads included.
    fn absent(&self) -> bool { false }
}

/// A presenter with canned answers, for tests and for running the hub without the policy binary.
pub struct Canned { pub reply: Value }
impl Presenter for Canned {
    fn name(&self) -> &'static str { "canned" }
    fn wants_log(&self) -> bool { false }
    fn fold<'a>(&'a self, _f: &'a Value) -> Fut<'a, ()> { Box::pin(async { Ok(()) }) }
    fn view<'a>(&'a self, _p: &'a Value) -> Fut<'a, Value> { let r = self.reply.clone(); Box::pin(async move { Ok(r) }) }
    fn ping(&self) -> Fut<'_, ()> { Box::pin(async { Ok(()) }) }
}

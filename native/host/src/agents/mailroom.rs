//! The seam to the bots lane's mailroom (hub.bend `Mail.funnel`, `Operational.*`, `Lr.dest`, `Lr.may`, `Run.queue.hold`, `Perm.run`, server.bend's `Hub.mcp.charter`): a message that is
//! not a person's goes through `Agents::inbound`, which may file it for the uberbot instead of delivering it. With no mailroom installed (a test hub) everything is delivered.
use super::Agents;
use crate::api::Fut;
use serde_json::Value;
use std::sync::Arc;

/// What becomes of a message (Mail.funnel): delivered, filed instead (the result is the filing's), or delivered and filed
pub enum Funnel { Deliver, Filed(Result<(), String>), Tee }
pub type Origin = (String, String, String);

pub trait Mailroom: Send + Sync {
    /// `extra`: the producer's acknowledgements, committed with the delivery or the header; `r`: the reference the mail is filed under
    fn funnel<'a>(&'a self, role: &'a str, to: &'a str, msg: &'a str, text: &'a str, origin: &'a Origin, r: &'a str, extra: &'a [Value]) -> Fut<'a, Funnel>;
    /// the mail a `Funnel::Tee` files once the message is delivered
    fn tee<'a>(&'a self, role: &'a str, to: &'a str, msg: &'a str, text: &'a str) -> Fut<'a, ()>;
    /// Lr.dest: the thread a report for `by` goes to
    fn dest<'a>(&'a self, by: &'a str) -> Fut<'a, String>;
    /// Ub.chat.is: one of the uberbot's threads
    fn chat_is<'a>(&'a self, thread: &'a str) -> Fut<'a, bool>;
    /// Mcp.same_project's part: false when `to` is a back office `from` does not own, true when `from` works as a bot (across projects), None to go by the projects
    fn reach<'a>(&'a self, from: &'a str, to: &'a str) -> Fut<'a, Option<bool>>;
    /// Mcp.run.visible: a mail run is listed only to its own bot's live runs
    fn hidden<'a>(&'a self, caller: &'a str, thread: &'a str) -> Fut<'a, bool>;
    /// the thread a launch from `thread` is made as (a back office launches as its owner)
    fn launcher<'a>(&'a self, thread: &'a str) -> Fut<'a, String>;
    /// Run.queue.hold: the person or an agent stopped `thread`
    fn stopping<'a>(&'a self, thread: &'a str) -> Fut<'a, ()>;
    /// Bots.prompt: the bot part of a thread's system prompt ("" for a thread that works for no bot)
    fn prompt<'a>(&'a self, thread: &'a str) -> Fut<'a, String>;
    /// Bots.turn's block: what a turn of a bot's thread starts with (its memory), "" for none
    fn turn_block<'a>(&'a self, thread: &'a str, text: &'a str) -> Fut<'a, String>;
    /// Bots.mentions: a person's message in `thread` names these bots
    fn mentions<'a>(&'a self, thread: &'a str, bots: Vec<String>, text: &'a str) -> Fut<'a, ()>;
    /// Bots.mention.room: a person's message in `thread` names these rooms (ids): it is posted to each, as the person, with where it was said
    fn room_mentions<'a>(&'a self, thread: &'a str, rooms: Vec<String>, text: &'a str) -> Fut<'a, ()>;
    /// Perm.run: a mail run's verdict on a tool, with no ask (None: not a run)
    fn permit<'a>(&'a self, thread: &'a str, tool: &'a str, input: &'a Value) -> Fut<'a, Option<Value>>;
    /// Hub.mcp.charter: a run's risky direct tool: Some is the refusal, None lets the call go on
    fn gate<'a>(&'a self, thread: &'a str, tool: &'a str, args: &'a Value) -> Fut<'a, Option<(bool, Value)>>;
}

impl Agents {
    pub fn set_mailroom(&self, m: Arc<dyn Mailroom>) { let _ = self.mailroom.set(m); }
    pub fn mailroom(&self) -> Option<Arc<dyn Mailroom>> { self.mailroom.get().cloned() }
    /// The threads the start's recovery cut, once (Run.recover answers for those that were runs)
    pub fn cut_at_start(&self) -> Vec<String> { std::mem::take(&mut *self.cut.lock().unwrap()) }
    /// Mail.inbound: a message that is no person's: through the funnel, filed under the origin's own reference
    pub async fn inbound(self: &Arc<Self>, role: &str, thread: &str, msg: &str, text: &str, origin: Origin, extra: Vec<Value>) -> Result<(), String> {
        let r = origin.2.clone(); self.inbound_ref(role, thread, msg, text, origin, &r, extra).await
    }
    /// the same, filed under `r` (a wake's reference is not the one its delivery carries)
    #[allow(clippy::too_many_arguments)]
    pub async fn inbound_ref(self: &Arc<Self>, role: &str, thread: &str, msg: &str, text: &str, origin: Origin, r: &str, extra: Vec<Value>) -> Result<(), String> {
        let Some(m) = self.mailroom() else { return self.deliver_as(role, thread, msg, text, Some(origin), extra).await };
        match m.funnel(role, thread, msg, text, &origin, r, &extra).await {
            Funnel::Deliver => self.deliver_as(role, thread, msg, text, Some(origin), extra).await,
            Funnel::Filed(res) => res,
            Funnel::Tee => { let res = self.deliver_as(role, thread, msg, text, Some(origin), extra).await; m.tee(role, thread, msg, text).await; res }
        }
    }
}

//! The bots lane: bots, rooms, memory and spaces, routines and reminders, mail, uber ops and the bot tools
//!. decider is a pure function from state and request to
//! changes and outs; here a request commits the same events through `Hub::commit_many`, then performs the outs.
//! A message into a bot's thread goes through `Inbox` (the agents module's `deliver`).
pub mod api;
pub mod mail;
pub mod clock;
pub mod hooks;
pub mod ledger;
pub mod plane;
pub mod post;
pub mod relocate;
pub mod reply;
pub mod routines;
pub mod rpc;
pub mod tools;
pub mod sched_text;
pub mod uber;
pub mod world;
pub use api::{chat_thread, default_tab, page as bot_page, page_from as bot_page_from, room_page};
pub use rpc::{Bots, Inbox};
#[cfg(test)]
mod tests;

use crate::{agents::Agents, hub::Hub};
use std::sync::Arc;

/// Start the lane: registers the `bots.*` requests. Called once after `Agents::start` (main.rs seam, asked of core).
pub async fn start(hub: Arc<Hub>, agents: Arc<Agents>) -> Arc<Bots> { Bots::start(hub, agents).await }

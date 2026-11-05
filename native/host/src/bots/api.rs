//! What other modules ask the bots lane (the session actor for a view whose target is a bot). Free functions over the hub: they find the
//! `Bots` started on it (`start` registers it) and answer from its world. Not started (a test hub): nothing known.
use super::rpc::Bots;
use crate::hub::Hub;
use std::sync::{Arc, Mutex, Weak};

static REG: Mutex<Vec<(usize, Weak<Bots>)>> = Mutex::new(Vec::new());
/// Called by `Bots::start_with`: the hub's bots, found again by `of`.
pub(super) fn register(hub: &Arc<Hub>, b: &Arc<Bots>) {
    let mut r = REG.lock().unwrap(); r.retain(|(_, w)| w.strong_count() > 0); r.push((Arc::as_ptr(hub) as usize, Arc::downgrade(b)));
}
/// the bots lane started on this hub
pub fn of(hub: &Arc<Hub>) -> Option<Arc<Bots>> { let p = Arc::as_ptr(hub) as usize; REG.lock().unwrap().iter().find(|(k, _)| *k == p).and_then(|(_, w)| w.upgrade()) }

/// `U.Bots.chat.of(st, id, B.Bot.thread(bot))`: the thread a bot's Chat tab shows (its own; the uberbot's is where the conversation runs now,
/// `Ub.chat`: this machine's while it holds the lease or no other machine's thread is known, which is always so until the uber sync lands).
/// None for a bot that is not known.
pub async fn chat_thread(hub: &Arc<Hub>, bot: &str) -> Option<String> {
    let b = of(hub)?; let w = b.world().await;
    let own = w.bot(bot).map(|b| b.thread.clone()).unwrap_or_default();
    let th = if w.uber_is(bot) { w.bot(bot).map(|b| b.thread.clone()).unwrap_or_default() } else { own };
    (!th.is_empty()).then_some(th)
}
/// `BU.tab.default(u, bot)`: a client with the Mailroom (`local.mailroom` on) opens Chat; one without (the phones) opens the uberbot on its Status.
pub async fn default_tab(hub: &Arc<Hub>, bot: &str, mailroom: bool) -> String {
    if mailroom { return "chat".into() }
    match of(hub) { Some(b) if b.world().await.uber_is(bot) => "status".into(), _ => "chat".into() }
}

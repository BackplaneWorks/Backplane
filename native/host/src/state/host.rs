//! The state policies in the Rust hub: a thin shell around `policy::Core`. The Observer sends each committed batch to one task over a channel
//! (it never blocks the writer and takes no lock); the Ticker asks the same task for the tick's events; the task folds, decides and commits
//! what it derives in order. The log is read at start in `spawn_blocking`, after the listener is up; batches committed meanwhile wait in the
//! channel. Declared from main.rs as `#[path = "state/host.rs"] mod statehost;`, started with `statehost::start(hub.clone())` next to files::start.
use crate::api::{Fut, Observer, Ticker};
use crate::hub::Hub;
use crate::state::fam::policy::{self, Core, Msg, Sink};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

struct Obs { tx: mpsc::UnboundedSender<Msg> }
impl Observer for Obs {
    fn committed(&self, _hub: &Arc<Hub>, events: &[Value], first_seq: u32) { let _ = self.tx.send(Msg::Batch(first_seq as u64, events.to_vec())); }
}
struct Tick { tx: mpsc::UnboundedSender<Msg> }
impl Ticker for Tick {
    fn tick<'a>(&'a self, _hub: &'a Arc<Hub>, now_s: u64) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let (tx, rx) = oneshot::channel();
            if self.tx.send(Msg::Tick(now_s, tx)).is_err() { return vec![] }
            rx.await.unwrap_or_default()
        })
    }
}

/// the lean fold of the whole log, on a blocking thread
fn boot(hub: &Hub) -> Core {
    let mut core = Core::new();
    let len = hub.store.len();
    let mut at = 0usize;
    while at < len {
        let to = (at + 2000).min(len);
        for l in hub.store.range(at, to).unwrap_or_default() { core.fold_line(&l); }
        at = to;
    }
    core
}

/// What other modules ask of the policies. `read` is Auto.read: the person opened a thread's newest reply (the view and desk-look
/// handlers call it, then commit the events it returns with the same batch as their own effect).
pub struct Policies { tx: mpsc::UnboundedSender<Msg> }
impl Policies {
    pub async fn read(&self, thread: &str, now_s: u64) -> Vec<Value> {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(Msg::Read(thread.to_string(), now_s, tx)).is_err() { return vec![] }
        rx.await.unwrap_or_default()
    }
}

pub fn start(hub: Arc<Hub>) -> Arc<Policies> {
    let (tx, rx) = mpsc::unbounded_channel();
    hub.register_observer(Arc::new(Obs { tx: tx.clone() }));
    hub.register_ticker(Arc::new(Tick { tx: tx.clone() }));
    tokio::spawn(async move {
        let h = hub.clone();
        let Ok(core) = tokio::task::spawn_blocking(move || boot(&h)).await else { return };
        let sink: Sink = Box::new(move |evs: Vec<Value>| { let hub = hub.clone(); Box::pin(async move { hub.commit_many(&evs).await.is_ok() }) });
        policy::run(rx, core, sink).await;
    });
    Arc::new(Policies { tx })
}

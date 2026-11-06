//! The task that keeps the checkpoints (ckpt.rs) fresh while the hub runs.
use crate::ckpt::*;
use std::sync::Arc;

/// Keeps the checkpoints fresh while the hub runs: the line index and the read model as they are (when no write is between them and the log),
/// the policies' fold brought forward from its own file, and every registered fold. The first round waits for the hub to settle; a round that
/// finds nothing new writes nothing.
pub fn start(hub: Arc<crate::hub::Hub>) {
    tokio::spawn(async move {
        let mut wait = 10u64;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(wait)).await; wait = 120;
            let h = hub.clone();
            let _ = tokio::task::spawn_blocking(move || round(&h)).await;
        }
    });
}
fn round(hub: &crate::hub::Hub) {
    let t = std::time::Instant::now();
    let n = hub.store.len();
    if let Err(e) = hub.store.save_index() { eprintln!("checkpoint: index not saved: {e:#}") }
    { let m = hub.model.blocking_lock(); match m.save(&hub.store) { Ok(_) => {} Err(e) => eprintln!("checkpoint: model not saved: {e:#}") } }
    crate::statehost::refresh_state(&hub.store, 5_000);
    run_savers(&hub.store);
    eprintln!("checkpoint: saved through {n} lines in {} ms", t.elapsed().as_millis());
}


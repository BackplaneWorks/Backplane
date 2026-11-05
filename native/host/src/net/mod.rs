//! The net lane: pairing and /auth/*, QR, devices, push, far threads, peers.
//!
//! Security properties (owner trust, pairing proofs, invitations, device credentials) live in `owner.rs`
//! and `pairing.rs` as the same rules core/auth.bend and server/effects/pairing.js state, each with a
//! test named after its law. They are never weaker than the Bend rule: a case the Bend rule refuses is
//! refused here.
//!
//! Seams other lanes provide (asked of the core lead, see docs/gpui/HUB-API.md "net"):
//! * routes that do their own credential check (`/auth/enroll`, `/bots/pair`, `/far/*`, `/hook/*`) and
//!   take POST: `routes::auth` is complete and tested without the hub's GET-only gate.
//! * `Auth` asking `pairing::Registry::resolve` instead of reading devices-v1.json once.
pub mod botnet;
pub mod devices;
pub mod far;
pub mod farsync;
pub mod farstore;
pub mod listener;
pub mod machines;
pub mod origin;
pub mod owner;
pub mod pairing;
pub mod peerhttp;
pub mod peering;
pub mod peers;
pub mod qr;
pub mod routes;
pub mod sign;
pub mod start;
pub mod tailnet;
pub use start::routes;

/// The mirrors: open them off the async workers, tell the hub (`FarView`), show them to the presenter (state and the last
/// message of each thread, not their history), accept `/far/push`, and pull every five minutes from each of the owner's
/// machines (anti-entropy; a gap or another epoch pulls at once).
pub fn farhttp_start(hub: std::sync::Arc<crate::hub::Hub>, peering: std::sync::Arc<peering::Peering>) {
    tokio::spawn(async move {
        let home = hub.cfg.home.clone();
        let Ok(Ok(store)) = tokio::task::spawn_blocking(move || farstore::FarStore::open(&home)).await else { eprintln!("backplane: could not read the far mirrors"); return };
        hub.set_far(store.clone());
        for l in store.links() { let ev = store.replay_events(&l); if !ev.is_empty() { hub.fold_mirror(&ev).await } }
        let sync = farsync::FarSync::new(store, peering.clone());
        hub.register_http(std::sync::Arc::new(farsync::FarRoutes(sync.clone())));
        loop {
            let links: Vec<String> = peering.table.with(|p| p.live().into_iter().map(|x| x.id).collect());
            for l in links { sync.pull(&hub, &l).await; }
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
        }
    });
}

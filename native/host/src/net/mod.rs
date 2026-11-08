//! The net lane: pairing and /auth/*, QR, devices, push, far threads, peers.
//!
//! Security properties (owner trust, pairing proofs, invitations, device credentials) live in `owner.rs`
//! and `pairing.rs` as the same rules core/auth and server/effects/pairing.js state, each with a
//! test named after its law. They are never weaker than rule: a case rule refuses is
//! refused here.
//!
//! Seams other lanes provide (asked of the core lead, see docs/HUB-API.md "net"):
//! * routes that do their own credential check (`/auth/enroll`, `/bots/pair`, `/far/*`, `/hook/*`) and
//!   take POST: `routes::auth` is complete and tested without the hub's GET-only gate.
//! * `Auth` asking `pairing::Registry::resolve` instead of reading devices-v1.json once.
#[cfg(test)]
mod copy_outs;
pub mod botnet;
pub mod devices;
pub mod far;
pub mod feed;
pub mod farsync;
pub mod farview;
pub mod farstore;
pub mod listener;
pub mod machinespage;
pub mod machines;
pub mod origin;
pub mod outbox;
pub mod owner;
pub mod pairflow;
pub mod pairing;
pub mod peerrpc;
pub mod peerhttp;
pub mod peering;
pub mod peers;
pub mod qr;
pub mod routes;
pub mod rpc;
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
        hub.set_mirror_feed(std::sync::Arc::new(farsync::Refeed(store.clone())));
        for l in store.links() {
            // (read off the async workers, and sent a chunk at a time)
            let s = store.clone(); let link = l.clone();
            let _g = hub.mirror_gate().await; // (snapshot and fold together, against the pushes and a rebuild)
            let ev = tokio::task::spawn_blocking(move || s.replay_events(&link)).await.unwrap_or_default();
            farsync::fold_held(&hub, &l, &ev).await;
        }
        { let p = peering.clone(); hub.mirrors().set_names(std::sync::Arc::new(move || farsync::shown_names(&p))); } // (the machines the sidebar shows)
        let sync = farsync::FarSync::new(store, peering.clone());
        // the owner's side: the feed index (built from the log off the workers) answers pulls and drives the pushes
        let feed = feed::Feed::new();
        let rx = feed.pushes();
        feed.start(&hub).await;
        let _ = sync.feed.set(feed.clone());
        tokio::spawn(outbox::run(hub.clone(), peering.clone(), outbox::Outbox::new(), rx));
        hub.register_http(std::sync::Arc::new(farsync::FarRoutes(sync.clone())));
        // /far/rpc: asking (a client's request about a thing elsewhere goes there) and answering; the bots lane's owner.mail transport
        hub.register_http(std::sync::Arc::new(rpc::RpcRoutes(sync.clone())));
        hub.set_far_router(std::sync::Arc::new(rpc::Router::new(sync.clone())));
        crate::mirror::watch_origins(hub.clone()); // (each linked machine's projects' origins, for one sidebar project per repository)
        { // (the bots start on their own; the transport is handed over once they exist, looked for for ten minutes)
            let (hub, sync) = (hub.clone(), sync.clone());
            tokio::spawn(async move {
                for _ in 0..300 {
                    if let Some(b) = crate::bots::api::of(&hub) { b.set_owner_link(std::sync::Arc::new(rpc::Link { sync, hub: std::sync::Arc::downgrade(&hub), copy: crate::copy::on() })); b.far_start(); return }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            });
        }
        loop {
            let links: Vec<String> = peering.table.with(|p| p.live().into_iter().map(|x| x.id).collect());
            for l in links { sync.pull(&hub, &l).await; }
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
        }
    });
}

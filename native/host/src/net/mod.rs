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
pub mod machines;
pub mod owner;
pub mod pairing;
pub mod peers;
pub mod qr;
pub mod routes;
pub mod sign;
pub mod start;
pub mod tailnet;
pub use start::routes;

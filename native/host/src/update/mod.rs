//! The hub's side of updating itself from GitHub Releases: what may be installed (`rel`), and finding, fetching and staging it for the guard (`run`). The guard owns the swap, the health check and the fallback.
pub mod rel;
pub mod run;
pub mod svc;

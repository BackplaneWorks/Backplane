//! The interfaces other streams build on. Stable names; bodies may grow.
//!
//! * `agents` (native/host/src/agents/) registers an `RpcHandler` for `turn.start`, `turn.interrupt`,
//!   ... and calls `Hub::commit_many` for every event an agent turn produces, `Hub::live_delta` /
//!   `Hub::live_end` for streamed text that is not yet an event.
//! * `files` (native/host/src/files/, kicad/) registers `RpcHandler`s (`files.dir`, `kicad_ipc`...) and
//!   `HttpHandler`s (`/img`, `/plot/info`, `/files/get`...).
//! * Everyone reads the world through `Hub::model()` (threads, projects, entries) and listens with
//!   `Hub::subscribe()`.
use crate::{auth::Req, hub::Hub};
use serde_json::Value;
use std::{future::Future, pin::Pin, sync::Arc};

pub type Fut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What a handler knows of the caller.
pub struct Caller { pub gen: String, pub client: u64 }

/// One WebSocket RPC family. `call` returns the frames to send back to the caller, in order, normally
/// one `{"t":"reply","id":..,"ok":..}`; it may take its time (the connection keeps reading meanwhile in a
/// later milestone; today replies are in request order).
pub trait RpcHandler: Send + Sync {
    /// Every method name this handler answers, exact match (`"turn.start"`, `"files.dir"`).
    fn methods(&self) -> &'static [&'static str];
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, method: &'a str, params: &'a Value) -> Fut<'a, Vec<Value>>;
}

pub struct HttpReply { pub status: &'static str, pub ctype: String, pub extra: String, pub body: Vec<u8> }
impl HttpReply {
    pub fn ok(ctype: &str, body: Vec<u8>) -> HttpReply { HttpReply { status: "200 OK", ctype: ctype.into(), extra: String::new(), body } }
    pub fn text(status: &'static str, body: &str) -> HttpReply { HttpReply { status, ctype: "text/plain; charset=utf-8".into(), extra: String::new(), body: body.as_bytes().to_vec() } }
}

/// One family of HTTP GET routes. The hub checks the pairing token (`Auth::allowed`) before `call` unless
/// `public()` says the route needs none (`/hello` only).
pub trait HttpHandler: Send + Sync {
    /// Route prefixes it answers (`"/img"`, `"/plot/info"`).
    fn routes(&self) -> &'static [&'static str];
    /// DEFAULT-DENY: false means the hub checks the pairing credential (`Auth::allowed`) before `call`. A handler that
    /// returns true takes the whole decision itself (enrollment, a peer's signed request): it must refuse every request
    /// that does not prove itself, and the hub gives it no credential check at all. Only `/hello` is public otherwise.
    fn own_auth(&self) -> bool { false }
    /// `call` with the connection's id (Pairing.resolve remembers a connection's device by it). Handlers that do not
    /// need it implement `call` only.
    fn call_with<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req, _client: u64) -> Fut<'a, HttpReply> { self.call(hub, req) }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply>;
}

/// A periodic job (the minute tick: autosettle, alerts, told, summaries, liveness, star nudge...). `tick` runs every
/// `every_s` seconds on the hub's runtime, never inside the commit path; the events it returns are committed together
/// (one batch). Register with `Hub::register_ticker`.
pub trait Ticker: Send + Sync {
    fn every_s(&self) -> u64 { 60 }
    fn tick<'a>(&'a self, hub: &'a Arc<Hub>, now_s: u64) -> Fut<'a, Vec<Value>>;
}

/// Sees every committed batch, in log order, right after the model applied it (so `hub.model()` already holds it) and
/// before the committer is answered. `first_seq` is the 1-based log line of `events[0]`. It runs on the writer task: do
/// not block, and never commit from it (spawn a task that does). Register with `Hub::register_observer`.
pub trait Observer: Send + Sync {
    fn committed(&self, hub: &Arc<Hub>, events: &[Value], first_seq: u32);
}

/// Paired devices and the connections they hold (net lane: `net::pairing::Registry`). `Auth` asks it instead of reading
/// devices-v1.json once; a /ws connection is resolved on upgrade, checked `active` before anything is published, and
/// forgotten on close.
pub trait Pairing: Send + Sync {
    /// the credential is the hub token or a live paired device's (no side effects)
    fn holds(&self, token: &str) -> bool;
    /// pairing.js resolve: the credential that holds for this connection, remembered for it, compared by the caller with the one presented:
    /// the hub token for an empty or unknown one (so callers must compare, never test for non-empty); a held device's token for its own
    fn resolve(&self, token: &str, client: u64, route: &str) -> String;
    /// false once the connection's device was revoked
    fn active(&self, client: u64) -> bool;
    fn forget(&self, client: u64);
}

/// What an /auth route did: the answer, the connections to close (a revoke), a link to remove.
pub struct AuthOutcome { pub reply: HttpReply, pub close: Vec<u64>, pub unpeer: Option<String> }
/// `/auth/enroll`, `/auth/invitations|devices|revoke|known`: each does its own credential check (net lane). `handle`
/// answers None for any other route, which then goes the ordinary way (credential first). Whatever it answers for an
/// /auth route is final, so it must deny by default.
pub trait AuthRoute: Send + Sync {
    fn handle(&self, req: &Req, client: u64) -> Option<AuthOutcome>;
    /// A used machine invitation was revoked: remove the link it made (Bend's uninvite). Called by the hub with `AuthOutcome.unpeer`.
    fn unpeer(&self, _link: &str) {}
}

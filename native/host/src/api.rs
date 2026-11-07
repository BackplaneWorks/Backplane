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
    /// True for a POST route whose body streams (an upload): the hub then calls `call_body` instead of `call`, after the credential check
    /// (or none, with `own_auth`) and a size check, and reads no body itself.
    fn takes_body(&self) -> bool { false }
    /// The streaming body: `body.len` is the Content-Length (the hub refuses a missing one or a chunked one with 411 and nothing else:
    /// refuse a size you will not take with your own answer BEFORE `proceed()`); `body` reads exactly that many bytes and no more, and
    /// fails with TimedOut when no byte arrives for `Body::IDLE`; `body.proceed().await` answers `Expect: 100-continue` (call it once you
    /// accept the size). The connection closes after the reply.
    fn call_body<'a>(&'a self, _hub: &'a Arc<Hub>, _req: &'a Req, _body: Body<'a>) -> Fut<'a, HttpReply> { Box::pin(async { HttpReply::text("405 Method Not Allowed", "this route takes no body") }) }
    /// The most body bytes read for this route when it is `own_auth` (default `auth::BODY_MAX`, 1 MiB). Over it: 413 before anything is read.
    fn body_max(&self) -> usize { crate::auth::BODY_MAX }
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
    /// A used machine invitation was revoked: remove the link it made. Called by the hub with `AuthOutcome.unpeer`.
    fn unpeer(&self, _link: &str) {}
}

/// A ceiling a streaming handler may use for its own refusal. The hub does NOT enforce it: the handler decides every size (the file goes to disk, and its limit is a setting).
pub const STREAM_MAX: u64 = 4000 << 20;
/// A request body read straight off the connection: at most `len` bytes. `AsyncRead` ends at `len`.
pub struct Body<'a> { pub len: u64, io: tokio::io::Take<&'a mut tokio::net::TcpStream>, expect: bool, idle: Pin<Box<tokio::time::Sleep>> }
impl<'a> Body<'a> {
    /// No byte for this long ends a read with TimedOut (a stalled upload does not hold its connection and half-written file).
    pub const IDLE: std::time::Duration = std::time::Duration::from_secs(60);
    pub fn new(s: &'a mut tokio::net::TcpStream, len: u64, expect: bool) -> Body<'a> { Body { len, io: tokio::io::AsyncReadExt::take(s, len), expect, idle: Box::pin(tokio::time::sleep(Self::IDLE)) } }
    /// Writes `HTTP/1.1 100 Continue` once, if the client sent `Expect: 100-continue`.
    pub async fn proceed(&mut self) -> std::io::Result<()> {
        if !std::mem::take(&mut self.expect) { return Ok(()) }
        tokio::io::AsyncWriteExt::write_all(&mut **self.io.get_mut(), b"HTTP/1.1 100 Continue\r\n\r\n").await
    }
    /// Bytes still unread.
    pub fn left(&self) -> u64 { self.io.limit() }
}
impl tokio::io::AsyncRead for Body<'_> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &mut tokio::io::ReadBuf<'_>) -> std::task::Poll<std::io::Result<()>> {
        use std::task::Poll;
        let before = buf.filled().len();
        match Pin::new(&mut self.io).poll_read(cx, buf) {
            Poll::Ready(r) => { if buf.filled().len() > before { self.idle.as_mut().reset(tokio::time::Instant::now() + Self::IDLE); } Poll::Ready(r) }
            Poll::Pending => match self.idle.as_mut().poll(cx) { Poll::Ready(()) => Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "upload stalled"))), Poll::Pending => Poll::Pending },
        }
    }
}

/// Runs on the writer task for every batch about to be written, in commit order, AFTER the secret mask (so the policy never sees or
/// copies a secret). `first_seq` is the 1-based log line the first returned event will get. Returns the events to write: normally
/// `events` plus policy events appended (quiet marks, message metadata, wakes, housekeeping); it may drop or replace policy-owned
/// events of the batch. SYNCHRONOUS and fast: no await, no IO, no commit_many; it may take a std Mutex it shares with a Ticker, never
/// the writer's own locks. A panic leaves the batch as it came. Several are applied in registration order; the Observer still sees the
/// batch as written. Each caller's `commit_many` answer counts the events actually written for its commit.
pub trait Transform: Send + Sync {
    fn before_commit(&self, first_seq: u64, events: Vec<Value>) -> Vec<Value>;
    /// The batch returned above could not be written (disk full, closed log): the policy must resync / mark its copy stale.
    fn commit_failed(&self, first_seq: u64);
}

/// Threads mirrored from the owner's other machines: NOT in this hub's log or `Model` (hub reads and writes the same
/// events.jsonl; a mirror there would fold as local ids). Their events live in the net lane's per-link store and reach the
/// presenter through `Hub::fold_mirror` (renamed to `<link>~<id>`); pages of a mirrored thread come from here.
/// Gives the presenter the mirrors again after it lost them (a restart, or `Hub::forget_mirror`): the net lane folds every mirror's
/// events (`Hub::fold_mirror`) here. Called at the end of `resync_presenter`, so a crash restart re-feeds them too.
pub trait MirrorFeed: Send + Sync {
    fn replay<'a>(&'a self, hub: &'a Arc<Hub>) -> Fut<'a, ()>;
}
/// A client's request that names a thing on a linked machine (`<link>~<id>`, or `on`) goes to that machine instead (net lane, `/far/rpc`):
/// consulted first by `Hub::request`. `wants` decides from the method and params; `call` answers (the owner's reply with the caller's id).
pub trait FarRouter: Send + Sync {
    fn wants(&self, method: &str, params: &Value) -> bool;
    /// Answers at once with the frames to send now (an error, or none when the answer comes later through `Hub::push(client, ..)`): the connection never waits on another machine.
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, client: u64, id: &'a Value, method: &'a str, params: &'a Value) -> Fut<'a, Vec<Value>>;
    /// An agent's (or a bot's) tool call about another machine: `tool` with `args` runs there for the thread `from` (its id here), as a signed `mcp.call`.
    /// Ok((ok, body)) is the tool's own answer; Err when the machine is not one of the owner's, away, or refused.
    fn mcp<'a>(&'a self, _hub: &'a Arc<Hub>, _link: &'a str, _from: &'a str, _tool: &'a str, _args: &'a Value) -> Fut<'a, Result<(bool, Value), String>> { Box::pin(async { Err("no machine is linked".to_string()) }) }
    /// The machines this hub may act on, as the owner's own: (link id, name, address).
    fn machines(&self) -> Vec<(String, String, String)> { vec![] }
}
/// The owning side of a tool call made from another machine (`mcp.call`): the agents lane runs the tool for `from` (an id like "<link>~<thread>").
pub trait FarMcp: Send + Sync {
    fn serve<'a>(&'a self, link: &'a str, from: &'a str, tool: &'a str, args: &'a Value) -> Fut<'a, (bool, Value)>;
}
pub trait FarView: Send + Sync {
    /// whether `thread` (an id like "<link>~<id>") is a mirrored thread
    fn has(&self, thread: &str) -> bool;
    /// the page as `Model::page` gives it for a local thread: rows older than ordinal `before` (0: newest), at most `limit` plus lookahead
    fn page(&self, thread: &str, before: u64, limit: usize) -> Option<crate::model::Page>;
    /// the newest rows up to the latest human prompt and assistant text (`Model::latest_context`)
    fn latest_context(&self, thread: &str) -> Vec<crate::model::Row>;
}

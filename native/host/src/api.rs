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
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply>;
}

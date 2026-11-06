//! `/far/rpc` (server.bend `Hub.far.rpc`, `Far.rpc.go`, core/far.bend `Fr.*`, core/fmcp.bend): a request about a thing on another of the owner's machines.
//!
//! Asking side (`Router`, installed with `Hub::set_far_router`): a client's request whose params name `<link>~<id>` (or `on`) goes to that machine as a signed
//! POST and the owner's reply comes back with the caller's request id and the thread ids made ours. It goes only to one of the owner's own machines with sync on
//! (`Far.own.id`), and only the methods of `Fr.methods`; anything else is told "that is done on its own machine" (law far_no_delete: thread.delete is not one).
//!
//! Owning side (`RpcRoutes`): the same signature check and owner-only gate as `/far/log`; then `owner.mail` goes to the bots lane's `Bots::owner_received`; a
//! request of `Fr.methods` runs as a local client's request, only when it is about something this hub shares (`Fr.allowed`, `Feed::allowed`); everything else is
//! refused 403. NOT ported here: `mcp.call` (remote agents' tools, `Fm.serve`), `uber.sync`, and the viewers' reads: they answer 501 and say so.
//!
//! `OwnerLink` (the bots lane's transport for `owner.mail`): a signed POST of the request to a linked machine, only to one of the owner's own.
use super::{far, farsync::FarSync, peering::PReq};
use crate::{api::{Fut, FarRouter, HttpHandler, HttpReply}, bots::mail::owner::OwnerLink, hub::{Conn, Hub}};
use serde_json::{json, Value};
use std::sync::{atomic::{AtomicU64, Ordering::Relaxed}, Arc};

static NONCE: AtomicU64 = AtomicU64::new(0);
fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }
/// `Fr.nonced`: a body of its own, so two equal requests in one second are two signed bodies, not a replay of one
fn nonced(body: &str) -> String { format!("{{\"n\":{},{}", Value::String(format!("{}.{}", now(), NONCE.fetch_add(1, Relaxed))), &body[1..]) }

/// One signed POST to `link`'s /far/rpc: (status, body); status 0 when it could not be made (no address, away, a copy).
async fn post(sync: &FarSync, hub: &Arc<Hub>, link: &str, body: &str) -> (u16, String) {
    let url = sync.peering.table.with(|p| p.url(link));
    if url.is_empty() { return (0, String::new()) }
    let r = super::botnet::to_peer(&hub.cfg.home, &url, link, now(), "/far/rpc", true, &nonced(body)).await;
    (r.code, r.body)
}
fn err(id: &Value, e: &str) -> Vec<Value> { vec![json!({"t":"reply","id":id,"ok":false,"error":e})] }

#[cfg(test)]
mod tests;

/// Requests to other machines one connection may have waiting at once (more are told so, at once).
const IN_FLIGHT: usize = 8;
pub struct Router { sync: Arc<FarSync>, copy: bool, waiting: Arc<std::sync::Mutex<std::collections::HashMap<u64, usize>>> }
impl Router {
    pub fn new(sync: Arc<FarSync>) -> Router { Router::with_copy(sync, crate::copy::on()) }
    /// `copy`: a hub on a copy of a log reaches no other machine: a request about a thing elsewhere is answered with an error here and no connection is made
    pub fn with_copy(sync: Arc<FarSync>, copy: bool) -> Router { Router { sync, copy, waiting: Default::default() } }
}
impl FarRouter for Router {
    fn wants(&self, _m: &str, p: &Value) -> bool { !far::params_link(p).is_empty() }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, client: u64, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let l = far::params_link(p);
            if self.copy { return err(id, "a hub on a copied log does not reach other machines") }
            // (Far.rpc.pick: only the owner's own machines with sync on, and only what Fr.methods lets through)
            if !self.sync.own(&l) || !far::may(m) { return err(id, "that is done on its own machine") }
            { let mut w = self.waiting.lock().unwrap(); let n = w.entry(client).or_insert(0); if *n >= IN_FLIGHT { return err(id, "too many requests to other machines are waiting") } *n += 1; }
            // (Far.rpc.job): the request is out on a task of its own and the answer is pushed when it comes; this connection goes on meanwhile
            let (sync, hub, waiting, id, m, p) = (self.sync.clone(), hub.clone(), self.waiting.clone(), id.clone(), m.to_string(), p.clone());
            tokio::spawn(async move {
                let body = json!({"id": 0, "m": m, "p": far::params_local(&p, &l)}).to_string();
                let (code, text) = post(&sync, &hub, &l, &body).await;
                let frame = if code == 200 {
                    far::reply_on(serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({"t":"reply","ok":false,"error":"invalid reply from that machine","id":0})), &id, &l)
                } else if code == 0 { err(&id, "that machine is away").remove(0) } else { err(&id, &format!("that machine did not take it: {}", text.chars().take(120).collect::<String>())).remove(0) };
                { let mut w = waiting.lock().unwrap(); if let Some(n) = w.get_mut(&client) { *n -= 1; if *n == 0 { w.remove(&client); } } }
                hub.push(client, frame);
            });
            vec![]
        })
    }
}

/// The bots lane's transport for `owner.mail` (a signed POST to a linked machine); only to one of the owner's own (the request carries mail text).
pub struct Link { pub sync: Arc<FarSync>, pub hub: std::sync::Weak<Hub>, pub copy: bool }
impl OwnerLink for Link {
    fn post<'a>(&'a self, link: &'a str, body: String) -> Fut<'a, Option<(u16, String)>> {
        Box::pin(async move {
            if self.copy { return None }
            let hub = self.hub.upgrade()?;
            if !self.sync.own(link) { return None }
            match post(&self.sync, &hub, link, &body).await { (0, _) => None, r => Some(r) }
        })
    }
}

pub struct RpcRoutes(pub Arc<FarSync>);
impl HttpHandler for RpcRoutes {
    fn routes(&self) -> &'static [&'static str] { &["/far/rpc"] }
    fn own_auth(&self) -> bool { true }
    fn body_max(&self) -> usize { 256 << 10 }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a crate::auth::Req) -> Fut<'a, HttpReply> {
        Box::pin(async move { if req.method != "POST" { return HttpReply::text("404 Not Found", "not found") } serve(&self.0, hub, &super::peerhttp::PeerRoutes::preq(req)).await })
    }
}
fn json_reply(v: &Value) -> HttpReply { HttpReply { status: "200 OK", ctype: "application/json".into(), extra: String::new(), body: v.to_string().into_bytes() } }

/// POST /far/rpc on the owning hub
pub async fn serve(sync: &Arc<FarSync>, hub: &Arc<Hub>, r: &PReq) -> HttpReply { serve_with(crate::copy::on(), sync, hub, r).await }
/// A hub on a copy of a log takes nothing here: `turn.start` would be a real agent in a real folder, on a copy that is not the person's machine to act for.
/// (Default-deny, stated: not left to the copy holding no peer secrets.)
pub async fn serve_with(copy: bool, sync: &Arc<FarSync>, hub: &Arc<Hub>, r: &PReq) -> HttpReply {
    if copy { return HttpReply::text("403 Forbidden", "a hub on a copied log does not take /far/rpc") }
    let link = match sync.peering.signed_in("far-rpc", r) { Ok(p) => p, Err(e) => return e };
    if !sync.own(&link) { return HttpReply::text("403 Forbidden", "threads are shared with the owner's own machines") }
    let Ok(o) = serde_json::from_slice::<Value>(&r.body) else { return HttpReply::text("400 Bad Request", "not JSON") };
    let m = o["m"].as_str().unwrap_or("");
    if m == "owner.mail" {
        let Some(bots) = crate::bots::api::of(hub) else { return HttpReply::text("503 Service Unavailable", "bots are not running") };
        let (code, body) = bots.owner_received(&link, &o).await;
        return HttpReply { status: status(code), ctype: "text/plain".into(), extra: String::new(), body: body.into_bytes() };
    }
    let p = o.get("p").cloned().unwrap_or(Value::Null);
    if m == "uber.sync" { return HttpReply::text("501 Not Implemented", "uber.sync is not ported on this hub yet") }
    if m == "mcp.call" {
        return if MCP_REMOTE.contains(&p["tool"].as_str().unwrap_or("")) { HttpReply::text("501 Not Implemented", "mcp.call is not ported on this hub yet") } else { HttpReply::text("403 Forbidden", "not something this machine shares") };
    }
    let Some(feed) = sync.feed.get() else { return HttpReply::text("503 Service Unavailable", "not ready") };
    if !feed.allowed(m, &p) { return HttpReply::text("403 Forbidden", "not something this machine shares") }
    // as a local client's request (Far.client), here and never onward: the reply frame is the answer
    let mut c = Conn { sent: vec![], client: hub.fresh_client(), gen: "far".into(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 };
    let out = hub.request_here(&mut c, &json!({"id": 0, "m": m, "p": p})).await;
    match out.into_iter().find(|v| v["t"] == "reply") { Some(v) => json_reply(&v), None => HttpReply::text("500 Internal Server Error", "no answer") }
}
/// `Fm.names` the owner's machines may call here (Fm.remote)
const MCP_REMOTE: &[&str] = &["thread_send", "thread_wait_now", "thread_interrupt", "thread_update", "thread_list", "thread_launch", "machine_list", "thread_read", "thread_handoff", "plane_read", "launch_report"];
fn status(code: u16) -> &'static str { match code { 200 => "200 OK", 400 => "400 Bad Request", 403 => "403 Forbidden", 409 => "409 Conflict", 503 => "503 Service Unavailable", _ => "500 Internal Server Error" } }

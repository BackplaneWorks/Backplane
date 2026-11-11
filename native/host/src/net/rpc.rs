//! `/far/rpc`: a request about a thing on another of the owner's machines.
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
use super::{far, farsend::{self, Outbox}, farsync::FarSync, peering::PReq};
use crate::{api::{Fut, FarRouter, HttpHandler, HttpReply}, bots::mail::owner::OwnerLink, hub::{Conn, Hub}};
use serde_json::{json, Value};
use std::sync::{atomic::{AtomicU64, Ordering::Relaxed}, Arc};

static NONCE: AtomicU64 = AtomicU64::new(0);
fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) }
/// `Fr.nonced`: a body of its own, so two equal requests in one second are two signed bodies, not a replay of one
pub(super) fn nonced(body: &str) -> String { format!("{{\"n\":{},{}", Value::String(format!("{}.{}", now(), NONCE.fetch_add(1, Relaxed))), &body[1..]) }

/// One signed POST to `link`'s /far/rpc: (status, body); status 0 when it could not be made (no address, away, a copy).
pub(super) async fn post(sync: &FarSync, hub: &Arc<Hub>, link: &str, body: &str) -> (u16, String) {
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
/// Where a window's viewers are drawn: client -> (the link of the machine it watches, that watch's root over there).
type Watching = Arc<std::sync::Mutex<std::collections::HashMap<u64, (String, String)>>>;
pub struct Router { sync: Arc<FarSync>, copy: bool, waiting: Arc<std::sync::Mutex<std::collections::HashMap<u64, usize>>>, watching: Watching, pulling: Arc<std::sync::Mutex<std::collections::HashSet<u64>>>,
    /// a person's messages to threads elsewhere that have not reached their machine yet (net/farsend.rs), and whether its worker runs
    pub out: Arc<Outbox>, working: std::sync::atomic::AtomicBool }
impl Router {
    pub fn new(sync: Arc<FarSync>) -> Router { Router::with_copy(sync, crate::copy::on()) }
    /// `copy`: a hub on a copy of a log reaches no other machine: a request about a thing elsewhere is answered with an error here and no connection is made
    pub fn with_copy(sync: Arc<FarSync>, copy: bool) -> Router { Router::with_outbox(sync, copy, Outbox::open(None)) }
    pub fn with_outbox(sync: Arc<FarSync>, copy: bool, out: Arc<Outbox>) -> Router { Router { sync, copy, waiting: Default::default(), watching: Default::default(), pulling: Default::default(), out, working: Default::default() } }
    /// Start the worker that posts the held messages (once): at startup, for what a restart left held, and on the first message held.
    pub fn work(&self, hub: &Arc<Hub>) {
        if self.copy || self.working.swap(true, Relaxed) { return }
        tokio::spawn(send_held(self.sync.clone(), Arc::downgrade(hub), self.out.clone()));
    }
    /// The machine a window's viewer read goes to: the one named by its params, else the one its watch is on (a read about a piece it tapped names only the plot's key).
    fn viewer_link(&self, client: u64, m: &str, p: &Value) -> String {
        let named = far::params_link(p);
        if !named.is_empty() || m == "kicad.watch" { return named }
        let w = self.watching.lock().unwrap();
        let Some((link, root)) = w.get(&client) else { return String::new() };
        let on_root = p["key"].as_str().map_or(false, |k| k.split('|').nth(1) == Some(root.as_str())) || p["root"].as_str() == Some(root.as_str());
        if on_root { link.clone() } else { String::new() }
    }
}
/// Forward what the owning machine pushed to this window (plot frames and the like) until the window leaves or looks elsewhere.
fn pull_loop(sync: Arc<FarSync>, hub: Arc<Hub>, watching: Watching, pulling: Arc<std::sync::Mutex<std::collections::HashSet<u64>>>, client: u64, link: String) {
    tokio::spawn(async move {
        let body = json!({"id": 0, "m": "viewer.pull", "p": {"sub": client}}).to_string();
        let mut failed = 0;
        loop {
            let here = watching.lock().unwrap().get(&client).map_or(false, |(l, _)| *l == link);
            if !here || !hub.is_attached(client) { break }
            let (code, text) = post(&sync, &hub, &link, &body).await;
            if code != 200 { failed += 1; if failed > 4 || code == 403 || code == 404 { break } tokio::time::sleep(std::time::Duration::from_secs(2)).await; continue }
            failed = 0;
            let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            for f in super::farview::frames_of(&v) { match f { crate::hub::Out::Value(j) => { hub.push(client, term_here(j, &link)); } crate::hub::Out::Raw(b) => { hub.push_raw(client, b); } crate::hub::Out::Close => {} } }
            if v["gone"] == true { break }
        }
        pulling.lock().unwrap().remove(&client);
        let _ = post(&sync, &hub, &link, &json!({"id": 0, "m": "viewer.drop", "p": {"sub": client}}).to_string()).await;
    });
}
/// A terminal frame of the owning machine names its thread by that machine's id: here it is `<link>~<id>`, the thread the window shows.
fn term_here(mut j: Value, link: &str) -> Value {
    if j["t"] == "term" { if let Some(t) = j["thread"].as_str().filter(|t| !far::is(t)).map(|t| far::id(link, t)) { j["thread"] = Value::String(t); } }
    j
}
/// What a request to another machine needs besides the router's checks (cheap to clone into a task).
struct Parts { sync: Arc<FarSync>, waiting: Arc<std::sync::Mutex<std::collections::HashMap<u64, usize>>>, watching: Watching, pulling: Arc<std::sync::Mutex<std::collections::HashSet<u64>>> }
impl Router {
    fn parts(&self) -> Parts { Parts { sync: self.sync.clone(), waiting: self.waiting.clone(), watching: self.watching.clone(), pulling: self.pulling.clone() } }
    /// The machine a request goes to, or the error frames to answer at once; counts it as waiting (`Parts::run` gives it back).
    /// A person's message to a thread on one of the owner's machines, answering now or away: held here and answered at once (net/farsend.rs); the worker takes it
    /// there, where it cuts into the running turn, and the thread's queue strip shows it while it has not arrived. None for anything else.
    /// A person's post in a room there (`bots.post`) is held the same way, under the room: the owner logs it once by its id (`msg`), however often it is posted.
    fn hold(&self, hub: &Arc<Hub>, id: &Value, m: &str, p: &Value) -> Option<Value> {
        if !held_method(m) || self.copy { return None }
        let l = far::params_link(p); if l.is_empty() || !self.sync.mine(&l) { return None }
        let mut local = far::params_local(p, &l); if let Some(o) = local.as_object_mut() { o.remove("on"); }
        if local["msg"].as_str().map_or(true, str::is_empty) { local["msg"] = json!(format!("{}{}-{}", if m == "bots.post" { "p" } else { "m" }, farsend::now_ms(), hub.fresh_client())); }
        let key = if m == "bots.post" { "room" } else { "thread" };
        let th = p[key].as_str().unwrap_or("").to_string();
        let msg = local["msg"].as_str().unwrap_or("").to_string();
        self.out.hold_m(&th, &l, m, &msg, local, farsend::now_ms()); self.work(hub);
        Some(json!({"t":"reply","id":id,"ok":true,"result":{"held":true}}))
    }
    fn plan(&self, client: u64, id: &Value, m: &str, p: &Value) -> Result<String, Vec<Value>> {
        let viewer = super::farview::is_viewer(m);
        let l = if viewer { self.viewer_link(client, m, p) } else { far::params_link(p) };
        if self.copy { return Err(err(id, "a hub on a copied log does not reach other machines")) }
        // (Far.rpc.pick: only the owner's own machines with sync on, and only what Fr.methods lets through; the viewers' reads are theirs)
        if !self.sync.own(&l) || !(viewer || far::may(m)) { return Err(err(id, "that is done on its own machine")) }
        { let mut w = self.waiting.lock().unwrap(); let n = w.entry(client).or_insert(0); if *n >= IN_FLIGHT { return Err(err(id, "too many requests to other machines are waiting")) } *n += 1; }
        Ok(l)
    }
}
impl Parts {
    async fn run(&self, hub: &Arc<Hub>, client: u64, id: &Value, m: &str, p: &Value, l: &str) -> Value {
        let (sync, waiting, watching, pulling) = (self.sync.clone(), self.waiting.clone(), self.watching.clone(), self.pulling.clone());
        let viewer = super::farview::is_viewer(m);
        let mut local = far::params_local(p, l); if let Some(o) = local.as_object_mut() { o.remove("on"); }
        let body = if viewer { json!({"id": 0, "m": "viewer.call", "p": {"sub": client, "m": m, "p": local}}).to_string() } else { json!({"id": 0, "m": m, "p": local}).to_string() };
        let (code, text) = post(&sync, hub, l, &body).await;
        if viewer && code == 200 && (m == "kicad.watch" || m == "term.open") {
            let ok = serde_json::from_str::<Value>(&text).map_or(false, |v| v["ok"] == true);
            // a drawing watched there, or its terminal opened: what that machine pushes for this window is pulled and forwarded (a terminal keeps the
            // drawing's root, so the reads about it still go there)
            if ok && (m == "term.open" || !p["kind"].as_str().unwrap_or("").is_empty()) {
                let root = if m == "kicad.watch" { p["root"].as_str().unwrap_or("").to_string() } else { watching.lock().unwrap().get(&client).filter(|(wl, _)| wl == l).map(|(_, r)| r.clone()).unwrap_or_default() };
                watching.lock().unwrap().insert(client, (l.to_string(), root));
                if pulling.lock().unwrap().insert(client) { pull_loop(sync.clone(), hub.clone(), watching.clone(), pulling.clone(), client, l.to_string()); }
            } else if ok { watching.lock().unwrap().remove(&client); }
        }
        let frame = if code == 200 {
            far::reply_on(serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({"t":"reply","ok":false,"error":"invalid reply from that machine","id":0})), id, l)
        } else if code == 0 { err(id, "that machine is away").remove(0) } else { err(id, &format!("that machine did not take it: {}", text.chars().take(120).collect::<String>())).remove(0) };
        { let mut w = waiting.lock().unwrap(); if let Some(n) = w.get_mut(&client) { *n -= 1; if *n == 0 { w.remove(&client); } } }
        frame
    }
}
impl FarRouter for Router {
    fn wants(&self, _m: &str, p: &Value) -> bool { !far::params_link(p).is_empty() }
    fn wants_for(&self, client: u64, m: &str, p: &Value) -> bool {
        if super::farview::is_viewer(m) {
            let l = self.viewer_link(client, m, p);
            // a watch of a project of this machine: what the window held from another machine is let go
            if l.is_empty() && m == "kicad.watch" { self.watching.lock().unwrap().remove(&client); }
            return !l.is_empty()
        }
        self.wants(m, p)
    }
    fn announce<'a>(&'a self, hub: &'a Arc<Hub>, m: &'a str, p: &'a Value) -> Fut<'a, ()> {
        Box::pin(async move {
            if self.copy { return }
            let body = json!({"id": 0, "m": m, "p": p}).to_string();
            for (link, _, _) in machines_of(&self.sync) { let _ = post(&self.sync, hub, &link, &body).await; }
        })
    }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, client: u64, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if let Some(r) = self.hold(hub, id, m, p) { return vec![r] }
            let l = match self.plan(client, id, m, p) { Ok(l) => l, Err(e) => return e };
            // (Far.rpc.job): the request is out on a task of its own and the answer is pushed when it comes; this connection goes on meanwhile
            let (me, hub, id, m, p) = (self.parts(), hub.clone(), id.clone(), m.to_string(), p.clone());
            tokio::spawn(async move { let frame = me.run(&hub, client, &id, &m, &p, &l).await; hub.far_replied(client, &frame); /* (a Diff's or a folder's answer is the view's read: its next request carries it) */ hub.push(client, frame); });
            vec![]
        })
    }
    fn ask<'a>(&'a self, hub: &'a Arc<Hub>, client: u64, id: &'a Value, m: &'a str, p: &'a Value) -> Fut<'a, Option<Value>> {
        Box::pin(async move {
            if !self.wants_for(client, m, p) { return None }
            if let Some(r) = self.hold(hub, id, m, p) { return Some(r) }
            let l = match self.plan(client, id, m, p) { Ok(l) => l, Err(mut e) => return Some(e.remove(0)) };
            Some(self.parts().run(hub, client, id, m, p, &l).await)
        })
    }
    fn mcp<'a>(&'a self, hub: &'a Arc<Hub>, link: &'a str, from: &'a str, tool: &'a str, args: &'a Value) -> Fut<'a, Result<(bool, Value), String>> {
        Box::pin(async move {
            if self.copy { return Err("a hub on a copied log does not reach other machines".into()) }
            if !self.sync.own(link) { if self.sync.mine(link) { return Err("that machine is away".into()) } return Err(format!("no machine of yours is linked as {link} (machine_list shows them)")) }
            let body = json!({"id": 0, "m": "mcp.call", "p": {"tool": tool, "from": from, "args": args}}).to_string();
            let (code, text) = post(&self.sync, hub, link, &body).await;
            match code {
                0 => Err("that machine is away".into()),
                // an older Backplane there answers but cannot run this yet (it predates mcp.call): it is not away, it needs the update
                501 => Err(format!("{OLDER}: {}", text.chars().take(160).collect::<String>())),
                200 => { let v: Value = serde_json::from_str(&text).map_err(|_| "invalid reply from that machine".to_string())?; Ok((v["ok"].as_bool().unwrap_or(false), v.get("body").cloned().unwrap_or(Value::Null))) }
                _ => Err(format!("that machine did not take it: {}", text.chars().take(160).collect::<String>())),
            }
        })
    }
    fn machines(&self) -> Vec<(String, String, String)> { machines_of(&self.sync) }
    fn raw<'a>(&'a self, hub: &'a Arc<Hub>, link: &'a str, m: &'a str, p: &'a Value) -> Fut<'a, (u16, String)> {
        Box::pin(async move {
            if self.copy || !self.sync.own(link) { return (0, String::new()) }
            post(&self.sync, hub, link, &json!({"id": 0, "m": m, "p": p}).to_string()).await
        })
    }
}
/// The worker of the held messages: each thread's oldest goes to its machine (several machines at once), the owner's answer decides (farsend::outcome), and when
/// what the strip shows changes the views are told (the window asks again) and the phones get a `queue` push for the thread.
async fn send_held(sync: Arc<FarSync>, hub: std::sync::Weak<Hub>, out: Arc<Outbox>) {
    let busy: Arc<std::sync::Mutex<Vec<String>>> = Default::default();
    let mut shown = vec![];
    loop {
        let Some(h) = hub.upgrade() else { return };
        let now = farsend::now_ms();
        let b = busy.lock().unwrap().clone();
        for item in farsend::due(&out.items(), now, &b) {
            busy.lock().unwrap().push(item.msg.clone());
            let (sync, hub2, out2, busy2) = (sync.clone(), h.clone(), out.clone(), busy.clone());
            tokio::spawn(async move {
                let body = item.body().to_string();
                let (code, text) = post(&sync, &hub2, &item.link, &body).await;
                out2.answered(&item.msg, code, &text, farsend::now_ms());
                busy2.lock().unwrap().retain(|m| *m != item.msg);
                out2.kick.notify_one();
            });
        }
        let now_shown = out.shown(farsend::now_ms());
        if now_shown != shown {
            let mut threads: Vec<String> = now_shown.iter().chain(shown.iter()).map(|x| x.0.clone()).collect(); threads.sort(); threads.dedup();
            for t in threads { let _ = h.frames.send(json!({"t":"queue","thread":t})); }
            h.bump_views(); shown = now_shown;
        }
        drop(h);
        tokio::select! { _ = out.kick.notified() => {}, _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {} }
    }
}
/// What `Router::hold` keeps until its machine takes it: a person's message to a thread there, and a person's post in a room there
pub fn held_method(m: &str) -> bool { m == "turn.start" || m == "bots.post" }
#[cfg(test)] #[test] fn a_message_and_a_room_post_are_held_and_nothing_else() { assert!(held_method("turn.start") && held_method("bots.post") && !held_method("bots.say") && !held_method("thread.rename")); }
/// The owner's side: the ids of the messages a linked machine posted lately, so one it posts again (its answer was lost on the way) runs once.
static TAKEN: std::sync::Mutex<std::collections::VecDeque<String>> = std::sync::Mutex::new(std::collections::VecDeque::new());
fn taken_before(link: &str, msg: &str) -> bool { !msg.is_empty() && TAKEN.lock().unwrap().iter().any(|k| *k == format!("{link}/{msg}")) }
fn taken(link: &str, msg: &str) { if msg.is_empty() { return } let mut t = TAKEN.lock().unwrap(); t.push_back(format!("{link}/{msg}")); while t.len() > 512 { t.pop_front(); } }

/// The start of the error `mcp` gives when the machine answered 501: it is up, on a build that cannot take the request.
pub const OLDER: &str = "that machine runs an older Backplane: update it there";
/// The owner's own machines this hub is linked to: (link id, name, address)
pub fn machines_of(sync: &FarSync) -> Vec<(String, String, String)> {
    let j = sync.peering.routes.machines_value();
    let rows: Vec<(String, String, String)> = sync.peering.table.with(|p| p.live().into_iter().filter(|r| super::farsync::own_link(p, &j, &r.id)).map(|r| (r.id, r.name, r.url)).collect());
    // names are unique: two machines called the same are told apart (`bots::mesh::unique_labels`), so `~name` and `name@machine` mean one machine
    let names: Vec<(String, String)> = rows.iter().map(|r| (r.0.clone(), r.1.clone())).collect();
    let uniq = crate::bots::mesh::unique_labels(&names, crate::bots::mesh::RESERVED);
    rows.into_iter().zip(uniq).map(|(r, (_, label))| (r.0, label, r.2)).collect()
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
    fn machines(&self) -> Vec<(String, String, String)> { machines_of(&self.sync) }
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
    // a link says where it is now: taken for an approved link whether or not the machine finder lists it (it is how the finder comes to find it)
    if let Ok(o) = serde_json::from_slice::<Value>(&r.body) { if o["m"] == "peer.address" { return super::mesh::address_received(sync, hub, &link, &o["p"]).await } }
    if !sync.own(&link) { return HttpReply::text("403 Forbidden", "threads are shared with the owner's own machines") }
    let Ok(o) = serde_json::from_slice::<Value>(&r.body) else { return HttpReply::text("400 Bad Request", "not JSON") };
    let m = o["m"].as_str().unwrap_or("");
    if m == "owner.mail" {
        let Some(bots) = crate::bots::api::of(hub) else { return HttpReply::text("503 Service Unavailable", "bots are not running") };
        let (code, body) = bots.owner_received(&link, &o).await;
        return HttpReply { status: status(code), ctype: "text/plain".into(), extra: String::new(), body: body.into_bytes() };
    }
    let p = o.get("p").cloned().unwrap_or(Value::Null);
    if m.starts_with("bots.") && FAR_BOTS.contains(&m) {
        let Some(bots) = crate::bots::api::of(hub) else { return HttpReply::text("503 Service Unavailable", "bots are not running") };
        let (code, body) = bots.far_received(&link, m, &p).await;
        return HttpReply { status: status(code), ctype: "text/plain".into(), extra: String::new(), body: body.into_bytes() };
    }
    // the viewers of a window of another machine (net/farview.rs): the drawing is made here, where the files are
    match m {
        "viewer.call" => return json_reply(&super::farview::call(hub, &link, &o["p"]).await),
        "viewer.pull" => return json_reply(&super::farview::pull(hub, &link, &o["p"]).await),
        "viewer.drop" => return json_reply(&super::farview::drop_sub(hub, &link, &o["p"]).await),
        // an agent here showed a drawing: the windows of the owner's other machines show it too (their thread is this one through the link)
        "viewer.show" => {
            let g = |k: &str| o["p"][k].as_str().unwrap_or("").to_string();
            if !g("thread").is_empty() && !g("kind").is_empty() { hub.push_all(&json!({"t":"view","thread": far::id(&link, &g("thread")),"path": g("path"),"kind": g("kind")})); }
            return json_reply(&json!({"t":"reply","ok":true}))
        }
        _ => {}
    }
    if m == "bots.mesh_code" { return super::mesh::serve_code(sync, hub, &o.get("p").cloned().unwrap_or(Value::Null)).await }
    if m == "ring.front" { return json_reply(&crate::bots::front::serve(hub, &link, &p).await) }
    if m.starts_with("home.") { return json_reply(&super::homemove::serve(hub, &link, m, &p).await) }
    if m == "uber.sync" { return HttpReply::text("501 Not Implemented", "uber.sync is not ported on this hub yet") }
    if m == "mcp.call" {
        let tool = p["tool"].as_str().unwrap_or("");
        if !MCP_REMOTE.contains(&tool) { return HttpReply::text("403 Forbidden", "not something this machine shares") }
        let from = p["from"].as_str().unwrap_or("");
        if from.is_empty() || from.contains('~') { return HttpReply::text("400 Bad Request", "who is asking?") }
        if tool == "plane_read" {
            let Some(bots) = crate::bots::api::of(hub) else { return HttpReply::text("503 Service Unavailable", "bots are not running") };
            let (code, body) = bots.far_received(&link, &format!("tool.{tool}"), &p["args"]).await;
            return if code == 200 { json_reply(&json!({"ok": true, "body": serde_json::from_str::<Value>(&body).unwrap_or(Value::String(body))})) } else { HttpReply { status: status(code), ctype: "text/plain".into(), extra: String::new(), body: body.into_bytes() } };
        }
        let Some(srv) = hub.far_mcp() else { return HttpReply::text("503 Service Unavailable", "agents are not running") };
        let (ok, body) = srv.serve(&link, &super::far::id(&link, from), tool, &p["args"]).await;
        return json_reply(&json!({"ok": ok, "body": body}));
    }
    let Some(feed) = sync.feed.get() else { return HttpReply::text("503 Service Unavailable", "not ready") };
    if !feed.allowed(m, &p) { return HttpReply::text("403 Forbidden", "not something this machine shares") }
    // as a local client's request (Far.client), here and never onward: the reply frame is the answer
    let msg = if m == "turn.start" { p["msg"].as_str().unwrap_or("").to_string() } else { String::new() };
    if taken_before(&link, &msg) { return json_reply(&json!({"t":"reply","id":0,"ok":true})) }
    let mut c = Conn { sent: vec![], client: hub.fresh_client(), gen: "far".into(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 };
    let out = hub.request_here(&mut c, &json!({"id": 0, "m": m, "p": p})).await;
    match out.into_iter().find(|v| v["t"] == "reply") { Some(v) => { if v["ok"] == true { taken(&link, &msg) } json_reply(&v) } None => HttpReply::text("500 Internal Server Error", "no answer") }
}
/// The bots lane's requests between machines: a room's members, a post in it
const FAR_BOTS: &[&str] = &["bots.room_sync", "bots.room_post", "bots.list", "bots.room_join", "bots.room_leave", "bots.hello"];
/// `Fm.names` the owner's machines may call here (Fm.remote)
const MCP_REMOTE: &[&str] = &["thread_send", "thread_wait_now", "thread_interrupt", "thread_update", "thread_list", "thread_launch", "machine_list", "thread_read", "thread_handoff", "plane_read", "launch_report", "thread_settle"];
fn status(code: u16) -> &'static str { match code { 200 => "200 OK", 400 => "400 Bad Request", 403 => "403 Forbidden", 409 => "409 Conflict", 503 => "503 Service Unavailable", _ => "500 Internal Server Error" } }

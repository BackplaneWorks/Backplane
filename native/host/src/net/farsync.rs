//! Threads elsewhere, the receiving side: `POST /far/push` from a linked machine of the owner's, and the pull
//! (`POST /far/log`) that fills a gap or follows another history. Both end in `FarStore::take`, whose events go to the
//! presenter (`Hub::fold_mirror`) renamed to this hub's ids, never to the log or `Model`. Admission is the signed
//! intake (`Peering::signed_in`: fresh, genuine, once) plus "one of the owner's own machines" (`Far.own.id`: live,
//! verified, sync not off, one of the machines this hub found). Served here too: `/far/log` (feed.rs) and the pushes (outbox.rs). Not yet: `/far/rpc`, streamed text (`live`) and desk notes (`note`): those answer ok and are dropped.
use super::{far::Item, farstore::{epoch_changed, FarStore}, machines, peering::{PReq, Peering}, peers::Peers};
use crate::{api::{Fut, HttpHandler, HttpReply}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex}};

pub struct FarSync { pub store: Arc<FarStore>, pub peering: Arc<Peering>, pub feed: std::sync::OnceLock<Arc<super::feed::Feed>>, catalogs: Mutex<HashMap<String, Value>>, pulling: Mutex<std::collections::HashSet<String>> }

/// Far.own.id: a live link that is one of the owner's machines (verified, not off, found by this hub).
pub fn own_link(p: &Peers, machines_json: &Value, link: &str) -> bool {
    let Some(row) = p.peer(link).filter(|r| !r.revoked) else { return false };
    // (right after a start nothing is published yet, until the first pass of the machine finder has asked every machine /hello: a link the
    // person approved does not stop being theirs for those seconds, so the machine list only decides once it exists)
    let finding = machines_json.as_object().map_or(true, |o| o.is_empty());
    p.verified(link) && !p.sync_off(&row.name) && (finding || machines::machine(machines_json, &row.name, &row.url).is_some())
}
/// The links whose mirrors the sidebar shows, with the machine's name: verified (the owner's own), not revoked, sync not set off.
pub fn shown_names(peering: &Peering) -> Vec<(String, String)> {
    peering.table.with(|p| p.live().into_iter().filter(|r| p.verified(&r.id) && !p.sync_off(&r.name)).map(|r| (r.id, r.name)).collect())
}
fn items_of(o: &Value) -> Vec<Item> { o["items"].as_array().into_iter().flatten().map(Item::of).collect() }
fn forbidden() -> HttpReply { HttpReply::text("403 Forbidden", "threads are shared with the owner's own machines") }

impl FarSync {
    pub fn new(store: Arc<FarStore>, peering: Arc<Peering>) -> Arc<FarSync> { Arc::new(FarSync { store, peering, feed: Default::default(), catalogs: Mutex::new(HashMap::new()), pulling: Default::default() }) }
    /// One of the owner's machines whether or not it answers now (verified, not revoked, sync not off): a message to it may wait for it (net/farsend.rs).
    pub(super) fn mine(&self, link: &str) -> bool { self.peering.table.with(|p| p.peer(link).map_or(false, |r| !r.revoked && p.verified(link) && !p.sync_off(&r.name))) }
    pub(super) fn own(&self, link: &str) -> bool { let j = self.peering.routes.machines_value(); self.peering.table.with(|p| own_link(p, &j, link)) }
    pub fn catalog(&self, link: &str) -> Value { self.catalogs.lock().unwrap().get(link).cloned().unwrap_or(json!([])) }

    /// Take a batch (blocking fsyncs off the workers) and hand the events to the presenter. Ok(gap).
    async fn take(&self, hub: &Arc<Hub>, link: &str, epoch: &str, since: u64, to: u64, items: Vec<Item>) -> anyhow::Result<bool> {
        let (store, l, e) = (self.store.clone(), link.to_string(), epoch.to_string());
        // store and fold under the hub's mirror gate: a replay (after a presenter rebuild) snapshots either before this batch is stored
        // (and this batch is folded after it) or after it is folded, never in between
        let gate = hub.mirror_gate().await;
        let t = tokio::task::spawn_blocking(move || store.take(&l, &e, since, to, &items)).await??;
        // another history starts the mirror over, and forgets the link's old rows first (Fr.forget): the presenter is rebuilt (the
        // log folded again) and given every mirror, this link's new one included, by the feed (`Hub::forget_mirror`)
        if t.reset {
            eprintln!("far: the mirror of {link} started over (another history)");
            hub.mirrors().forget(link);
            drop(gate); // (the rebuild takes the gate itself; its replay gives the new history)
            if !hub.forget_mirror().await {
                // the presenter would not restart: its old rows stay, the new ones are folded anyway, and it is said
                eprintln!("far: the presenter did not restart; the old rows of {link} stay beside the new history");
                fold_in_chunks(hub, link, &t.events).await;
            }
        } else if !t.events.is_empty() { for c in t.events.chunks(500) { hub.fold_mirror_held(link, c).await } }
        Ok(t.got.gap)
    }

    /// POST /far/log: a page of what this hub shares after `since`.
    pub async fn log(self: &Arc<Self>, hub: &Arc<Hub>, r: &PReq) -> HttpReply {
        let link = match self.peering.signed_in("far-log", r) { Ok(p) => p, Err(e) => return e };
        if !self.own(&link) { return forbidden() }
        let Ok(o) = serde_json::from_slice::<Value>(&r.body) else { return HttpReply::text("400 Bad Request", "not JSON") };
        let Some(feed) = self.feed.get() else { return HttpReply::text("503 Service Unavailable", "not ready") };
        let cat = feed.catalog(hub).await;
        match feed.answer(hub, o["since"].as_u64().unwrap_or(0), o["epoch"].as_str().unwrap_or(""), cat).await {
            Some(v) => HttpReply { status: "200 OK", ctype: "application/json".into(), extra: String::new(), body: v.to_string().into_bytes() },
            None => HttpReply::text("503 Service Unavailable", "not ready"),
        }
    }
    /// POST /far/push
    pub async fn push(self: &Arc<Self>, hub: &Arc<Hub>, r: &PReq) -> HttpReply {
        let link = match self.peering.signed_in("far-push", r) { Ok(p) => p, Err(e) => return e };
        if !self.own(&link) { return forbidden() }
        let Ok(o) = serde_json::from_slice::<Value>(&r.body) else { return HttpReply::text("400 Bad Request", "not JSON") };
        let ok = || HttpReply { status: "200 OK", ctype: "application/json".into(), extra: String::new(), body: br#"{"ok":true}"#.to_vec() };
        if o.get("live").is_some() || o.get("note").is_some() { return ok() } // (streamed text and desk notes: not here yet)
        let (epoch, since, head) = (o["epoch"].as_str().unwrap_or("").to_string(), o["since"].as_u64().unwrap_or(0), o["head"].as_u64().unwrap_or(0));
        // a push from another history is only a resync hint: ask the owner for its current epoch instead of trusting a late old push
        if epoch_changed(&self.store.epoch(&link), &epoch) { self.pull_later(hub, &link); return ok() }
        match self.take(hub, &link, &epoch, since, head, items_of(&o)).await {
            Ok(true) => { self.pull_later(hub, &link); ok() }
            Ok(false) => ok(),
            Err(_) => HttpReply::text("500 Internal Server Error", "could not keep it"),
        }
    }
    fn pull_later(self: &Arc<Self>, hub: &Arc<Hub>, link: &str) { let (me, hub, link) = (self.clone(), hub.clone(), link.to_string()); tokio::spawn(async move { me.pull(&hub, &link).await }); }

    /// Far.pull.here: ask the owner for what follows our head, page after page, until it says there is no more. One pull
    /// per link at a time. Returns whether the mirror is caught up (false: the owner is away or not ready).
    pub async fn pull(&self, hub: &Arc<Hub>, link: &str) -> bool {
        if !self.pulling.lock().unwrap().insert(link.to_string()) { return false }
        let done = self.pull_pages(hub, link).await;
        self.pulling.lock().unwrap().remove(link);
        done
    }
    async fn pull_pages(&self, hub: &Arc<Hub>, link: &str) -> bool {
        let url = self.peering.table.with(|p| p.url(link));
        if url.is_empty() || !self.own(link) { return false }
        for _ in 0..200 {
            let (since, epoch) = (self.store.head(link), self.store.epoch(link));
            let body = json!({"since": since, "epoch": epoch}).to_string();
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let r = super::botnet::to_peer(&self.peering.home, &url, link, now, "/far/log", true, &body).await;
            if r.code != 200 { return false }
            let Ok(o) = serde_json::from_str::<Value>(&r.body) else { return false };
            if let Some(c) = o.get("catalog") { self.catalogs.lock().unwrap().insert(link.to_string(), c.clone()); }
            let (from, head, more) = (o["since"].as_u64().unwrap_or(since), o["head"].as_u64().unwrap_or(0), o["more"].as_bool().unwrap_or(false));
            let ep = o["epoch"].as_str().unwrap_or("").to_string();
            match self.take(hub, link, &ep, from, head, items_of(&o)).await { Ok(false) => {} _ => return false }
            if !more { return true }
        }
        false
    }
}

/// Events to the presenter a chunk at a time, so one frame is never a whole history.
pub async fn fold_in_chunks(hub: &Arc<Hub>, link: &str, events: &[Value]) { for c in events.chunks(500) { hub.fold_mirror(link, c).await } }
/// (the caller holds the mirror gate: the replay after a rebuild)
pub async fn fold_held(hub: &Arc<Hub>, link: &str, events: &[Value]) { for c in events.chunks(500) { hub.fold_mirror_held(link, c).await } }

/// Re-feeds every mirror (state and the last message of each thread) to the presenter after it lost them.
pub struct Refeed(pub Arc<FarStore>);
impl crate::api::MirrorFeed for Refeed {
    fn replay<'a>(&'a self, hub: &'a Arc<Hub>) -> Fut<'a, ()> {
        Box::pin(async move {
            for l in self.0.links() {
                let (s, link) = (self.0.clone(), l.clone());
                let ev = tokio::task::spawn_blocking(move || s.replay_events(&link)).await.unwrap_or_default();
                fold_held(hub, &l, &ev).await; // (called by the hub with the gate held)
            }
        })
    }
}

pub struct FarRoutes(pub Arc<FarSync>);
impl HttpHandler for FarRoutes {
    fn routes(&self) -> &'static [&'static str] { &["/far/push", "/far/log"] }
    fn own_auth(&self) -> bool { true }
    fn body_max(&self) -> usize { 256 << 10 }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a crate::auth::Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            if req.method != "POST" { return HttpReply::text("404 Not Found", "not found") }
            let p = super::peerhttp::PeerRoutes::preq(req);
            if req.route() == "/far/log" { self.0.log(hub, &p).await } else { self.0.push(hub, &p).await }
        })
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod jstests;

//! Pushing this hub's shared changes to the owner's other machines.
//! One push at a time per link. A batch says it follows the last push that got through (`acked`) only while every push
//! since did (`clean`); after a failure (or a restart: nothing is known) it says it follows only its own start, so the
//! other end, missing what failed, finds the gap and pulls. Batches that come while one is out wait, merged. A failure
//! puts the batch back ahead of what waited and the link holds for a backoff (2, 4 ... 60 s) before it goes again.
//! `Out` is that state machine, pure; `run` drives it. Laws `far_fail_keeps_items`, `far_ack_*`, `far_retry_*`,
//! `far_hold_waits`, `far_release_resends`, `far_backoff`.
use super::{far::Item, farsync, feed, peering::Peering};
use crate::hub::Hub;
use serde_json::json;
use std::{collections::HashMap, sync::{Arc, Mutex}, time::Duration};

/// Waiting items kept per link; more than this and the link is sent the gap form instead.
pub const MAX_WAITING: usize = 10_000;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Out { pub lost: bool, pub flight_lost: bool, pub acked: u64, pub clean: bool, pub busy: bool, pub from: u64, pub to: u64, pub items: Vec<Item>, pub flight: Vec<Item>, pub ffrom: u64, pub hold: bool, pub fails: u64 }
/// A batch to send: what it follows, where it ends, its items.
#[derive(Debug, PartialEq)]
pub struct Send { pub since: u64, pub to: u64, pub items: Vec<Item> }

impl Out {
    /// A commit that took the log from `from` to `to`, with what it shares.
    pub fn add(&mut self, from: u64, to: u64, xs: Vec<Item>) {
        if xs.is_empty() { self.to = to; return }
        if self.items.is_empty() { self.from = from }
        self.to = to; self.items.extend(xs);
        self.cap();
    }
    /// A link that takes nothing (a 403, a wedged hub) must not hold every shared line in memory: past the limit the
    /// items are dropped and the next push is the gap form (`since = to`, no items), so the other end pulls when it is back.
    fn cap(&mut self) { if self.items.len() > MAX_WAITING { self.items.clear(); self.lost = true; self.clean = false; self.from = self.to } }
    /// The push to start now, if one is due (none out, not holding, something waiting).
    pub fn next(&mut self) -> Option<Send> {
        if self.busy || self.hold || (self.items.is_empty() && !self.lost) { return None }
        if self.items.is_empty() {
            // the gap form: the other end is told only where this log ends
            self.lost = false; self.flight_lost = true; self.busy = true; self.flight.clear(); self.ffrom = self.to;
            return Some(Send { since: self.to, to: self.to, items: vec![] });
        }
        let xs = std::mem::take(&mut self.items);
        self.lost = false; // (what follows the loss says where it begins: since = from, past the other end's head)
        let s = Send { since: if self.clean { self.acked } else { self.from }, to: self.to, items: xs.clone() };
        self.busy = true; self.flight = xs; self.ffrom = self.from;
        Some(s)
    }
    /// The push out ended: through (up to `to`), or not (its items go back first).
    pub fn done(&mut self, ok: bool, to: u64) {
        if ok { self.flight_lost = false; self.acked = to; self.clean = true; self.busy = false; self.flight.clear(); self.ffrom = 0; self.hold = false; self.fails = 0 }
        else {
            let fl = std::mem::take(&mut self.flight);
            if !fl.is_empty() { self.from = self.ffrom }
            let mut back = fl; back.append(&mut self.items); self.items = back;
            if self.flight_lost { self.lost = true; self.flight_lost = false }
            self.cap();
            self.clean = false; self.busy = false; self.ffrom = 0; self.hold = true; self.fails += 1;
        }
    }
    pub fn release(&mut self) { self.hold = false }
    pub fn due(&self) -> bool { !self.busy && !self.hold && (!self.items.is_empty() || self.lost) }
}
/// Seconds to hold after `n` failures in a row: 2, 4, 8, 16, 32, 60.
pub fn delay(n: u64) -> u64 { let mut d = 2; for _ in 0..n.saturating_sub(1) { d = if d >= 32 { 60 } else { d * 2 } } d }

/// What a commit shares: (from, to, items) for the push machine.
pub type Batch = (u64, u64, Vec<Item>);

pub struct Outbox { outs: Mutex<HashMap<String, Out>> }
impl Outbox {
    pub fn new() -> Arc<Outbox> { Arc::new(Outbox { outs: Mutex::new(HashMap::new()) }) }
    fn with<R>(&self, link: &str, f: impl FnOnce(&mut Out) -> R) -> R { f(self.outs.lock().unwrap().entry(link.to_string()).or_default()) }
    pub fn add(&self, link: &str, b: &Batch) { self.with(link, |o| o.add(b.0, b.1, b.2.clone())) }
    pub fn next(&self, link: &str) -> Option<Send> { self.with(link, Out::next) }
    pub fn done(&self, link: &str, ok: bool, to: u64) -> u64 { self.with(link, |o| { o.done(ok, to); o.fails }) }
    pub fn release(&self, link: &str) { self.with(link, Out::release) }
}

/// Far.push.body: past 200000 characters only the end is said (a gap: the other end pulls in pages).
pub fn body(epoch: &str, name: &str, s: &Send) -> String {
    let full = json!({"epoch": epoch, "name": name, "since": s.since, "head": s.to, "items": s.items.iter().map(Item::json).collect::<Vec<_>>()}).to_string();
    if full.chars().count() > 200_000 { json!({"epoch": epoch, "name": name, "since": s.to, "head": s.to, "items": []}).to_string() } else { full }
}

/// Drive the pushes: each batch of shared lines goes to every one of the owner's machines (live, verified, sync on).
pub async fn run(hub: Arc<Hub>, peering: Arc<Peering>, out: Arc<Outbox>, mut rx: tokio::sync::mpsc::UnboundedReceiver<Batch>) {
    while let Some(b) = rx.recv().await {
        let j = peering.routes.machines_value();
        let links: Vec<String> = peering.table.with(|p| p.live().into_iter().filter(|r| farsync::own_link(p, &j, &r.id)).map(|r| r.id).collect());
        for l in links { out.add(&l, &b); drive(hub.clone(), peering.clone(), out.clone(), l) }
    }
}
/// Start the push due on `link`, if any; when it ends, go on.
fn drive(hub: Arc<Hub>, peering: Arc<Peering>, out: Arc<Outbox>, link: String) {
    let Some(send) = out.next(&link) else { return };
    tokio::spawn(async move {
        let url = peering.table.with(|p| p.url(&link));
        let epoch = feed::epoch(&hub);
        let name = super::botnet::name().await;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let r = if url.is_empty() { super::botnet::Resp { code: 0, body: String::new() } } else { super::botnet::to_peer(&peering.home, &url, &link, now, "/far/push", true, &body(&epoch, &name, &send)).await };
        let fails = out.done(&link, r.code == 200, send.to);
        if r.code != 200 { tokio::time::sleep(Duration::from_secs(delay(fails))).await; out.release(&link) }
        drive(hub, peering, out, link);
    });
}

#[cfg(test)]
mod tests;

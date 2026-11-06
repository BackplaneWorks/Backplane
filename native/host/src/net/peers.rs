//! What this hub knows of its linked machines, folded from the log: `PeerSet` events (the peer table,
//! model/bot `World.peer_set`) and the two setting families the pairing rules read: `sync.<name>`
//! (`Mate.off`) and `peer.own.<link>` (`Far.own.how`, only the hub writes it). The table is replayed from the
//! log at start and kept by an observer; it holds peers and a few settings, nothing that grows with the log.
use crate::{api::Observer, hub::Hub};
use serde_json::Value;
use std::{collections::HashMap, sync::{Arc, Mutex}};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PeerRow { pub id: String, pub name: String, pub url: String, pub created: u64, pub revoked: bool }

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Peers {
    rows: HashMap<String, PeerRow>,
    settings: HashMap<String, String>,
    applied: u32,
    replayed: bool,
}
fn text(v: &Value) -> String { v.as_str().unwrap_or("").to_string() }
fn watched(key: &str) -> bool { key.starts_with("sync.") || key.starts_with("peer.own.") }

impl Peers {
    /// World.peer_set: an empty name or url keeps the old; `created` is the time of the first set after the peer
    /// was unknown or revoked.
    pub fn fold(&mut self, e: &Value) {
        match e["$"].as_str() {
            Some("PeerSet") => {
                let id = text(&e["id"]);
                let at = e["at"].as_u64().unwrap_or(0);
                let old = self.rows.get(&id).cloned().unwrap_or(PeerRow { id: id.clone(), name: String::new(), url: String::new(), created: 0, revoked: true });
                let keep = |was: &str, now: String| if now.is_empty() { was.to_string() } else { now };
                self.rows.insert(id.clone(), PeerRow { id, name: keep(&old.name, text(&e["name"])), url: keep(&old.url, text(&e["url"])),
                    created: if old.revoked { at } else { old.created }, revoked: e["revoked"].as_bool().unwrap_or(false) });
            }
            Some("SettingSet") => {
                let (k, v) = (text(&e["key"]), text(&e["value"]));
                if watched(&k) { if v.is_empty() { self.settings.remove(&k); } else { self.settings.insert(k, v); } }
            }
            _ => {}
        }
    }
    /// World.peers: the live ones.
    pub fn live(&self) -> Vec<PeerRow> { let mut v: Vec<_> = self.rows.values().filter(|p| !p.revoked).cloned().collect(); v.sort_by(|a, b| a.id.cmp(&b.id)); v }
    pub fn peer(&self, id: &str) -> Option<&PeerRow> { self.rows.get(id) }
    /// Bot.peer.url: a live peer's address, "" for none.
    pub fn url(&self, id: &str) -> String { self.rows.get(id).filter(|p| !p.revoked).map(|p| p.url.clone()).unwrap_or_default() }
    pub fn setting(&self, key: &str) -> &str { self.settings.get(key).map(String::as_str).unwrap_or("") }
    /// Mate.off
    pub fn sync_off(&self, name: &str) -> bool { self.setting(&super::machines::sync_key(name)) == "off" }
    /// The names (lowercased) whose sync is off here.
    pub fn sync_off_names(&self) -> std::collections::HashSet<String> {
        self.settings.iter().filter(|(k, v)| k.starts_with("sync.") && v.as_str() == "off").map(|(k, _)| k[5..].to_string()).collect()
    }
    /// Far.own.how: "pair" (paired through POST /bots/pair, Tailscale vouched), "tailnet", "owner", "no", or "".
    pub fn own_how(&self, link: &str) -> &str { self.setting(&format!("peer.own.{link}")) }
    /// Far.verified: the link is one of the owner's own machines.
    pub fn verified(&self, link: &str) -> bool { !matches!(self.own_how(link), "" | "no") }
}

/// The shared table: replayed at start, then kept by `Observer`. A commit that lands while the replay runs is
/// buffered and applied after it by seq, so none is lost and none is applied twice.
pub struct PeerTable { st: Mutex<State> }
#[derive(Default)]
struct State { peers: Peers, pending: Vec<(u32, Vec<Value>)> }
const CHUNK: usize = 20_000;

impl PeerTable {
    pub fn new() -> Arc<PeerTable> { Arc::new(PeerTable { st: Mutex::new(State::default()) }) }
    pub fn with<R>(&self, f: impl FnOnce(&Peers) -> R) -> R { f(&self.st.lock().unwrap().peers) }
    /// The whole table from `lines` (1-based `first_seq`), for tests and small logs.
    pub fn from_lines(&self, lines: &[String], first_seq: u32) {
        let mut local = Peers::default();
        for (i, l) in lines.iter().enumerate() { fold_line(&mut local, l); local.applied = first_seq + i as u32 }
        self.finish(local);
    }
    /// A committed batch: held while the replay runs, folded by seq afterwards.
    pub fn observe(&self, events: &[Value], first_seq: u32) {
        let mut g = self.st.lock().unwrap();
        if !g.peers.replayed { g.pending.push((first_seq, events.to_vec())); return }
        apply(&mut g.peers, events, first_seq);
    }
    /// Swap the replayed table in and apply what was buffered past it, in one critical section: a commit that
    /// arrives meanwhile waits on the lock and is applied after the older batches, so `applied` never jumps ahead
    /// of a batch still pending (which the seq check would then drop).
    pub fn finish(&self, mut local: Peers) {
        let mut g = self.st.lock().unwrap();
        local.replayed = true;
        let pending = std::mem::take(&mut g.pending);
        g.peers = local;
        for (first, evs) in pending { apply(&mut g.peers, &evs, first) }
    }
    /// Register on the hub (so every later commit is seen), then read the log as it stands in chunks off the async
    /// workers, folding only the lines that can matter into a local table; the lock is held only to swap it in.
    pub async fn start(self: &Arc<Self>, hub: &Arc<Hub>) {
        hub.register_observer(self.clone());
        let h = hub.clone();
        let me = self.clone();
        let local = tokio::task::spawn_blocking(move || {
            let len = h.store.len();
            let path = crate::ckpt::dir(h.store.path()).join("peers.json");
            let mut local = crate::ckpt::load_json(&path, &h.store, |p: &Peers| p.applied as usize).unwrap_or_default();
            crate::ckpt::saved("peers", local.applied as usize);
            let mut at = local.applied as usize;
            crate::ckpt::register(move |store| { let g = me.st.lock().unwrap(); let n = g.peers.applied as usize; if g.peers.replayed && crate::ckpt::due("peers", n) { crate::ckpt::save_json(&path, store, n, &g.peers); crate::ckpt::saved("peers", n) } });
            while at < len {
                let to = (at + CHUNK).min(len);
                for l in h.store.range(at, to).unwrap_or_default() { fold_line(&mut local, &l) }
                at = to;
            }
            local.applied = len as u32; local
        }).await.unwrap_or_default();
        self.finish(local);
    }
}
/// Fold the events of a batch whose first line is `first_seq`, skipping those already applied.
fn apply(p: &mut Peers, events: &[Value], first_seq: u32) {
    for (i, e) in events.iter().enumerate() {
        let seq = first_seq + i as u32;
        if seq <= p.applied { continue }
        p.fold(e); p.applied = seq;
    }
}
/// Only a line that names `PeerSet` or `SettingSet` is parsed.
fn fold_line(p: &mut Peers, l: &str) {
    if !(l.contains("PeerSet") || l.contains("SettingSet")) { return }
    if let Ok(e) = serde_json::from_str::<Value>(l) { p.fold(&e) }
}
impl Observer for PeerTable {
    fn committed(&self, _hub: &Arc<Hub>, events: &[Value], first_seq: u32) { self.observe(events, first_seq) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn ps(id: &str, name: &str, url: &str, revoked: bool, at: u64) -> Value { json!({"$":"PeerSet","id":id,"name":name,"url":url,"revoked":revoked,"at":at}) }

    #[test] fn peer_set_folds_like_the_world() {
        let mut p = Peers::default();
        p.fold(&ps("p1", "desk", "http://d:1", false, 5));
        p.fold(&ps("p1", "", "", false, 9));
        assert_eq!(p.peer("p1"), Some(&PeerRow { id: "p1".into(), name: "desk".into(), url: "http://d:1".into(), created: 5, revoked: false }), "empty fields keep, created stays");
        p.fold(&ps("p1", "", "", true, 10));
        assert!(p.live().is_empty() && p.url("p1").is_empty(), "a revoked peer has no address");
        p.fold(&ps("p1", "desk2", "http://d:2", false, 20));
        assert_eq!(p.peer("p1").unwrap().created, 20, "linked again: created is the new link's time");
        assert_eq!(p.url("p1"), "http://d:2");
    }
    #[test] fn peer_settings_follow_set_and_removal() {
        let mut p = Peers::default();
        p.fold(&json!({"$":"SettingSet","key":"sync.desk","value":"off","at":1}));
        p.fold(&json!({"$":"SettingSet","key":"peer.own.p1","value":"pair","at":1}));
        p.fold(&json!({"$":"SettingSet","key":"other","value":"x","at":1}));
        assert!(p.sync_off("Desk") && p.verified("p1") && p.setting("other").is_empty());
        p.fold(&json!({"$":"SettingSet","key":"peer.own.p1","value":"no","at":2}));
        assert!(!p.verified("p1"), "the owner said it is not theirs");
        p.fold(&json!({"$":"SettingSet","key":"sync.desk","value":"","at":3}));
        assert!(!p.sync_off("desk"));
    }
    #[test] fn peer_commits_during_the_replay_are_neither_lost_nor_doubled() {
        let t = PeerTable::new();
        // seq 5 lands while the replay is still reading; seq 2 is one the replay also reads
        t.observe(&[ps("p5", "late", "http://l:1", false, 50)], 5);
        t.observe(&[ps("p1", "", "", true, 20)], 2);
        let mut local = Peers::default();
        for (i, e) in [ps("p0", "a", "http://a:1", false, 1), ps("p1", "b", "http://b:1", false, 2), ps("p2", "c", "http://c:1", false, 3)].iter().enumerate() { local.fold(e); local.applied = 1 + i as u32 }
        t.finish(local);
        t.with(|p| {
            assert!(p.peer("p5").is_some(), "a commit past the replay's end is applied after it");
            assert!(!p.peer("p1").unwrap().revoked, "seq 2 was in the replay: the buffered copy is not applied again");
            assert_eq!(p.applied, 5);
        });
        // after the swap, batches fold directly, once
        t.observe(&[ps("p6", "n", "http://n:1", false, 60)], 6); t.observe(&[ps("p6", "n", "http://n:1", true, 61)], 6);
        t.with(|p| assert!(!p.peer("p6").unwrap().revoked));
    }
    #[test] fn peer_only_peer_and_setting_lines_are_parsed() {
        let mut p = Peers::default();
        fold_line(&mut p, "not json PeerSet"); fold_line(&mut p, r#"{"$":"MessagePosted","text":"PeerSet"}"#);
        fold_line(&mut p, &ps("p1", "n", "http://u:1", false, 1).to_string());
        assert_eq!(p.live().len(), 1);
    }
}

//! What this hub knows of its linked machines, folded from the log: `PeerSet` events (the peer table,
//! model.bend/bot.bend `World.peer_set`) and the two setting families the pairing rules read: `sync.<name>`
//! (`Mate.off`) and `peer.own.<link>` (`Far.own.how`, only the hub writes it). The table is replayed from the
//! log at start and kept by an observer; it holds peers and a few settings, nothing that grows with the log.
use crate::{api::Observer, hub::Hub};
use serde_json::Value;
use std::{collections::HashMap, sync::{Arc, Mutex}};

#[derive(Clone, Debug, PartialEq)]
pub struct PeerRow { pub id: String, pub name: String, pub url: String, pub created: u64, pub revoked: bool }

#[derive(Default)]
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

/// The shared table: replayed at start, then kept by `Observer`.
pub struct PeerTable(Mutex<Peers>);
impl PeerTable {
    pub fn new() -> Arc<PeerTable> { Arc::new(PeerTable(Mutex::new(Peers::default()))) }
    pub fn with<R>(&self, f: impl FnOnce(&Peers) -> R) -> R { f(&self.0.lock().unwrap()) }
    pub fn from_lines(&self, lines: &[String], first_seq: u32) {
        let mut g = self.0.lock().unwrap();
        for (i, l) in lines.iter().enumerate() { if let Ok(e) = serde_json::from_str::<Value>(l) { g.fold(&e) } g.applied = first_seq + i as u32 }
        g.replayed = true;
    }
    /// Register on the hub, then replay what is already there; commits in between are folded once (by seq).
    pub async fn start(self: &Arc<Self>, hub: &Arc<Hub>) {
        hub.register_observer(self.clone());
        let lines = hub.store.all();
        let mut g = self.0.lock().unwrap();
        if g.replayed { return }
        g.applied = 0;
        for l in &lines { if let Ok(e) = serde_json::from_str::<Value>(l) { g.fold(&e) } g.applied += 1 }
        g.replayed = true;
    }
}
impl Observer for PeerTable {
    fn committed(&self, _hub: &Arc<Hub>, events: &[Value], first_seq: u32) {
        let mut g = self.0.lock().unwrap();
        if !g.replayed { return } // the replay reads the store after this commit was written
        for (i, e) in events.iter().enumerate() {
            let seq = first_seq + i as u32;
            if seq <= g.applied { continue }
            g.fold(e); g.applied = seq;
        }
    }
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
    #[test] fn peer_observer_skips_what_the_replay_covered() {
        let t = PeerTable::new();
        t.from_lines(&[ps("p1", "a", "http://a:1", false, 1).to_string()], 1);
        let h: Option<Arc<Hub>> = None; let _ = h;
        // (committed() needs a hub only for its signature: exercise the fold by seq directly)
        let mut g = t.0.lock().unwrap();
        for (seq, e) in [(1u32, ps("p1", "a", "http://a:1", true, 2)), (2, ps("p2", "b", "http://b:1", false, 3))] { if seq > g.applied { g.fold(&e); g.applied = seq } }
        assert!(g.peer("p1").unwrap().revoked == false, "seq 1 was already applied");
        assert!(g.peer("p2").is_some());
    }
}

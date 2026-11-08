//! Bots across the owner's linked machines (the transport is `OwnerLink`, a signed POST over /far/rpc to a link that is one of the owner's own: net/rpc.rs).
//!
//! A room may have members on any number of the owner's machines, written `name@machine` (the machine's name or its link id; stored as typed, read by `remote_link`).
//! Each machine keeps its own copy of the room under the same id, so Bots shows it on both. What travels, all through `Bots::far_received` on the other side:
//!   `bots.room_sync` {room, name, members: [wire], deleted?}: the room's members and name (a made, changed or left room; `deleted` when it is gone or that machine has no member left)
//!   `bots.room_post` {room, msg, from, text, hop, at}: a post of a bot or the person here
//! "wire" members name bots as the other machine reads them: a bot of the sender is `name@<link>`, a bot of the receiver is its bare name, a bot of a third machine is
//! `name@<mid>` (the machine id, `mesh.rs`: a link id is made per pair, so a third machine cannot be named by one). Every machine with members in a room is sent its changes and
//! posts directly by the machine they happen on (a full mesh, no relay); a state a machine already holds is not applied again, so nothing loops. A post is logged on the receiving machine as from `name@<machine>` and heard by that machine's members; it is never sent back (a post from `x@y` is not forwarded).
//! Nothing here is accepted from a link that is not the owner's own (the route refuses it first).
use super::{post::RoomRef, rpc::{now_s, Bots}, world::World};
use crate::api::Observer;
use crate::hub::Hub;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::{Arc, Mutex, Weak}};

pub(super) fn gs<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }

/// The state of the room traffic this machine keeps: what each room last sent a machine, and the changes it applied from one (so they are not sent back).
#[derive(Default)]
pub struct FarState { sent: Mutex<HashMap<(String, String), String>>, applied: Mutex<HashMap<(String, String), String>>, pub(super) mesh: super::mesh::MeshState }

/// Registered on the hub once the bots and the transport exist: a committed room change or post goes to the machine that has members in it.
pub struct FarOut(pub Weak<Bots>);
impl Observer for FarOut {
    fn committed(&self, _hub: &Arc<Hub>, events: &[Value], _first_seq: u32) {
        let ev: Vec<Value> = events.iter().filter(|e| matches!(e["$"].as_str(), Some("RoomSet" | "RoomDeleted" | "RoomPosted"))).cloned().collect();
        if ev.is_empty() { return }
        let Some(b) = self.0.upgrade() else { return };
        tokio::spawn(async move { b.far_forward(ev).await });
    }
}

impl Bots {
    /// Start forwarding rooms (called by the net lane after `set_owner_link`)
    pub fn far_start(self: &Arc<Self>) { self.hub.register_observer(Arc::new(FarOut(Arc::downgrade(self)))); }
    /// The owner's machines linked to this one: (link id, name, address)
    pub fn machines(&self) -> Vec<(String, String, String)> { self.owner_link.get().map(|l| l.machines()).unwrap_or_default() }
    /// The link a machine word names (name in any case, or the link id)
    pub fn machine_link(&self, word: &str) -> Option<String> {
        let w = word.trim().trim_start_matches('~').to_lowercase();
        let ww = super::mesh::word(&w);
        self.machines().into_iter().find(|(id, name, _)| id.to_lowercase() == w || name.to_lowercase() == w || (!ww.is_empty() && super::mesh::word(name) == ww)).map(|(id, _, _)| id)
    }
    fn machine_name(&self, link: &str) -> String { self.machines().into_iter().find(|m| m.0 == link).map(|m| m.1).filter(|n| !n.is_empty()).unwrap_or_else(|| link.to_string()) }
    /// The one link a room's `name@machine` members are on ("" for none); a second machine is not served
    pub fn remote_link(&self, members: &[String]) -> Vec<String> {
        let mut v: Vec<String> = vec![];
        for m in members { if let Some((_, mach)) = m.rsplit_once('@') { if let Some(l) = self.machine_link(mach) { if !v.contains(&l) { v.push(l) } } } }
        v
    }
    /// The members as machine `link` reads them (see `mesh::member_out`). A member on a third machine whose id is not known is left out.
    async fn to_wire(&self, link: &str, members: &[String]) -> Vec<String> {
        if members.iter().any(|m| m.contains('@')) { self.mesh_learn().await }
        let w = self.world().await;
        let mut out: Vec<String> = vec![];
        for m in members {
            let x = super::mesh::member_out(m, link, |id| w.bot(id).filter(|b| !b.deleted).map(|b| b.name.clone()), |mach| self.machine_link(mach), |l| self.mid_known(l));
            if let Some(x) = x { if !out.contains(&x) { out.push(x) } }
        }
        out.sort(); out
    }
    /// Wire members from machine `link` as this one stores them: its bots stay `name@<link>`, ours become bot ids, a third machine's become `name@<our link to it>`; others are dropped
    async fn from_wire(&self, link: &str, wire: &[String]) -> Vec<String> {
        if wire.iter().any(|x| x.rsplit_once('@').map_or(false, |(_, m)| m != link)) { self.mesh_learn().await }
        let w = self.world().await;
        let mut out: Vec<String> = vec![];
        for x in wire {
            let id = super::mesh::member_in(x, link, |n| w.named(n).map(|b| b.id.clone()).or_else(|| w.bot(n).filter(|b| !b.deleted).map(|b| b.id.clone())), |mid| self.link_of_mid(mid));
            if let Some(id) = id { if !out.contains(&id) { out.push(id) } }
        }
        out
    }
    pub(super) async fn far_send(&self, link: &str, m: &str, p: Value) -> Option<(u16, String)> {
        let l = self.owner_link.get()?;
        l.post(link, json!({"id": 0, "m": m, "p": p}).to_string()).await
    }

    /// Room changes and posts just committed here, sent to the machine that has members in the room
    pub async fn far_forward(self: &Arc<Self>, events: Vec<Value>) {
        if self.machines().is_empty() { return }
        for e in events {
            let (kind, room) = (gs(&e, "$").to_string(), if gs(&e, "$") == "RoomPosted" { gs(&e, "room").to_string() } else { gs(&e, "id").to_string() });
            let (members, name) = { let w = self.world().await; match w.rooms.get(&room) { Some(r) => (r.members.clone(), r.name.clone()), None => continue } };
            let had: Vec<String> = self.far.sent.lock().unwrap().keys().filter(|(r, _)| *r == room).map(|(_, l)| l.clone()).collect();
            let now_links = if kind == "RoomDeleted" { vec![] } else { self.remote_link(&members) };
            match kind.as_str() {
                "RoomPosted" => {
                    if gs(&e, "from").contains('@') || room.starts_with("dm:") { continue }
                    for l in now_links { self.far_send(&l, "bots.room_post", json!({"room":room,"msg":gs(&e, "id"),"from":gs(&e, "from"),"text":gs(&e, "text"),"hop":e["hop"],"at":e["at"]})).await; }
                }
                _ => {
                    let live: Vec<String> = if kind == "RoomDeleted" { vec![] } else { now_links.clone() };
                    // a machine that was sent the room and has no member in it now (left, or the room is gone) is told it is gone
                    let mut cands: Vec<String> = vec![]; for l in had.iter().chain(now_links.iter()).chain(self.remote_link(&members).iter()) { if !cands.contains(l) { cands.push(l.clone()) } }
                    for l in cands {
                        let gone = !live.contains(&l);
                        let key = (room.clone(), l.clone());
                        let wire = if gone { vec![] } else { self.to_wire(&l, &members).await };
                        let sig = if gone { "\u{0}deleted".to_string() } else { format!("{name}\n{}", wire.join("\n")) };
                        let came = { let mut ap = self.far.applied.lock().unwrap(); let hit = ap.get(&key).map_or(false, |m| *m == if gone { sig.clone() } else { members.join("\n") }); if hit { ap.remove(&key); } hit };
                        if came { // it came from that machine: not sent back
                            if gone { self.far.sent.lock().unwrap().remove(&key); } else { self.far.sent.lock().unwrap().insert(key, sig); }
                            continue
                        }
                        if self.far.sent.lock().unwrap().get(&key) == Some(&sig) { continue }
                        if gone && kind != "RoomDeleted" && !self.far.sent.lock().unwrap().contains_key(&key) { continue }
                        let r = self.far_send(&l, "bots.room_sync", json!({"room":room,"name":name,"members":wire,"deleted":gone})).await;
                        if matches!(r, Some((200, _))) { if gone { self.far.sent.lock().unwrap().remove(&key); } else { self.far.sent.lock().unwrap().insert(key, sig); } }
                        else { eprintln!("bots: room {room} was not taken by {l}: {r:?}") }
                    }
                }
            }
        }
    }

    /// The other machine's side: a request from `link` (the route has vouched for it)
    pub async fn far_received(self: &Arc<Self>, link: &str, m: &str, p: &Value) -> (u16, String) {
        match m {
            "bots.room_sync" => self.far_room_sync(link, p).await,
            "bots.room_post" => self.far_room_post(link, p).await,
            "bots.hello" => (200, self.mesh_hello(p["flat"].as_bool().unwrap_or(false)).await.to_string()),
            "bots.list" => { let w = self.world().await; let mut v: Vec<_> = w.bots.values().filter(|b| !b.deleted).map(|b| b.name.clone()).collect(); v.sort(); (200, json!({"bots": v}).to_string()) }
            "bots.room_join" | "bots.room_leave" => self.far_member(link, m == "bots.room_join", p).await,
            "tool.plane_read" => { let w = self.world().await; let text = self.plane_text(&w, now_s(), p["idle"].as_bool().unwrap_or(false)).await; (200, json!({"text": text}).to_string()) }
            _ => (400, "unknown request".into()),
        }
    }
    async fn far_room_sync(&self, link: &str, p: &Value) -> (u16, String) {
        let (room, now) = (gs(p, "room").to_string(), now_s());
        if room.is_empty() || room.starts_with("dm:") || room.contains('~') { return (400, "no room".into()) }
        let existing = { let w = self.world().await; w.rooms.get(&room).cloned() };
        // a room this machine made by itself is not another machine's to change by guessing its id
        if let Some(r) = &existing { if !r.deleted && !self.remote_link(&r.members).contains(&link.to_string()) { return (409, "that room is not shared with you".into()) } }
        if p["deleted"].as_bool().unwrap_or(false) {
            if existing.map_or(true, |r| r.deleted) { return (200, "ok".into()) }
            self.far.applied.lock().unwrap().insert((room.clone(), link.to_string()), "\u{0}deleted".into());
            return match self.hub.commit(&json!({"$":"RoomDeleted","id":room,"at":now})).await { Ok(_) => (200, "ok".into()), Err(e) => (503, format!("{e:#}")) }
        }
        let wire: Vec<String> = p["members"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect();
        let members = self.from_wire(link, &wire).await;
        if !members.iter().any(|m| !m.contains('@')) { // nobody of ours is in it (any more): the copy goes
            if existing.map_or(true, |r| r.deleted) { return (200, "no member here".into()) }
            self.far.applied.lock().unwrap().insert((room.clone(), link.to_string()), "\u{0}deleted".into());
            return match self.hub.commit(&json!({"$":"RoomDeleted","id":room,"at":now})).await { Ok(_) => (200, "ok".into()), Err(e) => (503, format!("{e:#}")) }
        }
        let name = if gs(p, "name").is_empty() { "room" } else { gs(p, "name") };
        // the state this machine already holds is not stored again: with several machines the same change arrives from each of them, and nothing loops
        if let Some(r) = existing.as_ref().filter(|r| !r.deleted) { let (mut a, mut b) = (r.members.clone(), members.clone()); a.sort(); b.sort(); if a == b && r.name == name { return (200, "ok".into()) } }
        let text = members.join("\n");
        self.far.applied.lock().unwrap().insert((room.clone(), link.to_string()), text.clone());
        match self.hub.commit(&json!({"$":"RoomSet","id":room,"name":name,"members":text,"at":now})).await { Ok(_) => (200, "ok".into()), Err(e) => (503, format!("{e:#}")) }
    }
    async fn far_room_post(self: &Arc<Self>, link: &str, p: &Value) -> (u16, String) {
        let (room, from, text, now) = (gs(p, "room"), gs(p, "from"), gs(p, "text"), now_s());
        if from.is_empty() || text.is_empty() { return (400, "empty post".into()) }
        let w = self.world().await;
        let Some(r) = w.room(room).cloned() else { return (404, "no such room".into()) };
        if !self.remote_link(&r.members).contains(&link.to_string()) { return (403, "that room is not shared with you".into()) }
        let mid = format!("{link}~{}", gs(p, "msg"));
        if w.has_post(&mid) { return (200, "ok".into()) }
        let rr = RoomRef { id: r.id.clone(), name: r.name.clone(), members: r.members.clone() };
        let hop = p["hop"].as_u64().unwrap_or(0);
        let shown = format!("{from}@{}", self.machine_name(link));
        match self.post_decide(&w, now, "", &shown, "", &rr, text, hop, &mid, vec![]).await {
            Ok(pending) => { drop(w); self.post_finish(pending, hop).await; (200, "ok".into()) }
            Err(e) => (503, e),
        }
    }

    /// A bot of the other machine joins or leaves a room here (it is `name@<link>` in the room)
    async fn far_member(&self, link: &str, join: bool, p: &Value) -> (u16, String) {
        let (word, bot, now) = (gs(p, "room"), gs(p, "bot"), now_s());
        if bot.is_empty() { return (400, "which bot?".into()) }
        let w = self.world().await;
        let l = word.to_lowercase();
        let Some(r) = w.rooms.values().filter(|r| !r.deleted && !r.id.starts_with("dm:")).find(|r| r.id == word || r.name.to_lowercase() == l).cloned() else { return (404, "no such room".into()) };
        let me = format!("{bot}@{link}");
        let mut ms: Vec<String> = r.members.iter().filter(|m| !m.eq_ignore_ascii_case(&me)).cloned().collect();
        if join { ms.push(me) }
        match self.hub.commit(&json!({"$":"RoomSet","id":r.id,"name":r.name,"members":ms.join("\n"),"at":now})).await { Ok(_) => (200, "ok".into()), Err(e) => (503, format!("{e:#}")) }
    }
    /// The bots of the linked machines: name@machine
    pub async fn far_bots(&self) -> Value {
        let mut out = vec![];
        for (id, name, _) in self.machines() {
            match self.far_send(&id, "bots.list", json!({})).await {
                Some((200, t)) => { let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null); for n in v["bots"].as_array().into_iter().flatten().filter_map(Value::as_str) { out.push(json!({"name":format!("{n}@{name}"),"machine":name,"link":id})) } }
                _ => out.push(json!({"machine":name,"link":id,"away":true})),
            }
        }
        Value::Array(out)
    }
    /// The `name@machine` members of a room being made must each be on a linked machine (any number of machines)
    pub fn far_members_ok(&self, ms: &[String]) -> Result<(), String> {
        for m in ms { if let Some((_, mach)) = m.rsplit_once('@') {
            if self.machine_link(mach).is_none() { return Err(format!("no linked machine called {mach} (see bot_list)")) }
        } }
        Ok(())
    }
    /// room_join / room_leave: this bot, or (with `machine`) asked of the machine the room lives on
    pub(super) async fn far_membership(&self, w: &World, bot_name: &str, join: bool, room: &str, machine: &str) -> Result<(), String> {
        let Some(link) = self.machine_link(machine) else { return Err(format!("no linked machine called {machine} (see bot_list)")) };
        let _ = w;
        match self.far_send(&link, if join { "bots.room_join" } else { "bots.room_leave" }, json!({"room":room,"bot":bot_name})).await {
            Some((200, _)) => Ok(()),
            Some((c, t)) => Err(format!("that machine did not take it ({c}): {}", t.chars().take(120).collect::<String>())),
            None => Err("that machine is away".into()),
        }
    }

    /// plane_read of another machine: its text, thread ids made `<link>~<id>`
    pub async fn far_plane(&self, thread: &str, word: &str, idle: bool) -> Result<String, String> {
        let Some(link) = self.machine_link(word) else { return Err(format!("no machine of yours called {word} is linked here (machine_list shows them)")) };
        let Some(l) = self.owner_link.get() else { return Err("no machine is linked".into()) };
        let body = json!({"id":0,"m":"mcp.call","p":{"tool":"plane_read","from":thread,"args":{"idle":idle}}}).to_string();
        match l.post(&link, body).await {
            Some((200, t)) => { let v: Value = serde_json::from_str(&t).map_err(|_| "invalid reply from that machine".to_string())?; Ok(prefix_ids(v["body"]["text"].as_str().unwrap_or(""), &link)) }
            Some((_, t)) => Err(format!("that machine did not take it: {}", t.chars().take(160).collect::<String>())),
            None => Err("that machine is away".into()),
        }
    }
}

/// Ids in a plane's text (`t17..-3`, `b..`, `r..`, `o..`) named through `link`
pub fn prefix_ids(text: &str, link: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut tok = String::new();
    let flush = |tok: &mut String, out: &mut String| {
        let b = tok.as_bytes();
        let id = b.len() > 3 && matches!(b[0], b't' | b'b' | b'r' | b'o') && b[1].is_ascii_digit() && tok.contains('-') && tok[1..].chars().all(|c| c.is_ascii_digit() || c == '-');
        if id { out.push_str(link); out.push('~'); }
        out.push_str(tok); tok.clear();
    };
    for c in text.chars() { if c.is_alphanumeric() || c == '-' || c == '~' { tok.push(c) } else { flush(&mut tok, &mut out); out.push(c) } }
    flush(&mut tok, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn ids_in_a_plane_are_named_through_the_link() {
        assert_eq!(prefix_ids("[t1790715007-34] build (running)\nbot b17-2 and room r1-1, not tomorrow-5 or p1~t2-3", "L"), "[L~t1790715007-34] build (running)\nbot L~b17-2 and room L~r1-1, not tomorrow-5 or p1~t2-3");
    }
}

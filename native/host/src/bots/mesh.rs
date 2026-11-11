//! The mesh: every paired machine talks to every other directly, so a machine has to be told apart from a link. A link id is made per pair
//! (A's link to C is not B's link to C), so what travels between three machines names a machine by its **machine id** (`mid`): 16 hex
//! digits made once and kept in `<home>/machine-id`. A machine tells its mid to a linked machine that asks (`bots.hello`); this machine
//! keeps what it was told in memory (`MeshState`) and asks again when a link is unknown.
//!
//! On the wire, a room's member is `name` (a bot of the receiver), `name@<link>` (a bot of the sender; the link id is the same on both
//! sides) or `name@<mid>` (a bot of a third machine, which the receiver maps to its own link to that machine; dropped when it has none).
//! The pure rules are `member_out` and `member_in`; names are made unique by `unique_labels`.
use super::{far::gs, rpc::Bots};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Mutex};

/// What this machine knows of the others' ids: link -> (mid, name the machine calls itself)
#[derive(Default)]
pub struct MeshState { pub(super) peers: Mutex<HashMap<String, (String, String)>>, pub(super) mine: std::sync::OnceLock<String> }

/// A machine's name as a word: `~my-laptop` names "My Laptop". Same rule as every other sigil word.
pub fn word(name: &str) -> String { crate::agents::refs::slug_of(name) }

/// Words that mean this machine, or all of them: no other machine is called one
pub const RESERVED: &[&str] = &["this machine", "this", "here", "me", "all"];

/// Names made unique: `(link, name)` in, `(link, label)` out. A name already taken (by `reserved`, this machine's own name, or an earlier link, in
/// link order so every machine would do the same) gets `-2`, `-3`... after it. Case and punctuation do not tell names apart (`word`).
pub fn unique_labels(rows: &[(String, String)], reserved: &[&str]) -> Vec<(String, String)> {
    let mut taken: Vec<String> = reserved.iter().map(|r| word(r)).collect();
    let mut order: Vec<&(String, String)> = rows.iter().collect();
    order.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out: HashMap<String, String> = HashMap::new();
    for (link, name) in order {
        let base = if word(name).is_empty() { link.clone() } else { name.trim().to_string() };
        let mut label = base.clone(); let mut n = 2;
        while taken.contains(&word(&label)) { label = format!("{base}-{n}"); n += 1 }
        taken.push(word(&label)); out.insert(link.clone(), label);
    }
    rows.iter().map(|(l, _)| (l.clone(), out[l].clone())).collect()
}

/// Whether `name` may be this machine's name for the machine at `link`: not empty, not taken by another link or by this machine.
pub fn name_free(rows: &[(String, String)], link: &str, name: &str, mine: &str) -> Result<(), String> {
    let w = word(name);
    if w.is_empty() { return Err("a name needs letters or digits".into()) }
    if RESERVED.iter().any(|r| word(r) == w) || w == word(mine) { return Err(format!("{name} is this machine's name or a word that means it")) }
    if rows.iter().any(|(l, n)| l != link && word(n) == w) { return Err(format!("another machine is already called {name}")) }
    Ok(())
}

/// A room member as the machine at `to_link` reads it. `own`: the bot's name when the member is a bot here. `link_of`: the link a `name@machine` member is on.
/// `mid_of`: a link's machine id. None: the member cannot be told to that machine (a third machine whose id is not known).
pub fn member_out(m: &str, to_link: &str, own: impl Fn(&str) -> Option<String>, link_of: impl Fn(&str) -> Option<String>, mid_of: impl Fn(&str) -> Option<String>) -> Option<String> {
    match m.rsplit_once('@') {
        None => own(m).map(|n| format!("{n}@{to_link}")),
        Some((n, mach)) => { let l = link_of(mach)?; if l == to_link { Some(n.to_string()) } else { mid_of(&l).map(|mid| format!("{n}@{mid}")) } }
    }
}
/// A wire member from machine `from` as this one stores it: `bot_id` finds a bot of ours by name, `link_of_mid` the link to a machine by its id.
pub fn member_in(x: &str, from: &str, bot_id: impl Fn(&str) -> Option<String>, link_of_mid: impl Fn(&str) -> Option<String>) -> Option<String> {
    match x.rsplit_once('@') {
        None => bot_id(x),
        Some((n, mach)) if mach == from => Some(format!("{n}@{from}")),
        Some((n, mid)) => link_of_mid(mid).map(|l| format!("{n}@{l}")),
    }
}

impl Bots {
    /// This machine's id, made on first use
    pub fn my_mid(&self) -> String {
        self.far.mesh.mine.get_or_init(|| {
            let p = self.hub.cfg.home.join("machine-id");
            let have = std::fs::read_to_string(&p).unwrap_or_default().trim().to_string();
            if have.len() == 16 && have.bytes().all(|c| c.is_ascii_hexdigit()) { return have }
            let id = hex::encode(crate::net::pairing::urandom(8));
            let _ = std::fs::write(&p, &id); id
        }).clone()
    }
    /// The machine id of a link when it is known (no network)
    pub fn mid_known(&self, link: &str) -> Option<String> { self.far.mesh.peers.lock().unwrap().get(link).map(|x| x.0.clone()).filter(|m| !m.is_empty()) }
    /// The link to the machine with this id, when known
    pub fn link_of_mid(&self, mid: &str) -> Option<String> { self.far.mesh.peers.lock().unwrap().iter().find(|(_, x)| x.0 == mid).map(|(l, _)| l.clone()) }
    /// Ask every linked machine it has no id for who it is (`bots.hello`), so the maps hold every link. A machine that is away or older stays unknown.
    pub async fn mesh_learn(&self) {
        for (link, _, _) in self.machines() {
            if self.mid_known(&link).is_some() { continue }
            if let Some((200, t)) = self.far_send(&link, "bots.hello", json!({"flat": true})).await {
                let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
                if !gs(&v, "mid").is_empty() { self.far.mesh.peers.lock().unwrap().insert(link.clone(), (gs(&v, "mid").to_string(), gs(&v, "name").to_string())); }
            }
        }
    }
    /// What a linked machine says it is, and the owner's machines it is linked to (`bots.hello` answers it): the other end of the request
    pub async fn mesh_hello(&self, flat: bool) -> Value {
        if !flat { self.mesh_learn().await }
        let mut others = vec![];
        for (link, name, url) in self.machines() { others.push(json!({"mid": self.mid_known(&link).unwrap_or_default(), "name": name, "url": url})) }
        json!({"mid": self.my_mid(), "name": crate::net::botnet::name().await, "machines": others, "server": crate::net::alwayson::other::server_now()})
    }
    /// What one linked machine says it is (`bots.hello`, flat): its id, name, machines, and `server` (it stays on: a guard under systemd, or its
    /// "stays on" switch). None when it is away or does not answer.
    pub async fn hello_of(&self, link: &str) -> Option<Value> {
        match self.far_send(link, "bots.hello", json!({"flat": true})).await { Some((200, t)) => serde_json::from_str(&t).ok(), _ => None }
    }
    /// The machines the linked ones are linked to that this one is not: `{via, via_name, name, url, mid}` (asked of each linked machine)
    pub async fn mesh_candidates(&self) -> Vec<Value> {
        self.mesh_learn().await;
        let mine = self.my_mid();
        let known: Vec<(String, String)> = self.machines().into_iter().map(|(_, _, u)| (u.clone(), crate::net::machines::hostport(&u).to_string())).collect();
        let mut out: Vec<Value> = vec![];
        for (link, name, _) in self.machines() {
            let Some((200, t)) = self.far_send(&link, "bots.hello", json!({})).await else { continue };
            let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
            for m in v["machines"].as_array().into_iter().flatten() {
                let (mid, url) = (gs(m, "mid"), gs(m, "url"));
                if mid == mine || (!mid.is_empty() && self.link_of_mid(mid).is_some()) || url.is_empty() { continue }
                if known.iter().any(|(u, hp)| u == url || hp == crate::net::machines::hostport(url)) { continue }
                if out.iter().any(|o| o["url"] == url) { continue }
                out.push(json!({"via": link, "via_name": name, "name": gs(m, "name"), "url": url, "mid": mid}));
            }
        }
        out
    }

    /// A machine code of the machine at `url`, asked through the linked machine `via` (None: `url` is a machine this one is linked to, and it is asked here).
    /// With `via`, `via` is asked to ask `url`; without, the machine at `url` is asked directly. Ok((code, the machine's address)).
    pub async fn mesh_code(&self, via: Option<&str>, url: &str) -> Result<(String, String), String> {
        let hp = crate::net::machines::hostport;
        let (link, p) = match via {
            Some(v) => { if !self.machines().iter().any(|m| m.0 == v) { return Err("no such link".into()) } (v.to_string(), json!({"url": url})) }
            None => (self.machines().into_iter().find(|m| hp(&m.2) == hp(url)).map(|m| m.0).ok_or("that machine is not linked here")?, json!({})),
        };
        match self.far_send(&link, "bots.mesh_code", p).await {
            Some((200, t)) => { let v: Value = serde_json::from_str(&t).map_err(|_| "no usable code came back".to_string())?;
                let (c, a) = (gs(&v, "code"), gs(&v, "address")); if c.is_empty() || a.is_empty() { Err("no usable code came back".into()) } else { Ok((c.to_string(), a.to_string())) } }
            Some((c, t)) => Err(format!("that machine did not give a code ({c}): {}", t.chars().take(120).collect::<String>())),
            None => Err("that machine is away".into()),
        }
    }

    /// Keep what the clients show of the mesh fresh: the machines the linked ones are linked to (Settings → Machines), and the paired machines with
    /// their projects and threads (the composer's `~machine`). Display only; nothing here is a decision.
    pub fn mesh_watch(self: &std::sync::Arc<Self>, peering: std::sync::Arc<crate::net::peering::Peering>) {
        let b = self.clone();
        tokio::spawn(async move {
            let mut n = 0u32;
            loop {
                b.mesh_publish();
                if n % 6 == 0 { let c = b.mesh_candidates().await; *peering.via.lock().unwrap() = c; }
                n += 1;
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
    }
    /// The paired machines as the composer offers them: name, whether they answered last, their projects and threads (their mirrors')
    pub fn mesh_rows(&self) -> Vec<crate::session::composer::mention::MachineRow> {
        use crate::session::composer::mention::MachineRow;
        self.machines().into_iter().map(|(link, name, _)| {
            let away = self.hub.mirrors().mark(&name)["away"].as_bool().unwrap_or(false);
            let (projects, threads) = self.hub.mirrors().shown(|v| v.iter().find(|(n, _)| *n == name).map(|(_, st)| (
                st.projects.iter().filter(|(_, p)| !p.removed).map(|(i, p)| (i.clone(), p.title.clone())).collect::<Vec<_>>(),
                st.threads.iter().filter(|(_, t)| !t.deleted && !t.title.trim().is_empty()).map(|(i, t)| (i.clone(), t.title.clone())).collect::<Vec<_>>())).unwrap_or_default());
            MachineRow { name, link, away, projects, threads }
        }).collect()
    }
    /// What the composer's popup and chips read (`mention::machines`)
    pub fn mesh_publish(&self) { crate::session::composer::mention::set_machines(self.mesh_rows()); }
    /// What an agent is told about the paired machines, by name (the prompt's `<machines>` block); "" when none is paired
    pub fn machines_prompt(&self) -> String {
        let ms = self.machines();
        if ms.is_empty() { return String::new() }
        let mut s = String::from("<machines>\nThis machine is paired with the person's other machines, each reachable by name. A name written ~name (or the word `machine`) in a tool call picks it.\n");
        for (link, name, _) in ms {
            let away = self.hub.mirrors().mark(&name)["away"].as_bool().unwrap_or(false);
            s += &format!("- ~{} ({}, link {link}): {}\n", word(&name), name, if away { "away now" } else { "here" });
        }
        s += "machine_list shows their projects; thread_launch with `machine` starts a thread on one; thread_list/thread_send/thread_read reach threads there as <link>~<id>; plane_read, room members (name@machine) and bots work the same on every machine.\n</machines>";
        s
    }
}

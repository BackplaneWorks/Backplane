//! The bots world folded from the log: src/core/bot.bend `World` and `M.State.world` (model.bend), for the events a
//! bots request reads. Only this lane reads it; the presenter folds the same events for the screens.
use crate::store::Store;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Bot { pub id: String, pub name: String, pub thread: String, pub look: u64, pub persona: String, pub created: u64, pub deleted: bool }
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Room { pub id: String, pub name: String, pub members: Vec<String>, pub created: u64, pub deleted: bool }
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Mem { pub bot: String, pub key: String, pub kind: String, pub text: String, pub tags: String, pub created: u64, pub updated: u64 }
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Routine { pub id: String, pub bot: String, pub name: String, pub cron: String, pub prompt: String, pub on: bool, pub last: u64, pub created: u64, pub deleted: bool,
    pub once: u64, pub tz: String, pub kind: String, pub done: bool, pub slot: u64 }
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Hook { pub id: String, pub bot: String, pub name: String, pub created: u64, pub revoked: bool, pub last: u64, pub count: u64 }
/// BT.Hops
#[derive(Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Hops { pub now: u64, pub wait: u64, pub held: bool }
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Peer { pub id: String, pub name: String, pub url: String, pub created: u64, pub revoked: bool }

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct World {
    pub bots: HashMap<String, Bot>, pub rooms: HashMap<String, Room>, post_ids: HashSet<String>,
    pub mems: HashMap<String, Mem>, pub spaces: HashMap<String, String>, pub pages: HashMap<String, (String, u64)>,
    pub routines: HashMap<String, Routine>, pub hooks: HashMap<String, Hook>, pub peers: HashMap<String, Peer>,
    /// only the settings a bots request reads (SETTINGS): a log of a million lines holds hundreds of thousands of keys
    pub settings: HashMap<String, String>,
    /// message id filed in a bot's thread -> its 1-based log line (Hub.has_msg; Rto.resolve reads the text from the log)
    #[serde(with = "crate::ckpt::pairs")] msgs: HashMap<(String, String), usize>, bot_threads: HashSet<String>,
    /// messages being delivered (decided, not yet filed by the agent): a request sent again meanwhile is the same request
    #[serde(skip)] pub inflight: HashSet<(String, String)>,
    /// the uberbot's ops and registers, folded by the state lane's fold (UberOp, UberSet, machine.id, uber.offer, uber.link.*): only those lines reach it
    pub ub: crate::state::State,
    /// the mailroom (ML.Box)
    pub mail: super::mail::mailbox::Box,
    /// a bot's conversation depth (Hops: the hop of the turn it runs, the lowest waiting, and whether messages wait)
    pub hops: HashMap<String, Hops>,
    /// a bot thread's turn state, and the asks open on it (Bots.status)
    pub turns: HashMap<String, String>, pub asks: HashMap<String, (String, String, bool)>,
    /// a room's posts as log lines (1-based), oldest first: the text is read from the log when a tool asks
    pub room_lines: HashMap<String, Vec<usize>>,
    /// projects (id -> title, root, removed): project_list
    pub projects: std::collections::BTreeMap<String, (String, String, bool)>,
    seen: usize, line: usize,
}
/// the settings the bots lane reads: the new bot's provider and model, the mail switch, the uberbot
pub const SETTINGS: [&str; 8] = ["agent.provider", "agent.model", "mail.on", "uber.bot", "uber.uid", "person.tz", "person.tz.device", "tz.machine"];
/// whether a setting key is held: the list above and the zone rules the hub resolved (`tz.rule.<name>`)
fn kept(k: &str) -> bool { SETTINGS.contains(&k) || k.starts_with("tz.rule.") }

fn s<'a>(e: &'a Value, k: &str) -> &'a str { e.get(k).and_then(Value::as_str).unwrap_or("") }
fn n(e: &Value, k: &str) -> u64 { e.get(k).and_then(Value::as_u64).unwrap_or(0) }
fn b(e: &Value, k: &str) -> bool { e.get(k).and_then(Value::as_bool).unwrap_or(false) }

/// Room.members.of: one member per line, empty lines dropped
pub fn members_of(m: &str) -> Vec<String> { m.split('\n').filter(|x| !x.is_empty()).map(str::to_string).collect() }
/// Mem.at
pub fn mem_at(bot: &str, key: &str) -> String { format!("{bot}/{key}") }

impl World {
    pub fn bot(&self, id: &str) -> Option<&Bot> { self.bots.get(id) }
    /// Bots.live: made and not deleted
    pub fn live(&self, id: &str) -> bool { self.bots.get(id).map_or(false, |b| !b.deleted) }
    /// World.find.name (any case among live bots)
    pub fn named(&self, name: &str) -> Option<&Bot> { let l = name.to_lowercase(); self.bots.values().find(|b| !b.deleted && b.name.to_lowercase() == l) }
    pub fn room(&self, id: &str) -> Option<&Room> { self.rooms.get(id).filter(|r| !r.deleted) }
    pub fn has_post(&self, id: &str) -> bool { self.post_ids.contains(id) }
    pub fn has_msg(&self, thread: &str, msg: &str) -> bool { self.msgs.contains_key(&(thread.to_string(), msg.to_string())) }
    /// the log line (1-based) of a message filed in a bot's thread
    pub fn msg_line(&self, thread: &str, msg: &str) -> Option<usize> { self.msgs.get(&(thread.to_string(), msg.to_string())).copied() }
    /// M.Setting.get: a stored "" stays "" (only the families Setting.blank_gone names are removed by an empty value, and none is read here)
    pub fn setting(&self, k: &str, d: &str) -> String { self.settings.get(k).cloned().unwrap_or_else(|| d.to_string()) }
    /// Uber.is: the uberbot of this machine
    pub fn uber_is(&self, bot: &str) -> bool { !bot.is_empty() && !bot.contains('~') && self.settings.get("uber.bot").map_or(false, |b| b == bot) && self.live(bot) }
    /// Uber.bot: the uberbot's bot id, "" when none or gone
    pub fn uber_bot(&self) -> String { let b = self.settings.get("uber.bot").cloned().unwrap_or_default(); if self.uber_is(&b) { b } else { String::new() } }
    /// Triage.owner: the uberbot, with a thread
    pub fn triage_owner(&self, bot: &str) -> bool { self.uber_is(bot) && self.bots.get(bot).map_or(false, |b| !b.thread.is_empty()) }
    pub fn lines_seen(&self) -> usize { self.seen }
    // ---- the uberbot's registers (ubersync.bend) ----
    fn ubs(&self, k: &str) -> String { self.ub.settings.get(k).cloned().unwrap_or_default() }
    /// Ub.uid, Ub.me
    pub fn uid(&self) -> String { self.ubs("uber.uid") }
    pub fn me(&self) -> String { self.ubs("machine.id") }
    /// Ub.on: this machine has an id and an uberbot of its own to sync
    pub fn ub_on(&self) -> bool { !self.me().is_empty() && !self.uid().is_empty() && self.bots.get(&self.ubs("uber.bot")).map_or(false, |b| !b.deleted) }
    /// Ub.vv.of: the highest op of an origin this machine has
    pub fn vv(&self, uid: &str, origin: &str) -> u64 { self.ubs(&format!("uber.vv.{uid}.{origin}")).parse().unwrap_or(0) }
    pub fn hlc(&self, uid: &str) -> String { self.ubs(&format!("uber.hlc.{uid}")) }
    /// a register's op (JSON text), "" when it has none
    pub fn reg(&self, uid: &str, key: &str) -> String { self.ubs(&format!("uber.r.{uid}.{key}")) }
    /// Ub.lease: (holder, term)
    pub fn lease(&self) -> (String, u64) { let v: Value = serde_json::from_str(&self.reg(&self.uid(), "lease")).unwrap_or(Value::Null); (v["holder"].as_str().unwrap_or("").to_string(), v["term"].as_u64().unwrap_or(0)) }
    /// Ub.holder.me: this machine holds the conversation (and with no lease yet, the only machine there is does)
    pub fn holder_me(&self) -> bool { let h = self.lease().0; h.is_empty() || h == self.me() }
    /// Ub.pin: where a routine (global id) is pinned
    pub fn pin(&self, gid: &str) -> String { let v: Value = serde_json::from_str(&self.reg(&self.uid(), &format!("routine:{gid}"))).unwrap_or(Value::Null); v["pin"].as_str().unwrap_or("").to_string() }

    fn ckpt_path(store: &Store) -> std::path::PathBuf { crate::ckpt::dir(store.path()).join("bots.json") }
    /// The saved world (ckpt/bots.json) when it is of this log; `sync` folds what is past it.
    pub fn load(store: &Store) -> Option<World> { let w = crate::ckpt::load_json(&Self::ckpt_path(store), store, |w: &World| w.seen); if let Some(w) = &w { crate::ckpt::saved("bots", w.seen);
        if crate::ckpt::check_on() { let (s, n, a) = (store.clone(), w.seen, serde_json::to_value(w).unwrap_or_default()); std::thread::spawn(move || { let mut f = World::default(); let _ = f.sync_to(&s, n); crate::ckpt::check("bots world", a, serde_json::to_value(&f).unwrap_or_default()) }); } } w }
    /// Save what is folded (the log lines `seen` covers, which the caller's `sync` has just brought up to date).
    pub fn save(&self, store: &Store) { if crate::ckpt::due("bots", self.seen) { crate::ckpt::save_json(&Self::ckpt_path(store), store, self.seen, self); crate::ckpt::saved("bots", self.seen) } }
    pub fn sync(&mut self, store: &Store) -> anyhow::Result<()> { self.sync_to(store, store.len()) }
    pub fn sync_to(&mut self, store: &Store, total: usize) -> anyhow::Result<()> {
        while self.seen < total {
            let end = (self.seen + 4096).min(total);
            for (i, l) in store.range(self.seen, end)?.iter().enumerate() { self.line = self.seen + i + 1; self.apply_line(l) }
            self.seen = end;
        }
        Ok(())
    }
    pub fn apply_line(&mut self, line: &str) {
        let Some(t) = crate::session::event_type(line) else { return };
        const READ: [&str; 35] = ["BotSet", "BotDeleted", "RoomSet", "RoomDeleted", "RoomPosted", "HookSet", "HookHit", "RoutineSet", "RoutineDeleted", "RoutineFired",
            "MemorySaved", "MemoryForgot", "SpaceSet", "SpacePageSet", "PeerSet", "RoutinePlan", "RoutineSlot", "RoutineDone", "SettingSet", "MessagePosted", "ThreadCreated", "TaskSet", "UberSet", "UberOp", "ConvSet", "MailPosted", "MailMarked", "MailTagged", "TurnChanged", "AskOpened", "AskClosed", "QueueTaken", "BotWoke", "ProjectCreated", "ProjectRemoved"];
        if !READ.contains(&t) { return }
        // cheap tests before a parse: most lines of a big log are messages of other threads and settings nobody here reads
        if t == "SettingSet" && !["agent.", "mail.on", "uber.", "person.tz", "tz.", "machine.id"].iter().any(|k| line.contains(k)) { return }
        if t == "TurnChanged" || t == "QueueTaken" {
            let th = line.split_once("\"thread\":\"").and_then(|(_, r)| r.split('"').next()).unwrap_or("");
            if !self.bot_threads.contains(th) { return }
        }
        if t == "MessagePosted" {
            let th = line.split_once("\"thread\":\"").and_then(|(_, r)| r.split('"').next()).unwrap_or("");
            if !self.bot_threads.contains(th) { return }
        }
        if let Ok(e) = serde_json::from_str::<Value>(line) { self.apply(t, &e) }
    }
    fn routine_mut(&mut self, id: &str) -> &mut Routine {
        self.routines.entry(id.to_string()).or_insert_with(|| Routine { id: id.into(), bot: String::new(), name: String::new(), cron: String::new(), prompt: String::new(), on: false, last: 0, created: 0, deleted: true,
            once: 0, tz: String::new(), kind: String::new(), done: false, slot: 0 })
    }
    pub fn apply(&mut self, t: &str, e: &Value) {
        let id = s(e, "id").to_string(); let at = n(e, "at");
        match t {
            "BotSet" => {
                let old = self.bots.get(&id).cloned();
                let (created, deleted, th) = old.map_or((0, true, String::new()), |o| (o.created, o.deleted, o.thread));
                let thread = if s(e, "thread").is_empty() { th } else { s(e, "thread").to_string() };
                self.bot_threads.insert(thread.clone());
                self.mail.bot_named(&id, s(e, "name"));
                self.bots.insert(id.clone(), Bot { id, name: s(e, "name").into(), thread, look: n(e, "look"), persona: s(e, "persona").into(), created: if deleted { at } else { created }, deleted: false });
            }
            "BotDeleted" => { self.mail.bot_deleted(&id); if let Some(x) = self.bots.get_mut(&id) { x.deleted = true } else { self.bots.insert(id.clone(), Bot { id, name: String::new(), thread: String::new(), look: 0, persona: String::new(), created: 0, deleted: true }); } }
            "RoomSet" => {
                let (created, deleted) = self.rooms.get(&id).map_or((0, true), |r| (r.created, r.deleted));
                self.mail.room_set(&id, s(e, "name"), s(e, "members"), at);
                self.rooms.insert(id.clone(), Room { id, name: s(e, "name").into(), members: members_of(s(e, "members")), created: if deleted { at } else { created }, deleted: false });
            }
            "RoomDeleted" => { if let Some(r) = self.rooms.get_mut(&id) { r.deleted = true } else { self.rooms.insert(id.clone(), Room { id, name: String::new(), members: vec![], created: 0, deleted: true }); } }
            "ProjectCreated" => { self.projects.insert(id, (s(e, "title").into(), s(e, "root").into(), false)); }
            "ProjectRemoved" => { if let Some(p) = self.projects.get_mut(&id) { p.2 = true } }
            "TurnChanged" => { self.turns.insert(s(e, "thread").into(), s(e, "state").into()); }
            "QueueTaken" => { let th = s(e, "thread"); if let Some(b) = self.bots.values().find(|b| !b.deleted && b.thread == th).map(|b| b.id.clone()) { let c = self.hops.entry(b).or_default(); *c = if c.held { Hops { now: c.wait, wait: 0, held: false } } else { Hops { now: c.now, wait: 0, held: false } } } }
            "AskOpened" => { self.asks.insert(id.clone(), (s(e, "thread").into(), s(e, "kind").into(), true)); }
            "AskClosed" => { if let Some(a) = self.asks.get_mut(&id) { a.2 = false } }
            "BotWoke" => { let bot = s(e, "bot").to_string(); let busy = self.bots.get(&bot).map_or(false, |b| self.turns.get(&b.thread).map_or(false, |t| t == "running")); let h = n(e, "hop");
                let c = self.hops.entry(bot).or_default(); *c = if busy { Hops { now: c.now, wait: if c.held { c.wait.min(h) } else { h }, held: true } } else { Hops { now: h, wait: 0, held: false } }; }
            "RoomPosted" => { self.room_lines.entry(s(e, "room").to_string()).or_default().push(self.line); if self.settings.get("mail.on").map_or(true, |v| v != "on") { self.mail.room_post(&id, s(e, "room"), s(e, "from"), s(e, "text"), at); } self.post_ids.insert(id); }
            "ConvSet" => { self.mail.conv_set(&id, s(e, "subject"), s(e, "kind"), super::mail::mailbox::lines_of(s(e, "parts")), s(e, "project"), super::mail::mailbox::lines_of(s(e, "tags")), at); }
            "MailPosted" => { self.mail.mail(super::mail::Hdr { id, conv: s(e, "conv").into(), from: s(e, "from").into(), to: s(e, "to").into(), kind: s(e, "kind").into(), r#ref: s(e, "ref").into(), want: s(e, "want").into(), head: s(e, "head").into(), size: n(e, "size"), at }); }
            "MailMarked" => { self.mail.marked(s(e, "bot"), s(e, "conv"), s(e, "status"), n(e, "seen"), n(e, "until")); }
            "MailTagged" => { self.mail.tagged(s(e, "bot"), s(e, "conv"), super::mail::mailbox::lines_of(s(e, "tags"))); }
            "HookSet" => {
                let old = self.hooks.get(&id).cloned();
                let (bot, name) = (if s(e, "bot").is_empty() { old.as_ref().map(|o| o.bot.clone()).unwrap_or_default() } else { s(e, "bot").into() },
                                   if s(e, "name").is_empty() { old.as_ref().map(|o| o.name.clone()).unwrap_or_default() } else { s(e, "name").into() });
                let (created, last, count) = old.map_or((at, 0, 0), |o| (if o.revoked { at } else { o.created }, o.last, o.count));
                self.hooks.insert(id.clone(), Hook { id, bot, name, created, revoked: b(e, "revoked"), last, count });
            }
            "HookHit" => { if let Some(h) = self.hooks.get_mut(&id) { h.last = at; h.count += 1 } }
            "RoutineSet" => {
                let old = self.routines.get(&id).cloned();
                let (was_on, deleted, last, created, bot, once, tz, kind, done, slot) = old.map_or((false, true, 0, 0, String::new(), 0, String::new(), String::new(), false, 0),
                    |o| (o.on, o.deleted, o.last, o.created, o.bot, o.once, o.tz, o.kind, o.done, o.slot));
                let was = was_on && !deleted; let on = b(e, "on");
                let nb = if s(e, "bot").is_empty() { bot } else { s(e, "bot").into() };
                self.routines.insert(id.clone(), Routine { id, bot: nb, name: s(e, "name").into(), cron: s(e, "cron").into(), prompt: s(e, "prompt").into(), on,
                    last: if !was && on { at } else { last }, created: if deleted { at } else { created }, deleted: false, once, tz, kind, done, slot: if !was && on { 0 } else { slot } });
            }
            "RoutineDeleted" => { self.routine_mut(&id).deleted = true }
            "RoutineFired" => { self.routine_mut(&id).last = at }
            "RoutinePlan" => { let r = self.routine_mut(&id); r.once = n(e, "once"); r.tz = s(e, "tz").into(); r.kind = s(e, "kind").into(); r.done = false; r.slot = 0 }
            "RoutineSlot" => { self.routine_mut(&id).slot = n(e, "slot") }
            "RoutineDone" => { self.routine_mut(&id).done = true }
            "MemorySaved" => {
                let (bot, key) = (s(e, "bot"), s(e, "key")); let k = mem_at(bot, key);
                let created = self.mems.get(&k).map_or(0, |m| m.created);
                self.mems.insert(k, Mem { bot: bot.into(), key: key.into(), kind: s(e, "kind").into(), text: s(e, "text").into(), tags: s(e, "tags").into(), created: if created == 0 { at } else { created }, updated: at });
            }
            "MemoryForgot" => { self.mems.remove(&mem_at(s(e, "bot"), s(e, "key"))); }
            "SpaceSet" => { self.spaces.insert(s(e, "bot").into(), s(e, "spec").into()); }
            "SpacePageSet" => { self.pages.insert(s(e, "bot").into(), (s(e, "html").into(), at)); }
            "PeerSet" => {
                let old = self.peers.get(&id).cloned();
                let (name, url) = (if s(e, "name").is_empty() { old.as_ref().map(|o| o.name.clone()).unwrap_or_default() } else { s(e, "name").into() },
                                   if s(e, "url").is_empty() { old.as_ref().map(|o| o.url.clone()).unwrap_or_default() } else { s(e, "url").into() });
                self.peers.insert(id.clone(), Peer { id, name, url, created: old.map_or(at, |o| if o.revoked { at } else { o.created }), revoked: b(e, "revoked") });
            }
            "UberSet" => { self.settings.insert("uber.bot".into(), s(e, "bot").into()); self.settings.insert("uber.uid".into(), s(e, "uid").into()); self.ub.apply(e); }
            "UberOp" => { self.ub.apply(e); }
            "SettingSet" => { let k = s(e, "key"); if k == "machine.id" || k == "uber.offer" || k.starts_with("uber.link.") { self.ub.apply(e); } if kept(k) { self.settings.insert(k.into(), s(e, "value").into()); } }
            "MessagePosted" => { let th = s(e, "thread"); if self.bot_threads.contains(th) { self.msgs.insert((th.into(), s(e, "msg").into()), self.line); } }
            _ => {}
        }
    }
}

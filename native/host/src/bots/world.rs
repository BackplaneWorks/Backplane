//! The bots world folded from the log: src/core/bot.bend `World` and `M.State.world` (model.bend), for the events a
//! bots request reads. Only this lane reads it; the presenter folds the same events for the screens.
use crate::store::Store;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct Bot { pub id: String, pub name: String, pub thread: String, pub look: u64, pub persona: String, pub created: u64, pub deleted: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Room { pub id: String, pub name: String, pub members: Vec<String>, pub created: u64, pub deleted: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Mem { pub bot: String, pub key: String, pub kind: String, pub text: String, pub tags: String, pub created: u64, pub updated: u64 }
#[derive(Clone, Debug, PartialEq)]
pub struct Routine { pub id: String, pub bot: String, pub name: String, pub cron: String, pub prompt: String, pub on: bool, pub last: u64, pub created: u64, pub deleted: bool,
    pub once: u64, pub tz: String, pub kind: String, pub done: bool, pub slot: u64 }
#[derive(Clone, Debug, PartialEq)]
pub struct Hook { pub id: String, pub bot: String, pub name: String, pub created: u64, pub revoked: bool, pub last: u64, pub count: u64 }
#[derive(Clone, Debug, PartialEq)]
pub struct Peer { pub id: String, pub name: String, pub url: String, pub created: u64, pub revoked: bool }

#[derive(Default)]
pub struct World {
    pub bots: HashMap<String, Bot>, pub rooms: HashMap<String, Room>, post_ids: HashSet<String>,
    pub mems: HashMap<String, Mem>, pub spaces: HashMap<String, String>, pub pages: HashMap<String, (String, u64)>,
    pub routines: HashMap<String, Routine>, pub hooks: HashMap<String, Hook>, pub peers: HashMap<String, Peer>,
    /// only the settings a bots request reads (SETTINGS): a log of a million lines holds hundreds of thousands of keys
    pub settings: HashMap<String, String>,
    /// message id filed in a bot's thread -> its 1-based log line (Hub.has_msg; Rto.resolve reads the text from the log)
    msgs: HashMap<(String, String), usize>, bot_threads: HashSet<String>,
    /// messages being delivered (decided, not yet filed by the agent): a request sent again meanwhile is the same request
    pub inflight: HashSet<(String, String)>,
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
    /// Triage.owner: the uberbot, with a thread
    pub fn triage_owner(&self, bot: &str) -> bool { self.uber_is(bot) && self.bots.get(bot).map_or(false, |b| !b.thread.is_empty()) }
    pub fn lines_seen(&self) -> usize { self.seen }

    pub fn sync(&mut self, store: &Store) -> anyhow::Result<()> {
        let total = store.len();
        while self.seen < total {
            let end = (self.seen + 4096).min(total);
            for (i, l) in store.range(self.seen, end)?.iter().enumerate() { self.line = self.seen + i + 1; self.apply_line(l) }
            self.seen = end;
        }
        Ok(())
    }
    pub fn apply_line(&mut self, line: &str) {
        let Some(t) = line.strip_prefix("{\"$\":\"").and_then(|r| r.split('"').next()) else { return };
        const READ: [&str; 23] = ["BotSet", "BotDeleted", "RoomSet", "RoomDeleted", "RoomPosted", "HookSet", "HookHit", "RoutineSet", "RoutineDeleted", "RoutineFired",
            "MemorySaved", "MemoryForgot", "SpaceSet", "SpacePageSet", "PeerSet", "RoutinePlan", "RoutineSlot", "RoutineDone", "SettingSet", "MessagePosted", "ThreadCreated", "TaskSet", "UberSet"];
        if !READ.contains(&t) { return }
        // cheap tests before a parse: most lines of a big log are messages of other threads and settings nobody here reads
        if t == "SettingSet" && !["agent.", "mail.on", "uber.", "person.tz", "tz."].iter().any(|k| line.contains(k)) { return }
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
                self.bots.insert(id.clone(), Bot { id, name: s(e, "name").into(), thread, look: n(e, "look"), persona: s(e, "persona").into(), created: if deleted { at } else { created }, deleted: false });
            }
            "BotDeleted" => { if let Some(x) = self.bots.get_mut(&id) { x.deleted = true } else { self.bots.insert(id.clone(), Bot { id, name: String::new(), thread: String::new(), look: 0, persona: String::new(), created: 0, deleted: true }); } }
            "RoomSet" => {
                let (created, deleted) = self.rooms.get(&id).map_or((0, true), |r| (r.created, r.deleted));
                self.rooms.insert(id.clone(), Room { id, name: s(e, "name").into(), members: members_of(s(e, "members")), created: if deleted { at } else { created }, deleted: false });
            }
            "RoomDeleted" => { if let Some(r) = self.rooms.get_mut(&id) { r.deleted = true } else { self.rooms.insert(id.clone(), Room { id, name: String::new(), members: vec![], created: 0, deleted: true }); } }
            "RoomPosted" => { self.post_ids.insert(id); }
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
            "UberSet" => { self.settings.insert("uber.bot".into(), s(e, "bot").into()); self.settings.insert("uber.uid".into(), s(e, "uid").into()); }
            "SettingSet" => { let k = s(e, "key"); if kept(k) { self.settings.insert(k.into(), s(e, "value").into()); } }
            "MessagePosted" => { let th = s(e, "thread"); if self.bot_threads.contains(th) { self.msgs.insert((th.into(), s(e, "msg").into()), self.line); } }
            _ => {}
        }
    }
}

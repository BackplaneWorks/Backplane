//! The bots' own MCP tools (src/core/bothub.bend `Bots.call.k`, `Bots.names`): what a bot's agent calls to talk to other bots, keep memory, set its space and
//! routines, and remind the person. Only a bot's thread has them (`Bots.route`: a thread that is no bot's does not list them). Every tool decides under the world
//! lock, logs through `World::also` (the uberbot's ops go beside its changes) and answers (ok, body) as `Mcp.content` shapes it.
//! Ported: bot_whoami, bot_list, bot_send, room_list, room_create, room_post, room_read, memory_*, space_set/get/page, routine_list/set/delete, project_list,
//! remind_person, person_timezone. Not yet (PARITY): bot_create, plane_read, notify_person, task_*, voice_dictionary_add, project_create.
use super::clock::{Zones, TzRule};
use super::post::{Pending, RoomRef};
use super::rpc::{now_s, Bots};
use super::routines::{plan, Pick};
use super::sched_text::{person_rule, rel, routine_json, saved, zone_source};
use super::world::{Bot, Routine, World};
use serde_json::{json, Value};
use std::sync::Arc;

pub const TOOLS_JSON: &str = include_str!("tools.json");
pub const NAMES: &[&str] = &["bot_whoami", "bot_list", "bot_send", "room_list", "room_create", "room_post", "room_read", "memory_save", "memory_recall", "memory_forget", "memory_list",
    "space_set", "space_get", "space_page", "routine_list", "routine_set", "routine_delete", "project_list", "bot_create", "remind_person", "person_timezone"];
const HOP_MAX: u64 = 6;
const MAX_PAGE: usize = 262_144;

type Res = (bool, Value);
fn fail(m: &str) -> Res { (false, json!({"error":m})) }
fn said(m: &str) -> Res { (true, json!({"ok":true,"note":m})) }
fn gs<'a>(a: &'a Value, k: &str) -> &'a str { a.get(k).and_then(Value::as_str).unwrap_or("") }
fn gb(a: &Value, k: &str, d: bool) -> bool { a.get(k).and_then(Value::as_bool).unwrap_or(d) }
/// Bots.nat: a number (or its digits) in the arguments, else the default
fn gn(a: &Value, k: &str, d: usize) -> usize { match a.get(k) { Some(Value::Number(n)) => n.as_u64().map_or(d, |x| x as usize), _ => d } }

/// The tool definitions an agent is shown in a bot's thread: the JS hub's own, in its order.
pub fn defs() -> Vec<Value> { serde_json::from_str(TOOLS_JSON).unwrap_or_default() }
pub fn owns(name: &str) -> bool { NAMES.contains(&name) }

/// Words.of: runs of letters and digits, lower case, three or more long (Mem.score counts a repeated word each time)
fn words(s: &str) -> Vec<String> {
    let mut out = vec![]; let mut cur = String::new();
    for c in s.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() { cur.extend(c.to_lowercase()) } else { if cur.chars().count() >= 3 { out.push(std::mem::take(&mut cur)) } else { cur.clear() } }
    }
    out
}
const STOP: [&str; 24] = ["the", "and", "for", "you", "your", "with", "that", "this", "are", "was", "can", "will", "what", "how", "please", "from", "have", "not", "but", "about", "into", "our", "any", "all"];
fn score(qs: &[String], key: &str, tags: &str, text: &str) -> usize {
    let (hay, text) = (format!("{key} {tags}").to_lowercase(), text.to_lowercase());
    qs.iter().filter(|w| !STOP.contains(&w.as_str())).map(|w| (if hay.contains(w.as_str()) { 3 } else { 0 }) + (if text.contains(w.as_str()) { 1 } else { 0 })).sum()
}
fn mem_json(m: &super::world::Mem) -> Value { json!({"key":m.key,"kind":m.kind,"text":m.text,"tags":m.tags,"updated":m.updated}) }
/// the bot's memories, newest first (the key decides equal times)
fn mems<'a>(w: &'a World, bot: &str) -> Vec<&'a super::world::Mem> {
    let mut v: Vec<_> = w.mems.values().filter(|m| m.bot == bot).collect(); v.sort_by(|a, c| c.updated.cmp(&a.updated).then_with(|| a.key.cmp(&c.key))); v
}
/// Bots.status: what a bot's thread is doing (a thread whose worktree is not made yet reads ready here: PARITY)
fn state(w: &World, thread: &str) -> &'static str {
    let open = |k: &str| w.asks.values().any(|(t, kind, open)| *open && t == thread && kind == k);
    if open("approval") { "approval" } else if open("question") { "input" } else { match w.turns.get(thread).map(String::as_str) { Some("running") => "working", Some("failed") => "failed", _ => "ready" } }
}
fn room_json(r: &super::world::Room) -> Value { json!({"id":r.id,"name":r.name,"members":r.members}) }
/// World.rooms: live and not a direct room, by id
fn rooms(w: &World) -> Vec<&super::world::Room> { let mut v: Vec<_> = w.rooms.values().filter(|r| !r.deleted && !r.id.starts_with("dm:")).collect(); v.sort_by(|a, b| a.id.cmp(&b.id)); v }
/// World.find_room: a room by id or by name in any case
fn find_room<'a>(w: &'a World, name: &str) -> Option<&'a super::world::Room> { let l = name.to_lowercase(); rooms(w).into_iter().find(|r| r.id == name || r.name.to_lowercase() == l) }
fn routines_of<'a>(w: &'a World, bot: &str) -> Vec<&'a Routine> { let mut v: Vec<_> = w.routines.values().filter(|r| !r.deleted && r.bot == bot).collect(); v.sort_by(|a, b| a.id.cmp(&b.id)); v }
/// the log position in a hub id (`b<secs>-<seq>`): bots made in one second keep the order they were made in
fn id_seq(id: &str) -> u64 { id.rsplit('-').next().and_then(|n| n.parse().ok()).unwrap_or(0) }
/// Room.dm
fn dm(a: &str, b: &str) -> String { if a <= b { format!("dm:{a}:{b}") } else { format!("dm:{b}:{a}") } }
fn held_text(t: &str) -> String { format!("[held: {HOP_MAX} messages between bots without a person, so no bot heard this]\n{t}") }
const HELD_NOTE: &str = "held: this exchange between bots is 6 messages deep, so no bot was woken. Your message is in the room for the person to read; don't send it again or anywhere else. A message from a person, a routine or a webhook starts a new exchange.";
/// Held.res: a held message's answer replaces the plain one
fn held_res(r: Res) -> Res { if r.0 { (true, json!({"ok":true,"held":true,"note":HELD_NOTE})) } else { r } }
/// Bots.members: the maker first, then the names given as ids (a name that is no bot is dropped; name@machine stays), one per line
fn members(w: &World, me: &str, text: &str) -> String {
    let mut v = vec![me.to_string()];
    for x in text.replace([',', '\n'], " ").split(' ').filter(|x| !x.is_empty()) {
        let id = if x.contains('@') { x.to_string() } else { w.named(x).map(|b| b.id.clone()).or_else(|| w.bot(x).filter(|b| !b.deleted).map(|b| b.id.clone())).unwrap_or_default() };
        if !id.is_empty() { v.push(id) }
    }
    v.join("\n")
}

impl Bots {
    /// The bot whose thread this is, if any
    pub async fn tool_bot(&self, thread: &str) -> bool { self.world().await.bots.values().any(|b| !b.deleted && b.thread == thread) }

    /// One tools/call from the agent of `thread`: (ok, body)
    pub async fn tool_call(self: &Arc<Self>, thread: &str, name: &str, a: &Value) -> Res {
        let mut w = self.world().await;
        let mut later: Option<(Pending, u64)> = None;
        let r = self.tool_decide(&w, thread, name, a, &mut later).await;
        drop(w);
        // the members of a room hear a post only once the lock is let go (a delivery can take a while)
        if let Some((p, hop)) = later { self.post_finish(p, hop).await }
        r
    }

    async fn tool_decide(self: &Arc<Self>, w: &World, thread: &str, name: &str, a: &Value, later: &mut Option<(Pending, u64)>) -> Res {
        let Some(b) = w.bots.values().find(|b| !b.deleted && b.thread == thread).cloned() else { return fail("unknown tool") };
        let now = now_s();
        match name {
            "bot_whoami" => {
                let z = Zones { settings: &w.settings }; let hop = w.hops.get(&b.id).map_or(0, |h| h.now);
                let mut hooks: Vec<_> = w.hooks.values().filter(|h| h.bot == b.id && !h.revoked).collect(); hooks.sort_by(|a, c| a.id.cmp(&c.id));
                (true, json!({"name":b.name,"id":b.id,"hop":hop,"maxHop":HOP_MAX,
                    "rooms":rooms(&w).into_iter().filter(|r| r.members.contains(&b.id)).map(room_json).collect::<Vec<_>>(),
                    "routines":routines_of(&w, &b.id).into_iter().map(|r| routine_json(&z, r, now)).collect::<Vec<_>>(),
                    "webhooks":hooks.into_iter().map(|h| json!({"id":h.id,"name":h.name,"path":format!("/hook/{}",h.id),"last":h.last,"count":h.count})).collect::<Vec<_>>()}))
            }
            "bot_list" => {
                let mut v: Vec<_> = w.bots.values().filter(|b| !b.deleted).collect(); v.sort_by_key(|x| (x.created, id_seq(&x.id), x.id.clone()));
                (true, json!({"here":v.into_iter().map(|x| json!({"name":x.name,"id":x.id,"state":state(&w, &x.thread)})).collect::<Vec<_>>(),"elsewhere":[]}))
            }
            "bot_send" => self.t_send(w, &b, now, a, later).await,
            "room_list" => (true, Value::Array(rooms(&w).into_iter().map(room_json).collect())),
            "room_create" => {
                let ms = members(&w, &b.id, gs(a, "members")); let rid = self.hub_id("r", now);
                let ev = json!({"$":"RoomSet","id":rid,"name":a.get("name").and_then(Value::as_str).unwrap_or("room"),"members":ms,"at":now});
                self.t_commit(w, now, vec![ev]).await.map_or_else(|e| fail(&e), |_| said("room made"))
            }
            "room_post" => {
                let Some(room) = find_room(&w, gs(a, "room")).cloned() else { return fail("no such room (see room_list)") };
                let rr = RoomRef { id: room.id, name: room.name, members: room.members };
                self.t_post(w, &b, now, &rr, gs(a, "text"), vec![], later, |_| said("posted")).await
            }
            "room_read" => {
                let room = gs(a, "room"); let n = gn(a, "limit", 30);
                let id = find_room(&w, room).map(|r| r.id.clone()).unwrap_or_else(|| dm(&b.name, &w.named(room).map_or(room.to_string(), |o| o.name.clone())));
                let lines = w.room_lines.get(&id).cloned().unwrap_or_default();
                let take: Vec<usize> = lines.iter().rev().take(n).rev().copied().collect();
                let mut out = vec![];
                for l in take { if let Ok(raw) = self.hub.store.get(l - 1) { if let Ok(e) = serde_json::from_str::<Value>(&raw) { out.push(json!({"from":e["from"],"text":e["text"],"at":e["at"]})) } } }
                (true, Value::Array(out))
            }
            "memory_save" => {
                let key = super::rpc::mem_key(gs(a, "key"));
                let ev = json!({"$":"MemorySaved","bot":b.id,"key":key,"kind":super::rpc::mem_kind(gs(a, "kind")),"text":gs(a, "text"),"tags":gs(a, "tags"),"at":now});
                self.t_commit(w, now, vec![ev]).await.map_or_else(|e| fail(&e), |_| said(&format!("remembered {key}")))
            }
            "memory_recall" => {
                let qs = words(gs(a, "query")); let mut hits: Vec<_> = mems(&w, &b.id).into_iter().map(|m| (score(&qs, &m.key, &m.tags, &m.text), m)).filter(|(s, _)| *s > 0).collect();
                hits.sort_by(|x, y| y.0.cmp(&x.0).then_with(|| y.1.updated.cmp(&x.1.updated)).then_with(|| x.1.key.cmp(&y.1.key)));
                (true, Value::Array(hits.into_iter().take(gn(a, "limit", 8)).map(|(_, m)| mem_json(m)).collect()))
            }
            "memory_forget" => self.t_commit(w, now, vec![json!({"$":"MemoryForgot","bot":b.id,"key":gs(a, "key"),"at":now})]).await.map_or_else(|e| fail(&e), |_| said("forgotten")),
            "memory_list" => { let k = gs(a, "kind"); (true, Value::Array(mems(&w, &b.id).into_iter().filter(|m| k.is_empty() || m.kind == k).map(mem_json).collect())) }
            "space_set" => {
                let spec = gs(a, "spec");
                if !lenient_json(spec) { return fail("that is not valid JSON") }
                self.t_commit(w, now, vec![json!({"$":"SpaceSet","bot":b.id,"spec":spec,"at":now})]).await.map_or_else(|e| fail(&e), |_| said("your space is updated"))
            }
            "space_get" => (true, json!({"spec":w.spaces.get(&b.id).cloned().unwrap_or_default(),"page":w.pages.get(&b.id).map(|p| p.0.clone()).unwrap_or_default()})),
            "space_page" => {
                let html = gs(a, "html");
                if html.chars().count() > MAX_PAGE { return fail("the page is over 256 KB") }
                self.t_commit(w, now, vec![json!({"$":"SpacePageSet","bot":b.id,"html":html,"at":now})]).await.map_or_else(|e| fail(&e), |_| said(if html.is_empty() { "your page is removed" } else { "your page is updated" }))
            }
            "routine_list" => { let z = Zones { settings: &w.settings }; (true, Value::Array(routines_of(&w, &b.id).into_iter().map(|r| routine_json(&z, r, now)).collect())) }
            "routine_set" => self.t_routine_set(&w, &b, now, a).await,
            "routine_delete" => {
                let id = gs(a, "id");
                if !w.routines.get(id).map_or(false, |r| r.bot == b.id) { return fail("that routine is not yours") }
                self.t_commit(w, now, vec![json!({"$":"RoutineDeleted","id":id,"at":now})]).await.map_or_else(|e| fail(&e), |_| said("deleted"))
            }
            "project_list" => (true, Value::Array(w.projects.iter().filter(|(_, p)| !p.2).map(|(id, p)| json!({"id":id,"name":p.0,"folder":p.1})).collect())),
            "bot_create" => self.t_spawn(w, &b, now, a).await,
            "remind_person" => {
                let text = gs(a, "text").trim().to_string();
                if text.is_empty() { return fail("say what to remind them of (text)") }
                let z = Zones { settings: &w.settings };
                match plan(&z, now, gs(a, "tz"), "", gs(a, "at"), None) {
                    Pick::Bad(m) => fail(&m),
                    Pick::Plan { once, tz, .. } => {
                        let rid = self.hub_id("o", now); let rule = person_rule(&z, now);
                        let note = format!("I'll remind them {} ({}): {text}", rule.show(once), rel(now, once));
                        let evs = vec![json!({"$":"RoutineSet","id":rid,"bot":b.id,"name":text.chars().take(48).collect::<String>(),"cron":"","prompt":text,"on":true,"at":now}),
                            json!({"$":"RoutinePlan","id":rid,"once":once,"tz":tz,"kind":"remind","at":now})];
                        self.t_commit(w, now, evs).await.map_or_else(|e| fail(&e), |_| said(&note))
                    }
                }
            }
            "person_timezone" => self.t_zone(w, now, gs(a, "zone").trim()).await,
            _ => fail("unknown tool"),
        }
    }

    /// Commit a tool's changes (with the uberbot's ops beside them); the lock is held by the caller
    async fn t_commit(&self, w: &World, now: u64, evs: Vec<Value>) -> Result<(), String> { self.hub.commit_many(&w.also(now, evs)).await.map(|_| ()).map_err(|e| format!("{e:#}")) }

    /// A bot's post in a room: within the depth bound the post goes out at the next hop; past it, held (kept in the room for the person, heard by no bot)
    async fn t_post(&self, w: &World, b: &Bot, now: u64, room: &RoomRef, text: &str, before: Vec<Value>, later: &mut Option<(Pending, u64)>, said: impl Fn(&RoomRef) -> Res) -> Res {
        let hop = w.hops.get(&b.id).map_or(0, |h| h.now) + 1;
        let (hop, text, held) = if hop <= HOP_MAX { (hop, text.to_string(), false) } else { (HOP_MAX + 1, held_text(text), true) };
        let mid = self.hub_id("p", now);
        let pending = match self.post_decide(w, now, &b.thread, &b.name, &b.id, room, &text, hop, &mid, before).await { Ok(p) => p, Err(e) => return fail(&e) };
        *later = Some((pending, hop));
        let r = said(room); if held { held_res(r) } else { r }
    }

    async fn t_send(&self, w: &World, b: &Bot, now: u64, a: &Value, later: &mut Option<(Pending, u64)>) -> Res {
        let (to, direct, topic) = (gs(a, "to"), gb(a, "direct", false), gs(a, "topic").trim());
        if to.contains('@') { return fail(&format!("no linked machine called {} (see bot_list)", to.rsplit('@').next().unwrap_or(""))) } // machines are the net lane's: PARITY
        let Some(t) = w.named(to).cloned().or_else(|| w.bot(to).filter(|x| !x.deleted).cloned()) else { return fail("no bot by that name here (see bot_list)") };
        if t.id == b.id { return fail("that is you") }
        let text = gs(a, "text");
        if direct {
            let rr = RoomRef { id: dm(&b.name, &t.name), name: t.name.clone(), members: vec![t.id.clone()] };
            let n = t.name.clone();
            return self.t_post(w, b, now, &rr, text, vec![], later, move |_| said(&format!("sent to {n}"))).await
        }
        // Req.send: the request goes to the room the two bots already have about this topic, else a new one
        let name = if topic.is_empty() { let (x, y) = (b.name.to_lowercase(), t.name.to_lowercase()); if x <= y { format!("{x} + {y}") } else { format!("{y} + {x}") } } else { topic.chars().take(60).collect() };
        let found = rooms(w).into_iter().find(|r| r.name.to_lowercase() == name.to_lowercase() && r.members.len() == 2 && r.members.contains(&b.id) && r.members.contains(&t.id)).cloned();
        let (rr, before) = match found {
            Some(r) => (RoomRef { id: r.id, name: r.name, members: r.members }, vec![]),
            None => { let id = self.hub_id("r", now); let ms = vec![b.id.clone(), t.id.clone()];
                let ev = json!({"$":"RoomSet","id":id,"name":name,"members":ms.join("\n"),"at":now}); (RoomRef { id, name, members: ms }, vec![ev]) }
        };
        self.t_post(w, b, now, &rr, text, before, later, |r| (true, json!({"ok":true,"room":r.id,"roomName":r.name,
            "note":format!("posted in #{}, where the person can follow it; keep everything about this request there (bot_send with the same topic, or room_post)", r.name)}))).await
    }

    async fn t_routine_set(&self, w: &World, b: &Bot, now: u64, a: &Value) -> Res {
        let rid_in = gs(a, "id");
        let ex = w.routines.get(rid_in).filter(|r| !r.deleted);
        if !rid_in.is_empty() && !w.routines.get(rid_in).map_or(false, |r| r.bot == b.id) { return fail("that routine is not yours") }
        let z = Zones { settings: &w.settings };
        match plan(&z, now, gs(a, "tz"), gs(a, "schedule"), gs(a, "at"), ex) {
            Pick::Bad(m) => fail(&m),
            Pick::Plan { cron, once, tz } => {
                let rid = if rid_in.is_empty() { self.hub_id("o", now) } else { rid_in.to_string() };
                let (name, prompt, on) = (a.get("name").and_then(Value::as_str).unwrap_or("routine"), gs(a, "prompt"), gb(a, "on", true));
                let kind = w.routines.get(rid_in).map_or("", |r| r.kind.as_str());
                let evs = vec![json!({"$":"RoutineSet","id":rid,"bot":b.id,"name":name,"cron":cron,"prompt":prompt,"on":on,"at":now}),
                    json!({"$":"RoutinePlan","id":rid,"once":once,"tz":tz,"kind":kind,"at":now})];
                // the routine as it will stand (World.routine_set / routine_plan)
                let old = w.routines.get(&rid).cloned();
                let (was_on, deleted, last, created, slot) = old.map_or((false, true, 0, 0, 0), |o| (o.on, o.deleted, o.last, o.created, o.slot));
                let was = was_on && !deleted;
                let after = Routine { id: rid.clone(), bot: b.id.clone(), name: name.into(), cron, prompt: prompt.into(), on, last: if !was && on { now } else { last }, created: if deleted { now } else { created }, deleted: false,
                    once, tz, kind: kind.into(), done: false, slot: 0 };
                let _ = slot;
                let note = format!("routine saved: {}", saved(&z, &after, now));
                self.t_commit(w, now, evs).await.map_or_else(|e| fail(&e), |_| said(&note))
            }
        }
    }

    /// Bots.spawn: a sub-bot (a persistent bot of its own, this one its maker): at most Spawn.max levels below a person-made bot, Spawn.kids of them each
    async fn t_spawn(self: &Arc<Self>, w: &World, b: &Bot, now: u64, a: &Value) -> Res {
        const MAX_DEPTH: u64 = 2; const KIDS: usize = 8;
        let name = gs(a, "name").trim().to_string();
        // Spawn.depth: how far below a person-made bot this one sits (walks at most four makers)
        let (mut depth, mut cur) = (0u64, b.id.clone());
        for _ in 0..4 { let p = self.hub.setting(&format!("bot.parent.{cur}")).await.unwrap_or_default(); if !p.is_empty() { depth += 1 } cur = p }
        if depth + 1 > MAX_DEPTH { return fail("bots made by bots go at most 2 levels deep; ask the person, or a bot higher up") }
        let mut kids = 0; for x in w.bots.values().filter(|x| !x.deleted) { if self.hub.setting(&format!("bot.parent.{}", x.id)).await.unwrap_or_default() == b.id { kids += 1 } }
        if kids >= KIDS { return fail("you keep 8 sub-bots already; ask the person to retire one") }
        if !super::rpc::name_ok(&name) { return fail("a bot's name is 1 to 24 letters, digits, - or _") }
        if w.named(&name).is_some() { return fail(&format!("a bot named {name} already exists")) }
        let (bot, th) = (self.hub_id("b", now), self.hub_id("t", now));
        let provider = { let p = gs(a, "provider"); if p.is_empty() { self.hub.model().await.threads.get(&b.thread).map(|t| t.provider.clone()).unwrap_or_default() } else { p.to_string() } };
        let evs = vec![json!({"$":"ThreadCreated","id":th,"project":"","title":name,"env":"local","branch":"","worktree":"","provider":provider,"model":gs(a, "model"),"at":now}),
            json!({"$":"BotSet","id":bot,"name":name,"thread":th,"look":super::rpc::seed(&name),"persona":gs(a, "persona"),"at":now}),
            json!({"$":"SettingSet","key":format!("bot.parent.{bot}"),"value":b.id,"at":now})];
        let evs = self.accounts_created(evs).await;
        if let Err(e) = self.t_commit(w, now, evs).await { return fail(&e) }
        let me = self.clone(); let (b2, t2) = (bot.clone(), th.clone()); tokio::spawn(async move { me.home_pub(&b2, &t2).await }); // BotFx home
        (true, json!({"bot":bot,"name":name,"note":format!("{name} is made: talk to it with bot_send")}))
    }

    async fn t_zone(&self, w: &World, now: u64, zone: &str) -> Res {
        let z = Zones { settings: &w.settings };
        if zone.is_empty() {
            let rule = person_rule(&z, now); let o = rule.off(now);
            return (true, json!({"zone":z.person(),"source":zone_source(&z),"offset":TzRule::offset_text(&o),"abbreviation":TzRule::abbr(&o),"localTime":rule.show(now),"iso":rule.iso(now)}))
        }
        if zone == "auto" {
            let now_zone = ["person.tz.device", "tz.machine"].iter().filter_map(|k| w.settings.get(*k)).find(|s| !s.is_empty()).cloned().unwrap_or_else(|| "UTC".into());
            return self.t_commit(w, now, vec![json!({"$":"SettingSet","key":"person.tz","value":"","at":now})]).await
                .map_or_else(|e| fail(&e), |_| said(&format!("the person's time zone follows their phone again (now {now_zone})")))
        }
        if !Zones::valid(zone) { return fail("that is not a time zone name (like Europe/Helsinki; auto follows their phone)") }
        if z.rule(zone).is_some() {
            let note = format!("the person's time zone was {}; it is {zone} now", z.person());
            return self.t_commit(w, now, vec![json!({"$":"SettingSet","key":"person.tz","value":zone,"at":now})]).await.map_or_else(|e| fail(&e), |_| said(&note))
        }
        // BotFx "zone": the rule is looked up and the zone set once it resolves (settings lane's zone.set), after this answer
        let hub = self.hub.clone(); let zn = zone.to_string();
        tokio::spawn(async move { if let Some(h) = hub.rpc_for("zone.set") { let _ = h.call(&hub, &crate::api::Caller { gen: "bots".into(), client: 0 }, &json!(0), "zone.set", &json!({"zone":zn})).await; } });
        said(&format!("looking up {zone}; the person's time zone changes from {} when it resolves (person_timezone shows it)", z.person()))
    }
}

/// Json.parse's verdict, as far as the recorded hub shows it: a value, or a bare word the lexer lets through; text that opens an object or an array and does not close is malformed
fn lenient_json(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() { return false }
    serde_json::from_str::<Value>(t).is_ok() || !(t.starts_with('{') || t.starts_with('[') || t.starts_with('"'))
}

/// The bots' tools as the agents' MCP server serves them: listed in a bot's thread only (Bots.route: a thread that is no bot's lists and answers none of them)
pub struct BotToolSet(pub Arc<Bots>);
impl crate::agents::mcp::ToolSet for BotToolSet {
    fn list(&self, thread: &str) -> Vec<Value> { if self.0.is_bot_thread(thread) { defs() } else { vec![] } }
    fn owns(&self, name: &str) -> bool { owns(name) }
    fn call<'a>(&'a self, thread: &'a str, name: &'a str, a: &'a Value) -> crate::api::Fut<'a, (bool, Value)> { Box::pin(async move { self.0.tool_call(thread, name, a).await }) }
}

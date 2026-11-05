//! `bots.*` requests (src/core/bothub.bend `Bots.request.k`): create, edit, delete, say, room, room_delete, post, remember,
//! forget, space, uber. Bend decides one request at a time; here one async lock (the world's) is held from the decision to the commit
//! of its events, so two requests never decide on the same state. The slow part (delivering to an agent) runs after, outside it.
use super::{reply, world::World};
use crate::{agents::{lines::hub_id, Agents}, api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

/// How a message reaches a bot's thread (B.Hub.deliver, with the origin Origin.step files) and how its agent is stopped (B.AgentStop).
pub trait Inbox: Send + Sync {
    fn deliver<'a>(&'a self, thread: &'a str, msg: &'a str, text: &'a str, origin: Option<(String, String, String)>) -> Fut<'a, Result<(), String>>;
    fn stop<'a>(&'a self, thread: &'a str) -> Fut<'a, ()>;
}
impl Inbox for Arc<Agents> {
    fn deliver<'a>(&'a self, thread: &'a str, msg: &'a str, text: &'a str, origin: Option<(String, String, String)>) -> Fut<'a, Result<(), String>> { Box::pin(async move { Agents::deliver(self, thread, msg, text, origin, vec![]).await }) }
    // B.AgentStop: ends the agent's process in any state, no events
    fn stop<'a>(&'a self, thread: &'a str) -> Fut<'a, ()> { Box::pin(async move { let _ = Agents::stop(self, thread).await; }) }
}

pub struct Bots { pub hub: Arc<Hub>, world: Mutex<World>, inbox: Arc<dyn Inbox> }

fn ok(id: &Value) -> Value { json!({"t":"reply","id":id,"ok":true}) }
fn err(id: &Value, e: &str) -> Value { json!({"t":"reply","id":id,"ok":false,"error":e}) }
pub fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }
fn gs<'a>(p: &'a Value, k: &str) -> &'a str { p.get(k).and_then(Value::as_str).unwrap_or("") }
/// J.Json.get_str(p, k, d)
fn gsd<'a>(p: &'a Value, k: &str, d: &'a str) -> &'a str { p.get(k).and_then(Value::as_str).unwrap_or(d) }
/// Nat.read(J.Json.as_raw(get(p, k), "")): a number, or a string of digits
fn gnat(p: &Value, k: &str) -> Option<u64> { match p.get(k)? { Value::Number(n) => n.as_u64(), Value::String(s) if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) => s.parse().ok(), _ => None } }

/// Bot.name.valid: 1 to 24 letters, digits, '-' or '_'
pub fn name_ok(n: &str) -> bool { let l = n.chars().count(); (1..=24).contains(&l) && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') }
/// Bots.seed: a stable look for a cat from its name
pub fn seed(name: &str) -> u64 { name.to_lowercase().chars().fold(7u64, |acc, c| (acc * 31 + c as u64) % 4294967291) }
/// Mem.kind.norm
pub fn mem_kind(k: &str) -> String { let l = k.to_lowercase(); if ["user", "pref", "fact", "howto", "note"].contains(&l.as_str()) { l } else { "fact".into() } }
/// Mem.key.norm: trimmed, anything but a letter, digit, '.' or '_' becomes '-', lower case, at most 64
pub fn mem_key(k: &str) -> String { k.trim().chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' { c.to_ascii_lowercase() } else { '-' }).take(64).collect() }
/// Hop.max: a bot's message goes out one hop past the turn that sent it, never past 6
pub const HOP_MAX: u64 = 6;
/// Hop.live: a message at this hop is heard by bots (a held one, past the bound, is kept in its room for the person)
pub fn hop_live(h: u64) -> bool { h <= HOP_MAX }
/// Bots.space.msg: a space's button or field, heard as a person's message
pub fn space_msg(action: &str, value: &str) -> String { format!("[space] {action}{}", if value.is_empty() { String::new() } else { format!(": {value}") }) }
/// Bots.note: what a bot reads when another bot (or a person) speaks to it
pub fn note(from: &str, room: &str, name: &str, text: &str) -> String { format!("[message from {from}{}]\n{text}", if room.starts_with("dm:") { String::new() } else { format!(" in #{name}") }) }
/// Mail.classify for the notes a person's post makes (`from` is empty): the origin filed under the woken bot's message
pub fn note_origin(text: &str) -> Option<(String, String, String)> {
    let head: String = text.chars().take(160).collect();
    if !head.starts_with("[message from ") { return None }
    let label = head.split(']').next().unwrap_or("");
    let after: String = label.chars().skip(14).collect();
    if label.contains(" in #") {
        let name = after.split(' ').next().unwrap_or("");
        let from = if name == "you" || name.starts_with("person@") { "person".to_string() } else if name.contains('@') { format!("peer:{}", name.rsplit('@').next().unwrap_or("")) } else { format!("bot:{name}") };
        Some((from, "room".into(), label.rsplit('#').next().unwrap_or("").to_string()))
    } else if after.starts_with("person@") { Some(("person".into(), "person".into(), String::new())) }
    else if after.contains('@') { Some((format!("peer:{after}"), "peer".into(), String::new())) }
    else { Some((format!("bot:{after}"), "bot".into(), String::new())) }
}
/// Bn.safe: a bot id fit to be a folder name
pub fn safe_id(id: &str) -> bool { !id.is_empty() && id.len() <= 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') }

impl Bots {
    /// Folds the log once, off the runtime's workers, then registers the `bots.*` requests.
    pub async fn start(hub: Arc<Hub>, agents: Arc<Agents>) -> Arc<Bots> { Bots::start_with(hub, Arc::new(agents)).await }
    pub async fn start_with(hub: Arc<Hub>, inbox: Arc<dyn Inbox>) -> Arc<Bots> {
        let store = hub.store.clone();
        let w = tokio::task::spawn_blocking(move || { let mut w = World::default(); if let Err(e) = w.sync(&store) { eprintln!("bots: reading the log: {e:#}") } w }).await.unwrap_or_default();
        let b = Arc::new(Bots { hub: hub.clone(), world: Mutex::new(w), inbox });
        hub.register_rpc(Arc::new(Rpc(b.clone())));
        b
    }
    /// The world caught up with the log; the guard is the lock a request holds from decision to commit.
    pub async fn world(&self) -> MutexGuard<'_, World> {
        let mut g = self.world.lock().await;
        let behind = self.hub.store.len().saturating_sub(g.lines_seen());
        if behind > 4096 { // a long catch-up runs off the workers
            let store = self.hub.store.clone(); let mut w = std::mem::take(&mut *g);
            *g = tokio::task::spawn_blocking(move || { if let Err(e) = w.sync(&store) { eprintln!("bots: reading the log: {e:#}") } w }).await.unwrap_or_default();
        } else if behind > 0 {
            if let Err(e) = g.sync(&self.hub.store) { eprintln!("bots: reading the log: {e:#}") }
        }
        g
    }
    /// B.Hub.id(prefix, now, seq): seq is the log's length; one request at a time makes it unique within a prefix
    fn id(&self, prefix: &str, now: u64) -> String { hub_id(prefix, now, self.hub.store.len() as u64) }

    async fn commit_reply(&self, id: &Value, events: Vec<Value>) -> Vec<Value> {
        match self.hub.commit_many(&events).await { Ok(_) => vec![ok(id)], Err(e) => vec![err(id, &format!("{e:#}"))] }
    }

    async fn create(self: &Arc<Self>, id: &Value, p: &Value) -> Vec<Value> {
        let name = gs(p, "name").trim().to_string(); let now = now_s();
        if !name_ok(&name) { return vec![err(id, "a bot's name is 1 to 24 letters, digits, - or _")] }
        let w = self.world().await;
        if w.named(&name).is_some() { return vec![err(id, &format!("a bot named {name} already exists"))] }
        let (provider, model) = (gsd(p, "provider", &w.setting("agent.provider", "claude")).to_string(), gsd(p, "model", &w.setting("agent.model", "")).to_string());
        let (bot, th) = (self.id("b", now), self.id("t", now));
        let look = gnat(p, "look").unwrap_or_else(|| seed(&name));
        let evs = vec![json!({"$":"ThreadCreated","id":th,"project":"","title":name,"env":"local","branch":"","worktree":"","provider":provider,"model":model,"at":now}),
            json!({"$":"BotSet","id":bot,"name":name,"thread":th,"look":look,"persona":gs(p, "persona"),"at":now})];
        let r = self.hub.commit_many(&evs).await;
        drop(w);
        if let Err(e) = r { return vec![err(id, &format!("{e:#}"))] }
        // the reply goes first; the bot's folder and the promotion follow (BotFx home, BotFx uber)
        let me = self.clone(); let (b2, t2) = (bot.clone(), th.clone());
        let uber = p.get("uber").and_then(Value::as_bool).unwrap_or(false);
        tokio::spawn(async move { me.home(&b2, &t2).await; if uber { let _ = me.promote_job(&b2).await; } });
        vec![json!({"t":"reply","id":id,"ok":true,"bot":bot})]
    }

    /// BotJob.home: the bot's folder `<home>/bots/<bot>` (git init'ed), then the thread works there.
    async fn home(&self, bot: &str, thread: &str) {
        if !safe_id(bot) { eprintln!("bots: not a folder name: {bot}"); return }
        let path = self.hub.cfg.home.join("bots").join(bot);
        let p2 = path.clone();
        let _ = tokio::task::spawn_blocking(move || { let _ = std::fs::create_dir_all(&p2); let _ = std::process::Command::new("git").args(["init", "-q"]).current_dir(&p2).output(); }).await;
        let _ = self.hub.commit(&json!({"$":"WorktreeSet","id":thread,"branch":"","worktree":path.to_string_lossy(),"at":now_s()})).await;
    }

    async fn edit(&self, id: &Value, p: &Value) -> Vec<Value> {
        let bot = gs(p, "bot"); let w = self.world().await;
        if !w.live(bot) { return vec![err(id, "no such bot")] }
        let b = w.bot(bot).cloned().unwrap();
        let ev = json!({"$":"BotSet","id":b.id,"name":b.name,"thread":"","look":gnat(p, "look").unwrap_or(b.look),"persona":gsd(p, "persona", &b.persona),"at":now_s()});
        self.commit_reply(id, vec![ev]).await
    }

    async fn delete(&self, id: &Value, p: &Value) -> Vec<Value> {
        let bot = gs(p, "bot"); let w = self.world().await;
        if !w.live(bot) { return vec![err(id, "no such bot")] }
        let thread = w.bot(bot).map(|b| b.thread.clone()).unwrap_or_default();
        let r = self.commit_reply(id, vec![json!({"$":"BotDeleted","id":bot,"at":now_s()})]).await;
        drop(w);
        if r[0]["ok"] == true { self.inbox.stop(&thread).await } // B.AgentStop only after the commit; BotFx gone (the browser page) is the term lane's: PARITY
        r
    }

    /// The text of a message filed in a bot's thread, read from the log
    fn quoted(&self, w: &World, thread: &str, msg: &str) -> Option<String> {
        let line = self.hub.store.get(w.msg_line(thread, msg)? - 1).ok()?;
        serde_json::from_str::<Value>(&line).ok()?.get("text")?.as_str().map(str::to_string)
    }

    /// Bots.say.msg / Bots.space: a person talks to a bot, the conversation starts over at hop 0. `msg` names a retry (stored once).
    async fn say(&self, id: &Value, bot: &str, text: &str, msg: &str, to: &str) -> Vec<Value> {
        let mut w = self.world().await;
        if !w.live(bot) { return vec![err(id, "no such bot")] }
        let th = w.bot(bot).map(|b| b.thread.clone()).unwrap_or_default();
        if !msg.is_empty() && (w.has_msg(&th, msg) || w.inflight.contains(&(th.clone(), msg.to_string()))) { return vec![ok(id)] }
        let q = if to.is_empty() { None } else { self.quoted(&w, &th, to) };
        let text = reply::resolve(to, q.as_deref(), text);
        let now = now_s(); let mid = if msg.is_empty() { self.id("m", now) } else { msg.to_string() };
        w.inflight.insert((th.clone(), mid.clone()));
        let woke = self.hub.commit(&json!({"$":"BotWoke","bot":bot,"hop":0,"at":now})).await;
        drop(w);
        let r = match woke { Ok(_) => self.inbox.deliver(&th, &mid, &text, None).await, Err(e) => Err(format!("{e:#}")) };
        self.world.lock().await.inflight.remove(&(th, mid));
        match r { Ok(()) => vec![ok(id)], Err(e) => vec![err(id, &e)] }
    }

    async fn room(&self, id: &Value, p: &Value) -> Vec<Value> {
        let _w = self.world().await; let now = now_s(); let rid = if gs(p, "room").is_empty() { self.id("r", now) } else { gs(p, "room").to_string() };
        self.commit_reply(id, vec![json!({"$":"RoomSet","id":rid,"name":gsd(p, "name", "room"),"members":gs(p, "members"),"at":now})]).await
    }

    /// Bots.post.id: a person's post in a room (hop 0): logged, then every other member here hears it (never a held one, never when the
    /// mail funnel files the post or the member is the uberbot: Bots.wake.room). Members elsewhere ("name@peer") are the net lane's.
    async fn post(&self, id: &Value, p: &Value) -> Vec<Value> {
        let (msg, text) = (gs(p, "msg"), gs(p, "text"));
        let w = self.world().await;
        let Some(room) = w.room(gs(p, "room")).cloned() else { return vec![err(id, "no such room")] };
        if !msg.is_empty() && w.has_post(msg) { return vec![ok(id)] }
        let now = now_s(); let mid = if msg.is_empty() { self.id("p", now) } else { msg.to_string() };
        let seq = self.hub.store.len() as u64;
        let mail = w.setting("mail.on", "off") == "on";
        if let Err(e) = self.hub.commit(&json!({"$":"RoomPosted","id":mid,"room":room.id,"from":"you","text":text,"hop":0,"at":now})).await { return vec![err(id, &format!("{e:#}"))] }
        let n = note("you", &room.id, &room.name, text);
        let mut wakes: Vec<(String, String, String)> = vec![];
        if !mail && hop_live(0) {
            for (i, m) in room.members.iter().enumerate() {
                if m.contains('@') { continue }
                let Some(b) = w.bot(m).filter(|b| !b.deleted) else { continue };
                if w.triage_owner(&b.id) { continue }
                wakes.push((b.id.clone(), b.thread.clone(), format!("m{now}-{seq}.{i}")));
            }
        }
        drop(w);
        // as Bots.fan: each member in turn is logged as woken, then hears the note
        for (bot, th, wake) in wakes {
            if self.hub.commit(&json!({"$":"BotWoke","bot":bot,"hop":0,"at":now})).await.is_ok() { let _ = self.inbox.deliver(&th, &wake, &n, note_origin(&n)).await; }
        }
        vec![ok(id)]
    }

    /// Bots.uber: the person makes a bot the uberbot ("" takes it back). A promotion first has the bot backed up; the log learns of it only when that worked.
    async fn uber(self: &Arc<Self>, id: &Value, bot: &str) -> Vec<Value> {
        let w = self.world().await;
        if bot.is_empty() { // demote: nothing to change when there is no uberbot
            if w.setting("uber.bot", "").is_empty() { return vec![ok(id)] }
            return self.commit_reply(id, vec![json!({"$":"UberSet","bot":"","uid":"","at":now_s()})]).await;
        }
        let live = w.live(bot) && !bot.contains('~');
        if !live { return vec![err(id, "only a bot on this machine can be the uberbot")] }
        if bot == w.setting("uber.bot", "") && !w.setting("uber.uid", "").is_empty() { return vec![ok(id)] }
        drop(w);
        match self.promote_job(bot).await { Ok(uid) => vec![json!({"t":"reply","id":id,"ok":true,"uid":uid})], Err(e) => vec![err(id, &e)] }
    }
    /// BotJob.uber: back the bot up (`<home>/backups/uber-<now>-<bot>`: its folder and the log lines naming it or its thread), then log UberSet
    async fn promote_job(&self, bot: &str) -> Result<String, String> {
        if !safe_id(bot) { return Err("not a bot id".into()) }
        let thread = { let w = self.world().await; w.bot(bot).map(|b| b.thread.clone()).unwrap_or_default() };
        if !thread.is_empty() && !safe_id(&thread) { return Err("not a bot id".into()) }
        let now = now_s(); let home = self.hub.cfg.home.to_string_lossy().to_string();
        let (b, t) = (bot.to_string(), if thread.is_empty() { bot.to_string() } else { thread });
        let ran = tokio::task::spawn_blocking(move || std::process::Command::new("sh").args(["-c", BACKUP_SH, "sh", &home, &now.to_string(), &b, &t]).current_dir("/").status().map(|s| s.success()).unwrap_or(false)).await.unwrap_or(false);
        if !ran { return Err("could not back the bot up; nothing changed".into()) }
        let mut seed = [0u8; 4]; if let Ok(mut f) = std::fs::File::open("/dev/urandom") { let _ = std::io::Read::read_exact(&mut f, &mut seed); }
        let uid = format!("u{now}-{}", hex::encode(seed));
        let _g = self.world().await;
        self.hub.commit(&json!({"$":"UberSet","bot":bot,"uid":uid,"at":now})).await.map_err(|e| format!("{e:#}"))?;
        Ok(uid)
    }
}
const BACKUP_SH: &str = "set -e; d=\"$1/backups/uber-$2-$3\"; mkdir -p \"$d\"; if [ -d \"$1/bots/$3\" ]; then cp -a \"$1/bots/$3\" \"$d/home\"; fi; grep -F -e \"$3\" -e \"$4\" \"$1/events.jsonl\" > \"$d/events.jsonl\" || [ $? -eq 1 ]";

struct Rpc(Arc<Bots>);
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["bots.create", "bots.edit", "bots.delete", "bots.say", "bots.room", "bots.room_delete", "bots.post", "bots.remember", "bots.forget", "bots.space", "bots.uber"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let b = &self.0;
            match method {
                "bots.create" => b.create(id, p).await,
                "bots.edit" => b.edit(id, p).await,
                "bots.delete" => b.delete(id, p).await,
                "bots.say" => b.say(id, gs(p, "bot"), gs(p, "text"), gs(p, "msg"), gs(p, "reply")).await,
                "bots.room" => b.room(id, p).await,
                "bots.room_delete" => { let _w = b.world().await; b.commit_reply(id, vec![json!({"$":"RoomDeleted","id":gs(p, "room"),"at":now_s()})]).await }
                "bots.post" => b.post(id, p).await,
                "bots.forget" => { let _w = b.world().await; b.commit_reply(id, vec![json!({"$":"MemoryForgot","bot":gs(p, "bot"),"key":gs(p, "key"),"at":now_s()})]).await }
                "bots.remember" => { let _w = b.world().await; b.commit_reply(id, vec![json!({"$":"MemorySaved","bot":gs(p, "bot"),"key":mem_key(gs(p, "key")),"kind":mem_kind(gs(p, "kind")),"text":gs(p, "text"),"tags":gs(p, "tags"),"at":now_s()})]).await }
                "bots.space" => { let text = space_msg(gs(p, "action"), gs(p, "value")); b.say(id, gs(p, "bot"), &text, "", "").await }
                "bots.uber" => b.uber(id, gs(p, "bot")).await,
                _ => vec![err(id, "unknown bots request")],
            }
        })
    }
}

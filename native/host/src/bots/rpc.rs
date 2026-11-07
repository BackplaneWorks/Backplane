//! `bots.*` requests: create, edit, delete, say, room, room_delete, post, remember,
//! forget, space, uber. decides one request at a time; here one async lock (the world's) is held from the decision to the commit
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
    /// The messages waiting behind the thread's turn (M.Queue.items)
    fn queued<'a>(&'a self, _thread: &'a str) -> Fut<'a, Vec<String>> { Box::pin(async { vec![] }) }
    /// An approval the person answers: the answer, once given (at once "session" when they allowed the tool for this session). The future is held until then.
    fn approve<'a>(&'a self, _thread: &'a str, _tool: &'a str, _detail: &'a str) -> Fut<'a, Result<String, String>> { Box::pin(async { Err("nobody can answer here".to_string()) }) }
    /// the same message with M.System's role (a mailroom escalation into a bot's thread)
    fn deliver_system<'a>(&'a self, thread: &'a str, msg: &'a str, text: &'a str, origin: Option<(String, String, String)>) -> Fut<'a, Result<(), String>> { self.deliver(thread, msg, text, origin) }
    /// Wake.ride: a note on a thread's ride
    fn ride<'a>(&'a self, thread: &'a str, id: &'a str, text: &'a str) -> Fut<'a, Result<(), String>>;
    /// Ride.note alone: the note joins the ride (a running Claude turn hears it at once) and starts no turn
    fn note<'a>(&'a self, thread: &'a str, id: &'a str, text: &'a str) -> Fut<'a, Result<(), String>>;
}
impl Inbox for Arc<Agents> {
    fn deliver<'a>(&'a self, thread: &'a str, msg: &'a str, text: &'a str, origin: Option<(String, String, String)>) -> Fut<'a, Result<(), String>> { Box::pin(async move { Agents::deliver(self, thread, msg, text, origin, vec![]).await }) }
    // B.AgentStop: ends the agent's process in any state, no events
    fn stop<'a>(&'a self, thread: &'a str) -> Fut<'a, ()> { Box::pin(async move { let _ = Agents::stop(self, thread).await; }) }
    fn queued<'a>(&'a self, thread: &'a str) -> Fut<'a, Vec<String>> { Box::pin(async move { self.world().await.queued.get(thread).cloned().unwrap_or_default() }) }
    fn approve<'a>(&'a self, thread: &'a str, tool: &'a str, detail: &'a str) -> Fut<'a, Result<String, String>> {
        Box::pin(async move {
            if self.world().await.sessions.contains(&(thread.to_string(), tool.to_string())) { return Ok("session".to_string()) } // Perm.session_ok
            let now = now_s(); let ask = hub_id("q", now, self.seq_n()); let (tx, rx) = tokio::sync::oneshot::channel();
            { let mut m = self.parked.lock().unwrap(); if m.values().filter(|x| x.thread == thread).count() >= 16 { return Err("too many questions are waiting for the person".to_string()) }
              m.insert(ask.clone(), crate::agents::mcp::Parked { thread: thread.into(), tool: tool.into(), input: Value::Null, id: Value::Null, tx }); }
            let pend = self.pending_ev(thread, 1, "", now).await;
            self.commit_events(vec![json!({"$":"AskOpened","id":ask,"thread":thread,"kind":"approval","tool":tool,"detail":detail,"options":"","at":now}), pend]).await;
            rx.await.map_err(|_| "the question went away unanswered".to_string())
        })
    }
    fn deliver_system<'a>(&'a self, thread: &'a str, msg: &'a str, text: &'a str, origin: Option<(String, String, String)>) -> Fut<'a, Result<(), String>> { Box::pin(async move { Agents::deliver_as(self, "system", thread, msg, text, origin, vec![]).await }) }
    fn ride<'a>(&'a self, thread: &'a str, id: &'a str, text: &'a str) -> Fut<'a, Result<(), String>> { Box::pin(async move { Agents::wake_ride(self, thread, id, text).await }) }
    fn note<'a>(&'a self, thread: &'a str, id: &'a str, text: &'a str) -> Fut<'a, Result<(), String>> { Box::pin(async move { Agents::ride_note(self, thread, id, text).await }) }
}

pub struct Bots { pub hub: Arc<Hub>, world: Mutex<World>, pub(super) inbox: Arc<dyn Inbox>, pub(crate) router: std::sync::OnceLock<Arc<dyn super::mail::file::Router>>, last_id: std::sync::Mutex<(u64, u64)>, pub(crate) owner_link: std::sync::OnceLock<Arc<dyn super::mail::owner::OwnerLink>>, pub(crate) owner_busy: std::sync::atomic::AtomicBool, star: std::sync::OnceLock<Arc<StarPrompt>>, threads: std::sync::RwLock<std::collections::HashSet<String>>, pub(crate) mail_lists: std::sync::RwLock<std::collections::HashMap<String, Vec<&'static str>>> }
/// the state lane's star-nudge count for a human prompt (statehost::policies()?.star_prompt): the events to commit with the prompt and, on the fifth, the offer for its connection
pub type StarPrompt = dyn Fn(&str, u64) -> (Vec<Value>, Option<Value>) + Send + Sync;

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
    if !text.starts_with("[message from ") { return None }
    let o = super::mail::classify("", "m", false, text); Some((o.from, o.kind, o.r#ref))
}
/// Bn.safe: a bot id fit to be a folder name
pub fn safe_id(id: &str) -> bool { !id.is_empty() && id.len() <= 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') }

impl Bots {
    /// Folds the log once, off the runtime's workers, then registers the `bots.*` requests.
    pub async fn start(hub: Arc<Hub>, agents: Arc<Agents>) -> Arc<Bots> {
        let b = Bots::start_with(hub, Arc::new(agents.clone())).await; b.start_ticker();
        agents.add_tools(Arc::new(super::tools::BotToolSet(b.clone()))); // the bots' own MCP tools join Backplane's in a bot's thread
        agents.add_tools(Arc::new(super::mail::tools::MailTools(b.clone()))); // the mailroom's, listed by what the thread is
        agents.set_mailroom(Arc::new(super::mail::funnel::Room(Arc::downgrade(&b))));
        let cut = agents.cut_at_start(); b.run_recover(&cut).await; b.owner_legacy().await;
        b
    }
    pub async fn start_with(hub: Arc<Hub>, inbox: Arc<dyn Inbox>) -> Arc<Bots> {
        let store = hub.store.clone();
        let w = tokio::task::spawn_blocking(move || { let mut w = crate::ckpt::take("bots", &store, || World::load(&store)).unwrap_or_default(); if let Err(e) = w.sync(&store) { eprintln!("bots: reading the log: {e:#}") } w }).await.unwrap_or_default();
        let mut w = w; w.steps.clear(); // what the start folded was answered when it happened
        let b = Arc::new(Bots { hub: hub.clone(), world: Mutex::new(w), inbox, router: Default::default(), last_id: Default::default(), owner_link: Default::default(), owner_busy: Default::default(), star: Default::default(), threads: Default::default(), mail_lists: Default::default() });
        { let g = b.world.lock().await; b.refresh_threads(&g); }
        { let me = b.clone(); crate::ckpt::register(move |store| me.world.blocking_lock().save(store)); }
        { let me = b.clone(); tokio::spawn(async move { loop { tokio::time::sleep(std::time::Duration::from_secs(20)).await; me.owner_outbox().await } }); } // OwnerJob: the outbox is walked again
        let _ = b.router.set(Arc::new(super::mail::run::MailRouter(Arc::downgrade(&b))));
        b.start_after();
        hub.register_rpc(Arc::new(Rpc(b.clone())));
        // the browser driver's wake (a task run asked for, a teach recording finished): the bot hears it as a message from the person; `bot` is a bot id or its thread's
        if let Some(d) = crate::term::driver::get(&hub) {
            let me = Arc::downgrade(&b);
            d.set_wake(Arc::new(move |bot, text| { if let Some(b) = me.upgrade() { let bot = bot.to_string(); tokio::spawn(async move {
                let id = { let w = b.world().await; if w.live(&bot) { Some(bot.clone()) } else { w.bots.values().find(|x| !x.deleted && x.thread == bot).map(|x| x.id.clone()) } };
                if let Some(id) = id { let _ = b.say(&json!(0), &id, &text, "", "", None).await; }
            }); } }));
        }
        super::api::register(&hub, &b);
        hub.register_http(Arc::new(super::hooks::HookHttp { bots: b.clone(), seen: Default::default() }));
        super::mail::requests::register(&hub, &b);
        let me = b.clone(); tokio::spawn(async move { let n = me.mail_recover().await; if n > 0 { eprintln!("bots: {n} mail header(s) recovered from the journal") } });
        b
    }
    /// the threads that are live bots' (a tool list asks without waiting for the world lock). Refreshed only when the world catches up (`world()`): a path that starts a bot's agent
    /// without touching the world first would list no bot tools for that whole process (tools/list is read once at start), and a deleted bot's thread stays listed until the
    /// next catch-up (its calls are refused). Today a bot's first turn goes through `say`, which does; a new path (bot_create, a far-mirrored bot, uber promotion) must refresh.
    fn refresh_threads(&self, w: &World) {
        let mut t: std::collections::HashSet<String> = w.bots.values().filter(|b| !b.deleted && !b.thread.is_empty()).map(|b| b.thread.clone()).collect();
        t.extend(w.mail.runs.values().filter(|r| r.live && w.run_bot(&r.thread).is_some()).map(|r| r.thread.clone())); // a run and a back office work as their bot (Bots.route: Run.bot)
        *self.mail_lists.write().unwrap() = super::mail::tools::lists(w);
        *self.threads.write().unwrap() = t;
    }
    pub fn is_bot_thread(&self, th: &str) -> bool { self.threads.read().unwrap().contains(th) }
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
        if behind > 0 { self.refresh_threads(&g) }
        g
    }
    /// B.Hub.id(prefix, now, seq): seq is the log's length; one request at a time makes it unique within a prefix
    pub(super) fn hub_id(&self, prefix: &str, now: u64) -> String { self.id(prefix, now) }
    pub(super) async fn commit_reply_pub(&self, id: &Value, events: Vec<Value>) -> Vec<Value> { self.commit_reply(id, events).await }
    pub(super) async fn inbox_deliver_raw(&self, th: &str, msg: &str, text: &str, o: Option<(String, String, String)>) -> Result<(), String> { self.inbox.deliver(th, msg, text, o).await }
    pub(super) async fn inbox_deliver_role(&self, system: bool, th: &str, msg: &str, text: &str, o: Option<(String, String, String)>) -> Result<(), String> { if system { self.inbox.deliver_system(th, msg, text, o).await } else { self.inbox.deliver(th, msg, text, o).await } }
    pub(super) async fn inbox_note(&self, th: &str, id: &str, text: &str) -> Result<(), String> { self.inbox.note(th, id, text).await }
    pub(super) async fn inbox_ride(&self, th: &str, id: &str, text: &str) -> Result<(), String> { self.inbox.ride(th, id, text).await }
    pub(super) async fn inbox_deliver(&self, k: &super::routines::Wake) -> Result<(), String> { self.inbox.deliver(&k.thread, &k.msg, &k.text, k.origin.clone()).await }
    /// B.Hub.id(prefix, now, seq) with seq the log's length. A Step is atomic, so two ids of one second never share a length; here a request decides
    /// before it commits, so a second id asked for in the same second while the log has not grown gets the next number (an id stays what it was otherwise).
    pub(crate) fn id(&self, prefix: &str, now: u64) -> String {
        let len = self.hub.store.len() as u64; let mut l = self.last_id.lock().unwrap();
        let seq = if l.0 == now && len <= l.1 { l.1 + 1 } else { len };
        *l = (now, seq); hub_id(prefix, now, seq)
    }

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
        let evs = self.accounts_created(evs).await;
        let r = self.hub.commit_many(&evs).await;
        drop(w);
        if let Err(e) = r { return vec![err(id, &format!("{e:#}"))] }
        // the reply goes first; the bot's folder and the promotion follow (BotFx home, BotFx uber)
        let me = self.clone(); let (b2, t2) = (bot.clone(), th.clone());
        let uber = p.get("uber").and_then(Value::as_bool).unwrap_or(false);
        // the folder first, then the reply: a message sent at once to a new bot starts a turn that needs its folder (without it: "its folder  is gone")
        me.home(&b2, &t2).await;
        if uber { tokio::spawn(async move { let _ = me.promote_job(&b2).await; }); }
        vec![json!({"t":"reply","id":id,"ok":true,"bot":bot})]
    }

    /// BotJob.home: the bot's folder `~/.backplane/bots/<bot>` (relocate::root; git init'ed), then the thread works there.
    pub(super) async fn home_pub(&self, bot: &str, thread: &str) { self.home(bot, thread).await }
    async fn home(&self, bot: &str, thread: &str) {
        if !safe_id(bot) { eprintln!("bots: not a folder name: {bot}"); return }
        let path = super::relocate::root(&self.hub.cfg.home).join(bot);
        let p2 = path.clone();
        let _ = tokio::task::spawn_blocking(move || { let _ = std::fs::create_dir_all(&p2); let _ = crate::proc::std_cmd(crate::proc::Kind::Local, "git").args(["init", "-q"]).current_dir(&p2).output(); }).await;
        let _ = self.hub.commit(&json!({"$":"WorktreeSet","id":thread,"branch":"","worktree":path.to_string_lossy(),"at":now_s()})).await;
    }

    async fn edit(&self, id: &Value, p: &Value) -> Vec<Value> {
        let bot = gs(p, "bot"); let w = self.world().await;
        if !w.live(bot) { return vec![err(id, "no such bot")] }
        let b = w.bot(bot).cloned().unwrap();
        let now = now_s();
        let ev = json!({"$":"BotSet","id":b.id,"name":b.name,"thread":"","look":gnat(p, "look").unwrap_or(b.look),"persona":gsd(p, "persona", &b.persona),"at":now});
        self.commit_reply(id, w.also(now, vec![ev])).await
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
    /// Accounts.created: a thread of a provider with accounts starts on the default account (frozen at creation); the binding follows its ThreadCreated
    pub(super) async fn accounts_created(&self, evs: Vec<Value>) -> Vec<Value> {
        let mut out = Vec::with_capacity(evs.len() + 1);
        for e in evs {
            let bind = if e["$"] == "ThreadCreated" && matches!(e["provider"].as_str(), Some("claude") | Some("codex")) {
                let p = e["provider"].as_str().unwrap_or("");
                let v = self.hub.setting(&format!("account.default.{p}")).await.unwrap_or_else(|| "{\"id\":\"\",\"epoch\":0}".into());
                Some(json!({"$":"SettingSet","key":format!("account.thread.{}.{p}", e["id"].as_str().unwrap_or("")),"value":v,"at":e["at"]}))
            } else { None };
            out.push(e); out.extend(bind);
        }
        out
    }
    pub fn set_star(&self, f: Arc<StarPrompt>) { let _ = self.star.set(f); }
    /// Star.accepted: a prompt a person typed (a client request, not a retry, not blank) counts once; (events to commit with it, the offer frame to push after)
    pub(super) fn star_for(&self, client: Option<u64>, msg: &str, text: &str, now: u64) -> (Vec<Value>, Option<(u64, Value)>) {
        let (Some(c), Some(f)) = (client, self.star.get()) else { return (vec![], None) };
        if text.trim().is_empty() { return (vec![], None) }
        let (evs, offer) = f(msg, now); (evs, offer.map(|o| (c, o)))
    }
    async fn say(&self, id: &Value, bot: &str, text: &str, msg: &str, to: &str, star: Option<u64>) -> Vec<Value> {
        let mut w = self.world().await;
        if !w.live(bot) { return vec![err(id, "no such bot")] }
        let th = w.bot(bot).map(|b| b.thread.clone()).unwrap_or_default();
        if !msg.is_empty() && (w.has_msg(&th, msg) || w.inflight.contains(&(th.clone(), msg.to_string()))) { return vec![ok(id)] }
        let q = if to.is_empty() { None } else { self.quoted(&w, &th, to) };
        let text = reply::resolve(to, q.as_deref(), text);
        let now = now_s(); let mid = if msg.is_empty() { self.id("m", now) } else { msg.to_string() };
        w.inflight.insert((th.clone(), mid.clone()));
        let (extra, offer) = self.star_for(star, &mid, &text, now);
        let mut evs = vec![json!({"$":"BotWoke","bot":bot,"hop":0,"at":now})]; evs.extend(extra);
        let woke = self.hub.commit_many(&evs).await;
        if woke.is_ok() { if let Some((c, f)) = offer { self.hub.push(c, f); } }
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
    /// A post from the person that no client made (a mention in a thread): no star count, no reply.
    pub(crate) async fn post_as_person(&self, room: &str, text: &str) -> Vec<Value> { self.post(&Value::Null, &json!({"room": room, "text": text}), None).await }
    async fn post(&self, id: &Value, p: &Value, star: Option<u64>) -> Vec<Value> {
        let (msg, text) = (gs(p, "msg"), gs(p, "text"));
        let w = self.world().await;
        let Some(room) = w.room(gs(p, "room")).cloned() else { return vec![err(id, "no such room")] };
        if !msg.is_empty() && w.has_post(msg) { return vec![ok(id)] }
        let now = now_s(); let mid = if msg.is_empty() { self.id("p", now) } else { msg.to_string() };
        let rr = super::post::RoomRef { id: room.id.clone(), name: room.name.clone(), members: room.members.clone() };
        let (extra, offer) = self.star_for(star, &mid, text, now);
        let pending = match self.post_decide(&w, now, "", "you", "", &rr, text, 0, &mid, extra).await { Ok(p) => p, Err(e) => return vec![err(id, &e)] };
        if let Some((c, f)) = offer { self.hub.push(c, f); }
        drop(w);
        self.post_finish(pending, 0).await;
        vec![ok(id)]
    }

    /// Bots.uber: the person makes a bot the uberbot ("" takes it back). A promotion first has the bot backed up; the log learns of it only when that worked.
    async fn uber(self: &Arc<Self>, id: &Value, bot: &str) -> Vec<Value> {
        let w = self.world().await;
        if bot.is_empty() { // demote: nothing to change when there is no uberbot
            if w.setting("uber.bot", "").is_empty() { return vec![ok(id)] }
            return self.commit_reply(id, vec![json!({"$":"UberSet","bot":"","uid":"","at":now_s()})]).await;
        }
        if bot.contains('~') { return vec![err(id, "that machine did not take it: that is done on its own machine")] } // a bot elsewhere: the far layer answers (net lane)
        if !w.live(bot) { return vec![err(id, "only a bot on this machine can be the uberbot")] }
        if bot == w.setting("uber.bot", "") && !w.setting("uber.uid", "").is_empty() { return vec![ok(id)] }
        drop(w);
        match self.promote_job(bot).await { Ok(uid) => vec![json!({"t":"reply","id":id,"ok":true,"uid":uid})], Err(e) => vec![err(id, &e)] }
    }
    /// BotJob.uber: back the bot up (`<home>/backups/uber-<now>-<bot>`: its folder and the log lines naming it or its thread), then log UberSet
    async fn promote_job(&self, bot: &str) -> Result<String, String> {
        if !safe_id(bot) { return Err("not a bot id".into()) }
        let thread = { let w = self.world().await; w.bot(bot).map(|b| b.thread.clone()).unwrap_or_default() };
        if !thread.is_empty() && !safe_id(&thread) { return Err("not a bot id".into()) }
        let now = now_s(); let home = self.hub.cfg.home.to_string_lossy().to_string(); let broot = super::relocate::root(&self.hub.cfg.home).to_string_lossy().into_owned();
        let (b, t) = (bot.to_string(), if thread.is_empty() { bot.to_string() } else { thread });
        let ran = tokio::task::spawn_blocking(move || crate::proc::std_cmd(crate::proc::Kind::Local, "sh").args(["-c", BACKUP_SH, "sh", &home, &now.to_string(), &b, &t, &broot]).current_dir("/").status().map(|s| s.success()).unwrap_or(false)).await.unwrap_or(false);
        if !ran { return Err("could not back the bot up; nothing changed".into()) }
        let mut seed = [0u8; 4]; let _ = crate::plat::fs::random(&mut seed);
        let uid = format!("u{now}-{}", hex::encode(seed));
        let _g = self.world().await;
        self.hub.commit(&json!({"$":"UberSet","bot":bot,"uid":uid,"at":now})).await.map_err(|e| format!("{e:#}"))?;
        Ok(uid)
    }
}
const BACKUP_SH: &str = "set -e; d=\"$1/backups/uber-$2-$3\"; mkdir -p \"$d\"; if [ -d \"$5/$3\" ]; then cp -a \"$5/$3\" \"$d/home\"; fi; grep -F -e \"$3\" -e \"$4\" \"$1/events.jsonl\" > \"$d/events.jsonl\" || [ $? -eq 1 ]";

/// a person's prompt through a client connection (never a retry): the connection the star offer would go to
fn human(who: &Caller, p: &Value) -> Option<u64> { if who.client > 0 && matches!(p.get("retry"), None | Some(Value::Null) | Some(Value::Bool(false))) { Some(who.client) } else { None } }

struct Rpc(Arc<Bots>);
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["bots.create", "bots.edit", "bots.delete", "bots.say", "bots.room", "bots.room_delete", "bots.post", "bots.remember", "bots.forget", "bots.space", "bots.uber", "bots.routine", "bots.routine_delete", "bots.routine_run", "bots.hook", "bots.hook_token", "bots.hook_revoke", "bots.pin", "bots.uber_home", "bots.uber_join", "rules.set", "rules.drop", "board.set", "board.drop"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let b = &self.0;
            match method {
                "bots.create" => b.create(id, p).await,
                "bots.edit" => b.edit(id, p).await,
                "bots.delete" => b.delete(id, p).await,
                "bots.say" => b.say(id, gs(p, "bot"), gs(p, "text"), gs(p, "msg"), gs(p, "reply"), human(who, p)).await,
                "bots.room" => b.room(id, p).await,
                "bots.room_delete" => { let _w = b.world().await; b.commit_reply(id, vec![json!({"$":"RoomDeleted","id":gs(p, "room"),"at":now_s()})]).await }
                "bots.post" => b.post(id, p, human(who, p)).await,
                "bots.forget" => { let w = b.world().await; let now = now_s(); b.commit_reply(id, w.also(now, vec![json!({"$":"MemoryForgot","bot":gs(p, "bot"),"key":gs(p, "key"),"at":now})])).await }
                "bots.remember" => { let w = b.world().await; let now = now_s(); b.commit_reply(id, w.also(now, vec![json!({"$":"MemorySaved","bot":gs(p, "bot"),"key":mem_key(gs(p, "key")),"kind":mem_kind(gs(p, "kind")),"text":gs(p, "text"),"tags":gs(p, "tags"),"at":now})])).await }
                "bots.space" => { let text = space_msg(gs(p, "action"), gs(p, "value")); b.say(id, gs(p, "bot"), &text, "", "", None).await }
                "bots.uber" => b.uber(id, gs(p, "bot")).await,
                "bots.routine" => b.routine(id, p).await,
                "bots.routine_delete" => { let w = b.world().await; let now = now_s(); b.commit_reply(id, w.also(now, vec![json!({"$":"RoutineDeleted","id":gs(p, "routine"),"at":now})])).await }
                // Bots.hook_revoke: the log notes it (a revoked hook keeps its name and counts); the secret is the server's and has no Rust store yet (bots.hook makes none)
                "bots.hook" => b.hook_new(id, p).await,
                "bots.hook_token" => b.hook_token(id, p).await,
                "bots.hook_revoke" => { let w = b.world().await; let hid = gs(p, "hook"); if !w.hooks.contains_key(hid) { return vec![err(id, "no such webhook")] } let now = now_s(); let r = b.commit_reply(id, vec![json!({"$":"HookSet","id":hid,"bot":"","name":"","revoked":true,"at":now})]).await; super::hooks::forget(&b.hub.cfg.home, hid); r }
                "bots.routine_run" => b.routine_run(id, gs(p, "routine")).await,
                "bots.pin" => b.pin(id, p).await,
                "bots.uber_home" => b.uber_home(id).await,
                "bots.uber_join" => b.uber_join(id, gs(p, "uid")).await,
                "rules.set" | "rules.drop" | "board.set" | "board.drop" => {
                    let w = b.world().await; let (ok, r) = b.person_call(&w, method, p, now_s()).await; drop(w);
                    vec![if ok { json!({"t":"reply","id":id,"ok":true,"result":r}) } else { err(id, gs(&r, "error")) }]
                }
                _ => vec![err(id, "unknown bots request")],
            }
        })
    }
}

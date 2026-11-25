//! Swarms (docs/SWARM.md): a permanent room whose members are threads, peers with no lead. Threads are room members beside bots
//! (`thread:<id>`); every room post goes through the guards (`gate`); a member thread hears the board in batches (`give`); a claim takes
//! hub locks; a synthesis ends the swarm and is reported to whoever started it. The client requests `swarm.*` are here too.
use super::gate::{self, Budget, PostMeta, Verdict, LIMITS};
use crate::present::swarm_need as need;
use super::post::{Heard, Pending, RoomRef};
use super::rpc::{now_s, Bots};
use super::world::{Room, World, THREAD};
use crate::api::{Caller, Fut, RpcHandler};
use crate::hub::Hub;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};

type Res = (bool, Value);
fn fail(m: &str) -> Res { (false, json!({"error":m})) }
fn gs<'a>(a: &'a Value, k: &str) -> &'a str { a.get(k).and_then(Value::as_str).unwrap_or("") }

/// A thread as a swarm sees it (agents' world): its agent, turn, whether it waits on the person, and where it works.
#[derive(Clone, Debug, Default)]
pub struct ThreadInfo { pub provider: String, pub model: String, pub title: String, pub turn: String, pub asking: bool, pub gone: bool, pub project: String, pub root: String,
    /// the plain words of the error its newest turn ended on (agents world `T.why`), "" while it runs or ended well
    pub why: String,
    /// the work it owns between turns (background work, a watched work_run, a wake it set): a member that owns some is not idle
    pub busy: u64 }
impl ThreadInfo {
    /// the chip's word: working, waiting (an ask open), failed, monitoring (its turn is over, work it owns still runs: a landing, a build), idle
    pub fn word(&self) -> &'static str { if self.gone { "removed" } else if self.asking { "waiting" } else { match self.turn.as_str() { "running" => "working", "failed" => "failed", _ if self.busy > 0 => "monitoring", _ => "idle" } } }
}

/// The batching clock of the member threads: when each was last given the board, and when it is due next (a timer runs for it).
#[derive(Default)]
pub struct Give { me: std::sync::OnceLock<Weak<Bots>>, pub(super) members: std::sync::RwLock<HashSet<String>>, last: Mutex<HashMap<(String, String), u64>>, due: Mutex<HashMap<(String, String), u64>>,
    /// the posts a resume released (held while the swarm was paused), each member's to hear with its next batch
    released: Mutex<HashMap<(String, String), Vec<Value>>> }

pub const NAMES: [&str; 2] = ["swarm_create", "swarm_control"];
pub const MAX_MEMBERS: usize = 8;

/// The lock a claim on `file` takes: a KiCad board or schematic's own design lock (agents/design.rs), else `file-<fnv64 of the path from the root>`.
pub fn lock_for(root: &str, file: &str) -> String {
    use crate::files::kicad::canonical as kc;
    let rel = file.trim().trim_start_matches("./");
    let abs = if rel.starts_with('/') || root.is_empty() { rel.to_string() } else { format!("{}/{rel}", root.trim_end_matches('/')) };
    let doc = kc::doc_of(&kc::route(&abs));
    if !doc.is_empty() && kc::is_document(&doc) { return kc::doc_lock(&doc) }
    let key = abs.strip_prefix(root).unwrap_or(&abs).trim_start_matches('/').to_string();
    format!("file-{:016x}", crate::ckpt::fnv(key.as_bytes()))
}

/// The brief every member starts with (its first message, with the seed after it).
pub fn brief(topic: &str, room: &str, goal: &str, me: &str, role: &str, peers: &[String], joining: bool) -> String {
    let n = peers.len() + 1;
    format!("You are {me}, one of {n} peers in the swarm \"{topic}\" (room {room}){}{}. {}There is no boss: nobody assigns you work and you report to nobody; you are all equals. \
The board is the room: post ideas, critiques, questions, claims and results with room_post {{room: \"{room}\", text, tags}} (tags: idea, critique, question, claim, done, synthesis; reply: the id of the post you answer), and read it with room_read {{room: \"{room}\", since}}. \
Build on each other's ideas and say whose you build on. Before you change anything, post a claim naming the files (files: [paths from the project root]) and work only on what you claimed; post done (reply: your claim) when it is finished, which frees it. \
@name gets one peer's attention. The person may post too: their word steers everyone. Posts come to you by themselves, a few at a time: end your turn when you are waiting on others instead of polling. \
Keep posts short and new: repeating yourself or talking without claiming or finishing anything pauses the swarm. When the goal is met, any of you may post a synthesis (tag synthesis): the outcome and the decisions, which ends the swarm.",
        if peers.is_empty() { String::new() } else { format!(", with {}", peers.join(", ")) },
        if role.is_empty() { String::new() } else { format!("; your strength: {role}") },
        if goal.is_empty() { String::new() } else { format!("The goal: {goal}. ") })
        + if joining { "\n\nYou join a swarm already under way: room_read the board first." } else { "" }
}

/// What a member is given: the posts it has not heard, oldest first.
pub fn batch_note(room: &Room, posts: &[Value], names: &HashMap<String, String>) -> String {
    let mut s = format!("[#{}] {} new post{} on the board (room {}):", room.name, posts.len(), if posts.len() == 1 { "" } else { "s" }, room.id);
    let mut used = 0usize;
    for p in posts {
        let who = if p["guard"] == true { "hub".to_string() } else { gs(p, "from").to_string() };
        let tags: Vec<&str> = p["tags"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let mut text: String = gs(p, "text").chars().take(1500).collect(); if text.len() < gs(p, "text").len() { text.push_str(" …") }
        let line = format!("\n- {who}{} ({}{}): {text}", if tags.is_empty() { String::new() } else { format!(" [{}]", tags.join(", ")) }, gs(p, "id"), if gs(p, "reply").is_empty() { String::new() } else { format!(", re {}", gs(p, "reply")) });
        used += line.len(); if used > 12000 { s.push_str("\n- … (room_read for the rest)"); break }
        s.push_str(&line);
    }
    let _ = names;
    if room.paused() && !room.ended() { s.push_str(&format!("\nThe swarm is paused ({}): nobody is woken by posts until the person resumes it.", room.reason)) }
    if room.ended() { s.push_str("\nThe swarm is over: nothing more is needed from you here.") }
    else { s.push_str(&format!("\nAnswer with room_post {{room: \"{}\"}} when you have something new; end your turn when you are waiting on others.", room.id)) }
    s
}

fn member_meta(w: &World, room: &str, member: &str) -> Value { serde_json::from_str(&w.setting(&format!("room.member.{room}.{member}"), "")).unwrap_or(Value::Null) }
/// (member, name) of every member: a thread's short name, a bot's name
pub fn names(w: &World, room: &Room) -> Vec<(String, String)> {
    room.members.iter().map(|m| { let n = if m.starts_with(THREAD) { member_meta(w, &room.id, m)["name"].as_str().unwrap_or("peer").to_string() } else { w.bot(m).map(|b| b.name.clone()).unwrap_or_else(|| m.clone()) }; (m.clone(), n) }).collect()
}
fn tags_arg(a: &Value) -> Vec<String> {
    match a.get("tags") { Some(Value::Array(v)) => v.iter().filter_map(Value::as_str).map(str::to_string).collect(), Some(Value::String(s)) => s.split([',', ' ']).filter(|x| !x.is_empty()).map(str::to_string).collect(), _ => vec![] }
}
fn files_arg(a: &Value) -> Vec<String> {
    let v: Vec<String> = match a.get("files") { Some(Value::Array(v)) => v.iter().filter_map(Value::as_str).map(str::to_string).collect(), Some(Value::String(s)) => s.split([',', '\n']).map(str::to_string).collect(), _ => vec![] };
    let mut out: Vec<String> = vec![]; for f in v { let f = f.trim().to_string(); if !f.is_empty() && !f.contains("..") && !out.contains(&f) && out.len() < 32 { out.push(f) } } out
}

/// Who posts: the room member it is (`thread:<id>` or a bot's id), the key the guards count it by, its shown name, its thread, its hop, and whether it is a bot.
pub struct Who { pub member: String, pub meta: String, pub name: String, pub thread: String, pub hop: u64, pub bot: bool }

/// A claim as kept in `room.claim.<room>.<post>`.
fn claim_key(room: &str, post: &str) -> String { format!("room.claim.{room}.{post}") }
fn claims(w: &World, room: &str) -> Vec<(String, Value)> {
    let p = format!("room.claim.{room}."); let mut v: Vec<(String, Value)> = w.settings.iter().filter(|(k, x)| k.starts_with(&p) && !x.is_empty()).filter_map(|(k, x)| { let mut c = serde_json::from_str::<Value>(x).ok()?; c["_key"] = json!(k); Some((k[p.len()..].to_string(), c)) }).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0)); v
}

/// The hub's need facts of a swarm (present/swarm_need.rs `key`): who a pause is for, the budget part that ran out, the members that failed and why.
fn need_rec(w: &World, room: &str) -> Value { serde_json::from_str::<Value>(&w.setting(&need::key(room), "")).ok().filter(Value::is_object).unwrap_or_else(|| json!({})) }
fn need_ev(room: &str, rec: &Value, now: u64) -> Value {
    let empty = rec.as_object().map_or(true, |o| o.iter().all(|(_, v)| v.is_null() || v == &json!("") || v == &json!([])));
    json!({"$":"SettingSet","key":need::key(room),"value":if empty { String::new() } else { rec.to_string() },"at":now})
}
/// the pause's facts in `rec`: for whom and which budget part (none: the pause is over)
fn with_pause(rec: &Value, to: Option<(&str, &str)>) -> Value {
    let mut r = rec.clone(); let o = r.as_object_mut().unwrap();
    match to { Some((f, part)) => { o.insert("for".into(), json!(f)); if part.is_empty() { o.remove("part"); } else { o.insert("part".into(), json!(part)); } } None => { o.remove("for"); o.remove("part"); } }
    r
}
/// Whether a bot or the uberbot started the swarm (its routine pauses are the starter's, by policy: swarm_need::policy).
fn by_bot(w: &World, r: &Room) -> bool { !r.by.is_empty() && (w.run_bot(&r.by).is_some() || w.chat_is(&r.by) || w.triage_thread() == r.by) }
fn auto_key(room: &str) -> String { format!("room.auto.{room}") }
fn part_of(b: &Budget, part: &str) -> u64 { match part { "posts" => b.posts, "turns" => b.turns, "tokens" => b.tokens, _ => 0 } }
fn set_part(b: &mut Budget, part: &str, v: u64) { match part { "posts" => b.posts = v, "turns" => b.turns = v, "tokens" => b.tokens = v, _ => {} } }
/// The member posts held while the swarm was paused, since the swarm last resumed (oldest first): a resume delivers them (`released`).
/// A post after which the room ran again: the hub's resume, or the person's own post (it resumes a paused room: rpc.rs `post_k`).
fn resumes(e: &Value) -> bool { if e["guard"] == true { e["tags"].as_array().map_or(false, |t| t.iter().any(|x| x == "resume")) } else { gs(e, "from") == "you" && gs(e, "thread").is_empty() } }
fn held_since_resume(posts: &[Value]) -> Vec<Value> {
    let start = posts.iter().rposition(resumes).map_or(0, |i| i + 1);
    posts[start..].iter().filter(|e| e["guard"] != true && !gs(e, "held").is_empty() && gs(e, "held") != "backstop").cloned().collect()
}
/// The sign-in command a provider's CLI takes in a terminal (Sign in on a member that failed on its sign-in), when it has one.
fn signin_cmd(provider: &str) -> Option<&'static str> { match provider { "antigravity" | "agy" => Some("agy"), "grok" => Some("grok login"), "codex" => Some("codex login"), "claude" => Some("claude /login"), _ => None } }

/// What `guard_pause` leaves to its caller, after the world lock.
enum After { Nothing, Tell(String), Wake }

impl Bots {
    fn arc(&self) -> Option<Arc<Bots>> { self.swarm.me.get().and_then(Weak::upgrade) }
    pub(super) fn swarm_start(self: &Arc<Self>) { let _ = self.swarm.me.set(Arc::downgrade(self)); self.hub.register_rpc(Arc::new(SwarmRpc(self.clone()))); }
    /// the rooms a thread is a member of (cached for tool lists)
    pub fn member_of(&self, thread: &str) -> bool { self.swarm.members.read().unwrap().contains(thread) }

    /// The one way a member's post goes in (a thread's room_post, a bot's room_post and bot_send): the guards' verdict, the post with its tags,
    /// mentions and files, a pause when a guard fires, a claim's locks, a done's release, a synthesis's end. `before`: a new room's RoomSet.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn member_post(&self, w: &World, now: u64, who: &Who, rr: &RoomRef, a: &Value, before: Vec<Value>) -> Result<(Value, Pending), String> {
        let text = gs(a, "text").trim().to_string(); if text.is_empty() { return Err("a post needs text".into()) }
        let rec = w.rooms.get(&rr.id).filter(|r| !r.deleted).cloned();
        if let Some(r) = &rec { if r.ended() { return Err(format!("the swarm #{} is over ({}): its members are not woken any more", r.name, r.state)) } }
        let tags = gate::tags_of(&tags_arg(a));
        let posts: Vec<PostMeta> = w.room_meta.get(&rr.id).map(|q| q.iter().cloned().collect()).unwrap_or_default();
        let meta = PostMeta { id: String::new(), member: who.meta.clone(), tags: tags.clone(), text: text.chars().take(gate::TEXT).collect(), at: now, held: false };
        let (budget, used) = (rec.as_ref().map(|r| r.budget).unwrap_or_default(), w.room_used.get(&rr.id).copied().unwrap_or_default());
        let mut v = gate::verdict(&posts, &meta, rec.as_ref().map_or(false, Room::paused), who.hop, &budget, &used, &LIMITS);
        // a bot's swarm whose budget ran out is raised by policy (up to its cap) and the post goes on; past the cap it is the person's
        if v == Verdict::Pause("budget") { if let Some(r) = &rec { let next = Budget { posts: used.posts + 1, ..used }; if self.raise_by_policy(w, r, &next, now).await { v = Verdict::Live } } }
        // work moving again: the idle pauses the policy resumed count from here
        if tags.iter().any(|t| t == "claim" || t == "done") && !w.setting(&auto_key(&rr.id), "").is_empty() { let _ = self.hub.commit(&json!({"$":"SettingSet","key":auto_key(&rr.id),"value":"","at":now})).await; }
        let held = match &v { Verdict::Refuse(m) => return Err(m.clone()), Verdict::Live => None, Verdict::Hold(w) | Verdict::Pause(w) => Some(*w) };
        let mut f = Map::new();
        if !tags.is_empty() { f.insert("tags".into(), json!(tags)); }
        if !gs(a, "reply").is_empty() { f.insert("reply".into(), json!(gs(a, "reply"))); }
        let ms: Vec<String> = match &rec { Some(r) => gate::mentions(&text, &names(w, r)).into_iter().filter(|m| m != &who.member).collect(), None => vec![] };
        if !ms.is_empty() { f.insert("mentions".into(), json!(ms)); }
        let files = if tags.iter().any(|t| t == "claim") { files_arg(a) } else { vec![] };
        if !files.is_empty() { f.insert("files".into(), json!(files)); }
        if !who.bot { f.insert("thread".into(), json!(who.thread)); }
        if let Some(h) = held { f.insert("held".into(), json!(h)); }
        let mid = self.id("p", now);
        let pending = self.post_decide(w, now, &who.thread, &who.name, if who.bot { &who.member } else { "" }, rr, &text, who.hop, &mid, before, Heard { live: held.is_none(), fields: f }).await?;
        let mut ans = json!({"ok":true,"room":rr.id,"post":mid});
        if !ms.is_empty() { ans["mentioned"] = json!(ms) }
        if let (Verdict::Pause(why), Some(r)) = (&v, &rec) { // the post that tripped a guard is held, and the room pauses (an exchange closes, as it did at the cap)
            let note = gate::pause_text(why, &LIMITS);
            let part = if *why == "budget" { budget.hit(&Budget { posts: used.posts + 1, ..used }).unwrap_or("") } else { "" };
            let to = if r.swarm() && by_bot(w, r) && need::policy(true, why, 0, 0, 0) == need::Policy::Starter { "starter" } else { "person" };
            let mut evs = vec![r.set_ev(now, |x| { x.state = "paused".into(); x.reason = (*why).into(); if x.ephemeral { x.closed = true } }), self.guard_post(&r.id, &note, &["pause"], true, now)];
            if r.swarm() { evs.push(need_ev(&r.id, &with_pause(&need_rec(w, &r.id), Some((to, part))), now)) }
            self.hub.commit_many(&w.also(now, evs)).await.map_err(|e| format!("{e:#}"))?;
            if r.swarm() && to == "starter" { self.tell_starter(w, r, &note, now).await }
            ans["held"] = json!(true); ans["paused"] = json!(why); ans["note"] = json!(format!("{note} Your post is on the board for the person; don't send it again or anywhere else."));
            if r.ephemeral { ans["closed"] = json!(true) }
        } else if let Some(h) = held { ans["held"] = json!(true); ans["note"] = json!(match h { "paused" => "held: the room is paused, so nobody was woken. Your post is on the board for the person; don't send it again or anywhere else.", _ => "held: this conversation is too deep, so nobody was woken. Your post is on the board for the person; don't send it again or anywhere else." }) }
        // a claim's locks (a thread's), a done's release, a synthesis's end
        if !who.bot && held.is_none() {
            if !files.is_empty() {
                let got = self.inbox.claim(&who.thread, files.clone(), format!("claim {mid} in #{}", rr.name)).await;
                let waiting: Vec<String> = got.iter().filter(|g| !g.2).map(|g| format!("{} (held by {})", g.1, if g.3.is_empty() { "another thread".into() } else { g.3.clone() })).collect();
                let rec_v = json!({"thread":who.thread,"files":files,"locks":got.iter().map(|g| g.0.clone()).collect::<Vec<_>>(),"state":if waiting.is_empty() { "held" } else { "waiting" }});
                let _ = self.hub.commit(&json!({"$":"SettingSet","key":claim_key(&rr.id, &mid),"value":rec_v.to_string(),"at":now})).await;
                ans["claim"] = rec_v;
                if !waiting.is_empty() { ans["note"] = json!(format!("claimed, but {} {} claimed by another thread: don't edit {} until the lock message says it is yours.", waiting.join(", "), if waiting.len() == 1 { "is" } else { "are" }, if waiting.len() == 1 { "it" } else { "them" })) }
            }
            if tags.iter().any(|t| t == "done") {
                let mine: Vec<(String, Value)> = claims(w, &rr.id).into_iter().filter(|(_, c)| c["thread"] == who.thread.as_str()).collect();
                let which: Vec<(String, Value)> = { let r = gs(a, "reply"); let m: Vec<_> = mine.iter().filter(|(p, _)| p == r).cloned().collect(); if m.is_empty() { mine } else { m } };
                if !which.is_empty() { ans["released"] = json!(which.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>()) }
                self.release_later(which, now);
            }
            if tags.iter().any(|t| t == "synthesis") { if let Some(r) = &rec { if r.swarm() { self.end(w, r, "done", &text, &who.name, now).await; ans["note"] = json!("synthesis posted: the swarm is done, and its starter hears the synthesis") } } }
        }
        Ok((ans, pending))
    }
    pub(super) async fn swarm_end_person(&self, w: &World, r: &Room, text: &str, now: u64) { self.end(w, r, "done", text, "you", now).await }
    fn guard_post(&self, room: &str, text: &str, tags: &[&str], held: bool, now: u64) -> Value {
        let mut e = json!({"$":"RoomPosted","id":self.id("p", now),"room":room,"from":"hub","text":text,"hop":0,"guard":true,"at":now});
        if !tags.is_empty() { e["tags"] = json!(tags) } if held { e["held"] = json!("guard") } e
    }
    /// let claims go: their locks, then their records (`_key`)
    async fn release(&self, which: &[(String, Value)], now: u64) {
        for (_, c) in which {
            let locks: Vec<String> = c["locks"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
            self.inbox.unclaim(gs(c, "thread"), locks).await;
        }
        let evs: Vec<Value> = which.iter().filter_map(|(_, c)| c.get("_key").and_then(Value::as_str)).map(|k| json!({"$":"SettingSet","key":k,"value":"","at":now})).collect();
        if !evs.is_empty() { let _ = self.hub.commit_many(&evs).await; }
    }
    /// `release` once the caller's world lock is let go: a released lock is granted to the thread waiting for it, and that delivery takes the
    /// world lock (a release under it hung the hub: a synthesis while a peer's claim of the same file waited)
    fn release_later(&self, which: Vec<(String, Value)>, now: u64) {
        if which.is_empty() { return }
        let Some(me) = self.arc() else { return };
        tokio::spawn(async move { me.release(&which, now).await });
    }
    /// End a swarm (done with a synthesis, or stopped): its state, every claim let go, the members told (no turn started), the starter told.
    async fn end(&self, w: &World, r: &Room, state: &str, synthesis: &str, by_name: &str, now: u64) {
        let all = claims(w, &r.id);
        let _ = self.hub.commit_many(&w.also(now, vec![r.set_ev(now, |x| { x.state = state.into(); x.reason = String::new() }), self.guard_post(&r.id, if state == "done" { "The swarm is done." } else { "The swarm was stopped." }, &[], true, now)])).await;
        self.release_later(all, now);
        let note = if state == "done" { format!("[#{}] The swarm is done: {by_name} posted the synthesis. Nothing more is needed from you here.\n\n{synthesis}", r.name) } else { format!("[#{}] The swarm was stopped. Nothing more is needed from you here; let go of anything you claimed.", r.name) };
        let me = self.arc(); let (members, by, name, id) = (r.threads(), r.by.clone(), r.name.clone(), r.id.clone()); let syn = synthesis.to_string(); let state = state.to_string(); let by_name = by_name.to_string();
        // after the world lock is let go: a delivery may go through the mailroom, which takes it
        tokio::spawn(async move { let Some(b) = me else { return };
            for th in members { let _ = b.inbox_note(&th, &format!("room:{id}:end"), &note).await; }
            if !by.is_empty() && !b.member_of(&by) {
                let text = if state == "done" { format!("[Backplane] Swarm \"{name}\" (room {id}) is done. Its synthesis, by {by_name}:\n\n{syn}") } else { format!("[Backplane] Swarm \"{name}\" (room {id}) was stopped, with no synthesis.") };
                let msg = format!("t:room:{id}:end"); let _ = b.inbox_deliver_raw(&by, &msg, &text, Some((format!("room:{id}"), "launch".into(), id.clone()))).await;
            }
        });
    }

    // ---- delivery: a member thread hears the board at most once a batch window, at once for the person or a mention ----
    pub(super) async fn give_later(&self, room: &str, threads: &[String], urgent: &[String], now: u64) {
        let Some(me) = self.arc() else { return };
        for th in threads {
            let key = (room.to_string(), th.clone());
            let last = self.swarm.last.lock().unwrap().get(&key).copied().unwrap_or(0);
            let at = gate::due(last, now, urgent.contains(th), &LIMITS);
            { let mut d = self.swarm.due.lock().unwrap(); if d.get(&key).map_or(false, |x| *x <= at) { continue } d.insert(key.clone(), at); }
            let b = me.clone(); let (room, th) = (room.to_string(), th.clone());
            if at <= now { b.give_now(&room, &th).await } else { tokio::spawn(async move { tokio::time::sleep(std::time::Duration::from_secs(at - now)).await; b.give_now(&room, &th).await }); }
        }
    }
    /// Give `th` every post of the room it has not heard (not its own, not a held one), as one note.
    pub(super) async fn give_now(&self, room: &str, th: &str) {
        let key = (room.to_string(), th.to_string());
        self.swarm.due.lock().unwrap().remove(&key);
        let member = format!("{THREAD}{th}");
        let (r, posts, hk) = { let w = self.world().await;
            let Some(r) = w.room(room).cloned() else { return }; if !r.members.contains(&member) || r.ended() { return }
            let hk = format!("room.heard.{room}.{member}"); let heard = w.setting(&hk, "");
            let lines = w.room_lines.get(room).cloned().unwrap_or_default(); let mut out: Vec<Value> = vec![];
            for l in lines.iter().rev().take(200) { let Ok(raw) = self.hub.store.get(l - 1) else { continue }; let Ok(e) = serde_json::from_str::<Value>(&raw) else { continue };
                if !heard.is_empty() && gs(&e, "id") == heard { break } out.push(e) }
            out.reverse();
            // a member that joined later hears what came after it joined (the board before is its room_read)
            if heard.is_empty() { let joined = w.setting(&format!("room.joined.{room}.{member}"), ""); if let Some(i) = out.iter().position(|e| gs(e, "id") == joined) { out.drain(..=i); } }
            let mut posts: Vec<Value> = out.into_iter().filter(|e| gs(e, "thread") != th && gs(e, "held").is_empty()).collect();
            // what a resume released: the posts held while it was paused, before the new ones
            let rel: Vec<Value> = self.swarm.released.lock().unwrap().remove(&key).unwrap_or_default().into_iter().filter(|e| gs(e, "thread") != th && !posts.iter().any(|p| p["id"] == e["id"])).collect();
            if !rel.is_empty() { posts.splice(0..0, rel); }
            (r, posts, hk) };
        if posts.is_empty() { return }
        let back = |me: &Self| { let rel: Vec<Value> = posts.iter().filter(|p| !gs(p, "held").is_empty()).cloned().collect(); if !rel.is_empty() { me.swarm.released.lock().unwrap().entry(key.clone()).or_default().splice(0..0, rel); } };
        // the newest post it heard now (a released one is older than its mark: the mark stays where it is)
        let newest = posts.iter().rev().find(|p| gs(p, "held").is_empty()).map(|p| gs(p, "id").to_string());
        let last = posts.last().map(|p| gs(p, "id").to_string()).unwrap_or_default();
        // only the person's word steers a running turn; a mention got here early, but a peer never restarts a running non-Claude turn (it would never finish)
        let person = posts.iter().any(|p| gs(p, "from") == "you" && p["guard"] != true);
        let note = batch_note(&r, &posts, &HashMap::new());
        let now = now_s();
        if let Some(n) = &newest { if self.hub.commit(&json!({"$":"SettingSet","key":hk,"value":n,"at":now})).await.is_err() { back(self); return } }
        self.swarm.last.lock().unwrap().insert(key.clone(), now);
        let Some(info) = self.inbox.info(th).await else { back(self); return }; if info.gone { return }
        let id = format!("room:{room}:{last}");
        // a running non-Claude turn takes it with its next Backplane tool result or at its end (never restarted by a peer's post); the person's post steers
        let _ = if !person && info.turn == "running" && info.provider != "claude" { self.inbox_note(th, &id, &note).await } else { self.inbox_ride(th, &id, &note).await };
    }
    /// The minute tick: a member that has posts it never heard (a restart cut its timer) is given them; a member whose turn failed has its cause read (a sign-in is
    /// the person's: its row says so, and the member goes on by itself once the sign-in works again); an active swarm whose members are all idle for
    /// `LIMITS.idle`, or whose turns or tokens are used up, pauses, as the policy has it (`guard_pause`).
    pub async fn swarm_tick(&self, now: u64) {
        let rooms: Vec<Room> = { let w = self.world().await; w.rooms.values().filter(|r| !r.deleted && r.swarm() && !r.ended()).cloned().collect() };
        for r in rooms {
            let mut all_idle = true; let mut infos = vec![];
            for th in r.threads() {
                let info = self.inbox.info(&th).await;
                if info.as_ref().map_or(false, |i| matches!(i.word(), "working" | "monitoring" | "waiting")) { all_idle = false }
                infos.push((th.clone(), info));
                if r.paused() { continue }
                let due = self.swarm.due.lock().unwrap().contains_key(&(r.id.clone(), th.clone())); if !due { self.give_later(&r.id, &[th.clone()], &[], now).await }
            }
            self.members_failed(&r, &infos, now).await;
            if r.paused() {
                let w = self.world().await; let Some(cur) = w.room(&r.id).cloned() else { continue };
                if cur.paused() && !cur.ended() && all_idle && now.saturating_sub(cur.last) >= crate::sidebar::SWARM_STALE {
                    let all_claims = claims(&w, &cur.id);
                    let evs = vec![
                        cur.set_ev(now, |x| { x.state = "stopped".into(); x.reason = "idle".into() }),
                        need_ev(&cur.id, &with_pause(&need_rec(&w, &cur.id), None), now),
                        self.guard_post(&cur.id, "Idle for 24 hours: swarm ended automatically.", &[], true, now),
                    ];
                    if self.hub.commit_many(&w.also(now, evs)).await.is_ok() {
                        drop(w);
                        self.release_later(all_claims, now);
                    }
                }
                continue
            }
            let w = self.world().await; let Some(cur) = w.room(&r.id).cloned() else { continue }; if cur.paused() { continue }
            let used = w.room_used.get(&r.id).copied().unwrap_or_default();
            let why = if cur.budget.hit(&used).is_some() { Some("budget") } else if all_idle && now.saturating_sub(cur.last) >= LIMITS.idle && self.swarm.due.lock().unwrap().keys().all(|k| k.0 != r.id) { Some("idle") } else { None };
            if let Some(why) = why { let after = self.guard_pause(&w, &cur, why, &used, now).await; drop(w);
                match after { After::Tell(n) => self.tell_starter_now(&cur, &n, now).await, After::Wake => { let ths = cur.threads(); self.give_later(&cur.id, &ths, &ths, now).await } After::Nothing => {} } }
        }
    }
    /// A guard's pause of a swarm (idle, budget: the tick's), as `swarm_need::policy` has it: a bot's swarm is resumed or raised at once (up to the cap), or
    /// left paused for its starter; the person's (and money past the cap) is theirs, with its line in Needs you. The starter hears of every pause left standing.
    /// What the caller does once it let the world lock go (a delivery takes it: `give_now`, the mailroom): tell the starter the pause stands, or wake the members.
    async fn guard_pause(&self, w: &World, r: &Room, why: &str, used: &Budget, now: u64) -> After {
        let bot = by_bot(w, r);
        let part = if why == "budget" { r.budget.hit(used).unwrap_or("") } else { "" };
        let auto = w.setting(&auto_key(&r.id), "").parse::<u64>().unwrap_or(0);
        let pol = need::policy(bot, why, auto, part_of(&r.budget, part), part_of(&gate::SWARM_BUDGET, part));
        match pol {
            need::Policy::Raise(_) => { self.raise_by_policy(w, r, used, now).await; After::Nothing }
            need::Policy::Resume => {
                let text = format!("Every member was idle for {} minutes: the hub woke them ({} of {}). Carry on from the board, or post a synthesis if the goal is met.", LIMITS.idle / 60, auto + 1, need::AUTO_RESUMES);
                let evs = vec![self.guard_post(&r.id, &text, &["resume"], false, now), json!({"$":"SettingSet","key":auto_key(&r.id),"value":(auto + 1).to_string(),"at":now})];
                self.release_held(w, r);
                if self.hub.commit_many(&w.also(now, evs)).await.is_err() { return After::Nothing }
                After::Wake
            }
            need::Policy::Starter | need::Policy::Person => {
                let note = gate::pause_text(why, &LIMITS);
                let to = if pol == need::Policy::Starter { "starter" } else { "person" };
                let evs = vec![r.set_ev(now, |x| { x.state = "paused".into(); x.reason = why.into() }), self.guard_post(&r.id, &note, &["pause"], true, now), need_ev(&r.id, &with_pause(&need_rec(w, &r.id), Some((to, part))), now)];
                if self.hub.commit_many(&w.also(now, evs)).await.is_ok() { After::Tell(note) } else { After::Nothing }
            }
        }
    }
    /// A bot's swarm whose budget part `used` reached: raised by policy (half again, up to `RAISE_CAP` times the default), said on the board and to the starter.
    /// False: not a bot's swarm, or the cap is reached (money: the person's).
    async fn raise_by_policy(&self, w: &World, r: &Room, used: &Budget, now: u64) -> bool {
        if !r.swarm() || !by_bot(w, r) { return false }
        let Some(part) = r.budget.hit(used) else { return false };
        let need::Policy::Raise(to) = need::policy(true, "budget", 0, part_of(&r.budget, part), part_of(&gate::SWARM_BUDGET, part)) else { return false };
        let from = part_of(&r.budget, part);
        let text = format!("The budget's {part} ran out at {from}: raised by policy to {to} (at most {} without the person).", part_of(&gate::SWARM_BUDGET, part) * need::RAISE_CAP);
        let evs = vec![r.set_ev(now, |x| set_part(&mut x.budget, part, to)), self.guard_post(&r.id, &text, &[], false, now)];
        if self.hub.commit_many(&w.also(now, evs)).await.is_err() { return false }
        let _ = self.inbox_note_starter(w, r, &format!("[Backplane] Swarm \"{}\" (room {}): {text}", r.name, r.id), now).await;
        true
    }
    /// The starter (a thread that is no member) hears a pause its guards left standing, with what resolves it.
    async fn tell_starter_now(&self, r: &Room, note: &str, now: u64) {
        if r.by.is_empty() || self.member_of(&r.by) { return }
        let _ = self.inbox_deliver_raw(&r.by, &format!("t:room:{}:pause:{now}", r.id), &format!("[Backplane] Swarm \"{}\" (room {}) paused itself: {note} swarm_control resumes it, raises its budget or stops it.", r.name, r.id), Some((format!("room:{}", r.id), "launch".into(), r.id.clone()))).await;
    }
    /// `tell_starter_now` from under the world lock (a delivery may go through the mailroom, which takes it).
    async fn tell_starter(&self, _w: &World, r: &Room, note: &str, now: u64) {
        if r.by.is_empty() || self.member_of(&r.by) { return }
        let note = note.to_string();
        let Some(me) = self.arc() else { return };
        // after the caller's world lock is let go: a delivery may go through the mailroom, which takes it
        let r = r.clone();
        tokio::spawn(async move { me.tell_starter_now(&r, &note, now).await });
    }
    async fn inbox_note_starter(&self, _w: &World, r: &Room, text: &str, now: u64) -> Result<(), String> {
        if r.by.is_empty() || self.member_of(&r.by) { return Ok(()) }
        let Some(me) = self.arc() else { return Ok(()) }; let (by, id, text) = (r.by.clone(), r.id.clone(), text.to_string());
        tokio::spawn(async move { let _ = me.inbox_note(&by, &format!("room:{id}:policy:{now}"), &text).await; }); Ok(())
    }
    /// What any resume of a paused swarm does besides its RoomSet (`swarm_control` resume and raise, the person's post: rpc.rs `post_k`): the posts held while it was
    /// paused are released to the members, the pause's facts go, and the person's own resume starts the policy's count again. Before the commit.
    pub(super) fn resume_facts(&self, w: &World, r: &Room, person: bool, now: u64) -> Vec<Value> {
        if !r.swarm() { return vec![] }
        self.release_held(w, r);
        let mut ev = vec![need_ev(&r.id, &with_pause(&need_rec(w, &r.id), None), now)];
        if person || !by_bot(w, r) { ev.push(json!({"$":"SettingSet","key":auto_key(&r.id),"value":"","at":now})) }
        ev
    }
    /// The posts held while the swarm was paused go to every member with its next batch (none is held for good: #4 of the Nov 20 swarm review).
    fn release_held(&self, w: &World, r: &Room) {
        let lines = w.room_lines.get(&r.id).cloned().unwrap_or_default();
        let posts: Vec<Value> = lines.iter().rev().take(200).filter_map(|l| self.hub.store.get(l - 1).ok()).filter_map(|raw| serde_json::from_str::<Value>(&raw).ok()).collect::<Vec<_>>().into_iter().rev().collect();
        let held = held_since_resume(&posts); // (called before the resume is committed: what was held since the last one)
        if held.is_empty() { return }
        let mut m = self.swarm.released.lock().unwrap();
        // (each post once: a resume whose commit failed, then the next one, queues the same posts again)
        for th in r.threads() { let q = m.entry((r.id.clone(), th.clone())).or_default(); for e in held.iter().filter(|e| gs(e, "thread") != th) { if !q.iter().any(|x| x["id"] == e["id"]) { q.push(e.clone()) } } }
        m.retain(|_, q| !q.is_empty());
    }
    /// The members whose turn failed: the cause read once (a sign-in or account problem is the person's: `swarm_need::signin_cause`) and kept in the need facts,
    /// the member told on its own thread; one that works again leaves them, and one whose sign-in works again goes on by itself.
    async fn members_failed(&self, r: &Room, infos: &[(String, Option<ThreadInfo>)], now: u64) {
        let mut alerts: HashMap<String, bool> = HashMap::new();
        for (_, i) in infos.iter().filter_map(|(t, i)| i.as_ref().filter(|i| i.turn == "failed").map(|i| (t, i))) { if !alerts.contains_key(&i.provider) {
            let on = !self.hub.setting(&crate::settings::accounts_hub::alert_key(&i.provider)).await.unwrap_or_default().is_empty(); alerts.insert(i.provider.clone(), on); } }
        let w = self.world().await; let rec = need_rec(&w, &r.id);
        let old: Vec<Value> = rec["failed"].as_array().cloned().unwrap_or_default();
        let mut failed: Vec<Value> = vec![]; let mut evs = vec![]; let mut back: Vec<(String, String)> = vec![];
        for (th, info) in infos {
            let Some(i) = info else { continue };
            let prev = old.iter().find(|f| f["thread"] == th.as_str()).cloned();
            if i.turn != "failed" { continue } // working again (or idle after a turn that went well): it leaves the facts
            let cause = if need::signin_cause(&i.why) { "sign-in" } else { "other" };
            match prev {
                Some(p) if p["cause"] == cause => {
                    // a sign-in fixed since: the member goes on by itself
                    if cause == "sign-in" && self.signin_back(alerts.get(&i.provider).copied().unwrap_or(false), &i.provider, &p) { back.push((th.clone(), i.provider.clone())); continue }
                    failed.push(p)
                }
                _ => {
                    let alert = alerts.get(&i.provider).copied().unwrap_or(false);
                    failed.push(json!({"thread":th,"provider":i.provider,"cause":cause,"say":need::short(&i.why),"at":now,"alert":alert}));
                    if cause == "sign-in" { let p = need::provider_say(&i.provider);
                        evs.push(json!({"$":"ActivityLogged","thread":th,"act":format!("swarm-signin:{}:{now}", r.id),"tone":"warn","kind":"swarm","summary":format!("{p} sign-in expired: this swarm member cannot start a turn. Sign in to {p} (the swarm's row in Needs you has the button); it goes on by itself once the sign-in works."),"at":now})); }
                }
            }
        }
        // written only when the list changed (an unchanged one, empty or not, writes nothing: no event a minute per swarm)
        if failed != old { let mut next = rec.clone(); next["failed"] = json!(failed); evs.push(need_ev(&r.id, &next, now)) }
        for (th, prov) in &back { let nm = member_meta(&w, &r.id, &format!("{THREAD}{th}"))["name"].as_str().unwrap_or("peer").to_string();
            evs.push(self.guard_post(&r.id, &format!("{nm} is back: its {} sign-in works again.", need::provider_say(prov)), &[], false, now)); }
        if evs.is_empty() { return }
        if self.hub.commit_many(&w.also(now, evs)).await.is_err() { return }
        drop(w);
        for (th, prov) in back { let _ = self.inbox_ride(&th, &format!("room:{}:signin:{now}", r.id), &format!("[#{}] The {} sign-in works again: carry on from the board (room_read {{room: \"{}\"}}).", r.name, need::provider_say(&prov), r.id)).await; }
    }
    /// Whether a member's provider sign-in works again since it failed (`f`: its failed record): Antigravity's token file was written after the failure (a sign-in
    /// or a refresh; agy keeps it in `~/.gemini/antigravity-cli/antigravity-oauth-token`, its keyring store leaves the file's metadata written too), or the hub's own expired-sign-in record of a Claude or Codex account, there when it failed, is gone.
    fn signin_back(&self, alert_now: bool, provider: &str, f: &Value) -> bool {
        let at = f["at"].as_u64().unwrap_or(0);
        match provider {
            "antigravity" | "agy" => { let home = std::path::PathBuf::from(crate::plat::dirs::home_or_empty());
                std::fs::metadata(home.join(".gemini/antigravity-cli/antigravity-oauth-token")).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(false, |d| d.as_secs() > at) }
            _ => f["alert"] == true && !alert_now,
        }
    }

    // ---- the tools a member thread (and any thread, for swarm_create) calls ----
    pub async fn thread_tool(self: &Arc<Self>, thread: &str, name: &str, a: &Value) -> Res {
        match name {
            "room_post" => {
                let w = self.world().await;
                let Some(r) = thread_room(&w, thread, gs(a, "room")).cloned() else { return fail("no such room of yours (room_read lists the rooms you are in)") };
                if r.closed { return fail(&format!("#{} is closed", r.name)) }
                let member = format!("{THREAD}{thread}");
                let nm = member_meta(&w, &r.id, &member)["name"].as_str().unwrap_or("peer").to_string();
                let who = Who { member: member.clone(), meta: member, name: nm, thread: thread.into(), hop: 1, bot: false };
                let rr = RoomRef { id: r.id.clone(), name: r.name.clone(), members: r.members.clone() };
                match self.member_post(&w, now_s(), &who, &rr, a, vec![]).await { Ok((ans, p)) => { drop(w); self.post_finish(p, 1).await; (true, ans) } Err(e) => fail(&e) }
            }
            "room_read" => { let w = self.world().await; let mine: Vec<Room> = w.rooms.values().filter(|r| !r.deleted && r.members.iter().any(|m| m == &format!("{THREAD}{thread}"))).cloned().collect();
                let room = gs(a, "room");
                if room.is_empty() && mine.len() != 1 { return (true, json!({"rooms":mine.iter().map(|r| json!({"id":r.id,"name":r.name,"kind":r.kind,"state":r.state})).collect::<Vec<_>>(),"note":"name a room to read it"})) }
                let Some(r) = (if room.is_empty() { mine.first() } else { mine.iter().find(|r| r.id == room || r.name.eq_ignore_ascii_case(room)) }).cloned() else { return fail("no such room of yours") };
                drop(w); let lim = a.get("limit").and_then(Value::as_u64).unwrap_or(40).clamp(1, 200) as usize;
                (true, self.board(&r.id, gs(a, "since"), lim, true).await.unwrap_or(Value::Null)) }
            "swarm_create" => self.swarm_create(thread, a).await,
            "swarm_control" => self.swarm_control(thread, a).await,
            _ => fail("unknown tool"),
        }
    }

    /// swarm_create as `by` (a thread, "" for the person): the room, then each member launched with the brief.
    pub async fn swarm_create(self: &Arc<Self>, by: &str, a: &Value) -> Res {
        if crate::copy::on() { return fail("a copy run starts no swarm") }
        let topic: String = gs(a, "topic").trim().chars().take(60).collect(); if topic.is_empty() { return fail("a swarm needs a topic") }
        let ms: Vec<Value> = a.get("members").and_then(Value::as_array).cloned().unwrap_or_default();
        if ms.len() < 2 || ms.len() > MAX_MEMBERS { return fail(&format!("a swarm has 2 to {MAX_MEMBERS} members (each {{provider, model?, role?, account?}})")) }
        let caller = if by.is_empty() { None } else { self.inbox.info(by).await };
        let now = now_s(); let goal = gs(a, "goal").trim().to_string(); let seed = gs(a, "seed").trim().to_string();
        let (rid, plan, project) = { let w = self.world().await;
            let asked = gs(a, "project");
            let project = if !asked.is_empty() { w.projects.iter().find(|(id, p)| !p.2 && (id.as_str() == asked || p.0.eq_ignore_ascii_case(asked))).map(|(id, _)| id.clone()) } else { caller.as_ref().map(|c| c.project.clone()).filter(|p| !p.is_empty()) };
            let Some(project) = project else { return fail("name the swarm's project (project_list)") };
            let rid = self.id("r", now); let mut taken: Vec<String> = vec![]; let mut plan = vec![]; let mut evs = vec![];
            for (i, m) in ms.iter().enumerate() {
                let (prov, raw_model, role) = (gs(m, "provider"), gs(m, "model"), gs(m, "role"));
                let model = crate::settings::models::sanitize_model_now(prov, raw_model);
                let nm = gate::short_name(role, prov, &model, &taken); taken.push(nm.clone());
                let tid = self.id("t", now); let key = format!("{THREAD}{tid}");
                evs.push(json!({"$":"SettingSet","key":format!("room.member.{rid}.{key}"),"value":json!({"name":nm,"color":gate::color(i),"role":role,"provider":prov,"model":model}).to_string(),"at":now}));
                plan.push((tid, nm, m.clone()));
            }
            let mut budget = gate::SWARM_BUDGET; if let Some(b) = a.get("budget") { if let Ok(x) = serde_json::from_value::<Budget>(b.clone()) { budget = Budget { posts: if x.posts > 0 { x.posts } else { budget.posts }, turns: if x.turns > 0 { x.turns } else { budget.turns }, tokens: if x.tokens > 0 { x.tokens } else { budget.tokens } } } }
            let members: Vec<String> = plan.iter().map(|(t, _, _)| format!("{THREAD}{t}")).collect();
            let set = json!({"$":"RoomSet","id":rid,"name":topic,"members":members.join("\n"),"ephemeral":false,"closed":false,"kind":"swarm","project":project,"goal":goal,"by":by,"state":"","reason":"","budget":budget,"at":now});
            let mut all = vec![set]; all.extend(evs);
            let byname = if by.is_empty() { "you".to_string() } else { w.run_bot(by).map(|b| b.name.clone()).or_else(|| caller.as_ref().map(|c| c.title.clone())).unwrap_or_else(|| "the starter".into()) };
            let mut seedp = self.guard_post(&rid, &format!("Swarm started by {byname} with {}.{}{}", plan.iter().map(|p| p.1.clone()).collect::<Vec<_>>().join(", "), if goal.is_empty() { String::new() } else { format!(" Goal: {goal}.") }, if seed.is_empty() { String::new() } else { format!("\n\n{seed}") }), &[], true, now);
            seedp["from"] = json!("hub");
            all.push(seedp);
            if let Err(e) = self.hub.commit_many(&w.also(now, all)).await { return fail(&format!("{e:#}")) }
            (rid, plan, project) };
        { let _w = self.world().await; } // the member cache sees the room before any member's agent lists its tools
        let names: Vec<String> = plan.iter().map(|p| p.1.clone()).collect();
        let mut started = vec![]; let mut failed = vec![];
        for (tid, nm, m) in &plan {
            let peers: Vec<String> = names.iter().filter(|x| *x != nm).cloned().collect();
            let mut prompt = brief(&topic, &rid, &goal, nm, gs(m, "role"), &peers, false); if !seed.is_empty() { prompt.push_str(&format!("\n\nTo start with:\n{seed}")) }
            let model = crate::settings::models::sanitize_model_now(gs(m, "provider"), gs(m, "model"));
            let mut args = json!({"project":project,"prompt":prompt,"provider":gs(m, "provider"),"model":model,"title":format!("{topic} · {nm}"),"subthread":false,"report":false,"_id":tid});
            if !gs(m, "account").is_empty() { args["account"] = m["account"].clone() }
            match self.inbox.launch(by, args).await { Ok(t) if &t == tid => started.push(json!({"thread":t,"name":nm})), Ok(t) => failed.push(format!("{nm}: started as {t}, not the id the room lists")), Err(e) => failed.push(format!("{nm}: {e}")) }
        }
        if !failed.is_empty() { // a member that did not start is no member
            let w = self.world().await; if let Some(r) = w.room(&rid).cloned() { let keep: Vec<String> = started.iter().map(|s| format!("{THREAD}{}", s["thread"].as_str().unwrap_or(""))).collect();
                let _ = self.hub.commit_many(&w.also(now_s(), vec![r.set_ev(now_s(), |x| x.members.retain(|m| keep.contains(m))), self.guard_post(&rid, &format!("Not started: {}", failed.join("; ")), &[], true, now_s())])).await; }
        }
        (true, json!({"room":rid,"members":started,"failed":failed,"note":"The swarm runs on its own: its members post on the board (room_read it), and you hear when it pauses or ends with a synthesis. swarm_control pauses, resumes, stops, adds or removes members."}))
    }

    /// swarm_control: the person ("") or the swarm's starter (the uberbot for the person) pauses, resumes, stops, adds or removes a member, or sets the budget.
    pub async fn swarm_control(self: &Arc<Self>, by: &str, a: &Value) -> Res {
        let now = now_s(); let act = gs(a, "action");
        let w = self.world().await;
        let room = gs(a, "room"); let room = match room.rsplit_once('|') { Some((_, id)) if !w.rooms.contains_key(room) && w.rooms.contains_key(id) => id, _ => room }; // a row's id may carry its hub ("<hub>|<room>")
        let Some(r) = w.rooms.values().find(|r| !r.deleted && r.swarm() && (r.id == room || r.name.eq_ignore_ascii_case(room))).cloned() else { return fail("no such swarm") };
        if !by.is_empty() && by != r.by && w.triage_thread() != by { return fail("only the person or the swarm's starter controls it; members are equals and post on the board instead") }
        if r.ended() && act != "budget" { return fail(&format!("the swarm is over ({})", r.state)) }
        let who = if by.is_empty() { "the person".to_string() } else { w.run_bot(by).map(|b| b.name.clone()).unwrap_or_else(|| "its starter".into()) };
        match act {
            "pause" => { let ev = vec![r.set_ev(now, |x| { x.state = "paused".into(); x.reason = "person".into() }), self.guard_post(&r.id, &format!("Paused by {who}."), &["pause"], true, now)];
                self.hub.commit_many(&w.also(now, ev)).await.map_or_else(|e| fail(&format!("{e:#}")), |_| (true, json!({"room":r.id,"state":"paused"}))) }
            // resume: the pause's facts go, the posts held while it was paused are delivered, and a budget that is still used up gets a little more
            // (resuming into a spent budget would pause again at the next post); raise: the part that ran out half again (the person's own word has no cap)
            "resume" | "raise" => {
                let used = w.room_used.get(&r.id).copied().unwrap_or_default();
                let hit = r.budget.hit(&used).or_else(|| need_rec(&w, &r.id)["part"].as_str().filter(|_| act == "raise").and_then(|p| ["posts", "turns", "tokens"].into_iter().find(|x| *x == p)));
                let mut budget = r.budget; let mut said = String::new();
                if let Some(part) = hit { let (cur, dflt) = (part_of(&r.budget, part).max(part_of(&used, part)), part_of(&gate::SWARM_BUDGET, part));
                    let to = if act == "raise" { cur + (cur / 2).max(dflt / 2).max(1) } else { cur + (dflt / 4).max(1) }; set_part(&mut budget, part, to); said = format!(" Budget {part}: {to}.") }
                let mut ev = vec![r.set_ev(now, |x| { x.state = String::new(); x.reason = String::new(); x.budget = budget }), self.guard_post(&r.id, &format!("Resumed by {who}.{said} Carry on from the board."), &["resume"], false, now)];
                ev.extend(self.resume_facts(&w, &r, by.is_empty(), now));
                if let Err(e) = self.hub.commit_many(&w.also(now, ev)).await { return fail(&format!("{e:#}")) }
                let ths = r.threads(); drop(w); self.give_later(&r.id, &ths, &ths, now).await; (true, json!({"room":r.id,"state":"active","budget":budget})) }
            // a member that failed: its sign-in (a Claude or Codex account opens its own sign-in; another CLI's sign-in runs in a terminal on the member's thread),
            // its turn again, or its work moved to a new Claude member (the failed one leaves)
            "signin" | "retry" | "move" => {
                let th = gs(a, "thread").rsplit('|').next().unwrap_or("").to_string(); let key = format!("{THREAD}{th}");
                if th.is_empty() || !r.members.contains(&key) { return fail("no such member (its thread)") }
                let meta = member_meta(&w, &r.id, &key); let nm = meta["name"].as_str().unwrap_or("peer").to_string();
                drop(w);
                let info = self.inbox.info(&th).await.unwrap_or_default();
                let prov = if info.provider.is_empty() { meta["provider"].as_str().unwrap_or("").to_string() } else { info.provider.clone() };
                match act {
                    "signin" => {
                        if matches!(prov.as_str(), "claude" | "codex") {
                            let raw = self.hub.setting(&crate::settings::accounts::binding_key(&th, &prov)).await.unwrap_or_default();
                            let id = serde_json::from_str::<Value>(&raw).ok().and_then(|b| b["id"].as_str().map(str::to_string)).unwrap_or_default();
                            if !id.is_empty() { return (true, json!({"room":r.id,"thread":th,"open":format!("account:{id}")})) }
                        }
                        let Some(cmd) = signin_cmd(&prov) else { return fail(&format!("{} signs in under Settings › Agents", need::provider_say(&prov))) };
                        let (ok, o) = crate::term::tools::call(&self.hub, &th, "terminal_open", &json!({"command":cmd,"title":format!("Sign in to {}", need::provider_say(&prov))})).await.unwrap_or((false, json!({"error":"terminals are not available"})));
                        if !ok { return fail(o["error"].as_str().unwrap_or("could not open a terminal")) }
                        (true, json!({"room":r.id,"thread":th,"open":format!("thread:{th}"),"terminal":o["id"]}))
                    }
                    "retry" => { let _ = self.inbox_ride(&th, &format!("room:{}:retry:{now}", r.id), &format!("[#{}] {who} asks you to carry on: your last turn failed. Read the board (room_read {{room: \"{}\"}}) and go on with your part.", r.name, r.id)).await;
                        (true, json!({"room":r.id,"thread":th})) }
                    _ => { // move: a Claude member with the same role joins, then the failed one leaves (its claims go)
                        let role = meta["role"].as_str().unwrap_or("").to_string();
                        let full = r.members.len() >= MAX_MEMBERS; // a full swarm frees the failed member's place first
                        if full { let (ok, x) = Box::pin(self.swarm_control(by, &json!({"room":r.id,"action":"remove","thread":th}))).await; if !ok { return (false, x) } }
                        let (ok, added) = Box::pin(self.swarm_control(by, &json!({"room":r.id,"action":"add","member":{"provider":"claude","role":role}}))).await;
                        if !ok { return (false, added) }
                        if !full { let (ok, x) = Box::pin(self.swarm_control(by, &json!({"room":r.id,"action":"remove","thread":th}))).await; if !ok { return (false, x) } }
                        let w = self.world().await; let mut rec = need_rec(&w, &r.id); if let Some(f) = rec["failed"].as_array_mut() { f.retain(|x| x["thread"] != th.as_str()) }
                        let _ = self.hub.commit_many(&w.also(now, vec![need_ev(&r.id, &rec, now), self.guard_post(&r.id, &format!("{nm}'s part moved to {} (Claude).", added["name"].as_str().unwrap_or("a new member")), &[], false, now)])).await;
                        (true, json!({"room":r.id,"removed":th,"thread":added["thread"],"name":added["name"]}))
                    }
                }
            }
            "stop" => { self.end(&w, &r, "stopped", "", &who, now).await; (true, json!({"room":r.id,"state":"stopped"})) }
            "budget" => { let x: Budget = serde_json::from_value(a.get("budget").cloned().unwrap_or(Value::Null)).unwrap_or_default();
                let ev = r.set_ev(now, |y| { if x.posts > 0 { y.budget.posts = x.posts } if x.turns > 0 { y.budget.turns = x.turns } if x.tokens > 0 { y.budget.tokens = x.tokens } });
                self.hub.commit_many(&w.also(now, vec![ev])).await.map_or_else(|e| fail(&format!("{e:#}")), |_| (true, json!({"room":r.id}))) }
            "remove" => { let th = gs(a, "thread"); let key = format!("{THREAD}{th}");
                let th = if r.members.contains(&key) { th.to_string() } else { names(&w, &r).into_iter().find(|(_, n)| n.eq_ignore_ascii_case(gs(a, "member")) || n.eq_ignore_ascii_case(th)).and_then(|(m, _)| m.strip_prefix(THREAD).map(str::to_string)).unwrap_or_default() };
                if th.is_empty() { return fail("no such member (its thread or name)") }
                let key = format!("{THREAD}{th}"); let nm = member_meta(&w, &r.id, &key)["name"].as_str().unwrap_or("peer").to_string();
                let mine: Vec<(String, Value)> = claims(&w, &r.id).into_iter().filter(|(_, c)| c["thread"] == th.as_str()).collect();
                let ev = vec![r.set_ev(now, |x| x.members.retain(|m| m != &key)), self.guard_post(&r.id, &format!("{nm} left the swarm ({who} removed it)."), &[], true, now)];
                if let Err(e) = self.hub.commit_many(&w.also(now, ev)).await { return fail(&format!("{e:#}")) }
                drop(w); self.release(&mine, now).await;
                let _ = self.inbox_note(&th, &format!("room:{}:removed", r.id), &format!("[#{}] You were removed from the swarm: nothing more is needed from you there.", r.name)).await;
                (true, json!({"room":r.id,"removed":th})) }
            "add" => { if r.members.len() >= MAX_MEMBERS { return fail(&format!("a swarm has at most {MAX_MEMBERS} members")) }
                let m = a.get("member").cloned().unwrap_or_else(|| json!({"provider":gs(a, "provider"),"model":gs(a, "model"),"role":gs(a, "role")}));
                let (prov, model) = (gs(&m, "provider"), crate::settings::models::sanitize_model_now(gs(&m, "provider"), gs(&m, "model")));
                let taken: Vec<String> = names(&w, &r).into_iter().map(|x| x.1).collect();
                let nm = gate::short_name(gs(&m, "role"), prov, &model, &taken);
                let tid = self.id("t", now); let key = format!("{THREAD}{tid}");
                let ann = self.guard_post(&r.id, &format!("{nm} joined the swarm (added by {who})."), &[], true, now); let ann_id = gs(&ann, "id").to_string();
                let ev = vec![r.set_ev(now, |x| x.members.push(key.clone())),
                    json!({"$":"SettingSet","key":format!("room.member.{}.{key}", r.id),"value":json!({"name":nm,"color":gate::color(r.members.len()),"role":gs(&m, "role"),"provider":prov,"model":model}).to_string(),"at":now}),
                    json!({"$":"SettingSet","key":format!("room.joined.{}.{key}", r.id),"value":ann_id,"at":now}), ann];
                if let Err(e) = self.hub.commit_many(&w.also(now, ev)).await { return fail(&format!("{e:#}")) }
                let project = r.project.clone(); drop(w); { let _w = self.world().await; }
                let mut args = json!({"project":project,"prompt":brief(&r.name, &r.id, &r.goal, &nm, gs(&m, "role"), &taken, true),"provider":prov,"model":model,"title":format!("{} · {nm}", r.name),"subthread":false,"report":false,"_id":tid});
                if !gs(&m, "account").is_empty() { args["account"] = m["account"].clone() }
                match self.inbox.launch(&r.by, args).await { Ok(t) => (true, json!({"room":r.id,"thread":t,"name":nm})), Err(e) => {
                    let w = self.world().await; if let Some(r2) = w.room(&r.id).cloned() { let _ = self.hub.commit_many(&w.also(now_s(), vec![r2.set_ev(now_s(), |x| x.members.retain(|m| m != &key))])).await; }
                    fail(&format!("{nm} did not start: {e}")) } } }
            _ => fail("action: pause, resume, raise, stop, add, remove, budget, signin, retry or move"),
        }
    }

    /// The board as the clients and room_read show it: the room, its members with their live state, the posts (after `since`), the open claims.
    pub async fn board(&self, room: &str, since: &str, limit: usize, for_agent: bool) -> Option<Value> {
        let (r, members, lines, cl, need_rec) = { let w = self.world().await; let r = w.room(room)?.clone(); let nr = need_rec(&w, room);
            let members: Vec<(String, String, Value)> = names(&w, &r).into_iter().map(|(m, n)| { let meta = member_meta(&w, &r.id, &m); (m, n, meta) }).collect();
            (r.clone(), members, w.room_lines.get(room).cloned().unwrap_or_default(), claims(&w, room), nr) };
        let used = self.world().await.room_used.get(room).copied().unwrap_or_default();
        let mut posts: Vec<Value> = vec![];
        for l in lines.iter().rev() { let Ok(raw) = self.hub.store.get(l - 1) else { continue }; let Ok(e) = serde_json::from_str::<Value>(&raw) else { continue };
            if !since.is_empty() && gs(&e, "id") == since { break } posts.push(e); if posts.len() >= limit { break } }
        posts.reverse();
        let color: HashMap<String, String> = members.iter().map(|(m, _, meta)| (m.clone(), meta["color"].as_str().unwrap_or("").to_string())).collect();
        let mut ms = vec![];
        for (i, (m, n, meta)) in members.iter().enumerate() {
            let (thread, info) = match m.strip_prefix(THREAD) { Some(t) => (t.to_string(), self.inbox.info(t).await), None => (String::new(), None) };
            let c = if meta["color"].is_string() { meta["color"].as_str().unwrap().to_string() } else { gate::color(i).to_string() };
            ms.push(json!({"member":m,"thread":thread,"name":n,"color":c,"role":meta["role"],"provider":info.as_ref().map(|x| x.provider.clone()).unwrap_or_else(|| meta["provider"].as_str().unwrap_or("").into()),
                "model":info.as_ref().map(|x| x.model.clone()).unwrap_or_else(|| meta["model"].as_str().unwrap_or("").into()),"state":info.as_ref().map_or(if thread.is_empty() { "bot" } else { "removed" }, |x| x.word()),"title":info.map(|x| x.title).unwrap_or_default()}));
        }
        // a post held while the swarm was paused and released by a resume after it is no longer held (it was delivered)
        let resumed = posts.iter().rposition(resumes);
        let posts: Vec<Value> = posts.into_iter().enumerate().map(|(i, mut e)| { if resumed.map_or(false, |at| i < at) && e["guard"] != true && !gs(&e, "held").is_empty() && gs(&e, "held") != "backstop" { e["held"] = json!(""); e["released"] = json!(true) } e })
            .map(|e| { let th = gs(&e, "thread"); let key = if th.is_empty() { String::new() } else { format!("{THREAD}{th}") };
            let mut p = json!({"id":e["id"],"from":e["from"],"thread":th,"color":color.get(&key).cloned().unwrap_or_default(),"text":e["text"],"tags":e.get("tags").cloned().unwrap_or(json!([])),"reply":e.get("reply").cloned().unwrap_or(json!("")),
                "mentions":e.get("mentions").cloned().unwrap_or(json!([])),"files":e.get("files").cloned().unwrap_or(json!([])),"guard":e["guard"] == true,"held":e.get("held").cloned().unwrap_or(json!("")),"released":e["released"] == true,"at":e["at"]});
            // what a client draws a copy button (and Run in terminal) for: the post's code (md/copy.rs), rendered Markdown blocks, and images for data/1 clients
            if !for_agent {
                let text = e["text"].as_str().unwrap_or("");
                let c = crate::md::copy::spans(text); if !c.is_empty() { p["copy"] = json!(c); }
                if e["guard"] != true {
                    p["blocks"] = json!(crate::present::thread::md_blocks(text));
                    if crate::files::mimg::may(text) {
                        let imgs = crate::files::mimg::paths(text);
                        if !imgs.is_empty() { p["images"] = json!(imgs); }
                    }
                }
            }
            if for_agent { let o = p.as_object_mut().unwrap(); o.retain(|k, v| !matches!(k.as_str(), "color") && !(v == &json!("") || v == &json!([]) || v == &json!(false))); } p }).collect();
        let claims: Vec<Value> = cl.into_iter().map(|(p, c)| json!({"post":p,"thread":c["thread"],"files":c["files"],"state":c["state"]})).collect();
        // what it wants of the person, as its row in Needs you says it (present/swarm_need.rs)
        let need_now = { let ms: Vec<need::Member> = ms.iter().filter(|m| !gs(m, "thread").is_empty()).map(|m| need::Member { thread: gs(m, "thread"), name: gs(m, "name"), provider: gs(m, "provider"), failed: m["state"] == "failed", asking: m["state"] == "waiting" }).collect();
            need::of(&r.state, &r.reason, &need_rec, &ms).map_or(Value::Null, |n| n.json("", &r.id)) };
        let _ = HashSet::<()>::new;
        Some(json!({"room":{"id":r.id,"name":r.name,"kind":r.kind,"project":r.project,"goal":r.goal,"state":if r.state.is_empty() { "active" } else { r.state.as_str() },"reason":r.reason,"by":r.by,"budget":r.budget,"used":used,"need":need_now},
            "members":ms,"posts":posts,"claims":claims}))
    }
    /// The swarms of a project (all with ""), newest first: the sidebar's rows and swarm.list.
    pub async fn swarm_list(&self, project: &str) -> Vec<Value> {
        let w = self.world().await; let mut v: Vec<&Room> = w.rooms.values().filter(|r| !r.deleted && r.swarm() && (project.is_empty() || r.project == project)).collect();
        v.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.cmp(&a.id)));
        v.into_iter().map(|r| json!({"id":r.id,"name":r.name,"project":r.project,"goal":r.goal,"state":if r.state.is_empty() { "active" } else { r.state.as_str() },"reason":r.reason,"members":r.threads().len(),"last":r.last})).collect()
    }
}

/// a thread's room by id or name (one it is a member of)
fn thread_room<'a>(w: &'a World, thread: &str, room: &str) -> Option<&'a Room> {
    let key = format!("{THREAD}{thread}");
    let mine: Vec<&Room> = w.rooms.values().filter(|r| !r.deleted && r.members.contains(&key)).collect();
    if room.is_empty() { return if mine.len() == 1 { mine.into_iter().next() } else { None } }
    mine.into_iter().find(|r| r.id == room || r.name.eq_ignore_ascii_case(room))
}

/// The tool definitions: swarm_create and swarm_control for every thread; room_post and room_read (tools.json's, extended) for a member thread.
pub fn defs() -> Vec<Value> {
    serde_json::from_value(json!([
        {"name":"swarm_create","description":"Start a swarm: a group of peer threads (any providers and models) that work on one goal together with no lead, talking on one shared board (a permanent room the person sees). Each member starts with a brief that says it is one of N equals, to post ideas, critiques and claims on the board, build on the others' and claim work before doing it. Members' turns are woken by new posts, batched; guards pause the swarm when it loops, chatters without progress, goes idle or uses up its budget. You hear when it pauses or ends: any member or the person posts a synthesis, which is reported to you. Only when the person asks for a swarm, or several minds on one problem clearly help.","inputSchema":{"type":"object","properties":{
            "project":{"type":"string","description":"The project's name or id (default: this thread's; bots must give one)."},
            "topic":{"type":"string","description":"The swarm's name, a few words."},
            "goal":{"type":"string","description":"What it is to achieve, in a line."},
            "members":{"type":"array","description":"2 to 8 members.","items":{"type":"object","properties":{"provider":{"type":"string","description":"claude, codex, antigravity, grok, opencode, pi, omp, muse"},"model":{"type":"string"},"role":{"type":"string","description":"A hint of its strength (its name on the board comes from it)."},"account":{"type":"string"}},"required":["provider"]}},
            "seed":{"type":"string","description":"The first message every member gets after the brief: the problem, its context, paths."},
            "budget":{"type":"object","description":"Optional limits: posts, turns, tokens (defaults 300, 120, 4000000).","properties":{"posts":{"type":"number"},"turns":{"type":"number"},"tokens":{"type":"number"}}}},"required":["topic","members"]}},
        {"name":"swarm_control","description":"For the swarm's starter acting for the person: pause, resume, stop it, add or remove a member, or change its budget. Members cannot control it; they post on the board.","inputSchema":{"type":"object","properties":{
            "room":{"type":"string","description":"The swarm's room id or name."},
            "action":{"type":"string","enum":["pause","resume","raise","stop","add","remove","budget","signin","retry","move"]},
            "thread":{"type":"string","description":"remove, retry, move, signin: the member's thread (remove also takes member: its name). raise: the budget part that ran out half again, then resume; move: a Claude member takes a failed member's part."},
            "member":{"description":"add: {provider, model?, role?, account?}; remove: the member's name."},
            "budget":{"type":"object","properties":{"posts":{"type":"number"},"turns":{"type":"number"},"tokens":{"type":"number"}}}},"required":["room","action"]}}
    ])).unwrap()
}
pub fn room_defs() -> Vec<Value> { super::tools::defs().into_iter().filter(|d| matches!(d["name"].as_str(), Some("room_post") | Some("room_read"))).collect() }

// ---- the clients' requests ----
struct SwarmRpc(Arc<Bots>);
fn ok(id: &Value, r: Value) -> Vec<Value> { vec![json!({"t":"reply","id":id,"ok":true,"result":r})] }
fn err(id: &Value, e: &str) -> Vec<Value> { vec![json!({"t":"reply","id":id,"ok":false,"error":e})] }
impl RpcHandler for SwarmRpc {
    fn methods(&self) -> &'static [&'static str] { &["swarm.create", "swarm.get", "swarm.list", "swarm.post", "swarm.act"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move { let b = &self.0;
            match method {
                "swarm.create" => { let (o, r) = b.swarm_create("", p).await; if o { ok(id, r) } else { err(id, r["error"].as_str().unwrap_or("")) } }
                "swarm.get" => match b.board(gs(p, "room"), gs(p, "since"), p.get("limit").and_then(Value::as_u64).unwrap_or(200).clamp(1, 500) as usize, false).await { Some(v) => ok(id, v), None => err(id, "no such room") },
                "swarm.list" => ok(id, json!({"swarms":b.swarm_list(gs(p, "project")).await})),
                "swarm.post" => { let mut q = json!({"room":gs(p, "room"),"text":gs(p, "text")}); if p.get("tags").is_some() { q["tags"] = p["tags"].clone() } if !gs(p, "msg").is_empty() { q["msg"] = p["msg"].clone() }
                    let r = b.post_person(&q).await; match r { Ok(()) => ok(id, json!({})), Err(e) => err(id, &e) } }
                "swarm.act" => { let (o, r) = b.swarm_control("", p).await; if o { ok(id, r) } else { err(id, r["error"].as_str().unwrap_or("")) } }
                _ => err(id, "unknown method"),
            }
        })
    }
}

//! What other modules ask the bots lane (the session actor for a view whose target is a bot). Free functions over the hub: they find the
//! `Bots` started on it (`start` registers it) and answer from its world. Not started (a test hub): nothing known.
use super::rpc::Bots;
use crate::hub::Hub;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, Weak};

static REG: Mutex<Vec<(usize, Weak<Bots>)>> = Mutex::new(Vec::new());
/// Called by `Bots::start_with`: the hub's bots, found again by `of`.
pub(super) fn register(hub: &Arc<Hub>, b: &Arc<Bots>) {
    let mut r = REG.lock().unwrap(); r.retain(|(_, w)| w.strong_count() > 0); r.push((Arc::as_ptr(hub) as usize, Arc::downgrade(b)));
}
/// the bots lane started on this hub
pub fn of(hub: &Arc<Hub>) -> Option<Arc<Bots>> { let p = Arc::as_ptr(hub) as usize; REG.lock().unwrap().iter().find(|(k, _)| *k == p).and_then(|(_, w)| w.upgrade()) }

/// `U.Bots.chat.of(st, id, B.Bot.thread(bot))`: the thread a bot's Chat tab shows (its own; the uberbot's is where the conversation runs now,
/// `Ub.chat`: this machine's while it holds the lease or no other machine's thread is known).
/// None for a bot that is not known.
pub async fn chat_thread(hub: &Arc<Hub>, bot: &str) -> Option<String> {
    let b = of(hub)?; let w = b.world().await;
    let own = w.bot(bot).map(|b| b.thread.clone()).unwrap_or_default();
    let th = if w.uber_is(bot) { w.ub_chat() } else { own };
    (!th.is_empty()).then_some(th)
}
/// `BU.tab.dflt`: every client opens a bot on Chat, the uberbot too (Status is not drawn yet, and it stays an explicit choice).
pub fn default_tab() -> &'static str { "chat" }

/// BU.tabs.mail, BU.nav.tabs: a bot's tabs; the uberbot's bar folds Status under Mailroom, Tasks under Browser and Hooks under Settings
const TABS: [&str; 9] = ["chat", "mail", "space", "browser", "tasks", "memory", "routines", "hooks", "settings"];
const UBER_TABS: [&str; 7] = ["chat", "mail", "space", "browser", "memory", "routines", "settings"];
/// BU.tab.label
fn label(t: &str) -> &'static str { match t { "chat" => "Chat", "mail" => "Mailroom", "status" => "Status", "space" => "Space", "browser" => "Browser", "tasks" => "Tasks", "memory" => "Memory", "routines" => "Routines", "hooks" => "Webhooks", _ => "Settings" } }
fn tabs_json(ts: &[&str]) -> Vec<Value> { ts.iter().map(|t| json!({"id":t,"label":label(t)})).collect() }
/// BU.nav.parent
fn nav_parent(folded: bool, t: &str) -> &str { if !folded { return t } match t { "status" => "mail", "tasks" => "browser", "hooks" => "settings", _ => t } }
/// BU.nav.sections
fn nav_sections(folded: bool, parent: &str) -> &'static [&'static str] { if !folded { return &[] } match parent { "mail" => &["mail", "status"], "browser" => &["browser", "tasks"], "settings" => &["settings", "hooks"], _ => &[] } }
/// What the Memory tab carries in a view: `shown` and `overlay` both carry it, a reply over REPLY_BYTES is refused by the client, and an uberbot's memories can be many times that
const MEMORY_BYTES: usize = 64 * 1024;
/// Where each page of the Memory tab starts: whole memories up to MEMORY_BYTES a page (one bigger than a page stands alone).
fn memory_pages(sizes: impl Iterator<Item = usize>) -> Vec<usize> {
    let (mut starts, mut left) = (vec![0usize], MEMORY_BYTES);
    for (i, n) in sizes.enumerate() { if n > left && i > *starts.last().unwrap() { starts.push(i); left = MEMORY_BYTES } left = left.saturating_sub(n) }
    starts
}
/// BU.ago, in words short enough for a row
fn ago(now: u64, at: u64) -> String { if at == 0 { return "never".into() } let s = now.saturating_sub(at); if s < 60 { "just now".into() } else if s < 3600 { format!("{}m ago", s / 60) } else if s < 86400 { format!("{}h ago", s / 3600) } else { format!("{}d ago", s / 86400) } }

/// MBots.bot.m for a bot here, without what the presenter adds from its own state (mood, note): its header, tabs and the page of `tab` (BU.tab.of: the tab asked for when the bot has it, else Chat).
/// `chat` is the thread its Chat shows. None for a bot that is not known, or one of another machine's (not ported).
pub async fn page(hub: &Arc<Hub>, bot: &str, tab: &str, now: u64) -> Option<Value> { page_from(hub, bot, tab, now, 0).await }
/// `from`: the first memory of the Memory tab's page (an index the page boundaries gave: `memoryNext` and `memoryPrev` of the reply; any other number is rounded down to a boundary).
pub async fn page_from(hub: &Arc<Hub>, bot: &str, tab: &str, now: u64, from: usize) -> Option<Value> {
    if bot.contains('~') { return None }
    let b = of(hub)?; let w = b.world().await;
    let me = w.bot(bot).filter(|b| !b.deleted)?.clone();
    let uber = w.uber_is(bot); let tabs: &[&str] = if uber { &UBER_TABS } else { &TABS };
    let on = if tabs.contains(&nav_parent(uber, tab)) { tab } else { default_tab() };
    let parent = nav_parent(uber, on);
    let chat = if uber { w.ub_chat() } else { me.thread.clone() };
    let mut v = json!({"kind":"bot","id":bot,"name":me.name,"title":me.name,"look":me.look,"tab":on,"navTab":parent,"tabs":tabs_json(tabs),"subtabs":tabs_json(nav_sections(uber, parent)),"edit":true,"uber":uber,"chat":chat});
    match on {
        "memory" => {
            let mut ms: Vec<_> = w.mems.values().filter(|m| m.bot == bot).collect(); ms.sort_by(|a, c| c.updated.cmp(&a.updated).then_with(|| a.key.cmp(&c.key)));
            let starts = memory_pages(ms.iter().map(|m| m.text.len() + m.key.len() + 96));
            let at = starts.iter().rposition(|s| *s <= from).unwrap_or(0); let first = starts.get(at).copied().unwrap_or(0);
            let end = starts.get(at + 1).copied().unwrap_or(ms.len());
            let rows: Vec<Value> = ms[first..end].iter().map(|m| json!({"key":m.key,"kind":m.kind,"text":m.text,"tags":m.tags,"updated":ago(now, m.updated)})).collect();
            v["memoryMore"] = json!(ms.len() - end); v["memoryFrom"] = json!(first); v["memoryTotal"] = json!(ms.len()); v["memory"] = Value::Array(rows);
            if end < ms.len() { v["memoryNext"] = json!(end) }
            if at > 0 { v["memoryPrev"] = json!(starts[at - 1]) }
        }
        "routines" => {
            let z = super::clock::Zones { settings: &w.settings };
            let mut rs: Vec<_> = w.routines.values().filter(|r| r.bot == bot && !r.deleted).collect(); rs.sort_by(|a, c| a.id.cmp(&c.id));
            v["routines"] = rs.iter().map(|r| { let s = super::sched_text::row(&z, r, now);
                json!({"id":r.id,"name":r.name,"cron":r.cron,"when":s.when,"prompt":r.prompt,"on":r.on,"last":ago(now, r.last),"kind":s.kind,"label":s.when,"next":s.next_text,"rel":s.rel,"done":s.done}) }).collect();
        }
        "hooks" => {
            let mut hs: Vec<_> = w.hooks.values().filter(|h| h.bot == bot && !h.revoked).collect(); hs.sort_by(|a, c| a.id.cmp(&c.id));
            v["hooks"] = hs.iter().map(|h| json!({"id":h.id,"name":h.name,"path":format!("/hook/{}", h.id),"count":h.count,"last":ago(now, h.last)})).collect();
        }
        "status" => {
            // the plane as text, what the uberbot's Status tab and plane_read show (Plane.of / Plane.text)
            let rooms = b.room_news(&w, now).await;
            let text = crate::statehost::policies().and_then(|p| p.with_state(|st| super::plane::read(st, &w, &rooms, now, false)));
            let text = match text { Some(t) => t, None => { let st = b.state_folded(); super::plane::read(&st, &w, &rooms, now, false) } };
            v["plane"] = json!(text);
        }
        "mail" => {
            let l = super::mail::requests::list(&w, &json!({"bot":bot,"n":40}));
            let rows: Vec<Value> = l["convs"].as_array().map(|a| a.iter().map(|c| json!({"id":c["id"],"subject":c["subject"],"from":c["from"],"head":c["head"],"n":c["n"],"unread":c["unread"],"status":c["status"],"ago":ago(now, c["last"].as_u64().unwrap_or(0))})).collect()).unwrap_or_default();
            v["mail"] = Value::Array(rows); v["mailTotal"] = l["total"].clone();
        }
        "space" => {
            let spec = w.spaces.get(bot).map(String::as_str).unwrap_or("");
            v["space"] = serde_json::from_str::<Value>(spec).ok().filter(Value::is_object).unwrap_or(Value::Null);
            v["spacePage"] = json!(w.pages.get(bot).map_or(false, |p| !p.0.is_empty()));
        }
        "tasks" => { if let Some(d) = crate::term::driver::get(hub) { let t = d.tasks_view(bot).await; v["tasks"] = t["items"].clone(); v["taskRuns"] = t["runs"].clone(); } }
        "settings" => { v["persona"] = json!(me.persona); v["created"] = json!(ago(now, me.created)); }
        _ => {}
    }
    Some(v)
}

/// The phone data API (data/1 `bots.list`): this machine's live bots and rooms. A bot's conversation is its own thread (open it like any thread).
pub async fn directory(hub: &Arc<Hub>) -> serde_json::Value {
    let Some(b) = of(hub) else { return serde_json::json!({"bots":[],"rooms":[]}) };
    let w = b.world().await;
    let mut bots: Vec<_> = w.bots.values().filter(|x| !x.deleted).map(|x| (x.created, serde_json::json!({"id":x.id,"name":x.name,"thread":x.thread,"uber":w.uber_is(&x.id)}))).collect();
    bots.sort_by_key(|x| x.0);
    let mut rooms: Vec<_> = w.rooms.values().filter(|x| !x.deleted).map(|x| (x.created, serde_json::json!({"id":x.id,"name":x.name,"members":x.members}))).collect();
    rooms.sort_by_key(|x| x.0);
    serde_json::json!({"bots":bots.into_iter().map(|x| x.1).collect::<Vec<_>>(),"rooms":rooms.into_iter().map(|x| x.1).collect::<Vec<_>>()})
}
/// `room.posts`: the newest `limit` posts of a room, oldest first (`{id, from, text, at}`).
pub async fn room_posts(hub: &Arc<Hub>, room: &str, limit: usize) -> Option<Vec<serde_json::Value>> {
    let b = of(hub)?; let w = b.world().await;
    w.room(room)?;
    let lines = w.room_lines.get(room).cloned().unwrap_or_default();
    let take: Vec<usize> = lines.iter().rev().take(limit).rev().copied().collect();
    drop(w);
    let mut out = vec![];
    for l in take { if let Ok(raw) = hub.store.get(l - 1) { if let Ok(e) = serde_json::from_str::<serde_json::Value>(&raw) { out.push(serde_json::json!({"id":e["id"],"from":e["from"],"text":e["text"],"at":e["at"]})) } } }
    Some(out)
}

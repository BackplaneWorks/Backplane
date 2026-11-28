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
pub fn of(hub: &Arc<Hub>) -> Option<Arc<Bots>> { of_ptr(hub) }
pub(crate) fn of_ptr(hub: &Hub) -> Option<Arc<Bots>> { let p = std::ptr::from_ref(hub) as usize; REG.lock().unwrap().iter().find(|(k, _)| *k == p).and_then(|(_, w)| w.upgrade()) }

/// `U.Bots.chat.of(st, id, B.Bot.thread(bot))`: the thread a bot's Chat tab shows (its own; the uberbot's is where the conversation runs now,
/// `Ub.chat`: this machine's while it holds the lease or no other machine's thread is known).
/// None for a bot that is not known.
pub async fn chat_thread(hub: &Arc<Hub>, bot: &str) -> Option<String> {
    // a bot of a linked machine: its own thread there, as the mirror names it
    if bot.contains('~') { let mut th = None; on_far(hub, &crate::net::far::link(bot), |_, w| th = w.bot(bot).filter(|b| !b.deleted).map(|b| b.thread.clone())); return th.filter(|t| crate::net::far::is(t)) }
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
/// A mailroom row's sender as the person knows it: a bot by its name, a thread by its title (the address, `bot:<id>` / `thread:<id>`, when neither is known).
pub(crate) fn mail_who(w: &super::world::World, addr: &str, title: &dyn Fn(&str) -> Option<String>) -> String {
    if let Some(id) = addr.strip_prefix("bot:") { if let Some(b) = w.bot(id) { return b.name.clone() } }
    if let Some(th) = addr.strip_prefix("thread:") { if let Some(t) = thread_name(w, th, title) { return t } }
    addr.to_string()
}
/// A thread as the person knows it: the bot whose chat it is, else its title.
fn thread_name(w: &super::world::World, th: &str, title: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    w.bots.values().find(|b| !b.deleted && b.thread == th).map(|b| b.name.clone()).or_else(|| title(th))
}
/// A mailroom row's first line with each `backplane://thread/<id>` the hub wrote into it (a bot's report) named as `thread_name` names it.
/// What the bots were sent is not changed: only the row shows names.
pub(crate) fn mail_head(w: &super::world::World, head: &str, title: &dyn Fn(&str) -> Option<String>) -> String {
    const REF: &str = "backplane://thread/";
    let mut out = String::new(); let mut rest = head;
    while let Some(i) = rest.find(REF) {
        out.push_str(&rest[..i]); let after = &rest[i + REF.len()..];
        let n = after.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~'))).unwrap_or(after.len());
        match thread_name(w, &after[..n], title) { Some(t) if n > 0 => out.push_str(&t), _ => { out.push_str(REF); out.push_str(&after[..n]); } }
        rest = &after[n..];
    }
    out.push_str(rest); out
}
/// A routine row's last run, for the person who pressed Run or waits for the next: "ran just now", "ran 5m ago"; empty when it never ran.
pub(crate) fn ran_text(now: u64, last: u64) -> String { if last == 0 { String::new() } else { format!("ran {}", ago(now, last)) } }
/// How long after a routine fired a running turn of its bot is taken as that routine's run (Plane's ROUTINE_WINDOW).
pub(crate) const ROUTINE_RUN_WINDOW: u64 = 900;
/// Whether a routine that last fired at `last` is running now: its bot's turn runs and it fired within the window.
pub(crate) fn routine_running(turn_running: bool, last: u64, now: u64) -> bool { turn_running && last > 0 && now <= last + ROUTINE_RUN_WINDOW }
/// Whether the hub's folded state has `thread`'s turn running.
pub(crate) fn turn_running(thread: &str) -> bool {
    !thread.is_empty() && crate::statehost::policies().and_then(|p| p.with_state(|st| st.threads.get(thread).map(|t| t.turn == crate::state::Turn::Running))).flatten().unwrap_or(false)
}
fn ago(now: u64, at: u64) -> String { if at == 0 { return "never".into() } let s = now.saturating_sub(at); if s < 60 { "just now".into() } else if s < 3600 { format!("{}m ago", s / 60) } else if s < 86400 { format!("{}h ago", s / 3600) } else { format!("{}d ago", s / 86400) } }

/// MBots.bot.m for a bot here, without what the presenter adds from its own state (mood, note): its header, tabs and the page of `tab` (BU.tab.of: the tab asked for when the bot has it, else Chat).
/// `chat` is the thread its Chat shows. None for a bot that is not known; one of a linked machine is read from its mirror (`far_page`).
pub async fn page(hub: &Arc<Hub>, bot: &str, tab: &str, now: u64) -> Option<Value> { page_from(hub, bot, tab, now, 0).await }
/// `from`: the first memory of the Memory tab's page (an index the page boundaries gave: `memoryNext` and `memoryPrev` of the reply; any other number is rounded down to a boundary).
pub async fn page_from(hub: &Arc<Hub>, bot: &str, tab: &str, now: u64, from: usize) -> Option<Value> {
    if bot.contains('~') {
        let link = crate::net::far::link(bot);
        let uber = far_uber(hub).await == bot;
        let mut v = None; on_far(hub, &link, |mark, w| v = far_page(w, bot, tab, now, from, uber, mark));
        return v
    }
    let b = of(hub)?; let w = b.world().await;
    let me = w.bot(bot).filter(|b| !b.deleted)?.clone();
    let uber = w.uber_is(bot); let tabs: &[&str] = if uber { &UBER_TABS } else { &TABS };
    let on = if tabs.contains(&nav_parent(uber, tab)) { tab } else { default_tab() };
    let parent = nav_parent(uber, on);
    let chat = if uber { w.ub_chat() } else { me.thread.clone() };
    let mut v = json!({"kind":"bot","id":bot,"name":me.name,"title":me.name,"look":me.look,"color":me.color,"auto":me.auto,"tab":on,"navTab":parent,"tabs":tabs_json(tabs),"subtabs":tabs_json(nav_sections(uber, parent)),"edit":true,"uber":uber,"chat":chat});
    // Delete bot: any bot of this hub but the uberbot (it is taken back first); the clients ask before they send it
    if !uber { v["delete"] = json!({"label":"Delete bot","action":"bot-delete","value":bot,"ask":delete_ask(&me.name)}); }
    // Settings: its name and cat, the control setup's Your uberbot draws (present/onboard_uber.rs), its presses `bot-cat` (present/screen.rs)
    if on == "settings" { v["catpick"] = crate::present::onboard_uber::cat_row(&format!("bcn.{bot}"), &format!("bcc.{bot}"), &me.name, "", (!me.auto).then_some(me.look), &me.color, "bot-cat", Some("Save name"), ""); }
    if tab_of_world(&w, bot, on, now, from, &mut v) {
        // Routines: the one whose turn is running now says so (its Run does nothing until it ends: routines.rs `routine_run`)
        if on == "routines" && !uber { let busy = turn_running(&me.thread); if let Some(rs) = v["routines"].as_array_mut() { for r in rs { let last = w.routines.get(r["id"].as_str().unwrap_or("")).map_or(0, |x| x.last); r["running"] = json!(routine_running(busy, last, now)); } } }
        // Settings: the memory tree's state (its engine reads the world itself)
        if on == "settings" && v["recall"].is_object() { drop(w); if let Some(rc) = b.recall.get() { let (n, lines, left) = rc.stats(&me.thread).await; v["recall"]["messages"] = json!(n); v["recall"]["lines"] = json!(lines); v["recall"]["left"] = json!(left); if v["recall"]["on"] == true && rc.on(&me.thread) { v["recall"]["progress"] = rc.publish(&me.thread, false).await; } } }
        return Some(v)
    }
    match on {
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
            let title = |th: &str| crate::statehost::policies().and_then(|p| p.with_state(|st| st.threads.get(th).map(|t| t.title.clone()))).flatten().filter(|t| !t.is_empty());
            let rows: Vec<Value> = l["convs"].as_array().map(|a| a.iter().map(|c| json!({"id":c["id"],"subject":c["subject"],"from":mail_who(&w, c["from"].as_str().unwrap_or(""), &title),"head":mail_head(&w, c["head"].as_str().unwrap_or(""), &title),"n":c["n"],"unread":c["unread"],"status":c["status"],"ago":ago(now, c["last"].as_u64().unwrap_or(0))})).collect()).unwrap_or_default();
            v["mail"] = Value::Array(rows); v["mailTotal"] = l["total"].clone();
        }
        "tasks" => { if let Some(d) = crate::term::driver::get(hub) { let t = d.tasks_view(bot).await; v["tasks"] = t["items"].clone(); v["taskRuns"] = t["runs"].clone(); } }
        _ => {}
    }
    Some(v)
}

/// The tabs read from a bots world alone (this hub's, or a linked machine's mirror): Memory, Routines, Space and Settings fill `v` and say true.
fn tab_of_world(w: &super::world::World, bot: &str, on: &str, now: u64, from: usize, v: &mut Value) -> bool {
    let Some(me) = w.bot(bot) else { return false };
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
                json!({"id":r.id,"name":r.name,"cron":r.cron,"when":s.when,"prompt":r.prompt,"on":r.on,"last":ago(now, r.last),"ran":ran_text(now, r.last),"kind":s.kind,"label":s.when,"next":s.next_text,"rel":s.rel,"done":s.done}) }).collect();
        }
        "space" => {
            let spec = w.spaces.get(bot).map(String::as_str).unwrap_or("");
            v["space"] = serde_json::from_str::<Value>(spec).ok().filter(Value::is_object).unwrap_or(Value::Null);
            v["spacePage"] = json!(w.pages.get(bot).map_or(false, |p| !p.0.is_empty()));
        }
        "settings" => { v["persona"] = json!(me.persona); v["created"] = json!(ago(now, me.created)); if super::recall::enabled(w) { v["recall"] = json!({"on": super::recall::bot_on(w, bot)}) } }
        _ => return false,
    }
    true
}

/// A linked machine's bot, read-only from its mirror: its chat (its thread there, which takes messages through the link), space, memory, routines and
/// settings. `edit` is false and `mark` names the machine: changes are made there (a request about it is answered "that is done on its own machine").
pub fn far_page(w: &super::world::World, bot: &str, tab: &str, now: u64, from: usize, uber: bool, mark: &Value) -> Option<Value> {
    let me = w.bot(bot).filter(|b| !b.deleted)?;
    let on = if FAR_TABS.contains(&tab) { tab } else { default_tab() };
    let mut v = json!({"kind":"bot","id":bot,"name":me.name,"title":me.name,"look":me.look,"color":me.color,"auto":me.auto,"tab":on,"navTab":on,"tabs":tabs_json(&FAR_TABS),"subtabs":[],"edit":false,"uber":uber,"chat":me.thread,"mark":mark});
    tab_of_world(w, bot, on, now, from, &mut v);
    Some(v)
}
/// What a bot of a linked machine shows here: what its mirror holds
const FAR_TABS: [&str; 5] = ["chat", "space", "memory", "routines", "settings"];

/// The uberbot a linked machine's mirror names, when it is a live bot there ("" for none)
pub fn mirrored_uber(w: &super::world::World) -> String { let b = w.setting("uber.bot", ""); if crate::net::far::is(&b) && w.live(&b) { b } else { String::new() } }
/// Which uberbot this hub's clients show: this hub's own; with none, the first linked machine's (in the order the machines are shown) that has one
pub fn uber_shown(here: &str, far: &[String]) -> String { if !here.is_empty() { return here.to_string() } far.iter().find(|b| !b.is_empty()).cloned().unwrap_or_default() }
/// Each shown linked machine's bots world: `f(link, its marker, world)`, in the machines' order
fn each_far(hub: &Arc<Hub>, mut f: impl FnMut(&str, &Value, &super::world::World)) {
    let Some(fv) = hub.far() else { return };
    let m = hub.mirrors();
    for (link, name) in m.names() { let mark = m.mark(&name); fv.bots(&link, &mut |w| f(&link, &mark, w)); }
}
/// One shown linked machine's bots world (nothing when it is not shown: sync off, or not the owner's)
fn on_far(hub: &Arc<Hub>, link: &str, mut f: impl FnMut(&Value, &super::world::World)) { each_far(hub, |l, mark, w| if l == link { f(mark, w) }) }
/// The far uberbot shown here ("" when this hub has one of its own, or no linked machine does)
async fn far_uber(hub: &Arc<Hub>) -> String {
    let here = match of(hub) { Some(b) => b.world().await.uber_bot(), None => String::new() };
    if !here.is_empty() { return String::new() }
    let mut far = vec![]; each_far(hub, |_, _, w| far.push(mirrored_uber(w)));
    uber_shown("", &far)
}

/// Delete bot's confirm, the same on every client (each shows it as it is, translated): the bot by name and exactly what `bots.delete`
/// removes with it (rpc.rs `delete`: its threads, its routines, its home folder)
pub fn delete_ask(name: &str) -> Value { json!({"title":format!("Delete {name}?"),"body":format!("This removes {name}'s threads, its routines and its home folder."),"yes":"Delete bot"}) }

/// The phone data API (data/1 `bots.list`): this machine's live bots and rooms. A bot's conversation is its own thread (open it like any thread).
pub async fn directory(hub: &Arc<Hub>) -> serde_json::Value {
    let (mut bots, mut rooms, mut here_rooms, mut uber) = (vec![], vec![], std::collections::HashSet::new(), String::new());
    if let Some(b) = of(hub) { let w = b.world().await; uber = w.uber_bot(); let (bs, rs) = rows_of(&w, &uber, &Value::Null); bots = bs; rooms = rs; here_rooms = w.rooms.values().filter(|r| !r.deleted).map(|r| r.id.clone()).collect(); }
    let mut far = vec![]; each_far(hub, |_, _, w| far.push(mirrored_uber(w)));
    let uber = uber_shown(&uber, &far);
    each_far(hub, |_, mark, w| { let (bs, rs) = rows_of(w, &uber, mark); bots.extend(bs); rooms.extend(far_rooms(rs, &here_rooms)); });
    // a home move: the uberbot's row says where it is going (net/homemove.rs `moving_note`), as the desktop's sidebar does
    if let Some((_, note)) = crate::net::homemove::moving_note(&hub.setting("home.moving").await.unwrap_or_default(), crate::hub::now_ms() / 1000) { if let Some(b) = bots.iter_mut().find(|b| b["uber"] == true && b["mark"].is_null()) { b["note"] = serde_json::json!(note) } }
    serde_json::json!({"bots":bots,"rooms":rooms})
}
/// A linked machine's rooms as listed here: a room shared across machines is kept on each under the same id, and this machine's copy is the one listed
pub fn far_rooms(rows: Vec<Value>, here: &std::collections::HashSet<String>) -> Vec<Value> { rows.into_iter().filter(|r| !here.contains(&crate::net::far::local(r["id"].as_str().unwrap_or("")))).collect() }
/// A bots world's live bots and rooms (oldest first) as `bots.list` rows; `uber` is the bot shown as the uberbot, `mark` the machine's marker (null for this one);
/// `delete` says the row offers Delete bot (`bot.delete`): this machine's bots but the uberbot, `ask` its confirm (`delete_ask`); on a room, Delete room (`room.delete`): this machine's rooms
pub fn rows_of(w: &super::world::World, uber: &str, mark: &Value) -> (Vec<Value>, Vec<Value>) {
    let mut bots: Vec<_> = w.bots.values().filter(|x| !x.deleted).map(|x| (x.created, x.id.clone(), json!({"id":x.id,"name":x.name,"thread":x.thread,"look":x.look,"color":x.color,"uber":!uber.is_empty() && x.id == uber,"mark":mark,"delete":mark.is_null() && x.id != uber}))).collect();
    for b in bots.iter_mut().filter(|b| b.2["delete"] == true) { let n = b.2["name"].as_str().unwrap_or("").to_string(); b.2["ask"] = delete_ask(&n); }
    bots.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut rooms: Vec<_> = w.rooms.values().filter(|x| x.listed() && !x.swarm() && !crate::net::far::local(&x.id).starts_with("dm:")).map(|x| (x.created, x.id.clone(), json!({"id":x.id,"name":x.name,"members":x.members,"mark":mark,"delete":mark.is_null()}))).collect();
    rooms.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    (bots.into_iter().map(|x| x.2).collect(), rooms.into_iter().map(|x| x.2).collect())
}
/// A post as a room's page shows it, with its Markdown blocks and code to copy (md/copy.rs: `copy`, [{text, cmd}]) when it has any.
fn post_json(p: &Value, from: &str, now: u64) -> Value {
    let txt = p["text"].as_str().unwrap_or("");
    let mut o = json!({"id":p["id"],"from":from,"text":p["text"],"ago":ago(now, p["at"].as_u64().unwrap_or(0)),"mine":from == "you","blocks":crate::present::thread::md_blocks(txt)});
    let c = crate::md::copy::spans(txt); if !c.is_empty() { o["copy"] = json!(c) }
    o
}
/// MBots.room.view for a room here (MBots.room.view.loc): `{kind:"room", id, name, title, members, posts:[{id, from, text, ago, mine}], ephemeral, closed, keep}`
/// (`keep`: the page offers Keep this room, `bots.room_keep`), the newest 200 posts oldest first. None for an unknown room.
pub async fn room_page(hub: &Arc<Hub>, room: &str, now: u64) -> Option<Value> {
    if room.starts_with("dm:") { return None }
    if room.contains('~') { // a linked machine's room, from its mirror; a post goes there (net/rpc.rs holds it while that machine is away)
        let mut head = None; on_far(hub, &crate::net::far::link(room), |mark, w| head = w.room(room).map(|r| (r.name.clone(), r.members.iter().map(|m| w.bot(m).map_or(m.clone(), |b| b.name.clone())).collect::<Vec<_>>(), mark.clone(), (r.ephemeral, r.closed))));
        let (name, members, mark, life) = head?;
        let posts: Vec<Value> = room_posts(hub, room, 200).await?.into_iter().map(|p| { let from = p["from"].as_str().unwrap_or("").to_string();
            post_json(&p, &from, now) }).collect();
        return Some(json!({"kind":"room","id":room,"name":name,"title":name,"members":members.join(", "),"posts":posts,"mark":mark,"ephemeral":life.0,"closed":life.1,"keep":false}))
    }
    let b = of(hub)?;
    let (name, members, life, swarm) = { let w = b.world().await; let r = w.room(room).filter(|r| !r.deleted)?; (r.name.clone(), super::swarm::names(&w, r).into_iter().map(|x| x.1).collect::<Vec<_>>(), (r.ephemeral, r.closed), r.swarm()) };
    let posts: Vec<Value> = if swarm { vec![] } else {
        room_posts(hub, room, 200).await?.into_iter().map(|p| { let from = p["from"].as_str().unwrap_or("").to_string();
            post_json(&p, &from, now) }).collect()
    };
    // Life.keep: a bots' exchange opened by its id offers Keep (a linked machine's is kept there)
    let mut pg = json!({"kind":"room","id":room,"name":name,"title":name,"members":members.join(", "),"posts":posts,"ephemeral":life.0,"closed":life.1,"keep":life.0});
    // a swarm's room (docs/SWARM.md): its board as the swarm page draws it (members with live state and colour, posts with tags, claims) and the controls it offers
    if swarm { if let Some(mut sw) = b.board(room, "", 200, false).await {
        let st = sw["room"]["state"].as_str().unwrap_or("").to_string();
        let acts: Vec<Value> = match st.as_str() { "active" => vec![json!({"label":"Pause","action":"swarm-act","value":"pause"}), json!({"label":"Stop","action":"swarm-act","value":"stop"})],
            "paused" => vec![json!({"label":"Resume","action":"swarm-act","value":"resume"}), json!({"label":"Stop","action":"swarm-act","value":"stop"})], _ => vec![] };
        sw["actions"] = json!(acts); sw["ended"] = json!(st == "done" || st == "stopped");
        let n_posts = sw["posts"].as_array().map_or(0, |a| a.len());
        for (i, p) in sw["posts"].as_array_mut().into_iter().flatten().enumerate() {
            let at = p["at"].as_u64().unwrap_or(0);
            let guard = p["guard"] == true;
            if let Some(m) = p.as_object_mut() { m.remove("images"); }
            p["ago"] = json!(ago(now, at));
            p["mine"] = json!(p["from"] == "you" && !guard);
            if !guard {
                let text = p["text"].as_str().unwrap_or("").to_string();
                let id = p["id"].as_str().unwrap_or("").to_string();
                if !p["blocks"].is_array() { p["blocks"] = json!(crate::present::thread::md_blocks(&text)); }
                if i + 30 >= n_posts && crate::files::mimg::may(&text) {
                    let imgs = crate::present::thread::images(&id, &text);
                    if !imgs.is_empty() { p["images"] = json!(imgs); }
                }
                let lf = crate::files::share::linkable(&text);
                if !lf.is_empty() { p["linkFiles"] = json!(lf); }
            }
        }
        pg["swarm"] = sw; } }
    Some(pg)
}
/// a room's life (ephemeral, closed), here or in a linked machine's mirror; None for an unknown room
/// Whether `room.delete` may delete it: a room of this machine the lists show (no bots' exchange, swarm or direct line)
pub async fn room_deletable(hub: &Arc<Hub>, room: &str) -> bool {
    if room.is_empty() || room.contains('~') || room.starts_with("dm:") { return false }
    let Some(b) = of(hub) else { return false }; let w = b.world().await; w.room(room).map_or(false, |r| r.listed() && !r.swarm())
}
pub async fn room_life(hub: &Arc<Hub>, room: &str) -> Option<(bool, bool)> {
    if room.contains('~') { let mut life = None; on_far(hub, &crate::net::far::link(room), |_, w| life = w.room(room).map(|r| (r.ephemeral, r.closed))); return life }
    let b = of(hub)?; let w = b.world().await; w.room(room).map(|r| (r.ephemeral, r.closed))
}
/// `room.posts`: the newest `limit` posts of a room, oldest first (`{id, from, text, at}`).
pub async fn room_posts(hub: &Arc<Hub>, room: &str, limit: usize) -> Option<Vec<serde_json::Value>> {
    if room.contains('~') {
        let link = crate::net::far::link(room);
        let mut lines = None; on_far(hub, &link, |_, w| lines = w.room(room).map(|_| w.room_lines.get(room).cloned().unwrap_or_default()));
        let lines = lines?; let fv = hub.far()?;
        return Some(lines.iter().rev().take(limit).rev().filter_map(|n| fv.event(&link, *n)).filter_map(|raw| serde_json::from_str::<Value>(&raw).ok())
            .map(|e| json!({"id":e["id"],"from":e["from"],"text":e["text"],"at":e["at"]})).collect())
    }
    let b = of(hub)?; let w = b.world().await;
    w.room(room)?;
    let lines = w.room_lines.get(room).cloned().unwrap_or_default();
    let take: Vec<usize> = lines.iter().rev().take(limit).rev().copied().collect();
    drop(w);
    let mut out = vec![];
    for l in take { if let Ok(raw) = hub.store.get(l - 1) { if let Ok(e) = serde_json::from_str::<serde_json::Value>(&raw) { out.push(serde_json::json!({"id":e["id"],"from":e["from"],"text":e["text"],"at":e["at"]})) } } }
    Some(out)
}

#[cfg(test)]
mod far_tests {
    use super::*;
    use super::super::world::World;
    fn world(lines: &[Value]) -> World { let mut w = World::default(); for (i, l) in lines.iter().enumerate() { w.apply_numbered(i + 1, &l.to_string()) } w }
    fn boat() -> World { world(&[json!({"$":"BotSet","id":"L~bub","name":"Bub","thread":"L~tb","look":7,"persona":"kind","at":1}),
        json!({"$":"BotSet","id":"L~cal","name":"Calendar","thread":"L~tc","look":3,"persona":"","at":2}), json!({"$":"UberSet","bot":"L~bub","uid":"u","at":3}),
        json!({"$":"MemorySaved","bot":"L~bub","key":"tea","kind":"fact","text":"likes green tea","tags":"","at":4}),
        json!({"$":"RoutineSet","id":"L~o1","bot":"L~bub","name":"Morning","cron":"daily 09:00","prompt":"plan the day","on":true,"at":5}),
        json!({"$":"SpaceSet","bot":"L~bub","spec":"{\"title\":\"Desk\",\"blocks\":[]}","at":6}),
        json!({"$":"RoomSet","id":"L~r9","name":"Ring","members":"L~bub","at":7}), json!({"$":"RoomSet","id":"L~r2","name":"Shared","members":"L~bub","at":7}),
        json!({"$":"RoomPosted","id":"L~p1","room":"L~r9","from":"you","text":"ding","hop":0,"at":8}), json!({"$":"RoomSet","id":"L~dm:a:b","name":"x","members":"","at":8})]) }
    #[test] fn the_uberbot_shown_is_this_hubs_else_the_first_linked_machines() {
        assert_eq!(mirrored_uber(&boat()), "L~bub");
        assert_eq!(uber_shown("b1", &["L~bub".into()]), "b1");
        assert_eq!(uber_shown("", &["".into(), "M~x".into(), "L~bub".into()]), "M~x");
        assert_eq!(uber_shown("", &[]), "");
        let gone = world(&[json!({"$":"BotSet","id":"L~bub","name":"Bub","thread":"L~tb","look":7,"persona":"","at":1}), json!({"$":"UberSet","bot":"L~bub","uid":"u","at":2}), json!({"$":"BotDeleted","id":"L~bub","at":3})]);
        assert_eq!(mirrored_uber(&gone), "", "a deleted uberbot is none");
    }
    #[test] fn a_linked_machines_rows_carry_its_mark_and_skip_a_room_kept_here() {
        let mark = crate::mirror::mark_of("boat-hub", false);
        let (bots, rooms) = rows_of(&boat(), "L~bub", &mark);
        assert_eq!(bots.iter().map(|b| (b["id"].as_str().unwrap(), b["uber"] == true, b["thread"].as_str().unwrap())).collect::<Vec<_>>(), [("L~bub", true, "L~tb"), ("L~cal", false, "L~tc")]);
        assert_eq!(bots[0]["mark"]["short"], "boat-hub");
        assert!(bots.iter().all(|b| b["delete"] == false), "a linked machine's bots are deleted there");
        let (own, _) = rows_of(&boat(), "L~bub", &Value::Null);
        assert_eq!(own.iter().map(|b| b["delete"] == true).collect::<Vec<_>>(), [false, true], "this machine's bots offer Delete bot, the uberbot never");
        let here: std::collections::HashSet<String> = ["r2".to_string()].into();
        let rooms = far_rooms(rooms, &here);
        assert_eq!(rooms.iter().map(|r| r["id"].as_str().unwrap()).collect::<Vec<_>>(), ["L~r9"], "the shared room is this hub's copy; a direct room is never listed");
        assert_eq!(rooms[0]["mark"]["name"], "boat-hub");
    }
    #[test] fn a_linked_machines_bot_page_is_read_only_from_its_mirror() {
        let (w, mark) = (boat(), crate::mirror::mark_of("boat-hub", false));
        let p = far_page(&w, "L~bub", "memory", 100, 0, true, &mark).unwrap();
        assert_eq!((p["edit"].as_bool(), p["uber"].as_bool(), p["chat"].as_str(), p["mark"]["short"].as_str()), (Some(false), Some(true), Some("L~tb"), Some("boat-hub")));
        assert_eq!(p["memory"][0]["text"], "likes green tea");
        assert_eq!(p["tabs"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect::<Vec<_>>(), ["chat", "space", "memory", "routines", "settings"]);
        let r = far_page(&w, "L~bub", "routines", 100, 0, true, &mark).unwrap(); assert_eq!(r["routines"][0]["name"], "Morning");
        let sp = far_page(&w, "L~bub", "space", 100, 0, true, &mark).unwrap(); assert_eq!(sp["space"]["title"], "Desk");
        let st = far_page(&w, "L~bub", "settings", 100, 0, true, &mark).unwrap(); assert_eq!(st["persona"], "kind");
        assert_eq!(far_page(&w, "L~bub", "hooks", 100, 0, true, &mark).unwrap()["tab"], "chat", "a tab the mirror does not hold opens Chat");
        assert!(far_page(&w, "L~nobody", "chat", 100, 0, false, &mark).is_none());
        assert_eq!(w.room_lines["L~r9"], vec![9], "a post is found again by its place in the mirror");
    }
    #[test] fn post_json_includes_markdown_blocks_and_preserves_copy() {
        let p = json!({"id": "p1", "text": "Use `EPC2306` and:\n```bash\ncargo test\n```", "at": 1000});
        let o = post_json(&p, "Bub", 2000);
        assert!(o["copy"].is_array());
        let b = o["blocks"].to_string();
        assert!(b.contains(r#""tag":"code""#) && b.contains("EPC2306") && b.contains(r#""tag":"pre""#) && b.contains("cargo test"), "{b}");
    }
}

#[cfg(test)]
mod routine_row_tests {
    use super::*;
    #[test] fn a_mailroom_row_names_bots_and_threads() {
        let mut w = super::super::world::World::default();
        w.apply_line(r#"{"$":"BotSet","id":"b1","thread":"t1","name":"Scout","color":"","at":1}"#);
        let title = |th: &str| (th == "t2").then(|| "Fix the regulator".to_string());
        assert_eq!(mail_who(&w, "bot:b1", &title), "Scout"); assert_eq!(mail_who(&w, "thread:t2", &title), "Fix the regulator");
        assert_eq!(mail_who(&w, "bot:gone", &title), "bot:gone", "unknown: as it is");
        assert_eq!(mail_head(&w, "Bot report from backplane://thread/t1:", &title), "Bot report from Scout:");
        assert_eq!(mail_head(&w, "see backplane://thread/t2 and backplane://thread/t9.", &title), "see Fix the regulator and backplane://thread/t9.");
    }
    #[test] fn a_routine_row_says_when_it_ran_and_whether_it_runs_now() {
        assert_eq!(ran_text(1000, 0), "", "never ran: nothing");
        assert_eq!(ran_text(1000, 990), "ran just now"); assert_eq!(ran_text(4000, 400), "ran 1h ago");
        assert!(routine_running(true, 990, 1000), "fired 10 s ago and its bot's turn runs");
        assert!(!routine_running(false, 990, 1000), "the turn ended");
        assert!(!routine_running(true, 0, 1000), "never fired: the turn is the person's");
        assert!(!routine_running(true, 10, 10 + ROUTINE_RUN_WINDOW + 1), "fired long ago: the turn running now is another");
    }
}

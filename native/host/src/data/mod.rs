//! The phone data API (docs/PHONE-DATA.md, proto "data/1"): plain data over one WebSocket, no presenter and no view schema.
//! `/ws?data=1[&enc=json]&token=…` (the same auth and pairing as every socket; `enc=json` is text frames, otherwise binary CBOR with plain string keys).
//! Reads come from the model (`Model::page`, thread and project metadata) and the policy state; writes are the hub's own RPCs (`project.add`, `thread.create`,
//! `turn.start`, `ask.answer`, `turn.interrupt`) run as this connection; subscriptions turn each new log line, and the streamed text of a running turn, into
//! pushes (`thread`, `project`, `entry`, `text`, `turn`) for the lists and threads a connection asked for.
pub(crate) mod ext;
mod link;
pub mod term;
pub mod machines;
/// What a phone hears of a push on its socket (ext.rs `notify_of`).
pub fn ext_notify(e: &Value) -> Value { ext::notify_of(e) }
use crate::hub::{Conn, Hub, Out};
use crate::wire;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet, VecDeque}, sync::{Arc, Mutex}, time::Duration};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::{protocol::WebSocketConfig, Message};

pub const PROTO: &str = "data/1";
/// What this hub does beyond the base of data/1; a hub from before an addition does not list it (the app can warn instead of sending what would be dropped).
pub const FEATURES: &[&str] = &["images", "actions", "search", "models", "mentions", "bots", "notify", "viewers", "files", "browser", "diff", "review", "fold", "picker", "voice", "order", "layers", "onboard", "push", "settled", "commands", "work", "star", "accounts", "machines", "history", "terms", "links", "devices", "queue", "buy", "status", "subview", "find", "notifysettings", "ring", "hubOffer", "swarm", "sections", "telemetryNotice", "mcpsettings"];
const PREVIEW: usize = 140;
const ASK_SCAN: usize = 4000;

fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64) }
/// Event times are seconds (older logs) or milliseconds.
fn ms(at: u64) -> u64 { if at < 100_000_000_000 { at * 1000 } else { at } }
fn clip(s: &str, n: usize) -> String { let t: String = s.chars().take(n).collect(); t.replace('\n', " ") }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }
fn err<T: ToString>(e: T) -> Value { json!(e.to_string()) }

// ---- plain CBOR (no key dictionary: the app's decoder knows nothing of the hub's) ----
fn head(out: &mut Vec<u8>, major: u8, n: u64) {
    let m = major << 5;
    match n { 0..=23 => out.push(m | n as u8), 24..=255 => { out.push(m | 24); out.push(n as u8) } 256..=65535 => { out.push(m | 25); out.extend((n as u16).to_be_bytes()) }
        65536..=4294967295 => { out.push(m | 26); out.extend((n as u32).to_be_bytes()) } _ => { out.push(m | 27); out.extend(n.to_be_bytes()) } }
}
pub fn cbor_plain(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Null => out.push(0xf6), Value::Bool(b) => out.push(if *b { 0xf5 } else { 0xf4 }),
        Value::Number(n) => { if let Some(u) = n.as_u64() { head(out, 0, u) } else if let Some(i) = n.as_i64() { head(out, 1, (-1 - i) as u64) } else { out.push(0xfb); out.extend(n.as_f64().unwrap_or(0.0).to_be_bytes()) } }
        Value::String(t) => { head(out, 3, t.len() as u64); out.extend(t.as_bytes()) }
        Value::Array(xs) => { head(out, 4, xs.len() as u64); for x in xs { cbor_plain(x, out) } }
        Value::Object(o) => { head(out, 5, o.len() as u64); for (k, x) in o { head(out, 3, k.len() as u64); out.extend(k.as_bytes()); cbor_plain(x, out) } }
    }
}
fn pack(v: &Value, json_mode: bool) -> Message { if json_mode { Message::Text(v.to_string().into()) } else { let mut o = Vec::new(); cbor_plain(v, &mut o); Message::Binary(o.into()) } }

// ---- reading ----
/// Open asks per thread: one pass over the log's tail (an ask is open from its AskOpened until its AskClosed).
/// Every ask still open, for the badge (push).
pub fn open_ask_total(hub: &Hub) -> u64 { open_ask_counts(hub).values().sum() }
/// The thread of an ask still open.
pub fn ask_thread(hub: &Hub, ask: &str) -> Option<String> {
    let n = hub.store.len(); let lines = hub.store.range(n.saturating_sub(ASK_SCAN), n).ok()?; let mut found = None;
    for l in lines { if !l.contains("\"Ask") || !l.contains(ask) { continue } let Ok(e) = serde_json::from_str::<Value>(&l) else { continue };
        if s(&e, "id") != ask { continue } match s(&e, "$") { "AskOpened" => found = Some(s(&e, "thread").to_string()), "AskClosed" => found = None, _ => {} } }
    found
}
fn open_ask_counts(hub: &Hub) -> HashMap<String, u64> {
    let n = hub.store.len(); let mut m = HashMap::new();
    let Ok(lines) = hub.store.range(n.saturating_sub(ASK_SCAN), n) else { return m };
    let (mut opened, mut closed): (Vec<(String, String)>, HashSet<String>) = (vec![], HashSet::new());
    for l in lines { if !l.contains("\"Ask") { continue } let Ok(e) = serde_json::from_str::<Value>(&l) else { continue };
        match s(&e, "$") { "AskOpened" => opened.push((s(&e, "id").into(), s(&e, "thread").into())), "AskClosed" => { closed.insert(s(&e, "id").into()); } _ => {} } }
    for (id, th) in opened { if !closed.contains(&id) { *m.entry(th).or_insert(0) += 1 } }
    m
}
/// (pinned, on the Settled shelf, when it was settled in unix ms): the desktop's own shelf (shelf.rs: a manual settle or an unsettle wins over the clock, a snooze is not a settle).
fn flags(hub: &Hub, id: &str) -> (bool, bool, u64) {
    let _ = hub;
    crate::statehost::policies().and_then(|p| p.with_state(|st| st.threads.get(id).map(shelf_flags))).flatten().unwrap_or((false, false, 0))
}
pub(crate) fn shelf_flags(t: &crate::state::Thread) -> (bool, bool, u64) {
    let settled = crate::shelf::shelf_of(t) == crate::shelf::Shelf::Settled;
    (t.pinned, settled, if settled { ms(t.settled_at) } else { 0 })
}
fn flags_of(id: &str) -> (bool, bool, u64) { crate::statehost::policies().and_then(|p| p.with_state(|st| st.threads.get(id).map(shelf_flags))).flatten().unwrap_or((false, false, 0)) }
/// A setting of the policy state ("" when unset or the state is not up).
fn setting_of(k: &str) -> String { crate::statehost::policies().and_then(|p| p.with_state(|st| st.settings.get(k).cloned())).flatten().unwrap_or_default() }
/// Why `thread.create` refuses the Scratch project: its chats are kept (Conversation history) and open as they were, but no new one starts there.
pub(crate) const SCRATCH_CLOSED: &str = "Conversation history takes no new chats. Start one in a project.";
fn thread_json(hub: &Hub, m: &crate::model::Model, id: &str, asks: &HashMap<String, u64>) -> Option<Value> {
    let t = m.threads.get(id).filter(|t| !t.deleted)?;
    let waiting = asks.get(id).copied().unwrap_or(0);
    let state = if waiting > 0 { "asking" } else { match t.turn.as_str() { "running" => "working", "failed" => "failed", _ => "idle" } };
    // what was last said, not the last tool call or a design step's record; a thread with only activity shows nothing
    let preview = m.page(&hub.store, id, 0, 8).ok().and_then(|p| p.rows.iter().find(|r| r.kind == "msg" && !r.text.trim().is_empty()).map(|r| clip(&shown_text(&r.text), PREVIEW))).unwrap_or_default();
    let (pinned, settled, settled_at) = flags(hub, id);
    // a model chosen within the provider is a ModesSet (the policy state), a switch of provider a ProviderSet (the model's fold)
    // `status`: the sidebar dot's word (signal.rs, children's work included): "finished" is the desktop's green dot, a turn done and not yet settled
    let (model, effort, runtime, combo, status, pin_order, swarm) = crate::statehost::policies().and_then(|p| p.with_state(|st| (st.threads.get(id).map_or(String::new(), |t| t.model.clone()), st.settings.get(&format!("effort.{id}")).cloned().unwrap_or_default(), st.runtime.get(id).cloned().unwrap_or_default(),
        crate::present::keys::combo_of(st, id, crate::hub::now_ms() / 1000), crate::signal::index(st).get(id).map_or("", |n| crate::signal::display_name(st, *n, id)), crate::sidebar::pin_place(st, id), st.swarm_of.get(id).cloned().unwrap_or_default()))).unwrap_or_default();
    let model = if model.is_empty() { t.model.clone() } else { model };
    // the composer's one chip: the model and what the account has left (present/keys.rs `combo`, its short form: a phone's row has room for about 20 characters;
    // the picker's title says it all)
    let chip = combo["short"].as_str().filter(|c| !c.is_empty()).map(str::to_string).unwrap_or_else(|| ext::chip(&t.provider, &model, &effort));
    Some(json!({"id":id,"project":t.project,"title":t.title,"state":state,"updatedAt":ms(t.activity.max(t.created)),"pinned":pinned,"pinOrder":pin_order,"settled":settled,"settledAt":settled_at,"archived":t.archived,
        "provider":t.provider,"model":model,"effort":effort,"runtime":runtime,"chip":chip,"preview":preview,"waiting":waiting,"status":status,"parent":setting_of(&format!("thread.parent.{id}")),"history":t.project == crate::sidebar::SCRATCH,"swarm":swarm}))
}
fn project_json(m: &crate::model::Model, id: &str, asks: &HashMap<String, u64>) -> Option<Value> {
    let settled = m.threads.iter().filter(|(i, t)| t.project == id && !t.deleted && !t.archived && flags_of(i).1).count();
    if id == crate::sidebar::SCRATCH { return None } // Conversation history (`history.list`), not a project to start a thread in
    let p = m.projects.get(id).filter(|p| !p.removed)?;
    let ts: Vec<_> = m.threads.iter().filter(|(_, t)| t.project == id && !t.deleted).collect();
    let count = ts.iter().filter(|(_, t)| !t.archived).count();
    let updated = ts.iter().map(|(_, t)| t.activity.max(t.created)).max().unwrap_or(0);
    let waiting: u64 = ts.iter().map(|(i, _)| asks.get(*i).copied().unwrap_or(0)).sum();
    let pick = setting_of(&format!("project.pick.{id}"));
    let color = crate::present::badge::color_eff(&setting_of(&format!("project.color.{id}")), &pick, id).to_string();
    Some(json!({"id":id,"title":p.title,"root":p.root,"origin":crate::mirror::origin_of_root(&p.root),"threadCount":count,"settledCount":settled,"updatedAt":ms(updated),"waiting":waiting,
        "icon":crate::present::badge::icon_eff(&setting_of(&format!("project.icon.{id}")), &pick, &p.title),"css":crate::present::badge::css_of(&color),"color":color}))
}

/// The linked machines' projects and threads next to this hub's: `f(shown machines, unified marks and join map, the pick)` (docs/PHONE-DATA.md, Your machines).
fn with_far<R>(hub: &Hub, want: &str, f: impl FnOnce(&machines::Far, &crate::present::screen::Unified, &str) -> R) -> R {
    let m = hub.mirrors(); let here = m.here();
    m.shown(|far| {
        let pick = machines::pick_of(want, far);
        // the join stays whatever is picked: a machine picked filters the merged rows (machines::pick_rows), it never splits them
        let join = crate::statehost::policies().and_then(|p| p.with_state(|st| crate::present::screen::join_full(st, far, m))).unwrap_or(crate::present::merged::Join { far: HashMap::new(), rows: HashMap::new() });
        let marks: HashMap<String, Value> = far.iter().map(|(n, _)| (n.clone(), m.mark(n))).collect();
        f(far, &crate::present::screen::Unified { far: &join.far, marks: &marks, here: &here, rows: &join.rows }, &pick)
    })
}
/// The merged project rows (projects.list, and what is pushed when one moves): one row per project key over every machine (present/merged.rs):
/// this hub's copy, else a linked machine's, holds the others' counts; `key`, `machines` (chips), `default` (where new work runs) on each.
fn merged_rows(g: &crate::model::Model, asks: &HashMap<String, u64>, far: &machines::Far, un: &crate::present::screen::Unified) -> Vec<(u64, String, Value)> {
    let ps = machines::partners(far, "", un.far);
    let mut items: Vec<(u64, String, Value)> = g.projects.keys().filter_map(|i| project_json(g, i, asks).map(|mut v| { if let Some(x) = ps.get(i) { machines::join_project(&mut v, x, un) } (v["updatedAt"].as_u64().unwrap_or(0), i.clone(), v) })).collect();
    items.extend(machines::project_rows(far, "", un.far, un).into_iter().map(|(a, i, mut v)| { if let Some(x) = ps.get(&i) { machines::join_project(&mut v, x, un) } (a, i, v) }));
    items.extend(machines::off_rows(un));
    for it in items.iter_mut() { machines::merged_fields(&mut it.2, un); }
    items
}
/// One merged project row by its id ("" pick), for a push.
fn merged_row(hub: &Hub, g: &crate::model::Model, asks: &HashMap<String, u64>, id: &str) -> Option<Value> {
    with_far(hub, "", |far, un, _| merged_rows(g, asks, far, un).into_iter().find(|x| x.1 == id).map(|x| x.2))
}
/// A thread row of this hub or of a linked machine (`<link>~<id>`, built from its mirror: no log to read, so its preview is the mirror's last words).
fn thread_row(hub: &Hub, m: &crate::model::Model, id: &str, asks: &HashMap<String, u64>, join: &HashMap<String, String>) -> Option<Value> {
    if !id.contains('~') || m.threads.contains_key(id) { return thread_json(hub, m, id, asks) }
    let far = hub.far();
    let preview = far.as_ref().and_then(|f| f.page(id, 0, 8)).and_then(|pg| pg.rows.iter().filter(|r| r.thread == id).find(|r| r.kind == "msg" && !r.text.trim().is_empty()).map(|r| clip(&shown_text(&r.text), PREVIEW))).unwrap_or_default();
    hub.mirrors().shown(|fars| { let (mname, st) = machines::holder(fars, id)?; let t = st.threads.get(id)?;
        machines::thread_json(id, st, &machines::project_shown(&t.project, join), &hub.mirrors().mark(mname), &preview) })
}
/// A thread just made on a linked machine: from its mirror when that has it already, else what is known (the mirror catches up and the phone's list pushes it again).
fn far_made(hub: &Hub, m: &crate::model::Model, id: &str, project: &str, title: &str) -> Value {
    if let Some(t) = thread_row(hub, m, id, &HashMap::new(), &HashMap::new()) { return t }
    let mark = hub.mirrors().shown(|far| far.iter().find(|(_, st)| st.projects.contains_key(project)).map(|(n, _)| hub.mirrors().mark(n))).unwrap_or_else(|| json!(null));
    json!({"id":id,"project":project,"title":title,"state":"idle","updatedAt":now_ms(),"pinned":false,"settled":false,"settledAt":0,"archived":false,"provider":"","model":"","effort":"","runtime":"","chip":"","preview":"","waiting":0,"status":"","parent":"","history":false,"mark":mark})
}
/// A cursor is "<updatedAt>:<id>" over a list sorted newest first, ties by id.
fn page_of(mut items: Vec<(u64, String, Value)>, limit: usize, before: &str) -> (Vec<Value>, Value) {
    items.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    if let Some((a, i)) = before.split_once(':') { if let Ok(a) = a.parse::<u64>() { items.retain(|x| x.0 < a || (x.0 == a && x.1.as_str() > i)) } }
    let more = items.len() > limit; items.truncate(limit);
    let next = if more { items.last().map(|x| json!(format!("{}:{}", x.0, x.1))).unwrap_or(Value::Null) } else { Value::Null };
    (items.into_iter().map(|x| x.2).collect(), next)
}
fn limit_of(p: &Value, dflt: usize, max: usize) -> usize { p.get("limit").and_then(Value::as_u64).map_or(dflt, |n| (n as usize).clamp(1, max)) }

/// The buttons of an ask, in the desktop's words (present/thread.rs asks_json): the files' approval of a buy thread is Send or Not now, a settings proposal Approve or Reject.
fn ask_options(kind: &str, tool: &str, options: &str) -> Value {
    if kind == "approval" { return match tool {
        "order_quote" => json!([{"id":"accept","label":"Send"},{"id":"decline","label":"Not now"}]),
        "settings_proposal" => json!([{"id":"accept","label":"Approve"},{"id":"decline","label":"Reject"}]),
        _ => json!([{"id":"accept","label":"Allow"},{"id":"session","label":"Allow for this session"},{"id":"decline","label":"Decline"}]) } }
    let labels: Vec<String> = match serde_json::from_str::<Value>(options) {
        Ok(Value::Array(xs)) => xs.iter().filter_map(|x| x.as_str().map(str::to_string).or_else(|| x["label"].as_str().map(str::to_string))).collect(),
        _ => options.split(|c| c == '\n' || c == '|').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
    };
    Value::Array(labels.into_iter().map(|l| json!({"id":l,"label":l})).collect())
}
fn ask_entry(e: &Value, answered: Option<&str>) -> Value {
    let (kind, tool, detail) = (s(e, "kind"), s(e, "tool"), s(e, "detail"));
    let q = if kind == "approval" && tool == "order_quote" { format!("Send these files for a quote?\n{detail}") }
        else if kind == "approval" { if detail.is_empty() { tool.to_string() } else { format!("{tool}: {detail}") } } else { detail.to_string() };
    let mut ask = json!({"id":s(e,"id"),"state":if answered.is_some() {"answered"} else {"open"},"question":q,"options":ask_options(kind, tool, s(e, "options"))});
    if let Some(a) = answered { ask["answered"] = json!(a) }
    json!({"id":format!("ask:{}",s(e,"id")),"kind":"ask","text":q,"at":ms(e["at"].as_u64().unwrap_or(0)),"ask":ask})
}
/// The thread's asks still open, from the log's tail.
fn open_asks(hub: &Hub, thread: &str) -> Vec<Value> {
    let n = hub.store.len();
    let Ok(lines) = hub.store.range(n.saturating_sub(ASK_SCAN), n) else { return vec![] };
    let (mut opened, mut closed): (Vec<Value>, HashSet<String>) = (vec![], HashSet::new());
    for l in lines { if !l.contains("\"Ask") { continue } let Ok(e) = serde_json::from_str::<Value>(&l) else { continue };
        match s(&e, "$") { "AskOpened" if s(&e, "thread") == thread => opened.push(e), "AskClosed" => { closed.insert(s(&e, "id").to_string()); } _ => {} } }
    opened.into_iter().filter(|e| !closed.contains(s(e, "id"))).map(|e| ask_entry(&e, None)).collect()
}
/// The AskOpened line for `id`, from the log's tail (an AskClosed names only the ask).
fn ask_opened(hub: &Hub, id: &str) -> Option<Value> {
    let n = hub.store.len();
    let lines = hub.store.range(n.saturating_sub(ASK_SCAN), n).ok()?;
    lines.iter().rev().filter(|l| l.contains("\"AskOpened\"") && l.contains(id)).filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|e| s(e, "id") == id)
}
fn far_entries(f: &Arc<dyn crate::api::FarView>, th: &str, p: &Value, bot: bool) -> Result<Value, Value> {
    let before: u64 = s(p, "before").parse().unwrap_or(0);
    let page = f.page(th, before, limit_of(p, 40, 32)).ok_or_else(|| err("unknown thread"))?;
    let ids: HashSet<u64> = page.ids.iter().filter_map(|i| i.parse().ok()).collect();
    let mut rows: Vec<&crate::model::Row> = page.rows.iter().filter(|r| ids.contains(&r.ord) && r.thread == th).collect();
    rows.sort_by_key(|r| r.ord);
    let backend = if bot { let o = oldest_ord(&page.rows); backend_of(&page.rows, if o > 1 { f.page(th, o, 32).map(|p| p.rows).unwrap_or_default() } else { vec![] }) } else { HashSet::new() };
    let out: Vec<Value> = rows.iter().filter_map(|r| row_entry_hid(r, bot, &backend)).collect();
    let more = page.below > 0 || page.rows.iter().map(|r| r.ord).collect::<HashSet<_>>().len() > ids.len();
    let next = if more { rows.first().map(|r| json!(r.ord.to_string())).unwrap_or(Value::Null) } else { Value::Null };
    let title = rows.iter().find(|r| r.kind == "msg" && r.role == "user").map(|r| clip(&shown_text(&r.text), 60)).unwrap_or_default();
    let thread = json!({"id":th,"project":"","title":title,"state":"idle","updatedAt":rows.last().map_or(0, |r| ms(r.at)),"pinned":false,"settled":false,"settledAt":0,"archived":false,"provider":"","model":"","effort":"","runtime":"","chip":"","preview":"","waiting":0,"parent":"","history":false});
    Ok(json!({"thread":thread,"entries":out,"next":next}))
}
/// Bot.backend over a page: its rows oldest first, and `older` (the rows before it, any order) for the turn the first one was written in.
fn backend_of(rows: &[crate::model::Row], older: Vec<crate::model::Row>) -> HashSet<String> {
    let mut old = older; old.sort_by(|a, b| b.seq.cmp(&a.seq)); let before = crate::present::thread::last_input_backend(&old);
    let mut rs: Vec<&crate::model::Row> = rows.iter().collect(); rs.sort_by_key(|r| r.seq);
    crate::present::thread::backend_replies(rs.into_iter().map(|r| (r.id.as_str(), r.kind == "msg", r.role.as_str(), r.text.as_str())), before)
}
/// The backend replies on the newest page of a local thread (what its chat leaves out: `present::thread::backend_replies`)
pub async fn backend_now(hub: &Hub, th: &str) -> HashSet<String> {
    let m = hub.model().await; let Ok(page) = m.page(&hub.store, th, 0, 32) else { return HashSet::new() };
    let o = oldest_ord(&page.rows); backend_of(&page.rows, if o > 1 { m.page(&hub.store, th, o, 32).map(|p| p.rows).unwrap_or_default() } else { vec![] })
}
fn oldest_ord(rows: &[crate::model::Row]) -> u64 { rows.iter().map(|r| r.ord).min().unwrap_or(0) }
/// Whether `thread` is a bot's conversation (the uberbot's chat among them), from the hub's state; false before the state is up
pub fn bot_chat(thread: &str) -> bool { crate::statehost::policies().and_then(|p| p.with_state(|st| st.bot_threads_has(thread))).unwrap_or(false) }
/// a linked machine's thread is a bot's conversation there (its mirror's state says)
pub(crate) fn far_bot(hub: &Hub, th: &str) -> bool { hub.mirrors().shown(|far| machines::holder(far, th).map_or(false, |(_, st)| st.bot_threads_has(th))) }
/// `bot`: the thread is a bot's conversation (`present::thread::bot_private`)
fn row_entry(r: &crate::model::Row, bot: bool) -> Option<Value> { row_entry_hid(r, bot, &HashSet::new()) }
/// `backend`: the replies of turns that were not the person's (`present::thread::backend_replies`), agentOnly in a bot's conversation too
fn row_entry_hid(r: &crate::model::Row, bot: bool, backend: &HashSet<String>) -> Option<Value> {
    let at = ms(r.at);
    if r.kind == "msg" {
        let kind = match r.role.as_str() { "user" => "user", "assistant" => "assistant", _ => "note" };
        if kind == "assistant" && r.text.trim().is_empty() { return None }
        let mut en = json!({"id":r.id,"kind":kind,"text":if kind == "user" { shown_text(&r.text) } else { crate::settings::order::pay_shown(&r.text) },"at":at});
        // a reply (`<in_reply_to>` block, bots/reply.rs): its body as the text and the message it answers, as the desktop draws it (present/thread.rs)
        if kind == "user" { if let Some((to, quote, body)) = crate::bots::reply::of(&r.text) {
            en["text"] = json!(shown_text(&body)); en["replyTo"] = json!({"id": to, "quote": quote.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("")}); } }
        if crate::present::thread::hidden(bot, &r.id, &r.role, &r.text) || (bot && backend.contains(&r.id)) { en["agentOnly"] = json!(true) }
        return Some(en);
    }
    act_entry(&r.thread, &r.id, &r.role, &r.akind, &r.text, at)
}
/// An activity as the phones see it, in the desktop's words (present/thread.rs): a tool call by its shown name, a design step by what changed, anything else a note
/// with its name and tone. A phone folds a run of `tool` and `step` entries (docs/PHONE-DATA.md); the design as it was before an edit is no entry at all.
fn act_entry(thread: &str, id: &str, role: &str, akind: &str, text: &str, at: u64) -> Option<Value> {
    use crate::present::thread as pt;
    let e = pt::Ent { id: id.into(), msg: false, role: role.into(), akind: akind.into(), text: text.into(), at };
    Some(match role {
        // a `/give` handoff received: its card (docs/PHONE-DATA.md, "Handoffs")
        h if h == crate::present::thread::HANDOFF_TONE && crate::present::thread::handoff_card(text).is_some() => { let c = crate::present::thread::handoff_card(text).unwrap_or_default(); json!({"id":id,"kind":"handoff","text":c["summary"],"handoff":c,"at":at}) }
        "tool" => { let mut v = json!({"id":id,"kind":"tool","text":text,"at":at,"tool":{"name":pt::shown_name(akind),"summary":if text == akind { String::new() } else { clip(text, 200) },"state":"done"}});
            // an Agent or Task call names the subagent it started: `thread.sub {thread, key: sub}` opens its view
            let sub = crate::present::panels::sub_key(akind, id); if !sub.is_empty() { v["tool"]["sub"] = json!(sub) } v }
        // a chat terminal: a card the phone draws and streams (data/term.rs)
        r if r == crate::term::emb::TONE && akind == crate::term::emb::KIND => term::entry(thread, id, text, at),
        "step" if pt::step_base(&e) => return None,
        "step" => json!({"id":id,"kind":"step","text":text,"label":pt::step_row(&e),"at":at}),
        _ => { let a = pt::plain_act(&e); json!({"id":id,"kind":"note","text":a["text"],"label":a["label"],"tone":a["tone"],"at":at}) }
    })
}
/// The paths of attached images are told to the agent in the message's text (`[attachment: <path>]` lines, the ones agents/inputs.rs reads as image
/// blocks; messages from before said `[Attached image: <path>]`, which the agent got as words only); the phone shows the pictures instead.
fn is_attachment_line(l: &str) -> bool { (l.starts_with("[attachment: ") || l.starts_with("[Attached image: ")) && l.ends_with(']') }
fn shown_text(t: &str) -> String { if !t.contains("[attachment: ") && !t.contains("[Attached image: ") { return t.to_string() } t.lines().filter(|l| !is_attachment_line(l)).collect::<Vec<_>>().join("\n").trim_end().to_string() }
fn live_entry(thread: &str, text: &str) -> Value { json!({"id":format!("live:{thread}"),"kind":"assistant","text":text,"at":now_ms(),"live":true}) }

async fn entries(hub: &Hub, p: &Value) -> Result<Value, Value> {
    let th = s(p, "thread");
    // a thread of a linked machine, as this hub mirrors it: its page from the mirror (no asks or live text here)
    if th.contains('~') && !hub.model().await.threads.contains_key(th) { if let Some(f) = hub.far().filter(|f| f.has(th)) {
        let mut r = far_entries(&f, th, p, far_bot(hub, th))?; let g = hub.model().await; let join = with_far(hub, "", |_, un, _| un.far.clone());
        if let Some(t) = thread_row(hub, &g, th, &HashMap::new(), &join) { r["thread"] = t } // (the mirror knows its title, project and machine)
        return Ok(r) } }
    let m = hub.model().await;
    let asks = open_ask_counts(hub);
    let thread = thread_json(hub, &m, th, &asks).ok_or_else(|| err("unknown thread"))?;
    let before: u64 = s(p, "before").parse().unwrap_or(0);
    let page = m.page(&hub.store, th, before, limit_of(p, 40, 32)).map_err(err)?;
    let ids: HashSet<u64> = page.ids.iter().filter_map(|i| i.parse().ok()).collect();
    let mut rows: Vec<&crate::model::Row> = page.rows.iter().filter(|r| ids.contains(&r.ord) && r.thread == th).collect();
    rows.sort_by_key(|r| r.ord);
    let bot = bot_chat(th);
    let backend = if bot { let o = oldest_ord(&page.rows); backend_of(&page.rows, if o > 1 { m.page(&hub.store, th, o, 32).map(|p| p.rows).unwrap_or_default() } else { vec![] }) } else { HashSet::new() };
    let mut out: Vec<Value> = rows.iter().filter_map(|r| { let mut e = row_entry_hid(r, bot, &backend)?;
        // a user message's images are on its log line
        if r.kind == "msg" && r.role == "user" { if let Some(a) = attachments_of(hub, r.seq as usize - 1, &r.id) { e["attachments"] = a } }
        Some(e) }).collect();
    // older entries exist when the page read more ordinals than it returns (its lookahead) or has rows below it
    let more = page.below > 0 || page.rows.iter().map(|r| r.ord).collect::<HashSet<_>>().len() > ids.len();
    let next = if more { rows.first().map(|r| json!(r.ord.to_string())).unwrap_or(Value::Null) } else { Value::Null };
    drop(m);
    if before == 0 {
        out.extend(open_asks(hub, th));
        if let Some((_, text)) = hub.live_snapshot().into_iter().find(|(t, _)| t == th) { out.push(live_entry(th, &text)) }
    }
    Ok(json!({"thread":thread,"entries":out,"next":next}))
}

/// A user message's images: on its own line (the built-in turn.start) or on the MessageAttached line the agents' turn.start commits a few lines after it.
fn attachments_of(hub: &Hub, line: usize, msg: &str) -> Option<Value> {
    let own = hub.store.get(line).ok()?;
    if own.contains("\"attachments\"") { return serde_json::from_str::<Value>(&own).ok().and_then(|v| v.get("attachments").cloned()) }
    let hi = (line + 14).min(hub.store.len());
    for l in hub.store.range(line + 1, hi).ok()? { if l.contains("\"MessageAttached\"") && l.contains(msg) { if let Ok(v) = serde_json::from_str::<Value>(&l) { if s(&v, "msg") == msg { return v.get("attachments").cloned() } } } }
    None
}

fn dirs(p: &Value) -> Result<Value, Value> {
    let home = crate::plat::dirs::home().unwrap_or_else(|| if cfg!(windows) { "C:\\".into() } else { "/".into() });
    let want = s(p, "path"); let path = crate::plat::fs::canonicalize(if want.is_empty() { home.as_str() } else { want }).map_err(|_| err("no such folder"))?;
    let mut names: Vec<String> = std::fs::read_dir(&path).map_err(|_| err("cannot read that folder"))?.flatten()
        .filter(|e| e.file_type().map_or(false, |t| t.is_dir()) || e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| !n.starts_with('.')).collect();
    names.sort_by_key(|n| n.to_lowercase()); names.truncate(1000);
    Ok(json!({"path":path.to_string_lossy(),"parent":path.parent().map(|x| x.to_string_lossy().into_owned()),"dirs":names}))
}

// ---- writes: the hub's own RPCs, run as this connection ----
async fn rpc(hub: &Arc<Hub>, c: &mut Conn, m: &str, p: Value) -> Result<Value, Value> {
    // a thing on a linked machine (`<link>~<id>`, or `on`) goes there and its owner's answer comes back (net/rpc.rs `Router::ask`)
    let r = match hub.far_ask(c.client, m, &p).await {
        Some(r) => r,
        None => hub.request_here(c, &json!({"id":0,"m":m,"p":p})).await.into_iter().find(|v| v["t"] == "reply").ok_or_else(|| err("no answer"))?,
    };
    if r["ok"] == true { Ok(r) } else { Err(r["error"].clone().as_str().map(|e| json!(e)).unwrap_or(r["error"].clone())) }
}
fn safe_id(x: &str) -> bool { !x.is_empty() && x.len() <= 64 && x.chars().all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c)) }
static SENT: Mutex<Option<(HashMap<String, Value>, VecDeque<String>)>> = Mutex::new(None);
fn remember(id: &str, entry: &Value) { let mut g = SENT.lock().unwrap(); let (m, q) = g.get_or_insert_with(Default::default); if m.insert(id.into(), entry.clone()).is_none() { q.push_back(id.into()); if q.len() > 2000 { if let Some(o) = q.pop_front() { m.remove(&o); } } } }
fn recalled(id: &str) -> Option<Value> { SENT.lock().unwrap().as_ref().and_then(|(m, _)| m.get(id).cloned()) }

const MAX_ATTACH: usize = 4;
const MAX_ATTACH_BYTES: usize = 8 << 20;
fn mime_ok(m: &str) -> bool { matches!(m, "image/jpeg" | "image/png" | "image/webp" | "image/gif" | "image/heic" | "image/heif") }
fn mime_of(name: &str) -> &'static str { match name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() { "jpg" | "jpeg" => "image/jpeg", "png" => "image/png", "webp" => "image/webp", "gif" => "image/gif", "heic" => "image/heic", "heif" => "image/heif", _ => "application/octet-stream" } }
/// Stores a message's images under `<home>/attachments/<thread>/` (safe names, 0600) and returns their metadata and paths.
fn store_attachments(hub: &Hub, thread: &str, list: &Value) -> Result<(Vec<Value>, Vec<String>), Value> {
    use base64::Engine; use crate::plat::fs::{DirBuilderExt, OpenOptionsExt}; use std::io::Write;
    let Some(xs) = list.as_array() else { return Ok((vec![], vec![])) };
    if xs.len() > MAX_ATTACH { return Err(err(format!("at most {MAX_ATTACH} images in a message"))) }
    let dir = crate::files::attach::dir(&hub.cfg.home, thread);
    let (mut meta, mut paths) = (vec![], vec![]);
    for x in xs {
        let (name, mime) = (s(x, "name"), s(x, "mime"));
        if !mime_ok(mime) { return Err(err("images only: jpeg, png, webp, gif, heic")) }
        let bytes = base64::engine::general_purpose::STANDARD.decode(s(x, "data")).map_err(|_| err("an image is not valid base64"))?;
        if bytes.is_empty() || bytes.len() > MAX_ATTACH_BYTES { return Err(err("an image must be 1 byte to 8 MB")) }
        let key = hex::encode(&crate::net::pairing::urandom(8));
        let stored = crate::files::attach::stored(&key, if name.is_empty() { "image" } else { name });
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir).map_err(|_| err("cannot store the image"))?;
        let path = dir.join(&stored);
        std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).and_then(|mut f| f.write_all(&bytes)).map_err(|_| err("cannot store the image"))?;
        meta.push(json!({"id":format!("{}~{}",crate::files::attach::safe(thread),stored),"name":name,"mime":mime,"bytes":bytes.len()}));
        paths.push(path.to_string_lossy().into_owned());
    }
    Ok((meta, paths))
}
async fn send_message(hub: &Arc<Hub>, c: &mut Conn, thread: &str, text: &str, client_id: &str, attachments: &Value) -> Result<Value, Value> {
    let has = attachments.as_array().map_or(false, |a| !a.is_empty());
    if text.trim().is_empty() && !has { return Err(err("empty message")) }
    // a command of the composer is never a message: an app that does not know `composer.run` would post a side question to the thread, or a secret to the log
    if crate::session::composer::word_of(text).is_some() { return Err(err("This app is too old for /btw, /give, /secret, /intent and /settle. Update it. Nothing was sent.")) }
    if !client_id.is_empty() { if !safe_id(client_id) { return Err(err("clientId: letters, digits and - _ . : only, 64 at most")) } if let Some(e) = recalled(client_id) { return Ok(e) } }
    let far = machines::is_far(thread);
    if far { if !hub.far().map_or(false, |f| f.has(thread)) { return Err(err("unknown thread")) } if has { return Err(err("Images go to threads on this machine only.")) } }
    else if !hub.model().await.threads.get(thread).map_or(false, |t| !t.deleted) { return Err(err("unknown thread")) }
    let (meta, paths) = store_attachments(hub, thread, attachments)?;
    let agent_text = if paths.is_empty() { String::new() } else { format!("{}{}{}", text, if text.is_empty() { "" } else { "\n\n" }, paths.iter().map(|p| format!("[attachment: {p}]")).collect::<Vec<_>>().join("\n")) };
    let msg = if client_id.is_empty() { format!("m{}-{}", now_ms(), hub.fresh_client()) } else { client_id.to_string() };
    let mut p = json!({"thread":thread,"text":text,"msg":msg});
    if !meta.is_empty() { p["attachments"] = json!(meta); p["agentText"] = json!(agent_text) }
    rpc(hub, c, "turn.start", p).await?;
    let mut e = json!({"id":msg,"kind":"user","text":text,"at":now_ms()});
    if !meta.is_empty() { e["attachments"] = json!(meta) }
    if !client_id.is_empty() { e["clientId"] = json!(client_id); remember(client_id, &e) }
    Ok(e)
}
/// `attachment.get {id}`: an image a message carries, from the thread's folder; the id names the thread and the stored file.
fn attachment_get(hub: &Hub, p: &Value) -> Result<Value, Value> {
    use base64::Engine;
    let id = s(p, "id");
    let Some((th, file)) = id.split_once('~') else { return Err(err("unknown attachment")) };
    let clean = |x: &str| !x.is_empty() && x.len() <= 220 && x.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) && !x.starts_with('.');
    if !clean(th) || !clean(file) { return Err(err("unknown attachment")) }
    let path = hub.cfg.home.join("attachments").join(th).join(file);
    let meta = std::fs::metadata(&path).map_err(|_| err("unknown attachment"))?;
    if !meta.is_file() || meta.len() as usize > MAX_ATTACH_BYTES { return Err(err("unknown attachment")) }
    let bytes = std::fs::read(&path).map_err(|_| err("unknown attachment"))?;
    Ok(json!({"mime":mime_of(file),"data":base64::engine::general_purpose::STANDARD.encode(bytes)}))
}

async fn call(hub: &Arc<Hub>, c: &mut Conn, m: &str, p: &Value) -> Result<Value, Value> {
    match m {
        "projects.list" => {
            // a machine just linked whose origins are not known yet is asked first (bounded), so its copies are in the merged rows of this answer
            for l in crate::mirror::unknown_links(hub) { let _ = tokio::time::timeout(Duration::from_secs(3), crate::mirror::ask_origins(hub, &l)).await; }
            let g = hub.model().await; let asks = open_ask_counts(hub);
            let (items, chips) = with_far(hub, s(p, "machine"), |far, un, pick| (machines::pick_rows(merged_rows(&g, &asks, far, un), pick), machines::chips(far, un, pick)));
            let (projects, next) = page_of(items, limit_of(p, 50, 200), s(p, "before")); Ok(json!({"projects":projects,"next":next,"machines":chips})) }
        // the sidebar's sections (docs/ALWAYS-ON.md section 8, present/sections.rs), the same the window draws: Needs you, Pinned, Active, the groups, Quiet;
        // `open` the folds opened ("sec:quiet", "sec:group:<gid>"), `query` the search, `page` a project's key for its page ("pageDone": its Done open).
        // Thread ids are this hub's (a linked machine's `<link>~<id>`); a row whose machine is off has `off` and no `nav`.
        "sidebar.sections" => {
            let Some(pol) = crate::statehost::policies() else { return Err(err("state is starting")) };
            let open: std::collections::HashSet<String> = p["open"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
            let (q, key, done) = (s(p, "query").to_string(), s(p, "page").to_string(), p["pageDone"] == true);
            let now = crate::hub::now_ms() / 1000;
            pol.with_state(|st| crate::present::merged::merged_of(st, hub.mirrors(), |rows| {
                let c = crate::present::sections::Ctx { hub: "", now, query: &q, open: &open, selected: "" };
                let mut v = json!({"sections": crate::present::sections::build(st, &rows, &Value::Null, &c), "groupsTo": crate::present::sections::move_targets(st, &rows)});
                if p["jump"] == true { v["jump"] = crate::present::sections::jump(&rows, &c); }
                if !key.is_empty() { v["page"] = crate::present::sections::page(st, &rows, &key, &c, done); }
                v
            })).ok_or_else(|| err("state is starting"))
        }
        // a sidebar section's press: pin a project (`project-pin` <key>), fold or unfold its threads (`project-fold` <key>), archive it (`project-archive` <key>, again to unarchive), hide another machine's thread here (`thread-hide` <id>), name a group (`group-rename` "<gid>\u{1f}<name>"), move a project (`group-move` "<key>\u{1f}<gid>")
        "sidebar.act" => {
            let Some(pol) = crate::statehost::policies() else { return Err(err("state is starting")) };
            let (a, v) = (s(p, "action"), s(p, "value"));
            if !matches!(a, "project-pin" | "project-fold" | "thread-hide" | "group-rename" | "group-move" | "project-archive") { return Err(err("no such action")) }
            let now = crate::hub::now_ms() / 1000;
            let sets = pol.with_state(|st| crate::present::merged::merged_of(st, hub.mirrors(), |rows| crate::present::sections::act(st, &rows, a, v, &json!({}), now).map(|x| x.sets))).flatten().ok_or_else(|| err("unknown project or group"))?;
            for (k, val) in sets { rpc(hub, c, "setting.set", json!({"key":k,"value":val})).await?; }
            Ok(json!({}))
        }
        "threads.list" => { let at = hub.setting("home.at").await.unwrap_or_default(); let farv = hub.far();
            // the old copy of a thread the home took to another machine (the uberbot's, a bot's) is not listed: that machine's is (net/homemove.rs)
            let moved = |id: &str, project: &str| crate::net::homemove::moved_thread(&at, id, Some(project), &|x| farv.as_ref().map_or(false, |f| f.has(x))).is_some();
            let g = hub.model().await; let asks = open_ask_counts(hub); let (proj, arch) = (s(p, "project"), p["archived"] == true);
            // `settled`: true is only the Settled shelf (newest settle first), false only the rest (newest activity first); left out, both, as before
            let want = p.get("settled").and_then(Value::as_bool);
            let lim = limit_of(p, 50, 200);
            // the linked machines' threads are listed with this hub's, newest first across machines (`machine`: "" every machine, "@here", or one machine's name)
            let (mut keyed, join, chips) = with_far(hub, s(p, "machine"), |far, un, pick| {
                let mut keyed: Vec<(u64, String)> = if machines::here_on(pick) { g.threads.iter().filter(|(i, t)| !t.deleted && t.archived == arch && (proj.is_empty() || t.project == proj) && t.project != crate::sidebar::SCRATCH && !moved(i, &t.project)).filter_map(|(i, t)| {
                    let (_, settled, at) = flags_of(i);
                    if want.map_or(false, |w| w != settled) { return None }
                    Some((if want == Some(true) { at } else { ms(t.activity.max(t.created)) }, i.clone())) }).collect() } else { vec![] };
                keyed.extend(machines::thread_keys(far, pick, un.far, proj, arch, want));
                (keyed, un.far.clone(), machines::chips(far, un, pick))
            });
            keyed.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            if let Some((a, i)) = s(p, "before").split_once(':') { if let Ok(a) = a.parse::<u64>() { keyed.retain(|x| x.0 < a || (x.0 == a && x.1.as_str() > i)) } }
            let more = keyed.len() > lim; keyed.truncate(lim);
            // only the page's threads are built in full (the preview reads the log)
            let mut items = vec![];
            for (at, i) in &keyed { if let Some(v) = thread_row(hub, &g, i, &asks, &join) { items.push((*at, i.clone(), v)) } }
            let next = if more { keyed.last().map(|x| json!(format!("{}:{}", x.0, x.1))).unwrap_or(Value::Null) } else { Value::Null };
            Ok(json!({"threads":items.into_iter().map(|x| x.2).collect::<Vec<_>>(),"next":next,"machines":chips})) }
        "thread.entries" => entries(hub, p).await,
        "link.resolve" => link::resolve(hub, p).await,
        // a buy thread's Order card (present/thread.rs `order_json`, what the desktop draws), null for any other thread; the app asks again on each push for the thread
        "thread.order" => { let th = s(p, "thread"); if th.is_empty() { return Err(err("thread")) }
            let o = crate::statehost::policies().and_then(|q| q.with_state(|st| crate::present::thread::order_json(st, th, st.threads.get(th).map_or(false, |t| t.turn == crate::state::Turn::Running)))).unwrap_or(Value::Null);
            Ok(json!({"order":o})) }
        "dirs.list" => dirs(p),
        "project.add" => {
            let path = s(p, "path"); if path.is_empty() { return Err(err("path")) }
            let want = crate::plat::fs::canonicalize(path).map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|_| path.to_string());
            // the phone opens its New thread screen next and makes the first thread from it: no empty starter thread (desktop's + keeps one)
            rpc(hub, c, "project.add", json!({"path":path, "starter": false})).await?;
            for _ in 0..40 {
                { let g = hub.model().await; let asks = HashMap::new();
                  if let Some((id, _)) = g.projects.iter().find(|(_, x)| !x.removed && (x.root == want || x.root == path)) { if let Some(v) = project_json(&g, id, &asks) { return Ok(json!({"project":v})) } } }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(err("the project was not added"))
        }
        "thread.create" => {
            let project = s(p, "project"); if project.is_empty() { return Err(err("project")) }
            if project == crate::sidebar::SCRATCH { return Err(err(SCRATCH_CLOSED)) }
            // like the desktop's +: the env follows the thread.env setting (auto: a worktree in a git project) unless the phone names one
            let mut q = json!({"project":project});
            if !s(p, "env").is_empty() { if !matches!(s(p, "env"), "auto" | "worktree" | "local") { return Err(err("env: auto, worktree or local")) } q["env"] = p["env"].clone() }
            for k in ["title", "provider", "model"] { if !s(p, k).is_empty() { q[k] = p[k].clone() } }
            let r = rpc(hub, c, "thread.create", q).await?;
            let id = r["thread"].as_str().unwrap_or("").to_string(); if id.is_empty() { return Err(err("no thread was made")) }
            let far = machines::is_far(&id);
            let runtime = if !s(p, "runtime").is_empty() { s(p, "runtime") } else if s(p, "permission") == "auto" { "full-access" } else { "approval-required" };
            rpc(hub, c, "thread.modes", json!({"thread":id,"runtime":runtime})).await?;
            if !s(p, "effort").is_empty() { rpc(hub, c, "thread.effort", json!({"thread":id,"effort":s(p, "effort")})).await?; }
            // (a thread just made on a linked machine is not in the mirror yet: its first message goes straight there)
            if !s(p, "text").is_empty() { if far { rpc(hub, c, "turn.start", json!({"thread":id,"text":s(p, "text"),"msg":format!("m{}-{}", now_ms(), hub.fresh_client())})).await?; } else { send_message(hub, c, &id, s(p, "text"), "", &Value::Null).await?; } }
            let g = hub.model().await; let asks = open_ask_counts(hub);
            if far { return Ok(json!({"thread":far_made(hub, &g, &id, project, s(p, "title"))})) }
            thread_json(hub, &g, &id, &asks).map(|t| json!({"thread":t})).ok_or_else(|| err("no thread was made"))
        }
        "message.send" => Ok(json!({"entry":send_message(hub, c, s(p, "thread"), s(p, "text"), s(p, "clientId"), p.get("attachments").unwrap_or(&Value::Null)).await?})),
        "attachment.get" => attachment_get(hub, p),
        "ask.answer" => { rpc(hub, c, "ask.answer", json!({"id":s(p, "ask"),"answer":s(p, "answer")})).await?; Ok(json!({})) }
        // the same answer every client gets (its `result`, not the reply around it); the phones draw it
        "update.status" | "update.check" | "app.update" | "update.now" => Ok(rpc(hub, c, m, json!({})).await?["result"].clone()),
        "hub.offer.take" | "hub.offer.done" => Ok(rpc(hub, c, m, p.clone()).await?["result"].clone()),
        "update.channel" => { let ch = s(p, "channel"); if ch != "stable" && ch != "beta" { return Err(err("no such channel")) } rpc(hub, c, "setting.set", json!({"key":"update.channel","value":ch})).await?; Ok(rpc(hub, c, "update.status", json!({})).await?["result"].clone()) }
        // Improvements: the projects of every machine a request may go to (searched on the hub) and the last one chosen, as the desktop gets them
        "improvements.projects" | "improvements.project" => Ok(rpc(hub, c, m, p.clone()).await?["result"].clone()),
        "turn.interrupt" => { rpc(hub, c, "turn.interrupt", json!({"thread":s(p, "thread")})).await?; Ok(json!({})) }
        _ => match ext::call(hub, c, m, p).await { Some(r) => r, None => Err(err("unknown method")) },
    }
}

// ---- subscriptions ----
#[derive(Clone, PartialEq)]
enum Topic { Lists, Thread(String), Sub(String, String) }
#[derive(Default)]
struct Subs { far_rows: HashMap<String, (HashSet<String>, String)>, far_seen: Option<HashMap<String, String>>, proj_seen: Option<HashMap<String, String>>, next: u64, subs: Vec<(u64, Topic)>, live: HashMap<String, Streams>, counts: HashMap<String, (u64, u64)>, terms: HashSet<String> }
/// A thread's streamed replies on one connection. Frames (deltas, their end) and log lines (the reply's final message) reach the connection on two channels, so
/// the final message of a reply a person's message cut can arrive after the next reply has begun streaming. Each stream has its own entry (`live:<thread>`, then
/// `live:<thread>.<n>`), a stream ends at its `delta_end` frame (which comes in order with the deltas), and a final message replaces the oldest stream that ended
/// without one; one that arrives before its stream's end replaces the open stream, and that end is then owed.
#[derive(Default, Debug)]
pub(crate) struct Streams { open: Option<u64>, ended: std::collections::VecDeque<u64>, owed: u32, next: u64 }
impl Streams {
    pub(crate) fn id(th: &str, n: u64) -> String { if n == 0 { format!("live:{th}") } else { format!("live:{th}.{n}") } }
    /// A delta: the stream it belongs to, and whether it starts here.
    pub(crate) fn delta(&mut self) -> (u64, bool) { match self.open { Some(n) => (n, false), None => { let n = self.next; self.next += 1; self.open = Some(n); (n, true) } } }
    pub(crate) fn end(&mut self) { if let Some(n) = self.open.take() { if self.owed > 0 { self.owed -= 1 } else { self.ended.push_back(n) } } }
    /// A reply's final message: the stream it replaces, if any.
    pub(crate) fn finished(&mut self) -> Option<u64> { if let Some(n) = self.ended.pop_front() { return Some(n) } let n = self.open.take()?; self.owed += 1; Some(n) }
}
impl Subs {
    fn threads(&self, th: &str) -> Vec<u64> { self.subs.iter().filter(|(_, t)| *t == Topic::Thread(th.into())).map(|(i, _)| *i).collect() }
    /// the subscriptions to one subagent's view, `(sub, key)` for thread `th`; `key` None: every one of the thread's
    fn sub_views(&self, th: &str, key: Option<&str>) -> Vec<(u64, String)> { self.subs.iter().filter_map(|(i, t)| match t { Topic::Sub(x, k) if x == th && key.map_or(true, |w| w == k) => Some((*i, k.clone())), _ => None }).collect() }
    fn lists(&self) -> Vec<u64> { self.subs.iter().filter(|(_, t)| *t == Topic::Lists).map(|(i, _)| *i).collect() }
}
fn swarm_of(th: &str) -> Option<String> { crate::statehost::policies().and_then(|p| p.with_state(|st| st.swarm_of.get(th).cloned())).flatten() }
fn is_swarm(room: &str) -> bool { crate::statehost::policies().and_then(|p| p.with_state(|st| st.rooms_nav.get(room).map_or(false, |r| r.swarm()))).unwrap_or(false) }
fn d(sub: u64, k: &str, mut body: Value) -> Value { body["t"] = json!("d"); body["sub"] = json!(sub); body["k"] = json!(k); body }
fn turn_word(state: &str) -> &'static str { match state { "running" => "working", "failed" => "failed", _ => "idle" } }

/// The pushes for new log lines, for the lists and threads this connection subscribed to.
async fn deltas(hub: &Hub, lines: &[String], subs: &mut Subs) -> Vec<Value> {
    if subs.subs.is_empty() { return vec![] }
    let mut out = vec![]; let (mut upsert, mut gone, mut projects): (Vec<String>, Vec<String>, Vec<(String, bool)>) = (vec![], vec![], vec![]);
    let (mut worked, mut locks_moved, mut bots_moved): (Vec<String>, bool, bool) = (vec![], false, false); let mut swarms: Vec<String> = vec![];
    // Bot.backend: per bot conversation, whether the turn now being written is the person's (the lines before this batch come from the model)
    let base = hub.store.len().saturating_sub(lines.len()) as u64; let mut last_in: HashMap<String, crate::present::thread::Turnk> = HashMap::new();
    let mut viewed: Vec<(u64, String, String)> = vec![];
    for l in lines {
        let Ok(e) = serde_json::from_str::<Value>(l) else { continue };
        let opened = if s(&e, "$") == "AskClosed" { ask_opened(hub, s(&e, "id")) } else { None };
        let th = if matches!(s(&e, "$"), "ThreadCreated" | "ThreadRenamed" | "ThreadSettled" | "ThreadUnsettled" | "ThreadWoke" | "ThreadMerged" | "ThreadArchived" | "ThreadPinned" | "ThreadForked" | "ProviderSet" | "ModesSet" | "ThreadDeleted") { s(&e, "id") } else if let Some(o) = &opened { s(o, "thread") } else { s(&e, "thread") }.to_string();
        let mut entry: Option<Value> = None;
        // a subagent's view moves with its own log line, and with its thread's background rows and turn
        if s(&e, "$") == "ActivityLogged" { if let Some(of) = th.strip_prefix("@sub:") { for (sub, k) in subs.sub_views(of, Some(s(&e, "tone"))) { if !viewed.contains(&(sub, of.to_string(), k.clone())) { viewed.push((sub, of.to_string(), k)) } } } }
        if matches!(s(&e, "$"), "WorkSet" | "TurnChanged") { for (sub, k) in subs.sub_views(&th, None) { if !viewed.contains(&(sub, th.clone(), k.clone())) { viewed.push((sub, th.clone(), k)) } } }
        match s(&e, "$") {
            "ThreadDeleted" => gone.push(th.clone()),
            "WorkSet" => { let w = s(&e, "thread").to_string(); if !worked.contains(&w) { worked.push(w) } }
            "LockChanged" => locks_moved = true,
            // the bots, the uberbot or where the home is changed (a home move): the phones read bots.list again, and an open chat of the uberbot follows it
            "BotSet" | "BotDeleted" | "UberSet" => bots_moved = true,
            "SettingSet" if matches!(s(&e, "key"), "home.moving" | "home.at") => bots_moved = true,
            "ProjectCreated" | "ProjectRenamed" => projects.push((s(&e, "id").to_string(), true)),
            "ProjectRemoved" => projects.push((s(&e, "id").to_string(), false)),
            "MessagePosted" => {
                let (id, role) = (s(&e, "msg"), s(&e, "role"));
                let kind = match role { "user" => "user", "assistant" => "assistant", _ => "note" };
                // as a page has it: a quote's Buy now link ready to open, and the hub's note to the agent marked so no phone shows it
                let mut en = json!({"id":id,"kind":kind,"text":if kind == "user" { shown_text(s(&e, "text")) } else { crate::settings::order::pay_shown(s(&e, "text")) },"at":ms(e["at"].as_u64().unwrap_or(0))});
                let bot = bot_chat(&th);
                let backend = bot && {
                    if !last_in.contains_key(&th) { let m = hub.model().await; let rows: Vec<crate::model::Row> = m.tail(&hub.store, &th, 48).into_iter().filter(|r| r.seq <= base).collect(); last_in.insert(th.clone(), crate::present::thread::turn_of(&rows)); }
                    let t = last_in.get_mut(&th).unwrap(); t.step(id, role, s(&e, "text")) && role == "assistant" && !crate::present::thread::told_row(id)
                };
                if crate::present::thread::hidden(bot, id, role, s(&e, "text")) || backend { en["agentOnly"] = json!(true) }
                if let Some(a) = e.get("attachments") { en["attachments"] = a.clone() }
                if kind == "user" { if let Some(x) = recalled(id) { if let Some(c) = x.get("clientId") { en["clientId"] = c.clone() } } }
                let replaced = if kind == "assistant" { subs.live.get_mut(&th).and_then(|l| l.finished()) } else { None };
                if let Some(n) = replaced { en["replaces"] = json!(Streams::id(&th, n)) }
                if replaced.is_some() || kind != "assistant" || !s(&e, "text").trim().is_empty() { entry = Some(en) }
            }
            "MessageAttached" => { let id = s(&e, "msg"); let mut en = json!({"id":id,"kind":"user","text":"","at":ms(e["at"].as_u64().unwrap_or(0)),"attachments":e["attachments"]}); if let Some(x) = recalled(id) { en["text"] = x["text"].clone(); if let Some(c) = x.get("clientId") { en["clientId"] = c.clone() } } entry = Some(en) }
            "ActivityLogged" => entry = act_entry(&th, s(&e, "act"), s(&e, "tone"), s(&e, "kind"), s(&e, "summary"), ms(e["at"].as_u64().unwrap_or(0))),
            "AskOpened" => entry = Some(ask_entry(&e, None)),
            // the only alert a phone hears on its socket: what the uberbot's gate let through (push), and its clear
            "PushSent" => { for sub in subs.lists() { out.push(d(sub, "notify", ext::notify_of(&e))) } }
            "PushCleared" => { for sub in subs.lists() { out.push(d(sub, "clear", json!({"keys":e["keys"],"badge":e["badge"]}))) } }
            // the whole ask again, answered (an AskClosed names only the ask: its thread and question come from its AskOpened)
            "AskClosed" => { let id = s(&e, "id"); entry = Some(match &opened { Some(o) => ask_entry(o, Some(s(&e, "answer"))), None => json!({"id":format!("ask:{id}"),"kind":"ask","text":"","at":ms(e["at"].as_u64().unwrap_or(0)),"ask":{"id":id,"state":"answered","question":"","options":[],"answered":s(&e,"answer")}}) }); }
            // an agent asked for a secret, or the ask ended: the phone reads the list again (`secret.asks`). Only a name and a thread: no value ever passes here
            "SettingSet" if s(&e, "key").starts_with("secret.ask.") => {
                let t: Value = serde_json::from_str(s(&e, "value")).unwrap_or(Value::Null); let at = s(&t, "thread").to_string();
                let open: Vec<(u64, String)> = subs.subs.iter().filter_map(|(i, t)| match t { Topic::Thread(x) if at.is_empty() || *x == at => Some((*i, x.clone())), _ => None }).collect(); // (an ended ask names no thread: each open thread looks again)
                for (sub, x) in open { out.push(d(sub, "secret", json!({"thread":x}))) }
            }
            "TurnChanged" => { for sub in subs.threads(&th) { out.push(d(sub, "turn", json!({"thread":th,"state":turn_word(s(&e, "state"))}))) }
                if let Some(r) = swarm_of(&th) { if !swarms.contains(&r) { swarms.push(r) } } },
            // a swarm's board or members moved (docs/SWARM.md): the phone reads `swarm.get {since}` again
            "RoomSet" | "RoomPosted" => { let r = if s(&e, "$") == "RoomSet" { s(&e, "id") } else { s(&e, "room") }.to_string(); if is_swarm(&r) && !swarms.contains(&r) { swarms.push(r) } },
            _ => {}
        }
        if !th.is_empty() && !matches!(s(&e, "$"), "ThreadDeleted") && !th.starts_with("remind") { if !upsert.contains(&th) { upsert.push(th.clone()) } }
        if let Some(en) = entry { for sub in subs.threads(&th) { out.push(d(sub, "entry", json!({"thread":th,"entry":en}))) } }
    }
    // a lock moving changes the rows of every thread with a place on it, so each open thread is sent its own again
    if locks_moved { for (_, t) in &subs.subs { if let Topic::Thread(th) = t { if !worked.contains(th) { worked.push(th.clone()) } } } }
    for (sub, th, k) in &viewed { let v = hub.sub_view_for(th, k).await; out.push(d(*sub, "sub", json!({"thread":th,"key":k,"view":v}))) }
    for th in &worked { let subs_now = ext::work_now(hub, th).await; for sub in subs.threads(th) { out.push(d(sub, "work", json!({"thread":th,"subs":subs_now}))) } }
    if bots_moved { for sub in subs.lists() { out.push(d(sub, "bots", json!({}))) } }
    if locks_moved { let l = ext::locks_now(hub).await; for sub in subs.lists() { out.push(d(sub, "locks", json!({"locks":l}))) } }
    for r in &swarms { for sub in subs.lists() { out.push(d(sub, "swarm", json!({"room":r}))) } }
    if upsert.is_empty() && gone.is_empty() && projects.is_empty() { return out }
    let g = hub.model().await; let asks = open_ask_counts(hub);
    for th in &upsert { if let Some(t) = thread_json(hub, &g, th, &asks) { for sub in subs.lists().into_iter().chain(subs.threads(th)) { out.push(d(sub, "thread", json!({"thread":t}))) } } }
    // a project's counts (threads, settled) move with its threads
    let moved: Vec<String> = { let mut v: Vec<String> = vec![]; for th in upsert.iter() { if let Some(t) = g.threads.get(th) { if !projects.iter().any(|(i, _)| *i == t.project) && !v.contains(&t.project) { v.push(t.project.clone()) } } } v };
    // pushed only when a count moved (a message in a thread leaves them as they were)
    for pid in &moved { if let Some(pj) = merged_row(hub, &g, &asks, pid) { let key = (pj["threadCount"].as_u64().unwrap_or(0), pj["settledCount"].as_u64().unwrap_or(0));
        if subs.counts.insert(pid.clone(), key) != Some(key) { for sub in subs.lists() { out.push(d(sub, "project", json!({"project":pj}))) } } } }
    for th in &gone { for sub in subs.lists().into_iter().chain(subs.threads(th)) { out.push(d(sub, "thread.removed", json!({"id":th}))) } }
    for (id, alive) in &projects { for sub in subs.lists() { match alive.then(|| merged_row(hub, &g, &asks, id)).flatten() { Some(p) => out.push(d(sub, "project", json!({"project":p}))), None => out.push(d(sub, "project.removed", json!({"id":id}))) } } }
    out
}
/// The pushes for the linked machines' threads, which no log line of this hub announces: their mirrors are read again every few seconds, and what moved is pushed
/// (`thread` rows to a lists subscription, `thread.removed`, and for an open far thread its new `entry` rows and its `turn`). Nothing is pushed when nothing moved.
async fn far_pushes(hub: &Hub, subs: &mut Subs) -> Vec<Value> {
    let mut out = vec![];
    let join = with_far(hub, "", |_, un, _| un.far.clone());
    let g = hub.model().await; let asks = HashMap::new();
    if !subs.lists().is_empty() {
        let ids: Vec<String> = with_far(hub, "", |far, un, pick| machines::thread_keys(far, pick, un.far, "", false, None).into_iter().chain(machines::thread_keys(far, pick, un.far, "", true, None)).map(|x| x.1).collect());
        let first = subs.far_seen.is_none(); let mut seen = subs.far_seen.take().unwrap_or_default(); let mut now = HashMap::new();
        for id in &ids { if let Some(t) = thread_row(hub, &g, id, &asks, &join) {
            let digest = format!("{}|{}|{}|{}|{}|{}|{}|{}", t["updatedAt"], t["state"], t["title"], t["settled"], t["archived"], t["pinned"], t["project"], t["mark"]["away"]);
            if !first && seen.get(id) != Some(&digest) { for sub in subs.lists() { out.push(d(sub, "thread", json!({"thread":t}))) } }
            now.insert(id.clone(), digest); } }
        if !first { for id in seen.keys().filter(|i| !now.contains_key(*i)) { for sub in subs.lists() { out.push(d(sub, "thread.removed", json!({"id":id}))) } } }
        subs.far_seen = Some(now); let _ = seen;
        // the merged project rows move with no log line here (a linked machine's origins learned, it away or back, its rows from the registry): each
        // row that changed is pushed, one that went is removed
        let asks = open_ask_counts(hub);
        let rows: HashMap<String, Value> = with_far(hub, "", |far, un, _| merged_rows(&g, &asks, far, un).into_iter().map(|x| (x.1, x.2)).collect());
        let firstp = subs.proj_seen.is_none(); let before = subs.proj_seen.take().unwrap_or_default(); let mut now: HashMap<String, String> = HashMap::new();
        for (id, r) in &rows { let digest = r.to_string(); if !firstp && before.get(id) != Some(&digest) { for sub in subs.lists() { out.push(d(sub, "project", json!({"project":r}))) } } now.insert(id.clone(), digest); }
        if !firstp { for id in before.keys().filter(|i| !now.contains_key(*i)) { for sub in subs.lists() { out.push(d(sub, "project.removed", json!({"id":id}))) } } }
        subs.proj_seen = Some(now);
    }
    let open: Vec<String> = subs.subs.iter().filter_map(|(_, t)| match t { Topic::Thread(x) if machines::is_far(x) => Some(x.clone()), _ => None }).collect();
    if let Some(f) = hub.far() { for th in open {
        let Some(page) = f.page(&th, 0, 32) else { continue };
        let mut rows: Vec<&crate::model::Row> = page.rows.iter().filter(|r| r.thread == th).collect(); rows.sort_by_key(|r| r.ord);
        let turn = hub.mirrors().shown(|far| machines::holder(far, &th).map_or(String::new(), |(_, st)| st.threads.get(&th).map_or(String::new(), |t| turn_word(t.turn.word()).to_string())));
        let targets = subs.threads(&th); let bot = far_bot(hub, &th);
        let known = subs.far_rows.entry(th.clone()).or_insert_with(|| (rows.iter().map(|r| r.id.clone()).collect(), turn.clone()));
        let backend = if bot { let o = oldest_ord(&page.rows); backend_of(&page.rows, if o > 1 { f.page(&th, o, 32).map(|p| p.rows).unwrap_or_default() } else { vec![] }) } else { HashSet::new() };
        for r in rows { if known.0.insert(r.id.clone()) { if let Some(en) = row_entry_hid(r, bot, &backend) { for sub in &targets { out.push(d(*sub, "entry", json!({"thread":th,"entry":en}))) } } } }
        if known.1 != turn && !turn.is_empty() { known.1 = turn.clone(); for sub in &targets { out.push(d(*sub, "turn", json!({"thread":th,"state":turn}))) } }
    } }
    out
}
/// What an open far thread already shows when it is subscribed to: what the mirror adds after this is pushed (`far_pushes`).
fn far_seed(hub: &Hub, subs: &mut Subs, th: &str) {
    if !machines::is_far(th) || subs.far_rows.contains_key(th) { return }
    let Some(page) = hub.far().and_then(|f| f.page(th, 0, 32)) else { return };
    let turn = hub.mirrors().shown(|far| machines::holder(far, th).map_or(String::new(), |(_, st)| st.threads.get(th).map_or(String::new(), |t| turn_word(t.turn.word()).to_string())));
    subs.far_rows.insert(th.to_string(), (page.rows.iter().filter(|r| r.thread == th).map(|r| r.id.clone()).collect(), turn));
}
/// Streamed text of a running turn.
fn live_pushes(f: &Value, subs: &mut Subs) -> Vec<Value> {
    let th = s(f, "thread").to_string(); let mut out = vec![];
    match s(f, "t") {
        "delta" => { let (n, first) = subs.live.entry(th.clone()).or_default().delta(); let text = s(f, "text"); let id = Streams::id(&th, n);
            for sub in subs.threads(&th) {
                if first { let mut e = live_entry(&th, ""); e["id"] = json!(id); out.push(d(sub, "entry", json!({"thread":th,"entry":e}))) }
                out.push(d(sub, "text", json!({"thread":th,"entry":id,"append":text,"at":now_ms()})));
            } }
        "delta_end" => { if let Some(l) = subs.live.get_mut(&th) { l.end() } }
        // what a thread's queue strip shows changed with no line in the log (a message on its way to another machine: net/farsend.rs): ask thread.queue again
        "queue" => for sub in subs.threads(&th) { out.push(d(sub, "queue", json!({"thread":th}))) },
        // the hub the person's home moved to (net/homemove.rs): the phone adds it by itself
        "info" => if let Some(o) = f["info"].get("hub.offer") { out.push(json!({"t":"info","info":{"hub.offer":o}})) },
        _ => {}
    }
    out
}

async fn handle(hub: &Arc<Hub>, c: &mut Conn, subs: &mut Subs, v: &Value) -> Value {
    let (id, m, p) = (v["id"].clone(), s(v, "m"), v.get("p").cloned().unwrap_or(Value::Null));
    let res = match m {
        "subscribe" => match (s(&p, "topic"), s(&p, "thread")) {
            ("lists", _) => { subs.next += 1; subs.subs.push((subs.next, Topic::Lists)); Ok(json!({"sub":subs.next})) }
            ("thread", th) if hub.model().await.threads.get(th).map_or(false, |t| !t.deleted) || hub.far().map_or(false, |f| f.has(th)) => { subs.next += 1; subs.subs.push((subs.next, Topic::Thread(th.into()))); far_seed(hub, subs, th); Ok(json!({"sub":subs.next})) }
            ("thread", _) => Err(err("unknown thread")),
            // feature "subview": one subagent's view, pushed whole (`k:"sub"` {thread, key, view}) when its log, its row or its turn moves
            ("sub", th) if !s(&p, "key").is_empty() && hub.model().await.threads.get(th).map_or(false, |t| !t.deleted) => { subs.next += 1; subs.subs.push((subs.next, Topic::Sub(th.into(), s(&p, "key").into()))); Ok(json!({"sub":subs.next})) }
            ("sub", _) => Err(err("unknown thread or key")),
            _ => Err(err("topic: lists, thread or sub")),
        },
        "unsubscribe" => { let n = p["sub"].as_u64().unwrap_or(0); subs.subs.retain(|(i, _)| *i != n); Ok(json!({})) }
        _ => match term::call(hub, c.client, &mut subs.terms, m, &p).await { Some(r) => r, None => call(hub, c, m, &p).await },
    };
    match res { Ok(r) => json!({"t":"reply","id":id,"ok":true,"result":r}), Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}) }
}

/// A request that waits on another machine (its thread or project is `<link>~<id>`, or a viewer read about a drawing watched there) is run off the socket's loop,
/// so a machine that is slow or away holds up nothing else on this phone; its reply comes when it comes.
fn waits_far(v: &Value) -> bool {
    let (m, p) = (s(v, "m"), v.get("p").unwrap_or(&Value::Null));
    if m.starts_with("term.") { return term::far_wait(m, p) }
    if matches!(m, "subscribe" | "unsubscribe" | "thread.entries" | "link.resolve") { return false }
    machines::is_far(s(p, "thread")) || machines::is_far(s(p, "project")) || matches!(m, "viewer.info" | "viewer.probe" | "viewer.watch")
}
pub async fn socket(hub: Arc<Hub>, s: TcpStream, json_mode: bool, token: String, host: String) -> Result<()> {
    let mut cfg = WebSocketConfig::default(); cfg.max_message_size = Some(40 << 20); cfg.max_frame_size = Some(40 << 20); // (images come as base64 in one JSON frame: 4 x 8 MB)
    let (mut tx, mut rx) = tokio_tungstenite::accept_async_with_config(s, Some(cfg)).await?.split();
    let (mut conn, _) = hub.connect(true);
    let mut outbox = hub.attach(conn.client);
    struct Detach(Arc<Hub>, u64); impl Drop for Detach { fn drop(&mut self) { self.0.detach(self.1) } }
    let _detach = Detach(hub.clone(), conn.client);
    if !hub.admit_blocking(conn.client, token).await { return Ok(()) }
    // `name`: the machine's name (BACKPLANE_NAME, else its host name), for the phone's title instead of the address it was paired at
    static NAME: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
    let name = NAME.get_or_init(crate::net::botnet::name).await.clone();
    tx.send(pack(&json!({"t":"hello","proto":PROTO,"hub":host,"name":name,"version":hub.cfg.version,"epoch":hub.epoch,"now":now_ms(),"features":FEATURES,"hubOffer":hub.info_now()["hub.offer"].as_str().unwrap_or(""),"commands":crate::session::composer::words(),"notice":crate::guard::notice(&crate::guard::status(&hub.cfg.home)),"migrating":crate::import014::phone_state(&hub).await}), json_mode)).await?;
    let (late_tx, mut late_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
    let (mut changes, mut frames) = (hub.changed.subscribe(), hub.frames.subscribe());
    let mut seen = hub.store.len(); let mut subs = Subs::default();
    let mut beat = tokio::time::interval(Duration::from_millis(15000)); beat.tick().await;
    let mut far_beat = tokio::time::interval(Duration::from_millis(2000)); far_beat.tick().await;
    loop {
        tokio::select! {
            m = rx.next() => {
                let Some(m) = m else { return Ok(()) }; let m = m?;
                if matches!(m, Message::Close(_)) { return Ok(()) }
                let v = match wire::decode(&m) { None => continue, Some(Err(r)) => { tx.send(pack(&r, json_mode)).await?; continue } Some(Ok(v)) => v };
                if waits_far(&v) {
                    let (hub, late, id, m, p) = (hub.clone(), late_tx.clone(), v["id"].clone(), v["m"].as_str().unwrap_or("").to_string(), v.get("p").cloned().unwrap_or(Value::Null));
                    let mut c2 = Conn { sent: vec![], client: conn.client, gen: conn.gen.clone(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 };
                    tokio::spawn(async move {
                        let res = match term::far_wait(&m, &p) { true => term::far_call(&hub, c2.client, &m, &p).await, false => call(&hub, &mut c2, &m, &p).await };
                        let _ = late.send(match res { Ok(r) => json!({"t":"reply","id":id,"ok":true,"result":r}), Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}) });
                    });
                    continue
                }
                let r = handle(&hub, &mut conn, &mut subs, &v).await; tx.send(pack(&r, json_mode)).await?;
            }
            r = late_rx.recv() => { if let Some(r) = r { tx.send(pack(&r, json_mode)).await?; } }
            n = changes.recv() => {
                if let Err(tokio::sync::broadcast::error::RecvError::Closed) = n { return Ok(()) }
                let len = hub.store.len();
                if len > seen { let lines = hub.store.range(seen, len.min(seen + 2000)).unwrap_or_default(); seen += lines.len(); for f in deltas(&hub, &lines, &mut subs).await { tx.send(pack(&f, json_mode)).await?; } }
                if len > seen { let _ = hub.changed.send(len as u64); } // (more than one pass: wake again)
            }
            f = frames.recv() => { if let Ok(f) = f { for p in live_pushes(&f, &mut subs) { tx.send(pack(&p, json_mode)).await?; } } }
            o = outbox.recv() => { let Some(o) = o else { return Ok(()) };
                match o { Out::Close => return Ok(()),
                    // a viewer's plot frames (binary CBOR) and its small announcements; anything else the hub pushes is for the other clients
                    Out::Raw(b) => if ext::wants_raw(&b) { tx.send(Message::Binary(b.into())).await?; },
                    // a terminal this socket watches: its bytes and where they start (data/term.rs)
                    Out::Value(v) if v["t"] == "term" => if let Some(f) = term::push_of(&subs.terms, &v) { tx.send(pack(&f, json_mode)).await?; },
                    Out::Value(v) => if ext::wants_value(&v) {
                        // a plot's "none" is binary like the plots (one decoder in the app); a browser frame's announcement is JSON
                        if v["t"] == "plot" { let mut b = Vec::new(); cbor_plain(&v, &mut b); tx.send(Message::Binary(b.into())).await?; } else { tx.send(pack(&v, json_mode)).await?; } } } }
            _ = far_beat.tick() => { if !subs.subs.is_empty() { for f in far_pushes(&hub, &mut subs).await { tx.send(pack(&f, json_mode)).await?; } } }
            _ = beat.tick() => { tx.send(pack(&json!({"t":"hb"}), json_mode)).await?; }
        }
    }
}

#[cfg(test)]
mod tests;

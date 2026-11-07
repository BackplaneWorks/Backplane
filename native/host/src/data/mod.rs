//! The phone data API (docs/PHONE-DATA.md, proto "data/1"): plain data over one WebSocket, no presenter and no view schema.
//! `/ws?data=1[&enc=json]&token=…` (the same auth and pairing as every socket; `enc=json` is text frames, otherwise binary CBOR with plain string keys).
//! Reads come from the model (`Model::page`, thread and project metadata) and the policy state; writes are the hub's own RPCs (`project.add`, `thread.create`,
//! `turn.start`, `ask.answer`, `turn.interrupt`) run as this connection; subscriptions turn each new log line, and the streamed text of a running turn, into
//! pushes (`thread`, `project`, `entry`, `text`, `turn`) for the lists and threads a connection asked for.
mod ext;
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
pub const FEATURES: &[&str] = &["images", "actions", "search", "models", "mentions", "bots", "notify", "viewers", "files", "browser", "diff", "review", "fold", "picker", "voice", "order", "layers", "onboard", "push"];
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
fn flags(hub: &Hub, id: &str) -> (bool, bool) {
    let _ = hub;
    crate::statehost::policies().and_then(|p| p.with_state(|st| st.threads.get(id).map(|t| (t.pinned, t.settled_at > t.unsettled_at)))).flatten().unwrap_or((false, false))
}
/// A setting of the policy state ("" when unset or the state is not up).
fn setting_of(k: &str) -> String { crate::statehost::policies().and_then(|p| p.with_state(|st| st.settings.get(k).cloned())).flatten().unwrap_or_default() }
fn thread_json(hub: &Hub, m: &crate::model::Model, id: &str, asks: &HashMap<String, u64>) -> Option<Value> {
    let t = m.threads.get(id).filter(|t| !t.deleted)?;
    let waiting = asks.get(id).copied().unwrap_or(0);
    let state = if waiting > 0 { "asking" } else { match t.turn.as_str() { "running" => "working", "failed" => "failed", _ => "idle" } };
    // what was last said, not the last tool call or a design step's record; a thread with only activity shows nothing
    let preview = m.page(&hub.store, id, 0, 8).ok().and_then(|p| p.rows.iter().find(|r| r.kind == "msg" && !r.text.trim().is_empty()).map(|r| clip(&shown_text(&r.text), PREVIEW))).unwrap_or_default();
    let (pinned, settled) = flags(hub, id);
    // a model chosen within the provider is a ModesSet (the policy state), a switch of provider a ProviderSet (the model's fold)
    let (model, effort, runtime) = crate::statehost::policies().and_then(|p| p.with_state(|st| (st.threads.get(id).map_or(String::new(), |t| t.model.clone()), st.settings.get(&format!("effort.{id}")).cloned().unwrap_or_default(), st.runtime.get(id).cloned().unwrap_or_default()))).unwrap_or_default();
    let model = if model.is_empty() { t.model.clone() } else { model };
    Some(json!({"id":id,"project":t.project,"title":t.title,"state":state,"updatedAt":ms(t.activity.max(t.created)),"pinned":pinned,"settled":settled,"archived":t.archived,
        "provider":t.provider,"model":model,"effort":effort,"runtime":runtime,"chip":ext::chip(&t.provider, &model, &effort),"preview":preview,"waiting":waiting,"parent":setting_of(&format!("thread.parent.{id}"))}))
}
fn project_json(m: &crate::model::Model, id: &str, asks: &HashMap<String, u64>) -> Option<Value> {
    let p = m.projects.get(id).filter(|p| !p.removed)?;
    let ts: Vec<_> = m.threads.iter().filter(|(_, t)| t.project == id && !t.deleted).collect();
    let count = ts.iter().filter(|(_, t)| !t.archived).count();
    let updated = ts.iter().map(|(_, t)| t.activity.max(t.created)).max().unwrap_or(0);
    let waiting: u64 = ts.iter().map(|(i, _)| asks.get(*i).copied().unwrap_or(0)).sum();
    let color = crate::present::badge::color_of(&setting_of(&format!("project.color.{id}")), id).to_string();
    Some(json!({"id":id,"title":p.title,"root":p.root,"threadCount":count,"updatedAt":ms(updated),"waiting":waiting,
        "icon":crate::present::badge::icon_of(&setting_of(&format!("project.icon.{id}"))),"css":crate::present::badge::css_of(&color),"color":color}))
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

fn ask_options(kind: &str, options: &str) -> Value {
    if kind == "approval" { return json!([{"id":"accept","label":"Allow"},{"id":"session","label":"Allow for this session"},{"id":"decline","label":"Decline"}]) }
    let labels: Vec<String> = match serde_json::from_str::<Value>(options) {
        Ok(Value::Array(xs)) => xs.iter().filter_map(|x| x.as_str().map(str::to_string).or_else(|| x["label"].as_str().map(str::to_string))).collect(),
        _ => options.split(|c| c == '\n' || c == '|').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
    };
    Value::Array(labels.into_iter().map(|l| json!({"id":l,"label":l})).collect())
}
fn ask_entry(e: &Value, answered: Option<&str>) -> Value {
    let (kind, tool, detail) = (s(e, "kind"), s(e, "tool"), s(e, "detail"));
    let q = if kind == "approval" { if detail.is_empty() { tool.to_string() } else { format!("{tool}: {detail}") } } else { detail.to_string() };
    let mut ask = json!({"id":s(e,"id"),"state":if answered.is_some() {"answered"} else {"open"},"question":q,"options":ask_options(kind, s(e, "options"))});
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
fn row_entry(r: &crate::model::Row) -> Option<Value> {
    let at = ms(r.at);
    if r.kind == "msg" {
        let kind = match r.role.as_str() { "user" => "user", "assistant" => "assistant", _ => "note" };
        if kind == "assistant" && r.text.trim().is_empty() { return None }
        let mut en = json!({"id":r.id,"kind":kind,"text":if kind == "user" { shown_text(&r.text) } else { crate::settings::order::pay_shown(&r.text) },"at":at});
        if crate::present::thread::agent_only(&r.role, &r.text) { en["agentOnly"] = json!(true) }
        return Some(en);
    }
    act_entry(&r.id, &r.role, &r.akind, &r.text, at)
}
/// An activity as the phones see it, in the desktop's words (present/thread.rs): a tool call by its shown name, a design step by what changed, anything else a note
/// with its name and tone. A phone folds a run of `tool` and `step` entries (docs/PHONE-DATA.md); the design as it was before an edit is no entry at all.
fn act_entry(id: &str, role: &str, akind: &str, text: &str, at: u64) -> Option<Value> {
    use crate::present::thread as pt;
    let e = pt::Ent { id: id.into(), msg: false, role: role.into(), akind: akind.into(), text: text.into(), at };
    Some(match role {
        "tool" => json!({"id":id,"kind":"tool","text":text,"at":at,"tool":{"name":pt::shown_name(akind),"summary":if text == akind { String::new() } else { clip(text, 200) },"state":"done"}}),
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
    let m = hub.model().await;
    let asks = open_ask_counts(hub);
    let thread = thread_json(hub, &m, th, &asks).ok_or_else(|| err("unknown thread"))?;
    let before: u64 = s(p, "before").parse().unwrap_or(0);
    let page = m.page(&hub.store, th, before, limit_of(p, 40, 32)).map_err(err)?;
    let ids: HashSet<u64> = page.ids.iter().filter_map(|i| i.parse().ok()).collect();
    let mut rows: Vec<&crate::model::Row> = page.rows.iter().filter(|r| ids.contains(&r.ord) && r.thread == th).collect();
    rows.sort_by_key(|r| r.ord);
    let mut out: Vec<Value> = rows.iter().filter_map(|r| { let mut e = row_entry(r)?;
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
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
    let want = s(p, "path"); let path = std::fs::canonicalize(if want.is_empty() { home.as_str() } else { want }).map_err(|_| err("no such folder"))?;
    let mut names: Vec<String> = std::fs::read_dir(&path).map_err(|_| err("cannot read that folder"))?.flatten()
        .filter(|e| e.file_type().map_or(false, |t| t.is_dir()) || e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| !n.starts_with('.')).collect();
    names.sort_by_key(|n| n.to_lowercase()); names.truncate(1000);
    Ok(json!({"path":path.to_string_lossy(),"parent":path.parent().map(|x| x.to_string_lossy().into_owned()),"dirs":names}))
}

// ---- writes: the hub's own RPCs, run as this connection ----
async fn rpc(hub: &Arc<Hub>, c: &mut Conn, m: &str, p: Value) -> Result<Value, Value> {
    let out = hub.request_here(c, &json!({"id":0,"m":m,"p":p})).await;
    let r = out.into_iter().find(|v| v["t"] == "reply").ok_or_else(|| err("no answer"))?;
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
    use base64::Engine; use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt}; use std::io::Write;
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
    if !client_id.is_empty() { if !safe_id(client_id) { return Err(err("clientId: letters, digits and - _ . : only, 64 at most")) } if let Some(e) = recalled(client_id) { return Ok(e) } }
    if !hub.model().await.threads.get(thread).map_or(false, |t| !t.deleted) { return Err(err("unknown thread")) }
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
        "projects.list" => { let g = hub.model().await; let asks = open_ask_counts(hub);
            let items = g.projects.keys().filter_map(|i| project_json(&g, i, &asks).map(|v| (v["updatedAt"].as_u64().unwrap_or(0), i.clone(), v))).collect();
            let (projects, next) = page_of(items, limit_of(p, 50, 200), s(p, "before")); Ok(json!({"projects":projects,"next":next})) }
        "threads.list" => { let g = hub.model().await; let asks = open_ask_counts(hub); let (proj, arch) = (s(p, "project"), p["archived"] == true);
            let ids: Vec<String> = g.threads.iter().filter(|(_, t)| !t.deleted && t.archived == arch && (proj.is_empty() || t.project == proj)).map(|(i, _)| i.clone()).collect();
            let (mut items, lim) = (vec![], limit_of(p, 50, 200));
            // only the page's threads are built in full (the preview reads the log)
            let mut keyed: Vec<(u64, String)> = ids.iter().map(|i| (ms(g.threads[i].activity.max(g.threads[i].created)), i.clone())).collect();
            keyed.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            if let Some((a, i)) = s(p, "before").split_once(':') { if let Ok(a) = a.parse::<u64>() { keyed.retain(|x| x.0 < a || (x.0 == a && x.1.as_str() > i)) } }
            let more = keyed.len() > lim; keyed.truncate(lim);
            for (at, i) in &keyed { if let Some(v) = thread_json(hub, &g, i, &asks) { items.push((*at, i.clone(), v)) } }
            let next = if more { keyed.last().map(|x| json!(format!("{}:{}", x.0, x.1))).unwrap_or(Value::Null) } else { Value::Null };
            Ok(json!({"threads":items.into_iter().map(|x| x.2).collect::<Vec<_>>(),"next":next})) }
        "thread.entries" => entries(hub, p).await,
        // a buy thread's Order card (present/thread.rs `order_json`, what the desktop draws), null for any other thread; the app asks again on each push for the thread
        "thread.order" => { let th = s(p, "thread"); if th.is_empty() { return Err(err("thread")) }
            let o = crate::statehost::policies().and_then(|q| q.with_state(|st| crate::present::thread::order_json(st, th, st.threads.get(th).map_or(false, |t| t.turn == crate::state::Turn::Running)))).unwrap_or(Value::Null);
            Ok(json!({"order":o})) }
        "dirs.list" => dirs(p),
        "project.add" => {
            let path = s(p, "path"); if path.is_empty() { return Err(err("path")) }
            let want = std::fs::canonicalize(path).map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|_| path.to_string());
            rpc(hub, c, "project.add", json!({"path":path})).await?;
            for _ in 0..40 {
                { let g = hub.model().await; let asks = HashMap::new();
                  if let Some((id, _)) = g.projects.iter().find(|(_, x)| !x.removed && (x.root == want || x.root == path)) { if let Some(v) = project_json(&g, id, &asks) { return Ok(json!({"project":v})) } } }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(err("the project was not added"))
        }
        "thread.create" => {
            let project = s(p, "project"); if project.is_empty() { return Err(err("project")) }
            let mut q = json!({"project":project,"env":"local"});
            for k in ["title", "provider", "model"] { if !s(p, k).is_empty() { q[k] = p[k].clone() } }
            let r = rpc(hub, c, "thread.create", q).await?;
            let id = r["thread"].as_str().unwrap_or("").to_string(); if id.is_empty() { return Err(err("no thread was made")) }
            let runtime = if !s(p, "runtime").is_empty() { s(p, "runtime") } else if s(p, "permission") == "auto" { "full-access" } else { "approval-required" };
            rpc(hub, c, "thread.modes", json!({"thread":id,"runtime":runtime})).await?;
            if !s(p, "effort").is_empty() { rpc(hub, c, "thread.effort", json!({"thread":id,"effort":s(p, "effort")})).await?; }
            if !s(p, "text").is_empty() { send_message(hub, c, &id, s(p, "text"), "", &Value::Null).await?; }
            let g = hub.model().await; let asks = open_ask_counts(hub);
            thread_json(hub, &g, &id, &asks).map(|t| json!({"thread":t})).ok_or_else(|| err("no thread was made"))
        }
        "message.send" => Ok(json!({"entry":send_message(hub, c, s(p, "thread"), s(p, "text"), s(p, "clientId"), p.get("attachments").unwrap_or(&Value::Null)).await?})),
        "attachment.get" => attachment_get(hub, p),
        "ask.answer" => { rpc(hub, c, "ask.answer", json!({"id":s(p, "ask"),"answer":s(p, "answer")})).await?; Ok(json!({})) }
        "update.status" | "update.check" | "app.update" => rpc(hub, c, m, json!({})).await, // the same answer every client gets; the phones draw it
        "update.channel" => { let ch = s(p, "channel"); if ch != "stable" && ch != "beta" { return Err(err("no such channel")) } rpc(hub, c, "setting.set", json!({"key":"update.channel","value":ch})).await?; rpc(hub, c, "update.status", json!({})).await }
        "turn.interrupt" => { rpc(hub, c, "turn.interrupt", json!({"thread":s(p, "thread")})).await?; Ok(json!({})) }
        _ => match ext::call(hub, c, m, p).await { Some(r) => r, None => Err(err("unknown method")) },
    }
}

// ---- subscriptions ----
#[derive(Clone, PartialEq)]
enum Topic { Lists, Thread(String) }
#[derive(Default)]
struct Subs { next: u64, subs: Vec<(u64, Topic)>, live: HashSet<String> }
impl Subs {
    fn threads(&self, th: &str) -> Vec<u64> { self.subs.iter().filter(|(_, t)| *t == Topic::Thread(th.into())).map(|(i, _)| *i).collect() }
    fn lists(&self) -> Vec<u64> { self.subs.iter().filter(|(_, t)| *t == Topic::Lists).map(|(i, _)| *i).collect() }
}
fn d(sub: u64, k: &str, mut body: Value) -> Value { body["t"] = json!("d"); body["sub"] = json!(sub); body["k"] = json!(k); body }
fn turn_word(state: &str) -> &'static str { match state { "running" => "working", "failed" => "failed", _ => "idle" } }

/// The pushes for new log lines, for the lists and threads this connection subscribed to.
async fn deltas(hub: &Hub, lines: &[String], subs: &mut Subs) -> Vec<Value> {
    if subs.subs.is_empty() { return vec![] }
    let mut out = vec![]; let (mut upsert, mut gone, mut projects): (Vec<String>, Vec<String>, Vec<(String, bool)>) = (vec![], vec![], vec![]);
    for l in lines {
        let Ok(e) = serde_json::from_str::<Value>(l) else { continue };
        let opened = if s(&e, "$") == "AskClosed" { ask_opened(hub, s(&e, "id")) } else { None };
        let th = if matches!(s(&e, "$"), "ThreadCreated" | "ThreadRenamed" | "ThreadSettled" | "ThreadUnsettled" | "ThreadArchived" | "ThreadPinned" | "ThreadForked" | "ProviderSet" | "ModesSet" | "ThreadDeleted") { s(&e, "id") } else if let Some(o) = &opened { s(o, "thread") } else { s(&e, "thread") }.to_string();
        let mut entry: Option<Value> = None;
        match s(&e, "$") {
            "ThreadDeleted" => gone.push(th.clone()),
            "ProjectCreated" | "ProjectRenamed" => projects.push((s(&e, "id").to_string(), true)),
            "ProjectRemoved" => projects.push((s(&e, "id").to_string(), false)),
            "MessagePosted" => {
                let (id, role) = (s(&e, "msg"), s(&e, "role"));
                let kind = match role { "user" => "user", "assistant" => "assistant", _ => "note" };
                // as a page has it: a quote's Buy now link ready to open, and the hub's note to the agent marked so no phone shows it
                let mut en = json!({"id":id,"kind":kind,"text":if kind == "user" { shown_text(s(&e, "text")) } else { crate::settings::order::pay_shown(s(&e, "text")) },"at":ms(e["at"].as_u64().unwrap_or(0))});
                if crate::present::thread::agent_only(role, s(&e, "text")) { en["agentOnly"] = json!(true) }
                if let Some(a) = e.get("attachments") { en["attachments"] = a.clone() }
                if kind == "user" { if let Some(x) = recalled(id) { if let Some(c) = x.get("clientId") { en["clientId"] = c.clone() } } }
                let replaced = kind == "assistant" && subs.live.remove(&th);
                if replaced { en["replaces"] = json!(format!("live:{th}")) }
                if replaced || kind != "assistant" || !s(&e, "text").trim().is_empty() { entry = Some(en) }
            }
            "MessageAttached" => { let id = s(&e, "msg"); let mut en = json!({"id":id,"kind":"user","text":"","at":ms(e["at"].as_u64().unwrap_or(0)),"attachments":e["attachments"]}); if let Some(x) = recalled(id) { en["text"] = x["text"].clone(); if let Some(c) = x.get("clientId") { en["clientId"] = c.clone() } } entry = Some(en) }
            "ActivityLogged" => entry = act_entry(s(&e, "act"), s(&e, "tone"), s(&e, "kind"), s(&e, "summary"), ms(e["at"].as_u64().unwrap_or(0))),
            "AskOpened" => entry = Some(ask_entry(&e, None)),
            // the only alert a phone hears on its socket: what the uberbot's gate let through (push), and its clear
            "PushSent" => { for sub in subs.lists() { out.push(d(sub, "notify", ext::notify_of(&e))) } }
            "PushCleared" => { for sub in subs.lists() { out.push(d(sub, "clear", json!({"keys":e["keys"],"badge":e["badge"]}))) } }
            // the whole ask again, answered (an AskClosed names only the ask: its thread and question come from its AskOpened)
            "AskClosed" => { let id = s(&e, "id"); entry = Some(match &opened { Some(o) => ask_entry(o, Some(s(&e, "answer"))), None => json!({"id":format!("ask:{id}"),"kind":"ask","text":"","at":ms(e["at"].as_u64().unwrap_or(0)),"ask":{"id":id,"state":"answered","question":"","options":[],"answered":s(&e,"answer")}}) }); }
            "TurnChanged" => { for sub in subs.threads(&th) { out.push(d(sub, "turn", json!({"thread":th,"state":turn_word(s(&e, "state"))}))) } },
            _ => {}
        }
        if !th.is_empty() && !matches!(s(&e, "$"), "ThreadDeleted") && !th.starts_with("remind") { if !upsert.contains(&th) { upsert.push(th.clone()) } }
        if let Some(en) = entry { for sub in subs.threads(&th) { out.push(d(sub, "entry", json!({"thread":th,"entry":en}))) } }
    }
    if upsert.is_empty() && gone.is_empty() && projects.is_empty() { return out }
    let g = hub.model().await; let asks = open_ask_counts(hub);
    for th in &upsert { if let Some(t) = thread_json(hub, &g, th, &asks) { for sub in subs.lists().into_iter().chain(subs.threads(th)) { out.push(d(sub, "thread", json!({"thread":t}))) } } }
    for th in &gone { for sub in subs.lists().into_iter().chain(subs.threads(th)) { out.push(d(sub, "thread.removed", json!({"id":th}))) } }
    for (id, alive) in &projects { for sub in subs.lists() { match alive.then(|| project_json(&g, id, &asks)).flatten() { Some(p) => out.push(d(sub, "project", json!({"project":p}))), None => out.push(d(sub, "project.removed", json!({"id":id}))) } } }
    out
}
/// Streamed text of a running turn.
fn live_pushes(f: &Value, subs: &mut Subs) -> Vec<Value> {
    let th = s(f, "thread").to_string(); let mut out = vec![];
    match s(f, "t") {
        "delta" => { let first = subs.live.insert(th.clone()); let text = s(f, "text");
            for sub in subs.threads(&th) {
                if first { out.push(d(sub, "entry", json!({"thread":th,"entry":live_entry(&th, "")}))) }
                out.push(d(sub, "text", json!({"thread":th,"entry":format!("live:{th}"),"append":text,"at":now_ms()})));
            } }
        _ => {}
    }
    out
}

async fn handle(hub: &Arc<Hub>, c: &mut Conn, subs: &mut Subs, v: &Value) -> Value {
    let (id, m, p) = (v["id"].clone(), s(v, "m"), v.get("p").cloned().unwrap_or(Value::Null));
    let res = match m {
        "subscribe" => match (s(&p, "topic"), s(&p, "thread")) {
            ("lists", _) => { subs.next += 1; subs.subs.push((subs.next, Topic::Lists)); Ok(json!({"sub":subs.next})) }
            ("thread", th) if hub.model().await.threads.get(th).map_or(false, |t| !t.deleted) => { subs.next += 1; subs.subs.push((subs.next, Topic::Thread(th.into()))); Ok(json!({"sub":subs.next})) }
            ("thread", _) => Err(err("unknown thread")),
            _ => Err(err("topic: lists or thread")),
        },
        "unsubscribe" => { let n = p["sub"].as_u64().unwrap_or(0); subs.subs.retain(|(i, _)| *i != n); Ok(json!({})) }
        _ => call(hub, c, m, &p).await,
    };
    match res { Ok(r) => json!({"t":"reply","id":id,"ok":true,"result":r}), Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}) }
}

pub async fn socket(hub: Arc<Hub>, s: TcpStream, json_mode: bool, token: String, host: String) -> Result<()> {
    let mut cfg = WebSocketConfig::default(); cfg.max_message_size = Some(40 << 20); cfg.max_frame_size = Some(40 << 20); // (images come as base64 in one JSON frame: 4 x 8 MB)
    let (mut tx, mut rx) = tokio_tungstenite::accept_async_with_config(s, Some(cfg)).await?.split();
    let (mut conn, _) = hub.connect(true);
    let mut outbox = hub.attach(conn.client);
    struct Detach(Arc<Hub>, u64); impl Drop for Detach { fn drop(&mut self) { self.0.detach(self.1) } }
    let _detach = Detach(hub.clone(), conn.client);
    if !hub.admit_blocking(conn.client, token).await { return Ok(()) }
    tx.send(pack(&json!({"t":"hello","proto":PROTO,"hub":host,"version":hub.cfg.version,"epoch":hub.epoch,"now":now_ms(),"features":FEATURES,"notice":crate::guard::notice(&crate::guard::status(&hub.cfg.home))}), json_mode)).await?;
    let (mut changes, mut frames) = (hub.changed.subscribe(), hub.frames.subscribe());
    let mut seen = hub.store.len(); let mut subs = Subs::default();
    let mut beat = tokio::time::interval(Duration::from_millis(15000)); beat.tick().await;
    loop {
        tokio::select! {
            m = rx.next() => {
                let Some(m) = m else { return Ok(()) }; let m = m?;
                if matches!(m, Message::Close(_)) { return Ok(()) }
                let v = match wire::decode(&m) { None => continue, Some(Err(r)) => { tx.send(pack(&r, json_mode)).await?; continue } Some(Ok(v)) => v };
                let r = handle(&hub, &mut conn, &mut subs, &v).await; tx.send(pack(&r, json_mode)).await?;
            }
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
                    Out::Value(v) => if ext::wants_value(&v) {
                        // a plot's "none" is binary like the plots (one decoder in the app); a browser frame's announcement is JSON
                        if v["t"] == "plot" { let mut b = Vec::new(); cbor_plain(&v, &mut b); tx.send(Message::Binary(b.into())).await?; } else { tx.send(pack(&v, json_mode)).await?; } } } }
            _ = beat.tick() => { tx.send(pack(&json!({"t":"hb"}), json_mode)).await?; }
        }
    }
}

#[cfg(test)]
mod tests;

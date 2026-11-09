//! Views: what a native client asks for with `view.open`, `view.update`, `view.action` and `view.page`, and how the
//! reply is made (window from the Rust index, picture from the presenter, keyed delta, segmentation). Lane
//! parity-session owns this directory. Methods the core does not answer itself (`view.search`, `view.message`,
//! `view.cancel`) are registered here with `Hub::register_view`; `start` is called once from `main`.
use crate::{api::Fut, hub::{error, now_ms, Conn, Hub}};
use serde_json::{json, Map, Value};
use std::sync::Arc;

/// One view method family beyond the core's open/update/action/page. `call` runs after the core has validated
/// the request's identity (`version`, `generation`, `epoch`, `hub`, `requestRevision`, the target) and the view
/// (`viewId`) for every method except `view.open`; it returns the frames to send back.
pub trait ViewMethod: Send + Sync {
    fn methods(&self) -> &'static [&'static str];
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, conn: &'a mut Conn, id: &'a Value, method: &'a str, params: &'a Value) -> Fut<'a, Vec<Value>>;
}

mod actor;
pub mod search;
#[allow(dead_code)] pub mod composer;
#[allow(dead_code)] pub mod dispatch;
#[allow(dead_code)] pub mod patch;
#[allow(dead_code)] pub mod select;
#[allow(dead_code)] pub mod slash;
pub mod thread_actions;
pub mod pages;
pub mod event_type;
pub use event_type::event_type;
pub use actor::{forget, is_job, record_answer, record_read, record_read_bound, record_reply, record_reply_bound, record_reply_bound_m, replied, replied_late, take_reads};
/// Registers this lane's handlers: `thread.search` (search.rs), what was said in every thread or in one.
pub fn start(hub: &Arc<Hub>) { hub.register_rpc(Arc::new(search::Search)); }

pub const REPLY_BYTES: usize = 262144;
pub const CONTROL_CHARS: usize = 32768 / 6; // core Part.take(text, 32768): left / 6 characters a part
pub const CONTROL_ID_FLOOR: u64 = 1 << 47;   // Wire.control.id: view request ids are above 2^47

fn empty_window() -> Value { json!({"contextMessages":[],"messageIds":[],"historyUnits":[],"partial":[],"ids":[],"anchor":{"id":"","offset":0},"older":"","olderNext":"","hasMore":false,"remaining":0,"newer":0,"total":0}) }
fn within(s: &str, n: usize) -> bool { s.len() <= n }
/// view.page's cursor ({version, epoch, target, query, before}, as an object or its JSON text): the ordinal to page before.
fn page_before(p: &Value) -> u64 {
    let c = match &p["cursor"] { Value::String(t) => serde_json::from_str::<Value>(t).unwrap_or(Value::Null), v => v.clone() };
    match &c["before"] { Value::String(t) => t.parse().unwrap_or(0), v => v.as_u64().unwrap_or(0) }
}

/// The page as the presenter takes it: the thread's rows as strings (10 cells each) and the counts.
fn page_json(thread: &str, before: u64, pg: &crate::model::Page) -> Value {
    let rows: Vec<Value> = pg.rows.iter().map(|r| Value::Array(r.json().as_array().unwrap().iter().map(|c| Value::String(match c { Value::String(t) => t.clone(), v => v.to_string() })).collect())).collect();
    json!({"thread":thread,"before":before,"below":pg.below,"above":pg.above,"rows":rows})
}

fn target_valid(t: &Value) -> bool {
    let Some(o) = t.as_object() else { return false };
    if o.len() != 2 { return false }
    let (Some(k), Some(i)) = (o.get("kind").and_then(Value::as_str), o.get("id").and_then(Value::as_str)) else { return false };
    within(i, 1024) && ((k == "recent" && i.is_empty()) || (!i.is_empty() && matches!(k, "project" | "thread" | "bot" | "room")))
}

/// The fields of the presenter's params only the hub makes: the thread's `window` and `page` (rows read from the index), `reads` and `local`. A client's own copy would
/// be taken for the hub's (an unported native page for indexed rows), so every one is removed from the request's clone before the hub's go in.
fn strip_hub_owned(params: &mut Value) { if let Some(o) = params.as_object_mut() { for k in ["window", "page", "reads", "local", "wants", "botPage", "roomPage", "subLog"] { o.remove(k); } } }

/// PRESENTER_UNAVAILABLE is retryable on the wire: a target the Rust presenter has not ported (the helper is not running) or a helper that is restarting answers "not yet", never "no"
/// (Hub::error has retryable:false for every code).
fn unavailable(id: &Value) -> Value { json!({"t":"reply","id":id,"ok":false,"error":{"code":"PRESENTER_UNAVAILABLE","retryable":true,"compatibilityRequired":false}}) }

/// The error a policy reply stands for: `refused` (page action not offered / token not held) is INVALID_REQUEST, `stale` (the target does not exist) STALE_VIEW.
fn policy_error(pol: &Value) -> Option<&'static str> {
    if pol.get("refused") == Some(&json!(true)) { Some("INVALID_REQUEST") } else if pol.get("stale") == Some(&json!(true)) { Some("STALE_VIEW") } else { None }
}

/// Hub.autosettle.read_view: only a rendered `view.open` of a thread that is not a mirror marks its newest reply read (view.update polls,
/// view.page history pages and every other request do not).
fn acknowledges(method: &str, mirror: bool) -> bool { method == "view.open" && !mirror }

impl Hub {
    /// The view branch of `Hub::request`: `m` starts with "view.".
    pub(crate) async fn view_request(&self, c: &mut Conn, id: Value, m: &str, msg: &Value) -> Vec<Value> {
        let p = msg.get("p").cloned().unwrap_or(Value::Null);
        let rid = id.as_u64().unwrap_or(0);
        let hubname = p.get("hub").and_then(Value::as_str).unwrap_or("");
        let valid = rid > CONTROL_ID_FLOOR && p.get("version") == Some(&json!(1))
            && p.get("generation").and_then(Value::as_str) == Some(c.gen.as_str()) && p.get("epoch").and_then(Value::as_str) == Some(self.epoch.as_str())
            && !hubname.is_empty() && within(hubname, 1024)
            && p.get("requestRevision").and_then(Value::as_u64).map(|n| n > 0 && n <= u32::MAX as u64).unwrap_or(false)
            && target_valid(p.get("target").unwrap_or(&Value::Null));
        if !valid { return vec![error(&id, "STALE_CONNECTION")] }
        match m {
            "view.open" | "view.update" | "view.action" | "view.page" => {}
            "view.search" | "view.message" | "view.cancel" => {}
            _ => return vec![error(&id, "INVALID_REQUEST")],
        }
        if m != "view.open" && p.get("viewId").and_then(Value::as_str) != Some(c.view_id.as_str()) { return vec![error(&id, "STALE_VIEW")] }
        // the methods beyond open/update/action/page belong to the handlers registered with `Hub::register_view`
        if matches!(m, "view.search" | "view.message" | "view.cancel") {
            return match self.view_for(m) { Some(h) => { let me = self.arc(); h.call(&me, c, &id, m, &p).await } None => vec![error(&id, "UNSUPPORTED")] };
        }
        let view_id = if m == "view.open" { format!("{}:{}", c.gen, rid) } else { c.view_id.clone() };
        let rev = self.revision().await;
        let mut params = p.clone();
        strip_hub_owned(&mut params);   // window, page, reads and local are the hub's: whatever the client sent is gone before the hub's own are put in
        if let Some(o) = params.as_object_mut() {
            // what the presenter needs of the hub: the view's id, the hub's signal revision (SignalRevision: the context's metadataRevision;
            // saved sidebar pages are kept only while it, the view and the choices stay the same: activity lines and assistant text do not move it), this run's key
            o.insert("revision".into(), json!(self.signal_seq())); o.insert("viewId".into(), json!(view_id)); o.insert("nativeKey".into(), json!(self.native_key)); o.insert("now".into(), json!(now_ms()));
        }
        // a thread's window: the page out of the Rust index, the fold/context logic in native
        // view.page: the cursor names the page (`before`, an ordinal); the reply is a view.open-shaped one
        // a bot target's owner is its chat thread, which the bots lane knows (U.Bots.chat.of)
        let bot = match (p["target"]["kind"].as_str(), p["target"]["id"].as_str()) {
            (Some("bot"), Some(id)) => { let me = self.arc(); crate::bots::chat_thread(&me, id).await.map(|t| (id.to_string(), t)) }
            _ => None,
        };
        actor::set_bot(c.client, bot.clone());
        // the bot's page (the presenter holds neither memories nor routines): the tab the action picks, else the one the page's route names
        if let (Some((id, _)), Some(o)) = (&bot, params.as_object_mut()) {
            let tab = if p["action"] == "bot-tab" { p["value"].as_str().unwrap_or("").to_string() } else { actor::route_tab(&p).unwrap_or_default() };
            let from = if p["action"] == "bot-memory" { p["value"].as_str() } else if p["action"] == "bot-tab" { None } else { p["session"]["memoryFrom"].as_str() }.and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            let tab = if p["action"] == "bot-memory" { "memory".to_string() } else { tab };
            let me = self.arc(); if let Some(pg) = crate::bots::bot_page_from(&me, id, &tab, now_ms() / 1000, from).await { o.insert("botPage".into(), pg); }
        }
        if let (("room", id), Some(o)) = ((p["target"]["kind"].as_str().unwrap_or(""), p["target"]["id"].as_str().unwrap_or("")), params.as_object_mut()) {
            let me = self.arc(); if let Some(pg) = crate::bots::room_page(&me, id, now_ms() / 1000).await { o.insert("roomPage".into(), pg); }
        }
        let before = actor::page(c.client, &view_id, m, p["action"].as_str().unwrap_or(""), &p["target"], if m == "view.page" { page_before(&p) } else { 0 });
        // the conversation the target shows: a thread's own, a bot's chat thread (a bot view without its window stays on "Updating conversation...")
        let shown: Option<String> = match p["target"]["kind"].as_str() { Some("thread") => p["target"]["id"].as_str().map(str::to_string), Some("bot") => bot.as_ref().map(|(_, t)| t.clone()), _ => None };
        let (window, page) = match self.window_for(&p, before, shown.as_deref()).await { Some((w, pg)) => (Some(w), Some(pg)), None => (None, None) };
        if let (Some(w), Some(o)) = (&window, params.as_object_mut()) { o.insert("window".into(), w.clone()); }
        if let (Some(pg), Some(o)) = (&page, params.as_object_mut()) { o.insert("page".into(), page_json(shown.as_deref().unwrap_or(""), before, pg)); }
        // the thread's subagent log, for the Subagents panel's rows and their drill-down (present/panels.rs `subs_at`)
        if let (Some(th), true) = (shown.as_deref(), page.is_some()) { let log = self.sub_log_for(th).await; if let Some(o) = params.as_object_mut() { o.insert("subLog".into(), log); } }
        if let (Some(o), true) = (params.as_object_mut(), p.get("session").map_or(false, Value::is_object)) { o.insert("session".into(), actor::input(c.client, &view_id, &p)); }
        { let reads = actor::take_reads(c.client, &view_id, &p["target"]); if let Some(o) = params.as_object_mut() { o.remove("reads"); if !reads.is_empty() { o.insert("reads".into(), Value::Array(reads)); } } }   // only the hub says what it read
        actor::set_local(&mut params, actor::local_in(c.client, &view_id, m, &p["target"]));
        let mut pol = match self.presenter.view(&params).await { Ok(v) => v, Err(e) => { eprintln!("view unavailable: target {} action {:?}: {e:#}", params["target"]["kind"].as_str().unwrap_or("?"), params["action"].as_str().unwrap_or("")); return vec![unavailable(&id)] } }; // (content-free: kind, action and the presenter's reason)
        // the policy's verdicts on the request: a refused native-sidebar-page action (Hub.thin.act.native) and a target that does not exist
        // (an unknown bot, room or project: STALE_VIEW), answered before anything is stored
        if let Some(code) = policy_error(&pol) { return vec![error(&id, code)] }
        if let Some(o) = pol.as_object_mut() { o.remove("refused"); o.remove("stale"); }
        actor::local_out(c.client, &view_id, &p["target"], &mut pol);
        let mut pol = actor::apply(c.client, &view_id, &p, &pol);
        actor::mask_page_reply(&p, &mut pol);
        let target = p["target"].clone();
        if m == "view.open" { c.view_id = view_id.clone(); c.target = target.clone(); }
        c.lastrev = rev;
        if m == "view.open" { c.sent.clear(); }
        let mut result = self.assemble(&p, &view_id, rev, &pol);
        if let (Some(w), Some(o)) = (window, result.as_object_mut()) { if o.contains_key("window") { o.insert("window".into(), w); } }
        let result = Self::delta_reply(c, &p, result);
        // Hub.autosettle.read_view: runs after any rendered view.open (its context's anchor is always 0, `Hub.thin.dispatch` kind 0, so the request's
        // own `anchor` plays no part); errors never get here. Only a mirror (hub.far().has) is left out. The policies decide (0 or 1 SettingSet
        // autosettle.read.<t>); committed in a spawned task after the answer is built.
        if m == "view.open" {
            let th = match p["target"]["kind"].as_str() { Some("thread") => p["target"]["id"].as_str().map(str::to_string), Some("bot") => bot.as_ref().map(|(_, t)| t.clone()), _ => None };
            if let (Some(th), Some(pol)) = (th.filter(|t| !t.is_empty() && acknowledges(m, self.far().map_or(false, |f| f.has(t)))), crate::statehost::policies()) {
                let me = self.arc();
                tokio::spawn(async move { let evs = pol.read(&th, now_ms() / 1000).await; if !evs.is_empty() { let _ = me.commit_many(&evs).await; } });
            }
        }
        let answer = json!({"t":"reply","id":id,"ok":true,"result":result});
        let text = serde_json::to_string(&answer).unwrap_or_default();
        if text.len() <= REPLY_BYTES { return vec![answer] }
        if p.get("viewControl") != Some(&json!(1)) { return vec![answer] } // as the JS hub: sent whole, the client refuses it
        // segment: everything but `shown` travels as view.control parts, then the picture
        let mut rest = Map::new();
        for (k, v) in answer["result"].as_object().unwrap() { if k != "shown" { rest.insert(k.clone(), v.clone()); } }
        let control = serde_json::to_string(&Value::Object(rest)).unwrap_or_default();
        let chars: Vec<char> = control.chars().collect();
        let n = chars.chunks(CONTROL_CHARS).count();
        let mut out: Vec<Value> = chars.chunks(CONTROL_CHARS).enumerate()
            .map(|(i, part)| json!({"t":"view.control","id":id,"part":i,"last":i + 1 == n,"text":part.iter().collect::<String>()})).collect();
        let mut picture = Map::new();
        if let Some(s) = answer["result"].get("shown") { picture.insert("shown".into(), s.clone()); }
        picture.insert("controlId".into(), id.clone());
        out.push(json!({"t":"reply","id":id,"ok":true,"result":picture}));
        out
    }

    /// The window object for a thread target (newest page); None for other targets or without program.
    /// `thread` is the conversation the target shows (V.Target.thread): a thread's own id, a bot's chat thread; none for the other targets.
    /// A thread's subagent log (`@sub:<thread>`, its newest SUB_WINDOW entries) as present/panels.rs's `sub_log`, with this machine's name: `{host, calls}`, null when
    /// the thread has none (a thread of another machine has none here). Read again only when the log grew, so a picture of a busy thread does not re-read it.
    async fn sub_log_for(&self, thread: &str) -> Value {
        static CACHE: std::sync::Mutex<Option<std::collections::HashMap<String, (usize, Value)>>> = std::sync::Mutex::new(None);
        static HOST: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
        let key = crate::agents::bg::log(thread);
        let calls = {
            let m = self.model.lock().await; let n = m.count(&key);
            if n == 0 { return Value::Null }
            let held = CACHE.lock().unwrap().as_ref().and_then(|c| c.get(thread)).filter(|(k, _)| *k == n).map(|(_, v)| v.clone());
            match held { Some(v) => v, None => {
                let rows: Vec<(String, String, String, u64)> = m.tail(&self.store, &key, crate::present::panels::SUB_WINDOW).into_iter().map(|r| (r.role, r.akind, r.text, r.at)).collect();
                drop(m);
                let v = crate::present::panels::sub_log(&rows);
                let mut c = CACHE.lock().unwrap(); let c = c.get_or_insert_with(Default::default); if c.len() > 64 { c.clear() } c.insert(thread.to_string(), (n, v.clone())); v
            } }
        };
        let host = HOST.get_or_init(|| async { crate::net::botnet::name().await }).await;
        json!({"host": host, "calls": calls})
    }

    async fn window_for(&self, p: &Value, before: u64, thread: Option<&str>) -> Option<(Value, crate::model::Page)> {
        let t = p.get("target")?;
        let thread = thread?;
        // a thread of a linked machine: its page and context come from the mirror (net/farstore.rs), not from this hub's model
        let far = self.far().filter(|f| f.has(thread));
        if far.is_none() && !self.model.lock().await.threads.contains_key(thread) { return None }
        let page = match &far { Some(f) => f.page(thread, before, 32)?, None => { let m = self.model.lock().await; m.page(&self.store, thread, before, 32).ok()? } };
        let mut req = Map::new(); req.insert("target".into(), t.clone());
        if let Some(a) = p.get("anchor") { req.insert("anchor".into(), a.clone()); }
        let req = Value::Object(req);
        // an older page still names the thread's latest human prompt and assistant text
        let ctx = if before == 0 { vec![] } else if let Some(f) = &far { f.latest_context(thread) } else { let m = self.model.lock().await; m.latest_context(&self.store, thread).unwrap_or_default() };
        // the Rust window first (present::window), computed here because the model lock is async and a presenter Screen cannot take it
        if let Some(w) = crate::present::window::window(&page, &ctx, &self.epoch, before, &req) { return Some((w, page)) }
        // what Rust does not answer (None) is the helper's, and only while the policy helper runs: the window helper is a child process the policy's V needs. A hub without the helper
        // starts nothing for any target, whatever BACKPLANE_WINDOW_BIN says (wants_log is false while the helper is not running): the native refusal stands
        if !self.presenter.wants_log() { return None }
        let bin = self.window_bin.get()?;
        let w = crate::window::compute(bin, &std::env::temp_dir(), thread, &self.epoch, before, &page, &ctx, &req).await?;
        Some((w, page))
    }

    /// Vd.output: a client that offered {viewDelta:{version:1, token}} holds the body sent under that token; it gets
    /// the ops from it ({"viewDelta":{version, base, token, ops}}) in place of the body's fields, else the whole body
    /// with a token to name it. The last three bodies of the view are kept.
    fn delta_reply(c: &mut Conn, p: &Value, result: Value) -> Value {
        let offered = p["viewDelta"]["version"] == json!(1);
        let rev = p["requestRevision"].as_u64().unwrap_or(0);
        if !(offered && result["remoteReady"] == json!(true) && result.get("shown").is_some() && result.get("nextTarget").is_none() && rev > 0) { return result }
        let token = rev.to_string();
        let base = p["viewDelta"]["token"].as_str().unwrap_or("").to_string();
        let mut body = Map::new();
        for k in ["shown", "session", "window", "overlay"] { if let Some(v) = result.get(k) { body.insert(k.into(), v.clone()); } }
        let body = Value::Object(body);
        let draft = p["session"]["drafts"][p["session"]["sel"].as_str().unwrap_or("")].as_str().unwrap_or("").to_string();
        let old = if base.is_empty() { None } else { c.sent.iter().find(|(t, _)| *t == base).map(|(_, b)| b) };
        let mut meta = Map::new(); meta.insert("version".into(), json!(1));
        let strip = |r: &Value| { let mut o = Map::new(); for (k, v) in r.as_object().unwrap() { if !["shown", "session", "window", "overlay"].contains(&k.as_str()) { o.insert(k.clone(), v.clone()); } } o };
        let out = match old.and_then(|b| crate::delta::diff(b, &body)) {
            Some(ops) => { let mut o = strip(&result); meta.insert("base".into(), json!(base)); meta.insert("token".into(), json!(token)); meta.insert("ops".into(), crate::delta::ops_json(&ops, &draft)); o.insert("viewDelta".into(), Value::Object(meta)); Value::Object(o) }
            None if crate::delta::draft_is_long(&draft) => { let mut o = strip(&result); meta.insert("base".into(), json!("")); meta.insert("token".into(), json!(token)); meta.insert("ops".into(), crate::delta::whole_ops(&body, &draft)); o.insert("viewDelta".into(), Value::Object(meta)); Value::Object(o) }
            None => { let mut o = result.as_object().unwrap().clone(); meta.insert("base".into(), json!("")); meta.insert("token".into(), json!(token)); o.insert("viewDelta".into(), Value::Object(meta)); Value::Object(o) }
        };
        c.sent.insert(0, (token, body)); c.sent.truncate(3);
        out
    }

    /// Hub.thin.result: identity fields in front, in the order the JS hub gives them.
    fn assemble(&self, p: &Value, view_id: &str, rev: u64, pol: &Value) -> Value {
        let mut o = Map::new();
        o.insert("remoteReady".into(), json!(pol.get("remoteReady").and_then(Value::as_bool).unwrap_or(false)));
        o.insert("viewRevision".into(), json!(rev.to_string()));
        o.insert("renderer".into(), json!("mobile"));
        o.insert("viewId".into(), json!(view_id));
        o.insert("target".into(), p["target"].clone());
        o.insert("requestRevision".into(), p["requestRevision"].clone());
        o.insert("epoch".into(), p["epoch"].clone());
        o.insert("generation".into(), p["generation"].clone());
        o.insert("hub".into(), p["hub"].clone());
        o.insert("version".into(), json!(1));
        o.insert("mailDraftContext".into(), Value::Null);
        if pol.get("remoteReady") == Some(&json!(false)) {
            if let Some(n) = pol.get("nextTarget") { o.insert("nextTarget".into(), n.clone()); }
            o.insert("session".into(), pol.get("session").cloned().unwrap_or(Value::Null));
            o.insert("cmds".into(), pol.get("cmds").cloned().unwrap_or(json!([])));
        } else {
            o.insert("window".into(), empty_window());
            o.insert("cmds".into(), pol.get("cmds").cloned().unwrap_or(json!([])));
            o.insert("session".into(), pol.get("session").cloned().unwrap_or(Value::Null));
            o.insert("shown".into(), crate::wire::shown_with_side_by_side(pol.get("shown").cloned().unwrap_or(Value::Null)));
            if let Some(ov) = pol.get("overlay") { o.insert("overlay".into(), ov.clone()); }
        }
        Value::Object(o)
    }
}

#[cfg(test)]
mod nohelper_tests {
    use super::*;
    // the person's rule: a hub with no helper starts no process for a thread view, BACKPLANE_WINDOW_BIN or not
    #[tokio::test] async fn without_the_helper_window_for_starts_nothing() {
        let home = std::env::temp_dir().join(format!("bp-nohelper-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("events.jsonl"), "{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"x\",\"root\":\"/tmp\",\"at\":1}\n{\"$\":\"ThreadCreated\",\"id\":\"t1\",\"project\":\"p1\",\"title\":\"T\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":1}\n").unwrap();
        let spy = home.join("spy-ran"); let script = home.join("window-spy.sh");
        std::fs::write(&script, format!("#!/bin/sh\ntouch {}\n", crate::plat::fs::script_path(&spy))).unwrap();
        { use crate::plat::fs::PermissionsExt; std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap(); }
        let cfg = crate::hub::Cfg { home: home.clone(), version: "0.1.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
        let h = Hub::start(cfg, std::sync::Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
        let _ = h.window_bin.set(script);
        let out = h.window_for(&json!({"target": {"kind": "thread", "id": "t1"}}), 0, Some("t1")).await;
        // the Rust window may answer (that is the point: a thread view without the helper); what matters is that nothing was started for it
        let _ = out;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(!spy.exists(), "the window helper must never have been started");
        let _ = std::fs::remove_dir_all(&home);
    }
}

#[cfg(test)]
mod read_tests {
    use super::*;
    #[test] fn a_log_line_has_its_type_whatever_the_spacing() {
        assert_eq!(event_type(r#"{"$":"ThreadCreated","id":"t1"}"#), Some("ThreadCreated"));
        assert_eq!(event_type(r#"{"$": "ThreadCreated", "id": "t1"}"#), Some("ThreadCreated"));
        assert_eq!(event_type(r#"  { "$" : "ProjectCreated" , "id":"p"}"#), Some("ProjectCreated"));
        assert_eq!(event_type(r#"{"id":"t1","$":"ThreadCreated"}"#), None, "the type is the first field, as the hub writes it");
        assert_eq!(event_type("not json"), None);
        assert_eq!(event_type(r#"{"$":5}"#), None);
        assert_eq!(event_type(""), None);
    }
    #[test] fn client_supplied_hub_fields_are_stripped() {
        let mut p = json!({"target": {"kind": "thread", "id": "t1"}, "window": {"ids": ["x"]}, "page": {"rows": []}, "reads": [1], "local": {"a": "b"}, "wants": [2], "session": {}});
        strip_hub_owned(&mut p);
        assert_eq!(p, json!({"target": {"kind": "thread", "id": "t1"}, "session": {}}));
    }
    #[test] fn an_unavailable_presenter_is_retryable_and_the_others_are_not() {
        let e = unavailable(&json!(5));
        assert_eq!((e["error"]["code"].as_str(), e["error"]["retryable"].as_bool(), e["id"].as_u64()), (Some("PRESENTER_UNAVAILABLE"), Some(true), Some(5)));
        assert_eq!(error(&json!(5), "STALE_VIEW")["error"]["retryable"], json!(false));
    }
    #[test] fn the_policys_verdicts_become_errors() {
        assert_eq!(policy_error(&json!({"refused": true})), Some("INVALID_REQUEST"));
        assert_eq!(policy_error(&json!({"stale": true})), Some("STALE_VIEW"));
        assert_eq!(policy_error(&json!({"remoteReady": true, "stale": false})), None);
        assert_eq!(policy_error(&json!({"remoteReady": true})), None);
    }
    // Hub.autosettle.read_view: view.open reads; view.update and view.page do not; a mirror does not
    #[test] fn only_a_view_open_of_a_local_thread_acknowledges() {
        assert!(acknowledges("view.open", false));
        assert!(!acknowledges("view.open", true));
        for m in ["view.update", "view.page", "view.action", "view.search", "view.message"] { assert!(!acknowledges(m, false), "{m}") }
    }
}

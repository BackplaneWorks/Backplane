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

/// Registers this lane's handlers. Nothing yet.
mod actor;
pub use actor::forget;
pub fn start(_hub: &Arc<Hub>) {}

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
        if let Some(o) = params.as_object_mut() {
            // what the presenter needs of the hub (native-sidebar.bend Ns.prepare): the view's id, the request's revision, this run's key
            o.insert("revision".into(), p.get("requestRevision").cloned().unwrap_or(json!(0))); o.insert("viewId".into(), json!(view_id)); o.insert("nativeKey".into(), json!(self.native_key)); o.insert("now".into(), json!(now_ms()));
        }
        // a thread's window: the page out of the Rust index, the fold/context logic in native Bend
        // view.page: the cursor names the page (`before`, an ordinal); the reply is a view.open-shaped one
        let before = actor::page(c.client, &view_id, m, p["action"].as_str().unwrap_or(""), &p["target"], if m == "view.page" { page_before(&p) } else { 0 });
        let (window, page) = match self.window_for(&p, before).await { Some((w, pg)) => (Some(w), Some(pg)), None => (None, None) };
        if let (Some(w), Some(o)) = (&window, params.as_object_mut()) { o.insert("window".into(), w.clone()); }
        if let (Some(pg), Some(o)) = (&page, params.as_object_mut()) { o.insert("page".into(), page_json(p["target"]["id"].as_str().unwrap_or(""), before, pg)); }
        actor::set_local(&mut params, actor::local_in(c.client, &view_id, m, &p["target"]));
        let mut pol = match self.presenter.view(&params).await { Ok(v) => v, Err(_) => return vec![error(&id, "PRESENTER_UNAVAILABLE")] };
        actor::local_out(c.client, &view_id, &p["target"], &mut pol);
        let pol = actor::apply(c.client, &view_id, &p, &pol);
        let target = p["target"].clone();
        if m == "view.open" { c.view_id = view_id.clone(); c.target = target.clone(); }
        c.lastrev = rev;
        if m == "view.open" { c.sent.clear(); }
        let mut result = self.assemble(&p, &view_id, rev, &pol);
        if let (Some(w), Some(o)) = (window, result.as_object_mut()) { if o.contains_key("window") { o.insert("window".into(), w); } }
        let result = Self::delta_reply(c, &p, result);
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

    /// The window object for a thread target (newest page); None for other targets or without the Bend program.
    async fn window_for(&self, p: &Value, before: u64) -> Option<(Value, crate::model::Page)> {
        let bin = self.window_bin.get()?;
        let t = p.get("target")?;
        if t["kind"] != "thread" { return None }
        let thread = t["id"].as_str()?;
        if !self.model.lock().await.threads.contains_key(thread) { return None }
        let page = { let m = self.model.lock().await; m.page(&self.store, thread, before, 32).ok()? };
        let mut req = Map::new(); req.insert("target".into(), t.clone());
        if let Some(a) = p.get("anchor") { req.insert("anchor".into(), a.clone()); }
        // an older page still names the thread's latest human prompt and assistant text (message-context.bend)
        let ctx = if before == 0 { vec![] } else { let m = self.model.lock().await; m.latest_context(&self.store, thread).unwrap_or_default() };
        let w = crate::window::compute(bin, &std::env::temp_dir(), thread, &self.epoch, before, &page, &ctx, &Value::Object(req)).await?;
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
            o.insert("shown".into(), pol.get("shown").cloned().unwrap_or(Value::Null));
            if let Some(ov) = pol.get("overlay") { o.insert("overlay".into(), ov.clone()); }
        }
        Value::Object(o)
    }
}

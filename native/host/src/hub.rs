//! The hub core for native clients: connection identity, view requests, reply assembly,
//! segmentation, change notices. State is what the event log folds to; the picture comes from
//! the presenter. (window/overlay for phones arrive with M2's windowed history.)
use crate::{api::{Caller, HttpHandler, RpcHandler}, auth::Auth, model::Model, presenter::Presenter, store::Store};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use std::sync::{atomic::{AtomicU64, Ordering::Relaxed}, Arc};
use tokio::sync::{broadcast, Mutex};

pub const REPLY_BYTES: usize = 262144;
pub const CONTROL_CHARS: usize = 32768 / 6; // core Part.take(text, 32768): left / 6 characters a part
pub const CONTROL_ID_FLOOR: u64 = 1 << 47;   // Wire.control.id: view request ids are above 2^47

pub struct Cfg { pub home: PathBuf, pub version: String, pub commit: String, pub listen: String, pub os: String, pub arch: String }

pub struct Hub {
    pub cfg: Cfg, pub auth: Auth, pub epoch: String, pub presenter: Arc<dyn Presenter>,
    pub store: Store, pub model: Mutex<Model>, gen: AtomicU64, pub changed: broadcast::Sender<u64>, pub frames: broadcast::Sender<Value>, pub origin: String,
    commits: tokio::sync::mpsc::UnboundedSender<Commit>,
    rpc: std::sync::RwLock<Vec<(&'static str, Arc<dyn RpcHandler>)>>, http: std::sync::RwLock<Vec<(&'static str, Arc<dyn HttpHandler>)>>,
    live: std::sync::Mutex<std::collections::HashMap<String, String>>,
    me: std::sync::OnceLock<std::sync::Weak<Hub>>, next_client: AtomicU64,
    outboxes: std::sync::Mutex<std::collections::HashMap<u64, tokio::sync::mpsc::UnboundedSender<Out>>>,
}

/// One item of a connection's outbox: a value the hub encodes (CBOR with the dictionaries), or bytes that go out as
/// one binary frame exactly as given (plot frames carry byte strings and plain text the dictionaries would change).
pub enum Out { Value(Value), Raw(Vec<u8>) }

struct Commit { events: Vec<Value>, done: tokio::sync::oneshot::Sender<Result<u64>> }

/// One WebSocket connection's identity and its single open view.
pub struct Conn { pub sent: Vec<(String, Value)>, pub client: u64, pub gen: String, pub view_id: String, pub target: Value, pub cbor: bool, pub lastrev: u64 }

fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }
pub fn limits() -> Value { json!({"replyBytes":262144,"entries":32,"recent":24,"searchResults":25,"searchCandidates":128,"chunkBytes":65536}) }
pub fn capabilities() -> Value { json!({"version":1,"views":{"version":1,"websocketQuery":"view=1","renderers":["mobile"],"compatibility":"explicit-only"}}) }
pub fn error(id: &Value, code: &str) -> Value { json!({"t":"reply","id":id,"ok":false,"error":{"code":code,"retryable":false,"compatibilityRequired":false}}) }
fn empty_window() -> Value { json!({"contextMessages":[],"messageIds":[],"historyUnits":[],"partial":[],"ids":[],"anchor":{"id":"","offset":0},"older":"","olderNext":"","hasMore":false,"remaining":0,"newer":0,"total":0}) }

fn within(s: &str, n: usize) -> bool { s.len() <= n }
fn target_valid(t: &Value) -> bool {
    let Some(o) = t.as_object() else { return false };
    if o.len() != 2 { return false }
    let (Some(k), Some(i)) = (o.get("kind").and_then(Value::as_str), o.get("id").and_then(Value::as_str)) else { return false };
    within(i, 1024) && ((k == "recent" && i.is_empty()) || (!i.is_empty() && matches!(k, "project" | "thread" | "bot" | "room")))
}

impl Hub {
    pub async fn start(cfg: Cfg, presenter: Arc<dyn Presenter>) -> Result<Arc<Hub>> {
        let store = Store::open(&cfg.home.join("events.jsonl"))?;
        let auth = Auth::load(&cfg.home);
        let epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs().to_string();
        let (ctx, crx) = tokio::sync::mpsc::unbounded_channel::<Commit>();
        let (tx, _) = broadcast::channel(64);
        let (ftx, _) = broadcast::channel(1024);
        let origin = { let s = store.get(0).ok().map(|l| { use sha2::{Digest, Sha256}; hex::encode(&Sha256::digest(l.as_bytes())[..6]) }); s.unwrap_or_else(|| epoch.clone()) };
        let model = Model::build(&store)?;
        let hub = Arc::new(Hub { cfg, auth, epoch, presenter, store: store.clone(), commits: ctx, model: Mutex::new(model), gen: AtomicU64::new(1), changed: tx, frames: ftx, origin,
            rpc: Default::default(), http: Default::default(), live: Default::default(), me: Default::default(), next_client: AtomicU64::new(1), outboxes: Default::default() });
        let _ = hub.me.set(Arc::downgrade(&hub));
        hub.fold_log().await?;
        tokio::spawn(Hub::writer(hub.clone(), crx));
        Ok(hub)
    }

    fn info(&self) -> Value {
        let c = &self.cfg;
        json!({"accounts.live":"{\"version\":1,\"revision\":0,\"default_allowance\":null,\"error\":\"\",\"profiles\":[],\"attempts\":[]}",
            "telemetry.channel":"release","telemetry.id":"","telemetry.live":"","os":c.os,"arch":c.arch,"home":c.home.to_string_lossy(),
            "listen":c.listen,"models":"","machines":"","bots.remote":"[]","tailnet":"off","update":"package","tailscale":"off",
            "freecad":"","kicad.cli":"","kicad.fork":"","kicad.commit":"","kicad.status":"checking","kicad.source":"path","kicad.version":"",
            "kicad.api":"0","kicad.caps":"","kicad.why":"","kicad.min":"10.99.0-bp.4","kicad.can_install":"","kicad.ipc.servers":"",
            "browser.engine.live":"","browser.engine.err":""})
    }

    /// The whole log, as the frames a client's join receives, into the presenter.
    async fn fold_log(&self) -> Result<()> {
        if !self.presenter.wants_log() { return Ok(()) }
        let total = self.store.len().min(std::env::var("BACKPLANE_FOLD_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX)); // dev: fold only the first N events
        let chunk = |a: usize| async move { self.store.range(a, a + 200).map(|ls| ls.iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect::<Vec<Value>>()) };
        let first: Vec<Value> = chunk(0).await?;
        // version "": the JS hub's mobile pictures carry no hub version (compare.mjs, shown.version)
        self.presenter.fold(&json!({"t":"log","since":0,"origin":self.origin,"version":"","info":self.info(),"items":first,"now":now_ms()})).await?;
        let mut at = 200;
        while at < total { let c = chunk(at).await?; self.presenter.fold(&json!({"t":"changes","items":c,"now":now_ms()})).await?; at += 200; }
        self.presenter.fold(&json!({"t":"live","items":[],"now":now_ms()})).await?;
        self.presenter.fold(&json!({"t":"info","info":{"pair":"","pair.qr":""},"now":now_ms()})).await?;
        Ok(())
    }

    /// Commit one event: durable, then folded, then announced. Callers acknowledge after this returns.
    pub async fn commit(&self, event: &Value) -> Result<u64> { self.commit_many(std::slice::from_ref(event)).await }

    /// Events of one request. Requests queue to one writer task that group-commits everything waiting: one
    /// write and one fsync for the batch, the model and the presenter in log order, then every caller is told.
    pub async fn commit_many(&self, events: &[Value]) -> Result<u64> {
        let (done, rx) = tokio::sync::oneshot::channel();
        self.commits.send(Commit { events: events.to_vec(), done }).map_err(|_| anyhow::anyhow!("hub is stopping"))?;
        rx.await?
    }

    async fn writer(hub: Arc<Hub>, mut rx: tokio::sync::mpsc::UnboundedReceiver<Commit>) {
        while let Some(first) = rx.recv().await {
            let mut batch = vec![first];
            while let Ok(c) = rx.try_recv() { batch.push(c); }
            let lines: Vec<String> = batch.iter().flat_map(|c| c.events.iter().map(|e| serde_json::to_string(e).unwrap_or_default())).collect();
            let store = hub.store.clone(); let owned = lines.clone();
            let wrote = tokio::task::spawn_blocking(move || store.append_many(&owned.iter().map(String::as_str).collect::<Vec<_>>())).await;
            match wrote {
                Ok(Ok(first_idx)) => {
                    { let mut m = hub.model.lock().await; for (i, l) in lines.iter().enumerate() { m.apply_line((first_idx + i + 1) as u32, l); } }
                    let all: Vec<Value> = batch.iter().flat_map(|c| c.events.iter().cloned()).collect();
                    let _ = hub.presenter.fold(&json!({"t":"changes","items":all,"now":now_ms()})).await;
                    let n = hub.store.len() as u64; let _ = hub.changed.send(n);
                    let mut upto = first_idx;
                    for c in batch { upto += c.events.len(); let _ = c.done.send(Ok(upto as u64)); }
                }
                Ok(Err(e)) => { let m = format!("{e:#}"); for c in batch { let _ = c.done.send(Err(anyhow::anyhow!(m.clone()))); } }
                Err(e) => { let m = format!("{e}"); for c in batch { let _ = c.done.send(Err(anyhow::anyhow!(m.clone()))); } }
            }
        }
    }

    // ---- the published interface (see api.rs, docs/gpui/HUB-API.md) ----
    /// Later registrations win over earlier ones for the same method.
    pub fn register_rpc(&self, h: Arc<dyn RpcHandler>) { let mut v = self.rpc.write().unwrap(); for m in h.methods() { v.retain(|(n, _)| n != m); v.push((m, h.clone())) } }
    pub fn register_http(&self, h: Arc<dyn HttpHandler>) { let mut v = self.http.write().unwrap(); for r in h.routes() { v.retain(|(n, _)| n != r); v.push((r, h.clone())) } }
    pub fn rpc_for(&self, method: &str) -> Option<Arc<dyn RpcHandler>> { self.rpc.read().unwrap().iter().find(|(n, _)| *n == method).map(|(_, h)| h.clone()) }
    pub fn http_for(&self, route: &str) -> Option<Arc<dyn HttpHandler>> { self.http.read().unwrap().iter().filter(|(p, _)| route == *p || route.starts_with(&format!("{p}/"))).max_by_key(|(p, _)| p.len()).map(|(_, h)| h.clone()) }
    pub fn arc(&self) -> Arc<Hub> { self.me.get().and_then(|w| w.upgrade()).expect("hub is alive") }
    /// Send an unsolicited frame (a plot, a notice) to one connection; false when it has gone.
    pub fn push(&self, client: u64, frame: Value) -> bool { self.outboxes.lock().unwrap().get(&client).map_or(false, |t| t.send(Out::Value(frame)).is_ok()) }
    /// Bytes as one binary WebSocket frame, not re-encoded (plot frames). False when the connection has gone.
    pub fn push_raw(&self, client: u64, bytes: Vec<u8>) -> bool { self.outboxes.lock().unwrap().get(&client).map_or(false, |t| t.send(Out::Raw(bytes)).is_ok()) }
    /// ... to every connection.
    pub fn push_all(&self, frame: &Value) { for t in self.outboxes.lock().unwrap().values() { let _ = t.send(Out::Value(frame.clone())); } }
    pub fn attach(&self, client: u64) -> tokio::sync::mpsc::UnboundedReceiver<Out> { let (t, r) = tokio::sync::mpsc::unbounded_channel(); self.outboxes.lock().unwrap().insert(client, t); r }
    /// True while the connection's outbox exists (it has not gone away).
    pub fn is_attached(&self, client: u64) -> bool { self.outboxes.lock().unwrap().contains_key(&client) }
    pub fn detach(&self, client: u64) { self.outboxes.lock().unwrap().remove(&client); }
    /// Read access to the model: threads, projects, entry indexes, pages.
    pub async fn model(&self) -> tokio::sync::MutexGuard<'_, Model> { self.model.lock().await }
    /// A setting's current value (None when it has none or was removed); read from the log line the model points at.
    pub async fn setting(&self, key: &str) -> Option<String> {
        let seq = self.model.lock().await.setting_seq(key)?;
        let v: Value = serde_json::from_str(&self.store.get(seq as usize - 1).ok()?).ok()?;
        v["value"].as_str().map(str::to_string)
    }
    /// A receiver that hears the log's new length after every commit.
    pub fn subscribe(&self) -> broadcast::Receiver<u64> { self.changed.subscribe() }
    /// Streamed text of a running turn, not yet an event: shown to views as the thread's live reply.
    pub async fn live_delta(&self, thread: &str, text: &str) -> Result<()> {
        self.live.lock().unwrap().entry(thread.to_string()).or_default().push_str(text);
        let _ = self.frames.send(json!({"t":"delta","thread":thread,"text":text}));
        self.presenter.fold(&json!({"t":"delta","thread":thread,"text":text,"now":now_ms()})).await?;
        let _ = self.changed.send(self.store.len() as u64);
        Ok(())
    }
    /// The stream for `thread` ended (its MessagePosted follows or has been committed).
    pub async fn live_end(&self, thread: &str) -> Result<()> {
        self.live.lock().unwrap().remove(thread);
        let _ = self.frames.send(json!({"t":"delta_end","thread":thread}));
        self.presenter.fold(&json!({"t":"delta_end","thread":thread,"now":now_ms()})).await?;
        let _ = self.changed.send(self.store.len() as u64);
        Ok(())
    }
    /// The live text of every streaming thread (for a client that joins mid-turn).
    pub fn live_snapshot(&self) -> Vec<(String, String)> { self.live.lock().unwrap().iter().map(|(a, b)| (a.clone(), b.clone())).collect() }
    /// The `live` frame a joining client gets: every streaming thread's text so far (core/live.bend Live.set).
    pub fn live_frame(&self) -> Value {
        let mut v = self.live_snapshot(); v.sort();
        json!({"t":"live","items":v.into_iter().map(|(t, x)| json!({"thread":t,"text":x,"trunc":false,"gen":0})).collect::<Vec<_>>()})
    }
    /// A log line as a live client receives it: a turn's input travels blank (core/model.bend Setting.input,
    /// codec.bend Change.wire); the log and a join's `log` frame keep it whole.
    pub fn wire_item(line: &str, blank_inputs: bool) -> Option<Value> {
        let mut v: Value = serde_json::from_str(line).ok()?;
        if blank_inputs && v["$"] == "SettingSet" {
            if let Some(k) = v["key"].as_str() { if k.starts_with("launch.input.") || k.starts_with("fallback.input.") { v["value"] = json!(""); } }
        }
        Some(v)
    }
    pub fn info_frame(&self) -> Value { json!({"t":"info","info":{"pair":"","pair.qr":""}}) }
    pub fn log_info(&self) -> Value { self.info() }

    pub async fn revision(&self) -> u64 { self.store.len() as u64 + 1 }

    pub fn connect(&self, cbor: bool) -> (Conn, Value) {
        let gen = self.gen.fetch_add(1, Relaxed).to_string();
        let g = json!({"t":"view","version":1,"renderer":"mobile","generation":gen,"epoch":self.epoch,"limits":limits()});
        (Conn { sent: vec![], client: self.next_client.fetch_add(1, Relaxed), gen, view_id: String::new(), target: Value::Null, cbor, lastrev: 0 }, g)
    }

    pub fn notice(&self, c: &Conn, rev: u64) -> Value {
        json!({"t":"view.changed","version":1,"generation":c.gen,"epoch":self.epoch,"viewId":c.view_id,"target":c.target,"viewRevision":rev.to_string()})
    }

    /// One inbound request -> the frames to send back, in order.
    pub async fn request(&self, c: &mut Conn, msg: &Value) -> Vec<Value> {
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let m = msg.get("m").and_then(Value::as_str).unwrap_or("");
        if m == "ping" { return vec![] }
        if m == "load.ping" { return vec![json!({"t":"reply","id":id,"ok":true})] }
        if let Some(handler) = self.rpc_for(m) {
            let me = self.arc();
            return handler.call(&me, &Caller { gen: c.gen.clone(), client: c.client }, &id, m, msg.get("p").unwrap_or(&Value::Null)).await;
        }
        if m == "turn.start" { return self.turn_start(&id, msg.get("p").unwrap_or(&Value::Null)).await }
        if !m.starts_with("view.") {
            return vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown method"})];
        }
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
            "view.open" | "view.update" | "view.action" => {}
            "view.page" | "view.search" | "view.message" | "view.cancel" => return vec![error(&id, "UNSUPPORTED")],
            _ => return vec![error(&id, "INVALID_REQUEST")],
        }
        if m != "view.open" && p.get("viewId").and_then(Value::as_str) != Some(c.view_id.as_str()) { return vec![error(&id, "STALE_VIEW")] }
        let view_id = if m == "view.open" { format!("{}:{}", c.gen, rid) } else { c.view_id.clone() };
        let rev = self.revision().await;
        let mut params = p.clone();
        if let Some(o) = params.as_object_mut() { o.insert("revision".into(), json!(rev)); o.insert("now".into(), json!(now_ms())); }
        let pol = match self.presenter.view(&params).await { Ok(v) => v, Err(_) => return vec![error(&id, "PRESENTER_UNAVAILABLE")] };
        let target = p["target"].clone();
        if m == "view.open" { c.view_id = view_id.clone(); c.target = target.clone(); }
        c.lastrev = rev;
        if m == "view.open" { c.sent.clear(); }
        let result = self.assemble(&p, &view_id, rev, &pol);
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

    /// turn.start: the person's message is committed (durable) and queued, then acknowledged. The
    /// events are the JS hub's first two for it (MessagePosted, TurnQueued); the agent turn is M5.
    async fn turn_start(&self, id: &Value, p: &Value) -> Vec<Value> {
        let (thread, text) = (p.get("thread").and_then(Value::as_str).unwrap_or(""), p.get("text").and_then(Value::as_str).unwrap_or(""));
        if thread.is_empty() || !self.model.lock().await.threads.contains_key(thread) { return vec![json!({"t":"reply","id":id,"ok":false,"error":"unknown thread"})] }
        let at = now_ms() / 1000;
        let msg = p.get("msg").and_then(Value::as_str).filter(|m| !m.is_empty()).map(str::to_string).unwrap_or_else(|| format!("m{at}-{}", id.as_u64().unwrap_or(0)));
        let events = [json!({"$":"MessagePosted","thread":thread,"msg":msg,"role":"user","text":text,"at":at}), json!({"$":"TurnQueued","thread":thread,"msg":msg,"text":text,"at":at})];
        if self.commit_many(&events).await.is_err() { return vec![json!({"t":"reply","id":id,"ok":false,"error":"store write failed"})] }
        vec![json!({"t":"reply","id":id,"ok":true})]
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
        }
        Value::Object(o)
    }
}

#[cfg(test)]
mod tests {
    use super::*; use crate::presenter::Canned;
    fn cfg(home: &std::path::Path) -> Cfg { Cfg { home: home.to_path_buf(), version: "0.14.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() } }
    fn home(name: &str) -> PathBuf { let p = std::env::temp_dir().join(format!("bp-hub-{}-{}", std::process::id(), name)); let _ = std::fs::remove_dir_all(&p); std::fs::create_dir_all(&p).unwrap(); p }
    fn req(c: &Conn, hub: &Hub, m: &str, id: u64, extra: Value) -> Value {
        let mut p = json!({"version":1,"hub":"http://x","generation":c.gen,"epoch":hub.epoch,"requestRevision":1,"target":{"kind":"recent","id":""},"viewId":c.view_id,"session":{}});
        for (k, v) in extra.as_object().into_iter().flatten() { p[k] = v.clone() }
        json!({"id":id,"m":m,"p":p})
    }
    #[tokio::test] async fn open_assembles_identity_in_js_order() {
        let h = Hub::start(cfg(&home("open")), Arc::new(Canned { reply: json!({"remoteReady":true,"shown":{"a":1},"session":{"sel":""},"cmds":[]}) })).await.unwrap();
        let (mut c, g) = h.connect(true); assert_eq!(g["renderer"], "mobile");
        let r0 = req(&c, &h, "view.open", (1 << 47) + 5, json!({})); let out = h.request(&mut c, &r0).await;
        let r = &out[0]["result"]; let keys: Vec<&String> = r.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["remoteReady","viewRevision","renderer","viewId","target","requestRevision","epoch","generation","hub","version","mailDraftContext","window","cmds","session","shown"]);
        assert_eq!(r["viewId"], format!("{}:{}", c.gen, (1u64 << 47) + 5));
    }
    #[tokio::test] async fn stale_and_invalid_requests_are_refused() {
        let h = Hub::start(cfg(&home("stale")), Arc::new(Canned { reply: json!({"remoteReady":true,"shown":{},"session":{},"cmds":[]}) })).await.unwrap();
        let (mut c, _) = h.connect(true);
        let mut bad = req(&c, &h, "view.open", (1 << 47) + 1, json!({})); bad["p"]["generation"] = json!("999");
        assert_eq!(h.request(&mut c, &bad).await[0]["error"]["code"], "STALE_CONNECTION");
        let low = req(&c, &h, "view.open", 5, json!({})); assert_eq!(h.request(&mut c, &low).await[0]["error"]["code"], "STALE_CONNECTION");
        let upd = req(&c, &h, "view.update", (1 << 47) + 2, json!({"viewId":"nope"})); assert_eq!(h.request(&mut c, &upd).await[0]["error"]["code"], "STALE_VIEW");
        assert_eq!(h.request(&mut c, &json!({"id":3,"m":"x.y","p":{}})).await[0]["error"], "unknown method");
    }
    #[tokio::test] async fn big_replies_are_segmented_when_asked() {
        let big = "x".repeat(300_000);
        let h = Hub::start(cfg(&home("seg")), Arc::new(Canned { reply: json!({"remoteReady":true,"shown":{"s":1},"session":{"big":big},"cmds":[]}) })).await.unwrap();
        let (mut c, _) = h.connect(true);
        let r0 = req(&c, &h, "view.open", (1 << 47) + 9, json!({"viewControl":1})); let out = h.request(&mut c, &r0).await;
        assert!(out.len() > 2); assert_eq!(out.last().unwrap()["result"]["controlId"], (1u64 << 47) + 9);
        let text: String = out[..out.len() - 1].iter().map(|f| f["text"].as_str().unwrap().to_string()).collect();
        let values: Value = serde_json::from_str(&text).unwrap(); assert!(values.get("shown").is_none()); assert_eq!(values["session"]["big"].as_str().unwrap().len(), 300_000);
        assert_eq!(out[out.len() - 2]["last"], true);
        for f in &out { assert!(serde_json::to_string(f).unwrap().len() <= REPLY_BYTES) }
    }
    #[tokio::test] async fn commit_is_durable_and_counts_revisions() {
        let dir = home("commit"); let h = Hub::start(cfg(&dir), Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let r0 = h.revision().await; h.commit(&json!({"$":"SettingSet","key":"k","value":"v","at":1})).await.unwrap();
        assert_eq!(h.revision().await, r0 + 1); assert_eq!(Store::open(&dir.join("events.jsonl")).unwrap().len(), 1);
    }
}

#[cfg(test)]
mod write_tests {
    use super::*; use crate::presenter::Canned;
    #[tokio::test] async fn turn_start_commits_a_message_and_acks_after_it_is_durable() {
        let dir = std::env::temp_dir().join(format!("bp-hub-ts-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("events.jsonl"), "{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"p\",\"root\":\"/x\",\"at\":1}\n{\"$\":\"ThreadCreated\",\"id\":\"t1\",\"project\":\"p1\",\"title\":\"T\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":1}\n").unwrap();
        let cfg = Cfg { home: dir.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() };
        let h = Hub::start(cfg, Arc::new(Canned { reply: json!({}) })).await.unwrap();
        let (mut c, _) = h.connect(true);
        let out = h.request(&mut c, &json!({"id":9,"m":"turn.start","p":{"thread":"t1","text":"hello","msg":"m1"}})).await;
        assert_eq!(out[0]["ok"], true);
        let disk = Store::open(&dir.join("events.jsonl")).unwrap(); assert_eq!(disk.len(), 4); assert!(disk.get(2).unwrap().contains("\"MessagePosted\""));
        assert_eq!(h.model.lock().await.entries(), 1);
        let bad = h.request(&mut c, &json!({"id":10,"m":"turn.start","p":{"thread":"nope","text":"x"}})).await; assert_eq!(bad[0]["ok"], false);
    }
}

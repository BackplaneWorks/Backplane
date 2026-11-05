//! The browser helper driver (src/server/browser.bend; tools/browser stays a helper): one `backplane-browser` process, started on the first
//! request, spoken to in JSON lines, one persistent Chrome profile at <home>/browser-profile and every page's JPEG frames in <home>/shots.
//! Pages are keyed by an owner (the thread's id here; a bot's id once the bots lane has bots), so each thread's preview_* tools drive a page of their
//! own. The `preview_*` MCP tools (op = the name after `preview_`), `browser.close` (thread.delete closes the thread's page), the helper's frame
//! events (`{"t":"shot","bot":<owner>,"n":N}` to every client) and the take-over rule (while the person drives, the agent's calls are refused and
//! never reach the helper; set by the teach driver through `set_drive`).
//! Bounded: at most MAX_PENDING calls in flight (more answer "busy"), a call waits at most CALL_TIMEOUT, helper lines over MAX_LINE are skipped
//! without being kept (a reply that long fails its call when its id leads the line; otherwise that call waits out CALL_TIMEOUT), the helper's input
//! queue holds 64 lines, at most MAX_OWNERS page owners are remembered. A hub on a copied log (BACKPLANE_COPY=1) starts no helper (its pages and tasks name real sites).
use crate::hub::Hub;
use serde_json::{json, Map, Value};
use std::{collections::HashMap, path::PathBuf, process::Stdio, sync::{Arc, OnceLock}, time::Duration};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, process::Command, sync::{mpsc, oneshot, Mutex}};

pub const MAX_PENDING: usize = 64;
pub const CALL_TIMEOUT: Duration = Duration::from_secs(75);
pub const MAX_LINE: usize = 16 << 20;
pub const MAX_OWNERS: usize = 1024;
/// Teach.refusal
pub const REFUSAL: &str = "The person has the browser (teaching or taking over); wait for them to hand it back.";
const NO_HELPER: &str = "the Backplane browser could not start (is backplane-browser installed next to Backplane?)";
pub const PREVIEW: &[&str] = &["preview_open", "preview_navigate", "preview_snapshot", "preview_click", "preview_type", "preview_press", "preview_scroll", "preview_evaluate", "preview_wait_for", "preview_resize", "preview_status"];

#[derive(Default)]
struct St { input: Option<mpsc::Sender<String>>, pending: HashMap<u64, oneshot::Sender<String>>, next: u64, gen: u64, drive: HashMap<String, String>, tabs: HashMap<String, String> }
pub struct Browser { hub: Arc<Hub>, st: Mutex<St>, exe: Option<PathBuf>, events: tokio::sync::broadcast::Sender<Value> }

static BROWSERS: OnceLock<std::sync::Mutex<HashMap<PathBuf, Arc<Browser>>>> = OnceLock::new();
pub fn get(hub: &Hub) -> Option<Arc<Browser>> { BROWSERS.get()?.lock().unwrap().get(&hub.cfg.home).cloned() }

/// BACKPLANE_BROWSER, else `backplane-browser` beside the hub's binary.
fn helper_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("BACKPLANE_BROWSER") { if !p.is_empty() { return Some(PathBuf::from(p)) } }
    let me = std::env::current_exe().ok()?; let p = me.parent()?.join("backplane-browser"); p.exists().then_some(p)
}
pub fn fail(m: &str) -> String { json!({"ok":false,"error":m}).to_string() }

pub fn start(hub: &Arc<Hub>) {
    let b = Arc::new(Browser { hub: hub.clone(), st: Mutex::new(St::default()), exe: helper_path(), events: tokio::sync::broadcast::channel(256).0 });
    BROWSERS.get_or_init(Default::default).lock().unwrap().insert(hub.cfg.home.clone(), b.clone());
    hub.register_rpc(Arc::new(Close(b.clone())));
    super::driver::start(hub, b);
}

/// The request's arguments with "page" set to this owner. Tool arguments cannot reach another owner's page, claim to be the person, or
/// choose the op or the id (`id` picks the pending slot a reply goes to; `op`, and for an agent `type`/`method` the helper falls back to, pick what
/// runs). The hub's own calls (the person's mouse events carry a `type`) keep `type` and `method`; `op` is always set, so they pick nothing.
pub fn paged(owner: &str, args: &Value, agent: bool) -> Map<String, Value> {
    let mut m: Map<String, Value> = args.as_object().map(|o| o.iter().filter(|(k, _)| !(matches!(k.as_str(), "page" | "__desktopActor" | "id" | "op") || (agent && matches!(k.as_str(), "type" | "method")))).map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    m.insert("page".into(), owner.into()); m
}

impl Browser {
    /// The helper's reply line for `op` on `owner`'s page ("" is the shared page).
    pub async fn ask(self: &Arc<Self>, owner: &str, op: &str, args: &Value) -> String { self.ask_as(owner, op, args, false).await }
    /// `agent`: the call is an agent's, and is refused (never sent) if the person has the page now: checked under the same lock that sends it.
    pub async fn ask_as(self: &Arc<Self>, owner: &str, op: &str, args: &Value, agent: bool) -> String {
        if crate::copy::on() { return fail(&crate::copy::refusal("the browser helper")) }
        let (id, rx) = {
            let mut st = self.st.lock().await;
            if agent && st.drive.get(owner).map_or(false, |m| !m.is_empty()) { return fail(REFUSAL) }
            if st.pending.len() >= MAX_PENDING { return fail("the browser is busy: too many calls are waiting") }
            if st.input.is_none() { match self.spawn(&mut st).await { Some(i) => st.input = Some(i), None => return fail(NO_HELPER) } }
            st.next += 1; let id = st.next; let (tx, rx) = oneshot::channel(); st.pending.insert(id, tx);
            let mut o = Map::new(); o.insert("id".into(), id.into()); o.insert("op".into(), op.into()); for (k, v) in paged(owner, args, agent) { o.insert(k, v); }
            let line = Value::Object(o).to_string();
            if st.input.as_ref().unwrap().try_send(line).is_err() { st.pending.remove(&id); st.input = None; return fail("the browser is busy") }
            (id, rx)
        };
        match tokio::time::timeout(CALL_TIMEOUT, rx).await {
            Ok(Ok(line)) => line,
            Ok(Err(_)) => fail("the browser stopped"),
            Err(_) => { self.st.lock().await.pending.remove(&id); fail("the browser did not answer in time") }
        }
    }

    /// Start the helper and the tasks that read and write it.
    async fn spawn(self: &Arc<Self>, st: &mut St) -> Option<mpsc::Sender<String>> {
        let exe = self.exe.clone()?; let home = &self.hub.cfg.home;
        let _ = std::fs::create_dir_all(home.join("shots"));
        let mut child = crate::proc::tokio_cmd(crate::proc::Kind::Browser, exe).args(["--profile".into(), home.join("browser-profile").to_string_lossy().into_owned(), "--jpeg-dir".into(), home.join("shots").to_string_lossy().into_owned()])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true).spawn().ok()?;
        let (mut stdin, stdout) = (child.stdin.take()?, child.stdout.take()?);
        st.gen += 1; let gen = st.gen;
        let (tx, mut rx) = mpsc::channel::<String>(64);
        tokio::spawn(async move { while let Some(l) = rx.recv().await { if stdin.write_all(l.as_bytes()).await.is_err() || stdin.write_all(b"\n").await.is_err() || stdin.flush().await.is_err() { break } } });
        let me = self.clone();
        tokio::spawn(async move {
            let mut rd = BufReader::new(stdout); let mut buf = Vec::new();
            loop {
                buf.clear();
                // a line is read up to MAX_LINE; a longer one is skipped to its end
                let n = match (&mut rd).take(MAX_LINE as u64 + 1).read_until(b'\n', &mut buf).await { Ok(n) => n, Err(_) => 0 };
                if n == 0 { break }
                if buf.len() > MAX_LINE && buf.last() != Some(&b'\n') {
                    me.too_long(&buf[..256.min(buf.len())]).await; buf.clear();
                    // the rest of the line is dropped as it is read, never kept
                    loop { let (used, done) = match rd.fill_buf().await { Ok(b) if !b.is_empty() => match b.iter().position(|&c| c == b'\n') { Some(i) => (i + 1, true), None => (b.len(), false) }, _ => (0, true) }; rd.consume(used); if done { break } }
                    continue
                }
                if let Ok(line) = std::str::from_utf8(&buf) { me.line(line.trim_end()).await }
            }
            let _ = child.wait().await;
            let mut st = me.st.lock().await;
            if st.gen == gen { st.input = None; for (_, tx) in st.pending.drain() { let _ = tx.send(fail("the browser stopped")); } let _ = me.events.send(json!({"event":"helper-exit"})); }
        });
        Some(tx)
    }

    /// A reply too long to keep: if its id leads the line, fail that call at once.
    async fn too_long(&self, head: &[u8]) {
        let h = String::from_utf8_lossy(head); let Some(rest) = h.trim_start().strip_prefix("{\"id\":") else { return };
        if let Ok(id) = rest.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u64>() { if let Some(tx) = self.st.lock().await.pending.remove(&id) { let _ = tx.send(fail("the browser's answer was too large")); } }
    }

    /// A line from the helper: a reply (by id), or an event: a page's frame (`shot` to every client), `tabs` (the `browser.tabs.<page>` info),
    /// and record, navigated, load, shield-click, engine to the teach driver.
    pub(crate) async fn line(&self, text: &str) {
        let Ok(o) = serde_json::from_str::<Value>(text) else { return };
        if let Some(ev) = o.get("event").and_then(Value::as_str) {
            let page = o.get("page").and_then(Value::as_str).unwrap_or("");
            match ev {
                "frame" => if o.get("page").is_some() { self.hub.push_all(&json!({"t":"shot","bot":page,"n":o.get("n").cloned().unwrap_or(json!(0))})); },
                "tabs" => {
                    { let mut st = self.st.lock().await; if st.tabs.contains_key(page) || st.tabs.len() < MAX_OWNERS { st.tabs.insert(page.to_string(), o.to_string()); } }
                    if !self.hub.set_info(&format!("browser.tabs.{page}"), &o.to_string()).await { eprintln!("term: info browser.tabs.{page} refused (bounded)") }
                }
                "record" | "navigated" | "load" | "shield-click" | "engine" => { let _ = self.events.send(o.clone()); }
                _ => {}
            }
            return
        }
        if let Some(id) = o.get("id").and_then(Value::as_u64) { if let Some(tx) = self.st.lock().await.pending.remove(&id) { let _ = tx.send(text.to_string()); } }
    }

    /// Who drives an owner's page: "" (the agent), "teach" or "takeover" (docs/teach.md `drive.<bot>`).
    /// Past MAX_OWNERS owners a new mode is refused (false) and the caller must not hand the page over; an existing owner's can always change.
    pub async fn set_drive(&self, owner: &str, mode: &str) -> bool {
        let mut st = self.st.lock().await;
        if mode.is_empty() { st.drive.remove(owner); true } else if st.drive.contains_key(owner) || st.drive.len() < MAX_OWNERS { st.drive.insert(owner.into(), mode.into()); true } else { false }
    }
    /// Helper events about pages for the teach driver: record, navigated, load, shield-click, engine, helper-exit (tabs also go to clients).
    pub fn events(&self) -> tokio::sync::broadcast::Receiver<Value> { self.events.subscribe() }
    /// Whether the agent may use `owner`'s page now (tests and the teach driver).
    pub async fn agent_drives_ok(&self, owner: &str) -> bool { self.st.lock().await.drive.get(owner).map_or(true, |m| m.is_empty()) }
    /// The latest `tabs` event of a page (what `browser.tabs.<page>` info carries).
    pub async fn tabs_of(&self, page: &str) -> Option<String> { self.st.lock().await.tabs.get(page).cloned() }
    }

/// Preview.content: the helper's reply as an MCP result's content blocks: the result as text, a screenshot as an image block; an error is
/// one text block and the call is flagged.
pub fn content(line: &str) -> (bool, Vec<Value>) {
    let o: Value = serde_json::from_str(line).unwrap_or(Value::Null);
    if o.get("ok").and_then(Value::as_bool) != Some(true) { return (false, vec![json!({"type":"text","text":o.get("error").and_then(Value::as_str).unwrap_or("browser error")})]) }
    let mut r = o.get("result").cloned().unwrap_or(Value::Null);
    let shot = r.get("screenshot").cloned().unwrap_or(Value::Null);
    if let Some(m) = r.as_object_mut() { m.remove("screenshot"); }
    let mut blocks = vec![json!({"type":"text","text":r.to_string()})];
    let data = shot.get("data").and_then(Value::as_str).unwrap_or("");
    if !data.is_empty() { blocks.push(json!({"type":"image","data":data,"mimeType":shot.get("mimeType").and_then(Value::as_str).unwrap_or("image/png")})) }
    (true, blocks)
}

/// A `preview_*` tool call by the agent of `thread`: the body is `{"$content":[blocks]}` (see tools.rs).
pub async fn preview(hub: &Arc<Hub>, thread: &str, name: &str, args: &Value) -> (bool, Value) {
    let blocks = |ok: bool, b: Vec<Value>| (ok, json!({ "$content": b }));
    let Some(b) = get(hub) else { return blocks(false, vec![json!({"type":"text","text":"no browser in this Backplane"})]) };
    // preview_open / preview_navigate bring the Browser tab forward in the thread's viewer, as the call starts (Hub.auto.k + Preview.view; Bend sends it for every thread that is not a bot's)
    if matches!(name, "preview_open" | "preview_navigate") { hub.push_all(&json!({"t":"view","thread":thread,"kind":"browser","path":""})); }
    let line = b.ask_as(thread, &name[8..], args, true).await;
    let (ok, bl) = content(&line); blocks(ok, bl)
}

struct Close(Arc<Browser>);
impl crate::api::RpcHandler for Close {
    fn methods(&self) -> &'static [&'static str] { &["browser.close"] }
    /// thread.delete closes the thread's page (BotJob.close). No events; a missing helper is fine.
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a crate::api::Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> crate::api::Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let t = p.get("thread").and_then(Value::as_str).unwrap_or("");
            if !t.is_empty() && self.0.st.lock().await.input.is_some() { let _ = self.0.ask(t, "close", &json!({})).await; }
            self.0.set_drive(t, "").await; self.0.st.lock().await.tabs.remove(t); self.0.hub.set_info(&format!("browser.tabs.{t}"), "").await;
            vec![json!({"t":"reply","id":id,"ok":true})]
        })
    }
}

#[cfg(test)]
pub(crate) mod tests;
#[cfg(test)]
mod chrome;

//! The browser helper driver (src/server/browser.bend; tools/browser stays a helper): one `backplane-browser` process, started on the first
//! request, spoken to in JSON lines, one persistent Chrome profile at <home>/browser-profile and every page's JPEG frames in <home>/shots.
//! Pages are keyed by an owner (the thread's id here; a bot's id once the bots lane has bots), so each thread's preview_* tools drive a page of their
//! own. The `preview_*` MCP tools (op = the name after `preview_`), `browser.close` (thread.delete closes the thread's page), the helper's frame
//! events (`{"t":"shot","bot":<owner>,"n":N}` to every client) and the take-over rule (while the person drives, the agent's calls are refused and
//! never reach the helper; set by the teach driver through `set_drive`).
//! Bounded: at most MAX_PENDING calls in flight (more answer "busy"), a call waits at most CALL_TIMEOUT, helper lines over MAX_LINE are dropped,
//! the helper's input queue holds 64 lines. A hub on a copied log (BACKPLANE_COPY=1) starts no helper (its pages and tasks name real sites).
use crate::hub::Hub;
use serde_json::{json, Map, Value};
use std::{collections::HashMap, path::PathBuf, process::Stdio, sync::{Arc, OnceLock}, time::Duration};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, process::Command, sync::{mpsc, oneshot, Mutex}};

pub const MAX_PENDING: usize = 64;
pub const CALL_TIMEOUT: Duration = Duration::from_secs(75);
pub const MAX_LINE: usize = 16 << 20;
/// Teach.refusal
pub const REFUSAL: &str = "The person has the browser (teaching or taking over); wait for them to hand it back.";
const NO_HELPER: &str = "the Backplane browser could not start (is backplane-browser installed next to Backplane?)";
pub const PREVIEW: &[&str] = &["preview_open", "preview_navigate", "preview_snapshot", "preview_click", "preview_type", "preview_press", "preview_scroll", "preview_evaluate", "preview_wait_for", "preview_resize", "preview_status"];

#[derive(Default)]
struct St { input: Option<mpsc::Sender<String>>, pending: HashMap<u64, oneshot::Sender<String>>, next: u64, gen: u64, drive: HashMap<String, String> }
pub struct Browser { hub: Arc<Hub>, st: Mutex<St>, exe: Option<PathBuf> }

static BROWSERS: OnceLock<std::sync::Mutex<HashMap<PathBuf, Arc<Browser>>>> = OnceLock::new();
pub fn get(hub: &Hub) -> Option<Arc<Browser>> { BROWSERS.get()?.lock().unwrap().get(&hub.cfg.home).cloned() }

/// BACKPLANE_BROWSER, else `backplane-browser` beside the hub's binary.
fn helper_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("BACKPLANE_BROWSER") { if !p.is_empty() { return Some(PathBuf::from(p)) } }
    let me = std::env::current_exe().ok()?; let p = me.parent()?.join("backplane-browser"); p.exists().then_some(p)
}
pub fn fail(m: &str) -> String { json!({"ok":false,"error":m}).to_string() }

pub fn start(hub: &Arc<Hub>) {
    let b = Arc::new(Browser { hub: hub.clone(), st: Mutex::new(St::default()), exe: helper_path() });
    BROWSERS.get_or_init(Default::default).lock().unwrap().insert(hub.cfg.home.clone(), b.clone());
    hub.register_rpc(Arc::new(Close(b)));
}

/// The request's arguments with "page" set to this owner; any page or actor mark they carried is dropped, so tool arguments cannot reach
/// another owner's page or claim to be the person.
pub fn paged(owner: &str, args: &Value) -> Map<String, Value> {
    let mut m: Map<String, Value> = args.as_object().map(|o| o.iter().filter(|(k, _)| k.as_str() != "page" && k.as_str() != "__desktopActor").map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    m.insert("page".into(), owner.into()); m
}

impl Browser {
    /// The helper's reply line for `op` on `owner`'s page ("" is the shared page).
    pub async fn ask(self: &Arc<Self>, owner: &str, op: &str, args: &Value) -> String {
        if crate::copy::on() { return fail(NO_HELPER) }
        let (id, rx) = {
            let mut st = self.st.lock().await;
            if st.pending.len() >= MAX_PENDING { return fail("the browser is busy: too many calls are waiting") }
            if st.input.is_none() { match self.spawn(&mut st).await { Some(i) => st.input = Some(i), None => return fail(NO_HELPER) } }
            st.next += 1; let id = st.next; let (tx, rx) = oneshot::channel(); st.pending.insert(id, tx);
            let mut o = Map::new(); o.insert("id".into(), id.into()); o.insert("op".into(), op.into()); o.extend(paged(owner, args));
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
        let mut child = Command::new(exe).args(["--profile".into(), home.join("browser-profile").to_string_lossy().into_owned(), "--jpeg-dir".into(), home.join("shots").to_string_lossy().into_owned()])
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
                if buf.len() > MAX_LINE { let mut skip = Vec::new(); let _ = rd.read_until(b'\n', &mut skip).await; continue }
                if let Ok(line) = std::str::from_utf8(&buf) { me.line(line.trim_end()).await }
            }
            let _ = child.wait().await;
            let mut st = me.st.lock().await;
            if st.gen == gen { st.input = None; for (_, tx) in st.pending.drain() { let _ = tx.send(fail("the browser stopped")); } }
        });
        Some(tx)
    }

    /// A line from the helper: a reply (by id), or a page's frame event.
    async fn line(&self, text: &str) {
        let Ok(o) = serde_json::from_str::<Value>(text) else { return };
        if let Some(ev) = o.get("event").and_then(Value::as_str) {
            if ev == "frame" { if let Some(page) = o.get("page").and_then(Value::as_str) { self.hub.push_all(&json!({"t":"shot","bot":page,"n":o.get("n").cloned().unwrap_or(json!(0))})); } }
            return
        }
        if let Some(id) = o.get("id").and_then(Value::as_u64) { if let Some(tx) = self.st.lock().await.pending.remove(&id) { let _ = tx.send(text.to_string()); } }
    }

    /// Who drives an owner's page: "" (the agent), "teach" or "takeover" (docs/teach.md `drive.<bot>`).
    pub async fn set_drive(&self, owner: &str, mode: &str) { let mut st = self.st.lock().await; if mode.is_empty() { st.drive.remove(owner); } else if st.drive.len() < 1024 { st.drive.insert(owner.into(), mode.into()); } }
    pub async fn agent_may(&self, owner: &str) -> bool { self.st.lock().await.drive.get(owner).map_or(true, |m| m.is_empty()) }
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
    if !b.agent_may(thread).await { return blocks(false, vec![json!({"type":"text","text":REFUSAL})]) }
    let line = b.ask(thread, &name[8..], args).await;
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
            self.0.set_drive(t, "").await;
            vec![json!({"t":"reply","id":id,"ok":true})]
        })
    }
}

#[cfg(test)]
mod tests;

//! The teach and task driver (src/server/server.bend's "Teaching a bot's browser" and "Task runs", docs/teach.md): `bots.browser`, `bots.teach`,
//! `bots.task` requests, the task_* tools, recording events, run folders and the pause ask. The pure rules are task.rs and teach.rs.
//! A "bot" here is the page owner (a thread's id until the bots lane says which threads are a bot's). Tasks are folded from the log's
//! TaskSet/TaskDeleted events (a copy of each goes to <home>/bots/<bot>/tasks/<slug>.json). Bounded: MAX_RUNS runs in memory, MAX_WAITERS parked
//! pauses per thread (not in all), the person's input queued per page (INPUT_QUEUE, MAX_PAGES pages), a run keeps its last 20 folders, the draft file is capped with the draft (400 steps), tool arguments are cut as in Bend.
//! Until `Driver::answered` is wired by the agents lane a `task_pause` waits up to PAUSE_WAIT_MS (24 h) for an answer nobody delivers. Seams for other lanes: `Driver::set_wake` (the bots lane wakes a bot with a message), `Driver::answered` (the agents lane calls it when an ask of kind
//! browser/tool task_pause is answered), `Driver::info` (for a joining client's info).
use super::{browser::Browser, now_s, task::{self, In, Run}, teach::{self, Teach, TeachBot}};
use crate::{api::{Caller, Fut, HttpHandler, HttpReply, RpcHandler}, auth::Req, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::{Arc, OnceLock}};
use tokio::sync::{mpsc, oneshot, Mutex};

pub const MAX_RUNS: usize = 256;
pub const MAX_WAITERS: usize = 16;
pub const KEEP_RUNS: usize = 20;
pub const INPUT_QUEUE: usize = 64;
pub const MAX_PAGES: usize = 1024;
const PAUSE_WAIT_MS: u64 = 24 * 3600 * 1000;

pub(crate) struct RunRec { bot: String, slug: String, pub(crate) state: Run }
#[derive(Default)]
pub(crate) struct DState { pub(crate) teach: Teach, tasks: HashMap<(String, String), String>, seen: usize, pub(crate) runs: HashMap<String, RunRec> }
type Waiters = std::sync::Mutex<HashMap<String, (String, oneshot::Sender<String>)>>;
/// What the person did on a page, in order (one worker per page, so mouse down and up keep their order).
struct Input { op: String, p: Value }
pub type Wake = Arc<dyn Fn(&str, String) + Send + Sync>;
pub struct Driver { hub: Arc<Hub>, pub(crate) br: Arc<Browser>, pub(crate) st: Mutex<DState>, pub(crate) waiters: Waiters, pub(crate) pause_ms: std::sync::atomic::AtomicU64, queues: std::sync::Mutex<HashMap<String, mpsc::Sender<Input>>>, wake: std::sync::Mutex<Option<Wake>> }

static DRIVERS: OnceLock<std::sync::Mutex<HashMap<PathBuf, Arc<Driver>>>> = OnceLock::new();
pub fn get(hub: &Hub) -> Option<Arc<Driver>> { DRIVERS.get()?.lock().unwrap().get(&hub.cfg.home).cloned() }

pub fn start(hub: &Arc<Hub>, br: Arc<Browser>) {
    let d = Arc::new(Driver { hub: hub.clone(), br: br.clone(), st: Mutex::new(DState::default()), waiters: Default::default(), pause_ms: std::sync::atomic::AtomicU64::new(PAUSE_WAIT_MS), queues: Default::default(), wake: Default::default() });
    DRIVERS.get_or_init(Default::default).lock().unwrap().insert(hub.cfg.home.clone(), d.clone());
    hub.register_rpc(Arc::new(Rpc(d.clone())));
    hub.register_http(Arc::new(BotsHttp));
    { let w = d.clone(); tokio::spawn(async move { w.sync().await }); } // the first scan of the log, off any request
    // the helper's recorder steps and shield presses; a recording ends after ten minutes
    let (e, mut ev) = (d.clone(), br.events());
    tokio::spawn(async move { loop { match ev.recv().await { Ok(o) => e.event(&o).await, Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue, Err(_) => break } } });
    let t = d.clone();
    tokio::spawn(async move { let mut iv = tokio::time::interval(std::time::Duration::from_secs(10)); loop { iv.tick().await; t.tick(now_s()).await } });
}

/// Cef.url: a typed address as one to open (http(s) and about:blank/credits as they are; a bare host gets https://, a local one or one with a port http://; "" for what is not an address).
pub fn cef_url(s: &str) -> String {
    let t = s.trim(); let l = t.to_lowercase();
    if l.starts_with("http://") || l.starts_with("https://") || t == "about:blank" || t == "about:credits" { return t.into() }
    let host = t.split('/').next().unwrap_or("").split(':').next().unwrap_or("").to_lowercase();
    let local = host == "localhost" || host.ends_with(".localhost") || t.starts_with("127.") || t.starts_with("[::1]") || host == "0.0.0.0";
    let auth = t.split('/').next().unwrap_or(""); let hostport = auth.split(':').nth(1).map_or(false, |p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    if t.is_empty() || t.contains(' ') || (t.contains(':') && !local && !hostport) { return String::new() }
    format!("{}{t}", if local || hostport { "http://" } else { "https://" })
}
/// Cef.toolbar.args: what the helper is asked for a toolbar command.
pub fn toolbar_args(cmd: &str, o: &Value) -> Value {
    match cmd {
        "navigate" => json!({"url": cef_url(s(o, "url"))}),
        "tabs" => if o.get("new").and_then(Value::as_bool).unwrap_or(false) { json!({"new":"https://www.google.com/"}) } else if !s(o, "selectId").is_empty() { json!({"selectId": s(o, "selectId")}) } else { json!({"closeId": s(o, "closeId")}) },
        _ => json!({}),
    }
}
fn ok(id: &Value) -> Vec<Value> { vec![json!({"t":"reply","id":id,"ok":true})] }
fn err(id: &Value, m: &str) -> Vec<Value> { vec![json!({"t":"reply","id":id,"ok":false,"error":m})] }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }

impl Driver {
    pub fn set_wake(&self, w: Wake) { *self.wake.lock().unwrap() = Some(w) }
    fn wake(&self, bot: &str, text: String) { if let Some(w) = self.wake.lock().unwrap().clone() { w(bot, text) } }
    pub(crate) fn dir(&self, bot: &str) -> PathBuf { self.hub.cfg.home.join("bots").join(bot).join("tasks") }
    fn safe(bot: &str) -> bool { teach::part_ok(bot, false) }
    /// A joining client's info: every bot with something to say.
    pub async fn info(&self) -> Vec<(String, String)> { self.st.lock().await.teach.info() }

    /// Fold the log's new TaskSet/TaskDeleted lines (the first call reads them all, once). The reading is on a blocking thread and does not hold the state lock; a
    /// failed read is logged and tried again from the same line next time.
    async fn sync(&self) {
        loop {
            let from = self.st.lock().await.seen; let store = self.hub.store.clone(); let n = store.len(); if from >= n { return }
            let r = tokio::task::spawn_blocking(move || -> Result<Vec<(bool, String, String, String)>, String> {
                let mut out = vec![]; let mut a = from;
                while a < n {
                    let b = (a + 4000).min(n);
                    for line in store.range(a, b).map_err(|e| format!("{e:#}"))? {
                        if !(line.contains("\"TaskSet\"") || line.contains("\"TaskDeleted\"")) { continue }
                        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                        let set = s(&v, "$") == "TaskSet"; if set || s(&v, "$") == "TaskDeleted" { out.push((set, s(&v, "bot").to_string(), s(&v, "slug").to_string(), s(&v, "json").to_string())) }
                    }
                    a = b;
                }
                Ok(out)
            }).await;
            match r {
                Ok(Ok(evs)) => { let mut g = self.st.lock().await; if g.seen != from { continue } for (set, bot, slug, js) in evs { if set { g.tasks.insert((bot, slug), js); } else { g.tasks.remove(&(bot, slug)); } } g.seen = n; }
                Ok(Err(e)) => { eprintln!("term: could not read the log for tasks: {e}"); return }
                Err(e) => { eprintln!("term: the task scan failed: {e}"); return }
            }
        }
    }
    /// The page's driver, and the three info keys (drive.<bot>, teach.<bot>, teach.draft.<bot>) a joining client gets and connected ones are told; an idle bot's are removed.
    async fn put_drive(&self, b: &TeachBot) { self.br.set_drive(&b.bot, &b.drive).await; for (k, v) in b.kvs() { if !self.hub.set_info(&k, &v).await { eprintln!("term: info {k} refused (bounded)") } } }
    async fn set_teach(&self, g: &mut DState, b: TeachBot) -> bool { let okk = g.teach.put(b.clone()); if okk { self.put_drive(&b).await } okk }

    // ---- bots.browser ----
    /// A page owner is a thread the log knows (or, with bots, a live bot): Cef.page.ok.
    async fn known_owner(&self, bot: &str) -> bool { !bot.is_empty() && Self::safe(bot) && self.hub.model().await.threads.contains_key(bot) }
    /// bots.browser {bot, op, ...}. None: the reply comes later (a toolbar command's own).
    async fn browser(self: &Arc<Self>, p: &Value, client: u64, id: &Value) -> Option<Result<(), String>> {
        let bot = s(p, "bot"); let op = s(p, "op");
        if !self.known_owner(bot).await { return Some(Err("unknown browser owner".into())) }
        let mut g = self.st.lock().await; let b = g.teach.get(bot);
        if op == "toolbar" {
            // explicit person input: take the page over, send the command, and the helper's own reply goes back when it comes
            let cmd = s(p, "command").to_string();
            if !matches!(cmd.as_str(), "navigate" | "back" | "forward" | "reload" | "tabs") { return Some(Err("unsupported browser toolbar command".into())) }
            let nb = b.takeover(); if !self.set_teach(&mut g, nb).await { return Some(Err("too many pages are taken over".into())) } drop(g);
            let (me, bot, id, p) = (self.clone(), bot.to_string(), id.clone(), p.clone());
            tokio::spawn(async move {
                let line: Value = serde_json::from_str(&me.br.ask(&bot, &cmd, &toolbar_args(&cmd, &p)).await).unwrap_or(Value::Null);
                me.hub.push(client, json!({"t":"reply","id":id,"ok":line.get("ok").and_then(Value::as_bool).unwrap_or(false),"error":s(&line, "error")}));
            });
            return None
        }
        match op {
            // the native client's embedded Chromium: the helper attaches over CDP to its loopback endpoint and drives the tabs the person sees (docs/browser-gpui-cef.md)
            "attach" | "detach" => {
                let ep = s(p, "endpoint"); if op == "attach" && !(ep.starts_with("http://127.0.0.1:") || ep.starts_with("http://localhost:")) { return Some(Err("attach needs a loopback endpoint".into())) }
                drop(g);
                let (me, bot, id, p, op) = (self.clone(), bot.to_string(), id.clone(), p.clone(), op.to_string());
                tokio::spawn(async move {
                    let line: Value = serde_json::from_str(&me.br.ask(&bot, &op, &p).await).unwrap_or(Value::Null);
                    me.hub.push(client, json!({"t":"reply","id":id,"ok":line.get("ok").and_then(Value::as_bool).unwrap_or(false),"error":s(&line, "error")}));
                });
                None
            }
            "takeover" => { let nb = b.takeover(); if !self.set_teach(&mut g, nb).await { return Some(Err("too many pages are taken over".into())) } Some(Ok(())) }
            "handback" => { let nb = b.handback(); self.set_teach(&mut g, nb).await; Some(Ok(())) }
            _ => {
                // the person's input goes through only while they have the browser; the reply is at once and the input runs in order on the page's queue
                if b.drive.is_empty() { return Some(Err("take over the browser first".into())) }
                drop(g); self.enqueue(bot, Input { op: op.into(), p: p.clone() }); Some(Ok(()))
            }
        }
    }
    /// One ordered queue and worker per page: down and up keep their order, and a slow helper never holds a reply.
    fn enqueue(self: &Arc<Self>, bot: &str, i: Input) {
        let mut q = self.queues.lock().unwrap();
        if !q.contains_key(bot) {
            if q.len() >= MAX_PAGES { return }
            let (tx, mut rx) = mpsc::channel::<Input>(INPUT_QUEUE); q.insert(bot.to_string(), tx);
            let (me, bot) = (self.clone(), bot.to_string());
            tokio::spawn(async move { while let Some(i) = rx.recv().await { me.input(&bot, &i).await } });
        }
        let _ = q.get(bot).unwrap().try_send(i); // a full queue drops the newest input
    }
    async fn input(&self, bot: &str, i: &Input) {
        let (p, op) = (&i.p, i.op.as_str());
        if !matches!(op, "click" | "mouse" | "type" | "press" | "scroll" | "navigate" | "back" | "reload") { return }
        let (vw, vh) = if matches!(op, "click" | "mouse") { self.viewport(bot).await } else { (1280, 800) };
        let (x, y) = task::scale(p.get("x").and_then(Value::as_u64).unwrap_or(0), p.get("y").and_then(Value::as_u64).unwrap_or(0), 1000, 1000, vw, vh);
        let num = |k: &str| p.get(k).filter(|v| v.is_number()).cloned();
        let args = match op {
            "click" => json!({"x":x,"y":y}),
            "mouse" => { let t = s(p, "type"); json!({"type": if t == "down" || t == "up" { t } else { "move" }, "x":x, "y":y}) }
            "type" => json!({"text": s(p, "text")}), "press" => json!({"key": s(p, "key")}), "navigate" => json!({"url": s(p, "url")}),
            "scroll" => { let mut o = serde_json::Map::new(); if let Some(v) = num("dx") { o.insert("dx".into(), v); } if let Some(v) = num("dy") { o.insert("dy".into(), v); } Value::Object(o) }
            _ => json!({}),
        };
        self.br.ask(bot, op, &args).await;
    }
    /// The page's size from a status reply (the viewport a click lands in).
    async fn viewport(&self, bot: &str) -> (u64, u64) {
        let l: Value = serde_json::from_str(&self.br.ask(bot, "status", &json!({})).await).unwrap_or(Value::Null); let r = l.get("result").cloned().unwrap_or(Value::Null);
        (r.get("width").and_then(Value::as_u64).unwrap_or(1280), r.get("height").and_then(Value::as_u64).unwrap_or(800))
    }

    // ---- bots.teach ----
    async fn teach(&self, p: &Value) -> Result<Value, String> {
        let bot = s(p, "bot"); let op = s(p, "op"); let now = now_s();
        if bot.is_empty() || !Self::safe(bot) { return Err("unknown browser owner".into()) }
        let mut g = self.st.lock().await; let b = g.teach.get(bot);
        match op {
            "start" => {
                let nb = TeachBot::started(bot, now); if !self.set_teach(&mut g, nb).await { return Err("too many pages are taken over".into()) }
                drop(g); let _ = std::fs::remove_file(self.dir(bot).join(".draft.jsonl")); self.br.ask(bot, "record", &json!({"on":true})).await; Ok(json!({}))
            }
            "note" => {
                let nb = b.add(&teach::note(&s(p, "text").trim().chars().take(500).collect::<String>())); let line = nb.last(); self.set_teach(&mut g, nb).await; drop(g); self.draft_add(bot, &line); Ok(json!({}))
            }
            "stop" => { let nb = b.stop(); self.set_teach(&mut g, nb).await; drop(g); self.br.ask(bot, "record", &json!({"on":false})).await; Ok(json!({})) }
            "discard" => { self.set_teach(&mut g, TeachBot::blank(bot)).await; drop(g); let _ = std::fs::remove_file(self.dir(bot).join(".draft.jsonl")); self.br.ask(bot, "record", &json!({"on":false})).await; Ok(json!({})) }
            "save" => {
                let name = s(p, "name").trim().to_string();
                if name.is_empty() || b.steps.is_empty() { return Err("name the task and record at least one step".into()) }
                let marks: Vec<Value> = p.get("params").and_then(Value::as_array).cloned().unwrap_or_default();
                let t = teach::build(&b, &name, s(p, "description"), &marks, now);
                if t.steps.len() > 200 { return Err("at most 200 steps".into()) }
                self.set_teach(&mut g, TeachBot::blank(bot)).await; drop(g);
                let (slug, js) = (t.slug.clone(), task::json(&t).to_string());
                self.set_task(bot, &slug, &js, now).await;
                self.br.ask(bot, "record", &json!({"on":false})).await; let _ = std::fs::remove_file(self.dir(bot).join(".draft.jsonl"));
                self.wake(bot, teach::taught(&name, &t)); Ok(json!({"slug": slug}))
            }
            _ => Err("unknown teach request".into()),
        }
    }
    fn draft_add(&self, bot: &str, line: &str) {
        if line.is_empty() || !Self::safe(bot) { return }
        let d = self.dir(bot); let _ = std::fs::create_dir_all(&d);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join(".draft.jsonl")) { use std::io::Write; let _ = writeln!(f, "{line}"); }
    }

    // ---- tasks ----
    /// TaskSet: logged, folded, and copied to <home>/bots/<bot>/tasks/<slug>.json (a plain id and slug only).
    pub async fn set_task(&self, bot: &str, slug: &str, js: &str, now: u64) {
        let _ = self.hub.commit(&json!({"$":"TaskSet","bot":bot,"slug":slug,"json":js,"at":now})).await;
        { self.sync().await; let mut g = self.st.lock().await; }
        if Self::safe(bot) && teach::part_ok(slug, false) { let d = self.dir(bot); let _ = std::fs::create_dir_all(&d); let _ = std::fs::write(d.join(format!("{slug}.json")), js); }
    }
    async fn task_rpc(&self, p: &Value) -> Result<Value, String> {
        let bot = s(p, "bot"); let slug = task::slug(s(p, "slug")); let now = now_s();
        match s(p, "op") {
            "delete" => { let _ = self.hub.commit(&json!({"$":"TaskDeleted","bot":bot,"slug":slug,"at":now})).await; self.sync().await; let mut g = self.st.lock().await; if Self::safe(bot) && teach::part_ok(&slug, false) { let _ = std::fs::remove_file(self.dir(bot).join(format!("{slug}.json"))); } Ok(json!({})) }
            "run" => {
                let t = { self.sync().await; let mut g = self.st.lock().await; g.tasks.get(&(bot.to_string(), slug.clone())).cloned() };
                if t.is_none() { return Err("no such task".into()) }
                let params = match p.get("params") { Some(v) if v.is_object() => serde_json::to_string(v).unwrap_or_default(), _ => "{}".into() };
                self.wake(bot, teach::run_msg(&slug, &params)); Ok(json!({}))
            }
            _ => Err("unknown task op".into()),
        }
    }

    /// The Tasks tab's data (MBots.tasks.tab): the bot's tasks by name with how the last run went, and its newest runs (at most 8).
    pub async fn tasks_view(&self, bot: &str) -> Value {
        self.sync().await; let g = self.st.lock().await;
        let n = |r: &str| r.rsplit('-').next().and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
        let mut runs: Vec<(&String, &RunRec)> = g.runs.iter().filter(|(_, r)| r.bot == bot).collect(); runs.sort_by(|a, b| n(b.0).cmp(&n(a.0)));
        let word = |r: &RunRec| match &r.state { Run::Running(_) => "running".to_string(), Run::Waiting { .. } => "waiting".into(), Run::Person(_) => "you drive".into(), Run::Ended(s) => s.clone() };
        let mut ts: Vec<task::Task> = g.tasks.iter().filter(|((b, _), _)| b == bot).filter_map(|(_, j)| task::read(j)).collect();
        ts.sort_by(|a, b| task::first_name(a).to_lowercase().cmp(&task::first_name(b).to_lowercase()));
        let items: Vec<Value> = ts.iter().map(|t| {
            let last = runs.iter().find(|(_, r)| r.slug == t.slug).map(|(_, r)| word(r)).unwrap_or_default();
            json!({"slug":t.slug,"name":task::first_name(t),"desc":t.description,"last":last,"params":t.params.iter().map(|p| json!({"name":p.name,"label":if p.label.is_empty() { &p.name } else { &p.label },"example":p.example})).collect::<Vec<_>>()})
        }).collect();
        json!({"items":items,"runs":runs.iter().take(8).map(|(id, r)| json!({"run":id,"slug":r.slug,"status":word(r)})).collect::<Vec<_>>()})
    }

    // ---- recorder events, expiry ----
    async fn event(&self, o: &Value) {
        let page = s(o, "page").to_string();
        match s(o, "event") {
            "record" => {
                let mut g = self.st.lock().await; let b = g.teach.get(&page);
                if b.recording() { let nb = b.add(&o["step"]); let line = nb.last(); self.set_teach(&mut g, nb).await; drop(g); self.draft_add(&page, &line) }
            }
            "shield-click" => { let mut g = self.st.lock().await; let nb = g.teach.get(&page).takeover(); if !nb.idle() { self.set_teach(&mut g, nb).await; } }
            _ => {}
        }
    }
    pub async fn tick(&self, now: u64) {
        let stopped: Vec<String> = { let mut g = self.st.lock().await; let ex = g.teach.expired(now); for bot in &ex { let nb = g.teach.get(bot).stop(); self.set_teach(&mut g, nb).await; } ex };
        // the helper is asked with the lock dropped: a stuck helper holds nothing else up
        for bot in stopped { self.br.ask(&bot, "record", &json!({"on":false})).await; }
    }

    // ---- the pause answer (the agents lane calls this when the ask is answered) ----
    pub async fn answered(&self, ask: &str, answer: &str) -> bool {
        let w = self.waiters.lock().unwrap().remove(ask); match w { Some((_, tx)) => { let _ = tx.send(answer.to_string()); true } None => false }
    }
    /// The person's answer also hands the browser back (TeachBack).
    async fn back(&self, bot: &str) { let mut g = self.st.lock().await; let nb = g.teach.get(bot).handback(); if nb.drive != g.teach.get(bot).drive { self.set_teach(&mut g, nb).await; } }

    // ---- the task_* tools ----
    pub async fn tool(self: &Arc<Self>, thread: &str, name: &str, a: &Value) -> (bool, Value) {
        let bot = thread; let now = now_s(); let fail = |m: &str| (false, json!({"error": m}));
        match name {
            "task_list" => { self.sync().await; let mut g = self.st.lock().await; let mut rows: Vec<(String, String)> = g.tasks.iter().filter(|((b, _), _)| b == bot).map(|((_, sl), j)| (sl.clone(), j.clone())).collect(); rows.sort();
                (true, Value::Array(rows.iter().map(|(sl, j)| match task::read(j) { Some(t) => json!({"slug":sl,"name":t.name,"description":t.description,"params":t.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>()}), None => json!({"slug":sl,"name":sl}) }).collect())) }
            "task_get" => { self.sync().await; let mut g = self.st.lock().await; match g.tasks.get(&(bot.to_string(), task::slug(s(a, "slug")))).and_then(|j| serde_json::from_str::<Value>(j).ok()) { Some(j) => (true, j), None => fail("no such task: task_list shows yours") } }
            "task_save" => {
                let slug = task::slug(if s(a, "slug").trim().is_empty() { s(a, "name") } else { s(a, "slug") });
                let existing = { self.sync().await; let mut g = self.st.lock().await; g.tasks.get(&(bot.to_string(), slug.clone())).cloned() };
                match teach::save(existing.as_deref(), a, now) { Ok((sl, js)) => { self.set_task(bot, &sl, &js, now).await; (true, json!({"ok":true,"slug":sl})) } Err(e) => fail(&e) }
            }
            "task_delete" => { let _ = self.task_rpc(&json!({"bot":bot,"op":"delete","slug":s(a, "slug")})).await; (true, Value::String(format!("deleted {}", task::slug(s(a, "slug"))))) }
            "task_run" => self.run(thread, a, now).await,
            "task_step" => self.step(thread, a, now).await,
            "task_pause" => self.pause(thread, a, now).await,
            "task_end" => self.end(thread, a, now).await,
            _ => fail("unknown tool"),
        }
    }
    pub(crate) fn run_dir(&self, bot: &str, run: &str) -> PathBuf { self.hub.cfg.home.join("bots").join(bot).join(teach::run_dir(run)) }
    async fn run(&self, thread: &str, a: &Value, now: u64) -> (bool, Value) {
        let bot = thread; let slug = task::slug(s(a, "slug")); let fail = |m: &str| (false, json!({"error": m}));
        if !Self::safe(bot) { return fail("no bot") }
        let js = { self.sync().await; let mut g = self.st.lock().await; g.tasks.get(&(bot.to_string(), slug.clone())).cloned() };
        let Some(js) = js else { return fail("no such task: task_list shows yours") };
        let Some(t) = task::read(&js) else { return fail("that task is damaged: save it again with task_save") };
        let ps = a.get("params").and_then(Value::as_object).cloned().unwrap_or_default();
        let filled = match task::fill(&t, &ps) { Ok(f) => f, Err(e) => return fail(&e) };
        let run = teach::run_id(&slug, now);
        { let mut g = self.st.lock().await; if g.runs.len() >= MAX_RUNS { let old: Vec<String> = g.runs.iter().filter(|(_, r)| matches!(r.state, Run::Ended(_))).map(|(k, _)| k.clone()).collect(); for k in old { g.runs.remove(&k); } }
          if g.runs.len() >= MAX_RUNS { return fail("too many task runs are open") } g.runs.insert(run.clone(), RunRec { bot: bot.into(), slug: slug.clone(), state: Run::Running(0) }); }
        let _ = std::fs::create_dir_all(self.run_dir(bot, &run));
        let _ = self.hub.commit(&json!({"$":"ActivityLogged","thread":thread,"act":format!("task:{run}"),"tone":"task","kind":"task","summary":teach::act_text(&slug, &run, "running", 0, "", &[]),"at":now})).await;
        (true, json!({"run": run, "plan": teach::plan(&filled)}))
    }
    async fn step(&self, thread: &str, a: &Value, now: u64) -> (bool, Value) {
        let bot = thread; let run = s(a, "run");
        if !teach::run_ok(run) || !Self::safe(bot) { return (false, json!({"error":"no such run"})) }
        let n = a.get("n").and_then(Value::as_u64).unwrap_or(0); let shot = a.get("shot").and_then(Value::as_bool).unwrap_or(false); let dir = self.run_dir(bot, run); let _ = std::fs::create_dir_all(&dir);
        append(&dir.join("log.jsonl"), &json!({"n":n,"ok":a.get("ok").and_then(Value::as_bool).unwrap_or(true),"note":s(a, "note").chars().take(500).collect::<String>(),"shot": if shot { teach::step_file(n) } else { String::new() },"at":now}).to_string());
        { let mut g = self.st.lock().await; if let Some(r) = g.runs.get_mut(run) { r.state = task::step(&r.state, &In::Advance); } }
        if shot { self.br.ask(bot, "screenshot", &json!({"path": dir.join(teach::step_file(n)).to_string_lossy(), "fullPage": false})).await; }
        (true, json!({"ok":true,"shot": if shot { format!("/bots/{bot}/tasks/shot?run={run}&f={}", teach::step_file(n)) } else { String::new() }}))
    }
    async fn pause(&self, thread: &str, a: &Value, now: u64) -> (bool, Value) {
        let bot = thread; let run = s(a, "run");
        if !teach::run_ok(run) || !Self::safe(bot) { return (false, json!({"error":"no such run"})) }
        let (n, kind, risky) = (a.get("n").and_then(Value::as_u64).unwrap_or(now), if s(a, "kind").is_empty() { "stuck" } else { s(a, "kind") }, a.get("risky").and_then(Value::as_bool).unwrap_or(false));
        let question: String = s(a, "question").chars().take(300).collect(); let dir = self.run_dir(bot, run); let _ = std::fs::create_dir_all(&dir);
        self.br.ask(bot, "screenshot", &json!({"path": dir.join(teach::pause_file(n)).to_string_lossy(), "fullPage": false})).await;
        let ask = format!("q{now}-{}", self.hub.store.len()); let (tx, rx) = oneshot::channel();
        // parked: at most MAX_WAITERS per thread; the guard takes it out again however this future ends (an agent stopped, its connection gone)
        { let mut w = self.waiters.lock().unwrap(); if w.values().filter(|(t, _)| t == thread).count() >= MAX_WAITERS { return (false, json!({"error":"too many questions are waiting for the person"})) } w.insert(ask.clone(), (thread.to_string(), tx)); }
        let _guard = WaiterGuard { d: self, ask: ask.clone() };
        { let mut g = self.st.lock().await; if let Some(r) = g.runs.get_mut(run) { r.state = task::step(&r.state, &In::Pause { kind: kind.into(), risky }); } }
        // the ask and the thread's pending count go out together (Hub.step's Pend.fix), so the thread reads as waiting on the person
        let pend = self.hub.model().await.threads.get(thread).map_or(0, |t| t.pending) + 1;
        let slug = teach::run_slug(run);
        if self.hub.commit_many(&[json!({"$":"AskOpened","id":ask,"thread":thread,"kind":"browser","tool":"task_pause","detail":teach::pause_detail(bot, run, &slug, n, kind, &question, risky),"options":teach::pause_options(risky),"at":now}),
            json!({"$":"PendingChanged","thread":thread,"count":pend,"at":now})]).await.is_err() { return (false, json!({"error":"could not ask the person"})) }
        let answer = match tokio::time::timeout(std::time::Duration::from_millis(self.pause_ms.load(std::sync::atomic::Ordering::Relaxed)), rx).await {
            Ok(Ok(x)) => x,
            _ => { // nobody answered: the ask is closed so it does not stay open for an answer nobody hears
                let pend = self.hub.model().await.threads.get(thread).map_or(0, |t| t.pending).saturating_sub(1);
                let _ = self.hub.commit_many(&[json!({"$":"AskClosed","id":ask,"answer":"","at":now_s()}), json!({"$":"PendingChanged","thread":thread,"count":pend,"at":now_s()})]).await;
                return (false, json!({"error":"nobody answered the question"}))
            }
        };
        { let mut g = self.st.lock().await; if let Some(r) = g.runs.get_mut(run) { r.state = task::step(&r.state, &In::Answer(answer.clone())); } }
        self.back(bot).await;
        (true, Value::String(teach::pause_reply(&answer)))
    }
    async fn end(&self, thread: &str, a: &Value, now: u64) -> (bool, Value) {
        let bot = thread; let run = s(a, "run");
        if !teach::run_ok(run) || !Self::safe(bot) { return (false, json!({"error":"no such run"})) }
        let (status, summary) = (if s(a, "status").is_empty() { "ok" } else { s(a, "status") }, s(a, "summary").to_string()); let dir = self.run_dir(bot, run);
        append(&dir.join("log.jsonl"), &json!({"end":true,"status":status,"summary":summary.chars().take(500).collect::<String>(),"at":now}).to_string());
        // the oldest runs are pruned (the newest KEEP_RUNS stay), then the final Act with the log's length and the pictures
        if let Some(parent) = dir.parent() { prune(parent, KEEP_RUNS) }
        let n = std::fs::read_to_string(dir.join("log.jsonl")).map(|t| t.lines().count() as u64).unwrap_or(0);
        let mut shots: Vec<String> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|f| f.ends_with(".jpg") && teach::file_ok(f)).collect()).unwrap_or_default(); shots.sort();
        let urls: Vec<String> = shots.iter().map(|f| format!("/bots/{bot}/tasks/shot?run={run}&f={f}")).collect();
        let _ = self.hub.commit(&json!({"$":"ActivityLogged","thread":thread,"act":format!("task:{run}.end"),"tone":"task","kind":"task","summary":teach::act_text(&teach::run_slug(run), run, status, n, &summary, &urls),"at":now})).await;
        { let mut g = self.st.lock().await; if let Some(r) = g.runs.get_mut(run) { r.state = task::step(&r.state, &In::Finish(status.into())); } }
        (true, Value::String("run ended".into()))
    }
}
/// Takes a parked pause out when the tool's future ends or is dropped.
struct WaiterGuard<'a> { d: &'a Driver, ask: String }
impl Drop for WaiterGuard<'_> { fn drop(&mut self) { self.d.waiters.lock().unwrap().remove(&self.ask); } }
fn append(path: &std::path::Path, line: &str) { if let Some(d) = path.parent() { let _ = std::fs::create_dir_all(d); } if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) { use std::io::Write; let _ = writeln!(f, "{line}"); } }
/// Keep the newest `keep` entries of a folder (by modification time); remove the rest.
fn prune(dir: &std::path::Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut es: Vec<(std::time::SystemTime, PathBuf)> = rd.filter_map(|e| e.ok()).filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect(); es.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in es.into_iter().skip(keep) { let _ = std::fs::remove_dir_all(p); }
}

/// GET /bots/<id>/tasks/shot?run=<run>&n=<n> (or &f=<file>): a picture of a task run (a pause's, or a step's), only from that bot's own tasks folder; the
/// bot, run and file names are plain (no / or ..). None: not this route. The hub already checked the pairing token.
pub async fn tasks_http(hub: &Arc<Hub>, req: &Req) -> Option<HttpReply> {
    let route = req.route(); let rest = route.strip_prefix("/bots/")?;
    if let Some(owner) = rest.strip_suffix("/screen.jpg") { return Some(screen(hub, owner).await) }
    let bot = rest.strip_suffix("/tasks/shot")?;
    let nf = || Some(HttpReply::text("404 Not Found", "no such picture"));
    let (run, f) = (crate::files::fm::from_uri(&req.param("run")), crate::files::fm::from_uri(&req.param("f")));
    let file = if f.is_empty() { teach::pause_file(req.param("n").parse().unwrap_or(0)) } else { f };
    if !teach::part_ok(bot, false) || !teach::run_ok(&run) || !teach::file_ok(&file) { return nf() }
    let path = hub.cfg.home.join("bots").join(bot).join(teach::run_dir(&run)).join(&file);
    match tokio::task::spawn_blocking(move || { let m = std::fs::metadata(&path).ok()?; if m.len() > 8 << 20 { return None } std::fs::read(&path).ok() }).await.ok().flatten() {
        Some(b) => Some(HttpReply::ok(if file.ends_with(".png") { "image/png" } else { "image/jpeg" }, b)), None => nf(),
    }
}
/// GET /bots/<id>/screen.jpg: the page's newest browser frame as the helper wrote it (`<home>/shots/<owner>.jpg`; an owner's file name keeps [A-Za-z0-9_-], the
/// rest becomes '_'), never cached. The Browser tab of every client shows it, a new one announced by `{"t":"shot","bot":<owner>,"n":N}`. 404 "no screen yet".
async fn screen(hub: &Arc<Hub>, owner: &str) -> HttpReply {
    if !teach::part_ok(owner, false) { return HttpReply::text("404 Not Found", "no such bot") }
    let file: String = owner.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let path = hub.cfg.home.join("shots").join(format!("{file}.jpg"));
    match tokio::task::spawn_blocking(move || { let m = std::fs::metadata(&path).ok()?; if m.len() > 8 << 20 { return None } std::fs::read(&path).ok() }).await.ok().flatten() {
        Some(b) => HttpReply { status: "200 OK", ctype: "image/jpeg".into(), extra: "Cache-Control: no-store\r\n".into(), body: b },
        None => HttpReply::text("404 Not Found", "no screen yet"),
    }
}
/// The one "/bots" route this lane serves. The bots lane's own `/bots` handler replaces this one and must call `tasks_http` first.
struct BotsHttp;
impl HttpHandler for BotsHttp {
    fn routes(&self) -> &'static [&'static str] { &["/bots"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> { Box::pin(async move { match tasks_http(hub, req).await { Some(r) => r, None => HttpReply::text("404 Not Found", "not found") } }) }
}

struct Rpc(Arc<Driver>);
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["bots.browser", "bots.teach", "bots.task"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            match method {
                "bots.browser" => match self.0.browser(p, who.client, id).await { Some(Ok(())) => ok(id), Some(Err(e)) => err(id, &e), None => vec![] },
                "bots.teach" => match self.0.teach(p).await { Ok(_) => ok(id), Err(e) => err(id, &e) },
                _ => match self.0.task_rpc(p).await { Ok(_) => ok(id), Err(e) => err(id, &e) },
            }
        })
    }
}

#[cfg(test)]
mod tests;

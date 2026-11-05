//! The teach and task driver (src/server/server.bend's "Teaching a bot's browser" and "Task runs", docs/teach.md): `bots.browser`, `bots.teach`,
//! `bots.task` requests, the task_* tools, recording events, run folders and the pause ask. The pure rules are task.rs and teach.rs.
//! A "bot" here is the page owner (a thread's id until the bots lane says which threads are a bot's). Tasks are folded from the log's
//! TaskSet/TaskDeleted events (a copy of each goes to <home>/bots/<bot>/tasks/<slug>.json). Bounded: MAX_RUNS runs in memory, MAX_WAITERS parked
//! pauses, a run keeps its last 20 folders, the draft file is capped with the draft (400 steps), tool arguments are cut as in Bend.
//! Seams for other lanes: `Driver::set_wake` (the bots lane wakes a bot with a message), `Driver::answered` (the agents lane calls it when an ask of kind
//! browser/tool task_pause is answered), `Driver::info` (for a joining client's info).
use super::{browser::Browser, now_s, task::{self, In, Run}, teach::{self, Teach, TeachBot}};
use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, sync::{Arc, OnceLock}};
use tokio::sync::{oneshot, Mutex};

pub const MAX_RUNS: usize = 256;
pub const MAX_WAITERS: usize = 16;
pub const KEEP_RUNS: usize = 20;
const PAUSE_WAIT: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

struct RunRec { bot: String, slug: String, state: Run }
#[derive(Default)]
struct DState { teach: Teach, tasks: HashMap<(String, String), String>, seen: usize, runs: HashMap<String, RunRec>, waiters: HashMap<String, (String, oneshot::Sender<String>)> }
pub type Wake = Arc<dyn Fn(&str, String) + Send + Sync>;
pub struct Driver { hub: Arc<Hub>, br: Arc<Browser>, st: Mutex<DState>, wake: std::sync::Mutex<Option<Wake>> }

static DRIVERS: OnceLock<std::sync::Mutex<HashMap<PathBuf, Arc<Driver>>>> = OnceLock::new();
pub fn get(hub: &Hub) -> Option<Arc<Driver>> { DRIVERS.get()?.lock().unwrap().get(&hub.cfg.home).cloned() }

pub fn start(hub: &Arc<Hub>, br: Arc<Browser>) {
    let d = Arc::new(Driver { hub: hub.clone(), br: br.clone(), st: Mutex::new(DState::default()), wake: Default::default() });
    DRIVERS.get_or_init(Default::default).lock().unwrap().insert(hub.cfg.home.clone(), d.clone());
    hub.register_rpc(Arc::new(Rpc(d.clone())));
    // the helper's recorder steps and shield presses; a recording ends after ten minutes
    let (e, mut ev) = (d.clone(), br.events());
    tokio::spawn(async move { loop { match ev.recv().await { Ok(o) => e.event(&o).await, Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue, Err(_) => break } } });
    let t = d.clone();
    tokio::spawn(async move { let mut iv = tokio::time::interval(std::time::Duration::from_secs(10)); loop { iv.tick().await; t.tick(now_s()).await } });
}

fn ok(id: &Value) -> Vec<Value> { vec![json!({"t":"reply","id":id,"ok":true})] }
fn err(id: &Value, m: &str) -> Vec<Value> { vec![json!({"t":"reply","id":id,"ok":false,"error":m})] }
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(Value::as_str).unwrap_or("") }

impl Driver {
    pub fn set_wake(&self, w: Wake) { *self.wake.lock().unwrap() = Some(w) }
    fn wake(&self, bot: &str, text: String) { if let Some(w) = self.wake.lock().unwrap().clone() { w(bot, text) } }
    fn dir(&self, bot: &str) -> PathBuf { self.hub.cfg.home.join("bots").join(bot).join("tasks") }
    fn safe(bot: &str) -> bool { teach::part_ok(bot, false) }
    /// A joining client's info: every bot with something to say.
    pub async fn info(&self) -> Vec<(String, String)> { self.st.lock().await.teach.info() }

    /// Fold the log's new TaskSet/TaskDeleted lines (the first call reads them all, once).
    async fn sync(&self, g: &mut DState) {
        let n = self.hub.store.len(); if g.seen >= n { return }
        let from = g.seen; g.seen = n;
        let mut a = from; while a < n {
            let b = (a + 4000).min(n);
            for line in self.hub.store.range(a, b).unwrap_or_default() {
                if !(line.contains("\"TaskSet\"") || line.contains("\"TaskDeleted\"")) { continue }
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                let key = (s(&v, "bot").to_string(), s(&v, "slug").to_string());
                match s(&v, "$") { "TaskSet" => { g.tasks.insert(key, s(&v, "json").to_string()); } "TaskDeleted" => { g.tasks.remove(&key); } _ => {} }
            }
            a = b;
        }
    }
    async fn put_drive(&self, b: &TeachBot) { self.br.set_drive(&b.bot, &b.drive).await; self.hub.push_all(&b.msg()) }
    async fn set_teach(&self, g: &mut DState, b: TeachBot) -> bool { let okk = g.teach.put(b.clone()); if okk { self.put_drive(&b).await } okk }

    // ---- bots.browser ----
    async fn browser(&self, p: &Value) -> Result<(), String> {
        let bot = s(p, "bot"); let op = s(p, "op");
        if bot.is_empty() { return Err("no bot".into()) }
        let mut g = self.st.lock().await; let b = g.teach.get(bot);
        match op {
            "takeover" => { let nb = b.takeover(); if !self.set_teach(&mut g, nb).await { return Err("too many pages are taken over".into()) } Ok(()) }
            "handback" => { let nb = b.handback(); self.set_teach(&mut g, nb).await; Ok(()) }
            "click" | "mouse" | "type" | "press" | "scroll" | "navigate" | "back" | "reload" => {
                if b.drive.is_empty() { return Err("the bot has the browser; take it over first".into()) }
                drop(g);
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
                self.br.ask(bot, op, &args).await; Ok(())
            }
            _ => Err("unknown browser op".into()),
        }
    }
    /// The page's size from a status reply (the viewport a click lands in).
    async fn viewport(&self, bot: &str) -> (u64, u64) {
        let l: Value = serde_json::from_str(&self.br.ask(bot, "status", &json!({})).await).unwrap_or(Value::Null); let r = l.get("result").cloned().unwrap_or(Value::Null);
        (r.get("width").and_then(Value::as_u64).unwrap_or(1280), r.get("height").and_then(Value::as_u64).unwrap_or(800))
    }

    // ---- bots.teach ----
    async fn teach(&self, p: &Value) -> Result<Value, String> {
        let bot = s(p, "bot"); let op = s(p, "op"); let now = now_s();
        if bot.is_empty() || !Self::safe(bot) { return Err("no bot".into()) }
        let mut g = self.st.lock().await; let b = g.teach.get(bot);
        match op {
            "start" => {
                if !b.idle() { return Err("the browser is busy: finish or discard what is on it".into()) }
                let nb = TeachBot::started(bot, now); if !self.set_teach(&mut g, nb).await { return Err("too many pages are taken over".into()) }
                drop(g); let _ = std::fs::remove_file(self.dir(bot).join(".draft.jsonl")); self.br.ask(bot, "record", &json!({"on":true})).await; Ok(json!({}))
            }
            "note" => {
                let nb = b.add(&teach::note(&s(p, "text").chars().take(2000).collect::<String>())); let line = nb.last(); self.set_teach(&mut g, nb).await; drop(g); self.draft_add(bot, &line); Ok(json!({}))
            }
            "stop" => { let nb = b.stop(); self.set_teach(&mut g, nb).await; drop(g); self.br.ask(bot, "record", &json!({"on":false})).await; Ok(json!({})) }
            "discard" => { self.set_teach(&mut g, TeachBot::blank(bot)).await; drop(g); let _ = std::fs::remove_file(self.dir(bot).join(".draft.jsonl")); self.br.ask(bot, "record", &json!({"on":false})).await; Ok(json!({})) }
            "save" => {
                if b.mode.is_empty() { return Err("there is no recording to save".into()) }
                let name = s(p, "name").trim().to_string(); if name.is_empty() { return Err("name the task".into()) }
                let marks: Vec<Value> = p.get("params").and_then(Value::as_array).cloned().unwrap_or_default();
                let t = teach::build(&b, &name, s(p, "description"), &marks, now);
                if t.steps.len() > 200 { return Err("at most 200 steps".into()) }
                self.set_teach(&mut g, TeachBot::blank(bot)).await; drop(g);
                let (slug, js) = (t.slug.clone(), task::json(&t).to_string());
                self.set_task(bot, &slug, &js, now).await;
                self.br.ask(bot, "record", &json!({"on":false})).await; let _ = std::fs::remove_file(self.dir(bot).join(".draft.jsonl"));
                self.wake(bot, teach::taught(&name, &t)); Ok(json!({"slug": slug}))
            }
            _ => Err("unknown teach op".into()),
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
        { let mut g = self.st.lock().await; self.sync(&mut g).await; }
        if Self::safe(bot) && teach::part_ok(slug, false) { let d = self.dir(bot); let _ = std::fs::create_dir_all(&d); let _ = std::fs::write(d.join(format!("{slug}.json")), js); }
    }
    async fn task_rpc(&self, p: &Value) -> Result<Value, String> {
        let bot = s(p, "bot"); let slug = task::slug(s(p, "slug")); let now = now_s();
        match s(p, "op") {
            "delete" => { let _ = self.hub.commit(&json!({"$":"TaskDeleted","bot":bot,"slug":slug,"at":now})).await; let mut g = self.st.lock().await; self.sync(&mut g).await; if Self::safe(bot) && teach::part_ok(&slug, false) { let _ = std::fs::remove_file(self.dir(bot).join(format!("{slug}.json"))); } Ok(json!({})) }
            "run" => {
                let t = { let mut g = self.st.lock().await; self.sync(&mut g).await; g.tasks.get(&(bot.to_string(), slug.clone())).cloned() };
                if t.is_none() { return Err("no such task".into()) }
                let params = match p.get("params") { Some(v) if v.is_object() => serde_json::to_string(v).unwrap_or_default(), _ => "{}".into() };
                self.wake(bot, teach::run_msg(&slug, &params)); Ok(json!({}))
            }
            _ => Err("unknown task op".into()),
        }
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
        let mut g = self.st.lock().await;
        for bot in g.teach.expired(now) { let nb = g.teach.get(&bot).stop(); self.set_teach(&mut g, nb).await; self.br.ask(&bot, "record", &json!({"on":false})).await; }
    }

    // ---- the pause answer (the agents lane calls this when the ask is answered) ----
    pub async fn answered(&self, ask: &str, answer: &str) -> bool {
        let w = self.st.lock().await.waiters.remove(ask); match w { Some((_, tx)) => { let _ = tx.send(answer.to_string()); true } None => false }
    }
    /// The person's answer also hands the browser back (TeachBack).
    async fn back(&self, bot: &str) { let mut g = self.st.lock().await; let nb = g.teach.get(bot).handback(); if nb.drive != g.teach.get(bot).drive { self.set_teach(&mut g, nb).await; } }

    // ---- the task_* tools ----
    pub async fn tool(self: &Arc<Self>, thread: &str, name: &str, a: &Value) -> (bool, Value) {
        let bot = thread; let now = now_s(); let fail = |m: &str| (false, json!({"error": m}));
        match name {
            "task_list" => { let mut g = self.st.lock().await; self.sync(&mut g).await; let mut rows: Vec<(String, String)> = g.tasks.iter().filter(|((b, _), _)| b == bot).map(|((_, sl), j)| (sl.clone(), j.clone())).collect(); rows.sort();
                (true, Value::Array(rows.iter().map(|(sl, j)| match task::read(j) { Some(t) => json!({"slug":sl,"name":t.name,"description":t.description,"params":t.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>()}), None => json!({"slug":sl,"name":sl}) }).collect())) }
            "task_get" => { let mut g = self.st.lock().await; self.sync(&mut g).await; match g.tasks.get(&(bot.to_string(), task::slug(s(a, "slug")))).and_then(|j| serde_json::from_str::<Value>(j).ok()) { Some(j) => (true, j), None => fail("no such task: task_list shows yours") } }
            "task_save" => {
                let slug = task::slug(if s(a, "slug").trim().is_empty() { s(a, "name") } else { s(a, "slug") });
                let existing = { let mut g = self.st.lock().await; self.sync(&mut g).await; g.tasks.get(&(bot.to_string(), slug.clone())).cloned() };
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
    fn run_dir(&self, bot: &str, run: &str) -> PathBuf { self.hub.cfg.home.join("bots").join(bot).join(teach::run_dir(run)) }
    async fn run(&self, thread: &str, a: &Value, now: u64) -> (bool, Value) {
        let bot = thread; let slug = task::slug(s(a, "slug")); let fail = |m: &str| (false, json!({"error": m}));
        if !Self::safe(bot) { return fail("no bot") }
        let js = { let mut g = self.st.lock().await; self.sync(&mut g).await; g.tasks.get(&(bot.to_string(), slug.clone())).cloned() };
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
        { let mut g = self.st.lock().await; if g.waiters.values().filter(|(t, _)| t == thread).count() >= MAX_WAITERS { return (false, json!({"error":"too many questions are waiting for the person"})) }
          g.waiters.insert(ask.clone(), (thread.to_string(), tx));
          if let Some(r) = g.runs.get_mut(run) { r.state = task::step(&r.state, &In::Pause { kind: kind.into(), risky }); } }
        let slug = teach::run_slug(run);
        let _ = self.hub.commit(&json!({"$":"AskOpened","id":ask,"thread":thread,"kind":"browser","tool":"task_pause","detail":teach::pause_detail(bot, run, &slug, n, kind, &question, risky),"options":teach::pause_options(risky),"at":now})).await;
        let answer = match tokio::time::timeout(PAUSE_WAIT, rx).await { Ok(Ok(x)) => x, _ => { self.st.lock().await.waiters.remove(&ask); return (false, json!({"error":"nobody answered the question"})) } };
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
fn append(path: &std::path::Path, line: &str) { if let Some(d) = path.parent() { let _ = std::fs::create_dir_all(d); } if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) { use std::io::Write; let _ = writeln!(f, "{line}"); } }
/// Keep the newest `keep` entries of a folder (by modification time); remove the rest.
fn prune(dir: &std::path::Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut es: Vec<(std::time::SystemTime, PathBuf)> = rd.filter_map(|e| e.ok()).filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect(); es.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, p) in es.into_iter().skip(keep) { let _ = std::fs::remove_dir_all(p); }
}

struct Rpc(Arc<Driver>);
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["bots.browser", "bots.teach", "bots.task"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            match method {
                "bots.browser" => match self.0.browser(p).await { Ok(()) => ok(id), Err(e) => err(id, &e) },
                "bots.teach" => match self.0.teach(p).await { Ok(_) => ok(id), Err(e) => err(id, &e) },
                _ => match self.0.task_rpc(p).await { Ok(_) => ok(id), Err(e) => err(id, &e) },
            }
        })
    }
}

#[cfg(test)]
mod tests;

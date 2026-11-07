//! M5: agent process management. One actor per thread owns that thread's agent process (Claude: one
//! long-lived stream-json process), turns the lines it writes into the events the JS hub writes, and commits
//! them through `Hub::commit_many`. Ported from src/core/hub (Hub.post.go, Claude.line.k, Retire.turn.f,
//! Hub.agent_gone, Hub.recover, Carry.*) and the Claude paths in src/server/server.
//!
//! Ported: Claude. Out (answered with an error, never silently): Codex, Grok, OpenCode, Muse, Antigravity, Pi, Oh My Pi.
//! Out for Claude: images/`$skill` input, the Backplane MCP server and runtime prompt in the agent's arguments,
//! subagents and background tasks, todo tools, the allowance stream, the legacy queue, mail provenance beyond carry,
//! and the hub's time-driven policies (liveness, autosettle, alert/told/summary, star nudge), which belong to M4.
pub mod mailroom; mod devturns; pub mod bg; pub mod refs; pub mod person; pub mod work; pub mod textgen; pub mod launch; pub mod review; pub mod tasks; pub mod codex; pub mod providers; pub mod cmds; pub mod hist; pub mod host; pub mod inputs; pub mod skills; pub mod lines; pub mod mcp; pub mod proc; pub mod prompt; pub mod prompt_text; pub mod rules; pub mod tools; pub mod wake; pub mod world;
#[cfg(test)] mod tests;
#[cfg(test)] mod laws;

use crate::{api::{Caller, Fut, RpcHandler}, hub::Hub};
use anyhow::Result;
use lines::{hub_id, Cx, Flow};
use proc::{Cli, Exit};
use serde_json::{json, Value};
use std::{collections::{HashMap, VecDeque}, sync::{atomic::{AtomicU64, Ordering::Relaxed}, Arc, Mutex as StdMutex}, time::{Duration, Instant}};
use tokio::sync::{mpsc, oneshot, Mutex};
use world::World;

pub const CMD_QUEUE: usize = 64;      // messages waiting for one thread's actor
pub const BATCH_BYTES: usize = 64 << 10; // "one read" of the agent's output (Delta.merge)
pub const LIVE_CAP: usize = 262144;   // Live.cap: a thread's streamed-but-unposted text
pub const BURST: usize = 4;           // Rec.burst: turns a restart carries on at once
pub const CARRY_GAP_S: u64 = 3;       // the rest follow one at a time, this far apart
pub const RECENT: usize = 256;        // message ids remembered per thread against a resend

#[derive(Default)]
pub struct Stats { pub hist_dropped: AtomicU64, pub jobs: AtomicU64, pub turns: AtomicU64, pub procs_spawned: AtomicU64, pub procs_exited: AtomicU64, pub spawn_failures: AtomicU64, pub lines: AtomicU64, pub deltas: AtomicU64,
    pub events: AtomicU64, pub commit_errors: AtomicU64, pub interrupts: AtomicU64, pub refused_full: AtomicU64, pub bad_lines: AtomicU64, pub max_batch_lines: AtomicU64 }

pub enum Cmd {
    User { person: bool, msg: String, text: String, role: String, origin: Option<(String, String, String)>, extra: Vec<Value>, reply: oneshot::Sender<Result<(), String>> },
    Interrupt { reply: oneshot::Sender<()> },
    /// The person answered an ask a driver's process made (providers::AskInfo).
    Answer { ask: String, answer: String },
    /// AgentStop: end the thread's agent process in any state, no events.
    Stop { reply: oneshot::Sender<()> },
    /// Wt.start: queued text runs as one turn (the caller logged the queue events it owns).
    StartQueued { text: String, taken: bool, reply: oneshot::Sender<Result<(), String>> },
    /// Steer.go: a running non-Claude turn stops and starts again with the text; Claude takes it in place.
    /// Wake.ride: add a message to the thread's ride (see `Agents::wake_ride`).
    WakeRide { id: String, text: String, start: bool, reply: oneshot::Sender<Result<(), String>> },
    Steer { text: String, reply: oneshot::Sender<Result<(), String>> },
}

/// The hub's one agent manager, for hub-side callers that were not handed it (commands, bots).
static HANDLE: std::sync::OnceLock<Arc<Agents>> = std::sync::OnceLock::new();
pub use textgen::{summarizer, Summarizer};
/// Mark the turn ends runs Ended.out for with `"_end": true` for state's policy (which writes alert/told/summary.at and removes the field before the write).
/// On: state's policy (statehost, `_end` handling) is on the tree and strips the field before the write.
const MARK_ENDS: bool = true;
/// A design-history job in flight (checkpoint, step): counted so a test, or a shutdown, can wait for what a turn's end left running.
pub(crate) struct JobGuard(Arc<Agents>);
impl JobGuard { fn new(a: &Arc<Agents>) -> JobGuard { a.stats.jobs.fetch_add(1, Relaxed); JobGuard(a.clone()) } }
impl Drop for JobGuard { fn drop(&mut self) { self.0.stats.jobs.fetch_sub(1, Relaxed); } }
pub fn handle() -> Option<Arc<Agents>> { HANDLE.get().cloned() }

pub struct Agents { carry_gone: std::sync::atomic::AtomicBool, pub copy_point: StdMutex<(Vec<(String, String, bool)>, Vec<String>)>, pub works: StdMutex<HashMap<String, work::Ctl>>, pub work_lock: tokio::sync::Mutex<()>, pub tool_sets: StdMutex<Vec<Arc<dyn mcp::ToolSet>>>, pub stopped_by: StdMutex<HashMap<String, String>>, pub hub: Arc<Hub>, pub cli: Cli, pub host: host::HostInfo, world: Mutex<World>, actors: StdMutex<HashMap<String, mpsc::Sender<Cmd>>>, pub stats: Stats, n: AtomicU64,
    pub driver_asks: StdMutex<HashMap<String, String>>, pub mcp: std::sync::OnceLock<Arc<mcp::Mcp>>, gens: StdMutex<HashMap<String, String>>, pub parked: StdMutex<HashMap<String, mcp::Parked>>, pub order_asks: StdMutex<HashMap<String, crate::settings::order::Asked>>, pub mailroom: std::sync::OnceLock<Arc<dyn mailroom::Mailroom>>, /** the account manager (set by the accounts mod): a thread bound to a named account is launched through it */ pub accounts: std::sync::OnceLock<Arc<crate::settings::accounts_io::Accounts>>, cut: StdMutex<Vec<String>> }

pub fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }
pub fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }
fn ok(id: &Value) -> Value { json!({"t":"reply","id":id,"ok":true}) }
fn err(id: &Value, e: &str) -> Value { json!({"t":"reply","id":id,"ok":false,"error":e}) }
fn set(k: String, v: String, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }
/// a thread a restart cut, waiting in line to carry on
fn wait_key(th: &str) -> String { format!("carry.wait.{th}") }

impl Agents {
    pub async fn start(hub: Arc<Hub>) -> Result<Arc<Agents>> { let h = host::HostInfo::probe(&hub.cfg.home); Agents::start_host(hub, Cli::from_env(), h).await }
    pub async fn start_with(hub: Arc<Hub>, cli: Cli) -> Result<Arc<Agents>> { Agents::start_host(hub, cli, host::HostInfo::default()).await }
    pub async fn start_host(hub: Arc<Hub>, cli: Cli, host: host::HostInfo) -> Result<Arc<Agents>> {
        let no_dir = cli.dir().is_none();
        let ag = Arc::new(Agents { carry_gone: Default::default(), hub: hub.clone(), cli, host, world: Mutex::new(crate::ckpt::take("agents", &hub.store, || World::load(&hub.store)).unwrap_or_default()), actors: StdMutex::new(HashMap::new()), stats: Stats::default(), n: AtomicU64::new(0),
            mailroom: Default::default(), accounts: std::sync::OnceLock::new(), cut: StdMutex::new(vec![]), driver_asks: StdMutex::new(HashMap::new()), order_asks: StdMutex::new(HashMap::new()), stopped_by: StdMutex::new(HashMap::new()), tool_sets: StdMutex::new(vec![]), works: StdMutex::new(HashMap::new()), copy_point: StdMutex::new((vec![], vec![])), work_lock: tokio::sync::Mutex::new(()), mcp: std::sync::OnceLock::new(), gens: StdMutex::new(HashMap::new()), parked: StdMutex::new(HashMap::new()) });
        ag.world.lock().await.home = hub.cfg.home.to_string_lossy().into_owned();
        // reap BEFORE the owner mark is set: marked children of this process (another test's, in a shared test binary) must not be taken for a dead hub's
        if no_dir && !crate::copy::on() { tokio::task::spawn_blocking(|| { let _ = proc::search_dirs(); }); } // the login shell's PATH is read now, off the workers, not inside the person's first send
        crate::startup::mark("agents: world loaded");
        { let a = ag.clone(); crate::ckpt::register(move |store| a.world.blocking_lock().save(store)); }
        let home = hub.cfg.home.to_string_lossy().into_owned(); let h2 = home.clone();
        let left = tokio::task::spawn_blocking(move || proc::reap_orphans(&h2)).await.unwrap_or(0); proc::set_owner(&home);
        if left > 0 { eprintln!("agents: ended {left} process(es) a previous hub left behind") }
        crate::startup::mark("agents: orphans reaped");
        { // bots live in ~/.backplane/bots/<bot>: copy what older builds kept elsewhere (folder, session history, memory) before any turn resumes
            let moves: Vec<(String, String)> = { let w = ag.world.lock().await; w.bots.iter().filter(|(_, x)| !x.2).map(|(id, x)| (id.clone(), w.threads.get(&x.1).map(|t| t.worktree.clone()).unwrap_or_default())).collect() };
            let h = hub.cfg.home.clone();
            for l in tokio::task::spawn_blocking(move || crate::bots::relocate::run(&h, &moves)).await.unwrap_or_default() { eprintln!("{l}") }
        }
        let _ = ag.mcp.set(mcp::Mcp::start(&ag).await?);
        crate::startup::mark("agents: mcp started");
        if crate::copy::person_turn_allowed() { let w = ag.world().await; let n = hub.store.len(); let last = if n > 0 { hub.store.range(n.saturating_sub(50), n).unwrap_or_default() } else { vec![] }; *ag.copy_point.lock().unwrap() = (w.threads.iter().map(|(id, x)| (id.clone(), w.cwd(id), x.turn == "running")).collect(), last); } // threads running at the copy point: the live hub may still be running them
        if crate::copy::person_turn_allowed() { let cleared: Vec<Value> = { let w = ag.world().await; w.threads.keys().filter(|t| !w.setting(&format!("ride.{t}")).is_empty()).map(|t| set(format!("ride.{t}"), String::new(), now_s())).collect() }; if !cleared.is_empty() { let _ = hub.commit_many(&cleared).await; } } // mail copied from the real log is not this app's to deliver to a fresh agent
        let repair = { let w = ag.world().await; tasks::repair_plan(&w) }; // Task.repair judges the state the hub started from, before recovery
        crate::startup::mark("agents: before recover");
        ag.recover().await;
        crate::startup::mark("agents: recovered");
        let _ = HANDLE.set(ag.clone()); tasks::watch(&ag); { let a = ag.clone(); tokio::spawn(async move { a.work_rearm().await }); } if !repair.is_empty() { let a = ag.clone(); tokio::spawn(async move { a.task_repair_run(repair).await }); }
        hub.register_rpc(Arc::new(Rpc(ag.clone())));
        hub.register_http(Arc::new(devturns::DevTurns(ag.clone())));
        wake::Wakes::start(ag.clone()).await; // wake_on / wake_list / wake_cancel
        Ok(ag)
    }

    /// The world, caught up with the log.
    pub async fn world(&self) -> tokio::sync::MutexGuard<'_, World> {
        let mut w = self.world.lock().await;
        { if let Err(e) = w.sync(&self.hub.store) { eprintln!("agents: reading the log: {e:#}") } }
        w
    }
    /// Hub.prompt for one thread: the host's facts, the project's files, the settings that change the words.
    pub async fn prompt_for(&self, th: &str, cwd: &str) -> String {
        let bot = match self.mailroom() { Some(m) => m.prompt(th).await, None => String::new() }; // Bots.prompt
        format!("{}{bot}", self.prompt_base(th, cwd).await)
    }
    async fn prompt_base(&self, th: &str, cwd: &str) -> String {
        let (ipc, orders, lang, buy, names) = { let w = self.world().await;
            (w.setting("kicad.ipc") != "off", w.setting("order.enabled") != "off", w.setting("agent.lang").to_string(), !w.setting(&format!("buy.{th}")).is_empty(), ()) };
        let _ = names;
        let (root, home, host) = (cwd.to_string(), self.host.home.clone(), self.host.clone());
        tokio::task::spawn_blocking(move || {
            let f = inputs::read(&root); let keys_path = home.join("secrets/api.env");
            let keys = prompt::api_keys(&keys_path.to_string_lossy(), &prompt::key_names(&std::fs::read_to_string(&keys_path).unwrap_or_default())) + &prompt::secrets(&crate::settings::vault::names());
            prompt::build(&prompt::PromptEnv { fc: Some(crate::files::kicad::stepglb::freecad_setting()).filter(|s| !s.is_empty()).unwrap_or_else(|| host.fc.clone()), render: host.render.clone(), kicad: host.kicad.clone(), kicad_ipc: ipc, models: host.models_text(), may_delegate: true, orders_on: orders,
                root, pro: f.pro, pcb: f.pcb, sch: f.sch, all: f.all, keys, machines: String::new(), buy, lang })
        }).await.unwrap_or_default()
    }
    /// PendingChanged: the number of open asks on a thread, written after an ask opens or closes (`extra` asks counted on top).
    pub async fn pending_ev(&self, thread: &str, extra: usize, skip: &str, at: u64) -> Value {
        let n = { let w = self.world().await; w.asks.iter().filter(|(id, a)| a.thread == thread && id.as_str() != skip).count() };
        json!({"$":"PendingChanged","thread":thread,"count":n + extra,"at":at})
    }
    fn seq(&self) -> u64 { self.n.fetch_add(1, Relaxed) + 1 }

    fn actor(self: &Arc<Self>, th: &str) -> mpsc::Sender<Cmd> {
        let mut m = self.actors.lock().unwrap();
        if let Some(t) = m.get(th) { if !t.is_closed() { return t.clone() } }
        let (tx, rx) = mpsc::channel(CMD_QUEUE); m.insert(th.into(), tx.clone());
        let (ag, th) = (self.clone(), th.to_string());
        tokio::spawn(async move { Actor::new(ag, th).run(rx).await });
        tx
    }
    /// Every actor ends and takes its process (and the process's group) with it.
    pub fn shutdown(&self) { self.carry_gone.store(true, Relaxed); self.actors.lock().unwrap().clear(); }
    pub fn actors(&self) -> usize { self.actors.lock().unwrap().len() }

    /// Hub.turn then Hub.post.go: take the person's message, start or feed the thread's agent. Acknowledged after the events are durable.
    pub async fn deliver(self: &Arc<Self>, thread: &str, msg: &str, text: &str, origin: Option<(String, String, String)>, extra: Vec<Value>) -> Result<(), String> { self.deliver_as("user", thread, msg, text, origin, extra).await }
    pub async fn deliver_k(self: &Arc<Self>, person: bool, role: &str, thread: &str, msg: &str, text: &str, origin: Option<(String, String, String)>, extra: Vec<Value>) -> Result<(), String> {
        for _ in 0..3 {
            let tx = self.actor(thread); let (reply, rx) = oneshot::channel();
            match tx.try_send(Cmd::User { person, msg: msg.into(), text: text.into(), role: role.into(), origin: origin.clone(), extra: extra.clone(), reply }) {
                Ok(()) => return rx.await.unwrap_or_else(|_| Err("the thread's agent stopped".into())),
                Err(mpsc::error::TrySendError::Full(_)) => { self.stats.refused_full.fetch_add(1, Relaxed); return Err("too many messages are waiting for this thread".into()) }
                Err(mpsc::error::TrySendError::Closed(_)) => { self.actors.lock().unwrap().remove(thread); }
            }
        }
        Err("the thread's agent is busy".into())
    }
    /// Ride.mcp: a running turn's Backplane tool result carries the waiting mail (once); the ride is emptied in the same commit.
    pub async fn ride_take(&self, thread: &str) -> Option<String> {
        if crate::copy::person_turn_allowed() { return None } // side-by-side: mail waits for the person's next message, never into a running real agent
        let (ride, running) = { let w = self.world().await; (w.setting(&format!("ride.{thread}")).to_string(), w.running(thread)) };
        if ride.is_empty() || !running { return None }
        let ev = codex::ride_done(thread, &ride, "into the running turn", now_s());
        if self.hub.commit_many(&ev).await.is_err() { return None }
        Some(codex::ride_block(&ride))
    }
    /// AgentStop: end the thread's agent process in ANY state (an idle Claude process too); no events. For hub-side callers (archive, delete, a provider switch); not a client method.
    pub async fn deliver_as(self: &Arc<Self>, role: &str, thread: &str, msg: &str, text: &str, origin: Option<(String, String, String)>, extra: Vec<Value>) -> Result<(), String> { self.deliver_k(false, role, thread, msg, text, origin, extra).await }
    /// A message a PERSON typed (the client's turn.start): the only delivery that may start or write to a real agent in side-by-side mode.
    pub async fn deliver_person(self: &Arc<Self>, thread: &str, msg: &str, text: &str, extra: Vec<Value>) -> Result<(), String> { self.deliver_k(true, "user", thread, msg, text, None, extra).await }
    pub async fn stop(self: &Arc<Self>, thread: &str) -> Result<(), String> {
        let tx = self.actor(thread); let (reply, rx) = oneshot::channel();
        tx.try_send(Cmd::Stop { reply }).map_err(|_| "too many requests are waiting for this thread".to_string())?; rx.await.map_err(|_| "the thread's agent stopped".to_string())
    }
    /// Wt.start / Hub.resume: an idle thread's queued `text` runs as one turn (QueueTaken, TurnChanged running). The CALLER decides that it may (Config.held, Queue.waiting)
    /// and logs the queue events it owns; this does not check them. Not a client method: it logs no message.
    pub async fn start_queued(self: &Arc<Self>, thread: &str, text: &str) -> Result<(), String> {
        let tx = self.actor(thread); let (reply, rx) = oneshot::channel();
        tx.try_send(Cmd::StartQueued { text: text.into(), taken: true, reply }).map_err(|_| "too many requests are waiting for this thread".to_string())?; rx.await.unwrap_or_else(|_| Err("the thread's agent stopped".into()))
    }
    /// Task.repair: the idle parent that holds a task's result as its newest message starts its turn with it (TurnChanged running only; the caller logs TaskDelivered).
    pub async fn resume_with(self: &Arc<Self>, thread: &str, text: &str) -> Result<(), String> {
        let tx = self.actor(thread); let (reply, rx) = oneshot::channel();
        tx.try_send(Cmd::StartQueued { text: text.into(), taken: false, reply }).map_err(|_| "too many requests are waiting for this thread".to_string())?; rx.await.unwrap_or_else(|_| Err("the thread's agent stopped".into()))
    }
    /// Queue.steer.k's idle branch: log TurnChanged running (a normal turn start: design baseline, checkpoint) and run `text` as a turn of its own. No stop, no "Steered" act,
    /// no QueueTaken: the caller logs QueueTakenOne and the rest of its queue stays. For a running turn use `steer`.
    pub async fn start_one(self: &Arc<Self>, thread: &str, text: &str) -> Result<(), String> { self.resume_with(thread, text).await }
    /// Steer.go: a running non-Claude turn stops and starts again with `text`; Claude takes it in place (and starts a process if none lives). Logs TurnChanged running either way.
    pub async fn steer(self: &Arc<Self>, thread: &str, text: &str) -> Result<(), String> {
        let tx = self.actor(thread); let (reply, rx) = oneshot::channel();
        tx.try_send(Cmd::Steer { text: text.into(), reply }).map_err(|_| "too many requests are waiting for this thread".to_string())?; rx.await.unwrap_or_else(|_| Err("the thread's agent stopped".into()))
    }
    /// Register a set of MCP tools (the bots'): listed to the threads its `list` names, called through the same gate as Backplane's own.
    pub fn add_tools(&self, s: Arc<dyn mcp::ToolSet>) { self.tool_sets.lock().unwrap().push(s) }
    pub fn tool_sets(&self) -> Vec<Arc<dyn mcp::ToolSet>> { self.tool_sets.lock().unwrap().clone() }
    /// Wake.ride (Ride.note + Ride.start): add a note `{id, text}` to the thread's ride, once per id. A running Claude turn takes the ride on its input at once; another running turn keeps it for
    /// its next Backplane tool result or its end; an idle thread starts a turn with the ride (ActivityLogged "ride:<id>" "Delivered with a new turn: N waiting message(s)", the ride cleared).
    /// The caller owns Config.held. Logs no message of its own: the note is the agent's input, not a person's.
    pub async fn wake_ride(self: &Arc<Self>, thread: &str, id: &str, text: &str) -> Result<(), String> { self.ride_k(thread, id, text, true).await }
    /// Ride.note alone: a running Claude turn takes it on its input at once, any other keeps it in the ride for the next tool result or the
    /// next turn; an idle thread is NOT started (the published text is already on screen, and a note must not run an agent turn in the person's conversation).
    pub async fn ride_note(self: &Arc<Self>, thread: &str, id: &str, text: &str) -> Result<(), String> { self.ride_k(thread, id, text, false).await }
    async fn ride_k(self: &Arc<Self>, thread: &str, id: &str, text: &str, start: bool) -> Result<(), String> {
        let tx = self.actor(thread); let (reply, rx) = oneshot::channel();
        tx.try_send(Cmd::WakeRide { id: id.into(), text: text.into(), start, reply }).map_err(|_| "too many requests are waiting for this thread".to_string())?; rx.await.unwrap_or_else(|_| Err("the thread's agent stopped".into()))
    }
    pub async fn interrupt(self: &Arc<Self>, thread: &str) -> Result<(), String> {
        let tx = self.actor(thread); let (reply, rx) = oneshot::channel();
        tx.try_send(Cmd::Interrupt { reply }).map_err(|_| "too many requests are waiting for this thread".to_string())?;
        rx.await.map_err(|_| "the thread's agent stopped".to_string())
    }

    /// Hub.recover.k (Rec.turn) and Carry: a turn the restart cut short is marked stopped with a note; with restart.continue on
    /// (and not repeating itself) it goes on with a message: the oldest-running BURST at once, then the rest one every CARRY_GAP_S, so none stays stopped and a restart with many
    /// threads does not start every agent in the same instant. Those still waiting hold `carry.wait.<thread>`, so a second restart in the meantime keeps them in line.
    pub async fn recover(self: &Arc<Self>) {
        let (cut, carry, cut_threads): (Vec<Value>, Vec<String>, Vec<String>) = {
            let w = self.world().await; let now = now_s(); let zone = w.setting("tz.machine").to_string(); let mut ev = vec![]; let mut carry: Vec<(u64, String)> = vec![]; let mut cut_threads: Vec<String> = vec![]; let mut told = std::collections::HashSet::new();
            // Rec.asks: an open ask is closed unanswered and its thread told which one went unanswered (an order's question stays open)
            let mut asks: Vec<(&String, &world::Ask)> = w.asks.iter().filter(|(_, a)| !a.tool.starts_with("order_ask")).collect(); asks.sort_by(|a, b| a.0.cmp(b.0));
            for (id, a) in asks {
                ev.push(json!({"$":"AskClosed","id":id,"answer":"","at":now}));
                ev.push(json!({"$":"PendingChanged","thread":a.thread,"count":w.asks.values().filter(|x| x.thread == a.thread && x.tool.starts_with("order_ask")).count(),"at":now})); // Pend.*: what stays open (an order's question)
                if w.threads.contains_key(&a.thread) { told.insert(a.thread.clone());
                    ev.push(json!({"$":"ActivityLogged","thread":a.thread,"act":hub_id("r", now, self.seq()),"tone":"error","kind":"restart","summary":format!("{}{}", rules::restart_cut(&zone, now), rules::unanswered(&a.kind, &a.tool, &a.detail)),"at":now})) }
            }
            let mut ths: Vec<(&String, &world::T)> = w.threads.iter().filter(|(_, t)| t.turn == "running").collect(); ths.sort_by(|a, b| a.0.cmp(b.0));
            for (th, t) in ths {
                if !told.contains(th) { ev.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("r", now, self.seq()),"tone":"error","kind":"restart","summary":rules::restart_cut(&zone, now),"at":now})) }
                ev.push(json!({"$":"TurnChanged","thread":th,"state":"interrupted","at":now})); cut_threads.push(th.clone());
                if !t.last_user.is_empty() { ev.push(set(format!("stopped.{}", t.last_user), "1".into(), now)) }
                let will = world::carry_will(t, w.setting("restart.continue")) && !th.starts_with("run") && !th.starts_with("mailcoord-"); // Ephemeral.is: nothing carries a mail run on
                if will && !crate::copy::on() { carry.push((t.running_at, th.clone())) } // a hub on a copy of a log resumes no turn, whatever restart.continue says
            }
            // those the last restart left waiting: stopped already, nothing to mark, still owed their turn
            for (th, t) in w.threads.iter().filter(|(th, t)| t.turn != "running" && w.setting(&wait_key(th)) == "1") {
                if t.turn != "interrupted" { ev.push(set(wait_key(th), String::new(), now)); continue } // the person's own turn came and went: nothing is owed
                if !crate::copy::on() && world::carry_may(t, w.setting("restart.continue")) && !th.starts_with("run") && !th.starts_with("mailcoord-") { carry.push((t.running_at, th.clone())) }
                else { ev.push(set(wait_key(th), String::new(), now)); cut_threads.push(th.clone()) }
            }
            carry.sort();
            let carry: Vec<String> = carry.into_iter().map(|c| c.1).collect();
            for th in carry.iter().skip(BURST) { if w.setting(&wait_key(th)) != "1" { ev.push(set(wait_key(th), "1".into(), now)) } }
            (ev, carry, cut_threads)
        };
        if !cut.is_empty() { if let Err(e) = self.hub.commit_many(&cut).await { eprintln!("agents: recovery: {e:#}") } }
        *self.cut.lock().unwrap() = cut_threads.clone();
        for th in &cut_threads { if !carry.contains(th) { self.task_end(th, "interrupted").await; self.launch_tell(th, &[json!({"$":"TurnChanged","thread":th,"state":"interrupted","at":now_s()})], "").await } } // Rec.turn.k: a cut turn that will not carry on ends its task (a carried one is still its business)
        let rest = carry.iter().skip(BURST).cloned().collect::<Vec<_>>();
        for th in carry.iter().take(BURST) { self.carry_on(th).await }
        if rest.is_empty() { return }
        let me = Arc::downgrade(self);
        tokio::spawn(async move {
            for th in rest {
                tokio::time::sleep(std::time::Duration::from_secs(CARRY_GAP_S)).await;
                let Some(ag) = me.upgrade() else { return }; if ag.carry_gone.load(Relaxed) { return }
                ag.carry_next(&th).await;
            }
        });
    }
    /// One thread's turn of the queue, judged as it is now, not as it was at the restart. The person may have written to it (their turn stands), or archived it or turned restart.continue off
    /// meanwhile (it ends as a cut turn that does not carry on does).
    async fn carry_next(self: &Arc<Self>, th: &str) {
        let (waits, may) = { let w = self.world().await; let t = w.threads.get(th); (t.map_or(false, |t| t.turn == "interrupted"), t.map_or(false, |t| world::carry_may(t, w.setting("restart.continue")))) };
        if waits && may { return self.carry_on(th).await }
        let _ = self.hub.commit_many(&[set(wait_key(th), String::new(), now_s())]).await;
        if waits { self.task_end(th, "interrupted").await; self.launch_tell(th, &[json!({"$":"TurnChanged","thread":th,"state":"interrupted","at":now_s()})], "").await }
    }
    async fn carry_on(self: &Arc<Self>, th: &str) {
        let now = now_s(); let id = hub_id("c", now, self.seq());
        let note = json!({"$":"ActivityLogged","thread":th,"act":hub_id("r", now, self.seq()),"tone":"info","kind":"restart","summary":"Continuing after the restart","at":now});
        let waited = self.world().await.setting(&wait_key(th)) == "1";
        let extra = if waited { vec![note, set(wait_key(th), String::new(), now)] } else { vec![note] };
        let text = format!("{} {}", world::CARRY_TEXT, rules::CARRY_CUT);
        if let Err(e) = self.deliver(th, &id, &text, Some(("backplane".into(), "carry".into(), String::new())), extra).await { eprintln!("agents: carrying on {th}: {e}") }
    }
}

struct Rpc(Arc<Agents>);
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["turn.start", "turn.interrupt", "ask.answer", "thread.retitle"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if method == "ask.answer" { return match self.0.answer(p.get("id").and_then(Value::as_str).unwrap_or(""), p.get("answer").and_then(Value::as_str).unwrap_or("")).await { Ok(()) => vec![ok(id)], Err(e) => vec![err(id, &e)] } }
            let th = p.get("thread").and_then(Value::as_str).unwrap_or("");
            if th.is_empty() || !self.0.world().await.threads.contains_key(th) { eprintln!("agents: {method} refused: unknown thread {th:?}"); return vec![err(id, "unknown thread")] } // refused, and said so on the hub's log
            if method == "thread.retitle" { // the thread menu's Regenerate title: the person asked, so it replaces even a title they typed
                let ag = self.0.clone(); let th = th.to_string(); let r = ag.retitle(&th, None, true).await;
                return vec![match r { Ok(t) => json!({"t":"reply","id":id,"ok":true,"result":{"title":t}}), Err(e) => err(id, &e) }]
            }
            let r = match method {
                "turn.start" => {
                    let shown = p.get("text").and_then(Value::as_str).unwrap_or("");
                    // `agentText` (the phone data API: the text with the paths of the images attached) is what the agent is told and the message carries; `attachments` (their metadata) ride as a MessageAttached event in the same commit
                    let text = p.get("agentText").and_then(Value::as_str).filter(|t| !t.is_empty()).unwrap_or(shown);
                    let msg = p.get("msg").and_then(Value::as_str).filter(|m| !m.is_empty()).map(str::to_string).unwrap_or_else(|| hub_id("m", now_s(), self.0.seq()));
                    let retry = p.get("retry").and_then(Value::as_bool).unwrap_or(false);
                    let attached: Vec<Value> = p.get("attachments").filter(|a| a.as_array().map_or(false, |x| !x.is_empty())).map(|a| vec![json!({"$":"MessageAttached","thread":th,"msg":msg,"attachments":a,"at":now_s()})]).unwrap_or_default();
                    if who.client > 0 && !retry && !text.trim().is_empty() {
                        // the star nudge counts this prompt in the same commit; the offer (the fifth) goes to this connection after it
                        let (mut extra, offer) = crate::statehost::policies().map(|p| p.star_prompt(&msg, now_s())).unwrap_or((vec![], None));
                        extra.extend(attached);
                        let r = self.0.deliver_person(th, &msg, text, extra).await; if r.is_ok() { if let Some(f) = offer { self.0.hub.push(who.client, f); } }
                        if r.is_ok() { if let Some(m) = self.0.mailroom() { let bots: Vec<String> = { let w = self.0.world().await; refs::found(&w, th, text).into_iter().filter(|(h, _)| h.kind == "bot" && !h.id.contains('@')).map(|(h, _)| h.id).collect() }; if !bots.is_empty() { m.mentions(th, bots, text).await } let rooms: Vec<String> = { let w = self.0.world().await; refs::found(&w, th, text).into_iter().filter(|(h, _)| h.kind == "room").map(|(h, _)| h.id).collect() }; if !rooms.is_empty() { m.room_mentions(th, rooms, text).await } } } // Bots.mentions
                        r
                    } else { self.0.deliver(th, &msg, text, None, attached).await }
                }
                _ => { let r = self.0.interrupt(th).await; if let Some(m) = self.0.mailroom() { m.stopping(th).await } r }
            };
            match r { Ok(()) => vec![ok(id)], Err(e) => vec![err(id, &e)] }
        })
    }
}

/// A thread whose creation asked for a worktree (a branch) or that is an improvement thread must not fall back to its project folder.
/// A thread's folder that vanished (a removed worktree) never stops a turn: it is made again as a fresh git worktree of the project (detached at HEAD),
/// else as a plain folder, so threads, bots and restarts always have somewhere to run.
async fn remake_folder(cwd: &str, root: &str) -> Result<(), String> {
    let p = std::path::Path::new(cwd);
    if cwd.is_empty() { return Err("the thread has no folder".into()) }
    if p.is_dir() { return Ok(()) }
    if !root.is_empty() && std::path::Path::new(root).join(".git").exists() {
        if let Some(parent) = p.parent() { let _ = std::fs::create_dir_all(parent); }
        let run = |args: Vec<&str>| { let mut c = tokio::process::Command::new("git"); c.arg("-C").arg(root).args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()); c.status() };
        let _ = run(vec!["worktree", "prune"]).await;
        let _ = run(vec!["worktree", "add", "--detach", cwd, "HEAD"]).await;
    }
    if !p.is_dir() { std::fs::create_dir_all(p).map_err(|e| format!("its folder {cwd} is gone and could not be made again: {e}"))? }
    Ok(())
}
fn wants_worktree(id: &str, branch: &str, env: &str) -> bool { id.starts_with("improvement-") || (!branch.is_empty() && env != "local") }
/// What the Claude CLI says when `--resume` names a session it does not have.
const SESSION_GONE: &str = "No conversation found with session ID";
struct Live { input: mpsc::Sender<String>, pid: u32, key: String, person: bool, _busy: Option<crate::settings::accounts_hub::Busy> }

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum HistJob { Start(String, String), Step(String, String), End(String) }
impl HistJob { fn clone_job(&self) -> HistJob { self.clone() } }
const HIST_CAP: usize = 256; const HIST_STEPS_MAX: usize = 128;
const HIST_IDLE: Duration = Duration::from_secs(300); const HIST_JOB: Duration = Duration::from_secs(120);
/// The worker behind `hist`: jobs one at a time in arrival order; ends when idle for `idle` or when every sender is gone; gives a job up after `per_job`.
pub(crate) async fn hist_worker<F, Fut>(mut rx: mpsc::Receiver<(Option<JobGuard>, HistJob)>, idle: Duration, per_job: Duration, mut run: F) where F: FnMut(HistJob) -> Fut, Fut: std::future::Future<Output = ()> {
    loop {
        match tokio::time::timeout(idle, rx.recv()).await {
            Ok(Some((_g, j))) => { if tokio::time::timeout(per_job, run(j.clone())).await.is_err() { eprintln!("agents: a design-history capture ({j:?}) took more than {per_job:?}: no longer waited for (its git child, if started, is not killed), the later ones go on") } }
            _ => break,
        }
    }
}
struct Actor {
    hist_tx: Option<mpsc::Sender<(Option<JobGuard>, HistJob)>>, person: bool,
    ag: Arc<Agents>, th: String, cx: Cx, live_proc: Option<Live>,
    lines: Option<mpsc::Receiver<String>>, exit: Option<oneshot::Receiver<Exit>>,
    live: String, live_gen: u64, ahead: String, last_user: String, recent: VecDeque<String>, pacing: Option<Instant>, rid: u64,
    cwd: String, cmds: cmds::Cmds, sent: VecDeque<(String, String, String)>, wait: String, prog: bg::Prog, heard: Instant,
    /// Codex-shaped threads: one process per turn; the thread's provider and model; a ride waiting when the turn ended
    codex: bool, prov: String, model: String, next_ride: bool, driver: Option<Box<dyn providers::Driver>>, asked: HashMap<String, Value>,
    /// Claude: the session id the process was started with (`--resume`, "" for a new one; cleared once it says init), the message that started it (msg, text, what the agent gets), and a count of starts
    steer_now: bool, resumed: String, spawn_in: Option<(String, String, String)>, spawn_n: u64,
}

async fn next_batch(rx: &mut mpsc::Receiver<String>) -> Option<Vec<String>> {
    let first = rx.recv().await?; let mut bytes = first.len(); let mut v = vec![first];
    while bytes < BATCH_BYTES { match rx.try_recv() { Ok(l) => { bytes += l.len(); v.push(l) } Err(_) => break } }
    Some(v)
}

async fn recv_opt(r: &mut Option<mpsc::Receiver<String>>) -> Option<Vec<String>> { match r { Some(rx) => next_batch(rx).await, None => std::future::pending().await } }
async fn exit_opt(r: &mut Option<oneshot::Receiver<Exit>>) -> Result<Exit, oneshot::error::RecvError> { match r { Some(rx) => rx.await, None => std::future::pending().await } }

impl Actor {
    fn new(ag: Arc<Agents>, th: String) -> Actor { Actor { hist_tx: None, person: false, cx: Cx::new(&th), ag, th, live_proc: None, lines: None, exit: None, live: String::new(), live_gen: 0, ahead: String::new(), last_user: String::new(), recent: VecDeque::new(), pacing: None, rid: 0, cwd: String::new(), cmds: cmds::Cmds::new(), sent: VecDeque::new(), wait: String::new(), prog: bg::Prog::default(), heard: Instant::now(), codex: false, prov: String::new(), model: String::new(), next_ride: false, driver: None, asked: HashMap::new(), steer_now: false, resumed: String::new(), spawn_in: None, spawn_n: 0 } }

    async fn run(mut self, mut rx: mpsc::Receiver<Cmd>) {
        let mut tick = tokio::time::interval(Duration::from_secs(1)); tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let pace = self.pacing;
            tokio::select! {
                c = rx.recv() => match c { None => break, Some(c) => self.cmd(c).await },
                b = recv_opt(&mut self.lines), if self.lines.is_some() && pace.map_or(true, |t| Instant::now() >= t) => match b {
                    Some(b) => self.batch(b).await,
                    None => { self.lines = None }
                },
                e = exit_opt(&mut self.exit), if self.exit.is_some() => { self.exit = None; let e = e.unwrap_or(Exit { code: None, signal: None, stderr: String::new() }); self.gone(e).await }
                _ = tokio::time::sleep_until(pace.unwrap_or_else(Instant::now).into()), if pace.is_some() => { self.pacing = None }
                _ = tick.tick(), if self.cx.running || !self.prog.pend.is_empty() => { self.beat().await }
            }
        }
        self.stop().await;
    }

    async fn stop(&mut self) { if let Some(p) = self.live_proc.take() { proc::terminate(p.pid, Duration::from_secs(2)) } }

    async fn commit(&mut self, ev: Vec<Value>) {
        if ev.is_empty() { return }
        self.ag.stats.events.fetch_add(ev.len() as u64, Relaxed);
        if let Err(e) = self.ag.hub.commit_many(&ev).await { self.ag.stats.commit_errors.fetch_add(1, Relaxed); eprintln!("agents: {}: could not write {} events: {e:#}", self.th, ev.len()) }
    }
    async fn live_end(&mut self) { self.live.clear(); let _ = self.ag.hub.live_end(&self.th).await; }
    /// The stream is over without an assistant line having closed it (a turn that ends, is stopped or fails mid-text): sends no delta_end then (only Claude.assistant does), so the
    /// frame is sent only when streamed text is still on screen and nothing else would clear it. A turn whose last assistant line already closed its stream sends none.
    async fn live_close(&mut self) { if !self.live.is_empty() { self.live_end().await } }

    async fn cmd(&mut self, c: Cmd) {
        match c {
            Cmd::User { person, msg, text, role, origin, extra, reply } => { let r = self.user(person, msg, text, role, origin, extra).await; let _ = reply.send(r); }
            Cmd::Interrupt { reply } => { self.interrupt().await; let _ = reply.send(()); }
            Cmd::Stop { reply } => { self.cx.running = false; if self.live_proc.is_some() { self.ag.stats.procs_exited.fetch_add(1, Relaxed); } self.stop().await; self.lines = None; self.exit = None; self.next_ride = false; self.asked.clear(); self.live_proc = None; let _ = reply.send(()); }
            Cmd::StartQueued { text, taken, reply } => { let r = self.start_queued(text, taken).await; let _ = reply.send(r); }
            Cmd::WakeRide { id, text, start, reply } => { let r = self.wake_ride(id, text, start).await; let _ = reply.send(r); }
            Cmd::Steer { text, reply } => { let r = self.steer(text).await; let _ = reply.send(r); }
            Cmd::Answer { ask, answer } => { if let Some(req) = self.asked.remove(&ask) { let lines = self.driver.as_mut().map(|d| d.answer(&req, &answer)).unwrap_or_default(); if let Some(p) = &self.live_proc { for l in lines { let _ = p.input.try_send(l); } } } }
        }
    }

    /// Hub.turn.go + Hub.post.go (the Claude path: the message goes straight to the process, running or not).
    async fn user(&mut self, person: bool, msg: String, text: String, role: String, origin: Option<(String, String, String)>, extra: Vec<Value>) -> Result<(), String> {
        if self.recent.contains(&msg) { return Ok(()) } // Hub.dup: a message resent is stored once
        let (t, cwd, cx, ride) = {
            let w = self.ag.world().await;
            let Some(t) = w.threads.get(&self.th).cloned() else { return Err("unknown thread".into()) };
            (t, w.cwd(&self.th), w.cx(&self.th), w.setting(&format!("ride.{}", self.th)).to_string())
        };
        let (t, cwd, cx, ride) = if t.worktree.is_empty() && wants_worktree(&self.th, &t.branch, &t.env) { // made with a branch (or an improvement thread): never the project folder, which may be a guarded checkout
            let end = Instant::now() + Duration::from_secs(60); let mut got = None;
            loop { { let w = self.ag.world().await; if let Some(x) = w.threads.get(&self.th) { if !x.worktree.is_empty() { got = Some((x.clone(), w.cwd(&self.th), w.cx(&self.th), w.setting(&format!("ride.{}", self.th)).to_string())); } } } if got.is_some() || Instant::now() > end { break } tokio::time::sleep(Duration::from_millis(200)).await }
            match got { Some(g) => g, None => { let why = "this thread's worktree is not ready (its creation made none): not started in the project folder".to_string(); let (now, th) = (now_s(), self.th.clone()); self.commit(vec![json!({"$":"ActivityLogged","thread":th,"act":hub_id("rf", now, self.ag.seq()),"tone":"error","kind":"refused","summary":format!("not started: {why}"),"at":now})]).await; return Err(why) } }
        } else { (t, cwd, cx, ride) };
        if t.provider != "claude" && providers::driver(&t.provider).is_none() { return Err(format!("{} threads are not run by the Rust hub yet", if t.provider.is_empty() { "this provider's" } else { &t.provider })) }
        let sbs = crate::copy::person_turn_allowed();
        if sbs && !person { return self.codex_ride(&msg, &text, &role, origin).await } // default-deny: only the person's own turn.start starts or writes to a real agent; anything else waits in the ride
        let mut t = t;
        if sbs { // the person's turn here (SIDE-BY-SIDE.md checks 2-5; 1 is the caller, 6 the child's environment): refused with its reason, or forced permissions and a fresh session
            if let Err(why) = Self::person_checks(self.ag.clone(), cwd.clone()).await {
                let (now, th) = (now_s(), self.th.clone()); self.commit(vec![json!({"$":"ActivityLogged","thread":th,"act":hub_id("rf", now, self.ag.seq()),"tone":"error","kind":"refused","summary":format!("not started: {why}"),"at":now})]).await; return Err(why) }
            t.runtime = person::runtime_for(&t.runtime).to_string(); t.session.clear();
        }
        self.person = sbs;
        self.steer_now = person && role == "user" && origin.is_none() && (self.cx.running || t.turn == "running"); // a person's message into a running Claude turn: priority now
        let codex = t.provider != "claude";
        if codex && (self.cx.running || t.turn == "running") {
            if person && !sbs && role == "user" && origin.is_none() { // a person's message always steers: it is posted, and the running turn starts again with it (Steer.go)
                let now = now_s(); let mut ev = vec![json!({"$":"MessagePosted","thread":self.th,"msg":msg,"role":role,"text":text,"at":now})]; ev.extend(extra);
                self.commit(ev).await; self.recent.push_back(msg.clone()); if self.recent.len() > RECENT { self.recent.pop_front(); }
                return self.steer(text).await }
            return self.codex_ride(&msg, &text, &role, origin).await } // Hub.deliver.ride: a running turn takes it on its next Backplane tool result
        self.codex = codex; self.prov = t.provider.clone(); self.model = t.model.clone();
        let was_running = self.cx.running || t.turn == "running"; // Ride.post: a turn that starts on an idle thread takes the mail waiting for it
        self.cx = cx; self.last_user = msg.clone(); self.cwd = cwd.clone();
        let (now, th) = (now_s(), self.th.clone());
        let mut ev: Vec<Value> = vec![];
        let takes_ride = !ride.is_empty() && (codex || sbs || !was_running);
        if takes_ride { ev.extend(codex::ride_done(&th, &ride, "with a new turn", now)) } // Ride.post: an idle thread starts with the mail still waiting
        let first_title = if t.title == "New thread" && person && role == "user" && origin.is_none() { Some(rules::title_of(&text)).filter(|x| !x.is_empty()) } else { None }; // Title.out: the first line now, a model's title after
        if t.title == "New thread" { ev.push(json!({"$":"ThreadRenamed","id":th,"title":rules::title_of(&text),"at":now})) }
        // Steer.change: a message into a streaming reply posts what streamed, up to its last line break, ahead of it
        let head = rules::steer_head(&self.live);
        if !head.is_empty() {
            ev.push(json!({"$":"MessagePosted","thread":th,"msg":format!("{msg}:before"),"role":"assistant","text":head,"at":now}));
            let tail = rules::steer_tail(&self.live); self.ahead.push_str(&head); self.live_end().await;
            if !tail.is_empty() { self.live = tail.clone(); let _ = self.ag.hub.live_delta(&th, &tail).await; }
        }
        let refs = if role == "user" && origin.is_none() { crate::settings::mentions::resolve(&text, &crate::settings::models::all()) } else { vec![] }; // `&model` mentions: shown to the agent, never change the thread's model
        let mut posted = json!({"$":"MessagePosted","thread":th,"msg":msg,"role":role,"text":text,"at":now});
        if !refs.is_empty() { posted["refs"] = Value::Array(refs.iter().map(|r| r.json()).collect()); }
        ev.push(posted);
        if let Some((from, kind, r)) = &origin { ev.push(json!({"$":"MessageFrom","thread":th,"msg":msg,"from":from,"kind":kind,"ref":r,"at":now})) }
        ev.push(json!({"$":"TurnChanged","thread":th,"state":"running","at":now}));
        ev.push(set(format!("turn.from.{th}"), format!("@{msg}"), now));            // Turn.mark: the newest entry then
        ev.push(set(format!("restart.retry.{th}"), String::new(), now));             // Fallback.manual
        ev.push(set(format!("provider.resume.{th}"), String::new(), now));
        ev.extend(extra);
        ev.push(set(format!("fallback.input.{th}"), text.clone(), now));             // Fallback.attempt
        self.commit(ev).await;
        if let Some(ph) = first_title { let (ag, th2) = (self.ag.clone(), th.clone()); tokio::spawn(async move { if let Err(e) = ag.retitle(&th2, Some(&ph), false).await { eprintln!("titles: {th2}: kept the first line: {e}") } }); }
        self.recent.push_back(msg.clone()); if self.recent.len() > RECENT { self.recent.pop_front(); }
        self.cx.running = true; self.ag.stats.turns.fetch_add(1, Relaxed);
        self.jobs_start(&cwd, &msg);
        // the person's message starts the turn: the <terminals> note follows it (and the chat's unpinned terminals stop)
        let rv_block = { let w = self.ag.world().await; let ops: Vec<_> = w.reviews.get(&th).map(|v| v.iter().filter_map(|(k, sm, at)| crate::files::kicad::review::of(crate::files::kicad::review::TONE, k, sm, *at)).collect()).unwrap_or_default(); crate::files::kicad::review::prompt(&crate::files::kicad::review::fold(&ops)) }; // Review.agent: the open comments follow every message
        let text_ref = { let w = self.ag.world().await; let model_lines: Vec<String> = refs.iter().map(|r| r.line()).collect(); format!("{text}{}", refs::block(&w, &th, &text, &model_lines)) }; // Refs.agent: the text, then what it references (@ > % # words, then the `&model` lines, one block)
        let text_rv = if rv_block.is_empty() { text_ref } else { format!("{text_ref}\n\n{rv_block}") };
        let agent_text = if role == "user" && origin.is_none() { let n = crate::term::tools::on_person_message(&self.ag.hub, &th).await; if n.is_empty() { text_rv.clone() } else { format!("{text_rv}\n\n{n}") } } else { text_rv.clone() };
        let agent_text = if takes_ride && !codex { format!("{}\n\n{}", codex::ride_input(&ride), agent_text) } else { agent_text }; // what waited for the person goes with their next message
        let agent_text = match self.ag.mailroom() { Some(m) => { let b = m.turn_block(&th, &agent_text).await; if b.is_empty() { agent_text } else { format!("{b}\n\n{agent_text}") } } None => agent_text }; // Bots.turn: a bot's memory
        let agent_text = { let w = self.ag.world().await; rules::rules_agent(&rules::rules_block(&w.st, w.bot(&th), &t.project), &agent_text) }; // Rules.agent: the standing rules come first
        if codex { let run = if ride.is_empty() { agent_text.clone() } else { format!("{}\n\n{}", codex::ride_input(&ride), agent_text) }; self.codex_run(&t, &cwd, &run).await; return Ok(()) }
        self.claude_send(&t, &cwd, &msg, &text, &agent_text, now).await;
        Ok(())
    }

    /// Wt.start / Hub.resume: an idle thread's queued text runs as one turn (QueueTaken, TurnChanged running).
    async fn start_queued(&mut self, text: String, taken: bool) -> Result<(), String> {
        if crate::copy::person_turn_allowed() { return Err("on a copy in side-by-side mode a turn starts only when the person sends a message".into()) }
        let (t, cwd, cx) = { let w = self.ag.world().await; let Some(t) = w.threads.get(&self.th).cloned() else { return Err("unknown thread".into()) }; (t, w.cwd(&self.th), w.cx(&self.th)) };
        if self.cx.running || t.turn == "running" { return Err("the thread's turn is running".into()) }
        if t.provider != "claude" && providers::driver(&t.provider).is_none() { return Err(format!("{} threads are not run by the Rust hub yet", t.provider)) }
        let (now, th) = (now_s(), self.th.clone());
        self.codex = t.provider != "claude"; self.prov = t.provider.clone(); self.model = t.model.clone(); self.cx = cx; self.cwd = cwd.clone();
        let mut ev = vec![]; if taken { ev.push(json!({"$":"QueueTaken","thread":th,"at":now})) } ev.push(json!({"$":"TurnChanged","thread":th,"state":"running","at":now})); self.commit(ev).await;
        self.cx.running = true; self.ag.stats.turns.fetch_add(1, Relaxed); self.jobs_start(&cwd, "");
        if self.codex { self.codex_run(&t, &cwd, &text).await } else { let msg = hub_id("q", now, self.ag.seq()); self.claude_send(&t, &cwd, &msg, &text, &text, now).await }
        Ok(())
    }

    /// Steer.go: Claude takes the text on its input; any other provider's process stops and the turn starts again with it (and whatever waited in the ride).
    async fn steer(&mut self, text: String) -> Result<(), String> {
        if crate::copy::person_turn_allowed() { return Err("on a copy in side-by-side mode a turn starts only when the person sends a message".into()) }
        let (t, cwd, ride) = { let w = self.ag.world().await; let Some(t) = w.threads.get(&self.th).cloned() else { return Err("unknown thread".into()) }; (t, w.cwd(&self.th), w.setting(&format!("ride.{}", self.th)).to_string()) };
        let (now, th) = (now_s(), self.th.clone());
        if t.provider == "claude" { // Steer.go: TurnChanged running always (Steer.note is empty for Claude), and the run starts even with no live process
            self.cwd = cwd.clone(); self.commit(vec![json!({"$":"TurnChanged","thread":th,"state":"running","at":now})]).await; self.cx.running = true;
            let msg = hub_id("s", now, self.ag.seq()); self.steer_now = true; self.claude_send(&t, &cwd, &msg, &text, &text, now).await; return Ok(()) }
        if providers::driver(&t.provider).is_none() { return Err(format!("{} threads are not run by the Rust hub yet", t.provider)) }
        self.stop().await; self.lines = None; self.exit = None; self.next_ride = false; self.live_proc = None; self.asked.clear();
        let mut ev = vec![json!({"$":"ActivityLogged","thread":th,"act":hub_id("s", now, self.ag.seq()),"tone":"info","kind":"steer","summary":"Steered: the turn starts again with this message","at":now})];
        let run = if ride.is_empty() { text.clone() } else { ev.extend(codex::ride_done(&th, &ride, "with the steer", now)); format!("{}\n\n{}", codex::ride_input(&ride), text) };
        ev.push(json!({"$":"TurnChanged","thread":th,"state":"running","at":now}));
        self.live_close().await; self.commit(ev).await; self.cwd = cwd.clone(); self.cx.running = true; self.codex = true; self.prov = t.provider.clone(); self.model = t.model.clone();
        self.codex_run(&t, &cwd, &run).await; Ok(())
    }

    /// Hub.deliver.ride: a message for a running Codex turn is posted now and waits as the thread's ride.
    async fn codex_ride(&mut self, msg: &str, text: &str, role: &str, origin: Option<(String, String, String)>) -> Result<(), String> {
        let (now, th) = (now_s(), self.th.clone());
        let ride = { let w = self.ag.world().await; w.setting(&format!("ride.{th}")).to_string() };
        let mut ev = vec![json!({"$":"MessagePosted","thread":th,"msg":msg,"role":role,"text":text,"at":now})];
        if let Some((from, kind, r)) = &origin { ev.push(json!({"$":"MessageFrom","thread":th,"msg":msg,"from":from,"kind":kind,"ref":r,"at":now})) }
        ev.push(set(format!("ride.{th}"), codex::ride_add(&ride, msg, text), now));
        self.commit(ev).await; self.recent.push_back(msg.to_string()); if self.recent.len() > RECENT { self.recent.pop_front(); }
        Ok(())
    }

    /// Codex.go: one process for this turn (`codex exec`, resumed on the session); its lines come back as Codex shapes.
    async fn codex_run(&mut self, t: &world::T, cwd: &str, text: &str) {
        let (now, th) = (now_s(), self.th.clone());
        if crate::copy::person_turn_allowed() && !self.person { return } // side-by-side: only a person's turn starts a real agent
        self.cx.running = true; self.ag.stats.turns.fetch_add(1, Relaxed);
        let root = self.ag.world().await.roots.get(&t.project).cloned().unwrap_or_default();
        if let Err(e) = remake_folder(cwd, &root).await { self.fail(format!("could not start codex: {e}"), "spawn", &format!("spawn-{now}")).await; self.ag.stats.spawn_failures.fetch_add(1, Relaxed); return }
        let Some(made) = providers::driver(&t.provider) else { self.fail(format!("{} threads are not run by the Rust hub yet", t.provider), "spawn", &format!("spawn-{now}")).await; return };
        let prog = match (if self.person { self.person_prog(made.cli) } else { self.ag.cli.resolve(made.cli) }) { Ok(p) => p, Err(e) => { let hint = made.driver.install_hint().map(|h| format!(". {h}")).unwrap_or_default(); self.fail(format!("could not start {}: {e}{hint}", made.cli), "spawn", &format!("spawn-{now}")).await; self.ag.stats.spawn_failures.fetch_add(1, Relaxed); return } };
        self.stop().await; self.lines = None; self.exit = None;
        let gen = format!("{now}-{}", self.ag.seq()); self.ag.set_gen(&th, &gen);
        let prompt = self.ag.prompt_for(&th, cwd).await;
        let (effort, think, session) = { let w = self.ag.world().await; (w.setting(&format!("effort.{th}")).to_string(), self.cx.think, w.threads.get(&th).map(|x| x.session.clone()).unwrap_or_else(|| t.session.clone())) };
        let session = if self.person { String::new() } else { session }; // a copied session is never resumed in side-by-side mode
        let url = self.ag.mcp.get().map(|m| m.url(&th, &gen)).unwrap_or_default();
        let (host, cwd2, text2, prov2) = (self.ag.host.clone(), cwd.to_string(), text.to_string(), t.provider.clone());
        let (body, imgs) = tokio::task::spawn_blocking(move || { let body = if text2.contains('$') { let ss = skills::scan_all(&skills::env_from(&host.user_home, &host.home, host.kistack.as_deref()), &cwd2); skills::prepare(&text2, &ss, &prov2, false) } else { text2.clone() }; (body, inputs::att_paths(&text2).into_iter().filter(|p| !inputs::mime(p).is_empty()).collect::<Vec<_>>()) }).await.unwrap_or_default();
        let a = providers::TurnIn { thread: th.clone(), cwd: cwd.into(), home: self.ag.host.home.to_string_lossy().into_owned(), model: t.model.clone(), session, runtime: t.runtime.clone(), interaction: t.interaction.clone(), effort, think, text: body, images: imgs, prompt, mcp_url: url, pcbparts_url: "https://pcbparts.dev/mcp".into(), gen: gen.clone(), settings: { let w = self.ag.world().await; providers::SETTINGS.iter().map(|k| (k.to_string(), w.setting(k).to_string())).collect() } };
        let mut driver = made.driver;
        if let Err(e) = driver.prepare(&a) { let hint = driver.install_hint().map(|h| format!(". {h}")).unwrap_or_default(); self.fail(format!("could not start {}: {e}{hint}", made.cli), "spawn", &format!("spawn-{now}")).await; self.ag.stats.spawn_failures.fetch_add(1, Relaxed); return }
        let (args, env, start) = (driver.argv(&a), driver.env(&a), driver.start(&a));
        let spawned = if self.person { match self.person_child() { Ok(pc) => proc::spawn_person(&pc, &prog, &args, std::path::Path::new(cwd), &env), Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)) } } else { proc::spawn(&prog, &args, std::path::Path::new(cwd), self.ag.cli.dir(), &env) };
        match spawned {
            Ok(p) => { self.ag.stats.procs_spawned.fetch_add(1, Relaxed); self.heard = Instant::now(); for l in start { let _ = p.input.try_send(l); } self.driver = Some(driver); self.live_proc = Some(Live { input: p.input, pid: p.pid, key: String::new(), person: self.person, _busy: None }); self.lines = Some(p.lines); self.exit = Some(p.exit) }
            Err(e) => { self.fail(format!("could not start {}: {e}", made.cli), "spawn", &format!("spawn-{now}")).await; self.ag.stats.spawn_failures.fetch_add(1, Relaxed); }
        }
    }

    /// A permission/question the process asked (providers::Driver docs): allowed-for-session answers at once, else an ask for the person.
    async fn driver_ask(&mut self, request: &Value, at: u64) -> Option<Vec<Value>> {
        let info = self.driver.as_ref()?.ask_info(request)?; let th = self.th.clone();
        let session_ok = info.kind == "approval" && self.ag.world().await.sessions.contains(&(th.clone(), info.tool.clone())); // Perm.session_ok
        if session_ok { let lines = self.driver.as_mut().map(|d| d.answer(request, "accept")).unwrap_or_default(); if let Some(p) = &self.live_proc { for l in lines { let _ = p.input.try_send(l); } } return None }
        if self.asked.len() >= 16 { let lines = self.driver.as_mut().map(|d| d.answer(request, "deny")).unwrap_or_default(); if let Some(p) = &self.live_proc { for l in lines { let _ = p.input.try_send(l); } } return None } // bounded: too many waiting
        let ask = hub_id("q", at, self.ag.seq());
        self.asked.insert(ask.clone(), request.clone()); self.ag.driver_asks.lock().unwrap().insert(ask.clone(), th.clone());
        let pend = self.ag.pending_ev(&th, 1, "", at).await;
        Some(vec![json!({"$":"AskOpened","id":ask,"thread":th,"kind":info.kind,"tool":info.tool,"detail":info.detail,"options":info.options,"at":at}), pend])
    }

    /// Ride.note + Ride.start for a note that is not a person's message (a wake, an escalation, a coordinator's report): the note joins the thread's ride (once per id). A running Claude turn takes
    /// the ride on its input at once; any other running turn keeps it for its next Backplane tool result or its end; an idle thread starts a turn with the ride. (The caller decides Config.held.)
    async fn wake_ride(&mut self, id: String, text: String, start: bool) -> Result<(), String> {
        let (t, cwd, cx, ride) = { let w = self.ag.world().await; let Some(t) = w.threads.get(&self.th).cloned() else { return Err("unknown thread".into()) }; (t, w.cwd(&self.th), w.cx(&self.th), w.setting(&format!("ride.{}", self.th)).to_string()) };
        if t.archived || t.deleted { return Err("the thread is gone".into()) }
        if t.provider != "claude" && providers::driver(&t.provider).is_none() { return Err(format!("{} threads are not run by the Rust hub yet", t.provider)) }
        let (now, th) = (now_s(), self.th.clone()); let v = codex::ride_add(&ride, &id, &text);
        let running = self.cx.running || t.turn == "running";
        if crate::copy::person_turn_allowed() { self.commit(vec![set(format!("ride.{th}"), v.clone(), now)]).await; return Ok(()) } // side-by-side: the note waits in the ride for the person's next message
        if running && t.provider == "claude" && self.live_proc.is_some() { // Ride.note.k True: into the running turn at once, with whatever waited (a "running" turn with no process, after a restart, keeps the ride)
            let mut ev = codex::ride_done(&th, &v, "into the running turn", now); ev.push(set(format!("ridden.{th}"), now.to_string(), now)); self.commit(ev).await;
            self.cwd = cwd.clone(); let msg = hub_id("w", now, self.ag.seq()); let block = codex::ride_input(&v);
            self.claude_send(&t, &cwd, &msg, &block, &block, now).await; return Ok(())
        }
        self.commit(vec![set(format!("ride.{th}"), v.clone(), now)]).await;
        if running || !start { return Ok(()) } // carried by the next tool result (ride_take) or started at the turn's end; a note alone waits for the next turn
        // Ride.start: an idle thread starts a turn with the waiting rides
        self.cx = cx; self.cwd = cwd.clone(); self.codex = t.provider != "claude"; self.prov = t.provider.clone(); self.model = t.model.clone();
        let mut ev = codex::ride_done(&th, &v, "with a new turn", now); ev.push(json!({"$":"TurnChanged","thread":th,"state":"running","at":now}));
        self.commit(ev).await; self.cx.running = true; self.ag.stats.turns.fetch_add(1, Relaxed); self.jobs_start(&cwd, &codex::ride_last(&v));
        let input = codex::ride_input(&v);
        if self.codex { self.codex_run(&t, &cwd, &input).await } else { let msg = hub_id("w", now, self.ag.seq()); self.claude_send(&t, &cwd, &msg, &input, &input, now).await }
        Ok(())
    }

    /// Ride.after: a turn that ended with mail waiting starts the next at once with it (Ride.run).
    async fn codex_next(&mut self) {
        if crate::copy::person_turn_allowed() { return } // the ride waits for the person's next message
        let th = self.th.clone();
        let (t, cwd, ride) = { let w = self.ag.world().await; (w.threads.get(&th).cloned(), w.cwd(&th), w.setting(&format!("ride.{th}")).to_string()) };
        let Some(t) = t else { return }; if ride.is_empty() || t.archived || t.deleted || providers::driver(&t.provider).is_none() { return }
        let now = now_s(); let mut ev = codex::ride_done(&th, &ride, "with a new turn", now);
        ev.push(json!({"$":"TurnChanged","thread":th,"state":"running","at":now}));
        self.commit(ev).await; self.cwd = cwd.clone(); self.jobs_start(&cwd, &codex::ride_last(&ride));
        self.codex_run(&t, &cwd, &codex::ride_input(&ride)).await;
    }

    /// One read of a Codex-shaped process's lines (Hub.agent_line for the provider): only while the turn runs.
    async fn codex_batch(&mut self, ls: Vec<String>) {
        let st = &self.ag.stats; st.lines.fetch_add(ls.len() as u64, Relaxed); st.max_batch_lines.fetch_max(ls.len() as u64, Relaxed);
        let at = now_s(); self.heard = Instant::now();
        let mut events: Vec<Value> = vec![]; let mut clear = false; let mut ended = false; let mut act: Option<crate::settings::driver::Act> = None;
        for l in ls {
            if l.trim().is_empty() { continue }
            let raw = crate::store::clean(&l);
            let line = serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| { self.ag.stats.bad_lines.fetch_add(1, Relaxed); Value::String(raw.to_string()) });
            if let Some(p) = crate::statehost::policies() { events.extend(p.line(&self.th, at, &line)) } // Watchdog.line
            let re = match self.driver.as_mut() { Some(d) => d.on_line(&line), None => continue };
            if let Some(p) = &self.live_proc { for s in re.send { let _ = p.input.try_send(s); } }
            if !self.cx.running { continue } // stopped_turn_quiet: what a stopped turn's process still says is old news
            for v in re.shapes {
            if v["type"] == "bp.ask" { if let Some(e) = self.driver_ask(&v["request"], at).await { events.extend(e) } continue }
            let fx = self.cx.codex_line(&v, &self.prov.clone(), &self.model.clone(), at);
            let terminal = fx.events.iter().any(|e| e["$"] == "TurnChanged" && matches!(e["state"].as_str(), Some("failed" | "interrupted" | "completed")));
            let verdict = crate::settings::driver::on_line(&self.ag.hub, &self.th, &v, terminal).await;
            let (drop_end, vevents) = (verdict.drop_turn_end, verdict.events); if verdict.act.is_some() { act = verdict.act }
            if let Some(id) = codex::step_for(&v) { self.jobs_step(&id) }
            for d in &fx.deltas {
                self.ag.stats.deltas.fetch_add(1, Relaxed);
                if self.live.is_empty() { self.live_gen = now_ms() }
                if self.live.len() + d.len() <= LIVE_CAP { self.live.push_str(d) }
                let _ = self.ag.hub.live_delta(&self.th, d).await;
            }
            for mut e in fx.events {
                if drop_end && e["$"] == "TurnChanged" && e["state"] == "failed" { continue }
                if MARK_ENDS && e["$"] == "TurnChanged" && e["state"] != "running" { e["_end"] = json!(true) }
                if e["$"] == "ActivityLogged" && e["tone"] == "tool" { if let Some(a) = e["act"].as_str() { self.jobs_step(a) } }
                if e["$"] == "TurnChanged" && e["state"] != "running" {
                    self.cx.running = false; ended = true; clear = true; self.jobs_end();
                    if e["state"] == "failed" && !self.last_user.is_empty() { events.push(e); events.push(set(format!("failed.{}", self.last_user), "1".into(), at)); continue }
                }
                events.push(e);
            }
            events.extend(vevents);
            if fx.delta_end { clear = true }
            }
        }
        if clear { self.live_close().await }
        self.commit(events).await;
        if let Some(a) = act { crate::settings::driver::spawn_act(self.ag.clone(), self.th.clone(), a) }
        if ended && self.driver.as_ref().map_or(false, |d| d.ends_on_turn_end()) { self.stop().await; self.lines = None; self.exit = None; self.asked.clear(); self.live_proc = None; self.ag.stats.procs_exited.fetch_add(1, Relaxed); } // Grok.stop: SIGTERM after the turn's events
        if ended { if self.live_proc.is_none() { self.codex_next().await } else { self.next_ride = true } }
    }

    /// Claude: the process (started if it is not the right one) takes the message on its input.
    async fn claude_send(&mut self, t: &world::T, cwd: &str, msg: &str, text: &str, agent_text: &str, now: u64) {
        if crate::copy::person_turn_allowed() && !self.person { return } // side-by-side: nothing but a person's turn writes to a real agent
        let th = self.th.clone(); let (cwd, msg, text, agent_text) = (cwd.to_string(), msg.to_string(), text.to_string(), agent_text.to_string());
        let key = rules::agent_key(&t.model, &t.runtime, &t.interaction, self.cx.think);
        // a live process with a command still open (a turn in flight) is never replaced for a changed model/mode key: the message goes on its input (the CLI queues it) and the new key applies from the next process
        if self.live_proc.as_ref().map_or(true, |p| p.person != self.person || (p.key != key && !self.cmds.open(""))) {
            self.stop().await; self.lines = None; self.exit = None;
            if let Err(e) = self.spawn(t, &cwd, key).await { self.fail(format!("could not start claude: {e}"), "spawn", &format!("spawn-{now}")).await; self.ag.stats.spawn_failures.fetch_add(1, Relaxed); return }
            self.spawn_in = Some((msg.to_string(), text.to_string(), agent_text.to_string()));
        }
        let uuid = rules::hub_uuid(now, &format!("{th}/{msg}"));
        self.cmds = self.cmds.accept(&uuid); self.sent.push_back((uuid.clone(), msg.clone(), text.clone())); if self.sent.len() > RECENT { self.sent.pop_front(); }
        let steer = std::mem::take(&mut self.steer_now);
        let bound = { let raw = self.ag.hub.setting(&crate::settings::accounts::binding_key(&th, "claude")).await.unwrap_or_default(); !serde_json::from_str::<Value>(&raw).ok().map_or(String::new(), |b| b["id"].as_str().unwrap_or("").to_string()).is_empty() };
        let isolated = self.person || bound; // an account profile or the person mode runs Claude without the person's own skills and plugins
        let line = { let (text2, cwd2, host, uuid2) = (agent_text.clone(), cwd.clone(), self.ag.host.clone(), uuid.clone()); // Media.claude: images read, `$skill` mentions made real
            tokio::task::spawn_blocking(move || { let body = if text2.contains('$') { let ss = skills::scan_all(&skills::env_from(&host.user_home, &host.home, host.kistack.as_deref()), &cwd2); skills::prepare(&text2, &ss, "claude", isolated) } else { text2.clone() }; let imgs = inputs::read_images(&text2); inputs::claude_line_p(&body, &imgs, &uuid2, steer) }).await.unwrap_or_default() };
        let sent = self.live_proc.as_ref().map(|p| p.input.try_send(line));
        if matches!(sent, Some(Ok(()))) { self.cmds = self.cmds.sent(&uuid) } else { self.fail("the agent is not taking input".into(), "input", &hub_id("in", now, 0)).await }
    }

    /// Hub.post.go: a new turn's design baseline (checkpoint 0 and a step), taken in the background.
    fn jobs_start(&mut self, cwd: &str, entry: &str) { self.hist(HistJob::Start(cwd.to_string(), entry.to_string())) }
    /// A tool call ended (Steps.claude): a step after it. A turn ended (Ended.out): a step and a checkpoint.
    fn jobs_step(&mut self, entry: &str) { let cwd = self.cwd.clone(); self.hist(HistJob::Step(cwd, entry.to_string())) }
    fn jobs_end(&mut self) { let cwd = self.cwd.clone(); self.hist(HistJob::End(cwd)) }
    /// A thread's design-history captures run one after another, in the order they were asked for: two captures overlapping on one thread's index could see each other's
    /// changes (a step logged against the wrong entry, or an index lock clash), and a turn's end capture must not be overtaken by the next turn's start.
    /// Bounded: a burst of tool-end steps past HIST_STEPS_MAX waiting is dropped (counted), starts and ends always fit. The worker ends after HIST_IDLE with nothing to do
    /// (a deleted or retired thread's actor sends nothing more), and the wait for one capture that hangs ends after HIST_JOB so the later ones still run (the future is dropped; a git child it already started is not killed and may run beside the next one, rarely).
    fn hist(&mut self, j: HistJob) {
        let is_step = matches!(j, HistJob::Step(..));
        for attempt in 0..2 {
            if self.hist_tx.is_none() {
                let (tx, rx) = mpsc::channel::<(Option<JobGuard>, HistJob)>(HIST_CAP); self.hist_tx = Some(tx);
                let (hub, th, home) = (self.ag.hub.clone(), self.th.clone(), self.ag.host.home.to_string_lossy().into_owned());
                tokio::spawn(hist_worker(rx, HIST_IDLE, HIST_JOB, move |j| { let (hub, th, home) = (hub.clone(), th.clone(), home.clone()); async move {
                    match j {
                        HistJob::Start(cwd, entry) => { hist::checkpoint0(cwd.clone(), th.clone()).await; hist::step(hub, th, cwd, entry, home, "start").await }
                        HistJob::Step(cwd, entry) => hist::step(hub, th, cwd, entry, home, "").await,
                        HistJob::End(cwd) => { hist::step(hub.clone(), th.clone(), cwd.clone(), String::new(), home, "").await; hist::checkpoint(cwd, th).await }
                    } } }));
            }
            let tx = self.hist_tx.as_ref().unwrap();
            if is_step && tx.capacity() < HIST_CAP - HIST_STEPS_MAX { self.ag.stats.hist_dropped.fetch_add(1, Relaxed); return } // a burst of tool ends: the newest state is captured by the next step or the end
            match tx.try_send((Some(JobGuard::new(&self.ag)), j.clone_job())) {
                Ok(()) => return,
                Err(mpsc::error::TrySendError::Closed(_)) if attempt == 0 => { self.hist_tx = None } // the worker ended while idle: start another
                Err(_) => { self.ag.stats.hist_dropped.fetch_add(1, Relaxed); return }
            }
        }
    }

    /// SIDE-BY-SIDE.md checks 2, 5 and the person's home: the folder, the live log's tail, and BACKPLANE_PERSON_HOME. Err: the reason, nothing starts.
    async fn person_checks(ag: Arc<Agents>, cwd: String) -> Result<(), String> {
        let home = person::person_home().ok_or("BACKPLANE_PERSON_HOME is not set: the person's agent has no home to run with")?; if !home.is_dir() { return Err(format!("BACKPLANE_PERSON_HOME {} is not a folder", home.display())) }
        let roots: Vec<String> = { let w = ag.world().await; w.roots.values().cloned().collect() };
        let live = person::live_log().ok_or("BACKPLANE_LIVE_LOG is not set: the same-folder check against the live Backplane cannot be made")?;
        let live_home = live.parent().map(|p| p.to_path_buf());
        let real = person::check_folder(&cwd, &roots, &ag.hub.cfg.home, live_home.as_deref())?;
        let (base, copy_last) = ag.copy_point.lock().unwrap().clone(); let tail = person::lines_since(&live, 16 << 20, &copy_last)?;
        let lines: Vec<&str> = tail.iter().map(String::as_str).collect();
        if let Some(why) = person::live_conflict(&person::LiveFold::fold(&base, &lines), &real) { return Err(why) }
        Ok(())
    }
    fn person_child(&self) -> Result<proc::PersonChild, String> {
        let home = person::person_home().ok_or("BACKPLANE_PERSON_HOME is not set")?; let path = std::env::join_paths(person::person_dirs(&home)).map_err(|e| e.to_string())?; Ok(proc::PersonChild { home, path })
    }
    fn person_prog(&self, cli: &str) -> Result<std::path::PathBuf, String> {
        let c = self.person_child()?; std::env::split_paths(&c.path).map(|d| d.join(cli)).find(|p| p.is_file()).ok_or_else(|| format!("{cli} is not installed for the person (looked in their PATH and the usual install directories under {})", c.home.display()))
    }

    async fn spawn(&mut self, t: &world::T, cwd: &str, key: String) -> Result<(), String> {
        remake_folder(cwd, &self.ag.world().await.roots.get(&t.project).cloned().unwrap_or_default()).await?;
        let mut prog = if self.person { self.person_prog("claude")? } else { self.ag.cli.resolve("claude")? };
        let (now, th) = (now_s(), self.th.clone());
        // a thread bound to a named account runs under that profile's own CLAUDE_CONFIG_DIR, after its identity is rechecked (no model turn); a stale or disconnected one never falls back to the default login
        let (mut acct_env, mut acct_args, mut busy) = (vec![], vec![], None);
        if !self.person {
            let raw = self.ag.hub.setting(&crate::settings::accounts::binding_key(&th, "claude")).await.unwrap_or_default(); // the hub's own settings: the agents' world does not track this key
            let b: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
            let id = b["id"].as_str().unwrap_or("").to_string();
            if !id.is_empty() {
                let Some(mgr) = self.ag.accounts.get().cloned() else { return Err("Accounts are not available; select the default account".into()) };
                let plan = mgr.launch_plan(&id, &b["epoch"].as_u64().unwrap_or(0).to_string(), "claude").await?;
                prog = plan.program; acct_env = plan.env; acct_args = plan.args; busy = Some(crate::settings::accounts_hub::Busy::new(mgr, &id));
            }
        }
        let gen = format!("{now}-{}", self.ag.seq()); // Hub.gen: the process's generation, which its MCP URL names
        let prompt = self.ag.prompt_for(&th, cwd).await;
        let mut dirs: Vec<String> = self.ag.host.kistack.iter().map(|k| k.to_string_lossy().into_owned()).collect(); dirs.push(self.ag.host.home.join("attachments").to_string_lossy().into_owned());
        let plugins: Vec<String> = self.ag.host.kistack.iter().map(|k| k.to_string_lossy().into_owned()).collect();
        let (effort, think) = { let w = self.ag.world().await; (w.setting(&format!("effort.{th}")).to_string(), self.cx.think) };
        let url = self.ag.mcp.get().map(|m| m.url(&th, &gen)).unwrap_or_default();
        let mut args = rules::claude_args(&rules::ArgsIn { model: &t.model, session: &t.session, runtime: &t.runtime, interaction: &t.interaction, prompt: &prompt, dirs: &dirs, effort: &effort, think, plugins: &plugins, mcp_url: &url });
        if !acct_args.is_empty() { acct_args.extend(args); args = acct_args }
        if self.person { args.extend(person::CLAUDE_STRICT.iter().map(|x| x.to_string())) } // only this hub's MCP config; the person's user-level hooks, MCP servers and rules do not load
        self.ag.set_gen(&th, &gen);
        let p = if self.person { proc::spawn_person(&self.person_child()?, &prog, &args, std::path::Path::new(cwd), &[]) } else { proc::spawn(&prog, &args, std::path::Path::new(cwd), self.ag.cli.dir(), &acct_env) }.map_err(|e| e.to_string())?;
        self.cmds = cmds::Cmds::new(); self.wait.clear(); self.resumed = if self.person { String::new() } else { t.session.clone() }; self.spawn_n += 1;
        self.ag.stats.procs_spawned.fetch_add(1, Relaxed);
        self.live_proc = Some(Live { input: p.input, pid: p.pid, key, person: self.person, _busy: busy }); self.lines = Some(p.lines); self.exit = Some(p.exit);
        Ok(())
    }

    /// The turn ends in an error row and a failed turn (Hub.spawned.fail, Hub.agent_gone).
    async fn fail(&mut self, text: String, kind: &str, act: &str) { self.fail_k(text, kind, act, false).await }
    /// `ended`: the failure is an agent-gone end; a spawn failure is not marked.
    async fn fail_k(&mut self, text: String, kind: &str, act: &str, ended: bool) {
        let (now, th) = (now_s(), self.th.clone());
        let mut ev = vec![json!({"$":"ActivityLogged","thread":th,"act":act,"tone":"error","kind":kind,"summary":text,"at":now}), json!({"$":"TurnChanged","thread":th,"state":"failed","at":now})]; if ended && MARK_ENDS { ev[1]["_end"] = json!(true) };
        if ended && !self.last_user.is_empty() { ev.push(set(format!("failed.{}", self.last_user), "1".into(), now)) } // writes failed.<msg> for an agent-gone end, not for a spawn failure
        self.live_close().await; self.commit(ev).await; self.cx.running = false; self.jobs_end();
    }

    /// Hub.interrupt: only a running turn is stopped. What streamed so far is posted as the reply; Claude keeps its process.
    async fn interrupt(&mut self) {
        self.ag.stats.interrupts.fetch_add(1, Relaxed);
        let running = { let w = self.ag.world().await; w.running(&self.th) };
        if !running { return }
        let (now, th) = (now_s(), self.th.clone()); let mut ev = vec![];
        if !self.live.trim().is_empty() && self.live.len() < LIVE_CAP {
            ev.push(json!({"$":"MessagePosted","thread":th,"msg":format!("live:{th}:{}:stopped", self.live_gen),"role":"assistant","text":self.live,"at":now}));
            self.ahead.push_str(&self.live.clone());
        }
        ev.push(json!({"$":"TurnChanged","thread":th,"state":"interrupted","at":now}));
        let user = if self.last_user.is_empty() { self.ag.world().await.threads.get(&th).map(|t| t.last_user.clone()).unwrap_or_default() } else { self.last_user.clone() };
        if !user.is_empty() { ev.push(set(format!("stopped.{user}"), "1".into(), now)) }
        ev.push(set(format!("restart.retry.{th}"), String::new(), now)); ev.push(set(format!("provider.resume.{th}"), String::new(), now));
        self.rid += 1;
        if let Some(p) = &self.live_proc { let _ = p.input.try_send(rules::interrupt_line(&format!("int-{now}-{}", self.rid))); }
        // Claude.unread: the commands the CLI held queued are cancelled unread; the person is told which
        let waiting = self.cmds.waiting(); self.cmds = self.cmds.stop(&[]);
        let xs: Vec<&String> = self.sent.iter().filter(|(u, _, _)| waiting.contains(u)).map(|(_, _, t)| t).collect();
        if !xs.is_empty() {
            let q = xs.iter().map(|x| format!("\u{201C}{}\u{201D}", x.trim().chars().take(80).collect::<String>().replace(['\t', '\n'], " "))).collect::<Vec<_>>().join(" · "); let n = xs.len();
            let say = if n == 1 { format!("Stop cancelled a message the agent had not read yet: {q}. Send it again if you still want it.") } else { format!("Stop cancelled {n} messages the agent had not read yet: {q}. Send them again if you still want them.") };
            ev.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("u", now, self.ag.seq()),"tone":"error","kind":"stop","summary":say,"at":now}));
        }
        self.live_close().await; self.commit(ev).await; self.cx.running = false; self.jobs_end();
        if let (Some(d), Some(p)) = (self.driver.as_mut(), self.live_proc.as_ref()) { for s in d.on_stop() { let _ = p.input.try_send(s); } }
        if self.codex { self.stop().await; self.lines = None; self.exit = None; self.next_ride = false; self.codex_next().await } // AgentStop ends the process; mail waiting starts the next turn (ride_interrupted_starts)
    }

    /// One read's lines: deltas merged, each line judged by the command ledger and turned into events, the events committed together.
    async fn batch(&mut self, ls: Vec<String>) {
        if self.codex { return self.codex_batch(ls).await }
        let st = &self.ag.stats; st.lines.fetch_add(ls.len() as u64, Relaxed); st.max_batch_lines.fetch_max(ls.len() as u64, Relaxed);
        let ls = lines::merge(ls); let pace = lines::streaming(&ls); let at = now_s(); self.heard = Instant::now();
        let mut events: Vec<Value> = vec![]; let mut clear = false; let mut lost = false; let mut sends: Vec<String> = vec![]; let mut act: Option<crate::settings::driver::Act> = None;
        for l in ls {
            if l.trim().is_empty() { continue }
            let Ok(v) = serde_json::from_str::<Value>(&crate::store::clean(&l)) else { self.ag.stats.bad_lines.fetch_add(1, Relaxed); continue };
            if v["type"] == "system" && v["subtype"] == "init" { self.resumed.clear() } // the session it resumed (or made) exists
            if v["type"] == "result" && v["is_error"] == true && self.can_retry_lost() && lines::Cx::why_of(&v).contains(SESSION_GONE) { lost = true; continue } // the CLI never had the session: not a failed turn yet (session_lost)
            if let Some(p) = crate::statehost::policies() { events.extend(p.line(&self.th, at, &v)) } // Watchdog.line: a long streamed turn is not silent to the liveness record
            // Prog: a main-agent task_progress line is written at most once every few seconds (the newest of each task always lands)
            if v["type"] == "system" && v["subtype"] == "task_progress" && v["parent_tool_use_id"].as_str().unwrap_or("").is_empty() {
                let s = self.prog.line(at, v); self.prog = bg::Prog { at: s.at, pend: s.pend }; if !s.flush.is_empty() { events.extend(self.cx.progress(&s.flush, at)) } continue
            }
            let verdict = self.cmds.verdict(self.cx.running, &v); let text = self.cmds.text(&v); self.cmds = self.cmds.see(&v);
            let fx = match verdict {
                0 => self.cx.line(&v, at),
                1 => self.cx.line_flow(&v, at, Flow::Quiet),
                2 => self.cx.line_flow(&v, at, Flow::Stream),
                3..=5 => {
                    let text = if text.trim().is_empty() { lines::Cx::why_of(&v) } else { text };
                    if verdict >= 4 { sends.extend(self.cx.bg_stop_lines(&format!("b{at}-{}", self.ag.seq()))) }
                    lines::Fx { events: self.cx.end(verdict, &text, at), ..Default::default() }
                }
                _ => { // 6: Claude started a turn by itself (a background task ended, a wakeup): the thread runs again, with an Act saying why
                    let th = self.th.clone(); let seq = self.ag.seq();
                    let why = lines::wake_text(v["type"] == "command_lifecycle", &self.cx.bg, &v);
                    let newest = { let m = self.ag.hub.model().await; m.threads.get(&th).map(|x| x.last_entry.clone()).unwrap_or_default() };
                    let mut ev = self.cx.wake_start(&why, &newest, seq, at);
                    let mut fx = self.cx.line_flow(&v, at, Flow::Stream); ev.append(&mut fx.events); fx.events = ev; fx
                }
            };
            let terminal = fx.events.iter().any(|e| e["$"] == "TurnChanged" && matches!(e["state"].as_str(), Some("failed" | "interrupted" | "completed")));
            let verdict = crate::settings::driver::on_line(&self.ag.hub, &self.th, &v, terminal).await; // a provider quota/limit line: hold, switch, start the same request again (Fallback.line)
            let (drop_end, vevents) = (verdict.drop_turn_end, verdict.events); if verdict.act.is_some() { act = verdict.act }
            if v["type"] == "user" && v["parent_tool_use_id"].as_str().unwrap_or("").is_empty() { // Steps.claude: a step after the first tool result of a line
                if let Some(id) = v["message"]["content"].as_array().and_then(|a| a.iter().find(|b| b["type"] == "tool_result")).and_then(|b| b["tool_use_id"].as_str()) { if !id.is_empty() { self.jobs_step(id) } } }
            if !drop_end && fx.events.iter().any(|e| e["$"] == "TurnChanged" && (e["state"] == "completed" || e["state"] == "failed" || e["state"] == "interrupted")) { self.jobs_end() }
            for d in &fx.deltas {
                self.ag.stats.deltas.fetch_add(1, Relaxed);
                if self.live.is_empty() { self.live_gen = now_ms() }
                if self.live.len() + d.len() <= LIVE_CAP { self.live.push_str(d) }
                let _ = self.ag.hub.live_delta(&self.th, d).await;
            }
            for mut e in fx.events {
                if drop_end && e["$"] == "TurnChanged" && e["state"] == "failed" { continue } // the driver commits Interrupted and the switch itself
                if MARK_ENDS && e["$"] == "TurnChanged" && e["state"] != "running" { e["_end"] = json!(true) } // Ended.out runs for a turn an agent line ends (state's policy writes the end records and removes the marker)
                if e["$"] == "MessagePosted" && e["role"] == "assistant" && !e["msg"].as_str().unwrap_or("").ends_with(":before") {
                    // Steer.whole: after a reply posted ahead, only what streamed after the cut is posted
                    let mut e = e; let text = e["text"].as_str().unwrap_or("").to_string();
                    if !self.ahead.is_empty() && text.starts_with(&self.ahead) { let rest = self.live.clone(); if rest.trim().is_empty() { self.ahead.clear(); continue } e["text"] = json!(rest) }
                    self.ahead.clear(); events.push(e); continue;
                }
                if e["$"] == "TurnChanged" && e["state"] == "failed" && !self.last_user.is_empty() { events.push(e); events.push(set(format!("failed.{}", self.last_user), "1".into(), at)); clear = true; continue }
                if e["$"] == "TurnChanged" && e["state"] != "running" { clear = true }
                events.push(e);
            }
            events.extend(vevents);
            if fx.delta_end { self.live_end().await } // Claude.assistant's Broadcast: the end comes in line order with the deltas of the lines around it
            if cmds::moves(&v) { // Wait.line: the messages the CLI has not read yet
                let ids: Vec<&str> = self.cmds.unread().iter().filter_map(|u| self.sent.iter().find(|(x, _, _)| x == u).map(|(_, m, _)| m.as_str())).collect::<Vec<_>>().into_iter().map(|s| s).collect();
                let w = ids.join(" "); if w != self.wait { events.push(set(format!("wait.{}", self.th), w.clone(), at)); self.wait = w }
            }
        }
        if clear { self.live_close().await; }
        self.commit(events).await;
        if lost && self.session_lost().await { return }
        if let Some(a) = act { crate::settings::driver::spawn_act(self.ag.clone(), self.th.clone(), a) } // after the commit, from a task of its own
        if let Some(p) = &self.live_proc { for s in sends { let _ = p.input.try_send(s); } }
        if pace { self.pacing = Some(Instant::now() + Duration::from_millis(lines::PACE_MS)) }
    }

    /// The once-a-second beat: held progress lands once its gap is over; a running turn whose ledger shows nothing at work ends (Stuck).
    async fn beat(&mut self) {
        let at = now_s(); let mut ev = vec![];
        if !self.prog.pend.is_empty() { let s = self.prog.beat(at); self.prog = bg::Prog { at: s.at, pend: s.pend }; if !s.flush.is_empty() { ev.extend(self.cx.progress(&s.flush, at)) } }
        let after = std::env::var("BACKPLANE_STUCK_S").ok().and_then(|s| s.parse().ok()).unwrap_or(120u64);
        let code = self.cmds.stuck(self.cx.running, 0, self.heard.elapsed().as_secs(), self.cx.bg.len(), after);
        if code != 0 {
            self.cmds = self.cmds.cleared();
            ev.push(json!({"$":"ActivityLogged","thread":self.th,"act":hub_id("x", at, self.ag.seq()),"tone":"info","kind":"stuck","summary":"Its agent finished its last command but its turn never ended: ended now","at":at}));
            let text = String::new(); ev.extend(self.cx.end(code, &text, at)); if code == 4 && !self.last_user.is_empty() { ev.push(set(format!("failed.{}", self.last_user), "1".into(), at)) }
            self.live_close().await;
        }
        self.commit(ev).await;
    }

    /// The process is gone: what it still wrote is handled first, then Hub.agent_gone: a running turn fails.
    async fn gone(&mut self, e: Exit) {
        let n0 = self.spawn_n;
        if let Some(mut rx) = self.lines.take() { while let Ok(Some(b)) = tokio::time::timeout(Duration::from_millis(200), next_batch(&mut rx)).await { self.batch(b).await } }
        if self.spawn_n != n0 { return } // what it wrote was its lost session: another process runs the message now (session_lost)
        self.live_proc = None; self.ag.stats.procs_exited.fetch_add(1, Relaxed);
        if self.cx.running && e.stderr.contains(SESSION_GONE) && self.can_retry_lost() && self.session_lost().await { return } // it said so on stderr only
        if self.cx.running {
            let code = e.code.or(e.signal.map(|s| 128 + s)).unwrap_or(1);
            self.fail_k(match self.driver.as_ref().and_then(|d| d.exit_why(code)) { Some(w) => rules::exit_say(&w, code, &e.stderr), None => rules::exit_why(code) }, "exit", &hub_id("x", now_s(), 0), true).await;
        }
        self.cmds = cmds::Cmds::new(); self.wait.clear();
        let live = { let w = self.ag.world().await; w.threads.get(&self.th).map_or(false, |t| !t.archived && !t.deleted) };
        if live { self.lost("exit").await }
        if self.next_ride { self.next_ride = false; self.codex_next().await; return }
        let cleared = self.cx.bg_clear(now_s()); self.commit(cleared).await; // lostwork_exit_clears: the process is gone, and with it every row it filed
    }

    /// A Claude start that asked to resume a session may be refused because the CLI no longer has it; only then, and only for the message that started the process.
    fn can_retry_lost(&self) -> bool { !self.codex && self.cx.running && !self.resumed.is_empty() && self.spawn_in.is_some() }
    /// The CLI says "No conversation found with session ID": the thread's id is cleared and the same message runs in a fresh session with the recent conversation
    /// handed over, said once as an info row. One retry at most: the new process resumes nothing (`resumed` is empty), so a second refusal is an ordinary failed turn.
    /// Every start of a Claude turn (a person's, a bot's, a queued message, a restart's carry-on, a ride) goes through `claude_send`, which remembers what started the process.
    async fn session_lost(&mut self) -> bool {
        let Some((msg, text, agent_text)) = self.spawn_in.clone() else { return false };
        if !self.can_retry_lost() { return false }
        let (t, cwd) = { let w = self.ag.world().await; match w.threads.get(&self.th) { Some(t) => (t.clone(), w.cwd(&self.th)), None => return false } };
        if t.archived || t.deleted { return false }
        let (now, th) = (now_s(), self.th.clone());
        if self.live_proc.is_some() { self.ag.stats.procs_exited.fetch_add(1, Relaxed); }
        self.stop().await; self.lines = None; self.exit = None; self.cmds = cmds::Cmds::new(); self.wait.clear(); self.resumed.clear(); self.spawn_in = None; self.cx.session.clear();
        self.live_close().await;
        let ctx = crate::commands::lost_session_context(&self.ag.hub, &th, &text).await;
        self.commit(vec![json!({"$":"SessionSet","id":th,"session":"","at":now}),
            json!({"$":"ActivityLogged","thread":th,"act":hub_id("ls", now, self.ag.seq()),"tone":"info","kind":"session","summary":"The agent's earlier session is gone, so this message runs in a new session with the recent conversation handed over.","at":now})]).await;
        let mut t = t; t.session.clear();
        let agent_text = if ctx.is_empty() { agent_text } else { format!("{ctx}\n\n{agent_text}") };
        self.claude_send(&t, &cwd, &msg, &text, &agent_text, now).await;
        true
    }

    /// Bg.lost: the process that owned the thread's background work is gone; the agent is told once (a message), or with restart.continue off the timeline is.
    async fn lost(&mut self, why: &str) {
        let rows = self.cx.bg.clone(); if !bg::lost_new(&rows) { return }
        let (now, th) = (now_s(), self.th.clone()); let text = bg::lost_text(&rows, why);
        let carry = { let w = self.ag.world().await; w.setting("restart.continue") != "off" };
        if carry {
            let cleared = self.cx.bg_clear(now); self.commit(cleared).await;
            let id = format!("t:backplane:{}", hub_id("l", now, self.ag.seq()));
            let ag = self.ag.clone(); tokio::spawn(async move { let _ = ag.inbound("system", &th, &id, &text, ("backplane".into(), "system".into(), String::new()), vec![]).await; });
        } else {
            let unknown = bg::unknown(&rows); let mut ev = vec![json!({"$":"ActivityLogged","thread":th,"act":hub_id("l", now, self.ag.seq()),"tone":"info","kind":"lost","summary":text,"at":now})];
            ev.extend(self.cx.bg_replace(unknown, now)); self.commit(ev).await;
        }
    }
}

#[cfg(test)]
mod remake_tests {
    #[tokio::test] async fn a_vanished_folder_is_made_again_never_stopping_the_turn() {
        let d = std::env::temp_dir().join(format!("remake-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d);
        let root = d.join("proj"); std::fs::create_dir_all(&root).unwrap();
        for a in [vec!["init", "-q"], vec!["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "x"]] { std::process::Command::new("git").arg("-C").arg(&root).args(a).status().unwrap(); }
        let gone = d.join("lanes/imp-1");
        super::remake_folder(gone.to_str().unwrap(), root.to_str().unwrap()).await.unwrap();
        assert!(gone.is_dir() && gone.join(".git").exists(), "a fresh worktree");
        let plain = d.join("plain/x"); super::remake_folder(plain.to_str().unwrap(), "").await.unwrap(); assert!(plain.is_dir());
        let _ = std::fs::remove_dir_all(&d);
    }
}

//! M5: agent process management. One actor per thread owns that thread's agent process (Claude: one
//! long-lived stream-json process), turns the lines it writes into the events the JS hub writes, and commits
//! them through `Hub::commit_many`. Ported from src/core/hub.bend (Hub.post.go, Claude.line.k, Retire.turn.f,
//! Hub.agent_gone, Hub.recover, Carry.*) and the Claude paths in src/server/server.bend.
//!
//! Ported: Claude. Out (answered with an error, never silently): Codex, Grok, OpenCode, Muse, Antigravity, Pi, Oh My Pi.
//! Out for Claude: images/`$skill` input, the Backplane MCP server and runtime prompt in the agent's arguments,
//! subagents and background tasks, todo tools, the allowance stream, the legacy queue, mail provenance beyond carry,
//! and the hub's time-driven policies (liveness, autosettle, alert/told/summary, star nudge), which belong to M4.
pub mod bg; pub mod cmds; pub mod host; pub mod inputs; pub mod lines; pub mod mcp; pub mod proc; pub mod prompt; pub mod prompt_text; pub mod rules; pub mod tools; pub mod world;
#[cfg(test)] mod tests;

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
pub const RECENT: usize = 256;        // message ids remembered per thread against a resend

#[derive(Default)]
pub struct Stats { pub turns: AtomicU64, pub procs_spawned: AtomicU64, pub procs_exited: AtomicU64, pub spawn_failures: AtomicU64, pub lines: AtomicU64, pub deltas: AtomicU64,
    pub events: AtomicU64, pub commit_errors: AtomicU64, pub interrupts: AtomicU64, pub refused_full: AtomicU64, pub bad_lines: AtomicU64, pub max_batch_lines: AtomicU64 }

pub enum Cmd {
    User { msg: String, text: String, role: String, origin: Option<(String, String, String)>, extra: Vec<Value>, reply: oneshot::Sender<Result<(), String>> },
    Interrupt { reply: oneshot::Sender<()> },
}

pub struct Agents { pub hub: Arc<Hub>, pub cli: Cli, pub host: host::HostInfo, world: Mutex<World>, actors: StdMutex<HashMap<String, mpsc::Sender<Cmd>>>, pub stats: Stats, n: AtomicU64,
    pub mcp: std::sync::OnceLock<Arc<mcp::Mcp>>, gens: StdMutex<HashMap<String, String>>, pub parked: StdMutex<HashMap<String, mcp::Parked>> }

pub fn now_s() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }
pub fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }
fn ok(id: &Value) -> Value { json!({"t":"reply","id":id,"ok":true}) }
fn err(id: &Value, e: &str) -> Value { json!({"t":"reply","id":id,"ok":false,"error":e}) }
fn set(k: String, v: String, at: u64) -> Value { json!({"$":"SettingSet","key":k,"value":v,"at":at}) }

impl Agents {
    pub async fn start(hub: Arc<Hub>) -> Result<Arc<Agents>> { let h = host::HostInfo::probe(&hub.cfg.home); Agents::start_host(hub, Cli::from_env(), h).await }
    pub async fn start_with(hub: Arc<Hub>, cli: Cli) -> Result<Arc<Agents>> { Agents::start_host(hub, cli, host::HostInfo::default()).await }
    pub async fn start_host(hub: Arc<Hub>, cli: Cli, host: host::HostInfo) -> Result<Arc<Agents>> {
        let ag = Arc::new(Agents { hub: hub.clone(), cli, host, world: Mutex::new(World::default()), actors: StdMutex::new(HashMap::new()), stats: Stats::default(), n: AtomicU64::new(0),
            mcp: std::sync::OnceLock::new(), gens: StdMutex::new(HashMap::new()), parked: StdMutex::new(HashMap::new()) });
        let _ = ag.mcp.set(mcp::Mcp::start(&ag).await?);
        ag.recover().await;
        hub.register_rpc(Arc::new(Rpc(ag.clone())));
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
        let (ipc, orders, lang, buy, names) = { let w = self.world().await;
            (w.setting("kicad.ipc") != "off", w.setting("order.enabled") != "off", w.setting("agent.lang").to_string(), !w.setting(&format!("buy.{th}")).is_empty(), ()) };
        let _ = names;
        let (root, home, host) = (cwd.to_string(), self.host.home.clone(), self.host.clone());
        tokio::task::spawn_blocking(move || {
            let f = inputs::read(&root); let keys_path = home.join("secrets/api.env");
            let keys = prompt::api_keys(&keys_path.to_string_lossy(), &prompt::key_names(&std::fs::read_to_string(&keys_path).unwrap_or_default())) + &prompt::secrets(&[]);
            prompt::build(&prompt::PromptEnv { fc: host.fc.clone(), render: host.render.clone(), kicad: host.kicad.clone(), kicad_ipc: ipc, models: host.models_text(), may_delegate: true, orders_on: orders,
                root, pro: f.pro, pcb: f.pcb, sch: f.sch, all: f.all, keys, machines: String::new(), buy, lang })
        }).await.unwrap_or_default()
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
    pub fn shutdown(&self) { self.actors.lock().unwrap().clear(); }
    pub fn actors(&self) -> usize { self.actors.lock().unwrap().len() }

    /// Hub.turn then Hub.post.go: take the person's message, start or feed the thread's agent. Acknowledged after the events are durable.
    pub async fn deliver(self: &Arc<Self>, thread: &str, msg: &str, text: &str, origin: Option<(String, String, String)>, extra: Vec<Value>) -> Result<(), String> { self.deliver_as("user", thread, msg, text, origin, extra).await }
    pub async fn deliver_as(self: &Arc<Self>, role: &str, thread: &str, msg: &str, text: &str, origin: Option<(String, String, String)>, extra: Vec<Value>) -> Result<(), String> {
        for _ in 0..3 {
            let tx = self.actor(thread); let (reply, rx) = oneshot::channel();
            match tx.try_send(Cmd::User { msg: msg.into(), text: text.into(), role: role.into(), origin: origin.clone(), extra: extra.clone(), reply }) {
                Ok(()) => return rx.await.unwrap_or_else(|_| Err("the thread's agent stopped".into())),
                Err(mpsc::error::TrySendError::Full(_)) => { self.stats.refused_full.fetch_add(1, Relaxed); return Err("too many messages are waiting for this thread".into()) }
                Err(mpsc::error::TrySendError::Closed(_)) => { self.actors.lock().unwrap().remove(thread); }
            }
        }
        Err("the thread's agent is busy".into())
    }
    pub async fn interrupt(self: &Arc<Self>, thread: &str) -> Result<(), String> {
        let tx = self.actor(thread); let (reply, rx) = oneshot::channel();
        tx.try_send(Cmd::Interrupt { reply }).map_err(|_| "too many requests are waiting for this thread".to_string())?;
        rx.await.map_err(|_| "the thread's agent stopped".to_string())
    }

    /// Hub.recover.k (Rec.turn) and Carry: a turn the restart cut short is marked stopped with a note; with restart.continue on
    /// (and not repeating itself) it goes on with a message, BURST threads at once, the rest stay stopped.
    pub async fn recover(self: &Arc<Self>) {
        let (cut, carry): (Vec<Value>, Vec<String>) = {
            let w = self.world().await; let now = now_s(); let zone = w.setting("tz.machine").to_string(); let mut ev = vec![]; let mut carry = vec![]; let mut told = std::collections::HashSet::new();
            // Rec.asks: an open ask is closed unanswered and its thread told which one went unanswered (an order's question stays open)
            let mut asks: Vec<(&String, &world::Ask)> = w.asks.iter().filter(|(_, a)| !a.tool.starts_with("order_ask")).collect(); asks.sort_by(|a, b| a.0.cmp(b.0));
            for (id, a) in asks {
                ev.push(json!({"$":"AskClosed","id":id,"answer":"","at":now}));
                if w.threads.contains_key(&a.thread) { told.insert(a.thread.clone());
                    ev.push(json!({"$":"ActivityLogged","thread":a.thread,"act":hub_id("r", now, self.seq()),"tone":"error","kind":"restart","summary":format!("{}{}", rules::restart_cut(&zone, now), rules::unanswered(&a.kind, &a.tool, &a.detail)),"at":now})) }
            }
            let mut ths: Vec<(&String, &world::T)> = w.threads.iter().filter(|(_, t)| t.turn == "running").collect(); ths.sort_by(|a, b| a.0.cmp(b.0));
            for (th, t) in ths {
                if !told.contains(th) { ev.push(json!({"$":"ActivityLogged","thread":th,"act":hub_id("r", now, self.seq()),"tone":"error","kind":"restart","summary":rules::restart_cut(&zone, now),"at":now})) }
                ev.push(json!({"$":"TurnChanged","thread":th,"state":"interrupted","at":now}));
                if !t.last_user.is_empty() { ev.push(set(format!("stopped.{}", t.last_user), "1".into(), now)) }
                let will = world::carry_will(t, w.setting("restart.continue"));
                if will && carry.len() < BURST { carry.push(th.clone()) }
            }
            (ev, carry)
        };
        if !cut.is_empty() { if let Err(e) = self.hub.commit_many(&cut).await { eprintln!("agents: recovery: {e:#}") } }
        for th in carry {
            let now = now_s(); let id = hub_id("c", now, self.seq());
            let note = json!({"$":"ActivityLogged","thread":th,"act":hub_id("r", now, self.seq()),"tone":"info","kind":"restart","summary":"Continuing after the restart","at":now});
            let text = format!("{} {}", world::CARRY_TEXT, rules::CARRY_CUT);
            if let Err(e) = self.deliver(&th, &id, &text, Some(("backplane".into(), "carry".into(), String::new())), vec![note]).await { eprintln!("agents: carrying on {th}: {e}") }
        }
    }
}

struct Rpc(Arc<Agents>);
impl RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["turn.start", "turn.interrupt", "ask.answer"] }
    fn call<'a>(&'a self, _hub: &'a Arc<Hub>, _who: &'a Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if method == "ask.answer" { return match self.0.answer(p.get("id").and_then(Value::as_str).unwrap_or(""), p.get("answer").and_then(Value::as_str).unwrap_or("")).await { Ok(()) => vec![ok(id)], Err(e) => vec![err(id, &e)] } }
            let th = p.get("thread").and_then(Value::as_str).unwrap_or("");
            if th.is_empty() || !self.0.world().await.threads.contains_key(th) { return vec![err(id, "unknown thread")] }
            let r = match method {
                "turn.start" => {
                    let text = p.get("text").and_then(Value::as_str).unwrap_or("");
                    let msg = p.get("msg").and_then(Value::as_str).filter(|m| !m.is_empty()).map(str::to_string).unwrap_or_else(|| hub_id("m", now_s(), self.0.seq()));
                    self.0.deliver(th, &msg, text, None, vec![]).await
                }
                _ => self.0.interrupt(th).await,
            };
            match r { Ok(()) => vec![ok(id)], Err(e) => vec![err(id, &e)] }
        })
    }
}

struct Live { input: mpsc::Sender<String>, pid: u32, key: String }

struct Actor {
    ag: Arc<Agents>, th: String, cx: Cx, live_proc: Option<Live>,
    lines: Option<mpsc::Receiver<String>>, exit: Option<oneshot::Receiver<Exit>>,
    live: String, live_gen: u64, ahead: String, last_user: String, recent: VecDeque<String>, pacing: Option<Instant>, rid: u64,
    cmds: cmds::Cmds, sent: VecDeque<(String, String, String)>, wait: String, prog: bg::Prog, heard: Instant,
}

async fn next_batch(rx: &mut mpsc::Receiver<String>) -> Option<Vec<String>> {
    let first = rx.recv().await?; let mut bytes = first.len(); let mut v = vec![first];
    while bytes < BATCH_BYTES { match rx.try_recv() { Ok(l) => { bytes += l.len(); v.push(l) } Err(_) => break } }
    Some(v)
}

async fn recv_opt(r: &mut Option<mpsc::Receiver<String>>) -> Option<Vec<String>> { match r { Some(rx) => next_batch(rx).await, None => std::future::pending().await } }
async fn exit_opt(r: &mut Option<oneshot::Receiver<Exit>>) -> Result<Exit, oneshot::error::RecvError> { match r { Some(rx) => rx.await, None => std::future::pending().await } }

impl Actor {
    fn new(ag: Arc<Agents>, th: String) -> Actor { Actor { cx: Cx::new(&th), ag, th, live_proc: None, lines: None, exit: None, live: String::new(), live_gen: 0, ahead: String::new(), last_user: String::new(), recent: VecDeque::new(), pacing: None, rid: 0, cmds: cmds::Cmds::new(), sent: VecDeque::new(), wait: String::new(), prog: bg::Prog::default(), heard: Instant::now() } }

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

    async fn cmd(&mut self, c: Cmd) {
        match c {
            Cmd::User { msg, text, role, origin, extra, reply } => { let r = self.user(msg, text, role, origin, extra).await; let _ = reply.send(r); }
            Cmd::Interrupt { reply } => { self.interrupt().await; let _ = reply.send(()); }
        }
    }

    /// Hub.turn.go + Hub.post.go (the Claude path: the message goes straight to the process, running or not).
    async fn user(&mut self, msg: String, text: String, role: String, origin: Option<(String, String, String)>, extra: Vec<Value>) -> Result<(), String> {
        if self.recent.contains(&msg) { return Ok(()) } // Hub.dup: a message resent is stored once
        let (t, cwd, cx) = {
            let w = self.ag.world().await;
            let Some(t) = w.threads.get(&self.th).cloned() else { return Err("unknown thread".into()) };
            (t, w.cwd(&self.th), w.cx(&self.th))
        };
        if t.provider != "claude" { return Err(format!("{} threads are not run by the Rust hub yet", if t.provider.is_empty() { "this provider's" } else { &t.provider })) }
        self.cx = cx; self.last_user = msg.clone();
        let (now, th) = (now_s(), self.th.clone());
        let mut ev: Vec<Value> = vec![];
        if t.title == "New thread" { ev.push(json!({"$":"ThreadRenamed","id":th,"title":rules::title_of(&text),"at":now})) }
        // Steer.change: a message into a streaming reply posts what streamed, up to its last line break, ahead of it
        let head = rules::steer_head(&self.live);
        if !head.is_empty() {
            ev.push(json!({"$":"MessagePosted","thread":th,"msg":format!("{msg}:before"),"role":"assistant","text":head,"at":now}));
            let tail = rules::steer_tail(&self.live); self.ahead.push_str(&head); self.live_end().await;
            if !tail.is_empty() { self.live = tail.clone(); let _ = self.ag.hub.live_delta(&th, &tail).await; }
        }
        ev.push(json!({"$":"MessagePosted","thread":th,"msg":msg,"role":role,"text":text,"at":now}));
        if let Some((from, kind, r)) = &origin { ev.push(json!({"$":"MessageFrom","thread":th,"msg":msg,"from":from,"kind":kind,"ref":r,"at":now})) }
        ev.push(json!({"$":"TurnChanged","thread":th,"state":"running","at":now}));
        ev.push(set(format!("turn.from.{th}"), format!("@{msg}"), now));            // Turn.mark: the newest entry then
        ev.push(set(format!("restart.retry.{th}"), String::new(), now));             // Fallback.manual
        ev.push(set(format!("provider.resume.{th}"), String::new(), now));
        ev.extend(extra);
        ev.push(set(format!("fallback.input.{th}"), text.clone(), now));             // Fallback.attempt
        self.commit(ev).await;
        self.recent.push_back(msg.clone()); if self.recent.len() > RECENT { self.recent.pop_front(); }
        self.cx.running = true; self.ag.stats.turns.fetch_add(1, Relaxed);
        let key = rules::agent_key(&t.model, &t.runtime, &t.interaction, self.cx.think);
        if self.live_proc.as_ref().map_or(true, |p| p.key != key) {
            self.stop().await; self.lines = None; self.exit = None;
            if let Err(e) = self.spawn(&t, &cwd, key).await { self.fail(format!("could not start claude: {e}"), "spawn", &format!("spawn-{now}")).await; self.ag.stats.spawn_failures.fetch_add(1, Relaxed); return Ok(()) }
        }
        let uuid = rules::hub_uuid(now, &format!("{th}/{msg}"));
        self.cmds = self.cmds.accept(&uuid); self.sent.push_back((uuid.clone(), msg.clone(), text.clone())); if self.sent.len() > RECENT { self.sent.pop_front(); }
        let line = { let (text2, cwd2, host, uuid2) = (text.clone(), cwd.clone(), self.ag.host.clone(), uuid.clone()); // Media.claude: images read, `$skill` mentions made real
            tokio::task::spawn_blocking(move || { let ss = if text2.contains('$') { inputs::scan(&host.user_home, &cwd2, host.kistack.as_deref()) } else { vec![] }; let body = inputs::expand_claude(&text2, &ss); let imgs = inputs::read_images(&text2); inputs::claude_line(&body, &imgs, &uuid2) }).await.unwrap_or_default() };
        let sent = self.live_proc.as_ref().map(|p| p.input.try_send(line));
        if matches!(sent, Some(Ok(()))) { self.cmds = self.cmds.sent(&uuid) } else { self.fail("the agent is not taking input".into(), "input", &hub_id("in", now, 0)).await }
        Ok(())
    }

    async fn spawn(&mut self, t: &world::T, cwd: &str, key: String) -> Result<(), String> {
        if !std::path::Path::new(cwd).is_dir() { return Err(format!("its folder {cwd} is gone")) }
        let prog = self.ag.cli.resolve("claude")?;
        let (now, th) = (now_s(), self.th.clone());
        let gen = format!("{now}-{}", self.ag.seq()); // Hub.gen: the process's generation, which its MCP URL names
        let prompt = self.ag.prompt_for(&th, cwd).await;
        let mut dirs: Vec<String> = self.ag.host.kistack.iter().map(|k| k.to_string_lossy().into_owned()).collect(); dirs.push(self.ag.host.home.join("attachments").to_string_lossy().into_owned());
        let plugins: Vec<String> = self.ag.host.kistack.iter().map(|k| k.to_string_lossy().into_owned()).collect();
        let (effort, think) = { let w = self.ag.world().await; (w.setting(&format!("effort.{th}")).to_string(), self.cx.think) };
        let url = self.ag.mcp.get().map(|m| m.url(&th, &gen)).unwrap_or_default();
        let args = rules::claude_args(&rules::ArgsIn { model: &t.model, session: &t.session, runtime: &t.runtime, interaction: &t.interaction, prompt: &prompt, dirs: &dirs, effort: &effort, think, plugins: &plugins, mcp_url: &url });
        self.ag.set_gen(&th, &gen);
        let p = proc::spawn(&prog, &args, std::path::Path::new(cwd), self.ag.cli.dir(), &[]).map_err(|e| e.to_string())?;
        self.cmds = cmds::Cmds::new(); self.wait.clear();
        self.ag.stats.procs_spawned.fetch_add(1, Relaxed);
        self.live_proc = Some(Live { input: p.input, pid: p.pid, key }); self.lines = Some(p.lines); self.exit = Some(p.exit);
        Ok(())
    }

    /// The turn ends in an error row and a failed turn (Hub.spawned.fail, Hub.agent_gone).
    async fn fail(&mut self, text: String, kind: &str, act: &str) {
        let (now, th) = (now_s(), self.th.clone());
        let mut ev = vec![json!({"$":"ActivityLogged","thread":th,"act":act,"tone":"error","kind":kind,"summary":text,"at":now}), json!({"$":"TurnChanged","thread":th,"state":"failed","at":now})];
        if !self.last_user.is_empty() { ev.push(set(format!("failed.{}", self.last_user), "1".into(), now)) }
        self.live_end().await; self.commit(ev).await; self.cx.running = false;
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
        self.live_end().await; self.commit(ev).await; self.cx.running = false;
    }

    /// One read's lines: deltas merged, each line judged by the command ledger and turned into events, the events committed together.
    async fn batch(&mut self, ls: Vec<String>) {
        let st = &self.ag.stats; st.lines.fetch_add(ls.len() as u64, Relaxed); st.max_batch_lines.fetch_max(ls.len() as u64, Relaxed);
        let ls = lines::merge(ls); let pace = lines::streaming(&ls); let at = now_s(); self.heard = Instant::now();
        let mut events: Vec<Value> = vec![]; let mut clear = false; let mut sends: Vec<String> = vec![];
        for l in ls {
            if l.trim().is_empty() { continue }
            let Ok(v) = serde_json::from_str::<Value>(&crate::store::clean(&l)) else { self.ag.stats.bad_lines.fetch_add(1, Relaxed); continue };
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
                    let why = if v["type"] == "command_lifecycle" { "Resumed after Stop".to_string() } else {
                        let sum = v["summary"].as_str().unwrap_or("").to_string(); let title = if !sum.is_empty() { sum } else { let id = v["task_id"].as_str().unwrap_or("");
                            let r = self.cx.bg.iter().find(|r| bg::field(r, 0) == bg::clean(id)).map(|r| { let t = bg::field(r, 3); if t.is_empty() { bg::doing(r) } else { t } }).unwrap_or_default(); if r.is_empty() { "background work".into() } else { r } };
                        let status = v["status"].as_str().unwrap_or(""); format!("Woken: {title}{}", if status.is_empty() || status == "completed" { String::new() } else { format!(" ({status})") }) };
                    let newest = { let m = self.ag.hub.model().await; m.threads.get(&th).map(|x| x.last_entry.clone()).unwrap_or_default() };
                    self.cx.running = true;
                    let mut ev = vec![json!({"$":"TurnChanged","thread":th,"state":"running","at":at}), set(format!("turn.from.{th}"), format!("@{newest}"), at), set(format!("restart.retry.{th}"), String::new(), at), set(format!("provider.resume.{th}"), String::new(), at),
                        json!({"$":"ActivityLogged","thread":th,"act":hub_id("k", at, seq),"tone":"info","kind":"wake","summary":why,"at":at})];
                    let mut fx = self.cx.line_flow(&v, at, Flow::Stream); ev.append(&mut fx.events); fx.events = ev; fx
                }
            };
            for d in &fx.deltas {
                self.ag.stats.deltas.fetch_add(1, Relaxed);
                if self.live.is_empty() { self.live_gen = now_ms() }
                if self.live.len() + d.len() <= LIVE_CAP { self.live.push_str(d) }
                let _ = self.ag.hub.live_delta(&self.th, d).await;
            }
            for e in fx.events {
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
            if fx.delta_end { clear = true }
            if cmds::moves(&v) { // Wait.line: the messages the CLI has not read yet
                let ids: Vec<&str> = self.cmds.unread().iter().filter_map(|u| self.sent.iter().find(|(x, _, _)| x == u).map(|(_, m, _)| m.as_str())).collect::<Vec<_>>().into_iter().map(|s| s).collect();
                let w = ids.join(" "); if w != self.wait { events.push(set(format!("wait.{}", self.th), w.clone(), at)); self.wait = w }
            }
        }
        if clear { self.live_end().await; }
        self.commit(events).await;
        if let Some(p) = &self.live_proc { for s in sends { let _ = p.input.try_send(s); } }
        if pace { self.pacing = Some(Instant::now() + Duration::from_millis(lines::PACE_MS)) }
    }

    /// The once-a-second beat: held progress lands once its gap is over; a running turn whose ledger shows nothing at work ends (Stuck).
    async fn beat(&mut self) {
        let at = now_s(); let mut ev = vec![];
        if !self.prog.pend.is_empty() { let s = self.prog.beat(at); self.prog = bg::Prog { at: s.at, pend: s.pend }; if !s.flush.is_empty() { ev.extend(self.cx.progress(&s.flush, at)) } }
        let after = std::env::var("BACKPLANE_STUCK_S").ok().and_then(|s| s.parse().ok()).unwrap_or(120u64);
        if self.cx.running && self.cmds.idle() && bg::running(&self.cx.bg) == 0 && self.heard.elapsed() >= Duration::from_secs(after) {
            let code = self.cmds.idle_end(); self.cmds = self.cmds.cleared();
            ev.push(json!({"$":"ActivityLogged","thread":self.th,"act":hub_id("x", at, self.ag.seq()),"tone":"info","kind":"stuck","summary":"Its agent finished its last command but its turn never ended: ended now","at":at}));
            let text = String::new(); ev.extend(self.cx.end(code, &text, at)); if code == 4 && !self.last_user.is_empty() { ev.push(set(format!("failed.{}", self.last_user), "1".into(), at)) }
            self.live_end().await;
        }
        self.commit(ev).await;
    }

    /// The process is gone: what it still wrote is handled first, then Hub.agent_gone: a running turn fails.
    async fn gone(&mut self, e: Exit) {
        if let Some(mut rx) = self.lines.take() { while let Ok(Some(b)) = tokio::time::timeout(Duration::from_millis(200), next_batch(&mut rx)).await { self.batch(b).await } }
        self.live_proc = None; self.ag.stats.procs_exited.fetch_add(1, Relaxed);
        if self.cx.running {
            let code = e.code.or(e.signal.map(|s| 128 + s)).unwrap_or(1);
            self.fail(rules::exit_why(code), "exit", &hub_id("x", now_s(), 0)).await;
        }
        self.cmds = cmds::Cmds::new(); self.wait.clear();
        let live = { let w = self.ag.world().await; w.threads.get(&self.th).map_or(false, |t| !t.archived && !t.deleted) };
        if live { self.lost("exit").await }
    }

    /// Bg.lost: the process that owned the thread's background work is gone; the agent is told once (a message), or with restart.continue off the timeline is.
    async fn lost(&mut self, why: &str) {
        let rows = self.cx.bg.clone(); if !bg::lost_new(&rows) { return }
        let (now, th) = (now_s(), self.th.clone()); let text = bg::lost_text(&rows, why);
        let carry = { let w = self.ag.world().await; w.setting("restart.continue") != "off" };
        if carry {
            let cleared = self.cx.bg_clear(now); self.commit(cleared).await;
            let id = format!("t:backplane:{}", hub_id("l", now, self.ag.seq()));
            let ag = self.ag.clone(); tokio::spawn(async move { let _ = ag.deliver_as("system", &th, &id, &text, Some(("backplane".into(), "system".into(), String::new())), vec![]).await; });
        } else {
            let unknown = bg::unknown(&rows); let mut ev = vec![json!({"$":"ActivityLogged","thread":th,"act":hub_id("l", now, self.ag.seq()),"tone":"info","kind":"lost","summary":text,"at":now})];
            ev.extend(self.cx.bg_replace(unknown, now)); self.commit(ev).await;
        }
    }
}

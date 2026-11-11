//! `backplane hub run`: start the hub, watch it, and walk the ladder in plan.rs when it fails. One guard per home.
use crate::{http::{self, Handler, Req, Server}, plan::{self, Action, Candidate, End, Fail, Mode, Next, Tuning}, store::{now, Builds, Dir}, sys};
use serde_json::{json, Value};
use std::{fs::{self, File, OpenOptions}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::{Arc, Mutex}, time::{Duration, Instant}};

/// Exit codes the hub uses for what is not its build's fault (sysexits.h): EX_TEMPFAIL and EX_CONFIG.
pub const MACHINE: i32 = 75; pub const REFUSED: i32 = 78;
const BALLAST: usize = 32 << 20;

pub struct Opts { pub home: PathBuf, pub port: u16, pub host: String, pub builds: Option<PathBuf>, pub build: String, pub log: Option<PathBuf>, pub pidfile: Option<PathBuf>, pub mem: String, pub safe: bool, pub default_home: bool, pub hub_args: Vec<String> }
struct Times { probe: u64, misses: u32, stable: u64, stall: u64, boot: u64, floor: u64 }
fn env(n: &str, d: u64) -> u64 { std::env::var(n).ok().and_then(|v| v.parse().ok()).unwrap_or(d) }

struct Guard { o: Opts, d: Dir, b: Option<Builds>, t: Tuning, tm: Times, build: String, mode: Mode, skip: Vec<String>, hist: Vec<Fail>, remote: Vec<u64>, why: String, hub_pid: u32, state: &'static str, stable: bool, last_req: Value, since: u64, probation: Option<String>, low: Vec<String>, space_at: Option<Instant>, down: Vec<String>, down_before: Vec<String> }

/// What a request changed.
enum Move { Nothing, Restart, Exit }

pub fn say(line: &str) { eprintln!("guard: {line}"); }

impl Guard {
    fn dir_of(&self, id: &str) -> PathBuf { match &self.b { Some(b) => b.dir(id), None => PathBuf::from(id) } }
    fn exists(&self, id: &str) -> bool { sys::hub_exe(&self.dir_of(id)).exists() }
    fn bad(&self) -> Vec<String> { self.b.as_ref().map_or(vec![], Builds::bad) }
    fn addr(&self) -> String { format!("{}:{}", self.o.host, self.o.port) }
    fn short(id: &str) -> &str { id.rsplit(['-', '/']).next().unwrap_or(id) }

    /// The person hears it three ways: the log, the desktop, and the app once the hub is up (it reads incidents.jsonl).
    fn alert(&self, what: &str, why: &str, did: &str) {
        say(&format!("{what}: {why}; {did}")); self.d.incident(what, why, did, &self.build);
        sys::notify("Backplane hub", &format!("{what}: {did}"));
    }
    fn rollback_to(&self) -> Option<String> { let (bad, good) = (self.bad(), self.d.good()); plan::fallback(&good, &bad, &self.build, &|b| self.exists(b)).cloned() }
    fn has_control(&self, id: &str) -> bool { self.d.lines("control").iter().any(|b| b == id) }

    fn actions(&self) -> Value {
        let mut a = vec![json!({"id": "restart", "label": if self.mode == Mode::Safe { "Restart normally" } else { "Restart hub" }})];
        if self.mode == Mode::Normal { a.push(json!({"id": "safe", "label": "Restart in safe mode", "text": "Agents, routines and bots stay paused until the next restart."})) }
        if let Some(r) = self.rollback_to() { a.push(json!({"id": "rollback", "label": format!("Roll back to {}", Self::short(&r)), "text": format!("Marks {} bad.", Self::short(&self.build)), "remote": self.has_control(&r)})) }
        Value::Array(a)
    }
    fn text(&self) -> String {
        let b = Self::short(&self.build);
        match (self.state, self.mode) {
            ("rescue", _) => format!("Hub is down: {}", self.why),
            ("restarting", _) => format!("Restarting: {}", self.why),
            ("starting", _) => format!("Starting {b}"),
            (_, Mode::Safe) => format!("Safe mode on {b}: agents, routines and bots are paused. {}", self.why),
            _ => format!("Running {b}"),
        }
    }
    fn publish(&self) {
        let v = json!({"state": self.state, "mode": self.mode.name(), "build": self.build, "commit": fs::read_to_string(self.dir_of(&self.build).join("COMMIT")).unwrap_or_default().trim(),
            "since": self.since, "at": now(), "why": self.why, "text": self.text(), "port": self.o.port, "guardPid": std::process::id(), "hubPid": self.hub_pid, "stable": self.stable,
            "good": self.d.good(), "bad": self.bad(), "skip": self.skip, "low": self.low, "actions": self.actions(), "lastRequest": self.last_req});
        let _ = self.d.put("status.json", &v.to_string());
    }
    fn set(&mut self, state: &'static str) { if self.state != state { self.state = state; self.since = now(); } self.publish() }
    fn pin(&self) { if let Some(b) = &self.b { let mut ids = self.d.good(); ids.push(self.build.clone()); let from = ids.len().saturating_sub(3); b.pin(self.o.port, &ids[from..]); } }

    /// Checkpoints are only ever a faster way to the same state, and one written by a bad build is the likeliest thing to trip the next start: they go aside, never away.
    fn set_aside_checkpoints(&self) {
        let (c, q) = (self.o.home.join("ckpt"), self.o.home.join("quarantine"));
        if !c.exists() { return }
        let _ = fs::create_dir_all(&q);
        if fs::rename(&c, q.join(format!("ckpt-{}", now()))).is_ok() { say("checkpoints set aside; this start folds the log") }
        let mut old: Vec<PathBuf> = fs::read_dir(&q).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.file_name().map_or(false, |n| n.to_string_lossy().starts_with("ckpt-"))).collect()).unwrap_or_default();
        old.sort(); while old.len() > 2 { let _ = fs::remove_dir_all(old.remove(0)); }
    }

    /// The home's disk and the temp folder, looked at every half minute. Running low on either is told once, and once more when there is room again.
    fn temp_dir() -> PathBuf { std::env::temp_dir() }
    fn watch_space(&mut self) {
        if self.space_at.map_or(false, |t| t.elapsed().as_secs() < 30) { return }
        self.space_at = Some(Instant::now());
        for (what, p) in [("home", self.o.home.clone()), ("temp", Self::temp_dir())] {
            let Some((free, total)) = sys::space(&p) else { continue };
            let (low, was) = (plan::low_space(free, total, self.tm.floor), self.low.iter().any(|x| x == what));
            let left = format!("{}: {} free of {}", p.display(), plan::gb(free), plan::gb(total));
            if low && !was {
                self.low.push(what.into());
                if what == "temp" { self.alert("Temp folder nearly full", &left, "agents' temporary files will fail to write; the next hub start gets a temp folder on the home's disk. Free it, or `backplane hub restart`") }
                else { self.alert("Disk nearly full", &left, "the hub keeps its log whole and refuses writes it cannot finish; free space on this disk") }
            } else if !low && was { self.low.retain(|x| x != what); say(&format!("room again on {left}")); }
        }
    }
    /// A temp folder for the hub and every agent under it: the usual one, unless it is nearly full and the home's disk is not.
    fn hub_tmp(&self) -> Option<PathBuf> {
        let low = |p: &Path| sys::space(p).map_or(false, |(f, t)| plan::low_space(f, t, self.tm.floor));
        if !low(&Self::temp_dir()) || low(&self.o.home) { return None }
        let t = self.o.home.join("tmp"); fs::create_dir_all(&t).ok()?; Some(t)
    }

    /// Room to report a full disk in: a file made while there is space, given back when there is none.
    fn ballast(&self) {
        let p = self.d.path("ballast");
        match sys::free_bytes(&self.o.home) {
            Some(n) if n < 64 << 20 => { if fs::remove_file(&p).is_ok() { say("disk nearly full: gave back the reserve") } }
            Some(n) if n > 1 << 30 && !p.exists() => { let _ = fs::write(&p, vec![0u8; BALLAST]); }
            _ => {}
        }
    }
    fn open_log(&self) -> Stdio {
        let Some(l) = &self.o.log else { return Stdio::inherit() };
        if fs::metadata(l).map_or(false, |m| m.len() > 64 << 20) { let _ = fs::rename(l, l.with_extension("log.1")); }
        OpenOptions::new().create(true).append(true).open(l).map_or(Stdio::null(), Stdio::from)
    }

    /// A hub of this home still holding hub.lock while this guard holds the guard lock is an orphan: its guard was killed. The pid comes from the lock it holds.
    fn end_orphan(&self) {
        let lock = self.o.home.join("hub.lock");
        let Ok(f) = File::open(&lock) else { return };
        if f.try_lock().is_ok() { let _ = f.unlock(); return }
        let Some(pid) = fs::read_to_string(&lock).ok().and_then(|s| s.split_whitespace().next().and_then(|p| p.parse::<u32>().ok())) else { return };
        say(&format!("a hub (pid {pid}) from before this guard still holds {}: ending it", self.o.home.display()));
        sys::term(pid); for _ in 0..100 { if !sys::alive(pid) { return } std::thread::sleep(Duration::from_millis(100)); }
        sys::abort(pid); std::thread::sleep(Duration::from_millis(500));
    }

    fn port_free(&self) -> bool { std::net::TcpListener::bind(self.addr()).is_ok() }

    fn spawn(&mut self) -> Result<Child, End> {
        let dir = self.dir_of(&self.build); let exe = sys::hub_exe(&dir);
        if !exe.exists() { return Err(End::Failed(format!("no hub in {}", dir.display()))) }
        self.end_orphan(); self.ballast();
        if !self.port_free() { return Err(End::Machine(format!("port {} is held by another program", self.o.port))) }
        let _ = fs::remove_file(self.d.path("phase"));
        let mut c = Command::new(&exe);
        c.args(["--home", &self.o.home.to_string_lossy(), "--port", &self.o.port.to_string(), "--host", &self.o.host]).args(&self.o.hub_args)
            .env_remove("BACKPLANE_AGENT_OWNER").env("BACKPLANE_GUARD", "1").env("BACKPLANE_BUILD", &self.build).stdin(Stdio::null()).stdout(self.open_log()).stderr(self.open_log());
        // a turn the crash cut should not start again on a hub that is about to crash: the hub waits for this many seconds of life before it carries on
        c.env("BACKPLANE_CARRY_WAIT", if self.hist.iter().any(|f| now().saturating_sub(f.at) < self.t.window) { self.tm.stable.to_string() } else { "0".into() });
        if self.mode == Mode::Safe { c.env("BACKPLANE_SAFE", "1").env("BACKPLANE_SAFE_SKIP", self.skip.join(",")); } else { c.env_remove("BACKPLANE_SAFE").env_remove("BACKPLANE_SAFE_SKIP"); }
        if let Some(b) = &self.b { c.env("BACKPLANE_BUILDS", &b.0); }
        if let Some(t) = self.hub_tmp() { say(&format!("the temp folder is nearly full: this hub and its agents use {}", t.display())); c.env("TMPDIR", &t).env("TMP", &t).env("TEMP", &t); }
        c.env_remove("BACKPLANE_COMMIT"); // the hub says the commit it was compiled from: one set here would make the update check below prove nothing
        say(&format!("starting {} ({} mode, pid to follow)", Self::short(&self.build), self.mode.name()));
        c.spawn().map_err(|e| End::Failed(format!("{} did not start: {e}", exe.display())))
    }

    fn classify(&self, s: std::process::ExitStatus) -> End {
        let tail = || { let p = fs::read_to_string(self.d.path("panic.txt")).unwrap_or_default(); let ph = self.d.text("phase"); let ph = ph.trim(); let line = p.lines().next().unwrap_or("").chars().take(200).collect::<String>(); (line, ph.to_string()) };
        if let Some(sig) = sys::signal_of(&s) { let (p, ph) = tail(); let at = if ph.is_empty() { String::new() } else { format!(" during {ph}") }; return End::Failed(if p.is_empty() { format!("killed by signal {sig}{at}{}", if sig == 9 { " (out of memory, or killed)" } else { "" }) } else { p }) }
        match s.code().unwrap_or(1) {
            0 | 129 | 130 | 143 => End::External,
            2 => End::Refused(self.last_line("exit 2: the hub refused its arguments")),
            MACHINE => End::Machine(self.last_line("exit 75")),
            REFUSED => End::Refused(self.last_line("exit 78")),
            n => { let (p, ph) = tail(); End::Failed(if !p.is_empty() { p } else { let l = self.last_line(""); format!("exit {n}{}{}", if ph.is_empty() { String::new() } else { format!(" during {ph}") }, if l.is_empty() { String::new() } else { format!(": {l}") }) }) }
        }
    }
    /// The hub's last words: the last line of its log that is not start-up timing.
    fn last_line(&self, or: &str) -> String {
        let Some(l) = &self.o.log else { return or.into() };
        let t = fs::read(l).map(|b| String::from_utf8_lossy(&b[b.len().saturating_sub(4096)..]).into_owned()).unwrap_or_default();
        t.lines().rev().find(|x| !x.is_empty() && !x.starts_with("startup:") && !x.starts_with("guard:")).map_or(or.to_string(), |x| x.chars().take(200).collect())
    }

    fn answer(&mut self, id: &str, ok: bool, text: &str) { self.last_req = json!({"id": id, "ok": ok, "text": text, "at": now()}); self.publish(); say(text); }

    /// A request from the CLI (`from`: "local") or a client through the hub or rescue ("remote"). The answer is on status.json before anything is stopped, so the asker hears it.
    fn request(&mut self, r: Value) -> Move {
        let (id, from) = (r["id"].as_str().unwrap_or("").to_string(), r["from"].as_str().unwrap_or("remote").to_string());
        let Some(a) = Action::parse(r["action"].as_str().unwrap_or(""), r["build"].as_str().unwrap_or("")) else { self.answer(&id, false, "no such action"); return Move::Nothing };
        let target = match &a { Action::Rollback => self.rollback_to(), Action::Use(b) | Action::Update(b) => Some(b.clone()), _ => None };
        if from != "local" {
            let c = target.as_ref().map(|b| Candidate { build: b.clone(), control: self.has_control(b) });
            if let Some(why) = plan::refuse_remote(&a, &self.remote, now(), c.as_ref()) { self.answer(&id, false, &why); return Move::Nothing }
            self.remote.push(now()); self.remote.retain(|t| now().saturating_sub(*t) < 600);
        }
        if let Action::Use(b) | Action::Update(b) = &a { if !self.exists(b) { self.answer(&id, false, &format!("no build {b}")); return Move::Nothing } }
        if matches!(a, Action::Rollback) && target.is_none() { self.answer(&id, false, "no build to go back to"); return Move::Nothing }
        match a {
            Action::Stop => { self.answer(&id, true, "stopping"); return Move::Exit }
            Action::Restart => { self.mode = Mode::Normal; self.skip.clear(); self.answer(&id, true, "restarting") }
            Action::Safe => { self.mode = Mode::Safe; self.answer(&id, true, "restarting in safe mode") }
            Action::Rollback => {
                let to = target.unwrap_or_default();
                if let Some(b) = &self.b { b.mark_bad(&self.build, &format!("rolled back by {from}")); }
                self.alert("Rolled back", &format!("asked from {from}"), &format!("{} marked bad, now on {}", Self::short(&self.build), Self::short(&to)));
                self.set_aside_checkpoints(); self.build = to; self.mode = Mode::Normal; self.answer(&id, true, "rolling back");
            }
            Action::Use(b) => { if let Some(x) = &self.b { x.clear_bad(&b); } self.set_aside_checkpoints(); self.probation = None; self.build = b; self.mode = Mode::Normal; self.answer(&id, true, "switching build") }
            Action::Update(b) => {
                if self.bad().contains(&b) { self.answer(&id, false, &format!("{b} is marked bad")); return Move::Nothing }
                self.probation = Some(self.build.clone()); self.down_before = self.down.clone(); self.set_aside_checkpoints(); self.build = b; self.mode = Mode::Normal; self.answer(&id, true, "updating");
            }
        }
        self.hist.clear(); self.why.clear();
        std::thread::sleep(Duration::from_millis(600)); // the hub's route is reading the answer
        Move::Restart
    }

    /// (answered, says it is well, what it said, it has the control route). A hub still folding history after it began to serve is well, and not yet proven: see `one_run`.
    fn probe(&self) -> (bool, bool, String, bool) {
        let t = Duration::from_secs(5); let a = self.addr();
        let token = fs::read_to_string(self.o.home.join("token")).unwrap_or_default(); // so the hub says which build it is
        match http::call(&a, "GET", "/healthz", token.trim(), "", t) {
            Some((200, b)) => { let v: Value = serde_json::from_str(&b).unwrap_or(Value::Null); (true, true, b, v["control"] == 1) }
            Some((503, b)) => { let v: Value = serde_json::from_str(&b).unwrap_or(Value::Null); (v["status"] == "starting", false, b, false) }
            Some((404, _)) => match http::call(&a, "GET", "/hello", "", "", t) { Some((200, b)) => (true, true, b, false), _ => (false, false, String::new(), false) }, // a build from before /healthz
            Some((c, b)) => (false, false, format!("{c} {b}"), false),
            None => (false, false, "no answer".into(), false),
        }
    }

    /// Start the hub and stay with it until it ends.
    fn one_run(&mut self) -> End {
        self.stable = false; self.hub_pid = 0; self.set("starting");
        let mut child = match self.spawn() { Ok(c) => c, Err(e) => return e };
        self.hub_pid = child.id(); self.pin(); self.publish();
        let (t0, mut ready_at, mut misses, mut last_probe, mut last_said) = (Instant::now(), None::<Instant>, 0u32, Instant::now(), String::new());
        loop {
            if sys::stop_asked() { self.quit(Some(&mut child)) }
            match child.try_wait() {
                Ok(Some(s)) => { self.hub_pid = 0; return self.classify(s) }
                Ok(None) => {}
                Err(e) => say(&format!("cannot wait for the hub: {e}")),
            }
            if let Some(r) = self.d.take_request() {
                match self.request(r) {
                    Move::Nothing => {}
                    Move::Restart => { sys::stop_child(&mut child, 10_000); self.hub_pid = 0; return End::Asked }
                    Move::Exit => self.quit(Some(&mut child)),
                }
            }
            if last_probe.elapsed().as_secs() >= self.tm.probe {
                last_probe = Instant::now(); self.watch_space();
                let (answered, well, said, control) = self.probe();
                if well {
                    let down: Vec<String> = serde_json::from_str::<Value>(&said).ok().and_then(|v| v["degraded"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())).unwrap_or_default();
                    // on probation, a part the build before had running and this one has not is the update failing: a start step panicked or ran out of time
                    let lost = if self.probation.is_some() { plan::lost_parts(&self.down_before, &down) } else { vec![] };
                    if !lost.is_empty() {
                        let why = format!("came up without {} (a start step failed; the build before had it)", lost.join(", "));
                        say(&format!("{}: {why}; ending it", Self::short(&self.build))); sys::stop_child(&mut child, 10_000); self.hub_pid = 0; return End::Failed(why)
                    }
                    self.down = down;
                }
                match ready_at {
                    None if well && self.probation.is_some() && self.wrong_build(&said).is_some() => {
                        let why = self.wrong_build(&said).unwrap_or_default(); say(&format!("{} is not the build it was staged as: {why}; ending it", Self::short(&self.build)));
                        sys::stop_child(&mut child, 10_000); self.hub_pid = 0; return End::Failed(why)
                    }
                    None if well => { ready_at = Some(Instant::now()); say(&format!("{} answers, {} ms after its start", Self::short(&self.build), t0.elapsed().as_millis())); self.set(if self.mode == Mode::Safe { "safe" } else { "running" }); if control && !self.has_control(&self.build) { let mut c = self.d.lines("control"); c.push(self.build.clone()); let from = c.len().saturating_sub(32); let _ = self.d.put("control", &(c[from..].join("\n") + "\n")); } }
                    None => {
                        let quiet = fs::metadata(self.d.path("phase")).ok().and_then(|m| m.modified().ok()).and_then(|m| m.elapsed().ok()).map_or(t0.elapsed().as_secs(), |e| e.as_secs());
                        if quiet >= self.tm.stall || t0.elapsed().as_secs() >= self.tm.boot { let ph = self.d.text("phase"); return self.kill_hung(&mut child, &format!("start stalled for {quiet} s at {}", if ph.trim().is_empty() { "its first step" } else { ph.trim() })) }
                    }
                    Some(at) => {
                        if well || answered { misses = 0 } else { misses += 1; last_said = said.clone(); say(&format!("health check {misses} of {} failed: {}", self.tm.misses, said.chars().take(160).collect::<String>())) }
                        if misses >= self.tm.misses { let why = format!("stopped answering: {}", last_said.chars().take(160).collect::<String>()); return self.kill_hung(&mut child, &why) }
                        // the clock that proves a build starts when its folds and any migration are over: a crash at the end of a long fold or import is the
                        // start failing, while a long import itself is not (plan::holds_proof)
                        let busy = plan::holds_proof(&serde_json::from_str::<Value>(&said).unwrap_or(Value::Null));
                        if busy { ready_at = Some(Instant::now()); }
                        if !busy && !self.stable && at.elapsed().as_secs() >= self.tm.stable { self.stable = true; self.d.add_good(&self.build); self.pin(); say(&format!("{} started cleanly", Self::short(&self.build))); if self.probation.take().is_some() { if let Some(b) = &self.b { b.publish(&self.build); } } self.publish(); }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    /// On probation: `builds/<id>/EXPECT` ({"version","commit"}) against what the hub says on /healthz. No EXPECT, no check.
    fn wrong_build(&self, said: &str) -> Option<String> {
        let e: Value = serde_json::from_str(&fs::read_to_string(self.dir_of(&self.build).join("EXPECT")).ok()?).ok()?;
        plan::identity_mismatch(&e, &serde_json::from_str(said).unwrap_or(Value::Null))
    }
    fn kill_hung(&mut self, c: &mut Child, why: &str) -> End {
        say(&format!("hub (pid {}) is hung: {why}; ending it", c.id()));
        sys::abort(c.id()); for _ in 0..30 { if let Ok(Some(_)) = c.try_wait() { self.hub_pid = 0; return End::Failed(why.into()) } std::thread::sleep(Duration::from_millis(100)); }
        let _ = c.kill(); let _ = c.wait(); self.hub_pid = 0; End::Failed(why.into())
    }

    fn quit(&mut self, c: Option<&mut Child>) -> ! {
        say("stopping"); if let Some(c) = c { sys::stop_child(c, 10_000); }
        self.hub_pid = 0; self.set("stopped"); if let Some(b) = &self.b { b.unpin(self.o.port); }
        if let Some(p) = &self.o.pidfile { let _ = fs::remove_file(p); }
        std::process::exit(0)
    }

    /// Wait, listening for requests. `true`: a request changed the plan.
    fn wait(&mut self, secs: u64) -> bool {
        let t = Instant::now();
        while t.elapsed().as_secs() < secs {
            if sys::stop_asked() { self.quit(None) }
            if let Some(r) = self.d.take_request() { match self.request(r) { Move::Restart => return true, Move::Exit => self.quit(None), Move::Nothing => {} } }
            std::thread::sleep(Duration::from_millis(200));
        }
        false
    }

    /// Nothing starts. The guard answers on the hub's own addresses, so a phone or another machine that knew the hub still reaches something that can restart it.
    fn rescue(&mut self, after: u64, why: &str) {
        self.why = why.to_string(); self.set("rescue");
        let mut addrs = vec![self.addr()]; addrs.extend(self.d.lines("listeners").into_iter().filter(|a| *a != self.addr()));
        let server = Server::start(&addrs, rescue_handler(self.o.home.clone(), self.d.clone()));
        for f in &server.failed { say(&format!("rescue cannot listen on {f}")) }
        say(&format!("rescue listening on {}; trying again in {after} s", server.bound.join(", ")));
        let _ = self.wait(after); drop(server);
    }

    fn fold_failure(&mut self, end: &End) {
        match end {
            End::Failed(_) => self.hist.push(Fail { at: now(), build: self.build.clone(), mode: self.mode, machine: false }),
            End::Machine(_) => self.hist.push(Fail { at: now(), build: self.build.clone(), mode: self.mode, machine: true }),
            _ => {}
        }
        let w = self.t.window; self.hist.retain(|f| now().saturating_sub(f.at) < w);
    }

    fn step_that_failed(&self) -> Option<String> { self.d.text("phase").trim().strip_prefix("step:").map(str::to_string) }

    fn go(mut self) -> ! {
        loop {
            let end = self.one_run();
            if sys::stop_asked() { self.quit(None) }
            self.fold_failure(&end);
            let (good, bad) = (self.d.good(), self.bad());
            let b = self.build.clone();
            let next = { let me = &self; plan::next_on(&me.t, &me.hist, &end, &b, me.mode, &good, &bad, &|x| me.exists(x), now(), me.probation.as_deref()) };
            let why = match &end { End::Failed(w) | End::Machine(w) | End::Refused(w) => w.clone(), _ => String::new() };
            if !why.is_empty() { self.why = why.clone(); }
            match next {
                Next::Again { after } => {
                    if !why.is_empty() { say(&format!("hub ended: {why}; trying again in {after} s")); self.set("restarting"); if self.wait(after) { continue } }
                }
                Next::Fallback { from, to } => {
                    if let Some(x) = &self.b { x.mark_bad(&from, &why); }
                    self.alert(&format!("Build {} would not run", Self::short(&from)), &why, &format!("marked it bad, back on {}", Self::short(&to)));
                    self.set_aside_checkpoints(); self.probation = None; self.build = to;
                }
                Next::Safe { build, .. } => {
                    if let Some(s) = self.step_that_failed() { if !self.skip.contains(&s) { self.skip.push(s); } }
                    self.alert("Hub would not start", &why, &format!("safe mode on {}: agents, routines and bots paused{}", Self::short(&build), if self.skip.is_empty() { String::new() } else { format!(", skipping {}", self.skip.join(", ")) }));
                    self.set_aside_checkpoints(); self.build = build; self.mode = Mode::Safe;
                }
                Next::Rescue { after, why } => {
                    if self.state != "rescue" { self.alert("Hub is down", &why, "answering on its port for a restart, and trying again by itself"); }
                    self.rescue(after, &why);
                }
            }
        }
    }
}

/// What rescue says. Same routes and same credentials as the hub: the hub token, a paired device, or a signature from one of the owner's machines.
pub fn rescue_handler(home: PathBuf, d: Dir) -> Handler {
    let seen: Arc<Mutex<Vec<(String, u64)>>> = Default::default();
    Arc::new(move |r: &Req| {
        let st = d.json("status.json");
        match (r.method.as_str(), r.path.as_str()) {
            ("GET", "/healthz") => (503, json!({"status": "down", "rescue": 1, "control": 1, "why": st["why"]}).to_string()),
            ("GET", "/hub/status") if allowed(&home, &d, r, &seen) => (200, json!({"guard": st, "incidents": d.incidents(5), "rescue": 1}).to_string()),
            ("POST", "/hub/control") if allowed(&home, &d, r, &seen) => {
                let b: Value = serde_json::from_str(&r.body).unwrap_or(Value::Null);
                let id = format!("r{}-{}", now(), std::process::id());
                match d.ask(b["action"].as_str().unwrap_or(""), b["build"].as_str().unwrap_or(""), "remote", &id) {
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return (409, json!({"ok": false, "text": e.to_string()}).to_string()),
                    Err(_) => return (503, json!({"ok": false, "text": "the guard cannot be reached"}).to_string()),
                    Ok(()) => {}
                }
                for _ in 0..30 {
                    let lr = d.json("status.json")["lastRequest"].clone();
                    if lr["id"] == id.as_str() { return (if lr["ok"] == true { 202 } else { 409 }, json!({"ok": lr["ok"], "text": lr["text"]}).to_string()) }
                    std::thread::sleep(Duration::from_millis(100));
                }
                (202, json!({"ok": true, "text": "asked"}).to_string())
            }
            ("GET", "/hub/status") | ("POST", "/hub/control") => (401, json!({"ok": false, "text": "this Backplane needs its pairing token"}).to_string()),
            _ => (503, json!({"status": "down", "rescue": 1, "text": st["text"]}).to_string()),
        }
    })
}

fn allowed(home: &Path, d: &Dir, r: &Req, seen: &Mutex<Vec<(String, u64)>>) -> bool {
    let c = r.credential();
    if !c.is_empty() {
        let root = fs::read_to_string(home.join("token")).unwrap_or_default();
        if !root.trim().is_empty() && http::same(root.trim(), &c) { return true }
        let v: Value = fs::read_to_string(home.join("devices-v1.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
        let h = http::sha(&c);
        if v["devices"].as_array().into_iter().flatten().any(|x| x["hash"].as_str().map_or(false, |k| http::same(k, &h)) && !truthy(&x["revoked"])) { return true }
    }
    let (peer, ts, sig) = (r.header("x-backplane-peer"), r.header("x-backplane-timestamp"), r.header("x-backplane-signature"));
    if peer.is_empty() || sig.is_empty() { return false }
    let Ok(t) = ts.parse::<u64>() else { return false };
    if now().abs_diff(t) > 300 { return false }
    // the owner's own machines, as the hub last saw them; the secret stays in the hub's own file for the link
    if !d.json("peers.json")["peers"].as_array().into_iter().flatten().any(|p| p == peer) || peer.contains(['/', '\\', '.']) { return false }
    let kept: Value = fs::read_to_string(home.join("secrets").join("peers").join(peer)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    let Some(secret) = kept["secret"].as_str().filter(|s| !s.is_empty()) else { return false };
    let key = unhex(secret).unwrap_or_else(|| secret.as_bytes().to_vec());
    if !http::same(sig, &format!("sha256={}", http::hmac(&key, format!("{t}.{}", r.body).as_bytes()))) { return false }
    let mut s = seen.lock().unwrap_or_else(|e| e.into_inner()); s.retain(|(_, at)| now().saturating_sub(*at) < 600);
    if s.iter().any(|(k, _)| k == sig) { return false } // a signature works once
    s.push((sig.to_string(), now())); true
}
fn unhex(s: &str) -> Option<Vec<u8>> { if s.len() % 2 != 0 { return None } (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect() }
/// The hub's rule for `revoked` (auth.rs): anything but false, null, 0 and "".
fn truthy(v: &Value) -> bool { match v { Value::Null => false, Value::Bool(b) => *b, Value::Number(n) => n.as_f64().map_or(true, |f| f != 0.0), Value::String(s) => !s.is_empty(), _ => true } }

pub fn run(o: Opts) -> i32 {
    let d = Dir::of(&o.home); d.make();
    let lock = match OpenOptions::new().create(true).write(true).truncate(false).open(d.path("lock")) { Ok(f) => f, Err(e) => { say(&format!("cannot use {}: {e}", d.0.display())); return 1 } };
    if lock.try_lock().is_err() { say(&format!("a guard already runs for {}", o.home.display())); return 3 }
    sys::watch_stop();
    if let Some(p) = &o.pidfile { let _ = fs::write(p, format!("{}\n", std::process::id())); }
    if o.default_home { if let Some(p) = crate::store::pointer() { if let Some(dir) = p.parent() { let _ = fs::create_dir_all(dir); } let _ = fs::write(p, o.home.to_string_lossy().as_bytes()); } }
    let _ = d.put("launch.json", &json!({"home": o.home, "port": o.port, "host": o.host, "builds": o.builds, "log": o.log, "pidfile": o.pidfile, "mem": o.mem, "default": o.default_home, "hubArgs": o.hub_args,
        "env": std::env::vars().filter(|(k, _)| k.starts_with("BACKPLANE_GUARD_") || ["BACKPLANE_HTTPS_PORT", "BACKPLANE_NO_UPDATE", "BACKPLANE_AGENTS", "BACKPLANE_COPY", "BACKPLANE_SIDE_BY_SIDE"].contains(&k.as_str())).map(|(k, v)| (k, Value::String(v))).collect::<serde_json::Map<_, _>>()}).to_string());
    let b = o.builds.clone().map(Builds);
    let mut build = o.build.clone();
    if build.is_empty() { build = b.as_ref().map_or(String::new(), Builds::current); }
    let bad = b.as_ref().map_or(vec![], Builds::bad);
    let t = Tuning { tries: env("BACKPLANE_GUARD_TRIES", 3) as usize, window: env("BACKPLANE_GUARD_WINDOW", 600), pause: env("BACKPLANE_GUARD_PAUSE", 1), machine_pause: env("BACKPLANE_GUARD_MACHINE_PAUSE", 5), rescue_retry: env("BACKPLANE_GUARD_RESCUE_RETRY", 120) };
    let tm = Times { probe: env("BACKPLANE_GUARD_PROBE", 5), misses: env("BACKPLANE_GUARD_MISSES", 6) as u32, stable: env("BACKPLANE_GUARD_STABLE", 60), stall: env("BACKPLANE_GUARD_STALL", 300), boot: env("BACKPLANE_GUARD_BOOT", 900), floor: env("BACKPLANE_GUARD_MIN_FREE", 1 << 30) };
    let mode = if o.safe { Mode::Safe } else { Mode::Normal };
    let mut g = Guard { d, b, t, tm, build, mode, skip: vec![], hist: vec![], remote: vec![], why: String::new(), hub_pid: 0, state: "starting", stable: false, last_req: Value::Null, since: now(), probation: None, low: vec![], space_at: None, down: vec![], down_before: vec![], o };
    // a build already marked bad is not started: the newest one that is not
    if bad.contains(&g.build) { if let Some(to) = g.rollback_to() { say(&format!("{} is marked bad: starting {to}", g.build)); g.build = to; } }
    g.pin(); g.go()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn req(path: &str, method: &str, headers: Vec<(&str, String)>, body: &str) -> Req { Req { method: method.into(), path: path.into(), query: String::new(), headers: headers.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), body: body.into() } }
    fn home(n: &str) -> (PathBuf, Dir) {
        let h = std::env::temp_dir().join(format!("bp-rescue-{n}-{}", std::process::id())); let _ = fs::remove_dir_all(&h); fs::create_dir_all(&h).unwrap();
        fs::write(h.join("token"), "roottoken\n").unwrap();
        fs::write(h.join("devices-v1.json"), json!({"version":1,"devices":[{"id":"a","hash":http::sha("0123456789abcdef0123456789abcdef")},{"id":"b","hash":http::sha("ffffffffffffffffffffffffffffffff"),"revoked":true}]}).to_string()).unwrap();
        let d = Dir::of(&h); d.make(); (h, d)
    }
    fn bearer(t: &str) -> Vec<(&'static str, String)> { vec![("Authorization", format!("Bearer {t}"))] }
    #[test] fn rescue_takes_the_hub_token_and_a_live_device_and_nothing_else() {
        let (h, d) = home("auth"); let f = rescue_handler(h, d);
        assert_eq!(f(&req("/hub/status", "GET", bearer("roottoken"), "")).0, 200);
        assert_eq!(f(&req("/hub/status", "GET", bearer("0123456789abcdef0123456789abcdef"), "")).0, 200, "a paired phone");
        assert_eq!(f(&req("/hub/status", "GET", bearer("ffffffffffffffffffffffffffffffff"), "")).0, 401, "a revoked one");
        assert_eq!(f(&req("/hub/status", "GET", bearer("nope"), "")).0, 401); assert_eq!(f(&req("/hub/status", "GET", vec![], "")).0, 401);
        assert_eq!(f(&req("/hub/control", "POST", vec![], "{\"action\":\"restart\"}")).0, 401, "no credential, no restart");
        assert!(!Dir::of(&std::env::temp_dir().join(format!("bp-rescue-auth-{}", std::process::id()))).path("request").exists(), "and nothing was asked");
    }
    #[test] fn health_answers_with_no_credential_and_says_down() { let (h, d) = home("health"); let (c, b) = rescue_handler(h, d)(&req("/healthz", "GET", vec![], "")); assert_eq!(c, 503); assert!(b.contains("\"down\"") && b.contains("\"control\":1")); }
    #[test] fn a_machine_signs_in_with_its_link_and_a_signature_works_once() {
        let (h, d) = home("peer"); d.put("peers.json", r#"{"peers":["p1"]}"#).unwrap();
        fs::create_dir_all(h.join("secrets/peers")).unwrap(); fs::write(h.join("secrets/peers/p1"), r#"{"secret":"abcd"}"#).unwrap();
        let ts = now(); let body = "{\"action\":\"nope\"}"; let sig = format!("sha256={}", http::hmac(&[0xab, 0xcd], format!("{ts}.{body}").as_bytes()));
        let hs = |s: String, t: u64| vec![("X-Backplane-Peer", "p1".to_string()), ("X-Backplane-Timestamp", t.to_string()), ("X-Backplane-Signature", s)];
        let seen = Mutex::new(vec![]);
        assert!(!allowed(&h, &d, &req("/hub/control", "POST", hs("sha256=00".into(), ts), body), &seen), "a wrong signature");
        assert!(!allowed(&h, &d, &req("/hub/control", "POST", hs(sig.clone(), ts - 1000), body), &seen), "a stale time");
        assert!(allowed(&h, &d, &req("/hub/control", "POST", hs(sig.clone(), ts), body), &seen));
        assert!(!allowed(&h, &d, &req("/hub/control", "POST", hs(sig, ts), body), &seen), "a replay");
        d.put("peers.json", r#"{"peers":[]}"#).unwrap(); let ts2 = now() + 1; let sig2 = format!("sha256={}", http::hmac(&[0xab, 0xcd], format!("{ts2}.{body}").as_bytes()));
        assert!(!allowed(&h, &d, &req("/hub/control", "POST", hs(sig2, ts2), body), &seen), "a link that is no longer the owner's");
    }
}

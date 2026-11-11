//! The hub's side of being looked after (`native/guard`, the `backplane` program; docs/HUB-RESILIENCE.md). The guard is another process: it reads what this module writes to
//! `<home>/guard/` and asks `/healthz`. Everything here also works with no guard: the start is as tolerant, and only restarting needs one.
//!   * `phase`: where the start has got to, so a hang can be told from a slow fold and the step that hung can be skipped
//!   * `step`: a part of the start that may panic or hang without taking the hub with it
//!   * `watch`: the hub's inner locks and tasks, checked from outside them; `/healthz` says which one stopped
//!   * safe mode (`BACKPLANE_SAFE=1`): the hub serves, nothing acts by itself
use crate::{api::{Fut, HttpHandler, HttpReply}, auth::Req, hub::Hub};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::{atomic::{AtomicBool, AtomicU64, Ordering::Relaxed}, Arc, Mutex, OnceLock}, time::Duration};

/// Exit codes for what is not this build's fault (sysexits.h). The guard reads them; a build is never blamed for these.
pub const EXIT_MACHINE: i32 = 75;
pub const EXIT_REFUSED: i32 = 78;
/// How long a step of the start may take before the hub goes on without it.
pub fn step_budget() -> u64 { std::env::var("BACKPLANE_STEP_BUDGET").ok().and_then(|v| v.parse().ok()).unwrap_or(120) }
/// The least a start has to hold before the uberbot is told of it.
pub const WAKE_HOLD_S: u64 = 30;
/// How often the watched parts are checked, how long each may take, and how old the last good check may be before the hub calls itself hung.
const WATCH_EVERY_S: u64 = 10;
const WATCH_TIMEOUT_S: u64 = 30;
const WATCH_STALE_S: u64 = 90;

static HOME: OnceLock<PathBuf> = OnceLock::new();
static READY: AtomicBool = AtomicBool::new(false);
static DEGRADED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
static WATCHED: Mutex<Vec<(&'static str, Arc<AtomicU64>)>> = Mutex::new(Vec::new());

pub fn supervised() -> bool { static ON: OnceLock<bool> = OnceLock::new(); *ON.get_or_init(|| std::env::var("BACKPLANE_GUARD").map_or(false, |v| v == "1")) }
/// Safe mode: agents, routines, bots and the policies do nothing by themselves. Read once.
pub fn safe() -> bool { static ON: OnceLock<bool> = OnceLock::new(); *ON.get_or_init(|| std::env::var("BACKPLANE_SAFE").map_or(false, |v| v == "1")) }
fn skips() -> &'static Vec<String> { static S: OnceLock<Vec<String>> = OnceLock::new(); S.get_or_init(|| parse_skips(&std::env::var("BACKPLANE_SAFE_SKIP").unwrap_or_default())) }
pub fn parse_skips(v: &str) -> Vec<String> { v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect() }
/// What safe mode tells a client whose request would start an agent.
pub const SAFE_TOLD: &str = "the hub is in safe mode: agents are paused until it restarts normally";
/// Seconds a restarted hub waits before it carries on the turns a crash cut, when the guard says the last start failed: a build that dies at once must not start every agent each time.
pub fn carry_wait() -> u64 { std::env::var("BACKPLANE_CARRY_WAIT").ok().and_then(|v| v.parse().ok()).unwrap_or(0) }

fn dir() -> Option<PathBuf> { HOME.get().map(|h| h.join("guard")) }
fn now_s() -> u64 { crate::hub::now_ms() / 1000 }
/// Small files beside and renamed: the guard never reads half. A full disk loses the note, nothing else.
fn put(name: &str, text: &str) { if let Some(d) = dir() { let _ = crate::ckpt::write_atomic(&d.join(name), &[text.as_bytes()]); } }

/// Something the person should hear of, for the app and `backplane hub status`: the guard's own record, written by the hub.
pub fn incident(what: &str, why: &str, did: &str) {
    eprintln!("backplane-hub: {what}: {why}; {did}");
    if let Some(d) = dir() { use std::io::Write; if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("incidents.jsonl")) { let _ = writeln!(f, "{}", json!({"at": now_s(), "what": what, "why": why, "did": did})); } }
}
/// `BACKPLANE_FAULT=panic:<step>` or `hang:<step>`, for test/guard/faults.sh: a real hub, failing in a real place. Read at the start of a step only.
fn fault(name: &str) -> Option<&'static str> {
    let v = std::env::var("BACKPLANE_FAULT").ok()?;
    if v == format!("panic:{name}") { Some("panic") } else if v == format!("hang:{name}") { Some("hang") } else if v == format!("die:{name}") { Some("die") } else { None }
}
fn injected(name: &str) {
    match fault(name) {
        Some("panic") => panic!("BACKPLANE_FAULT: {name} panicked"),
        Some("hang") => { eprintln!("BACKPLANE_FAULT: {name} hangs"); loop { std::thread::sleep(Duration::from_secs(3600)); } }
        Some("die") => { eprintln!("BACKPLANE_FAULT: {name} takes the process down"); std::process::abort() }
        _ => {}
    }
}
/// First thing in a start: where to write, and the panic hook. A panic anywhere leaves its place in `guard/panic.txt` and the log, whichever thread it was on.
pub fn begin(home: &Path) {
    let _ = HOME.set(home.to_path_buf());
    let _ = crate::startup::ON_MARK.set(phase);
    let _ = crate::store::ON_QUARANTINE.set(|line| incident("A log record broke a fold", &format!("line {line}"), "quarantined it and went on; the log keeps it"));
    if safe() { let _ = crate::proc::PAUSED.set(format!("backplane: {SAFE_TOLD}")); }
    // a long fold writes no phase: while lines are being read, the phase file is kept fresh, so a slow start is not taken for a hung one
    std::thread::spawn(|| { let mut seen = 0; while !READY.load(Relaxed) {
        std::thread::sleep(Duration::from_secs(5));
        let n = crate::store::PROGRESS.load(Relaxed);
        if n != seen { seen = n; if let Some(d) = dir() { if let Ok(t) = std::fs::read_to_string(d.join("phase")) { put("phase", &t); } } }
    } });
    if let Some(d) = dir() { let _ = std::fs::create_dir_all(&d); let _ = std::fs::remove_file(d.join("panic.txt")); let _ = std::fs::remove_file(d.join("listeners")); }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |i| {
        let what = i.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| i.payload().downcast_ref::<String>().cloned()).unwrap_or_else(|| "panic".into());
        let at = i.location().map_or(String::new(), |l| format!(" at {}:{}", l.file(), l.line()));
        let line: String = format!("panic{at}: {what}").chars().take(300).collect();
        if !READY.load(Relaxed) { put("panic.txt", &(line.clone() + "\n")); }
        prev(i);
    }));
}
/// Called by `startup::mark`: the guard watches this file's age while the hub starts.
pub fn phase(what: &str) { put("phase", what); }
pub fn listening(addr: &str) { if let Some(d) = dir() { use std::io::Write; if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("listeners")) { let _ = writeln!(f, "{addr}"); } } }

pub fn degraded(what: &str, why: &str) {
    let why: String = why.chars().take(200).collect();
    eprintln!("backplane-hub: {what} is not running: {why}");
    let mut d = DEGRADED.lock().unwrap_or_else(|e| e.into_inner()); if !d.iter().any(|(w, _)| w == what) { d.push((what.to_string(), why)); }
}
pub fn degraded_list() -> Vec<(String, String)> { DEGRADED.lock().unwrap_or_else(|e| e.into_inner()).clone() }
pub fn skipped(name: &str) -> bool { safe() && skips().iter().any(|s| s == name) }

/// A part of the start as a task of its own: a panic or a hang costs that part, not the hub. None: it did not finish (the hub says so on /healthz).
pub async fn step<T: Send + 'static>(name: &'static str, budget_s: u64, fut: impl std::future::Future<Output = T> + Send + 'static) -> Option<T> {
    if skipped(name) { degraded(name, "skipped in safe mode: it stopped the last start"); return None }
    phase(&format!("step:{name}"));
    let h = tokio::spawn(async move { injected(name); fut.await });
    match tokio::time::timeout(Duration::from_secs(budget_s), h).await {
        Ok(Ok(v)) => Some(v),
        Ok(Err(e)) => { degraded(name, &if e.is_panic() { format!("it panicked: {}", panic_text(e)) } else { "it was cancelled".to_string() }); None }
        Err(_) => { degraded(name, &format!("no answer in {budget_s} s")); None }
    }
}
fn panic_text(e: tokio::task::JoinError) -> String { e.try_into_panic().ok().map_or("unknown".into(), |p| p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown".into())) }
/// `step` for a part that does not wait for anything.
pub fn step_now<T>(name: &'static str, f: impl FnOnce() -> T) -> Option<T> {
    if skipped(name) { degraded(name, "skipped in safe mode: it stopped the last start"); return None }
    phase(&format!("step:{name}"));
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { injected(name); f() })) { Ok(v) => Some(v), Err(_) => { degraded(name, "it panicked"); None } }
}

/// Keep asking `check`, from outside what it checks. `check` should do what a real request does: take the lock, go through the task.
pub fn watch(name: &'static str, check: impl Fn() -> Fut<'static, ()> + Send + Sync + 'static) {
    let last = Arc::new(AtomicU64::new(now_s()));
    WATCHED.lock().unwrap_or_else(|e| e.into_inner()).push((name, last.clone()));
    tokio::spawn(async move { loop {
        if tokio::time::timeout(Duration::from_secs(WATCH_TIMEOUT_S), check()).await.is_ok() { last.store(now_s(), Relaxed); } else { eprintln!("backplane-hub: {name} did not answer in {WATCH_TIMEOUT_S} s"); }
        tokio::time::sleep(Duration::from_secs(WATCH_EVERY_S)).await;
    } });
}
/// What stopped answering (none: the hub is well).
pub fn hung_at(rows: &[(&str, u64)], now: u64) -> Vec<String> { rows.iter().filter(|(_, at)| now.saturating_sub(*at) > WATCH_STALE_S).map(|(n, _)| n.to_string()).collect() }
fn hung() -> Vec<String> { let w = WATCHED.lock().unwrap_or_else(|e| e.into_inner()); hung_at(&w.iter().map(|(n, t)| (*n, t.load(Relaxed))).collect::<Vec<_>>(), now_s()) }

static FOLDING: AtomicU64 = AtomicU64::new(0);
/// A fold of history is still running after the hub began to serve. `/healthz` says so, and the guard does not begin counting a build as proven until it is over: a crash at the end of a long fold is the start failing.
pub struct Folding;
impl Folding { pub fn begin() -> Folding { FOLDING.fetch_add(1, Relaxed); Folding } }
impl Drop for Folding { fn drop(&mut self) { FOLDING.fetch_sub(1, Relaxed); } }

/// Whether the start is over (the hub serves).
pub fn is_ready() -> bool { READY.load(Relaxed) }
/// The start is over: from here the hub answers everything.
pub fn ready() { READY.store(true, Relaxed); phase("ready"); }

/// GET /healthz: for the guard and anyone who wants to know, so no credential and nothing private. 200: serving. 503: starting, or something inside stopped.
pub fn health() -> (&'static str, Value) {
    let (hung, degraded) = (hung(), degraded_list());
    let status = if !READY.load(Relaxed) { "starting" } else if !hung.is_empty() { "hung" } else if !degraded.is_empty() { "degraded" } else { "ok" };
    let code = if matches!(status, "ok" | "degraded") { "200 OK" } else { "503 Service Unavailable" };
    let phase = dir().and_then(|d| std::fs::read_to_string(d.join("phase")).ok()).unwrap_or_default();
    let mut v = json!({"status": status, "phase": phase.trim(), "safe": safe(), "control": 1, "guarded": supervised(), "folding": FOLDING.load(Relaxed), "hung": hung, "degraded": degraded.iter().map(|(w, _)| w).collect::<Vec<_>>()});
    // a 0.14 home coming across: well, and not yet proven (it holds a Folding too); the guard and the watchdog read it as progress, never as a hang
    if let Some(m) = crate::import014::migrating() { v["migrating"] = m; }
    (code, v)
}

/// Before the hub exists: the listener is bound first, and this answers it until the start is over. A client treats it as "not yet", as it would a refused connection.
pub async fn answer_early(mut s: tokio::net::TcpStream) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut b = [0u8; 2048];
    let n = tokio::time::timeout(Duration::from_secs(3), s.read(&mut b)).await.ok().and_then(Result::ok).unwrap_or(0);
    let head = String::from_utf8_lossy(&b[..n]);
    let (_, body) = health();
    let body = if head.starts_with("GET /healthz") { body } else { json!({"status": "starting", "phase": body["phase"]}) }.to_string();
    let out = format!("HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nRetry-After: 2\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    let _ = s.write_all(out.as_bytes()).await; let _ = s.shutdown().await;
}

/// `guard/status.json`, the last incidents and this hub's own view, as clients get them. The same shape rescue answers with.
pub fn status(home: &Path) -> Value {
    let g = home.join("guard");
    let st: Value = std::fs::read_to_string(g.join("status.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    let lines = std::fs::read_to_string(g.join("incidents.jsonl")).unwrap_or_default();
    let all: Vec<Value> = lines.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let (_, h) = health();
    json!({"guard": st, "incidents": all[all.len().saturating_sub(5)..], "hub": h, "guarded": supervised()})
}
/// Why the hub was last restarted, when something went wrong in the last hour: for the uberbot's turn.
pub fn last_why(home: &Path) -> String {
    let s = status(home); let last = open_incident(&s);
    if last["at"].as_u64().map_or(true, |at| now_s().saturating_sub(at) >= 3600) { return String::new() }
    format!("{}: {}", last["what"].as_str().unwrap_or(""), last["did"].as_str().unwrap_or(""))
}
/// The newest incident still open: a record that `clears` an alert (the guard's "Disk has room again") closes it, and is no news itself.
fn open_incident(s: &Value) -> Value {
    let mut cleared: Vec<&str> = vec![];
    for i in s["incidents"].as_array().map(|a| a.as_slice()).unwrap_or(&[]).iter().rev() {
        if let Some(c) = i["clears"].as_str() { cleared.push(c); continue }
        if cleared.contains(&i["what"].as_str().unwrap_or("")) { continue }
        return i.clone();
    }
    Value::Null
}
/// What a client shows about the hub, and when: `text` empty means all is well.
pub fn notice(s: &Value) -> Value {
    let safe = s["hub"]["safe"] == true; let degraded = s["hub"]["degraded"].as_array().map_or(0, Vec::len) > 0;
    let last = open_incident(s);
    let fresh = last["at"].as_u64().map_or(false, |at| now_s().saturating_sub(at) < 3600);
    let text = if safe { format!("Safe mode: agents, routines and bots are paused. {}", last["why"].as_str().unwrap_or("")) }
        else if degraded { format!("Hub is running without {}.", s["hub"]["degraded"].as_array().map_or(String::new(), |a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))) }
        else if fresh { format!("{}: {}", last["what"].as_str().unwrap_or(""), last["did"].as_str().unwrap_or("")) }
        else { String::new() };
    json!({"text": text.trim(), "tone": if safe || degraded { "warn" } else { "info" }, "actions": s["guard"]["actions"]})
}

/// `/hub/status` and `/hub/control`. Control writes the guard a request and waits for its answer; it needs nothing from the views, the presenter or the agents, so it works when they do not.
pub struct Routes;
fn reply(status: &'static str, v: Value) -> HttpReply { HttpReply { status, ctype: "application/json".into(), extra: "Cache-Control: no-store\r\n".into(), body: v.to_string().into_bytes() } }
impl HttpHandler for Routes {
    fn routes(&self) -> &'static [&'static str] { &["/hub/status", "/hub/control"] }
    fn own_auth(&self) -> bool { true }
    fn body_max(&self) -> usize { 16 << 10 }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, req: &'a Req) -> Fut<'a, HttpReply> {
        Box::pin(async move {
            let (person, machine) = (hub.auth.allowed(req), own_machine(req));
            if !person && !machine { return reply("401 Unauthorized", json!({"ok": false, "text": "this Backplane needs its pairing token"})) }
            if req.route() == "/hub/status" { let mut s = status(&hub.cfg.home); s["notice"] = notice(&s); return reply("200 OK", s) }
            if req.method != "POST" { return reply("405 Method Not Allowed", json!({"ok": false, "text": "POST"})) }
            let b: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let s = |k: &str| b[k].as_str().unwrap_or("").to_string();
            if !s("machine").is_empty() { return forward(hub, &s("machine"), &req.body).await }
            control(&hub.cfg.home, &s("action"), &s("build"), "remote").await
        })
    }
}

/// Hand the guard a request and bring back what it said. `from`: "remote" for a client or another machine (the guard's limits apply), "local" for the hub's own
/// updater, which stages a build that has never run here.
pub async fn control(home: &Path, action: &str, build: &str, from: &str) -> HttpReply {
    if !supervised() { return reply("409 Conflict", json!({"ok": false, "text": "this hub runs without a guard: restart it at its machine"})) }
    if !matches!(action, "restart" | "normal" | "safe" | "rollback" | "use" | "update") { return reply("409 Conflict", json!({"ok": false, "text": "no such action here"})) } // `stop` only at the machine
    let id = format!("h{}-{}", crate::hub::now_ms(), std::process::id());
    let g = home.join("guard");
    let body = json!({"id": id, "action": action, "build": build, "from": from, "at": now_s()}).to_string();
    match ask(&g, &id, &body) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return reply("409 Conflict", json!({"ok": false, "text": e.to_string()})),
        Err(_) => return reply("503 Service Unavailable", json!({"ok": false, "text": "cannot reach the guard: the disk may be full"})),
        Ok(()) => {}
    }
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let lr = std::fs::read_to_string(g.join("status.json")).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()).map_or(Value::Null, |v| v["lastRequest"].clone());
        if lr["id"] == id.as_str() { return reply(if lr["ok"] == true { "202 Accepted" } else { "409 Conflict" }, json!({"ok": lr["ok"], "text": lr["text"]})) }
    }
    reply("503 Service Unavailable", json!({"ok": false, "text": "the guard did not answer"}))
}

/// The guard's request file, one at a time (as `backplane hub` writes it, native/guard store.rs): one already waiting is never replaced, unless it is too old to be read.
pub fn ask(g: &Path, id: &str, body: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(g)?;
    let (r, t) = (g.join("request"), g.join(format!(".request.{id}")));
    std::fs::write(&t, body)?;
    let out = match std::fs::hard_link(&t, &r) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let stale = std::fs::metadata(&r).and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok()).map_or(true, |a| a.as_secs() >= 30);
            if stale { crate::plat::fs::rename(&t, &r) } else { Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "another request is waiting for the guard: try again in a moment")) }
        }
        o => o,
    };
    let _ = std::fs::remove_file(&t); out
}

/// A signed request from one of the owner's own linked machines.
fn own_machine(req: &Req) -> bool {
    let (peer, ts, sig) = (req.header("x-backplane-peer"), req.header("x-backplane-timestamp"), req.header("x-backplane-signature"));
    let (Some(p), Ok(ts)) = (crate::net::start::peering(), ts.parse::<u64>()) else { return false };
    if peer.is_empty() || !p.table.with(|t| t.peer(peer).map_or(false, |r| !r.revoked) && t.verified(peer)) { return false }
    crate::net::sign::check(&crate::net::botnet::peer_secret(&p.home, peer), now_s(), ts, &String::from_utf8_lossy(&req.body), sig)
}
/// Restart another of the owner's machines from this one. If its hub is down its guard answers on the same address, with the same signature.
async fn forward(hub: &Arc<Hub>, machine: &str, body: &[u8]) -> HttpReply {
    let Some(p) = crate::net::start::peering() else { return reply("409 Conflict", json!({"ok": false, "text": "no linked machines here"})) };
    let found = p.table.with(|t| t.live().into_iter().find(|x| (x.id == machine || x.name == machine) && t.verified(&x.id)).map(|x| (x.id, x.url)));
    let Some((link, url)) = found else { return reply("409 Conflict", json!({"ok": false, "text": "not one of your linked machines"})) };
    let mut b: Value = serde_json::from_slice(body).unwrap_or(Value::Null); if let Some(o) = b.as_object_mut() { o.remove("machine"); }
    let r = crate::net::botnet::to_peer(&hub.cfg.home, &url, &link, now_s(), "/hub/control", true, &b.to_string()).await;
    let text = serde_json::from_str::<Value>(&r.body).ok().and_then(|v| v["text"].as_str().map(str::to_string)).unwrap_or_else(|| format!("{machine} did not answer"));
    reply(if r.code == 202 || r.code == 200 { "202 Accepted" } else { "409 Conflict" }, json!({"ok": r.code == 202 || r.code == 200, "text": text}))
}

/// What the app and the phones are told once the hub is up, and the watch on the writer, the read model and the live pairing rescue will need if the hub goes down.
pub fn start(hub: &Arc<Hub>) {
    hub.register_http(Arc::new(Routes)); hub.register_rpc(Arc::new(Rpc));
    { let h = hub.clone(); watch("writer", move || { let h = h.clone(); Box::pin(async move { let _ = h.commit_many(&[]).await; }) }); }
    { let h = hub.clone(); watch("model", move || { let h = h.clone(); Box::pin(async move { drop(h.model.lock().await); }) }); }
    let h = hub.clone();
    tokio::spawn(async move { loop {
        let s = status(&h.cfg.home); let n = notice(&s);
        h.set_info("hub.notice", &if n["text"] == "" { String::new() } else { n.to_string() }).await;
        if let Some(p) = crate::net::start::peering() {
            let ids: Vec<String> = p.table.with(|t| t.live().into_iter().filter(|x| t.verified(&x.id)).map(|x| x.id).collect());
            put("peers.json", &json!({"peers": ids}).to_string());
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    } });
}

/// `hub.status` and `hub.control` on a socket: the same answers as the HTTP routes, for clients that hold only a socket. The socket is already signed in.
pub struct Rpc;
impl crate::api::RpcHandler for Rpc {
    fn methods(&self) -> &'static [&'static str] { &["hub.status", "hub.control"] }
    fn call<'a>(&'a self, hub: &'a Arc<Hub>, _who: &'a crate::api::Caller, id: &'a Value, method: &'a str, p: &'a Value) -> Fut<'a, Vec<Value>> {
        Box::pin(async move {
            if method == "hub.status" { let mut s = status(&hub.cfg.home); s["notice"] = notice(&s); return vec![json!({"t": "reply", "id": id, "ok": true, "result": s})] }
            let g = |k: &str| p[k].as_str().unwrap_or("").to_string();
            let r = if g("machine").is_empty() { control(&hub.cfg.home, &g("action"), &g("build"), "remote").await } else { forward(hub, &g("machine"), p.to_string().as_bytes()).await };
            let v: Value = serde_json::from_slice(&r.body).unwrap_or(Value::Null);
            vec![json!({"t": "reply", "id": id, "ok": v["ok"] == true, "result": v, "error": if v["ok"] == true { Value::Null } else { v["text"].clone() }})]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn a_waiting_request_is_never_replaced() {
        let g = crate::plat::tmp::uncreated("ask");
        ask(&g, "a", "{\"id\":\"a\"}").unwrap();
        let e = ask(&g, "b", "{\"id\":\"b\"}").unwrap_err(); assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(g.join("request")).unwrap(), "{\"id\":\"a\"}", "the first one still waits");
        std::fs::remove_file(g.join("request")).unwrap(); ask(&g, "c", "c").unwrap(); assert_eq!(std::fs::read_to_string(g.join("request")).unwrap(), "c", "taken: the next one goes in");
        assert_eq!(std::fs::read_dir(&g).unwrap().count(), 1, "no temp files left");
    }
    #[test] fn skips_parse() { assert_eq!(parse_skips(" bots, ,agents"), vec!["bots", "agents"]); assert!(parse_skips("").is_empty()); }
    #[test] fn a_stale_check_names_what_stopped() {
        assert!(hung_at(&[("writer", 1000), ("model", 950)], 1000).is_empty());
        assert_eq!(hung_at(&[("writer", 1000), ("agents", 800)], 1000), vec!["agents"]);
    }
    #[tokio::test] async fn a_step_that_panics_or_hangs_costs_only_itself() {
        assert_eq!(step("ok", 5, async { 7 }).await, Some(7));
        assert_eq!(step("boom", 5, async { if true { panic!("x") } 1 }).await, None);
        assert_eq!(step("slow", 0, async { tokio::time::sleep(Duration::from_secs(30)).await; 1 }).await, None);
        let d = degraded_list(); assert!(d.iter().any(|(w, y)| w == "boom" && y.contains("panicked")), "{d:?}"); assert!(d.iter().any(|(w, y)| w == "slow" && y.contains("no answer")));
        assert_eq!(step_now("ok2", || 1), Some(1)); assert_eq!(step_now("boom2", || -> i32 { panic!("y") }), None);
    }
    #[test] fn a_notice_says_nothing_when_all_is_well() {
        assert_eq!(notice(&json!({"hub": {"safe": false, "degraded": []}, "incidents": [], "guard": {}}))["text"], "");
        assert_eq!(notice(&json!({"hub": {"safe": false, "degraded": []}, "incidents": [{"at": 5, "what": "x", "did": "y"}], "guard": {}}))["text"], "", "an old incident is not news");
        let n = notice(&json!({"hub": {"safe": true, "degraded": []}, "incidents": [{"at": now_s(), "why": "a panic"}], "guard": {"actions": [{"id": "restart"}]}}));
        assert!(n["text"].as_str().unwrap().starts_with("Safe mode") && n["text"].as_str().unwrap().contains("a panic")); assert_eq!(n["actions"][0]["id"], "restart");
        assert!(notice(&json!({"hub": {"safe": false, "degraded": ["bots"]}, "incidents": []}))["text"].as_str().unwrap().contains("bots"));
        let full = json!({"at": now_s() - 60, "what": "Disk nearly full", "why": "7.1 GB free", "did": "free space on this disk"});
        let calm = json!({"hub": {"safe": false, "degraded": []}, "guard": {}});
        let with = |i: Vec<Value>| { let mut c = calm.clone(); c["incidents"] = json!(i); notice(&c)["text"].as_str().unwrap().to_string() };
        assert!(with(vec![full.clone()]).starts_with("Disk nearly full"), "a full disk is told");
        let room = json!({"at": now_s(), "what": "Disk has room again", "why": "201 GB free", "did": "", "clears": "Disk nearly full"});
        assert_eq!(with(vec![full.clone(), room.clone()]), "", "room again: the alert leaves the app and the phones, and the clear is no news itself");
        let crash = json!({"at": now_s() - 30, "what": "Hub crashed", "why": "x", "did": "restarted"});
        assert!(with(vec![crash.clone(), full.clone(), room.clone()]).starts_with("Hub crashed"), "an earlier incident it did not clear still shows");
        assert!(with(vec![full.clone(), room, full]).starts_with("Disk nearly full"), "full again: told again");
    }
}

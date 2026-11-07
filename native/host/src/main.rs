//! backplane-hub: the Backplane hub for native clients (GPUI, SwiftUI, Compose).
//! HTTP + authenticated WebSocket; renderer `mobile`; the picture comes from the presenter.
mod agents; mod api; mod auth; mod bots; mod cbor; mod commands; mod data; mod copy; mod delta; mod files; mod hub; mod md; mod model; mod net; mod present;
mod presenter; mod proc; mod session; mod settings; mod shelf; mod sidebar; mod signal; mod stamp; mod state; #[path = "state/host.rs"] mod statehost; mod ckpt; mod ckptrun; mod startup; mod store; mod term; mod update; mod voice; mod wire; mod window;

use anyhow::{Context, Result};
use auth::Req;
use futures_util::{SinkExt, StreamExt};
use hub::{Conn, Hub};
use wire::*;
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::{handshake::server::ErrorResponse, protocol::WebSocketConfig, Message};

const USAGE: &str = "usage: backplane-hub --home DIR --port N [--host ADDR] [--no-tailscale | --tailscale | --tailscale-https | --tailnet-only] [--foreground] [--headless]\n       backplane-hub --render MODEL...\n       backplane-hub --help | --version\n";

struct Args { window: Option<PathBuf>, home: PathBuf, port: u16, host: String, version: String, commit: String, tailscale: bool, https: bool }

/// (tailscale, https) from the flags: a hub on a copied log (BACKPLANE_COPY=1) runs neither (no `tailscale serve` on a copy), and
/// `--no-tailscale` turns https off with it.
fn tailnet_flags(no_tailscale: bool, https: bool, copy: bool) -> (bool, bool) { let on = !no_tailscale && !copy; (on, on && https) }

/// `--tailnet-only` (daily use from a phone): the hub listens on loopback and on this machine's Tailscale address, never on a public one, and gives phones
/// an HTTPS address from `tailscale serve`. It refuses a non-loopback `--host`, a copy of a log, and a missing `BACKPLANE_HTTPS_PORT` (the serve port must be chosen:
/// the default 443 may be another hub's). Err is the one line said before exit 2.
fn tailnet_only(host: &str, https_port: Option<&str>, copy: bool) -> std::result::Result<(), &'static str> {
    if copy { return Err("--tailnet-only is for a hub of your own, not a copy of a log") }
    if !matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]") { return Err("--tailnet-only: --host must be loopback; the tailnet address is bound separately, and nothing public is ever bound") }
    match https_port { Some(v) if net::listener::https_port_of(Some(v)).is_some() => Ok(()), _ => Err("--tailnet-only needs BACKPLANE_HTTPS_PORT, the port tailscale serve takes for HTTPS (a free one: 443 may be another hub's)") }
}

fn parse() -> std::result::Result<Args, i32> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (mut home, mut port, mut host, mut window) = (None, None, "127.0.0.1".to_string(), None::<PathBuf>);
    let (mut no_tailscale, mut https, mut only) = (false, false, false);
    let (mut version, mut commit) = (env!("CARGO_PKG_VERSION").to_string(), std::env::var("BACKPLANE_COMMIT").unwrap_or_else(|_| option_env!("BACKPLANE_COMMIT").unwrap_or("unknown").into())); // a launch can name it; a build stamps it (BACKPLANE_COMMIT at compile time)
    let mut i = 0;
    while i < a.len() {
        let value = |i: usize| a.get(i + 1).cloned().ok_or_else(|| { eprint!("{USAGE}"); 2 });
        match a[i].as_str() {
            "--help" | "-h" => { print!("{USAGE}"); return Err(0) }
            "--version" => { println!("backplane-hub {}", env!("CARGO_PKG_VERSION")); return Err(0) }
            "--home" => { home = Some(PathBuf::from(value(i)?)); i += 1 }
            "--port" => { port = Some(value(i)?.parse::<u16>().map_err(|_| { eprint!("{USAGE}"); 2 })?); i += 1 }
            "--host" => { host = value(i)?; i += 1 }
            "--window-bin" => { window = Some(PathBuf::from(value(i)?)); i += 1 }
            "--version-string" => { version = value(i)?; i += 1 }
            "--commit" => { commit = value(i)?; i += 1 }
            "--no-tailscale" => { no_tailscale = true; https = false }
            "--tailscale" => no_tailscale = false,
            "--tailscale-https" => { no_tailscale = false; https = true }
            "--tailnet-only" => { no_tailscale = false; https = true; only = true }
            "--foreground" | "--headless" => {}
            _ => { eprint!("{USAGE}"); return Err(2) }
        }
        i += 1;
    }
    if only { if let Err(why) = tailnet_only(&host, std::env::var("BACKPLANE_HTTPS_PORT").ok().as_deref(), copy::on()) { eprintln!("{why}"); return Err(2) } }
    if window.is_none() { window = std::env::var("BACKPLANE_WINDOW_BIN").ok().filter(|p| !p.is_empty()).map(PathBuf::from); }
    // no default home or port: this binary must never land on a live hub's by accident
    match (home, port) { (Some(home), Some(port)) => Ok(Args { window, home, port, host, version, commit, tailscale: tailnet_flags(no_tailscale, https, copy::on()).0, https: tailnet_flags(no_tailscale, https, copy::on()).1 }), _ => { eprint!("{USAGE}"); Err(2) } }
}

/// One hub per home: an exclusive flock on <home>/hub.lock, which says "<pid> <port> flock".
fn take_lock(home: &std::path::Path, port: u16) -> Result<std::fs::File> {
    use std::os::fd::AsRawFd;
    let path = home.join("hub.lock");
    let f = std::fs::OpenOptions::new().create(true).read(true).write(true).truncate(false).open(&path)?;
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let said = std::fs::read_to_string(&path).unwrap_or_default();
        let mut it = said.split_whitespace();
        eprintln!("a hub already serves {} (pid {}, port {})", home.display(), it.next().unwrap_or("?"), it.next().unwrap_or("?"));
        std::process::exit(1);
    }
    f.set_len(0)?;
    use std::io::Write; (&f).write_all(format!("{} {} flock\n", std::process::id(), port).as_bytes())?;
    Ok(f)
}

fn ensure_token(home: &std::path::Path) -> Result<()> {
    let p = home.join("token");
    if !p.exists() {
        let mut b = [0u8; 24]; std::fs::File::open("/dev/urandom")?.read_exact_sync(&mut b)?;
        use std::os::unix::fs::OpenOptionsExt; use std::io::Write;
        std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&p)?.write_all(hex::encode(b).as_bytes())?;
    }
    Ok(())
}
trait ReadExactSync { fn read_exact_sync(&mut self, b: &mut [u8]) -> std::io::Result<()>; }
impl ReadExactSync for std::fs::File { fn read_exact_sync(&mut self, b: &mut [u8]) -> std::io::Result<()> { std::io::Read::read_exact(self, b) } }

/// `backplane-hub --render MODEL...` (what the agent prompt tells agents to run): each model's standard views as PNGs beside it
///. Returns the exit code.
fn render_cli() -> i32 {
    let a: Vec<String> = std::env::args().skip(2).collect();
    let ms: Vec<&str> = a.iter().map(String::as_str).collect();
    let cwd = std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_else(|_| "/".into());
    let tools = files::kicad::stepglb::machine(std::path::Path::new(&std::env::var("BACKPLANE_HOME").unwrap_or_default()));
    let r = files::kicad::render::run(&ms, &cwd, &tools);
    if !r.out.is_empty() { println!("{}", r.out) }
    r.code
}

/// True when the signal is currently ignored (inherited SIG_IGN from nohup or a background job).
fn signal_ignored(sig: i32) -> bool {
    unsafe { let mut old: libc::sigaction = std::mem::zeroed(); libc::sigaction(sig, std::ptr::null(), &mut old) == 0 && old.sa_sigaction == libc::SIG_IGN }
}

/// Every agent, terminal and socket is a few descriptors; macOS starts a process with a soft limit of 256. Raise it to the hard limit (macOS refuses more than 10240).
fn raise_nofile() {
    unsafe {
        let mut l = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut l) != 0 { return }
        let want = if cfg!(target_os = "macos") { l.rlim_max.min(10240) } else { l.rlim_max };
        if l.rlim_cur < want { l.rlim_cur = want; libc::setrlimit(libc::RLIMIT_NOFILE, &l); }
    }
}

#[tokio::main]
async fn main() {
    raise_nofile();
    if std::env::args().nth(1).as_deref() == Some("--agent-reaper") { std::process::exit(agents::proc::reaper_main()) }
    if std::env::args().nth(1).as_deref() == Some("--render") { std::process::exit(render_cli()) }
    let args = match parse() { Ok(a) => a, Err(code) => std::process::exit(code) };
    // a signal that ends the hub says so in hub.log and exits nonzero (128 + the signal); SIGKILL cannot be caught.
    // SIGHUP and SIGINT stay as the parent left them when it ignored them (nohup, a background `&`): registering would override that.
    for (kind, name, n) in [(tokio::signal::unix::SignalKind::terminate(), "SIGTERM", 15), (tokio::signal::unix::SignalKind::interrupt(), "SIGINT", 2), (tokio::signal::unix::SignalKind::hangup(), "SIGHUP", 1)] {
        if n != 15 && signal_ignored(n) { continue }
        if let Ok(mut sig) = tokio::signal::unix::signal(kind) { tokio::spawn(async move { sig.recv().await; eprintln!("backplane-hub: {name} received, exiting"); std::process::exit(128 + n) }); }
    }
    if let Err(e) = run(args).await { eprintln!("backplane-hub: {e:#}"); std::process::exit(1) }
}

async fn run(a: Args) -> Result<()> {
    startup::begin();
    let _ = rejected; // reserved for the handshake callback once the query rules grow
    // the safety gate, before the home is touched: a home that looks like a copy of a real hub's data needs BACKPLANE_COPY=1
    if let Err(why) = copy::gate(&a.home, copy::on()) { eprintln!("{why}"); std::process::exit(1); }
    if let Err(why) = copy::side_by_side_gate(copy::on(), copy::side_by_side_requested()) { eprintln!("{why}"); std::process::exit(1); }
    if copy::on() { eprintln!("{}", copy::NOTE); }
    std::fs::create_dir_all(&a.home)?;
    let _lock = take_lock(&a.home, a.port)?;
    copy::mark(&a.home);
    ensure_token(&a.home)?;
    let presenter: Arc<dyn presenter::Presenter> = Arc::new(present::NotPorted);
    let recent = present::screen::Recent::new();
    let presenter: Arc<dyn presenter::Presenter> = Arc::new(present::Routed::new(presenter, vec![Box::new(recent.clone())], &a.home, false));
    let listen = format!("{}:{}", a.host, a.port);
    startup::mark("home ready");
    session::composer::mention::set_models(settings::models::all);
    let hub = Hub::start(hub::Cfg { home: a.home.clone(), version: a.version, commit: a.commit, listen: listen.clone(), os: match std::env::consts::OS { "macos" => "darwin", o => o }.into(), arch: match std::env::consts::ARCH { "x86_64" => "x64", "aarch64" => "arm64", a => a }.into() }, presenter).await?;
    startup::mark("hub started");
    ckptrun::start(hub.clone());
    recent.bind(&hub);
    session::start(&hub);
    startup::mark("session started");
    // pairing and the /auth routes, before the listener accepts (net lane)
    let _routes = net::start::start_full(&hub, &a.host, a.port, a.tailscale, a.https);
    startup::mark("net started");
    files::start(hub.clone());
    startup::mark("files started");
    settings::open(&hub.cfg.home);
    startup::mark("settings open started");
    term::start(hub.clone());
    startup::mark("term started");
    let agents = agents::Agents::start(hub.clone()).await?;
    settings::order::resume(agents.clone()); // Od.resume: follow every order not yet delivered (a copy follows none)
    files::tools::register(&hub, &agents);
    startup::mark("agents started");
    let _bots = bots::start(hub.clone(), agents.clone()).await;
    startup::mark("bots started");
    let _voice = voice::session::start(&hub); // bots.voice and /voice/in, before settings::start (which then leaves bots.voice alone)
    settings::start(hub.clone(), Some(agents));
    startup::mark("settings start started");
    commands::start(hub.clone());
    startup::mark("commands started");
    if let Some(w) = &a.window { let _ = hub.window_bin.set(w.clone()); }
    let l = TcpListener::bind(&listen).await.with_context(|| format!("bind {listen}"))?;
    startup::mark("listening");
    eprintln!("Backplane listening on http://{listen}");
    // the policies (autosettle, alerts, told, summaries, liveness, star nudge...): keep the Arc, the view lane reads it
    let _policies = statehost::start_with(hub.clone(), copy::passive_from(copy::person_turn_allowed()));
    { let p = _policies.clone(); _bots.set_star(std::sync::Arc::new(move |msg: &str, now: u64| p.star_prompt(msg, now))); } // the star nudge counts a person's bots.say / bots.post
    let (events, torn) = (hub.store.len(), hub.store.torn_bytes);
    eprintln!("presenter: {}; events: {}; torn tail bytes: {}", hub.presenter.name(), events, torn);
    loop {
        let (s, _) = l.accept().await?; s.set_nodelay(true).ok();
        let h = hub.clone();
        // loopback peers are trusted; a tailnet listener (net lane) passes its whois verdict with serve_with
        let trusted = s.peer_addr().map_or(false, |p| p.ip().is_loopback());
        tokio::spawn(async move { let _ = serve_with(h, s, trusted).await; });
    }
}

#[cfg(test)]
mod tests {
    #[test] fn tailnet_only_binds_nothing_public_and_picks_its_serve_port() {
        use super::tailnet_only as t;
        assert_eq!(t("127.0.0.1", Some("8451"), false), Ok(()));
        assert_eq!(t("localhost", Some("8451"), false), Ok(()));
        for h in ["0.0.0.0", "::", "192.168.1.5", "100.64.84.41", ""] { assert!(t(h, Some("8451"), false).unwrap_err().contains("loopback"), "{h:?}") }
        for p in [None, Some(""), Some("0"), Some("x")] { assert!(t("127.0.0.1", p, false).unwrap_err().contains("BACKPLANE_HTTPS_PORT"), "{p:?}") }
        assert!(t("127.0.0.1", Some("8451"), true).unwrap_err().contains("copy"));
    }
    #[test] fn tailnet_flags_follow_the_flags_and_a_copy_runs_neither() {
        use super::tailnet_flags as f;
        assert_eq!(f(false, false, false), (true, false), "default: tailscale on, no https");
        assert_eq!(f(false, true, false), (true, true), "--tailscale-https");
        assert_eq!(f(true, true, false), (false, false), "--no-tailscale turns https off with it");
        assert_eq!(f(false, false, true), (false, false), "a copy: no tailscale");
        assert_eq!(f(false, true, true), (false, false), "a copy: no `tailscale serve` either");
    }
    use super::*;
    // law hub_frames_cbor: for every text t, the hub's frame for it is one binary CBOR frame
    #[test] fn hub_frames_cbor() {
        let (c, _) = (Conn { sent: vec![], client: 1, gen: "1".into(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 }, ());
        for t in ["", "x", "日本語🙂", "reply", "\u{0}\n\t", &"y".repeat(70000)] {
            let m = pack(&c, &json!({"t": t}));
            match m { Message::Binary(b) => assert_eq!(cbor::decode(&b).unwrap(), json!({"t": t})), other => panic!("not binary: {other:?}") }
        }
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    #[test] fn a_handlers_extra_header_replaces_the_default_of_the_same_name() {
        let r = String::from_utf8(response_for("206 Partial Content", "image/png", "Cache-Control: private, no-cache\r\nAccept-Ranges: bytes\r\nX-Content-Type-Options: nosniff\r\nContent-Range: bytes 0-2/10\r\n", b"abc")).unwrap();
        assert_eq!(r.matches("Cache-Control:").count(), 1); assert!(r.contains("Cache-Control: private, no-cache\r\n"));
        assert!(r.contains("Content-Length: 3\r\n") && r.contains("Accept-Ranges: bytes") && r.contains("Content-Range: bytes 0-2/10") && r.starts_with("HTTP/1.1 206 Partial Content\r\n"));
        assert!(r.ends_with("\r\n\r\nabc"));
    }
}

#[cfg(test)]
mod reconnect_tests {
    use super::*;
    use tokio_tungstenite::connect_async;

    type Items = Arc<std::sync::Mutex<Vec<Value>>>;
    struct Probe { items: Items, log_since: Arc<std::sync::Mutex<Option<u64>>>, lives: Arc<std::sync::Mutex<usize>>, task: tokio::task::JoinHandle<()> }

    async fn join(port: u16, since: Option<(usize, String)>) -> Probe {
        let q = since.map(|(s, o)| format!("&since={s}&origin={o}")).unwrap_or_default();
        let (ws, _) = connect_async(format!("ws://127.0.0.1:{port}/ws?token=tok{q}")).await.unwrap();
        let items: Items = Default::default(); let log_since: Arc<std::sync::Mutex<Option<u64>>> = Default::default(); let lives: Arc<std::sync::Mutex<usize>> = Default::default();
        let (i2, l2, v2) = (items.clone(), log_since.clone(), lives.clone());
        let task = tokio::spawn(async move {
            let (_tx, mut rx) = ws.split();
            while let Some(Ok(m)) = rx.next().await {
                let Message::Binary(b) = m else { continue };
                let v = cbor::decode(&b).unwrap();
                match v["t"].as_str().unwrap_or("") {
                    // only log lines count towards `since`; a `live` frame is a snapshot of streaming text
                    "log" => { *l2.lock().unwrap() = v["since"].as_u64(); i2.lock().unwrap().extend(v["items"].as_array().unwrap().iter().cloned()) }
                    "changes" => i2.lock().unwrap().extend(v["items"].as_array().unwrap().iter().cloned()),
                    "live" => *v2.lock().unwrap() += v["items"].as_array().map_or(0, Vec::len),
                    _ => {}
                }
            }
        });
        Probe { items, log_since, lives, task }
    }
    fn norm(v: &Value) -> String { let mut v = v.clone(); if let Some(k) = v["key"].as_str() { if k.starts_with("fallback.input.") || k.starts_with("launch.input.") { v["value"] = json!(""); } } v.to_string() }

    /// Wait until `f` holds (events, not a clock: the 20 s only bounds a hang), so a loaded runner makes the test slower, not wrong.
    async fn settle(f: impl Fn() -> bool) { for _ in 0..2000 { if f() { return } tokio::time::sleep(Duration::from_millis(10)).await } panic!("never settled") }
    // The JS hub's "dropped link" experiment (RESULTS.md, 20 streaming agents): the reference stream against
    // first + resume(since = items held, origin). Nothing may be missing or repeated, in order.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn reconnect_loses_nothing_under_streaming_load() {
        let dir = std::env::temp_dir().join(format!("bp-reconnect-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("token"), "tok").unwrap();
        let mut log = String::from("{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"p\",\"root\":\"/x\",\"at\":1}\n");
        for t in 0..20 { log.push_str(&format!("{{\"$\":\"ThreadCreated\",\"id\":\"t{t}\",\"project\":\"p1\",\"title\":\"T{t}\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":1}}\n")); }
        std::fs::write(dir.join("events.jsonl"), log).unwrap();
        let cfg = hub::Cfg { home: dir.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(presenter::Canned { reply: json!({}) })).await.unwrap();
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
        { let hub = hub.clone(); tokio::spawn(async move { loop { let (s, _) = l.accept().await.unwrap(); let h = hub.clone(); tokio::spawn(async move { let _ = serve(h, s).await; }); } }); }

        let reference = join(port, None).await;
        // 20 streaming agents: live text, then the committed events (a message, a setting whose value the wire blanks)
        let writers: Vec<_> = (0..20).map(|t| { let hub = hub.clone(); tokio::spawn(async move {
            for i in 0..60 {
                hub.live_delta(&format!("t{t}"), "streaming ").await.unwrap();
                hub.commit_many(&[json!({"$":"MessagePosted","thread":format!("t{t}"),"msg":format!("m{t}-{i}"),"role":"assistant","text":"x","at":2}),
                                  json!({"$":"SettingSet","key":format!("fallback.input.t{t}"),"value":format!("long input {i}"),"at":2})]).await.unwrap();
                if i % 7 == 0 { hub.live_end(&format!("t{t}")).await.unwrap(); }
                tokio::time::sleep(Duration::from_millis(1 + (t as u64 % 3))).await;
            } }) }).collect();
        hub.live_delta("tx", "a reply still streaming when the client joins").await.unwrap(); // a live item that is not a log line
        tokio::time::sleep(Duration::from_millis(150)).await;
        let first = join(port, None).await;
        settle(|| !first.items.lock().unwrap().is_empty() && *first.lives.lock().unwrap() > 0).await;   // the join has been sent the log and a live frame
        let held = first.items.lock().unwrap().len();
        let live_seen = *first.lives.lock().unwrap();
        first.task.abort();                                   // the link drops
        tokio::time::sleep(Duration::from_millis(200)).await;
        let resume = join(port, Some((held, wire::origin(&hub)))).await;
        for w in writers { w.await.unwrap(); }
        let total = hub.store.len();
        settle(|| reference.items.lock().unwrap().len() >= total && held + resume.items.lock().unwrap().len() >= total).await;
        assert_eq!(*resume.log_since.lock().unwrap(), Some(held as u64), "the hub honours since for its own origin");
        let r: Vec<String> = reference.items.lock().unwrap().iter().map(norm).collect();
        let mut joined: Vec<String> = first.items.lock().unwrap().iter().take(held).map(norm).collect();
        joined.extend(resume.items.lock().unwrap().iter().map(norm));
        assert_eq!(r.len(), total, "the reference saw every log line once");
        assert_eq!(joined.len(), total, "first + resume hold every log line once (held {held}, live items seen {live_seen})");
        assert_eq!(joined, r, "same items, same order");
        reference.task.abort(); resume.task.abort();
        // and a client with another origin is sent the whole log, not a wrong suffix
        let other = join(port, Some((held, "not-this-log".into()))).await; settle(|| other.items.lock().unwrap().len() >= total).await;
        assert_eq!(*other.log_since.lock().unwrap(), Some(0)); assert_eq!(other.items.lock().unwrap().len(), total); other.task.abort();
        assert!(live_seen > 0, "the join happened while agents were streaming: the live frame carried items that are not log lines");
    }
}

#[cfg(test)]
mod auth_wire_tests {
    use super::*;
    use crate::api::{AuthOutcome, AuthRoute, Fut, HttpHandler, HttpReply, Pairing};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};
    use tokio_tungstenite::connect_async;

    const ROOT: &str = "roottoken"; const DEV: &str = "0123456789abcdef0123456789abcdef";
    // net's Registry in miniature: pairing.js resolve answers the credential that holds, the hub token for an empty or unknown one
    struct Reg { revoked: AtomicBool, last: AtomicU64 }
    impl Pairing for Reg {
        fn holds(&self, t: &str) -> bool { t == ROOT || (t == DEV && !self.revoked.load(SeqCst)) }
        fn resolve(&self, t: &str, client: u64, _r: &str) -> String { if t == DEV && !self.revoked.load(SeqCst) { self.last.store(client, SeqCst); t.into() } else { ROOT.into() } }
        fn active(&self, _c: u64) -> bool { !self.revoked.load(SeqCst) }
        fn forget(&self, _c: u64) {}
    }
    struct NoRoutes; impl AuthRoute for NoRoutes { fn handle(&self, _r: &Req, _c: u64) -> Option<AuthOutcome> { None } }
    struct Probe(bool);
    impl HttpHandler for Probe {
        fn routes(&self) -> &'static [&'static str] { if self.0 { &["/open"] } else { &["/closed"] } }
        fn own_auth(&self) -> bool { self.0 }
        fn call<'a>(&'a self, _h: &'a Arc<Hub>, _r: &'a Req) -> Fut<'a, HttpReply> { { let n = _r.body.len(); let p = _r.peer.is_some(); Box::pin(async move { HttpReply::text("200 OK", &format!("ran {n} peer={p}")) }) } }
    }
    async fn up(name: &str) -> (Arc<Hub>, u16, Arc<Reg>) {
        let dir = std::env::temp_dir().join(format!("bp-authwire-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("token"), ROOT).unwrap();
        std::fs::write(dir.join("events.jsonl"), "{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"p\",\"root\":\"/x\",\"at\":1}\n").unwrap();
        let cfg = hub::Cfg { home: dir, version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(presenter::Canned { reply: json!({"remoteReady":false,"cmds":[],"session":{}}) })).await.unwrap();
        let reg = Arc::new(Reg { revoked: AtomicBool::new(false), last: AtomicU64::new(0) });
        hub.set_pairing(reg.clone(), Arc::new(NoRoutes));
        hub.register_http(Arc::new(Probe(false))); hub.register_http(Arc::new(Probe(true)));
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
        { let hub = hub.clone(); tokio::spawn(async move { loop { let (s, _) = l.accept().await.unwrap(); let h = hub.clone(); tokio::spawn(async move { let _ = serve(h, s).await; }); } }); }
        (hub, port, reg)
    }
    async fn post(port: u16, path: &str, head: &str, body: &[u8]) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(format!("POST {path} HTTP/1.1\r\nHost: x\r\n{head}Connection: close\r\n\r\n").as_bytes()).await.unwrap(); s.write_all(body).await.unwrap();
        let mut out = Vec::new(); let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out)).await; String::from_utf8_lossy(&out).into_owned()
    }
    // bodies: only for POSTs to routes that decide for themselves; Content-Length only, capped; the peer address reaches the handler
    #[tokio::test] async fn a_body_is_read_for_own_auth_posts_only_with_a_length_and_under_the_cap() {
        let (_hub, port, _reg) = up("body").await;
        assert!(post(port, "/open", "Content-Length: 5\r\n", b"hello").await.ends_with("ran 5 peer=true"), "body and peer reach the handler");
        assert!(post(port, "/open", "", b"").await.ends_with("ran 0 peer=true"), "no Content-Length and no Transfer-Encoding is an empty body");
        assert!(post(port, "/open", "Transfer-Encoding: chunked\r\n", b"0\r\n\r\n").await.starts_with("HTTP/1.1 411"), "chunked");
        assert!(post(port, "/open", &format!("Content-Length: {}\r\n", (1 << 20) + 1), b"").await.starts_with("HTTP/1.1 413"), "over the cap");
        assert!(post(port, "/closed", "Content-Length: 5\r\n", b"hello").await.starts_with("HTTP/1.1 401"), "a handler that does not opt out gets no body and no credential-free answer");
    }

    struct Up(&'static str);
    impl HttpHandler for Up {
        fn routes(&self) -> &'static [&'static str] { &["/upload"] }
        fn takes_body(&self) -> bool { true }
        fn call<'a>(&'a self, _h: &'a Arc<Hub>, _r: &'a Req) -> Fut<'a, HttpReply> { Box::pin(async { HttpReply::text("200 OK", "get") }) }
        fn call_body<'a>(&'a self, _h: &'a Arc<Hub>, _r: &'a Req, mut body: crate::api::Body<'a>) -> Fut<'a, HttpReply> {
            Box::pin(async move { body.proceed().await.ok(); let mut got = Vec::new(); tokio::io::AsyncReadExt::read_to_end(&mut body, &mut got).await.ok();
                HttpReply::text("200 OK", &format!("got {} of {} ({})", got.len(), body.len, self.0)) })
        }
    }
    // streaming uploads: credential first, length required, exactly Content-Length bytes, 100-continue answered
    #[tokio::test] async fn a_streaming_upload_needs_the_credential_a_length_and_reads_no_more_than_it() {
        let (hub, port, _reg) = up("stream").await; hub.register_http(Arc::new(Up("x")));
        let send = |head: String, body: Vec<u8>| async move { let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            s.write_all(format!("POST /upload HTTP/1.1\r\nHost: x\r\n{head}Connection: close\r\n\r\n").as_bytes()).await.unwrap(); s.write_all(&body).await.unwrap();
            let mut out = Vec::new(); let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut out)).await; String::from_utf8_lossy(&out).into_owned() };
        assert!(send("Content-Length: 4\r\n".into(), b"abcd".to_vec()).await.starts_with("HTTP/1.1 401"), "no credential");
        let auth = format!("Authorization: Bearer {ROOT}\r\n");
        assert!(send(format!("{auth}Content-Length: 5\r\n"), b"hello".to_vec()).await.ends_with("got 5 of 5 (x)"));
        assert!(send(format!("{auth}Content-Length: 3\r\n"), b"hello".to_vec()).await.ends_with("got 3 of 3 (x)"), "no more than Content-Length is read");
        assert!(send(auth.clone(), vec![]).await.starts_with("HTTP/1.1 411"), "no length");
        let r = send(format!("{auth}Content-Length: 2\r\nExpect: 100-continue\r\n"), b"hi".to_vec()).await;
        assert!(r.starts_with("HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200"), "{r}");
    }

    async fn get(port: u16, path: &str, auth: Option<&str>) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let a = auth.map(|t| format!("Authorization: Bearer {t}\r\n")).unwrap_or_default();
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n{a}Connection: close\r\n\r\n").as_bytes()).await.unwrap();
        let mut out = Vec::new(); s.read_to_end(&mut out).await.unwrap(); String::from_utf8_lossy(&out).into_owned()
    }

    // default-deny: a handler that does not opt out is refused without a credential; only an explicit own_auth skips the check
    #[tokio::test] async fn a_handler_without_own_auth_answers_401_with_no_credential() {
        let (_hub, port, _reg) = up("deny").await;
        assert!(get(port, "/closed", None).await.starts_with("HTTP/1.1 401"), "no credential");
        assert!(get(port, "/closed", Some("wrong")).await.starts_with("HTTP/1.1 401"), "a wrong one");
        assert!(get(port, "/closed", Some("0123456789abcdef0123456789abcdee")).await.starts_with("HTTP/1.1 401"), "a well-formed one nobody holds");
        assert!(get(port, "/closed", Some(DEV)).await.starts_with("HTTP/1.1 200"), "a held device credential");
        assert!(get(port, "/closed", Some(ROOT)).await.starts_with("HTTP/1.1 200"), "the hub token");
        assert!(get(port, "/open", None).await.starts_with("HTTP/1.1 200"), "the opt-out route takes its own decision");
        assert!(get(port, "/nope", None).await.starts_with("HTTP/1.1 404"));
    }
    #[tokio::test] async fn admit_refuses_an_empty_an_unknown_and_a_revoked_token() {
        let (hub, _port, reg) = up("admit").await;
        assert!(hub.admit(7, DEV), "a held device");
        assert!(hub.admit(8, ROOT), "the hub token resolves to itself");
        assert!(!hub.admit(9, ""), "empty resolves to the hub token but is not it");
        assert!(!hub.admit(10, "0123456789abcdef0123456789abcdee"), "unknown");
        reg.revoked.store(true, SeqCst);
        assert!(!hub.admit(11, DEV), "revoked");
    }
    // Out::Close ends the socket: a revoke returns client ids and the hub closes those streams
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn out_close_ends_a_live_stream_and_a_revoke_during_connect_is_not_missed() {
        let (hub, port, reg) = up("close").await;
        let (mut ws, _) = connect_async(format!("ws://127.0.0.1:{port}/ws?view=1&mobile=1&enc=cbor&token={DEV}")).await.unwrap();
        let first = tokio::time::timeout(Duration::from_secs(3), ws.next()).await.expect("greeting in time");
        assert!(matches!(first, Some(Ok(Message::Binary(_)))), "the greeting");
        let client = reg.last.load(SeqCst); assert!(client != 0, "the connection was resolved for its device");
        assert!(hub.is_attached(client), "attached before anything was published");
        hub.close_clients(&[client]);
        let end = tokio::time::timeout(Duration::from_secs(3), async { loop { match ws.next().await { None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break, _ => {} } } }).await;
        assert!(end.is_ok(), "the stream ended after Out::Close");
        // a connection whose device is revoked before it is admitted never gets a greeting
        reg.revoked.store(true, SeqCst);
        let r = connect_async(format!("ws://127.0.0.1:{port}/ws?view=1&mobile=1&enc=cbor&token={DEV}")).await;
        assert!(r.is_err(), "a revoked credential is refused at the upgrade (Auth::allowed)");
    }
}

#[cfg(test)]
mod wire_private_tests {
    use super::*;
    use tokio_tungstenite::connect_async;
    // join and live: a stored or new token, input and ride value never appears in a frame a client receives
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)] async fn a_stored_token_input_and_ride_value_never_reach_a_client_join_or_live() {
        let dir = std::env::temp_dir().join(format!("bp-wirepriv-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("token"), "tok").unwrap();
        let log = [r#"{"$":"ProjectCreated","id":"p1","title":"p","root":"/x","at":1}"#,
            r#"{"$":"DeviceRegistered","id":"d1","token":"STORED-PUSH-TOKEN-1","platform":"ios","bundle":"dev.backplane.mobile","name":"Phone","at":2}"#,
            r#"{"$":"SettingSet","key":"launch.input.t1","value":"STORED-INPUT-SECRET","at":3}"#,
            r#"{"$":"SettingSet","key":"ride.t1","value":"STORED-RIDE-MAIL","at":4}"#].join("\n") + "\n";
        std::fs::write(dir.join("events.jsonl"), log).unwrap();
        let cfg = hub::Cfg { home: dir.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() };
        let hub = Hub::start(cfg, Arc::new(presenter::Canned { reply: json!({}) })).await.unwrap();
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
        { let hub = hub.clone(); tokio::spawn(async move { loop { let (s, _) = l.accept().await.unwrap(); let h = hub.clone(); tokio::spawn(async move { let _ = serve(h, s).await; }); } }); }
        let (ws, _) = connect_async(format!("ws://127.0.0.1:{port}/ws?token=tok")).await.unwrap();
        let (_tx, mut rx) = ws.split();
        let frames: Arc<std::sync::Mutex<Vec<String>>> = Default::default(); let f2 = frames.clone();
        let task = tokio::spawn(async move { while let Some(Ok(Message::Binary(b))) = rx.next().await { f2.lock().unwrap().push(cbor::decode(&b).unwrap().to_string()); } });
        tokio::time::sleep(Duration::from_millis(300)).await;
        hub.commit_many(&[json!({"$":"DeviceRegistered","id":"d2","token":"LIVE-PUSH-TOKEN-2","platform":"android","bundle":"b","name":"x","at":9}),
            json!({"$":"DeviceDropped","id":"d2","token":"LIVE-PUSH-TOKEN-2","at":10}),
            json!({"$":"SettingSet","key":"fallback.input.t1","value":"LIVE-INPUT-SECRET","at":11}),
            json!({"$":"SettingSet","key":"ride.t2","value":"LIVE-RIDE-MAIL","at":12})]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(400)).await;
        task.abort();
        let all = frames.lock().unwrap().join("\n");
        assert!(all.contains("ProjectCreated") && all.contains("DeviceRegistered"), "the frames carried the log (join) and the live batch");
        for s in ["STORED-PUSH-TOKEN-1", "STORED-INPUT-SECRET", "STORED-RIDE-MAIL", "LIVE-PUSH-TOKEN-2", "LIVE-INPUT-SECRET", "LIVE-RIDE-MAIL", "dev.backplane.mobile", "android"] {
            assert!(!all.contains(s), "a frame holds {s}");
        }
    }
    // a signal that was ignored when the hub started (nohup, `&`) reads as ignored, and one that was not does not; the disposition is restored
    #[test] fn an_ignored_signal_reads_as_ignored() {
        let sig = libc::SIGUSR2;
        unsafe {
            let mut old: libc::sigaction = std::mem::zeroed(); libc::sigaction(sig, std::ptr::null(), &mut old);
            let was = signal_ignored(sig);
            let mut ign: libc::sigaction = std::mem::zeroed(); ign.sa_sigaction = libc::SIG_IGN; libc::sigaction(sig, &ign, std::ptr::null_mut());
            assert!(signal_ignored(sig), "SIG_IGN is read as ignored");
            let mut dfl: libc::sigaction = std::mem::zeroed(); dfl.sa_sigaction = libc::SIG_DFL; libc::sigaction(sig, &dfl, std::ptr::null_mut());
            assert!(!signal_ignored(sig), "the default is not ignored");
            libc::sigaction(sig, &old, std::ptr::null_mut()); assert_eq!(signal_ignored(sig), was, "restored");
        }
    }
}

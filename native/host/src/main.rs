//! backplane-hub: the Backplane hub for native clients (GPUI, SwiftUI, Compose).
//! HTTP + authenticated WebSocket; renderer `mobile`; the picture comes from the presenter.
mod agents; mod api; mod auth; mod cbor; mod copy; mod delta; mod files; mod hub; mod model; mod net; mod presenter; mod session; mod store; mod term; mod wire; mod window;

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

const USAGE: &str = "usage: backplane-hub --home DIR --port N [--host ADDR] [--policy-bin PATH] [--no-tailscale] [--foreground] [--headless]\n       backplane-hub --help | --version\n";

struct Args { window: Option<PathBuf>, home: PathBuf, port: u16, host: String, policy: Option<PathBuf>, version: String, commit: String }

fn parse() -> std::result::Result<Args, i32> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (mut home, mut port, mut host, mut policy, mut window) = (None, None, "127.0.0.1".to_string(), None, None::<PathBuf>);
    let (mut version, mut commit) = ("0.14.0".to_string(), std::env::var("BACKPLANE_COMMIT").unwrap_or_else(|_| "unknown".into()));
    let mut i = 0;
    while i < a.len() {
        let value = |i: usize| a.get(i + 1).cloned().ok_or_else(|| { eprint!("{USAGE}"); 2 });
        match a[i].as_str() {
            "--help" | "-h" => { print!("{USAGE}"); return Err(0) }
            "--version" => { println!("backplane-hub {}", env!("CARGO_PKG_VERSION")); return Err(0) }
            "--home" => { home = Some(PathBuf::from(value(i)?)); i += 1 }
            "--port" => { port = Some(value(i)?.parse::<u16>().map_err(|_| { eprint!("{USAGE}"); 2 })?); i += 1 }
            "--host" => { host = value(i)?; i += 1 }
            "--policy-bin" => { policy = Some(PathBuf::from(value(i)?)); i += 1 }
            "--window-bin" => { window = Some(PathBuf::from(value(i)?)); i += 1 }
            "--version-string" => { version = value(i)?; i += 1 }
            "--commit" => { commit = value(i)?; i += 1 }
            "--no-tailscale" | "--foreground" | "--headless" => {}
            _ => { eprint!("{USAGE}"); return Err(2) }
        }
        i += 1;
    }
    if window.is_none() { window = std::env::var("BACKPLANE_WINDOW_BIN").ok().filter(|p| !p.is_empty()).map(PathBuf::from); }
    if policy.is_none() { policy = std::env::var("BACKPLANE_POLICY_BIN").ok().filter(|p| !p.is_empty()).map(PathBuf::from); }
    // no default home or port: this binary must never land on a live hub's by accident
    match (home, port) { (Some(home), Some(port)) => Ok(Args { window, home, port, host, policy, version, commit }), _ => { eprint!("{USAGE}"); Err(2) } }
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

#[tokio::main]
async fn main() {
    let args = match parse() { Ok(a) => a, Err(code) => std::process::exit(code) };
    if let Err(e) = run(args).await { eprintln!("backplane-hub: {e:#}"); std::process::exit(1) }
}

async fn run(a: Args) -> Result<()> {
    let _ = rejected; // reserved for the handshake callback once the query rules grow
    // the safety gate, before the home is touched: a home that looks like a copy of a real hub's data needs BACKPLANE_COPY=1
    if let Err(why) = copy::gate(&a.home, copy::on()) { eprintln!("{why}"); std::process::exit(1); }
    if copy::on() { eprintln!("{}", copy::NOTE); }
    std::fs::create_dir_all(&a.home)?;
    let _lock = take_lock(&a.home, a.port)?;
    copy::mark(&a.home);
    ensure_token(&a.home)?;
    let presenter: Arc<dyn presenter::Presenter> = match &a.policy {
        Some(p) => Arc::new(presenter::BpPolicy::spawn(p).await?),
        None => Arc::new(presenter::Canned { reply: json!({"remoteReady":false,"cmds":[],"session":{}}) }),
    };
    let listen = format!("{}:{}", a.host, a.port);
    let hub = Hub::start(hub::Cfg { home: a.home.clone(), version: a.version, commit: a.commit, listen: listen.clone(), os: "linux".into(), arch: "x64".into() }, presenter).await?;
    session::start(&hub);
    // pairing and the /auth routes, before the listener accepts (net lane)
    let _routes = net::start(&hub, &a.host, a.port);
    files::start(hub.clone());
    term::start(hub.clone());
    agents::Agents::start(hub.clone()).await?;
    if let Some(w) = &a.window { let _ = hub.window_bin.set(w.clone()); }
    let l = TcpListener::bind(&listen).await.with_context(|| format!("bind {listen}"))?;
    eprintln!("Backplane listening on http://{listen}");
    let (events, torn) = (hub.store.len(), hub.store.torn_bytes);
    eprintln!("presenter: {}; events: {}; torn tail bytes: {}", hub.presenter.name(), events, torn);
    loop {
        let (s, _) = l.accept().await?; s.set_nodelay(true).ok();
        let h = hub.clone();
        tokio::spawn(async move { let _ = serve(h, s).await; });
    }
}

#[cfg(test)]
mod tests {
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
        tokio::time::sleep(Duration::from_millis(250)).await;
        let held = first.items.lock().unwrap().len();
        let live_seen = *first.lives.lock().unwrap();
        first.task.abort();                                   // the link drops
        tokio::time::sleep(Duration::from_millis(200)).await;
        let resume = join(port, Some((held, wire::origin(&hub)))).await;
        for w in writers { w.await.unwrap(); }
        tokio::time::sleep(Duration::from_millis(600)).await;

        let total = hub.store.len();
        assert_eq!(*resume.log_since.lock().unwrap(), Some(held as u64), "the hub honours since for its own origin");
        let r: Vec<String> = reference.items.lock().unwrap().iter().map(norm).collect();
        let mut joined: Vec<String> = first.items.lock().unwrap().iter().take(held).map(norm).collect();
        joined.extend(resume.items.lock().unwrap().iter().map(norm));
        assert_eq!(r.len(), total, "the reference saw every log line once");
        assert_eq!(joined.len(), total, "first + resume hold every log line once (held {held}, live items seen {live_seen})");
        assert_eq!(joined, r, "same items, same order");
        reference.task.abort(); resume.task.abort();
        // and a client with another origin is sent the whole log, not a wrong suffix
        let other = join(port, Some((held, "not-this-log".into()))).await; tokio::time::sleep(Duration::from_millis(300)).await;
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
        fn call<'a>(&'a self, _h: &'a Arc<Hub>, _r: &'a Req) -> Fut<'a, HttpReply> { Box::pin(async { HttpReply::text("200 OK", "ran") }) }
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

//! backplane-hub: the Backplane hub for native clients (GPUI, SwiftUI, Compose).
//! HTTP + authenticated WebSocket; renderer `mobile`; the picture comes from the presenter.
mod agents; mod api; mod auth; mod cbor; mod copy; mod delta; mod files; mod hub; mod model; mod presenter; mod session; mod store; mod wire; mod window;

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
    files::start(hub.clone());
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
        let resume = join(port, Some((held, hub.origin.clone()))).await;
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

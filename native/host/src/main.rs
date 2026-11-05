//! backplane-hub: the Backplane hub for native clients (GPUI, SwiftUI, Compose).
//! HTTP + authenticated WebSocket; renderer `mobile`; the picture comes from the presenter.
mod agents; mod api; mod auth; mod cbor; mod delta; mod files; mod hub; mod model; mod presenter; mod store;

use anyhow::{Context, Result};
use auth::Req;
use futures_util::{SinkExt, StreamExt};
use hub::{Conn, Hub};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::{handshake::server::ErrorResponse, protocol::WebSocketConfig, Message};

const USAGE: &str = "usage: backplane-hub --home DIR --port N [--host ADDR] [--policy-bin PATH] [--no-tailscale] [--foreground] [--headless]\n       backplane-hub --help | --version\n";

struct Args { home: PathBuf, port: u16, host: String, policy: Option<PathBuf>, version: String, commit: String }

fn parse() -> std::result::Result<Args, i32> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (mut home, mut port, mut host, mut policy) = (None, None, "127.0.0.1".to_string(), None);
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
            "--version-string" => { version = value(i)?; i += 1 }
            "--commit" => { commit = value(i)?; i += 1 }
            "--no-tailscale" | "--foreground" | "--headless" => {}
            _ => { eprint!("{USAGE}"); return Err(2) }
        }
        i += 1;
    }
    if policy.is_none() { policy = std::env::var("BACKPLANE_POLICY_BIN").ok().filter(|p| !p.is_empty()).map(PathBuf::from); }
    // no default home or port: this binary must never land on a live hub's by accident
    match (home, port) { (Some(home), Some(port)) => Ok(Args { home, port, host, policy, version, commit }), _ => { eprint!("{USAGE}"); Err(2) } }
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

/// A handler's reply: a header it names in `extra` (Cache-Control, Content-Type...) replaces the default of
/// the same name; Content-Length is always the body's.
fn response_for(status: &str, ctype: &str, extra: &str, body: &[u8]) -> Vec<u8> {
    let names: Vec<String> = extra.lines().filter_map(|l| l.split_once(':').map(|(k, _)| k.trim().to_ascii_lowercase())).collect();
    let mut head = format!("HTTP/1.1 {status}\r\n");
    if !names.contains(&"content-type".to_string()) { head.push_str(&format!("Content-Type: {ctype}\r\n")); }
    head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    if !names.contains(&"cache-control".to_string()) { head.push_str("Cache-Control: no-cache\r\n"); }
    head.push_str("Connection: close\r\n");
    for l in extra.split("\r\n").filter(|l| !l.is_empty() && !l.to_ascii_lowercase().starts_with("content-length")) { head.push_str(l); head.push_str("\r\n"); }
    head.push_str("\r\n");
    let mut v = head.into_bytes(); v.extend_from_slice(body); v
}

fn response(status: &str, ctype: &str, extra: &str, body: &[u8]) -> Vec<u8> {
    // core/http.bend Http.response: the same header lines in the same order
    let mut v = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n{extra}\r\n", body.len()).into_bytes();
    v.extend_from_slice(body); v
}

async fn http(hub: &Hub, mut s: TcpStream, req: Req) -> Result<()> {
    let out = match req.route() {
        "/hello" => {
            let body = cbor::encode(&json!({"backplane":hub.cfg.version,"busy":0,"commit":hub.cfg.commit}));
            response("200 OK", "application/cbor", "", &body)
        }
        "/client/capabilities" => if hub.auth.paired_request(&req) {
            response("200 OK", "application/json", "Cache-Control: no-store\r\n", serde_json::to_string(&hub::capabilities())?.as_bytes())
        } else { response("401 Unauthorized", "text/plain", "Cache-Control: no-store\r\n", b"paired authentication required") },
        other => match hub.http_for(other) {
            Some(h) if hub.auth.allowed(&req) => { let r = h.call(&hub.arc(), &req).await; response_for(r.status, &r.ctype, &r.extra, &r.body) }
            Some(_) => response("401 Unauthorized", "text/plain; charset=utf-8", "", b"this Backplane needs its pairing token"),
            None => response("404 Not Found", "text/plain; charset=utf-8", "", b"not found"),
        },
    };
    s.write_all(&out).await?; s.shutdown().await.ok(); Ok(())
}

/// hub_frames_cbor: every frame the hub sends is binary (opcode 2, final): CBOR, never text.
fn pack(_c: &Conn, v: &Value) -> Message { Message::Binary(cbor::encode(v).into()) }

async fn socket(hub: Arc<Hub>, s: TcpStream, cbor_wanted: bool) -> Result<()> {
    socket_inner(hub, s, cbor_wanted).await
}
async fn socket_inner(hub: Arc<Hub>, s: TcpStream, cbor_wanted: bool) -> Result<()> {
    let mut cfg = WebSocketConfig::default(); cfg.max_message_size = Some(524288); cfg.max_frame_size = Some(524288);
    let (mut tx, mut rx) = tokio_tungstenite::accept_async_with_config(s, Some(cfg)).await?.split();
    let (mut conn, greeting) = hub.connect(cbor_wanted);
    tx.send(pack(&conn, &greeting)).await?;
    let mut changes = hub.changed.subscribe();
    let mut outbox = hub.attach(conn.client);
    struct Detach(Arc<Hub>, u64); impl Drop for Detach { fn drop(&mut self) { self.0.detach(self.1) } }
    let _detach = Detach(hub.clone(), conn.client);
    let mut beat = tokio::time::interval(Duration::from_millis(15000)); beat.tick().await;
    let mut due: Option<tokio::time::Instant> = None;
    loop {
        tokio::select! {
            m = rx.next() => {
                let Some(m) = m else { return Ok(()) };
                let v: Value = match m? { Message::Binary(b) => match cbor::decode(&b) { Ok(v) => v, Err(_) => continue }, Message::Text(t) => match serde_json::from_str(t.as_str()) { Ok(v) => v, Err(_) => continue }, Message::Close(_) => return Ok(()), _ => continue };
                for f in hub.request(&mut conn, &v).await { tx.send(pack(&conn, &f)).await?; }
            }
            n = changes.recv() => { match n { Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => if due.is_none() && !conn.view_id.is_empty() { due = Some(tokio::time::Instant::now() + Duration::from_millis(50)) }, Err(_) => return Ok(()) } }
            _ = async { match due { Some(t) => tokio::time::sleep_until(t).await, None => std::future::pending().await } } => {
                due = None; let rev = hub.revision().await;
                if rev != conn.lastrev { tx.send(pack(&conn, &hub.notice(&conn, rev))).await?; }
            }
            f = outbox.recv() => { let Some(f) = f else { return Ok(()) }; tx.send(match f { hub::Out::Value(v) => pack(&conn, &v), hub::Out::Raw(b) => Message::Binary(b.into()) }).await?; }
            _ = beat.tick() => { tx.send(pack(&conn, &json!({"t":"hb"}))).await?; }
        }
    }
}

/// The plain wire stream (what the Bend hub gives a client that did not ask for views): `log` from `since`
/// (honoured only for this log's `origin`), `live`, `info`, then every commit as `changes`, streamed text as
/// `delta`/`delta_end`. One cursor (`sent`) over the log: no commit is skipped or repeated, whatever the timing.
async fn wire(hub: Arc<Hub>, s: TcpStream, since: usize, origin: String) -> Result<()> {
    let mut cfg = WebSocketConfig::default(); cfg.max_message_size = Some(524288); cfg.max_frame_size = Some(524288);
    let (mut tx, mut rx) = tokio_tungstenite::accept_async_with_config(s, Some(cfg)).await?.split();
    let (mut conn, _) = hub.connect(true);
    let mut outbox = hub.attach(conn.client);
    struct Detach(Arc<Hub>, u64); impl Drop for Detach { fn drop(&mut self) { self.0.detach(self.1) } }
    let _detach = Detach(hub.clone(), conn.client);
    // atomic join point: the length now, and a subscription taken under the same lock as commits
    let (mut changes, mut deltas) = (hub.changed.subscribe(), hub.frames.subscribe());
    let n0 = hub.store.len(); // subscribed first: a commit after this point is announced, and the cursor below reads from `sent`
    let from = if origin == hub.origin && since <= n0 { since } else { 0 };
    let mut sent = from;
    let mut first = true;
    while sent < n0 || first {
        let to = (sent + 2000).min(n0);
        let lines = hub.store.range(sent, to)?;
        let items: Vec<Value> = lines.iter().filter_map(|l| Hub::wire_item(l, false)).collect();
        let frame = if first { json!({"t":"log","since":from,"origin":hub.origin,"version":"","info":hub.log_info(),"items":items}) } else { json!({"t":"changes","items":items}) };
        tx.send(pack(&conn, &frame)).await?; sent = to; first = false;
    }
    tx.send(pack(&conn, &hub.live_frame())).await?;
    tx.send(pack(&conn, &hub.info_frame())).await?;
    let mut beat = tokio::time::interval(Duration::from_millis(15000)); beat.tick().await;
    loop {
        tokio::select! {
            m = rx.next() => {
                let Some(m) = m else { return Ok(()) };
                let v: Value = match m? { Message::Binary(b) => match cbor::decode(&b) { Ok(v) => v, Err(_) => continue }, Message::Text(t) => match serde_json::from_str(t.as_str()) { Ok(v) => v, Err(_) => continue }, Message::Close(_) => return Ok(()), _ => continue };
                for f in hub.request(&mut conn, &v).await { tx.send(pack(&conn, &f)).await?; }
            }
            n = changes.recv() => {
                if let Err(tokio::sync::broadcast::error::RecvError::Closed) = n { return Ok(()) }
                // everything committed since the cursor, in order, in frames of bounded size
                loop {
                    let upto = hub.store.len(); if sent >= upto { break }
                    let to = (sent + 500).min(upto);
                    let lines = hub.store.range(sent, to)?;
                    let items: Vec<Value> = lines.iter().filter_map(|l| Hub::wire_item(l, true)).collect();
                    sent = to; tx.send(pack(&conn, &json!({"t":"changes","items":items}))).await?;
                }
            }
            d = deltas.recv() => { if let Ok(f) = d { tx.send(pack(&conn, &f)).await?; } }
            f = outbox.recv() => { let Some(f) = f else { return Ok(()) }; tx.send(match f { hub::Out::Value(v) => pack(&conn, &v), hub::Out::Raw(b) => Message::Binary(b.into()) }).await?; }
            _ = beat.tick() => { tx.send(pack(&conn, &json!({"t":"hb"}))).await?; }
        }
    }
}

async fn serve(hub: Arc<Hub>, s: TcpStream) -> Result<()> {
    let mut buf = vec![0u8; 16384];
    let mut tries = 0;
    let (head_len, req) = loop {
        let n = s.peek(&mut buf).await?; if n == 0 { return Ok(()) }
        let mut hs = [httparse::EMPTY_HEADER; 64];
        let mut r = httparse::Request::new(&mut hs);
        match r.parse(&buf[..n]) {
            Ok(httparse::Status::Complete(len)) => break (len, Req { method: r.method.unwrap_or("").to_string(), target: r.path.unwrap_or("/").to_string(),
                headers: r.headers.iter().map(|h| (h.name.to_string(), String::from_utf8_lossy(h.value).into_owned())).collect() }),
            Ok(httparse::Status::Partial) if n < buf.len() && tries < 200 => { tries += 1; tokio::time::sleep(Duration::from_millis(10)).await }
            _ => { let mut s = s; s.write_all(&response("400 Bad Request", "text/plain; charset=utf-8", "", b"bad request")).await.ok(); return Ok(()) }
        }
    };
    let mut s = s;
    if req.route() == "/ws" && req.header("upgrade").eq_ignore_ascii_case("websocket") {
        if !hub.auth.allowed(&req) {
            let mut sink = vec![0u8; head_len]; s.read_exact(&mut sink).await?;
            s.write_all(&response("401 Unauthorized", "text/plain; charset=utf-8", "", b"this Backplane needs its pairing token")).await?; s.shutdown().await.ok(); return Ok(());
        }
        let (view, mobile) = (req.param("view") == "1", req.param("mobile") == "1");
        if !view { // the plain wire stream: log, live, changes, deltas
            return wire(hub, s, req.param("since").parse().unwrap_or(0), req.param("origin")).await;
        }
        if !mobile { // renderer web is dropped
            let mut sink = vec![0u8; head_len]; s.read_exact(&mut sink).await?;
            s.write_all(&response("400 Bad Request", "text/plain; charset=utf-8", "", b"this hub serves renderer mobile only")).await?; s.shutdown().await.ok(); return Ok(());
        }
        return socket(hub, s, req.param("enc") == "cbor").await;
    }
    let mut sink = vec![0u8; head_len]; s.read_exact(&mut sink).await?;
    http(&hub, s, req).await
}

fn rejected(e: &str) -> ErrorResponse { let mut r = ErrorResponse::new(Some(e.to_string())); *r.status_mut() = 401.try_into().unwrap(); r }

#[tokio::main]
async fn main() {
    let args = match parse() { Ok(a) => a, Err(code) => std::process::exit(code) };
    if let Err(e) = run(args).await { eprintln!("backplane-hub: {e:#}"); std::process::exit(1) }
}

async fn run(a: Args) -> Result<()> {
    let _ = rejected; // reserved for the handshake callback once the query rules grow
    std::fs::create_dir_all(&a.home)?;
    let _lock = take_lock(&a.home, a.port)?;
    ensure_token(&a.home)?;
    let presenter: Arc<dyn presenter::Presenter> = match &a.policy {
        Some(p) => Arc::new(presenter::BpPolicy::spawn(p).await?),
        None => Arc::new(presenter::Canned { reply: json!({"remoteReady":false,"cmds":[],"session":{}}) }),
    };
    let listen = format!("{}:{}", a.host, a.port);
    let hub = Hub::start(hub::Cfg { home: a.home.clone(), version: a.version, commit: a.commit, listen: listen.clone(), os: "linux".into(), arch: "x64".into() }, presenter).await?;
    files::start(hub.clone());
    agents::Agents::start(hub.clone()).await?;
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

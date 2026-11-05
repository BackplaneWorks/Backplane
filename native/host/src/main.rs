//! backplane-hub: the Backplane hub for native clients (GPUI, SwiftUI, Compose).
//! HTTP + authenticated WebSocket; renderer `mobile`; the picture comes from the presenter.
mod auth; mod cbor; mod hub; mod model; mod presenter; mod store;

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
        _ => response("404 Not Found", "text/plain; charset=utf-8", "", b"not found"),
    };
    s.write_all(&out).await?; s.shutdown().await.ok(); Ok(())
}

/// hub_frames_cbor: every frame the hub sends is binary (opcode 2, final): CBOR, never text.
fn pack(_c: &Conn, v: &Value) -> Message { Message::Binary(cbor::encode(v).into()) }

async fn socket(hub: Arc<Hub>, s: TcpStream, cbor_wanted: bool) -> Result<()> {
    let mut cfg = WebSocketConfig::default(); cfg.max_message_size = Some(524288); cfg.max_frame_size = Some(524288);
    let (mut tx, mut rx) = tokio_tungstenite::accept_async_with_config(s, Some(cfg)).await?.split();
    let (mut conn, greeting) = hub.connect(cbor_wanted);
    tx.send(pack(&conn, &greeting)).await?;
    let mut changes = hub.changed.subscribe();
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
        if !(view && mobile) { // plain wire streams belong to the web client, which this hub does not serve
            let mut sink = vec![0u8; head_len]; s.read_exact(&mut sink).await?;
            s.write_all(&response("400 Bad Request", "text/plain; charset=utf-8", "", b"this hub serves view=1&mobile=1 only")).await?; s.shutdown().await.ok(); return Ok(());
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
    let l = TcpListener::bind(&listen).await.with_context(|| format!("bind {listen}"))?;
    eprintln!("Backplane listening on http://{listen}");
    let (events, torn) = { let s = hub.store.lock().await; (s.len(), s.torn_bytes) };
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
        let (c, _) = (Conn { gen: "1".into(), view_id: String::new(), target: Value::Null, cbor: false, lastrev: 0 }, ());
        for t in ["", "x", "日本語🙂", "reply", "\u{0}\n\t", &"y".repeat(70000)] {
            let m = pack(&c, &json!({"t": t}));
            match m { Message::Binary(b) => assert_eq!(cbor::decode(&b).unwrap(), json!({"t": t})), other => panic!("not binary: {other:?}") }
        }
    }
}

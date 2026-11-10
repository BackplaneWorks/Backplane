//! The edge of the hub: HTTP, the authenticated WebSocket, and the two streams over it (the view stream a native client
//! asks for with `?view=1`, the plain log stream otherwise). Lane parity-edge owns this directory; `main.rs` only starts
//! the listener and calls `serve`. Behaviour that is hub's at the edge lives here: junk frames get
//! `{id 0, "malformed request"}`, a view method on the plain stream gets STALE_CONNECTION, the changes a request committed go
//! out before its reply, `/hello`, pairing and header rules.
use crate::{auth::{Auth, Req}, cbor, hub::{self, Conn, Hub}};
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::{handshake::server::ErrorResponse, protocol::WebSocketConfig, Message};

/// `/hello`: the version, the busy count and the commit; `"sideBySide": true` only on a copy that lets the person start a real agent
/// (`copy::person_turn_allowed()`, docs/SIDE-BY-SIDE.md), absent otherwise.
pub fn hello_body(version: &str, commit: &str, side_by_side: bool) -> Value {
    let mut v = json!({"backplane":version,"busy":0,"commit":commit});
    if side_by_side { v["sideBySide"] = json!(true); }
    // a 0.14 home coming across: numbers only (this answer needs no credential), so a client can show progress the moment the hub answers
    if let Some(m) = crate::import014::migrating() { v["migrating"] = m; }
    v
}
/// The same fact in the hub info the views read (info values are text): `sideBySide` = "1" when on, no key when off.
pub fn side_by_side_info_with(o: &mut serde_json::Map<String, Value>, on: bool) { if on { o.insert("sideBySide".into(), json!("1")); } }
/// The view picture the client gets: `shown.sideBySide = true` (top level) on a copy that lets the person start a real agent; untouched otherwise
/// and when `shown` is not an object. The GPUI strip reads it from here.
pub fn shown_with_side_by_side(shown: Value) -> Value { shown_marked(shown, crate::copy::person_turn_allowed()) }
pub fn shown_marked(mut shown: Value, on: bool) -> Value { if on { if let Some(o) = shown.as_object_mut() { o.insert("sideBySide".into(), json!(true)); } } shown }
pub fn side_by_side_info(o: &mut serde_json::Map<String, Value>) { side_by_side_info_with(o, crate::copy::person_turn_allowed()) }

/// `Hub.origin`: the decimal `at` of the log's first change ("0" for an empty log or a first line without `at`).
/// A client resumes (`since`) only into the log it came from.
/// Read from the log at every join: a home that starts empty has "0" until its first commit.
pub fn origin(hub: &Hub) -> String { origin_of(hub.store.get(0).ok().as_deref()) }
pub fn origin_of(first_line: Option<&str>) -> String {
    first_line.and_then(|l| serde_json::from_str::<Value>(l).ok()).and_then(|v| v.get("at").and_then(Value::as_u64)).unwrap_or(0).to_string()
}

/// One inbound frame as a request, or the reply the hub gives for it: text is JSON, binary is CBOR;
/// a frame that does not parse is `malformed request` to id 0; a parsed value answers to its `id`, or 0 when it has none or is not an object.
/// None for frames that are not requests (ping, pong).
pub fn decode(m: &Message) -> Option<std::result::Result<Value, Value>> {
    let v = match m {
        Message::Binary(b) => cbor::decode(b).ok(),
        Message::Text(t) => serde_json::from_str::<Value>(t.as_str()).ok(),
        _ => return None,
    };
    Some(match v {
        Some(Value::Object(mut o)) => { if !o.contains_key("id") { o.insert("id".into(), json!(0)); } Ok(Value::Object(o)) }
        Some(_) => Ok(json!({"id":0})),
        None => Err(json!({"t":"reply","id":0,"ok":false,"error":"malformed request"})),
    })
}

/// A handler's reply: a header it names in `extra` (Cache-Control, Content-Type...) replaces the default of
/// the same name; Content-Length is always the body's.
pub fn response_for(status: &str, ctype: &str, extra: &str, body: &[u8]) -> Vec<u8> {
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

pub fn response(status: &str, ctype: &str, extra: &str, body: &[u8]) -> Vec<u8> {
    // core/http Http.response: the same header lines in the same order
    let mut v = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n{extra}\r\n", body.len()).into_bytes();
    v.extend_from_slice(body); v
}

/// After a body handler refused before reading the body (an upload over its cap): the answer is written, the write side ends, and what the client
/// still sends is read and dropped until it stops (end of stream), goes quiet for 5 s, or 60 s have passed, never more than it declared. A socket
/// closed with unread input resets the connection (Windows always), and a client that sends its whole body before reading the answer, as most do,
/// then saw a broken upload instead of "too big".
pub async fn drain_refused(s: &mut TcpStream, declared: u64) {
    use tokio::io::AsyncReadExt;
    let _ = s.shutdown().await; let mut b = vec![0u8; 1 << 16]; let mut n = 0u64;
    let end = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    while n <= declared {
        let left = end.saturating_duration_since(tokio::time::Instant::now()); if left.is_zero() { break }
        match tokio::time::timeout(left.min(std::time::Duration::from_secs(5)), s.read(&mut b)).await { Ok(Ok(k)) if k > 0 => n += k as u64, _ => break }
    }
}
fn esc(s: &str) -> String { s.chars().map(|c| match c { '&' => "&amp;".into(), '<' => "&lt;".into(), '>' => "&gt;".into(), '"' => "&quot;".into(), '\'' => "&#39;".into(), c => c.to_string() }).collect() }
/// What a browser sees at the hub's address: whose it is, its version and how to connect. Public: nothing on it is a secret.
pub fn landing_page(name: &str, version: &str) -> String {
    let (n, v) = (esc(if name.is_empty() { "this machine" } else { name }), esc(version));
    format!("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Backplane on {n}</title>\
<style>body{{font:16px/1.5 system-ui,sans-serif;max-width:36em;margin:3em auto;padding:0 1em;color:#222}}h1{{font-size:1.3em}}li{{margin:.5em 0}}small{{color:#666}}</style></head><body>\
<h1>This is Backplane on {n}</h1><p><small>Version {v}</small></p><p>To connect, open the Backplane app:</p><ul>\
<li>To link this machine's hub with yours: Settings &rarr; Machines.</li>\
<li>To use this hub from the computer you are on: Settings &rarr; Hubs &rarr; Add a hub.</li>\
<li>On a phone: scan the QR code from Settings &rarr; Devices.</li></ul></body></html>")
}
pub async fn http(hub: &Hub, mut s: TcpStream, mut req: Req) -> Result<()> {
    // health first: it asks nothing of the views, the presenter, the log or the credentials
    if req.route() == "/healthz" {
        let (status, mut v) = crate::guard::health();
        // which build answers: the guard checks it against the update it asked for. Only to a caller the hub knows (its token, a paired device), never to the open network
        if hub.auth.allowed(&req) { v["version"] = hub.cfg.version.clone().into(); v["commit"] = hub.cfg.commit.clone().into(); }
        s.write_all(&response(status, "application/json", "Cache-Control: no-store\r\n", v.to_string().as_bytes())).await.ok(); s.shutdown().await.ok(); return Ok(());
    }
    // the body of a POST to a route that decides for itself (an own_auth handler, /auth/*): Content-Length only, capped; never buffered for the rest
    // a streaming upload: credential first (unless the handler decides itself), then the size, then the handler reads the body
    if let Some(h) = hub.http_for(req.route()).filter(|h| h.takes_body()) {
        if req.method != "POST" { s.write_all(&response("405 Method Not Allowed", "text/plain; charset=utf-8", "", b"POST only")).await.ok(); crate::plat::net::refused_close(&mut s).await; return Ok(()) }
        if !(h.own_auth() || hub.auth.allowed(&req)) { s.write_all(&response("401 Unauthorized", "text/plain; charset=utf-8", "", b"this Backplane needs its pairing token")).await.ok(); crate::plat::net::refused_close(&mut s).await; return Ok(()) }
        let len = req.header("content-length").trim().parse::<u64>().ok();
        // only the framing is the wire's: the handler refuses a size it will not take (with its own JSON answer) before `proceed()`
        let st = if !req.header("transfer-encoding").is_empty() || len.is_none() { Some("411 Length Required") } else { None };
        if let Some(st) = st { s.write_all(&response(st, "text/plain; charset=utf-8", "", st.as_bytes())).await.ok(); crate::plat::net::refused_close(&mut s).await; return Ok(()) }
        let expect = req.header("expect").eq_ignore_ascii_case("100-continue");
        let r = h.call_body(&hub.arc(), &req, crate::api::Body::new(&mut s, len.unwrap(), expect)).await;
        let out = response_for(r.status, &r.ctype, &r.extra, &r.body);
        s.write_all(&out).await?;
        if r.status.starts_with('2') { s.shutdown().await.ok(); } else { drain_refused(&mut s, len.unwrap()).await; }
        return Ok(());
    }
    if req.method == "POST" && (req.route().starts_with("/auth/") || hub.http_for(req.route()).map_or(false, |h| h.own_auth())) {
        // no Content-Length and no Transfer-Encoding is an empty body; chunked is refused
        let len = if req.header("content-length").trim().is_empty() && req.header("transfer-encoding").is_empty() { Some(0) } else { req.header("content-length").trim().parse::<usize>().ok() };
        let status = if !req.header("transfer-encoding").is_empty() || len.is_none() { Some("411 Length Required") } else if len.unwrap() > hub.http_for(req.route()).map_or(crate::auth::BODY_MAX, |h| h.body_max()) { Some("413 Payload Too Large") } else { None };
        if let Some(st) = status { s.write_all(&response(st, "text/plain; charset=utf-8", "", st.as_bytes())).await.ok(); crate::plat::net::refused_close(&mut s).await; return Ok(()) }
        if len.unwrap() > 0 && req.header("expect").eq_ignore_ascii_case("100-continue") { s.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").await.ok(); }
        let mut body = vec![0u8; len.unwrap()];
        match tokio::time::timeout(Duration::from_secs(10), s.read_exact(&mut body)).await {
            Ok(Ok(_)) => req.body = body,
            _ => { s.write_all(&response("408 Request Timeout", "text/plain; charset=utf-8", "", b"408 Request Timeout")).await.ok(); crate::plat::net::refused_close(&mut s).await; return Ok(()) }
        }
    }
    // the /auth routes decide for themselves (enrollment proves itself with its invitation); nothing else does
    if let Some(ar) = hub.auth_route() {
        let client = hub.fresh_client();
        // the registry fsyncs now and then: off the async threads
        let (ar2, req2) = (ar.clone(), req.clone());
        if let Some(o) = tokio::task::spawn_blocking(move || ar2.handle(&req2, client)).await.ok().flatten() {
            hub.close_clients(&o.close);
            if let Some(link) = &o.unpeer { ar.unpeer(link) }
            let r = o.reply; let out = response_for(r.status, &r.ctype, &r.extra, &r.body);
            s.write_all(&out).await?; s.shutdown().await.ok(); return Ok(());
        }
    }
    let out = match req.route() {
        "/hello" => {
            let body = cbor::encode(&hello_body(&hub.cfg.version, &hub.cfg.commit, crate::copy::person_turn_allowed()));
            response("200 OK", "application/cbor", "", &body)
        }
        "/client/capabilities" => if hub.auth.paired_request(&req) {
            response("200 OK", "application/json", "Cache-Control: no-store\r\n", serde_json::to_string(&hub::capabilities())?.as_bytes())
        } else { response("401 Unauthorized", "text/plain", "Cache-Control: no-store\r\n", b"paired authentication required") },
        // the address typed into a browser, and a pairing link (`/pair#invite=...`, whose invite stays in the fragment, which never reaches the hub): a plain page, nothing secret, no credential
        "/" | "/pair" if req.method == "GET" || req.method == "HEAD" => {
            let body = landing_page(&crate::net::botnet::name().await, &hub.cfg.version);
            response_for("200 OK", "text/html; charset=utf-8", "Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\n", if req.method == "HEAD" { b"" } else { body.as_bytes() })
        }
        other => match hub.http_for(other) {
            // DEFAULT-DENY: the credential is checked here for every route unless its handler says `own_auth`
            Some(h) if h.own_auth() || hub.auth.allowed(&req) => { let r = h.call_with(&hub.arc(), &req, hub.fresh_client()).await; response_for(r.status, &r.ctype, &r.extra, &r.body) }
            Some(_) => response("401 Unauthorized", "text/plain; charset=utf-8", "", b"this Backplane needs its pairing token"),
            None => response("404 Not Found", "text/plain; charset=utf-8", "", b"not found"),
        },
    };
    s.write_all(&out).await?; s.shutdown().await.ok(); Ok(())
}

/// hub_frames_cbor: every frame the hub sends is binary (opcode 2, final): CBOR, never text.
pub fn pack(_c: &Conn, v: &Value) -> Message { Message::Binary(cbor::encode(v).into()) }

/// A view is told of streamed text alone at most this often: a reply reads as written at ten steps a second, and each step costs the hub a picture and the client a frame.
pub const LIVE_EVERY: Duration = Duration::from_millis(100);
#[derive(Debug, PartialEq)]
pub enum Tell { Now, In(Duration), No }
/// Whether a view whose revision is current is told (`view.changed`) because the reply being streamed in the thread it shows moved: `seen` is the
/// text it was last given or told about, `(thread, len)` the text now, `since` the time since it was last told of text alone.
/// Only the view showing the thread hears of it: a stream in another thread changes nothing this one draws.
pub fn live_notice(seen: &(String, usize), thread: &str, len: usize, since: Duration) -> Tell {
    if thread.is_empty() || (seen.0 == thread && seen.1 == len) { return Tell::No }
    // a view that just came to this thread and has seen none of its text: nothing to tell while there is none
    if seen.0 != thread && len == 0 { return Tell::No }
    if since >= LIVE_EVERY { Tell::Now } else { Tell::In(LIVE_EVERY - since) }
}

/// A view stream with no credential of its own (tests, no pairing registry installed). Fails closed once one is.
pub async fn socket(hub: Arc<Hub>, s: TcpStream, cbor_wanted: bool) -> Result<()> {
    socket_inner(hub, s, cbor_wanted, String::new()).await
}
/// `token` is the credential the /ws upgrade presented (`Auth::credential`).
pub async fn socket_inner(hub: Arc<Hub>, s: TcpStream, cbor_wanted: bool, token: String) -> Result<()> {
    let mut cfg = WebSocketConfig::default(); cfg.max_message_size = Some(524288); cfg.max_frame_size = Some(524288);
    let (mut tx, mut rx) = tokio_tungstenite::accept_async_with_config(s, Some(cfg)).await?.split();
    let (mut conn, greeting) = hub.connect(cbor_wanted);
    let mut outbox = hub.attach(conn.client);
    struct Detach(Arc<Hub>, u64); impl Drop for Detach { fn drop(&mut self) { self.0.detach(self.1) } }
    let _detach = Detach(hub.clone(), conn.client);
    // attached first, then admitted: a revoke after this point finds the outbox and closes the stream; before it, the credential no longer holds
    if !hub.admit_blocking(conn.client, token.clone()).await { return Ok(()) }
    tx.send(pack(&conn, &greeting)).await?;
    let mut changes = hub.changed.subscribe();
    let mut beat = tokio::time::interval(Duration::from_millis(15000)); beat.tick().await;
    let mut due: Option<tokio::time::Instant> = None;
    // the streamed text this view was last given or told about (its thread, how many bytes), and when it was last told of text alone
    let (mut live, mut live_at): ((String, usize), tokio::time::Instant) = (Default::default(), tokio::time::Instant::now());
    loop {
        tokio::select! {
            m = rx.next() => {
                let Some(m) = m else { return Ok(()) };
                let m = m?;
                if matches!(m, Message::Close(_)) { return Ok(()) }
                let v = match decode(&m) { None => continue, Some(Err(reply)) => { tx.send(pack(&conn, &reply)).await?; continue } Some(Ok(v)) => v };
                // a view request's picture carries the streamed text as it is now: read before the request, so text that comes while it is answered is still news
                let lens = v.get("m").and_then(Value::as_str).filter(|m| m.starts_with("view.")).map(|_| hub.live_lens());
                for f in hub.request(&mut conn, &v).await { tx.send(pack(&conn, &f)).await?; }
                if let Some(lens) = lens { let th = crate::session::shown_thread(conn.client, &conn.target); let n = lens.iter().find(|(t, _)| *t == th).map_or(0, |x| x.1); live = (th, n); }
            }
            n = changes.recv() => { match n { Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => if due.is_none() && !conn.view_id.is_empty() { due = Some(tokio::time::Instant::now() + Duration::from_millis(50)) }, Err(_) => return Ok(()) } }
            _ = async { match due { Some(t) => tokio::time::sleep_until(t).await, None => std::future::pending().await } } => {
                due = None; let rev = hub.revision().await;
                if rev != conn.lastrev { tx.send(pack(&conn, &hub.notice(&conn, rev))).await?; }
                else {
                    // streamed text is no log line, so the revision stands still while a reply is written: the view showing that thread is told as its text grows
                    let th = crate::session::shown_thread(conn.client, &conn.target); let n = hub.live_text_len(&th); let now = tokio::time::Instant::now();
                    match live_notice(&live, &th, n, now.saturating_duration_since(live_at)) {
                        Tell::Now => { tx.send(pack(&conn, &hub.notice(&conn, rev))).await?; live = (th, n); live_at = now; }
                        Tell::In(wait) => due = Some(now + wait),
                        Tell::No => {}
                    }
                }
            }
            f = outbox.recv() => { let Some(f) = f else { return Ok(()) }; tx.send(match f { hub::Out::Value(v) => pack(&conn, &v), hub::Out::Raw(b) => Message::Binary(b.into()), hub::Out::Close => return Ok(()) }).await?; }
            _ = beat.tick() => { tx.send(pack(&conn, &json!({"t":"hb"}))).await?; }
        }
    }
}

/// The plain wire stream: `log` from `since`
/// (honoured only for this log's `origin`), `live`, `info`, then every commit as `changes`, streamed text as
/// `delta`/`delta_end`. One cursor (`sent`) over the log: no commit is skipped or repeated, whatever the timing.
pub async fn wire(hub: Arc<Hub>, s: TcpStream, since: usize, origin_asked: String, token: String) -> Result<()> {
    let mut cfg = WebSocketConfig::default(); cfg.max_message_size = Some(524288); cfg.max_frame_size = Some(524288);
    let (mut tx, mut rx) = tokio_tungstenite::accept_async_with_config(s, Some(cfg)).await?.split();
    let (mut conn, _) = hub.connect(true);
    let mut outbox = hub.attach(conn.client);
    struct Detach(Arc<Hub>, u64); impl Drop for Detach { fn drop(&mut self) { self.0.detach(self.1) } }
    let _detach = Detach(hub.clone(), conn.client);
    if !hub.admit_blocking(conn.client, token.clone()).await { return Ok(()) }
    // atomic join point: the length now, and a subscription taken under the same lock as commits
    let (mut changes, mut deltas) = (hub.changed.subscribe(), hub.frames.subscribe());
    let n0 = hub.store.len(); // subscribed first: a commit after this point is announced, and the cursor below reads from `sent`
    let mine = origin(&hub);
    let from = if origin_asked == mine && since <= n0 { since } else { 0 };
    let mut sent = from;
    let mut first = true;
    while sent < n0 || first {
        let to = (sent + 2000).min(n0);
        let lines = hub.store.range(sent, to)?;
        let items: Vec<Value> = lines.iter().filter_map(|l| Hub::wire_item(l)).collect();
        let frame = if first { json!({"t":"log","since":from,"origin":mine,"version":hub.cfg.version,"info":hub.log_info(),"items":items}) } else { json!({"t":"changes","items":items}) };
        tx.send(pack(&conn, &frame)).await?; sent = to; first = false;
    }
    tx.send(pack(&conn, &hub.live_frame())).await?;
    tx.send(pack(&conn, &hub.info_frame())).await?;
    let mut beat = tokio::time::interval(Duration::from_millis(15000)); beat.tick().await;
    loop {
        tokio::select! {
            m = rx.next() => {
                let Some(m) = m else { return Ok(()) };
                let m = m?;
                if matches!(m, Message::Close(_)) { return Ok(()) }
                let v = match decode(&m) { None => continue, Some(Err(reply)) => { tx.send(pack(&conn, &reply)).await?; continue } Some(Ok(v)) => v };
                // a plain stream has no views: hub answers a view method there STALE_CONNECTION
                if v.get("m").and_then(Value::as_str).map_or(false, |m| m.starts_with("view.")) { tx.send(pack(&conn, &hub::error(&v["id"], "STALE_CONNECTION"))).await?; continue }
                let frames = hub.request(&mut conn, &v).await;
                // events the request committed (project.add, turn.start...) reach this client before the reply that names them
                loop {
                    let upto = hub.store.len(); if sent >= upto { break }
                    let to = (sent + 500).min(upto);
                    let lines = hub.store.range(sent, to)?;
                    let items: Vec<Value> = lines.iter().filter_map(|l| Hub::wire_item(l)).collect();
                    sent = to; tx.send(pack(&conn, &json!({"t":"changes","items":items}))).await?;
                }
                for f in frames { tx.send(pack(&conn, &f)).await?; }
            }
            n = changes.recv() => {
                if let Err(tokio::sync::broadcast::error::RecvError::Closed) = n { return Ok(()) }
                // everything committed since the cursor, in order, in frames of bounded size
                loop {
                    let upto = hub.store.len(); if sent >= upto { break }
                    let to = (sent + 500).min(upto);
                    let lines = hub.store.range(sent, to)?;
                    let items: Vec<Value> = lines.iter().filter_map(|l| Hub::wire_item(l)).collect();
                    sent = to; tx.send(pack(&conn, &json!({"t":"changes","items":items}))).await?;
                }
            }
            d = deltas.recv() => { if let Ok(f) = d { tx.send(pack(&conn, &f)).await?; } }
            f = outbox.recv() => { let Some(f) = f else { return Ok(()) }; tx.send(match f { hub::Out::Value(v) => pack(&conn, &v), hub::Out::Raw(b) => Message::Binary(b.into()), hub::Out::Close => return Ok(()) }).await?; }
            _ = beat.tick() => { tx.send(pack(&conn, &json!({"t":"hb"}))).await?; }
        }
    }
}

/// A connection from a listener that has no opinion on its peer: untrusted.
pub async fn serve(hub: Arc<Hub>, s: TcpStream) -> Result<()> { serve_with(hub, s, false).await }
/// `trusted`: the listener decided this socket's peer is the owner's own device (the tailnet listener, after whois) or loopback
/// (Accept.local). It reaches handlers as `Req.trusted`; it is never a credential.
pub async fn serve_with(hub: Arc<Hub>, s: TcpStream, trusted: bool) -> Result<()> {
    let mut buf = vec![0u8; 16384];
    let mut tries = 0;
    let (head_len, req) = loop {
        let n = s.peek(&mut buf).await?; if n == 0 { return Ok(()) }
        let mut hs = [httparse::EMPTY_HEADER; 64];
        let mut r = httparse::Request::new(&mut hs);
        match r.parse(&buf[..n]) {
            Ok(httparse::Status::Complete(len)) => break (len, { let mut q = Req::new(r.method.unwrap_or(""), r.path.unwrap_or("/"),
                r.headers.iter().map(|h| (h.name.to_string(), String::from_utf8_lossy(h.value).into_owned())).collect()); q.peer = s.peer_addr().ok(); q.trusted = trusted; q }),
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
        if req.param("data") == "1" { return crate::data::socket(hub, s, req.param("enc") == "json", Auth::credential(&req), req.header("host").to_string()).await } // the phone data API (docs/PHONE-DATA.md)
        let (view, mobile) = (req.param("view") == "1", req.param("mobile") == "1");
        // BACKPLANE_WS_DEBUG=1: what the client offers on the upgrade (does it ask for permessage-deflate?), once per connection, to stderr
        if std::env::var("BACKPLANE_WS_DEBUG").map_or(false, |v| v == "1") { eprintln!("ws upgrade: extensions={:?} user-agent={:?}", req.header("sec-websocket-extensions"), req.header("user-agent")) }
        if !view { // the plain wire stream: log, live, changes, deltas
            return wire(hub, s, req.param("since").parse().unwrap_or(0), req.param("origin"), Auth::credential(&req)).await;
        }
        if !mobile { // renderer web is dropped
            let mut sink = vec![0u8; head_len]; s.read_exact(&mut sink).await?;
            s.write_all(&response("400 Bad Request", "text/plain; charset=utf-8", "", b"this hub serves renderer mobile only")).await?; s.shutdown().await.ok(); return Ok(());
        }
        return socket_inner(hub, s, req.param("enc") == "cbor", Auth::credential(&req)).await;
    }
    let mut sink = vec![0u8; head_len]; s.read_exact(&mut sink).await?;
    http(&hub, s, req).await
}

pub fn rejected(e: &str) -> ErrorResponse { let mut r = ErrorResponse::new(Some(e.to_string())); *r.status_mut() = 401.try_into().unwrap(); r }


#[cfg(test)]
mod tests;

//! The listener Boat's public HTTPS address forwards to, on a Boat made without a tailnet (`BACKPLANE_PUBLIC_PORT`, set by its unit). That address
//! is on the open internet, so this listener trusts nobody: it is bound to the Boat's loopback (Boat's relay is the only thing that reaches it),
//! every connection is served untrusted (`wire::serve_with(.., false)`: no loopback trust, no owner), and before the hub sees a request it must
//! present the hub's device or machine credential, unless its route is one that proves itself:
//! - `/healthz` (says nothing about the hub to a stranger), the landing page (`/`, `/pair`) and `/hello`;
//! - `/hook/<id>` and `/ring/audio/<id>`, each checked against its own secret by its handler;
//! - pairing: `/auth/*` (a device's one-time, ten-minute invitation; enrollments are rate limited by the registry) and `/bots/pair/request`,
//!   `/bots/pair/poll` (a machine code: ten minutes, once, ten tries a minute hub-wide, and the person approves);
//! - a linked machine's signed requests: `/far/*`, `/bots/link`, `/hub/status`, `/hub/control` (each checks the signature).
//! Everything else (`/ws`, the phone data API, `/bots/pair/local`, `/oauth/*`, every other route) without a credential is answered 401 here.
use crate::{auth::Req, hub::Hub};
use std::{sync::Arc, time::Duration};
use tokio::io::AsyncWriteExt;

/// The routes that decide for themselves (above). Anything not listed needs the credential at this door.
pub fn open_route(route: &str) -> bool {
    const EXACT: &[&str] = &["/healthz", "/", "/pair", "/hello", "/bots/pair/request", "/bots/pair/poll", "/bots/link", "/hub/status", "/hub/control"];
    const UNDER: &[&str] = &["/hook/", "/ring/audio/", "/auth/", "/far/"];
    if route.contains("..") || route.contains("//") { return false }
    EXACT.contains(&route) || UNDER.iter().any(|p| route.starts_with(p) && route.len() > p.len())
}
/// The door's decision: an open route, or a request the hub's auth admits (its token or a paired device's).
pub fn admits(hub_auth: &crate::auth::Auth, r: &Req) -> bool { open_route(r.route()) || hub_auth.allowed(r) }

/// The head of the request on `s`, peeked (the hub reads it again): None when it is not HTTP.
async fn head(s: &tokio::net::TcpStream) -> Option<Req> {
    let mut buf = vec![0u8; 16384];
    for _ in 0..200 {
        let n = s.peek(&mut buf).await.ok()?; if n == 0 { return None }
        let mut hs = [httparse::EMPTY_HEADER; 64];
        let mut r = httparse::Request::new(&mut hs);
        match r.parse(&buf[..n]) {
            Ok(httparse::Status::Complete(_)) => return Some(Req::new(r.method.unwrap_or(""), r.path.unwrap_or("/"), r.headers.iter().map(|h| (h.name.to_string(), String::from_utf8_lossy(h.value).into_owned())).collect())),
            Ok(httparse::Status::Partial) if n < buf.len() => tokio::time::sleep(Duration::from_millis(10)).await,
            _ => return None,
        }
    }
    None
}

/// Serve one connection through the door.
pub async fn serve(hub: Arc<Hub>, mut s: tokio::net::TcpStream) {
    let Some(r) = head(&s).await else { let _ = s.write_all(&crate::wire::response("400 Bad Request", "text/plain; charset=utf-8", "", b"bad request")).await; return };
    if !admits(&hub.auth, &r) {
        let _ = s.write_all(&crate::wire::response("401 Unauthorized", "text/plain; charset=utf-8", "Cache-Control: no-store\r\n", b"this Backplane needs its pairing token")).await;
        crate::plat::net::refused_close(&mut s).await; return
    }
    let _ = crate::wire::serve_with(hub, s, false).await;
}

/// Bound to the loopback only: `BACKPLANE_PUBLIC_PORT` names the port; nothing when it is unset (every hub but a public Boat's).
pub fn start_from_env(hub: &Arc<Hub>) {
    let Some(port) = std::env::var("BACKPLANE_PUBLIC_PORT").ok().and_then(|p| p.trim().parse::<u16>().ok()).filter(|p| *p > 0) else { return };
    if crate::copy::on() { return }
    let hub = hub.clone();
    tokio::spawn(async move {
        let l = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await { Ok(l) => l, Err(e) => { eprintln!("backplane-hub: the public door on 127.0.0.1:{port}: {e}"); return } };
        println!("Backplane's public door on 127.0.0.1:{port} (every request but pairing, hooks and signed machines needs a credential)");
        accept(hub, l).await
    });
}
pub async fn accept(hub: Arc<Hub>, l: tokio::net::TcpListener) {
    let gate = Arc::new(tokio::sync::Semaphore::new(128));
    loop {
        let Ok((s, _)) = l.accept().await else { tokio::time::sleep(Duration::from_millis(50)).await; continue };
        let Ok(permit) = gate.clone().try_acquire_owned() else { continue };
        s.set_nodelay(true).ok();
        let hub = hub.clone();
        tokio::spawn(async move { let _ = tokio::time::timeout(Duration::from_secs(24 * 3600), serve(hub, s)).await; drop(permit) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn only_the_routes_that_prove_themselves_are_open() {
        for r in ["/healthz", "/", "/pair", "/hello", "/hook/h1", "/ring/audio/a1.m4a", "/auth/enroll", "/far/rpc", "/far/push", "/bots/pair/request", "/bots/pair/poll", "/bots/link", "/hub/status"] { assert!(open_route(r), "{r}") }
        for r in ["/ws", "/bots/pair/local", "/bots/pair", "/oauth/google", "/client/capabilities", "/voice/in", "/files/x", "/mcp/t1", "/hook/", "/far/", "/hook/../ws", "//ws", "/healthz/x", "/auth"] { assert!(!open_route(r), "{r}") }
    }
}

//! The always-on Boat against stand-ins: Boat's sign-in and API (a small HTTP server here), and the public door in front of a real hub.
use super::*;
use crate::{hub::Cfg, presenter::Canned};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The tests that move Boat's bases (process-wide environment variables) take turns.
static ENV: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("bp-aon-{tag}-{}-{}", std::process::id(), hex::encode(crate::net::pairing::urandom(3))));
    std::fs::create_dir_all(&d).unwrap(); d
}

/// A stand-in Boat: answers by method and path from `routes` (a missing one is 404) and records what it was asked (method, path, auth, body).
async fn stand_in(routes: Vec<(&'static str, &'static str, u16, String)>) -> (String, Arc<Mutex<Vec<(String, String, String, String)>>>) {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let seen: Arc<Mutex<Vec<(String, String, String, String)>>> = Default::default();
    let s2 = seen.clone();
    tokio::spawn(async move { loop {
        let Ok((mut s, _)) = l.accept().await else { return };
        let (routes, seen) = (routes.clone(), s2.clone());
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536]; let mut n = 0;
            loop { let k = s.read(&mut buf[n..]).await.unwrap_or(0); if k == 0 { break } n += k;
                let t = String::from_utf8_lossy(&buf[..n]).to_string();
                if let Some(i) = t.find("\r\n\r\n") { let cl = t.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0); if n >= i + 4 + cl { break } } }
            let t = String::from_utf8_lossy(&buf[..n]).to_string();
            let first = t.lines().next().unwrap_or("").to_string(); let mut w = first.split_whitespace();
            let (m, p) = (w.next().unwrap_or("").to_string(), w.next().unwrap_or("").to_string());
            let confirm = t.lines().find_map(|l| l.strip_prefix("X-Ascii-Confirm-Delete: ")).unwrap_or("").to_string();
            if m == "DELETE" && !p.ends_with(&confirm) { let _ = s.write_all(b"HTTP/1.1 409 X\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").await; return }
            let auth = t.lines().find_map(|l| l.strip_prefix("Authorization: ").or_else(|| l.strip_prefix("authorization: "))).unwrap_or("").to_string();
            let body = t.split_once("\r\n\r\n").map(|x| x.1.to_string()).unwrap_or_default();
            seen.lock().unwrap().push((m.clone(), p.clone(), auth, body));
            let (code, b) = routes.iter().find(|r| r.0 == m && p.starts_with(r.1)).map(|r| (r.2, r.3.clone())).unwrap_or((404, "{\"error\":\"Not Found\"}".into()));
            let _ = s.write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len()).as_bytes()).await;
        });
    } });
    (base, seen)
}


/// The pairing machinery on a test hub as `net::start::start_full` wires it (registry, /auth routes, the machine pairing routes), without its
/// process-wide parts (the far mirrors, `farfiles::bind`, the peering globals), which other tests expect to find unset.
fn wire_net(hub: &Arc<Hub>) -> Arc<crate::net::routes::AuthRoutes> {
    struct W(Arc<crate::net::routes::AuthRoutes>);
    impl crate::api::AuthRoute for W {
        fn handle(&self, req: &crate::auth::Req, client: u64) -> Option<crate::api::AuthOutcome> { self.0.handle(req, client).map(|o| crate::api::AuthOutcome { reply: o.reply, close: o.close, unpeer: o.unpeer }) }
        fn unpeer(&self, _link: &str) {}
    }
    let (reg, routes) = crate::net::start::open(&hub.cfg.home, "127.0.0.1", 1).unwrap();
    let table = crate::net::peers::PeerTable::new();
    let peering = Arc::new(crate::net::peering::Peering::new(hub.cfg.home.clone(), reg.clone(), table, routes.clone(), false, "127.0.0.1", 1));
    hub.register_http(Arc::new(crate::net::peerhttp::PeerRoutes(peering)));
    hub.set_pairing(reg, Arc::new(W(routes.clone())));
    routes
}

#[tokio::test] async fn boats_browser_sign_in_gives_a_token_and_a_missing_endpoint_says_to_paste_a_key() {
    let _env = ENV.lock().await;
    let home = dir("login");
    let (base, seen) = stand_in(vec![
        ("POST", "/api/boat/cli/login/start", 200, r#"{"login_url":"https://boat.dev/api/boat/auth/github?state=s","poll_token":"P0llToken123","user_code":"M2SD-BJHQ","expires_at":"2026-11-11T06:11:41.433Z","auth_method":"github","email":null}"#.into()),
        ("GET", "/api/boat/cli/login/poll/P0llToken123", 200, r#"{"status":"complete","token":"boat_abc123"}"#.into()),
    ]).await;
    std::env::set_var("BACKPLANE_BOAT_WEB", &base);
    let l = boat::login_start(&home, "google").await.unwrap();
    assert_eq!((l["url"].as_str(), l["code"].as_str(), l["poll"].as_str()), (Some("https://boat.dev/api/boat/auth/github?state=s"), Some("M2SD-BJHQ"), Some("P0llToken123")));
    assert!(seen.lock().unwrap()[0].3.contains("\"provider\":\"google\""), "the provider is asked for");
    assert_eq!(boat::login_poll(&home, "P0llToken123").await.unwrap().as_deref(), Some("boat_abc123"));
    assert!(boat::login_poll(&home, "../x").await.is_err(), "a poll token is never a path");
    // the endpoint gone (Boat changed its CLI): plainly said, with the key box as the way on
    let (gone, _) = stand_in(vec![]).await;
    std::env::set_var("BACKPLANE_BOAT_WEB", &gone);
    let e = boat::login_start(&home, "github").await.unwrap_err();
    assert!(e.contains("did not start") && e.contains("Paste an API key instead"), "{e}");
    // an answer of another shape is refused too
    let (odd, _) = stand_in(vec![("POST", "/api/boat/cli/login/start", 200, r#"{"url":"x"}"#.into())]).await;
    std::env::set_var("BACKPLANE_BOAT_WEB", &odd);
    assert!(boat::login_start(&home, "github").await.unwrap_err().contains("Paste an API key instead"));
    std::env::remove_var("BACKPLANE_BOAT_WEB");
    assert!(std::fs::read_dir(home.join("secrets/boat")).unwrap().next().is_none(), "no request file is left behind");
}

#[tokio::test] async fn the_api_is_called_with_the_key_as_a_bearer_and_the_create_has_no_auto_stop() {
    let _env = ENV.lock().await;
    let home = dir("api");
    let (base, seen) = stand_in(vec![
        ("GET", "/me", 200, r#"{"ok":true,"user":{"email":"a@b.c","login":"a"}}"#.into()),
        ("GET", "/limits", 200, r#"{"ok":true,"planName":"Standard","planIncluded":{"vmHours":100,"vmHoursRemaining":40,"periodEnd":"2026-12-03T00:00:00Z"},"overagePrices":{"vmHourDollars":0.036},"machineTypes":[{"type":"small","available":true}]}"#.into()),
        ("POST", "/sandboxes", 202, r#"{"ok":true,"sandbox":{"id":"bx_test1","state":"provisioning","createdAt":"2026-11-11T00:00:00Z"}}"#.into()),
        ("DELETE", "/sandboxes/bx_test1", 200, r#"{"ok":true}"#.into()),
    ]).await;
    std::env::set_var("BACKPLANE_BOAT_API", &base);
    let b = boat::Boat::with_key(&home, "boat_secret_key");
    let a = b.account().await.unwrap();
    assert_eq!((a["email"].as_str(), a["hours_left"].as_f64()), (Some("a@b.c"), Some(40.0)));
    let s = b.create("small", "bp-test", "idem1").await.unwrap(); assert_eq!(s["id"], "bx_test1");
    b.delete("bx_test1").await.unwrap();
    std::env::remove_var("BACKPLANE_BOAT_API");
    let seen = seen.lock().unwrap();
    assert!(seen.iter().all(|r| r.2 == "Bearer boat_secret_key"), "the key is the bearer on every call");
    let create = seen.iter().find(|r| r.0 == "POST").unwrap();
    let body: Value = serde_json::from_str(&create.3).unwrap();
    assert_eq!((body["ttlSeconds"].clone(), body["noEnv"].clone(), body["type"].clone()), (Value::Null, json!(true), json!("small")));
    // the key is in no file once the calls are done
    for e in std::fs::read_dir(home.join("secrets/boat")).unwrap() { let t = std::fs::read_to_string(e.unwrap().path()).unwrap_or_default(); assert!(!t.contains("boat_secret_key")) }
}

/// The public door: a stranger reaching the Boat's public address gets the pairing pages, /healthz and the routes that check their own
/// signature or secret, and a 401 everywhere else: /ws, the phone data API, every registered route; the hub token passes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_public_door_refuses_every_unauthenticated_request_but_pairing_health_and_self_checked_routes() {
    let home = dir("door");
    std::fs::write(home.join("token"), "doortoken0123456789").unwrap();
    let cfg = Cfg { home: home.clone(), version: "0.15.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
    let hub = Hub::start(cfg, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let _routes = wire_net(&hub);
    crate::voice::session::start(&hub);
    hub.register_http(Arc::new(crate::settings::diagnostics_http::DiagnosticsJson));
    hub.register_http(Arc::new(crate::guard::Routes));
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    tokio::spawn(public::accept(hub.clone(), l));
    async fn ask(port: u16, req: String) -> u16 {
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(req.as_bytes()).await.unwrap();
        let mut out = vec![]; let _ = tokio::time::timeout(std::time::Duration::from_secs(10), s.read_to_end(&mut out)).await;
        String::from_utf8_lossy(&out).split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0)
    }
    let get = |p: &str, extra: &str| format!("GET {p} HTTP/1.1\r\nHost: abc-3798.on.boat.dev\r\n{extra}Connection: close\r\n\r\n");
    let post = |p: &str| format!("POST {p} HTTP/1.1\r\nHost: abc-3798.on.boat.dev\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}");
    let ws = |q: &str, extra: &str| format!("GET /ws{q} HTTP/1.1\r\nHost: abc-3798.on.boat.dev\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n{extra}\r\n");
    // the sockets: the wire, the views, the phone data API (and so every RPC over them), without and with a wrong credential
    for q in ["", "?data=1", "?data=1&enc=json", "?view=1&mobile=1", "?token=nope", "?data=1&token=0123456789abcdef0123456789abcdef"] {
        assert_eq!(ask(port, ws(q, "")).await, 401, "/ws{q}");
    }
    assert_eq!(ask(port, ws("", "Cookie: bp_token=wrong\r\n")).await, 401);
    // every registered route, GET and POST, with no credential
    let routes = hub.http_routes();
    assert!(routes.len() >= 5 && routes.contains(&"/bots/pair") && routes.contains(&"/voice/in"), "{routes:?}");
    for r in routes.iter().chain(["/bots/pair/local", "/client/capabilities", "/mcp/t1", "/nonexistent"].iter()) {
        let sub = format!("{r}/x1");
        for p in [r.to_string(), sub] {
            for code in [ask(port, get(&p, "")).await, ask(port, post(&p)).await] {
                assert!(!(200..300).contains(&code), "{p} answered {code} to a stranger");
                if !public::open_route(&p) { assert_eq!(code, 401, "{p}: refused at the door") }
            }
        }
    }
    // the far RPC unsigned, and a forged signature: refused by its own check
    assert!(!(200..300).contains(&ask(port, format!("POST /far/rpc HTTP/1.1\r\nHost: h\r\nX-Backplane-Peer: p1\r\nX-Backplane-Timestamp: 1\r\nX-Backplane-Signature: ab\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}")).await));
    // what a stranger may see: health (without the build), the landing page, /hello
    { let h = ask(port, get("/healthz", "")).await; assert!(h == 200 || h == 503, "health answers (503 while a test hub is not booted): {h}"); }
    assert_eq!(ask(port, get("/", "")).await, 200);
    // the hub's own credential passes the door (the same request, now with the token, is no longer a 401)
    assert_ne!(ask(port, get("/hub/status", "Authorization: Bearer doortoken0123456789\r\n")).await, 401);
    assert_ne!(ask(port, ws("", "Authorization: Bearer doortoken0123456789\r\n")).await, 401);
}

#[test] fn the_month_starts_on_the_first() {
    assert_eq!(month_start(1_794_376_392), "2026-11-11T00:00:00Z".replace("11T", "01T"));
    assert_eq!(month_start(0), "1970-01-01T00:00:00Z");
}

/// Pairing through the public door keeps its limits: a machine code works once, and ten tries a minute hub-wide, right or wrong.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pairing_through_the_public_door_is_one_time_and_rate_limited() {
    let home = dir("doorpair");
    std::fs::write(home.join("token"), "doortoken0123456789").unwrap();
    let cfg = Cfg { home: home.clone(), version: "0.15.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
    let hub = Hub::start(cfg, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let routes = wire_net(&hub);
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    tokio::spawn(public::accept(hub.clone(), l));
    async fn claim(port: u16, code: &str) -> (u16, String) {
        let body = json!({"code": code, "name": "stranger", "url": "http://127.0.0.1:9", "check": "123456"}).to_string();
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(format!("POST /bots/pair/request HTTP/1.1\r\nHost: abc-3798.on.boat.dev\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        let mut out = vec![]; let _ = tokio::time::timeout(std::time::Duration::from_secs(10), s.read_to_end(&mut out)).await;
        let t = String::from_utf8_lossy(&out).to_string();
        (t.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0), t)
    }
    let made = routes.reg.machine_code("p0123456789abcdef");
    let code = made["code"].as_str().unwrap().to_string();
    let (c1, _) = claim(port, &code).await; assert_eq!(c1, 200, "the code works through the door");
    let (c2, t2) = claim(port, &code).await; assert_eq!(c2, 403, "and only once"); assert!(t2.contains("used already"), "{t2}");
    let mut last = (0, String::new());
    for i in 0..10 { last = claim(port, &format!("WRONG{i:03}")).await; if last.0 == 429 { break } }
    assert_eq!(last.0, 429, "ten tries a minute, right or wrong: {}", last.1);
    let made2 = routes.reg.machine_code("pfedcba9876543210");
    assert_eq!(claim(port, made2["code"].as_str().unwrap()).await.0, 429, "even the right code waits out the minute");
}

/// The reverse link: hub A, reachable only on its own loopback (B is given an address for it that answers nothing), polls hub B through B's
/// public door; B's signed call to A goes back over the poll and is answered within a few seconds, with A's own signature check applied.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_hub_nobody_can_reach_gets_its_calls_over_its_own_poll_within_seconds() {
    use crate::api::{HttpHandler, HttpReply};
    let (peer, secret) = ("ptunnel0123456789", "ab".repeat(32));
    let mk = |tag: &'static str| async move {
        let home = dir(tag); std::fs::write(home.join("token"), "tunneltoken0123456789").unwrap();
        let cfg = Cfg { home: home.clone(), version: "0.15.0".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() };
        Hub::start(cfg, Arc::new(Canned { reply: json!({}) })).await.unwrap()
    };
    let (a, b) = (mk("tun-a").await, mk("tun-b").await);
    // B: the always-on end, its door, the tunnel route for the one verified link
    let s2 = secret.clone();
    b.register_http(Arc::new(tunnel::Route { secret_of: Arc::new(move |p: &str| (p == "ptunnel0123456789").then(|| s2.clone())), home: b.cfg.home.clone() }));
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let door = l.local_addr().unwrap().port();
    tokio::spawn(public::accept(b.clone(), l));
    // A: a route that checks the link's signature as every far route does, and answers with what it was sent
    struct Echo(String);
    impl HttpHandler for Echo {
        fn routes(&self) -> &'static [&'static str] { &["/tunnel-echo"] }
        fn own_auth(&self) -> bool { true }
        fn call<'a>(&'a self, _h: &'a Arc<Hub>, r: &'a crate::auth::Req) -> Fut<'a, HttpReply> { Box::pin(async move {
            let ok = crate::net::sign::check(&self.0, crate::hub::now_ms() / 1000, r.header("x-backplane-timestamp").parse().unwrap_or(0), &String::from_utf8_lossy(&r.body), r.header("x-backplane-signature"));
            if !ok || r.trusted { return HttpReply::text("401 Unauthorized", "unsigned, or a trusted socket") }
            HttpReply::text("200 OK", &format!("echo:{}", String::from_utf8_lossy(&r.body)))
        }) }
    }
    a.register_http(Arc::new(Echo(secret.clone())));
    let (s3, door_url) = (secret.clone(), format!("http://127.0.0.1:{door}"));
    tunnel::start(&a, Arc::new(move || vec![(peer.to_string(), door_url.clone(), s3.clone())]), Arc::new(|| Box::pin(async { "http://127.0.0.1:9".to_string() })));
    for _ in 0..100 { if tunnel::ends().live(peer) { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    assert!(tunnel::ends().live(peer), "A is polling B");
    // B calls A at an address that answers nothing: it goes over the poll
    let t0 = std::time::Instant::now();
    let now = crate::hub::now_ms() / 1000;
    let r = crate::net::botnet::signed(&b.cfg.home, "http://127.0.0.1:9", peer, &secret, now, "/tunnel-echo", true, r#"{"x":1}"#).await;
    assert_eq!((r.code, r.body.as_str()), (200, r#"echo:{"x":1}"#), "answered by A, its signature checked there, on an untrusted socket");
    assert!(t0.elapsed() < std::time::Duration::from_secs(3), "within seconds: {:?}", t0.elapsed());
    // a second call, while the poll is held again
    let t1 = std::time::Instant::now();
    let r = crate::net::botnet::signed(&b.cfg.home, "http://127.0.0.1:9", peer, &secret, crate::hub::now_ms() / 1000, "/tunnel-echo", true, "{}").await;
    assert_eq!(r.code, 200); assert!(t1.elapsed() < std::time::Duration::from_secs(3));
    // a forged signature, and a call signed with the wrong secret, are refused at A
    let bad = crate::net::botnet::signed(&b.cfg.home, "http://127.0.0.1:9", peer, &"cd".repeat(32), crate::hub::now_ms() / 1000, "/tunnel-echo", true, "{}").await;
    assert_eq!(bad.code, 401);
    // a poll that is not a verified link's, or unsigned, is refused at B's door
    async fn post(port: u16, hs: &str, body: &str) -> u16 {
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(format!("POST /far/tunnel HTTP/1.1\r\nHost: h\r\n{hs}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        let mut out = vec![]; let _ = s.read_to_end(&mut out).await; String::from_utf8_lossy(&out).split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0)
    }
    let ts = crate::hub::now_ms() / 1000; let body = r#"{"wait":false}"#;
    assert_eq!(post(door, "", body).await, 401, "unsigned");
    assert_eq!(post(door, &format!("X-Backplane-Peer: pother0123456789\r\nX-Backplane-Timestamp: {ts}\r\nX-Backplane-Signature: {}\r\n", crate::net::sign::header(&secret, ts, body)), body).await, 401, "not a verified link");
    assert_eq!(post(door, &format!("X-Backplane-Peer: {peer}\r\nX-Backplane-Timestamp: {ts}\r\nX-Backplane-Signature: {}\r\n", crate::net::sign::header(&secret, ts, body)), body).await, 200, "the link itself");
    // a link that is not polling: None, the caller goes directly
    assert!(tunnel::ends().call("pnobody0123456789", "GET", "/x", "", "").await.is_none());
}

/// A browser sign-in may do everything: the hub makes a key of only the always-on Boat's actions with it, and ends the session.
#[tokio::test] async fn a_sign_in_is_traded_for_a_key_of_only_what_the_boat_needs() {
    let _env = ENV.lock().await;
    let home = dir("mint");
    let (base, seen) = stand_in(vec![
        ("POST", "/api-keys/scoped", 201, r#"{"ok":true,"type":"api_key.created","secret":"boat_narrow_secret_1234"}"#.into()),
        ("POST", "/api/boat/auth/logout", 200, r#"{"ok":true}"#.into()),
    ]).await;
    std::env::set_var("BACKPLANE_BOAT_API", &base); std::env::set_var("BACKPLANE_BOAT_WEB", &base);
    let s = boat::Boat::with_key(&home, "session_token_full");
    assert_eq!(s.make_key("backplane-always-on-desk").await.unwrap(), "boat_narrow_secret_1234");
    s.logout().await.unwrap();
    std::env::remove_var("BACKPLANE_BOAT_API"); std::env::remove_var("BACKPLANE_BOAT_WEB");
    let seen = seen.lock().unwrap();
    let body: Value = serde_json::from_str(&seen[0].3).unwrap();
    assert_eq!(body["actions"], json!(boat::ACTIONS)); assert_eq!(body["ttl"], "365d"); assert!(body.get("preset").is_none());
    assert!(!seen[0].3.contains('*') && !seen[0].3.contains("admin"), "nothing more than the Boat needs");
    assert_eq!((seen[1].0.as_str(), seen[1].1.as_str(), seen[1].2.as_str()), ("POST", "/api/boat/auth/logout", "Bearer session_token_full"), "the session is ended with its own token");
}

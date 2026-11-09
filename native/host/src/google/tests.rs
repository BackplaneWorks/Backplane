//! Against a real Hub, its agents (stand-ins only) and bots on a temp home, and a mocked Google: a local HTTP server answering with recorded responses
//! (test/google/*.json, shaped as Google's; every token in them and here is made up). Nothing reaches Google.
use super::*;
use crate::{agents::{proc::Cli, Agents}, api::Caller, hub::{Cfg, Hub}, presenter::Canned};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};

const LIST: &str = include_str!("../../../../test/google/message_list.json");
const META: &str = include_str!("../../../../test/google/message_meta.json");
const FULL: &str = include_str!("../../../../test/google/message_full.json");
const DRAFT: &str = include_str!("../../../../test/google/draft_full.json");
const EVENTS: &str = include_str!("../../../../test/google/events.json");
const EVENT: &str = include_str!("../../../../test/google/event.json");
const REFRESH: &str = "1//test-refresh-not-a-real-token";

#[derive(Default)]
struct Mock { seen: StdMutex<Vec<(String, String, String)>>, dead: StdMutex<bool>, draft_mid: StdMutex<String> }
fn idt(email: &str) -> String { use base64::Engine; format!("h.{}.s", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!("{{\"email\":\"{email}\"}}"))) }
impl Mock {
    fn answer(&self, method: &str, path: &str, body: &str) -> (u16, String) {
        let route = path.split('?').next().unwrap_or("");
        match (method, route) {
            ("POST", "/token") => {
                let q = |k: &str| oauth::param(body, k);
                if q("client_id") != "test-client.apps.googleusercontent.com" || q("client_secret") != "test-client-secret" { return (401, r#"{"error":"invalid_client"}"#.into()) }
                if q("grant_type") == "authorization_code" {
                    if q("code") != "4/0-test-code" || q("code_verifier").len() < 43 { return (400, r#"{"error":"invalid_grant","error_description":"Bad Request"}"#.into()) }
                    return (200, json!({"access_token":"ya29.test-access-1","expires_in":3599,"refresh_token":REFRESH,"scope":"openid https://www.googleapis.com/auth/gmail.modify https://www.googleapis.com/auth/calendar.events https://www.googleapis.com/auth/userinfo.email","token_type":"Bearer","id_token":idt("bub@example.org")}).to_string())
                }
                if *self.dead.lock().unwrap() { return (400, r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#.into()) }
                (200, json!({"access_token":"ya29.test-access-2","expires_in":3599,"scope":"openid https://www.googleapis.com/auth/gmail.modify https://www.googleapis.com/auth/calendar.events","token_type":"Bearer"}).to_string())
            }
            ("POST", "/revoke") => (200, "{}".into()),
            ("GET", "/gmail/v1/users/me/messages") => (200, LIST.into()),
            ("GET", r) if r.starts_with("/gmail/v1/users/me/messages/") => { if path.contains("format=full") { (200, FULL.into()) } else if r.ends_with("d4e6") { (404, r#"{"error":{"code":404,"message":"Requested entity was not found."}}"#.into()) } else { (200, META.into()) } }
            ("GET", r) if r.starts_with("/gmail/v1/users/me/drafts/") => { let mut d: Value = serde_json::from_str(DRAFT).unwrap(); let m = self.draft_mid.lock().unwrap().clone(); if !m.is_empty() { d["message"]["id"] = json!(m) } (200, d.to_string()) }
            ("POST", "/gmail/v1/users/me/drafts/send") => (200, r#"{"id":"18c2f0a1b2c3d4f1","threadId":"18c2f0a1b2c3d4e5","labelIds":["SENT"]}"#.into()),
            ("POST", "/upload/gmail/v1/users/me/drafts") => (200, r#"{"id":"r-5112007431117813722","message":{"id":"18c2f0a1b2c3d4f1","threadId":"18c2f0a1b2c3d4e5","labelIds":["DRAFT"]}}"#.into()),
            ("POST", "/upload/gmail/v1/users/me/messages/send") => (200, r#"{"id":"18c2f0a1b2c3d4f9","threadId":"18c2f0a1b2c3d4e5","labelIds":["SENT"]}"#.into()),
            ("GET", "/calendar/v3/calendars/primary/events") => (200, EVENTS.into()),
            ("GET", r) if r.starts_with("/calendar/v3/calendars/primary/events/") => if r.ends_with("gone") { (410, r#"{"error":{"code":410,"message":"Resource has been deleted"}}"#.into()) } else { (200, EVENT.into()) },
            ("PATCH", _) => { let mut e: Value = serde_json::from_str(EVENT).unwrap(); let b: Value = serde_json::from_str(body).unwrap_or_default(); for (k, v) in b.as_object().cloned().unwrap_or_default() { e[k] = v } (200, e.to_string()) }
            ("DELETE", _) => (204, String::new()),
            ("POST", "/calendar/v3/calendars/primary/events") => { let mut e: Value = serde_json::from_str(body).unwrap_or_default(); e["id"] = json!("newevent1"); e["htmlLink"] = json!("https://www.google.com/calendar/event?eid=bmV3"); (200, e.to_string()) }
            _ => (404, r#"{"error":{"code":404,"message":"no such mock route"}}"#.into()),
        }
    }
    fn saw(&self, method: &str, route: &str) -> Vec<(String, String, String)> { self.seen.lock().unwrap().iter().filter(|(m, p, _)| m == method && p.starts_with(route)).cloned().collect() }
}
async fn serve(m: Arc<Mock>) -> String {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let base = format!("http://{}", l.local_addr().unwrap());
    tokio::spawn(async move { loop { let Ok((mut s, _)) = l.accept().await else { return }; let m = m.clone(); tokio::spawn(async move {
        let mut buf = vec![]; let mut tmp = [0u8; 65536];
        let (head_end, len) = loop { let n = s.read(&mut tmp).await.unwrap_or(0); if n == 0 { return } buf.extend_from_slice(&tmp[..n]);
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") { let head = String::from_utf8_lossy(&buf[..i]).to_string(); let len = head.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0); break (i + 4, len) } };
        while buf.len() < head_end + len { let n = s.read(&mut tmp).await.unwrap_or(0); if n == 0 { break } buf.extend_from_slice(&tmp[..n]); }
        let head = String::from_utf8_lossy(&buf[..head_end]).to_string(); let mut first = head.lines().next().unwrap_or("").split(' ');
        let (method, path) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
        let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
        let auth = head.lines().find(|l| l.to_lowercase().starts_with("authorization:")).unwrap_or("").to_string();
        let (code, out) = if !path.starts_with("/token") && !path.starts_with("/revoke") && !auth.contains("ya29.test-access") { (401, r#"{"error":{"code":401,"message":"Request had invalid authentication credentials."}}"#.to_string()) } else { m.answer(&method, &path, &body) };
        m.seen.lock().unwrap().push((method, path, body));
        let _ = s.write_all(format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).as_bytes()).await;
    }); } });
    base
}

fn dir(tag: &str) -> std::path::PathBuf { let h = std::env::temp_dir().join(format!("bp-google-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&h); std::fs::create_dir_all(&h).unwrap(); h }
fn log(hub: &Arc<Hub>) -> Vec<Value> { hub.store.range(0, usize::MAX).unwrap().iter().filter_map(|l| serde_json::from_str(l).ok()).collect() }
async fn until<T>(f: impl Fn() -> Option<T>) -> T { for _ in 0..200 { if let Some(x) = f() { return x } tokio::time::sleep(std::time::Duration::from_millis(25)).await } panic!("timed out") }
fn text(r: &(bool, Value)) -> String { r.1.as_str().unwrap_or("").to_string() }
fn req(target: &str, host: &str) -> crate::auth::Req { crate::auth::Req { method: "GET".into(), target: target.into(), headers: vec![("Host".into(), host.into()), ("Sec-Fetch-Mode".into(), "navigate".into()), ("Sec-Fetch-Dest".into(), "document".into()), ("Sec-Fetch-Site".into(), "cross-site".into())], body: vec![], peer: Some("127.0.0.1:5555".parse().unwrap()), trusted: true } }

struct Rig { hub: Arc<Hub>, ag: Arc<Agents>, g: Arc<Google>, mock: Arc<Mock>, bot: String, thread: String, _lock: std::sync::MutexGuard<'static, ()> }
async fn rig(tag: &str) -> Rig {
    let lock = crate::settings::vault::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("BACKPLANE_VAULT_KEYRING", "file");
    let (home, sd) = (dir(&format!("{tag}-home")), dir(&format!("{tag}-standins")));
    std::fs::write(home.join("events.jsonl"), "").unwrap();
    crate::settings::vault::open(&home);
    let hub = Hub::start(Cfg { home: home.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "127.0.0.1:3799".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = Agents::start_with(hub.clone(), Cli::standin(&sd)).await.unwrap();
    let bots = crate::bots::start(hub.clone(), ag.clone()).await;
    let mock = Arc::new(Mock::default()); let base = serve(mock.clone()).await;
    let g = Google::new(&hub, &ag, Some(bots.clone()), net::Ends { oauth: base.clone(), api: base.clone(), auth: format!("{base}/auth") });
    *g.client_override.lock().unwrap() = Some(store::Client { id: "test-client.apps.googleusercontent.com".into(), secret: "test-client-secret".into(), from: "vault" });
    *g.here_override.lock().unwrap() = Some(String::new());
    let h = hub.rpc_for("bots.create").unwrap(); let r = h.call(&hub, &Caller { gen: "1".into(), client: 1 }, &json!(1), "bots.create", &json!({"name":"Calendar"})).await.remove(0);
    let bot = r["bot"].as_str().unwrap().to_string();
    let thread = until(|| log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == bot.as_str()).and_then(|e| e["thread"].as_str().map(str::to_string))).await;
    until(|| bots.thread_bot(&thread)).await;
    Rig { hub, ag, g, mock, bot, thread, _lock: lock }
}
/// a sign-in through the hub's own route, as the browser comes back
async fn sign_in(r: &Rig) -> String {
    let a = r.g.begin("", "").await.unwrap();
    let url = r.g.url(&a);
    assert!(url.contains("client_id=test-client.apps.googleusercontent.com") && url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A3799%2Foauth%2Fgoogle") && url.contains("code_challenge_method=S256"), "{url}");
    let route = r.hub.http_for("/oauth/google").unwrap();
    let back = format!("/oauth/google?state={}&code=4%2F0-test-code&scope=email", oauth::enc(&a.state));
    let page = route.call(&r.hub, &req(&back, "127.0.0.1:3799")).await;
    assert_eq!(page.status, "200 OK", "{}", String::from_utf8_lossy(&page.body)); assert!(String::from_utf8_lossy(&page.body).contains("Signed in to Google as bub@example.org"));
    store::id_of("bub@example.org")
}
async fn answer_next(r: &Rig, tool: &str, answer: &str) -> String {
    let (id, detail) = until(|| log(&r.hub).iter().rev().find(|e| e["$"] == "AskOpened" && e["tool"] == tool && !log(&r.hub).iter().any(|c| c["$"] == "AskClosed" && c["id"] == e["id"])).map(|e| (e["id"].as_str().unwrap().to_string(), e["detail"].as_str().unwrap().to_string()))).await;
    r.ag.answer(&id, answer).await.unwrap(); detail
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sign_in_keeps_the_refresh_token_in_the_vault_where_no_agent_reaches_it() {
    let r = rig("signin").await;
    // a browser that comes back with another sign-in's state, or not as a navigation to loopback, gets nothing
    let route = r.hub.http_for("/oauth/google").unwrap();
    assert_eq!(route.call(&r.hub, &req("/oauth/google?state=nope&code=4%2F0-test-code", "127.0.0.1:3799")).await.status, "400 Bad Request");
    let mut x = req("/oauth/google?state=a&code=b", "127.0.0.1:3799"); x.headers.push(("Origin".into(), "https://evil.example".into())); assert_eq!(route.call(&r.hub, &x).await.status, "403 Forbidden");
    let mut y = req("/oauth/google?state=a&code=b", "127.0.0.1:3799"); y.peer = Some("100.64.0.9:1".parse().unwrap()); assert_eq!(route.call(&r.hub, &y).await.status, "403 Forbidden", "a loopback Host from off the machine");
    let id = sign_in(&r).await;
    let a = store::get(&id).unwrap(); assert_eq!((a.email.as_str(), a.refresh.as_str(), a.state.as_str()), ("bub@example.org", REFRESH, "ok"));
    let posted = r.mock.saw("POST", "/token"); assert!(posted[0].2.contains("grant_type=authorization_code") && posted[0].2.contains("code_verifier="), "PKCE: {:?}", posted[0]);
    // the code is spent: the same redirect again finishes nothing
    // the hub's own: not listed, not runnable, not requestable, not settable, and masked
    assert!(!crate::settings::vault::names().iter().any(|n| n.starts_with("hub.")) && !crate::settings::vault::list().to_string().contains("hub.google"));
    let run = crate::settings::vault::run(json!({"command":"echo $T","env":{"T":format!("hub.google.{id}")}})).await; assert!(run["error"].as_str().unwrap().starts_with("no secret named"), "{run}");
    assert!(crate::settings::secret::tool_request(&r.hub, &r.thread, &json!({"name":format!("hub.google.{id}")})).await.1["error"].as_str().unwrap().contains("Backplane's own"));
    assert!(crate::settings::vault::put(&format!("hub.google.{id}"), "xxxxxxxxxxxx", "", "person").starts_with("fail"));
    assert_eq!(crate::settings::mask::mask(&format!("t={REFRESH}")), format!("t={}", crate::settings::mask::mask(REFRESH))); assert!(!crate::settings::mask::mask(REFRESH).contains("refresh-not"));
    // the view: the account, its scopes, no bot yet
    let v = r.g.view().await; assert_eq!(v["accounts"][0]["email"], "bub@example.org"); assert_eq!(v["accounts"][0]["scopes"], "Gmail, Calendar"); assert_eq!(v["accounts"][0]["line"], "connected"); assert_eq!(v["signin"]["done"], true);
    // a bare pasted code with nothing waiting is refused
    assert!(r.g.finish("4/0-test-code", "").await.unwrap_err().contains("not started here"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bot_has_the_tools_only_for_the_accounts_it_is_allowed() {
    let r = rig("gate").await;
    let id = sign_in(&r).await;
    assert!(r.g.call(&r.thread, "gmail_search", &json!({})).await.1.as_str().unwrap().contains("no Google account is allowed for Calendar"));
    assert!(!r.g.listed(&r.thread)); assert!(!r.g.listed("t-not-a-bot"));
    r.g.grant(&r.bot, &id, true).await.unwrap();
    assert!(r.g.listed(&r.thread)); assert!(!r.g.listed("t-not-a-bot"));
    assert_eq!(text(&r.g.call("t-not-a-bot", "google_accounts", &json!({})).await), "error: only a bot has Google's tools (and a run working for one)");
    assert_eq!(text(&r.g.call(&r.thread, "google_accounts", &json!({})).await), "Google accounts this bot may use (the first is the default):\nbub@example.org (Gmail, Calendar)");
    assert!(text(&r.g.call(&r.thread, "gmail_search", &json!({"account":"other@example.org"})).await).contains("may not use a Google account other@example.org"));
    // search lists what Google answered (a message that went missing between the list and the read is left out)
    let s = text(&r.g.call(&r.thread, "gmail_search", &json!({"query":"from:ada is:unread","max":99})).await);
    assert_eq!(s, "1 messages:\n- id 18c2f0a1b2c3d4e5 [unread]  Sun, 8 Nov 2026 09:13:20 -0800\n  From: Ada Example <ada@example.com>\n  Subject: Board review\n  Can we move the review to Thursday? '10:00' works for me.");
    assert!(r.mock.saw("GET", "/gmail/v1/users/me/messages?").iter().any(|x| x.1.contains("q=from%3Aada%20is%3Aunread&maxResults=25")));
    let m = text(&r.g.call(&r.thread, "gmail_read", &json!({"id":"18c2f0a1b2c3d4e5"})).await);
    assert!(m.contains("Attachments: rev-b.pdf (application/pdf, 48213 bytes)") && m.ends_with("\nCan we move the review to Thursday?\n10:00 works for me.\n\nAda"), "{m}");
    // the grant taken back: gone from the list and refused
    r.g.grant(&r.bot, &id, false).await.unwrap(); assert!(!r.g.listed(&r.thread));
    assert!(text(&r.g.call(&r.thread, "gmail_read", &json!({"id":"x"})).await).contains("no Google account is allowed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_draft_is_saved_at_once_and_a_send_waits_for_the_person() {
    let r = rig("mail").await;
    let id = sign_in(&r).await; r.g.grant(&r.bot, &id, true).await.unwrap();
    let _ = r.hub.commit(&json!({"$":"SettingSet","key":"gmail.sig.bub@example.org","value":"-- Bub","at":1})).await;
    let d = text(&r.g.call(&r.thread, "gmail_draft", &json!({"reply_to":"18c2f0a1b2c3d4e5","body":"Thursday works."})).await);
    assert_eq!(d, "Draft saved (draft id r-5112007431117813722, message id 18c2f0a1b2c3d4f1, thread 18c2f0a1b2c3d4e5). The person can review it in Gmail; send it with gmail_send {draft: \"r-5112007431117813722\"}.");
    let up = &r.mock.saw("POST", "/upload/gmail/v1/users/me/drafts")[0].2;
    for want in ["{\"message\":{\"threadId\":\"18c2f0a1b2c3d4e5\"}}", "To: Ada Example <ada@example.com>\r\n", "Subject: Re: Board review\r\n", "In-Reply-To: <CAF=ex1@mail.example.com>\r\n", "References: <CAF=ex0@mail.example.com> <CAF=ex1@mail.example.com>\r\n"] { assert!(up.contains(want), "{want} in {up}") }
    assert!(!log(&r.hub).iter().any(|e| e["$"] == "AskOpened"), "a draft asks nobody");
    // a send: the person sees the whole email; declined, nothing goes
    let g = r.g.clone(); let th = r.thread.clone();
    let call = tokio::spawn(async move { g.call(&th, "gmail_send", &json!({"to":"ada@example.com","subject":"Hi","body":"Line one\nLine two"})).await });
    let detail = answer_next(&r, "gmail_send", "decline").await;
    assert_eq!(detail, "Account: bub@example.org\nFrom: bub@example.org\nTo: ada@example.com\nSubject: Hi\n\nLine one\nLine two\n\n-- Bub");
    assert_eq!(text(&call.await.unwrap()), "The person declined this."); assert!(r.mock.saw("POST", "/upload/gmail/v1/users/me/messages/send").is_empty());
    // allowed: exactly that one goes
    let g = r.g.clone(); let th = r.thread.clone();
    let call = tokio::spawn(async move { g.call(&th, "gmail_send", &json!({"to":"ada@example.com","subject":"Hi","body":"Line one"})).await });
    answer_next(&r, "gmail_send", "accept").await;
    assert_eq!(text(&call.await.unwrap()), "Sent (message id 18c2f0a1b2c3d4f9, thread 18c2f0a1b2c3d4e5).");
    assert_eq!(r.mock.saw("POST", "/upload/gmail/v1/users/me/messages/send").len(), 1);
    // a draft sent as it is; one changed after the person read it is not sent
    let g = r.g.clone(); let th = r.thread.clone();
    let call = tokio::spawn(async move { g.call(&th, "gmail_send", &json!({"draft":"r-5112007431117813722"})).await });
    let detail = answer_next(&r, "gmail_send", "accept").await; assert!(detail.starts_with("Draft: r-5112007431117813722\nAccount: bub@example.org\n") && detail.ends_with("Draft text"), "{detail}");
    assert!(text(&call.await.unwrap()).starts_with("Sent ("));
    let g = r.g.clone(); let th = r.thread.clone(); let mk = r.mock.clone();
    let call = tokio::spawn(async move { g.call(&th, "gmail_send", &json!({"draft":"r-5112007431117813722"})).await });
    let (id2, _) = until(|| log(&r.hub).iter().rev().find(|e| e["$"] == "AskOpened" && !log(&r.hub).iter().any(|c| c["$"] == "AskClosed" && c["id"] == e["id"])).map(|e| (e["id"].as_str().unwrap().to_string(), ()))).await;
    *mk.draft_mid.lock().unwrap() = "18c2f0a1b2c3d4ff".into(); r.ag.answer(&id2, "accept").await.unwrap();
    assert_eq!(text(&call.await.unwrap()), "error: the draft changed after the person reviewed it; ask to send it again");
    assert_eq!(text(&r.g.call(&r.thread, "gmail_send", &json!({"draft":"d","to":"x@y"})).await), "error: send a draft as it is, or update it with gmail_draft first");
    assert!(text(&r.g.call(&r.thread, "gmail_draft", &json!({"to":"a@x","body":"b","attachments":["relative.pdf"]})).await).contains("relative.pdf is not an absolute path"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn calendar_changes_are_shown_in_words_and_run_as_shown() {
    let r = rig("cal").await;
    let id = sign_in(&r).await; r.g.grant(&r.bot, &id, true).await.unwrap();
    let ev = text(&r.g.call(&r.thread, "calendar_events", &json!({"from":"2026-11-09","to":"2026-11-16"})).await);
    assert!(ev.starts_with("1 events (calendar time zone America/Los_Angeles):\n- 2026-11-12T10:00:00-08:00 -> 2026-11-12T10:30:00-08:00  Board review\n  at Lab\n  id 4kq2v7e1sd0m8b3o1jq5rbl1ag, 1 attendees"), "{ev}");
    let g = r.g.clone(); let th = r.thread.clone();
    let call = tokio::spawn(async move { g.call(&th, "calendar_update", &json!({"id":"4kq2v7e1sd0m8b3o1jq5rbl1ag","start":"2026-11-13T11:00:00-08:00"})).await });
    let detail = answer_next(&r, "calendar_update", "accept").await;
    assert_eq!(detail, "Change \"Board review\" (Thu 12 Nov 10:00 -> 10:30)\nTime: Thu 12 Nov 10:00 -> 10:30  =>  Fri 13 Nov 11:00 -> 11:30\nGuests will be told.");
    assert!(text(&call.await.unwrap()).starts_with("Updated:\n- 2026-11-13T11:00:00-08:00 -> 2026-11-13T11:30:00-08:00  Board review"));
    let p = &r.mock.saw("PATCH", "/calendar/v3/calendars/primary/events/")[0]; assert!(p.1.ends_with("?sendUpdates=all")); assert_eq!(serde_json::from_str::<Value>(&p.2).unwrap(), json!({"start":{"dateTime":"2026-11-13T11:00:00-08:00"},"end":{"dateTime":"2026-11-13T11:30:00-08:00"}}));
    // the session allows the next ones of that tool in that thread without a question
    let g = r.g.clone(); let th = r.thread.clone();
    let call = tokio::spawn(async move { g.call(&th, "calendar_create", &json!({"summary":"Ship","start":"2026-11-20","end":"2026-11-20","all_day":true})).await });
    assert_eq!(answer_next(&r, "calendar_create", "session").await, "Ship\n2026-11-20 to 2026-11-20"); assert!(text(&call.await.unwrap()).starts_with("Created:\n- 2026-11-20 -> 2026-11-21  Ship"));
    let asks = log(&r.hub).iter().filter(|e| e["$"] == "AskOpened").count();
    assert!(text(&r.g.call(&r.thread, "calendar_create", &json!({"summary":"Two","start":"2026-11-21","end":"2026-11-21","all_day":true})).await).starts_with("Created:"));
    assert_eq!(log(&r.hub).iter().filter(|e| e["$"] == "AskOpened").count(), asks);
    // refused before anyone is asked
    assert_eq!(text(&r.g.call(&r.thread, "calendar_delete", &json!({"id":"gone"})).await), "error: no event with id gone");
    assert_eq!(text(&r.g.call(&r.thread, "calendar_create", &json!({"summary":"x","start":"2026-11-21T10:00:00Z","end":"2026-11-21T11:00:00Z","reminders":[{"method":"sms","minutes":5}]})).await), "error: an alert's method is popup or email");
    assert_eq!(text(&r.g.call(&r.thread, "calendar_update", &json!({"id":"e1"})).await), "error: nothing to change");
    let g = r.g.clone(); let th = r.thread.clone();
    let call = tokio::spawn(async move { g.call(&th, "calendar_delete", &json!({"id":"4kq2v7e1sd0m8b3o1jq5rbl1ag"})).await });
    assert_eq!(answer_next(&r, "calendar_delete", "accept").await, "Delete \"Board review\" (Thu 12 Nov 10:00 -> 10:30)\n1 guest will be told.");
    assert_eq!(text(&call.await.unwrap()), "Deleted: Board review (Thu 12 Nov 10:00 -> 10:30)");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refresh_google_refuses_is_a_sign_in_expired_and_remove_revokes() {
    let r = rig("expire").await;
    let id = sign_in(&r).await; r.g.grant(&r.bot, &id, true).await.unwrap();
    // an access token that expired is refreshed
    r.g.forget_access(&id);
    assert!(text(&r.g.call(&r.thread, "calendar_events", &json!({})).await).contains("events"));
    assert!(r.mock.saw("POST", "/token").iter().any(|x| x.2.contains("grant_type=refresh_token")));
    // Google refuses the refresh token: the account reads sign-in expired, its tools say so, the phones and the desktop show it
    *r.mock.dead.lock().unwrap() = true; r.g.forget_access(&id);
    assert_eq!(text(&r.g.call(&r.thread, "gmail_search", &json!({})).await), format!("error: {}", expired_text("bub@example.org")));
    assert_eq!(store::get(&id).unwrap().state, "expired");
    let v = r.g.view().await; assert_eq!((v["accounts"][0]["line"].as_str(), v["accounts"][0]["tone"].as_str()), (Some("sign-in expired"), Some("bad")));
    let p = r.g.phone_provider().await; assert_eq!(p["accounts"][0]["state"], "expired"); assert_eq!(p["accounts"][0]["id"], format!("google:{id}").as_str());
    let pv = r.g.phone_view(&format!("google:{id}")).await.unwrap(); assert_eq!(pv["message"].as_str().unwrap().lines().next().unwrap(), "Bots: Calendar");
    assert_eq!(pv["actions"].as_array().unwrap().iter().map(|a| a["label"].as_str().unwrap()).collect::<Vec<_>>(), ["Sign in again", "Calendar: allowed", "Remove"]);
    // signing in again brings it back
    *r.mock.dead.lock().unwrap() = false; sign_in(&r).await; assert_eq!(store::get(&id).unwrap().state, "ok");
    // a phone toggles the bot off, then removes the account: revoked at Google, forgotten, the grant gone
    r.g.phone_act(&json!({"profile":format!("google:{id}"),"action":format!("bot:{}", r.bot)})).await.unwrap(); assert!(!r.g.listed(&r.thread));
    r.g.grant(&r.bot, &id, true).await.unwrap();
    assert!(r.g.phone_act(&json!({"profile":format!("google:{id}"),"action":"remove"})).await.unwrap().is_none());
    assert!(r.mock.saw("POST", "/revoke")[0].2.contains("token=1%2F%2Ftest-refresh")); assert!(store::get(&id).is_none()); assert!(r.g.grants(&r.bot).await.is_empty()); assert!(!r.g.listed(&r.thread));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_phone_signs_in_with_its_own_loopback_and_the_paste_finishes_it() {
    let r = rig("phone").await;
    let v = r.g.phone_act(&json!({"provider":"google","action":"add","redirect":"http://127.0.0.1:49152/oauth/google"})).await.unwrap().unwrap();
    assert_eq!(v["id"], phone::NEW); assert_eq!(v["loopback"], true); assert_eq!(v["signin"]["paste"], true);
    let url = v["signin"]["url"].as_str().unwrap(); assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A49152%2Foauth%2Fgoogle"), "{url}");
    // what the phone caught, sent back whole
    let state = oauth::param(oauth::query(url), "state");
    let done = r.g.phone_act(&json!({"profile":phone::NEW,"action":"code","code":format!("http://127.0.0.1:49152/oauth/google?state={}&code=4%2F0-test-code", oauth::enc(&state))})).await.unwrap().unwrap();
    assert_eq!(done["email"], "bub@example.org"); assert_eq!(done["state"], "connected");
    assert!(r.mock.saw("POST", "/token")[0].2.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A49152%2Foauth%2Fgoogle"), "the code is traded with the redirect it began with");
    // a redirect that is not a client's loopback is not taken: the hub's own instead
    let a = r.g.begin("https://evil.example/oauth/google", "").await.unwrap(); assert_eq!(a.redirect, "http://127.0.0.1:3799/oauth/google");
    // the desktop's `google` request: refused to a connection not granted the accounts
    let rpc = r.hub.rpc_for("google").unwrap();
    let no = rpc.call(&r.hub, &Caller { gen: "x".into(), client: 99 }, &json!(1), "google", &json!({"op":"view"})).await.remove(0); assert_eq!(no["ok"], false);
    crate::settings::accounts_hub::grant_client("x", 99);
    let yes = rpc.call(&r.hub, &Caller { gen: "x".into(), client: 99 }, &json!(1), "google", &json!({"op":"view"})).await.remove(0); assert_eq!(yes["ok"], true); assert_eq!(yes["result"]["bots"][0]["name"], "Calendar");
    crate::settings::accounts_hub::revoke_client(99);
}

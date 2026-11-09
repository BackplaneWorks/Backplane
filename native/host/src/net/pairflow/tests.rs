use super::*;
use crate::net::peering::tests::{node, settle, url, Node};
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};

async fn until<F: Fn() -> bool>(f: F) -> bool { for _ in 0..100 { if f() { return true } tokio::time::sleep(std::time::Duration::from_millis(30)).await } false }
fn pending(n: &Node) -> Vec<Value> { n.p.page_data()["requests"].as_array().cloned().unwrap_or_default() }
fn live(n: &Node) -> Vec<String> { n.p.table.with(|p| p.live().into_iter().map(|x| x.id).collect()) }
fn own(n: &Node, id: &str) -> String { n.p.table.with(|p| p.own_how(id).to_string()) }
fn req(body: Value) -> PReq { PReq { method: "POST".into(), target: "/bots/pair/request".into(), headers: vec![], body: body.to_string().into_bytes(), trusted: false, own_device: false } }
fn clocked(n: &Node, host: &str, t: Arc<AtomicU64>) -> Peering {
    Peering::new(n.home.clone(), n.p.reg.clone(), n.p.table.clone(), n.p.routes.clone(), false, host, n.port).with_clock(Box::new(move || t.load(SeqCst)))
}

#[tokio::test] async fn pair_a_code_a_waiting_request_an_approval_and_the_machines_link() {
    let (a, b) = (node("fa", false).await, node("fb", false).await);
    // (B sits on another Tailscale account: the code and the approval are what pair them)
    b.p.routes.machines(json!({"self":{"name":"b","url":url(&b),"domain":""},"peers":[{"name":"a","url":url(&a),"version":"1","same_user":false}]}), vec![]);
    let made = a.p.make_code().await.unwrap();
    let code = made["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 8);
    let (attempt, check) = b.p.pair_with(&b.hub, &url(&a), &code).await.unwrap();
    assert_eq!(check.len(), 6);
    assert!(until(|| pending(&a).len() == 1).await, "the request waits on A");
    let q = pending(&a).remove(0);
    assert_eq!((q["check"].as_str().unwrap(), q["url"].as_str().unwrap()), (check.as_str(), url(&b).as_str()), "A shows the check code B shows, and B's address");
    assert!(live(&a).is_empty() && live(&b).is_empty(), "nothing is linked before the person approves");
    assert_eq!(b.p.attempt(&attempt)["state"], "waiting");
    // the page rows say so, with Approve and Deny
    let rows = crate::net::machinespage::rows(&a.p.page_data(), &|_| String::new());
    let r = rows.iter().find(|r| r["buttons"][0]["action"] == "machine-approve").expect("an Approve row");
    assert!(r["note"].as_str().unwrap().contains(&check) && r["buttons"][1]["action"] == "machine-deny");
    a.p.approve(&a.hub, q["id"].as_str().unwrap()).await.unwrap();
    assert!(until(|| b.p.attempt(&attempt)["state"] == "linked").await, "{}", b.p.attempt(&attempt));
    settle().await;
    let (la, lb) = (live(&a), live(&b));
    assert_eq!((la.len(), lb.len()), (1, 1)); assert_eq!(la, lb);
    assert_eq!((own(&a, &la[0]), own(&b, &lb[0])), ("approved".into(), "approved".into()));
    assert!(a.p.table.with(|p| p.verified(&la[0])) && b.p.table.with(|p| p.verified(&lb[0])), "trust = paired");
    assert_eq!(botnet::peer_secret(&a.home, &la[0]), botnet::peer_secret(&b.home, &la[0]));
    assert_eq!(botnet::peer_secret(&a.home, &la[0]).len(), 64);
    // the code is spent
    let again = b.p.pair_with(&b.hub, &url(&a), &code).await;
    assert!(again.is_err(), "{again:?}");
}

#[tokio::test] async fn pair_deny_refuses_and_burns_the_code() {
    let (a, b) = (node("da", false).await, node("db", false).await);
    let code = a.p.make_code().await.unwrap()["code"].as_str().unwrap().to_string();
    let (attempt, _) = b.p.pair_with(&b.hub, &url(&a), &code).await.unwrap();
    assert!(until(|| pending(&a).len() == 1).await);
    let id = pending(&a)[0]["id"].as_str().unwrap().to_string();
    a.p.deny(&a.hub, &id).await.unwrap();
    assert!(until(|| b.p.attempt(&attempt)["state"] == "refused").await, "{}", b.p.attempt(&attempt));
    settle().await;
    assert!(live(&a).is_empty() && live(&b).is_empty());
    assert!(botnet::peer_secret(&a.home, "p0").is_empty() && a.p.approve(&a.hub, &id).await.is_err(), "a denied request cannot be approved after");
    let e = b.p.pair_with(&b.hub, &url(&a), &code).await.unwrap_err();
    assert!(e.contains("used already"), "{e}");
}

#[tokio::test] async fn pair_a_request_nobody_answers_expires() {
    let a = node("ea", false).await;
    let t = Arc::new(AtomicU64::new(1_000_000));
    let p = clocked(&a, "127.0.0.1", t.clone());
    let made = a.p.make_code().await.unwrap();
    let r = p.request(&a.hub, &req(json!({"code": made["code"], "name": "m", "url": "http://127.0.0.1:9", "check": "123456"}))).await;
    assert_eq!(r.status, "200 OK", "{}", String::from_utf8_lossy(&r.body));
    let j: Value = serde_json::from_slice(&r.body).unwrap();
    let poll = |p: &Peering| { let q = PReq { body: json!({"id": j["id"], "poll": j["poll"]}).to_string().into_bytes(), ..req(json!({})) }; serde_json::from_slice::<Value>(&p.poll(&q).body).unwrap() };
    assert_eq!(poll(&p)["state"], "pending");
    t.fetch_add(WAIT_S + 1, SeqCst);
    assert!(p.page_data()["requests"].as_array().unwrap().is_empty(), "an expired request is no longer shown");
    let id = j["id"].as_str().unwrap();
    assert!(p.approve(&a.hub, id).await.is_err(), "and cannot be approved");
    assert_eq!(poll(&p)["state"], "denied", "the asker hears a refusal");
    assert_eq!(poll(&p)["state"], "unknown", "once");
}

#[tokio::test] async fn pair_wrong_codes_are_refused_and_rate_limited() {
    let a = node("wa", false).await;
    let made = a.p.make_code().await.unwrap();
    let good = made["code"].as_str().unwrap().to_string();
    let ask = |code: String| { let a = &a; async move { a.p.request(&a.hub, &req(json!({"code": code, "name": "m", "url": "http://127.0.0.1:9", "check": "123456"}))).await } };
    for i in 0..10 { let r = ask(format!("WRONG{i:03}")).await; assert_eq!(r.status, "403 Forbidden", "try {i}"); }
    let r = ask("ZZZZZZZZ".into()).await; assert_eq!(r.status, "429 Too Many Requests");
    let r = ask(good.clone()).await; assert_eq!(r.status, "429 Too Many Requests", "the right code does not get past the limit either");
    assert!(pending(&a).is_empty());
    // a malformed check code does not burn the code
    let b = node("wb", false).await;
    let made = b.p.make_code().await.unwrap();
    let r = b.p.request(&b.hub, &req(json!({"code": made["code"], "name": "m", "url": "http://127.0.0.1:9", "check": "12"}))).await;
    assert_eq!(r.status, "400 Bad Request");
    let r = b.p.request(&b.hub, &req(json!({"code": made["code"], "name": "m", "url": "http://127.0.0.1:9", "check": "123456"}))).await;
    assert_eq!(r.status, "200 OK");
}

#[tokio::test] async fn pair_a_code_works_once_and_a_poll_answers_once() {
    let a = node("ra", false).await;
    let code = a.p.make_code().await.unwrap()["code"].as_str().unwrap().to_string();
    let body = json!({"code": code, "name": "m", "url": "http://127.0.0.1:9", "check": "123456"});
    let first = a.p.request(&a.hub, &req(body.clone())).await; assert_eq!(first.status, "200 OK");
    let second = a.p.request(&a.hub, &req(body)).await;
    assert_eq!((second.status, String::from_utf8_lossy(&second.body).contains("used already")), ("403 Forbidden", true), "a replay of the request");
    let j: Value = serde_json::from_slice(&first.body).unwrap();
    let id = j["id"].as_str().unwrap().to_string();
    a.p.approve(&a.hub, &id).await.unwrap();
    let q = |poll: &str| PReq { body: json!({"id": id, "poll": poll}).to_string().into_bytes(), ..req(json!({})) };
    assert_eq!(a.p.poll(&q("wrong")).status, "404 Not Found", "not the asker's poll token");
    let got: Value = serde_json::from_slice(&a.p.poll(&q(j["poll"].as_str().unwrap())).body).unwrap();
    assert_eq!(got["state"], "approved"); assert!(got["invite"].as_str().unwrap().starts_with("bp1:"));
    assert_eq!(a.p.poll(&q(j["poll"].as_str().unwrap())).status, "404 Not Found", "the invite is handed over once");
}

#[tokio::test] async fn pair_loopback_is_refused_both_ways() {
    let a = node("la", false).await;
    // A reaches other machines at a network address; a request that announces loopback is refused
    let p = clocked(&a, "100.64.0.9", Arc::new(AtomicU64::new(5)));
    let code = a.p.make_code().await.unwrap()["code"].as_str().unwrap().to_string();
    let r = p.request(&a.hub, &req(json!({"code": code, "name": "m", "url": "http://127.0.0.1:9", "check": "123456"}))).await;
    assert_eq!(r.status, "403 Forbidden"); assert!(String::from_utf8_lossy(&r.body).contains("loopback"));
    assert!(pending(&a).is_empty());
    let r = p.request(&a.hub, &req(json!({"code": code, "name": "m", "url": "http://100.64.0.3:3787", "check": "123456"}))).await;
    assert_eq!(r.status, "200 OK", "the refusal did not spend the code");
    // and B, with only a loopback address, will not ask a network hub at all
    let b = Arc::new(clocked(&a, "127.0.0.1", Arc::new(AtomicU64::new(5))));
    let e = b.pair_with(&a.hub, "http://100.64.0.2:3787", "ABCDEFGH").await.unwrap_err();
    assert!(e.contains("loopback"), "{e}");
}

#[tokio::test] async fn pair_a_machine_found_on_the_tailnet_is_listed_never_linked() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    let seen = Arc::new(std::sync::Mutex::new(Vec::<String>::new())); let s2 = seen.clone();
    tokio::spawn(async move { loop { let Ok((mut c, _)) = l.accept().await else { return }; let s3 = s2.clone(); tokio::spawn(async move {
        let mut b = [0u8; 1024]; let n = c.read(&mut b).await.unwrap_or(0); s3.lock().unwrap().push(String::from_utf8_lossy(&b[..n]).lines().next().unwrap_or("").to_string());
        let body = crate::cbor::encode(&json!({"backplane":"0.14.0"}));
        let _ = c.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes()).await; let _ = c.write_all(&body).await; }); } });
    let status = r#"{"Self":{"HostName":"me","DNSName":"me.t.ts.net.","UserID":1,"TailscaleIPs":["100.1.0.1"]},"Peer":{"k":{"HostName":"stranger","UserID":2,"Online":true,"TailscaleIPs":["127.0.0.1"]}}}"#;
    let a = node("ga", false).await;
    let j = crate::net::tailnet::find(status, port, "").await;
    assert_eq!(j["peers"][0]["name"], "stranger"); assert_eq!(j["peers"][0]["same_user"], false, "found, and labelled as another account's");
    a.p.routes.machines(j, vec![]);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(live(&a).is_empty(), "discovery links nothing");
    assert!(seen.lock().unwrap().iter().all(|l| l.starts_with("GET /hello")), "and asks the machine nothing but /hello: {:?}", seen.lock().unwrap());
    let found = a.p.page_data()["found"].clone();
    assert_eq!(found[0]["same_user"], false);
    let rows = crate::net::machinespage::rows(&a.p.page_data(), &|_| String::new());
    assert!(rows.iter().any(|r| r["buttons"][0]["action"] == "machine-pair" && r["buttons"][0]["value"].as_str().unwrap().contains(&port.to_string())), "offered as a suggestion to pair, with a code field");
    // the old doors are gone
    let r = botnet::plain(&format!("{}/bots/pair?name=x&url=http://127.0.0.1:1", url(&a))).await;
    assert_eq!(r.code, 404);
}

#[tokio::test] async fn pair_links_made_before_codes_stay_trusted_and_an_unvouched_one_is_unpaired() {
    let a = node("ua", false).await;
    let (old, pasted) = ("p0123456789abcdef", "pfedcba9876543210");
    a.hub.commit_many(&[json!({"$":"PeerSet","id":old,"name":"old","url":"http://100.64.0.2:3787","revoked":false,"at":1}), json!({"$":"SettingSet","key":format!("peer.own.{old}"),"value":"pair","at":1}),
        json!({"$":"PeerSet","id":pasted,"name":"pasted","url":"http://100.64.0.3:3787","revoked":false,"at":1})]).await.unwrap();
    settle().await;
    assert_eq!(live(&a).len(), 2, "both kept");
    assert!(a.p.table.with(|p| p.verified(old)), "a link made before pairing needed a code stays trusted (the person's decision)");
    assert!(!a.p.table.with(|p| p.verified(pasted)), "a pasted invite nobody vouched for is not");
    let rows = crate::net::machinespage::rows(&a.p.page_data(), &|_| String::new());
    let r = rows.iter().find(|r| r["label"] == "old").unwrap();
    assert!(r["note"].as_str().unwrap().ends_with("Paired"), "{r}");
    let r = rows.iter().find(|r| r["label"] == "pasted").unwrap();
    assert!(r["note"].as_str().unwrap().contains("Unpaired") && r["buttons"][0]["action"] == "machine-mine" && r["buttons"][1]["action"] == "machine-remove", "{r}");
    let p = crate::net::peerrpc::PeerRpc(a.p.clone());
    use crate::api::{Caller, RpcHandler};
    let c = Caller { gen: String::new(), client: 1 };
    let ok = p.call(&a.hub, &c, &json!(1), "machines.page.act", &json!({"action": "machine-mine", "value": pasted})).await.remove(0);
    assert_eq!(ok["ok"], true, "{ok}"); settle().await;
    assert!(a.p.table.with(|p| p.verified(pasted)), "Mine confirms it");
    let page = p.call(&a.hub, &c, &json!(2), "machines.page", &json!({})).await.remove(0);
    assert!(page["rows"].as_array().unwrap().iter().any(|r| r["label"] == "pasted" && r["note"].as_str().unwrap().ends_with("Paired")));
    let gone = p.call(&a.hub, &c, &json!(3), "machines.page.act", &json!({"action": "machine-remove", "value": pasted})).await.remove(0);
    assert_eq!(gone["ok"], true); settle().await; assert_eq!(live(&a).len(), 1);
}

#[tokio::test] async fn pair_the_cli_door_needs_the_hubs_token_and_makes_codes() {
    let a = node("ca", false).await;
    let call = |auth: &'static str, body: &'static str| { let u = format!("{}/bots/pair/local", url(&a)); async move {
        let mut c = tokio::process::Command::new("curl"); c.args(["-s", "-X", "POST", "-H", auth, "-d", body, "-w", "\n%{http_code}", &u]);
        let o = c.output().await.unwrap(); let t = String::from_utf8_lossy(&o.stdout).to_string(); let (b, code) = t.rsplit_once('\n').unwrap(); (code.to_string(), b.to_string()) } };
    assert_eq!(call("X-A: b", r#"{"op":"machine"}"#).await.0, "401");
    assert_eq!(call("Authorization: Bearer nope", r#"{"op":"machine"}"#).await.0, "401");
    let (code, body) = call("Authorization: Bearer token-ca", r#"{"op":"machine"}"#).await;
    let j: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(code, "200"); assert_eq!(j["code"].as_str().unwrap().len(), 8); assert!(j["address"].as_str().unwrap().starts_with("http://127.0.0.1:"));
    let (_, body) = call("Authorization: Bearer token-ca", r#"{"op":"requests"}"#).await;
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["requests"], json!([]));
}

#[test] fn pair_addresses_are_normalized() {
    assert_eq!(normalize("box.ts.net:3787").as_deref(), Some("http://box.ts.net:3787"));
    assert_eq!(normalize(" http://100.1.2.3:3787/ ").as_deref(), Some("http://100.1.2.3:3787"));
    assert_eq!(normalize("https://b.ts.net").as_deref(), Some("https://b.ts.net"));
    assert_eq!(normalize("box.t1.ts.net").as_deref(), Some("https://box.t1.ts.net"));
    assert!(normalize("").is_none() && normalize("http://x?y=1").is_none() && normalize("a b").is_none());
}

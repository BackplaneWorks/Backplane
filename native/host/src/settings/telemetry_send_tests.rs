//! The sender's tests: the wire against docs/telemetry.md and the receiver's schema (Backplane.Works telemetry.py), the planner, the outbox,
//! and the consent paths end to end against a loopback receiver that checks the HMAC the way the real one does.
use super::*;
use crate::settings::{telemetry as tc, telemetry_usage as tu};
use std::sync::Mutex as StdMutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
const ID: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";

fn view(day: &str, now: u64) -> View { View { reset: "0:".into(), ver: "0.15.3".into(), ch: "release".into(), os: "linux".into(), arch: "x64".into(), day: day.into(), now } }

// ---- the wire ----

#[test] fn the_install_event_has_exactly_the_documented_fields_in_order() {
    let st = plan(&Tm::default(), &view("2026-11-11", 100), ID, "");
    assert_eq!(st.queue.len(), 1); let mut e = st.queue[0].clone(); e.pow = "1a2b".into();
    let j = ev_json(&e); let keys: Vec<&str> = j.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["e", "day", "id", "share", "ver", "ch", "os", "arch", "pow"]);
    assert_eq!(j, json!({"e":"install","day":"2026-11-11","id":ID,"share":true,"ver":"0.15.3","ch":"release","os":"linux","arch":"x64","pow":"1a2b"}));
}
#[test] fn an_update_names_the_previous_version_and_active_has_no_prev() {
    let s0 = Tm { id: ID.into(), ver: "0.15.2".into(), reset: "0:".into(), ..Default::default() };
    let s1 = plan(&s0, &view("2026-11-11", 100), "unused", "2026-11-11");
    let kinds: Vec<&str> = s1.queue.iter().map(|e| e.e.as_str()).collect(); assert_eq!(kinds, ["update", "active"]);
    assert_eq!(ev_json(&s1.queue[0])["prev"], "0.15.2");
    let a = ev_json(&s1.queue[1]); assert!(a.get("prev").is_none() && a.get("pow").is_none()); assert_eq!(a["day"], "2026-11-11");
    for e in &s1.queue { assert!(clean(e), "{e:?}") }
}
#[test] fn the_envelope_is_v1_ts_nonce_events() {
    let b: Value = serde_json::from_str(&body(1_794_000_000, "0123456789abcdef", &[json!({"e":"active"})])).unwrap();
    let keys: Vec<&str> = b.as_object().unwrap().keys().map(String::as_str).collect(); assert_eq!(keys, ["v", "ts", "nonce", "events"]);
    assert_eq!((b["v"].as_u64(), b["ts"].as_u64()), (Some(1), Some(1_794_000_000)));
    let bad: Value = serde_json::from_str(&body(1, "NOTHEX", &[])).unwrap(); assert_eq!(bad["nonce"], "0000000000000000");
}
#[test] fn the_signature_is_the_receivers_hmac_of_ts_dot_body() {
    let h = headers("rel-1", KEY, 1700000000, "{}");
    let expect = format!("X-Backplane-Signature: sha256={}", hex::encode(crate::net::owner::hmac_sha256(&hex::decode(KEY).unwrap(), b"1700000000.{}")));
    assert_eq!(h, ["Content-Type: application/json".to_string(), "X-Backplane-Key: rel-1".into(), "X-Backplane-Timestamp: 1700000000".into(), expect]);
    assert!(!h.concat().contains(KEY), "the key itself is never sent");
}
#[test] fn the_usage_event_carries_twelve_fixed_counters() {
    let mut row = [0u64; 12]; row[0] = 2; row[11] = 1;
    let e = tu::event("2026-11-11", &row, ID, &view("2026-11-11", 1));
    let keys: Vec<&str> = e.as_object().unwrap().keys().map(String::as_str).collect(); assert_eq!(keys, ["e", "day", "share", "id", "ver", "ch", "os", "arch", "counts"]);
    let c: Vec<&str> = e["counts"].as_object().unwrap().keys().map(String::as_str).collect(); assert_eq!(c, tu::NAMES);
    assert_eq!((e["counts"]["threads_created"].as_u64(), e["counts"]["boards_ordered"].as_u64(), e["counts"]["user_messages"].as_u64()), (Some(2), Some(1), Some(0)));
}
#[test] fn validators_match_the_receivers_patterns() {
    assert!(uuid_ok(ID) && !uuid_ok("0f8fad5b-d9cb-569f-a165-70867728950e") && !uuid_ok("0f8fad5b-d9cb-469f-c165-70867728950e"));
    let mut b = [0u8; 16]; crate::plat::fs::random(&mut b).unwrap(); assert!(uuid_ok(&uuid(&b)));
    assert!(version_ok("0.15.3") && version_ok("0.16.0-nightly.20261111.1") && !version_ok("0.15") && !version_ok("12345.0.0"));
    assert!(day_ok("2026-11-11") && !day_ok("2026-1-11")); assert_eq!(day_of(1_794_355_200), "2026-11-11"); assert_eq!(day_of(951_782_400), "2000-02-29"); assert_eq!(day_of(0), "1970-01-01");
    assert_eq!(channel_of("0.15.3"), "release"); assert_eq!(channel_of("0.16.0-nightly.20261111.1"), "nightly");
    assert_eq!(retry_after("HTTP/1.1 429\r\nRetry-After: 7200\r\n"), 7200); assert_eq!(retry_after("HTTP/1.1 500\r\n"), 0);
    for s in [202, 400, 401, 403, 409, 413] { assert!(http_drop(s)) } for s in [0, 200, 301, 429, 500, 503] { assert!(!http_drop(s)) }
}
#[test] fn the_proof_of_work_meets_eighteen_bits_and_is_checked() {
    let p = pow(ID, POW_BITS); assert!(!p.is_empty() && pow_ok(ID, &p, POW_BITS)); assert!(!pow_ok(ID, "zz", POW_BITS) || pow_ok(ID, "zz", POW_BITS) == pow_ok(ID, "zz", 18));
    assert_eq!(pow("not-a-uuid", POW_BITS), "");
}
#[test] fn a_release_config_must_be_the_canonical_https_endpoint_and_a_release_key() {
    let c = |u: &str, k: &str, ch: &str| Cfg { url: u.into(), kid: k.into(), key: KEY.into(), channel: ch.into() };
    assert!(config_ok(false, &c(ENDPOINT, "rel-1", "release")) && config_ok(false, &c(ENDPOINT, "rel-1", "nightly")));
    assert!(!config_ok(false, &c("https://evil.example/api/v1/events", "rel-1", "release")) && !config_ok(false, &c(ENDPOINT, "dev-1", "release")) && !config_ok(false, &c(ENDPOINT, "rel-1", "dev")));
    assert!(!config_ok(false, &Cfg { key: "short".into(), ..c(ENDPOINT, "rel-1", "release") }), "a missing or malformed key closes the gate");
    assert!(config_ok(true, &c("http://127.0.0.1:4567/api/v1/events", "dev-1", "dev")) && !config_ok(true, &c("http://10.0.0.1:4567/api/v1/events", "dev-1", "dev")) && !config_ok(true, &c(ENDPOINT, "rel-1", "release")), "test mode is loopback only");
    let env = |m: &'static [(&'static str, &'static str)]| move |k: &str| m.iter().find(|(a, _)| *a == k).map(|(_, v)| v.to_string()).unwrap_or_default();
    assert!(gate(&env(&[]), false) && gate(&env(&[("BACKPLANE_SANDBOX", "0")]), false));
    for kill in [&[("BACKPLANE_TELEMETRY", "0")][..], &[("BACKPLANE_TELEMETRY", "off")], &[("DO_NOT_TRACK", "1")], &[("CI", "true")], &[("BACKPLANE_SANDBOX", "1")]] { assert!(!gate(&env(kill), false), "{kill:?}") }
    assert!(!gate(&env(&[]), true), "--sandbox");
    // a developer build bakes nothing, so it can never send whatever its environment says
    if option_env!("BACKPLANE_TELEMETRY_KEY").is_none() { assert!(cfg().map_or(true, |c| loopback(&c.url))) }
}

// ---- the planner and the outbox ----

#[test] fn at_most_one_daily_active_per_day() {
    let v = view("2026-11-11", 1000);
    let mut st = plan(&Tm::default(), &v, ID, "2026-11-11");
    for _ in 0..5 { st = plan(&st, &v, "x", "2026-11-11"); }
    assert_eq!(st.queue.iter().filter(|e| e.e == "active").count(), 1);
    // sent and acknowledged, then more actions the same day: still nothing new
    let sent_all = sent(&st, &st.queue.clone(), 202, 0, 1000); assert!(sent_all.queue.is_empty());
    let again = plan(&sent_all, &v, "x", "2026-11-11"); assert!(again.queue.iter().all(|e| e.e != "active"), "acknowledged today already");
    let next_day = plan(&again, &view("2026-11-12", 90_000), "x", "2026-11-12"); assert_eq!(next_day.queue.iter().filter(|e| e.e == "active").count(), 1);
    // an active event is kept once per day even if a stale copy of the queue comes back (dedupe)
    let mut dup = next_day.clone(); dup.queue.extend(next_day.queue.clone()); assert_eq!(plan(&dup, &view("2026-11-12", 90_000), "x", "").queue.len(), 1);
}
#[test] fn no_action_no_active_and_reset_rotates_the_id_only() {
    let v = view("2026-11-11", 1000);
    let st = plan(&Tm::default(), &v, ID, ""); assert!(st.queue.iter().all(|e| e.e != "active"), "idle, reconnects and keepalives count nothing");
    let other = "6ba7b810-9dad-41d1-80b4-00c04fd430c8";
    let r = plan(&st, &View { reset: "1:".into(), ..v.clone() }, other, "");
    assert_eq!(r.id, other); assert_eq!(r.queue.len(), 1); assert_eq!((r.queue[0].e.as_str(), r.queue[0].id.as_str()), ("install", other), "the old id's queue went; the new one registers");
}
#[test] fn failures_back_off_and_only_acknowledged_or_permanent_answers_consume() {
    let st = plan(&Tm::default(), &view("2026-11-11", 1000), ID, "2026-11-11");
    let q = st.queue.clone();
    let failed = sent(&st, &q, 503, 0, 1000); assert_eq!(failed.queue, q); assert_eq!(failed.next, 4600);
    let limited = sent(&st, &q, 429, 7200, 1000); assert_eq!(limited.next, 8200);
    let gone = sent(&st, &q, 400, 0, 1000); assert!(gone.queue.is_empty(), "a schema rejection drops its batch");
}
#[test] fn the_queue_expires_after_a_week_and_caps_at_two_hundred() {
    let mut st = Tm { id: ID.into(), reset: "0:".into(), ver: "0.15.3".into(), ..Default::default() };
    for d in 0..250u64 { st.queue.push(Ev { e: "active".into(), day: day_of(d * 86400), share: true, id: ID.into(), ver: "0.15.3".into(), ch: "release".into(), os: "linux".into(), arch: "x64".into(), made: 10_000_000 - 250 + d, ..Default::default() }); }
    assert_eq!(plan(&st, &view("2026-11-11", 10_000_000), "x", "").queue.len(), 200);
    st.queue[0].made = 1; assert!(!plan(&st, &view("2026-11-11", 10_000_000), "x", "").queue.contains(&st.queue[0]));
    // old anonymous or held entries from an earlier version are never released
    let old = Tm { id: ID.into(), reset: "0:".into(), queue: vec![Ev { e: "install".into(), share: false, made: 9_999_999, ..Default::default() }, Ev { e: "update".into(), share: true, held: true, id: ID.into(), made: 9_999_999, ..Default::default() }], ..Default::default() };
    assert!(plan(&old, &view("2026-11-11", 10_000_000), "x", "").queue.is_empty());
}
#[test] fn a_batch_is_at_most_fifty_events_and_sixteen_kib() {
    let st = Tm { id: ID.into(), queue: (0..80).map(|d| Ev { e: "active".into(), day: day_of(d * 86400), share: true, id: ID.into(), ver: "0.15.3".into(), ch: "release".into(), os: "linux".into(), arch: "x64".into(), ..Default::default() }).collect(), ..Default::default() };
    let b = batch(&st.queue); assert_eq!(b.len(), 50); assert!(body(1, "0123456789abcdef", &b.iter().map(ev_json).collect::<Vec<_>>()).len() <= 16 * 1024);
}
#[test] fn an_old_state_file_from_0_14_still_reads() {
    let t = r#"{"id":"0f8fad5b-d9cb-469f-a165-70867728950e","ver":"0.14.1","today":"2026-11-01","share":true,"reset":"0","queue":[{"e":"active","day":"2026-11-01","share":true,"id":"0f8fad5b-d9cb-469f-a165-70867728950e","ver":"0.14.1","prev":"","ch":"release","os":"linux","arch":"x64","pow":"","made":5,"held":false}],"next":0,"busy":0,"seen":true}"#;
    let st: Tm = serde_json::from_str(t).unwrap(); assert_eq!(st.queue.len(), 1); assert_eq!(st.ver, "0.14.1");
}

// ---- end to end: a hub, a loopback receiver ----

static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// A receiver on 127.0.0.1 that checks the signature and answers `status`; keeps each accepted body.
struct Recv { port: u16, bodies: Arc<StdMutex<Vec<Value>>>, status: Arc<AtomicU64> }
async fn receiver() -> Recv {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    let (bodies, status) = (Arc::new(StdMutex::new(vec![])), Arc::new(AtomicU64::new(202)));
    let (b2, s2) = (bodies.clone(), status.clone());
    tokio::spawn(async move { loop {
        let Ok((mut sock, _)) = l.accept().await else { return };
        let (b3, s3) = (b2.clone(), s2.clone());
        tokio::spawn(async move {
            let mut buf = vec![]; let mut chunk = [0u8; 4096];
            let (head, body) = loop {
                let n = sock.read(&mut chunk).await.unwrap_or(0); if n == 0 { return } buf.extend_from_slice(&chunk[..n]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..i]).to_string();
                    let len: usize = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
                    while buf.len() < i + 4 + len { let n = sock.read(&mut chunk).await.unwrap_or(0); if n == 0 { break } buf.extend_from_slice(&chunk[..n]); }
                    break (head, String::from_utf8_lossy(&buf[i + 4..]).to_string());
                }
            };
            let h = |k: &str| head.lines().find_map(|l| { let (a, b) = l.split_once(':')?; a.trim().eq_ignore_ascii_case(k).then(|| b.trim().to_string()) }).unwrap_or_default();
            let ts: u64 = h("X-Backplane-Timestamp").parse().unwrap_or(0);
            let genuine = h("X-Backplane-Key") == "dev-test" && crate::net::sign::check(KEY, now_ms() / 1000, ts, &body, &h("X-Backplane-Signature")) && h("Content-Type") == "application/json";
            let code = if !genuine { 401 } else { s3.load(Relaxed) as u16 };
            if genuine && code == 202 { b3.lock().unwrap().push(serde_json::from_str(&body).unwrap_or(Value::Null)); }
            let _ = sock.write_all(format!("HTTP/1.1 {code} X\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").as_bytes()).await;
        });
    } });
    Recv { port, bodies, status }
}
fn tcfg(port: u16) -> Cfg { Cfg { url: format!("http://127.0.0.1:{port}/api/v1/events"), kid: "dev-test".into(), key: KEY.into(), channel: "dev".into() } }
async fn hub(name: &str) -> Arc<Hub> {
    let home = std::env::temp_dir().join(format!("bp-tm-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
    let h = Hub::start(crate::hub::Cfg { home, version: "0.15.3".into(), commit: "c".into(), listen: "127.0.0.1:0".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(crate::presenter::Canned { reply: json!({}) })).await.unwrap();
    h.register_rpc(Arc::new(tc::TelemetryRpc)); observe(&h); TEST_CAPABLE.store(true, Relaxed); tu::clear(); HUMAN.store(0, Relaxed);
    h
}
async fn set(h: &Arc<Hub>, kv: &[(&str, &str)]) { let evs: Vec<Value> = kv.iter().map(|(k, v)| json!({"$":"SettingSet","key":k,"value":v,"at":1})).collect(); h.commit_many(&evs).await.unwrap(); }
async fn call(h: &Arc<Hub>, m: &str, p: Value) -> Value { h.rpc_for(m).unwrap().call(h, &crate::api::Caller { gen: String::new(), client: 0 }, &json!(1), m, &p).await.remove(0) }
async fn project(h: &Arc<Hub>) { h.commit(&json!({"$":"ProjectCreated","id":"p1","title":"Secret Board Co","root":"/home/someone/secret-path","at":1})).await.unwrap(); }
fn sent_kinds(r: &Recv) -> Vec<String> { r.bodies.lock().unwrap().iter().flat_map(|b| b["events"].as_array().cloned().unwrap_or_default()).map(|e| e["e"].as_str().unwrap_or("").to_string()).collect() }

#[tokio::test] async fn a_new_install_that_keeps_it_on_sends_once_setup_finishes() {
    let _g = SERIAL.lock().await; let r = receiver().await; let h = hub("new-on").await; let c = tcfg(r.port);
    // setup is on screen, On preselected: nothing yet, not even a state file
    set(&h, &[("onboard.step", "welcome")]).await;
    job(&h, &c).await; assert!(r.bodies.lock().unwrap().is_empty()); assert!(!dir(&h.cfg.home).join("state.json").exists());
    assert!(tc::offer(&h).await.is_null(), "a new install gets setup's step, not the notice");
    // finishing setup (present/onboard.rs `end`) records the disclosure, then marks setup done
    let ob = crate::present::onboard::act(&crate::state::State::new(), "h", &json!({"rid":1,"drafts":{"@ob.step":"done"}}), &json!({}), &crate::present::onboard::Client { phone: false, os: "linux".into(), launcher: false }, "/h", "ob-end", "").unwrap();
    let first: Value = serde_json::from_str(ob.cmds[0]["text"].as_str().unwrap()).unwrap(); assert_eq!((first["m"].as_str(), first["p"]["action"].as_str()), (Some("telemetry.notice.act"), Some("setup")));
    call(&h, "telemetry.notice.act", first["p"].clone()).await; set(&h, &[("onboard.done", "1"), ("onboard.step", "")]).await;
    job(&h, &c).await;
    assert_eq!(sent_kinds(&r), ["install"]); let ev = &r.bodies.lock().unwrap()[0]["events"][0];
    assert!(pow_ok(ev["id"].as_str().unwrap(), ev["pow"].as_str().unwrap(), POW_BITS), "registered with its proof of work");
    assert!(tc::offer(&h).await.is_null(), "no notice after setup showed it");
}
#[tokio::test] async fn a_new_install_that_turns_it_off_in_setup_sends_nothing() {
    let _g = SERIAL.lock().await; let r = receiver().await; let h = hub("new-off").await; let c = tcfg(r.port);
    set(&h, &[("onboard.step", "welcome")]).await;
    let st = crate::state::State::new();
    let ob = crate::present::onboard::act(&st, "h", &json!({"rid":1}), &json!({}), &crate::present::onboard::Client { phone: false, os: "linux".into(), launcher: false }, "/h", "ob-usage", "off").unwrap();
    for cmd in &ob.cmds { let t: Value = serde_json::from_str(cmd["text"].as_str().unwrap()).unwrap(); let (k, v) = (t["p"]["key"].as_str().unwrap(), t["p"]["value"].as_str().unwrap());
        if tc::reserved(k) { tc::legacy(&h, &json!(1), k, v).await; } else { set(&h, &[(k, v)]).await; } }
    call(&h, "telemetry.notice.act", json!({"action":"setup"})).await; set(&h, &[("onboard.done", "1")]).await;
    for _ in 0..3 { job(&h, &c).await; }
    assert!(r.bodies.lock().unwrap().is_empty()); assert!(!dir(&h.cfg.home).join("state.json").exists()); assert!(tc::offer(&h).await.is_null());
    assert_eq!(h.setting("telemetry.share").await.as_deref(), Some("off"));
}
#[tokio::test] async fn an_existing_install_upgraded_with_no_answer_shows_the_notice_then_sends() {
    let _g = SERIAL.lock().await; let r = receiver().await; let h = hub("existing").await; let c = tcfg(r.port);
    set(&h, &[("onboard.done", "1")]).await; project(&h).await;
    job(&h, &c).await; assert!(r.bodies.lock().unwrap().is_empty(), "nothing before the notice is drawn");
    let o = tc::offer(&h).await; assert_eq!(o["token"], tc::NOTICE_TOKEN); assert_eq!((o["off"].as_str(), o["settings"].as_str()), (Some("Turn off"), Some("Settings")));
    assert!(o["text"].as_str().unwrap().contains("It is on"));
    call(&h, "telemetry.notice.act", json!({"token": tc::NOTICE_TOKEN, "action": "shown"})).await;
    assert!(tc::offer(&h).await.is_object(), "it stays up until closed");
    job(&h, &c).await; assert_eq!(sent_kinds(&r), ["install"]);
    call(&h, "telemetry.notice.act", json!({"token": tc::NOTICE_TOKEN, "action": "close"})).await; assert!(tc::offer(&h).await.is_null());
}
#[tokio::test] async fn an_existing_install_with_an_explicit_off_stays_off_and_sends_nothing() {
    let _g = SERIAL.lock().await; let r = receiver().await; let h = hub("existing-off").await; let c = tcfg(r.port);
    set(&h, &[("onboard.done", "1"), ("telemetry.share", "off"), ("telemetry.asked", "1"), ("telemetry.usage.asked", "0")]).await; project(&h).await;
    assert!(tc::offer(&h).await.is_null(), "no notice: Off is the answer");
    call(&h, "telemetry.notice.act", json!({"token": tc::NOTICE_TOKEN, "action": "shown"})).await; assert_eq!(h.setting(tc::NOTICE).await.unwrap_or_default(), "", "a notice that is not out records nothing");
    call(&h, "telemetry.notice.act", json!({"action": "setup"})).await; // even a disclosure recorded later never flips an Off
    for _ in 0..3 { job(&h, &c).await; }
    assert!(r.bodies.lock().unwrap().is_empty()); assert!(!dir(&h.cfg.home).join("state.json").exists());
    // an older decline in setup is the same
    let h2 = hub("existing-declined").await; set(&h2, &[("onboard.done", "1"), ("onboard.usage", "off"), (tc::NOTICE, "shown")]).await;
    job(&h2, &c).await; assert!(r.bodies.lock().unwrap().is_empty());
}
#[tokio::test] async fn the_notices_turn_off_stops_everything_and_clears_the_outbox() {
    let _g = SERIAL.lock().await; let r = receiver().await; let h = hub("notice-off").await; let c = tcfg(r.port);
    set(&h, &[("onboard.done", "1")]).await; project(&h).await;
    call(&h, "telemetry.notice.act", json!({"token": tc::NOTICE_TOKEN, "action": "shown"})).await;
    r.status.store(503, Relaxed); job(&h, &c).await; assert!(dir(&h.cfg.home).join("state.json").exists(), "the install waits in the outbox");
    call(&h, "telemetry.notice.act", json!({"token": tc::NOTICE_TOKEN, "action": "off"})).await;
    assert_eq!(h.setting("telemetry.share").await.as_deref(), Some("off")); assert!(tc::offer(&h).await.is_null());
    assert!(!dir(&h.cfg.home).join("state.json").exists(), "Off forgets the id and the outbox at once");
    r.status.store(202, Relaxed); job(&h, &c).await; assert!(r.bodies.lock().unwrap().is_empty());
}
#[tokio::test] async fn the_outbox_survives_a_restart_and_is_replayed_once() {
    let _g = SERIAL.lock().await; let r = receiver().await; let h = hub("replay").await; let c = tcfg(r.port);
    set(&h, &[("onboard.done", "1"), (tc::NOTICE, "chosen")]).await;
    r.status.store(503, Relaxed); ALLOWED.store(true, Relaxed); human(); job(&h, &c).await;
    let st = read_state(&h.cfg.home); assert_eq!(st.queue.iter().map(|e| e.e.as_str()).collect::<Vec<_>>(), ["install", "active"]); assert!(st.next > now_ms() / 1000);
    // a receiver down: nothing sent again before the backoff
    r.status.store(202, Relaxed); job(&h, &c).await; assert!(r.bodies.lock().unwrap().is_empty());
    // "restart": the in-memory state goes; the file is all that is left (the backoff is let lapse)
    let mut st = read_state(&h.cfg.home); st.next = 0; write_private(&h.cfg.home, "state.json", &serde_json::to_string(&st).unwrap()).unwrap(); HUMAN.store(0, Relaxed);
    job(&h, &c).await; job(&h, &c).await;
    assert_eq!(sent_kinds(&r), ["install", "active"], "each queued event once, the outbox's own"); assert!(read_state(&h.cfg.home).queue.is_empty());
    let id = read_state(&h.cfg.home).id; assert!(r.bodies.lock().unwrap()[0]["events"].as_array().unwrap().iter().all(|e| e["id"] == id.as_str()));
}
#[tokio::test] async fn nothing_but_the_documented_fields_and_numbers_ever_leaves() {
    let _g = SERIAL.lock().await; let r = receiver().await; let h = hub("content").await; let c = tcfg(r.port);
    set(&h, &[("onboard.done", "1"), (tc::NOTICE, "shown")]).await;
    job(&h, &c).await; // registered
    project(&h).await;
    h.commit_many(&[json!({"$":"ThreadCreated","id":"t-secret","project":"p1","title":"My Secret Thread Title","at":1}),
        json!({"$":"MessagePosted","thread":"t-secret","msg":"m1","role":"user","text":"PROMPT-CONTENT-sk-ant-api03-SECRETKEY /home/someone/secret-path/board.kicad_pcb","at":1}),
        json!({"$":"MessagePosted","thread":"t-secret","msg":"m2","role":"assistant","text":"ASSISTANT-CONTENT","at":1}),
        json!({"$":"MessagePosted","thread":"L1~t9","msg":"m3","role":"user","text":"mirrored","at":1}),
        json!({"$":"MailPosted","id":"mail-1","conv":"c","from":"bot:Secret Bot Name","to":"thread:t-secret","kind":"note","ref":"","want":"","head":"SECRET-HEAD","size":3,"at":1}),
        json!({"$":"MailPosted","id":"mail-1","conv":"c","from":"bot:Secret Bot Name","to":"thread:t-secret","kind":"note","ref":"","want":"","head":"SECRET-HEAD","size":3,"at":1}),
        json!({"$":"BotWoke","bot":"Secret Bot Name","hop":0,"at":1}),
        json!({"$":"TurnUsage","thread":"t-secret","provider":"claude","model":"claude-secret-model","key":"k","input":10,"cread":20,"cwrite":30,"output":40,"reasoning":5,"cost":99,"known":"irwo","basis":"b","at":1})]).await.unwrap();
    tu::bump(tu::BOARDS_CREATED, 1); tu::bump(tu::BOARDS_ORDERED, 1);
    human(); job(&h, &c).await;
    let all: String = r.bodies.lock().unwrap().iter().map(|b| b.to_string()).collect();
    for bad in ["Secret", "SECRET", "CONTENT", "secret-path", "kicad_pcb", "sk-ant", "claude", "t-secret", "mail-1", "p1", "/home", KEY, "cost", "reasoning", "mirrored"] { assert!(!all.contains(bad), "{bad} leaked: {all}") }
    let allowed_ev: std::collections::HashSet<&str> = ["e", "day", "id", "share", "ver", "prev", "ch", "os", "arch", "pow", "counts"].into();
    for b in r.bodies.lock().unwrap().iter() {
        assert_eq!(b.as_object().unwrap().keys().collect::<Vec<_>>(), ["v", "ts", "nonce", "events"]);
        for e in b["events"].as_array().unwrap() { for k in e.as_object().unwrap().keys() { assert!(allowed_ev.contains(k.as_str()), "{k}") } }
    }
    let usage: Vec<Value> = r.bodies.lock().unwrap().iter().flat_map(|b| b["events"].as_array().cloned().unwrap_or_default()).filter(|e| e["e"] == "usage").collect();
    assert_eq!(usage.len(), 1); let n = &usage[0]["counts"];
    assert_eq!([&n["threads_created"], &n["user_messages"], &n["assistant_messages"], &n["bot_messages"], &n["bot_wakes"], &n["input_tokens"], &n["cache_read_tokens"], &n["cache_write_tokens"], &n["output_tokens"], &n["usage_turns"], &n["boards_created"], &n["boards_ordered"]].map(|v| v.as_u64().unwrap()),
        [1, 1, 1, 1, 1, 10, 20, 30, 40, 1, 1, 1], "one mail id counts once; a mirrored thread counts nothing");
    // an unchanged snapshot is not sent again
    job(&h, &c).await; assert_eq!(r.bodies.lock().unwrap().iter().flat_map(|b| b["events"].as_array().cloned().unwrap_or_default()).filter(|e| e["e"] == "usage").count(), 1);
}

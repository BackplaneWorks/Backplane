//! The phone push gate on a real hub with the real tools, and a carrier that only records. Stand-ins only: no agent runs and nothing leaves. The pure rules are at the end.
use super::*;
use super::triage::{settle, turn_said, uber};
type Res = (bool, Value);
use crate::push::{self, apns::{self, Frame}, Book, Carrier, Item, Sent};
use crate::bots::world::Device;

#[derive(Default)] struct Fake { sent: StdMutex<Vec<(String, Frame)>>, dead: bool }
impl Carrier for Fake {
    fn send(self: Arc<Self>, _hub: Arc<Hub>, d: Device, f: Frame) -> futures_util::future::BoxFuture<'static, Sent> {
        self.sent.lock().unwrap().push((d.token, f)); let r = if self.dead { Sent::Dead } else { Sent::Ok }; Box::pin(async move { r })
    }
}
impl Fake {
    fn alerts(&self) -> Vec<Value> { self.sent.lock().unwrap().iter().filter(|x| x.1.push_type == "alert").map(|x| x.1.payload.clone()).collect() }
    fn clears(&self) -> Vec<Value> { self.sent.lock().unwrap().iter().filter(|x| x.1.push_type == "background").map(|x| x.1.payload.clone()).collect() }
    async fn wait(&self, n: usize) -> Vec<Value> { for _ in 0..2000 { if self.alerts().len() >= n { break } tokio::time::sleep(std::time::Duration::from_millis(30)).await } self.alerts() }
}
struct S { hub: Arc<Hub>, bots: Arc<Bots>, rec: Arc<Rec>, bub: String, chat: String, fake: Arc<Fake>, push: Arc<push::Push> }
async fn set(hub: &Arc<Hub>, k: &str, v: &str) { hub.commit(&json!({"$":"SettingSet","key":k,"value":v,"at":now_s()})).await.unwrap() ; }
async fn device(hub: &Arc<Hub>, token: &str) { hub.commit(&json!({"$":"DeviceRegistered","token":token,"platform":"ios","kind":"alert","thread":"","env":"sandbox","bundle":"dev.backplane.mobile","at":now_s()})).await.unwrap(); }
async fn setup_with(name: &str, fake: Fake) -> S {
    let (hub, bots, rec, bub, chat) = uber(&format!("push-{name}")).await;
    set(&hub, push::QUIET, "off").await; set(&hub, push::BURST, "0").await; device(&hub, "tok1").await;
    let fake = Arc::new(fake); let p = push::start_with(&hub, bots.clone(), fake.clone());
    S { hub, bots, rec, bub, chat, fake, push: p }
}
async fn setup(name: &str) -> S { setup_with(name, Fake::default()).await }
/// a new message from the person: each call below is then a turn of its own
async fn person(s: &S, n: u64) { s.hub.commit(&json!({"$":"MessagePosted","thread":"elsewhere","msg":format!("m{n}"),"role":"user","text":"hi","at":now_s()})).await.unwrap(); s.hub.commit(&json!({"$":"MessagePosted","thread":s.chat,"msg":format!("p{n}"),"role":"user","text":format!("go {n}"),"at":now_s() + n})).await.unwrap(); }
async fn notify(s: &S, reason: &str, text: &str, at: u64) -> Res { s.bots.tool_call_at(&s.chat, "notify_person", &json!({"reason":reason,"text":text}), at).await }
async fn order(s: &S, how: &str, bot: &str) { s.hub.commit(&json!({"$":"PushOrdered","id":format!("x{how}{bot}"),"how":how,"bot":bot,"reason":"needs_you","thread":"t1","ask":"","text":"forged","at":now_s()})).await.unwrap(); }

#[tokio::test] async fn the_uberbots_notify_reaches_the_phone_once_with_its_link() {
    let s = setup("once").await;
    let r = notify(&s, "needs_you", "Approve the deploy?\nmore detail", now_s()).await; assert!(r.0, "{r:?}");
    let a = s.fake.wait(1).await; assert_eq!(a.len(), 1, "{a:?}");
    assert_eq!((a[0]["aps"]["alert"]["body"].as_str(), a[0]["thread"].as_str(), a[0]["aps"]["thread-id"].as_str()), (Some("Approve the deploy?"), Some(s.chat.as_str()), Some(s.chat.as_str())), "plain and short, linked to and grouped by the thread");
    assert_eq!(a[0]["aps"]["badge"], 1);
    until(&s.hub, |l| l.iter().any(|e| e["$"] == "PushSent")).await; // recorded once the carrier answered
    let sent = log(&s.hub).into_iter().find(|e| e["$"] == "PushSent").expect("what a socket hears"); assert_eq!((sent["kind"].as_str(), sent["sent"].as_u64()), (Some("needs_you"), Some(1)));
}

#[tokio::test] async fn a_link_names_the_exact_thread_and_approval_and_refuses_what_is_not_there() {
    let s = setup("link").await;
    s.hub.commit(&json!({"$":"ThreadCreated","id":"tw","project":"","title":"Worker","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap();
    s.hub.commit(&json!({"$":"AskOpened","id":"a1","thread":"tw","tool":"Bash","detail":"rm x","at":2})).await.unwrap();
    let r = s.bots.tool_call(&s.chat, "notify_person", &json!({"reason":"needs_you","text":"The worker needs your approval","ask":"a1"})).await; assert!(r.0, "{r:?}");
    let a = s.fake.wait(1).await; assert_eq!((a[0]["thread"].as_str(), a[0]["ask"].as_str()), (Some("tw"), Some("a1")), "the thread is found from the approval");
    for (t, ask) in [("nope", ""), ("", "gone"), ("tw", "gone")] {
        let r = s.bots.tool_call(&s.chat, "notify_person", &json!({"reason":"needs_you","text":"another thing","thread":t,"ask":ask})).await; assert!(!r.0, "{t} {ask} {r:?}");
    }
}

#[tokio::test] async fn every_other_path_is_refused() {
    let s = setup("refused").await;
    // another bot, by the real tool: it reaches the uberbot and no phone
    let ada = bot(&s.hub, "ada").await; let ada_th = s.bots.world().await.bot(&ada).unwrap().thread.clone();
    let r = s.bots.tool_call(&ada_th, "notify_person", &json!({"reason":"needs_you","text":"ada wants a push"})).await; assert!(r.0, "{r:?}");
    assert!(r.1.to_string().contains("no direct person alert"), "{r:?}");
    // (a thread's alert record is a push of its own unless a focused desk shows the thread: a_threads_alert_reaches_the_phone_unless_a_focused_desk_shows_it)
    // a routine's turn that is not a reminder, and a reminder another bot planned, even after a toggle
    let past = 1_000_000u64;
    for e in [json!({"$":"RoutineSet","id":"w1","bot":ada,"name":"work","cron":"","prompt":"p","on":true,"at":past}), json!({"$":"RoutinePlan","id":"w1","once":past + 10,"tz":"person","kind":"","at":past}),
        json!({"$":"RoutineSet","id":"r1","bot":ada,"name":"nag","cron":"","prompt":"nag them","on":true,"at":past}), json!({"$":"RoutinePlan","id":"r1","once":past + 10,"tz":"person","kind":"remind","by":ada,"at":past}),
        json!({"$":"RoutineSet","id":"r1","bot":ada,"name":"nag","cron":"","prompt":"nag them","on":true,"at":past + 1})] { s.hub.commit(&e).await.unwrap(); }
    assert_eq!(s.bots.world().await.routines["r1"].by, ada, "a toggle keeps who planned it");
    assert_eq!(s.bots.tick_at(past + 20).await, 2);
    // an order made by anything but the two sources, committed straight to the log
    for (how, b) in [("notify", ada.as_str()), ("notify", ""), ("turn", s.bub.as_str()), ("webhook", s.bub.as_str()), ("mailroom", s.bub.as_str()), ("reminder", ada.as_str()), ("", "")] { order(&s, how, b).await }
    // a client writing the book
    let h = crate::settings::set::SettingSet { agents: None };
    let r = crate::api::RpcHandler::call(&h, &s.hub, &caller(), &json!(7), "setting.set", &json!({"key":push::BOOK,"value":"{}"})).await.remove(0); assert_eq!(r["ok"], false, "{r:?}");
    settle().await; settle().await;
    assert!(s.fake.sent.lock().unwrap().is_empty(), "nothing reached a phone: {:?}", s.fake.sent.lock().unwrap());
    assert!(!log(&s.hub).iter().any(|e| e["$"] == "PushSent"), "and no socket heard one either");
    // the gate's opposite: the same hub still delivers the uberbot's
    person(&s, 5).await; let r = notify(&s, "needs_you", "the real one", now_s() + 10).await; assert!(r.0, "{r:?}"); assert_eq!(s.fake.wait(1).await.len(), 1);
}

#[test] fn only_the_gate_can_build_an_order() {
    // a new path cannot quietly mint one: only these files may name the event or a source
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) { for e in std::fs::read_dir(d).unwrap().flatten() { let p = e.path(); if p.is_dir() { walk(&p, out) } else if p.extension().map_or(false, |x| x == "rs") { out.push(p) } } }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"); let mut files = vec![]; walk(&root, &mut files);
    // import014: the hub's own one notice that a 0.14 import ended (Source::import_done, admitted only as how "import" with no bot)
    let allowed = ["push/mod.rs", "bots/tools.rs", "bots/routines.rs", "bots/mail/escalate.rs", "state/mod.rs", "bots/tests/push.rs", "import014/mod.rs", "import014/tests.rs"];
    for f in files {
        let rel = f.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
        if allowed.contains(&rel.as_str()) { continue }
        let t = std::fs::read_to_string(&f).unwrap();
        assert!(!t.contains("PushOrdered") && !t.contains("Source::notify") && !t.contains("Source::reminder"), "{rel} names the push gate");
    }
}

#[tokio::test] async fn the_persons_reminder_and_the_uberbots_remind_person_push() {
    let s = setup("remind").await; let past = 1_000_000u64;
    let ada = bot(&s.hub, "ada").await;
    for e in [json!({"$":"RoutineSet","id":"mine","bot":ada,"name":"water","cron":"","prompt":"drink water","on":true,"at":past}), json!({"$":"RoutinePlan","id":"mine","once":past + 10,"tz":"person","kind":"remind","at":past})] { s.hub.commit(&e).await.unwrap(); }
    assert_eq!(s.bots.tick_at(past + 20).await, 1);
    let a = s.fake.wait(1).await; assert_eq!(a[0]["aps"]["alert"]["body"], "drink water", "the person's own, on any bot");
    let r = s.bots.tool_call(&s.chat, "remind_person", &json!({"text":"call the fab","at":"in 20m"})).await; assert!(r.0, "{r:?}");
    assert!(s.bots.world().await.routines.values().any(|r| r.by == s.bub && r.kind == "remind"), "the uberbot's name is on it");
    assert_eq!(s.bots.tick_at(now_s() + 1300).await, 1);
    let a = s.fake.wait(2).await; assert_eq!(a[1]["aps"]["alert"]["body"], "call the fab");
}

#[tokio::test] async fn a_repeat_is_refused_against_the_told_records() {
    let s = setup("repeat").await; let now = now_s();
    set(&s.hub, "told.k1", &json!({"key":"k","text":"Bub: Approve the deploy to staging","channel":"alert","bot":"","at":now - 60}).to_string()).await;
    let r = notify(&s, "needs_you", "Approve the deploy to staging", now).await; assert!(!r.0 && r.1.to_string().contains("already told"), "{r:?}");
    settle().await; assert!(s.fake.alerts().is_empty());
    set(&s.hub, "told.k2", &json!({"key":"k2","text":"Bub: The nightly report is ready","channel":"alert","bot":"","at":now - 2 * 86400}).to_string()).await;
    let r = notify(&s, "needs_you", "The nightly report is ready", now).await; assert!(r.0, "older than a day is no repeat: {r:?}");
    assert_eq!(s.fake.wait(1).await.len(), 1);
}

#[tokio::test] async fn quiet_hours_hold_until_they_end_and_a_needs_you_passes_only_if_allowed() {
    let s = setup("quiet").await;
    set(&s.hub, push::QUIET, "00:00-23:59").await; settle().await;
    s.hub.commit(&json!({"$":"ThreadCreated","id":"tw","project":"","title":"Worker","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap();
    // about a worker thread: the person writing in the uberbot's thread below deals with what is about that thread only
    let r = s.bots.tool_call_at(&s.chat, "notify_person", &json!({"reason":"needs_you","text":"Held while quiet","thread":"tw"}), now_s()).await; assert!(r.0, "{r:?}");
    settle().await; assert!(s.fake.alerts().is_empty(), "held");
    // the worker saves the book after it routes the order: wait for it rather than a fixed pause (a loaded machine is slower than settle)
    let mut held = 0; for _ in 0..2000 { held = s.hub.setting(push::BOOK).await.and_then(|v| serde_json::from_str::<Book>(&v).ok()).map_or(0, |b| b.held.len()); if held == 1 { break } tokio::time::sleep(std::time::Duration::from_millis(30)).await }
    assert_eq!(held, 1, "and it survives a restart in the book");
    set(&s.hub, push::QUIET_URGENT, "on").await; settle().await;
    person(&s, 1).await;
    let r = notify(&s, "needs_you", "This one passes", now_s() + 5).await; assert!(r.0, "{r:?}");
    let a = s.fake.wait(1).await; assert_eq!(a[0]["aps"]["alert"]["body"], "This one passes");
    set(&s.hub, push::QUIET, "off").await;
    assert_eq!(s.fake.wait(2).await.len(), 2, "quiet off lets the held one go");
}

#[tokio::test] async fn a_burst_collapses_into_one() {
    let s = setup("burst").await; set(&s.hub, push::BURST, "5").await; settle().await; // 5 s: wide enough that a loaded machine still sends the four inside it
    // about a worker thread, so the person's messages in the uberbot's thread (one per turn) deal with none of them
    s.hub.commit(&json!({"$":"ThreadCreated","id":"tw","project":"","title":"Worker","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap();
    for (i, t) in ["first thing", "second thing", "third thing", "fourth thing"].iter().enumerate() { person(&s, 10 * i as u64 + 5).await; let r = s.bots.tool_call_at(&s.chat, "notify_person", &json!({"reason":"needs_you","text":t,"thread":"tw"}), now_s() + 10 * (i as u64 + 1)).await; assert!(r.0, "{r:?}") }
    let a = s.fake.wait(2).await; assert_eq!(a.len(), 2, "the first, then one for the rest: {a:?}");
    assert_eq!(a[1]["aps"]["alert"]["body"], "3 of 3 need you");
    assert_eq!(a[1]["aps"]["badge"], 4, "the badge still counts each");
}

#[tokio::test] async fn a_muted_device_hears_only_what_the_person_let_through() {
    let s = setup("mute").await; device(&s.hub, "tok2").await;
    s.hub.commit(&json!({"$":"DevicePrefs","token":"tok1","mute":true,"at":now_s()})).await.unwrap();
    let r = notify(&s, "needs_you", "only the other phone", now_s()).await; assert!(r.0, "{r:?}");
    s.fake.wait(1).await; settle().await;
    assert_eq!(s.fake.sent.lock().unwrap().iter().map(|x| x.0.clone()).collect::<Vec<_>>(), vec!["tok2".to_string()]);
    set(&s.hub, push::QUIET_URGENT, "on").await; settle().await; person(&s, 1).await;
    let r = notify(&s, "needs_you", "both phones", now_s() + 5).await; assert!(r.0, "{r:?}");
    assert_eq!(s.fake.wait(3).await.len(), 3);
}

#[tokio::test] async fn dealing_with_it_clears_every_device_and_the_badge() {
    let s = setup("clear").await; device(&s.hub, "tok2").await;
    s.hub.commit(&json!({"$":"ThreadCreated","id":"tw","project":"","title":"Worker","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap();
    s.hub.commit(&json!({"$":"AskOpened","id":"a1","thread":"tw","tool":"Bash","detail":"rm x","at":2})).await.unwrap();
    let r = s.bots.tool_call(&s.chat, "notify_person", &json!({"reason":"needs_you","text":"Approve the rm","ask":"a1"})).await; assert!(r.0, "{r:?}");
    let a = s.fake.wait(2).await; let key = a[0]["key"].as_str().unwrap().to_string(); assert_eq!(a[0]["aps"]["badge"], 1, "the ask counts once, not with its item too");
    s.push.seen("tw"); settle().await; assert!(s.fake.clears().is_empty(), "opening an approval is not answering it");
    s.hub.commit(&json!({"$":"AskClosed","id":"a1","answer":"accept","at":3})).await.unwrap();
    for _ in 0..300 { if s.fake.clears().len() >= 2 { break } tokio::time::sleep(std::time::Duration::from_millis(30)).await }
    let c = s.fake.clears(); assert_eq!(c.len(), 2, "both phones: {c:?}"); assert_eq!(c[0], json!({"aps":{"content-available":1,"badge":0},"clear":[key]}));
    until(&s.hub, |l| l.iter().any(|e| e["$"] == "PushCleared")).await; // a socket hears it too
    // opening the thread on any device, and the person writing in it
    person(&s, 1).await; let r = notify(&s, "needs_you", "Second thing to look at", now_s() + 5).await; assert!(r.0, "{r:?}"); s.fake.wait(4).await;
    s.push.seen(&s.chat);
    for _ in 0..300 { if s.fake.clears().len() >= 4 { break } tokio::time::sleep(std::time::Duration::from_millis(30)).await }
    assert_eq!(s.fake.clears().len(), 4, "push.seen clears");
}

#[tokio::test] async fn a_desk_looking_at_a_thread_and_push_clear_take_items_off_the_badge() {
    let s = setup("desk").await; crate::settings::alerts::start(&s.hub);
    let rpc = |m: &'static str, p: Value| { let hub = s.hub.clone(); async move { let h = hub.rpc_for(m).expect("handler"); h.call(&hub, &crate::api::Caller { gen: "1".into(), client: 1 }, &json!(7), m, &p).await.remove(0) } };
    person(&s, 1).await; let r = notify(&s, "done", "Built it", now_s() + 5).await; assert!(r.0, "{r:?}"); s.fake.wait(1).await;
    rpc("desk.look", json!({"thread":s.chat,"focused":false})).await; settle().await; assert!(s.fake.clears().is_empty(), "an unfocused desk has not seen it");
    rpc("desk.look", json!({"thread":s.chat,"focused":true})).await;
    for _ in 0..300 { if !s.fake.clears().is_empty() { break } tokio::time::sleep(std::time::Duration::from_millis(30)).await }
    let c = s.fake.clears(); assert_eq!(c.len(), 1, "{c:?}"); assert_eq!(c[0]["aps"]["badge"], 0, "read on the desktop: off the phone's badge");
    // a backlog, cleared at once; an open ask still counts
    s.hub.commit(&json!({"$":"ThreadCreated","id":"tw","project":"","title":"Worker","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap();
    s.hub.commit(&json!({"$":"AskOpened","id":"a9","thread":"tw","tool":"Bash","detail":"rm x","at":2})).await.unwrap();
    person(&s, 10).await; let r = notify(&s, "needs_you", "Approve rm", now_s() + 15).await; assert!(r.0, "{r:?}");
    person(&s, 20).await; let r = notify(&s, "done", "Another thing", now_s() + 25).await; assert!(r.0, "{r:?}"); s.fake.wait(3).await;
    let book = |hub: Arc<crate::hub::Hub>| async move { serde_json::from_str::<push::Book>(&hub.setting(push::BOOK).await.unwrap_or_default()).unwrap_or_default() };
    settle().await; settle().await; // the thread alert records the notes wrote order pushes of their own (settings/alerts.rs): all of them in the book first
    rpc("push.clear", json!({})).await;
    for _ in 0..300 { if s.fake.clears().len() >= 2 { break } tokio::time::sleep(std::time::Duration::from_millis(30)).await }
    assert_eq!(s.fake.clears().len(), 2); settle().await; let b = book(s.hub.clone()).await; assert!(b.open.is_empty() && b.held.is_empty(), "{b:?}");
    assert_eq!(s.fake.clears()[1]["aps"]["badge"], 1, "the open ask still counts");
}

#[tokio::test] async fn a_dead_token_is_retired() {
    let s = setup_with("dead", Fake { dead: true, ..Fake::default() }).await;
    let r = notify(&s, "needs_you", "to a dead phone", now_s()).await; assert!(r.0, "{r:?}");
    until(&s.hub, |l| l.iter().any(|e| e["$"] == "DeviceDropped")).await;
    assert!(s.bots.world().await.devices.is_empty());
}

#[tokio::test] async fn a_turn_end_nobody_manages_is_mail_the_uberbot_reads_and_starts_no_turn() {
    let s = setup("relay").await;
    s.hub.commit(&json!({"$":"ThreadCreated","id":"tw","project":"","title":"Worker","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap();
    let n = s.rec.got.lock().unwrap().len();
    turn_said(&s.hub, "tw", "completed", "finished the layout").await;
    until(&s.hub, |l| l.iter().any(|e| e["$"] == "MailPosted" && e["id"].as_str().unwrap_or("").starts_with("triage-end:tw"))).await;
    settle().await;
    let m = log(&s.hub).into_iter().find(|e| e["$"] == "MailPosted" && e["id"].as_str().unwrap_or("").starts_with("triage-end:tw")).unwrap(); assert_eq!(m["want"], "fyi");
    let got = s.rec.got.lock().unwrap().clone(); assert_eq!(got.len(), n, "fyi wakes no turn: {:?}", &got[n..]);
    assert!(!log(&s.hub).iter().any(|e| e["$"] == "RunSet" && e["mail"] == m["id"]), "and starts no run");
    assert!(s.fake.alerts().is_empty());
}
#[tokio::test] async fn a_workers_ask_is_mail_that_asks_the_uberbot_to_act_and_its_own_ask_orders_a_push() {
    let s = setup("asks").await;
    s.hub.commit(&json!({"$":"ThreadCreated","id":"tw","project":"","title":"Worker","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap();
    s.hub.commit(&json!({"$":"AskOpened","id":"a1","thread":"tw","tool":"Bash","detail":"rm x","at":2})).await.unwrap();
    until(&s.hub, |l| l.iter().any(|e| e["$"] == "MailPosted" && e["id"] == "triage-ask:a1")).await;
    assert_eq!(log(&s.hub).into_iter().find(|e| e["id"] == "triage-ask:a1").unwrap()["want"], "act");
    settle().await; assert!(s.fake.alerts().is_empty());
    s.hub.commit(&json!({"$":"AskOpened","id":"a2","thread":s.chat,"tool":"order_quote","detail":"Pay for the boards?","at":3})).await.unwrap();
    let a = s.fake.wait(1).await; assert_eq!((a[0]["ask"].as_str(), a[0]["aps"]["alert"]["body"].as_str()), (Some("a2"), Some("Pay for the boards?")));
}

// the pure rules
#[test] fn quiet_windows_cross_midnight_and_off_is_off() {
    assert_eq!(push::window("22:00-07:00"), Some((1320, 420)));
    for bad in ["off", "", "25:00-07:00", "07:00-07:00", "7-8"] { assert_eq!(push::window(bad), None, "{bad}") }
    let h = |x: u64| x * 3600;
    assert_eq!(push::quiet_left("22:00-07:00", h(23)), Some(h(8))); assert_eq!(push::quiet_left("22:00-07:00", h(6)), Some(h(1)));
    assert_eq!(push::quiet_left("22:00-07:00", h(7)), None, "the end is out"); assert_eq!(push::quiet_left("22:00-07:00", h(22)), Some(h(9)), "the start is in");
    assert_eq!(push::quiet_left("09:00-17:00", h(12)), Some(h(5))); assert_eq!(push::quiet_left("09:00-17:00", h(18)), None);
    assert_eq!(push::quiet_left("off", h(23)), None);
}
#[test] fn the_new_york_fallback_tracks_daylight_time() {
    let r = crate::bots::clock::TzRule::parse(push::ZONE_DEFAULT).unwrap();
    assert_eq!(r.wall(1_782_000_000) as i64 - 1_782_000_000, -4 * 3600, "June: EDT"); assert_eq!(r.wall(1_798_000_000) as i64 - 1_798_000_000, -5 * 3600, "December: EST");
}
#[test] fn route_takes_the_later_hold() {
    assert_eq!(push::route(1000, 0, 120, None, false), None, "nothing went yet");
    assert_eq!(push::route(1000, 950, 120, None, false), Some(1070)); assert_eq!(push::route(1000, 800, 120, None, false), None);
    assert_eq!(push::route(1000, 0, 120, Some(5000), false), Some(5000)); assert_eq!(push::route(1000, 0, 120, Some(5000), true), None, "through passes quiet");
    assert_eq!(push::route(1000, 950, 120, Some(5000), true), Some(1070), "but not the burst"); assert_eq!(push::route(1000, 950, 120, Some(1010), false), Some(1070));
}
fn item(id: &str, th: &str, ask: &str, reason: &str) -> Item { Item { id: id.into(), reason: reason.into(), thread: th.into(), ask: ask.into(), text: format!("text {id}"), at: 1, due: 0, key: format!("k-{id}"), link: String::new() } }
#[test] fn a_collapsed_push_links_its_shared_thread_else_home() {
    let one = push::collapse(&[item("a", "t1", "x", "done")], "Bub", "home", "s"); assert_eq!((one.key.as_str(), one.title.as_str(), one.body.as_str(), one.thread.as_str(), one.ask.as_str()), ("push-a", "Bub", "text a", "t1", "x"));
    assert_eq!(push::collapse(&[item("a", "t1", "", "done")], "", "home", "s").title, "Backplane");
    let same = push::collapse(&[item("a", "t1", "", "done"), item("b", "t1", "", "digest")], "Bub", "home", "s"); assert_eq!((same.thread.as_str(), same.body.as_str(), same.reason.as_str(), same.ask.as_str()), ("t1", "2 updates", "digest", ""));
    let mix = push::collapse(&[item("a", "t1", "", "needs_you"), item("b", "t2", "", "done")], "Bub", "home", "s"); assert_eq!((mix.thread.as_str(), mix.body.as_str(), mix.reason.as_str()), ("home", "1 of 2 need you", "needs_you"));
}
#[test] fn the_book_counts_and_clears() {
    let mut b = Book { open: vec![item("a", "t1", "x", "needs_you"), item("b", "t1", "", "done")], held: vec![Item { due: 50, ..item("c", "t2", "", "done") }], last: 0 };
    assert_eq!(b.badge(1), 3, "an ask once, and the items not about one"); assert_eq!(b.next(), Some(50));
    assert!(b.due(49).is_empty()); assert_eq!(b.due(50).len(), 1); assert!(b.held.is_empty());
    b.open[1].key = "k-a".into(); // two items shown as one
    let (keys, changed) = b.deal(|i| i.id == "a"); assert!(changed && keys.is_empty(), "the notification stays while one of its items is open");
    let (keys, _) = b.deal(|i| i.id == "b"); assert_eq!(keys, vec!["k-a"]); assert_eq!(b.deal(|_| true), (vec![], false));
    let mut b = Book { open: (0..250).map(|i| Item { at: 1000 + i, ..item(&i.to_string(), "t", "", "done") }).collect(), ..Book::default() }; b.trim(2000); assert_eq!(b.open.len(), 200); assert_eq!(b.open[0].id, "50");
    b.trim(1000 + 8 * 86400); assert!(b.open.is_empty(), "a week on it is gone");
}
#[test] fn who_hears() {
    let d = |p: &str, k: &str, m: bool| Device { token: "t".into(), platform: p.into(), kind: k.into(), env: "sandbox".into(), bundle: "b".into(), mute: m };
    assert!(push::hears(&d("ios", "alert", false), false) && push::hears(&d("ios", "alert", true), true) && !push::hears(&d("ios", "alert", true), false));
    assert!(!push::hears(&d("ios", "activity", false), true) && !push::hears(&d("ios", "start", false), true) && !push::hears(&d("android", "alert", false), true));
}
#[test] fn short_is_one_line_and_cut_at_a_word() {
    assert_eq!(push::short("\n  first line\nsecond", 50), "first line"); assert_eq!(push::short("one two three four", 10), "one two…"); assert!(push::short(&"x".repeat(300), 180).chars().count() <= 180);
}
#[test] fn apns_answers_and_frames() {
    assert_eq!(apns::outcome("200", ""), Sent::Ok); assert_eq!(apns::outcome("410", r#"{"reason":"Unregistered"}"#), Sent::Dead);
    for r in ["BadDeviceToken", "DeviceTokenNotForTopic", "Unregistered"] { assert_eq!(apns::outcome("400", &format!("{{\"reason\":\"{r}\"}}")), Sent::Dead, "{r}") }
    assert!(matches!(apns::outcome("400", r#"{"reason":"TopicDisallowed"}"#), Sent::Later(_)), "the key may not push to the topic: the phone's token is fine (Nov 10)");
    assert_eq!(apns::said("alert push-a to abc (production dev.b)", "200", "", "ID-1", &Sent::Ok), "push: APNs alert push-a to abc (production dev.b): 200 apns-id ID-1");
    assert_eq!(apns::said("alert k to abc (production dev.b)", "400", r#"{"reason":"TopicDisallowed"}"#, "ID-2", &Sent::Later(String::new())), "push: APNs alert k to abc (production dev.b): 400 TopicDisallowed apns-id ID-2: kept");
    assert!(matches!(apns::outcome("400", r#"{"reason":"BadTopic"}"#), Sent::Later(_))); assert!(matches!(apns::outcome("403", r#"{"reason":"ExpiredProviderToken"}"#), Sent::Later(_)), "our key, not the phone's token"); assert!(matches!(apns::outcome("429", ""), Sent::Later(_))); assert!(matches!(apns::outcome("503", ""), Sent::Later(_)));
    assert_eq!(apns::host("sandbox"), "https://api.sandbox.push.apple.com"); assert_eq!(apns::host("production"), "https://api.push.apple.com");
    let say = push::collapse(&[item("a", "t1", "x", "needs_you")], "Bub", "h", "s");
    let f = Frame::alert(&say, 3); assert_eq!((f.push_type, f.priority, f.collapse.as_str()), ("alert", 10, "push-a"));
    let c = apns::curl_config("https://u/3/device/T", "JWT", "dev.b", &f, std::path::Path::new("/b"), std::path::Path::new("/o"));
    for line in ["authorization: bearer JWT", "apns-topic: dev.b", "apns-push-type: alert", "apns-priority: 10", "apns-collapse-id: push-a", "data-binary = \"@/b\"", "%header{apns-id}"] { assert!(c.contains(line), "{line}: {c}") }
    let f = Frame::clear(&["k".to_string()], 0); assert_eq!((f.push_type, f.priority), ("background", 5)); assert!(f.payload["aps"].get("alert").is_none() && f.payload["aps"].get("sound").is_none(), "a clear shows nothing");
    assert!(!apns::curl_config("u", "J", "t", &f, std::path::Path::new("/b"), std::path::Path::new("/o")).contains("apns-collapse-id"));
}
#[test] fn a_der_signature_becomes_64_bytes() {
    let mut der = vec![0x30, 0x45, 0x02, 0x21, 0x00]; der.extend([0xAA; 32]); der.extend([0x02, 0x20]); der.extend([0xBB; 32]);
    let r = apns::raw_sig(&der).unwrap(); assert_eq!(&r[..32], &[0xAA; 32]); assert_eq!(&r[32..], &[0xBB; 32]);
    let mut short = vec![0x30, 0x24, 0x02, 0x1F]; short.extend([0xCC; 31]); short.extend([0x02, 0x01, 0x01]); let r = apns::raw_sig(&short).unwrap(); assert_eq!(r[0], 0, "a short integer is padded left"); assert_eq!(r[1], 0xCC); assert_eq!(r[63], 1);
    assert!(apns::raw_sig(&[0x31, 0]).is_none() && apns::raw_sig(&[]).is_none());
}
#[test] fn a_real_key_signs_a_token_openssl_verifies() {
    let d = std::env::temp_dir().join(format!("bp-push-jwt-{}", std::process::id())); std::fs::create_dir_all(&d).unwrap(); let (k, p) = (d.join("k.pem"), d.join("p.pem"));
    let sh = |a: &[&str]| std::process::Command::new("openssl").args(a).output().unwrap();
    assert!(sh(&["ecparam", "-name", "prime256v1", "-genkey", "-noout", "-out", k.to_str().unwrap()]).status.success());
    assert!(sh(&["ec", "-in", k.to_str().unwrap(), "-pubout", "-out", p.to_str().unwrap()]).status.success());
    let t = apns::token(&k, "KID", "TEAM", 1700000000).unwrap(); let parts: Vec<&str> = t.split('.').collect(); assert_eq!(parts.len(), 3);
    use base64::Engine; let un = |s: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&un(parts[0])).unwrap(), json!({"alg":"ES256","kid":"KID"})); assert_eq!(serde_json::from_slice::<Value>(&un(parts[1])).unwrap(), json!({"iss":"TEAM","iat":1700000000}));
    let raw = un(parts[2]); assert_eq!(raw.len(), 64);
    // back to DER for openssl to check it
    let int = |b: &[u8]| { let b: Vec<u8> = b.iter().copied().skip_while(|x| *x == 0).collect(); let mut v = vec![0x02]; if b[0] & 0x80 != 0 { v.push(b.len() as u8 + 1); v.push(0) } else { v.push(b.len() as u8) } v.extend(b); v };
    let (r, s2) = (int(&raw[..32]), int(&raw[32..])); let mut der = vec![0x30, (r.len() + s2.len()) as u8]; der.extend(r); der.extend(s2);
    std::fs::write(d.join("sig"), der).unwrap(); std::fs::write(d.join("msg"), format!("{}.{}", parts[0], parts[1])).unwrap();
    let o = sh(&["dgst", "-sha256", "-verify", p.to_str().unwrap(), "-signature", d.join("sig").to_str().unwrap(), d.join("msg").to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout)); let _ = std::fs::remove_dir_all(d);
}

#[tokio::test] async fn an_expired_sign_in_is_one_alert_in_the_uberbots_name_that_opens_the_account_and_leaves_when_it_is_back() {
    let s = setup("signin").await;
    let rec = json!({"profile":"p1","epoch":2,"provider":"claude","label":"Work","text":"Claude sign-in expired: tap to sign in again","link":"account:p1","at":now_s()});
    set(&s.hub, "account.signin.alert.claude", &rec.to_string()).await;
    let a = s.fake.wait(1).await; assert_eq!(a.len(), 1, "{a:?}");
    assert_eq!((a[0]["aps"]["alert"]["body"].as_str(), a[0]["link"].as_str(), a[0]["thread"].as_str(), a[0]["kind"].as_str()), (Some("Claude sign-in expired: tap to sign in again"), Some("account:p1"), Some(s.chat.as_str()), Some("needs_you")));
    let ordered = log(&s.hub).into_iter().find(|e| e["$"] == "PushOrdered").unwrap(); assert_eq!((ordered["how"].as_str(), ordered["bot"].as_str()), (Some("notify"), Some(s.bub.as_str())), "in the uberbot's name");
    until(&s.hub, |l| l.iter().any(|e| e["$"] == "PushSent")).await;
    let sent = log(&s.hub).into_iter().find(|e| e["$"] == "PushSent").unwrap(); assert_eq!(sent["link"], "account:p1", "a socket hears the link too");
    assert_eq!(crate::data::ext_notify(&sent)["link"], "account:p1");
    set(&s.hub, "account.signin.alert.claude", "").await;
    until(&s.hub, |l| l.iter().any(|e| e["$"] == "PushCleared")).await;
    assert_eq!(s.fake.alerts().len(), 1, "no second alert");
    assert!(push::reserved("account.signin.alert.claude"), "no client may write it");
}

// 0.14's rule (src/server/push.bend): an alert is a push of its own, raised when a thread's alert record is written, unless a desk in use is showing the thread
#[tokio::test] async fn a_threads_alert_reaches_the_phone_unless_a_focused_desk_shows_it() {
    let (hub, bots, _rec) = start("push-alert", "").await;
    set(&hub, push::QUIET, "off").await; set(&hub, push::BURST, "0").await; device(&hub, "tok1").await;
    let fake = Arc::new(Fake::default()); let _p = push::start_with(&hub, bots.clone(), fake.clone());
    let desks = crate::settings::alerts::start(&hub);
    for (id, title) in [("tw", "Worker"), ("tv", "Viewed")] { hub.commit(&json!({"$":"ThreadCreated","id":id,"project":"","title":title,"env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap(); }
    turn_said(&hub, "tw", "completed", "Done: the board is routed.\nMore below.").await;
    // a desk in use, focused on tv
    let _o = hub.attach(7); desks.say(7, "tv", true, now_s());
    set(&hub, "alert.tw", "done\tturn-tw\tm\t1.1\t").await;
    let a = fake.wait(1).await; assert_eq!(a.len(), 1, "{a:?}");
    assert_eq!((a[0]["aps"]["alert"]["title"].as_str(), a[0]["aps"]["alert"]["body"].as_str(), a[0]["thread"].as_str()), (Some("Worker"), Some("Done: the board is routed."), Some("tw")), "the last reply's line: {a:?}");
    until(&hub, |l| l.iter().any(|e| e["$"] == "PushSent" && e["kind"] == "done")).await;
    let ordered = log(&hub).into_iter().find(|e| e["$"] == "PushOrdered").unwrap(); assert_eq!((ordered["how"].as_str(), ordered["bot"].as_str()), (Some("alert"), Some("")));
    // the focused desk shows tv: its end is the desk's, not the phone's
    set(&hub, "alert.tv", "done\tturn-tv\tm\t1.2\t").await; settle().await; assert_eq!(fake.alerts().len(), 1, "{:?}", fake.alerts());
    // an ask: needs_you, with its ask so its answer clears it; a failure; a merge; a note
    hub.commit(&json!({"$":"AskOpened","id":"a7","thread":"tw","tool":"Bash","detail":"rm x","at":2})).await.unwrap();
    set(&hub, "alert.tw", "ask\task-tw\tm\t1.3\tBash rm x").await;
    let a = fake.wait(2).await; assert_eq!((a[1]["aps"]["alert"]["body"].as_str(), a[1]["ask"].as_str(), a[1]["kind"].as_str()), (Some("Needs you: Bash rm x"), Some("a7"), Some("needs_you")), "{a:?}");
    for (v, body) in [("fail\tturn-tw\tm\t1.4\t", "Failed"), ("merged\tm-tw\tm\t1.5\tbranch", "Merged: branch"), ("note\ttell-1\tm\t1.6\tthe mailroom says", "the mailroom says")] {
        let n = fake.alerts().len(); set(&hub, "alert.tw", v).await; let a = fake.wait(n + 1).await; assert_eq!(a[n]["aps"]["alert"]["body"].as_str(), Some(body), "{a:?}");
    }
    // a quiet record (no kind) is no alert
    let n = fake.alerts().len(); set(&hub, "alert.tw", "\t\tm\t1.7\t").await; settle().await; assert_eq!(fake.alerts().len(), n);
    // an order in this way's name about no thread, or in a bot's name, is refused
    for (bot, th) in [("", ""), ("ada", "tw")] { hub.commit(&json!({"$":"PushOrdered","id":format!("x{bot}"),"how":"alert","bot":bot,"reason":"done","thread":th,"ask":"","text":"forged","at":now_s()})).await.unwrap(); }
    settle().await; assert_eq!(fake.alerts().len(), n, "{:?}", fake.alerts());
}

// ---- Settings → Notifications: every press changes what a phone is alerted to (the page's own half is `push/page.rs`; the rows are `present/settings.rs`) ----
async fn press(s: &S, action: &str, value: &str, fields: Value) -> Result<(), String> { push::page::act(&s.hub, &s.bots, action, value, &fields).await }
async fn worker(hub: &Arc<Hub>, id: &str) { hub.commit(&json!({"$":"ThreadCreated","id":id,"project":"","title":"Worker","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1})).await.unwrap(); }
/// the person gives a thread work, and its turn ends
async fn work(hub: &Arc<Hub>, th: &str, n: u64, end: &str, said: &str) {
    hub.commit(&json!({"$":"MessagePosted","thread":th,"msg":format!("w{n}"),"role":"user","text":format!("go {n}"),"at":now_s() + n})).await.unwrap();
    turn_said(hub, th, end, said).await;
}

#[tokio::test] async fn the_pages_quiet_hours_and_urgent_presses_decide_what_is_held() {
    let s = setup("page-quiet").await; worker(&s.hub, "tw").await;
    // a time the gate could not read is refused and changes nothing
    for (a, b) in [("25:00", "07:00"), ("22:00", "22:00"), ("22", "07:00"), ("", "")] { assert!(press(&s, "notify-quiet", "save", json!({"qfrom": a, "qto": b})).await.is_err(), "{a}-{b}"); }
    assert!(press(&s, "setting", "notify.quiet=x", json!({})).await.is_err() && press(&s, "setting", "push.book=on", json!({})).await.is_err(), "only the page's own switches");
    press(&s, "notify-quiet", "save", json!({"qfrom": "00:00", "qto": "23:59"})).await.unwrap();
    assert_eq!(s.hub.setting(push::QUIET).await.as_deref(), Some("00:00-23:59"));
    let r = s.bots.tool_call_at(&s.chat, "notify_person", &json!({"reason":"needs_you","text":"Held while quiet","thread":"tw"}), now_s()).await; assert!(r.0, "{r:?}");
    settle().await; assert!(s.fake.alerts().is_empty(), "quiet hours on: held");
    press(&s, "setting", "notify.quiet.urgent=on", json!({})).await.unwrap(); person(&s, 1).await;
    let r = notify(&s, "needs_you", "Urgent passes", now_s() + 5).await; assert!(r.0, "{r:?}");
    assert_eq!(s.fake.wait(1).await[0]["aps"]["alert"]["body"], "Urgent passes");
    press(&s, "notify-quiet", "off", json!({})).await.unwrap();
    assert_eq!(s.fake.wait(2).await.len(), 2, "quiet hours off: the held one goes");
    press(&s, "notify-quiet", "on", json!({})).await.unwrap();
    assert_eq!(s.hub.setting(push::QUIET).await.as_deref(), Some(push::QUIET_DEFAULT), "on again is the default window");
}

#[tokio::test] async fn the_pages_mute_silences_one_phone_and_urgent_lets_it_through() {
    let s = setup("page-mute").await; device(&s.hub, "tok2").await; settle().await;
    let h1 = push::page::handle("tok1");
    let said = { let w = s.bots.world().await; push::page::devices_info(w.devices.values()) };
    assert!(said.contains(&h1) && !said.contains("tok1") && !said.contains("tok2"), "a phone is a handle on the page, never its token: {said}");
    press(&s, "notify-mute", &format!("{h1}=on"), json!({})).await.unwrap();
    let r = notify(&s, "needs_you", "only the other phone", now_s()).await; assert!(r.0, "{r:?}");
    s.fake.wait(1).await; settle().await;
    assert_eq!(s.fake.sent.lock().unwrap().iter().map(|x| x.0.clone()).collect::<Vec<_>>(), vec!["tok2".to_string()], "muted from the page");
    press(&s, "setting", "notify.quiet.urgent=on", json!({})).await.unwrap(); person(&s, 1).await;
    let r = notify(&s, "needs_you", "urgent reaches both", now_s() + 5).await; assert!(r.0, "{r:?}");
    assert_eq!(s.fake.wait(3).await.len(), 3);
    press(&s, "setting", "notify.quiet.urgent=off", json!({})).await.unwrap(); press(&s, "notify-mute", &format!("{h1}=off"), json!({})).await.unwrap(); person(&s, 20).await;
    let r = notify(&s, "needs_you", "unmuted", now_s() + 25).await; assert!(r.0, "{r:?}");
    assert_eq!(s.fake.wait(5).await.len(), 5, "unmuted: both hear it");
    assert!(press(&s, "notify-mute", "nobody=on", json!({})).await.is_err());
}

/// What a turn's end and an opened ask write, from the policy's own pure rules (`state/turnend.rs`, the ones the hub runs over its state), committed on this hub so its push worker sees them.
/// (The hub's policy state is one per process, so a test of one hub mirrors the settings and the person's messages in a state of its own.)
async fn ends(hub: &Arc<Hub>, st: &mut crate::state::State, th: &str, state: &str) { for e in crate::state::fam::turnend::policy_end(st, 1, now_s(), th, state) { st.apply_line(&e.to_string()); hub.commit(&e).await.unwrap(); } }
async fn asks(hub: &Arc<Hub>, st: &mut crate::state::State, ev: Value) { for e in crate::state::fam::turnend::alert_asks(st, 1, &[ev]) { st.apply_line(&e.to_string()); hub.commit(&e).await.unwrap(); } }
async fn flip(s: &S, st: &mut crate::state::State, k: &str, v: &str) { press(s, "setting", &format!("{k}={v}"), json!({})).await.unwrap(); st.apply_line(&json!({"$":"SettingSet","key":k,"value":v,"at":1}).to_string()); }

#[tokio::test] async fn each_notify_switch_changes_whether_a_thread_alerts_the_phone() {
    let s = setup("page-switch").await; worker(&s.hub, "tw").await; let _desks = crate::settings::alerts::start(&s.hub);
    let mut st = crate::state::State::new();
    st.apply_line(r#"{"$":"ThreadCreated","id":"tw","project":"","title":"Worker","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":1}"#);
    let say = |n: u64, text: &str| json!({"$":"MessagePosted","thread":"tw","msg":format!("w{n}"),"role":"user","text":text,"at":n});
    let mut ask = 0u64;
    macro_rules! person { ($n:expr) => {{ let m = say($n, "go"); st.apply_line(&m.to_string()); s.hub.commit(&m).await.unwrap(); }} }
    macro_rules! quiet { ($expect:expr, $why:expr) => {{ settle().await; assert_eq!(s.fake.alerts().len(), $expect, "{}", $why); }} }
    // Settled: the person's work finished, or failed
    person!(1); ends(&s.hub, &mut st, "tw", "completed").await; let a = s.fake.wait(1).await; assert_eq!(a.len(), 1, "on by default: {a:?}");
    flip(&s, &mut st, "notify.settled", "off").await;
    person!(2); ends(&s.hub, &mut st, "tw", "completed").await; person!(3); ends(&s.hub, &mut st, "tw", "failed").await; quiet!(1, "Settled off: neither the finish nor the failure alerts");
    flip(&s, &mut st, "notify.settled", "on").await;
    person!(4); ends(&s.hub, &mut st, "tw", "failed").await; assert_eq!(s.fake.wait(2).await[1]["aps"]["alert"]["body"], "Failed", "Settled on again");
    // Needs me: an ask opens
    let mut open = |detail: &str| { ask += 1; json!({"$":"AskOpened","id":format!("a{ask}"),"thread":"tw","kind":"approval","tool":"Bash","detail":detail,"options":"","at":now_s()}) };
    flip(&s, &mut st, "notify.needs", "off").await;
    let e = open("rm one"); s.hub.commit(&e).await.unwrap(); asks(&s.hub, &mut st, e).await; quiet!(2, "Needs me off: an ask alerts no one");
    flip(&s, &mut st, "notify.needs", "on").await;
    let e = open("rm two"); s.hub.commit(&e).await.unwrap(); asks(&s.hub, &mut st, e).await;
    assert_eq!(s.fake.wait(3).await[2]["aps"]["alert"]["body"], "Needs you: Bash rm two");
    // Everything: an end the person did not ask for (their message above is settled)
    ends(&s.hub, &mut st, "tw", "completed").await; ends(&s.hub, &mut st, "tw", "failed").await; quiet!(3, "Everything off: a plain end is quiet");
    flip(&s, &mut st, "notify.all", "on").await;
    turn_said(&s.hub, "tw", "completed", "Loud end").await; ends(&s.hub, &mut st, "tw", "completed").await;
    assert_eq!(s.fake.wait(4).await.len(), 4, "Everything on: it alerts");
    ends(&s.hub, &mut st, "tw", "stopped").await; quiet!(4, "a stop never does");
}

#[tokio::test] async fn the_reminders_switch_decides_whether_a_due_reminder_alerts_the_phone() {
    let s = setup("page-remind").await; let past = 1_000_000u64; let ada = bot(&s.hub, "ada").await;
    press(&s, "setting", "notify.reminders=off", json!({})).await.unwrap();
    for e in [json!({"$":"RoutineSet","id":"r1","bot":ada,"name":"water","cron":"","prompt":"drink water","on":true,"at":past}), json!({"$":"RoutinePlan","id":"r1","once":past + 10,"tz":"person","kind":"remind","at":past})] { s.hub.commit(&e).await.unwrap(); }
    assert_eq!(s.bots.tick_at(past + 20).await, 1); settle().await; assert!(s.fake.alerts().is_empty(), "Reminders off: it fires and says nothing to the phone");
    press(&s, "setting", "notify.reminders=on", json!({})).await.unwrap();
    for e in [json!({"$":"RoutineSet","id":"r2","bot":ada,"name":"stretch","cron":"","prompt":"stretch","on":true,"at":past}), json!({"$":"RoutinePlan","id":"r2","once":past + 30,"tz":"person","kind":"remind","at":past})] { s.hub.commit(&e).await.unwrap(); }
    assert_eq!(s.bots.tick_at(past + 40).await, 1); assert_eq!(s.fake.wait(1).await[0]["aps"]["alert"]["body"], "stretch");
}

#[test] fn apns_credentials_come_from_the_settings_else_the_env_file_so_push_cannot_silently_be_off() {
    let d = std::env::temp_dir().join(format!("bp-apns-env-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(d.join("apns")).unwrap();
    std::fs::write(d.join("apns/AuthKey_KID1.p8"), "k").unwrap(); std::fs::write(d.join("own.p8"), "k").unwrap();
    let env = "export BACKPLANE_APNS_TEAM_ID=TEAM1\nexport BACKPLANE_APNS_KEY_ID=\"KID1\"\n# c\nexport BACKPLANE_APNS_BUNDLE_ID=x.y\n";
    let none = || (String::new(), String::new(), String::new());
    assert_eq!(apns::resolve(none(), None), None, "nothing set, no file: off");
    let r = apns::resolve(none(), Some((env, &d))).unwrap(); assert_eq!((r.1.as_str(), r.2.as_str(), r.3), ("KID1", "TEAM1", "apns.env"), "empty settings: the env file, the key beside it named for the key id");
    assert!(r.0.ends_with("apns/AuthKey_KID1.p8"));
    let own = d.join("own.p8").to_string_lossy().into_owned();
    let r = apns::resolve((own.clone(), "K".into(), "T".into()), Some((env, &d))).unwrap(); assert_eq!((r.0, r.1, r.2, r.3), (own.clone(), "K".into(), "T".into(), "settings"), "settings win and the file is not used");
    let env2 = format!("{env}export BACKPLANE_APNS_PRIVATE_KEY_PATH={own}\n"); assert_eq!(apns::resolve(none(), Some((&env2, &d))).unwrap().0, own, "the file's own key path");
    assert_eq!(apns::resolve(none(), Some(("export BACKPLANE_APNS_KEY_ID=KID9\n", &d))), None, "a team missing, or a key file that is not there: off");
    let _ = std::fs::remove_dir_all(&d);
}

// ---- Nov 10: what reached the phone and should not have (a room's bot-to-bot chatter, a mailroom run's failure, raw markdown) ----
/// the mailroom's run a filing starts for the uberbot, and its thread
async fn a_run(s: &S, n: usize) -> (String, String) {
    until(&s.hub, |l| l.iter().filter(|e| e["$"] == "RunSet").count() >= n).await;
    let r = log(&s.hub).into_iter().filter(|e| e["$"] == "RunSet").nth(n - 1).unwrap();
    (r["thread"].as_str().unwrap().to_string(), r["mail"].as_str().unwrap().to_string())
}
#[tokio::test] async fn a_room_post_between_bots_and_the_run_it_starts_reach_no_phone() {
    let s = setup("room").await; let _desks = crate::settings::alerts::start(&s.hub);
    let ada = bot(&s.hub, "pebble").await; let ada_th = s.bots.world().await.bot(&ada).unwrap().thread.clone();
    assert_eq!(rpc(&s.hub, "bots.room", json!({"room":"r1","name":"ring-requests","members":format!("{ada}\n{}", s.bub)})).await["ok"], true);
    let (ok, r) = s.bots.tool_call(&ada_th, "room_post", &json!({"room":"ring-requests","text":"Ring note `n4d35`: when should I set the reminder?"})).await; assert!(ok, "{r}");
    // the post is filed for the uberbot and a run triages it; the run's end says it needs nothing
    let (run, _) = a_run(&s, 1).await;
    turn_said(&s.hub, &run, "completed", "The ring bot already asked the person for a time in #ring-requests, so this needs no action from me.").await;
    settle().await; settle().await;
    assert!(s.fake.alerts().is_empty(), "{:?}", s.fake.alerts());
    let l = log(&s.hub);
    assert!(!l.iter().any(|e| e["$"] == "PushOrdered"), "nothing was even ordered");
    let rec = l.iter().rev().find(|e| e["$"] == "SettingSet" && e["key"] == format!("alert.{run}").as_str()).map(|e| e["value"].as_str().unwrap().to_string());
    assert!(rec.as_ref().map_or(true, |v| v.starts_with('\t')), "a run's end is no alert record with a kind: {rec:?}");
    // and a record somebody writes for a run anyway is still no push
    set(&s.hub, &format!("alert.{run}"), "done\tturn-x\tm\t1.1\tforged").await; settle().await; assert!(s.fake.alerts().is_empty());
}
#[tokio::test] async fn a_mailroom_run_that_fails_over_keeps_its_tools_and_its_failure_reaches_no_room_and_no_phone() {
    let s = setup("failover").await; let _desks = crate::settings::alerts::start(&s.hub);
    assert_eq!(rpc(&s.hub, "bots.room", json!({"room":"r1","name":"ring-requests","members":s.bub.clone()})).await["ok"], true);
    assert_eq!(rpc(&s.hub, "bots.post", json!({"room":"r1","text":"remind me to get that couch off","msg":"p1"})).await["ok"], true);
    let (run, input) = a_run(&s, 1).await;
    s.hub.commit(&json!({"$":"TurnChanged","thread":run,"state":"running","at":now_s()})).await.unwrap(); settle().await;
    let tools = |w: &crate::bots::world::World| crate::bots::mail::tools::lists(w).get(&run).cloned().unwrap_or_default();
    assert!(tools(&*s.bots.world().await).contains(&"mail_dispose"), "a live run lists its tools");
    // the account ran out: the driver stops the turn to change accounts and starts it again at once (settings/driver.rs)
    s.hub.commit(&json!({"$":"TurnChanged","thread":run,"state":"interrupted","failover":true,"at":now_s()})).await.unwrap(); settle().await;
    { let w = s.bots.world().await; assert!(w.mail.run_of(&run).map_or(false, |r| r.live), "the run goes on"); assert!(tools(&w).contains(&"mail_dispose"), "and the restarted session lists its tools: {:?}", tools(&w)); }
    assert_eq!(log(&s.hub).iter().filter(|e| e["$"] == "RunSet").count(), 1, "nothing was forwarded as undone");
    // a run that still ends without its disposition: its bot hears privately; no room and no phone does
    let posts = |l: &[Value]| l.iter().filter(|e| e["$"] == "RoomPosted").count();
    let before = posts(&log(&s.hub));
    turn_said(&s.hub, &run, "completed", &format!("I couldn't dispose of input `{input}` in this run. The mailroom tools aren't in my tool list.")).await;
    let (lane, _) = a_run(&s, 2).await; assert!(lane.starts_with(&format!("mailcoord-{}-", s.bub)), "its bot's back office takes it: {lane}");
    turn_said(&s.hub, &lane, "completed", "Nothing for the person: the ring bot asks them itself.").await;
    settle().await; settle().await;
    assert_eq!(posts(&log(&s.hub)), before, "no room post");
    assert!(s.fake.alerts().is_empty(), "no push: {:?}", s.fake.alerts());
}
#[tokio::test] async fn the_uberbots_own_orders_are_not_pushed_twice_by_their_records() {
    let s = setup("twice").await; let _desks = crate::settings::alerts::start(&s.hub);
    let r = notify(&s, "needs_you", "Approve the deploy?", now_s()).await; assert!(r.0, "{r:?}");
    s.fake.wait(1).await; settle().await; settle().await;
    assert_eq!(s.fake.alerts().len(), 1, "the note record notify_person wrote is no second push: {:?}", s.fake.alerts());
    // a tell_person from the back office is private-lane output: on screen, no phone
    set(&s.hub, &format!("alert.{}", s.chat), "note\ttell-x\tm\t1.1\tThe [Boat thread](backplane://thread/t1) has answered my questions.").await; settle().await;
    assert_eq!(s.fake.alerts().len(), 1);
}
#[test] fn a_push_body_is_plain_words() {
    let p = |t: &str| push::plain(t, 180);
    assert_eq!(p("The [Boat thread](backplane://thread/t1794347576-102) has answered my questions. Nothing of mine is running on the Boat any more.  **You can set up the Pebble app now.** The ring stays on the desktop hub: 1. Open the desktop's **Settings → Ring**"),
        "The Boat thread has answered my questions. Nothing of mine is running on the Boat any more. You can set up the Pebble app now.");
    assert_eq!(p("## Done\n\nMore"), "Done");
    assert_eq!(p("- **Field-23** is `live` in ~~old~~ *main*"), "Field-23 is live in old main");
    assert_eq!(p("See [`docs/ring.md`](file:///home/h/x/docs/ring.md#L56) and ![shot](file:///a.png)."), "See docs/ring.md and shot.");
    assert_eq!(p("I couldn't dispose of input `p1794357742-592218` in this run (thread t1794347576-102)."), "I couldn't dispose of input in this run (thread).");
    assert_eq!(p("Open backplane://thread/t1 now"), "Open now");
    assert_eq!(p("1. First step\n2. Second"), "First step");
    assert_eq!(p("Commit e2246f7e5 and snake_case stay"), "Commit e2246f7e5 and snake_case stay");
    let long = "word ".repeat(60); let b = p(&long); assert!(b.chars().count() <= 180 && b.ends_with('…'), "{b}");
    assert_eq!(p("Plain text."), "Plain text.");
}

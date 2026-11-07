//! Against a real Hub on a temp home; messages into threads are recorded, no agent runs (no model, no network).
use super::*;
use crate::{api::{Caller, Fut}, hub::{Cfg, Hub}, presenter::Canned};
use super::rpc::*;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex as StdMutex};

type Got = (String, String, String, Option<(String, String, String)>);
#[derive(Default)] struct Rec { rides: StdMutex<Vec<(String, String, String)>>, notes: StdMutex<Vec<(String, String, String)>>, got: StdMutex<Vec<Got>>, stopped: StdMutex<Vec<String>>, slow: bool, hub: std::sync::OnceLock<Arc<Hub>>, answer: StdMutex<String>, queue: StdMutex<Vec<String>> }
impl Inbox for Arc<Rec> {
    fn deliver<'a>(&'a self, th: &'a str, msg: &'a str, text: &'a str, o: Option<(String, String, String)>) -> Fut<'a, Result<(), String>> {
        Box::pin(async move {
            if self.slow { tokio::time::sleep(std::time::Duration::from_millis(150)).await }
            self.got.lock().unwrap().push((th.into(), msg.into(), text.into(), o.clone()));
            // what the agents module files for a delivered message (agents::Agents::user): the message, then its origin
            if let Some(hub) = self.hub.get() {
                let mut ev = vec![json!({"$":"MessagePosted","thread":th,"msg":msg,"role":"user","text":text,"at":1})];
                if let Some((from, kind, r)) = o { ev.push(json!({"$":"MessageFrom","thread":th,"msg":msg,"from":from,"kind":kind,"ref":r,"at":1})) }
                let _ = hub.commit_many(&ev).await;
            }
            Ok(())
        })
    }
    fn stop<'a>(&'a self, th: &'a str) -> Fut<'a, ()> { Box::pin(async move { self.stopped.lock().unwrap().push(th.into()); }) }
    fn queued<'a>(&'a self, _th: &'a str) -> Fut<'a, Vec<String>> { Box::pin(async move { self.queue.lock().unwrap().clone() }) }
    fn approve<'a>(&'a self, _th: &'a str, _tool: &'a str, _detail: &'a str) -> Fut<'a, Result<String, String>> { Box::pin(async move { Ok(self.answer.lock().unwrap().clone()) }) }
    fn note<'a>(&'a self, th: &'a str, id: &'a str, text: &'a str) -> Fut<'a, Result<(), String>> { Box::pin(async move { self.notes.lock().unwrap().push((th.into(), id.into(), text.into())); Ok(()) }) }
    fn ride<'a>(&'a self, th: &'a str, id: &'a str, text: &'a str) -> Fut<'a, Result<(), String>> { Box::pin(async move { self.rides.lock().unwrap().push((th.into(), id.into(), text.into())); Ok(()) }) }
}
async fn start_slow(name: &str, log: &str, slow: bool) -> (Arc<Hub>, Arc<Bots>, Arc<Rec>) {
    let home = std::env::temp_dir().join(format!("bp-bots-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("events.jsonl"), log).unwrap();
    let hub = Hub::start(Cfg { home, version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let rec = Arc::new(Rec { slow, ..Default::default() });
    if name.starts_with("parity") { let _ = rec.hub.set(hub.clone()); }
    let bots = Bots::start_with(hub.clone(), Arc::new(rec.clone())).await;
    (hub, bots, rec)
}
async fn start(name: &str, log: &str) -> (Arc<Hub>, Arc<Bots>, Arc<Rec>) { start_slow(name, log, false).await }
const DESK_ONLY: &str = "shown on the person's desktop; only the uberbot reaches their phone";
fn caller() -> Caller { Caller { gen: "1".into(), client: 1 } }
async fn rpc(hub: &Arc<Hub>, m: &str, p: Value) -> Value { let h = hub.rpc_for(m).expect("handler"); h.call(hub, &caller(), &json!(7), m, &p).await.remove(0) }
fn log(hub: &Arc<Hub>) -> Vec<Value> { hub.store.range(0, usize::MAX).unwrap().iter().filter_map(|l| serde_json::from_str(l).ok()).collect() }
fn kinds(hub: &Arc<Hub>, from: usize) -> Vec<String> { log(hub)[from..].iter().map(|e| e["$"].as_str().unwrap_or("").to_string()).collect() }
async fn until(hub: &Arc<Hub>, f: impl Fn(&[Value]) -> bool) { for _ in 0..100 { if f(&log(hub)) { return } tokio::time::sleep(std::time::Duration::from_millis(50)).await } panic!("timed out") }
/// creates a bot and waits for its home job (the WorktreeSet that follows the reply), so a later count of events is steady
async fn bot(hub: &Arc<Hub>, name: &str) -> String {
    let b = rpc(hub, "bots.create", json!({"name":name})).await["bot"].as_str().unwrap().to_string();
    let th = log(hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == b.as_str()).unwrap()["thread"].as_str().unwrap().to_string();
    until(hub, |l| l.iter().any(|e| e["$"] == "WorktreeSet" && e["id"] == th.as_str())).await; b
}

#[tokio::test] async fn bots_create_logs_thread_and_bot_and_refuses_bad_or_taken_names() {
    // bothub Bots.create.k: a name is 1 to 24 of letters, digits, - and _, and free (any case); the reply comes before the home job
    let (hub, _b, _r) = start("create", "").await;
    let r = rpc(&hub, "bots.create", json!({"name":" Ada ","persona":"dry"})).await;
    assert_eq!(r["ok"], true); let b = r["bot"].as_str().unwrap().to_string();
    let ev: Vec<Value> = log(&hub).into_iter().filter(|e| e["$"] != "SettingSet").collect(); // the hub's own start-up settings come first
    assert_eq!(ev[0]["$"], "ThreadCreated"); assert_eq!((ev[0]["title"].as_str().unwrap(), ev[0]["env"].as_str().unwrap(), ev[0]["project"].as_str().unwrap()), ("Ada", "local", ""));
    assert_eq!(ev[1]["$"], "BotSet"); assert_eq!(ev[1]["id"], b.as_str()); assert_eq!(ev[1]["thread"], ev[0]["id"]); assert_eq!(ev[1]["look"], seed("Ada"));
    assert!(log(&hub).iter().any(|e| e["$"] == "WorktreeSet"), "the folder is made before the reply: a message sent at once to a new bot has somewhere to run");
    until(&hub, |l| l.iter().any(|e| e["$"] == "WorktreeSet")).await;
    let w = log(&hub).into_iter().find(|e| e["$"] == "WorktreeSet").unwrap();
    assert_eq!(w["id"], ev[0]["id"]); assert!(w["worktree"].as_str().unwrap().ends_with(&format!("/bots/{b}")));
    let taken = rpc(&hub, "bots.create", json!({"name":"ADA"})).await; assert_eq!(taken["error"], "a bot named ADA already exists");
    for bad in ["", "a b", &"x".repeat(25), "é"] { assert_eq!(rpc(&hub, "bots.create", json!({"name":bad})).await["error"], "a bot's name is 1 to 24 letters, digits, - or _", "{bad:?}") }
}

#[tokio::test] async fn bots_create_requests_do_not_race() {
    // one request decides and commits at a time: the same name is taken once, other names get their own ids
    let (hub, _b, _r) = start("race", "").await;
    let hs: Vec<_> = (0..8).map(|_| { let h = hub.clone(); tokio::spawn(async move { rpc(&h, "bots.create", json!({"name":"same"})).await }) }).collect();
    let mut oks = 0; for h in hs { if h.await.unwrap()["ok"] == true { oks += 1 } }
    assert_eq!(oks, 1);
    let hs: Vec<_> = (0..6).map(|i| { let h = hub.clone(); tokio::spawn(async move { rpc(&h, "bots.create", json!({"name":format!("n{i}")})).await }) }).collect();
    for h in hs { assert_eq!(h.await.unwrap()["ok"], true) }
    let ev = log(&hub); let mut ids: Vec<String> = ev.iter().filter(|e| e["$"] == "BotSet" || e["$"] == "ThreadCreated").map(|e| e["id"].as_str().unwrap().to_string()).collect();
    let n = ids.len(); ids.sort(); ids.dedup(); assert_eq!(ids.len(), n, "ids are unique");
}

#[tokio::test] async fn bots_create_uber_promotes_after_a_backup() {
    let (hub, _b, _r) = start("uber", "").await;
    let r = rpc(&hub, "bots.create", json!({"name":"kit","uber":true})).await; let b = r["bot"].as_str().unwrap().to_string();
    until(&hub, |l| l.iter().any(|e| e["$"] == "UberSet")).await;
    let u = log(&hub).into_iter().find(|e| e["$"] == "UberSet").unwrap(); assert_eq!(u["bot"], b.as_str()); assert!(u["uid"].as_str().unwrap().starts_with('u'));
    let dir = hub.cfg.home.join("backups"); assert!(std::fs::read_dir(dir).unwrap().next().is_some(), "backed up first");
    assert_eq!(rpc(&hub, "bots.uber", json!({"bot":b})).await["ok"], true, "the uberbot again changes nothing");
    assert_eq!(log(&hub).iter().filter(|e| e["$"] == "UberSet").count(), 1);
    assert_eq!(rpc(&hub, "bots.uber", json!({"bot":"p1~b9"})).await["error"], "that machine did not take it: that is done on its own machine");
    assert_eq!(rpc(&hub, "bots.uber", json!({"bot":"nope"})).await["error"], "only a bot on this machine can be the uberbot");
    assert_eq!(rpc(&hub, "bots.uber", json!({"bot":""})).await["ok"], true);
    assert_eq!(log(&hub).iter().filter(|e| e["$"] == "UberSet").count(), 2);
}

#[tokio::test] async fn bots_edit_keeps_what_is_not_given_and_delete_stops_the_thread_after_the_commit() {
    let (hub, _b, rec) = start("edit", "").await;
    let b = rpc(&hub, "bots.create", json!({"name":"bo","look":5,"persona":"p1"})).await["bot"].as_str().unwrap().to_string();
    until(&hub, |l| l.iter().any(|e| e["$"] == "WorktreeSet")).await;
    let n = log(&hub).len();
    assert_eq!(rpc(&hub, "bots.edit", json!({"bot":b,"persona":"p2"})).await["ok"], true);
    let e = &log(&hub)[n]; assert_eq!((e["$"].as_str().unwrap(), e["thread"].as_str().unwrap(), e["look"].as_u64().unwrap(), e["persona"].as_str().unwrap(), e["name"].as_str().unwrap()), ("BotSet", "", 5, "p2", "bo"));
    assert_eq!(rpc(&hub, "bots.edit", json!({"bot":"nope"})).await["error"], "no such bot");
    let th = log(&hub).iter().find(|e| e["$"] == "ThreadCreated").unwrap()["id"].as_str().unwrap().to_string();
    assert_eq!(rpc(&hub, "bots.delete", json!({"bot":b})).await["ok"], true);
    assert_eq!(*rec.stopped.lock().unwrap(), vec![th]);
    assert_eq!(rpc(&hub, "bots.delete", json!({"bot":b})).await["error"], "no such bot");
    assert_eq!(rec.stopped.lock().unwrap().len(), 1, "a refused delete stops nothing");
    assert_eq!(rpc(&hub, "bots.create", json!({"name":"bo"})).await["ok"], true, "a deleted bot's name is free again");
}

#[tokio::test] async fn bots_say_once() {
    // law bots_say_once: sent again with the same msg, answered ok, stored and woken once, also while the first is still being delivered
    let (hub, bots, rec) = start_slow("say", "", true).await;
    let b = bot(&hub, "bo").await;
    let (h1, h2) = (hub.clone(), hub.clone()); let (b1, b2) = (b.clone(), b.clone());
    let a = tokio::spawn(async move { rpc(&h1, "bots.say", json!({"bot":b1,"text":"hi","msg":"m1"})).await });
    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    let c = tokio::spawn(async move { rpc(&h2, "bots.say", json!({"bot":b2,"text":"hi","msg":"m1"})).await });
    assert_eq!(a.await.unwrap()["ok"], true); assert_eq!(c.await.unwrap()["ok"], true);
    { let got = rec.got.lock().unwrap(); assert_eq!(got.len(), 1); assert_eq!((got[0].1.as_str(), got[0].2.as_str()), ("m1", "hi")); }
    assert_eq!(log(&hub).iter().filter(|e| e["$"] == "BotWoke" && e["hop"] == 0).count(), 1);
    // once the agent filed it, a later repeat is still a repeat
    let th = bots.world().await.bot(&b).unwrap().thread.clone();
    hub.commit(&json!({"$":"MessagePosted","thread":th,"msg":"m1","role":"user","text":"hi","at":9})).await.unwrap();
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":b,"text":"hi","msg":"m1"})).await["ok"], true);
    assert_eq!(rec.got.lock().unwrap().len(), 1);
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":"gone","text":"x"})).await["error"], "no such bot", "a dead bot");
}

#[tokio::test] async fn bots_say_reply_quotes_the_message() {
    let (hub, bots, rec) = start("reply", "").await;
    let b = bot(&hub, "bo").await; let th = bots.world().await.bot(&b).unwrap().thread.clone();
    hub.commit(&json!({"$":"MessagePosted","thread":th,"msg":"q1","role":"assistant","text":"the answer","at":9})).await.unwrap();
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":b,"text":"thanks","reply":"q1"})).await["ok"], true);
    assert_eq!(rec.got.lock().unwrap()[0].2, "<in_reply_to msg=q1 chars=10>\nthe answer\n</in_reply_to>\n\nthanks");
    rpc(&hub, "bots.say", json!({"bot":b,"text":"plain","reply":"nope"})).await;
    assert_eq!(rec.got.lock().unwrap()[1].2, "plain");
}

#[tokio::test] async fn bots_post_once_and_wakes_other_members_here() {
    // laws bots_post_once, bot_held_wakes_none: a person's post (hop 0) is logged once, every live member bot hears it with its origin;
    // members elsewhere, a deleted bot and the uberbot do not
    let (hub, _b, rec) = start("post", "").await;
    let (a, b, c, u) = (bot(&hub, "aa").await, bot(&hub, "bb").await, bot(&hub, "cc").await, bot(&hub, "uu").await);
    rpc(&hub, "bots.delete", json!({"bot":c})).await;
    hub.commit(&json!({"$":"SettingSet","key":"uber.bot","value":u,"at":1})).await.unwrap();
    assert_eq!(rpc(&hub, "bots.room", json!({"room":"r1","name":"lab","members":format!("{a}\n{b}\n{c}\n{u}\nzed@far")})).await["ok"], true);
    let n = log(&hub).len();
    assert_eq!(rpc(&hub, "bots.post", json!({"room":"r1","text":"hello all","msg":"p1"})).await["ok"], true);
    assert_eq!(kinds(&hub, n)[..3], ["RoomPosted", "BotWoke", "BotWoke"]); // the uberbot is not woken: the post is filed for it
    assert!(kinds(&hub, n).iter().any(|k| k == "MailPosted"), "the uberbot in the room: filed as the room's mail, though the fold already holds the post's header");
    let e = &log(&hub)[n]; assert_eq!((e["id"].as_str().unwrap(), e["from"].as_str().unwrap(), e["hop"].as_u64().unwrap()), ("p1", "you", 0));
    let got = rec.got.lock().unwrap().clone(); assert_eq!(got.len(), 2);
    assert_eq!(got[0].2, "[message from you in #lab]\nhello all");
    assert_eq!(got[0].3, Some(("person".into(), "room".into(), "lab".into())));
    assert!(got[0].1.ends_with(".0") && got[1].1.ends_with(".1"));
    assert_eq!(rpc(&hub, "bots.post", json!({"room":"r1","text":"hello all","msg":"p1"})).await["ok"], true);
    let count = |k: &str| kinds(&hub, n).iter().filter(|x| x.as_str() == k).count();
    assert_eq!((count("RoomPosted"), count("BotWoke")), (1, 2), "a repeat stores and wakes nothing");
    assert_eq!(rpc(&hub, "bots.post", json!({"room":"nope","text":"x"})).await["error"], "no such room");
    assert_eq!(rpc(&hub, "bots.room_delete", json!({"room":"r1"})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.post", json!({"room":"r1","text":"x"})).await["error"], "no such room");
}

#[tokio::test] async fn bots_post_with_mail_on_files_it_and_wakes_nobody_here() {
    let (hub, _b, rec) = start("postmail", "").await;
    let a = bot(&hub, "aa").await; hub.commit(&json!({"$":"SettingSet","key":"mail.on","value":"on","at":1})).await.unwrap();
    rpc(&hub, "bots.room", json!({"room":"r1","name":"lab","members":a})).await;
    assert_eq!(rpc(&hub, "bots.post", json!({"room":"r1","text":"x"})).await["ok"], true);
    assert!(rec.got.lock().unwrap().is_empty());
}

#[test] fn bot_held_wakes_none() {
    // law bot_held_wakes_none: a message past the bound (Hop.held) is heard by no bot
    assert!(hop_live(0) && hop_live(HOP_MAX) && !hop_live(HOP_MAX + 1));
}

#[tokio::test] async fn bots_remember_normalises_and_forget_removes() {
    let (hub, bots, _r) = start("mem", "").await;
    assert_eq!(rpc(&hub, "bots.remember", json!({"bot":"b1","key":" My Key! ","kind":"PREF","text":"t","tags":"x"})).await["ok"], true);
    { let w = bots.world().await; let m = w.mems.get("b1/my-key-").expect("normalised key"); assert_eq!((m.kind.as_str(), m.text.as_str()), ("pref", "t")); }
    rpc(&hub, "bots.remember", json!({"bot":"b1","key":"k2","kind":"weird","text":"u"})).await;
    assert_eq!(bots.world().await.mems["b1/k2"].kind, "fact");
    assert_eq!(rpc(&hub, "bots.forget", json!({"bot":"b1","key":"k2"})).await["ok"], true);
    assert!(!bots.world().await.mems.contains_key("b1/k2"));
}

#[tokio::test] async fn bots_space_is_heard_as_a_persons_message() {
    let (hub, _b, rec) = start("space", "").await;
    let b = bot(&hub, "bo").await;
    assert_eq!(rpc(&hub, "bots.space", json!({"bot":b,"action":"go","value":"x"})).await["ok"], true);
    assert_eq!(rec.got.lock().unwrap()[0].2, "[space] go: x");
    rpc(&hub, "bots.space", json!({"bot":b,"action":"ping"})).await;
    assert_eq!(rec.got.lock().unwrap()[1].2, "[space] ping");
}

#[tokio::test] async fn bots_settings_keep_a_stored_empty_value_and_only_the_keys_read() {
    // M.Setting.get keeps a stored "": an agent.provider of "" gives "" here, not the default; other settings are not held at all
    let log0 = "{\"$\":\"SettingSet\",\"key\":\"agent.provider\",\"value\":\"\",\"at\":1}\n{\"$\":\"SettingSet\",\"key\":\"theme\",\"value\":\"dark\",\"at\":1}\n";
    let (hub, bots, _r) = start("settings", log0).await;
    assert_eq!(bots.world().await.setting("agent.provider", "claude"), ""); assert!(!bots.world().await.settings.contains_key("theme"));
    rpc(&hub, "bots.create", json!({"name":"bo"})).await;
    assert_eq!(log(&hub).iter().find(|e| e["$"] == "ThreadCreated").unwrap()["provider"], "");
}

#[test] fn world_fold_matches_bot() {
    use super::world::World;
    let mut w = World::default();
    for l in [r#"{"$":"BotSet","id":"b","name":"A","thread":"t1","look":1,"persona":"","at":5}"#, r#"{"$":"BotSet","id":"b","name":"A","thread":"","look":2,"persona":"x","at":6}"#] { w.apply_line(l) }
    let b = w.bot("b").unwrap(); assert_eq!((b.thread.as_str(), b.look, b.created), ("t1", 2, 5));
    w.apply_line(r#"{"$":"BotDeleted","id":"b","at":7}"#); assert!(!w.live("b"));
    w.apply_line(r#"{"$":"BotSet","id":"b","name":"A","thread":"","look":2,"persona":"x","at":9}"#); assert_eq!(w.bot("b").unwrap().created, 9);
    for l in [r#"{"$":"RoutineSet","id":"r","bot":"b","name":"n","cron":"c","prompt":"p","on":false,"at":10}"#, r#"{"$":"RoutineSlot","id":"r","slot":4,"at":11}"#, r#"{"$":"RoutineSet","id":"r","bot":"","name":"n","cron":"c","prompt":"p","on":true,"at":12}"#] { w.apply_line(l) }
    let r = &w.routines["r"]; assert_eq!((r.bot.as_str(), r.last, r.slot, r.created), ("b", 12, 0, 10));
    w.apply_line(r#"{"$":"RoutineDone","id":"r","at":13}"#); assert!(w.routines["r"].done);
    w.apply_line(r#"{"$":"RoutinePlan","id":"r","once":99,"tz":"person","kind":"remind","at":14}"#); let r = &w.routines["r"]; assert_eq!((r.done, r.once, r.kind.as_str()), (false, 99, "remind"));
}

#[test] fn name_seed_and_key_rules() {
    assert!(name_ok("a_b-9") && !name_ok("") && !name_ok("a.b"));
    assert_eq!(seed("Ada"), seed("ADA")); assert_eq!(mem_key("A b/C"), "a-b-c"); assert_eq!(mem_key(&"k".repeat(70)).len(), 64);
    assert_eq!(space_msg("a", ""), "[space] a"); assert_eq!(note("you", "dm:x", "n", "t"), "[message from you]\nt");
    assert_eq!(note_origin("[message from you]\nt"), Some(("bot:you".into(), "bot".into(), String::new())));
    assert_eq!(note_origin("[message from person@m1]\nt"), Some(("person".into(), "person".into(), String::new())));
    assert_eq!(note_origin("[message from ada in #lab]\nt"), Some(("bot:ada".into(), "room".into(), "lab".into())));
    assert_eq!(note_origin("hello"), None);
}

// ---- the same scenario as the JS hub's (test/bots/record_js.mjs), compared event by event ----
const JS_EVENTS: &str = include_str!("../../../../test/bots/fixtures/js-bots.jsonl");
const JS_STEPS: &str = include_str!("../../../../test/bots/fixtures/js-steps.json");

/// test/bots/filter.py: the events a bots scenario is about, without clocks
fn filter(ev: &[Value]) -> Vec<Value> {
    let threads: std::collections::HashSet<String> = ev.iter().filter(|e| e["$"] == "BotSet" && !e["thread"].as_str().unwrap_or("").is_empty()).map(|e| e["thread"].as_str().unwrap().to_string()).collect();
    const KEEP: [&str; 22] = ["BotSet", "BotDeleted", "RoomSet", "RoomDeleted", "RoomPosted", "BotWoke", "MemorySaved", "MemoryForgot", "SpaceSet", "WorktreeSet", "UberSet",
        "RoutineSet", "RoutinePlan", "RoutineFired", "RoutineSlot", "RoutineDone", "RoutineDeleted", "Reminded", "UberOp", "ConvSet", "MailPosted", "MailFile"];
    ev.iter().filter(|e| { let k = e["$"].as_str().unwrap_or(""); let th = e["thread"].as_str().unwrap_or("");
        let s = serde_json::to_string(e).unwrap_or_default();
        // what the JS hub's routing adds after a mail is filed (triage, escalations, claims) is slice 3: not compared
        if s.contains("triage-") || (k == "ThreadCreated" && (e["id"].as_str().unwrap_or("").starts_with("run") || e["id"].as_str().unwrap_or("").starts_with("mailcoord-"))) || (k == "ConvSet" && e["id"].as_str().unwrap_or("").starts_with("pair:")) || (k == "MailPosted" && e["conv"].as_str().unwrap_or("").starts_with("pair:"))
            || (k == "MessageFrom" && e["kind"] == "escalation") || (k == "SettingSet" && e["key"].as_str().unwrap_or("").starts_with("owner.mail.") && e["value"] == "") { return false }
        match k { "ThreadCreated" => e["project"] == "", "MessagePosted" => threads.contains(th) && e["role"] == "user", "MessageFrom" => threads.contains(th), "SettingSet" => e["key"].as_str().unwrap_or("").starts_with("owner.mail."),
            "WorktreeSet" => threads.contains(e["id"].as_str().unwrap_or("")), _ => KEEP.contains(&k) } })
        .map(|e| { let mut o = e.clone(); let m = o.as_object_mut().unwrap(); m.remove("at");
            for k in ["once", "slot"] { if m.get(k).and_then(Value::as_u64).map_or(false, |n| n > 0) { m.insert(k.into(), json!("<T>")); } } // instants are the run's own
            o }).collect()
}
/// ids the hub made (b/t/m + 10-digit clock + "-" + counter) become ordinals by first appearance; a worktree path loses its home
fn norm(ev: &[Value]) -> Vec<String> {
    let mut seen: Vec<String> = vec![];
    ev.iter().map(|e| {
        let s = serde_json::to_string(e).unwrap();
        let s = match s.rfind("/bots/") { Some(i) if s[..i].contains("\"worktree\":\"") => { let a = s.rfind("\"worktree\":\"").unwrap() + 12; format!("{}<home>{}", &s[..a], &s[i..]) } _ => s };
        let cs: Vec<char> = s.chars().collect(); let mut out = String::new(); let mut i = 0;
        while i < cs.len() {
            let c = cs[i];
            let ok = "btm".contains(c) && (i == 0 || !cs[i - 1].is_ascii_alphanumeric() || (cs[i - 1] == 'n' && i >= 2 && cs[i - 2] == '\\')) && cs.len() > i + 12 && cs[i + 1..i + 11].iter().all(|d| d.is_ascii_digit()) && cs[i + 11] == '-' && cs[i + 12].is_ascii_digit();
            if ok { let mut j = i + 12; while j < cs.len() && cs[j].is_ascii_digit() { j += 1 } let tok: String = cs[i..j].iter().collect();
                let n = match seen.iter().position(|x| *x == tok) { Some(p) => p, None => { seen.push(tok); seen.len() - 1 } }; out += &format!("<{c}#{n}>"); i = j }
            else { out.push(c); i += 1 }
        }
        out
    }).collect()
}

#[tokio::test] async fn js_parity() {
    let (hub, _b, _r) = start("parity", "").await;
    let steps: Vec<Value> = serde_json::from_str(JS_STEPS).unwrap();
    let mut ids: std::collections::HashMap<&str, String> = Default::default();
    let mut run = |i: usize, m: &'static str, p: Value| { let hub = hub.clone(); let want = steps[i].clone(); async move {
        let r = rpc(&hub, m, p).await;
        assert_eq!((r["ok"].as_bool().unwrap(), r.get("error").and_then(Value::as_str).map(str::to_string)), (want["ok"].as_bool().unwrap(), want["error"].as_str().map(str::to_string)), "step {} ({})", i, want["name"]);
        if m == "bots.create" && r["ok"] == true { let b = r["bot"].as_str().unwrap().to_string(); let th = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == b.as_str()).unwrap()["thread"].as_str().unwrap().to_string();
            until(&hub, |l| l.iter().any(|e| e["$"] == "WorktreeSet" && e["id"] == th.as_str())).await; }
        r } };
    let ada = run(0, "bots.create", json!({"name":"ada","persona":"dry"})).await["bot"].as_str().unwrap().to_string(); ids.insert("ada", ada.clone());
    run(1, "bots.create", json!({"name":"ADA"})).await;
    let bob = run(2, "bots.create", json!({"name":"bob"})).await["bot"].as_str().unwrap().to_string();
    run(3, "bots.create", json!({"name":"bad name"})).await;
    run(4, "bots.edit", json!({"bot":ada,"persona":"new"})).await;
    run(5, "bots.edit", json!({"bot":"nope"})).await;
    run(6, "bots.room", json!({"room":"r1","name":"lab","members":format!("{ada}\n{bob}\nzed@far")})).await;
    run(7, "bots.post", json!({"room":"r1","text":"hello all","msg":"p1"})).await;
    run(8, "bots.post", json!({"room":"r1","text":"hello all","msg":"p1"})).await;
    run(9, "bots.post", json!({"room":"nope","text":"x"})).await;
    run(10, "bots.say", json!({"bot":ada,"text":"hi","msg":"m1"})).await;
    run(11, "bots.say", json!({"bot":ada,"text":"hi","msg":"m1"})).await;
    run(12, "bots.remember", json!({"bot":ada,"key":" My Key! ","kind":"PREF","text":"t","tags":"x"})).await;
    run(13, "bots.remember", json!({"bot":ada,"key":"k2","kind":"weird","text":"u"})).await;
    run(14, "bots.forget", json!({"bot":ada,"key":"k2"})).await;
    run(15, "bots.space", json!({"bot":ada,"action":"go","value":"x"})).await;
    run(16, "bots.delete", json!({"bot":bob})).await;
    run(17, "bots.delete", json!({"bot":bob})).await;
    run(18, "bots.say", json!({"bot":bob,"text":"x"})).await;
    run(19, "bots.post", json!({"room":"r1","text":"second","msg":"p2"})).await;
    run(20, "bots.room_delete", json!({"room":"r1"})).await;
    run(21, "bots.post", json!({"room":"r1","text":"x"})).await;
    let js: Vec<Value> = JS_EVENTS.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let ours = filter(&log(&hub));
    let (a, b) = (norm(&js), norm(&ours));
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() { assert_eq!(x, y, "event {i}: JS left, Rust right") }
    assert_eq!(a.len(), b.len(), "same number of events\nJS: {a:#?}\nRust: {b:#?}");
}

const JS_ROUTINE_EVENTS: &str = include_str!("../../../../test/bots/fixtures/js-routines.jsonl");
const JS_ROUTINE_STEPS: &str = include_str!("../../../../test/bots/fixtures/js-routines-steps.json");

#[tokio::test] async fn js_parity_routines() {
    std::env::set_var("TZ", "Etc/UTC"); // the recording's hub ran in UTC
    let (hub, _b, _r) = start("parity-routines", "").await;
    let steps: Vec<Value> = serde_json::from_str(JS_ROUTINE_STEPS).unwrap();
    let step = |i: usize, r: &Value| { let want = &steps[i]; assert_eq!((r["ok"].as_bool().unwrap(), r.get("error").and_then(Value::as_str).map(str::to_string)), (want["ok"].as_bool().unwrap(), want["error"].as_str().map(str::to_string)), "step {} ({})", i, want["name"]) };
    let ada = bot(&hub, "ada").await;
    let mut i = 1;
    for (m, p) in [
        ("bots.routine", json!({"bot":ada,"routine":"ro1","name":"morning","schedule":"daily 09:30","prompt":"good morning"})),
        ("bots.routine", json!({"bot":ada,"name":"x","schedule":"whenever","prompt":"p"})),
        ("bots.routine", json!({"bot":ada,"name":"x","prompt":"p"})),
        ("bots.routine", json!({"bot":ada,"name":"x","schedule":"daily 09:30","at":"in 1h","prompt":"p"})),
        ("bots.routine", json!({"bot":ada,"name":"x","schedule":"daily 09:30","tz":"not a zone","prompt":"p"})),
        ("bots.routine", json!({"bot":ada,"name":"x","at":"banana","prompt":"p"})),
        ("bots.routine", json!({"bot":"nope","name":"x","schedule":"daily 09:30","prompt":"p"})),
        ("bots.routine", json!({"bot":ada,"routine":"ro2","name":"soon","at":"in 3s","prompt":"once please"})),
        ("bots.routine", json!({"bot":ada,"routine":"ro3","name":"hourly","schedule":"@hourly","tz":"UTC","prompt":"tick"})),
        ("bots.routine", json!({"bot":ada,"routine":"ro3","name":"hourly","prompt":"tick","on":false,"keep":true})),
        ("bots.routine_run", json!({"routine":"ro1"})),
        ("bots.routine_run", json!({"routine":"ro2"})),
        ("bots.routine_delete", json!({"routine":"ro1"})),
        ("bots.routine_run", json!({"routine":"ro1"})),
        ("bots.routine_run", json!({"routine":"zz"})),
    ] { let r = rpc(&hub, m, p).await; step(i, &r); i += 1; }
    let js: Vec<Value> = JS_ROUTINE_EVENTS.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let (a, b) = (norm(&js), norm(&filter(&log(&hub))));
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() { assert_eq!(x, y, "event {i}: JS left, Rust right") }
    assert_eq!(a.len(), b.len(), "same number of events\nJS: {a:#?}\nRust: {b:#?}");
}

#[tokio::test] async fn routines_tick_fires_what_is_due_once_and_a_reminder_alerts() {
    std::env::set_var("TZ", "Etc/UTC");
    let (hub, bots, rec) = start("routine-tick", "").await;
    let ada = bot(&hub, "ada").await; let th = bots.world().await.bot(&ada).unwrap().thread.clone();
    // a one-time routine at a past instant is due; it fires once, then is done
    let past = 1_000_000u64;
    for e in [json!({"$":"RoutineSet","id":"o1","bot":ada,"name":"once","cron":"","prompt":"p1","on":true,"at":past}), json!({"$":"RoutinePlan","id":"o1","once":past + 10,"tz":"person","kind":"","at":past})] { hub.commit(&e).await.unwrap(); }
    assert_eq!(bots.tick_at(past + 20).await, 1); assert_eq!(bots.tick_at(past + 21).await, 0, "done is never due again");
    { let got = rec.got.lock().unwrap(); assert_eq!(got.len(), 1); assert_eq!(got[0].2, "[routine: once]\np1"); assert_eq!(got[0].3, Some(("routine:once".into(), "routine".into(), "once".into()))); }
    assert!(log(&hub).iter().any(|e| e["$"] == "RoutineDone" && e["id"] == "o1"));
    // a repeating routine fires once per slot
    for e in [json!({"$":"RoutineSet","id":"d1","bot":ada,"name":"daily","cron":"30 9 * * *","prompt":"p2","on":true,"at":past}), json!({"$":"RoutinePlan","id":"d1","once":0,"tz":"person","kind":"","at":past})] { hub.commit(&e).await.unwrap(); }
    let day = (past / 86400 + 1) * 86400 + 9 * 3600 + 30 * 60; // 09:30 UTC the next day
    assert_eq!(bots.tick_at(day - 1).await, 0); assert_eq!(bots.tick_at(day).await, 1); assert_eq!(bots.tick_at(day + 30).await, 0, "one fire per slot");
    // a reminder: no agent turn, an alert record for its thread
    for e in [json!({"$":"RoutineSet","id":"r1","bot":ada,"name":"water","cron":"","prompt":"drink\twater","on":true,"at":past}), json!({"$":"RoutinePlan","id":"r1","once":past + 10,"tz":"person","kind":"remind","at":past})] { hub.commit(&e).await.unwrap(); }
    let n = rec.got.lock().unwrap().len();
    assert_eq!(bots.tick_at(past + 30).await, 1);
    assert_eq!(rec.got.lock().unwrap().len(), n, "a reminder wakes no one");
    let ev = log(&hub); let rem = ev.iter().find(|e| e["$"] == "Reminded").unwrap();
    assert_eq!((rem["id"].as_str().unwrap(), rem["thread"].as_str().unwrap(), rem["text"].as_str().unwrap()), ("r1", th.as_str(), "drink\twater"));
    let al = ev.iter().rev().find(|e| e["$"] == "SettingSet" && e["key"] == format!("alert.{th}").as_str()).unwrap()["value"].as_str().unwrap().to_string();
    let f: Vec<&str> = al.split('\t').collect(); assert_eq!((f[0], f[1], f[4]), ("reminder", "remind-r1", "drink water"));
}

#[test] fn routines_plan_reads_like_bots_plan() {
    use super::clock::Zones; use super::routines::{plan, Pick};
    use std::collections::HashMap;
    let mut s: HashMap<String, String> = HashMap::new();
    s.insert("person.tz".into(), "Europe/Helsinki".into()); s.insert("tz.rule.Europe/Helsinki".into(), "EET-2EEST,M3.5.0/3,M10.5.0/4".into());
    let z = Zones { settings: &s };
    assert_eq!(plan(&z, 1791957600, "", " daily 09:30 ", "", None), Pick::Plan { cron: "30 9 * * *".into(), once: 0, tz: "person".into() });
    assert_eq!(plan(&z, 1791957600, "machine", "@hourly", "", None), Pick::Plan { cron: "0 * * * *".into(), once: 0, tz: String::new() });
    assert_eq!(plan(&z, 1791957600, "", "", "tomorrow 9am", None), Pick::Plan { cron: String::new(), once: 1792044000, tz: "person".into() });
    assert!(matches!(plan(&z, 1, "Mars/Base", "", "tomorrow 9am", None), Pick::Plan { .. }) == false);
    assert_eq!(plan(&z, 1, "Mars/Base", "", "tomorrow 9am", None), Pick::Bad("I don't have that time zone's rules yet; try again in a minute (or give the time with its offset, or as in 20m)".into()));
    assert_eq!(plan(&z, 1, "Mars/Base", "", "in 5m", None), Pick::Plan { cron: String::new(), once: 301, tz: "Mars/Base".into() });
}

async fn th_a(bots: &Arc<Bots>, b: &str) -> String { bots.world().await.bot(b).unwrap().thread.clone() }
#[tokio::test] async fn session_asks_the_chat_thread_and_the_default_tab() {
    // U.Bots.chat.of and BU.tab.default for a view whose target is a bot
    let (hub, bots, _r) = start("api", "").await;
    let (a, u) = (bot(&hub, "aa").await, bot(&hub, "uu").await);
    let th = bots.world().await.bot(&a).unwrap().thread.clone();
    assert_eq!(chat_thread(&hub, &a).await, Some(th)); assert_eq!(chat_thread(&hub, "nope").await, None);
    assert_eq!(default_tab(), "chat");
    hub.commit(&json!({"$":"UberSet","bot":u,"uid":"u1-x","at":5})).await.unwrap();
    // the page: Chat unless a tab this bot has is asked for; the uberbot's bar folds Status under Mailroom; a bot nobody knows has none
    let p = crate::bots::bot_page(&hub, &u, "", 10).await.unwrap(); assert_eq!((p["tab"].as_str(), p["navTab"].as_str(), p["uber"].as_bool()), (Some("chat"), Some("chat"), Some(true)));
    let p = crate::bots::bot_page(&hub, &u, "status", 10).await.unwrap(); assert_eq!((p["tab"].as_str(), p["navTab"].as_str(), p["subtabs"].as_array().unwrap().len()), (Some("status"), Some("mail"), 2));
    let p = crate::bots::bot_page(&hub, &a, "status", 10).await.unwrap(); assert_eq!(p["tab"], "chat", "a plain bot has no Status");
    assert!(crate::bots::bot_page(&hub, "nope", "", 10).await.is_none());
    hub.commit(&json!({"$":"MemorySaved","bot":a,"key":"k","kind":"fact","text":"t","tags":"","at":5})).await.unwrap();
    let p = crate::bots::bot_page(&hub, &a, "memory", 10).await.unwrap(); assert_eq!(p["memory"].as_array().unwrap().len(), 1); assert_eq!(p["memoryMore"], 0); assert_eq!(p["chat"], th_a(&bots, &a).await);
    // the Memory tab pages: 200 memories of ~1 KB are three pages; each page starts where the last ended, Newer comes back, every memory is on exactly one page
    for i in 0..200 { hub.commit(&json!({"$":"MemorySaved","bot":a,"key":format!("m{i:03}"),"kind":"fact","text":"x".repeat(1000),"tags":"","at":100 + i})).await.unwrap(); }
    let mut seen = std::collections::BTreeSet::new(); let mut from = 0usize; let mut pages = 0;
    loop {
        let p = crate::bots::bot_page_from(&hub, &a, "memory", 1000, from).await.unwrap(); pages += 1;
        assert_eq!(p["memoryFrom"], from); assert_eq!(p["memoryTotal"], 201);
        for r in p["memory"].as_array().unwrap() { assert!(seen.insert(r["key"].as_str().unwrap().to_string()), "once"); }
        assert!(serde_json::to_string(&p).unwrap().len() < 80 * 1024, "a page stays under the reply limit");
        if from > 0 { assert!(p["memoryPrev"].is_u64()); } else { assert!(p["memoryPrev"].is_null()); }
        match p["memoryNext"].as_u64() { Some(n) => { assert!(n as usize > from); from = n as usize } None => { assert_eq!(p["memoryMore"], 0); break } }
    }
    assert_eq!((seen.len(), pages >= 3), (201, true), "{pages} pages");
    let back = crate::bots::bot_page_from(&hub, &a, "memory", 1000, from).await.unwrap(); let prev = back["memoryPrev"].as_u64().unwrap() as usize;
    assert_eq!(crate::bots::bot_page_from(&hub, &a, "memory", 1000, prev).await.unwrap()["memoryNext"], from, "Newer then Older is the same page boundary");
    // Webhooks: revoking one takes it off the tab; an unknown hook is refused
    hub.commit(&json!({"$":"HookSet","id":"hk1","bot":a,"name":"ci","revoked":false,"at":6})).await.unwrap();
    assert_eq!(crate::bots::bot_page(&hub, &a, "hooks", 10).await.unwrap()["hooks"].as_array().unwrap().len(), 1);
    assert_eq!(rpc(&hub, "bots.hook_revoke", json!({"hook":"nope"})).await["ok"], false);
    assert_eq!(rpc(&hub, "bots.hook_revoke", json!({"hook":"hk1"})).await["ok"], true);
    assert_eq!(crate::bots::bot_page(&hub, &a, "hooks", 10).await.unwrap()["hooks"].as_array().unwrap().len(), 0);
    // the pages that were "not drawn yet": Status (the plane as text), Mail (headers), Space (the bot's blocks), Settings (persona)
    let p = crate::bots::bot_page(&hub, &u, "status", 10).await.unwrap(); assert!(p["plane"].is_string(), "Status carries the plane as text");
    let p = crate::bots::bot_page(&hub, &u, "mail", 10).await.unwrap(); assert!(p["mail"].is_array() && p["mailTotal"].is_u64(), "Mail carries its conversation headers");
    let p = crate::bots::bot_page(&hub, &u, "space", 10).await.unwrap(); assert!(p["space"].is_null() && p["spacePage"] == false, "no space yet");
    hub.commit(&json!({"$":"SpaceSet","bot":u,"spec":"{\"title\":\"T\",\"blocks\":[{\"type\":\"heading\",\"text\":\"H\"}]}","at":6})).await.unwrap();
    let p = crate::bots::bot_page(&hub, &u, "space", 10).await.unwrap(); assert_eq!(p["space"]["blocks"][0]["text"], "H");
    let p = crate::bots::bot_page(&hub, &u, "settings", 10).await.unwrap(); assert!(p["persona"].is_string() && p["created"].is_string());
    let uth = bots.world().await.bot(&u).unwrap().thread.clone(); assert_eq!(chat_thread(&hub, &u).await, Some(uth));
}

/// the run's own machine id, uberbot id, stamp seconds and one-time instants inside op bodies, as test/bots/filter.py makes them
fn canon(raw: &[Value]) -> Vec<String> {
    let me = raw.iter().rev().find(|e| e["$"] == "SettingSet" && e["key"] == "machine.id").and_then(|e| e["value"].as_str()).unwrap_or("").to_string();
    let uid = raw.iter().find(|e| e["$"] == "UberSet" && !e["uid"].as_str().unwrap_or("").is_empty()).and_then(|e| e["uid"].as_str()).unwrap_or("").to_string();
    norm(&filter(raw)).into_iter().map(|mut s| {
        if !me.is_empty() { s = s.replace(&me, "<me>") } if !uid.is_empty() { s = s.replace(&uid, "<uid>") }
        let cs: Vec<char> = s.chars().collect(); let mut o = String::new(); let mut i = 0;
        while i < cs.len() { // 10 digits . 6 digits . <me>  ->  <S>.dddddd.<me>
            let st: String = cs[i..].iter().take(25).collect();
            if i + 10 <= cs.len() && cs[i..i + 10].iter().all(|c| c.is_ascii_digit()) && (i == 0 || !cs[i - 1].is_ascii_digit()) && st.len() >= 21 && &st[10..11] == "." && st[11..17].bytes().all(|b| b.is_ascii_digit()) && st[17..].starts_with(".<me>") { o += "<S>.<c>"; i += 17 } else { o.push(cs[i]); i += 1 }
        }
        let mut res = String::new(); let mut rest = o.as_str(); // once\":N inside an op body
        while let Some(p) = rest.find("once\\\":") { let (a, b) = rest.split_at(p + 7); res += a; let d = b.bytes().take_while(u8::is_ascii_digit).count(); if d > 0 && !b.starts_with('0') { res += "<T>"; rest = &b[d..] } else { rest = b } }
        res + rest
    }).collect()
}

const JS_UBER_EVENTS: &str = include_str!("../../../../test/bots/fixtures/js-uber.jsonl");
const JS_UBER_STEPS: &str = include_str!("../../../../test/bots/fixtures/js-uber-steps.json");

#[tokio::test] async fn js_parity_uber() {
    std::env::set_var("TZ", "Etc/UTC");
    let (hub, bots, _r) = start("parity-uber", "").await;
    let steps: Vec<Value> = serde_json::from_str(JS_UBER_STEPS).unwrap();
    let step = |i: usize, r: &Value| { let want = &steps[i]; assert_eq!((r["ok"].as_bool().unwrap(), r.get("error").and_then(Value::as_str).map(str::to_string)), (want["ok"].as_bool().unwrap(), want["error"].as_str().map(str::to_string)), "step {} ({})", i, want["name"]) };
    let ada = bot(&hub, "ada").await;
    let r = rpc(&hub, "bots.create", json!({"name":"bub","uber":true})).await; step(1, &r); let bub = r["bot"].as_str().unwrap().to_string();
    until(&hub, |l| l.iter().any(|e| e["$"] == "UberSet")).await;
    until(&hub, |l| l.iter().any(|e| e["$"] == "WorktreeSet" && e["id"] != "x") && l.iter().filter(|e| e["$"] == "WorktreeSet").count() >= 2).await;
    step(2, &rpc(&hub, "bots.uber", json!({"bot":bub})).await);
    step(3, &rpc(&hub, "bots.uber", json!({"bot":"p1~b9"})).await);
    assert!(bots.uber_init(now_s()).await > 0, "the first ops");   // the minute tick
    assert_eq!(bots.uber_init(now_s()).await, 0, "only once");
    let mut i = 4;
    for (m, p) in [
        ("bots.remember", json!({"bot":bub,"key":"Home Town","kind":"fact","text":"Oulu","tags":"x"})),
        ("bots.remember", json!({"bot":ada,"key":"k","kind":"note","text":"t"})),
        ("bots.forget", json!({"bot":bub,"key":"Home Town"})),
        ("bots.routine", json!({"bot":bub,"routine":"ro1","name":"later","at":"in 3d","prompt":"p"})),
        ("bots.routine", json!({"bot":bub,"routine":"ro2","name":"daily","schedule":"daily 09:30","prompt":"q"})),
        ("bots.pin", json!({"routine":"ro2","pin":"here"})),
        ("bots.pin", json!({"routine":"ro2","pin":""})),
        ("bots.pin", json!({"routine":"zz","pin":"here"})),
        ("bots.routine_delete", json!({"routine":"ro1"})),
        ("bots.edit", json!({"bot":bub,"persona":"calm"})),
        ("bots.uber_home", json!({})),
        ("bots.uber_join", json!({"uid":"u1-x"})),
    ] { let r = rpc(&hub, m, p).await; step(i, &r); i += 1; }
    let js: Vec<Value> = JS_UBER_EVENTS.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let a: Vec<String> = js.iter().map(|e| serde_json::to_string(e).unwrap()).collect();
    let a = norm(&js).into_iter().zip(a).map(|(n, _)| n).collect::<Vec<_>>();
    let b = canon(&log(&hub));
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() { assert_eq!(x, y, "event {i}: JS left, Rust right") }
    assert_eq!(a.len(), b.len(), "same number of events\nJS: {a:#?}\nRust: {b:#?}");
}

#[test] fn uber_stamps_are_later_than_the_last_whatever_the_clock_says() {
    // Ub.stamp.next: a later second starts the counter again; the same or an earlier one counts on from the last
    use super::uber::stamp_next;
    assert_eq!(stamp_next("", 100, "m"), "0000000100.000000.m");
    assert_eq!(stamp_next("0000000100.000000.m", 100, "m"), "0000000100.000001.m");
    assert_eq!(stamp_next("0000000100.000007.m", 90, "m"), "0000000100.000008.m");
    assert_eq!(stamp_next("0000000100.000007.m", 101, "m"), "0000000101.000000.m");
}

const JS_UBER2_EVENTS: &str = include_str!("../../../../test/bots/fixtures/js-uber2.jsonl");

#[tokio::test] async fn js_parity_uber_first_ops_of_a_bot_with_memories_and_routines() {
    // a bot that has memories (newest first) and routines (by id) before it is promoted: Ub.init.bodies
    std::env::set_var("TZ", "Etc/UTC");
    let (hub, bots, _r) = start("parity-uber2", "").await;
    let bub = rpc(&hub, "bots.create", json!({"name":"bub","persona":"dry"})).await["bot"].as_str().unwrap().to_string();
    until(&hub, |l| l.iter().any(|e| e["$"] == "WorktreeSet")).await;
    let pause = || tokio::time::sleep(std::time::Duration::from_millis(1200));
    rpc(&hub, "bots.remember", json!({"bot":bub,"key":"k1","kind":"fact","text":"one","tags":"a"})).await; pause().await;
    rpc(&hub, "bots.remember", json!({"bot":bub,"key":"k2","kind":"pref","text":"two","tags":""})).await; pause().await;
    rpc(&hub, "bots.remember", json!({"bot":bub,"key":"k3","kind":"note","text":"three","tags":"c"})).await; pause().await;
    rpc(&hub, "bots.remember", json!({"bot":bub,"key":"k1","kind":"fact","text":"one!","tags":"a"})).await; pause().await;
    rpc(&hub, "bots.routine", json!({"bot":bub,"routine":"rz","name":"zed","at":"in 3d","prompt":"z"})).await;
    rpc(&hub, "bots.routine", json!({"bot":bub,"routine":"ra","name":"alpha","schedule":"daily 09:30","prompt":"a"})).await;
    rpc(&hub, "bots.routine", json!({"bot":bub,"routine":"rm","name":"mid","schedule":"weekdays 08:00","prompt":"m"})).await;
    rpc(&hub, "bots.routine", json!({"bot":bub,"routine":"rb","name":"beta","schedule":"every 2h","prompt":"b","on":false})).await;
    assert_eq!(rpc(&hub, "bots.uber", json!({"bot":bub})).await["ok"], true);
    until(&hub, |l| l.iter().any(|e| e["$"] == "UberSet")).await;
    assert_eq!(bots.uber_init(now_s()).await, 10);
    assert_eq!(rpc(&hub, "bots.uber_home", json!({})).await["ok"], true);
    let js: Vec<Value> = JS_UBER2_EVENTS.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let (a, b) = (norm(&js), canon(&log(&hub)));
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() { assert_eq!(x, y, "event {i}: JS left, Rust right") }
    assert_eq!(a.len(), b.len(), "same number of events\nJS: {a:#?}\nRust: {b:#?}");
}

#[tokio::test] async fn uber_first_ops_carry_space_and_page_between_memories_and_routines() {
    // Ub.init.bot: who, memories, space, page, routines, lease (term 1), bind; the expected text is text
    let (hub, bots, _r) = start("uber-sp", "").await;
    let bub = bot(&hub, "bub").await; let th = bots.world().await.bot(&bub).unwrap().thread.clone();
    rpc(&hub, "bots.remember", json!({"bot":bub,"key":"k","kind":"fact","text":"t","tags":""})).await;
    hub.commit_many(&[json!({"$":"SpaceSet","bot":bub,"spec":"{\"title\":\"Home\"}","at":5}), json!({"$":"SpacePageSet","bot":bub,"html":"<p>hi</p>","at":6})]).await.unwrap();
    rpc(&hub, "bots.routine", json!({"bot":bub,"routine":"r1","name":"n","schedule":"daily 09:30","prompt":"p"})).await;
    hub.commit(&json!({"$":"UberSet","bot":bub,"uid":"u7-aa","at":9})).await.unwrap();
    assert_eq!(bots.uber_init(1_800_000_000).await, 7);
    let me = bots.world().await.me(); assert!(!me.is_empty());
    let ops: Vec<Value> = log(&hub).into_iter().filter(|e| e["$"] == "UberOp").collect();
    let bodies: Vec<&str> = ops.iter().map(|e| e["op"].as_str().unwrap()).collect();
    assert_eq!(bodies, vec![
        r#"{"k":"who","name":"bub","look":0,"persona":""}"#.replace("\"look\":0", &format!("\"look\":{}", seed("bub"))).as_str(),
        r#"{"k":"mem","key":"k","kind":"fact","text":"t","tags":""}"#,
        r#"{"k":"space","spec":"{\"title\":\"Home\"}"}"#,
        r#"{"k":"page","html":"<p>hi</p>"}"#,
        format!(r#"{{"k":"routine","id":"{me}:r1","name":"n","cron":"30 9 * * *","prompt":"p","on":true,"pin":"","once":0,"tz":"person","kind":""}}"#).as_str(),
        format!(r#"{{"k":"lease","holder":"{me}","term":1}}"#).as_str(),
        format!(r#"{{"k":"bind","machine":"{me}","bot":"{bub}","thread":"{th}","name":"bub"}}"#).as_str(),
    ]);
    assert_eq!((ops[0]["n"].as_u64(), ops[0]["origin"].as_str(), ops[0]["stamp"].as_str().unwrap()), (Some(1), Some(me.as_str()), format!("1800000000.000000.{me}").as_str()));
    assert_eq!(ops[6]["stamp"], format!("1800000000.000006.{me}").as_str());
}

#[tokio::test] async fn uber_join_makes_a_replica_bot_tied_to_the_offered_uid() {
    // Ub.joined: ThreadCreated, BotSet, UberSet, the offer cleared, then the bind op as this machine's first op under the uid
    let (hub, bots, _r) = start("uber-join", "").await;
    hub.commit(&json!({"$":"SettingSet","key":"uber.offer","value":"{\"uid\":\"u9-x\",\"name\":\"kit\",\"look\":7,\"machine\":\"m2\",\"link\":\"l1\"}","at":3})).await.unwrap();
    let n = log(&hub).len();
    let r = rpc(&hub, "bots.uber_join", json!({"uid":"u9-x"})).await; assert_eq!(r["ok"], true); let b = r["bot"].as_str().unwrap().to_string();
    let ev = log(&hub); let all: Vec<&Value> = ev[n..].iter().filter(|e| e["$"] != "WorktreeSet").collect();
    assert!(all[1]["$"] == "SettingSet" && all[1]["key"].as_str().unwrap().starts_with("account.thread."), "the thread's account binding follows its ThreadCreated");
    let new: Vec<&Value> = all.iter().enumerate().filter(|(i, _)| *i != 1).map(|(_, e)| *e).collect();
    let kinds: Vec<&str> = new.iter().map(|e| e["$"].as_str().unwrap()).collect();
    assert_eq!(kinds, vec!["ThreadCreated", "BotSet", "UberSet", "SettingSet", "UberOp"]);
    assert_eq!((new[0]["title"].as_str().unwrap(), new[0]["project"].as_str().unwrap()), ("kit", ""));
    assert_eq!((new[1]["id"].as_str().unwrap(), new[1]["look"].as_u64().unwrap(), new[1]["persona"].as_str().unwrap(), new[1]["thread"].as_str().unwrap()), (b.as_str(), 7, "", new[0]["id"].as_str().unwrap()));
    assert_eq!((new[2]["bot"].as_str().unwrap(), new[2]["uid"].as_str().unwrap()), (b.as_str(), "u9-x"));
    assert_eq!((new[3]["key"].as_str().unwrap(), new[3]["value"].as_str().unwrap()), ("uber.offer", ""));
    let me = bots.world().await.me();
    assert_eq!((new[4]["uid"].as_str().unwrap(), new[4]["origin"].as_str().unwrap(), new[4]["n"].as_u64().unwrap()), ("u9-x", me.as_str(), 1));
    assert_eq!(new[4]["op"], format!(r#"{{"k":"bind","machine":"{me}","bot":"{b}","thread":"{}","name":"kit"}}"#, new[0]["id"].as_str().unwrap()).as_str());
    assert_eq!(bots.world().await.uber_bot(), b);
    // the offer is spent; a second join is refused as an uberbot already here
    assert_eq!(rpc(&hub, "bots.uber_join", json!({"uid":"u9-x"})).await["error"], "that uberbot is not on offer here");
    // a name taken: offer again under another uid with the name of a bot here
    bot(&hub, "kit2").await;
    hub.commit(&json!({"$":"SettingSet","key":"uber.offer","value":"{\"uid\":\"u10-y\",\"name\":\"kit2\",\"look\":1}","at":4})).await.unwrap();
    assert_eq!(rpc(&hub, "bots.uber_join", json!({"uid":"u10-y"})).await["error"], "this machine has an uberbot already; make it not the uberbot first");
}

#[tokio::test] async fn uber_home_moves_the_lease_one_term_on_from_another_machines() {
    // Ub.home: with another machine holding the lease at term 2, this machine's lease op is term 3 and its stamp is later than the last one seen
    let (hub, bots, _r) = start("uber-home", "").await;
    let bub = bot(&hub, "bub").await;
    hub.commit(&json!({"$":"UberSet","bot":bub,"uid":"u5-z","at":9})).await.unwrap();
    hub.commit(&json!({"$":"UberOp","uid":"u5-z","origin":"mX","n":1,"stamp":"4102444800.000004.mX","op":"{\"k\":\"lease\",\"holder\":\"mX\",\"term\":2}","at":9})).await.unwrap();
    { let w = bots.world().await; assert_eq!(w.lease(), ("mX".to_string(), 2)); assert!(!w.holder_me()); }
    let n = log(&hub).len();
    assert_eq!(rpc(&hub, "bots.uber_home", json!({})).await["ok"], true);
    let op = &log(&hub)[n]; let me = bots.world().await.me();
    assert_eq!((op["$"].as_str().unwrap(), op["origin"].as_str().unwrap(), op["n"].as_u64().unwrap()), ("UberOp", me.as_str(), 1));
    assert_eq!(op["op"], format!(r#"{{"k":"lease","holder":"{me}","term":3}}"#).as_str());
    assert_eq!(op["stamp"], format!("4102444800.000005.{me}").as_str(), "later than the other machine's stamp whatever the clock says");
    assert_eq!(log(&hub).len(), n + 1);
    { let w = bots.world().await; assert_eq!(w.lease(), (me.clone(), 3)); assert!(w.holder_me()); }
    assert_eq!(rpc(&hub, "bots.uber_home", json!({})).await["ok"], true); assert_eq!(log(&hub).len(), n + 1, "already the holder: nothing");
}

const JS_MAIL1_EVENTS: &str = include_str!("../../../../test/bots/fixtures/js-mail1.jsonl");
const JS_MAIL1_STEPS: &str = include_str!("../../../../test/bots/fixtures/js-mail1-steps.json");

#[tokio::test] async fn js_parity_mail_filing() {
    // the mailroom on: a room post is filed, a routine's firing is filed instead of delivered; the uberbot's routine is filed with an owner ticket. Compared with a JS-hub recording
    // (events, and the journal files line by line) up to the post in the uberbot's room: there the JS hub filed nothing, so the uberbot never heard it, and this hub files it on purpose.
    std::env::set_var("TZ", "Etc/UTC");
    let (hub, bots, rec) = start("parity-mail1", "").await;
    let steps: Vec<Value> = serde_json::from_str(JS_MAIL1_STEPS).unwrap();
    let step = |i: usize, r: &Value| { let want = &steps[i]; assert_eq!((r["ok"].as_bool().unwrap(), r.get("error").and_then(Value::as_str).map(str::to_string)), (want["ok"].as_bool().unwrap(), want["error"].as_str().map(str::to_string)), "step {} ({})", i, want["name"]) };
    let ada = bot(&hub, "ada").await; let bob = bot(&hub, "bob").await;
    hub.commit(&json!({"$":"SettingSet","key":"mail.on","value":"on","at":1})).await.unwrap();
    step(3, &rpc(&hub, "bots.room", json!({"room":"r1","name":"lab","members":format!("{ada}\n{bob}")})).await);
    step(4, &rpc(&hub, "bots.post", json!({"room":"r1","text":"hello all","msg":"p1"})).await);
    step(5, &rpc(&hub, "bots.post", json!({"room":"r1","text":"hello all","msg":"p1"})).await);
    step(6, &rpc(&hub, "bots.routine", json!({"bot":ada,"routine":"ro1","name":"later","at":"in 3d","prompt":"do it"})).await);
    step(7, &rpc(&hub, "bots.routine_run", json!({"routine":"ro1"})).await);
    // with the mailroom on the bot's agent is not told directly: its mail starts a run, whose brief is the only delivery
    for _ in 0..100 { if !rec.got.lock().unwrap().is_empty() { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    assert!(rec.got.lock().unwrap().iter().all(|g| g.0.starts_with("run") && g.2.starts_with("<charter>")), "only a run's brief is delivered");
    hub.commit(&json!({"$":"SettingSet","key":"mail.on","value":"off","at":2})).await.unwrap();
    let r = rpc(&hub, "bots.create", json!({"name":"bub","uber":true})).await; step(9, &r); let bub = r["bot"].as_str().unwrap().to_string();
    until(&hub, |l| l.iter().any(|e| e["$"] == "UberSet")).await;
    until(&hub, |l| l.iter().filter(|e| e["$"] == "WorktreeSet").count() >= 3).await;
    assert!(bots.uber_init(now_s()).await > 0);
    step(10, &rpc(&hub, "bots.room", json!({"room":"r2","name":"ops","members":format!("{bub}\n{ada}")})).await);
    step(11, &rpc(&hub, "bots.post", json!({"room":"r2","text":"status?","msg":"p2"})).await);
    step(12, &rpc(&hub, "bots.routine", json!({"bot":bub,"routine":"ro2","name":"morning","at":"in 3d","prompt":"plan the day"})).await);
    step(13, &rpc(&hub, "bots.routine_run", json!({"routine":"ro2"})).await);
    let js: Vec<Value> = JS_MAIL1_EVENTS.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let mut ours = log(&hub);
    let dir = hub.cfg.home.join("mail"); let mut names: Vec<String> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default(); names.sort();
    for n in names.iter().filter(|n| n.ends_with(".jsonl")) { let lines: Vec<String> = std::fs::read_to_string(dir.join(n)).unwrap().lines().map(str::to_string).collect(); ours.push(json!({"$":"MailFile","name":n,"lines":lines})); }
    let (a, b): (Vec<String>, Vec<String>) = (norm(&js).iter().map(|s| unclock(s)).collect(), canon(&ours).iter().map(|s| unclock(s)).collect());
    let at = |v: &[String]| v.iter().position(|s| s.contains("RoomPosted") && s.contains("\"p2\"")).expect("the post in the uberbot's room");
    assert_eq!(a[..=at(&a)], b[..=at(&b)], "JS left, Rust right");
    let file = |v: &[String], n: &str| v.iter().find(|s| s.contains("MailFile") && s.contains(n)).cloned();
    assert_eq!(file(&a, "room%3Ar1.jsonl"), file(&b, "room%3Ar1.jsonl")); assert!(file(&a, "routine%3Aro1.jsonl").is_some()); assert_eq!(file(&a, "routine%3Aro1.jsonl"), file(&b, "routine%3Aro1.jsonl"));
    assert!(file(&a, "room%3Ar2.jsonl").is_none() && file(&b, "room%3Ar2.jsonl").is_some(), "the post in the uberbot's room is filed here");
    let l = log(&hub); assert!(l.iter().any(|e| e["$"] == "RunSet" && e["conv"] == "room:r2" && e["bot"] == bub.as_str()), "and wakes a triage run");
    assert!(l.iter().any(|e| e["$"] == "MailPosted" && e["conv"] == "routine:ro2") && l.iter().any(|e| e["$"] == "SettingSet" && e["key"].as_str().unwrap_or("").starts_with("owner.mail.pending.")), "the uberbot's routine: filed with its ticket");
}
/// the run's clock inside a record (a ten-digit second, and the log length in an owner ticket's stamp)
fn unclock(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect(); let mut o = String::new(); let mut i = 0;
    while i < cs.len() {
        if cs[i].is_ascii_digit() && (i == 0 || !cs[i - 1].is_ascii_alphanumeric()) { let mut j = i; while j < cs.len() && cs[j].is_ascii_digit() { j += 1 }
            if j - i == 10 { o += "<T>"; i = j; if i < cs.len() && cs[i] == ':' { o.push(':'); i += 1; let k = i; while i < cs.len() && cs[i].is_ascii_digit() { i += 1 } if i > k { o += "<n>" } } continue } }
        o.push(cs[i]); i += 1
    }
    o
}

mod mail_tests {
    use super::super::mail::*;
    use super::*;
    fn o(f: &str, k: &str, r: &str) -> Origin { Origin::new(f, k, r) }
    #[test] fn mail_route_names_the_conversation() {
        // Mail.route_kind: the conversation a mail belongs to, by the origin's kind
        assert_eq!(route(&o("thread:b", "thread", ""), "bot:a"), "pair:bot:a|thread:b");
        assert_eq!(route(&o("bot:a", "bot", ""), "bot:b"), "pair:bot:a|bot:b");
        assert_eq!(route(&o("x", "room", "lab"), "room:lab"), "room:lab");
        assert_eq!(route(&o("x", "room", "dm:ada:you"), "room:dm:ada:you"), "pair:bot:ada|person");
        assert_eq!(route(&o("thread:t1", "launch", ""), "bot:a"), "launch:t1");
        assert_eq!(route(&o("thread:t1", "task", ""), "bot:a"), "launch:t1");
        assert_eq!(route(&o("thread:t1", "wake", ""), "bot:a"), "launch:t1");
        assert_eq!(route(&o("thread:t1", "wake", "w9"), "bot:a"), "wake:w9");
        assert_eq!(route(&o("routine:r", "routine", "r"), "bot:a"), "routine:r");
        assert_eq!(route(&o("hook:h", "hook", "h"), "bot:a"), "hook:h");
        assert_eq!(route(&o("peer:m2", "peer", "x"), "bot:a"), "peer:m2:x");
        assert_eq!(route(&o("backplane", "deploy", ""), "bot:a"), "system:a");
        assert_eq!(route(&o("person", "person", ""), "bot:a"), "");
    }
    #[test] fn mail_classify_reads_where_a_message_came_from() {
        assert_eq!(classify("", "m1", false, "[message from you in #lab]\nx"), o("person", "room", "lab"));
        assert_eq!(classify("", "m1", false, "[message from ada in #lab]\nx"), o("bot:ada", "room", "lab"));
        assert_eq!(classify("", "m1", false, "[message from ada]\nx"), o("bot:ada", "bot", ""));
        assert_eq!(classify("", "m1", false, "[message from person@m2]\nx"), o("person", "person", ""));
        assert_eq!(classify("", "m1", false, "[message from kit@m2]\nx"), o("peer:kit@m2", "peer", ""));
        assert_eq!(classify("", "m1", false, "[routine: later]\nx"), o("routine:later", "routine", "later"));
        assert_eq!(classify("", "m1", false, "[webhook: gh]\nx"), o("hook:gh", "hook", "gh"));
        assert_eq!(classify("", "m1", false, "Backplane restarted while you were working"), o("backplane", "carry", ""));
        assert_eq!(classify("", "t:backplane:l1", true, "x"), o("backplane", "lost", ""));
        assert_eq!(classify("deploy", "m1", true, "x"), o("backplane", "deploy", ""));
        assert_eq!(classify("lock:merge", "m1", true, "x"), o("backplane", "lock", "merge"));
        assert_eq!(classify("t9", "t:t9:td-1", false, "x"), o("thread:t9", "task", ""));
        assert_eq!(classify("t9", "t:t9:lr-1", false, "x"), o("thread:t9", "launch", ""));
        assert_eq!(classify("", "m1", false, "hello"), o("person", "person", ""));
        assert_eq!(classify("", "m1", true, "hello"), o("backplane", "system", ""));
        assert_eq!(classify("bot:ada", "m1", false, "hello"), o("bot:ada", "bot", ""));
    }
    #[test] fn mail_want_head_and_sizes() {
        let f = |k: &str, t: &str| flags(&o("x", k, ""), t);
        assert_eq!(want("room", "", &f("room", "t")), "fyi"); assert_eq!(want("routine", "", &f("routine", "t")), "act"); assert_eq!(want("launch", "", &f("launch", "it completed")), "fyi");
        assert_eq!(want("launch", "", &f("launch", "failed")), "act"); assert_eq!(want("person", "", &f("person", "t")), "act"); assert_eq!(want("task", "", &f("task", "t")), "act");
        assert_eq!(want("room", "act", &f("room", "t")), "act"); assert_eq!(want("routine", "fyi", &f("routine", "t")), "fyi");
        assert_eq!(head("a\tb\r\nrest"), "a b "); assert_eq!(head(&"x".repeat(200)).len(), 160); assert_eq!(bytes("é€"), 5);
    }
    #[test] fn mail_body_line_is_what_writes_and_reads_back() {
        let b = Body { conv: Conv { id: "room:r1".into(), subject: "lab".into(), kind: "room".into(), parts: vec!["bot:b".into(), "person".into()], project: String::new(), tags: vec![], made: 5, last: 5, n: 0 },
            mail: Hdr { id: "p1".into(), conv: "room:r1".into(), from: "person".into(), to: "room:r1".into(), kind: "room".into(), r#ref: "r1".into(), want: "fyi".into(), head: "h".into(), size: 3, at: 5 }, text: "a\"b\\c\nd\u{1}é".into() };
        let line = body_line(&b);
        assert_eq!(line, "{\"v\":1,\"conv\":{\"id\":\"room:r1\",\"subject\":\"lab\",\"kind\":\"room\",\"parts\":[\"bot:b\",\"person\"],\"project\":\"\",\"tags\":[],\"made\":5,\"last\":5,\"n\":0},\"mail\":{\"id\":\"p1\",\"conv\":\"room:r1\",\"from\":\"person\",\"to\":\"room:r1\",\"kind\":\"room\",\"ref\":\"r1\",\"want\":\"fyi\",\"head\":\"h\",\"size\":3,\"at\":5},\"text\":\"a\\\"b\\\\c\\nd\\u0001é\"}\n");
        assert_eq!(body_parse(&line), Some(b));
        assert_eq!(body_parse("{\"v\":2}"), None); assert_eq!(pct("pair:bot:b1|bot:b2"), "pair%3Abot%3Ab1%7Cbot%3Ab2"); assert_eq!(pct("é"), "%C3%A9");
    }
    #[tokio::test] async fn mail_recover_gives_a_journaled_body_its_header_once() {
        let (hub, bots, _r) = start("mail-recover", "").await;
        let b = Body { conv: Conv { id: "routine:r9".into(), subject: "s".into(), kind: "routine".into(), parts: vec!["routine:r9".into(), "bot:x".into()], project: String::new(), tags: vec![], made: 7, last: 7, n: 0 },
            mail: Hdr { id: "m-lost".into(), conv: "routine:r9".into(), from: "routine:r9".into(), to: "bot:x".into(), kind: "routine".into(), r#ref: "r9".into(), want: "act".into(), head: "h".into(), size: 1, at: 7 }, text: "t".into() };
        store::append(&hub.cfg.home, &b).unwrap();
        // a torn last write is cut off by the next append, and does not count
        { use std::io::Write; let mut f = std::fs::OpenOptions::new().append(true).open(store::path(&hub.cfg.home, "routine:r9")).unwrap(); f.write_all(b"{\"v\":1,\"conv").unwrap(); }
        assert_eq!(bots.mail_recover().await, 1);
        let ev = log(&hub); assert_eq!(ev.iter().filter(|e| e["$"] == "MailPosted" && e["id"] == "m-lost").count(), 1); assert!(ev.iter().any(|e| e["$"] == "ConvSet" && e["id"] == "routine:r9"));
        assert_eq!(bots.mail_recover().await, 0, "once");
        let mut b2 = b.clone(); b2.mail.id = "m-two".into(); assert_eq!(store::append(&hub.cfg.home, &b2), Ok(true), "the torn line was repaired");
        assert_eq!(store::read(&store::path(&hub.cfg.home, "routine:r9")).len(), 2);
    }
}

mod store_tests {
    use super::super::mail::*;
    use super::*;
    fn body(conv: &str, id: &str) -> Body {
        Body { conv: Conv { id: conv.into(), subject: "s".into(), kind: "pair".into(), parts: vec![], project: String::new(), tags: vec![], made: 1, last: 1, n: 0 },
            mail: Hdr { id: id.into(), conv: conv.into(), from: "a".into(), to: "b".into(), kind: "bot".into(), r#ref: String::new(), want: "fyi".into(), head: "h".into(), size: 1, at: 1 }, text: "SECRET".into() }
    }
    fn home(name: &str) -> std::path::PathBuf { let h = std::env::temp_dir().join(format!("bp-store-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&h); std::fs::create_dir_all(&h).unwrap(); h }
    #[test] fn journal_reads_skip_a_line_that_is_not_text_and_open_without_blocking() {
        use std::io::Write;
        let h = home("nonutf"); let p = store::path(&h, "c1"); std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let good = body_line(&body("c1", "m1"));
        let mut f = std::fs::File::create(&p).unwrap(); f.write_all(good.as_bytes()).unwrap(); f.write_all(b"\n\xff\xfe broken\n").unwrap(); f.write_all(good.as_bytes()).unwrap(); f.write_all(b"\n").unwrap();
        assert_eq!(store::read(&p).len(), 2, "the bytes that are no text cost one line, not the rest of the file");
        assert_eq!(store::headers(&p).len(), 2);
        // a FIFO in a journal's place is refused, never waited on
        let q = store::path(&h, "c2"); assert!(std::process::Command::new("mkfifo").arg(&q).status().unwrap().success());
        assert!(store::read(&q).is_empty() && store::headers(&q).is_empty());
    }
    #[test] fn the_journal_is_not_opened_through_a_link() {
        use crate::plat::fs::symlink;
        let h = home("links"); let outside = home("links-outside");
        // a link in the journal's place: refused, and what it points at is not written
        std::fs::create_dir_all(h.join("mail")).unwrap(); let target = outside.join("elsewhere"); std::fs::write(&target, b"").unwrap();
        symlink(&target, store::path(&h, "c1")).unwrap();
        assert_eq!(store::append(&h, &body("c1", "m1")), Err(store::StoreError::Link)); assert_eq!(std::fs::read(&target).unwrap().len(), 0);
        assert!(store::read(&store::path(&h, "c1")).is_empty() && store::headers(&store::path(&h, "c1")).is_empty(), "nor read through it");
        // a second name for a journal: refused for a write
        assert_eq!(store::append(&h, &body("c2", "m1")), Ok(false)); std::fs::hard_link(store::path(&h, "c2"), h.join("mail").join("alias")).unwrap();
        assert_eq!(store::append(&h, &body("c2", "m2")), Err(store::StoreError::Hardlink)); assert_eq!(store::read(&store::path(&h, "c2")).len(), 1);
        // not a regular file
        std::fs::create_dir(store::path(&h, "c3")).unwrap(); assert_eq!(store::append(&h, &body("c3", "m1")), Err(store::StoreError::NotRegular));
        // the directory itself a link
        let h2 = home("links-dir"); symlink(&outside, h2.join("mail")).unwrap();
        assert_eq!(store::append(&h2, &body("c4", "m1")), Err(store::StoreError::Link)); assert!(!outside.join("c4.jsonl").exists());
        // errors say the kind, never the conversation
        assert!(!format!("{}", store::StoreError::Link).contains("c1"));
    }
    #[test] fn headers_drop_the_text_and_a_new_journal_is_private() {
        use crate::plat::fs::PermissionsExt;
        let h = home("hdr"); store::append(&h, &body("c", "m1")).unwrap(); store::append(&h, &body("c", "m2")).unwrap();
        let hs = store::headers(&store::path(&h, "c")); assert_eq!(hs.iter().map(|(_, m)| m.id.as_str()).collect::<Vec<_>>(), vec!["m1", "m2"]);
        assert_eq!(crate::plat::fs::mode_bits(std::fs::metadata(store::path(&h, "c")).unwrap().permissions().mode() & 0o777, 0o600), 0o600);
    }
}

// ---- the bots' MCP tools against the recorded JS hub (test/bots/record_tools.mjs) ----
fn tool_norm(v: &Value) -> Value {
    fn id_like(s: &str) -> bool { let b = s.as_bytes(); b.len() > 12 && "brotp".contains(b[0] as char) && b[1..11].iter().all(u8::is_ascii_digit) && b[11] == b'-' }
    fn go(k: &str, v: &Value) -> Value {
        match v {
            Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), go(k, v))).collect()),
            Value::Array(a) => Value::Array(a.iter().map(|x| go(k, x)).collect()),
            Value::Number(n) if ["at", "updated", "last"].contains(&k) && n.as_u64().map_or(false, |x| x > 1_000_000_000) => json!(0),
            Value::String(s) if id_like(s) => json!("ID"),
            Value::String(s) if ["in", "next", "localTime", "iso"].contains(&k) => json!(if s.is_empty() { "" } else { "TIME" }),
            Value::String(s) => { // dates inside sentences: "Thu 5 Nov 14:29 UTC", "next Fri 6 Nov 09:30 UTC"
                let mut out = String::new(); let w: Vec<&str> = s.split(' ').collect(); let mut i = 0;
                while i < w.len() {
                    let day = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].contains(&w[i]);
                    if day && i + 3 < w.len() && w[i + 3].len() == 5 && w[i + 3].as_bytes()[2] == b':' { out.push_str("DATE"); i += 4; if i < w.len() && w[i] == "UTC" { i += 1 } } else { out.push_str(w[i]); i += 1 }
                    if i < w.len() { out.push(' ') }
                }
                json!(out.trim_end().to_string())
            }
            _ => v.clone(),
        }
    }
    go("", v)
}
#[tokio::test] async fn js_bot_tools() {
    // every call the JS hub answered in the recording, replayed on a hub with the same bots: the same answers (ids, times and dates aside)
    let calls: Vec<Value> = serde_json::from_str(include_str!("../../../../test/bots/fixtures/js-bot-calls.json")).unwrap();
    let (hub, bots, rec) = start("tools", "{\"$\":\"SettingSet\",\"key\":\"tz.machine\",\"value\":\"Etc/UTC\",\"at\":1}\n{\"$\":\"ProjectCreated\",\"id\":\"p1700000000-1\",\"title\":\"fixture\",\"root\":\"/home/h/Documents/i2cjak/backplane-gpui/build/gpui/results/bots/rec6/work/fixroot\",\"at\":1700000000}\n").await;
    let (ada, _bob) = (bot(&hub, "ada").await, bot(&hub, "bob").await);
    let th = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == ada.as_str()).unwrap()["thread"].as_str().unwrap().to_string();
    let mut room = String::new(); let mut n = 0;
    for c in &calls {
        let name = c["name"].as_str().unwrap();
        if !tools::owns(name) { continue }
        let mut args = c["args"].clone();
        if let Some(o) = args.as_object_mut() { for v in o.values_mut() { if v.as_str().map_or(false, |s| s.starts_with('r') && s.contains('-') && s[1..].chars().next().map_or(false, |x| x.is_ascii_digit())) { *v = json!(room) } } }
        let (ok, body) = bots.tool_call(&th, name, &args).await;
        let want: Value = serde_json::from_str(c["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(c["result"]["isError"].as_bool().unwrap(), !ok, "{name} {args}");
        assert_eq!(tool_norm(&body), tool_norm(&want), "{name} {args}");
        if name == "room_list" && room.is_empty() { room = body[0]["id"].as_str().unwrap().to_string() }
        if name == "person_timezone" && args["zone"] == "Europe/Helsinki" { // the settings lane resolves the zone and sets it
            hub.commit(&json!({"$":"SettingSet","key":"person.tz","value":"Europe/Helsinki","at":1})).await.unwrap();
        }
        n += 1;
    }
    assert!(n >= 38, "{n} calls replayed");
    // the words that reached bob: the request in its room, the private word as a direct message
    let got = rec.got.lock().unwrap().clone();
    assert!(got.iter().any(|g| g.2 == "[message from ada in #greet]\nhi bob"), "{got:?}");
    assert!(got.iter().any(|g| g.2 == "[message from ada]\na private word"), "{got:?}");
}
#[tokio::test] async fn bot_tools_listed_in_the_js_order_and_only_for_bots() {
    let names: Vec<String> = tools::defs().iter().map(|d| d["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(names, tools::NAMES);
    let (_hub, bots, _r) = start("tools-nobot", "").await;
    assert_eq!(bots.tool_call("t-none", "bot_whoami", &json!({})).await, (false, json!({"error":"unknown tool"})));
}
#[tokio::test] async fn bot_messages_past_the_depth_bound_are_held_and_wake_nobody() {
    // BT.Hop: a message goes out one hop past the turn that sent it, never past 6; the seventh is kept in its room for the person
    let (hub, bots, rec) = start("tools-held", "").await;
    let (ada, bob) = (bot(&hub, "ada").await, bot(&hub, "bob").await);
    let lt = |b: &str| log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == b).unwrap()["thread"].as_str().unwrap().to_string();
    let (ta, tb) = (lt(&ada), lt(&bob));
    let mut hop = 0;
    for i in 1..=7u64 {
        let (from, to) = if i % 2 == 1 { (&ta, "bob") } else { (&tb, "ada") };
        let before = rec.got.lock().unwrap().len();
        let (ok, body) = bots.tool_call(from, "bot_send", &json!({"to":to,"text":format!("m{i}"),"direct":true})).await;
        assert!(ok, "{body}");
        let woke = rec.got.lock().unwrap().len() > before;
        if i <= 6 { assert!(woke && body["held"].is_null(), "message {i}: {body}"); hop = i } else { assert!(!woke && body["held"] == true, "message {i}: {body}") }
        // the person's turn does not end between them: the hop the last woken bot stands at is what the next message goes past
        let _ = hop;
    }
    assert!(log(&hub).iter().any(|e| e["$"] == "RoomPosted" && e["hop"] == 7 && e["text"].as_str().unwrap().starts_with("[held: 6 messages")));
}
#[tokio::test] async fn bot_request_in_room() {
    // law bot_request_in_room: a request goes in a room the person can follow, never a dm; a found room is used only when it is not dm:, and a new id is not dm: either.
    // (bot_send with direct:true is the one way into a dm: room, and says so.)
    let (hub, bots, _r) = start("tools-req", "").await;
    let (ada, bob) = (bot(&hub, "ada").await, bot(&hub, "bob").await);
    let th = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == ada.as_str()).unwrap()["thread"].as_str().unwrap().to_string();
    // a dm: room that carries the topic's name and both members is there to be found
    hub.commit(&json!({"$":"RoomSet","id":"dm:ada:bob","name":"greet","members":format!("{ada}\n{bob}"),"at":5})).await.unwrap();
    let (ok, r) = bots.tool_call(&th, "bot_send", &json!({"to":"bob","text":"one","topic":"greet"})).await;
    assert!(ok, "{r}"); let room = r["room"].as_str().unwrap().to_string();
    assert!(!room.starts_with("dm:"), "the request went to {room}");
    // the same request again is kept in the room it made
    let (_, r2) = bots.tool_call(&th, "bot_send", &json!({"to":"bob","text":"two","topic":"greet"})).await;
    assert_eq!(r2["room"], room.as_str());
    // direct is the dm: path, and the request path never reuses it
    let (_, d) = bots.tool_call(&th, "bot_send", &json!({"to":"bob","text":"private","direct":true})).await; assert_eq!(d["note"], "sent to bob");
    assert!(log(&hub).iter().any(|e| e["$"] == "RoomPosted" && e["room"].as_str().map_or(false, |r| r.starts_with("dm:")) && e["text"] == "private"));
    assert!(log(&hub).iter().filter(|e| e["$"] == "RoomPosted" && (e["text"] == "one" || e["text"] == "two")).all(|e| e["room"] == room.as_str()));
}
#[tokio::test(flavor = "multi_thread")] async fn person_timezone_looks_up_an_unknown_zone_and_sets_it() {
    let d = std::env::temp_dir().join(format!("bp-zi-bots-{}", std::process::id())); std::fs::create_dir_all(d.join("Europe")).unwrap();
    std::fs::write(d.join("Europe/Helsinki"), b"TZif2\0\0binary\n\nEET-2EEST,M3.5.0/3,M10.5.0/4\n").unwrap(); std::env::set_var("BACKPLANE_ZONEINFO", &d);
    let (hub, bots, _r) = start("tools-zone", "").await;
    hub.register_rpc(Arc::new(crate::settings::zone::ZoneRpc));
    let ada = bot(&hub, "ada").await;
    let th = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == ada.as_str()).unwrap()["thread"].as_str().unwrap().to_string();
    let (ok, r) = bots.tool_call(&th, "person_timezone", &json!({"zone":"Europe/Helsinki"})).await;
    assert!(ok && r["note"].as_str().unwrap().starts_with("looking up Europe/Helsinki"), "{r}");
    for _ in 0..100 { if hub.setting("person.tz").await.as_deref() == Some("Europe/Helsinki") { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    assert_eq!(hub.setting("person.tz").await.as_deref(), Some("Europe/Helsinki"));
    assert_eq!(hub.setting("tz.rule.Europe/Helsinki").await.as_deref(), Some("EET-2EEST,M3.5.0/3,M10.5.0/4"));
}
#[tokio::test] async fn mail_requests_list_read_find_mark_tag_send() {
    // Mail.request.ui: always on; a send to a bot is filed in the journal and listed, read, found, marked and tagged
    let (hub, _bots, _r) = start("mailreq", "").await;
    let ada = bot(&hub, "ada").await;
    assert_eq!(rpc(&hub, "mail.send", json!({"to":"bot:nobody","text":"x"})).await["error"], "invalid mail target");
    assert_eq!(rpc(&hub, "mail.send", json!({"to":format!("bot:{ada}"),"text":"hello ada\nsecond line"})).await["ok"], true);
    let l = rpc(&hub, "mail.list", json!({})).await["result"].clone();
    assert_eq!(l["total"], 1); let c = &l["convs"][0]; let cid = c["id"].as_str().unwrap().to_string();
    assert_eq!((c["head"].as_str().unwrap(), c["from"].as_str().unwrap(), c["n"].clone(), c["status"].as_str().unwrap()), ("hello ada", "person", json!(1), ""));
    let bl = rpc(&hub, "mail.list", json!({"bot":ada})).await["result"].clone();
    assert_eq!((bl["total"].clone(), bl["convs"][0]["status"].as_str().unwrap(), bl["convs"][0]["unread"].clone()), (json!(1), "unread", json!(true)));
    assert_eq!(rpc(&hub, "mail.list", json!({"bot":ada,"status":"done"})).await["result"]["total"], 0);
    let f = rpc(&hub, "mail.find", json!({"q":"HELLO"})).await["result"].clone(); assert_eq!(f["total"], 1);
    assert_eq!(rpc(&hub, "mail.find", json!({"q":"zzz"})).await["result"]["total"], 0);
    let r = rpc(&hub, "mail.read", json!({"conv":cid,"bot":ada})).await["result"].clone();
    assert_eq!((r["mails"][0]["text"].as_str().unwrap(), r["more"].clone(), r["conv"]["status"].as_str().unwrap()), ("hello ada\nsecond line", json!(false), "open"));
    assert_eq!(rpc(&hub, "mail.read", json!({"conv":cid,"before":"nope"})).await["ok"], false);
    assert_eq!(rpc(&hub, "mail.mark", json!({"bot":ada,"conv":cid,"status":"bogus"})).await["error"], "invalid mail status");
    assert_eq!(rpc(&hub, "mail.mark", json!({"bot":ada,"conv":cid,"status":"done"})).await["ok"], true);
    assert_eq!(rpc(&hub, "mail.tag", json!({"bot":ada,"conv":cid,"tags":["a","b"]})).await["ok"], true);
    let bl = rpc(&hub, "mail.list", json!({"bot":ada,"status":"done"})).await["result"].clone();
    assert_eq!((bl["total"].clone(), bl["convs"][0]["tags"].clone()), (json!(1), json!(["a", "b"])));
    assert_eq!(rpc(&hub, "mail.find", json!({"bot":ada,"q":"b"})).await["result"]["total"], 1);
}
#[tokio::test] async fn mail_paging_scopes_filters_and_sends_to_a_plain_thread() {
    let (hub, _bots, rec) = start("mailpage", "").await;
    let ada = bot(&hub, "ada").await;
    hub.commit(&json!({"$":"ProjectCreated","id":"p1","title":"proj","root":"/x","at":1})).await.unwrap();
    hub.commit(&json!({"$":"ThreadCreated","id":"tw","project":"p1","title":"work","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2})).await.unwrap();
    // a send to a plain thread: delivered to it as the person's, and filed (a thread: address, the project of the thread)
    assert_eq!(rpc(&hub, "mail.send", json!({"to":"thread:tw","text":"do the thing","subject":"S"})).await["ok"], true);
    assert!(rec.got.lock().unwrap().iter().any(|g| g.0 == "tw" && g.2 == "do the thing" && g.3.is_none()));
    let l = rpc(&hub, "mail.list", json!({})).await["result"].clone(); assert_eq!(l["total"], 1); assert_eq!((l["convs"][0]["subject"].as_str().unwrap(), l["convs"][0]["project"].as_str().unwrap()), ("S", "p1"));
    let tl = rpc(&hub, "mail.list", json!({"bot":"thread:tw"})).await["result"].clone(); assert_eq!(tl["total"], 1);
    assert_eq!(rpc(&hub, "mail.list", json!({"bot":"thread:zz"})).await["result"]["total"], 0);
    assert_eq!(rpc(&hub, "mail.list", json!({"project":"p1"})).await["result"]["total"], 1);
    assert_eq!(rpc(&hub, "mail.list", json!({"project":"nope"})).await["result"]["total"], 0);
    // a conversation with 5 mails, read two at a time; and 210 conversations to skip and cap
    let cid = l["convs"][0]["id"].as_str().unwrap().to_string();
    for i in 0..4 { assert_eq!(rpc(&hub, "mail.send", json!({"to":"thread:tw","conv":cid,"text":format!("m{i}")})).await["ok"], true) }
    let mut before = String::new(); let mut got: Vec<String> = vec![]; let mut mores = vec![];
    loop {
        let r = rpc(&hub, "mail.read", json!({"conv":cid,"before":before,"n":2})).await["result"].clone();
        let ms = r["mails"].as_array().unwrap().clone(); mores.push(r["more"].as_bool().unwrap());
        got.extend(ms.iter().map(|m| m["text"].as_str().unwrap().to_string())); before = ms.last().unwrap()["id"].as_str().unwrap().to_string();
        if !r["more"].as_bool().unwrap() { break }
    }
    assert_eq!(got, ["m3", "m2", "m1", "m0", "do the thing"]); assert_eq!(mores, [true, true, false], "a full page with older mail says more, the last does not");
    assert_eq!(rpc(&hub, "mail.read", json!({"conv":cid,"n":0})).await["result"]["mails"], json!([]));
    for i in 0..210 { assert_eq!(rpc(&hub, "mail.send", json!({"to":format!("bot:{ada}"),"conv":format!("c{i}"),"text":"x"})).await["ok"], true) }
    let a = rpc(&hub, "mail.list", json!({"n":1000})).await["result"].clone();
    assert_eq!((a["total"].clone(), a["convs"].as_array().unwrap().len()), (json!(211), 200), "n is capped at 200");
    let s = rpc(&hub, "mail.list", json!({"skip":205,"n":10})).await["result"].clone(); assert_eq!(s["convs"].as_array().unwrap().len(), 6);
    assert_eq!(rpc(&hub, "mail.list", json!({"n":3})).await["result"]["convs"].as_array().unwrap().len(), 3);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn two_sends_in_one_tick_get_two_message_ids_and_both_are_delivered() {
    // the id is decided before the commit: two sends of one second used to share `mail<now>-<len>`, and the second delivery was skipped with an ok
    let (hub, bots, rec) = start("sameid", "").await;
    let _ada = bot(&hub, "ada").await;
    hub.commit(&json!({"$":"ThreadCreated","id":"tw","project":"","title":"work","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":2})).await.unwrap();
    let (a, b) = tokio::join!(rpc(&hub, "mail.send", json!({"to":"thread:tw","text":"one"})), rpc(&hub, "mail.send", json!({"to":"thread:tw","text":"two"})));
    assert_eq!((a["ok"].clone(), b["ok"].clone()), (json!(true), json!(true)));
    let got = rec.got.lock().unwrap().clone();
    let ids: std::collections::HashSet<_> = got.iter().filter(|g| g.0 == "tw").map(|g| g.1.clone()).collect();
    assert_eq!(got.iter().filter(|g| g.0 == "tw").count(), 2, "{got:?}"); assert_eq!(ids.len(), 2, "two ids: {ids:?}");
    assert_eq!(rpc(&hub, "mail.list", json!({})).await["result"]["total"], 1, "one conversation, two mails");
    let c = rpc(&hub, "mail.list", json!({})).await["result"]["convs"][0]["n"].clone(); assert_eq!(c, 2);
    // ids asked for in one second are distinct whatever the log's length
    let ids: std::collections::HashSet<String> = (0..50).map(|_| bots.hub_id("x", 5)).collect(); assert_eq!(ids.len(), 50);
}
#[tokio::test] async fn mail_read_of_a_room_conversation_pages_its_posts() {
    // MailJob.room.*: with the mailroom off a room post is a header, its text read from the RoomPosted line; newest first, a cursor, `more`, unknown cursor/conversation
    let (hub, _bots, _r) = start("roomread", "").await;
    let (ada, bob) = (bot(&hub, "ada").await, bot(&hub, "bob").await);
    assert_eq!(rpc(&hub, "bots.room", json!({"room":"r1","name":"lab","members":format!("{ada}\n{bob}")})).await["ok"], true);
    for i in 0..5 { assert_eq!(rpc(&hub, "bots.post", json!({"room":"r1","text":format!("post {i}"),"msg":format!("p{i}")})).await["ok"], true) }
    let r = rpc(&hub, "mail.read", json!({"conv":"room:r1","n":2})).await["result"].clone();
    let t = |r: &Value| r["mails"].as_array().unwrap().iter().map(|m| m["text"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(t(&r), ["post 4", "post 3"]); assert_eq!(r["more"], true); assert_eq!(r["conv"]["subject"], "lab");
    assert_eq!(r["mails"][0]["from"], "person"); assert_eq!(r["mails"][0]["kind"], "room");
    let r2 = rpc(&hub, "mail.read", json!({"conv":"room:r1","n":2,"before":"p3"})).await["result"].clone(); assert_eq!((t(&r2), r2["more"].clone()), (vec!["post 2".to_string(), "post 1".to_string()], json!(true)));
    let r3 = rpc(&hub, "mail.read", json!({"conv":"room:r1","n":2,"before":"p1"})).await["result"].clone(); assert_eq!((t(&r3), r3["more"].clone()), (vec!["post 0".to_string()], json!(false)));
    assert_eq!(rpc(&hub, "mail.read", json!({"conv":"room:r1","before":"nope"})).await["error"], "room:r1: unknown room conversation or mail cursor");
    assert_eq!(rpc(&hub, "mail.read", json!({"conv":"room:zz"})).await["error"], "room:zz: unknown room conversation or mail cursor");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_mail_read_does_not_hold_the_world_lock_for_its_file_work() {
    // a deep page of a big room walks the log and the journal; meanwhile other bot requests must go through. The test starts a read of a 4000-post room's oldest page and
    // runs bots.edit requests (which take the world lock) until the read ends; it asserts the read finished and that at least one edit went through while it ran.
    let (hub, _bots, _r) = start("readlock", "").await;
    let (ada, bob) = (bot(&hub, "ada").await, bot(&hub, "bob").await);
    assert_eq!(rpc(&hub, "bots.room", json!({"room":"r1","name":"lab","members":format!("{ada}\n{bob}")})).await["ok"], true);
    let evs: Vec<Value> = (0..4000).map(|i| json!({"$":"RoomPosted","id":format!("q{i}"),"room":"r1","from":"you","text":format!("post {i}"),"hop":0,"at":10})).collect();
    hub.commit_many(&evs).await.unwrap();
    let h2 = hub.clone();
    let read = tokio::spawn(async move { let t = std::time::Instant::now(); let r = rpc(&h2, "mail.read", json!({"conv":"room:r1","n":3,"before":"q3"})).await; (r, t.elapsed()) });
    let mut said = 0;
    while !read.is_finished() && said < 200 { assert_eq!(rpc(&hub, "bots.edit", json!({"bot":ada,"persona":"x"})).await["ok"], true); said += 1; tokio::task::yield_now().await }
    let (r, took) = read.await.unwrap();
    assert_eq!(r["ok"], true, "{r}"); assert_eq!(r["result"]["mails"][0]["text"], "post 2");
    eprintln!("read took {took:?}, {said} edits went through meanwhile");
    assert!(said > 0, "the read was over before any other request ran: the room is too small to show it");
}
struct ToHolder { holder: Arc<Bots>, as_link: String, sent: StdMutex<Vec<String>> }
impl mail::owner::OwnerLink for ToHolder {
    fn post<'a>(&'a self, link: &'a str, body: String) -> Fut<'a, Option<(u16, String)>> {
        Box::pin(async move { self.sent.lock().unwrap().push(link.to_string()); if link != "linkB" { return None } let o: Value = serde_json::from_str(&body).unwrap(); Some(self.holder.owner_received(&self.as_link, &o).await) })
    }
}
/// Walk A's outbox until `f` holds for its pending tickets (a walk already going makes ours return at once; the clock only bounds a hang).
async fn walk_until(b: &Arc<Bots>, f: impl Fn(&[Value]) -> bool) { for _ in 0..800 { b.owner_outbox().await; if f(&b.owner_records().await) { return } tokio::time::sleep(std::time::Duration::from_millis(25)).await } panic!("the outbox never got there") }
struct Pair { hb: Arc<Hub>, bb: Arc<Bots>, ha: Arc<Hub>, ba: Arc<Bots>, ua: String, ubb: String, uid: String, ame: String, bme: String }
/// machine B holds the uberbot's conversation; machine A has a replica of it (same uid, the lease held by B) and the two know each other's links
async fn owner_pair(tag: &str) -> Pair {
    std::env::set_var("TZ", "Etc/UTC");
    let (hb, bb, _r) = start(&format!("owner{tag}B"), "").await;
    let r = rpc(&hb, "bots.create", json!({"name":"bub","uber":true})).await; assert_eq!(r["ok"], true);
    until(&hb, |l| l.iter().any(|e| e["$"] == "UberSet") && l.iter().any(|e| e["$"] == "WorktreeSet")).await;
    assert!(bb.uber_init_now(now_s()).await > 0);
    let lb = log(&hb); let uid = lb.iter().find(|e| e["$"] == "UberSet").unwrap()["uid"].as_str().unwrap().to_string();
    let bme = hb.setting("machine.id").await.unwrap(); let ubb = lb.iter().find(|e| e["$"] == "UberSet").unwrap()["bot"].as_str().unwrap().to_string();
    let (ha, ba, _ra) = start(&format!("owner{tag}A"), "").await;
    let ua = bot(&ha, "ua").await; let ame = ha.setting("machine.id").await.unwrap(); assert_ne!(ame, bme);
    let mut evs: Vec<Value> = vec![json!({"$":"UberSet","bot":ua,"uid":uid,"at":5})];
    evs.extend(lb.iter().filter(|e| e["$"] == "UberOp").cloned());
    evs.push(json!({"$":"SettingSet","key":format!("uber.link.{uid}.{bme}"),"value":"linkB","at":6}));
    ha.commit_many(&evs).await.unwrap();
    hb.commit(&json!({"$":"SettingSet","key":format!("uber.link.{uid}.{ame}"),"value":"linkA","at":6})).await.unwrap();
    Pair { hb, bb, ha, ba, ua, ubb, uid, ame, bme }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn owner_mail_moves_waiting_mail_to_the_holder_and_is_acknowledged_once() {
    let Pair { hb, bb, ha, ba, ua, ubb, uid, ame, bme } = owner_pair("main").await;
    let link = Arc::new(ToHolder { holder: bb.clone(), as_link: "linkA".into(), sent: Default::default() });
    ba.set_owner_link(link.clone());
    // mail for the uberbot arrives on A: filed, with a pending ticket
    assert_eq!(rpc(&ha, "mail.send", json!({"to":format!("bot:{ua}"),"text":"hello owner\nmore"})).await["ok"], true);
    let pend = ba.owner_records().await; assert_eq!(pend.len(), 1, "{pend:?}"); let r = pend[0].clone(); assert_eq!(r["id"].as_str().unwrap().starts_with("mail"), true);
    // a walk of the background timer or owner_kick may hold the walker, and ours then returns at once: walk until the step has happened, not once
    walk_until(&ba, |r| r.is_empty() || r[0]["to"] == bme.as_str()).await;   // plans: the holder is chosen and persisted before IO
    walk_until(&ba, |r| r.is_empty()).await;                                 // sends
    assert_eq!(link.sent.lock().unwrap().as_slice(), ["linkB"]);
    assert!(ba.owner_records().await.is_empty(), "committed: the ticket is cleared");
    // B has the mail: its own conversation, the original text, addressed to its uberbot, and a ticket of its own
    let l = rpc(&hb, "mail.list", json!({})).await["result"].clone(); assert_eq!(l["total"], 1, "{l}");
    let cid = l["convs"][0]["id"].as_str().unwrap().to_string(); assert!(cid.starts_with("owner:"), "{cid}");
    assert!(l["convs"][0]["parts"].as_array().unwrap().iter().any(|p| p == &json!(format!("bot:{ubb}"))));
    let rd = rpc(&hb, "mail.read", json!({"conv":cid})).await["result"].clone(); assert_eq!(rd["mails"][0]["text"], "hello owner\nmore"); assert_eq!(rd["mails"][0]["to"], format!("bot:{ubb}").as_str());
    until(&hb, |l| l.iter().any(|e| e["$"] == "RunSet")).await;
    assert!(bb.owner_records().await.is_empty(), "the holder's run claims the ticket as it starts");
    // the same request again is acknowledged without a second filing; a wrong link, a stale term and a body that is not the ticket's are refused
    let key = mail::owner::key(&r);
    let req = |term: u64, id: &str| json!({"m":"owner.mail","p":{"sender":ame,"uid":uid,"term":term,"ticket":{"uid":uid,"origin":ame,"mail":{"id":id},"conversation":{},"id":id,"conv":"c"},"body":{},"hold":""}});
    let ticket = r.clone(); let again = json!({"m":"owner.mail","p":{"sender":ame,"uid":uid,"term":1,"ticket":ticket,"body":null,"hold":""}});
    assert_eq!(bb.owner_received("linkA", &again).await, (200, mail::owner::response(&ticket, "committed")));
    assert_eq!(bb.owner_received("linkZ", &again).await.0, 403);
    assert_eq!(bb.owner_received("linkA", &req(9, "m-new")).await, (409, mail::owner::response(&req(9, "m-new")["p"]["ticket"], "moved")));
    assert_eq!(bb.owner_received("linkA", &req(1, "m-new")).await.0, 400, "a ticket whose body is not there is invalid");
    assert_eq!(rpc(&hb, "mail.list", json!({})).await["result"]["total"], 1);
    assert!(key.starts_with("owner:["));
}

/// a link whose request takes a while (and counts), standing in for a slow machine
struct Slow { n: std::sync::atomic::AtomicUsize }
impl mail::owner::OwnerLink for Slow {
    fn post<'a>(&'a self, _l: &'a str, _b: String) -> Fut<'a, Option<(u16, String)>> { Box::pin(async move { self.n.fetch_add(1, std::sync::atomic::Ordering::SeqCst); tokio::time::sleep(std::time::Duration::from_millis(400)).await; None }) }
}
/// plan a pending ticket for A's outbox: file mail for the owner, then one walk that chooses the recipient
async fn owner_planned(p: &Pair) {
    assert_eq!(rpc(&p.ha, "mail.send", json!({"to":format!("bot:{}", p.ua),"text":"hi"})).await["ok"], true);
    walk_until(&p.ba, |r| r.first().map_or(false, |x| x["to"] == p.bme.as_str())).await;
}
/// a link whose request stays inside `post` until the test lets it go: no clock, so a loaded runner changes nothing
struct Held { n: std::sync::atomic::AtomicUsize, entered: tokio::sync::Notify, release: tokio::sync::Semaphore }
impl mail::owner::OwnerLink for Held {
    fn post<'a>(&'a self, _l: &'a str, _b: String) -> Fut<'a, Option<(u16, String)>> { Box::pin(async move { self.n.fetch_add(1, std::sync::atomic::Ordering::SeqCst); self.entered.notify_one(); self.release.acquire().await.unwrap().forget(); None }) }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn the_owner_outbox_has_one_walker_at_a_time_and_a_walk_ends() {
    let p = owner_pair("walk").await; owner_planned(&p).await;
    let link = Arc::new(Held { n: Default::default(), entered: Default::default(), release: tokio::sync::Semaphore::new(0) }); p.ba.set_owner_link(link.clone());
    let (b1, b2) = (p.ba.clone(), p.ba.clone());
    let w1 = tokio::spawn(async move { b1.owner_outbox().await });
    link.entered.notified().await;                     // the first walk is inside its request, and stays there until released
    tokio::spawn(async move { b2.owner_outbox().await }).await.unwrap();   // the second walk returns at once: it saw the first still going
    assert_eq!(link.n.load(std::sync::atomic::Ordering::SeqCst), 1, "the second walk saw the first still going and left");
    link.release.add_permits(100); w1.await.unwrap();   // requests go through now, whoever walks (the background timer and owner_kick may too)
    for _ in 0..800 { p.ba.owner_outbox().await; if link.n.load(std::sync::atomic::Ordering::SeqCst) >= 2 { break } tokio::time::sleep(std::time::Duration::from_millis(25)).await }
    assert!(link.n.load(std::sync::atomic::Ordering::SeqCst) >= 2, "a walk that ended lets the next one run");
}
#[test] fn owner_copy_child_probe() {
    if std::env::var("BP_OWNER_COPY_PROBE").ok().as_deref() != Some("1") { return }
    assert!(crate::copy::on());
    tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap().block_on(async {
        let p = owner_pair("copy").await;
        assert_eq!(rpc(&p.ha, "mail.send", json!({"to":format!("bot:{}", p.ua),"text":"hi"})).await["ok"], true);
        // a walk plans nothing on a copy, so the ticket is given its recipient by hand: the one thing left to stop is the request
        let r = p.ba.owner_records().await.remove(0); let next = mail::owner::attempt(&r, &p.bme, 1, "s");
        p.ha.commit(&json!({"$":"SettingSet","key":mail::owner::pending_key(&mail::owner::key(&r)),"value":next.to_string(),"at":9})).await.unwrap();
        let link = Arc::new(Slow { n: Default::default() }); p.ba.set_owner_link(link.clone());
        p.ba.owner_outbox().await;
        assert_eq!(link.n.load(std::sync::atomic::Ordering::SeqCst), 0, "a hub on a copy of a log sends no mail to anyone");
        assert_eq!(p.ba.owner_records().await.len(), 1);
    });
}
#[test] fn a_hub_on_a_copy_walks_no_owner_outbox() {
    let me = std::env::current_exe().unwrap();
    let o = std::process::Command::new(me).args(["bots::tests::owner_copy_child_probe", "--exact", "--nocapture", "--test-threads=1"])
        .env("BACKPLANE_COPY", "1").env("BP_OWNER_COPY_PROBE", "1").env_remove("BACKPLANE_AGENTS").output().unwrap();
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("owner_copy_child_probe ... ok"), "the probe ran: {}", String::from_utf8_lossy(&o.stdout));
}
#[tokio::test] async fn a_prompt_a_person_types_counts_for_the_star_nudge_once() {
    // Star.accepted: bots.say and bots.post from a client (not a retry, not blank) hand their message id to the state lane's count, whose events commit with the prompt's own;
    // a room's fanout is one prompt, and bots.space, retries and blank text are not prompts
    let (hub, bots, _r) = start("parity-star", "").await;
    let (ada, bob) = (bot(&hub, "ada").await, bot(&hub, "bob").await);
    assert_eq!(rpc(&hub, "bots.room", json!({"room":"r1","name":"lab","members":format!("{ada}\n{bob}")})).await["ok"], true);
    let calls: Arc<StdMutex<Vec<String>>> = Default::default(); let c2 = calls.clone();
    // the stand-in count offers at "p1", as the real one does at the fifth
    bots.set_star(Arc::new(move |msg: &str, now: u64| { c2.lock().unwrap().push(msg.to_string()); (vec![json!({"$":"SettingSet","key":format!("star.seen.{msg}"),"value":"1","at":now})], (msg == "p1").then(|| json!({"t":"star","offer":{"token":msg}}))) }));
    let (mut mine, mut other) = (hub.attach(caller().client), hub.attach(2));
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":ada,"text":"hi","msg":"m1"})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.post", json!({"room":"r1","text":"hello room","msg":"p1"})).await["ok"], true);
    assert_eq!(*calls.lock().unwrap(), ["m1", "p1"], "one count each, the room's two members are not two prompts");
    let frames = |o: &mut crate::hub::Outbox| { let mut v = vec![]; while let Ok(crate::hub::Out::Value(f)) = o.try_recv() { if f["t"] == "star" { v.push(f) } } v };
    assert_eq!(frames(&mut mine), vec![json!({"t":"star","offer":{"token":"p1"}})], "the offer goes to the connection that typed the prompt");
    assert!(frames(&mut other).is_empty(), "and to no other");
    // a prompt with no client connection behind it (an agent, a routine, the hub itself) is no person's
    let h = hub.rpc_for("bots.say").unwrap(); h.call(&hub, &Caller { gen: "agents".into(), client: 0 }, &json!(8), "bots.say", &json!({"bot":ada,"text":"from an agent","msg":"a1"})).await;
    assert_eq!(*calls.lock().unwrap(), ["m1", "p1"]);
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":ada,"text":"hi","msg":"m1"})).await["ok"], true); // the same msg again: already accepted
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":ada,"text":"again","msg":"m2","retry":true})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.say", json!({"bot":ada,"text":"   ","msg":"m3"})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.space", json!({"bot":ada,"action":"open","value":""})).await["ok"], true);
    assert_eq!(*calls.lock().unwrap(), ["m1", "p1"]);
    let l = log(&hub);
    assert!(l.iter().any(|e| e["$"] == "SettingSet" && e["key"] == "star.seen.m1") && l.iter().any(|e| e["$"] == "SettingSet" && e["key"] == "star.seen.p1"));
    // the star events are in the same commit as the prompt's own: right after the wake / with the room post
    let i = l.iter().position(|e| e["key"] == "star.seen.m1").unwrap(); assert_eq!(l[i - 1]["$"], "BotWoke");
}

#[tokio::test] async fn the_bots_tools_reach_a_bots_agent_through_the_tool_set() {
    use crate::agents::mcp::ToolSet;
    let (hub, bots, _r) = start("toolset", "").await;
    let ada = bot(&hub, "ada").await;
    let th = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == ada.as_str()).unwrap()["thread"].as_str().unwrap().to_string();
    let set = tools::BotToolSet(bots.clone());
    assert_eq!(set.list(&th).len(), ledger::NAMES.len() + tools::NAMES.len(), "a bot's thread lists every bot tool");
    let plain: Vec<String> = set.list("t-other").iter().map(|d| d["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(plain, ledger::NAMES, "a thread that is no bot's lists the ledgers' only");
    assert!(set.owns("memory_save") && !set.owns("thread_list"));
    let (ok, body) = set.call(&th, "memory_save", &json!({"key":"k","text":"v"})).await; assert!(ok, "{body}");
    assert_eq!(set.call("t-other", "memory_save", &json!({})).await, (false, json!({"error":"unknown tool"})));
    // a bot made after the start is listed at once (its creation went through the world)
    let bob = bot(&hub, "bob").await;
    let tb = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == bob.as_str()).unwrap()["thread"].as_str().unwrap().to_string();
    bots.world().await; assert!(set.list(&tb).len() > 0);
}
#[tokio::test] async fn bot_create_makes_a_sub_bot_within_the_depth_and_kid_limits() {
    // Bots.spawn: the maker is recorded, the provider defaults to the maker's, the home job runs; 2 levels below a person-made bot, 8 kids, a valid free name
    let (hub, bots, _r) = start("spawn", "").await;
    let top = bot(&hub, "top").await;
    let thread_of = |id: &str| log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == id).unwrap()["thread"].as_str().unwrap().to_string();
    let t0 = thread_of(&top);
    let (ok, r) = bots.tool_call(&t0, "bot_create", &json!({"name":" kid1 ","persona":"p"})).await; assert!(ok, "{r}");
    assert_eq!((r["name"].as_str().unwrap(), r["note"].as_str().unwrap()), ("kid1", "kid1 is made: talk to it with bot_send"));
    let kid = r["bot"].as_str().unwrap().to_string();
    until(&hub, |l| l.iter().any(|e| e["$"] == "WorktreeSet" && e["id"] == thread_of(&kid).as_str())).await;
    assert_eq!(hub.setting(&format!("bot.parent.{kid}")).await.as_deref(), Some(top.as_str()));
    let l = log(&hub); let tc = l.iter().find(|e| e["$"] == "ThreadCreated" && e["title"] == "kid1").unwrap(); assert_eq!((tc["provider"].as_str().unwrap(), tc["project"].as_str().unwrap()), ("claude", ""));
    let bs = l.iter().find(|e| e["$"] == "BotSet" && e["id"] == kid.as_str()).unwrap(); assert_eq!((bs["persona"].as_str().unwrap(), bs["look"].clone()), ("p", json!(seed("kid1"))));
    // the sub-bot's own sub-bot is the second level; a third is refused
    let (ok, r2) = bots.tool_call(&thread_of(&kid), "bot_create", &json!({"name":"kid2"})).await; assert!(ok, "{r2}");
    let kid2 = r2["bot"].as_str().unwrap().to_string();
    let (ok, e) = bots.tool_call(&thread_of(&kid2), "bot_create", &json!({"name":"kid3"})).await; assert!(!ok); assert_eq!(e["error"], "bots made by bots go at most 2 levels deep; ask the person, or a bot higher up");
    // names: invalid and taken
    assert_eq!(bots.tool_call(&t0, "bot_create", &json!({"name":"a b"})).await.1["error"], "a bot's name is 1 to 24 letters, digits, - or _");
    assert_eq!(bots.tool_call(&t0, "bot_create", &json!({"name":"KID1"})).await.1["error"], "a bot named KID1 already exists");
    // eight kids at most (kid1 is one)
    for i in 2..=8 { assert!(bots.tool_call(&t0, "bot_create", &json!({"name":format!("k{i}")})).await.0) }
    assert_eq!(bots.tool_call(&t0, "bot_create", &json!({"name":"k9"})).await.1["error"], "you keep 8 sub-bots already; ask the person to retire one");
    // the new bot is listed at once in its own thread
    bots.world().await; assert!(bots.is_bot_thread(&thread_of(&kid)));
}
#[tokio::test] async fn a_bots_thread_starts_on_the_default_account() {
    // Accounts.created: the binding follows ThreadCreated for claude and codex (the default account, else none), for bots.create and bot_create; other providers get none
    let (hub, bots, _r) = start("accounts", "").await;
    hub.commit(&json!({"$":"SettingSet","key":"account.default.codex","value":"{\"id\":\"a1\",\"epoch\":2}","at":1})).await.unwrap();
    let a = bot(&hub, "ada").await;
    let th = |id: &str| log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == id).unwrap()["thread"].as_str().unwrap().to_string();
    let ta = th(&a);
    let l = log(&hub); let i = l.iter().position(|e| e["$"] == "ThreadCreated" && e["id"] == ta.as_str()).unwrap();
    assert_eq!(l[i + 1], json!({"$":"SettingSet","key":format!("account.thread.{ta}.claude"),"value":"{\"id\":\"\",\"epoch\":0}","at":l[i]["at"]}), "right after its ThreadCreated");
    let (ok, r) = bots.tool_call(&ta, "bot_create", &json!({"name":"kidx","provider":"codex"})).await; assert!(ok, "{r}");
    let tk = th(r["bot"].as_str().unwrap());
    assert_eq!(hub.setting(&format!("account.thread.{tk}.codex")).await.as_deref(), Some("{\"id\":\"a1\",\"epoch\":2}"));
    let (ok, r) = bots.tool_call(&ta, "bot_create", &json!({"name":"kidg","provider":"grok"})).await; assert!(ok, "{r}");
    let tg = th(r["bot"].as_str().unwrap());
    assert_eq!(hub.setting(&format!("account.thread.{tg}.grok")).await, None); assert_eq!(hub.setting(&format!("account.thread.{tg}.claude")).await, None);
}
/// the unprompted work of the lane: a due routine, the uberbot's first ops and a journal body whose header never reached the log
fn unprompted_probe() {
    let copy = crate::copy::on();
    assert_eq!(copy, std::env::var("BP_UNPROMPTED_EXPECT").ok().as_deref() == Some("quiet"), "the probe runs the mode it expects");
    tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap().block_on(async {
        let (hub, bots, _r) = start(if copy { "unprompted-copy" } else { "unprompted-live" }, "").await;
        let ada = bot(&hub, "ada").await;
        assert_eq!(rpc(&hub, "bots.routine", json!({"bot":ada,"name":"r","cron":"* * * * *","prompt":"p"})).await["ok"], true);
        let r = rpc(&hub, "bots.create", json!({"name":"bub","uber":true})).await; assert_eq!(r["ok"], true);
        until(&hub, |l| l.iter().any(|e| e["$"] == "UberSet") && l.iter().any(|e| e["$"] == "WorktreeSet" && e["id"] != "x")).await;
        let home = hub.cfg.home.clone();
        let conv = mail::Conv { id: "pair:orphan".into(), subject: "s".into(), kind: "pair".into(), parts: vec![], project: String::new(), tags: vec![], made: 1, last: 1, n: 0 };
        let mail = mail::Hdr { id: "orph1".into(), conv: "pair:orphan".into(), from: "a".into(), to: "b".into(), kind: "bot".into(), r#ref: String::new(), want: "fyi".into(), head: "h".into(), size: 1, at: 1 };
        assert_eq!(mail::store::append(&home, &mail::Body { conv, mail, text: "t".into() }), Ok(false));
        let n0 = hub.store.len();
        let (ticks, init, rec) = (bots.tick_at(now_s() + 300).await, bots.uber_init(now_s()).await, bots.mail_recover().await);
        if copy { assert_eq!((ticks, init, rec, hub.store.len()), (0, 0, 0, n0), "a hub on a copy fires no routine, makes no first ops, recovers no header, and writes nothing"); }
        else { assert!(ticks > 0 && init > 0 && rec > 0, "the same work runs on a live hub: {ticks} {init} {rec}"); assert!(hub.store.len() > n0) }
    });
}
#[test] fn unprompted_work_runs_live_and_not_on_a_copy_hub() {
    if std::env::var("BP_UNPROMPTED").ok().as_deref() == Some("1") { return unprompted_probe() }
    let me = std::env::current_exe().unwrap();
    for (copy, expect) in [(true, "quiet"), (false, "work")] {
        let mut c = std::process::Command::new(&me);
        c.args(["bots::tests::unprompted_work_runs_live_and_not_on_a_copy_hub", "--exact", "--nocapture", "--test-threads=1"]).env("BP_UNPROMPTED", "1").env("BP_UNPROMPTED_EXPECT", expect).env_remove("BACKPLANE_AGENTS");
        if copy { c.env("BACKPLANE_COPY", "1"); } else { c.env_remove("BACKPLANE_COPY"); }
        let o = c.output().unwrap();
        assert!(o.status.success(), "{expect}: {}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        assert!(String::from_utf8_lossy(&o.stdout).contains("unprompted_work_runs_live_and_not_on_a_copy_hub ... ok"), "the probe ran ({expect})");
    }
}

// ---- the ledger tools, notify_person and project_create against the recorded JS hub (test/bots/record_ledger.mjs) ----
/// ids and times inside a stored value or a sentence: `b1793945350-204`, `k...-1-0`, `1793945367.219`
fn scrub(s: &str) -> String {
    let c: Vec<char> = s.chars().collect(); let (mut o, mut i) = (String::new(), 0);
    let run = |i: usize| c[i..].iter().take_while(|x| x.is_ascii_digit()).count();
    while i < c.len() {
        let stamp = run(i) == 10 && (i == 0 || !c[i - 1].is_ascii_digit());
        let id = "bkpnqmto".contains(c[i]) && i + 1 < c.len() && run(i + 1) == 10 && (i == 0 || !c[i - 1].is_alphanumeric());
        if id || stamp {
            i += if id { 11 } else { 10 }; o.push_str(if id { "ID" } else { "T" });
            while i + 1 < c.len() && (c[i] == '-' || c[i] == '.') && c[i + 1].is_ascii_digit() { i += 1 + run(i + 1) }
        } else { o.push(c[i]); i += 1 }
    }
    o
}
fn scrubbed(v: &Value) -> Value {
    match v { Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), if ["at", "updated"].contains(&k.as_str()) { json!(0) } else { scrubbed(x) })).collect()),
        Value::Array(a) => Value::Array(a.iter().map(scrubbed).collect()), Value::String(s) => json!(scrub(s)), x => x.clone() }
}
/// what a tool writes, as a recording shows it: its board and told records, the alert record, a new project, a post and the wakes it makes
fn tool_line(e: &Value) -> bool { ["ProjectCreated", "RoomPosted", "BotWoke"].contains(&e["$"].as_str().unwrap_or("")) || (e["$"] == "SettingSet" && ["told.", "board.", "alert."].iter().any(|k| e["key"].as_str().unwrap_or("").starts_with(k))) }
fn stored(e: &Value) -> Value {
    let mut v = e.clone(); if let Some(o) = v.as_object_mut() { o.remove("at"); }
    if let Some(x) = e["value"].as_str() { if let Ok(j) = serde_json::from_str::<Value>(x) { v["value"] = scrubbed(&j) } else { v["value"] = json!(scrub(x)) } }
    v["key"] = json!(scrub(e["key"].as_str().unwrap_or(""))); if e["id"].is_string() { v["id"] = json!(scrub(e["id"].as_str().unwrap())) } v
}
#[tokio::test] async fn js_ledger_tools() {
    let rec: Value = serde_json::from_str(include_str!("../../../../test/bots/fixtures/js-ledger-calls.json")).unwrap();
    let js: Vec<Value> = include_str!("../../../../test/bots/fixtures/js-ledger.jsonl").lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let (hub, bots, inbox) = start("ledger", "").await;
    let work = std::env::temp_dir().join(format!("bp-ledger-{}", std::process::id())); let _ = std::fs::remove_dir_all(&work); std::fs::create_dir_all(&work).unwrap(); let work = crate::plat::fs::canonicalize(&work).unwrap();
    std::fs::write(work.join("afile"), "x").unwrap(); let w = work.to_str().unwrap().to_string();
    let (mut done, mut n) = (0usize, 0); let mut ids: Vec<(String, String)> = vec![];
    for c in rec["calls"].as_array().unwrap() {
        let (name, pre, post) = (c["name"].as_str().unwrap(), c["pre"].as_u64().unwrap() as usize, c["post"].as_u64().unwrap() as usize);
        // the two folders under the person's home would land in the home of whoever runs this test; commands::fs has their path rules
        let home = name == "project_create" && c["args"]["path"].as_str().map_or(false, |p| !p.is_empty() && !p.starts_with('/'));
        if !tools::owns(name) || home { continue }
        if pre > done { hub.commit_many(&js[done..pre]).await.unwrap(); done = pre }
        let mut text = c["args"].to_string(); for (a, b) in &ids { text = text.replace(a.as_str(), b) } text = text.replace("/tmp/bp-rec-work", &w);
        let args: Value = serde_json::from_str(&text).unwrap();
        *inbox.answer.lock().unwrap() = c["answer"].as_str().filter(|a| !a.is_empty()).unwrap_or("session").to_string();
        let th = c["thread"].as_str().unwrap(); let at = log(&hub).len();
        let (ok, body) = bots.tool_call_at(th, name, &args, c["t"].as_u64().unwrap()).await;
        let raw = c["result"]["content"][0]["text"].as_str().unwrap();
        let want: Value = serde_json::from_str(raw).unwrap_or_else(|_| json!(raw));
        assert_eq!(c["result"]["isError"].as_bool().unwrap(), !ok, "{name} {args}: {body}");
        let got: Value = serde_json::from_str(&body.to_string().replace(&w, "/tmp/bp-rec-work")).unwrap();
        // Push.gate: the JS hub let any bot reach the phone; here only the uberbot does, and this recording has none
        let mut want = want; if name == "notify_person" && want["note"] == "sent to the person" { want["note"] = json!(DESK_ONLY) }
        assert_eq!(scrubbed(&got), scrubbed(&want), "{name} {args}");
        if let (Some(a), Some(b)) = (want["id"].as_str(), body["id"].as_str()) { ids.push((a.to_string(), b.to_string())) }
        // what the call left in the log; a mark an alert gets is the state lane's (no Transform on this hub)
        let mine: Vec<Value> = log(&hub)[at..].iter().filter(|e| tool_line(e)).map(stored).collect();
        let theirs: Vec<Value> = js[pre.max(done)..post].iter().filter(|e| tool_line(e) && !(["notify_person", "project_create"].contains(&name) && e["key"].as_str().map_or(false, |k| k.starts_with("told.") || k.starts_with("alert.") && name == "project_create"))).map(stored).collect();
        let mine: Vec<Value> = serde_json::from_str(&serde_json::to_string(&mine).unwrap().replace(&w, "/tmp/bp-rec-work")).unwrap();
        assert_eq!(mine, theirs, "{name} {args}");
        // the rest of what happened meanwhile (an ask closing, a turn) is the hub's, and comes from the recording
        let rest: Vec<Value> = js[pre.max(done)..post].iter().filter(|e| !tool_line(e)).cloned().collect(); hub.commit_many(&rest).await.unwrap(); done = post.max(done);
        n += 1;
    }
    assert_eq!(n, rec["calls"].as_array().unwrap().iter().filter(|c| tools::owns(c["name"].as_str().unwrap())).count() - 2, "every recorded call of these tools but the two folders under the home");
    let names = |k: &str| rec[k].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).filter(|x| tools::owns(x)).collect::<Vec<_>>();
    use crate::agents::mcp::ToolSet;
    let set = tools::BotToolSet(bots.clone()); let list = |t: &str| set.list(t).iter().map(|d| d["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let mut mine = list(rec["thread"].as_str().unwrap()); mine.retain(|n| n != "room_join" && n != "room_leave"); // (ours, beside the JS hub's)
    assert_eq!(mine, names("listBot"), "a bot's tools in the JS order");
    assert_eq!(list("t1700000001-2"), names("listPlain"), "a plain thread's");
    let _ = std::fs::remove_dir_all(&work);
}
#[test] fn told_keys_words_and_overlap_follow_told() {
    assert_eq!(ledger::key_pick("  Lunch: Dana!!  ", "x"), "lunch:-dana-");
    assert_eq!(ledger::key_pick("", "The new board revision shipped to the fab today"), "the-new-board-revision-shipped-the-fab-today");
    assert_eq!(ledger::key_pick("", "ok"), "ok", "a text with no word of three is its own key");
    assert_eq!(ledger::words("the The THE a1 b2c é"), vec!["the", "b2c"], "distinct, lower case, three or more, ASCII runs");
    let (w, x) = (ledger::words("dana lunch nyc wed"), ledger::words("Dana lunch in NYC Wed 10/28"));
    assert!(ledger::same(&w, &x)); assert!(!ledger::same(&ledger::words("board"), &ledger::words("nothing here"))); assert!(ledger::same(&ledger::words("board"), &ledger::words("the board shipped")));
    assert!(!ledger::same(&ledger::words("one two three four five"), &ledger::words("one two six seven eight nine")), "two of five is under 60%");
    assert_eq!(ledger::channel_ok(""), "chat"); assert_eq!(ledger::channel_ok(" DIGEST "), "digest"); assert_eq!(ledger::channel_ok("weird"), "other");
}
#[test] fn told_check_stops_at_the_window_and_the_tenth() {
    let r = |at: u64| ledger::Told { key: "k".into(), text: "t".into(), channel: "chat".into(), bot: String::new(), at };
    let all: Vec<_> = (0..12).map(|i| r(1000 * 86400 - i)).chain([r(1000)]).collect();
    assert_eq!(ledger::check(&all, 1000 * 86400, "k", "", 0).len(), 10);
    assert_eq!(ledger::check(&all[10..], 1000 * 86400, "k", "", 99).len(), 2, "at most 30 days back, the old one left out");
}
#[test] fn board_dates_and_items_follow_tboard() {
    for (d, ok) in [("2026-12-01", true), ("2026-13-01", false), ("2026-12-32", false), ("2026-12-00", false), ("12/1", false), ("2026/12/01", false), ("20a6-12-01", false)] { assert_eq!(ledger::date(d), ok, "{d}") }
    let i = ledger::item_of("- [x] two (due 2026-11-30)").unwrap(); assert_eq!((i.title.as_str(), i.status.as_str(), i.due.as_str()), ("two", "done", "2026-11-30"));
    let i = ledger::item_of("12. [~] three due:2026-12-24").unwrap(); assert_eq!((i.title.as_str(), i.status.as_str(), i.due.as_str()), ("three", "doing", "2026-12-24"));
    assert_eq!(ledger::item_of("- [q] not a box"), None); assert_eq!(ledger::item_of("[ ] no bullet"), None); assert_eq!(ledger::item_of("- [ ]"), None); assert_eq!(ledger::item_of("- [ ]x"), None);
    assert_eq!(ledger::title_norm("  Two\t WORDS\n"), "two words");
    assert_eq!(ledger::status_norm(" DOING "), "doing"); assert_eq!(ledger::status_norm("nonsense"), "todo");
}
#[tokio::test] async fn what_a_tool_does_for_the_uberbot_makes_ops() {
    // Bots.ub: every commit a tool makes goes through World::also, so the uberbot's own changes reach its other machines
    let (hub, bots, _r) = start("tool-ops", "{\"$\":\"SettingSet\",\"key\":\"machine.id\",\"value\":\"m1\",\"at\":1}\n").await;
    let b = bot(&hub, "kit").await; assert_eq!(rpc(&hub, "bots.uber", json!({"bot":b})).await["ok"], true);
    let th = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == b.as_str()).unwrap()["thread"].as_str().unwrap().to_string();
    let ops = |hub: &Arc<Hub>| log(hub).iter().filter(|e| e["$"] == "UberOp").map(|e| serde_json::from_str::<Value>(e["op"].as_str().unwrap()).unwrap()["k"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    for (name, a) in [("memory_save", json!({"key":"k","kind":"fact","text":"v"})), ("memory_forget", json!({"key":"k"})), ("space_set", json!({"spec":"{}"})), ("space_page", json!({"html":"<p>"})),
        ("routine_set", json!({"id":"","name":"n","schedule":"daily 09:30","prompt":"p"})), ("remind_person", json!({"at":"in 20m","text":"t"}))] {
        let (ok, body) = bots.tool_call(&th, name, &a).await; assert!(ok, "{name}: {body}");
    }
    let rid = log(&hub).iter().find(|e| e["$"] == "RoutineSet").unwrap()["id"].as_str().unwrap().to_string();
    assert!(bots.tool_call(&th, "routine_delete", &json!({"id":rid})).await.0);
    assert_eq!(ops(&hub), ["mem", "forget", "space", "page", "routine", "routine", "routine-del"]);
    let n = ops(&hub).len();
    assert!(bots.tool_call(&th, "told_mark", &json!({"text":"said it"})).await.0); assert!(bots.tool_call(&th, "task_board_add", &json!({"title":"t"})).await.0);
    assert_eq!(ops(&hub).len(), n, "the ledgers are this machine's own");
}
#[tokio::test] async fn notify_skips_a_queued_message_when_it_finds_the_turn() {
    // Turn.of.state: a message waiting behind the turn is not its trigger
    let (hub, bots, rec) = start("notify-q", "").await;
    let b = bot(&hub, "ada").await; let th = log(&hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == b.as_str()).unwrap()["thread"].as_str().unwrap().to_string();
    hub.commit_many(&[json!({"$":"MessagePosted","thread":th,"msg":"m1","role":"user","text":"[routine: digest]\nmake it","at":100}), json!({"$":"MessagePosted","thread":th,"msg":"m2","role":"user","text":"later","at":101})]).await.unwrap();
    let digest = |bots: Arc<Bots>, th: String| async move { bots.tool_call_at(&th, "notify_person", &json!({"text":"d","reason":"digest"}), 200).await };
    assert!(!digest(bots.clone(), th.clone()).await.0, "the turn's message is the newest");
    *rec.queue.lock().unwrap() = vec!["m2".into()];
    assert_eq!(digest(bots.clone(), th.clone()).await, (true, json!({"ok":true,"note":DESK_ONLY})), "no uberbot here, so the desk only");
}
#[tokio::test] async fn a_filed_mail_to_the_uberbot_gives_its_agent_a_turn_once_however_often_it_is_routed() {
    // a routine's firing is filed for the uberbot; its run is bound to that input, claims the owner ticket, and the agent is handed the brief with the exact body.
    // Routing the same header again (the owner outbox walk does, every 20 s) starts nothing.
    std::env::set_var("TZ", "Etc/UTC");
    let (hub, bots, rec) = start("route-once", "").await;
    let r = rpc(&hub, "bots.create", json!({"name":"bub","uber":true})).await; let bub = r["bot"].as_str().unwrap().to_string();
    until(&hub, |l| l.iter().any(|e| e["$"] == "UberSet")).await;
    until(&hub, |l| l.iter().filter(|e| e["$"] == "WorktreeSet").count() >= 1).await;
    assert!(bots.uber_init(now_s()).await > 0);
    assert_eq!(rpc(&hub, "bots.routine", json!({"bot":bub,"routine":"ro2","name":"morning","at":"in 3d","prompt":"plan the day"})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.routine_run", json!({"routine":"ro2"})).await["ok"], true);
    for _ in 0..100 { if !rec.got.lock().unwrap().is_empty() { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    let got = rec.got.lock().unwrap().clone(); assert_eq!(got.len(), 1, "one brief");
    let (th, _msg, text, _o) = &got[0];
    assert!(th.starts_with("run") && text.starts_with("<charter>") && text.contains("<mail_history newest-first>") && text.contains("private mailroom triage"), "the owner's brief");
    let ev = log(&hub);
    let run = ev.iter().find(|e| e["$"] == "RunSet").expect("RunSet"); let mail_id = run["mail"].as_str().unwrap().to_string();
    assert_eq!((run["bot"].as_str().unwrap(), run["conv"].as_str().unwrap(), run["thread"].as_str().unwrap()), (bub.as_str(), "routine:ro2", th.as_str()));
    assert!(ev.iter().any(|e| e["$"] == "SettingSet" && e["key"] == format!("run.bound.{th}") && e["value"] == mail_id.as_str()), "bound to its input");
    assert!(ev.iter().any(|e| e["$"] == "SettingSet" && e["key"].as_str().unwrap_or("").starts_with("owner.mail.pending.") && e["value"] == ""), "the owner ticket is claimed with the run");
    let body = mail::store::read(&mail::store::path(&hub.cfg.home, "routine:ro2")).remove(0);
    let n = ev.len();
    bots.route_body(body.clone()).await; bots.route_body(body).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(log(&hub).iter().filter(|e| e["$"] == "RunSet").count(), 1, "a header already bound to a run starts no second one");
    assert_eq!(log(&hub).len(), n, "and logs nothing");
    assert_eq!(rec.got.lock().unwrap().len(), 1);
}

mod triage;
mod push;
mod standin;

async fn call_hook(hub: &Arc<Hub>, hid: &str, hs: Vec<(String, String)>, body: &str) -> (String, String) {
    let h = hub.http_for(&format!("/hook/{hid}")).expect("the /hook route is registered");
    let mut r = crate::auth::Req::new("POST", &format!("/hook/{hid}"), hs); r.body = body.as_bytes().to_vec();
    let a = h.call(hub, &r).await; (a.status.to_string(), String::from_utf8_lossy(&a.body).into_owned())
}
#[tokio::test] async fn a_webhook_is_made_signed_calls_reach_the_bot_once_and_revoking_drops_its_secret() {
    use crate::net::sign;
    let (hub, _b, rec) = start("hooks", "").await; let a = bot(&hub, "ada").await;
    assert_eq!(rpc(&hub, "bots.hook", json!({"bot":"nobody","name":"ci"})).await["ok"], false);
    let r = rpc(&hub, "bots.hook", json!({"bot":a,"name":"ci"})).await; assert_eq!(r["ok"], true, "{r}");
    let (hid, secret) = (r["hook"].as_str().unwrap().to_string(), r["secret"].as_str().unwrap().to_string());
    assert_eq!(r["path"], format!("/hook/{hid}")); assert_eq!(secret.len(), 64);
    let page = crate::bots::bot_page(&hub, &a, "hooks", 10).await.unwrap(); assert_eq!(page["hooks"][0]["name"], "ci");
    // no credentials, then a wrong signature, then a stale one
    assert_eq!(call_hook(&hub, &hid, vec![], "{}").await.0, "401 Unauthorized");
    let now = crate::bots::rpc::now_s(); let body = r#"{"event":"build","ok":true}"#;
    let hs = |ts: u64, sig: String| vec![("X-Backplane-Timestamp".to_string(), ts.to_string()), ("X-Backplane-Signature".to_string(), sig)];
    assert_eq!(call_hook(&hub, &hid, hs(now, "sha256=00".into()), body).await.1, "bad or stale signature");
    assert_eq!(call_hook(&hub, &hid, hs(now - 1000, sign::header(&secret, now - 1000, body)), body).await.1, "bad or stale signature");
    assert!(rec.got.lock().unwrap().is_empty(), "nothing reached the bot yet");
    // a genuine call: heard once as data, counted; the same call again is a replay
    let good = hs(now, sign::header(&secret, now, body));
    assert_eq!(call_hook(&hub, &hid, good.clone(), body).await, ("200 OK".to_string(), "ok".to_string()));
    assert_eq!(call_hook(&hub, &hid, good, body).await.1, "already seen");
    let got = rec.got.lock().unwrap().clone(); assert_eq!(got.len(), 1, "{got:?}"); assert!(got[0].2.starts_with("[webhook: ci] A signed request arrived."), "{}", got[0].2); assert!(got[0].2.contains(body));
    assert_eq!(crate::bots::bot_page(&hub, &a, "hooks", 10).await.unwrap()["hooks"][0]["count"], 1);
    // GitHub's signature, keyed by the secret's text and replayed by its delivery id
    let gh = format!("sha256={}", hex::encode(crate::net::owner::hmac_sha256(secret.as_bytes(), body.as_bytes())));
    let ghs = vec![("X-Hub-Signature-256".to_string(), gh), ("X-GitHub-Delivery".to_string(), "d1".to_string())];
    assert_eq!(call_hook(&hub, &hid, ghs.clone(), body).await.0, "200 OK"); assert_eq!(call_hook(&hub, &hid, ghs, body).await.1, "already seen");
    // a bearer token: none until one is made, then each call is new, then off
    let bearer = |t: &str| vec![("Authorization".to_string(), format!("Bearer {t}"))];
    assert_eq!(call_hook(&hub, &hid, bearer("x"), body).await.1, "this webhook has no bearer token: make one in its bot's Webhooks tab");
    let t = rpc(&hub, "bots.hook_token", json!({"hook":hid,"op":"show"})).await; let tok = t["token"].as_str().unwrap().to_string();
    assert_eq!(tok.len(), 32); assert!(t["tokenUrl"].as_str().unwrap().ends_with(&format!("/hook/{hid}")));
    assert_eq!(rpc(&hub, "bots.hook_token", json!({"hook":hid,"op":"show"})).await["token"], tok.as_str(), "shown again, not remade");
    assert_eq!(call_hook(&hub, &hid, bearer("wrong"), body).await.1, "wrong bearer token");
    assert_eq!(call_hook(&hub, &hid, bearer(&tok), body).await.0, "200 OK"); assert_eq!(call_hook(&hub, &hid, bearer(&tok), body).await.0, "200 OK");
    let new = rpc(&hub, "bots.hook_token", json!({"hook":hid,"op":"new"})).await["token"].as_str().unwrap().to_string(); assert_ne!(new, tok);
    assert_eq!(call_hook(&hub, &hid, bearer(&tok), body).await.1, "wrong bearer token");
    assert_eq!(rpc(&hub, "bots.hook_token", json!({"hook":"nope","op":"show"})).await["ok"], false);
    // revoking drops the hook and its files: the secret that signed before is refused
    assert_eq!(rpc(&hub, "bots.hook_revoke", json!({"hook":hid})).await["ok"], true);
    assert!(!hub.cfg.home.join("secrets/hooks").join(&hid).exists() && !hub.cfg.home.join("secrets/hooks").join(format!("{hid}.token")).exists());
    let n = rec.got.lock().unwrap().len(); let later = hs(now + 1, sign::header(&secret, now + 1, "x"));
    assert_eq!(call_hook(&hub, &hid, later, "x").await.0, "401 Unauthorized"); assert_eq!(rec.got.lock().unwrap().len(), n);
}

#[tokio::test] async fn a_task_run_asked_for_wakes_the_bot_with_its_parameters() {
    // main.rs starts the browser driver before the bots: this hub does the same, so Bots wires the driver's wake
    let home = std::env::temp_dir().join(format!("bp-bots-{}-taskrun", std::process::id())); let _ = std::fs::remove_dir_all(&home); std::fs::create_dir_all(&home).unwrap(); std::fs::write(home.join("events.jsonl"), "").unwrap();
    let hub = Hub::start(Cfg { home, version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    crate::term::start(hub.clone());
    let rec = Arc::new(Rec::default()); let _bots = Bots::start_with(hub.clone(), Arc::new(rec.clone())).await;
    let a = bot(&hub, "ada").await; let d = crate::term::driver::get(&hub).expect("driver");
    d.set_task(&a, "login", r#"{"v":1,"slug":"login","name":"Log in","description":"","params":[{"name":"user","label":"","kind":"text","example":""}],"steps":[]}"#, 1700000001).await;
    let r = rpc(&hub, "bots.task", json!({"bot":a,"op":"run","slug":"login","params":{"user":"ada"}})).await; assert_eq!(r["ok"], true, "{r}");
    for _ in 0..100 { if rec.got.lock().unwrap().iter().any(|x| x.2.starts_with("[task: login] Run the task skill login with") && x.2.contains("ada")) { return } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    panic!("the bot never heard the task: {:?}", rec.got.lock().unwrap());
}

// rules.set/drop, board.set/drop: the person's own edits of the standing rules and the task board, as settings the agents' side reads
#[tokio::test] async fn the_person_sets_and_drops_rules_and_board_items() {
    let (hub, _bots, _r) = start("person-edits", "").await;
    let set = |hub: &Arc<Hub>, k: &str| log(hub).iter().rev().find(|e| e["$"] == "SettingSet" && e["key"] == k).map(|e| e["value"].as_str().unwrap().to_string());
    let r = rpc(&hub, "rules.set", json!({"rule":{"text":"Never push to main","tool":"Bash, git_*","enforce":true,"word":"push"}})).await; assert_eq!(r["ok"], true, "{r}");
    let id = r["result"]["id"].as_str().unwrap().to_string();
    let stored: Value = serde_json::from_str(&set(&hub, &format!("rule.{id}")).unwrap()).unwrap();
    assert_eq!((stored["text"].as_str(), stored["tool"].as_str(), stored["enforce"].as_bool(), stored["on"].as_bool(), stored["by"].as_str()), (Some("Never push to main"), Some("Bash, git_*"), Some(true), Some(true), Some("person")));
    let st: std::collections::HashMap<String, String> = [(format!("rule.{id}"), stored.to_string())].into();
    assert!(crate::agents::rules::rules_block(&st, "", "").contains("Never push to main"), "the agents' block reads it");
    let r = rpc(&hub, "rules.set", json!({"rule":{"id":id,"on":false}})).await; assert_eq!(r["ok"], true, "{r}");
    let stored: Value = serde_json::from_str(&set(&hub, &format!("rule.{id}")).unwrap()).unwrap(); assert_eq!((stored["on"].as_bool(), stored["text"].as_str()), (Some(false), Some("Never push to main")), "only what was given changes");
    for (rule, why) in [(json!({"text":""}), "a rule needs its words"), (json!({"text":"x","enforce":true}), "an enforced rule names the tools it refuses"), (json!({"text":"x","bot":"nobody"}), "no bot \"nobody\" here"), (json!({"id":"zz","text":"x"}), "no rule zz")] {
        assert_eq!(rpc(&hub, "rules.set", json!({"rule":rule})).await["error"], why);
    }
    assert_eq!(rpc(&hub, "rules.drop", json!({"id":id})).await["ok"], true); assert_eq!(set(&hub, &format!("rule.{id}")).as_deref(), Some(""));
    assert_eq!(rpc(&hub, "rules.drop", json!({"id":id})).await["error"], format!("no rule {id}"));
    let r = rpc(&hub, "board.set", json!({"item":{"title":"Order the boards","due":"2026-12-01"}})).await; assert_eq!(r["ok"], true, "{r}");
    let bid = r["result"]["id"].as_str().unwrap().to_string(); assert_eq!(r["result"]["item"]["status"], "todo");
    assert_eq!(rpc(&hub, "board.set", json!({"item":{"id":bid,"status":"doing"}})).await["result"]["item"]["title"], "Order the boards");
    let item: Value = serde_json::from_str(&set(&hub, &format!("board.{bid}")).unwrap()).unwrap(); assert_eq!((item["status"].as_str(), item["by"].as_str()), (Some("doing"), Some("person")));
    assert_eq!(rpc(&hub, "board.set", json!({"item":{"title":"x","due":"soon"}})).await["error"], "a due date is YYYY-MM-DD");
    assert_eq!(rpc(&hub, "board.drop", json!({"id":bid})).await["ok"], true); assert_eq!(set(&hub, &format!("board.{bid}")).as_deref(), Some(""));
    assert_eq!(rpc(&hub, "board.drop", json!({"id":bid})).await["error"], format!("no item {bid}"));
}

// ---- bots across linked machines (bots/far.rs): two hubs, each with its Bots, wired to each other through an OwnerLink that calls the other's `far_received`
// (the route's own gate, "only a link that is one of the owner's own", is net/rpc/tests.rs)
struct Wire { to: std::sync::OnceLock<Arc<Bots>>, as_link: String, peer_name: String, up: std::sync::atomic::AtomicBool, seen: StdMutex<Vec<String>> }
impl mail::owner::OwnerLink for Wire {
    fn post<'a>(&'a self, link: &'a str, body: String) -> Fut<'a, Option<(u16, String)>> {
        Box::pin(async move {
            if link != "L1" || !self.up.load(std::sync::atomic::Ordering::SeqCst) { return None }
            let o: Value = serde_json::from_str(&body).unwrap(); let (m, p) = (o["m"].as_str().unwrap().to_string(), o["p"].clone());
            self.seen.lock().unwrap().push(m.clone());
            let to = self.to.get().unwrap();
            if m == "mcp.call" { // what net/rpc.rs makes of it
                let (c, t) = to.far_received(&self.as_link, &format!("tool.{}", p["tool"].as_str().unwrap()), &p["args"]).await;
                return Some(if c == 200 { (200, json!({"ok":true,"body":serde_json::from_str::<Value>(&t).unwrap()}).to_string()) } else { (c, t) });
            }
            Some(to.far_received(&self.as_link, &m, &p).await)
        })
    }
    fn machines(&self) -> Vec<(String, String, String)> { vec![("L1".into(), self.peer_name.clone(), "http://x".into())] }
}
struct Two { ha: Arc<Hub>, ba: Arc<Bots>, ra: Arc<Rec>, hb: Arc<Hub>, bb: Arc<Bots>, rb: Arc<Rec>, ada: String, bob: String, wa: Arc<Wire> }
async fn two(tag: &str) -> Two {
    let (ha, ba, ra) = start(&format!("far{tag}A"), "").await; let (hb, bb, rb) = start(&format!("far{tag}B"), "").await;
    let wa = Arc::new(Wire { to: Default::default(), as_link: "L1".into(), peer_name: "deskB".into(), up: true.into(), seen: Default::default() });
    let wb = Arc::new(Wire { to: Default::default(), as_link: "L1".into(), peer_name: "deskA".into(), up: true.into(), seen: Default::default() });
    wa.to.set(bb.clone()).ok(); wb.to.set(ba.clone()).ok();
    ba.set_owner_link(wa.clone()); bb.set_owner_link(wb); ba.far_start(); bb.far_start();
    let (ada, bob) = (bot(&ha, "ada").await, bot(&hb, "bob").await);
    Two { ha, ba, ra, hb, bb, rb, ada, bob, wa }
}
fn thread_of(hub: &Arc<Hub>, bot: &str) -> String { log(hub).iter().find(|e| e["$"] == "BotSet" && e["id"] == bot).unwrap()["thread"].as_str().unwrap().to_string() }
fn posts(hub: &Arc<Hub>, room: &str) -> Vec<(String, String)> { log(hub).iter().filter(|e| e["$"] == "RoomPosted" && e["room"] == room).map(|e| (e["from"].as_str().unwrap().to_string(), e["text"].as_str().unwrap().to_string())).collect() }
fn room_members(hub: &Arc<Hub>, room: &str) -> Option<String> { log(hub).iter().rev().find(|e| e["$"] == "RoomSet" && e["id"] == room).map(|e| e["members"].as_str().unwrap().to_string()) }

#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn a_room_with_a_bot_on_another_machine_is_kept_on_both_and_posts_travel_both_ways() {
    let t = two("room").await;
    let (th_a, th_b) = (thread_of(&t.ha, &t.ada), thread_of(&t.hb, &t.bob));
    let (ok, r) = t.ba.tool_call(&th_a, "room_create", &json!({"name":"lab","members":"bob@deskB"})).await; assert!(ok, "{r}");
    let rid = log(&t.ha).iter().find(|e| e["$"] == "RoomSet").unwrap()["id"].as_str().unwrap().to_string();
    until(&t.hb, |l| l.iter().any(|e| e["$"] == "RoomSet" && e["id"] == rid.as_str())).await;
    assert_eq!(sorted(&room_members(&t.hb, &rid).unwrap()), sorted(&format!("{}\nada@L1", t.bob)), "B holds the room, ada as a bot of the other machine, bob as its own");
    // ada posts: bob hears it on B, from ada@deskA
    let (ok, _) = t.ba.tool_call(&th_a, "room_post", &json!({"room":"lab","text":"hi bob"})).await; assert!(ok);
    until(&t.hb, |l| l.iter().any(|e| e["$"] == "RoomPosted" && e["text"] == "hi bob")).await;
    assert_eq!(posts(&t.hb, &rid), vec![("ada@deskA".to_string(), "hi bob".to_string())]);
    for _ in 0..100 { if t.rb.got.lock().unwrap().iter().any(|g| g.0 == th_b) { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    assert!(t.rb.got.lock().unwrap().iter().any(|g| g.0 == th_b && g.2.contains("[message from ada@deskA in #lab]\nhi bob")), "bob's thread is told: {:?}", t.rb.got.lock().unwrap());
    // bob answers: A logs it and wakes ada
    let (ok, _) = t.bb.tool_call(&th_b, "room_post", &json!({"room":"lab","text":"hi ada"})).await; assert!(ok);
    until(&t.ha, |l| l.iter().any(|e| e["$"] == "RoomPosted" && e["text"] == "hi ada")).await;
    for _ in 0..100 { if t.ra.got.lock().unwrap().iter().any(|g| g.0 == th_a && g.2.contains("hi ada")) { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    assert!(t.ra.got.lock().unwrap().iter().any(|g| g.0 == th_a && g.2.contains("[message from bob@deskB in #lab]")));
    tokio::time::sleep(std::time::Duration::from_millis(300)).await; // an echo would show by now
    assert_eq!((posts(&t.ha, &rid).len(), posts(&t.hb, &rid).len()), (2, 2), "each post once on each machine, none sent back: {:?} {:?}", posts(&t.ha, &rid), posts(&t.hb, &rid));
    assert_eq!(log(&t.ha).iter().filter(|e| e["$"] == "RoomSet" && e["id"] == rid.as_str()).count(), 1, "the room change was not sent back and forth");
    assert_eq!(log(&t.hb).iter().filter(|e| e["$"] == "RoomSet" && e["id"] == rid.as_str()).count(), 1);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn leaving_and_joining_a_shared_room_reaches_the_other_machine() {
    let t = two("member").await;
    let (th_a, th_b) = (thread_of(&t.ha, &t.ada), thread_of(&t.hb, &t.bob));
    // bob makes a room on B; ada joins it from A by naming the machine: it is copied to A
    assert!(t.bb.tool_call(&th_b, "room_create", &json!({"name":"b-room","members":""})).await.0);
    let rid = log(&t.hb).iter().find(|e| e["$"] == "RoomSet").unwrap()["id"].as_str().unwrap().to_string();
    let (ok, r) = t.ba.tool_call(&th_a, "room_join", &json!({"room":"b-room","machine":"deskB"})).await; assert!(ok, "{r}");
    until(&t.ha, |l| l.iter().any(|e| e["$"] == "RoomSet" && e["id"] == rid.as_str())).await;
    assert_eq!(sorted(&room_members(&t.ha, &rid).unwrap()), sorted(&format!("{}\nbob@L1", t.ada)), "A holds it, bob as the other machine's");
    until(&t.hb, |l| l.iter().filter(|e| e["$"] == "RoomSet" && e["id"] == rid.as_str()).count() >= 2).await;
    assert!(room_members(&t.hb, &rid).unwrap().contains("ada@L1"));
    // bob leaves on B: A's copy loses him
    assert!(t.bb.tool_call(&th_b, "room_leave", &json!({"room":"b-room"})).await.0);
    until(&t.ha, |l| room_members_in(l, &rid).map_or(false, |m| !m.contains("bob"))).await;
    // and a post from the person on A reaches nobody on B now
    assert_eq!(room_members_in(&log(&t.ha), &rid).unwrap(), t.ada);
}
fn sorted(s: &str) -> Vec<String> { let mut v: Vec<String> = s.lines().map(str::to_string).collect(); v.sort(); v }
fn room_members_in(l: &[Value], room: &str) -> Option<String> { l.iter().rev().find(|e| e["$"] == "RoomSet" && e["id"] == room).map(|e| e["members"].as_str().unwrap().to_string()) }
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn bot_send_to_name_at_machine_opens_a_shared_room_and_the_plane_of_the_other_machine_reads() {
    let t = two("send").await;
    let (th_a, th_b) = (thread_of(&t.ha, &t.ada), thread_of(&t.hb, &t.bob));
    let (ok, r) = t.ba.tool_call(&th_a, "bot_list", &json!({})).await; assert!(ok);
    assert_eq!(r["elsewhere"][0]["name"], "bob@deskB", "{r}");
    let (ok, r) = t.ba.tool_call(&th_a, "bot_send", &json!({"to":"bob@deskB","text":"build the board","topic":"board"})).await; assert!(ok, "{r}");
    until(&t.hb, |l| l.iter().any(|e| e["$"] == "RoomPosted" && e["text"] == "build the board")).await;
    for _ in 0..100 { if t.rb.got.lock().unwrap().iter().any(|g| g.0 == th_b) { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    assert!(t.rb.got.lock().unwrap().iter().any(|g| g.0 == th_b && g.2.contains("build the board")));
    // the same topic again is the same room
    let (ok, _) = t.ba.tool_call(&th_a, "bot_send", &json!({"to":"bob@deskB","text":"and test it","topic":"board"})).await; assert!(ok);
    until(&t.hb, |l| l.iter().any(|e| e["$"] == "RoomPosted" && e["text"] == "and test it")).await;
    assert_eq!(log(&t.ha).iter().filter(|e| e["$"] == "RoomSet").count(), 1);
    // plane_read with `machine`: B's plane, its ids through the link
    let (ok, r) = t.ba.tool_call(&th_a, "plane_read", &json!({"machine":"deskB"})).await; assert!(ok, "{r}");
    let text = r["text"].as_str().unwrap(); assert!(text.contains("Status plane"), "{text}"); assert!(text.contains("L1~"), "bob's thread is named through the link: {text}");
    let (ok, r) = t.ba.tool_call(&th_a, "plane_read", &json!({"machine":"nowhere"})).await; assert!(!ok); assert!(r["error"].as_str().unwrap().contains("no machine of yours called nowhere"));
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] async fn nothing_crosses_a_link_that_is_not_there_and_a_room_is_not_taken_by_guessing_its_id() {
    let t = two("down").await;
    let th_a = thread_of(&t.ha, &t.ada);
    t.wa.up.store(false, std::sync::atomic::Ordering::SeqCst); // away, or not the owner's own: the transport answers nothing
    let (ok, r) = t.ba.tool_call(&th_a, "bot_send", &json!({"to":"bob@deskB","text":"x","topic":"t"})).await; assert!(ok, "the room is made here; {r}");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(!log(&t.hb).iter().any(|e| e["$"] == "RoomSet" || e["$"] == "RoomPosted"), "B heard nothing");
    let (ok, r) = t.ba.tool_call(&th_a, "plane_read", &json!({"machine":"deskB"})).await; assert!(!ok); assert!(r["error"].as_str().unwrap().contains("away"), "{r}");
    let (ok, r) = t.ba.tool_call(&th_a, "room_create", &json!({"name":"q","members":"bob@elsewhere"})).await; assert!(!ok); assert!(r["error"].as_str().unwrap().contains("no linked machine called elsewhere"), "{r}");
    // B has a room of its own; the other machine names its id and is told no
    let th_b = thread_of(&t.hb, &t.bob);
    assert!(t.bb.tool_call(&th_b, "room_create", &json!({"name":"mine","members":""})).await.0);
    let mine = log(&t.hb).iter().find(|e| e["$"] == "RoomSet").unwrap()["id"].as_str().unwrap().to_string();
    let (c, _) = t.bb.far_received("L1", "bots.room_sync", &json!({"room":mine,"name":"x","members":["ada@L1"],"deleted":false})).await; assert_eq!(c, 409);
    let (c, _) = t.bb.far_received("L1", "bots.room_post", &json!({"room":mine,"msg":"m1","from":"ada","text":"hello"})).await; assert_eq!(c, 403);
    let (c, _) = t.bb.far_received("L1", "bots.room_sync", &json!({"room":mine,"deleted":true})).await; assert_eq!(c, 409, "a room of its own is not deleted from elsewhere either");
}

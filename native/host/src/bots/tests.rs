//! Against a real Hub on a temp home; messages into threads are recorded, no agent runs (no model, no network).
use super::*;
use crate::{api::{Caller, Fut}, hub::{Cfg, Hub}, presenter::Canned};
use super::rpc::*;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex as StdMutex};

type Got = (String, String, String, Option<(String, String, String)>);
#[derive(Default)] struct Rec { got: StdMutex<Vec<Got>>, stopped: StdMutex<Vec<String>>, slow: bool, hub: std::sync::OnceLock<Arc<Hub>> }
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
    // bothub.bend Bots.create.k: a name is 1 to 24 of letters, digits, - and _, and free (any case); the reply comes before the home job
    let (hub, _b, _r) = start("create", "").await;
    let r = rpc(&hub, "bots.create", json!({"name":" Ada ","persona":"dry"})).await;
    assert_eq!(r["ok"], true); let b = r["bot"].as_str().unwrap().to_string();
    let ev: Vec<Value> = log(&hub).into_iter().filter(|e| e["$"] != "SettingSet").collect(); // the hub's own start-up settings come first
    assert_eq!(ev[0]["$"], "ThreadCreated"); assert_eq!((ev[0]["title"].as_str().unwrap(), ev[0]["env"].as_str().unwrap(), ev[0]["project"].as_str().unwrap()), ("Ada", "local", ""));
    assert_eq!(ev[1]["$"], "BotSet"); assert_eq!(ev[1]["id"], b.as_str()); assert_eq!(ev[1]["thread"], ev[0]["id"]); assert_eq!(ev[1]["look"], seed("Ada"));
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
    assert_eq!(kinds(&hub, n), vec!["RoomPosted", "BotWoke", "BotWoke"]);
    let e = &log(&hub)[n]; assert_eq!((e["id"].as_str().unwrap(), e["from"].as_str().unwrap(), e["hop"].as_u64().unwrap()), ("p1", "you", 0));
    let got = rec.got.lock().unwrap().clone(); assert_eq!(got.len(), 2);
    assert_eq!(got[0].2, "[message from you in #lab]\nhello all");
    assert_eq!(got[0].3, Some(("person".into(), "room".into(), "lab".into())));
    assert!(got[0].1.ends_with(".0") && got[1].1.ends_with(".1"));
    assert_eq!(rpc(&hub, "bots.post", json!({"room":"r1","text":"hello all","msg":"p1"})).await["ok"], true);
    assert_eq!(log(&hub).len(), n + 3, "a repeat stores and wakes nothing");
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

#[test] fn world_fold_matches_bot_bend() {
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

// ---- the same scenario as the JS hub's (test/gpui/bots/record_js.mjs), compared event by event ----
const JS_EVENTS: &str = include_str!("../../../../test/gpui/bots/fixtures/js-bots.jsonl");
const JS_STEPS: &str = include_str!("../../../../test/gpui/bots/fixtures/js-steps.json");

/// test/gpui/bots/filter.py: the events a bots scenario is about, without clocks
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

const JS_ROUTINE_EVENTS: &str = include_str!("../../../../test/gpui/bots/fixtures/js-routines.jsonl");
const JS_ROUTINE_STEPS: &str = include_str!("../../../../test/gpui/bots/fixtures/js-routines-steps.json");

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

#[tokio::test] async fn session_asks_the_chat_thread_and_the_default_tab() {
    // U.Bots.chat.of and BU.tab.default for a view whose target is a bot
    let (hub, bots, _r) = start("api", "").await;
    let (a, u) = (bot(&hub, "aa").await, bot(&hub, "uu").await);
    let th = bots.world().await.bot(&a).unwrap().thread.clone();
    assert_eq!(chat_thread(&hub, &a).await, Some(th)); assert_eq!(chat_thread(&hub, "nope").await, None);
    assert_eq!(default_tab(&hub, &a, false).await, "chat"); assert_eq!(default_tab(&hub, &u, false).await, "chat");
    hub.commit(&json!({"$":"UberSet","bot":u,"uid":"u1-x","at":5})).await.unwrap();
    assert_eq!(default_tab(&hub, &u, false).await, "status", "a client without the Mailroom opens the uberbot on its Status");
    assert_eq!(default_tab(&hub, &u, true).await, "chat"); assert_eq!(default_tab(&hub, &a, false).await, "chat");
    let uth = bots.world().await.bot(&u).unwrap().thread.clone(); assert_eq!(chat_thread(&hub, &u).await, Some(uth));
}

/// the run's own machine id, uberbot id, stamp seconds and one-time instants inside op bodies, as test/gpui/bots/filter.py makes them
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

const JS_UBER_EVENTS: &str = include_str!("../../../../test/gpui/bots/fixtures/js-uber.jsonl");
const JS_UBER_STEPS: &str = include_str!("../../../../test/gpui/bots/fixtures/js-uber-steps.json");

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

const JS_UBER2_EVENTS: &str = include_str!("../../../../test/gpui/bots/fixtures/js-uber2.jsonl");

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
    // Ub.init.bot: who, memories, space, page, routines, lease (term 1), bind; the expected text is the Bend text (ubop.bend Ub.b.*)
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
    let ev = log(&hub); let new: Vec<&Value> = ev[n..].iter().filter(|e| e["$"] != "WorktreeSet").collect();
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

const JS_MAIL1_EVENTS: &str = include_str!("../../../../test/gpui/bots/fixtures/js-mail1.jsonl");
const JS_MAIL1_STEPS: &str = include_str!("../../../../test/gpui/bots/fixtures/js-mail1-steps.json");

#[tokio::test] async fn js_parity_mail_filing() {
    // the mailroom on: a room post is filed, a routine's firing is filed instead of delivered; the uberbot's routine is filed with an owner ticket; mail off with the
    // uberbot in a room files nothing (the post is already in the box). Compared with a JS-hub recording: events, and the journal files line by line.
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
    assert!(rec.got.lock().unwrap().is_empty(), "with the mailroom on nothing is delivered to a bot's agent");
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
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() { assert_eq!(x, y, "event {i}: JS left, Rust right") }
    assert_eq!(a.len(), b.len(), "same number of events\nJS: {a:#?}\nRust: {b:#?}");
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
    #[test] fn mail_body_line_is_what_bend_writes_and_reads_back() {
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
    #[test] fn the_journal_is_not_opened_through_a_link() {
        use std::os::unix::fs::symlink;
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
        use std::os::unix::fs::PermissionsExt;
        let h = home("hdr"); store::append(&h, &body("c", "m1")).unwrap(); store::append(&h, &body("c", "m2")).unwrap();
        let hs = store::headers(&store::path(&h, "c")); assert_eq!(hs.iter().map(|(_, m)| m.id.as_str()).collect::<Vec<_>>(), vec!["m1", "m2"]);
        assert_eq!(std::fs::metadata(store::path(&h, "c")).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

//! Mail slice 3 against a real Hub on a temp home: what a filed mail does next, what a run's end owes, the funnel, the uberbot's routines. Stand-ins only: a turn is events the test commits.
use super::*;
use crate::agents::mailroom::{Funnel, Mailroom};
use crate::bots::mail::{funnel::Room, tools::{lists, MailTools}, escalate::disposition_key};
use crate::agents::mcp::ToolSet;

pub(super) async fn uber(name: &str) -> (crate::hub::TestHub, Arc<Bots>, Arc<Rec>, String, String) {
    let (hub, bots, rec) = start(&format!("parity-{name}"), "").await;
    let r = rpc(&hub, "bots.create", json!({"name":"bub","uber":true})).await; let bub = r["bot"].as_str().unwrap().to_string();
    until(&hub, |l| l.iter().any(|e| e["$"] == "UberSet") && l.iter().any(|e| e["$"] == "WorktreeSet")).await;
    let th = log(&hub).iter().find(|e| e["$"] == "BotSet").unwrap()["thread"].as_str().unwrap().to_string();
    hub.commit(&json!({"$":"SettingSet","key":"machine.id","value":"m1","at":1})).await.unwrap();
    assert!(bots.uber_init(now_s()).await > 0);
    (hub, bots, rec, bub, th)
}
fn runs(hub: &Arc<Hub>) -> Vec<Value> { log(hub).into_iter().filter(|e| e["$"] == "RunSet").collect() }
pub(super) async fn settle() { tokio::time::sleep(std::time::Duration::from_millis(300)).await }
/// what an agent's turn leaves in the log; the mailroom answers from the log, as it does for a real one
async fn turn(hub: &Arc<Hub>, th: &str, end: &str) { turn_said(hub, th, end, "worked on it").await }
pub(super) async fn turn_said(hub: &Arc<Hub>, th: &str, end: &str, text: &str) {
    hub.commit_many(&[json!({"$":"TurnChanged","thread":th,"state":"running","at":now_s()})]).await.unwrap();
    hub.commit_many(&[json!({"$":"MessagePosted","thread":th,"msg":format!("a{}", hub.store.len()),"role":"assistant","text":text,"at":now_s()}), json!({"$":"TurnChanged","thread":th,"state":end,"at":now_s()})]).await.unwrap();
}
async fn running(hub: &Arc<Hub>, th: &str) { hub.commit(&json!({"$":"TurnChanged","thread":th,"state":"running","at":now_s()})).await.unwrap(); }
async fn end(hub: &Arc<Hub>, th: &str, st: &str) { hub.commit(&json!({"$":"TurnChanged","thread":th,"state":st,"at":now_s()})).await.unwrap(); }
async fn routine(hub: &Arc<Hub>, bot: &str, id: &str) { assert_eq!(rpc(hub, "bots.routine", json!({"bot":bot,"routine":id,"name":id,"schedule":"daily 09:30","prompt":"go"})).await["ok"], true); assert_eq!(rpc(hub, "bots.routine_run", json!({"routine":id})).await["ok"], true); }
fn setting_of(hub: &Arc<Hub>, k: &str) -> String { log(hub).iter().rev().find(|e| e["$"] == "SettingSet" && e["key"] == k).map_or(String::new(), |e| e["value"].as_str().unwrap().to_string()) }
fn origin(f: &str, k: &str, r: &str) -> (String, String, String) { (f.into(), k.into(), r.into()) }

#[tokio::test] async fn overdue_uberbot_routines_fire_once_each_never_a_backlog_per_missed_minute() {
    // the machine slept ten hours through two hourly routines of the uberbot: each fires once, each with its own op, and no later minute of that hour fires anything
    let (hub, bots, _r, bub, _) = uber("backlog").await;
    for id in ["ra", "rb"] { assert_eq!(rpc(&hub, "bots.routine", json!({"bot":bub,"routine":id,"name":id,"schedule":"@hourly","prompt":"go"})).await["ok"], true) }
    let wake = (now_s() + 36000) / 3600 * 3600 + 60; // a minute past an hour, ten slots later
    assert_eq!(bots.tick_at(wake).await, 2, "two routines overdue by ten slots each: one firing each");
    for m in 1..30 { assert_eq!(bots.tick_at(wake + m * 60).await, 0, "minute {m} after the wake fires nothing") }
    let l = log(&hub);
    assert_eq!(l.iter().filter(|e| e["$"] == "RoutineFired").count(), 2);
    let ops: Vec<&Value> = l.iter().filter(|e| e["$"] == "UberOp" && e["op"].as_str().unwrap_or("").contains("\"fired\"")).collect(); assert_eq!(ops.len(), 2, "one fired op a slot");
    assert_ne!(ops[0]["n"], ops[1]["n"], "the second routine of a tick decides on the first's op count");
    { let w = bots.world().await; let me = w.me(); assert_eq!(w.vv(&w.uid(), &me), ops[1]["n"].as_u64().unwrap(), "both ops were taken by the fold"); }
    settle().await;
    assert_eq!(log(&hub).iter().filter(|e| e["$"] == "MailPosted" && e["kind"] == "routine").count(), 2, "filed once each");
    assert_eq!(bots.tick_at(wake + 3700).await, 2, "the next slot fires once more");
}

#[tokio::test] async fn a_routine_of_the_uberbot_reaches_a_run_and_never_its_chat() {
    let (hub, _b, rec, bub, chat) = uber("reach").await;
    routine(&hub, &bub, "r1").await; until(&hub, |l| l.iter().any(|e| e["$"] == "RunSet")).await;
    settle().await;
    let got = rec.got.lock().unwrap().clone(); assert_eq!(got.len(), 1, "{got:?}"); assert!(got[0].0.starts_with("run"), "a thread of its own"); assert!(got[0].2.starts_with("<charter>"));
    assert!(got.iter().all(|g| g.0 != chat), "nothing in the person's conversation");
}

#[tokio::test] async fn a_run_that_ends_without_a_receipt_goes_to_a_private_lane_once_and_the_lane_is_cut_back_to_the_owner() {
    let (hub, _b, rec, bub, chat) = uber("lane").await;
    routine(&hub, &bub, "r1").await; until(&hub, |l| l.iter().any(|e| e["$"] == "RunSet")).await;
    let run = runs(&hub)[0].clone(); let th = run["thread"].as_str().unwrap().to_string(); let input = run["mail"].as_str().unwrap().to_string();
    turn(&hub, &th, "completed").await;
    until(&hub, |l| l.iter().filter(|e| e["$"] == "RunSet").count() == 2).await; settle().await;
    let lane = runs(&hub)[1]["thread"].as_str().unwrap().to_string(); assert!(lane.starts_with(&format!("mailcoord-{bub}-")), "{lane}");
    assert_eq!(setting_of(&hub, &format!("run.forwarded.{th}")), input);
    assert_eq!(setting_of(&hub, &format!("mail.private.owner.{lane}")), bub);
    assert_eq!(setting_of(&hub, &format!("mail.private.input.{lane}")), input);
    let rides = rec.rides.lock().unwrap().clone(); assert_eq!(rides.len(), 1); assert_eq!(rides[0].0, lane); assert_eq!(rides[0].1, format!("triage:{input}")); assert!(rides[0].2.contains("<private_mailroom_turn") && rides[0].2.contains(&input));
    assert!(rec.got.lock().unwrap().iter().all(|g| g.0 != chat), "never the person's conversation");
    // the back office is cut off before it told anyone: its owner takes the note over, once
    turn(&hub, &lane, "failed").await; settle().await; turn(&hub, &lane, "failed").await; settle().await;
    let rides = rec.rides.lock().unwrap().clone(); assert_eq!(rides.len(), 2, "{rides:?}"); assert_eq!(rides[1].0, chat); assert_eq!(rides[1].1, format!("lost:{lane}"));
    assert_eq!(runs(&hub).len(), 2, "no lane starts another");
}

#[tokio::test] async fn a_disposed_input_needs_nothing_more_and_the_queue_goes_on() {
    let (hub, bots, _r, bub, _) = uber("dispose").await;
    routine(&hub, &bub, "r1").await; until(&hub, |l| l.iter().any(|e| e["$"] == "RunSet")).await;
    let run = runs(&hub)[0].clone(); let th = run["thread"].as_str().unwrap().to_string(); let input = run["mail"].as_str().unwrap().to_string();
    running(&hub, &th).await; settle().await;
    // a second firing while the run is live waits in the queue, unbound to it
    assert_eq!(rpc(&hub, "bots.routine_run", json!({"routine":"r1"})).await["ok"], true); settle().await;
    assert_eq!(runs(&hub).len(), 1); let key = format!("triage.queue.{}", serde_json::to_string(&json!([bub, "routine:r1"])).unwrap()); assert_ne!(setting_of(&hub, &key), "[]");
    let bad = bots.mail_tool(&th, "mail_dispose", &json!({"input":"nope","mode":"done","reason":"x"})).await; assert!(!bad.0); assert_eq!(bad.1["current_input"], input.as_str());
    let extra = bots.mail_tool(&th, "mail_dispose", &json!({"input":input,"mode":"done","reason":"x","more":"y"})).await; assert!(!extra.0, "no other fields");
    let ok = bots.mail_tool(&th, "mail_dispose", &json!({"input":input,"mode":"done","reason":"routine status"})).await; assert!(ok.0, "{ok:?}"); assert_eq!(ok.1["state"], "handled"); assert_eq!(ok.1["next_input"], "");
    let again = bots.mail_tool(&th, "mail_dispose", &json!({"input":input,"mode":"done","reason":"routine status"})).await; assert_eq!(again.1["state"], "already handled");
    assert!(!setting_of(&hub, &disposition_key(&th, &input)).is_empty());
    end(&hub, &th, "completed").await;
    until(&hub, |l| l.iter().filter(|e| e["$"] == "RunSet").count() == 2).await; settle().await;
    let next = runs(&hub)[1].clone(); assert!(next["thread"].as_str().unwrap().starts_with("run"), "the waiting input is a fresh run, no back office"); assert_ne!(next["mail"], run["mail"]);
    assert_eq!(setting_of(&hub, &key), "[]");
    assert!(!log(&hub).iter().any(|e| e["$"] == "ThreadCreated" && e["id"].as_str().unwrap().starts_with("mailcoord-")));
}

#[tokio::test] async fn a_stop_holds_the_queue_with_the_input_at_its_front_and_a_restart_cut_does_not() {
    let (hub, bots, _r, bub, _) = uber("hold").await;
    routine(&hub, &bub, "r1").await; until(&hub, |l| l.iter().any(|e| e["$"] == "RunSet")).await;
    let run = runs(&hub)[0].clone(); let th = run["thread"].as_str().unwrap().to_string(); let input = run["mail"].as_str().unwrap();
    running(&hub, &th).await; settle().await;
    let room = Room(Arc::downgrade(&bots)); room.stopping(&th).await;
    end(&hub, &th, "interrupted").await; settle().await;
    let conv = "routine:r1"; let hold = format!("triage.hold.{}", serde_json::to_string(&json!([bub, conv])).unwrap()); let q = format!("triage.queue.{}", serde_json::to_string(&json!([bub, conv])).unwrap());
    assert_eq!(setting_of(&hub, &hold), th); assert_eq!(setting_of(&hub, &q), serde_json::to_string(&json!([input])).unwrap());
    assert_eq!(runs(&hub).len(), 1, "an interrupted run is not forwarded");
    assert_eq!(rpc(&hub, "bots.routine_run", json!({"routine":"r1"})).await["ok"], true); settle().await;
    assert_eq!(runs(&hub).len(), 1, "a held conversation starts nothing");
}

#[tokio::test] async fn escalate_starts_a_lane_once_per_key_and_tell_person_publishes_once() {
    let (hub, bots, rec, bub, chat) = uber("tell").await;
    routine(&hub, &bub, "r1").await; until(&hub, |l| l.iter().any(|e| e["$"] == "RunSet")).await;
    let run = runs(&hub)[0].clone(); let th = run["thread"].as_str().unwrap().to_string(); let input = run["mail"].as_str().unwrap().to_string();
    running(&hub, &th).await; settle().await;
    assert!(!bots.mail_tool(&th, "escalate", &json!({"to":"person","text":"x"})).await.0, "the exact input is required");
    assert!(bots.mail_tool(&th, "escalate", &json!({"to":"person","text":"done","input":input,"key":"ev-1"})).await.0);
    until(&hub, |l| l.iter().filter(|e| e["$"] == "RunSet").count() == 2).await; settle().await;
    let lane = runs(&hub)[1]["thread"].as_str().unwrap().to_string(); running(&hub, &lane).await; settle().await;
    let names: Vec<String> = MailTools(bots.clone()).list(&lane).iter().map(|t| t["name"].as_str().unwrap().to_string()).collect(); assert!(names.contains(&"tell_person".to_string()) && names.contains(&"mail_context".to_string()), "{names:?}");
    assert!(!bots.mail_tool(&th, "tell_person", &json!({"text":"t","key":"k"})).await.0, "only a live back office publishes");
    assert!(!bots.mail_tool(&lane, "tell_person", &json!({"text":"t","key":"k","x":"y"})).await.0);
    assert!(bots.mail_tool(&lane, "tell_person", &json!({"text":"The report is in.","key":"ev-1"})).await.0); settle().await;
    assert!(bots.mail_tool(&lane, "tell_person", &json!({"text":"The report is in.","key":"ev-1"})).await.0, "said again: no second post"); settle().await;
    let posted: Vec<Value> = log(&hub).into_iter().filter(|e| e["$"] == "MessagePosted" && e["thread"] == chat.as_str() && e["role"] == "assistant").collect(); assert_eq!(posted.len(), 1, "{posted:?}"); assert_eq!(posted[0]["text"], "The report is in.");
    assert!(rec.notes.lock().unwrap().iter().any(|r| r.0 == chat && r.1.starts_with("told:")), "the owner's agent hears what was said");
    assert!(!rec.rides.lock().unwrap().iter().any(|r| r.1.starts_with("told:")), "a tell is a note alone: it starts no turn in the person's conversation");
    assert!(matches!(bots.notify_relay(&lane, &json!({"reason":"r","text":"t"})).await, Some((false, _))), "a back office has only tell_person");
    // the lane's end owes nothing more: it told
    end(&hub, &lane, "failed").await; settle().await;
    assert!(!rec.rides.lock().unwrap().iter().any(|r| r.1.starts_with("lost:")));
}

#[tokio::test] async fn the_funnel_files_every_inbound_for_the_uberbot_and_leaves_the_rest() {
    let (hub, bots, _r, _bub, chat) = uber("funnel").await;
    let ada = bot(&hub, "ada").await; let at = bots.world().await.bot(&ada).unwrap().thread.clone();
    let room = Room(Arc::downgrade(&bots));
    let f = |r: Funnel| matches!(r, Funnel::Filed(Ok(())));
    // a thread's message, a launch report, a task's note, a wake, work's note: filed, none posted in the chat
    assert!(f(room.funnel("user", &chat, &format!("t:{at}:m1"), "a word", &origin(&format!("thread:{at}"), "thread", ""), "", &[]).await));
    let ack = json!({"$":"SettingSet","key":"autosettle.report.tx","value":"1","at":1});
    assert!(f(room.funnel("user", &chat, "t:tx:lr-1", "[Backplane] Launched thread", &origin("tx", "launch", ""), "", std::slice::from_ref(&ack)).await));
    assert!(f(room.funnel("system", &chat, "t:tx:td-1", "[Backplane] Delegated task", &origin("tx", "task", ""), "", &[]).await));
    assert!(f(room.funnel("user", &chat, "t:tx:lr-2", "Wake w1: done", &origin("tx", "wake", "w1"), "thread:tx:k", &[]).await));
    assert!(f(room.funnel("system", &chat, "w1", "work ended", &origin("backplane", "system", "work"), "", &[]).await));
    settle().await;
    let l = log(&hub); let mails: Vec<&Value> = l.iter().filter(|e| e["$"] == "MailPosted").collect(); assert_eq!(mails.len(), 5, "{mails:?}");
    assert!(mails.iter().all(|m| m["to"].as_str().unwrap().starts_with("bot:")));
    let by = |k: &str| mails.iter().find(|m| m["kind"] == k).unwrap();
    assert_eq!((by("thread")["from"].as_str().unwrap(), by("thread")["want"].as_str().unwrap()), (format!("bot:{ada}").as_str(), "act"), "a bot's thread is its bot; a direct message asks");
    assert_eq!(by("launch")["want"], "fyi"); assert_eq!(by("task")["want"], "fyi"); assert_eq!(by("wake")["id"], "wake:thread:tx:k");
    let i = l.iter().position(|e| e["$"] == "MailPosted" && e["kind"] == "launch").unwrap(); assert!(l[i..].iter().any(|e| e["key"] == "autosettle.report.tx"), "the producer's acknowledgement rides with the header");
    assert!(!l.iter().any(|e| e["$"] == "MessagePosted" && e["thread"] == chat.as_str()), "no line in the person's conversation");
    assert!(l.iter().any(|e| e["$"] == "RunSet"), "and the mail is routed to a run");
    // the same id again only commits the acknowledgement
    let n = log(&hub).iter().filter(|e| e["$"] == "MailPosted").count(); assert!(f(room.funnel("user", &chat, "t:tx:lr-1", "x", &origin("tx", "launch", ""), "", &[]).await)); settle().await; assert_eq!(log(&hub).iter().filter(|e| e["$"] == "MailPosted").count(), n);
    // a person's message and any other thread pass; with the mailroom on a thread is delivered and filed, a bot only filed
    assert!(matches!(room.funnel("user", &chat, "m9", "hi", &origin("", "person", ""), "", &[]).await, Funnel::Deliver));
    assert!(matches!(room.funnel("user", &at, "t:x:m2", "hi", &origin("thread:x", "thread", ""), "", &[]).await, Funnel::Deliver));
    hub.commit(&json!({"$":"SettingSet","key":"mail.on","value":"on","at":2})).await.unwrap();
    assert!(matches!(room.funnel("user", &at, "t:x:m3", "[message from bob]\nhi", &origin("thread:x", "thread", ""), "", &[]).await, Funnel::Filed(_)));
    let t2 = rpc(&hub, "bots.create", json!({"name":"tmp"})).await; let _ = t2;
    assert!(matches!(room.funnel("user", "t-plain", "t:x:m4", "[message from bob]\nhi", &origin("thread:x", "thread", ""), "", &[]).await, Funnel::Tee));
}

#[tokio::test] async fn reports_go_to_the_holders_thread_and_an_ask_to_triage() {
    let (hub, bots, _r, _bub, chat) = uber("dest").await;
    let room = Room(Arc::downgrade(&bots));
    assert_eq!(room.dest(&chat).await, chat, "this machine holds the conversation"); assert_eq!(room.dest("t-other").await, "t-other");
    let (uid, me) = { let w = bots.world().await; (w.uid(), w.me()) };
    // the lease moves to m2, whose binding and link are known: the holder's thread is that machine's
    let ops = { let w = bots.world().await; w.ops(now_s() + 5, &[uber_bodies_lease("m2", 2)]) }; hub.commit_many(&ops).await.unwrap();
    hub.commit(&json!({"$":"SettingSet","key":format!("uber.link.{uid}.m2"),"value":"l2","at":3})).await.unwrap();
    hub.commit(&json!({"$":"UberOp","uid":uid,"origin":"m2","n":1,"stamp":"9999999999.000000.m2","op":crate::bots::uber::b_bind("m2", "b2", "t2", "bub"),"at":3})).await.unwrap();
    assert_eq!(room.dest(&chat).await, "l2~t2"); assert!(room.chat_is("l2~t2").await); assert!(room.chat_is(&chat).await, "this machine's thread is still one of them");
    assert_eq!(room.dest("t-other").await, "t-other"); let _ = me;
}
fn uber_bodies_lease(m: &str, t: u64) -> String { crate::bots::uber::b_lease(m, t) }

#[tokio::test] async fn a_memory_saved_before_the_first_ops_does_not_lose_who_lease_and_bind() {
    // hub's bug: an op before the minute's init made vv 1, and init never ran. Here init runs whenever this machine has no bind op, and once
    let (hub, bots, _r) = start("parity-init", "").await;
    let r = rpc(&hub, "bots.create", json!({"name":"bub","uber":true})).await; let bub = r["bot"].as_str().unwrap().to_string();
    until(&hub, |l| l.iter().any(|e| e["$"] == "UberSet")).await;
    hub.commit(&json!({"$":"SettingSet","key":"machine.id","value":"m1","at":1})).await.unwrap();
    assert_eq!(rpc(&hub, "bots.remember", json!({"bot":bub,"key":"k","kind":"fact","text":"t"})).await["ok"], true);
    assert_eq!(log(&hub).iter().filter(|e| e["$"] == "UberOp").count(), 1);
    assert!(bots.uber_init(now_s()).await > 0, "the memory's op does not stop init");
    assert!(log(&hub).iter().any(|e| e["$"] == "UberOp" && e["op"].as_str().unwrap().contains("\"bind\"")));
    assert_eq!(bots.uber_init(now_s()).await, 0, "and it runs once");
}

#[tokio::test] async fn a_report_the_journal_refused_reaches_a_private_lane_and_is_not_marked_filed() {
    let (hub, bots, rec, bub, chat) = uber("failure").await;
    let _ = std::fs::remove_dir_all(hub.cfg.home.join("mail")); std::fs::write(hub.cfg.home.join("mail"), "not a folder").unwrap();
    let room = Room(Arc::downgrade(&bots)); let ack = json!({"$":"TaskDelivered","task":"k1","at":1});
    let r = room.funnel("system", &chat, "t:tx:td-1", "note", &origin("tx", "task", ""), "", std::slice::from_ref(&ack)).await;
    assert!(matches!(r, Funnel::Filed(Err(_))), "the producer hears it failed");
    until(&hub, |l| l.iter().any(|e| e["$"] == "RunSet")).await; settle().await;
    assert!(!log(&hub).iter().any(|e| e["$"] == "TaskDelivered"), "no acknowledgement for what was never filed");
    assert!(!log(&hub).iter().any(|e| e["$"] == "MailPosted"));
    let run = runs(&hub)[0].clone(); assert!(run["thread"].as_str().unwrap().starts_with(&format!("mailcoord-{bub}-"))); assert_eq!(run["mail"], "delivery-failed:t:tx:td-1");
    assert!(rec.rides.lock().unwrap()[0].2.contains("Internal mail delivery FAILED"));
}

#[tokio::test] async fn a_runs_risky_tools_follow_the_charter_and_a_run_asks_no_one() {
    let (hub, bots, _r, bub, _) = uber("charter").await;
    routine(&hub, &bub, "r1").await; until(&hub, |l| l.iter().any(|e| e["$"] == "RunSet")).await;
    let th = runs(&hub)[0]["thread"].as_str().unwrap().to_string(); let room = Room(Arc::downgrade(&bots));
    assert_eq!(room.permit(&th, "Read", &json!({"file_path":"/x"})).await.unwrap()["behavior"], "allow", "not risky: no ask");
    let v = room.permit(&th, "Bash", &json!({"command":"git push"})).await.unwrap(); assert_eq!(v["behavior"], "deny"); assert!(v["message"].as_str().unwrap().contains("private triage charter"));
    assert!(room.permit("t-other", "Bash", &json!({"command":"git push"})).await.is_none(), "any other thread keeps its ask");
    hub.commit(&json!({"$":"CharterSet","bot":bub,"text":"may: tool:deploy_main","by":"bot","at":1})).await.unwrap();
    assert!(room.gate(&th, "deploy_main", &json!({})).await.is_none(), "granted: the call goes on"); settle().await;
    assert!(log(&hub).iter().any(|e| e["$"] == "DecisionLogged" && e["reason"] == "charter: may: tool:deploy_main"));
    assert_eq!(room.gate(&th, "deploy_restart", &json!({})).await.unwrap().0, false);
}

#[tokio::test] async fn a_private_lane_is_read_only_by_its_owner_and_a_run_works_as_its_bot() {
    let (hub, bots, _r, bub, chat) = uber("reach2").await; let ada = bot(&hub, "ada").await; let at = bots.world().await.bot(&ada).unwrap().thread.clone();
    routine(&hub, &bub, "r1").await; until(&hub, |l| l.iter().any(|e| e["$"] == "RunSet")).await;
    let th = runs(&hub)[0]["thread"].as_str().unwrap().to_string(); turn(&hub, &th, "failed").await;
    until(&hub, |l| l.iter().filter(|e| e["$"] == "RunSet").count() == 2).await;
    let lane = runs(&hub)[1]["thread"].as_str().unwrap().to_string(); let room = Room(Arc::downgrade(&bots));
    assert_eq!(room.reach(&at, &lane).await, Some(false), "another bot may not read a back office"); assert_eq!(room.reach(&chat, &lane).await, Some(true));
    assert_eq!(room.reach("t-plain", &at).await, None, "a plain thread goes by its project");
    assert_eq!(room.launcher(&lane).await, chat, "a back office launches as its owner");
    let w = bots.world().await; assert_eq!(w.run_bot(&lane).unwrap().id, bub); assert!(w.managed(&at) && w.managed(&lane)); let l = lists(&w); assert!(l[&chat].contains(&"mail_list"));
}

// ---- the JS hub's own run of the scenario (test/bots/record_triage.mjs), compared line by line ----
const JS_TRIAGE: &str = include_str!("../../../../../test/bots/fixtures/js-triage.jsonl");
const JS_TRIAGE_CALLS: &str = include_str!("../../../../../test/bots/fixtures/js-triage-calls.json");

/// the events routing and escalation are about: a hub's run ids become ordinals, its clock, machine and uid go
/// a pair's two addresses in one order: `pair` sorts them as text, and a hub id's own digits (`-15` before `-7`) differ from run to run
fn pair_sorted(s: &str) -> String {
    let (mut o, mut rest) = (String::new(), s);
    while let Some(i) = rest.find("pair:") {
        let (head, tail) = rest.split_at(i + 5); o += head;
        let end = tail.find(|c| c == '"' || c == '\\').unwrap_or(tail.len()); let (id, after) = tail.split_at(end);
        match id.split_once('|') { Some((a, b)) if a > b => { o += &format!("{b}|{a}") } _ => o += id }
        rest = after;
    }
    o + rest
}
fn triage_view(ev: &[Value]) -> Vec<String> {
    let mine = |k: &str| ["run.", "triage.", "mail.private.", "owner.mail.pending."].iter().any(|p| k.starts_with(p)) || k.starts_with("told.tell") || (k.starts_with("account.thread.") && (k.contains(".run") || k.contains("mailcoord")));
    let (uid, me): (String, String) = (ev.iter().find(|e| e["$"] == "UberSet").map_or(String::new(), |e| e["uid"].as_str().unwrap().into()), ev.iter().find(|e| e["$"] == "UberOp").map_or(String::new(), |e| e["origin"].as_str().unwrap().into()));
    let keep: Vec<Value> = ev.iter().filter(|e| { let (k, id) = (e["$"].as_str().unwrap_or(""), e["id"].as_str().unwrap_or(""));
        let ours = id.starts_with("run") || id.starts_with("mailcoord-");
        match k { "RunSet" | "MailMarked" | "DecisionLogged" | "ConvSet" | "MailPosted" | "MessageFrom" => !(k == "MessageFrom" && e["kind"] != "tell-person"), "ThreadCreated" | "ModesSet" => ours, "SettingSet" => mine(e["key"].as_str().unwrap_or("")), "MessagePosted" => id.is_empty() && e["msg"].as_str().unwrap_or("").starts_with("tell"), _ => false } })
        .map(|e| { let mut o = e.clone(); let m = o.as_object_mut().unwrap(); m.remove("at"); if m.contains_key("worktree") { m.insert("worktree".into(), json!("<w>")); } o }).collect();
    let mut seen: Vec<String> = vec![];
    keep.iter().map(|e| {
        let s = serde_json::to_string(e).unwrap().replace(&uid, "<uid>").replace(&me, "<me>"); let cs: Vec<char> = s.chars().collect(); let (mut o, mut i) = (String::new(), 0);
        while i < cs.len() {
            // <letters><10 digits>-<digits>: an id the hub made
            let mut j = i; while j < cs.len() && cs[j].is_ascii_lowercase() { j += 1 }
            let mut d = j; while d < cs.len() && cs[d].is_ascii_digit() { d += 1 }
            if d - j == 10 && d < cs.len() && cs[d] == '-' && cs.get(d + 1).map_or(false, |c| c.is_ascii_digit()) && (i == 0 || !cs[i - 1].is_ascii_alphanumeric()) {
                let mut e2 = d + 1; while e2 < cs.len() && cs[e2].is_ascii_digit() { e2 += 1 }
                let tok: String = cs[i..e2].iter().collect(); let pre: String = cs[i..j].iter().collect();
                let n = seen.iter().position(|x| *x == tok).unwrap_or_else(|| { seen.push(tok); seen.len() - 1 }); o += &format!("<{pre}#{n}>"); i = e2; continue
            }
            o.push(cs[i]); i += 1
        }
        pair_sorted(&unclock(&o))
    }).collect()
}
// Off the gate: parity with the JS hub is not a goal for the uberbot. It reads a report's `size`, which holds a thread id's digits, and those differ from run to run (`-147` here, `-7` there),
// so it is red whatever the code is. The behaviours it walks have tests of their own in this file. Run it by hand with --ignored.
#[ignore = "compares with a JS recording; a hub id's length moves a mail's size"]
#[tokio::test] async fn js_parity_triage() {
    // the scenario of record_triage.mjs on the Rust hub: a turn here is the events a stand-in's turn leaves. Compared with the JS hub's log, line by line.
    std::env::set_var("TZ", "Etc/UTC");
    let (hub, bots, _rec) = start("parity-triage", "").await; let room = Room(Arc::downgrade(&bots));
    let ada = bot(&hub, "ada").await;
    let r = rpc(&hub, "bots.create", json!({"name":"bub","uber":true})).await; let bub = r["bot"].as_str().unwrap().to_string();
    until(&hub, |l| l.iter().any(|e| e["$"] == "UberSet") && l.iter().filter(|e| e["$"] == "WorktreeSet").count() == 2).await;
    hub.commit(&json!({"$":"SettingSet","key":"machine.id","value":"machine-one","at":1})).await.unwrap(); bots.uber_init(now_s()).await;
    let one = |id: &str, name: &str, p: &str| json!({"bot":bub,"routine":id,"name":name,"at":"in 3d","prompt":p});
    let next = |n: usize| { let h = hub.clone(); async move { until(&h, |l| l.iter().filter(|e| e["$"] == "RunSet").count() >= n).await; settle().await; runs(&h)[n - 1].clone() } };
    let th = |r: &Value| r["thread"].as_str().unwrap().to_string();
    // a run with no receipt: its back office
    rpc(&hub, "bots.routine", one("ra", "quick", "mode:plain quick report")).await; rpc(&hub, "bots.routine_run", json!({"routine":"ra"})).await;
    let r1 = next(1).await; turn(&hub, &th(&r1), "completed").await; let l1 = next(2).await; turn(&hub, &th(&l1), "completed").await; settle().await;
    // disposed
    rpc(&hub, "bots.routine", one("rb", "slow", "mode:stop slow report")).await; rpc(&hub, "bots.routine_run", json!({"routine":"rb"})).await;
    let r2 = next(3).await; running(&hub, &th(&r2)).await; settle().await; let i2 = r2["mail"].as_str().unwrap();
    let want: Vec<Value> = serde_json::from_str(JS_TRIAGE_CALLS).unwrap(); let call = |tag: &str| want.iter().find(|c| c["tag"] == tag).unwrap().clone();
    let text = |c: &Value| c["result"]["content"][0]["text"].as_str().unwrap().to_string(); let err = |c: &Value| c["result"]["isError"].as_bool().unwrap();
    let same = |tag: &str, got: (bool, Value)| { let c = call(tag); assert_eq!(!got.0, err(&c), "{tag}"); let w: Value = serde_json::from_str(&text(&c)).unwrap(); for k in ["state", "scope", "error", "ok"] { assert_eq!(got.1.get(k), w.get(k), "{tag}: {k}") } };
    same("wrong input", bots.mail_tool(&th(&r2), "mail_dispose", &json!({"input":"nope","mode":"done","reason":"x"})).await);
    same("done", bots.mail_tool(&th(&r2), "mail_dispose", &json!({"input":i2,"mode":"done","reason":"routine status"})).await);
    same("again", bots.mail_tool(&th(&r2), "mail_dispose", &json!({"input":i2,"mode":"done","reason":"routine status"})).await);
    same("decide", bots.mail_tool(&th(&r2), "decide", &json!({"text":"filed it","reason":"routine"})).await);
    end(&hub, &th(&r2), "completed").await; settle().await;
    // escalated with a key, told once
    rpc(&hub, "bots.routine", one("rc", "escalated", "mode:stop to escalate")).await; rpc(&hub, "bots.routine_run", json!({"routine":"rc"})).await;
    let r3 = next(4).await; running(&hub, &th(&r3)).await; settle().await; let i3 = r3["mail"].as_str().unwrap();
    same("no input", bots.mail_tool(&th(&r3), "escalate", &json!({"to":"person","text":"x"})).await);
    same("keyed", bots.mail_tool(&th(&r3), "escalate", &json!({"to":"person","text":"mode:stop it finished","input":i3,"key":"ev-1"})).await);
    let l3 = next(5).await; running(&hub, &th(&l3)).await; settle().await;
    same("tell", bots.mail_tool(&th(&l3), "tell_person", &json!({"text":"The report is in.","key":"ev-1"})).await);
    same("tell again", bots.mail_tool(&th(&l3), "tell_person", &json!({"text":"The report is in.","key":"ev-1"})).await);
    same("tell bad args", bots.mail_tool(&th(&l3), "tell_person", &json!({"text":"x","key":"k","extra":"no"})).await);
    same("notify from a back office", bots.notify_relay(&th(&l3), &json!({"reason":"r","text":"t"})).await.unwrap());
    end(&hub, &th(&r3), "completed").await; settle().await; end(&hub, &th(&l3), "completed").await; settle().await;
    // stopped
    rpc(&hub, "bots.routine", one("rd", "held", "mode:stop to stop")).await; rpc(&hub, "bots.routine_run", json!({"routine":"rd"})).await;
    let r4 = next(6).await; running(&hub, &th(&r4)).await; settle().await;
    end(&hub, &th(&r4), "interrupted").await; room.stopping(&th(&r4)).await; settle().await;
    assert_eq!(rpc(&hub, "bots.routine_run", json!({"routine":"rd"})).await["ok"], false, "a done routine is not run again");
    // another bot's report, then a thread's word for the uberbot while that run is live
    let at = bots.world().await.bot(&ada).unwrap().thread.clone();
    rpc(&hub, "bots.say", json!({"bot":ada,"text":"mode:plain report please"})).await; turn_said(&hub, &at, "completed", "REPLY mode:plain report please").await; // the recorded stand-in's words: a report's size is in the log
    let r5 = next(7).await; running(&hub, &th(&r5)).await; settle().await;
    let chat = bots.world().await.triage_thread();
    assert!(matches!(room.funnel("user", &chat, &format!("t:{at}:m{}-9", now_s()), "mode:plain a word for the uberbot", &origin(&format!("thread:{at}"), "thread", ""), "", &[]).await, Funnel::Filed(Ok(())))); settle().await;
    let js: Vec<Value> = JS_TRIAGE.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let (a, b) = (triage_view(&js), triage_view(&log(&hub)));
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() { assert_eq!(x, y, "line {i}: JS left, Rust right") }
    assert_eq!(a.len(), b.len(), "the same lines\nJS: {:#?}\nRust: {:#?}", &a[a.len().min(b.len())..], &b[a.len().min(b.len())..]);
}

#[tokio::test] async fn a_bots_agent_is_told_who_it_is_and_a_run_only_its_role() {
    let (hub, bots, _r, bub, chat) = uber("prompt").await; let ada = rpc(&hub, "bots.create", json!({"name":"ada","persona":"dry and brief"})).await["bot"].as_str().unwrap().to_string();
    until(&hub, |l| l.iter().filter(|e| e["$"] == "WorktreeSet").count() == 2).await;
    let at = bots.world().await.bot(&ada).unwrap().thread.clone();
    let p = bots.bot_prompt(&at).await; assert!(p.contains("# You are ada, a Backplane bot") && p.contains("dry and brief") && p.contains(&format!("/bots/{ada})")), "{p}"); assert!(!p.contains("You are the uberbot"));
    assert!(p.contains("A room pauses after 12 messages in a row") && p.contains("in their time zone, which is "), "{p}"); // the zone itself is the host's, or whatever TZ another test of this process set
    let u = bots.bot_prompt(&chat).await; assert!(u.contains("# You are the uberbot") && u.contains("<recent_private_triage_assessments>"), "{u}");
    assert_eq!(bots.bot_prompt("t-plain").await, "", "a plain thread has no bot part");
    routine(&hub, &bub, "r1").await; until(&hub, |l| l.iter().any(|e| e["$"] == "RunSet")).await; settle().await;
    let th = runs(&hub)[0]["thread"].as_str().unwrap().to_string(); let r = bots.bot_prompt(&th).await;
    assert!(r.starts_with("\n# Private coordinator mailroom") && !r.contains("# You are bub"), "a triage run borrows no persona: {r}");
    assert_eq!(rpc(&hub, "bots.remember", json!({"bot":ada,"key":"tea","kind":"pref","text":"likes tea"})).await["ok"], true);
    assert!(crate::bots::mail::prompt::memory(&*bots.world().await, &at, "anything", now_s()).contains("likes tea"), "what a turn starts with");
}

#[tokio::test] async fn a_mention_and_a_webhook_for_the_uberbot_are_filed_and_for_another_bot_delivered() {
    let (hub, bots, rec, bub, chat) = uber("mention").await; let ada = bot(&hub, "ada").await; let at = bots.world().await.bot(&ada).unwrap().thread.clone();
    bots.mentions("t-plain", vec![bub.clone(), ada.clone()], "hello @bub @ada").await; settle().await;
    let l = log(&hub); let m: Vec<&Value> = l.iter().filter(|e| e["$"] == "MailPosted").collect(); assert_eq!(m.len(), 1, "{m:?}"); assert_eq!(m[0]["to"], format!("bot:{bub}").as_str()); assert_eq!(m[0]["from"], "thread:t-plain");
    assert_eq!(l.iter().filter(|e| e["$"] == "BotWoke").count(), 2);
    let got = rec.got.lock().unwrap().clone(); assert!(got.iter().any(|g| g.0 == at && g.2.starts_with("[the person mentioned you in the thread")), "{got:?}"); assert!(got.iter().all(|g| g.0 != chat));
    bots.mentions(&at, vec![ada.clone()], "@ada").await; settle().await; assert_eq!(log(&hub).iter().filter(|e| e["$"] == "BotWoke").count(), 2, "never the thread's own bot");
    hub.commit_many(&[json!({"$":"HookSet","id":"h1","bot":bub,"name":"ci","revoked":false,"at":1}), json!({"$":"HookSet","id":"h2","bot":ada,"name":"ci","revoked":false,"at":1})]).await.unwrap();
    assert!(bots.hook_hit("h1", "{\"a\":1}").await); assert!(bots.hook_hit("h2", "{}").await); assert!(!bots.hook_hit("nope", "").await); settle().await;
    let l = log(&hub); let hook: Vec<&Value> = l.iter().filter(|e| e["$"] == "MailPosted" && e["kind"] == "hook").collect(); assert_eq!(hook.len(), 1); assert_eq!(hook[0]["conv"], "hook:h1"); assert_eq!(hook[0]["want"], "act");
    assert!(rec.got.lock().unwrap().iter().any(|g| g.0 == at && g.2.starts_with("[webhook: ci]")));
    assert_eq!(l.iter().filter(|e| e["$"] == "HookHit").count(), 2);
}

#[tokio::test] async fn a_held_input_keeps_a_ticket_and_a_snooze_reopens_with_the_mailroom_on() {
    let (hub, bots, _r, bub, _) = uber("legacy").await;
    routine(&hub, &bub, "r1").await; until(&hub, |l| l.iter().any(|e| e["$"] == "RunSet")).await; settle().await;
    let th = runs(&hub)[0]["thread"].as_str().unwrap().to_string(); running(&hub, &th).await; settle().await;
    assert!(bots.owner_records().await.is_empty(), "the run claimed its input");
    Room(Arc::downgrade(&bots)).stopping(&th).await; end(&hub, &th, "interrupted").await; settle().await;
    bots.owner_legacy().await; assert_eq!(bots.owner_records().await.len(), 1, "a stopped input is waiting again");
    hub.commit_many(&[json!({"$":"SettingSet","key":"mail.on","value":"on","at":1}), json!({"$":"MailMarked","bot":bub,"conv":"routine:r1","status":"snoozed","seen":0,"until":50,"at":1})]).await.unwrap();
    bots.mail_due(40).await; assert!(!log(&hub).iter().any(|e| e["$"] == "MailMarked" && e["status"] == "unread"), "not yet");
    bots.mail_due(60).await; assert!(log(&hub).iter().any(|e| e["$"] == "MailMarked" && e["status"] == "unread" && e["conv"] == "routine:r1"));
}

#[tokio::test] async fn a_room_a_thread_message_names_gets_the_message_with_where_it_was_said() {
    let (hub, bots, _rec, _bub, _chat) = uber("roommention").await; let ada = bot(&hub, "ada").await;
    assert_eq!(rpc(&hub, "bots.room", json!({"room":"r9","name":"ops lab","members":ada.clone()})).await["ok"], true);
    bots.room_mentions("t-plain", vec!["r9".into(), "gone".into()], "see #ops-lab please").await; settle().await;
    let l = log(&hub); let posts: Vec<&Value> = l.iter().filter(|e| e["$"] == "RoomPosted" && e["room"] == "r9").collect();
    assert_eq!(posts.len(), 1, "one post, the missing room skipped: {posts:?}"); assert_eq!(posts[0]["from"], "you");
    assert!(posts[0]["text"].as_str().unwrap().starts_with("[from the thread \"") && posts[0]["text"].as_str().unwrap().ends_with("see #ops-lab please"), "{:?}", posts[0]["text"]);
}

// ---- Done is material (mail/done.rs): a launched thread's finish reaches the person once, near-simultaneous finishes go out together, progress stays quiet ----
/// what the agents lane does with a launched thread's report to the uberbot (`launch_tell` → the funnel)
async fn report(bots: &Arc<Bots>, chat: &str, from: &str, kind: &str, text: &str) {
    let msg = format!("t:{from}:lr-{}", rand_n()); let o = origin(from, kind, "");
    assert!(matches!(Room(Arc::downgrade(bots)).funnel("user", chat, &msg, text, &o, "", &[]).await, Funnel::Filed(Ok(()))), "filed for the uberbot");
}
fn rand_n() -> u64 { static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1); N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) }
/// the assistant lines the person's Bub chat shows: in the uberbot's conversation, not left out as a backend turn's reply
fn shown_lines(hub: &Arc<Hub>, chat: &str) -> Vec<String> {
    let rows: Vec<Value> = log(hub).into_iter().filter(|e| e["$"] == "MessagePosted" && e["thread"] == chat).collect();
    let hid = crate::present::thread::backend_replies(rows.iter().map(|e| (e["msg"].as_str().unwrap(), true, e["role"].as_str().unwrap(), e["text"].as_str().unwrap())), true);
    rows.iter().filter(|e| e["role"] == "assistant" && !hid.contains(e["msg"].as_str().unwrap()) && !crate::present::thread::hidden(true, e["msg"].as_str().unwrap(), "assistant", e["text"].as_str().unwrap())).map(|e| e["text"].as_str().unwrap().to_string()).collect()
}
async fn due_now(hub: &Arc<Hub>, bots: &Arc<Bots>, bub: &str) { hub.commit(&json!({"$":"SettingSet","key":crate::bots::mail::done::due_key(bub),"value":"1","at":now_s()})).await.unwrap(); bots.done_tick().await; settle().await }
fn lanes(hub: &Arc<Hub>, bub: &str) -> Vec<String> { runs(hub).iter().filter_map(|r| r["thread"].as_str().filter(|t| t.starts_with(&format!("mailcoord-{bub}-"))).map(str::to_string)).collect() }

#[tokio::test] async fn a_launched_thread_finishing_reaches_the_persons_chat_as_exactly_one_line() {
    let (hub, bots, rec, bub, chat) = uber("done-one").await;
    let text = crate::agents::launch::note("t1794400000-1", "KiCad embed", "completed", "Landed 1a2b3c4 on main: KiCad opens inside the Board tab.\nDetails follow.");
    report(&bots, &chat, "t1794400000-1", "launch", &text).await; settle().await;
    assert!(runs(&hub).is_empty(), "a finish is no triage run to keep quiet: it joins the done batch");
    assert!(setting_of(&hub, &crate::bots::mail::done::batch_key(&bub)).contains("KiCad embed"));
    bots.done_tick().await; settle().await; assert!(lanes(&hub, &bub).is_empty(), "the batch waits for its window");
    due_now(&hub, &bots, &bub).await;
    let ls = lanes(&hub, &bub); assert_eq!(ls.len(), 1, "one lane for the batch");
    let ride = rec.rides.lock().unwrap().iter().find(|r| r.0 == ls[0]).cloned().unwrap();
    assert!(ride.2.contains("Done is material") && ride.2.contains("KiCad embed") && ride.2.contains("Running hub build:") && ride.2.contains("tell_person key: done:"), "{}", ride.2);
    let key = ride.2.split("tell_person key: ").nth(1).unwrap().lines().next().unwrap().to_string();
    running(&hub, &ls[0]).await; settle().await;
    let line = "KiCad embed landed (1a2b3c4): not in your running build yet, it needs a hub restart; then open a board and press Open in KiCad.";
    assert!(bots.mail_tool(&ls[0], "tell_person", &json!({"text":line,"key":key})).await.0); settle().await;
    assert!(bots.mail_tool(&ls[0], "tell_person", &json!({"text":line,"key":key})).await.0); settle().await;
    end(&hub, &ls[0], "completed").await; settle().await;
    assert_eq!(shown_lines(&hub, &chat), vec![line.to_string()], "one visible line, shown although the chat's last input was the hub's");
    let posted: Vec<Value> = log(&hub).into_iter().filter(|e| e["$"] == "MessagePosted" && e["thread"] == chat.as_str() && e["role"] == "assistant").collect();
    assert!(posted[0]["msg"].as_str().unwrap().starts_with("tell:"), "{posted:?}");
    // the same report routed again (the owner's outbox walk routes pending tickets again and again) is not a second finish
    let body = { let w = bots.world().await; let m = log(&hub).into_iter().find(|e| e["$"] == "MailPosted" && e["kind"] == "launch").unwrap(); let conv = m["conv"].as_str().unwrap();
        crate::bots::mail::Body { conv: w.mail.conv(conv), mail: w.mail.headers(conv).iter().find(|h| h.id == m["id"].as_str().unwrap()).cloned().unwrap(), text: text.clone() } };
    bots.route_body(body).await; due_now(&hub, &bots, &bub).await;
    assert_eq!(lanes(&hub, &bub).len(), 1, "no second lane"); assert!(runs(&hub).len() == 1, "and no triage run");
    assert_eq!(shown_lines(&hub, &chat).len(), 1);
}

#[tokio::test] async fn two_finishes_within_a_minute_are_one_batched_line_and_a_silent_lane_gets_the_hubs_digest() {
    let (hub, bots, rec, bub, chat) = uber("done-two").await;
    report(&bots, &chat, "t1794400000-2", "launch", &crate::agents::launch::note("t1794400000-2", "Sync-token fix", "completed", "Landed 5d6e7f8 on main.")).await;
    report(&bots, &chat, "t1794400000-3", "launch", &crate::agents::launch::note("t1794400000-3", "Scroll fixes", "completed", "Chat scrolls up without jumps.")).await; settle().await;
    assert!(runs(&hub).is_empty());
    due_now(&hub, &bots, &bub).await;
    let ls = lanes(&hub, &bub); assert_eq!(ls.len(), 1, "both finishes in one lane");
    let ride = rec.rides.lock().unwrap().iter().find(|r| r.0 == ls[0]).cloned().unwrap(); assert!(ride.2.contains("Sync-token fix") && ride.2.contains("Scroll fixes") && ride.2.contains("Finished (2)"));
    // the lane says nothing and ends: the hub posts the plain digest, once
    running(&hub, &ls[0]).await; settle().await; end(&hub, &ls[0], "completed").await; settle().await;
    end(&hub, &ls[0], "failed").await; settle().await;
    let shown = shown_lines(&hub, &chat); assert_eq!(shown.len(), 1, "{shown:?}");
    assert_eq!(shown[0], "Done:\n- Sync-token fix: Landed 5d6e7f8 on main. (backplane://thread/t1794400000-2)\n- Scroll fixes: Chat scrolls up without jumps. (backplane://thread/t1794400000-3)");
    assert!(!rec.rides.lock().unwrap().iter().any(|r| r.1.starts_with("lost:")), "a done lane is not handed over as lost");
    bots.done_tick().await; settle().await; assert_eq!(lanes(&hub, &bub).len(), 1, "nothing left to flush");
}

#[tokio::test] async fn progress_pings_and_unfinished_reports_put_no_line_in_the_persons_chat() {
    let (hub, bots, _rec, bub, chat) = uber("done-none").await;
    report(&bots, &chat, "t1794400000-4", "thread", "Still building: 3 of 7 crates done.").await;
    report(&bots, &chat, "t1794400000-4", "launch", &crate::agents::launch::note("t1794400000-4", "Windows #1102", "finished its turn and waits for an answer", "Which installer, MSI or MSIX?")).await;
    report(&bots, &chat, "t1794400000-4", "launch", &crate::agents::launch::note("t1794400000-4", "Windows #1102", "failed", "rate limited")).await; settle().await;
    assert_eq!(setting_of(&hub, &crate::bots::mail::done::batch_key(&bub)), "", "no finish among them");
    due_now(&hub, &bots, &bub).await;
    assert!(lanes(&hub, &bub).is_empty(), "no done lane");
    assert!(shown_lines(&hub, &chat).is_empty(), "nothing in the person's chat: progress goes to quiet triage");
}

#[test] fn the_owners_preferences_reach_the_mailroom_lanes_reporting_ones_first() {
    let m = |k: &str, t: &str, at: u64| crate::bots::world::Mem { bot: "b".into(), key: k.into(), kind: "pref".into(), text: t.into(), tags: String::new(), created: at, updated: at };
    let mut ms: Vec<crate::bots::world::Mem> = (0..40).map(|i| m(&format!("pref.other-{i}"), &"x".repeat(400), 100 + i)).collect();
    ms.push(m("pref.report-completions", "When work the person asked for lands or finishes, tell them right away in one short line.", 1));
    let refs: Vec<&crate::bots::world::Mem> = ms.iter().collect();
    let b = crate::bots::mail::route::pref_block(&refs);
    assert!(b.contains("<owner_preferences>") && b.contains("pref.report-completions") && b.contains("Obey them over this brief"), "an old reporting rule is kept ahead of newer others");
    assert!(b.chars().count() < 6400);
    assert!(!crate::bots::mail::route::mem_block(&refs, "anything", 200).contains("pref.report-completions"), "the <memory> block alone loses it");
}

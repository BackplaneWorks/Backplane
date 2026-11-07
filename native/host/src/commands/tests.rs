//! The thread, project and queue requests against a hub on a seeded log: the events appended, byte for byte what
//! hub's `Change.encode` writes, and the reply. Each test names def it checks.
use crate::{hub::{Cfg, Hub}, presenter::Canned};
use serde_json::{json, Value};
use std::sync::Arc;

const SEED: &str = concat!(
    "{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"proj\",\"root\":\"/work/proj\",\"at\":1}\n",
    "{\"$\":\"ThreadCreated\",\"id\":\"t1\",\"project\":\"p1\",\"title\":\"One\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":2}\n",
    "{\"$\":\"ThreadCreated\",\"id\":\"t2\",\"project\":\"p1\",\"title\":\"Two\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"/work/proj/.backplane/worktrees/w2\",\"provider\":\"claude\",\"model\":\"\",\"at\":3}\n",
    "{\"$\":\"TurnQueued\",\"thread\":\"t1\",\"msg\":\"q1\",\"text\":\"first\",\"at\":4}\n",
    "{\"$\":\"TurnQueued\",\"thread\":\"t1\",\"msg\":\"q2\",\"text\":\"second\",\"at\":4}\n",
    "{\"$\":\"AskOpened\",\"id\":\"a1\",\"thread\":\"t1\",\"kind\":\"approval\",\"tool\":\"Bash\",\"detail\":\"ls\",\"options\":\"\",\"at\":5}\n",
    "{\"$\":\"AskOpened\",\"id\":\"a2\",\"thread\":\"t1\",\"kind\":\"plan\",\"tool\":\"ExitPlanMode\",\"detail\":\"\",\"options\":\"\",\"at\":5}\n",
    "{\"$\":\"WorkSet\",\"thread\":\"t1\",\"kind\":\"bg\",\"rows\":\"b1\\tx\",\"at\":5}\n",
    "{\"$\":\"ThreadSnoozed\",\"id\":\"t1\",\"until\":99999999999,\"at\":5}\n");

async fn hub(name: &str) -> Arc<Hub> {
    let dir = std::env::temp_dir().join(format!("bp-cmds-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("events.jsonl"), SEED).unwrap(); std::fs::write(dir.join("token"), "tok").unwrap();
    let h = Hub::start(Cfg { home: dir, version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    rec(); super::worktree::SUPPRESS.store(true, std::sync::atomic::Ordering::Relaxed); // the worktree job reports on its own, later: tested by itself below
    super::start(h.clone()); h
}
/// the request, then the log lines it appended (the `at` stamp blanked) and the reply
async fn ask(h: &Arc<Hub>, m: &str, p: Value) -> (Vec<Value>, Value) {
    let n0 = h.store.len(); let (mut c, _) = h.connect(true);
    let out = h.request(&mut c, &json!({"id":7,"m":m,"p":p})).await;
    let ev = h.store.range(n0, h.store.len()).unwrap().iter().map(|l| { let mut v: Value = serde_json::from_str(l).unwrap(); if v.get("at").is_some() { v["at"] = json!(0) } v })
        .collect();
    (ev, out[0].clone())
}
fn ok() -> Value { json!({"t":"reply","id":7,"ok":true}) }

// Hub.request.k 5-10: the flag events, whatever thread the name says
#[tokio::test] async fn the_flag_requests_log_their_event_and_acknowledge() {
    let h = hub("flags").await;
    for (m, ev) in [("thread.unsettle", json!({"$":"ThreadUnsettled","id":"t1","at":0})),
        ("thread.pin", json!({"$":"ThreadPinned","id":"t1","pinned":true,"at":0})), ("thread.unpin", json!({"$":"ThreadPinned","id":"t1","pinned":false,"at":0})),
        ("thread.unarchive", json!({"$":"ThreadArchived","id":"t1","archived":false,"at":0}))] {
        let (e, r) = ask(&h, m, json!({"thread":"t1"})).await; assert_eq!(e, vec![ev], "{m}"); assert_eq!(r, ok());
    }
    // Settle also clears the background shells' rows (the seed has some); the agents module is asked to stop the rest
    let (e, r) = ask(&h, "thread.settle", json!({"thread":"t1"})).await; assert_eq!(r, ok());
    assert_eq!(e, vec![json!({"$":"ThreadSettled","id":"t1","manual":true,"at":0}), json!({"$":"WorkSet","thread":"t1","kind":"bg","rows":"","at":0})], "thread.settle");
    let (e, r) = ask(&h, "thread.pin", json!({"thread":"nope"})).await; assert_eq!(e.len(), 1); assert_eq!(r, ok());
}
// Rename.changes: trimmed, a blank title changes nothing
#[tokio::test] async fn rename_trims_and_a_blank_title_changes_nothing() {
    let h = hub("rename").await;
    let (e, r) = ask(&h, "thread.rename", json!({"thread":"t1","title":"  New name  "})).await; assert_eq!(e, vec![json!({"$":"ThreadRenamed","id":"t1","title":"New name","at":0}), json!({"$":"SettingSet","key":"title.person.t1","value":"1","at":0})]); assert_eq!(r, ok());
    let (e, r) = ask(&h, "thread.rename", json!({"thread":"t1","title":"   "})).await; assert!(e.is_empty()); assert_eq!(r, ok());
}
// Retire.archive + Archive.changes + Watch.retire: the asks that wait on the process close (a plan stays), the bg rows clear, a snooze wakes
#[tokio::test] async fn archive_closes_asks_clears_background_rows_and_wakes_a_snooze() {
    let h = hub("archive").await;
    let (e, r) = ask(&h, "thread.archive", json!({"thread":"t1"})).await; assert_eq!(r, ok());
    assert_eq!(e, vec![json!({"$":"ThreadArchived","id":"t1","archived":true,"at":0}), json!({"$":"ThreadSnoozed","id":"t1","until":0,"at":0}), json!({"$":"AskClosed","id":"a1","answer":"","at":0}),
        json!({"$":"WorkSet","thread":"t1","kind":"bg","rows":"","at":0}), json!({"$":"WorkSet","thread":"t1","kind":"watch","rows":"","at":0}),
        json!({"$":"PendingChanged","thread":"t1","count":1,"at":0})], "Pend.fix: the plan ask stays open");
    let (e, _) = ask(&h, "thread.delete", json!({"thread":"t2"})).await;
    assert_eq!(e, vec![json!({"$":"ThreadDeleted","id":"t2","at":0}), json!({"$":"WorkSet","thread":"t2","kind":"watch","rows":"","at":0})]);
}
// Hub.snooze.k
#[tokio::test] async fn snooze_takes_a_named_span_or_an_until() {
    let h = hub("snooze").await; let now = super::now_s();
    for (p, f) in [(json!({"thread":"t1","for":"1h"}), Box::new(|u: u64| u >= now + 3600 && u <= now + 3602) as Box<dyn Fn(u64) -> bool>), (json!({"thread":"t1","for":"1w"}), Box::new(move |u| u >= now + 604800 && u <= now + 604802)),
        (json!({"thread":"t1","for":"off"}), Box::new(|u| u == 0)), (json!({"thread":"t1","until":1234}), Box::new(|u| u == 1234)), (json!({"thread":"t1","for":"tomorrow"}), Box::new(move |u| u > now && u <= now + 2 * 86400))] {
        let n0 = h.store.len(); let (mut c, _) = h.connect(true); let out = h.request(&mut c, &json!({"id":7,"m":"thread.snooze","p":p})).await; assert_eq!(out[0]["ok"], true); assert_eq!(out[0]["result"]["thread"].is_string(), p["for"] == "tomorrow");
        let v: Value = serde_json::from_str(&h.store.get(n0).unwrap()).unwrap(); assert_eq!(v["$"], "ThreadSnoozed"); assert!(f(v["until"].as_u64().unwrap()), "{p} -> {v}");
    }
}
// Hub.reorder.k / Hub.reorder_projects
#[tokio::test] async fn reorder_gives_keys_to_the_shelf_in_its_new_order() {
    let h = hub("reorder").await;
    let (e, r) = ask(&h, "thread.reorder", json!({"section":"active","thread":"t2","ids":["t2","t1"]})).await; assert_eq!(r, ok());
    assert_eq!(e, vec![json!({"$":"ThreadOrdered","id":"t2","section":"active","key":"8","at":0})]);
    let (e, r) = ask(&h, "thread.reorder", json!({"section":"settled","thread":"t2","ids":[]})).await; assert!(e.is_empty()); assert_eq!(r["error"], "only pinned and active threads reorder");
    let (e, _) = ask(&h, "project.reorder", json!({"project":"p1","ids":["p1"]})).await; assert_eq!(e, vec![json!({"$":"ProjectOrdered","id":"p1","key":"8","at":0})]);
}
// Home.rename.project keeps the id, root and creation time; Roots.move
#[tokio::test] async fn project_rename_and_move_keep_what_the_hub_keeps() {
    let h = hub("proj").await;
    let (e, _) = ask(&h, "project.rename", json!({"project":"p1","title":" Renamed "})).await; assert_eq!(e, vec![json!({"$":"ProjectCreated","id":"p1","title":"Renamed","root":"/work/proj","at":0})]);
    let (e, _) = ask(&h, "project.rename", json!({"project":"zz","title":"x"})).await; assert!(e.is_empty());
    let (e, r) = ask(&h, "project.move", json!({"project":"p1","path":"/new/place/"})).await; assert_eq!(r, ok());
    assert_eq!(e, vec![json!({"$":"ProjectCreated","id":"p1","title":"Renamed","root":"/new/place","at":0}), json!({"$":"WorktreeSet","id":"t2","branch":"","worktree":"/new/place/.backplane/worktrees/w2","at":0}),
        json!({"$":"SettingSet","key":"projects.missing","value":"","at":0})]);
    let (e, r) = ask(&h, "project.move", json!({"project":"p1","path":"relative"})).await; assert!(e.is_empty()); assert_eq!(r["error"], "Type the folder's full path, starting with /");
    let (e, r) = ask(&h, "project.remove", json!({"project":"p1"})).await; assert_eq!(e, vec![json!({"$":"ProjectRemoved","id":"p1","at":0})]); assert_eq!(r, ok());
}
// Queue.op: the log, then the queue folds as State.queue does
#[tokio::test] async fn queue_edit_remove_move_log_their_events() {
    let h = hub("queue").await;
    let (e, _) = ask(&h, "queue.edit", json!({"thread":"t1","msg":"q1","text":"changed"})).await; assert_eq!(e, vec![json!({"$":"QueueEdited","thread":"t1","msg":"q1","text":"changed","at":0})]);
    let (e, _) = ask(&h, "queue.remove", json!({"thread":"t1","msg":"q2"})).await; assert_eq!(e, vec![json!({"$":"QueueRemoved","thread":"t1","msg":"q2","at":0})]);
    let (e, _) = ask(&h, "queue.move", json!({"thread":"t1","msg":"q1","to":"1"})).await; assert_eq!(e, vec![json!({"$":"QueueMoved","thread":"t1","msg":"q1","to":1,"at":0})]);
    let (e, _) = ask(&h, "queue.move", json!({"thread":"t1","msg":"nope","to":0})).await; assert_eq!(e.len(), 1);
    let mut f = super::fold::Fold::default(); f.sync(&h.store).unwrap(); assert_eq!(f.queue("t1"), [("q1".to_string(), "changed".to_string())], "an unknown message moves nothing");
}
// Hub.thread_parent: same project, alive, no cycle
#[tokio::test] async fn reparent_checks_the_ancestry() {
    let h = hub("tree").await;
    let (e, r) = ask(&h, "thread.reparent", json!({"thread":"t2","parent":"t1"})).await; assert_eq!(r, ok());
    assert_eq!(e, vec![json!({"$":"ThreadParentSet","id":"t2","parent":"t1","at":0}), json!({"$":"SettingSet","key":"thread.tree.t1","value":"1","at":0})]);
    // the ThreadParentSet is logged: a cycle is refused now
    let (e, r) = ask(&h, "thread.reparent", json!({"thread":"t1","parent":"t2"})).await; assert!(e.is_empty()); assert!(r["error"].as_str().unwrap().starts_with("Parent must be a live conversation"));
    let (e, r) = ask(&h, "thread.detach", json!({"thread":"t2"})).await; assert_eq!(r, ok()); assert_eq!(e, vec![json!({"$":"ThreadParentSet","id":"t2","parent":"","at":0})]);
    let (_, r) = ask(&h, "thread.reparent", json!({"thread":"t2","parent":"ghost"})).await; assert_eq!(r["ok"], false);
}
// Hub.thread_create.*
#[tokio::test] async fn thread_create_makes_a_thread_with_the_settings_defaults() {
    let h = hub("create").await;
    let (e, r) = ask(&h, "thread.create", json!({"project":"p1"})).await; assert_eq!(r["ok"], true); let th = r["thread"].as_str().unwrap().to_string(); assert!(th.starts_with('t'));
    assert_eq!(e[0], json!({"$":"ThreadCreated","id":th,"project":"p1","title":"New thread","env":"worktree","branch":"","worktree":"","provider":"claude","model":"","at":0}));
    let (e, _) = ask(&h, "thread.create", json!({"project":"p1","title":"Mine","env":"local","provider":"codex","model":"m"})).await;
    assert_eq!(e[0]["env"], "local"); assert_eq!(e[0]["title"], "Mine"); assert_eq!(e[0]["provider"], "codex");
    let (e, r) = ask(&h, "thread.create", json!({"project":"p1","parent":"t1"})).await; assert_eq!(r["ok"], true);
    assert!(e.iter().any(|v| v["$"] == "ThreadParentSet" && v["parent"] == "t1") && e.iter().any(|v| v["key"] == "thread.tree.t1"));
    let (e, r) = ask(&h, "thread.create", json!({"project":"p1","parent":"ghost"})).await; assert!(e.is_empty()); assert!(r["error"].as_str().unwrap().starts_with("Subthread parent"));
    let key = "0123456789abcdef0123456789abcdef";
    let (_, r) = ask(&h, "thread.create", json!({"project":"p1","creationKey":key})).await; assert_eq!(r["thread"], format!("improvement-{key}"));
    let (e, r) = ask(&h, "thread.create", json!({"project":"p1","creationKey":key})).await; assert!(e.is_empty()); assert_eq!(r["thread"], format!("improvement-{key}"));
    let (_, r) = ask(&h, "thread.create", json!({"project":"p1","creationKey":"short"})).await; assert_eq!(r["error"], "Choose an existing project and retry this request.");
}

async fn say(h: &Arc<Hub>, th: &str, role: &str, text: &str, n: u32) { h.commit(&json!({"$":"MessagePosted","thread":th,"msg":format!("m{n}"),"role":role,"text":text,"at":10})).await.unwrap(); }

// Hub.modes: what the request leaves out stays; naming a model holds the old work (Config.hold)
#[tokio::test] async fn modes_keep_what_is_left_out_and_a_model_choice_holds_waiting_work() {
    let h = hub("modes").await;
    let (e, r) = ask(&h, "thread.modes", json!({"thread":"t1","runtime":"auto","interaction":"plan","model":"opus"})).await; assert_eq!(r, ok());
    assert_eq!(e, vec![json!({"$":"ModesSet","id":"t1","runtime":"auto","interaction":"plan","model":"opus","at":0}), json!({"$":"SettingSet","key":"thread.config.hold.t1","value":"on","at":0}),
        json!({"$":"SettingSet","key":"account.failover.preference.t1","value":"","at":0}), json!({"$":"SettingSet","key":"fallback.intent.t1","value":"","at":0})]);
    let (e, _) = ask(&h, "thread.modes", json!({"thread":"t1","runtime":"nonsense"})).await;
    assert_eq!(e[0], json!({"$":"ModesSet","id":"t1","runtime":"full-access","interaction":"plan","model":"opus","at":0}));
}
#[tokio::test] async fn effort_is_kept_only_as_a_level_the_provider_offers() {
    let h = hub("effort").await;
    let (e, r) = ask(&h, "thread.effort", json!({"thread":"t1","effort":"max"})).await; assert_eq!(r, ok()); assert_eq!(e, vec![json!({"$":"SettingSet","key":"effort.t1","value":"max","at":0})]);
    let (e, _) = ask(&h, "thread.effort", json!({"thread":"t1","effort":"minimal"})).await; assert_eq!(e[0]["value"], "");
}
// Hub.switch.as + Switch.changes: between turns only; the new provider gets what it missed
#[tokio::test] async fn a_provider_switch_hands_over_what_the_new_one_missed() {
    let h = hub("switch").await;
    h.commit(&json!({"$":"TurnChanged","thread":"t2","state":"running","at":9})).await.unwrap();
    let (e, r) = ask(&h, "thread.provider", json!({"thread":"t2","provider":"codex"})).await; assert!(e.is_empty()); assert_eq!(r["error"], "a provider changes between turns: stop this one first");
    let (e, r) = ask(&h, "thread.provider", json!({"thread":"t1","provider":"claude"})).await; assert_eq!(r, ok()); assert_eq!(e.len(), 1, "the same provider only holds the old work: {e:?}"); // t1 waits (two queued messages): the hold
    say(&h, "t1", "user", "make a board", 1).await; say(&h, "t1", "assistant", "done", 2).await;
    let (e, r) = ask(&h, "thread.provider", json!({"thread":"t1","provider":"grok","model":"grok-4.5","effort":"high"})).await; assert_eq!(r, ok());
    assert_eq!(e[0]["$"], "ProviderSet"); assert_eq!(e[0]["provider"], "grok"); assert_eq!(e[0]["model"], "grok-4.5");
    assert_eq!(e[1], json!({"$":"SettingSet","key":"effort.t1","value":"high","at":0}));
    assert_eq!(e[2]["$"], "TurnQueued"); assert!(e[2]["msg"].as_str().unwrap().starts_with("handoff:switch:"));
    let text = e[2]["text"].as_str().unwrap();
    assert!(text.starts_with("[Backplane] This thread was worked on with claude until now; you (grok) take it over. Its conversation so far, oldest first:\n\nUser: make a board\n\nAssistant: done\n\n[End of the handed-over context."), "{text}");
    let last = e.iter().find(|v| v["$"] == "ActivityLogged").unwrap(); assert_eq!(last["summary"], "Now on grok; it gets what it missed with the next message");
    // State.switch.n: the provider left keeps how many entries it had seen, so going back hands over only what happened meanwhile
    let (e, r) = ask(&h, "thread.provider", json!({"thread":"t1","provider":"claude"})).await; assert_eq!(r, ok());
    let q = e.iter().find(|v| v["$"] == "TurnQueued").expect("the switch back hands over what claude missed");
    let text = q["text"].as_str().unwrap();
    assert!(text.starts_with("[Backplane] This thread went on with grok since you (claude) last took part. What happened meanwhile, oldest first:\n\nTool provider: Now on grok;"), "{text}");
    assert!(!text.contains("User: make a board"), "claude had seen the first two entries: {text}");
    assert_eq!(e.iter().find(|v| v["$"] == "ActivityLogged").unwrap()["summary"], "Now on claude; it gets what it missed with the next message");
    // nothing missed: no handoff and no suffix
    let (e, _) = ask(&h, "thread.provider", json!({"thread":"t1","provider":"grok"})).await; assert!(e.iter().any(|v| v["$"] == "TurnQueued"), "grok missed the switch notes");
}
// Give.step: the conversation queued ahead of the target's next message, the rest of the text as the note; a thread never gives to itself; naming nothing is refused in words
#[tokio::test] async fn give_hands_the_conversation_over_and_refuses_what_names_nothing() {
    let h = hub("give").await;
    say(&h, "t1", "user", "hello", 1).await;
    let (e, r) = ask(&h, "bots.give", json!({"thread":"t1","text":"/give >two see this"})).await; assert_eq!(r, ok());
    assert_eq!((e[0]["$"].as_str(), e[0]["thread"].as_str()), (Some("TurnQueued"), Some("t2"))); assert!(e[0]["msg"].as_str().unwrap().starts_with("handoff:give:t1:"));
    let hand = e[0]["text"].as_str().unwrap(); assert!(hand.contains("hands you the conversation of the thread \"One\" (t1). The user's note with it: >two see this") && hand.contains("User: hello"), "{hand}");
    assert_eq!((e[1]["thread"].as_str(), e[1]["summary"].as_str()), (Some("t2"), Some("\"One\" was handed over; it goes with the next message")));
    assert_eq!((e[2]["thread"].as_str(), e[2]["summary"].as_str()), (Some("t1"), Some("Given to \"Two\"")));
    { let mut fo = super::fold::Fold::default(); fo.sync(&h.store).unwrap(); assert!(!fo.waiting("t2"), "it waits for the next message; it starts no turn"); }
    // before the %proj give below, which makes a second thread titled "One"
    for text in ["/give", "/give nobody here", "/give >one"] { let (e, r) = ask(&h, "bots.give", json!({"thread":"t1","text":text})).await; assert_eq!((r["ok"].as_bool(), r["error"].as_str()), (Some(false), Some(super::give::NOTHING_NAMED)), "{text}"); assert!(e.is_empty(), "{text}"); }
    let (e, r) = ask(&h, "bots.give", json!({"thread":"t1","text":"/give %proj"})).await; assert_eq!(r, ok());
    let made = e.iter().find(|v| v["$"] == "ThreadCreated").unwrap(); assert_eq!((made["project"].as_str(), made["title"].as_str()), (Some("p1"), Some("One")));
    assert_eq!(e.iter().find(|v| v["$"] == "TurnQueued").unwrap()["thread"], made["id"]); assert_eq!(e.last().unwrap()["summary"], "Given to a new thread in proj");
    let (_, r) = ask(&h, "bots.give", json!({"thread":"ghost","text":"/give >two"})).await; assert_eq!(r["error"], "no such thread");
}
// Hub.fork / Hub.back: a fork records its source and carries the conversation as handed-over context
#[tokio::test] async fn a_fork_carries_the_conversation_and_comes_back_into_its_source() {
    let h = hub("fork").await;
    say(&h, "t1", "user", "hello", 1).await;
    let (e, r) = ask(&h, "thread.fork", json!({"thread":"t1"})).await; assert_eq!(r["ok"], true); let th = r["thread"].as_str().unwrap().to_string();
    assert_eq!(e[0], json!({"$":"ThreadCreated","id":th,"project":"p1","title":"Fork: One","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":0}));
    assert_eq!(e[1], json!({"$":"SettingSet","key":format!("account.thread.{th}.claude"),"value":"{\"id\":\"\",\"epoch\":0}","at":0}), "Accounts.created follows the thread");
    assert_eq!(e[2], json!({"$":"ModesSet","id":th,"runtime":"full-access","interaction":"default","model":"","at":0}));
    assert_eq!(e[3], json!({"$":"ThreadForked","id":th,"source":"t1","at":0}));
    assert_eq!((e[4]["key"].as_str().unwrap(), e[5]["key"].as_str().unwrap()), (format!("account.thread.{th}.claude").as_str(), format!("account.thread.{th}.codex").as_str()));
    assert_eq!(e[6]["msg"], "handoff:t1"); assert!(e[6]["text"].as_str().unwrap().contains("This thread is a fork of \"One\" (t1)") && e[6]["text"].as_str().unwrap().contains("User: hello"));
    { let mut fo = super::fold::Fold::default(); fo.sync(&h.store).unwrap(); assert!(!fo.waiting(&th), "handed-over context alone does not wait for a turn"); assert_eq!(fo.forks[&th], "t1"); }
    assert_eq!(e[7]["summary"], "Forked into \"Fork: One\""); assert_eq!(e[7]["thread"], "t1");
    assert!(e[8]["summary"].as_str().unwrap().starts_with("working in the project folder with \"One\""));
    let (e, r) = ask(&h, "thread.back", json!({"thread":th})).await; assert_eq!(r, ok());
    assert_eq!(e[0]["thread"], "t1"); assert!(e[0]["msg"].as_str().unwrap().starts_with(&format!("handoff:back:{th}:")));
    assert_eq!(e[1]["summary"], "The work of \"Fork: One\" goes with the next message"); assert_eq!(e[2]["thread"], th); assert_eq!(e[2]["summary"], "Brought back to \"One\"");
    let (_, r) = ask(&h, "thread.back", json!({"thread":"t1"})).await; assert_eq!(r["error"], "this thread is not a fork");
    let (_, r) = ask(&h, "thread.fork", json!({"thread":"ghost"})).await; assert_eq!(r["error"], "no such thread");
}
// Hand.text: the newest items that fit the budget, the first request and a count when older ones are left out
#[test] fn a_long_conversation_keeps_its_newest_items_and_says_how_many_were_left_out() {
    let es: Vec<super::Line> = (0..10).rev().map(|i| super::Line { msg: true, role: if i == 0 { "user".into() } else { "assistant".into() }, kind: String::new(), text: format!("{i} {}", "x".repeat(3900)) }).collect();
    let t = super::plan::hand_text("why", &es, "t9");
    assert!(t.contains("[6 earlier items left out; thread_read with threadId t9 has them all]"), "{}", &t[..200]);
    assert!(t.contains("User: 0 xxx") && t.contains("Assistant: 9 xxx") && !t.contains("Assistant: 5 xxx"));
}

// Accounts.created.one / get_str defaults: the default account of the provider, only for providers with accounts; a null field takes the setting
#[tokio::test] async fn a_new_thread_starts_on_the_default_account_and_a_null_field_takes_the_setting() {
    let h = hub("acct").await;
    h.commit(&json!({"$":"SettingSet","key":"account.default.codex","value":"{\"id\":\"a1\",\"epoch\":2}","at":9})).await.unwrap();
    h.commit(&json!({"$":"SettingSet","key":"agent.provider","value":"codex","at":9})).await.unwrap();
    let (e, r) = ask(&h, "thread.create", json!({"project":"p1","provider":null,"env":null,"model":null})).await; let th = r["thread"].as_str().unwrap();
    assert_eq!(e[0]["provider"], "codex"); assert_eq!(e[0]["env"], "worktree");
    assert_eq!(e[1], json!({"$":"SettingSet","key":format!("account.thread.{th}.codex"),"value":"{\"id\":\"a1\",\"epoch\":2}","at":0}));
    let (e, _) = ask(&h, "thread.create", json!({"project":"p1","provider":"grok"})).await; assert!(!e.iter().any(|v| v["key"].as_str().map_or(false, |k| k.starts_with("account.thread."))), "no binding for a provider without accounts");
    h.commit(&json!({"$":"SettingSet","key":"agent.provider","value":"","at":9})).await.unwrap();
    let (e, _) = ask(&h, "thread.create", json!({"project":"p1"})).await; assert_eq!(e[0]["provider"], "", "a stored empty setting is the value");
}
// BotJob.scratch: a scratch thread's folder exists when the reply arrives
#[tokio::test] async fn a_scratch_thread_gets_its_own_folder() {
    let h = hub("scratch").await;
    h.commit(&json!({"$":"ProjectCreated","id":"scratch","title":"Scratch","root":h.cfg.home.join("scratch").to_string_lossy(),"at":9})).await.unwrap();
    let (e, r) = ask(&h, "thread.create", json!({"project":"scratch"})).await; let th = r["thread"].as_str().unwrap();
    assert_eq!(e[0]["env"], "local"); assert_eq!(e[0]["worktree"].as_str().unwrap().replace('\\', "/"), format!("{}/scratch/{th}", h.cfg.home.display()).replace('\\', "/"));
    assert!(h.cfg.home.join("scratch").join(th).is_dir());
}

fn tmp(name: &str) -> std::path::PathBuf { let d = std::env::temp_dir().join(format!("bp-fs-{}-{name}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
// FsJob.list / FsJob.mkdir: the folder's subfolders, a new folder is refused when it exists
#[tokio::test] async fn the_pickers_folder_requests_list_and_make_folders() {
    let h = hub("fs").await; let d = tmp("pick"); std::fs::create_dir_all(d.join("b")).unwrap(); std::fs::create_dir_all(d.join("a")).unwrap(); std::fs::write(d.join("f"), "x").unwrap();
    let (_, r) = ask(&h, "fs.list", json!({"path":d.to_str().unwrap()})).await; assert_eq!(r["ok"], true); assert_eq!(r["result"]["entries"], json!(["a", "b"])); assert!(r["result"]["dir"].as_str().unwrap().ends_with("bp-fs-".to_string().as_str()) || r["result"]["dir"].is_string());
    let (_, r) = ask(&h, "fs.list", json!({"path":d.join("nope").to_str().unwrap()})).await; assert_eq!(r["ok"], false); assert!(r["result"]["error"].as_str().unwrap().starts_with("no such folder"));
    let (_, r) = ask(&h, "fs.mkdir", json!({"path":d.join("c").to_str().unwrap()})).await; assert_eq!(r["ok"], true); assert!(d.join("c").is_dir());
    let (_, r) = ask(&h, "fs.mkdir", json!({"path":d.join("c").to_str().unwrap()})).await; assert_eq!(r["ok"], false); assert!(r["result"]["error"].as_str().unwrap().starts_with("already exists"));
}
// Hub.project_add: a project once, with a first thread; its folder again is refused; removed, it comes back
#[tokio::test] async fn a_folder_becomes_a_project_once_and_a_removed_one_comes_back() {
    let h = hub("padd").await; let d = tmp("proj");
    let (e, r) = ask(&h, "project.create", json!({"path": format!("{}/", d.to_str().unwrap())})).await; // project.add itself is answered by hub.rs before the registry (the lead drops that line) assert_eq!(r["ok"], true); let th = r["thread"].as_str().unwrap().to_string();
    assert_eq!(e[0]["$"], "ProjectCreated"); assert_eq!(e[0]["root"], d.to_str().unwrap()); assert_eq!(e[0]["title"], d.file_name().unwrap().to_str().unwrap());
    assert_eq!(e[1]["$"], "ThreadCreated"); assert_eq!(e[1]["id"], r["thread"]); assert_eq!(e[1]["project"], e[0]["id"]);
    let pid = e[0]["id"].as_str().unwrap().to_string();
    let (e2, r) = ask(&h, "project.create", json!({"path": d.to_str().unwrap()})).await; assert!(e2.is_empty()); assert_eq!(r["ok"], false);
    let (_, r) = ask(&h, "project.create", json!({"path": d.to_str().unwrap()})).await; assert_eq!(r["error"], format!("already a project: {}", d.display()));
    ask(&h, "project.remove", json!({"project":pid})).await;
    let (e, r) = ask(&h, "project.create", json!({"path": d.to_str().unwrap()})).await; assert_eq!(r, ok()); assert_eq!(e[0]["id"], pid); assert_eq!(e.iter().filter(|v| v["$"] == "ThreadCreated").count(), 0);
    let (_, r) = ask(&h, "project.create", json!({"path": "  "})).await; assert_eq!(r["error"], "no folder given");
    let (_, r) = ask(&h, "project.new", json!({"path": d.join("fresh").to_str().unwrap()})).await; assert_eq!(r["ok"], true); assert!(d.join("fresh").is_dir());
}

fn git_repo(name: &str) -> std::path::PathBuf {
    let d = tmp(name); let p = d.to_str().unwrap().to_string();
    let g = |a: &[&str]| assert!(std::process::Command::new("git").args(a).current_dir(&p).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap().status.success());
    g(&["init", "-q"]); std::fs::write(d.join("f"), "x").unwrap(); g(&["add", "."]); g(&["commit", "-qm", "x"]); d
}
/// The agents module as a recorder: (call, thread, text). One for the process (the seam is global), so each test asks about its own threads.
struct Rec(std::sync::Mutex<Vec<(&'static str, String, String)>>, std::sync::Mutex<Option<Arc<Hub>>>);
impl super::seam::Seam for Rec {
    fn stop<'a>(&'a self, th: &'a str) -> crate::api::Fut<'a, ()> { Box::pin(async move { self.0.lock().unwrap().push(("stop", th.into(), String::new())) }) }
    // agents' interrupt contract: a running turn ends as interrupted (TurnChanged), logged by the hub that holds the thread
    fn interrupt<'a>(&'a self, th: &'a str) -> crate::api::Fut<'a, ()> { Box::pin(async move {
        self.0.lock().unwrap().push(("interrupt", th.into(), String::new()));
        let hub = interrupt_hubs().lock().unwrap().get(th).cloned(); if let Some(h) = hub { let _ = h.commit(&json!({"$":"TurnChanged","thread":th,"state":"interrupted","at":10})).await; } }) }
    fn start_queued<'a>(&'a self, th: &'a str, text: &'a str) -> crate::api::Fut<'a, ()> { Box::pin(async move { self.0.lock().unwrap().push(("start_queued", th.into(), text.into())) }) }
    fn steer<'a>(&'a self, th: &'a str, text: &'a str) -> crate::api::Fut<'a, ()> { Box::pin(async move { self.0.lock().unwrap().push(("steer", th.into(), text.into())) }) }
    fn start_one<'a>(&'a self, th: &'a str, text: &'a str) -> crate::api::Fut<'a, ()> { Box::pin(async move { self.0.lock().unwrap().push(("start_one", th.into(), text.into())) }) }
}
fn interrupt_hubs() -> &'static std::sync::Mutex<std::collections::HashMap<String, Arc<Hub>>> { static M: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, Arc<Hub>>>> = std::sync::OnceLock::new(); M.get_or_init(Default::default) }
fn rec() -> &'static Arc<Rec> { static R: std::sync::OnceLock<Arc<Rec>> = std::sync::OnceLock::new(); R.get_or_init(|| { let r = Arc::new(Rec(Default::default(), Default::default())); super::seam::inject(Some(r.clone())); r }) }
fn calls(th: &str) -> Vec<(&'static str, String)> { rec().0.lock().unwrap().iter().filter(|c| c.1 == th).map(|c| (c.0, c.2.clone())).collect() }
async fn settled(h: &Arc<Hub>, n0: usize, want: usize) -> Vec<Value> {
    for _ in 0..100 { if h.store.len() >= n0 + want { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    h.store.range(n0, h.store.len()).unwrap().iter().map(|l| serde_json::from_str(l).unwrap()).collect()
}
// Copy.git.wt: on a copy of a log the job runs on "/": the thread is told at once that it uses the project folder, and git writes nothing
#[tokio::test] async fn on_a_copy_the_worktree_job_skips_at_once_and_writes_no_git() {
    let h = hub("wtcopy").await; let repo = git_repo("wtcopy-repo");
    h.commit(&json!({"$":"ProjectCreated","id":"pr","title":"r","root":repo.to_str().unwrap(),"at":9})).await.unwrap();
    h.commit(&json!({"$":"ThreadCreated","id":"tc","project":"pr","title":"x","env":"worktree","branch":"","worktree":"","provider":"claude","model":"","at":9})).await.unwrap();
    let fold = Arc::new(tokio::sync::Mutex::new(super::fold::Fold::default()));
    let n0 = h.store.len();
    super::worktree::spawn_with(true, h.clone(), fold, "tc".into(), repo.to_str().unwrap().into(), "auto".into(), String::new(), "claude".into(), String::new(), Default::default());
    let ev = settled(&h, n0, 2).await;
    assert_eq!(ev[0]["$"], "ActivityLogged"); assert_eq!(ev[0]["summary"], "working in the project folder: not a git repository"); assert_eq!(ev[0]["kind"], "git"); assert_eq!(ev[0]["tone"], "info");
    assert_eq!(ev[1]["$"], "WorktreeSkipped"); assert_eq!(ev[1]["id"], "tc");
    assert!(!repo.join(".backplane").exists(), "no worktree was made"); assert!(!repo.join(".git/info/exclude").to_str().map_or(false, |p| std::fs::read_to_string(p).unwrap_or_default().contains(".backplane")));
}
// Hub.job.ready: the report lands, the waiting messages start through the agents seam, and a stale report (another provider now) does not start them
#[tokio::test] async fn a_made_worktree_is_recorded_and_the_waiting_messages_start() {
    let h = hub("wtreal").await; let repo = git_repo("wtreal-repo");
    h.commit(&json!({"$":"ProjectCreated","id":"pr","title":"r","root":repo.to_str().unwrap(),"at":9})).await.unwrap();
    for th in ["t11", "t22"] { h.commit(&json!({"$":"ThreadCreated","id":th,"project":"pr","title":"x","env":"worktree","branch":"","worktree":"","provider":"claude","model":"","at":9})).await.unwrap(); }
    let fold = Arc::new(tokio::sync::Mutex::new(super::fold::Fold::default()));
    for (m, x) in [("w1", "hello"), ("w2", "there")] { h.commit(&json!({"$":"TurnQueued","thread":"t11","msg":m,"text":x,"at":9})).await.unwrap(); }
    h.commit(&json!({"$":"TurnQueued","thread":"t22","msg":"w3","text":"queued too","at":9})).await.unwrap();
    let n0 = h.store.len();
    super::worktree::spawn_with(false, h.clone(), fold.clone(), "t11".into(), repo.to_str().unwrap().into(), "auto".into(), String::new(), "claude".into(), String::new(), Default::default());
    let ev = settled(&h, n0, 2).await;
    assert_eq!(ev[0]["$"], "WorktreeSet", "{ev:?}"); assert!(ev[0]["worktree"].as_str().unwrap().contains("/.backplane/worktrees/backplane-"), "{}", ev[0]); assert!(std::path::Path::new(ev[0]["worktree"].as_str().unwrap()).is_dir());
    assert_eq!(ev[1]["$"], "ActivityLogged"); assert!(ev[1]["summary"].as_str().unwrap().starts_with("worktree "));
    assert_eq!(calls("t11"), vec![("start_queued", "hello\n\nthere".to_string())]);
    // the thread changed provider while the job ran: the change is kept, nothing starts
    let n1 = h.store.len();
    h.commit(&json!({"$":"ProviderSet","id":"t22","provider":"codex","model":"","at":10})).await.unwrap();
    super::worktree::spawn_with(false, h.clone(), fold, "t22".into(), repo.to_str().unwrap().into(), "auto".into(), String::new(), "claude".into(), String::new(), Default::default());
    let ev = settled(&h, n1 + 1, 2).await; assert_eq!(ev[0]["$"], "WorktreeSet");
    assert_eq!(calls("t22"), vec![], "a stale report starts nothing");
}

async fn mk(h: &Arc<Hub>, th: &str) { h.commit(&json!({"$":"ThreadCreated","id":th,"project":"p1","title":"x","env":"local","branch":"","worktree":"","provider":"claude","model":"","at":9})).await.unwrap(); }
async fn queue(h: &Arc<Hub>, th: &str, msg: &str, text: &str) { h.commit(&json!({"$":"TurnQueued","thread":th,"msg":msg,"text":text,"at":9})).await.unwrap(); }
// Retire.archive / Switch.go: AgentStop every time (an idle process too), a running turn interrupted first
#[tokio::test] async fn archive_delete_and_a_provider_switch_stop_the_agent_process() {
    let h = hub("stops").await;
    mk(&h, "s1").await; ask(&h, "thread.archive", json!({"thread":"s1"})).await; assert_eq!(calls("s1"), vec![("stop", String::new())]);
    mk(&h, "s5").await; h.commit(&json!({"$":"TurnChanged","thread":"s5","state":"running","at":9})).await.unwrap();
    ask(&h, "thread.delete", json!({"thread":"s5"})).await; assert_eq!(calls("s5"), vec![("interrupt", String::new()), ("stop", String::new())]);
    mk(&h, "s6").await; ask(&h, "thread.provider", json!({"thread":"s6","provider":"codex"})).await; assert_eq!(calls("s6"), vec![("stop", String::new())]);
    // a message waiting: Config.waits keeps the process (no AgentStop), as Switch.go
    mk(&h, "s7").await; queue(&h, "s7", "w", "waiting").await; ask(&h, "thread.provider", json!({"thread":"s7","provider":"codex"})).await; assert_eq!(calls("s7"), vec![]);
}
// Queue.steer / Hub.resume: the queue events are ours, the run is the agents module's
#[tokio::test] async fn queue_steer_and_resume_hand_the_text_to_the_agents_module() {
    let h = hub("steer").await; mk(&h, "u1").await; queue(&h, "u1", "q1", "first").await; queue(&h, "u1", "q2", "second").await; mk(&h, "u2").await;
    let (e, r) = ask(&h, "queue.steer", json!({"thread":"u1","msg":"q2"})).await; assert_eq!(r, ok()); assert_eq!(e, vec![json!({"$":"QueueTakenOne","thread":"u1","msg":"q2","at":0})]); assert_eq!(calls("u1"), vec![("start_one", "second".to_string())]);
    { let mut fo = super::fold::Fold::default(); fo.sync(&h.store).unwrap(); assert_eq!(fo.queue("u1"), [("q1".to_string(), "first".to_string())], "steering one leaves the others waiting, in order"); }
    // running: into the running turn (steer)
    mk(&h, "u5").await; queue(&h, "u5", "r", "now").await; h.commit(&json!({"$":"TurnChanged","thread":"u5","state":"running","at":9})).await.unwrap();
    ask(&h, "queue.steer", json!({"thread":"u5","msg":"r"})).await; assert_eq!(calls("u5"), vec![("steer", "now".to_string())]);
    // idle: the one message runs as a turn of its own (start_one), the thread is not running
    mk(&h, "u3").await; queue(&h, "u3", "q1", "alone").await;
    ask(&h, "queue.steer", json!({"thread":"u3","msg":"q1"})).await; assert_eq!(calls("u3"), vec![("start_one", "alone".to_string())]);
    // Queue.join: an empty first text is skipped
    mk(&h, "u4").await; queue(&h, "u4", "e", "").await; queue(&h, "u4", "f", "x").await; queue(&h, "u4", "g", "y").await;
    let mut fo = super::fold::Fold::default(); fo.sync(&h.store).unwrap(); assert_eq!(fo.queue_text("u4"), "x\n\ny");
    let (e, r) = ask(&h, "queue.steer", json!({"thread":"u1","msg":"nope"})).await; assert!(e.is_empty()); assert_eq!(r["error"], "no such queued message");
    h.commit(&json!({"$":"SettingSet","key":"thread.config.hold.u1","value":"on","at":9})).await.unwrap();
    let (e, r) = ask(&h, "queue.resume", json!({"thread":"u1"})).await; assert_eq!(r, ok()); assert_eq!(e, vec![json!({"$":"SettingSet","key":"thread.config.hold.u1","value":"","at":0})]);
    assert_eq!(calls("u1"), vec![("start_one", "second".to_string()), ("start_queued", "first".to_string())]);
    let (e, _) = ask(&h, "queue.resume", json!({"thread":"u2"})).await; assert!(e.is_empty()); assert_eq!(calls("u2"), vec![], "nothing waits");
}

// switch_same_nothing / switch_hands_new_provider / switch_back_missed_nothing / switch_effort_offered / the session laws
#[tokio::test] async fn a_switch_changes_nothing_for_the_same_provider_and_hands_nothing_to_one_that_missed_nothing() {
    let h = hub("switch2").await;
    mk(&h, "w1").await; let (e, r) = ask(&h, "thread.provider", json!({"thread":"w1","provider":"claude"})).await; assert_eq!(r, ok()); assert!(e.is_empty(), "same provider, nothing waiting: {e:?}");
    // an effort the new provider does not offer is kept as its default
    let (e, _) = ask(&h, "thread.provider", json!({"thread":"w1","provider":"codex","effort":"max"})).await; assert_eq!(e[1], json!({"$":"SettingSet","key":"effort.w1","value":"","at":0}));
    // each provider keeps its own session: a new one starts fresh, coming back resumes the one it had (State.switch.n)
    mk(&h, "w2").await; h.commit(&json!({"$":"SessionSet","id":"w2","session":"S1","at":9})).await.unwrap();
    h.commit(&json!({"$":"ProviderSet","id":"w2","provider":"codex","model":"","at":9})).await.unwrap();
    let mut fo = super::fold::Fold::default(); fo.sync(&h.store).unwrap(); assert_eq!(fo.threads["w2"].session, "", "a provider new to the thread starts fresh");
    h.commit(&json!({"$":"SessionSet","id":"w2","session":"S2","at":9})).await.unwrap();
    h.commit(&json!({"$":"ProviderSet","id":"w2","provider":"claude","model":"","at":9})).await.unwrap(); fo.sync(&h.store).unwrap(); assert_eq!(fo.threads["w2"].session, "S1", "coming back resumes the session it had");
    h.commit(&json!({"$":"ProviderSet","id":"w2","provider":"codex","model":"","at":9})).await.unwrap(); fo.sync(&h.store).unwrap(); assert_eq!(fo.threads["w2"].session, "S2", "both keep theirs");
    // a provider new to a thread is handed it all; one coming back that missed nothing is handed nothing
    mk(&h, "w3").await; say(&h, "w3", "user", "hello", 1).await;
    let (e, _) = ask(&h, "thread.provider", json!({"thread":"w3","provider":"grok"})).await; assert_eq!(e.iter().filter(|v| v["$"] == "TurnQueued").count(), 1);
    mk(&h, "w4").await; say(&h, "w4", "user", "hello", 2).await;
    for p in ["codex", "claude"] { h.commit(&json!({"$":"ProviderSet","id":"w4","provider":p,"model":"","at":9})).await.unwrap(); }
    let (e, _) = ask(&h, "thread.provider", json!({"thread":"w4","provider":"codex"})).await; assert_eq!(e.iter().filter(|v| v["$"] == "TurnQueued").count(), 0, "codex missed nothing");
}
// project_drive_root_absolute / project_unc_root: Hub.root.norm
#[test] fn project_roots_are_kept_absolute() {
    for (a, b) in [("C:/", "C:/"), ("//server/share/", "//server/share"), ("/a/b//", "/a/b"), ("/", "/"), ("c://", "c:/")] { assert_eq!(super::plan::norm_root(a), b, "{a}") }
}

// archive_running_stops: archiving a running thread ends its turn (the agents module logs the interrupt) and its process goes; thread.delete also closes its page
struct Close(std::sync::Mutex<Vec<String>>);
impl crate::api::RpcHandler for Arc<Close> {
    fn methods(&self) -> &'static [&'static str] { &["browser.close"] }
    fn call<'a>(&'a self, _h: &'a Arc<Hub>, _w: &'a crate::api::Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> crate::api::Fut<'a, Vec<Value>> {
        Box::pin(async move { self.0.lock().unwrap().push(p["thread"].as_str().unwrap_or("").into()); vec![json!({"t":"reply","id":id,"ok":true})] })
    }
}
#[tokio::test] async fn archive_running_stops() {
    let h = hub("runstop").await; let closes = Arc::new(Close(Default::default())); h.register_rpc(Arc::new(closes.clone()));
    for th in ["r1", "r2"] { mk(&h, th).await; h.commit(&json!({"$":"TurnChanged","thread":th,"state":"running","at":9})).await.unwrap(); interrupt_hubs().lock().unwrap().insert(th.into(), h.clone()); }
    let running = |h: &Arc<Hub>, th: &str| { let mut f = super::fold::Fold::default(); f.sync(&h.store).unwrap(); f.threads[th].turn == "running" };
    assert!(running(&h, "r1"));
    ask(&h, "thread.archive", json!({"thread":"r1"})).await;
    assert!(!running(&h, "r1"), "archive ends a running turn"); assert_eq!(calls("r1"), vec![("interrupt", String::new()), ("stop", String::new())]);
    // an idle thread gets no turn change (archive_idle_no_turn)
    mk(&h, "r3").await; let (e, _) = ask(&h, "thread.archive", json!({"thread":"r3"})).await; assert!(!e.iter().any(|v| v["$"] == "TurnChanged"));
    ask(&h, "thread.delete", json!({"thread":"r2"})).await; assert!(!running(&h, "r2"));
    for _ in 0..40 { if !closes.0.lock().unwrap().is_empty() { break } tokio::time::sleep(std::time::Duration::from_millis(25)).await }
    assert_eq!(*closes.0.lock().unwrap(), vec!["r2".to_string()], "thread.delete closes the thread's browser page, after the reply");
}

// AG.Title.clean / Title.changes: the writer's title replaces the placeholder only while the thread still has it
#[test] fn a_writers_answer_becomes_a_clean_title() {
    for (a, b) in [("Title: Fix the board.", "Fix the board"), ("  # **Hello**  world  ", "Hello world"), ("'quoted'", "quoted"), ("one\ntwo", "one"), ("", ""), ("wait...", "wait..."), (&"x".repeat(70), &format!("{}...", "x".repeat(57)))] { assert_eq!(super::plan::title_clean(a), b, "{a:?}") }
}
/// The planners of the job reports (no client method reaches them: the jobs that report are not ported). Straight through the planner.
async fn plan_of(h: &Arc<Hub>, method: &str, p: Value) -> Vec<Value> {
    let mut f = super::fold::Fold::default(); f.sync(&h.store).unwrap();
    super::plan(&f, 0, &json!(7), method, &p, &super::Ctx::default()).unwrap().events.into_iter().map(|mut v| { v["at"] = json!(0); v }).collect()
}
#[tokio::test] async fn the_job_reports_titled_unrooted_and_merged_have_planners_and_no_client_method() {
    let h = hub("jobs").await;
    for m in ["thread.titled", "thread.merged", "thread.unrooted"] { let (e, r) = ask(&h, m, json!({"thread":"t1"})).await; assert!(e.is_empty()); assert_eq!(r["error"], "unknown method", "a client cannot reach {m}"); }
    h.commit(&json!({"$":"ThreadRenamed","id":"t2","title":"New thread","at":9})).await.unwrap();
    assert_eq!(plan_of(&h, "thread.titled", json!({"thread":"t2","placeholder":"New thread","title":"Title: Route the clock."})).await, vec![json!({"$":"ThreadRenamed","id":"t2","title":"Route the clock","at":0})]);
    h.commit(&json!({"$":"ThreadRenamed","id":"t2","title":"Mine","at":9})).await.unwrap();
    assert!(plan_of(&h, "thread.titled", json!({"thread":"t2","placeholder":"New thread","title":"Again"})).await.is_empty(), "a rename by the person wins");
    assert!(plan_of(&h, "thread.titled", json!({"thread":"t2","placeholder":"","title":"x"})).await.is_empty());
    h.commit(&json!({"$":"SettingSet","key":"wt.again.t2","value":"5","at":9})).await.unwrap();
    assert_eq!(plan_of(&h, "thread.unrooted", json!({"thread":"t2"})).await, vec![json!({"$":"WorktreeRemoved","id":"t2","at":0}), json!({"$":"SettingSet","key":"wt.again.t2","value":"","at":0})]);
    h.commit(&json!({"$":"WorktreeSet","id":"t2","branch":"backplane/x","worktree":"/w","at":9})).await.unwrap();
    let e = plan_of(&h, "thread.merged", json!({"thread":"t2","url":"https://github.com/o/r/pull/3"})).await;
    assert_eq!(e[0], json!({"$":"ThreadMerged","id":"t2","url":"https://github.com/o/r/pull/3","at":0})); assert_eq!(e[1]["summary"], "merged: https://github.com/o/r/pull/3"); assert_eq!(e[1]["kind"], "git");
    h.commit(&e[0]).await.unwrap();
    assert!(plan_of(&h, "thread.merged", json!({"thread":"t2","url":"u"})).await.is_empty(), "a recorded merge is not due again");
    assert!(plan_of(&h, "thread.merged", json!({"thread":"t1","url":"u"})).await.is_empty(), "no branch, no merge");
}
// Home.start / Home.request / ProjectHome.read
#[tokio::test] async fn a_home_send_makes_one_conversation_and_asks_the_agents_to_post_the_message() {
    let h = hub("homestart").await;
    let (e, r) = ask(&h, "project.start", json!({"project":"p1","msg":"hm_1-a","text":"Build a clock\nwith a display"})).await; assert_eq!(r["ok"], false, "no agents module in this test: the message could not be delivered, and the reply says so"); assert_eq!(r["error"], "no agent module is running"); let r = json!({"ok":true,"thread":"home-hm_1-a"}); assert_eq!(r["thread"], "home-hm_1-a");
    assert_eq!(e[0], json!({"$":"ThreadCreated","id":"home-hm_1-a","project":"p1","title":"Build a clock\nwith a display","env":"worktree","branch":"","worktree":"","provider":"claude","model":"","at":0}));
    let (e, r) = ask(&h, "project.start", json!({"project":"p1","msg":"hm_1-a","text":"again"})).await; assert!(e.is_empty()); assert_eq!(r["error"], "no agent module is running", "the retry is the same conversation, and posts the message that never arrived");
    for bad in [json!({"project":"p1","msg":"has space","text":"x"}), json!({"project":"p1","msg":"ok","text":"  "}), json!({"project":"nope","msg":"ok","text":"x"})] {
        let (e, r) = ask(&h, "project.start", bad).await; assert!(e.is_empty()); assert_eq!(r["error"], "Choose a project and enter a message before starting a thread.");
    }
    h.commit(&json!({"$":"ThreadDeleted","id":"home-hm_1-a","at":9})).await.unwrap();
    let (_, r) = ask(&h, "project.start", json!({"project":"p1","msg":"hm_1-a","text":"x"})).await; assert!(r["error"].as_str().unwrap().starts_with("This request belongs to a different or removed conversation"));
}
#[tokio::test] async fn the_project_home_reads_the_projects_folder() {
    let h = hub("home").await; let d = tmp("homefolder"); std::fs::write(d.join("README.md"), "hello").unwrap();
    h.commit(&json!({"$":"ProjectCreated","id":"ph","title":"h","root":d.to_str().unwrap(),"at":9})).await.unwrap();
    let (_, r) = ask(&h, "project.home", json!({"project":"ph"})).await; assert_eq!(r["ok"], true); assert_eq!(r["result"]["project_home"], "ph"); assert_eq!(r["result"]["artifacts"][0]["kind"], "readme");
    let (_, r) = ask(&h, "project.home", json!({"project":"ghost"})).await; assert_eq!(r["error"], "Choose an existing project.");
    // an error inside the result is still an answer (GitJob.reply(..., True, r)): a stale revision reaches the "your edit is retained" text
    let (_, r) = ask(&h, "project.home", json!({"project":"ph","op":"intent","revision":"stale","editor":0,"removeBytes":0,"text":"x"})).await; assert_eq!(r["ok"], true); assert_eq!(r["result"]["error"], "Goal and notes changed. Reload before saving; your edit is retained.");
}

// archive_running_stops, through the real agents module (a stand-in agent that never answers): archiving a running thread ends its turn and its process
#[tokio::test] async fn archive_running_stops_through_the_real_agents_module() {
    use crate::plat::fs::PermissionsExt;
    let base = std::env::temp_dir().join(format!("bp-cmds-real-{}", std::process::id())); let _ = std::fs::remove_dir_all(&base);
    let (home, bin, work) = (base.join("home"), base.join("bin"), base.join("work")); for d in [&home, &bin, &work] { std::fs::create_dir_all(d).unwrap() }
    std::fs::write(bin.join("claude"), "#!/bin/sh\nsleep 600\n").unwrap(); std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(home.join("token"), "tok").unwrap();
    std::fs::write(home.join("events.jsonl"), format!("{{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"p\",\"root\":{},\"at\":1}}\n{{\"$\":\"ThreadCreated\",\"id\":\"ta1\",\"project\":\"p1\",\"title\":\"T\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":2}}\n", serde_json::to_string(work.to_str().unwrap()).unwrap())).unwrap();
    let h = Hub::start(Cfg { home, version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let ag = crate::agents::Agents::start_with(h.clone(), crate::agents::proc::Cli::standin(&bin)).await.unwrap();
    super::start_with(h.clone(), Some(Arc::new(ag.clone()) as Arc<dyn super::seam::Seam>));
    let (mut c, _) = h.connect(true);
    let out = h.request(&mut c, &json!({"id":1,"m":"turn.start","p":{"thread":"ta1","text":"go"}})).await; assert_eq!(out[0]["ok"], true);
    let state = |h: &Arc<Hub>| { let mut f = super::fold::Fold::default(); f.sync(&h.store).unwrap(); f.threads["ta1"].turn.clone() };
    for _ in 0..100 { if state(&h) == "running" { break } tokio::time::sleep(std::time::Duration::from_millis(50)).await }
    assert_eq!(state(&h), "running", "the stand-in holds the turn");
    let out = h.request(&mut c, &json!({"id":2,"m":"thread.archive","p":{"thread":"ta1"}})).await; assert_eq!(out[0]["ok"], true);
    assert_ne!(state(&h), "running", "archive ended the running turn"); assert_eq!(state(&h), "interrupted");
    let mut f = super::fold::Fold::default(); f.sync(&h.store).unwrap(); assert!(f.threads["ta1"].archived);
}

// Home.start is atomic for the client: turn.start failing once leaves the thread, the retry delivers the message, a third try posts nothing
struct Starter(std::sync::Mutex<u32>, std::sync::Mutex<Vec<String>>);
impl crate::api::RpcHandler for Arc<Starter> {
    fn methods(&self) -> &'static [&'static str] { &["turn.start"] }
    fn call<'a>(&'a self, h: &'a Arc<Hub>, _w: &'a crate::api::Caller, id: &'a Value, _m: &'a str, p: &'a Value) -> crate::api::Fut<'a, Vec<Value>> {
        Box::pin(async move {
            let n = { let mut c = self.0.lock().unwrap(); *c += 1; *c };
            if n == 1 { return vec![json!({"t":"reply","id":id,"ok":false,"error":"the agent is busy"})] }
            self.1.lock().unwrap().push(p["text"].as_str().unwrap_or("").into());
            h.commit(&json!({"$":"MessagePosted","thread":p["thread"],"msg":p["msg"],"role":"user","text":p["text"],"at":9})).await.unwrap();
            vec![json!({"t":"reply","id":id,"ok":true})]
        })
    }
}
#[tokio::test] async fn project_start_survives_a_failed_first_delivery() {
    let h = hub("homeatomic").await; let st = Arc::new(Starter(Default::default(), Default::default())); h.register_rpc(Arc::new(st.clone()));
    let p = json!({"project":"p1","msg":"hm9","text":"Make it"});
    let (e, r) = ask(&h, "project.start", p.clone()).await; assert_eq!(e[0]["$"], "ThreadCreated"); assert_eq!(r["ok"], false); assert_eq!(r["error"], "the agent is busy");
    let (e, r) = ask(&h, "project.start", p.clone()).await; assert!(e.iter().all(|v| v["$"] != "ThreadCreated"), "the thread is not made twice"); assert_eq!(r["ok"], true); assert_eq!(r["thread"], "home-hm9");
    assert_eq!(*st.1.lock().unwrap(), vec!["Make it".to_string()]);
    let (_, r) = ask(&h, "project.start", p).await; assert_eq!(r["ok"], true); assert_eq!(st.1.lock().unwrap().len(), 1, "a message that arrived is not posted again");
}

// thread.create with a creation key may name a base (a local branch that must exist) and a branch; no key: the params are ignored; a missing base refuses
#[tokio::test] async fn an_improvements_send_branches_from_the_named_local_base() {
    let h = hub("base").await; let repo = git_repo("base-repo");
    let g = |a: &[&str]| assert!(std::process::Command::new("git").args(a).current_dir(&repo).output().unwrap().status.success());
    g(&["branch", "gpui"]);
    h.commit(&json!({"$":"ProjectCreated","id":"pb","title":"r","root":repo.to_str().unwrap(),"at":9})).await.unwrap();
    let key = "0123456789abcdef0123456789abcdef";
    let (e, r) = ask(&h, "thread.create", json!({"project":"pb","creationKey":key,"base":"nope","branch":"x"})).await; assert!(e.is_empty(), "no thread for a missing base"); assert_eq!(r["error"], "base branch \"nope\" does not exist in this project");
    let (e, r) = ask(&h, "thread.create", json!({"project":"pb","creationKey":key,"base":"-x"})).await; assert!(e.is_empty()); assert!(r["error"].as_str().unwrap().contains("not a local branch name"));
    let (e, r) = ask(&h, "thread.create", json!({"project":"pb","creationKey":key,"base":"gpui","branch":"imp-ab12cd34"})).await; assert_eq!(r["ok"], true, "{r}"); assert_eq!(e[0]["$"], "ThreadCreated");
    // the job: branches from gpui, named imp-ab12cd34, under .backplane/worktrees
    let fold = Arc::new(tokio::sync::Mutex::new(super::fold::Fold::default()));
    let n0 = h.store.len();
    super::worktree::spawn_with(false, h.clone(), fold.clone(), "t77".into(), repo.to_str().unwrap().into(), "auto".into(), String::new(), "claude".into(), String::new(), super::worktree::Opts { base: "gpui".into(), branch: "imp-ab12cd34".into(), ..Default::default() });
    h.commit(&json!({"$":"ThreadCreated","id":"t77","project":"pb","title":"x","env":"worktree","branch":"","worktree":"","provider":"claude","model":"","at":9})).await.unwrap();
    let ev = settled(&h, n0 + 1, 2).await;
    let set = ev.iter().find(|v| v["$"] == "WorktreeSet").unwrap_or_else(|| panic!("{ev:?}"));
    assert_eq!(set["branch"], "imp-ab12cd34"); assert!(set["worktree"].as_str().unwrap().ends_with("/.backplane/worktrees/imp-ab12cd34"));
    // a base that vanished after the check: an error note, and NO WorktreeSkipped (the thread must not fall back to the project folder)
    let n1 = h.store.len();
    super::worktree::spawn_with(false, h.clone(), fold, "t78".into(), repo.to_str().unwrap().into(), "auto".into(), String::new(), "claude".into(), String::new(), super::worktree::Opts { base: "gone".into(), branch: "b2".into(), ..Default::default() });
    let ev = settled(&h, n1, 1).await; tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let ev2: Vec<Value> = h.store.range(n1, h.store.len()).unwrap().iter().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert!(ev.iter().chain(ev2.iter()).any(|v| v["$"] == "ActivityLogged" && v["tone"] == "error" && v["summary"].as_str().unwrap().contains("could not make the worktree from gone")));
    assert!(!ev2.iter().any(|v| v["$"] == "WorktreeSkipped"));
    // no creation key: base and branch are ignored (today's thread.create), and nothing is checked
    let (e, r) = ask(&h, "thread.create", json!({"project":"pb","base":"nope","branch":"x"})).await; assert_eq!(r["ok"], true); assert_eq!(e[0]["$"], "ThreadCreated");
}

// a project's default base (worktree.base.<project>): a thread made with no base branches from that local branch, not from origin's default or HEAD; a missing one changes nothing
#[tokio::test] async fn a_project_default_base_is_where_new_thread_worktrees_branch_from() {
    let h = hub("prefer").await; let repo = git_repo("prefer-repo");
    let g = |a: &[&str]| assert!(std::process::Command::new("git").args(a).current_dir(&repo).output().unwrap().status.success());
    g(&["branch", "gpui"]); g(&["checkout", "-q", "-b", "elsewhere"]); // HEAD is on another branch than gpui
    let tip = String::from_utf8(std::process::Command::new("git").args(["rev-parse", "gpui"]).current_dir(&repo).output().unwrap().stdout).unwrap().trim().to_string();
    h.commit(&json!({"$":"ProjectCreated","id":"pp","title":"r","root":repo.to_str().unwrap(),"at":9})).await.unwrap();
    h.commit(&json!({"$":"SettingSet","key":"worktree.base.pp","value":"gpui","at":9})).await.unwrap();
    let fold = Arc::new(tokio::sync::Mutex::new(super::fold::Fold::default()));
    let n0 = h.store.len();
    super::worktree::spawn_with(false, h.clone(), fold.clone(), "t90".into(), repo.to_str().unwrap().into(), "auto".into(), String::new(), "claude".into(), String::new(), super::worktree::Opts { prefer: "gpui".into(), ..Default::default() });
    h.commit(&json!({"$":"ThreadCreated","id":"t90","project":"pp","title":"x","env":"worktree","branch":"","worktree":"","provider":"claude","model":"","at":9})).await.unwrap();
    let ev = settled(&h, n0 + 1, 2).await;
    let set = ev.iter().find(|v| v["$"] == "WorktreeSet").unwrap_or_else(|| panic!("{ev:?}"));
    let wt = set["worktree"].as_str().unwrap();
    let head = String::from_utf8(std::process::Command::new("git").args(["merge-base", "HEAD", &tip]).current_dir(wt).output().unwrap().stdout).unwrap().trim().to_string();
    assert_eq!(head, tip, "the worktree grew from gpui");
    // a default that is not a local branch is ignored: the worktree is made as before
    let n1 = h.store.len();
    super::worktree::spawn_with(false, h.clone(), fold, "t91".into(), repo.to_str().unwrap().into(), "auto".into(), String::new(), "claude".into(), String::new(), super::worktree::Opts { prefer: "no-such".into(), ..Default::default() });
    h.commit(&json!({"$":"ThreadCreated","id":"t91","project":"pp","title":"y","env":"worktree","branch":"","worktree":"","provider":"claude","model":"","at":9})).await.unwrap();
    let ev = settled(&h, n1 + 1, 2).await;
    assert!(ev.iter().any(|v| v["$"] == "WorktreeSet"), "{ev:?}");
}
async fn ask_on(h: &Arc<Hub>, c: &mut crate::hub::Conn, m: &str, p: Value) -> (Vec<Value>, Value) {
    let n0 = h.store.len(); let out = h.request(c, &json!({"id":7,"m":m,"p":p})).await;
    let ev = h.store.range(n0, h.store.len()).unwrap().iter().map(|l| { let mut v: Value = serde_json::from_str(l).unwrap(); if v.get("at").is_some() { v["at"] = json!(0) } v }).collect();
    (ev, out[0].clone())
}
// Hub.accounts.select: between turns, on a fresh menu, a connected profile of the thread's provider; the sessions are kept per account and what the new one missed is handed over
#[tokio::test] async fn moving_all_threads_to_an_account_selects_it_for_each_idle_thread_of_that_agent_and_names_the_running_ones() {
    use crate::settings::accounts as ac;
    let h = hub("acct-move-all").await;
    let inv = json!({"version":1,"revision":1,"profiles":[{"id":"w","provider":"claude","state":"connected","epoch":3,"label":"Work"},{"id":"d","provider":"claude","state":"disconnected","epoch":1,"label":"Old"}],"error":""}).to_string();
    h.commit(&json!({"$":"SettingSet","key":"accounts.inventory","value":inv,"at":1})).await.unwrap();
    h.commit(&json!({"$":"ThreadCreated","id":"t3","project":"p1","title":"Codex","env":"local","branch":"","worktree":"","provider":"codex","model":"","at":4})).await.unwrap();
    h.commit(&json!({"$":"SessionSet","id":"t1","session":"sess-default","at":5})).await.unwrap();
    say(&h, "t1", "user", "make a board", 6).await;
    h.commit(&json!({"$":"TurnChanged","thread":"t2","state":"running","at":7})).await.unwrap();
    let all = |profile: &str, epoch: u64| json!({"provider":"claude","profile":profile,"epoch":epoch,"default":true});
    let (_, r) = ask(&h, "accounts.move_all", all("w", 3)).await; assert_eq!(r["error"], ac::GATE, "only a client granted account access");
    let (mut c, _) = h.connect(true); crate::settings::accounts_hub::grant_client(&c.gen, c.client);
    let (e, r) = ask_on(&h, &mut c, "accounts.move_all", all("d", 1)).await; assert_eq!(r["ok"], false, "a disconnected account takes no thread: {r}"); assert!(e.is_empty(), "and nothing is written: {e:?}");
    let (e, r) = ask_on(&h, &mut c, "accounts.move_all", all("w", 3)).await; assert_eq!(r["ok"], true, "{r}");
    assert_eq!(r["result"], json!({"moved":1,"already":0,"running":["t2"],"failed":[]}), "t1 moves, t2 is running so it waits, t3 is Codex");
    let set = |k: &str| e.iter().filter(|v| v["$"] == "SettingSet" && v["key"] == k).last().map(|v| v["value"].clone());
    assert_eq!(set("account.thread.t1.claude"), Some(json!(ac::binding_show("w", 3))));
    assert_eq!(set(&ac::session_key("t1", "claude", "", 0)), Some(json!("sess-default")), "each thread keeps its old account's session, as one select does");
    assert!(set(&ac::chosen_key("t1", "claude")).map_or(false, |v| v.as_str().unwrap_or("").contains("person")), "the person chose it: {e:?}");
    assert_eq!(set("account.thread.t2.claude"), None, "a running thread is not switched under its turn");
    assert!(e.iter().all(|v| v["key"] != "account.thread.t3.codex" && v["key"] != "account.thread.t3.claude"), "another agent's thread is left alone");
    assert_eq!(set("account.default.claude"), Some(json!(ac::binding_show("w", 3))), "and new threads start on it");
    // the running one ends: moving again takes it and leaves the one already there
    h.commit(&json!({"$":"TurnChanged","thread":"t2","state":"completed","at":8})).await.unwrap();
    let (_, r) = ask_on(&h, &mut c, "accounts.move_all", json!({"provider":"claude","profile":"w","epoch":3})).await;
    assert_eq!(r["result"], json!({"moved":1,"already":1,"running":[],"failed":[]}), "{r}");
    assert_eq!(h.setting("account.thread.t2.claude").await.unwrap_or_default(), ac::binding_show("w", 3));
    crate::settings::accounts_hub::revoke_client(c.client);
}
#[tokio::test] async fn selecting_an_account_keeps_each_accounts_session_and_hands_over_what_it_missed() {
    use crate::settings::accounts as ac;
    let h = hub("acct-select").await;
    let inv = json!({"version":1,"revision":1,"profiles":[{"id":"w","provider":"claude","state":"connected","epoch":3,"label":"Work"},{"id":"d","provider":"claude","state":"disconnected","epoch":1,"label":"Old"}],"error":""}).to_string();
    h.commit(&json!({"$":"SettingSet","key":"accounts.inventory","value":inv,"at":1})).await.unwrap();
    h.commit(&json!({"$":"SettingSet","key":"account.thread.t1.claude","value":ac::binding_show("", 0),"at":1})).await.unwrap();
    h.commit(&json!({"$":"SessionSet","id":"t1","session":"sess-default","at":2})).await.unwrap();
    say(&h, "t1", "user", "make a board", 1).await; say(&h, "t1", "assistant", "done", 2).await;
    let sel = |profile: &str, epoch: u64, expected: String| json!({"thread":"t1","profile":profile,"epoch":epoch,"expected":expected});
    let (e, r) = ask(&h, "accounts.select", sel("w", 3, ac::binding_show("", 0))).await;
    assert_eq!(r["error"], ac::GATE, "only a client granted account access (e {e:?})");
    let (mut c, _) = h.connect(true); crate::settings::accounts_hub::grant_client(&c.gen, c.client);
    let (e, r) = ask_on(&h, &mut c, "accounts.select", sel("w", 3, ac::binding_show("w", 3))).await; assert_eq!(r["ok"], false, "a menu from before another selection: {e:?}");
    let (_, r) = ask_on(&h, &mut c, "accounts.select", sel("d", 1, ac::binding_show("", 0))).await; assert_eq!(r["error"], ac::unavailable(&ac::profile(&inv, "d"), 1), "a disconnected profile");
    let (_, r) = ask_on(&h, &mut c, "accounts.select", sel("w", 2, ac::binding_show("", 0))).await; assert_eq!(r["error"], ac::unavailable(&ac::profile(&inv, "w"), 2), "the sign-in changed");
    let (e, r) = ask_on(&h, &mut c, "accounts.select", sel("w", 3, ac::binding_show("", 0))).await; assert_eq!(r, ok());
    let set = |k: &str| e.iter().find(|v| v["$"] == "SettingSet" && v["key"] == k).map(|v| v["value"].clone());
    assert_eq!(set("account.thread.t1.claude"), Some(json!(ac::binding_show("w", 3))));
    assert_eq!(set(&ac::session_key("t1", "claude", "", 0)), Some(json!("sess-default")), "the old account's session is kept under its own key");
    assert_eq!(e.iter().find(|v| v["$"] == "SessionSet").unwrap()["session"], "", "the new account has no session yet: it starts fresh");
    assert!(e.iter().any(|v| v["$"] == "TurnQueued" && v["text"].as_str().unwrap().contains("Selected another account") && v["text"].as_str().unwrap().contains("make a board")), "it is handed what it missed: {e:?}");
    let back = json!({"thread":"hub|t1","profile":"","epoch":0,"expected":ac::binding_show("w", 3)});
    let (_, r) = ask_on(&h, &mut c, "accounts.select", back).await; assert_eq!(r, ok(), "the picture names a thread hub|id: it is the same thread");
    let (_, r) = ask_on(&h, &mut c, "accounts.select", sel("w", 3, ac::binding_show("", 0))).await; assert_eq!(r, ok());
    h.commit(&json!({"$":"TurnChanged","thread":"t1","state":"running","at":9})).await.unwrap();
    let (_, r) = ask_on(&h, &mut c, "accounts.select", sel("", 0, ac::binding_show("w", 3))).await; assert_eq!(r["error"], "Finish or stop the current turn before selecting another account.");
    crate::settings::accounts_hub::revoke_client(c.client);
}

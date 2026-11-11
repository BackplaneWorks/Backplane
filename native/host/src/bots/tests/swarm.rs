//! Swarms on a real hub (temp home) with a stand-in for the agents: launches, rides and locks are recorded, no agent runs (docs/SWARM.md).
use super::*;
use crate::bots::swarm::ThreadInfo;
use std::collections::HashMap;

#[derive(Default)]
struct Peers { launched: StdMutex<Vec<Value>>, rides: StdMutex<Vec<(String, String, String)>>, notes: StdMutex<Vec<(String, String, String)>>, got: StdMutex<Vec<(String, String)>>,
    turn: StdMutex<HashMap<String, String>>, locks: StdMutex<HashMap<String, String>>,
    /// the bots, so `unclaim` takes their world lock as the real one does (a freed lock is granted to its waiter, and that delivery takes it)
    bots: std::sync::OnceLock<std::sync::Weak<Bots>> }
impl Inbox for Arc<Peers> {
    fn deliver<'a>(&'a self, th: &'a str, _msg: &'a str, text: &'a str, _o: Option<(String, String, String)>) -> Fut<'a, Result<(), String>> { Box::pin(async move { self.got.lock().unwrap().push((th.into(), text.into())); Ok(()) }) }
    fn stop<'a>(&'a self, _th: &'a str) -> Fut<'a, ()> { Box::pin(async {}) }
    fn note<'a>(&'a self, th: &'a str, id: &'a str, text: &'a str) -> Fut<'a, Result<(), String>> { Box::pin(async move { self.notes.lock().unwrap().push((th.into(), id.into(), text.into())); Ok(()) }) }
    fn ride<'a>(&'a self, th: &'a str, id: &'a str, text: &'a str) -> Fut<'a, Result<(), String>> { Box::pin(async move { self.rides.lock().unwrap().push((th.into(), id.into(), text.into())); Ok(()) }) }
    fn info<'a>(&'a self, th: &'a str) -> Fut<'a, Option<ThreadInfo>> { Box::pin(async move {
        let known = th == "t-boss" || self.launched.lock().unwrap().iter().any(|a| a["_id"] == th);
        if !known { return None }
        let provider = self.launched.lock().unwrap().iter().find(|a| a["_id"] == th).map(|a| a["provider"].as_str().unwrap_or("").to_string()).unwrap_or("claude".into());
        Some(ThreadInfo { provider, turn: self.turn.lock().unwrap().get(th).cloned().unwrap_or_else(|| "completed".into()), project: "p1".into(), root: "/w".into(), ..Default::default() })
    }) }
    fn launch<'a>(&'a self, _by: &'a str, a: Value) -> Fut<'a, Result<String, String>> { Box::pin(async move { let id = a["_id"].as_str().unwrap().to_string(); self.launched.lock().unwrap().push(a); Ok(id) }) }
    fn claim<'a>(&'a self, th: &'a str, files: Vec<String>, _n: String) -> Fut<'a, Vec<(String, String, bool, String)>> { Box::pin(async move {
        let mut l = self.locks.lock().unwrap();
        files.into_iter().map(|f| { let n = crate::bots::swarm::lock_for("/w", &f); let h = l.entry(n.clone()).or_insert_with(|| th.to_string()).clone(); (n, f, h == th, h) }).collect()
    }) }
    fn unclaim<'a>(&'a self, th: &'a str, locks: Vec<String>) -> Fut<'a, ()> { Box::pin(async move {
        if let Some(b) = self.bots.get().and_then(std::sync::Weak::upgrade) { let _w = b.world().await; }
        let mut l = self.locks.lock().unwrap(); for n in locks { if l.get(&n).map_or(false, |h| h == th) { l.remove(&n); } } }) }
}

async fn hub_of(name: &str) -> (crate::hub::TestHub, Arc<Bots>, Arc<Peers>) {
    let home = crate::plat::tmp::dir(&format!("swarm-{name}"));
    std::fs::write(home.join("events.jsonl"), "{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"toy\",\"root\":\"/w\",\"at\":1}\n").unwrap();
    let hub = Hub::start(Cfg { home: home.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    let peers = Arc::new(Peers::default());
    let bots = Bots::start_with(hub.clone(), Arc::new(peers.clone())).await;
    let _ = peers.bots.set(Arc::downgrade(&bots));
    (crate::hub::TestHub { hub, _tmp: home }, bots, peers)
}
fn rides_to(p: &Peers, th: &str) -> Vec<String> { p.rides.lock().unwrap().iter().filter(|r| r.0 == th).map(|r| r.2.clone()).collect() }
async fn post(b: &Arc<Bots>, th: &str, a: Value) -> (bool, Value) { b.tool_call(th, "room_post", &a).await }

#[tokio::test] async fn a_swarm_of_three_peers_wakes_batches_claims_guards_and_ends_with_a_synthesis() {
    let (hub, bots, peers) = hub_of("three").await;
    // started by a thread (the uberbot's would be the same): three peers on three providers, no parent, no reports
    let (ok, r) = bots.tool_call("t-boss", "swarm_create", &json!({"topic":"debounce","goal":"pick a debounce for the encoder","seed":"The encoder bounces for 2 ms.",
        "members":[{"provider":"claude","model":"claude-opus-5-5"},{"provider":"antigravity","model":"gemini-carbon-4"},{"provider":"codex"}]})).await;
    assert!(ok, "{r}");
    let room = r["room"].as_str().unwrap().to_string();
    let l = peers.launched.lock().unwrap().clone(); assert_eq!(l.len(), 3);
    for a in &l { assert_eq!((a["subthread"].clone(), a["report"].clone(), a["project"].clone()), (json!(false), json!(false), json!("p1")), "peers: top level, reporting to nobody"); }
    assert!(l[0]["prompt"].as_str().unwrap().contains("one of 3 peers") && l[0]["prompt"].as_str().unwrap().contains("There is no boss") && l[0]["prompt"].as_str().unwrap().contains("The encoder bounces"), "{}", l[0]["prompt"]);
    let (opus, gem, codex) = (l[0]["_id"].as_str().unwrap().to_string(), l[1]["_id"].as_str().unwrap().to_string(), l[2]["_id"].as_str().unwrap().to_string());
    { let w = bots.world().await; let rm = w.room(&room).unwrap(); assert!(rm.swarm() && !rm.ephemeral && rm.by == "t-boss" && rm.threads() == vec![opus.clone(), gem.clone(), codex.clone()]); }
    // a member thread lists the room tools; a thread in no room does not
    { use crate::agents::mcp::ToolSet; let set = tools::BotToolSet(bots.clone()); let has = |t: &str| set.list(t).iter().any(|d| d["name"] == "room_post");
      assert!(has(&opus) && !has("t-boss")); }

    // a post wakes every other member at once the first time
    let (ok, p1) = post(&bots, &opus, json!({"room":room,"text":"Idea: a 5 ms integrator.","tags":["idea"]})).await; assert!(ok, "{p1}");
    assert_eq!((rides_to(&peers, &gem).len(), rides_to(&peers, &codex).len(), rides_to(&peers, &opus).len()), (1, 1, 0), "the others hear it, not the poster");
    assert!(rides_to(&peers, &gem)[0].contains("opus [idea]") && rides_to(&peers, &gem)[0].contains("5 ms integrator"));
    // a second post inside the batch window waits; an @mention gets through at once, with what waited
    post(&bots, &opus, json!({"room":room,"text":"Or a state machine with hysteresis."})).await;
    assert_eq!(rides_to(&peers, &gem).len(), 1, "batched: no second wake within the window");
    let (_, m) = post(&bots, &opus, json!({"room":room,"text":"@gemini what does your encoder datasheet say?","tags":["question"]})).await;
    assert_eq!(m["mentioned"], json!([format!("thread:{gem}")]));
    let g = rides_to(&peers, &gem); assert_eq!(g.len(), 2, "a mention is urgent"); assert!(g[1].contains("2 new posts") && g[1].contains("hysteresis") && g[1].contains("@gemini"), "{}", g[1]);
    assert_eq!(rides_to(&peers, &codex).len(), 1, "codex was not mentioned: still batched");

    // claims: one lock per file; a peer's claim of the same file waits; done frees it
    let (_, c1) = post(&bots, &gem, json!({"room":room,"text":"I'll write the integrator.","tags":["claim"],"files":["src/debounce.rs"]})).await;
    assert_eq!(c1["claim"]["state"], "held", "{c1}");
    let (_, c2) = post(&bots, &codex, json!({"room":room,"text":"I'll take debounce.rs too.","tags":["claim"],"files":["src/debounce.rs"]})).await;
    assert_eq!(c2["claim"]["state"], "waiting"); assert!(c2["note"].as_str().unwrap().contains("don't edit"), "{c2}");
    let (_, d) = post(&bots, &gem, json!({"room":room,"text":"Integrator done, tests pass.","tags":["done"],"reply":c1["post"]})).await;
    assert_eq!(d["released"], json!([c1["post"]]));
    for _ in 0..100 { if peers.locks.lock().unwrap().values().all(|h| h != &gem) { break } tokio::time::sleep(std::time::Duration::from_millis(10)).await; }
    assert!(peers.locks.lock().unwrap().values().all(|h| h != &gem), "the lock is free, after the world lock is let go");

    // the rate guard: a seventh post in five minutes is refused
    for i in 0..3 { let (ok, x) = post(&bots, &opus, json!({"room":room,"text":format!("note {i} on bounce timing {}", i * 7)})).await; assert!(ok, "post {i}: {x}") }
    let (ok, x) = post(&bots, &opus, json!({"room":room,"text":"and another"})).await; assert!(!ok && x["error"].as_str().unwrap().contains("slow down"), "{x}");

    // the loop guard: gemini and codex repeat each other with no claim or done: the room pauses, the tripping post wakes nobody
    let say = ["we should really just use the hall sensor instead of the encoder", "really we should just use the hall sensor instead of the encoder", "the hall sensor instead of the encoder is what we should use"];
    let mut paused = Value::Null;
    for i in 0..8 { let th = if i % 2 == 0 { &gem } else { &codex }; let (ok, x) = post(&bots, th, json!({"room":room,"text":say[i % 3],"tags":["critique"]})).await; assert!(ok, "{x}"); if x["paused"].is_string() { paused = x; break } }
    assert_eq!(paused["paused"], "circular", "{paused}");
    { let w = bots.world().await; let rm = w.room(&room).unwrap(); assert_eq!((rm.state.as_str(), rm.reason.as_str()), ("paused", "circular")); }
    assert!(peers.got.lock().unwrap().is_empty() || !peers.got.lock().unwrap().iter().any(|g| g.1.contains("paused itself")), "a guard post, not a report");
    let before = peers.rides.lock().unwrap().len();
    let (_, h) = post(&bots, &gem, json!({"room":room,"text":"still, hall sensor"})).await; assert_eq!(h["held"], true, "paused: held");
    assert_eq!(peers.rides.lock().unwrap().len(), before, "nobody is woken while paused");

    // the person steers: their post resumes the room and reaches every member at once
    let r2 = rpc(&hub, "swarm.post", json!({"room":room,"text":"No hall sensor: keep the encoder. Codex, write the state machine."})).await; assert_eq!(r2["ok"], true, "{r2}");
    { let w = bots.world().await; assert_eq!(w.room(&room).unwrap().state, ""); }
    for th in [&opus, &gem, &codex] { assert!(rides_to(&peers, th).last().unwrap().contains("keep the encoder"), "{th} hears the person at once") }

    // the board as the clients read it
    let g = rpc(&hub, "swarm.get", json!({"room":room})).await; let b = &g["result"];
    let names: Vec<&str> = b["members"].as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap()).collect(); assert_eq!(names, ["opus", "gemini", "codex"]);
    assert!(b["members"].as_array().unwrap().iter().all(|m| m["color"].as_str().unwrap().starts_with('#')));
    assert!(b["posts"].as_array().unwrap().iter().any(|p| p["guard"] == true && p["text"].as_str().unwrap().contains("circles")), "the guard's own post is on the board");
    assert!(b["posts"].as_array().unwrap().iter().any(|p| p["tags"] == json!(["claim"]) && p["files"] == json!(["src/debounce.rs"])));
    assert_eq!(b["claims"].as_array().unwrap().len(), 1, "codex's waiting claim is still open: {}", b["claims"]);
    assert_eq!(b["room"]["state"], "active");

    // a synthesis ends it: the members are told (no turn started), the starter hears the synthesis, the open claims go
    let (ok, s) = post(&bots, &codex, json!({"room":room,"text":"Synthesis: encoder kept; 3-state machine with 4 ms hysteresis; codex wrote it.","tags":["synthesis"]})).await; assert!(ok, "{s}");
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    { let w = bots.world().await; assert_eq!(w.room(&room).unwrap().state, "done"); }
    let told = peers.got.lock().unwrap().clone(); assert!(told.iter().any(|(t, x)| t == "t-boss" && x.contains("is done") && x.contains("4 ms hysteresis")), "{told:?}");
    assert!(peers.notes.lock().unwrap().iter().filter(|n| n.2.contains("The swarm is done")).count() == 3);
    assert!(peers.locks.lock().unwrap().is_empty(), "every claim let go");
    let (ok, e) = post(&bots, &opus, json!({"room":room,"text":"one more thing"})).await; assert!(!ok && e["error"].as_str().unwrap().contains("over"));
    // in the swarm list, done
    let ls = rpc(&hub, "swarm.list", json!({"project":"p1"})).await; assert_eq!(ls["result"]["swarms"][0]["state"], "done");
}

#[tokio::test] async fn a_swarm_whose_members_are_all_idle_pauses_and_its_starter_hears() {
    let (hub, bots, peers) = hub_of("idle").await;
    let (ok, r) = bots.tool_call("t-boss", "swarm_create", &json!({"topic":"quiet","members":[{"provider":"claude"},{"provider":"codex"}]})).await; assert!(ok, "{r}");
    let room = r["room"].as_str().unwrap().to_string();
    bots.swarm_tick(now_s()).await;
    { let w = bots.world().await; assert_eq!(w.room(&room).unwrap().state, "", "not idle long enough yet"); }
    bots.swarm_tick(now_s() + crate::bots::gate::LIMITS.idle + 1).await;
    { let w = bots.world().await; let rm = w.room(&room).unwrap(); assert_eq!((rm.state.as_str(), rm.reason.as_str()), ("paused", "idle")); }
    assert!(peers.got.lock().unwrap().iter().any(|(t, x)| t == "t-boss" && x.contains("paused itself")));
    // the person (a client) resumes; then stops it: no synthesis, the members told
    assert_eq!(rpc(&hub, "swarm.act", json!({"room":room,"action":"resume"})).await["ok"], true);
    assert_eq!(rpc(&hub, "swarm.act", json!({"room":room,"action":"stop"})).await["ok"], true);
    { let w = bots.world().await; assert_eq!(w.room(&room).unwrap().state, "stopped"); }
    // a member cannot control it
    let m = peers.launched.lock().unwrap()[0]["_id"].as_str().unwrap().to_string();
    let (ok, x) = bots.tool_call(&m, "swarm_control", &json!({"room":room,"action":"resume"})).await; assert!(!ok, "{x}");
}

#[tokio::test] async fn members_join_and_leave_and_a_running_codex_member_is_not_restarted_by_a_peer() {
    let (hub, bots, peers) = hub_of("members").await;
    let (_, r) = bots.tool_call("t-boss", "swarm_create", &json!({"topic":"pair","members":[{"provider":"claude","role":"designer"},{"provider":"codex","role":"tester"}]})).await;
    let room = r["room"].as_str().unwrap().to_string();
    let (des, tst) = { let l = peers.launched.lock().unwrap(); (l[0]["_id"].as_str().unwrap().to_string(), l[1]["_id"].as_str().unwrap().to_string()) };
    peers.turn.lock().unwrap().insert(tst.clone(), "running".into());
    post(&bots, &des, json!({"room":room,"text":"first sketch"})).await;
    assert!(rides_to(&peers, &tst).is_empty() && peers.notes.lock().unwrap().iter().any(|n| n.0 == tst), "a running Codex turn takes it with its next tool result, not a restart");
    post(&bots, &des, json!({"room":room,"text":"@tester look at this"})).await;
    assert!(rides_to(&peers, &tst).is_empty(), "a mention gets there at once, but still never restarts a running Codex turn (the live run: five restarts and no post)");
    assert_eq!(rpc(&hub, "swarm.post", json!({"room":room,"text":"tester: stop and write the tests"})).await["ok"], true);
    assert_eq!(rides_to(&peers, &tst).len(), 1, "the person's word steers it");
    // added by the person: it starts with the joining brief and hears only what comes after it joined
    let a = rpc(&hub, "swarm.act", json!({"room":room,"action":"add","provider":"claude","role":"critic"})).await; assert_eq!(a["ok"], true, "{a}");
    let crit = a["result"]["thread"].as_str().unwrap().to_string();
    assert!(peers.launched.lock().unwrap().last().unwrap()["prompt"].as_str().unwrap().contains("room_read the board first"));
    post(&bots, &des, json!({"room":room,"text":"second sketch, with @critic please"})).await;
    let c = rides_to(&peers, &crit); assert_eq!(c.len(), 1); assert!(c[0].contains("second sketch") && !c[0].contains("first sketch"), "{}", c[0]);
    // removed: no longer a member
    assert_eq!(rpc(&hub, "swarm.act", json!({"room":room,"action":"remove","thread":tst})).await["ok"], true);
    let (ok, _) = post(&bots, &tst, json!({"room":room,"text":"am I still here?"})).await; assert!(!ok);
}

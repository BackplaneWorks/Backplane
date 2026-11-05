//! The teach and task driver against a real Hub and the stand-in helper (see browser/tests.rs): requests, recorder events, saving, task runs and the pause.
use super::*;
use crate::{auth::Req, hub::Hub};
use super::super::{browser::tests::hub_with_helper, tests::Tap};
use std::time::{Duration, Instant};

fn caller() -> Caller { Caller { gen: "1".into(), client: 1 } }
async fn rpc(hub: &Arc<Hub>, m: &str, p: Value) -> Value { let h = hub.rpc_for(m).expect("handler"); h.call(hub, &caller(), &json!(7), m, &p).await.remove(0) }
async fn wait<F: FnMut() -> bool>(secs: u64, mut f: F) -> bool { let end = Instant::now() + Duration::from_secs(secs); while Instant::now() < end { if f() { return true } tokio::time::sleep(Duration::from_millis(25)).await } f() }
fn sent(log: &std::path::Path) -> Vec<Value> { std::fs::read_to_string(log).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect() }
async fn setup(name: &str) -> (Arc<Hub>, Arc<Driver>, PathBuf) {
    let (hub, d) = hub_with_helper(name).await; let log = d.join("standin.log"); (hub.clone(), get(&hub).unwrap(), log)
}

#[tokio::test] async fn teaching_records_the_persons_steps_and_a_save_makes_a_task_the_bot_is_woken_with() {
    let (hub, d, log) = setup("teach").await; let tap = Tap::new(&hub, 1);
    let woke: Arc<std::sync::Mutex<Vec<(String, String)>>> = Default::default(); let w2 = woke.clone(); d.set_wake(Arc::new(move |b, t| w2.lock().unwrap().push((b.into(), t))));
    assert_eq!(rpc(&hub, "bots.teach", json!({"bot":"b1","op":"start"})).await["ok"], true);
    assert!(!d.br.agent_drives_ok("b1").await, "the agent is refused while the person teaches");
    assert!(wait(5, || sent(&log).iter().any(|r| r["op"] == "record" && r["on"] == true && r["page"] == "b1")).await);
    // the helper's recorder steps join the draft; a secret keeps only its label
    d.br.line(r#"{"event":"record","page":"b1","step":{"do":"navigate","url":"https://x.test/o"}}"#).await;
    d.br.line(r#"{"event":"record","page":"b1","step":{"do":"type","value":"1042","target":{"name":"Order"}}}"#).await;
    d.br.line(r#"{"event":"record","page":"b1","step":{"do":"type","secret":true,"value":"hunter2","target":{"name":"Password"}}}"#).await;
    assert!(wait(5, || d.st.try_lock().map_or(false, |g| g.teach.get("b1").steps.len() == 3)).await);
    rpc(&hub, "bots.teach", json!({"bot":"b1","op":"note","text":"then wait"})).await;
    assert!(wait(5, || { let g = d.st.try_lock(); g.map_or(false, |g| g.teach.get("b1").steps.len() == 4) }).await);
    let draft = std::fs::read_to_string(d.dir("b1").join(".draft.jsonl")).unwrap(); assert!(!draft.contains("hunter2") && draft.lines().count() == 4);
    let infos: Vec<Value> = { tokio::time::sleep(Duration::from_millis(100)).await; tap.take().into_iter().filter(|v| v["t"] == "info").collect() };
    assert!(infos.iter().any(|v| v["info"]["drive.b1"] == "teach"), "one frame per key (Hub::set_info)"); assert!(infos.iter().any(|v| v["info"]["teach.b1"].as_str().map_or(false, |t| t.starts_with("rec ")))); assert!(!infos.iter().any(|v| v.to_string().contains("hunter2")));
    assert!(wait(5, || hub.info_frame()["info"]["teach.draft.b1"].as_str().map_or(false, |t| t.contains("1042") && t.contains("then wait"))).await, "a client that joins now gets the draft"); let joined = hub.info_frame()["info"].clone(); assert_eq!(joined["drive.b1"], "teach"); assert!(!joined.to_string().contains("hunter2"));
    rpc(&hub, "bots.teach", json!({"bot":"b1","op":"stop"})).await; assert_eq!(d.st.lock().await.teach.get("b1").mode, "review");
    assert_eq!(rpc(&hub, "bots.teach", json!({"bot":"b1","op":"save","name":" "})).await["error"], "name the task and record at least one step");
    let r = rpc(&hub, "bots.teach", json!({"bot":"b1","op":"save","name":"Order status","description":"look up","params":[{"step":2,"name":"order","label":"Order number"}]})).await; assert_eq!(r["ok"], true, "{r}");
    // logged as TaskSet, copied to a file, listed to the bot, the bot woken with what it was shown, the page the bot's again
    let ev: Value = (0..hub.store.len()).rev().filter_map(|i| serde_json::from_str::<Value>(&hub.store.get(i).unwrap()).ok()).find(|v| v["$"] == "TaskSet").unwrap();
    assert_eq!((ev["bot"].as_str(), ev["slug"].as_str()), (Some("b1"), Some("order-status"))); assert!(!ev["json"].as_str().unwrap().contains("hunter2"));
    assert!(d.dir("b1").join("order-status.json").exists());
    let (ok, list) = d.tool("b1", "task_list", &json!({})).await; assert!(ok && list[0]["slug"] == "order-status" && list[0]["params"] == json!(["order"]), "{list}");
    let w = woke.lock().unwrap().clone(); assert_eq!(w.len(), 1); assert!(w[0].1.starts_with("[taught: Order status]") && w[0].1.contains("<secret: Password>"));
    assert!(d.br.agent_drives_ok("b1").await); assert!(d.st.lock().await.teach.get("b1").idle());
    let after = hub.info_frame()["info"].clone(); assert!(after.get("drive.b1").is_none() && after.get("teach.b1").is_none() && after.get("teach.draft.b1").is_none(), "an idle bot's keys are removed: {after}");
}

#[tokio::test] async fn the_persons_input_is_scaled_and_refused_while_the_bot_drives() {
    let (hub, d, log) = setup("input").await;
    let r = rpc(&hub, "bots.browser", json!({"bot":"b1","op":"click","x":500,"y":500})).await; assert_eq!(r["ok"], false, "{r}");
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"takeover"})).await["ok"], true);
    assert!(!d.br.agent_drives_ok("b1").await);
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"click","x":500,"y":1000})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"mouse","type":"drag","x":0,"y":0})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"scroll","dy":120})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"explode"})).await["ok"], true, "an unknown op while the person drives is a no-op, as in Bend");
    assert!(wait(10, || { let r = sent(&log); ["click", "mouse", "scroll"].iter().all(|o| r.iter().any(|x| x["op"] == *o)) }).await, "the queued input reached the helper");
    let reqs = sent(&log); let click = reqs.iter().find(|r| r["op"] == "click").unwrap(); assert_eq!((click["x"].as_u64(), click["y"].as_u64(), click["page"].as_str()), (Some(640), Some(799), Some("b1")));
    assert_eq!(reqs.iter().find(|r| r["op"] == "mouse").unwrap()["type"], "move", "only move, down or up");
    assert_eq!(reqs.iter().find(|r| r["op"] == "scroll").unwrap()["dy"], 120);
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"handback"})).await["ok"], true); assert!(d.br.agent_drives_ok("b1").await);
}

#[tokio::test] async fn a_recording_ends_after_ten_minutes() {
    let (hub, d, _log) = setup("expire").await; rpc(&hub, "bots.teach", json!({"bot":"b1","op":"start"})).await;
    let start = d.st.lock().await.teach.get("b1").start; d.tick(start + 599).await; assert_eq!(d.st.lock().await.teach.get("b1").mode, "rec"); d.tick(start + 600).await; assert_eq!(d.st.lock().await.teach.get("b1").mode, "review");
    rpc(&hub, "bots.teach", json!({"bot":"b1","op":"discard"})).await; assert!(d.st.lock().await.teach.get("b1").idle());
}

#[tokio::test] async fn a_task_runs_with_its_parameters_logs_its_steps_pauses_for_the_person_and_ends_pruned() {
    let (hub, d, _log) = setup("run").await; let thread = "b1";
    let t = json!({"slug":"look-up","name":"Look up","steps":[{"n":1,"do":"navigate","url":"https://x.test"},{"n":2,"do":"type","value":"{{order}}","target":{"role":"textbox","name":"Order"}},{"n":3,"do":"click","target":{"role":"button","name":"Pay now"}}],"params":[{"name":"order"}]});
    let (ok, _) = d.tool(thread, "task_save", &t).await; assert!(ok);
    let (ok, e) = d.tool(thread, "task_run", &json!({"slug":"look-up"})).await; assert!(!ok); assert_eq!(e["error"], "missing parameter: order");
    let (ok, e) = d.tool(thread, "task_run", &json!({"slug":"nope"})).await; assert!(!ok && e["error"].as_str().unwrap().starts_with("no such task"));
    let (ok, r) = d.tool(thread, "task_run", &json!({"slug":"look-up","params":{"order":"1042"}})).await; assert!(ok, "{r}");
    let run = r["run"].as_str().unwrap().to_string(); assert!(teach::run_ok(&run)); assert_eq!(r["plan"]["steps"][1]["value"], "1042"); assert_eq!(r["plan"]["steps"][2]["risk"], "purchase"); assert_eq!(r["plan"]["steps"][1]["locators"][0], "role=textbox[name=\"Order\"]");
    assert!(d.run_dir(thread, &run).is_dir());
    let (ok, s) = d.tool(thread, "task_step", &json!({"run":run,"n":1,"ok":true,"note":"opened"})).await; assert!(ok && s["shot"] == "");
    assert_eq!(d.tool(thread, "task_step", &json!({"run":"../x-1","n":1})).await.1["error"], "no such run");
    // a pause opens an ask and waits for the person; a risky one is answered only by confirm; the answer reaches the bot
    let (d2, run2) = (d.clone(), run.clone()); let h = tokio::spawn(async move { d2.tool("b1", "task_pause", &json!({"run":run2,"n":3,"kind":"purchase","question":"Pay?","risky":true})).await });
    let mut ask = String::new(); assert!(wait(10, || { for i in (0..hub.store.len()).rev() { if let Ok(v) = serde_json::from_str::<Value>(&hub.store.get(i).unwrap()) { if v["$"] == "AskOpened" && v["tool"] == "task_pause" { ask = v["id"].as_str().unwrap().into(); assert_eq!(v["options"], "confirm\nskip\nstop"); assert_eq!(v["kind"], "browser"); assert!(v["detail"].as_str().unwrap().contains("/bots/b1/tasks/shot?run=")); return true } } } false }).await);
    assert!(matches!(d.st.lock().await.runs[&run].state, Run::Waiting { risky: true, .. }));
    assert!(d.answered(&ask, "confirm").await && !d.answered(&ask, "confirm").await);
    let (ok, text) = h.await.unwrap(); assert!(ok && text.as_str().unwrap().starts_with("The person confirmed")); assert!(matches!(d.st.lock().await.runs[&run].state, Run::Running(_)));
    let (ok, _) = d.tool(thread, "task_end", &json!({"run":run,"status":"ok","summary":"done"})).await; assert!(ok); assert!(matches!(d.st.lock().await.runs[&run].state, Run::Ended(_)));
    let end: Value = (0..hub.store.len()).rev().filter_map(|i| serde_json::from_str::<Value>(&hub.store.get(i).unwrap()).ok()).find(|v| v["act"].as_str().map_or(false, |a| a.ends_with(".end"))).unwrap();
    let sum: Value = serde_json::from_str(end["summary"].as_str().unwrap()).unwrap(); assert_eq!((sum["status"].as_str(), sum["summary"].as_str()), (Some("ok"), Some("done"))); assert!(sum["n"].as_u64().unwrap() >= 2);
    let log = std::fs::read_to_string(d.run_dir(thread, &run).join("log.jsonl")).unwrap(); assert!(log.contains("opened") && log.contains("\"end\":true"));
}

#[tokio::test] async fn runs_beyond_the_newest_twenty_are_pruned() {
    let dir = std::env::temp_dir().join(format!("bp-prune-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir);
    for i in 0..25 { std::fs::create_dir_all(dir.join(format!("r{i:02}"))).unwrap(); std::thread::sleep(Duration::from_millis(15)) }
    prune(&dir, KEEP_RUNS); let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect(); left.sort();
    assert_eq!(left.len(), 20); assert_eq!(left[0], "r05"); assert_eq!(left[19], "r24");
}

#[tokio::test] async fn tasks_come_back_from_the_log_and_a_delete_removes_them() {
    let (hub, d, _log) = setup("fold").await;
    hub.commit(&json!({"$":"TaskSet","bot":"b9","slug":"old","json":task::json(&task::parse(&json!({"slug":"old","name":"Old"})).unwrap()).to_string(),"at":1})).await.unwrap();
    let (ok, l) = d.tool("b9", "task_list", &json!({})).await; assert!(ok && l[0]["slug"] == "old");
    let (ok, g) = d.tool("b9", "task_get", &json!({"slug":"old"})).await; assert!(ok && g["name"] == "Old");
    d.tool("b9", "task_delete", &json!({"slug":"old"})).await; assert_eq!(d.tool("b9", "task_list", &json!({})).await.1, json!([]));
    assert_eq!(d.tool("b9", "task_get", &json!({"slug":"old"})).await.1["error"], "no such task: task_list shows yours");
    assert_eq!(d.tool("b2", "task_list", &json!({})).await.1, json!([]), "another bot's tasks are its own");
}

#[tokio::test] async fn a_press_on_the_shield_takes_the_page_over() {
    let (_hub, d, _log) = setup("shield").await;
    d.event(&json!({"event":"shield-click","page":"b1"})).await; assert_eq!(d.st.lock().await.teach.get("b1").drive, "takeover");
    d.event(&json!({"event":"record","page":"b1","step":{"do":"click"}})).await; assert!(d.st.lock().await.teach.get("b1").steps.is_empty(), "not recording");
}

#[tokio::test] async fn a_runs_pictures_are_served_only_from_the_bots_own_tasks_folder() {
    let (hub, d, _log) = setup("shot").await; let run = "look-up-5";
    let dir = d.run_dir("b1", run); std::fs::create_dir_all(&dir).unwrap(); std::fs::write(dir.join("pause-3.jpg"), b"JPEG3").unwrap(); std::fs::write(dir.join("1.jpg"), b"JPEG1").unwrap(); std::fs::write(d.dir("b1").join("secret.jpg"), b"NO").unwrap();
    let get = |t: &str| Req::new("GET", t, vec![]);
    let call = |t: String| { let hub = hub.clone(); async move { hub.http_for("/bots/b1/tasks/shot").unwrap().call(&hub, &get(&t)).await } };
    let r = call("/bots/b1/tasks/shot?run=look-up-5&n=3".into()).await; assert_eq!((r.status, r.ctype.as_str(), r.body.as_slice()), ("200 OK", "image/jpeg", &b"JPEG3"[..]));
    assert_eq!(call("/bots/b1/tasks/shot?run=look-up-5&f=1.jpg".into()).await.body, b"JPEG1");
    for bad in ["/bots/b1/tasks/shot?run=look-up-5&f=../../secret.jpg", "/bots/b1/tasks/shot?run=..-1&n=3", "/bots/b1/tasks/shot?run=look-up-5&f=nope.jpg", "/bots/b2/tasks/shot?run=look-up-5&n=3", "/bots/../tasks/shot?run=look-up-5&n=3", "/bots/b1/tasks/shot?run=look-up-5&f=%2e%2e%2fsecret.jpg", "/bots/b1/other"] {
        assert_eq!(call(bad.into()).await.status, "404 Not Found", "{bad}");
    }
}

#[tokio::test] async fn an_unknown_owner_is_refused_and_the_words_are_bends() {
    let (hub, d, _log) = setup("owner").await;
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"nobody","op":"takeover"})).await["error"], "unknown browser owner");
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"","op":"takeover"})).await["error"], "unknown browser owner");
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"../x","op":"takeover"})).await["error"], "unknown browser owner");
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"click"})).await["error"], "take over the browser first");
    assert_eq!(rpc(&hub, "bots.teach", json!({"bot":"b1","op":"dance"})).await["error"], "unknown teach request");
    assert!(d.st.lock().await.teach.get("nobody").idle());
}
#[tokio::test] async fn the_persons_input_runs_in_order_on_one_queue_per_page() {
    let (hub, _d, log) = setup("order").await; rpc(&hub, "bots.browser", json!({"bot":"b1","op":"takeover"})).await;
    for t in ["down", "move", "up"] { assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"mouse","type":t,"x":10,"y":10})).await["ok"], true) }
    for k in ["a", "b", "c"] { rpc(&hub, "bots.browser", json!({"bot":"b1","op":"press","key":k})).await; }
    assert!(wait(10, || sent(&log).iter().filter(|r| r["op"] == "press").count() == 3).await);
    let ops: Vec<String> = sent(&log).iter().filter(|r| r["op"] == "mouse" || r["op"] == "press").map(|r| r["type"].as_str().or(r["key"].as_str()).unwrap().to_string()).collect();
    assert_eq!(ops, vec!["down", "move", "up", "a", "b", "c"]);
}
#[tokio::test] async fn a_toolbar_command_takes_the_page_over_and_its_reply_comes_later() {
    let (hub, d, log) = setup("toolbar").await; let tap = Tap::new(&hub, 1);
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"toolbar","command":"explode"})).await["error"], "unsupported browser toolbar command");
    let h = hub.rpc_for("bots.browser").unwrap(); let frames = h.call(&hub, &caller(), &json!(42), "bots.browser", &json!({"bot":"b1","op":"toolbar","command":"navigate","url":"example.com/x"})).await; assert!(frames.is_empty(), "the reply is the helper's, later");
    assert_eq!(d.st.lock().await.teach.get("b1").drive, "takeover");
    let mut reply = None; assert!(wait(10, || { reply = tap.take().into_iter().find(|v| v["t"] == "reply" && v["id"] == 42); reply.is_some() }).await); assert_eq!(reply.unwrap()["ok"], true);
    assert_eq!(sent(&log).iter().find(|r| r["op"] == "navigate").unwrap()["url"], "https://example.com/x");
    assert_eq!((cef_url("localhost:3000"), cef_url("host.test:8080/p"), cef_url("a b"), cef_url("javascript:x"), cef_url(" about:blank "), cef_url("")), ("http://localhost:3000".to_string(), "http://host.test:8080/p".to_string(), String::new(), String::new(), "about:blank".to_string(), String::new()));
    assert_eq!(toolbar_args("tabs", &json!({"new":true}))["new"], "https://www.google.com/"); assert_eq!(toolbar_args("tabs", &json!({"selectId":"e1"})), json!({"selectId":"e1"})); assert_eq!(toolbar_args("tabs", &json!({"closeId":"e2"})), json!({"closeId":"e2"})); assert_eq!(toolbar_args("back", &json!({})), json!({}));
}
#[tokio::test] async fn a_pause_marks_the_thread_pending_frees_its_waiter_when_dropped_and_closes_an_unheard_ask() {
    let (hub, d, _log) = setup("pend").await; let run = "r-1".to_string(); std::fs::create_dir_all(d.run_dir("b1", &run)).unwrap();
    let find = |kind: &str| { let mut out = vec![]; for i in 0..hub.store.len() { if let Ok(v) = serde_json::from_str::<Value>(&hub.store.get(i).unwrap()) { if v["$"] == kind { out.push(v) } } } out };
    let (d2, r2) = (d.clone(), run.clone()); let h = tokio::spawn(async move { d2.tool("b1", "task_pause", &json!({"run":r2,"n":1})).await });
    assert!(wait(10, || !find("AskOpened").is_empty()).await); assert_eq!(find("PendingChanged").last().unwrap()["count"], 1);
    assert_eq!(d.waiters.lock().unwrap().len(), 1);
    h.abort(); let _ = h.await; assert!(wait(5, || d.waiters.lock().unwrap().is_empty()).await, "an aborted tool call leaves no waiter behind");
    // the ask nobody hears is closed, with the count back down
    d.pause_ms.store(200, std::sync::atomic::Ordering::Relaxed);
    let (ok, e) = d.tool("b1", "task_pause", &json!({"run":run,"n":2})).await; assert!(!ok && e["error"] == "nobody answered the question");
    let closed = find("AskClosed"); assert_eq!(closed.last().unwrap()["answer"], ""); assert!(d.waiters.lock().unwrap().is_empty());
    for i in 0..MAX_WAITERS { d.waiters.lock().unwrap().insert(format!("w{i}"), ("b1".into(), oneshot::channel().0)); }
    let (ok, e) = d.tool("b1", "task_pause", &json!({"run":run,"n":3})).await; assert!(!ok && e["error"].as_str().unwrap().contains("too many"), "per thread: {e}");
    let (d3, r3) = (d.clone(), run.clone()); let (ok, _) = tokio::spawn(async move { d3.tool("b2", "task_pause", &json!({"run":r3,"n":4})).await }).await.unwrap(); assert!(!ok, "b2 is a different thread: it asks, and times out here");
}

#[tokio::test] async fn the_pages_newest_frame_is_served_as_screen_jpg_and_never_cached() {
    let (hub, d, _log) = setup("screen").await; let _ = d; let get = |t: &str| Req::new("GET", t, vec![]);
    let call = |t: String| { let hub = hub.clone(); async move { hub.http_for("/bots/b1/screen.jpg").unwrap().call(&hub, &get(&t)).await } };
    assert_eq!(call("/bots/b1/screen.jpg".into()).await.status, "404 Not Found");
    std::fs::create_dir_all(hub.cfg.home.join("shots")).unwrap(); std::fs::write(hub.cfg.home.join("shots/b1.jpg"), b"FRAME").unwrap();
    let r = call("/bots/b1/screen.jpg".into()).await; assert_eq!((r.status, r.ctype.as_str(), r.body.as_slice()), ("200 OK", "image/jpeg", &b"FRAME"[..])); assert!(r.extra.contains("no-store"));
    for bad in ["/bots/../screen.jpg", "/bots/a%2fb/screen.jpg", "/bots//screen.jpg"] { assert_eq!(call(bad.into()).await.status, "404 Not Found", "{bad}") }
}

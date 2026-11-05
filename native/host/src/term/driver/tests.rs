//! The teach and task driver against a real Hub and the stand-in helper (see browser/tests.rs): requests, recorder events, saving, task runs and the pause.
use super::*;
use crate::hub::Hub;
use super::super::{browser::tests::hub_with_helper, tests::Tap};
use std::time::{Duration, Instant};

fn caller() -> Caller { Caller { gen: "1".into(), client: 1 } }
async fn rpc(hub: &Arc<Hub>, m: &str, p: Value) -> Value { let h = hub.rpc_for(m).expect("handler"); h.call(hub, &caller(), &json!(7), m, &p).await.remove(0) }
async fn wait<F: FnMut() -> bool>(secs: u64, mut f: F) -> bool { let end = Instant::now() + Duration::from_secs(secs); while Instant::now() < end { if f() { return true } tokio::time::sleep(Duration::from_millis(25)).await } f() }
fn sent(log: &std::path::Path) -> Vec<Value> { std::fs::read_to_string(log).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect() }
async fn setup(name: &str) -> (Arc<Hub>, Arc<Driver>, PathBuf) {
    let log = std::env::temp_dir().join(format!("bp-drv-{}-{name}.log", std::process::id())); let _ = std::fs::remove_file(&log); std::env::set_var("BP_STANDIN_LOG", &log);
    let (hub, _d) = hub_with_helper(name).await; (hub.clone(), get(&hub).unwrap(), log)
}

#[tokio::test] async fn teaching_records_the_persons_steps_and_a_save_makes_a_task_the_bot_is_woken_with() {
    let (hub, d, log) = setup("teach").await; let tap = Tap::new(&hub, 1);
    let woke: Arc<std::sync::Mutex<Vec<(String, String)>>> = Default::default(); let w2 = woke.clone(); d.set_wake(Arc::new(move |b, t| w2.lock().unwrap().push((b.into(), t))));
    assert_eq!(rpc(&hub, "bots.teach", json!({"bot":"b1","op":"start"})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.teach", json!({"bot":"b1","op":"start"})).await["ok"], false, "one teach at a time");
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
    assert!(infos.iter().any(|v| v["info"]["drive.b1"] == "teach" && v["info"]["teach.b1"].as_str().map_or(false, |t| t.starts_with("rec ")))); assert!(!infos.iter().any(|v| v.to_string().contains("hunter2")));
    rpc(&hub, "bots.teach", json!({"bot":"b1","op":"stop"})).await; assert_eq!(d.st.lock().await.teach.get("b1").mode, "review");
    assert_eq!(rpc(&hub, "bots.teach", json!({"bot":"b1","op":"save","name":" "})).await["error"], "name the task");
    let r = rpc(&hub, "bots.teach", json!({"bot":"b1","op":"save","name":"Order status","description":"look up","params":[{"step":2,"name":"order","label":"Order number"}]})).await; assert_eq!(r["ok"], true, "{r}");
    // logged as TaskSet, copied to a file, listed to the bot, the bot woken with what it was shown, the page the bot's again
    let ev: Value = (0..hub.store.len()).rev().filter_map(|i| serde_json::from_str::<Value>(&hub.store.get(i).unwrap()).ok()).find(|v| v["$"] == "TaskSet").unwrap();
    assert_eq!((ev["bot"].as_str(), ev["slug"].as_str()), (Some("b1"), Some("order-status"))); assert!(!ev["json"].as_str().unwrap().contains("hunter2"));
    assert!(d.dir("b1").join("order-status.json").exists());
    let (ok, list) = d.tool("b1", "task_list", &json!({})).await; assert!(ok && list[0]["slug"] == "order-status" && list[0]["params"] == json!(["order"]), "{list}");
    let w = woke.lock().unwrap().clone(); assert_eq!(w.len(), 1); assert!(w[0].1.starts_with("[taught: Order status]") && w[0].1.contains("<secret: Password>"));
    assert!(d.br.agent_drives_ok("b1").await); assert!(d.st.lock().await.teach.get("b1").idle());
}

#[tokio::test] async fn the_persons_input_is_scaled_and_refused_while_the_bot_drives() {
    let (hub, d, log) = setup("input").await;
    let r = rpc(&hub, "bots.browser", json!({"bot":"b1","op":"click","x":500,"y":500})).await; assert_eq!(r["ok"], false, "{r}");
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"takeover"})).await["ok"], true);
    assert!(!d.br.agent_drives_ok("b1").await);
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"click","x":500,"y":1000})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"mouse","type":"drag","x":0,"y":0})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"scroll","dy":120})).await["ok"], true);
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":"b1","op":"explode"})).await["ok"], false);
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

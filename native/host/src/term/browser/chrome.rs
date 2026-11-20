//! The driver against the real helper (tools/browser, Bun + playwright-core) and real headless Chrome, on a page this test serves from loopback: no accounts,
//! no real sites. `#[ignore]`d: it needs BP_REAL_BROWSER (a script that runs the helper) and BACKPLANE_CHROME. Run by test/term/boat-chrome-job.sh.
use super::super::{driver, task, tests::Tap};
use super::*;
use crate::{api::Caller, hub::{Cfg, Hub}, presenter::Canned};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PAGE: &str = "<!doctype html><title>start</title><h1>Hello Backplane</h1><button onclick=\"document.title='clicked'\">Go</button> <input id=q placeholder=\"Search\"> <input id=pw type=password aria-label=\"Password\">";

async fn serve() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    tokio::spawn(async move { loop { let Ok((mut s, _)) = l.accept().await else { break }; tokio::spawn(async move { let mut b = [0u8; 4096]; let _ = s.read(&mut b).await; let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}", PAGE.len()).as_bytes()).await; }); } });
    port
}
fn caller() -> Caller { Caller { gen: "1".into(), client: 1 } }
async fn rpc(hub: &Arc<Hub>, m: &str, p: Value) -> Value { let h = hub.rpc_for(m).expect("handler"); h.call(hub, &caller(), &json!(7), m, &p).await.remove(0) }
fn text(body: &Value) -> String { body["$content"][0]["text"].as_str().unwrap_or("").to_string() }
async fn eval(hub: &Arc<Hub>, bot: &str, e: &str) -> Value { let (ok, b) = preview(hub, bot, "preview_evaluate", &json!({"expression": e})).await; assert!(ok, "{b}"); serde_json::from_str::<Value>(&text(&b)).unwrap_or(Value::Null) }
/// The hub's own evaluate (the agent is refused while the person teaches).
async fn beval(drv: &Arc<driver::Driver>, bot: &str, e: &str) -> Value { let l: Value = serde_json::from_str(&drv.br.ask(bot, "evaluate", &json!({"expression": e})).await).unwrap(); assert_eq!(l["ok"], true, "{l}"); l["result"].clone() }
async fn wait<F: FnMut() -> bool>(secs: u64, mut f: F) -> bool { let end = std::time::Instant::now() + Duration::from_secs(secs); while std::time::Instant::now() < end { if f() { return true } tokio::time::sleep(Duration::from_millis(100)).await } f() }

#[tokio::test(flavor = "multi_thread", worker_threads = 4)] #[ignore]
async fn real_chrome_through_the_driver_preview_teach_and_task() {
    let exe = std::env::var("BP_REAL_BROWSER").expect("BP_REAL_BROWSER"); std::env::set_var("BACKPLANE_BROWSER", exe);
    let d = crate::plat::tmp::dir("chrome");
    let mut log = String::from("{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"f\",\"root\":\"/tmp\",\"at\":1700000000}\n");
    for t in ["t1", "t2"] { log += &format!("{{\"$\":\"ThreadCreated\",\"id\":\"{t}\",\"project\":\"p1\",\"title\":\"T\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":1700000001}}\n"); }
    std::fs::write(d.join("events.jsonl"), log).unwrap();
    let hub = Hub::start(Cfg { home: d.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    super::super::start(hub.clone()); let tap = Tap::new(&hub, 1); let port = serve().await; let url = format!("http://127.0.0.1:{port}/"); let bot = "t1";
    let drv = super::super::driver::get(&hub).unwrap();

    // preview_*: open, read, click, type, evaluate, screenshot
    let (ok, b) = preview(&hub, bot, "preview_open", &json!({"url": url})).await; assert!(ok, "{b}");
    let (ok, b) = preview(&hub, bot, "preview_snapshot", &json!({})).await; assert!(ok, "{b}"); assert!(text(&b).contains("Hello Backplane") && text(&b).contains("Go"), "{}", text(&b));
    assert_eq!(eval(&hub, bot, "document.title").await, json!("start"));
    let (ok, b) = preview(&hub, bot, "preview_click", &json!({"locator":"role=button[name='Go']"})).await; assert!(ok, "{b}"); assert_eq!(eval(&hub, bot, "document.title").await, json!("clicked"));
    let (ok, b) = preview(&hub, bot, "preview_type", &json!({"locator":"input#q","text":"abc"})).await; assert!(ok, "{b}"); assert_eq!(eval(&hub, bot, "document.querySelector('#q').value").await, json!("abc"));
    let (ok, b) = preview(&hub, bot, "preview_snapshot", &json!({"screenshot": true})).await; assert!(ok, "{b}");
    let img = b["$content"].as_array().unwrap().iter().find(|x| x["type"] == "image").expect("an image block"); assert!(img["data"].as_str().unwrap().starts_with("iVBOR") || img["data"].as_str().unwrap().starts_with("/9j/"), "png or jpeg");
    let (ok, b) = preview(&hub, bot, "preview_status", &json!({})).await; assert!(ok); assert!(text(&b).contains("127.0.0.1"), "{}", text(&b));
    // another thread has a page of its own
    let (ok, _) = preview(&hub, "t2", "preview_open", &json!({"url": url})).await; assert!(ok); assert_eq!(eval(&hub, "t2", "document.title").await, json!("start"), "t2's page did not see t1's click");
    // the JPEG frame and its announcement
    assert!(wait(20, || tap.take().iter().any(|v| v["t"] == "shot" && v["bot"] == bot)).await, "a shot event"); assert!(wait(10, || d.join("shots").join("t1.jpg").exists()).await, "the JPEG frame file");
    // a take-over refuses the agent; the person clicks the button by frame thousandths
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":bot,"op":"takeover"})).await["ok"], true);
    let (ok, b) = preview(&hub, bot, "preview_click", &json!({"locator":"role=button[name='Go']"})).await; assert!(!ok && text(&b) == REFUSAL);
    assert_eq!(rpc(&hub, "bots.browser", json!({"bot":bot,"op":"handback"})).await["ok"], true);

    // teaching: the person's own clicks and typing are recorded; a password keeps only its label
    assert_eq!(eval(&hub, bot, "document.title='start'").await, json!("start"));
    assert_eq!(rpc(&hub, "bots.teach", json!({"bot":bot,"op":"start"})).await["ok"], true, "teach start");
    let pt = |sel: &'static str| { let drv = drv.clone(); async move { let r = beval(&drv, bot, &format!("(()=>{{const r=document.querySelector(\"{sel}\").getBoundingClientRect();return [Math.round((r.x+r.width/2)*1000/innerWidth),Math.round((r.y+r.height/2)*1000/innerHeight)]}})()")).await; (r[0].as_u64().unwrap(), r[1].as_u64().unwrap()) } };
    let (x, y) = pt("button").await; assert_eq!(rpc(&hub, "bots.browser", json!({"bot":bot,"op":"click","x":x,"y":y})).await["ok"], true);
    let (x, y) = pt("#q").await; rpc(&hub, "bots.browser", json!({"bot":bot,"op":"click","x":x,"y":y})).await; rpc(&hub, "bots.browser", json!({"bot":bot,"op":"type","text":"order-1042"})).await;
    let (x, y) = pt("#pw").await; rpc(&hub, "bots.browser", json!({"bot":bot,"op":"click","x":x,"y":y})).await; rpc(&hub, "bots.browser", json!({"bot":bot,"op":"type","text":"hunter2"})).await;
    rpc(&hub, "bots.browser", json!({"bot":bot,"op":"press","key":"Tab"})).await;
    assert!(wait(30, || { let s = drv.st.try_lock(); s.map_or(false, |s| { let b = s.teach.get(bot); b.steps.iter().any(|x| x.act == "click" && x.target.name == "Go") && b.steps.iter().any(|x| matches!(x.value, task::Value::Secret(_))) }) }).await, "recorded steps: {:?}", drv.st.lock().await.teach.get(bot).steps.iter().map(|s| (s.act.clone(), s.target.name.clone(), s.value.clone())).collect::<Vec<_>>());
    let draft = std::fs::read_to_string(drv.dir(bot).join(".draft.jsonl")).unwrap(); assert!(!draft.contains("hunter2"), "no secret in the draft file");
    let b = drv.st.lock().await.teach.get(bot); let typed = b.steps.iter().find(|s| s.act == "type" && matches!(s.value, task::Value::Lit(_))).map(|s| s.n);
    rpc(&hub, "bots.teach", json!({"bot":bot,"op":"stop"})).await;
    let marks = match typed { Some(n) => json!([{"step": n, "name": "term", "label": "Search term"}]), None => json!([]) };
    let r = rpc(&hub, "bots.teach", json!({"bot":bot,"op":"save","name":"Look up","description":"types a term","params":marks})).await; assert_eq!(r["ok"], true, "{r}");
    let (_, saved) = drv.tool(bot, "task_get", &json!({"slug":"look-up"})).await; assert!(!saved.to_string().contains("hunter2"), "{saved}"); assert!(saved["steps"].as_array().unwrap().len() >= 2);

    // running it: the plan's locators work on the page, a pause is asked and answered, the run ends
    let (ok, run) = drv.tool(bot, "task_run", &json!({"slug":"look-up","params":{"term":"abc"}})).await; assert!(ok, "{run}");
    assert_eq!(eval(&hub, bot, "document.title='start'").await, json!("start"));
    let first_click = run["plan"]["steps"].as_array().unwrap().iter().find(|s| s["do"] == "click" && s["locators"].as_array().map_or(false, |l| !l.is_empty())).expect("a click step with locators");
    let (ok, b) = preview(&hub, bot, "preview_click", &json!({"locator": first_click["locators"][0]})).await; assert!(ok, "the recorded locator {} did not work: {b}", first_click["locators"][0]);
    assert_eq!(eval(&hub, bot, "document.title").await, json!("clicked"));
    let rid = run["run"].as_str().unwrap().to_string();
    let (d2, r2) = (drv.clone(), rid.clone()); let h = tokio::spawn(async move { d2.tool("t1", "task_pause", &json!({"run":r2,"n":1,"kind":"stuck","question":"Which one?"})).await });
    let mut ask = String::new(); assert!(wait(30, || { for i in (0..hub.store.len()).rev() { if let Ok(v) = serde_json::from_str::<Value>(&hub.store.get(i).unwrap()) { if v["$"] == "AskOpened" && v["tool"] == "task_pause" { ask = v["id"].as_str().unwrap().into(); return true } } } false }).await);
    assert!(wait(20, || drv.run_dir(bot, &rid).join("pause-1.jpg").exists()).await, "the pause's picture");
    let req = crate::auth::Req::new("GET", &format!("/bots/{bot}/tasks/shot?run={rid}&n=1"), vec![]);
    let pic = driver::tasks_http(&hub, &req).await.unwrap(); assert_eq!(pic.status, "200 OK"); assert!(pic.body.len() > 500 && pic.body.starts_with(&[0xFF, 0xD8]), "a JPEG");
    assert!(drv.answered(&ask, "continue").await); let (ok, t) = h.await.unwrap(); assert!(ok && t.as_str().unwrap().contains("continue"));
    let (ok, _) = drv.tool(bot, "task_end", &json!({"run":rid,"status":"ok","summary":"done"})).await; assert!(ok);
    assert_eq!(rpc(&hub, "browser.close", json!({"thread":bot})).await["ok"], true);
}

/// Settings › Browser › Start page end to end: the setting written as a client writes it, an agent's preview_open with no address opens it on a new page and leaves a
/// page that shows something alone, a phone's first look at a page with none opens it too (the screen.jpg lease), Blank leaves a new page empty, a bad address is refused
/// by setting.set and browser.page.act alike, and preview_open with an address is unchanged. The start page is this test's loopback page (no real site).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)] #[ignore]
async fn real_chrome_start_page_follows_the_setting() {
    use crate::settings::browser_start as bs;
    let exe = std::env::var("BP_REAL_BROWSER").expect("BP_REAL_BROWSER"); std::env::set_var("BACKPLANE_BROWSER", exe);
    let d = crate::plat::tmp::dir("chrome-start");
    let mut log = String::from("{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"f\",\"root\":\"/tmp\",\"at\":1700000000}\n");
    for t in ["t1", "t2", "t3", "t4"] { log += &format!("{{\"$\":\"ThreadCreated\",\"id\":\"{t}\",\"project\":\"p1\",\"title\":\"T\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":1700000001}}\n"); }
    std::fs::write(d.join("events.jsonl"), log).unwrap();
    let hub = Hub::start(Cfg { home: d.to_path_buf(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    super::super::start(hub.clone());
    hub.register_rpc(Arc::new(crate::settings::set::SettingSet { agents: None })); hub.register_rpc(Arc::new(bs::BrowserPage));
    let port = serve().await; let start = format!("http://127.0.0.1:{port}/start");
    let href = |bot: &'static str| { let hub = hub.clone(); async move { eval(&hub, bot, "location.href").await } };

    // a bad address is refused wherever it is written, and nothing is stored
    assert_eq!(rpc(&hub, "setting.set", json!({"key": bs::KEY, "value": "javascript:alert(1)"})).await["error"], bs::BAD);
    assert_eq!(rpc(&hub, "browser.page.act", json!({"action": bs::ACTION, "value": "save", "fields": {"bstart": "ftp://x"}})).await["error"], bs::BAD);
    assert_eq!(hub.setting(bs::KEY).await, None);
    let rows = rpc(&hub, "browser.page", json!({})).await["rows"].clone(); assert_eq!(rows[0]["fields"][0]["value"], bs::DEFAULT, "the default until one is chosen: {rows}");

    // the start page chosen as the desktop's Settings writes it; an agent's preview_open with no address opens it on its new page
    assert_eq!(rpc(&hub, "setting.set", json!({"key": bs::KEY, "value": start})).await["ok"], true);
    let (ok, b) = preview(&hub, "t1", "preview_open", &json!({})).await; assert!(ok, "{b}");
    assert_eq!(href("t1").await, json!(start)); assert_eq!(eval(&hub, "t1", "document.title").await, json!("start"));
    // a page that shows something stays where it is
    let elsewhere = format!("http://127.0.0.1:{port}/elsewhere");
    let (ok, b) = preview(&hub, "t1", "preview_navigate", &json!({"url": elsewhere})).await; assert!(ok, "{b}");
    let (ok, b) = preview(&hub, "t1", "preview_open", &json!({})).await; assert!(ok, "{b}"); assert_eq!(href("t1").await, json!(elsewhere));
    // preview_open with an address is unchanged
    let (ok, b) = preview(&hub, "t2", "preview_open", &json!({"url": elsewhere})).await; assert!(ok, "{b}"); assert_eq!(href("t2").await, json!(elsewhere));

    // a phone opening a Browser tab with no page yet: its first screen.jpg lease opens the start page
    let br = super::get(&hub).unwrap(); br.watch("t3").await;
    let mut seen = Value::Null;
    for _ in 0..100 { let s: Value = serde_json::from_str(&br.ask("t3", "status", &json!({})).await).unwrap(); seen = s["result"]["url"].clone(); if seen == json!(start) { break } tokio::time::sleep(Duration::from_millis(100)).await; }
    assert_eq!(seen, json!(start), "the phone's Browser tab opens the start page");

    // Blank, pressed on a phone: a new page stays empty
    assert_eq!(rpc(&hub, "browser.page.act", json!({"action": bs::ACTION, "value": "blank", "fields": {}})).await["ok"], true);
    assert_eq!(hub.setting(bs::KEY).await.as_deref(), Some("blank"));
    let (ok, b) = preview(&hub, "t4", "preview_open", &json!({})).await; assert!(ok, "{b}"); assert_eq!(href("t4").await, json!("about:blank"));
    // and Reset to default goes back to the default (not opened here: no real site in a test)
    assert_eq!(rpc(&hub, "browser.page.act", json!({"action": bs::ACTION, "value": "reset", "fields": {}})).await["ok"], true);
    assert_eq!(hub.setting(bs::KEY).await, None); assert_eq!(br.home_url().await.as_deref(), Some(bs::DEFAULT));
    let _ = std::fs::remove_dir_all(&d);
}

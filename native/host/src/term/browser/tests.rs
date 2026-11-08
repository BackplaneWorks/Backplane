//! The driver against a stand-in helper (a shell script that answers each request line and announces a frame on `open`): no Chrome.
use super::*;
use crate::{api::Caller, hub::{Cfg, Hub}, presenter::Canned};
use super::super::tests::Tap;

const STANDIN: &str = r#"#!/bin/sh
# stand-in backplane-browser: echoes each request line back in result.line; "open" also announces a frame; "big" answers a screenshot
while IFS= read -r l; do
  printf '%s\n' "$l" >> "$(dirname "$0")/standin.log"
  id=$(printf '%s' "$l" | sed 's/.*"id":\([0-9]*\).*/\1/')
  esc=$(printf '%s' "$l" | sed 's/\\/\\\\/g; s/"/\\"/g')
  case "$l" in
    *'"op":"open"'*) printf '{"event":"frame","page":"t1","n":3}\n' ;;
  esac
  case "$l" in
    *'"op":"snapshot"'*) printf '{"id":%s,"ok":true,"result":{"url":"u","screenshot":{"data":"QUJD","mimeType":"image/png"}}}\n' "$id" ;;
    *'"op":"evaluate"'*) printf '{"id":%s,"ok":false,"error":"boom"}\n' "$id" ;;
    *'"op":"die"'*) exit 3 ;;
    *) printf '{"id":%s,"ok":true,"result":{"line":"%s"}}\n' "$id" "$esc" ;;
  esac
done
"#;

pub(crate) async fn hub_with_helper(name: &str) -> (Arc<Hub>, PathBuf) {
    use crate::plat::fs::PermissionsExt;
    let d = std::env::temp_dir().join(format!("bp-br-{}-{}", std::process::id(), name)); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
    let exe = d.join("backplane-browser"); std::fs::write(&exe, STANDIN).unwrap(); std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut log = String::from("{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"f\",\"root\":\"/tmp\",\"at\":1700000000}\n");
    for t in ["t1", "t2", "b1", "b2", "b9"] { log += &format!("{{\"$\":\"ThreadCreated\",\"id\":\"{t}\",\"project\":\"p1\",\"title\":\"T\",\"env\":\"local\",\"branch\":\"\",\"worktree\":\"\",\"provider\":\"claude\",\"model\":\"\",\"at\":1700000001}}\n"); }
    std::fs::write(d.join("events.jsonl"), log).unwrap();
    let hub = Hub::start(Cfg { home: d.clone(), version: "0.1.0-dev".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() }, Arc::new(Canned { reply: json!({}) })).await.unwrap();
    // the helper is found when the driver starts: the variable and the start are one step, or a parallel test would give this hub its own helper
    { static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(()); let _g = ENV.lock().unwrap(); std::env::set_var("BACKPLANE_BROWSER", &exe); super::super::start(hub.clone()); }
    (hub, d)
}
fn text(body: &Value) -> String { body["$content"][0]["text"].as_str().unwrap().to_string() }

#[tokio::test] async fn a_preview_call_goes_to_the_helper_on_the_threads_own_page_and_arguments_cannot_change_the_page() {
    let (hub, _d) = hub_with_helper("page").await;
    let (ok, body) = preview(&hub, "t1", "preview_navigate", &json!({"url":"example.com","page":"someone-else","__desktopActor":"person"})).await;
    assert!(ok, "{body}"); let line: Value = serde_json::from_str(&text(&body)).unwrap(); let req: Value = serde_json::from_str(line["line"].as_str().unwrap()).unwrap();
    assert_eq!((req["op"].as_str(), req["page"].as_str(), req["url"].as_str()), (Some("navigate"), Some("t1"), Some("example.com")));
    assert!(req.get("__desktopActor").is_none(), "the actor mark is the dispatcher's, never an argument's");
}
#[tokio::test] async fn a_screenshot_becomes_an_image_block_and_an_error_is_flagged() {
    let (hub, _d) = hub_with_helper("shot").await;
    let (ok, body) = preview(&hub, "t1", "preview_snapshot", &json!({"screenshot":true})).await;
    assert!(ok && body["$content"][1] == json!({"type":"image","data":"QUJD","mimeType":"image/png"}), "{body}"); assert!(!text(&body).contains("QUJD"));
    let (ok, body) = preview(&hub, "t1", "preview_evaluate", &json!({"expression":"1"})).await; assert!(!ok); assert_eq!(text(&body), "boom");
}
#[tokio::test] async fn a_new_frame_is_announced_to_every_client() {
    let (hub, _d) = hub_with_helper("frame").await; let rx = Tap::new(&hub, 1);
    preview(&hub, "t1", "preview_open", &json!({"url":"example.com"})).await;
    let mut got = None; for _ in 0..100 { if let Some(v) = rx.take().into_iter().find(|v| v["t"] == "shot") { got = Some(v); break } tokio::time::sleep(Duration::from_millis(20)).await }
    assert_eq!(got.unwrap(), json!({"t":"shot","bot":"t1","n":3}));
}
#[tokio::test] async fn while_the_person_drives_the_agents_calls_never_reach_the_helper() {
    let (hub, _d) = hub_with_helper("drive").await; let b = get(&hub).unwrap();
    assert!(b.set_drive("t1", "takeover").await);
    let (ok, body) = preview(&hub, "t1", "preview_click", &json!({"x":1,"y":2})).await; assert!(!ok); assert_eq!(text(&body), REFUSAL);
    assert!(b.st.lock().await.input.is_none(), "the helper was never started for it");
    b.set_drive("t1", "").await; let (ok, _) = preview(&hub, "t1", "preview_click", &json!({"x":1,"y":2})).await; assert!(ok);
}
#[tokio::test] async fn a_helper_that_dies_fails_the_waiting_call_and_is_started_again_for_the_next() {
    let (hub, _d) = hub_with_helper("die").await; let b = get(&hub).unwrap();
    let line = b.ask("t1", "die", &json!({})).await; assert!(line.contains("\"ok\":false"), "{line}");
    let line = b.ask("t1", "status", &json!({})).await; assert!(line.contains("\"ok\":true"), "{line}");
}
#[tokio::test] async fn calls_in_flight_are_bounded() {
    let (hub, _d) = hub_with_helper("busy").await; let b = get(&hub).unwrap();
    { let mut st = b.st.lock().await; for i in 0..MAX_PENDING as u64 { st.pending.insert(1_000_000 + i, oneshot::channel().0); } }
    assert!(b.ask("t1", "status", &json!({})).await.contains("busy"));
}
#[tokio::test] async fn browser_close_is_an_ok_reply_with_or_without_a_helper() {
    let (hub, _d) = hub_with_helper("close").await; let h = hub.rpc_for("browser.close").unwrap();
    let r = h.call(&hub, &Caller { gen: "1".into(), client: 1 }, &json!(7), "browser.close", &json!({"thread":"t1"})).await.remove(0); assert_eq!(r["ok"], true);
    preview(&hub, "t1", "preview_status", &json!({})).await;
    let r = h.call(&hub, &Caller { gen: "1".into(), client: 1 }, &json!(8), "browser.close", &json!({"thread":"t1"})).await.remove(0); assert_eq!(r["ok"], true);
}
#[test] fn content_of_a_reply() {
    let (ok, b) = content(r#"{"id":1,"ok":true,"result":{"a":1}}"#); assert!(ok && b == vec![json!({"type":"text","text":"{\"a\":1}"})]);
    assert!(!content("garbage").0);
}

#[tokio::test] async fn arguments_cannot_choose_the_op_or_the_id() {
    let (hub, _d) = hub_with_helper("inject").await;
    let (ok, body) = preview(&hub, "t1", "preview_status", &json!({"op":"close","id":1,"type":"evaluate","method":"x","path":"/tmp/x"})).await;
    assert!(ok, "{body}"); let line: Value = serde_json::from_str(&text(&body)).unwrap(); let req: Value = serde_json::from_str(line["line"].as_str().unwrap()).unwrap();
    assert_eq!(req["op"], "status"); assert!(req.get("type").is_none() && req.get("method").is_none());
    let m = paged("t1", &json!({"op":"close","id":1,"type":"a","method":"b","keep":2}), true); assert_eq!(m.len(), 2); assert!(m.contains_key("page") && m.contains_key("keep"));
}
#[tokio::test] async fn tabs_events_become_info_and_others_go_to_the_teach_driver() {
    let (hub, _d) = hub_with_helper("events").await; let b = get(&hub).unwrap(); let rx = Tap::new(&hub, 1); let mut ev = b.events();
    b.line(r#"{"event":"tabs","page":"t1","tabs":[{"index":0}]}"#).await; b.line(r#"{"event":"navigated","page":"t1","url":"u"}"#).await; b.line(r#"{"event":"mystery"}"#).await;
    tokio::time::sleep(Duration::from_millis(100)).await; let info = rx.take().into_iter().find(|v| v["t"] == "info");
    assert!(info.unwrap()["info"]["browser.tabs.t1"].as_str().unwrap().contains("\"tabs\"")); assert!(b.tabs_of("t1").await.is_some()); assert!(hub.info_frame()["info"]["browser.tabs.t1"].as_str().unwrap().contains("tabs"), "a client that joins later gets it");
    assert_eq!(ev.try_recv().unwrap()["event"], "navigated"); assert!(ev.try_recv().is_err());
}
#[tokio::test] async fn set_drive_fails_closed_past_the_owner_limit() {
    let (hub, _d) = hub_with_helper("owners").await; let b = get(&hub).unwrap();
    for i in 0..MAX_OWNERS { assert!(b.set_drive(&format!("o{i}"), "teach").await) }
    assert!(!b.set_drive("one-more", "teach").await); assert!(b.set_drive("o0", "takeover").await); assert!(b.set_drive("o1", "").await); assert!(b.set_drive("one-more", "teach").await);
}

#[tokio::test] async fn open_and_navigate_bring_the_browser_tab_forward_and_other_calls_do_not() {
    let (hub, _d) = hub_with_helper("view").await; let rx = Tap::new(&hub, 1);
    let views = |rx: &Tap| -> Vec<Value> { std::thread::sleep(Duration::from_millis(100)); rx.take().into_iter().filter(|v| v["t"] == "view").collect() };
    preview(&hub, "t1", "preview_open", &json!({"url":"example.com"})).await; preview(&hub, "t1", "preview_navigate", &json!({"url":"example.org"})).await;
    let v = views(&rx); assert_eq!(v.len(), 2); assert_eq!(v[0], json!({"t":"view","thread":"t1","kind":"browser","path":""}));
    preview(&hub, "t1", "preview_snapshot", &json!({})).await; preview(&hub, "t1", "preview_click", &json!({"x":1,"y":1})).await; assert!(views(&rx).is_empty());
}

/// A download the person started in the Browser tab: the helper's file moves into the hub's home, the windows are told, and the file is there to fetch by its id.
#[tokio::test] async fn a_persons_download_is_kept_and_offered_to_the_windows() {
    let (hub, d) = hub_with_helper("dl").await; let b = get(&hub).unwrap(); let rx = Tap::new(&hub, 1);
    let src = d.join("helper-dl"); std::fs::create_dir_all(&src).unwrap(); let f = src.join("report.pdf"); std::fs::write(&f, b"%PDF bytes").unwrap();
    b.line(&json!({"event":"download","page":"t1","name":"report.pdf","path":f.to_string_lossy(),"bytes":10}).to_string()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let push = rx.take().into_iter().find(|v| v["t"] == "browser.download").expect("the windows are told");
    assert_eq!((push["bot"].as_str(), push["name"].as_str(), push["bytes"].as_u64()), (Some("t1"), Some("report.pdf"), Some(10)));
    let id = push["id"].as_str().unwrap();
    let (p, name) = b.download_path(id).await.expect("kept under its id");
    assert!(p.starts_with(hub.cfg.home.join("browser-downloads")) && name == "report.pdf"); assert_eq!(std::fs::read(&p).unwrap(), b"%PDF bytes");
    assert!(!f.exists(), "moved out of the helper's folder");
    assert!(b.download_path("d0-nope").await.is_none());
    // a failure is said, nothing kept
    b.line(&json!({"event":"download","page":"t1","name":"x.bin","error":"canceled"}).to_string()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(rx.take().iter().any(|v| v["t"] == "browser.download" && v["error"] == "canceled"));
}

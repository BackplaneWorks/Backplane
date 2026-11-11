use super::*;
use crate::presenter;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;

// ---- pure rules ------------------------------------------------------------------------------------------------
// law: the origin is the decimal `at` of the first change
#[test] fn origin_is_the_time_of_the_first_change() {
    assert_eq!(origin_of(Some(r#"{"$":"ProjectCreated","id":"p","at":1700000000}"#)), "1700000000");
    assert_eq!(origin_of(Some(r#"{"$":"SettingSet","key":"k","value":"v"}"#)), "0");
    assert_eq!(origin_of(Some("not json")), "0"); assert_eq!(origin_of(None), "0");
}
// law (Hub.request_text.k): a frame that does not parse is "malformed request" to id 0; a value without id answers to 0
#[test] fn a_frame_that_does_not_parse_is_malformed_request() {
    let bad = decode(&Message::Binary(vec![0xff, 0xfe, 0x00].into())).unwrap().unwrap_err();
    assert_eq!(bad, json!({"t":"reply","id":0,"ok":false,"error":"malformed request"}));
    let bad_text = decode(&Message::Text("{".into())).unwrap().unwrap_err(); assert_eq!(bad_text["error"], "malformed request");
    assert_eq!(decode(&Message::Text(r#"{"m":"x.y"}"#.into())).unwrap().unwrap()["id"], 0);
    assert_eq!(decode(&Message::Text(r#"{"id":"s7","m":"x.y"}"#.into())).unwrap().unwrap()["id"], "s7");
    assert_eq!(decode(&Message::Text("[1,2]".into())).unwrap().unwrap(), json!({"id":0}));   // valid JSON, not an object: unknown method, id 0
    assert!(decode(&Message::Ping(vec![].into())).is_none());
}

// ---- over a socket -----------------------------------------------------------------------------------------------
type Items = Arc<std::sync::Mutex<Vec<Value>>>;
struct Probe { items: Items, frames: Arc<std::sync::Mutex<Vec<Value>>>, log_since: Arc<std::sync::Mutex<Option<u64>>>, origin: Arc<std::sync::Mutex<String>>, lives: Arc<std::sync::Mutex<usize>>, task: tokio::task::JoinHandle<()>,
    tx: futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>, Message> }
impl Probe {
    async fn send(&mut self, v: Value) { self.tx.send(Message::Binary(cbor::encode(&v).into())).await.unwrap() }
    async fn send_raw(&mut self, b: Vec<u8>) { self.tx.send(Message::Binary(b.into())).await.unwrap() }
}
async fn join(port: u16, since: Option<(usize, String)>) -> Probe {
    let q = since.map(|(s, o)| format!("&since={s}&origin={o}")).unwrap_or_default();
    let (ws, _) = connect_async(format!("ws://127.0.0.1:{port}/ws?token=tok{q}")).await.unwrap();
    let items: Items = Default::default(); let frames: Items = Default::default(); let log_since: Arc<std::sync::Mutex<Option<u64>>> = Default::default(); let origin: Arc<std::sync::Mutex<String>> = Default::default(); let lives: Arc<std::sync::Mutex<usize>> = Default::default();
    let (i2, f2, l2, o2, v2) = (items.clone(), frames.clone(), Arc::clone(&log_since), origin.clone(), lives.clone());
    let (tx, mut rx) = ws.split();
    let task = tokio::spawn(async move {
        while let Some(Ok(m)) = rx.next().await {
            let Message::Binary(b) = m else { continue };
            let v = cbor::decode(&b).unwrap();
            match v["t"].as_str().unwrap_or("") {
                // only log lines count towards `since`; a `live` frame is a snapshot of streaming text, one item per running thread
                "log" => { *l2.lock().unwrap() = v["since"].as_u64(); *o2.lock().unwrap() = v["origin"].as_str().unwrap_or("").into(); i2.lock().unwrap().extend(v["items"].as_array().unwrap().iter().cloned()) }
                "changes" => i2.lock().unwrap().extend(v["items"].as_array().unwrap().iter().cloned()),
                "live" => *v2.lock().unwrap() += v["items"].as_array().map_or(0, Vec::len),
                _ => {}
            }
            f2.lock().unwrap().push(v);
        }
    });
    Probe { items, frames, log_since, origin, lives, task, tx }
}
fn norm(v: &Value) -> String { let mut v = v.clone(); if let Some(k) = v["key"].as_str() { if k.starts_with("fallback.input.") || k.starts_with("launch.input.") { v["value"] = json!(""); } } v.to_string() }

async fn test_hub(name: &str, log: &str) -> (hub::TestHub, u16) {
    let dir = crate::plat::tmp::dir(name);
    std::fs::write(dir.join("token"), "tok").unwrap(); std::fs::write(dir.join("events.jsonl"), log).unwrap();
    let cfg = hub::Cfg { home: dir.to_path_buf(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() };
    let hub = Hub::start(cfg, Arc::new(presenter::Canned { reply: json!({}) })).await.unwrap();
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    { let hub = hub.clone(); tokio::spawn(async move { loop { let (s, _) = l.accept().await.unwrap(); let h = hub.clone(); tokio::spawn(async move { let _ = serve(h, s).await; }); } }); }
    (hub::TestHub { hub, _tmp: dir }, port)
}
const FIXTURE: &str = "{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"p\",\"root\":\"/x\",\"at\":1700000000}\n";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn junk_and_unknown_requests_get_the_replies_gives() {
    let (_hub, port) = test_hub("wire-junk", FIXTURE).await;
    let mut c = join(port, None).await; tokio::time::sleep(Duration::from_millis(300)).await;
    c.send_raw(vec![0xff, 0xfe, 0x00]).await; tokio::time::sleep(Duration::from_millis(150)).await;
    c.send(json!({"id":2,"m":"no.such.method","p":{}})).await; tokio::time::sleep(Duration::from_millis(150)).await;
    c.send(json!({"id":3,"m":"view.open","p":{"version":1}})).await; tokio::time::sleep(Duration::from_millis(150)).await;
    c.send(json!({"m":"no.such.method"})).await; tokio::time::sleep(Duration::from_millis(150)).await;
    let replies: Vec<Value> = c.frames.lock().unwrap().iter().filter(|f| f["t"] == "reply").cloned().collect();
    assert_eq!(replies[0], json!({"t":"reply","id":0,"ok":false,"error":"malformed request"}));
    assert_eq!(replies[1], json!({"t":"reply","id":2,"ok":false,"error":"unknown method"}));
    assert_eq!(replies[2]["id"], 3); assert_eq!(replies[2]["error"]["code"], "STALE_CONNECTION");
    assert_eq!(replies[3], json!({"t":"reply","id":0,"ok":false,"error":"unknown method"}));
    assert_eq!(replies.len(), 4); c.task.abort();
}

// law (Hub.join): a request's committed changes reach the client before its reply
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_requests_changes_come_before_its_reply() {
    let (hub, port) = test_hub("wire-order", FIXTURE).await;
    crate::commands::start(hub.clone()); // project.add is the commands lane's handler
    let proj = crate::plat::tmp::dir("wire-order-proj"); // the handler refuses a folder that does not exist
    let mut c = join(port, None).await;
    // a request of no kind is answered after the join's whole backlog, so once its reply is in, the next request starts from a caught-up client (a loaded runner joins slowly)
    c.send(json!({"id":0,"m":"no.such.request"})).await; for _ in 0..3000 { if c.frames.lock().unwrap().iter().any(|f| f["t"] == "reply") { break } tokio::time::sleep(Duration::from_millis(10)).await }
    let before = c.frames.lock().unwrap().len();
    c.send(json!({"id":1,"m":"project.add","p":{"path":proj.to_string_lossy()}})).await; for _ in 0..2000 { if c.frames.lock().unwrap()[before..].iter().any(|f| f["t"] == "reply") { break } tokio::time::sleep(Duration::from_millis(10)).await } // until the reply is in, not for a clock
    let frames: Vec<Value> = c.frames.lock().unwrap()[before..].to_vec();
    let at = |t: &str| frames.iter().position(|f| f["t"] == t).unwrap_or(usize::MAX);
    // a refused request commits nothing, so there is no `changes` to come first: say the refusal, not a position of usize::MAX
    let reply = frames.iter().find(|f| f["t"] == "reply").unwrap_or_else(|| panic!("no reply to project.add: {frames:?}"));
    assert_eq!(reply["ok"], true, "project.add was refused: {reply}");
    assert!(at("changes") < at("reply"), "changes {} reply {}: {frames:?}", at("changes"), at("reply"));
    let first = frames[at("changes")]["items"].as_array().unwrap();
    assert_eq!(first[0]["$"], "ProjectCreated"); assert_eq!(first[1]["$"], "ThreadCreated");
    assert_eq!(frames[at("reply")]["ok"], true);
    // exactly once: no commit is sent twice (the request's flush and the broadcast share one cursor)
    let all: Vec<String> = c.items.lock().unwrap().iter().map(|v| v.to_string()).collect();
    assert_eq!(all.len(), all.iter().collect::<std::collections::HashSet<_>>().len()); c.task.abort();
}


// the decision alone: only the thread on show, only when its text moved, and no more often than LIVE_EVERY
#[test] fn streamed_text_tells_the_view_showing_its_thread_at_a_bounded_rate() {
    let seen = ("t1".to_string(), 10usize); let long = LIVE_EVERY * 2;
    assert_eq!(live_notice(&seen, "t1", 10, long), Tell::No, "nothing new");
    assert_eq!(live_notice(&seen, "t1", 14, long), Tell::Now, "the text grew");
    assert_eq!(live_notice(&seen, "t1", 0, long), Tell::Now, "the stream ended");
    assert_eq!(live_notice(&seen, "t1", 14, Duration::from_millis(30)), Tell::In(LIVE_EVERY - Duration::from_millis(30)), "told a moment ago: the rest of the wait");
    assert_eq!(live_notice(&seen, "", 0, long), Tell::No, "a view that shows no conversation");
    assert_eq!(live_notice(&(String::new(), 0), "t2", 0, long), Tell::No, "a thread with no stream");
    assert_eq!(live_notice(&(String::new(), 0), "t2", 5, long), Tell::Now, "a stream this view has seen none of");
}

// the steps of a streamed reply come further apart as a picture costs more (a long reply, a busy machine): one part in LIVE_SHARE of the hub's time, between bounds
#[test] fn the_gap_between_streamed_steps_follows_what_a_picture_costs() {
    let ms = Duration::from_millis;
    assert_eq!(live_gap(Duration::ZERO), LIVE_EVERY, "no picture yet");
    assert_eq!(live_gap(ms(5)), LIVE_EVERY, "a cheap picture: ten steps a second");
    assert_eq!(live_gap(ms(60)), ms(480), "a 60 ms picture: two steps a second");
    assert_eq!(live_gap(ms(900)), LIVE_SLOWEST, "never further apart than this");
    let seen = ("t1".to_string(), 10usize);
    assert_eq!(live_notice_every(&seen, "t1", 14, ms(300), live_gap(ms(60))), Tell::In(ms(180)));
    assert_eq!(live_notice_every(&seen, "t1", 14, ms(480), live_gap(ms(60))), Tell::Now);
}

/// A view stream on `thread`: opened, its frames collected.
async fn view_on(port: u16, thread: &str) -> (Items, tokio::task::JoinHandle<()>) {
    let (ws, _) = connect_async(format!("ws://127.0.0.1:{port}/ws?view=1&mobile=1&enc=cbor&token=tok")).await.unwrap();
    let (mut tx, mut rx) = ws.split();
    let g = loop { if let Some(Ok(Message::Binary(b))) = rx.next().await { break cbor::decode(&b).unwrap() } };
    assert_eq!(g["t"], "view", "the greeting: {g}");
    let open = json!({"id":(1u64 << 47) + 1,"m":"view.open","p":{"version":1,"generation":g["generation"],"epoch":g["epoch"],"hub":"h","requestRevision":1,"target":{"kind":"thread","id":thread}}});
    tx.send(Message::Binary(cbor::encode(&open).into())).await.unwrap();
    let frames: Items = Default::default(); let f2 = frames.clone();
    let task = tokio::spawn(async move { let _tx = tx; while let Some(Ok(m)) = rx.next().await { if let Message::Binary(b) = m { f2.lock().unwrap().push(cbor::decode(&b).unwrap()); } } });
    for _ in 0..3000 { if frames.lock().unwrap().iter().any(|f| f["t"] == "reply") { break } tokio::time::sleep(Duration::from_millis(10)).await }
    (frames, task)
}

// A reply being streamed is no log line, so the revision stands still while it is written. The view showing that thread is told all the same
// (it was told only at the next commit: a streamed reply reached the desktop in five-second steps); a view of another thread is not.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_view_is_told_of_text_streamed_in_the_thread_it_shows_and_in_no_other() {
    let (hub, port) = test_hub("wire-live", FIXTURE).await;
    let ((a, ta), (b, tb)) = (view_on(port, "t1").await, view_on(port, "t2").await);
    let told = |f: &Items| f.lock().unwrap().iter().filter(|f| f["t"] == "view.changed").count();
    tokio::time::sleep(Duration::from_millis(400)).await;   // the start-up commits have been told
    let (a0, b0) = (told(&a), told(&b));
    hub.live_delta("t1", "The pour").await.unwrap();
    for _ in 0..500 { if told(&a) > a0 { break } tokio::time::sleep(Duration::from_millis(10)).await }
    assert_eq!(told(&a), a0 + 1, "the view on t1 is told of its streamed text");
    // more text, faster than a view is told: a few notices, not one per piece
    for i in 0..20 { hub.live_delta("t1", &format!(" goes {i}")).await.unwrap(); tokio::time::sleep(Duration::from_millis(10)).await; }
    tokio::time::sleep(Duration::from_millis(400)).await;
    let more = told(&a) - a0 - 1;
    assert!((1..=6).contains(&more), "20 pieces in 0.2 s: told {more} times");
    assert_eq!(told(&b), b0, "a view on another thread hears nothing of it");
    ta.abort(); tb.abort();
}

// origin follows the log (read at each join), and a client holding another origin, "0" included, is sent the whole log.
// (Hub::start's bootstrap writes housekeeping events, so a started hub's log is never empty; the origin is still read per join.)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn origin_is_read_at_each_join_and_a_zero_resumes_from_nothing() {
    let (hub, port) = test_hub("wire-origin", "").await;
    let first_at = |h: &Hub| h.store.get(0).ok().and_then(|l| serde_json::from_str::<Value>(&l).ok()).and_then(|v| v["at"].as_u64()).unwrap();
    let a = join(port, None).await; tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(*a.origin.lock().unwrap(), first_at(&hub).to_string());
    hub.commit(&json!({"$":"ProjectCreated","id":"p9","title":"p","root":"/x","at":1700000042})).await.unwrap();
    let b = join(port, None).await; tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(*b.origin.lock().unwrap(), first_at(&hub).to_string(), "a join reports the log's first change's at");
    assert_eq!(origin(&hub), first_at(&hub).to_string());
    let n = hub.store.len();
    let r = join(port, Some((n, "0".into()))).await; tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(*r.log_since.lock().unwrap(), Some(0), "origin 0 is not this log's origin");
    assert_eq!(r.items.lock().unwrap().len(), n);
    let ok = join(port, Some((n, origin(&hub)))).await; tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(*ok.log_since.lock().unwrap(), Some(n as u64)); assert_eq!(ok.items.lock().unwrap().len(), 0);
    a.task.abort(); b.task.abort(); r.task.abort(); ok.task.abort();
}

// side-by-side (docs/SIDE-BY-SIDE.md): /hello and the info carry `sideBySide` only on a copy with the flag. The switch is read once per process,
// so each case is a child process of this test binary with the environment as given.
#[test] fn side_by_side_probe_child() {
    let Ok(mode) = std::env::var("BP_SBS_WIRE_PROBE") else { return };
    let on = crate::copy::person_turn_allowed();
    assert_eq!(on, mode == "on");
    let (yes, one) = (json!(true), json!("1"));
    let want_bool = if mode == "on" { Some(&yes) } else { None };
    let want_text = if mode == "on" { Some(&one) } else { None };
    let h = hello_body("0.14.0", "c", on);
    assert_eq!(h.get("sideBySide"), want_bool, "{h}");
    assert_eq!(h["backplane"], "0.14.0"); assert_eq!(h["busy"], 0);
    let mut info = serde_json::Map::new(); side_by_side_info(&mut info);
    assert_eq!(info.get("sideBySide"), want_text);
    let shown = shown_with_side_by_side(json!({"projects":[]}));
    assert_eq!(shown.get("sideBySide"), want_bool, "{shown}");
    println!("wire_sbs_ok {mode}");
}
fn sbs_wire_child(mode: &str, copy: Option<&str>, flag: Option<&str>) -> String {
    let mut c = std::process::Command::new(std::env::current_exe().unwrap());
    c.args(["wire::tests::side_by_side_probe_child", "--exact", "--nocapture", "--test-threads=1"]).env("BP_SBS_WIRE_PROBE", mode);
    match copy { Some(v) => { c.env("BACKPLANE_COPY", v); } None => { c.env_remove("BACKPLANE_COPY"); } }
    match flag { Some(v) => { c.env("BACKPLANE_SIDE_BY_SIDE", v); } None => { c.env_remove("BACKPLANE_SIDE_BY_SIDE"); } }
    let o = c.output().unwrap(); assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)); String::from_utf8_lossy(&o.stdout).into_owned()
}
#[test] fn hello_and_info_say_side_by_side_only_on_a_copy_with_the_flag() {
    assert!(sbs_wire_child("on", Some("1"), Some("1")).contains("wire_sbs_ok on"));
    assert!(sbs_wire_child("off", Some("1"), None).contains("wire_sbs_ok off"), "a copy without the flag");
    assert!(sbs_wire_child("off", None, None).contains("wire_sbs_ok off"), "no flags");
    assert!(sbs_wire_child("off", None, Some("1")).contains("wire_sbs_ok off"), "the flag without the copy switch");
    assert!(sbs_wire_child("off", Some("1"), Some("0")).contains("wire_sbs_ok off"), "a wrong flag value");
}

#[test] fn shown_is_marked_only_when_on_and_only_an_object() {
    assert_eq!(shown_marked(json!({"a":1}), true), json!({"a":1,"sideBySide":true}));
    assert_eq!(shown_marked(json!({"a":1}), false), json!({"a":1}));
    assert_eq!(shown_marked(Value::Null, true), Value::Null);
}
#[test] fn the_landing_page_names_the_machine_and_the_ways_in_and_escapes() {
    let p = landing_page("desk<b>", "1.2.3");
    assert!(p.contains("This is Backplane on desk&lt;b&gt;") && p.contains("1.2.3"), "{p}");
    for w in ["Settings &rarr; Machines", "Settings &rarr; Hubs &rarr; Add a hub", "Settings &rarr; Devices"] { assert!(p.contains(w), "{w}") }
    assert!(!p.contains("token"), "no secret words on the page");
    assert!(landing_page("", "1").contains("this machine"));
}

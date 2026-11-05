use super::*;
use crate::presenter;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;

// ---- pure rules ------------------------------------------------------------------------------------------------
// law (Bend Hub.origin): the origin is the decimal `at` of the first change
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

async fn test_hub(name: &str, log: &str) -> (Arc<Hub>, u16) {
    let dir = std::env::temp_dir().join(format!("bp-{name}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("token"), "tok").unwrap(); std::fs::write(dir.join("events.jsonl"), log).unwrap();
    let cfg = hub::Cfg { home: dir.clone(), version: "0.14.0".into(), commit: "c".into(), listen: "x".into(), os: "linux".into(), arch: "x64".into() };
    let hub = Hub::start(cfg, Arc::new(presenter::Canned { reply: json!({}) })).await.unwrap();
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap(); let port = l.local_addr().unwrap().port();
    { let hub = hub.clone(); tokio::spawn(async move { loop { let (s, _) = l.accept().await.unwrap(); let h = hub.clone(); tokio::spawn(async move { let _ = serve(h, s).await; }); } }); }
    (hub, port)
}
const FIXTURE: &str = "{\"$\":\"ProjectCreated\",\"id\":\"p1\",\"title\":\"p\",\"root\":\"/x\",\"at\":1700000000}\n";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn junk_and_unknown_requests_get_the_replies_bend_gives() {
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
    let (_hub, port) = test_hub("wire-order", FIXTURE).await;
    let mut c = join(port, None).await; tokio::time::sleep(Duration::from_millis(300)).await;
    let before = c.frames.lock().unwrap().len();
    c.send(json!({"id":1,"m":"project.add","p":{"path":"/tmp/wire-order-proj"}})).await; tokio::time::sleep(Duration::from_millis(400)).await;
    let frames: Vec<Value> = c.frames.lock().unwrap()[before..].to_vec();
    let at = |t: &str| frames.iter().position(|f| f["t"] == t).unwrap_or(usize::MAX);
    assert!(at("changes") < at("reply"), "changes {} reply {}", at("changes"), at("reply"));
    let first = frames[at("changes")]["items"].as_array().unwrap();
    assert_eq!(first[0]["$"], "ProjectCreated"); assert_eq!(first[1]["$"], "ThreadCreated");
    assert_eq!(frames[at("reply")]["ok"], true);
    // exactly once: no commit is sent twice (the request's flush and the broadcast share one cursor)
    let all: Vec<String> = c.items.lock().unwrap().iter().map(|v| v.to_string()).collect();
    assert_eq!(all.len(), all.iter().collect::<std::collections::HashSet<_>>().len()); c.task.abort();
}


// Bend's origin follows the log (read at each join), and a client holding another origin, "0" included, is sent the whole log.
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
